//! Integer typing: untyped literals taking their type from context, and the checks that go with it.

use ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use source::Span;

use super::{Checker, Ty, TypeId, TypeKind};

impl Checker {
    /// Tells whether a type is one of the eight integer types.
    pub(super) fn is_integer(&self, ty: TypeId) -> bool {
        matches!(self.checked.types.kind(ty), TypeKind::Int { .. })
    }

    /// Tells whether a checker type is one of the eight integer types, which an untyped integer is not.
    fn is_typed_integer(&self, ty: Ty) -> bool {
        match ty {
            Ty::Known(id) => self.is_integer(id),
            Ty::Error => false,
        }
    }

    /// Like `expect`, but an untyped integer where `expected` is an integer type takes that type.
    ///
    /// Returns false when it reported an error.
    pub(super) fn expect_expr(&mut self, expected: Ty, found: Ty, expr: &Expr) -> bool {
        if let Ty::Known(expected_id) = expected
            && found == Ty::UNTYPED
            && self.is_integer(expected_id)
        {
            return self.settle(expr, expected_id);
        }
        self.expect(expected, found, expr.span)
    }

    /// Gives an untyped integer the type `i64`, its default; any other type is returned unchanged.
    pub(super) fn settle_to_default(&mut self, expr: &Expr, ty: Ty) -> Ty {
        if ty != Ty::UNTYPED {
            return ty;
        }
        self.settle(expr, TypeId::I64);
        Ty::I64
    }

    /// Gives an untyped expression the integer type `ty`, on it and on every untyped node inside it.
    ///
    /// Reports literals that do not fit and negations of unsigned values; returns false when it did.
    /// Each node is visited once, because a settled node is never settled again.
    pub(super) fn settle(&mut self, expr: &Expr, ty: TypeId) -> bool {
        self.record_expr_type(expr.id, Ty::Known(ty));
        match &expr.kind {
            ExprKind::Int(magnitude) => self.check_literal_fits(*magnitude, false, expr.span, ty),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => self.settle_negation(expr, operand, ty),
            ExprKind::Binary {
                op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
                left,
                right,
            } => {
                let left_ok = self.settle(left, ty);
                let right_ok = self.settle(right, ty);
                left_ok && right_ok
            }
            _ => {
                self.internal_error(
                    "an expression that is not a constant has no type",
                    expr.span,
                );
                false
            }
        }
    }

    /// Settles `-operand`; a minus sign directly on a literal belongs to the literal (decision 0049, number 3).
    fn settle_negation(&mut self, expr: &Expr, operand: &Expr, ty: TypeId) -> bool {
        if let ExprKind::Int(magnitude) = &operand.kind {
            self.record_expr_type(operand.id, Ty::Known(ty));
            return self.check_literal_fits(*magnitude, true, operand.span, ty);
        }

        // The operand is left alone after this error, so `- -1` for a `u8` is reported once.
        if !self.check_negatable(ty, expr.span) {
            return false;
        }
        self.settle(operand, ty)
    }

    /// Reports a negation of an unsigned type; returns false when it did.
    pub(super) fn check_negatable(&mut self, ty: TypeId, span: Span) -> bool {
        if let TypeKind::Int { signed: false, .. } = self.checked.types.kind(ty) {
            let name = self.type_name(ty);
            let message = format!("cannot negate a value of unsigned type `{name}`");
            self.error(message, span);
            return false;
        }
        true
    }

    /// Reports an integer literal that is outside the range of `ty`; returns false when it did.
    ///
    /// `negative` is true for a literal under a minus sign. `span` is where the digits are.
    fn check_literal_fits(
        &mut self,
        magnitude: u64,
        negative: bool,
        span: Span,
        ty: TypeId,
    ) -> bool {
        let TypeKind::Int { bits, signed } = self.checked.types.kind(ty) else {
            self.internal_error(
                "an integer literal was given a type that is not an integer",
                span,
            );
            return false;
        };

        // i128 holds every value of every integer type, so the comparison cannot overflow.
        let value = if negative {
            -i128::from(magnitude)
        } else {
            i128::from(magnitude)
        };
        let (min, max) = if signed {
            (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
        } else {
            (0, (1_i128 << bits) - 1)
        };
        if value < min || value > max {
            let name = self.type_name(ty);
            self.error(format!("integer literal does not fit in `{name}`"), span);
            return false;
        }
        true
    }

    /// The type that two operands must share: the type of the first typed integer, left first.
    ///
    /// Two untyped operands stay untyped. Otherwise the type is `i64`, so that an operand that is
    /// not an integer is reported against `i64`, as in Phase 1 (decision 0049, number 1).
    pub(super) fn operand_type(&self, left_ty: Ty, right_ty: Ty) -> Ty {
        for ty in [left_ty, right_ty] {
            if self.is_typed_integer(ty) {
                return ty;
            }
        }
        if left_ty == Ty::UNTYPED && right_ty == Ty::UNTYPED {
            return Ty::UNTYPED;
        }
        Ty::I64
    }
}
