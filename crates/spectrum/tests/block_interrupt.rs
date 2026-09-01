//! Gate: a block instruction is **actually interrupted**, mid-loop, by the machine's own
//! 50 Hz interrupt — and resumes and completes correctly afterwards.
//!
//! # Why this exists
//!
//! `docs/Z80-REFERENCE.md` records the repeat mechanism and then justifies it:
//!
//! > Their repeat is `PC -= 2` with **one `step()` per iteration** — the instruction
//! > re-fetches its own opcode each pass, so `R` advances by two per iteration. That is not an
//! > implementation choice: it is what keeps a 64 KB `LDIR` interruptible, and therefore what
//! > lets it coexist with a 50 Hz frame interrupt.
//!
//! **That justification has never been tested.** `block_contention.rs` grades the rewind — it
//! asserts that every repeating pass leaves `PC` on the instruction's own first byte and only
//! the exit pass steps past it — which *shows* the loop is interruptible. Nothing interrupts
//! one. `docs/STATUS.md` lists interrupt acceptance as having **no oracle in this project at
//! all**, and this is the one place where the CPU's interruptibility design and the machine's
//! frame interrupt meet.
//!
//! So this file drives a `LDIR` of [`BLOCK_LEN`] bytes — long enough to span three frames —
//! with interrupts enabled, and grades what the hardware does when the line goes low partway
//! through: the interrupt is accepted **between iterations**, `PC` points at the instruction
//! rather than past it, the remaining `BC` is intact, and the loop resumes and finishes the
//! copy.
//!
//! # A finished copy proves almost nothing, so this file does not rest on one
//!
//! The measurable trap here is that **the end state is the same whether the loop was
//! interruptible or not.** A core that ran the whole `LDIR` inside one `step()` would copy the
//! same 8192 bytes, leave `BC` at zero and `HL`/`DE` in the same places, and satisfy any gate
//! written around the destination block. The interrupt would simply be taken later — after the
//! instruction rather than inside it — and by then the ULA's 32-T-state window is long gone,
//! so the machine would silently drop **two whole frames' interrupts** and a game relying on
//! them would stutter with nothing failing.
//!
//! What separates the two is not the destination but the **acceptances**: how many there were,
//! which iteration each landed on, what `BC` still held, and what address was pushed. Those
//! are what [`EXPECTED_ACCEPTANCES`] pins, and they are the discriminating half of the file.
//! [`the_copy_is_exact_and_the_interrupts_cost_only_their_own_time`] then adds the other
//! direction: the same run with interrupts disabled must reach the identical end state having
//! spent exactly the two handlers less, so no iteration was lost, repeated or re-priced around
//! an acceptance.
//!
//! ## The blindness was measured rather than asserted
//!
//! `crates/z80/src/instructions.rs`'s transfer arm was mutated to run the whole loop inside one
//! `dispatch` — `while repeats(opcode) && outcome.repeat { … }` in place of the `PC -= 2`
//! rewind — with the landing proven and the file restored from held bytes. It reddens **15
//! tests across the workspace**, three of them here. A scratch gate asserting *only* the end
//! state — the 8192 copied bytes, the four guards, `BC`, `HL`, `DE` and `PC` — was run
//! alongside and **stayed green**, which is the measurement: written that way, this file would
//! have been decoration. What reddens is the acceptance count, the iteration each landed on,
//! the pushed address and the total.
//!
//! Two further mutations bound what the rest of the file grades, each landed and restored the
//! same way:
//!
//! | Mutation | Workspace | Caught here by |
//! |---|---|---|
//! | `LDIR` runs its whole loop inside one `step()` | RED 15 | all three behavioural gates |
//! | The interrupt offered *after* the instruction rather than before | RED 6 | the acceptance points |
//! | `acknowledge` no longer increments `R` | RED 4 | the refresh count |
//!
//! The last is the one worth naming: `2 * 8192 = 16384` is a whole number of seven-bit wraps,
//! so `R` would come back to exactly where it started were it not for the three M1 cycles each
//! interrupt adds. The six is the entire visible trace of an interrupt acknowledge being an M1
//! cycle at all, and it is the only place in this crate that observes it.
//!
//! # How the expected values were obtained
//!
//! Every figure was derived from the published Z80 and ULA figures **before** the emulator was
//! measured, and never by adjusting an observed one. The machine-cycle streams were recorded
//! off a real `Cpu` with a recording bus rather than read off the source —
//!
//! ```text
//!   LDIR, repeating   M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x7      21
//!   LDIR, exit        M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x2      16
//!   IM 2 acknowledge  IC@ir:1 x7  MW@sp-1:3  MW@sp-2:3  MR@vec:3  MR@vec+1:3   19
//!   EI                M1@pc:4                                                   4
//!   RET               M1@pc:4  MR@sp:3  MR@sp+1:3                              10
//! ```
//!
//! — and the acceptance points were then computed by a second implementation of the frame
//! clock and the acceptance rule, written with no sight of `crates/spectrum`. The recorder was
//! validated against a known answer first: `INC (HL)` must decompose as
//! `pc:4, hl:3, hl:1, hl:3` and cost 26 at contention phase 0 and 19 at phase 7, and it does.
//!
//! # Everything here runs uncontended, deliberately
//!
//! Source, destination, code, handler and vector are all in banks a 48K never contends, so
//! every pass costs exactly its published 21 or 16 T-states and the iteration an interrupt
//! lands on is **arithmetic** rather than a simulation of the contention model that
//! `block_contention.rs` exists to grade. Mixing the two would make this file's numbers depend
//! on that one's subject.
//!
//! # What is not graded here
//!
//! - **An interrupt arriving mid-loop while the loop is being contended.** The acceptance
//!   point would then depend on the ULA pattern as well as on the frame clock, and deriving it
//!   by hand across 3,326 iterations that each open on a different column is not tractable.
//!   That is the shape M7 will want, because a 128 contends banks in whichever slot they are
//!   paged into.
//! - **The compare, input and output families.** `CPIR`, `INIR` and `OTIR` share one
//!   `repeat_block` with the transfers, which is the *by construction* argument this project
//!   distrusts on principle — recorded here rather than relied on. `block_contention.rs` drives
//!   all eight forms for timing; only the transfers are interrupted.
//! - **`NMI` mid-loop.** It is always accepted and takes a different path.
//! - **A handler that does not return**, or one that itself runs a block instruction.
//! - **Whether 32 T-states is the right window**, or 14335 the right phase. No oracle;
//!   `docs/STATUS.md` lists both. `frame_boundary.rs` grades the window's *edge* against
//!   itself — 31 accepted, 32 missed — which pins it against drift and does not establish it.

