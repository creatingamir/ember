//! Source files, byte spans and compile errors shared by every compiler stage.

/// Byte offsets into a source text, half-open: `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Offset of the first byte.
    pub start: usize,
    /// Offset one past the last byte.
    pub end: usize,
}

impl Span {
    /// Creates a span from its start and end offsets.
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }
}

/// One source file: its display name, its text, and where each line starts.
pub struct SourceFile {
    name: String,
    text: String,
    /// Byte offset of the first byte of each line; the first entry is always 0.
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Creates a source file and indexes where its lines start.
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
        let text = text.into();

        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }

        SourceFile {
            name: name.into(),
            text,
            line_starts,
        }
    }

    /// Returns the display name of the file.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the full text of the file.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the 1-based line and column of a byte offset.
    ///
    /// The column counts chars, so a tab and a multi-byte char both count as 1.
    /// Offsets past the end of the text are clamped to the end.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());

        // Err(i) means `offset` falls inside line i - 1, so the line index is i - 1.
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];

        let mut column = 1;
        for (char_offset, _) in self.text[line_start..].char_indices() {
            if line_start + char_offset >= offset {
                break;
            }
            column += 1;
        }

        (line_index + 1, column)
    }

    /// Returns the text of a 1-based line without its line ending, or "" if there is no such line.
    pub fn line_text(&self, line: usize) -> &str {
        if line == 0 || line > self.line_starts.len() {
            return "";
        }

        let start = self.line_starts[line - 1];
        let end = match self.line_starts.get(line) {
            Some(next_start) => *next_start,
            None => self.text.len(),
        };

        let with_ending = &self.text[start..end];
        let without_newline = with_ending.strip_suffix('\n').unwrap_or(with_ending);
        without_newline
            .strip_suffix('\r')
            .unwrap_or(without_newline)
    }
}

/// A compile error with the span it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What went wrong, in one line.
    pub message: String,
    /// The part of the source the error points at.
    pub span: Span,
}

impl Diagnostic {
    /// Creates an error diagnostic.
    pub fn error(message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            message: message.into(),
            span,
        }
    }

    /// Renders the error for people: a header, the source line, and a caret under the span start.
    pub fn render(&self, file: &SourceFile) -> String {
        let (line, column) = file.line_col(self.span.start);
        let line_text = file.line_text(line);

        // Copy tabs from the source line so the caret lines up under tab-indented code.
        let mut padding = String::new();
        for ch in line_text.chars().take(column - 1) {
            if ch == '\t' {
                padding.push('\t');
            } else {
                padding.push(' ');
            }
        }

        format!(
            "{}:{line}:{column}: error: {}\n    {line_text}\n    {padding}^\n",
            file.name(),
            self.message,
        )
    }
}
