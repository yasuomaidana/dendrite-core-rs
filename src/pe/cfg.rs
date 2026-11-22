//! The control-flow walk of one function: which instructions it reaches, where it ends, what it calls and its
//! cyclomatic complexity.
//!
//! Complexity is `E - N + 2` over the graph of instructions plus one virtual exit node. Splitting a block into
//! two instructions adds a node and an edge, so the value equals the one over basic blocks, and no block ever has
//! to be split when a jump lands in the middle of one. A jump through a register (a jump table) is an exit: its
//! targets are not resolved, so a function with a `switch` is undercounted.

use std::collections::BTreeSet;

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction};

/// The most instructions walked in one function; past it the walk stops and says `truncated`.
pub const MAX_FUNCTION_INSTRUCTIONS: usize = 20_000;

/// The machine code of one section and where it sits in the image.
pub struct Code<'a> {
    /// RVA of the first byte of `bytes`.
    pub rva: u32,
    pub bytes: &'a [u8],
    /// 32 or 64.
    pub bitness: u32,
}

impl Code<'_> {
    /// One past the RVA of the last byte.
    pub fn end(&self) -> u32 {
        self.rva.saturating_add(self.bytes.len() as u32)
    }

    pub fn contains(&self, rva: u32) -> bool {
        (self.rva..self.end()).contains(&rva)
    }

    /// The instruction at `rva`, or `None` when it lies outside the section or is not a valid opcode.
    fn decode(&self, rva: u32) -> Option<Instruction> {
        let rest = self.bytes.get(rva.checked_sub(self.rva)? as usize..)?;
        let mut decoder = Decoder::with_ip(self.bitness, rest, rva.into(), DecoderOptions::NONE);
        let instruction = decoder.decode();
        (!instruction.is_invalid()).then_some(instruction)
    }
}

/// What the walk of one function found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Walk {
    /// One past the last byte of the last instruction reached.
    pub end: u32,
    pub instructions: usize,
    pub blocks: usize,
    pub cyclomatic: u32,
    /// Targets of direct calls: the start of other functions.
    pub calls: BTreeSet<u32>,
    /// Targets of direct jumps that leave the function: tail calls.
    pub tails: BTreeSet<u32>,
    pub truncated: bool,
}

/// Where control goes after an instruction.
enum Flow {
    Next,
    Branch(u32),
    Jump(u32),
    Exit,
}

fn target(instruction: &Instruction) -> u32 {
    u32::try_from(instruction.near_branch_target()).unwrap_or(u32::MAX)
}

fn classify(instruction: &Instruction) -> Flow {
    match instruction.flow_control() {
        FlowControl::ConditionalBranch => Flow::Branch(target(instruction)),
        FlowControl::UnconditionalBranch => Flow::Jump(target(instruction)),
        FlowControl::Next
        | FlowControl::Call
        | FlowControl::IndirectCall
        | FlowControl::XbeginXabortXend => Flow::Next,
        _ => Flow::Exit,
    }
}

struct Walker<'a> {
    code: &'a Code<'a>,
    start: u32,
    limit: u32,
    seen: BTreeSet<u32>,
    leaders: BTreeSet<u32>,
    pending: Vec<u32>,
    nodes: i64,
    edges: i64,
    walk: Walk,
}

impl Walker<'_> {
    /// An edge to `to`: followed when it stays inside the function, an edge to the virtual exit otherwise.
    fn edge(&mut self, to: u32) {
        self.edges += 1;
        if (self.start..self.limit).contains(&to) && self.seen.insert(to) {
            self.pending.push(to);
        }
    }

    /// An edge that starts a basic block: a branch target or the instruction after a conditional branch.
    fn leader(&mut self, to: u32) {
        if (self.start..self.limit).contains(&to) {
            self.leaders.insert(to);
        }
        self.edge(to);
    }

    fn step(&mut self, at: u32) {
        self.nodes += 1;
        let Some(instruction) = self.code.decode(at) else {
            self.edges += 1;
            return;
        };
        let next = at + instruction.len() as u32;
        self.walk.instructions += 1;
        self.walk.end = self.walk.end.max(next);
        match classify(&instruction) {
            Flow::Next => {
                if instruction.is_call_near() {
                    self.walk.calls.insert(target(&instruction));
                }
                self.edge(next);
            }
            Flow::Branch(to) => {
                self.leader(to);
                self.leader(next);
            }
            Flow::Jump(to) => {
                if !(self.start..self.limit).contains(&to) {
                    self.walk.tails.insert(to);
                }
                self.leader(to);
            }
            Flow::Exit => self.edges += 1,
        }
    }
}

/// Walks the function that starts at `start` and may not extend to `limit` or beyond, decoding at most `budget`
/// instructions.
pub fn walk(code: &Code, start: u32, limit: u32, budget: usize) -> Walk {
    let mut walker = Walker {
        code,
        start,
        limit,
        seen: BTreeSet::from([start]),
        leaders: BTreeSet::from([start]),
        pending: vec![start],
        nodes: 0,
        edges: 0,
        walk: Walk {
            end: start,
            ..Walk::default()
        },
    };
    while let Some(at) = walker.pending.pop() {
        if walker.walk.instructions >= budget {
            walker.walk.truncated = true;
            break;
        }
        walker.step(at);
    }
    let cyclomatic = (walker.edges - walker.nodes + 1).max(1);
    Walk {
        blocks: walker.leaders.len(),
        cyclomatic: u32::try_from(cyclomatic).unwrap_or(u32::MAX),
        ..walker.walk
    }
}

#[cfg(test)]
#[path = "cfg_tests.rs"]
pub(crate) mod tests;
