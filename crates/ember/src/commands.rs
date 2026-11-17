//! The `build` and `run` commands.

use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::cli::{BuildArgs, Cli, Command, RunArgs};
use crate::error::Failure;
use crate::pipeline;
use crate::run;
use crate::toolchain::{self, TempDir};

/// Runs the chosen command and turns its result into the process exit code.
pub fn execute(cli: &Cli) -> ExitCode {
    let result = match &cli.command {
        Command::Build(args) => build(args),
        Command::Run(args) => run(args),
    };
    match result {
        Ok(exit_code) => exit_code,
        Err(failure) => failure.report(),
    }
}

/// Builds an executable, or prints one stage when `--emit` is given.
fn build(args: &BuildArgs) -> Result<ExitCode, Failure> {
    let file = pipeline::read_source(&args.file)?;

    if let Some(kind) = args.emit {
        let text = pipeline::emit(kind, &file)?;
        print_to_stdout(&text)?;
        return Ok(ExitCode::SUCCESS);
    }

    let output = output_path(args)?;
    let assembly = pipeline::to_assembly(&file)?;
    // The guard deletes the temporary directory when this function returns, on success or error.
    let temp = TempDir::create()?;
    toolchain::build_executable(&assembly, &output, &temp)?;
    Ok(ExitCode::SUCCESS)
}

/// Compiles into a temporary directory, runs the program and passes its exit code on.
fn run(args: &RunArgs) -> Result<ExitCode, Failure> {
    let file = pipeline::read_source(&args.file)?;
    let assembly = pipeline::to_assembly(&file)?;
    let temp = TempDir::create()?;
    let executable = temp.path().join("program");
    toolchain::build_executable(&assembly, &executable, &temp)?;
    run::run_program(&executable)
}

/// The executable's path: `-o` if given, else the source file's name without extension.
fn output_path(args: &BuildArgs) -> Result<PathBuf, Failure> {
    let output = match &args.output {
        Some(path) => path.clone(),
        None => default_output(&args.file)?,
    };
    if is_same_file(&output, &args.file) {
        return Err(Failure::message(format!(
            "the output {} would overwrite the source file",
            output.display()
        )));
    }
    Ok(output)
}

/// True when both paths lead to the same existing file, however they are spelled.
fn is_same_file(first: &Path, second: &Path) -> bool {
    // `fs::metadata` follows symlinks. A missing or unreadable path is simply a different file.
    match (fs::metadata(first), fs::metadata(second)) {
        (Ok(first_info), Ok(second_info)) => {
            first_info.dev() == second_info.dev() && first_info.ino() == second_info.ino()
        }
        _ => false,
    }
}

/// The file name without its extension, as a path in the current directory.
fn default_output(source: &Path) -> Result<PathBuf, Failure> {
    match source.file_stem() {
        Some(stem) => Ok(PathBuf::from(stem)),
        None => Err(Failure::message(format!(
            "cannot choose an output name for {}; use -o",
            source.display()
        ))),
    }
}

/// Writes text to stdout; unlike `print!`, a closed pipe is an error and not a panic.
fn print_to_stdout(text: &str) -> Result<(), Failure> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|io_error| Failure::io("cannot write to stdout", io_error))
}
