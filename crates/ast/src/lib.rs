//! The syntax tree of an Ember program, and a printer that shows it as text.

use std::fmt;

use source::Span;

/// A number that identifies one node of the tree within a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

/// A name and where it appears in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// The name as written.
    pub name: String,
    /// Where the name appears.
    pub span: Span,
}

/// A whole source file: its functions, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The functions of the file.
    pub functions: Vec<Function>,
}

/// A function declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The node id.
    pub id: NodeId,
    /// The function name.
    pub name: Ident,
    /// The parameters, in order.
    pub params: Vec<Param>,
    /// The return type, or `None` when the function returns nothing.
    pub return_type: Option<TypeExpr>,
    /// The function body.
    pub body: Block,
    /// From `fn` to the end of the body.
    pub span: Span,
}

/// One function parameter, `name: type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The node id.
    pub id: NodeId,
    /// The parameter name.
    pub name: Ident,
    /// The parameter type.
    pub ty: TypeExpr,
    /// From the name to the end of the type.
    pub span: Span,
}

/// A type as written in the source; for now only a plain name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeExpr {
    /// The node id.
    pub id: NodeId,
    /// The type name.
    pub name: Ident,
    /// Where the type appears.
    pub span: Span,
}

/// A list of statements between braces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The node id.
    pub id: NodeId,
    /// The statements, in order.
    pub stmts: Vec<Stmt>,
    /// From `{` to `}`.
    pub span: Span,
}

/// One statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    /// The node id.
    pub id: NodeId,
    /// What kind of statement this is.
    pub kind: StmtKind,
    /// From the first token to the end of the last part.
    pub span: Span,
}

/// The different kinds of statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtKind {
    /// A new variable: `x := e` or `x: T = e`.
    Let {
        /// The variable name.
        name: Ident,
        /// The declared type, if written.
        ty: Option<TypeExpr>,
        /// The initial value.
        value: Expr,
    },
    /// An assignment to an existing variable.
    Assign {
        /// The variable being assigned.
        target: Ident,
        /// The assignment operator.
        op: AssignOp,
        /// The assigned value.
        value: Expr,
    },
    /// An `if` with an optional `else`.
    If {
        /// The condition.
        cond: Expr,
        /// The block run when the condition is true.
        then_block: Block,
        /// The `else` part, if any.
        else_branch: Option<ElseBranch>,
    },
    /// A `for` loop.
    For {
        /// Which form of loop this is.
        kind: ForKind,
        /// The loop body.
        body: Block,
    },
    /// `return`, with an optional value.
    Return(Option<Expr>),
    /// `break`.
    Break,
    /// `continue`.
    Continue,
    /// An expression used as a statement.
    Expr(Expr),
}

/// The part of an `if` after `else`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElseBranch {
    /// `else if ...`: a statement whose kind is `If`.
    If(Box<Stmt>),
    /// `else { ... }`.
    Block(Block),
}

/// The forms of `for` loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForKind {
    /// `for { ... }`
    Infinite,
    /// `for cond { ... }`
    While(Expr),
    /// `for var in iterable { ... }`
    In {
        /// The loop variable.
        var: Ident,
        /// The value looped over.
        iterable: Expr,
    },
}

/// The assignment operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    /// `=`
    Assign,
    /// `+=`
    Add,
    /// `-=`
    Sub,
    /// `*=`
    Mul,
    /// `/=`
    Div,
    /// `@=`
    MatMul,
}

/// One expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    /// The node id.
    pub id: NodeId,
    /// What kind of expression this is.
    pub kind: ExprKind,
    /// Where the expression appears.
    pub span: Span,
}

/// The different kinds of expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// An integer literal.
    Int(u64),
    /// `true` or `false`.
    Bool(bool),
    /// A variable name.
    Name(String),
    /// A prefix operator applied to one operand.
    Unary {
        /// The operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<Expr>,
    },
    /// An infix operator applied to two operands.
    Binary {
        /// The operator.
        op: BinaryOp,
        /// The left operand.
        left: Box<Expr>,
        /// The right operand.
        right: Box<Expr>,
    },
    /// A function call.
    Call {
        /// The function being called.
        callee: Ident,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
}

/// The prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-`
    Neg,
    /// `!`
    Not,
    /// `~`
    BitNot,
    /// `&`
    AddrOf,
    /// `*`
    Deref,
}

/// The infix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `@`
    MatMul,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `&`
    BitAnd,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `..`
    Range,
    /// `..=`
    RangeInclusive,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
}

impl fmt::Display for UnaryOp {
    /// Writes the operator as it is spelled in source.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spelling = match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::BitNot => "~",
            UnaryOp::AddrOf => "&",
            UnaryOp::Deref => "*",
        };
        write!(f, "{spelling}")
    }
}

impl fmt::Display for BinaryOp {
    /// Writes the operator as it is spelled in source.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spelling = match self {
            BinaryOp::MatMul => "@",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::BitAnd => "&",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Range => "..",
            BinaryOp::RangeInclusive => "..=",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        };
        write!(f, "{spelling}")
    }
}

