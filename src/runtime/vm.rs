//! Virtual machine / interpreter
//!
//! This module executes parsed and resolved ASTs.

use super::context::Context;
use super::error::{RuntimeError, RuntimeResult};
use super::value::Value;
use crate::language::ast::*;
use std::collections::HashMap;

pub fn execute(ast: &Ast, context: &mut Context) -> RuntimeResult<Value> {
    let mut vm = Vm::new(context);
    vm.execute_ast(ast)
}

struct Vm<'a> {
    context: &'a mut Context,
}

impl<'a> Vm<'a> {
    fn new(context: &'a mut Context) -> Self {
        Self { context }
    }

    fn execute_ast(&mut self, ast: &Ast) -> RuntimeResult<Value> {
        for statement in &ast.statements {
            self.execute_statement(statement)?;

            if self.context.has_return() {
                return Ok(self.context.take_return().unwrap());
            }

            if self.context.should_break() {
                self.context.clear_control_flags();
                return Err(RuntimeError::General(
                    "'end' used outside of a loop".to_string(),
                ));
            }

            if self.context.should_continue() {
                self.context.clear_control_flags();
                return Err(RuntimeError::General(
                    "'continue' used outside of a loop".to_string(),
                ));
            }
        }
        Ok(Value::Undefined)
    }

    fn execute_statement(&mut self, statement: &Statement) -> RuntimeResult<()> {
        self.context.tick()?;

        match statement {
            Statement::VarDecl { name, value, .. } => {
                let val = self.evaluate_expression(value)?;
                self.context.define(name.clone(), val);
            }
            Statement::CallbackDecl {
                name, params, body, ..
            } => {
                let callback = Value::Callback {
                    params: params.clone(),
                    body: body.clone(),
                };
                self.context.define(name.clone(), callback);
            }
            Statement::Assignment { target, value, .. } => {
                let val = self.evaluate_expression(value)?;
                self.assign_target(target, val)?;
            }
            Statement::Loop {
                var,
                iterable,
                body,
                ..
            } => {
                let iter_val = self.evaluate_expression(iterable)?;
                self.execute_loop(var, iter_val, body)?;
            }
            Statement::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let cond_val = self.evaluate_expression(condition)?;
                if cond_val.is_truthy() {
                    self.context.enter_scope();
                    for stmt in then_body {
                        self.execute_statement(stmt)?;

                        if self.context.has_return()
                            || self.context.should_break()
                            || self.context.should_continue()
                        {
                            self.context.exit_scope();
                            return Ok(());
                        }
                    }
                    self.context.exit_scope();
                } else if let Some(else_stmts) = else_body {
                    self.context.enter_scope();
                    for stmt in else_stmts {
                        self.execute_statement(stmt)?;

                        if self.context.has_return()
                            || self.context.should_break()
                            || self.context.should_continue()
                        {
                            self.context.exit_scope();
                            return Ok(());
                        }
                    }
                    self.context.exit_scope();
                }
            }
            Statement::Return { value, .. } => {
                let val = self.evaluate_expression(value)?;
                self.context.set_return(val);
            }
            Statement::Continue { .. } => {
                self.context.set_continue();
            }
            Statement::End { .. } => {
                self.context.set_break();
            }
            Statement::Block { statements, .. } => {
                self.context.enter_scope();
                for stmt in statements {
                    self.execute_statement(stmt)?;

                    if self.context.has_return()
                        || self.context.should_break()
                        || self.context.should_continue()
                    {
                        self.context.exit_scope();
                        return Ok(());
                    }
                }
                self.context.exit_scope();
            }
            Statement::Expression(expr) => {
                self.evaluate_expression(expr)?;
            }
        }

