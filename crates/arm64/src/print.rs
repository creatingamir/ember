//! Printing: writes a function in MIR with physical registers and stack addresses as assembly text.

use source::Diagnostic;

use crate::asm::Asm;
use crate::frame::{MAX_ACCESS_OFFSET, MAX_IMMEDIATE_FRAME};
use crate::mir::{Cond, MFunction, MInst, Mem, Reg, internal_error};

/// Writes `function` as the global symbol `_em_<name>`, or reports an operand that was not allocated or placed.
pub fn print(asm: &mut Asm, function: &MFunction) -> Result<(), Diagnostic> {
    let symbol = format!("_em_{}", function.name);
    asm.directive(&format!(".globl {symbol}"));
    asm.directive(".p2align 2");
    asm.label(&symbol);
    for inst in &function.insts {
        print_inst(asm, inst, function.frame_size)?;
    }
    Ok(())
}

/// Writes one instruction.
fn print_inst(asm: &mut Asm, inst: &MInst, frame_size: u64) -> Result<(), Diagnostic> {
    match inst {
        MInst::Label(name) => asm.label(name),
        MInst::MovImm { dst, value } => asm.load_imm(&name(*dst)?, *value),
        MInst::Mov { dst, src } => asm.inst(&format!("mov {}, {}", name(*dst)?, name(*src)?)),
        MInst::LoadAddress { dst, label } => asm.load_address(&name(*dst)?, label),
        MInst::Load { dst, mem } => print_access(asm, "ldr", *dst, *mem)?,
        MInst::Store { src, mem } => print_access(asm, "str", *src, *mem)?,
        MInst::Adds { dst, lhs, rhs } => print_three(asm, "adds", *dst, *lhs, *rhs)?,
        MInst::Subs { dst, lhs, rhs } => print_three(asm, "subs", *dst, *lhs, *rhs)?,
        MInst::Negs { dst, src } => asm.inst(&format!("negs {}, {}", name(*dst)?, name(*src)?)),
        MInst::Mul { dst, lhs, rhs } => print_three(asm, "mul", *dst, *lhs, *rhs)?,
        MInst::Smulh { dst, lhs, rhs } => print_three(asm, "smulh", *dst, *lhs, *rhs)?,
        MInst::Sdiv { dst, lhs, rhs } => print_three(asm, "sdiv", *dst, *lhs, *rhs)?,
        MInst::Msub {
            dst,
            mul_lhs,
            mul_rhs,
            minuend,
        } => asm.inst(&format!(
            "msub {}, {}, {}, {}",
            name(*dst)?,
            name(*mul_lhs)?,
            name(*mul_rhs)?,
            name(*minuend)?
        )),
        MInst::Cmp { lhs, rhs } => asm.inst(&format!("cmp {}, {}", name(*lhs)?, name(*rhs)?)),
        MInst::CmpAsr63 { lhs, rhs } => {
            asm.inst(&format!("cmp {}, {}, asr #63", name(*lhs)?, name(*rhs)?));
        }
        MInst::CmpImm { lhs, imm } => asm.inst(&format!("cmp {}, #{imm}", name(*lhs)?)),
        MInst::CmnImm { lhs, imm } => asm.inst(&format!("cmn {}, #{imm}", name(*lhs)?)),
        MInst::Cset { dst, cond } => {
            asm.inst(&format!("cset {}, {}", name(*dst)?, condition(*cond)));
        }
        MInst::EorOne { dst, src } => {
            asm.inst(&format!("eor {}, {}, #1", name(*dst)?, name(*src)?));
        }
        MInst::B { target } => asm.inst(&format!("b {target}")),
        MInst::BCond { cond, target } => asm.inst(&format!("b.{} {target}", condition(*cond))),
        MInst::Cbnz { reg, target } => asm.inst(&format!("cbnz {}, {target}", name(*reg)?)),
        MInst::Cbz { reg, target } => asm.inst(&format!("cbz {}, {target}", name(*reg)?)),
        MInst::PanicIf { cond, target } => {
            asm.inst(&format!("b.{} {target}", condition(*cond)));
        }
        MInst::Bl { symbol, .. } => asm.inst(&format!("bl {symbol}")),
        MInst::Brk => asm.inst("brk #1"),
        MInst::Prologue => print_prologue(asm, frame_size),
        MInst::Epilogue => asm.epilogue(),
    }
    Ok(())
}

/// Writes an instruction with a destination and two source registers.
fn print_three(
    asm: &mut Asm,
    mnemonic: &str,
    dst: Reg,
    lhs: Reg,
    rhs: Reg,
) -> Result<(), Diagnostic> {
    asm.inst(&format!(
        "{mnemonic} {}, {}, {}",
        name(dst)?,
        name(lhs)?,
        name(rhs)?
    ));
    Ok(())
}

/// Saves the caller's registers and reserves the frame.
fn print_prologue(asm: &mut Asm, frame_size: u64) {
    asm.prologue();
    if frame_size <= MAX_IMMEDIATE_FRAME {
        asm.inst(&format!("sub sp, sp, #{frame_size}"));
    } else {
        // The immediate of `sub` has only 12 bits, so a big size goes through a register.
        asm.load_imm("x16", frame_size);
        asm.inst("sub sp, sp, x16");
    }
}

