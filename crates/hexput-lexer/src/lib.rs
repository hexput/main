#![forbid(clippy::undocumented_unsafe_blocks)]

//! Tokenizer for the Hexput language (Epic 1 Story 1.2). Depends on hexput-shared only.
//! Binds no Architecture Decision directly; part of the language pipeline that pre-dates the FRs
//! the ADs govern.
//!
//! [`tokenize`] turns Hexput source text into a flat, context-free stream of position-carrying
//! [`Token`]s, as defined by LANGUAGE-REFERENCE §2–§6. Comments and whitespace are discarded
//! without shifting the recorded position of any surrounding token. There is no `Eof` token: the
//! parser detects end of input from the slice ending, so empty source yields an empty stream.
//!
//! String literals are multi-line (§3): a raw newline between the quotes is ordinary content, so
//! a string is unterminated only at end of input and its span may cover several lines. A leading
//! UTF-8 BOM is skipped without disturbing byte offsets.
//!
//! The first malformed input aborts tokenization with a `lexical` [`Diagnostic`]; there is no
//! error recovery and no multi-error collection.

use hexput_shared::diagnostics::{Code, Diagnostic, Span};

/// One lexed token: what it is, and where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What the token is, with its decoded payload for literals and identifiers.
    pub kind: TokenKind,
    /// Where in the source the token was written, including its extent.
    pub span: Span,
}

impl Token {
    /// Pair a kind with its span.
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Every token the Hexput grammar admits: literals, identifiers, the fourteen reserved words of
/// §2, the operators of §4, and the punctuation of §5, §6 and §9.
///
/// Payloads are owned rather than borrowed from the source: a borrowing lexer would push a
/// lifetime parameter through the AST, the parser and the AST cache, for a gain the cache (which
/// parses each script once) makes marginal.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // --- literals and identifiers (§2, §3) ---
    /// A numeric literal, already parsed to its finite f64 value.
    Number(f64),
    /// A string literal, with every escape already decoded and the quotes removed.
    Str(String),
    /// An identifier: ASCII letters, digits and `_`, not starting with a digit.
    Ident(String),

    // --- reserved words (§2) ---
    /// `let`
    Let,
    /// `fn`
    Fn,
    /// `if`
    If,
    /// `else`
    Else,
    /// `while`
    While,
    /// `for`
    For,
    /// `in`
    In,
    /// `return`
    Return,
    /// `break`
    Break,
    /// `continue`
    Continue,
    /// `true`
    True,
    /// `false`
    False,
    /// `null`
    Null,
    /// `plugin`
    Plugin,

    // --- operators (§4) ---
    /// `+`
    Plus,
    /// `-` — always its own token, never part of a numeric literal (except an exponent sign).
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `!`
    Bang,
    /// `=`
    Assign,
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,
    /// `.`
    Dot,
    /// `?.` — one token, covering both `a?.b` and `a?.[b]` (§4.4).
    QuestionDot,

    // --- punctuation (§5, §6, §9) ---
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `:` — object literal key/value separator.
    Colon,
    /// `@` — introduces a plugin annotation, `@Event` / `@Global` (§9).
    At,
}

/// Turn Hexput source into tokens.
///
/// Returns every token in source order, each carrying a [`Span`]. Whitespace and both comment
/// forms are discarded. Empty or comment-only source yields an empty vector.
///
/// # Errors
///
/// Returns the first `lexical` [`Diagnostic`] encountered — unterminated string or block comment,
/// unknown escape, malformed `\u{...}`, a numeric literal that is not a finite f64, or a character
/// that begins no token — and stops. No input is silently skipped, and no input panics.
pub fn tokenize(source: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer::new(source).run()
}

/// A cursor position: the byte offset, 1-based line, and 1-based scalar-value column of the next
/// character to be read. Captured before a token starts so the token's span can be closed later.
#[derive(Debug, Clone, Copy)]
struct Mark {
    offset: usize,
    line: usize,
    column: usize,
}