mod common;

use common::{
    PROLOGUE, advance_to, elapsed, enable_interrupts, machine, set_pc, with_cpu_state,
    write_program,
};
use spectrum::Spectrum;
use spectrum::timing::T_STATES_PER_FRAME;
use z80::InterruptMode;

// ---------------------------------------------------------------------------
// The published costs, written here as expectations
// ---------------------------------------------------------------------------

/// T-states a repeating pass of a transfer block instruction costs, uncontended.
const REPEATING_PASS: u64 = 21;

/// T-states the pass that exhausts `BC` costs.
const EXIT_PASS: u64 = 16;

/// T-states a mode 2 interrupt acknowledge costs.
///
/// Seven of stretched M1 on the refresh address, two write cycles for the return address and
/// two read cycles for the vector: `7 + 3 + 3 + 3 + 3`.
const ACKNOWLEDGE: u64 = 19;

/// T-states the handler costs: `EI` and `RET`.
const HANDLER_T_STATES: u64 = 4 + 10;

/// The whole cost of one accepted interrupt, from the offer to the resumed loop.
const INTERRUPT_COST: u64 = ACKNOWLEDGE + HANDLER_T_STATES;

// ---------------------------------------------------------------------------
// Where everything lives
// ---------------------------------------------------------------------------

/// Bytes the block instruction moves.
///
/// 8192 passes cost `8191 * 21 + 16 = 172_027` T-states, which spans two frame boundaries —
/// so the run takes **two** interrupts rather than one, and a second acceptance is what shows
/// the first one did not merely happen to work.
const BLOCK_LEN: usize = 8192;

