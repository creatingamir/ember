//! Integration tests for the `ember` command-line interface.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

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

// ---------------------------------------------------------------------------
// M1.6: the driver (`build`, `run`, `--emit`)
// ---------------------------------------------------------------------------

/// A program that prints one number.
const HELLO: &str = "fn main() {\n    print(42)\n}\n";

/// A program with a function, a loop and a call, so every stage has something to show.
const FIB: &str = "\
fn fib(n: i64) -> i64 {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() {
    for i in 0..=5 {
        print(fib(i))
    }
}
";

/// A program that prints 1 and 2 and then overflows at line 5, column 11.
const PANICS: &str = "\
fn main() {
    print(1)
    print(2)
    x := 9223372036854775807
    print(x + 1)
}
";

/// Lexes and parses, but the checker rejects it (undefined name at line 2, column 11).
const CHECK_ERROR: &str = "fn main() {\n    print(y)\n}\n";

/// The exact rendering of the error in `CHECK_ERROR` when the file is named `bad.em`.
const CHECK_ERROR_RENDERED: &str = "\
bad.em:2:11: error: undefined name `y`
        print(y)
              ^
";

/// Counts how many scratch directories this test process has made, to keep their names unique.
static SCRATCH_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A scratch directory under the system temp directory that is deleted when dropped.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// Creates a fresh, empty directory.
    fn new() -> Scratch {
        let number = SCRATCH_COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("ember-cli-test-{}-{number}", std::process::id());
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).unwrap();
        Scratch { path }
    }

    /// Writes a file inside the directory, creating parent folders, and returns its path.
    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    /// Creates a sub-directory and returns its path.
    fn make_dir(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Lists the names of everything directly inside `relative` (use "" for the root), sorted.
    fn entries(&self, relative: &str) -> Vec<String> {
        list_entries(&self.path.join(relative))
    }
}

impl Drop for Scratch {
    /// Removes the directory and everything in it, ignoring errors.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Lists the names of everything directly inside `dir`, sorted.
fn list_entries(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        names.push(entry.unwrap().file_name().to_string_lossy().into_owned());
    }
    names.sort();
    names
}

/// Runs `ember` with `dir` as its working directory.
fn run_ember_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ember"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

/// Runs `ember` in `dir` with `TMPDIR` pointing at `tmp`, so the test sees every temp file it makes.
fn run_ember_with_tmpdir(dir: &Path, tmp: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ember"))
        .current_dir(dir)
        .env("TMPDIR", tmp)
        .args(args)
        .output()
        .unwrap()
}

/// The text a process wrote to stdout.
fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The text a process wrote to stderr.
fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Runs the library pipeline on `text` up to the IR, the way the driver should.
fn library_module(file_name: &str, text: &str) -> ir::Module {
    let file = source::SourceFile::new(file_name, text);
    let program = parser::parse_source(text).unwrap();
    let checked = check::check(&program).unwrap();
    ir::lower(&program, &checked, &file).unwrap()
}

/// Runs `arm64::emit`, failing the test with a clear message if the backend reports an error.
fn library_asm(module: &ir::Module, file_name: &str) -> String {
    arm64::emit(module, file_name)
        .map_err(|diagnostic| {
            format!(
                "arm64::emit failed on a checked program: {}",
                diagnostic.message
            )
        })
        .unwrap()
}

// --- --emit ---------------------------------------------------------------

/// `--emit tokens` prints one `L:C KIND` line per token, ending with the end of file.
#[test]
fn emit_tokens_prints_one_token_per_line() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let output = run_ember_in(&dir.path, &["build", "--emit", "tokens", "prog.em"]);

    let expected = "\
1:1 fn
1:4 name main
1:8 (
1:9 )
1:11 {
2:5 name print
2:10 (
2:11 int 42
2:13 )
2:14 newline
3:1 }
3:2 newline
4:1 end of file
";
    assert_eq!(stdout_of(&output), expected);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_of(&output), "");
}

