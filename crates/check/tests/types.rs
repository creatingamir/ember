//! Integration tests for the type table: the built-in ids, interning, names and what a check records.

mod common;

use check::{Checked, TypeId, TypeKind, TypeTable};
use common::checked;

/// Every built-in id with the kind it stands for and its source spelling.
fn builtins() -> Vec<(TypeId, TypeKind, &'static str)> {
    let int = |bits, signed| TypeKind::Int { bits, signed };
    vec![
        (TypeId::UNIT, TypeKind::Unit, "()"),
        (TypeId::BOOL, TypeKind::Bool, "bool"),
        (TypeId::I8, int(8, true), "i8"),
        (TypeId::I16, int(16, true), "i16"),
        (TypeId::I32, int(32, true), "i32"),
        (TypeId::I64, int(64, true), "i64"),
        (TypeId::U8, int(8, false), "u8"),
        (TypeId::U16, int(16, false), "u16"),
        (TypeId::U32, int(32, false), "u32"),
        (TypeId::U64, int(64, false), "u64"),
        (TypeId::UNTYPED_INT, TypeKind::UntypedInt, "i64"),
    ]
}

/// A program that uses `i64`, `bool` and a call to a function without a return type.
const MIXED_PROGRAM: &str = "\
fn quiet(n: i64) {
    print(n)
}
fn truth(n: i64) -> bool {
    return n < 3
}
fn main() {
    count := 1
    flag: bool = truth(count)
    for i in 0..3 {
        quiet(i + count)
    }
    print(flag && !flag)
}
";

/// Every built-in id has the kind listed in the spec.
#[test]
fn builtin_ids_have_their_kinds() {
    let table = TypeTable::new();
    for (id, kind, _) in builtins() {
        assert_eq!(table.kind(id), kind, "{id:?}");
    }
}

/// Every built-in id has its source spelling, and the untyped integer is spelled `i64`.
#[test]
fn builtin_ids_have_their_names() {
    let table = TypeTable::new();
    for (id, _, name) in builtins() {
        assert_eq!(table.name(id), name, "{id:?}");
    }
}

/// `UNTYPED_INT` is named like its default type.
#[test]
fn untyped_int_is_named_i64() {
    assert_eq!(TypeTable::new().name(TypeId::UNTYPED_INT), "i64");
}

/// The eleven built-in ids are all different.
#[test]
fn builtin_ids_are_distinct() {
    let ids: Vec<TypeId> = builtins().into_iter().map(|(id, _, _)| id).collect();
    for (index, id) in ids.iter().enumerate() {
        assert!(!ids[index + 1..].contains(id), "{id:?} appears twice");
    }
}

/// Interning a built-in kind returns the id of the constant.
#[test]
fn interning_a_builtin_kind_returns_its_constant() {
    let mut table = TypeTable::new();
    for (id, kind, _) in builtins() {
        assert_eq!(table.intern(kind), id, "{kind:?}");
    }
}

/// Interning a built-in kind adds nothing to the table.
#[test]
fn interning_a_builtin_kind_leaves_the_table_unchanged() {
    let mut table = TypeTable::new();
    for (_, kind, _) in builtins() {
        table.intern(kind);
    }
    assert_eq!(table, TypeTable::new());
}

/// A new kind gets a new id once; asking again returns the same id, and the table can say what it is.
#[test]
fn interning_a_new_kind_adds_it_once() {
    let mut table = TypeTable::new();
    let wide = TypeKind::Int {
        bits: 128,
        signed: true,
    };
    let first = table.intern(wide);
    let second = table.intern(wide);
    assert_eq!(first, second);
    assert!(builtins().iter().all(|(id, _, _)| *id != first));
    assert_eq!(table.kind(first), wide);
}

/// A new kind does not change the built-in ids.
#[test]
fn interning_a_new_kind_keeps_the_builtin_ids() {
    let mut table = TypeTable::new();
    table.intern(TypeKind::Int {
        bits: 128,
        signed: false,
    });
    for (id, kind, _) in builtins() {
        assert_eq!(table.kind(id), kind, "{id:?}");
    }
}

/// `Default` is the same table as `new`.
#[test]
fn default_table_is_the_new_table() {
    assert_eq!(TypeTable::default(), TypeTable::new());
}

/// `Checked::default()` holds the built-in table (the IR tests lower against it).
#[test]
fn default_checked_has_the_builtin_table() {
    assert_eq!(Checked::default().types, TypeTable::new());
}

/// Checking a program that uses only `i64` and `bool` adds no type to the table.
#[test]
fn checking_adds_no_types_in_this_milestone() {
    assert_eq!(checked(MIXED_PROGRAM).types, TypeTable::new());
}

/// Two checks of the same program give the same table and the same type for every node.
#[test]
fn checking_twice_gives_the_same_types() {
    let first = checked(MIXED_PROGRAM);
    let second = checked(MIXED_PROGRAM);
    assert_eq!(first.types, second.types);
    assert_eq!(first.signatures, second.signatures);
    assert_eq!(first.expr_types, second.expr_types);
    assert_eq!(first.local_types, second.local_types);
}

/// Every type recorded for an expression is `i64`, `bool` or `()`.
#[test]
fn expression_types_are_only_i64_bool_or_unit() {
    let allowed = [TypeId::I64, TypeId::BOOL, TypeId::UNIT];
    let result = checked(MIXED_PROGRAM);
    assert!(!result.expr_types.is_empty());
    for (node, id) in &result.expr_types {
        assert!(allowed.contains(id), "{node:?} has {id:?}");
    }
}

/// Every type recorded for a parameter, `let` or loop variable is `i64` or `bool`.
#[test]
fn local_types_are_only_i64_or_bool() {
    let allowed = [TypeId::I64, TypeId::BOOL];
    let result = checked(MIXED_PROGRAM);
    assert!(!result.local_types.is_empty());
    for (node, id) in &result.local_types {
        assert!(allowed.contains(id), "{node:?} has {id:?}");
    }
}

/// The call to a function without a return type has type `()`, and the program records both other types too.
#[test]
fn the_unit_call_is_recorded_as_unit() {
    let result = checked(MIXED_PROGRAM);
    let recorded: Vec<TypeId> = result.expr_types.values().copied().collect();
    for id in [TypeId::I64, TypeId::BOOL, TypeId::UNIT] {
        assert!(recorded.contains(&id), "{id:?} not recorded");
    }
}

/// Signatures use ids: `ret` is `UNIT` for a function without a return type.
#[test]
fn signatures_use_type_ids() {
    let result = checked(MIXED_PROGRAM);
    assert_eq!(result.signatures["quiet"].params, vec![TypeId::I64]);
    assert_eq!(result.signatures["quiet"].ret, TypeId::UNIT);
    assert_eq!(result.signatures["truth"].ret, TypeId::BOOL);
}
