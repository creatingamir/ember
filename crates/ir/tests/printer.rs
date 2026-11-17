//! Integration tests for `print_module` on hand-built modules.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use ir::{
    BinOp, Block, BlockId, Function, Inst, IrType, Loc, Module, Param, PrintKind, Sign, Slot,
    SlotId, Temp, Terminator, print_module,
};
use source::Span;

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

/// How many temps the default helpers give a type to; no test names a higher one.
const LISTED_TEMPS: usize = 64;

/// Builds a function with every parameter, slot and temp of type `i64`, from parameter slot numbers, slot names and blocks.
fn function(
    name: &str,
    params: &[u32],
    returns_value: bool,
    slots: &[&str],
    blocks: Vec<Block>,
) -> Function {
    Function {
        name: name.to_string(),
        params: params
            .iter()
            .map(|&number| Param {
                slot: SlotId(number),
                ty: IrType::I64,
                sign: Sign::Signed,
            })
            .collect(),
        ret: returns_value.then_some(IrType::I64),
        slots: slots.iter().map(|name| slot(name, 8)).collect(),
        temp_types: vec![IrType::I64; LISTED_TEMPS],
        blocks,
        span: Span::new(0, 0),
    }
}

/// Builds a slot of `size` bytes, aligned to its size.
fn slot(name: &str, size: u32) -> Slot {
    Slot {
        name: name.to_string(),
        size,
        align: size,
    }
}

/// Builds a `main` with the given parameters, slots and temp types whose only block holds `insts` and returns.
fn typed_main(
    params: Vec<Param>,
    slots: Vec<Slot>,
    temp_types: Vec<IrType>,
    insts: Vec<Inst>,
) -> Function {
    Function {
        name: "main".to_string(),
        params,
        ret: None,
        slots,
        temp_types,
        blocks: vec![block(0, insts, Terminator::Return(None))],
        span: Span::new(0, 0),
    }
}

/// Prints `typed_main` with the given parts.
fn print_typed(
    params: Vec<Param>,
    slots: Vec<Slot>,
    temp_types: Vec<IrType>,
    insts: Vec<Inst>,
) -> String {
    print_function(typed_main(params, slots, temp_types, insts))
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

/// Builds a binary instruction `t2 = OP t0, t1` with the location 3:7.
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
        "fn f(a:i64, b:i64) -> i64 {",
        "  slots: a:8, b:8, c:8",
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
    assert_eq!(output.lines().nth(1).unwrap(), "  slots: a:8, b:8, $end:8");
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
            sign: Sign::Signed,
        },
    ];
    let output = print_insts(&["a", "$end", "b", "$end"], insts);
    let expected = text(&[
        "fn main() {",
        "  slots: a:8, $end:8, b:8, $end#3:8",
        "bb0:",
        "  store $end, t0",
        "  store $end#3, t0",
        "  t1:i64 = load $end#3",
        "  ret",
        "}",
    ]);
    assert_eq!(output, expected);
}

