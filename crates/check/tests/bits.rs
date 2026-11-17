//! Integration tests for the bitwise operators, the shifts and the `T(x)` integer conversions in the checker.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use check::{Binding, TypeId};
use common::{assert_single_error, checked, errors, fixture, main_with, span_of};
use source::Span;

/// One integer type, with the limits a conversion test needs.
struct Int {
    /// The source spelling, such as `u8`.
    name: &'static str,
    /// The id the checker records for the type.
    id: TypeId,
    /// The smallest value, with its minus sign for signed types.
    min: &'static str,
    /// The largest value.
    max: &'static str,
    /// The largest value plus one; `None` for `u64`, whose `MAX + 1` is too big for the lexer.
    above_max: Option<&'static str>,
    /// The magnitude of the smallest value minus one; `None` for unsigned types.
    below_min_magnitude: Option<&'static str>,
}

/// All eight integer types, signed first.
const INTS: [Int; 8] = [
    Int {
        name: "i8",
        id: TypeId::I8,
        min: "-128",
        max: "127",
        above_max: Some("128"),
        below_min_magnitude: Some("129"),
    },
    Int {
        name: "i16",
        id: TypeId::I16,
        min: "-32768",
        max: "32767",
        above_max: Some("32768"),
        below_min_magnitude: Some("32769"),
    },
    Int {
        name: "i32",
        id: TypeId::I32,
        min: "-2147483648",
        max: "2147483647",
        above_max: Some("2147483648"),
        below_min_magnitude: Some("2147483649"),
    },
    Int {
        name: "i64",
        id: TypeId::I64,
        min: "-9223372036854775808",
        max: "9223372036854775807",
        above_max: Some("9223372036854775808"),
        below_min_magnitude: Some("9223372036854775809"),
    },
    Int {
        name: "u8",
        id: TypeId::U8,
        min: "0",
        max: "255",
        above_max: Some("256"),
        below_min_magnitude: None,
    },
    Int {
        name: "u16",
        id: TypeId::U16,
        min: "0",
        max: "65535",
        above_max: Some("65536"),
        below_min_magnitude: None,
    },
    Int {
        name: "u32",
        id: TypeId::U32,
        min: "0",
        max: "4294967295",
        above_max: Some("4294967296"),
        below_min_magnitude: None,
    },
    Int {
        name: "u64",
        id: TypeId::U64,
        min: "0",
        max: "18446744073709551615",
        above_max: None,
        below_min_magnitude: None,
    },
];

/// The three binary bitwise operators.
const BITWISE: [&str; 3] = ["&", "|", "^"];

/// The two shift operators.
const SHIFTS: [&str; 2] = ["<<", ">>"];

/// A program that declares `lhs: left_type = 7` and `rhs: right_type = 5`, then runs `statement`.
fn with_two_locals(left_type: &str, right_type: &str, statement: &str) -> String {
    let body = format!("    lhs: {left_type} = 7\n    rhs: {right_type} = 5\n{statement}");
    main_with(&body)
}

/// A program that declares `val: type_name = 7`, then runs `statement`.
fn with_one_local(type_name: &str, statement: &str) -> String {
    main_with(&format!("    val: {type_name} = 7\n{statement}"))
}

/// A program that declares `amount: amount_type = 5`, then runs `statement`.
fn with_amount(amount_type: &str, statement: &str) -> String {
    main_with(&format!("    amount: {amount_type} = 5\n{statement}"))
}

/// Asserts one error, ``integer literal does not fit in `T` ``, at the digits of `literal`.
///
/// `literal` may start with a minus sign; the error is at the digits after it.
fn assert_literal_does_not_fit(src: &str, type_name: &str, literal: &str) {
    let message = format!("integer literal does not fit in `{type_name}`");
    let digits = literal.trim_start_matches('-');
    let start = span_of(src, literal, 0).start + (literal.len() - digits.len());
    let expected = (message, Span::new(start, start + digits.len()));
    assert_eq!(errors(src), vec![expected]);
}

// ---------------------------------------------------------------- bitwise: types

/// `a & b`, `a | b` and `a ^ b` on two values of one type have that type, for every integer type.
#[test]
fn bitwise_result_has_the_operand_type() {
    for int in &INTS {
        for op in BITWISE {
            let expr = format!("lhs {op} rhs");
            let src = with_two_locals(int.name, int.name, &format!("    print({expr})"));
            assert_eq!(
                fixture(&src).expr_type(&expr, 0),
                int.id,
                "{} {op}",
                int.name
            );
        }
    }
}

