//! Frame layout: the frame size, and the stack address of every cell.

use source::Diagnostic;

use crate::mir::{Access, Base, MFunction, Mem, internal_error};

/// The largest frame that `sub sp, sp, #imm` can reserve (a 12-bit immediate); bigger frames are probed.
pub const MAX_IMMEDIATE_FRAME: u64 = 4095;
/// The largest immediate offset `ldr`/`str` encode, in units of the access size.
const MAX_ACCESS_UNITS: u64 = 4095;
/// The largest frame Ember accepts: half of the default 8 MiB main-thread stack (decision 0032).
const MAX_FRAME_SIZE: u64 = 4 * 1024 * 1024;
/// The size of one stack cell in bytes.
const CELL_SIZE: u64 = 8;
/// Where incoming stack arguments start, relative to `x29`: above the saved `x29` and `x30`.
const INCOMING_ARGS_OFFSET: u64 = 16;

/// The largest byte offset that an access of this width encodes as an immediate: 4095, 8190, 16380 or 32760.
pub fn max_access_offset(access: Access) -> u64 {
    MAX_ACCESS_UNITS * access.bytes()
}

/// Sets the frame size and turns every slot, cell and stack argument into an address.
///
/// A frame over 4 MiB is a user error at the function's name (a valid program can reach it).
pub fn lay_out(mut function: MFunction) -> Result<MFunction, Diagnostic> {
    // The outgoing arguments are at the bottom of the frame, so the cells start above them.
    let outgoing_size = round_up_to_16(u64::from(function.out_arg_bytes));
    let layout = Layout {
        slot_count: function.slot_count,
        vreg_count: function.vreg_count,
        outgoing_size,
    };
    for inst in &mut function.insts {
        if let Some(mem) = inst.mem_mut() {
            *mem = layout.place(*mem)?;
        }
    }

    // Checked after the addresses, so a nonexistent slot is reported as the compiler bug it is.
    let cell_count = u64::from(function.slot_count) + u64::from(function.vreg_count);
    let frame_size = outgoing_size + round_up_to_16(cell_count * CELL_SIZE);
    if frame_size > MAX_FRAME_SIZE {
        let message = format!(
            "stack frame of `{}` is too large: {frame_size} bytes (the limit is {MAX_FRAME_SIZE} bytes)",
            function.name
        );
        return Err(Diagnostic::error(message, function.span));
    }
    function.frame_size = frame_size;
    Ok(function)
}

/// Rounds up to a multiple of 16 so that `sp` stays aligned.
fn round_up_to_16(bytes: u64) -> u64 {
    bytes.div_ceil(16) * 16
}

/// Where the parts of a frame are.
struct Layout {
    /// The number of IR slots; slot `k` is cell `k`.
    slot_count: u32,
    /// The number of virtual registers; register `v` is cell `slot_count + v`.
    vreg_count: u32,
    /// The size of the outgoing argument area, below the cells.
    outgoing_size: u64,
}

impl Layout {
    /// The address of a slot, a virtual register's cell or a stack argument. Cells stay 8 bytes wide
    /// whatever they hold (decision 0049, number 6).
    fn place(&self, mem: Mem) -> Result<Mem, Diagnostic> {
        let cell = match mem {
            Mem::Slot(slot) if slot.0 < self.slot_count => u64::from(slot.0),
            Mem::Cell(vreg) if vreg.0 < self.vreg_count => {
                u64::from(self.slot_count) + u64::from(vreg.0)
            }
            Mem::InArg { offset } => {
                return Ok(Mem::Address {
                    base: Base::Fp,
                    offset: INCOMING_ARGS_OFFSET + u64::from(offset),
                });
            }
            Mem::OutArg { offset } => {
                return Ok(Mem::Address {
                    base: Base::Sp,
                    offset: u64::from(offset),
                });
            }
            Mem::Address { .. } => return Ok(mem),
            Mem::Slot(_) | Mem::Cell(_) => {
                return Err(internal_error("a slot or temp does not exist"));
            }
        };
        Ok(Mem::Address {
            base: Base::Sp,
            offset: self.outgoing_size + cell * CELL_SIZE,
        })
    }
}

/// How many 4-byte machine instructions each MIR instruction prints as (branch relaxation needs exact sizes).
pub mod size_table {
    use super::{MAX_IMMEDIATE_FRAME, max_access_offset};
    use crate::mir::{Access, MInst, Mem};

    /// The number of instructions `inst` prints as, for a function with `frame_size`.
    pub fn size(inst: &MInst, frame_size: u64) -> u32 {
        match inst {
            MInst::Label(_) => 0,
            MInst::MovImm { value, size, .. } => mov_imm_size(size.truncate(*value)),
            MInst::LoadAddress { .. } => 2,
            MInst::Load { mem, access, .. } | MInst::Store { mem, access, .. } => {
                1 + far_offset_size(mem, *access)
            }
            // `stp`, `mov`, then `sub`; a probed frame builds the size, calls the probe, then `sub`.
            MInst::Prologue => {
                if frame_size <= MAX_IMMEDIATE_FRAME {
                    3
                } else {
                    4 + mov_imm_size(frame_size)
                }
            }
            MInst::Epilogue => 3,
            // Listed one by one, without `_`, so a new instruction kind fails to
            // compile until its size is decided (branch relaxation relies on it).
            MInst::Mov { .. }
            | MInst::Adds { .. }
            | MInst::Subs { .. }
            | MInst::Add { .. }
            | MInst::Sub { .. }
            | MInst::Neg { .. }
            | MInst::Negs { .. }
            | MInst::Mul { .. }
            | MInst::Smulh { .. }
            | MInst::Umulh { .. }
            | MInst::Smull { .. }
            | MInst::Umull { .. }
            | MInst::Sdiv { .. }
            | MInst::Udiv { .. }
            | MInst::Msub { .. }
            | MInst::Cmp { .. }
            | MInst::CmpExt { .. }
            | MInst::Tst { .. }
            | MInst::Sxtw { .. }
            | MInst::Uxtw { .. }
            | MInst::CmpAsr63 { .. }
            | MInst::CmpImm { .. }
            | MInst::CmnImm { .. }
            | MInst::Cset { .. }
            | MInst::EorOne { .. }
            | MInst::B { .. }
            | MInst::BCond { .. }
            | MInst::Cbnz { .. }
            | MInst::Cbz { .. }
            | MInst::PanicIf { .. }
            | MInst::Bl { .. }
            | MInst::Brk => 1,
        }
    }

