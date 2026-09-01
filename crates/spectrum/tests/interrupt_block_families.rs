//! Gate: **all four** block families are interrupted mid-loop, not just the transfers.
//!
//! # Why this exists
//!
//! `block_interrupt.rs` drives `LDIR` and `LDDR` through two real 50 Hz acceptances and lists
//! its own hole in as many words:
//!
//! > **The compare, input and output families.** `CPIR`, `INIR` and `OTIR` share one
//! > `repeat_block` with the transfers, which is the *by construction* argument this project
//! > distrusts on principle — recorded here rather than relied on.
//!
//! *By construction* is the argument that has been wrong here before: `Ula`'s four-case I/O
//! rule was enumerated exhaustively and exercised at one phase, and deleting a whole term left
//! it green. The reason it matters more than usual for these three is that they do **not** in
//! fact behave identically to the transfers where an interrupt is concerned. Each family
//! spends its five repeat T-states on **a different register's address**, and
//! `docs/Z80-REFERENCE.md` records that as a trace finding rather than a specification:
//!
//! | | repeats on | because |
//! |---|---|---|
//! | `LDIR` / `LDDR` | `DE` | the write |
//! | `CPIR` / `CPDR` | `HL` | the read |
//! | `INIR` / `INDR` | `HL` | the write |
//! | `OTIR` / `OTDR` | the port — `BC` **after** `B`'s decrement | the output |
//!
//! So the address the loop is sitting on when the line goes low is a different register in
//! each family, and the input and output families additionally count **`B` alone**, leaving
//! `C` in place as half of the port. A wrong resume address and a wrong resume count would
//! compose exactly here, and only the transfer family had ever been driven through an
//! acceptance.
//!
//! # What is graded, and what would be decoration
//!
//! The end state is nearly useless as evidence, for the reason `block_interrupt.rs` measured:
//! a core that ran the whole loop inside one `step()` reaches the same registers and the same
//! memory, and merely drops the frame's interrupt. What discriminates is the **acceptance**:
//! the frame and offset the offer was made at, the counter still remaining, the address
//! pushed, and the T-states the acknowledge charged. Those are asserted for every one of the
//! eight forms.
//!
//! `LDIR` and `LDDR` are in the table too, and not as filler. They are the two forms whose
//! answer is already established independently, so their agreement here is the control that
//! the harness and the arithmetic are right before the other six are believed.
//!
//! # How the expected values were obtained
//!
//! Every machine-cycle stream below was **recorded off a real `Cpu` through a bus that logs
//! every transfer and tick**, in an out-of-tree scratch crate, and the recorder was validated
//! against a known answer first: `INC (HL)` must decompose as `pc:4, hl:3, hl:1, hl:3`, and it
//! does.
//!
//! ```text
//!   LDIR/LDDR  repeating  M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x7            21
//!              exit       M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x2            16
//!   CPIR/CPDR  repeating  M1@pc:4  M1@pc+1:4  MR@hl:3  IC@hl:1 x10                    21
//!              exit       M1@pc:4  M1@pc+1:4  MR@hl:3  IC@hl:1 x5                     16
//!   INIR/INDR  repeating  M1@pc:4  M1@pc+1:4  IC@ir:1  PR@bc:4  MW@hl:3  IC@hl:1 x5   21
//!              exit       M1@pc:4  M1@pc+1:4  IC@ir:1  PR@bc:4  MW@hl:3               16
//!   OTIR/OTDR  repeating  M1@pc:4  M1@pc+1:4  IC@ir:1  MR@hl:3  PW@bc:4  IC@bc:1 x5   21
//!              exit       M1@pc:4  M1@pc+1:4  IC@ir:1  MR@hl:3  PW@bc:4               16
//!   IM 2 acknowledge      IC@ir:1 x7  MW@sp-1:3  MW@sp-2:3  MR@vec:3  MR@vec+1:3      19
//!   EI 4   RET 10
//! ```
//!
//! Three things in that table are worth reading twice, because none of them is guessable and
//! each is load-bearing here. The **repeat address differs per family**, exactly as the
//! reference's trace table says. The input and output families carry an **extra T-state on
//! `IR`** — the `ED` page's second M1 is stretched to five — which is why they still reach 21
//! with a four-T-state port cycle in the middle. And `OTIR`'s port is `BC` with `B`
//! **already decremented**, while `INIR`'s is `BC` with `B` still at its entry value.
//!
//! All eight nevertheless cost a flat 21 repeating and 16 on the exit, which is what lets one
//! arithmetic serve the whole table. The acceptance points were then computed by a **second
//! implementation** of the frame clock and the acceptance rule, written from the published
//! figures with no sight of `crates/spectrum`; it agreed with the hand arithmetic before the
//! emulator was consulted.
//!
//! # A repeating pass can never step over the interrupt window, and that is a fact not a choice
//!
//! `frame_boundary.rs` grades an instruction long enough to overshoot the window and miss a
//! whole frame's interrupt. **No block instruction can do that.** A repeating pass is 21
//! T-states and the window is 32, so a pass that begins `d` T-states before a boundary ends at
//! offset `21 - d`, which is at most 20 — always inside. Checked over all 21 values of `d` in
//! the second implementation: **zero** miss. So a loop that crosses a boundary always takes
//! that frame's interrupt, and there is no "missed" case to gate for these families. It is
//! recorded here rather than left as an absence, because an absence reads as an oversight.
//!
//! # Everything here runs uncontended, deliberately
//!
//! Both blocks, the code, the handler, the vector table and every port the run puts on the bus
//! are in banks a 48K never contends, and the whole run additionally sits in the bottom border
//! and the top of the next frame, where the ULA is not fetching at all. So every pass costs
//! exactly 21 or 16 and the iteration an interrupt lands on is arithmetic.
//! `interrupt_contended_block.rs` is where the contended interaction is graded.
//!
//! # What is not graded here
//!
//! - **A `CPIR` that exits on a match rather than on `BC`.** The early exit is a second way out
//!   of the loop and nothing here or elsewhere interrupts a run that takes it.
//! - **What an `IN` from an unclaimed port returns.** This file asserts the 48K's idle bus
//!   value at positions in the border where a real machine's bus genuinely floats high; the
//!   position-dependent floating bus is not modelled at all, and `docs/STATUS.md` lists it as
//!   not gradeable rather than ungraded.
//! - **`OUT`'s effect.** The output family writes to a port nothing answers, so its only
//!   observable trace is `HL`, `B` and the time. That is stated rather than hidden: the
//!   read-only families are gated on having written **nothing**, which is the property that
//!   remains.

