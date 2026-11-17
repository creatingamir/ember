//! Integration tests for the printed IR that `lower` and `print_module` produce.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ir::{lower, print_module};
use source::SourceFile;

/// Joins lines into one text that ends with a newline.
fn text(lines: &[&str]) -> String {
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

/// Parses, checks, lowers and prints a program that must be valid.
fn ir_text(src: &str) -> String {
    let program = parser::parse_source(src).unwrap();
    let checked = check::check(&program).unwrap();
    let file = SourceFile::new("t.em", src);
    let module = lower(&program, &checked, &file).expect("lowering a checked program must succeed");
    print_module(&module)
}

/// Returns the printed text of one function of a program, ending in `}\n`.
fn function_ir(src: &str, name: &str) -> String {
    let header = format!("fn {name}(");
    let full = ir_text(src);
    let section = full
        .split("\n\n")
        .find(|section| section.starts_with(&header))
        .unwrap();
    format!("{}\n", section.trim_end())
}

/// Returns the printed `main` of a program given as source lines.
fn main_ir(source_lines: &[&str]) -> String {
    function_ir(&text(source_lines), "main")
}

/// Checks `x OP= 3` after `x := 10`; the add/sub/mul/div sits at the statement start.
fn assert_compound_assign(symbol: &str, mnemonic: &str) {
    let statement = format!("    x {symbol} 3");
    let actual = main_ir(&[
        "fn main() {",
        "    x := 10",
        &statement,
        "    print(x)",
        "}",
    ]);
    let last_op = format!("  t3 = {mnemonic} t1, t2 @3:5");
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 10",
        "  store x, t0",
        "  t1 = load x",
        "  t2 = const 3",
        &last_op,
        "  store x, t3",
        "  t4 = load x",
        "  print.i64 t4",
        "  ret",
        "}",
    ]);
    assert_eq!(actual, expected, "operator {symbol}");
}

/// Checks `return a OP b` for one operator; `suffix` is the expected location text.
fn assert_operator(symbol: &str, return_type: &str, mnemonic: &str, suffix: &str) {
    let header = format!("fn f(a: i64, b: i64) -> {return_type} {{");
    let statement = format!("    return a {symbol} b");
    let source = text(&[&header, &statement, "}", "fn main() {", "}"]);
    let operation = format!("  t2 = {mnemonic} t0, t1{suffix}");
    let expected = text(&[
        "fn f(a, b) -> i64 {",
        "  slots: a, b",
        "bb0:",
        "  t0 = load a",
        "  t1 = load b",
        &operation,
        "  ret t2",
        "}",
    ]);
    assert_eq!(function_ir(&source, "f"), expected, "operator {symbol}");
}

/// The spec's fib example prints exactly, with a blank line between functions.
#[test]
fn fib_program_prints_in_full() {
    let source = text(&[
        "fn fib(n: i64) -> i64 {",
        "    if n < 2 {",
        "        return n",
        "    }",
        "    return fib(n - 1) + fib(n - 2)",
        "}",
        "fn main() {",
        "    print(fib(10))",
        "}",
    ]);
    let expected = text(&[
        "fn fib(n) -> i64 {",
        "  slots: n",
        "bb0:",
        "  t0 = load n",
        "  t1 = const 2",
        "  t2 = lt t0, t1",
        "  br t2, bb1, bb2",
        "bb1:",
        "  t3 = load n",
        "  ret t3",
        "bb2:",
        "  t4 = load n",
        "  t5 = const 1",
        "  t6 = sub t4, t5 @5:16",
        "  t7 = call fib(t6)",
        "  t8 = load n",
        "  t9 = const 2",
        "  t10 = sub t8, t9 @5:29",
        "  t11 = call fib(t10)",
        "  t12 = add t7, t11 @5:12",
        "  ret t12",
        "}",
        "",
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 10",
        "  t1 = call fib(t0)",
        "  print.i64 t1",
        "  ret",
        "}",
    ]);
    assert_eq!(ir_text(&source), expected);
}

/// The printed text of a program ends with `}` and one newline.
#[test]
fn output_ends_with_closing_brace_and_one_newline() {
    let output = ir_text(&text(&["fn main() {", "}"]));
    assert!(output.ends_with("}\n"));
    assert!(!output.ends_with("\n\n"));
}

