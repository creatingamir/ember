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

/// Checks `x OP= 3` after `x := 10`; the add/sub/mul/div sits at the statement start. `mnemonic` includes the sign suffix.
fn assert_compound_assign(symbol: &str, mnemonic: &str) {
    let statement = format!("    x {symbol} 3");
    let actual = main_ir(&[
        "fn main() {",
        "    x := 10",
        &statement,
        "    print(x)",
        "}",
    ]);
    let last_op = format!("  t3:i64 = {mnemonic} t1, t2 @3:5");
    let expected = text(&[
        "fn main() {",
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 10",
        "  store x, t0",
        "  t1:i64 = load x",
        "  t2:i64 = const 3",
        &last_op,
        "  store x, t3",
        "  t4:i64 = load x",
        "  print.i64 t4",
        "  ret",
        "}",
    ]);
    assert_eq!(actual, expected, "operator {symbol}");
}

/// Checks `return a OP b` for one operator; `mnemonic` includes the sign suffix and `suffix` is the expected location text.
fn assert_operator(symbol: &str, return_type: &str, mnemonic: &str, suffix: &str) {
    let header = format!("fn f(a: i64, b: i64) -> {return_type} {{");
    let statement = format!("    return a {symbol} b");
    let source = text(&[&header, &statement, "}", "fn main() {", "}"]);
    let ir_type = if return_type == "bool" { "i8" } else { "i64" };
    let operation = format!("  t2:{ir_type} = {mnemonic} t0, t1{suffix}");
    let header = format!("fn f(a:i64, b:i64) -> {ir_type} {{");
    let expected = text(&[
        &header,
        "  slots: a:8, b:8",
        "bb0:",
        "  t0:i64 = load a",
        "  t1:i64 = load b",
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
        "fn fib(n:i64) -> i64 {",
        "  slots: n:8",
        "bb0:",
        "  t0:i64 = load n",
        "  t1:i64 = const 2",
        "  t2:i8 = lt.s t0, t1",
        "  br t2, bb1, bb2",
        "bb1:",
        "  t3:i64 = load n",
        "  ret t3",
        "bb2:",
        "  t4:i64 = load n",
        "  t5:i64 = const 1",
        "  t6:i64 = sub.s t4, t5 @5:16",
        "  t7:i64 = call fib(t6)",
        "  t8:i64 = load n",
        "  t9:i64 = const 2",
        "  t10:i64 = sub.s t8, t9 @5:29",
        "  t11:i64 = call fib(t10)",
        "  t12:i64 = add.s t7, t11 @5:12",
        "  ret t12",
        "}",
        "",
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0:i64 = const 10",
        "  t1:i64 = call fib(t0)",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 5",
        "  store x, t0",
        "  t1:i64 = load x",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 4",
        "  store x, t0",
        "  t1:i64 = load x",
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
        ("+", "add.s"),
        ("-", "sub.s"),
        ("*", "mul.s"),
        ("/", "div.s"),
        ("%", "rem.s"),
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
        ("<", "lt.s"),
        ("<=", "le.s"),
        (">", "gt.s"),
        (">=", "ge.s"),
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
        "  t0:i64 = const 1",
        "  t1:i64 = const 2",
        "  t2:i8 = lt.s t0, t1",
        "  print.bool t2",
        "  ret",
        "}",
    ]);
    assert_eq!(main_ir(&["fn main() {", "    print(1 < 2)", "}"]), expected);
}

/// A binary expression's location is its first operand's start; a parenthesized operand starts at its `(`.
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
        "fn f(a:i64, b:i64, c:i64) -> i64 {",
        "  slots: a:8, b:8, c:8",
        "bb0:",
        "  t0:i64 = load a",
        "  t1:i64 = load b",
        "  t2:i64 = add.s t0, t1 @2:12",
        "  t3:i64 = load c",
        "  t4:i64 = mul.s t2, t3 @2:12",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 3",
        "  store x, t0",
        "  t1:i64 = load x",
        "  t2:i64 = neg t1 @3:11",
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
        "  t0:i64 = const 5",
        "  t1:i64 = neg t0 @2:11",
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
        "  t0:i64 = const -9223372036854775808",
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
        "  slots: b:1",
        "bb0:",
        "  t0:i8 = const 1",
        "  store b, t0",
        "  t1:i8 = load.u b",
        "  t2:i8 = not t1",
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
        "  t0:i8 = const 1",
        "  print.bool t0",
        "  t1:i8 = const 0",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 1",
        "  store x, t0",
        "  t1:i64 = const 2",
        "  store x, t1",
        "  t2:i64 = load x",
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
    assert_compound_assign("+=", "add.s");
}

/// `-=` loads the target, subtracts, and stores, with the statement's location.
#[test]
fn sub_assign_lowers_to_load_sub_store() {
    assert_compound_assign("-=", "sub.s");
}

/// `*=` loads the target, multiplies, and stores, with the statement's location.
#[test]
fn mul_assign_lowers_to_load_mul_store() {
    assert_compound_assign("*=", "mul.s");
}

/// `/=` loads the target, divides, and stores, with the statement's location.
#[test]
fn div_assign_lowers_to_load_div_store() {
    assert_compound_assign("/=", "div.s");
}

