//! Integration tests for the public API of the `source` crate.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use source::{Diagnostic, SourceFile, Span};

/// Builds a source file named `t.em` with the given text.
fn file(text: &str) -> SourceFile {
    SourceFile::new("t.em", text)
}

// ---------------------------------------------------------------- Span

/// `Span::new` stores the start and end offsets in its public fields.
#[test]
fn span_new_stores_start_and_end() {
    let span = Span::new(3, 7);

    assert_eq!(span.start, 3);
    assert_eq!(span.end, 7);
}

/// Spans with the same offsets are equal and spans with different offsets are not.
#[test]
fn span_equality_compares_both_offsets() {
    assert_eq!(Span::new(1, 2), Span::new(1, 2));
    assert_ne!(Span::new(1, 2), Span::new(1, 3));
    assert_ne!(Span::new(1, 2), Span::new(0, 2));
}

/// A span is `Copy`, so using it twice after a move still compiles.
#[test]
fn span_is_copy() {
    let span = Span::new(0, 1);
    let copy = span;

    assert_eq!(span, copy);
}

// ---------------------------------------------------------------- SourceFile basics

/// `name` returns the display name given to `SourceFile::new`.
#[test]
fn source_file_returns_its_name() {
    let source = SourceFile::new("main.em", "x");

    assert_eq!(source.name(), "main.em");
}

/// `text` returns the text given to `SourceFile::new`, unchanged.
#[test]
fn source_file_returns_its_text() {
    let source = SourceFile::new("main.em", "fn main() {\r\n}\n");

    assert_eq!(source.text(), "fn main() {\r\n}\n");
}

/// `SourceFile::new` accepts owned `String` arguments as well as `&str`.
#[test]
fn source_file_new_accepts_owned_strings() {
    let source = SourceFile::new(String::from("a.em"), String::from("abc"));

    assert_eq!(source.name(), "a.em");
    assert_eq!(source.text(), "abc");
}

// ---------------------------------------------------------------- line_col

/// Offset 0 is line 1, column 1.
#[test]
fn line_col_of_first_char_is_1_1() {
    assert_eq!(file("ab\ncd").line_col(0), (1, 1));
}

/// A later offset on the first line has the matching column.
#[test]
fn line_col_counts_columns_on_the_first_line() {
    assert_eq!(file("ab\ncd").line_col(1), (1, 2));
}

/// The `\n` itself belongs to the line it ends.
#[test]
fn line_col_of_the_newline_char_is_on_its_own_line() {
    assert_eq!(file("ab\ncd").line_col(2), (1, 3));
}

/// The first char after a `\n` is column 1 of the next line.
#[test]
fn line_col_of_first_char_on_a_later_line_is_column_1() {
    assert_eq!(file("ab\ncd").line_col(3), (2, 1));
}

/// A later offset on a later line has the matching column.
#[test]
fn line_col_counts_columns_on_a_later_line() {
    assert_eq!(file("ab\ncd\nef").line_col(7), (3, 2));
}

/// The column counts chars, not bytes, so a 2-byte char is one column.
#[test]
fn line_col_counts_a_multi_byte_char_as_one_column() {
    // `é` occupies bytes 0..2, so `=` starts at byte offset 2.
    assert_eq!(file("é=1").line_col(2), (1, 2));
}

/// Columns on a later line also count chars when an earlier line has multi-byte chars.
#[test]
fn line_col_handles_multi_byte_chars_on_the_same_line_before_the_offset() {
    // Bytes: `a`0 `é`1..3 `é`3..5 `b`5.
    assert_eq!(file("aééb").line_col(5), (1, 4));
}

/// A line that follows a line with multi-byte chars still starts at column 1.
#[test]
fn line_col_after_a_multi_byte_line_starts_at_column_1() {
    // `é\n` is 3 bytes, so line 2 starts at byte offset 3.
    assert_eq!(file("é\nx").line_col(3), (2, 1));
}

/// A tab counts as a single column.
#[test]
fn line_col_counts_a_tab_as_one_column() {
    assert_eq!(file("\tx").line_col(1), (1, 2));
}

/// An offset equal to the text length is valid and points just past the last char.
#[test]
fn line_col_at_end_of_file_without_trailing_newline() {
    assert_eq!(file("ab\ncd").line_col(5), (2, 3));
}

/// An offset at the end of text that ends in `\n` is column 1 of the next (empty) line.
#[test]
fn line_col_at_end_of_file_with_trailing_newline() {
    assert_eq!(file("ab\n").line_col(3), (2, 1));
}

