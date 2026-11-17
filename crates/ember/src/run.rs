//! Running a built program and passing its exit code on.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::error::Failure;

/// Runs the executable with the terminal's stdin, stdout and stderr, and returns its exit code.
pub fn run_program(executable: &Path) -> Result<ExitCode, Failure> {
    let status = Command::new(executable)
        .status()
        .map_err(|io_error| Failure::io("cannot run the program", io_error))?;

    if let Some(code) = status.code() {
        // On Unix an exit code is always 0..=255, so the conversion cannot fail in practice.
        return Ok(ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX)));
    }
    match status.signal() {
        Some(signal) => Err(Failure::message(format!(
            "program was killed by signal {signal}"
        ))),
        None => Err(Failure::message("program ended without an exit code")),
    }
}
