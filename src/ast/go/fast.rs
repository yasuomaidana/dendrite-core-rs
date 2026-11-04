//! Complexity of Go source without a syntax tree.
//!
//! `ast.complexity` for Go used to cost a tree-sitter parse (a general GLR parser that builds a full syntax tree
//! with error recovery) plus a walk, which made it several times slower than `go/parser` (DCORE-13). The metrics
//! need far less than a tree: the keywords that branch, the braces that say where a construct ends, and the
//! operators of each boolean expression. This scan reads the tokens once and tracks just that.
//!
//! It must give the answer the tree-sitter path gives, so it refuses (`None`) whenever it meets something it
//! cannot decide from tokens alone, and the caller falls back to the parser. The differential test in
//! `fast_tests.rs` compares the two over any directory of Go source.

use crate::ast::go::lex::{lex, Token, K};
use crate::complexity::FunctionMetrics;

/// The metrics of every function declaration of `source`, or `None` when the scan is not certain of them.
pub fn complexity(source: &str) -> Option<Vec<FunctionMetrics>> {
    let src = source.as_bytes();
    let toks = lex(src)?;
    let mut out = Vec::new();
    let mut i = 0;
    let mut depth = 0i32;
    while i < toks.len() {
        match toks[i].kind {
            K::Func if depth == 0 && starts_statement(&toks, i) => {
                i = declaration(src, &toks, i, &mut out)?;
                continue;
            }
            K::LBrace | K::LParen | K::LBracket => depth += 1,
            K::RBrace | K::RParen | K::RBracket => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
        i += 1;
    }
    (depth == 0).then_some(out)
}

fn kind(t: &[Token], i: usize) -> Option<K> {
    t.get(i).map(|tok| tok.kind)
}

/// Whether token `i` begins a statement or declaration: after a semicolon, or after a line break that ends one.
fn starts_statement(t: &[Token], i: usize) -> bool {
    i == 0 || t[i - 1].kind == K::Semi || (t[i].nl_before && t[i - 1].kind.ends_statement())
}

/// The index after the group that opens at `at` with `open` and closes with the matching `close`.
fn skip_group(t: &[Token], at: usize, open: K, close: K) -> Option<usize> {
    let mut depth = 0i32;
    for (j, tok) in t.iter().enumerate().skip(at) {
        if tok.kind == open {
            depth += 1;
        } else if tok.kind == close {
            depth -= 1;
            if depth == 0 {
                return Some(j + 1);
            }
        }
    }
    None
}

fn can_start_type(k: K) -> bool {
    matches!(
        k,
        K::Star
            | K::Arrow
            | K::LBracket
            | K::Map
            | K::Chan
            | K::Ident
            | K::LParen
            | K::Func
            | K::Struct
            | K::Interface
    )
}

/// The index after the type that starts at `j`.
fn skip_type(t: &[Token], mut j: usize) -> Option<usize> {
    loop {
        match kind(t, j)? {
            K::Star | K::Arrow | K::Chan => j += 1,
            K::Ident => return skip_named_type(t, j),
            K::Func => return skip_signature(t, j + 1),
            _ => return skip_enclosed_type(t, j),
        }
    }
}

/// A type name: `T`, `pkg.T`, with type arguments `T[int]`.
fn skip_named_type(t: &[Token], mut j: usize) -> Option<usize> {
    j += 1;
    if kind(t, j) == Some(K::Dot) && kind(t, j + 1) == Some(K::Ident) {
        j += 2;
    }
    if t.get(j)
        .is_some_and(|x| x.kind == K::LBracket && !x.nl_before)
    {
        j = skip_group(t, j, K::LBracket, K::RBracket)?;
    }
    Some(j)
}

/// A type that starts with a delimiter or keyword: `[]T`, `map[K]V`, `(T)`, `struct{...}`, `interface{...}`.
fn skip_enclosed_type(t: &[Token], j: usize) -> Option<usize> {
    match kind(t, j)? {
        K::LBracket => skip_type(t, skip_group(t, j, K::LBracket, K::RBracket)?),
        K::Map if kind(t, j + 1) == Some(K::LBracket) => {
            skip_type(t, skip_group(t, j + 1, K::LBracket, K::RBracket)?)
        }
        K::LParen => skip_group(t, j, K::LParen, K::RParen),
        K::Struct | K::Interface if kind(t, j + 1) == Some(K::LBrace) => {
            skip_group(t, j + 1, K::LBrace, K::RBrace)
        }
        _ => None,
    }
}

/// The index after a function signature whose parameter list starts at `j`: the parameters, then a result
/// on the same line.
fn skip_signature(t: &[Token], j: usize) -> Option<usize> {
    if kind(t, j)? != K::LParen {
        return None;
    }
    let j = skip_group(t, j, K::LParen, K::RParen)?;
    match t.get(j) {
        Some(next) if next.kind != K::LBrace && !next.nl_before && can_start_type(next.kind) => {
            skip_type(t, j)
        }
        _ => Some(j),
    }
}

/// A top-level `func`: reads the signature, scans the body and records the function.
fn declaration(
    src: &[u8],
    t: &[Token],
    at: usize,
    out: &mut Vec<FunctionMetrics>,
) -> Option<usize> {
    let mut j = at + 1;
    if kind(t, j) == Some(K::LParen) {
        j = skip_group(t, j, K::LParen, K::RParen)?;
    }
    let name = t.get(j).filter(|n| n.kind == K::Ident)?;
    j += 1;
    if kind(t, j) == Some(K::LBracket) {
        j = skip_group(t, j, K::LBracket, K::RBracket)?;
    }
    j = skip_signature(t, j)?;
    let name_bytes = &src[name.start as usize..name.end as usize];
    let mut function = FunctionMetrics {
        symbol: String::from_utf8_lossy(name_bytes).into_owned(),
        start_line: t[at].line as usize,
        end_line: t[j - 1].line as usize,
        cyclomatic: 1,
        cognitive: 0,
    };
    match t.get(j) {
        Some(open) if open.kind == K::LBrace && !open.nl_before => {
            let done = Scan::new(src, t, name_bytes).run(j)?;
            function.end_line = done.end_line as usize;
            function.cyclomatic = done.cyclomatic;
            function.cognitive = done.cognitive;
            out.push(function);
            Some(done.next)
        }
        _ => {
            out.push(function);
            Some(j)
        }
    }
}

/// What a brace-delimited region of a body is, for the nesting level of what is inside it.
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Func,
    Closure,
    /// An `if`; it stays open across its `else` branches.
    If,
    ElseIf,
    ElseBlock,
    /// `for`, `switch` or `select`.
    Loop,
}