/// `BC` on entry.
const BC_START: u16 = BLOCK_LEN as u16;

/// The source block, in slot 2. Uncontended, and clear of the positioning prologue.
const SOURCE: u16 = 0x9000;

/// The destination block, spanning the top of slot 2 into slot 3. Also uncontended.
const DESTINATION: u16 = 0xB800;

/// The block instruction under test, at the top of RAM and clear of both blocks.
const CODE: u16 = 0xFF00;

/// The interrupt handler: `EI`, `RET`.
///
/// `EI` rather than a bare `RET` because the loop has to survive **more than one** interrupt,
/// and acceptance clears both flip-flops. Its one-instruction deferral is also load-bearing:
/// the acknowledge and `EI` together are 23 T-states, so without the deferral the `RET` would
/// still be inside the 32-T-state window on the first acceptance and would take a second
/// interrupt before returning.
const HANDLER: u16 = 0x8100;
const HANDLER_CODE: [u8; 2] = [0xFB, 0xC9];

/// `I`, and the vector pointer it forms with a floating `0xFF` on the data bus.
const VECTOR_HIGH: u8 = 0x82;
const VECTOR_POINTER: u16 = 0x82FF;

/// Where the stack starts, so the pushed return address has a known home.
///
/// [`enable_interrupts`] sets this; it is repeated as a constant because the two bytes below
/// it are read back to check *what* was pushed.
const STACK_TOP: u16 = 0xFF00;

/// `R` on entry, with bit 7 set.
///
/// The refresh counter is seven bits wide with bit 7 a latch only `LD R,A` moves, so pinning
/// it here buys a second property for free — and pinning it is necessary rather than tidy,
/// since [`advance_to`] runs a prologue of hundreds of instructions and `R` arrives at
/// whatever that left behind.
const REFRESH_WITH_THE_LATCH_SET: u8 = 0x80;

/// A byte written on either side of both blocks, and asserted to survive.
///
/// A copy that ran one iteration long or one short around an acceptance would step onto one of
/// these, and would otherwise be invisible: the destination block itself is identical whether
/// the loop was interrupted or not.
const GUARD: u8 = 0x5A;

/// Where the machine is positioned before the block instruction starts.
///
/// 42 T-states into frame zero: past the interrupt window, so the run starts with the line
/// already low and released, and composable by [`advance_to`] as two `LD A,0`s and seven
/// `NOP`s. It is also a multiple of 21, which is what makes the first acceptance land on
/// **exactly** the top of frame one — see [`EXPECTED_ACCEPTANCES`].
const START: u32 = 42;

/// Steps to allow before concluding the loop is not going to finish.
const STEP_BUDGET: usize = 20_000;

// ---------------------------------------------------------------------------
// The two directions
// ---------------------------------------------------------------------------

/// One member of the transfer family: which way the pointers walk.
///
/// Both are driven against the same expectations, and that is a measurement rather than an
/// assumption: `block_contention.rs` recorded the two streams and compared them address by
/// address, and on a single pass they are identical — the step direction only changes where
/// the pointers land afterwards. So the acceptance arithmetic is shared and what the twin adds
/// is that the rewind, the resume and the copy are not direction-specific.
struct Direction {
    name: &'static str,
    opcode: [u8; 2],
    descending: bool,
}

static DIRECTIONS: [Direction; 2] = [
    Direction {
        name: "LDIR",
        opcode: [0xED, 0xB0],
        descending: false,
    },
    Direction {
        name: "LDDR",
        opcode: [0xED, 0xB8],
        descending: true,
    },
];

