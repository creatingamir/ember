//! Code generation for one function: the frame, instruction selection and the out-of-line panic paths.

use ir::{BinOp, BlockId, Function, Inst, Loc, PrintKind, SlotId, Temp, Terminator};
use source::{Diagnostic, Span};

use crate::asm::Asm;
use crate::runtime;
use crate::strings;

/// How many arguments and parameters travel in registers (`x0` to `x7`).
const MAX_REGISTER_ARGS: usize = 8;
/// The largest frame that `sub sp, sp, #imm` can reserve (a 12-bit immediate).
const MAX_IMMEDIATE_FRAME: usize = 4095;
/// The largest offset that `ldr`/`str` encode as an immediate: 4095 cells of 8 bytes.
const MAX_ACCESS_OFFSET: usize = 4095 * 8;

/// Writes one function as the global symbol `_em_<name>`, or reports the first impossible case found.
pub fn emit_function(asm: &mut Asm, function: &Function) -> Result<(), Diagnostic> {
    let mut emitter = FunctionEmitter::new(asm, function);
    emitter.emit();
    match emitter.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
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
struct PanicStub {
    /// The label the checks branch to.
    label: String,
    /// What the panic message says.
    reason: PanicReason,
    /// The source position the panic message shows.
    loc: Loc,
}

/// The state of the function being written.
struct FunctionEmitter<'a> {
    /// Where the text goes.
    asm: &'a mut Asm,
    /// The function being written.
    function: &'a Function,
    /// The number of temps, found by scanning the function.
    temp_count: usize,
    /// Numbers the local labels (`ok` and `panic`) of this function.
    next_label: u32,
    /// The panic paths still to be written after the body.
    stubs: Vec<PanicStub>,
    /// The first impossible case found; once set, the text is not used.
    error: Option<Diagnostic>,
}

