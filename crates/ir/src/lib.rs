//! The Ember intermediate representation: its types, lowering from the checked tree, and a text printer.

mod blocks;
mod lower;
mod print;
mod types;

pub use lower::lower;
pub use print::print_module;
pub use types::{
    BinOp, Block, BlockId, Function, Inst, IrType, Loc, Module, Param, PrintKind, Sign, Slot,
    SlotId, Temp, Terminator,
};
