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

/// The width of an operation: `w` (32-bit) or `x` (64-bit) register names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// `w` registers; the top 32 bits of the destination are cleared.
    W,
    /// `x` registers.
    X,
}

impl Size {
    /// The part of `value` that a constant load of this size writes: all 64 bits, or the low 32.
    pub fn truncate(self, value: u64) -> u64 {
        match self {
            Size::W => value & 0xffff_ffff,
            Size::X => value,
        }
    }
}

/// How a load or store moves memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// One byte: `ldrsb`/`ldrb` into a `w` register, `strb`.
    Byte {
        /// Whether a load sign-extends (`ldrsb`) or zero-extends (`ldrb`).
        signed: bool,
    },
    /// Two bytes: `ldrsh`/`ldrh`, `strh`.
    Half {
        /// Whether a load sign-extends (`ldrsh`) or zero-extends (`ldrh`).
        signed: bool,
    },
    /// Four bytes: `ldr w`/`str w`.
    Word,
    /// Eight bytes: `ldr x`/`str x`.
    Double,
}

impl Access {
    /// The number of bytes moved: 1, 2, 4 or 8.
    pub fn bytes(self) -> u64 {
        match self {
            Access::Byte { .. } => 1,
            Access::Half { .. } => 2,
            Access::Word => 4,
            Access::Double => 8,
        }
    }
}

/// How `CmpExt` extends its second operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ext {
    /// Sign-extend the low byte.
    Sxtb,
    /// Sign-extend the low half-word.
    Sxth,
    /// Sign-extend the low word.
    Sxtw,
}

/// The register an address is relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    /// The stack pointer: cells and outgoing arguments.
    Sp,
    /// The frame pointer `x29`: incoming stack arguments, which sit above the saved `x29` and `x30`.
    Fp,
}

