//! Printing: writes a function in MIR with physical registers and stack addresses as assembly text.

use source::Diagnostic;

use crate::asm::Asm;
use crate::frame::{MAX_IMMEDIATE_FRAME, max_access_offset};
use crate::mir::{Access, Base, Cond, Ext, MFunction, MInst, Mem, Reg, Size, internal_error};

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
        MInst::MovImm { dst, value, size } => {
            asm.load_imm(&sized(*dst, *size)?, size.truncate(*value));
        }
        MInst::Mov { dst, src } => asm.inst(&format!("mov {}, {}", x(*dst)?, x(*src)?)),
        MInst::LoadAddress { dst, label } => asm.load_address(&x(*dst)?, label),
        MInst::Load { dst, mem, access } => print_load(asm, *dst, *mem, *access)?,
        MInst::Store { src, mem, access } => print_store(asm, *src, *mem, *access)?,
        MInst::Adds {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "adds", [*dst, *lhs, *rhs], *size)?,
        MInst::Subs {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "subs", [*dst, *lhs, *rhs], *size)?,
        MInst::Add {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "add", [*dst, *lhs, *rhs], *size)?,
        MInst::Sub {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "sub", [*dst, *lhs, *rhs], *size)?,
        MInst::Negs { dst, src, size } => print_two(asm, "negs", *dst, *src, *size)?,
        MInst::Neg { dst, src, size } => print_two(asm, "neg", *dst, *src, *size)?,
        MInst::Mul {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "mul", [*dst, *lhs, *rhs], *size)?,
        MInst::Smulh { dst, lhs, rhs } => print_three(asm, "smulh", [*dst, *lhs, *rhs], Size::X)?,
        MInst::Umulh { dst, lhs, rhs } => print_three(asm, "umulh", [*dst, *lhs, *rhs], Size::X)?,
        MInst::Smull { dst, lhs, rhs } => print_widening(asm, "smull", *dst, *lhs, *rhs)?,
        MInst::Umull { dst, lhs, rhs } => print_widening(asm, "umull", *dst, *lhs, *rhs)?,
        MInst::Sdiv {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "sdiv", [*dst, *lhs, *rhs], *size)?,
        MInst::Udiv {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "udiv", [*dst, *lhs, *rhs], *size)?,
        MInst::Msub {
            dst,
            mul_lhs,
            mul_rhs,
            minuend,
            size,
        } => asm.inst(&format!(
            "msub {}, {}, {}, {}",
            sized(*dst, *size)?,
            sized(*mul_lhs, *size)?,
            sized(*mul_rhs, *size)?,
            sized(*minuend, *size)?
        )),
        MInst::And {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "and", [*dst, *lhs, *rhs], *size)?,
        MInst::Orr {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "orr", [*dst, *lhs, *rhs], *size)?,
        MInst::Eor {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "eor", [*dst, *lhs, *rhs], *size)?,
        MInst::Lsl {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "lsl", [*dst, *lhs, *rhs], *size)?,
        MInst::Lsr {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "lsr", [*dst, *lhs, *rhs], *size)?,
        MInst::Asr {
            dst,
            lhs,
            rhs,
            size,
        } => print_three(asm, "asr", [*dst, *lhs, *rhs], *size)?,
        MInst::Extend { dst, src, ext } => {
            if *ext == Ext::Sxtw {
                return Err(internal_error(
                    "an extension to 64 bits was selected on w registers",
                ));
            }
            print_two(asm, extension(*ext), *dst, *src, Size::W)?;
        }
        MInst::Cmp { lhs, rhs, size } => asm.inst(&format!(
            "cmp {}, {}",
            sized(*lhs, *size)?,
            sized(*rhs, *size)?
        )),
        MInst::CmpAsr63 { lhs, rhs } => {
            asm.inst(&format!("cmp {}, {}, asr #63", x(*lhs)?, x(*rhs)?));
        }
        MInst::CmpExt {
            lhs,
            rhs,
            ext,
            size,
        } => asm.inst(&format!(
            "cmp {}, {}, {}",
            sized(*lhs, *size)?,
            sized(*rhs, Size::W)?,
            extension(*ext)
        )),
        MInst::CmpImm { lhs, imm, size } => {
            asm.inst(&format!("cmp {}, #{imm}", sized(*lhs, *size)?));
        }
        MInst::CmnImm { lhs, imm, size } => {
            asm.inst(&format!("cmn {}, #{imm}", sized(*lhs, *size)?));
        }
        MInst::Tst { reg, imm, size } => {
            asm.inst(&format!("tst {}, #{imm:#x}", sized(*reg, *size)?));
        }
        MInst::Sxtw { dst, src } => {
            asm.inst(&format!("sxtw {}, {}", x(*dst)?, sized(*src, Size::W)?));
        }
        // A write to a `w` register clears the top half of the `x` register.
        MInst::Uxtw { dst, src } => {
            asm.inst(&format!(
                "mov {}, {}",
                sized(*dst, Size::W)?,
                sized(*src, Size::W)?
            ));
        }
        MInst::Cset { dst, cond } => {
            asm.inst(&format!("cset {}, {}", x(*dst)?, condition(*cond)));
        }
        MInst::EorOne { dst, src } => {
            asm.inst(&format!("eor {}, {}, #1", x(*dst)?, x(*src)?));
        }
        MInst::B { target } => asm.inst(&format!("b {target}")),
        MInst::BCond { cond, target } => asm.inst(&format!("b.{} {target}", condition(*cond))),
        MInst::Cbnz { reg, target } => asm.inst(&format!("cbnz {}, {target}", x(*reg)?)),
        MInst::Cbz { reg, target } => asm.inst(&format!("cbz {}, {target}", x(*reg)?)),
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

