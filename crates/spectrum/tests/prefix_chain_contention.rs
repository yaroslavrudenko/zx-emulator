//! Gate: a run of `DD`/`FD` prefix bytes is **one instruction of unbounded length**, and
//! every byte of it is its own M1 cycle, its own `R` increment and its own contention point.
//!
//! # Why this exists
//!
//! `contention_magnitude.rs` listed this in its *what is not graded here*: *"one `DD` is
//! exercised; a run of them is one instruction of unbounded length, and nothing prices a
//! contended one."*
//!
//! It is also the shape that has already produced a hard process abort in this repository.
//! `docs/STATUS.md` records it under *comments rot at milestone boundaries*: the CPU's
//! T-state accumulator was a `u8`, justified by a comment arguing that the longest Z80
//! instruction is 23 T-states. That was true when written and falsified the moment M2's
//! `dispatch` made a run of prefixes into *one* instruction whose length **guest memory
//! decides**. With `overflow-checks = true` in every profile and `panic = "abort"`, the
//! comment's own safety argument became the defect, and the "loud panic rather than a silent
//! wrap" it promised turned out to be reporting a **legal instruction stream**.
//!
//! So the long chains here are not padding. A 64-byte chain costs 260 nominal T-states and a
//! 200-byte chain costs 804, and both numbers are things a byte cannot hold.
//!
//! # What makes this different from a run of `NOP`s
//!
//! Nothing, to the bus — and that is the assertion, not an admission.
//! [`a_prefix_chain_costs_what_the_same_bytes_cost_as_separate_nops`] states it directly:
//! the two emit the same stream of four-T-state M1 cycles at the same consecutive addresses,
//! so they must cost the same from the same starting position. What differs is entirely on
//! the CPU's side of the trait — the chain is **one** `step()` where the `NOP`s are many —
//! which is exactly why it can overflow an accumulator that the `NOP`s cannot.
//!
//! That equivalence is also a second, independent construction for every figure in
//! [`CHAINS`]: those come from walking the published delay pattern position by position over
//! a recorded cycle list, and the `NOP` comparison reaches them again without using the
//! pattern at all.
//!
//! # What is not graded here
//!
//! - **Whether the published pattern is right.** No oracle; see `contention_magnitude.rs`.
//! - **The phase.** Every position is relative to
//!   [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE], so every
//!   assertion survives that constant being wrong. That is `contention_phase.rs`.
//! - **Which index register a chain selects.** `docs/Z80-REFERENCE.md`'s rule that the last
//!   prefix wins, that `ED` ignores an index prefix, and that a prefix before an opcode
//!   which does not involve `HL` behaves as a `NOP` are graded by the FUSE vectors in
//!   `crates/z80` — `dd00` and `ddfd00` are prefix-chain cases and pass. This file grades
//!   only time and `R`.
//! - **A chain that crosses a frame boundary.** The clock rolls over mid-instruction by
//!   design and `ula.rs` unit-tests that it does, but no chain here is positioned to.
//! - **A chain long enough to wrap `PC`.** Nothing stops guest memory from containing one.

mod common;

use common::{
    CONTENDED_CODE, NOP, NOP_T_STATES, UNCONTENDED_CODE, advance_to, cost_of_running, machine,
    set_pc, with_cpu_state, write_program,
};
use spectrum::timing::FIRST_CONTENDED_T_STATE;

/// The prefix that substitutes `IX` for `HL` in the following instruction.
const PREFIX_DD: u8 = 0xDD;

/// The prefix that substitutes `IY`.
const PREFIX_FD: u8 = 0xFD;

/// The two positions within the ULA's group that every chain is measured at.
const PHASES: [u32; 2] = [0, 7];

/// How a chain's prefix bytes are chosen.
#[derive(Clone, Copy)]
enum Prefixes {
    /// Every byte is `DD`.
    AllDd,
    /// Every byte is `FD`.
    AllFd,
    /// `DD`, `FD`, `DD`, … — the last one wins, and the timing must not care.
    Alternating,
}

/// One chain: some prefix bytes, then a terminal `NOP`.
///
/// `NOP` terminates every chain because it is the opcode that adds nothing of its own — one
/// M1 cycle and no operand — so the whole instruction is `length + 1` identical fetches and
/// the arithmetic is about the chain rather than about what it prefixes.
struct Chain {
    name: &'static str,
    prefixes: Prefixes,
    length: usize,
    /// T-states out of uncontended memory: `4 * (length + 1)`.
    nominal: u64,
    /// T-states out of the screen bank, at [`PHASES`].
    contended: [u64; 2],
}

