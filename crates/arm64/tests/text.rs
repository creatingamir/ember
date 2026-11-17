//! Text-level tests for the `arm64` crate: what `emit` puts in the assembly, without running it.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{
    FILE, asm, asm_named, binary, block_of, constant, counter_program, emit_ok, i64_param,
    i64_slot, i64_temp_types, main_module, main_with, program, unreachable_blocks,
};
use ir::{
    BinOp, Block, BlockId, Inst, IrType, Loc, Module, PrintKind, Sign, SlotId, Temp, Terminator,
};
use source::Span;
use std::sync::OnceLock;

/// A recursive `fib` and a `main` that prints `fib(10)`.
fn fib_program() -> String {
    program(&[
        "fn fib(n: i64) -> i64 {",
        "    if n < 2 {",
        "        return n",
        "    }",
        "    return fib(n - 1) + fib(n - 2)",
        "}",
        "fn main() {",
        "    print(fib(10))",
        "}",
    ])
}

/// Assembly for a `main` with the given statement lines.
fn asm_of_main(body: &[&str]) -> String {
    asm(&main_with(body))
}

/// Asserts that `text` contains `needle`, showing the text when it does not.
fn assert_contains(text: &str, needle: &str) {
    assert!(text.contains(needle), "`{needle}` not found in:\n{text}");
}

/// Asserts that `text` does not contain `needle`, showing the text when it does.
fn assert_lacks(text: &str, needle: &str) {
    assert!(!text.contains(needle), "`{needle}` found in:\n{text}");
}

/// Counts the lines of `text` that equal `line` once trimmed.
fn count_lines(text: &str, line: &str) -> usize {
    text.lines()
        .filter(|candidate| candidate.trim() == line)
        .count()
}

/// The generated entry point `_main` is defined and global.
#[test]
fn main_symbol_is_defined_and_global() {
    let text = asm_of_main(&[]);
    assert_contains(&text, ".globl _main");
    assert_eq!(count_lines(&text, "_main:"), 1);
}

/// The user's `main` becomes `_em_main`.
#[test]
fn user_main_is_em_main() {
    let text = asm_of_main(&[]);
    assert_eq!(count_lines(&text, "_em_main:"), 1);
}

/// `_main` calls `_em_main`.
#[test]
fn main_calls_em_main() {
    assert_contains(&asm_of_main(&[]), "bl _em_main");
}

/// User functions are global and prefixed with `_em_`.
#[test]
fn user_functions_are_global_with_the_em_prefix() {
    let text = asm(&fib_program());
    assert_contains(&text, ".globl _em_fib");
    assert_eq!(count_lines(&text, "_em_fib:"), 1);
}

/// A call to a user function targets its prefixed symbol.
#[test]
fn calls_target_the_prefixed_symbol() {
    assert_contains(&asm(&fib_program()), "bl _em_fib");
}

/// Functions named like C library functions do not take over the C symbols.
#[test]
fn user_functions_named_like_libc_get_prefixed_symbols() {
    let src = program(&[
        "fn exit() {",
        "}",
        "fn printf() {",
        "}",
        "fn puts() {",
        "}",
        "fn main() {",
        "    exit()",
        "    printf()",
        "    puts()",
        "}",
    ]);
    let text = asm(&src);
    for name in ["_em_exit", "_em_printf", "_em_puts"] {
        assert_eq!(count_lines(&text, &format!("{name}:")), 1, "{name}");
    }
}

/// `_ember_rt_panic` is not exported.
#[test]
fn panic_helper_is_not_global() {
    let text = asm_of_main(&[]);
    assert_lacks(&text, ".globl _ember_rt_panic");
    assert_eq!(count_lines(&text, "_ember_rt_panic:"), 1);
}

/// No runtime helper is exported.
#[test]
fn no_runtime_helper_is_global() {
    assert_lacks(&asm_of_main(&[]), ".globl _ember_rt");
}

/// Every runtime helper is defined exactly once, even when the program never prints.
#[test]
fn runtime_helpers_are_emitted_once_per_program() {
    let text = asm_of_main(&[]);
    for name in [
        "_ember_rt_print_i64",
        "_ember_rt_print_bool",
        "_ember_rt_panic",
    ] {
        assert_eq!(count_lines(&text, &format!("{name}:")), 1, "{name}");
    }
}

/// Printing an int calls the int helper.
#[test]
fn print_of_an_int_calls_the_int_helper() {
    assert_contains(&asm_of_main(&["print(1)"]), "bl _ember_rt_print_i64");
}

/// Printing a bool calls the bool helper.
#[test]
fn print_of_a_bool_calls_the_bool_helper() {
    assert_contains(&asm_of_main(&["print(true)"]), "bl _ember_rt_print_bool");
}

/// The same input gives the same text.
#[test]
fn output_is_deterministic() {
    let src = fib_program();
    assert_eq!(asm(&src), asm(&src));
}

/// Two separately lowered modules of the same source give the same text.
#[test]
fn output_is_deterministic_across_separate_lowerings() {
    let first = common::emit_ok(&common::module(&fib_program()), FILE);
    let second = common::emit_ok(&common::module(&fib_program()), FILE);
    assert_eq!(first, second);
}

/// The output ends with a newline.
#[test]
fn output_ends_with_a_newline() {
    assert!(asm_of_main(&[]).ends_with('\n'));
}

/// The file name is stored in an `.asciz` string.
#[test]
fn file_name_is_stored_as_an_asciz_string() {
    let text = asm_of_main(&[]);
    let found = text
        .lines()
        .any(|line| line.contains(".asciz") && line.contains("\"t.em\""));
    assert!(found, "no `.asciz \"t.em\"` line in:\n{text}");
}

/// Asserts that compiling under `file_name` puts `escaped` inside an `.asciz` line.
fn assert_file_name_escaped(file_name: &str, escaped: &str) {
    let text = asm_named(&main_with::<&str>(&[]), file_name);
    let found = text
        .lines()
        .any(|line| line.contains(".asciz") && line.contains(escaped));
    assert!(found, "no `{escaped}` in an `.asciz` line in:\n{text}");
}

/// Quotes and backslashes in the file name are escaped inside the `.asciz` string.
#[test]
fn file_name_is_escaped() {
    assert_file_name_escaped("a\"b\\c.em", "a\\\"b\\\\c.em");
}

/// The two panic reasons are string constants.
#[test]
fn panic_reasons_are_string_constants() {
    let text = asm_of_main(&[]);
    assert_contains(&text, ".asciz \"integer overflow\"");
    assert_contains(&text, ".asciz \"division by zero\"");
}

/// The panic message format is emitted with a real newline escape.
#[test]
fn panic_format_string_is_present() {
    assert_contains(&asm_of_main(&[]), "panic: %s at %s:%lld:%lld\\n");
}

/// The panic helper reaches `stderr` through the GOT.
#[test]
fn panic_helper_uses_the_got_for_stderr() {
    let text = asm_of_main(&[]);
    assert_contains(&text, "___stderrp@GOTPAGE");
    assert_contains(&text, "___stderrp@GOTPAGEOFF");
}

/// The panic helper exits with code 101.
#[test]
fn panic_helper_exits_with_101() {
    assert_contains(&asm_of_main(&[]), "mov x0, #101");
}

/// Block labels are named after the function and the block number.
#[test]
fn block_labels_use_the_function_name() {
    let text = asm(&fib_program());
    assert_eq!(count_lines(&text, "Lfib_bb0:"), 1);
    assert_eq!(count_lines(&text, "Lfib_bb1:"), 1);
    assert_eq!(count_lines(&text, "Lfib_bb2:"), 1);
}

/// Labels of different functions do not collide.
#[test]
fn block_labels_of_other_functions_are_separate() {
    let text = asm(&fib_program());
    assert_eq!(count_lines(&text, "Lmain_bb0:"), 1);
}

/// A branch jumps to the labels of its blocks.
#[test]
fn branch_targets_use_block_labels() {
    let text = asm(&fib_program());
    assert_contains(&text, "cbnz");
    assert_contains(&text, "Lfib_bb1");
    assert_contains(&text, "Lfib_bb2");
}

/// A prologue saves the frame pointer and link register.
#[test]
fn prologue_saves_the_frame_pointer_and_link_register() {
    let text = asm_of_main(&[]);
    assert_contains(&text, "stp x29, x30, [sp, #-16]!");
    assert_contains(&text, "mov x29, sp");
}

/// An epilogue restores the stack and returns.
#[test]
fn epilogue_restores_the_stack_and_returns() {
    let text = asm_of_main(&[]);
    assert_contains(&text, "mov sp, x29");
    assert_contains(&text, "ldp x29, x30, [sp], #16");
    assert_contains(&text, "ret");
}

