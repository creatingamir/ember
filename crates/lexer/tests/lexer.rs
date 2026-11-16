//! Integration tests for the public API of the `lexer` crate.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use lexer::{Token, TokenKind, lex};
use source::{Diagnostic, Span};

// ---------------------------------------------------------------- helpers

/// Lexes text that must be valid and returns all its tokens.
fn tokens(text: &str) -> Vec<Token> {
    lex(text).unwrap()
}

/// Lexes text that must be valid and returns only the token kinds.
fn kinds(text: &str) -> Vec<TokenKind> {
    tokens(text).into_iter().map(|token| token.kind).collect()
}

/// Lexes text that must be valid and returns its first token.
fn first_token(text: &str) -> Token {
    tokens(text).remove(0)
}

/// Builds an identifier token kind.
fn ident(name: &str) -> TokenKind {
    TokenKind::Ident(name.to_string())
}

/// Builds a string token kind from its raw (undecoded) value.
fn string(value: &str) -> TokenKind {
    TokenKind::Str(value.to_string())
}

/// Builds a token from its kind and byte span.
fn token(kind: TokenKind, start: usize, end: usize) -> Token {
    Token {
        kind,
        span: Span::new(start, end),
    }
}

/// Builds the error the lexer is expected to return.
fn error(message: &str, start: usize, end: usize) -> Diagnostic {
    Diagnostic::error(message, Span::new(start, end))
}

/// Every keyword spelling with its token kind (the 23 keywords of the spec).
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

/// Every delimiter, assignment and operator spelling with its token kind.
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

/// Token spellings after which a `\n` produces a `Newline` (decision 0015).
fn statement_ending_table() -> Vec<(&'static str, TokenKind)> {
    vec![
        ("x", ident("x")),
        ("42", TokenKind::Int(42)),
        ("1.5", TokenKind::Float(1.5)),
        ("\"s\"", string("s")),
        ("true", TokenKind::KwTrue),
        ("false", TokenKind::KwFalse),
        ("none", TokenKind::KwNone),
        (")", TokenKind::RParen),
        ("]", TokenKind::RBracket),
        ("}", TokenKind::RBrace),
        ("?", TokenKind::Question),
        ("return", TokenKind::KwReturn),
        ("break", TokenKind::KwBreak),
        ("continue", TokenKind::KwContinue),
    ]
}

/// Every keyword and punctuation spelling after which a `\n` is skipped.
fn statement_continuing_table() -> Vec<(&'static str, TokenKind)> {
    let ending: Vec<TokenKind> = statement_ending_table()
        .into_iter()
        .map(|(_, kind)| kind)
        .collect();
    let mut table = keyword_table();
    table.extend(punctuation_table());
    table.retain(|(_, kind)| !ending.contains(kind));
    table
}

// ---------------------------------------------------------------- keywords

/// The keyword table really holds the 23 keywords of the spec.
#[test]
fn keyword_table_has_23_entries() {
    assert_eq!(keyword_table().len(), 23);
}

/// Every keyword spelling lexes to its keyword token, spanning the whole word.
#[test]
fn every_keyword_lexes_to_its_token() {
    for (text, kind) in keyword_table() {
        let expected = token(kind, 0, text.len());
        assert_eq!(first_token(text), expected, "keyword `{text}`");
    }
}

/// Keywords are case-sensitive: a capitalized keyword is an identifier.
#[test]
fn capitalized_keyword_is_an_identifier() {
    assert_eq!(first_token("Fn").kind, ident("Fn"));
    assert_eq!(first_token("TRUE").kind, ident("TRUE"));
}

/// An identifier that merely starts with a keyword is an identifier.
#[test]
fn identifier_starting_with_a_keyword_is_an_identifier() {
    for text in ["fns", "iff", "format", "fn_", "returned", "none1", "in_"] {
        assert_eq!(first_token(text).kind, ident(text), "text `{text}`");
    }
}

// ---------------------------------------------------------------- identifiers

/// A plain lowercase identifier keeps its text and spans the whole word.
#[test]
fn identifier_keeps_its_text_and_span() {
    assert_eq!(first_token("abc"), token(ident("abc"), 0, 3));
}

/// A single `_` is an identifier.
#[test]
fn underscore_alone_is_an_identifier() {
    assert_eq!(first_token("_"), token(ident("_"), 0, 1));
}

/// An identifier may start with `_` and contain digits.
#[test]
fn identifier_may_start_with_underscore_and_contain_digits() {
    assert_eq!(first_token("_x1"), token(ident("_x1"), 0, 3));
}

/// An identifier may mix uppercase, digits and underscores.
#[test]
fn identifier_may_mix_case_digits_and_underscores() {
    assert_eq!(first_token("A_b9_C"), token(ident("A_b9_C"), 0, 6));
}

