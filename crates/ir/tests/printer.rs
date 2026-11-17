//! Integration tests for `print_module` on hand-built modules.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use ir::{
    BinOp, Block, BlockId, Function, Inst, Loc, Module, PrintKind, Slot, SlotId, Temp, Terminator,
    print_module,
};

/// Joins lines into one text that ends with a newline.
fn text(lines: &[&str]) -> String {
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

/// Builds a temp.
fn t(number: u32) -> Temp {
    Temp(number)
}

/// Builds a location.
fn loc(line: u32, col: u32) -> Loc {
    Loc { line, col }
}

/// Builds a block whose id is its position.
fn block(id: u32, insts: Vec<Inst>, term: Terminator) -> Block {
    Block {
        id: BlockId(id),
        insts,
        term,
    }
}

/// Builds a function from parameter slot numbers, slot names and blocks.
fn function(
    name: &str,
    params: &[u32],
    returns_value: bool,
    slots: &[&str],
    blocks: Vec<Block>,
) -> Function {
    Function {
        name: name.to_string(),
        params: params.iter().map(|&number| SlotId(number)).collect(),
        returns_value,
        slots: slots
            .iter()
            .map(|name| Slot {
                name: name.to_string(),
            })
            .collect(),
        blocks,
    }
}

/// Prints a module holding one function.
fn print_function(function: Function) -> String {
    print_module(&Module {
        functions: vec![function],
    })
}

/// Prints `fn main` whose only block holds the given instructions and ends with `ret`.
fn print_insts(slots: &[&str], insts: Vec<Inst>) -> String {
    let entry = block(0, insts, Terminator::Return(None));
    print_function(function("main", &[], false, slots, vec![entry]))
}

/// Prints `fn main` whose only block has no instructions and the given terminator.
fn print_terminator(terminator: Terminator) -> String {
    let entry = block(0, vec![], terminator);
    print_function(function("main", &[], false, &[], vec![entry]))
}

/// Builds the expected text of an empty-slots `fn main` with one block of these lines.
fn main_text(block_lines: &[&str]) -> String {
    let mut lines = vec!["fn main() {", "  slots:", "bb0:"];
    lines.extend_from_slice(block_lines);
    lines.push("}");
    text(&lines)
}

/// Builds a binary instruction `t2 = OP t0, t1` with a location.
fn binary(op: BinOp) -> Inst {
    Inst::Binary {
        dst: t(2),
        op,
        lhs: t(0),
        rhs: t(1),
        loc: loc(3, 7),
    }
}

/// A function without parameters or a value prints `fn name() {`.
#[test]
fn header_without_params_or_value() {
    let output = print_function(function(
        "main",
        &[],
        false,
        &[],
        vec![block(0, vec![], Terminator::Return(None))],
    ));
    assert_eq!(output.lines().next().unwrap(), "fn main() {");
}

/// Parameters print by slot name, and ` -> i64` follows when the function returns a value.
#[test]
fn header_with_params_and_value() {
    let output = print_function(function(
        "f",
        &[0, 1],
        true,
        &["a", "b", "c"],
        vec![block(0, vec![], Terminator::Return(None))],
    ));
    let expected = text(&[
        "fn f(a, b) -> i64 {",
        "  slots: a, b, c",
        "bb0:",
        "  ret",
        "}",
    ]);
    assert_eq!(output, expected);
}

/// With no slots the line is `  slots:` with nothing after the colon.
#[test]
fn empty_slots_line_has_nothing_after_the_colon() {
    let output = print_insts(&[], vec![]);
    assert_eq!(output.lines().nth(1).unwrap(), "  slots:");
}

/// Slots print comma-separated, in slot order.
#[test]
fn slots_print_comma_separated() {
    let output = print_insts(&["a", "b", "$end"], vec![]);
    assert_eq!(output.lines().nth(1).unwrap(), "  slots: a, b, $end");
}

/// The second slot with a name gets `#k` where k is its slot number, in slots, loads and stores.
#[test]
fn duplicate_slot_names_get_their_slot_number() {
    let insts = vec![
        Inst::Store {
            slot: SlotId(1),
            src: t(0),
        },
        Inst::Store {
            slot: SlotId(3),
            src: t(0),
        },
        Inst::Load {
            dst: t(1),
            slot: SlotId(3),
        },
    ];
    let output = print_insts(&["a", "$end", "b", "$end"], insts);
    let expected = text(&[
        "fn main() {",
        "  slots: a, $end, b, $end#3",
        "bb0:",
        "  store $end, t0",
        "  store $end#3, t0",
        "  t1 = load $end#3",
        "  ret",
        "}",
    ]);
    assert_eq!(output, expected);
}

/// A name used three times prints plain, then `#1`, then `#2`.
#[test]
fn a_name_used_three_times_numbers_the_later_slots() {
    let output = print_insts(&["x", "x", "x"], vec![]);
    assert_eq!(output.lines().nth(1).unwrap(), "  slots: x, x#1, x#2");
}

/// `const` prints its value, including negative values.
#[test]
fn const_prints_its_value() {
    let insts = vec![
        Inst::Const {
            dst: t(0),
            value: 42,
        },
        Inst::Const {
            dst: t(1),
            value: i64::MIN,
        },
    ];
    let expected = main_text(&[
        "  t0 = const 42",
        "  t1 = const -9223372036854775808",
        "  ret",
    ]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// `load` and `store` print the slot name.
#[test]
fn load_and_store_print_the_slot_name() {
    let insts = vec![
        Inst::Load {
            dst: t(0),
            slot: SlotId(0),
        },
        Inst::Store {
            slot: SlotId(0),
            src: t(0),
        },
    ];
    let expected = text(&[
        "fn main() {",
        "  slots: x",
        "bb0:",
        "  t0 = load x",
        "  store x, t0",
        "  ret",
        "}",
    ]);
    assert_eq!(print_insts(&["x"], insts), expected);
}

/// `add sub mul div rem` print their mnemonic and the `@L:C` location.
#[test]
fn checked_operators_print_a_location() {
    let table = [
        (BinOp::Add, "add"),
        (BinOp::Sub, "sub"),
        (BinOp::Mul, "mul"),
        (BinOp::Div, "div"),
        (BinOp::Rem, "rem"),
    ];
    for (op, mnemonic) in table {
        let line = format!("  t2 = {mnemonic} t0, t1 @3:7");
        let expected = main_text(&[&line, "  ret"]);
        assert_eq!(print_insts(&[], vec![binary(op)]), expected, "{mnemonic}");
    }
}

/// Comparisons print their mnemonic and no location, even though the instruction holds one.
#[test]
fn comparison_operators_print_no_location() {
    let table = [
        (BinOp::Eq, "eq"),
        (BinOp::Ne, "ne"),
        (BinOp::Lt, "lt"),
        (BinOp::Le, "le"),
        (BinOp::Gt, "gt"),
        (BinOp::Ge, "ge"),
    ];
    for (op, mnemonic) in table {
        let line = format!("  t2 = {mnemonic} t0, t1");
        let expected = main_text(&[&line, "  ret"]);
        assert_eq!(print_insts(&[], vec![binary(op)]), expected, "{mnemonic}");
    }
}

/// `neg` prints its location.
#[test]
fn neg_prints_a_location() {
    let insts = vec![Inst::Neg {
        dst: t(1),
        src: t(0),
        loc: loc(12, 34),
    }];
    let expected = main_text(&["  t1 = neg t0 @12:34", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// `not` prints no location.
#[test]
fn not_prints_no_location() {
    let insts = vec![Inst::Not {
        dst: t(1),
        src: t(0),
    }];
    let expected = main_text(&["  t1 = not t0", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// A call with a result and several arguments prints them comma-separated.
#[test]
fn call_with_result_and_arguments() {
    let insts = vec![Inst::Call {
        dst: Some(t(3)),
        func: "fib".to_string(),
        args: vec![t(0), t(1), t(2)],
    }];
    let expected = main_text(&["  t3 = call fib(t0, t1, t2)", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// A call with one argument prints no separator.
#[test]
fn call_with_one_argument() {
    let insts = vec![Inst::Call {
        dst: Some(t(1)),
        func: "f".to_string(),
        args: vec![t(0)],
    }];
    let expected = main_text(&["  t1 = call f(t0)", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// A call with no arguments prints empty parentheses.
#[test]
fn call_with_no_arguments() {
    let insts = vec![Inst::Call {
        dst: Some(t(0)),
        func: "zero".to_string(),
        args: vec![],
    }];
    let expected = main_text(&["  t0 = call zero()", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// A call without a result prints no `tD =`.
#[test]
fn call_without_result_has_no_destination() {
    let insts = vec![Inst::Call {
        dst: None,
        func: "say".to_string(),
        args: vec![t(0)],
    }];
    let expected = main_text(&["  call say(t0)", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// Print instructions spell their kind: `print.i64` and `print.bool`.
#[test]
fn print_instructions_spell_their_kind() {
    let insts = vec![
        Inst::Print {
            src: t(0),
            kind: PrintKind::I64,
        },
        Inst::Print {
            src: t(1),
            kind: PrintKind::Bool,
        },
    ];
    let expected = main_text(&["  print.i64 t0", "  print.bool t1", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// `jump bbN` prints the target block.
#[test]
fn jump_terminator() {
    let expected = main_text(&["  jump bb7"]);
    assert_eq!(print_terminator(Terminator::Jump(BlockId(7))), expected);
}

/// `br tC, bbT, bbE` prints the condition and both targets.
#[test]
fn branch_terminator() {
    let terminator = Terminator::Branch {
        cond: t(4),
        then_to: BlockId(1),
        else_to: BlockId(2),
    };
    let expected = main_text(&["  br t4, bb1, bb2"]);
    assert_eq!(print_terminator(terminator), expected);
}

/// `ret` prints bare when there is no value.
#[test]
fn return_without_value() {
    let expected = main_text(&["  ret"]);
    assert_eq!(print_terminator(Terminator::Return(None)), expected);
}

/// `ret tV` prints the value temp.
#[test]
fn return_with_value() {
    let expected = main_text(&["  ret t5"]);
    assert_eq!(print_terminator(Terminator::Return(Some(t(5)))), expected);
}

/// `unreachable` prints bare.
#[test]
fn unreachable_terminator() {
    let expected = main_text(&["  unreachable"]);
    assert_eq!(print_terminator(Terminator::Unreachable), expected);
}

/// Block labels start at column 0 and their lines are indented two spaces, in block order.
#[test]
fn blocks_print_in_order_with_labels_at_column_zero() {
    let blocks = vec![
        block(
            0,
            vec![Inst::Const {
                dst: t(0),
                value: 1,
            }],
            Terminator::Jump(BlockId(1)),
        ),
        block(
            1,
            vec![Inst::Print {
                src: t(0),
                kind: PrintKind::I64,
            }],
            Terminator::Return(None),
        ),
    ];
    let output = print_function(function("main", &[], false, &[], blocks));
    let expected = text(&[
        "fn main() {",
        "  slots:",
        "bb0:",
        "  t0 = const 1",
        "  jump bb1",
        "bb1:",
        "  print.i64 t0",
        "  ret",
        "}",
    ]);
    assert_eq!(output, expected);
}

/// Functions are separated by one blank line and the output ends with `}\n`.
#[test]
fn functions_are_separated_by_a_blank_line() {
    let entry = || block(0, vec![], Terminator::Return(None));
    let module = Module {
        functions: vec![
            function("a", &[], false, &[], vec![entry()]),
            function("main", &[], false, &[], vec![entry()]),
        ],
    };
    let expected = text(&[
        "fn a() {",
        "  slots:",
        "bb0:",
        "  ret",
        "}",
        "",
        "fn main() {",
        "  slots:",
        "bb0:",
        "  ret",
        "}",
    ]);
    assert_eq!(print_module(&module), expected);
}

/// A single function's output ends with `}` and one newline, with no blank line after.
#[test]
fn single_function_ends_with_brace_and_newline() {
    let output = print_insts(&[], vec![]);
    assert!(output.ends_with("}\n"));
    assert!(!output.ends_with("\n\n"));
}

/// Builds a one-block `main` with the given slot names, whose block loads the last slot.
fn print_with_slots(names: &[String]) -> String {
    let slot_names: Vec<&str> = names.iter().map(String::as_str).collect();
    let last = SlotId(u32::try_from(names.len() - 1).unwrap());
    let insts = vec![Inst::Load {
        dst: t(0),
        slot: last,
    }];
    print_insts(&slot_names, insts)
}

/// Printing 3,000 distinct slots is fast, and one repeated name still gets `#k`.
#[test]
fn printing_thousands_of_slots_is_fast_and_numbers_duplicates() {
    let mut names: Vec<String> = (0..3000).map(|number| format!("s{number}")).collect();
    names.push("s5".to_string());

    let started = std::time::Instant::now();
    let output = print_with_slots(&names);
    let elapsed = started.elapsed();

    let mut expected_names = names.clone();
    expected_names[3000] = "s5#3000".to_string();
    let expected_slots_line = format!("  slots: {}", expected_names.join(", "));
    assert_eq!(output.lines().nth(1).unwrap(), expected_slots_line);
    assert!(output.contains("  t0 = load s5#3000\n"));
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "printing took {elapsed:?}"
    );
}

/// Printing 3,000 slots that share one name is fast and numbers all but the first.
#[test]
fn printing_thousands_of_equal_names_is_fast() {
    let names = vec!["x".to_string(); 3000];

    let started = std::time::Instant::now();
    let output = print_with_slots(&names);
    let elapsed = started.elapsed();

    let slots_line = output.lines().nth(1).unwrap();
    assert!(slots_line.starts_with("  slots: x, x#1, x#2, x#3,"));
    assert!(slots_line.ends_with(", x#2999"));
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "printing took {elapsed:?}"
    );
}