/// Float and string tokens show their value, a string without decoding.
#[test]
fn emit_tokens_shows_float_and_string_values() {
    let dir = Scratch::new();
    dir.write("prog.em", "fn main() {\n    x := 1.5\n    y := \"hi\"\n}\n");

    let output = run_ember_in(&dir.path, &["build", "--emit", "tokens", "prog.em"]);

    let stdout = stdout_of(&output);
    assert!(
        stdout.contains("\n2:10 float 1.5\n"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("\n3:10 string \"hi\"\n"),
        "stdout was: {stdout}"
    );
    assert_eq!(output.status.code(), Some(0));
}

/// `--emit tokens` stops after the lexer, so a program the parser rejects still prints.
#[test]
fn emit_tokens_succeeds_when_the_parser_would_reject() {
    let dir = Scratch::new();
    dir.write("prog.em", "fn main() {\n    x := 1.5\n}\n");

    let output = run_ember_in(&dir.path, &["build", "--emit", "tokens", "prog.em"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_of(&output), "");
    assert!(stdout_of(&output).starts_with("1:1 fn\n"));
}

/// `--emit tokens` stops after the lexer, so a program the checker rejects still prints.
#[test]
fn emit_tokens_succeeds_when_the_checker_would_reject() {
    let dir = Scratch::new();
    dir.write("bad.em", CHECK_ERROR);

    let output = run_ember_in(&dir.path, &["build", "--emit", "tokens", "bad.em"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_of(&output), "");
}

/// A lexer error under `--emit tokens` is rendered on stderr, with exit code 1 and no stdout.
#[test]
fn emit_tokens_reports_a_lexer_error() {
    let dir = Scratch::new();
    dir.write("bad.em", "fn main() {\n    x := 5 $ 3\n}\n");

    let output = run_ember_in(&dir.path, &["build", "--emit", "tokens", "bad.em"]);

    let expected = "\
bad.em:2:12: error: unexpected character `$`
        x := 5 $ 3
               ^
";
    assert_eq!(stderr_of(&output), expected);
    assert_eq!(stdout_of(&output), "");
    assert_eq!(output.status.code(), Some(1));
}

/// `--emit ast` prints the syntax tree in the printer's format.
#[test]
fn emit_ast_prints_the_tree() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let output = run_ember_in(&dir.path, &["build", "--emit", "ast", "prog.em"]);

    assert_eq!(stdout_of(&output), "fn main()\n  (call print 42)\n");
    assert_eq!(output.status.code(), Some(0));
}

/// `--emit ast` equals `ast::print_program` on the same file.
#[test]
fn emit_ast_equals_the_library_output() {
    let dir = Scratch::new();
    dir.write("prog.em", FIB);

    let output = run_ember_in(&dir.path, &["build", "--emit", "ast", "prog.em"]);

    let program = parser::parse_source(FIB).unwrap();
    assert_eq!(stdout_of(&output), ast::print_program(&program));
    assert_eq!(output.status.code(), Some(0));
}

/// `--emit ast` stops after the parser, so a program the checker rejects still prints.
#[test]
fn emit_ast_succeeds_when_the_checker_would_reject() {
    let dir = Scratch::new();
    dir.write("bad.em", CHECK_ERROR);

    let output = run_ember_in(&dir.path, &["build", "--emit", "ast", "bad.em"]);

    let program = parser::parse_source(CHECK_ERROR).unwrap();
    assert_eq!(stdout_of(&output), ast::print_program(&program));
    assert_eq!(stderr_of(&output), "");
    assert_eq!(output.status.code(), Some(0));
}

/// A parse error under `--emit ast` is rendered on stderr with exit code 1.
#[test]
fn emit_ast_reports_a_parse_error() {
    let dir = Scratch::new();
    dir.write("bad.em", "fn main() {\n    x := 1.5\n}\n");

    let output = run_ember_in(&dir.path, &["build", "--emit", "ast", "bad.em"]);

    let expected = "\
bad.em:2:10: error: float literals are not supported yet
        x := 1.5
             ^
";
    assert_eq!(stderr_of(&output), expected);
    assert_eq!(stdout_of(&output), "");
    assert_eq!(output.status.code(), Some(1));
}

/// `--emit ir` equals `ir::print_module` on the same file.
#[test]
fn emit_ir_equals_the_library_output() {
    let dir = Scratch::new();
    dir.write("prog.em", FIB);

    let output = run_ember_in(&dir.path, &["build", "--emit", "ir", "prog.em"]);

    let module = library_module("prog.em", FIB);
    assert_eq!(stdout_of(&output), ir::print_module(&module));
    assert_eq!(output.status.code(), Some(0));
}

/// `--emit ir` needs a checked program, so a checker error is rendered with exit code 1.
#[test]
fn emit_ir_reports_a_checker_error() {
    let dir = Scratch::new();
    dir.write("bad.em", CHECK_ERROR);

    let output = run_ember_in(&dir.path, &["build", "--emit", "ir", "bad.em"]);

    assert_eq!(stderr_of(&output), CHECK_ERROR_RENDERED);
    assert_eq!(stdout_of(&output), "");
    assert_eq!(output.status.code(), Some(1));
}

/// `--emit asm` equals `arm64::emit` on the same file, with the file name as given.
#[test]
fn emit_asm_equals_the_library_output() {
    let dir = Scratch::new();
    dir.write("prog.em", FIB);

    let output = run_ember_in(&dir.path, &["build", "--emit", "asm", "prog.em"]);

    let module = library_module("prog.em", FIB);
    assert_eq!(stdout_of(&output), library_asm(&module, "prog.em"));
    assert_eq!(output.status.code(), Some(0));
}

/// The file name shown in the assembly (and so in panics) is the path exactly as given.
#[test]
fn emit_asm_uses_the_path_as_given() {
    let dir = Scratch::new();
    dir.write("sub/prog.em", FIB);

    let output = run_ember_in(&dir.path, &["build", "--emit", "asm", "sub/prog.em"]);

    let module = library_module("sub/prog.em", FIB);
    assert_eq!(stdout_of(&output), library_asm(&module, "sub/prog.em"));
}

/// Every `--emit` kind prints to stdout only and writes no file.
#[test]
fn emit_writes_no_files() {
    for kind in ["tokens", "ast", "ir", "asm"] {
        let dir = Scratch::new();
        dir.write("prog.em", HELLO);

        let output = run_ember_in(&dir.path, &["build", "--emit", kind, "prog.em"]);

        assert_eq!(output.status.code(), Some(0), "kind {kind}");
        assert_eq!(dir.entries(""), ["prog.em"], "kind {kind}");
    }
}

/// An unknown `--emit` kind is a usage error.
#[test]
fn emit_unknown_kind_exits_with_code_two() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let output = run_ember_in(&dir.path, &["build", "--emit", "bogus", "prog.em"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout_of(&output), "");
}

/// `-o` together with `--emit` is a usage error and writes nothing.
#[test]
fn output_with_emit_exits_with_code_two() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let output = run_ember_in(
        &dir.path,
        &["build", "--emit", "tokens", "-o", "out", "prog.em"],
    );

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout_of(&output), "");
    assert!(!stderr_of(&output).is_empty());
    assert_eq!(dir.entries(""), ["prog.em"]);
}

