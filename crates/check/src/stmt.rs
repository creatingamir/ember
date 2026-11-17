//! Statements: `let`, assignment, `if`, `for` and `return`.

use ast::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ExprKind, ForKind, Ident, Stmt, StmtKind, TypeExpr,
};
use source::Span;

use super::{Binding, Checker, LetDecl, Ty, TypeId};

impl Checker {
    /// Checks the statements of a block in a new scope.
    pub(super) fn check_block(&mut self, block: &Block) {
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
                    self.expect_expr(declared, value_ty, value);
                }
                declared
            }
            None => {
                if self.require_value(value_ty, value.span) {
                    self.settle_to_default(value, value_ty)
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
                self.expect_expr(target_ty, value_ty, value);
            }
            _ => {
                // A bad target is reported alone; the value is only checked when the target is fine.
                if self.check_compound_target(target_ty, target.span) {
                    self.expect_expr(target_ty, value_ty, value);
                }
            }
        }
    }

    /// Reports a target of `+=` and the like that is not an integer; returns false when it did.
    fn check_compound_target(&mut self, target_ty: Ty, span: Span) -> bool {
        match target_ty {
            // `i64` is an integer, so this call reports every other type.
            Ty::Known(ty) if !self.is_integer(ty) => self.expect(Ty::I64, target_ty, span),
            _ => true,
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
                let var_ty = self.check_iterable(iterable);
                self.push_scope();
                self.declare(var, stmt.id, var_ty);
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
    ///
    /// Returns the type of the loop variable: the type of the bounds.
    fn check_iterable(&mut self, iterable: &Expr) -> Ty {
        match &iterable.kind {
            ExprKind::Binary {
                op: BinaryOp::Range | BinaryOp::RangeInclusive,
                left,
                right,
            } => self.check_operands_with_default(left, right),
            _ => {
                let errors_before = self.diagnostics.len();
                self.check_expr(iterable);
                if self.diagnostics.len() == errors_before {
                    self.error("only ranges can be iterated over yet", iterable.span);
                }
                Ty::Error
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
            && ty != TypeId::UNIT
        {
            let name = self.type_name(ty);
            let message = format!("expected a return value of type `{name}`");
            self.error(message, stmt.span);
        }
    }

    /// Checks the value of `return e` against the return type of the current function.
    fn check_return_value(&mut self, expr: &Expr) {
        let ty = self.check_expr(expr);
        if self.fn_ret != Ty::UNIT {
            self.expect_expr(self.fn_ret, ty, expr);
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
}
