//! Integration tests for the `check` crate: functions, type names, scopes and name resolution.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{assert_single_error, checked, error, errors, main_with};
use source::Span;

// ---------------------------------------------------------------- program and functions

/// Defining two functions with the same name is an error at the second name.
#[test]
fn duplicate_function_is_reported_at_second_name() {
    let src = "\
fn helper() {
}
fn helper() {
}
fn main() {
}
";
    assert_single_error(src, "function `helper` is already defined", "helper", 1);
}

/// A program without `main` is reported at the very start of the file.
#[test]
fn missing_main_is_reported_at_empty_span_at_start() {
    let src = "fn helper() {\n}\n";
    assert_eq!(
        errors(src),
        vec![("missing `fn main()`".to_string(), Span::new(0, 0))]
    );
}

/// An empty file has no `main` either.
#[test]
fn empty_program_is_missing_main() {
    assert_eq!(
        errors(""),
        vec![("missing `fn main()`".to_string(), Span::new(0, 0))]
    );
}

/// `main` must not take parameters.
#[test]
fn main_with_parameters_is_rejected_at_its_name() {
    let src = "fn main(count: i64) {\n}\n";
    assert_single_error(
        src,
        "`main` must take no parameters and return nothing",
        "main",
        0,
    );
}

/// `main` must not have a return type.
#[test]
fn main_with_return_type_is_rejected_at_its_name() {
    let src = "fn main() -> i64 {\n    return 1\n}\n";
    assert_single_error(
        src,
        "`main` must take no parameters and return nothing",
        "main",
        0,
    );
}

/// The ninth parameter is the one that is reported.
#[test]
fn nine_parameters_are_reported_at_the_ninth_name() {
    let src = "\
fn helper(p1: i64, p2: i64, p3: i64, p4: i64, p5: i64, p6: i64, p7: i64, p8: i64, p9: i64) {
}
fn main() {
}
";
    assert_single_error(src, "functions can take at most 8 parameters yet", "p9", 0);
}

/// Eight parameters, the register limit, are accepted.
#[test]
fn eight_parameters_are_accepted() {
    let src = "\
fn helper(p1: i64, p2: i64, p3: i64, p4: i64, p5: i64, p6: i64, p7: i64, p8: bool) {
}
fn main() {
}
";
    checked(src);
}

/// A function can be called before its definition appears.
#[test]
fn functions_can_be_called_before_their_definition() {
    let src = "\
fn main() {
    print(later(1))
}
fn later(val: i64) -> i64 {
    return val
}
";
    checked(src);
}

/// A function can call itself.
#[test]
fn recursion_is_allowed() {
    let src = "\
fn countdown(val: i64) -> i64 {
    if val <= 0 {
        return 0
    }
    return countdown(val - 1)
}
fn main() {
    print(countdown(3))
}
";
    checked(src);
}

// ---------------------------------------------------------------- type names

/// A parameter type that is not a known type name is an error at the type.
#[test]
fn unknown_parameter_type_is_reported_at_the_type() {
    let src = "fn helper(val: foo) {\n}\nfn main() {\n}\n";
    assert_single_error(src, "unknown type `foo`", "foo", 0);
}

/// A return type that is not a known type name is an error at the type.
#[test]
fn unknown_return_type_is_reported_at_the_type() {
    let src = "\
fn helper() -> foo {
    for {
    }
}
fn main() {
}
";
    assert_single_error(src, "unknown type `foo`", "foo", 0);
}

/// A `let` annotation that is not a known type name is an error at the type.
#[test]
fn unknown_let_annotation_is_reported_at_the_type() {
    let src = main_with("    count: foo = 1\n    print(count)");
    assert_single_error(&src, "unknown type `foo`", "foo", 0);
}

/// Every spec type outside the Phase 1 subset says it is not supported yet.
#[test]
fn spec_types_outside_the_subset_are_not_supported_yet() {
    let names = [
        "i8", "i16", "i32", "u8", "u16", "u32", "u64", "f16", "bf16", "f32", "f64", "str",
    ];
    for name in names {
        let src = format!("fn helper(val: {name}) {{\n}}\nfn main() {{\n}}\n");
        let message = format!("type `{name}` is not supported yet");
        assert_single_error(&src, &message, name, 0);
    }
}

