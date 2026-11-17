//! The Ember checker: resolves names and checks types, and records what it learns in tables keyed by node id.

use std::collections::{HashMap, HashSet};
use std::fmt;

use ast::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ExprKind, ForKind, Function, Ident, NodeId,
    Program, Stmt, StmtKind, TypeExpr, UnaryOp,
};
use source::{Diagnostic, Span};

/// The name of the built-in print function.
const PRINT: &str = "print";

/// The type names from the language spec that Phase 1 does not support yet.
const UNSUPPORTED_TYPES: [&str; 12] = [
    "i8", "i16", "i32", "u8", "u16", "u32", "u64", "f16", "bf16", "f32", "f64", "str",
];

/// The largest value of an `i64` literal.
const I64_MAX_MAGNITUDE: u64 = i64::MAX.unsigned_abs();

/// The magnitude of `i64::MIN`, which is only allowed directly under a unary minus.
const I64_MIN_MAGNITUDE: u64 = i64::MIN.unsigned_abs();

/// A type of the Phase 1 language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// A 64-bit signed integer.
    I64,
    /// A boolean.
    Bool,
    /// No value, the result of a call to a function without a return type.
    Unit,
}

impl fmt::Display for Type {
    /// Writes the type as it is spelled in source, and `()` for no value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spelling = match self {
            Type::I64 => "i64",
            Type::Bool => "bool",
            Type::Unit => "()",
        };
        write!(f, "{spelling}")
    }
}

/// What a name or an assignment target refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// A parameter, a `let` or a loop variable, by the id of the node that declares it.
    Local(NodeId),
    /// A user function, by name.
    Function(String),
    /// The built-in `print`.
    Print,
}

/// The parameter types and the return type of a function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// The parameter types, in order.
    pub params: Vec<Type>,
    /// The return type, `Unit` when the function has none.
    pub ret: Type,
}

/// Everything the checker learned about a valid program.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Checked {
    /// The type of every expression, except a range directly after `in` in a `for`.
    pub expr_types: HashMap<NodeId, Type>,
    /// What every name expression, assignment statement and call expression refers to.
    pub bindings: HashMap<NodeId, Binding>,
    /// The type of every parameter, `let` and loop variable, by its declaring node id.
    pub local_types: HashMap<NodeId, Type>,
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
    Known(Type),
    /// The expression already has an error. It matches every type, so no second error follows.
    Error,
}

impl Ty {
    /// The `i64` type.
    const I64: Ty = Ty::Known(Type::I64);
    /// The `bool` type.
    const BOOL: Ty = Ty::Known(Type::Bool);
    /// The type of no value.
    const UNIT: Ty = Ty::Known(Type::Unit);