/// An empty `main` has an empty slots line, one block and a bare `ret`.
#[test]
fn empty_main_prints_empty_slots_and_ret() {
    let expected = text(&["fn main() {", "  slots:", "bb0:", "  ret", "}"]);
    assert_eq!(main_ir(&["fn main() {", "}"]), expected);
}

/// The empty slots line is exactly `  slots:` with no trailing space.
#[test]
fn empty_slots_line_has_nothing_after_the_colon() {
    let output = main_ir(&["fn main() {", "    print(1)", "}"]);
    let slots_line = output.lines().nth(1).unwrap();
    assert_eq!(slots_line, "  slots:");
}

/// `let` lowers the value, stores it, and a later read loads the slot.
#[test]
fn let_stores_and_name_read_loads() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 5",
        "  store x, t0",
        "  t1 = load x",
        "  print.i64 t1",
        "  ret",
        "}",
    ]);
    assert_eq!(
        main_ir(&["fn main() {", "    x := 5", "    print(x)", "}"]),
        expected
    );
}

/// A typed `let` lowers like an untyped one.
#[test]
fn typed_let_lowers_like_untyped_let() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 4",
        "  store x, t0",
        "  t1 = load x",
        "  print.i64 t1",
        "  ret",
        "}",
    ]);
    assert_eq!(
        main_ir(&["fn main() {", "    x: i64 = 4", "    print(x)", "}"]),
        expected
    );
}

/// `+ - * / %` print their mnemonic and carry the expression's `@L:C`.
#[test]
fn arithmetic_operators_carry_their_location() {
    let table = [
        ("+", "add"),
        ("-", "sub"),
        ("*", "mul"),
        ("/", "div"),
        ("%", "rem"),
    ];
    for (symbol, mnemonic) in table {
        assert_operator(symbol, "i64", mnemonic, " @2:12");
    }
}

/// Comparison operators print their mnemonic and carry no location.
#[test]
fn comparison_operators_carry_no_location() {
    let table = [
        ("==", "eq"),
        ("!=", "ne"),
        ("<", "lt"),
        ("<=", "le"),
        (">", "gt"),
        (">=", "ge"),
    ];
    for (symbol, mnemonic) in table {
        assert_operator(symbol, "bool", mnemonic, "");
    }
}

/// Printing a comparison result uses `print.bool`.
#[test]
fn printing_a_comparison_uses_print_bool() {
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 1",
        "  t1 = const 2",
        "  t2 = lt t0, t1",
        "  print.bool t2",
        "  ret",
        "}",
    ]);
    assert_eq!(main_ir(&["fn main() {", "    print(1 < 2)", "}"]), expected);
}

/// A binary expression's location is its first operand's start, inside parentheses.
#[test]
fn binary_location_is_the_start_of_the_expression() {
    let source = text(&[
        "fn f(a: i64, b: i64, c: i64) -> i64 {",
        "    return (a + b) * c",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn f(a, b, c) -> i64 {",
        "  slots: a, b, c",
        "bb0:",
        "  t0 = load a",
        "  t1 = load b",
        "  t2 = add t0, t1 @2:13",
        "  t3 = load c",
        "  t4 = mul t2, t3 @2:13",
        "  ret t4",
        "}",
    ]);
    assert_eq!(function_ir(&source, "f"), expected);
}

/// Unary minus prints `neg` with the location of the `-`.
#[test]
fn neg_carries_the_expression_location() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 3",
        "  store x, t0",
        "  t1 = load x",
        "  t2 = neg t1 @3:11",
        "  print.i64 t2",
        "  ret",
        "}",
    ]);
    assert_eq!(
        main_ir(&["fn main() {", "    x := 3", "    print(-x)", "}"]),
        expected
    );
}

/// Unary minus on an ordinary literal is a const followed by `neg`.
#[test]
fn negated_ordinary_literal_is_const_then_neg() {
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 5",
        "  t1 = neg t0 @2:11",
        "  print.i64 t1",
        "  ret",
        "}",
    ]);
    assert_eq!(main_ir(&["fn main() {", "    print(-5)", "}"]), expected);
}

