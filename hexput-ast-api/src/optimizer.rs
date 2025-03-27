use crate::ast_structs::{Block, Expression, Program, Property, Statement};
use crate::parallel;
use tokio::runtime::Runtime;

const PARALLELISM_THRESHOLD: usize = 2; 

pub fn optimize_ast(program: Program, runtime: &Runtime) -> Program {
    
    let optimized_statements = if program.statements.len() > PARALLELISM_THRESHOLD {
        parallel::process_items_sync(runtime, program.statements, |stmt, rt| {
            optimize_statement(stmt, rt)
        })
            .into_iter()
            .filter_map(|s| s)
            .collect()
    } else {
        optimize_statements(program.statements, runtime)
    };
    
    
    let optimized = Program::new(optimized_statements, program.location);
  
    optimized
}

fn optimize_statements(statements: Vec<Statement>, runtime: &Runtime) -> Vec<Statement> {
    let mut optimized = Vec::new();
    
    for statement in statements {
        match optimize_statement(statement, runtime) {
            Some(stmt) => optimized.push(stmt),
            None => {} 
        }
    }
    
    optimized
}

fn optimize_statement(statement: Statement, runtime: &Runtime) -> Option<Statement> {
    match statement {
        Statement::Block { block, location } => {
            
            let optimized_block = optimize_block(block, runtime);
            
            
            if optimized_block.statements.is_empty() {
                return None;
            }
            
            
            
            if optimized_block.statements.len() == 1 {
                match &optimized_block.statements[0] {
                    Statement::Block { .. } => {},
                    _ => return Some(optimized_block.statements.into_iter().next().unwrap())
                }
            }
            
            Some(Statement::Block { block: optimized_block, location })
        },
        Statement::IfStatement { condition, body, else_body, location } => {
            
            let optimized_condition = optimize_expression(condition, runtime);
            
            
            let optimized_body = optimize_block(body, runtime);
            
            
            let optimized_else_body = else_body.map(|body| optimize_block(body, runtime));
            
            
            if optimized_body.statements.is_empty() && 
               optimized_else_body.as_ref().map_or(true, |b| b.statements.is_empty()) {
                return None;
            }
            
            Some(Statement::IfStatement { 
                condition: optimized_condition, 
                body: optimized_body,
                else_body: optimized_else_body,
                location
            })
        },
        Statement::ExpressionStatement { expression, location } => {
            let optimized_expr = optimize_expression(expression, runtime);
            
            Some(Statement::ExpressionStatement { expression: optimized_expr, location })
        },
        Statement::CallbackDeclaration { name, params, body, location } => {
            
            let optimized_body = optimize_block(body, runtime);
            
            Some(Statement::CallbackDeclaration {
                name,
                params,
                body: optimized_body,
                location,
            })
        },
        Statement::ReturnStatement { value, location } => {
            
            let optimized_value = optimize_expression(value, runtime);
            
            
            Some(Statement::ReturnStatement { value: optimized_value, location })
        },
        Statement::LoopStatement { variable, iterable, body, location } => {
            
            let optimized_iterable = optimize_expression(iterable, runtime);
            
            
            let optimized_body = optimize_block(body, runtime);
            
            
            if optimized_body.statements.is_empty() {
                return None;
            }
            
            Some(Statement::LoopStatement {
                variable,
                iterable: optimized_iterable,
                body: optimized_body,
                location,
            })
        },
        
        Statement::EndStatement { location } => Some(Statement::EndStatement { location }),
        Statement::ContinueStatement { location } => Some(Statement::ContinueStatement { location }),
        
        
        Statement::VariableDeclaration { name, value, location } => {
            let optimized_value = optimize_expression(value, runtime);
            Some(Statement::VariableDeclaration { name, value: optimized_value, location })
        }
    }
}

fn optimize_block(block: Block, runtime: &Runtime) -> Block {
    
    let statements = if block.statements.len() > PARALLELISM_THRESHOLD {
        parallel::process_items_sync(runtime, block.statements, |stmt, rt| optimize_statement(stmt, rt))
            .into_iter()
            .filter_map(|s| s)
            .collect()
    } else {
        optimize_statements(block.statements, runtime)
    };
    
    
    let mut flattened = Vec::new();
    for stmt in statements {
        match stmt {
            
            Statement::Block { block: inner_block, .. } => {
                flattened.extend(inner_block.statements);
            },
            _ => flattened.push(stmt)
        }
    }
    
    Block::new(flattened, block.location)
}


