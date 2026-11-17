//! The compiler stages, from reading a file to assembly text.

use std::fs;
use std::path::Path;

use ast::Program;
use check::Checked;
use source::{Diagnostic, SourceFile};

use crate::cli::EmitKind;
use crate::error::Failure;
use crate::tokens;

/// Reads a source file; the file's name in messages is the path exactly as given.
pub fn read_source(path: &Path) -> Result<SourceFile, Failure> {
    let shown = path.display();
    let bytes =
        fs::read(path).map_err(|io_error| Failure::io(format!("cannot read {shown}"), io_error))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| Failure::message(format!("{shown} is not valid UTF-8")))?;
    Ok(SourceFile::new(shown.to_string(), text))
}

/// Runs the stages up to the one `kind` names and returns that stage's text.
pub fn emit(kind: EmitKind, file: &SourceFile) -> Result<String, Failure> {
    match kind {
        EmitKind::Tokens => {
            let token_list = lexer::lex(file.text()).map_err(|error| render_one(&error, file))?;
            Ok(tokens::format_tokens(&token_list, file))
        }
        EmitKind::Ast => {
            let program = parse(file)?;
            Ok(ast::print_program(&program))
        }
        EmitKind::Ir => {
            let module = lower(file)?;
            Ok(ir::print_module(&module))
        }
        EmitKind::Asm => to_assembly(file),
    }
}

/// Runs every stage up to the ARM64 assembly text.
pub fn to_assembly(file: &SourceFile) -> Result<String, Failure> {
    let module = lower(file)?;
    arm64::emit(&module, file.name()).map_err(|error| render_one(&error, file))
}

/// Lexes and parses the file.
fn parse(file: &SourceFile) -> Result<Program, Failure> {
    parser::parse_source(file.text()).map_err(|error| render_one(&error, file))
}

/// Parses, checks and lowers the file to IR.
fn lower(file: &SourceFile) -> Result<ir::Module, Failure> {
    let program = parse(file)?;
    let checked = check_program(&program, file)?;
    ir::lower(&program, &checked, file).map_err(|error| render_one(&error, file))
}

/// Checks a program; the checker reports every error it finds, in source order.
fn check_program(program: &Program, file: &SourceFile) -> Result<Checked, Failure> {
    check::check(program).map_err(|errors| {
        let rendered: String = errors.iter().map(|error| error.render(file)).collect();
        Failure::rendered(rendered)
    })
}

/// Turns one compile error into a failure with the rendered diagnostic.
fn render_one(error: &Diagnostic, file: &SourceFile) -> Failure {
    Failure::rendered(error.render(file))
}
