//! End-to-end tests for integer widths: hand-built IR is assembled, linked and run, and its output is compared with Rust.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{
    FILE, binary_at, block_of, constant, emit_ok, load_of, module_with_main, param_of, print_of,
    run_module, run_module_in_abi_harness, single_block_function, slot_of, store_of,
    typed_function, unreachable_blocks,
};
use ir::{BinOp, Function, Inst, IrType, Loc, Module, PrintKind, Sign, Temp, Terminator};

// ----------------------------------------------------------------------------------------------
// Types, values and the reference arithmetic
// ----------------------------------------------------------------------------------------------

/// An integer type: an IR width and a signedness.
#[derive(Clone, Copy, Debug)]
struct Ty {
    /// The IR width.
    ir: IrType,
    /// Whether the type is signed.
    signed: bool,
}

impl Ty {
    /// The smallest value of the type.
    fn min(self) -> i128 {
        if self.signed {
            -(1i128 << (self.ir.bits() - 1))
        } else {
            0
        }
    }

    /// The largest value of the type.
    fn max(self) -> i128 {
        if self.signed {
            (1i128 << (self.ir.bits() - 1)) - 1
        } else {
            (1i128 << self.ir.bits()) - 1
        }
    }

    /// The sign that operations and loads of this type carry.
    fn sign(self) -> Sign {
        if self.signed {
            Sign::Signed
        } else {
            Sign::Unsigned
        }
    }

    /// How `print` shows a value of this type: as a signed or an unsigned 64-bit number.
    fn print_kind(self) -> PrintKind {
        if self.signed {
            PrintKind::I64
        } else {
            PrintKind::U64
        }
    }

    /// The value as the `i64` bit pattern of an IR constant: a `u64` above `i64::MAX` wraps to a negative number.
    fn encode(self, value: i128) -> i64 {
        if self.signed {
            i64::try_from(value).unwrap()
        } else {
            u64::try_from(value).unwrap() as i64
        }
    }

    /// The boundary values: for signed `MIN`, `MIN+1`, -2, -1, 0, 1, 2, `MAX-1`, `MAX`; for unsigned 0, 1, 2, `MAX/2`, `MAX-1`, `MAX`.
    fn boundaries(self) -> Vec<i128> {
        if self.signed {
            vec![
                self.min(),
                self.min() + 1,
                -2,
                -1,
                0,
                1,
                2,
                self.max() - 1,
                self.max(),
            ]
        } else {
            vec![0, 1, 2, self.max() / 2, self.max() - 1, self.max()]
        }
    }

    /// The boundary values plus every `step`th value of the whole range, sorted and without repeats.
    fn dense(self, step: usize) -> Vec<i128> {
        let mut values = self.boundaries();
        let range = self.min()..=self.max();
        values.extend(range.step_by(step));
        values.sort_unstable();
        values.dedup();
        values
    }

    /// Every value of the type; only sensible for 8 bits.
    fn all(self) -> Vec<i128> {
        (self.min()..=self.max()).collect()
    }
}

/// The four checked arithmetic operations that have two operands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl Arith {
    /// The IR operation for the sign of the operands.
    fn bin_op(self, sign: Sign) -> BinOp {
        match self {
            Arith::Add => BinOp::Add(sign),
            Arith::Sub => BinOp::Sub(sign),
            Arith::Mul => BinOp::Mul(sign),
            Arith::Div => BinOp::Div(sign),
            Arith::Rem => BinOp::Rem(sign),
        }
    }

    /// A number for the operation, used to give each test its own source location.
    fn number(self) -> u32 {
        match self {
            Arith::Add => 0,
            Arith::Sub => 1,
            Arith::Mul => 2,
            Arith::Div => 3,
            Arith::Rem => 4,
        }
    }
}

/// Why an operation panics.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Rejected {
    /// The result does not fit the type, or `MIN / -1`.
    Overflow,
    /// The divisor is 0.
    DivisionByZero,
}

impl Rejected {
    /// The panic reason the runtime prints.
    fn reason(self) -> &'static str {
        match self {
            Rejected::Overflow => "integer overflow",
            Rejected::DivisionByZero => "division by zero",
        }
    }
}

/// The result of `a OP b` as Rust's `checked_*` methods define it for the type, or why it panics.
///
/// The arithmetic is done in `i128`, which holds every `u64` product, and then range-checked.
fn evaluate(ty: Ty, op: Arith, a: i128, b: i128) -> Result<i128, Rejected> {
    let exact = match op {
        Arith::Add => a.checked_add(b),
        Arith::Sub => a.checked_sub(b),
        Arith::Mul => a.checked_mul(b),
        Arith::Div | Arith::Rem => {
            if b == 0 {
                return Err(Rejected::DivisionByZero);
            }
            if ty.signed && a == ty.min() && b == -1 {
                return Err(Rejected::Overflow);
            }
            if op == Arith::Div {
                a.checked_div(b)
            } else {
                a.checked_rem(b)
            }
        }
    };
    match exact {
        Some(value) if (ty.min()..=ty.max()).contains(&value) => Ok(value),
        _ => Err(Rejected::Overflow),
    }
}

/// The source location that test `(index, op)` gives its operation: unique per type and operation.
fn location(ty_index: u32, op: Arith) -> Loc {
    Loc {
        line: 10 + 10 * ty_index + op.number(),
        col: 3 + op.number(),
    }
}

/// A module whose only function is `main` with the given temp types and instructions.
fn main_only(temp_types: Vec<IrType>, insts: Vec<Inst>) -> Module {
    let main = single_block_function(
        "main",
        vec![],
        None,
        vec![],
        temp_types,
        insts,
        Terminator::Return(None),
    );
    Module {
        functions: vec![main],
    }
}

/// Builds instructions with consecutive temp numbers and remembers each temp's type.
struct Builder {
    /// The instructions so far.
    insts: Vec<Inst>,
    /// The type of every temp made so far.
    temp_types: Vec<IrType>,
}

impl Builder {
    /// A builder with nothing in it.
    fn new() -> Builder {
        Builder {
            insts: vec![],
            temp_types: vec![],
        }
    }

