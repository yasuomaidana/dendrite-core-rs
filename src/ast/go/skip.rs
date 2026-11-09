//! Stepping over the parts of a Go declaration the metrics do not read: groups of brackets, types, signatures.

use crate::ast::go::lex::{Token, K};

pub(super) fn kind(t: &[Token], i: usize) -> Option<K> {
    t.get(i).map(|tok| tok.kind)
}

/// Whether token `i` begins a statement or declaration: after a semicolon, or after a line break that ends one.
pub(super) fn starts_statement(t: &[Token], i: usize) -> bool {
    i == 0 || t[i - 1].kind == K::Semi || (t[i].nl_before && t[i - 1].kind.ends_statement())
}

/// The index after the group that opens at `at` with `open` and closes with the matching `close`.
pub(super) fn skip_group(t: &[Token], at: usize, open: K, close: K) -> Option<usize> {
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

/// The kinds of token a type can start with.
const TYPE_START: [K; 10] = [
    K::Star,
    K::Arrow,
    K::LBracket,
    K::Map,
    K::Chan,
    K::Ident,
    K::LParen,
    K::Func,
    K::Struct,
    K::Interface,
];

fn can_start_type(k: K) -> bool {
    TYPE_START.contains(&k)
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
pub(super) fn skip_signature(t: &[Token], j: usize) -> Option<usize> {
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
