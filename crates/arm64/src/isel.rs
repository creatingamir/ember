//! Instruction selection: turns an IR function into MIR over virtual registers.

use ir::{BinOp, BlockId, Function, Inst, Loc, PrintKind, SlotId, Temp, Terminator};
use source::Diagnostic;

use crate::mir::{Cond, MFunction, MInst, Mem, Reg, VReg, internal_error};
use crate::runtime;
use crate::strings;

/// How many arguments and parameters travel in registers (`x0` to `x7`).
const MAX_REGISTER_ARGS: usize = 8;

/// Selects MIR for `function`, or reports the first impossible case found.
pub fn select(function: &Function) -> Result<MFunction, Diagnostic> {
    let mut selector = Selector::new(function);
    selector.select_function()?;
    Ok(MFunction {
        name: function.name.clone(),
        slot_count: function.slots.len() as u32,
        vreg_count: selector.next_vreg,
        insts: selector.insts,
        frame_size: 0,
    })
}

/// Why a check can fail; it picks the reason string of the panic.
#[derive(Clone, Copy)]
enum PanicReason {
    /// A result outside the `i64` range.
    Overflow,
    /// A zero divisor.
    DivByZero,
}

impl PanicReason {
    /// The label of the reason's string constant.
    fn label(self) -> &'static str {
        match self {
            PanicReason::Overflow => strings::OVERFLOW,
            PanicReason::DivByZero => strings::DIV_BY_ZERO,
        }
    }
}

/// A panic path written after the function body: its label, reason and source position.
struct PanicPath {
    /// The label the checks branch to.
    label: String,
    /// What the panic message says.
    reason: PanicReason,
    /// The source position the panic message shows.
    loc: Loc,
}

/// The state of the function being selected.
struct Selector<'a> {
    /// The function being selected.
    function: &'a Function,
    /// The MIR written so far.
    insts: Vec<MInst>,
    /// The number of the next virtual register to hand out; starts after the IR temps.
    next_vreg: u32,
    /// Numbers the local labels (`ok` and `panic`) of this function.
    next_label: u32,
    /// The panic paths still to be written after the body.
    panics: Vec<PanicPath>,
}

