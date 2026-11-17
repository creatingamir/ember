//! Lowering of a checked program into IR.
//!
//! The checker has accepted the program, so lowering should never fail. Where a lookup in the
//! checker's tables misses anyway, lowering records an "internal compiler error" and carries on
//! with a harmless placeholder; `lower` then returns the first such error instead of the module.

use std::collections::HashMap;

use ast::{
    AssignOp, BinaryOp, Block as AstBlock, ElseBranch, Expr, ExprKind, ForKind, Ident, NodeId,
    Stmt, StmtKind, UnaryOp,
};
use check::{Binding, Checked, TypeId, TypeKind};
use source::{Diagnostic, SourceFile, Span};

use crate::blocks::remove_unreachable;
use crate::{
    BinOp, Block, BlockId, Function, Inst, IrType, Loc, Module, Param, PrintKind, Sign, Slot,
    SlotId, Temp, Terminator,
};

/// The internal error for a value whose type the checker did not record, or that has no value.
const MISSING_TYPE: &str = "an expression has no type";

/// The name of the hidden slot that holds the value of an `&&`.
const AND_SLOT: &str = "$and";

/// The name of the hidden slot that holds the value of an `||`.
const OR_SLOT: &str = "$or";

/// The name of the hidden slot that holds the end of a `for` range.
const END_SLOT: &str = "$end";

/// Lowers every function of a checked program, or returns the first internal compiler error.
pub fn lower(
    program: &ast::Program,
    checked: &Checked,
    file: &SourceFile,
) -> Result<Module, Diagnostic> {
    let mut functions = Vec::new();
    for function in &program.functions {
        let lowering = FunctionLowering::new(checked, file, function.name.span);
        functions.push(lowering.lower_function(function)?);
    }
    Ok(Module { functions })
}

/// The IR type of a value and how its bits are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValueType {
    /// The width.
    ty: IrType,
    /// Signed for signed integers, unsigned for unsigned integers and `bool`.
    sign: Sign,
}

impl ValueType {
    /// The type of a `bool`: an `i8` holding 0 or 1.
    const BOOL: ValueType = ValueType {
        ty: IrType::I8,
        sign: Sign::Unsigned,
    };

    /// The type that stands in for an unknown one after an internal error has been recorded.
    const PLACEHOLDER: ValueType = ValueType {
        ty: IrType::I64,
        sign: Sign::Signed,
    };
}

/// A block that is still being filled in; its terminator is set when the block ends.
struct BlockBuilder {
    /// The instructions added so far.
    insts: Vec<Inst>,
    /// How the block ends, once known.
    term: Option<Terminator>,
}

/// Where `break` and `continue` jump to inside one loop.
#[derive(Debug, Clone, Copy)]
struct LoopTargets {
    /// The block after the loop.
    break_to: BlockId,
    /// The block that starts the next round.
    continue_to: BlockId,
}

/// The two statements that jump out of the current round of a loop.
#[derive(Debug, Clone, Copy)]
enum LoopJump {
    /// `break`
    Break,
    /// `continue`
    Continue,
}

/// The parts of a `for i in a..b` loop.
struct RangeLoop<'a> {
    /// The loop variable.
    var: &'a Ident,
    /// The first value.
    start: &'a Expr,
    /// The end of the range.
    end: &'a Expr,
    /// True for `..=`, where the end value is included.
    inclusive: bool,
}

/// The state while lowering one function.
struct FunctionLowering<'a> {
    /// What the checker learned about the program.
    checked: &'a Checked,
    /// The source file, used to turn offsets into lines and columns.
    file: &'a SourceFile,
    /// The name of the function, where errors that have no better place are reported.
    span: Span,
    /// The first internal compiler error found, if any.
    error: Option<Diagnostic>,
    /// The slots created so far.
    slots: Vec<Slot>,
    /// The type of the value in each slot: `slot_types[k]` belongs to `slots[k]`.
    slot_types: Vec<ValueType>,
    /// The slot of every parameter, `let` and loop variable, by its declaring node id.
    slot_of: HashMap<NodeId, SlotId>,
    /// The blocks created so far; the block at index `i` has id `BlockId(i)`.
    blocks: Vec<BlockBuilder>,
    /// The block that instructions are added to.
    current: BlockId,
    /// The type of every temp created so far: `temp_types[n]` is the type of `tn`.
    temp_types: Vec<IrType>,
    /// The loops that enclose the statement being lowered, innermost last.
    loops: Vec<LoopTargets>,
}

