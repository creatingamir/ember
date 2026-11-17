//! Integration tests for the public API of the `parser` crate.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use ast::{
    Block, ElseBranch, Expr, ExprKind, ForKind, Function, NodeId, Program, Stmt, StmtKind,
    print_program,
};
use parser::{parse, parse_source};
use source::{Diagnostic, Span};

// ---------------------------------------------------------------- helpers

/// The text that comes before the expression in a source built by `expr_source`.
const EXPR_PREFIX: &str = "fn main() {\n    x := ";

/// Parses source that must be valid.
fn program(src: &str) -> Program {
    parse_source(src).unwrap()
}

/// Parses source that must be valid and prints its AST.
fn printed(src: &str) -> String {
    print_program(&program(src))
}

/// Joins expected output lines, ending each with a newline.
fn lines(expected: &[&str]) -> String {
    let mut text = String::new();
    for line in expected {
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Wraps statements in `fn main() { ... }`, one statement group per line.
fn main_source(stmts: &str) -> String {
    format!("fn main() {{\n{stmts}\n}}\n")
}

/// Parses statements inside `fn main` and prints the whole program.
fn printed_main(stmts: &str) -> String {
    printed(&main_source(stmts))
}

/// Wraps an expression as the value of `x := ...` inside `fn main`.
fn expr_source(expr: &str) -> String {
    format!("{EXPR_PREFIX}{expr}\n}}\n")
}

/// Parses an expression and returns it printed in prefix form.
fn printed_expr(expr: &str) -> String {
    let out = printed(&expr_source(expr));
    let rest = out.strip_prefix("fn main()\n  let x = ").unwrap();
    rest.strip_suffix('\n').unwrap().to_string()
}

/// Returns the first statement of the first function of valid source.
fn first_stmt(src: &str) -> Stmt {
    program(src).functions.remove(0).body.stmts.remove(0)
}

/// Parses an expression and returns its AST node.
fn parsed_expr(expr: &str) -> Expr {
    let stmt = first_stmt(&expr_source(expr));
    let value = match stmt.kind {
        StmtKind::Let { value, .. } => Some(value),
        _ => None,
    };
    value.unwrap()
}

/// The span of bytes `start..end` of an expression inside `expr_source`.
fn expr_span(start: usize, end: usize) -> Span {
    let offset = EXPR_PREFIX.len();
    Span::new(offset + start, offset + end)
}

/// The span of the `n`th (0-based) occurrence of `needle` in `src`.
fn nth_span(src: &str, needle: &str, n: usize) -> Span {
    let (start, _) = src.match_indices(needle).nth(n).unwrap();
    Span::new(start, start + needle.len())
}

/// The span of the first occurrence of `needle` in `src`.
fn span_of(src: &str, needle: &str) -> Span {
    nth_span(src, needle, 0)
}

/// Parses source that must be invalid and returns its error.
fn error_of(src: &str) -> Diagnostic {
    parse_source(src).unwrap_err()
}

/// Asserts that parsing `src` fails with exactly this message and span.
fn assert_error(src: &str, message: &str, span: Span) {
    assert_eq!(error_of(src), Diagnostic::error(message, span));
}

/// Asserts that an invalid expression fails with this message at bytes `start..end` of the expression.
fn assert_expr_error(expr: &str, message: &str, start: usize, end: usize) {
    assert_error(&expr_source(expr), message, expr_span(start, end));
}

/// Splits a binary expression into its left and right operands.
fn binary_operands(expr: Expr) -> (Expr, Expr) {
    let operands = match expr.kind {
        ExprKind::Binary { left, right, .. } => Some((*left, *right)),
        _ => None,
    };
    operands.unwrap()
}

/// Returns the operand of a unary expression.
fn unary_operand(expr: Expr) -> Expr {
    let operand = match expr.kind {
        ExprKind::Unary { operand, .. } => Some(*operand),
        _ => None,
    };
    operand.unwrap()
}

/// Returns the then-block and else branch of an `if` statement.
fn if_parts(stmt: Stmt) -> (Block, Option<ElseBranch>) {
    let parts = match stmt.kind {
        StmtKind::If {
            then_block,
            else_branch,
            ..
        } => Some((then_block, else_branch)),
        _ => None,
    };
    parts.unwrap()
}

/// Returns the first statement inside the body of the first `for` of valid source.
fn first_for_body_stmt(src: &str) -> Stmt {
    let body = match first_stmt(src).kind {
        StmtKind::For { body, .. } => Some(body),
        _ => None,
    };
    body.unwrap().stmts.remove(0)
}

/// Returns the first function of valid source.
fn first_function(src: &str) -> Function {
    program(src).functions.remove(0)
}

// ---------------------------------------------------------------- node id walker

/// Collects the ids of every node of a program.
fn all_ids(program: &Program) -> Vec<NodeId> {
    let mut ids = Vec::new();
    for function in &program.functions {
        function_ids(function, &mut ids);
    }
    ids
}

/// Collects the ids of a function, its parameters, types and body.
fn function_ids(function: &Function, ids: &mut Vec<NodeId>) {
    ids.push(function.id);
    for param in &function.params {
        ids.push(param.id);
        ids.push(param.ty.id);
    }
    if let Some(return_type) = &function.return_type {
        ids.push(return_type.id);
    }
    block_ids(&function.body, ids);
}

/// Collects the ids of a block and all its statements.
fn block_ids(block: &Block, ids: &mut Vec<NodeId>) {
    ids.push(block.id);
    for stmt in &block.stmts {
        stmt_ids(stmt, ids);
    }
}

/// Collects the ids of a statement and everything inside it.
fn stmt_ids(stmt: &Stmt, ids: &mut Vec<NodeId>) {
    ids.push(stmt.id);
    match &stmt.kind {
        StmtKind::Let { ty, value, .. } => {
            if let Some(ty) = ty {
                ids.push(ty.id);
            }
            expr_ids(value, ids);
        }
        StmtKind::Assign { value, .. } => expr_ids(value, ids),
        StmtKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            expr_ids(cond, ids);
            block_ids(then_block, ids);
            match else_branch {
                Some(ElseBranch::If(nested)) => stmt_ids(nested, ids),
                Some(ElseBranch::Block(block)) => block_ids(block, ids),
                None => {}
            }
        }
        StmtKind::For { kind, body } => {
            match kind {
                ForKind::Infinite => {}
                ForKind::While(cond) => expr_ids(cond, ids),
                ForKind::In { iterable, .. } => expr_ids(iterable, ids),
            }
            block_ids(body, ids);
        }
        StmtKind::Return(Some(value)) => expr_ids(value, ids),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
        StmtKind::Expr(expr) => expr_ids(expr, ids),
    }
}

/// Collects the ids of an expression and all its sub-expressions.
fn expr_ids(expr: &Expr, ids: &mut Vec<NodeId>) {
    ids.push(expr.id);
    match &expr.kind {
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
        ExprKind::Unary { operand, .. } => expr_ids(operand, ids),
        ExprKind::Binary { left, right, .. } => {
            expr_ids(left, ids);
            expr_ids(right, ids);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                expr_ids(arg, ids);
            }
        }
    }
}

/// Asserts that no node id appears twice in the program parsed from `src`.
fn assert_ids_unique(src: &str) {
    let mut ids = all_ids(&program(src));
    let count = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate NodeId in {src:?}");
}

/// A program that uses every statement and expression form and two functions.
const EVERYTHING: &str = concat!(
    "fn helper(a: i64, b: i64) -> i64 = a + b * 2\n",
    "fn main() {\n",
    "    x := helper(1, 2)\n",
    "    y: i64 = -x\n",
    "    if x < y {\n",
    "        x += 1\n",
    "    } else if x == y {\n",
    "        y -= 1\n",
    "    } else {\n",
    "        return\n",
    "    }\n",
    "    for i in 0..10 {\n",
    "        continue\n",
    "    }\n",
    "    for x < 100 {\n",
    "        x = x * 2\n",
    "    }\n",
    "    for {\n",
    "        break\n",
    "    }\n",
    "    helper(x, y)\n",
    "}\n",
);

// ---------------------------------------------------------------- the spec's example

/// The spec's fib example prints exactly as documented.
#[test]
fn fib_example() {
    let src = concat!(
        "fn fib(n: i64) -> i64 {\n",
        "    if n < 2 {\n",
        "        return n\n",
        "    }\n",
        "    return fib(n - 1) + fib(n - 2)\n",
        "}\n",
    );
    let expected = lines(&[
        "fn fib(n: i64) -> i64",
        "  if (< n 2)",
        "    return n",
        "  return (+ (call fib (- n 1)) (call fib (- n 2)))",
    ]);
    assert_eq!(printed(src), expected);
}