/// `a & b` on two `u16` values has type `u16`, as the spec's first example says.
#[test]
fn and_of_two_u16_values_is_u16() {
    let src = with_two_locals("u16", "u16", "    print(lhs & rhs)");
    assert_eq!(fixture(&src).expr_type("lhs & rhs", 0), TypeId::U16);
}

/// `~a` has the type of `a`, for every integer type.
#[test]
fn not_result_has_the_operand_type() {
    for int in &INTS {
        let src = with_one_local(int.name, "    print(~val)");
        assert_eq!(fixture(&src).expr_type("~val", 0), int.id, "{}", int.name);
    }
}

/// `~a` with `a: i8` has type `i8`, as the spec's second example says.
#[test]
fn not_of_an_i8_is_i8() {
    let src = with_one_local("i8", "    print(~val)");
    assert_eq!(fixture(&src).expr_type("~val", 0), TypeId::I8);
}

/// A literal next to a typed operand of a bitwise operator takes that operand's type.
#[test]
fn bitwise_literal_right_of_a_typed_operand_takes_its_type() {
    for int in &INTS {
        for op in BITWISE {
            let src = with_one_local(int.name, &format!("    print(val {op} 9)"));
            let fixture = fixture(&src);
            assert_eq!(fixture.expr_type("9", 0), int.id, "{} {op}", int.name);
            assert_eq!(
                fixture.expr_type(&format!("val {op} 9"), 0),
                int.id,
                "{} {op}",
                int.name
            );
        }
    }
}

/// A literal left of a typed operand of a bitwise operator takes that operand's type.
#[test]
fn bitwise_literal_left_of_a_typed_operand_takes_its_type() {
    for int in &INTS {
        for op in BITWISE {
            let src = with_one_local(int.name, &format!("    print(9 {op} val)"));
            let fixture = fixture(&src);
            assert_eq!(fixture.expr_type("9", 0), int.id, "{} {op}", int.name);
            assert_eq!(
                fixture.expr_type(&format!("9 {op} val"), 0),
                int.id,
                "{} {op}",
                int.name
            );
        }
    }
}

/// `x: u8 = ~0` settles the `0` to `u8`, and so does the `~0` around it, for every integer type.
#[test]
fn not_of_a_literal_settles_to_the_annotation() {
    for int in &INTS {
        let src = main_with(&format!("    x: {} = ~0\n    print(x)", int.name));
        let fixture = fixture(&src);
        assert_eq!(fixture.expr_type("0", 0), int.id, "{}", int.name);
        assert_eq!(fixture.expr_type("~0", 0), int.id, "{}", int.name);
    }
}