struct Frame {
    part: Part,
    /// The brace depth inside the region.
    open_depth: u32,
    /// The nesting level to return to when the region closes.
    level_before: u32,
    /// The region closed and an `else` follows: it ends with the last branch of the chain.
    waiting: bool,
}

/// A structure whose header is being read; its body is the first brace outside parentheses and literals.
struct Pending {
    part: Part,
    level_before: u32,
    depth: u32,
    paren: u32,
    literal: u32,
}

/// The `&&` and `||` of one expression: how many runs it has paid for.
#[derive(Clone, Copy, Default)]
struct Chain {
    has_or: bool,
    and_open: bool,
}

struct Done {
    cyclomatic: u32,
    cognitive: u32,
    end_line: u32,
    next: usize,
}

/// Where the scan goes after a token: on to another, or out with the finished function.
enum Step {
    Next(usize),
    Done(Done),
}

struct Scan<'a> {
    src: &'a [u8],
    t: &'a [Token],
    name: &'a [u8],
    cyclomatic: u32,
    cognitive: u32,
    level: u32,
    depth: u32,
    frames: Vec<Frame>,
    pending: Vec<Pending>,
    chains: Vec<Chain>,
}

impl<'a> Scan<'a> {
    fn new(src: &'a [u8], t: &'a [Token], name: &'a [u8]) -> Self {
        Scan {
            src,
            t,
            name,
            cyclomatic: 1,
            cognitive: 0,
            level: 0,
            depth: 0,
            frames: Vec::new(),
            pending: Vec::new(),
            chains: Vec::new(),
        }
    }