/// `parse` on the lexer's tokens gives the same program as `parse_source`.
#[test]
fn parse_of_lexed_tokens_equals_parse_source() {
    let src = "fn main() {\n    x := 1 + 2\n}\n";
    let tokens = lexer::lex(src).unwrap();
    assert_eq!(parse(&tokens).unwrap(), program(src));
}

// ---------------------------------------------------------------- program and functions

/// Empty source is a program with no functions.
#[test]
fn empty_source_has_no_functions() {
    assert_eq!(printed(""), "");
}

/// Source with only comments and blank lines has no functions.
#[test]
fn comments_and_blank_lines_only_has_no_functions() {
    assert_eq!(printed("// nothing here\n\n// still nothing\n"), "");
}

/// A function with no parameters, no return type and an empty body.
#[test]
fn empty_function() {
    assert_eq!(printed("fn main() {}\n"), "fn main()\n");
}

/// The last line may end without a newline.
#[test]
fn function_at_end_of_file_without_newline() {
    assert_eq!(printed("fn main() {}"), "fn main()\n");
}

/// A function with one parameter.
#[test]
fn function_with_one_param() {
    assert_eq!(printed("fn f(a: i64) {}\n"), "fn f(a: i64)\n");
}

/// A function with several parameters.
#[test]
fn function_with_several_params() {
    let expected = "fn f(a: i64, b: f32, c: bool)\n";
    assert_eq!(printed("fn f(a: i64, b: f32, c: bool) {}\n"), expected);
}

/// A trailing comma after the last parameter is allowed.
#[test]
fn function_params_allow_a_trailing_comma() {
    let expected = "fn f(a: i64, b: i64)\n";
    assert_eq!(printed("fn f(a: i64, b: i64,) {}\n"), expected);
}

/// A function with a return type and no parameters.
#[test]
fn function_with_return_type() {
    assert_eq!(printed("fn answer() -> i64 {}\n"), "fn answer() -> i64\n");
}

/// A function with parameters and a return type.
#[test]
fn function_with_params_and_return_type() {
    let expected = "fn add(a: i64, b: i64) -> i64\n";
    assert_eq!(printed("fn add(a: i64, b: i64) -> i64 {}\n"), expected);
}

/// The one-line form with a return type returns the expression.
#[test]
fn one_line_function_with_return_type() {
    let expected = lines(&["fn double(x: i64) -> i64", "  return (* x 2)"]);
    assert_eq!(printed("fn double(x: i64) -> i64 = x * 2\n"), expected);
}

/// The one-line form without a return type is an expression statement.
#[test]
fn one_line_function_without_return_type() {
    let expected = lines(&["fn log(x: i64)", "  (call print x)"]);
    assert_eq!(printed("fn log(x: i64) = print(x)\n"), expected);
}

/// The one-line form works without parameters and at the end of the file.
#[test]
fn one_line_function_without_params_at_end_of_file() {
    let expected = lines(&["fn zero() -> i64", "  return 0"]);
    assert_eq!(printed("fn zero() -> i64 = 0"), expected);
}

/// The body block of a one-line function holds exactly one statement.
#[test]
fn one_line_function_body_has_one_statement() {
    let function = first_function("fn double(x: i64) -> i64 = x * 2\n");
    assert_eq!(function.body.stmts.len(), 1);
}

/// Several functions print one after the other, in source order.
#[test]
fn multiple_functions() {
    let src = concat!(
        "fn first() {\n",
        "    break\n",
        "}\n",
        "fn second() -> i64 = 1\n",
        "fn third(a: i64) {\n",
        "    return\n",
        "}\n",
    );
    let expected = lines(&[
        "fn first()",
        "  break",
        "fn second() -> i64",
        "  return 1",
        "fn third(a: i64)",
        "  return",
    ]);
    assert_eq!(printed(src), expected);
}

/// Blank lines and comments between functions are ignored.
#[test]
fn functions_separated_by_blank_lines_and_comments() {
    let src = "// first\nfn a() {}\n\n// second\n\nfn b(x: i64) -> i64 {\n    return x\n}\n";
    let expected = lines(&["fn a()", "fn b(x: i64) -> i64", "  return x"]);
    assert_eq!(printed(src), expected);
}

// ---------------------------------------------------------------- statements

/// An untyped let.
#[test]
fn let_without_type() {
    let expected = lines(&["fn main()", "  let x = (+ 1 2)"]);
    assert_eq!(printed_main("x := 1 + 2"), expected);
}

/// A typed let.
#[test]
fn let_with_type() {
    let expected = lines(&["fn main()", "  let x: i64 = (+ 1 2)"]);
    assert_eq!(printed_main("x: i64 = 1 + 2"), expected);
}

/// Every assignment operator.
#[test]
fn assign_with_every_operator() {
    for op in ["=", "+=", "-=", "*=", "/=", "@="] {
        let expected = lines(&["fn main()", &format!("  x {op} (+ y 1)")]);
        assert_eq!(
            printed_main(&format!("x {op} y + 1")),
            expected,
            "op `{op}`"
        );
    }
}

/// An expression statement starting with a name.
#[test]
fn expression_statement_starting_with_a_name() {
    let expected = lines(&["fn main()", "  (+ x 1)"]);
    assert_eq!(printed_main("x + 1"), expected);
}

/// A call as a statement.
#[test]
fn call_statement() {
    let expected = lines(&["fn main()", "  (call print a b)"]);
    assert_eq!(printed_main("print(a, b)"), expected);
}

/// An expression statement starting with a unary operator.
#[test]
fn expression_statement_starting_with_a_unary_operator() {
    let expected = lines(&["fn main()", "  (- x)"]);
    assert_eq!(printed_main("-x"), expected);
}

/// A bare `return` before a newline.
#[test]
fn return_without_value_at_end_of_block() {
    let expected = lines(&["fn main()", "  return"]);
    assert_eq!(printed_main("return"), expected);
}

/// A bare `return` followed by more statements.
#[test]
fn return_without_value_followed_by_a_statement() {
    let expected = lines(&["fn main()", "  return", "  let x = 1"]);
    assert_eq!(printed_main("return\nx := 1"), expected);
}

/// A bare `return` right before the closing brace on the same line.
#[test]
fn return_without_value_before_closing_brace_on_the_same_line() {
    let expected = lines(&["fn main()", "  return"]);
    assert_eq!(printed("fn main() { return }"), expected);
}

/// A `return` with a value before the closing brace on the same line.
#[test]
fn return_with_value_before_closing_brace_on_the_same_line() {
    let expected = lines(&["fn f() -> i64", "  return 1"]);
    assert_eq!(printed("fn f() -> i64 { return 1 }"), expected);
}

/// A `return` with an expression value.
#[test]
fn return_with_value() {
    let expected = lines(&["fn main()", "  return (+ a 1)"]);
    assert_eq!(printed_main("return a + 1"), expected);
}

/// `break` and `continue` statements.
#[test]
fn break_and_continue() {
    let expected = lines(&["fn main()", "  for", "    continue", "    break"]);
    assert_eq!(printed_main("for {\ncontinue\nbreak\n}"), expected);
}

/// A statement list separated by newlines keeps its order.
#[test]
fn statements_separated_by_newlines() {
    let expected = lines(&["fn main()", "  let a = 1", "  let b = 2", "  a += b"]);
    assert_eq!(printed_main("a := 1\nb := 2\na += b"), expected);
}

/// Blank lines and comments between statements are ignored.
#[test]
fn statements_separated_by_blank_lines_and_comments() {
    let expected = lines(&["fn main()", "  let a = 1", "  let b = 2"]);
    assert_eq!(
        printed_main("a := 1\n\n// note\n\nb := 2 // trailing"),
        expected
    );
}

/// A block can sit on one line with a single statement.
#[test]
fn block_on_one_line() {
    let expected = lines(&["fn main()", "  let x = 1"]);
    assert_eq!(printed("fn main() { x := 1 }"), expected);
}

/// An empty block prints no lines for its body.
#[test]
fn empty_block_inside_a_statement() {
    let expected = lines(&["fn main()", "  if a"]);
    assert_eq!(printed_main("if a {}"), expected);
}

// ---------------------------------------------------------------- if / else

