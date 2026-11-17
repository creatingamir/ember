//! Deterministic fuzz tests: the parser never panics, and every program it accepts is consistent.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use ast::{
    Block, ElseBranch, Expr, ExprKind, ForKind, Function, NodeId, Program, Stmt, StmtKind,
    print_program,
};
use parser::parse_source;
use source::Span;

// ---------------------------------------------------------------- random numbers

/// A xorshift64 pseudo-random generator, so every run sees the same inputs.
struct Rng {
    state: u64,
}

impl Rng {
    /// Starts the generator from a fixed non-zero seed.
    fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// Returns the next 64 random bits.
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Returns a number in `0..limit`.
    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

// ---------------------------------------------------------------- input generators

/// Ember fragments that random token soups are made of.
const FRAGMENTS: &[&str] = &[
    "fn", "struct", "pub", "if", "else", "for", "in", "return", "break", "continue", "match",
    "defer", "arena", "grad", "on", "none", "true", "false", "x", "y", "f", "main", "i64", "0",
    "1", "42", "0xff", "1.5", "\"s\"", "(", ")", "(", ")", "{", "}", "{", "}", "[", "]", ",", ":",
    ":=", "=", "+=", "@=", "->", "=>", "?", ".", "..", "..=", "+", "-", "*", "/", "%", "@", "!",
    "~", "&", "|", "^", "<<", ">>", "==", "!=", "<", "<=", ">", ">=", "&&", "||", "\n", "\n", "\n",
    "// c",
];

/// Builds a random sequence of fragments, often wrapped in `fn main() { ... }`.
fn random_soup(rng: &mut Rng) -> String {
    let wrapped = rng.below(2) == 0;
    let count = 1 + rng.below(60);
    let mut text = String::new();
    if wrapped {
        text.push_str("fn main() {\n");
    }
    for _ in 0..count {
        text.push_str(FRAGMENTS[rng.below(FRAGMENTS.len())]);
        text.push_str([" ", "\n", ""][rng.below(3)]);
    }
    if wrapped {
        text.push_str("\n}\n");
    }
    text
}

/// Builds one of six deeply nested shapes with up to 300 levels, sometimes broken by a stray token.
fn random_deep_source(rng: &mut Rng) -> String {
    let levels = 1 + rng.below(300);
    let stray = ["", "", ")", "}", "1.5", "\n"][rng.below(6)];
    let body = match rng.below(6) {
        0 => format!("x := {}x{}{stray}", "(".repeat(levels), ")".repeat(levels)),
        1 => format!("x := {}x{stray}", "-".repeat(levels)),
        2 => format!("x := {}x{}{stray}", "f(".repeat(levels), ")".repeat(levels)),
        3 => format!(
            "{}x := 1{stray}\n{}",
            "if a {\n".repeat(levels),
            "}\n".repeat(levels)
        ),
        4 => format!("if a {{\n}}{}{stray}", " else if a {\n}".repeat(levels)),
        _ => format!(
            "{}break{stray}\n{}",
            "for {\n".repeat(levels),
            "}\n".repeat(levels)
        ),
    };
    format!("fn main() {{\n{body}\n}}\n")
}

// ---------------------------------------------------------------- consistency walker

/// Collects the id and span of every node of a program.
#[derive(Default)]
struct Nodes {
    ids: Vec<NodeId>,
    spans: Vec<Span>,
}

impl Nodes {
    /// Records one node.
    fn add(&mut self, id: NodeId, span: Span) {
        self.ids.push(id);
        self.spans.push(span);
    }

    /// Records a function and everything inside it.
    fn function(&mut self, function: &Function) {
        self.add(function.id, function.span);
        self.spans.push(function.name.span);
        for param in &function.params {
            self.add(param.id, param.span);
            self.add(param.ty.id, param.ty.span);
        }
        if let Some(return_type) = &function.return_type {
            self.add(return_type.id, return_type.span);
        }
        self.block(&function.body);
    }

    /// Records a block and its statements.
    fn block(&mut self, block: &Block) {
        self.add(block.id, block.span);
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
    }

    /// Records a statement and everything inside it.
    fn stmt(&mut self, stmt: &Stmt) {
        self.add(stmt.id, stmt.span);
        match &stmt.kind {
            StmtKind::Let { name, ty, value } => {
                self.spans.push(name.span);
                if let Some(ty) = ty {
                    self.add(ty.id, ty.span);
                }
                self.expr(value);
            }
            StmtKind::Assign { target, value, .. } => {
                self.spans.push(target.span);
                self.expr(value);
            }
            StmtKind::If {
                cond,
                then_block,
                else_branch,
            } => {
                self.expr(cond);
                self.block(then_block);
                match else_branch {
                    Some(ElseBranch::If(nested)) => self.stmt(nested),
                    Some(ElseBranch::Block(block)) => self.block(block),
                    None => {}
                }
            }
            StmtKind::For { kind, body } => {
                match kind {
                    ForKind::Infinite => {}
                    ForKind::While(cond) => self.expr(cond),
                    ForKind::In { var, iterable } => {
                        self.spans.push(var.span);
                        self.expr(iterable);
                    }
                }
                self.block(body);
            }
            StmtKind::Return(Some(value)) => self.expr(value),
            StmtKind::Return(None) | StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(expr) => self.expr(expr),
        }
    }

