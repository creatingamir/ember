//! Integration tests for the public API of the `golden` library.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use golden::{Case, Error, Expected, Outcome, bless, check, discover};

/// A fresh temporary directory that is removed when dropped.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates an empty directory unique to this process and test name.
    fn new(test_name: &str) -> Self {
        let folder = format!("golden_lib_{}_{}", std::process::id(), test_name);
        let path = std::env::temp_dir().join(folder);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }

    /// Returns the path of the directory.
    fn path(&self) -> &Path {
        &self.path
    }

    /// Writes a file at a relative path, creating parent directories.
    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let file = self.path.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, contents).unwrap();
        file
    }

    /// Reads a file at a relative path.
    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path.join(relative)).unwrap()
    }

    /// Tells whether a file exists at a relative path.
    fn exists(&self, relative: &str) -> bool {
        self.path.join(relative).exists()
    }
}

impl Drop for TempDir {
    /// Removes the directory and everything in it.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Builds an outcome from its three fields.
fn outcome(stdout: &str, stderr: &str, success: bool) -> Outcome {
    Outcome {
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        success,
    }
}

/// Writes `case.em` and `case.out` and returns the matching case.
fn success_case(dir: &TempDir, expected_stdout: &str) -> Case {
    let source = dir.write("case.em", "program");
    let stdout = dir.write("case.out", expected_stdout);
    Case {
        name: "case".to_string(),
        source,
        expected: Some(Expected::Success { stdout }),
    }
}

/// Writes `case.em` and `case.err` and returns the matching case.
fn failure_case(dir: &TempDir, expected_stderr: &str) -> Case {
    let source = dir.write("case.em", "program");
    let stderr = dir.write("case.err", expected_stderr);
    Case {
        name: "case".to_string(),
        source,
        expected: Some(Expected::Failure { stderr }),
    }
}

/// Writes `case.em` only and returns a case with no expectation.
fn unblessed_case(dir: &TempDir) -> Case {
    let source = dir.write("case.em", "program");
    Case {
        name: "case".to_string(),
        source,
        expected: None,
    }
}

/// Finds the case with the given name in a discovered list.
fn find<'a>(cases: &'a [Case], name: &str) -> &'a Case {
    cases.iter().find(|case| case.name == name).unwrap()
}

/// An empty directory has no cases.
#[test]
fn discover_empty_dir_returns_no_cases() {
    let dir = TempDir::new("discover_empty_dir");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases, vec![]);
}

/// A missing directory is an Io error carrying that path.
#[test]
fn discover_missing_dir_is_io_error_with_that_path() {
    let dir = TempDir::new("discover_missing_dir");
    let missing = dir.path().join("does_not_exist");

    let result = discover(&missing);

    assert!(matches!(result, Err(Error::Io { path, .. }) if path == missing));
}

/// A top-level program gets its file stem as the case name.
#[test]
fn discover_names_top_level_case_without_extension() {
    let dir = TempDir::new("discover_top_level_name");
    let source = dir.write("fib.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "fib");
    assert_eq!(cases[0].source, source);
}

/// Programs in subdirectories are found and named with `/` separators.
#[test]
fn discover_walks_subdirectories_with_slash_separated_names() {
    let dir = TempDir::new("discover_recursive");
    dir.write("fib.em", "program");
    let nested = dir.write("errors/undefined.em", "program");
    let deep = dir.write("a/b/c.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases.len(), 3);
    assert_eq!(find(&cases, "errors/undefined").source, nested);
    assert_eq!(find(&cases, "a/b/c").source, deep);
    assert_eq!(find(&cases, "fib").name, "fib");
}

/// Cases come back sorted by name, whatever the directory layout.
#[test]
fn discover_sorts_cases_by_name() {
    let dir = TempDir::new("discover_sorted");
    dir.write("b.em", "program");
    dir.write("errors/x.em", "program");
    dir.write("c.em", "program");
    dir.write("a.em", "program");

    let cases = discover(dir.path()).unwrap();

    let names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "c", "errors/x"]);
}

