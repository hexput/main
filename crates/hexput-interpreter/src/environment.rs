//! Lexical, block-level scopes (§5) as an `Arc`-linked chain, so Story 1.7 closures can capture
//! a scope by reference and see later mutations of its bindings.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::Value;

pub(crate) struct Scope {
    bindings: Mutex<HashMap<String, Value>>,
    parent: Option<Arc<Scope>>,
}

impl Scope {
    pub(crate) fn root() -> Arc<Self> {
        Arc::new(Self {
            bindings: Mutex::new(HashMap::new()),
            parent: None,
        })
    }

    pub(crate) fn child(parent: &Arc<Self>) -> Arc<Self> {
        Arc::new(Self {
            bindings: Mutex::new(HashMap::new()),
            parent: Some(Arc::clone(parent)),
        })
    }

    fn bindings(&self) -> MutexGuard<'_, HashMap<String, Value>> {
        self.bindings.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Bind `name` in this scope. The parser has already rejected same-block redeclaration.
    pub(crate) fn declare(&self, name: &str, value: Value) {
        self.bindings().insert(name.to_owned(), value);
    }

    /// Read the innermost binding of `name`, walking outward iteratively.
    pub(crate) fn lookup(&self, name: &str) -> Option<Value> {
        let mut scope = self;
        loop {
            if let Some(value) = scope.bindings().get(name) {
                return Some(value.clone());
            }
            scope = scope.parent.as_deref()?;
        }
    }

    /// Overwrite the innermost binding of `name`. Returns the value back when no scope declares
    /// it — there is no implicit global creation (§5).
    pub(crate) fn assign(&self, name: &str, value: Value) -> Result<(), Value> {
        let mut scope = self;
        loop {
            if let Some(slot) = scope.bindings().get_mut(name) {
                *slot = value;
                return Ok(());
            }
            match scope.parent.as_deref() {
                Some(parent) => scope = parent,
                None => return Err(value),
            }
        }
    }
}

// A long chain of otherwise-unreferenced scopes (an evaluation that failed 12,000 blocks deep)
// would drop recursively through `parent`; unlink it iteratively instead.
impl Drop for Scope {
    fn drop(&mut self) {
        let mut parent = self.parent.take();
        while let Some(scope) = parent {
            parent = Arc::into_inner(scope).and_then(|mut owned| owned.parent.take());
        }
    }
}