    /// Records an expression and its sub-expressions.
    fn expr(&mut self, expr: &Expr) {
        self.add(expr.id, expr.span);
        match &expr.kind {
            ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
            ExprKind::Unary { operand, .. } => self.expr(operand),
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Call { callee, args } => {
                self.spans.push(callee.span);
                for arg in args {
                    self.expr(arg);
                }
            }
        }
    }
}

/// Collects every node of a program.
fn collect_nodes(program: &Program) -> Nodes {
    let mut nodes = Nodes::default();
    for function in &program.functions {
        nodes.function(function);
    }
    nodes
}

/// Parses text, which must not panic, and checks the invariants of an accepted program.
fn check_parse(text: &str) {
    let Ok(program) = parse_source(text) else {
        return;
    };
    let mut nodes = collect_nodes(&program);

    let id_count = nodes.ids.len();
    nodes.ids.sort();
    nodes.ids.dedup();
    assert_eq!(nodes.ids.len(), id_count, "duplicate NodeId in {text:?}");

    for span in &nodes.spans {
        let inside = span.start <= span.end && span.end <= text.len();
        assert!(inside, "span {span:?} outside the text in {text:?}");
    }
}

// ---------------------------------------------------------------- tests

/// 18,000 random token soups never panic, and accepted ones have unique ids and in-range spans.
#[test]
fn random_token_soups_never_panic() {
    let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);
    for _ in 0..18_000 {
        check_parse(&random_soup(&mut rng));
    }
}

/// 2,000 deeply nested inputs (up to 300 levels, on both sides of the limit) never panic.
#[test]
fn deep_nesting_never_panics() {
    let mut rng = Rng::new(0xD1B5_4A32_D192_ED03);
    for _ in 0..2_000 {
        check_parse(&random_deep_source(&mut rng));
    }
}

// ---------------------------------------------------------------- small-stack regression

/// The stack size of a default test thread; the parser must fit in it.
const SMALL_STACK_BYTES: usize = 2 * 1024 * 1024;

/// One precedence level of operators, ending in an open group or call, repeated to nest deeply.
const LADDER_STEP: &str = "1 || 1 && 1 == 1 .. 1 + 1 * 1 @ ";

/// Source with `levels` ladder steps, each nested in a parenthesized group.
fn paren_ladder_source(levels: usize) -> String {
    let open = format!("({LADDER_STEP}").repeat(levels);
    let close = ")".repeat(levels);
    format!("fn main() {{\n    x := {open}1{close}\n}}\n")
}

/// Source with `levels` ladder steps, each nested in the argument of a call.
fn call_ladder_source(levels: usize) -> String {
    let open = format!("f({LADDER_STEP}").repeat(levels);
    let close = ")".repeat(levels);
    format!("fn main() {{\n    x := {open}1{close}\n}}\n")
}

/// Runs `work` on a thread with a 2 MiB stack and asserts that it neither panicked nor aborted.
fn assert_fits_in_small_stack(work: impl FnOnce() + Send + 'static) {
    let handle = std::thread::Builder::new()
        .stack_size(SMALL_STACK_BYTES)
        .spawn(work)
        .unwrap();
    assert!(handle.join().is_ok(), "the work thread panicked");
}

/// Parses source; an accepted program is also printed, cloned, compared and dropped.
fn parse_and_walk(src: &str) {
    let Ok(program) = parse_source(src) else {
        return;
    };
    let printed = print_program(&program);
    assert!(printed.starts_with("fn main()"));
    let copy = program.clone();
    assert_eq!(copy, program);
    drop(copy);
    drop(program);
}

/// Parses source that is far too deep and asserts that it gives an error instead of aborting.
fn parse_too_deep(src: &str) {
    assert!(parse_source(src).is_err());
}

/// A paren ladder at the nesting limit fits in a 2 MiB stack for parse, print, clone, compare and drop.
#[test]
fn paren_ladder_at_the_limit_fits_in_a_small_stack() {
    assert_fits_in_small_stack(|| parse_and_walk(&paren_ladder_source(64)));
}

/// A call ladder at the nesting limit fits in a 2 MiB stack for parse, print, clone, compare and drop.
#[test]
fn call_ladder_at_the_limit_fits_in_a_small_stack() {
    assert_fits_in_small_stack(|| parse_and_walk(&call_ladder_source(64)));
}

/// A paren ladder of 10,000 levels is an error, not a stack overflow.
#[test]
fn paren_ladder_of_10000_levels_is_an_error() {
    assert_fits_in_small_stack(|| parse_too_deep(&paren_ladder_source(10_000)));
}

/// A call ladder of 10,000 levels is an error, not a stack overflow.
#[test]
fn call_ladder_of_10000_levels_is_an_error() {
    assert_fits_in_small_stack(|| parse_too_deep(&call_ladder_source(10_000)));
}
