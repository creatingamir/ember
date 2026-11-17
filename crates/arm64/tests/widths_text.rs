//! Text-level tests for integer widths: the exact instructions `emit` selects for every IR type and sign.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{
    FILE, assert_contains, assert_lacks, assert_run, binary_at, block_of, constant, emit_ok,
    function_text, load_of, module_with_main, param_of, print_of, single_block_function, slot_of,
    store_of, trimmed_lines, typed_function,
};
use ir::{BinOp, Function, Inst, IrType, Loc, Module, PrintKind, Sign, Temp, Terminator};

/// The four IR types, narrowest first.
const WIDTHS: [IrType; 4] = [IrType::I8, IrType::I16, IrType::I32, IrType::I64];

/// The register letter an operation of this width uses: `w` for 8, 16 and 32 bits, `x` for 64.
fn reg(ty: IrType) -> &'static str {
    if ty == IrType::I64 { "x" } else { "w" }
}

/// `fn f(a: ty, b: ty) -> ty { return a OP b }` with the loads of the given sign; the result is `i8` for a comparison.
fn binary_function(ty: IrType, sign: Sign, op: BinOp, result: IrType) -> Function {
    let insts = vec![
        load_of(0, 0, sign),
        load_of(1, 1, sign),
        binary_at(2, op, 0, 1, 2, 12),
    ];
    single_block_function(
        "f",
        vec![param_of(0, ty, sign), param_of(1, ty, sign)],
        Some(result),
        vec![slot_of("a", ty), slot_of("b", ty)],
        vec![ty, ty, result],
        insts,
        Terminator::Return(Some(Temp(2))),
    )
}

/// The body of `_em_f` for `a OP b` on `ty`, where `make` builds the operation from its sign.
fn f_text(ty: IrType, sign: Sign, make: fn(Sign) -> BinOp) -> String {
    let module = module_with_main(vec![binary_function(ty, sign, make(sign), ty)]);
    function_text(&emit_ok(&module, FILE), "_em_f")
}

/// The body of `_em_f` for a comparison of two `ty` values, which gives an `i8`.
fn compare_text(ty: IrType, sign: Sign, op: BinOp) -> String {
    let module = module_with_main(vec![binary_function(ty, sign, op, IrType::I8)]);
    function_text(&emit_ok(&module, FILE), "_em_f")
}

/// The body of `_em_f` for `-a` on `ty` (one parameter, cells: slot 0, temps at 8 and 16).
fn neg_text(ty: IrType) -> String {
    let neg = Inst::Neg {
        dst: Temp(1),
        src: Temp(0),
        loc: Loc { line: 2, col: 12 },
    };
    let function = single_block_function(
        "f",
        vec![param_of(0, ty, Sign::Signed)],
        Some(ty),
        vec![slot_of("a", ty)],
        vec![ty, ty],
        vec![load_of(0, 0, Sign::Signed), neg],
        Terminator::Return(Some(Temp(1))),
    );
    let module = module_with_main(vec![function]);
    function_text(&emit_ok(&module, FILE), "_em_f")
}

/// The body of `main` for a module whose `main` holds `insts` over temps of the given types.
fn main_text(slots: Vec<ir::Slot>, temp_types: Vec<IrType>, insts: Vec<Inst>) -> String {
    let function = single_block_function(
        "main",
        vec![],
        None,
        slots,
        temp_types,
        insts,
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![function],
    };
    function_text(&emit_ok(&module, FILE), "_em_main")
}

/// The immediate that `tst` uses to find bits above an unsigned 8- or 16-bit result.
fn high_bits(ty: IrType) -> &'static str {
    match ty {
        IrType::I8 => "0xffffff00",
        IrType::I16 => "0xffff0000",
        _ => "",
    }
}

/// The sign-extension that checks a narrow signed result.
fn extension(ty: IrType) -> &'static str {
    match ty {
        IrType::I8 => "sxtb",
        IrType::I16 => "sxth",
        _ => "sxtw",
    }
}

// ----------------------------------------------------------------------------------------------
// Parameters and the exact `i8` add of the spec
// ----------------------------------------------------------------------------------------------

/// The spec's `i8` add: byte stores of the parameters, sign-extending loads, `add w`, extension check, panic branch.
#[test]
fn i8_add_is_exact() {
    let text = f_text(IrType::I8, Sign::Signed, BinOp::Add);
    assert_run(
        &text,
        &[
            "strb w0, [sp, #0]",
            "strb w1, [sp, #8]",
            "Lf_bb0:",
            "ldrsb w8, [sp, #0]",
            "str x8, [sp, #16]",
            "ldrsb w8, [sp, #8]",
            "str x8, [sp, #24]",
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "add w8, w8, w9",
            "str x8, [sp, #32]",
            "ldr x8, [sp, #32]",
            "cmp w8, w8, sxtb",
            "b.ne Lf_panic0",
            "ldr x0, [sp, #32]",
        ],
    );
}

/// The frame of that function has five cells: 40 bytes, rounded to 48.
#[test]
fn i8_add_frame_is_48_bytes() {
    let text = f_text(IrType::I8, Sign::Signed, BinOp::Add);
    assert_contains(&text, "sub sp, sp, #48");
}

/// Register parameters are stored with the width of their type: `strb`, `strh`, `str w`, `str x`.
#[test]
fn register_parameters_are_stored_with_their_width() {
    let tys = [IrType::I8, IrType::I16, IrType::I32, IrType::I64];
    let params = (0..4).map(|n| param_of(n, tys[n as usize], Sign::Signed));
    let slots = (0..4).map(|n| slot_of(&format!("p{n}"), tys[n as usize]));
    let function = single_block_function(
        "f",
        params.collect(),
        None,
        slots.collect(),
        vec![],
        vec![],
        Terminator::Return(None),
    );
    let text = emit_ok(&module_with_main(vec![function]), FILE);
    assert_run(
        &text,
        &[
            "strb w0, [sp, #0]",
            "strh w1, [sp, #8]",
            "str w2, [sp, #16]",
            "str x3, [sp, #24]",
        ],
    );
}