/// An `if` without else.
#[test]
fn if_without_else() {
    let expected = lines(&["fn main()", "  if (== a b)", "    x = 1"]);
    assert_eq!(printed_main("if a == b {\nx = 1\n}"), expected);
}

/// An `if` with an else block.
#[test]
fn if_with_else_block() {
    let expected = lines(&["fn main()", "  if a", "    x = 1", "  else", "    x = 2"]);
    assert_eq!(printed_main("if a {\nx = 1\n} else {\nx = 2\n}"), expected);
}

/// An `else if` nests the second `if` under `else`.
#[test]
fn else_if_without_final_else() {
    let expected = lines(&[
        "fn main()",
        "  if a",
        "    x = 1",
        "  else",
        "    if b",
        "      x = 2",
    ]);
    assert_eq!(
        printed_main("if a {\nx = 1\n} else if b {\nx = 2\n}"),
        expected
    );
}

/// An else-if chain ending with an else block.
#[test]
fn else_if_chain_with_final_else() {
    let src = "if a {\nx = 1\n} else if b {\nx = 2\n} else {\nx = 3\n}";
    let expected = lines(&[
        "fn main()",
        "  if a",
        "    x = 1",
        "  else",
        "    if b",
        "      x = 2",
        "    else",
        "      x = 3",
    ]);
    assert_eq!(printed_main(src), expected);
}

/// A chain of three conditions nests twice.
#[test]
fn else_if_chain_of_three_conditions() {
    let src = "if a {\nx = 1\n} else if b {\nx = 2\n} else if c {\nx = 3\n}";
    let expected = lines(&[
        "fn main()",
        "  if a",
        "    x = 1",
        "  else",
        "    if b",
        "      x = 2",
        "    else",
        "      if c",
        "        x = 3",
    ]);
    assert_eq!(printed_main(src), expected);
}

/// An `if`/`else` on one line.
#[test]
fn if_else_on_one_line() {
    let expected = lines(&["fn main()", "  if a", "    x = 1", "  else", "    x = 2"]);
    assert_eq!(
        printed("fn main() { if a { x = 1 } else { x = 2 } }"),
        expected
    );
}

/// The else of an `else if` stores a nested `if` statement.
#[test]
fn else_if_is_stored_as_a_nested_if_statement() {
    let stmt = first_stmt("fn main() { if a { x = 1 } else if b { x = 2 } }");
    let (_, else_branch) = if_parts(stmt);
    let nested_is_if = match else_branch {
        Some(ElseBranch::If(nested)) => matches!(nested.kind, StmtKind::If { .. }),
        _ => false,
    };
    assert!(nested_is_if);
}

/// A statement after an `if`/`else` continues at the same level.
#[test]
fn statement_after_if_else() {
    let src = "if a {\nx = 1\n} else {\nx = 2\n}\nreturn";
    let expected = lines(&[
        "fn main()",
        "  if a",
        "    x = 1",
        "  else",
        "    x = 2",
        "  return",
    ]);
    assert_eq!(printed_main(src), expected);
}

/// A call can be the condition.
#[test]
fn if_with_call_condition() {
    let expected = lines(&["fn main()", "  if (call ready x)", "    break"]);
    assert_eq!(printed_main("if ready(x) {\nbreak\n}"), expected);
}

// ---------------------------------------------------------------- for

/// An infinite loop.
#[test]
fn infinite_for() {
    let expected = lines(&["fn main()", "  for", "    break"]);
    assert_eq!(printed_main("for {\nbreak\n}"), expected);
}

/// A while loop with a comparison condition.
#[test]
fn while_for() {
    let expected = lines(&["fn main()", "  for (< i 10)", "    i += 1"]);
    assert_eq!(printed_main("for i < 10 {\ni += 1\n}"), expected);
}

/// A while loop whose condition is a single name.
#[test]
fn while_for_with_a_name_condition() {
    let expected = lines(&["fn main()", "  for done", "    break"]);
    assert_eq!(printed_main("for done {\nbreak\n}"), expected);
}

/// A while loop whose condition is a call.
#[test]
fn while_for_with_a_call_condition() {
    let expected = lines(&["fn main()", "  for (call more x)", "    break"]);
    assert_eq!(printed_main("for more(x) {\nbreak\n}"), expected);
}

/// A `for ... in` loop over an exclusive range.
#[test]
fn for_in_range() {
    let expected = lines(&["fn main()", "  for i in (.. 0 n)", "    (call f i)"]);
    assert_eq!(printed_main("for i in 0..n {\nf(i)\n}"), expected);
}

/// A `for ... in` loop over an inclusive range.
#[test]
fn for_in_inclusive_range() {
    let expected = lines(&["fn main()", "  for i in (..= 1 n)", "    break"]);
    assert_eq!(printed_main("for i in 1..=n {\nbreak\n}"), expected);
}

/// A `for ... in` loop over a plain name.
#[test]
fn for_in_name() {
    let expected = lines(&["fn main()", "  for item in items", "    break"]);
    assert_eq!(printed_main("for item in items {\nbreak\n}"), expected);
}

/// Nested loops and `if` indent two spaces per level.
#[test]
fn nested_blocks_indent_two_spaces_per_level() {
    let src = "for i in 0..n {\nif c {\nx += 1\n}\n}";
    let expected = lines(&[
        "fn main()",
        "  for i in (.. 0 n)",
        "    if c",
        "      x += 1",
    ]);
    assert_eq!(printed_main(src), expected);
}

// ---------------------------------------------------------------- expression literals and names

/// An integer literal.
#[test]
fn integer_literal() {
    assert_eq!(printed_expr("42"), "42");
}

/// Hex, binary and underscored integers print as decimal.
#[test]
fn integer_literal_in_other_notations_prints_as_decimal() {
    assert_eq!(printed_expr("0xFF"), "255");
    assert_eq!(printed_expr("0b101"), "5");
    assert_eq!(printed_expr("1_000"), "1000");
}

/// The largest integer is accepted.
#[test]
fn largest_integer_literal() {
    assert_eq!(printed_expr("18446744073709551615"), "18446744073709551615");
}

/// Boolean literals.
#[test]
fn boolean_literals() {
    assert_eq!(printed_expr("true"), "true");
    assert_eq!(printed_expr("false"), "false");
}

/// A name.
#[test]
fn name_expression() {
    assert_eq!(printed_expr("total"), "total");
}

// ---------------------------------------------------------------- precedence

/// Multiplication binds tighter than addition on its right.
#[test]
fn mul_binds_tighter_than_add_on_the_right() {
    assert_eq!(printed_expr("a + b * c"), "(+ a (* b c))");
}

/// Multiplication binds tighter than addition on its left.
#[test]
fn mul_binds_tighter_than_add_on_the_left() {
    assert_eq!(printed_expr("a * b + c"), "(+ (* a b) c)");
}

/// Unary minus binds tighter than multiplication.
#[test]
fn unary_minus_binds_tighter_than_mul() {
    assert_eq!(printed_expr("-a * b"), "(* (- a) b)");
}

/// Every unary operator binds tighter than `@`.
#[test]
fn every_unary_operator_binds_tighter_than_matmul() {
    for op in ["-", "!", "~", "&", "*"] {
        let expected = format!("(@ ({op} a) b)");
        assert_eq!(printed_expr(&format!("{op}a @ b")), expected, "op `{op}`");
    }
}

/// Matrix multiplication binds tighter than multiplication on its left.
#[test]
fn matmul_binds_tighter_than_mul_on_the_left() {
    assert_eq!(printed_expr("a @ b * c"), "(* (@ a b) c)");
}

/// Matrix multiplication binds tighter than multiplication on its right.
#[test]
fn matmul_binds_tighter_than_mul_on_the_right() {
    assert_eq!(printed_expr("a * b @ c"), "(* a (@ b c))");
}

/// Addition binds tighter than equality.
#[test]
fn add_binds_tighter_than_equality() {
    assert_eq!(printed_expr("a + b == c"), "(== (+ a b) c)");
}

/// Equality binds tighter than `&&`.
#[test]
fn equality_binds_tighter_than_and() {
    assert_eq!(printed_expr("a == b && c"), "(&& (== a b) c)");
}

/// `&&` binds tighter than `||` on its left.
#[test]
fn and_binds_tighter_than_or_on_the_left() {
    assert_eq!(printed_expr("a && b || c"), "(|| (&& a b) c)");
}

/// `&&` binds tighter than `||` on its right.
#[test]
fn and_binds_tighter_than_or_on_the_right() {
    assert_eq!(printed_expr("a || b && c"), "(|| a (&& b c))");
}

