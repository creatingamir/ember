//! Integration tests for how the parser groups the bitwise operators, the shifts and `T(x)` conversion calls.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use ast::{Expr, ExprKind, Program, StmtKind, UnaryOp, print_program};
use parser::parse_source;
use source::{Diagnostic, Span};

/// The text that comes before the expression in a source built by `expr_source`.
const EXPR_PREFIX: &str = "fn main() {\n    x := ";

/// The operators of the multiplication level, which includes the shifts and bitwise and.
const MULTIPLICATION_LEVEL: [&str; 6] = ["*", "/", "%", "<<", ">>", "&"];

/// The operators of the addition level, which includes bitwise or and xor.
const ADDITION_LEVEL: [&str; 4] = ["+", "-", "|", "^"];

/// The comparison operators.
const COMPARISONS: [&str; 6] = ["==", "!=", "<", "<=", ">", ">="];

/// Parses source that must be valid.
fn program(src: &str) -> Program {
    parse_source(src).unwrap()
}

/// Wraps an expression as the value of `x := ...` inside `fn main`.
fn expr_source(expr: &str) -> String {
    format!("{EXPR_PREFIX}{expr}\n}}\n")
}

/// Parses an expression and returns it printed in prefix form.
fn printed_expr(expr: &str) -> String {
    let out = print_program(&program(&expr_source(expr)));
    let rest = out.strip_prefix("fn main()\n  let x = ").unwrap();
    rest.strip_suffix('\n').unwrap().to_string()
}