/// The result of a narrow function is returned as it sits in its cell: no extension before the epilogue.
#[test]
fn narrow_result_is_returned_without_extension() {
    let text = f_text(IrType::I8, Sign::Unsigned, BinOp::Add);
    assert_run(
        &text,
        &[
            "ldr x0, [sp, #32]",
            "mov sp, x29",
            "ldp x29, x30, [sp], #16",
        ],
    );
}

// ----------------------------------------------------------------------------------------------
// Constants
// ----------------------------------------------------------------------------------------------

/// The lines that build `value` of type `ty` in a one-temp `main`, then store it in cell 0.
fn const_text(ty: IrType, value: i64) -> String {
    main_text(vec![], vec![ty], vec![constant(0, value)])
}

/// An `i8` of -1 is built as its 32-bit extension: `0xffffffff`.
#[test]
fn i8_minus_one_is_built_as_a_32_bit_extension() {
    let text = const_text(IrType::I8, -1);
    assert_run(
        &text,
        &[
            "movz w8, #65535",
            "movk w8, #65535, lsl #16",
            "str x8, [sp, #0]",
        ],
    );
}

/// A `u8` of 255 needs one `movz`.
#[test]
fn u8_255_is_one_movz() {
    let text = const_text(IrType::I8, 255);
    assert_run(&text, &["movz w8, #255", "str x8, [sp, #0]"]);
}

/// An `i16` of -32768 is `0xffff8000`.
#[test]
fn i16_min_is_built_as_0xffff8000() {
    let text = const_text(IrType::I16, -32768);
    assert_run(&text, &["movz w8, #32768", "movk w8, #65535, lsl #16"]);
}

/// A `u16` of 65535 is one `movz`.
#[test]
fn u16_max_is_one_movz() {
    let text = const_text(IrType::I16, 65535);
    assert_run(&text, &["movz w8, #65535", "str x8, [sp, #0]"]);
}

/// An `i32` of `i32::MIN` is `0x80000000`: a zero low chunk and `movk #32768, lsl #16`.
#[test]
fn i32_min_is_built_in_two_chunks() {
    let text = const_text(IrType::I32, i64::from(i32::MIN));
    assert_run(&text, &["movz w8, #0", "movk w8, #32768, lsl #16"]);
}

/// A `u32` of `u32::MAX` is two full chunks.
#[test]
fn u32_max_is_two_full_chunks() {
    let text = const_text(IrType::I32, i64::from(u32::MAX));
    assert_run(&text, &["movz w8, #65535", "movk w8, #65535, lsl #16"]);
}

/// A small `i32` is one `movz w`.
#[test]
fn small_i32_is_one_movz() {
    let text = const_text(IrType::I32, 5);
    assert_run(&text, &["movz w8, #5", "str x8, [sp, #0]"]);
}

/// An `i64` of -1 keeps its four chunks in `x`.
#[test]
fn i64_minus_one_is_four_chunks_in_x() {
    let text = const_text(IrType::I64, -1);
    assert_run(
        &text,
        &[
            "movz x8, #65535",
            "movk x8, #65535, lsl #16",
            "movk x8, #65535, lsl #32",
            "movk x8, #65535, lsl #48",
        ],
    );
}

/// A narrow constant never uses an `x` register.
#[test]
fn narrow_constants_do_not_use_x8_for_building() {
    let text = const_text(IrType::I16, -2);
    assert_lacks(&text, "movz x8");
    assert_lacks(&text, "movk x8");
}

// ----------------------------------------------------------------------------------------------
// Add and subtract
// ----------------------------------------------------------------------------------------------

/// Signed add: `adds` and `b.vs` for 32 and 64 bits (store after the branch); `add w`, an extension compare and `b.ne` for 8 and 16.
#[test]
fn signed_add_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Signed, BinOp::Add);
        match ty {
            IrType::I32 | IrType::I64 => assert_run(
                &text,
                &[
                    &format!("adds {r}8, {r}8, {r}9"),
                    "b.vs Lf_panic0",
                    "str x8, [sp, #32]",
                ],
            ),
            _ => assert_run(
                &text,
                &[
                    "add w8, w8, w9",
                    "str x8, [sp, #32]",
                    "ldr x8, [sp, #32]",
                    &format!("cmp w8, w8, {}", extension(ty)),
                    "b.ne Lf_panic0",
                ],
            ),
        }
    }
}

/// Unsigned add: `adds` and `b.hs` for 32 and 64 bits; `add w`, `tst` of the high bits and `b.ne` for 8 and 16.
#[test]
fn unsigned_add_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Unsigned, BinOp::Add);
        match ty {
            IrType::I32 | IrType::I64 => assert_run(
                &text,
                &[
                    &format!("adds {r}8, {r}8, {r}9"),
                    "b.hs Lf_panic0",
                    "str x8, [sp, #32]",
                ],
            ),
            _ => assert_run(
                &text,
                &[
                    "add w8, w8, w9",
                    "str x8, [sp, #32]",
                    "ldr x8, [sp, #32]",
                    &format!("tst w8, #{}", high_bits(ty)),
                    "b.ne Lf_panic0",
                ],
            ),
        }
    }
}

