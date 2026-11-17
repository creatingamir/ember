//! Integration tests for the printed IR of the bitwise operators, shifts and `T(x)` conversions.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use ir::{BinOp, Inst, Sign, lower, print_module};
use source::SourceFile;

/// How lowering shows one source integer type.
struct IntCase {
    /// The source spelling, such as `u8`.
    name: &'static str,
    /// The IR type: signless, so `u8` and `i8` are both `i8`.
    ir: &'static str,
    /// The slot size in bytes.
    size: u32,
    /// The extension suffix of a load or a parameter: `.s` or `.u` for 8 and 16 bits, nothing above.
    extension: &'static str,
    /// The sign suffix of an operation: `s` or `u`.
    sign: &'static str,
    /// The bit width.
    bits: u32,
    /// The all-ones constant of the type, as the IR prints it.
    all_ones: &'static str,
}

/// All eight integer types, with how lowering shows each.
const CASES: [IntCase; 8] = [
    IntCase {
        name: "i8",
        ir: "i8",
        size: 1,
        extension: ".s",
        sign: "s",
        bits: 8,
        all_ones: "-1",
    },
    IntCase {
        name: "i16",
        ir: "i16",
        size: 2,
        extension: ".s",
        sign: "s",
        bits: 16,
        all_ones: "-1",
    },
    IntCase {
        name: "i32",
        ir: "i32",
        size: 4,
        extension: "",
        sign: "s",
        bits: 32,
        all_ones: "-1",
    },
    IntCase {
        name: "i64",
        ir: "i64",
        size: 8,
        extension: "",
        sign: "s",
        bits: 64,
        all_ones: "-1",
    },
    IntCase {
        name: "u8",
        ir: "i8",
        size: 1,
        extension: ".u",
        sign: "u",
        bits: 8,
        all_ones: "255",
    },
    IntCase {
        name: "u16",
        ir: "i16",
        size: 2,
        extension: ".u",
        sign: "u",
        bits: 16,
        all_ones: "65535",
    },
    IntCase {
        name: "u32",
        ir: "i32",
        size: 4,
        extension: "",
        sign: "u",
        bits: 32,
        all_ones: "4294967295",
    },
    IntCase {
        name: "u64",
        ir: "i64",
        size: 8,
        extension: "",
        sign: "u",
        bits: 64,
        all_ones: "-1",
    },
];

