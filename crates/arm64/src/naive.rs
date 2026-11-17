//! The naive allocator: every virtual register lives in its own stack cell and is reloaded for each use.

use crate::mir::{Access, Ext, MFunction, MInst, Mem, Reg, RegRole, VReg, internal_error};
use source::Diagnostic;

/// The scratch registers `x8` to `x11`, handed out in this order, so one instruction reads at most four values.
const FIRST_SCRATCH: u8 = 8;
/// How many scratch registers there are.
const SCRATCH_COUNT: usize = 4;

/// Replaces every virtual register of `function` with physical registers and stack cells.
pub fn allocate(mut function: MFunction) -> Result<MFunction, Diagnostic> {
    let mut insts = Vec::with_capacity(function.insts.len() * 2);
    // The store of a result waits until after a panic branch that directly follows its instruction,
    // so the flag check stays next to the instruction and the store sits right before the next reload.
    let mut pending_store: Option<MInst> = None;
    for inst in function.insts {
        if matches!(inst, MInst::PanicIf { .. }) {
            // Safe: a panic path never reads the value, so skipping the store when it is taken is harmless.
            insts.push(inst);
            insts.extend(pending_store.take());
            continue;
        }
        if let Some(ext) = in_place_extension(&inst, pending_store.as_ref()) {
            // The value is still in `x8`, so it is extended there and stored later.
            insts.push(MInst::Extend {
                dst: scratch(0),
                src: scratch(0),
                ext,
            });
            continue;
        }
        insts.extend(pending_store.take());
        pending_store = rewrite(inst, &mut insts)?;
    }
    insts.extend(pending_store);
    function.insts = insts;
    Ok(function)
}

/// The extension `inst` applies, when it extends in place the register whose store is still waiting.
fn in_place_extension(inst: &MInst, pending_store: Option<&MInst>) -> Option<Ext> {
    let MInst::Extend {
        dst: Reg::Virt(dst),
        src: Reg::Virt(src),
        ext,
    } = inst
    else {
        return None;
    };
    // The store waits with its value in `x8`, so `x8` still holds the register that `inst` extends.
    let Some(MInst::Store {
        src: stored,
        mem: Mem::Cell(cell),
        ..
    }) = pending_store
    else {
        return None;
    };
    if *stored == scratch(0) && dst == src && dst == cell {
        Some(*ext)
    } else {
        None
    }
}

/// Appends the allocated form of `inst` to `out`, except the store of its result, which is returned.
fn rewrite(inst: MInst, out: &mut Vec<MInst>) -> Result<Option<MInst>, Diagnostic> {
    match inst {
        MInst::Mov { dst, src } => {
            rewrite_mov(dst, src, out);
            Ok(None)
        }
        other => rewrite_with_scratch(other, out),
    }
}

/// A copy to or from a physical register goes straight to memory; a copy between two virtual registers uses `x8`.
fn rewrite_mov(dst: Reg, src: Reg, out: &mut Vec<MInst>) {
    let scratch = Reg::phys(FIRST_SCRATCH);
    match (dst, src) {
        (Reg::Phys(_), Reg::Virt(source)) => out.push(MInst::Load {
            dst,
            mem: Mem::Cell(source),
            access: Access::Double,
        }),
        (Reg::Virt(target), Reg::Phys(_)) => out.push(MInst::Store {
            src,
            mem: Mem::Cell(target),
            access: Access::Double,
        }),
        (Reg::Virt(target), Reg::Virt(source)) => {
            out.push(MInst::Load {
                dst: scratch,
                mem: Mem::Cell(source),
                access: Access::Double,
            });
            out.push(MInst::Store {
                src: scratch,
                mem: Mem::Cell(target),
                access: Access::Double,
            });
        }
        (Reg::Phys(_), Reg::Phys(_)) => out.push(MInst::Mov { dst, src }),
    }
}

