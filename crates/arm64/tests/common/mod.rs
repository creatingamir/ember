//! Helpers shared by the `arm64` integration tests: the front-end pipeline, the toolchain run and locations.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]
// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use ir::{
    BinOp, Block, BlockId, Function, Inst, IrType, Loc, Param, PrintKind, Sign, Temp, Terminator,
};
use source::{SourceFile, Span};

/// The file name every test program is compiled under; it shows up in panic messages.
pub const FILE: &str = "t.em";

/// The largest `i64`, as source text.
pub const MAX: &str = "9223372036854775807";

/// The smallest `i64`, as source text (a negated literal).
pub const MIN: &str = "-9223372036854775808";

/// Joins source lines into program text with a trailing newline.
pub fn program<S: AsRef<str>>(lines: &[S]) -> String {
    let mut text = lines
        .iter()
        .map(|line| line.as_ref())
        .collect::<Vec<&str>>()
        .join("\n");
    text.push('\n');
    text
}

/// Wraps statement lines into `fn main() { ... }`; the first body line is source line 2.
pub fn main_with<S: AsRef<str>>(body: &[S]) -> String {
    let mut lines = vec!["fn main() {".to_string()];
    lines.extend(body.iter().map(|line| line.as_ref().to_string()));
    lines.push("}".to_string());
    program(&lines)
}

/// Runs the front end on valid source text and returns the IR module.
pub fn module(src: &str) -> ir::Module {
    let program = parser::parse_source(src).unwrap();
    let checked = check::check(&program).unwrap();
    ir::lower(&program, &checked, &SourceFile::new(FILE, src))
        .map_err(|diagnostic| {
            format!(
                "ir::lower failed on a checked program: {}",
                diagnostic.message
            )
        })
        .unwrap()
}

/// Emits assembly for a module that must be accepted, with `file_name` shown in panic messages.
pub fn emit_ok(module: &ir::Module, file_name: &str) -> String {
    arm64::emit(module, file_name)
        .map_err(|diagnostic| format!("arm64::emit failed: {}", diagnostic.message))
        .unwrap()
}

/// Compiles valid source text to assembly text, with `file_name` shown in panic messages.
pub fn asm_named(src: &str, file_name: &str) -> String {
    emit_ok(&module(src), file_name)
}

/// Compiles valid source text to assembly text under the default file name.
pub fn asm(src: &str) -> String {
    asm_named(src, FILE)
}

/// Returns the 1-based `line:col` of the `nth` (0-based) occurrence of `needle` in `src`.
pub fn loc(src: &str, needle: &str, nth: usize) -> String {
    let found = src.match_indices(needle).nth(nth);
    assert!(
        found.is_some(),
        "occurrence {nth} of `{needle}` not found in:\n{src}"
    );
    let (offset, _) = found.unwrap();
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    let col = offset - line_start + 1;
    format!("{line}:{col}")
}

/// The exact stderr of a panic with `reason` at the `nth` occurrence of `needle` in `src`.
pub fn panic_stderr(reason: &str, src: &str, needle: &str, nth: usize) -> String {
    format!("panic: {reason} at {FILE}:{}\n", loc(src, needle, nth))
}

/// A scratch directory removed when dropped, even when the test fails.
struct TempDir(PathBuf);

impl TempDir {
    /// Creates a directory named after the process id and a per-process counter.
    fn new() -> TempDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        let name = format!("ember-arm64-{}-{id}", std::process::id());
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).unwrap();
        TempDir(path)
    }

    /// The directory's path.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    /// Removes the directory and everything in it.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs `clang` with the arguments inside `dir` and asserts that it succeeded.
fn clang(dir: &Path, args: &[&str], asm: &str) {
    let output = Command::new("clang")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "clang {args:?} failed:\n{}\n--- assembly ---\n{asm}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Compiles, assembles, links and runs a program; returns its stdout, stderr and exit code.
pub fn run(src: &str) -> (String, String, i32) {
    run_asm(&asm(src))
}

/// Emits assembly for an IR module under the default file name, then assembles, links and runs it.
pub fn run_module(module: &ir::Module) -> (String, String, i32) {
    run_asm(&emit_ok(module, FILE))
}

/// Assembles, links and runs assembly text; returns the program's stdout, stderr and exit code.
fn run_asm(asm: &str) -> (String, String, i32) {
    run_asm_linked(asm, &["prog.o", "-o", "prog"])
}

/// The test-only C harness that runs `_em_main` on a small thread stack next to a guard and a canary.
pub const GUARD_HARNESS: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/guard_harness.c");

/// Emits assembly for an IR module and runs it under the guard harness (its own entry point, `_main` unused).
pub fn run_module_in_guard_harness(module: &ir::Module) -> (String, String, i32) {
    let asm = emit_ok(module, FILE);
    run_asm_linked(
        &asm,
        &[
            "prog.o",
            GUARD_HARNESS,
            "-Wl,-e,_harness_main",
            "-o",
            "prog",
        ],
    )
}

/// Assembles `asm` to `prog.o`, links with the given clang arguments, runs `prog`; returns stdout, stderr and exit code.
fn run_asm_linked(asm: &str, link_args: &[&str]) -> (String, String, i32) {
    let dir = TempDir::new();
    fs::write(dir.path().join("prog.s"), asm).unwrap();

    clang(dir.path(), &["-c", "prog.s", "-o", "prog.o"], asm);
    clang(dir.path(), link_args, asm);

    let output = Command::new(dir.path().join("prog")).output().unwrap();
    // A process killed by a signal has no exit code; -1 makes the comparison fail loudly.
    let code = output.status.code().unwrap_or(-1);
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        code,
    )
}

