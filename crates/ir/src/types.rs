//! The types that make up an IR module.

/// A temporary that is assigned exactly once; printed as `t0`, `t1`, and so on.
///
/// Temps are function-wide: a temp may be used in a block other than the one that defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Temp(pub u32);

/// The index of a slot in `Function::slots`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub u32);

/// The index of a block in `Function::blocks`; printed as `bb0`, `bb1`, and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

/// A 1-based source position, used in panic messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    /// The line, counting from 1.
    pub line: u32,
    /// The column, counting from 1.
    pub col: u32,
}

/// A whole program: its functions in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The functions of the program.
    pub functions: Vec<Function>,
}

/// One function: its slots and its blocks of instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The function name.
    pub name: String,
    /// The slot each parameter is stored in on entry, in order.
    pub params: Vec<SlotId>,
    /// Whether the function returns a value (every value is 64 bits wide).
    pub returns_value: bool,
    /// The parameters first, then locals and hidden slots in creation order.
    pub slots: Vec<Slot>,
    /// The blocks, where `blocks[i].id == BlockId(i)`; `blocks[0]` is the entry.
    pub blocks: Vec<Block>,
}

/// A memory cell that holds one named variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The source name; hidden slots are named `$end`, `$and` and `$or`, and names may repeat.
    pub name: String,
}

/// A straight run of instructions that ends in one terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The position of the block in `Function::blocks`.
    pub id: BlockId,
    /// The instructions, in order.
    pub insts: Vec<Inst>,
    /// How the block ends.
    pub term: Terminator,
}

/// One instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inst {
    /// `dst = value`; booleans are 0 and 1.
    Const {
        /// The temp that receives the value.
        dst: Temp,
        /// The constant.
        value: i64,
    },
    /// `dst = the value in slot`.
    Load {
        /// The temp that receives the value.
        dst: Temp,
        /// The slot that is read.
        slot: SlotId,
    },
    /// `slot = src`.
    Store {
        /// The slot that is written.
        slot: SlotId,
        /// The temp that holds the value.
        src: Temp,
    },
    /// `dst = lhs op rhs`.
    Binary {
        /// The temp that receives the result.
        dst: Temp,
        /// The operation.
        op: BinOp,
        /// The left operand.
        lhs: Temp,
        /// The right operand.
        rhs: Temp,
        /// The start of the expression, reported if the operation panics.
        loc: Loc,
    },
    /// `dst = -src`, which panics on `i64::MIN`.
    Neg {
        /// The temp that receives the result.
        dst: Temp,
        /// The operand.
        src: Temp,
        /// The start of the expression, reported if the operation panics.
        loc: Loc,
    },
    /// `dst = 1 - src` for a boolean `src`.
    Not {
        /// The temp that receives the result.
        dst: Temp,
        /// The operand.
        src: Temp,
    },
    /// A call to a user function.
    Call {
        /// The temp that receives the result, if the function returns a value.
        dst: Option<Temp>,
        /// The name of the function.
        func: String,
        /// The arguments, in order.
        args: Vec<Temp>,
    },
    /// The built-in `print`.
    Print {
        /// The value to print.
        src: Temp,
        /// How to show the value.
        kind: PrintKind,
    },
}

/// The binary operations; `Add` to `Rem` are checked for overflow and division by zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`; panics on a zero divisor ("division by zero", first) or `i64::MIN / -1` ("integer overflow").
    Div,
    /// `%`; panics like `Div`, including for `i64::MIN % -1`.
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// How `print` shows its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintKind {
    /// As a signed 64-bit integer.
    I64,
    /// As `true` or `false`.
    Bool,
}

/// How a block ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Continue in another block.
    Jump(BlockId),
    /// Continue in one of two blocks, depending on a boolean temp.
    Branch {
        /// The boolean temp that is tested.
        cond: Temp,
        /// Where to continue when it is true.
        then_to: BlockId,
        /// Where to continue when it is false.
        else_to: BlockId,
    },
    /// Leave the function, with a value when it returns one.
    Return(Option<Temp>),
    /// Control never gets here.
    Unreachable,
}
