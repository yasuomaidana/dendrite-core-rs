//! What the scan of a function body keeps while it reads: regions, headers being read, boolean runs.

/// What a brace-delimited region of a body is, for the nesting level of what is inside it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Part {
    Func,
    Closure,
    /// An `if`; it stays open across its `else` branches.
    If,
    ElseIf,
    ElseBlock,
    /// `for`, `switch` or `select`.
    Loop,
}

pub(super) struct Frame {
    pub(super) part: Part,
    /// The brace depth inside the region.
    pub(super) open_depth: u32,
    /// The nesting level to return to when the region closes.
    pub(super) level_before: u32,
    /// The region closed and an `else` follows: it ends with the last branch of the chain.
    pub(super) waiting: bool,
}

/// A structure whose header is being read; its body is the first brace outside parentheses and literals.
pub(super) struct Pending {
    pub(super) part: Part,
    pub(super) level_before: u32,
    pub(super) depth: u32,
    pub(super) paren: u32,
    pub(super) literal: u32,
}

/// The `&&` and `||` of one expression: how many runs it has paid for.
#[derive(Clone, Copy, Default)]
pub(super) struct Chain {
    pub(super) has_or: bool,
    pub(super) and_open: bool,
}

pub(super) struct Done {
    pub(super) cyclomatic: u32,
    pub(super) cognitive: u32,
    pub(super) end_line: u32,
    pub(super) next: usize,
}

/// Where the scan goes after a token: on to another, or out with the finished function.
pub(super) enum Step {
    Next(usize),
    Done(Done),
}