    /// Returns the language type, or `None` for the error marker.
    fn known(self) -> Option<Type> {
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

/// An argument of a call, checked before the callee is looked at.
struct Argument {
    /// The type of the argument.
    ty: Ty,
    /// Where the argument appears.
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

    /// Checks every function of the program.
    fn check_program(&mut self, program: &Program) {
        let signatures = self.collect_signatures(program);
        self.check_main(program, &signatures);
        for (function, signature) in program.functions.iter().zip(&signatures) {
            self.check_function(function, signature);
        }
    }

    /// Resolves every signature first, so functions can be called before they are defined.
    fn collect_signatures(&mut self, program: &Program) -> Vec<FnSig> {
        let mut signatures = Vec::new();
        for function in &program.functions {
            let signature = self.resolve_signature(function);
            let name = &function.name.name;

            if self.functions.contains_key(name) {
                let message = format!("function `{name}` is already defined");
                self.error(message, function.name.span);
            } else {
                self.functions.insert(name.clone(), signature.clone());
                if let Some(public) = signature.to_public() {
                    self.checked.signatures.insert(name.clone(), public);
                }
            }
            signatures.push(signature);
        }
        signatures
    }

    /// Resolves the types of a function header.
    fn resolve_signature(&mut self, function: &Function) -> FnSig {
        let mut params = Vec::new();
        for param in &function.params {
            params.push(self.resolve_type(&param.ty));
        }

        let ret = match &function.return_type {
            Some(type_expr) => self.resolve_type(type_expr),
            None => Ty::UNIT,
        };
        FnSig { params, ret }
    }

    /// Turns a written type into a type, reporting names that are not usable types.
    fn resolve_type(&mut self, type_expr: &TypeExpr) -> Ty {
        let name = type_expr.name.name.as_str();
        match name {
            "i64" => Ty::I64,
            "bool" => Ty::BOOL,
            _ if UNSUPPORTED_TYPES.contains(&name) => {
                self.error(
                    format!("type `{name}` is not supported yet"),
                    type_expr.span,
                );
                Ty::Error
            }
            _ => {
                self.error(format!("unknown type `{name}`"), type_expr.span);
                Ty::Error
            }
        }
    }

    /// Checks that `main` exists and takes and returns nothing.
    fn check_main(&mut self, program: &Program, signatures: &[FnSig]) {
        let mut main = None;
        for (function, signature) in program.functions.iter().zip(signatures) {
            if function.name.name == "main" {
                main = Some((function, signature));
                break;
            }
        }

        match main {
            None => self.error("missing `fn main()`", Span::new(0, 0)),
            Some((function, signature)) => {
                let breaks_rule = !function.params.is_empty() || function.return_type.is_some();
                // A type error in the header has been reported already, so it is the only error.
                if breaks_rule && !signature.has_error() {
                    self.error(
                        "`main` must take no parameters and return nothing",
                        function.name.span,
                    );
                }
            }
        }
    }

    /// Checks the body of one function and that it returns on every path.
    fn check_function(&mut self, function: &Function, signature: &FnSig) {
        self.fn_name = function.name.name.clone();
        self.fn_ret = signature.ret;
        self.loop_depth = 0;

        self.push_scope();
        for (param, ty) in function.params.iter().zip(&signature.params) {
            self.declare(&param.name, param.id, *ty);
        }
        self.check_block(&function.body);
        self.pop_scope();

        self.check_missing_return(function, signature.ret);
    }

    /// Reports a function with a return type whose body can finish without returning.
    fn check_missing_return(&mut self, function: &Function, ret: Ty) {
        let needs_value = match ret {
            Ty::Known(ty) => ty != Type::Unit,
            // An unknown return type has been reported already.
            Ty::Error => false,
        };
        if needs_value && !block_always_returns(&function.body) {
            let message = format!(
                "function `{}` must return a value on every path",
                function.name.name
            );
            self.error(message, function.name.span);
        }
    }

    /// Opens a new scope.
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Closes the innermost scope.
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Finds a visible parameter or local by name.
    fn lookup_local(&self, name: &str) -> Option<LocalInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.get(name) {
                return Some(*local);
            }
        }
        None
    }

    /// Tells whether a name is a user function or the built-in `print`.
    fn is_function(&self, name: &str) -> bool {
        self.functions.contains_key(name) || name == PRINT
    }