/// A name used three times prints plain, then `#1`, then `#2`.
#[test]
fn a_name_used_three_times_numbers_the_later_slots() {
    let output = print_insts(&["x", "x", "x"], vec![]);
    assert_eq!(output.lines().nth(1).unwrap(), "  slots: x:8, x#1:8, x#2:8");
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
        "  t0:i64 = const 42",
        "  t1:i64 = const -9223372036854775808",
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
            sign: Sign::Signed,
        },
        Inst::Store {
            slot: SlotId(0),
            src: t(0),
        },
    ];
    let expected = text(&[
        "fn main() {",
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = load x",
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
        (BinOp::Add(Sign::Signed), "add.s"),
        (BinOp::Sub(Sign::Signed), "sub.s"),
        (BinOp::Mul(Sign::Signed), "mul.s"),
        (BinOp::Div(Sign::Signed), "div.s"),
        (BinOp::Rem(Sign::Signed), "rem.s"),
    ];
    for (op, mnemonic) in table {
        let line = format!("  t2:i64 = {mnemonic} t0, t1 @3:7");
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
        (BinOp::Lt(Sign::Signed), "lt.s"),
        (BinOp::Le(Sign::Signed), "le.s"),
        (BinOp::Gt(Sign::Signed), "gt.s"),
        (BinOp::Ge(Sign::Signed), "ge.s"),
    ];
    for (op, mnemonic) in table {
        let line = format!("  t2:i8 = {mnemonic} t0, t1");
        let expected = main_text(&[&line, "  ret"]);
        let types = vec![IrType::I64, IrType::I64, IrType::I8];
        let output = print_typed(vec![], vec![], types, vec![binary(op)]);
        assert_eq!(output, expected, "{mnemonic}");
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
    let expected = main_text(&["  t1:i64 = neg t0 @12:34", "  ret"]);
    assert_eq!(print_insts(&[], insts), expected);
}

/// `not` prints no location.
#[test]
fn not_prints_no_location() {
    let insts = vec![Inst::Not {
        dst: t(1),
        src: t(0),
    }];
    let expected = main_text(&["  t1:i8 = not t0", "  ret"]);
    let output = print_typed(vec![], vec![], vec![IrType::I8, IrType::I8], insts);
    assert_eq!(output, expected);
}

/// A call with a result and several arguments prints them comma-separated.
#[test]
fn call_with_result_and_arguments() {
    let insts = vec![Inst::Call {
        dst: Some(t(3)),
        func: "fib".to_string(),
        args: vec![t(0), t(1), t(2)],
    }];
    let expected = main_text(&["  t3:i64 = call fib(t0, t1, t2)", "  ret"]);
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
    let expected = main_text(&["  t1:i64 = call f(t0)", "  ret"]);
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
    let expected = main_text(&["  t0:i64 = call zero()", "  ret"]);
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
        "  t0:i64 = const 1",
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
        sign: Sign::Signed,
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
    let labels: Vec<String> = expected_names
        .iter()
        .map(|name| format!("{name}:8"))
        .collect();
    let expected_slots_line = format!("  slots: {}", labels.join(", "));
    assert_eq!(output.lines().nth(1).unwrap(), expected_slots_line);
    assert!(output.contains("  t0:i64 = load s5#3000\n"));
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
    assert!(slots_line.starts_with("  slots: x:8, x#1:8, x#2:8, x#3:8,"));
    assert!(slots_line.ends_with(", x#2999:8"));
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "printing took {elapsed:?}"
    );
}

/// A temp with no entry in `temp_types` prints its type as `?`.
#[test]
fn temp_without_a_type_prints_a_question_mark() {
    let insts = vec![Inst::Const {
        dst: t(5),
        value: 1,
    }];
    let output = print_typed(vec![], vec![], vec![IrType::I64; 5], insts);
    assert_eq!(output, main_text(&["  t5:? = const 1", "  ret"]));
}

/// A temp with an entry prints that entry, not a default.
#[test]
fn temp_types_come_from_the_table() {
    let insts = vec![
        Inst::Const {
            dst: t(0),
            value: 1,
        },
        Inst::Const {
            dst: t(1),
            value: 2,
        },
        Inst::Const {
            dst: t(2),
            value: 3,
        },
        Inst::Const {
            dst: t(3),
            value: 4,
        },
    ];
    let types = vec![IrType::I8, IrType::I16, IrType::I32, IrType::I64];
    let expected = main_text(&[
        "  t0:i8 = const 1",
        "  t1:i16 = const 2",
        "  t2:i32 = const 3",
        "  t3:i64 = const 4",
        "  ret",
    ]);
    assert_eq!(print_typed(vec![], vec![], types, insts), expected);
}

/// A load into an `i16` with a signed load prints `load.s`.
#[test]
fn signed_load_of_an_i16_prints_load_s() {
    let insts = vec![Inst::Load {
        dst: t(0),
        slot: SlotId(0),
        sign: Sign::Signed,
    }];
    let output = print_typed(vec![], vec![slot("small", 2)], vec![IrType::I16], insts);
    let expected = text(&[
        "fn main() {",
        "  slots: small:2",
        "bb0:",
        "  t0:i16 = load.s small",
        "  ret",
        "}",
    ]);
    assert_eq!(output, expected);
}

/// A load into an `i8` with an unsigned load prints `load.u`.
#[test]
fn unsigned_load_of_an_i8_prints_load_u() {
    let insts = vec![Inst::Load {
        dst: t(0),
        slot: SlotId(0),
        sign: Sign::Unsigned,
    }];
    let output = print_typed(vec![], vec![slot("flag", 1)], vec![IrType::I8], insts);
    assert!(output.contains("  t0:i8 = load.u flag\n"), "{output}");
}

/// A signed load of an `i8` prints `load.s`.
#[test]
fn signed_load_of_an_i8_prints_load_s() {
    let insts = vec![Inst::Load {
        dst: t(0),
        slot: SlotId(0),
        sign: Sign::Signed,
    }];
    let output = print_typed(vec![], vec![slot("byte", 1)], vec![IrType::I8], insts);
    assert!(output.contains("  t0:i8 = load.s byte\n"), "{output}");
}