    /// The extra instructions a memory operand needs to load a far offset into `x17`.
    fn far_offset_size(mem: &Mem, access: Access) -> u32 {
        match mem {
            Mem::Address { offset, .. } if *offset > max_access_offset(access) => {
                mov_imm_size(*offset)
            }
            _ => 0,
        }
    }

    /// `movz` for the low chunk plus one `movk` for each other non-zero 16-bit chunk.
    fn mov_imm_size(value: u64) -> u32 {
        let upper_chunks = [16, 32, 48]
            .iter()
            .filter(|shift| (value >> **shift) & 0xffff != 0)
            .count();
        1 + upper_chunks as u32
    }
}

#[cfg(test)]
mod tests {
    use super::size_table::size;
    use super::*;
    use crate::mir::{MInst, Reg, Size};

    /// A load or store at `offset`.
    fn load_at(offset: u64) -> MInst {
        MInst::Load {
            dst: Reg::phys(8),
            mem: Mem::Address {
                base: Base::Sp,
                offset,
            },
            access: Access::Double,
        }
    }

    /// A `MovImm` of `value` at `size`.
    fn mov_imm(value: u64, size: Size) -> MInst {
        MInst::MovImm {
            dst: Reg::phys(8),
            value,
            size,
        }
    }

    /// A constant takes one `movz` plus one `movk` per non-zero upper chunk.
    #[test]
    fn constants_count_their_non_zero_chunks() {
        assert_eq!(size(&mov_imm(0, Size::X), 0), 1);
        assert_eq!(size(&mov_imm(65535, Size::X), 0), 1);
        assert_eq!(size(&mov_imm(1 << 16, Size::X), 0), 2);
        assert_eq!(size(&mov_imm(i64::MIN as u64, Size::X), 0), 2);
        assert_eq!(size(&mov_imm(u64::MAX, Size::X), 0), 4);
    }

    /// A `w` constant has at most two chunks: only its low 32 bits are loaded.
    #[test]
    fn w_constants_ignore_the_upper_half() {
        assert_eq!(size(&mov_imm(u64::MAX, Size::W), 0), 2);
        assert_eq!(size(&mov_imm(0xffff_ffff_0000_0005, Size::W), 0), 1);
        assert_eq!(size(&mov_imm(i64::MIN as u64, Size::W), 0), 1);
    }

    /// The immediate limit of a load or store grows with the access width: 4095, 8190, 16380, 32760.
    #[test]
    fn far_limit_depends_on_the_access() {
        let at = |offset, access| MInst::Load {
            dst: Reg::phys(8),
            mem: Mem::Address {
                base: Base::Sp,
                offset,
            },
            access,
        };
        let byte = Access::Byte { signed: false };
        assert_eq!(size(&at(4095, byte), 0), 1);
        assert_eq!(size(&at(4096, byte), 0), 2);
        assert_eq!(size(&at(8190, Access::Half { signed: true }), 0), 1);
        assert_eq!(size(&at(8192, Access::Half { signed: true }), 0), 2);
        assert_eq!(size(&at(16380, Access::Word), 0), 1);
        assert_eq!(size(&at(16384, Access::Word), 0), 2);
        assert_eq!(size(&at(32760, Access::Double), 0), 1);
        assert_eq!(size(&at(32768, Access::Double), 0), 2);
    }

    /// A far offset adds the instructions that build it in `x17`.
    #[test]
    fn far_offsets_add_the_cost_of_loading_x17() {
        assert_eq!(size(&load_at(32760), 0), 1);
        assert_eq!(size(&load_at(32768), 0), 2);
        assert_eq!(size(&load_at(1 << 20), 0), 3);
    }

    /// The prologue grows once the frame size no longer fits in 12 bits.
    #[test]
    fn prologue_size_depends_on_the_frame() {
        assert_eq!(size(&MInst::Prologue, 4080), 3);
        assert_eq!(size(&MInst::Prologue, 4096), 5);
        assert_eq!(size(&MInst::Prologue, 70_000), 6);
    }

    /// Labels print nothing; the epilogue and address loads have fixed sizes.
    #[test]
    fn fixed_sizes() {
        assert_eq!(size(&MInst::Label("L".to_string()), 0), 0);
        assert_eq!(size(&MInst::Epilogue, 0), 3);
        assert_eq!(size(&MInst::Brk, 0), 1);
        let load_address = MInst::LoadAddress {
            dst: Reg::phys(2),
            label: "L".to_string(),
        };
        assert_eq!(size(&load_address, 0), 2);
    }
}