    /// Scans the body whose opening brace is token `open`.
    fn run(mut self, open: usize) -> Option<Done> {
        self.depth = 1;
        self.frames.push(Frame {
            part: Part::Func,
            open_depth: 1,
            level_before: 0,
            waiting: false,
        });
        self.chains.push(Chain::default());
        let mut k = open + 1;
        while let Some(&tok) = self.t.get(k) {
            if tok.nl_before && self.t[k - 1].kind.ends_statement() {
                self.end_expression();
            }
            match self.step(k, tok)? {
                Step::Next(next) => k = next,
                Step::Done(done) => return Some(done),
            }
        }
        None
    }

    /// Handles token `k`.
    fn step(&mut self, k: usize, tok: Token) -> Option<Step> {
        match tok.kind {
            K::LBrace => {
                self.open_brace(k);
                Some(Step::Next(k + 1))
            }
            K::RBrace => self.close_brace(k),
            K::If | K::For | K::Switch | K::Select => {
                self.structure_keyword(k, tok);
                Some(Step::Next(k + 1))
            }
            K::Else | K::Goto | K::Case | K::Break | K::Continue => {
                self.flow_keyword(k, tok.kind);
                Some(Step::Next(k + 1))
            }
            K::Func => self.func_literal(k).map(Step::Next),
            K::Struct | K::Interface => self.type_body(k).map(Step::Next),
            K::Ident => self.identifier(k, tok).map(|()| Step::Next(k + 1)),
            _ => self.expression_token(tok.kind).map(|()| Step::Next(k + 1)),
        }
    }

    /// Brackets, boolean operators and everything that ends an expression.
    fn expression_token(&mut self, kind: K) -> Option<()> {
        match kind {
            K::LParen | K::LBracket => {
                self.chains.push(Chain::default());
                self.move_paren(true);
            }
            K::RParen | K::RBracket => {
                self.chains.pop();
                self.move_paren(false);
                if self.chains.is_empty() {
                    return None;
                }
            }
            K::AndAnd | K::OrOr => self.boolean_operator(kind)?,
            K::Semi | K::Comma | K::Colon | K::Assign | K::IncDec | K::Keyword | K::EndKeyword => {
                self.end_expression()
            }
            _ => {}
        }
        Some(())
    }

    /// `&&` and `||`: each is a decision, and each run of one operator in an expression is paid once.
    fn boolean_operator(&mut self, kind: K) -> Option<()> {
        self.cyclomatic += 1;
        let chain = self.chains.last_mut()?;
        let paid = if kind == K::AndAnd {
            std::mem::replace(&mut chain.and_open, true)
        } else {
            chain.and_open = false;
            std::mem::replace(&mut chain.has_or, true)
        };
        if !paid {
            self.cognitive += 1;
        }
        Some(())
    }

    fn end_expression(&mut self) {
        if let Some(chain) = self.chains.last_mut() {
            *chain = Chain::default();
        }
    }

    /// `if`, `for`, `switch` and `select`: a structure that nests what is inside it.
    fn structure_keyword(&mut self, k: usize, tok: Token) {
        self.end_expression();
        if matches!(tok.kind, K::If | K::For) {
            self.cyclomatic += 1;
        }
        // A comment between `else` and `if` makes the parser's previous sibling the comment, not the `else`:
        // the `if` is then scored as a structure of its own.
        if tok.kind == K::If && self.t[k - 1].kind == K::Else && !tok.comment_before {
            self.begin(Part::ElseIf, false);
        } else {
            self.begin(
                if tok.kind == K::If {
                    Part::If
                } else {
                    Part::Loop
                },
                true,
            );
        }
    }