    /// Reserves the next temp with type `ir` and returns its number.
    fn temp(&mut self, ir: IrType) -> u32 {
        self.temp_types.push(ir);
        u32::try_from(self.temp_types.len() - 1).unwrap()
    }

    /// Adds a constant of type `ty` holding `value`, and returns its temp.
    fn constant(&mut self, ty: Ty, value: i128) -> u32 {
        let temp = self.temp(ty.ir);
        self.insts.push(constant(temp, ty.encode(value)));
        temp
    }

    /// Adds `dst = lhs OP rhs` at `loc` with a result of type `result`, and returns `dst`.
    fn binary(&mut self, op: BinOp, lhs: u32, rhs: u32, result: IrType, loc: Loc) -> u32 {
        let dst = self.temp(result);
        self.insts
            .push(binary_at(dst, op, lhs, rhs, loc.line, loc.col));
        dst
    }

    /// Adds a print of `src` as `kind`.
    fn print(&mut self, src: u32, kind: PrintKind) {
        self.insts.push(print_of(src, kind));
    }

    /// The module whose `main` runs everything added so far.
    fn finish(self) -> Module {
        main_only(self.temp_types, self.insts)
    }
}

/// Runs `module` and returns its stdout, stderr and exit code.
fn run(module: &Module) -> (String, String, i32) {
    run_module(module)
}

/// Asserts a clean run with exactly this stdout.
fn assert_prints(module: &Module, expected: &str) {
    let (stdout, stderr, code) = run(module);
    assert_eq!(stderr, "", "unexpected stderr");
    assert_eq!(code, 0, "unexpected exit code");
    assert_eq!(stdout, expected);
}

// ----------------------------------------------------------------------------------------------
// Test 13: arithmetic tables
// ----------------------------------------------------------------------------------------------

/// A module that prints `a OP b` for every pair that Rust accepts, and the text it must print.
fn table(ty: Ty, ty_index: u32, op: Arith, values: &[i128]) -> (Module, String) {
    let mut builder = Builder::new();
    let mut expected = String::new();
    let loc = location(ty_index, op);
    for &a in values {
        for &b in values {
            let Ok(result) = evaluate(ty, op, a, b) else {
                continue;
            };
            let lhs = builder.constant(ty, a);
            let rhs = builder.constant(ty, b);
            let dst = builder.binary(op.bin_op(ty.sign()), lhs, rhs, ty.ir, loc);
            builder.print(dst, ty.print_kind());
            expected.push_str(&format!("{result}\n"));
        }
    }
    (builder.finish(), expected)
}

/// Checks the boundary-value table of one type and operation.
fn check_table(ty: Ty, ty_index: u32, op: Arith) {
    let (module, expected) = table(ty, ty_index, op, &ty.boundaries());
    assert!(!expected.is_empty());
    assert_prints(&module, &expected);
}

// ----------------------------------------------------------------------------------------------
// Test 14: panics
// ----------------------------------------------------------------------------------------------

/// The first boundary pair, in table order, that Rust rejects; every type and operation has one.
fn first_rejected(ty: Ty, op: Arith) -> (i128, i128, Rejected) {
    let values = ty.boundaries();
    let rejected = values.iter().flat_map(|&a| {
        values
            .iter()
            .filter_map(move |&b| evaluate(ty, op, a, b).err().map(|reason| (a, b, reason)))
    });
    rejected.into_iter().next().unwrap()
}

/// Asserts that printing 1 and then computing `a OP b` panics with `reason` at the location the module gave.
fn assert_panics(ty: Ty, ty_index: u32, op: Arith, a: i128, b: i128, reason: Rejected) {
    let mut builder = Builder::new();
    let one = builder.temp(IrType::I64);
    builder.insts.push(constant(one, 1));
    builder.print(one, PrintKind::I64);
    let loc = location(ty_index, op);
    let lhs = builder.constant(ty, a);
    let rhs = builder.constant(ty, b);
    builder.binary(op.bin_op(ty.sign()), lhs, rhs, ty.ir, loc);
    let (stdout, stderr, code) = run(&builder.finish());
    let expected = format!(
        "panic: {} at {FILE}:{}:{}\n",
        reason.reason(),
        loc.line,
        loc.col
    );
    assert_eq!(stderr, expected, "{ty:?} {op:?} {a} {b}");
    assert_eq!(code, 101, "{ty:?} {op:?} {a} {b}");
    assert_eq!(stdout, "1\n", "{ty:?} {op:?} {a} {b}");
}

/// Checks the first rejected boundary pair of one type and operation.
fn check_panic(ty: Ty, ty_index: u32, op: Arith) {
    let (a, b, reason) = first_rejected(ty, op);
    assert_panics(ty, ty_index, op, a, b, reason);
}

/// Defines, for each listed type and operation, a table test and a panic test.
macro_rules! width_tests {
    ($( ($table:ident, $panic:ident, $ir:expr, $signed:expr, $index:expr, $op:expr) ),* $(,)?) => {
        $(
            /// Every boundary pair that Rust accepts prints Rust's result.
            #[test]
            fn $table() {
                check_table(Ty { ir: $ir, signed: $signed }, $index, $op);
            }

            /// The first boundary pair that Rust rejects panics with the exact message and location.
            #[test]
            fn $panic() {
                check_panic(Ty { ir: $ir, signed: $signed }, $index, $op);
            }
        )*
    };
}