/// A block with the given id, instructions and terminator.
pub fn block_of(id: u32, insts: Vec<Inst>, term: Terminator) -> Block {
    Block {
        id: BlockId(id),
        insts,
        term,
    }
}

/// A module with one function `main` (no slots, no parameters) made of the given blocks.
pub fn main_module(blocks: Vec<Block>) -> ir::Module {
    ir::Module {
        functions: vec![Function {
            name: "main".to_string(),
            params: vec![],
            ret: None,
            slots: vec![],
            temp_types: i64_temp_types(&blocks),
            blocks,
            span: Span::new(0, 0),
        }],
    }
}

/// `count` blocks that end in `Unreachable` (one `brk` each), numbered from `first_id`.
pub fn unreachable_blocks(first_id: u32, count: u32) -> Vec<Block> {
    (first_id..first_id + count)
        .map(|id| block_of(id, vec![], Terminator::Unreachable))
        .collect()
}

/// A temp holding a constant, as an instruction.
pub fn constant(temp: u32, value: i64) -> Inst {
    Inst::Const {
        dst: Temp(temp),
        value,
    }
}

/// A binary instruction on temps with the source location 3:5.
pub fn binary(dst: u32, op: BinOp, lhs: u32, rhs: u32) -> Inst {
    Inst::Binary {
        dst: Temp(dst),
        op,
        lhs: Temp(lhs),
        rhs: Temp(rhs),
        loc: Loc { line: 3, col: 5 },
    }
}

/// A `main` that sets `x` with `first_line`, adds 1 to it `count` times and prints it; line 2 is `first_line`, line 3 the first `x += 1`.
pub fn counter_program(first_line: &str, count: usize) -> String {
    let mut body = vec![format!("    {first_line}")];
    body.extend((0..count).map(|_| "    x += 1".to_string()));
    body.push("    print(x)".to_string());
    main_with(&body)
}

/// A function whose first `param_count` slots are its parameters, with `slot_count` unnamed slots in all.
pub fn ir_function(
    name: &str,
    param_count: u32,
    returns_value: bool,
    slot_count: u32,
    blocks: Vec<Block>,
) -> Function {
    Function {
        name: name.to_string(),
        params: (0..param_count).map(i64_param).collect(),
        ret: returns_value.then_some(IrType::I64),
        slots: (0..slot_count)
            .map(|number| i64_slot(format!("s{number}")))
            .collect(),
        temp_types: i64_temp_types(&blocks),
        blocks,
        span: Span::new(0, 0),
    }
}

/// A parameter stored in `slot`: a signed `i64`.
pub fn i64_param(slot: u32) -> Param {
    Param {
        slot: ir::SlotId(slot),
        ty: IrType::I64,
        sign: Sign::Signed,
    }
}

/// An 8-byte, 8-aligned slot, the size of every Phase 1 variable.
pub fn i64_slot(name: String) -> ir::Slot {
    ir::Slot {
        name,
        size: 8,
        align: 8,
    }
}

/// The most temps `i64_temp_types` lists; tests that name a temp near `u32::MAX` must not allocate billions.
const MAX_LISTED_TEMPS: u32 = 1 << 20;

/// One `i64` entry for every temp the blocks mention (up to `MAX_LISTED_TEMPS`), in temp order.
pub fn i64_temp_types(blocks: &[Block]) -> Vec<IrType> {
    let mut highest: Option<u32> = None;
    let mut note = |temp: &Temp| highest = highest.max(Some(temp.0));
    for block in blocks {
        for inst in &block.insts {
            match inst {
                Inst::Const { dst, .. } => note(dst),
                Inst::Load { dst, .. } => note(dst),
                Inst::Store { src, .. } => note(src),
                Inst::Binary { dst, lhs, rhs, .. } => {
                    [dst, lhs, rhs].into_iter().for_each(&mut note)
                }
                Inst::Neg { dst, src, .. } | Inst::Not { dst, src } => {
                    note(dst);
                    note(src);
                }
                Inst::Call { dst, args, .. } => {
                    dst.iter().for_each(&mut note);
                    args.iter().for_each(&mut note);
                }
                Inst::Print { src, .. } => note(src),
            }
        }
        match &block.term {
            Terminator::Branch { cond, .. } => note(cond),
            Terminator::Return(Some(temp)) => note(temp),
            Terminator::Jump(_) | Terminator::Return(None) | Terminator::Unreachable => {}
        }
    }
    let count = highest.map_or(0, |last| last.saturating_add(1).min(MAX_LISTED_TEMPS));
    vec![IrType::I64; count as usize]
}

