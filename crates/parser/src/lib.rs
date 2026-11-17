//! The Ember parser: turns tokens into a syntax tree.

use ast::{
    AssignOp, BinaryOp, Block, ElseBranch, Expr, ExprKind, ForKind, Function, Ident, NodeId, Param,
    Program, Stmt, StmtKind, TypeExpr, UnaryOp,
};
use lexer::{Token, TokenKind, lex};
use source::{Diagnostic, Span};

/// The precedence level of comparison operators (`<`, `==`, ...), which cannot be chained.
const COMPARISON_LEVEL: u8 = 7;

/// The loosest binding level, used to start parsing a whole expression.
const LOOSEST_LEVEL: u8 = 9;

/// The deepest nesting of `if`, `for`, parentheses, calls and prefix operators.
const MAX_NESTING: usize = 64;

/// The most expression nodes one full expression may have.
const MAX_EXPR_NODES: usize = 1000;

/// Parses tokens, which must end with `Eof` as `lex` returns them, into a program.
pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    Parser::new(tokens).parse_program()
}

/// Lexes and parses source text into a program.
pub fn parse_source(text: &str) -> Result<Program, Diagnostic> {
    let tokens = lex(text)?;
    parse(&tokens)
}

/// Reads tokens one by one and builds the syntax tree.
struct Parser {
    // An owned copy of the tokens avoids a lifetime parameter, which keeps the code easier to read.
    tokens: Vec<Token>,
    /// Index of the next token to read.
    pos: usize,
    /// The token returned when reading past the end, so reads never fail.
    eof: Token,
    /// The id the next node will get.
    next_id: u32,
    /// How many nested constructs the parser is inside right now.
    depth: usize,
    /// How many expression nodes the current full expression has so far.
    expr_nodes: usize,
}

impl Parser {
    /// Creates a parser positioned at the first token.
    fn new(tokens: &[Token]) -> Parser {
        let end = match tokens.last() {
            Some(last) => last.span.end,
            None => 0,
        };
        Parser {
            tokens: tokens.to_vec(),
            pos: 0,
            eof: Token {
                kind: TokenKind::Eof,
                span: Span::new(end, end),
            },
            next_id: 0,
            depth: 0,
            expr_nodes: 0,
        }
    }

    /// Returns the token `offset` places after the current one.
    fn token_at(&self, offset: usize) -> &Token {
        self.tokens.get(self.pos + offset).unwrap_or(&self.eof)
    }

    /// Returns a copy of the current token's kind.
    fn peek(&self) -> TokenKind {
        self.token_at(0).kind.clone()
    }

    /// Returns a copy of the kind of the token `offset` places after the current one.
    fn peek_at(&self, offset: usize) -> TokenKind {
        self.token_at(offset).kind.clone()
    }