/// Parses an expression and returns its AST node.
fn parsed_expr(expr: &str) -> Expr {
    let stmt = program(&expr_source(expr))
        .functions
        .remove(0)
        .body
        .stmts
        .remove(0);
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

/// Splits a binary expression into its left and right operands.
fn binary_operands(expr: Expr) -> (Expr, Expr) {
    let operands = match expr.kind {
        ExprKind::Binary { left, right, .. } => Some((*left, *right)),
        _ => None,
    };
    operands.unwrap()
}

// ---------------------------------------------------------------- levels

/// Shift binds as tightly as multiplication, so a multiplication on its left groups first.
#[test]
fn shift_groups_with_multiplication_on_the_left() {
    assert_eq!(printed_expr("a * b << c"), "(<< (* a b) c)");
}

/// Shift and multiplication group left to right, so a shift on the left groups first.
#[test]
fn shift_groups_with_multiplication_on_the_right() {
    assert_eq!(printed_expr("a << b * c"), "(* (<< a b) c)");
}

/// Bitwise and shares the level of shifts, so `a & b << c` groups left to right.
#[test]
fn bitwise_and_and_shift_group_left_to_right() {
    assert_eq!(printed_expr("a & b << c"), "(<< (& a b) c)");
    assert_eq!(printed_expr("a << b & c"), "(& (<< a b) c)");
}

/// Bitwise or and xor share the level of addition, so they group left to right with it.
#[test]
fn bitwise_or_xor_and_addition_group_left_to_right() {
    assert_eq!(printed_expr("a | b + c"), "(+ (| a b) c)");
    assert_eq!(printed_expr("a + b ^ c"), "(^ (+ a b) c)");
}

/// Bitwise or and xor share one level, so `a | b ^ c` groups left to right.
#[test]
fn bitwise_or_and_xor_group_left_to_right() {
    assert_eq!(printed_expr("a | b ^ c"), "(^ (| a b) c)");
    assert_eq!(printed_expr("a ^ b | c"), "(| (^ a b) c)");
}

/// Every operator of the multiplication level binds tighter than every operator of the addition level.
#[test]
fn multiplication_level_binds_tighter_than_addition_level() {
    for high in MULTIPLICATION_LEVEL {
        for low in ADDITION_LEVEL {
            let high_right = printed_expr(&format!("a {low} b {high} c"));
            assert_eq!(high_right, format!("({low} a ({high} b c))"));
            let high_left = printed_expr(&format!("a {high} b {low} c"));
            assert_eq!(high_left, format!("({low} ({high} a b) c)"));
        }
    }
}

/// Every operator of the addition level binds tighter than every comparison.
#[test]
fn addition_level_binds_tighter_than_comparisons() {
    for high in ADDITION_LEVEL {
        for low in COMPARISONS {
            let high_right = printed_expr(&format!("a {low} b {high} c"));
            assert_eq!(high_right, format!("({low} a ({high} b c))"));
            let high_left = printed_expr(&format!("a {high} b {low} c"));
            assert_eq!(high_left, format!("({low} ({high} a b) c)"));
        }
    }
}

/// Every operator of the multiplication level binds tighter than every comparison.
#[test]
fn multiplication_level_binds_tighter_than_comparisons() {
    for high in MULTIPLICATION_LEVEL {
        for low in COMPARISONS {
            let high_left = printed_expr(&format!("a {high} b {low} c"));
            assert_eq!(high_left, format!("({low} ({high} a b) c)"));
        }
    }
}

/// `a & b == c` is `(a & b) == c`: bitwise and binds tighter than comparison.
#[test]
fn bitwise_and_binds_tighter_than_equality() {
    assert_eq!(printed_expr("a & b == c"), "(== (& a b) c)");
    assert_eq!(printed_expr("a == b & c"), "(== a (& b c))");
}

/// `a | b != c` is `(a | b) != c`.
#[test]
fn bitwise_or_binds_tighter_than_inequality() {
    assert_eq!(printed_expr("a | b != c"), "(!= (| a b) c)");
}

/// A shift binds tighter than `<`.
#[test]
fn shift_binds_tighter_than_less_than() {
    assert_eq!(printed_expr("a << b < c"), "(< (<< a b) c)");
    assert_eq!(printed_expr("a < b >> c"), "(< a (>> b c))");
}

/// The bitwise operators bind tighter than a range.
#[test]
fn bitwise_operators_bind_tighter_than_range() {
    assert_eq!(printed_expr("a..b | c"), "(.. a (| b c))");
    assert_eq!(printed_expr("a & b..c"), "(.. (& a b) c)");
}

/// The bitwise operators bind tighter than `&&` and `||`.
#[test]
fn bitwise_operators_bind_tighter_than_logical_operators() {
    assert_eq!(printed_expr("a | b && c"), "(&& (| a b) c)");
    assert_eq!(printed_expr("a && b | c"), "(&& a (| b c))");
    assert_eq!(printed_expr("a || b ^ c"), "(|| a (^ b c))");
    assert_eq!(printed_expr("a & b || c"), "(|| (& a b) c)");
}

/// `&&` and `&` are different operators at different levels.
#[test]
fn logical_and_is_looser_than_bitwise_and() {
    assert_eq!(printed_expr("a && b & c"), "(&& a (& b c))");
    assert_eq!(printed_expr("a & b && c"), "(&& (& a b) c)");
}

// ---------------------------------------------------------------- associativity

/// Every pair of operators of the multiplication level groups left to right.
#[test]
fn multiplication_level_groups_left_to_right() {
    for first in MULTIPLICATION_LEVEL {
        for second in MULTIPLICATION_LEVEL {
            let printed = printed_expr(&format!("a {first} b {second} c"));
            assert_eq!(printed, format!("({second} ({first} a b) c)"));
        }
    }
}

/// Every pair of operators of the addition level groups left to right.
#[test]
fn addition_level_groups_left_to_right() {
    for first in ADDITION_LEVEL {
        for second in ADDITION_LEVEL {
            let printed = printed_expr(&format!("a {first} b {second} c"));
            assert_eq!(printed, format!("({second} ({first} a b) c)"));
        }
    }
}

/// Shifts are left associative.
#[test]
fn shifts_are_left_associative() {
    assert_eq!(printed_expr("a << b << c"), "(<< (<< a b) c)");
    assert_eq!(printed_expr("a >> b >> c"), "(>> (>> a b) c)");
    assert_eq!(printed_expr("a << b >> c"), "(>> (<< a b) c)");
}

/// Parentheses change the grouping of shifts.
#[test]
fn parentheses_override_shift_associativity() {
    assert_eq!(printed_expr("a << (b << c)"), "(<< a (<< b c))");
}

// ---------------------------------------------------------------- unary `~`

/// `~` binds tighter than every binary operator on its right.
#[test]
fn tilde_binds_tighter_than_binary_operators() {
    for op in MULTIPLICATION_LEVEL.iter().chain(ADDITION_LEVEL.iter()) {
        let printed = printed_expr(&format!("~a {op} b"));
        assert_eq!(printed, format!("({op} (~ a) b)"), "op `{op}`");
    }
}

/// `~` binds tighter than every binary operator on its left, so it applies to the right operand only.
#[test]
fn tilde_applies_to_the_right_operand_only() {
    for op in MULTIPLICATION_LEVEL.iter().chain(ADDITION_LEVEL.iter()) {
        let printed = printed_expr(&format!("a {op} ~b"));
        assert_eq!(printed, format!("({op} a (~ b))"), "op `{op}`");
    }
}

/// `~` can repeat and mix with the other unary operators.
#[test]
fn tilde_stacks_with_other_unary_operators() {
    assert_eq!(printed_expr("~~a"), "(~ (~ a))");
    assert_eq!(printed_expr("~-a"), "(~ (- a))");
    assert_eq!(printed_expr("-~a"), "(- (~ a))");
    assert_eq!(printed_expr("!~a"), "(! (~ a))");
}

/// Parentheses let `~` apply to a whole expression.
#[test]
fn tilde_of_a_parenthesized_expression() {
    assert_eq!(printed_expr("~(a & b)"), "(~ (& a b))");
}

/// `~` is the bitwise-not unary operator in the tree.
#[test]
fn tilde_is_the_bitwise_not_node() {
    let expr = parsed_expr("~a");
    let op = match expr.kind {
        ExprKind::Unary { op, .. } => Some(op),
        _ => None,
    };
    assert_eq!(op, Some(UnaryOp::BitNot));
}

/// `~a & b | ~c << d` groups each `~` with its own operand and the rest by level.
#[test]
fn tilde_in_a_long_expression() {
    assert_eq!(
        printed_expr("~a & b | ~c << d"),
        "(| (& (~ a) b) (<< (~ c) d))"
    );
}

// ---------------------------------------------------------------- parentheses

/// Parentheses override the levels of the bitwise operators.
#[test]
fn parentheses_override_bitwise_precedence() {
    assert_eq!(printed_expr("(a | b) & c"), "(& (| a b) c)");
    assert_eq!(printed_expr("a << (b + c)"), "(<< a (+ b c))");
    assert_eq!(printed_expr("(a ^ b) << c"), "(<< (^ a b) c)");
}

// ---------------------------------------------------------------- spans

/// A shift spans from its left operand to its amount.
#[test]
fn shift_span_covers_both_operands() {
    let expr = parsed_expr("abc << de");
    assert_eq!(expr.span, expr_span(0, 9));
    let (left, right) = binary_operands(expr);
    assert_eq!(left.span, expr_span(0, 3));
    assert_eq!(right.span, expr_span(7, 9));
}

/// A bitwise `&` spans from its left operand to its right operand.
#[test]
fn bitwise_and_span_covers_both_operands() {
    assert_eq!(parsed_expr("a & b").span, expr_span(0, 5));
}

/// `~a` spans from the tilde to the end of its operand.
#[test]
fn tilde_span_covers_operator_and_operand() {
    assert_eq!(parsed_expr("~abc").span, expr_span(0, 4));
}

/// A parenthesized left operand keeps its parentheses, so the shift spans them too.
#[test]
fn shift_of_a_parenthesized_operand_spans_the_parentheses() {
    let expr = parsed_expr("(a | b) << c");
    assert_eq!(expr.span, expr_span(0, 12));
    let (left, _) = binary_operands(expr);
    assert_eq!(left.span, expr_span(0, 7));
}

// ---------------------------------------------------------------- conversion calls

/// `u8(x)` is an ordinary call whose callee is the type name.
#[test]
fn conversion_parses_as_a_call() {
    assert_eq!(printed_expr("u8(x)"), "(call u8 x)");
}

/// Every integer type name parses as a callee.
#[test]
fn every_integer_name_parses_as_a_callee() {
    for name in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        assert_eq!(
            printed_expr(&format!("{name}(x)")),
            format!("(call {name} x)")
        );
    }
}

