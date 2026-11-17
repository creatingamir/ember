//! Instruction selection: turns an IR function into MIR over virtual registers.

mod arith;

use ir::{BlockId, Function, Inst, IrType, Loc, PrintKind, Sign, SlotId, Temp, Terminator};
use source::Diagnostic;

use crate::mir::{Access, Cond, MFunction, MInst, Mem, Reg, Size, VReg, internal_error};
use crate::runtime;
use crate::strings;

/// How many arguments and parameters travel in registers (`x0` to `x7`); the rest go on the stack.
const MAX_REGISTER_ARGS: usize = 8;

/// Selects MIR for `function`, or reports the first impossible case found.
pub fn select(function: &Function) -> Result<MFunction, Diagnostic> {
    let mut selector = Selector::new(function);
    selector.select_function()?;
    Ok(MFunction {
        name: function.name.clone(),
        slot_count: function.slots.len() as u32,
        vreg_count: selector.next_vreg,
        span: function.span,
        out_arg_bytes: selector.out_arg_bytes,
        next_label: selector.next_label,
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
    /// The end of the largest outgoing stack-argument area of any call so far, in bytes.
    out_arg_bytes: u32,
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
            out_arg_bytes: 0,
            next_label: 0,
            panics: Vec::new(),
        }
    }

    /// Selects the whole function: prologue, parameter stores, blocks and panic paths.
    fn select_function(&mut self) -> Result<(), Diagnostic> {
        self.insts.push(MInst::Prologue);
        // Stack parameters are packed after each other, from offset 0 of the argument area.
        let mut stack_cursor = 0;
        for (index, param) in self.function.params.iter().enumerate() {
            // A parameter must name one of the function's slots.
            if param.slot.0 as usize >= self.function.slots.len() {
                return Err(internal_error("a parameter does not exist"));
            }
            let mem = Mem::Slot(param.slot);
            let access = store_access(param.ty);
            if index < MAX_REGISTER_ARGS {
                self.insts.push(MInst::Store {
                    src: Reg::phys(index as u8),
                    mem,
                    access,
                });
            } else {
                // A stack parameter is copied into its slot through a new virtual register.
                let copy = self.new_vreg()?;
                let offset = next_stack_offset(&mut stack_cursor, param.ty);
                self.insts.push(MInst::Load {
                    dst: copy,
                    mem: Mem::InArg { offset },
                    access: load_access(param.ty, param.sign),
                });
                self.insts.push(MInst::Store {
                    src: copy,
                    mem,
                    access,
                });
            }
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

    /// The type of a temp; a temp without a type is an internal error.
    fn temp_type(&self, temp: Temp) -> Result<IrType, Diagnostic> {
        match self.function.temp_types.get(temp.0 as usize) {
            Some(ty) => Ok(*ty),
            None => Err(internal_error("a temp has no type")),
        }
    }

    /// Selects one IR instruction.
    fn select_inst(&mut self, inst: &Inst) -> Result<(), Diagnostic> {
        match inst {
            Inst::Const { dst, value } => {
                let size = size_of(self.temp_type(*dst)?);
                // The bit pattern is what matters, so a negative value is cast as is.
                self.insts.push(MInst::MovImm {
                    dst: virt(*dst),
                    value: *value as u64,
                    size,
                });
            }
            Inst::Load { dst, slot, sign } => {
                let mem = self.slot(*slot)?;
                let access = load_access(self.temp_type(*dst)?, *sign);
                self.insts.push(MInst::Load {
                    dst: virt(*dst),
                    mem,
                    access,
                });
            }
            Inst::Store { slot, src } => {
                let mem = self.slot(*slot)?;
                let access = store_access(self.temp_type(*src)?);
                self.insts.push(MInst::Store {
                    src: virt(*src),
                    mem,
                    access,
                });
            }
            Inst::Binary {
                dst,
                op,
                lhs,
                rhs,
                loc,
            } => {
                // Both operands have the same type.
                let ty = self.temp_type(*lhs)?;
                let regs = [virt(*dst), virt(*lhs), virt(*rhs)];
                self.select_binary(regs, *op, ty, *loc)?;
            }
            Inst::Neg { dst, src, loc } => {
                let ty = self.temp_type(*src)?;
                self.select_neg(virt(*dst), virt(*src), ty, *loc)?;
            }
            Inst::Not { dst, src } => {
                self.insts.push(MInst::EorOne {
                    dst: virt(*dst),
                    src: virt(*src),
                });
            }
            Inst::Call { dst, func, args } => self.select_call(*dst, func, args)?,
            Inst::Print { src, kind } => self.select_print(*src, *kind)?,
        }
        Ok(())
    }

    /// Selects `print`: the value goes to `x0`, extended to 64 bits when it is narrower, then the helper is called.
    fn select_print(&mut self, src: Temp, kind: PrintKind) -> Result<(), Diagnostic> {
        let ty = self.temp_type(src)?;
        self.insts.push(MInst::Mov {
            dst: Reg::phys(0),
            src: virt(src),
        });
        // A value of 32 bits or fewer has undefined upper bits in its register.
        let narrow = ty != IrType::I64;
        let x0 = Reg::phys(0);
        let helper = match kind {
            PrintKind::I64 => {
                if narrow {
                    self.insts.push(MInst::Sxtw { dst: x0, src: x0 });
                }
                runtime::PRINT_I64
            }
            PrintKind::U64 if !narrow => runtime::PRINT_U64,
            PrintKind::U64 => {
                self.insts.push(MInst::Uxtw { dst: x0, src: x0 });
                runtime::PRINT_I64
            }
            PrintKind::Bool => runtime::PRINT_BOOL,
        };
        self.insts.push(MInst::Bl {
            symbol: helper.to_string(),
            args: 1,
        });
        Ok(())
    }

    /// Selects a call: arguments into `x0` to `x7`, then `bl`, then the result from `x0`.
    fn select_call(
        &mut self,
        dst: Option<Temp>,
        func: &str,
        args: &[Temp],
    ) -> Result<(), Diagnostic> {
        // The stack arguments go first, so that `x0` to `x7` are set last and nothing can clobber them.
        // They are packed after each other, each aligned to its own size (Apple's rule).
        let mut stack_cursor = 0;
        for arg in args.iter().skip(MAX_REGISTER_ARGS) {
            let ty = self.temp_type(*arg)?;
            let offset = next_stack_offset(&mut stack_cursor, ty);
            self.insts.push(MInst::Store {
                src: virt(*arg),
                mem: Mem::OutArg { offset },
                access: store_access(ty),
            });
        }
        self.out_arg_bytes = self.out_arg_bytes.max(stack_cursor);
        for (index, arg) in args.iter().enumerate().take(MAX_REGISTER_ARGS) {
            self.insts.push(MInst::Mov {
                dst: Reg::phys(index as u8),
                src: virt(*arg),
            });
        }
        self.insts.push(MInst::Bl {
            symbol: format!("_em_{func}"),
            args: args.len().min(MAX_REGISTER_ARGS) as u8,
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
                size: Size::X,
            });
            self.insts.push(MInst::MovImm {
                dst: Reg::phys(1),
                value: u64::from(path.loc.col),
                size: Size::X,
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

/// The register size an operation on values of this type uses: `w` for 8 to 32 bits, `x` for 64.
fn size_of(ty: IrType) -> Size {
    match ty {
        IrType::I64 => Size::X,
        IrType::I8 | IrType::I16 | IrType::I32 => Size::W,
    }
}

/// How a load of a value of this type moves memory; 8- and 16-bit values extend as `sign` says.
fn load_access(ty: IrType, sign: Sign) -> Access {
    let signed = sign == Sign::Signed;
    match ty {
        IrType::I8 => Access::Byte { signed },
        IrType::I16 => Access::Half { signed },
        IrType::I32 => Access::Word,
        IrType::I64 => Access::Double,
    }
}

/// How a store of a value of this type moves memory.
fn store_access(ty: IrType) -> Access {
    match ty {
        IrType::I8 => Access::Byte { signed: false },
        IrType::I16 => Access::Half { signed: false },
        IrType::I32 => Access::Word,
        IrType::I64 => Access::Double,
    }
}

/// The offset of the next stack argument of type `ty`: aligned to its size. Moves `cursor` past it.
fn next_stack_offset(cursor: &mut u32, ty: IrType) -> u32 {
    let bytes = ty.bytes();
    // A saturated offset only happens for an absurd argument count, which the frame limit rejects.
    let offset = cursor.checked_next_multiple_of(bytes).unwrap_or(u32::MAX);
    *cursor = offset.saturating_add(bytes);
    offset
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