// ---------------------------------------------------------------- reserved words

/// Each reserved word is an error naming the word, spanning the word.
#[test]
fn every_reserved_word_is_an_error() {
    let words = ["interface", "async", "spawn", "unsafe", "macro", "yield"];
    for word in words {
        let message = format!("`{word}` is reserved for future use");
        let expected = Err(error(&message, 0, word.len()));
        assert_eq!(lex(word), expected, "reserved word `{word}`");
    }
}

/// The reserved-word error span starts where the word starts.
#[test]
fn reserved_word_error_span_is_offset_by_earlier_text() {
    let expected = Err(error("`spawn` is reserved for future use", 7, 12));
    assert_eq!(lex("x := 1 spawn"), expected);
}

/// An identifier that merely contains or starts with a reserved word is fine.
#[test]
fn identifier_starting_with_a_reserved_word_is_an_identifier() {
    assert_eq!(first_token("asyncio").kind, ident("asyncio"));
    assert_eq!(first_token("async_").kind, ident("async_"));
    assert_eq!(first_token("_async").kind, ident("_async"));
}

// ---------------------------------------------------------------- punctuation and operators

/// Every delimiter, assignment and operator lexes to its token, spanning its text.
#[test]
fn every_punctuation_token_lexes_to_its_kind() {
    assert_eq!(punctuation_table().len(), 42);
    for (text, kind) in punctuation_table() {
        let expected = token(kind, 0, text.len());
        assert_eq!(first_token(text), expected, "punctuation `{text}`");
    }
}

/// `<=` is one `LtEq` token, not `Lt` then `Assign`.
#[test]
fn longest_match_lt_eq() {
    assert_eq!(kinds("<="), [TokenKind::LtEq, TokenKind::Eof]);
}

/// `<<` is one `Shl` token, not two `Lt`.
#[test]
fn longest_match_shl() {
    assert_eq!(kinds("<<"), [TokenKind::Shl, TokenKind::Eof]);
}

/// `..=` is one `DotDotEq` token.
#[test]
fn longest_match_dot_dot_eq() {
    assert_eq!(kinds("..="), [TokenKind::DotDotEq, TokenKind::Eof]);
}

/// `..` is one `DotDot` token, not two `Dot`.
#[test]
fn longest_match_dot_dot() {
    assert_eq!(kinds(".."), [TokenKind::DotDot, TokenKind::Eof]);
}

/// `&&` is one `AndAnd` token, not two `Amp`.
#[test]
fn longest_match_and_and() {
    assert_eq!(kinds("&&"), [TokenKind::AndAnd, TokenKind::Eof]);
}

/// `&` followed by a space and `&` is two `Amp` tokens.
#[test]
fn separated_ampersands_are_two_amp_tokens() {
    let expected = [TokenKind::Amp, TokenKind::Amp, TokenKind::Eof];
    assert_eq!(kinds("& &"), expected);
}

/// `->` is one `Arrow` token, not `Minus` then `Gt`.
#[test]
fn longest_match_arrow() {
    assert_eq!(kinds("->"), [TokenKind::Arrow, TokenKind::Eof]);
}

/// `=>` is one `FatArrow` token, not `Assign` then `Gt`.
#[test]
fn longest_match_fat_arrow() {
    assert_eq!(kinds("=>"), [TokenKind::FatArrow, TokenKind::Eof]);
}

/// `:=` is one `ColonAssign` token, not `Colon` then `Assign`.
#[test]
fn longest_match_colon_assign() {
    assert_eq!(kinds(":="), [TokenKind::ColonAssign, TokenKind::Eof]);
}

/// `+=` is one `PlusAssign` token, not `Plus` then `Assign`.
#[test]
fn longest_match_plus_assign() {
    assert_eq!(kinds("+="), [TokenKind::PlusAssign, TokenKind::Eof]);
}

/// `@=` is one `AtAssign` token, not `At` then `Assign`.
#[test]
fn longest_match_at_assign() {
    assert_eq!(kinds("@="), [TokenKind::AtAssign, TokenKind::Eof]);
}

/// `==` is `EqEq` and `=>` is `FatArrow`, so `===` starts with `EqEq`.
#[test]
fn longest_match_eq_eq_then_assign() {
    let expected = [TokenKind::EqEq, TokenKind::Assign, TokenKind::Eof];
    assert_eq!(kinds("==="), expected);
}

