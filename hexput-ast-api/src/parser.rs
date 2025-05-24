use crate::ast_structs::{Block, Expression, Operator, Program, Property, Statement, SourceLocation, UnaryOperator};
use crate::feature_flags::FeatureFlags;
use crate::lexer::{Token, TokenWithSpan};
use std::fmt;
use std::iter::Peekable;
use std::slice::Iter;

// Define a macro to extract location from an expression
macro_rules! get_expr_location {
    ($expr:expr) => {
        match &$expr {
            Expression::StringLiteral { location, .. } |
            Expression::NumberLiteral { location, .. } |
            Expression::Identifier { location, .. } |
            Expression::BinaryExpression { location, .. } |
            Expression::AssignmentExpression { location, .. } |
            Expression::MemberAssignmentExpression { location, .. } |
            Expression::CallExpression { location, .. } |
            Expression::MemberCallExpression { location, .. } |  
            Expression::InlineCallbackExpression { location, .. } |
            Expression::ArrayExpression { location, .. } |
            Expression::ObjectExpression { location, .. } |
            Expression::MemberExpression { location, .. } |
            Expression::KeysOfExpression { location, .. } |
            Expression::BooleanLiteral { location, .. } |
            Expression::UnaryExpression { location, .. } |
            Expression::NullLiteral { location, .. } => location.clone(),
        }
    };
}

// Define a macro to extract start line from an expression
macro_rules! get_expr_start_line {
    ($expr:expr) => {
        match &$expr {
            Expression::StringLiteral { location, .. } |
            Expression::NumberLiteral { location, .. } |
            Expression::Identifier { location, .. } |
            Expression::BinaryExpression { location, .. } |
            Expression::AssignmentExpression { location, .. } |
            Expression::MemberAssignmentExpression { location, .. } |
            Expression::CallExpression { location, .. } |
            Expression::MemberCallExpression { location, .. } |  
            Expression::InlineCallbackExpression { location, .. } |
            Expression::ArrayExpression { location, .. } |
            Expression::ObjectExpression { location, .. } |
            Expression::MemberExpression { location, .. } |
            Expression::KeysOfExpression { location, .. } |
            Expression::BooleanLiteral { location, .. } |
            Expression::UnaryExpression { location, .. } |
            Expression::NullLiteral { location, .. } => location.start_line,
        }
    };
}

// Define a macro to extract start column from an expression
macro_rules! get_expr_start_column {
    ($expr:expr) => {
        match &$expr {
            Expression::StringLiteral { location, .. } |
            Expression::NumberLiteral { location, .. } |
            Expression::Identifier { location, .. } |
            Expression::BinaryExpression { location, .. } |
            Expression::AssignmentExpression { location, .. } |
            Expression::MemberAssignmentExpression { location, .. } |
            Expression::CallExpression { location, .. } |
            Expression::MemberCallExpression { location, .. } |  
            Expression::InlineCallbackExpression { location, .. } |
            Expression::ArrayExpression { location, .. } |
            Expression::ObjectExpression { location, .. } |
            Expression::MemberExpression { location, .. } |
            Expression::KeysOfExpression { location, .. } |
            Expression::BooleanLiteral { location, .. } |
            Expression::UnaryExpression { location, .. } |
            Expression::NullLiteral { location, .. } => location.start_column,
        }
    };
}