mod common;

use common::{
    AcceptedInterrupt, InterruptedRun, PROLOGUE, SLED, advance_to_absolute, enable_interrupts,
    machine, run_recording_interrupts, set_pc, with_cpu_state, write_program,
};
use spectrum::Spectrum;
use z80::InterruptMode;

// ---------------------------------------------------------------------------
// The published costs, written here as expectations
// ---------------------------------------------------------------------------

/// T-states one 48K frame lasts.
///
/// The literal rather than `timing::T_STATES_PER_FRAME`, for the reason `frame_boundary.rs`
/// uses the same literal: the positions below are this file's claim about where the boundary
/// is, and a claim taken from the subject is not a claim.
const FRAME_T_STATES: u64 = 69_888;

/// T-states a repeating pass costs, uncontended — the same for all eight forms.
const REPEATING_PASS: u64 = 21;

/// T-states the pass that exhausts the counter costs.
const EXIT_PASS: u64 = 16;

/// T-states a mode 2 interrupt acknowledge costs.
///
/// Seven of stretched M1 on the refresh address, two writes for the return address and two
/// reads for the vector: `7 + 3 + 3 + 3 + 3`.
const ACKNOWLEDGE: u64 = 19;

/// T-states the handler costs: `EI` and `RET`.
///
/// `EI` rather than a bare `RET` because its one-instruction deferral is load-bearing —
/// without it the `RET` would still be inside the window on an acceptance made at offset zero
/// and would take a second interrupt before returning.
const HANDLER_T_STATES: u64 = 4 + 10;