/// `build` without a file is a usage error.
#[test]
fn build_without_a_file_exits_with_code_two() {
    let dir = Scratch::new();

    let output = run_ember_in(&dir.path, &["build"]);

    assert_eq!(output.status.code(), Some(2));
}

/// `run` without a file is a usage error.
#[test]
fn run_without_a_file_exits_with_code_two() {
    let dir = Scratch::new();

    let output = run_ember_in(&dir.path, &["run"]);

    assert_eq!(output.status.code(), Some(2));
}

/// `build --help` documents the `--emit` and `-o` options.
#[test]
fn build_help_mentions_emit_and_output() {
    let dir = Scratch::new();

    let output = run_ember_in(&dir.path, &["build", "--help"]);

    let stdout = stdout_of(&output);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout.contains("--emit"), "stdout was: {stdout}");
    assert!(stdout.contains("-o"), "stdout was: {stdout}");
}

// --- build ----------------------------------------------------------------

/// `build -o` writes an executable that runs and prints the program's output.
#[test]
fn build_with_output_makes_a_runnable_file() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let build = run_ember_in(&dir.path, &["build", "prog.em", "-o", "hello"]);

    assert_eq!(
        build.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&build)
    );
    assert_eq!(stdout_of(&build), "");
    let mode = fs::metadata(dir.path.join("hello"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0, "the output file is not executable");
    let run = Command::new(dir.path.join("hello")).output().unwrap();
    assert_eq!(stdout_of(&run), "42\n");
    assert_eq!(run.status.code(), Some(0));
}

/// `build` leaves only the executable behind: the `.s` and `.o` files go to a temp directory.
#[test]
fn build_leaves_only_the_output_file() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let build = run_ember_in(&dir.path, &["build", "prog.em", "-o", "hello"]);

    assert_eq!(
        build.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&build)
    );
    assert_eq!(dir.entries(""), ["hello", "prog.em"]);
}

