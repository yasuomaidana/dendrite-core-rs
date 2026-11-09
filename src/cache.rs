//! A bounded cache of analysis results keyed by the content of the input (DCORE-20). Analysis is a pure
//! function of the language and the source, so an answer found once is valid for every later request that
//! carries the same bytes: a re-run over a tree where little changed, or vendored copies of one file, skip the
//! parser entirely. The cache knows nothing about any language; the key is a BLAKE3 digest.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use crate::complexity::FunctionMetrics;

type Key = [u8; 32];

/// What is cached: the functions of one file. (Not generic on purpose: one instantiation, one set of covered lines.)
pub type Value = Vec<FunctionMetrics>;

struct Inner {
    map: HashMap<Key, Arc<Value>>,
    /// Keys in insertion order; the oldest is evicted first.
    order: VecDeque<Key>,
    hits: u64,
    misses: u64,
}

/// What the cache has answered so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub hits: u64,
    pub misses: u64,
}

/// A thread-safe, first-in-first-out cache of at most `capacity` results.
pub struct Cache {
    capacity: usize,
    inner: Mutex<Inner>,
}

/// The identity of an input: its language and its bytes, kept apart by a NUL so `("a", "bc")` and `("ab", "c")`
/// differ.
pub fn key(lang: &str, source: &str) -> Key {
    let mut hasher = blake3::Hasher::new();
    hasher.update(lang.as_bytes());
    hasher.update(&[0]);
    hasher.update(source.as_bytes());
    *hasher.finalize().as_bytes()
}

impl Cache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                order: VecDeque::new(),
                hits: 0,
                misses: 0,
            }),
        }
    }

    /// The cached value for `key`, counting a hit or a miss. On a miss the caller computes the value outside any
    /// lock, so a slow analysis never blocks other threads, and hands it to [`Cache::insert`].
    pub fn get(&self, key: &Key) -> Option<Arc<Value>> {
        let mut inner = self.lock();
        let found = inner.map.get(key).cloned();
        if found.is_some() {
            inner.hits += 1;
        } else {
            inner.misses += 1;
        }
        found
    }

    /// Remembers `value` under `key`, evicting the oldest entry when full, and returns the value now cached.
    /// Two threads that missed the same key both compute it; the first answer stays.
    pub fn insert(&self, key: Key, value: Value) -> Arc<Value> {
        let mut inner = self.lock();
        if let Some(existing) = inner.map.get(&key) {
            return Arc::clone(existing);
        }
        let value = Arc::new(value);
        if self.capacity > 0 {
            if inner.map.len() >= self.capacity {
                let oldest = inner.order.pop_front();
                oldest.map(|k| inner.map.remove(&k));
            }
            inner.map.insert(key, Arc::clone(&value));
            inner.order.push_back(key);
        }
        value
    }

    pub fn stats(&self) -> Stats {
        let inner = self.lock();
        Stats {
            hits: inner.hits,
            misses: inner.misses,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(n: u32) -> Value {
        vec![FunctionMetrics {
            symbol: "f".into(),
            start_line: 1,
            end_line: 1,
            cyclomatic: n,
            cognitive: 0,
        }]
    }

    fn lookup(cache: &Cache, k: Key, n: u32) -> Arc<Value> {
        cache.get(&k).unwrap_or_else(|| cache.insert(k, value(n)))
    }

    #[test]
    fn a_second_request_for_the_same_input_is_a_hit() {
        let cache = Cache::new(8);
        let k = key("go", "package p");
        assert_eq!(*lookup(&cache, k, 7), value(7));
        assert_eq!(*lookup(&cache, k, 99), value(7), "the cached 7, not 99");
        assert_eq!(cache.stats(), Stats { hits: 1, misses: 1 });
    }

    #[test]
    fn language_and_source_are_both_part_of_the_key() {
        assert_ne!(key("a", "bc"), key("ab", "c"));
        assert_ne!(key("go", "x"), key("rust", "x"));
    }

    #[test]
    fn the_oldest_entry_goes_first_when_full() {
        let cache = Cache::new(2);
        for n in 0..3u32 {
            lookup(&cache, key("go", &n.to_string()), n);
        }
        let again = cache.stats().misses;
        lookup(&cache, key("go", "2"), 2);
        assert_eq!(cache.stats().misses, again, "the newest is still there");
        lookup(&cache, key("go", "0"), 0);
        assert_eq!(cache.stats().misses, again + 1, "the oldest was evicted");
    }

    #[test]
    fn a_cache_of_no_capacity_remembers_nothing() {
        let cache = Cache::new(0);
        let k = key("go", "x");
        lookup(&cache, k, 1);
        lookup(&cache, k, 1);
        assert_eq!(cache.stats(), Stats { hits: 0, misses: 2 });
    }

    #[test]
    fn a_panic_while_holding_the_lock_does_not_wedge_the_cache() {
        let cache = Arc::new(Cache::new(2));
        let c = Arc::clone(&cache);
        let _ = std::thread::spawn(move || {
            let _guard = c.lock();
            panic!("poison the lock");
        })
        .join();
        assert_eq!(*lookup(&cache, key("go", "x"), 5), value(5));
    }
}