struct Lexer<'a> {
    source: &'a str,
    /// `(byte offset, character)` for every scalar value in the source. Indexing this rather than
    /// walking a `Peekable<CharIndices>` keeps arbitrary lookahead (needed for `1e-3` and for
    /// `1.` vs `1.5`) trivial and total.
    chars: Vec<(usize, char)>,
    /// Index into `chars`, not a byte offset.
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        let chars: Vec<(usize, char)> = source.char_indices().collect();
        // Skip a leading UTF-8 BOM. It is not `char::is_whitespace`, so without this an editor
        // that writes one would make the whole script fail at 1:1 with `lex.unknown_character`.
        // Only the cursor index moves: `source` and every recorded byte offset stay relative to
        // the original text, so Story 1.8 can still slice it.
        let pos = usize::from(matches!(chars.first(), Some(&(_, '\u{feff}'))));
        Self {
            source,
            chars,
            pos,
            line: 1,
            column: 1,
        }
    }

    // --- cursor primitives ---

    fn peek(&self) -> Option<char> {
        self.peek_at(0)
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).map(|&(_, c)| c)
    }

    /// Byte offset of the next character, or the source length at end of input.
    fn byte_offset(&self) -> usize {
        self.chars
            .get(self.pos)
            .map_or(self.source.len(), |&(offset, _)| offset)
    }

    fn mark(&self) -> Mark {
        Mark {
            offset: self.byte_offset(),
            line: self.line,
            column: self.column,
        }
    }

    /// Consume one character, keeping the line/column counters honest. Columns count scalar
    /// values, so this advances by one per character regardless of its UTF-8 width.
    ///
    /// Line breaks are `\n`, `\r\n`, and a lone `\r`. In `\r\n` the `\r` does not increment — the
    /// `\n` that follows does — so the pair counts once. U+2028/U+2029 are deliberately *not*
    /// line breaks here, matching Rust and the LSP position model.
    fn bump(&mut self) -> Option<char> {
        let (_, c) = *self.chars.get(self.pos)?;
        self.pos += 1;
        let ends_line = c == '\n' || (c == '\r' && self.peek() != Some('\n'));
        if ends_line {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    /// Consume one character if it is `expected`, reporting whether it did.
    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Close a span that started at `start` and ends at the current position.
    fn span_from(&self, start: Mark) -> Span {
        Span::new(
            start.offset,
            self.byte_offset().saturating_sub(start.offset),
            start.line,
            start.column,
        )
    }

    /// A span covering exactly the `len_in_bytes` bytes at `start` — used for errors that must
    /// point at where a construct *opened* rather than where it failed.
    fn span_at(start: Mark, len_in_bytes: usize) -> Span {
        Span::new(start.offset, len_in_bytes, start.line, start.column)
    }

    fn error(code: Code, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic::lexical(code, message, span)
    }

    // --- driver ---

    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia()?;
            let start = self.mark();
            let Some(c) = self.peek() else {
                return Ok(tokens);
            };
            let kind = match c {
                '0'..='9' => self.lex_number(start)?,
                'a'..='z' | 'A'..='Z' | '_' => self.lex_word(),
                '"' | '\'' => self.lex_string(start)?,
                _ => self.lex_operator(start)?,
            };
            tokens.push(Token::new(kind, self.span_from(start)));
        }
    }

    /// Discard whitespace and comments. Positions are tracked while discarding, so nothing here
    /// shifts the recorded position of the tokens on either side.
    fn skip_trivia(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    // Runs to end of line; the break itself is left for the whitespace arm so
                    // the line counter is advanced in exactly one place.
                    while let Some(c) = self.peek() {
                        if c == '\n' || c == '\r' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let start = self.mark();
                    self.bump();
                    self.bump();
                    loop {
                        match self.bump() {
                            // Non-nesting (§2): the first `*/` closes the comment.
                            Some('*') if self.eat('/') => break,
                            Some(_) => {}
                            None => {
                                return Err(Self::error(
                                    Code::UNTERMINATED_COMMENT,
                                    "unterminated block comment: reached end of input without `*/`",
                                    Self::span_at(start, 2),
                                ));
                            }
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    // --- token scanners ---

    /// An identifier or, when it spells one, a reserved word (§2). Reserved words never lex as
    /// identifiers.
    fn lex_word(&mut self) -> TokenKind {
        let start_byte = self.byte_offset();
        while matches!(self.peek(), Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_')) {
            self.bump();
        }
        let text = &self.source[start_byte..self.byte_offset()];
        match text {
            "let" => TokenKind::Let,
            "fn" => TokenKind::Fn,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "plugin" => TokenKind::Plugin,
            _ => TokenKind::Ident(text.to_owned()),
        }
    }

    /// A numeric literal (§3). A leading `-` is never part of it — `-3` is unary minus applied to
    /// `3` (§4), which is exactly what makes `a-3` lex as three tokens. The one `-` a literal may
    /// contain is an exponent sign.
    ///
    /// A literal must start with a digit, so `.5` is `Dot` then `Number(5)`, not `0.5`; the
    /// parser rejects it. Trailing `.` behaves the mirror way: `1.` is `Number(1)` then `Dot`.
    fn lex_number(&mut self, start: Mark) -> Result<TokenKind, Diagnostic> {
        let start_byte = self.byte_offset();
        self.eat_digits();

        // A `.` joins the literal only when a digit follows, so `1.5` is one number while `1.` is
        // `Number(1)` followed by `Dot`.
        if self.peek() == Some('.') && matches!(self.peek_at(1), Some('0'..='9')) {
            self.bump();
            self.eat_digits();
        }

        // Likewise an exponent joins only when it is complete. An incomplete one leaves the `e`
        // abutting the literal, which the check below then rejects.
        if matches!(self.peek(), Some('e' | 'E')) {
            let sign_width = usize::from(matches!(self.peek_at(1), Some('+' | '-')));
            if matches!(self.peek_at(1 + sign_width), Some('0'..='9')) {
                self.bump();
                for _ in 0..sign_width {
                    self.bump();
                }
                self.eat_digits();
            }
        }

        // A literal butting straight up against an identifier character is a typo, not two
        // tokens: without this, `0x10` would quietly lex as `Number(0)` + `Ident("x10")` and
        // `1_000` as `Number(1)` + `Ident("_000")`, surfacing later as a baffling syntax error
        // somewhere else. The language has neither hex literals nor digit separators (§3).
        if let Some(c) = self.peek()
            && (c.is_alphanumeric() || c == '_')
        {
            self.bump();
            return Err(Self::error(
                Code::INVALID_NUMBER,
                format!(
                    "`{}` directly after a number literal: a number is decimal digits with an \
                     optional `.` and a complete exponent — there are no hex literals and no \
                     digit separators",
                    c.escape_debug()
                ),
                self.span_from(start),
            ));
        }

        let text = &self.source[start_byte..self.byte_offset()];
        let span = self.span_from(start);
        // Every string the scanner accepts above is valid f64 syntax, so a parse failure is
        // impossible; out-of-range input parses to an infinity instead, which the check below
        // catches. §3: infinity is never a value, so reject it here rather than letting it
        // surface as an `arithmetic` surprise at runtime.
        let value: f64 = text
            .parse()
            .expect("the scanner above only accepts valid f64 syntax");
        if !value.is_finite() {
            return Err(Self::error(
                Code::INVALID_NUMBER,
                format!("`{text}` is out of range for a number (must be a finite value)"),
                span,
            ));
        }
        Ok(TokenKind::Number(value))
    }

    fn eat_digits(&mut self) {
        while matches!(self.peek(), Some('0'..='9')) {
            self.bump();
        }
    }

    /// A string literal in either quote style (§3), with escapes decoded.
    ///
    /// String literals are **multi-line**: a raw newline between the quotes is ordinary content,
    /// kept verbatim in the value. A string is therefore unterminated only at end of input, never
    /// at end of line. [`Self::bump`] keeps the line counter honest while scanning the body, so a
    /// token written after a multi-line string still reports its true line, and the string's own
    /// span simply covers several lines (Story 1.8 renders multi-line spans).
    fn lex_string(&mut self, start: Mark) -> Result<TokenKind, Diagnostic> {
        let quote = self.bump().expect("caller peeked an opening quote");
        let mut value = String::new();
        loop {
            let escape_start = self.mark();
            match self.bump() {
                Some(c) if c == quote => return Ok(TokenKind::Str(value)),
                Some('\\') => value.push(self.lex_escape(escape_start)?),
                None => {
                    return Err(Self::error(
                        Code::UNTERMINATED_STRING,
                        format!(
                            "unterminated string literal: reached end of input without a closing `{quote}`"
                        ),
                        Self::span_at(start, quote.len_utf8()),
                    ));
                }
                Some(c) => value.push(c),
            }
        }
    }

    /// The body of an escape sequence, with the `\` already consumed. `escape_start` marks the
    /// backslash, so errors point at the escape rather than at the character after it.
    fn lex_escape(&mut self, escape_start: Mark) -> Result<char, Diagnostic> {
        match self.bump() {
            Some('n') => Ok('\n'),
            Some('t') => Ok('\t'),
            Some('r') => Ok('\r'),
            Some('\\') => Ok('\\'),
            Some('"') => Ok('"'),
            Some('\'') => Ok('\''),
            Some('u') => self.lex_unicode_escape(escape_start),
            // Reported here, at the backslash, rather than deferred to the caller: a truncated
            // escape is a more precise complaint than "unterminated string" would be. There is
            // no line-continuation escape (§3), so `\` before a newline lands here too.
            Some('\n' | '\r') | None => Err(Self::error(
                Code::INVALID_ESCAPE,
                "incomplete escape sequence: `\\` must be followed by one of `n t r \\ \" ' u`",
                self.span_from(escape_start),
            )),
            Some(c) => Err(Self::error(
                Code::INVALID_ESCAPE,
                format!("unknown escape sequence `\\{}`", c.escape_debug()),
                self.span_from(escape_start),
            )),
        }
    }

    /// `\u{...}`, with `\u` already consumed. Accepts 1–6 hex digits naming a Unicode scalar
    /// value; anything else — no braces, no digits, too many digits, a surrogate, or a value above
    /// `U+10FFFF` — is an error at the escape.
    fn lex_unicode_escape(&mut self, escape_start: Mark) -> Result<char, Diagnostic> {
        let bad = |lexer: &Self, detail: &str| {
            Self::error(
                Code::INVALID_UNICODE_ESCAPE,
                format!("invalid `\\u{{...}}` escape: {detail}"),
                lexer.span_from(escape_start),
            )
        };

        if !self.eat('{') {
            return Err(bad(self, "expected `{` after `\\u`"));
        }
        let mut digits = String::new();
        loop {
            match self.peek() {
                Some('}') => {
                    self.bump();
                    break;
                }
                Some(c) if c.is_ascii_hexdigit() => {
                    self.bump();
                    digits.push(c);
                    if digits.len() > 6 {
                        return Err(bad(self, "expected at most 6 hex digits"));
                    }
                }
                // Reported here rather than running on to the end of the string: a `\u{` that
                // never closes is a more precise complaint than the unterminated string it
                // would otherwise become.
                Some('\n' | '\r') | None => {
                    return Err(bad(self, "expected `}` to close the escape"));
                }
                Some(c) => {
                    return Err(bad(
                        self,
                        &format!("`{}` is not a hex digit", c.escape_debug()),
                    ));
                }
            }
        }
        if digits.is_empty() {
            return Err(bad(self, "expected at least one hex digit"));
        }
        let Ok(scalar) = u32::from_str_radix(&digits, 16) else {
            return Err(bad(self, "value is out of range"));
        };
        // Rejects surrogates (U+D800..=U+DFFF) and anything above U+10FFFF.
        char::from_u32(scalar)
            .ok_or_else(|| bad(self, &format!("U+{digits} is not a Unicode scalar value")))
    }

    /// Operators and punctuation, maximal munch on every ambiguous prefix (§4). `/` reaching here
    /// is division: [`Self::skip_trivia`] has already consumed `//` and `/* ... */`.
    fn lex_operator(&mut self, start: Mark) -> Result<TokenKind, Diagnostic> {
        let c = self.bump().expect("caller peeked a character");
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '=' if self.eat('=') => TokenKind::EqEq,
            '=' => TokenKind::Assign,
            '!' if self.eat('=') => TokenKind::BangEq,
            '!' => TokenKind::Bang,
            '<' if self.eat('=') => TokenKind::LtEq,
            '<' => TokenKind::Lt,
            '>' if self.eat('=') => TokenKind::GtEq,
            '>' => TokenKind::Gt,
            // `&` and `|` exist only doubled: there are no bitwise operators (§11).
            '&' if self.eat('&') => TokenKind::AmpAmp,
            '|' if self.eat('|') => TokenKind::PipePipe,
            // `?` exists only as `?.` (§4.4): there is no ternary (§11).
            '?' if self.eat('.') => TokenKind::QuestionDot,
            '.' => TokenKind::Dot,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,
            '@' => TokenKind::At,
            _ => return Err(self.unknown_character(c, start)),
        };
        Ok(kind)
    }

    /// The position lives in the span, not in the message: Story 1.8 renders the span with a
    /// caret, so repeating line/column in the text would print it twice. `escape_debug` keeps a
    /// raw control character out of a message that travels over the wire and into logs.
    fn unknown_character(&self, c: char, start: Mark) -> Diagnostic {
        let span = Self::span_at(start, c.len_utf8());
        // §2 restricts identifiers to ASCII, so a non-ASCII letter or digit is almost always
        // someone writing `café` rather than a stray symbol. Say so.
        if !c.is_ascii() && c.is_alphanumeric() {
            return Self::error(
                Code::NON_ASCII_IDENTIFIER,
                format!(
                    "non-ASCII character `{}` in an identifier: identifiers may contain only \
                     ASCII letters, digits and `_`",
                    c.escape_debug()
                ),
                span,
            );
        }
        Self::error(
            Code::UNKNOWN_CHARACTER,
            format!("unexpected character `{}`", c.escape_debug()),
            span,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Token, TokenKind, tokenize};
    use hexput_shared::diagnostics::{Category, Code, Span};

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .expect("expected source to tokenize")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    fn toks(source: &str) -> Vec<Token> {
        tokenize(source).expect("expected source to tokenize")
    }

    fn err(source: &str) -> hexput_shared::diagnostics::Diagnostic {
        tokenize(source).expect_err("expected source to fail lexing")
    }

    fn ident(name: &str) -> TokenKind {
        TokenKind::Ident(name.to_owned())
    }

    fn string(value: &str) -> TokenKind {
        TokenKind::Str(value.to_owned())
    }

    // --- matrix: empty and trivia-only ---

    #[test]
    fn empty_source_yields_empty_stream() {
        assert_eq!(tokenize(""), Ok(vec![]));
    }

    #[test]
    fn whitespace_and_comments_only_yield_empty_stream() {
        assert_eq!(tokenize("  // hi\n/* x */"), Ok(vec![]));
        assert_eq!(tokenize("\n\n\t  \r\n"), Ok(vec![]));
    }

    #[test]
    fn a_leading_bom_is_skipped_without_shifting_offsets() {
        let tokens = toks("\u{feff}let");
        assert_eq!(tokens[0].kind, TokenKind::Let);
        // The BOM is 3 bytes, so the offset still indexes the ORIGINAL source; the column does
        // not count it, because a BOM is not something a human sees.
        assert_eq!(tokens[0].span, Span::new(3, 3, 1, 1));
        // A BOM anywhere else is still an unknown character.
        assert_eq!(err("a\u{feff}b").code, Code::UNKNOWN_CHARACTER);
    }

    #[test]
    fn crlf_counts_as_one_line_break_and_a_lone_cr_counts_as_one() {
        assert_eq!(toks("a\r\nb")[1].span.line, 2, "CRLF must count once");
        assert_eq!(toks("a\rb")[1].span.line, 2, "a lone CR ends a line");
        assert_eq!(toks("a\r\n\r\nb")[1].span.line, 3);
        // A `//` comment ends at a CR just as it does at a newline.
        assert_eq!(kinds("// c\rb"), vec![ident("b")]);
    }

    #[test]
    fn block_comment_does_not_nest() {
        // The inner `/*` opens nothing, so the single `*/` closes the whole comment and `c` is
        // left as ordinary source.
        assert_eq!(
            kinds("/* a /* b */ c"),
            vec![ident("c")],
            "the first `*/` must close the comment"
        );
    }

    // --- matrix: positions ---

    #[test]
    fn position_after_comment() {
        let tokens = toks("// c\nx");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, ident("x"));
        assert_eq!(tokens[0].span, Span::new(5, 1, 2, 1));
    }

    #[test]
    fn discarded_trivia_does_not_shift_neighbouring_positions() {
        let with = toks("a /* gap */ b");
        let expected_b = Span::new(12, 1, 1, 13);
        assert_eq!(with[0].span, Span::new(0, 1, 1, 1));
        assert_eq!(with[1].span, expected_b);
        // The span still slices the original source correctly.
        assert_eq!(&"a /* gap */ b"[with[1].span.range()], "b");
    }

    #[test]
    fn positions_across_multiple_lines() {
        let source = "let a = 1;\n  b == c;\n\nreturn;";
        let tokens = toks(source);
        let located: Vec<(&TokenKind, usize, usize, usize)> = tokens
            .iter()
            .map(|t| (&t.kind, t.span.offset, t.span.line, t.span.column))
            .collect();
        assert_eq!(
            located,
            vec![
                (&TokenKind::Let, 0, 1, 1),
                (&ident("a"), 4, 1, 5),
                (&TokenKind::Assign, 6, 1, 7),
                (&TokenKind::Number(1.0), 8, 1, 9),
                (&TokenKind::Semicolon, 9, 1, 10),
                (&ident("b"), 13, 2, 3),
                (&TokenKind::EqEq, 15, 2, 5),
                (&ident("c"), 18, 2, 8),
                (&TokenKind::Semicolon, 19, 2, 9),
                (&TokenKind::Return, 22, 4, 1),
                (&TokenKind::Semicolon, 28, 4, 7),
            ]
        );
        for token in &tokens {
            assert!(
                source.get(token.span.range()).is_some(),
                "every span must slice the source: {token:?}"
            );
        }
    }

    #[test]
    fn spans_record_token_extent_not_just_start() {
        let tokens = toks("continue >= \"ab\"");
        assert_eq!(tokens[0].span.len, "continue".len());
        assert_eq!(tokens[1].span.len, 2);
        assert_eq!(tokens[2].span.len, 4, "a string's span covers its quotes");
    }

    // --- matrix: words ---

    #[test]
    fn keyword_versus_identifier() {
        assert_eq!(kinds("let letter"), vec![TokenKind::Let, ident("letter")]);
        assert_eq!(
            kinds("_in in2 in"),
            vec![ident("_in"), ident("in2"), TokenKind::In]
        );
    }

    #[test]
    fn every_reserved_word_has_its_own_kind() {
        assert_eq!(
            kinds("let fn if else while for in return break continue true false null plugin"),
            vec![
                TokenKind::Let,
                TokenKind::Fn,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::While,
                TokenKind::For,
                TokenKind::In,
                TokenKind::Return,
                TokenKind::Break,
                TokenKind::Continue,
                TokenKind::True,
                TokenKind::False,
                TokenKind::Null,
                TokenKind::Plugin,
            ]
        );
    }

    // --- matrix: maximal munch ---

    #[test]
    fn maximal_munch_on_ambiguous_prefixes() {
        assert_eq!(kinds("a==b"), vec![ident("a"), TokenKind::EqEq, ident("b")]);
        assert_eq!(
            kinds("a=b"),
            vec![ident("a"), TokenKind::Assign, ident("b")]
        );
        assert_eq!(
            kinds("!a != b"),
            vec![TokenKind::Bang, ident("a"), TokenKind::BangEq, ident("b")]
        );
        assert_eq!(
            kinds("a<=b<c"),
            vec![
                ident("a"),
                TokenKind::LtEq,
                ident("b"),
                TokenKind::Lt,
                ident("c")
            ]
        );
        assert_eq!(
            kinds("a>=b>c"),
            vec![
                ident("a"),
                TokenKind::GtEq,
                ident("b"),
                TokenKind::Gt,
                ident("c")
            ]
        );
        assert_eq!(
            kinds("a&&b||c"),
            vec![
                ident("a"),
                TokenKind::AmpAmp,
                ident("b"),
                TokenKind::PipePipe,
                ident("c")
            ]
        );
        // `/` vs `//` vs `/*`
        assert_eq!(
            kinds("a/b // c\n/* d */ e"),
            vec![ident("a"), TokenKind::Slash, ident("b"), ident("e")]
        );
    }

    #[test]
    fn every_operator_and_punctuator_lexes() {
        assert_eq!(
            kinds("+ - * / % ! = == != < <= > >= && || . ?. ( ) { } [ ] , ; : @"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Bang,
                TokenKind::Assign,
                TokenKind::EqEq,
                TokenKind::BangEq,
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
                TokenKind::AmpAmp,
                TokenKind::PipePipe,
                TokenKind::Dot,
                TokenKind::QuestionDot,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::Comma,
                TokenKind::Semicolon,
                TokenKind::Colon,
                TokenKind::At,
            ]
        );
    }

    // --- matrix: numbers ---

    #[test]
    fn unary_minus_is_never_part_of_a_literal() {
        assert_eq!(
            kinds("a-3"),
            vec![ident("a"), TokenKind::Minus, TokenKind::Number(3.0)]
        );
        assert_eq!(kinds("-3"), vec![TokenKind::Minus, TokenKind::Number(3.0)]);
    }

    #[test]
    fn exponent_sign_belongs_to_the_literal() {
        assert_eq!(kinds("1e-3"), vec![TokenKind::Number(0.001)]);
        assert_eq!(kinds("1e3"), vec![TokenKind::Number(1000.0)]);
        assert_eq!(kinds("1E+3"), vec![TokenKind::Number(1000.0)]);
        // An incomplete exponent does not join the literal, and the `e` left abutting the number
        // is then rejected rather than becoming an identifier — `1e` is a typo, not `1` times a
        // variable named `e`, and this language has no implicit multiplication.
        assert_eq!(err("1e").code, Code::INVALID_NUMBER);
        assert_eq!(err("1e+").code, Code::INVALID_NUMBER);
    }

    #[test]
    fn dot_joins_a_literal_only_before_a_digit() {
        assert_eq!(kinds("1.5"), vec![TokenKind::Number(1.5)]);
        assert_eq!(kinds("1."), vec![TokenKind::Number(1.0), TokenKind::Dot]);
        assert_eq!(
            kinds("1.a"),
            vec![TokenKind::Number(1.0), TokenKind::Dot, ident("a")]
        );
        assert_eq!(kinds("0.1"), vec![TokenKind::Number(0.1)]);
    }

    #[test]
    fn a_literal_must_start_with_a_digit() {
        // `.5` is not 0.5: a literal starts with a digit, so this is `Dot` then `Number(5)` and
        // the parser is what rejects it.
        assert_eq!(kinds(".5"), vec![TokenKind::Dot, TokenKind::Number(5.0)]);
    }

    #[test]
    fn an_identifier_character_may_not_abut_a_number() {
        // Without this the language's lack of hex literals and digit separators would show up as
        // a baffling syntax error somewhere downstream instead of here.
        for source in ["0x10", "1abc", "1_000", "1e3x"] {
            let d = err(source);
            assert_eq!(d.code, Code::INVALID_NUMBER, "for {source}");
            assert_eq!(d.category, Category::Lexical, "for {source}");
        }
        // A number may still be followed by anything that is not an identifier character.
        assert_eq!(
            kinds("1+a"),
            vec![TokenKind::Number(1.0), TokenKind::Plus, ident("a")]
        );
    }

    #[test]
    fn numeric_overflow_is_a_lexical_error() {
        let d = err("1e999");
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::INVALID_NUMBER);
        assert_eq!(d.span, Span::new(0, 5, 1, 1));
    }

    // --- matrix: optional access ---

    #[test]
    fn optional_access_forms() {
        assert_eq!(
            kinds("a?.b"),
            vec![ident("a"), TokenKind::QuestionDot, ident("b")]
        );
        assert_eq!(
            kinds("a?.[0]"),
            vec![
                ident("a"),
                TokenKind::QuestionDot,
                TokenKind::LBracket,
                TokenKind::Number(0.0),
                TokenKind::RBracket
            ]
        );
    }

    #[test]
    fn a_lone_question_mark_is_an_error() {
        // There is no ternary operator (§11), so `?` only ever introduces `?.`.
        assert_eq!(err("a ? b").code, Code::UNKNOWN_CHARACTER);
    }

    // --- matrix: strings ---

    #[test]
    fn string_escapes_decode() {
        assert_eq!(kinds(r#""a\n\u{1F600}""#), vec![string("a\n\u{1F600}")]);
        assert_eq!(kinds(r#""\t\r\\\"\'""#), vec![string("\t\r\\\"'")]);
    }

    #[test]
    fn both_quote_styles_are_equivalent() {
        assert_eq!(kinds("'x'"), kinds(r#""x""#));
        assert_eq!(kinds("'x'"), vec![string("x")]);
        // The other quote needs no escape inside a literal.
        assert_eq!(kinds("'he said \"hi\"'"), vec![string("he said \"hi\"")]);
    }

    #[test]
    fn non_ascii_string_content_is_supported_and_columns_count_characters() {
        let source = "\"café\" x";
        let tokens = toks(source);
        assert_eq!(tokens[0].kind, string("café"));
        assert_eq!(tokens[0].span, Span::new(0, 7, 1, 1));
        // `café` is 4 characters but 5 bytes: the byte offset and the column must disagree.
        assert_eq!(tokens[1].kind, ident("x"));
        assert_eq!(tokens[1].span, Span::new(8, 1, 1, 8));
    }

    #[test]
    fn multibyte_comment_content_keeps_columns_in_characters() {
        // `✓` is 3 bytes but 1 column, so offset and column must disagree.
        let source = "/* ✓ */ a";
        let tokens = toks(source);
        assert_eq!(tokens[0].span, Span::new(10, 1, 1, 9));
    }

    #[test]
    fn an_escape_is_measured_in_source_width_not_decoded_width() {
        // `\u{1F600}` is 9 source characters decoding to one scalar value: the span must cover
        // the source text, and the next token's column must follow the source, not the value.
        let tokens = toks(r#""\u{1F600}" a"#);
        assert_eq!(tokens[0].kind, string("\u{1F600}"));
        assert_eq!(tokens[0].span, Span::new(0, 11, 1, 1));
        assert_eq!(tokens[1].span, Span::new(12, 1, 1, 13));
    }

    #[test]
    fn unterminated_string_points_at_the_opening_quote() {
        let d = err("let s = \"abc");
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::UNTERMINATED_STRING);
        assert_eq!(d.span, Span::new(8, 1, 1, 9));
    }

    #[test]
    fn string_literals_are_multi_line() {
        // §3: a raw newline between the quotes is ordinary content.
        assert_eq!(kinds("\"ab\ncd\""), vec![string("ab\ncd")]);
        assert_eq!(
            kinds("let a = \"merhaba\nsosis\nben\";"),
            vec![
                TokenKind::Let,
                ident("a"),
                TokenKind::Assign,
                string("merhaba\nsosis\nben"),
                TokenKind::Semicolon,
            ]
        );
    }

    #[test]
    fn a_multi_line_string_spans_lines_and_does_not_disturb_later_positions() {
        let tokens = toks("let a = \"x\ny\";\nb");
        let s = &tokens[3];
        assert_eq!(s.kind, string("x\ny"));
        // The span opens at the quote on line 1 and covers the newline.
        assert_eq!(s.span, Span::new(8, 5, 1, 9));
        // The token after it must report its true line, not line 1.
        let b = tokens.last().expect("trailing identifier");
        assert_eq!(b.kind, ident("b"));
        assert_eq!(b.span.line, 3);
        assert_eq!(b.span.column, 1);
    }

    #[test]
    fn a_string_is_unterminated_only_at_end_of_input() {
        let d = err("\"ab\ncd");
        assert_eq!(d.code, Code::UNTERMINATED_STRING);
        assert_eq!(d.span, Span::new(0, 1, 1, 1));
    }

    #[test]
    fn a_backslash_before_a_newline_is_not_a_line_continuation() {
        let d = err("\"a\\\nb\"");
        assert_eq!(d.code, Code::INVALID_ESCAPE);
        assert_eq!(d.span.line, 1);
    }

    #[test]
    fn unknown_escape_points_at_the_escape() {
        let d = err(r#""a\q""#);
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::INVALID_ESCAPE);
        assert_eq!(d.span, Span::new(2, 2, 1, 3));
    }

    #[test]
    fn bad_unicode_escapes_are_errors() {
        for source in [
            r#""\u{110000}""#,  // above U+10FFFF
            r#""\u{D800}""#,    // lone surrogate
            r#""\u{}""#,        // no digits
            r#""\u{1234567}""#, // too many digits
            "\"\\uABCD\"",      // missing braces
            r#""\u{12G}""#,     // not hex
            r#""\u{12"#,        // unterminated at EOF
        ] {
            let d = err(source);
            assert_eq!(d.category, Category::Lexical, "for {source}");
            assert_eq!(d.code, Code::INVALID_UNICODE_ESCAPE, "for {source}");
        }
        assert_eq!(kinds(r#""\u{10FFFF}""#), vec![string("\u{10FFFF}")]);
    }

    #[test]
    fn escape_at_end_of_input_reports_the_incomplete_escape() {
        // `Category::Lexical` alone would be trivially true for any lexer error, so pin the code
        // and the span: this is the diagnostic a Backend sees for a truncated script.
        let d = err("\"a\\");
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::INVALID_ESCAPE);
        assert_eq!(d.span, Span::new(2, 1, 1, 3));
    }

    // --- matrix: unknown characters ---

    #[test]
    fn unknown_character_names_the_character_and_position() {
        let d = err("a $ b");
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::UNKNOWN_CHARACTER);
        assert_eq!(d.span, Span::new(2, 1, 1, 3));
        assert!(d.message.contains('$'), "message must name the character");
        // The position belongs to the span alone. Story 1.8 renders the span with a caret, so a
        // message that also spelled out line/column would print it twice.
        assert!(
            !d.message.contains("column"),
            "position must live in the span, not the message: {}",
            d.message
        );
    }

    #[test]
    fn a_control_character_is_escaped_in_the_message() {
        // Diagnostics travel over the wire and into logs; a raw control byte must not ride along.
        let d = err("a \u{7} b");
        assert_eq!(d.code, Code::UNKNOWN_CHARACTER);
        assert!(!d.message.contains('\u{7}'), "raw control char in message");
        assert!(d.message.contains("\\u{7}"), "got: {}", d.message);
    }

    #[test]
    fn a_single_ampersand_or_pipe_is_an_error() {
        assert_eq!(err("a & b").code, Code::UNKNOWN_CHARACTER);
        assert_eq!(err("a | b").code, Code::UNKNOWN_CHARACTER);
    }

    #[test]
    fn non_ascii_identifier_is_rejected() {
        let d = err("café = 1");
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::NON_ASCII_IDENTIFIER);
        // `caf` lexes, then `é` (byte 3, column 4) is rejected.
        assert_eq!(d.span, Span::new(3, 2, 1, 4));
    }

    // --- matrix: unterminated block comment ---

    #[test]
    fn unterminated_block_comment_points_at_the_opening() {
        let d = err("let x = 1;\n/* x");
        assert_eq!(d.category, Category::Lexical);
        assert_eq!(d.code, Code::UNTERMINATED_COMMENT);
        assert_eq!(d.span, Span::new(11, 2, 2, 1));
    }

    #[test]
    fn a_star_without_a_slash_does_not_close_a_comment() {
        assert_eq!(err("/* a * b").code, Code::UNTERMINATED_COMMENT);
        assert_eq!(kinds("/* a * b */ c"), vec![ident("c")]);
    }

    // --- broad coverage ---

    #[test]
    fn a_representative_program_lexes_with_trivia_removed() {
        let source = "\
plugin { name = \"p\", }
@Event(OrderPlaced, priority = 1)
fn on_order(params) { // a line comment
  let total = 0.5 + params.items?.[0].price * -2;
  /* block */
  if (total >= 1e2 && !params.free) { return { ok: true, tag: 'x' }; }
  for (k in params) { continue; }
  return null;
}";
        let tokens = toks(source);
        assert!(
            tokens
                .iter()
                .all(|t| source.get(t.span.range()).is_some() && t.span.len > 0),
            "every span must be a non-empty slice of the source"
        );
        assert!(
            !tokens
                .iter()
                .any(|t| matches!(&t.kind, TokenKind::Ident(n) if n == "a")),
            "comment contents must not reach the token stream"
        );
        assert_eq!(tokens[0].kind, TokenKind::Plugin);
        assert_eq!(tokens.last().map(|t| &t.kind), Some(&TokenKind::RBrace));
    }

    #[test]
    fn stops_at_the_first_error_without_recovering() {
        // Two malformed constructs; only the first is reported.
        let d = err("a $ b # c");
        assert_eq!(d.code, Code::UNKNOWN_CHARACTER);
        assert_eq!(d.span, Span::new(2, 1, 1, 3));
        assert!(d.message.contains('$'), "the `$` is reported, not the `#`");
        assert!(!d.message.contains('#'), "the second error is not reported");
    }

    #[test]
    fn never_panics_on_awkward_input() {
        for source in [
            "",
            "\u{0}",
            "\\",
            "/*",
            "//",
            "\"",
            "'",
            "?",
            "1e",
            "1.",
            "\u{1F600}",
            "@",
            ".5",
            "\r\n\r\n",
            "\u{feff}a",
            "0.0.0",
            "'''",
        ] {
            let _ = tokenize(source);
        }
    }
}
