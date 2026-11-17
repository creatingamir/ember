//! Expressions: names, operators and calls.

use ast::{BinaryOp, Expr, ExprKind, Ident, UnaryOp};
use source::Span;

use super::items::{UNSUPPORTED_TYPES, integer_type};
use super::{Binding, Checker, PRINT, Ty, TypeId};

impl Checker {
    /// Checks an expression and records its type.
    pub(super) fn check_expr(&mut self, expr: &Expr) -> Ty {
        let ty = match &expr.kind {
            // A literal has no type until its context gives it one.
            ExprKind::Int(_) => Ty::UNTYPED,
            ExprKind::Bool(_) => Ty::BOOL,
            ExprKind::Name(name) => self.check_name(expr, name),
            ExprKind::Unary { op, operand } => self.check_unary(expr, *op, operand),
            ExprKind::Binary { op, left, right } => self.check_binary(expr, *op, left, right),
            ExprKind::Call { callee, args } => self.check_call(expr, callee, args),
        };
        self.record_expr_type(expr.id, ty);
        ty
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
            UnaryOp::Neg => self.check_negation(expr, operand),
            UnaryOp::Not => {
                let operand_ty = self.check_expr(operand);
                self.expect(Ty::BOOL, operand_ty, operand.span);
                Ty::BOOL
            }
            UnaryOp::BitNot => self.check_bit_not(operand),
            UnaryOp::AddrOf | UnaryOp::Deref => {
                self.check_expr(operand);
                self.error(format!("`{op}` is not supported yet"), expr.span);
                Ty::Error
            }
        }
    }

    /// Checks `-e`: `e` must be a signed integer, and the result has its type.
    fn check_negation(&mut self, expr: &Expr, operand: &Expr) -> Ty {
        let operand_ty = self.check_expr(operand);
        let Ty::Known(id) = operand_ty else {
            return Ty::Error;
        };
        if operand_ty == Ty::UNTYPED {
            // The context decides the type, and then whether it can be negated.
            return Ty::UNTYPED;
        }
        if !self.is_integer(id) {
            self.expect(Ty::I64, operand_ty, operand.span);
            return Ty::Error;
        }
        self.check_negatable(id, expr.span);
        operand_ty
    }

    /// Checks `~e`: `e` must be an integer, and the result has its type.
    fn check_bit_not(&mut self, operand: &Expr) -> Ty {
        let operand_ty = self.check_expr(operand);
        if operand_ty == Ty::UNTYPED || self.is_typed_integer(operand_ty) {
            return operand_ty;
        }
        // Reports a `bool` or `()`; the error marker matches every type and reports nothing.
        self.expect(Ty::I64, operand_ty, operand.span);
        Ty::Error
    }

    /// Checks an infix operator.
    fn check_binary(&mut self, expr: &Expr, op: BinaryOp, left: &Expr, right: &Expr) -> Ty {
        match op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor => self.check_operands(left, right),
            BinaryOp::Shl | BinaryOp::Shr => self.check_shift(left, right),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                self.check_operands_with_default(left, right);
                Ty::BOOL
            }
            BinaryOp::And | BinaryOp::Or => {
                self.check_bool_operands(left, right);
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
            BinaryOp::MatMul => {
                self.check_expr(left);
                self.check_expr(right);
                self.error(format!("`{op}` is not supported yet"), expr.span);
                Ty::Error
            }
        }
    }

    /// Checks two operands that must be integers of one type, reporting only the first that is not.
    ///
    /// Returns that type; the untyped type when both operands are untyped; `Ty::Error` when an
    /// operand is wrong or already had an error, so that no second error follows from the result.
    fn check_operands(&mut self, left: &Expr, right: &Expr) -> Ty {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        let expected = self.operand_type(left_ty, right_ty);
        if expected == Ty::UNTYPED {
            // Both operands are untyped, so the context settles them later.
            return expected;
        }

        // `&&` stops at the first error, so only one is reported.
        let operands_ok = self.expect_expr(expected, left_ty, left)
            && self.expect_expr(expected, right_ty, right);
        if operands_ok && left_ty != Ty::Error && right_ty != Ty::Error {
            return expected;
        }
        Ty::Error
    }

    /// Checks `a << b` and `a >> b`: `a` is an integer, `b` is an integer of any type.
    ///
    /// The amount settles on its own and defaults to `i64`, so the type of `a` comes from the
    /// context. Returns the type of `a`, or `Ty::Error` when an operand is wrong.
    fn check_shift(&mut self, left: &Expr, right: &Expr) -> Ty {
        let left_ty = self.check_expr(left);
        let amount_ty = self.check_expr(right);
        let amount_ty = self.settle_to_default(right, amount_ty);

        let left_ok = left_ty == Ty::UNTYPED || self.is_typed_integer(left_ty);
        if !left_ok {
            // Reports a `bool` or `()`; the error marker matches every type and reports nothing.
            self.expect(Ty::I64, left_ty, left.span);
            return Ty::Error;
        }
        if !self.is_typed_integer(amount_ty) {
            self.expect(Ty::I64, amount_ty, right.span);
            return Ty::Error;
        }
        left_ty
    }

    /// Like `check_operands`, but two untyped operands settle to `i64` right here.
    pub(super) fn check_operands_with_default(&mut self, left: &Expr, right: &Expr) -> Ty {
        let ty = self.check_operands(left, right);
        if ty != Ty::UNTYPED {
            return ty;
        }
        self.settle(left, TypeId::I64);
        self.settle(right, TypeId::I64);
        Ty::I64
    }

    /// Checks that both operands are `bool`, reporting only the first that is not.
    fn check_bool_operands(&mut self, left: &Expr, right: &Expr) {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        if self.expect(Ty::BOOL, left_ty, left.span) {
            self.expect(Ty::BOOL, right_ty, right.span);
        }
    }

    /// Checks `==` and `!=`, whose operands must have the same type and a value.
    ///
    /// An untyped operand takes the type of the other one, or `i64` when both are untyped.
    fn check_comparison(&mut self, expr: &Expr, left: &Expr, right: &Expr) {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        let (Ty::Known(left_known), Ty::Known(right_known)) = (left_ty, right_ty) else {
            return;
        };

        if left_ty == Ty::UNTYPED && right_ty == Ty::UNTYPED {
            self.settle(left, TypeId::I64);
            self.settle(right, TypeId::I64);
        } else if left_ty == Ty::UNTYPED && self.is_integer(right_known) {
            self.settle(left, right_known);
        } else if right_ty == Ty::UNTYPED && self.is_integer(left_known) {
            self.settle(right, left_known);
        } else if left_known != right_known || left_known == TypeId::UNIT {
            let left_name = self.type_name(left_known);
            let right_name = self.type_name(right_known);
            let message = format!("cannot compare `{left_name}` with `{right_name}`");
            self.error(message, expr.span);
        }
    }

    /// Checks a call to a local, a user function or `print`.
    fn check_call(&mut self, expr: &Expr, callee: &Ident, args: &[Expr]) -> Ty {
        // The arguments are checked first so their errors and reads count even if the call is bad.
        let mut arg_types = Vec::new();
        for arg in args {
            arg_types.push(self.check_expr(arg));
        }

        let name = callee.name.as_str();

        // A function wins over a local of the same name: that clash is reported where the local is declared.
        if let Some((param_count, ret)) = self.function_shape(name) {
            self.record_binding(expr.id, Binding::Function(name.to_string()));
            self.check_user_call(name, param_count, expr.span, args, &arg_types);
            return ret;
        }

        if let Some(local) = self.lookup_local(name) {
            self.used.insert(local.id);
            self.error(format!("`{name}` is not a function"), callee.span);
            return Ty::Error;
        }

        if name == PRINT {
            self.record_binding(expr.id, Binding::Print);
            self.check_print_call(expr.span, args, &arg_types);
            return Ty::UNIT;
        }

        if let Some(target) = integer_type(name) {
            return self.check_conversion(expr, target, args, &arg_types);
        }

        if name == "bool" {
            return self.check_bool_conversion(expr, args, &arg_types);
        }

        if UNSUPPORTED_TYPES.contains(&name) {
            self.error(format!("type `{name}` is not supported yet"), callee.span);
            return Ty::Error;
        }

        self.error(format!("undefined function `{name}`"), callee.span);
        Ty::Error
    }

    /// Checks `T(x)`, a conversion to the integer type `target`; `x` must be an integer.
    ///
    /// An untyped argument settles to `target`, so a literal that does not fit is an error here.
    fn check_conversion(
        &mut self,
        expr: &Expr,
        target: TypeId,
        args: &[Expr],
        arg_types: &[Ty],
    ) -> Ty {
        self.record_binding(expr.id, Binding::Convert(target));
        let result = Ty::Known(target);
        let name = self.type_name(target);
        if !self.check_arity(&name, 1, args.len(), expr.span) {
            return result;
        }

        for (arg, arg_ty) in args.iter().zip(arg_types) {
            match arg_ty {
                Ty::Known(TypeId::UNTYPED_INT) => {
                    self.settle(arg, target);
                }
                Ty::Known(from) if !self.is_integer(*from) => {
                    let from_name = self.type_name(*from);
                    let message = format!("cannot convert `{from_name}` to `{name}`");
                    self.error(message, arg.span);
                }
                Ty::Known(_) | Ty::Error => {}
            }
        }
        result
    }

    /// Checks `bool(x)`, which is never allowed: no integer converts to `bool`.
    fn check_bool_conversion(&mut self, expr: &Expr, args: &[Expr], arg_types: &[Ty]) -> Ty {
        if !self.check_arity("bool", 1, args.len(), expr.span) {
            return Ty::Error;
        }
        for arg_ty in arg_types {
            // An argument with an earlier error is not reported again.
            let Ty::Known(from) = arg_ty else {
                continue;
            };
            let message = if *from == TypeId::BOOL {
                "there is no conversion to `bool`".to_string()
            } else {
                let from_name = self.type_name(*from);
                format!("cannot convert `{from_name}` to `bool`")
            };
            self.error(message, expr.span);
        }
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
    ///
    /// `arg_types[i]` is the type of `args[i]`.
    fn check_user_call(
        &mut self,
        name: &str,
        param_count: usize,
        span: Span,
        args: &[Expr],
        arg_types: &[Ty],
    ) {
        if !self.check_arity(name, param_count, args.len(), span) {
            return;
        }
        for (index, (arg, arg_ty)) in args.iter().zip(arg_types).enumerate() {
            if let Some(expected) = self.param_type(name, index) {
                self.expect_expr(expected, *arg_ty, arg);
            }
        }
    }

    /// Checks the argument of `print`, which can show any integer or a `bool`.
    ///
    /// `arg_types[i]` is the type of `args[i]`.
    fn check_print_call(&mut self, span: Span, args: &[Expr], arg_types: &[Ty]) {
        if !self.check_arity(PRINT, 1, args.len(), span) {
            return;
        }
        for (arg, arg_ty) in args.iter().zip(arg_types) {
            if *arg_ty == Ty::UNIT {
                self.error("`print` cannot show `()`", arg.span);
            } else {
                self.settle_to_default(arg, *arg_ty);
            }
        }
    }
}
