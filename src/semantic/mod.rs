//! Semantic analysis layer
//!
//! This module handles:
//! - Name resolution
//! - Symbol tables
//! - Capability checking
//! - Type checking (future)
//!
//! This layer sits between syntax and execution.

pub mod resolver;
pub mod symbols;
pub mod capabilities;
pub mod error;

pub use resolver::resolve;
pub use symbols::SymbolTable;
pub use capabilities::Capability;
pub use error::SemanticError;
