//! The per-execution heap: every collection and scope an execution creates lives in one arena
//! owned by its `Machine`, and runtime values are plain handles into it.
//!
//! Nothing inside the heap is reference counted, so a reference cycle (`let a = []; a[0] = a;`,
//! or a Story 1.7 closure capturing its own scope) costs nothing extra: when the execution
//! ends — by result or by error — the `Machine` drops and the whole heap goes with it. There is
//! no collection within an execution: garbage stays until the execution ends (Story 3.5's
//! memory budget bounds it). The one exception is scopes, reclaimed eagerly on block exit.
//!
//! Dropping the heap is flat by construction: slots hold handles, never owned children.

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::environment::ScopeRecord;
use crate::value::{Array, Object, Value};

/// A handle to one heap slot. Meaningful only within the heap that issued it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SlotId(usize);

/// A runtime value. Collections are handles, so cloning one aliases the same collection and
/// `==` on collections is handle identity (§4.2).
#[derive(Clone)]
pub(crate) enum RtValue {
    Null,
    Bool(bool),
    /// Always finite: every operation that would produce a non-finite result raises instead.
    Number(f64),
    /// Strings are immutable and cannot contain other values, so sharing them through `Arc`
    /// can never form a cycle.
    String(Arc<str>),
    Array(SlotId),
    Object(SlotId),
}

impl RtValue {
    /// The type name used in diagnostics: `null`, `bool`, `number`, `string`, `array`, `object`.
    pub(crate) const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    pub(crate) const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// §4.2 language equality (`==`). Same type compares directly, collections by identity;
    /// number vs string converts the string (a non-numeric string is simply unequal); every
    /// other cross-type pair is unequal.
    pub(crate) fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) | (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Number(n), Self::String(s)) | (Self::String(s), Self::Number(n)) => {
                crate::convert::parse_number(s) == Some(*n)
            }
            _ => false,
        }
    }
}

pub(crate) enum Slot {
    /// Reclaimed; its id is on the free list.
    Free,
    Array(Vec<RtValue>),
    Object(IndexMap<Arc<str>, RtValue>),
    Scope(ScopeRecord),
}

#[derive(Default)]
pub(crate) struct Heap {
    slots: Vec<Slot>,
    free: Vec<SlotId>,
}

impl Heap {
    pub(crate) fn alloc(&mut self, slot: Slot) -> SlotId {
        while let Some(id) = self.free.pop() {
            if let Some(free @ Slot::Free) = self.slots.get_mut(id.0) {
                *free = slot;
                return id;
            }
        }
        self.slots.push(slot);
        SlotId(self.slots.len() - 1)
    }

    /// Return a slot to the free list, dropping its contents. Contents are handles, so this is
    /// shallow; any collection it referenced stays until the execution ends.
    pub(crate) fn release(&mut self, id: SlotId) {
        if let Some(slot) = self.slots.get_mut(id.0)
            && !matches!(slot, Slot::Free)
        {
            *slot = Slot::Free;
            self.free.push(id);
        }
    }

    pub(crate) fn slot(&self, id: SlotId) -> Option<&Slot> {
        self.slots.get(id.0)
    }

    pub(crate) fn slot_mut(&mut self, id: SlotId) -> Option<&mut Slot> {
        self.slots.get_mut(id.0)
    }

    pub(crate) fn new_array(&mut self, items: Vec<RtValue>) -> RtValue {
        RtValue::Array(self.alloc(Slot::Array(items)))
    }

    pub(crate) fn new_object(&mut self, entries: IndexMap<Arc<str>, RtValue>) -> RtValue {
        RtValue::Object(self.alloc(Slot::Object(entries)))
    }

    /// The elements of the array at `id`; empty for a handle that is not an array (which the
    /// machine never produces).
    fn elements(&self, id: SlotId) -> &[RtValue] {
        match self.slot(id) {
            Some(Slot::Array(items)) => items,
            _ => &[],
        }
    }

    fn entries(&self, id: SlotId) -> Option<&IndexMap<Arc<str>, RtValue>> {
        match self.slot(id) {
            Some(Slot::Object(entries)) => Some(entries),
            _ => None,
        }
    }

    pub(crate) fn array_len(&self, id: SlotId) -> usize {
        self.elements(id).len()
    }

    pub(crate) fn array_get(&self, id: SlotId, index: usize) -> Option<RtValue> {
        self.elements(id).get(index).cloned()
    }

    /// Set `index`, or append when `index` is exactly the length. Returns `false` otherwise.
    pub(crate) fn array_store(&mut self, id: SlotId, index: usize, value: RtValue) -> bool {
        let Some(Slot::Array(items)) = self.slot_mut(id) else {
            return false;
        };
        if let Some(slot) = items.get_mut(index) {
            *slot = value;
            true
        } else if index == items.len() {
            items.push(value);
            true
        } else {
            false
        }
    }

    pub(crate) fn object_get(&self, id: SlotId, key: &str) -> Option<RtValue> {
        self.entries(id)?.get(key).cloned()
    }

