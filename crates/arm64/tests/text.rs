//! Text-level tests for the `arm64` crate: what `emit` puts in the assembly, without running it.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{FILE, asm, asm_named, emit_ok, main_with, program};
use ir::{Block, BlockId, Function, Inst, Module, SlotId, Temp, Terminator};
use source::Span;

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

/// A frame above 4095 bytes is reserved through `x16`.
#[test]
fn large_frame_is_built_in_x16() {
    let mut body: Vec<String> = (0..600).map(|i| format!("v{i} := {i}")).collect();
    body.extend((0..600).map(|i| format!("print(v{i})")));
    let text = asm(&main_with(&body));
    assert_contains(&text, "sub sp, sp, x16");
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
    assert_contains(&text, "mul x10, x8, x9");
    assert_contains(&text, "smulh x11, x8, x9");
}

/// Division uses `sdiv`.
#[test]
fn div_uses_sdiv() {
    assert_contains(
        &asm_of_main(&["a := 1", "print(a / a)"]),
        "sdiv x10, x8, x9",
    );
}

/// Remainder uses `msub` after `sdiv`.
#[test]
fn rem_uses_msub() {
    let text = asm_of_main(&["a := 1", "print(a % a)"]);
    assert_contains(&text, "sdiv x10, x8, x9");
    assert_contains(&text, "msub x10, x10, x9, x8");
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
    Module {
        functions: vec![Function {
            name: "main".to_string(),
            params: vec![],
            returns_value: false,
            slots: vec![],
            blocks: vec![Block {
                id: BlockId(0),
                insts,
                term,
            }],
        }],
    }
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

/// A call with nine arguments is an internal error: arguments travel in `x0` to `x7`.
#[test]
fn call_with_nine_arguments_is_an_internal_error() {
    let mut insts: Vec<Inst> = (0..9)
        .map(|n| Inst::Const {
            dst: Temp(n),
            value: i64::from(n),
        })
        .collect();
    insts.push(Inst::Call {
        dst: None,
        func: "main".to_string(),
        args: (0..9).map(Temp).collect(),
    });
    assert_internal_error(&module_of(insts, Terminator::Return(None)));
}

/// A function with nine parameters is an internal error.
#[test]
fn function_with_nine_parameters_is_an_internal_error() {
    let mut module = module_of(vec![], Terminator::Return(None));
    module.functions[0].params = (0..9).map(SlotId).collect();
    module.functions[0].slots = (0..9)
        .map(|n| ir::Slot {
            name: format!("p{n}"),
        })
        .collect();
    assert_internal_error(&module);
}

/// A load from a slot beyond the function's slots is an internal error.
#[test]
fn load_from_an_unknown_slot_is_an_internal_error() {
    let insts = vec![Inst::Load {
        dst: Temp(0),
        slot: SlotId(5),
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
