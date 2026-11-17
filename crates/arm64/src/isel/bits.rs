//! Selection of shifts and integer conversions.

use ir::{IrType, Loc, Sign, ValueType, conversion_can_fail};
use source::Diagnostic;

use super::{Selector, size_of};
use crate::mir::{Cond, Ext, MInst, Reg, Size, internal_error};
use crate::strings::PanicReason;

/// A shift and what its selection needs to know about it.
#[derive(Clone, Copy)]
pub(super) enum Shift {
    /// `<<`, on a left operand of this signedness, which decides how a narrow result is extended again.
    Left(Sign),
    /// `>>`, arithmetic for `Signed` and logical for `Unsigned`.
    Right(Sign),
}

/// The test that finds out whether a conversion changed the value.
#[derive(Clone, Copy)]
enum Check {
    /// The value must equal its own extension, as in `cmp x8, w8, sxtb`.
    Extension(Ext),
    /// None of these bits may be set, as in `tst x8, #mask`.
    HighBits(u64),
}

/// The top bit of a 64-bit value, which tells a negative `i64` or a `u64` above `i64::MAX`.
const TOP_BIT: u64 = 1 << 63;

impl Selector<'_> {
    /// Selects a shift: compare the amount, shift, and for `<<` on 8 and 16 bits extend the result again.
    ///
    /// `types` are the types of the shifted value and of the amount.
    pub(super) fn select_shift(
        &mut self,
        [dst, lhs, rhs]: [Reg; 3],
        shift: Shift,
        [ty, amount_ty]: [IrType; 2],
        loc: Loc,
    ) -> Result<(), Diagnostic> {
        // The compare is unsigned in the amount's own width, so a negative amount looks huge and panics too.
        let Ok(width) = u16::try_from(ty.bits()) else {
            return Err(internal_error("a type is wider than 65535 bits"));
        };
        self.insts.push(MInst::CmpImm {
            lhs: rhs,
            imm: width,
            size: size_of(amount_ty),
        });
        self.branch_to_panic(Cond::Hs, PanicReason::Shift, loc);

        // The amount's register may be narrower than `size`; `lsl` only reads its low 5 or 6 bits,
        // which the compare above has bounded.
        let size = size_of(ty);
        match shift {
            Shift::Left(sign) => {
                self.insts.push(MInst::Lsl {
                    dst,
                    lhs,
                    rhs,
                    size,
                });
                self.extend_narrow_shift_result(dst, ty, sign);
            }
            Shift::Right(Sign::Signed) => self.insts.push(MInst::Asr {
                dst,
                lhs,
                rhs,
                size,
            }),
            Shift::Right(Sign::Unsigned) => self.insts.push(MInst::Lsr {
                dst,
                lhs,
                rhs,
                size,
            }),
        }
        Ok(())
    }

    /// After `<<` on 8 or 16 bits: puts the result back in its extended form, which the bits shifted past the top broke.
    fn extend_narrow_shift_result(&mut self, dst: Reg, ty: IrType, sign: Sign) {
        let ext = match (ty, sign) {
            (IrType::I8, Sign::Signed) => Ext::Sxtb,
            (IrType::I8, Sign::Unsigned) => Ext::Uxtb,
            (IrType::I16, Sign::Signed) => Ext::Sxth,
            (IrType::I16, Sign::Unsigned) => Ext::Uxth,
            // A 32- or 64-bit register already holds exactly the shifted value.
            (IrType::I32 | IrType::I64, _) => return,
        };
        self.insts.push(MInst::Extend { dst, src: dst, ext });
    }

    /// Selects a conversion between integer types, which panics when the value does not fit.
    ///
    /// `regs` is the result and the source; `from` and `to` are the types of the source and the result.
    pub(super) fn select_conversion(
        &mut self,
        [dst, src]: [Reg; 2],
        from: ValueType,
        to: ValueType,
        loc: Loc,
    ) -> Result<(), Diagnostic> {
        // A value of 32 bits or fewer is first extended to 64 bits by its own signedness, into the
        // result; a 64-bit value is checked where it is and copied after the check.
        let widened = from.ty != IrType::I64;
        if widened {
            self.insts.push(match from.sign {
                Sign::Signed => MInst::Sxtw { dst, src },
                Sign::Unsigned => MInst::Uxtw { dst, src },
            });
        }
        let value = if widened { dst } else { src };

        if let Some(check) = conversion_check(from, to)? {
            self.insts.push(match check {
                Check::Extension(ext) => MInst::CmpExt {
                    lhs: value,
                    rhs: value,
                    ext,
                    size: Size::X,
                },
                Check::HighBits(imm) => MInst::Tst {
                    reg: value,
                    imm,
                    size: Size::X,
                },
            });
            self.branch_to_panic(Cond::Ne, PanicReason::Conversion, loc);
        }

        // Once the check has passed, the low 32 bits of the 64-bit value are the target value, extended as its type says.
        if !widened {
            self.insts.push(MInst::Mov { dst, src });
        }
        Ok(())
    }
}

/// The test for a conversion, or `None` when every source value fits the target type.
///
/// It runs on the source value extended to 64 bits.
fn conversion_check(from: ValueType, to: ValueType) -> Result<Option<Check>, Diagnostic> {
    if !conversion_can_fail(from, to) {
        return Ok(None);
    }
    let bits = to.ty.bits();
    let check = match (to.sign, from.sign) {
        (Sign::Signed, Sign::Signed) => Check::Extension(sign_extension(to.ty)?),
        // Every bit above the sign bit of the target must be clear.
        (Sign::Signed, Sign::Unsigned) if bits == 64 => Check::HighBits(TOP_BIT),
        (Sign::Signed, Sign::Unsigned) => Check::HighBits(!((1 << (bits - 1)) - 1)),
        // A `u64` target can only fail for a negative source: its top bit is set.
        (Sign::Unsigned, _) if bits == 64 => Check::HighBits(TOP_BIT),
        (Sign::Unsigned, _) => Check::HighBits(!((1 << bits) - 1)),
    };
    Ok(Some(check))
}

/// The sign extension that turns the low bits of a value into the value of the type.
///
/// A conversion that can fail between signed types has a target narrower than 64 bits.
fn sign_extension(ty: IrType) -> Result<Ext, Diagnostic> {
    match ty {
        IrType::I8 => Ok(Ext::Sxtb),
        IrType::I16 => Ok(Ext::Sxth),
        IrType::I32 => Ok(Ext::Sxtw),
        IrType::I64 => Err(internal_error(
            "a signed conversion to 64 bits was selected with a check",
        )),
    }
}
