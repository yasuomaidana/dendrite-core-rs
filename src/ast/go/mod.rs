//! Go without a syntax tree: a lexer and a scan that give the complexity of Go functions several times faster
//! than parsing them.

pub mod fast;
mod lex;