/// Signed subtract: like signed add with `subs`/`sub`.
#[test]
fn signed_sub_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Signed, BinOp::Sub);
        match ty {
            IrType::I32 | IrType::I64 => assert_run(
                &text,
                &[
                    &format!("subs {r}8, {r}8, {r}9"),
                    "b.vs Lf_panic0",
                    "str x8, [sp, #32]",
                ],
            ),
            _ => assert_run(
                &text,
                &[
                    "sub w8, w8, w9",
                    "str x8, [sp, #32]",
                    "ldr x8, [sp, #32]",
                    &format!("cmp w8, w8, {}", extension(ty)),
                    "b.ne Lf_panic0",
                ],
            ),
        }
    }
}

/// Unsigned subtract: `subs` and `b.lo` (borrow) for 32 and 64 bits; `sub w`, `tst` and `b.ne` for 8 and 16.
#[test]
fn unsigned_sub_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Unsigned, BinOp::Sub);
        match ty {
            IrType::I32 | IrType::I64 => assert_run(
                &text,
                &[
                    &format!("subs {r}8, {r}8, {r}9"),
                    "b.lo Lf_panic0",
                    "str x8, [sp, #32]",
                ],
            ),
            _ => assert_run(
                &text,
                &[
                    "sub w8, w8, w9",
                    "str x8, [sp, #32]",
                    "ldr x8, [sp, #32]",
                    &format!("tst w8, #{}", high_bits(ty)),
                    "b.ne Lf_panic0",
                ],
            ),
        }
    }
}

/// An unsigned 64-bit add: `adds x8, x8, x9`, `b.hs`, then the store (the spec's second text test).
#[test]
fn u64_add_is_adds_then_hs_then_store() {
    let text = f_text(IrType::I64, Sign::Unsigned, BinOp::Add);
    assert_run(
        &text,
        &["adds x8, x8, x9", "b.hs Lf_panic0", "str x8, [sp, #32]"],
    );
}

/// An unsigned 64-bit subtract: `subs x8, x8, x9`, then `b.lo` (the spec's third text test).
#[test]
fn u64_sub_is_subs_then_lo() {
    let text = f_text(IrType::I64, Sign::Unsigned, BinOp::Sub);
    assert_run(&text, &["subs x8, x8, x9", "b.lo Lf_panic0"]);
}

// ----------------------------------------------------------------------------------------------
// Multiply
// ----------------------------------------------------------------------------------------------

/// A signed 64-bit multiply is Phase 1's `mul`, `smulh`, `cmp ..., asr #63`, `b.ne`.
#[test]
fn signed_mul_i64_keeps_smulh() {
    let text = f_text(IrType::I64, Sign::Signed, BinOp::Mul);
    assert_run(
        &text,
        &[
            "mul x8, x8, x9",
            "str x8, [sp, #32]",
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "smulh x8, x8, x9",
            "str x8, [sp, #40]",
            "ldr x8, [sp, #40]",
            "ldr x9, [sp, #32]",
            "cmp x8, x9, asr #63",
            "b.ne Lf_panic0",
        ],
    );
}

/// A signed 32-bit multiply widens with `smull` and compares the product with its own `sxtw`.
#[test]
fn signed_mul_i32_uses_smull_and_sxtw_compare() {
    let text = f_text(IrType::I32, Sign::Signed, BinOp::Mul);
    assert_run(
        &text,
        &[
            "smull x8, w8, w9",
            "str x8, [sp, #32]",
            "ldr x8, [sp, #32]",
            "cmp x8, w8, sxtw",
            "b.ne Lf_panic0",
        ],
    );
}

/// A signed 8- or 16-bit multiply is `mul w` and a compare with the sign extension of the low part.
#[test]
fn signed_mul_narrow_uses_mul_w_and_extension_compare() {
    for ty in [IrType::I8, IrType::I16] {
        let text = f_text(ty, Sign::Signed, BinOp::Mul);
        assert_run(
            &text,
            &[
                "mul w8, w8, w9",
                "str x8, [sp, #32]",
                "ldr x8, [sp, #32]",
                &format!("cmp w8, w8, {}", extension(ty)),
                "b.ne Lf_panic0",
            ],
        );
    }
}

/// An unsigned 64-bit multiply is `mul`, `umulh` and a compare of the high half with zero.
#[test]
fn unsigned_mul_i64_uses_umulh() {
    let text = f_text(IrType::I64, Sign::Unsigned, BinOp::Mul);
    assert_run(
        &text,
        &[
            "mul x8, x8, x9",
            "str x8, [sp, #32]",
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "umulh x8, x8, x9",
            "str x8, [sp, #40]",
            "ldr x8, [sp, #40]",
            "cmp x8, #0",
            "b.ne Lf_panic0",
        ],
    );
}

/// An unsigned 32-bit multiply widens with `umull` and tests the upper 32 bits (the spec's fourth text test).
#[test]
fn unsigned_mul_i32_uses_umull_and_tst() {
    let text = f_text(IrType::I32, Sign::Unsigned, BinOp::Mul);
    assert_run(
        &text,
        &[
            "umull x8, w8, w9",
            "str x8, [sp, #32]",
            "ldr x8, [sp, #32]",
            "tst x8, #0xffffffff00000000",
            "b.ne Lf_panic0",
        ],
    );
}

/// An unsigned 8- or 16-bit multiply is `mul w` and a test of the bits above the type.
#[test]
fn unsigned_mul_narrow_uses_mul_w_and_tst() {
    for ty in [IrType::I8, IrType::I16] {
        let text = f_text(ty, Sign::Unsigned, BinOp::Mul);
        assert_run(
            &text,
            &[
                "mul w8, w8, w9",
                "str x8, [sp, #32]",
                "ldr x8, [sp, #32]",
                &format!("tst w8, #{}", high_bits(ty)),
                "b.ne Lf_panic0",
            ],
        );
    }
}

// ----------------------------------------------------------------------------------------------
// Negate
// ----------------------------------------------------------------------------------------------

