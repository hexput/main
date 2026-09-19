//! Runtime values (LANGUAGE-REFERENCE §3).
//!
//! Collections are shared, mutable, and compared by identity, so they are `Arc<Mutex<…>>`: every
//! clone of a [`Value::Array`] is the same array. The mutex keeps [`Value`] `Send` so a future
//! Executor can hold a suspended evaluation across an `.await`; every critical section is short,
//! synchronous, never nested, and recovers from poisoning instead of panicking.
//!
//! `Value` deliberately has no `PartialEq`: structural equality is not language equality
//! (§4.2 compares collections by identity and some cross-type pairs by conversion). Use
//! [`Value::equals`] for the language's `==`.

use core::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use indexmap::IndexMap;

/// One Hexput value.
///
/// `#[non_exhaustive]` because Story 1.7 adds functions as values.
#[derive(Clone)]
#[non_exhaustive]
pub enum Value {
    Null,
    Bool(bool),
    /// Always finite: every operation that would produce a non-finite result raises instead.
    Number(f64),
    String(Arc<str>),
    Array(Array),
    Object(Object),
}

impl Value {
    /// The type name used in diagnostics: `null`, `bool`, `number`, `string`, `array`, `object`.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// §4.1 truthiness: `null`, `false`, `0`, `""`, and empty collections are falsy.
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            Self::String(s) => !s.is_empty(),
            Self::Array(a) => !a.is_empty(),
            Self::Object(o) => !o.is_empty(),
        }
    }

    /// §4.2 language equality (`==`). Same type compares directly, collections by identity;
    /// number vs string converts the string (a non-numeric string is simply unequal); every
    /// other cross-type pair is unequal.
    #[must_use]
    pub fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a.ptr_eq(b),
            (Self::Object(a), Self::Object(b)) => a.ptr_eq(b),
            (Self::Number(n), Self::String(s)) | (Self::String(s), Self::Number(n)) => {
                crate::convert::parse_number(s) == Some(*n)
            }
            _ => false,
        }
    }

    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_array(&self) -> Option<&Array> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }
}

/// Shallow on purpose: collections print their length, never their contents, so formatting a
/// deeply nested or self-containing value cannot recurse or loop.
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("Null"),
            Self::Bool(b) => write!(f, "Bool({b})"),
            Self::Number(n) => write!(f, "Number({n:?})"),
            Self::String(s) => write!(f, "String({s:?})"),
            Self::Array(a) => write!(f, "Array(len = {})", a.len()),
            Self::Object(o) => write!(f, "Object(len = {})", o.len()),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A shared, ordered array. Clones alias the same storage.
#[derive(Clone)]
pub struct Array(Arc<Mutex<Elements>>);

/// A shared, insertion-ordered object with string keys. Clones alias the same storage.
#[derive(Clone)]
pub struct Object(Arc<Mutex<Entries>>);

struct Elements(Vec<Value>);
struct Entries(IndexMap<Arc<str>, Value>);

impl Array {
    pub(crate) fn new(items: Vec<Value>) -> Self {
        Self(Arc::new(Mutex::new(Elements(items))))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.0).0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The element at `index`, if present.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Value> {
        lock(&self.0).0.get(index).cloned()
    }

    /// A snapshot of the elements; later mutation of the array does not affect it.
    #[must_use]
    pub fn to_vec(&self) -> Vec<Value> {
        lock(&self.0).0.clone()
    }

    /// Whether both handles are the same array (language identity).
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Set `index`, or append when `index` is exactly the length. Returns `false` otherwise.
    pub(crate) fn store(&self, index: usize, value: Value) -> bool {
        let mut elements = lock(&self.0);
        let items = &mut elements.0;
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
}

impl Object {
    pub(crate) fn new(entries: IndexMap<Arc<str>, Value>) -> Self {
        Self(Arc::new(Mutex::new(Entries(entries))))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.0).0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The value under `key`, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Value> {
        lock(&self.0).0.get(key).cloned()
    }

    /// A snapshot of the entries in insertion order.
    #[must_use]
    pub fn entries(&self) -> Vec<(Arc<str>, Value)> {
        lock(&self.0)
            .0
            .iter()
            .map(|(k, v)| (Arc::clone(k), v.clone()))
            .collect()
    }

    /// Whether both handles are the same object (language identity).
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Replace an existing key in place, or append a new one at the end.
    pub(crate) fn store(&self, key: &str, value: Value) {
        let mut entries = lock(&self.0);
        if let Some(slot) = entries.0.get_mut(key) {
            *slot = value;
        } else {
            entries.0.insert(Arc::from(key), value);
        }
    }
}

// Dropping a deeply nested collection must not recurse once per level: each collection that is
// about to be freed hands its children to a flat work list instead.
impl Drop for Elements {
    fn drop(&mut self) {
        drop_flat(core::mem::take(&mut self.0));
    }
}

impl Drop for Entries {
    fn drop(&mut self) {
        let values = core::mem::take(&mut self.0).into_values().collect();
        drop_flat(values);
    }
}

fn drop_flat(mut pending: Vec<Value>) {
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(Array(shared)) => {
                // `into_inner` succeeds for exactly one of the last handles, so the storage is
                // emptied here and then dropped shallowly.
                if let Some(mutex) = Arc::into_inner(shared) {
                    let mut elements = mutex.into_inner().unwrap_or_else(PoisonError::into_inner);
                    pending.append(&mut elements.0);
                }
            }
            Value::Object(Object(shared)) => {
                if let Some(mutex) = Arc::into_inner(shared) {
                    let mut entries = mutex.into_inner().unwrap_or_else(PoisonError::into_inner);
                    pending.extend(entries.0.drain(..).map(|(_, v)| v));
                }
            }
            _ => {}
        }
    }
}
