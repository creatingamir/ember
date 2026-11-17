//! The machine IR (MIR): ARM64 instructions over virtual registers, shared by every back-end step.

use source::{Diagnostic, Span};

/// A virtual register. Numbers below the function's IR temp count are those temps (`v3` is `t3`);
/// numbers from there up are values that instruction selection creates inside one IR instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VReg(pub u32);

/// A physical integer register `x0` to `x30`; `x18` is never used (Apple reserves it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XReg(pub u8);

/// An operand register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    /// A virtual register, until allocation.
    Virt(VReg),
    /// A physical register.
    Phys(XReg),
}

impl Reg {
    /// The physical register `x<number>`.
    pub fn phys(number: u8) -> Reg {
        Reg::Phys(XReg(number))
    }
}

/// A memory operand. `Slot` and `Cell` exist until frame layout; `Address` after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mem {
    /// The cell of an IR slot.
    Slot(ir::SlotId),
    /// The cell of a virtual register (only the allocator creates these).
    Cell(VReg),
    /// `[sp, #offset]` when the offset is at most 32760, else `[sp, x17]` after `x17` is loaded.
    Address {
        /// The distance from `sp` in bytes.
        offset: u64,
    },
}

/// A condition for `b.<cond>` and `cset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Signed less than.
    Lt,
    /// Signed less than or equal.
    Le,
    /// Signed greater than.
    Gt,
    /// Signed greater than or equal.
    Ge,
    /// Signed overflow.
    Vs,
    /// No signed overflow; only branch relaxation creates it.
    Vc,
}

impl Cond {
    /// The condition that holds exactly when this one does not.
    pub fn inverse(self) -> Cond {
        match self {
            Cond::Eq => Cond::Ne,
            Cond::Ne => Cond::Eq,
            Cond::Lt => Cond::Ge,
            Cond::Ge => Cond::Lt,
            Cond::Le => Cond::Gt,
            Cond::Gt => Cond::Le,
            Cond::Vs => Cond::Vc,
            Cond::Vc => Cond::Vs,
        }
    }
}

/// How an instruction uses one of its registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegRole {
    /// The instruction reads the register.
    Use,
    /// The instruction writes the register.
    Def,
}

/// One machine instruction (or a pseudo-instruction that the printer expands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MInst {
    /// A label on its own line.
    Label(String),
    /// `movz` then one `movk` per further non-zero 16-bit chunk.
    MovImm {
        /// The register that receives the constant.
        dst: Reg,
        /// The 64-bit constant (a negative `i64` is kept as its bit pattern).
        value: u64,
    },
    /// `mov dst, src` (both physical after allocation).
    Mov {
        /// The destination register.
        dst: Reg,
        /// The source register.
        src: Reg,
    },
    /// `adrp` + `add` of a local label.
    LoadAddress {
        /// The register that receives the address.
        dst: Reg,
        /// The local label.
        label: String,
    },
    /// `ldr dst, <mem>`
    Load {
        /// The register that receives the value.
        dst: Reg,
        /// The memory read.
        mem: Mem,
    },
    /// `str src, <mem>`
    Store {
        /// The register stored.
        src: Reg,
        /// The memory written.
        mem: Mem,
    },
    /// `adds dst, lhs, rhs`
    Adds {
        /// The sum.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
    },
    /// `subs dst, lhs, rhs`
    Subs {
        /// The difference.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
    },
    /// `negs dst, src`
    Negs {
        /// The negated value.
        dst: Reg,
        /// The operand.
        src: Reg,
    },
    /// `mul dst, lhs, rhs`
    Mul {
        /// The low 64 bits of the product.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
    },
    /// `smulh dst, lhs, rhs`
    Smulh {
        /// The high 64 bits of the signed product.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
    },
    /// `sdiv dst, lhs, rhs`
    Sdiv {
        /// The quotient.
        dst: Reg,
        /// The dividend.
        lhs: Reg,
        /// The divisor.
        rhs: Reg,
    },
    /// `msub dst, mul_lhs, mul_rhs, minuend` (dst = minuend - mul_lhs * mul_rhs)
    Msub {
        /// The result.
        dst: Reg,
        /// The left factor.
        mul_lhs: Reg,
        /// The right factor.
        mul_rhs: Reg,
        /// The value the product is subtracted from.
        minuend: Reg,
    },
    /// `cmp lhs, rhs`
    Cmp {
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
    },
    /// `cmp lhs, rhs, asr #63`
    CmpAsr63 {
        /// The left operand.
        lhs: Reg,
        /// The right operand, shifted right by 63 (its sign) before the comparison.
        rhs: Reg,
    },
    /// `cmp lhs, #imm`
    CmpImm {
        /// The register compared.
        lhs: Reg,
        /// The 12-bit unsigned constant.
        imm: u16,
    },
    /// `cmn lhs, #imm` (compares with the negated constant)
    CmnImm {
        /// The register compared.
        lhs: Reg,
        /// The 12-bit unsigned constant.
        imm: u16,
    },
    /// `cset dst, <cond>`
    Cset {
        /// 1 when the condition holds, else 0.
        dst: Reg,
        /// The condition read from the flags.
        cond: Cond,
    },
    /// `eor dst, src, #1`
    EorOne {
        /// The flipped value.
        dst: Reg,
        /// The operand (0 or 1).
        src: Reg,
    },
    /// `b target`
    B {
        /// The label jumped to.
        target: String,
    },
    /// `b.<cond> target`
    BCond {
        /// The condition read from the flags.
        cond: Cond,
        /// The label jumped to.
        target: String,
    },
    /// `cbnz reg, target`
    Cbnz {
        /// The register tested.
        reg: Reg,
        /// The label jumped to when the register is not zero.
        target: String,
    },
    /// `cbz reg, target`; only branch relaxation creates it.
    Cbz {
        /// The register tested.
        reg: Reg,
        /// The label jumped to when the register is zero.
        target: String,
    },
    /// `b.<cond> target` to a panic path. The panic path never reads a value, so the allocator may
    /// store a result after this branch.
    PanicIf {
        /// The condition read from the flags.
        cond: Cond,
        /// The label of the panic path.
        target: String,
    },
    /// `bl symbol`
    Bl {
        /// The function called.
        symbol: String,
        /// How many argument registers (`x0` upward) the call reads.
        args: u8,
    },
    /// `brk #1`
    Brk,
    /// Printed as the prologue: save `x29` and `x30`, point `x29` at the frame, reserve the frame.
    Prologue,
    /// Printed as the epilogue: `mov sp, x29`, `ldp x29, x30, [sp], #16`, `ret`.
    Epilogue,
}