/// An unsigned load of an `i16` prints `load.u`.
#[test]
fn unsigned_load_of_an_i16_prints_load_u() {
    let insts = vec![Inst::Load {
        dst: t(0),
        slot: SlotId(0),
        sign: Sign::Unsigned,
    }];
    let output = print_typed(vec![], vec![slot("half", 2)], vec![IrType::I16], insts);
    assert!(output.contains("  t0:i16 = load.u half\n"), "{output}");
}

/// Loads of `i32` and `i64` print no sign, whatever the instruction holds.
#[test]
fn wide_loads_print_no_sign() {
    for (ty, size, sign) in [
        (IrType::I32, 4, Sign::Signed),
        (IrType::I32, 4, Sign::Unsigned),
        (IrType::I64, 8, Sign::Signed),
        (IrType::I64, 8, Sign::Unsigned),
    ] {
        let insts = vec![Inst::Load {
            dst: t(0),
            slot: SlotId(0),
            sign,
        }];
        let output = print_typed(vec![], vec![slot("w", size)], vec![ty], insts);
        let expected = format!("  t0:{ty} = load w\n");
        assert!(output.contains(&expected), "{output}");
    }
}

/// A constant of `-1` into an `i64` prints `const -1`.
#[test]
fn const_minus_one_prints_signed() {
    let insts = vec![Inst::Const {
        dst: t(0),
        value: -1,
    }];
    let output = print_insts(&[], insts);
    assert_eq!(output, main_text(&["  t0:i64 = const -1", "  ret"]));
}

/// A `u64` constant above `i64::MAX` is held as its two's-complement `i64` and prints negative.
#[test]
fn const_with_the_top_bit_set_prints_as_a_signed_number() {
    let insts = vec![Inst::Const {
        dst: t(0),
        value: u64::MAX as i64,
    }];
    let output = print_insts(&[], insts);
    assert_eq!(output, main_text(&["  t0:i64 = const -1", "  ret"]));
}

/// A parameter of `i8` with `Sign::Unsigned` prints `p:i8.u`, and the slot size is 1.
#[test]
fn unsigned_i8_parameter_prints_a_u_suffix() {
    let param = Param {
        slot: SlotId(0),
        ty: IrType::I8,
        sign: Sign::Unsigned,
    };
    let output = print_typed(vec![param], vec![slot("p", 1)], vec![], vec![]);
    let expected = text(&["fn main(p:i8.u) {", "  slots: p:1", "bb0:", "  ret", "}"]);
    assert_eq!(output, expected);
}

/// A signed `i16` parameter prints `.s`, an unsigned one `.u`.
#[test]
fn i16_parameters_print_their_sign() {
    let params = vec![
        Param {
            slot: SlotId(0),
            ty: IrType::I16,
            sign: Sign::Signed,
        },
        Param {
            slot: SlotId(1),
            ty: IrType::I16,
            sign: Sign::Unsigned,
        },
    ];
    let slots = vec![slot("a", 2), slot("b", 2)];
    let output = print_typed(params, slots, vec![], vec![]);
    assert_eq!(
        output.lines().next().unwrap(),
        "fn main(a:i16.s, b:i16.u) {"
    );
}

/// Parameters of `i32` and `i64` print no sign suffix.
#[test]
fn wide_parameters_print_no_sign() {
    let params = vec![
        Param {
            slot: SlotId(0),
            ty: IrType::I32,
            sign: Sign::Unsigned,
        },
        Param {
            slot: SlotId(1),
            ty: IrType::I64,
            sign: Sign::Unsigned,
        },
    ];
    let slots = vec![slot("a", 4), slot("b", 8)];
    let output = print_typed(params, slots, vec![], vec![]);
    assert_eq!(output.lines().next().unwrap(), "fn main(a:i32, b:i64) {");
}

/// A return type other than `i64` prints after the arrow: `-> i8`.
#[test]
fn return_type_is_printed_after_the_arrow() {
    for (ty, name) in [
        (IrType::I8, "i8"),
        (IrType::I16, "i16"),
        (IrType::I32, "i32"),
        (IrType::I64, "i64"),
    ] {
        let mut function = typed_main(vec![], vec![], vec![], vec![]);
        function.ret = Some(ty);
        let output = print_function(function);
        assert_eq!(
            output.lines().next().unwrap(),
            format!("fn main() -> {name} {{")
        );
    }
}