impl fmt::Display for AssignOp {
    /// Writes the operator as it is spelled in source.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spelling = match self {
            AssignOp::Assign => "=",
            AssignOp::Add => "+=",
            AssignOp::Sub => "-=",
            AssignOp::Mul => "*=",
            AssignOp::Div => "/=",
            AssignOp::MatMul => "@=",
        };
        write!(f, "{spelling}")
    }
}

/// Shows a program as indented text, one line per function header and statement.
pub fn print_program(program: &Program) -> String {
    let mut out = String::new();
    for function in &program.functions {
        print_function(function, &mut out);
    }
    out
}

/// Appends the header line of a function, then its body indented.
fn print_function(function: &Function, out: &mut String) {
    let mut params = Vec::new();
    for param in &function.params {
        params.push(format!("{}: {}", param.name.name, param.ty.name.name));
    }

    let mut header = format!("fn {}({})", function.name.name, params.join(", "));
    if let Some(return_type) = &function.return_type {
        header.push_str(&format!(" -> {}", return_type.name.name));
    }

    push_line(out, 0, &header);
    print_block(&function.body, 1, out);
}

/// Appends the indentation of a line, two spaces per level.
fn start_line(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

/// Appends one line of text, indented two spaces per level.
fn push_line(out: &mut String, indent: usize, text: &str) {
    start_line(out, indent);
    out.push_str(text);
    out.push('\n');
}

/// Appends every statement of a block at the given indentation.
fn print_block(block: &Block, indent: usize, out: &mut String) {
    for stmt in &block.stmts {
        print_stmt(stmt, indent, out);
    }
}

/// Appends a statement, and the statements nested inside it.
fn print_stmt(stmt: &Stmt, indent: usize, out: &mut String) {
    match &stmt.kind {
        StmtKind::Let { name, ty, value } => {
            start_line(out, indent);
            out.push_str("let ");
            out.push_str(&name.name);
            if let Some(ty) = ty {
                out.push_str(": ");
                out.push_str(&ty.name.name);
            }
            out.push_str(" = ");
            print_expr(value, out);
            out.push('\n');
        }
        StmtKind::Assign { target, op, value } => {
            start_line(out, indent);
            out.push_str(&format!("{} {op} ", target.name));
            print_expr(value, out);
            out.push('\n');
        }
        StmtKind::If {
            cond,
            then_block,
            else_branch,
        } => print_if(cond, then_block, else_branch, indent, out),
        StmtKind::For { kind, body } => {
            start_line(out, indent);
            out.push_str("for");
            match kind {
                ForKind::Infinite => {}
                ForKind::While(cond) => {
                    out.push(' ');
                    print_expr(cond, out);
                }
                ForKind::In { var, iterable } => {
                    out.push_str(&format!(" {} in ", var.name));
                    print_expr(iterable, out);
                }
            }
            out.push('\n');
            print_block(body, indent + 1, out);
        }
        StmtKind::Return(value) => {
            start_line(out, indent);
            out.push_str("return");
            if let Some(value) = value {
                out.push(' ');
                print_expr(value, out);
            }
            out.push('\n');
        }
        StmtKind::Break => push_line(out, indent, "break"),
        StmtKind::Continue => push_line(out, indent, "continue"),
        StmtKind::Expr(expr) => {
            start_line(out, indent);
            print_expr(expr, out);
            out.push('\n');
        }
    }
}

/// Appends an `if` statement with its optional `else` part.
fn print_if(
    cond: &Expr,
    then_block: &Block,
    else_branch: &Option<ElseBranch>,
    indent: usize,
    out: &mut String,
) {
    start_line(out, indent);
    out.push_str("if ");
    print_expr(cond, out);
    out.push('\n');
    print_block(then_block, indent + 1, out);

    match else_branch {
        Some(ElseBranch::Block(block)) => {
            push_line(out, indent, "else");
            print_block(block, indent + 1, out);
        }
        Some(ElseBranch::If(nested)) => {
            push_line(out, indent, "else");
            print_stmt(nested, indent + 1, out);
        }
        None => {}
    }
}

/// Appends an expression in fully parenthesized prefix form, such as `(+ a b)`.
fn print_expr(expr: &Expr, out: &mut String) {
    match &expr.kind {
        ExprKind::Int(value) => out.push_str(&value.to_string()),
        ExprKind::Bool(value) => out.push_str(&value.to_string()),
        ExprKind::Name(name) => out.push_str(name),
        ExprKind::Unary { op, operand } => {
            out.push_str(&format!("({op} "));
            print_expr(operand, out);
            out.push(')');
        }
        ExprKind::Binary { op, left, right } => {
            out.push_str(&format!("({op} "));
            print_expr(left, out);
            out.push(' ');
            print_expr(right, out);
            out.push(')');
        }
        ExprKind::Call { callee, args } => {
            out.push_str(&format!("(call {}", callee.name));
            for arg in args {
                out.push(' ');
                print_expr(arg, out);
            }
            out.push(')');
        }
    }
}