/// A small frame is reserved with an immediate.
#[test]
fn small_frame_uses_an_immediate_subtraction() {
    let text = asm_of_main(&["x := 1", "print(x)"]);
    assert_contains(&text, "sub sp, sp, #");
    assert_lacks(&text, "sub sp, sp, x16");
}

/// A frame above 4095 bytes is probed with `___chkstk_darwin`, with its size kept in `x9`.
#[test]
fn large_frame_is_probed_and_built_in_x9() {
    let mut body: Vec<String> = (0..600).map(|i| format!("v{i} := {i}")).collect();
    body.extend((0..600).map(|i| format!("print(v{i})")));
    let text = asm(&main_with(&body));
    assert_run(&text, &["bl ___chkstk_darwin", "sub sp, sp, x9"]);
    assert_lacks(&text, "sub sp, sp, x16");
}

/// A small constant needs one `movz` and no `movk`.
#[test]
fn small_constant_uses_movz_only() {
    let text = asm_of_main(&["print(42)"]);
    assert_contains(&text, "movz x8, #42");
    assert_lacks(&text, "movk");
}

/// A constant with high chunks set is built with `movk`.
#[test]
fn wide_constant_uses_movk_with_a_shift() {
    let text = asm_of_main(&["print(1311768467463790320)"]);
    assert_contains(&text, "movk x8, #");
    assert_contains(&text, ", lsl #48");
}

/// Addition is checked with `adds`.
#[test]
fn add_uses_adds() {
    assert_contains(&asm_of_main(&["a := 1", "print(a + a)"]), "adds x8, x8, x9");
}

/// Subtraction is checked with `subs`.
#[test]
fn sub_uses_subs() {
    assert_contains(&asm_of_main(&["a := 1", "print(a - a)"]), "subs x8, x8, x9");
}

/// Multiplication checks the high half with `smulh`.
#[test]
fn mul_uses_smulh() {
    let text = asm_of_main(&["a := 1", "print(a * a)"]);
    assert_contains(&text, "mul x8, x8, x9");
    assert_contains(&text, "smulh x8, x8, x9");
}

/// Division uses `sdiv`.
#[test]
fn div_uses_sdiv() {
    assert_contains(&asm_of_main(&["a := 1", "print(a / a)"]), "sdiv x8, x8, x9");
}

/// Remainder uses `msub` after `sdiv`.
#[test]
fn rem_uses_msub() {
    let text = asm_of_main(&["a := 1", "print(a % a)"]);
    assert_contains(&text, "sdiv x8, x8, x9");
    assert_contains(&text, "msub x8, x8, x9, x10");
}

/// Negation is checked with `negs`.
#[test]
fn neg_uses_negs() {
    assert_contains(&asm_of_main(&["a := 1", "print(-a)"]), "negs x8, x8");
}

/// Logical not flips the low bit.
#[test]
fn not_uses_eor() {
    assert_contains(&asm_of_main(&["a := true", "print(!a)"]), "eor x8, x8, #1");
}

/// Each comparison compares with `cmp` and sets the result with the matching condition.
#[test]
fn comparisons_use_cset_with_the_matching_condition() {
    let cases = [
        ("==", "eq"),
        ("!=", "ne"),
        ("<", "lt"),
        ("<=", "le"),
        (">", "gt"),
        (">=", "ge"),
    ];
    for (op, condition) in cases {
        let text = asm_of_main(&["a := 1", &format!("print(a {op} a)")]);
        assert_contains(&text, "cmp x8, x9");
        assert_contains(&text, &format!("cset x8, {condition}"));
    }
}

/// The first eight parameters are stored from `x0`..`x7`.
#[test]
fn eight_parameters_are_stored_from_registers() {
    let src = program(&[
        "fn pick(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64, h: i64) -> i64 {",
        "    return a + h",
        "}",
        "fn main() {",
        "    print(pick(1, 2, 3, 4, 5, 6, 7, 8))",
        "}",
    ]);
    let text = asm(&src);
    for register in 0..8 {
        assert_contains(&text, &format!("str x{register}, [sp"));
    }
}

/// Builds a module with one function `main` made of the given instructions and terminator.
fn module_of(insts: Vec<Inst>, term: Terminator) -> Module {
    main_module(vec![block_of(0, insts, term)])
}

/// A block that ends in `Unreachable` becomes a `brk` trap.
#[test]
fn unreachable_becomes_a_trap() {
    let module = module_of(vec![], Terminator::Unreachable);
    assert_contains(&emit_ok(&module, FILE), "brk #1");
}

/// A function with more than 4096 cells (a frame above 32760 bytes) addresses far cells through `x17`.
#[test]
fn huge_frame_uses_the_x17_register_offset_form() {
    let mut body = vec!["i := 7".to_string()];
    body.extend((0..4200).map(|_| "print(i)".to_string()));
    let text = asm(&main_with(&body));
    assert_contains(&text, "[sp, x17]");
}

/// A small function never touches `x17`.
#[test]
fn small_function_does_not_use_x17() {
    let text = asm(&fib_program());
    assert_lacks(&text, "x17");
}

/// Asserts that emitting `module` fails with an internal compiler error at span 0..0.
fn assert_internal_error(module: &Module) {
    let error = arm64::emit(module, FILE).unwrap_err();
    assert!(
        error.message.starts_with("internal compiler error:"),
        "unexpected message: {}",
        error.message
    );
    assert_eq!(error.span, Span::new(0, 0));
}

/// A module whose `main` calls itself with `count` constant arguments (temp `n` holds the value `n`).
fn call_with_constants(count: u32) -> Module {
    let mut insts: Vec<Inst> = (0..count).map(|n| constant(n, i64::from(n))).collect();
    insts.push(Inst::Call {
        dst: None,
        func: "main".to_string(),
        args: (0..count).map(Temp).collect(),
    });
    module_of(insts, Terminator::Return(None))
}

/// A module whose `main` has `count` parameters named `p0`, `p1`, … and no body.
fn module_with_parameters(count: u32) -> Module {
    let mut module = module_of(vec![], Terminator::Return(None));
    module.functions[0].params = (0..count).map(i64_param).collect();
    module.functions[0].slots = (0..count).map(|n| i64_slot(format!("p{n}"))).collect();
    module
}

/// A module whose `main` has `count` unused slots and returns at once.
fn module_with_slots(count: u32) -> Module {
    let mut module = module_of(vec![], Terminator::Return(None));
    module.functions[0].slots = (0..count).map(|n| i64_slot(format!("s{n}"))).collect();
    module
}

/// The ninth argument of a call is stored on the stack, before the register arguments are loaded.
#[test]
fn call_with_nine_arguments_stores_the_ninth_on_the_stack() {
    let text = emit_ok(&call_with_constants(9), FILE);
    assert_lacks(&text, "internal compiler error");
    // The outgoing area is 16 bytes, so cell `c` is at `16 + 8c`: temp 8 is at 80, temp 0 at 16.
    assert_run(
        &text,
        &["ldr x8, [sp, #80]", "str x8, [sp, #0]", "ldr x0, [sp, #16]"],
    );
}

/// The ninth parameter is loaded from `[x29, #16]`, above the saved frame pointer and link register.
#[test]
fn ninth_parameter_is_read_above_the_frame_pointer() {
    let text = emit_ok(&module_with_parameters(9), FILE);
    assert_contains(&text, "ldr x8, [x29, #16]");
}

/// The ninth parameter is copied through a virtual register's cell (9) into its slot (8), as the naive allocator does.
#[test]
fn ninth_parameter_is_copied_into_its_slot_through_a_cell() {
    let text = emit_ok(&module_with_parameters(9), FILE);
    assert_run(
        &text,
        &[
            "ldr x8, [x29, #16]",
            "str x8, [sp, #72]",
            "ldr x8, [sp, #72]",
            "str x8, [sp, #64]",
        ],
    );
}

/// The tenth argument of a call goes to `[sp, #8]`.
#[test]
fn call_with_ten_arguments_stores_the_tenth_at_sp_plus_8() {
    let text = emit_ok(&call_with_constants(10), FILE);
    assert_run(&text, &["ldr x8, [sp, #88]", "str x8, [sp, #8]"]);
}

/// A call with nine arguments reserves a 16-byte outgoing area: 9 cells (72, rounded to 80) plus 16.
#[test]
fn call_with_nine_arguments_reserves_the_outgoing_area() {
    let text = emit_ok(&call_with_constants(9), FILE);
    assert_contains(&function_text(&text, "_em_main"), "sub sp, sp, #96");
}

/// A call with ten arguments reserves a 16-byte outgoing area: 10 cells (80) plus 16.
#[test]
fn call_with_ten_arguments_reserves_the_outgoing_area() {
    let text = emit_ok(&call_with_constants(10), FILE);
    assert_contains(&function_text(&text, "_em_main"), "sub sp, sp, #96");
}

