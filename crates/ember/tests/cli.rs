//! Integration tests for the `ember` command-line interface.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use std::process::{Command, Output};

/// Runs the built `ember` binary with the given arguments.
fn run_ember(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ember"))
        .args(args)
        .output()
        .unwrap()
}

/// `--version` prints exactly the name and version on stdout.
#[test]
fn version_prints_name_and_version() {
    let output = run_ember(&["--version"]);

    assert_eq!(String::from_utf8_lossy(&output.stdout), "ember 0.1.0\n");
}

/// `--version` exits with code 0.
#[test]
fn version_exits_with_zero() {
    let output = run_ember(&["--version"]);

    assert_eq!(output.status.code(), Some(0));
}

/// `--help` exits with code 0.
#[test]
fn help_exits_with_zero() {
    let output = run_ember(&["--help"]);

    assert_eq!(output.status.code(), Some(0));
}

/// `--help` prints a usage line on stdout.
#[test]
fn help_prints_usage_on_stdout() {
    let output = run_ember(&["--help"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: ember"), "stdout was: {stdout}");
}

/// Running with no arguments exits with code 2.
#[test]
fn no_arguments_exits_with_code_two() {
    let output = run_ember(&[]);

    assert_eq!(output.status.code(), Some(2));
}

/// Running with no arguments prints the usage on stderr.
#[test]
fn no_arguments_prints_usage_on_stderr() {
    let output = run_ember(&[]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage"), "stderr was: {stderr}");
}

/// An unknown subcommand exits with code 2.
#[test]
fn unknown_subcommand_exits_with_code_two() {
    let output = run_ember(&["frobnicate"]);

    assert_eq!(output.status.code(), Some(2));
}

/// An unknown flag exits with code 2.
#[test]
fn unknown_flag_exits_with_code_two() {
    let output = run_ember(&["--frobnicate"]);

    assert_eq!(output.status.code(), Some(2));
}
