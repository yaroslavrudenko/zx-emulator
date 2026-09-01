//! Gate: an interrupt arriving mid-loop **while the loop is being contended**.
//!
//! # Why this exists
//!
//! `block_interrupt.rs` and `interrupt_block_families.rs` both run wholly uncontended on
//! purpose, so the iteration an interrupt lands on is arithmetic. `docs/STATUS.md` carries the
//! remainder as an open row:
//!
//! > **An interrupt arriving mid-loop while the loop is being *contended*.** `block_interrupt.rs`
//! > runs wholly uncontended on purpose, so the iteration an interrupt lands on is arithmetic
//! > rather than a simulation of the model `block_contention.rs` grades.
//!
//! `block_contention.rs` prices a contended loop and nothing interrupts one; the two gates meet
//! nowhere. That seam is where a loop mispriced under contention and an acceptance rule that
//! reads the clock would compose: the emulator would take the interrupt on the wrong iteration,
//! with every individual figure still defensible.
//!
//! # The 48K's own geometry decides the shape of this gate, and it is worth stating first
//!
//! The ULA holds `/INT` low for the frame's first 32 T-states. The first **contended**
//! T-state is 14335. So the window lies wholly, and by a margin of three orders of magnitude,
//! **before** anything contends:
//!
//! > **An interrupt acknowledge on a 48K can never be contended.** Not "is not, in this
//! > fixture" — cannot be, at any address, for any program. Every acceptance happens in the top
//! > border, and `delay()` is zero there whatever is on the bus.
//!
//! That is not a licence to skip the case; it is the reason the case has the shape it does.
//! What can be contended is the **loop**, and the accumulated stall then decides which
//! iteration is still running when the boundary arrives. So this file grades two things that
//! sound like one:
//!
//! - the acknowledge itself, driven with the stack, the vector table and the handler **all in
//!   the contended bank**, costing exactly its nominal 19 T-states — with a control proving
//!   those same addresses do stall when touched inside the display area, so the 19 is a
//!   measurement and not a tautology about addresses that never contend;
//! - the loop, contended, crossing a frame boundary and taking its interrupt **twenty-two
//!   iterations earlier** than the identical run with its source one bank over.
//!
//! # The fixture has exactly one contention point per pass, and that is deliberate
//!
//! `LDIR`'s recorded stream — taken off a real `Cpu` through a recording bus in an out-of-tree
//! scratch crate, the recorder validated against `INC (HL)` decomposing as
//! `pc:4, hl:3, hl:1, hl:3` first — is
//!
//! ```text
//!   M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x7        21
//! ```
//!
//! With the **source** in the screen bank and the code, the destination and its seven internal
//! T-states all in a bank a 48K never contends, exactly one cycle in the pass can stall: the
//! `MR` on `HL`, which opens **eight** T-states into the pass. A pass beginning at frame
//! position `t` therefore costs `21 + delay(t + 8)`, and the stall it suffers **shifts the
//! start of every pass after it**.
//!
//! That last clause is the whole reason this file cannot be written by adding a correction to a
//! total. This project has the scar: two derivations of a contended `INC (HL)` produced 26 and
//! 30, and the 30 came from taking an observed 25 and adding "the missing 5". **A missing stall
//! cannot be added to a total, because every stall moves the ones after it.**
//!
//! # How the expected values were obtained
//!
//! By a **second implementation** of the frame clock, the delay pattern and the acceptance
//! rule, written from the published 48K figures — `224 * 312`, a 32 T-state window, the first
//! contended T-state at 14335, 192 display lines of 128 contended T-states each, the pattern
//! `[6, 5, 4, 3, 2, 1, 0, 0]` by position within an eight-T-state group — with no sight of
//! `crates/spectrum`, and run before the emulator was consulted. It walks the pass structure
//! above from [`START`] and reports:
//!
//! ```text
//!   nominal, 1000 passes                999 * 21 + 16      = 20995
//!   stall accumulated in frame 0's display area            =   459
//!   the run with interrupts disabled                       = 21454
//!   one acknowledge and one EI/RET handler                 =    33
//!   the run with interrupts enabled                        = 21487
//!
//!   the boundary is crossed by pass 926, which ends at frame 1 offset 17
//!   with the source one bank over it is crossed by pass 948, at offset 20
//! ```
//!
//! The 459 is what moves the acceptance twenty-two iterations: at 21 T-states a pass, 459
//! T-states of stall is nearly twenty-two passes of head start.
//!
//! # What is not graded here
//!
//! - **Whether the delay pattern is right.** `block_contention.rs` and `contention_magnitude.rs`
//!   grade the pattern; `timing_oracle.rs` grades the interval it starts at against measured
//!   hardware. This file takes all three as given and grades what happens when an acceptance
//!   meets them.
//! - **A contended *handler*.** The handler here lives in the contended bank, but it runs at
//!   offsets 19–33 of a frame, where nothing stalls. There is no position at which a handler
//!   reached from a frame interrupt could begin inside the display area, for the geometric
//!   reason above.
//! - **The compare, input and output families under contention.** They repeat on different
//!   addresses — `HL`, `HL` and the port — so each would need its own fixture to isolate one
//!   contention point per pass. `interrupt_block_families.rs` drives all eight uncontended.

