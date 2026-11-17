//! Integration tests for the `Module` values that `lower` builds.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use ir::{
    BinOp, BlockId, Function, Inst, IrType, Loc, Module, Param, PrintKind, Sign, SlotId, Temp,
    Terminator, lower,
};
use source::SourceFile;

/// Joins lines into one text that ends with a newline.
fn text(lines: &[&str]) -> String {
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

/// Parses, checks and lowers a program that must be valid.
fn module(source_lines: &[&str]) -> Module {
    let src = text(source_lines);
    let program = parser::parse_source(&src).unwrap();
    let checked = check::check(&program).unwrap();
    lower(&program, &checked, &SourceFile::new("t.em", &src))
        .expect("lowering a checked program must succeed")
}

/// Finds a function of the module by name.
fn function<'a>(module: &'a Module, name: &str) -> &'a Function {
    module
        .functions
        .iter()
        .find(|function| function.name == name)
        .unwrap()
}

/// Lowers a program and returns its `main` function.
fn main_function(source_lines: &[&str]) -> Function {
    function(&module(source_lines), "main").clone()
}

/// Returns the slot names of a function, in slot order.
fn slot_names(function: &Function) -> Vec<&str> {
    function
        .slots
        .iter()
        .map(|slot| slot.name.as_str())
        .collect()
}

/// Returns the blocks a terminator can continue to.
fn successors(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Jump(target) => vec![*target],
        Terminator::Branch {
            then_to, else_to, ..
        } => vec![*then_to, *else_to],
        Terminator::Return(_) | Terminator::Unreachable => vec![],
    }
}

/// Returns the ids of the blocks reachable from the entry block.
fn reachable_blocks(function: &Function) -> HashSet<BlockId> {
    let mut seen = HashSet::new();
    let mut pending = vec![BlockId(0)];
    while let Some(id) = pending.pop() {
        if seen.insert(id) {
            let block = &function.blocks[id.0 as usize];
            pending.extend(successors(&block.term));
        }
    }
    seen
}

/// A program whose lowering removes an unreachable block that has higher-numbered survivors.
fn program_with_removed_block() -> Vec<&'static str> {
    vec![
        "fn main() {",
        "    c := true",
        "    if c {",
        "        for {",
        "        }",
        "    }",
        "    if c {",
        "        print(1)",
        "    }",
        "}",
    ]
}

/// Functions appear in the module in source order.
#[test]
fn functions_keep_source_order() {
    let lowered = module(&["fn zeta() {", "}", "fn alpha() {", "}", "fn main() {", "}"]);
    let names: Vec<&str> = lowered
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    assert_eq!(names, vec!["zeta", "alpha", "main"]);
}

/// Parameters occupy the first slots, in order, ahead of locals.
#[test]
fn params_are_the_first_slots_in_order() {
    let lowered = module(&[
        "fn f(a: i64, b: i64) -> i64 {",
        "    x := a + b",
        "    return x",
        "}",
        "fn main() {",
        "}",
    ]);
    let f = function(&lowered, "f");
    let expected = vec![
        Param {
            slot: SlotId(0),
            ty: IrType::I64,
            sign: Sign::Signed,
        },
        Param {
            slot: SlotId(1),
            ty: IrType::I64,
            sign: Sign::Signed,
        },
    ];
    assert_eq!(f.params, expected);
    assert_eq!(slot_names(f), vec!["a", "b", "x"]);
}

/// A function without parameters has an empty `params` list.
#[test]
fn function_without_parameters_has_no_params() {
    let main = main_function(&["fn main() {", "}"]);
    assert!(main.params.is_empty());
    assert!(main.slots.is_empty());
}

/// `ret` is the IR type of the result: `i64` for `i64` and `i8` for `bool`.
#[test]
fn ret_is_the_return_type_for_value_returning_functions() {
    let lowered = module(&[
        "fn number() -> i64 {",
        "    return 1",
        "}",
        "fn flag() -> bool {",
        "    return true",
        "}",
        "fn main() {",
        "}",
    ]);
    assert_eq!(function(&lowered, "number").ret, Some(IrType::I64));
    assert_eq!(function(&lowered, "flag").ret, Some(IrType::I8));
}

/// `ret` is `None` when there is no return type.
#[test]
fn ret_is_none_without_a_return_type() {
    let main = main_function(&["fn main() {", "}"]);
    assert_eq!(main.ret, None);
}