/// Without `-o` the executable is named after the source file's stem, in the current directory.
#[test]
fn build_default_output_is_the_stem_in_the_current_directory() {
    let dir = Scratch::new();
    dir.write("hello.em", HELLO);

    let build = run_ember_in(&dir.path, &["build", "hello.em"]);

    assert_eq!(
        build.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&build)
    );
    assert_eq!(dir.entries(""), ["hello", "hello.em"]);
    let run = Command::new(dir.path.join("hello")).output().unwrap();
    assert_eq!(stdout_of(&run), "42\n");
}

/// For a source in another folder the default output is still in the current directory.
#[test]
fn build_default_output_ignores_the_source_folder() {
    let dir = Scratch::new();
    dir.write("sub/prog.em", HELLO);

    let build = run_ember_in(&dir.path, &["build", "sub/prog.em"]);

    assert_eq!(
        build.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&build)
    );
    assert_eq!(dir.entries(""), ["prog", "sub"]);
    assert_eq!(dir.entries("sub"), ["prog.em"]);
}

/// A built program that panics exits with 101 and prints the panic with the path given to `build`.
#[test]
fn built_program_panics_with_exit_code_101() {
    let dir = Scratch::new();
    dir.write("sub/prog.em", PANICS);

    let build = run_ember_in(&dir.path, &["build", "sub/prog.em", "-o", "prog"]);
    assert_eq!(
        build.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&build)
    );
    let run = Command::new(dir.path.join("prog")).output().unwrap();

    assert_eq!(run.status.code(), Some(101));
    assert_eq!(stdout_of(&run), "1\n2\n");
    assert_eq!(
        stderr_of(&run),
        "panic: integer overflow at sub/prog.em:5:11\n"
    );
}