/// A range with an addition on its right.
#[test]
fn addition_binds_tighter_than_range() {
    assert_eq!(printed_expr("0..n + 1"), "(.. 0 (+ n 1))");
}

/// A range binds tighter than comparison.
#[test]
fn range_binds_tighter_than_comparison() {
    assert_eq!(printed_expr("a..b == c"), "(== (.. a b) c)");
}

/// Shift binds as tightly as multiplication, so tighter than addition.
#[test]
fn shift_binds_tighter_than_add() {
    assert_eq!(printed_expr("a << b + c"), "(+ (<< a b) c)");
}

/// Bitwise and binds as tightly as multiplication, so tighter than bitwise or.
#[test]
fn bit_and_binds_tighter_than_bit_or() {
    assert_eq!(printed_expr("a & b | c"), "(| (& a b) c)");
}

/// Bitwise and binds tighter than addition on its right.
#[test]
fn bit_and_binds_tighter_than_add_on_the_right() {
    assert_eq!(printed_expr("a + b & c"), "(+ a (& b c))");
}

/// For each pair of adjacent levels, the higher level binds tighter on both sides.
#[test]
fn adjacent_levels_bind_in_table_order() {
    let high_to_low = ["@", "*", "+", "..", "==", "&&", "||"];
    for pair in high_to_low.windows(2) {
        let (high, low) = (pair[0], pair[1]);
        let high_first = printed_expr(&format!("a {high} b {low} c"));
        assert_eq!(high_first, format!("({low} ({high} a b) c)"));
        let low_first = printed_expr(&format!("a {low} b {high} c"));
        assert_eq!(low_first, format!("({low} a ({high} b c))"));
    }
}

/// Every comparison operator parses at the comparison level.
#[test]
fn every_comparison_operator() {
    for op in ["==", "!=", "<", "<=", ">", ">="] {
        let expected = format!("({op} (+ a 1) b)");
        assert_eq!(
            printed_expr(&format!("a + 1 {op} b")),
            expected,
            "op `{op}`"
        );
    }
}

/// Every operator of the multiplication level binds tighter than addition.
#[test]
fn every_multiplication_level_operator() {
    for op in ["*", "/", "%", "<<", ">>", "&"] {
        let expected = format!("(+ a ({op} b c))");
        assert_eq!(
            printed_expr(&format!("a + b {op} c")),
            expected,
            "op `{op}`"
        );
    }
}

/// Every operator of the addition level binds tighter than a range.
#[test]
fn every_addition_level_operator() {
    for op in ["+", "-", "|", "^"] {
        let expected = format!("(.. a ({op} b c))");
        assert_eq!(
            printed_expr(&format!("a .. b {op} c")),
            expected,
            "op `{op}`"
        );
    }
}

// ---------------------------------------------------------------- associativity

/// Subtraction is left associative.
#[test]
fn subtraction_is_left_associative() {
    assert_eq!(printed_expr("a - b - c"), "(- (- a b) c)");
}

/// Division is left associative.
#[test]
fn division_is_left_associative() {
    assert_eq!(printed_expr("a / b / c"), "(/ (/ a b) c)");
}

/// Matrix multiplication is left associative.
#[test]
fn matmul_is_left_associative() {
    assert_eq!(printed_expr("a @ b @ c"), "(@ (@ a b) c)");
}

/// Operators of the same level mix left to right.
#[test]
fn same_level_operators_group_left_to_right() {
    assert_eq!(printed_expr("a * b / c % d"), "(% (/ (* a b) c) d)");
    assert_eq!(
        printed_expr("a + b - c | d ^ e"),
        "(^ (| (- (+ a b) c) d) e)"
    );
}

/// `&&` is left associative.
#[test]
fn and_is_left_associative() {
    assert_eq!(printed_expr("a && b && c"), "(&& (&& a b) c)");
}

/// `||` is left associative.
#[test]
fn or_is_left_associative() {
    assert_eq!(printed_expr("a || b || c"), "(|| (|| a b) c)");
}

// ---------------------------------------------------------------- unary and parentheses

/// A unary operator can repeat.
#[test]
fn repeated_unary_minus() {
    assert_eq!(printed_expr("--x"), "(- (- x))");
}

/// Different unary operators stack.
#[test]
fn stacked_unary_operators() {
    assert_eq!(printed_expr("!-x"), "(! (- x))");
    assert_eq!(printed_expr("~*p"), "(~ (* p))");
    assert_eq!(printed_expr("&-x"), "(& (- x))");
}

/// Every unary operator.
#[test]
fn every_unary_operator() {
    for op in ["-", "!", "~", "&", "*"] {
        assert_eq!(printed_expr(&format!("{op}x")), format!("({op} x)"));
    }
}

/// A binary minus can be followed by a unary minus.
#[test]
fn binary_minus_followed_by_unary_minus() {
    assert_eq!(printed_expr("a - -b"), "(- a (- b))");
}

/// A unary dereference on the left of an addition.
#[test]
fn unary_deref_before_addition() {
    assert_eq!(printed_expr("*p + 1"), "(+ (* p) 1)");
}

/// Parentheses override precedence on the left.
#[test]
fn parentheses_override_precedence() {
    assert_eq!(printed_expr("(a + b) * c"), "(* (+ a b) c)");
}

/// Parentheses override precedence on the right.
#[test]
fn parentheses_override_left_associativity() {
    assert_eq!(printed_expr("a - (b - c)"), "(- a (- b c))");
}

/// Parentheses produce no node of their own.
#[test]
fn redundant_parentheses_vanish() {
    assert_eq!(printed_expr("((a))"), "a");
}

/// Parentheses can follow a unary operator.
#[test]
fn parenthesized_operand_of_unary() {
    assert_eq!(printed_expr("-(a + b)"), "(- (+ a b))");
}

// ---------------------------------------------------------------- comparisons

/// A parenthesized comparison can be the left operand of another comparison.
#[test]
fn parenthesized_comparison_on_the_left_is_allowed() {
    assert_eq!(printed_expr("(a < b) == c"), "(== (< a b) c)");
}

/// A parenthesized comparison can be the right operand of another comparison.
#[test]
fn parenthesized_comparison_on_the_right_is_allowed() {
    assert_eq!(printed_expr("a == (b == c)"), "(== a (== b c))");
}

/// Two comparisons joined by `&&` are not chained.
#[test]
fn comparisons_joined_by_and_are_allowed() {
    assert_eq!(printed_expr("a < b && b < c"), "(&& (< a b) (< b c))");
}

// ---------------------------------------------------------------- calls

/// A call with no arguments.
#[test]
fn call_with_no_arguments() {
    assert_eq!(printed_expr("f()"), "(call f)");
}

/// A call with one argument.
#[test]
fn call_with_one_argument() {
    assert_eq!(printed_expr("f(a)"), "(call f a)");
}

/// A call with many arguments.
#[test]
fn call_with_many_arguments() {
    assert_eq!(
        printed_expr("f(a, 2, true, b + c)"),
        "(call f a 2 true (+ b c))"
    );
}

/// A trailing comma after the last argument is allowed.
#[test]
fn call_allows_a_trailing_comma() {
    assert_eq!(printed_expr("f(a, b,)"), "(call f a b)");
}

/// A trailing comma after a single argument is allowed.
#[test]
fn call_with_one_argument_allows_a_trailing_comma() {
    assert_eq!(printed_expr("f(a,)"), "(call f a)");
}

/// Calls nest inside arguments.
#[test]
fn nested_calls() {
    assert_eq!(printed_expr("f(g(x), h())"), "(call f (call g x) (call h))");
}

/// A call is a primary expression usable as an operand.
#[test]
fn call_as_operand() {
    assert_eq!(
        printed_expr("-f(x) + g(y)"),
        "(+ (- (call f x)) (call g y))"
    );
}

/// An argument can be a comparison.
#[test]
fn call_with_a_comparison_argument() {
    assert_eq!(printed_expr("f(a < b)"), "(call f (< a b))");
}

// ---------------------------------------------------------------- node ids

/// All node ids are unique in the fib example.
#[test]
fn node_ids_are_unique_in_fib() {
    let src = "fn fib(n: i64) -> i64 {\n    if n < 2 {\n        return n\n    }\n    return fib(n - 1) + fib(n - 2)\n}\n";
    assert_ids_unique(src);
}

/// All node ids are unique in a program using every construct and two functions.
#[test]
fn node_ids_are_unique_in_a_program_using_every_construct() {
    assert_ids_unique(EVERYTHING);
}