width_tests! {
    (i8_add_table, i8_add_panics, IrType::I8, true, 0, Arith::Add),
    (i8_sub_table, i8_sub_panics, IrType::I8, true, 0, Arith::Sub),
    (i8_mul_table, i8_mul_panics, IrType::I8, true, 0, Arith::Mul),
    (i8_div_table, i8_div_panics, IrType::I8, true, 0, Arith::Div),
    (i8_rem_table, i8_rem_panics, IrType::I8, true, 0, Arith::Rem),
    (i16_add_table, i16_add_panics, IrType::I16, true, 1, Arith::Add),
    (i16_sub_table, i16_sub_panics, IrType::I16, true, 1, Arith::Sub),
    (i16_mul_table, i16_mul_panics, IrType::I16, true, 1, Arith::Mul),
    (i16_div_table, i16_div_panics, IrType::I16, true, 1, Arith::Div),
    (i16_rem_table, i16_rem_panics, IrType::I16, true, 1, Arith::Rem),
    (i32_add_table, i32_add_panics, IrType::I32, true, 2, Arith::Add),
    (i32_sub_table, i32_sub_panics, IrType::I32, true, 2, Arith::Sub),
    (i32_mul_table, i32_mul_panics, IrType::I32, true, 2, Arith::Mul),
    (i32_div_table, i32_div_panics, IrType::I32, true, 2, Arith::Div),
    (i32_rem_table, i32_rem_panics, IrType::I32, true, 2, Arith::Rem),
    (i64_add_table, i64_add_panics, IrType::I64, true, 3, Arith::Add),
    (i64_sub_table, i64_sub_panics, IrType::I64, true, 3, Arith::Sub),
    (i64_mul_table, i64_mul_panics, IrType::I64, true, 3, Arith::Mul),
    (i64_div_table, i64_div_panics, IrType::I64, true, 3, Arith::Div),
    (i64_rem_table, i64_rem_panics, IrType::I64, true, 3, Arith::Rem),
    (u8_add_table, u8_add_panics, IrType::I8, false, 4, Arith::Add),
    (u8_sub_table, u8_sub_panics, IrType::I8, false, 4, Arith::Sub),
    (u8_mul_table, u8_mul_panics, IrType::I8, false, 4, Arith::Mul),
    (u8_div_table, u8_div_panics, IrType::I8, false, 4, Arith::Div),
    (u8_rem_table, u8_rem_panics, IrType::I8, false, 4, Arith::Rem),
    (u16_add_table, u16_add_panics, IrType::I16, false, 5, Arith::Add),
    (u16_sub_table, u16_sub_panics, IrType::I16, false, 5, Arith::Sub),
    (u16_mul_table, u16_mul_panics, IrType::I16, false, 5, Arith::Mul),
    (u16_div_table, u16_div_panics, IrType::I16, false, 5, Arith::Div),
    (u16_rem_table, u16_rem_panics, IrType::I16, false, 5, Arith::Rem),
    (u32_add_table, u32_add_panics, IrType::I32, false, 6, Arith::Add),
    (u32_sub_table, u32_sub_panics, IrType::I32, false, 6, Arith::Sub),
    (u32_mul_table, u32_mul_panics, IrType::I32, false, 6, Arith::Mul),
    (u32_div_table, u32_div_panics, IrType::I32, false, 6, Arith::Div),
    (u32_rem_table, u32_rem_panics, IrType::I32, false, 6, Arith::Rem),
    (u64_add_table, u64_add_panics, IrType::I64, false, 7, Arith::Add),
    (u64_sub_table, u64_sub_panics, IrType::I64, false, 7, Arith::Sub),
    (u64_mul_table, u64_mul_panics, IrType::I64, false, 7, Arith::Mul),
    (u64_div_table, u64_div_panics, IrType::I64, false, 7, Arith::Div),
    (u64_rem_table, u64_rem_panics, IrType::I64, false, 7, Arith::Rem),
}

/// The reasons of the first rejected pair match Rust: division by zero when the divisor is 0, else overflow.
#[test]
fn first_rejected_pairs_name_the_right_reason() {
    for ty in [
        Ty {
            ir: IrType::I8,
            signed: true,
        },
        Ty {
            ir: IrType::I64,
            signed: false,
        },
    ] {
        for op in [Arith::Div, Arith::Rem] {
            let (a, b, reason) = first_rejected(ty, op);
            let expected = if b == 0 {
                Rejected::DivisionByZero
            } else {
                Rejected::Overflow
            };
            assert_eq!(reason, expected, "{ty:?} {op:?} {a} {b}");
        }
    }
}

/// `MIN / -1` and `MIN % -1` panic with an overflow at every signed width, even though the divisor is not 0.
#[test]
fn signed_min_divided_by_minus_one_overflows_at_every_width() {
    for (index, ir) in [IrType::I8, IrType::I16, IrType::I32, IrType::I64]
        .into_iter()
        .enumerate()
    {
        let ty = Ty { ir, signed: true };
        for op in [Arith::Div, Arith::Rem] {
            let index = u32::try_from(index).unwrap();
            assert_panics(ty, index, op, ty.min(), -1, Rejected::Overflow);
        }
    }
}

