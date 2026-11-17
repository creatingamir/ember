//! Text-level tests for the bitwise operations, shifts and conversions: the exact instructions `emit` selects.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{
    FILE, assert_contains, assert_lacks, assert_run, binary_at, emit_ok, function_text, load_of,
    module_with_main, param_of, single_block_function, slot_of, trimmed_lines,
};
use ir::{BinOp, Function, Inst, IrType, Loc, Sign, Temp, Terminator};

/// An integer type: an IR width and a signedness.
type Ty = (IrType, Sign);

/// The four IR widths, narrowest first.
const WIDTHS: [IrType; 4] = [IrType::I8, IrType::I16, IrType::I32, IrType::I64];

/// Both signs.
const SIGNS: [Sign; 2] = [Sign::Signed, Sign::Unsigned];

/// The register letter an operation of this width uses: `w` for 8, 16 and 32 bits, `x` for 64.
fn reg(ty: IrType) -> &'static str {
    if ty == IrType::I64 { "x" } else { "w" }
}

/// `fn f(a: value, b: amount) -> value { return a OP b }`, with the operation at line 2, column 12.
fn binary_function(value: Ty, amount: Ty, op: BinOp) -> Function {
    let insts = vec![
        load_of(0, 0, value.1),
        load_of(1, 1, amount.1),
        binary_at(2, op, 0, 1, 2, 12),
    ];
    single_block_function(
        "f",
        vec![
            param_of(0, value.0, value.1),
            param_of(1, amount.0, amount.1),
        ],
        Some(value.0),
        vec![slot_of("a", value.0), slot_of("b", amount.0)],
        vec![value.0, amount.0, value.0],
        insts,
        Terminator::Return(Some(Temp(2))),
    )
}

/// `fn f(a: from) -> to { return to(a) }`, with the conversion at line 2, column 12.
fn conversion_function(from: Ty, to: Ty) -> Function {
    let conv = Inst::Conv {
        dst: Temp(1),
        src: Temp(0),
        from: from.1,
        to: to.1,
        loc: Loc { line: 2, col: 12 },
    };
    single_block_function(
        "f",
        vec![param_of(0, from.0, from.1)],
        Some(to.0),
        vec![slot_of("a", from.0)],
        vec![from.0, to.0],
        vec![load_of(0, 0, from.1), conv],
        Terminator::Return(Some(Temp(1))),
    )
}

/// The whole assembly of a module holding `function` and an empty `main`.
fn asm_of(function: Function) -> String {
    emit_ok(&module_with_main(vec![function]), FILE)
}

/// The body of `_em_f` for `a OP b`.
fn binary_text(value: Ty, amount: Ty, op: BinOp) -> String {
    function_text(&asm_of(binary_function(value, amount, op)), "_em_f")
}

/// The body of `_em_f` for a conversion.
fn conversion_text(from: Ty, to: Ty) -> String {
    function_text(&asm_of(conversion_function(from, to)), "_em_f")
}

/// The index of the first line of `text` that equals `line` once trimmed.
fn index_of(text: &str, line: &str) -> usize {
    let found = trimmed_lines(text)
        .iter()
        .position(|candidate| *candidate == line);
    assert!(found.is_some(), "`{line}` not found in:\n{text}");
    found.unwrap()
}

/// Asserts that no line of `text` is an extension instruction: `sxtb`, `sxth`, `uxtb` or `uxth`.
fn assert_no_extension(text: &str) {
    for mnemonic in ["sxtb", "sxth", "uxtb", "uxth"] {
        assert_lacks(text, mnemonic);
    }
}

// ----------------------------------------------------------------------------------------------
// Bitwise operations
// ----------------------------------------------------------------------------------------------

/// `And`, `Or` and `Xor` print `and`, `orr` and `eor` at the operand's register width, for every width and sign.
#[test]
fn bitwise_operations_use_and_orr_eor_at_the_operand_width() {
    let table = [(BinOp::And, "and"), (BinOp::Or, "orr"), (BinOp::Xor, "eor")];
    for (op, mnemonic) in table {
        for width in WIDTHS {
            for sign in SIGNS {
                let text = binary_text((width, sign), (width, sign), op);
                let r = reg(width);
                let line = format!("{mnemonic} {r}8, {r}8, {r}9");
                assert_contains(&text, &line);
            }
        }
    }
}

