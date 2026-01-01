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

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;

pub use ast::{Ast, Expression, Statement};
pub use error::SyntaxError;
pub use parser::parse;
