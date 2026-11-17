//! The type table: every type of a program is stored once and named by a small number (decision 0028).

use std::collections::HashMap;

/// A type, as a small number that indexes the program's `TypeTable`; equal types have equal ids.
///
/// The field is private, so an id only comes from a table or from the constants below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(u32);

impl TypeId {
    /// `()`, the result of a function without a return type.
    pub const UNIT: TypeId = TypeId(0);
    /// `bool`.
    pub const BOOL: TypeId = TypeId(1);
    /// `i8`.
    pub const I8: TypeId = TypeId(2);
    /// `i16`.
    pub const I16: TypeId = TypeId(3);
    /// `i32`.
    pub const I32: TypeId = TypeId(4);
    /// `i64`.
    pub const I64: TypeId = TypeId(5);
    /// `u8`.
    pub const U8: TypeId = TypeId(6);
    /// `u16`.
    pub const U16: TypeId = TypeId(7);
    /// `u32`.
    pub const U32: TypeId = TypeId(8);
    /// `u64`.
    pub const U64: TypeId = TypeId(9);
    /// An integer literal (or an expression of literals) that has not taken its type yet; checker-only.
    pub const UNTYPED_INT: TypeId = TypeId(10);
}

/// What a type is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// `()`.
    Unit,
    /// `bool`.
    Bool,
    /// A fixed-width integer.
    Int {
        /// The width in bits.
        bits: u8,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// An integer constant whose type comes from its context (decision 0033 7c).
    UntypedInt,
}

/// The kinds of the built-in types, in the order of the `TypeId` constants.
const BUILTIN_KINDS: [TypeKind; 11] = [
    TypeKind::Unit,
    TypeKind::Bool,
    TypeKind::Int {
        bits: 8,
        signed: true,
    },
    TypeKind::Int {
        bits: 16,
        signed: true,
    },
    TypeKind::Int {
        bits: 32,
        signed: true,
    },
    TypeKind::Int {
        bits: 64,
        signed: true,
    },
    TypeKind::Int {
        bits: 8,
        signed: false,
    },
    TypeKind::Int {
        bits: 16,
        signed: false,
    },
    TypeKind::Int {
        bits: 32,
        signed: false,
    },
    TypeKind::Int {
        bits: 64,
        signed: false,
    },
    TypeKind::UntypedInt,
];

/// Every type of a program, each stored once (decision 0028).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeTable {
    /// The kind of each id: `kinds[id]`.
    kinds: Vec<TypeKind>,
    /// The id of each kind, so a kind is never added twice.
    ids: HashMap<TypeKind, TypeId>,
}

impl TypeTable {
    /// A table holding the built-in types at the positions of the `TypeId` constants.
    pub fn new() -> TypeTable {
        let mut table = TypeTable {
            kinds: Vec::new(),
            ids: HashMap::new(),
        };
        for kind in BUILTIN_KINDS {
            table.intern(kind);
        }
        table
    }

    /// The id of `kind`, adding it when it is new.
    ///
    /// Ids are `u32`. A program has far fewer than 2^32 distinct types: every one comes from a
    /// declaration or a written type, and the parser limits how many nodes a program can hold.
    pub fn intern(&mut self, kind: TypeKind) -> TypeId {
        if let Some(id) = self.ids.get(&kind) {
            return *id;
        }
        let id = TypeId(self.kinds.len() as u32);
        self.kinds.push(kind);
        self.ids.insert(kind, id);
        id
    }

    /// What `id` is.
    ///
    /// The built-in ids sit at the same positions in every table, so they are valid in any table.
    /// Panics if `id` comes from another table that interned more types (decision 0049, number 14).
    pub fn kind(&self, id: TypeId) -> TypeKind {
        self.kinds[id.0 as usize]
    }

    /// The source spelling: `i64`, `u8`, `bool`, `()`. An untyped integer is spelled `i64`, its default.
    pub fn name(&self, id: TypeId) -> String {
        match self.kind(id) {
            TypeKind::Unit => "()".to_string(),
            TypeKind::Bool => "bool".to_string(),
            TypeKind::Int { bits, signed } => {
                let letter = if signed { 'i' } else { 'u' };
                format!("{letter}{bits}")
            }
            TypeKind::UntypedInt => "i64".to_string(),
        }
    }
}

impl Default for TypeTable {
    /// The table of built-in types.
    fn default() -> TypeTable {
        TypeTable::new()
    }
}
