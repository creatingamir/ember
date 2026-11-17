//! Integration tests for the internal compiler errors that `lower` returns.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use check::Checked;
use ir::lower;
use source::{Diagnostic, SourceFile};

/// Lowers a program against an empty `Checked`, which knows nothing about it.
fn lower_unchecked(source_lines: &[&str]) -> Result<ir::Module, Diagnostic> {
    let mut src = source_lines.join("\n");
    src.push('\n');
    let program = parser::parse_source(&src).unwrap();
    lower(
        &program,
        &Checked::default(),
        &SourceFile::new("t.em", &src),
    )
}

/// Asserts that lowering fails with an internal compiler error.
fn assert_internal_error(source_lines: &[&str]) {
    let error = lower_unchecked(source_lines).unwrap_err();
    assert!(
        error.message.starts_with("internal compiler error:"),
        "unexpected message: {}",
        error.message
    );
}

/// Reading a name that the checker never bound is an internal error.
#[test]
fn unbound_name_is_an_internal_error() {
    assert_internal_error(&[
        "fn f(n: i64) -> i64 {",
        "    return n",
        "}",
        "fn main() {",
        "}",
    ]);
}

/// A call the checker never bound is an internal error.
#[test]
fn unbound_call_is_an_internal_error() {
    assert_internal_error(&["fn g() {", "}", "fn main() {", "    g()", "}"]);
}

/// A `print` the checker never bound is an internal error.
#[test]
fn unbound_print_is_an_internal_error() {
    assert_internal_error(&["fn main() {", "    print(1)", "}"]);
}
