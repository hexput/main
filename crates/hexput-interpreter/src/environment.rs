//! Lexical, block-level scopes (§5) as heap-resident records linked by parent handle, so Story
//! 1.7 closures can capture a scope by handle and see later mutations of its bindings — and a
//! closure that captures its own scope forms a cycle the per-execution heap frees for free.

use std::collections::HashMap;

use crate::heap::{Heap, RtValue, Slot, SlotId};

pub(crate) struct ScopeRecord {
    bindings: HashMap<String, RtValue>,
    parent: Option<SlotId>,
}

impl Heap {
    /// Allocate an empty scope nested in `parent` (or a root scope).
    pub(crate) fn push_scope(&mut self, parent: Option<SlotId>) -> SlotId {
        self.alloc(Slot::Scope(ScopeRecord {
            bindings: HashMap::new(),
            parent,
        }))
    }

    fn scope(&self, id: SlotId) -> Option<&ScopeRecord> {
        match self.slot(id) {
            Some(Slot::Scope(record)) => Some(record),
            _ => None,
        }
    }

    fn scope_mut(&mut self, id: SlotId) -> Option<&mut ScopeRecord> {
        match self.slot_mut(id) {
            Some(Slot::Scope(record)) => Some(record),
            _ => None,
        }
    }

    /// Bind `name` in `scope`. The parser has already rejected same-block redeclaration.
    pub(crate) fn declare(&mut self, scope: SlotId, name: &str, value: RtValue) {
        if let Some(record) = self.scope_mut(scope) {
            record.bindings.insert(name.to_owned(), value);
        }
    }

    /// The innermost scope, starting at `scope` and walking outward iteratively, that binds
    /// `name`.
    fn resolve(&self, mut scope: SlotId, name: &str) -> Option<SlotId> {
        loop {
            let record = self.scope(scope)?;
            if record.bindings.contains_key(name) {
                return Some(scope);
            }
            scope = record.parent?;
        }
    }

    /// Read the innermost binding of `name`.
    pub(crate) fn lookup(&self, scope: SlotId, name: &str) -> Option<RtValue> {
        let owner = self.resolve(scope, name)?;
        self.scope(owner)?.bindings.get(name).cloned()
    }

    /// Overwrite the innermost binding of `name`. Returns the value back when no scope declares
    /// it — there is no implicit global creation (§5).
    pub(crate) fn assign(
        &mut self,
        scope: SlotId,
        name: &str,
        value: RtValue,
    ) -> Result<(), RtValue> {
        let Some(owner) = self.resolve(scope, name) else {
            return Err(value);
        };
        match self
            .scope_mut(owner)
            .and_then(|record| record.bindings.get_mut(name))
        {
            Some(slot) => {
                *slot = value;
                Ok(())
            }
            None => Err(value),
        }
    }
}
