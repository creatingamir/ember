//! The ARM64 back end: turns an IR module into assembly text for Apple Silicon macOS.
//!
//! Each function goes through five steps: instruction selection (IR to MIR with virtual registers),
//! the naive allocator (every virtual register gets a stack cell), frame layout, branch relaxation, and printing.

mod asm;
mod frame;
mod isel;
mod mir;
mod naive;
mod print;
mod relax;
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
    let mut used_reasons = strings::PanicReasons::default();
    for function in &module.functions {
        emit_function(&mut asm, function, &mut used_reasons)?;
    }
    runtime::emit_helpers(&mut asm);
    strings::emit_strings(&mut asm, file_name, used_reasons);
    Ok(asm.finish())
}

/// Writes one function by running it through the five steps, and notes the panic reasons it uses.
fn emit_function(
    asm: &mut Asm,
    function: &ir::Function,
    used_reasons: &mut strings::PanicReasons,
) -> Result<(), Diagnostic> {
    let (selected, reasons) = isel::select(function)?;
    used_reasons.extend(reasons);
    let allocated = naive::allocate(selected)?;
    let laid_out = frame::lay_out(allocated)?;
    let relaxed = relax::relax(laid_out)?;
    print::print(asm, &relaxed)
}