/// Hidden files such as `.gitkeep` do not produce cases or errors.
#[test]
fn discover_ignores_gitkeep() {
    let dir = TempDir::new("discover_gitkeep");
    dir.write(".gitkeep", "");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases, vec![]);
}

/// Files with other extensions are ignored next to real cases.
#[test]
fn discover_ignores_unrelated_files() {
    let dir = TempDir::new("discover_unrelated");
    dir.write(".gitkeep", "");
    dir.write("README.md", "notes");
    dir.write("notes.txt", "notes");
    dir.write("sub/data.json", "{}");
    dir.write("fib.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "fib");
}

/// A program with a `.out` file expects success with that file.
#[test]
fn discover_pairs_out_file_as_success_expectation() {
    let dir = TempDir::new("discover_out");
    dir.write("fib.em", "program");
    let stdout = dir.write("fib.out", "55\n");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases[0].expected, Some(Expected::Success { stdout }));
}

/// A program with a `.err` file expects failure with that file.
#[test]
fn discover_pairs_err_file_as_failure_expectation() {
    let dir = TempDir::new("discover_err");
    dir.write("bad.em", "program");
    let stderr = dir.write("bad.err", "error: oops\n");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases[0].expected, Some(Expected::Failure { stderr }));
}

/// A program with neither expectation file has `expected` set to None.
#[test]
fn discover_leaves_expected_none_without_expectation_files() {
    let dir = TempDir::new("discover_none");
    dir.write("fresh.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases[0].expected, None);
}

/// Expectation files in subdirectories pair with their sibling program.
#[test]
fn discover_pairs_expectations_in_subdirectories() {
    let dir = TempDir::new("discover_nested_expectation");
    dir.write("errors/undefined.em", "program");
    let stderr = dir.write("errors/undefined.err", "error\n");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases[0].name, "errors/undefined");
    assert_eq!(cases[0].expected, Some(Expected::Failure { stderr }));
}

/// Having both `.out` and `.err` for one program is an error naming the source.
#[test]
fn discover_rejects_conflicting_expectations() {
    let dir = TempDir::new("discover_conflict");
    let source = dir.write("both.em", "program");
    dir.write("both.out", "ok\n");
    dir.write("both.err", "error\n");

    let result = discover(dir.path());

    assert!(
        matches!(result, Err(Error::ConflictingExpectations { source: found }) if found == source)
    );
}

/// A `.out` file without a program is an orphan error naming that file.
#[test]
fn discover_rejects_orphan_out_file() {
    let dir = TempDir::new("discover_orphan_out");
    let orphan = dir.write("ghost.out", "ok\n");

    let result = discover(dir.path());

    assert!(matches!(result, Err(Error::OrphanExpectation { path }) if path == orphan));
}

/// A `.err` file without a program is an orphan error naming that file.
#[test]
fn discover_rejects_orphan_err_file() {
    let dir = TempDir::new("discover_orphan_err");
    let orphan = dir.write("ghost.err", "error\n");

    let result = discover(dir.path());

    assert!(matches!(result, Err(Error::OrphanExpectation { path }) if path == orphan));
}

/// An orphan expectation inside a subdirectory is still detected.
#[test]
fn discover_rejects_orphan_in_subdirectory() {
    let dir = TempDir::new("discover_orphan_nested");
    dir.write("fib.em", "program");
    let orphan = dir.write("errors/ghost.err", "error\n");

    let result = discover(dir.path());

    assert!(matches!(result, Err(Error::OrphanExpectation { path }) if path == orphan));
}

/// Checking a case with no expectation reports the missing expectation.
#[test]
fn check_without_expectation_is_missing_expectation() {
    let dir = TempDir::new("check_missing");
    let case = unblessed_case(&dir);

    let result = check(&case, &outcome("out", "", true));

    assert!(matches!(result, Err(Error::MissingExpectation { source }) if source == case.source));
}

/// A successful run whose stdout equals the `.out` file passes.
#[test]
fn check_success_with_matching_stdout_passes() {
    let dir = TempDir::new("check_success_match");
    let case = success_case(&dir, "55\n");

    let result = check(&case, &outcome("55\n", "", true));

    assert!(result.is_ok());
}

