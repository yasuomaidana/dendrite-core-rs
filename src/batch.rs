//! Many files in one request, on every core. A request for one file spends most of its time parsing, and
//! parsing is the part that scales: the same 489 Go files that take 0.31 s one by one take about a quarter of
//! that on four cores (DCORE-13). Results come back in request order, whatever order the threads finish in.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::cache::{key, Cache};
use crate::complexity::{complexity, FunctionMetrics};
use crate::lang::Lang;

/// One file of a batch.
#[derive(Debug, Deserialize)]
pub struct BatchFile {
    pub lang: String,
    pub source: String,
}

/// What one file of a batch came to: its functions, or why it has none. One bad file does not fail the batch.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<FunctionMetrics>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Results the sidecar has already computed, shared by every request it serves. A file whose language and bytes
/// were seen before is answered from here without parsing.
pub static CACHE: LazyLock<Cache> = LazyLock::new(|| Cache::new(CACHE_ENTRIES));

/// How many files' results the shared cache holds.
const CACHE_ENTRIES: usize = 16_384;

/// Measures every file, using up to `workers` threads and the shared cache.
pub fn complexity_batch(files: &[BatchFile], workers: usize) -> Vec<FileResult> {
    complexity_batch_in(&CACHE, files, workers)
}

/// [`complexity_batch`] against a cache of the caller's choosing.
pub fn complexity_batch_in(cache: &Cache, files: &[BatchFile], workers: usize) -> Vec<FileResult> {
    par_map(files, workers, |file| match Lang::from_id(&file.lang) {
        Ok(lang) => FileResult {
            functions: Some(cached(cache, file, lang).to_vec()),
            error: None,
        },
        Err(e) => FileResult {
            functions: None,
            error: Some(e.to_string()),
        },
    })
}

/// The functions of `file`: from the cache when its language and bytes were seen before, else measured.
fn cached(cache: &Cache, file: &BatchFile, lang: Lang) -> Arc<Vec<FunctionMetrics>> {
    // Keyed by the resolved language, so `go`/`golang` or ` TS ` and `ts` share one entry.
    let k = key(&lang.index().to_string(), &file.source);
    cache
        .get(&k)
        .unwrap_or_else(|| cache.insert(k, complexity(lang, &file.source)))
}

/// The threads a batch of `items` files should use: one per core, never more than there are files.
pub fn default_workers(items: usize) -> usize {
    thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(items)
        .max(1)
}

/// `f` over every item, in order, on up to `workers` threads that take the next unclaimed item as they finish
/// the last, so a few large files do not leave other threads idle. Tree-sitter parsers are kept per thread.
fn par_map<T: Sync, R: Send>(items: &[T], workers: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let workers = workers.clamp(1, items.len().max(1));
    if workers == 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else {
                            return mine;
                        };
                        mine.push((i, f(item)));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a worker does not panic"))
            .collect()
    });
    done.sort_unstable_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn files() -> Vec<BatchFile> {
        (0..40)
            .map(|n| BatchFile {
                lang: "go".into(),
                source: format!("package p\nfunc f{n}() {{{}}}\n", "if a {}\n".repeat(n)),
            })
            .collect()
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(7)]
    #[case(64)]
    fn any_worker_count_gives_the_sequential_answer(files: Vec<BatchFile>, #[case] workers: usize) {
        assert_eq!(
            complexity_batch(&files, workers),
            complexity_batch(&files, 1)
        );
    }

    #[rstest]
    fn results_keep_request_order(files: Vec<BatchFile>) {
        let out = complexity_batch(&files, 4);
        for (n, result) in out.iter().enumerate() {
            let functions = result.functions.as_ref().expect("parsed");
            assert_eq!(functions[0].symbol, format!("f{n}"));
            assert_eq!(functions[0].cyclomatic, 1 + n as u32);
        }
    }

    #[rstest]
    #[case("go", "GO")]
    #[case("kotlin", "kt")]
    #[case("ts", " TS ")]
    fn language_aliases_share_one_cache_entry(#[case] a: &str, #[case] b: &str) {
        let cache = Cache::new(64);
        let source = "fun f() {}\n";
        let batch = |lang: &str| {
            vec![BatchFile {
                lang: lang.into(),
                source: source.into(),
            }]
        };
        complexity_batch_in(&cache, &batch(a), 1);
        assert_eq!(cache.stats().misses, 1);
        complexity_batch_in(&cache, &batch(b), 1);
        assert_eq!(cache.stats().misses, 1, "the alias hits the same entry");
    }

    #[test]
    fn repeated_content_is_parsed_once() {
        let cache = Cache::new(64);
        let mut files = files();
        files.extend(files_copy());
        let first = complexity_batch_in(&cache, &files, 1);
        assert_eq!(cache.stats().misses, 40, "the 40 distinct files");
        let second = complexity_batch_in(&cache, &files, 4);
        assert_eq!(first, second);
        assert_eq!(cache.stats().misses, 40, "nothing is parsed twice");
    }

    fn files_copy() -> Vec<BatchFile> {
        files()
    }

    #[test]
    fn one_bad_file_does_not_fail_the_batch() {
        let files = vec![
            BatchFile {
                lang: "cobol".into(),
                source: String::new(),
            },
            BatchFile {
                lang: "go".into(),
                source: "package p\nfunc f() {}\n".into(),
            },
        ];
        let out = complexity_batch(&files, 2);
        assert!(out[0].functions.is_none());
        assert!(out[0].error.as_ref().unwrap().contains("cobol"));
        assert_eq!(out[1].functions.as_ref().unwrap().len(), 1);
    }

    #[rstest]
    #[case(0, 1)]
    #[case(1, 1)]
    fn an_empty_or_single_batch_needs_one_worker(#[case] items: usize, #[case] want: usize) {
        assert_eq!(default_workers(items), want);
        assert!(complexity_batch(&[], default_workers(0)).is_empty());
    }

    #[test]
    fn a_big_batch_uses_no_more_threads_than_cores() {
        let cores = thread::available_parallelism().map_or(1, NonZeroUsize::get);
        assert_eq!(default_workers(10_000), cores);
    }
}