/// A bitwise operation has no check and no panic path.
#[test]
fn bitwise_operations_have_no_panic_path() {
    for op in [BinOp::And, BinOp::Or, BinOp::Xor] {
        for width in WIDTHS {
            let text = binary_text((width, Sign::Signed), (width, Sign::Signed), op);
            assert_lacks(&text, "panic");
            assert_lacks(&text, "cmp");
            assert_lacks(&text, "tst");
        }
    }
}

/// The result of two extended values is extended, so a bitwise operation adds no extension.
#[test]
fn bitwise_operations_add_no_extension() {
    for op in [BinOp::And, BinOp::Or, BinOp::Xor] {
        for sign in SIGNS {
            for width in [IrType::I8, IrType::I16] {
                assert_no_extension(&binary_text((width, sign), (width, sign), op));
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Shifts
// ----------------------------------------------------------------------------------------------

/// The spec's `i8` shift left: compare the amount with 8, branch to the panic, `lsl`, then `sxtb`.
#[test]
fn i8_shift_left_checks_shifts_and_sign_extends() {
    let signed = (IrType::I8, Sign::Signed);
    let text = binary_text(signed, signed, BinOp::Shl(Sign::Signed));
    assert_run(&text, &["cmp w8, #8", "b.hs Lf_panic0"]);
    assert_run(&text, &["lsl w8, w8, w9", "sxtb w8, w8"]);
}

/// The amount is checked before the shift happens.
#[test]
fn the_amount_is_checked_before_the_shift() {
    let signed = (IrType::I8, Sign::Signed);
    let text = binary_text(signed, signed, BinOp::Shl(Sign::Signed));
    assert!(index_of(&text, "b.hs Lf_panic0") < index_of(&text, "lsl w8, w8, w9"));
}

/// An `i8` shift left restores the invariant with `sxtb`, a `u8` one with `uxtb`.
#[test]
fn narrow_shift_left_extends_by_the_sign_of_the_type() {
    let table = [
        (IrType::I8, Sign::Signed, "sxtb"),
        (IrType::I8, Sign::Unsigned, "uxtb"),
        (IrType::I16, Sign::Signed, "sxth"),
        (IrType::I16, Sign::Unsigned, "uxth"),
    ];
    for (width, sign, extension) in table {
        let text = binary_text((width, sign), (width, sign), BinOp::Shl(sign));
        let line = format!("{extension} w8, w8");
        assert_run(&text, &["lsl w8, w8, w9", &line]);
    }
}

/// A 16-bit shift compares the amount with 16.
#[test]
fn sixteen_bit_shift_compares_with_16() {
    let value = (IrType::I16, Sign::Signed);
    let text = binary_text(value, value, BinOp::Shl(value.1));
    assert_run(&text, &["cmp w8, #16", "b.hs Lf_panic0"]);
}

/// A 32-bit shift left compares with 32 and adds no extension.
#[test]
fn shift_left_of_32_bits_has_no_extension() {
    for sign in SIGNS {
        let value = (IrType::I32, sign);
        let text = binary_text(value, value, BinOp::Shl(value.1));
        assert_run(&text, &["cmp w8, #32", "b.hs Lf_panic0"]);
        assert_contains(&text, "lsl w8, w8, w9");
        assert_no_extension(&text);
    }
}

/// A 64-bit shift left compares with 64, uses `x` registers and adds no extension.
#[test]
fn shift_left_of_64_bits_uses_x_registers() {
    for sign in SIGNS {
        let value = (IrType::I64, sign);
        let text = binary_text(value, value, BinOp::Shl(value.1));
        assert_run(&text, &["cmp x8, #64", "b.hs Lf_panic0"]);
        assert_contains(&text, "lsl x8, x8, x9");
        assert_no_extension(&text);
    }
}

/// `>>` of a signed value is `asr` and of an unsigned value is `lsr`, at every width.
#[test]
fn shift_right_picks_asr_or_lsr_by_sign() {
    let table = [(Sign::Signed, "asr"), (Sign::Unsigned, "lsr")];
    for (sign, mnemonic) in table {
        for width in WIDTHS {
            let value = (width, sign);
            let text = binary_text(value, value, BinOp::Shr(sign));
            let r = reg(width);
            let line = format!("{mnemonic} {r}8, {r}8, {r}9");
            assert_contains(&text, &line);
        }
    }
}

/// A shift right needs no extension afterwards: the result is already extended.
#[test]
fn shift_right_adds_no_extension() {
    for sign in SIGNS {
        for width in [IrType::I8, IrType::I16] {
            let value = (width, sign);
            assert_no_extension(&binary_text(value, value, BinOp::Shr(sign)));
        }
    }
}

/// A shift right compares the amount with the width of the left operand.
#[test]
fn shift_right_compares_with_the_width_of_the_left_operand() {
    for width in WIDTHS {
        let value = (width, Sign::Unsigned);
        let text = binary_text(value, value, BinOp::Shr(Sign::Unsigned));
        let line = format!("cmp {}8, #{}", reg(width), width.bits());
        assert_run(&text, &[&line, "b.hs Lf_panic0"]);
    }
}

/// The spec's `u64` shift right by an `i8` amount: `cmp w8, #64` then `lsr x8, x8, x9`.
#[test]
fn u64_shift_right_by_an_i8_amount() {
    let value = (IrType::I64, Sign::Unsigned);
    let amount = (IrType::I8, Sign::Signed);
    let text = binary_text(value, amount, BinOp::Shr(Sign::Unsigned));
    assert_run(&text, &["cmp w8, #64", "b.hs Lf_panic0"]);
    assert_contains(&text, "lsr x8, x8, x9");
}

/// The compare uses the amount's own register width: `w` for 8 to 32 bits, `x` for 64.
#[test]
fn amount_compare_uses_the_amounts_own_width() {
    for amount_width in WIDTHS {
        for amount_sign in SIGNS {
            let value = (IrType::I32, Sign::Signed);
            let amount = (amount_width, amount_sign);
            let text = binary_text(value, amount, BinOp::Shl(value.1));
            let line = format!("cmp {}8, #32", reg(amount_width));
            assert_run(&text, &[&line, "b.hs Lf_panic0"]);
        }
    }
}

/// The shift itself uses the left operand's width, whatever the amount's width.
#[test]
fn shift_instruction_uses_the_left_operands_width() {
    for amount_width in WIDTHS {
        let amount = (amount_width, Sign::Signed);
        let narrow = binary_text(
            (IrType::I8, Sign::Unsigned),
            amount,
            BinOp::Shl(Sign::Unsigned),
        );
        assert_contains(&narrow, "lsl w8, w8, w9");
        let wide = binary_text(
            (IrType::I64, Sign::Unsigned),
            amount,
            BinOp::Shl(Sign::Unsigned),
        );
        assert_contains(&wide, "lsl x8, x8, x9");
    }
}

/// The check is an unsigned compare for a signed amount too, so a negative amount panics.
#[test]
fn signed_amount_uses_the_unsigned_branch() {
    let value = (IrType::I64, Sign::Signed);
    let amount = (IrType::I64, Sign::Signed);
    let text = binary_text(value, amount, BinOp::Shl(value.1));
    assert_contains(&text, "b.hs Lf_panic0");
    assert_lacks(&text, "b.lt");
    assert_lacks(&text, "b.ge");
}

/// The shift panic path passes the location and the reason `shift amount out of range`.
#[test]
fn shift_panic_path_names_the_reason_and_location() {
    let value = (IrType::I32, Sign::Signed);
    let text = binary_text(value, value, BinOp::Shl(value.1));
    assert_run(
        &text,
        &[
            "Lf_panic0:",
            "movz x0, #2",
            "movz x1, #12",
            "adrp x2, Lstr_shift@PAGE",
            "add x2, x2, Lstr_shift@PAGEOFF",
            "bl _ember_rt_panic",
        ],
    );
}

/// The string section holds the shift reason under the label `Lstr_shift`.
#[test]
fn shift_reason_string_is_emitted() {
    let value = (IrType::I32, Sign::Signed);
    let text = asm_of(binary_function(value, value, BinOp::Shl(value.1)));
    assert_run(
        &text,
        &["Lstr_shift:", ".asciz \"shift amount out of range\""],
    );
}

// ----------------------------------------------------------------------------------------------
// Conversions
// ----------------------------------------------------------------------------------------------

/// The spec's `i64` to `u8`: test the bits above the low 8, then branch to the panic.
#[test]
fn i64_to_u8_tests_the_high_bits() {
    let text = conversion_text((IrType::I64, Sign::Signed), (IrType::I8, Sign::Unsigned));
    assert_run(&text, &["tst x8, #0xffffffffffffff00", "b.ne Lf_panic0"]);
}

/// The spec's `u32` to `i64`: widen with `mov w8, w8` and no check.
#[test]
fn u32_to_i64_widens_without_a_check() {
    let text = conversion_text((IrType::I32, Sign::Unsigned), (IrType::I64, Sign::Signed));
    assert_contains(&text, "mov w8, w8");
    assert_lacks(&text, "tst");
    assert_lacks(&text, "panic");
}

/// A signed source of 64 bits to a narrower signed type compares with its own extension.
#[test]
fn i64_to_narrower_signed_compares_with_the_extension() {
    let table = [
        (IrType::I8, "cmp x8, w8, sxtb"),
        (IrType::I16, "cmp x8, w8, sxth"),
        (IrType::I32, "cmp x8, w8, sxtw"),
    ];
    for (target, compare) in table {
        let text = conversion_text((IrType::I64, Sign::Signed), (target, Sign::Signed));
        assert_run(&text, &[compare, "b.ne Lf_panic0"]);
    }
}

/// An unsigned 64-bit source to a narrower signed type tests every bit above the target's sign bit.
#[test]
fn u64_to_narrower_signed_tests_the_high_bits() {
    let table = [
        (IrType::I8, "tst x8, #0xffffffffffffff80"),
        (IrType::I16, "tst x8, #0xffffffffffff8000"),
        (IrType::I32, "tst x8, #0xffffffff80000000"),
    ];
    for (target, test) in table {
        let text = conversion_text((IrType::I64, Sign::Unsigned), (target, Sign::Signed));
        assert_run(&text, &[test, "b.ne Lf_panic0"]);
    }
}

/// A 64-bit signed or unsigned source to a narrower unsigned type tests every bit above the target's width.
#[test]
fn sixty_four_bit_to_narrower_unsigned_tests_the_high_bits() {
    let table = [
        (IrType::I8, "tst x8, #0xffffffffffffff00"),
        (IrType::I16, "tst x8, #0xffffffffffff0000"),
        (IrType::I32, "tst x8, #0xffffffff00000000"),
    ];
    for (target, test) in table {
        for source_sign in SIGNS {
            let from = (IrType::I64, source_sign);
            let text = conversion_text(from, (target, Sign::Unsigned));
            assert_run(&text, &[test, "b.ne Lf_panic0"]);
        }
    }
}

/// `i64` to `u64` and `u64` to `i64` test the top bit.
#[test]
fn sixty_four_bit_sign_change_tests_the_top_bit() {
    let pairs = [
        (Sign::Signed, Sign::Unsigned),
        (Sign::Unsigned, Sign::Signed),
    ];
    for (from_sign, to_sign) in pairs {
        let text = conversion_text((IrType::I64, from_sign), (IrType::I64, to_sign));
        assert_run(&text, &["tst x8, #0x8000000000000000", "b.ne Lf_panic0"]);
    }
}

/// The spec's `i32` to `u8`: widen with `sxtw`, then test the bits above the low 8.
#[test]
fn i32_to_u8_widens_then_tests() {
    let text = conversion_text((IrType::I32, Sign::Signed), (IrType::I8, Sign::Unsigned));
    assert_contains(&text, "sxtw x8, w8");
    assert_run(&text, &["tst x8, #0xffffffffffffff00", "b.ne Lf_panic0"]);
    assert!(index_of(&text, "sxtw x8, w8") < index_of(&text, "tst x8, #0xffffffffffffff00"));
}

/// The spec's `i8` to `u64`: widen with `sxtw`, then test the top bit.
#[test]
fn i8_to_u64_widens_then_tests_the_top_bit() {
    let text = conversion_text((IrType::I8, Sign::Signed), (IrType::I64, Sign::Unsigned));
    assert_contains(&text, "sxtw x8, w8");
    assert_run(&text, &["tst x8, #0x8000000000000000", "b.ne Lf_panic0"]);
}

/// The spec's `u32` to `i32`: widen with `mov w8, w8`, then test every bit above the sign bit.
#[test]
fn u32_to_i32_widens_then_tests() {
    let text = conversion_text((IrType::I32, Sign::Unsigned), (IrType::I32, Sign::Signed));
    assert_contains(&text, "mov w8, w8");
    assert_run(&text, &["tst x8, #0xffffffff80000000", "b.ne Lf_panic0"]);
}

/// The spec's `i16` to `i8`: widen with `sxtw`, then compare with the `sxtb` extension.
#[test]
fn i16_to_i8_widens_then_compares() {
    let text = conversion_text((IrType::I16, Sign::Signed), (IrType::I8, Sign::Signed));
    assert_contains(&text, "sxtw x8, w8");
    assert_run(&text, &["cmp x8, w8, sxtb", "b.ne Lf_panic0"]);
}

/// A signed source of 8 to 32 bits is widened with `sxtw`, an unsigned one with `mov w8, w8`.
#[test]
fn narrow_sources_are_widened_by_sign() {
    for width in [IrType::I8, IrType::I16, IrType::I32] {
        let signed = conversion_text((width, Sign::Signed), (IrType::I64, Sign::Signed));
        assert_contains(&signed, "sxtw x8, w8");
        let unsigned = conversion_text((width, Sign::Unsigned), (IrType::I64, Sign::Signed));
        assert_contains(&unsigned, "mov w8, w8");
    }
}

/// Conversions to `i64` from a signed type or from an unsigned type narrower than 64 bits have no check.
#[test]
fn conversions_to_i64_that_cannot_fail_have_no_check() {
    for width in [IrType::I8, IrType::I16, IrType::I32] {
        for sign in SIGNS {
            let text = conversion_text((width, sign), (IrType::I64, Sign::Signed));
            assert_lacks(&text, "tst");
            assert_lacks(&text, "cmp");
            assert_lacks(&text, "panic");
        }
    }
}

/// Conversions to `u64` from an unsigned type have no check.
#[test]
fn conversions_to_u64_from_unsigned_have_no_check() {
    for width in [IrType::I8, IrType::I16, IrType::I32] {
        let text = conversion_text((width, Sign::Unsigned), (IrType::I64, Sign::Unsigned));
        assert_lacks(&text, "tst");
        assert_lacks(&text, "cmp");
        assert_lacks(&text, "panic");
    }
}

/// A conversion adds no extension after its check: the low 32 bits already hold the target value.
#[test]
fn checked_conversion_adds_no_final_extension() {
    let text = conversion_text((IrType::I64, Sign::Signed), (IrType::I8, Sign::Unsigned));
    assert_lacks(&text, "uxtb");
    assert_lacks(&text, "sxtb");
}

/// The conversion panic path passes the location and the reason `integer conversion out of range`.
#[test]
fn conversion_panic_path_names_the_reason_and_location() {
    let text = conversion_text((IrType::I64, Sign::Signed), (IrType::I8, Sign::Unsigned));
    assert_run(
        &text,
        &[
            "Lf_panic0:",
            "movz x0, #2",
            "movz x1, #12",
            "adrp x2, Lstr_conversion@PAGE",
            "add x2, x2, Lstr_conversion@PAGEOFF",
            "bl _ember_rt_panic",
        ],
    );
}

/// The string section holds the conversion reason under the label `Lstr_conversion`.
#[test]
fn conversion_reason_string_is_emitted() {
    let text = asm_of(conversion_function(
        (IrType::I64, Sign::Signed),
        (IrType::I8, Sign::Unsigned),
    ));
    assert_run(
        &text,
        &[
            "Lstr_conversion:",
            ".asciz \"integer conversion out of range\"",
        ],
    );
}
