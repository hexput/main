//! Category, Code, Diagnostic, Span — the one error/finding shape shared by the language
//! (LANGUAGE-REFERENCE §7) and by `hexput-port`'s error responses.
//!
//! Story 1.2 lands the shape itself; Story 1.8 owns *rendering* it (source line, caret) and must
//! be able to do so without re-scanning the source, which is why [`Span`] carries a line and a
//! column alongside the byte offset.

use core::fmt;

/// A half-open region of source text, recorded three ways.
///
/// * `offset`/`len` are **bytes**, so `&source[span.range()]` slices the original text.
/// * `line` is 1-based.
/// * `column` is 1-based and counted in **Unicode scalar values, not bytes** — a column has to
///   mean what a human sees, and while identifiers are ASCII, strings and comments are full UTF-8.
///
/// Deliberately `Copy` and free of any lexer-specific field: `hexput-ast` reuses it verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 0-based byte offset of the first byte of the span.
    pub offset: usize,
    /// Length of the span in bytes. Zero is legal (an empty span points between two characters).
    pub len: usize,
    /// 1-based line number of the first character.
    pub line: usize,
    /// 1-based column, counted in Unicode scalar values.
    pub column: usize,
}

impl Span {
    /// Construct a span from its byte offset, byte length, 1-based line and 1-based column.
    #[must_use]
    pub const fn new(offset: usize, len: usize, line: usize, column: usize) -> Self {
        Self {
            offset,
            len,
            line,
            column,
        }
    }

    /// Byte offset one past the end of the span.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.offset + self.len
    }

    /// The span as a byte range, suitable for slicing the original source.
    #[must_use]
    pub const fn range(&self) -> core::ops::Range<usize> {
        self.offset..self.end()
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Failure categories from LANGUAGE-REFERENCE §7. Every diagnostic the workspace produces —
/// lexical, syntactic, runtime, or static-check finding — carries exactly one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Unterminated string, unknown character. Detected at lex time.
    Lexical,
    /// Malformed construct, `break` outside a loop, duplicate `let`. Detected at parse time.
    Syntax,
    /// A conversion §4.2 does not perform. Detected at runtime.
    Type,
    /// Undeclared identifier, property access on `null`. Detected at runtime.
    Reference,
    /// Wrong argument count. Detected at runtime.
    Arity,
    /// Division by zero, non-finite result. Detected at runtime.
    Arithmetic,
    /// Call-depth limit exceeded. Detected at runtime.
    Depth,
    /// Call to an unregistered or denied Registered Function. Detected at runtime.
    Capability,
    /// A Resource Budget dimension exceeded. Detected at runtime.
    Budget,
    /// A disabled language construct was used. Detected at parse time or runtime.
    Policy,
}

impl Category {
    /// The category's wire spelling — the lowercase name used in §7 and in error responses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Syntax => "syntax",
            Self::Type => "type",
            Self::Reference => "reference",
            Self::Arity => "arity",
            Self::Arithmetic => "arithmetic",
            Self::Depth => "depth",
            Self::Capability => "capability",
            Self::Budget => "budget",
            Self::Policy => "policy",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stable, machine-readable identifier for one specific failure.
///
/// Stable in the sense that Backends may match on it: the string of an existing code never
/// changes. New codes are added as associated constants next to the ones below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Code(&'static str);

impl Code {
    /// Define a code. Kept `const` so codes are associated constants, not runtime strings.
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self(code)
    }

    /// The code's stable string form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    // --- `lexical` codes (Story 1.2) ---

    /// A string literal ran to a raw newline or to end of input without its closing quote.
    pub const UNTERMINATED_STRING: Self = Self::new("lex.unterminated_string");
    /// A `/*` block comment ran to end of input without its closing `*/`.
    pub const UNTERMINATED_COMMENT: Self = Self::new("lex.unterminated_comment");
    /// A character that begins no token appeared outside a string or comment.
    pub const UNKNOWN_CHARACTER: Self = Self::new("lex.unknown_character");
    /// A non-ASCII letter or digit appeared where an identifier was being read (§2).
    pub const NON_ASCII_IDENTIFIER: Self = Self::new("lex.non_ascii_identifier");
    /// A `\` escape in a string literal names no escape the language defines (§3).
    pub const INVALID_ESCAPE: Self = Self::new("lex.invalid_escape");
    /// A `\u{...}` escape is malformed or names no Unicode scalar value.
    pub const INVALID_UNICODE_ESCAPE: Self = Self::new("lex.invalid_unicode_escape");
    /// A numeric literal is not representable as a finite f64 (§3 — infinity is never a value).
    pub const INVALID_NUMBER: Self = Self::new("lex.invalid_number");
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// One failure or static-check finding: what kind, which specific one, what to tell a human, and
/// where in the source it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The §7 category.
    pub category: Category,
    /// The stable code within that category.
    pub code: Code,
    /// Human-readable message. Story 1.8 renders it with the source line the span points at.
    pub message: String,
    /// Where the failure is, including its extent — Story 1.8 marks the span, not just its start.
    pub span: Span,
}

impl Diagnostic {
    /// Build a diagnostic.
    #[must_use]
    pub fn new(category: Category, code: Code, message: impl Into<String>, span: Span) -> Self {
        Self {
            category,
            code,
            message: message.into(),
            span,
        }
    }

    /// Build a [`Category::Lexical`] diagnostic — the only category Story 1.2 can produce.
    #[must_use]
    pub fn lexical(code: Code, message: impl Into<String>, span: Span) -> Self {
        Self::new(Category::Lexical, code, message, span)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} error [{}] at {}: {}",
            self.category, self.code, self.span, self.message
        )
    }
}

impl core::error::Error for Diagnostic {}