/// `~0` with no context defaults to `i64` on the literal and on the operator.
#[test]
fn not_of_a_literal_without_context_defaults_to_i64() {
    let fixture = fixture(&main_with("    print(~0)"));
    assert_eq!(fixture.expr_type("0", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("~0", 0), TypeId::I64);
}

/// A bitwise operator on two literals defaults to `i64` on every node.
#[test]
fn bitwise_of_two_literals_defaults_to_i64() {
    for op in BITWISE {
        let expr = format!("5 {op} 9");
        let fixture = fixture(&main_with(&format!("    print({expr})")));
        assert_eq!(fixture.expr_type("5", 0), TypeId::I64, "{op}");
        assert_eq!(fixture.expr_type("9", 0), TypeId::I64, "{op}");
        assert_eq!(fixture.expr_type(&expr, 0), TypeId::I64, "{op}");
    }
}

/// A bitwise operator on two literals settles both to the annotation of the `let`.
#[test]
fn bitwise_of_two_literals_settles_to_the_annotation() {
    for op in BITWISE {
        let expr = format!("5 {op} 9");
        let src = main_with(&format!("    x: i16 = {expr}\n    print(x)"));
        let fixture = fixture(&src);
        assert_eq!(fixture.expr_type("5", 0), TypeId::I16, "{op}");
        assert_eq!(fixture.expr_type("9", 0), TypeId::I16, "{op}");
        assert_eq!(fixture.expr_type(&expr, 0), TypeId::I16, "{op}");
    }
}

/// An untyped `~` of a sum settles every node of the sum to the annotation.
#[test]
fn not_of_an_untyped_sum_settles_every_node() {
    let src = main_with("    x: u32 = ~(6 + 9)\n    print(x)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("6", 0), TypeId::U32);
    assert_eq!(fixture.expr_type("9", 0), TypeId::U32);
    assert_eq!(fixture.expr_type("(6 + 9)", 0), TypeId::U32);
    assert_eq!(fixture.expr_type("~(6 + 9)", 0), TypeId::U32);
}

/// Bitwise and binds tighter than `==`, so `val & 9 == 0` compares a `u8` with a literal.
#[test]
fn bitwise_and_inside_a_comparison_keeps_its_own_type() {
    let src = with_one_local("u8", "    print(val & 9 == 0)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("val & 9", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("0", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("val & 9 == 0", 0), TypeId::BOOL);
}

/// Several bitwise operators in a row, mixed with `+`, check as one `u8` expression.
#[test]
fn mixed_bitwise_and_arithmetic_chain_is_accepted() {
    let src = with_one_local("u8", "    print(val | val ^ val & val + val)");
    checked(&src);
}

// ---------------------------------------------------------------- bitwise: errors

/// `true & false` is rejected at `true`, since `&` on `bool` is not allowed; the same for `|` and `^`.
#[test]
fn bitwise_on_bool_literals_is_rejected_at_the_left_operand() {
    for op in BITWISE {
        let src = main_with(&format!("    print(true {op} false)"));
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// A `bool` right of a typed integer is reported against that integer's type, at the `bool`.
#[test]
fn bitwise_with_a_bool_right_operand_is_rejected_at_the_bool() {
    for op in BITWISE {
        let src = with_one_local("u8", &format!("    print(val {op} true)"));
        assert_single_error(&src, "expected `u8`, found `bool`", "true", 0);
    }
}

/// A `bool` left of an untyped literal is reported against `i64`, at the `bool`.
#[test]
fn bitwise_with_a_bool_left_operand_and_a_literal_is_rejected_at_the_bool() {
    for op in BITWISE {
        let src = main_with(&format!("    print(true {op} 9)"));
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// Operands of two integer types are rejected at the right operand, naming the left type.
#[test]
fn bitwise_with_mismatched_types_is_rejected_at_the_right_operand() {
    for op in BITWISE {
        let src = with_two_locals("u8", "u16", &format!("    print(lhs {op} rhs)"));
        assert_single_error(&src, "expected `u8`, found `u16`", "rhs", 1);
    }
}

/// A signed and an unsigned type of one width are different types for a bitwise operator.
#[test]
fn bitwise_with_signed_and_unsigned_of_one_width_is_rejected() {
    for op in BITWISE {
        let src = with_two_locals("i32", "u32", &format!("    print(lhs {op} rhs)"));
        assert_single_error(&src, "expected `i32`, found `u32`", "rhs", 1);
    }
}

/// A literal that does not fit the other operand's type is rejected at the literal.
#[test]
fn bitwise_literal_above_the_operand_type_is_rejected_at_the_literal() {
    for op in BITWISE {
        let src = with_one_local("u8", &format!("    print(val {op} 256)"));
        assert_literal_does_not_fit(&src, "u8", "256");
    }
}

/// A negative literal next to an unsigned operand is rejected at its digits.
#[test]
fn bitwise_negative_literal_for_an_unsigned_operand_is_rejected_at_the_digits() {
    let src = with_one_local("u8", "    print(val & -9)");
    assert_literal_does_not_fit(&src, "u8", "-9");
}

/// `~true` is rejected at `true`, against `i64`.
#[test]
fn not_of_a_bool_literal_is_rejected_at_the_operand() {
    let src = main_with("    print(~true)");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// A literal that does not fit the annotation is rejected at the literal under `~`.
#[test]
fn not_of_a_literal_that_does_not_fit_the_annotation_is_rejected_at_the_literal() {
    let src = main_with("    x: u8 = ~256\n    print(x)");
    assert_literal_does_not_fit(&src, "u8", "256");
}

/// A bitwise result of one type is not accepted where another type is declared; the error covers the expression.
#[test]
fn bitwise_result_for_another_annotation_is_rejected_at_the_whole_expression() {
    let body = "    a: u8 = 7\n    b: u8 = 5\n    x: u16 = a & b\n    print(x)";
    assert_single_error(&main_with(body), "expected `u16`, found `u8`", "a & b", 0);
}

/// A `~` result of one type is not accepted where another type is declared.
#[test]
fn not_result_for_another_annotation_is_rejected_at_the_whole_expression() {
    let body = "    a: u8 = 7\n    x: u16 = ~a\n    print(x)";
    assert_single_error(&main_with(body), "expected `u16`, found `u8`", "~a", 0);
}

/// An undefined name left of a bitwise operator is reported once.
#[test]
fn undefined_left_operand_of_a_bitwise_operator_gives_one_error() {
    let src = main_with("    print(missing & 9)");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// An undefined name right of a typed operand is reported once, with no mismatch error.
#[test]
fn undefined_right_operand_of_a_bitwise_operator_gives_one_error() {
    let src = with_one_local("u8", "    print(val | missing)");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// An undefined name under `~` is reported once, with no mismatch against the annotation.
#[test]
fn undefined_operand_of_not_gives_one_error() {
    let src = main_with("    x: u8 = ~missing\n    print(x)");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

// ---------------------------------------------------------------- shifts: types

/// `x << n` and `x >> n` have the type of `x`, whatever the integer type of `n`: all 64 pairs.
#[test]
fn shift_result_has_the_left_operands_type() {
    for value in &INTS {
        for amount in &INTS {
            for op in SHIFTS {
                let expr = format!("lhs {op} rhs");
                let src = with_two_locals(value.name, amount.name, &format!("    print({expr})"));
                assert_eq!(
                    fixture(&src).expr_type(&expr, 0),
                    value.id,
                    "{} {op} {}",
                    value.name,
                    amount.name
                );
            }
        }
    }
}

/// `x << n` with `x: u16` and `n: i8` has type `u16`, as the spec's example says.
#[test]
fn shift_of_a_u16_by_an_i8_is_u16() {
    let src = with_two_locals("u16", "i8", "    print(lhs << rhs)");
    assert_eq!(fixture(&src).expr_type("lhs << rhs", 0), TypeId::U16);
}

/// The amount of a shift keeps its own type.
#[test]
fn shift_amount_keeps_its_own_type() {
    for amount in &INTS {
        let src = with_two_locals("u32", amount.name, "    print(lhs >> rhs)");
        assert_eq!(
            fixture(&src).expr_type("rhs", 1),
            amount.id,
            "{}",
            amount.name
        );
    }
}

/// `x: u8 = 9 << n` records the annotation's type on the `9`, whatever the type of `n`.
#[test]
fn shifted_literal_takes_the_annotation_whatever_the_amount_type() {
    for value in &INTS {
        for amount in &INTS {
            for op in SHIFTS {
                let statement = format!("    x: {} = 9 {op} amount\n    print(x)", value.name);
                let src = with_amount(amount.name, &statement);
                let fixture = fixture(&src);
                let label = format!("{} {op} {}", value.name, amount.name);
                assert_eq!(fixture.expr_type("9", 0), value.id, "{label}");
                assert_eq!(
                    fixture.expr_type(&format!("9 {op} amount"), 0),
                    value.id,
                    "{label}"
                );
            }
        }
    }
}

/// `9 << n` with `n: u8` and no context records `i64` on the `9`, as the spec says.
#[test]
fn shifted_literal_without_context_defaults_to_i64() {
    let src = with_amount("u8", "    print(9 << amount)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("9", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("9 << amount", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("amount", 1), TypeId::U8);
}

/// A literal amount settles on its own to `i64`, even when the left operand is a `u8`.
#[test]
fn literal_amount_defaults_to_i64() {
    for op in SHIFTS {
        let src = with_one_local("u8", &format!("    print(val {op} 5)"));
        let fixture = fixture(&src);
        assert_eq!(fixture.expr_type("5", 0), TypeId::I64, "{op}");
        assert_eq!(
            fixture.expr_type(&format!("val {op} 5"), 0),
            TypeId::U8,
            "{op}"
        );
    }
}

/// With two literals and an annotation, only the left one takes the annotation; the amount is `i64`.
#[test]
fn shift_of_two_literals_settles_only_the_left_operand() {
    let src = main_with("    x: u8 = 9 << 5\n    print(x)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("9", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("5", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("9 << 5", 0), TypeId::U8);
}

/// A shift of two literals with no context defaults to `i64` on every node.
#[test]
fn shift_of_two_literals_without_context_defaults_to_i64() {
    let fixture = fixture(&main_with("    print(9 << 5)"));
    assert_eq!(fixture.expr_type("9", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("5", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("9 << 5", 0), TypeId::I64);
}

/// A negative literal amount is accepted by the checker; it panics at run time.
#[test]
fn negative_literal_amount_is_accepted() {
    let src = with_one_local("u8", "    print(val << -5)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("5", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("val << -5", 0), TypeId::U8);
}

/// A literal amount settles to `i64`, so one above `i64::MAX` does not fit.
#[test]
fn literal_amount_above_i64_is_rejected_at_the_literal() {
    let src = with_one_local("u8", "    print(val << 9223372036854775808)");
    assert_literal_does_not_fit(&src, "i64", "9223372036854775808");
}

/// A literal left operand is still checked against the annotation's range.
#[test]
fn shifted_literal_that_does_not_fit_the_annotation_is_rejected() {
    let src = main_with("    x: u8 = 256 << 5\n    print(x)");
    assert_literal_does_not_fit(&src, "u8", "256");
}

/// In `a << 1 >> 5` the left operand's type flows through the whole chain.
#[test]
fn chained_shifts_keep_the_first_operands_type() {
    let src = with_one_local("u8", "    print(val << 5 >> 5)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("val << 5", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("val << 5 >> 5", 0), TypeId::U8);
}

/// An untyped shifted literal next to a typed operand of `==` settles to that operand's type.
#[test]
fn shifted_literal_in_a_comparison_settles_to_the_other_side() {
    let src = with_amount("i64", "    val: u8 = 7\n    print(val == 9 << amount)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("9", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("9 << amount", 0), TypeId::U8);
}

/// An untyped shifted literal next to a typed operand of `+` settles to that operand's type.
#[test]
fn shifted_literal_in_a_sum_settles_to_the_other_side() {
    let src = with_amount("u16", "    val: u8 = 7\n    print(val + 9 << amount)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("9", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("9 << amount", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("val + 9 << amount", 0), TypeId::U8);
}

// ---------------------------------------------------------------- shifts: errors

/// A `bool` left operand is rejected at the `bool`, against `i64`.
#[test]
fn shift_of_a_bool_is_rejected_at_the_left_operand() {
    for op in SHIFTS {
        let src = main_with(&format!("    print(true {op} 5)"));
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// A `bool` amount is rejected at the `bool`; the amount settles on its own, so it is checked against `i64`.
#[test]
fn shift_by_a_bool_is_rejected_at_the_amount() {
    for op in SHIFTS {
        let src = with_one_local("u8", &format!("    print(val {op} true)"));
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// A shift result of one type is not accepted where another type is declared; the error covers the expression.
#[test]
fn shift_result_for_another_annotation_is_rejected_at_the_whole_expression() {
    let body = "    a: u8 = 7\n    x: u16 = a << 5\n    print(x)";
    assert_single_error(&main_with(body), "expected `u16`, found `u8`", "a << 5", 0);
}

/// An undefined left operand is reported once.
#[test]
fn undefined_left_operand_of_a_shift_gives_one_error() {
    let src = main_with("    x: u8 = missing << 5\n    print(x)");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// An undefined amount is reported once.
#[test]
fn undefined_amount_of_a_shift_gives_one_error() {
    let src = with_one_local("u8", "    print(val << missing)");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

// ---------------------------------------------------------------- conversions: results

/// `T(x)` has type `T` and the binding `Binding::Convert(T)`, for every ordered pair of integer types.
#[test]
fn conversion_has_the_target_type_and_binding_for_every_pair() {
    for from in &INTS {
        for to in &INTS {
            let call = format!("{}(val)", to.name);
            let src = with_one_local(from.name, &format!("    print({call})"));
            let fixture = fixture(&src);
            let label = format!("{} -> {}", from.name, to.name);
            assert_eq!(fixture.expr_type(&call, 0), to.id, "{label}");
            assert_eq!(
                fixture.expr_binding(&call, 0),
                Binding::Convert(to.id),
                "{label}"
            );
        }
    }
}

/// `u8(x)` with `x: i64` has type `u8` and the binding `Binding::Convert(TypeId::U8)`.
#[test]
fn u8_of_an_i64_is_a_u8_conversion() {
    let src = with_one_local("i64", "    print(u8(val))");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("u8(val)", 0), TypeId::U8);
    assert_eq!(
        fixture.expr_binding("u8(val)", 0),
        Binding::Convert(TypeId::U8)
    );
}

/// The argument of a conversion keeps its own type.
#[test]
fn conversion_argument_keeps_its_own_type() {
    for from in &INTS {
        let src = with_one_local(from.name, "    print(u8(val))");
        assert_eq!(fixture(&src).expr_type("val", 1), from.id, "{}", from.name);
    }
}

/// An untyped literal argument settles to the target type, for every integer type.
#[test]
fn conversion_of_a_literal_settles_it_to_the_target() {
    for to in &INTS {
        let call = format!("{}({})", to.name, to.max);
        let src = main_with(&format!("    print({call})"));
        let fixture = fixture(&src);
        assert_eq!(fixture.expr_type(to.max, 0), to.id, "{}", to.name);
        assert_eq!(fixture.expr_type(&call, 0), to.id, "{}", to.name);
        assert_eq!(
            fixture.expr_binding(&call, 0),
            Binding::Convert(to.id),
            "{}",
            to.name
        );
    }
}

/// An untyped sum argument settles every node of the sum to the target type.
#[test]
fn conversion_of_an_untyped_sum_settles_every_node() {
    let fixture = fixture(&main_with("    print(u8(9 + 5))"));
    assert_eq!(fixture.expr_type("9", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("5", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("9 + 5", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("u8(9 + 5)", 0), TypeId::U8);
}

/// The smallest value of every type is accepted as a literal argument.
#[test]
fn conversion_of_the_smallest_literal_is_accepted() {
    for to in &INTS {
        let src = main_with(&format!("    print({}({}))", to.name, to.min));
        checked(&src);
    }
}

/// A typed expression of any integer type is a valid argument.
#[test]
fn conversion_of_a_typed_expression_is_accepted() {
    let body = "    a: i32 = 7\n    b: i32 = 5\n    print(u8(a + b))";
    let fixture = fixture(&main_with(body));
    assert_eq!(fixture.expr_type("a + b", 0), TypeId::I32);
    assert_eq!(fixture.expr_type("u8(a + b)", 0), TypeId::U8);
}

/// The result of a conversion is a typed operand, so a literal next to it takes its type.
#[test]
fn literal_next_to_a_conversion_takes_the_target_type() {
    let src = with_one_local("i64", "    print(u16(val) + 9)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("9", 0), TypeId::U16);
    assert_eq!(fixture.expr_type("u16(val) + 9", 0), TypeId::U16);
}

/// Conversions nest: the inner result is the argument of the outer.
#[test]
fn conversions_nest() {
    let src = with_one_local("i64", "    print(u8(u16(val)))");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("u16(val)", 0), TypeId::U16);
    assert_eq!(fixture.expr_type("u8(u16(val))", 0), TypeId::U8);
}

/// A conversion result is not accepted where another integer type is declared.
#[test]
fn conversion_result_for_another_annotation_is_rejected_at_the_call() {
    let body = "    a: i64 = 7\n    x: u16 = u8(a)\n    print(x)";
    assert_single_error(&main_with(body), "expected `u16`, found `u8`", "u8(a)", 0);
}

/// A conversion result of one type does not mix with another integer type in arithmetic.
#[test]
fn conversion_result_does_not_mix_with_another_type() {
    let body = "    a: i64 = 7\n    b: u16 = 5\n    print(u8(a) + b)";
    assert_single_error(&main_with(body), "expected `u8`, found `u16`", "b", 1);
}

// ---------------------------------------------------------------- conversions: errors

/// A literal above the target's largest value is rejected at the literal.
#[test]
fn conversion_of_a_literal_above_the_target_is_rejected_at_the_literal() {
    for to in &INTS {
        let Some(above_max) = to.above_max else {
            continue;
        };
        let src = main_with(&format!("    print({}({above_max}))", to.name));
        assert_literal_does_not_fit(&src, to.name, above_max);
    }
}

/// `u8(300)` is rejected at `300`, as the spec's example says.
#[test]
fn u8_of_300_is_rejected_at_the_literal() {
    let src = main_with("    print(u8(300))");
    assert_literal_does_not_fit(&src, "u8", "300");
}

/// A negative literal below the target's smallest value is rejected at its digits.
#[test]
fn conversion_of_a_literal_below_the_target_is_rejected_at_the_digits() {
    for to in &INTS {
        let Some(magnitude) = to.below_min_magnitude else {
            continue;
        };
        let literal = format!("-{magnitude}");
        let src = main_with(&format!("    print({}({literal}))", to.name));
        assert_literal_does_not_fit(&src, to.name, &literal);
    }
}

/// A negative literal for an unsigned target is rejected at its digits.
#[test]
fn conversion_of_a_negative_literal_to_an_unsigned_type_is_rejected() {
    for to in INTS.iter().filter(|int| int.min == "0") {
        let src = main_with(&format!("    print({}(-9))", to.name));
        assert_literal_does_not_fit(&src, to.name, "-9");
    }
}

/// `u8(1, 2)` is rejected at the call, with the existing arity text.
#[test]
fn conversion_with_two_arguments_is_rejected_at_the_call() {
    let src = main_with("    print(u8(1, 2))");
    let message = "`u8` takes 1 argument, but 2 were given";
    assert_single_error(&src, message, "u8(1, 2)", 0);
}

/// Every integer name reports the arity of a conversion the same way.
#[test]
fn conversion_with_two_arguments_is_rejected_for_every_type() {
    for to in &INTS {
        let call = format!("{}(1, 2)", to.name);
        let src = main_with(&format!("    print({call})"));
        let message = format!("`{}` takes 1 argument, but 2 were given", to.name);
        assert_single_error(&src, &message, &call, 0);
    }
}

/// A conversion without an argument is rejected at the call.
#[test]
fn conversion_without_arguments_is_rejected_at_the_call() {
    let src = main_with("    print(i16())");
    let message = "`i16` takes 1 argument, but 0 were given";
    assert_single_error(&src, message, "i16()", 0);
}

/// `u8(true)` is rejected at the argument, for every integer target.
#[test]
fn conversion_of_a_bool_literal_is_rejected_at_the_argument() {
    for to in &INTS {
        let src = main_with(&format!("    print({}(true))", to.name));
        let message = format!("cannot convert `bool` to `{}`", to.name);
        assert_single_error(&src, &message, "true", 0);
    }
}

/// A `bool` variable is rejected at the variable.
#[test]
fn conversion_of_a_bool_variable_is_rejected_at_the_argument() {
    let src = main_with("    flag := true\n    print(u8(flag))");
    assert_single_error(&src, "cannot convert `bool` to `u8`", "flag", 1);
}

/// `bool(x)` with `x: i64` is rejected at the whole call.
#[test]
fn bool_conversion_is_rejected_at_the_call() {
    let src = with_one_local("i64", "    print(bool(val))");
    assert_single_error(&src, "cannot convert `i64` to `bool`", "bool(val)", 0);
}

/// `bool(1)` names the literal's default type.
#[test]
fn bool_conversion_of_a_literal_names_i64() {
    let src = main_with("    print(bool(9))");
    assert_single_error(&src, "cannot convert `i64` to `bool`", "bool(9)", 0);
}

/// `bool(x)` names the type of `x` when `x` is an integer, whatever its type.
#[test]
fn bool_conversion_names_the_argument_type() {
    for from in &INTS {
        let src = with_one_local(from.name, "    print(bool(val))");
        let message = format!("cannot convert `{}` to `bool`", from.name);
        assert_single_error(&src, &message, "bool(val)", 0);
    }
}

/// `bool(true)` says there is no conversion to `bool`, at the call.
#[test]
fn bool_conversion_of_a_bool_literal_says_there_is_no_conversion() {
    let src = main_with("    print(bool(true))");
    assert_single_error(&src, "there is no conversion to `bool`", "bool(true)", 0);
}

/// A `bool` variable as the argument of `bool(x)` gets the same message, at the call.
#[test]
fn bool_conversion_of_a_bool_variable_says_there_is_no_conversion() {
    let src = main_with("    flag := true\n    print(bool(flag))");
    assert_single_error(&src, "there is no conversion to `bool`", "bool(flag)", 0);
}

/// The spec types that are not supported yet are rejected at the callee.
#[test]
fn unsupported_type_conversion_is_rejected_at_the_callee() {
    for name in ["f16", "bf16", "f32", "f64", "str"] {
        // The extra read keeps `val` used even if the call's argument is not checked.
        let statement = format!("    print(val)\n    print({name}(val))");
        let src = with_one_local("i64", &statement);
        let message = format!("type `{name}` is not supported yet");
        assert_single_error(&src, &message, name, 0);
    }
}

/// An undefined argument of a conversion is reported once, with no follow-on error.
#[test]
fn conversion_of_an_undefined_name_gives_one_error() {
    let src = main_with("    print(u8(missing))");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// A name that is neither a function, a local, `print` nor a type is still undefined.
#[test]
fn callee_that_is_not_a_type_name_is_still_undefined() {
    let src = main_with("    print(u9(5))");
    assert_single_error(&src, "undefined function `u9`", "u9", 0);
}

// ---------------------------------------------------------------- conversions: resolution order

/// A user function named like an integer type shadows the conversion: `u8(5)` calls it.
#[test]
fn user_function_named_u8_shadows_the_conversion() {
    let src = "\
fn u8(x: i64) -> i64 {
    return x
}
fn main() {
    print(u8(5))
}
";
    let fixture = fixture(src);
    assert_eq!(
        fixture.expr_binding("u8(5)", 0),
        Binding::Function("u8".to_string())
    );
    assert_eq!(fixture.expr_type("u8(5)", 0), TypeId::I64);
}

/// The shadowing function's own signature applies: its argument is not settled to `u8`.
#[test]
fn shadowing_function_checks_its_arguments_by_its_signature() {
    let src = "\
fn u8(x: i64) -> i64 {
    return x
}
fn main() {
    print(u8(300))
}
";
    let fixture = fixture(src);
    assert_eq!(fixture.expr_type("300", 0), TypeId::I64);
}

/// A user function named `bool` is called, not rejected as a conversion.
#[test]
fn user_function_named_bool_shadows_the_conversion_error() {
    let src = "\
fn bool(x: i64) -> i64 {
    return x
}
fn main() {
    print(bool(5))
}
";
    let fixture = fixture(src);
    assert_eq!(
        fixture.expr_binding("bool(5)", 0),
        Binding::Function("bool".to_string())
    );
}

/// A user function named `f32` is called, not rejected as an unsupported type.
#[test]
fn user_function_named_f32_shadows_the_unsupported_type_error() {
    let src = "\
fn f32(x: i64) -> i64 {
    return x
}
fn main() {
    print(f32(5))
}
";
    checked(src);
}

/// A local named like an integer type is not a function, and it shadows the conversion.
#[test]
fn local_named_u8_is_not_a_function() {
    let src = main_with("    u8 := 5\n    print(u8(3))\n    print(u8)");
    assert_single_error(&src, "`u8` is not a function", "u8", 1);
}

/// A local named `bool` is not a function either.
#[test]
fn local_named_bool_is_not_a_function() {
    let src = main_with("    bool := 5\n    print(bool(3))\n    print(bool)");
    assert_single_error(&src, "`bool` is not a function", "bool", 1);
}

/// A local named like a type in one function does not stop another function from converting.
#[test]
fn conversion_is_available_when_another_function_declares_a_local_named_like_it() {
    let src = "\
fn first() {
    u8 := 5
    print(u8)
}
fn main() {
    first()
    print(u8(5))
}
";
    let fixture = fixture(src);
    assert_eq!(
        fixture.expr_binding("u8(5)", 0),
        Binding::Convert(TypeId::U8)
    );
}

// ---------------------------------------------------------------- nothing is left untyped or unbound

/// A program that uses every new construct in a context that settles its literals.
const EVERY_BITS_CONTEXT: &str = "\
fn main() {
    a: u8 = 200
    n: i8 = 3
    b := 7
    x: u16 = ~0
    y: i32 = 6 | 9 ^ 5 & 3
    z: u64 = 9 << n
    print(a & 15)
    print(~a)
    print(a >> n)
    print(1 << b)
    print(u8(b))
    print(i16(300))
    print(u64(a) + 1)
    print(x)
    print(y)
    print(z)
}
";

/// After checking, no expression or local has the untyped type, and every name and call has a binding.
#[test]
fn no_untyped_type_survives_in_a_program_using_every_new_construct() {
    let fixture = fixture(EVERY_BITS_CONTEXT);
    for (node, id) in &fixture.checked.expr_types {
        assert_ne!(*id, TypeId::UNTYPED_INT, "expression {node:?}");
    }
    for (node, id) in &fixture.checked.local_types {
        assert_ne!(*id, TypeId::UNTYPED_INT, "local {node:?}");
    }
    for id in fixture.all_expr_ids() {
        assert!(
            fixture.checked.expr_types.contains_key(&id),
            "type of {id:?}"
        );
    }
    for id in fixture.name_and_call_ids() {
        assert!(
            fixture.checked.bindings.contains_key(&id),
            "binding of {id:?}"
        );
    }
}