/// A call with eight arguments needs no outgoing area: 8 cells is 64 bytes and nothing else.
#[test]
fn call_with_eight_arguments_has_no_outgoing_area() {
    let text = emit_ok(&call_with_constants(8), FILE);
    assert_contains(&function_text(&text, "_em_main"), "sub sp, sp, #64");
}

/// Eight parameters stay in registers: no load from above the frame pointer.
#[test]
fn eight_parameters_read_nothing_from_the_caller_frame() {
    let text = emit_ok(&module_with_parameters(8), FILE);
    assert_lacks(&text, "x29, #");
}

/// A load from a slot beyond the function's slots is an internal error.
#[test]
fn load_from_an_unknown_slot_is_an_internal_error() {
    let insts = vec![Inst::Load {
        dst: Temp(0),
        slot: SlotId(5),
        sign: Sign::Signed,
    }];
    assert_internal_error(&module_of(insts, Terminator::Return(None)));
}

/// A jump to a block that does not exist is an internal error.
#[test]
fn jump_to_a_missing_block_is_an_internal_error() {
    let module = module_of(vec![], Terminator::Jump(BlockId(7)));
    assert_internal_error(&module);
}

/// A non-ASCII character is escaped byte by byte in octal: U+0085 is `\302\205`.
#[test]
fn file_name_escapes_a_control_character_as_utf8_octal() {
    assert_file_name_escaped("c1\u{0085}x.em", "c1\\302\\205x.em");
}

/// A non-ASCII letter is escaped byte by byte in octal: `é` is `\303\251`.
#[test]
fn file_name_escapes_a_non_ascii_letter() {
    assert_file_name_escaped("\u{e9}.em", "\\303\\251.em");
}

/// A tab in the file name becomes `\011`.
#[test]
fn file_name_escapes_a_tab() {
    assert_file_name_escaped("a\tb.em", "a\\011b.em");
}

/// The DEL character (0x7f) becomes `\177`.
#[test]
fn file_name_escapes_delete() {
    assert_file_name_escaped("a\u{7f}b.em", "a\\177b.em");
}

/// Printable ASCII other than quote and backslash stays as written.
#[test]
fn file_name_keeps_printable_ascii() {
    assert_file_name_escaped("my file-1_x.em", "my file-1_x.em");
}

// ---------------------------------------------------------------------------------------------
// M2.1a: the machine IR and the naive allocator. The MIR modules are private to the crate, so
// these tests go through `arm64::emit` and check the exact text the naive allocator produces.
// ---------------------------------------------------------------------------------------------

/// The trimmed lines of `text`.
fn trimmed_lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// Asserts that `run` appears in `text` as consecutive lines (ignoring indentation).
fn assert_run(text: &str, run: &[&str]) {
    let lines = trimmed_lines(text);
    let found = lines.windows(run.len()).any(|window| window == run);
    assert!(found, "lines {run:#?} not found consecutively in:\n{text}");
}

/// The lines of one function's body: everything after `symbol:` up to the next directive or symbol.
fn function_text(text: &str, symbol: &str) -> String {
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

/// A program with `fn f(a: i64, b: i64) -> i64 { return a OP b }` and a `main` that calls it.
fn binary_program(op: &str) -> String {
    program(&[
        "fn f(a: i64, b: i64) -> i64 {".to_string(),
        format!("    return a {op} b"),
        "}".to_string(),
        "fn main() {".to_string(),
        "    print(f(7, 2))".to_string(),
        "}".to_string(),
    ])
}

/// Assembly for `i := 7`, 4,200 prints of `i`, then the given extra statements, all in `main`.
fn asm_of_huge_main(tail: &[&str]) -> String {
    let mut body = vec!["i := 7".to_string()];
    body.extend((0..4200).map(|_| "print(i)".to_string()));
    body.extend(tail.iter().map(|line| line.to_string()));
    asm(&main_with(&body))
}

/// A module with one function `main` that has no slots and the given blocks.
fn module_of_blocks(returns_value: bool, blocks: Vec<Block>) -> Module {
    let mut module = module_of(vec![], Terminator::Return(None));
    module.functions[0].ret = returns_value.then_some(IrType::I64);
    module.functions[0].temp_types = i64_temp_types(&blocks);
    module.functions[0].blocks = blocks;
    module
}

/// Assembly for the module with one `main` block made of `insts` and `term`.
fn asm_of_insts(insts: Vec<Inst>, term: Terminator) -> String {
    emit_ok(&module_of(insts, term), FILE)
}

/// The frame of `f(a, b)` with `*` has 6 cells: two slots, `t0`..`t2` and isel's `h`.
#[test]
fn mul_frame_counts_the_high_half_register() {
    let text = asm(&binary_program("*"));
    assert_run(&function_text(&text, "_em_f"), &["sub sp, sp, #48"]);
}

/// The product is computed from the loaded operands and stored to its own cell before the high half.
#[test]
fn mul_stores_the_product_in_its_cell() {
    let text = function_text(&asm(&binary_program("*")), "_em_f");
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "mul x8, x8, x9",
            "str x8, [sp, #32]",
        ],
    );
}

/// The high half is computed from freshly loaded operands and stored in isel's register (cell 5).
#[test]
fn mul_stores_the_high_half_in_the_isel_register_cell() {
    let text = function_text(&asm(&binary_program("*")), "_em_f");
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "smulh x8, x8, x9",
            "str x8, [sp, #40]",
        ],
    );
}

/// The overflow check compares the high half with the sign of the product, then branches.
#[test]
fn mul_overflow_check_loads_both_cells() {
    let text = function_text(&asm(&binary_program("*")), "_em_f");
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #40]",
            "ldr x9, [sp, #32]",
            "cmp x8, x9, asr #63",
            "b.ne Lf_panic0",
        ],
    );
}

/// The product is stored before the high half is computed.
#[test]
fn mul_computes_the_product_before_the_high_half() {
    let text = function_text(&asm(&binary_program("*")), "_em_f");
    assert_run(
        &text,
        &[
            "str x8, [sp, #32]",
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "smulh x8, x8, x9",
        ],
    );
}

/// The frame of `f(a, b)` with `/` has 6 cells: two slots, `t0`..`t2` and isel's `m`.
#[test]
fn div_frame_counts_the_minimum_register() {
    let text = asm(&binary_program("/"));
    assert_run(&function_text(&text, "_em_f"), &["sub sp, sp, #48"]);
}

/// The zero check reloads the divisor into `x8` and branches to the first panic label.
#[test]
fn div_zero_check_loads_the_divisor() {
    let text = function_text(&asm(&binary_program("/")), "_em_f");
    assert_run(
        &text,
        &["ldr x8, [sp, #24]", "cmp x8, #0", "b.eq Lf_panic0"],
    );
}

/// The `-1` check reloads the divisor and skips the `MIN` check through the `ok` label.
#[test]
fn div_minus_one_check_loads_the_divisor() {
    let text = function_text(&asm(&binary_program("/")), "_em_f");
    assert_run(&text, &["ldr x8, [sp, #24]", "cmn x8, #1", "b.ne Lf_ok1"]);
}

/// `i64::MIN` is built in `x8` and stored in isel's register (cell 5), then compared with the dividend.
#[test]
fn div_min_check_goes_through_the_isel_register() {
    let text = function_text(&asm(&binary_program("/")), "_em_f");
    assert_run(
        &text,
        &[
            "movz x8, #0",
            "movk x8, #32768, lsl #48",
            "str x8, [sp, #40]",
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #40]",
            "cmp x8, x9",
            "b.eq Lf_panic2",
            "Lf_ok1:",
        ],
    );
}

/// After the `ok` label the quotient is computed from the reloaded operands and stored.
#[test]
fn div_quotient_is_loaded_computed_and_stored() {
    let text = function_text(&asm(&binary_program("/")), "_em_f");
    assert_run(
        &text,
        &[
            "Lf_ok1:",
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "sdiv x8, x8, x9",
            "str x8, [sp, #32]",
        ],
    );
}

/// The zero-division panic label is numbered before the `ok` label, which is numbered before the overflow label.
#[test]
fn div_labels_are_numbered_in_creation_order() {
    let text = function_text(&asm(&binary_program("/")), "_em_f");
    let lines = trimmed_lines(&text);
    let position = |label: &str| lines.iter().position(|line| *line == label).unwrap();
    assert!(position("Lf_ok1:") < position("Lf_panic0:"));
    assert!(position("Lf_panic0:") < position("Lf_panic2:"));
}

/// The frame of `f(a, b)` with `%` has 7 cells: two slots, `t0`..`t2`, `m` and `q` (56 bytes, rounded to 64).
#[test]
fn rem_frame_counts_both_isel_registers() {
    let text = asm(&binary_program("%"));
    assert_run(&function_text(&text, "_em_f"), &["sub sp, sp, #64"]);
}

