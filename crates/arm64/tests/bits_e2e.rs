//! End-to-end tests for the bitwise operators, shifts and conversions: programs run and print what Rust computes.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

mod common;

use common::{main_with, panic_stderr, run};

// ----------------------------------------------------------------------------------------------
// Types, values and the Rust references
// ----------------------------------------------------------------------------------------------

/// One integer type of the language.
#[derive(Clone, Copy, Debug)]
struct Ty {
    /// The source spelling, such as `u8`.
    name: &'static str,
    /// The width in bits.
    bits: u32,
    /// Whether the type can hold negative values.
    signed: bool,
}

/// All eight integer types, signed first.
const TYPES: [Ty; 8] = [
    Ty {
        name: "i8",
        bits: 8,
        signed: true,
    },
    Ty {
        name: "i16",
        bits: 16,
        signed: true,
    },
    Ty {
        name: "i32",
        bits: 32,
        signed: true,
    },
    Ty {
        name: "i64",
        bits: 64,
        signed: true,
    },
    Ty {
        name: "u8",
        bits: 8,
        signed: false,
    },
    Ty {
        name: "u16",
        bits: 16,
        signed: false,
    },
    Ty {
        name: "u32",
        bits: 32,
        signed: false,
    },
    Ty {
        name: "u64",
        bits: 64,
        signed: false,
    },
];

impl Ty {
    /// The smallest value of the type.
    fn min(self) -> i128 {
        if self.signed {
            -(1i128 << (self.bits - 1))
        } else {
            0
        }
    }

    /// The largest value of the type.
    fn max(self) -> i128 {
        if self.signed {
            (1i128 << (self.bits - 1)) - 1
        } else {
            (1i128 << self.bits) - 1
        }
    }

    /// Whether `value` is a value of the type.
    fn holds(self, value: i128) -> bool {
        (self.min()..=self.max()).contains(&value)
    }

    /// The value whose bit pattern is the low `bits` bits of `pattern`.
    fn value_of_pattern(self, pattern: u128) -> i128 {
        let low_bits = pattern & ((1u128 << self.bits) - 1);
        let value = i128::try_from(low_bits).unwrap();
        if self.signed && value > self.max() {
            value - (1i128 << self.bits)
        } else {
            value
        }
    }

    /// The boundary values: for signed `MIN`, `MIN+1`, -2, -1, 0, 1, 2, `MAX-1`, `MAX`; for unsigned 0, 1, 2, `MAX/2`, `MAX-1`, `MAX`.
    fn boundaries(self) -> Vec<i128> {
        if self.signed {
            vec![
                self.min(),
                self.min() + 1,
                -2,
                -1,
                0,
                1,
                2,
                self.max() - 1,
                self.max(),
            ]
        } else {
            vec![0, 1, 2, self.max() / 2, self.max() - 1, self.max()]
        }
    }

    /// The boundary values plus the alternating bit patterns `0x55..` and `0xAA..`, sorted and without repeats.
    fn bit_values(self) -> Vec<i128> {
        let mut values = self.boundaries();
        values.push(self.value_of_pattern(0x5555_5555_5555_5555));
        values.push(self.value_of_pattern(0xAAAA_AAAA_AAAA_AAAA));
        values.sort_unstable();
        values.dedup();
        values
    }
}

/// Runs `$body` with the Rust integer type that matches `$ty` bound to the type alias `$t`.
macro_rules! with_rust_type {
    ($ty:expr, $t:ident => $body:expr) => {
        match ($ty.bits, $ty.signed) {
            (8, true) => {
                type $t = i8;
                $body
            }
            (16, true) => {
                type $t = i16;
                $body
            }
            (32, true) => {
                type $t = i32;
                $body
            }
            (64, true) => {
                type $t = i64;
                $body
            }
            (8, false) => {
                type $t = u8;
                $body
            }
            (16, false) => {
                type $t = u16;
                $body
            }
            (32, false) => {
                type $t = u32;
                $body
            }
            (64, false) => {
                type $t = u64;
                $body
            }
            _ => unreachable!("every width and sign is listed"),
        }
    };
}

/// A binary bitwise operator.
#[derive(Clone, Copy, Debug)]
enum Bitwise {
    And,
    Or,
    Xor,
}

/// The three binary bitwise operators.
const BITWISE: [Bitwise; 3] = [Bitwise::And, Bitwise::Or, Bitwise::Xor];

impl Bitwise {
    /// The source spelling.
    fn symbol(self) -> &'static str {
        match self {
            Bitwise::And => "&",
            Bitwise::Or => "|",
            Bitwise::Xor => "^",
        }
    }
}