/// A conversion of a bitwise expression takes the whole expression as its argument.
#[test]
fn conversion_of_a_masked_value() {
    assert_eq!(printed_expr("u8(x & 255)"), "(call u8 (& x 255))");
}

/// A conversion is a primary expression, so a binary operator applies to its result.
#[test]
fn conversion_result_is_an_operand() {
    assert_eq!(printed_expr("u8(x) << 1"), "(<< (call u8 x) 1)");
    assert_eq!(printed_expr("~u8(x)"), "(~ (call u8 x))");
}

/// A conversion spans from the type name to the closing parenthesis.
#[test]
fn conversion_span_covers_the_name_and_arguments() {
    assert_eq!(parsed_expr("u16(x)").span, expr_span(0, 6));
}

// ---------------------------------------------------------------- no compound bitwise assignment

/// `&=`, `|=`, `^=`, `<<=` and `>>=` are not statements: the error is at the `=`, as no expression follows the operator.
#[test]
fn compound_bitwise_assignments_are_rejected_at_the_equals_sign() {
    for op in ["&=", "|=", "^=", "<<=", ">>="] {
        let src = format!("fn main() {{\n    x := 1\n    x {op} 1\n}}\n");
        let operator_start = src.find(op).unwrap();
        let equals = operator_start + op.len() - 1;
        let expected = Diagnostic::error(
            "expected expression, found `=`",
            Span::new(equals, equals + 1),
        );
        assert_eq!(parse_source(&src).unwrap_err(), expected, "op `{op}`");
    }
}