impl Direction {
    /// `HL` on entry.
    fn hl(&self) -> u16 {
        if self.descending {
            SOURCE + BC_START - 1
        } else {
            SOURCE
        }
    }

    /// `DE` on entry.
    fn de(&self) -> u16 {
        if self.descending {
            DESTINATION + BC_START - 1
        } else {
            DESTINATION
        }
    }

    /// Where a pointer that started at `base` stands after `passes` iterations.
    fn after(&self, base: u16, passes: u16) -> u16 {
        if self.descending {
            base - passes
        } else {
            base + passes
        }
    }
}

// ---------------------------------------------------------------------------
// What the interrupts must do
// ---------------------------------------------------------------------------

/// One accepted interrupt: the frame it landed in, how far into it, and what `BC` still held.
#[derive(Debug, PartialEq, Eq)]
struct Acceptance {
    frame: u64,
    offset: u32,
    remaining: u16,
}

/// The two acceptances, derived from the published costs alone.
///
/// ```text
///   A frame is 69888 T-states and a repeating pass is 21, and 69888 = 21 x 3328 exactly.
///   From offset 42 the passes therefore land on 42, 63, 84, ... and the pass that ends
///   the frame is the 3326th:
///
///       42 + 3326 * 21 = 69888              frame 1, offset 0
///
///   Offset 0 is inside the ULA's 32-T-state window, so the next step is offered the
///   interrupt and accepts it. Nothing is half-done: the pass that just ended wrote its
///   byte, moved both pointers, decremented BC and rewound PC onto its own first byte.
///
///       acceptance #0                       BC = 8192 - 3326 = 4866
///
///   Acknowledge, EI and RET are 19 + 4 + 10 = 33, so the loop resumes at offset 33. The
///   next boundary is 2 x 69888 = 139776, which is 69855 away, and 69855 / 21 = 3326.4 —
///   so it takes 3327 passes to cross, and the machine lands 12 T-states past the top
///   rather than exactly on it:
///
///       69921 + 3327 * 21 = 139788          frame 2, offset 12
///       acceptance #1                       BC = 4866 - 3327 = 1539
///
///   Another 33, then the remaining 1539 passes — 1538 repeating and one exit — cost
///   1538 * 21 + 16 = 32314, and the instruction ends at 172135: frame 2, offset 32359.
///   The third boundary is at 209664 and is never reached.
/// ```
///
/// The two rows are deliberately different shapes. The first lands **exactly** on a frame
/// boundary, where the offer is made at offset zero; the second lands twelve T-states past
/// one, which is where a machine that reset its clock at the boundary rather than carrying the
/// overshoot would start to disagree.
static EXPECTED_ACCEPTANCES: [Acceptance; 2] = [
    Acceptance {
        frame: 1,
        offset: 0,
        remaining: 4866,
    },
    Acceptance {
        frame: 2,
        offset: 12,
        remaining: 1539,
    },
];

/// T-states the whole instruction costs with no interrupt at all.
///
/// `8191 * 21 + 16`.
const UNINTERRUPTED: u64 = (BC_START as u64 - 1) * REPEATING_PASS + EXIT_PASS;

/// And with the two interrupts, which cost exactly their own time and nothing else.
const INTERRUPTED: u64 = UNINTERRUPTED + 2 * INTERRUPT_COST;

/// Where the machine stands when an uninterrupted run finishes: `42 + 172_027`.
const UNINTERRUPTED_END: (u64, u32) = (2, 32_293);

/// And an interrupted one: `42 + 172_093`.
const INTERRUPTED_END: (u64, u32) = (2, 32_359);

/// M1 cycles the interrupted run performs.
///
/// Two per pass — the instruction re-fetches both of its own opcode bytes every iteration,
/// which is what the rewind means — plus three per interrupt: the acknowledge, which
/// **refreshes without fetching**, and the handler's `EI` and `RET`.
///
/// `2 * 8192 = 16384` is a whole number of seven-bit wraps, so without the three refreshes per
/// interrupt `R` would come back to exactly where it started. It is `16390` instead, and the
/// six is the entire visible trace of the acknowledge being an M1 cycle at all.
const M1_CYCLES: u32 = 2 * BC_START as u32 + 3 * EXPECTED_ACCEPTANCES.len() as u32;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The block to copy: a period-251 ramp, so any shift of the copy is visible everywhere.
fn source_bytes() -> Vec<u8> {
    (0..BLOCK_LEN).map(|i| (i % 251) as u8).collect()
}

