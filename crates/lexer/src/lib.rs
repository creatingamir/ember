//! The Ember lexer: turns source text into tokens.

use std::fmt;

use source::{Diagnostic, Span};

/// The words that are reserved for future use and rejected today.
const RESERVED_WORDS: [&str; 6] = ["interface", "async", "spawn", "unsafe", "macro", "yield"];

/// What kind of token this is, with the value for literals and names.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// The keyword `fn`.
    KwFn,
    /// The keyword `struct`.
    KwStruct,
    /// The keyword `enum`.
    KwEnum,
    /// The keyword `const`.
    KwConst,
    /// The keyword `pub`.
    KwPub,
    /// The keyword `import`.
    KwImport,
    /// The keyword `extern`.
    KwExtern,
    /// The keyword `if`.
    KwIf,
    /// The keyword `else`.
    KwElse,
    /// The keyword `for`.
    KwFor,
    /// The keyword `in`.
    KwIn,
    /// The keyword `break`.
    KwBreak,
    /// The keyword `continue`.
    KwContinue,
    /// The keyword `match`.
    KwMatch,
    /// The keyword `return`.
    KwReturn,
    /// The keyword `defer`.
    KwDefer,
    /// The keyword `arena`.
    KwArena,
    /// The keyword `grad`.
    KwGrad,
    /// The keyword `on`.
    KwOn,
    /// The keyword `test`.
    KwTest,
    /// The keyword `true`.
    KwTrue,
    /// The keyword `false`.
    KwFalse,
    /// The keyword `none`.
    KwNone,

    /// A name that is not a keyword.
    Ident(String),
    /// An integer literal.
    Int(u64),
    /// A floating-point literal.
    Float(f64),
    /// A string literal: the raw text between the quotes, escapes not decoded.
    Str(String),

    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `->`
    Arrow,
    /// `=>`
    FatArrow,
    /// `?`
    Question,

    /// `=`
    Assign,
    /// `:=`
    ColonAssign,
    /// `+=`
    PlusAssign,
    /// `-=`
    MinusAssign,
    /// `*=`
    StarAssign,
    /// `/=`
    SlashAssign,
    /// `@=`
    AtAssign,

    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `@`
    At,
    /// `!`
    Bang,
    /// `~`
    Tilde,
    /// `&`
    Amp,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,
    /// `..`
    DotDot,
    /// `..=`
    DotDotEq,

    /// A line break that ends a statement.
    Newline,
    /// The end of the input.
    Eof,
}

impl TokenKind {
    /// Returns the source spelling of a fixed token, or `None` for names, literals and markers.
    fn spelling(&self) -> Option<&'static str> {
        match self {
            TokenKind::KwFn => Some("fn"),
            TokenKind::KwStruct => Some("struct"),
            TokenKind::KwEnum => Some("enum"),
            TokenKind::KwConst => Some("const"),
            TokenKind::KwPub => Some("pub"),
            TokenKind::KwImport => Some("import"),
            TokenKind::KwExtern => Some("extern"),
            TokenKind::KwIf => Some("if"),
            TokenKind::KwElse => Some("else"),
            TokenKind::KwFor => Some("for"),
            TokenKind::KwIn => Some("in"),
            TokenKind::KwBreak => Some("break"),
            TokenKind::KwContinue => Some("continue"),
            TokenKind::KwMatch => Some("match"),
            TokenKind::KwReturn => Some("return"),
            TokenKind::KwDefer => Some("defer"),
            TokenKind::KwArena => Some("arena"),
            TokenKind::KwGrad => Some("grad"),
            TokenKind::KwOn => Some("on"),
            TokenKind::KwTest => Some("test"),
            TokenKind::KwTrue => Some("true"),
            TokenKind::KwFalse => Some("false"),
            TokenKind::KwNone => Some("none"),
            TokenKind::LParen => Some("("),
            TokenKind::RParen => Some(")"),
            TokenKind::LBracket => Some("["),
            TokenKind::RBracket => Some("]"),
            TokenKind::LBrace => Some("{"),
            TokenKind::RBrace => Some("}"),
            TokenKind::Comma => Some(","),
            TokenKind::Colon => Some(":"),
            TokenKind::Dot => Some("."),
            TokenKind::Arrow => Some("->"),
            TokenKind::FatArrow => Some("=>"),
            TokenKind::Question => Some("?"),
            TokenKind::Assign => Some("="),
            TokenKind::ColonAssign => Some(":="),
            TokenKind::PlusAssign => Some("+="),
            TokenKind::MinusAssign => Some("-="),
            TokenKind::StarAssign => Some("*="),
            TokenKind::SlashAssign => Some("/="),
            TokenKind::AtAssign => Some("@="),
            TokenKind::Plus => Some("+"),
            TokenKind::Minus => Some("-"),
            TokenKind::Star => Some("*"),
            TokenKind::Slash => Some("/"),
            TokenKind::Percent => Some("%"),
            TokenKind::At => Some("@"),
            TokenKind::Bang => Some("!"),
            TokenKind::Tilde => Some("~"),
            TokenKind::Amp => Some("&"),
            TokenKind::Pipe => Some("|"),
            TokenKind::Caret => Some("^"),
            TokenKind::Shl => Some("<<"),
            TokenKind::Shr => Some(">>"),
            TokenKind::EqEq => Some("=="),
            TokenKind::NotEq => Some("!="),
            TokenKind::Lt => Some("<"),
            TokenKind::LtEq => Some("<="),
            TokenKind::Gt => Some(">"),
            TokenKind::GtEq => Some(">="),
            TokenKind::AndAnd => Some("&&"),
            TokenKind::OrOr => Some("||"),
            TokenKind::DotDot => Some(".."),
            TokenKind::DotDotEq => Some("..="),
            TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Str(_)
            | TokenKind::Newline
            | TokenKind::Eof => None,
        }
    }
}