impl<'a> FunctionEmitter<'a> {
    /// Prepares to write `function` into `asm`.
    fn new(asm: &'a mut Asm, function: &'a Function) -> FunctionEmitter<'a> {
        FunctionEmitter {
            asm,
            function,
            temp_count: count_temps(function),
            next_label: 0,
            stubs: Vec::new(),
            error: None,
        }
    }

    /// Writes the whole function: header, prologue, blocks and panic paths.
    fn emit(&mut self) {
        let symbol = format!("_em_{}", self.function.name);
        self.asm.directive(&format!(".globl {symbol}"));
        self.asm.directive(".p2align 2");
        self.asm.label(&symbol);
        self.emit_prologue();
        for block in &self.function.blocks {
            let label = self.block_label(block.id);
            self.asm.label(&label);
            for inst in &block.insts {
                self.emit_inst(inst);
            }
            self.emit_terminator(&block.term);
        }
        self.emit_panic_stubs();
    }

    /// The label of a block; a block the function does not have is an internal error.
    fn block_label(&mut self, block: BlockId) -> String {
        if block.0 as usize >= self.function.blocks.len() {
            self.internal_error("a jump targets a block that does not exist");
        }
        format!("L{}_bb{}", self.function.name, block.0)
    }

    /// A new local label such as `Lfib_ok3`.
    fn new_label(&mut self, kind: &str) -> String {
        let label = format!("L{}_{kind}{}", self.function.name, self.next_label);
        self.next_label += 1;
        label
    }

    /// The number of 8-byte cells in the frame: the slots, then the temps.
    fn cell_count(&self) -> usize {
        self.function.slots.len() + self.temp_count
    }

    /// The frame size in bytes, rounded up to a multiple of 16 so that sp stays aligned.
    fn frame_size(&self) -> usize {
        (self.cell_count() * 8).div_ceil(16) * 16
    }

    /// Saves the caller's registers, reserves the frame and stores the parameters in their slots.
    fn emit_prologue(&mut self) {
        self.asm.prologue();
        let size = self.frame_size();
        if size <= MAX_IMMEDIATE_FRAME {
            self.asm.inst(&format!("sub sp, sp, #{size}"));
        } else {
            // The immediate of `sub` has only 12 bits, so a big size goes through a register.
            self.asm.load_imm("x16", size as u64);
            self.asm.inst("sub sp, sp, x16");
        }
        for (index, slot) in self.function.params.iter().enumerate() {
            if index < MAX_REGISTER_ARGS {
                self.store_slot(&format!("x{index}"), *slot);
            } else {
                self.internal_error("a function has more than 8 parameters");
            }
        }
    }

    /// The cell of a slot, or `None` for a slot the function does not have.
    fn slot_cell(&self, slot: SlotId) -> Option<usize> {
        let cell = slot.0 as usize;
        (cell < self.function.slots.len()).then_some(cell)
    }

    /// The cell of a temp, which comes after all the slots.
    fn temp_cell(&self, temp: Temp) -> Option<usize> {
        let index = temp.0 as usize;
        (index < self.temp_count).then_some(self.function.slots.len() + index)
    }

    /// Emits `ldr` or `str` for a cell at `[sp, #8*cell]`.
    fn access(&mut self, op: &str, reg: &str, cell: Option<usize>) {
        let Some(cell) = cell else {
            self.internal_error("a slot or temp does not exist");
            return;
        };
        let offset = cell * 8;
        if offset <= MAX_ACCESS_OFFSET {
            self.asm.inst(&format!("{op} {reg}, [sp, #{offset}]"));
        } else {
            // The immediate form stops at 32760 bytes, so a far cell gets its offset in a register.
            // x17 is safe: it is set and used by back-to-back instructions, never across a `bl`.
            self.asm.load_imm("x17", offset as u64);
            self.asm.inst(&format!("{op} {reg}, [sp, x17]"));
        }
    }

    /// Loads a temp into `reg`.
    fn load_temp(&mut self, reg: &str, temp: Temp) {
        self.access("ldr", reg, self.temp_cell(temp));
    }

    /// Stores `reg` into a temp.
    fn store_temp(&mut self, reg: &str, temp: Temp) {
        self.access("str", reg, self.temp_cell(temp));
    }

    /// Stores `reg` into a slot.
    fn store_slot(&mut self, reg: &str, slot: SlotId) {
        self.access("str", reg, self.slot_cell(slot));
    }

    /// Loads a slot into `reg`.
    fn load_slot(&mut self, reg: &str, slot: SlotId) {
        self.access("ldr", reg, self.slot_cell(slot));
    }

    /// Records a case that cannot happen for a checked program (decision 0026); only the first one is kept.
    ///
    /// The IR has no source spans, so the diagnostic has an empty one.
    fn internal_error(&mut self, what: &str) {
        if self.error.is_none() {
            let message = format!("internal compiler error: {what}");
            self.error = Some(Diagnostic::error(message, Span::new(0, 0)));
        }
    }

    /// Writes the code of one instruction.
    fn emit_inst(&mut self, inst: &Inst) {
        match inst {
            Inst::Const { dst, value } => {
                // The bit pattern is what matters, so a negative value is cast as is.
                self.asm.load_imm("x8", *value as u64);
                self.store_temp("x8", *dst);
            }
            Inst::Load { dst, slot } => {
                self.load_slot("x8", *slot);
                self.store_temp("x8", *dst);
            }
            Inst::Store { slot, src } => {
                self.load_temp("x8", *src);
                self.store_slot("x8", *slot);
            }
            Inst::Binary {
                dst,
                op,
                lhs,
                rhs,
                loc,
            } => self.emit_binary(*dst, *op, *lhs, *rhs, *loc),
            Inst::Neg { dst, src, loc } => {
                self.load_temp("x8", *src);
                self.asm.inst("negs x8, x8");
                self.branch_to_panic("b.vs", PanicReason::Overflow, *loc);
                self.store_temp("x8", *dst);
            }
            Inst::Not { dst, src } => {
                self.load_temp("x8", *src);
                self.asm.inst("eor x8, x8, #1");
                self.store_temp("x8", *dst);
            }
            Inst::Call { dst, func, args } => self.emit_call(*dst, func, args),
            Inst::Print { src, kind } => {
                self.load_temp("x0", *src);
                match kind {
                    PrintKind::I64 => self.asm.inst(&format!("bl {}", runtime::PRINT_I64)),
                    PrintKind::Bool => self.asm.inst(&format!("bl {}", runtime::PRINT_BOOL)),
                }
            }
        }
    }

    /// Writes a binary operation: both operands go to `x8` and `x9`, the result is stored from `x8` or `x10`.
    fn emit_binary(&mut self, dst: Temp, op: BinOp, lhs: Temp, rhs: Temp, loc: Loc) {
        self.load_temp("x8", lhs);
        self.load_temp("x9", rhs);
        let result = match op {
            BinOp::Add => self.emit_checked("adds x8, x8, x9", loc),
            BinOp::Sub => self.emit_checked("subs x8, x8, x9", loc),
            BinOp::Mul => self.emit_mul(loc),
            BinOp::Div => self.emit_div(loc, false),
            BinOp::Rem => self.emit_div(loc, true),
            BinOp::Eq => self.emit_compare("eq"),
            BinOp::Ne => self.emit_compare("ne"),
            BinOp::Lt => self.emit_compare("lt"),
            BinOp::Le => self.emit_compare("le"),
            BinOp::Gt => self.emit_compare("gt"),
            BinOp::Ge => self.emit_compare("ge"),
        };
        self.store_temp(result, dst);
    }

    /// Writes `adds` or `subs`, which set the V flag on signed overflow. The result is in `x8`.
    fn emit_checked(&mut self, instruction: &str, loc: Loc) -> &'static str {
        self.asm.inst(instruction);
        self.branch_to_panic("b.vs", PanicReason::Overflow, loc);
        "x8"
    }

    /// Writes a checked multiplication. The result is in `x10`.
    fn emit_mul(&mut self, loc: Loc) -> &'static str {
        // The full 128-bit product is `x11:x10`. It fits in 64 bits exactly when
        // the high half is the sign extension of the low half (all zeros or all ones).
        self.asm.inst("mul x10, x8, x9");
        self.asm.inst("smulh x11, x8, x9");
        self.asm.inst("cmp x11, x10, asr #63");
        self.branch_to_panic("b.ne", PanicReason::Overflow, loc);
        "x10"
    }

    /// Writes a checked division, or a remainder when `remainder` is set. The result is in `x10`.
    fn emit_div(&mut self, loc: Loc, remainder: bool) -> &'static str {
        // Zero is checked first, so `i64::MIN / 0` reports "division by zero".
        self.asm.inst("cmp x9, #0");
        self.branch_to_panic("b.eq", PanicReason::DivByZero, loc);
        // `i64::MIN / -1` does not fit in an i64 (and `sdiv` would silently return i64::MIN).
        let ok = self.new_label("ok");
        self.asm.inst("cmn x9, #1");
        self.asm.inst(&format!("b.ne {ok}"));
        self.asm.load_imm("x11", i64::MIN as u64);
        self.asm.inst("cmp x8, x11");
        self.branch_to_panic("b.eq", PanicReason::Overflow, loc);
        self.asm.label(&ok);
        self.asm.inst("sdiv x10, x8, x9");
        if remainder {
            // remainder = dividend - quotient * divisor
            self.asm.inst("msub x10, x10, x9, x8");
        }
        "x10"
    }

    /// Writes a signed comparison that leaves 0 or 1 in `x8`.
    fn emit_compare(&mut self, condition: &str) -> &'static str {
        self.asm.inst("cmp x8, x9");
        self.asm.inst(&format!("cset x8, {condition}"));
        "x8"
    }

    /// Writes a call: arguments into `x0` to `x7`, then `bl`, then the result from `x0`.
    fn emit_call(&mut self, dst: Option<Temp>, func: &str, args: &[Temp]) {
        if args.len() > MAX_REGISTER_ARGS {
            self.internal_error("a call has more than 8 arguments");
            return;
        }
        for (index, arg) in args.iter().enumerate() {
            self.load_temp(&format!("x{index}"), *arg);
        }
        self.asm.inst(&format!("bl _em_{func}"));
        if let Some(dst) = dst {
            self.store_temp("x0", dst);
        }
    }

    /// Writes the end of a block.
    fn emit_terminator(&mut self, term: &Terminator) {
        match term {
            Terminator::Jump(target) => {
                let label = self.block_label(*target);
                self.asm.inst(&format!("b {label}"));
            }
            Terminator::Branch {
                cond,
                then_to,
                else_to,
            } => {
                let then_label = self.block_label(*then_to);
                let else_label = self.block_label(*else_to);
                self.load_temp("x8", *cond);
                self.asm.inst(&format!("cbnz x8, {then_label}"));
                self.asm.inst(&format!("b {else_label}"));
            }
            Terminator::Return(value) => {
                if let Some(temp) = value {
                    self.load_temp("x0", *temp);
                }
                self.asm.epilogue();
            }
            // The IR may contain a block that is never reached; trap if it ever is.
            Terminator::Unreachable => self.asm.inst("brk #1"),
        }
    }

    /// Writes a conditional branch (`b.vs`, `b.ne`, ...) to a new panic path, which is written after the body.
    fn branch_to_panic(&mut self, branch: &str, reason: PanicReason, loc: Loc) {
        let label = self.new_label("panic");
        self.asm.inst(&format!("{branch} {label}"));
        self.stubs.push(PanicStub { label, reason, loc });
    }

    /// Writes every panic path: line, column and reason go to `x0` to `x2`, then the call that never returns.
    fn emit_panic_stubs(&mut self) {
        for stub in std::mem::take(&mut self.stubs) {
            self.asm.label(&stub.label);
            self.asm.load_imm("x0", u64::from(stub.loc.line));
            self.asm.load_imm("x1", u64::from(stub.loc.col));
            self.asm.load_address("x2", stub.reason.label());
            self.asm.inst(&format!("bl {}", runtime::PANIC));
        }
    }
}

/// The number of temps of a function: one more than the highest temp it mentions.
fn count_temps(function: &Function) -> usize {
    let mut count = 0usize;
    for block in &function.blocks {
        let mut temps: Vec<Temp> = block.insts.iter().flat_map(temps_of_inst).collect();
        match &block.term {
            Terminator::Branch { cond, .. } => temps.push(*cond),
            Terminator::Return(Some(temp)) => temps.push(*temp),
            Terminator::Return(None) | Terminator::Jump(_) | Terminator::Unreachable => {}
        }
        for temp in temps {
            count = count.max(temp.0 as usize + 1);
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