/// A 64-bit negate is `negs x` then `b.vs`, with the store after the branch.
#[test]
fn neg_i64_is_negs_then_vs() {
    let text = neg_text(IrType::I64);
    assert_run(
        &text,
        &["negs x8, x8", "b.vs Lf_panic0", "str x8, [sp, #16]"],
    );
}

/// A 32-bit negate is `negs w` then `b.vs`.
#[test]
fn neg_i32_is_negs_w_then_vs() {
    let text = neg_text(IrType::I32);
    assert_run(
        &text,
        &["negs w8, w8", "b.vs Lf_panic0", "str x8, [sp, #16]"],
    );
}

/// An 8- or 16-bit negate is `neg w`, then the extension compare and `b.ne`.
#[test]
fn neg_narrow_is_neg_then_extension_compare() {
    for ty in [IrType::I8, IrType::I16] {
        let text = neg_text(ty);
        assert_run(
            &text,
            &[
                "neg w8, w8",
                "str x8, [sp, #16]",
                "ldr x8, [sp, #16]",
                &format!("cmp w8, w8, {}", extension(ty)),
                "b.ne Lf_panic0",
            ],
        );
    }
}

// ----------------------------------------------------------------------------------------------
// Divide and remainder
// ----------------------------------------------------------------------------------------------

/// The lines that build the `MIN` of a width as an extended 32- or 64-bit pattern.
fn min_pattern(ty: IrType) -> [&'static str; 2] {
    match ty {
        IrType::I8 => ["movz w8, #65408", "movk w8, #65535, lsl #16"],
        IrType::I16 => ["movz w8, #32768", "movk w8, #65535, lsl #16"],
        IrType::I32 => ["movz w8, #0", "movk w8, #32768, lsl #16"],
        IrType::I64 => ["movz x8, #0", "movk x8, #32768, lsl #48"],
    }
}

/// Signed division at every width: zero check first, then `-1` check, `MIN` of the width, `sdiv` of the width.
#[test]
fn signed_div_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Signed, BinOp::Div);
        let [low, high] = min_pattern(ty);
        assert_run(
            &text,
            &[
                "ldr x8, [sp, #24]",
                &format!("cmp {r}8, #0"),
                "b.eq Lf_panic0",
                "ldr x8, [sp, #24]",
                &format!("cmn {r}8, #1"),
                "b.ne Lf_ok1",
                low,
                high,
                "str x8, [sp, #40]",
                "ldr x8, [sp, #16]",
                "ldr x9, [sp, #40]",
                &format!("cmp {r}8, {r}9"),
                "b.eq Lf_panic2",
                "Lf_ok1:",
                "ldr x8, [sp, #16]",
                "ldr x9, [sp, #24]",
                &format!("sdiv {r}8, {r}8, {r}9"),
                "str x8, [sp, #32]",
            ],
        );
    }
}

/// An `i16` signed division builds `MIN` as `0xffff8000` (the spec's sixth text test).
#[test]
fn i16_signed_div_builds_min_as_0xffff8000() {
    let text = f_text(IrType::I16, Sign::Signed, BinOp::Div);
    assert_run(&text, &["movz w8, #32768", "movk w8, #65535, lsl #16"]);
    assert_contains(&text, "sdiv w8, w8, w9");
}

/// Signed remainder at every width: the division sequence, then `msub` of the width from the quotient cell.
#[test]
fn signed_rem_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Signed, BinOp::Rem);
        assert_run(
            &text,
            &[
                &format!("sdiv {r}8, {r}8, {r}9"),
                "str x8, [sp, #48]",
                "ldr x8, [sp, #48]",
                "ldr x9, [sp, #24]",
                "ldr x10, [sp, #16]",
                &format!("msub {r}8, {r}8, {r}9, {r}10"),
                "str x8, [sp, #32]",
            ],
        );
    }
}

/// Unsigned division at every width: a zero check and `udiv`; no `-1` check and no `ok` label.
#[test]
fn unsigned_div_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Unsigned, BinOp::Div);
        assert_run(
            &text,
            &[
                "ldr x8, [sp, #24]",
                &format!("cmp {r}8, #0"),
                "b.eq Lf_panic0",
                "ldr x8, [sp, #16]",
                "ldr x9, [sp, #24]",
                &format!("udiv {r}8, {r}8, {r}9"),
                "str x8, [sp, #32]",
            ],
        );
        assert_lacks(&text, "cmn");
        assert_lacks(&text, "Lf_ok");
    }
}

/// A `u8` division is `udiv w8, w8, w9` with no `cmn` (the spec's seventh text test).
#[test]
fn u8_div_is_udiv_w_without_cmn() {
    let text = f_text(IrType::I8, Sign::Unsigned, BinOp::Div);
    assert_contains(&text, "udiv w8, w8, w9");
    assert_lacks(&text, "cmn");
}

/// Unsigned remainder at every width: `udiv` into a quotient cell, then `msub` (the spec's eighth text test).
#[test]
fn unsigned_rem_forms() {
    for ty in WIDTHS {
        let r = reg(ty);
        let text = f_text(ty, Sign::Unsigned, BinOp::Rem);
        assert_run(
            &text,
            &[
                &format!("udiv {r}8, {r}8, {r}9"),
                "str x8, [sp, #40]",
                "ldr x8, [sp, #40]",
                "ldr x9, [sp, #24]",
                "ldr x10, [sp, #16]",
                &format!("msub {r}8, {r}8, {r}9, {r}10"),
                "str x8, [sp, #32]",
            ],
        );
    }
}

/// An unsigned remainder has only a zero check and so only one panic label.
#[test]
fn unsigned_rem_has_one_panic_path() {
    let text = f_text(IrType::I16, Sign::Unsigned, BinOp::Rem);
    assert_contains(&text, "Lf_panic0:");
    assert_lacks(&text, "Lf_panic1");
    assert_lacks(&text, "Lf_panic2");
}

