//! What `bin.inspect` answers, and the budget that keeps the answer small enough for an agent to read.

use std::collections::BTreeMap;

use serde::Serialize;

/// Names, symbols, files and strings share this many characters; with the JSON around them an answer stays under
/// 4,000 tokens (`tokens::estimate`).
pub const BUDGET_CHARS: usize = 9_000;
/// A longer name is cut to this many bytes.
pub const MAX_NAME_LEN: usize = 256;
/// Strings of an image that has other things to say get this many characters of their own, so symbols cannot take
/// them all; a file of unknown format spends the whole [`BUDGET_CHARS`] on them.
pub const STRINGS_CHARS: usize = 2_000;
/// What each entry costs on top of its text: quotes, a comma and the key names around it.
const ENTRY_COST: usize = 8;
/// What a section costs besides its name: its address, size and flags in the JSON.
pub const SECTION_COST: usize = 80;
/// The most sections or segments listed.
pub const MAX_SECTIONS: usize = 96;

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Section {
    pub name: String,
    pub address: String,
    pub size: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub raw_size: u64,
    pub flags: Vec<&'static str>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// A library the image depends on and the symbols taken from it; an import by ordinal is `#<ordinal>`.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Library {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
}

/// One entry of a package, bundle or source tree.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub kind: String,
}

/// The answer, whatever the format. Empty parts are left out of the JSON.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Image {
    pub kind: &'static str,
    pub format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endian: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<Library>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub strings: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub notes: BTreeMap<&'static str, String>,
    pub entropy: f64,
    pub truncated: bool,
}

/// The characters left for names; once spent, nothing more is taken and the answer says `truncated`.
#[derive(Debug)]
pub struct Budget {
    left: usize,
    pub truncated: bool,
}

impl Default for Budget {
    fn default() -> Self {
        Budget::new(BUDGET_CHARS)
    }
}

impl Budget {
    pub fn new(chars: usize) -> Self {
        Budget {
            left: chars,
            truncated: false,
        }
    }

    /// `bytes` as text cut to [`MAX_NAME_LEN`], charged to the budget; `None` once the budget is spent.
    pub fn take(&mut self, bytes: &[u8]) -> Option<String> {
        self.take_costing(bytes, 0)
    }

    /// Like [`Budget::take`] for an entry that also carries `extra` characters of fields besides its name.
    pub fn take_costing(&mut self, bytes: &[u8], extra: usize) -> Option<String> {
        let cut = &bytes[..bytes.len().min(MAX_NAME_LEN)];
        let cost = cut.len() + ENTRY_COST + extra;
        if cost > self.left {
            self.left = 0;
            self.truncated = true;
            return None;
        }
        self.left -= cost;
        self.truncated |= cut.len() < bytes.len();
        Some(String::from_utf8_lossy(cut).into_owned())
    }

    /// Takes every name of `names` while the budget lasts.
    pub fn take_all<'a>(&mut self, names: impl IntoIterator<Item = &'a [u8]>) -> Vec<String> {
        let mut taken = Vec::new();
        for name in names {
            match self.take(name) {
                Some(text) => taken.push(text),
                None => break,
            }
        }
        taken
    }

    /// Libraries with their symbols: every name first, then the symbols of each in turn, while the budget lasts.
    pub fn libraries(&mut self, raw: Vec<(String, Vec<String>)>) -> Vec<Library> {
        let mut libraries = Vec::new();
        let mut pending = Vec::new();
        for (name, symbols) in raw {
            let Some(name) = self.take(name.as_bytes()) else {
                break;
            };
            libraries.push(Library {
                name,
                symbols: Vec::new(),
            });
            pending.push(symbols);
        }
        for (library, symbols) in libraries.iter_mut().zip(pending) {
            library.symbols = self.take_all(symbols.iter().map(String::as_bytes));
        }
        libraries
    }

    /// Notes that a cap other than the budget cut something.
    pub fn cut(&mut self, cut: bool) {
        self.truncated |= cut;
    }
}

/// `0x...` text, so 64-bit values survive JSON.
pub fn hex(value: u64) -> String {
    format!("{value:#x}")
}

/// The name a client expects for an architecture.
pub fn arch_name(arch: object::Architecture) -> String {
    use object::Architecture as A;
    match arch {
        A::X86_64 | A::X86_64_X32 => "x86_64".into(),
        A::I386 => "x86".into(),
        A::Aarch64 | A::Aarch64_Ilp32 => "arm64".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// `little` or `big`.
pub fn endian_name(little: bool) -> &'static str {
    ["big", "little"][usize::from(little)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_cut_and_charged_and_a_spent_budget_takes_nothing() {
        let mut budget = Budget::new(270);
        let long = vec![b'a'; 400];
        assert_eq!(budget.take(&long).unwrap().len(), MAX_NAME_LEN);
        assert!(budget.truncated);
        assert_eq!(budget.take(b"x"), None);
        assert_eq!(budget.take(b"y"), None);
    }

    #[test]
    fn take_all_stops_at_the_first_name_the_budget_cannot_pay() {
        let mut budget = Budget::new(2 * (4 + ENTRY_COST));
        let names: [&[u8]; 3] = [b"abcd", b"efgh", b"ijkl"];
        assert_eq!(budget.take_all(names), ["abcd", "efgh"]);
        assert!(budget.truncated);
    }

    #[test]
    fn libraries_get_their_names_first_and_their_symbols_after() {
        let raw = |name: &str, symbols: &[&str]| {
            (
                name.to_string(),
                symbols.iter().map(|s| s.to_string()).collect(),
            )
        };
        // Three names cost 27 and leave 11: room for the first symbol only.
        let mut budget = Budget::new(38);
        let libraries = budget.libraries(vec![
            raw("a", &["s1", "s2"]),
            raw("b", &["t1"]),
            raw("c", &[]),
        ]);
        let shape: Vec<(&str, usize)> = libraries
            .iter()
            .map(|l| (l.name.as_str(), l.symbols.len()))
            .collect();
        assert_eq!(shape, [("a", 1), ("b", 0), ("c", 0)]);
        assert!(budget.truncated);
        let mut tiny = Budget::new(ENTRY_COST + 1);
        assert_eq!(
            tiny.libraries(vec![raw("a", &["s"]), raw("b", &[])]).len(),
            1
        );
    }

    #[test]
    fn endian_names() {
        assert_eq!((endian_name(true), endian_name(false)), ("little", "big"));
    }
}
