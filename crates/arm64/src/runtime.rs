//! The entry point `_main` and the runtime helpers, written as assembly that calls libc.

use crate::asm::Asm;
use crate::strings;

/// Name of the helper that prints an integer.
pub const PRINT_I64: &str = "_ember_rt_print_i64";
/// Name of the helper that prints an unsigned integer.
pub const PRINT_U64: &str = "_ember_rt_print_u64";
/// Name of the helper that prints a bool.
pub const PRINT_BOOL: &str = "_ember_rt_print_bool";
/// Name of the helper that reports a panic and exits.
pub const PANIC: &str = "_ember_rt_panic";

/// The exit code of a panic.
const PANIC_EXIT_CODE: u64 = 101;

/// Writes the C entry point `_main`, which calls the user's `main` and returns 0.
pub fn emit_main(asm: &mut Asm) {
    asm.directive(".globl _main");
    asm.directive(".p2align 2");
    asm.label("_main");
    asm.prologue();
    asm.inst("bl _em_main");
    asm.inst("mov x0, #0");
    asm.epilogue();
}

/// Writes the runtime helpers; they are local to the file.
pub fn emit_helpers(asm: &mut Asm) {
    emit_print_i64(asm);
    emit_print_u64(asm);
    emit_print_bool(asm);
    emit_panic(asm);
}

/// Starts a helper: alignment, label and prologue.
fn start_helper(asm: &mut Asm, name: &str) {
    asm.directive(".p2align 2");
    asm.label(name);
    asm.prologue();
}

/// `_ember_rt_print_i64(x0)`: `printf("%lld\n", x0)`.
fn emit_print_i64(asm: &mut Asm) {
    emit_print_integer(asm, PRINT_I64, strings::FMT_I64);
}

/// `_ember_rt_print_u64(x0)`: `printf("%llu\n", x0)`.
fn emit_print_u64(asm: &mut Asm) {
    emit_print_integer(asm, PRINT_U64, strings::FMT_U64);
}

/// A helper that prints `x0` with the `printf` format at `format`.
fn emit_print_integer(asm: &mut Asm, name: &str, format: &str) {
    start_helper(asm, name);
    // On Apple ARM64 the variadic arguments of printf go on the stack, not in registers.
    // 16 bytes keep sp 16-byte aligned.
    asm.inst("sub sp, sp, #16");
    asm.inst("str x0, [sp]");
    asm.load_address("x0", format);
    asm.inst("bl _printf");
    asm.epilogue();
}

/// `_ember_rt_print_bool(x0)`: `puts("true")` or `puts("false")`.
fn emit_print_bool(asm: &mut Asm) {
    start_helper(asm, PRINT_BOOL);
    asm.load_address("x9", strings::TRUE);
    asm.load_address("x10", strings::FALSE);
    asm.inst("cmp x0, #0");
    asm.inst("csel x0, x9, x10, ne");
    asm.inst("bl _puts");
    asm.epilogue();
}

/// `_ember_rt_panic(line x0, col x1, reason x2)`: prints the message to stderr and exits with 101.
fn emit_panic(asm: &mut Asm) {
    start_helper(asm, PANIC);
    // fprintf(stderr, fmt, reason, file, line, col): the four variadic values go on the stack.
    asm.inst("sub sp, sp, #32");
    asm.inst("str x2, [sp]");
    asm.load_address("x8", strings::FILE);
    asm.inst("str x8, [sp, #8]");
    asm.inst("str x0, [sp, #16]");
    asm.inst("str x1, [sp, #24]");
    // stderr is the variable `__stderrp`; its address comes from the GOT.
    asm.inst("adrp x8, ___stderrp@GOTPAGE");
    asm.inst("ldr x8, [x8, ___stderrp@GOTPAGEOFF]");
    asm.inst("ldr x0, [x8]");
    asm.load_address("x1", strings::FMT_PANIC);
    asm.inst("bl _fprintf");
    // exit flushes stdout, so everything printed before the panic is kept.
    asm.inst(&format!("mov x0, #{PANIC_EXIT_CODE}"));
    asm.inst("bl _exit");
}
