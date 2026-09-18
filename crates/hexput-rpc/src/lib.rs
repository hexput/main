//! Host-function registry, capability grants, and outbound RPC dispatch to the Backend.
//! Depends on hexput-port only.
//!
//! Note the direction: this crate does not reach enforcement at all. hexput-exec depends on
//! hexput-rpc (not the reverse) and wraps every dispatch through here with the Capability and
//! Resource Budget checks from hexput-enforce, which hexput-exec alone may depend on. An
//! `rpc -> exec` edge would invert that and create a dependency cycle.
//!
//! Binds: AD-3.