/// `a OP b` computed by Rust's operator on the Rust type of `ty`.
fn reference_bitwise(ty: Ty, op: Bitwise, a: i128, b: i128) -> i128 {
    with_rust_type!(ty, T => {
        let (a, b) = (T::try_from(a).unwrap(), T::try_from(b).unwrap());
        let result: T = match op {
            Bitwise::And => a & b,
            Bitwise::Or => a | b,
            Bitwise::Xor => a ^ b,
        };
        i128::from(result)
    })
}

/// `!a` computed by Rust's operator on the Rust type of `ty`.
fn reference_not(ty: Ty, a: i128) -> i128 {
    with_rust_type!(ty, T => {
        let result: T = !T::try_from(a).unwrap();
        i128::from(result)
    })
}

/// `a << amount` or `a >> amount` computed by `wrapping_shl` and `wrapping_shr`; `amount` must be below the width.
fn reference_shift(ty: Ty, left: bool, a: i128, amount: u32) -> i128 {
    with_rust_type!(ty, T => {
        let a = T::try_from(a).unwrap();
        let result: T = if left {
            a.wrapping_shl(amount)
        } else {
            a.wrapping_shr(amount)
        };
        i128::from(result)
    })
}

/// `to::try_from(value)` computed by Rust: the converted value, or `None` when it does not fit.
fn reference_convert(to: Ty, value: i128) -> Option<i128> {
    with_rust_type!(to, T => T::try_from(value).ok().map(i128::from))
}

/// The source text of a value: decimal, with a minus sign when negative.
fn literal(value: i128) -> String {
    value.to_string()
}

/// Declarations `v0: ty = ...`, one per value.
fn declare_values(ty: Ty, values: &[i128]) -> Vec<String> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("    v{index}: {} = {}", ty.name, literal(*value)))
        .collect()
}

/// Runs a program that must succeed and returns its stdout.
fn run_ok(src: &str) -> String {
    let (stdout, stderr, code) = run(src);
    assert_eq!(stderr, "", "unexpected stderr");
    assert_eq!(code, 0, "unexpected exit code");
    stdout
}

/// Asserts that the program prints `1`, then panics with `reason` at the `nth` occurrence of `needle`, exit code 101.
fn assert_panics_after_one(src: &str, reason: &str, needle: &str, context: &str) {
    let (stdout, stderr, code) = run(src);
    assert_eq!(stderr, panic_stderr(reason, src, needle, 0), "{context}");
    assert_eq!(code, 101, "{context}");
    assert_eq!(stdout, "1\n", "{context}");
}

// ----------------------------------------------------------------------------------------------
// Test 13: bitwise operators
// ----------------------------------------------------------------------------------------------

/// A program that prints `&`, `|`, `^` of every pair of values and `~` of every value, and its expected output.
fn bitwise_program(ty: Ty) -> (String, String) {
    let values = ty.bit_values();
    let mut body = declare_values(ty, &values);
    let mut expected = String::new();
    for op in BITWISE {
        for (i, a) in values.iter().enumerate() {
            for (j, b) in values.iter().enumerate() {
                body.push(format!("    print(v{i} {} v{j})", op.symbol()));
                expected.push_str(&format!("{}\n", reference_bitwise(ty, op, *a, *b)));
            }
        }
    }
    for (i, a) in values.iter().enumerate() {
        body.push(format!("    print(~v{i})"));
        expected.push_str(&format!("{}\n", reference_not(ty, *a)));
    }
    (main_with(&body), expected)
}

/// The bitwise operators print Rust's results for every pair of boundary and pattern values of one type.
fn check_bitwise(ty: Ty) {
    let (src, expected) = bitwise_program(ty);
    assert_eq!(run_ok(&src), expected, "{}", ty.name);
}

// ----------------------------------------------------------------------------------------------
// Test 14: shifts
// ----------------------------------------------------------------------------------------------

/// A program that shifts every value of `ty` left and right by every amount below the width, with amounts of every type.
fn shift_program(ty: Ty) -> (String, String) {
    let values = ty.bit_values();
    let mut body = declare_values(ty, &values);
    for amount_type in TYPES {
        body.push(format!(
            "    limit_{0}: {0} = {1}",
            amount_type.name, ty.bits
        ));
    }
    let mut expected = String::new();
    for amount_type in TYPES {
        for (i, a) in values.iter().enumerate() {
            for (symbol, left) in [("<<", true), (">>", false)] {
                body.push(format!("    for s in 0..limit_{} {{", amount_type.name));
                body.push(format!("        print(v{i} {symbol} s)"));
                body.push("    }".to_string());
                for amount in 0..ty.bits {
                    expected.push_str(&format!("{}\n", reference_shift(ty, left, *a, amount)));
                }
            }
        }
    }
    (main_with(&body), expected)
}