/// The whole cost of one accepted interrupt, from the offer to the resumed loop.
const INTERRUPT_COST: u64 = ACKNOWLEDGE + HANDLER_T_STATES;

// ---------------------------------------------------------------------------
// The run, and where its interrupt falls
// ---------------------------------------------------------------------------

/// Iterations each form runs.
///
/// Bounded by the input and output families, which count the **eight-bit** `B` and so cannot
/// exceed 256 passes — 5376 T-states, far less than a frame. A run that reaches a boundary by
/// sheer length, as `block_interrupt.rs`'s 8192-byte transfer does, is therefore impossible
/// for half this table. The boundary is brought to the loop instead: the machine is positioned
/// so that one nominated pass crosses it.
const ITERATIONS: u64 = 40;

/// The pass that crosses the frame boundary.
///
/// Deliberately in the middle. On the first pass the loop would not yet have proven it can
/// repeat at all, and on the last there would be no resumed tail to grade.
const CROSSING_PASS: u64 = 20;

/// How far past the boundary the crossing pass lands in the second case.
const OVERSHOOT: u64 = 12;

// The overshoot has to be reachable by a single pass, or the second case is not one pass past
// the boundary but two. A compile-time assertion rather than a runtime one, matching
// `timing.rs`'s own idiom: this is a property of the constants, and there is no run in which it
// could be true and then false.
const _: () = assert!(OVERSHOOT < REPEATING_PASS);

/// T-states of the tail: the passes after the acceptance, ending with the exit pass.
const TAIL: u64 = (ITERATIONS - CROSSING_PASS - 1) * REPEATING_PASS + EXIT_PASS;

/// T-states the whole run costs with no interrupt at all.
const UNINTERRUPTED: u64 = (ITERATIONS - 1) * REPEATING_PASS + EXIT_PASS;

/// And with the one interrupt, which must cost exactly its own time and nothing else.
const INTERRUPTED: u64 = UNINTERRUPTED + INTERRUPT_COST;

/// M1 cycles the interrupted run performs.
///
/// Two per pass — every form re-fetches both of its own opcode bytes each iteration, which is
/// what the `PC -= 2` rewind means, and measured to be two for all eight rather than assumed —
/// plus three for the interrupt: the acknowledge, which **refreshes without fetching**, and
/// the handler's `EI` and `RET`.
const M1_CYCLES: u32 = 2 * ITERATIONS as u32 + 3;

/// `R` on entry, with bit 7 set.
///
/// Seven bits wide with bit 7 a latch only `LD R,A` moves, so pinning it buys that property
/// for free — and pinning it is necessary rather than tidy, since positioning runs a prologue
/// of hundreds of instructions and `R` arrives at whatever that left behind.
const REFRESH_WITH_THE_LATCH_SET: u8 = 0x80;

/// One positioning of the run relative to the frame boundary.
struct Case {
    name: &'static str,
    /// T-states after power-on at which the block instruction starts.
    start: u64,
    /// The frame position at which the offer is made — where the crossing pass ends.
    accepted_at: u32,
}