/// The quotient of a remainder is stored in isel's register `q` (cell 6).
#[test]
fn rem_stores_the_quotient_in_the_isel_register_cell() {
    let text = function_text(&asm(&binary_program("%")), "_em_f");
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #16]",
            "ldr x9, [sp, #24]",
            "sdiv x8, x8, x9",
            "str x8, [sp, #48]",
        ],
    );
}

/// `msub` reads the quotient, the divisor and the dividend into `x8`, `x9` and `x10`.
#[test]
fn rem_msub_reads_quotient_divisor_dividend() {
    let text = function_text(&asm(&binary_program("%")), "_em_f");
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #48]",
            "ldr x9, [sp, #24]",
            "ldr x10, [sp, #16]",
            "msub x8, x8, x9, x10",
            "str x8, [sp, #32]",
        ],
    );
}

/// `rem` keeps the zero check and the `MIN / -1` check of `div`.
#[test]
fn rem_keeps_the_division_checks() {
    let text = function_text(&asm(&binary_program("%")), "_em_f");
    assert_run(
        &text,
        &["ldr x8, [sp, #24]", "cmp x8, #0", "b.eq Lf_panic0"],
    );
    assert_run(&text, &["ldr x8, [sp, #24]", "cmn x8, #1", "b.ne Lf_ok1"]);
}

/// `*` uses only `x8` and `x9`; `x10` and `x11` are not touched.
#[test]
fn mul_uses_no_scratch_beyond_x9() {
    let text = function_text(&asm(&binary_program("*")), "_em_f");
    assert_lacks(&text, "x10");
    assert_lacks(&text, "x11");
}

/// `/` uses only `x8` and `x9`; `x10` and `x11` are not touched.
#[test]
fn div_uses_no_scratch_beyond_x9() {
    let text = function_text(&asm(&binary_program("/")), "_em_f");
    assert_lacks(&text, "x10");
    assert_lacks(&text, "x11");
}

/// Only `%` reads three registers, so only `%` uses `x10`, and nothing uses `x11`.
#[test]
fn rem_uses_x10_but_not_x11() {
    let text = function_text(&asm(&binary_program("%")), "_em_f");
    assert_contains(&text, "x10");
    assert_lacks(&text, "x11");
}

/// `main` with a plain `print(1)` uses neither `x10` nor `x11` (the runtime helpers do, so only `main` is checked).
#[test]
fn print_main_uses_no_x10_or_x11() {
    let text = function_text(&asm_of_main(&["print(1)"]), "_em_main");
    assert_lacks(&text, "x10");
    assert_lacks(&text, "x11");
}

/// `fib` copies its parameter into its slot (cell 0) from `x0`.
#[test]
fn fib_parameter_is_stored_in_cell_zero() {
    let text = function_text(&asm(&fib_program()), "_em_fib");
    assert_run(&text, &["str x0, [sp, #0]", "Lfib_bb0:"]);
}

/// The first block of `fib` loads the slot and stores the temp in the next cell.
#[test]
fn fib_first_block_loads_the_slot_into_its_temp_cell() {
    let text = function_text(&asm(&fib_program()), "_em_fib");
    assert_run(
        &text,
        &["Lfib_bb0:", "ldr x8, [sp, #0]", "str x8, [sp, #8]"],
    );
}

/// In `fib` the first `subs` is followed by its overflow branch, then by the store of its result.
#[test]
fn fib_branches_on_the_difference_before_storing_it() {
    let text = function_text(&asm(&fib_program()), "_em_fib");
    let lines = trimmed_lines(&text);
    let subs = lines.iter().position(|line| *line == "subs x8, x8, x9");
    assert!(subs.is_some(), "no `subs x8, x8, x9` in:\n{text}");
    let subs = subs.unwrap();
    assert_eq!(lines[subs + 1], "b.vs Lfib_panic0");
    assert!(
        lines[subs + 2].starts_with("str x8, [sp, #"),
        "line after the branch is `{}`",
        lines[subs + 2]
    );
}

/// In `fib` every `adds` and `subs` is followed by a `b.vs`, then by a store of `x8`.
#[test]
fn every_checked_add_and_sub_branches_before_storing() {
    let text = function_text(&asm(&fib_program()), "_em_fib");
    let lines = trimmed_lines(&text);
    let checked = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("adds ") || line.starts_with("subs "))
        .map(|(index, _)| index)
        .collect::<Vec<usize>>();
    assert_eq!(
        checked.len(),
        3,
        "expected two subs and one adds in:\n{text}"
    );
    for index in checked {
        assert!(
            lines[index + 1].starts_with("b.vs Lfib_panic"),
            "after `{}`",
            lines[index]
        );
        assert!(
            lines[index + 2].starts_with("str x8, [sp, #"),
            "after the branch of `{}`",
            lines[index]
        );
    }
}

/// Phase 1's `_em_fib` body (from `feat/m1.6-driver`), with the unindented labels and indented instructions as printed.
const PHASE1_FIB_BODY: &[&str] = &[
    "    stp x29, x30, [sp, #-16]!",
    "    mov x29, sp",
    "    sub sp, sp, #112",
    "    str x0, [sp, #0]",
    "Lfib_bb0:",
    "    ldr x8, [sp, #0]",
    "    str x8, [sp, #8]",
    "    movz x8, #2",
    "    str x8, [sp, #16]",
    "    ldr x8, [sp, #8]",
    "    ldr x9, [sp, #16]",
    "    cmp x8, x9",
    "    cset x8, lt",
    "    str x8, [sp, #24]",
    "    ldr x8, [sp, #24]",
    "    cbnz x8, Lfib_bb1",
    "    b Lfib_bb2",
    "Lfib_bb1:",
    "    ldr x8, [sp, #0]",
    "    str x8, [sp, #32]",
    "    ldr x0, [sp, #32]",
    "    mov sp, x29",
    "    ldp x29, x30, [sp], #16",
    "    ret",
    "Lfib_bb2:",
    "    ldr x8, [sp, #0]",
    "    str x8, [sp, #40]",
    "    movz x8, #1",
    "    str x8, [sp, #48]",
    "    ldr x8, [sp, #40]",
    "    ldr x9, [sp, #48]",
    "    subs x8, x8, x9",
    "    b.vs Lfib_panic0",
    "    str x8, [sp, #56]",
    "    ldr x0, [sp, #56]",
    "    bl _em_fib",
    "    str x0, [sp, #64]",
    "    ldr x8, [sp, #0]",
    "    str x8, [sp, #72]",
    "    movz x8, #2",
    "    str x8, [sp, #80]",
    "    ldr x8, [sp, #72]",
    "    ldr x9, [sp, #80]",
    "    subs x8, x8, x9",
    "    b.vs Lfib_panic1",
    "    str x8, [sp, #88]",
    "    ldr x0, [sp, #88]",
    "    bl _em_fib",
    "    str x0, [sp, #96]",
    "    ldr x8, [sp, #64]",
    "    ldr x9, [sp, #96]",
    "    adds x8, x8, x9",
    "    b.vs Lfib_panic2",
    "    str x8, [sp, #104]",
    "    ldr x0, [sp, #104]",
    "    mov sp, x29",
    "    ldp x29, x30, [sp], #16",
    "    ret",
    "Lfib_panic0:",
    "    movz x0, #5",
    "    movz x1, #16",
    "    adrp x2, Lstr_overflow@PAGE",
    "    add x2, x2, Lstr_overflow@PAGEOFF",
    "    bl _ember_rt_panic",
    "Lfib_panic1:",
    "    movz x0, #5",
    "    movz x1, #29",
    "    adrp x2, Lstr_overflow@PAGE",
    "    add x2, x2, Lstr_overflow@PAGEOFF",
    "    bl _ember_rt_panic",
    "Lfib_panic2:",
    "    movz x0, #5",
    "    movz x1, #12",
    "    adrp x2, Lstr_overflow@PAGE",
    "    add x2, x2, Lstr_overflow@PAGEOFF",
    "    bl _ember_rt_panic",
    "    .globl _em_main",
    "    .p2align 2",
];

/// `fib` has no `mul`, `div` or `rem`, so its body is byte-identical to Phase 1's (comments and blank lines aside).
#[test]
fn fib_body_is_identical_to_phase_1() {
    let text = function_text(&asm(&fib_program()), "_em_fib");
    let actual: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter(|line| !line.trim_start().starts_with("//") && !line.trim_start().starts_with(';'))
        .collect();
    assert_eq!(actual, PHASE1_FIB_BODY);
}

/// A `Const` is built in `x8` and stored in its temp's cell.
#[test]
fn const_is_built_in_x8_and_stored() {
    let text = asm_of_insts(vec![constant(0, 42)], Terminator::Return(None));
    assert_run(&text, &["movz x8, #42", "str x8, [sp, #0]"]);
}