    /// Declares a parameter, `let` or loop variable; true means valid (and so checked for unused).
    fn declare(&mut self, name: &Ident, id: NodeId, ty: Ty) -> bool {
        // `_` is never added to a scope, so it can be declared any number of times.
        if name.name == "_" {
            self.record_local_type(id, ty);
            return true;
        }

        let local_taken = self.lookup_local(&name.name).is_some();
        let function_taken = self.functions.contains_key(&name.name);
        if local_taken || function_taken {
            let message = format!("`{}` is already declared", name.name);
            self.error(message, name.span);
        }
        if local_taken {
            return false;
        }

        self.record_local_type(id, ty);
        // A local that clashes with a function is still added, so reading it is not a second error.
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.name.clone(), LocalInfo { id, ty });
        }
        !function_taken
    }

    /// Stores the type of a declared local, unless it is the error marker.
    fn record_local_type(&mut self, id: NodeId, ty: Ty) {
        if let Ty::Known(known) = ty {
            self.checked.local_types.insert(id, known);
        }
    }

    /// Stores the type of an expression, unless it is the error marker.
    fn record_expr_type(&mut self, id: NodeId, ty: Ty) {
        if let Ty::Known(known) = ty {
            self.checked.expr_types.insert(id, known);
        }
    }

    /// Stores what a name, assignment or call refers to.
    fn record_binding(&mut self, id: NodeId, binding: Binding) {
        self.checked.bindings.insert(id, binding);
    }

    /// Reports a type mismatch; returns false when it did. The error marker matches every type.
    fn expect(&mut self, expected: Ty, found: Ty, span: Span) -> bool {
        match (expected, found) {
            (Ty::Known(expected), Ty::Known(found)) if expected != found => {
                self.error(format!("expected `{expected}`, found `{found}`"), span);
                false
            }
            _ => true,
        }
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

    /// Checks the statements of a block in a new scope.
    fn check_block(&mut self, block: &Block) {
        self.push_scope();
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        self.pop_scope();
    }

    /// Checks one statement.
    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => self.check_let(stmt, name, ty.as_ref(), value),
            StmtKind::Assign { target, op, value } => self.check_assign(stmt, target, *op, value),
            StmtKind::If {
                cond,
                then_block,
                else_branch,
            } => self.check_if(cond, then_block, else_branch.as_ref()),
            StmtKind::For { kind, body } => self.check_for(stmt, kind, body),
            StmtKind::Return(value) => self.check_return(stmt, value.as_ref()),
            StmtKind::Break => self.check_inside_loop(stmt, "break"),
            StmtKind::Continue => self.check_inside_loop(stmt, "continue"),
            StmtKind::Expr(expr) => self.check_expr_stmt(expr),
        }
    }

    /// Checks `x := e` and `x: T = e`.
    fn check_let(
        &mut self,
        stmt: &Stmt,
        name: &Ident,
        annotation: Option<&TypeExpr>,
        value: &Expr,
    ) {
        // The value is checked before the name is declared, so `x := x` does not see itself.
        let value_ty = self.check_expr(value);

        let local_ty = match annotation {
            Some(type_expr) => {
                let declared = self.resolve_type(type_expr);
                if self.require_value(value_ty, value.span) {
                    self.expect(declared, value_ty, value.span);
                }
                declared
            }
            None => {
                if self.require_value(value_ty, value.span) {
                    value_ty
                } else {
                    Ty::Error
                }
            }
        };

        let declared = self.declare(name, stmt.id, local_ty);
        // A local whose value had an error is not reported as unused, to avoid a follow-on error.
        if declared && name.name != "_" && local_ty != Ty::Error {
            self.lets.push(LetDecl {
                id: stmt.id,
                name: name.name.clone(),
                span: name.span,
            });
        }
    }

    /// Checks an assignment, which writes a local but does not read it.
    fn check_assign(&mut self, stmt: &Stmt, target: &Ident, op: AssignOp, value: &Expr) {
        let value_ty = self.check_expr(value);

        if op == AssignOp::MatMul {
            self.error(format!("`{op}` is not supported yet"), stmt.span);
            return;
        }

        let target_ty = match self.resolve_assign_target(stmt, target) {
            Some(ty) => ty,
            None => return,
        };

        match op {
            AssignOp::Assign => {
                self.expect(target_ty, value_ty, value.span);
            }
            _ => {
                // A bad target is reported alone; the value is only checked when the target is fine.
                if self.expect(Ty::I64, target_ty, target.span) {
                    self.expect(Ty::I64, value_ty, value.span);
                }
            }
        }
    }

    /// Finds the local an assignment writes, reporting targets that are not locals.
    fn resolve_assign_target(&mut self, stmt: &Stmt, target: &Ident) -> Option<Ty> {
        let name = target.name.as_str();
        if name == "_" {
            self.error("cannot assign to `_`", target.span);
            return None;
        }

        if let Some(local) = self.lookup_local(name) {
            self.record_binding(stmt.id, Binding::Local(local.id));
            return Some(local.ty);
        }

        if self.is_function(name) {
            self.error(format!("cannot assign to function `{name}`"), target.span);
        } else {
            self.error(format!("undefined name `{name}`"), target.span);
        }
        None
    }

    /// Checks an `if`, its blocks and its `else` part.
    fn check_if(&mut self, cond: &Expr, then_block: &Block, else_branch: Option<&ElseBranch>) {
        self.check_condition(cond);
        self.check_block(then_block);
        match else_branch {
            Some(ElseBranch::Block(block)) => self.check_block(block),
            Some(ElseBranch::If(nested)) => self.check_stmt(nested),
            None => {}
        }
    }

    /// Checks that a condition is a `bool`.
    fn check_condition(&mut self, cond: &Expr) {
        let ty = self.check_expr(cond);
        self.expect(Ty::BOOL, ty, cond.span);
    }

    /// Checks the three forms of `for` loop.
    fn check_for(&mut self, stmt: &Stmt, kind: &ForKind, body: &Block) {
        match kind {
            ForKind::Infinite => self.check_loop_body(body),
            ForKind::While(cond) => {
                self.check_condition(cond);
                self.check_loop_body(body);
            }
            ForKind::In { var, iterable } => {
                // The range is checked before the loop variable exists, so it cannot use it.
                self.check_iterable(iterable);
                self.push_scope();
                self.declare(var, stmt.id, Ty::I64);
                self.check_loop_body(body);
                self.pop_scope();
            }
        }
    }

    /// Checks the body of a loop, where `break` and `continue` are allowed.
    fn check_loop_body(&mut self, body: &Block) {
        self.loop_depth += 1;
        self.check_block(body);
        self.loop_depth -= 1;
    }

    /// Checks the value after `in`, which must be a range written right there.
    fn check_iterable(&mut self, iterable: &Expr) {
        match &iterable.kind {
            ExprKind::Binary {
                op: BinaryOp::Range | BinaryOp::RangeInclusive,
                left,
                right,
            } => self.check_operands(Ty::I64, left, right),
            _ => {
                let errors_before = self.diagnostics.len();
                self.check_expr(iterable);
                if self.diagnostics.len() == errors_before {
                    self.error("only ranges can be iterated over yet", iterable.span);
                }
            }
        }
    }

    /// Checks `break` and `continue`, which only make sense inside a loop.
    fn check_inside_loop(&mut self, stmt: &Stmt, keyword: &str) {
        if self.loop_depth == 0 {
            self.error(format!("`{keyword}` outside of a loop"), stmt.span);
        }
    }

    /// Checks `return` against the return type of the current function.
    fn check_return(&mut self, stmt: &Stmt, value: Option<&Expr>) {
        match value {
            None => self.check_bare_return(stmt),
            Some(expr) => self.check_return_value(expr),
        }
    }

    /// Checks a `return` without a value, which a function with a return type may not use.
    fn check_bare_return(&mut self, stmt: &Stmt) {
        if let Ty::Known(ty) = self.fn_ret
            && ty != Type::Unit
        {
            let message = format!("expected a return value of type `{ty}`");
            self.error(message, stmt.span);
        }
    }

    /// Checks the value of `return e` against the return type of the current function.
    fn check_return_value(&mut self, expr: &Expr) {
        let ty = self.check_expr(expr);
        if self.fn_ret != Ty::UNIT {
            self.expect(self.fn_ret, ty, expr.span);
        } else if ty != Ty::Error {
            let message = format!("function `{}` does not return a value", self.fn_name);
            self.error(message, expr.span);
        }
    }

    /// Checks an expression used as a statement, which must be a call.
    fn check_expr_stmt(&mut self, expr: &Expr) {
        // Any error inside the expression already explains the problem, so no second one is added.
        let errors_before = self.diagnostics.len();
        self.check_expr(expr);
        let is_call = matches!(expr.kind, ExprKind::Call { .. });
        if !is_call && self.diagnostics.len() == errors_before {
            self.error("only calls can be used as statements", expr.span);
        }
    }

    /// Checks an expression and records its type.
    fn check_expr(&mut self, expr: &Expr) -> Ty {
        let ty = match &expr.kind {
            ExprKind::Int(value) => self.check_int(*value, expr.span),
            ExprKind::Bool(_) => Ty::BOOL,
            ExprKind::Name(name) => self.check_name(expr, name),
            ExprKind::Unary { op, operand } => self.check_unary(expr, *op, operand),
            ExprKind::Binary { op, left, right } => self.check_binary(expr, *op, left, right),
            ExprKind::Call { callee, args } => self.check_call(expr, callee, args),
        };
        self.record_expr_type(expr.id, ty);
        ty
    }

    /// Checks that an integer literal fits in an `i64`.
    fn check_int(&mut self, value: u64, span: Span) -> Ty {
        if value > I64_MAX_MAGNITUDE {
            self.error("integer literal does not fit in `i64`", span);
            return Ty::Error;
        }
        Ty::I64
    }

    /// Resolves a name that is read as a value.
    fn check_name(&mut self, expr: &Expr, name: &str) -> Ty {
        if name == "_" {
            self.error("`_` cannot be used as a value", expr.span);
            return Ty::Error;
        }

        if let Some(local) = self.lookup_local(name) {
            self.used.insert(local.id);
            self.record_binding(expr.id, Binding::Local(local.id));
            return local.ty;
        }

        if self.is_function(name) {
            self.error(format!("`{name}` is a function, not a value"), expr.span);
        } else {
            self.error(format!("undefined name `{name}`"), expr.span);
        }
        Ty::Error
    }

    /// Checks a prefix operator.
    fn check_unary(&mut self, expr: &Expr, op: UnaryOp, operand: &Expr) -> Ty {
        match op {
            UnaryOp::Neg => {
                let operand_ty = self.check_negated_operand(operand);
                self.expect(Ty::I64, operand_ty, operand.span);
                Ty::I64
            }
            UnaryOp::Not => {
                let operand_ty = self.check_expr(operand);
                self.expect(Ty::BOOL, operand_ty, operand.span);
                Ty::BOOL
            }
            UnaryOp::BitNot | UnaryOp::AddrOf | UnaryOp::Deref => {
                self.check_expr(operand);
                self.error(format!("`{op}` is not supported yet"), expr.span);
                Ty::Error
            }
        }
    }

    /// Checks the operand of a unary minus, where the literal 9223372036854775808 is `i64::MIN`.
    fn check_negated_operand(&mut self, operand: &Expr) -> Ty {
        if operand.kind == ExprKind::Int(I64_MIN_MAGNITUDE) {
            self.record_expr_type(operand.id, Ty::I64);
            return Ty::I64;
        }
        self.check_expr(operand)
    }

    /// Checks an infix operator.
    fn check_binary(&mut self, expr: &Expr, op: BinaryOp, left: &Expr, right: &Expr) -> Ty {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                self.check_operands(Ty::I64, left, right);
                Ty::I64
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                self.check_operands(Ty::I64, left, right);
                Ty::BOOL
            }
            BinaryOp::And | BinaryOp::Or => {
                self.check_operands(Ty::BOOL, left, right);
                Ty::BOOL
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                self.check_comparison(expr, left, right);
                Ty::BOOL
            }
            BinaryOp::Range | BinaryOp::RangeInclusive => {
                self.check_expr(left);
                self.check_expr(right);
                self.error("ranges can only be used in `for` loops", expr.span);
                Ty::Error
            }
            BinaryOp::MatMul
            | BinaryOp::Shl
            | BinaryOp::Shr
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor => {
                self.check_expr(left);
                self.check_expr(right);
                self.error(format!("`{op}` is not supported yet"), expr.span);
                Ty::Error
            }
        }
    }

    /// Checks that both operands have the expected type, reporting only the first that does not.
    fn check_operands(&mut self, expected: Ty, left: &Expr, right: &Expr) {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        if self.expect(expected, left_ty, left.span) {
            self.expect(expected, right_ty, right.span);
        }
    }

    /// Checks `==` and `!=`, whose operands must have the same type and a value.
    fn check_comparison(&mut self, expr: &Expr, left: &Expr, right: &Expr) {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        if let (Ty::Known(left_known), Ty::Known(right_known)) = (left_ty, right_ty) {
            let comparable = left_known == right_known && left_known != Type::Unit;
            if !comparable {
                let message = format!("cannot compare `{left_known}` with `{right_known}`");
                self.error(message, expr.span);
            }
        }
    }

    /// Checks a call to a local, a user function or `print`.
    fn check_call(&mut self, expr: &Expr, callee: &Ident, args: &[Expr]) -> Ty {
        // The arguments are checked first so their errors and reads count even if the call is bad.
        let mut arguments = Vec::new();
        for arg in args {
            let ty = self.check_expr(arg);
            arguments.push(Argument { ty, span: arg.span });
        }

        let name = callee.name.as_str();

        // A function wins over a local of the same name: that clash is reported where the local is declared.
        if let Some((param_count, ret)) = self.function_shape(name) {
            self.record_binding(expr.id, Binding::Function(name.to_string()));
            self.check_user_call(name, param_count, expr.span, &arguments);
            return ret;
        }

        if let Some(local) = self.lookup_local(name) {
            self.used.insert(local.id);
            self.error(format!("`{name}` is not a function"), callee.span);
            return Ty::Error;
        }

        if name == PRINT {
            self.record_binding(expr.id, Binding::Print);
            self.check_print_call(expr.span, &arguments);
            return Ty::UNIT;
        }

        self.error(format!("undefined function `{name}`"), callee.span);
        Ty::Error
    }

    /// Returns the number of parameters and the return type of a user function.
    fn function_shape(&self, name: &str) -> Option<(usize, Ty)> {
        let signature = self.functions.get(name)?;
        Some((signature.params.len(), signature.ret))
    }

    /// Returns the type of a parameter of a user function.
    fn param_type(&self, name: &str, index: usize) -> Option<Ty> {
        let signature = self.functions.get(name)?;
        signature.params.get(index).copied()
    }

    /// Checks the arguments of a call to a user function against its signature.
    fn check_user_call(
        &mut self,
        name: &str,
        param_count: usize,
        span: Span,
        arguments: &[Argument],
    ) {
        if !self.check_arity(name, param_count, arguments.len(), span) {
            return;
        }
        for (index, argument) in arguments.iter().enumerate() {
            if let Some(expected) = self.param_type(name, index) {
                self.expect(expected, argument.ty, argument.span);
            }
        }
    }

    /// Checks the argument of `print`, which can show an `i64` or a `bool`.
    fn check_print_call(&mut self, span: Span, arguments: &[Argument]) {
        if !self.check_arity(PRINT, 1, arguments.len(), span) {
            return;
        }
        for argument in arguments {
            if argument.ty == Ty::UNIT {
                self.error("`print` expects `i64` or `bool`, found `()`", argument.span);
            }
        }
    }
}