mod common;

use common::{
    InterruptedRun, NOP, PROLOGUE, SLED, UNCONTENDED_CODE, advance_to_absolute, elapsed,
    enable_interrupts, machine, run_recording_interrupts, set_pc, with_cpu_state, write_program,
};
use spectrum::Spectrum;
use z80::InterruptMode;

// ---------------------------------------------------------------------------
// The published costs, written here as expectations
// ---------------------------------------------------------------------------

/// T-states one 48K frame lasts.
const FRAME_T_STATES: u64 = 69_888;

/// The first T-state of a frame at which a contended access is delayed.
///
/// `64 * 224 - 1`: 64 lines of vertical retrace and top border, less one. The literal rather
/// than `timing::FIRST_CONTENDED_T_STATE` for the same reason as everything else here — this
/// file's claim is that the interrupt window lies before it, and a claim taken from the subject
/// is not a claim. `timing_oracle.rs` is what grades the number itself, against hardware.
const FIRST_CONTENDED: u64 = 14_335;

/// T-states a repeating `LDIR` pass costs before any stall.
const REPEATING_PASS: u64 = 21;

/// T-states the pass that exhausts `BC` costs before any stall.
const EXIT_PASS: u64 = 16;

/// T-states a mode 2 interrupt acknowledge costs.
const ACKNOWLEDGE: u64 = 19;

/// T-states the handler costs: `EI` and `RET`.
const HANDLER_T_STATES: u64 = 4 + 10;

/// The whole cost of one accepted interrupt.
const INTERRUPT_COST: u64 = ACKNOWLEDGE + HANDLER_T_STATES;

/// T-states a `PUSH` costs before any stall: `M1@pc:4  IC@ir:1  MW@sp-1:3  MW@sp-2:3`.
///
/// Recorded, not assumed — the stretched five-T-state M1 is not guessable from the mnemonic,
/// and the two writes are the same pair of cycles an interrupt acknowledge performs, which is
/// what makes this the right control for it.
const PUSH_T_STATES: u64 = 11;

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// Bytes the loop moves.
///
/// Large enough to start inside the display area and still be running when the frame boundary
/// arrives twelve thousand T-states later, and small enough that the run ends well before the
/// **next** frame's display area — so all the contention it suffers is in one place and the
/// tail is flat.
const BLOCK_LEN: usize = 1000;

/// `BC` on entry.
const BC_START: u16 = BLOCK_LEN as u16;