impl<'a> FunctionLowering<'a> {
    /// Creates a lowering that has the entry block and nothing else.
    fn new(checked: &'a Checked, file: &'a SourceFile, span: Span) -> FunctionLowering<'a> {
        let mut lowering = FunctionLowering {
            checked,
            file,
            span,
            error: None,
            slots: Vec::new(),
            slot_types: Vec::new(),
            slot_of: HashMap::new(),
            blocks: Vec::new(),
            current: BlockId(0),
            temp_types: Vec::new(),
            loops: Vec::new(),
        };
        lowering.new_block();
        lowering
    }

    /// Lowers one function, then removes the blocks that nothing jumps to.
    fn lower_function(mut self, function: &ast::Function) -> Result<Function, Diagnostic> {
        let ret = self.function_return(function.name.span, &function.name.name);

        let mut params = Vec::new();
        for param in &function.params {
            let value_type = self.local_type(param.id);
            let slot = self.declare_slot(&param.name.name, param.id, value_type);
            params.push(Param {
                slot,
                ty: value_type.ty,
                sign: value_type.sign,
            });
        }

        self.lower_block(&function.body);

        // The checker guarantees that a function with a value never falls off its end.
        let end = if ret.is_some() {
            Terminator::Unreachable
        } else {
            Terminator::Return(None)
        };
        self.set_terminator(end);

        let builders = std::mem::take(&mut self.blocks);
        let finished = self.finish_blocks(builders);
        let blocks = match remove_unreachable(finished) {
            Ok(blocks) => blocks,
            Err(what) => {
                self.internal_error(self.span, what);
                Vec::new()
            }
        };
        if blocks
            .iter()
            .any(|block| block.term == Terminator::Unreachable)
        {
            self.internal_error(self.span, "a value-returning function can reach its end");
        }

        match self.error {
            Some(error) => Err(error),
            None => Ok(Function {
                name: function.name.name.clone(),
                params,
                ret: ret.map(|value_type| value_type.ty),
                slots: self.slots,
                temp_types: self.temp_types,
                blocks,
                span: function.name.span,
            }),
        }
    }

    /// Records an internal compiler error at a span; only the first one is kept.
    fn internal_error(&mut self, span: Span, what: &str) {
        if self.error.is_none() {
            let message = format!("internal compiler error: {what}");
            self.error = Some(Diagnostic::error(message, span));
        }
    }

    /// Records that the checker did not record a type; this error has no better place than span 0..0.
    fn missing_type_error(&mut self) {
        self.internal_error(Span::new(0, 0), MISSING_TYPE);
    }

    /// The IR type of a checker type, or the internal error for `()`, the checker-only untyped
    /// integer and a width the IR does not have.
    fn value_type_of(&self, id: TypeId) -> Result<ValueType, &'static str> {
        match self.checked.types.kind(id) {
            TypeKind::Bool => Ok(ValueType::BOOL),
            TypeKind::Int { bits, signed } => {
                let ty = match bits {
                    8 => IrType::I8,
                    16 => IrType::I16,
                    32 => IrType::I32,
                    64 => IrType::I64,
                    _ => return Err("a type has no IR width"),
                };
                let sign = if signed { Sign::Signed } else { Sign::Unsigned };
                Ok(ValueType { ty, sign })
            }
            TypeKind::Unit | TypeKind::UntypedInt => Err(MISSING_TYPE),
        }
    }

    /// The type of a value from its checker type; a missing or valueless type is an internal error.
    fn value_type(&mut self, id: Option<TypeId>) -> ValueType {
        let Some(id) = id else {
            self.missing_type_error();
            return ValueType::PLACEHOLDER;
        };
        match self.value_type_of(id) {
            Ok(value_type) => value_type,
            Err(what) => {
                self.internal_error(Span::new(0, 0), what);
                ValueType::PLACEHOLDER
            }
        }
    }

    /// The type of an expression, as the checker recorded it.
    fn expr_type(&mut self, expr: &Expr) -> ValueType {
        let id = self.checked.expr_types.get(&expr.id).copied();
        self.value_type(id)
    }

    /// The type of a parameter, `let` or loop variable, by its declaring node.
    fn local_type(&mut self, node: NodeId) -> ValueType {
        let id = self.checked.local_types.get(&node).copied();
        self.value_type(id)
    }

    /// The result type of a user function, or `None` when it returns nothing.
    fn function_return(&mut self, span: Span, name: &str) -> Option<ValueType> {
        let Some(signature) = self.checked.signatures.get(name) else {
            self.internal_error(span, &format!("the function `{name}` has no signature"));
            return None;
        };
        if signature.ret == TypeId::UNIT {
            return None;
        }
        let id = Some(signature.ret);
        Some(self.value_type(id))
    }

    /// Converts a count to `u32`; a count that does not fit is an internal error.
    fn count_as_u32(&mut self, number: usize) -> u32 {
        match u32::try_from(number) {
            Ok(number) => number,
            Err(_) => {
                self.internal_error(self.span, "the function is too large");
                u32::MAX
            }
        }
    }

    /// Turns the finished builders into blocks; a block without a terminator is an internal error.
    fn finish_blocks(&mut self, builders: Vec<BlockBuilder>) -> Vec<Block> {
        let mut blocks = Vec::new();
        for (index, builder) in builders.into_iter().enumerate() {
            let id = BlockId(self.count_as_u32(index));
            let term = match builder.term {
                Some(term) => term,
                None => {
                    self.internal_error(self.span, "a block has no terminator");
                    Terminator::Unreachable
                }
            };
            blocks.push(Block {
                id,
                insts: builder.insts,
                term,
            });
        }
        blocks
    }

    /// Creates a block and returns its id; it stays empty until it is made current.
    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.count_as_u32(self.blocks.len()));
        self.blocks.push(BlockBuilder {
            insts: Vec::new(),
            term: None,
        });
        id
    }

    /// Makes a block the one that instructions are added to.
    fn switch_to(&mut self, block: BlockId) {
        self.current = block;
    }

    /// Ends the current block with a terminator.
    fn set_terminator(&mut self, term: Terminator) {
        match self.blocks.get_mut(self.current.0 as usize) {
            Some(block) => block.term = Some(term),
            None => self.internal_error(self.span, "the current block does not exist"),
        }
    }

    /// Ends the current block and moves on to a fresh block that nothing jumps to.
    ///
    /// Statements after a `return`, `break` or `continue` land there and are removed later,
    /// which keeps every real block free of instructions after its terminator.
    fn terminate(&mut self, term: Terminator) {
        self.set_terminator(term);
        let unreachable_block = self.new_block();
        self.switch_to(unreachable_block);
    }

    /// Adds an instruction to the current block.
    fn emit(&mut self, inst: Inst) {
        match self.blocks.get_mut(self.current.0 as usize) {
            Some(block) => block.insts.push(inst),
            None => self.internal_error(self.span, "the current block does not exist"),
        }
    }

    /// Returns a temp of type `ty` that has not been used yet.
    fn new_temp(&mut self, ty: IrType) -> Temp {
        let temp = Temp(self.count_as_u32(self.temp_types.len()));
        self.temp_types.push(ty);
        temp
    }

    /// Creates a slot with the given name, sized for a value of type `value_type`.
    fn new_slot(&mut self, name: &str, value_type: ValueType) -> SlotId {
        let id = SlotId(self.count_as_u32(self.slots.len()));
        self.slots.push(Slot {
            name: name.to_string(),
            size: value_type.ty.bytes(),
            align: value_type.ty.bytes(),
        });
        self.slot_types.push(value_type);
        id
    }

    /// Creates the slot of a parameter, `let` or loop variable and remembers it by node id.
    fn declare_slot(&mut self, name: &str, node: NodeId, value_type: ValueType) -> SlotId {
        let slot = self.new_slot(name, value_type);
        self.slot_of.insert(node, slot);
        slot
    }

    /// The type of the value in a slot.
    fn slot_type(&mut self, slot: SlotId) -> ValueType {
        match self.slot_types.get(slot.0 as usize) {
            Some(value_type) => *value_type,
            None => {
                self.internal_error(self.span, "a slot does not exist");
                ValueType::PLACEHOLDER
            }
        }
    }

    /// Finds the slot of the local that a name expression or an assignment statement refers to.
    fn local_slot(&self, node_id: NodeId) -> Option<SlotId> {
        match self.checked.bindings.get(&node_id) {
            Some(Binding::Local(declaration)) => self.slot_of.get(declaration).copied(),
            _ => None,
        }
    }

    /// Adds `dst = const value` with `dst` of type `ty`, and returns `dst`.
    fn emit_const(&mut self, value: i64, ty: IrType) -> Temp {
        let dst = self.new_temp(ty);
        self.emit(Inst::Const { dst, value });
        dst
    }

    /// Adds `dst = load slot`, with `dst` typed like the slot's value, and returns `dst`.
    fn emit_load(&mut self, slot: SlotId) -> Temp {
        let value_type = self.slot_type(slot);
        let dst = self.new_temp(value_type.ty);
        self.emit(Inst::Load {
            dst,
            slot,
            sign: value_type.sign,
        });
        dst
    }

    /// Adds `store slot, src`.
    fn emit_store(&mut self, slot: SlotId, src: Temp) {
        self.emit(Inst::Store { slot, src });
    }

    /// Adds `dst = lhs op rhs` with `dst` of type `ty`, and returns `dst`.
    fn emit_binary(&mut self, op: BinOp, operands: [Temp; 2], loc: Loc, ty: IrType) -> Temp {
        let [lhs, rhs] = operands;
        let dst = self.new_temp(ty);
        self.emit(Inst::Binary {
            dst,
            op,
            lhs,
            rhs,
            loc,
        });
        dst
    }

    /// Turns the start of a span into a line and column.
    fn loc(&mut self, span: Span) -> Loc {
        let (line, col) = self.file.line_col(span.start);
        Loc {
            line: self.count_as_u32(line),
            col: self.count_as_u32(col),
        }
    }

    /// Lowers the statements of a block in order.
    fn lower_block(&mut self, block: &AstBlock) {
        for stmt in &block.stmts {
            self.lower_stmt(stmt);
        }
    }

    /// Lowers one statement.
    fn lower_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => self.lower_let(stmt, name, value),
            StmtKind::Assign { op, value, .. } => self.lower_assign(stmt, *op, value),
            StmtKind::If {
                cond,
                then_block,
                else_branch,
            } => self.lower_if(cond, then_block, else_branch.as_ref()),
            StmtKind::For { kind, body } => self.lower_for(stmt, kind, body),
            StmtKind::Return(value) => self.lower_return(value.as_ref()),
            StmtKind::Break => self.lower_loop_jump(stmt.span, LoopJump::Break),
            StmtKind::Continue => self.lower_loop_jump(stmt.span, LoopJump::Continue),
            StmtKind::Expr(expr) => self.lower_expr_stmt(expr),
        }
    }

    /// Lowers `x := e`; the slot is created after the value, so hidden slots of the value come first.
    fn lower_let(&mut self, stmt: &Stmt, name: &Ident, value: &Expr) {
        let src = self.lower_expr(value);
        let value_type = self.local_type(stmt.id);
        let slot = self.declare_slot(&name.name, stmt.id, value_type);
        self.emit_store(slot, src);
    }

    /// Lowers `x = e` and the compound forms such as `x += e`.
    fn lower_assign(&mut self, stmt: &Stmt, op: AssignOp, value: &Expr) {
        let Some(slot) = self.local_slot(stmt.id) else {
            self.internal_error(stmt.span, "an assignment target is not bound to a variable");
            self.lower_expr(value);
            return;
        };

        if op == AssignOp::MatMul {
            // The placeholder below treats `@=` as a plain `=`.
            self.internal_error(stmt.span, "the `@=` assignment is not supported");
        }

        let target = self.slot_type(slot);
        match compound_op(op, target.sign) {
            Some(bin_op) => {
                let old = self.emit_load(slot);
                let operand = self.lower_expr(value);
                let loc = self.loc(stmt.span);
                let new = self.emit_binary(bin_op, [old, operand], loc, target.ty);
                self.emit_store(slot, new);
            }
            None => {
                let src = self.lower_expr(value);
                self.emit_store(slot, src);
            }
        }
    }

    /// Lowers an `if` with an optional `else`; `else if` is an else block holding the nested `if`.
    fn lower_if(&mut self, cond: &Expr, then_block: &AstBlock, else_branch: Option<&ElseBranch>) {
        let then_id = self.new_block();
        // Without an `else`, a false condition goes straight to the join block.
        let (else_id, join) = match else_branch {
            Some(_) => (self.new_block(), self.new_block()),
            None => {
                let join = self.new_block();
                (join, join)
            }
        };

        let cond = self.lower_expr(cond);
        self.set_terminator(Terminator::Branch {
            cond,
            then_to: then_id,
            else_to: else_id,
        });

        self.switch_to(then_id);
        self.lower_block(then_block);
        self.set_terminator(Terminator::Jump(join));

        if let Some(else_branch) = else_branch {
            self.switch_to(else_id);
            match else_branch {
                ElseBranch::Block(block) => self.lower_block(block),
                ElseBranch::If(nested) => self.lower_stmt(nested),
            }
            self.set_terminator(Terminator::Jump(join));
        }

        self.switch_to(join);
    }

    /// Lowers the three forms of `for`.
    fn lower_for(&mut self, stmt: &Stmt, kind: &ForKind, body: &AstBlock) {
        match kind {
            ForKind::Infinite => self.lower_infinite_loop(body),
            ForKind::While(cond) => self.lower_while_loop(cond, body),
            ForKind::In { var, iterable } => self.lower_in_loop(stmt, var, iterable, body),
        }
    }

    /// Lowers `for { ... }`: the body is the loop head and jumps back to its own start.
    fn lower_infinite_loop(&mut self, body: &AstBlock) {
        let body_id = self.new_block();
        let exit = self.new_block();

        self.set_terminator(Terminator::Jump(body_id));
        self.switch_to(body_id);
        self.lower_loop_body(body, exit, body_id);
        self.set_terminator(Terminator::Jump(body_id));
        self.switch_to(exit);
    }

    /// Lowers `for cond { ... }`: a header block tests the condition on every round.
    fn lower_while_loop(&mut self, cond: &Expr, body: &AstBlock) {
        let header = self.new_block();
        let body_id = self.new_block();
        let exit = self.new_block();

        self.set_terminator(Terminator::Jump(header));
        self.switch_to(header);
        let cond = self.lower_expr(cond);
        self.set_terminator(Terminator::Branch {
            cond,
            then_to: body_id,
            else_to: exit,
        });

        self.switch_to(body_id);
        self.lower_loop_body(body, exit, header);
        self.set_terminator(Terminator::Jump(header));
        self.switch_to(exit);
    }

    /// Lowers `for i in a..b` and `for i in a..=b`.
    fn lower_in_loop(&mut self, stmt: &Stmt, var: &Ident, iterable: &Expr, body: &AstBlock) {
        let ExprKind::Binary {
            op: range_op @ (BinaryOp::Range | BinaryOp::RangeInclusive),
            left,
            right,
        } = &iterable.kind
        else {
            // The loop is skipped; the error makes `lower` fail anyway.
            self.internal_error(iterable.span, "a `for` loop does not iterate over a range");
            return;
        };

        let range = RangeLoop {
            var,
            start: left,
            end: right,
            inclusive: *range_op == BinaryOp::RangeInclusive,
        };
        self.lower_range_loop(stmt, &range, body);
    }

    /// Lowers a range loop: a header that compares, the body, and a step block that counts up.
    fn lower_range_loop(&mut self, stmt: &Stmt, range: &RangeLoop, body: &AstBlock) {
        let header = self.new_block();
        let body_id = self.new_block();
        let step = self.new_block();
        let exit = self.new_block();

        let start = self.lower_expr(range.start);
        let var_type = self.local_type(stmt.id);
        let var_slot = self.declare_slot(&range.var.name, stmt.id, var_type);
        self.emit_store(var_slot, start);
        let end = self.lower_expr(range.end);
        let end_slot = self.new_slot(END_SLOT, var_type);
        self.emit_store(end_slot, end);
        self.set_terminator(Terminator::Jump(header));

        let loc = self.loc(stmt.span);
        let compare = if range.inclusive {
            BinOp::Le(var_type.sign)
        } else {
            BinOp::Lt(var_type.sign)
        };
        self.switch_to(header);
        let current = self.emit_load(var_slot);
        let limit = self.emit_load(end_slot);
        let in_range = self.emit_binary(compare, [current, limit], loc, IrType::I8);
        self.set_terminator(Terminator::Branch {
            cond: in_range,
            then_to: body_id,
            else_to: exit,
        });

        self.switch_to(body_id);
        self.lower_loop_body(body, exit, step);
        self.set_terminator(Terminator::Jump(step));

        self.switch_to(step);
        if range.inclusive {
            self.lower_inclusive_step(var_slot, end_slot, exit, header, loc);
        } else {
            self.lower_increment(var_slot, header, loc);
        }
        self.switch_to(exit);
    }

    /// Ends the step block of `a..=b`: leave when `i == end`, otherwise count up.
    ///
    /// The equality test comes first so that `end + 1` is never computed, which would
    /// overflow when `end` is `i64::MAX`.
    fn lower_inclusive_step(
        &mut self,
        var_slot: SlotId,
        end_slot: SlotId,
        exit: BlockId,
        header: BlockId,
        loc: Loc,
    ) {
        let current = self.emit_load(var_slot);
        let limit = self.emit_load(end_slot);
        let at_end = self.emit_binary(BinOp::Eq, [current, limit], loc, IrType::I8);
        let increment = self.new_block();
        self.set_terminator(Terminator::Branch {
            cond: at_end,
            then_to: exit,
            else_to: increment,
        });

        self.switch_to(increment);
        self.lower_increment(var_slot, header, loc);
    }

    /// Adds `i = i + 1` to the current block and jumps to the loop header.
    fn lower_increment(&mut self, var_slot: SlotId, header: BlockId, loc: Loc) {
        let var_type = self.slot_type(var_slot);
        let current = self.emit_load(var_slot);
        let one = self.emit_const(1, var_type.ty);
        let add = BinOp::Add(var_type.sign);
        let next = self.emit_binary(add, [current, one], loc, var_type.ty);
        self.emit_store(var_slot, next);
        self.set_terminator(Terminator::Jump(header));
    }

    /// Lowers a loop body with `break` and `continue` pointing at the given blocks.
    fn lower_loop_body(&mut self, body: &AstBlock, break_to: BlockId, continue_to: BlockId) {
        self.loops.push(LoopTargets {
            break_to,
            continue_to,
        });
        self.lower_block(body);
        self.loops.pop();
    }

    /// Lowers `return` and `return e`.
    fn lower_return(&mut self, value: Option<&Expr>) {
        let value = value.map(|expr| self.lower_expr(expr));
        self.terminate(Terminator::Return(value));
    }

    /// Lowers `break` and `continue`, which jump to the targets of the innermost loop.
    fn lower_loop_jump(&mut self, span: Span, jump: LoopJump) {
        let Some(targets) = self.loops.last().copied() else {
            self.internal_error(span, "`break` or `continue` outside of a loop");
            return;
        };

        let target = match jump {
            LoopJump::Break => targets.break_to,
            LoopJump::Continue => targets.continue_to,
        };
        self.terminate(Terminator::Jump(target));
    }

    /// Lowers an expression statement, which the checker only allows to be a call.
    fn lower_expr_stmt(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                self.lower_call(expr, callee, args);
            }
            _ => {
                // Lowered anyway, so the temps stay consistent; the error makes `lower` fail.
                self.internal_error(expr.span, "an expression statement is not a call");
                self.lower_expr(expr);
            }
        }
    }

    /// Lowers an expression and returns the temp that holds its value.
    fn lower_expr(&mut self, expr: &Expr) -> Temp {
        match &expr.kind {
            ExprKind::Int(value) => self.lower_int(expr, *value),
            ExprKind::Bool(value) => self.emit_const(i64::from(*value), IrType::I8),
            ExprKind::Name(_) => self.lower_name(expr),
            ExprKind::Unary { op, operand } => self.lower_unary(expr, *op, operand),
            ExprKind::Binary { op, left, right } => self.lower_binary(expr, *op, left, right),
            ExprKind::Call { callee, args } => self.lower_call_value(expr, callee, args),
        }
    }

    /// Lowers an integer literal; the checker keeps literals within `i64`.
    fn lower_int(&mut self, expr: &Expr, value: u64) -> Temp {
        let value = match i64::try_from(value) {
            Ok(value) => value,
            Err(_) => {
                self.internal_error(expr.span, "an integer literal does not fit in `i64`");
                i64::MAX
            }
        };
        let value_type = self.expr_type(expr);
        self.emit_const(value, value_type.ty)
    }

    /// Lowers a read of a variable.
    fn lower_name(&mut self, expr: &Expr) -> Temp {
        match self.local_slot(expr.id) {
            Some(slot) => self.emit_load(slot),
            None => {
                self.internal_error(expr.span, "a name is not bound to a variable");
                self.emit_const(0, IrType::I64)
            }
        }
    }

    /// Lowers a prefix operator.
    fn lower_unary(&mut self, expr: &Expr, op: UnaryOp, operand: &Expr) -> Temp {
        match op {
            UnaryOp::Neg => self.lower_neg(expr, operand),
            UnaryOp::Not => {
                let src = self.lower_expr(operand);
                let dst = self.new_temp(IrType::I8);
                self.emit(Inst::Not { dst, src });
                dst
            }
            UnaryOp::BitNot | UnaryOp::AddrOf | UnaryOp::Deref => {
                self.internal_error(expr.span, &format!("the operator `{op}` is not supported"));
                self.lower_expr(operand)
            }
        }
    }

    /// Lowers `-e`; the literal `-9223372036854775808` is a single constant.
    fn lower_neg(&mut self, expr: &Expr, operand: &Expr) -> Temp {
        if operand.kind == ExprKind::Int(i64::MIN.unsigned_abs()) {
            let value_type = self.expr_type(expr);
            return self.emit_const(i64::MIN, value_type.ty);
        }

        let src = self.lower_expr(operand);
        let value_type = self.expr_type(expr);
        let dst = self.new_temp(value_type.ty);
        let loc = self.loc(expr.span);
        self.emit(Inst::Neg { dst, src, loc });
        dst
    }

    /// Lowers an infix operator.
    fn lower_binary(&mut self, expr: &Expr, op: BinaryOp, left: &Expr, right: &Expr) -> Temp {
        match op {
            BinaryOp::And => self.lower_short_circuit(AND_SLOT, true, left, right),
            BinaryOp::Or => self.lower_short_circuit(OR_SLOT, false, left, right),
            _ => self.lower_plain_binary(expr, op, left, right),
        }
    }

    /// Lowers an operator that evaluates both operands, left first.
    fn lower_plain_binary(&mut self, expr: &Expr, op: BinaryOp, left: &Expr, right: &Expr) -> Temp {
        let lhs = self.lower_expr(left);
        let rhs = self.lower_expr(right);
        // Both operands have the same type, so the left one decides the signedness.
        let operand_type = self.expr_type(left);
        match bin_op(op, operand_type.sign) {
            Some(bin_op) => {
                let loc = self.loc(expr.span);
                let result_type = self.expr_type(expr);
                self.emit_binary(bin_op, [lhs, rhs], loc, result_type.ty)
            }
            None => {
                self.internal_error(expr.span, &format!("the operator `{op}` is not supported"));
                self.emit_const(0, IrType::I64)
            }
        }
    }

    /// Lowers `a && b` or `a || b` through a hidden slot that holds the result.
    ///
    /// `evaluate_right_if_true` is true for `&&`, where `b` only runs when `a` is true.
    fn lower_short_circuit(
        &mut self,
        slot_name: &str,
        evaluate_right_if_true: bool,
        left: &Expr,
        right: &Expr,
    ) -> Temp {
        let slot = self.new_slot(slot_name, ValueType::BOOL);
        let right_block = self.new_block();
        let join = self.new_block();

        let lhs = self.lower_expr(left);
        self.emit_store(slot, lhs);
        let (then_to, else_to) = if evaluate_right_if_true {
            (right_block, join)
        } else {
            (join, right_block)
        };
        self.set_terminator(Terminator::Branch {
            cond: lhs,
            then_to,
            else_to,
        });

        self.switch_to(right_block);
        let rhs = self.lower_expr(right);
        self.emit_store(slot, rhs);
        self.set_terminator(Terminator::Jump(join));

        self.switch_to(join);
        self.emit_load(slot)
    }

    /// Lowers a call used as a value.
    fn lower_call_value(&mut self, expr: &Expr, callee: &Ident, args: &[Expr]) -> Temp {
        match self.lower_call(expr, callee, args) {
            Some(result) => result,
            None => {
                self.internal_error(expr.span, "a call without a value is used as a value");
                self.emit_const(0, IrType::I64)
            }
        }
    }

    /// Lowers a call to `print` or to a user function; returns the result temp, if there is one.
    fn lower_call(&mut self, expr: &Expr, callee: &Ident, args: &[Expr]) -> Option<Temp> {
        match self.checked.bindings.get(&expr.id) {
            Some(Binding::Print) => {
                self.lower_print(expr.span, args);
                return None;
            }
            Some(Binding::Function(_)) => {}
            _ => self.internal_error(expr.span, "a call is not bound to a function"),
        }

        let mut arg_temps = Vec::new();
        for arg in args {
            arg_temps.push(self.lower_expr(arg));
        }

        let dst = self
            .function_return(expr.span, &callee.name)
            .map(|value_type| self.new_temp(value_type.ty));
        self.emit(Inst::Call {
            dst,
            func: callee.name.clone(),
            args: arg_temps,
        });
        dst
    }

    /// Lowers `print(e)`; the checker guarantees exactly one argument.
    fn lower_print(&mut self, span: Span, args: &[Expr]) {
        let Some(arg) = args.first() else {
            self.internal_error(span, "`print` has no argument");
            return;
        };

        let src = self.lower_expr(arg);
        let kind = self.print_kind(arg);
        self.emit(Inst::Print { src, kind });
    }

    /// Chooses how to print a value from its type.
    fn print_kind(&mut self, arg: &Expr) -> PrintKind {
        let Some(id) = self.checked.expr_types.get(&arg.id).copied() else {
            self.missing_type_error();
            return PrintKind::I64;
        };
        match self.checked.types.kind(id) {
            TypeKind::Int {
                bits: 64,
                signed: true,
            } => PrintKind::I64,
            TypeKind::Bool => PrintKind::Bool,
            _ => {
                self.internal_error(arg.span, "the type of a printed value is unknown");
                PrintKind::I64
            }
        }
    }
}