/// A machine at [`START`], loaded with `direction`'s opcode, the blocks and the guards.
///
/// `interrupts` decides whether the flip-flops are set; everything else is identical between
/// the two, which is what makes the difference between their costs attributable to the
/// interrupts and to nothing else.
fn loaded(direction: &Direction, interrupts: bool) -> Spectrum {
    let mut machine = machine();
    advance_to(&mut machine, START);

    write_program(&mut machine, SOURCE, &source_bytes());
    write_program(&mut machine, DESTINATION, &vec![0; BLOCK_LEN]);
    for guard in guards() {
        write_program(&mut machine, guard, &[GUARD]);
    }
    write_program(&mut machine, CODE, &direction.opcode);
    write_program(&mut machine, HANDLER, &HANDLER_CODE);
    write_program(&mut machine, VECTOR_POINTER, &HANDLER.to_le_bytes());

    if interrupts {
        enable_interrupts(&mut machine, InterruptMode::Mode2);
    }
    with_cpu_state(&mut machine, |state| {
        state.hl = direction.hl();
        state.de = direction.de();
        state.bc = BC_START;
        state.i = VECTOR_HIGH;
        state.r = REFRESH_WITH_THE_LATCH_SET;
        state.sp = STACK_TOP;
    });
    set_pc(&mut machine, CODE);
    machine
}

/// The four bytes that must survive: one on each side of each block.
fn guards() -> [u16; 4] {
    [
        SOURCE - 1,
        SOURCE + BC_START,
        DESTINATION - 1,
        DESTINATION + BC_START,
    ]
}

/// What one run produced.
struct Run {
    acceptances: Vec<Acceptance>,
    return_addresses: Vec<u16>,
    accepted_t_states: Vec<u32>,
    cost: u64,
    end: (u64, u32),
}

/// Step until the instruction finishes, recording every interrupt it takes on the way.
///
/// An acceptance is a step after which `PC` is the handler: [`z80::Cpu::interrupt`] executes
/// no instruction, so the register file is exactly as the last completed iteration left it —
/// which is precisely the state this file is about.
fn run(machine: &mut Spectrum) -> Run {
    let before = elapsed(machine);
    let mut acceptances = Vec::new();
    let mut return_addresses = Vec::new();
    let mut accepted_t_states = Vec::new();

    for _ in 0..STEP_BUDGET {
        // Where the offer is made, taken before the step rather than reconstructed from where
        // the acknowledge left the clock.
        let offered_at = (machine.frames(), machine.frame_t_state());
        let charged = machine.step();
        let state = machine.cpu_state();
        if state.pc == HANDLER {
            acceptances.push(Acceptance {
                frame: offered_at.0,
                offset: offered_at.1,
                remaining: state.bc,
            });
            return_addresses.push(u16::from_le_bytes([
                machine.memory().read(state.sp),
                machine.memory().read(state.sp + 1),
            ]));
            accepted_t_states.push(charged);
        }
        if state.pc == CODE + 2 && state.bc == 0 {
            return Run {
                acceptances,
                return_addresses,
                accepted_t_states,
                cost: elapsed(machine) - before,
                end: (machine.frames(), machine.frame_t_state()),
            };
        }
    }
    panic!("the block instruction did not finish within {STEP_BUDGET} steps");
}