/// Writes an instruction with a destination and two source registers of the same size.
fn print_three(
    asm: &mut Asm,
    mnemonic: &str,
    [dst, lhs, rhs]: [Reg; 3],
    size: Size,
) -> Result<(), Diagnostic> {
    asm.inst(&format!(
        "{mnemonic} {}, {}, {}",
        sized(dst, size)?,
        sized(lhs, size)?,
        sized(rhs, size)?
    ));
    Ok(())
}

/// Writes an instruction with a destination and one source register of the same size.
fn print_two(
    asm: &mut Asm,
    mnemonic: &str,
    dst: Reg,
    src: Reg,
    size: Size,
) -> Result<(), Diagnostic> {
    asm.inst(&format!(
        "{mnemonic} {}, {}",
        sized(dst, size)?,
        sized(src, size)?
    ));
    Ok(())
}

/// Writes `smull` or `umull`: an `x` result from two `w` operands.
fn print_widening(
    asm: &mut Asm,
    mnemonic: &str,
    dst: Reg,
    lhs: Reg,
    rhs: Reg,
) -> Result<(), Diagnostic> {
    asm.inst(&format!(
        "{mnemonic} {}, {}, {}",
        x(dst)?,
        sized(lhs, Size::W)?,
        sized(rhs, Size::W)?
    ));
    Ok(())
}

/// Saves the caller's registers and reserves the frame.
fn print_prologue(asm: &mut Asm, frame_size: u64) {
    asm.prologue();
    if frame_size <= MAX_IMMEDIATE_FRAME {
        asm.inst(&format!("sub sp, sp, #{frame_size}"));
    } else {
        // The immediate of `sub` has only 12 bits, so a big size goes through a register. The probe
        // touches every page of the new frame before `sp` moves. It keeps `x9` (clang relies on this
        // too); `x16` and `x17` would not do, because a linker stub on `bl` may overwrite them.
        asm.load_imm("x9", frame_size);
        asm.inst("bl ___chkstk_darwin");
        asm.inst("sub sp, sp, x9");
    }
}

/// Writes a load: the mnemonic and register name follow the access width.
fn print_load(asm: &mut Asm, dst: Reg, mem: Mem, access: Access) -> Result<(), Diagnostic> {
    let (mnemonic, size) = match access {
        Access::Byte { signed: true } => ("ldrsb", Size::W),
        Access::Byte { signed: false } => ("ldrb", Size::W),
        Access::Half { signed: true } => ("ldrsh", Size::W),
        Access::Half { signed: false } => ("ldrh", Size::W),
        Access::Word => ("ldr", Size::W),
        Access::Double => ("ldr", Size::X),
    };
    print_access(asm, mnemonic, sized(dst, size)?, mem, access)
}