/// Dividing by zero panics at every width and signedness, for both `/` and `%`, even when the dividend is `MIN`.
#[test]
fn division_by_zero_panics_at_every_width_and_sign() {
    for (index, ir) in [IrType::I8, IrType::I16, IrType::I32, IrType::I64]
        .into_iter()
        .enumerate()
    {
        for signed in [true, false] {
            let ty = Ty { ir, signed };
            let index = u32::try_from(index).unwrap();
            for op in [Arith::Div, Arith::Rem] {
                assert_panics(ty, index, op, ty.min(), 0, Rejected::DivisionByZero);
                assert_panics(ty, index, op, ty.max(), 0, Rejected::DivisionByZero);
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Dense and exhaustive 8-bit coverage
// ----------------------------------------------------------------------------------------------

/// Every `step`th value (and the boundaries) of an 8-bit type as both operands: all accepted pairs print Rust's result; step 1 is exhaustive.
fn check_dense_8bit(signed: bool, op: Arith, step: usize) {
    let ty = Ty {
        ir: IrType::I8,
        signed,
    };
    let index = if signed { 0 } else { 4 };
    let (module, expected) = table(ty, index, op, &ty.dense(step));
    assert_prints(&module, &expected);
}

/// Every pair of `i8` values: all that Rust accepts print Rust's result of `add`.
#[test]
fn exhaustive_i8_add() {
    check_dense_8bit(true, Arith::Add, 1);
}

/// Every pair of `i8` values: all that Rust accepts print Rust's result of `sub`.
#[test]
fn exhaustive_i8_sub() {
    check_dense_8bit(true, Arith::Sub, 1);
}

/// Every pair of `i8` values: all that Rust accepts print Rust's result of `mul`.
#[test]
fn exhaustive_i8_mul() {
    check_dense_8bit(true, Arith::Mul, 1);
}

/// Dense `i8` div.
#[test]
fn dense_i8_div() {
    check_dense_8bit(true, Arith::Div, 2);
}

/// Dense `i8` rem.
#[test]
fn dense_i8_rem() {
    check_dense_8bit(true, Arith::Rem, 2);
}

/// Every pair of `u8` values: all that Rust accepts print Rust's result of `add`.
#[test]
fn exhaustive_u8_add() {
    check_dense_8bit(false, Arith::Add, 1);
}

/// Every pair of `u8` values: all that Rust accepts print Rust's result of `sub`.
#[test]
fn exhaustive_u8_sub() {
    check_dense_8bit(false, Arith::Sub, 1);
}

/// Every pair of `u8` values: all that Rust accepts print Rust's result of `mul`.
#[test]
fn exhaustive_u8_mul() {
    check_dense_8bit(false, Arith::Mul, 1);
}

/// Dense `u8` div.
#[test]
fn dense_u8_div() {
    check_dense_8bit(false, Arith::Div, 2);
}

/// Dense `u8` rem.
#[test]
fn dense_u8_rem() {
    check_dense_8bit(false, Arith::Rem, 2);
}

/// `count` pairs that Rust rejects, spread evenly over the whole range of an 8-bit type.
fn sampled_rejections(ty: Ty, op: Arith, count: usize) -> Vec<(i128, i128, Rejected)> {
    let values = ty.all();
    let mut rejected = Vec::new();
    for &a in &values {
        for &b in &values {
            if let Err(reason) = evaluate(ty, op, a, b) {
                rejected.push((a, b, reason));
            }
        }
    }
    (0..count)
        .map(|n| rejected[n * (rejected.len() - 1) / (count - 1)])
        .collect()
}

/// Checks that `count` rejected pairs of an 8-bit type each panic, with the right reason.
fn check_sampled_rejections(signed: bool, op: Arith, count: usize) {
    let ty = Ty {
        ir: IrType::I8,
        signed,
    };
    let index = if signed { 0 } else { 4 };
    for (a, b, reason) in sampled_rejections(ty, op, count) {
        assert_panics(ty, index, op, a, b, reason);
    }
}

/// Spread-out rejected `i8` additions panic.
#[test]
fn sampled_i8_add_rejections_panic() {
    check_sampled_rejections(true, Arith::Add, 4);
}

/// Spread-out rejected `i8` subtractions panic.
#[test]
fn sampled_i8_sub_rejections_panic() {
    check_sampled_rejections(true, Arith::Sub, 4);
}

/// Spread-out rejected `i8` multiplications panic.
#[test]
fn sampled_i8_mul_rejections_panic() {
    check_sampled_rejections(true, Arith::Mul, 4);
}

/// Spread-out rejected `i8` divisions panic.
#[test]
fn sampled_i8_div_rejections_panic() {
    check_sampled_rejections(true, Arith::Div, 4);
}

/// Spread-out rejected `i8` remainders panic.
#[test]
fn sampled_i8_rem_rejections_panic() {
    check_sampled_rejections(true, Arith::Rem, 4);
}

/// Spread-out rejected `u8` additions panic.
#[test]
fn sampled_u8_add_rejections_panic() {
    check_sampled_rejections(false, Arith::Add, 4);
}

/// Spread-out rejected `u8` subtractions panic.
#[test]
fn sampled_u8_sub_rejections_panic() {
    check_sampled_rejections(false, Arith::Sub, 4);
}

/// Spread-out rejected `u8` multiplications panic.
#[test]
fn sampled_u8_mul_rejections_panic() {
    check_sampled_rejections(false, Arith::Mul, 4);
}

/// Spread-out rejected `u8` divisions panic.
#[test]
fn sampled_u8_div_rejections_panic() {
    check_sampled_rejections(false, Arith::Div, 4);
}

/// Spread-out rejected `u8` remainders panic.
#[test]
fn sampled_u8_rem_rejections_panic() {
    check_sampled_rejections(false, Arith::Rem, 4);
}

// ----------------------------------------------------------------------------------------------
// Test 15: negation
// ----------------------------------------------------------------------------------------------

/// A signed type of the given width.
fn signed(ir: IrType) -> Ty {
    Ty { ir, signed: true }
}

/// A module that prints `-a` for each value of `values`, and the text it must print.
fn neg_table(ty: Ty, values: &[i128]) -> (Module, String) {
    let mut builder = Builder::new();
    let mut expected = String::new();
    for &value in values {
        let src = builder.constant(ty, value);
        let dst = builder.temp(ty.ir);
        builder.insts.push(Inst::Neg {
            dst: Temp(dst),
            src: Temp(src),
            loc: Loc { line: 5, col: 9 },
        });
        builder.print(dst, ty.print_kind());
        expected.push_str(&format!("{}\n", -value));
    }
    (builder.finish(), expected)
}

/// Checks the negations that fit: of `MIN+1`, -1, 0, 1 and `MAX`.
fn check_neg_table(ir: IrType) {
    let ty = signed(ir);
    let values = [ty.min() + 1, -1, 0, 1, ty.max()];
    let (module, expected) = neg_table(ty, &values);
    assert_prints(&module, &expected);
}

/// Checks that negating `MIN` panics with an overflow after printing 1.
fn check_neg_min_panics(ir: IrType) {
    let ty = signed(ir);
    let mut builder = Builder::new();
    let one = builder.temp(IrType::I64);
    builder.insts.push(constant(one, 1));
    builder.print(one, PrintKind::I64);
    let src = builder.constant(ty, ty.min());
    let dst = builder.temp(ty.ir);
    builder.insts.push(Inst::Neg {
        dst: Temp(dst),
        src: Temp(src),
        loc: Loc { line: 5, col: 9 },
    });
    let (stdout, stderr, code) = run(&builder.finish());
    assert_eq!(stderr, "panic: integer overflow at t.em:5:9\n");
    assert_eq!(code, 101);
    assert_eq!(stdout, "1\n");
}

/// `i8` negations of `MIN+1`, -1, 0, 1, `MAX`.
#[test]
fn i8_negations_print_rust_results() {
    check_neg_table(IrType::I8);
}

/// `i16` negations of `MIN+1`, -1, 0, 1, `MAX`.
#[test]
fn i16_negations_print_rust_results() {
    check_neg_table(IrType::I16);
}

/// `i32` negations of `MIN+1`, -1, 0, 1, `MAX`.
#[test]
fn i32_negations_print_rust_results() {
    check_neg_table(IrType::I32);
}

/// `i64` negations of `MIN+1`, -1, 0, 1, `MAX`.
#[test]
fn i64_negations_print_rust_results() {
    check_neg_table(IrType::I64);
}

/// Negating `i8::MIN` panics.
#[test]
fn i8_negating_min_panics() {
    check_neg_min_panics(IrType::I8);
}

/// Negating `i16::MIN` panics.
#[test]
fn i16_negating_min_panics() {
    check_neg_min_panics(IrType::I16);
}

/// Negating `i32::MIN` panics.
#[test]
fn i32_negating_min_panics() {
    check_neg_min_panics(IrType::I32);
}

/// Negating `i64::MIN` panics.
#[test]
fn i64_negating_min_panics() {
    check_neg_min_panics(IrType::I64);
}

/// Every `i8` value negates correctly except `MIN`: all 255 results print (the spec's exhaustive `neg_i8`).
#[test]
fn exhaustive_i8_neg() {
    let ty = signed(IrType::I8);
    let values: Vec<i128> = ty.all().into_iter().filter(|v| *v != ty.min()).collect();
    let (module, expected) = neg_table(ty, &values);
    assert_prints(&module, &expected);
}

/// Negating every `i8` value but `MIN`'s neighbour pair matches Rust: negation twice is the identity.
#[test]
fn i8_negation_twice_is_the_identity() {
    let ty = signed(IrType::I8);
    let mut builder = Builder::new();
    let mut expected = String::new();
    for value in ty.all().into_iter().filter(|v| *v != ty.min()) {
        let src = builder.constant(ty, value);
        let once = builder.temp(ty.ir);
        let loc = Loc { line: 5, col: 9 };
        builder.insts.push(Inst::Neg {
            dst: Temp(once),
            src: Temp(src),
            loc,
        });
        let twice = builder.temp(ty.ir);
        builder.insts.push(Inst::Neg {
            dst: Temp(twice),
            src: Temp(once),
            loc,
        });
        builder.print(twice, ty.print_kind());
        expected.push_str(&format!("{value}\n"));
    }
    assert_prints(&builder.finish(), &expected);
}

// ----------------------------------------------------------------------------------------------
// Test 16: comparisons
// ----------------------------------------------------------------------------------------------

/// A comparison operation with the Rust function that computes it on two values.
type Comparison = (BinOp, fn(i128, i128) -> bool);

/// The six comparisons of the language.
fn comparisons(sign: Sign) -> Vec<Comparison> {
    vec![
        (BinOp::Eq, |a, b| a == b),
        (BinOp::Ne, |a, b| a != b),
        (BinOp::Lt(sign), |a, b| a < b),
        (BinOp::Le(sign), |a, b| a <= b),
        (BinOp::Gt(sign), |a, b| a > b),
        (BinOp::Ge(sign), |a, b| a >= b),
    ]
}

/// Checks every comparison of every boundary pair of a type against Rust, printing each as a bool.
fn check_comparisons(ty: Ty) {
    let mut builder = Builder::new();
    let mut expected = String::new();
    let values = ty.boundaries();
    for &a in &values {
        for &b in &values {
            for (op, rust) in comparisons(ty.sign()) {
                let lhs = builder.constant(ty, a);
                let rhs = builder.constant(ty, b);
                let loc = Loc { line: 1, col: 1 };
                let dst = builder.binary(op, lhs, rhs, IrType::I8, loc);
                builder.print(dst, PrintKind::Bool);
                expected.push_str(if rust(a, b) { "true\n" } else { "false\n" });
            }
        }
    }
    assert_prints(&builder.finish(), &expected);
}

/// `i8` comparisons.
#[test]
fn i8_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I8,
        signed: true,
    });
}

/// `i16` comparisons.
#[test]
fn i16_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I16,
        signed: true,
    });
}

