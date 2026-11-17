//! Selection of the checked arithmetic, negation and comparisons for every integer width and sign.

use ir::{BinOp, IrType, Loc, Sign};
use source::Diagnostic;

use super::bits::Shift;
use super::{Selector, size_of};
use crate::mir::{Cond, Ext, MInst, Reg, Size, internal_error};
use crate::strings::PanicReason;

/// The operands, type and position of one binary operation.
#[derive(Clone, Copy)]
struct Arith {
    /// The result register.
    dst: Reg,
    /// The left operand.
    lhs: Reg,
    /// The right operand.
    rhs: Reg,
    /// The type of the operands, which is also the type of an arithmetic result.
    ty: IrType,
    /// Whether the operands are signed or unsigned.
    sign: Sign,
    /// Where a panic is reported.
    loc: Loc,
}

impl Selector<'_> {
    /// Selects a binary operation on operands of the types `[lhs, rhs]`.
    ///
    /// The types differ only for a shift, whose amount may have any type.
    pub(super) fn select_binary(
        &mut self,
        [dst, lhs, rhs]: [Reg; 3],
        op: BinOp,
        [ty, amount_ty]: [IrType; 2],
        loc: Loc,
    ) -> Result<(), Diagnostic> {
        let with = |sign| Arith {
            dst,
            lhs,
            rhs,
            ty,
            sign,
            loc,
        };
        match op {
            BinOp::Add(sign) => self.select_add_or_sub(with(sign), true)?,
            BinOp::Sub(sign) => self.select_add_or_sub(with(sign), false)?,
            BinOp::Mul(sign) => self.select_mul(with(sign))?,
            BinOp::Div(sign) => self.select_div(with(sign), false)?,
            BinOp::Rem(sign) => self.select_div(with(sign), true)?,
            BinOp::Eq => self.select_compare(with(Sign::Signed), Cond::Eq),
            BinOp::Ne => self.select_compare(with(Sign::Signed), Cond::Ne),
            BinOp::Lt(sign) => self.select_compare(with(sign), ordered(sign, Order::Lt)),
            BinOp::Le(sign) => self.select_compare(with(sign), ordered(sign, Order::Le)),
            BinOp::Gt(sign) => self.select_compare(with(sign), ordered(sign, Order::Gt)),
            BinOp::Ge(sign) => self.select_compare(with(sign), ordered(sign, Order::Ge)),
            // Two extended values give an extended result, so a bitwise operation needs no check.
            BinOp::And => self.insts.push(MInst::And {
                dst,
                lhs,
                rhs,
                size: size_of(ty),
            }),
            BinOp::Or => self.insts.push(MInst::Orr {
                dst,
                lhs,
                rhs,
                size: size_of(ty),
            }),
            BinOp::Xor => self.insts.push(MInst::Eor {
                dst,
                lhs,
                rhs,
                size: size_of(ty),
            }),
            BinOp::Shl(sign) => {
                self.select_shift([dst, lhs, rhs], Shift::Left(sign), [ty, amount_ty], loc)?;
            }
            BinOp::Shr(sign) => {
                self.select_shift([dst, lhs, rhs], Shift::Right(sign), [ty, amount_ty], loc)?;
            }
        }
        Ok(())
    }

    /// Selects `-src` on a signed type; the minimum value of the type panics.
    pub(super) fn select_neg(
        &mut self,
        dst: Reg,
        src: Reg,
        ty: IrType,
        loc: Loc,
    ) -> Result<(), Diagnostic> {
        let size = size_of(ty);
        if is_narrow(ty) {
            self.insts.push(MInst::Neg { dst, src, size });
            let arith = Arith {
                dst,
                lhs: src,
                rhs: src,
                ty,
                sign: Sign::Signed,
                loc,
            };
            return self.check_narrow_result(arith);
        }
        self.insts.push(MInst::Negs { dst, src, size });
        self.branch_to_panic(Cond::Vs, PanicReason::Overflow, loc);
        Ok(())
    }

    /// Selects a checked addition or subtraction.
    fn select_add_or_sub(&mut self, a: Arith, is_add: bool) -> Result<(), Diagnostic> {
        let size = size_of(a.ty);
        let (dst, lhs, rhs) = (a.dst, a.lhs, a.rhs);
        if is_narrow(a.ty) {
            // The 32-bit result cannot wrap, so the check is whether it still fits the type.
            if is_add {
                self.insts.push(MInst::Add {
                    dst,
                    lhs,
                    rhs,
                    size,
                });
            } else {
                self.insts.push(MInst::Sub {
                    dst,
                    lhs,
                    rhs,
                    size,
                });
            }
            return self.check_narrow_result(a);
        }
        // The flags say whether the 32- or 64-bit result is wrong: V for signed, carry for unsigned.
        let cond = match (is_add, a.sign) {
            (_, Sign::Signed) => Cond::Vs,
            (true, Sign::Unsigned) => Cond::Hs,
            (false, Sign::Unsigned) => Cond::Lo,
        };
        if is_add {
            self.insts.push(MInst::Adds {
                dst,
                lhs,
                rhs,
                size,
            });
        } else {
            self.insts.push(MInst::Subs {
                dst,
                lhs,
                rhs,
                size,
            });
        }
        self.branch_to_panic(cond, PanicReason::Overflow, a.loc);
        Ok(())
    }

    /// After an 8- or 16-bit operation done in 32 bits: panics when the result is outside the type.
    fn check_narrow_result(&mut self, a: Arith) -> Result<(), Diagnostic> {
        match a.sign {
            Sign::Signed => self.insts.push(MInst::CmpExt {
                lhs: a.dst,
                rhs: a.dst,
                ext: narrow_extension(a.ty)?,
                size: Size::W,
            }),
            Sign::Unsigned => self.insts.push(MInst::Tst {
                reg: a.dst,
                imm: narrow_high_bits(a.ty)?,
                size: Size::W,
            }),
        }
        self.branch_to_panic(Cond::Ne, PanicReason::Overflow, a.loc);
        Ok(())
    }

    /// Selects a checked multiplication.
    fn select_mul(&mut self, a: Arith) -> Result<(), Diagnostic> {
        let (dst, lhs, rhs) = (a.dst, a.lhs, a.rhs);
        match (a.ty, a.sign) {
            (IrType::I64, Sign::Signed) => {
                let high = self.new_vreg()?;
                // The full 128-bit product is `high:dst`. It fits in 64 bits exactly when
                // the high half is the sign extension of the low half (all zeros or all ones).
                let size = Size::X;
                self.insts.push(MInst::Mul {
                    dst,
                    lhs,
                    rhs,
                    size,
                });
                self.insts.push(MInst::Smulh {
                    dst: high,
                    lhs,
                    rhs,
                });
                self.insts.push(MInst::CmpAsr63 {
                    lhs: high,
                    rhs: dst,
                });
            }
            (IrType::I64, Sign::Unsigned) => {
                let high = self.new_vreg()?;
                // The unsigned product fits in 64 bits exactly when its high half is zero.
                let size = Size::X;
                self.insts.push(MInst::Mul {
                    dst,
                    lhs,
                    rhs,
                    size,
                });
                self.insts.push(MInst::Umulh {
                    dst: high,
                    lhs,
                    rhs,
                });
                self.insts.push(MInst::CmpImm {
                    lhs: high,
                    imm: 0,
                    size,
                });
            }
            (IrType::I32, Sign::Signed) => {
                // The 64-bit product fits in 32 bits exactly when it equals its own sign extension.
                self.insts.push(MInst::Smull { dst, lhs, rhs });
                self.insts.push(MInst::CmpExt {
                    lhs: dst,
                    rhs: dst,
                    ext: Ext::Sxtw,
                    size: Size::X,
                });
            }
            (IrType::I32, Sign::Unsigned) => {
                self.insts.push(MInst::Umull { dst, lhs, rhs });
                self.insts.push(MInst::Tst {
                    reg: dst,
                    imm: 0xffff_ffff_0000_0000,
                    size: Size::X,
                });
            }
            (IrType::I8 | IrType::I16, _) => {
                // The product of two 8- or 16-bit values fits in 32 bits.
                self.insts.push(MInst::Mul {
                    dst,
                    lhs,
                    rhs,
                    size: Size::W,
                });
                return self.check_narrow_result(a);
            }
        }
        self.branch_to_panic(Cond::Ne, PanicReason::Overflow, a.loc);
        Ok(())
    }

    /// Selects a checked division, or a remainder when `remainder` is set.
    fn select_div(&mut self, a: Arith, remainder: bool) -> Result<(), Diagnostic> {
        let size = size_of(a.ty);
        let (dst, lhs, rhs) = (a.dst, a.lhs, a.rhs);
        // Zero is checked first, so `MIN / 0` reports "division by zero".
        self.insts.push(MInst::CmpImm {
            lhs: rhs,
            imm: 0,
            size,
        });
        self.branch_to_panic(Cond::Eq, PanicReason::DivByZero, a.loc);
        if a.sign == Sign::Signed {
            // `MIN / -1` does not fit the type (and `sdiv` would silently return `MIN`).
            let ok = self.new_label("ok");
            self.insts.push(MInst::CmnImm {
                lhs: rhs,
                imm: 1,
                size,
            });
            self.insts.push(MInst::BCond {
                cond: Cond::Ne,
                target: ok.clone(),
            });
            let min = self.new_vreg()?;
            self.insts.push(MInst::MovImm {
                dst: min,
                value: min_pattern(a.ty),
                size,
            });
            self.insts.push(MInst::Cmp {
                lhs,
                rhs: min,
                size,
            });
            self.branch_to_panic(Cond::Eq, PanicReason::Overflow, a.loc);
            self.insts.push(MInst::Label(ok));
        }
        let divide = |dst| match a.sign {
            Sign::Signed => MInst::Sdiv {
                dst,
                lhs,
                rhs,
                size,
            },
            Sign::Unsigned => MInst::Udiv {
                dst,
                lhs,
                rhs,
                size,
            },
        };
        if remainder {
            // remainder = dividend - quotient * divisor
            let quotient = self.new_vreg()?;
            self.insts.push(divide(quotient));
            self.insts.push(MInst::Msub {
                dst,
                mul_lhs: quotient,
                mul_rhs: rhs,
                minuend: lhs,
                size,
            });
        } else {
            self.insts.push(divide(dst));
        }
        Ok(())
    }

    /// Selects a comparison that leaves 0 or 1 in `dst`.
    fn select_compare(&mut self, a: Arith, cond: Cond) {
        let size = size_of(a.ty);
        self.insts.push(MInst::Cmp {
            lhs: a.lhs,
            rhs: a.rhs,
            size,
        });
        self.insts.push(MInst::Cset { dst: a.dst, cond });
    }
}