impl MInst {
    /// Calls `visit` once for every register operand, in the order the operands are written.
    ///
    /// A call reads its argument registers `x0` to `x<args - 1>`. It also clobbers every caller-saved
    /// register (`x0` to `x17`), which this method does not report. Changes to the argument registers
    /// are ignored.
    pub fn for_each_reg(&mut self, mut visit: impl FnMut(&mut Reg, RegRole)) {
        use RegRole::{Def, Use};
        match self {
            MInst::MovImm { dst, .. }
            | MInst::LoadAddress { dst, .. }
            | MInst::Cset { dst, .. } => {
                visit(dst, Def);
            }
            MInst::Mov { dst, src } | MInst::Negs { dst, src } | MInst::EorOne { dst, src } => {
                visit(dst, Def);
                visit(src, Use);
            }
            MInst::Load { dst, .. } => visit(dst, Def),
            MInst::Store { src, .. } => visit(src, Use),
            MInst::Adds { dst, lhs, rhs }
            | MInst::Subs { dst, lhs, rhs }
            | MInst::Mul { dst, lhs, rhs }
            | MInst::Smulh { dst, lhs, rhs }
            | MInst::Sdiv { dst, lhs, rhs } => {
                visit(dst, Def);
                visit(lhs, Use);
                visit(rhs, Use);
            }
            MInst::Msub {
                dst,
                mul_lhs,
                mul_rhs,
                minuend,
            } => {
                visit(dst, Def);
                visit(mul_lhs, Use);
                visit(mul_rhs, Use);
                visit(minuend, Use);
            }
            MInst::Cmp { lhs, rhs } | MInst::CmpAsr63 { lhs, rhs } => {
                visit(lhs, Use);
                visit(rhs, Use);
            }
            MInst::CmpImm { lhs, .. } | MInst::CmnImm { lhs, .. } => visit(lhs, Use),
            MInst::Cbnz { reg, .. } | MInst::Cbz { reg, .. } => visit(reg, Use),
            MInst::Bl { args, .. } => {
                for number in 0..*args {
                    visit(&mut Reg::phys(number), Use);
                }
            }
            MInst::Label(_)
            | MInst::B { .. }
            | MInst::BCond { .. }
            | MInst::PanicIf { .. }
            | MInst::Brk
            | MInst::Prologue
            | MInst::Epilogue => {}
        }
    }

    /// The memory operand of a load or store, if this is one.
    pub fn mem_mut(&mut self) -> Option<&mut Mem> {
        match self {
            MInst::Load { mem, .. } | MInst::Store { mem, .. } => Some(mem),
            _ => None,
        }
    }
}

/// One function in MIR: a flat instruction list (blocks are `Label`s inside it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MFunction {
    /// The user function name (the symbol is `_em_<name>`).
    pub name: String,
    /// The number of IR slots; slot `k` has cell `k`.
    pub slot_count: u32,
    /// The number of virtual registers (IR temps plus the ones isel makes); set by isel.
    pub vreg_count: u32,
    /// The number the next local label gets; isel stops at the first unused one and relaxation continues.
    pub next_label: u32,
    /// The instructions in order: `Prologue`, parameter stores, blocks, panic paths.
    pub insts: Vec<MInst>,
    /// The frame size in bytes reserved below `x29`; set by frame layout, 0 before.
    pub frame_size: u64,
}

/// A case that cannot happen for a checked program (decision 0026), at the empty span 0..0.
///
/// The IR has no source spans, so no better location exists.
pub fn internal_error(what: &str) -> Diagnostic {
    Diagnostic::error(format!("internal compiler error: {what}"), Span::new(0, 0))
}
