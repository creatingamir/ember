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

/// An expression whose type the checker did not record is an internal error with the exact message, at span 0..0.
#[test]
fn expression_without_a_type_is_an_internal_error() {
    let src = "fn main() {\n    print(1)\n}\n";
    let program = parser::parse_source(src).unwrap();
    let mut checked = check::check(&program).unwrap();
    checked.expr_types.clear();
    let error = lower(&program, &checked, &SourceFile::new("t.em", src)).unwrap_err();
    assert_eq!(
        error.message,
        "internal compiler error: an expression has no type"
    );
    assert_eq!(error.span, source::Span::new(0, 0));
}

/// Checks `src`, removes the signature of `function`, lowers it and returns the error.
fn lower_without_signature(src: &str, function: &str) -> Diagnostic {
    let program = parser::parse_source(src).unwrap();
    let mut checked = check::check(&program).unwrap();
    assert!(checked.signatures.remove(function).is_some());
    lower(&program, &checked, &SourceFile::new("t.em", src)).unwrap_err()
}

/// A callee without a signature is reported at the call, not at the function that contains it.
#[test]
fn missing_callee_signature_is_reported_at_the_call() {
    let src = "fn main() {\n    g()\n}\nfn g() {\n}\n";
    let error = lower_without_signature(src, "g");
    assert_eq!(
        error.message,
        "internal compiler error: the function `g` has no signature"
    );
    let start = src.find("g()").unwrap();
    assert_eq!(error.span, source::Span::new(start, start + "g()".len()));
}

/// A function without its own signature is reported at its name.
#[test]
fn missing_function_signature_is_reported_at_its_name() {
    let src = "fn main() {\n}\n";
    let error = lower_without_signature(src, "main");
    assert_eq!(
        error.message,
        "internal compiler error: the function `main` has no signature"
    );
    let start = src.find("main").unwrap();
    assert_eq!(error.span, source::Span::new(start, start + "main".len()));
}