/// An unsupported type in a `let` annotation is reported at the type.
#[test]
fn unsupported_let_annotation_is_reported_at_the_type() {
    let src = main_with("    count: f32 = 1\n    print(count)");
    assert_single_error(&src, "type `f32` is not supported yet", "f32", 0);
}

// ---------------------------------------------------------------- scopes and redeclaration

/// Declaring a name twice in the same scope is an error at the second name.
#[test]
fn redeclaration_in_same_scope_is_reported_at_the_new_name() {
    let src = main_with("    count := 1\n    print(count)\n    count := 2\n    print(count)");
    assert_single_error(&src, "`count` is already declared", "count", 2);
}

/// Shadowing an outer local inside an `if` block is an error.
#[test]
fn shadowing_in_if_block_is_rejected() {
    let body = "\
    count := 1
    print(count)
    if true {
        count := 2
        print(count)
    }";
    assert_single_error(&main_with(body), "`count` is already declared", "count", 2);
}

/// Shadowing an outer local inside a `for` body is an error.
#[test]
fn shadowing_in_for_body_is_rejected() {
    let body = "\
    count := 1
    print(count)
    for idx in 0..3 {
        count := 2
        print(count)
    }";
    assert_single_error(&main_with(body), "`count` is already declared", "count", 2);
}

/// Shadowing from a block nested two levels deep is an error.
#[test]
fn shadowing_in_nested_blocks_is_rejected() {
    let body = "\
    count := 1
    print(count)
    if true {
        if true {
            count := 2
            print(count)
        }
    }";
    assert_single_error(&main_with(body), "`count` is already declared", "count", 2);
}

/// A loop variable may not reuse the name of a visible local.
#[test]
fn loop_variable_shadowing_a_local_is_rejected() {
    let body = "\
    idx := 1
    print(idx)
    for idx in 0..3 {
    }";
    assert_single_error(&main_with(body), "`idx` is already declared", "idx", 2);
}

/// A local may not reuse the name of a parameter.
#[test]
fn local_named_like_a_parameter_is_rejected() {
    let src = "\
fn helper(val: i64) {
    val := 1
    print(val)
}
fn main() {
}
";
    assert_single_error(src, "`val` is already declared", "val", 1);
}

/// Two parameters with the same name are an error at the second one.
#[test]
fn duplicate_parameters_are_reported_at_the_second_name() {
    let src = "fn helper(dup: i64, dup: i64) {\n}\nfn main() {\n}\n";
    assert_single_error(src, "`dup` is already declared", "dup", 1);
}

/// A local may not reuse the name of a user function.
#[test]
fn local_named_like_a_function_is_rejected() {
    let src = "\
fn helper() {
}
fn main() {
    helper := 1
    print(helper)
}
";
    assert_single_error(src, "`helper` is already declared", "helper", 1);
}

/// A parameter may not reuse the name of a user function.
#[test]
fn parameter_named_like_a_function_is_rejected() {
    let src = "\
fn other() {
}
fn helper(other: i64) {
}
fn main() {
}
";
    assert_single_error(src, "`other` is already declared", "other", 1);
}

/// Once a scope has ended, its names can be declared again.
#[test]
fn name_can_be_reused_after_its_scope_ended() {
    let body = "\
    if true {
        count := 1
        print(count)
    }
    count := 2
    print(count)";
    checked(&main_with(body));
}

/// Sibling blocks are separate scopes and may declare the same name.
#[test]
fn sibling_blocks_can_declare_the_same_name() {
    let body = "\
    if true {
        count := 1
        print(count)
    } else {
        count := 2
        print(count)
    }";
    checked(&main_with(body));
}

/// Different functions are separate scopes and may declare the same name.
#[test]
fn different_functions_can_declare_the_same_name() {
    let src = "\
fn helper() {
    count := 1
    print(count)
}
fn main() {
    count := 2
    print(count)
}
";
    checked(src);
}

/// The loop variable lives only in the loop body.
#[test]
fn loop_variable_is_not_visible_after_the_loop() {
    let body = "    for idx in 0..3 {\n    }\n    print(idx)";
    assert_single_error(&main_with(body), "undefined name `idx`", "idx", 1);
}

// ---------------------------------------------------------------- predeclared names and `_`

/// The names of the types can be used for locals.
#[test]
fn locals_may_be_named_like_types() {
    let body = "    i64 := 1\n    bool := 2\n    print(i64 + bool)";
    checked(&main_with(body));
}

