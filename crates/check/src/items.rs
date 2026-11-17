//! Declarations: the signatures and bodies of functions, written types, and the return rules.

use ast::{Block, ElseBranch, ForKind, Function, Program, Stmt, StmtKind, TypeExpr};
use source::Span;

use super::{Checker, FnSig, Ty, TypeId};

/// The type names from the language spec that are not supported yet.
const UNSUPPORTED_TYPES: [&str; 5] = ["f16", "bf16", "f32", "f64", "str"];

impl Checker {
    /// Checks every function of the program.
    pub(super) fn check_program(&mut self, program: &Program) {
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
    pub(super) fn resolve_type(&mut self, type_expr: &TypeExpr) -> Ty {
        let name = type_expr.name.name.as_str();
        match name {
            "i8" => Ty::Known(TypeId::I8),
            "i16" => Ty::Known(TypeId::I16),
            "i32" => Ty::Known(TypeId::I32),
            "i64" => Ty::I64,
            "u8" => Ty::Known(TypeId::U8),
            "u16" => Ty::Known(TypeId::U16),
            "u32" => Ty::Known(TypeId::U32),
            "u64" => Ty::Known(TypeId::U64),
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
            Ty::Known(ty) => ty != TypeId::UNIT,
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
