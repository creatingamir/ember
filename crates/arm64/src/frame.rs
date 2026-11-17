//! Frame layout: the frame size, and the stack address of every cell.

use source::Diagnostic;

use crate::mir::{MFunction, Mem, internal_error};

/// The largest frame that `sub sp, sp, #imm` can reserve (a 12-bit immediate).
pub const MAX_IMMEDIATE_FRAME: u64 = 4095;
/// The largest offset that `ldr`/`str` encode as an immediate: 4095 cells of 8 bytes.
pub const MAX_ACCESS_OFFSET: u64 = 4095 * 8;
/// The size of one stack cell in bytes.
const CELL_SIZE: u64 = 8;

/// Sets the frame size and turns every slot and cell into a stack address.
pub fn lay_out(mut function: MFunction) -> Result<MFunction, Diagnostic> {
    // Rounded up to a multiple of 16 so that sp stays aligned.
    let slot_count = function.slot_count;
    let vreg_count = function.vreg_count;
    let cell_count = u64::from(slot_count) + u64::from(vreg_count);
    function.frame_size = (cell_count * CELL_SIZE).div_ceil(16) * 16;

    for inst in &mut function.insts {
        if let Some(mem) = inst.mem_mut() {
            *mem = place(*mem, slot_count, vreg_count)?;
        }
    }
    Ok(function)
}

/// The stack address of a slot or a virtual register's cell: slots come first, then the registers.
fn place(mem: Mem, slot_count: u32, vreg_count: u32) -> Result<Mem, Diagnostic> {
    let cell = match mem {
        Mem::Slot(slot) if slot.0 < slot_count => u64::from(slot.0),
        Mem::Cell(vreg) if vreg.0 < vreg_count => u64::from(slot_count) + u64::from(vreg.0),
        Mem::Address { .. } => return Ok(mem),
        Mem::Slot(_) | Mem::Cell(_) => {
            return Err(internal_error("a slot or temp does not exist"));
        }
    };
    Ok(Mem::Address {
        offset: cell * CELL_SIZE,
    })
}

/// How many 4-byte machine instructions each MIR instruction prints as (branch relaxation needs exact sizes).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "branch relaxation (M2.1b) is the first caller")
)]
pub mod size_table {
    use super::{MAX_ACCESS_OFFSET, MAX_IMMEDIATE_FRAME};
    use crate::mir::{MInst, Mem};

    /// The number of instructions `inst` prints as, for a function with `frame_size`.
    pub fn size(inst: &MInst, frame_size: u64) -> u32 {
        match inst {
            MInst::Label(_) => 0,
            MInst::MovImm { value, .. } => mov_imm_size(*value),
            MInst::LoadAddress { .. } => 2,
            MInst::Load { mem, .. } | MInst::Store { mem, .. } => 1 + far_offset_size(mem),
            MInst::Prologue => {
                if frame_size <= MAX_IMMEDIATE_FRAME {
                    3
                } else {
                    3 + mov_imm_size(frame_size)
                }
            }
            MInst::Epilogue => 3,
            // Listed one by one, without `_`, so a new instruction kind fails to
            // compile until its size is decided (branch relaxation relies on it).
            MInst::Mov { .. }
            | MInst::Adds { .. }
            | MInst::Subs { .. }
            | MInst::Negs { .. }
            | MInst::Mul { .. }
            | MInst::Smulh { .. }
            | MInst::Sdiv { .. }
            | MInst::Msub { .. }
            | MInst::Cmp { .. }
            | MInst::CmpAsr63 { .. }
            | MInst::CmpImm { .. }
            | MInst::CmnImm { .. }
            | MInst::Cset { .. }
            | MInst::EorOne { .. }
            | MInst::B { .. }
            | MInst::BCond { .. }
            | MInst::Cbnz { .. }
            | MInst::PanicIf { .. }
            | MInst::Bl { .. }
            | MInst::Brk => 1,
        }
    }

    /// The extra instructions a memory operand needs to load a far offset into `x17`.
    fn far_offset_size(mem: &Mem) -> u32 {
        match mem {
            Mem::Address { offset } if *offset > MAX_ACCESS_OFFSET => mov_imm_size(*offset),
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
    use crate::mir::{MInst, Reg};

    /// A load or store at `offset`.
    fn load_at(offset: u64) -> MInst {
        MInst::Load {
            dst: Reg::phys(8),
            mem: Mem::Address { offset },
        }
    }

    /// A constant takes one `movz` plus one `movk` per non-zero upper chunk.
    #[test]
    fn constants_count_their_non_zero_chunks() {
        let dst = Reg::phys(8);
        assert_eq!(size(&MInst::MovImm { dst, value: 0 }, 0), 1);
        assert_eq!(size(&MInst::MovImm { dst, value: 65535 }, 0), 1);
        assert_eq!(
            size(
                &MInst::MovImm {
                    dst,
                    value: 1 << 16
                },
                0
            ),
            2
        );
        assert_eq!(
            size(
                &MInst::MovImm {
                    dst,
                    value: i64::MIN as u64
                },
                0
            ),
            2
        );
        assert_eq!(
            size(
                &MInst::MovImm {
                    dst,
                    value: u64::MAX
                },
                0
            ),
            4
        );
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
        assert_eq!(size(&MInst::Prologue, 4096), 4);
        assert_eq!(size(&MInst::Prologue, 70_000), 5);
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