/// An offset past the end of the text is clamped to the end.
#[test]
fn line_col_clamps_an_offset_past_the_end() {
    let source = file("ab\ncd");

    assert_eq!(source.line_col(100), source.line_col(5));
    assert_eq!(source.line_col(100), (2, 3));
}

/// In an empty file offset 0 is line 1, column 1.
#[test]
fn line_col_in_an_empty_file_is_1_1() {
    assert_eq!(file("").line_col(0), (1, 1));
}

/// In a `\r\n` file the first char of the second line is column 1 of line 2.
#[test]
fn line_col_after_crlf_starts_the_next_line() {
    assert_eq!(file("ab\r\ncd").line_col(4), (2, 1));
}

/// In a `\r\n` file the `\r` still belongs to the line it ends.
#[test]
fn line_col_of_the_carriage_return_is_on_its_own_line() {
    assert_eq!(file("ab\r\ncd").line_col(2), (1, 3));
}

/// An offset inside a multi-byte char does not panic.
#[test]
fn line_col_with_an_offset_inside_a_multi_byte_char_returns() {
    // Byte 1 is the middle of `é` (bytes 0..2).
    let (line, column) = file("é=1").line_col(1);

    assert_eq!(line, 1);
    assert!(column >= 1);
}

// ---------------------------------------------------------------- line_text

/// Line 1 is returned without its trailing `\n`.
#[test]
fn line_text_strips_the_newline() {
    assert_eq!(file("ab\ncd\n").line_text(1), "ab");
}

/// A later line is returned without its trailing `\n`.
#[test]
fn line_text_returns_a_later_line() {
    assert_eq!(file("ab\ncd\n").line_text(2), "cd");
}

/// A `\r\n` line ending is stripped completely.
#[test]
fn line_text_strips_a_crlf_line_ending() {
    let source = file("ab\r\ncd\r\n");

    assert_eq!(source.line_text(1), "ab");
    assert_eq!(source.line_text(2), "cd");
}

/// The last line of a file without a trailing newline is returned in full.
#[test]
fn line_text_returns_the_last_line_without_trailing_newline() {
    assert_eq!(file("ab\ncd").line_text(2), "cd");
}

/// A blank line in the middle of a file is the empty string.
#[test]
fn line_text_of_a_blank_line_is_empty() {
    assert_eq!(file("a\n\nb").line_text(2), "");
}

/// Leading whitespace and tabs on a line are kept.
#[test]
fn line_text_keeps_indentation() {
    assert_eq!(file("\t  x := 1\n").line_text(1), "\t  x := 1");
}

/// Multi-byte chars in a line are returned intact.
#[test]
fn line_text_keeps_multi_byte_chars() {
    assert_eq!(file("é=1\nb").line_text(1), "é=1");
}

/// A line number past the last line gives the empty string.
#[test]
fn line_text_past_the_last_line_is_empty() {
    assert_eq!(file("ab\ncd\n").line_text(10), "");
}

/// Line 0 does not exist (lines are 1-based), so it gives the empty string.
#[test]
fn line_text_of_line_zero_is_empty() {
    assert_eq!(file("ab\ncd").line_text(0), "");
}

/// A very large line number gives the empty string rather than panicking.
#[test]
fn line_text_of_a_huge_line_number_is_empty() {
    assert_eq!(file("ab").line_text(usize::MAX), "");
}

/// Every line of an empty file is the empty string.
#[test]
fn line_text_of_an_empty_file_is_empty() {
    assert_eq!(file("").line_text(1), "");
}

// ---------------------------------------------------------------- Diagnostic

/// `Diagnostic::error` stores the message and the span in its public fields.
#[test]
fn diagnostic_error_stores_message_and_span() {
    let diagnostic = Diagnostic::error("oops", Span::new(2, 5));

    assert_eq!(diagnostic.message, "oops");
    assert_eq!(diagnostic.span, Span::new(2, 5));
}

/// `Diagnostic::error` accepts an owned `String` message.
#[test]
fn diagnostic_error_accepts_an_owned_message() {
    let diagnostic = Diagnostic::error(String::from("owned"), Span::new(0, 1));

    assert_eq!(diagnostic.message, "owned");
}

/// Diagnostics are equal when message and span are both equal.
#[test]
fn diagnostic_equality_compares_message_and_span() {
    let diagnostic = Diagnostic::error("a", Span::new(0, 1));

    assert_eq!(diagnostic, Diagnostic::error("a", Span::new(0, 1)));
    assert_ne!(diagnostic, Diagnostic::error("b", Span::new(0, 1)));
    assert_ne!(diagnostic, Diagnostic::error("a", Span::new(0, 2)));
}