/// A successful run with different stdout is an output mismatch.
#[test]
fn check_success_with_different_stdout_is_mismatch() {
    let dir = TempDir::new("check_stdout_mismatch");
    let case = success_case(&dir, "55\n");

    let result = check(&case, &outcome("56\n", "", true));

    assert!(matches!(
        result,
        Err(Error::OutputMismatch { expected, actual }) if expected == "55\n" && actual == "56\n"
    ));
}

/// Stderr is not compared when success is expected.
#[test]
fn check_success_ignores_stderr() {
    let dir = TempDir::new("check_success_ignores_stderr");
    let case = success_case(&dir, "55\n");

    let result = check(&case, &outcome("55\n", "warning: noisy\n", true));

    assert!(result.is_ok());
}

/// A failed run when success is expected carries its stderr.
#[test]
fn check_failed_run_when_success_expected_is_unexpected_failure() {
    let dir = TempDir::new("check_unexpected_failure");
    let case = success_case(&dir, "55\n");

    let result = check(&case, &outcome("55\n", "boom\n", false));

    assert!(matches!(
        result,
        Err(Error::UnexpectedFailure { stderr }) if stderr == "boom\n"
    ));
}

/// A successful run when failure is expected carries its stdout.
#[test]
fn check_successful_run_when_failure_expected_is_unexpected_success() {
    let dir = TempDir::new("check_unexpected_success");
    let case = failure_case(&dir, "error: oops\n");

    let result = check(&case, &outcome("surprise\n", "", true));

    assert!(matches!(
        result,
        Err(Error::UnexpectedSuccess { stdout }) if stdout == "surprise\n"
    ));
}

/// A failed run whose stderr equals the `.err` file passes.
#[test]
fn check_failure_with_matching_stderr_passes() {
    let dir = TempDir::new("check_failure_match");
    let case = failure_case(&dir, "error: oops\n");

    let result = check(&case, &outcome("", "error: oops\n", false));

    assert!(result.is_ok());
}

/// A failed run with different stderr is an output mismatch.
#[test]
fn check_failure_with_different_stderr_is_mismatch() {
    let dir = TempDir::new("check_stderr_mismatch");
    let case = failure_case(&dir, "error: oops\n");

    let result = check(&case, &outcome("", "error: other\n", false));

    assert!(matches!(
        result,
        Err(Error::OutputMismatch { expected, actual })
            if expected == "error: oops\n" && actual == "error: other\n"
    ));
}

/// Stdout is not compared when failure is expected.
#[test]
fn check_failure_ignores_stdout() {
    let dir = TempDir::new("check_failure_ignores_stdout");
    let case = failure_case(&dir, "error: oops\n");

    let result = check(&case, &outcome("partial output\n", "error: oops\n", false));

    assert!(result.is_ok());
}

/// A missing trailing newline in actual stdout is a mismatch.
#[test]
fn check_missing_trailing_newline_is_mismatch() {
    let dir = TempDir::new("check_missing_newline");
    let case = success_case(&dir, "55\n");

    let result = check(&case, &outcome("55", "", true));

    assert!(matches!(result, Err(Error::OutputMismatch { .. })));
}

/// An extra trailing newline in actual stdout is a mismatch.
#[test]
fn check_extra_trailing_newline_is_mismatch() {
    let dir = TempDir::new("check_extra_newline");
    let case = success_case(&dir, "55");

    let result = check(&case, &outcome("55\n", "", true));

    assert!(matches!(result, Err(Error::OutputMismatch { .. })));
}

/// Windows line endings are not normalized to Unix ones.
#[test]
fn check_does_not_normalize_line_endings() {
    let dir = TempDir::new("check_line_endings");
    let case = success_case(&dir, "a\nb\n");

    let result = check(&case, &outcome("a\r\nb\r\n", "", true));

    assert!(matches!(result, Err(Error::OutputMismatch { .. })));
}

/// An unreadable expected stdout file is an Io error with its path.
#[test]
fn check_unreadable_out_file_is_io_error() {
    let dir = TempDir::new("check_unreadable_out");
    let missing = dir.path().join("gone.out");
    let case = Case {
        name: "case".to_string(),
        source: dir.write("case.em", "program"),
        expected: Some(Expected::Success {
            stdout: missing.clone(),
        }),
    };

    let result = check(&case, &outcome("", "", true));

    assert!(matches!(result, Err(Error::Io { path, .. }) if path == missing));
}

