//! Name resolution and semantic analysis

use super::error::{SemanticError, SemanticResult};
use super::symbols::{SymbolKind, SymbolTable};
use crate::language::ast::*;

/// Perform semantic analysis on an AST
pub fn resolve(ast: &Ast) -> SemanticResult<()> {
    let mut resolver = Resolver::new();
    resolver.resolve_ast(ast)?;
    Ok(())
}

struct Resolver {
    symbols: SymbolTable,
}

impl Resolver {
    fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
        }
    }

    fn resolve_ast(&mut self, ast: &Ast) -> SemanticResult<()> {
        for statement in &ast.statements {
            self.resolve_statement(statement)?;
        }
        Ok(())
    }

    fn resolve_statement(&mut self, statement: &Statement) -> SemanticResult<()> {
        match statement {
            Statement::VarDecl { name, value, .. } => {
                self.resolve_expression(value)?;
                self.symbols.declare(name.clone(), SymbolKind::Variable);
            }
            Statement::CallbackDecl {
                name, params, body, ..
            } => {
                self.symbols.declare(name.clone(), SymbolKind::Callback);
                self.symbols.enter_scope();

                for param in params {
                    self.symbols.declare(param.clone(), SymbolKind::Parameter);
                }

                for stmt in body {
                    self.resolve_statement(stmt)?;
                }

                self.symbols.exit_scope();
            }
            Statement::Assignment { target, value, .. } => {
                self.resolve_assign_target(target)?;
                self.resolve_expression(value)?;
            }
            Statement::Loop {
                var,
                iterable,
                body,
                ..
            } => {
                self.resolve_expression(iterable)?;
                self.symbols.enter_scope();
                self.symbols.declare(var.clone(), SymbolKind::Variable);

                for stmt in body {
                    self.resolve_statement(stmt)?;
                }

                self.symbols.exit_scope();
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                self.resolve_expression(condition)?;
                self.symbols.enter_scope();

                for stmt in then_body {
                    self.resolve_statement(stmt)?;
                }

                self.symbols.exit_scope();

                if let Some(else_stmts) = else_body {
                    self.symbols.enter_scope();
                    for stmt in else_stmts {
                        self.resolve_statement(stmt)?;
                    }
                    self.symbols.exit_scope();
                }
            }
            Statement::Return { value, .. } => {
                self.resolve_expression(value)?;
            }
            Statement::Continue { .. } | Statement::End { .. } => {
                // No resolution needed
            }
            Statement::Block { statements, .. } => {
                self.symbols.enter_scope();
                for stmt in statements {
                    self.resolve_statement(stmt)?;
                }
                self.symbols.exit_scope();
            }
            Statement::Expression(expr) => {
                self.resolve_expression(expr)?;
            }
        }
        Ok(())
    }

    fn resolve_expression(&mut self, expression: &Expression) -> SemanticResult<()> {
        match expression {
            Expression::Identifier(name) => {
                if !self.symbols.is_defined(name) {
                    // This might be a remote call, which is allowed
                    // For now, we don't error on undefined identifiers in expressions
                }
            }
            Expression::Binary { left, right, .. } => {
                self.resolve_expression(left)?;
                self.resolve_expression(right)?;
            }
            Expression::Unary { operand, .. } => {
                self.resolve_expression(operand)?;
            }
            Expression::Logical { left, right, .. } => {
                self.resolve_expression(left)?;
                self.resolve_expression(right)?;
            }
            Expression::Call { callee: _, args } => {
                // Check if function is defined locally
                // If not, it will be treated as a remote call at runtime
                for arg in args {
                    self.resolve_expression(arg)?;
                }
            }
            Expression::Property { object, .. } => {
                self.resolve_expression(object)?;
            }
            Expression::Index { object, index } => {
                self.resolve_expression(object)?;
                self.resolve_expression(index)?;
            }
            Expression::Object(properties) => {
                for value in properties.values() {
                    self.resolve_expression(value)?;
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    self.resolve_expression(element)?;
                }
            }
            Expression::KeysOf(expr) => {
                self.resolve_expression(expr)?;
            }
            Expression::TypeOf(expr) => {
                self.resolve_expression(expr)?;
            }
            Expression::Grouped(expr) => {
                self.resolve_expression(expr)?;
            }
            Expression::Number(_)
            | Expression::String(_)
            | Expression::Boolean(_)
            | Expression::Undefined => {
                // Literals need no resolution
            }
        }
        Ok(())
    }

    fn resolve_assign_target(&mut self, target: &AssignTarget) -> SemanticResult<()> {
        match target {
            AssignTarget::Identifier(name) => {
                if !self.symbols.is_defined(name) {
                    return Err(SemanticError::UndefinedVariable(name.clone()));
                }
            }
            AssignTarget::Index { object, index } => {
                self.resolve_expression(object)?;
                self.resolve_expression(index)?;
            }
            AssignTarget::Property { object, .. } => {
                self.resolve_expression(object)?;
            }
        }
        Ok(())
    }
}