/// Returns the operation of a compound assignment on operands of signedness `sign`, or `None` for a plain `=` (and for `@=`).
fn compound_op(op: AssignOp, sign: Sign) -> Option<BinOp> {
    match op {
        AssignOp::Add => Some(BinOp::Add(sign)),
        AssignOp::Sub => Some(BinOp::Sub(sign)),
        AssignOp::Mul => Some(BinOp::Mul(sign)),
        AssignOp::Div => Some(BinOp::Div(sign)),
        AssignOp::Assign | AssignOp::MatMul => None,
    }
}

/// Returns the IR operation of an infix operator on operands of signedness `sign`, or `None` when the IR has no such operation.
fn bin_op(op: BinaryOp, sign: Sign) -> Option<BinOp> {
    match op {
        BinaryOp::Add => Some(BinOp::Add(sign)),
        BinaryOp::Sub => Some(BinOp::Sub(sign)),
        BinaryOp::Mul => Some(BinOp::Mul(sign)),
        BinaryOp::Div => Some(BinOp::Div(sign)),
        BinaryOp::Rem => Some(BinOp::Rem(sign)),
        BinaryOp::Eq => Some(BinOp::Eq),
        BinaryOp::Ne => Some(BinOp::Ne),
        BinaryOp::Lt => Some(BinOp::Lt(sign)),
        BinaryOp::Le => Some(BinOp::Le(sign)),
        BinaryOp::Gt => Some(BinOp::Gt(sign)),
        BinaryOp::Ge => Some(BinOp::Ge(sign)),
        BinaryOp::MatMul
        | BinaryOp::Shl
        | BinaryOp::Shr
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::And
        | BinaryOp::Or => None,
    }
}