/// The first address at which the destination differs from what was asked for.
fn first_difference(machine: &Spectrum, at: u16, expected: &[u8]) -> Option<(u16, u8, u8)> {
    expected.iter().enumerate().find_map(|(offset, want)| {
        let address = at + u16::try_from(offset).expect("a block inside the address space");
        let got = machine.memory().read(address);
        (got != *want).then_some((address, *want, got))
    })
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

#[test]
fn the_interrupt_is_accepted_between_iterations_with_the_loop_intact() {
    // The discriminating gate. A core that ran the whole instruction inside one `step()`
    // would copy the same bytes and end in the same registers; what it could not do is push
    // the instruction's own address, twice, with `BC` still holding a derived non-zero count.
    for direction in &DIRECTIONS {
        let mut machine = loaded(direction, true);
        let outcome = run(&mut machine);

        assert_eq!(
            outcome.acceptances.len(),
            EXPECTED_ACCEPTANCES.len(),
            "{}: a run spanning two frame boundaries must take two interrupts, one per \
             boundary. It took {}",
            direction.name,
            outcome.acceptances.len()
        );

        for (index, expected) in EXPECTED_ACCEPTANCES.iter().enumerate() {
            let got = &outcome.acceptances[index];
            assert_eq!(
                got,
                expected,
                "{}: acceptance #{index} must land in frame {} at offset {} with BC still \
                 holding {} — {} of the {BC_START} iterations complete, and the rest still to \
                 run. BC at zero would mean the interrupt was taken after the whole \
                 instruction rather than inside it",
                direction.name,
                expected.frame,
                expected.offset,
                expected.remaining,
                BC_START - expected.remaining
            );
            assert_eq!(
                outcome.return_addresses[index],
                CODE,
                "{}: acceptance #{index} must push the instruction's **own** address. \
                 {:#06X} is where the interrupt would return to if the loop had exited",
                direction.name,
                CODE + 2
            );
            assert_eq!(
                u64::from(outcome.accepted_t_states[index]),
                ACKNOWLEDGE,
                "{}: a mode 2 acknowledge is {ACKNOWLEDGE} T-states",
                direction.name
            );
        }
    }
}

#[test]
fn the_loop_resumes_after_the_handler_and_runs_to_completion() {
    for direction in &DIRECTIONS {
        let mut machine = loaded(direction, true);
        let outcome = run(&mut machine);
        let state = machine.cpu_state();

        assert_eq!(
            state.bc, 0,
            "{}: the counter must be exhausted",
            direction.name
        );
        assert_eq!(
            state.pc,
            CODE + 2,
            "{}: only the pass that exhausts BC steps past the instruction",
            direction.name
        );
        assert_eq!(
            state.hl,
            direction.after(direction.hl(), BC_START),
            "{}: HL must have walked the whole block",
            direction.name
        );
        assert_eq!(
            state.de,
            direction.after(direction.de(), BC_START),
            "{}: and so must DE",
            direction.name
        );
        assert!(
            state.iff1,
            "{}: the handler's EI must have left interrupts enabled",
            direction.name
        );
        assert_eq!(
            state.sp, STACK_TOP,
            "{}: every acceptance's return address must have been popped again",
            direction.name
        );
        assert_eq!(
            outcome.end, INTERRUPTED_END,
            "{}: the run must end in frame {} at offset {}",
            direction.name, INTERRUPTED_END.0, INTERRUPTED_END.1
        );

        assert_eq!(
            state.r,
            REFRESH_WITH_THE_LATCH_SET
                | u8::try_from(M1_CYCLES % 128).expect("a seven-bit refresh counter"),
            "{}: R counts M1 cycles — two per pass, and one more per interrupt for the \
             acknowledge, which refreshes without fetching, plus the handler's two. {M1_CYCLES} \
             in all through seven bits, leaving the latch in bit 7 where it started",
            direction.name
        );
    }
}

#[test]
fn the_copy_is_exact_and_the_interrupts_cost_only_their_own_time() {
    // The other direction, and the one that rules out an iteration being lost, repeated or
    // re-priced around an acceptance: the same run with the flip-flops clear must reach the
    // identical end state having spent exactly two handlers less.
    let expected = source_bytes();

    for direction in &DIRECTIONS {
        let mut interrupted = loaded(direction, true);
        let with_interrupts = run(&mut interrupted);

        let mut quiet = loaded(direction, false);
        let without = run(&mut quiet);

        assert!(
            without.acceptances.is_empty(),
            "{}: the control must take no interrupts at all",
            direction.name
        );
        assert_eq!(
            without.cost,
            UNINTERRUPTED,
            "{}: {} passes at {REPEATING_PASS} T-states and one at {EXIT_PASS} is \
             {UNINTERRUPTED}",
            direction.name,
            BC_START - 1
        );
        assert_eq!(
            without.end, UNINTERRUPTED_END,
            "{}: and it must end in frame {} at offset {}",
            direction.name, UNINTERRUPTED_END.0, UNINTERRUPTED_END.1
        );
        assert_eq!(
            with_interrupts.cost,
            INTERRUPTED,
            "{}: two interrupts must cost exactly {} T-states more — {ACKNOWLEDGE} of \
             acknowledge and {HANDLER_T_STATES} of handler each — and no iteration may be \
             lost or repeated around them",
            direction.name,
            2 * INTERRUPT_COST
        );
        assert_eq!(
            with_interrupts.cost - without.cost,
            2 * INTERRUPT_COST,
            "{}: stated as the difference, which is the form that survives both figures \
             being wrong in the same direction",
            direction.name
        );

        for (label, machine) in [("interrupted", &interrupted), ("quiet", &quiet)] {
            assert_eq!(
                first_difference(machine, DESTINATION, &expected),
                None,
                "{}, {label}: every one of the {BLOCK_LEN} bytes must be copied exactly once, \
                 in order",
                direction.name
            );
            for guard in guards() {
                assert_eq!(
                    machine.memory().read(guard),
                    GUARD,
                    "{}, {label}: the byte at {guard:#06X} is outside both blocks and must be \
                     untouched — a loop that ran one pass long or short around an acceptance \
                     would step onto it",
                    direction.name
                );
            }
        }
    }
}

#[test]
fn the_positioning_and_the_layout_are_what_the_derivation_assumes() {
    // The control for the arithmetic above rather than for the machine. Every figure in
    // `EXPECTED_ACCEPTANCES` rests on four facts that are cheap to state and would otherwise
    // be silent assumptions: where the run starts, that a frame is a whole number of repeating
    // passes, that the blocks and the code are in banks a 48K never contends, and that nothing
    // in the layout overlaps.
    //
    // None of the mutations run against this file reddens it, and that is by construction
    // rather than an oversight: its subject is this file's own premises, not the emulator. Its
    // reachable failing cases are a change to the contended range, to the frame length, or to
    // any of the five addresses above — each of which would silently move every acceptance in
    // the table rather than fail anything.
    assert_eq!(
        u64::from(T_STATES_PER_FRAME) % REPEATING_PASS,
        0,
        "a frame is {} repeating passes exactly, which is why the first acceptance lands on \
         offset zero",
        u64::from(T_STATES_PER_FRAME) / REPEATING_PASS
    );
    assert_eq!(
        u64::from(START) % REPEATING_PASS,
        0,
        "and the start position is a whole number of them past the top of the frame"
    );

    let mut machine = machine();
    advance_to(&mut machine, START);
    assert_eq!(elapsed(&machine), u64::from(START));

    // Uncontended, asserted through the machine rather than by reading off an address range:
    // one NOP from each region must cost its nominal four T-states at a position where the
    // ULA is fetching, which is the only thing that makes "uncontended" mean anything.
    for region in [SOURCE, DESTINATION, CODE, HANDLER, PROLOGUE] {
        assert!(
            !machine.memory().is_contended(region),
            "{region:#06X} must be in a bank a 48K never contends, or the passes do not cost \
             a flat {REPEATING_PASS} and every acceptance moves"
        );
    }

    // No two of the five regions may overlap, and the guards must sit outside both blocks.
    let mut spans = [
        (SOURCE, BC_START),
        (DESTINATION, BC_START),
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