/// Joins lines into one text that ends with a newline.
fn text(lines: &[&str]) -> String {
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

/// Parses, checks, lowers and prints a program that must be valid.
fn ir_text(src: &str) -> String {
    let program = parser::parse_source(src).unwrap();
    let checked = check::check(&program).unwrap();
    let file = SourceFile::new("t.em", src);
    let module = lower(&program, &checked, &file).expect("lowering a checked program must succeed");
    print_module(&module)
}

/// Returns the printed text of one function of a program, ending in `}\n`.
fn function_ir(src: &str, name: &str) -> String {
    let header = format!("fn {name}(");
    let full = ir_text(src);
    let section = full
        .split("\n\n")
        .find(|section| section.starts_with(&header))
        .unwrap();
    format!("{}\n", section.trim_end())
}

/// Returns the printed `f` of a program that is `f` and an empty `main`, given the lines of `f`.
fn f_ir(f_lines: &[&str]) -> String {
    let mut lines = f_lines.to_vec();
    lines.extend(["fn main() {", "}"]);
    function_ir(&text(&lines), "f")
}

/// The printed `fn f(a: ty, ...) -> ret { return EXPR }` for a one-parameter function.
fn unary_function_ir(param_type: &str, ret_type: &str, expr: &str) -> String {
    let header = format!("fn f(a: {param_type}) -> {ret_type} {{");
    let statement = format!("    return {expr}");
    f_ir(&[&header, &statement, "}"])
}

/// The printed `fn f(a: ty, b: ty2) -> ret { return a OP b }`.
fn binary_function_ir(left_type: &str, right_type: &str, ret_type: &str, expr: &str) -> String {
    let header = format!("fn f(a: {left_type}, b: {right_type}) -> {ret_type} {{");
    let statement = format!("    return {expr}");
    f_ir(&[&header, &statement, "}"])
}

/// The header and slots lines of a function with parameters `a` and `b` of the given cases.
fn two_parameter_head(left: &IntCase, right: &IntCase) -> [String; 2] {
    [
        format!(
            "fn f(a:{}{}, b:{}{}) -> {} {{",
            left.ir, left.extension, right.ir, right.extension, left.ir
        ),
        format!("  slots: a:{}, b:{}", left.size, right.size),
    ]
}

// ---------------------------------------------------------------- bitwise operators

/// `a & b`, `a | b` and `a ^ b` lower to `and`, `or` and `xor` of the loaded operands, at every type.
#[test]
fn bitwise_operators_lower_to_and_or_xor_without_a_location() {
    let table = [("&", "and"), ("|", "or"), ("^", "xor")];
    for case in &CASES {
        for (symbol, mnemonic) in table {
            let expr = format!("a {symbol} b");
            let actual = binary_function_ir(case.name, case.name, case.name, &expr);
            let [header, slots] = two_parameter_head(case, case);
            let (ir, extension) = (case.ir, case.extension);
            let expected = text(&[
                &header,
                &slots,
                "bb0:",
                &format!("  t0:{ir} = load{extension} a"),
                &format!("  t1:{ir} = load{extension} b"),
                &format!("  t2:{ir} = {mnemonic} t0, t1"),
                "  ret t2",
                "}",
            ]);
            assert_eq!(actual, expected, "{} {symbol}", case.name);
        }
    }
}

/// `~a` lowers to an `xor` with a constant of all ones in the type, at every type.
#[test]
fn not_lowers_to_xor_with_all_ones() {
    for case in &CASES {
        let actual = unary_function_ir(case.name, case.name, "~a");
        let (ir, extension, size) = (case.ir, case.extension, case.size);
        let expected = text(&[
            &format!("fn f(a:{ir}{extension}) -> {ir} {{"),
            &format!("  slots: a:{size}"),
            "bb0:",
            &format!("  t0:{ir} = load{extension} a"),
            &format!("  t1:{ir} = const {}", case.all_ones),
            &format!("  t2:{ir} = xor t0, t1"),
            "  ret t2",
            "}",
        ]);
        assert_eq!(actual, expected, "{}", case.name);
    }
}

/// The spec's example: `~a` with `a: u8` is `const 255` then `xor`.
#[test]
fn not_of_a_u8_uses_the_constant_255() {
    let actual = unary_function_ir("u8", "u8", "~a");
    let expected = text(&[
        "fn f(a:i8.u) -> i8 {",
        "  slots: a:1",
        "bb0:",
        "  t0:i8 = load.u a",
        "  t1:i8 = const 255",
        "  t2:i8 = xor t0, t1",
        "  ret t2",
        "}",
    ]);
    assert_eq!(actual, expected);
}

/// The spec's example: `~a` with `a: i8` uses the constant -1.
#[test]
fn not_of_an_i8_uses_the_constant_minus_one() {
    let actual = unary_function_ir("i8", "i8", "~a");
    assert!(actual.contains("  t1:i8 = const -1\n"), "{actual}");
    assert!(actual.contains("  t2:i8 = xor t0, t1\n"), "{actual}");
}

/// `~0` with a `u8` annotation settles the literal, so the IR is `const 0`, `const 255`, `xor`.
#[test]
fn not_of_a_literal_lowers_in_the_annotated_type() {
    let source = text(&["fn main() {", "    x: u8 = ~0", "    print(x)", "}"]);
    let expected = text(&[
        "fn main() {",
        "  slots: x:1",
        "bb0:",
        "  t0:i8 = const 0",
        "  t1:i8 = const 255",
        "  t2:i8 = xor t0, t1",
        "  store x, t2",
        "  t3:i8 = load.u x",
        "  print.u64 t3",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A bitwise operator with a literal operand lowers the literal in the other operand's type.
#[test]
fn bitwise_with_a_literal_lowers_the_literal_in_the_operand_type() {
    let actual = unary_function_ir("u16", "u16", "a & 255");
    let expected = text(&[
        "fn f(a:i16.u) -> i16 {",
        "  slots: a:2",
        "bb0:",
        "  t0:i16 = load.u a",
        "  t1:i16 = const 255",
        "  t2:i16 = and t0, t1",
        "  ret t2",
        "}",
    ]);
    assert_eq!(actual, expected);
}

// ---------------------------------------------------------------- shifts

/// Checks `a << b` or `a >> b` for every pair of value type and amount type.
fn assert_shift_for_every_pair(symbol: &str, mnemonic_for: fn(&IntCase) -> String) {
    for value in &CASES {
        for amount in &CASES {
            let expr = format!("a {symbol} b");
            let actual = binary_function_ir(value.name, amount.name, value.name, &expr);
            let [header, slots] = two_parameter_head(value, amount);
            let expected = text(&[
                &header,
                &slots,
                "bb0:",
                &format!("  t0:{} = load{} a", value.ir, value.extension),
                &format!("  t1:{} = load{} b", amount.ir, amount.extension),
                &format!("  t2:{} = {} t0, t1 @2:12", value.ir, mnemonic_for(value)),
                "  ret t2",
                "}",
            ]);
            assert_eq!(actual, expected, "{} {symbol} {}", value.name, amount.name);
        }
    }
}

/// `a << b` lowers to `shl` with a location, whatever the types of `a` and `b`.
#[test]
fn shl_lowers_for_every_pair_of_types() {
    assert_shift_for_every_pair("<<", |_| "shl".to_string());
}

/// `a >> b` lowers to `shr.s` for a signed `a` and `shr.u` for an unsigned one, with a location.
#[test]
fn shr_follows_the_sign_of_the_left_operand() {
    assert_shift_for_every_pair(">>", |case| format!("shr.{}", case.sign));
}

/// The spec's example: `a >> n` with `a: u32` is `shr.u` and with `a: i32` is `shr.s`.
#[test]
fn shr_of_32_bit_values_picks_the_sign() {
    let unsigned = binary_function_ir("u32", "i8", "u32", "a >> b");
    assert!(
        unsigned.contains("  t2:i32 = shr.u t0, t1 @2:12\n"),
        "{unsigned}"
    );
    let signed = binary_function_ir("i32", "i8", "i32", "a >> b");
    assert!(
        signed.contains("  t2:i32 = shr.s t0, t1 @2:12\n"),
        "{signed}"
    );
}

/// The shift's location is the start of the left operand, so a parenthesized operand starts at its parenthesis.
#[test]
fn shift_location_is_the_start_of_the_expression() {
    let actual = binary_function_ir("u8", "u8", "u8", "(a) << b");
    assert!(actual.contains("  t2:i8 = shl t0, t1 @2:12\n"), "{actual}");
}

/// A literal left operand takes the annotation's type and the amount keeps its own type.
#[test]
fn shifted_literal_lowers_in_the_annotated_type() {
    let source = text(&[
        "fn main() {",
        "    n: i32 = 3",
        "    x: u8 = 1 << n",
        "    print(x)",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots: n:4, x:1",
        "bb0:",
        "  t0:i32 = const 3",
        "  store n, t0",
        "  t1:i8 = const 1",
        "  t2:i32 = load n",
        "  t3:i8 = shl t1, t2 @3:13",
        "  store x, t3",
        "  t4:i8 = load.u x",
        "  print.u64 t4",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A literal amount lowers as an `i64` constant, whatever the left operand's type.
#[test]
fn literal_amount_lowers_as_an_i64_constant() {
    let actual = unary_function_ir("u8", "u8", "a << 5");
    let expected = text(&[
        "fn f(a:i8.u) -> i8 {",
        "  slots: a:1",
        "bb0:",
        "  t0:i8 = load.u a",
        "  t1:i64 = const 5",
        "  t2:i8 = shl t0, t1 @2:12",
        "  ret t2",
        "}",
    ]);
    assert_eq!(actual, expected);
}

// ---------------------------------------------------------------- conversions

/// Whether every value of `from` fits `to`, so the conversion cannot fail and prints no location.
fn cannot_fail(from: &IntCase, to: &IntCase) -> bool {
    let from_signed = from.sign == "s";
    let to_signed = to.sign == "s";
    match (from_signed, to_signed) {
        (true, false) => false,
        (false, true) => to.bits > from.bits,
        _ => to.bits >= from.bits,
    }
}

/// Checks `fn f(a: S) -> T { return T(a) }` for every pair of different types.
#[test]
fn conversion_lowers_to_conv_for_every_pair_of_different_types() {
    for from in &CASES {
        for to in &CASES {
            if from.name == to.name {
                continue;
            }
            let expr = format!("{}(a)", to.name);
            let actual = unary_function_ir(from.name, to.name, &expr);
            let location = if cannot_fail(from, to) { "" } else { " @2:12" };
            let expected = text(&[
                &format!("fn f(a:{}{}) -> {} {{", from.ir, from.extension, to.ir),
                &format!("  slots: a:{}", from.size),
                "bb0:",
                &format!("  t0:{} = load{} a", from.ir, from.extension),
                &format!(
                    "  t1:{} = conv.{}.{} t0{location}",
                    to.ir, from.name, to.name
                ),
                "  ret t1",
                "}",
            ]);
            assert_eq!(actual, expected, "{} -> {}", from.name, to.name);
        }
    }
}

/// The spec's example: `u8(x)` with `x: i64` prints `conv.i64.u8` with a location.
#[test]
fn u8_of_an_i64_prints_a_location() {
    let actual = unary_function_ir("i64", "u8", "u8(a)");
    assert!(
        actual.contains("  t1:i8 = conv.i64.u8 t0 @2:12\n"),
        "{actual}"
    );
}

/// The spec's example: `i32(y)` with `y: u8` prints `conv.u8.i32` with no location.
#[test]
fn i32_of_a_u8_prints_no_location() {
    let actual = unary_function_ir("u8", "i32", "i32(a)");
    assert!(actual.contains("  t1:i32 = conv.u8.i32 t0\n"), "{actual}");
}

/// The spec's example: `i64(z)` with `z: i64` emits no `conv`: the result is the loaded temp.
#[test]
fn conversion_to_the_same_type_emits_no_conv() {
    for case in &CASES {
        let expr = format!("{}(a)", case.name);
        let actual = unary_function_ir(case.name, case.name, &expr);
        let expected = text(&[
            &format!("fn f(a:{}{}) -> {} {{", case.ir, case.extension, case.ir),
            &format!("  slots: a:{}", case.size),
            "bb0:",
            &format!("  t0:{} = load{} a", case.ir, case.extension),
            "  ret t0",
            "}",
        ]);
        assert_eq!(actual, expected, "{}", case.name);
    }
}

/// The location of a conversion is the start of the call, so a parenthesized call starts at its parenthesis.
#[test]
fn conversion_location_is_the_start_of_the_expression() {
    let actual = unary_function_ir("i64", "u8", "(u8(a))");
    assert!(
        actual.contains("  t1:i8 = conv.i64.u8 t0 @2:12\n"),
        "{actual}"
    );
}

/// A literal argument settles to the target type, so the conversion is a plain constant.
#[test]
fn conversion_of_a_literal_is_a_constant() {
    let source = text(&["fn main() {", "    x := u8(200)", "    print(x)", "}"]);
    let expected = text(&[
        "fn main() {",
        "  slots: x:1",
        "bb0:",
        "  t0:i8 = const 200",
        "  store x, t0",
        "  t1:i8 = load.u x",
        "  print.u64 t1",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// The smallest value of a signed type converts as one constant.
#[test]
fn conversion_of_the_smallest_literal_is_one_constant() {
    let source = text(&["fn main() {", "    x := i8(-128)", "    print(x)", "}"]);
    let actual = function_ir(&source, "main");
    assert!(actual.contains("  t0:i8 = const -128\n"), "{actual}");
    assert!(!actual.contains("conv"), "{actual}");
}

/// A converted value can be printed: the result type decides the print kind.
#[test]
fn printing_a_conversion_uses_the_target_signedness() {
    let source = text(&[
        "fn main() {",
        "    x: i32 = 7",
        "    print(u16(x))",
        "    print(i8(x))",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots: x:4",
        "bb0:",
        "  t0:i32 = const 7",
        "  store x, t0",
        "  t1:i32 = load x",
        "  t2:i16 = conv.i32.u16 t1 @3:11",
        "  print.u64 t2",
        "  t3:i32 = load x",
        "  t4:i8 = conv.i32.i8 t3 @4:11",
        "  print.i64 t4",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

/// A masked conversion lowers the mask, the `and` and then the conversion, in that order.
#[test]
fn conversion_of_a_masked_value_lowers_in_order() {
    let source = text(&[
        "fn main() {",
        "    x: i64 = 1000",
        "    print(u8(x & 255))",
        "}",
    ]);
    let expected = text(&[
        "fn main() {",
        "  slots: x:8",
        "bb0:",
        "  t0:i64 = const 1000",
        "  store x, t0",
        "  t1:i64 = load x",
        "  t2:i64 = const 255",
        "  t3:i64 = and t1, t2",
        "  t4:i8 = conv.i64.u8 t3 @3:11",
        "  print.u64 t4",
        "  ret",
        "}",
    ]);
    assert_eq!(function_ir(&source, "main"), expected);
}

// ---------------------------------------------------------------- the sign of `shl`

/// The sign of every `shl` in the `main` of a program, read from the lowered structure.
fn shl_signs(src: &str) -> Vec<Sign> {
    let program = parser::parse_source(src).unwrap();
    let checked = check::check(&program).unwrap();
    let file = SourceFile::new("t.em", src);
    let module = lower(&program, &checked, &file).expect("lowering a checked program must succeed");
    let main = module.functions.iter().find(|f| f.name == "main").unwrap();
    let mut signs = Vec::new();
    for block in &main.blocks {
        for inst in &block.insts {
            if let Inst::Binary {
                op: BinOp::Shl(sign),
                ..
            } = inst
            {
                signs.push(*sign);
            }
        }
    }
    signs
}

/// A constant left operand takes the annotation's sign: `x: u8 = 1 << n` is an unsigned `shl`.
#[test]
fn shl_of_a_constant_for_a_u8_is_unsigned() {
    let src = text(&[
        "fn main() {",
        "    n: i32 = 1",
        "    x: u8 = 1 << n",
        "    print(x)",
        "}",
    ]);
    assert_eq!(shl_signs(&src), [Sign::Unsigned]);
}

/// A constant left operand for an `i8` annotation is a signed `shl`.
#[test]
fn shl_of_a_constant_for_an_i8_is_signed() {
    let src = text(&[
        "fn main() {",
        "    n: i32 = 1",
        "    y: i8 = 64 << n",
        "    print(y)",
        "}",
    ]);
    assert_eq!(shl_signs(&src), [Sign::Signed]);
}

/// A call result as the left operand gives the sign of the callee's return type.
#[test]
fn shl_of_a_call_result_follows_the_return_type() {
    for (ret, sign) in [("u8", Sign::Unsigned), ("i8", Sign::Signed)] {
        let src = text(&[
            &format!("fn f() -> {ret} {{"),
            "    return 1",
            "}",
            "fn main() {",
            "    n: i32 = 1",
            "    print(f() << n)",
            "}",
        ]);
        assert_eq!(shl_signs(&src), [sign], "{ret}");
    }
}

/// A variable as the left operand gives its own sign, for every type.
#[test]
fn shl_of_a_variable_follows_its_type() {
    for case in &CASES {
        let declaration = format!("    a: {} = 1", case.name);
        let src = text(&[
            "fn main() {",
            &declaration,
            "    n: i32 = 1",
            "    print(a << n)",
            "}",
        ]);
        let expected = if case.sign == "s" {
            Sign::Signed
        } else {
            Sign::Unsigned
        };
        assert_eq!(shl_signs(&src), [expected], "{}", case.name);
    }
}

/// The printed text of that `shl` carries no sign.
#[test]
fn shl_of_a_constant_prints_without_a_sign() {
    let src = text(&[
        "fn main() {",
        "    n: i32 = 1",
        "    x: u8 = 1 << n",
        "    print(x)",
        "}",
    ]);
    assert!(ir_text(&src).contains("  t3:i8 = shl t1, t2 @3:13\n"));
}