/// A compile error makes `build` exit with 1, print the rendered error, and write no output.
#[test]
fn build_compile_error_exits_with_code_one() {
    let dir = Scratch::new();
    dir.write("bad.em", CHECK_ERROR);

    let output = run_ember_in(&dir.path, &["build", "bad.em", "-o", "out"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr_of(&output), CHECK_ERROR_RENDERED);
    assert_eq!(stdout_of(&output), "");
    assert_eq!(dir.entries(""), ["bad.em"]);
}

/// `build` removes its temporary directory when it succeeds.
#[test]
fn build_leaves_no_temp_directory_behind() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);
    let tmp = dir.make_dir("tmp");

    let output = run_ember_with_tmpdir(&dir.path, &tmp, &["build", "prog.em", "-o", "hello"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    assert_eq!(list_entries(&tmp), Vec::<String>::new());
}

// --- compile errors -------------------------------------------------------

/// A checker error: exit code 1, the rendered error on stderr, nothing on stdout.
#[test]
fn run_compile_error_exits_with_code_one_and_renders_the_error() {
    let dir = Scratch::new();
    dir.write("bad.em", CHECK_ERROR);

    let output = run_ember_in(&dir.path, &["run", "bad.em"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr_of(&output), CHECK_ERROR_RENDERED);
    assert_eq!(stdout_of(&output), "");
}

/// Every checker error is rendered, in source order.
#[test]
fn run_renders_all_checker_errors_in_order() {
    let dir = Scratch::new();
    dir.write(
        "bad.em",
        "fn main() {\n    print(a)\n    print(b)\n    print(c)\n}\n",
    );

    let output = run_ember_in(&dir.path, &["run", "bad.em"]);

    let expected = "\
bad.em:2:11: error: undefined name `a`
        print(a)
              ^
bad.em:3:11: error: undefined name `b`
        print(b)
              ^
bad.em:4:11: error: undefined name `c`
        print(c)
              ^
";
    assert_eq!(stderr_of(&output), expected);
    assert_eq!(output.status.code(), Some(1));
}

/// The parser stops at its first error, so only one is rendered.
#[test]
fn run_renders_only_the_first_parse_error() {
    let dir = Scratch::new();
    dir.write("bad.em", "fn main() {\n    x := 1.5\n    y := \"s\"\n}\n");

    let output = run_ember_in(&dir.path, &["run", "bad.em"]);

    let expected = "\
bad.em:2:10: error: float literals are not supported yet
        x := 1.5
             ^
";
    assert_eq!(stderr_of(&output), expected);
    assert_eq!(output.status.code(), Some(1));
}

/// Diagnostics show the file path exactly as it was given on the command line.
#[test]
fn diagnostics_show_the_path_as_given() {
    let dir = Scratch::new();
    dir.write("sub/bad.em", CHECK_ERROR);

    let output = run_ember_in(&dir.path, &["run", "sub/bad.em"]);

    let expected = CHECK_ERROR_RENDERED.replace("bad.em:", "sub/bad.em:");
    assert_eq!(stderr_of(&output), expected);
}

// --- unreadable input -----------------------------------------------------

/// A missing file: `error: cannot read FILE: <io error>` and exit code 1.
#[test]
fn missing_file_exits_with_code_one() {
    let dir = Scratch::new();
    let io_error = fs::read(dir.path.join("missing.em")).unwrap_err();

    let commands: [&[&str]; 2] = [&["run", "missing.em"], &["build", "missing.em"]];

    for command in commands {
        let output = run_ember_in(&dir.path, command);

        let expected = format!("error: cannot read missing.em: {io_error}\n");
        assert_eq!(stderr_of(&output), expected, "command {command:?}");
        assert_eq!(stdout_of(&output), "", "command {command:?}");
        assert_eq!(output.status.code(), Some(1), "command {command:?}");
    }
}

/// A directory given as the file cannot be read either.
#[test]
fn directory_as_file_exits_with_code_one() {
    let dir = Scratch::new();
    dir.make_dir("folder.em");
    let io_error = fs::read(dir.path.join("folder.em")).unwrap_err();

    let output = run_ember_in(&dir.path, &["run", "folder.em"]);

    let expected = format!("error: cannot read folder.em: {io_error}\n");
    assert_eq!(stderr_of(&output), expected);
    assert_eq!(output.status.code(), Some(1));
}

/// A file that is not UTF-8 is reported by every command, with exit code 1.
#[test]
fn non_utf8_file_exits_with_code_one() {
    let dir = Scratch::new();
    dir.write("bad.em", [0xff, 0xfe, b'\n']);
    let commands: [&[&str]; 3] = [
        &["run", "bad.em"],
        &["build", "bad.em", "-o", "out"],
        &["build", "--emit", "tokens", "bad.em"],
    ];

    for command in commands {
        let output = run_ember_in(&dir.path, command);

        assert_eq!(
            stderr_of(&output),
            "error: bad.em is not valid UTF-8\n",
            "command {command:?}"
        );
        assert_eq!(stdout_of(&output), "", "command {command:?}");
        assert_eq!(output.status.code(), Some(1), "command {command:?}");
    }
}

// --- run ------------------------------------------------------------------

/// `run` passes the program's stdout through and exits with 0.
#[test]
fn run_passes_stdout_and_exit_code_zero() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let output = run_ember_in(&dir.path, &["run", "prog.em"]);

    assert_eq!(stdout_of(&output), "42\n");
    assert_eq!(stderr_of(&output), "");
    assert_eq!(output.status.code(), Some(0));
}

/// A panic exits with 101 and prints `panic: <reason> at FILE:L:C` using the path as given.
#[test]
fn run_passes_through_exit_code_101_on_a_panic() {
    let dir = Scratch::new();
    dir.write("sub/prog.em", PANICS);

    let output = run_ember_in(&dir.path, &["run", "sub/prog.em"]);

    assert_eq!(output.status.code(), Some(101));
    assert_eq!(
        stderr_of(&output),
        "panic: integer overflow at sub/prog.em:5:11\n"
    );
}

/// Everything printed before a panic is still written to stdout.
#[test]
fn run_keeps_stdout_printed_before_a_panic() {
    let dir = Scratch::new();
    dir.write("prog.em", PANICS);

    let output = run_ember_in(&dir.path, &["run", "prog.em"]);

    assert_eq!(stdout_of(&output), "1\n2\n");
    assert_eq!(output.status.code(), Some(101));
}

/// `run` leaves no executable or other file in the current directory.
#[test]
fn run_leaves_the_current_directory_untouched() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);

    let output = run_ember_in(&dir.path, &["run", "prog.em"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(dir.entries(""), ["prog.em"]);
}

/// `run` removes its temporary directory after a normal run, a panic and a compile error.
#[test]
fn run_leaves_no_temp_directory_behind() {
    let cases = [
        (HELLO, 0),
        (PANICS, 101),
        (CHECK_ERROR, 1),
        ("fn main() {\n    x := $\n}\n", 1),
    ];

    for (source, expected_code) in cases {
        let dir = Scratch::new();
        dir.write("prog.em", source);
        let tmp = dir.make_dir("tmp");

        let output = run_ember_with_tmpdir(&dir.path, &tmp, &["run", "prog.em"]);

        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "source: {source}"
        );
        assert_eq!(list_entries(&tmp), Vec::<String>::new(), "source: {source}");
    }
}

