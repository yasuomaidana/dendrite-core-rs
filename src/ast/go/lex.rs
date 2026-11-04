//! A lexer for Go source that keeps only what the fast complexity scan needs: token kinds, byte ranges, lines
//! and whether a line break came before the token. It does one pass over the bytes and allocates one vector.
//!
//! It returns `None` for input it cannot tokenise with certainty (an unterminated string, rune or comment), so
//! the caller can fall back to the full parser.

/// What a token is. Keywords the scan acts on have their own kind; everything it only has to step over is `Op`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum K {
    Ident,
    Lit,
    AndAnd,
    OrOr,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semi,
    Comma,
    Colon,
    Dot,
    /// `=`, `:=` and the `op=` forms.
    Assign,
    /// `++` and `--`.
    IncDec,
    /// `<-`.
    Arrow,
    Star,
    Op,
    Func,
    If,
    Else,
    For,
    Switch,
    Select,
    Case,
    Break,
    Continue,
    Goto,
    Interface,
    Struct,
    Map,
    Chan,
    /// Any other keyword (`default`, `go`, `defer`, `range`, `type`, `var`, ...): it ends an expression.
    Keyword,
    /// `return` and `fallthrough`: keywords after which a line break ends the statement.
    EndKeyword,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Token {
    pub kind: K,
    pub start: u32,
    pub end: u32,
    /// The line the token starts on, from 1.
    pub line: u32,
    /// A line break (or a comment holding one) sits between this token and the one before it.
    pub nl_before: bool,
    /// A comment sits between this token and the one before it. The syntax tree keeps comments as nodes, which
    /// changes what a node's previous sibling is (`else` // note \n `if`), and so its score.
    pub comment_before: bool,
}

impl K {
    /// Whether a line break after a token of this kind ends the statement (Go's automatic semicolon).
    pub(crate) fn ends_statement(self) -> bool {
        matches!(
            self,
            K::Ident
                | K::Lit
                | K::Break
                | K::Continue
                | K::EndKeyword
                | K::IncDec
                | K::RParen
                | K::RBracket
                | K::RBrace
        )
    }
}

/// Keywords by their first letter (`a` to `z`), so a word is compared with the few that share its initial.
const KEYWORDS: [&[(&[u8], K)]; 26] = [
    &[],
    &[(b"break", K::Break)],
    &[
        (b"case", K::Case),
        (b"chan", K::Chan),
        (b"const", K::Keyword),
        (b"continue", K::Continue),
    ],
    &[(b"default", K::Keyword), (b"defer", K::Keyword)],
    &[(b"else", K::Else)],
    &[
        (b"fallthrough", K::EndKeyword),
        (b"for", K::For),
        (b"func", K::Func),
    ],
    &[(b"go", K::Keyword), (b"goto", K::Goto)],
    &[],
    &[
        (b"if", K::If),
        (b"import", K::Keyword),
        (b"interface", K::Interface),
    ],
    &[],
    &[],
    &[],
    &[(b"map", K::Map)],
    &[],
    &[],
    &[(b"package", K::Keyword)],
    &[],
    &[(b"range", K::Keyword), (b"return", K::EndKeyword)],
    &[
        (b"select", K::Select),
        (b"struct", K::Struct),
        (b"switch", K::Switch),
    ],
    &[(b"type", K::Keyword)],
    &[],
    &[(b"var", K::Keyword)],
    &[],
    &[],
    &[],
    &[],
];

fn keyword(word: &[u8]) -> Option<K> {
    let first = *word.first()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    let candidates = KEYWORDS[usize::from(first - b'a')];
    candidates
        .iter()
        .find(|(w, _)| *w == word)
        .map(|&(_, kind)| kind)
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

/// What comes at a `/`: a comment skipped, a bad one, or no comment.
enum Slash {
    Skipped,
    Unterminated,
    Division,
}

struct Lexer<'a> {
    src: &'a [u8],
    i: usize,
    line: u32,
    /// A line break has been passed since the last token.
    nl: bool,
    /// A comment has been passed since the last token.
    comment: bool,
}