/// The two positionings.
///
/// ```text
///   A frame is 69888 T-states and a repeating pass is 21.
///
///   "lands exactly on the boundary": start at 69888 - 20 * 21 = 69468. Passes 1..20 cost
///   420 and end on 69888 — frame 1, offset 0, the first T-state of the window. The offer
///   is made and accepted with 20 iterations still to run.
///
///   "overshoots it by 12": start twelve T-states later, at 69480. Passes 1..19 cost 399
///   and end on 69879, nine T-states short of the boundary; pass 20 runs 69879 -> 69900,
///   which is frame 1, offset 12. Still inside the window, so the offer is accepted — but
///   the clock carried its overshoot rather than resetting, which is where a machine that
///   restarted the frame at the boundary would begin to disagree.
///
///   Either way: acknowledge 19, EI 4, RET 10 = 33, then 19 repeating passes and one exit
///   pass = 19 * 21 + 16 = 415.
///
///       case A ends at   0 + 33 + 415 = 448     frame 1
///       case B ends at  12 + 33 + 415 = 460     frame 1
/// ```
///
/// The two are deliberately different shapes, for the reason `block_interrupt.rs` gives for
/// its own pair: one lands exactly on a boundary and one lands past it, and only the second
/// can see an overshoot being discarded.
const CASES: [Case; 2] = [
    Case {
        name: "the crossing pass lands exactly on the boundary",
        start: FRAME_T_STATES - CROSSING_PASS * REPEATING_PASS,
        accepted_at: 0,
    },
    Case {
        name: "the crossing pass overshoots the boundary",
        start: FRAME_T_STATES + OVERSHOOT - CROSSING_PASS * REPEATING_PASS,
        accepted_at: OVERSHOOT as u32,
    },
];

impl Case {
    /// Where the machine stands when the interrupted run finishes.
    const fn interrupted_end(&self) -> (u64, u32) {
        (1, self.accepted_at + (INTERRUPT_COST + TAIL) as u32)
    }

    /// And where the control, which takes no interrupt at all, finishes.
    const fn uninterrupted_end(&self) -> (u64, u32) {
        (1, (self.start + UNINTERRUPTED - FRAME_T_STATES) as u32)
    }
}

// ---------------------------------------------------------------------------
// Where everything lives
// ---------------------------------------------------------------------------

/// The block `HL` walks: read by the transfers, compares and outputs, written by the inputs.
const BLOCK: u16 = 0x9000;

/// The transfers' destination, which `DE` walks.
const SECOND: u16 = 0x9100;

/// The block instruction under test.
const CODE: u16 = 0x9200;

/// The interrupt handler: `EI`, `RET`.
const HANDLER: u16 = 0xA000;
const HANDLER_CODE: [u8; 2] = [0xFB, 0xC9];

/// `I`, and the vector pointer it forms with a floating `0xFF` on the data bus.
const VECTOR_HIGH: u8 = 0xA1;
const VECTOR_POINTER: u16 = 0xA1FF;

/// Where the stack starts, so the pushed return address has a known home.
const STACK_TOP: u16 = 0xFF00;

/// The low half of the port the input and output families use.
///
/// Odd, so `A0` is high and the ULA does not answer — the four-case I/O rule's *address free,
/// not the ULA's* case, which costs a flat port cycle with no stall. `C` is never touched by
/// either family, so this byte is on the bus for every pass.
const PORT_LOW: u8 = 0xFF;

/// What an `IN` from a port nothing answers reads back.
///
/// The 48K's data bus floats high, and every port cycle in this run happens in the border or
/// the top of a frame, where a real machine's bus genuinely idles. Written as the literal
/// rather than imported from `spectrum::FLOATING_BUS_BYTE`, which would be reading the
/// expectation off the subject.
const IDLE_BUS: u8 = 0xFF;

/// `DE` for the six families that never touch it, and which must leave it exactly so.
const DE_UNUSED: u16 = 0x1357;

/// The byte filling everything in the watched window that is not one of the two blocks.
///
/// A pass lost or repeated around an acceptance steps onto one of these, and would otherwise
/// be invisible: the blocks themselves look the same whether the loop was interrupted or not.
const FILLER: u8 = 0x5A;

/// The first address of the watched window, and how many bytes it spans.
///
/// One byte either side of both blocks and the gap between them, compared as a whole image
/// rather than as four guard bytes — the gap costs nothing to check and catches a stray write
/// that a guard pair would step over.
const IMAGE_AT: u16 = BLOCK - 1;
const IMAGE_LEN: usize = (SECOND - BLOCK) as usize + ITERATIONS as usize + 2;

/// Steps to allow before concluding the loop is not going to finish.
const STEP_BUDGET: usize = 200;