/// The program using every construct really contains many nodes.
#[test]
fn everything_program_has_many_nodes() {
    assert!(all_ids(&program(EVERYTHING)).len() > 50);
}

/// Ids start at 0.
#[test]
fn node_ids_start_at_zero() {
    let ids = all_ids(&program("fn main() {\n    x := 1\n}\n"));
    assert_eq!(ids.into_iter().min(), Some(NodeId(0)));
}

// ---------------------------------------------------------------- spans

/// A binary expression spans from its left operand to its right operand.
#[test]
fn binary_span_covers_both_operands() {
    let expr = parsed_expr("a + b * c");
    assert_eq!(expr.span, expr_span(0, 9));
    let (left, right) = binary_operands(expr);
    assert_eq!(left.span, expr_span(0, 1));
    assert_eq!(right.span, expr_span(4, 9));
}

/// A unary expression spans from the operator to the end of the operand.
#[test]
fn unary_span_covers_operator_and_operand() {
    let expr = parsed_expr("-abc");
    assert_eq!(expr.span, expr_span(0, 4));
    assert_eq!(unary_operand(expr).span, expr_span(1, 4));
}

/// A call spans from its name to the closing parenthesis.
#[test]
fn call_span_covers_name_and_closing_parenthesis() {
    let expr = parsed_expr("f(a, b)");
    assert_eq!(expr.span, expr_span(0, 7));
}

/// A call with no arguments spans its name and both parentheses.
#[test]
fn empty_call_span() {
    assert_eq!(parsed_expr("foo()").span, expr_span(0, 5));
}

/// A trailing comma inside a call is part of the call's span.
#[test]
fn call_span_includes_the_trailing_comma() {
    assert_eq!(parsed_expr("f(a,)").span, expr_span(0, 5));
}

/// A parenthesized expression keeps the span of the inner expression.
#[test]
fn parenthesized_expression_keeps_the_inner_span() {
    assert_eq!(parsed_expr("(a + b)").span, expr_span(1, 6));
}

/// A literal and a name span their own token.
#[test]
fn literal_and_name_spans() {
    assert_eq!(parsed_expr("12345").span, expr_span(0, 5));
    assert_eq!(parsed_expr("true").span, expr_span(0, 4));
    assert_eq!(parsed_expr("name").span, expr_span(0, 4));
}

/// A let statement spans from the name to the end of the value.
#[test]
fn let_statement_span() {
    let src = main_source("x := 1 + 2");
    assert_eq!(first_stmt(&src).span, span_of(&src, "x := 1 + 2"));
}

/// A typed let statement spans from the name to the end of the value.
#[test]
fn typed_let_statement_span() {
    let src = main_source("x: i64 = 1 + 2");
    assert_eq!(first_stmt(&src).span, span_of(&src, "x: i64 = 1 + 2"));
}

/// An assignment spans from the target to the end of the value.
#[test]
fn assign_statement_span() {
    let src = main_source("x += f(1)");
    assert_eq!(first_stmt(&src).span, span_of(&src, "x += f(1)"));
}

/// An expression statement spans its expression.
#[test]
fn expression_statement_span() {
    let src = main_source("f(a, b)");
    assert_eq!(first_stmt(&src).span, span_of(&src, "f(a, b)"));
}

/// A `return` with a value spans to the end of the value.
#[test]
fn return_with_value_span() {
    let src = main_source("return 1 + 2");
    assert_eq!(first_stmt(&src).span, span_of(&src, "return 1 + 2"));
}

/// A bare `return` spans only the keyword.
#[test]
fn return_without_value_span() {
    let src = "fn main() { return }";
    assert_eq!(first_stmt(src).span, span_of(src, "return"));
}

/// `break` spans its keyword.
#[test]
fn break_span() {
    let src = "fn main() { for { break } }";
    assert_eq!(first_for_body_stmt(src).span, span_of(src, "break"));
}

/// `continue` spans its keyword.
#[test]
fn continue_span() {
    let src = "fn main() { for { continue } }";
    assert_eq!(first_for_body_stmt(src).span, span_of(src, "continue"));
}

/// An `if` spans to the closing brace of its then-block when it has no else.
#[test]
fn if_statement_span_without_else() {
    let src = "fn main() { if a { x = 1 } }";
    assert_eq!(first_stmt(src).span, span_of(src, "if a { x = 1 }"));
}

/// An `if` with an else spans to the closing brace of the else block.
#[test]
fn if_statement_span_with_else() {
    let src = "fn main() { if a { x = 1 } else { x = 2 } }";
    assert_eq!(
        first_stmt(src).span,
        span_of(src, "if a { x = 1 } else { x = 2 }")
    );
}

/// An `if` with an else-if spans to the last closing brace, and so does the nested `if`.
#[test]
fn if_statement_span_with_else_if() {
    let src = "fn main() { if a { x = 1 } else if b { x = 2 } }";
    let stmt = first_stmt(src);
    assert_eq!(
        stmt.span,
        span_of(src, "if a { x = 1 } else if b { x = 2 }")
    );
    let (_, else_branch) = if_parts(stmt);
    let nested_span = match else_branch {
        Some(ElseBranch::If(nested)) => Some(nested.span),
        _ => None,
    };
    assert_eq!(nested_span, Some(span_of(src, "if b { x = 2 }")));
}

/// A `for` spans from the keyword to the closing brace of its body.
#[test]
fn for_statement_span() {
    let src = "fn main() { for i in 0..n { f(i) } }";
    assert_eq!(first_stmt(src).span, span_of(src, "for i in 0..n { f(i) }"));
}

/// A block spans from its opening brace to its closing brace.
#[test]
fn block_span_covers_the_braces() {
    let src = "fn f() {\n    x := 1\n}\n";
    let function = first_function(src);
    assert_eq!(function.body.span, span_of(src, "{\n    x := 1\n}"));
}

/// A function spans from `fn` to the end of its body.
#[test]
fn function_span_covers_header_and_body() {
    let src = "fn f() {\n    x := 1\n}\nfn g() {}\n";
    let function = first_function(src);
    assert_eq!(function.span, span_of(src, "fn f() {\n    x := 1\n}"));
}

/// The second function has its own span.
#[test]
fn second_function_span() {
    let src = "fn f() {}\nfn g(a: i64) -> i64 {\n    return a\n}\n";
    let second = program(src).functions.remove(1);
    assert_eq!(
        second.span,
        span_of(src, "fn g(a: i64) -> i64 {\n    return a\n}")
    );
}

/// A one-line function spans from `fn` to the end of its expression.
#[test]
fn one_line_function_span() {
    let src = "fn double(x: i64) -> i64 = x * 2\n";
    let function = first_function(src);
    assert_eq!(
        function.span,
        span_of(src, "fn double(x: i64) -> i64 = x * 2")
    );
}

/// The body block of a one-line function has the span of its expression.
#[test]
fn one_line_function_body_span_is_the_expression_span() {
    let src = "fn double(x: i64) -> i64 = x * 2\n";
    let function = first_function(src);
    assert_eq!(function.body.span, span_of(src, "x * 2"));
}

// ---------------------------------------------------------------- errors: top level

/// Each unsupported top-level declaration keyword is reported at the keyword.
#[test]
fn unsupported_top_level_declarations() {
    for keyword in ["struct", "enum", "const", "import", "extern", "test"] {
        let src = format!("{keyword} x\n");
        let message = format!("`{keyword}` declarations are not supported yet");
        assert_error(&src, &message, Span::new(0, keyword.len()));
    }
}

/// `pub` has its own message.
#[test]
fn pub_is_not_supported_yet() {
    assert_error(
        "pub fn f() {}\n",
        "`pub` is not supported yet",
        Span::new(0, 3),
    );
}

/// A declaration keyword after a valid function is still reported.
#[test]
fn unsupported_declaration_after_a_function() {
    let src = "fn main() {}\nstruct P {}\n";
    assert_error(
        src,
        "`struct` declarations are not supported yet",
        span_of(src, "struct"),
    );
}

/// An identifier at the top level is not a function.
#[test]
fn identifier_at_top_level() {
    let src = "x := 1\n";
    assert_error(
        src,
        "expected `fn`, found identifier `x`",
        span_of(src, "x"),
    );
}

/// An integer at the top level is not a function.
#[test]
fn integer_at_top_level() {
    assert_error(
        "42\n",
        "expected `fn`, found integer literal",
        Span::new(0, 2),
    );
}

