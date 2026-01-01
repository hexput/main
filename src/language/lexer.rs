//! Lexical analyzer (tokenizer)
//!
//! Converts source text into a stream of tokens.

use super::error::{SyntaxError, SyntaxResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Vl,        // vl
    Cb,        // cb
    Res,       // res
    Loop,      // loop
    In,        // in
    If,        // if
    Else,      // else
    Continue,  // continue
    End,       // end
    Keysof,    // keysof

    // Literals
    Identifier(String),
    String(String),
    Number(f64),
    Boolean(bool),

    // Operators
    Equals,      // ==
    NotEquals,   // !=
    Assign,      // =
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    Percent,     // %
    Lt,          // <
    LtEq,        // <=
    Gt,          // >
    GtEq,        // >=
    And,         // &&
    Or,          // ||
    Not,         // !
    Typeof,      // typeof
    
    // Delimiters
    LeftParen,   // (
    RightParen,  // )
    LeftBrace,   // {
    RightBrace,  // }
    LeftBracket, // [
    RightBracket,// ]
    Semicolon,   // ;
    Comma,       // ,
    Dot,         // .
    Colon,       // :
    
    // Special
    Eof,
}

#[derive(Debug)]
pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_line: usize,
    current_column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            current_line: 1,
            current_column: 1,
        }
    }

    pub fn tokenize(&mut self) -> SyntaxResult<Vec<Token>> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            
            if self.is_at_end() {
                tokens.push(Token::Eof);
                break;
            }

            let token = self.next_token()?;
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn next_token(&mut self) -> SyntaxResult<Token> {
        let ch = self.current_char();

        let token = match ch {
            '(' => Token::LeftParen,
            ')' => Token::RightParen,
            '{' => Token::LeftBrace,
            '}' => Token::RightBrace,
            '[' => Token::LeftBracket,
            ']' => Token::RightBracket,
            ';' => Token::Semicolon,
            ',' => Token::Comma,
            '.' => Token::Dot,
            ':' => Token::Colon,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '%' => Token::Percent,
            '/' => {
                self.advance();
                if self.current_char() == '/' {
                    self.skip_line_comment();
                    self.skip_whitespace();
                    return self.next_token();
                }
                return Ok(Token::Slash);
            }
            '=' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    return Ok(Token::Equals);
                }
                return Ok(Token::Assign);
            }
            '!' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    return Ok(Token::NotEquals);
                }
                return Ok(Token::Not);
            }
            '<' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    return Ok(Token::LtEq);
                }
                return Ok(Token::Lt);
            }
            '>' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    return Ok(Token::GtEq);
                }
                return Ok(Token::Gt);
            }
            '&' => {
                self.advance();
                if self.current_char() == '&' {
                    self.advance();
                    return Ok(Token::And);
                }
                return Err(SyntaxError::unexpected_char('&', self.current_line, self.current_column));
            }
            '|' => {
                self.advance();
                if self.current_char() == '|' {
                    self.advance();
                    return Ok(Token::Or);
                }
                return Err(SyntaxError::unexpected_char('|', self.current_line, self.current_column));
            }
            '"' => return self.read_string(),
            _ if ch.is_ascii_digit() => return self.read_number(),
            _ if ch.is_alphabetic() || ch == '_' => return self.read_identifier(),
            _ => return Err(SyntaxError::unexpected_char(ch, self.current_line, self.current_column)),
        };

        self.advance();
        Ok(token)
    }

    fn read_identifier(&mut self) -> SyntaxResult<Token> {
        let start = self.position;
        while !self.is_at_end() && (self.current_char().is_alphanumeric() || self.current_char() == '_') {
            self.advance();
        }

        let text: String = self.input[start..self.position].iter().collect();
        
        let token = match text.as_str() {
            "vl" => Token::Vl,
            "cb" => Token::Cb,
            "res" => Token::Res,
            "loop" => Token::Loop,
            "in" => Token::In,
            "if" => Token::If,
            "else" => Token::Else,
            "continue" => Token::Continue,
            "end" => Token::End,
            "keysof" => Token::Keysof,
            "typeof" => Token::Typeof,
            "true" => Token::Boolean(true),
            "false" => Token::Boolean(false),
            _ => Token::Identifier(text),
        };

        Ok(token)
    }

    fn read_number(&mut self) -> SyntaxResult<Token> {
        let start = self.position;
        
        while !self.is_at_end() && self.current_char().is_ascii_digit() {
            self.advance();
        }

        if !self.is_at_end() && self.current_char() == '.' {
            self.advance();
            while !self.is_at_end() && self.current_char().is_ascii_digit() {
                self.advance();
            }
        }

        let text: String = self.input[start..self.position].iter().collect();
        let number = text.parse::<f64>()
            .map_err(|_| SyntaxError::invalid_number(text, self.current_line, self.current_column))?;

        Ok(Token::Number(number))
    }

    fn read_string(&mut self) -> SyntaxResult<Token> {
        self.advance(); // Skip opening quote
        let start = self.position;

        while !self.is_at_end() && self.current_char() != '"' {
            self.advance();
        }

        if self.is_at_end() {
            return Err(SyntaxError::unterminated_string(self.current_line, self.current_column));
        }

        let text: String = self.input[start..self.position].iter().collect();
        self.advance(); // Skip closing quote

        Ok(Token::String(text))
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() && self.current_char().is_whitespace() {
            self.advance();
        }
    }

    fn skip_line_comment(&mut self) {
        while !self.is_at_end() && self.current_char() != '\n' {
            self.advance();
        }
    }

    fn current_char(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.input[self.position]
        }
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            if self.input[self.position] == '\n' {
                self.current_line += 1;
                self.current_column = 1;
            } else {
                self.current_column += 1;
            }
            self.position += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let mut lexer = Lexer::new("vl x = 42;");
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0], Token::Vl);
        assert_eq!(tokens[1], Token::Identifier("x".to_string()));
        assert_eq!(tokens[2], Token::Assign);
        assert_eq!(tokens[3], Token::Number(42.0));
        assert_eq!(tokens[4], Token::Semicolon);
        assert_eq!(tokens[5], Token::Eof);
    }
}