/// T-states after power-on at which the loop starts.
///
/// Inside frame zero's display area — between the first contended T-state, 14335, and the last,
/// 57342 — with roughly seven thousand T-states of it left to run through.
const START: u64 = 50_000;

/// Stall the contended run accumulates, all of it in frame zero's display area.
const STALL: u64 = 459;

/// T-states the loop costs with neither stall nor interrupt.
const NOMINAL: u64 = (BC_START as u64 - 1) * REPEATING_PASS + EXIT_PASS;

/// One expected outcome: which pass crosses the boundary, and where the run finishes.
struct Expectation {
    /// Passes completed when the offer is made.
    accepted_after: u64,
    /// The frame position the offer is made at.
    accepted_at: u32,
    /// Stall the run suffers.
    stall: u64,
}

impl Expectation {
    /// Where the machine stands when the interrupted run finishes.
    fn interrupted_end(&self) -> (u64, u32) {
        (
            1,
            (START + NOMINAL + self.stall + INTERRUPT_COST - FRAME_T_STATES) as u32,
        )
    }

    /// And where the control, which takes no interrupt, finishes.
    fn uninterrupted_end(&self) -> (u64, u32) {
        (1, (START + NOMINAL + self.stall - FRAME_T_STATES) as u32)
    }

    /// `BC` as the acceptance must find it.
    fn remaining(&self) -> u16 {
        BC_START - self.accepted_after as u16
    }
}

/// The contended run: the source in the screen bank.
const CONTENDED: Expectation = Expectation {
    accepted_after: 926,
    accepted_at: 17,
    stall: STALL,
};

/// The same run with the source one bank over, where nothing contends.
const FREE: Expectation = Expectation {
    accepted_after: 948,
    accepted_at: 20,
    stall: 0,
};

// ---------------------------------------------------------------------------
// Where everything lives
// ---------------------------------------------------------------------------

/// The source in the screen bank — bank 5, the only bank a 48K contends.
const SOURCE_CONTENDED: u16 = 0x4800;

/// And the same block one bank over, in RAM a 48K never contends.
///
/// The two runs differ in this address and in nothing else, which is what makes the difference
/// between them attributable to contention and to nothing else.
const SOURCE_FREE: u16 = 0x8C00;

/// The destination, uncontended — so the write and the seven internal T-states after it never
/// stall, and exactly one cycle per pass can.
const DESTINATION: u16 = 0xA000;

/// The block instruction under test.
const CODE: u16 = 0x9200;

/// The handler: `EI`, `RET`.
const HANDLER: u16 = 0x9400;
const HANDLER_CODE: [u8; 2] = [0xFB, 0xC9];

/// `I`, and the vector pointer it forms with a floating `0xFF` on the data bus.
const VECTOR_HIGH: u8 = 0x95;
const VECTOR_POINTER: u16 = 0x95FF;

/// Where the stack starts.
const STACK_TOP: u16 = 0xFF00;

/// A byte written either side of every block, and asserted to survive.
const GUARD: u8 = 0x5A;

/// Steps to allow before concluding the loop is not going to finish.
const STEP_BUDGET: usize = 4_000;

// ---------------------------------------------------------------------------
// The contended acknowledge fixture
// ---------------------------------------------------------------------------

/// A stack inside the screen bank.
const CONTENDED_STACK: u16 = 0x7000;

/// A handler and a vector table inside it too.
const CONTENDED_HANDLER: u16 = 0x5000;
const CONTENDED_VECTOR_HIGH: u8 = 0x51;
const CONTENDED_VECTOR_POINTER: u16 = 0x51FF;

/// `PUSH HL`.
const PUSH_HL: u8 = 0xE5;

/// A frame position at which a `PUSH` onto [`CONTENDED_STACK`] straddles the first contended
/// T-state.
///
/// `PUSH` spends four T-states fetching and one on `IR` before its first write, so a `PUSH`
/// beginning five T-states before 14335 opens that write exactly on it — the pattern's first
/// column, where it stalls the full six. Written as the arithmetic rather than as a bare
/// number so the choice is checkable.
const PUSH_STRADDLING_THE_FIRST_CONTENDED_T_STATE: u64 = FIRST_CONTENDED - 5;

