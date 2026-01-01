//! Execution context
//!
//! Each script runs in an isolated context with:
//! - Local variables
//! - Execution limits
//! - Capabilities

use std::collections::HashMap;
use std::sync::Arc;
use super::value::Value;
use super::rpc_handler::RpcHandler;
use crate::sandbox::Limits;
use crate::semantic::capabilities::CapabilitySet;

pub struct Context {
    /// Local variable bindings
    variables: Vec<HashMap<String, Value>>,
    
    /// Execution limits
    limits: Limits,
    
    /// Instruction counter
    instruction_count: usize,
    
    /// Capabilities granted to this context
    capabilities: CapabilitySet,
    
    /// Return value (if any)
    return_value: Option<Value>,
    
    /// Control flow flags
    should_break: bool,
    should_continue: bool,
    
    /// RPC handler for remote function calls
    rpc_handler: Option<Arc<dyn RpcHandler>>,
}

impl Context {
    pub fn new(limits: Limits) -> Self {
        Self {
            variables: vec![HashMap::new()],
            limits,
            instruction_count: 0,
            capabilities: CapabilitySet::new(),
            return_value: None,
            should_break: false,
            should_continue: false,
            rpc_handler: None,
        }
    }

    pub fn with_capabilities(limits: Limits, capabilities: CapabilitySet) -> Self {
        Self {
            variables: vec![HashMap::new()],
            limits,
            instruction_count: 0,
            capabilities,
            return_value: None,
            should_break: false,
            should_continue: false,
            rpc_handler: None,
        }
    }
    
    pub fn with_rpc_handler(
        limits: Limits,
        capabilities: CapabilitySet,
        rpc_handler: Arc<dyn RpcHandler>,
    ) -> Self {
        Self {
            variables: vec![HashMap::new()],
            limits,
            instruction_count: 0,
            capabilities,
            return_value: None,
            should_break: false,
            should_continue: false,
            rpc_handler: Some(rpc_handler),
        }
    }
    
    pub fn set_rpc_handler(&mut self, handler: Arc<dyn RpcHandler>) {
        self.rpc_handler = Some(handler);
    }
    
    pub fn rpc_handler(&self) -> Option<&Arc<dyn RpcHandler>> {
        self.rpc_handler.as_ref()
    }

    pub fn enter_scope(&mut self) {
        self.variables.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        if self.variables.len() > 1 {
            self.variables.pop();
        }
    }

    pub fn define(&mut self, name: String, value: Value) {
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(name, value);
        }
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.variables.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value.clone());
            }
        }
        None
    }

    pub fn set(&mut self, name: &str, value: Value) -> bool {
        for scope in self.variables.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }

    pub fn tick(&mut self) -> Result<(), crate::runtime::RuntimeError> {
        self.instruction_count += 1;
        if self.instruction_count > self.limits.max_instructions {
            return Err(crate::runtime::RuntimeError::LimitExceeded(
                format!("Maximum instruction count ({}) exceeded", self.limits.max_instructions)
            ));
        }
        Ok(())
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    pub fn set_return(&mut self, value: Value) {
        self.return_value = Some(value);
    }

    pub fn has_return(&self) -> bool {
        self.return_value.is_some()
    }

    pub fn take_return(&mut self) -> Option<Value> {
        self.return_value.take()
    }

    pub fn set_break(&mut self) {
        self.should_break = true;
    }

    pub fn set_continue(&mut self) {
        self.should_continue = true;
    }

    pub fn should_break(&self) -> bool {
        self.should_break
    }

    pub fn should_continue(&self) -> bool {
        self.should_continue
    }

    pub fn clear_control_flags(&mut self) {
        self.should_break = false;
        self.should_continue = false;
    }
}
