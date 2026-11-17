//! A small builder for assembly text.

/// The assembly text written so far.
pub struct Asm {
    text: String,
}

impl Asm {
    /// Starts an empty program.
    pub fn new() -> Asm {
        Asm {
            text: String::new(),
        }
    }

    /// Returns the finished text, which ends with a newline.
    pub fn finish(self) -> String {
        self.text
    }

    /// Writes one indented instruction.
    pub fn inst(&mut self, text: &str) {
        self.text.push_str("    ");
        self.text.push_str(text);
        self.text.push('\n');
    }

    /// Writes one indented directive such as `.text`.
    pub fn directive(&mut self, text: &str) {
        self.inst(text);
    }

    /// Writes a label on its own line.
    pub fn label(&mut self, name: &str) {
        self.text.push_str(name);
        self.text.push_str(":\n");
    }

    /// Loads a 64-bit constant into `reg`: `movz` for the low chunk, `movk` for each other non-zero chunk.
    pub fn load_imm(&mut self, reg: &str, value: u64) {
        self.inst(&format!("movz {reg}, #{}", value & 0xffff));
        for shift in [16, 32, 48] {
            let chunk = (value >> shift) & 0xffff;
            if chunk != 0 {
                self.inst(&format!("movk {reg}, #{chunk}, lsl #{shift}"));
            }
        }
    }

    /// Loads the address of the local label `label` into `reg` (the `adrp` page plus the offset in the page).
    pub fn load_address(&mut self, reg: &str, label: &str) {
        self.inst(&format!("adrp {reg}, {label}@PAGE"));
        self.inst(&format!("add {reg}, {reg}, {label}@PAGEOFF"));
    }

    /// Saves the frame pointer and link register and points `x29` at the new frame.
    pub fn prologue(&mut self) {
        self.inst("stp x29, x30, [sp, #-16]!");
        self.inst("mov x29, sp");
    }

    /// Drops the frame, restores the saved registers and returns.
    pub fn epilogue(&mut self) {
        self.inst("mov sp, x29");
        self.inst("ldp x29, x30, [sp], #16");
        self.inst("ret");
    }
}