pub struct Parser<'a> {
    tokens: Peekable<Iter<'a, TokenWithSpan>>,
    current_token: Option<&'a TokenWithSpan>,
    flags: FeatureFlags,
    source_code: &'a str,
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(String, SourceLocation),
    ExpectedToken(String, SourceLocation),
    EndOfInput(SourceLocation),
    FeatureDisabled(String, SourceLocation),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken(msg, loc) => 
                write!(f, "Unexpected token: {} at line {}, column {}", 
                    msg, loc.start_line, loc.start_column),
            ParseError::ExpectedToken(msg, loc) => 
                write!(f, "Expected token: {} at line {}, column {}", 
                    msg, loc.start_line, loc.start_column),
            ParseError::EndOfInput(loc) => 
                write!(f, "Unexpected end of input at line {}, column {}", 
                    loc.start_line, loc.start_column),
            ParseError::FeatureDisabled(feature, loc) => 
                write!(f, "Feature disabled: {} is not allowed with current settings at line {}, column {}", 
                    feature, loc.start_line, loc.start_column),
        }
    }
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [TokenWithSpan], flags: FeatureFlags, source_code: &'a str) -> Self {
        let mut parser = Parser {
            tokens: tokens.iter().peekable(),
            current_token: None,
            flags,
            source_code,
        };
        parser.advance();
        parser
    }

    fn current_location(&self) -> SourceLocation {
        match self.current_token {
            Some(token) => token.get_location(self.source_code),
            None => {
                let end_pos = self.source_code.len();
                SourceLocation::from_spans(self.source_code, end_pos, end_pos)
            }
        }
    }

    fn advance(&mut self) {
        self.current_token = self.tokens.next();
    }

    fn peek(&mut self) -> Option<&&TokenWithSpan> {
        self.tokens.peek()
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let location = self.current_location();
        match self.current_token {
            Some(token_with_span) if token_with_span.token == expected => {
                self.advance();
                Ok(())
            }
            Some(token_with_span) => Err(ParseError::UnexpectedToken(format!(
                "Expected {:?}, got {:?}",
                expected, token_with_span.token
            ), location)),
            None => Err(ParseError::EndOfInput(location)),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let start_location = self.current_location();
        
        let mut statements = Vec::new();

        while self.current_token.is_some() {
            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }

        let end_location = if let Some(last_token) = self.tokens.clone().last() {
            last_token.get_location(self.source_code)
        } else {
            let end_pos = self.source_code.len();
            SourceLocation::from_spans(self.source_code, end_pos, end_pos)
        };
        
        let program_location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Program::new(statements, program_location))
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let start_location = self.current_location();
        
        let stmt = match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Vl => {
                    if self.flags.allow_variable_declaration {
                        self.parse_variable_declaration(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Variable declarations".to_string(), start_location))
                    }
                },
                Token::If => {
                    if self.flags.allow_conditionals {
                        self.parse_if_statement(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Conditional statements".to_string(), start_location))
                    }
                },
                Token::Loop => {
                    if self.flags.allow_loops {
                        self.parse_loop_statement(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Loop statements".to_string(), start_location))
                    }
                },
                Token::OpenBrace => self.parse_block_statement(start_location),
                Token::Cb => {
                    if self.flags.allow_callbacks {
                        self.parse_callback_declaration(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Callback declarations".to_string(), start_location))
                    }
                },
                Token::Res => {
                    if self.flags.allow_return_statements {
                        self.parse_return_statement(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Return statements".to_string(), start_location))
                    }
                },
                Token::End => {
                    if self.flags.allow_loop_control {
                        self.parse_end_statement(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Loop control statements (end)".to_string(), start_location))
                    }
                },
                Token::Continue => {
                    if self.flags.allow_loop_control {
                        self.parse_continue_statement(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Loop control statements (continue)".to_string(), start_location))
                    }
                },
                _ => {
                    let expr = self.parse_expression()?;
                    let end_location = self.current_location();
                    self.expect(Token::Semicolon)?;
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        end_location.end_line,
                        end_location.end_column
                    );
                    
                    Ok(Statement::ExpressionStatement { 
                        expression: expr,
                        location
                    })
                }
            },
            None => Err(ParseError::EndOfInput(start_location)),
        }?;
        
        Ok(stmt)
    }
    
    fn parse_variable_declaration(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();

        let name = match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Identifier(name) => name.clone(),
                _ => return Err(ParseError::ExpectedToken("identifier".to_string(), self.current_location())),
            },
            None => return Err(ParseError::EndOfInput(self.current_location())),
        };
        self.advance();

        self.expect(Token::Equal)?;

        let value = self.parse_expression()?;
        
        let semicolon_location = self.current_location();
        self.expect(Token::Semicolon)?;

        let end_location = semicolon_location;
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::VariableDeclaration { name, value, location })
    }

    fn parse_if_statement(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        
        let else_body = if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::Else {
                self.advance();
                Some(self.parse_block()?)
            } else {
                None
            }
        } else {
            None
        };

        let end_location = else_body.as_ref().map_or_else(
            || body.location.clone(),
            |else_block| else_block.location.clone()
        );
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::IfStatement {
            condition, 
            body,
            else_body,
            location,
        })
    }

    fn parse_block_statement(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        let block = self.parse_block()?;
        
        let end_location = block.location.clone();
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::Block { block, location })
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start_location = self.current_location();
        
        self.expect(Token::OpenBrace)?;

        let mut statements = Vec::new();
        while let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::CloseBrace {
                break;
            }
            statements.push(self.parse_statement()?);
        }

        let end_location = self.current_location();
        self.expect(Token::CloseBrace)?;

        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Block::new(statements, location))
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        let expr = self.parse_assignment()?;
        Ok(expr)
    }

    fn parse_assignment(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let expr = self.parse_logical_or()?;
        
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::Equal {
                if !self.flags.allow_assignments {
                    return Err(ParseError::FeatureDisabled("Assignments".to_string(), start_location));
                }
                
                self.advance();
                let value = self.parse_logical_or()?;
                let end_location = get_expr_location!(value);
                
                let location = SourceLocation::new(
                    start_location.start_line,
                    start_location.start_column,
                    end_location.end_line,
                    end_location.end_column
                );
                
                let new_expr = match expr {
                    Expression::Identifier { name, .. } => Expression::AssignmentExpression {
                        target: name,
                        value: Box::new(value),
                        location,
                    },
                    Expression::MemberExpression { object, property, property_expr, computed, .. } => {
                        if !self.flags.allow_object_navigation {
                            return Err(ParseError::FeatureDisabled("Object property assignment".to_string(), start_location));
                        }
                        
                        Expression::MemberAssignmentExpression {
                            object,
                            property,
                            property_expr,
                            computed,
                            value: Box::new(value),
                            location,
                        }
                    },
                    _ => return Err(ParseError::UnexpectedToken("Invalid assignment target".to_string(), start_location)),
                };
                return Ok(new_expr);
            }
        }
        Ok(expr)
    }

    fn parse_logical_or(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let mut expr = self.parse_logical_and()?;
        
        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::Or => {
                    self.advance();
                    let right = self.parse_logical_and()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Or,
                        right: Box::new(right),
                        location,
                    };
                }
                _ => break,
            }
        }
        
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let mut expr = self.parse_equality()?;
        
        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::And => {
                    self.advance();
                    let right = self.parse_equality()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::And,
                        right: Box::new(right),
                        location,
                    };
                }
                _ => break,
            }
        }
        
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let mut expr = self.parse_additive()?;
        
        while let Some(token_with_span) = self.current_token {
            expr = match token_with_span.token {
                Token::Greater => {
                    self.advance();
                    let right = self.parse_additive()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Greater,
                        right: Box::new(right),
                        location,
                    }
                },
                Token::Less => {
                    self.advance();
                    let right = self.parse_additive()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Less,
                        right: Box::new(right),
                        location,
                    }
                },
                Token::GreaterEqual => {
                    self.advance();
                    let right = self.parse_additive()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::GreaterEqual,
                        right: Box::new(right),
                        location,
                    }
                },
                Token::LessEqual => {
                    self.advance();
                    let right = self.parse_additive()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::LessEqual,
                        right: Box::new(right),
                        location,
                    }
                },
                _ => break,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let mut expr = self.parse_comparison()?;
        
        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::EqualEqual => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Equal,
                        right: Box::new(right),
                        location,
                    };
                },
                Token::NotEqual => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::NotEqual,
                        right: Box::new(right),
                        location,
                    };
                }
                _ => break,
            }
        }
        
        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let mut expr = self.parse_multiplicative()?;

        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::Plus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Plus,
                        right: Box::new(right),
                        location,
                    };
                },
                Token::Minus => {
                    self.advance();
                    let right = self.parse_multiplicative()?;
                    let right_loc = get_expr_location!(right);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Minus,
                        right: Box::new(right),
                        location,
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }
    
    fn parse_multiplicative(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        let mut expr = self.parse_unary()?;
        
        expr = self.parse_member_access(expr)?;

        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::Multiply => {
                    self.advance();
                    let right = self.parse_primary()?;
                    let right_with_member = self.parse_member_access(right)?;
                    let right_loc = get_expr_location!(right_with_member);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Multiply,
                        right: Box::new(right_with_member),
                        location,
                    };
                }
                Token::Divide => {
                    self.advance();
                    let right = self.parse_primary()?;
                    let right_with_member = self.parse_member_access(right)?;
                    let right_loc = get_expr_location!(right_with_member);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        right_loc.end_line,
                        right_loc.end_column
                    );
                    
                    expr = Expression::BinaryExpression {
                        left: Box::new(expr),
                        operator: Operator::Divide,
                        right: Box::new(right_with_member),
                        location,
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        
        match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Bang => {
                    self.advance();
                    let operand = self.parse_unary()?;
                    let operand_loc = get_expr_location!(operand);
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        operand_loc.end_line,
                        operand_loc.end_column
                    );
                    
                    Ok(Expression::UnaryExpression {
                        operator: UnaryOperator::Not,
                        operand: Box::new(operand),
                        location,
                    })
                },
                _ => self.parse_primary()
            },
            None => Err(ParseError::EndOfInput(start_location)),
        }
    }

    fn parse_primary(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        
        match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::KeysOf => {
                    if self.flags.allow_object_keys {
                        self.advance();
                        
                        // First parse the primary expression
                        let mut object_expr = self.parse_primary()?;
                        
                        // Then handle any member access operations after it
                        object_expr = self.parse_member_access(object_expr)?;
                        
                        let object_loc = get_expr_location!(object_expr);
                        
                        let location = SourceLocation::new(
                            start_location.start_line,
                            start_location.start_column,
                            object_loc.end_line,
                            object_loc.end_column
                        );
                        
                        Ok(Expression::KeysOfExpression {
                            object: Box::new(object_expr),
                            location,
                        })
                    } else {
                        Err(ParseError::FeatureDisabled("Object keys operator (keysof)".to_string(), start_location))
                    }
                },
                Token::OpenBracket => {
                    if self.flags.allow_array_constructions {
                        self.parse_array_literal(start_location)
                    } else {
                        Err(ParseError::FeatureDisabled("Array literals".to_string(), start_location))
                    }
                },
                Token::OpenBrace => {
                    match self.peek() {
                        Some(next_token) => match &next_token.token {
                            Token::Identifier(_) => {
                                if self.flags.allow_object_constructions {
                                    self.parse_object_literal(start_location)
                                } else {
                                    Err(ParseError::FeatureDisabled("Object literals".to_string(), start_location))
                                }
                            },
                            Token::CloseBrace => {
                                self.advance();
                                self.advance();
                                if self.flags.allow_object_constructions {
                                    Ok(Expression::ObjectExpression { properties: vec![], location: start_location })
                                } else {
                                    Err(ParseError::FeatureDisabled("Object literals".to_string(), start_location))
                                }
                            },
                            _ => {
                                if self.flags.allow_object_constructions {
                                    self.parse_object_literal(start_location)
                                } else {
                                    Err(ParseError::FeatureDisabled("Object literals".to_string(), start_location))
                                }
                            }
                        },
                        None => Err(ParseError::EndOfInput(start_location)),
                    }
                },
                Token::OpenParen => {
                    self.advance();
                    let expr = self.parse_expression()?;
                    self.expect(Token::CloseParen)?;
                    Ok(expr)
                },
                Token::Identifier(name) => {
                    let id_name = name.clone();
                    self.advance();
                    
                    if let Some(next_token) = self.current_token {
                        if next_token.token == Token::OpenParen {
                            return self.parse_function_call(id_name, start_location);
                        }
                    }
                    
                    Ok(Expression::Identifier { 
                        name: id_name,
                        location: start_location,
                    })
                },
                Token::StringLiteral(value) => {
                    let str_value = value.clone();
                    self.advance();
                    Ok(Expression::StringLiteral { 
                        value: str_value,
                        location: start_location,
                    })
                },
                Token::NumberLiteral(value) => {
                    let num_value = *value;
                    self.advance();
                    Ok(Expression::NumberLiteral { 
                        value: num_value,
                        location: start_location,
                    })
                },
                Token::True => {
                    self.advance();
                    Ok(Expression::BooleanLiteral { 
                        value: true,
                        location: start_location,
                    })
                },
                
                Token::False => {
                    self.advance();
                    Ok(Expression::BooleanLiteral { 
                        value: false,
                        location: start_location,
                    })
                },
                
                Token::Null => {
                    self.advance();
                    Ok(Expression::NullLiteral { 
                        location: start_location,
                    })
                },
                _ => Err(ParseError::UnexpectedToken(format!(
                    "Unexpected token: {:?}",
                    token_with_span.token
                ), start_location)),
            },
            None => Err(ParseError::EndOfInput(start_location)),
        }
    }
    
    fn parse_function_call(&mut self, callee: String, start_location: SourceLocation) -> Result<Expression, ParseError> {
        self.expect(Token::OpenParen)?;
        
        let mut arguments = Vec::new();
        
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::CloseParen {
                let end_location = self.current_location();
                self.advance();
                
                let location = SourceLocation::new(
                    start_location.start_line,
                    start_location.start_column,
                    end_location.end_line,
                    end_location.end_column
                );
                
                return Ok(Expression::CallExpression { callee, arguments, location });
            }
        }
        
        // Check for inline callback syntax: cb name(params) { ... }
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::Cb {
                if !self.flags.allow_callbacks {
                    return Err(ParseError::FeatureDisabled("Inline callback expressions".to_string(), self.current_location()));
                }
                
                let inline_callback = self.parse_inline_callback()?;
                arguments.push(inline_callback);
                
                // Check for more arguments after callback
                while let Some(token_with_span) = self.current_token {
                    match &token_with_span.token {
                        Token::Comma => {
                            self.advance();
                            arguments.push(self.parse_expression()?);
                        }
                        Token::CloseParen => {
                            let end_location = self.current_location();
                            self.advance();
                            
                            let location = SourceLocation::new(
                                start_location.start_line,
                                start_location.start_column,
                                end_location.end_line,
                                end_location.end_column
                            );
                            
                            return Ok(Expression::CallExpression { callee, arguments, location });
                        }
                        _ => return Err(ParseError::ExpectedToken("',' or ')'".to_string(), self.current_location())),
                    }
                }
            } else {
                arguments.push(self.parse_expression()?);
            }
        } else {
            arguments.push(self.parse_expression()?);
        }
        
        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::Comma => {
                    self.advance();
                    
                    // Check for inline callback syntax after comma
                    if let Some(token_with_span) = self.current_token {
                        if token_with_span.token == Token::Cb {
                            if !self.flags.allow_callbacks {
                                return Err(ParseError::FeatureDisabled("Inline callback expressions".to_string(), self.current_location()));
                            }
                            let inline_callback = self.parse_inline_callback()?;
                            arguments.push(inline_callback);
                        } else {
                            arguments.push(self.parse_expression()?);
                        }
                    } else {
                        arguments.push(self.parse_expression()?);
                    }
                }
                Token::CloseParen => {
                    let end_location = self.current_location();
                    self.advance();
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        end_location.end_line,
                        end_location.end_column
                    );
                    
                    return Ok(Expression::CallExpression { callee, arguments, location });
                }
                _ => return Err(ParseError::ExpectedToken("',' or ')'".to_string(), self.current_location())),
            }
        }
        
        Err(ParseError::ExpectedToken("')'".to_string(), self.current_location()))
    }

    fn parse_inline_callback(&mut self) -> Result<Expression, ParseError> {
        let start_location = self.current_location();
        self.advance(); // consume 'cb'

        let name = match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Identifier(name) => name.clone(),
                _ => return Err(ParseError::ExpectedToken("callback name".to_string(), self.current_location())),
            },
            None => return Err(ParseError::EndOfInput(self.current_location())),
        };
        self.advance();

        self.expect(Token::OpenParen)?;
        let mut params = Vec::new();
        
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::CloseParen {
                self.advance();
            } else {
                if let Some(token_with_span) = self.current_token {
                    match &token_with_span.token {
                        Token::Identifier(param) => {
                            params.push(param.clone());
                            self.advance();
                        },
                        _ => return Err(ParseError::ExpectedToken("parameter name".to_string(), self.current_location())),
                    }
                }

                while let Some(token_with_span) = self.current_token {
                    match &token_with_span.token {
                        Token::Comma => {
                            self.advance();
                            match &self.current_token {
                                Some(token_with_span) => match &token_with_span.token {
                                    Token::Identifier(param) => {
                                        params.push(param.clone());
                                        self.advance();
                                    },
                                    _ => return Err(ParseError::ExpectedToken("parameter name".to_string(), self.current_location())),
                                },
                                None => return Err(ParseError::EndOfInput(self.current_location())),
                            }
                        },
                        Token::CloseParen => {
                            self.advance();
                            break;
                        },
                        _ => return Err(ParseError::ExpectedToken("',' or ')'".to_string(), self.current_location())),
                    }
                }
            }
        } else {
            return Err(ParseError::EndOfInput(self.current_location()));
        }

        let body = self.parse_block()?;

        let end_location = body.location.clone();
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Expression::InlineCallbackExpression {
            name,
            params,
            body,
            location,
        })
    }

    fn parse_callback_declaration(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();

        let name = match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Identifier(name) => name.clone(),
                _ => return Err(ParseError::ExpectedToken("callback name".to_string(), self.current_location())),
            },
            None => return Err(ParseError::EndOfInput(self.current_location())),
        };
        self.advance();

        self.expect(Token::OpenParen)?;
        let mut params = Vec::new();
        
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::CloseParen {
                self.advance();
            } else {
                if let Some(token_with_span) = self.current_token {
                    match &token_with_span.token {
                        Token::Identifier(param) => {
                            params.push(param.clone());
                            self.advance();
                        },
                        _ => return Err(ParseError::ExpectedToken("parameter name".to_string(), self.current_location())),
                    }
                }

                while let Some(token_with_span) = self.current_token {
                    match &token_with_span.token {
                        Token::Comma => {
                            self.advance();
                            match &self.current_token {
                                Some(token_with_span) => match &token_with_span.token {
                                    Token::Identifier(param) => {
                                        params.push(param.clone());
                                        self.advance();
                                    },
                                    _ => return Err(ParseError::ExpectedToken("parameter name".to_string(), self.current_location())),
                                },
                                None => return Err(ParseError::EndOfInput(self.current_location())),
                            }
                        },
                        Token::CloseParen => {
                            self.advance();
                            break;
                        },
                        _ => return Err(ParseError::ExpectedToken("',' or ')'".to_string(), self.current_location())),
                    }
                }
            }
        } else {
            return Err(ParseError::EndOfInput(self.current_location()));
        }

        let body = self.parse_block()?;

        let end_location = body.location.clone();
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::CallbackDeclaration {
            name,
            params,
            body,
            location,
        })
    }

    fn parse_return_statement(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();

        let value = self.parse_expression()?;
        
        let semicolon_location = self.current_location();
        self.expect(Token::Semicolon)?;

        let end_location = semicolon_location;
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::ReturnStatement { value, location })
    }

    fn parse_array_literal(&mut self, start_location: SourceLocation) -> Result<Expression, ParseError> {
        self.advance();
        
        let mut elements = Vec::new();
        
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::CloseBracket {
                let end_location = self.current_location();
                self.advance();
                
                let location = SourceLocation::new(
                    start_location.start_line,
                    start_location.start_column,
                    end_location.end_line,
                    end_location.end_column
                );
                
                return Ok(Expression::ArrayExpression { elements, location });
            }
        }
        
        elements.push(self.parse_expression()?);
        
        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::Comma => {
                    self.advance();
                    elements.push(self.parse_expression()?);
                }
                Token::CloseBracket => {
                    let end_location = self.current_location();
                    self.advance();
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        end_location.end_line,
                        end_location.end_column
                    );
                    
                    return Ok(Expression::ArrayExpression { elements, location });
                }
                _ => return Err(ParseError::ExpectedToken("',' or ']'".to_string(), self.current_location())),
            }
        }
        
        Err(ParseError::ExpectedToken("']'".to_string(), self.current_location()))
    }
    
    fn parse_object_literal(&mut self, start_location: SourceLocation) -> Result<Expression, ParseError> {
        self.advance();
        
        let mut properties = Vec::new();
        
        if let Some(token_with_span) = self.current_token {
            if token_with_span.token == Token::CloseBrace {
                let end_location = self.current_location();
                self.advance();
                
                let location = SourceLocation::new(
                    start_location.start_line,
                    start_location.start_column,
                    end_location.end_line,
                    end_location.end_column
                );
                
                return Ok(Expression::ObjectExpression { properties, location });
            }
        }
        
        let property = self.parse_object_property()?;
        properties.push(property);
        
        while let Some(token_with_span) = self.current_token {
            match &token_with_span.token {
                Token::Comma => {
                    self.advance();
                    let property = self.parse_object_property()?;
                    properties.push(property);
                }
                Token::CloseBrace => {
                    let end_location = self.current_location();
                    self.advance();
                    
                    let location = SourceLocation::new(
                        start_location.start_line,
                        start_location.start_column,
                        end_location.end_line,
                        end_location.end_column
                    );
                    
                    return Ok(Expression::ObjectExpression { properties, location });
                }
                _ => return Err(ParseError::ExpectedToken("',' or '}'".to_string(), self.current_location())),
            }
        }
        
        Err(ParseError::ExpectedToken("'}'".to_string(), self.current_location()))
    }
    
    fn parse_object_property(&mut self) -> Result<Property, ParseError> {
        let start_location = self.current_location();
        
        let key = match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Identifier(name) => name.clone(),
                Token::StringLiteral(value) => value.clone(),
                _ => return Err(ParseError::ExpectedToken("property key".to_string(), self.current_location())),
            },
            None => return Err(ParseError::EndOfInput(self.current_location())),
        };
        self.advance();
        
        self.expect(Token::Colon)?;
        
        let value = self.parse_expression()?;
        
        let end_location = get_expr_location!(value);
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );
        
        Ok(Property::new(key, value, location))
    }

    fn parse_member_access(&mut self, mut object: Expression) -> Result<Expression, ParseError> {
        loop {
            match &self.current_token {
                Some(token_with_span) => match &token_with_span.token {
                    Token::Dot => {
                        if !self.flags.allow_object_navigation {
                            return Err(ParseError::FeatureDisabled("Object navigation (dot notation)".to_string(), self.current_location()));
                        }
                        
                        self.advance();
                        
                        match &self.current_token {
                            Some(token_with_span) => match &token_with_span.token {
                                Token::Identifier(prop_name) => {
                                    let property = prop_name.clone();
                                    let property_location = self.current_location();
                                    self.advance();
                                    
                                    
                                    let obj_start_line = get_expr_start_line!(object);
                                    
                                    let obj_start_column = get_expr_start_column!(object);
                                    
                                    
                                    if let Some(token_with_span) = self.current_token {
                                        if token_with_span.token == Token::OpenParen {
                                            self.advance(); 
                                            
                                            
                                            let mut arguments = Vec::new();
                                            
                                            
                                            if let Some(token_with_span) = self.current_token {
                                                if token_with_span.token == Token::CloseParen {
                                                    
                                                    let end_call_location = self.current_location();
                                                    self.advance(); 
                                                    
                                                    
                                                    let call_location = SourceLocation::new(
                                                        obj_start_line,
                                                        obj_start_column,
                                                        end_call_location.end_line,
                                                        end_call_location.end_column
                                                    );
                                                    
                                                    
                                                    object = Expression::MemberCallExpression {
                                                        object: Box::new(object),
                                                        property: Some(property),
                                                        property_expr: None,
                                                        computed: false,
                                                        arguments,
                                                        location: call_location,
                                                    };
                                                    continue;
                                                }
                                            }
                                            
                                            
                                            arguments.push(self.parse_expression()?);
                                            
                                            
                                            while let Some(token_with_span) = self.current_token {
                                                match &token_with_span.token {
                                                    Token::Comma => {
                                                        self.advance(); 
                                                        arguments.push(self.parse_expression()?);
                                                    }
                                                    Token::CloseParen => {
                                                        let end_call_location = self.current_location();
                                                        self.advance(); 
                                                        
                                                        
                                                        let call_location = SourceLocation::new(
                                                            obj_start_line,
                                                            obj_start_column,
                                                            end_call_location.end_line,
                                                            end_call_location.end_column
                                                        );
                                                        
                                                        
                                                        object = Expression::MemberCallExpression {
                                                            object: Box::new(object),
                                                            property: Some(property),
                                                            property_expr: None,
                                                            computed: false,
                                                            arguments,
                                                            location: call_location,
                                                        };
                                                        break;
                                                    }
                                                    _ => return Err(ParseError::ExpectedToken("',' or ')'".to_string(), self.current_location())),
                                                }
                                            }
                                        } else {
                                            
                                            let member_expr_location = SourceLocation::new(
                                                obj_start_line,
                                                obj_start_column,
                                                property_location.end_line,
                                                property_location.end_column
                                            );
                                            
                                            object = Expression::MemberExpression {
                                                object: Box::new(object),
                                                property: Some(property),
                                                property_expr: None,
                                                computed: false,
                                                location: member_expr_location,
                                            };
                                        }
                                    } else {
                                        
                                        let member_expr_location = SourceLocation::new(
                                            obj_start_line,
                                            obj_start_column,
                                            property_location.end_line,
                                            property_location.end_column
                                        );
                                        
                                        object = Expression::MemberExpression {
                                            object: Box::new(object),
                                            property: Some(property),
                                            property_expr: None,
                                            computed: false,
                                            location: member_expr_location,
                                        };
                                    }
                                }
                                _ => return Err(ParseError::ExpectedToken("property name".to_string(), self.current_location())),
                            },
                            None => return Err(ParseError::EndOfInput(self.current_location())),
                        }
                    },
                    
                    Token::OpenBracket => {
                        if !self.flags.allow_object_navigation {
                            return Err(ParseError::FeatureDisabled("Object navigation (bracket notation)".to_string(), self.current_location()));
                        }
                        
                        self.advance();
                        
                        let property_expr = self.parse_expression()?;
                        
                        let close_bracket_location = self.current_location();
                        self.expect(Token::CloseBracket)?;
                        
                        let obj_start_line = get_expr_start_line!(object);
                        
                        let obj_start_column = get_expr_start_column!(object);
                        
                        let member_expr_location = SourceLocation::new(
                            obj_start_line,
                            obj_start_column,
                            close_bracket_location.end_line,
                            close_bracket_location.end_column
                        );
                        
                        if let Some(token_with_span) = self.current_token {
                            if token_with_span.token == Token::OpenParen {
                                
                                self.advance();
                                
                                let mut arguments = Vec::new();
                                
                                
                                if let Some(token_with_span) = self.current_token {
                                    if token_with_span.token == Token::CloseParen {
                                        let end_call_location = self.current_location();
                                        self.advance();
                                        
                                        let call_location = SourceLocation::new(
                                            obj_start_line,
                                            obj_start_column,
                                            end_call_location.end_line,
                                            end_call_location.end_column
                                        );
                                        
                                        
                                        object = Expression::MemberCallExpression {
                                            object: Box::new(object),
                                            property: None,
                                            property_expr: Some(Box::new(property_expr)),
                                            computed: true,
                                            arguments,
                                            location: call_location,
                                        };
                                        continue;
                                    }
                                }
                                
                                
                                arguments.push(self.parse_expression()?);
                                
                                while let Some(token_with_span) = self.current_token {
                                    match &token_with_span.token {
                                        Token::Comma => {
                                            self.advance();
                                            arguments.push(self.parse_expression()?);
                                        }
                                        Token::CloseParen => {
                                            let end_call_location = self.current_location();
                                            self.advance();
                                            
                                            let call_location = SourceLocation::new(
                                                obj_start_line,
                                                obj_start_column,
                                                end_call_location.end_line,
                                                end_call_location.end_column
                                            );
                                            
                                            
                                            object = Expression::MemberCallExpression {
                                                object: Box::new(object),
                                                property: None,
                                                property_expr: Some(Box::new(property_expr)),
                                                computed: true,
                                                arguments,
                                                location: call_location,
                                            };
                                            break;
                                        }
                                        _ => return Err(ParseError::ExpectedToken("',' or ')'".to_string(), self.current_location())),
                                    }
                                }
                            } else {
                                
                                object = Expression::MemberExpression {
                                    object: Box::new(object),
                                    property: None,
                                    property_expr: Some(Box::new(property_expr)),
                                    computed: true,
                                    location: member_expr_location,
                                };
                            }
                        } else {
                            
                            object = Expression::MemberExpression {
                                object: Box::new(object),
                                property: None,
                                property_expr: Some(Box::new(property_expr)),
                                computed: true,
                                location: member_expr_location,
                            };
                        }
                    },
                    _ => break,
                },
                None => break,
            }
        }
        Ok(object)
    }

    fn parse_loop_statement(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();
        
        let variable = match &self.current_token {
            Some(token_with_span) => match &token_with_span.token {
                Token::Identifier(name) => name.clone(),
                _ => return Err(ParseError::ExpectedToken("identifier".to_string(), self.current_location())),
            },
            None => return Err(ParseError::EndOfInput(self.current_location())),
        };
        self.advance();
        
        match &self.current_token {
            Some(token_with_span) => {
                if token_with_span.token != Token::In {
                    return Err(ParseError::ExpectedToken("'in'".to_string(), self.current_location()));
                }
                self.advance();
            },
            None => return Err(ParseError::EndOfInput(self.current_location())),
        }
        
        let iterable = self.parse_expression()?;
        
        let body = self.parse_block()?;
        
        let end_location = body.location.clone();
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::LoopStatement {
            variable,
            iterable,
            body,
            location,
        })
    }

    fn parse_end_statement(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();
        
        let semicolon_location = self.current_location();
        self.expect(Token::Semicolon)?;

        let end_location = semicolon_location;
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::EndStatement { location })
    }
    
    fn parse_continue_statement(&mut self, start_location: SourceLocation) -> Result<Statement, ParseError> {
        self.advance();
        
        let semicolon_location = self.current_location();
        self.expect(Token::Semicolon)?;

        let end_location = semicolon_location;
        
        let location = SourceLocation::new(
            start_location.start_line,
            start_location.start_column,
            end_location.end_line,
            end_location.end_column
        );

        Ok(Statement::ContinueStatement { location })
    }
}
