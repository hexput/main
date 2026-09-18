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
