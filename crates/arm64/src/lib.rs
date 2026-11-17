//! The ARM64 back end: turns an IR module into assembly text for Apple Silicon macOS.

mod asm;
mod function;
mod runtime;
mod strings;

use asm::Asm;
use source::Diagnostic;

/// Assembly for a whole program. `file_name` is shown in panic messages.
///
/// A module the IR should never produce (decision 0026) is an "internal compiler error" diagnostic.
pub fn emit(module: &ir::Module, file_name: &str) -> Result<String, Diagnostic> {
    let mut asm = Asm::new();
    asm.directive(".text");
    runtime::emit_main(&mut asm);
    for function in &module.functions {
        function::emit_function(&mut asm, function)?;
    }
    runtime::emit_helpers(&mut asm);
    strings::emit_strings(&mut asm, file_name);
    Ok(asm.finish())
}
