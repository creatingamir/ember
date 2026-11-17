//! Branch relaxation: rewrites conditional branches whose target is too far for their encoding.

use std::collections::HashMap;
use std::ops::RangeInclusive;

use source::Diagnostic;

use crate::frame::size_table::size;
use crate::mir::{MFunction, MInst, internal_error};

/// Every instruction is 4 bytes long.
const INSTRUCTION_BYTES: u64 = 4;

/// The distances, in bytes, that a branch can reach.
struct Reach {
    /// `b.<cond>`, `cbz` and `cbnz` have a 19-bit offset counted in instructions.
    conditional: RangeInclusive<i64>,
    /// `b` has a 26-bit offset counted in instructions (128 MiB each way).
    unconditional: RangeInclusive<i64>,
}

/// The reach of the real instructions.
const REACH: Reach = Reach {
    conditional: -1_048_576..=1_048_572,
    unconditional: -134_217_728..=134_217_724,
};

/// Rewrites every conditional branch whose target is out of range; repeats until none is.
pub fn relax(function: MFunction) -> Result<MFunction, Diagnostic> {
    relax_within(function, &REACH)
}

/// `relax` for a given reach, so tests can use small distances.
fn relax_within(mut function: MFunction, reach: &Reach) -> Result<MFunction, Diagnostic> {
    // A rewrite only adds instructions, so distances only grow. Each branch is rewritten at most
    // once (its replacement targets a label two instructions away), so this loop ends.
    loop {
        let far = find_far_branches(&function, reach)?;
        if !far.contains(&true) {
            return Ok(function);
        }
        rewrite_far_branches(&mut function, &far)?;
    }
}

/// Marks, for every instruction, whether it is a conditional branch that cannot reach its target.
///
/// Also reports a branch to a missing label and a `b` that is out of range.
fn find_far_branches(function: &MFunction, reach: &Reach) -> Result<Vec<bool>, Diagnostic> {
    let mut offsets = Vec::with_capacity(function.insts.len());
    let mut label_offsets: HashMap<&str, i64> = HashMap::new();
    let mut offset: u64 = 0;
    for inst in &function.insts {
        offsets.push(offset);
        if let MInst::Label(name) = inst
            && label_offsets.insert(name, offset as i64).is_some()
        {
            return Err(internal_error("a label is defined twice"));
        }
        offset += u64::from(size(inst, function.frame_size)) * INSTRUCTION_BYTES;
    }

    let mut far = vec![false; function.insts.len()];
    for (index, inst) in function.insts.iter().enumerate() {
        let Some(target) = branch_target(inst) else {
            continue;
        };
        let Some(target_offset) = label_offsets.get(target) else {
            return Err(internal_error("a branch targets a missing label"));
        };
        let distance = target_offset - offsets[index] as i64;
        if matches!(inst, MInst::B { .. }) {
            if !reach.unconditional.contains(&distance) {
                return Err(internal_error(
                    "a function is too large for a branch (over 128 MiB)",
                ));
            }
        } else {
            far[index] = !reach.conditional.contains(&distance);
        }
    }
    Ok(far)
}

/// The label a branch jumps to.
fn branch_target(inst: &MInst) -> Option<&str> {
    match inst {
        MInst::B { target }
        | MInst::BCond { target, .. }
        | MInst::PanicIf { target, .. }
        | MInst::Cbnz { target, .. }
        | MInst::Cbz { target, .. } => Some(target),
        _ => None,
    }
}