/// Writes a store: the mnemonic and register name follow the access width.
fn print_store(asm: &mut Asm, src: Reg, mem: Mem, access: Access) -> Result<(), Diagnostic> {
    let (mnemonic, size) = match access {
        Access::Byte { .. } => ("strb", Size::W),
        Access::Half { .. } => ("strh", Size::W),
        Access::Word => ("str", Size::W),
        Access::Double => ("str", Size::X),
    };
    print_access(asm, mnemonic, sized(src, size)?, mem, access)
}

/// Writes `ldr` or `str` (any width) for a stack address.
fn print_access(
    asm: &mut Asm,
    mnemonic: &str,
    reg: String,
    mem: Mem,
    access: Access,
) -> Result<(), Diagnostic> {
    let Mem::Address { base, offset } = mem else {
        return Err(internal_error(
            "a memory operand was not placed in the frame",
        ));
    };
    let base = match base {
        Base::Sp => "sp",
        Base::Fp => "x29",
    };
    if offset <= max_access_offset(access) {
        asm.inst(&format!("{mnemonic} {reg}, [{base}, #{offset}]"));
    } else {
        // The immediate form stops at 4095 times the access size, so a far address gets its offset in a register.
        // x17 is safe: it is set and used by back-to-back instructions, never across a `bl`.
        asm.load_imm("x17", offset);
        asm.inst(&format!("{mnemonic} {reg}, [{base}, x17]"));
    }
    Ok(())
}

/// The text of a physical register as an `x` register.
fn x(reg: Reg) -> Result<String, Diagnostic> {
    sized(reg, Size::X)
}

/// The text of a physical register with the `w` or `x` name; a virtual one means the allocator missed it.
fn sized(reg: Reg, size: Size) -> Result<String, Diagnostic> {
    let letter = match size {
        Size::W => 'w',
        Size::X => 'x',
    };
    match reg {
        Reg::Phys(register) => Ok(format!("{letter}{}", register.0)),
        Reg::Virt(_) => Err(internal_error("a virtual register was not allocated")),
    }
}