/// The chains, and where their figures come from.
///
/// Each was derived by walking the published delay pattern `[6, 5, 4, 3, 2, 1, 0, 0]` over a
/// cycle list **recorded** from a real `Cpu` — `length + 1` opcode fetches of four T-states
/// at consecutive addresses, nothing else — and cross-checked against a second
/// implementation of the rule written with no sight of `crates/spectrum`.
///
/// The two short chains are worth writing out, because the run **self-synchronises** after
/// its first fetch and that is what makes the long ones tractable. A fetch costs its stall
/// plus four T-states; from column 0 that is `6 + 4 = 10`, and from column 2 it is
/// `4 + 4 = 8` — exactly one ULA group, so every later fetch lands on column 2 again:
///
/// ```text
///   DD x4 + NOP, from phase 0                DD x4 + NOP, from phase 7
///     M1 4000 at +0    d=6  ->  4 -> +10       M1 4000 at +7    d=0  ->  4 -> +11
///     M1 4001 at +10   d=4  ->  4 -> +18       M1 4001 at +11   d=3  ->  4 -> +18
///     M1 4002 at +18   d=4  ->  4 -> +26       M1 4002 at +18   d=4  ->  4 -> +26
///     M1 4003 at +26   d=4  ->  4 -> +34       M1 4003 at +26   d=4  ->  4 -> +34
///     M1 4004 at +34   d=4  ->  4 -> +42       M1 4004 at +34   d=4  ->  4 -> +42
///                                  42 - 0 = 42                              42 - 7 = 35
/// ```
///
/// Sixteen prefixes is the largest run that stays inside one line's 128-T-state fetch
/// window, so `DD x16 + NOP` is the first chain that leaves it: fetch 15 opens at column 122
/// and stalls its 4, fetch 16 opens at column 130 and stalls nothing. `6 + 15 * 4 = 66` of
/// stall on 68 nominal is 134.
///
/// From there the long chains simply repeat that structure across lines, and `DD x63 + NOP`
/// — 64 fetches — is the one worth following, because it is the first that walks a whole
/// line and re-enters the next one:
///
/// ```text
///   fetches  0..15   line 0, columns 0..122     stall 6 + 15 * 4 = 66
///   fetches 16..39   line 0 border, from 130    free                        (24 fetches)
///   fetches 40..55   line 1, columns 2..114     stall 16 * 4     = 64
///   fetches 56..63   line 1 border, from 130    free                        (8 fetches)
///                                               total stall      = 130
///                                               256 nominal + 130 =    386
/// ```
///
/// `DD x64 + NOP` adds one more fetch, and it lands in the border at column 162 — free — so
/// the stall is unchanged at 130 and the total is `260 + 130 = 390`. `DD x200 + NOP`
/// continues the same walk; its figure is not written out here because the structure above
/// is the derivation and the arithmetic is mechanical, and because
/// [`a_prefix_chain_costs_what_the_same_bytes_cost_as_separate_nops`] reaches every one of
/// these numbers a second time by a construction that does not use the pattern at all.
static CHAINS: &[Chain] = &[
    Chain {
        name: "DD x4 + NOP",
        prefixes: Prefixes::AllDd,
        length: 4,
        nominal: 20,
        contended: [42, 35],
    },
    Chain {
        name: "DD x16 + NOP",
        prefixes: Prefixes::AllDd,
        length: 16,
        nominal: 68,
        contended: [134, 127],
    },
    Chain {
        name: "FD x16 + NOP",
        prefixes: Prefixes::AllFd,
        length: 16,
        nominal: 68,
        contended: [134, 127],
    },
    Chain {
        name: "DD/FD alternating x16 + NOP",
        prefixes: Prefixes::Alternating,
        length: 16,
        nominal: 68,
        contended: [134, 127],
    },
    Chain {
        name: "DD x63 + NOP",
        prefixes: Prefixes::AllDd,
        length: 63,
        nominal: 256,
        contended: [386, 379],
    },
    Chain {
        name: "DD x64 + NOP",
        prefixes: Prefixes::AllDd,
        length: 64,
        nominal: 260,
        contended: [390, 383],
    },
    Chain {
        name: "DD x200 + NOP",
        prefixes: Prefixes::AllFd,
        length: 200,
        nominal: 804,
        contended: [1130, 1123],
    },
];