/// Hidden slots are named `$end`, `$and` and `$or`, and duplicates are allowed.
#[test]
fn hidden_slots_use_their_reserved_names() {
    let main = main_function(&[
        "fn main() {",
        "    a := true",
        "    for i in 0..2 {",
        "        print(a && a)",
        "        print(a || a)",
        "    }",
        "    for j in 0..2 {",
        "    }",
        "}",
    ]);
    assert_eq!(
        slot_names(&main),
        vec!["a", "i", "$end", "$and", "$or", "j", "$end"]
    );
}

/// A `let` inside a block gets its slot in creation order, after earlier slots.
#[test]
fn locals_get_slots_in_creation_order() {
    let main = main_function(&[
        "fn main() {",
        "    a := 1",
        "    if a < 2 {",
        "        b := 2",
        "        print(b)",
        "    }",
        "    c := 3",
        "    print(c)",
        "}",
    ]);
    assert_eq!(slot_names(&main), vec!["a", "b", "c"]);
}

/// Every block's id equals its position, including after unreachable blocks are removed.
#[test]
fn block_ids_match_positions_after_removal() {
    let main = main_function(&program_with_removed_block());
    assert_eq!(main.blocks.len(), 6);
    for (position, block) in main.blocks.iter().enumerate() {
        assert_eq!(block.id, BlockId(position as u32));
    }
}

/// Every jump, branch target and the entry block refer to blocks that exist.
#[test]
fn terminator_targets_exist_after_removal() {
    let main = main_function(&program_with_removed_block());
    for block in &main.blocks {
        for target in successors(&block.term) {
            assert!(
                (target.0 as usize) < main.blocks.len(),
                "bb{} jumps to missing bb{}",
                block.id.0,
                target.0
            );
        }
    }
}

/// Every block left after lowering can be reached from the entry block.
#[test]
fn all_blocks_are_reachable_from_the_entry() {
    let main = main_function(&program_with_removed_block());
    assert_eq!(reachable_blocks(&main).len(), main.blocks.len());
}

/// Code after `return` leaves no unreachable blocks behind.
#[test]
fn code_after_return_leaves_no_unreachable_blocks() {
    let main = main_function(&[
        "fn main() {",
        "    return",
        "    print(1)",
        "    if true {",
        "        print(2)",
        "    }",
        "}",
    ]);
    assert_eq!(main.blocks.len(), 1);
    assert_eq!(main.blocks[0].term, Terminator::Return(None));
}

/// No `Unreachable` terminator remains in a program the checker accepted.
#[test]
fn accepted_programs_have_no_unreachable_terminators() {
    let lowered = module(&[
        "fn f(c: bool) -> i64 {",
        "    if c {",
        "        return 1",
        "    } else {",
        "        return 2",
        "    }",
        "}",
        "fn g() -> i64 {",
        "    for {",
        "        return 1",
        "    }",
        "}",
        "fn main() {",
        "}",
    ]);
    for function in &lowered.functions {
        for block in &function.blocks {
            assert_ne!(block.term, Terminator::Unreachable, "in {}", function.name);
        }
    }
}

/// Temps start again at `t0` in every function.
#[test]
fn temps_restart_at_zero_in_each_function() {
    let lowered = module(&[
        "fn a() {",
        "    print(1)",
        "}",
        "fn main() {",
        "    print(2)",
        "}",
    ]);
    for (name, value) in [("a", 1), ("main", 2)] {
        let first = &function(&lowered, name).blocks[0].insts[0];
        let expected = Inst::Const {
            dst: Temp(0),
            value,
        };
        assert_eq!(first, &expected, "in {name}");
    }
}

/// Each temp is assigned by exactly one instruction.
#[test]
fn every_temp_is_assigned_once() {
    let lowered = module(&[
        "fn fib(n: i64) -> i64 {",
        "    if n < 2 {",
        "        return n",
        "    }",
        "    return fib(n - 1) + fib(n - 2)",
        "}",
        "fn main() {",
        "    print(fib(10))",
        "}",
    ]);
    let fib = function(&lowered, "fib");
    let mut seen = HashSet::new();
    for block in &fib.blocks {
        for inst in &block.insts {
            let dst = match inst {
                Inst::Const { dst, .. }
                | Inst::Load { dst, .. }
                | Inst::Binary { dst, .. }
                | Inst::Neg { dst, .. }
                | Inst::Not { dst, .. } => Some(*dst),
                Inst::Call { dst, .. } => *dst,
                Inst::Store { .. } | Inst::Print { .. } => None,
            };
            if let Some(dst) = dst {
                assert!(seen.insert(dst), "{dst:?} assigned twice");
            }
        }
    }
    assert_eq!(seen.len(), 13);
}

