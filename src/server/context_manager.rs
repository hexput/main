//! Context manager for handling multiple execution contexts
//!
//! Each context has its own set of registered functions and methods.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::language::ast::Ast;
use crate::rpc::registry::Registry;
use crate::runtime::Value;

/// Callback function type
pub type CallbackFn = Arc<dyn Fn(Vec<Value>) -> Result<Value, String> + Send + Sync>;

/// Cached parsed code
#[derive(Debug, Clone)]
pub struct CachedCode {
    pub ast: Ast,
    pub source: String, // Keep for debugging
}

/// Manages multiple execution contexts
pub struct ContextManager {
    /// Maps context_id -> Registry
    contexts: HashMap<String, Arc<RwLock<Registry>>>,
    /// Maps code_id -> CachedCode
    cached_code: HashMap<String, CachedCode>,
    /// Counter for generating code IDs
    code_id_counter: u64,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            cached_code: HashMap::new(),
            code_id_counter: 0,
        }
    }

    /// Add or get a context
    pub async fn get_or_create_context(&mut self, context_id: &str) -> Arc<RwLock<Registry>> {
        if let Some(registry) = self.contexts.get(context_id) {
            Arc::clone(registry)
        } else {
            let registry = Arc::new(RwLock::new(Registry::new()));
            self.contexts
                .insert(context_id.to_string(), Arc::clone(&registry));
            registry
        }
    }

    /// Get a context registry
    pub fn get_context(&self, context_id: &str) -> Option<Arc<RwLock<Registry>>> {
        self.contexts.get(context_id).map(Arc::clone)
    }

    /// Remove a context
    pub fn remove_context(&mut self, context_id: &str) -> bool {
        self.contexts.remove(context_id).is_some()
    }

    /// List all context IDs
    pub fn list_contexts(&self) -> Vec<String> {
        self.contexts.keys().cloned().collect()
    }

    /// Get number of contexts
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// Register code and return generated code_id
    pub fn register_code(&mut self, ast: Ast, source: String) -> String {
        self.code_id_counter += 1;
        let code_id = format!("code_{}", self.code_id_counter);

        self.cached_code
            .insert(code_id.clone(), CachedCode { ast, source });

        code_id
    }

    /// Get cached code by code_id
    pub fn get_cached_code(&self, code_id: &str) -> Option<&CachedCode> {
        self.cached_code.get(code_id)
    }

    /// Remove cached code
    pub fn remove_cached_code(&mut self, code_id: &str) -> bool {
        self.cached_code.remove(code_id).is_some()
    }

    /// Get number of cached code entries
    pub fn cached_code_count(&self) -> usize {
        self.cached_code.len()
    }
}
