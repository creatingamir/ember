//! Integration tests for the `check` crate: statements, loops, `return` and missing returns.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{assert_single_error, checked, error, errors, main_with};

/// Builds a program with a `noop` function (no value) and `main` with the given body.
fn with_noop(body: &str) -> String {
    format!("fn noop() {{\n}}\n{}", main_with(body))
}

/// Builds a program with `fn helper(cond: bool) -> i64` containing `body`, and an empty `main`.
fn returning_helper(body: &str) -> String {
    format!("fn helper(cond: bool) -> i64 {{\n{body}\n}}\nfn main() {{\n}}\n")
}

const MISSING_RETURN: &str = "function `helper` must return a value on every path";

// ---------------------------------------------------------------- let

/// Binding a call that returns nothing is an error at the call.
#[test]
fn let_with_unit_value_is_rejected_at_the_value() {
    let src = with_noop("    _ := noop()");
    assert_single_error(&src, "this expression has no value", "noop()", 1);
}

/// A typed `let` whose value has another type is an error at the value.
#[test]
fn typed_let_with_bool_value_for_i64_is_rejected() {
    let src = main_with("    count: i64 = true\n    print(count)");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// A typed `let` of `bool` with an integer value is an error at the value.
#[test]
fn typed_let_with_i64_value_for_bool_is_rejected() {
    let src = main_with("    flag: bool = 1\n    print(flag)");
    assert_single_error(&src, "expected `bool`, found `i64`", "1", 0);
}

/// A typed `let` whose value matches is accepted.
#[test]
fn typed_let_with_matching_value_is_accepted() {
    checked(&main_with(
        "    count: i64 = 1\n    flag: bool = true\n    print(count)\n    print(flag)",
    ));
}

// ---------------------------------------------------------------- assignment

/// Assigning to a function is an error at the target.
#[test]
fn assign_to_function_is_rejected_at_the_target() {
    let src = "fn helper() {\n}\nfn main() {\n    helper = 1\n}\n";
    assert_single_error(src, "cannot assign to function `helper`", "helper", 1);
}

/// Assigning to an unknown name is an error at the target.
#[test]
fn assign_to_undefined_name_is_rejected_at_the_target() {
    assert_single_error(&main_with("    z = 1"), "undefined name `z`", "z", 0);
}

/// `=` with a `bool` value for an `i64` local is an error at the value.
#[test]
fn assign_bool_to_i64_is_rejected_at_the_value() {
    let src = main_with("    count := 1\n    count = true\n    print(count)");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// `=` with an `i64` value for a `bool` local is an error at the value.
#[test]
fn assign_i64_to_bool_is_rejected_at_the_value() {
    let src = main_with("    flag := true\n    flag = 1\n    print(flag)");
    assert_single_error(&src, "expected `bool`, found `i64`", "1", 0);
}

/// Parameters can be assigned.
#[test]
fn assign_to_parameter_is_accepted() {
    let src = "fn helper(val: i64) {\n    val = val + 1\n}\nfn main() {\n}\n";
    checked(src);
}

/// Compound assignment on a `bool` target is an error at the target.
#[test]
fn compound_assign_to_bool_target_is_rejected_at_the_target() {
    for op in ["+=", "-=", "*=", "/="] {
        let body = format!("    flag := true\n    flag {op} 1\n    print(flag)");
        let src = main_with(&body);
        assert_single_error(&src, "expected `i64`, found `bool`", "flag", 1);
    }
}

/// Compound assignment of a `bool` value to an `i64` target is an error at the value.
#[test]
fn compound_assign_of_bool_value_is_rejected_at_the_value() {
    for op in ["+=", "-=", "*=", "/="] {
        let body = format!("    count := 1\n    count {op} true\n    print(count)");
        let src = main_with(&body);
        assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
    }
}

/// When both sides of a compound assignment are `bool`, only the target is reported.
#[test]
fn compound_assign_with_bool_target_and_value_reports_the_target_only() {
    let src = main_with("    flag := true\n    flag += true\n    print(flag)");
    assert_single_error(&src, "expected `i64`, found `bool`", "flag", 1);
}

/// Compound assignment with `i64` on both sides is accepted.
#[test]
fn compound_assign_with_i64_is_accepted() {
    let body = "    count := 1\n    count += 2\n    count -= 3\n    count *= 4\n    count /= 5\n    print(count)";
    checked(&main_with(body));
}

/// `@=` is not supported yet; the error covers the whole statement.
#[test]
fn matmul_assign_is_not_supported_yet() {
    let src = main_with("    count := 1\n    count @= 2\n    print(count)");
    assert_single_error(&src, "`@=` is not supported yet", "count @= 2", 0);
}

// ---------------------------------------------------------------- conditions

/// An `if` condition of type `i64` is an error at the condition.
#[test]
fn if_condition_must_be_bool() {
    let src = main_with("    if 1 {\n    }");
    assert_single_error(&src, "expected `bool`, found `i64`", "1", 0);
}

/// The condition of an `else if` is checked too.
#[test]
fn else_if_condition_must_be_bool() {
    let src = main_with("    if true {\n    } else if 2 {\n    }");
    assert_single_error(&src, "expected `bool`, found `i64`", "2", 0);
}

/// A while-style `for` condition of type `i64` is an error at the condition.
#[test]
fn while_condition_must_be_bool() {
    let src = main_with("    for 1 {\n    }");
    assert_single_error(&src, "expected `bool`, found `i64`", "1", 0);
}

// ---------------------------------------------------------------- for-in loops

/// Iterating over something that is not a range is an error at the iterable.
#[test]
fn for_in_over_non_range_is_rejected() {
    let src = main_with("    for idx in 5 {\n    }");
    assert_single_error(&src, "only ranges can be iterated over yet", "5", 0);
}

/// A `bool` upper bound is an error at the bound.
#[test]
fn range_end_must_be_i64() {
    let src = main_with("    for idx in 0..true {\n    }");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// A `bool` lower bound is an error at the bound.
#[test]
fn range_start_must_be_i64() {
    let src = main_with("    for idx in true..3 {\n    }");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// Inclusive ranges check their bounds too.
#[test]
fn inclusive_range_bounds_must_be_i64() {
    let src = main_with("    for idx in 0..=true {\n    }");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// Both exclusive and inclusive ranges with `i64` bounds are accepted.
#[test]
fn ranges_with_i64_bounds_are_accepted() {
    let body = "    for idx in 0..3 {\n        print(idx)\n    }\n    for jdx in 0..=3 {\n        print(jdx)\n    }";
    checked(&main_with(body));
}

/// The loop variable can be assigned an `i64`.
#[test]
fn loop_variable_is_assignable() {
    let body = "    for idx in 0..3 {\n        idx = idx + 1\n    }";
    checked(&main_with(body));
}

/// The loop variable is an `i64`, so a `bool` cannot be assigned to it.
#[test]
fn loop_variable_is_i64() {
    let body = "    for idx in 0..3 {\n        idx = true\n    }";
    assert_single_error(&main_with(body), "expected `i64`, found `bool`", "true", 0);
}

// ---------------------------------------------------------------- break and continue

/// `break` outside any loop is an error at the statement.
#[test]
fn break_outside_loop_is_rejected() {
    let src = main_with("    break");
    assert_single_error(&src, "`break` outside of a loop", "break", 0);
}

/// `continue` outside any loop is an error at the statement.
#[test]
fn continue_outside_loop_is_rejected() {
    let src = main_with("    continue");
    assert_single_error(&src, "`continue` outside of a loop", "continue", 0);
}

/// An `if` alone does not make `break` legal.
#[test]
fn break_in_if_outside_loop_is_rejected() {
    let src = main_with("    if true {\n        break\n    }");
    assert_single_error(&src, "`break` outside of a loop", "break", 0);
}

/// Once the loop has ended, `break` is illegal again.
#[test]
fn break_after_loop_is_rejected() {
    let src = main_with("    for {\n        break\n    }\n    break");
    assert_single_error(&src, "`break` outside of a loop", "break", 1);
}

/// `break` and `continue` are fine in every kind of loop, also inside an `if`.
#[test]
fn break_and_continue_inside_loops_are_accepted() {
    let body = "\
    for {
        break
    }
    for true {
        if true {
            continue
        }
        break
    }
    for idx in 0..3 {
        continue
    }";
    checked(&main_with(body));
}

/// `break` and `continue` are fine in nested loops.
#[test]
fn break_and_continue_in_nested_loops_are_accepted() {
    let body = "\
    for true {
        for true {
            break
        }
        continue
    }";
    checked(&main_with(body));
}

// ---------------------------------------------------------------- return

/// `return` with a value in a function without a return type is an error at the value.
#[test]
fn return_value_in_function_without_return_type_is_rejected() {
    let src = "fn helper() {\n    return 1\n}\nfn main() {\n}\n";
    assert_single_error(src, "function `helper` does not return a value", "1", 0);
}

/// A bare `return` is fine in a function without a return type.
#[test]
fn bare_return_in_function_without_return_type_is_accepted() {
    checked("fn helper() {\n    return\n}\nfn main() {\n}\n");
}

/// A bare `return` in a function returning `i64` is an error at the statement.
#[test]
fn bare_return_in_i64_function_is_rejected() {
    let src = "fn helper() -> i64 {\n    return\n}\nfn main() {\n}\n";
    assert_single_error(src, "expected a return value of type `i64`", "return", 0);
}

/// The message names the function's own return type.
#[test]
fn bare_return_in_bool_function_names_bool() {
    let src = "fn helper() -> bool {\n    return\n}\nfn main() {\n}\n";
    assert_single_error(src, "expected a return value of type `bool`", "return", 0);
}

/// Returning the wrong type is an error at the value.
#[test]
fn return_of_wrong_type_is_rejected_at_the_value() {
    let src = "fn helper() -> i64 {\n    return true\n}\nfn main() {\n}\n";
    assert_single_error(src, "expected `i64`, found `bool`", "true", 0);
}

/// Returning the right type is accepted.
#[test]
fn return_of_matching_type_is_accepted() {
    checked("fn helper() -> bool {\n    return true\n}\nfn main() {\n}\n");
}

/// The one-line form `fn f() -> T = EXPR` returns its expression.
#[test]
fn expression_bodied_function_returns_its_expression() {
    checked("fn helper(val: i64) -> i64 = val + 1\nfn main() {\n}\n");
}

/// The one-line form is checked against the declared return type.
#[test]
fn expression_bodied_function_of_wrong_type_is_rejected() {
    let src = "fn helper() -> bool = 1\nfn main() {\n}\n";
    assert_single_error(src, "expected `bool`, found `i64`", "1", 0);
}

/// The one-line form without a return type is an expression statement that must be a call.
#[test]
fn expression_bodied_function_without_return_type_accepts_a_call() {
    checked("fn helper() = print(1)\nfn main() {\n}\n");
}

// ---------------------------------------------------------------- missing return

/// An empty body does not return.
#[test]
fn empty_body_with_return_type_is_missing_a_return() {
    let src = "fn helper() -> i64 {\n}\nfn main() {\n}\n";
    assert_single_error(src, MISSING_RETURN, "helper", 0);
}

/// An `if` without `else` can fall through.
#[test]
fn if_without_else_is_missing_a_return() {
    let body = "    if cond {\n        return 1\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// An `if` whose two branches both return always returns.
#[test]
fn if_else_returning_in_both_branches_is_accepted() {
    let body = "    if cond {\n        return 1\n    } else {\n        return 2\n    }";
    checked(&returning_helper(body));
}

/// An `if` whose `else` branch can fall through does not always return.
#[test]
fn if_else_returning_only_in_then_is_missing_a_return() {
    let body = "    if cond {\n        return 1\n    } else {\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// An `if` whose `then` branch can fall through does not always return.
#[test]
fn if_else_returning_only_in_else_is_missing_a_return() {
    let body = "    if cond {\n    } else {\n        return 1\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// An `else if` chain that returns in every branch and ends with `else` always returns.
#[test]
fn else_if_chain_with_final_else_is_accepted() {
    let body = "\
    if cond {
        return 1
    } else if !cond {
        return 2
    } else {
        return 3
    }";
    checked(&returning_helper(body));
}

/// An `else if` chain without a final `else` can fall through.
#[test]
fn else_if_chain_without_final_else_is_missing_a_return() {
    let body = "\
    if cond {
        return 1
    } else if !cond {
        return 2
    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// An `else if` chain with one branch that falls through does not always return.
#[test]
fn else_if_chain_with_a_non_returning_branch_is_missing_a_return() {
    let body = "\
    if cond {
        return 1
    } else if !cond {
    } else {
        return 3
    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// An infinite loop without `break` never finishes, so the function needs no final return.
#[test]
fn infinite_loop_without_break_always_returns() {
    checked(&returning_helper("    for {\n    }"));
}

/// An infinite loop that contains a `break` can finish.
#[test]
fn infinite_loop_with_break_is_missing_a_return() {
    let body = "    for {\n        break\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// A `break` nested in an `if` still belongs to the loop.
#[test]
fn infinite_loop_with_break_in_if_is_missing_a_return() {
    let body = "    for {\n        if cond {\n            break\n        }\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// A `break` inside a nested loop belongs to that loop, not the outer one.
#[test]
fn break_in_nested_loop_does_not_count_for_the_outer_loop() {
    let body = "    for {\n        for {\n            break\n        }\n    }";
    checked(&returning_helper(body));
}

/// A `break` in the outer loop counts even when the inner loop has its own.
#[test]
fn break_in_outer_loop_counts_even_with_a_nested_loop() {
    let body = "    for {\n        for {\n            break\n        }\n        break\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// A while-style loop may never run, so returning inside it is not enough.
#[test]
fn while_loop_with_return_is_missing_a_return() {
    let body = "    for cond {\n        return 1\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// A range loop may run zero times, so returning inside it is not enough.
#[test]
fn range_loop_with_return_is_missing_a_return() {
    let body = "    for idx in 0..3 {\n        return idx\n    }";
    assert_single_error(&returning_helper(body), MISSING_RETURN, "helper", 0);
}

/// A block always returns when any one of its statements does, even if more follow.
#[test]
fn return_followed_by_more_statements_always_returns() {
    let body = "    return 1\n    print(2)";
    checked(&returning_helper(body));
}

/// A `return` after an earlier statement that can fall through is enough.
#[test]
fn return_after_a_non_returning_statement_always_returns() {
    let body = "    for {\n        break\n    }\n    return 1";
    checked(&returning_helper(body));
}

/// A function without a return type never needs a `return`.
#[test]
fn function_without_return_type_needs_no_return() {
    checked("fn helper() {\n    print(1)\n}\nfn main() {\n}\n");
}

// ---------------------------------------------------------------- expression statements

/// A bare arithmetic expression is not a statement.
#[test]
fn expression_statement_must_be_a_call() {
    let src = main_with("    1 + 2");
    assert_single_error(&src, "only calls can be used as statements", "1 + 2", 0);
}

/// A bare literal is not a statement.
#[test]
fn literal_expression_statement_is_rejected() {
    let src = main_with("    true");
    assert_single_error(&src, "only calls can be used as statements", "true", 0);
}

/// A call may have its result discarded.
#[test]
fn call_result_may_be_discarded() {
    let src = "\
fn helper() -> i64 {
    return 1
}
fn main() {
    helper()
}
";
    checked(src);
}

// ---------------------------------------------------------------- second clarifications: statements

/// Only the first bad range bound is reported, the left one.
#[test]
fn only_the_first_bad_range_bound_is_reported() {
    let src = main_with("    for idx in true..false {\n    }");
    assert_single_error(&src, "expected `i64`, found `bool`", "true", 0);
}

/// `@=` reports its own error and still checks the value.
#[test]
fn matmul_assign_still_checks_the_value() {
    let src = main_with("    count := 1\n    count @= missing\n    print(count)");
    assert_eq!(
        errors(&src),
        vec![
            error(&src, "`@=` is not supported yet", "count @= missing", 0),
            error(&src, "undefined name `missing`", "missing", 0),
        ]
    );
}

/// `@=` does not resolve its target, so an unknown target adds no error.
#[test]
fn matmul_assign_does_not_resolve_its_target() {
    let src = main_with("    missing @= 1");
    assert_single_error(&src, "`@=` is not supported yet", "missing @= 1", 0);
}

/// A failed iterable gets no extra "only ranges" error.
#[test]
fn failed_iterable_gets_no_only_ranges_error() {
    let src = main_with("    for idx in missing {\n    }");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}

/// A failed expression statement gets no extra "only calls" error.
#[test]
fn failed_expression_statement_gets_no_only_calls_error() {
    let src = main_with("    1 + missing");
    assert_single_error(&src, "undefined name `missing`", "missing", 0);
}