/// What that `PUSH` costs.
///
/// ```text
///   M1@pc:4      14330 -> 14334      uncontended code, no stall
///   IC@ir:1      14334 -> 14335      I is zero here, so IR is not a contended address
///   MW@sp-1:3    opens on 14335, column 0 of the pattern: stall 6, then 3   -> 14344
///   MW@sp-2:3    opens on 14344, column 9, pattern index 1: stall 5, then 3 -> 14352
/// ```
///
/// 22 T-states for an 11 T-state instruction — and the second stall is priced at a column the
/// first stall moved it to, which is the arithmetic that cannot be done by adding a correction
/// to a total.
const PUSH_CONTENDED_T_STATES: u64 = 22;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The block to copy: a period-251 ramp, so any shift of the copy is visible everywhere.
fn source_bytes() -> Vec<u8> {
    (0..BLOCK_LEN).map(|i| (i % 251) as u8).collect()
}

/// The four bytes that must survive, for a run whose source is at `source`.
fn guards(source: u16) -> [u16; 4] {
    [
        source - 1,
        source + BC_START,
        DESTINATION - 1,
        DESTINATION + BC_START,
    ]
}

/// A machine at [`START`] running `LDIR` out of `source`, with the flip-flops as asked.
fn loaded(source: u16, interrupts: bool) -> Spectrum {
    let mut machine = machine();
    advance_to_absolute(&mut machine, START);

    write_program(&mut machine, source, &source_bytes());
    write_program(&mut machine, DESTINATION, &vec![0; BLOCK_LEN]);
    for guard in guards(source) {
        write_program(&mut machine, guard, &[GUARD]);
    }
    write_program(&mut machine, CODE, &[0xED, 0xB0]);
    write_program(&mut machine, HANDLER, &HANDLER_CODE);
    write_program(&mut machine, VECTOR_POINTER, &HANDLER.to_le_bytes());

    if interrupts {
        enable_interrupts(&mut machine, InterruptMode::Mode2);
    }
    with_cpu_state(&mut machine, |state| {
        state.hl = source;
        state.de = DESTINATION;
        state.bc = BC_START;
        state.i = VECTOR_HIGH;
        state.sp = STACK_TOP;
    });
    set_pc(&mut machine, CODE);
    machine
}

/// Step until the transfer leaves, recording every interrupt it takes.
fn run(machine: &mut Spectrum) -> InterruptedRun {
    run_recording_interrupts(machine, HANDLER, STEP_BUDGET, |state| {
        state.pc == CODE + 2 && state.bc == 0
    })
}

/// A machine at `at` T-states after power-on, with a mode 2 interrupt able to be accepted and
/// its stack, vector table and handler all inside the contended bank.
fn armed_with_everything_contended(at: u64) -> Spectrum {
    let mut machine = machine();
    advance_to_absolute(&mut machine, at);

    write_program(&mut machine, CONTENDED_HANDLER, &HANDLER_CODE);
    write_program(
        &mut machine,
        CONTENDED_VECTOR_POINTER,
        &CONTENDED_HANDLER.to_le_bytes(),
    );
    write_program(&mut machine, UNCONTENDED_CODE, &[NOP; 4]);
    enable_interrupts(&mut machine, InterruptMode::Mode2);
    with_cpu_state(&mut machine, |state| {
        state.i = CONTENDED_VECTOR_HIGH;
        state.sp = CONTENDED_STACK;
    });
    set_pc(&mut machine, UNCONTENDED_CODE);
    machine
}

