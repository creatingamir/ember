//! End-to-end tests for the `arm64` crate: Ember source is compiled, assembled and linked with clang, then run.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{MAX, MIN, loc, main_with, panic_stderr, program, run};

/// Asserts that the program runs cleanly: exit code 0, nothing on stderr, exactly this stdout.
fn assert_output(src: &str, expected_stdout: &str) {
    let (stdout, stderr, code) = run(src);
    assert_eq!(stderr, "", "unexpected stderr");
    assert_eq!(code, 0, "unexpected exit code");
    assert_eq!(stdout, expected_stdout);
}

/// Asserts that `fn main` with these body lines runs cleanly and prints exactly `expected_stdout`.
fn assert_prints(body: &[&str], expected_stdout: &str) {
    assert_output(&main_with(body), expected_stdout);
}

/// Asserts a panic: exit code 101, the exact stderr for `reason` at `needle`, and the stdout printed before it.
fn assert_panics(src: &str, reason: &str, needle: &str, nth: usize, expected_stdout: &str) {
    let (stdout, stderr, code) = run(src);
    assert_eq!(stderr, panic_stderr(reason, src, needle, nth));
    assert_eq!(code, 101, "unexpected exit code");
    assert_eq!(stdout, expected_stdout);
}

/// Asserts that `a OP b`, computed from variables, prints `expected`.
fn assert_binary(a: &str, op: &str, b: &str, expected: &str) {
    let expr = format!("print(a {op} b)");
    assert_prints(
        &[&format!("a := {a}"), &format!("b := {b}"), &expr],
        &format!("{expected}\n"),
    );
}

/// Asserts that `a OP b`, computed from variables, panics with `reason` after printing 5.
fn assert_binary_panics(a: &str, op: &str, b: &str, reason: &str) {
    let src = main_with(&[
        "print(5)".to_string(),
        format!("a := {a}"),
        format!("b := {b}"),
        format!("print(a {op} b)"),
    ]);
    assert_panics(&src, reason, &format!("a {op} b"), 0, "5\n");
}

/// An empty `main` exits with 0 and prints nothing.
#[test]
fn empty_main_prints_nothing() {
    assert_prints(&[], "");
}

/// Zero prints as `0`.
#[test]
fn prints_zero() {
    assert_prints(&["print(0)"], "0\n");
}

/// A positive number prints in decimal.
#[test]
fn prints_positive_number() {
    assert_prints(&["print(42)"], "42\n");
}

/// A negative number prints with a minus sign.
#[test]
fn prints_negative_number() {
    assert_prints(&["print(-7)"], "-7\n");
}

/// The largest `i64` prints in full.
#[test]
fn prints_i64_max() {
    assert_prints(&["print(9223372036854775807)"], "9223372036854775807\n");
}

/// The smallest `i64`, written as a negated literal, prints in full.
#[test]
fn prints_i64_min_literal() {
    assert_prints(&["print(-9223372036854775808)"], "-9223372036854775808\n");
}

/// The smallest `i64` survives a round trip through a variable.
#[test]
fn prints_i64_min_through_a_variable() {
    assert_prints(
        &["x := -9223372036854775808", "print(x)"],
        "-9223372036854775808\n",
    );
}

/// A value that needs all four 16-bit chunks prints unchanged.
#[test]
fn prints_constant_with_every_chunk_set() {
    assert_prints(&["print(1311768467463790320)"], "1311768467463790320\n");
}

/// `true` prints as `true`.
#[test]
fn prints_true() {
    assert_prints(&["print(true)"], "true\n");
}

/// `false` prints as `false`.
#[test]
fn prints_false() {
    assert_prints(&["print(false)"], "false\n");
}

/// A bool stored in a variable prints correctly.
#[test]
fn prints_bool_variable() {
    assert_prints(&["b := true", "print(b)"], "true\n");
}

/// Several prints keep their order, and ints and bools mix.
#[test]
fn prints_keep_their_order() {
    assert_prints(
        &["print(1)", "print(true)", "print(-2)", "print(false)"],
        "1\ntrue\n-2\nfalse\n",
    );
}

/// Adding two positive numbers.
#[test]
fn add_positive_operands() {
    assert_binary("40", "+", "2", "42");
}

/// Adding two negative numbers.
#[test]
fn add_negative_operands() {
    assert_binary("-5", "+", "-3", "-8");
}

/// Adding numbers of different signs.
#[test]
fn add_mixed_signs() {
    assert_binary("-5", "+", "3", "-2");
}

/// A sum that lands exactly on `i64::MAX` does not panic.
#[test]
fn add_reaching_i64_max_does_not_panic() {
    assert_binary("9223372036854775806", "+", "1", MAX);
}

/// A sum that lands exactly on `i64::MIN` does not panic.
#[test]
fn add_reaching_i64_min_does_not_panic() {
    assert_binary("-9223372036854775807", "+", "-1", MIN);
}

/// Subtracting a smaller number.
#[test]
fn sub_smaller_number() {
    assert_binary("10", "-", "3", "7");
}

/// Subtracting a larger number gives a negative result.
#[test]
fn sub_larger_number() {
    assert_binary("3", "-", "10", "-7");
}

/// Subtracting a negative number.
#[test]
fn sub_negative_operand() {
    assert_binary("-5", "-", "-3", "-2");
}

/// A difference that lands exactly on `i64::MIN` does not panic.
#[test]
fn sub_reaching_i64_min_does_not_panic() {
    assert_binary("-9223372036854775807", "-", "1", MIN);
}

/// Subtracting `i64::MAX` from zero does not panic.
#[test]
fn sub_i64_max_from_zero_does_not_panic() {
    assert_binary("0", "-", MAX, "-9223372036854775807");
}

/// Multiplying two positive numbers.
#[test]
fn mul_positive_operands() {
    assert_binary("6", "*", "7", "42");
}

/// Multiplying by a negative number.
#[test]
fn mul_one_negative_operand() {
    assert_binary("-6", "*", "7", "-42");
}