impl fmt::Display for TokenKind {
    /// Describes the token for error messages, such as `` `fn` `` or `identifier `x``.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Ident(name) => write!(f, "identifier `{name}`"),
            TokenKind::Int(_) => write!(f, "integer literal"),
            TokenKind::Float(_) => write!(f, "float literal"),
            TokenKind::Str(_) => write!(f, "string literal"),
            TokenKind::Newline => write!(f, "newline"),
            TokenKind::Eof => write!(f, "end of file"),
            fixed => match fixed.spelling() {
                Some(spelling) => write!(f, "`{spelling}`"),
                // The compiler cannot see that the six kinds above are the only ones without a
                // spelling, so this arm is required but never reached.
                None => Ok(()),
            },
        }
    }
}

/// A token and the part of the source it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What kind of token this is.
    pub kind: TokenKind,
    /// Where the token is in the source text.
    pub span: Span,
}

/// Turns source text into tokens, ending with exactly one `Eof`.
pub fn lex(text: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer::new(text).run()
}

/// Walks over the text one char at a time and collects tokens.
struct Lexer {
    // An owned copy of the text avoids a lifetime parameter, which keeps the code easier to read.
    text: String,
    /// Byte offset of the next char to read; always on a char boundary.
    pos: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    /// Creates a lexer positioned at the start of the text.
    fn new(text: &str) -> Lexer {
        Lexer {
            text: text.to_string(),
            pos: 0,
            tokens: Vec::new(),
        }
    }

