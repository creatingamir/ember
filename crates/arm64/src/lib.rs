//! The ARM64 back end: turns an IR module into assembly text for Apple Silicon macOS.
//!
//! Each function goes through four steps: instruction selection (IR to MIR with virtual registers),
//! the naive allocator (every virtual register gets a stack cell), frame layout, and printing.

mod asm;
mod frame;
mod isel;
mod mir;
mod naive;
mod print;
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
        emit_function(&mut asm, function)?;
    }
    runtime::emit_helpers(&mut asm);
    strings::emit_strings(&mut asm, file_name);
    Ok(asm.finish())
}

/// Writes one function by running it through the four steps.
fn emit_function(asm: &mut Asm, function: &ir::Function) -> Result<(), Diagnostic> {
    let selected = isel::select(function)?;
    let allocated = naive::allocate(selected)?;
    let laid_out = frame::lay_out(allocated)?;
    print::print(asm, &laid_out)
}
