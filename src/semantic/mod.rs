//! Semantic analysis layer
//!
//! This module handles:
//! - Name resolution
//! - Symbol tables
//! - Capability checking
//! - Type checking (future)
//!
//! This layer sits between syntax and execution.

pub mod capabilities;
pub mod error;
pub mod resolver;
pub mod symbols;

pub use capabilities::Capability;
pub use error::SemanticError;
pub use resolver::resolve;
pub use symbols::SymbolTable;