impl Chain {
    /// The bytes: `length` prefixes, then the terminal `NOP`.
    fn assemble(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.length + 1);
        for byte in 0..self.length {
            bytes.push(match self.prefixes {
                Prefixes::AllDd => PREFIX_DD,
                Prefixes::AllFd => PREFIX_FD,
                Prefixes::Alternating if byte % 2 == 0 => PREFIX_DD,
                Prefixes::Alternating => PREFIX_FD,
            });
        }
        bytes.push(NOP);
        bytes
    }

    /// M1 cycles the whole instruction performs — one per byte.
    fn m1_cycles(&self) -> u64 {
        self.length as u64 + 1
    }
}

/// Run the chain once from `code_at`, with the clock at `at`, and report what it cost.
fn cost_of(chain: &Chain, code_at: u16, at: u32) -> u64 {
    let mut machine = machine();
    advance_to(&mut machine, at);
    write_program(&mut machine, code_at, &chain.assemble());
    cost_of_running(&mut machine, code_at, 1)
}

#[test]
fn a_prefix_chain_is_one_instruction_of_as_many_m1_cycles_as_it_has_bytes() {
    // The structural claim, and the reason the timing below is one number rather than a sum
    // over several. `Cpu::dispatch` consumes prefixes in a loop *within* one `step()`, so a
    // 200-byte run is a single instruction — while each byte is still its own M1 cycle with
    // its own refresh, which is what `R` settles.
    //
    // `Spectrum::step` returns the CPU's own T-state count, contention excluded, so it is
    // the right instrument for the nominal length and it is also the accumulator that a
    // `u8` could not hold.
    // `R` is pinned with bit 7 **set**, which costs nothing and buys a second property: the
    // refresh counter is seven bits wide with bit 7 a latch that only `LD R,A` moves, so a
    // 201-cycle chain must wrap the low seven bits — 201 mod 128 = 73 — and leave the top
    // one alone. Pinning it is also necessary rather than tidy: `advance_to` runs a prologue
    // of hundreds of instructions, so `R` arrives at whatever that left behind.
    const REFRESH_WITH_THE_LATCH_SET: u8 = 0x80;

    for chain in CHAINS {
        let mut machine = machine();
        advance_to(&mut machine, FIRST_CONTENDED_T_STATE);
        write_program(&mut machine, UNCONTENDED_CODE, &chain.assemble());
        with_cpu_state(&mut machine, |state| state.r = REFRESH_WITH_THE_LATCH_SET);
        set_pc(&mut machine, UNCONTENDED_CODE);

        let before = machine.cpu_state();
        let nominal = u64::from(machine.step());
        let after = machine.cpu_state();

        assert_eq!(
            nominal,
            chain.nominal,
            "{} is one instruction of {} M1 cycles, so one step must report {} T-states",
            chain.name,
            chain.m1_cycles(),
            chain.nominal
        );
        assert_eq!(
            nominal,
            chain.m1_cycles() * u64::from(NOP_T_STATES),
            "{}: every byte of a chain is a four-T-state opcode fetch",
            chain.name
        );
        assert_eq!(
            after.pc,
            UNCONTENDED_CODE
                + u16::try_from(chain.length + 1).expect("a chain that fits the address space"),
            "{}: one step must consume the whole chain and its terminal opcode",
            chain.name
        );
        assert_eq!(
            after.r,
            REFRESH_WITH_THE_LATCH_SET
                | u8::try_from(chain.m1_cycles() % 128).expect("a seven-bit refresh counter"),
            "{}: R counts M1 cycles, so it advances once per prefix byte and once more for \
             the opcode — {} in all — through seven bits only, leaving the latch in bit 7 \
             where it started at {:#04X}",
            chain.name,
            chain.m1_cycles(),
            before.r
        );
    }
}

#[test]
fn a_contended_prefix_chain_is_stalled_once_per_prefix_byte() {
    // The gate this file exists for. Each byte contends **on its own account**, at the frame
    // position its own fetch opens at — so the chain is not one stall multiplied but a walk
    // across the pattern, in and out of the fetch window, for as many bytes as guest memory
    // supplies.
    for chain in CHAINS {
        for (index, phase) in PHASES.into_iter().enumerate() {
            let at = FIRST_CONTENDED_T_STATE + phase;
            let contended = cost_of(chain, CONTENDED_CODE, at);
            let uncontended = cost_of(chain, UNCONTENDED_CODE, at);

            assert_eq!(
                uncontended, chain.nominal,
                "{} out of bank 0 must cost its nominal {} T-states at phase +{phase}. If \
                 this fails the control is wrong and the comparison below means nothing",
                chain.name, chain.nominal
            );
            assert_eq!(
                contended, chain.contended[index],
                "{} out of the screen bank at phase +{phase} must cost {} T-states against \
                 its nominal {}",
                chain.name, chain.contended[index], chain.nominal
            );
        }
    }
}

