//! The command-line arguments, described with clap's derive macros.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// The Ember compiler.
#[derive(Parser)]
#[command(version, arg_required_else_help = true)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The things `ember` can do.
#[derive(Subcommand)]
pub enum Command {
    /// Compile a file to an executable, or print one compiler stage with --emit.
    Build(BuildArgs),
    /// Compile a file into a temporary directory, run it and exit with its exit code.
    Run(RunArgs),
}

/// The arguments of `ember build`.
#[derive(Args)]
pub struct BuildArgs {
    /// The Ember source file.
    pub file: PathBuf,

    /// Where to write the executable (default: the file's name without extension).
    #[arg(short, long, conflicts_with = "emit")]
    pub output: Option<PathBuf>,

    /// Print one compiler stage to stdout instead of building.
    #[arg(long, value_enum)]
    pub emit: Option<EmitKind>,
}

/// The arguments of `ember run`.
#[derive(Args)]
pub struct RunArgs {
    /// The Ember source file.
    pub file: PathBuf,
}

/// A compiler stage whose output `--emit` can print.
#[derive(Clone, Copy, ValueEnum)]
pub enum EmitKind {
    /// The tokens, one per line.
    Tokens,
    /// The syntax tree.
    Ast,
    /// The intermediate representation.
    Ir,
    /// The ARM64 assembly.
    Asm,
}
