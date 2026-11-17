//! Integration tests for the `check` crate: expressions, calls, error aggregation and results.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use std::collections::HashMap;

use check::{Binding, Signature, TypeId};
use common::{assert_single_error, checked, error, errors, fixture, main_with};
use source::Span;

/// A function taking two integers, used by the call tests.
const ADD: &str = "fn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n";

/// Builds a program with `add` and a `main` with the given body.
fn with_add(body: &str) -> String {
    format!("{ADD}{}", main_with(body))
}

/// Builds a program with a `noop` function (no value) and `main` with the given body.
fn with_noop(body: &str) -> String {
    format!("fn noop() {{\n}}\n{}", main_with(body))
}

/// Builds `main` that binds the expression to a local and prints it.
fn binding_expr(expr: &str) -> String {
    main_with(&format!("    count := {expr}\n    print(count)"))
}

// ---------------------------------------------------------------- literals

/// The largest `i64` is a valid literal.
#[test]
fn largest_i64_literal_is_accepted() {
    checked(&binding_expr("9223372036854775807"));
}

/// One more than the largest `i64` is an error at the literal.
#[test]
fn integer_literal_above_i64_max_is_rejected() {
    let src = binding_expr("9223372036854775808");
    assert_single_error(
        &src,
        "integer literal does not fit in `i64`",
        "9223372036854775808",
        0,
    );
}

/// The smallest `i64` is allowed as a negated literal.
#[test]
fn negated_i64_min_literal_is_accepted() {
    let fixture = fixture(&binding_expr("-9223372036854775808"));
    assert_eq!(fixture.expr_type("-9223372036854775808", 0), TypeId::I64);
}

/// Negating a literal beyond `i64::MIN` is an error at the literal, not the negation.
#[test]
fn negated_literal_below_i64_min_is_rejected_at_the_literal() {
    let src = binding_expr("-9223372036854775809");
    assert_single_error(
        &src,
        "integer literal does not fit in `i64`",
        "9223372036854775809",
        0,
    );
}

/// The `i64::MIN` exception applies only directly under a unary minus.
#[test]
fn i64_min_magnitude_outside_negation_is_rejected() {
    let src = binding_expr("1 - 9223372036854775808");
    assert_single_error(
        &src,
        "integer literal does not fit in `i64`",
        "9223372036854775808",
        0,
    );
}

// ---------------------------------------------------------------- unary operators

