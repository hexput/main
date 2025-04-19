use logos::{Logos, Lexer, Span};
use crate::ast_structs::SourceLocation;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+", error = TokenError)]
pub enum Token {
    // Keywords
    #[token("vl")]
    Vl,
    
    #[token("if")]
    If,
    
    #[token("else")]
    Else,
    
    #[token("cb")]
    Cb,
    
    #[token("res")]
    Res,
    
    #[token("loop")]
    Loop,
    
    #[token("in")]
    In,
    
    #[token("end")]
    End,
    
    #[token("continue")]
    Continue,
    
    #[token("keysof")]
    KeysOf,
    
    // Boolean and null literals
    #[token("true")]
    True,
    
    #[token("false")]
    False,
    
    #[token("null")]
    Null,
    
    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_owned(), priority = 2)]
    Identifier(String),

    // Comments
    #[regex(r"//[^\n]*", logos::skip)]
    Comment,
    
    // Literals
    #[regex(r#""([^"\\]|\\.)*""#, string_literal)]
    StringLiteral(String),
    
    #[regex(r"-?[0-9]+(\.[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    NumberLiteral(f64),
    
    // Operators
    #[token("!")]
    Bang,
    
    #[token("=")]
    Equal,
    
    #[token("==")]
    EqualEqual,
    
    #[token("+")]
    Plus,
    
    #[token("*")]
    Multiply,
    
    #[token("/", priority = 1)]
    Divide,
    
    // Logical operators
    #[token("&&")]
    And,
    
    #[token("||")]
    Or,
    
    // Comparators
    #[token(">=")]
    GreaterEqual,
    
    #[token("<=")]
    LessEqual,
    
    #[token(">")]
    Greater,
    
    #[token("<")]
    Less,
    
    // Delimiters
    #[token("{")]
    OpenBrace,
    
    #[token("}")]
    CloseBrace,
    
    #[token("(")]
    OpenParen,
    
    #[token(")")]
    CloseParen,
    
    #[token(",")]
    Comma,
    
    #[token(";")]
    Semicolon,

    // Object and array tokens
    #[token("[")]
    OpenBracket,
    
    #[token("]")]
    CloseBracket,
    
    #[token(":")]
    Colon,

    #[token(".")]
    Dot,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TokenError;

fn string_literal(lex: &mut Lexer<Token>) -> Option<String> {
    let slice = lex.slice();
    
    let content = &slice[1..slice.len() - 1];
    
    let mut processed = String::new();
    let mut chars = content.chars();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => processed.push('\n'),
                    't' => processed.push('\t'),
                    'r' => processed.push('\r'),
                    '\\' => processed.push('\\'),
                    '"' => processed.push('"'),
                    _ => {
                        // Invalid escape sequence
                        processed.push('\\');
                        processed.push(next);
                    }
                }
            }
        } else {
            processed.push(c);
        }
    }
    
    Some(processed)
}

pub struct TokenWithSpan {
    pub token: Token,
    pub span: Span,
}

impl TokenWithSpan {
    pub fn get_location(&self, source_code: &str) -> SourceLocation {
        SourceLocation::from_spans(source_code, self.span.start, self.span.end)
    }
}

pub fn tokenize(source: &str) -> Vec<TokenWithSpan> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    
    while let Some(token) = lexer.next() {
        if let Ok(token) = token {
            tokens.push(TokenWithSpan {
                token: token.clone(),
                span: lexer.span(),
            });
        }
    }
    
    tokens
}
