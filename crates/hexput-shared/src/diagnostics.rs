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

#[cfg(test)]
mod tests {
    use super::*;

    /// The category strings are a declared external contract — Backends match on them — so pin
    /// every one. Comparing a constant against itself would not catch a rename.
    #[test]
    fn category_wire_strings_are_stable() {
        for (category, expected) in [
            (Category::Lexical, "lexical"),
            (Category::Syntax, "syntax"),
            (Category::Type, "type"),
            (Category::Reference, "reference"),
            (Category::Arity, "arity"),
            (Category::Arithmetic, "arithmetic"),
            (Category::Depth, "depth"),
            (Category::Capability, "capability"),
            (Category::Budget, "budget"),
            (Category::Policy, "policy"),
        ] {
            assert_eq!(category.as_str(), expected);
            assert_eq!(category.to_string(), expected);
        }
    }

    /// Likewise for codes: "the string of an existing code never changes".
    #[test]
    fn code_strings_are_stable() {
        for (code, expected) in [
            (Code::UNTERMINATED_STRING, "lex.unterminated_string"),
            (Code::UNTERMINATED_COMMENT, "lex.unterminated_comment"),
            (Code::UNKNOWN_CHARACTER, "lex.unknown_character"),
            (Code::NON_ASCII_IDENTIFIER, "lex.non_ascii_identifier"),
            (Code::INVALID_ESCAPE, "lex.invalid_escape"),
            (Code::INVALID_UNICODE_ESCAPE, "lex.invalid_unicode_escape"),
            (Code::INVALID_NUMBER, "lex.invalid_number"),
        ] {
            assert_eq!(code.as_str(), expected);
            assert_eq!(code.to_string(), expected);
        }
    }

    #[test]
    fn span_end_and_range_describe_the_same_region() {
        let span = Span::new(4, 3, 2, 5);
        assert_eq!(span.end(), 7);
        assert_eq!(span.range(), 4..7);
        assert_eq!(&"0123456789"[span.range()], "456");
    }

    #[test]
    fn an_empty_span_is_legal_and_points_between_characters() {
        let span = Span::new(2, 0, 1, 3);
        assert_eq!(span.end(), 2);
        assert!(span.range().is_empty());
    }

    #[test]
    fn span_displays_as_line_and_column() {
        assert_eq!(Span::new(40, 2, 7, 12).to_string(), "7:12");
    }

    #[test]
    fn diagnostic_display_carries_category_code_position_and_message() {
        let d = Diagnostic::lexical(
            Code::UNKNOWN_CHARACTER,
            "unexpected character `$`",
            Span::new(2, 1, 1, 3),
        );
        assert_eq!(
            d.to_string(),
            "lexical error [lex.unknown_character] at 1:3: unexpected character `$`"
        );
    }

    #[test]
    fn the_lexical_constructor_sets_the_category() {
        let d = Diagnostic::lexical(Code::INVALID_NUMBER, "x", Span::new(0, 1, 1, 1));
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(
            d,
            Diagnostic::new(
                Category::Lexical,
                Code::INVALID_NUMBER,
                "x",
                Span::new(0, 1, 1, 1)
            )
        );
    }
}
