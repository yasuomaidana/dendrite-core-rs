//! The errors every operation reports.

use thiserror::Error;

/// Why an operation failed. `code` and `kind` are what the JSON-RPC server sends back.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("unknown language `{0}`")]
    UnknownLanguage(String),
    #[error("method `{0}` not found")]
    MethodNotFound(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("the buffer holds no complete node")]
    NoCompleteNode,
    #[error("not a PE image: {0}")]
    NotPe(String),
}

impl CoreError {
    /// JSON-RPC error code: -32601 for an unknown method, -32602 for bad input, -32000 for an operation that failed.
    pub fn code(&self) -> i64 {
        match self {
            CoreError::MethodNotFound(_) => -32601,
            CoreError::UnknownLanguage(_) | CoreError::InvalidParams(_) | CoreError::NotPe(_) => {
                -32602
            }
            _ => -32000,
        }
    }

    /// The `data.kind` of an operation error, or of a file that is not a PE image; empty for other bad input.
    pub fn kind(&self) -> &'static str {
        match self {
            CoreError::NoCompleteNode => "no_complete_node",
            CoreError::NotPe(_) => "not_pe",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(CoreError::UnknownLanguage("x".into()), -32602, "")]
    #[case(CoreError::InvalidParams("x".into()), -32602, "")]
    #[case(CoreError::MethodNotFound("x".into()), -32601, "")]
    #[case(CoreError::NoCompleteNode, -32000, "no_complete_node")]
    #[case(CoreError::NotPe("x".into()), -32602, "not_pe")]
    fn codes_and_kinds(#[case] err: CoreError, #[case] code: i64, #[case] kind: &str) {
        assert_eq!((err.code(), err.kind()), (code, kind));
        assert!(!err.to_string().is_empty());
    }
}
