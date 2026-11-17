//! The Ember checker: resolves names and checks types, and records what it learns in tables keyed by node id.

use std::collections::{HashMap, HashSet};

use ast::{NodeId, Program};
use source::{Diagnostic, Span};

mod expr;
mod ints;
mod items;
mod scope;
mod stmt;
mod types;

pub use types::{TypeId, TypeKind, TypeTable};

/// The name of the built-in print function.
const PRINT: &str = "print";

/// What a name or an assignment target refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// A parameter, a `let` or a loop variable, by the id of the node that declares it.
    Local(NodeId),
    /// A user function, by name.
    Function(String),
    /// The built-in `print`.
    Print,
    /// A conversion `T(x)` to the integer type `T`.
    Convert(TypeId),
}

/// The parameter types and the return type of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// The parameter types, in order.
    pub params: Vec<TypeId>,
    /// The return type, `TypeId::UNIT` when the function has none.
    pub ret: TypeId,
}

/// Everything the checker learned about a valid program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Checked {
    /// Every type the program uses.
    pub types: TypeTable,
    /// The type of every expression, except a range directly after `in` in a `for`.
    pub expr_types: HashMap<NodeId, TypeId>,
    /// What every name expression, assignment statement and call expression refers to.
    pub bindings: HashMap<NodeId, Binding>,
    /// The type of every parameter, `let` and loop variable, by its declaring node id.
    pub local_types: HashMap<NodeId, TypeId>,
    /// The signature of every user function.
    pub signatures: HashMap<String, Signature>,
}

/// Checks a program, returning what was learned or every error found, sorted by position.
pub fn check(program: &Program) -> Result<Checked, Vec<Diagnostic>> {
    let mut checker = Checker::new();
    checker.check_program(program);
    checker.finish()
}

/// The type of an expression while checking: a real type, or the marker of an earlier error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    /// A type of the language.
    Known(TypeId),
    /// The expression already has an error. It matches every type, so no second error follows.
    Error,
}

impl Ty {
    /// The `i64` type.
    const I64: Ty = Ty::Known(TypeId::I64);
    /// An integer that has not taken its type yet.
    const UNTYPED: Ty = Ty::Known(TypeId::UNTYPED_INT);
    /// The `bool` type.
    const BOOL: Ty = Ty::Known(TypeId::BOOL);
    /// The type of no value.
    const UNIT: Ty = Ty::Known(TypeId::UNIT);

    /// Returns the language type, or `None` for the error marker.
    fn known(self) -> Option<TypeId> {
        match self {
            Ty::Known(ty) => Some(ty),
            Ty::Error => None,
        }
    }
}

/// A function signature while checking, where a type may be the error marker.
#[derive(Debug, Clone)]
struct FnSig {
    /// The parameter types, in order.
    params: Vec<Ty>,
    /// The return type.
    ret: Ty,
}

impl FnSig {
    /// Tells whether any type of the signature is the error marker.
    fn has_error(&self) -> bool {
        self.ret == Ty::Error || self.params.contains(&Ty::Error)
    }

    /// Returns the public signature, or `None` when any type is the error marker.
    fn to_public(&self) -> Option<Signature> {
        let mut params = Vec::new();
        for param in &self.params {
            params.push(param.known()?);
        }
        Some(Signature {
            params,
            ret: self.ret.known()?,
        })
    }
}

/// A parameter, `let` or loop variable that is in scope.
#[derive(Debug, Clone, Copy)]
struct LocalInfo {
    /// The id of the declaring node.
    id: NodeId,
    /// The type of the variable.
    ty: Ty,
}

/// A `let` that is reported when nothing ever reads it.
struct LetDecl {
    /// The id of the `let` statement.
    id: NodeId,
    /// The variable name.
    name: String,
    /// Where the name appears.
    span: Span,
}

/// Walks the program once, collecting results and errors.
struct Checker {
    /// The user functions by name; for a duplicated name, the first one.
    functions: HashMap<String, FnSig>,
    /// The errors found so far.
    diagnostics: Vec<Diagnostic>,
    /// The results found so far.
    checked: Checked,
    /// The scopes open right now, outermost first.
    scopes: Vec<HashMap<String, LocalInfo>>,
    /// The ids of the locals that were read.
    used: HashSet<NodeId>,
    /// The `let`s that must be read somewhere.
    lets: Vec<LetDecl>,
    /// The name of the function being checked.
    fn_name: String,
    /// The return type of the function being checked.
    fn_ret: Ty,
    /// How many loops enclose the statement being checked.
    loop_depth: usize,
}

impl Checker {
    /// Creates a checker that has not seen any code.
    fn new() -> Checker {
        Checker {
            functions: HashMap::new(),
            diagnostics: Vec::new(),
            checked: Checked::default(),
            scopes: Vec::new(),
            used: HashSet::new(),
            lets: Vec::new(),
            fn_name: String::new(),
            fn_ret: Ty::UNIT,
            loop_depth: 0,
        }
    }

    /// Reports the unused `let`s, then returns the results or the sorted errors.
    fn finish(mut self) -> Result<Checked, Vec<Diagnostic>> {
        self.report_unused_lets();
        if self.diagnostics.is_empty() {
            return Ok(self.checked);
        }
        // The sort is stable, so errors that start at the same place keep their order.
        self.diagnostics
            .sort_by_key(|diagnostic| diagnostic.span.start);
        Err(self.diagnostics)
    }

    /// The source spelling of a type, for messages.
    fn type_name(&self, ty: TypeId) -> String {
        self.checked.types.name(ty)
    }

    /// Records an error.
    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.push(Diagnostic::error(message, span));
    }

    /// Reports every `let` that was never read.
    fn report_unused_lets(&mut self) {
        let lets = std::mem::take(&mut self.lets);
        for decl in lets {
            if !self.used.contains(&decl.id) {
                let message = format!("`{}` is declared but never used", decl.name);
                self.error(message, decl.span);
            }
        }
    }

    /// Reports a type mismatch; returns false when it did. The error marker matches every type.
    fn expect(&mut self, expected: Ty, found: Ty, span: Span) -> bool {
        match (expected, found) {
            (Ty::Known(expected), Ty::Known(found)) if expected != found => {
                let expected = self.type_name(expected);
                let found = self.type_name(found);
                self.error(format!("expected `{expected}`, found `{found}`"), span);
                false
            }
            _ => true,
        }
    }

    /// Reports an error that is a bug of the compiler, not of the program.
    fn internal_error(&mut self, what: &str, span: Span) {
        self.error(format!("internal compiler error: {what}"), span);
    }

    /// Reports a value that is `()`; returns false when it did.
    fn require_value(&mut self, ty: Ty, span: Span) -> bool {
        if ty == Ty::UNIT {
            self.error("this expression has no value", span);
            return false;
        }
        true
    }

    /// Reports a wrong number of arguments; returns false when it did.
    fn check_arity(&mut self, name: &str, expected: usize, given: usize, span: Span) -> bool {
        if expected == given {
            return true;
        }
        let noun = if expected == 1 {
            "argument"
        } else {
            "arguments"
        };
        let message = format!("`{name}` takes {expected} {noun}, but {given} were given");
        self.error(message, span);
        false
    }
}