/// Multiplying two negative numbers.
#[test]
fn mul_two_negative_operands() {
    assert_binary("-6", "*", "-7", "42");
}

/// Multiplying by zero.
#[test]
fn mul_by_zero() {
    assert_binary("0", "*", "-5", "0");
}

/// The largest square that fits in an `i64` does not panic.
#[test]
fn mul_largest_fitting_square_does_not_panic() {
    assert_binary("3037000499", "*", "3037000499", "9223372030926249001");
}

/// The same square, negated, does not panic.
#[test]
fn mul_large_negative_result_does_not_panic() {
    assert_binary("-3037000499", "*", "3037000499", "-9223372030926249001");
}

/// `i64::MIN * 1` does not panic.
#[test]
fn mul_i64_min_by_one_does_not_panic() {
    assert_binary(MIN, "*", "1", MIN);
}

/// `i64::MAX * -1` does not panic.
#[test]
fn mul_i64_max_by_minus_one_does_not_panic() {
    assert_binary(MAX, "*", "-1", "-9223372036854775807");
}

/// Division of positive numbers drops the fraction.
#[test]
fn div_positive_operands() {
    assert_binary("7", "/", "2", "3");
}

/// A negative dividend truncates toward zero: `-7 / 2 == -3`.
#[test]
fn div_negative_dividend_truncates_toward_zero() {
    assert_binary("-7", "/", "2", "-3");
}

/// A negative divisor truncates toward zero: `7 / -2 == -3`.
#[test]
fn div_negative_divisor_truncates_toward_zero() {
    assert_binary("7", "/", "-2", "-3");
}

/// Two negative operands give a positive result: `-7 / -2 == 3`.
#[test]
fn div_two_negative_operands() {
    assert_binary("-7", "/", "-2", "3");
}

/// Zero divided by a number is zero.
#[test]
fn div_zero_dividend() {
    assert_binary("0", "/", "5", "0");
}

/// `i64::MIN / 1` does not panic.
#[test]
fn div_i64_min_by_one_does_not_panic() {
    assert_binary(MIN, "/", "1", MIN);
}

/// `i64::MIN / 2` is exact.
#[test]
fn div_i64_min_by_two() {
    assert_binary(MIN, "/", "2", "-4611686018427387904");
}

/// `i64::MAX / -1` does not panic.
#[test]
fn div_i64_max_by_minus_one_does_not_panic() {
    assert_binary(MAX, "/", "-1", "-9223372036854775807");
}

/// Remainder of positive numbers.
#[test]
fn rem_positive_operands() {
    assert_binary("7", "%", "2", "1");
}

/// The remainder takes the sign of the dividend: `-7 % 2 == -1`.
#[test]
fn rem_negative_dividend_keeps_its_sign() {
    assert_binary("-7", "%", "2", "-1");
}

/// A negative divisor does not change the sign: `7 % -2 == 1`.
#[test]
fn rem_negative_divisor() {
    assert_binary("7", "%", "-2", "1");
}

/// Two negative operands: `-7 % -2 == -1`.
#[test]
fn rem_two_negative_operands() {
    assert_binary("-7", "%", "-2", "-1");
}

/// An exact division leaves no remainder.
#[test]
fn rem_exact_division_is_zero() {
    assert_binary("6", "%", "3", "0");
}

/// `i64::MIN % 2` is zero.
#[test]
fn rem_i64_min_by_two_is_zero() {
    assert_binary(MIN, "%", "2", "0");
}

/// Multiplication binds tighter than addition.
#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_prints(&["print(2 + 3 * 4)"], "14\n");
}

/// Parentheses override precedence.
#[test]
fn parentheses_override_precedence() {
    assert_prints(&["print((2 + 3) * 4)"], "20\n");
}

/// Subtraction is left associative.
#[test]
fn subtraction_is_left_associative() {
    assert_prints(&["print(10 - 3 - 2)"], "5\n");
}

/// A variable can be reassigned.
#[test]
fn assignment_replaces_the_value() {
    assert_prints(&["x := 1", "x = 9", "print(x)"], "9\n");
}

/// `+=` adds to a variable.
#[test]
fn add_assign() {
    assert_prints(&["x := 10", "x += 5", "print(x)"], "15\n");
}

/// `-=` subtracts from a variable.
#[test]
fn sub_assign() {
    assert_prints(&["x := 10", "x -= 4", "print(x)"], "6\n");
}

/// `*=` multiplies a variable.
#[test]
fn mul_assign() {
    assert_prints(&["x := 10", "x *= 3", "print(x)"], "30\n");
}

/// `/=` divides a variable.
#[test]
fn div_assign() {
    assert_prints(&["x := -7", "x /= 2", "print(x)"], "-3\n");
}

/// `==` on equal numbers is true.
#[test]
fn eq_true() {
    assert_binary("3", "==", "3", "true");
}

/// `==` on different numbers is false.
#[test]
fn eq_false() {
    assert_binary("3", "==", "4", "false");
}

/// `==` on equal negative numbers is true.
#[test]
fn eq_negative_numbers() {
    assert_binary("-5", "==", "-5", "true");
}

/// `!=` on different numbers is true.
#[test]
fn ne_true() {
    assert_binary("3", "!=", "4", "true");
}

/// `!=` on equal numbers is false.
#[test]
fn ne_false() {
    assert_binary("3", "!=", "3", "false");
}

/// `<` on a smaller left side is true.
#[test]
fn lt_true() {
    assert_binary("3", "<", "4", "true");
}

/// `<` on equal numbers is false.
#[test]
fn lt_false_when_equal() {
    assert_binary("3", "<", "3", "false");
}

/// `<` on a larger left side is false.
#[test]
fn lt_false_when_greater() {
    assert_binary("4", "<", "3", "false");
}

/// `<=` on equal numbers is true.
#[test]
fn le_true_when_equal() {
    assert_binary("3", "<=", "3", "true");
}

/// `<=` on a smaller left side is true.
#[test]
fn le_true_when_smaller() {
    assert_binary("2", "<=", "3", "true");
}

