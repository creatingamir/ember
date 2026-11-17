//! Integration tests for the eight integer types: literal typing, operator rules, calls, ranges and printing.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use std::fs;
use std::path::Path;

use check::{Signature, TypeId, TypeTable};
use common::{assert_single_error, checked, errors, fixture, main_with, parse, span_of};
use source::Span;

/// One integer type, with its limits written the way a program writes them.
struct IntType {
    /// The source spelling, such as `u8`.
    name: &'static str,
    /// The id the checker records for the type.
    id: TypeId,
    /// Whether the type can hold negative values.
    signed: bool,
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
const INT_TYPES: [IntType; 8] = [
    IntType {
        name: "i8",
        id: TypeId::I8,
        signed: true,
        min: "-128",
        max: "127",
        above_max: Some("128"),
        below_min_magnitude: Some("129"),
    },
    IntType {
        name: "i16",
        id: TypeId::I16,
        signed: true,
        min: "-32768",
        max: "32767",
        above_max: Some("32768"),
        below_min_magnitude: Some("32769"),
    },
    IntType {
        name: "i32",
        id: TypeId::I32,
        signed: true,
        min: "-2147483648",
        max: "2147483647",
        above_max: Some("2147483648"),
        below_min_magnitude: Some("2147483649"),
    },
    IntType {
        name: "i64",
        id: TypeId::I64,
        signed: true,
        min: "-9223372036854775808",
        max: "9223372036854775807",
        above_max: Some("9223372036854775808"),
        below_min_magnitude: Some("9223372036854775809"),
    },
    IntType {
        name: "u8",
        id: TypeId::U8,
        signed: false,
        min: "0",
        max: "255",
        above_max: Some("256"),
        below_min_magnitude: None,
    },
    IntType {
        name: "u16",
        id: TypeId::U16,
        signed: false,
        min: "0",
        max: "65535",
        above_max: Some("65536"),
        below_min_magnitude: None,
    },
    IntType {
        name: "u32",
        id: TypeId::U32,
        signed: false,
        min: "0",
        max: "4294967295",
        above_max: Some("4294967296"),
        below_min_magnitude: None,
    },
    IntType {
        name: "u64",
        id: TypeId::U64,
        signed: false,
        min: "0",
        max: "18446744073709551615",
        above_max: None,
        below_min_magnitude: None,
    },
];

/// The arithmetic operators.
const ARITHMETIC: [&str; 5] = ["+", "-", "*", "/", "%"];

/// The ordering comparisons.
const ORDERING: [&str; 4] = ["<", "<=", ">", ">="];

/// The equality comparisons.
const EQUALITY: [&str; 2] = ["==", "!="];

/// A program with a typed `let` of one value, printed so the local counts as used.
fn typed_let(type_name: &str, value: &str) -> String {
    main_with(&format!("    x: {type_name} = {value}\n    print(x)"))
}

/// A program that declares `lhs: left_type` and `rhs: right_type`, then runs `statement`.
fn with_two_locals(left_type: &str, right_type: &str, statement: &str) -> String {
    let body = format!("    lhs: {left_type} = 1\n    rhs: {right_type} = 2\n{statement}");
    main_with(&body)
}

/// A program that declares `val: type_name = 1`, then runs `statement`.
fn with_one_local(type_name: &str, statement: &str) -> String {
    main_with(&format!("    val: {type_name} = 1\n{statement}"))
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

// ---------------------------------------------------------------- type names

/// Every integer type name is accepted as a parameter type.
#[test]
fn every_integer_type_is_accepted_as_a_parameter_type() {
    for int in &INT_TYPES {
        let src = format!("fn f(val: {}) {{\n}}\nfn main() {{\n}}\n", int.name);
        checked(&src);
    }
}

/// Every integer type name is accepted as a return type.
#[test]
fn every_integer_type_is_accepted_as_a_return_type() {
    for int in &INT_TYPES {
        let src = format!(
            "fn f(val: {0}) -> {0} {{\n    return val\n}}\nfn main() {{\n}}\n",
            int.name
        );
        checked(&src);
    }
}

/// Every integer type name is accepted as a `let` annotation.
#[test]
fn every_integer_type_is_accepted_as_a_let_annotation() {
    for int in &INT_TYPES {
        checked(&typed_let(int.name, "1"));
    }
}

/// A parameter of each integer type is recorded with that type.
#[test]
fn parameter_types_are_recorded() {
    for int in &INT_TYPES {
        let src = format!("fn f(val: {}) {{\n}}\nfn main() {{\n}}\n", int.name);
        let fixture = fixture(&src);
        let recorded = fixture.checked.local_types[&fixture.param("val")];
        assert_eq!(recorded, int.id, "{}", int.name);
    }
}

/// A `let` annotation of each integer type gives the local that type.
#[test]
fn annotated_local_types_are_recorded() {
    for int in &INT_TYPES {
        let fixture = fixture(&typed_let(int.name, "1"));
        let recorded = fixture.checked.local_types[&fixture.stmt("x: ", 0)];
        assert_eq!(recorded, int.id, "{}", int.name);
    }
}

/// The signature of a function lists each integer type by its id.
#[test]
fn signatures_hold_the_integer_type_ids() {
    for int in &INT_TYPES {
        let src = format!(
            "fn f(val: {0}) -> {0} {{\n    return val\n}}\nfn main() {{\n}}\n",
            int.name
        );
        let expected = Signature {
            params: vec![int.id],
            ret: int.id,
        };
        assert_eq!(checked(&src).signatures["f"], expected, "{}", int.name);
    }
}

/// Checking integer programs adds no type to the table: the integer types are built in.
#[test]
fn integer_types_add_nothing_to_the_type_table() {
    let src = typed_let("u16", "1");
    assert_eq!(checked(&src).types, TypeTable::new());
}

// ---------------------------------------------------------------- literal fit

/// The largest value of every type is accepted.
#[test]
fn largest_value_of_every_type_is_accepted() {
    for int in &INT_TYPES {
        checked(&typed_let(int.name, int.max));
    }
}

/// The smallest value of every type is accepted; for signed types it is written with a minus sign.
#[test]
fn smallest_value_of_every_type_is_accepted() {
    for int in &INT_TYPES {
        checked(&typed_let(int.name, int.min));
    }
}

/// One more than the largest value is an error at the literal.
#[test]
fn literal_above_the_largest_value_is_rejected_at_the_literal() {
    for int in &INT_TYPES {
        let Some(above_max) = int.above_max else {
            continue;
        };
        let src = typed_let(int.name, above_max);
        assert_literal_does_not_fit(&src, int.name, above_max);
    }
}

/// One below the smallest value is an error at the digits, not at the minus sign.
#[test]
fn literal_below_the_smallest_value_is_rejected_at_the_digits() {
    for int in &INT_TYPES {
        let Some(magnitude) = int.below_min_magnitude else {
            continue;
        };
        let literal = format!("-{magnitude}");
        let src = typed_let(int.name, &literal);
        assert_literal_does_not_fit(&src, int.name, &literal);
    }
}

/// A negative literal for an unsigned type is an error at the digits.
#[test]
fn negative_literal_for_an_unsigned_type_is_rejected_at_the_digits() {
    for int in INT_TYPES.iter().filter(|int| !int.signed) {
        let src = typed_let(int.name, "-1");
        assert_literal_does_not_fit(&src, int.name, "-1");
    }
}

/// The minus sign belongs to the literal, so `-128` is a valid `i8`.
#[test]
fn minus_sign_belongs_to_the_literal_for_i8() {
    let fixture = fixture(&typed_let("i8", "-128"));
    assert_eq!(fixture.expr_type("-128", 0), TypeId::I8);
    assert_eq!(fixture.expr_type("128", 0), TypeId::I8);
}

/// `-129` is not a valid `i8`; the error is at `129`.
#[test]
fn minus_129_is_not_an_i8() {
    assert_literal_does_not_fit(&typed_let("i8", "-129"), "i8", "-129");
}

/// `-1` is not a valid `u8`; the error is at `1`, with exactly this message.
#[test]
fn minus_one_is_not_a_u8() {
    let src = typed_let("u8", "-1");
    let message = "integer literal does not fit in `u8`";
    let digits = span_of(&src, "-1", 0).start + 1;
    let expected = (message.to_string(), Span::new(digits, digits + 1));
    assert_eq!(errors(&src), vec![expected]);
}

/// The smallest value is accepted as an operand of a binary operator too.
#[test]
fn smallest_value_is_accepted_as_an_operand() {
    let src = with_one_local("i8", "    print(val - -128)");
    let fixture = fixture(&src);
    assert_eq!(fixture.expr_type("-128", 0), TypeId::I8);
}

/// A hexadecimal literal at the limit of its type is accepted.
#[test]
fn hexadecimal_literal_at_the_limit_is_accepted() {
    checked(&typed_let("u8", "0xFF"));
    checked(&typed_let("u64", "0xFFFF_FFFF_FFFF_FFFF"));
}

/// A hexadecimal literal above the limit of its type is an error at the literal.
#[test]
fn hexadecimal_literal_above_the_limit_is_rejected() {
    assert_literal_does_not_fit(&typed_let("u8", "0x100"), "u8", "0x100");
    assert_literal_does_not_fit(&typed_let("i8", "0x80"), "i8", "0x80");
}

/// A literal that settles to a smaller type than `i64` is checked against that type, not `i64`.
#[test]
fn literal_fit_is_checked_in_the_settled_type() {
    let src = with_one_local("u8", "    print(val + 256)");
    assert_literal_does_not_fit(&src, "u8", "256");
}

/// A literal on the left of an operator is checked against the right operand's type.
#[test]
fn literal_left_of_an_operator_is_checked_against_the_right_type() {
    let src = with_one_local("i16", "    print(32768 + val)");
    assert_literal_does_not_fit(&src, "i16", "32768");
}

/// A literal compared with a typed value is checked against that value's type.
#[test]
fn compared_literal_is_checked_in_the_other_operands_type() {
    let src = with_one_local("u8", "    print(val == 300)");
    assert_literal_does_not_fit(&src, "u8", "300");
}

/// A literal returned from a function is checked against the return type.
#[test]
fn returned_literal_is_checked_in_the_return_type() {
    let src = "fn f() -> u8 {\n    return 300\n}\nfn main() {\n}\n";
    assert_literal_does_not_fit(src, "u8", "300");
}

/// A literal assigned to a variable is checked against the variable's type.
#[test]
fn assigned_literal_is_checked_in_the_variables_type() {
    let src = with_one_local("u8", "    val = 300\n    print(val)");
    assert_literal_does_not_fit(&src, "u8", "300");
}

/// A literal in a compound assignment is checked against the target's type.
#[test]
fn compound_assigned_literal_is_checked_in_the_targets_type() {
    let src = with_one_local("u8", "    val += 256\n    print(val)");
    assert_literal_does_not_fit(&src, "u8", "256");
}

/// A negative literal in a compound assignment to an unsigned target is an error at the digits.
#[test]
fn negative_literal_in_a_compound_assignment_to_unsigned_is_rejected() {
    let src = with_one_local("u16", "    val += -1\n    print(val)");
    assert_literal_does_not_fit(&src, "u16", "-1");
}

/// A literal argument is checked against the parameter's type.
#[test]
fn argument_literal_is_checked_in_the_parameter_type() {
    let src = "fn f(val: u8) {\n}\nfn main() {\n    f(300)\n}\n";
    assert_literal_does_not_fit(src, "u8", "300");
}

/// A literal that defaults to `i64` must fit `i64`, even if it would fit `u64`.
#[test]
fn defaulted_literal_above_i64_max_is_rejected() {
    let src = main_with("    print(18446744073709551615)");
    assert_literal_does_not_fit(&src, "i64", "18446744073709551615");
}

// ---------------------------------------------------------------- defaulting

/// `x := 1` records `i64` for the literal and for the local.
#[test]
fn untyped_let_defaults_to_i64() {
    let fixture = fixture(&main_with("    x := 1\n    print(x)"));
    assert_eq!(fixture.expr_type("1", 0), TypeId::I64);
    assert_eq!(
        fixture.checked.local_types[&fixture.stmt("x := 1", 0)],
        TypeId::I64
    );
}

/// `print(1 + 2)` records `i64` for both literals and the sum.
#[test]
fn print_of_untyped_arithmetic_records_i64_on_every_node() {
    let fixture = fixture(&main_with("    print(1 + 2)"));
    assert_eq!(fixture.expr_type("1", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("2", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("1 + 2", 0), TypeId::I64);
}

/// A negated literal that defaults to `i64` records `i64` for the unary expression and the literal.
#[test]
fn print_of_a_negated_literal_records_i64() {
    let fixture = fixture(&main_with("    print(-5)"));
    assert_eq!(fixture.expr_type("-5", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("5", 0), TypeId::I64);
}

/// Two untyped literals compared with each other default to `i64`.
#[test]
fn comparison_of_two_untyped_literals_defaults_to_i64() {
    let fixture = fixture(&main_with("    print(7 < 8)"));
    assert_eq!(fixture.expr_type("7", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("8", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("7 < 8", 0), TypeId::BOOL);
}

/// An untyped sum bound with `:=` defaults to `i64` and may exceed the `i32` range.
#[test]
fn defaulted_arithmetic_may_exceed_narrow_ranges() {
    let fixture = fixture(&main_with("    y := 3000000000 * 3\n    print(y)"));
    assert_eq!(fixture.expr_type("3000000000 * 3", 0), TypeId::I64);
}

/// An untyped condition is not an integer context: it is reported as `i64`.
#[test]
fn untyped_condition_is_reported_as_i64() {
    let src = main_with("    if 1 + 2 {\n    }");
    assert_single_error(&src, "expected `bool`, found `i64`", "1 + 2", 0);
}

/// An untyped literal given to a `bool` is reported as `i64`, never as an untyped type.
#[test]
fn untyped_literal_for_a_bool_is_reported_as_i64() {
    let src = typed_let("bool", "1");
    assert_single_error(&src, "expected `bool`, found `i64`", "1", 0);
}

/// An untyped sum given to a `bool` is reported as `i64`.
#[test]
fn untyped_sum_for_a_bool_is_reported_as_i64() {
    let src = typed_let("bool", "1 + 2");
    assert_single_error(&src, "expected `bool`, found `i64`", "1 + 2", 0);
}

// ---------------------------------------------------------------- settling through contexts

/// A literal right of a typed operand takes the operand's type.
#[test]
fn literal_right_of_a_typed_operand_takes_its_type() {
    let fixture = fixture(&with_one_local("u8", "    print(val + 7)"));
    assert_eq!(fixture.expr_type("val + 7", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("7", 0), TypeId::U8);
}

/// A literal left of a typed operand takes the operand's type.
#[test]
fn literal_left_of_a_typed_operand_takes_its_type() {
    let fixture = fixture(&with_one_local("u8", "    print(7 + val)"));
    assert_eq!(fixture.expr_type("7 + val", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("7", 0), TypeId::U8);
}

/// Every arithmetic operator settles a literal to the other operand's type, for every integer type.
#[test]
fn every_arithmetic_operator_settles_the_literal() {
    for int in &INT_TYPES {
        for op in ARITHMETIC {
            let statement = format!("    print(val {op} 7)");
            let fixture = fixture(&with_one_local(int.name, &statement));
            let expr = format!("val {op} 7");
            assert_eq!(fixture.expr_type(&expr, 0), int.id, "{} {op}", int.name);
            assert_eq!(fixture.expr_type("7", 0), int.id, "{} {op}", int.name);
        }
    }
}

/// Every comparison settles a literal to the other operand's type.
#[test]
fn every_comparison_settles_the_literal() {
    for op in ORDERING.iter().chain(EQUALITY.iter()) {
        let statement = format!("    print(val {op} 7)");
        let fixture = fixture(&with_one_local("i16", &statement));
        assert_eq!(fixture.expr_type("7", 0), TypeId::I16, "{op}");
    }
}

/// `x: i16 = 1 + 2 * 3` records `i16` on all five nodes.
#[test]
fn nested_untyped_arithmetic_settles_every_node() {
    let fixture = fixture(&typed_let("i16", "7 + 4 * 5"));
    for needle in ["7 + 4 * 5", "7", "4 * 5", "4", "5"] {
        assert_eq!(fixture.expr_type(needle, 0), TypeId::I16, "{needle}");
    }
}

/// An untyped sum next to a typed operand settles as a whole.
#[test]
fn untyped_sum_next_to_a_typed_operand_settles_as_a_whole() {
    let fixture = fixture(&with_one_local("u8", "    print(val == 4 + 5)"));
    for needle in ["4 + 5", "4", "5"] {
        assert_eq!(fixture.expr_type(needle, 0), TypeId::U8, "{needle}");
    }
}

/// An untyped sum on the left of a typed operand settles as a whole too.
#[test]
fn untyped_sum_left_of_a_typed_operand_settles_as_a_whole() {
    let fixture = fixture(&with_one_local("i8", "    print(4 + 5 + val)"));
    for needle in ["4 + 5", "4", "5"] {
        assert_eq!(fixture.expr_type(needle, 0), TypeId::I8, "{needle}");
    }
}

/// A literal in a `return` takes the function's return type.
#[test]
fn returned_literal_takes_the_return_type() {
    let src = "fn f() -> u16 {\n    return 7\n}\nfn main() {\n}\n";
    assert_eq!(fixture(src).expr_type("7", 0), TypeId::U16);
}

/// A literal argument takes the parameter's type.
#[test]
fn argument_literal_takes_the_parameter_type() {
    let src = "fn f(val: i16) {\n}\nfn main() {\n    f(7)\n}\n";
    assert_eq!(fixture(src).expr_type("7", 0), TypeId::I16);
}

/// Each literal argument takes the type of its own parameter.
#[test]
fn each_argument_takes_its_own_parameter_type() {
    let src = "\
fn f(a: u8, b: i32, c: u64) {
}
fn main() {
    f(5, 7, 9)
}
";
    let fixture = fixture(src);
    assert_eq!(fixture.expr_type("5", 0), TypeId::U8);
    assert_eq!(fixture.expr_type("7", 0), TypeId::I32);
    assert_eq!(fixture.expr_type("9", 0), TypeId::U64);
}

/// A literal assigned to a variable takes the variable's type.
#[test]
fn assigned_literal_takes_the_variables_type() {
    let fixture = fixture(&with_one_local("u8", "    val = 7\n    print(val)"));
    assert_eq!(fixture.expr_type("7", 0), TypeId::U8);
}

/// A literal in a compound assignment takes the target's type.
#[test]
fn compound_assigned_literal_takes_the_targets_type() {
    for op in ["+=", "-=", "*=", "/="] {
        let statement = format!("    val {op} 7\n    print(val)");
        let fixture = fixture(&with_one_local("u32", &statement));
        assert_eq!(fixture.expr_type("7", 0), TypeId::U32, "{op}");
    }
}

/// A literal passed to `print` defaults to `i64`, whatever else is in the program.
#[test]
fn print_gives_a_literal_the_default_type() {
    let fixture = fixture(&with_one_local("u8", "    print(7)\n    print(val)"));
    assert_eq!(fixture.expr_type("7", 0), TypeId::I64);
}

// ---------------------------------------------------------------- operators

/// Arithmetic on two values of one type has that type, for every integer type.
#[test]
fn arithmetic_result_has_the_operand_type() {
    for int in &INT_TYPES {
        for op in ARITHMETIC {
            let statement = format!("    print(lhs {op} rhs)");
            let src = with_two_locals(int.name, int.name, &statement);
            let expr = format!("lhs {op} rhs");
            assert_eq!(
                fixture(&src).expr_type(&expr, 0),
                int.id,
                "{} {op}",
                int.name
            );
        }
    }
}

/// A comparison of two values of one integer type is a `bool`, for every integer type.
#[test]
fn comparison_result_is_bool() {
    for int in &INT_TYPES {
        for op in ORDERING.iter().chain(EQUALITY.iter()) {
            let statement = format!("    print(lhs {op} rhs)");
            let src = with_two_locals(int.name, int.name, &statement);
            let expr = format!("lhs {op} rhs");
            let recorded = fixture(&src).expr_type(&expr, 0);
            assert_eq!(recorded, TypeId::BOOL, "{} {op}", int.name);
        }
    }
}

/// Mismatched integer operands report the first operand's type as expected, at the right operand.
#[test]
fn mismatched_operands_are_reported_at_the_right_operand() {
    for op in ARITHMETIC.iter().chain(ORDERING.iter()) {
        let src = with_two_locals("i32", "i64", &format!("    print(lhs {op} rhs)"));
        assert_single_error(&src, "expected `i32`, found `i64`", "rhs", 1);
    }
}

/// An untyped left operand next to a typed right operand of the same width is accepted.
#[test]
fn untyped_left_operand_next_to_a_typed_right_operand_is_accepted() {
    let src = with_two_locals("u8", "u8", "    print(1 + rhs)\n    print(lhs)");
    checked(&src);
}

/// A `bool` right of a typed integer is reported with the integer's type as expected.
#[test]
fn bool_right_of_a_typed_integer_is_rejected_at_the_bool() {
    let src = with_one_local("u8", "    print(val + true)");
    assert_single_error(&src, "expected `u8`, found `bool`", "true", 0);
}

/// With only untyped operands, the expected type is the default: `1 + true` is reported against `i64`.
#[test]
fn bool_right_of_an_untyped_literal_is_reported_as_i64() {
    let src = main_with("    print(1 + true)");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// The first typed operand fixes the type: in `a + b + c`, `c` is checked against the type of `a + b`.
#[test]
fn the_first_typed_operand_fixes_the_type_of_a_chain() {
    let body = "\
    first: i8 = 1
    second: i8 = 2
    third: i16 = 3
    print(first + second + third)";
    let src = main_with(body);
    assert_single_error(&src, "expected `i8`, found `i16`", "third", 1);
}

/// Comparing two different integer types says so, naming both, at the whole expression.
#[test]
fn comparing_different_integer_types_is_rejected_at_the_whole_expression() {
    for op in EQUALITY {
        let expr = format!("lhs {op} rhs");
        let src = with_two_locals("u8", "i8", &format!("    print({expr})"));
        assert_single_error(&src, "cannot compare `u8` with `i8`", &expr, 0);
    }
}

/// Comparing an integer with a `bool` says so, naming both, at the whole expression.
#[test]
fn comparing_an_integer_with_a_bool_is_rejected() {
    let src = with_one_local("u8", "    print(val == true)");
    assert_single_error(&src, "cannot compare `u8` with `bool`", "val == true", 0);
}

/// An untyped literal compared for equality takes the other side's type.
#[test]
fn equality_settles_an_untyped_side_to_the_other_type() {
    let fixture = fixture(&with_one_local("u32", "    print(7 == val)"));
    assert_eq!(fixture.expr_type("7", 0), TypeId::U32);
}

/// `!` needs a `bool`; an integer is reported with its own type.
#[test]
fn not_of_an_integer_names_its_type() {
    let src = with_one_local("u8", "    print(!val)");
    assert_single_error(&src, "expected `bool`, found `u8`", "val", 1);
}

/// `&&` needs `bool` operands; an integer is reported with its own type.
#[test]
fn logical_and_of_an_integer_names_its_type() {
    let src = with_one_local("i16", "    print(val && true)");
    assert_single_error(&src, "expected `bool`, found `i16`", "val", 1);
}

/// An `if` condition of an integer type is reported with that type.
#[test]
fn integer_condition_names_its_type() {
    let src = with_one_local("u8", "    if val {\n    }");
    assert_single_error(&src, "expected `bool`, found `u8`", "val", 1);
}

// ---------------------------------------------------------------- negation

/// `-val` of a signed type has that type.
#[test]
fn negating_a_signed_value_keeps_its_type() {
    for int in INT_TYPES.iter().filter(|int| int.signed) {
        let fixture = fixture(&with_one_local(int.name, "    print(-val)"));
        assert_eq!(fixture.expr_type("-val", 0), int.id, "{}", int.name);
    }
}

/// `-val` of an unsigned value is an error at the unary expression.
#[test]
fn negating_an_unsigned_value_is_rejected_at_the_unary_expression() {
    for int in INT_TYPES.iter().filter(|int| !int.signed) {
        let src = with_one_local(int.name, "    print(-val)");
        let message = format!("cannot negate a value of unsigned type `{}`", int.name);
        assert_single_error(&src, &message, "-val", 0);
    }
}

/// Negating a parenthesized untyped sum for an unsigned type is an error at the whole `-(1 + 2)`, closing parenthesis included.
#[test]
fn negating_a_parenthesized_sum_for_an_unsigned_type_is_rejected_at_the_whole_expression() {
    let src = typed_let("u8", "-(1 + 2)");
    let message = "cannot negate a value of unsigned type `u8`";
    assert_single_error(&src, message, "-(1 + 2)", 0);
}

/// Negating an untyped non-literal (`- -0`) that settles to an unsigned type is an error at the outer minus.
#[test]
fn negating_an_untyped_non_literal_for_an_unsigned_type_is_rejected() {
    let src = typed_let("u8", "- -0");
    let start = span_of(&src, "- -0", 0).start;
    let expected = (
        "cannot negate a value of unsigned type `u8`".to_string(),
        Span::new(start, start + "- -0".len()),
    );
    assert_eq!(errors(&src), vec![expected]);
}

/// Negating an untyped non-literal (`- -5`) that settles to a signed type is accepted and typed on every node.
#[test]
fn negating_an_untyped_non_literal_for_a_signed_type_is_accepted() {
    let fixture = fixture(&typed_let("i8", "- -5"));
    for needle in ["- -5", "-5", "5"] {
        assert_eq!(fixture.expr_type(needle, 0), TypeId::I8, "{needle}");
    }
}

/// Negating an untyped non-literal (`- -5`) that defaults is accepted and typed `i64` on every node.
#[test]
fn negating_an_untyped_non_literal_that_defaults_is_accepted() {
    let fixture = fixture(&main_with("    print(- -5)"));
    for needle in ["- -5", "-5", "5"] {
        assert_eq!(fixture.expr_type(needle, 0), TypeId::I64, "{needle}");
    }
}

// ---------------------------------------------------------------- locals, assignment, compound assignment

/// A variable never settles: an `i64` local is not accepted where a `u8` is needed.
#[test]
fn typed_variable_does_not_convert_to_another_type() {
    let src = main_with("    wide := 5\n    x: u8 = wide\n    print(x)");
    assert_single_error(&src, "expected `u8`, found `i64`", "wide", 1);
}

/// A `bool` is not accepted where an integer type is declared.
#[test]
fn bool_value_for_an_integer_annotation_is_rejected() {
    let src = typed_let("u8", "true");
    assert_single_error(&src, "expected `u8`, found `bool`", "true", 0);
}

/// Assigning a value of another integer type is an error at the value.
#[test]
fn assigning_another_integer_type_is_rejected_at_the_value() {
    let src = with_two_locals("u8", "i8", "    lhs = rhs\n    print(lhs)");
    assert_single_error(&src, "expected `u8`, found `i8`", "rhs", 1);
}

/// A compound assignment with a value of another integer type is an error at the value.
#[test]
fn compound_assignment_of_another_integer_type_is_rejected_at_the_value() {
    for op in ["+=", "-=", "*=", "/="] {
        let src = with_two_locals("u8", "i8", &format!("    lhs {op} rhs\n    print(lhs)"));
        assert_single_error(&src, "expected `u8`, found `i8`", "rhs", 1);
    }
}

/// A compound assignment on a target of any integer type is accepted.
#[test]
fn compound_assignment_is_accepted_for_every_integer_type() {
    for int in &INT_TYPES {
        let statement = "    val += 1\n    val -= 1\n    val *= 1\n    val /= 1\n    print(val)";
        checked(&with_one_local(int.name, statement));
    }
}

// ---------------------------------------------------------------- calls

/// A function taking and returning each integer type can be called with that type's largest value.
#[test]
fn calls_are_accepted_for_every_integer_type() {
    for int in &INT_TYPES {
        let src = format!(
            "fn same(val: {0}) -> {0} {{\n    return val\n}}\nfn main() {{\n    x: {0} = same({1})\n    print(x)\n}}\n",
            int.name, int.max
        );
        checked(&src);
    }
}

/// The result of a call has the callee's return type.
#[test]
fn call_result_has_the_return_type() {
    for int in &INT_TYPES {
        let src = format!(
            "fn same(val: {0}) -> {0} {{\n    return val\n}}\nfn main() {{\n    print(same(1))\n}}\n",
            int.name
        );
        assert_eq!(
            fixture(&src).expr_type("same(1)", 0),
            int.id,
            "{}",
            int.name
        );
    }
}

/// A call result is a typed operand, so a literal next to it takes its type.
#[test]
fn literal_next_to_a_call_result_takes_its_type() {
    let src = "fn small() -> u8 {\n    return 1\n}\nfn main() {\n    print(small() + 2)\n}\n";
    assert_eq!(fixture(src).expr_type("2", 0), TypeId::U8);
}

/// Passing a variable of another integer type is an error at the argument.
#[test]
fn argument_of_another_integer_type_is_rejected_at_the_argument() {
    let src = "\
fn f(val: u8) {
}
fn main() {
    wide := 1
    f(wide)
}
";
    assert_single_error(src, "expected `u8`, found `i64`", "wide", 1);
}

/// Returning a value of another integer type is an error at the value.
#[test]
fn returning_another_integer_type_is_rejected_at_the_value() {
    let src = "fn f(val: i8) -> u8 {\n    return val\n}\nfn main() {\n}\n";
    assert_single_error(src, "expected `u8`, found `i8`", "val", 1);
}

/// A call result of one type does not fit a parameter of another.
#[test]
fn call_result_of_another_type_is_rejected_at_the_call() {
    let src = "\
fn small() -> u8 {
    return 1
}
fn take(val: i8) {
}
fn main() {
    take(small())
}
";
    assert_single_error(src, "expected `i8`, found `u8`", "small()", 1);
}

/// A signature mixing every width lists each parameter's own id.
#[test]
fn signature_with_mixed_widths_lists_each_id() {
    let src = "fn mix(a: i8, b: u16, c: i32, d: u64) -> u32 {\n    return 1\n}\nfn main() {\n}\n";
    let expected = Signature {
        params: vec![TypeId::I8, TypeId::U16, TypeId::I32, TypeId::U64],
        ret: TypeId::U32,
    };
    assert_eq!(checked(src).signatures["mix"], expected);
}

// ---------------------------------------------------------------- ranges

/// The loop variable of `for i in 0..n` has the type of `n`, whatever the integer type.
#[test]
fn loop_variable_takes_the_end_bounds_type() {
    for int in &INT_TYPES {
        let statement = "    for idx in 0..val {\n        print(idx)\n    }";
        let fixture = fixture(&with_one_local(int.name, statement));
        let recorded = fixture.checked.local_types[&fixture.stmt("for idx", 0)];
        assert_eq!(recorded, int.id, "{}", int.name);
    }
}

/// The loop variable of `for i in n..10` has the type of `n`.
#[test]
fn loop_variable_takes_the_start_bounds_type() {
    let statement = "    for idx in val..10 {\n        print(idx)\n    }";
    let fixture = fixture(&with_one_local("u16", statement));
    let recorded = fixture.checked.local_types[&fixture.stmt("for idx", 0)];
    assert_eq!(recorded, TypeId::U16);
}

/// The untyped bound of `for i in 0..n` takes the type of `n`.
#[test]
fn untyped_start_bound_takes_the_end_bounds_type() {
    let statement = "    for idx in 7..val {\n        print(idx)\n    }";
    let fixture = fixture(&with_one_local("u16", statement));
    assert_eq!(fixture.expr_type("7", 0), TypeId::U16);
}

/// The untyped bound of `for i in n..10` takes the type of `n`.
#[test]
fn untyped_end_bound_takes_the_start_bounds_type() {
    let statement = "    for idx in val..10 {\n        print(idx)\n    }";
    let fixture = fixture(&with_one_local("u16", statement));
    assert_eq!(fixture.expr_type("10", 0), TypeId::U16);
}

/// Inclusive ranges give the loop variable the bounds' type too.
#[test]
fn inclusive_range_gives_the_loop_variable_the_bounds_type() {
    let statement = "    for idx in 0..=val {\n        print(idx)\n    }";
    let fixture = fixture(&with_one_local("u8", statement));
    let recorded = fixture.checked.local_types[&fixture.stmt("for idx", 0)];
    assert_eq!(recorded, TypeId::U8);
}

/// A range of two untyped bounds is `i64`, for the variable and both bounds.
#[test]
fn range_of_untyped_bounds_defaults_to_i64() {
    let fixture = fixture(&main_with(
        "    for idx in 7..9 {\n        print(idx)\n    }",
    ));
    assert_eq!(fixture.expr_type("7", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("9", 0), TypeId::I64);
    let recorded = fixture.checked.local_types[&fixture.stmt("for idx", 0)];
    assert_eq!(recorded, TypeId::I64);
}

/// A typed expression is a valid bound, and the loop variable takes its type.
#[test]
fn typed_expression_bound_gives_the_loop_variable_its_type() {
    let statement = "    for idx in val..val + 1 {\n        print(idx)\n    }";
    let fixture = fixture(&with_one_local("u8", statement));
    let recorded = fixture.checked.local_types[&fixture.stmt("for idx", 0)];
    assert_eq!(recorded, TypeId::U8);
}

/// Bounds of different integer types are an error at the end bound, naming the start bound's type.
#[test]
fn bounds_of_different_types_are_rejected_at_the_end_bound() {
    let src = with_two_locals(
        "u8",
        "i8",
        "    for idx in lhs..rhs {\n        print(idx)\n    }",
    );
    assert_single_error(&src, "expected `u8`, found `i8`", "rhs", 1);
}

/// A `bool` end bound next to a typed start bound is an error at the bool.
#[test]
fn bool_end_bound_is_rejected_with_the_start_bounds_type() {
    let src = with_one_local(
        "u8",
        "    for idx in val..true {\n        print(idx)\n    }",
    );
    assert_single_error(&src, "expected `u8`, found `bool`", "true", 0);
}

/// An untyped bound that does not fit the other bound's type is an error at the literal.
#[test]
fn bound_literal_is_checked_in_the_other_bounds_type() {
    let src = with_one_local("u8", "    for idx in val..300 {\n        print(idx)\n    }");
    assert_literal_does_not_fit(&src, "u8", "300");
}

/// A negative untyped bound next to a signed bound is accepted and typed.
#[test]
fn negative_bound_next_to_a_signed_bound_is_accepted() {
    let statement = "    for idx in -3..val {\n        print(idx)\n    }";
    let fixture = fixture(&with_one_local("i8", statement));
    assert_eq!(fixture.expr_type("-3", 0), TypeId::I8);
}

/// A negative untyped bound next to an unsigned bound is an error at the digits.
#[test]
fn negative_bound_next_to_an_unsigned_bound_is_rejected() {
    let src = with_one_local("u8", "    for idx in -3..val {\n        print(idx)\n    }");
    assert_literal_does_not_fit(&src, "u8", "-3");
}

/// The loop variable can be assigned a value of its own type but not another.
#[test]
fn loop_variable_rejects_another_type() {
    let statement = "    for idx in 0..val {\n        idx = true\n    }";
    let src = with_one_local("u8", statement);
    assert_single_error(&src, "expected `u8`, found `bool`", "true", 0);
}

// ---------------------------------------------------------------- print

/// `print` accepts a value of every integer type.
#[test]
fn print_accepts_every_integer_type() {
    for int in &INT_TYPES {
        checked(&with_one_local(int.name, "    print(val)"));
    }
}

/// `print` of an integer expression needs no annotation.
#[test]
fn print_accepts_arithmetic_of_a_narrow_type() {
    checked(&with_one_local("i8", "    print(val * 2 - 1)"));
}

// ---------------------------------------------------------------- one error causes no second error

/// A bad operand of an untyped sum is reported once; the sum does not then fail against the `u8` annotation.
#[test]
fn bool_in_an_untyped_sum_for_a_narrow_type_gives_one_error() {
    let src = typed_let("u8", "1 + true");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// An undefined name under a minus is reported once, not again as a mismatch with the `u8` annotation.
#[test]
fn undefined_name_under_a_minus_for_a_narrow_type_gives_one_error() {
    let src = typed_let("u8", "-missing");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// An undefined name in a sum is reported once, not again as a mismatch with the `u8` annotation.
#[test]
fn undefined_name_in_a_sum_for_a_narrow_type_gives_one_error() {
    let src = typed_let("u8", "missing + 1");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// An undefined name under a minus in an argument is reported once, not again against the `u8` parameter.
#[test]
fn undefined_name_under_a_minus_in_an_argument_gives_one_error() {
    let src = "fn g(p: u8) {\n    print(p)\n}\nfn main() {\n    g(-missing)\n}\n";
    assert_single_error(src, "undefined name `missing`", "missing", 0);
}

/// An undefined name in a sum next to a typed operand is reported once, with no comparison error.
#[test]
fn undefined_name_in_a_compared_sum_gives_one_error() {
    let src = with_one_local("u8", "    print(val == missing + 1)");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// A loop over a non-range reports only that, not a mismatch when its variable is used as a `u8`.
#[test]
fn loop_over_a_non_range_gives_one_error_even_if_the_variable_is_used() {
    let src = main_with("    for idx in 5 {\n        x: u8 = idx\n        print(x)\n    }");
    assert_single_error(&src, "only ranges can be iterated over yet", "5", 0);
}

/// Negating an untyped non-literal for an unsigned type is reported once; the operand is not checked further.
#[test]
fn double_negation_for_an_unsigned_type_gives_one_error() {
    let src = typed_let("u8", "- -1");
    let start = span_of(&src, "- -1", 0).start;
    let expected = (
        "cannot negate a value of unsigned type `u8`".to_string(),
        Span::new(start, start + "- -1".len()),
    );
    assert_eq!(errors(&src), vec![expected]);
}

/// A bad operand inside a parenthesised sum is reported once, with no follow-on comparison error.
#[test]
fn bool_in_a_parenthesised_sum_that_is_compared_gives_one_error() {
    let src = main_with("    print((1 + true) == false)");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

// ---------------------------------------------------------------- nothing untyped survives

/// A program that uses an untyped literal in every context the spec lists.
const EVERY_CONTEXT: &str = "\
fn half(val: u16) -> u16 {
    return val / 2
}
fn main() {
    a: u8 = 200
    b := 7
    c: i16 = 1 + 2 * 3
    a = 10
    a += 5
    print(a + 1)
    print(1 + a)
    print(a == 15)
    print(1 < 2)
    print(- -4)
    print(half(100))
    for i in 0..a {
        print(i)
    }
    for j in 1..=3 {
        print(j + b)
    }
    print(c)
}
";

/// After checking, no expression or local has the untyped type.
#[test]
fn no_untyped_type_survives_in_a_program_using_every_context() {
    let result = checked(EVERY_CONTEXT);
    assert!(!result.expr_types.is_empty());
    for (node, id) in &result.expr_types {
        assert_ne!(*id, TypeId::UNTYPED_INT, "expression {node:?}");
    }
    for (node, id) in &result.local_types {
        assert_ne!(*id, TypeId::UNTYPED_INT, "local {node:?}");
    }
}

/// The golden programs of `tests/golden/ints` that compile pass the checker with no untyped type left.
#[test]
fn no_untyped_type_survives_in_the_golden_integer_programs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/ints");
    let mut checked_programs = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".em") || name.starts_with("err_") {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap();
        let result = check::check(&parse(&src))
            .unwrap_or_else(|diagnostics| panic!("{name} does not check: {diagnostics:?}"));
        for (node, id) in &result.expr_types {
            assert_ne!(*id, TypeId::UNTYPED_INT, "{name}: expression {node:?}");
        }
        for (node, id) in &result.local_types {
            assert_ne!(*id, TypeId::UNTYPED_INT, "{name}: local {node:?}");
        }
        checked_programs += 1;
    }
    // 14 success programs and 13 panic programs.
    assert_eq!(checked_programs, 27);
}
