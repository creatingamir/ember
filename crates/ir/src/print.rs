//! The text form of an IR module, shown by `--emit ir`.

use std::collections::HashSet;
use std::fmt;

use crate::{BinOp, BlockId, Function, Inst, Loc, Module, PrintKind, SlotId, Temp, Terminator};

/// What a slot is called when the module has no slot with that number.
const UNKNOWN_SLOT: &str = "?";

impl fmt::Display for Temp {
    /// Writes the temp as `t3`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

impl fmt::Display for BlockId {
    /// Writes the block id as `bb3`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

impl fmt::Display for Loc {
    /// Writes the location as `@line:col`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}:{}", self.line, self.col)
    }
}

/// Shows a module as text, with a blank line between functions.
pub fn print_module(module: &Module) -> String {
    let mut functions = Vec::new();
    for function in &module.functions {
        functions.push(print_function(function));
    }
    functions.join("\n")
}

/// Shows one function: its header, its slots line and its blocks; the text ends in `}\n`.
fn print_function(function: &Function) -> String {
    let labels = slot_labels(function);

    let mut params = Vec::new();
    for param in &function.params {
        params.push(label(&labels, *param));
    }
    let mut out = format!("fn {}({})", function.name, params.join(", "));
    if function.returns_value {
        out.push_str(" -> i64");
    }
    out.push_str(" {\n");

    out.push_str("  slots:");
    for (index, name) in labels.iter().enumerate() {
        out.push_str(if index == 0 { " " } else { ", " });
        out.push_str(name);
    }
    out.push('\n');

    for block in &function.blocks {
        out.push_str(&format!("{}:\n", block.id));
        for inst in &block.insts {
            out.push_str(&format!("  {}\n", inst_text(inst, &labels)));
        }
        out.push_str(&format!("  {}\n", terminator_text(&block.term)));
    }
    out.push_str("}\n");
    out
}

/// Returns the printed name of every slot; a name seen before gets `#` and its slot number.
fn slot_labels(function: &Function) -> Vec<String> {
    let mut seen_names = HashSet::new();
    let mut labels = Vec::new();
    for (index, slot) in function.slots.iter().enumerate() {
        // `insert` returns false when the name is already in the set.
        if seen_names.insert(slot.name.as_str()) {
            labels.push(slot.name.clone());
        } else {
            labels.push(format!("{}#{index}", slot.name));
        }
    }
    labels
}

/// Returns the printed name of a slot.
fn label(labels: &[String], slot: SlotId) -> &str {
    match labels.get(slot.0 as usize) {
        Some(name) => name,
        None => UNKNOWN_SLOT,
    }
}

/// Shows one instruction without indentation.
fn inst_text(inst: &Inst, labels: &[String]) -> String {
    match inst {
        Inst::Const { dst, value } => format!("{dst} = const {value}"),
        Inst::Load { dst, slot } => format!("{dst} = load {}", label(labels, *slot)),
        Inst::Store { slot, src } => format!("store {}, {src}", label(labels, *slot)),
        Inst::Binary {
            dst,
            op,
            lhs,
            rhs,
            loc,
        } => {
            let mut text = format!("{dst} = {} {lhs}, {rhs}", mnemonic(*op));
            if can_panic(*op) {
                text.push_str(&format!(" {loc}"));
            }
            text
        }
        Inst::Neg { dst, src, loc } => format!("{dst} = neg {src} {loc}"),
        Inst::Not { dst, src } => format!("{dst} = not {src}"),
        Inst::Call { dst, func, args } => {
            let call = format!("call {func}({})", temp_list(args));
            match dst {
                Some(dst) => format!("{dst} = {call}"),
                None => call,
            }
        }
        Inst::Print { src, kind } => format!("print.{} {src}", print_kind_name(*kind)),
    }
}

/// Shows a terminator without indentation.
fn terminator_text(term: &Terminator) -> String {
    match term {
        Terminator::Jump(target) => format!("jump {target}"),
        Terminator::Branch {
            cond,
            then_to,
            else_to,
        } => format!("br {cond}, {then_to}, {else_to}"),
        Terminator::Return(Some(value)) => format!("ret {value}"),
        Terminator::Return(None) => "ret".to_string(),
        Terminator::Unreachable => "unreachable".to_string(),
    }
}

/// Shows temps separated by commas.
fn temp_list(temps: &[Temp]) -> String {
    let mut names = Vec::new();
    for temp in temps {
        names.push(temp.to_string());
    }
    names.join(", ")
}

/// Returns the name of an operation in the printed IR.
fn mnemonic(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Gt => "gt",
        BinOp::Ge => "ge",
    }
}

/// Tells whether an operation can panic, and so prints its location.
fn can_panic(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
    )
}

/// Returns the suffix of `print.` for a print kind.
fn print_kind_name(kind: PrintKind) -> &'static str {
    match kind {
        PrintKind::I64 => "i64",
        PrintKind::Bool => "bool",
    }
}