/// `i32` comparisons.
#[test]
fn i32_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I32,
        signed: true,
    });
}

/// `i64` comparisons.
#[test]
fn i64_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I64,
        signed: true,
    });
}

/// `u8` comparisons.
#[test]
fn u8_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I8,
        signed: false,
    });
}

/// `u16` comparisons.
#[test]
fn u16_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I16,
        signed: false,
    });
}

/// `u32` comparisons.
#[test]
fn u32_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I32,
        signed: false,
    });
}

/// `u64` comparisons.
#[test]
fn u64_comparisons_match_rust() {
    check_comparisons(Ty {
        ir: IrType::I64,
        signed: false,
    });
}

// ----------------------------------------------------------------------------------------------
// Test 17: narrow memory round trips, and far slots
// ----------------------------------------------------------------------------------------------

/// A one-slot-per-value `main` that stores each `(type, value)` into its slot, loads it back and prints it.
fn round_trip(cases: &[(Ty, i128)]) -> (Module, String) {
    let slots: Vec<ir::Slot> = cases
        .iter()
        .enumerate()
        .map(|(n, (ty, _))| slot_of(&format!("s{n}"), ty.ir))
        .collect();
    let mut builder = Builder::new();
    let mut expected = String::new();
    for (n, (ty, value)) in cases.iter().enumerate() {
        let slot = u32::try_from(n).unwrap();
        let source = builder.constant(*ty, *value);
        builder.insts.push(store_of(slot, source));
        let loaded = builder.temp(ty.ir);
        builder.insts.push(load_of(loaded, slot, ty.sign()));
        builder.print(loaded, ty.print_kind());
        expected.push_str(&format!("{value}\n"));
    }
    let main = single_block_function(
        "main",
        vec![],
        None,
        slots,
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![main],
    };
    (module, expected)
}

/// The spec's round trips: `i8` -1, `u8` 255, `i16` -32768, `u16` 65535, `i32` -1, `u32` 4294967295.
#[test]
fn narrow_values_survive_a_slot_round_trip() {
    let cases = [
        (
            Ty {
                ir: IrType::I8,
                signed: true,
            },
            -1,
        ),
        (
            Ty {
                ir: IrType::I8,
                signed: false,
            },
            255,
        ),
        (
            Ty {
                ir: IrType::I16,
                signed: true,
            },
            -32768,
        ),
        (
            Ty {
                ir: IrType::I16,
                signed: false,
            },
            65535,
        ),
        (
            Ty {
                ir: IrType::I32,
                signed: true,
            },
            -1,
        ),
        (
            Ty {
                ir: IrType::I32,
                signed: false,
            },
            4294967295,
        ),
    ];
    let (module, expected) = round_trip(&cases);
    assert_prints(&module, &expected);
}