    /// Lexes the whole text and returns the tokens, or the first error.
    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        while let Some(ch) = self.peek() {
            self.lex_next(ch)?;
        }
        self.finish();
        Ok(self.tokens)
    }

    /// Lexes whatever starts with `ch`: whitespace, a comment, or one token.
    fn lex_next(&mut self, ch: char) -> Result<(), Diagnostic> {
        match ch {
            ' ' | '\t' | '\r' => {
                self.advance();
                Ok(())
            }
            '\n' => {
                self.lex_newline();
                Ok(())
            }
            '/' if self.peek_at(1) == Some('/') => {
                self.skip_comment();
                Ok(())
            }
            '"' => self.lex_string(),
            '0'..='9' => self.lex_number(),
            'a'..='z' | 'A'..='Z' | '_' => self.lex_word(),
            _ => self.lex_punctuation(),
        }
    }

    /// Returns the text that has not been read yet.
    fn rest(&self) -> &str {
        self.text.get(self.pos..).unwrap_or("")
    }

    /// Returns the next char without reading it.
    fn peek(&self) -> Option<char> {
        self.peek_at(0)
    }

    /// Returns the char `count` chars ahead without reading it.
    fn peek_at(&self, count: usize) -> Option<char> {
        self.rest().chars().nth(count)
    }

    /// Reads the next char and moves past it.
    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    /// Reads the next char only if it is `expected`, and tells whether it did.
    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Adds a token that runs from `start` to the current position.
    fn push(&mut self, kind: TokenKind, start: usize) {
        let span = Span::new(start, self.pos);
        self.tokens.push(Token { kind, span });
    }

    /// Adds the final `Newline` (if one is due) and the `Eof` token.
    fn finish(&mut self) {
        let end = self.text.len();
        if self.previous_ends_statement() {
            self.tokens.push(Token {
                kind: TokenKind::Newline,
                span: Span::new(end, end),
            });
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(end, end),
        });
    }

    /// Tells whether the last token is one a statement can end with.
    fn previous_ends_statement(&self) -> bool {
        let Some(last) = self.tokens.last() else {
            return false;
        };
        matches!(
            last.kind,
            TokenKind::Ident(_)
                | TokenKind::Int(_)
                | TokenKind::Float(_)
                | TokenKind::Str(_)
                | TokenKind::KwTrue
                | TokenKind::KwFalse
                | TokenKind::KwNone
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Question
                | TokenKind::KwReturn
                | TokenKind::KwBreak
                | TokenKind::KwContinue
        )
    }

    /// Reads a `\n`; it becomes a `Newline` token only if the statement could end here.
    fn lex_newline(&mut self) {
        let start = self.pos;
        let ends_statement = self.previous_ends_statement();
        self.advance();
        if ends_statement {
            self.push(TokenKind::Newline, start);
        }
    }

    /// Skips a `//` comment up to, but not including, the end of the line.
    fn skip_comment(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.advance();
        }
    }

    /// Reads a keyword or an identifier, and rejects reserved words.
    fn lex_word(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if !(ch.is_ascii_alphanumeric() || ch == '_') {
                break;
            }
            self.advance();
        }
        let word = self.text[start..self.pos].to_string();

        if RESERVED_WORDS.contains(&word.as_str()) {
            let message = format!("`{word}` is reserved for future use");
            return Err(Diagnostic::error(message, Span::new(start, self.pos)));
        }

        let kind = keyword_kind(&word).unwrap_or(TokenKind::Ident(word));
        self.push(kind, start);
        Ok(())
    }

    /// Reads an operator or delimiter, preferring the longest match.
    fn lex_punctuation(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        let Some(ch) = self.advance() else {
            return Ok(());
        };

        let kind = match ch {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ',' => TokenKind::Comma,
            '?' => TokenKind::Question,
            '~' => TokenKind::Tilde,
            '^' => TokenKind::Caret,
            '%' => TokenKind::Percent,
            '+' => self.either('=', TokenKind::PlusAssign, TokenKind::Plus),
            '*' => self.either('=', TokenKind::StarAssign, TokenKind::Star),
            '/' => self.either('=', TokenKind::SlashAssign, TokenKind::Slash),
            '@' => self.either('=', TokenKind::AtAssign, TokenKind::At),
            '!' => self.either('=', TokenKind::NotEq, TokenKind::Bang),
            ':' => self.either('=', TokenKind::ColonAssign, TokenKind::Colon),
            '&' => self.either('&', TokenKind::AndAnd, TokenKind::Amp),
            '|' => self.either('|', TokenKind::OrOr, TokenKind::Pipe),
            '-' => self.lex_minus(),
            '=' => self.lex_equals(),
            '<' => self.lex_less_than(),
            '>' => self.lex_greater_than(),
            '.' => self.lex_dot(),
            other => {
                let message = format!("unexpected character `{other}`");
                return Err(Diagnostic::error(message, Span::new(start, self.pos)));
            }
        };

        self.push(kind, start);
        Ok(())
    }

    /// Returns `if_next` when the next char is `expected` (and reads it), else `otherwise`.
    fn either(&mut self, expected: char, if_next: TokenKind, otherwise: TokenKind) -> TokenKind {
        if self.eat(expected) {
            if_next
        } else {
            otherwise
        }
    }

    /// Decides between `->`, `-=` and `-` after the `-` was read.
    fn lex_minus(&mut self) -> TokenKind {
        if self.eat('>') {
            TokenKind::Arrow
        } else if self.eat('=') {
            TokenKind::MinusAssign
        } else {
            TokenKind::Minus
        }
    }

    /// Decides between `==`, `=>` and `=` after the `=` was read.
    fn lex_equals(&mut self) -> TokenKind {
        if self.eat('=') {
            TokenKind::EqEq
        } else if self.eat('>') {
            TokenKind::FatArrow
        } else {
            TokenKind::Assign
        }
    }

    /// Decides between `<<`, `<=` and `<` after the `<` was read.
    fn lex_less_than(&mut self) -> TokenKind {
        if self.eat('<') {
            TokenKind::Shl
        } else if self.eat('=') {
            TokenKind::LtEq
        } else {
            TokenKind::Lt
        }
    }

    /// Decides between `>>`, `>=` and `>` after the `>` was read.
    fn lex_greater_than(&mut self) -> TokenKind {
        if self.eat('>') {
            TokenKind::Shr
        } else if self.eat('=') {
            TokenKind::GtEq
        } else {
            TokenKind::Gt
        }
    }

    /// Decides between `..=`, `..` and `.` after the first `.` was read.
    fn lex_dot(&mut self) -> TokenKind {
        if !self.eat('.') {
            return TokenKind::Dot;
        }
        if self.eat('=') {
            TokenKind::DotDotEq
        } else {
            TokenKind::DotDot
        }
    }

    /// Reads a string literal and checks its escapes; the token keeps the raw text.
    fn lex_string(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        self.advance();

        loop {
            match self.peek() {
                None | Some('\n') => return Err(unterminated_string(start)),
                Some('"') => break,
                Some('\\') => self.check_escape(start)?,
                Some(_) => {
                    self.advance();
                }
            }
        }

        let value = self.text[start + 1..self.pos].to_string();
        self.advance();
        self.push(TokenKind::Str(value), start);
        Ok(())
    }

    /// Reads a backslash and the char after it, which must be an allowed escape.
    ///
    /// `string_start` is where the string's opening quote is, for the unterminated error.
    fn check_escape(&mut self, string_start: usize) -> Result<(), Diagnostic> {
        let backslash_start = self.pos;
        self.advance();

        match self.peek() {
            // A backslash right before the line end (LF or CRLF) leaves the string open.
            None | Some('\n') => Err(unterminated_string(string_start)),
            Some('\r') if self.peek_at(1) == Some('\n') => Err(unterminated_string(string_start)),
            Some('n' | 't' | '"' | '\\' | '{') => {
                self.advance();
                Ok(())
            }
            Some(other) => {
                self.advance();
                let message = format!("invalid escape sequence `\\{other}`");
                Err(Diagnostic::error(
                    message,
                    Span::new(backslash_start, self.pos),
                ))
            }
        }
    }

    /// Reads an integer or float literal, in decimal, hex or binary.
    fn lex_number(&mut self) -> Result<(), Diagnostic> {
        match (self.peek(), self.peek_at(1)) {
            (Some('0'), Some('x')) => self.lex_prefixed_integer(16, "0x"),
            (Some('0'), Some('b')) => self.lex_prefixed_integer(2, "0b"),
            _ => self.lex_decimal_number(),
        }
    }

    /// Reads a `0x` or `0b` integer; `radix` is 16 or 2.
    fn lex_prefixed_integer(&mut self, radix: u32, prefix: &str) -> Result<(), Diagnostic> {
        let start = self.pos;
        self.advance();
        self.advance();
        let prefix_end = self.pos;

        let mut digits = String::new();
        while let Some(ch) = self.peek() {
            if ch == '_' {
                self.advance();
            } else if ch.is_digit(radix) {
                digits.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if digits.is_empty() {
            let message = format!("expected digits after `{prefix}`");
            return Err(Diagnostic::error(message, Span::new(start, prefix_end)));
        }

        self.reject_trailing_name_char()?;
        let value = self.parse_integer(&digits, radix, start)?;
        self.push(TokenKind::Int(value), start);
        Ok(())
    }

    /// Reads a decimal integer or a float (fraction and/or exponent).
    fn lex_decimal_number(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        let mut text = String::new();
        let mut is_float = false;
        self.read_digits(&mut text);

        // A `.` belongs to the number only when a digit follows, so `1..10` stays a range.
        let dot_then_digit =
            self.peek() == Some('.') && self.peek_at(1).is_some_and(|ch| ch.is_ascii_digit());
        if dot_then_digit {
            self.advance();
            text.push('.');
            self.read_digits(&mut text);
            is_float = true;
        }

        if matches!(self.peek(), Some('e' | 'E')) {
            self.read_exponent(&mut text)?;
            is_float = true;
        }

        self.reject_trailing_name_char()?;

        if is_float {
            let value = self.parse_float(&text, start)?;
            self.push(TokenKind::Float(value), start);
        } else {
            let value = self.parse_integer(&text, 10, start)?;
            self.push(TokenKind::Int(value), start);
        }
        Ok(())
    }

    /// Reads decimal digits and underscores, appending only the digits to `text`.
    fn read_digits(&mut self, text: &mut String) -> usize {
        let mut digit_count = 0;
        while let Some(ch) = self.peek() {
            if ch == '_' {
                self.advance();
            } else if ch.is_ascii_digit() {
                text.push(ch);
                digit_count += 1;
                self.advance();
            } else {
                break;
            }
        }
        digit_count
    }

    /// Reads `e`, an optional sign and the exponent digits, appending them to `text`.
    fn read_exponent(&mut self, text: &mut String) -> Result<(), Diagnostic> {
        let exponent_start = self.pos;
        self.advance();
        text.push('e');

        if let Some(sign @ ('+' | '-')) = self.peek() {
            self.advance();
            text.push(sign);
        }

        if self.read_digits(text) == 0 {
            return Err(Diagnostic::error(
                "expected digits in exponent",
                Span::new(exponent_start, self.pos),
            ));
        }
        Ok(())
    }

    /// Fails if a letter or digit follows a number, such as the `a` in `123abc`.
    fn reject_trailing_name_char(&mut self) -> Result<(), Diagnostic> {
        let Some(ch) = self.peek() else {
            return Ok(());
        };
        if !ch.is_ascii_alphanumeric() {
            return Ok(());
        }

        let start = self.pos;
        self.advance();
        let message = format!("invalid character `{ch}` in number literal");
        Err(Diagnostic::error(message, Span::new(start, self.pos)))
    }

    /// Converts digits to a `u64`, failing if the value does not fit.
    fn parse_integer(&self, digits: &str, radix: u32, start: usize) -> Result<u64, Diagnostic> {
        u64::from_str_radix(digits, radix).map_err(|_| {
            Diagnostic::error("integer literal is too large", Span::new(start, self.pos))
        })
    }

    /// Converts float text such as `2.5e+10` to an `f64`, rejecting values that overflow.
    fn parse_float(&self, text: &str, start: usize) -> Result<f64, Diagnostic> {
        let too_large =
            || Diagnostic::error("float literal is too large", Span::new(start, self.pos));

        // Parsing cannot fail for text built from digits, so this branch is only a safety net.
        let value = text.parse::<f64>().map_err(|_| too_large())?;
        if value.is_infinite() {
            return Err(too_large());
        }
        Ok(value)
    }
}

/// Builds the error for a string whose opening quote is at `start`.
fn unterminated_string(start: usize) -> Diagnostic {
    Diagnostic::error("unterminated string literal", Span::new(start, start + 1))
}

/// Returns the keyword token for a word, or `None` if the word is an ordinary name.
fn keyword_kind(word: &str) -> Option<TokenKind> {
    let kind = match word {
        "fn" => TokenKind::KwFn,
        "struct" => TokenKind::KwStruct,
        "enum" => TokenKind::KwEnum,
        "const" => TokenKind::KwConst,
        "pub" => TokenKind::KwPub,
        "import" => TokenKind::KwImport,
        "extern" => TokenKind::KwExtern,
        "if" => TokenKind::KwIf,
        "else" => TokenKind::KwElse,
        "for" => TokenKind::KwFor,
        "in" => TokenKind::KwIn,
        "break" => TokenKind::KwBreak,
        "continue" => TokenKind::KwContinue,
        "match" => TokenKind::KwMatch,
        "return" => TokenKind::KwReturn,
        "defer" => TokenKind::KwDefer,
        "arena" => TokenKind::KwArena,
        "grad" => TokenKind::KwGrad,
        "on" => TokenKind::KwOn,
        "test" => TokenKind::KwTest,
        "true" => TokenKind::KwTrue,
        "false" => TokenKind::KwFalse,
        "none" => TokenKind::KwNone,
        _ => return None,
    };
    Some(kind)
}
