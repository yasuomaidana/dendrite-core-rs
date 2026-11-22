//! Hand-assembled functions with the complexity each must have. Every snippet starts at RVA 0x1000.

use super::*;
use rstest::rstest;

const BASE: u32 = 0x1000;

/// The bytes of `hex`: pairs of hex digits; spaces are ignored.
pub(crate) fn assemble(hex: &str) -> Vec<u8> {
    let digits: String = hex.split_whitespace().collect();
    (0..digits.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap())
        .collect()
}

/// `xor eax,eax; ret`
pub(crate) const LEAF: &str = "31C0 C3";

/// `if (ecx == 0) eax = 2 else eax = 1; ret`: test, jz +0xB, mov, jmp +0x10, mov, ret.
pub(crate) const IF_ELSE: &str = "85C9 7407 B801000000 EB05 B802000000 C3";

/// A loop whose body has an `if`, so two decisions: xor, test (loop head), jz +0x10, test, jz +0xC, inc, dec,
/// jmp +0x2, ret.
pub(crate) const LOOP_IF: &str = "31C0 85C9 740A A801 7402 FFC0 FFC9 EBF2 C3";

/// `call +0xA; ret; int3 x4; xor eax,eax; ret`
const CALL: &str = "E805000000 C3 CCCCCCCC 31C0 C3";

fn run(hex: &str, bitness: u32, limit: u32, budget: usize) -> Walk {
    let bytes = assemble(hex);
    let code = Code {
        rva: BASE,
        bytes: &bytes,
        bitness,
    };
    walk(&code, BASE, limit, budget)
}

/// The walk of a whole snippet, which ends where the snippet does.
fn whole(hex: &str) -> Walk {
    let len = assemble(hex).len() as u32;
    run(hex, 64, BASE + len, MAX_FUNCTION_INSTRUCTIONS)
}

#[rstest]
#[case::leaf(LEAF, 2, 1, 1)]
#[case::if_else(IF_ELSE, 6, 4, 2)]
#[case::loop_if(LOOP_IF, 9, 6, 3)]
fn counts(
    #[case] hex: &str,
    #[case] instructions: usize,
    #[case] blocks: usize,
    #[case] cyclomatic: u32,
) {
    let found = whole(hex);
    let end = BASE + assemble(hex).len() as u32;
    assert_eq!(
        (
            found.instructions,
            found.blocks,
            found.cyclomatic,
            found.end
        ),
        (instructions, blocks, cyclomatic, end)
    );
    assert!(!found.truncated);
}

#[test]
fn the_same_code_has_the_same_complexity_in_32_bits() {
    let found = run(IF_ELSE, 32, BASE + 17, 100);
    assert_eq!((found.cyclomatic, found.instructions), (2, 6));
}

#[test]
fn a_direct_call_names_the_start_of_another_function() {
    let found = run(CALL, 64, BASE + 6, 100);
    assert_eq!(found.calls, BTreeSet::from([BASE + 0xA]));
    assert_eq!((found.end, found.cyclomatic), (BASE + 6, 1));
}

#[test]
fn a_jump_out_of_the_function_is_a_tail_call() {
    // jmp +0x20
    let found = run("E91B000000", 64, BASE + 5, 100);
    assert_eq!(found.tails, BTreeSet::from([BASE + 0x20]));
    assert_eq!(found.cyclomatic, 1);
}

#[rstest]
#[case::invalid_opcode("FFFF", 0)]
#[case::past_the_section("", 0)]
#[case::indirect_jump("FFE0", 1)]
#[case::ud2("0F0B", 1)]
#[case::falls_off_the_limit("90", 1)]
fn code_that_cannot_be_followed_ends_the_walk(#[case] hex: &str, #[case] instructions: usize) {
    let found = whole(hex);
    assert_eq!((found.instructions, found.cyclomatic), (instructions, 1));
}

#[test]
fn a_start_before_the_section_decodes_nothing() {
    let bytes = assemble(LEAF);
    let code = Code {
        rva: BASE,
        bytes: &bytes,
        bitness: 64,
    };
    assert_eq!(walk(&code, BASE - 1, BASE + 2, 10).instructions, 0);
}

#[test]
fn the_instruction_budget_cuts_the_walk_and_says_so() {
    let found = run(&"90".repeat(50), 64, BASE + 50, 3);
    assert_eq!((found.instructions, found.truncated), (3, true));
}

#[test]
fn a_jump_to_itself_ends() {
    let found = run("EBFE", 64, BASE + 2, 100);
    assert_eq!((found.instructions, found.cyclomatic), (1, 1));
}