/// Every boundary value of every type survives a slot round trip.
#[test]
fn every_boundary_value_survives_a_slot_round_trip() {
    let mut cases = Vec::new();
    for ir in [IrType::I8, IrType::I16, IrType::I32, IrType::I64] {
        for signed in [true, false] {
            let ty = Ty { ir, signed };
            cases.extend(ty.boundaries().into_iter().map(|value| (ty, value)));
        }
    }
    let (module, expected) = round_trip(&cases);
    assert_prints(&module, &expected);
}

/// A function with `slot_count` slots of `ty` that stores `value` into the last slot and prints it back.
fn far_round_trip(ty: Ty, slot_count: u32, value: i128) -> (Module, String) {
    let slots = (0..slot_count)
        .map(|n| slot_of(&format!("s{n}"), ty.ir))
        .collect();
    let last = slot_count - 1;
    let mut builder = Builder::new();
    let source = builder.constant(ty, value);
    builder.insts.push(store_of(last, source));
    let loaded = builder.temp(ty.ir);
    builder.insts.push(load_of(loaded, last, ty.sign()));
    builder.print(loaded, ty.print_kind());
    let main = single_block_function(
        "main",
        vec![],
        None,
        slots,
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![main],
    };
    (module, format!("{value}\n"))
}

/// A byte slot beyond the 4095-byte immediate range holds its value.
#[test]
fn far_byte_slot_round_trips() {
    for (signed, value) in [(true, -7), (false, 250)] {
        let (module, expected) = far_round_trip(
            Ty {
                ir: IrType::I8,
                signed,
            },
            600,
            value,
        );
        assert_prints(&module, &expected);
    }
}

/// A half-word slot beyond the 8190-byte immediate range holds its value.
#[test]
fn far_half_slot_round_trips() {
    for (signed, value) in [(true, -300), (false, 60000)] {
        let (module, expected) = far_round_trip(
            Ty {
                ir: IrType::I16,
                signed,
            },
            1100,
            value,
        );
        assert_prints(&module, &expected);
    }
}

/// A word slot beyond the 16380-byte immediate range holds its value.
#[test]
fn far_word_slot_round_trips() {
    for (signed, value) in [(true, -70000), (false, 4_000_000_000)] {
        let (module, expected) = far_round_trip(
            Ty {
                ir: IrType::I32,
                signed,
            },
            2100,
            value,
        );
        assert_prints(&module, &expected);
    }
}

// ----------------------------------------------------------------------------------------------
// print.u64
// ----------------------------------------------------------------------------------------------

/// `print.u64` of an `i64` shows it as unsigned: `u64::MAX`, 2^63 and 0.
#[test]
fn print_u64_shows_the_unsigned_value() {
    let ty = Ty {
        ir: IrType::I64,
        signed: false,
    };
    let values = [0, 1, i128::from(i64::MAX), 1i128 << 63, ty.max()];
    let mut builder = Builder::new();
    let mut expected = String::new();
    for value in values {
        let temp = builder.constant(ty, value);
        builder.print(temp, PrintKind::U64);
        expected.push_str(&format!("{value}\n"));
    }
    assert_prints(&builder.finish(), &expected);
}

/// `print.i64` of the same bit patterns shows them signed, so the two kinds differ for the top bit.
#[test]
fn print_i64_of_the_top_bit_is_negative() {
    let ty = Ty {
        ir: IrType::I64,
        signed: false,
    };
    let mut builder = Builder::new();
    let temp = builder.constant(ty, ty.max());
    builder.print(temp, PrintKind::I64);
    assert_prints(&builder.finish(), "-1\n");
}

/// A narrow value is extended by its print kind: `u8` 255 as unsigned is 255, and `i8` -1 as signed is -1.
#[test]
fn narrow_print_kinds_extend_by_sign() {
    let mut builder = Builder::new();
    let unsigned = builder.constant(
        Ty {
            ir: IrType::I8,
            signed: false,
        },
        255,
    );
    builder.print(unsigned, PrintKind::U64);
    let signed_value = builder.constant(
        Ty {
            ir: IrType::I8,
            signed: true,
        },
        -1,
    );
    builder.print(signed_value, PrintKind::I64);
    let wide = builder.constant(
        Ty {
            ir: IrType::I32,
            signed: false,
        },
        i128::from(u32::MAX),
    );
    builder.print(wide, PrintKind::U64);
    assert_prints(&builder.finish(), "255\n-1\n4294967295\n");
}

// ----------------------------------------------------------------------------------------------
// Tests 10 and 18: stack arguments, and calls with narrow values
// ----------------------------------------------------------------------------------------------

/// The six stack parameters of the spec's example, with their types and the values test 18 passes.
fn stack_values() -> [(Ty, i128); 6] {
    [
        (
            Ty {
                ir: IrType::I8,
                signed: true,
            },
            -5,
        ),
        (
            Ty {
                ir: IrType::I8,
                signed: false,
            },
            200,
        ),
        (
            Ty {
                ir: IrType::I16,
                signed: true,
            },
            -300,
        ),
        (
            Ty {
                ir: IrType::I32,
                signed: true,
            },
            -70000,
        ),
        (
            Ty {
                ir: IrType::I16,
                signed: false,
            },
            60000,
        ),
        (
            Ty {
                ir: IrType::I64,
                signed: true,
            },
            1i128 << 40,
        ),
    ]
}

/// The 14 parameters of the example: eight `i64` and the six stack parameters.
fn stack_params() -> Vec<(Ty, i128)> {
    let mut params: Vec<(Ty, i128)> = (1..=8)
        .map(|n| {
            (
                Ty {
                    ir: IrType::I64,
                    signed: true,
                },
                n,
            )
        })
        .collect();
    params.extend(stack_values());
    params
}