/// The extension named in `cmp ..., <ext>`.
fn extension(ext: Ext) -> &'static str {
    match ext {
        Ext::Sxtb => "sxtb",
        Ext::Sxth => "sxth",
        Ext::Sxtw => "sxtw",
        Ext::Uxtb => "uxtb",
        Ext::Uxth => "uxth",
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
        Cond::Lo => "lo",
        Cond::Ls => "ls",
        Cond::Hi => "hi",
        Cond::Hs => "hs",
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

    /// The text `inst` prints as, trimmed.
    fn printed_text(inst: &MInst) -> String {
        let mut asm = Asm::new();
        assert!(print_inst(&mut asm, inst, 0).is_ok());
        asm.finish().trim().to_string()
    }

    /// Every access width, with both signednesses of the narrow loads.
    const ACCESSES: [Access; 6] = [
        Access::Byte { signed: true },
        Access::Byte { signed: false },
        Access::Half { signed: true },
        Access::Half { signed: false },
        Access::Word,
        Access::Double,
    ];

    /// Every instruction form, with constants and offsets on both sides of each encoding limit.
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
            MInst::Smulh {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Umulh {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Smull {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::Umull {
                dst: reg,
                lhs: reg,
                rhs: reg,
            },
            MInst::CmpAsr63 { lhs: reg, rhs: reg },
            MInst::Sxtw { dst: reg, src: reg },
            MInst::Uxtw { dst: reg, src: reg },
            MInst::Cset {
                dst: reg,
                cond: Cond::Lo,
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
                cond: Cond::Hs,
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
        for size in [Size::W, Size::X] {
            insts.extend([
                MInst::Adds {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Subs {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Add {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Sub {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Neg {
                    dst: reg,
                    src: reg,
                    size,
                },
                MInst::Negs {
                    dst: reg,
                    src: reg,
                    size,
                },
                MInst::Mul {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Sdiv {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Udiv {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Msub {
                    dst: reg,
                    mul_lhs: reg,
                    mul_rhs: reg,
                    minuend: reg,
                    size,
                },
                MInst::Cmp {
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::CmpImm {
                    lhs: reg,
                    imm: 0,
                    size,
                },
                MInst::CmnImm {
                    lhs: reg,
                    imm: 1,
                    size,
                },
                MInst::Tst {
                    reg,
                    imm: 0xffff_ff00,
                    size,
                },
            ]);
            for ext in [Ext::Sxtb, Ext::Sxth, Ext::Sxtw, Ext::Uxtb, Ext::Uxth] {
                insts.push(MInst::CmpExt {
                    lhs: reg,
                    rhs: reg,
                    ext,
                    size,
                });
            }
            insts.extend([
                MInst::And {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Orr {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Eor {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Lsl {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Lsr {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
                MInst::Asr {
                    dst: reg,
                    lhs: reg,
                    rhs: reg,
                    size,
                },
            ]);
        }
        for ext in [Ext::Sxtb, Ext::Sxth, Ext::Uxtb, Ext::Uxth] {
            insts.push(MInst::Extend {
                dst: reg,
                src: reg,
                ext,
            });
        }
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
            for size in [Size::W, Size::X] {
                insts.push(MInst::MovImm {
                    dst: reg,
                    value,
                    size,
                });
            }
            for base in [Base::Sp, Base::Fp] {
                let mem = Mem::Address { base, offset };
                for access in ACCESSES {
                    insts.push(MInst::Load {
                        dst: reg,
                        mem,
                        access,
                    });
                    insts.push(MInst::Store {
                        src: reg,
                        mem,
                        access,
                    });
                }
            }
        }
        // Offsets around the immediate limit of each access width.
        for access in ACCESSES {
            let limit = max_access_offset(access);
            for offset in [
                limit - access.bytes(),
                limit,
                limit + access.bytes(),
                limit + 8,
            ] {
                let mem = Mem::Address {
                    base: Base::Sp,
                    offset,
                };
                insts.push(MInst::Load {
                    dst: reg,
                    mem,
                    access,
                });
                insts.push(MInst::Store {
                    src: reg,
                    mem,
                    access,
                });
            }
        }
        insts
    }

    /// The size table matches the printer for every instruction, so branch relaxation can trust it.
    #[test]
    fn size_table_matches_the_printed_instruction_count() {
        let frames = [
            0,
            16,
            4080,
            4096,
            4112,
            65536,
            70_000,
            1 << 20,
            4 * 1024 * 1024,
            1 << 32,
        ];
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

    /// Every `Access` prints its own mnemonic and register width.
    #[test]
    fn accesses_print_their_mnemonics() {
        let mem = Mem::Address {
            base: Base::Sp,
            offset: 8,
        };
        let cases = [
            (
                Access::Byte { signed: true },
                "ldrsb w8, [sp, #8]",
                "strb w8, [sp, #8]",
            ),
            (
                Access::Byte { signed: false },
                "ldrb w8, [sp, #8]",
                "strb w8, [sp, #8]",
            ),
            (
                Access::Half { signed: true },
                "ldrsh w8, [sp, #8]",
                "strh w8, [sp, #8]",
            ),
            (
                Access::Half { signed: false },
                "ldrh w8, [sp, #8]",
                "strh w8, [sp, #8]",
            ),
            (Access::Word, "ldr w8, [sp, #8]", "str w8, [sp, #8]"),
            (Access::Double, "ldr x8, [sp, #8]", "str x8, [sp, #8]"),
        ];
        for (access, load, store) in cases {
            let dst = Reg::phys(8);
            assert_eq!(printed_text(&MInst::Load { dst, mem, access }), load);
            assert_eq!(
                printed_text(&MInst::Store {
                    src: dst,
                    mem,
                    access
                }),
                store
            );
        }
    }

    /// The immediate of `tst` prints in lowercase hex, and a `w` constant keeps only its low 32 bits.
    #[test]
    fn tst_and_w_constants_print_as_specified() {
        let reg = Reg::phys(8);
        let tst = MInst::Tst {
            reg,
            imm: 0xffff_ffff_0000_0000,
            size: Size::X,
        };
        assert_eq!(printed_text(&tst), "tst x8, #0xffffffff00000000");
        let constant = MInst::MovImm {
            dst: reg,
            value: u64::MAX,
            size: Size::W,
        };
        assert_eq!(
            printed_text(&constant),
            "movz w8, #65535\n    movk w8, #65535, lsl #16"
        );
    }
}