/// Tells whether a block always returns: any one of its statements does.
fn block_always_returns(block: &Block) -> bool {
    block.stmts.iter().any(stmt_always_returns)
}

/// Tells whether a statement always returns.
fn stmt_always_returns(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Return(_) => true,
        StmtKind::If {
            then_block,
            else_branch: Some(else_branch),
            ..
        } => block_always_returns(then_block) && else_always_returns(else_branch),
        // Only a loop that never ends, because nothing breaks out of it, always returns.
        StmtKind::For {
            kind: ForKind::Infinite,
            body,
        } => !block_has_break(body),
        _ => false,
    }
}

/// Tells whether the part after `else` always returns.
fn else_always_returns(else_branch: &ElseBranch) -> bool {
    match else_branch {
        ElseBranch::Block(block) => block_always_returns(block),
        ElseBranch::If(nested) => stmt_always_returns(nested),
    }
}

/// Tells whether a block holds a `break` that belongs to the loop around it.
fn block_has_break(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_break)
}

/// Tells whether a statement is, or holds, a `break` that belongs to the loop around it.
fn stmt_has_break(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Break => true,
        StmtKind::If {
            then_block,
            else_branch,
            ..
        } => {
            let in_else = match else_branch {
                Some(else_branch) => else_has_break(else_branch),
                None => false,
            };
            block_has_break(then_block) || in_else
        }
        // A `break` inside a nested loop belongs to that loop, so it is not looked at.
        _ => false,
    }
}

/// Tells whether the part after `else` holds a `break` that belongs to the loop around it.
fn else_has_break(else_branch: &ElseBranch) -> bool {
    match else_branch {
        ElseBranch::Block(block) => block_has_break(block),
        ElseBranch::If(nested) => stmt_has_break(nested),
    }
}
