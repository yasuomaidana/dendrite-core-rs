//! Token counts. Without a local tokenizer for the model the count is an estimate and says so.

use serde::Serialize;

/// A token count and whether it is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TokenCount {
    pub count: usize,
    pub exact: bool,
}

/// The estimate `ceil(chars / 4 * 1.15)`: callers keep this safety margin, or ask the provider's own endpoint.
pub fn estimate(text: &str) -> usize {
    (text.chars().count() * 115).div_ceil(400)
}

/// Counts the tokens of `text` for `model`. No tokenizer files are bundled, so every count is an estimate
/// (`exact = false`); the model name is accepted so exact tokenizers can be added without changing callers.
pub fn count(_model: &str, text: &str) -> TokenCount {
    TokenCount {
        count: estimate(text),
        exact: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("", 0)]
    #[case("abcd", 2)]
    #[case("a", 1)]
    #[case(&"x".repeat(400), 115)]
    #[case("héllo wörld ✓", 4)]
    fn estimates_with_the_margin(#[case] text: &str, #[case] want: usize) {
        assert_eq!(estimate(text), want);
    }

    #[test]
    fn counts_are_marked_inexact() {
        assert_eq!(
            count("qwen", "abcd"),
            TokenCount {
                count: 2,
                exact: false
            }
        );
    }
}