/// Two `run`s at once do not share a temp directory, so both give the right output.
#[test]
fn concurrent_runs_do_not_interfere() {
    let dir = Scratch::new();
    dir.write("prog.em", FIB);
    let tmp = dir.make_dir("tmp");
    let mut handles = Vec::new();

    for _ in 0..4 {
        let dir_path = dir.path.clone();
        let tmp_path = tmp.clone();
        handles.push(std::thread::spawn(move || {
            run_ember_with_tmpdir(&dir_path, &tmp_path, &["run", "prog.em"])
        }));
    }

    for handle in handles {
        let output = handle.join().unwrap();
        assert_eq!(stdout_of(&output), "0\n1\n1\n2\n3\n5\n");
        assert_eq!(output.status.code(), Some(0));
    }
    assert_eq!(list_entries(&tmp), Vec::<String>::new());
}

/// When the temp directory cannot be created, `run` reports an error and exits with 1 (no panic).
#[test]
fn run_reports_an_unusable_temp_directory() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);
    let missing = dir.path.join("no-such-tmp");

    let output = run_ember_with_tmpdir(&dir.path, &missing, &["run", "prog.em"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).starts_with("error: "),
        "stderr was: {}",
        stderr_of(&output)
    );
    assert_eq!(stdout_of(&output), "");
}

/// A function whose frame is far above 32 KB (1,500 statements) still compiles and runs correctly.
#[test]
fn run_handles_a_very_large_frame() {
    let dir = Scratch::new();
    let mut source = String::from("fn main() {\n    x := 0\n");
    for _ in 0..1500 {
        source.push_str("    x = x + 1\n");
    }
    source.push_str("    print(x)\n}\n");
    dir.write("big.em", source);

    let output = run_ember_in(&dir.path, &["run", "big.em"]);

    assert_eq!(stderr_of(&output), "");
    assert_eq!(stdout_of(&output), "1500\n");
    assert_eq!(output.status.code(), Some(0));
}

// --- output must not overwrite the source ---------------------------------

/// Asserts that `build` refused `output_arg` as the output, with the exact message, and left the source alone.
fn assert_overwrite_refused(dir: &Path, args: &[&str], output_arg: &str, source: &Path) {
    let before = fs::read(source).unwrap();

    let output = run_ember_in(dir, args);

    let expected = format!("error: the output {output_arg} would overwrite the source file\n");
    assert_eq!(stderr_of(&output), expected, "args {args:?}");
    assert_eq!(stdout_of(&output), "", "args {args:?}");
    assert_eq!(output.status.code(), Some(1), "args {args:?}");
    assert_eq!(fs::read(source).unwrap(), before, "args {args:?}");
}

/// `-o` with the source's own name is refused and the source is untouched.
#[test]
fn build_refuses_the_source_name_as_output() {
    let dir = Scratch::new();
    let source = dir.write("x.em", HELLO);

    assert_overwrite_refused(&dir.path, &["build", "x.em", "-o", "x.em"], "x.em", &source);
}

/// `-o ./x.em` is the same file as `x.em` and is refused.
#[test]
fn build_refuses_a_dot_slash_spelling_of_the_source() {
    let dir = Scratch::new();
    let source = dir.write("x.em", HELLO);

    assert_overwrite_refused(
        &dir.path,
        &["build", "x.em", "-o", "./x.em"],
        "./x.em",
        &source,
    );
}

/// An absolute path to the source is refused.
#[test]
fn build_refuses_an_absolute_path_to_the_source() {
    let dir = Scratch::new();
    let source = dir.write("x.em", HELLO);
    let absolute = source.to_string_lossy().into_owned();

    assert_overwrite_refused(
        &dir.path,
        &["build", "x.em", "-o", &absolute],
        &absolute,
        &source,
    );
}

