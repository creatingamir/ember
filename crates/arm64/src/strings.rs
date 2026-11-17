//! The string constants of a program: format strings, panic reasons and the file name.

use crate::asm::Asm;

/// Label of the `printf` format for an integer.
pub const FMT_I64: &str = "Lstr_fmt_i64";
/// Label of the `printf` format for an unsigned integer.
pub const FMT_U64: &str = "Lstr_fmt_u64";
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
/// Label of the panic reason `shift amount out of range`.
pub const SHIFT: &str = "Lstr_shift";
/// Label of the panic reason `integer conversion out of range`.
pub const CONVERSION: &str = "Lstr_conversion";
/// Label of the source file name.
pub const FILE: &str = "Lstr_file";

/// Why a check can fail; it picks the reason string of the panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanicReason {
    /// A result outside the range of its type.
    Overflow,
    /// A zero divisor.
    DivByZero,
    /// A shift amount that is negative or at least the width.
    Shift,
    /// A conversion to a type that cannot hold the value.
    Conversion,
}

impl PanicReason {
    /// The label of the reason's string constant.
    pub fn label(self) -> &'static str {
        match self {
            PanicReason::Overflow => OVERFLOW,
            PanicReason::DivByZero => DIV_BY_ZERO,
            PanicReason::Shift => SHIFT,
            PanicReason::Conversion => CONVERSION,
        }
    }

    /// The bit of this reason in a `PanicReasons` set.
    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A set of panic reasons, one bit each.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PanicReasons(u8);

impl PanicReasons {
    /// Adds a reason to the set.
    pub fn insert(&mut self, reason: PanicReason) {
        self.0 |= reason.bit();
    }

    /// Adds every reason of `other` to the set.
    pub fn extend(&mut self, other: PanicReasons) {
        self.0 |= other.0;
    }

    /// Tells whether the set holds a reason.
    pub fn contains(self, reason: PanicReason) -> bool {
        self.0 & reason.bit() != 0
    }
}

/// Writes the string constants into the `__cstring` section.
///
/// The reasons `integer overflow` and `division by zero` are always written. The later ones
/// (`shift amount out of range`, `integer conversion out of range`) are written only when a
/// panic path in `used` says them, so programs that never panic that way keep their assembly.
pub fn emit_strings(asm: &mut Asm, file_name: &str, used: PanicReasons) {
    asm.directive(".section __TEXT,__cstring,cstring_literals");
    emit_string(asm, FMT_I64, "%lld\\n");
    emit_string(asm, FMT_U64, "%llu\\n");
    // `puts` adds the newline itself.
    emit_string(asm, TRUE, "true");
    emit_string(asm, FALSE, "false");
    emit_string(asm, FMT_PANIC, "panic: %s at %s:%lld:%lld\\n");
    emit_string(asm, OVERFLOW, "integer overflow");
    emit_string(asm, DIV_BY_ZERO, "division by zero");
    if used.contains(PanicReason::Shift) {
        emit_string(asm, SHIFT, "shift amount out of range");
    }
    if used.contains(PanicReason::Conversion) {
        emit_string(asm, CONVERSION, "integer conversion out of range");
    }
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
