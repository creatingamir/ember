//! Deterministic property test: `lex` never panics and always returns well-formed spans.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use lexer::{TokenKind, lex};
use source::{SourceFile, Span};

/// How many random inputs the test generates.
const INPUT_COUNT: usize = 20_000;

/// Text pieces that tend to trigger edge cases in the lexer.
const TRICKY_FRAGMENTS: [&str; 17] = [
    "0x",
    "0b",
    "1e+",
    "1e",
    "_",
    "\\",
    "\"",
    "//",
    "é",
    "😀",
    "\r\n",
    "\t",
    "..=",
    "..",
    "\u{0}",
    "\u{FEFF}",
    "18446744073709551616",
];

/// A small xorshift64 pseudo-random generator with a fixed seed, so runs are repeatable.
struct Random {
    state: u64,
}

impl Random {
    /// Creates a generator from a non-zero seed.
    fn new(seed: u64) -> Self {
        Random { state: seed }
    }

    /// Returns the next pseudo-random 64-bit number.
    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Returns a pseudo-random number below `limit`.
    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    /// Returns a random printable ASCII char or one of `\n`, `\t`, `\r`.
    fn ascii_char(&mut self) -> char {
        let printable_count = 0x7F - 0x20;
        let choice = self.below(printable_count + 3);
        match choice {
            c if c < printable_count => char::from_u32(0x20 + c as u32).unwrap(),
            c if c == printable_count => '\n',
            c if c == printable_count + 1 => '\t',
            _ => '\r',
        }
    }

    /// Returns a random Unicode scalar value (surrogates are replaced by `?`).
    fn unicode_char(&mut self) -> char {
        let code = self.below(0x11_0000) as u32;
        char::from_u32(code).unwrap_or('?')
    }

    /// Builds a random input mixing ASCII, tricky fragments and Unicode chars.
    fn input(&mut self) -> String {
        let piece_count = self.below(40);
        let mut text = String::new();
        for _ in 0..piece_count {
            match self.below(4) {
                0 | 1 => text.push(self.ascii_char()),
                2 => text.push_str(TRICKY_FRAGMENTS[self.below(TRICKY_FRAGMENTS.len())]),
                _ => text.push(self.unicode_char()),
            }
        }
        text
    }
}

/// Asserts a span lies within the text and on char boundaries.
fn assert_span_is_valid(span: Span, text: &str) {
    assert!(span.start <= span.end, "start after end in {span:?}");
    assert!(span.end <= text.len(), "span {span:?} past end of text");
    assert!(text.is_char_boundary(span.start), "bad start in {span:?}");
    assert!(text.is_char_boundary(span.end), "bad end in {span:?}");
}

/// Checks every property of the lexer's result for one input.
fn check_input(text: &str) {
    match lex(text) {
        Ok(tokens) => {
            let eof_count = tokens
                .iter()
                .filter(|token| token.kind == TokenKind::Eof)
                .count();
            assert_eq!(eof_count, 1, "input {text:?}");
            assert_eq!(
                tokens.last().unwrap().kind,
                TokenKind::Eof,
                "input {text:?}"
            );

            let mut previous_start = 0;
            for token in &tokens {
                assert_span_is_valid(token.span, text);
                assert!(token.span.start >= previous_start, "input {text:?}");
                previous_start = token.span.start;
            }
        }
        Err(diagnostic) => {
            assert_span_is_valid(diagnostic.span, text);

            let rendered = diagnostic.render(&SourceFile::new("f", text));
            assert!(rendered.ends_with("^\n"), "input {text:?}");
        }
    }
}

/// Lexing 20,000 pseudo-random inputs never panics and always gives well-formed results.
#[test]
fn lex_never_panics_and_spans_are_well_formed() {
    let mut random = Random::new(0x9E37_79B9_7F4A_7C15);

    for _ in 0..INPUT_COUNT {
        let text = random.input();
        check_input(&text);
    }
}