/// A stray closing brace at the top level is not a function.
#[test]
fn closing_brace_at_top_level() {
    let src = "fn main() {}\n}\n";
    assert_error(src, "expected `fn`, found `}`", nth_span(src, "}", 1));
}

// ---------------------------------------------------------------- errors: functions and types

/// A parameter without a type is not supported yet.
#[test]
fn untyped_parameter() {
    let src = "fn f(a, b: i64) {}\n";
    assert_error(
        src,
        "untyped parameters are not supported yet",
        span_of(src, "a"),
    );
}

/// A later untyped parameter is reported at its name.
#[test]
fn untyped_second_parameter() {
    let src = "fn f(a: i64, bee) {}\n";
    assert_error(
        src,
        "untyped parameters are not supported yet",
        span_of(src, "bee"),
    );
}

/// A parenthesized return type means multiple return values.
#[test]
fn return_type_in_parentheses() {
    let src = "fn f() -> (i64, i64) {}\n";
    assert_error(
        src,
        "multiple return values are not supported yet",
        nth_span(src, "(", 1),
    );
}

/// A return type with `!` is an error return type.
#[test]
fn return_type_with_bang() {
    let src = "fn f() -> !i64 {}\n";
    assert_error(
        src,
        "error return types are not supported yet",
        span_of(src, "!"),
    );
}

/// A `[` in a parameter type position.
#[test]
fn bracket_in_parameter_type() {
    let src = "fn f(a: [i64]) {}\n";
    assert_error(src, "only named types are supported yet", span_of(src, "["));
}

/// A `*` in a parameter type position.
#[test]
fn star_in_parameter_type() {
    let src = "fn f(a: *i64) {}\n";
    assert_error(src, "only named types are supported yet", span_of(src, "*"));
}

/// A `?` in a parameter type position.
#[test]
fn question_in_parameter_type() {
    let src = "fn f(a: ?i64) {}\n";
    assert_error(src, "only named types are supported yet", span_of(src, "?"));
}

/// A `fn` in a parameter type position.
#[test]
fn fn_in_parameter_type() {
    let src = "fn f(a: fn) {}\n";
    assert_error(
        src,
        "only named types are supported yet",
        nth_span(src, "fn", 1),
    );
}

/// A named type followed by `[` is a tensor type and is reported at the `[`.
#[test]
fn tensor_type_in_parameter() {
    let src = "fn f(a: f32[3]) {}\n";
    assert_error(src, "only named types are supported yet", span_of(src, "["));
}

/// A `[` in the return type position.
#[test]
fn bracket_in_return_type() {
    let src = "fn f() -> [i64] {}\n";
    assert_error(src, "only named types are supported yet", span_of(src, "["));
}

/// A `*` in a let's type position.
#[test]
fn star_in_let_type() {
    let src = main_source("x: *i64 = 1");
    assert_error(
        &src,
        "only named types are supported yet",
        span_of(&src, "*"),
    );
}

/// A missing `)` after the parameters.
#[test]
fn missing_closing_parenthesis_after_parameters() {
    let src = "fn f(a: i64 {}\n";
    assert_error(src, "expected `)`, found `{`", span_of(src, "{"));
}

// ---------------------------------------------------------------- errors: blocks and statements

/// A block that is never closed.
#[test]
fn missing_closing_brace_at_end_of_file() {
    let src = "fn main() {\n    x := 1\n";
    assert_error(
        src,
        "expected `}`, found end of file",
        Span::new(src.len(), src.len()),
    );
}

/// Two statements on one line need a newline between them.
#[test]
fn two_statements_on_one_line() {
    let src = "fn main() { x := 1 y := 2 }";
    assert_error(
        src,
        "expected newline or `}` after statement, found identifier `y`",
        span_of(src, "y"),
    );
}

/// A closing parenthesis after a statement is neither a newline nor `}`.
#[test]
fn stray_token_after_statement() {
    let src = "fn main() { x := 1 ) }";
    assert_error(
        src,
        "expected newline or `}` after statement, found `)`",
        nth_span(src, ")", 1),
    );
}

/// `else` on the line after the closing brace is not allowed.
#[test]
fn else_on_the_next_line() {
    let src =
        "fn main() {\n    if a {\n        x := 1\n    }\n    else {\n        x := 2\n    }\n}\n";
    assert_error(
        src,
        "`else` must be on the same line as the closing `}`",
        span_of(src, "else"),
    );
}

/// `match` is not supported yet.
#[test]
fn match_statement() {
    let src = main_source("match x {\n}");
    assert_error(&src, "`match` is not supported yet", span_of(&src, "match"));
}

/// `defer` is not supported yet.
#[test]
fn defer_statement() {
    let src = main_source("defer f()");
    assert_error(&src, "`defer` is not supported yet", span_of(&src, "defer"));
}

/// `arena` is not supported yet.
#[test]
fn arena_statement() {
    let src = main_source("arena {\n}");
    assert_error(&src, "`arena` is not supported yet", span_of(&src, "arena"));
}

/// An assignment to a binary expression is an error at the left side.
#[test]
fn cannot_assign_to_a_binary_expression() {
    let src = main_source("a + b = 1");
    assert_error(
        &src,
        "cannot assign to this expression",
        span_of(&src, "a + b"),
    );
}

/// An assignment to a call is an error at the left side.
#[test]
fn cannot_assign_to_a_call() {
    let src = main_source("f(x) = 1");
    assert_error(
        &src,
        "cannot assign to this expression",
        span_of(&src, "f(x)"),
    );
}

/// An assignment to a literal is an error at the left side.
#[test]
fn cannot_assign_to_a_literal() {
    let src = main_source("7 = 1");
    assert_error(&src, "cannot assign to this expression", span_of(&src, "7"));
}

/// A compound assignment to a unary expression is an error at the left side.
#[test]
fn cannot_compound_assign_to_a_unary_expression() {
    let src = main_source("-x += 1");
    assert_error(
        &src,
        "cannot assign to this expression",
        span_of(&src, "-x"),
    );
}

/// Multiple assignment is not supported yet (span: the first comma).
#[test]
fn multiple_assignment() {
    let src = main_source("a, b := 1, 2");
    assert_error(
        &src,
        "multiple assignment is not supported yet",
        span_of(&src, ","),
    );
}

/// Multiple loop variables are not supported yet (span: the first comma).
#[test]
fn multiple_loop_variables() {
    let src = main_source("for i, v in items {\n}");
    assert_error(
        &src,
        "multiple loop variables are not supported yet",
        span_of(&src, ","),
    );
}

/// Multiple return values are not supported yet (span: the first comma).
#[test]
fn multiple_return_values() {
    let src = main_source("return a, b");
    assert_error(
        &src,
        "multiple return values are not supported yet",
        span_of(&src, ","),
    );
}

/// Multiple return values on one line before the closing brace.
#[test]
fn multiple_return_values_on_one_line() {
    let src = "fn f() { return 1, 2 }";
    assert_error(
        src,
        "multiple return values are not supported yet",
        span_of(src, ","),
    );
}

// ---------------------------------------------------------------- errors: unsupported expressions

/// A float literal is not supported yet.
#[test]
fn float_literal() {
    assert_expr_error("1.5", "float literals are not supported yet", 0, 3);
}

/// A string literal is not supported yet.
#[test]
fn string_literal() {
    assert_expr_error("\"hi\"", "string literals are not supported yet", 0, 4);
}

/// `none` is not supported yet.
#[test]
fn none_literal() {
    assert_expr_error("none", "`none` is not supported yet", 0, 4);
}

/// A tensor literal is not supported yet, reported at the `[`.
#[test]
fn tensor_literal() {
    assert_expr_error("[1, 2]", "tensor literals are not supported yet", 0, 1);
}

/// `grad` is not supported yet.
#[test]
fn grad_expression() {
    assert_expr_error("grad(f)", "`grad` is not supported yet", 0, 4);
}

/// Field access is not supported yet, reported at the `.`.
#[test]
fn field_access() {
    assert_expr_error("a.b", "field access is not supported yet", 1, 2);
}

/// Indexing is not supported yet, reported at the `[`.
#[test]
fn indexing() {
    assert_expr_error("a[0]", "indexing is not supported yet", 1, 2);
}

/// Error propagation is not supported yet, reported at the `?`.
#[test]
fn error_propagation() {
    assert_expr_error("a?", "error propagation is not supported yet", 1, 2);
}

/// `on` after an expression is not supported yet.
#[test]
fn on_after_expression() {
    assert_expr_error("a on b", "`on` is not supported yet", 2, 4);
}