    /// Returns the current token and moves to the next one.
    fn advance(&mut self) -> Token {
        let token = self.token_at(0).clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    /// Moves past the current token only if it has the given kind, and tells whether it did.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek() == *kind {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Moves past a token of the given kind, or fails with "expected X, found Y".
    fn expect(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
        if self.peek() == kind {
            Ok(self.advance())
        } else {
            Err(self.expected_error(&kind.to_string()))
        }
    }

    /// Returns where the previously read token ends.
    fn previous_end(&self) -> usize {
        if self.pos == 0 {
            return 0;
        }
        match self.tokens.get(self.pos - 1) {
            Some(token) => token.span.end,
            None => self.eof.span.end,
        }
    }

    /// Hands out the next node id.
    fn new_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        // Saturating keeps this panic-free; a program would need 4 billion nodes to reach it.
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Hands out the id of a new expression node, failing if the expression is too large.
    fn new_expr_id(&mut self) -> Result<NodeId, Diagnostic> {
        self.expr_nodes += 1;
        if self.expr_nodes > MAX_EXPR_NODES {
            return Err(self.error_here("expression is too large (limit 1000 nodes)"));
        }
        Ok(self.new_id())
    }

    /// Enters one more nested construct, failing at the current token if that is too deep.
    ///
    /// Callers do not undo this when they fail: the first error stops the whole parse.
    fn enter(&mut self) -> Result<(), Diagnostic> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(self.error_here("nesting is too deep (limit 64)"));
        }
        Ok(())
    }

    /// Leaves the construct entered last.
    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Builds the error "expected X, found Y" pointing at the current token.
    fn expected_error(&self, expected: &str) -> Diagnostic {
        let found = self.token_at(0);
        let message = format!("expected {expected}, found {}", found.kind);
        Diagnostic::error(message, found.span)
    }

    /// Builds an error with the given message pointing at the current token.
    fn error_here(&self, message: &str) -> Diagnostic {
        Diagnostic::error(message, self.token_at(0).span)
    }

    /// Builds the error "`X` is not supported yet" for the current token.
    fn unsupported_here(&self) -> Diagnostic {
        let token = self.token_at(0);
        let message = format!("{} is not supported yet", token.kind);
        Diagnostic::error(message, token.span)
    }

    /// Parses the whole file: a list of functions.
    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut functions = Vec::new();
        loop {
            match self.peek() {
                TokenKind::Eof => break,
                TokenKind::KwFn => {
                    functions.push(self.parse_function()?);
                    self.expect_function_end()?;
                }
                TokenKind::KwPub => return Err(self.unsupported_here()),
                TokenKind::KwStruct
                | TokenKind::KwEnum
                | TokenKind::KwConst
                | TokenKind::KwImport
                | TokenKind::KwExtern
                | TokenKind::KwTest => {
                    let token = self.token_at(0);
                    let message = format!("{} declarations are not supported yet", token.kind);
                    return Err(Diagnostic::error(message, token.span));
                }
                _ => return Err(self.expected_error("`fn`")),
            }
        }
        Ok(Program { functions })
    }

    /// Requires a newline or the end of the file after a function.
    fn expect_function_end(&mut self) -> Result<(), Diagnostic> {
        match self.peek() {
            TokenKind::Newline => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof => Ok(()),
            _ => Err(self.expected_error("newline after function")),
        }
    }

    /// Parses `fn name(params) [-> type]` followed by a block or `= expression`.
    fn parse_function(&mut self) -> Result<Function, Diagnostic> {
        let fn_token = self.advance();
        let name = self.parse_ident("function name")?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;

        let return_type = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_return_type()?)
        } else {
            None
        };

        let body = match self.peek() {
            TokenKind::LBrace => self.parse_block()?,
            TokenKind::Assign => self.parse_expression_body(return_type.is_some())?,
            _ => return Err(self.expected_error("`{` or `=`")),
        };

        Ok(Function {
            id: self.new_id(),
            name,
            params,
            return_type,
            span: Span::new(fn_token.span.start, body.span.end),
            body,
        })
    }

    /// Parses an identifier; `what` names it in the error message.
    fn parse_ident(&mut self, what: &str) -> Result<Ident, Diagnostic> {
        match self.peek() {
            TokenKind::Ident(name) => {
                let token = self.advance();
                Ok(Ident {
                    name,
                    span: token.span,
                })
            }
            _ => Err(self.expected_error(what)),
        }
    }

    /// Parses the parameters and the closing `)`; the opening `(` is already read.
    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        while self.peek() != TokenKind::RParen {
            params.push(self.parse_param()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(params)
    }

    /// Parses one parameter, `name: type`.
    fn parse_param(&mut self) -> Result<Param, Diagnostic> {
        let name = self.parse_ident("parameter name")?;
        if self.peek() != TokenKind::Colon {
            let message = "untyped parameters are not supported yet";
            return Err(Diagnostic::error(message, name.span));
        }
        self.advance();

        let ty = self.parse_type()?;
        Ok(Param {
            id: self.new_id(),
            span: Span::new(name.span.start, ty.span.end),
            name,
            ty,
        })
    }

    /// Parses the type after `->`, rejecting the forms that are not supported yet.
    fn parse_return_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        match self.peek() {
            TokenKind::LParen => {
                Err(self.error_here("multiple return values are not supported yet"))
            }
            TokenKind::Bang => Err(self.error_here("error return types are not supported yet")),
            _ => self.parse_type(),
        }
    }

    /// Parses a type, which for now must be a plain name.
    fn parse_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let only_named = "only named types are supported yet";
        match self.peek() {
            TokenKind::Ident(_) => {
                let name = self.parse_ident("type")?;
                // A `[` after the name would start a tensor type.
                if self.peek() == TokenKind::LBracket {
                    return Err(self.error_here(only_named));
                }
                Ok(TypeExpr {
                    id: self.new_id(),
                    span: name.span,
                    name,
                })
            }
            TokenKind::LBracket | TokenKind::Star | TokenKind::Question | TokenKind::KwFn => {
                Err(self.error_here(only_named))
            }
            _ => Err(self.expected_error("type")),
        }
    }

    /// Parses `= expression` as a block holding one statement.
    fn parse_expression_body(&mut self, returns_value: bool) -> Result<Block, Diagnostic> {
        self.advance();
        // The body starts at its first token, which may be a `(` that the expression drops.
        let start = self.token_at(0).span.start;
        let expr = self.parse_full_expr()?;
        let span = Span::new(start, self.previous_end());

        let kind = if returns_value {
            StmtKind::Return(Some(expr))
        } else {
            StmtKind::Expr(expr)
        };
        let stmt = Stmt {
            id: self.new_id(),
            kind,
            span,
        };
        Ok(Block {
            id: self.new_id(),
            stmts: vec![stmt],
            span,
        })
    }

    /// Parses `{`, the statements, and `}`.
    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let open = self.expect(TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
            self.expect_statement_end()?;
        }

        let close = self.expect(TokenKind::RBrace)?;
        Ok(Block {
            id: self.new_id(),
            stmts,
            span: Span::new(open.span.start, close.span.end),
        })
    }

    /// Requires a newline (which is consumed) or a `}` (which is left for the block).
    fn expect_statement_end(&mut self) -> Result<(), Diagnostic> {
        match self.peek() {
            TokenKind::Newline => {
                self.advance();
                Ok(())
            }
            // At the end of the file the block reports the missing `}` itself.
            TokenKind::RBrace | TokenKind::Eof => Ok(()),
            _ => Err(self.expected_error("newline or `}` after statement")),
        }
    }

    /// Parses one statement, chosen by its first one or two tokens.
    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        match (self.peek(), self.peek_at(1)) {
            (TokenKind::Ident(_), TokenKind::ColonAssign) => self.parse_inferred_let(),
            (TokenKind::Ident(_), TokenKind::Colon) => self.parse_typed_let(),
            (TokenKind::Ident(_), TokenKind::Comma) => Err(Diagnostic::error(
                "multiple assignment is not supported yet",
                self.token_at(1).span,
            )),
            (TokenKind::KwIf, _) => self.parse_if(),
            (TokenKind::KwFor, _) => self.parse_for(),
            (TokenKind::KwReturn, _) => self.parse_return(),
            (TokenKind::KwBreak, _) => Ok(self.parse_keyword_stmt(StmtKind::Break)),
            (TokenKind::KwContinue, _) => Ok(self.parse_keyword_stmt(StmtKind::Continue)),
            (TokenKind::KwMatch | TokenKind::KwDefer | TokenKind::KwArena, _) => {
                Err(self.unsupported_here())
            }
            _ => self.parse_expr_or_assign(),
        }
    }

    /// Builds a statement made of a single keyword, such as `break`.
    fn parse_keyword_stmt(&mut self, kind: StmtKind) -> Stmt {
        let token = self.advance();
        Stmt {
            id: self.new_id(),
            kind,
            span: token.span,
        }
    }

    /// Parses `name := value`.
    fn parse_inferred_let(&mut self) -> Result<Stmt, Diagnostic> {
        let name = self.parse_ident("variable name")?;
        self.expect(TokenKind::ColonAssign)?;
        let value = self.parse_full_expr()?;
        Ok(self.finish_let(name, None, value))
    }

    /// Parses `name: type = value`.
    fn parse_typed_let(&mut self) -> Result<Stmt, Diagnostic> {
        let name = self.parse_ident("variable name")?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Assign)?;
        let value = self.parse_full_expr()?;
        Ok(self.finish_let(name, Some(ty), value))
    }

    /// Builds a `Let` statement that ends where the value ends.
    fn finish_let(&mut self, name: Ident, ty: Option<TypeExpr>, value: Expr) -> Stmt {
        let span = Span::new(name.span.start, self.previous_end());
        Stmt {
            id: self.new_id(),
            kind: StmtKind::Let { name, ty, value },
            span,
        }
    }

    /// Parses `if condition { ... }` with an optional `else`.
    fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        self.enter()?;
        let if_token = self.advance();
        let cond = self.parse_full_expr()?;
        let then_block = self.parse_block()?;
        // An `else if` chain nests: each link is parsed inside the previous `if`.
        let else_branch = self.parse_else()?;
        self.leave();

        Ok(Stmt {
            id: self.new_id(),
            kind: StmtKind::If {
                cond,
                then_block,
                else_branch,
            },
            span: Span::new(if_token.span.start, self.previous_end()),
        })
    }

    /// Parses the optional `else` part that follows the block of an `if`.
    fn parse_else(&mut self) -> Result<Option<ElseBranch>, Diagnostic> {
        // A newline after `}` is only a problem when `else` comes next.
        if self.peek() == TokenKind::Newline && self.peek_at(1) == TokenKind::KwElse {
            return Err(Diagnostic::error(
                "`else` must be on the same line as the closing `}`",
                self.token_at(1).span,
            ));
        }

        if !self.eat(&TokenKind::KwElse) {
            return Ok(None);
        }

        if self.peek() == TokenKind::KwIf {
            let nested = self.parse_if()?;
            Ok(Some(ElseBranch::If(Box::new(nested))))
        } else {
            Ok(Some(ElseBranch::Block(self.parse_block()?)))
        }
    }

    /// Parses the three forms of `for`: infinite, while, and `for x in ...`.
    fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        self.enter()?;
        let for_token = self.advance();

        let kind = match (self.peek(), self.peek_at(1)) {
            (TokenKind::LBrace, _) => ForKind::Infinite,
            (TokenKind::Ident(_), TokenKind::KwIn) => {
                let var = self.parse_ident("loop variable")?;
                self.advance();
                let iterable = self.parse_full_expr()?;
                ForKind::In { var, iterable }
            }
            (TokenKind::Ident(_), TokenKind::Comma) => {
                return Err(Diagnostic::error(
                    "multiple loop variables are not supported yet",
                    self.token_at(1).span,
                ));
            }
            _ => ForKind::While(self.parse_full_expr()?),
        };
        let body = self.parse_block()?;
        self.leave();

        Ok(Stmt {
            id: self.new_id(),
            kind: StmtKind::For { kind, body },
            span: Span::new(for_token.span.start, self.previous_end()),
        })
    }

    /// Parses `return` with an optional value.
    fn parse_return(&mut self) -> Result<Stmt, Diagnostic> {
        let return_token = self.advance();

        let has_value = !matches!(
            self.peek(),
            TokenKind::Newline | TokenKind::RBrace | TokenKind::Eof
        );
        let value = if has_value {
            Some(self.parse_full_expr()?)
        } else {
            None
        };

        if self.peek() == TokenKind::Comma {
            return Err(self.error_here("multiple return values are not supported yet"));
        }

        Ok(Stmt {
            id: self.new_id(),
            kind: StmtKind::Return(value),
            span: Span::new(return_token.span.start, self.previous_end()),
        })
    }

    /// Parses an expression statement, or an assignment if an assignment operator follows.
    fn parse_expr_or_assign(&mut self) -> Result<Stmt, Diagnostic> {
        // The statement starts at its first token, which may be a `(` that the expression drops.
        let start = self.token_at(0).span.start;
        let expr = self.parse_full_expr()?;

        let Some(op) = assign_op(&self.peek()) else {
            return Ok(Stmt {
                id: self.new_id(),
                span: Span::new(start, self.previous_end()),
                kind: StmtKind::Expr(expr),
            });
        };

        let ExprKind::Name(name) = expr.kind else {
            return Err(Diagnostic::error(
                "cannot assign to this expression",
                expr.span,
            ));
        };
        let target = Ident {
            name,
            span: expr.span,
        };

        self.advance();
        let value = self.parse_full_expr()?;
        Ok(Stmt {
            id: self.new_id(),
            kind: StmtKind::Assign { target, op, value },
            span: Span::new(start, self.previous_end()),
        })
    }

    /// Parses an expression that stands on its own, with a fresh count of its nodes.
    fn parse_full_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.expr_nodes = 0;
        self.parse_expr()
    }

    /// Parses an expression that may be part of a bigger one, such as a call argument.
    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(LOOSEST_LEVEL)
    }

    /// Parses binary operators up to `max_level` by precedence climbing.
    ///
    /// A smaller level binds tighter. The right operand is parsed one level tighter
    /// than the operator, which makes every operator left-associative.
    fn parse_binary(&mut self, max_level: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        let mut left_is_comparison = false;

        loop {
            let token_kind = self.peek();
            let (Some(level), Some(op)) = (precedence(&token_kind), binary_op(&token_kind)) else {
                break;
            };
            if level > max_level {
                break;
            }

            let op_token = self.advance();
            // Only a comparison built in this loop counts; a parenthesized one is a fresh operand.
            if level == COMPARISON_LEVEL && left_is_comparison {
                return Err(Diagnostic::error(
                    "comparison operators cannot be chained",
                    op_token.span,
                ));
            }

            let right = self.parse_binary(level - 1)?;
            let span = Span::new(left.span.start, right.span.end);
            left = Expr {
                id: self.new_expr_id()?,
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
            left_is_comparison = level == COMPARISON_LEVEL;
        }

        Ok(left)
    }

    /// Parses prefix operators, which may repeat, then the operand.
    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let Some(op) = unary_op(&self.peek()) else {
            return self.parse_postfix();
        };

        self.enter()?;
        let op_token = self.advance();
        let operand = self.parse_unary()?;
        self.leave();

        let span = Span::new(op_token.span.start, operand.span.end);
        Ok(Expr {
            id: self.new_expr_id()?,
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            span,
        })
    }

    /// Parses a primary expression and rejects postfix forms that are not supported yet.
    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let expr = self.parse_primary()?;

        let message = match self.peek() {
            TokenKind::Dot => "field access is not supported yet",
            TokenKind::LBracket => "indexing is not supported yet",
            TokenKind::Question => "error propagation is not supported yet",
            // A name followed by `(` was already parsed as a call, so this is another expression.
            TokenKind::LParen => "only named functions can be called",
            TokenKind::KwOn => return Err(self.unsupported_here()),
            _ => return Ok(expr),
        };
        Err(self.error_here(message))
    }

    /// Parses a literal, a name, a call, or a parenthesized expression.
    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        match self.peek() {
            TokenKind::Int(value) => self.parse_literal(ExprKind::Int(value)),
            TokenKind::KwTrue => self.parse_literal(ExprKind::Bool(true)),
            TokenKind::KwFalse => self.parse_literal(ExprKind::Bool(false)),
            TokenKind::Ident(name) => {
                if self.peek_at(1) == TokenKind::LParen {
                    self.parse_call()
                } else {
                    self.parse_literal(ExprKind::Name(name))
                }
            }
            TokenKind::LParen => {
                self.enter()?;
                let open = self.advance();
                // Parentheses only group: they leave no node behind, but the inner
                // expression's span grows to cover them, so that `-(1 + 2)` and
                // `(1 + 2) * 3` include both parentheses in their own spans.
                let mut inner = self.parse_expr()?;
                let close = self.expect(TokenKind::RParen)?;
                self.leave();
                inner.span = Span::new(open.span.start, close.span.end);
                Ok(inner)
            }
            TokenKind::Float(_) => Err(self.error_here("float literals are not supported yet")),
            TokenKind::Str(_) => Err(self.error_here("string literals are not supported yet")),
            TokenKind::LBracket => Err(self.error_here("tensor literals are not supported yet")),
            TokenKind::KwNone | TokenKind::KwGrad => Err(self.unsupported_here()),
            _ => Err(self.expected_error("expression")),
        }
    }

    /// Consumes one token and wraps it as an expression of the given kind.
    fn parse_literal(&mut self, kind: ExprKind) -> Result<Expr, Diagnostic> {
        let token = self.advance();
        Ok(Expr {
            id: self.new_expr_id()?,
            kind,
            span: token.span,
        })
    }

    /// Parses `name(arguments)`.
    fn parse_call(&mut self) -> Result<Expr, Diagnostic> {
        self.enter()?;
        let callee = self.parse_ident("function name")?;
        self.expect(TokenKind::LParen)?;

        let mut args = Vec::new();
        while self.peek() != TokenKind::RParen {
            if self.peek_at(1) == TokenKind::Colon && matches!(self.peek(), TokenKind::Ident(_)) {
                return Err(self.error_here("named arguments are not supported yet"));
            }
            args.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let close = self.expect(TokenKind::RParen)?;
        self.leave();

        Ok(Expr {
            id: self.new_expr_id()?,
            span: Span::new(callee.span.start, close.span.end),
            kind: ExprKind::Call { callee, args },
        })
    }
}