/// Slots print their size in bytes after a colon.
#[test]
fn slots_print_their_size() {
    let slots = vec![slot("a", 1), slot("b", 2), slot("c", 4), slot("d", 8)];
    let output = print_typed(vec![], slots, vec![], vec![]);
    assert_eq!(
        output.lines().nth(1).unwrap(),
        "  slots: a:1, b:2, c:4, d:8"
    );
}

/// The alignment is not printed.
#[test]
fn slots_do_not_print_their_alignment() {
    let wide_aligned = Slot {
        name: "a".to_string(),
        size: 4,
        align: 16,
    };
    let output = print_typed(vec![], vec![wide_aligned], vec![], vec![]);
    assert_eq!(output.lines().nth(1).unwrap(), "  slots: a:4");
}

/// Duplicate names get `#k` before the size: `x#1:2`.
#[test]
fn duplicate_slot_names_keep_the_size_after_the_number() {
    let output = print_typed(vec![], vec![slot("x", 8), slot("x", 2)], vec![], vec![]);
    assert_eq!(output.lines().nth(1).unwrap(), "  slots: x:8, x#1:2");
}

/// The unsigned operations print `.u`.
#[test]
fn unsigned_operators_print_a_u_suffix() {
    let table = [
        (BinOp::Add(Sign::Unsigned), "add.u"),
        (BinOp::Sub(Sign::Unsigned), "sub.u"),
        (BinOp::Mul(Sign::Unsigned), "mul.u"),
        (BinOp::Div(Sign::Unsigned), "div.u"),
        (BinOp::Rem(Sign::Unsigned), "rem.u"),
    ];
    for (op, mnemonic) in table {
        let line = format!("  t2:i32 = {mnemonic} t0, t1 @3:7");
        let types = vec![IrType::I32; 3];
        let output = print_typed(vec![], vec![], types, vec![binary(op)]);
        assert_eq!(output, main_text(&[&line, "  ret"]), "{mnemonic}");
    }
}

/// The unsigned comparisons print `.u`.
#[test]
fn unsigned_comparisons_print_a_u_suffix() {
    let table = [
        (BinOp::Lt(Sign::Unsigned), "lt.u"),
        (BinOp::Le(Sign::Unsigned), "le.u"),
        (BinOp::Gt(Sign::Unsigned), "gt.u"),
        (BinOp::Ge(Sign::Unsigned), "ge.u"),
    ];
    for (op, mnemonic) in table {
        let line = format!("  t2:i8 = {mnemonic} t0, t1");
        let types = vec![IrType::I32, IrType::I32, IrType::I8];
        let output = print_typed(vec![], vec![], types, vec![binary(op)]);
        assert_eq!(output, main_text(&[&line, "  ret"]), "{mnemonic}");
    }
}

/// The same operation with the two signs prints two different mnemonics.
#[test]
fn signed_and_unsigned_mnemonics_differ() {
    let signed = print_insts(&[], vec![binary(BinOp::Add(Sign::Signed))]);
    let unsigned = print_insts(&[], vec![binary(BinOp::Add(Sign::Unsigned))]);
    assert!(signed.contains("add.s"));
    assert!(unsigned.contains("add.u"));
}

/// A call whose result is narrow prints the result type before the `=`.
#[test]
fn call_result_prints_its_temp_type() {
    let insts = vec![Inst::Call {
        dst: Some(t(0)),
        func: "flag".to_string(),
        args: vec![],
    }];
    let output = print_typed(vec![], vec![], vec![IrType::I8], insts);
    assert_eq!(output, main_text(&["  t0:i8 = call flag()", "  ret"]));
}

/// A call without a result prints no type.
#[test]
fn call_without_a_result_prints_no_type() {
    let insts = vec![Inst::Call {
        dst: None,
        func: "say".to_string(),
        args: vec![t(0)],
    }];
    let output = print_typed(vec![], vec![], vec![IrType::I64], insts);
    assert_eq!(output, main_text(&["  call say(t0)", "  ret"]));
}

/// Stores print no types: `store x, t0`.
#[test]
fn store_prints_no_type() {
    let insts = vec![Inst::Store {
        slot: SlotId(0),
        src: t(0),
    }];
    let output = print_typed(vec![], vec![slot("x", 8)], vec![IrType::I64], insts);
    assert!(output.contains("\n  store x, t0\n"), "{output}");
}
