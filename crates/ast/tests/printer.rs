//! Tests for `print_program` and the `Display` spellings of the operator enums.

// Test helpers are plain functions, which clippy's allow-unwrap-in-tests does not cover.
#![allow(clippy::unwrap_used)]

use std::cell::Cell;

use ast::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ExprKind, ForKind, Function, Ident, NodeId, Param,
    Program, Stmt, StmtKind, TypeExpr, UnaryOp, print_program,
};
use source::Span;

// ---------------------------------------------------------------- helpers

/// Builds AST nodes by hand, handing out unique ids and a dummy span.
struct Builder {
    next_id: Cell<u32>,
}

impl Builder {
    /// Starts a builder whose first node gets id 0.
    fn new() -> Builder {
        Builder {
            next_id: Cell::new(0),
        }
    }

    /// Hands out the next unused node id.
    fn id(&self) -> NodeId {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        NodeId(id)
    }

    /// The span every hand-built node gets; the printer ignores spans.
    fn span(&self) -> Span {
        Span::new(0, 0)
    }

    /// Builds an identifier.
    fn ident(&self, name: &str) -> Ident {
        Ident {
            name: name.to_string(),
            span: self.span(),
        }
    }

    /// Builds a named type.
    fn ty(&self, name: &str) -> TypeExpr {
        TypeExpr {
            id: self.id(),
            name: self.ident(name),
            span: self.span(),
        }
    }

    /// Builds a function parameter `name: ty`.
    fn param(&self, name: &str, ty: &str) -> Param {
        Param {
            id: self.id(),
            name: self.ident(name),
            ty: self.ty(ty),
            span: self.span(),
        }
    }

    /// Builds an expression from its kind.
    fn expr(&self, kind: ExprKind) -> Expr {
        Expr {
            id: self.id(),
            kind,
            span: self.span(),
        }
    }

    /// Builds an integer literal.
    fn int(&self, value: u64) -> Expr {
        self.expr(ExprKind::Int(value))
    }

    /// Builds a boolean literal.
    fn boolean(&self, value: bool) -> Expr {
        self.expr(ExprKind::Bool(value))
    }

    /// Builds a name expression.
    fn name(&self, name: &str) -> Expr {
        self.expr(ExprKind::Name(name.to_string()))
    }

    /// Builds a unary expression.
    fn unary(&self, op: UnaryOp, operand: Expr) -> Expr {
        self.expr(ExprKind::Unary {
            op,
            operand: Box::new(operand),
        })
    }