#[test]
fn a_prefix_chain_costs_what_the_same_bytes_cost_as_separate_nops() {
    // The second, independent construction for every figure in `CHAINS`, and it uses no
    // pattern arithmetic at all.
    //
    // A chain of N prefixes and a run of N+1 `NOP`s occupy the same N+1 addresses and emit
    // the same N+1 four-T-state opcode fetches. To the bus they are indistinguishable, so
    // from one starting position they must cost the same — even though the chain is one
    // `step()` and the `NOP`s are N+1 of them.
    //
    // That is a real assertion rather than a restatement: a machine that recognised the
    // chain as a single long cycle and contended it once, or that charged the prefixes at
    // the terminal opcode's address, would agree with the published nominal length and
    // disagree here.
    for chain in CHAINS {
        for phase in PHASES {
            let at = FIRST_CONTENDED_T_STATE + phase;

            let mut nops = machine();
            advance_to(&mut nops, at);
            let filler = vec![NOP; chain.length + 1];
            write_program(&mut nops, CONTENDED_CODE, &filler);
            let as_nops = cost_of_running(&mut nops, CONTENDED_CODE, filler.len());

            assert_eq!(
                cost_of(chain, CONTENDED_CODE, at),
                as_nops,
                "{} must cost exactly what {} NOPs over the same {} addresses cost from the \
                 same position: the bus sees one stream of {} opcode fetches either way, and \
                 only the CPU knows it was one instruction",
                chain.name,
                chain.m1_cycles(),
                chain.m1_cycles(),
                chain.m1_cycles()
            );
        }
    }
}

#[test]
fn a_prefix_chain_longer_than_a_byte_can_count_still_reports_its_whole_length() {
    // The regression this file's longest chains exist for. A `u8` T-state accumulator
    // overflowed at 63 prefixes, and under `overflow-checks = true` with `panic = "abort"`
    // that aborted the process on a legal instruction stream.
    //
    // 63 prefixes plus the opcode is 64 M1 cycles and 256 nominal T-states — one past what a
    // byte holds — so the shortest chain that could have overflowed is included as well as
    // the ones that dwarf it. Reaching this assertion at all is half the test.
    const A_BYTE: u64 = 256;

    let over: Vec<&Chain> = CHAINS.iter().filter(|c| c.nominal >= A_BYTE).collect();
    assert!(
        over.len() >= 3,
        "the overflow guard needs chains past a byte's range; found {}",
        over.len()
    );

    for chain in over {
        let mut machine = machine();
        advance_to(&mut machine, FIRST_CONTENDED_T_STATE);
        write_program(&mut machine, CONTENDED_CODE, &chain.assemble());
        set_pc(&mut machine, CONTENDED_CODE);

        assert_eq!(
            u64::from(machine.step()),
            chain.nominal,
            "{} reports {} nominal T-states in one step, which a byte cannot hold",
            chain.name,
            chain.nominal
        );
    }
}

#[test]
fn a_prefix_chain_outside_the_fetch_window_costs_the_same_in_either_bank() {
    // The control. Contention is a property of *when*, not only of *where*: the same chain,
    // in the same contended bank, at a position the ULA is not fetching in, must cost
    // exactly what uncontended memory costs. A model that stalled on address alone would
    // pass everything above and fail this.
    //
    // The longest chain is 804 T-states, so the position has to clear the whole instruction
    // out of the window rather than merely start before it.
    let longest = CHAINS
        .iter()
        .map(|chain| chain.nominal)
        .max()
        .expect("chains are not empty");
    let before_display = FIRST_CONTENDED_T_STATE
        - u32::try_from(longest).expect("a chain shorter than a frame")
        - NOP_T_STATES;

    for chain in CHAINS {
        assert_eq!(
            cost_of(chain, CONTENDED_CODE, before_display),
            chain.nominal,
            "{} must be free in the screen bank while the ULA is in the top border",
            chain.name
        );
    }
}