/// A memory operand. `Slot`, `Cell`, `InArg` and `OutArg` exist until frame layout; `Address` after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mem {
    /// The cell of an IR slot.
    Slot(ir::SlotId),
    /// The cell of a virtual register (only the allocator creates these).
    Cell(VReg),
    /// An incoming stack argument at a byte offset into the argument area: `[x29, #16 + offset]`.
    InArg {
        /// The byte offset of the argument, packed by Apple's rule.
        offset: u32,
    },
    /// An outgoing stack argument at a byte offset into the argument area: `[sp, #offset]`.
    OutArg {
        /// The byte offset of the argument, packed by Apple's rule.
        offset: u32,
    },
    /// `[base, #offset]` when the offset fits the access, else `[base, x17]` after `x17` is loaded.
    Address {
        /// The register the offset is added to.
        base: Base,
        /// The distance from the base register in bytes.
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
    /// Unsigned lower (carry clear).
    Lo,
    /// Unsigned lower or the same.
    Ls,
    /// Unsigned higher.
    Hi,
    /// Unsigned higher or the same (carry set).
    Hs,
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
            Cond::Lo => Cond::Hs,
            Cond::Hs => Cond::Lo,
            Cond::Ls => Cond::Hi,
            Cond::Hi => Cond::Ls,
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
        /// `W` loads the low 32 bits of `value` and clears the rest.
        size: Size,
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
        /// How many bytes are read, and whether they are extended with their sign.
        access: Access,
    },
    /// `str src, <mem>`
    Store {
        /// The register stored.
        src: Reg,
        /// The memory written.
        mem: Mem,
        /// How many bytes are written.
        access: Access,
    },
    /// `adds dst, lhs, rhs`
    Adds {
        /// The sum.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `subs dst, lhs, rhs`
    Subs {
        /// The difference.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `negs dst, src`
    Negs {
        /// The negated value.
        dst: Reg,
        /// The operand.
        src: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `mul dst, lhs, rhs`
    Mul {
        /// The low 64 bits of the product.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
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
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
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
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `add dst, lhs, rhs` (no flags)
    Add {
        /// The sum.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `sub dst, lhs, rhs` (no flags)
    Sub {
        /// The difference.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `neg dst, src` (no flags)
    Neg {
        /// The negated value.
        dst: Reg,
        /// The operand.
        src: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `udiv dst, lhs, rhs`
    Udiv {
        /// The quotient.
        dst: Reg,
        /// The dividend.
        lhs: Reg,
        /// The divisor.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `umulh dst, lhs, rhs` (always `x`)
    Umulh {
        /// The high 64 bits of the unsigned product.
        dst: Reg,
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
    },
    /// `smull dst, lhs, rhs`: a 64-bit signed product of two `w` operands.
    Smull {
        /// The `x` result.
        dst: Reg,
        /// The left `w` operand.
        lhs: Reg,
        /// The right `w` operand.
        rhs: Reg,
    },
    /// `umull dst, lhs, rhs`: a 64-bit unsigned product of two `w` operands.
    Umull {
        /// The `x` result.
        dst: Reg,
        /// The left `w` operand.
        lhs: Reg,
        /// The right `w` operand.
        rhs: Reg,
    },
    /// `cmp lhs, rhs, <ext>`: compares `lhs` with the extension of the low part of `rhs`.
    CmpExt {
        /// The left operand, named as `size` says.
        lhs: Reg,
        /// The right operand, always a `w` register.
        rhs: Reg,
        /// How the right operand is extended.
        ext: Ext,
        /// Whether `lhs` is a `w` or an `x` register.
        size: Size,
    },
    /// `tst reg, #imm`: sets the flags from `reg & imm`.
    Tst {
        /// The register tested.
        reg: Reg,
        /// The mask, which must be a valid logical immediate.
        imm: u64,
        /// Whether `reg` is a `w` or an `x` register.
        size: Size,
    },
    /// `sxtw dst, src`: sign-extends a `w` register into an `x` register.
    Sxtw {
        /// The `x` result.
        dst: Reg,
        /// The `w` source.
        src: Reg,
    },
    /// `mov dst, src` on `w` registers, which zero-extends into the `x` register.
    Uxtw {
        /// The `x` result.
        dst: Reg,
        /// The `w` source.
        src: Reg,
    },
    /// `cmp lhs, rhs`
    Cmp {
        /// The left operand.
        lhs: Reg,
        /// The right operand.
        rhs: Reg,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
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
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
    },
    /// `cmn lhs, #imm` (compares with the negated constant)
    CmnImm {
        /// The register compared.
        lhs: Reg,
        /// The 12-bit unsigned constant.
        imm: u16,
        /// Whether the operation works on `w` or `x` registers.
        size: Size,
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
            MInst::Mov { dst, src }
            | MInst::Negs { dst, src, .. }
            | MInst::Neg { dst, src, .. }
            | MInst::Sxtw { dst, src }
            | MInst::Uxtw { dst, src }
            | MInst::EorOne { dst, src } => {
                visit(dst, Def);
                visit(src, Use);
            }
            MInst::Load { dst, .. } => visit(dst, Def),
            MInst::Store { src, .. } => visit(src, Use),
            MInst::Adds { dst, lhs, rhs, .. }
            | MInst::Subs { dst, lhs, rhs, .. }
            | MInst::Add { dst, lhs, rhs, .. }
            | MInst::Sub { dst, lhs, rhs, .. }
            | MInst::Mul { dst, lhs, rhs, .. }
            | MInst::Smulh { dst, lhs, rhs }
            | MInst::Umulh { dst, lhs, rhs }
            | MInst::Smull { dst, lhs, rhs }
            | MInst::Umull { dst, lhs, rhs }
            | MInst::Sdiv { dst, lhs, rhs, .. }
            | MInst::Udiv { dst, lhs, rhs, .. } => {
                visit(dst, Def);
                visit(lhs, Use);
                visit(rhs, Use);
            }
            MInst::Msub {
                dst,
                mul_lhs,
                mul_rhs,
                minuend,
                ..
            } => {
                visit(dst, Def);
                visit(mul_lhs, Use);
                visit(mul_rhs, Use);
                visit(minuend, Use);
            }
            MInst::Cmp { lhs, rhs, .. }
            | MInst::CmpAsr63 { lhs, rhs }
            | MInst::CmpExt { lhs, rhs, .. } => {
                visit(lhs, Use);
                visit(rhs, Use);
            }
            MInst::CmpImm { lhs, .. } | MInst::CmnImm { lhs, .. } => visit(lhs, Use),
            MInst::Tst { reg, .. } => visit(reg, Use),
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
    /// Where the function's name is written; errors about the whole function point here.
    pub span: Span,
    /// The end of the largest outgoing stack-argument area of any call, in bytes (before rounding up to 16).
    pub out_arg_bytes: u32,
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
