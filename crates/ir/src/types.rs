//! The types that make up an IR module.

use source::Span;

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

/// The type of an IR value: a width, without signedness (decision 0029). `bool` is `I8` holding 0 or 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrType {
    /// 8 bits.
    I8,
    /// 16 bits.
    I16,
    /// 32 bits.
    I32,
    /// 64 bits.
    I64,
}

impl IrType {
    /// The width in bits: 8, 16, 32 or 64.
    pub fn bits(self) -> u32 {
        match self {
            IrType::I8 => 8,
            IrType::I16 => 16,
            IrType::I32 => 32,
            IrType::I64 => 64,
        }
    }

    /// The size in bytes, which is also the alignment: 1, 2, 4 or 8.
    pub fn bytes(self) -> u32 {
        self.bits() / 8
    }
}

/// Whether an operation, load or parameter treats its bits as signed or unsigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sign {
    /// Two's-complement signed.
    Signed,
    /// Unsigned.
    Unsigned,
}

/// One parameter: the slot it is stored in on entry, its type and its signedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// The slot the parameter is stored in on entry.
    pub slot: SlotId,
    /// The type of the parameter.
    pub ty: IrType,
    /// How the bits are read when they are loaded with extension.
    pub sign: Sign,
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
    /// The parameters, in order.
    pub params: Vec<Param>,
    /// The type of the result, or `None` when the function returns nothing.
    pub ret: Option<IrType>,
    /// The parameters first, then locals and hidden slots in creation order.
    pub slots: Vec<Slot>,
    /// The type of every temp: `temp_types[n]` is the type of `tn`.
    pub temp_types: Vec<IrType>,
    /// The blocks, where `blocks[i].id == BlockId(i)`; `blocks[0]` is the entry.
    pub blocks: Vec<Block>,
    /// Where the function's name is written; errors about the whole function point here.
    pub span: Span,
}

/// Memory for one variable: sized and aligned, without a type (decision 0029).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    /// The source name; hidden slots are named `$end`, `$and` and `$or`, and names may repeat.
    pub name: String,
    /// The size in bytes.
    pub size: u32,
    /// The alignment in bytes.
    pub align: u32,
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
        /// The value in the type of `dst`, as an `i64` bit pattern (a `u64` above `i64::MAX` is its two's complement).
        value: i64,
    },
    /// `dst = the value in slot`.
    Load {
        /// The temp that receives the value.
        dst: Temp,
        /// The slot that is read.
        slot: SlotId,
        /// Whether an 8- or 16-bit value is sign- or zero-extended; unused for wider types.
        sign: Sign,
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
///
/// Signedness is part of the operation, because the backend needs it (decision 0029).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add(Sign),
    /// `-`
    Sub(Sign),
    /// `*`
    Mul(Sign),
    /// `/`; panics on a zero divisor ("division by zero", first) or `MIN / -1` ("integer overflow").
    Div(Sign),
    /// `%`; panics like `Div`, including for `MIN % -1`.
    Rem(Sign),
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt(Sign),
    /// `<=`
    Le(Sign),
    /// `>`
    Gt(Sign),
    /// `>=`
    Ge(Sign),
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
