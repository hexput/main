//! Language frontend: lexical and syntactic analysis
//!
//! This module handles:
//! - Tokenization (lexer)
//! - Parsing (parser)
//! - AST representation
//! - Syntax errors
//!
//! This layer knows nothing about:
//! - Execution semantics
//! - Remote calls
//! - Transport protocols

pub mod lexer;
pub mod parser;
pub mod ast;
pub mod error;

pub use parser::parse;
pub use ast::{Ast, Expression, Statement};
pub use error::SyntaxError;