/// Asserts that `text` contains `needle`, showing the text when it does not.
pub fn assert_contains(text: &str, needle: &str) {
    assert!(text.contains(needle), "`{needle}` not found in:\n{text}");
}

/// Asserts that `text` does not contain `needle`, showing the text when it does.
pub fn assert_lacks(text: &str, needle: &str) {
    assert!(!text.contains(needle), "`{needle}` found in:\n{text}");
}

/// The trimmed lines of `text`.
pub fn trimmed_lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// Asserts that `run` appears in `text` as consecutive lines (ignoring indentation).
pub fn assert_run(text: &str, run: &[&str]) {
    let lines = trimmed_lines(text);
    let found = lines.windows(run.len()).any(|window| window == run);
    assert!(found, "lines {run:#?} not found consecutively in:\n{text}");
}

/// The lines of one function's body: everything after `symbol:` up to the next directive or symbol.
pub fn function_text(text: &str, symbol: &str) -> String {
    let header = format!("{symbol}:");
    let body: Vec<&str> = text
        .lines()
        .skip_while(|line| *line != header)
        .skip(1)
        .take_while(|line| !line.starts_with('.') && !line.starts_with('_'))
        .collect();
    assert!(!body.is_empty(), "`{symbol}` not found in:\n{text}");
    body.join("\n")
}

/// A parameter stored in `slot`, with the given type and signedness.
pub fn param_of(slot: u32, ty: IrType, sign: Sign) -> Param {
    Param {
        slot: ir::SlotId(slot),
        ty,
        sign,
    }
}

/// A slot named `name` that holds one value of `ty`: its size and alignment are the type's.
pub fn slot_of(name: &str, ty: IrType) -> ir::Slot {
    ir::Slot {
        name: name.to_string(),
        size: ty.bytes(),
        align: ty.bytes(),
    }
}

/// A function built from its parts, with a zero span.
pub fn typed_function(
    name: &str,
    params: Vec<Param>,
    ret: Option<IrType>,
    slots: Vec<ir::Slot>,
    temp_types: Vec<IrType>,
    blocks: Vec<Block>,
) -> Function {
    Function {
        name: name.to_string(),
        params,
        ret,
        slots,
        temp_types,
        blocks,
        span: Span::new(0, 0),
    }
}

/// A `main` that does nothing, so a module of other functions links.
pub fn empty_main() -> Function {
    let entry = block_of(0, vec![], Terminator::Return(None));
    typed_function("main", vec![], None, vec![], vec![], vec![entry])
}

/// A module of `functions` followed by an empty `main`.
pub fn module_with_main(mut functions: Vec<Function>) -> ir::Module {
    functions.push(empty_main());
    ir::Module { functions }
}

/// A one-block function: `insts` then `term`, with the given parameters, return type, slots and temp types.
pub fn single_block_function(
    name: &str,
    params: Vec<Param>,
    ret: Option<IrType>,
    slots: Vec<ir::Slot>,
    temp_types: Vec<IrType>,
    insts: Vec<Inst>,
    term: Terminator,
) -> Function {
    let entry = block_of(0, insts, term);
    typed_function(name, params, ret, slots, temp_types, vec![entry])
}

/// A load of `slot` into `dst` with the given extension.
pub fn load_of(dst: u32, slot: u32, sign: Sign) -> Inst {
    Inst::Load {
        dst: Temp(dst),
        slot: ir::SlotId(slot),
        sign,
    }
}

/// A store of `src` into `slot`.
pub fn store_of(slot: u32, src: u32) -> Inst {
    Inst::Store {
        slot: ir::SlotId(slot),
        src: Temp(src),
    }
}

/// A print of `src` as `kind`.
pub fn print_of(src: u32, kind: PrintKind) -> Inst {
    Inst::Print {
        src: Temp(src),
        kind,
    }
}

/// A binary instruction on temps with the source location `line`:`col`.
pub fn binary_at(dst: u32, op: BinOp, lhs: u32, rhs: u32, line: u32, col: u32) -> Inst {
    Inst::Binary {
        dst: Temp(dst),
        op,
        lhs: Temp(lhs),
        rhs: Temp(rhs),
        loc: Loc { line, col },
    }
}

/// The test-only C harness that calls Ember functions with C types and checks their results.
pub const ABI_HARNESS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/abi_harness.c");

/// Emits assembly for an IR module and runs it under the ABI harness (its own entry point, `_main` unused).
pub fn run_module_in_abi_harness(module: &ir::Module) -> (String, String, i32) {
    let asm = emit_ok(module, FILE);
    run_asm_linked(
        &asm,
        &["prog.o", ABI_HARNESS, "-Wl,-e,_harness_main", "-o", "prog"],
    )
}