    /// Builds a binary expression.
    fn binary(&self, op: BinaryOp, left: Expr, right: Expr) -> Expr {
        self.expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    /// Builds a call expression.
    fn call(&self, callee: &str, args: Vec<Expr>) -> Expr {
        self.expr(ExprKind::Call {
            callee: self.ident(callee),
            args,
        })
    }

    /// Builds a statement from its kind.
    fn stmt(&self, kind: StmtKind) -> Stmt {
        Stmt {
            id: self.id(),
            kind,
            span: self.span(),
        }
    }

    /// Builds an untyped let statement `name := value`.
    fn let_stmt(&self, name: &str, value: Expr) -> Stmt {
        self.stmt(StmtKind::Let {
            name: self.ident(name),
            ty: None,
            value,
        })
    }

    /// Builds an assignment `target op value`.
    fn assign(&self, target: &str, op: AssignOp, value: Expr) -> Stmt {
        self.stmt(StmtKind::Assign {
            target: self.ident(target),
            op,
            value,
        })
    }

    /// Builds an expression statement.
    fn expr_stmt(&self, expr: Expr) -> Stmt {
        self.stmt(StmtKind::Expr(expr))
    }

    /// Builds a block holding the given statements.
    fn block(&self, stmts: Vec<Stmt>) -> Block {
        Block {
            id: self.id(),
            stmts,
            span: self.span(),
        }
    }

    /// Builds an `if` statement with no else branch.
    fn if_stmt(&self, cond: Expr, then_stmts: Vec<Stmt>) -> Stmt {
        let then_block = self.block(then_stmts);
        self.stmt(StmtKind::If {
            cond,
            then_block,
            else_branch: None,
        })
    }

    /// Builds an `if` statement with an `else { ... }` block.
    fn if_else_stmt(&self, cond: Expr, then_stmts: Vec<Stmt>, else_stmts: Vec<Stmt>) -> Stmt {
        let then_block = self.block(then_stmts);
        let else_block = self.block(else_stmts);
        self.stmt(StmtKind::If {
            cond,
            then_block,
            else_branch: Some(ElseBranch::Block(else_block)),
        })
    }

    /// Builds an `if` statement whose else branch is the given nested `if` statement.
    fn if_else_if_stmt(&self, cond: Expr, then_stmts: Vec<Stmt>, nested_if: Stmt) -> Stmt {
        let then_block = self.block(then_stmts);
        self.stmt(StmtKind::If {
            cond,
            then_block,
            else_branch: Some(ElseBranch::If(Box::new(nested_if))),
        })
    }

    /// Builds a `for` statement of the given kind.
    fn for_stmt(&self, kind: ForKind, body_stmts: Vec<Stmt>) -> Stmt {
        let body = self.block(body_stmts);
        self.stmt(StmtKind::For { kind, body })
    }

    /// Builds a function with the given header parts and body statements.
    fn function(
        &self,
        name: &str,
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        stmts: Vec<Stmt>,
    ) -> Function {
        Function {
            id: self.id(),
            name: self.ident(name),
            params,
            return_type,
            body: self.block(stmts),
            span: self.span(),
        }
    }
}

/// Prints a program made of one `fn main()` holding the given statements.
fn print_main(builder: &Builder, stmts: Vec<Stmt>) -> String {
    let main = builder.function("main", vec![], None, stmts);
    print_program(&Program {
        functions: vec![main],
    })
}

/// Prints one expression as the value of a `let x` statement and returns just the expression text.
fn print_expr(builder: &Builder, expr: Expr) -> String {
    let stmt = builder.let_stmt("x", expr);
    let printed = print_main(builder, vec![stmt]);
    let expression = printed.strip_prefix("fn main()\n  let x = ").unwrap();
    expression.strip_suffix('\n').unwrap().to_string()
}

/// All unary operators with their source spelling.
fn unary_table() -> Vec<(UnaryOp, &'static str)> {
    vec![
        (UnaryOp::Neg, "-"),
        (UnaryOp::Not, "!"),
        (UnaryOp::BitNot, "~"),
        (UnaryOp::AddrOf, "&"),
        (UnaryOp::Deref, "*"),
    ]
}

/// All binary operators with their source spelling.
fn binary_table() -> Vec<(BinaryOp, &'static str)> {
    vec![
        (BinaryOp::MatMul, "@"),
        (BinaryOp::Mul, "*"),
        (BinaryOp::Div, "/"),
        (BinaryOp::Rem, "%"),
        (BinaryOp::Shl, "<<"),
        (BinaryOp::Shr, ">>"),
        (BinaryOp::BitAnd, "&"),
        (BinaryOp::Add, "+"),
        (BinaryOp::Sub, "-"),
        (BinaryOp::BitOr, "|"),
        (BinaryOp::BitXor, "^"),
        (BinaryOp::Range, ".."),
        (BinaryOp::RangeInclusive, "..="),
        (BinaryOp::Eq, "=="),
        (BinaryOp::Ne, "!="),
        (BinaryOp::Lt, "<"),
        (BinaryOp::Le, "<="),
        (BinaryOp::Gt, ">"),
        (BinaryOp::Ge, ">="),
        (BinaryOp::And, "&&"),
        (BinaryOp::Or, "||"),
    ]
}

/// All assignment operators with their source spelling.
fn assign_table() -> Vec<(AssignOp, &'static str)> {
    vec![
        (AssignOp::Assign, "="),
        (AssignOp::Add, "+="),
        (AssignOp::Sub, "-="),
        (AssignOp::Mul, "*="),
        (AssignOp::Div, "/="),
        (AssignOp::MatMul, "@="),
    ]
}

// ---------------------------------------------------------------- Display of operators

/// The unary operator table lists all 5 operators.
#[test]
fn unary_table_has_5_entries() {
    assert_eq!(unary_table().len(), 5);
}

/// The binary operator table lists all 21 operators.
#[test]
fn binary_table_has_21_entries() {
    assert_eq!(binary_table().len(), 21);
}

/// The assignment operator table lists all 6 operators.
#[test]
fn assign_table_has_6_entries() {
    assert_eq!(assign_table().len(), 6);
}

/// Every unary operator displays as its source spelling.
#[test]
fn unary_op_displays_as_its_source_spelling() {
    for (op, spelling) in unary_table() {
        assert_eq!(op.to_string(), spelling);
    }
}

/// Every binary operator displays as its source spelling.
#[test]
fn binary_op_displays_as_its_source_spelling() {
    for (op, spelling) in binary_table() {
        assert_eq!(op.to_string(), spelling);
    }
}

/// Every assignment operator displays as its source spelling.
#[test]
fn assign_op_displays_as_its_source_spelling() {
    for (op, spelling) in assign_table() {
        assert_eq!(op.to_string(), spelling);
    }
}

// ---------------------------------------------------------------- program and function headers

/// A program with no functions prints nothing.
#[test]
fn empty_program_prints_nothing() {
    let program = Program { functions: vec![] };
    assert_eq!(print_program(&program), "");
}

/// A function with no parameters and no return type prints `fn main()`.
#[test]
fn function_without_params_or_return_type() {
    let builder = Builder::new();
    assert_eq!(print_main(&builder, vec![]), "fn main()\n");
}

/// One parameter prints as `name: Type`.
#[test]
fn function_with_one_param() {
    let builder = Builder::new();
    let params = vec![builder.param("n", "i64")];
    let function = builder.function("f", params, None, vec![]);
    let program = Program {
        functions: vec![function],
    };
    assert_eq!(print_program(&program), "fn f(n: i64)\n");
}

/// Several parameters are separated by a comma and a space.
#[test]
fn function_with_several_params() {
    let builder = Builder::new();
    let params = vec![
        builder.param("a", "i64"),
        builder.param("b", "f32"),
        builder.param("c", "bool"),
    ];
    let function = builder.function("f", params, None, vec![]);
    let program = Program {
        functions: vec![function],
    };
    assert_eq!(print_program(&program), "fn f(a: i64, b: f32, c: bool)\n");
}

/// A return type prints as ` -> Type` after the parameter list.
#[test]
fn function_with_return_type_and_no_params() {
    let builder = Builder::new();
    let return_type = Some(builder.ty("i64"));
    let function = builder.function("answer", vec![], return_type, vec![]);
    let program = Program {
        functions: vec![function],
    };
    assert_eq!(print_program(&program), "fn answer() -> i64\n");
}

/// Parameters and a return type print together.
#[test]
fn function_with_params_and_return_type() {
    let builder = Builder::new();
    let params = vec![builder.param("a", "i64"), builder.param("b", "i64")];
    let return_type = Some(builder.ty("i64"));
    let function = builder.function("add", params, return_type, vec![]);
    let program = Program {
        functions: vec![function],
    };
    assert_eq!(print_program(&program), "fn add(a: i64, b: i64) -> i64\n");
}

/// Functions are printed one after the other with nothing between them.
#[test]
fn two_functions_print_back_to_back() {
    let builder = Builder::new();
    let first_body = vec![builder.stmt(StmtKind::Break)];
    let first = builder.function("first", vec![], None, first_body);
    let second_body = vec![builder.stmt(StmtKind::Continue)];
    let second = builder.function("second", vec![], None, second_body);
    let program = Program {
        functions: vec![first, second],
    };
    let expected = "fn first()\n  break\nfn second()\n  continue\n";
    assert_eq!(print_program(&program), expected);
}

// ---------------------------------------------------------------- simple statements

/// An untyped let prints as `let x = EXPR`.
#[test]
fn let_without_type() {
    let builder = Builder::new();
    let stmt = builder.let_stmt("x", builder.int(1));
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  let x = 1\n");
}

/// A typed let prints as `let x: T = EXPR`.
#[test]
fn let_with_type() {
    let builder = Builder::new();
    let stmt = builder.stmt(StmtKind::Let {
        name: builder.ident("x"),
        ty: Some(builder.ty("i64")),
        value: builder.int(1),
    });
    let expected = "fn main()\n  let x: i64 = 1\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// Every assignment operator prints as `target OP value`.
#[test]
fn assign_with_every_operator() {
    for (op, spelling) in assign_table() {
        let builder = Builder::new();
        let stmt = builder.assign("x", op, builder.name("y"));
        let expected = format!("fn main()\n  x {spelling} y\n");
        assert_eq!(
            print_main(&builder, vec![stmt]),
            expected,
            "op `{spelling}`"
        );
    }
}

/// A bare `return` prints as `return`.
#[test]
fn return_without_value() {
    let builder = Builder::new();
    let stmt = builder.stmt(StmtKind::Return(None));
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  return\n");
}

/// A `return` with a value prints as `return EXPR`.
#[test]
fn return_with_value() {
    let builder = Builder::new();
    let stmt = builder.stmt(StmtKind::Return(Some(builder.int(7))));
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  return 7\n");
}

/// `break` prints as `break`.
#[test]
fn break_statement() {
    let builder = Builder::new();
    let stmt = builder.stmt(StmtKind::Break);
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  break\n");
}

/// `continue` prints as `continue`.
#[test]
fn continue_statement() {
    let builder = Builder::new();
    let stmt = builder.stmt(StmtKind::Continue);
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  continue\n");
}

/// An expression statement prints as just its expression.
#[test]
fn expression_statement() {
    let builder = Builder::new();
    let call = builder.call("print", vec![builder.name("x")]);
    let stmt = builder.expr_stmt(call);
    let expected = "fn main()\n  (call print x)\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// Several statements print one per line in order, at the same indentation.
#[test]
fn several_statements_print_in_order() {
    let builder = Builder::new();
    let stmts = vec![
        builder.let_stmt("a", builder.int(1)),
        builder.let_stmt("b", builder.int(2)),
        builder.assign("a", AssignOp::Add, builder.name("b")),
    ];
    let expected = "fn main()\n  let a = 1\n  let b = 2\n  a += b\n";
    assert_eq!(print_main(&builder, stmts), expected);
}

// ---------------------------------------------------------------- if / else

/// An `if` prints its condition, then the then-block indented.
#[test]
fn if_without_else() {
    let builder = Builder::new();
    let then_stmts = vec![builder.assign("x", AssignOp::Assign, builder.int(1))];
    let stmt = builder.if_stmt(builder.name("a"), then_stmts);
    let expected = "fn main()\n  if a\n    x = 1\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// An `if` with an empty then-block prints only the `if` line.
#[test]
fn if_with_empty_block_prints_only_the_condition() {
    let builder = Builder::new();
    let stmt = builder.if_stmt(builder.name("a"), vec![]);
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  if a\n");
}

/// An else block prints as an `else` line at the `if`'s indentation, then its statements indented.
#[test]
fn if_with_else_block() {
    let builder = Builder::new();
    let then_stmts = vec![builder.assign("x", AssignOp::Assign, builder.int(1))];
    let else_stmts = vec![builder.assign("x", AssignOp::Assign, builder.int(2))];
    let stmt = builder.if_else_stmt(builder.name("a"), then_stmts, else_stmts);
    let expected = "fn main()\n  if a\n    x = 1\n  else\n    x = 2\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// An empty else block prints only the `else` line.
#[test]
fn if_with_empty_else_block_prints_only_else() {
    let builder = Builder::new();
    let then_stmts = vec![builder.stmt(StmtKind::Break)];
    let stmt = builder.if_else_stmt(builder.name("a"), then_stmts, vec![]);
    let expected = "fn main()\n  if a\n    break\n  else\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// An else-if prints the nested `if` one level deeper under `else`.
#[test]
fn else_if_nests_the_inner_if_under_else() {
    let builder = Builder::new();
    let inner = builder.if_stmt(
        builder.name("b"),
        vec![builder.assign("x", AssignOp::Assign, builder.int(2))],
    );
    let outer_then = vec![builder.assign("x", AssignOp::Assign, builder.int(1))];
    let outer = builder.if_else_if_stmt(builder.name("a"), outer_then, inner);
    let expected = "fn main()\n  if a\n    x = 1\n  else\n    if b\n      x = 2\n";
    assert_eq!(print_main(&builder, vec![outer]), expected);
}

/// An else-if-else chain nests each link one level deeper than the previous one.
#[test]
fn else_if_else_chain_nests_each_link_deeper() {
    let builder = Builder::new();
    let last = builder.if_else_stmt(
        builder.name("b"),
        vec![builder.assign("x", AssignOp::Assign, builder.int(2))],
        vec![builder.assign("x", AssignOp::Assign, builder.int(3))],
    );
    let first_then = vec![builder.assign("x", AssignOp::Assign, builder.int(1))];
    let first = builder.if_else_if_stmt(builder.name("a"), first_then, last);
    let expected = concat!(
        "fn main()\n",
        "  if a\n",
        "    x = 1\n",
        "  else\n",
        "    if b\n",
        "      x = 2\n",
        "    else\n",
        "      x = 3\n",
    );
    assert_eq!(print_main(&builder, vec![first]), expected);
}

/// A three-link else-if chain keeps nesting deeper.
#[test]
fn three_link_else_if_chain() {
    let builder = Builder::new();
    let third = builder.if_stmt(builder.name("c"), vec![builder.stmt(StmtKind::Continue)]);
    let second = builder.if_else_if_stmt(
        builder.name("b"),
        vec![builder.stmt(StmtKind::Break)],
        third,
    );
    let first = builder.if_else_if_stmt(builder.name("a"), vec![], second);
    let expected = concat!(
        "fn main()\n",
        "  if a\n",
        "  else\n",
        "    if b\n",
        "      break\n",
        "    else\n",
        "      if c\n",
        "        continue\n",
    );
    assert_eq!(print_main(&builder, vec![first]), expected);
}

/// Statements after an `if` return to the `if`'s indentation.
#[test]
fn statement_after_if_returns_to_the_same_indentation() {
    let builder = Builder::new();
    let first = builder.if_else_stmt(
        builder.name("a"),
        vec![builder.stmt(StmtKind::Break)],
        vec![builder.stmt(StmtKind::Continue)],
    );
    let second = builder.stmt(StmtKind::Return(None));
    let expected = "fn main()\n  if a\n    break\n  else\n    continue\n  return\n";
    assert_eq!(print_main(&builder, vec![first, second]), expected);
}

// ---------------------------------------------------------------- for

/// An infinite loop prints as a bare `for`.
#[test]
fn infinite_for() {
    let builder = Builder::new();
    let stmt = builder.for_stmt(ForKind::Infinite, vec![builder.stmt(StmtKind::Break)]);
    let expected = "fn main()\n  for\n    break\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// A while loop prints as `for EXPR`.
#[test]
fn while_for() {
    let builder = Builder::new();
    let cond = builder.binary(BinaryOp::Lt, builder.name("i"), builder.int(10));
    let stmt = builder.for_stmt(ForKind::While(cond), vec![builder.stmt(StmtKind::Continue)]);
    let expected = "fn main()\n  for (< i 10)\n    continue\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// A range loop prints as `for i in EXPR`.
#[test]
fn for_in() {
    let builder = Builder::new();
    let iterable = builder.binary(BinaryOp::Range, builder.int(0), builder.name("n"));
    let kind = ForKind::In {
        var: builder.ident("i"),
        iterable,
    };
    let body = vec![builder.expr_stmt(builder.call("f", vec![builder.name("i")]))];
    let stmt = builder.for_stmt(kind, body);
    let expected = "fn main()\n  for i in (.. 0 n)\n    (call f i)\n";
    assert_eq!(print_main(&builder, vec![stmt]), expected);
}

/// A loop with an empty body prints only the `for` line.
#[test]
fn for_with_empty_body_prints_only_the_header() {
    let builder = Builder::new();
    let stmt = builder.for_stmt(ForKind::Infinite, vec![]);
    assert_eq!(print_main(&builder, vec![stmt]), "fn main()\n  for\n");
}

/// Nested blocks indent two more spaces per level.
#[test]
fn nested_blocks_indent_two_spaces_per_level() {
    let builder = Builder::new();
    let innermost = builder.assign("x", AssignOp::Add, builder.int(1));
    let inner_if = builder.if_stmt(builder.name("c"), vec![innermost]);
    let kind = ForKind::In {
        var: builder.ident("i"),
        iterable: builder.name("r"),
    };
    let outer_for = builder.for_stmt(kind, vec![inner_if]);
    let expected = concat!(
        "fn main()\n",
        "  for i in r\n",
        "    if c\n",
        "      x += 1\n",
    );
    assert_eq!(print_main(&builder, vec![outer_for]), expected);
}

// ---------------------------------------------------------------- expressions

/// An integer literal prints as its decimal value.
#[test]
fn integer_expression() {
    let builder = Builder::new();
    assert_eq!(print_expr(&builder, builder.int(42)), "42");
}

/// The largest `u64` prints in full.
#[test]
fn largest_integer_expression() {
    let builder = Builder::new();
    let printed = print_expr(&builder, builder.int(u64::MAX));
    assert_eq!(printed, "18446744073709551615");
}

/// Boolean literals print as `true` and `false`.
#[test]
fn boolean_expressions() {
    let builder = Builder::new();
    assert_eq!(print_expr(&builder, builder.boolean(true)), "true");
    assert_eq!(print_expr(&builder, builder.boolean(false)), "false");
}

/// A name prints as itself.
#[test]
fn name_expression() {
    let builder = Builder::new();
    assert_eq!(print_expr(&builder, builder.name("total")), "total");
}

/// Every unary operator prints as `(OP operand)`.
#[test]
fn unary_expression_with_every_operator() {
    for (op, spelling) in unary_table() {
        let builder = Builder::new();
        let printed = print_expr(&builder, builder.unary(op, builder.name("x")));
        assert_eq!(printed, format!("({spelling} x)"));
    }
}

/// Every binary operator prints as `(OP left right)`.
#[test]
fn binary_expression_with_every_operator() {
    for (op, spelling) in binary_table() {
        let builder = Builder::new();
        let expr = builder.binary(op, builder.name("a"), builder.name("b"));
        let printed = print_expr(&builder, expr);
        assert_eq!(printed, format!("({spelling} a b)"));
    }
}

/// A call with no arguments prints as `(call f)`.
#[test]
fn call_without_arguments() {
    let builder = Builder::new();
    assert_eq!(print_expr(&builder, builder.call("f", vec![])), "(call f)");
}

/// A call with one argument prints as `(call f a)`.
#[test]
fn call_with_one_argument() {
    let builder = Builder::new();
    let call = builder.call("f", vec![builder.name("a")]);
    assert_eq!(print_expr(&builder, call), "(call f a)");
}

/// A call with several arguments lists them separated by single spaces.
#[test]
fn call_with_several_arguments() {
    let builder = Builder::new();
    let args = vec![builder.name("a"), builder.int(2), builder.boolean(true)];
    let call = builder.call("f", args);
    assert_eq!(print_expr(&builder, call), "(call f a 2 true)");
}

/// Nested expressions print fully parenthesized, left to right.
#[test]
fn nested_expression() {
    let builder = Builder::new();
    let product = builder.binary(BinaryOp::Mul, builder.name("b"), builder.int(2));
    let sum = builder.binary(BinaryOp::Add, builder.name("a"), product);
    let negated = builder.unary(UnaryOp::Neg, sum);
    assert_eq!(print_expr(&builder, negated), "(- (+ a (* b 2)))");
}

/// A call can take nested expressions as arguments and be an operand.
#[test]
fn call_inside_expressions() {
    let builder = Builder::new();
    let inner_call = builder.call("g", vec![builder.name("x")]);
    let difference = builder.binary(BinaryOp::Sub, builder.name("n"), builder.int(1));
    let outer_call = builder.call("f", vec![inner_call, difference]);
    let sum = builder.binary(BinaryOp::Add, outer_call, builder.int(1));
    assert_eq!(
        print_expr(&builder, sum),
        "(+ (call f (call g x) (- n 1)) 1)"
    );
}

/// Repeated unary operators nest.
#[test]
fn repeated_unary_operators() {
    let builder = Builder::new();
    let inner = builder.unary(UnaryOp::Neg, builder.name("x"));
    let outer = builder.unary(UnaryOp::Not, inner);
    assert_eq!(print_expr(&builder, outer), "(! (- x))");
}

/// The spec's fib function prints exactly as documented.
#[test]
fn fib_example() {
    let builder = Builder::new();
    let params = vec![builder.param("n", "i64")];
    let return_type = Some(builder.ty("i64"));
    let early_return = builder.if_stmt(
        builder.binary(BinaryOp::Lt, builder.name("n"), builder.int(2)),
        vec![builder.stmt(StmtKind::Return(Some(builder.name("n"))))],
    );
    let fib_of = |offset: u64| {
        let argument = builder.binary(BinaryOp::Sub, builder.name("n"), builder.int(offset));
        builder.call("fib", vec![argument])
    };
    let sum = builder.binary(BinaryOp::Add, fib_of(1), fib_of(2));
    let final_return = builder.stmt(StmtKind::Return(Some(sum)));
    let fib = builder.function("fib", params, return_type, vec![early_return, final_return]);
    let program = Program {
        functions: vec![fib],
    };
    let expected = concat!(
        "fn fib(n: i64) -> i64\n",
        "  if (< n 2)\n",
        "    return n\n",
        "  return (+ (call fib (- n 1)) (call fib (- n 2)))\n",
    );
    assert_eq!(print_program(&program), expected);
}
