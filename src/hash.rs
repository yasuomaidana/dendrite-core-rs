//! BLAKE3 digests. Chunk identity is shared with every other Dendrite implementation, byte for byte.

/// The lowercase hex BLAKE3 digest of `data`.
pub fn blake3_hex(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// The identity of a chunk: `blake3(path 0x00 symbol 0x00 content)`. The NUL separators keep
/// `("a", "bc")` and `("ab", "c")` apart.
pub fn chunk_id(path: &str, symbol: &str, content: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in [path, symbol] {
        hasher.update(part.as_bytes());
        hasher.update(&[0]);
    }
    hasher.update(content.as_bytes());
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Vectors {
        blake3: Vec<(String, String)>,
        chunk_id: Vec<(String, String, String, String)>,
    }

    const VECTORS: &str = include_str!("../tests/vectors/blake3.json");

    #[test]
    fn matches_the_shared_vectors() {
        let vectors: Vectors = serde_json::from_str(VECTORS).unwrap();
        assert!(!vectors.blake3.is_empty() && !vectors.chunk_id.is_empty());
        for (input, hex) in &vectors.blake3 {
            assert_eq!(&blake3_hex(input.as_bytes()), hex, "blake3({input:?})");
        }
        for (path, symbol, content, hex) in &vectors.chunk_id {
            assert_eq!(
                &chunk_id(path, symbol, content),
                hex,
                "chunk_id({path:?}, {symbol:?})"
            );
        }
    }

    #[test]
    fn chunk_id_is_the_digest_of_nul_joined_fields() {
        assert_eq!(
            chunk_id("a/b.go", "F", "body"),
            blake3_hex(b"a/b.go\0F\0body")
        );
    }

    #[test]
    fn separators_keep_fields_apart() {
        assert_ne!(chunk_id("a", "bc", "x"), chunk_id("ab", "c", "x"));
    }
}
