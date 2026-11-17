//! The error type of the driver: text for stderr and an exit code of 1.

use std::io::Write;
use std::process::ExitCode;

/// A failed command, holding the exact text to print on stderr.
pub struct Failure {
    /// The full stderr text, ending with a newline.
    text: String,
}

impl Failure {
    /// A one-line failure that is printed as `error: <text>`.
    pub fn message(text: impl std::fmt::Display) -> Failure {
        Failure {
            text: format!("error: {text}\n"),
        }
    }

    /// An io failure, printed as `error: <context>: <io error>`.
    pub fn io(context: impl std::fmt::Display, io_error: std::io::Error) -> Failure {
        Failure::message(format!("{context}: {io_error}"))
    }

    /// A failure whose text is already complete, such as rendered compile errors.
    pub fn rendered(text: String) -> Failure {
        Failure { text }
    }

    /// Prints the failure on stderr and returns the exit code 1.
    pub fn report(&self) -> ExitCode {
        // If stderr itself is closed there is nowhere left to report, so the result is ignored.
        let _ = std::io::stderr().write_all(self.text.as_bytes());
        ExitCode::from(1)
    }
}