/// `<=` on a larger left side is false.
#[test]
fn le_false() {
    assert_binary("4", "<=", "3", "false");
}

/// `>` on a larger left side is true.
#[test]
fn gt_true() {
    assert_binary("4", ">", "3", "true");
}

/// `>` on equal numbers is false.
#[test]
fn gt_false_when_equal() {
    assert_binary("3", ">", "3", "false");
}

/// `>` on a smaller left side is false.
#[test]
fn gt_false_when_smaller() {
    assert_binary("2", ">", "3", "false");
}

/// `>=` on equal numbers is true.
#[test]
fn ge_true_when_equal() {
    assert_binary("3", ">=", "3", "true");
}

/// `>=` on a larger left side is true.
#[test]
fn ge_true_when_greater() {
    assert_binary("4", ">=", "3", "true");
}

/// `>=` on a smaller left side is false.
#[test]
fn ge_false() {
    assert_binary("3", ">=", "4", "false");
}

/// Comparisons are signed: a negative number is less than a positive one.
#[test]
fn comparison_is_signed_for_negative_against_positive() {
    assert_binary("-1", "<", "1", "true");
}

/// Comparisons are signed between two negative numbers.
#[test]
fn comparison_is_signed_between_negatives() {
    assert_binary("-2", "<", "-1", "true");
}

/// `i64::MIN < i64::MAX`.
#[test]
fn i64_min_is_less_than_i64_max() {
    assert_binary(MIN, "<", MAX, "true");
}

/// `i64::MAX > i64::MIN`.
#[test]
fn i64_max_is_greater_than_i64_min() {
    assert_binary(MAX, ">", MIN, "true");
}

/// `!true` is false.
#[test]
fn not_true() {
    assert_prints(&["b := true", "print(!b)"], "false\n");
}

/// `!false` is true.
#[test]
fn not_false() {
    assert_prints(&["b := false", "print(!b)"], "true\n");
}

/// `!` applies to a comparison result.
#[test]
fn not_of_a_comparison() {
    assert_prints(&["a := 1", "b := 2", "print(!(a < b))"], "false\n");
}

/// Unary minus on a positive number.
#[test]
fn neg_positive() {
    assert_prints(&["x := 5", "print(-x)"], "-5\n");
}

/// Unary minus on a negative number.
#[test]
fn neg_negative() {
    assert_prints(&["x := -5", "print(-x)"], "5\n");
}

/// Unary minus on zero.
#[test]
fn neg_zero() {
    assert_prints(&["x := 0", "print(-x)"], "0\n");
}

/// Negating `-i64::MAX` gives `i64::MAX` without a panic.
#[test]
fn neg_of_negated_i64_max_does_not_panic() {
    assert_prints(
        &["x := -9223372036854775807", "print(-x)"],
        "9223372036854775807\n",
    );
}

/// Source for two functions that print as a visible side effect: `yes` prints 1, `no` prints 2.
const PROBES: &[&str] = &[
    "fn yes() -> bool {",
    "    print(1)",
    "    return true",
    "}",
    "fn no() -> bool {",
    "    print(2)",
    "    return false",
    "}",
];

/// Builds a program with the probe functions and a `main` with the given body.
fn with_probes(body: &[&str]) -> String {
    format!("{}{}", program(PROBES), main_with(body))
}

/// `false && yes()` skips the call.
#[test]
fn and_skips_the_right_side_when_left_is_false() {
    let src = with_probes(&["a := false", "r := a && yes()", "print(r)"]);
    assert_output(&src, "false\n");
}

/// `true && yes()` runs the call.
#[test]
fn and_runs_the_right_side_when_left_is_true() {
    let src = with_probes(&["a := true", "r := a && yes()", "print(r)"]);
    assert_output(&src, "1\ntrue\n");
}

/// `true && no()` runs the call and is false.
#[test]
fn and_is_false_when_the_right_side_is_false() {
    let src = with_probes(&["a := true", "r := a && no()", "print(r)"]);
    assert_output(&src, "2\nfalse\n");
}

/// `true || no()` skips the call.
#[test]
fn or_skips_the_right_side_when_left_is_true() {
    let src = with_probes(&["a := true", "r := a || no()", "print(r)"]);
    assert_output(&src, "true\n");
}

/// `false || yes()` runs the call.
#[test]
fn or_runs_the_right_side_when_left_is_false() {
    let src = with_probes(&["a := false", "r := a || yes()", "print(r)"]);
    assert_output(&src, "1\ntrue\n");
}

/// `false || no()` runs the call and is false.
#[test]
fn or_is_false_when_both_sides_are_false() {
    let src = with_probes(&["a := false", "r := a || no()", "print(r)"]);
    assert_output(&src, "2\nfalse\n");
}

/// `&&` truth table: true and true.
#[test]
fn and_true_true() {
    assert_prints(&["a := true", "b := true", "print(a && b)"], "true\n");
}

/// `&&` truth table: true and false.
#[test]
fn and_true_false() {
    assert_prints(&["a := true", "b := false", "print(a && b)"], "false\n");
}

/// `&&` truth table: false and true.
#[test]
fn and_false_true() {
    assert_prints(&["a := false", "b := true", "print(a && b)"], "false\n");
}

/// `||` truth table: true and false.
#[test]
fn or_true_false() {
    assert_prints(&["a := true", "b := false", "print(a || b)"], "true\n");
}

/// `||` truth table: false and true.
#[test]
fn or_false_true() {
    assert_prints(&["a := false", "b := true", "print(a || b)"], "true\n");
}

/// `&&` binds tighter than `||`: `true || false && false` is true.
#[test]
fn and_binds_tighter_than_or() {
    assert_prints(
        &[
            "a := true",
            "b := false",
            "c := false",
            "print(a || b && c)",
        ],
        "true\n",
    );
}

