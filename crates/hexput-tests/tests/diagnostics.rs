//! Diagnostics tests (Epic 1 Story 1.2) — the workspace-wide error shape.
//!
//! The category and code strings are a declared external contract: Backends match on them,
//! and the doc comments promise an existing code's string never changes. Pin them literally,
//! because comparing a constant against itself would not catch a rename.

use hexput_shared::diagnostics::{Category, Code, Diagnostic, Span};

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
