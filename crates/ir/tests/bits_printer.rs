//! Integration tests for how `print_module` shows the bitwise operations, shifts and conversions of hand-built IR.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use ir::{
    BinOp, Block, BlockId, Function, Inst, IrType, Loc, Module, Sign, Temp, Terminator,
    print_module,
};
use source::Span;

/// The four IR widths, narrowest first.
const WIDTHS: [IrType; 4] = [IrType::I8, IrType::I16, IrType::I32, IrType::I64];

/// Both signs.
const SIGNS: [Sign; 2] = [Sign::Signed, Sign::Unsigned];

/// Joins lines into one text that ends with a newline.
fn text(lines: &[&str]) -> String {
    let mut joined = lines.join("\n");
    joined.push('\n');
    joined
}

/// Prints a `main` with the given temp types whose only block holds `insts` and returns.
fn print_main(temp_types: Vec<IrType>, insts: Vec<Inst>) -> String {
    let entry = Block {
        id: BlockId(0),
        insts,
        term: Terminator::Return(None),
    };
    let main = Function {
        name: "main".to_string(),
        params: vec![],
        ret: None,
        slots: vec![],
        temp_types,
        blocks: vec![entry],
        span: Span::new(0, 0),
    };
    print_module(&Module {
        functions: vec![main],
    })
}

/// Builds the expected text of an empty-slots `fn main` with one block of these lines.
fn main_text(block_lines: &[&str]) -> String {
    let mut lines = vec!["fn main() {", "  slots:", "bb0:"];
    lines.extend_from_slice(block_lines);
    lines.push("}");
    text(&lines)
}

/// Builds `t2 = OP t0, t1` at the location 3:7.
fn binary(op: BinOp) -> Inst {
    Inst::Binary {
        dst: Temp(2),
        op,
        lhs: Temp(0),
        rhs: Temp(1),
        loc: Loc { line: 3, col: 7 },
    }
}

/// How a type is spelled in a conversion: `i` or `u` and the bit width, such as `u8`.
fn type_name(ty: IrType, sign: Sign) -> String {
    let letter = match sign {
        Sign::Signed => 'i',
        Sign::Unsigned => 'u',
    };
    format!("{letter}{}", ty.bits())
}

/// The smallest and largest value of a type, as `i128`.
fn range(ty: IrType, sign: Sign) -> (i128, i128) {
    let bits = ty.bits();
    match sign {
        Sign::Signed => (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1),
        Sign::Unsigned => (0, (1i128 << bits) - 1),
    }
}

/// Whether every value of the source type fits the target type, so the conversion cannot fail.
fn cannot_fail(from: (IrType, Sign), to: (IrType, Sign)) -> bool {
    let (from_min, from_max) = range(from.0, from.1);
    let (to_min, to_max) = range(to.0, to.1);
    to_min <= from_min && from_max <= to_max
}

/// Prints one conversion from `from` to `to` at the location 4:9, with `t0` as the source and `t1` as the result.
fn print_conversion(from: (IrType, Sign), to: (IrType, Sign)) -> String {
    let conv = Inst::Conv {
        dst: Temp(1),
        src: Temp(0),
        from: from.1,
        to: to.1,
        loc: Loc { line: 4, col: 9 },
    };
    print_main(vec![from.0, to.0], vec![conv])
}

/// The expected text of a conversion line, with or without its location.
fn conversion_text(from: (IrType, Sign), to: (IrType, Sign), with_location: bool) -> String {
    let location = if with_location { " @4:9" } else { "" };
    let line = format!(
        "  t1:{to_ir} = conv.{from_name}.{to_name} t0{location}",
        to_ir = to.0,
        from_name = type_name(from.0, from.1),
        to_name = type_name(to.0, to.1),
    );
    main_text(&[&line, "  ret"])
}

/// `and`, `or` and `xor` print their mnemonic and no location, at every width.
#[test]
fn bitwise_operators_print_no_location() {
    let table = [(BinOp::And, "and"), (BinOp::Or, "or"), (BinOp::Xor, "xor")];
    for (op, mnemonic) in table {
        for width in WIDTHS {
            let line = format!("  t2:{width} = {mnemonic} t0, t1");
            let output = print_main(vec![width; 3], vec![binary(op)]);
            assert_eq!(output, main_text(&[&line, "  ret"]), "{mnemonic} {width}");
        }
    }
}