/// A guarded division in an `if` condition never runs the division.
#[test]
fn and_guard_prevents_a_division_by_zero() {
    assert_prints(
        &[
            "x := 0",
            "if x != 0 && 10 / x > 1 {",
            "print(1)",
            "} else {",
            "print(2)",
            "}",
        ],
        "2\n",
    );
}

/// A guarded division with `||` never runs the division either.
#[test]
fn or_guard_prevents_a_division_by_zero() {
    assert_prints(
        &[
            "x := 0",
            "if x == 0 || 10 / x > 1 {",
            "print(1)",
            "} else {",
            "print(2)",
            "}",
        ],
        "1\n",
    );
}

/// Call arguments are evaluated left to right.
#[test]
fn call_arguments_are_evaluated_left_to_right() {
    let src = with_probes(&["x := yes()", "y := no()", "print(x)", "print(y)"]);
    assert_output(&src, "1\n2\ntrue\nfalse\n");
}

/// A true condition runs the then-branch.
#[test]
fn if_runs_the_then_branch() {
    assert_prints(
        &["x := 1", "if x == 1 {", "print(10)", "}", "print(99)"],
        "10\n99\n",
    );
}

/// A false condition without `else` skips the branch.
#[test]
fn if_without_else_skips_the_branch() {
    assert_prints(
        &["x := 2", "if x == 1 {", "print(10)", "}", "print(99)"],
        "99\n",
    );
}

/// A false condition runs the else-branch.
#[test]
fn if_else_runs_the_else_branch() {
    assert_prints(
        &[
            "x := 2",
            "if x == 1 {",
            "print(10)",
            "} else {",
            "print(20)",
            "}",
        ],
        "20\n",
    );
}

/// Source for `classify`, an if / else-if / else chain that prints -1, 0 or 1.
const CLASSIFY: &[&str] = &[
    "fn classify(n: i64) {",
    "    if n < 0 {",
    "        print(-1)",
    "    } else if n == 0 {",
    "        print(0)",
    "    } else {",
    "        print(1)",
    "    }",
    "}",
];

/// Builds a program that calls `classify` with the argument.
fn classify_program(argument: &str) -> String {
    format!(
        "{}{}",
        program(CLASSIFY),
        main_with(&[format!("classify({argument})")])
    )
}

/// The first branch of an else-if chain.
#[test]
fn else_if_chain_takes_the_first_branch() {
    assert_output(&classify_program("-5"), "-1\n");
}

/// The middle branch of an else-if chain.
#[test]
fn else_if_chain_takes_the_middle_branch() {
    assert_output(&classify_program("0"), "0\n");
}

/// The final else of an else-if chain.
#[test]
fn else_if_chain_takes_the_final_else() {
    assert_output(&classify_program("9"), "1\n");
}

/// An else-if chain without a final else falls through when nothing matches.
#[test]
fn else_if_chain_without_else_can_match_nothing() {
    assert_prints(
        &[
            "x := 7",
            "if x == 1 {",
            "print(1)",
            "} else if x == 2 {",
            "print(2)",
            "}",
            "print(0)",
        ],
        "0\n",
    );
}

/// Nested ifs choose the inner branch.
#[test]
fn nested_if_runs_the_inner_branch() {
    assert_prints(
        &[
            "a := 1",
            "b := 2",
            "if a == 1 {",
            "if b == 3 {",
            "print(1)",
            "} else {",
            "print(2)",
            "}",
            "} else {",
            "print(3)",
            "}",
        ],
        "2\n",
    );
}

/// `for {}` runs until `break`.
#[test]
fn infinite_loop_ends_with_break() {
    assert_prints(
        &[
            "i := 0",
            "for {",
            "if i == 3 {",
            "break",
            "}",
            "print(i)",
            "i += 1",
            "}",
        ],
        "0\n1\n2\n",
    );
}

/// `continue` in `for {}` jumps back to the start of the body.
#[test]
fn infinite_loop_continue() {
    assert_prints(
        &[
            "i := 0",
            "for {",
            "i += 1",
            "if i > 5 {",
            "break",
            "}",
            "if i % 2 == 0 {",
            "continue",
            "}",
            "print(i)",
            "}",
        ],
        "1\n3\n5\n",
    );
}

/// `for cond {}` runs while the condition holds.
#[test]
fn while_loop_runs_while_the_condition_holds() {
    assert_prints(
        &["i := 0", "for i < 3 {", "print(i)", "i += 1", "}"],
        "0\n1\n2\n",
    );
}

/// `for cond {}` with a false condition never runs the body.
#[test]
fn while_loop_with_false_condition_never_runs() {
    assert_prints(
        &[
            "i := 5",
            "for i < 3 {",
            "print(i)",
            "i += 1",
            "}",
            "print(99)",
        ],
        "99\n",
    );
}

/// `continue` in `for cond {}` re-checks the condition.
#[test]
fn while_loop_continue() {
    assert_prints(
        &[
            "i := 0",
            "for i < 6 {",
            "i += 1",
            "if i % 2 == 0 {",
            "continue",
            "}",
            "print(i)",
            "}",
        ],
        "1\n3\n5\n",
    );
}

/// `break` in `for cond {}` leaves the loop.
#[test]
fn while_loop_break() {
    assert_prints(
        &[
            "i := 0",
            "for i < 10 {",
            "if i == 2 {",
            "break",
            "}",
            "print(i)",
            "i += 1",
            "}",
            "print(99)",
        ],
        "0\n1\n99\n",
    );
}

/// `for i in a..b` excludes the end.
#[test]
fn exclusive_range_excludes_the_end() {
    assert_prints(&["for i in 0..3 {", "print(i)", "}"], "0\n1\n2\n");
}

/// An empty exclusive range runs zero times.
#[test]
fn exclusive_range_with_equal_bounds_is_empty() {
    assert_prints(&["for i in 3..3 {", "print(i)", "}", "print(99)"], "99\n");
}

/// A reversed exclusive range runs zero times.
#[test]
fn exclusive_range_reversed_is_empty() {
    assert_prints(&["for i in 5..2 {", "print(i)", "}", "print(99)"], "99\n");
}