/// Replaces each marked branch by its inverse that skips a plain `b` to the original target.
fn rewrite_far_branches(function: &mut MFunction, far: &[bool]) -> Result<(), Diagnostic> {
    let old = std::mem::take(&mut function.insts);
    let mut insts = Vec::with_capacity(old.len());
    for (inst, is_far) in old.into_iter().zip(far) {
        if !*is_far {
            insts.push(inst);
            continue;
        }
        let skip = format!("L{}_skip{}", function.name, function.next_label);
        let (inverse, target) = match inst {
            MInst::BCond { cond, target } | MInst::PanicIf { cond, target } => {
                let inverse = MInst::BCond {
                    cond: cond.inverse(),
                    target: skip.clone(),
                };
                (inverse, target)
            }
            MInst::Cbnz { reg, target } => {
                let inverse = MInst::Cbz {
                    reg,
                    target: skip.clone(),
                };
                (inverse, target)
            }
            MInst::Cbz { reg, target } => {
                let inverse = MInst::Cbnz {
                    reg,
                    target: skip.clone(),
                };
                (inverse, target)
            }
            // Only the branches above are ever marked; another kind would never become near.
            _ => return Err(internal_error("a branch kind cannot be relaxed")),
        };
        function.next_label += 1;
        insts.push(inverse);
        insts.push(MInst::B { target });
        insts.push(MInst::Label(skip));
    }
    function.insts = insts;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{Cond, Reg};

    /// A function around `insts` with 10 labels already used by isel.
    fn function_of(insts: Vec<MInst>) -> MFunction {
        MFunction {
            name: "f".to_string(),
            slot_count: 0,
            vreg_count: 0,
            next_label: 10,
            insts,
            frame_size: 0,
        }
    }

    /// `count` one-instruction fillers.
    fn filler(count: usize) -> Vec<MInst> {
        vec![MInst::Brk; count]
    }

    /// A branch exactly at the end of the reach is kept; one instruction further is rewritten.
    #[test]
    fn conditional_reach_ends_at_one_mebibyte() {
        let branch = || MInst::BCond {
            cond: Cond::Vs,
            target: "T".to_string(),
        };
        // The target is 262,143 instructions after the branch (+1,048,572 bytes) ...
        let mut near = vec![branch()];
        near.extend(filler(262_142));
        near.push(MInst::Label("T".to_string()));
        near.push(MInst::Brk);
        let relaxed = relax(function_of(near.clone()));
        assert_eq!(relaxed.map(|f| f.insts), Ok(near));
        // ... and one instruction more is out of range.
        let mut far = vec![branch()];
        far.extend(filler(262_143));
        far.push(MInst::Label("T".to_string()));
        let relaxed = relax(function_of(far));
        assert!(relaxed.is_ok_and(|f| f.insts[0..3]
            == [
                MInst::BCond {
                    cond: Cond::Vc,
                    target: "Lf_skip10".to_string()
                },
                MInst::B {
                    target: "T".to_string()
                },
                MInst::Label("Lf_skip10".to_string()),
            ]));
    }

    /// A far backward branch is rewritten too.
    #[test]
    fn backward_branch_out_of_range_is_rewritten() {
        let mut insts = vec![MInst::Label("T".to_string())];
        insts.extend(filler(262_145));
        insts.push(MInst::PanicIf {
            cond: Cond::Eq,
            target: "T".to_string(),
        });
        let skip = MInst::BCond {
            cond: Cond::Ne,
            target: "Lf_skip10".to_string(),
        };
        let relaxed = relax(function_of(insts));
        assert!(relaxed.is_ok_and(|f| f.insts.contains(&skip)));
    }

    /// `cbz` turns into `cbnz` around a `b`, and `cbnz` into `cbz`.
    #[test]
    fn cbz_and_cbnz_swap_when_relaxed() {
        let reg = Reg::phys(8);
        let mut insts = vec![
            MInst::Cbz {
                reg,
                target: "T".to_string(),
            },
            MInst::Cbnz {
                reg,
                target: "T".to_string(),
            },
        ];
        insts.extend(filler(262_200));
        insts.push(MInst::Label("T".to_string()));
        let relaxed = relax(function_of(insts));
        let expected = [
            MInst::Cbnz {
                reg,
                target: "Lf_skip10".to_string(),
            },
            MInst::B {
                target: "T".to_string(),
            },
            MInst::Label("Lf_skip10".to_string()),
            MInst::Cbz {
                reg,
                target: "Lf_skip11".to_string(),
            },
            MInst::B {
                target: "T".to_string(),
            },
            MInst::Label("Lf_skip11".to_string()),
        ];
        assert!(relaxed.is_ok_and(|f| f.insts[0..6] == expected && f.next_label == 12));
    }

    /// A reach of three instructions each way, to test the cascade and the `b` limit with tiny programs.
    fn tiny_reach() -> Reach {
        Reach {
            conditional: -12..=12,
            unconditional: -20..=20,
        }
    }

    /// Relaxing one branch moves a label out of reach of another, which is rewritten in the next pass.
    #[test]
    fn growth_pushes_another_branch_out_of_range() {
        let branch = |target: &str| MInst::BCond {
            cond: Cond::Eq,
            target: target.to_string(),
        };
        // `a` reaches `X` (12 bytes) until `b`, which cannot reach `Y`, grows by two instructions.
        let mut insts = vec![branch("X"), branch("Y"), MInst::Brk];
        insts.push(MInst::Label("X".to_string()));
        insts.extend(filler(3));
        insts.push(MInst::Label("Y".to_string()));
        let relaxed = relax_within(function_of(insts), &tiny_reach());
        let skips = relaxed.as_ref().map(|f| f.next_label);
        assert_eq!(skips, Ok(12));
        let first = relaxed.map(|f| f.insts[0].clone());
        let inverse = MInst::BCond {
            cond: Cond::Ne,
            target: "Lf_skip11".to_string(),
        };
        assert_eq!(first, Ok(inverse));
    }

    /// A `b` beyond its reach is an internal compiler error.
    #[test]
    fn unconditional_branch_out_of_range_is_an_internal_error() {
        let mut insts = vec![MInst::B {
            target: "T".to_string(),
        }];
        insts.extend(filler(6));
        insts.push(MInst::Label("T".to_string()));
        let error = relax_within(function_of(insts), &tiny_reach()).err();
        let message = error.map(|e| e.message);
        let expected =
            "internal compiler error: a function is too large for a branch (over 128 MiB)";
        assert_eq!(message.as_deref(), Some(expected));
    }

    /// A branch to a label the function does not have is an internal compiler error.
    #[test]
    fn missing_label_is_an_internal_error() {
        let insts = vec![MInst::B {
            target: "nowhere".to_string(),
        }];
        let message = relax(function_of(insts)).err().map(|e| e.message);
        let expected = "internal compiler error: a branch targets a missing label";
        assert_eq!(message.as_deref(), Some(expected));
    }

    /// Two labels with the same name are an internal compiler error.
    #[test]
    fn duplicate_label_is_an_internal_error() {
        let insts = vec![MInst::Label("T".to_string()), MInst::Label("T".to_string())];
        let message = relax(function_of(insts)).err().map(|e| e.message);
        let expected = "internal compiler error: a label is defined twice";
        assert_eq!(message.as_deref(), Some(expected));
    }
}
