//! The `ember` command-line interface.

mod cli;
mod commands;
mod error;
mod pipeline;
mod run;
mod tokens;
mod toolchain;

use std::process::ExitCode;
use std::thread;

use clap::Parser;

use crate::cli::Cli;
use crate::error::Failure;

/// The compiler's stack size: deep but legal programs recurse a lot (decision 0025).
const COMPILER_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Parses the command line, then runs the command on a thread with a large stack.
fn main() -> ExitCode {
    // clap prints help, version and usage errors by itself and exits with code 2.
    let cli = Cli::parse();

    let spawned = thread::Builder::new()
        .stack_size(COMPILER_STACK_BYTES)
        .spawn(move || commands::execute(&cli));
    let handle = match spawned {
        Ok(handle) => handle,
        Err(io_error) => {
            let failure = Failure::message(format!("cannot start the compiler thread: {io_error}"));
            return failure.report();
        }
    };

    match handle.join() {
        Ok(exit_code) => exit_code,
        // The compiler never panics on purpose; this is a bug, reported without a second panic.
        Err(_) => {
            Failure::message("internal compiler error: the compiler thread panicked").report()
        }
    }
}