/// Ranges can span negative numbers.
#[test]
fn exclusive_range_with_negative_start() {
    assert_prints(&["for i in -2..2 {", "print(i)", "}"], "-2\n-1\n0\n1\n");
}

/// Range bounds can be variables.
#[test]
fn exclusive_range_with_variable_bounds() {
    assert_prints(
        &["a := 1", "b := 4", "for i in a..b {", "print(i)", "}"],
        "1\n2\n3\n",
    );
}

/// `continue` in an exclusive range still advances the counter.
#[test]
fn exclusive_range_continue() {
    assert_prints(
        &[
            "for i in 0..6 {",
            "if i % 2 == 0 {",
            "continue",
            "}",
            "print(i)",
            "}",
        ],
        "1\n3\n5\n",
    );
}

/// `break` in an exclusive range leaves the loop.
#[test]
fn exclusive_range_break() {
    assert_prints(
        &[
            "for i in 0..10 {",
            "if i == 3 {",
            "break",
            "}",
            "print(i)",
            "}",
            "print(99)",
        ],
        "0\n1\n2\n99\n",
    );
}

/// `for i in a..=b` includes the end.
#[test]
fn inclusive_range_includes_the_end() {
    assert_prints(&["for i in 0..=3 {", "print(i)", "}"], "0\n1\n2\n3\n");
}

/// An inclusive range with equal bounds runs once.
#[test]
fn inclusive_range_with_equal_bounds_runs_once() {
    assert_prints(&["for i in 3..=3 {", "print(i)", "}"], "3\n");
}

/// A reversed inclusive range runs zero times.
#[test]
fn inclusive_range_reversed_is_empty() {
    assert_prints(&["for i in 5..=2 {", "print(i)", "}", "print(99)"], "99\n");
}

/// `continue` in an inclusive range still advances the counter.
#[test]
fn inclusive_range_continue() {
    assert_prints(
        &[
            "for i in 0..=5 {",
            "if i % 2 == 0 {",
            "continue",
            "}",
            "print(i)",
            "}",
        ],
        "1\n3\n5\n",
    );
}

/// `break` in an inclusive range leaves the loop.
#[test]
fn inclusive_range_break() {
    assert_prints(
        &[
            "for i in 0..=10 {",
            "if i == 2 {",
            "break",
            "}",
            "print(i)",
            "}",
            "print(99)",
        ],
        "0\n1\n99\n",
    );
}

/// An inclusive range ending at `i64::MAX` prints both values and ends without a panic.
#[test]
fn inclusive_range_up_to_i64_max_ends_without_panic() {
    assert_prints(
        &[
            "for i in 9223372036854775806..=9223372036854775807 {",
            "print(i)",
            "}",
        ],
        "9223372036854775806\n9223372036854775807\n",
    );
}

/// `continue` on the last value of an inclusive range ending at `i64::MAX` also ends cleanly.
#[test]
fn inclusive_range_up_to_i64_max_with_continue() {
    assert_prints(
        &[
            "for i in 9223372036854775806..=9223372036854775807 {",
            "if i == 9223372036854775807 {",
            "continue",
            "}",
            "print(i)",
            "}",
            "print(99)",
        ],
        "9223372036854775806\n99\n",
    );
}

/// An exclusive range ending at `i64::MAX` stops before it.
#[test]
fn exclusive_range_up_to_i64_max_stops_before_it() {
    assert_prints(
        &[
            "for i in 9223372036854775806..9223372036854775807 {",
            "print(i)",
            "}",
        ],
        "9223372036854775806\n",
    );
}

/// Nested loops run the inner loop fully for each outer value.
#[test]
fn nested_loops() {
    assert_prints(
        &[
            "for i in 0..3 {",
            "for j in 0..3 {",
            "print(i * 10 + j)",
            "}",
            "}",
        ],
        "0\n1\n2\n10\n11\n12\n20\n21\n22\n",
    );
}

/// `break` in an inner loop leaves only the inner loop.
#[test]
fn break_in_nested_loop_leaves_only_the_inner_loop() {
    assert_prints(
        &[
            "for i in 0..3 {",
            "for j in 0..10 {",
            "if j == 2 {",
            "break",
            "}",
            "print(i * 10 + j)",
            "}",
            "}",
        ],
        "0\n1\n10\n11\n20\n21\n",
    );
}

/// `continue` in an inner loop skips only the inner iteration.
#[test]
fn continue_in_nested_loop_skips_only_the_inner_iteration() {
    assert_prints(
        &[
            "for i in 0..2 {",
            "for j in 0..3 {",
            "if j == 1 {",
            "continue",
            "}",
            "print(i * 10 + j)",
            "}",
            "}",
        ],
        "0\n2\n10\n12\n",
    );
}

/// Loops of different forms nest.
#[test]
fn mixed_loop_forms_nest() {
    assert_prints(
        &[
            "n := 0",
            "for n < 2 {",
            "for i in 0..=1 {",
            "print(n * 10 + i)",
            "}",
            "n += 1",
            "}",
        ],
        "0\n1\n10\n11\n",
    );
}

/// Source for a recursive `fib`.
const FIB: &[&str] = &[
    "fn fib(n: i64) -> i64 {",
    "    if n < 2 {",
    "        return n",
    "    }",
    "    return fib(n - 1) + fib(n - 2)",
    "}",
];

/// Recursion works: `fib(20) == 6765`.
#[test]
fn recursive_fib_of_20() {
    let src = format!("{}{}", program(FIB), main_with(&["print(fib(20))"]));
    assert_output(&src, "6765\n");
}

/// The base cases of `fib`.
#[test]
fn recursive_fib_base_cases() {
    let src = format!(
        "{}{}",
        program(FIB),
        main_with(&["print(fib(0))", "print(fib(1))"])
    );
    assert_output(&src, "0\n1\n");
}

