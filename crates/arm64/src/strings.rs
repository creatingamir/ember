//! The string constants of a program: format strings, panic reasons and the file name.

use crate::asm::Asm;

/// Label of the `printf` format for an integer.
pub const FMT_I64: &str = "Lstr_fmt_i64";
/// Label of the text `true`.
pub const TRUE: &str = "Lstr_true";
/// Label of the text `false`.
pub const FALSE: &str = "Lstr_false";
/// Label of the `fprintf` format of a panic message.
pub const FMT_PANIC: &str = "Lstr_fmt_panic";
/// Label of the panic reason `integer overflow`.
pub const OVERFLOW: &str = "Lstr_overflow";
/// Label of the panic reason `division by zero`.
pub const DIV_BY_ZERO: &str = "Lstr_div_by_zero";
/// Label of the source file name.
pub const FILE: &str = "Lstr_file";

/// Writes every string constant into the `__cstring` section.
pub fn emit_strings(asm: &mut Asm, file_name: &str) {
    asm.directive(".section __TEXT,__cstring,cstring_literals");
    emit_string(asm, FMT_I64, "%lld\\n");
    // `puts` adds the newline itself.
    emit_string(asm, TRUE, "true");
    emit_string(asm, FALSE, "false");
    emit_string(asm, FMT_PANIC, "panic: %s at %s:%lld:%lld\\n");
    emit_string(asm, OVERFLOW, "integer overflow");
    emit_string(asm, DIV_BY_ZERO, "division by zero");
    emit_string(asm, FILE, &escape(file_name));
}

/// Writes one labelled `.asciz` string; `text` must already be escaped.
fn emit_string(asm: &mut Asm, label: &str, text: &str) {
    asm.label(label);
    asm.directive(&format!(".asciz \"{text}\""));
}

/// Escapes text for an `.asciz` directive, byte by byte.
///
/// Printable ASCII stays as written, except `"` and `\`, which get a backslash. Every other byte
/// (control characters, DEL and the bytes of UTF-8 characters) is a three-digit octal escape,
/// so the stored bytes equal the file name's bytes.
fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for byte in text.bytes() {
        match byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            0x20..=0x7e => escaped.push(char::from(byte)),
            _ => escaped.push_str(&format!("\\{byte:03o}")),
        }
    }
    escaped
}