/// The four ordering comparisons.
#[derive(Clone, Copy)]
enum Order {
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// The condition of an ordering comparison: signed or unsigned.
fn ordered(sign: Sign, order: Order) -> Cond {
    match (sign, order) {
        (Sign::Signed, Order::Lt) => Cond::Lt,
        (Sign::Signed, Order::Le) => Cond::Le,
        (Sign::Signed, Order::Gt) => Cond::Gt,
        (Sign::Signed, Order::Ge) => Cond::Ge,
        (Sign::Unsigned, Order::Lt) => Cond::Lo,
        (Sign::Unsigned, Order::Le) => Cond::Ls,
        (Sign::Unsigned, Order::Gt) => Cond::Hi,
        (Sign::Unsigned, Order::Ge) => Cond::Hs,
    }
}

/// Tells whether a type is checked through a 32-bit operation and an extension test (8 and 16 bits).
fn is_narrow(ty: IrType) -> bool {
    matches!(ty, IrType::I8 | IrType::I16)
}

/// The extension that turns the low part of a narrow signed result back into the full value.
///
/// Only 8- and 16-bit types are checked this way; any other type is an internal error.
fn narrow_extension(ty: IrType) -> Result<Ext, Diagnostic> {
    match ty {
        IrType::I8 => Ok(Ext::Sxtb),
        IrType::I16 => Ok(Ext::Sxth),
        IrType::I32 | IrType::I64 => Err(not_narrow()),
    }
}

/// The bits of a 32-bit register above an unsigned 8- or 16-bit result; any of them set means overflow.
fn narrow_high_bits(ty: IrType) -> Result<u64, Diagnostic> {
    match ty {
        IrType::I8 => Ok(0xffff_ff00),
        IrType::I16 => Ok(0xffff_0000),
        IrType::I32 | IrType::I64 => Err(not_narrow()),
    }
}

/// The error for a narrow-only check that was asked about a 32- or 64-bit type.
fn not_narrow() -> Diagnostic {
    internal_error("a narrow overflow check was selected for a wide type")
}

/// The smallest value of a signed type, in the extended form that the register holds it.
fn min_pattern(ty: IrType) -> u64 {
    match ty {
        IrType::I8 => 0xffff_ff80,
        IrType::I16 => 0xffff_8000,
        IrType::I32 => 0x8000_0000,
        IrType::I64 => 0x8000_0000_0000_0000,
    }
}