/// A slot store loads the temp into `x8` and stores it in the slot's cell.
#[test]
fn slot_store_loads_the_temp_then_stores() {
    let mut module = module_of(
        vec![
            constant(0, 5),
            Inst::Store {
                slot: SlotId(0),
                src: Temp(0),
            },
        ],
        Terminator::Return(None),
    );
    module.functions[0].slots = vec![i64_slot("x".to_string())];
    // One slot, so the temp's cell is cell 1.
    assert_run(
        &emit_ok(&module, FILE),
        &["ldr x8, [sp, #8]", "str x8, [sp, #0]"],
    );
}

/// A slot load loads the slot into `x8` and stores it in the temp's cell.
#[test]
fn slot_load_loads_the_slot_then_stores_the_temp() {
    let mut module = module_of(
        vec![Inst::Load {
            dst: Temp(0),
            slot: SlotId(0),
            sign: Sign::Signed,
        }],
        Terminator::Return(None),
    );
    module.functions[0].slots = vec![i64_slot("x".to_string())];
    assert_run(
        &emit_ok(&module, FILE),
        &["ldr x8, [sp, #0]", "str x8, [sp, #8]"],
    );
}

/// An instruction that reads one register twice loads it once and uses the same scratch for both operands.
#[test]
fn a_register_read_twice_uses_one_scratch() {
    let text = asm_of_insts(
        vec![constant(0, 1), binary(1, BinOp::Add(Sign::Signed), 0, 0)],
        Terminator::Return(None),
    );
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #0]",
            "adds x8, x8, x8",
            "b.vs Lmain_panic0",
            "str x8, [sp, #8]",
        ],
    );
}

/// A comparison loads both operands, sets the result in `x8`, and stores it; `cmp` itself stores nothing.
#[test]
fn comparison_loads_compares_sets_and_stores() {
    let text = asm_of_insts(
        vec![
            constant(0, 1),
            constant(1, 2),
            binary(2, BinOp::Lt(Sign::Signed), 0, 1),
        ],
        Terminator::Return(None),
    );
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #0]",
            "ldr x9, [sp, #8]",
            "cmp x8, x9",
            "cset x8, lt",
            "str x8, [sp, #16]",
        ],
    );
}

/// Negation loads the operand, negates in `x8`, branches on overflow, then stores the result.
#[test]
fn neg_loads_negates_branches_then_stores() {
    let neg = Inst::Neg {
        dst: Temp(1),
        src: Temp(0),
        loc: Loc { line: 3, col: 5 },
    };
    let text = asm_of_insts(vec![constant(0, 1), neg], Terminator::Return(None));
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #0]",
            "negs x8, x8",
            "b.vs Lmain_panic0",
            "str x8, [sp, #8]",
        ],
    );
}

/// Logical not loads the operand, flips the low bit in `x8` and stores the result.
#[test]
fn not_loads_flips_and_stores() {
    let not = Inst::Not {
        dst: Temp(1),
        src: Temp(0),
    };
    let text = asm_of_insts(vec![constant(0, 1), not], Terminator::Return(None));
    assert_run(
        &text,
        &["ldr x8, [sp, #0]", "eor x8, x8, #1", "str x8, [sp, #8]"],
    );
}

/// A branch loads its condition into `x8`, then `cbnz` to the then-block and `b` to the else-block.
#[test]
fn branch_loads_the_condition_into_x8() {
    let module = module_of_blocks(
        false,
        vec![
            block_of(
                0,
                vec![constant(0, 1)],
                Terminator::Branch {
                    cond: Temp(0),
                    then_to: BlockId(1),
                    else_to: BlockId(2),
                },
            ),
            block_of(1, vec![], Terminator::Return(None)),
            block_of(2, vec![], Terminator::Return(None)),
        ],
    );
    assert_run(
        &emit_ok(&module, FILE),
        &["ldr x8, [sp, #0]", "cbnz x8, Lmain_bb1", "b Lmain_bb2"],
    );
}

/// A call loads each argument straight into its argument register, calls, and stores `x0` in the result cell.
#[test]
fn call_loads_arguments_into_registers_and_stores_the_result() {
    let call = Inst::Call {
        dst: Some(Temp(2)),
        func: "main".to_string(),
        args: vec![Temp(0), Temp(1)],
    };
    let text = asm_of_insts(
        vec![constant(0, 1), constant(1, 2), call],
        Terminator::Return(None),
    );
    assert_run(
        &text,
        &[
            "ldr x0, [sp, #0]",
            "ldr x1, [sp, #8]",
            "bl _em_main",
            "str x0, [sp, #16]",
        ],
    );
}

/// A call without a result stores nothing after `bl`.
#[test]
fn call_without_a_result_stores_nothing() {
    let call = Inst::Call {
        dst: None,
        func: "main".to_string(),
        args: vec![],
    };
    let text = asm_of_insts(vec![call], Terminator::Return(None));
    assert_run(&text, &["bl _em_main", "mov sp, x29"]);
}

/// A print loads its operand straight into `x0` and calls the helper for its kind.
#[test]
fn print_loads_x0_and_calls_the_helper() {
    let print = Inst::Print {
        src: Temp(0),
        kind: PrintKind::I64,
    };
    let text = asm_of_insts(vec![constant(0, 5), print], Terminator::Return(None));
    assert_run(&text, &["ldr x0, [sp, #0]", "bl _ember_rt_print_i64"]);
}

/// Returning a value loads it into `x0` before the epilogue.
#[test]
fn return_value_is_loaded_into_x0_before_the_epilogue() {
    let module = module_of_blocks(
        true,
        vec![block_of(
            0,
            vec![constant(0, 5)],
            Terminator::Return(Some(Temp(0))),
        )],
    );
    assert_run(
        &emit_ok(&module, FILE),
        &[
            "ldr x0, [sp, #0]",
            "mov sp, x29",
            "ldp x29, x30, [sp], #16",
            "ret",
        ],
    );
}

/// A panic path loads the source line and column into `x0` and `x1` and the reason into `x2`, then calls the runtime.
#[test]
fn panic_path_passes_line_column_and_reason() {
    let text = asm_of_insts(
        vec![constant(0, 1), binary(1, BinOp::Add(Sign::Signed), 0, 0)],
        Terminator::Return(None),
    );
    assert_run(
        &text,
        &[
            "Lmain_panic0:",
            "movz x0, #3",
            "movz x1, #5",
            "adrp x2, Lstr_overflow@PAGE",
            "add x2, x2, Lstr_overflow@PAGEOFF",
            "bl _ember_rt_panic",
        ],
    );
}

/// In a function with more than 4,096 cells the high half's cell is reached through `x17`.
#[test]
fn far_high_half_cell_is_addressed_through_x17() {
    let text = asm_of_huge_main(&["print(i * i)"]);
    let lines = trimmed_lines(&text);
    let smulh = lines.iter().position(|line| *line == "smulh x8, x8, x9");
    assert!(smulh.is_some(), "no smulh found");
    let smulh = smulh.unwrap();
    assert!(
        lines[smulh + 1].starts_with("movz x17, #"),
        "line after smulh is `{}`",
        lines[smulh + 1]
    );
    assert_eq!(lines[smulh + 2], "str x8, [sp, x17]");
}

/// In a function with more than 4,096 cells the `MIN` constant's cell is reached through `x17`.
#[test]
fn far_min_constant_cell_is_addressed_through_x17() {
    let text = asm_of_huge_main(&["z := 3", "print(i / z)"]);
    let lines = trimmed_lines(&text);
    let movk = lines
        .iter()
        .position(|line| *line == "movk x8, #32768, lsl #48");
    assert!(movk.is_some(), "no MIN constant found");
    let movk = movk.unwrap();
    assert!(
        lines[movk + 1].starts_with("movz x17, #"),
        "line after movk is `{}`",
        lines[movk + 1]
    );
    assert_eq!(lines[movk + 2], "str x8, [sp, x17]");
}

/// The frame of a function with more than 4,096 cells is probed, with its size (33,648 bytes: 4,206 cells) in `x9`.
#[test]
fn far_frame_with_isel_registers_is_probed_and_built_in_x9() {
    let text = asm_of_huge_main(&["print(i * i)"]);
    assert_run(
        &text,
        &["movz x9, #33648", "bl ___chkstk_darwin", "sub sp, sp, x9"],
    );
    assert_lacks(&text, "x16");
}

/// No `[sp, #offset]` in a function with more than 4,096 cells exceeds the 32760-byte immediate range.
#[test]
fn far_frame_never_prints_an_immediate_offset_above_32760() {
    let text = asm_of_huge_main(&["print(i * i)", "z := 3", "print(i % z)"]);
    for line in text.lines() {
        let Some((_, rest)) = line.split_once("[sp, #") else {
            continue;
        };
        // The prologue's `[sp, #-16]!` is a pre-index, not a frame offset.
        let digits = rest.split(']').next().unwrap();
        let Ok(offset) = digits.parse::<u64>() else {
            continue;
        };
        assert!(offset <= 32760, "offset out of range in `{line}`");
    }
}