/// A named argument is not supported yet (span: the argument name).
#[test]
fn named_argument() {
    assert_expr_error("f(a: 1)", "named arguments are not supported yet", 2, 3);
}

/// Calling a parenthesized name is not supported (span: the call's opening parenthesis).
#[test]
fn calling_a_parenthesized_expression() {
    assert_expr_error("(f)(x)", "only named functions can be called", 3, 4);
}

/// An unsupported feature inside a call argument is still reported.
#[test]
fn unsupported_feature_inside_call_arguments() {
    assert_expr_error("f(1, 2.5)", "float literals are not supported yet", 5, 8);
}

/// Parsing stops at the first error.
#[test]
fn only_the_first_error_is_reported() {
    let src = main_source("x := 1.5\ny := \"s\"");
    assert_error(
        &src,
        "float literals are not supported yet",
        span_of(&src, "1.5"),
    );
}

// ---------------------------------------------------------------- errors: expressions

/// A chained comparison is reported at the second operator.
#[test]
fn chained_less_than() {
    assert_expr_error("a < b < c", "comparison operators cannot be chained", 6, 7);
}

/// Chaining different comparison operators is reported at the second operator.
#[test]
fn chained_equality_and_not_equal() {
    assert_expr_error(
        "a == b != c",
        "comparison operators cannot be chained",
        7,
        9,
    );
}

/// Chaining a relational and an equality operator is reported at the second operator.
#[test]
fn chained_less_than_and_equal() {
    assert_expr_error("a < b == c", "comparison operators cannot be chained", 6, 8);
}

/// Operands of the chain may be arithmetic expressions.
#[test]
fn chained_comparison_with_arithmetic_operands() {
    assert_expr_error(
        "a + 1 < b * 2 <= c",
        "comparison operators cannot be chained",
        14,
        16,
    );
}

/// A missing `)` in a parenthesized expression is found at the newline.
#[test]
fn missing_closing_parenthesis_in_group_at_newline() {
    assert_expr_error("(a + b", "expected `)`, found newline", 6, 7);
}

/// A name after a parenthesized expression's content is not a `)`.
#[test]
fn missing_closing_parenthesis_in_group_before_a_name() {
    assert_expr_error("(a b)", "expected `)`, found identifier `b`", 3, 4);
}

/// A missing `)` in a call before another argument without a comma.
#[test]
fn missing_closing_parenthesis_in_call_before_a_name() {
    assert_expr_error("f(a b)", "expected `)`, found identifier `b`", 4, 5);
}

/// A call that is never closed is found at the newline.
#[test]
fn missing_closing_parenthesis_in_call_at_newline() {
    assert_expr_error("f(a", "expected `)`, found newline", 3, 4);
}

/// A closing parenthesis where an expression is expected.
#[test]
fn expected_expression_found_closing_parenthesis() {
    assert_expr_error("1 + )", "expected expression, found `)`", 4, 5);
}

/// A comma where an argument is expected.
#[test]
fn expected_expression_found_comma_in_call() {
    assert_expr_error("f(,)", "expected expression, found `,`", 2, 3);
}

/// A second comma in a row inside a call.
#[test]
fn expected_expression_after_double_comma() {
    assert_expr_error("f(a,, b)", "expected expression, found `,`", 4, 5);
}

/// A keyword where an expression is expected.
#[test]
fn expected_expression_found_keyword() {
    assert_expr_error("else", "expected expression, found `else`", 0, 4);
}

/// An operator at the end of a line continues on the next line, so the `}` is found.
#[test]
fn expected_expression_after_trailing_operator() {
    let src = "fn main() {\n    x := 1 +\n}\n";
    assert_error(src, "expected expression, found `}`", span_of(src, "}"));
}

/// End of file where an expression is expected.
#[test]
fn expected_expression_found_end_of_file() {
    let src = "fn f() =";
    assert_error(
        src,
        "expected expression, found end of file",
        Span::new(src.len(), src.len()),
    );
}

/// A unary operator without an operand.
#[test]
fn expected_expression_after_unary_operator() {
    assert_expr_error("-)", "expected expression, found `)`", 1, 2);
}

// ---------------------------------------------------------------- amendment: nesting limit

/// The deepest nesting the parser accepts for every shape (decision 0025).
const NESTING_LIMIT: usize = 64;

/// The error for nesting beyond the limit.
const NESTING_ERROR: &str = "nesting is too deep (limit 64)";

// In every shape below, "N levels" means N nested constructs of that shape, and the error span is
// the first token of the (N+1)th construct, which is where the limit is crossed.

/// Source with `levels` nested parenthesized groups around a name.
fn nested_parens_source(levels: usize) -> String {
    expr_source(&format!("{}x{}", "(".repeat(levels), ")".repeat(levels)))
}

/// Source with `levels` prefix operators in front of a name.
fn nested_prefix_operators_source(levels: usize) -> String {
    expr_source(&format!("{}x", "-".repeat(levels)))
}

/// Source with `levels` calls nested as the argument of the previous call.
fn nested_calls_source(levels: usize) -> String {
    expr_source(&format!("{}x{}", "f(".repeat(levels), ")".repeat(levels)))
}

/// Source with `levels` `if` statements nested in each other's then-block.
fn nested_ifs_source(levels: usize) -> String {
    let open = "if a {\n".repeat(levels);
    let close = "}\n".repeat(levels);
    main_source(&format!("{open}x := 1\n{close}"))
}

/// Source with an `if` followed by `levels - 1` `else if` links.
fn else_if_chain_source(levels: usize) -> String {
    let links = " else if a {\n}".repeat(levels - 1);
    main_source(&format!("if a {{\n}}{links}"))
}

/// Source with `levels` infinite `for` loops nested in each other.
fn nested_fors_source(levels: usize) -> String {
    let open = "for {\n".repeat(levels);
    let close = "}\n".repeat(levels);
    main_source(&format!("{open}break\n{close}"))
}

/// Exactly 64 nested parentheses parse.
#[test]
fn nesting_limit_parens_accepts_64() {
    assert!(parse_source(&nested_parens_source(NESTING_LIMIT)).is_ok());
}

/// 65 nested parentheses are rejected at the 65th `(`.
#[test]
fn nesting_limit_parens_rejects_65() {
    let src = nested_parens_source(NESTING_LIMIT + 1);
    assert_error(
        &src,
        NESTING_ERROR,
        expr_span(NESTING_LIMIT, NESTING_LIMIT + 1),
    );
}

/// Exactly 64 repeated prefix operators parse.
#[test]
fn nesting_limit_prefix_operators_accepts_64() {
    assert!(parse_source(&nested_prefix_operators_source(NESTING_LIMIT)).is_ok());
}

/// 65 repeated prefix operators are rejected at the 65th operator.
#[test]
fn nesting_limit_prefix_operators_rejects_65() {
    let src = nested_prefix_operators_source(NESTING_LIMIT + 1);
    assert_error(
        &src,
        NESTING_ERROR,
        expr_span(NESTING_LIMIT, NESTING_LIMIT + 1),
    );
}

/// Exactly 64 nested calls parse.
#[test]
fn nesting_limit_calls_accepts_64() {
    assert!(parse_source(&nested_calls_source(NESTING_LIMIT)).is_ok());
}

/// 65 nested calls are rejected at the name of the 65th call (each level is `f(`, 2 bytes).
#[test]
fn nesting_limit_calls_rejects_65() {
    let src = nested_calls_source(NESTING_LIMIT + 1);
    let start = 2 * NESTING_LIMIT;
    assert_error(&src, NESTING_ERROR, expr_span(start, start + 1));
}

/// Exactly 64 nested `if` statements parse.
#[test]
fn nesting_limit_ifs_accepts_64() {
    assert!(parse_source(&nested_ifs_source(NESTING_LIMIT)).is_ok());
}

/// 65 nested `if` statements are rejected at the 65th `if`.
#[test]
fn nesting_limit_ifs_rejects_65() {
    let src = nested_ifs_source(NESTING_LIMIT + 1);
    assert_error(&src, NESTING_ERROR, nth_span(&src, "if", NESTING_LIMIT));
}

/// An `if` with 63 `else if` links (64 levels) parses.
#[test]
fn nesting_limit_else_if_chain_accepts_64() {
    assert!(parse_source(&else_if_chain_source(NESTING_LIMIT)).is_ok());
}

/// An `if` with 64 `else if` links (65 levels) is rejected at the 65th `if`.
#[test]
fn nesting_limit_else_if_chain_rejects_65() {
    let src = else_if_chain_source(NESTING_LIMIT + 1);
    assert_error(&src, NESTING_ERROR, nth_span(&src, "if", NESTING_LIMIT));
}