/// Returns the binding level of a binary operator token; smaller binds tighter.
fn precedence(kind: &TokenKind) -> Option<u8> {
    let level = match kind {
        TokenKind::At => 3,
        TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Shl
        | TokenKind::Shr
        | TokenKind::Amp => 4,
        TokenKind::Plus | TokenKind::Minus | TokenKind::Pipe | TokenKind::Caret => 5,
        TokenKind::DotDot | TokenKind::DotDotEq => 6,
        TokenKind::EqEq
        | TokenKind::NotEq
        | TokenKind::Lt
        | TokenKind::LtEq
        | TokenKind::Gt
        | TokenKind::GtEq => COMPARISON_LEVEL,
        TokenKind::AndAnd => 8,
        TokenKind::OrOr => 9,
        _ => return None,
    };
    Some(level)
}

/// Returns the binary operator a token stands for, if any.
fn binary_op(kind: &TokenKind) -> Option<BinaryOp> {
    let op = match kind {
        TokenKind::At => BinaryOp::MatMul,
        TokenKind::Star => BinaryOp::Mul,
        TokenKind::Slash => BinaryOp::Div,
        TokenKind::Percent => BinaryOp::Rem,
        TokenKind::Shl => BinaryOp::Shl,
        TokenKind::Shr => BinaryOp::Shr,
        TokenKind::Amp => BinaryOp::BitAnd,
        TokenKind::Plus => BinaryOp::Add,
        TokenKind::Minus => BinaryOp::Sub,
        TokenKind::Pipe => BinaryOp::BitOr,
        TokenKind::Caret => BinaryOp::BitXor,
        TokenKind::DotDot => BinaryOp::Range,
        TokenKind::DotDotEq => BinaryOp::RangeInclusive,
        TokenKind::EqEq => BinaryOp::Eq,
        TokenKind::NotEq => BinaryOp::Ne,
        TokenKind::Lt => BinaryOp::Lt,
        TokenKind::LtEq => BinaryOp::Le,
        TokenKind::Gt => BinaryOp::Gt,
        TokenKind::GtEq => BinaryOp::Ge,
        TokenKind::AndAnd => BinaryOp::And,
        TokenKind::OrOr => BinaryOp::Or,
        _ => return None,
    };
    Some(op)
}

/// Returns the prefix operator a token stands for, if any.
fn unary_op(kind: &TokenKind) -> Option<UnaryOp> {
    let op = match kind {
        TokenKind::Minus => UnaryOp::Neg,
        TokenKind::Bang => UnaryOp::Not,
        TokenKind::Tilde => UnaryOp::BitNot,
        TokenKind::Amp => UnaryOp::AddrOf,
        TokenKind::Star => UnaryOp::Deref,
        _ => return None,
    };
    Some(op)
}

/// Returns the assignment operator a token stands for, if any.
fn assign_op(kind: &TokenKind) -> Option<AssignOp> {
    let op = match kind {
        TokenKind::Assign => AssignOp::Assign,
        TokenKind::PlusAssign => AssignOp::Add,
        TokenKind::MinusAssign => AssignOp::Sub,
        TokenKind::StarAssign => AssignOp::Mul,
        TokenKind::SlashAssign => AssignOp::Div,
        TokenKind::AtAssign => AssignOp::MatMul,
        _ => return None,
    };
    Some(op)
}