/// The callee `f`: its 14 parameters are slots; `body` runs over them.
fn stack_callee(
    name: &str,
    ret: Option<IrType>,
    temp_types: Vec<IrType>,
    insts: Vec<Inst>,
    term: Terminator,
) -> Function {
    let params = stack_params();
    let param_list = params
        .iter()
        .enumerate()
        .map(|(n, (ty, _))| param_of(u32::try_from(n).unwrap(), ty.ir, ty.sign()))
        .collect();
    let slots = params
        .iter()
        .enumerate()
        .map(|(n, (ty, _))| slot_of(&format!("p{n}"), ty.ir))
        .collect();
    single_block_function(name, param_list, ret, slots, temp_types, insts, term)
}

/// A caller that builds the 14 argument constants and calls `callee`; a result, if any, is printed as `kind`.
fn stack_caller(callee: &str, result: Option<(Ty, PrintKind)>) -> Function {
    let mut builder = Builder::new();
    let args: Vec<u32> = stack_params()
        .iter()
        .map(|(ty, value)| builder.constant(*ty, *value))
        .collect();
    let dst = result.map(|(ty, _)| builder.temp(ty.ir));
    builder.insts.push(Inst::Call {
        dst: dst.map(Temp),
        func: callee.to_string(),
        args: args.into_iter().map(Temp).collect(),
    });
    if let (Some(dst), Some((_, kind))) = (dst, result) {
        builder.print(dst, kind);
    }
    single_block_function(
        "main",
        vec![],
        None,
        vec![],
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    )
}

/// The callee prints each stack parameter: -5, 200, -300, -70000, 60000 and 1099511627776 (test 18).
#[test]
fn stack_parameters_of_every_width_arrive_intact() {
    let mut builder = Builder::new();
    for (n, (ty, _)) in stack_params().iter().enumerate().skip(8) {
        let loaded = builder.temp(ty.ir);
        builder
            .insts
            .push(load_of(loaded, u32::try_from(n).unwrap(), ty.sign()));
        builder.print(loaded, ty.print_kind());
    }
    let callee = stack_callee(
        "f",
        None,
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![callee, stack_caller("f", None)],
    };
    assert_prints(&module, "-5\n200\n-300\n-70000\n60000\n1099511627776\n");
}

/// The register parameters of the same call are still right: the callee prints the first and the last.
#[test]
fn register_parameters_next_to_stack_parameters_arrive_intact() {
    let ty = Ty {
        ir: IrType::I64,
        signed: true,
    };
    let mut builder = Builder::new();
    for slot in [0, 7] {
        let loaded = builder.temp(ty.ir);
        builder.insts.push(load_of(loaded, slot, ty.sign()));
        builder.print(loaded, ty.print_kind());
    }
    let callee = stack_callee(
        "f",
        None,
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![callee, stack_caller("f", None)],
    };
    assert_prints(&module, "1\n8\n");
}

/// Four recursive-looking calls with narrow stack arguments in different frames: each frame has its own outgoing area.
#[test]
fn a_callee_can_forward_its_narrow_stack_parameters() {
    // `g` has the 14 parameters of the example and calls `f` with its own parameters unchanged.
    let params = stack_params();
    let mut builder = Builder::new();
    let loaded: Vec<u32> = params
        .iter()
        .enumerate()
        .map(|(n, (ty, _))| {
            let temp = builder.temp(ty.ir);
            builder
                .insts
                .push(load_of(temp, u32::try_from(n).unwrap(), ty.sign()));
            temp
        })
        .collect();
    builder.insts.push(Inst::Call {
        dst: None,
        func: "f".to_string(),
        args: loaded.into_iter().map(Temp).collect(),
    });
    let forwarder = stack_callee(
        "g",
        None,
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    );

    let mut printing = Builder::new();
    for (n, (ty, _)) in params.iter().enumerate().skip(8) {
        let temp = printing.temp(ty.ir);
        printing
            .insts
            .push(load_of(temp, u32::try_from(n).unwrap(), ty.sign()));
        printing.print(temp, ty.print_kind());
    }
    let printer = stack_callee(
        "f",
        None,
        printing.temp_types,
        printing.insts,
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![printer, forwarder, stack_caller("g", None)],
    };
    assert_prints(&module, "-5\n200\n-300\n-70000\n60000\n1099511627776\n");
}

/// A function `NAME(p: ty) -> ty` that returns its parameter unchanged.
fn echo_function(name: &str, ty: Ty) -> Function {
    single_block_function(
        name,
        vec![param_of(0, ty.ir, ty.sign())],
        Some(ty.ir),
        vec![slot_of("p", ty.ir)],
        vec![ty.ir],
        vec![load_of(0, 0, ty.sign())],
        Terminator::Return(Some(Temp(0))),
    )
}

/// The eight types with the extreme value each is echoed with.
fn echo_cases() -> Vec<(Ty, i128)> {
    let mut cases = Vec::new();
    for ir in [IrType::I8, IrType::I16, IrType::I32, IrType::I64] {
        for signed in [true, false] {
            let ty = Ty { ir, signed };
            let value = if signed { ty.min() } else { ty.max() };
            cases.push((ty, value));
        }
    }
    cases
}

/// A narrow argument passed in a register and a narrow result come back unchanged, at every width and sign.
#[test]
fn narrow_register_arguments_and_results_survive_a_call() {
    let cases = echo_cases();
    let mut functions: Vec<Function> = Vec::new();
    let mut builder = Builder::new();
    let mut expected = String::new();
    for (n, (ty, value)) in cases.iter().enumerate() {
        let name = format!("echo{n}");
        functions.push(echo_function(&name, *ty));
        let arg = builder.constant(*ty, *value);
        let result = builder.temp(ty.ir);
        builder.insts.push(Inst::Call {
            dst: Some(Temp(result)),
            func: name,
            args: vec![Temp(arg)],
        });
        builder.print(result, ty.print_kind());
        expected.push_str(&format!("{value}\n"));
    }
    let main = single_block_function(
        "main",
        vec![],
        None,
        vec![],
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    );
    functions.push(main);
    assert_prints(&Module { functions }, &expected);
}

// ----------------------------------------------------------------------------------------------
// Test 19: the ABI against C
// ----------------------------------------------------------------------------------------------

