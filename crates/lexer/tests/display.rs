//! Tests for how `TokenKind` is described in parser error messages.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use lexer::TokenKind;

/// Every keyword token kind with its spelling.
fn keyword_table() -> Vec<(&'static str, TokenKind)> {
    vec![
        ("fn", TokenKind::KwFn),
        ("struct", TokenKind::KwStruct),
        ("enum", TokenKind::KwEnum),
        ("const", TokenKind::KwConst),
        ("pub", TokenKind::KwPub),
        ("import", TokenKind::KwImport),
        ("extern", TokenKind::KwExtern),
        ("if", TokenKind::KwIf),
        ("else", TokenKind::KwElse),
        ("for", TokenKind::KwFor),
        ("in", TokenKind::KwIn),
        ("break", TokenKind::KwBreak),
        ("continue", TokenKind::KwContinue),
        ("match", TokenKind::KwMatch),
        ("return", TokenKind::KwReturn),
        ("defer", TokenKind::KwDefer),
        ("arena", TokenKind::KwArena),
        ("grad", TokenKind::KwGrad),
        ("on", TokenKind::KwOn),
        ("test", TokenKind::KwTest),
        ("true", TokenKind::KwTrue),
        ("false", TokenKind::KwFalse),
        ("none", TokenKind::KwNone),
    ]
}

/// Every delimiter, assignment and operator token kind with its spelling.
fn punctuation_table() -> Vec<(&'static str, TokenKind)> {
    vec![
        ("(", TokenKind::LParen),
        (")", TokenKind::RParen),
        ("[", TokenKind::LBracket),
        ("]", TokenKind::RBracket),
        ("{", TokenKind::LBrace),
        ("}", TokenKind::RBrace),
        (",", TokenKind::Comma),
        (":", TokenKind::Colon),
        (".", TokenKind::Dot),
        ("->", TokenKind::Arrow),
        ("=>", TokenKind::FatArrow),
        ("?", TokenKind::Question),
        ("=", TokenKind::Assign),
        (":=", TokenKind::ColonAssign),
        ("+=", TokenKind::PlusAssign),
        ("-=", TokenKind::MinusAssign),
        ("*=", TokenKind::StarAssign),
        ("/=", TokenKind::SlashAssign),
        ("@=", TokenKind::AtAssign),
        ("+", TokenKind::Plus),
        ("-", TokenKind::Minus),
        ("*", TokenKind::Star),
        ("/", TokenKind::Slash),
        ("%", TokenKind::Percent),
        ("@", TokenKind::At),
        ("!", TokenKind::Bang),
        ("~", TokenKind::Tilde),
        ("&", TokenKind::Amp),
        ("|", TokenKind::Pipe),
        ("^", TokenKind::Caret),
        ("<<", TokenKind::Shl),
        (">>", TokenKind::Shr),
        ("==", TokenKind::EqEq),
        ("!=", TokenKind::NotEq),
        ("<", TokenKind::Lt),
        ("<=", TokenKind::LtEq),
        (">", TokenKind::Gt),
        (">=", TokenKind::GtEq),
        ("&&", TokenKind::AndAnd),
        ("||", TokenKind::OrOr),
        ("..", TokenKind::DotDot),
        ("..=", TokenKind::DotDotEq),
    ]
}

/// The keyword table holds all 23 keywords.
#[test]
fn keyword_table_has_23_entries() {
    assert_eq!(keyword_table().len(), 23);
}

/// The punctuation table holds all 42 delimiter, assignment and operator kinds.
#[test]
fn punctuation_table_has_42_entries() {
    assert_eq!(punctuation_table().len(), 42);
}

/// Every keyword is described by its spelling in backticks.
#[test]
fn keyword_is_described_by_its_spelling_in_backticks() {
    for (spelling, kind) in keyword_table() {
        assert_eq!(kind.to_string(), format!("`{spelling}`"));
    }
}

/// Every delimiter, assignment and operator is described by its spelling in backticks.
#[test]
fn punctuation_is_described_by_its_spelling_in_backticks() {
    for (spelling, kind) in punctuation_table() {
        assert_eq!(kind.to_string(), format!("`{spelling}`"));
    }
}

/// The `:=` token is described as `` `:=` ``.
#[test]
fn colon_assign_is_described_as_backticked_colon_equals() {
    assert_eq!(TokenKind::ColonAssign.to_string(), "`:=`");
}

/// The `..=` token is described as `` `..=` ``.
#[test]
fn dot_dot_eq_is_described_as_backticked_dot_dot_equals() {
    assert_eq!(TokenKind::DotDotEq.to_string(), "`..=`");
}

/// An identifier is described together with its name.
#[test]
fn identifier_is_described_with_its_name() {
    let kind = TokenKind::Ident("count".to_string());
    assert_eq!(kind.to_string(), "identifier `count`");
}

/// An identifier named like a type or with underscores keeps its exact name.
#[test]
fn identifier_description_keeps_the_exact_name() {
    let kind = TokenKind::Ident("_my_Var2".to_string());
    assert_eq!(kind.to_string(), "identifier `_my_Var2`");
}

/// An integer literal is described without its value.
#[test]
fn integer_is_described_as_integer_literal() {
    assert_eq!(TokenKind::Int(42).to_string(), "integer literal");
}

/// The largest integer is described the same as any other integer.
#[test]
fn large_integer_is_described_as_integer_literal() {
    assert_eq!(TokenKind::Int(u64::MAX).to_string(), "integer literal");
}

/// A float literal is described without its value.
#[test]
fn float_is_described_as_float_literal() {
    assert_eq!(TokenKind::Float(2.5).to_string(), "float literal");
}

/// A string literal is described without its contents.
#[test]
fn string_is_described_as_string_literal() {
    let kind = TokenKind::Str("hello".to_string());
    assert_eq!(kind.to_string(), "string literal");
}

/// A newline token is described as `newline`, without backticks.
#[test]
fn newline_is_described_as_newline() {
    assert_eq!(TokenKind::Newline.to_string(), "newline");
}

/// The end-of-file token is described as `end of file`, without backticks.
#[test]
fn eof_is_described_as_end_of_file() {
    assert_eq!(TokenKind::Eof.to_string(), "end of file");
}
