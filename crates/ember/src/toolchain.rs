//! Temporary directories and the clang calls that turn assembly into an executable.

use std::ffi::OsStr;
use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::Failure;

/// Makes the names of temporary directories from this process unique.
static NEXT_DIR_NUMBER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory that is deleted, with its contents, when the value is dropped.
pub struct TempDir {
    /// The directory's path.
    path: PathBuf,
}

impl TempDir {
    /// Creates a new directory named after the process id and a counter, never reusing one.
    pub fn create() -> Result<TempDir, Failure> {
        // An absolute parent means no path given to clang can start with `-` and look like an option.
        let parent = std::path::absolute(std::env::temp_dir())
            .map_err(|io_error| Failure::io("cannot find the temporary directory", io_error))?;
        loop {
            let number = NEXT_DIR_NUMBER.fetch_add(1, Ordering::Relaxed);
            let name = format!("ember-{}-{number}", std::process::id());
            let path = parent.join(name);

            // Mode 0700 keeps other users out; a non-recursive create fails if the name exists.
            match DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(TempDir { path }),
                // A leftover from an earlier process with the same id: try the next number.
                Err(io_error) if io_error.kind() == ErrorKind::AlreadyExists => continue,
                Err(io_error) => {
                    let context = format!(
                        "cannot create a temporary directory in {}",
                        parent.display()
                    );
                    return Err(Failure::io(context, io_error));
                }
            }
        }
    }

    /// The directory's path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    /// Deletes the directory; Rust calls this automatically when the value goes out of scope.
    fn drop(&mut self) {
        // Nothing useful can be done if cleanup fails, so the result is ignored.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Assembles and links `assembly` into the executable `output`, keeping scratch files in `temp`.
pub fn build_executable(assembly: &str, output: &Path, temp: &TempDir) -> Result<(), Failure> {
    let assembly_path = temp.path().join("program.s");
    let object_path = temp.path().join("program.o");

    fs::write(&assembly_path, assembly).map_err(|io_error| {
        Failure::message(format!(
            "cannot write {}: {io_error}",
            assembly_path.display()
        ))
    })?;

    run_clang(
        temp,
        &[
            "-c".as_ref(),
            assembly_path.as_os_str(),
            "-o".as_ref(),
            object_path.as_os_str(),
        ],
    )?;
    run_clang(
        temp,
        &[object_path.as_os_str(), "-o".as_ref(), output.as_os_str()],
    )
}

/// Runs clang with the given arguments (no shell is involved) and reports a failure.
fn run_clang(temp: &TempDir, arguments: &[&OsStr]) -> Result<(), Failure> {
    let result = Command::new("clang")
        // Clang makes its own scratch files in TMPDIR, so a relative TMPDIR must not reach it.
        .env("TMPDIR", temp.path())
        .args(arguments)
        .output()
        .map_err(|io_error| Failure::io("cannot run clang", io_error))?;

    if result.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    Err(Failure::message(format!(
        "clang failed: {}",
        stderr.trim_end()
    )))
}