// ---------------------------------------------------------------- Diagnostic::render

/// The exact example from the spec: an indented line, caret under the `+`.
#[test]
fn render_matches_the_spec_example() {
    let source = file("fn main() {\n    x := 1 +\n}\n");
    // The `+` is byte 23: line 2 starts at byte 12, and `+` is its 12th char.
    let diagnostic = Diagnostic::error("oops", Span::new(23, 24));

    let expected = concat!(
        "t.em:2:12: error: oops\n",
        "        x := 1 +\n",
        "               ^\n",
    );
    assert_eq!(diagnostic.render(&source), expected);
}

/// A span on line 1, column 1 puts the caret under the first char.
#[test]
fn render_at_line_1_column_1() {
    let source = SourceFile::new("a.em", "x\n");
    let diagnostic = Diagnostic::error("msg", Span::new(0, 1));

    let expected = concat!("a.em:1:1: error: msg\n", "    x\n", "    ^\n");
    assert_eq!(diagnostic.render(&source), expected);
}

/// A tab in the line text before the column is reproduced as a tab in the caret line.
#[test]
fn render_reproduces_tabs_in_the_caret_line() {
    let source = SourceFile::new("f.em", "\tx := 1 +\n");
    // Byte 8 is the `+`, column 9: a tab, then seven other chars.
    let diagnostic = Diagnostic::error("bad", Span::new(8, 9));

    let expected = concat!(
        "f.em:1:9: error: bad\n",
        "    \tx := 1 +\n",
        "    \t       ^\n",
    );
    assert_eq!(diagnostic.render(&source), expected);
}

/// A tab only turns into a tab in the caret line when it is before the column.
#[test]
fn render_ignores_tabs_after_the_column() {
    let source = SourceFile::new("f.em", "x\t=\n");
    let diagnostic = Diagnostic::error("bad", Span::new(0, 1));

    let expected = concat!("f.em:1:1: error: bad\n", "    x\t=\n", "    ^\n");
    assert_eq!(diagnostic.render(&source), expected);
}

/// Each multi-byte char before the column is one space in the caret line.
#[test]
fn render_uses_one_space_per_multi_byte_char() {
    let source = SourceFile::new("m.em", "é+\n");
    // `é` is bytes 0..2, so `+` is byte 2, column 2.
    let diagnostic = Diagnostic::error("msg", Span::new(2, 3));

    let expected = concat!("m.em:1:2: error: msg\n", "    é+\n", "     ^\n");
    assert_eq!(diagnostic.render(&source), expected);
}

/// The line text of a `\r\n` file is rendered without the `\r`.
#[test]
fn render_strips_the_carriage_return_from_the_line_text() {
    let source = SourceFile::new("c.em", "a +\r\nb\r\n");
    let diagnostic = Diagnostic::error("msg", Span::new(2, 3));

    let expected = concat!("c.em:1:3: error: msg\n", "    a +\n", "      ^\n");
    assert_eq!(diagnostic.render(&source), expected);
}

/// The rendered text names the file it is given, not a fixed name.
#[test]
fn render_uses_the_file_name() {
    let source = SourceFile::new("src/deep/thing.em", "x\n");
    let diagnostic = Diagnostic::error("msg", Span::new(0, 1));

    assert!(
        diagnostic
            .render(&source)
            .starts_with("src/deep/thing.em:1:1: error: msg\n")
    );
}

/// The rendered text has exactly three lines and ends with `\n`.
#[test]
fn render_ends_with_a_newline_and_has_three_lines() {
    let source = file("let x\n");
    let diagnostic = Diagnostic::error("msg", Span::new(4, 5));

    let rendered = diagnostic.render(&source);

    assert!(rendered.ends_with('\n'));
    assert_eq!(rendered.lines().count(), 3);
}

/// A span starting inside a multi-byte char renders without panicking.
#[test]
fn render_with_an_offset_inside_a_multi_byte_char_returns() {
    let source = file("é=1\n");
    let diagnostic = Diagnostic::error("msg", Span::new(1, 2));

    let rendered = diagnostic.render(&source);

    assert!(rendered.ends_with("^\n"));
}

/// A span starting past the end of the text renders at the end-of-file position.
#[test]
fn render_with_a_span_past_the_end_returns() {
    let source = file("ab\ncd");
    let diagnostic = Diagnostic::error("msg", Span::new(100, 101));

    let rendered = diagnostic.render(&source);

    assert!(rendered.starts_with("t.em:2:3: error: msg\n"));
    assert!(rendered.ends_with("^\n"));
}
