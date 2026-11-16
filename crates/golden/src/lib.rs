//! Golden tests: discover `.em` programs, check their output against expectation files, or bless it.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// One golden test: a `.em` program and what running it should produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// Path relative to the suite dir, without `.em`, with `/` separators.
    pub name: String,
    /// The `.em` source file.
    pub source: PathBuf,
    /// The expectation file, or `None` when neither `.out` nor `.err` exists yet.
    pub expected: Option<Expected>,
}

/// What a case expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// The run must succeed and its stdout must equal the `.out` file exactly.
    Success {
        /// The `name.out` file.
        stdout: PathBuf,
    },
    /// The run must fail and its stderr must equal the `.err` file exactly.
    Failure {
        /// The `name.err` file.
        stderr: PathBuf,
    },
}

/// What running a case produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Everything the program wrote to stdout.
    pub stdout: String,
    /// Everything the program wrote to stderr.
    pub stderr: String,
    /// Whether the program exited successfully.
    pub success: bool,
}

/// Everything that can go wrong while discovering, checking or blessing cases.
#[derive(Debug)]
pub enum Error {
    /// A file or directory could not be read, written or removed.
    Io {
        /// The path the operation failed on.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// Both `name.out` and `name.err` exist.
    ConflictingExpectations {
        /// The `.em` file that has both.
        source: PathBuf,
    },
    /// A `.out` or `.err` file has no matching `.em` file.
    OrphanExpectation {
        /// The expectation file without a program.
        path: PathBuf,
    },
    /// The case has no expectation file, so there is nothing to check against.
    MissingExpectation {
        /// The `.em` file without an expectation.
        source: PathBuf,
    },
    /// A success was expected but the run failed.
    UnexpectedFailure {
        /// What the failed run wrote to stderr.
        stderr: String,
    },
    /// A failure was expected but the run succeeded.
    UnexpectedSuccess {
        /// What the successful run wrote to stdout.
        stdout: String,
    },
    /// The compared stream differs from the expectation file.
    OutputMismatch {
        /// The contents of the expectation file.
        expected: String,
        /// What the run produced.
        actual: String,
    },
}

impl fmt::Display for Error {
    /// Writes a message a person can act on without other tools.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Error::ConflictingExpectations { source } => {
                write!(
                    f,
                    "{}: both a .out and a .err file exist, keep only one",
                    source.display()
                )
            }
            Error::OrphanExpectation { path } => {
                write!(
                    f,
                    "{}: no .em file next to this expectation",
                    path.display()
                )
            }
            Error::MissingExpectation { source } => {
                write!(
                    f,
                    "{}: no .out or .err file; run with EMBER_BLESS=1 to create one",
                    source.display()
                )
            }
            Error::UnexpectedFailure { stderr } => {
                write!(
                    f,
                    "expected the run to succeed, but it failed\n--- stderr ---\n{stderr}"
                )
            }
            Error::UnexpectedSuccess { stdout } => {
                write!(
                    f,
                    "expected the run to fail, but it succeeded\n--- stdout ---\n{stdout}"
                )
            }
            Error::OutputMismatch { expected, actual } => {
                write!(
                    f,
                    "output mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    /// Returns the underlying `io::Error` for `Io`, and nothing for the other variants.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Finds every case under `dir`, sorted by name.
pub fn discover(dir: &Path) -> Result<Vec<Case>, Error> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    // Sorting makes the order of reported errors predictable.
    files.sort();

    let mut cases = Vec::new();
    for file in &files {
        let extension = file.extension().and_then(|extension| extension.to_str());
        if extension == Some("em") {
            cases.push(build_case(dir, file)?);
        } else if extension == Some("out") || extension == Some("err") {
            let has_program = is_regular_file(&file.with_extension("em"));
            if !has_program {
                return Err(Error::OrphanExpectation { path: file.clone() });
            }
        }
    }

    cases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(cases)
}

/// Adds every regular file below `dir` to `files`, skipping hidden entries and symlinks.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries = fs::read_dir(dir).map_err(|source| io_error(dir, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| io_error(dir, source))?;
        let path = entry.path();
        // `file_type` does not follow symlinks, so a link is never mistaken for its target.
        let file_type = entry
            .file_type()
            .map_err(|source| io_error(&path, source))?;

        if file_type.is_symlink() {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if file_type.is_dir() {
            collect_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

/// Tells whether `path` is a regular file, without following symlinks.
fn is_regular_file(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.is_file(),
        Err(_) => false,
    }
}

/// Tells whether anything exists at `path`, including a dangling symlink.
fn exists_without_following(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Builds the case for one `.em` file, pairing it with its `.out` or `.err` file.
fn build_case(dir: &Path, source: &Path) -> Result<Case, Error> {
    let out_file = source.with_extension("out");
    let err_file = source.with_extension("err");

    let expected = match (is_regular_file(&out_file), is_regular_file(&err_file)) {
        (true, true) => {
            return Err(Error::ConflictingExpectations {
                source: source.to_path_buf(),
            });
        }
        (true, false) => Some(Expected::Success { stdout: out_file }),
        (false, true) => Some(Expected::Failure { stderr: err_file }),
        (false, false) => None,
    };

    Ok(Case {
        name: case_name(dir, source),
        source: source.to_path_buf(),
        expected,
    })
}

/// Turns `dir/errors/undefined.em` into `errors/undefined`.
fn case_name(dir: &Path, source: &Path) -> String {
    let without_extension = source.with_extension("");
    // `source` always comes from walking `dir`, so the prefix is always there.
    let relative = without_extension
        .strip_prefix(dir)
        .unwrap_or(&without_extension);

    let mut parts = Vec::new();
    for component in relative.components() {
        parts.push(component.as_os_str().to_string_lossy().into_owned());
    }
    // Joining by hand keeps `/` separators on every platform.
    parts.join("/")
}

/// Compares the outcome of a run with the case's expectation file.
pub fn check(case: &Case, outcome: &Outcome) -> Result<(), Error> {
    match &case.expected {
        None => Err(Error::MissingExpectation {
            source: case.source.clone(),
        }),
        Some(Expected::Success { stdout }) => {
            if !outcome.success {
                return Err(Error::UnexpectedFailure {
                    stderr: outcome.stderr.clone(),
                });
            }
            compare(stdout, &outcome.stdout)
        }
        Some(Expected::Failure { stderr }) => {
            if outcome.success {
                return Err(Error::UnexpectedSuccess {
                    stdout: outcome.stdout.clone(),
                });
            }
            compare(stderr, &outcome.stderr)
        }
    }
}

/// Reads the expectation file and requires `actual` to equal it exactly.
fn compare(expected_file: &Path, actual: &str) -> Result<(), Error> {
    let expected =
        fs::read_to_string(expected_file).map_err(|source| io_error(expected_file, source))?;
    if expected == actual {
        Ok(())
    } else {
        Err(Error::OutputMismatch {
            expected,
            actual: actual.to_string(),
        })
    }
}

/// Writes the expectation file that matches the outcome and removes the other kind.
///
/// The order is: remove the opposite kind, remove the same kind, then write. A failure
/// part-way leaves a missing expectation, never both kinds, and removing the same-kind
/// path first means a symlink there is replaced instead of written through.
pub fn bless(case: &Case, outcome: &Outcome) -> Result<(), Error> {
    let out_file = case.source.with_extension("out");
    let err_file = case.source.with_extension("err");

    let (opposite, target, contents) = if outcome.success {
        (&err_file, &out_file, &outcome.stdout)
    } else {
        (&out_file, &err_file, &outcome.stderr)
    };

    remove_if_exists(opposite)?;
    remove_if_exists(target)?;
    write_file(target, contents)
}

/// Writes `contents` to `path` exactly as given.
fn write_file(path: &Path, contents: &str) -> Result<(), Error> {
    fs::write(path, contents).map_err(|source| io_error(path, source))
}

/// Deletes `path` if anything exists there, including a dangling symlink.
fn remove_if_exists(path: &Path) -> Result<(), Error> {
    if exists_without_following(path) {
        fs::remove_file(path).map_err(|source| io_error(path, source))?;
    }
    Ok(())
}

/// Builds an `Error::Io` for `path`.
fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}
