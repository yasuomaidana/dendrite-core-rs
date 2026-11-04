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
use crate::ast::go::skip::{kind, skip_group, skip_signature, starts_statement};
use crate::ast::go::state::{Chain, Done, Frame, Part, Pending, Step};
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
        self.chains
            .last_mut()
            .into_iter()
            .for_each(|chain| *chain = Chain::default());
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
        // The header being read, when this brace is directly in it: does the brace open a literal?
        let header = self
            .pending
            .last()
            .filter(|p| p.paren == 0 && p.depth + p.literal == self.depth)
            .map(|p| p.literal > 0 || self.is_literal_brace(k));
        match header {
            Some(true) => self.open_literal_brace(),
            Some(false) => self.open_body_brace(),
            None => self.open_plain_brace(k),
        }
    }

    /// The brace that opens the body of the structure whose header was being read.
    fn open_body_brace(&mut self) {
        let p = self.pending.pop().expect("a header is pending");
        self.depth += 1;
        self.frames.push(Frame {
            part: p.part,
            open_depth: self.depth,
            level_before: p.level_before,
            waiting: false,
        });
        self.chains.push(Chain::default());
    }

    /// Any other brace: the block after an `else`, or a block or literal that nothing needs to track.
    fn open_plain_brace(&mut self, k: usize) {
        let in_header = self.pending.last().is_some_and(|p| p.depth == self.depth);
        self.depth += 1;
        if self.t[k - 1].kind == K::Else && !in_header {
            self.frames.push(Frame {
                part: Part::ElseBlock,
                open_depth: self.depth,
                level_before: self.level,
                waiting: false,
            });
        }
        self.chains.push(Chain::default());
    }

    /// Opens a brace that belongs to a composite literal in a structure's header, not to its body.
    fn open_literal_brace(&mut self) {
        self.pending
            .last_mut()
            .expect("a header is pending")
            .literal += 1;
        self.depth += 1;
        self.chains.push(Chain::default());
    }

    /// Closes the region the `}` at `k` ends. Ends the scan when it was the function's own brace.
    fn close_brace(&mut self, k: usize) -> Option<Step> {
        let closing = self.depth;
        self.depth = closing.checked_sub(1)?;
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
        while let Some(frame) = self.frames.pop_if(|f| f.waiting) {
            self.level = frame.level_before;
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