impl<'a> Selector<'a> {
    /// Prepares to select `function`; IR temp `tN` is `VReg(N)`.
    fn new(function: &'a Function) -> Selector<'a> {
        Selector {
            function,
            insts: Vec::new(),
            next_vreg: count_temps(function),
            next_label: 0,
            panics: Vec::new(),
        }
    }

    /// Selects the whole function: prologue, parameter stores, blocks and panic paths.
    fn select_function(&mut self) -> Result<(), Diagnostic> {
        self.insts.push(MInst::Prologue);
        for (index, slot) in self.function.params.iter().enumerate() {
            if index >= MAX_REGISTER_ARGS {
                return Err(internal_error("a function has more than 8 parameters"));
            }
            let mem = self.slot(*slot)?;
            self.insts.push(MInst::Store {
                src: Reg::phys(index as u8),
                mem,
            });
        }
        let function = self.function;
        for block in &function.blocks {
            let label = self.block_label(block.id)?;
            self.insts.push(MInst::Label(label));
            for inst in &block.insts {
                self.select_inst(inst)?;
            }
            self.select_terminator(&block.term)?;
        }
        self.select_panic_paths();
        Ok(())
    }

    /// The memory operand of a slot; a slot the function does not have is an internal error.
    fn slot(&self, slot: SlotId) -> Result<Mem, Diagnostic> {
        if slot.0 as usize >= self.function.slots.len() {
            return Err(internal_error("a slot or temp does not exist"));
        }
        Ok(Mem::Slot(slot))
    }

    /// The label of a block; a block the function does not have is an internal error.
    fn block_label(&self, block: BlockId) -> Result<String, Diagnostic> {
        if block.0 as usize >= self.function.blocks.len() {
            return Err(internal_error("a jump targets a block that does not exist"));
        }
        Ok(format!("L{}_bb{}", self.function.name, block.0))
    }

    /// A new local label such as `Lfib_ok3`.
    fn new_label(&mut self, kind: &str) -> String {
        let label = format!("L{}_{kind}{}", self.function.name, self.next_label);
        self.next_label += 1;
        label
    }

    /// A new virtual register that no IR temp uses; running out of numbers is an internal error.
    fn new_vreg(&mut self) -> Result<Reg, Diagnostic> {
        let Some(next) = self.next_vreg.checked_add(1) else {
            return Err(internal_error("a function has too many virtual registers"));
        };
        let reg = Reg::Virt(VReg(self.next_vreg));
        self.next_vreg = next;
        Ok(reg)
    }

    /// Selects one IR instruction.
    fn select_inst(&mut self, inst: &Inst) -> Result<(), Diagnostic> {
        match inst {
            Inst::Const { dst, value } => {
                // The bit pattern is what matters, so a negative value is cast as is.
                self.insts.push(MInst::MovImm {
                    dst: virt(*dst),
                    value: *value as u64,
                });
            }
            Inst::Load { dst, slot } => {
                let mem = self.slot(*slot)?;
                self.insts.push(MInst::Load {
                    dst: virt(*dst),
                    mem,
                });
            }
            Inst::Store { slot, src } => {
                let mem = self.slot(*slot)?;
                self.insts.push(MInst::Store {
                    src: virt(*src),
                    mem,
                });
            }
            Inst::Binary {
                dst,
                op,
                lhs,
                rhs,
                loc,
            } => self.select_binary(virt(*dst), *op, virt(*lhs), virt(*rhs), *loc)?,
            Inst::Neg { dst, src, loc } => {
                self.insts.push(MInst::Negs {
                    dst: virt(*dst),
                    src: virt(*src),
                });
                self.branch_to_panic(Cond::Vs, PanicReason::Overflow, *loc);
            }
            Inst::Not { dst, src } => {
                self.insts.push(MInst::EorOne {
                    dst: virt(*dst),
                    src: virt(*src),
                });
            }
            Inst::Call { dst, func, args } => self.select_call(*dst, func, args)?,
            Inst::Print { src, kind } => {
                let helper = match kind {
                    PrintKind::I64 => runtime::PRINT_I64,
                    PrintKind::Bool => runtime::PRINT_BOOL,
                };
                self.insts.push(MInst::Mov {
                    dst: Reg::phys(0),
                    src: virt(*src),
                });
                self.insts.push(MInst::Bl {
                    symbol: helper.to_string(),
                    args: 1,
                });
            }
        }
        Ok(())
    }

    /// Selects a binary operation.
    fn select_binary(
        &mut self,
        dst: Reg,
        op: BinOp,
        lhs: Reg,
        rhs: Reg,
        loc: Loc,
    ) -> Result<(), Diagnostic> {
        match op {
            BinOp::Add => {
                self.insts.push(MInst::Adds { dst, lhs, rhs });
                self.branch_to_panic(Cond::Vs, PanicReason::Overflow, loc);
            }
            BinOp::Sub => {
                self.insts.push(MInst::Subs { dst, lhs, rhs });
                self.branch_to_panic(Cond::Vs, PanicReason::Overflow, loc);
            }
            BinOp::Mul => self.select_mul(dst, lhs, rhs, loc)?,
            BinOp::Div => self.select_div(dst, lhs, rhs, loc, false)?,
            BinOp::Rem => self.select_div(dst, lhs, rhs, loc, true)?,
            BinOp::Eq => self.select_compare(dst, lhs, rhs, Cond::Eq),
            BinOp::Ne => self.select_compare(dst, lhs, rhs, Cond::Ne),
            BinOp::Lt => self.select_compare(dst, lhs, rhs, Cond::Lt),
            BinOp::Le => self.select_compare(dst, lhs, rhs, Cond::Le),
            BinOp::Gt => self.select_compare(dst, lhs, rhs, Cond::Gt),
            BinOp::Ge => self.select_compare(dst, lhs, rhs, Cond::Ge),
        }
        Ok(())
    }

    /// Selects a checked multiplication.
    fn select_mul(&mut self, dst: Reg, lhs: Reg, rhs: Reg, loc: Loc) -> Result<(), Diagnostic> {
        let high = self.new_vreg()?;
        // The full 128-bit product is `high:dst`. It fits in 64 bits exactly when
        // the high half is the sign extension of the low half (all zeros or all ones).
        self.insts.push(MInst::Mul { dst, lhs, rhs });
        self.insts.push(MInst::Smulh {
            dst: high,
            lhs,
            rhs,
        });
        self.insts.push(MInst::CmpAsr63 {
            lhs: high,
            rhs: dst,
        });
        self.branch_to_panic(Cond::Ne, PanicReason::Overflow, loc);
        Ok(())
    }

    /// Selects a checked division, or a remainder when `remainder` is set.
    fn select_div(
        &mut self,
        dst: Reg,
        lhs: Reg,
        rhs: Reg,
        loc: Loc,
        remainder: bool,
    ) -> Result<(), Diagnostic> {
        // Zero is checked first, so `i64::MIN / 0` reports "division by zero".
        self.insts.push(MInst::CmpImm { lhs: rhs, imm: 0 });
        self.branch_to_panic(Cond::Eq, PanicReason::DivByZero, loc);
        // `i64::MIN / -1` does not fit in an i64 (and `sdiv` would silently return i64::MIN).
        let ok = self.new_label("ok");
        self.insts.push(MInst::CmnImm { lhs: rhs, imm: 1 });
        self.insts.push(MInst::BCond {
            cond: Cond::Ne,
            target: ok.clone(),
        });
        let min = self.new_vreg()?;
        self.insts.push(MInst::MovImm {
            dst: min,
            value: i64::MIN as u64,
        });
        self.insts.push(MInst::Cmp { lhs, rhs: min });
        self.branch_to_panic(Cond::Eq, PanicReason::Overflow, loc);
        self.insts.push(MInst::Label(ok));
        if remainder {
            // remainder = dividend - quotient * divisor
            let quotient = self.new_vreg()?;
            self.insts.push(MInst::Sdiv {
                dst: quotient,
                lhs,
                rhs,
            });
            self.insts.push(MInst::Msub {
                dst,
                mul_lhs: quotient,
                mul_rhs: rhs,
                minuend: lhs,
            });
        } else {
            self.insts.push(MInst::Sdiv { dst, lhs, rhs });
        }
        Ok(())
    }

    /// Selects a signed comparison that leaves 0 or 1 in `dst`.
    fn select_compare(&mut self, dst: Reg, lhs: Reg, rhs: Reg, cond: Cond) {
        self.insts.push(MInst::Cmp { lhs, rhs });
        self.insts.push(MInst::Cset { dst, cond });
    }

    /// Selects a call: arguments into `x0` to `x7`, then `bl`, then the result from `x0`.
    fn select_call(
        &mut self,
        dst: Option<Temp>,
        func: &str,
        args: &[Temp],
    ) -> Result<(), Diagnostic> {
        if args.len() > MAX_REGISTER_ARGS {
            return Err(internal_error("a call has more than 8 arguments"));
        }
        for (index, arg) in args.iter().enumerate() {
            self.insts.push(MInst::Mov {
                dst: Reg::phys(index as u8),
                src: virt(*arg),
            });
        }
        self.insts.push(MInst::Bl {
            symbol: format!("_em_{func}"),
            args: args.len() as u8,
        });
        if let Some(dst) = dst {
            self.insts.push(MInst::Mov {
                dst: virt(dst),
                src: Reg::phys(0),
            });
        }
        Ok(())
    }

    /// Selects the end of a block.
    fn select_terminator(&mut self, term: &Terminator) -> Result<(), Diagnostic> {
        match term {
            Terminator::Jump(target) => {
                let target = self.block_label(*target)?;
                self.insts.push(MInst::B { target });
            }
            Terminator::Branch {
                cond,
                then_to,
                else_to,
            } => {
                let then_label = self.block_label(*then_to)?;
                let else_label = self.block_label(*else_to)?;
                self.insts.push(MInst::Cbnz {
                    reg: virt(*cond),
                    target: then_label,
                });
                self.insts.push(MInst::B { target: else_label });
            }
            Terminator::Return(value) => {
                if let Some(value) = value {
                    self.insts.push(MInst::Mov {
                        dst: Reg::phys(0),
                        src: virt(*value),
                    });
                }
                self.insts.push(MInst::Epilogue);
            }
            // The IR may contain a block that is never reached; trap if it ever is.
            Terminator::Unreachable => self.insts.push(MInst::Brk),
        }
        Ok(())
    }

    /// Selects a conditional branch to a new panic path, which is written after the body.
    fn branch_to_panic(&mut self, cond: Cond, reason: PanicReason, loc: Loc) {
        let label = self.new_label("panic");
        self.insts.push(MInst::PanicIf {
            cond,
            target: label.clone(),
        });
        self.panics.push(PanicPath { label, reason, loc });
    }

    /// Selects every panic path: line, column and reason go to `x0` to `x2`, then the call that never returns.
    fn select_panic_paths(&mut self) {
        for path in std::mem::take(&mut self.panics) {
            self.insts.push(MInst::Label(path.label));
            self.insts.push(MInst::MovImm {
                dst: Reg::phys(0),
                value: u64::from(path.loc.line),
            });
            self.insts.push(MInst::MovImm {
                dst: Reg::phys(1),
                value: u64::from(path.loc.col),
            });
            self.insts.push(MInst::LoadAddress {
                dst: Reg::phys(2),
                label: path.reason.label().to_string(),
            });
            self.insts.push(MInst::Bl {
                symbol: runtime::PANIC.to_string(),
                args: 3,
            });
        }
    }
}

/// The virtual register of an IR temp.
fn virt(temp: Temp) -> Reg {
    Reg::Virt(VReg(temp.0))
}

/// The number of temps of a function: one more than the highest temp it mentions.
fn count_temps(function: &Function) -> u32 {
    let mut count = 0;
    for block in &function.blocks {
        let mut temps: Vec<Temp> = block.insts.iter().flat_map(temps_of_inst).collect();
        match &block.term {
            Terminator::Branch { cond, .. } => temps.push(*cond),
            Terminator::Return(Some(temp)) => temps.push(*temp),
            Terminator::Return(None) | Terminator::Jump(_) | Terminator::Unreachable => {}
        }
        for temp in temps {
            count = count.max(temp.0.saturating_add(1));
        }
    }
    count
}

/// Every temp an instruction reads or writes.
fn temps_of_inst(inst: &Inst) -> Vec<Temp> {
    match inst {
        Inst::Const { dst, .. } | Inst::Load { dst, .. } => vec![*dst],
        Inst::Store { src, .. } | Inst::Print { src, .. } => vec![*src],
        Inst::Binary { dst, lhs, rhs, .. } => vec![*dst, *lhs, *rhs],
        Inst::Neg { dst, src, .. } | Inst::Not { dst, src } => vec![*dst, *src],
        Inst::Call { dst, args, .. } => {
            let mut temps = args.clone();
            if let Some(dst) = dst {
                temps.push(*dst);
            }
            temps
        }
    }
}