/// Recursion with a product: `fact(10) == 3628800`.
#[test]
fn recursive_factorial() {
    let src = program(&[
        "fn fact(n: i64) -> i64 {",
        "    if n <= 1 {",
        "        return 1",
        "    }",
        "    return n * fact(n - 1)",
        "}",
        "fn main() {",
        "    print(fact(10))",
        "}",
    ]);
    assert_output(&src, "3628800\n");
}

/// Mutual recursion, with a call before the definition.
#[test]
fn mutual_recursion_with_forward_reference() {
    let src = program(&[
        "fn main() {",
        "    print(is_even(10))",
        "    print(is_even(7))",
        "}",
        "fn is_even(n: i64) -> bool {",
        "    if n == 0 {",
        "        return true",
        "    }",
        "    return is_odd(n - 1)",
        "}",
        "fn is_odd(n: i64) -> bool {",
        "    if n == 0 {",
        "        return false",
        "    }",
        "    return is_even(n - 1)",
        "}",
    ]);
    assert_output(&src, "true\nfalse\n");
}

/// Deep recursion (10000 frames) works: the sum of 1 to 10000 is 50005000.
#[test]
fn deep_recursion() {
    let src = program(&[
        "fn sum(n: i64) -> i64 {",
        "    if n == 0 {",
        "        return 0",
        "    }",
        "    return n + sum(n - 1)",
        "}",
        "fn main() {",
        "    print(sum(10000))",
        "}",
    ]);
    assert_output(&src, "50005000\n");
}

/// Calls nest as arguments.
#[test]
fn nested_calls_as_arguments() {
    let src = program(&[
        "fn add(a: i64, b: i64) -> i64 {",
        "    return a + b",
        "}",
        "fn main() {",
        "    print(add(add(1, 2), add(3, 4)))",
        "}",
    ]);
    assert_output(&src, "10\n");
}

/// A function without a value can return early.
#[test]
fn early_return_from_a_function_without_a_value() {
    let src = program(&[
        "fn show(n: i64) {",
        "    if n < 0 {",
        "        return",
        "    }",
        "    print(n)",
        "}",
        "fn main() {",
        "    show(-1)",
        "    show(4)",
        "}",
    ]);
    assert_output(&src, "4\n");
}

/// `return` inside a loop leaves the function.
#[test]
fn return_from_inside_a_loop() {
    let src = program(&[
        "fn first_square_over(limit: i64) -> i64 {",
        "    for i in 1..100 {",
        "        if i * i > limit {",
        "            return i",
        "        }",
        "    }",
        "    return 0",
        "}",
        "fn main() {",
        "    print(first_square_over(50))",
        "}",
    ]);
    assert_output(&src, "8\n");
}

/// Locals survive a call: the caller's slots are not clobbered.
#[test]
fn locals_survive_a_call() {
    let src = program(&[
        "fn work(a: i64) -> i64 {",
        "    b := a * 2",
        "    c := b + 1",
        "    return c",
        "}",
        "fn main() {",
        "    x := 5",
        "    y := 6",
        "    z := work(x)",
        "    print(x)",
        "    print(y)",
        "    print(z)",
        "}",
    ]);
    assert_output(&src, "5\n6\n11\n");
}

/// Source for `weigh`, an 8-parameter function whose result depends on the parameter order.
const WEIGH: &[&str] = &[
    "fn weigh(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64) -> i64 {",
    "    return a * 1 + b * 10 + c * 100 + d * 1000 + e * 10000 + f * 100000 + g * 1000000 + h * 10000000",
    "}",
    "fn outer(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64) -> i64 {",
    "    x := weigh(h, g, f, e, d, c, b, a)",
    "    return x + a",
    "}",
    "fn flag(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: bool) -> bool {",
    "    return h && g == 7",
    "}",
];

/// Builds a program from the `weigh` functions and a `main` body.
fn weigh_program(body: &[&str]) -> String {
    format!("{}{}", program(WEIGH), main_with(body))
}

/// Eight arguments arrive in order: the weighted sum is 87654321.
#[test]
fn eight_parameters_keep_their_order() {
    let src = weigh_program(&["print(weigh(1, 2, 3, 4, 5, 6, 7, 8))"]);
    assert_output(&src, "87654321\n");
}

/// Eight parameters stay intact across a nested call with reversed arguments.
#[test]
fn eight_parameters_survive_a_nested_call() {
    let src = weigh_program(&["print(outer(1, 2, 3, 4, 5, 6, 7, 8))"]);
    assert_output(&src, "12345679\n");
}

/// A bool in the eighth position arrives intact.
#[test]
fn eighth_parameter_can_be_a_bool() {
    let src = weigh_program(&["print(flag(1, 2, 3, 4, 5, 6, 7, true))"]);
    assert_output(&src, "true\n");
}

/// Builds the lines of a `main` body with `count` locals, each assigned a distinct value and printed.
fn locals_lines(count: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for i in 0..count {
        lines.push(format!("v{i} := {}", i * 7 + 3));
    }
    for i in 0..count {
        lines.push(format!("print(v{i})"));
    }
    lines
}

/// The stdout `locals_lines(count)` must produce.
fn locals_output(count: usize) -> String {
    (0..count).map(|i| format!("{}\n", i * 7 + 3)).collect()
}

/// A function with 600 locals (a frame far above 4095 bytes) keeps every value apart.
#[test]
fn frame_larger_than_4095_bytes_keeps_every_local() {
    let src = main_with(&locals_lines(600));
    assert_output(&src, &locals_output(600));
}

/// The sum over 600 locals is right, so loads and stores pair up across the whole frame.
#[test]
fn frame_larger_than_4095_bytes_sums_every_local() {
    let mut lines: Vec<String> = (0..600).map(|i| format!("v{i} := {}", i * 7 + 3)).collect();
    lines.push("total := 0".to_string());
    lines.extend((0..600).map(|i| format!("total += v{i}")));
    lines.push("print(total)".to_string());

    let expected: usize = (0..600).map(|i| i * 7 + 3).sum();
    assert_output(&main_with(&lines), &format!("{expected}\n"));
}

