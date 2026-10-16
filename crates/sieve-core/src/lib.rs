//! Sieve (RFC 5228) lexer, parser, rule model, generator and evaluator.
//!
//! - The syntax layer ([`parse`], [`ast`], [`printer`]) is generic: it parses
//!   any syntactically valid script into a lossless tree, including comments,
//!   and prints it back in a canonical layout.
//! - The rule model ([`model`]) is what the app edits. [`import`] maps a
//!   script onto it, keeping anything it cannot represent as raw blocks, and
//!   [`export`] generates the script again.

pub mod ast;
pub mod compat;
pub mod export;
pub mod import;
mod lexer;
pub mod metadata;
pub mod model;
mod parser;
pub mod printer;
pub mod root;
pub mod span;

pub use export::export;
pub use import::import;
pub use parser::parse;
pub use printer::{print_script, PrintOptions};
pub use span::{LineIndex, ParseError, Span};