/// A compound assignment loads its target before it lowers the value.
#[test]
fn compound_assign_loads_target_before_the_value() {
    let expected = text(&[
        "fn main() {",
        "  slots: x:8, y:8",
        "bb0:",
        "  t0:i64 = const 1",
        "  store x, t0",
        "  t1:i64 = const 2",
        "  store y, t1",
        "  t2:i64 = load x",
        "  t3:i64 = load y",
        "  t4:i64 = const 3",
        "  t5:i64 = mul.s t3, t4 @4:10",
        "  t6:i64 = add.s t2, t5 @4:5",
        "  store x, t6",
        "  t7:i64 = load x",
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
        "fn inc(n:i64) -> i64 {",
        "  slots: n:8",
        "bb0:",
        "  t0:i64 = load n",
        "  t1:i64 = const 1",
        "  t2:i64 = add.s t0, t1 @2:5",
        "  store n, t2",
        "  t3:i64 = load n",
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
        "  slots: a:1, b:1, $and:1, c:1",
        "bb0:",
        "  t0:i8 = const 1",
        "  store a, t0",
        "  t1:i8 = const 0",
        "  store b, t1",
        "  t2:i8 = load.u a",
        "  store $and, t2",
        "  br t2, bb1, bb2",
        "bb1:",
        "  t3:i8 = load.u b",
        "  store $and, t3",
        "  jump bb2",
        "bb2:",
        "  t4:i8 = load.u $and",
        "  store c, t4",
        "  t5:i8 = load.u c",
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
        "  slots: a:1, $and:1, $and#2:1",
        "bb0:",
        "  t0:i8 = const 1",
        "  store a, t0",
        "  t1:i8 = load.u a",
        "  store $and, t1",
        "  br t1, bb1, bb2",
        "bb1:",
        "  t2:i8 = load.u a",
        "  store $and, t2",
        "  jump bb2",
        "bb2:",
        "  t3:i8 = load.u $and",
        "  print.bool t3",
        "  t4:i8 = load.u a",
        "  store $and#2, t4",
        "  br t4, bb3, bb4",
        "bb3:",
        "  t5:i8 = load.u a",
        "  store $and#2, t5",
        "  jump bb4",
        "bb4:",
        "  t6:i8 = load.u $and#2",
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
        "  slots: x:8, x#1:8",
        "bb0:",
        "  t0:i8 = const 1",
        "  br t0, bb1, bb2",
        "bb1:",
        "  t1:i64 = const 1",
        "  store x, t1",
        "  t2:i64 = load x",
        "  print.i64 t2",
        "  jump bb2",
        "bb2:",
        "  t3:i8 = const 1",
        "  br t3, bb3, bb4",
        "bb3:",
        "  t4:i64 = const 2",
        "  store x#1, t4",
        "  t5:i64 = load x#1",
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
        "  t0:i64 = const 1",
        "  t1:i64 = const 2",
        "  t2:i64 = add.s t0, t1 @5:15",
        "  t3:i64 = const 3",
        "  t4:i64 = const 4",
        "  t5:i64 = mul.s t3, t4 @5:22",
        "  t6:i64 = call two(t2, t5)",
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
        "  t0:i64 = const 5",
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
        "fn say(x:i64) {",
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = load x",
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
        "  t0:i64 = call zero()",
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
        "  t0:i64 = const 1",
        "  t1:i64 = const 2",
        "  t2:i64 = call two(t0, t1)",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A bool-returning function prints ` -> i8` and its call prints with `print.bool`.
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
        "fn yes() -> i8 {",
        "  slots:",
        "bb0:",
        "  t0:i8 = const 1",
        "  ret t0",
        "}",
    ]);
    let expected_main = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0:i8 = call yes()",
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
        "  slots: a:1, b:1, $and:1",
        "bb0:",
        "  t0:i8 = const 1",
        "  store a, t0",
        "  t1:i8 = const 0",
        "  store b, t1",
        "  t2:i8 = load.u a",
        "  store $and, t2",
        "  br t2, bb1, bb2",
        "bb1:",
        "  t3:i8 = load.u b",
        "  store $and, t3",
        "  jump bb2",
        "bb2:",
        "  t4:i8 = load.u $and",
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
        "  slots: a:1, b:1, $or:1",
        "bb0:",
        "  t0:i8 = const 1",
        "  store a, t0",
        "  t1:i8 = const 0",
        "  store b, t1",
        "  t2:i8 = load.u a",
        "  store $or, t2",
        "  br t2, bb2, bb1",
        "bb1:",
        "  t3:i8 = load.u b",
        "  store $or, t3",
        "  jump bb2",
        "bb2:",
        "  t4:i8 = load.u $or",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 1",
        "  store x, t0",
        "  t1:i64 = load x",
        "  t2:i64 = const 2",
        "  t3:i8 = lt.s t1, t2",
        "  br t3, bb1, bb2",
        "bb1:",
        "  t4:i64 = const 1",
        "  print.i64 t4",
        "  jump bb2",
        "bb2:",
        "  t5:i64 = const 3",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 1",
        "  store x, t0",
        "  t1:i64 = load x",
        "  t2:i64 = const 2",
        "  t3:i8 = lt.s t1, t2",
        "  br t3, bb1, bb2",
        "bb1:",
        "  t4:i64 = const 1",
        "  print.i64 t4",
        "  jump bb3",
        "bb2:",
        "  t5:i64 = const 2",
        "  print.i64 t5",
        "  jump bb3",
        "bb3:",
        "  t6:i64 = const 3",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 1",
        "  store x, t0",
        "  t1:i64 = load x",
        "  t2:i64 = const 1",
        "  t3:i8 = lt.s t1, t2",
        "  br t3, bb1, bb2",
        "bb1:",
        "  t4:i64 = const 1",
        "  print.i64 t4",
        "  jump bb3",
        "bb2:",
        "  t5:i64 = load x",
        "  t6:i64 = const 2",
        "  t7:i8 = lt.s t5, t6",
        "  br t7, bb4, bb5",
        "bb3:",
        "  ret",
        "bb4:",
        "  t8:i64 = const 2",
        "  print.i64 t8",
        "  jump bb6",
        "bb5:",
        "  t9:i64 = const 3",
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
        "  t0:i64 = const 1",
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
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store x, t0",
        "  jump bb1",
        "bb1:",
        "  t1:i64 = load x",
        "  t2:i64 = const 1",
        "  t3:i64 = add.s t1, t2 @4:9",
        "  store x, t3",
        "  t4:i64 = load x",
        "  t5:i64 = const 3",
        "  t6:i8 = gt.s t4, t5",
        "  br t6, bb3, bb4",
        "bb2:",
        "  t11:i64 = const 9",
        "  print.i64 t11",
        "  ret",
        "bb3:",
        "  jump bb2",
        "bb4:",
        "  t7:i64 = load x",
        "  t8:i64 = const 2",
        "  t9:i8 = eq t7, t8",
        "  br t9, bb5, bb6",
        "bb5:",
        "  jump bb1",
        "bb6:",
        "  t10:i64 = load x",
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
        "  slots: i:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store i, t0",
        "  jump bb1",
        "bb1:",
        "  t1:i64 = load i",
        "  t2:i64 = const 3",
        "  t3:i8 = lt.s t1, t2",
        "  br t3, bb2, bb3",
        "bb2:",
        "  t4:i64 = load i",
        "  t5:i64 = const 1",
        "  t6:i64 = add.s t4, t5 @4:9",
        "  store i, t6",
        "  jump bb1",
        "bb3:",
        "  t7:i64 = load i",
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
        "  slots: i:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store i, t0",
        "  jump bb1",
        "bb1:",
        "  t1:i64 = load i",
        "  t2:i64 = const 5",
        "  t3:i8 = lt.s t1, t2",
        "  br t3, bb2, bb3",
        "bb2:",
        "  t4:i64 = load i",
        "  t5:i64 = const 1",
        "  t6:i64 = add.s t4, t5 @4:9",
        "  store i, t6",
        "  t7:i64 = load i",
        "  t8:i64 = const 2",
        "  t9:i8 = eq t7, t8",
        "  br t9, bb4, bb5",
        "bb3:",
        "  ret",
        "bb4:",
        "  jump bb1",
        "bb5:",
        "  t10:i64 = load i",
        "  t11:i64 = const 4",
        "  t12:i8 = eq t10, t11",
        "  br t12, bb6, bb7",
        "bb6:",
        "  jump bb3",
        "bb7:",
        "  t13:i64 = load i",
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
        "  slots: i:8, $end:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store i, t0",
        "  t1:i64 = const 3",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2:i64 = load i",
        "  t3:i64 = load $end",
        "  t4:i8 = lt.s t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5:i64 = load i",
        "  print.i64 t5",
        "  jump bb3",
        "bb3:",
        "  t6:i64 = load i",
        "  t7:i64 = const 1",
        "  t8:i64 = add.s t6, t7 @2:5",
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
        "  slots: i:8, $end:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store i, t0",
        "  t1:i64 = const 5",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2:i64 = load i",
        "  t3:i64 = load $end",
        "  t4:i8 = lt.s t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5:i64 = load i",
        "  t6:i64 = const 3",
        "  t7:i8 = eq t5, t6",
        "  br t7, bb5, bb6",
        "bb3:",
        "  t12:i64 = load i",
        "  t13:i64 = const 1",
        "  t14:i64 = add.s t12, t13 @2:5",
        "  store i, t14",
        "  jump bb1",
        "bb4:",
        "  ret",
        "bb5:",
        "  jump bb4",
        "bb6:",
        "  t8:i64 = load i",
        "  t9:i64 = const 1",
        "  t10:i8 = eq t8, t9",
        "  br t10, bb7, bb8",
        "bb7:",
        "  jump bb3",
        "bb8:",
        "  t11:i64 = load i",
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
        "  slots: i:8, $end:8",
        "bb0:",
        "  t0:i64 = const 1",
        "  store i, t0",
        "  t1:i64 = const 3",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2:i64 = load i",
        "  t3:i64 = load $end",
        "  t4:i8 = le.s t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5:i64 = load i",
        "  print.i64 t5",
        "  jump bb3",
        "bb3:",
        "  t6:i64 = load i",
        "  t7:i64 = load $end",
        "  t8:i8 = eq t6, t7",
        "  br t8, bb4, bb5",
        "bb4:",
        "  ret",
        "bb5:",
        "  t9:i64 = load i",
        "  t10:i64 = const 1",
        "  t11:i64 = add.s t9, t10 @2:5",
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
        "  slots: i:8, $end:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store i, t0",
        "  t1:i64 = const 2",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2:i64 = load i",
        "  t3:i64 = load $end",
        "  t4:i8 = le.s t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  jump bb3",
        "bb3:",
        "  t5:i64 = load i",
        "  t6:i64 = load $end",
        "  t7:i8 = eq t5, t6",
        "  br t7, bb4, bb5",
        "bb4:",
        "  ret",
        "bb5:",
        "  t8:i64 = load i",
        "  t9:i64 = const 1",
        "  t10:i64 = add.s t8, t9 @2:5",
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
        "  slots: i:8, $end:8, j:8, $end#3:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  store i, t0",
        "  t1:i64 = const 2",
        "  store $end, t1",
        "  jump bb1",
        "bb1:",
        "  t2:i64 = load i",
        "  t3:i64 = load $end",
        "  t4:i8 = lt.s t2, t3",
        "  br t4, bb2, bb4",
        "bb2:",
        "  t5:i64 = const 0",
        "  store j, t5",
        "  t6:i64 = const 2",
        "  store $end#3, t6",
        "  jump bb5",
        "bb3:",
        "  t14:i64 = load i",
        "  t15:i64 = const 1",
        "  t16:i64 = add.s t14, t15 @2:5",
        "  store i, t16",
        "  jump bb1",
        "bb4:",
        "  ret",
        "bb5:",
        "  t7:i64 = load j",
        "  t8:i64 = load $end#3",
        "  t9:i8 = lt.s t7, t8",
        "  br t9, bb6, bb8",
        "bb6:",
        "  t10:i64 = load j",
        "  print.i64 t10",
        "  jump bb7",
        "bb7:",
        "  t11:i64 = load j",
        "  t12:i64 = const 1",
        "  t13:i64 = add.s t11, t12 @3:9",
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
        "fn g(c:i8.u) -> i64 {",
        "  slots: c:1",
        "bb0:",
        "  t0:i8 = load.u c",
        "  br t0, bb1, bb2",
        "bb1:",
        "  t1:i64 = const 1",
        "  ret t1",
        "bb2:",
        "  t3:i64 = const 3",
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
        "  slots: c:1",
        "bb0:",
        "  t0:i8 = const 1",
        "  store c, t0",
        "  t1:i8 = load.u c",
        "  br t1, bb1, bb2",
        "bb1:",
        "  jump bb3",
        "bb2:",
        "  t2:i8 = load.u c",
        "  br t2, bb4, bb5",
        "bb3:",
        "  jump bb3",
        "bb4:",
        "  t3:i64 = const 1",
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
        "  t0:i64 = const 1",
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
        "fn g(c:i8.u) -> i64 {",
        "  slots: c:1",
        "bb0:",
        "  t0:i8 = load.u c",
        "  br t0, bb1, bb2",
        "bb1:",
        "  t1:i64 = const 1",
        "  ret t1",
        "bb2:",
        "  t2:i64 = const 2",
        "  ret t2",
        "}",
    ]);
    assert_eq!(function_ir(&source, "g"), expected);
    assert!(!ir_text(&source).contains("unreachable"));
}