/// `-9223372036854775808` is a single const holding `i64::MIN`, with no `neg`.
#[test]
fn min_literal_is_one_const() {
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const -9223372036854775808",
        "  print.i64 t0",
        "  ret",
        "}",
    ]);
    assert_eq!(
        main_ir(&["fn main() {", "    print(-9223372036854775808)", "}"]),
        expected
    );
}

/// `!` prints `not` with no location.
#[test]
fn not_lowers_without_a_location() {
    let expected = text(&[
        "fn main() {",
        "  slots: b",
        "bb0:",
        "  t0 = const 1",
        "  store b, t0",
        "  t1 = load b",
        "  t2 = not t1",
        "  print.bool t2",
        "  ret",
        "}",
    ]);
    assert_eq!(
        main_ir(&["fn main() {", "    b := true", "    print(!b)", "}"]),
        expected
    );
}

/// `true` and `false` are consts 1 and 0, printed with `print.bool`.
#[test]
fn bool_literals_are_one_and_zero() {
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 1",
        "  print.bool t0",
        "  t1 = const 0",
        "  print.bool t1",
        "  ret",
        "}",
    ]);
    assert_eq!(
        main_ir(&["fn main() {", "    print(true)", "    print(false)", "}"]),
        expected
    );
}

/// Plain assignment lowers the value and stores it into the existing slot.
#[test]
fn assignment_stores_into_the_existing_slot() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 1",
        "  store x, t0",
        "  t1 = const 2",
        "  store x, t1",
        "  t2 = load x",
        "  print.i64 t2",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x := 1",
        "    x = 2",
        "    print(x)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `+=` loads the target, adds, and stores, with the statement's location.
#[test]
fn add_assign_lowers_to_load_add_store() {
    assert_compound_assign("+=", "add");
}

/// `-=` loads the target, subtracts, and stores, with the statement's location.
#[test]
fn sub_assign_lowers_to_load_sub_store() {
    assert_compound_assign("-=", "sub");
}

/// `*=` loads the target, multiplies, and stores, with the statement's location.
#[test]
fn mul_assign_lowers_to_load_mul_store() {
    assert_compound_assign("*=", "mul");
}

/// `/=` loads the target, divides, and stores, with the statement's location.
#[test]
fn div_assign_lowers_to_load_div_store() {
    assert_compound_assign("/=", "div");
}

