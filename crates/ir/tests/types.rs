//! Integration tests for the IR's own types: widths, sizes and how a type is written.

use ir::IrType;

/// The four IR types with their width in bits, size in bytes and spelling.
const TYPES: [(IrType, u32, u32, &str); 4] = [
    (IrType::I8, 8, 1, "i8"),
    (IrType::I16, 16, 2, "i16"),
    (IrType::I32, 32, 4, "i32"),
    (IrType::I64, 64, 8, "i64"),
];

/// `bits` is the width of each type.
#[test]
fn bits_are_the_width() {
    for (ty, bits, _, _) in TYPES {
        assert_eq!(ty.bits(), bits, "{ty}");
    }
}

/// `bytes` is the size of each type, which is also its alignment.
#[test]
fn bytes_are_the_size() {
    for (ty, _, bytes, _) in TYPES {
        assert_eq!(ty.bytes(), bytes, "{ty}");
    }
}

/// A type is eight times as many bits as bytes.
#[test]
fn bits_are_eight_times_bytes() {
    for (ty, _, _, _) in TYPES {
        assert_eq!(ty.bits(), 8 * ty.bytes(), "{ty}");
    }
}

/// `Display` writes `i8`, `i16`, `i32` and `i64`.
#[test]
fn display_writes_the_type_name() {
    for (ty, _, _, name) in TYPES {
        assert_eq!(ty.to_string(), name);
    }
}

/// The types are copyable and compare by value.
#[test]
fn types_compare_by_value() {
    let copy = IrType::I32;
    assert_eq!(copy, IrType::I32);
    assert_ne!(copy, IrType::I64);
}