/// The spec's example: a `bool` parameter is `i8.u`, its slot is 1 byte, its load is `load.u`, and the result is `i8`.
#[test]
fn bool_parameter_and_result_print_as_i8() {
    let source = text(&[
        "fn f(a: i64, flag: bool) -> bool {",
        "    return flag",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn f(a:i64, flag:i8.u) -> i8 {",
        "  slots: a:8, flag:1",
        "bb0:",
        "  t0:i8 = load.u flag",
        "  ret t0",
        "}",
    ]);
    assert_eq!(function_ir(&source, "f"), expected);
}

/// A call with a `bool` argument passes an `i8` temp, and the callee's `bool` result is an `i8` temp.
#[test]
fn bool_call_argument_and_result_are_i8() {
    let source = text(&[
        "fn id(b: bool) -> bool {",
        "    return b",
        "}",
        "fn main() {",
        "    print(id(true))",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0:i8 = const 1",
        "  t1:i8 = call id(t0)",
        "  print.bool t1",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// `a && b` keeps its value in a 1-byte `$and` slot, and the loads of `$and` are unsigned.
#[test]
fn and_slot_is_one_byte_and_loaded_unsigned() {
    let output = main_ir(&[
        "fn main() {",
        "    a := true",
        "    b := false",
        "    print(a && b)",
        "}",
    ]);
    assert!(output.contains("  slots: a:1, b:1, $and:1\n"), "{output}");
    assert!(output.contains("  t4:i8 = load.u $and\n"), "{output}");
}

/// `a || b` keeps its value in a 1-byte `$or` slot, and the loads of `$or` are unsigned.
#[test]
fn or_slot_is_one_byte_and_loaded_unsigned() {
    let output = main_ir(&[
        "fn main() {",
        "    a := true",
        "    b := false",
        "    print(a || b)",
        "}",
    ]);
    assert!(output.contains("  slots: a:1, b:1, $or:1\n"), "{output}");
    assert!(output.contains("  t4:i8 = load.u $or\n"), "{output}");
}

/// A range loop's `$end` slot is 8 bytes.
#[test]
fn range_loop_end_slot_is_eight_bytes() {
    let output = main_ir(&["fn main() {", "    for i in 0..3 {", "    }", "}"]);
    assert!(output.contains("  slots: i:8, $end:8\n"), "{output}");
}

/// A range loop's step is `add.s` of an `i64` and a `t…:i64 = const 1`.
#[test]
fn range_loop_step_is_a_signed_add_of_a_typed_one() {
    let output = main_ir(&["fn main() {", "    for i in 0..3 {", "    }", "}"]);
    let lines: Vec<&str> = output.lines().collect();
    let add = lines
        .iter()
        .position(|line| line.contains("= add.s "))
        .unwrap();
    assert!(lines[add].contains(":i64 = add.s "), "{}", lines[add]);
    assert!(
        lines[add - 1].ends_with(":i64 = const 1"),
        "{}",
        lines[add - 1]
    );
}

/// Every ordering comparison prints `.s`; `eq` and `ne` print no suffix.
#[test]
fn comparisons_print_their_sign_but_equality_does_not() {
    let output = main_ir(&[
        "fn main() {",
        "    a := 1",
        "    print(a < a)",
        "    print(a <= a)",
        "    print(a > a)",
        "    print(a >= a)",
        "    print(a == a)",
        "    print(a != a)",
        "}",
    ]);
    for mnemonic in ["lt.s", "le.s", "gt.s", "ge.s"] {
        assert!(output.contains(&format!(":i8 = {mnemonic} ")), "{mnemonic}");
    }
    for mnemonic in ["eq", "ne"] {
        assert!(
            output.contains(&format!(":i8 = {mnemonic} t")),
            "{mnemonic}"
        );
        assert!(!output.contains(&format!("{mnemonic}.")), "{mnemonic}");
    }
}

/// Comparing two `bool`s loads them unsigned and compares with `eq`, giving an `i8`.
#[test]
fn bool_equality_loads_unsigned() {
    let output = main_ir(&[
        "fn main() {",
        "    a := true",
        "    b := false",
        "    print(a == b)",
        "}",
    ]);
    assert!(output.contains("  t2:i8 = load.u a\n"), "{output}");
    assert!(output.contains("  t3:i8 = load.u b\n"), "{output}");
    assert!(output.contains("  t4:i8 = eq t2, t3\n"), "{output}");
}

/// Arithmetic on `i64` is signed: every checked operation prints `.s`.
#[test]
fn arithmetic_prints_the_signed_suffix() {
    let output = main_ir(&[
        "fn main() {",
        "    a := 7",
        "    print(a + a)",
        "    print(a - a)",
        "    print(a * a)",
        "    print(a / a)",
        "    print(a % a)",
        "    print(-a)",
        "}",
    ]);
    for mnemonic in ["add.s", "sub.s", "mul.s", "div.s", "rem.s"] {
        assert!(
            output.contains(&format!(":i64 = {mnemonic} t")),
            "{mnemonic}"
        );
    }
    assert!(!output.contains(".u "), "{output}");
}

/// The IR text of a program has no `?` type: every temp has one.
#[test]
fn every_lowered_temp_has_a_type() {
    let output = ir_text(&text(&[
        "fn fib(n: i64) -> i64 {",
        "    if n < 2 && n > 0 || n == 5 {",
        "        return n",
        "    }",
        "    return fib(n - 1) + fib(n - 2)",
        "}",
        "fn main() {",
        "    for i in 0..=3 {",
        "        print(fib(i))",
        "    }",
        "}",
    ]));
    assert!(!output.contains(":?"), "{output}");
}

/// Printing the same program twice gives the same text.
#[test]
fn typed_ir_text_is_deterministic() {
    let source = text(&[
        "fn f(a: i64, flag: bool) -> bool {",
        "    return flag && a < 3",
        "}",
        "fn main() {",
        "    print(f(1, true))",
        "}",
    ]);
    assert_eq!(ir_text(&source), ir_text(&source));
}

// ---------------------------------------------------------------- M2.2c: the eight integer types

/// How lowering shows one source integer type.
struct IntCase {
    /// The source spelling, such as `u8`.
    name: &'static str,
    /// The IR type: signless, so `u8` and `i8` are both `i8`.
    ir: &'static str,
    /// The slot size in bytes.
    size: u32,
    /// The extension suffix of a load or a parameter: `.s` or `.u` for 8 and 16 bits, nothing above.
    extension: &'static str,
    /// The sign suffix of an operation: `s` or `u`.
    sign: &'static str,
    /// The print instruction for the type, such as `print.u64`.
    print: &'static str,
}

/// All eight integer types, with how lowering shows each.
const INT_CASES: [IntCase; 8] = [
    IntCase {
        name: "i8",
        ir: "i8",
        size: 1,
        extension: ".s",
        sign: "s",
        print: "print.i64",
    },
    IntCase {
        name: "i16",
        ir: "i16",
        size: 2,
        extension: ".s",
        sign: "s",
        print: "print.i64",
    },
    IntCase {
        name: "i32",
        ir: "i32",
        size: 4,
        extension: "",
        sign: "s",
        print: "print.i64",
    },
    IntCase {
        name: "i64",
        ir: "i64",
        size: 8,
        extension: "",
        sign: "s",
        print: "print.i64",
    },
    IntCase {
        name: "u8",
        ir: "i8",
        size: 1,
        extension: ".u",
        sign: "u",
        print: "print.u64",
    },
    IntCase {
        name: "u16",
        ir: "i16",
        size: 2,
        extension: ".u",
        sign: "u",
        print: "print.u64",
    },
    IntCase {
        name: "u32",
        ir: "i32",
        size: 4,
        extension: "",
        sign: "u",
        print: "print.u64",
    },
    IntCase {
        name: "u64",
        ir: "i64",
        size: 8,
        extension: "",
        sign: "u",
        print: "print.u64",
    },
];

/// Checks `fn f(a: T, b: T) -> R { return a OP b }` for one type, operator and result type.
///
/// `mnemonic` includes the sign suffix and `suffix` is the expected location text.
fn assert_typed_operator(
    case: &IntCase,
    symbol: &str,
    mnemonic: &str,
    result: (&str, &str),
    suffix: &str,
) {
    let (result_name, result_ir) = result;
    let header = format!("fn f(a: {0}, b: {0}) -> {result_name} {{", case.name);
    let statement = format!("    return a {symbol} b");
    let source = text(&[&header, &statement, "}", "fn main() {", "}"]);

    let (ir, extension, size) = (case.ir, case.extension, case.size);
    let expected = text(&[
        &format!("fn f(a:{ir}{extension}, b:{ir}{extension}) -> {result_ir} {{"),
        &format!("  slots: a:{size}, b:{size}"),
        "bb0:",
        &format!("  t0:{ir} = load{extension} a"),
        &format!("  t1:{ir} = load{extension} b"),
        &format!("  t2:{result_ir} = {mnemonic} t0, t1{suffix}"),
        "  ret t2",
        "}",
    ]);
    assert_eq!(
        function_ir(&source, "f"),
        expected,
        "{} {symbol}",
        case.name
    );
}

/// Checks `x OP= 3` after `x: u8 = 10`; the operation sits at the statement start.
fn assert_narrow_compound_assign(symbol: &str, mnemonic: &str) {
    let statement = format!("    x {symbol} 3");
    let actual = main_ir(&[
        "fn main() {",
        "    x: u8 = 10",
        &statement,
        "    print(x)",
        "}",
    ]);
    let operation = format!("  t3:i8 = {mnemonic} t1, t2 @3:5");
    let expected = text(&[
        "fn main() {",
        "  slots: x:1",
        "bb0:",
        "  t0:i8 = const 10",
        "  store x, t0",
        "  t1:i8 = load.u x",
        "  t2:i8 = const 3",
        &operation,
        "  store x, t3",
        "  t4:i8 = load.u x",
        "  print.u64 t4",
        "  ret",
        "}",
    ]);
    assert_eq!(actual, expected, "operator {symbol}");
}

/// The spec's `fn f(n: u8) -> u8` example prints exactly: unsigned load and unsigned add.
#[test]
fn unsigned_byte_function_prints_in_full() {
    let source = text(&[
        "fn f(n: u8) -> u8 {",
        "    return n + 1",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn f(n:i8.u) -> i8 {",
        "  slots: n:1",
        "bb0:",
        "  t0:i8 = load.u n",
        "  t1:i8 = const 1",
        "  t2:i8 = add.u t0, t1 @2:12",
        "  ret t2",
        "}",
    ]);
    assert_eq!(function_ir(&source, "f"), expected);
}

/// A local of each type has a slot of its size, a typed const and a load with its own extension.
#[test]
fn local_of_each_type_has_its_size_const_and_load() {
    for case in &INT_CASES {
        let statement = format!("    x: {} = 1", case.name);
        let actual = main_ir(&["fn main() {", &statement, "    print(x)", "}"]);
        let (ir, extension) = (case.ir, case.extension);
        let expected = text(&[
            "fn main() {",
            &format!("  slots: x:{}", case.size),
            "bb0:",
            &format!("  t0:{ir} = const 1"),
            "  store x, t0",
            &format!("  t1:{ir} = load{extension} x"),
            &format!("  {} t1", case.print),
            "  ret",
            "}",
        ]);
        assert_eq!(actual, expected, "{}", case.name);
    }
}

/// `print` of an unsigned type lowers to `print.u64` and of a signed type to `print.i64`.
#[test]
fn print_kind_follows_the_signedness_of_the_type() {
    for case in &INT_CASES {
        let statement = format!("    x: {} = 1", case.name);
        let output = main_ir(&["fn main() {", &statement, "    print(x)", "}"]);
        let line = format!("  {} t1\n", case.print);
        assert!(output.contains(&line), "{}: {output}", case.name);
    }
}

/// `print` of an `i16` is `print.i64` and of a `u32` is `print.u64`, as the spec lists.
#[test]
fn print_of_i16_and_u32_use_the_spec_kinds() {
    let signed = main_ir(&["fn main() {", "    x: i16 = 5", "    print(x)", "}"]);
    assert!(signed.contains("  print.i64 t1\n"), "{signed}");
    let unsigned = main_ir(&["fn main() {", "    x: u32 = 5", "    print(x)", "}"]);
    assert!(unsigned.contains("  print.u64 t1\n"), "{unsigned}");
}

/// `print` of a `bool` stays `print.bool`.
#[test]
fn print_of_a_bool_variable_stays_print_bool() {
    let output = main_ir(&["fn main() {", "    x: bool = true", "    print(x)", "}"]);
    assert!(output.contains("  print.bool t1\n"), "{output}");
}

/// `x: u64 = 18446744073709551615` is one const holding the bit pattern, printed as `-1`.
#[test]
fn u64_max_literal_is_a_const_of_minus_one() {
    let expected = text(&[
        "fn main() {",
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const -1",
        "  store x, t0",
        "  t1:i64 = load x",
        "  print.u64 t1",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&[
        "fn main() {",
        "    x: u64 = 18446744073709551615",
        "    print(x)",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// A `u64` literal just above `i64::MAX` is a const holding `i64::MIN`'s bit pattern.
#[test]
fn u64_literal_above_i64_max_is_a_const_of_i64_min() {
    let output = main_ir(&[
        "fn main() {",
        "    x: u64 = 9223372036854775808",
        "    print(x)",
        "}",
    ]);
    assert!(
        output.contains("  t0:i64 = const -9223372036854775808\n"),
        "{output}"
    );
}

/// The largest value of a narrower unsigned type is a const of its own value, not sign-extended.
#[test]
fn narrow_unsigned_max_literals_are_consts_of_their_value() {
    let table = [
        ("u8", "i8", "255"),
        ("u16", "i16", "65535"),
        ("u32", "i32", "4294967295"),
    ];
    for (name, ir, max) in table {
        let statement = format!("    x: {name} = {max}");
        let output = main_ir(&["fn main() {", &statement, "    print(x)", "}"]);
        let line = format!("  t0:{ir} = const {max}\n");
        assert!(output.contains(&line), "{name}: {output}");
    }
}

/// `x: i8 = -128` is one `const -128` and no `neg`.
#[test]
fn negated_i8_min_literal_is_one_const() {
    let expected = text(&[
        "fn main() {",
        "  slots: x:1",
        "bb0:",
        "  t0:i8 = const -128",
        "  store x, t0",
        "  t1:i8 = load.s x",
        "  print.i64 t1",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&["fn main() {", "    x: i8 = -128", "    print(x)", "}"]);
    assert_eq!(actual, expected);
}

/// `x: i8 = -5` is `const 5` followed by `neg` at the minus sign.
#[test]
fn negated_ordinary_i8_literal_is_const_then_neg() {
    let expected = text(&[
        "fn main() {",
        "  slots: x:1",
        "bb0:",
        "  t0:i8 = const 5",
        "  t1:i8 = neg t0 @2:13",
        "  store x, t1",
        "  t2:i8 = load.s x",
        "  print.i64 t2",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&["fn main() {", "    x: i8 = -5", "    print(x)", "}"]);
    assert_eq!(actual, expected);
}

/// The smallest value of every signed type is a single const, with no `neg`.
#[test]
fn signed_min_literals_are_single_consts() {
    let table = [
        ("i8", "-128"),
        ("i16", "-32768"),
        ("i32", "-2147483648"),
        ("i64", "-9223372036854775808"),
    ];
    for (name, min) in table {
        let statement = format!("    x: {name} = {min}");
        let output = main_ir(&["fn main() {", &statement, "    print(x)", "}"]);
        let line = format!(":{name} = const {min}\n");
        assert!(output.contains(&line), "{name}: {output}");
        assert!(!output.contains(" neg "), "{name}: {output}");
    }
}

/// An untyped sum settled to `i16` lowers every node as `i16`.
#[test]
fn untyped_sum_lowers_in_the_settled_type() {
    let expected = text(&[
        "fn main() {",
        "  slots: x:2",
        "bb0:",
        "  t0:i16 = const 7",
        "  t1:i16 = const 6",
        "  t2:i16 = mul.s t0, t1 @2:14",
        "  store x, t2",
        "  t3:i16 = load.s x",
        "  print.i64 t3",
        "  ret",
        "}",
    ]);
    let actual = main_ir(&["fn main() {", "    x: i16 = 7 * 6", "    print(x)", "}"]);
    assert_eq!(actual, expected);
}

/// A literal returned from a `u8` function is an `i8` const.
#[test]
fn returned_literal_is_a_const_of_the_return_type() {
    let source = text(&["fn f() -> u8 {", "    return 200", "}", "fn main() {", "}"]);
    let expected = text(&[
        "fn f() -> i8 {",
        "  slots:",
        "bb0:",
        "  t0:i8 = const 200",
        "  ret t0",
        "}",
    ]);
    assert_eq!(function_ir(&source, "f"), expected);
}

/// `+ - * / %` take `.s` for signed types and `.u` for unsigned types, with their location.
#[test]
fn arithmetic_takes_its_sign_from_the_operand_type() {
    for case in &INT_CASES {
        for (symbol, name) in [
            ("+", "add"),
            ("-", "sub"),
            ("*", "mul"),
            ("/", "div"),
            ("%", "rem"),
        ] {
            let mnemonic = format!("{name}.{}", case.sign);
            let result = (case.name, case.ir);
            assert_typed_operator(case, symbol, &mnemonic, result, " @2:12");
        }
    }
}

/// `< <= > >=` take `.s` or `.u`, give an `i8` and carry no location.
#[test]
fn ordering_takes_its_sign_from_the_operand_type() {
    for case in &INT_CASES {
        for (symbol, name) in [("<", "lt"), ("<=", "le"), (">", "gt"), (">=", "ge")] {
            let mnemonic = format!("{name}.{}", case.sign);
            assert_typed_operator(case, symbol, &mnemonic, ("bool", "i8"), "");
        }
    }
}

/// `==` and `!=` print no sign for any type, and give an `i8`.
#[test]
fn equality_prints_no_sign_for_any_type() {
    for case in &INT_CASES {
        for (symbol, mnemonic) in [("==", "eq"), ("!=", "ne")] {
            assert_typed_operator(case, symbol, mnemonic, ("bool", "i8"), "");
        }
    }
}

/// Unary minus of a signed narrow value loads with extension and negates at the minus sign.
#[test]
fn negating_a_signed_value_prints_neg_in_its_type() {
    for case in INT_CASES.iter().filter(|case| case.sign == "s") {
        let header = format!("fn f(a: {0}) -> {0} {{", case.name);
        let source = text(&[&header, "    return -a", "}", "fn main() {", "}"]);
        let (ir, extension, size) = (case.ir, case.extension, case.size);
        let expected = text(&[
            &format!("fn f(a:{ir}{extension}) -> {ir} {{"),
            &format!("  slots: a:{size}"),
            "bb0:",
            &format!("  t0:{ir} = load{extension} a"),
            &format!("  t1:{ir} = neg t0 @2:12"),
            "  ret t1",
            "}",
        ]);
        assert_eq!(function_ir(&source, "f"), expected, "{}", case.name);
    }
}

/// Compound assignment on a `u8` loads with extension and uses the unsigned operation.
#[test]
fn compound_assignment_on_a_byte_is_unsigned() {
    let table = [
        ("+=", "add.u"),
        ("-=", "sub.u"),
        ("*=", "mul.u"),
        ("/=", "div.u"),
    ];
    for (symbol, mnemonic) in table {
        assert_narrow_compound_assign(symbol, mnemonic);
    }
}

/// A function with parameters of every width prints each parameter with its type, extension and slot size.
#[test]
fn parameters_of_mixed_widths_print_their_types_and_sizes() {
    let source = text(&[
        "fn mix(a: i8, b: u16, c: i32, d: u64) -> i64 {",
        "    return 0",
        "}",
        "fn main() {",
        "}",
    ]);
    let expected = text(&[
        "fn mix(a:i8.s, b:i16.u, c:i32, d:i64) -> i64 {",
        "  slots: a:1, b:2, c:4, d:8",
        "bb0:",
        "  t0:i64 = const 0",
        "  ret t0",
        "}",
    ]);
    assert_eq!(function_ir(&source, "mix"), expected);
}

/// A call passes typed temps and its result has the callee's return type.
#[test]
fn call_with_narrow_argument_and_wide_result_is_typed() {
    let source = text(&[
        "fn widen(a: u8) -> u32 {",
        "    return 1",
        "}",
        "fn main() {",
        "    print(widen(5))",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0:i8 = const 5",
        "  t1:i32 = call widen(t0)",
        "  print.u64 t1",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// The source of a `u8` inclusive range loop over `n`.
const U8_INCLUSIVE_LOOP: [&str; 6] = [
    "fn main() {",
    "    n: u8 = 255",
    "    for i in 0..=n {",
    "        print(i)",
    "    }",
    "}",
];

/// `for i in 0..=n` with `n: u8` prints in full: byte slots, unsigned loads, `le.u`, `eq`, `add.u`.
#[test]
fn inclusive_byte_range_loop_prints_in_full() {
    let expected = text(&[
        "fn main() {",
        "  slots: n:1, i:1, $end:1",
        "bb0:",
        "  t0:i8 = const 255",
        "  store n, t0",
        "  t1:i8 = const 0",
        "  store i, t1",
        "  t2:i8 = load.u n",
        "  store $end, t2",
        "  jump bb1",
        "bb1:",
        "  t3:i8 = load.u i",
        "  t4:i8 = load.u $end",
        "  t5:i8 = le.u t3, t4",
        "  br t5, bb2, bb4",
        "bb2:",
        "  t6:i8 = load.u i",
        "  print.u64 t6",
        "  jump bb3",
        "bb3:",
        "  t7:i8 = load.u i",
        "  t8:i8 = load.u $end",
        "  t9:i8 = eq t7, t8",
        "  br t9, bb4, bb5",
        "bb4:",
        "  ret",
        "bb5:",
        "  t10:i8 = load.u i",
        "  t11:i8 = const 1",
        "  t12:i8 = add.u t10, t11 @3:5",
        "  store i, t12",
        "  jump bb1",
        "}",
    ]);
    assert_eq!(main_ir(&U8_INCLUSIVE_LOOP), expected);
}

/// The `$end` slot of a `u8` range loop is one byte, like the loop variable.
#[test]
fn byte_range_loop_end_slot_is_one_byte() {
    let output = main_ir(&U8_INCLUSIVE_LOOP);
    assert!(output.contains("  slots: n:1, i:1, $end:1\n"), "{output}");
}

/// The step of a `u8` range loop adds an `i8` const 1 with `add.u`.
#[test]
fn byte_range_loop_step_is_an_unsigned_add_of_a_byte_one() {
    let output = main_ir(&U8_INCLUSIVE_LOOP);
    let lines: Vec<&str> = output.lines().collect();
    let add = lines
        .iter()
        .position(|line| line.contains("= add.u "))
        .unwrap();
    assert!(lines[add].contains(":i8 = add.u "), "{}", lines[add]);
    assert!(
        lines[add - 1].ends_with(":i8 = const 1"),
        "{}",
        lines[add - 1]
    );
}

/// An inclusive `u8` range loop compares with `le.u` and tests `eq` before stepping.
#[test]
fn byte_range_loop_compares_unsigned_and_tests_equality_first() {
    let output = main_ir(&U8_INCLUSIVE_LOOP);
    assert!(output.contains(":i8 = le.u "), "{output}");
    assert!(output.contains(":i8 = eq t"), "{output}");
    assert!(!output.contains("le.s"), "{output}");
}

/// An exclusive `i16` range loop compares with `lt.s` and steps with `add.s`, in two-byte slots.
#[test]
fn signed_short_range_loop_uses_signed_operations() {
    let output = main_ir(&[
        "fn main() {",
        "    a: i16 = -3",
        "    b: i16 = 3",
        "    for i in a..b {",
        "        print(i)",
        "    }",
        "}",
    ]);
    assert!(
        output.contains("  slots: a:2, b:2, i:2, $end:2\n"),
        "{output}"
    );
    assert!(output.contains(":i8 = lt.s "), "{output}");
    assert!(output.contains(":i16 = add.s "), "{output}");
    assert!(output.contains(":i16 = load.s i\n"), "{output}");
}

/// An exclusive `u32` range loop compares with `lt.u` and steps with `add.u`, in four-byte slots.
#[test]
fn unsigned_word_range_loop_uses_unsigned_operations() {
    let output = main_ir(&[
        "fn main() {",
        "    n: u32 = 3",
        "    for i in 0..n {",
        "        print(i)",
        "    }",
        "}",
    ]);
    assert!(output.contains("  slots: n:4, i:4, $end:4\n"), "{output}");
    assert!(output.contains(":i8 = lt.u "), "{output}");
    assert!(output.contains(":i32 = add.u "), "{output}");
}

/// No lowered temp of an integer program lacks a type.
#[test]
fn every_temp_of_an_integer_program_has_a_type() {
    let output = ir_text(&text(&[
        "fn f(a: u8, b: i16) -> u32 {",
        "    return 7",
        "}",
        "fn main() {",
        "    n: u16 = 4",
        "    for i in 0..=n {",
        "        print(f(1, -2) + 3)",
        "    }",
        "}",
    ]));
    assert!(!output.contains(":?"), "{output}");
}