/// An unreadable expected stderr file is an Io error with its path.
#[test]
fn check_unreadable_err_file_is_io_error() {
    let dir = TempDir::new("check_unreadable_err");
    let missing = dir.path().join("gone.err");
    let case = Case {
        name: "case".to_string(),
        source: dir.write("case.em", "program"),
        expected: Some(Expected::Failure {
            stderr: missing.clone(),
        }),
    };

    let result = check(&case, &outcome("", "", false));

    assert!(matches!(result, Err(Error::Io { path, .. }) if path == missing));
}

/// Blessing a successful run creates `<name>.out` with the stdout.
#[test]
fn bless_success_creates_out_file_with_stdout() {
    let dir = TempDir::new("bless_creates_out");
    let case = unblessed_case(&dir);

    bless(&case, &outcome("55\n", "ignored\n", true)).unwrap();

    assert_eq!(dir.read("case.out"), "55\n");
    assert!(!dir.exists("case.err"));
}

/// Blessing a failed run creates `<name>.err` with the stderr.
#[test]
fn bless_failure_creates_err_file_with_stderr() {
    let dir = TempDir::new("bless_creates_err");
    let case = unblessed_case(&dir);

    bless(&case, &outcome("ignored\n", "error: oops\n", false)).unwrap();

    assert_eq!(dir.read("case.err"), "error: oops\n");
    assert!(!dir.exists("case.out"));
}

/// Blessing a success replaces an existing `.err` file with a `.out` file.
#[test]
fn bless_success_deletes_existing_err_file() {
    let dir = TempDir::new("bless_success_deletes_err");
    let case = failure_case(&dir, "error: old\n");

    bless(&case, &outcome("now works\n", "", true)).unwrap();

    assert_eq!(dir.read("case.out"), "now works\n");
    assert!(!dir.exists("case.err"));
}

/// Blessing a failure replaces an existing `.out` file with a `.err` file.
#[test]
fn bless_failure_deletes_existing_out_file() {
    let dir = TempDir::new("bless_failure_deletes_out");
    let case = success_case(&dir, "old output\n");

    bless(&case, &outcome("", "error: new\n", false)).unwrap();

    assert_eq!(dir.read("case.err"), "error: new\n");
    assert!(!dir.exists("case.out"));
}

/// Blessing overwrites an existing expectation file of the same kind.
#[test]
fn bless_overwrites_stale_expectation_of_same_kind() {
    let dir = TempDir::new("bless_overwrites_same_kind");
    let case = success_case(&dir, "stale\n");

    bless(&case, &outcome("fresh\n", "", true)).unwrap();

    assert_eq!(dir.read("case.out"), "fresh\n");
}

/// The expectation file is written next to a source in a subdirectory.
#[test]
fn bless_writes_next_to_source_in_subdirectory() {
    let dir = TempDir::new("bless_nested");
    let source = dir.write("errors/undefined.em", "program");
    let case = Case {
        name: "errors/undefined".to_string(),
        source,
        expected: None,
    };

    bless(&case, &outcome("", "error: undefined\n", false)).unwrap();

    assert_eq!(dir.read("errors/undefined.err"), "error: undefined\n");
}

/// Blessing writes the output byte for byte, with no newline added.
#[test]
fn bless_writes_output_exactly() {
    let dir = TempDir::new("bless_exact");
    let case = unblessed_case(&dir);

    bless(&case, &outcome("no newline at end", "", true)).unwrap();

    assert_eq!(dir.read("case.out"), "no newline at end");
}

/// Blessing into a directory that does not exist is an Io error.
#[test]
fn bless_into_missing_directory_is_io_error() {
    let dir = TempDir::new("bless_missing_dir");
    let case = Case {
        name: "case".to_string(),
        source: dir.path().join("missing_folder").join("case.em"),
        expected: None,
    };

    let result = bless(&case, &outcome("55\n", "", true));

    assert!(matches!(result, Err(Error::Io { .. })));
}