        Ok(())
    }

    fn execute_loop(
        &mut self,
        var: &str,
        iterable: Value,
        body: &[Statement],
    ) -> RuntimeResult<()> {
        let items = match iterable {
            Value::Array(arr) => arr,
            Value::Object(obj) => obj.keys().map(|k| Value::String(k.clone())).collect(),
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "array or object".to_string(),
                    got: iterable.type_name().to_string(),
                })
            }
        };

        self.context.enter_scope();

        for item in items {
            self.context.define(var.to_string(), item);
            self.context.clear_control_flags();

            for stmt in body {
                self.execute_statement(stmt)?;

                if self.context.has_return() {
                    self.context.exit_scope();
                    return Ok(());
                }

                if self.context.should_break() {
                    self.context.clear_control_flags();
                    self.context.exit_scope();
                    return Ok(());
                }

                if self.context.should_continue() {
                    self.context.clear_control_flags();
                    break;
                }
            }
        }

        self.context.exit_scope();
        Ok(())
    }

    fn evaluate_expression(&mut self, expression: &Expression) -> RuntimeResult<Value> {
        self.context.tick()?;

        match expression {
            Expression::Number(n) => Ok(Value::Number(*n)),
            Expression::String(s) => Ok(Value::String(s.clone())),
            Expression::Boolean(b) => Ok(Value::Boolean(*b)),
            Expression::Identifier(name) => self
                .context
                .get(name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expression::Object(properties) => {
                let mut obj = HashMap::new();
                for (key, value_expr) in properties {
                    let value = self.evaluate_expression(value_expr)?;
                    obj.insert(key.clone(), value);
                }
                Ok(Value::Object(obj))
            }
            Expression::Array(elements) => {
                let mut arr = Vec::new();
                for element_expr in elements {
                    let value = self.evaluate_expression(element_expr)?;
                    arr.push(value);
                }
                Ok(Value::Array(arr))
            }
            Expression::Call { callee, args } => self.execute_call(callee, args),
            Expression::Binary { op, left, right } => {
                let left_val = self.evaluate_expression(left)?;
                let right_val = self.evaluate_expression(right)?;
                self.evaluate_binary_op(*op, left_val, right_val)
            }
            Expression::Property { object, property } => {
                let obj_val = self.evaluate_expression(object)?;
                self.access_property(obj_val, property)
            }
            Expression::Index { object, index } => {
                let obj_val = self.evaluate_expression(object)?;
                let index_val = self.evaluate_expression(index)?;
                self.access_index(obj_val, index_val)
            }
            Expression::KeysOf(expr) => {
                let val = self.evaluate_expression(expr)?;
                match val {
                    Value::Object(obj) => {
                        let keys: Vec<Value> =
                            obj.keys().map(|k| Value::String(k.clone())).collect();
                        Ok(Value::Array(keys))
                    }
                    _ => Err(RuntimeError::TypeError {
                        expected: "object".to_string(),
                        got: val.type_name().to_string(),
                    }),
                }
            }
            Expression::Undefined => Ok(Value::Undefined),
            Expression::Unary { op, operand } => {
                let val = self.evaluate_expression(operand)?;
                self.evaluate_unary_op(*op, val)
            }
            Expression::Logical { op, left, right } => {
                // Short-circuit evaluation
                let left_val = self.evaluate_expression(left)?;
                match op {
                    crate::language::ast::LogicalOp::And => {
                        if !left_val.is_truthy() {
                            Ok(left_val)
                        } else {
                            self.evaluate_expression(right)
                        }
                    }
                    crate::language::ast::LogicalOp::Or => {
                        if left_val.is_truthy() {
                            Ok(left_val)
                        } else {
                            self.evaluate_expression(right)
                        }
                    }
                }
            }
            Expression::TypeOf(expr) => {
                let val = self.evaluate_expression(expr)?;
                Ok(Value::String(val.type_name().to_string()))
            }
            Expression::Grouped(expr) => self.evaluate_expression(expr),
        }
    }

    fn execute_call(&mut self, callee: &str, args: &[Expression]) -> RuntimeResult<Value> {
        // Evaluate arguments
        let mut arg_values = Vec::new();
        for arg in args {
            arg_values.push(self.evaluate_expression(arg)?);
        }

        // Try to resolve locally
        if let Some(callback_val) = self.context.get(callee) {
            match callback_val {
                Value::Callback { params, body } => {
                    if params.len() != arg_values.len() {
                        return Err(RuntimeError::General(format!(
                            "Function {} expects {} arguments, got {}",
                            callee,
                            params.len(),
                            arg_values.len()
                        )));
                    }

                    self.context.enter_scope();

                    // Bind parameters
                    for (param, value) in params.iter().zip(arg_values) {
                        self.context.define(param.clone(), value);
                    }

                    // Execute body
                    for stmt in &body {
                        self.execute_statement(stmt)?;

                        if self.context.should_break() {
                            self.context.clear_control_flags();
                            self.context.exit_scope();
                            return Err(RuntimeError::General(
                                "'end' used outside of a loop".to_string(),
                            ));
                        }

                        if self.context.should_continue() {
                            self.context.clear_control_flags();
                            self.context.exit_scope();
                            return Err(RuntimeError::General(
                                "'continue' used outside of a loop".to_string(),
                            ));
                        }

                        if let Some(ret_val) = self.context.take_return() {
                            self.context.exit_scope();
                            return Ok(ret_val);
                        }
                    }

                    self.context.exit_scope();
                    return Ok(Value::Undefined);
                }
                _ => {
                    return Err(RuntimeError::TypeError {
                        expected: "callback".to_string(),
                        got: callback_val.type_name().to_string(),
                    });
                }
            }
        }

        // Not found locally - try remote call
        self.try_remote_call(callee, arg_values)
    }

    fn try_remote_call(&self, function: &str, args: Vec<Value>) -> RuntimeResult<Value> {
        // Check if remote calls are allowed
        if !self.context.capabilities().can_call_remote(function) {
            return Err(RuntimeError::PermissionDenied(format!(
                "Not allowed to call remote function '{}'",
                function
            )));
        }

        // Get RPC handler
        let handler = self
            .context
            .rpc_handler()
            .ok_or_else(|| RuntimeError::UndefinedFunction(function.to_string()))?;

        // Make the remote call (blocks from script's perspective)
        handler.call_remote(function, args)
    }

    fn evaluate_binary_op(&self, op: BinaryOp, left: Value, right: Value) -> RuntimeResult<Value> {
        match op {
            BinaryOp::Add => match (left, right) {
                (Value::Number(l), Value::Number(r)) => Ok(Value::Number(l + r)),
                (Value::String(l), Value::String(r)) => Ok(Value::String(format!("{}{}", l, r))),
                (l, r) => Err(RuntimeError::TypeError {
                    expected: "number or string".to_string(),
                    got: format!("{} and {}", l.type_name(), r.type_name()),
                }),
            },
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                let l = left.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: left.type_name().to_string(),
                })?;
                let r = right.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: right.type_name().to_string(),
                })?;

                let result = match op {
                    BinaryOp::Sub => l - r,
                    BinaryOp::Mul => l * r,
                    BinaryOp::Div => {
                        if r == 0.0 {
                            return Err(RuntimeError::DivisionByZero);
                        }
                        l / r
                    }
                    BinaryOp::Mod => {
                        if r == 0.0 {
                            return Err(RuntimeError::DivisionByZero);
                        }
                        l % r
                    }
                    _ => unreachable!(),
                };
                Ok(Value::Number(result))
            }
            BinaryOp::Eq => Ok(Value::Boolean(self.values_equal(&left, &right))),
            BinaryOp::NotEq => Ok(Value::Boolean(!self.values_equal(&left, &right))),
            BinaryOp::Lt => {
                let l = left.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: left.type_name().to_string(),
                })?;
                let r = right.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: right.type_name().to_string(),
                })?;
                Ok(Value::Boolean(l < r))
            }
            BinaryOp::LtEq => {
                let l = left.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: left.type_name().to_string(),
                })?;
                let r = right.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: right.type_name().to_string(),
                })?;
                Ok(Value::Boolean(l <= r))
            }
            BinaryOp::Gt => {
                let l = left.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: left.type_name().to_string(),
                })?;
                let r = right.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: right.type_name().to_string(),
                })?;
                Ok(Value::Boolean(l > r))
            }
            BinaryOp::GtEq => {
                let l = left.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: left.type_name().to_string(),
                })?;
                let r = right.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: right.type_name().to_string(),
                })?;
                Ok(Value::Boolean(l >= r))
            }
        }
    }

    fn evaluate_unary_op(
        &self,
        op: crate::language::ast::UnaryOp,
        operand: Value,
    ) -> RuntimeResult<Value> {
        use crate::language::ast::UnaryOp;
        match op {
            UnaryOp::Neg => {
                let n = operand.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: operand.type_name().to_string(),
                })?;
                Ok(Value::Number(-n))
            }
            UnaryOp::Not => Ok(Value::Boolean(!operand.is_truthy())),
            UnaryOp::Plus => {
                let n = operand.as_number().ok_or_else(|| RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: operand.type_name().to_string(),
                })?;
                Ok(Value::Number(n))
            }
        }
    }

    fn values_equal(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Number(l), Value::Number(r)) => l == r,
            (Value::String(l), Value::String(r)) => l == r,
            (Value::Boolean(l), Value::Boolean(r)) => l == r,
            (Value::Undefined, Value::Undefined) => true,
            _ => false,
        }
    }

    fn access_property(&self, object: Value, property: &str) -> RuntimeResult<Value> {
        match object {
            Value::Object(obj) => Ok(obj.get(property).cloned().unwrap_or(Value::Undefined)),
            Value::MethodObject {
                object_id: _,
                fields,
            } => {
                // Block access to secret_data from script execution
                if property == "secret_data" {
                    return Err(RuntimeError::InvalidPropertyAccess(
                        "Cannot access 'secret_data' from script".to_string(),
                    ));
                }
                Ok(fields.get(property).cloned().unwrap_or(Value::Undefined))
            }
            _ => Err(RuntimeError::InvalidPropertyAccess(format!(
                "Cannot access property '{}' on {}",
                property,
                object.type_name()
            ))),
        }
    }

    fn access_index(&self, object: Value, index: Value) -> RuntimeResult<Value> {
        match (object, index) {
            (Value::Array(arr), Value::Number(idx)) => {
                let idx = idx as usize;
                Ok(arr.get(idx).cloned().unwrap_or(Value::Undefined))
            }
            (Value::Object(obj), Value::String(key)) => {
                Ok(obj.get(&key).cloned().unwrap_or(Value::Undefined))
            }
            (obj, idx) => Err(RuntimeError::TypeError {
                expected: "array[number] or object[string]".to_string(),
                got: format!("{}[{}]", obj.type_name(), idx.type_name()),
            }),
        }
    }

    fn assign_target(&mut self, target: &AssignTarget, value: Value) -> RuntimeResult<()> {
        match target {
            AssignTarget::Identifier(name) => {
                if !self.context.set(name, value.clone()) {
                    self.context.define(name.clone(), value);
                }
            }
            AssignTarget::Index { object, index } => {
                self.assign_index(object, index, value)?;
            }
            AssignTarget::Property { object, property } => {
                self.assign_property(object, property, value)?;
            }
        }
        Ok(())
    }

    fn assign_index(
        &mut self,
        object_expr: &Expression,
        index_expr: &Expression,
        value: Value,
    ) -> RuntimeResult<()> {
        // For nested assignments like arr[0].prop or obj.arr[1], we need to handle recursively
        match object_expr {
            Expression::Identifier(name) => {
                // Simple case: arr[index] = value
                let index_val = self.evaluate_expression(index_expr)?;
                let mut obj = self
                    .context
                    .get(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;

                match (&mut obj, index_val) {
                    (Value::Array(arr), Value::Number(idx)) => {
                        let idx_usize = idx as usize;
                        if idx_usize >= arr.len() {
                            return Err(RuntimeError::IndexOutOfBounds(idx_usize));
                        }
                        arr[idx_usize] = value;
                        self.context.set(name, obj);
                    }
                    (Value::Object(obj_map), Value::String(key)) => {
                        obj_map.insert(key, value);
                        self.context.set(name, obj);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "array or object".to_string(),
                            got: obj.type_name().to_string(),
                        });
                    }
                }
            }
            Expression::Index {
                object: nested_obj,
                index: nested_idx,
            } => {
                // Nested index: arr[i][j] = value
                // We need to get arr[i], modify it, then update arr[i]
                let index_val = self.evaluate_expression(index_expr)?;

                // Evaluate the nested target to get the container
                let container = self.evaluate_expression(&Expression::Index {
                    object: nested_obj.clone(),
                    index: nested_idx.clone(),
                })?;

                // Apply the mutation
                let mut updated = container.clone();
                match (&mut updated, index_val) {
                    (Value::Array(arr), Value::Number(idx)) => {
                        let idx_usize = idx as usize;
                        if idx_usize >= arr.len() {
                            return Err(RuntimeError::IndexOutOfBounds(idx_usize));
                        }
                        arr[idx_usize] = value;
                    }
                    (Value::Object(obj_map), Value::String(key)) => {
                        obj_map.insert(key, value);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "array or object".to_string(),
                            got: updated.type_name().to_string(),
                        });
                    }
                }

                // Now recursively assign back
                self.assign_index(nested_obj, nested_idx, updated)?;
            }
            Expression::Property {
                object: nested_obj,
                property: nested_prop,
            } => {
                // Property then index: obj.arr[i] = value
                let index_val = self.evaluate_expression(index_expr)?;

                let container = self.evaluate_expression(&Expression::Property {
                    object: nested_obj.clone(),
                    property: nested_prop.clone(),
                })?;

                let mut updated = container.clone();
                match (&mut updated, index_val) {
                    (Value::Array(arr), Value::Number(idx)) => {
                        let idx_usize = idx as usize;
                        if idx_usize >= arr.len() {
                            return Err(RuntimeError::IndexOutOfBounds(idx_usize));
                        }
                        arr[idx_usize] = value;
                    }
                    (Value::Object(obj_map), Value::String(key)) => {
                        obj_map.insert(key, value);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "array or object".to_string(),
                            got: updated.type_name().to_string(),
                        });
                    }
                }

                self.assign_property(nested_obj, nested_prop, updated)?;
            }
            _ => {
                return Err(RuntimeError::General(
                    "Invalid assignment target: complex expression".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn assign_property(
        &mut self,
        object_expr: &Expression,
        property: &str,
        value: Value,
    ) -> RuntimeResult<()> {
        match object_expr {
            Expression::Identifier(name) => {
                // Simple case: obj.prop = value
                let mut obj = self
                    .context
                    .get(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;

                match &mut obj {
                    Value::Object(obj_map) => {
                        obj_map.insert(property.to_string(), value);
                        self.context.set(name, obj);
                    }
                    Value::MethodObject {
                        object_id: _,
                        fields,
                    } => {
                        // Block assignment to secret_data
                        if property == "secret_data" {
                            return Err(RuntimeError::InvalidPropertyAccess(
                                "Cannot assign to 'secret_data'".to_string(),
                            ));
                        }
                        fields.insert(property.to_string(), value);
                        self.context.set(name, obj);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "object".to_string(),
                            got: obj.type_name().to_string(),
                        });
                    }
                }
            }
            Expression::Index {
                object: nested_obj,
                index: nested_idx,
            } => {
                // Index then property: arr[i].prop = value
                let container = self.evaluate_expression(&Expression::Index {
                    object: nested_obj.clone(),
                    index: nested_idx.clone(),
                })?;

                let mut updated = container.clone();
                match &mut updated {
                    Value::Object(obj_map) => {
                        obj_map.insert(property.to_string(), value);
                    }
                    Value::MethodObject {
                        object_id: _,
                        fields,
                    } => {
                        if property == "secret_data" {
                            return Err(RuntimeError::InvalidPropertyAccess(
                                "Cannot assign to 'secret_data'".to_string(),
                            ));
                        }
                        fields.insert(property.to_string(), value);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "object".to_string(),
                            got: updated.type_name().to_string(),
                        });
                    }
                }

                self.assign_index(nested_obj, nested_idx, updated)?;
            }
            Expression::Property {
                object: nested_obj,
                property: nested_prop,
            } => {
                // Property then property: obj.outer.inner = value
                let container = self.evaluate_expression(&Expression::Property {
                    object: nested_obj.clone(),
                    property: nested_prop.clone(),
                })?;

                let mut updated = container.clone();
                match &mut updated {
                    Value::Object(obj_map) => {
                        obj_map.insert(property.to_string(), value);
                    }
                    Value::MethodObject {
                        object_id: _,
                        fields,
                    } => {
                        if property == "secret_data" {
                            return Err(RuntimeError::InvalidPropertyAccess(
                                "Cannot assign to 'secret_data'".to_string(),
                            ));
                        }
                        fields.insert(property.to_string(), value);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "object".to_string(),
                            got: updated.type_name().to_string(),
                        });
                    }
                }

                self.assign_property(nested_obj, nested_prop, updated)?;
            }
            _ => {
                return Err(RuntimeError::General(
                    "Invalid assignment target: complex expression".to_string(),
                ));
            }
        }
        Ok(())
    }
}