/// A local may be named `print` as long as it is not called.
#[test]
fn local_named_print_is_allowed() {
    let body = "    print := 1\n    print = print + 1";
    checked(&main_with(body));
}

/// After a local shadows `print`, calling `print` is an error at the callee.
#[test]
fn calling_shadowed_print_is_rejected() {
    let body = "    print := 1\n    print(print)";
    assert_single_error(&main_with(body), "`print` is not a function", "print", 1);
}

/// `_` can be declared any number of times without being used.
#[test]
fn underscore_can_be_declared_repeatedly_and_stays_unused() {
    checked(&main_with("    _ := 1\n    _ := 2"));
}

/// `_` can be a loop variable, even in nested loops.
#[test]
fn underscore_can_be_a_loop_variable_repeatedly() {
    let body = "    for _ in 0..3 {\n        for _ in 0..3 {\n        }\n    }";
    checked(&main_with(body));
}

/// `_` can be a parameter name more than once.
#[test]
fn underscore_can_be_a_parameter_repeatedly() {
    let src = "fn helper(_: i64, _: bool) {\n}\nfn main() {\n}\n";
    checked(src);
}

/// `_` is how a returned value is discarded.
#[test]
fn underscore_discards_a_call_result() {
    let src = "\
fn helper() -> i64 {
    return 1
}
fn main() {
    _ := helper()
}
";
    checked(src);
}

/// Reading `_` is an error at the name.
#[test]
fn reading_underscore_is_rejected() {
    let body = "    _ := 1\n    print(_)";
    assert_single_error(&main_with(body), "`_` cannot be used as a value", "_", 1);
}

// ---------------------------------------------------------------- undefined names and misuse

/// An unknown name in an expression is reported at the name.
#[test]
fn undefined_name_is_reported_at_the_name() {
    assert_single_error(&main_with("    print(z)"), "undefined name `z`", "z", 0);
}

/// An unknown callee is reported at the callee identifier.
#[test]
fn undefined_function_is_reported_at_the_callee() {
    assert_single_error(
        &main_with("    foo(1)"),
        "undefined function `foo`",
        "foo",
        0,
    );
}

/// Calling a local variable is an error at the callee.
#[test]
fn local_used_as_callee_is_rejected() {
    let body = "    value := 1\n    value(2)\n    print(value)";
    assert_single_error(&main_with(body), "`value` is not a function", "value", 1);
}

/// Using a function as a value is an error at the name.
#[test]
fn function_used_as_value_is_rejected() {
    let src = "\
fn helper() {
}
fn main() {
    print(helper)
}
";
    assert_single_error(src, "`helper` is a function, not a value", "helper", 1);
}

// ---------------------------------------------------------------- unused variables

/// A local that is never used is reported at its name.
#[test]
fn unused_local_is_reported_at_its_name() {
    let body = "    count := 1";
    assert_single_error(
        &main_with(body),
        "`count` is declared but never used",
        "count",
        0,
    );
}

/// Assigning to a local does not count as reading it.
#[test]
fn assigned_but_never_read_local_is_unused() {
    let body = "    count := 1\n    count = 2";
    assert_single_error(
        &main_with(body),
        "`count` is declared but never used",
        "count",
        0,
    );
}

/// Parameters and loop variables are exempt from the unused rule.
#[test]
fn unused_parameters_and_loop_variables_are_allowed() {
    let src = "\
fn helper(val: i64) {
    for idx in 0..3 {
    }
}
fn main() {
}
";
    checked(src);
}

/// A read inside a nested block counts as a use.
#[test]
fn read_in_nested_block_counts_as_use() {
    let body = "    count := 1\n    if true {\n        print(count)\n    }";
    checked(&main_with(body));
}

/// Locals of two functions are checked independently.
#[test]
fn unused_locals_in_each_function_are_reported() {
    let src = "\
fn helper() {
    first := 1
}
fn main() {
    second := 2
}
";
    assert_eq!(
        errors(src),
        vec![
            error(src, "`first` is declared but never used", "first", 0),
            error(src, "`second` is declared but never used", "second", 0),
        ]
    );
}

// ---------------------------------------------------------------- second clarifications: names

/// A local named like a function is reported once and is never reported as unused.
#[test]
fn local_named_like_a_function_is_never_reported_unused() {
    let src = "\
fn helper() {
}
fn main() {
    helper := 1
}
";
    assert_single_error(src, "`helper` is already declared", "helper", 1);
}