    /// `else`, `goto`, `case`, `break` and `continue`.
    fn flow_keyword(&mut self, k: usize, kind: K) {
        self.end_expression();
        match kind {
            K::Else | K::Goto => self.cognitive += 1,
            K::Case => self.cyclomatic += 1,
            // A label is on the same line; on the next line it would label another statement.
            _ => {
                self.cognitive += u32::from(
                    self.t
                        .get(k + 1)
                        .is_some_and(|n| n.kind == K::Ident && !n.nl_before),
                )
            }
        }
    }

    /// An identifier: a call of the function itself is recursion.
    fn identifier(&mut self, k: usize, tok: Token) -> Option<()> {
        if self.src[tok.start as usize..tok.end as usize] != *self.name {
            return Some(());
        }
        // `(` or `[` on the next line starts another statement: Go ends this one at the line break
        match self.t.get(k + 1).filter(|n| !n.nl_before).map(|n| n.kind) {
            Some(K::LParen) => self.cognitive += 1,
            // `f[T](x)` is an instantiated call or an index, which only the grammar can tell
            Some(K::LBracket) => {
                let end = skip_group(self.t, k + 1, K::LBracket, K::RBracket)?;
                if kind(self.t, end) == Some(K::LParen) {
                    return None;
                }
            }
            _ => {}
        }
        Some(())
    }

    /// `struct` or `interface`: a type body is stepped over. Returns the index to continue from.
    ///
    /// In a structure's header, a brace right after the type body can only open a composite literal
    /// (`range []struct{ ... }{ ... } {`, the shape of table-driven tests), never the body.
    fn type_body(&mut self, k: usize) -> Option<usize> {
        if kind(self.t, k + 1) != Some(K::LBrace) {
            return Some(k + 1);
        }
        let in_header = self.in_header();
        let after = skip_group(self.t, k + 1, K::LBrace, K::RBrace)?;
        if in_header && kind(self.t, after) == Some(K::LBrace) {
            self.open_literal_brace();
            return Some(after + 1);
        }
        Some(after)
    }

    /// Starts reading a structure's header. `nests` is false for an `else if`, which stays at its chain's level.
    fn begin(&mut self, part: Part, nests: bool) {
        let level_before = self.level;
        if nests {
            self.cognitive += 1 + self.level;
            self.level += 1;
        }
        self.pending.push(Pending {
            part,
            level_before,
            depth: self.depth,
            paren: 0,
            literal: 0,
        });
    }

    /// Whether the scan is directly in a structure's header, where a brace could be its body.
    fn in_header(&self) -> bool {
        self.pending
            .last()
            .is_some_and(|p| p.paren == 0 && p.literal == 0 && p.depth == self.depth)
    }

    fn move_paren(&mut self, open: bool) {
        if let Some(p) = self.pending.last_mut() {
            if p.depth == self.depth {
                p.paren = if open {
                    p.paren + 1
                } else {
                    p.paren.saturating_sub(1)
                };
            }
        }
    }

    /// Whether the `{` at `k` opens a composite literal of a slice, array or map type (`[]T{`), which Go allows
    /// in a header, rather than the body of the structure.
    fn is_literal_brace(&self, k: usize) -> bool {
        let t = self.t;
        let mut j = k - 1;
        if t[j].kind != K::Ident {
            return false;
        }
        if j >= 2 && t[j - 1].kind == K::Dot && t[j - 2].kind == K::Ident {
            j -= 2;
        }
        while j >= 1 && matches!(t[j - 1].kind, K::Star | K::Chan | K::Arrow) {
            j -= 1;
        }
        j >= 1 && t[j - 1].kind == K::RBracket
    }

    fn open_brace(&mut self, k: usize) {
        if self
            .pending
            .last()
            .is_some_and(|p| p.paren == 0 && p.depth + p.literal == self.depth)
        {
            let literal =
                self.pending.last().is_some_and(|p| p.literal > 0) || self.is_literal_brace(k);
            if literal {
                self.open_literal_brace();
                return;
            }
            if let Some(p) = self.pending.pop() {
                self.depth += 1;
                self.frames.push(Frame {
                    part: p.part,
                    open_depth: self.depth,
                    level_before: p.level_before,
                    waiting: false,
                });
                self.chains.push(Chain::default());
                return;
            }
        }
        if self.t[k - 1].kind == K::Else
            && !self.pending.last().is_some_and(|p| p.depth == self.depth)
        {
            self.depth += 1;
            self.frames.push(Frame {
                part: Part::ElseBlock,
                open_depth: self.depth,
                level_before: self.level,
                waiting: false,
            });
        } else {
            self.depth += 1;
        }
        self.chains.push(Chain::default());
    }