impl Lexer<'_> {
    /// The byte `ahead` places on, or 0 past the end.
    fn at(&self, ahead: usize) -> u8 {
        self.src.get(self.i + ahead).copied().unwrap_or(0)
    }

    fn newline(&mut self) {
        self.line += 1;
        self.nl = true;
    }

    /// Skips white space and comments. False when a comment is never closed.
    fn skip_trivia(&mut self) -> bool {
        loop {
            match self.at(0) {
                b'\n' => {
                    self.newline();
                    self.i += 1;
                }
                b' ' | b'\t' | b'\r' => self.i += 1,
                b'/' => match self.comment_here() {
                    Slash::Skipped => {}
                    Slash::Unterminated => return false,
                    Slash::Division => return true,
                },
                _ => return true,
            }
        }
    }

    fn comment_here(&mut self) -> Slash {
        match self.at(1) {
            b'/' => {
                self.comment = true;
                while self.i < self.src.len() && self.src[self.i] != b'\n' {
                    self.i += 1;
                }
                Slash::Skipped
            }
            b'*' => self.block_comment(),
            _ => Slash::Division,
        }
    }

    fn block_comment(&mut self) -> Slash {
        self.comment = true;
        self.i += 2;
        while self.i < self.src.len() {
            match (self.src[self.i], self.at(1)) {
                (b'*', b'/') => {
                    self.i += 2;
                    return Slash::Skipped;
                }
                (b'\n', _) => self.newline(),
                _ => {}
            }
            self.i += 1;
        }
        Slash::Unterminated
    }

    /// Lexes the token at `i`, or returns `None` for input that cannot be tokenised with certainty.
    fn token(&mut self) -> Option<K> {
        let c = self.at(0);
        if is_ident_start(c) {
            return Some(self.word());
        }
        if c.is_ascii_digit() || (c == b'.' && self.at(1).is_ascii_digit()) {
            self.i = number(self.src, self.i);
            return Some(K::Lit);
        }
        match c {
            b'"' | b'\'' => self.quoted(c),
            b'`' => self.raw_string(),
            _ => {
                let (kind, len) = operator(&self.src[self.i..]);
                self.i += len;
                Some(kind)
            }
        }
    }

    fn word(&mut self) -> K {
        let start = self.i;
        while is_ident_start(self.at(0)) || self.at(0).is_ascii_digit() {
            self.i += 1;
        }
        keyword(&self.src[start..self.i]).unwrap_or(K::Ident)
    }

    /// A string or rune literal: up to the closing `quote`, with backslash escapes, and never across a line.
    fn quoted(&mut self, quote: u8) -> Option<K> {
        self.i += 1;
        loop {
            match *self.src.get(self.i)? {
                c if c == quote => break,
                b'\n' => return None,
                b'\\' => self.i += 1,
                _ => {}
            }
            self.i += 1;
        }
        self.i += 1;
        Some(K::Lit)
    }

    fn raw_string(&mut self) -> Option<K> {
        self.i += 1;
        while *self.src.get(self.i)? != b'`' {
            if self.src[self.i] == b'\n' {
                self.line += 1;
            }
            self.i += 1;
        }
        self.i += 1;
        Some(K::Lit)
    }
}

/// Tokenises `src`, or returns `None` when it holds something the scan cannot be sure about.
pub(crate) fn lex(src: &[u8]) -> Option<Vec<Token>> {
    let bom = if src.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    let mut lexer = Lexer {
        src,
        i: bom,
        line: 1,
        nl: false,
        comment: false,
    };
    let mut toks = Vec::with_capacity(src.len() / 5 + 16);
    loop {
        if !lexer.skip_trivia() {
            return None;
        }
        if lexer.i >= src.len() {
            return Some(toks);
        }
        let (start, line) = (lexer.i, lexer.line);
        let kind = lexer.token()?;
        toks.push(Token {
            kind,
            start: start as u32,
            end: lexer.i as u32,
            line,
            nl_before: lexer.nl,
            comment_before: lexer.comment,
        });
        lexer.nl = false;
        lexer.comment = false;
    }
}

