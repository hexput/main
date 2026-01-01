//! Parser: Converts tokens into an AST
//!
//! This is a recursive descent parser for the hexput language.

use super::ast::*;
use super::error::{SyntaxError, SyntaxResult};
use super::lexer::{Lexer, Token};
use std::collections::HashMap;

pub fn parse(source: &str) -> SyntaxResult<Ast> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    
    let mut parser = Parser::new(tokens);
    parser.parse()
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn parse(&mut self) -> SyntaxResult<Ast> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            statements.push(self.statement()?);
        }

        Ok(Ast { statements })
    }

    fn statement(&mut self) -> SyntaxResult<Statement> {
        match &self.peek() {
            Token::Vl => self.var_decl(),
            Token::Cb => self.callback_decl(),
            Token::Loop => self.loop_stmt(),
            Token::If => self.if_stmt(),
            Token::Res => self.return_stmt(),
            Token::Continue => {
                self.advance();
                self.consume_semicolon()?;
                Ok(Statement::Continue { span: None })
            }
            Token::End => {
                self.advance();
                self.consume_semicolon()?;
                Ok(Statement::End { span: None })
            }
            Token::Identifier(_) => {
                // Could be assignment or expression statement
                self.assignment_or_expr()
            }
            _ => {
                let expr = self.expression()?;
                self.consume_semicolon()?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn var_decl(&mut self) -> SyntaxResult<Statement> {
        self.consume(Token::Vl)?;
        
        let name = self.consume_identifier()?;
        self.consume(Token::Assign)?;
        let value = self.expression()?;
        self.consume_semicolon()?;

        Ok(Statement::VarDecl { name, value, span: None })
    }

    fn callback_decl(&mut self) -> SyntaxResult<Statement> {
        self.consume(Token::Cb)?;
        
        let name = self.consume_identifier()?;
        self.consume(Token::LeftParen)?;
        
        let mut params = Vec::new();
        if !matches!(self.peek(), Token::RightParen) {
            loop {
                params.push(self.consume_identifier()?);
                if !matches!(self.peek(), Token::Comma) {
                    break;
                }
                self.advance();
            }
        }
        
        self.consume(Token::RightParen)?;
        self.consume(Token::LeftBrace)?;
        
        let mut body = Vec::new();
        while !matches!(self.peek(), Token::RightBrace) {
            body.push(self.statement()?);
        }
        
        self.consume(Token::RightBrace)?;

        Ok(Statement::CallbackDecl { name, params, body, span: None })
    }

    fn assignment_or_expr(&mut self) -> SyntaxResult<Statement> {
        // Parse the left side
        let expr = self.expression()?;
        
        // Check if this is an assignment
        if matches!(self.peek(), Token::Assign) {
            self.advance();
            let value = self.expression()?;
            self.consume_semicolon()?;
            
            // Convert expression to assignment target
            let target = match expr {
                Expression::Identifier(name) => AssignTarget::Identifier(name),
                Expression::Index { object, index } => AssignTarget::Index { object, index },
                Expression::Property { object, property } => AssignTarget::Property { object, property },
                _ => return Err(SyntaxError::parse_error("Invalid assignment target")),
            };
            
            Ok(Statement::Assignment { target, value, span: None })
        } else {
            self.consume_semicolon()?;
            Ok(Statement::Expression(expr))
        }
    }

    fn loop_stmt(&mut self) -> SyntaxResult<Statement> {
        self.consume(Token::Loop)?;
        
        let var = self.consume_identifier()?;
        self.consume(Token::In)?;
        let iterable = self.expression()?;
        self.consume(Token::LeftBrace)?;
        
        let mut body = Vec::new();
        while !matches!(self.peek(), Token::RightBrace) {
            body.push(self.statement()?);
        }
        
        self.consume(Token::RightBrace)?;

        Ok(Statement::Loop { var, iterable, body, span: None })
    }

    fn if_stmt(&mut self) -> SyntaxResult<Statement> {
        self.consume(Token::If)?;
        
        let condition = self.expression()?;
        self.consume(Token::LeftBrace)?;
        
        let mut then_body = Vec::new();
        while !matches!(self.peek(), Token::RightBrace) {
            then_body.push(self.statement()?);
        }
        
        self.consume(Token::RightBrace)?;
        
        let else_body = if matches!(self.peek(), Token::Else) {
            self.advance(); // consume 'else'
            self.consume(Token::LeftBrace)?;
            
            let mut else_stmts = Vec::new();
            while !matches!(self.peek(), Token::RightBrace) {
                else_stmts.push(self.statement()?);
            }
            
            self.consume(Token::RightBrace)?;
            Some(else_stmts)
        } else {
            None
        };

        Ok(Statement::If { condition, then_body, else_body, span: None })
    }

    fn return_stmt(&mut self) -> SyntaxResult<Statement> {
        self.consume(Token::Res)?;
        let value = self.expression()?;
        self.consume_semicolon()?;

        Ok(Statement::Return { value, span: None })
    }

    fn expression(&mut self) -> SyntaxResult<Expression> {
        self.logical_or()
    }

    fn logical_or(&mut self) -> SyntaxResult<Expression> {
        let mut left = self.logical_and()?;

        while matches!(self.peek(), Token::Or) {
            self.advance();
            let right = self.logical_and()?;
            left = Expression::Logical {
                op: crate::language::ast::LogicalOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn logical_and(&mut self) -> SyntaxResult<Expression> {
        let mut left = self.equality()?;

        while matches!(self.peek(), Token::And) {
            self.advance();
            let right = self.equality()?;
            left = Expression::Logical {
                op: crate::language::ast::LogicalOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn equality(&mut self) -> SyntaxResult<Expression> {
        let mut left = self.comparison()?;

        while matches!(self.peek(), Token::Equals | Token::NotEquals) {
            let op = match self.advance() {
                Token::Equals => BinaryOp::Eq,
                Token::NotEquals => BinaryOp::NotEq,
                _ => unreachable!(),
            };
            let right = self.comparison()?;
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn comparison(&mut self) -> SyntaxResult<Expression> {
        let mut left = self.additive()?;

        while matches!(self.peek(), Token::Lt | Token::LtEq | Token::Gt | Token::GtEq) {
            let op = match self.advance() {
                Token::Lt => BinaryOp::Lt,
                Token::LtEq => BinaryOp::LtEq,
                Token::Gt => BinaryOp::Gt,
                Token::GtEq => BinaryOp::GtEq,
                _ => unreachable!(),
            };
            let right = self.additive()?;
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn additive(&mut self) -> SyntaxResult<Expression> {
        let mut left = self.multiplicative()?;

        while matches!(self.peek(), Token::Plus | Token::Minus) {
            let op = match self.advance() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => unreachable!(),
            };
            let right = self.multiplicative()?;
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn multiplicative(&mut self) -> SyntaxResult<Expression> {
        let mut left = self.unary()?;

        while matches!(self.peek(), Token::Star | Token::Slash | Token::Percent) {
            let op = match self.advance() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                Token::Percent => BinaryOp::Mod,
                _ => unreachable!(),
            };
            let right = self.unary()?;
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn unary(&mut self) -> SyntaxResult<Expression> {
        if matches!(self.peek(), Token::Minus | Token::Not | Token::Plus) {
            let op = match self.advance() {
                Token::Minus => crate::language::ast::UnaryOp::Neg,
                Token::Not => crate::language::ast::UnaryOp::Not,
                Token::Plus => crate::language::ast::UnaryOp::Plus,
                _ => unreachable!(),
            };
            let operand = self.unary()?; // Allow chaining like --x
            Ok(Expression::Unary {
                op,
                operand: Box::new(operand),
            })
        } else {
            self.postfix()
        }
    }

    fn postfix(&mut self) -> SyntaxResult<Expression> {
        let mut expr = self.primary()?;

        loop {
            match self.peek() {
                Token::LeftParen => {
                    // Function call
                    self.advance();
                    let mut args = Vec::new();
                    
                    if !matches!(self.peek(), Token::RightParen) {
                        loop {
                            args.push(self.expression()?);
                            if !matches!(self.peek(), Token::Comma) {
                                break;
                            }
                            self.advance();
                        }
                    }
                    
                    self.consume(Token::RightParen)?;
                    
                    // Extract callee name
                    let callee = match expr {
                        Expression::Identifier(name) => name,
                        _ => return Err(SyntaxError::parse_error("Invalid function call")),
                    };
                    
                    expr = Expression::Call { callee, args };
                }
                Token::Dot => {
                    // Property access
                    self.advance();
                    let property = self.consume_identifier()?;
                    expr = Expression::Property {
                        object: Box::new(expr),
                        property,
                    };
                }
                Token::LeftBracket => {
                    // Index access
                    self.advance();
                    let index = self.expression()?;
                    self.consume(Token::RightBracket)?;
                    expr = Expression::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> SyntaxResult<Expression> {
        match self.peek() {
            Token::Typeof => {
                self.advance();
                let expr = self.primary()?; // typeof has high precedence
                Ok(Expression::TypeOf(Box::new(expr)))
            }
            Token::Number(n) => {
                let n = *n;
                self.advance();
                Ok(Expression::Number(n))
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression::String(s))
            }
            Token::Boolean(b) => {
                let b = *b;
                self.advance();
                Ok(Expression::Boolean(b))
            }
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Expression::Identifier(name))
            }
            Token::LeftBrace => self.object_literal(),
            Token::LeftBracket => self.array_literal(),
            Token::LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.consume(Token::RightParen)?;
                Ok(Expression::Grouped(Box::new(expr)))
            }
            Token::Keysof => {
                self.advance();
                let expr = self.expression()?;
                Ok(Expression::KeysOf(Box::new(expr)))
            }
            _ => Err(SyntaxError::parse_error(format!("Unexpected token: {:?}", self.peek()))),
        }
    }

    fn object_literal(&mut self) -> SyntaxResult<Expression> {
        self.consume(Token::LeftBrace)?;
        
        let mut properties = HashMap::new();
        
        while !matches!(self.peek(), Token::RightBrace) {
            let key = self.consume_identifier()?;
            self.consume(Token::Colon)?;
            let value = self.expression()?;
            
            properties.insert(key, value);
            
            if !matches!(self.peek(), Token::Comma) {
                break;
            }
            self.advance();
        }
        
        self.consume(Token::RightBrace)?;
        Ok(Expression::Object(properties))
    }

    fn array_literal(&mut self) -> SyntaxResult<Expression> {
        self.consume(Token::LeftBracket)?;
        
        let mut elements = Vec::new();
        
        while !matches!(self.peek(), Token::RightBracket) {
            elements.push(self.expression()?);
            
            if !matches!(self.peek(), Token::Comma) {
                break;
            }
            self.advance();
        }
        
        self.consume(Token::RightBracket)?;
        Ok(Expression::Array(elements))
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.tokens[self.current - 1].clone()
    }

    fn consume(&mut self, expected: Token) -> SyntaxResult<()> {
        if std::mem::discriminant(self.peek()) == std::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(SyntaxError::unexpected_token(
                format!("{:?}", expected),
                format!("{:?}", self.peek()),
                0,
            ))
        }
    }

    fn consume_identifier(&mut self) -> SyntaxResult<String> {
        match self.peek() {
            Token::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err(SyntaxError::unexpected_token("identifier", format!("{:?}", self.peek()), 0)),
        }
    }

    fn consume_semicolon(&mut self) -> SyntaxResult<()> {
        // Semicolons are optional in some contexts, so we make this lenient
        if matches!(self.peek(), Token::Semicolon) {
            self.advance();
        }
        Ok(())
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_var_decl() {
        let ast = parse("vl x = 42;").unwrap();
        assert_eq!(ast.statements.len(), 1);
        
        match &ast.statements[0] {
            Statement::VarDecl { name, value, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(value, Expression::Number(42.0)));
            }
            _ => panic!("Expected VarDecl"),
        }
    }

    #[test]
    fn test_parse_callback() {
        let ast = parse("cb add(a, b) { res a + b; }").unwrap();
        assert_eq!(ast.statements.len(), 1);
        
        match &ast.statements[0] {
            Statement::CallbackDecl { name, params, body, .. } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected CallbackDecl"),
        }
    }
}