/// A blessed success passes `check` when discovered again.
#[test]
fn check_passes_after_blessing_a_success() {
    let dir = TempDir::new("roundtrip_success");
    dir.write("case.em", "program");
    let run = outcome("55\n", "", true);
    let case = find_only_case(dir.path());

    bless(&case, &run).unwrap();

    let blessed = find_only_case(dir.path());
    assert!(check(&blessed, &run).is_ok());
}

/// A blessed failure passes `check` when discovered again.
#[test]
fn check_passes_after_blessing_a_failure() {
    let dir = TempDir::new("roundtrip_failure");
    dir.write("case.em", "program");
    let run = outcome("", "error: oops\n", false);
    let case = find_only_case(dir.path());

    bless(&case, &run).unwrap();

    let blessed = find_only_case(dir.path());
    assert!(check(&blessed, &run).is_ok());
}

/// Discovers a directory that holds exactly one case and returns it.
fn find_only_case(dir: &Path) -> Case {
    let mut cases = discover(dir).unwrap();
    assert_eq!(cases.len(), 1);
    cases.remove(0)
}

/// The mismatch message shows both the expected and the actual text.
#[test]
fn output_mismatch_display_contains_both_texts() {
    let error = Error::OutputMismatch {
        expected: "alpha text".to_string(),
        actual: "beta text".to_string(),
    };

    let message = error.to_string();

    assert!(message.contains("alpha text"));
    assert!(message.contains("beta text"));
}

/// The mismatch message labels which text is expected and which is actual.
#[test]
fn output_mismatch_display_labels_both_texts() {
    let error = Error::OutputMismatch {
        expected: "alpha text".to_string(),
        actual: "beta text".to_string(),
    };

    let message = error.to_string().to_lowercase();

    assert!(message.contains("expected"));
    assert!(message.contains("actual"));
}

/// Every error value can be used as a `std::error::Error` trait object.
#[test]
fn error_implements_std_error() {
    let error = Error::MissingExpectation {
        source: PathBuf::from("case.em"),
    };

    let as_trait_object: &dyn std::error::Error = &error;

    assert!(!as_trait_object.to_string().is_empty());
}

/// Creates a symlink at `link` (relative to `dir`) pointing at `target`.
fn link(dir: &TempDir, link: &str, target: &Path) {
    let link_path = dir.path().join(link);
    fs::create_dir_all(link_path.parent().unwrap()).unwrap();
    symlink(target, link_path).unwrap();
}

/// A symlink pointing back at a parent directory does not cause a loop.
#[test]
fn discover_survives_symlink_loop() {
    let dir = TempDir::new("discover_symlink_loop");
    dir.write("suite/ok.em", "program");
    fs::create_dir_all(dir.path().join("suite/a")).unwrap();
    link(&dir, "suite/a/up", Path::new(".."));

    let cases = discover(&dir.path().join("suite")).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "ok");
}

/// A symlinked directory is skipped, so its programs are not cases.
#[test]
fn discover_skips_symlinked_directory() {
    let dir = TempDir::new("discover_symlinked_dir");
    dir.write("outside/inner.em", "program");
    dir.write("suite/ok.em", "program");
    link(&dir, "suite/linked", &dir.path().join("outside"));

    let cases = discover(&dir.path().join("suite")).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "ok");
}

/// A symlinked `.em` file is skipped, so it is not a case.
#[test]
fn discover_skips_symlinked_source_file() {
    let dir = TempDir::new("discover_symlinked_source");
    let real = dir.write("outside/real.em", "program");
    dir.write("suite/ok.em", "program");
    link(&dir, "suite/linked.em", &real);

    let cases = discover(&dir.path().join("suite")).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "ok");
}

/// A dangling expectation symlink is skipped, leaving `expected` as None.
#[test]
fn discover_skips_dangling_expectation_symlink() {
    let dir = TempDir::new("discover_dangling_symlink");
    dir.write("suite/d.em", "program");
    link(&dir, "suite/d.out", Path::new("/nonexistent"));

    let cases = discover(&dir.path().join("suite")).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "d");
    assert_eq!(cases[0].expected, None);
}