/// Booleans lower to consts 1 and 0.
#[test]
fn bool_literals_lower_to_consts_one_and_zero() {
    let main = main_function(&["fn main() {", "    print(true)", "    print(false)", "}"]);
    let expected = vec![
        Inst::Const {
            dst: Temp(0),
            value: 1,
        },
        Inst::Print {
            src: Temp(0),
            kind: PrintKind::Bool,
        },
        Inst::Const {
            dst: Temp(1),
            value: 0,
        },
        Inst::Print {
            src: Temp(1),
            kind: PrintKind::Bool,
        },
    ];
    assert_eq!(main.blocks[0].insts, expected);
}

/// `-9223372036854775808` is one `Const` holding `i64::MIN`.
#[test]
fn min_literal_is_one_const_of_i64_min() {
    let main = main_function(&["fn main() {", "    print(-9223372036854775808)", "}"]);
    let expected = vec![
        Inst::Const {
            dst: Temp(0),
            value: i64::MIN,
        },
        Inst::Print {
            src: Temp(0),
            kind: PrintKind::I64,
        },
    ];
    assert_eq!(main.blocks[0].insts, expected);
}

/// A binary expression becomes a `Binary` with both operand temps and the start location.
#[test]
fn binary_instruction_has_operands_and_location() {
    let main = main_function(&["fn main() {", "    print(1 + 2)", "}"]);
    let expected = vec![
        Inst::Const {
            dst: Temp(0),
            value: 1,
        },
        Inst::Const {
            dst: Temp(1),
            value: 2,
        },
        Inst::Binary {
            dst: Temp(2),
            op: BinOp::Add(Sign::Signed),
            lhs: Temp(0),
            rhs: Temp(1),
            loc: Loc { line: 2, col: 11 },
        },
        Inst::Print {
            src: Temp(2),
            kind: PrintKind::I64,
        },
    ];
    assert_eq!(main.blocks[0].insts, expected);
}

/// Unary minus becomes a `Neg` with the location of the `-`.
#[test]
fn neg_instruction_has_the_expression_location() {
    let main = main_function(&["fn main() {", "    print(-5)", "}"]);
    let expected = Inst::Neg {
        dst: Temp(1),
        src: Temp(0),
        loc: Loc { line: 2, col: 11 },
    };
    assert_eq!(main.blocks[0].insts[1], expected);
}

/// `!` becomes a `Not` on the operand temp.
#[test]
fn not_instruction_has_the_operand_temp() {
    let main = main_function(&["fn main() {", "    print(!true)", "}"]);
    let expected = Inst::Not {
        dst: Temp(1),
        src: Temp(0),
    };
    assert_eq!(main.blocks[0].insts[1], expected);
}

/// `let` and a later read use the slot id of the new local.
#[test]
fn store_and_load_use_the_slot_id() {
    let main = main_function(&["fn main() {", "    x := 1", "    print(x)", "}"]);
    let expected = vec![
        Inst::Const {
            dst: Temp(0),
            value: 1,
        },
        Inst::Store {
            slot: SlotId(0),
            src: Temp(0),
        },
        Inst::Load {
            dst: Temp(1),
            slot: SlotId(0),
            sign: Sign::Signed,
        },
        Inst::Print {
            src: Temp(1),
            kind: PrintKind::I64,
        },
    ];
    assert_eq!(main.blocks[0].insts, expected);
}

/// A call has a destination only when the callee returns a value.
#[test]
fn call_has_a_destination_only_for_value_functions() {
    let main = main_function(&[
        "fn say(x: i64) {",
        "    print(x)",
        "}",
        "fn one() -> i64 {",
        "    return 1",
        "}",
        "fn main() {",
        "    say(one())",
        "}",
    ]);
    let expected = vec![
        Inst::Call {
            dst: Some(Temp(0)),
            func: "one".to_string(),
            args: vec![],
        },
        Inst::Call {
            dst: None,
            func: "say".to_string(),
            args: vec![Temp(0)],
        },
    ];
    assert_eq!(main.blocks[0].insts, expected);
}