/// Exactly 64 nested `for` loops parse.
#[test]
fn nesting_limit_fors_accepts_64() {
    assert!(parse_source(&nested_fors_source(NESTING_LIMIT)).is_ok());
}

/// 65 nested `for` loops are rejected at the 65th `for`.
#[test]
fn nesting_limit_fors_rejects_65() {
    let src = nested_fors_source(NESTING_LIMIT + 1);
    assert_error(&src, NESTING_ERROR, nth_span(&src, "for", NESTING_LIMIT));
}

// ---------------------------------------------------------------- amendment: expression size limit

/// The most expression nodes one full expression may have.
const EXPR_NODE_LIMIT: usize = 1000;

/// The error for an expression with more nodes than the limit.
const SIZE_ERROR: &str = "expression is too large (limit 1000 nodes)";

// Node counting: a literal is 1 node, each binary operator 1 node, each prefix operator 1 node,
// each call 1 node, and parentheses none. `sum_chain(prefix, terms)` is `prefix 1 +1 +1 ...` with
// `terms` literals, `terms - 1` binary nodes and one node per prefix `-`. So it has
// `prefix.len() + 2 * terms - 1` nodes, which is odd without a prefix; one prefix `-` makes the
// count even (500 terms: 1000 nodes) and two prefixes make it 1001 again.

/// Builds `prefix` followed by `terms` ones joined with `+`.
fn sum_chain(prefix: &str, terms: usize) -> String {
    format!("{prefix}1{}", "+1".repeat(terms - 1))
}

/// The span of the newline that ends an expression of `len` bytes made by `expr_source`.
fn newline_after_expr(len: usize) -> Span {
    expr_span(len, len + 1)
}

/// A flat expression with 999 nodes parses.
#[test]
fn expression_with_999_nodes_is_accepted() {
    assert!(parse_source(&expr_source(&sum_chain("", 500))).is_ok());
}

/// A flat expression with exactly 1000 nodes parses.
#[test]
fn expression_with_exactly_1000_nodes_is_accepted() {
    assert!(parse_source(&expr_source(&sum_chain("-", 500))).is_ok());
}

/// A flat expression with 1001 nodes is rejected at the token after it.
#[test]
fn expression_with_1001_nodes_is_rejected() {
    let expr = sum_chain("--", 500);
    assert_error(
        &expr_source(&expr),
        SIZE_ERROR,
        newline_after_expr(expr.len()),
    );
}

/// The limit is the number of nodes, not the number of terms: 501 plain terms have 1001 nodes.
#[test]
fn expression_with_501_terms_is_rejected() {
    let expr = sum_chain("", 501);
    assert_error(
        &expr_source(&expr),
        SIZE_ERROR,
        newline_after_expr(expr.len()),
    );
}

/// A call's node and its whole argument list count together: 1 + 999 = 1000 nodes parse.
#[test]
fn call_with_1000_nodes_in_total_is_accepted() {
    let expr = format!("f({})", sum_chain("", 500));
    assert!(parse_source(&expr_source(&expr)).is_ok());
}

/// A call's node and its argument list count together: 1 + 1000 = 1001 nodes are rejected.
#[test]
fn call_with_1001_nodes_in_total_is_rejected() {
    let expr = format!("f({})", sum_chain("-", 500));
    assert_error(
        &expr_source(&expr),
        SIZE_ERROR,
        newline_after_expr(expr.len()),
    );
}

/// The body of a one-line function is one full expression, so 1001 nodes are rejected there.
#[test]
fn one_line_function_body_with_1001_nodes_is_rejected() {
    let src = format!("fn f() -> i64 = {}\n", sum_chain("--", 500));
    let newline = Span::new(src.len() - 1, src.len());
    assert_error(&src, SIZE_ERROR, newline);
}

/// The body of a one-line function with exactly 1000 nodes parses.
#[test]
fn one_line_function_body_with_1000_nodes_is_accepted() {
    let src = format!("fn f() -> i64 = {}\n", sum_chain("-", 500));
    assert!(parse_source(&src).is_ok());
}

/// Each statement starts its own count: two statements of 900 nodes each parse.
#[test]
fn two_statements_of_900_nodes_are_accepted() {
    let chain = sum_chain("-", 450);
    let src = main_source(&format!("x := {chain}\ny := {chain}"));
    assert!(parse_source(&src).is_ok());
}

/// The size limit constant matches the number of nodes the tests build.
#[test]
fn size_limit_tests_build_exactly_the_limit() {
    let nodes = "-".len() + 500 + 499;
    assert_eq!(nodes, EXPR_NODE_LIMIT);
}

// ---------------------------------------------------------------- amendment: statement spans

/// A parenthesized expression statement spans the parentheses.
#[test]
fn parenthesized_expression_statement_span_includes_the_parentheses() {
    let src = main_source("(a + b)");
    assert_eq!(first_stmt(&src).span, span_of(&src, "(a + b)"));
}

/// A parenthesized expression statement prints as the inner expression.
#[test]
fn parenthesized_expression_statement_prints_the_inner_expression() {
    let expected = lines(&["fn main()", "  (+ a b)"]);
    assert_eq!(printed_main("(a + b)"), expected);
}

/// `(x) = 1` is an assignment to `x`.
#[test]
fn parenthesized_assignment_target_is_accepted() {
    let expected = lines(&["fn main()", "  x = 1"]);
    assert_eq!(printed_main("(x) = 1"), expected);
}

/// An assignment to a parenthesized name spans from the `(` to the end of the value.
#[test]
fn parenthesized_assignment_span_includes_the_parentheses() {
    let src = main_source("(x) = 1");
    assert_eq!(first_stmt(&src).span, span_of(&src, "(x) = 1"));
}

/// An assignment to a parenthesized non-name is still an error at the left side.
#[test]
fn cannot_assign_to_a_parenthesized_expression() {
    let src = main_source("(a + b) = 1");
    assert_error(
        &src,
        "cannot assign to this expression",
        span_of(&src, "a + b"),
    );
}

// ---------------------------------------------------------------- amendment: multi-line lists

/// A call continued over lines without a trailing comma fails at the newline after the last argument.
#[test]
fn multi_line_call_without_trailing_comma_is_an_error() {
    let src = main_source("f(a,\n b\n)");
    let newline = src.find("b\n").unwrap() + 1;
    assert_error(
        &src,
        "expected `)`, found newline",
        Span::new(newline, newline + 1),
    );
}

/// A call continued over lines with a trailing comma parses.
#[test]
fn multi_line_call_with_trailing_comma_parses() {
    let expected = lines(&["fn main()", "  (call f a b)"]);
    assert_eq!(printed_main("f(a,\n b,\n)"), expected);
}

// ---------------------------------------------------------------- amendment: token slices

/// An empty token slice parses to an empty program.
#[test]
fn parse_of_no_tokens_is_an_empty_program() {
    assert_eq!(parse(&[]).unwrap(), Program { functions: vec![] });
}

/// Tokens without the final `Eof` parse like tokens with it.
#[test]
fn parse_without_trailing_eof_equals_parse_with_it() {
    let src = "fn main() {\n    x := 1 + 2\n}\nfn f() -> i64 = 1\n";
    let mut tokens = lexer::lex(src).unwrap();
    let with_eof = parse(&tokens);
    tokens.pop();
    assert_eq!(parse(&tokens), with_eof);
}

/// Tokens without the final `Eof` give the same error as tokens with it.
#[test]
fn parse_without_trailing_eof_gives_the_same_error() {
    let src = "fn main() {\n    x := 1\n";
    let mut tokens = lexer::lex(src).unwrap();
    let with_eof = parse(&tokens);
    tokens.pop();
    assert!(with_eof.is_err());
    assert_eq!(parse(&tokens), with_eof);
}

// ---------------------------------------------------------------- second amendment: one-line function spans

/// A one-line function whose body starts with `(` spans through the closing `)`.
#[test]
fn one_line_function_with_parenthesized_body_span_includes_the_closing_parenthesis() {
    let src = "fn main() = (a + b)";
    assert_eq!(first_function(src).span, Span::new(0, src.len()));
}

/// The body block of a one-line function with a parenthesized body spans the parentheses.
#[test]
fn one_line_function_with_parenthesized_body_has_the_parenthesized_body_span() {
    let src = "fn main() = (a + b)";
    assert_eq!(first_function(src).body.span, span_of(src, "(a + b)"));
}
