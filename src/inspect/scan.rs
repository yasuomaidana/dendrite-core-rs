//! What can be said about any bytes: how random they look and which readable strings they hold.

use super::image::Budget;

/// Only this much of a file is searched for strings.
pub const MAX_SCAN: usize = 8 * 1024 * 1024;
/// A run of printable characters shorter than this is not a string worth reading.
const MIN_LEN: usize = 6;
/// A longer string is cut to this many bytes.
const MAX_LEN: usize = 120;
/// Candidates kept for each rank, so memory stays bounded however many strings the file holds.
const PER_RANK: usize = 400;

/// Shannon entropy in bits per byte, 0 to 8; near 8 suggests packing or encryption.
pub fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u64; 256];
    for &byte in data {
        counts[usize::from(byte)] += 1;
    }
    let total = data.len() as f64;
    let bits: f64 = counts
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f64 / total;
            -p * p.log2()
        })
        .sum();
    (bits * 100.0).round() / 100.0
}

fn printable(byte: u8) -> bool {
    matches!(byte, b' '..=b'~' | b'\t')
}

/// URLs first, then paths, then format strings, then the rest.
fn rank(text: &[u8]) -> usize {
    let has = |needle: &[u8]| text.windows(needle.len()).any(|window| window == needle);
    if has(b"://") {
        0
    } else if text[0] == b'/' || has(b":\\") || has(b"\\\\") {
        1
    } else if has(b"%s") || has(b"%d") || has(b"%x") {
        2
    } else {
        3
    }
}

/// Every printable run of at least [`MIN_LEN`] bytes, cut to [`MAX_LEN`].
fn runs(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    data.split(|&byte| !printable(byte))
        .filter(|run| run.len() >= MIN_LEN)
        .map(|run| &run[..run.len().min(MAX_LEN)])
}

/// The strings worth reading, best first, as many as the budget allows.
pub fn strings(data: &[u8], budget: &mut Budget) -> Vec<String> {
    let scanned = &data[..data.len().min(MAX_SCAN)];
    let mut ranked: [Vec<&[u8]>; 4] = Default::default();
    for run in runs(scanned) {
        let bucket = &mut ranked[rank(run)];
        if bucket.len() < PER_RANK {
            bucket.push(run);
        }
    }
    budget.cut(data.len() > MAX_SCAN);
    budget.take_all(ranked.iter().flatten().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::empty(&[], 0.0)]
    #[case::one_byte_value(&[7; 64], 0.0)]
    #[case::two_values(&[0, 1, 0, 1], 1.0)]
    fn entropy_of_small_inputs(#[case] data: &[u8], #[case] want: f64) {
        assert_eq!(entropy(data), want);
    }

    #[test]
    fn every_byte_value_equally_often_is_eight_bits() {
        let data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
        assert_eq!(entropy(&data), 8.0);
    }

    #[test]
    fn strings_come_best_first_and_short_runs_are_dropped() {
        let data = b"\0abc\0hello world\0usage: %s file\0/etc/passwd\0see https://x.test/a\0";
        let found = strings(data, &mut Budget::default());
        assert_eq!(
            found,
            [
                "see https://x.test/a",
                "/etc/passwd",
                "usage: %s file",
                "hello world"
            ]
        );
    }

    #[test]
    fn a_windows_path_ranks_as_a_path() {
        let found = strings(b"\0plain text\0C:\\Windows\\x\0", &mut Budget::default());
        assert_eq!(found, ["C:\\Windows\\x", "plain text"]);
    }

    #[test]
    fn a_long_run_is_cut_and_a_small_budget_cuts_the_list() {
        let long = [b'a'; 300];
        let found = strings(&long, &mut Budget::default());
        assert_eq!(found[0].len(), MAX_LEN);
        let mut budget = Budget::new(20);
        assert_eq!(strings(b"\0aaaaaaaa\0bbbbbbbb\0", &mut budget).len(), 1);
        assert!(budget.truncated);
    }

    #[test]
    fn only_the_first_part_of_a_huge_file_is_searched() {
        let mut data = vec![0u8; MAX_SCAN + 100];
        data[MAX_SCAN + 10..MAX_SCAN + 20].copy_from_slice(b"beyondscan");
        let mut budget = Budget::default();
        assert!(strings(&data, &mut budget).is_empty());
        assert!(budget.truncated);
    }

    #[test]
    fn each_rank_keeps_a_bounded_number_of_candidates() {
        let data: Vec<u8> = (0..PER_RANK + 50)
            .flat_map(|i| format!("\0string{i:05}").into_bytes())
            .collect();
        let found = strings(&data, &mut Budget::new(1_000_000));
        assert_eq!(found.len(), PER_RANK);
    }
}
