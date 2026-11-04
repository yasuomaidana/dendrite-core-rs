//! Deterministic, syntax-aware source analysis for Dendrite. Every operation is a pure function of its inputs.
//!
//! The analysis of source code lives in [`ast`]; its modules are re-exported here, so `dendrite_core::complexity`,
//! `dendrite_core::fold`, `dendrite_core::Lang` and the rest keep their paths.

pub mod ast;
pub mod batch;
pub mod error;
pub mod hash;
pub mod rpc;
pub mod tokens;

pub use ast::defs_uses::{defs_uses, RangeSymbols};
pub use ast::{complexity, defs_uses, embedded, fold, kinds, lang, last_node, parse, symbols};
pub use error::CoreError;
pub use lang::Lang;

/// Compiles the Rust examples of the README as doctests, so they cannot drift from the code.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
