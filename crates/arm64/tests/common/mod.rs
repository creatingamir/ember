//! Helpers shared by the `arm64` integration tests: the front-end pipeline, the toolchain run and locations.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]
// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use source::SourceFile;

/// The file name every test program is compiled under; it shows up in panic messages.
pub const FILE: &str = "t.em";

/// The largest `i64`, as source text.
pub const MAX: &str = "9223372036854775807";

/// The smallest `i64`, as source text (a negated literal).
pub const MIN: &str = "-9223372036854775808";

/// Joins source lines into program text with a trailing newline.
pub fn program<S: AsRef<str>>(lines: &[S]) -> String {
    let mut text = lines
        .iter()
        .map(|line| line.as_ref())
        .collect::<Vec<&str>>()
        .join("\n");
    text.push('\n');
    text
}

/// Wraps statement lines into `fn main() { ... }`; the first body line is source line 2.
pub fn main_with<S: AsRef<str>>(body: &[S]) -> String {
    let mut lines = vec!["fn main() {".to_string()];
    lines.extend(body.iter().map(|line| line.as_ref().to_string()));
    lines.push("}".to_string());
    program(&lines)
}

/// Runs the front end on valid source text and returns the IR module.
pub fn module(src: &str) -> ir::Module {
    let program = parser::parse_source(src).unwrap();
    let checked = check::check(&program).unwrap();
    ir::lower(&program, &checked, &SourceFile::new(FILE, src))
        .map_err(|diagnostic| {
            format!(
                "ir::lower failed on a checked program: {}",
                diagnostic.message
            )
        })
        .unwrap()
}

/// Emits assembly for a module that must be accepted, with `file_name` shown in panic messages.
pub fn emit_ok(module: &ir::Module, file_name: &str) -> String {
    arm64::emit(module, file_name)
        .map_err(|diagnostic| format!("arm64::emit failed: {}", diagnostic.message))
        .unwrap()
}

/// Compiles valid source text to assembly text, with `file_name` shown in panic messages.
pub fn asm_named(src: &str, file_name: &str) -> String {
    emit_ok(&module(src), file_name)
}

/// Compiles valid source text to assembly text under the default file name.
pub fn asm(src: &str) -> String {
    asm_named(src, FILE)
}

/// Returns the 1-based `line:col` of the `nth` (0-based) occurrence of `needle` in `src`.
pub fn loc(src: &str, needle: &str, nth: usize) -> String {
    let found = src.match_indices(needle).nth(nth);
    assert!(
        found.is_some(),
        "occurrence {nth} of `{needle}` not found in:\n{src}"
    );
    let (offset, _) = found.unwrap();
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    let col = offset - line_start + 1;
    format!("{line}:{col}")
}

/// The exact stderr of a panic with `reason` at the `nth` occurrence of `needle` in `src`.
pub fn panic_stderr(reason: &str, src: &str, needle: &str, nth: usize) -> String {
    format!("panic: {reason} at {FILE}:{}\n", loc(src, needle, nth))
}

/// A scratch directory removed when dropped, even when the test fails.
struct TempDir(PathBuf);

impl TempDir {
    /// Creates a directory named after the process id and a per-process counter.
    fn new() -> TempDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        let name = format!("ember-arm64-{}-{id}", std::process::id());
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).unwrap();
        TempDir(path)
    }

    /// The directory's path.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    /// Removes the directory and everything in it.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs `clang` with the arguments inside `dir` and asserts that it succeeded.
fn clang(dir: &Path, args: &[&str], asm: &str) {
    let output = Command::new("clang")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "clang {args:?} failed:\n{}\n--- assembly ---\n{asm}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Compiles, assembles, links and runs a program; returns its stdout, stderr and exit code.
pub fn run(src: &str) -> (String, String, i32) {
    let asm = asm(src);
    let dir = TempDir::new();
    fs::write(dir.path().join("prog.s"), &asm).unwrap();

    clang(dir.path(), &["-c", "prog.s", "-o", "prog.o"], &asm);
    clang(dir.path(), &["prog.o", "-o", "prog"], &asm);

    let output = Command::new(dir.path().join("prog")).output().unwrap();
    // A process killed by a signal has no exit code; -1 makes the comparison fail loudly.
    let code = output.status.code().unwrap_or(-1);
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        code,
    )
}
