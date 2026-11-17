//! The text form of an IR module, shown by `--emit ir`.

use std::collections::HashSet;
use std::fmt;

use crate::{
    BinOp, BlockId, Function, Inst, IrType, Loc, Module, PrintKind, Sign, SlotId, Temp, Terminator,
};

/// What a slot is called when the function has no slot with that number.
const UNKNOWN_SLOT: &str = "?";

/// What a temp's type is written as when the function lists no type for it.
const UNKNOWN_TYPE: &str = "?";

impl fmt::Display for IrType {
    /// Writes the type as `i8`, `i16`, `i32` or `i64`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "i{}", self.bits())
    }
}

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
        let name = label(&labels, param.slot);
        let sign = extension_suffix(param.ty, param.sign);
        params.push(format!("{name}:{}{sign}", param.ty));
    }
    let mut out = format!("fn {}({})", function.name, params.join(", "));
    if let Some(ret) = function.ret {
        out.push_str(&format!(" -> {ret}"));
    }
    out.push_str(" {\n");

    out.push_str("  slots:");
    for (index, (name, slot)) in labels.iter().zip(&function.slots).enumerate() {
        out.push_str(if index == 0 { " " } else { ", " });
        out.push_str(&format!("{name}:{}", slot.size));
    }
    out.push('\n');

    for block in &function.blocks {
        out.push_str(&format!("{}:\n", block.id));
        for inst in &block.insts {
            out.push_str(&format!("  {}\n", inst_text(inst, function, &labels)));
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

/// The type of a temp as written, or `?` when the function lists none.
fn temp_type(function: &Function, temp: Temp) -> String {
    match function.temp_types.get(temp.0 as usize) {
        Some(ty) => ty.to_string(),
        None => UNKNOWN_TYPE.to_string(),
    }
}

/// The text before `=` of an instruction that defines a temp: `t3:i64`.
fn defined(function: &Function, temp: Temp) -> String {
    format!("{temp}:{}", temp_type(function, temp))
}

/// `.s` or `.u` for the 8- and 16-bit types, which are loaded with extension; nothing for wider ones.
fn extension_suffix(ty: IrType, sign: Sign) -> &'static str {
    match (ty, sign) {
        (IrType::I8 | IrType::I16, Sign::Signed) => ".s",
        (IrType::I8 | IrType::I16, Sign::Unsigned) => ".u",
        (IrType::I32 | IrType::I64, _) => "",
    }
}

/// Shows one instruction without indentation.
fn inst_text(inst: &Inst, function: &Function, labels: &[String]) -> String {
    match inst {
        Inst::Const { dst, value } => format!("{} = const {value}", defined(function, *dst)),
        Inst::Load { dst, slot, sign } => {
            // A temp without a type has no width, so it prints no extension.
            let suffix = match function.temp_types.get(dst.0 as usize) {
                Some(ty) => extension_suffix(*ty, *sign),
                None => "",
            };
            let dst = defined(function, *dst);
            format!("{dst} = load{suffix} {}", label(labels, *slot))
        }
        Inst::Store { slot, src } => format!("store {}, {src}", label(labels, *slot)),
        Inst::Binary {
            dst,
            op,
            lhs,
            rhs,
            loc,
        } => {
            let dst = defined(function, *dst);
            let mut text = format!("{dst} = {} {lhs}, {rhs}", mnemonic(*op));
            if can_panic(*op) {
                text.push_str(&format!(" {loc}"));
            }
            text
        }
        Inst::Neg { dst, src, loc } => format!("{} = neg {src} {loc}", defined(function, *dst)),
        Inst::Not { dst, src } => format!("{} = not {src}", defined(function, *dst)),
        Inst::Call { dst, func, args } => {
            let call = format!("call {func}({})", temp_list(args));
            match dst {
                Some(dst) => format!("{} = {call}", defined(function, *dst)),
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

/// Returns the name of an operation in the printed IR, with `.s` or `.u` where the sign matters.
fn mnemonic(op: BinOp) -> String {
    let (name, sign) = match op {
        BinOp::Add(sign) => ("add", Some(sign)),
        BinOp::Sub(sign) => ("sub", Some(sign)),
        BinOp::Mul(sign) => ("mul", Some(sign)),
        BinOp::Div(sign) => ("div", Some(sign)),
        BinOp::Rem(sign) => ("rem", Some(sign)),
        BinOp::Eq => ("eq", None),
        BinOp::Ne => ("ne", None),
        BinOp::Lt(sign) => ("lt", Some(sign)),
        BinOp::Le(sign) => ("le", Some(sign)),
        BinOp::Gt(sign) => ("gt", Some(sign)),
        BinOp::Ge(sign) => ("ge", Some(sign)),
    };
    match sign {
        Some(Sign::Signed) => format!("{name}.s"),
        Some(Sign::Unsigned) => format!("{name}.u"),
        None => name.to_string(),
    }
}

/// Tells whether an operation can panic, and so prints its location.
fn can_panic(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add(_) | BinOp::Sub(_) | BinOp::Mul(_) | BinOp::Div(_) | BinOp::Rem(_)
    )
}

/// Returns the suffix of `print.` for a print kind.
fn print_kind_name(kind: PrintKind) -> &'static str {
    match kind {
        PrintKind::I64 => "i64",
        PrintKind::Bool => "bool",
    }
}
