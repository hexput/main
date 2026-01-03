//! Capability-based security model
//!
//! Capabilities define what a script is allowed to do.

use std::collections::HashSet;

/// A capability represents a permission to perform an action
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Call a specific remote function
    CallRemote(String),

    /// Access a specific resource
    AccessResource(String),
}

/// Set of capabilities granted to an execution context
#[derive(Debug, Clone)]
pub struct CapabilitySet {
    capabilities: HashSet<Capability>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self {
            capabilities: HashSet::new(),
        }
    }

    pub fn with_capabilities(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            capabilities: capabilities.into_iter().collect(),
        }
    }

    pub fn grant(&mut self, capability: Capability) {
        self.capabilities.insert(capability);
    }

    pub fn has(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn can_call_remote(&self, function_name: &str) -> bool {
        // Check for exact match
        if self.has(&Capability::CallRemote(function_name.to_string())) {
            return true;
        }

        // Check for wildcard
        if self.has(&Capability::CallRemote("*".to_string())) {
            return true;
        }

        false
    }

    pub fn can_access_resource(&self, resource: &str) -> bool {
        self.has(&Capability::AccessResource(resource.to_string()))
    }
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self::new()
    }
}