/// Writes `ldr` or `str` for a stack address.
fn print_access(asm: &mut Asm, mnemonic: &str, reg: Reg, mem: Mem) -> Result<(), Diagnostic> {
    let Mem::Address { offset } = mem else {
        return Err(internal_error(
            "a memory operand was not placed in the frame",
        ));
    };
    let reg = name(reg)?;
    if offset <= MAX_ACCESS_OFFSET {
        asm.inst(&format!("{mnemonic} {reg}, [sp, #{offset}]"));
    } else {
        // The immediate form stops at 32760 bytes, so a far cell gets its offset in a register.
        // x17 is safe: it is set and used by back-to-back instructions, never across a `bl`.
        asm.load_imm("x17", offset);
        asm.inst(&format!("{mnemonic} {reg}, [sp, x17]"));
    }
    Ok(())
}

/// The text of a physical register; a virtual one means the allocator missed it.
fn name(reg: Reg) -> Result<String, Diagnostic> {
    match reg {
        Reg::Phys(register) => Ok(format!("x{}", register.0)),
        Reg::Virt(_) => Err(internal_error("a virtual register was not allocated")),
    }
}

/// The condition code as written after `b.` or in `cset`.
fn condition(cond: Cond) -> &'static str {
    match cond {
        Cond::Eq => "eq",
        Cond::Ne => "ne",
        Cond::Lt => "lt",
        Cond::Le => "le",
        Cond::Gt => "gt",
        Cond::Ge => "ge",
        Cond::Vs => "vs",
        Cond::Vc => "vc",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::size_table::size;

    /// The number of instruction lines `inst` prints as.
    fn printed_size(inst: &MInst, frame_size: u64) -> u32 {
        let mut asm = Asm::new();
        assert!(print_inst(&mut asm, inst, frame_size).is_ok());
        let text = asm.finish();
        let instructions = text.lines().filter(|line| line.starts_with("    "));
        instructions.count() as u32
    }

    /// Every instruction kind, with constants and offsets on both sides of each encoding limit.
    fn all_insts() -> Vec<MInst> {
        let reg = Reg::phys(8);
        let label = "L".to_string();
        let mut insts = vec![
            MInst::Label(label.clone()),
            MInst::Mov { dst: reg, src: reg },
            MInst::LoadAddress {
                dst: reg,
                label: label.clone(),
            },
            MInst::Adds {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Subs {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Negs { dst: reg, src: reg },
            MInst::Mul {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Smulh {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Sdiv {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Msub {
                dst: reg,
                mul_lhs: reg,
                mul_rhs: reg,
                minuend: reg,
            },
            MInst::Cmp { lhs: reg, rhs: reg },
            MInst::CmpAsr63 { lhs: reg, rhs: reg },
            MInst::CmpImm { lhs: reg, imm: 0 },
            MInst::CmnImm { lhs: reg, imm: 1 },
            MInst::Cset {
                dst: reg,
                cond: Cond::Lt,
            },
            MInst::EorOne { dst: reg, src: reg },
            MInst::B {
                target: label.clone(),
            },
            MInst::BCond {
                cond: Cond::Vs,
                target: label.clone(),
            },
            MInst::PanicIf {
                cond: Cond::Vs,
                target: label.clone(),
            },
            MInst::Cbnz {
                reg,
                target: label.clone(),
            },
            MInst::Cbz { reg, target: label },
            MInst::Bl {
                symbol: "f".to_string(),
                args: 2,
            },
            MInst::Brk,
            MInst::Epilogue,
        ];
        let values: [u64; 14] = [
            0,
            1,
            32760,
            32768,
            65535,
            65536,
            1 << 20,
            1 << 32,
            0x1_0001_0000,
            1 << 48,
            0xffff_0000_0000_ffff,
            i64::MIN as u64,
            u64::MAX - 7,
            4_000_000_000,
        ];
        for value in values {
            let offset = value & !7;
            insts.push(MInst::MovImm { dst: reg, value });
            insts.push(MInst::Load {
                dst: reg,
                mem: Mem::Address { offset },
            });
            insts.push(MInst::Store {
                src: reg,
                mem: Mem::Address { offset },
            });
        }
        insts
    }

    /// The size table matches the printer for every instruction, so branch relaxation can trust it.
    #[test]
    fn size_table_matches_the_printed_instruction_count() {
        let frames = [0, 16, 4080, 4096, 65536, 1 << 32];
        for frame_size in frames {
            let prologue = MInst::Prologue;
            assert_eq!(
                size(&prologue, frame_size),
                printed_size(&prologue, frame_size),
                "prologue, frame {frame_size}"
            );
            for inst in all_insts() {
                assert_eq!(
                    size(&inst, frame_size),
                    printed_size(&inst, frame_size),
                    "{inst:?}, frame {frame_size}"
                );
            }
        }
    }
}
