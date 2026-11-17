//! Integration tests for the tokens of the bitwise operators and shifts: longest match and spans.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use lexer::{Token, TokenKind, lex};
use source::Span;

/// Lexes text that must be valid and returns only the token kinds.
fn kinds(text: &str) -> Vec<TokenKind> {
    lex(text)
        .unwrap()
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

/// Lexes text that must be valid and returns all its tokens.
fn tokens(text: &str) -> Vec<Token> {
    lex(text).unwrap()
}

/// Builds an identifier token kind.
fn ident(name: &str) -> TokenKind {
    TokenKind::Ident(name.to_string())
}

/// The kinds that end a line of text whose last token is a name: a newline, then the end.
fn line_end() -> [TokenKind; 2] {
    [TokenKind::Newline, TokenKind::Eof]
}

/// `~` is one `Tilde` token that spans its character.
#[test]
fn tilde_is_one_token() {
    let expected = Token {
        kind: TokenKind::Tilde,
        span: Span::new(0, 1),
    };
    assert_eq!(tokens("~")[0], expected);
}

/// `^` is one `Caret` token that spans its character.
#[test]
fn caret_is_one_token() {
    let expected = Token {
        kind: TokenKind::Caret,
        span: Span::new(0, 1),
    };
    assert_eq!(tokens("^")[0], expected);
}

/// `|` alone is one `Pipe` token.
#[test]
fn lone_pipe_is_one_token() {
    assert_eq!(kinds("|"), [TokenKind::Pipe, TokenKind::Eof]);
}

/// `||` is one `OrOr` token, not two `Pipe`.
#[test]
fn longest_match_or_or() {
    assert_eq!(kinds("||"), [TokenKind::OrOr, TokenKind::Eof]);
}

/// `|` followed by a space and `|` is two `Pipe` tokens.
#[test]
fn separated_pipes_are_two_pipe_tokens() {
    let expected = [TokenKind::Pipe, TokenKind::Pipe, TokenKind::Eof];
    assert_eq!(kinds("| |"), expected);
}

/// `>>` is one `Shr` token that spans both characters, not two `Gt`.
#[test]
fn longest_match_shr() {
    let expected = Token {
        kind: TokenKind::Shr,
        span: Span::new(0, 2),
    };
    assert_eq!(tokens(">>")[0], expected);
    assert_eq!(kinds(">>"), [TokenKind::Shr, TokenKind::Eof]);
}

/// `<<` is one `Shl` token that spans both characters.
#[test]
fn shl_spans_both_characters() {
    let expected = Token {
        kind: TokenKind::Shl,
        span: Span::new(0, 2),
    };
    assert_eq!(tokens("<<")[0], expected);
}

/// `> >` is two `Gt` tokens.
#[test]
fn separated_greater_than_signs_are_two_gt_tokens() {
    let expected = [TokenKind::Gt, TokenKind::Gt, TokenKind::Eof];
    assert_eq!(kinds("> >"), expected);
}

/// `< <` is two `Lt` tokens.
#[test]
fn separated_less_than_signs_are_two_lt_tokens() {
    let expected = [TokenKind::Lt, TokenKind::Lt, TokenKind::Eof];
    assert_eq!(kinds("< <"), expected);
}

/// `<<<` is `Shl` then `Lt`: the longest match is taken from the left.
#[test]
fn three_less_than_signs_are_shl_then_lt() {
    let expected = [TokenKind::Shl, TokenKind::Lt, TokenKind::Eof];
    assert_eq!(kinds("<<<"), expected);
}

/// `>>>` is `Shr` then `Gt`.
#[test]
fn three_greater_than_signs_are_shr_then_gt() {
    let expected = [TokenKind::Shr, TokenKind::Gt, TokenKind::Eof];
    assert_eq!(kinds(">>>"), expected);
}

/// `>=` stays one `GtEq` token, so `>>=` is `Shr` then `Assign`: there is no compound shift.
#[test]
fn shr_assign_is_shr_then_assign() {
    let expected = [TokenKind::Shr, TokenKind::Assign, TokenKind::Eof];
    assert_eq!(kinds(">>="), expected);
}

/// `<<=` is `Shl` then `Assign`: there is no compound shift.
#[test]
fn shl_assign_is_shl_then_assign() {
    let expected = [TokenKind::Shl, TokenKind::Assign, TokenKind::Eof];
    assert_eq!(kinds("<<="), expected);
}

/// `&=` is `Amp` then `Assign`: there is no compound bitwise assignment.
#[test]
fn amp_assign_is_amp_then_assign() {
    let expected = [TokenKind::Amp, TokenKind::Assign, TokenKind::Eof];
    assert_eq!(kinds("&="), expected);
}

/// `|=` is `Pipe` then `Assign`.
#[test]
fn pipe_assign_is_pipe_then_assign() {
    let expected = [TokenKind::Pipe, TokenKind::Assign, TokenKind::Eof];
    assert_eq!(kinds("|="), expected);
}

/// `^=` is `Caret` then `Assign`.
#[test]
fn caret_assign_is_caret_then_assign() {
    let expected = [TokenKind::Caret, TokenKind::Assign, TokenKind::Eof];
    assert_eq!(kinds("^="), expected);
}

/// `&&&` is `AndAnd` then `Amp`.
#[test]
fn three_ampersands_are_and_and_then_amp() {
    let expected = [TokenKind::AndAnd, TokenKind::Amp, TokenKind::Eof];
    assert_eq!(kinds("&&&"), expected);
}

/// `a<<b` lexes without spaces around the shift.
#[test]
fn shl_between_operands_without_spaces() {
    let expected = [ident("a"), TokenKind::Shl, ident("b")];
    assert_eq!(kinds("a<<b")[..3], expected);
    assert_eq!(kinds("a<<b")[3..], line_end());
}

/// `a>>b` lexes without spaces around the shift.
#[test]
fn shr_between_operands_without_spaces() {
    let expected = [ident("a"), TokenKind::Shr, ident("b")];
    assert_eq!(kinds("a>>b")[..3], expected);
    assert_eq!(kinds("a>>b")[3..], line_end());
}

/// `~a` lexes as `Tilde` then a name.
#[test]
fn tilde_before_a_name() {
    let expected = [TokenKind::Tilde, ident("a")];
    assert_eq!(kinds("~a")[..2], expected);
}

/// `a&b|c^d` lexes without spaces between the bitwise operators.
#[test]
fn bitwise_operators_without_spaces() {
    let expected = [
        ident("a"),
        TokenKind::Amp,
        ident("b"),
        TokenKind::Pipe,
        ident("c"),
        TokenKind::Caret,
        ident("d"),
    ];
    assert_eq!(kinds("a&b|c^d")[..7], expected);
    assert_eq!(kinds("a&b|c^d")[7..], line_end());
}

/// The spans of `x << 3` cover each token's own text.
#[test]
fn shift_expression_spans() {
    let found = tokens("x << 3");
    assert_eq!(found[0].span, Span::new(0, 1));
    assert_eq!(found[1].span, Span::new(2, 4));
    assert_eq!(found[2].span, Span::new(5, 6));
}

/// A hexadecimal mask next to `&` lexes as an integer and an operator.
#[test]
fn hex_mask_after_amp() {
    let expected = [ident("x"), TokenKind::Amp, TokenKind::Int(255)];
    assert_eq!(kinds("x & 0xFF")[..3], expected);
}