/// A `mul` whose destination temp is `u32::MAX` (isel's own registers would not fit) is an internal error, not a panic.
#[test]
fn mul_into_the_largest_temp_is_an_internal_error() {
    let insts = vec![
        constant(0, 1),
        binary(u32::MAX, BinOp::Mul(Sign::Signed), 0, 0),
    ];
    assert_internal_error(&module_of(insts, Terminator::Return(None)));
}

/// A `Const` into temp `u32::MAX` (the temp count would overflow) is an internal error, not a panic.
#[test]
fn const_into_the_largest_temp_is_an_internal_error() {
    let insts = vec![constant(u32::MAX, 1)];
    assert_internal_error(&module_of(insts, Terminator::Return(None)));
}

// ---------------------------------------------------------------------------------------------
// M2.1b: branch relaxation. A conditional branch reaches -1,048,576 ..= +1,048,572 bytes; beyond
// that it becomes `b.<inverse> Lskip` / `b target` / `Lskip:` (`cbz` for `cbnz`).
// ---------------------------------------------------------------------------------------------

/// The farthest forward distance in bytes that a conditional branch encodes.
const MAX_FORWARD: i64 = 1_048_572;

/// The farthest backward distance in bytes that a conditional branch encodes.
const MAX_BACKWARD: i64 = -1_048_576;

/// Enough `Unreachable` blocks (one instruction each) to push any panic path out of range.
const FAR_FILLERS: u32 = 262_200;

/// How many `x += 1` statements the large program has: its first panic path is over 1 MiB away.
const BIG_COUNT: usize = 20_000;

/// Assembly of the 20,000-increment program, built once and shared by the tests that read it.
fn big_asm() -> &'static str {
    static ASM: OnceLock<String> = OnceLock::new();
    ASM.get_or_init(|| asm(&counter_program("x := 0", BIG_COUNT)))
}

/// The body of `_em_main` in the large program, built once.
fn big_main() -> &'static str {
    static MAIN: OnceLock<String> = OnceLock::new();
    MAIN.get_or_init(|| function_text(big_asm(), "_em_main"))
}

/// The lines of `_em_main` in the large program, trimmed.
fn big_main_lines() -> Vec<&'static str> {
    trimmed_lines(big_main())
}

/// True when `line` (trimmed) is a machine instruction: not a label and not a directive.
fn is_instruction(line: &str) -> bool {
    !line.is_empty() && !line.starts_with('.') && !line.ends_with(':')
}

/// The distance in bytes from the first line equal to `branch` to the line `label:`, one 4-byte step per printed instruction.
fn distance_in_bytes(text: &str, branch: &str, label: &str) -> i64 {
    let label_line = format!("{label}:");
    let mut position = 0;
    let mut branch_position = None;
    let mut label_position = None;
    for line in trimmed_lines(text) {
        if line == branch && branch_position.is_none() {
            branch_position = Some(position);
        }
        if line == label_line {
            label_position = Some(position);
        }
        if is_instruction(line) {
            position += 4;
        }
    }
    let branch_position = branch_position.unwrap();
    let label_position = label_position.unwrap();
    label_position - branch_position
}

/// A `main` whose first block adds two constants, then returns; `fillers` unreachable blocks follow, then the panic path.
fn add_then_fillers(fillers: u32) -> Module {
    let mut blocks = vec![block_of(
        0,
        vec![
            constant(0, 1),
            constant(1, 1),
            binary(2, BinOp::Add(Sign::Signed), 0, 1),
        ],
        Terminator::Return(None),
    )];
    blocks.extend(unreachable_blocks(1, fillers));
    main_module(blocks)
}

/// A `main` whose first block runs `insts` and returns, followed by `FAR_FILLERS` unreachable blocks.
fn far_panic_module(insts: Vec<Inst>) -> Module {
    let mut blocks = vec![block_of(0, insts, Terminator::Return(None))];
    blocks.extend(unreachable_blocks(1, FAR_FILLERS));
    main_module(blocks)
}

/// A `main` whose block 0 branches forward on a true condition to block `fillers + 1`, past `fillers` unreachable blocks.
fn forward_branch_module(fillers: u32) -> Module {
    let last = fillers + 1;
    let branch = Terminator::Branch {
        cond: Temp(0),
        then_to: BlockId(last),
        else_to: BlockId(1),
    };
    let mut blocks = vec![block_of(0, vec![constant(0, 1)], branch)];
    blocks.extend(unreachable_blocks(1, fillers));
    blocks.push(block_of(last, vec![], Terminator::Return(None)));
    main_module(blocks)
}

/// A `main` that jumps to block 1, passes `fillers` more unreachable blocks, and branches back to block 1 on a true condition.
fn backward_branch_module(fillers: u32) -> Module {
    let branch_block = fillers + 2;
    let end = fillers + 3;
    let branch = Terminator::Branch {
        cond: Temp(0),
        then_to: BlockId(1),
        else_to: BlockId(end),
    };
    let mut blocks = vec![block_of(0, vec![], Terminator::Jump(BlockId(1)))];
    blocks.extend(unreachable_blocks(1, fillers + 1));
    blocks.push(block_of(branch_block, vec![constant(0, 1)], branch));
    blocks.push(block_of(end, vec![], Terminator::Return(None)));
    main_module(blocks)
}

/// The first overflow check of the large program is relaxed: inverted branch over a jump to the panic path.
#[test]
fn big_program_first_check_is_relaxed() {
    let lines = big_main_lines();
    let adds = lines.iter().position(|line| *line == "adds x8, x8, x9");
    assert!(adds.is_some(), "no `adds x8, x8, x9`");
    let adds = adds.unwrap();
    assert_eq!(
        lines[adds + 1..adds + 4],
        ["b.vc Lmain_skip20000", "b Lmain_panic0", "Lmain_skip20000:"]
    );
}

/// The first relaxation label continues the counter after isel's 20,000 panic labels.
#[test]
fn big_program_numbers_skip_labels_after_the_panic_labels() {
    assert!(big_asm().contains("Lmain_panic19999:"));
    assert!(big_asm().contains("Lmain_skip20000:"));
    assert!(!big_asm().contains("Lmain_skip19999:"));
}

/// The result of the relaxed first check is stored after the branch, as in an unrelaxed check.
#[test]
fn big_program_stores_the_sum_after_the_relaxed_branch() {
    let lines = big_main_lines();
    let label = lines.iter().position(|line| *line == "Lmain_skip20000:");
    assert!(label.is_some(), "no `Lmain_skip20000:`");
    assert!(
        lines[label.unwrap() + 1].starts_with("str x8, [sp, #"),
        "line after the skip label is `{}`",
        lines[label.unwrap() + 1]
    );
}

/// The last increment's panic path is within 1 MiB, so its check stays a plain `b.vs`.
#[test]
fn big_program_last_check_stays_short() {
    let lines = big_main_lines();
    assert!(lines.contains(&"b.vs Lmain_panic19999"));
    assert!(!lines.contains(&"b Lmain_panic19999"));
}

/// Every relaxed branch of the large program is followed by a `b` to a panic path and then its own skip label.
#[test]
fn big_program_relaxed_branches_have_the_three_line_form() {
    let lines = big_main_lines();
    let mut relaxed = 0;
    for (index, line) in lines.iter().enumerate() {
        let Some(skip) = line.strip_prefix("b.vc ") else {
            continue;
        };
        assert!(skip.starts_with("Lmain_skip"), "unexpected `{line}`");
        assert!(
            lines[index + 1].starts_with("b Lmain_panic"),
            "after `{line}`"
        );
        assert_eq!(lines[index + 2], format!("{skip}:"));
        relaxed += 1;
    }
    assert!(relaxed > 0, "no relaxed branch found");
}

/// Every skip label of the large program is defined exactly once.
#[test]
fn big_program_defines_each_skip_label_once() {
    let lines = big_main_lines();
    let mut seen = std::collections::HashSet::new();
    for line in lines.iter().filter(|line| line.starts_with("Lmain_skip")) {
        assert!(seen.insert(*line), "`{line}` defined twice");
    }
    assert!(!seen.is_empty());
}

/// The large program prints the same assembly on two separate compilations.
#[test]
fn big_program_output_is_deterministic() {
    let again = asm(&counter_program("x := 0", BIG_COUNT));
    assert!(big_asm() == again, "the two assemblies differ");
}

/// A relaxed module prints the same assembly twice.
#[test]
fn relaxed_module_output_is_deterministic() {
    let module = far_panic_module(vec![
        constant(0, 7),
        constant(1, 0),
        binary(2, BinOp::Div(Sign::Signed), 0, 1),
    ]);
    assert_eq!(emit_ok(&module, FILE), emit_ok(&module, FILE));
}

