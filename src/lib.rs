//! Deterministic, syntax-aware source analysis for Dendrite. Every operation is a pure function of its inputs.

pub mod complexity;
pub mod error;
pub mod fold;
pub mod hash;
pub mod lang;
pub mod last_node;
pub mod parse;
pub mod rpc;
pub mod symbols;
pub mod tokens;

pub use error::CoreError;
pub use lang::Lang;