/// The module the C harness calls: `pick_a` to `pick_g` (each returns one stack parameter), `echo0` to `echo7`, and an empty `main`.
fn abi_module() -> Module {
    let mut functions = Vec::new();
    let params = stack_params();
    let names = ["pick_a", "pick_b", "pick_c", "pick_d", "pick_e", "pick_g"];
    for (index, name) in names.iter().enumerate() {
        let slot = u32::try_from(8 + index).unwrap();
        let (ty, _) = params[8 + index];
        functions.push(stack_callee(
            name,
            Some(ty.ir),
            vec![ty.ir],
            vec![load_of(0, slot, ty.sign())],
            Terminator::Return(Some(Temp(0))),
        ));
    }
    for (n, (ty, _)) in echo_cases().iter().enumerate() {
        functions.push(echo_function(&format!("echo{n}"), *ty));
    }
    module_with_main(functions)
}

/// A C caller that passes `int8_t`, `uint8_t`, `int16_t`, `int32_t`, `uint16_t` and `int64_t` stack arguments, and the extreme echo values, gets them all back.
#[test]
fn c_and_ember_agree_on_stack_packing_and_narrow_values() {
    let (stdout, stderr, code) = run_module_in_abi_harness(&abi_module());
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
}

// ----------------------------------------------------------------------------------------------
// Relaxation sees the size of a far narrow access
// ----------------------------------------------------------------------------------------------

/// A `main` that skips (by a true branch) a block of 100,000 stores to a far byte slot, over 1 MiB of code.
///
/// Each store prints three instructions: the load of the temp, the `movz x17` and the `strb`. A size table that
/// counted two would put the branch target in range and the assembler would reject the `cbnz`.
#[test]
fn branch_over_far_narrow_stores_is_relaxed() {
    let slots: Vec<ir::Slot> = (0..600)
        .map(|n| slot_of(&format!("s{n}"), IrType::I8))
        .collect();
    let flag = constant(0, 1);
    let branch = Terminator::Branch {
        cond: Temp(0),
        then_to: ir::BlockId(2),
        else_to: ir::BlockId(1),
    };
    let stores: Vec<Inst> = (0..100_000).map(|_| store_of(599, 0)).collect();
    let blocks = vec![
        block_of(0, vec![flag], branch),
        block_of(1, stores, Terminator::Return(None)),
        block_of(
            2,
            vec![print_of(0, PrintKind::Bool)],
            Terminator::Return(None),
        ),
    ];
    let main = typed_function("main", vec![], None, slots, vec![IrType::I8], blocks);
    let module = Module {
        functions: vec![main],
    };
    assert!(emit_ok(&module, FILE).contains("cbz x8, Lmain_skip"));
    assert_prints(&module, "true\n");
}

/// The same branch but with unreachable filler blocks still prints (a control for the relaxation helper).
#[test]
fn branch_over_unreachable_blocks_is_unaffected_by_widths() {
    let flag = constant(0, 1);
    let branch = Terminator::Branch {
        cond: Temp(0),
        then_to: ir::BlockId(263_000),
        else_to: ir::BlockId(1),
    };
    let mut blocks = vec![block_of(0, vec![flag], branch)];
    blocks.extend(unreachable_blocks(1, 262_999));
    blocks.push(block_of(
        263_000,
        vec![print_of(0, PrintKind::Bool)],
        Terminator::Return(None),
    ));
    let main = typed_function("main", vec![], None, vec![], vec![IrType::I8], blocks);
    assert_prints(
        &Module {
            functions: vec![main],
        },
        "true\n",
    );
}

// ----------------------------------------------------------------------------------------------
// A bool beside its neighbours on the stack
// ----------------------------------------------------------------------------------------------

/// A callee `f(p0..p7: i64, flag: bool, next: i64)` that prints `p7`, `flag` and `next`.
fn bool_printing_callee() -> Function {
    let mut params: Vec<_> = (0..8)
        .map(|n| param_of(n, IrType::I64, Sign::Signed))
        .collect();
    params.push(param_of(8, IrType::I8, Sign::Unsigned));
    params.push(param_of(9, IrType::I64, Sign::Signed));
    let mut slots: Vec<_> = (0..8)
        .map(|n| slot_of(&format!("p{n}"), IrType::I64))
        .collect();
    slots.push(slot_of("flag", IrType::I8));
    slots.push(slot_of("next", IrType::I64));
    let insts = vec![
        load_of(0, 7, Sign::Signed),
        print_of(0, PrintKind::I64),
        load_of(1, 8, Sign::Unsigned),
        print_of(1, PrintKind::Bool),
        load_of(2, 9, Sign::Signed),
        print_of(2, PrintKind::I64),
    ];
    single_block_function(
        "f",
        params,
        None,
        slots,
        vec![IrType::I64, IrType::I8, IrType::I64],
        insts,
        Terminator::Return(None),
    )
}

/// A `main` that calls `f` twice: with `(true, 1234)` and with `(false, 5678)`, the first eight arguments being 1 to 8.
fn bool_calling_main() -> Function {
    let mut builder = Builder::new();
    for (flag, next) in [(1, 1234), (0, 5678)] {
        let mut args: Vec<u32> = (1..=8)
            .map(|n| {
                builder.constant(
                    Ty {
                        ir: IrType::I64,
                        signed: true,
                    },
                    n,
                )
            })
            .collect();
        let flag_temp = builder.temp(IrType::I8);
        builder.insts.push(constant(flag_temp, flag));
        args.push(flag_temp);
        args.push(builder.constant(
            Ty {
                ir: IrType::I64,
                signed: true,
            },
            next,
        ));
        builder.insts.push(Inst::Call {
            dst: None,
            func: "f".to_string(),
            args: args.into_iter().map(Temp).collect(),
        });
    }
    single_block_function(
        "main",
        vec![],
        None,
        vec![],
        builder.temp_types,
        builder.insts,
        Terminator::Return(None),
    )
}

/// `true` and `false` arrive intact on the stack, packed as one byte, with their `i64` neighbours unharmed.
#[test]
fn bool_stack_parameter_arrives_beside_its_neighbours() {
    let module = Module {
        functions: vec![bool_printing_callee(), bool_calling_main()],
    };
    assert_prints(&module, "8\ntrue\n1234\n8\nfalse\n5678\n");
}