    /// Opens a brace that belongs to a composite literal in a structure's header, not to its body.
    fn open_literal_brace(&mut self) {
        if let Some(p) = self.pending.last_mut() {
            p.literal += 1;
        }
        self.depth += 1;
        self.chains.push(Chain::default());
    }

    /// Closes the region the `}` at `k` ends. Ends the scan when it was the function's own brace.
    fn close_brace(&mut self, k: usize) -> Option<Step> {
        let closing = self.depth;
        if closing == 0 {
            return None;
        }
        self.depth -= 1;
        self.chains.pop();
        let owns = self
            .frames
            .last()
            .is_some_and(|f| !f.waiting && f.open_depth == closing);
        if self.close_header_literal(closing) || !owns {
            return Some(Step::Next(k + 1));
        }
        let frame = self.frames.pop()?;
        if frame.part == Part::Func {
            let (cyclomatic, cognitive) = (self.cyclomatic, self.cognitive);
            return Some(Step::Done(Done {
                cyclomatic,
                cognitive,
                end_line: self.t[k].line,
                next: k + 1,
            }));
        }
        self.finish_region(frame, k);
        Some(Step::Next(k + 1))
    }

    /// Whether the brace that closes at depth `closing` ended a composite literal of a structure's header.
    fn close_header_literal(&mut self, closing: u32) -> bool {
        match self.pending.last_mut() {
            Some(p) if p.literal > 0 && p.depth + p.literal == closing => {
                p.literal -= 1;
                true
            }
            _ => false,
        }
    }

    /// A region other than the function closed: nesting returns to what it was, unless an `else` follows.
    fn finish_region(&mut self, frame: Frame, k: usize) {
        let else_follows = kind(self.t, k + 1) == Some(K::Else);
        if matches!(frame.part, Part::If | Part::ElseIf) && else_follows {
            self.frames.push(Frame {
                waiting: true,
                ..frame
            });
            return;
        }
        self.level = frame.level_before;
        if matches!(frame.part, Part::If | Part::ElseIf | Part::ElseBlock) {
            self.end_chain();
        }
    }

    /// The last branch of an `if` chain closed: the `if` frames waiting on it close too.
    fn end_chain(&mut self) {
        while self.frames.last().is_some_and(|f| f.waiting) {
            if let Some(f) = self.frames.pop() {
                self.level = f.level_before;
            }
        }
    }

    /// A `func` inside a body: a closure (one level deeper) or a function type, which is stepped over.
    /// Returns the index to continue from.
    fn func_literal(&mut self, k: usize) -> Option<usize> {
        let t = self.t;
        if kind(t, k + 1) != Some(K::LParen) {
            return Some(k + 1);
        }
        let j = skip_signature(t, k + 1)?;
        // After `]` it is the element type of a literal (`[]func(){...}`), not a closure.
        let after_bracket = k > 0 && t[k - 1].kind == K::RBracket;
        match t.get(j) {
            Some(open) if open.kind == K::LBrace && !open.nl_before && !after_bracket => {
                self.depth += 1;
                self.frames.push(Frame {
                    part: Part::Closure,
                    open_depth: self.depth,
                    level_before: self.level,
                    waiting: false,
                });
                self.level += 1;
                self.chains.push(Chain::default());
                Some(j + 1)
            }
            Some(open) if open.kind == K::LBrace && after_bracket && self.in_header() => {
                // `[]func() T{ ... }` in a header: the brace opens the literal, not the body
                self.open_literal_brace();
                Some(j + 1)
            }
            _ => Some(j),
        }
    }
}

#[cfg(test)]
#[path = "fast_tests.rs"]
mod tests;