// ---------------------------------------------------------------------------
// The eight forms
// ---------------------------------------------------------------------------

/// Which register the family counts down, and therefore what `BC` holds mid-loop.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Counter {
    /// The transfers and the compares count the whole pair down to zero.
    Pair,
    /// The input and output families count `B` alone and leave `C` untouched — which is why
    /// `C` is still half of the port on the very last pass.
    HighByte,
}

/// What one pass does to memory.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Reads `(HL)`, writes `(DE)`.
    Transfer,
    /// Reads `(HL)` and writes nothing anywhere: the compares and the outputs.
    ReadOnly,
    /// Reads a port and writes `(HL)`.
    FillFromPort,
}

struct Family {
    name: &'static str,
    opcode: [u8; 2],
    /// Whether the pointers walk down rather than up.
    descending: bool,
    counter: Counter,
    effect: Effect,
}

static FAMILIES: [Family; 8] = [
    Family {
        name: "LDIR",
        opcode: [0xED, 0xB0],
        descending: false,
        counter: Counter::Pair,
        effect: Effect::Transfer,
    },
    Family {
        name: "LDDR",
        opcode: [0xED, 0xB8],
        descending: true,
        counter: Counter::Pair,
        effect: Effect::Transfer,
    },
    Family {
        name: "CPIR",
        opcode: [0xED, 0xB1],
        descending: false,
        counter: Counter::Pair,
        effect: Effect::ReadOnly,
    },
    Family {
        name: "CPDR",
        opcode: [0xED, 0xB9],
        descending: true,
        counter: Counter::Pair,
        effect: Effect::ReadOnly,
    },
    Family {
        name: "INIR",
        opcode: [0xED, 0xB2],
        descending: false,
        counter: Counter::HighByte,
        effect: Effect::FillFromPort,
    },
    Family {
        name: "INDR",
        opcode: [0xED, 0xBA],
        descending: true,
        counter: Counter::HighByte,
        effect: Effect::FillFromPort,
    },
    Family {
        name: "OTIR",
        opcode: [0xED, 0xB3],
        descending: false,
        counter: Counter::HighByte,
        effect: Effect::ReadOnly,
    },
    Family {
        name: "OTDR",
        opcode: [0xED, 0xBB],
        descending: true,
        counter: Counter::HighByte,
        effect: Effect::ReadOnly,
    },
];

impl Family {
    /// Where a pointer over `base` starts, given which way this form walks.
    fn walk_start(&self, base: u16) -> u16 {
        if self.descending {
            base + ITERATIONS as u16 - 1
        } else {
            base
        }
    }

    /// And where it stands after every iteration has run.
    fn walk_end(&self, base: u16) -> u16 {
        if self.descending {
            base.wrapping_sub(1)
        } else {
            base + ITERATIONS as u16
        }
    }

    /// `BC` on entry.
    fn bc_start(&self) -> u16 {
        self.bc_with(ITERATIONS as u16)
    }

    /// `BC` once the counter is exhausted.
    fn bc_end(&self) -> u16 {
        self.bc_with(0)
    }

    /// `BC` holding `remaining` iterations, in whichever register this form counts.
    fn bc_with(&self, remaining: u16) -> u16 {
        match self.counter {
            Counter::Pair => remaining,
            Counter::HighByte => (remaining << 8) | u16::from(PORT_LOW),
        }
    }

    /// `DE` on entry: the destination for a transfer, and an untouched sentinel otherwise.
    fn de_start(&self) -> u16 {
        match self.effect {
            Effect::Transfer => self.walk_start(SECOND),
            _ => DE_UNUSED,
        }
    }

