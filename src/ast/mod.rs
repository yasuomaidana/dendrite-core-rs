//! Syntax-aware analysis of source code: languages and their node tables, parsing, complexity, folding, the
//! last complete node of a truncated buffer, changed symbols and the symbols a range defines and uses.

pub mod complexity;
pub mod defs_uses;
pub mod embedded;
pub mod fold;
pub mod go;
pub mod kinds;
pub mod lang;
pub mod last_node;
pub mod parse;
mod specs;
pub mod symbols;