/// Frames just below and just above the 4095-byte limit both work.
#[test]
fn frames_around_the_4095_byte_limit_work() {
    // Each local costs 3 words (slot, constant temp, load temp), so these counts straddle 4095 bytes.
    for count in 168..=174 {
        let src = main_with(&locals_lines(count));
        assert_output(&src, &locals_output(count));
    }
}

/// A big frame in a callee works with calls from a small caller, and the caller's locals survive.
#[test]
fn big_frame_in_a_callee_leaves_the_caller_intact() {
    let mut lines = vec!["fn big() -> i64 {".to_string()];
    lines.extend((0..600).map(|i| format!("v{i} := {i}")));
    lines.push("total := 0".to_string());
    lines.extend((0..600).map(|i| format!("total += v{i}")));
    lines.push("return total".to_string());
    lines.push("}".to_string());
    lines.extend(
        [
            "fn main() {",
            "x := 11",
            "y := big()",
            "print(x)",
            "print(y)",
            "}",
        ]
        .map(String::from),
    );

    let expected: usize = (0..600).sum();
    assert_output(&program(&lines), &format!("11\n{expected}\n"));
}

/// A user function named `exit` does not call libc `exit`.
#[test]
fn user_function_named_exit() {
    let src = program(&[
        "fn exit(code: i64) {",
        "    print(code)",
        "}",
        "fn main() {",
        "    exit(3)",
        "    print(4)",
        "}",
    ]);
    assert_output(&src, "3\n4\n");
}

/// A user function named `printf` does not call libc `printf`.
#[test]
fn user_function_named_printf() {
    let src = program(&[
        "fn printf(a: i64, b: i64) -> i64 {",
        "    return a + b",
        "}",
        "fn main() {",
        "    print(printf(2, 3))",
        "}",
    ]);
    assert_output(&src, "5\n");
}

/// A user function named `puts` does not call libc `puts`.
#[test]
fn user_function_named_puts() {
    let src = program(&[
        "fn puts() {",
        "    print(11)",
        "}",
        "fn main() {",
        "    puts()",
        "}",
    ]);
    assert_output(&src, "11\n");
}

/// An add overflow reports the exact message, location and exit code, after earlier output.
#[test]
fn add_overflow_has_the_exact_message() {
    let src = main_with(&[
        "print(5)",
        "a := 9223372036854775807",
        "b := 1",
        "print(a + b)",
    ]);
    let (stdout, stderr, code) = run(&src);
    assert_eq!(stdout, "5\n");
    assert_eq!(stderr, "panic: integer overflow at t.em:5:7\n");
    assert_eq!(code, 101);
}

/// Adding past `i64::MAX` panics.
#[test]
fn add_overflow_past_i64_max() {
    assert_binary_panics(MAX, "+", "1", "integer overflow");
}

/// Adding past `i64::MIN` panics.
#[test]
fn add_overflow_past_i64_min() {
    assert_binary_panics(MIN, "+", "-1", "integer overflow");
}

/// Subtracting past `i64::MIN` panics.
#[test]
fn sub_overflow_past_i64_min() {
    assert_binary_panics(MIN, "-", "1", "integer overflow");
}

/// Subtracting past `i64::MAX` panics.
#[test]
fn sub_overflow_past_i64_max() {
    assert_binary_panics("0", "-", MIN, "integer overflow");
}

/// Multiplying past `i64::MAX` panics.
#[test]
fn mul_overflow_past_i64_max() {
    assert_binary_panics(MAX, "*", "2", "integer overflow");
}

/// `i64::MIN * -1` panics.
#[test]
fn mul_overflow_i64_min_by_minus_one() {
    assert_binary_panics(MIN, "*", "-1", "integer overflow");
}

/// The first square above `i64::MAX` panics.
#[test]
fn mul_overflow_first_square_above_the_limit() {
    assert_binary_panics("3037000500", "*", "3037000500", "integer overflow");
}

/// A large negative product beyond `i64::MIN` panics.
#[test]
fn mul_overflow_past_i64_min() {
    assert_binary_panics("-3037000500", "*", "3037000500", "integer overflow");
}

/// Negating `i64::MIN` through a variable panics at the unary expression.
#[test]
fn neg_overflow_of_i64_min() {
    let src = main_with(&[
        "print(5)",
        "x := -9223372036854775808",
        "y := -x",
        "print(y)",
    ]);
    assert_panics(&src, "integer overflow", "-x", 0, "5\n");
}

/// Dividing by zero panics.
#[test]
fn div_by_zero() {
    assert_binary_panics("10", "/", "0", "division by zero");
}

/// A remainder by zero panics.
#[test]
fn rem_by_zero() {
    assert_binary_panics("10", "%", "0", "division by zero");
}

/// `i64::MIN / -1` panics with an overflow.
#[test]
fn div_overflow_i64_min_by_minus_one() {
    assert_binary_panics(MIN, "/", "-1", "integer overflow");
}

/// `i64::MIN % -1` panics with an overflow.
#[test]
fn rem_overflow_i64_min_by_minus_one() {
    assert_binary_panics(MIN, "%", "-1", "integer overflow");
}

/// A zero divisor wins over the `i64::MIN` check: `i64::MIN / 0` is a division by zero.
#[test]
fn div_of_i64_min_by_zero_is_a_division_by_zero() {
    assert_binary_panics(MIN, "/", "0", "division by zero");
}

/// A zero divisor wins for the remainder too.
#[test]
fn rem_of_i64_min_by_zero_is_a_division_by_zero() {
    assert_binary_panics(MIN, "%", "0", "division by zero");
}

/// A compound assignment panics at the statement's start.
#[test]
fn compound_assignment_overflow_points_at_the_statement() {
    let src = main_with(&["x := 9223372036854775807", "x += 1", "print(x)"]);
    assert_panics(&src, "integer overflow", "x += 1", 0, "");
}

/// A compound division by zero panics at the statement's start.
#[test]
fn compound_division_by_zero_points_at_the_statement() {
    let src = main_with(&["x := 10", "z := 0", "x /= z", "print(x)"]);
    assert_panics(&src, "division by zero", "x /= z", 0, "");
}