/// A `../dir/x.em` path from a subdirectory is refused.
#[test]
fn build_refuses_a_parent_relative_path_to_the_source() {
    let dir = Scratch::new();
    let source = dir.write("project/x.em", HELLO);
    let sub = dir.make_dir("project/sub");

    assert_overwrite_refused(
        &sub,
        &["build", "../x.em", "-o", "../../project/x.em"],
        "../../project/x.em",
        &source,
    );
}

/// A symbolic link to the source is the same file, so it is refused too.
#[test]
fn build_refuses_a_symlink_to_the_source() {
    let dir = Scratch::new();
    let source = dir.write("x.em", HELLO);
    std::os::unix::fs::symlink(&source, dir.path.join("link")).unwrap();

    assert_overwrite_refused(&dir.path, &["build", "x.em", "-o", "link"], "link", &source);
}

// --- relative TMPDIR ------------------------------------------------------

/// A relative `TMPDIR` that looks like a clang option (`-x`) is used safely by `run`.
#[test]
fn run_works_with_a_relative_tmpdir_that_looks_like_an_option() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);
    dir.make_dir("-x");

    let output = run_ember_with_tmpdir(&dir.path, Path::new("-x"), &["run", "prog.em"]);

    assert_eq!(stderr_of(&output), "");
    assert_eq!(stdout_of(&output), "42\n");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(dir.entries("-x"), Vec::<String>::new());
}

/// A relative `TMPDIR` that looks like a clang option is used safely by `build`.
#[test]
fn build_works_with_a_relative_tmpdir_that_looks_like_an_option() {
    let dir = Scratch::new();
    dir.write("prog.em", HELLO);
    dir.make_dir("-x");

    let output = run_ember_with_tmpdir(
        &dir.path,
        Path::new("-x"),
        &["build", "prog.em", "-o", "hello"],
    );

    assert_eq!(stderr_of(&output), "");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(dir.entries("-x"), Vec::<String>::new());
    let run = Command::new(dir.path.join("hello")).output().unwrap();
    assert_eq!(stdout_of(&run), "42\n");
}

// ---------------------------------------------------------------------------
// M2.1c: the frame limit, seen from source
// ---------------------------------------------------------------------------

/// The largest expression decision 0025 allows: 500 ones added together (999 nodes, 999 cells in the frame).
fn five_hundred_ones() -> String {
    vec!["1"; 500].join(" + ")
}

/// A `main` that sets `x` once and then assigns it `assignments` more times, each from 500 ones, and prints it.
fn big_frame_program(assignments: usize) -> String {
    let expression = five_hundred_ones();
    let mut text = format!("fn main() {{\n    x := {expression}\n");
    for _ in 0..assignments {
        text.push_str(&format!("    x = {expression}\n"));
    }
    text.push_str("    print(x)\n}\n");
    text
}

/// A frame of 4,195,824 bytes (524,477 cells) makes `build` fail with a located error and exit 1.
#[test]
fn build_rejects_a_frame_over_4_mib() {
    let dir = Scratch::new();
    dir.write("big.em", big_frame_program(524));

    let output = run_ember_in(&dir.path, &["build", "big.em", "-o", "out"]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr_of(&output);
    assert_eq!(
        stderr.lines().next().unwrap(),
        "big.em:1:4: error: stack frame of `main` is too large: 4195824 bytes (the limit is 4194304 bytes)"
    );
    assert_eq!(stdout_of(&output), "");
    assert_eq!(dir.entries(""), ["big.em"]);
}

/// `run` reports the same frame error with exit 1.
#[test]
fn run_rejects_a_frame_over_4_mib() {
    let dir = Scratch::new();
    dir.write("big.em", big_frame_program(524));

    let output = run_ember_in(&dir.path, &["run", "big.em"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr_of(&output).starts_with("big.em:1:4: error: stack frame of `main` is too large"),
        "stderr: {}",
        stderr_of(&output)
    );
    assert_eq!(stdout_of(&output), "");
}

/// The same program with 400 assignments (a frame of about 3.2 MB) builds, runs and prints 500.
#[test]
fn run_accepts_a_frame_under_4_mib() {
    let dir = Scratch::new();
    dir.write("big.em", big_frame_program(400));

    let output = run_ember_in(&dir.path, &["run", "big.em"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    assert_eq!(stdout_of(&output), "500\n");
}