/// Programs under 1 MiB are never relaxed: no skip label, no inverted check, no `cbz`.
#[test]
fn programs_under_1_mib_are_not_relaxed() {
    let everything = program(&[
        "fn f(a: i64, b: i64) -> i64 {",
        "    if a < b {",
        "        return a * b - a / b + a % b",
        "    }",
        "    return -a",
        "}",
        "fn main() {",
        "    for i in 0..10 {",
        "        print(f(i, 3))",
        "    }",
        "}",
    ]);
    let mut huge = vec!["i := 7".to_string()];
    huge.extend((0..4200).map(|_| "print(i)".to_string()));
    huge.push("print(i * i)".to_string());
    for src in [fib_program(), everything, main_with(&huge)] {
        let text = asm(&src);
        assert_lacks(&text, "_skip");
        assert_lacks(&text, "b.vc");
        assert_lacks(&text, "cbz");
    }
}

/// A `cbnz` exactly 1,048,572 bytes ahead stays a `cbnz`.
#[test]
fn forward_cbnz_at_the_largest_distance_stays_short() {
    let text = emit_ok(&forward_branch_module(262_141), FILE);
    assert_contains(&text, "cbnz x8, Lmain_bb262142");
    assert_lacks(&text, "_skip");
    let distance = distance_in_bytes(&text, "cbnz x8, Lmain_bb262142", "Lmain_bb262142");
    assert_eq!(distance, MAX_FORWARD);
}

/// A `cbnz` one instruction beyond +1,048,572 bytes becomes `cbz` over a `b`.
#[test]
fn forward_cbnz_one_instruction_further_is_relaxed() {
    let text = emit_ok(&forward_branch_module(262_142), FILE);
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #0]",
            "cbz x8, Lmain_skip0",
            "b Lmain_bb262143",
            "Lmain_skip0:",
            "b Lmain_bb1",
        ],
    );
}

/// A relaxed `cbnz` leaves no `cbnz` to the far block.
#[test]
fn relaxed_cbnz_is_gone() {
    let text = emit_ok(&forward_branch_module(262_142), FILE);
    assert_lacks(&text, "cbnz");
}

/// A `cbnz` exactly 1,048,576 bytes back stays a `cbnz`.
#[test]
fn backward_cbnz_at_the_largest_distance_stays_short() {
    let text = emit_ok(&backward_branch_module(262_140), FILE);
    assert_contains(&text, "cbnz x8, Lmain_bb1");
    assert_lacks(&text, "_skip");
    let distance = distance_in_bytes(&text, "cbnz x8, Lmain_bb1", "Lmain_bb1");
    assert_eq!(distance, MAX_BACKWARD);
}

/// A `cbnz` one instruction beyond -1,048,576 bytes becomes `cbz` over a `b`.
#[test]
fn backward_cbnz_one_instruction_further_is_relaxed() {
    let text = emit_ok(&backward_branch_module(262_141), FILE);
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #0]",
            "cbz x8, Lmain_skip0",
            "b Lmain_bb1",
            "Lmain_skip0:",
            "b Lmain_bb262144",
        ],
    );
}

/// An overflow check exactly 1,048,572 bytes from its panic path stays a plain `b.vs`.
#[test]
fn overflow_check_at_the_largest_distance_stays_short() {
    let text = emit_ok(&add_then_fillers(262_138), FILE);
    assert_contains(&text, "b.vs Lmain_panic0");
    assert_lacks(&text, "_skip");
    let distance = distance_in_bytes(&text, "b.vs Lmain_panic0", "Lmain_panic0");
    assert_eq!(distance, MAX_FORWARD);
}

/// An overflow check one instruction beyond +1,048,572 bytes becomes `b.vc` over a `b`, then stores.
#[test]
fn overflow_check_one_instruction_further_is_relaxed() {
    let text = emit_ok(&add_then_fillers(262_139), FILE);
    assert_run(
        &text,
        &[
            "adds x8, x8, x9",
            "b.vc Lmain_skip1",
            "b Lmain_panic0",
            "Lmain_skip1:",
            "str x8, [sp, #16]",
        ],
    );
}

/// A far multiplication check inverts `b.ne` to `b.eq`.
#[test]
fn far_mul_check_is_inverted_to_eq() {
    let insts = vec![
        constant(0, 3),
        constant(1, 4),
        binary(2, BinOp::Mul(Sign::Signed), 0, 1),
    ];
    let text = emit_ok(&far_panic_module(insts), FILE);
    assert_run(
        &text,
        &[
            "cmp x8, x9, asr #63",
            "b.eq Lmain_skip1",
            "b Lmain_panic0",
            "Lmain_skip1:",
        ],
    );
}

/// A far negation check inverts `b.vs` to `b.vc`.
#[test]
fn far_neg_check_is_inverted_to_vc() {
    let neg = Inst::Neg {
        dst: Temp(1),
        src: Temp(0),
        loc: Loc { line: 3, col: 5 },
    };
    let text = emit_ok(&far_panic_module(vec![constant(0, 5), neg]), FILE);
    assert_run(
        &text,
        &[
            "negs x8, x8",
            "b.vc Lmain_skip1",
            "b Lmain_panic0",
            "Lmain_skip1:",
            "str x8, [sp, #8]",
        ],
    );
}

/// A far division-by-zero check inverts `b.eq` to `b.ne`; the next `ok` branch stays short.
#[test]
fn far_div_zero_check_is_inverted_to_ne() {
    let insts = vec![
        constant(0, 7),
        constant(1, 2),
        binary(2, BinOp::Div(Sign::Signed), 0, 1),
    ];
    let text = emit_ok(&far_panic_module(insts), FILE);
    assert_run(
        &text,
        &[
            "cmp x8, #0",
            "b.ne Lmain_skip3",
            "b Lmain_panic0",
            "Lmain_skip3:",
            "ldr x8, [sp, #8]",
            "cmn x8, #1",
            "b.ne Lmain_ok1",
        ],
    );
}

/// A far `MIN / -1` check inverts `b.eq` to `b.ne`; skip labels follow instruction order.
#[test]
fn far_div_min_check_is_inverted_to_ne() {
    let insts = vec![
        constant(0, 7),
        constant(1, 2),
        binary(2, BinOp::Div(Sign::Signed), 0, 1),
    ];
    let text = emit_ok(&far_panic_module(insts), FILE);
    assert_run(
        &text,
        &[
            "cmp x8, x9",
            "b.ne Lmain_skip4",
            "b Lmain_panic2",
            "Lmain_skip4:",
            "Lmain_ok1:",
        ],
    );
}

/// Relaxation keeps the `ok` label and the panic labels where isel put them.
#[test]
fn relaxation_keeps_isel_labels() {
    let insts = vec![
        constant(0, 7),
        constant(1, 2),
        binary(2, BinOp::Div(Sign::Signed), 0, 1),
    ];
    let text = emit_ok(&far_panic_module(insts), FILE);
    for label in ["Lmain_panic0:", "Lmain_ok1:", "Lmain_panic2:"] {
        assert_eq!(count_lines(&text, label), 1, "{label}");
    }
}

// ---------------------------------------------------------------------------------------------
// M2.1c: stack-passed arguments, stack probes and the 4 MiB frame limit.
// ---------------------------------------------------------------------------------------------

/// The largest frame, in bytes, that `emit` accepts.
const MAX_FRAME: u64 = 4_194_304;

/// The rendered message for a function whose frame is `bytes` long.
fn frame_error(name: &str, bytes: u64) -> String {
    format!("stack frame of `{name}` is too large: {bytes} bytes (the limit is {MAX_FRAME} bytes)")
}

/// A program with `fn f(a, b, c, d, e, f, g, h, i, j)` (empty body) and a `main` that calls it with 1..10.
fn ten_parameter_program() -> String {
    program(&[
        "fn f(a: i64, b: i64, c: i64, d: i64, e: i64, p6: i64, g: i64, h: i64, i: i64, j: i64) {",
        "}",
        "fn main() {",
        "    f(1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
        "}",
    ])
}

/// A function with a 16-byte frame (`print(1)` in `main`) is not probed.
#[test]
fn small_frame_has_no_stack_probe() {
    let text = function_text(&asm_of_main(&["print(1)"]), "_em_main");
    assert_lacks(&text, "___chkstk_darwin");
}

/// A frame of exactly 4,096 bytes (511 slots, 4,088 rounded up) is probed, with its size in `x9`.
#[test]
fn frame_of_4096_bytes_is_probed() {
    let text = emit_ok(&module_with_slots(511), FILE);
    assert_run(
        &text,
        &[
            "mov x29, sp",
            "movz x9, #4096",
            "bl ___chkstk_darwin",
            "sub sp, sp, x9",
        ],
    );
}