// ----------------------------------------------------------------------------------------------
// Comparisons
// ----------------------------------------------------------------------------------------------

/// Every comparison compares with the width of its operands and sets the result with `cset x`.
#[test]
fn comparison_forms() {
    let signed = [
        (BinOp::Lt(Sign::Signed), "lt"),
        (BinOp::Le(Sign::Signed), "le"),
        (BinOp::Gt(Sign::Signed), "gt"),
        (BinOp::Ge(Sign::Signed), "ge"),
    ];
    let unsigned = [
        (BinOp::Lt(Sign::Unsigned), "lo"),
        (BinOp::Le(Sign::Unsigned), "ls"),
        (BinOp::Gt(Sign::Unsigned), "hi"),
        (BinOp::Ge(Sign::Unsigned), "hs"),
    ];
    for ty in WIDTHS {
        let r = reg(ty);
        let cases = signed
            .iter()
            .map(|(op, cond)| (*op, Sign::Signed, *cond))
            .chain(
                unsigned
                    .iter()
                    .map(|(op, cond)| (*op, Sign::Unsigned, *cond)),
            )
            .chain([
                (BinOp::Eq, Sign::Signed, "eq"),
                (BinOp::Ne, Sign::Unsigned, "ne"),
            ]);
        for (op, sign, cond) in cases {
            let text = compare_text(ty, sign, op);
            assert_run(
                &text,
                &[
                    "ldr x8, [sp, #16]",
                    "ldr x9, [sp, #24]",
                    &format!("cmp {r}8, {r}9"),
                    &format!("cset x8, {cond}"),
                    "str x8, [sp, #32]",
                ],
            );
        }
    }
}

/// `u32` ordering comparisons give `lo`, `ls`, `hi`, `hs` (the spec's ninth text test).
#[test]
fn u32_compares_use_unsigned_conditions() {
    for (op, cond) in [
        (BinOp::Lt(Sign::Unsigned), "lo"),
        (BinOp::Le(Sign::Unsigned), "ls"),
        (BinOp::Gt(Sign::Unsigned), "hi"),
        (BinOp::Ge(Sign::Unsigned), "hs"),
    ] {
        let text = compare_text(IrType::I32, Sign::Unsigned, op);
        assert_run(&text, &["cmp w8, w9", &format!("cset x8, {cond}")]);
    }
}

/// Signed `i32` compares keep the signed conditions.
#[test]
fn i32_compares_use_signed_conditions() {
    let text = compare_text(IrType::I32, Sign::Signed, BinOp::Lt(Sign::Signed));
    assert_run(&text, &["cmp w8, w9", "cset x8, lt"]);
}

// ----------------------------------------------------------------------------------------------
// Bools stay 64 bits wide
// ----------------------------------------------------------------------------------------------

/// `!` on a bool flips the low bit of the whole `x` register.
#[test]
fn not_on_a_bool_uses_the_x_register() {
    let not = Inst::Not {
        dst: Temp(1),
        src: Temp(0),
    };
    let text = main_text(
        vec![],
        vec![IrType::I8, IrType::I8],
        vec![constant(0, 1), not],
    );
    assert_run(&text, &["ldr x8, [sp, #0]", "eor x8, x8, #1"]);
}

/// A branch on a bool tests the whole `x` register.
#[test]
fn branch_on_a_bool_uses_cbnz_x() {
    let branch = Terminator::Branch {
        cond: Temp(0),
        then_to: ir::BlockId(1),
        else_to: ir::BlockId(2),
    };
    let blocks = vec![
        block_of(0, vec![constant(0, 1)], branch),
        block_of(1, vec![], Terminator::Return(None)),
        block_of(2, vec![], Terminator::Return(None)),
    ];
    let function = typed_function("main", vec![], None, vec![], vec![IrType::I8], blocks);
    let text = emit_ok(
        &Module {
            functions: vec![function],
        },
        FILE,
    );
    assert_contains(&text, "cbnz x8, Lmain_bb1");
}

// ----------------------------------------------------------------------------------------------
// Sized loads and stores
// ----------------------------------------------------------------------------------------------

/// A one-slot `main` that stores a constant of `ty` into the slot and loads it back with `sign`.
fn slot_round_trip(ty: IrType, sign: Sign) -> String {
    let insts = vec![constant(0, 1), store_of(0, 0), load_of(1, 0, sign)];
    main_text(vec![slot_of("v", ty)], vec![ty, ty], insts)
}

/// Each width's store and load use their own mnemonic and register: the slot is cell 0, the temps are cells 1 and 2.
#[test]
fn memory_access_mnemonics() {
    let cases = [
        (
            IrType::I8,
            Sign::Signed,
            "strb w8, [sp, #0]",
            "ldrsb w8, [sp, #0]",
        ),
        (
            IrType::I8,
            Sign::Unsigned,
            "strb w8, [sp, #0]",
            "ldrb w8, [sp, #0]",
        ),
        (
            IrType::I16,
            Sign::Signed,
            "strh w8, [sp, #0]",
            "ldrsh w8, [sp, #0]",
        ),
        (
            IrType::I16,
            Sign::Unsigned,
            "strh w8, [sp, #0]",
            "ldrh w8, [sp, #0]",
        ),
        (
            IrType::I32,
            Sign::Signed,
            "str w8, [sp, #0]",
            "ldr w8, [sp, #0]",
        ),
        (
            IrType::I32,
            Sign::Unsigned,
            "str w8, [sp, #0]",
            "ldr w8, [sp, #0]",
        ),
        (
            IrType::I64,
            Sign::Signed,
            "str x8, [sp, #0]",
            "ldr x8, [sp, #0]",
        ),
    ];
    for (ty, sign, store, load) in cases {
        let text = slot_round_trip(ty, sign);
        assert_run(&text, &["ldr x8, [sp, #8]", store]);
        assert_run(&text, &[load, "str x8, [sp, #16]"]);
    }
}