/// Operators lex correctly when written without spaces between operands.
#[test]
fn operators_between_operands_without_spaces() {
    let expected = [
        ident("a"),
        TokenKind::LtEq,
        ident("b"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("a<=b"), expected);
}

// ---------------------------------------------------------------- integers

/// A decimal integer has its value and spans its digits.
#[test]
fn decimal_integer() {
    assert_eq!(first_token("42"), token(TokenKind::Int(42), 0, 2));
}

/// Zero is a valid integer.
#[test]
fn zero_integer() {
    assert_eq!(first_token("0"), token(TokenKind::Int(0), 0, 1));
}

/// Underscores in a decimal integer are ignored but belong to its span.
#[test]
fn decimal_integer_with_underscores() {
    assert_eq!(
        first_token("1_000_000"),
        token(TokenKind::Int(1_000_000), 0, 9)
    );
}

/// A hexadecimal integer with uppercase digits.
#[test]
fn hex_integer_uppercase() {
    assert_eq!(first_token("0xFF"), token(TokenKind::Int(255), 0, 4));
}

/// A hexadecimal integer with lowercase digits.
#[test]
fn hex_integer_lowercase() {
    assert_eq!(first_token("0xff"), token(TokenKind::Int(255), 0, 4));
}

/// Underscores in a hexadecimal integer are ignored.
#[test]
fn hex_integer_with_underscores() {
    assert_eq!(first_token("0xF_F"), token(TokenKind::Int(255), 0, 5));
}

/// In a hexadecimal integer `e` is a digit, not an exponent.
#[test]
fn hex_integer_with_e_digit_is_not_a_float() {
    let expected = [
        TokenKind::Int(30),
        TokenKind::Minus,
        TokenKind::Int(3),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("0x1e-3"), expected);
}

/// A binary integer.
#[test]
fn binary_integer() {
    assert_eq!(first_token("0b1010"), token(TokenKind::Int(10), 0, 6));
}

/// Underscores in a binary integer are ignored.
#[test]
fn binary_integer_with_underscores() {
    assert_eq!(first_token("0b1_0"), token(TokenKind::Int(2), 0, 5));
}

/// `u64::MAX` written in decimal is accepted.
#[test]
fn u64_max_in_decimal_is_accepted() {
    let expected = token(TokenKind::Int(u64::MAX), 0, 20);
    assert_eq!(first_token("18446744073709551615"), expected);
}

/// `u64::MAX` written in hex is accepted.
#[test]
fn u64_max_in_hex_is_accepted() {
    let expected = token(TokenKind::Int(u64::MAX), 0, 18);
    assert_eq!(first_token("0xFFFFFFFFFFFFFFFF"), expected);
}

/// `u64::MAX + 1` in decimal is an error spanning the whole literal.
#[test]
fn u64_max_plus_one_in_decimal_is_too_large() {
    let expected = Err(error("integer literal is too large", 0, 20));
    assert_eq!(lex("18446744073709551616"), expected);
}

/// A too-large hex literal is an error spanning the whole literal, prefix included.
#[test]
fn too_large_hex_literal_is_an_error() {
    let expected = Err(error("integer literal is too large", 0, 19));
    assert_eq!(lex("0x1FFFFFFFFFFFFFFFF"), expected);
}

/// The too-large error span starts where the literal starts.
#[test]
fn too_large_literal_span_is_offset_by_earlier_text() {
    let expected = Err(error("integer literal is too large", 5, 25));
    assert_eq!(lex("x := 18446744073709551616"), expected);
}

/// `0x` with no digits is an error spanning the prefix.
#[test]
fn hex_prefix_without_digits_is_an_error() {
    let expected = Err(error("expected digits after `0x`", 0, 2));
    assert_eq!(lex("0x"), expected);
}

/// `0x` followed by a space is also missing its digits.
#[test]
fn hex_prefix_followed_by_space_is_an_error() {
    let expected = Err(error("expected digits after `0x`", 4, 6));
    assert_eq!(lex("x = 0x 1"), expected);
}

/// `0b` with no digits is an error spanning the prefix.
#[test]
fn binary_prefix_without_digits_is_an_error() {
    let expected = Err(error("expected digits after `0b`", 0, 2));
    assert_eq!(lex("0b"), expected);
}

/// A digit that is invalid for the base is an error spanning that digit.
#[test]
fn invalid_digit_in_binary_literal_is_an_error() {
    let expected = Err(error("invalid character `2` in number literal", 4, 5));
    assert_eq!(lex("0b102"), expected);
}

/// A letter directly after a decimal number is an error spanning the letter.
#[test]
fn letter_after_decimal_number_is_an_error() {
    let expected = Err(error("invalid character `a` in number literal", 3, 4));
    assert_eq!(lex("123abc"), expected);
}

/// A non-hex letter inside a hex literal is an error spanning that letter.
#[test]
fn non_hex_letter_in_hex_literal_is_an_error() {
    let expected = Err(error("invalid character `G` in number literal", 3, 4));
    assert_eq!(lex("0xFG"), expected);
}

/// A letter directly after a float is an error spanning the letter.
#[test]
fn letter_after_float_is_an_error() {
    let expected = Err(error("invalid character `a` in number literal", 4, 5));
    assert_eq!(lex("3.14abc"), expected);
}

// ---------------------------------------------------------------- floats

/// A float with digits on both sides of the dot.
#[test]
// The spec's own example is 3.14, which clippy mistakes for an approximation of PI.
#[allow(clippy::approx_constant)]
fn float_with_fraction() {
    assert_eq!(first_token("3.14"), token(TokenKind::Float(3.14), 0, 4));
}

/// A float with a zero fraction is still a float.
#[test]
fn float_with_zero_fraction() {
    assert_eq!(first_token("2.0"), token(TokenKind::Float(2.0), 0, 3));
}

/// A negative exponent without a fraction makes a float.
#[test]
fn float_with_negative_exponent() {
    assert_eq!(first_token("1e-3"), token(TokenKind::Float(1e-3), 0, 4));
}

/// An uppercase `E` exponent makes a float.
#[test]
fn float_with_uppercase_exponent() {
    assert_eq!(first_token("1E5"), token(TokenKind::Float(1e5), 0, 3));
}

/// A fraction and an explicit `+` exponent sign.
#[test]
fn float_with_fraction_and_positive_exponent() {
    assert_eq!(
        first_token("2.5e+10"),
        token(TokenKind::Float(2.5e10), 0, 7)
    );
}

/// An exponent without a sign.
#[test]
fn float_with_unsigned_exponent() {
    assert_eq!(first_token("1.5e3"), token(TokenKind::Float(1500.0), 0, 5));
}

/// Underscores are allowed in the integer part of a float.
#[test]
fn float_with_underscores() {
    assert_eq!(
        first_token("1_000.5"),
        token(TokenKind::Float(1000.5), 0, 7)
    );
}

/// A range between two integers is `Int DotDot Int`, not a float.
#[test]
fn range_between_integers_is_not_a_float() {
    let expected = [
        TokenKind::Int(1),
        TokenKind::DotDot,
        TokenKind::Int(10),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("1..10"), expected);
}

/// An inclusive range between two integers is `Int DotDotEq Int`.
#[test]
fn inclusive_range_between_integers_is_not_a_float() {
    let expected = [
        TokenKind::Int(1),
        TokenKind::DotDotEq,
        TokenKind::Int(10),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("1..=10"), expected);
}

/// A range from an integer to an identifier is `Int DotDot Ident`.
#[test]
fn range_from_integer_to_identifier() {
    let expected = [
        TokenKind::Int(0),
        TokenKind::DotDot,
        ident("n"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("0..n"), expected);
}

/// A dot not followed by a digit does not belong to the number.
#[test]
fn dot_without_following_digit_is_not_part_of_the_number() {
    let expected = [TokenKind::Int(1), TokenKind::Dot, TokenKind::Eof];
    assert_eq!(kinds("1."), expected);
}

/// An exponent with no digits at all is an error from the `e` to the end of the text.
#[test]
fn exponent_without_digits_is_an_error() {
    let expected = Err(error("expected digits in exponent", 1, 2));
    assert_eq!(lex("1e"), expected);
}

/// An exponent with a sign but no digits includes the sign in the error span.
#[test]
fn exponent_with_sign_but_no_digits_includes_the_sign() {
    let expected = Err(error("expected digits in exponent", 1, 3));
    assert_eq!(lex("1e+"), expected);
}

/// The exponent error span starts at the `e` even after a fraction.
#[test]
fn exponent_without_digits_after_a_fraction() {
    let expected = Err(error("expected digits in exponent", 3, 5));
    assert_eq!(lex("1.5E- "), expected);
}

// ---------------------------------------------------------------- strings

/// A simple string keeps its text and spans both quotes.
#[test]
fn simple_string() {
    assert_eq!(first_token("\"hello\""), token(string("hello"), 0, 7));
}

/// An empty string has an empty value.
#[test]
fn empty_string() {
    assert_eq!(first_token("\"\""), token(string(""), 0, 2));
}

/// The value is the raw text: escapes and interpolation braces are not decoded.
#[test]
fn string_value_is_raw_and_undecoded() {
    let text = r#""a\n{x}""#;

    let expected = token(string(r"a\n{x}"), 0, 8);
    assert_eq!(first_token(text), expected);
}

/// Every allowed escape is accepted and kept as written.
#[test]
fn every_allowed_escape_is_accepted() {
    let text = r#""\n\t\"\\\{""#;

    let expected = token(string(r#"\n\t\"\\\{"#), 0, 12);
    assert_eq!(first_token(text), expected);
}

/// An escaped quote does not end the string.
#[test]
fn escaped_quote_does_not_end_the_string() {
    let text = r#""a\"b" x"#;

    let expected = [
        string(r#"a\"b"#),
        ident("x"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds(text), expected);
}

/// An escaped backslash before the closing quote does not escape the quote.
#[test]
fn escaped_backslash_before_closing_quote_ends_the_string() {
    let text = r#""a\\" x"#;

    let expected = [
        string(r"a\\"),
        ident("x"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds(text), expected);
}

/// Non-ASCII chars inside a string are kept, and the span counts bytes.
#[test]
fn string_with_multi_byte_char_has_a_byte_span() {
    assert_eq!(first_token("\"é\""), token(string("é"), 0, 4));
}

/// `//` inside a string is text, not a comment.
#[test]
fn comment_marker_inside_a_string_is_text() {
    let text = "\"a // b\"";

    assert_eq!(first_token(text), token(string("a // b"), 0, 8));
}

/// An escape that is not allowed is an error spanning the backslash and the next char.
#[test]
fn invalid_escape_is_an_error() {
    let expected = Err(error("invalid escape sequence `\\q`", 2, 4));
    assert_eq!(lex("\"a\\qb\""), expected);
}

/// `\r` is not an allowed escape.
#[test]
fn carriage_return_escape_is_invalid() {
    let expected = Err(error("invalid escape sequence `\\r`", 1, 3));
    assert_eq!(lex("\"\\r\""), expected);
}

/// The invalid-escape span covers the full UTF-8 width of a multi-byte next char.
#[test]
fn invalid_escape_with_multi_byte_char_spans_its_full_width() {
    let expected = Err(error("invalid escape sequence `\\é`", 1, 4));
    assert_eq!(lex("\"\\é\""), expected);
}

/// A string cut off by a newline is an error spanning only the opening quote.
#[test]
fn string_unterminated_by_newline_is_an_error() {
    let expected = Err(error("unterminated string literal", 0, 1));
    assert_eq!(lex("\"abc\nx"), expected);
}

/// A string cut off by the end of input is an error spanning only the opening quote.
#[test]
fn string_unterminated_by_end_of_input_is_an_error() {
    let expected = Err(error("unterminated string literal", 0, 1));
    assert_eq!(lex("\"abc"), expected);
}

/// The unterminated-string span is the opening quote wherever it is.
#[test]
fn unterminated_string_span_is_offset_by_earlier_text() {
    let expected = Err(error("unterminated string literal", 5, 6));
    assert_eq!(lex("x := \"abc\n"), expected);
}

/// A lone quote is an unterminated string.
#[test]
fn lone_quote_is_an_unterminated_string() {
    let expected = Err(error("unterminated string literal", 0, 1));
    assert_eq!(lex("\""), expected);
}

/// An invalid escape before the line ends is reported before the missing quote.
#[test]
fn invalid_escape_is_reported_before_unterminated_string() {
    let expected = Err(error("invalid escape sequence `\\q`", 1, 3));
    assert_eq!(lex("\"\\q\n"), expected);
}

/// A float literal that overflows to infinity is an error spanning the whole literal.
#[test]
fn float_that_overflows_is_too_large() {
    let expected = Err(error("float literal is too large", 0, 5));
    assert_eq!(lex("1e400"), expected);
}

/// A float literal that underflows is accepted as zero.
#[test]
fn float_that_underflows_is_zero() {
    assert_eq!(first_token("1e-400"), token(TokenKind::Float(0.0), 0, 6));
}

/// A backslash followed by `\r\n` inside a string is an unterminated string.
#[test]
fn backslash_before_crlf_in_a_string_is_unterminated() {
    let expected = Err(error("unterminated string literal", 0, 1));
    assert_eq!(lex("\"a\\\r\n"), expected);
}

/// A non-ASCII char right after a number is not part of the number: the lexer then
/// stops at it with the unexpected-character error.
#[test]
fn non_ascii_char_after_a_number_is_an_unexpected_character() {
    let expected = Err(error("unexpected character `é`", 1, 3));
    assert_eq!(lex("1é"), expected);
}

// ---------------------------------------------------------------- unexpected characters

/// A `$` outside a string is an error spanning that char.
#[test]
fn unexpected_dollar_is_an_error() {
    let expected = Err(error("unexpected character `$`", 0, 1));
    assert_eq!(lex("$"), expected);
}

/// The unexpected-character span is offset by earlier text.
#[test]
fn unexpected_character_span_is_offset_by_earlier_text() {
    let expected = Err(error("unexpected character `$`", 2, 3));
    assert_eq!(lex("a $"), expected);
}

/// Other ASCII chars that are not part of the language are errors too.
#[test]
fn other_unexpected_ascii_characters_are_errors() {
    for ch in [';', '#', '`', '\\', '\''] {
        let text = ch.to_string();
        let message = format!("unexpected character `{ch}`");
        let expected = Err(error(&message, 0, 1));
        assert_eq!(lex(&text), expected, "char `{ch}`");
    }
}

/// A non-ASCII char outside a string is an error spanning its 2 bytes.
#[test]
fn unexpected_two_byte_character_spans_two_bytes() {
    let expected = Err(error("unexpected character `é`", 0, 2));
    assert_eq!(lex("é"), expected);
}

/// A 4-byte char outside a string is an error spanning all 4 bytes.
#[test]
fn unexpected_four_byte_character_spans_four_bytes() {
    let expected = Err(error("unexpected character `😀`", 2, 6));
    assert_eq!(lex("x 😀"), expected);
}

/// The lexer stops at the first error, so an earlier error wins over a later one.
#[test]
fn lexer_reports_the_first_error() {
    let expected = Err(error("unexpected character `$`", 0, 1));
    assert_eq!(lex("$ \"abc"), expected);
}

/// A valid token before an error does not hide the error.
#[test]
fn error_after_valid_tokens_is_reported() {
    let expected = Err(error("unexpected character `$`", 7, 8));
    assert_eq!(lex("x := 1 $"), expected);
}

// ---------------------------------------------------------------- comments

/// A comment alone on the line produces only `Eof`.
#[test]
fn line_comment_is_skipped() {
    assert_eq!(kinds("// hello"), [TokenKind::Eof]);
}

/// A `///` doc comment is skipped like any other comment.
#[test]
fn doc_comment_is_skipped() {
    assert_eq!(kinds("/// doc"), [TokenKind::Eof]);
}

/// A bare `//` at the end of input is a comment.
#[test]
fn empty_comment_at_end_of_input_is_skipped() {
    assert_eq!(kinds("//"), [TokenKind::Eof]);
}

/// A comment may contain chars that would be errors in code.
#[test]
fn comment_may_contain_unexpected_characters() {
    assert_eq!(kinds("// é $ \" \\"), [TokenKind::Eof]);
}

/// A comment ends at `\n` and the next line is lexed normally.
#[test]
fn comment_ends_at_the_end_of_its_line() {
    let expected = [ident("b"), TokenKind::Newline, TokenKind::Eof];
    assert_eq!(kinds("// a\nb"), expected);
}

/// A comment after a statement-ending token still lets the newline rule fire.
#[test]
fn newline_after_trailing_comment_is_emitted() {
    let text = "a // c\nb";

    let expected = vec![
        token(ident("a"), 0, 1),
        token(TokenKind::Newline, 6, 7),
        token(ident("b"), 7, 8),
        token(TokenKind::Newline, 8, 8),
        token(TokenKind::Eof, 8, 8),
    ];
    assert_eq!(tokens(text), expected);
}

/// A trailing comment at the very end of input still lets the end-of-input newline fire.
#[test]
fn end_of_input_newline_after_trailing_comment() {
    let expected = vec![
        token(ident("a"), 0, 1),
        token(TokenKind::Newline, 6, 6),
        token(TokenKind::Eof, 6, 6),
    ];
    assert_eq!(tokens("a // c"), expected);
}

/// A single slash is the `Slash` operator, not a comment.
#[test]
fn single_slash_is_not_a_comment() {
    let expected = [
        ident("a"),
        TokenKind::Slash,
        ident("b"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("a / b"), expected);
}

/// `/=` is the `SlashAssign` operator, not a comment.
#[test]
fn slash_assign_is_not_a_comment() {
    let expected = [
        ident("a"),
        TokenKind::SlashAssign,
        TokenKind::Int(2),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("a /= 2"), expected);
}

// ---------------------------------------------------------------- whitespace

/// Spaces, tabs and `\r` between tokens are skipped.
#[test]
fn spaces_tabs_and_carriage_returns_are_skipped() {
    let expected = [ident("a"), ident("b"), TokenKind::Newline, TokenKind::Eof];
    assert_eq!(kinds("a \t\r b"), expected);
}

/// A `\r\n` line ending behaves like `\n`, and `\r` is not part of the newline span.
#[test]
fn crlf_line_ending_produces_one_newline_with_a_one_byte_span() {
    let expected = vec![
        token(ident("x"), 0, 1),
        token(TokenKind::Newline, 2, 3),
        token(TokenKind::Eof, 3, 3),
    ];
    assert_eq!(tokens("x\r\n"), expected);
}

/// Several `\r\n` lines produce one `Newline` per statement-ending line.
#[test]
fn crlf_lines_produce_newlines() {
    let expected = [
        ident("x"),
        TokenKind::Newline,
        ident("y"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("x\r\ny\r\n"), expected);
}

/// Token spans are offset by leading whitespace.
#[test]
fn token_spans_skip_leading_whitespace() {
    assert_eq!(first_token("  \t x"), token(ident("x"), 4, 5));
}

// ---------------------------------------------------------------- newline rule

/// Every statement-ending token followed by `\n` and more code gives a `Newline`.
#[test]
fn newline_after_each_statement_ending_token() {
    for (text, kind) in statement_ending_table() {
        let source = format!("{text}\nx");

        let expected = [
            kind,
            TokenKind::Newline,
            ident("x"),
            TokenKind::Newline,
            TokenKind::Eof,
        ];
        assert_eq!(kinds(&source), expected, "token `{text}`");
    }
}

/// The `Newline` token spans exactly the `\n`, one byte.
#[test]
fn newline_token_spans_the_newline_byte() {
    for (text, _) in statement_ending_table() {
        let source = format!("{text}\nx");

        let newline = tokens(&source).remove(1);
        let expected = token(TokenKind::Newline, text.len(), text.len() + 1);
        assert_eq!(newline, expected, "token `{text}`");
    }
}

/// Every other keyword and punctuation token followed by `\n` gives no `Newline`.
#[test]
fn no_newline_after_each_continuing_token() {
    for (text, kind) in statement_continuing_table() {
        let source = format!("{text}\nx");

        let expected = [kind, ident("x"), TokenKind::Newline, TokenKind::Eof];
        assert_eq!(kinds(&source), expected, "token `{text}`");
    }
}

/// The continuing table is the keyword and punctuation tables minus the 10 statement-ending ones.
#[test]
fn continuing_table_excludes_statement_ending_tokens() {
    let spellings: Vec<&str> = statement_continuing_table()
        .into_iter()
        .map(|(text, _)| text)
        .collect();

    for ending in [
        "true", "false", "none", ")", "]", "}", "?", "return", "break", "continue",
    ] {
        assert!(
            !spellings.contains(&ending),
            "`{ending}` should be excluded"
        );
    }
    assert_eq!(spellings.len(), 23 + 42 - 10);
}

/// A line ending in `+` continues onto the next line.
#[test]
fn line_ending_in_plus_continues() {
    let expected = [
        ident("a"),
        TokenKind::Plus,
        ident("b"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("a +\nb"), expected);
}

/// A line ending in `,` continues onto the next line.
#[test]
fn line_ending_in_comma_continues() {
    let expected = [
        TokenKind::LParen,
        ident("a"),
        TokenKind::Comma,
        ident("b"),
        TokenKind::RParen,
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("(a,\nb)"), expected);
}

/// A line ending in `{` continues onto the next line.
#[test]
fn line_ending_in_open_brace_continues() {
    let expected = [
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("{\n}"), expected);
}

/// A line ending in `(` continues onto the next line.
#[test]
fn line_ending_in_open_paren_continues() {
    let expected = [
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("(\n)"), expected);
}

/// Blank lines after a statement produce only one `Newline`.
#[test]
fn blank_lines_do_not_produce_extra_newlines() {
    let expected = [
        ident("x"),
        TokenKind::Newline,
        ident("y"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("x\n\n\ny"), expected);
}

/// Lines with only spaces count as blank.
#[test]
fn whitespace_only_lines_do_not_produce_extra_newlines() {
    let expected = [
        ident("x"),
        TokenKind::Newline,
        ident("y"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("x\n  \t \ny"), expected);
}

/// Comment-only lines after a statement produce only one `Newline`.
#[test]
fn comment_only_lines_do_not_produce_extra_newlines() {
    let expected = [
        ident("x"),
        TokenKind::Newline,
        ident("y"),
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds("x\n// one\n/// two\n\ny"), expected);
}

/// Newlines at the start of the input produce no `Newline`.
#[test]
fn no_newline_at_the_start_of_input() {
    let expected = [ident("x"), TokenKind::Newline, TokenKind::Eof];
    assert_eq!(kinds("\n\nx"), expected);
}

/// An input of only a newline is just `Eof`.
#[test]
fn only_a_newline_gives_only_eof() {
    assert_eq!(kinds("\n"), [TokenKind::Eof]);
}

/// A statement-ending last token at end of input gets an empty-span `Newline` before `Eof`.
#[test]
fn newline_at_end_of_input_has_an_empty_span() {
    let expected = vec![
        token(ident("x"), 0, 1),
        token(TokenKind::Newline, 1, 1),
        token(TokenKind::Eof, 1, 1),
    ];
    assert_eq!(tokens("x"), expected);
}

/// Every statement-ending token at end of input gets an empty-span `Newline`.
#[test]
fn newline_at_end_of_input_after_each_statement_ending_token() {
    for (text, kind) in statement_ending_table() {
        let len = text.len();

        let expected = vec![
            token(kind, 0, len),
            token(TokenKind::Newline, len, len),
            token(TokenKind::Eof, len, len),
        ];
        assert_eq!(tokens(text), expected, "token `{text}`");
    }
}

/// Trailing spaces after the last token do not move the end-of-input `Newline`.
#[test]
fn end_of_input_newline_is_at_the_text_length_after_trailing_spaces() {
    let expected = vec![
        token(ident("x"), 0, 1),
        token(TokenKind::Newline, 3, 3),
        token(TokenKind::Eof, 3, 3),
    ];
    assert_eq!(tokens("x  "), expected);
}

/// A final `\n` gives its own `Newline`, and no second one at end of input.
#[test]
fn no_extra_newline_when_the_text_ends_with_a_newline() {
    let expected = vec![
        token(ident("x"), 0, 1),
        token(TokenKind::Newline, 1, 2),
        token(TokenKind::Eof, 2, 2),
    ];
    assert_eq!(tokens("x\n"), expected);
}

/// A non-ending last token at end of input gets no `Newline`.
#[test]
fn no_newline_at_end_of_input_after_a_continuing_token() {
    let expected = vec![token(TokenKind::Plus, 0, 1), token(TokenKind::Eof, 1, 1)];
    assert_eq!(tokens("+"), expected);
}

/// Every continuing token at end of input gets no `Newline`.
#[test]
fn no_newline_at_end_of_input_after_each_continuing_token() {
    for (text, kind) in statement_continuing_table() {
        let len = text.len();

        let expected = vec![token(kind, 0, len), token(TokenKind::Eof, len, len)];
        assert_eq!(tokens(text), expected, "token `{text}`");
    }
}

// ---------------------------------------------------------------- Eof

/// Empty input is a single `Eof` with an empty span at 0.
#[test]
fn empty_input_is_only_eof() {
    assert_eq!(tokens(""), vec![token(TokenKind::Eof, 0, 0)]);
}

/// Whitespace-only input is a single `Eof` at the text length.
#[test]
fn whitespace_only_input_is_only_eof() {
    assert_eq!(tokens("  \t "), vec![token(TokenKind::Eof, 4, 4)]);
}

/// `Eof` has an empty span at the end of the text, counted in bytes.
#[test]
fn eof_span_is_empty_at_the_byte_length() {
    let eof = tokens("\"é\" +").pop().unwrap();

    assert_eq!(eof, token(TokenKind::Eof, 6, 6));
}

/// `Eof` is the last token and appears exactly once.
#[test]
fn eof_appears_exactly_once_at_the_end() {
    let kinds = kinds("fn f() {\n  return 1\n}\n");
    let eof_count = kinds.iter().filter(|kind| **kind == TokenKind::Eof).count();

    assert_eq!(eof_count, 1);
    assert_eq!(kinds.last(), Some(&TokenKind::Eof));
}

// ---------------------------------------------------------------- token spans

/// Spans of a small program count bytes and cover exactly each token's text.
#[test]
fn spans_of_a_small_program() {
    let expected = vec![
        token(ident("x"), 0, 1),
        token(TokenKind::ColonAssign, 2, 4),
        token(TokenKind::Int(1), 5, 6),
        token(TokenKind::Plus, 7, 8),
        token(TokenKind::Int(23), 9, 11),
        token(TokenKind::Newline, 11, 12),
        token(TokenKind::Eof, 12, 12),
    ];
    assert_eq!(tokens("x := 1 + 23\n"), expected);
}

/// Spans after a multi-byte string are offset by its byte length, not its char count.
#[test]
fn spans_after_a_multi_byte_string_count_bytes() {
    let expected = vec![
        token(string("é"), 0, 4),
        token(ident("x"), 5, 6),
        token(TokenKind::Newline, 6, 6),
        token(TokenKind::Eof, 6, 6),
    ];
    assert_eq!(tokens("\"é\" x"), expected);
}

/// Spans on later lines continue counting from the start of the text.
#[test]
fn spans_on_later_lines_are_absolute_offsets() {
    let expected = vec![
        token(ident("a"), 0, 1),
        token(TokenKind::Newline, 1, 2),
        token(ident("bc"), 2, 4),
        token(TokenKind::Newline, 4, 5),
        token(TokenKind::Eof, 5, 5),
    ];
    assert_eq!(tokens("a\nbc\n"), expected);
}

// ---------------------------------------------------------------- full example

/// The full example from the spec lexes to the exact token sequence.
#[test]
fn spec_example_function() {
    let text = "fn add(a: i64) -> i64 {\n    return a +\n        1\n}\n";

    let expected = [
        TokenKind::KwFn,
        ident("add"),
        TokenKind::LParen,
        ident("a"),
        TokenKind::Colon,
        ident("i64"),
        TokenKind::RParen,
        TokenKind::Arrow,
        ident("i64"),
        TokenKind::LBrace,
        TokenKind::KwReturn,
        ident("a"),
        TokenKind::Plus,
        TokenKind::Int(1),
        TokenKind::Newline,
        TokenKind::RBrace,
        TokenKind::Newline,
        TokenKind::Eof,
    ];
    assert_eq!(kinds(text), expected);
}