fn optimize_expression(expr: Expression, runtime: &Runtime) -> Expression {
    match expr {
        Expression::BinaryExpression { left, operator, right, location } => {
            
            let optimized_left = optimize_expression(*left, runtime);
            let optimized_right = optimize_expression(*right, runtime);
            let result = Expression::BinaryExpression {
                left: Box::new(optimized_left),
                operator,
                right: Box::new(optimized_right),
                location,
            };
            result
        },
        Expression::AssignmentExpression { target, value, location } => {
            let optimized_value = Box::new(optimize_expression(*value, runtime));
            let result = Expression::AssignmentExpression { target, value: optimized_value, location };
            result
        },
        Expression::MemberAssignmentExpression { object, property, property_expr, computed, value, location } => {
            
            let optimized_object = Box::new(optimize_expression(*object, runtime));
            
            
            let optimized_prop_expr = match property_expr {
                Some(expr) => Some(Box::new(optimize_expression(*expr, runtime))),
                None => None,
            };
            
            let optimized_value = Box::new(optimize_expression(*value, runtime));
            let result = Expression::MemberAssignmentExpression {
                object: optimized_object,
                property,
                property_expr: optimized_prop_expr,
                computed,
                value: optimized_value,
                location,
            };
            result
        },
        Expression::CallExpression { callee, arguments, location } => {
            
            let optimized_args = if arguments.len() > PARALLELISM_THRESHOLD {
                parallel::process_items_sync(runtime, arguments, |arg, rt| optimize_expression(arg, rt))
            } else {
                arguments.into_iter().map(|arg| optimize_expression(arg, runtime)).collect()
            };
            let result = Expression::CallExpression { callee, arguments: optimized_args, location };
            result
        },
        Expression::ArrayExpression { elements, location } => {
            
            let optimized_elements = if elements.len() > PARALLELISM_THRESHOLD {
                parallel::process_items_sync(runtime, elements, |elem, rt| optimize_expression(elem, rt))
            } else {
                elements.into_iter().map(|elem| optimize_expression(elem, runtime)).collect()
            };
            let result = Expression::ArrayExpression { elements: optimized_elements, location };
            result
        },
        Expression::ObjectExpression { properties, location } => {
            
            let optimized_properties = if properties.len() > PARALLELISM_THRESHOLD {
                parallel::process_items_sync(
                    runtime,
                    properties,
                    |prop, rt| Property::new(prop.key, optimize_expression(prop.value, rt), prop.location)
                )
            } else {
                properties.into_iter()
                    .map(|prop| Property::new(prop.key, optimize_expression(prop.value, runtime), prop.location))
                    .collect()
            };
            let result = Expression::ObjectExpression { properties: optimized_properties, location };
            result
        },
        Expression::MemberExpression { object, property, property_expr, computed, location } => {
            
            let optimized_object = Box::new(optimize_expression(*object, runtime));
            
            
            let optimized_prop_expr = match property_expr {
                Some(expr) => Some(Box::new(optimize_expression(*expr, runtime))),
                None => None,
            };
            let result = Expression::MemberExpression {
                object: optimized_object,
                property,
                property_expr: optimized_prop_expr,
                computed,
                location,
            };
            result
        },
        Expression::KeysOfExpression { object, location } => {
            let optimized_object = Box::new(optimize_expression(*object, runtime));
            let result = Expression::KeysOfExpression { object: optimized_object, location };
            result
        },
        Expression::MemberCallExpression { object, property, property_expr, computed, arguments, location } => {
            
            let optimized_object = Box::new(optimize_expression(*object, runtime));
            
            
            let optimized_prop_expr = match property_expr {
                Some(expr) => Some(Box::new(optimize_expression(*expr, runtime))),
                None => None,
            };
            
            
            let optimized_args = if arguments.len() > PARALLELISM_THRESHOLD {
                parallel::process_items_sync(runtime, arguments, |arg, rt| optimize_expression(arg, rt))
            } else {
                arguments.into_iter().map(|arg| optimize_expression(arg, runtime)).collect()
            };
            
            Expression::MemberCallExpression {
                object: optimized_object,
                property,
                property_expr: optimized_prop_expr,
                computed,
                arguments: optimized_args,
                location,
            }
        },
        Expression::UnaryExpression { operator, operand, location } => {
            let optimized_operand = Box::new(optimize_expression(*operand, runtime));
            Expression::UnaryExpression { 
                operator,
                operand: optimized_operand,
                location,
            }
        },
        Expression::StringLiteral { .. } |
        Expression::NumberLiteral { .. } |
        Expression::Identifier { .. } |
        Expression::CallbackReference { .. } |
        Expression::BooleanLiteral { .. } |
        Expression::NullLiteral { .. } => {
            expr
        },
    }
}