    /// Replace an existing key in place, or append a new one at the end.
    pub(crate) fn object_store(&mut self, id: SlotId, key: &str, value: RtValue) {
        if let Some(Slot::Object(entries)) = self.slot_mut(id) {
            if let Some(slot) = entries.get_mut(key) {
                *slot = value;
            } else {
                entries.insert(Arc::from(key), value);
            }
        }
    }

    /// §4.1 truthiness: `null`, `false`, `0`, `""`, and empty collections are falsy.
    pub(crate) fn is_truthy(&self, value: &RtValue) -> bool {
        match value {
            RtValue::Null => false,
            RtValue::Bool(b) => *b,
            RtValue::Number(n) => *n != 0.0,
            RtValue::String(s) => !s.is_empty(),
            RtValue::Array(id) => !self.elements(*id).is_empty(),
            RtValue::Object(id) => self.entries(*id).is_some_and(|e| !e.is_empty()),
        }
    }

    /// Copy `root` and everything reachable from it out of the heap into an owned [`Value`].
    /// `None` when the reachable graph contains a cycle.
    ///
    /// Walks with an explicit stack, so nesting depth never grows the host stack. A collection
    /// is "on the path" from when it is entered until its children are done; meeting one again
    /// in that window is a cycle. Meeting one again after it is done is sharing (`[x, x]`), not
    /// a cycle, and reuses the memoized detached form, so shared structure is detached once.
    pub(crate) fn detach(&self, root: &RtValue) -> Option<Value> {
        enum Visit {
            Enter(RtValue),
            Finish(SlotId),
        }
        enum Mark {
            OnPath,
            Done(Value),
        }
        let mut work = vec![Visit::Enter(root.clone())];
        let mut out: Vec<Value> = Vec::new();
        let mut marks: HashMap<SlotId, Mark> = HashMap::new();
        while let Some(visit) = work.pop() {
            match visit {
                Visit::Enter(value) => {
                    let id = match value {
                        RtValue::Null => {
                            out.push(Value::Null);
                            continue;
                        }
                        RtValue::Bool(b) => {
                            out.push(Value::Bool(b));
                            continue;
                        }
                        RtValue::Number(n) => {
                            out.push(Value::Number(n));
                            continue;
                        }
                        RtValue::String(s) => {
                            out.push(Value::String(s));
                            continue;
                        }
                        RtValue::Array(id) | RtValue::Object(id) => id,
                    };
                    match marks.get(&id) {
                        Some(Mark::Done(done)) => {
                            out.push(done.clone());
                            continue;
                        }
                        Some(Mark::OnPath) => return None,
                        None => {}
                    }
                    marks.insert(id, Mark::OnPath);
                    work.push(Visit::Finish(id));
                    // Reversed so children are entered, and land on `out`, in order.
                    match self.slot(id) {
                        Some(Slot::Array(items)) => {
                            work.extend(items.iter().rev().cloned().map(Visit::Enter));
                        }
                        Some(Slot::Object(entries)) => {
                            work.extend(entries.values().rev().cloned().map(Visit::Enter));
                        }
                        _ => {}
                    }
                }
                Visit::Finish(id) => {
                    let detached = match self.slot(id) {
                        Some(Slot::Array(items)) => {
                            let at = out.len().saturating_sub(items.len());
                            Value::Array(Array::new(out.split_off(at)))
                        }
                        Some(Slot::Object(entries)) => {
                            let at = out.len().saturating_sub(entries.len());
                            let values = out.split_off(at);
                            Value::Object(Object::new(
                                entries.keys().cloned().zip(values).collect(),
                            ))
                        }
                        _ => Value::Null,
                    };
                    marks.insert(id, Mark::Done(detached.clone()));
                    out.push(detached);
                }
            }
        }
        out.pop()
    }

    /// Number of slots ever allocated (live or free).
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Number of slots currently in use.
    #[cfg(test)]
    pub(crate) fn live(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| !matches!(s, Slot::Free))
            .count()
    }

    /// Every string held directly by a collection slot — lets a test keep a strong handle to a
    /// heap-resident string and watch it be released when the heap drops.
    #[cfg(test)]
    pub(crate) fn strings(&self) -> Vec<Arc<str>> {
        let mut found = Vec::new();
        for slot in &self.slots {
            let values: Vec<&RtValue> = match slot {
                Slot::Array(items) => items.iter().collect(),
                Slot::Object(entries) => entries.values().collect(),
                _ => Vec::new(),
            };
            for value in values {
                if let RtValue::String(s) = value {
                    found.push(Arc::clone(s));
                }
            }
        }
        found
    }

    /// Whether some array slot holds a handle to itself.
    #[cfg(test)]
    pub(crate) fn has_self_containing_array(&self) -> bool {
        self.slots.iter().enumerate().any(|(i, slot)| {
            matches!(slot, Slot::Array(items)
                if items.iter().any(|v| matches!(v, RtValue::Array(id) if id.0 == i)))
        })
    }
}
