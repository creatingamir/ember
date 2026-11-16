//! The `ember` command-line interface.

use clap::Parser;

/// The Ember compiler.
#[derive(Parser)]
#[command(version, arg_required_else_help = true)]
struct Cli {}

/// Parses the command line; clap handles help, version and usage errors.
fn main() {
    // clap prints help, version and errors and exits by itself, so the result is unused.
    Cli::parse();
}