/// A function with `count` slots of type `ty` that stores a constant into `slot` and loads it back.
fn far_module(ty: IrType, count: u32, slot: u32, sign: Sign) -> String {
    let slots = (0..count).map(|n| slot_of(&format!("s{n}"), ty)).collect();
    let insts = vec![constant(0, 1), store_of(slot, 0), load_of(1, slot, sign)];
    main_text(slots, vec![ty, ty], insts)
}

/// The access kinds with the first slot whose offset fits the immediate and the first that does not.
///
/// The immediate limit is 4095 bytes times the access size: 4095, 8190, 16380.
const OFFSET_LIMITS: [(IrType, u32, u32, &str, &str, &str, u64); 3] = [
    // (type, last short slot, first far slot, store mnemonic, signed load, unsigned load, far offset)
    (IrType::I8, 511, 512, "strb", "ldrsb", "ldrb", 4096),
    (IrType::I16, 1023, 1024, "strh", "ldrsh", "ldrh", 8192),
    (IrType::I32, 2047, 2048, "str", "ldr", "ldr", 16384),
];

/// The last slot whose byte offset is within the immediate range keeps the immediate form.
#[test]
fn offsets_within_the_limit_use_the_immediate() {
    for (ty, short, _, store, signed_load, unsigned_load, _) in OFFSET_LIMITS {
        let offset = u64::from(short) * 8;
        let text = far_module(ty, short + 1, short, Sign::Signed);
        assert_contains(&text, &format!("{store} w8, [sp, #{offset}]"));
        assert_contains(&text, &format!("{signed_load} w8, [sp, #{offset}]"));
        assert_lacks(&text, "x17");
        let text = far_module(ty, short + 1, short, Sign::Unsigned);
        assert_contains(&text, &format!("{unsigned_load} w8, [sp, #{offset}]"));
    }
}

/// The first slot beyond the immediate range builds its offset in `x17`.
#[test]
fn offsets_beyond_the_limit_go_through_x17() {
    for (ty, _, far, store, signed_load, unsigned_load, offset) in OFFSET_LIMITS {
        let text = far_module(ty, far + 1, far, Sign::Signed);
        assert_run(
            &text,
            &[
                &format!("movz x17, #{offset}"),
                &format!("{store} w8, [sp, x17]"),
            ],
        );
        assert_run(
            &text,
            &[
                &format!("movz x17, #{offset}"),
                &format!("{signed_load} w8, [sp, x17]"),
            ],
        );
        let text = far_module(ty, far + 1, far, Sign::Unsigned);
        assert_run(
            &text,
            &[
                &format!("movz x17, #{offset}"),
                &format!("{unsigned_load} w8, [sp, x17]"),
            ],
        );
    }
}

/// The spec's example: an `i8` stored to slot 599 (offset 4792) needs `x17`; slot 500 (offset 4000) does not.
#[test]
fn byte_store_at_4792_is_far_and_at_4000_is_near() {
    let far = far_module(IrType::I8, 600, 599, Sign::Signed);
    assert_run(&far, &["movz x17, #4792", "strb w8, [sp, x17]"]);
    let near = far_module(IrType::I8, 600, 500, Sign::Signed);
    assert_contains(&near, "strb w8, [sp, #4000]");
}

/// A 64-bit access keeps its 32760-byte limit: slot 4095 is an immediate, slot 4096 is far.
#[test]
fn double_access_keeps_its_limit() {
    let near = far_module(IrType::I64, 4096, 4095, Sign::Signed);
    assert_contains(&near, "str x8, [sp, #32760]");
    let far = far_module(IrType::I64, 4097, 4096, Sign::Signed);
    assert_run(&far, &["movz x17, #32768", "str x8, [sp, x17]"]);
}

// ----------------------------------------------------------------------------------------------
// Stack arguments
// ----------------------------------------------------------------------------------------------

/// The parameters of the spec's example: eight `i64`, then `i8`, `u8`, `i16`, `i32`, `u16` and `i64`.
fn stack_parameter_kinds() -> Vec<(IrType, Sign)> {
    let mut kinds = vec![(IrType::I64, Sign::Signed); 8];
    kinds.extend([
        (IrType::I8, Sign::Signed),
        (IrType::I8, Sign::Unsigned),
        (IrType::I16, Sign::Signed),
        (IrType::I32, Sign::Signed),
        (IrType::I16, Sign::Unsigned),
        (IrType::I64, Sign::Signed),
    ]);
    kinds
}

/// The callee `f` of the example with an empty body, so its cells are the 14 slots and then one cell per stack parameter.
fn stack_callee() -> Function {
    let kinds = stack_parameter_kinds();
    let params = kinds
        .iter()
        .enumerate()
        .map(|(n, (ty, sign))| param_of(n as u32, *ty, *sign))
        .collect();
    let slots = kinds
        .iter()
        .enumerate()
        .map(|(n, (ty, _))| slot_of(&format!("p{n}"), *ty))
        .collect();
    single_block_function(
        "f",
        params,
        None,
        slots,
        vec![],
        vec![],
        Terminator::Return(None),
    )
}

/// A `main` that makes 14 constants (one per parameter, of its type) and calls `f` with them.
fn stack_caller() -> Function {
    let kinds = stack_parameter_kinds();
    let mut insts: Vec<Inst> = (0..14).map(|n| constant(n, i64::from(n))).collect();
    insts.push(Inst::Call {
        dst: None,
        func: "f".to_string(),
        args: (0..14).map(Temp).collect(),
    });
    let temp_types = kinds.iter().map(|(ty, _)| *ty).collect();
    single_block_function(
        "main",
        vec![],
        None,
        vec![],
        temp_types,
        insts,
        Terminator::Return(None),
    )
}

