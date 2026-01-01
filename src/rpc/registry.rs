//! Function registry for RPC

use std::collections::HashMap;
use std::sync::Arc;

/// A function that can be called remotely
pub type RemoteFunction = Arc<dyn Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync>;

type MethodKey = (String, String);

/// Registry of available remote functions
#[derive(Clone)]
pub struct FunctionRegistry {
    functions: HashMap<String, RemoteFunction>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    pub fn register<F>(&mut self, name: impl Into<String>, func: F)
    where
        F: Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    {
        self.functions.insert(name.into(), Arc::new(func));
    }

    pub fn get(&self, name: &str) -> Option<&RemoteFunction> {
        self.functions.get(name)
    }

    pub fn call(&self, name: &str, args: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        match self.get(name) {
            Some(func) => func(args),
            None => Err(format!("Function '{}' not found", name)),
        }
    }

    pub fn list(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Host-facing registry that supports per-context function and method handlers.
///
/// - `addFunction(name, handler)` maps to `register_function(context_id, name, handler)`
/// - `addMethod(object_id, method_name, handler)` maps to `register_method(context_id, object_id, method_name, handler)`
///
/// The method handler receives the call args; the object identity is implicit from `object_id`.
#[derive(Clone, Default)]
pub struct Registry {
    default_functions: FunctionRegistry,
    functions_by_context: HashMap<String, FunctionRegistry>,
    methods_by_context: HashMap<String, HashMap<MethodKey, RemoteFunction>>,
    // Track which functions/methods are allowed to be called (for security)
    allowed_functions: HashMap<String, Vec<String>>, // context_id -> [function_names]
    allowed_methods: HashMap<String, Vec<MethodKey>>, // context_id -> [(object_id, method_name)]
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_default_functions(default: FunctionRegistry) -> Self {
        Self {
            default_functions: default,
            functions_by_context: HashMap::new(),
            methods_by_context: HashMap::new(),
            allowed_functions: HashMap::new(),
            allowed_methods: HashMap::new(),
        }
    }

    fn normalize_context_id<'a>(&'a self, context_id: Option<&'a str>) -> Option<&'a str> {
        context_id.filter(|s| !s.is_empty())
    }

    pub fn register_function<F>(&mut self, context_id: Option<&str>, name: impl Into<String>, func: F)
    where
        F: Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    {
        match self.normalize_context_id(context_id) {
            None => self.default_functions.register(name, func),
            Some(ctx) => self
                .functions_by_context
                .entry(ctx.to_string())
                .or_insert_with(FunctionRegistry::new)
                .register(name, func),
        }
    }

    pub fn register_method<F>(
        &mut self,
        context_id: Option<&str>,
        object_id: impl Into<String>,
        method_name: impl Into<String>,
        func: F,
    ) where
        F: Fn(Vec<serde_json::Value>) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    {
        let ctx = self.normalize_context_id(context_id).unwrap_or("default").to_string();
        let key: MethodKey = (object_id.into(), method_name.into());
        self.methods_by_context
            .entry(ctx)
            .or_insert_with(HashMap::new)
            .insert(key, Arc::new(func));
    }

    pub fn call_function(
        &self,
        context_id: Option<&str>,
        name: &str,
        args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        // Check if function is allowed
        let ctx = self.normalize_context_id(context_id).unwrap_or("default");
        if let Some(allowed) = self.allowed_functions.get(ctx) {
            if !allowed.contains(&name.to_string()) {
                return Err(format!("Function '{}' not registered in context '{}'", name, ctx));
            }
        } else {
            return Err(format!("No functions registered in context '{}'", ctx));
        }
        
        match self.normalize_context_id(context_id) {
            None => self.default_functions.call(name, args),
            Some(ctx) => self
                .functions_by_context
                .get(ctx)
                .unwrap_or(&self.default_functions)
                .call(name, args),
        }
    }

    pub fn call_method(
        &self,
        context_id: Option<&str>,
        object_id: &str,
        method_name: &str,
        args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let ctx = self.normalize_context_id(context_id).unwrap_or("default");
        
        // Check if method is allowed
        if let Some(allowed) = self.allowed_methods.get(ctx) {
            let key = (object_id.to_string(), method_name.to_string());
            if !allowed.contains(&key) {
                return Err(format!("Method '{}.{}' not registered in context '{}'", object_id, method_name, ctx));
            }
        } else {
            return Err(format!("No methods registered in context '{}'", ctx));
        }
        
        match self.methods_by_context.get(ctx) {
            Some(methods) => match methods.get(&(object_id.to_string(), method_name.to_string())) {
                Some(func) => func(args),
                None => Err(format!("Method '{}.{}' not found in context '{}'", object_id, method_name, ctx)),
            },
            None => Err(format!("No methods registered for context '{}'", ctx)),
        }
    }
    
    /// Mark a function as allowed to be called in a context
    pub fn allow_function(&mut self, context_id: &str, function_name: impl Into<String>) {
        self.allowed_functions
            .entry(context_id.to_string())
            .or_insert_with(Vec::new)
            .push(function_name.into());
    }
    
    /// Mark a method as allowed to be called in a context
    pub fn allow_method(&mut self, context_id: &str, object_id: impl Into<String>, method_name: impl Into<String>) {
        let key = (object_id.into(), method_name.into());
        self.allowed_methods
            .entry(context_id.to_string())
            .or_insert_with(Vec::new)
            .push(key);
    }
    
    /// Check if a function is allowed in a context
    pub fn is_function_allowed(&self, context_id: &str, function_name: &str) -> bool {
        self.allowed_functions
            .get(context_id)
            .map(|list| list.contains(&function_name.to_string()))
            .unwrap_or(false)
    }
    
    /// Check if a method is allowed in a context
    pub fn is_method_allowed(&self, context_id: &str, object_id: &str, method_name: &str) -> bool {
        self.allowed_methods
            .get(context_id)
            .map(|list| list.contains(&(object_id.to_string(), method_name.to_string())))
            .unwrap_or(false)
    }
    
    /// Get all allowed function names for a context
    pub fn get_allowed_functions(&self, context_id: &str) -> Vec<String> {
        self.allowed_functions
            .get(context_id)
            .cloned()
            .unwrap_or_default()
    }
}