/// The location of an inner operation is its own start, not the whole statement's.
#[test]
fn nested_operation_reports_its_own_location() {
    let src = main_with(&[
        "a := 1",
        "b := 9223372036854775807",
        "c := 2",
        "print(a + b * c)",
    ]);
    assert_panics(&src, "integer overflow", "b * c", 0, "");
}

/// A parenthesized operation reports the start of its left operand, as parentheses make no node.
#[test]
fn parenthesized_operation_reports_the_left_operand_start() {
    let src = main_with(&[
        "a := 4611686018427387904",
        "b := 0",
        "c := 4",
        "print((a + b) * c)",
    ]);
    assert_panics(&src, "integer overflow", "a + b", 0, "");
}

/// A panic inside a called function reports the location in that function.
#[test]
fn panic_in_a_callee_reports_the_callee_location() {
    let src = program(&[
        "fn halve(n: i64, d: i64) -> i64 {",
        "    return n / d",
        "}",
        "fn main() {",
        "    print(halve(8, 2))",
        "    print(halve(8, 0))",
        "}",
    ]);
    assert_panics(&src, "division by zero", "n / d", 0, "4\n");
}

/// A panic in the middle of a loop keeps the earlier output, ints and bools alike.
#[test]
fn panic_in_a_loop_keeps_earlier_output() {
    let src = main_with(&["print(true)", "for i in 0..5 {", "print(10 / (2 - i))", "}"]);
    assert_panics(&src, "division by zero", "10 / (2 - i)", 0, "true\n5\n10\n");
}

/// The panic location uses 1-based lines and columns on later lines with indentation.
#[test]
fn panic_location_counts_lines_and_columns_from_one() {
    let src = program(&[
        "fn main() {",
        "    a := 9223372036854775807",
        "",
        "    b := a + 1",
        "    print(b)",
        "}",
    ]);
    assert_eq!(loc(&src, "a + 1", 0), "4:10");
    let (stdout, stderr, code) = run(&src);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "panic: integer overflow at t.em:4:10\n");
    assert_eq!(code, 101);
}

/// 1,500 increments in one function (a frame above 32760 bytes) still count to 1500.
#[test]
fn frame_above_32760_bytes_counts_increments() {
    let mut body = vec!["x := 0".to_string()];
    body.extend((0..1500).map(|_| "x = x + 1".to_string()));
    body.push("print(x)".to_string());
    assert_output(&main_with(&body), "1500\n");
}

/// 4,200 prints of a local in one function all print.
#[test]
fn frame_above_32760_bytes_prints_every_line() {
    let mut body = vec!["i := 7".to_string()];
    body.extend((0..4200).map(|_| "print(i)".to_string()));
    assert_output(&main_with(&body), &"7\n".repeat(4200));
}

/// A callee with a frame above 32760 bytes returns correctly and restores the caller's frame.
#[test]
fn callee_with_a_frame_above_32760_bytes_restores_the_caller() {
    let mut lines = vec!["fn big() -> i64 {".to_string(), "x := 0".to_string()];
    lines.extend((0..1500).map(|_| "x = x + 1".to_string()));
    lines.push("return x".to_string());
    lines.push("}".to_string());
    lines.extend(
        [
            "fn main() {",
            "p := 11",
            "q := big()",
            "print(p)",
            "print(q)",
            "r := big() + p",
            "print(r)",
            "print(p)",
            "}",
        ]
        .map(String::from),
    );
    assert_output(&program(&lines), "11\n1500\n1511\n11\n");
}

/// `i := 7`, 4,200 prints of `i` (more than 4,096 cells), then the given statements.
fn huge_main_with(tail: &[&str]) -> String {
    let mut body = vec!["i := 7".to_string()];
    body.extend((0..4200).map(|_| "print(i)".to_string()));
    body.extend(tail.iter().map(|line| line.to_string()));
    main_with(&body)
}

/// The output of 4,200 prints of 7.
fn sevens() -> String {
    "7\n".repeat(4200)
}

/// A product in a function with far cells prints correctly (the high half lives in a far cell).
#[test]
fn far_frame_multiplication_prints_the_product() {
    let src = huge_main_with(&["print(i * i)"]);
    assert_output(&src, &format!("{}49\n", sevens()));
}

/// A quotient in a function with far cells prints correctly.
#[test]
fn far_frame_division_prints_the_quotient() {
    let src = huge_main_with(&["print(i / 2)"]);
    assert_output(&src, &format!("{}3\n", sevens()));
}

/// A remainder in a function with far cells prints correctly.
#[test]
fn far_frame_remainder_prints_the_remainder() {
    let src = huge_main_with(&["print(i % 4)"]);
    assert_output(&src, &format!("{}3\n", sevens()));
}

/// A multiplication overflow in a function with far cells panics with the exact message.
#[test]
fn far_frame_multiplication_overflow_panics() {
    let src = huge_main_with(&[&format!("j := {MAX}"), "print(i * j)"]);
    assert_panics(&src, "integer overflow", "i * j", 0, &sevens());
}

/// A division by zero in a function with far cells panics with the exact message.
#[test]
fn far_frame_division_by_zero_panics() {
    let src = huge_main_with(&["z := 0", "print(i / z)"]);
    assert_panics(&src, "division by zero", "i / z", 0, &sevens());
}

/// `MIN / -1` in a function with far cells panics with an overflow.
#[test]
fn far_frame_min_divided_by_minus_one_panics() {
    let src = huge_main_with(&[&format!("m := {MIN}"), "d := -1", "print(m / d)"]);
    assert_panics(&src, "integer overflow", "m / d", 0, &sevens());
}

/// A remainder by zero in a function with far cells panics with the exact message.
#[test]
fn far_frame_remainder_by_zero_panics() {
    let src = huge_main_with(&["z := 0", "print(i % z)"]);
    assert_panics(&src, "division by zero", "i % z", 0, &sevens());
}