/// Unary minus needs an `i64` operand; the error is at the operand.
#[test]
fn negating_bool_is_rejected_at_the_operand() {
    let src = binding_expr("-true");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// Logical not needs a `bool` operand; the error is at the operand.
#[test]
fn not_of_i64_is_rejected_at_the_operand() {
    let src = binding_expr("!1");
    assert_single_error(&src, "expected `bool`, found `i64`", "1", 0);
}

/// `~`, `&` and `*` are not supported yet; the error covers the whole expression.
#[test]
fn unsupported_unary_operators_are_rejected() {
    for op in ["~", "&", "*"] {
        let expr = format!("{op}1");
        let message = format!("`{op}` is not supported yet");
        assert_single_error(&binding_expr(&expr), &message, &expr, 0);
    }
}

// ---------------------------------------------------------------- binary operators

/// A `bool` right operand of an arithmetic operator is an error at that operand.
#[test]
fn arithmetic_with_bool_right_operand_is_rejected() {
    for op in ["+", "-", "*", "/", "%"] {
        let src = binding_expr(&format!("1 {op} true"));
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// A `bool` left operand of an arithmetic operator is an error at that operand.
#[test]
fn arithmetic_with_bool_left_operand_is_rejected() {
    for op in ["+", "-", "*", "/", "%"] {
        let src = binding_expr(&format!("true {op} 1"));
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// When both operands are wrong, only the left one is reported.
#[test]
fn arithmetic_reports_the_left_operand_first() {
    let src = binding_expr("true + false");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// Ordering comparisons need `i64` operands; the error is at the bad operand.
#[test]
fn ordering_with_bool_operand_is_rejected() {
    for op in ["<", "<=", ">", ">="] {
        let left = binding_expr(&format!("true {op} 1"));
        assert_single_error(&left, "expected `i64`, found `bool`", "true", 0);
        let right = binding_expr(&format!("1 {op} true"));
        assert_single_error(&right, "expected `i64`, found `bool`", "true", 0);
    }
}

/// Equality of an `i64` with a `bool` is an error covering the whole expression.
#[test]
fn comparing_i64_with_bool_is_rejected_at_the_whole_expression() {
    for op in ["==", "!="] {
        let expr = format!("1 {op} true");
        let src = binding_expr(&expr);
        assert_single_error(&src, "cannot compare `i64` with `bool`", &expr, 0);
    }
}

/// The message names the types in operand order.
#[test]
fn comparing_bool_with_i64_names_the_types_in_order() {
    let src = binding_expr("true == 1");
    assert_single_error(&src, "cannot compare `bool` with `i64`", "true == 1", 0);
}

/// `&&` and `||` need `bool` operands; the error is at the bad operand.
#[test]
fn logical_operators_with_i64_operand_are_rejected() {
    let and = binding_expr("true && 1");
    assert_single_error(&and, "expected `bool`, found `i64`", "1", 0);
    let or = binding_expr("1 || true");
    assert_single_error(&or, "expected `bool`, found `i64`", "1", 0);
}

/// A range outside a `for` loop is an error covering the range.
#[test]
fn exclusive_range_outside_for_is_rejected() {
    let src = binding_expr("0..3");
    assert_single_error(&src, "ranges can only be used in `for` loops", "0..3", 0);
}

/// An inclusive range outside a `for` loop is an error covering the range.
#[test]
fn inclusive_range_outside_for_is_rejected() {
    let src = binding_expr("0..=3");
    assert_single_error(&src, "ranges can only be used in `for` loops", "0..=3", 0);
}

/// A range passed as an argument is not directly iterated, so it is rejected.
#[test]
fn range_as_call_argument_is_rejected() {
    let src = main_with("    print(0..3)");
    assert_single_error(&src, "ranges can only be used in `for` loops", "0..3", 0);
}

/// Operators outside the subset say so, spelled as in the source.
#[test]
fn unsupported_binary_operators_are_rejected() {
    for op in ["@", "<<", ">>", "&", "|", "^"] {
        let expr = format!("1 {op} 2");
        let message = format!("`{op}` is not supported yet");
        assert_single_error(&binding_expr(&expr), &message, &expr, 0);
    }
}

/// Well-typed operators of every family are accepted.
#[test]
fn well_typed_operators_are_accepted() {
    let body = "\
    count := 7
    print(-count + count * 2 - count / 3 % 4)
    print(count < 8 && count >= 1 || count != 3)
    print(true == false)
    print(!(count <= 1) == (count > 2))";
    checked(&main_with(body));
}

// ---------------------------------------------------------------- calls

/// Too many arguments is an error covering the call.
#[test]
fn too_many_arguments_are_reported_at_the_call() {
    let src = with_add("    print(add(1, 2, 3))");
    let message = "`add` takes 2 arguments, but 3 were given";
    assert_single_error(&src, message, "add(1, 2, 3)", 0);
}

/// Too few arguments is an error covering the call.
#[test]
fn missing_arguments_are_reported_at_the_call() {
    let src = with_add("    print(add())");
    let message = "`add` takes 2 arguments, but 0 were given";
    assert_single_error(&src, message, "add()", 0);
}

/// The message says "1 argument" in the singular.
#[test]
fn arity_message_uses_singular_for_one_parameter() {
    let src = "\
fn one(val: i64) {
}
fn main() {
    one(1, 2)
}
";
    let message = "`one` takes 1 argument, but 2 were given";
    assert_single_error(src, message, "one(1, 2)", 0);
}

/// An argument of the wrong type is an error at that argument.
#[test]
fn argument_of_wrong_type_is_reported_at_the_argument() {
    let src = with_add("    print(add(1, true))");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// Every wrong argument is reported, in source order.
#[test]
fn every_wrong_argument_is_reported() {
    let src = with_add("    print(add(true, false))");
    assert_eq!(
        errors(&src),
        vec![
            error(&src, "expected `i64`, found `bool`", "true", 0),
            error(&src, "expected `i64`, found `bool`", "false", 0),
        ]
    );
}

/// A well-formed call to a user function is accepted.
#[test]
fn well_typed_call_is_accepted() {
    checked(&with_add("    print(add(1, 2))"));
}

/// `print` takes exactly one argument; extra ones are an error at the call.
#[test]
fn print_with_two_arguments_is_rejected_at_the_call() {
    let src = main_with("    print(1, 2)");
    let message = "`print` takes 1 argument, but 2 were given";
    assert_single_error(&src, message, "print(1, 2)", 0);
}

/// `print` without an argument is an error at the call.
#[test]
fn print_without_arguments_is_rejected_at_the_call() {
    let src = main_with("    print()");
    let message = "`print` takes 1 argument, but 0 were given";
    assert_single_error(&src, message, "print()", 0);
}

/// `print` of a call that returns nothing is an error at the argument.
#[test]
fn print_of_unit_is_rejected_at_the_argument() {
    let src = with_noop("    print(noop())");
    let message = "`print` cannot show `()`";
    assert_single_error(&src, message, "noop()", 1);
}

/// `print` returns nothing, so its result cannot be bound.
#[test]
fn binding_the_result_of_print_is_rejected() {
    let src = main_with("    _ := print(1)");
    assert_single_error(&src, "this expression has no value", "print(1)", 0);
}

/// `print` accepts both `i64` and `bool`.
#[test]
fn print_of_i64_and_bool_is_accepted() {
    checked(&main_with("    print(1)\n    print(true)"));
}

// ---------------------------------------------------------------- several errors

/// All errors are returned, sorted by position even when found in another order.
#[test]
fn all_errors_are_reported_sorted_by_position() {
    let body = "    first := 1\n    second: i64 = true\n    print(second)";
    let src = main_with(body);
    assert_eq!(
        errors(&src),
        vec![
            error(&src, "`first` is declared but never used", "first", 0),
            error(&src, "expected `i64`, found `bool`", "true", 0),
        ]
    );
}

/// The missing-`main` error at offset 0 sorts before later errors.
#[test]
fn missing_main_sorts_before_other_errors() {
    let src = "fn helper() -> i64 {\n}\n";
    assert_eq!(
        errors(src),
        vec![
            ("missing `fn main()`".to_string(), Span::new(0, 0)),
            error(
                src,
                "function `helper` must return a value on every path",
                "helper",
                0
            ),
        ]
    );
}

/// Errors in different functions are all reported.
#[test]
fn errors_in_different_functions_are_all_reported() {
    let src = "\
fn helper() {
    print(first)
}
fn main() {
    print(second)
}
";
    assert_eq!(
        errors(src),
        vec![
            error(src, "undefined name `first`", "first", 0),
            error(src, "undefined name `second`", "second", 0),
        ]
    );
}

/// Several errors inside one function are all reported.
#[test]
fn several_errors_in_one_function_are_all_reported() {
    let src = main_with("    print(first)\n    print(second)\n    foo(1)");
    assert_eq!(
        errors(&src),
        vec![
            error(&src, "undefined name `first`", "first", 0),
            error(&src, "undefined name `second`", "second", 0),
            error(&src, "undefined function `foo`", "foo", 0),
        ]
    );
}

// ---------------------------------------------------------------- no follow-on errors

/// A failed expression is compatible with everything around it.
#[test]
fn undefined_name_in_an_operator_causes_no_follow_on_error() {
    let expressions = [
        "undefined_name + 1",
        "1 + undefined_name",
        "-undefined_name",
        "!undefined_name",
        "undefined_name == true",
        "undefined_name < 1",
        "undefined_name && true",
    ];
    for expr in expressions {
        let src = main_with(&format!("    print({expr})"));
        assert_single_error(&src, "undefined name `undefined_name`", "undefined_name", 0);
    }
}

/// Using a local whose value had an error causes no second error.
#[test]
fn local_initialised_from_a_failed_expression_causes_no_follow_on_error() {
    let src = main_with("    count := undefined_name + 1\n    print(count)");
    assert_single_error(&src, "undefined name `undefined_name`", "undefined_name", 0);
}

/// A call to an unknown function is compatible with everything around it.
#[test]
fn undefined_function_result_causes_no_follow_on_error() {
    let src = main_with("    count := foo()\n    print(count)");
    assert_single_error(&src, "undefined function `foo`", "foo", 0);
}

/// A local bound to a value-less call causes no second error where it is used.
#[test]
fn local_initialised_from_unit_causes_no_follow_on_error() {
    let src = with_noop("    count := noop()\n    print(count + 1)");
    assert_single_error(&src, "this expression has no value", "noop()", 1);
}

/// A typed local keeps its declared type even when its value was wrong.
#[test]
fn typed_local_with_wrong_value_keeps_its_declared_type() {
    let src = main_with("    count: i64 = true\n    print(count + 1)");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// A failed argument causes no arity or parameter-type error.
#[test]
fn failed_call_argument_causes_no_follow_on_error() {
    let src = with_add("    print(add(undefined_name, 1))");
    assert_single_error(&src, "undefined name `undefined_name`", "undefined_name", 0);
}

/// A failed `if` condition causes no second error.
#[test]
fn failed_condition_causes_no_follow_on_error() {
    let src = main_with("    if undefined_name {\n    }");
    assert_single_error(&src, "undefined name `undefined_name`", "undefined_name", 0);
}

/// A failed returned value causes no type or missing-return error.
#[test]
fn failed_return_value_causes_no_follow_on_error() {
    let src = "\
fn helper() -> i64 {
    return undefined_name
}
fn main() {
}
";
    assert_single_error(src, "undefined name `undefined_name`", "undefined_name", 0);
}

/// A function used as a value in an operator causes only the function error.
#[test]
fn function_used_in_operator_causes_only_one_error() {
    let src = "\
fn helper() {
}
fn main() {
    print(helper + 1)
}
";
    assert_single_error(src, "`helper` is a function, not a value", "helper", 1);
}

// ---------------------------------------------------------------- results of a successful check

/// Types are named as they are written in source, and `()` for no value.
#[test]
fn types_are_named_as_in_source() {
    let types = checked("fn main() {\n}\n").types;
    assert_eq!(types.name(TypeId::I64), "i64");
    assert_eq!(types.name(TypeId::BOOL), "bool");
    assert_eq!(types.name(TypeId::UNIT), "()");
}

/// Literals, names, operators and comparisons get their types.
#[test]
fn expression_types_of_operators() {
    let body = "\
    count := 1 + 2
    flag := count < 3
    print(flag)
    print(-count)
    print(!flag)
    print(count == 5 && true)";
    let fixture = fixture(&main_with(body));
    assert_eq!(fixture.expr_type("1 + 2", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("1", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("count < 3", 0), TypeId::BOOL);
    assert_eq!(fixture.expr_type("count", 1), TypeId::I64);
    assert_eq!(fixture.expr_type("flag", 1), TypeId::BOOL);
    assert_eq!(fixture.expr_type("-count", 0), TypeId::I64);
    assert_eq!(fixture.expr_type("!flag", 0), TypeId::BOOL);
    assert_eq!(fixture.expr_type("count == 5 && true", 0), TypeId::BOOL);
    assert_eq!(fixture.expr_type("true", 0), TypeId::BOOL);
}

/// A call has the return type of the function, `()` when there is none, and `print` is `()`.
#[test]
fn expression_types_of_calls() {
    let src = "\
fn number() -> i64 {
    return 1
}
fn truth() -> bool {
    return true
}
fn quiet() {
}
fn main() {
    print(number())
    print(truth())
    quiet()
}
";
    let fixture = fixture(src);
    assert_eq!(fixture.expr_type("number()", 1), TypeId::I64);
    assert_eq!(fixture.expr_type("truth()", 1), TypeId::BOOL);
    assert_eq!(fixture.expr_type("quiet()", 1), TypeId::UNIT);
    assert_eq!(fixture.expr_type("print(number())", 0), TypeId::UNIT);
}

/// Every expression of a program gets a type.
#[test]
fn every_expression_has_a_type() {
    let fixture = fixture(FIB_PROGRAM);
    for id in fixture.all_expr_ids() {
        assert!(fixture.checked.expr_types.contains_key(&id));
    }
}

/// A recursive function with a loop, used by several tests.
const FIB_PROGRAM: &str = "\
fn fib(n: i64) -> i64 {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
fn main() {
    total := 0
    idx := 0
    for idx < 10 {
        total += fib(idx)
        idx += 1
    }
    print(total)
}
";

/// Every name, call and assignment gets a binding.
#[test]
fn every_name_call_and_assignment_has_a_binding() {
    let fixture = fixture(FIB_PROGRAM);
    let ids = fixture
        .name_and_call_ids()
        .into_iter()
        .chain(fixture.assign_ids());
    for id in ids {
        assert!(fixture.checked.bindings.contains_key(&id));
    }
}

/// A program with parameters, locals, a loop variable, calls and assignments.
const BINDING_PROGRAM: &str = "\
fn helper(val: i64) -> i64 {
    return val
}
fn setter(arg: i64) {
    arg = 2
}
fn main() {
    count := 1
    count = helper(count)
    for idx in 0..3 {
        print(idx)
    }
    print(count)
}
";

/// A name that refers to a parameter is bound to the parameter's id.
#[test]
fn name_bound_to_parameter() {
    let fixture = fixture(BINDING_PROGRAM);
    let expected = Binding::Local(fixture.param("val"));
    assert_eq!(fixture.expr_binding("val", 1), expected);
}

/// A name that refers to a `let` is bound to the `let` statement's id.
#[test]
fn name_bound_to_let_statement() {
    let fixture = fixture(BINDING_PROGRAM);
    let expected = Binding::Local(fixture.stmt("count := 1", 0));
    assert_eq!(fixture.expr_binding("count", 2), expected);
}

/// A name that refers to a loop variable is bound to the `for` statement's id.
#[test]
fn name_bound_to_loop_variable() {
    let fixture = fixture(BINDING_PROGRAM);
    let expected = Binding::Local(fixture.stmt("for idx", 0));
    assert_eq!(fixture.expr_binding("idx", 1), expected);
}

/// An assignment to a `let` is bound to the `let` statement's id.
#[test]
fn assignment_bound_to_let_statement() {
    let fixture = fixture(BINDING_PROGRAM);
    let expected = Binding::Local(fixture.stmt("count := 1", 0));
    assert_eq!(fixture.stmt_binding("count = helper", 0), expected);
}

/// An assignment to a parameter is bound to the parameter's id.
#[test]
fn assignment_bound_to_parameter() {
    let fixture = fixture(BINDING_PROGRAM);
    let expected = Binding::Local(fixture.param("arg"));
    assert_eq!(fixture.stmt_binding("arg = 2", 0), expected);
}

/// A call to a user function is bound to the function by name.
#[test]
fn call_bound_to_user_function() {
    let fixture = fixture(BINDING_PROGRAM);
    let expected = Binding::Function("helper".to_string());
    assert_eq!(fixture.expr_binding("helper(count)", 0), expected);
}

/// A call to `print` is bound to the built-in.
#[test]
fn call_bound_to_print() {
    let fixture = fixture(BINDING_PROGRAM);
    assert_eq!(fixture.expr_binding("print(idx)", 0), Binding::Print);
}

/// Parameters, `let`s and loop variables get their types in `local_types`.
#[test]
fn local_types_cover_parameters_lets_and_loop_variables() {
    let src = "\
fn helper(val: i64, flag: bool) {
    count := val
    other: bool = flag
    print(count)
    print(other)
    for idx in 0..3 {
        print(idx)
    }
}
fn main() {
}
";
    let fixture = fixture(src);
    let local_types = &fixture.checked.local_types;
    assert_eq!(local_types[&fixture.param("val")], TypeId::I64);
    assert_eq!(local_types[&fixture.param("flag")], TypeId::BOOL);
    assert_eq!(local_types[&fixture.stmt("count := val", 0)], TypeId::I64);
    assert_eq!(local_types[&fixture.stmt("other: bool", 0)], TypeId::BOOL);
    assert_eq!(local_types[&fixture.stmt("for idx", 0)], TypeId::I64);
}

/// Signatures list every user function with parameter types and return type.
#[test]
fn signatures_list_every_user_function() {
    let src = "\
fn both(a: i64, b: bool) -> i64 {
    return a
}
fn truth() -> bool {
    return true
}
fn quiet(a: i64) {
}
fn main() {
}
";
    let expected: HashMap<String, Signature> = HashMap::from([
        (
            "both".to_string(),
            Signature {
                params: vec![TypeId::I64, TypeId::BOOL],
                ret: TypeId::I64,
            },
        ),
        (
            "truth".to_string(),
            Signature {
                params: vec![],
                ret: TypeId::BOOL,
            },
        ),
        (
            "quiet".to_string(),
            Signature {
                params: vec![TypeId::I64],
                ret: TypeId::UNIT,
            },
        ),
        (
            "main".to_string(),
            Signature {
                params: vec![],
                ret: TypeId::UNIT,
            },
        ),
    ]);
    assert_eq!(checked(src).signatures, expected);
}

/// A typical recursive program with a loop passes the checker.
#[test]
fn fibonacci_program_is_accepted() {
    checked(FIB_PROGRAM);
}

// ---------------------------------------------------------------- second clarifications: expressions

/// Comparing two value-less results is an error covering the whole expression.
#[test]
fn comparing_two_units_is_rejected_at_the_whole_expression() {
    for op in ["==", "!="] {
        let expr = format!("noop() {op} noop()");
        let src = with_noop(&format!("    print({expr})"));
        assert_single_error(&src, "cannot compare `()` with `()`", &expr, 0);
    }
}

/// `print` of a user function that returns nothing gives the spec message at the argument.
#[test]
fn print_of_user_function_without_return_value_is_rejected() {
    let src = "\
fn quiet(val: i64) {
}
fn main() {
    print(quiet(1))
}
";
    let message = "`print` cannot show `()`";
    assert_single_error(src, message, "quiet(1)", 0);
}

/// A local initialised from a value-less call is never reported as unused.
#[test]
fn local_initialised_from_unit_is_never_reported_unused() {
    let src = with_noop("    count := noop()");
    assert_single_error(&src, "this expression has no value", "noop()", 1);
}