/// A frame of 4,080 bytes (510 slots) is below the threshold: an immediate subtraction and no probe.
#[test]
fn frame_of_4080_bytes_is_not_probed() {
    let text = emit_ok(&module_with_slots(510), FILE);
    assert_contains(&text, "sub sp, sp, #4080");
    assert_lacks(&text, "___chkstk_darwin");
}

/// A probed prologue keeps the frame pointer setup first, builds the size in `x9`, probes, then moves `sp`.
#[test]
fn probed_prologue_is_in_order() {
    let mut body: Vec<String> = (0..600).map(|i| format!("v{i} := {i}")).collect();
    body.extend((0..600).map(|i| format!("print(v{i})")));
    let text = function_text(&asm(&main_with(&body)), "_em_main");
    let lines = trimmed_lines(&text);
    assert_eq!(
        lines[..3],
        ["stp x29, x30, [sp, #-16]!", "mov x29, sp", lines[2]]
    );
    assert!(
        lines[2].starts_with("movz x9, #"),
        "third line is `{}`",
        lines[2]
    );
    let probe = lines.iter().position(|line| *line == "bl ___chkstk_darwin");
    assert!(probe.is_some(), "no probe in:\n{text}");
    let probe = probe.unwrap();
    // Between the `movz` and the probe there are only `movk x9` chunks of the size.
    assert!(
        lines[3..probe]
            .iter()
            .all(|line| line.starts_with("movk x9, #"))
    );
    assert_eq!(lines[probe + 1], "sub sp, sp, x9");
}

/// A frame of exactly 4 MiB is accepted; its size `0x400000` is `movz x9, #0` and `movk x9, #64, lsl #16`.
#[test]
fn frame_of_4_mib_is_accepted() {
    let text = emit_ok(&module_with_slots(524_288), FILE);
    assert_run(
        &text,
        &[
            "movz x9, #0",
            "movk x9, #64, lsl #16",
            "bl ___chkstk_darwin",
            "sub sp, sp, x9",
        ],
    );
}

/// Asserts that emitting `module` fails with exactly `message` at `span`.
fn assert_frame_error(module: &Module, message: &str, span: Span) {
    let error = arm64::emit(module, FILE).unwrap_err();
    assert_eq!(error.message, message);
    assert_eq!(error.span, span);
}

/// A frame of 4,194,320 bytes (524,289 slots) is one rounding step over the limit: an error with the exact text.
#[test]
fn frame_over_4_mib_is_an_error_with_the_exact_message() {
    let module = module_with_slots(524_289);
    assert_frame_error(&module, &frame_error("main", 4_194_320), Span::new(0, 0));
}

/// The frame error is an ordinary error, not an internal compiler error.
#[test]
fn frame_over_4_mib_is_not_an_internal_error() {
    let error = arm64::emit(&module_with_slots(524_289), FILE).unwrap_err();
    assert!(!error.message.starts_with("internal compiler error"));
}

/// The frame error points at the function's span and names the function.
#[test]
fn frame_error_uses_the_function_span_and_name() {
    let mut module = module_with_slots(524_289);
    module.functions[0].name = "huge".to_string();
    module.functions[0].span = Span::new(5, 9);
    assert_frame_error(&module, &frame_error("huge", 4_194_320), Span::new(5, 9));
}

/// The first oversized function is the one reported, even when another function precedes it.
#[test]
fn frame_error_names_the_oversized_function_not_the_first() {
    let mut module = module_with_slots(524_289);
    let mut small = module_of(vec![], Terminator::Return(None));
    small.functions[0].name = "small".to_string();
    module.functions.insert(0, small.functions.remove(0));
    assert_frame_error(&module, &frame_error("main", 4_194_320), Span::new(0, 0));
}

/// The outgoing area counts toward the limit: 4 MiB of cells plus a 16-byte area is over.
#[test]
fn outgoing_area_counts_toward_the_frame_limit() {
    // 524,287 temps are 4,194,296 bytes, rounded to 4,194,304 (allowed alone); the 16-byte area makes 4,194,320.
    let mut insts: Vec<Inst> = (0..524_287).map(|n| constant(n, 0)).collect();
    insts.push(Inst::Call {
        dst: None,
        func: "main".to_string(),
        args: (0..9).map(Temp).collect(),
    });
    let module = module_of(insts, Terminator::Return(None));
    assert_frame_error(&module, &frame_error("main", 4_194_320), Span::new(0, 0));
}

/// The caller stores arguments 9 and 10 at `[sp, #0]` and `[sp, #8]`, before it loads the register arguments.
#[test]
fn caller_stores_stack_arguments_before_loading_registers() {
    let text = function_text(&asm(&ten_parameter_program()), "_em_main");
    // The ten literals are temps 0..9 and the outgoing area is 16 bytes, so temp `n` is at `16 + 8n`.
    assert_run(
        &text,
        &[
            "ldr x8, [sp, #80]",
            "str x8, [sp, #0]",
            "ldr x8, [sp, #88]",
            "str x8, [sp, #8]",
            "ldr x0, [sp, #16]",
        ],
    );
}

/// The last register argument is loaded right before the call.
#[test]
fn caller_loads_the_eighth_register_argument_right_before_the_call() {
    let text = function_text(&asm(&ten_parameter_program()), "_em_main");
    assert_run(&text, &["ldr x7, [sp, #72]", "bl _em_f"]);
}

/// The callee reads its ninth parameter from `[x29, #16]` and copies it into slot 8 through cell 10.
#[test]
fn callee_reads_the_ninth_parameter_above_the_frame_pointer() {
    let text = function_text(&asm(&ten_parameter_program()), "_em_f");
    // 10 slots, then the isel registers for parameters 8 and 9 in cells 10 and 11; no outgoing area.
    assert_run(
        &text,
        &[
            "ldr x8, [x29, #16]",
            "str x8, [sp, #80]",
            "ldr x8, [sp, #80]",
            "str x8, [sp, #64]",
        ],
    );
}

/// The callee reads its tenth parameter from `[x29, #24]` and copies it into slot 9 through cell 11.
#[test]
fn callee_reads_the_tenth_parameter_above_the_frame_pointer() {
    let text = function_text(&asm(&ten_parameter_program()), "_em_f");
    assert_run(
        &text,
        &[
            "ldr x8, [x29, #24]",
            "str x8, [sp, #88]",
            "ldr x8, [sp, #88]",
            "str x8, [sp, #72]",
        ],
    );
}

/// The register parameters are still stored straight from `x0`..`x7`, before the stack ones are read.
#[test]
fn callee_stores_register_parameters_before_reading_stack_ones() {
    let text = function_text(&asm(&ten_parameter_program()), "_em_f");
    assert_run(&text, &["str x7, [sp, #56]", "ldr x8, [x29, #16]"]);
}

/// A function with 4,103 parameters: parameter 4,101 is `[x29, #32760]`, the last immediate offset.
#[test]
fn incoming_argument_at_offset_32760_is_an_immediate() {
    let text = emit_ok(&module_with_parameters(4_103), FILE);
    assert_contains(&text, "ldr x8, [x29, #32760]");
}

/// Parameter 4,102 is at `[x29, #32768]`, which needs `x17`.
#[test]
fn incoming_argument_at_offset_32768_goes_through_x17() {
    let text = emit_ok(&module_with_parameters(4_103), FILE);
    assert_run(&text, &["movz x17, #32768", "ldr x8, [x29, x17]"]);
}

/// A call with 4,105 arguments: argument 4,103 is stored at `[sp, #32760]`, the last immediate offset.
#[test]
fn outgoing_argument_at_offset_32760_is_an_immediate() {
    let text = emit_ok(&call_with_constants(4_105), FILE);
    assert_contains(&text, "str x8, [sp, #32760]");
}

/// Argument 4,104 of that call is stored at `[sp, #32768]`, which needs `x17`.
#[test]
fn outgoing_argument_at_offset_32768_goes_through_x17() {
    let text = emit_ok(&call_with_constants(4_105), FILE);
    assert_run(&text, &["movz x17, #32768", "str x8, [sp, x17]"]);
}

/// A function with no call of more than 8 arguments has the same cell offsets as Phase 1: slot 0 at `[sp, #0]`.
#[test]
fn functions_without_stack_arguments_keep_their_cell_offsets() {
    let text = function_text(&asm(&fib_program()), "_em_fib");
    assert_contains(&text, "str x0, [sp, #0]");
    assert_lacks(&text, "x29, #");
}

/// Emitting a module with stack arguments twice gives the same text.
#[test]
fn stack_argument_output_is_deterministic() {
    let module = call_with_constants(12);
    assert_eq!(emit_ok(&module, FILE), emit_ok(&module, FILE));
}
