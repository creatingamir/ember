//! Helpers shared by the `check` integration tests: parsing, checking and locating nodes.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]
// Each test file uses a different subset of these helpers.
#![allow(dead_code)]

use ast::{Block, ElseBranch, Expr, ExprKind, ForKind, NodeId, Param, Program, Stmt, StmtKind};
use check::{Binding, Checked, Type, check};
use source::Span;

/// Parses source text that must be valid Ember syntax.
pub fn parse(src: &str) -> Program {
    parser::parse_source(src).unwrap()
}

/// Parses and checks source text that must be a valid program.
pub fn checked(src: &str) -> Checked {
    check(&parse(src)).unwrap()
}

/// Parses and checks source text that must be rejected; returns each error's message and span.
pub fn errors(src: &str) -> Vec<(String, Span)> {
    check(&parse(src))
        .unwrap_err()
        .into_iter()
        .map(|diagnostic| (diagnostic.message, diagnostic.span))
        .collect()
}

/// Wraps statements into the body of `fn main()`.
pub fn main_with(body: &str) -> String {
    format!("fn main() {{\n{body}\n}}\n")
}

/// Returns the span of the `nth` (0-based) occurrence of `needle` in `src`.
pub fn span_of(src: &str, needle: &str, nth: usize) -> Span {
    let found = src.match_indices(needle).nth(nth);
    assert!(
        found.is_some(),
        "occurrence {nth} of `{needle}` not found in:\n{src}"
    );
    let (start, _) = found.unwrap();
    Span::new(start, start + needle.len())
}

/// Builds an expected error whose span is the `nth` occurrence of `needle` in `src`.
pub fn error(src: &str, message: &str, needle: &str, nth: usize) -> (String, Span) {
    (message.to_string(), span_of(src, needle, nth))
}

/// Asserts that checking `src` yields exactly one error, at the `nth` occurrence of `needle`.
pub fn assert_single_error(src: &str, message: &str, needle: &str, nth: usize) {
    assert_eq!(errors(src), vec![error(src, message, needle, nth)]);
}

/// Every expression, statement and parameter of a program, cloned into flat lists.
struct Nodes {
    exprs: Vec<Expr>,
    stmts: Vec<Stmt>,
    params: Vec<Param>,
}

/// Collects the nodes of a whole program.
fn collect_nodes(program: &Program) -> Nodes {
    let mut nodes = Nodes {
        exprs: Vec::new(),
        stmts: Vec::new(),
        params: Vec::new(),
    };
    for function in &program.functions {
        nodes.params.extend(function.params.iter().cloned());
        walk_block(&function.body, &mut nodes);
    }
    nodes
}

/// Adds every statement of a block, and everything inside them, to `nodes`.
fn walk_block(block: &Block, nodes: &mut Nodes) {
    for stmt in &block.stmts {
        walk_stmt(stmt, nodes);
    }
}

/// Adds a statement and everything inside it to `nodes`.
fn walk_stmt(stmt: &Stmt, nodes: &mut Nodes) {
    nodes.stmts.push(stmt.clone());
    match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::Assign { value, .. } => walk_expr(value, nodes),
        StmtKind::If {
            cond,
            then_block,
            else_branch,
        } => {
            walk_expr(cond, nodes);
            walk_block(then_block, nodes);
            match else_branch {
                Some(ElseBranch::If(nested)) => walk_stmt(nested, nodes),
                Some(ElseBranch::Block(block)) => walk_block(block, nodes),
                None => {}
            }
        }
        StmtKind::For { kind, body } => {
            match kind {
                ForKind::Infinite => {}
                ForKind::While(cond) => walk_expr(cond, nodes),
                ForKind::In { iterable, .. } => walk_expr(iterable, nodes),
            }
            walk_block(body, nodes);
        }
        StmtKind::Return(Some(value)) | StmtKind::Expr(value) => walk_expr(value, nodes),
        StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
    }
}

/// Adds an expression and its sub-expressions to `nodes`.
fn walk_expr(expr: &Expr, nodes: &mut Nodes) {
    nodes.exprs.push(expr.clone());
    match &expr.kind {
        ExprKind::Unary { operand, .. } => walk_expr(operand, nodes),
        ExprKind::Binary { left, right, .. } => {
            walk_expr(left, nodes);
            walk_expr(right, nodes);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                walk_expr(arg, nodes);
            }
        }
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
    }
}

/// A checked program together with its source, for looking up nodes by their text.
pub struct Fixture {
    /// The source text the program was parsed from.
    pub src: String,
    /// The result of checking the program.
    pub checked: Checked,
    nodes: Nodes,
}

/// Parses and checks `src` (which must be valid) and indexes its nodes.
pub fn fixture(src: &str) -> Fixture {
    let program = parse(src);
    Fixture {
        src: src.to_string(),
        checked: check(&program).unwrap(),
        nodes: collect_nodes(&program),
    }
}

impl Fixture {
    /// Returns the id of the only expression spanning the `nth` occurrence of `needle`.
    pub fn expr(&self, needle: &str, nth: usize) -> NodeId {
        let span = span_of(&self.src, needle, nth);
        let matches: Vec<&Expr> = self.nodes.exprs.iter().filter(|e| e.span == span).collect();
        assert_eq!(matches.len(), 1, "no unique expression for `{needle}`");
        matches[0].id
    }

    /// Returns the id of the only statement starting at the `nth` occurrence of `needle`.
    pub fn stmt(&self, needle: &str, nth: usize) -> NodeId {
        let start = span_of(&self.src, needle, nth).start;
        let matches: Vec<&Stmt> = self
            .nodes
            .stmts
            .iter()
            .filter(|s| s.span.start == start)
            .collect();
        assert_eq!(matches.len(), 1, "no unique statement for `{needle}`");
        matches[0].id
    }

    /// Returns the id of the only parameter with this name.
    pub fn param(&self, name: &str) -> NodeId {
        let matches: Vec<&Param> = self
            .nodes
            .params
            .iter()
            .filter(|p| p.name.name == name)
            .collect();
        assert_eq!(matches.len(), 1, "no unique parameter `{name}`");
        matches[0].id
    }

    /// Returns the type recorded for the expression at `needle`.
    pub fn expr_type(&self, needle: &str, nth: usize) -> Type {
        self.checked.expr_types[&self.expr(needle, nth)]
    }

    /// Returns the binding recorded for the name or call expression at `needle`.
    pub fn expr_binding(&self, needle: &str, nth: usize) -> Binding {
        self.checked.bindings[&self.expr(needle, nth)].clone()
    }

    /// Returns the binding recorded for the assignment statement at `needle`.
    pub fn stmt_binding(&self, needle: &str, nth: usize) -> Binding {
        self.checked.bindings[&self.stmt(needle, nth)].clone()
    }

    /// Returns the ids of all expressions that must have a recorded type.
    pub fn all_expr_ids(&self) -> Vec<NodeId> {
        self.nodes.exprs.iter().map(|e| e.id).collect()
    }

    /// Returns the ids of all name and call expressions, which must have a binding.
    pub fn name_and_call_ids(&self) -> Vec<NodeId> {
        self.nodes
            .exprs
            .iter()
            .filter(|e| matches!(e.kind, ExprKind::Name(_) | ExprKind::Call { .. }))
            .map(|e| e.id)
            .collect()
    }

    /// Returns the ids of all assignment statements, which must have a binding.
    pub fn assign_ids(&self) -> Vec<NodeId> {
        self.nodes
            .stmts
            .iter()
            .filter(|s| matches!(s.kind, StmtKind::Assign { .. }))
            .map(|s| s.id)
            .collect()
    }
}