/// Shifting by every amount below the width, with every amount type, prints Rust's wrapping shifts.
fn check_shifts(ty: Ty) {
    let (src, expected) = shift_program(ty);
    assert_eq!(run_ok(&src), expected, "{}", ty.name);
}

// ----------------------------------------------------------------------------------------------
// Test 15: shift panics
// ----------------------------------------------------------------------------------------------

/// The amounts that must panic for a shift of a `ty` value: the amount's type name and value.
fn bad_amounts(ty: Ty) -> Vec<(&'static str, i128)> {
    let width = i128::from(ty.bits);
    vec![
        ("i64", width),
        ("u8", width),
        ("i8", -1),
        ("i8", -128),
        ("i64", -1),
        ("i32", i128::from(i32::MIN)),
        ("u64", i128::from(u64::MAX)),
    ]
}

/// Asserts that shifting a `ty` value left and right by each bad amount panics with the exact message and location.
fn check_shift_panics(ty: Ty) {
    for (amount_type, amount) in bad_amounts(ty) {
        for symbol in ["<<", ">>"] {
            let body = [
                "    print(1)".to_string(),
                format!("    x: {} = 1", ty.name),
                format!("    n: {amount_type} = {}", literal(amount)),
                format!("    print(x {symbol} n)"),
            ];
            let src = main_with(&body);
            let needle = format!("x {symbol} n");
            let context = format!("{} {symbol} {amount_type} {amount}", ty.name);
            assert_panics_after_one(&src, "shift amount out of range", &needle, &context);
        }
    }
}

/// `x << 8` with `x: u8` and the amount in a variable panics at the shift.
#[test]
fn u8_shifted_left_by_8_panics() {
    let src = main_with(&[
        "    print(1)",
        "    x: u8 = 1",
        "    n := 8",
        "    print(x << n)",
    ]);
    assert_panics_after_one(&src, "shift amount out of range", "x << n", "u8 << 8");
}

/// `x >> -1` with an `i8` amount panics at the shift.
#[test]
fn shift_right_by_a_negative_i8_panics() {
    let src = main_with(&[
        "    print(1)",
        "    x: i32 = 100",
        "    n: i8 = -1",
        "    print(x >> n)",
    ]);
    assert_panics_after_one(&src, "shift amount out of range", "x >> n", "i32 >> -1");
}

/// `x << 64` with `x: i64` panics at the shift.
#[test]
fn i64_shifted_left_by_64_panics() {
    let src = main_with(&[
        "    print(1)",
        "    x: i64 = 1",
        "    n := 64",
        "    print(x << n)",
    ]);
    assert_panics_after_one(&src, "shift amount out of range", "x << n", "i64 << 64");
}

/// A shift by exactly width minus one is fine, and the next amount panics: the boundary is at the width.
#[test]
fn shift_by_width_minus_one_succeeds_and_by_width_panics() {
    let src = main_with(&[
        "    x: u8 = 255",
        "    n: u8 = 7",
        "    print(x >> n)",
        "    n = n + 1",
        "    print(x >> n)",
    ]);
    let (stdout, stderr, code) = run(&src);
    assert_eq!(stdout, "1\n");
    assert_eq!(
        stderr,
        panic_stderr("shift amount out of range", &src, "x >> n", 1)
    );
    assert_eq!(code, 101);
}

// ----------------------------------------------------------------------------------------------
// Test 16: conversions
// ----------------------------------------------------------------------------------------------