/// Blessing replaces an expectation symlink instead of writing through it.
#[test]
fn bless_does_not_write_through_symlink() {
    let dir = TempDir::new("bless_symlink");
    let outside = dir.write("outside/real.out", "OUTSIDE");
    dir.write("suite/f.em", "program");
    link(&dir, "suite/f.out", &outside);
    let suite = dir.path().join("suite");
    let case = discover(&suite).unwrap().remove(0);

    bless(&case, &outcome("NEW\n", "", true)).unwrap();

    assert_eq!(dir.read("outside/real.out"), "OUTSIDE");
    let blessed = suite.join("f.out");
    assert!(!fs::symlink_metadata(&blessed).unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&blessed).unwrap(), "NEW\n");
}

/// A hidden `.em` file is not a case.
#[test]
fn discover_ignores_hidden_source_file() {
    let dir = TempDir::new("discover_hidden_file");
    dir.write(".secret.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases, vec![]);
}

/// A hidden directory is not searched.
#[test]
fn discover_ignores_hidden_directory() {
    let dir = TempDir::new("discover_hidden_dir");
    dir.write(".hidden/x.em", "program");
    dir.write("ok.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "ok");
}

/// A directory named `x.em` is searched like any directory, not treated as a case.
#[test]
fn discover_recurses_into_directory_named_like_a_source() {
    let dir = TempDir::new("discover_dir_named_em");
    let inner = dir.write("x.em/inner.em", "program");

    let cases = discover(dir.path()).unwrap();

    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].name, "x.em/inner");
    assert_eq!(cases[0].source, inner);
}

/// A `.out` file next to a directory named `x.em` is an orphan.
#[test]
fn discover_treats_out_next_to_directory_named_like_a_source_as_orphan() {
    let dir = TempDir::new("discover_dir_named_em_orphan");
    dir.write("x.em/inner.em", "program");
    let orphan = dir.write("x.out", "ok\n");

    let result = discover(dir.path());

    assert!(matches!(result, Err(Error::OrphanExpectation { path }) if path == orphan));
}

/// A failed bless in a read-only directory is an Io error and leaves files untouched.
#[test]
fn bless_in_read_only_directory_fails_without_partial_write() {
    let dir = TempDir::new("bless_read_only");
    let source = dir.write("p.em", "program");
    let out = dir.write("p.out", "old out\n");
    dir.write("p.err", "old err\n");
    let case = Case {
        name: "p".to_string(),
        source,
        expected: Some(Expected::Success { stdout: out }),
    };
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

    let result = bless(&case, &outcome("new\n", "", true));

    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    // Root ignores directory permissions, so the failure cannot be provoked.
    if std::env::var("USER") == Ok("root".to_string()) {
        return;
    }
    assert!(matches!(result, Err(Error::Io { .. })));
    assert_eq!(dir.read("p.out"), "old out\n");
    assert_eq!(dir.read("p.err"), "old err\n");
}

/// The Io error message contains the path.
#[test]
fn io_display_contains_path() {
    let path = PathBuf::from("some/dir/file.out");
    let error = Error::Io {
        path: path.clone(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "gone"),
    };

    assert!(error.to_string().contains(&path.display().to_string()));
}

/// The orphan error message contains the path.
#[test]
fn orphan_expectation_display_contains_path() {
    let path = PathBuf::from("some/dir/ghost.out");
    let error = Error::OrphanExpectation { path: path.clone() };

    assert!(error.to_string().contains(&path.display().to_string()));
}

/// The conflicting-expectations error message contains the source path.
#[test]
fn conflicting_expectations_display_contains_path() {
    let path = PathBuf::from("some/dir/both.em");
    let error = Error::ConflictingExpectations {
        source: path.clone(),
    };

    assert!(error.to_string().contains(&path.display().to_string()));
}

/// The missing-expectation error message contains the source path.
#[test]
fn missing_expectation_display_contains_path() {
    let path = PathBuf::from("some/dir/fresh.em");
    let error = Error::MissingExpectation {
        source: path.clone(),
    };

    assert!(error.to_string().contains(&path.display().to_string()));
}
