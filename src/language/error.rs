//! Syntax error types

use thiserror::Error;

pub type SyntaxResult<T> = Result<T, SyntaxError>;

#[derive(Debug, Error, Clone)]
pub enum SyntaxError {
    #[error("Unexpected character '{0}' at line {1}, column {2}")]
    UnexpectedChar(char, usize, usize),

    #[error("Invalid number '{0}' at line {1}, column {2}")]
    InvalidNumber(String, usize, usize),

    #[error("Unterminated string at line {0}, column {1}")]
    UnterminatedString(usize, usize),

    #[error("Unexpected token: expected {expected}, got {got} at line {line}")]
    UnexpectedToken {
        expected: String,
        got: String,
        line: usize,
    },

    #[error("Unexpected end of input")]
    UnexpectedEof,

    #[error("Parse error: {0}")]
    ParseError(String),
}

impl SyntaxError {
    pub fn unexpected_char(ch: char, line: usize, column: usize) -> Self {
        Self::UnexpectedChar(ch, line, column)
    }

    pub fn invalid_number(text: String, line: usize, column: usize) -> Self {
        Self::InvalidNumber(text, line, column)
    }

    pub fn unterminated_string(line: usize, column: usize) -> Self {
        Self::UnterminatedString(line, column)
    }

    pub fn unexpected_token(
        expected: impl Into<String>,
        got: impl Into<String>,
        line: usize,
    ) -> Self {
        Self::UnexpectedToken {
            expected: expected.into(),
            got: got.into(),
            line,
        }
    }

    pub fn unexpected_eof() -> Self {
        Self::UnexpectedEof
    }

    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::ParseError(msg.into())
    }
}
