//! The `--emit tokens` output format: one `L:C KIND` line per token.

use lexer::{Token, TokenKind};
use source::SourceFile;

/// Formats every token as `line:column kind`, one per line.
pub fn format_tokens(tokens: &[Token], file: &SourceFile) -> String {
    let mut text = String::new();
    for token in tokens {
        let (line, column) = file.line_col(token.span.start);
        let kind = describe(&token.kind);
        text.push_str(&format!("{line}:{column} {kind}\n"));
    }
    text
}

/// Describes a token: names and literals show their value, other tokens their bare spelling.
fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(name) => format!("name {name}"),
        TokenKind::Int(value) => format!("int {value}"),
        TokenKind::Float(value) => format!("float {value}"),
        TokenKind::Str(raw) => format!("string \"{raw}\""),
        // Fixed tokens have a spelling; names and literals were handled above.
        TokenKind::Newline => "newline".to_string(),
        TokenKind::Eof => "end of file".to_string(),
        fixed => fixed.spelling().unwrap_or_default().to_string(),
    }
}