/// Loads each virtual register the instruction reads into a scratch register, runs the instruction on
/// the scratch registers, and returns the store of the register it writes.
fn rewrite_with_scratch(
    mut inst: MInst,
    out: &mut Vec<MInst>,
) -> Result<Option<MInst>, Diagnostic> {
    // The distinct virtual registers read, in operand order; each gets the next scratch register.
    // The result goes in `x8`, which is also the scratch register of the first read.
    let mut reads: Vec<VReg> = Vec::new();
    let mut written: Option<VReg> = None;
    inst.for_each_reg(|reg, role| {
        let Reg::Virt(vreg) = *reg else {
            return;
        };
        match role {
            RegRole::Use => {
                let index = match reads.iter().position(|read| *read == vreg) {
                    Some(index) => index,
                    None => {
                        reads.push(vreg);
                        reads.len() - 1
                    }
                };
                *reg = scratch(index);
            }
            RegRole::Def => {
                written = Some(vreg);
                *reg = scratch(0);
            }
        }
    });
    if reads.len() > SCRATCH_COUNT {
        return Err(internal_error(
            "an instruction reads more than four virtual registers",
        ));
    }
    for (index, vreg) in reads.iter().enumerate() {
        out.push(MInst::Load {
            dst: scratch(index),
            mem: Mem::Cell(*vreg),
            access: Access::Double,
        });
    }
    out.push(inst);
    Ok(written.map(|vreg| MInst::Store {
        src: scratch(0),
        mem: Mem::Cell(vreg),
        access: Access::Double,
    }))
}

/// The `index`-th scratch register.
fn scratch(index: usize) -> Reg {
    Reg::phys(FIRST_SCRATCH + index as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::Size;
    use source::Span;

    /// A virtual register.
    fn v(number: u32) -> Reg {
        Reg::Virt(VReg(number))
    }

    /// Allocates one instruction and returns the result.
    fn allocated(inst: MInst) -> Vec<MInst> {
        let mut out = Vec::new();
        let store = rewrite(inst, &mut out);
        assert!(store.is_ok());
        out.extend(store.ok().flatten());
        out
    }

    /// A load from the cell of a virtual register into a physical one.
    fn load(register: u8, vreg: u32) -> MInst {
        MInst::Load {
            dst: Reg::phys(register),
            mem: Mem::Cell(VReg(vreg)),
            access: Access::Double,
        }
    }

    /// An instruction that reads one register twice loads it once.
    #[test]
    fn a_register_read_twice_is_loaded_once() {
        let add = MInst::Adds {
            dst: v(2),
            lhs: v(0),
            rhs: v(0),
            size: Size::X,
        };
        let expected = vec![
            load(8, 0),
            MInst::Adds {
                dst: Reg::phys(8),
                lhs: Reg::phys(8),
                rhs: Reg::phys(8),
                size: Size::X,
            },
            MInst::Store {
                src: Reg::phys(8),
                mem: Mem::Cell(VReg(2)),
                access: Access::Double,
            },
        ];
        assert_eq!(allocated(add), expected);
    }

    /// A copy between two virtual registers goes through `x8`.
    #[test]
    fn copy_between_virtual_registers_uses_x8() {
        let mov = MInst::Mov {
            dst: v(1),
            src: v(0),
        };
        let expected = vec![
            load(8, 0),
            MInst::Store {
                src: Reg::phys(8),
                mem: Mem::Cell(VReg(1)),
                access: Access::Double,
            },
        ];
        assert_eq!(allocated(mov), expected);
    }

    /// A function around `insts`, with enough cells for the registers the tests use.
    fn function_of(insts: Vec<MInst>) -> MFunction {
        MFunction {
            name: "f".to_string(),
            slot_count: 0,
            vreg_count: 4,
            span: Span::new(0, 0),
            out_arg_bytes: 0,
            next_label: 0,
            insts,
            frame_size: 0,
        }
    }

    /// The store of a result moves after a panic branch that follows it, but not after another branch.
    #[test]
    fn store_waits_for_a_panic_branch_only() {
        let add = MInst::Adds {
            dst: v(2),
            lhs: v(0),
            rhs: v(1),
            size: Size::X,
        };
        let panic_if = MInst::PanicIf {
            cond: crate::mir::Cond::Vs,
            target: "P".to_string(),
        };
        let b_cond = MInst::BCond {
            cond: crate::mir::Cond::Vs,
            target: "P".to_string(),
        };
        let store = MInst::Store {
            src: Reg::phys(8),
            mem: Mem::Cell(VReg(2)),
            access: Access::Double,
        };
        let after_panic_if = allocate(function_of(vec![add.clone(), panic_if.clone()]));
        let after_b_cond = allocate(function_of(vec![add, b_cond.clone()]));
        assert!(after_panic_if.is_ok() && after_b_cond.is_ok());
        let panic_insts = after_panic_if.map(|f| f.insts).unwrap_or_default();
        let b_cond_insts = after_b_cond.map(|f| f.insts).unwrap_or_default();
        assert_eq!(panic_insts[3..], [panic_if, store.clone()]);
        assert_eq!(b_cond_insts[3..], [store, b_cond]);
    }
}