/// What one `PUSH HL` onto [`CONTENDED_STACK`] costs, positioned at `at`.
fn cost_of_a_push_at(at: u64) -> u64 {
    let mut machine = machine();
    advance_to_absolute(&mut machine, at);
    write_program(&mut machine, UNCONTENDED_CODE, &[PUSH_HL]);
    with_cpu_state(&mut machine, |state| {
        state.sp = CONTENDED_STACK;
        // `IR` is on the bus for this instruction's one internal T-state, and a contended `I`
        // would add a stall this control is not about.
        state.i = 0;
    });
    set_pc(&mut machine, UNCONTENDED_CODE);
    let before = elapsed(&machine);
    machine.step();
    elapsed(&machine) - before
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

#[test]
fn a_contended_loop_takes_its_interrupt_on_the_iteration_the_stalls_put_it_on() {
    let mut machine = loaded(SOURCE_CONTENDED, true);
    let outcome = run(&mut machine);

    assert_eq!(
        outcome.accepted.len(),
        1,
        "the run crosses exactly one frame boundary and must take exactly one interrupt"
    );
    let accepted = outcome.accepted[0];

    assert_eq!(
        (accepted.frame, accepted.offset),
        (1, CONTENDED.accepted_at),
        "the offer must be made in frame 1 at offset {}: {STALL} T-states of accumulated stall \
         put the boundary inside pass {}, which ends there",
        CONTENDED.accepted_at,
        CONTENDED.accepted_after
    );
    assert_eq!(
        accepted.bc,
        CONTENDED.remaining(),
        "{} of the {BLOCK_LEN} iterations must be complete when the line goes low",
        CONTENDED.accepted_after
    );
    assert_eq!(
        accepted.return_address,
        CODE,
        "the acknowledge must push the instruction's own address, not {:#06X}",
        CODE + 2
    );
    assert_eq!(
        outcome.end,
        CONTENDED.interrupted_end(),
        "and the run must end in frame {} at offset {}",
        CONTENDED.interrupted_end().0,
        CONTENDED.interrupted_end().1
    );
    assert_eq!(machine.cpu_state().bc, 0, "the counter must be exhausted");
    assert_eq!(machine.fault(), None);
}

#[test]
fn contention_moves_the_iteration_the_interrupt_lands_on() {
    // The discriminating comparison, and the reason this file exists rather than deferring to
    // `block_contention.rs` plus `block_interrupt.rs`. The two runs are identical in every
    // respect but one — which bank the source sits in — and they take their interrupt
    // twenty-two iterations apart. An emulator that priced a contended loop correctly and
    // offered the interrupt from a clock that had not been advanced by the stalls, or the
    // reverse, would agree with both of those gates and disagree here.
    let mut contended = loaded(SOURCE_CONTENDED, true);
    let hot = run(&mut contended);

    let mut free = loaded(SOURCE_FREE, true);
    let cold = run(&mut free);

    let hot_at = hot.accepted[0];
    let cold_at = cold.accepted[0];

    assert_eq!(
        (BC_START - hot_at.bc, BC_START - cold_at.bc),
        (CONTENDED.accepted_after as u16, FREE.accepted_after as u16),
        "the contended run must be {} passes in when the boundary arrives and the free run {}",
        CONTENDED.accepted_after,
        FREE.accepted_after
    );
    // The contended run is *fewer* passes in when the boundary arrives, so it has *more* left:
    // the stall spends frame time without spending iterations.
    assert_eq!(
        hot_at.bc - cold_at.bc,
        (FREE.accepted_after - CONTENDED.accepted_after) as u16,
        "the {STALL} T-states of stall are worth {} iterations of head start at \
         {REPEATING_PASS} T-states each — stated as the difference, which is the form that \
         survives both figures being wrong in the same direction",
        FREE.accepted_after - CONTENDED.accepted_after
    );
    assert_eq!(
        (cold_at.frame, cold_at.offset),
        (1, FREE.accepted_at),
        "and the free run's own offer must land at offset {}",
        FREE.accepted_at
    );
    assert_eq!(cold.end, FREE.interrupted_end());
}

#[test]
fn the_stall_is_the_only_difference_and_the_interrupt_costs_only_its_own_time() {
    // Four runs: contended and free, each with the flip-flops set and clear. Together they
    // separate the two quantities that a single run conflates — what contention cost and what
    // the interrupt cost — and show that neither moved the other.
    let expected = source_bytes();

    for (label, source, expectation) in [
        ("contended", SOURCE_CONTENDED, &CONTENDED),
        ("free", SOURCE_FREE, &FREE),
    ] {
        let mut interrupted = loaded(source, true);
        let with_interrupt = run(&mut interrupted);

        let mut quiet = loaded(source, false);
        let without = run(&mut quiet);

        assert!(
            without.accepted.is_empty(),
            "{label}: the control must take no interrupt at all"
        );
        assert_eq!(
            without.cost,
            NOMINAL + expectation.stall,
            "{label}: {BLOCK_LEN} passes cost {NOMINAL} nominal, and this one suffers {} \
             T-states of stall on the way through the display area",
            expectation.stall
        );
        assert_eq!(
            without.end,
            expectation.uninterrupted_end(),
            "{label}: and the control must end in frame {} at offset {}",
            expectation.uninterrupted_end().0,
            expectation.uninterrupted_end().1
        );
        assert_eq!(
            with_interrupt.cost - without.cost,
            INTERRUPT_COST,
            "{label}: the interrupt must cost exactly {ACKNOWLEDGE} of acknowledge and \
             {HANDLER_T_STATES} of handler — no iteration lost, repeated or re-priced around it, \
             and no stall gained or dropped because the loop resumed at a different column"
        );

        for (which, machine) in [("interrupted", &interrupted), ("quiet", &quiet)] {
            for (offset, want) in expected.iter().enumerate() {
                let address = DESTINATION + u16::try_from(offset).expect("a block in range");
                assert_eq!(
                    machine.memory().read(address),
                    *want,
                    "{label}, {which}: {address:#06X} must hold the byte copied to it"
                );
            }
            for guard in guards(source) {
                assert_eq!(
                    machine.memory().read(guard),
                    GUARD,
                    "{label}, {which}: {guard:#06X} is outside both blocks and must be untouched"
                );
            }
        }
    }

    // The two controls differ by exactly the stall, which is the same statement made without
    // reference to the nominal figure at all.
    let mut hot = loaded(SOURCE_CONTENDED, false);
    let mut cold = loaded(SOURCE_FREE, false);
    assert_eq!(
        run(&mut hot).cost - run(&mut cold).cost,
        STALL,
        "one bank apart, the same {BLOCK_LEN} passes over the same frame positions must differ \
         by exactly the stall the ULA's pattern imposes"
    );
}

#[test]
fn an_acknowledge_is_never_contended_because_the_window_precedes_the_display() {
    // The other half of the seam. The stack, the vector table and the handler are all in the
    // screen bank, so every one of the acknowledge's four memory cycles is at a **contended
    // address** — and none of them stalls, because the ULA's 32 T-state window is over twelve
    // thousand T-states before the first contended T-state.
    //
    // Measured on the **clock**, not on what `step` returns: `Spectrum::step` reports the CPU's
    // own charge, and contention is added on the bus's side where that number cannot see it. A
    // gate on `step`'s return would read 19 whether or not the acknowledge had stalled.
    for offset in [0, 31] {
        let mut machine = armed_with_everything_contended(FRAME_T_STATES + offset);
        assert_eq!(machine.frame_t_state(), offset as u32);

        let before = elapsed(&machine);
        machine.step();
        let cost = elapsed(&machine) - before;

        assert_eq!(
            machine.cpu_state().pc,
            CONTENDED_HANDLER,
            "offset {offset}: the offer must have been accepted, or the cost below is a NOP's"
        );
        assert_eq!(
            cost, ACKNOWLEDGE,
            "offset {offset}: a mode 2 acknowledge with its stack at {CONTENDED_STACK:#06X} and \
             its vector at {CONTENDED_VECTOR_POINTER:#06X} — both contended addresses — must \
             still cost its nominal {ACKNOWLEDGE} T-states"
        );
        assert!(
            machine.memory().is_contended(CONTENDED_STACK)
                && machine.memory().is_contended(CONTENDED_VECTOR_POINTER)
                && machine.memory().is_contended(CONTENDED_HANDLER),
            "offset {offset}: and those addresses must really be in the contended bank, or the \
             assertion above says nothing"
        );
    }

    // The control that makes the two 19s mean something. The same stack, written by the same
    // pair of memory cycles an acknowledge performs, costs its nominal 11 T-states at the top
    // of a frame and 11 more inside the display area.
    assert_eq!(
        cost_of_a_push_at(FRAME_T_STATES),
        PUSH_T_STATES,
        "a PUSH onto the contended stack costs its nominal {PUSH_T_STATES} at frame offset zero"
    );
    assert_eq!(
        cost_of_a_push_at(PUSH_STRADDLING_THE_FIRST_CONTENDED_T_STATE),
        PUSH_CONTENDED_T_STATES,
        "and {PUSH_CONTENDED_T_STATES} when its first write opens on the first contended \
         T-state — so the bank does contend, the two writes do stall, and the acknowledge's \
         escaping both is a property of *when* it happens rather than of *what* it touches"
    );
}

#[test]
fn the_positioning_and_the_layout_are_what_the_derivation_assumes() {
    // The control for this file's own premises rather than for the emulator: which bank each
    // region is in, that the run starts inside the display area, and that it ends before the
    // next one — each of which would silently move every figure above rather than fail
    // anything.
    let mut machine = machine();
    advance_to_absolute(&mut machine, START);
    assert_eq!(elapsed(&machine), START);

    assert!(
        machine.memory().is_contended(SOURCE_CONTENDED)
            && machine
                .memory()
                .is_contended(SOURCE_CONTENDED + BC_START - 1),
        "the whole contended source must be in the bank a 48K contends"
    );
    for region in [
        SOURCE_FREE,
        SOURCE_FREE + BC_START - 1,
        DESTINATION,
        DESTINATION + BC_START - 1,
        CODE,
        HANDLER,
        VECTOR_POINTER,
        STACK_TOP - 2,
        PROLOGUE,
        SLED,
        UNCONTENDED_CODE,
    ] {
        assert!(
            !machine.memory().is_contended(region),
            "{region:#06X} must be in a bank a 48K never contends, or a second cycle per pass \
             stalls and every figure in this file moves"
        );
    }

    // The run must begin inside frame zero's display area and end before frame one's, or the
    // stall is not all in one place and the tail is not flat. The two bounds are the published
    // geometry — 64 lines of top border, then 192 display lines of 224 T-states — written out
    // rather than imported, for the same reason every other figure in this file is.
    assert!(
        (FIRST_CONTENDED..=FIRST_CONTENDED + 192 * 224 - 1).contains(&START),
        "the loop must start inside the display area"
    );
    assert!(
        u64::from(CONTENDED.interrupted_end().1) < FIRST_CONTENDED,
        "and must finish before frame one's display area begins, or it accumulates a second \
         stall this file does not account for"
    );

    // No two regions may overlap.
    let mut spans = [
        (SOURCE_CONTENDED - 1, BC_START + 2),
        (SOURCE_FREE - 1, BC_START + 2),
        (DESTINATION - 1, BC_START + 2),
        (CODE, 2),
        (HANDLER, 2),
        (VECTOR_POINTER, 2),
        (CONTENDED_HANDLER, 2),
        (CONTENDED_VECTOR_POINTER, 2),
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