/// The values converted from `from`: its boundaries and the edges of every target (`MIN`, `MAX`, `MIN-1`, `MAX+1`) that it can hold.
fn conversion_values(from: Ty) -> Vec<i128> {
    let mut values = from.boundaries();
    for to in TYPES {
        for edge in [to.min() - 1, to.min(), to.max(), to.max() + 1] {
            if from.holds(edge) {
                values.push(edge);
            }
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

/// A program that converts every value of `from` to every type that Rust accepts it for, and its expected output.
fn conversion_program(from: Ty) -> (String, String) {
    let values = conversion_values(from);
    let mut body = declare_values(from, &values);
    let mut expected = String::new();
    for to in TYPES {
        for (index, value) in values.iter().enumerate() {
            if let Some(converted) = reference_convert(to, *value) {
                body.push(format!("    print({}(v{index}))", to.name));
                expected.push_str(&format!("{converted}\n"));
            }
        }
    }
    (main_with(&body), expected)
}

/// Every value that Rust's `try_from` accepts converts to the same number.
fn check_conversions(from: Ty) {
    let (src, expected) = conversion_program(from);
    assert_eq!(run_ok(&src), expected, "{}", from.name);
}

/// The values of `from` just outside `to`: `MIN-1` and `MAX+1` of the target, when the source holds them.
fn rejected_values(from: Ty, to: Ty) -> Vec<i128> {
    [to.min() - 1, to.max() + 1]
        .into_iter()
        .filter(|value| from.holds(*value))
        .collect()
}

/// Asserts that converting each rejected value of `from` to every other type panics with the exact message and location.
fn check_conversion_panics(from: Ty) {
    for to in TYPES {
        for value in rejected_values(from, to) {
            assert!(reference_convert(to, value).is_none());
            let body = [
                "    print(1)".to_string(),
                format!("    v: {} = {}", from.name, literal(value)),
                format!("    print({}(v))", to.name),
            ];
            let src = main_with(&body);
            let needle = format!("{}(v)", to.name);
            let context = format!("{} -> {} of {value}", from.name, to.name);
            assert_panics_after_one(&src, "integer conversion out of range", &needle, &context);
        }
    }
}

/// Defines one test per type for each check, so the programs run in parallel.
macro_rules! per_type_tests {
    ($( ($test:ident, $check:ident, $index:expr) ),* $(,)?) => {
        $(
            /// Runs the check for one type.
            #[test]
            fn $test() {
                $check(TYPES[$index]);
            }
        )*
    };
}

per_type_tests! {
    (i8_bitwise_matches_rust, check_bitwise, 0),
    (i16_bitwise_matches_rust, check_bitwise, 1),
    (i32_bitwise_matches_rust, check_bitwise, 2),
    (i64_bitwise_matches_rust, check_bitwise, 3),
    (u8_bitwise_matches_rust, check_bitwise, 4),
    (u16_bitwise_matches_rust, check_bitwise, 5),
    (u32_bitwise_matches_rust, check_bitwise, 6),
    (u64_bitwise_matches_rust, check_bitwise, 7),
    (i8_shifts_match_rust, check_shifts, 0),
    (i16_shifts_match_rust, check_shifts, 1),
    (i32_shifts_match_rust, check_shifts, 2),
    (i64_shifts_match_rust, check_shifts, 3),
    (u8_shifts_match_rust, check_shifts, 4),
    (u16_shifts_match_rust, check_shifts, 5),
    (u32_shifts_match_rust, check_shifts, 6),
    (u64_shifts_match_rust, check_shifts, 7),
    (i8_shift_panics, check_shift_panics, 0),
    (i16_shift_panics, check_shift_panics, 1),
    (i32_shift_panics, check_shift_panics, 2),
    (i64_shift_panics, check_shift_panics, 3),
    (u8_shift_panics, check_shift_panics, 4),
    (u16_shift_panics, check_shift_panics, 5),
    (u32_shift_panics, check_shift_panics, 6),
    (u64_shift_panics, check_shift_panics, 7),
    (i8_conversions_match_rust, check_conversions, 0),
    (i16_conversions_match_rust, check_conversions, 1),
    (i32_conversions_match_rust, check_conversions, 2),
    (i64_conversions_match_rust, check_conversions, 3),
    (u8_conversions_match_rust, check_conversions, 4),
    (u16_conversions_match_rust, check_conversions, 5),
    (u32_conversions_match_rust, check_conversions, 6),
    (u64_conversions_match_rust, check_conversions, 7),
    (i8_conversion_panics, check_conversion_panics, 0),
    (i16_conversion_panics, check_conversion_panics, 1),
    (i32_conversion_panics, check_conversion_panics, 2),
    (i64_conversion_panics, check_conversion_panics, 3),
    (u8_conversion_panics, check_conversion_panics, 4),
    (u16_conversion_panics, check_conversion_panics, 5),
    (u32_conversion_panics, check_conversion_panics, 6),
    (u64_conversion_panics, check_conversion_panics, 7),
}

/// Exactly the pairs where some source value does not fit the target have a panic program: 38 of the 64.
#[test]
fn thirty_eight_pairs_can_fail() {
    let mut failing = 0;
    for from in TYPES {
        for to in TYPES {
            if !rejected_values(from, to).is_empty() {
                failing += 1;
            }
        }
    }
    assert_eq!(failing, 38);
}

/// `u8(x & 255)` wraps explicitly: 1000 gives 232.
#[test]
fn masked_conversion_wraps_explicitly() {
    let src = main_with(&["    x := 1000", "    print(u8(x & 255))"]);
    assert_eq!(run_ok(&src), "232\n");
}

// ----------------------------------------------------------------------------------------------
// The extension invariant survives the new operations
// ----------------------------------------------------------------------------------------------

/// The results of `~`, `<<` and conversions compare and widen as their types say, so they stay extended.
#[test]
fn results_keep_the_extension_invariant() {
    let src = main_with(&[
        "    a: u8 = 0",
        "    b: i8 = 64",
        "    c: u8 = 128",
        "    d: i8 = -1",
        "    one: u8 = 1",
        "    print(~a == 255)",
        "    print(b << 1 == -128)",
        "    print(c << one == 0)",
        "    print(~a > 254)",
        "    print(d << 7 == -128)",
        "    print(u64(~a))",
        "    print(i64(b << 1))",
    ]);
    assert_eq!(run_ok(&src), "true\ntrue\ntrue\ntrue\ntrue\n255\n-128\n");
}

/// Narrow bitwise results pass through a call and come back unchanged.
#[test]
fn bitwise_results_survive_a_call() {
    let src = "\
fn mix(a: u8, b: i16) -> u8 {
    return (a ^ 255) | u8(b & 15)
}
fn main() {
    print(mix(1, 7))
    print(mix(255, -1))
}
";
    // (1 ^ 255) | 7 = 254 | 7 = 255; (255 ^ 255) | 15 = 15.
    assert_eq!(run_ok(src), "255\n15\n");
}

// ----------------------------------------------------------------------------------------------
// Narrow shifts whose left operand is a constant or a call result
// ----------------------------------------------------------------------------------------------

/// `x: u8 = 1 << n` with `n: i32 = 1` prints 2.
#[test]
fn constant_shifted_into_a_u8_prints_2() {
    let src = main_with(&["    n: i32 = 1", "    x: u8 = 1 << n", "    print(x)"]);
    assert_eq!(run_ok(&src), "2\n");
}

/// `y: i8 = 64 << n` with `n: i32 = 1` prints -128: the result 128 does not fit and is re-extended as an `i8`.
#[test]
fn constant_shifted_into_an_i8_wraps_to_minus_128() {
    let src = main_with(&["    n: i32 = 1", "    y: i8 = 64 << n", "    print(y)"]);
    assert_eq!(run_ok(&src), "-128\n");
}

/// Wide constants shifted into 16-bit types: `255 << 9` is 65024 as a `u16`, `16384 << 1` is -32768 as an `i16`.
#[test]
fn constants_shifted_into_16_bit_types_wrap() {
    let src = main_with(&[
        "    n: i32 = 9",
        "    one: i32 = 1",
        "    a: u16 = 255 << n",
        "    b: i16 = 16384 << one",
        "    print(a)",
        "    print(b)",
    ]);
    assert_eq!(run_ok(&src), "65024\n-32768\n");
}

/// A constant on the left of `>>` keeps its sign: 200 as a `u8` gives 100, -8 as an `i8` gives -4.
#[test]
fn constants_shifted_right_keep_their_sign() {
    let src = main_with(&[
        "    n: i32 = 1",
        "    a: u8 = 200 >> n",
        "    b: i8 = -8 >> n",
        "    print(a)",
        "    print(b)",
    ]);
    assert_eq!(run_ok(&src), "100\n-4\n");
}

/// `f() << n` with `fn f() -> u8 { return 128 }` prints 0: 128 << 1 in a `u8` loses the high bit.
#[test]
fn u8_call_result_shifted_left_prints_0() {
    let src = "\
fn f() -> u8 {
    return 128
}
fn main() {
    n: i32 = 1
    print(f() << n)
}
";
    assert_eq!(run_ok(src), "0\n");
}

/// `g() << n` with `fn g() -> i8 { return 64 }` prints -128.
#[test]
fn i8_call_result_shifted_left_prints_minus_128() {
    let src = "\
fn g() -> i8 {
    return 64
}
fn main() {
    n: i32 = 1
    print(g() << n)
}
";
    assert_eq!(run_ok(src), "-128\n");
}

/// The shifted call result keeps the extension invariant: widening it gives the same number the comparison sees.
#[test]
fn shifted_call_result_widens_correctly() {
    let src = "\
fn f() -> u8 {
    return 129
}
fn g() -> i8 {
    return 65
}
fn main() {
    n: i32 = 1
    print(u64(f() << n))
    print(i64(g() << n))
    print(f() << n == 2)
    print(g() << n == -126)
}
";
    // 129 << 1 = 258, which is 2 as a u8; 65 << 1 = 130, which is -126 as an i8.
    assert_eq!(run_ok(src), "2\n-126\ntrue\ntrue\n");
}