/// `shl` prints its location and no sign, since it can panic, at every width and for both signs.
#[test]
fn shl_prints_a_location_and_no_sign() {
    for sign in SIGNS {
        for width in WIDTHS {
            let line = format!("  t2:{width} = shl t0, t1 @3:7");
            let output = print_main(vec![width; 3], vec![binary(BinOp::Shl(sign))]);
            assert_eq!(output, main_text(&[&line, "  ret"]), "{width} {sign:?}");
        }
    }
}

/// `shr.s` and `shr.u` print their sign and a location, at every width.
#[test]
fn shr_prints_its_sign_and_a_location() {
    let table = [(Sign::Signed, "shr.s"), (Sign::Unsigned, "shr.u")];
    for (sign, mnemonic) in table {
        for width in WIDTHS {
            let line = format!("  t2:{width} = {mnemonic} t0, t1 @3:7");
            let output = print_main(vec![width; 3], vec![binary(BinOp::Shr(sign))]);
            assert_eq!(output, main_text(&[&line, "  ret"]), "{mnemonic} {width}");
        }
    }
}

/// The amount of a shift may have any width; the result keeps the type of the left operand.
#[test]
fn shift_amount_may_have_another_width() {
    for amount in WIDTHS {
        let output = print_main(
            vec![IrType::I16, amount, IrType::I16],
            vec![binary(BinOp::Shl(Sign::Signed))],
        );
        let expected = main_text(&["  t2:i16 = shl t0, t1 @3:7", "  ret"]);
        assert_eq!(output, expected, "amount {amount}");
    }
}

/// A conversion that can fail prints its location, for every pair of types.
#[test]
fn conversion_that_can_fail_prints_a_location() {
    let mut checked = 0;
    for from_ty in WIDTHS {
        for from_sign in SIGNS {
            for to_ty in WIDTHS {
                for to_sign in SIGNS {
                    let (from, to) = ((from_ty, from_sign), (to_ty, to_sign));
                    if cannot_fail(from, to) {
                        continue;
                    }
                    let expected = conversion_text(from, to, true);
                    assert_eq!(print_conversion(from, to), expected);
                    checked += 1;
                }
            }
        }
    }
    // 64 ordered pairs, of which 26 cannot fail.
    assert_eq!(checked, 38);
}

/// A conversion that cannot fail prints no location, for every pair of types.
#[test]
fn conversion_that_cannot_fail_prints_no_location() {
    let mut checked = 0;
    for from_ty in WIDTHS {
        for from_sign in SIGNS {
            for to_ty in WIDTHS {
                for to_sign in SIGNS {
                    let (from, to) = ((from_ty, from_sign), (to_ty, to_sign));
                    if !cannot_fail(from, to) {
                        continue;
                    }
                    let expected = conversion_text(from, to, false);
                    assert_eq!(print_conversion(from, to), expected);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 26);
}

/// The spec's example: `u8` to `i32` cannot fail and prints no location.
#[test]
fn u8_to_i32_prints_without_a_location() {
    let from = (IrType::I8, Sign::Unsigned);
    let to = (IrType::I32, Sign::Signed);
    let expected = main_text(&["  t1:i32 = conv.u8.i32 t0", "  ret"]);
    assert_eq!(print_conversion(from, to), expected);
}

/// The spec's example: `i64` to `u8` can fail and prints its location.
#[test]
fn i64_to_u8_prints_with_a_location() {
    let from = (IrType::I64, Sign::Signed);
    let to = (IrType::I8, Sign::Unsigned);
    let expected = main_text(&["  t1:i8 = conv.i64.u8 t0 @4:9", "  ret"]);
    assert_eq!(print_conversion(from, to), expected);
}

/// Widening a signed value to a wider unsigned type can still fail, so it prints a location.
#[test]
fn signed_to_wider_unsigned_prints_a_location() {
    let from = (IrType::I8, Sign::Signed);
    let to = (IrType::I64, Sign::Unsigned);
    let expected = main_text(&["  t1:i64 = conv.i8.u64 t0 @4:9", "  ret"]);
    assert_eq!(print_conversion(from, to), expected);
}

/// An unsigned value converted to a signed type of the same width can fail, so it prints a location.
#[test]
fn unsigned_to_signed_of_the_same_width_prints_a_location() {
    let from = (IrType::I32, Sign::Unsigned);
    let to = (IrType::I32, Sign::Signed);
    let expected = main_text(&["  t1:i32 = conv.u32.i32 t0 @4:9", "  ret"]);
    assert_eq!(print_conversion(from, to), expected);
}