/// The assembly of the stack-argument example.
fn stack_asm() -> String {
    let module = Module {
        functions: vec![stack_callee(), stack_caller()],
    };
    emit_ok(&module, FILE)
}

/// The caller writes the six stack arguments packed at 0, 1, 2, 4, 8 and 16, each with its width, before the register arguments.
#[test]
fn caller_packs_stack_arguments_by_size() {
    let text = function_text(&stack_asm(), "_em_main");
    // The outgoing area is 32 bytes, so temp `n` is at `32 + 8n`: the stack arguments are temps 8 to 13.
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #96]",
            "strb w8, [sp, #0]",
            "ldr x8, [sp, #104]",
            "strb w8, [sp, #1]",
            "ldr x8, [sp, #112]",
            "strh w8, [sp, #2]",
            "ldr x8, [sp, #120]",
            "str w8, [sp, #4]",
            "ldr x8, [sp, #128]",
            "strh w8, [sp, #8]",
            "ldr x8, [sp, #136]",
            "str x8, [sp, #16]",
            "ldr x0, [sp, #32]",
        ],
    );
}

/// The largest end offset is 24, so the outgoing area is 32 bytes: 14 cells (112) plus 32.
#[test]
fn outgoing_area_is_the_packed_size_rounded_to_16() {
    let text = function_text(&stack_asm(), "_em_main");
    assert_contains(&text, "sub sp, sp, #144");
}

/// The callee reads each stack parameter with the load for its type and sign at `[x29, #16 + offset]`.
#[test]
fn callee_reads_stack_parameters_with_their_width() {
    let text = function_text(&stack_asm(), "_em_f");
    // Slots are cells 0 to 13; the six stack values go through cells 14 to 19.
    let expected = [
        ("ldrsb w8, [x29, #16]", 112, "strb w8, [sp, #64]"),
        ("ldrb w8, [x29, #17]", 120, "strb w8, [sp, #72]"),
        ("ldrsh w8, [x29, #18]", 128, "strh w8, [sp, #80]"),
        ("ldr w8, [x29, #20]", 136, "str w8, [sp, #88]"),
        ("ldrh w8, [x29, #24]", 144, "strh w8, [sp, #96]"),
        ("ldr x8, [x29, #32]", 152, "str x8, [sp, #104]"),
    ];
    for (read, cell, store) in expected {
        assert_run(
            &text,
            &[
                read,
                &format!("str x8, [sp, #{cell}]"),
                &format!("ldr x8, [sp, #{cell}]"),
                store,
            ],
        );
    }
}

/// The register parameters of that callee are stored from `x0` to `x7` before any stack parameter is read.
#[test]
fn callee_stores_register_parameters_first() {
    let text = function_text(&stack_asm(), "_em_f");
    assert_run(&text, &["str x7, [sp, #56]", "ldrsb w8, [x29, #16]"]);
}

/// An `i8` stack argument followed by an `i64` is padded: the `i64` goes at offset 8.
#[test]
fn stack_argument_is_aligned_to_its_own_size() {
    let mut kinds = vec![(IrType::I64, Sign::Signed); 8];
    kinds.extend([(IrType::I8, Sign::Signed), (IrType::I64, Sign::Signed)]);
    let params = kinds
        .iter()
        .enumerate()
        .map(|(n, (ty, sign))| param_of(n as u32, *ty, *sign))
        .collect();
    let slots = kinds
        .iter()
        .enumerate()
        .map(|(n, (ty, _))| slot_of(&format!("p{n}"), *ty))
        .collect();
    let callee = single_block_function(
        "f",
        params,
        None,
        slots,
        vec![],
        vec![],
        Terminator::Return(None),
    );
    let text = function_text(&emit_ok(&module_with_main(vec![callee]), FILE), "_em_f");
    assert_contains(&text, "ldrsb w8, [x29, #16]");
    assert_contains(&text, "ldr x8, [x29, #24]");
}

// ----------------------------------------------------------------------------------------------
// Prints
// ----------------------------------------------------------------------------------------------

/// A one-temp `main` that prints a constant of `ty` as `kind`.
fn print_text(ty: IrType, kind: PrintKind) -> String {
    main_text(vec![], vec![ty], vec![constant(0, 1), print_of(0, kind)])
}

/// Printing a narrow value as signed sign-extends it to 64 bits before the call.
#[test]
fn narrow_signed_print_sign_extends() {
    for ty in [IrType::I8, IrType::I16, IrType::I32] {
        let text = print_text(ty, PrintKind::I64);
        assert_run(
            &text,
            &["ldr x0, [sp, #0]", "sxtw x0, w0", "bl _ember_rt_print_i64"],
        );
    }
}

/// Printing an `i64` as signed needs no extension.
#[test]
fn i64_signed_print_has_no_extension() {
    let text = print_text(IrType::I64, PrintKind::I64);
    assert_run(&text, &["ldr x0, [sp, #0]", "bl _ember_rt_print_i64"]);
}

/// Printing a narrow value as unsigned zero-extends it with `mov w0, w0` and uses the signed helper.
#[test]
fn narrow_unsigned_print_zero_extends() {
    for ty in [IrType::I8, IrType::I16, IrType::I32] {
        let text = print_text(ty, PrintKind::U64);
        assert_run(
            &text,
            &["ldr x0, [sp, #0]", "mov w0, w0", "bl _ember_rt_print_i64"],
        );
    }
}

/// Printing an `i64` as unsigned calls the new helper.
#[test]
fn u64_print_calls_the_unsigned_helper() {
    let text = print_text(IrType::I64, PrintKind::U64);
    assert_run(&text, &["ldr x0, [sp, #0]", "bl _ember_rt_print_u64"]);
    assert_lacks(&text, "print_i64");
}