/// A compound assignment loads its target before it lowers the value.
#[test]
fn compound_assign_loads_target_before_the_value() {
    let expected = text(&[
        "fn main() {",
        "  slots: x, y",
        "bb0:",
        "  t0 = const 1",
        "  store x, t0",
        "  t1 = const 2",
        "  store y, t1",
        "  t2 = load x",
        "  t3 = load y",
        "  t4 = const 3",
        "  t5 = mul t3, t4 @4:10",
        "  t6 = add t2, t5 @4:5",
        "  store x, t6",
        "  t7 = load x",
        "  print.i64 t7",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x := 1",
        "    y := 2",
        "    x += y * 3",
        "    print(x)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// A parameter is read and written through its own slot.
#[test]
fn compound_assign_on_a_parameter_uses_its_slot() {
    let source = text(&[
        "fn inc(n: i64) -> i64 {",
        "    n += 1",
        "    return n",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn inc(n) -> i64 {",
        "  slots: n",
        "bb0:",
        "  t0 = load n",
        "  t1 = const 1",
        "  t2 = add t0, t1 @2:5",
        "  store n, t2",
        "  t3 = load n",
        "  ret t3",
        "}",
    ]);
    assert_eq!(function_ir(&source, "inc"), expected);
}

/// A `let` slot is created after the hidden slots made while lowering its value.
#[test]
fn let_slot_comes_after_hidden_slots_of_its_value() {
    let expected = text(&[
        "fn main() {",
        "  slots: a, b, $and, c",
        "bb0:",
        "  t0 = const 1",
        "  store a, t0",
        "  t1 = const 0",
        "  store b, t1",
        "  t2 = load a",
        "  store $and, t2",
        "  br t2, bb1, bb2",
        "bb1:",
        "  t3 = load b",
        "  store $and, t3",
        "  jump bb2",
        "bb2:",
        "  t4 = load $and",
        "  store c, t4",
        "  t5 = load c",
        "  print.bool t5",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    a := true",
        "    b := false",
        "    c := a && b",
        "    print(c)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// Two `$and` slots print as `$and` and `$and#k`, in slots, loads and stores alike.
#[test]
fn duplicate_hidden_slot_names_get_their_slot_number() {
    let expected = text(&[
        "fn main() {",
        "  slots: a, $and, $and#2",
        "bb0:",
        "  t0 = const 1",
        "  store a, t0",
        "  t1 = load a",
        "  store $and, t1",
        "  br t1, bb1, bb2",
        "bb1:",
        "  t2 = load a",
        "  store $and, t2",
        "  jump bb2",
        "bb2:",
        "  t3 = load $and",
        "  print.bool t3",
        "  t4 = load a",
        "  store $and#2, t4",
        "  br t4, bb3, bb4",
        "bb3:",
        "  t5 = load a",
        "  store $and#2, t5",
        "  jump bb4",
        "bb4:",
        "  t6 = load $and#2",
        "  print.bool t6",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    a := true",
        "    print(a && a)",
        "    print(a && a)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// Locals with the same name in sibling scopes print as `x` and `x#k`.
#[test]
fn sibling_scope_locals_get_their_slot_number() {
    let expected = text(&[
        "fn main() {",
        "  slots: x, x#1",
        "bb0:",
        "  t0 = const 1",
        "  br t0, bb1, bb2",
        "bb1:",
        "  t1 = const 1",
        "  store x, t1",
        "  t2 = load x",
        "  print.i64 t2",
        "  jump bb2",
        "bb2:",
        "  t3 = const 1",
        "  br t3, bb3, bb4",
        "bb3:",
        "  t4 = const 2",
        "  store x#1, t4",
        "  t5 = load x#1",
        "  print.i64 t5",
        "  jump bb4",
        "bb4:",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    if true {",
        "        x := 1",
        "        print(x)",
        "    }",
        "    if true {",
        "        x := 2",
        "        print(x)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// Call arguments are lowered left to right and the call gets a result temp.
#[test]
fn call_with_result_lowers_arguments_left_to_right() {
    let source = text(&[
        "fn two(a: i64, b: i64) -> i64 {",
        "    return a - b",
        "}",
        "fn main() {",
        "    print(two(1 + 2, 3 * 4))",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 1",
        "  t1 = const 2",
        "  t2 = add t0, t1 @5:15",
        "  t3 = const 3",
        "  t4 = const 4",
        "  t5 = mul t3, t4 @5:22",
        "  t6 = call two(t2, t5)",
        "  print.i64 t6",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A call to a function without a return value has no result temp.
#[test]
fn call_without_result_has_no_destination() {
    let source = text(&[
        "fn say(x: i64) {",
        "    print(x)",
        "}",
        "fn main() {",
        "    say(5)",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 5",
        "  call say(t0)",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A function without a return value has no ` -> i64` and ends with `ret`.
#[test]
fn function_without_return_value_ends_with_ret() {
    let source = text(&[
        "fn say(x: i64) {",
        "    print(x)",
        "}",
        "fn main() {",
        "    say(5)",
        "}",
    ]);
    let expected = text(&[
        "fn say(x) {",
        "  slots: x",
        "bb0:",
        "  t0 = load x",
        "  print.i64 t0",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "say"), expected);
}

/// A call with no arguments prints empty parentheses.
#[test]
fn call_with_no_arguments_prints_empty_parentheses() {
    let source = text(&[
        "fn zero() -> i64 {",
        "    return 0",
        "}",
        "fn main() {",
        "    print(zero())",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = call zero()",
        "  print.i64 t0",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A discarded call result still has a destination temp.
#[test]
fn discarded_call_result_keeps_its_destination() {
    let source = text(&[
        "fn two(a: i64, b: i64) -> i64 {",
        "    return a - b",
        "}",
        "fn main() {",
        "    two(1, 2)",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 1",
        "  t1 = const 2",
        "  t2 = call two(t0, t1)",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A bool-returning function prints ` -> i64` and its call prints with `print.bool`.
#[test]
fn bool_function_has_value_header_and_prints_as_bool() {
    let source = text(&[
        "fn yes() -> bool {",
        "    return true",
        "}",
        "fn main() {",
        "    print(yes())",
        "}",
    ]);
    let expected_yes = text(&[
        "fn yes() -> i64 {",
        "  slots:",
        "bb0:",
        "  t0 = const 1",
        "  ret t0",
        "}",
    ]);
    let expected_main = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = call yes()",
        "  print.bool t0",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "yes"), expected_yes);
    assert_eq!(function_ir(&source, "main"), expected_main);
}

/// `a && b` stores a into `$and`, evaluates b only when a is true, and joins.
#[test]
fn and_uses_a_hidden_slot_and_two_blocks() {
    let expected = text(&[
        "fn main() {",
        "  slots: a, b, $and",
        "bb0:",
        "  t0 = const 1",
        "  store a, t0",
        "  t1 = const 0",
        "  store b, t1",
        "  t2 = load a",
        "  store $and, t2",
        "  br t2, bb1, bb2",
        "bb1:",
        "  t3 = load b",
        "  store $and, t3",
        "  jump bb2",
        "bb2:",
        "  t4 = load $and",
        "  print.bool t4",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    a := true",
        "    b := false",
        "    print(a && b)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `a || b` mirrors `&&`: a true goes straight to the join block.
#[test]
fn or_uses_a_hidden_slot_and_two_blocks() {
    let expected = text(&[
        "fn main() {",
        "  slots: a, b, $or",
        "bb0:",
        "  t0 = const 1",
        "  store a, t0",
        "  t1 = const 0",
        "  store b, t1",
        "  t2 = load a",
        "  store $or, t2",
        "  br t2, bb2, bb1",
        "bb1:",
        "  t3 = load b",
        "  store $or, t3",
        "  jump bb2",
        "bb2:",
        "  t4 = load $or",
        "  print.bool t4",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    a := true",
        "    b := false",
        "    print(a || b)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `if` without `else` branches straight to the join block.
#[test]
fn if_without_else_branches_to_the_join_block() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 1",
        "  store x, t0",
        "  t1 = load x",
        "  t2 = const 2",
        "  t3 = lt t1, t2",
        "  br t3, bb1, bb2",
        "bb1:",
        "  t4 = const 1",
        "  print.i64 t4",
        "  jump bb2",
        "bb2:",
        "  t5 = const 3",
        "  print.i64 t5",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x := 1",
        "    if x < 2 {",
        "        print(1)",
        "    }",
        "    print(3)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `if`/`else` has a then block, an else block and a join block, in that order.
#[test]
fn if_else_has_then_else_and_join_blocks() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 1",
        "  store x, t0",
        "  t1 = load x",
        "  t2 = const 2",
        "  t3 = lt t1, t2",
        "  br t3, bb1, bb2",
        "bb1:",
        "  t4 = const 1",
        "  print.i64 t4",
        "  jump bb3",
        "bb2:",
        "  t5 = const 2",
        "  print.i64 t5",
        "  jump bb3",
        "bb3:",
        "  t6 = const 3",
        "  print.i64 t6",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x := 1",
        "    if x < 2 {",
        "        print(1)",
        "    } else {",
        "        print(2)",
        "    }",
        "    print(3)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `else if` is an else block holding the nested `if`, which has its own join block.
#[test]
fn else_if_nests_an_if_in_the_else_block() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 1",
        "  store x, t0",
        "  t1 = load x",
        "  t2 = const 1",
        "  t3 = lt t1, t2",
        "  br t3, bb1, bb2",
        "bb1:",
        "  t4 = const 1",
        "  print.i64 t4",
        "  jump bb3",
        "bb2:",
        "  t5 = load x",
        "  t6 = const 2",
        "  t7 = lt t5, t6",
        "  br t7, bb4, bb5",
        "bb3:",
        "  ret",
        "bb4:",
        "  t8 = const 2",
        "  print.i64 t8",
        "  jump bb6",
        "bb5:",
        "  t9 = const 3",
        "  print.i64 t9",
        "  jump bb6",
        "bb6:",
        "  jump bb3",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x := 1",
        "    if x < 1 {",
        "        print(1)",
        "    } else if x < 2 {",
        "        print(2)",
        "    } else {",
        "        print(3)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `for {}` jumps to its body, and `break` jumps to the exit block.
#[test]
fn infinite_loop_with_break_jumps_to_the_exit_block() {
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  jump bb1",
        "bb1:",
        "  jump bb2",
        "bb2:",
        "  t0 = const 1",
        "  print.i64 t0",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for {",
        "        break",
        "    }",
        "    print(1)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// In `for {}`, `break` goes to the exit, `continue` to the body start, and the body end loops back.
#[test]
fn infinite_loop_with_break_and_continue_inside_ifs() {
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = const 0",
        "  store x, t0",
        "  jump bb1",
        "bb1:",
        "  t1 = load x",
        "  t2 = const 1",
        "  t3 = add t1, t2 @4:9",
        "  store x, t3",
        "  t4 = load x",
        "  t5 = const 3",
        "  t6 = gt t4, t5",
        "  br t6, bb3, bb4",
        "bb2:",
        "  t11 = const 9",
        "  print.i64 t11",
        "  ret",
        "bb3:",
        "  jump bb2",
        "bb4:",
        "  t7 = load x",
        "  t8 = const 2",
        "  t9 = eq t7, t8",
        "  br t9, bb5, bb6",
        "bb5:",
        "  jump bb1",
        "bb6:",
        "  t10 = load x",
        "  print.i64 t10",
        "  jump bb1",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x := 0",
        "    for {",
        "        x += 1",
        "        if x > 3 {",
        "            break",
        "        }",
        "        if x == 2 {",
        "            continue",
        "        }",
        "        print(x)",
        "    }",
        "    print(9)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// Statements after `break` form an unreachable block that is removed.
#[test]
fn statements_after_break_are_removed() {
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  jump bb1",
        "bb1:",
        "  jump bb2",
        "bb2:",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for {",
        "        break",
        "        print(1)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `for cond {}` has a header that tests the condition and a body that jumps back to it.
#[test]
fn while_loop_tests_the_condition_in_a_header_block() {
    let expected = text(&[
        "fn main() {",
        "  slots: i",
        "bb0:",
        "  t0 = const 0",
        "  store i, t0",
        "  jump bb1",
        "bb1:",
        "  t1 = load i",
        "  t2 = const 3",
        "  t3 = lt t1, t2",
        "  br t3, bb2, bb3",
        "bb2:",
        "  t4 = load i",
        "  t5 = const 1",
        "  t6 = add t4, t5 @4:9",
        "  store i, t6",
        "  jump bb1",
        "bb3:",
        "  t7 = load i",
        "  print.i64 t7",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    i := 0",
        "    for i < 3 {",
        "        i += 1",
        "    }",
        "    print(i)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// In `for cond {}`, `continue` goes to the header and `break` to the exit.
#[test]
fn while_loop_break_and_continue_targets() {
    let expected = text(&[
        "fn main() {",
        "  slots: i",
        "bb0:",
        "  t0 = const 0",
        "  store i, t0",
        "  jump bb1",
        "bb1:",
        "  t1 = load i",
        "  t2 = const 5",
        "  t3 = lt t1, t2",
        "  br t3, bb2, bb3",
        "bb2:",
        "  t4 = load i",
        "  t5 = const 1",
        "  t6 = add t4, t5 @4:9",
        "  store i, t6",
        "  t7 = load i",
        "  t8 = const 2",
        "  t9 = eq t7, t8",
        "  br t9, bb4, bb5",
        "bb3:",
        "  ret",
        "bb4:",
        "  jump bb1",
        "bb5:",
        "  t10 = load i",
        "  t11 = const 4",
        "  t12 = eq t10, t11",
        "  br t12, bb6, bb7",
        "bb6:",
        "  jump bb3",
        "bb7:",
        "  t13 = load i",
        "  print.i64 t13",
        "  jump bb1",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    i := 0",
        "    for i < 5 {",
        "        i += 1",
        "        if i == 2 {",
        "            continue",
        "        }",
        "        if i == 4 {",
        "            break",
        "        }",
        "        print(i)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `for i in a..b` stores i and `$end`, tests `lt` in the header, and steps with a checked add.
#[test]
fn exclusive_range_loop_uses_lt_and_a_step_block() {
    let expected = text(&[
        "fn main() {",
        "  slots: i, $end",
        "bb0:",
        "  t0 = const 0",
        "  store i, t0",
        "  t1 = const 3",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2 = load i",
        "  t3 = load $end",
        "  t4 = lt t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5 = load i",
        "  print.i64 t5",
        "  jump bb3",
        "bb3:",
        "  t6 = load i",
        "  t7 = const 1",
        "  t8 = add t6, t7 @2:5",
        "  store i, t8",
        "  jump bb1",
        "bb4:",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for i in 0..3 {",
        "        print(i)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// In `for i in a..b`, `break` goes to the exit and `continue` to the step block.
#[test]
fn exclusive_range_loop_break_and_continue_targets() {
    let expected = text(&[
        "fn main() {",
        "  slots: i, $end",
        "bb0:",
        "  t0 = const 0",
        "  store i, t0",
        "  t1 = const 5",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2 = load i",
        "  t3 = load $end",
        "  t4 = lt t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5 = load i",
        "  t6 = const 3",
        "  t7 = eq t5, t6",
        "  br t7, bb5, bb6",
        "bb3:",
        "  t12 = load i",
        "  t13 = const 1",
        "  t14 = add t12, t13 @2:5",
        "  store i, t14",
        "  jump bb1",
        "bb4:",
        "  ret",
        "bb5:",
        "  jump bb4",
        "bb6:",
        "  t8 = load i",
        "  t9 = const 1",
        "  t10 = eq t8, t9",
        "  br t10, bb7, bb8",
        "bb7:",
        "  jump bb3",
        "bb8:",
        "  t11 = load i",
        "  print.i64 t11",
        "  jump bb3",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for i in 0..5 {",
        "        if i == 3 {",
        "            break",
        "        }",
        "        if i == 1 {",
        "            continue",
        "        }",
        "        print(i)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// `..=` tests `le` in the header and checks `eq` before it increments, so `end + 1` is never computed.
#[test]
fn inclusive_range_loop_checks_eq_before_incrementing() {
    let expected = text(&[
        "fn main() {",
        "  slots: i, $end",
        "bb0:",
        "  t0 = const 1",
        "  store i, t0",
        "  t1 = const 3",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2 = load i",
        "  t3 = load $end",
        "  t4 = le t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5 = load i",
        "  print.i64 t5",
        "  jump bb3",
        "bb3:",
        "  t6 = load i",
        "  t7 = load $end",
        "  t8 = eq t6, t7",
        "  br t8, bb4, bb5",
        "bb4:",
        "  ret",
        "bb5:",
        "  t9 = load i",
        "  t10 = const 1",
        "  t11 = add t9, t10 @2:5",
        "  store i, t11",
        "  jump bb1",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for i in 1..=3 {",
        "        print(i)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// In `for i in a..=b`, `continue` goes to the step block that does the `eq` check.
#[test]
fn inclusive_range_loop_continue_goes_to_the_step_block() {
    let expected = text(&[
        "fn main() {",
        "  slots: i, $end",
        "bb0:",
        "  t0 = const 0",
        "  store i, t0",
        "  t1 = const 2",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2 = load i",
        "  t3 = load $end",
        "  t4 = le t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  jump bb3",
        "bb3:",
        "  t5 = load i",
        "  t6 = load $end",
        "  t7 = eq t5, t6",
        "  br t7, bb4, bb5",
        "bb4:",
        "  ret",
        "bb5:",
        "  t8 = load i",
        "  t9 = const 1",
        "  t10 = add t8, t9 @2:5",
        "  store i, t10",
        "  jump bb1",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for i in 0..=2 {",
        "        continue",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// Nested range loops make two `$end` slots; the step location is each `for` statement's start.
#[test]
fn nested_range_loops_number_their_end_slots() {
    let expected = text(&[
        "fn main() {",
        "  slots: i, $end, j, $end#3",
        "bb0:",
        "  t0 = const 0",
        "  store i, t0",
        "  t1 = const 2",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2 = load i",
        "  t3 = load $end",
        "  t4 = lt t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5 = const 0",
        "  store j, t5",
        "  t6 = const 2",
        "  store $end#3, t6",
        "  jump bb5",
        "bb3:",
        "  t14 = load i",
        "  t15 = const 1",
        "  t16 = add t14, t15 @2:5",
        "  store i, t16",
        "  jump bb1",
        "bb4:",
        "  ret",
        "bb5:",
        "  t7 = load j",
        "  t8 = load $end#3",
        "  t9 = lt t7, t8",
        "  br t9, bb6, bb8",
        "bb6:",
        "  t10 = load j",
        "  print.i64 t10",
        "  jump bb7",
        "bb7:",
        "  t11 = load j",
        "  t12 = const 1",
        "  t13 = add t11, t12 @3:9",
        "  store j, t13",
        "  jump bb5",
        "bb8:",
        "  jump bb3",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    for i in 0..2 {",
        "        for j in 0..2 {",
        "            print(j)",
        "        }",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// A bare `return` in a function without a value is a single `ret`.
#[test]
fn early_return_without_value_prints_ret() {
    let expected = text(&["fn main() {", "  slots:", "bb0:", "  ret", "}"]);
    assert_eq!(main_ir(&["fn main() {", "    return", "}"]), expected);
}

/// Statements after `return` are lowered into a block nothing jumps to, and that block is removed.
#[test]
fn code_after_return_is_removed() {
    let source = text(&[
        "fn g(c: bool) -> i64 {",
        "    if c {",
        "        return 1",
        "        print(2)",
        "    }",
        "    return 3",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn g(c) -> i64 {",
        "  slots: c",
        "bb0:",
        "  t0 = load c",
        "  br t0, bb1, bb2",
        "bb1:",
        "  t1 = const 1",
        "  ret t1",
        "bb2:",
        "  t3 = const 3",
        "  ret t3",
        "}",
    ]);
    assert_eq!(function_ir(&source, "g"), expected);
}

/// Blocks after a removed block are renumbered and every reference follows.
#[test]
fn removed_blocks_renumber_the_remaining_blocks() {
    let expected = text(&[
        "fn main() {",
        "  slots: c",
        "bb0:",
        "  t0 = const 1",
        "  store c, t0",
        "  t1 = load c",
        "  br t1, bb1, bb2",
        "bb1:",
        "  jump bb3",
        "bb2:",
        "  t2 = load c",
        "  br t2, bb4, bb5",
        "bb3:",
        "  jump bb3",
        "bb4:",
        "  t3 = const 1",
        "  print.i64 t3",
        "  jump bb5",
        "bb5:",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    c := true",
        "    if c {",
        "        for {",
        "        }",
        "    }",
        "    if c {",
        "        print(1)",
        "    }",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// A returning function whose loop always returns ends with no `unreachable` block.
#[test]
fn infinite_loop_with_return_leaves_no_unreachable_block() {
    let source = text(&[
        "fn f() -> i64 {",
        "    for {",
        "        return 1",
        "    }",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn f() -> i64 {",
        "  slots:",
        "bb0:",
        "  jump bb1",
        "bb1:",
        "  t0 = const 1",
        "  ret t0",
        "}",
    ]);
    let actual = function_ir(&source, "f");
    assert_eq!(actual, expected);
    assert!(!ir_text(&source).contains("unreachable"));
}

/// An `if`/`else` that returns on both sides leaves no join block and no `unreachable`.
#[test]
fn if_else_returning_on_both_sides_leaves_no_unreachable_block() {
    let source = text(&[
        "fn g(c: bool) -> i64 {",
        "    if c {",
        "        return 1",
        "    } else {",
        "        return 2",
        "    }",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn g(c) -> i64 {",
        "  slots: c",
        "bb0:",
        "  t0 = load c",
        "  br t0, bb1, bb2",
        "bb1:",
        "  t1 = const 1",
        "  ret t1",
        "bb2:",
        "  t2 = const 2",
        "  ret t2",
        "}",
    ]);
    assert_eq!(function_ir(&source, "g"), expected);
    assert!(!ir_text(&source).contains("unreachable"));
}