/// `if` ends its block with a `Branch`; the then block jumps to the join block.
#[test]
fn if_terminators_are_branch_jump_and_return() {
    let main = main_function(&[
        "fn main() {",
        "    if true {",
        "        print(1)",
        "    }",
        "}",
    ]);
    let branch = Terminator::Branch {
        cond: Temp(0),
        then_to: BlockId(1),
        else_to: BlockId(2),
    };
    assert_eq!(main.blocks[0].term, branch);
    assert_eq!(main.blocks[1].term, Terminator::Jump(BlockId(2)));
    assert_eq!(main.blocks[2].term, Terminator::Return(None));
}

/// `return e` ends the block with `Return(Some(temp))`.
#[test]
fn return_with_value_carries_the_temp() {
    let lowered = module(&["fn f() -> i64 {", "    return 7", "}", "fn main() {", "}"]);
    let entry = &function(&lowered, "f").blocks[0];
    assert_eq!(entry.term, Terminator::Return(Some(Temp(0))));
}

/// `Module` values can be cloned, compared and debug-printed.
#[test]
fn module_supports_clone_eq_and_debug() {
    let lowered = module(&["fn main() {", "    print(1)", "}"]);
    let copy = lowered.clone();
    assert_eq!(lowered, copy);
    assert!(format!("{lowered:?}").contains("main"));
}

/// The id types are ordered and hashable.
#[test]
fn id_types_are_ordered_and_hashable() {
    assert!(Temp(1) < Temp(2));
    assert!(SlotId(1) < SlotId(2));
    assert!(BlockId(1) < BlockId(2));
    let temps: HashSet<Temp> = [Temp(1), Temp(1), Temp(2)].into_iter().collect();
    assert_eq!(temps.len(), 2);
}

/// Every temp a function mentions has an entry in `temp_types`, and there are no spare entries.
#[test]
fn temp_types_has_one_entry_per_temp() {
    let lowered = module(&[
        "fn fib(n: i64) -> i64 {",
        "    if n < 2 {",
        "        return n",
        "    }",
        "    return fib(n - 1) + fib(n - 2)",
        "}",
        "fn main() {",
        "    print(fib(10))",
        "}",
    ]);
    let fib = function(&lowered, "fib");
    let mut defined = 0;
    for block in &fib.blocks {
        for inst in &block.insts {
            match inst {
                Inst::Const { .. }
                | Inst::Load { .. }
                | Inst::Binary { .. }
                | Inst::Neg { .. }
                | Inst::Not { .. }
                | Inst::Call { dst: Some(_), .. } => defined += 1,
                Inst::Call { dst: None, .. } | Inst::Store { .. } | Inst::Print { .. } => {}
            }
        }
    }
    assert_eq!(fib.temp_types.len(), defined);
    assert_eq!(fib.temp_types.len(), 13);
}

/// Integer temps are `i64`; comparison, `!`, `&&` and `||` results and bool literals are `i8`.
#[test]
fn temp_types_follow_the_value_kinds() {
    let main = main_function(&[
        "fn main() {",
        "    a := 1",
        "    print(a < 2)",
        "    print(!true)",
        "}",
    ]);
    // t0 const 1, t1 load a, t2 const 2, t3 lt, t4 const 1 (true), t5 not.
    let expected = vec![
        IrType::I64,
        IrType::I64,
        IrType::I64,
        IrType::I8,
        IrType::I8,
        IrType::I8,
    ];
    assert_eq!(main.temp_types, expected);
}

/// In a range loop the constants and the step are `i64` and the comparison result is `i8`; `$end` is 8 bytes.
#[test]
fn range_loop_temps_and_slots_are_typed() {
    let main = main_function(&[
        "fn main() {",
        "    for i in 0..3 {",
        "        print(i)",
        "    }",
        "}",
    ]);
    for block in &main.blocks {
        for inst in &block.insts {
            match inst {
                Inst::Const { dst, .. } => assert_eq!(main.temp_types[dst.0 as usize], IrType::I64),
                Inst::Binary {
                    dst,
                    op: BinOp::Add(_),
                    ..
                } => assert_eq!(main.temp_types[dst.0 as usize], IrType::I64),
                Inst::Binary { dst, .. } => {
                    assert_eq!(main.temp_types[dst.0 as usize], IrType::I8)
                }
                _ => {}
            }
        }
    }
    assert_eq!(slot_names(&main), vec!["i", "$end"]);
    assert_eq!(main.slots[1].size, 8);
}