/// Printing a bool needs no extension.
#[test]
fn bool_print_has_no_extension() {
    let text = print_text(IrType::I8, PrintKind::Bool);
    assert_run(&text, &["ldr x0, [sp, #0]", "bl _ember_rt_print_bool"]);
}

/// The program has one local, non-global `_ember_rt_print_u64`, even when it never prints.
#[test]
fn u64_helper_is_emitted_once_and_is_local() {
    let text = emit_ok(&module_with_main(vec![]), FILE);
    assert_eq!(
        trimmed_lines(&text)
            .iter()
            .filter(|l| **l == "_ember_rt_print_u64:")
            .count(),
        1
    );
    assert_lacks(&text, ".globl _ember_rt_print_u64");
}

/// The unsigned helper is the signed one with its own format string.
#[test]
fn u64_helper_is_the_signed_helper_with_the_unsigned_format() {
    let text = emit_ok(&module_with_main(vec![]), FILE);
    let signed = function_text(&text, "_ember_rt_print_i64");
    let unsigned = function_text(&text, "_ember_rt_print_u64");
    assert_eq!(unsigned, signed.replace("Lstr_fmt_i64", "Lstr_fmt_u64"));
}

/// The unsigned format string is `%llu\n`, defined once.
#[test]
fn u64_format_string_is_percent_llu() {
    let text = emit_ok(&module_with_main(vec![]), FILE);
    assert_run(&text, &["Lstr_fmt_u64:", ".asciz \"%llu\\n\""]);
    assert_eq!(
        trimmed_lines(&text)
            .iter()
            .filter(|l| **l == "Lstr_fmt_u64:")
            .count(),
        1
    );
}

// ----------------------------------------------------------------------------------------------
// Determinism and internal errors
// ----------------------------------------------------------------------------------------------

/// The same module prints the same assembly twice.
#[test]
fn width_output_is_deterministic() {
    let module = module_with_main(vec![binary_function(
        IrType::I8,
        Sign::Signed,
        BinOp::Add(Sign::Signed),
        IrType::I8,
    )]);
    assert_eq!(emit_ok(&module, FILE), emit_ok(&module, FILE));
}

/// A temp without an entry in `temp_types` is an internal compiler error, not a panic.
#[test]
fn temp_without_a_type_is_an_internal_error() {
    let function = single_block_function(
        "main",
        vec![],
        None,
        vec![],
        vec![],
        vec![constant(0, 1)],
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![function],
    };
    let error = arm64::emit(&module, FILE).unwrap_err();
    assert_eq!(error.message, "internal compiler error: a temp has no type");
}

/// A parameter that names a slot the function does not have is an internal compiler error with the spec's message.
#[test]
fn parameter_without_a_slot_is_an_internal_error() {
    let function = single_block_function(
        "main",
        vec![param_of(3, IrType::I64, Sign::Signed)],
        None,
        vec![slot_of("a", IrType::I64)],
        vec![],
        vec![],
        Terminator::Return(None),
    );
    let module = Module {
        functions: vec![function],
    };
    let error = arm64::emit(&module, FILE).unwrap_err();
    assert_eq!(
        error.message,
        "internal compiler error: a parameter does not exist"
    );
}

// ----------------------------------------------------------------------------------------------
// A bool on the stack is one byte (clang packs `_Bool` at [sp] and the next `int64_t` at [sp, #8])
// ----------------------------------------------------------------------------------------------

/// The callee `f(p0..p7: i64, flag: bool, next: i64)` with an empty body.
fn bool_callee() -> Function {
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
    single_block_function(
        "f",
        params,
        None,
        slots,
        vec![],
        vec![],
        Terminator::Return(None),
    )
}

/// A `main` that calls `f` with ten constants: the eighth to ninth are an `i8` bool and an `i64`.
fn bool_caller() -> Function {
    let mut temp_types = vec![IrType::I64; 8];
    temp_types.extend([IrType::I8, IrType::I64]);
    let mut insts: Vec<Inst> = (0..10).map(|n| constant(n, i64::from(n))).collect();
    insts.push(Inst::Call {
        dst: None,
        func: "f".to_string(),
        args: (0..10).map(Temp).collect(),
    });
    single_block_function(
        "main",
        vec![],
        None,
        vec![],
        temp_types,
        insts,
        Terminator::Return(None),
    )
}

/// The assembly of the bool example.
fn bool_asm() -> String {
    let module = Module {
        functions: vec![bool_callee(), bool_caller()],
    };
    emit_ok(&module, FILE)
}

/// The caller stores the bool with `strb` at offset 0 and the `i64` after it at offset 8 (8-byte alignment).
#[test]
fn bool_stack_parameter_is_passed_as_one_byte() {
    let text = function_text(&bool_asm(), "_em_main");
    // The outgoing area is 16 bytes, so temp `n` is at `16 + 8n`: the bool is temp 8, the `i64` temp 9.
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #80]",
            "strb w8, [sp, #0]",
            "ldr x8, [sp, #88]",
            "str x8, [sp, #8]",
        ],
    );
}

/// The callee reads the bool with `ldrb` at `[x29, #16]` and the next `i64` at `[x29, #24]`.
#[test]
fn callee_reads_a_bool_stack_parameter_as_one_byte() {
    let text = function_text(&bool_asm(), "_em_f");
    // Slots are cells 0 to 9; the two stack values go through cells 10 and 11.
    assert_run(
        &text,
        &[
            "ldrb w8, [x29, #16]",
            "str x8, [sp, #80]",
            "ldr x8, [sp, #80]",
            "strb w8, [sp, #64]",
            "ldr x8, [x29, #24]",
            "str x8, [sp, #88]",
            "ldr x8, [sp, #88]",
            "str x8, [sp, #72]",
        ],
    );
}