/// The end of the number literal that starts at `i`: digits, letters, `_` and `.`, with the sign of an exponent.
fn number(src: &[u8], mut i: usize) -> usize {
    let hex = src[i] == b'0' && src.get(i + 1).is_some_and(|c| c | 0x20 == b'x');
    while let Some(&d) = src.get(i) {
        if !(d.is_ascii_alphanumeric() || d == b'_' || d == b'.') {
            break;
        }
        let exponent = if hex {
            d | 0x20 == b'p'
        } else {
            d | 0x20 == b'e'
        };
        i += 1;
        if exponent && matches!(src.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
    }
    i
}

/// The kind and length of the operator or delimiter at the start of `s`.
fn operator(s: &[u8]) -> (K, usize) {
    let c = s[0];
    let (c1, c2) = (
        s.get(1).copied().unwrap_or(0),
        s.get(2).copied().unwrap_or(0),
    );
    match c {
        b'&' => ampersand(c1, c2),
        b'|' => bar(c1),
        b'<' | b'>' => angle(c, c1, c2),
        b'+' | b'-' => sign(c, c1),
        b':' | b'.' => colon_or_dot(c, c1, c2),
        b'=' | b'!' | b'*' | b'/' | b'%' | b'^' => equals_family(c, c1),
        _ => (delimiter(c), 1),
    }
}

fn delimiter(c: u8) -> K {
    match c {
        b'{' => K::LBrace,
        b'}' => K::RBrace,
        b'(' => K::LParen,
        b')' => K::RParen,
        b'[' => K::LBracket,
        b']' => K::RBracket,
        b';' => K::Semi,
        b',' => K::Comma,
        _ => K::Op,
    }
}

fn ampersand(c1: u8, c2: u8) -> (K, usize) {
    match (c1, c2) {
        (b'&', _) => (K::AndAnd, 2),
        (b'^', b'=') => (K::Assign, 3),
        (b'^', _) => (K::Op, 2),
        (b'=', _) => (K::Assign, 2),
        _ => (K::Op, 1),
    }
}

fn bar(c1: u8) -> (K, usize) {
    match c1 {
        b'|' => (K::OrOr, 2),
        b'=' => (K::Assign, 2),
        _ => (K::Op, 1),
    }
}

fn angle(c: u8, c1: u8, c2: u8) -> (K, usize) {
    if c == b'<' && c1 == b'-' {
        return (K::Arrow, 2);
    }
    if c1 == c && c2 == b'=' {
        return (K::Assign, 3);
    }
    if c1 == c || c1 == b'=' {
        return (K::Op, 2);
    }
    (K::Op, 1)
}

fn sign(c: u8, c1: u8) -> (K, usize) {
    match c1 {
        x if x == c => (K::IncDec, 2),
        b'=' => (K::Assign, 2),
        _ => (K::Op, 1),
    }
}

fn colon_or_dot(c: u8, c1: u8, c2: u8) -> (K, usize) {
    match (c, c1, c2) {
        (b'.', b'.', b'.') => (K::Op, 3),
        (b'.', _, _) => (K::Dot, 1),
        (_, b'=', _) => (K::Assign, 2),
        _ => (K::Colon, 1),
    }
}

/// `=`, `!`, `*`, `/`, `%` and `^`: alone, or followed by `=`.
fn equals_family(c: u8, c1: u8) -> (K, usize) {
    match (c, c1) {
        (b'=' | b'!', b'=') => (K::Op, 2),
        (b'=', _) => (K::Assign, 1),
        (b'!', _) => (K::Op, 1),
        (_, b'=') => (K::Assign, 2),
        (b'*', _) => (K::Star, 1),
        _ => (K::Op, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn kinds(src: &str) -> Vec<K> {
        lex(src.as_bytes())
            .unwrap()
            .iter()
            .map(|t| t.kind)
            .collect()
    }

    #[rstest]
    #[case("a && b || c", vec![K::Ident, K::AndAnd, K::Ident, K::OrOr, K::Ident])]
    #[case("x := <-ch", vec![K::Ident, K::Assign, K::Arrow, K::Ident])]
    #[case("x &^= 3", vec![K::Ident, K::Assign, K::Lit])]
    #[case("1e+5 + 0x1p-2", vec![K::Lit, K::Op, K::Lit])]
    #[case("0xE+1", vec![K::Lit, K::Op, K::Lit])]
    #[case("f(a...)", vec![K::Ident, K::LParen, K::Ident, K::Op, K::RParen])]
    #[case("s := \"if for { \\\" \" + `a\nb`", vec![K::Ident, K::Assign, K::Lit, K::Op, K::Lit])]
    #[case("r := '\\''", vec![K::Ident, K::Assign, K::Lit])]
    #[case("i++ // a && b\nj--", vec![K::Ident, K::IncDec, K::Ident, K::IncDec])]
    #[case("x /* a\n&& b */ y", vec![K::Ident, K::Ident])]
    #[case("héllo := 1", vec![K::Ident, K::Assign, K::Lit])]
    fn tokens(#[case] src: &str, #[case] want: Vec<K>) {
        assert_eq!(kinds(src), want);
    }

    #[test]
    fn lines_and_breaks_are_tracked() {
        let toks = lex(b"a\n/* x\ny */ b\n\nc").unwrap();
        let seen: Vec<(u32, bool)> = toks.iter().map(|t| (t.line, t.nl_before)).collect();
        assert_eq!(seen, vec![(1, false), (3, true), (5, true)]);
    }

    #[rstest]
    #[case("s := \"open")]
    #[case("s := \"a\nb\"")]
    #[case("r := 'a")]
    #[case("/* open")]
    #[case("s := `open")]
    fn input_it_cannot_be_sure_of_is_refused(#[case] src: &str) {
        assert!(lex(src.as_bytes()).is_none());
    }

    #[test]
    fn comments_between_tokens_are_noted() {
        let toks = lex(b"a // x\nb /* y */ c d").unwrap();
        let seen: Vec<bool> = toks.iter().map(|t| t.comment_before).collect();
        assert_eq!(seen, vec![false, true, true, false]);
    }

    #[test]
    fn a_byte_order_mark_is_skipped() {
        assert_eq!(kinds("\u{feff}package p").len(), 2);
    }
}