    /// And `DE` at the end.
    fn de_end(&self) -> u16 {
        match self.effect {
            Effect::Transfer => self.walk_end(SECOND),
            _ => DE_UNUSED,
        }
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The block's contents: a ramp of distinct non-zero bytes, none of them [`FILLER`].
///
/// Non-zero because the compare family is entered with `A` at zero and must never match — a
/// match is the loop's *other* exit and would end the run early, on a different iteration,
/// with the arithmetic below silently wrong rather than failing.
fn block_bytes() -> Vec<u8> {
    (1..=ITERATIONS as u8).collect()
}

/// The watched window as it is written before the run.
fn image_before() -> Vec<u8> {
    let mut image = vec![FILLER; IMAGE_LEN];
    let block = (BLOCK - IMAGE_AT) as usize;
    image[block..block + ITERATIONS as usize].copy_from_slice(&block_bytes());
    let second = (SECOND - IMAGE_AT) as usize;
    image[second..second + ITERATIONS as usize].fill(0);
    image
}

/// And as it must stand afterwards.
fn image_after(family: &Family) -> Vec<u8> {
    let mut image = image_before();
    match family.effect {
        Effect::Transfer => {
            let second = (SECOND - IMAGE_AT) as usize;
            image[second..second + ITERATIONS as usize].copy_from_slice(&block_bytes());
        }
        Effect::FillFromPort => {
            let block = (BLOCK - IMAGE_AT) as usize;
            image[block..block + ITERATIONS as usize].fill(IDLE_BUS);
        }
        Effect::ReadOnly => {}
    }
    image
}

/// What the machine's watched window actually holds.
fn image_of(machine: &Spectrum) -> Vec<u8> {
    (0..IMAGE_LEN)
        .map(|offset| {
            machine
                .memory()
                .read(IMAGE_AT + u16::try_from(offset).expect("a small window"))
        })
        .collect()
}

/// A machine positioned at `case.start`, loaded with `family`, with the flip-flops as asked.
fn loaded(family: &Family, case: &Case, interrupts: bool) -> Spectrum {
    let mut machine = machine();
    advance_to_absolute(&mut machine, case.start);

    write_program(&mut machine, IMAGE_AT, &image_before());
    write_program(&mut machine, CODE, &family.opcode);
    write_program(&mut machine, HANDLER, &HANDLER_CODE);
    write_program(&mut machine, VECTOR_POINTER, &HANDLER.to_le_bytes());

    if interrupts {
        enable_interrupts(&mut machine, InterruptMode::Mode2);
    }
    with_cpu_state(&mut machine, |state| {
        // Zero, so the compare family never matches a block byte. See `block_bytes`.
        state.af = 0;
        state.hl = family.walk_start(BLOCK);
        state.de = family.de_start();
        state.bc = family.bc_start();
        state.i = VECTOR_HIGH;
        state.r = REFRESH_WITH_THE_LATCH_SET;
        state.sp = STACK_TOP;
    });
    set_pc(&mut machine, CODE);
    machine
}

/// Step until the block instruction leaves, recording every interrupt it takes.
///
/// The predicate is `PC` rather than the counter because the eight forms do not agree on what
/// the counter is: the inputs and outputs finish with `C` still in `BC`. What they do agree on
/// is that only the pass which exhausts the counter steps past the instruction.
fn run(machine: &mut Spectrum) -> InterruptedRun {
    run_recording_interrupts(machine, HANDLER, STEP_BUDGET, |state| state.pc == CODE + 2)
}

/// The one acceptance a run must have taken, or a legible failure if it took some other number.
fn sole_acceptance<'a>(outcome: &'a InterruptedRun, label: &str) -> &'a AcceptedInterrupt {
    assert_eq!(
        outcome.accepted.len(),
        1,
        "{label}: a run crossing exactly one frame boundary must take exactly one interrupt. \
         Zero would mean the offer was never made or never accepted inside the loop; more than \
         one would mean the handler's EI let a second in before the loop resumed"
    );
    &outcome.accepted[0]
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

#[test]
fn every_family_accepts_the_interrupt_between_iterations_with_its_own_loop_intact() {
    // The discriminating gate. A core that ran a whole block instruction inside one `step()`
    // would reach the same registers and the same memory; what it could not do is push the
    // instruction's own address with the counter still holding a derived, non-zero value.
    for family in &FAMILIES {
        for case in &CASES {
            let label = format!("{}, {}", family.name, case.name);
            let mut machine = loaded(family, case, true);
            let outcome = run(&mut machine);
            let accepted = sole_acceptance(&outcome, &label);

            assert_eq!(
                (accepted.frame, accepted.offset),
                (1, case.accepted_at),
                "{label}: the offer must be made in frame 1 at offset {}, where pass \
                 {CROSSING_PASS} ends",
                case.accepted_at
            );
            assert_eq!(
                accepted.bc,
                family.bc_with((ITERATIONS - CROSSING_PASS) as u16),
                "{label}: {CROSSING_PASS} of the {ITERATIONS} iterations are complete and the \
                 rest are still to run. An exhausted counter would mean the interrupt was taken \
                 after the whole instruction rather than inside it",
            );
            assert_eq!(
                accepted.return_address,
                CODE,
                "{label}: the acknowledge must push the instruction's **own** address. \
                 {:#06X} is where it would return to if the loop had already exited",
                CODE + 2
            );
            assert_eq!(
                u64::from(accepted.charged),
                ACKNOWLEDGE,
                "{label}: a mode 2 acknowledge is {ACKNOWLEDGE} T-states"
            );
            assert_eq!(machine.fault(), None, "{label}");
        }
    }
}

#[test]
fn every_family_resumes_after_the_handler_and_runs_its_loop_to_completion() {
    for family in &FAMILIES {
        for case in &CASES {
            let label = format!("{}, {}", family.name, case.name);
            let mut machine = loaded(family, case, true);
            let outcome = run(&mut machine);
            let state = machine.cpu_state();

            assert_eq!(
                state.bc,
                family.bc_end(),
                "{label}: the counter must be exhausted, and only the counter — the input and \
                 output families leave C alone",
            );
            assert_eq!(
                state.hl,
                family.walk_end(BLOCK),
                "{label}: HL must have walked exactly {ITERATIONS} bytes"
            );
            assert_eq!(
                state.de,
                family.de_end(),
                "{label}: DE must have walked with it for a transfer, and must be untouched for \
                 every other family"
            );
            assert_eq!(
                state.pc,
                CODE + 2,
                "{label}: only the pass that exhausts the counter steps past the instruction"
            );
            assert!(
                state.iff1,
                "{label}: the handler's EI must have left interrupts enabled again"
            );
            assert_eq!(
                state.sp, STACK_TOP,
                "{label}: the acceptance's return address must have been popped again"
            );
            assert_eq!(
                outcome.end,
                case.interrupted_end(),
                "{label}: the run must end in frame {} at offset {}",
                case.interrupted_end().0,
                case.interrupted_end().1
            );
            assert_eq!(
                state.r,
                REFRESH_WITH_THE_LATCH_SET
                    | u8::try_from(M1_CYCLES % 128).expect("a seven-bit refresh counter"),
                "{label}: R counts M1 cycles — two per pass whatever the family, and three more \
                 for the interrupt: the acknowledge, which refreshes without fetching, and the \
                 handler's EI and RET. {M1_CYCLES} in all through seven bits, with the latch \
                 left in bit 7 where it started"
            );
        }
    }
}

#[test]
fn the_interrupt_costs_only_its_own_time_and_no_iteration_is_lost_around_it() {
    // The other direction, and the one that rules out an iteration being dropped, repeated or
    // re-priced around the acceptance: the same run with the flip-flops clear must reach the
    // identical memory having spent exactly one handler less.
    for family in &FAMILIES {
        for case in &CASES {
            let label = format!("{}, {}", family.name, case.name);

            let mut interrupted = loaded(family, case, true);
            let with_interrupt = run(&mut interrupted);

            let mut quiet = loaded(family, case, false);
            let without = run(&mut quiet);

            assert!(
                without.accepted.is_empty(),
                "{label}: the control must take no interrupt at all"
            );
            assert_eq!(
                without.cost,
                UNINTERRUPTED,
                "{label}: {} passes at {REPEATING_PASS} T-states and one at {EXIT_PASS}",
                ITERATIONS - 1
            );
            assert_eq!(
                without.end,
                case.uninterrupted_end(),
                "{label}: and the control must end in frame {} at offset {}",
                case.uninterrupted_end().0,
                case.uninterrupted_end().1
            );
            assert_eq!(
                with_interrupt.cost, INTERRUPTED,
                "{label}: the interrupt must cost exactly {INTERRUPT_COST} T-states more — \
                 {ACKNOWLEDGE} of acknowledge and {HANDLER_T_STATES} of handler"
            );
            assert_eq!(
                with_interrupt.cost - without.cost,
                INTERRUPT_COST,
                "{label}: stated as the difference, which is the form that survives both \
                 figures being wrong in the same direction"
            );

            let expected = image_after(family);
            for (label, machine) in [("interrupted", &interrupted), ("quiet", &quiet)] {
                assert_eq!(
                    image_of(machine),
                    expected,
                    "{}, {}, {label}: the watched window must hold exactly what {ITERATIONS} \
                     passes leave behind — and nothing outside the family's own span may move, \
                     which is what a pass run long or short around the acceptance would do",
                    family.name,
                    case.name
                );
            }
        }
    }
}

#[test]
fn the_positioning_and_the_layout_are_what_the_derivation_assumes() {
    // The control for this file's own arithmetic rather than for the emulator. Every figure
    // above rests on premises that are cheap to state and would otherwise be silent: where the
    // crossing pass lands, that nothing in the run is contended, and that no two regions
    // overlap. None of the mutations run against this file reddens it, by construction — its
    // subject is the premises, and its reachable failing cases are a change to the frame
    // length, to the contended range, or to one of the addresses, each of which would silently
    // move every expectation rather than fail anything.
    assert_eq!(
        CASES[0].start + CROSSING_PASS * REPEATING_PASS,
        FRAME_T_STATES,
        "case A's crossing pass must end exactly on the top of frame 1"
    );
    assert_eq!(
        CASES[1].start + CROSSING_PASS * REPEATING_PASS,
        FRAME_T_STATES + OVERSHOOT,
        "and case B's must end {OVERSHOOT} T-states past it"
    );

    let mut machine = machine();
    advance_to_absolute(&mut machine, CASES[1].start);

    // Uncontended, asserted through the machine's own memory map rather than by reading off an
    // address range.
    for region in [
        BLOCK,
        SECOND,
        CODE,
        HANDLER,
        VECTOR_POINTER,
        STACK_TOP - 2,
        PROLOGUE,
        SLED,
    ] {
        assert!(
            !machine.memory().is_contended(region),
            "{region:#06X} must be in a bank a 48K never contends, or the passes do not cost a \
             flat {REPEATING_PASS} and every expectation in this file moves"
        );
    }

    // Every port the input and output families put on the bus, across the whole run. `INIR`
    // uses B before its decrement and `OTIR` after it, so between them they cover 0..=40 in
    // the high half.
    for counter in 0..=ITERATIONS as u16 {
        let port = (counter << 8) | u16::from(PORT_LOW);
        assert!(
            !machine.memory().is_contended(port),
            "{port:#06X} must not be a contended address, or the port cycle stalls"
        );
        assert_eq!(
            port & 1,
            1,
            "{port:#06X} must have A0 high, or the ULA answers it and the cycle stalls"
        );
    }

    // No two regions may overlap, and the watched window must contain both blocks whole.
    assert_eq!(IMAGE_AT + IMAGE_LEN as u16, SECOND + ITERATIONS as u16 + 1);
    let mut spans = [
        (IMAGE_AT, IMAGE_LEN as u16),
        (CODE, 2),
        (HANDLER, 2),
        (VECTOR_POINTER, 2),
    ];
    spans.sort_unstable();
    for pair in spans.windows(2) {
        let (start, len) = pair[0];
        assert!(
            start + len <= pair[1].0,
            "{start:#06X}+{len} runs into {:#06X}",
            pair[1].0
        );
    }
}