/// A parameter named like a function stays in scope, so reading it gives no extra error.
#[test]
fn parameter_named_like_a_function_stays_in_scope() {
    let src = "\
fn other() {
}
fn helper(other: i64) {
    print(other)
}
fn main() {
}
";
    assert_single_error(src, "`other` is already declared", "other", 1);
}

/// A local that clashes with another local is not added again, so the first one stays visible.
#[test]
fn clashing_local_is_not_added_again() {
    let body = "    count := 1\n    print(count)\n    count := true\n    print(count + 1)";
    assert_single_error(&main_with(body), "`count` is already declared", "count", 2);
}

/// Only the ninth parameter is reported, not the ones after it.
#[test]
fn tenth_parameter_is_not_reported_again() {
    let src = "\
fn helper(p1: i64, p2: i64, p3: i64, p4: i64, p5: i64, p6: i64, p7: i64, p8: i64, p9: i64, p10: i64) {
}
fn main() {
}
";
    assert_single_error(src, "functions can take at most 8 parameters yet", "p9", 0);
}

/// Assigning to `_` is an error at the target, not an undefined name.
#[test]
fn assigning_to_underscore_is_rejected_at_the_target() {
    let src = main_with("    _ := 1\n    _ = 2");
    assert_single_error(&src, "cannot assign to `_`", "_", 1);
}

/// Compound assignment to `_` is rejected the same way.
#[test]
fn compound_assigning_to_underscore_is_rejected_at_the_target() {
    let src = main_with("    _ := 1\n    _ += 2");
    assert_single_error(&src, "cannot assign to `_`", "_", 1);
}

/// The value of an assignment to `_` is still checked.
#[test]
fn value_assigned_to_underscore_is_still_checked() {
    let src = main_with("    _ := 1\n    _ = missing");
    assert_eq!(
        errors(&src),
        vec![
            error(&src, "cannot assign to `_`", "_", 1),
            error(&src, "undefined name `missing`", "missing", 0),
        ]
    );
}

// ---------------------------------------------------------------- amendment after review

/// A parameter named like its own function is reported once; calling the function is fine.
#[test]
fn parameter_named_like_its_function_can_still_call_it() {
    let src = "\
fn helper(helper: i64) -> i64 {
    return helper(1)
}
fn main() {
}
";
    assert_single_error(src, "`helper` is already declared", "helper", 1);
}

/// A local named like a function is reported once; calling the function is fine.
#[test]
fn local_named_like_a_function_can_still_call_it() {
    let src = "\
fn calc() -> i64 {
    return 1
}
fn main() {
    calc := 1
    print(calc)
    print(calc())
}
";
    assert_single_error(src, "`calc` is already declared", "calc", 1);
}

/// `main` with an unsupported return type gets only the type error.
#[test]
fn main_with_unsupported_return_type_gets_only_the_type_error() {
    let src = "fn main() -> f32 {\n    for {\n    }\n}\n";
    assert_single_error(src, "type `f32` is not supported yet", "f32", 0);
}

/// `main` with an unknown return type gets only the type error.
#[test]
fn main_with_unknown_return_type_gets_only_the_type_error() {
    let src = "fn main() -> foo {\n    for {\n    }\n}\n";
    assert_single_error(src, "unknown type `foo`", "foo", 0);
}

/// `main` with an unknown parameter type gets only the type error.
#[test]
fn main_with_unknown_parameter_type_gets_only_the_type_error() {
    let src = "fn main(val: foo) {\n}\n";
    assert_single_error(src, "unknown type `foo`", "foo", 0);
}

/// `main` with an unsupported parameter type gets only the type error.
#[test]
fn main_with_unsupported_parameter_type_gets_only_the_type_error() {
    let src = "fn main(val: f32) {\n}\n";
    assert_single_error(src, "type `f32` is not supported yet", "f32", 0);
}

/// `main` with a valid parameter still gets the `main` error.
#[test]
fn main_with_valid_parameter_type_still_gets_the_main_error() {
    let src = "fn main(val: i64) {\n}\n";
    assert_single_error(
        src,
        "`main` must take no parameters and return nothing",
        "main",
        0,
    );
}

/// A local whose initializer failed is never reported as unused.
#[test]
fn local_with_failed_initializer_is_never_reported_unused() {
    let src = main_with("    count := nope");
    assert_single_error(&src, "undefined name `nope`", "nope", 0);
}