/// A `bool` parameter is an unsigned `i8`.
#[test]
fn bool_parameter_is_an_unsigned_i8() {
    let lowered = module(&[
        "fn f(a: i64, flag: bool) -> bool {",
        "    return flag",
        "}",
        "fn main() {",
        "}",
    ]);
    let f = function(&lowered, "f");
    assert_eq!(f.params[0].ty, IrType::I64);
    assert_eq!(f.params[0].sign, Sign::Signed);
    assert_eq!(f.params[1].ty, IrType::I8);
    assert_eq!(f.params[1].sign, Sign::Unsigned);
}

/// An `i64` slot is 8 bytes aligned to 8; a `bool` slot is 1 byte aligned to 1.
#[test]
fn slot_sizes_follow_the_variable_types() {
    let main = main_function(&[
        "fn main() {",
        "    n := 1",
        "    flag := true",
        "    print(n)",
        "    print(flag)",
        "}",
    ]);
    let sizes: Vec<(u32, u32)> = main.slots.iter().map(|s| (s.size, s.align)).collect();
    assert_eq!(sizes, vec![(8, 8), (1, 1)]);
}

/// The hidden slots: `$end` is 8 bytes, `$and` and `$or` are 1 byte.
#[test]
fn hidden_slot_sizes() {
    let main = main_function(&[
        "fn main() {",
        "    a := true",
        "    for i in 0..2 {",
        "        print(a && a)",
        "        print(a || a)",
        "    }",
        "}",
    ]);
    let sizes: Vec<(&str, u32)> = main
        .slots
        .iter()
        .map(|slot| (slot.name.as_str(), slot.size))
        .collect();
    let expected = vec![("a", 1), ("i", 8), ("$end", 8), ("$and", 1), ("$or", 1)];
    assert_eq!(sizes, expected);
}

/// A slot's alignment equals its size.
#[test]
fn slot_alignment_equals_size() {
    let main = main_function(&[
        "fn main() {",
        "    a := true",
        "    n := 5",
        "    print(a)",
        "    print(n)",
        "}",
    ]);
    for slot in &main.slots {
        assert_eq!(slot.align, slot.size, "{}", slot.name);
    }
}

/// Loads of `i64` variables are signed and loads of `bool` variables are unsigned.
#[test]
fn loads_carry_the_sign_of_their_type() {
    let main = main_function(&[
        "fn main() {",
        "    n := 1",
        "    flag := true",
        "    print(n)",
        "    print(flag)",
        "}",
    ]);
    let signs: Vec<(SlotId, Sign)> = main.blocks[0]
        .insts
        .iter()
        .filter_map(|inst| match inst {
            Inst::Load { slot, sign, .. } => Some((*slot, *sign)),
            _ => None,
        })
        .collect();
    assert_eq!(
        signs,
        vec![(SlotId(0), Sign::Signed), (SlotId(1), Sign::Unsigned)]
    );
}

/// Arithmetic and comparison operations on `i64` are `Signed`.
#[test]
fn i64_operations_are_signed() {
    let main = main_function(&[
        "fn main() {",
        "    a := 6",
        "    b := 3",
        "    print(a + b)",
        "    print(a - b)",
        "    print(a * b)",
        "    print(a / b)",
        "    print(a % b)",
        "    print(a < b)",
        "    print(a <= b)",
        "    print(a > b)",
        "    print(a >= b)",
        "    print(a == b)",
        "    print(a != b)",
        "}",
    ]);
    let ops: Vec<BinOp> = main.blocks[0]
        .insts
        .iter()
        .filter_map(|inst| match inst {
            Inst::Binary { op, .. } => Some(*op),
            _ => None,
        })
        .collect();
    let signed = Sign::Signed;
    let expected = vec![
        BinOp::Add(signed),
        BinOp::Sub(signed),
        BinOp::Mul(signed),
        BinOp::Div(signed),
        BinOp::Rem(signed),
        BinOp::Lt(signed),
        BinOp::Le(signed),
        BinOp::Gt(signed),
        BinOp::Ge(signed),
        BinOp::Eq,
        BinOp::Ne,
    ];
    assert_eq!(ops, expected);
}

/// The function's span is its name's span.
#[test]
fn function_span_covers_the_name() {
    let src = text(&["fn alpha() {", "}", "fn main() {", "}"]);
    let lowered = module(&["fn alpha() {", "}", "fn main() {", "}"]);
    let alpha = function(&lowered, "alpha");
    assert_eq!(&src[alpha.span.start..alpha.span.end], "alpha");
}
