//! Gate: an `IN` or an `OUT` executed by a real CPU is stalled by the four-case ULA rule.
//!
//! # Why this exists
//!
//! `crates/spectrum/src/ula.rs` unit-tests the four-case pattern directly, and
//! `docs/STATUS.md` listed the consequence in its *Still ungraded* deliverable:
//!
//! > **I/O contention is graded against a hand-written tick stream**, not through a real
//! > `Cpu<Ula>`. The four-case rule reads the clock rather than the cycle, so it was never
//! > part of the retired heuristic — but it is the one contention path still verified
//! > against a synthesised input.
//!
//! That is the same defect in a different place as the one M5 already caught twice. The
//! keyboard matrix was graded against a port derived from the map under test, and the
//! machine-cycle constants were graded against a stream this crate wrote — in both cases the
//! expectation and the subject came from one source, and in both cases the way out was to
//! obtain the other side from somewhere the code under test does not reach. Here that is the
//! CPU: it decides how many machine cycles precede the port cycle, and therefore **where in
//! the frame the port cycle opens**, and it decides **which 16 bits go on the address bus**,
//! and therefore which of the four cases applies at all. A hand-written stream supplies both
//! answers itself.
//!
//! # What is graded here
//!
//! Three instruction forms x four port cases x eight phases, driven through a real
//! `Cpu<Ula>` and asserted against costs derived by hand from the published rule.
//!
//! The four cases are the cross product of two independent properties, which is the point of
//! the rule being four cases rather than one: the ULA contends because the port **address**
//! happens to lie in contended memory, *and* separately because the ULA **owns** the port
//! and answers it. Neither implies the other, and the port `0x4001` — in the contended range
//! but with bit 0 set, so not the ULA's — is the case that proves it.
//!
//! The port is never handed to the bus directly. It is formed the way the hardware forms it:
//! `IN A,(n)` and `OUT (n),A` put `A` on the high half and the immediate byte on the low
//! half, and `IN A,(C)` puts the whole of `BC` on the bus. So a machine that decoded the
//! port from the wrong half would select the wrong case and fail here, while a test that
//! passed `0x4000` to `in_port` itself could not see it.
//!
//! # What is not graded here
//!
//! - **Whether the published four-case pattern is right.** It is the emulator community's
//!   figure for an issue 3 48K and this project has no oracle for it, exactly as for the
//!   memory pattern — see `contention_magnitude.rs`.
//! - **The phase.** Every position here is relative to
//!   [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE], so every
//!   assertion survives that constant being wrong. That is `contention_phase.rs`.
//! - **`OUT (C),r`.** `IN A,(C)` covers the `ED`-page shape — two M1 fetches then the port
//!   cycle — and nothing here reaches the `OUT` half of it.
//! - **The `ED` block I/O forms** (`INI`/`OUTI`/`INIR`/`OTIR` and their twins), which add
//!   internal cycles and a repeat. They are `block_contention.rs`, where the port cycle is
//!   priced by this file's four-case rule as one term of a longer walk — and where the
//!   output family's port turns out to be `BC` *after* `B`'s decrement, so a chosen `B` can
//!   move the port cycle from a contended address to a free one.
//! - **What the port *returns*.** `keyboard_matrix.rs` grades the value; this file grades
//!   only the time.

mod common;

use common::{
    UNCONTENDED_CODE, advance_to, cost_of_running, machine, with_cpu_state, write_program,
};
use spectrum::timing::FIRST_CONTENDED_T_STATE;

/// T-states an I/O machine cycle occupies once its stall is paid.
///
/// The published Z80 figure, written here as the expectation rather than read from the
/// crate. `crates/z80` now exports [`PORT_ACCESS_T_STATES`][z80::PORT_ACCESS_T_STATES] and
/// `crates/spectrum/src/ula.rs` consumes it, so the CPU and the machine share one
/// definition — which is exactly why this file must **not** import it. A single source
/// removes the risk of two implementations disagreeing; it does nothing about both being
/// wrong, and an expectation taken from an implementation agrees with any implementation.
const PORT_CYCLE: u64 = 4;

/// How the instruction puts sixteen bits of port address on the bus.
#[derive(Clone, Copy)]
enum PortSource {
    /// `A` in the high half, the instruction's immediate byte in the low half.
    AccumulatorAndImmediate,
    /// The whole of `BC`.
    RegisterPair,
}

/// One `IN`/`OUT` form, and where in its own stream the port cycle falls.
struct Form {
    name: &'static str,
    /// The instruction, less the immediate byte the caller appends for
    /// [`PortSource::AccumulatorAndImmediate`].
    opcode: &'static [u8],
    source: PortSource,
    /// T-states of the machine cycles that precede the port cycle.
    ///
    /// Not assumed: recorded from a real `Cpu` through a bus that logs every transfer and
    /// tick. `IN A,(n)` is `pc:4, pc+1:3, port:4` — an M1 fetch and the immediate byte's
    /// own three-T-state read. `IN A,(C)` is `pc:4, pc+1:4, port:4`, because the `ED`
    /// page's opcode byte is **its own M1 cycle** and not an operand read. Those two are
    /// seven and eight, and the difference is exactly what a hand-written stream would have
    /// had to guess.
    before_port: u32,
}

const FORMS: [Form; 3] = [
    Form {
        name: "IN A,(n)",
        opcode: &[0xDB],
        source: PortSource::AccumulatorAndImmediate,
        before_port: 7,
    },
    Form {
        name: "OUT (n),A",
        opcode: &[0xD3],
        source: PortSource::AccumulatorAndImmediate,
        before_port: 7,
    },
    Form {
        name: "IN A,(C)",
        opcode: &[0xED, 0x78],
        source: PortSource::RegisterPair,
        before_port: 8,
    },
];

/// One of the four cases, and the stall it costs at each phase of the ULA's group.
///
/// **Derived by hand from the published table, not read from the crate.** The pattern is
/// `[6, 5, 4, 3, 2, 1, 0, 0]` by position within an eight-T-state group, `C` charges it and
/// `N` does not, and — the part that makes this arithmetic rather than a lookup — **each
/// stall shifts the ones after it**, so every later term is priced at a position that
/// already includes the T-states spent so far.
///
/// Writing `p` for the group position the port cycle opens at and `D` for the pattern:
///
/// | case | port | rule | stall |
/// |---|---|---|---|
/// | address free, not the ULA's | `0x8001` | `N:4` | `0` |
/// | address free, the ULA's | `0x8000` | `N:1, C:3` | `D[p+1]` |
/// | address contended, the ULA's | `0x4000` | `C:1, C:3` | `a = D[p]`, then `D[p+a+1]` |
/// | address contended, not the ULA's | `0x4001` | `C:1, C:1, C:1, C:1` | `a = D[p]`, `b = D[p+a+1]`, `c = D[p+a+b+2]`, `d = D[p+a+b+c+3]` |
///
/// Worked at `p = 0`, which is where the four are most clearly distinct:
///
/// ```text
///   0x8001   nothing is charged                                              0
///   0x8000   D[1] = 5                                                        5
///   0x4000   a = D[0] = 6; D[0+6+1] = D[7] = 0                               6
///   0x4001   a = D[0] = 6; b = D[7] = 0; c = D[8] = 6; d = D[15] = 0        12
/// ```
///
/// and at `p = 7`, where the first term falls in the pattern's two zero slots and the
/// charge lands on the *next* group instead:
///
/// ```text
///   0x8000   D[8] = 6                                                        6
///   0x4000   a = D[7] = 0; D[7+0+1] = D[8] = 6                               6
///   0x4001   a = 0; b = D[8] = 6; c = D[15] = 0; d = D[16] = 6              12
/// ```
///
/// The rows below carry all eight positions. Two properties of the table are worth reading
/// off it rather than rediscovering: the `0x4000` row is the delay pattern itself, because
/// the second `C` always lands in a zero slot except at `p = 7`; and the `0x4001` row is
/// `12, 11, ... 6` and then jumps back to `12`, because four accumulating charges walk
/// exactly two groups.
struct PortCase {
    port: u16,
    rule: &'static str,
    stall_by_phase: [u64; 8],
}

const PORT_CASES: [PortCase; 4] = [
    PortCase {
        port: 0x8001,
        rule: "N:4",
        stall_by_phase: [0, 0, 0, 0, 0, 0, 0, 0],
    },
    PortCase {
        port: 0x8000,
        rule: "N:1, C:3",
        stall_by_phase: [5, 4, 3, 2, 1, 0, 0, 6],
    },
    PortCase {
        port: 0x4000,
        rule: "C:1, C:3",
        stall_by_phase: [6, 5, 4, 3, 2, 1, 0, 6],
    },
    PortCase {
        port: 0x4001,
        rule: "C:1, C:1, C:1, C:1",
        stall_by_phase: [12, 11, 10, 9, 8, 7, 6, 12],
    },
];

/// Assemble `form` addressing `port`, and set the registers the port is formed from.
///
/// Called **after** `advance_to`: the clock-positioning prologue executes `LD A,0`, so a
/// value put in `A` before it would not survive.
fn arm(machine: &mut spectrum::Spectrum, form: &Form, port: u16) {
    let mut program = form.opcode.to_vec();
    match form.source {
        PortSource::AccumulatorAndImmediate => {
            program.push((port & 0x00FF) as u8);
            with_cpu_state(machine, |state| state.af = port & 0xFF00);
        }
        PortSource::RegisterPair => with_cpu_state(machine, |state| state.bc = port),
    }
    write_program(machine, UNCONTENDED_CODE, &program);
}

/// Run one instruction of `form` at `port`, with its **port cycle** opening at `phase`.
///
/// The code is in bank 0, so the cycles before the port cycle cost exactly their nominal
/// length and the landing position is arithmetic. Positioning the *port cycle* rather than
/// the instruction is what lets one expectation table serve all three forms, whose port
/// cycles open seven and eight T-states in.
fn cost_at_phase(form: &Form, port: u16, phase: u32) -> u64 {
    let mut machine = machine();
    advance_to(
        &mut machine,
        FIRST_CONTENDED_T_STATE + phase - form.before_port,
    );
    arm(&mut machine, form, port);
    cost_of_running(&mut machine, UNCONTENDED_CODE, 1)
}

#[test]
fn a_real_in_or_out_is_stalled_by_the_four_case_rule_at_every_phase() {
    for form in &FORMS {
        for case in &PORT_CASES {
            for phase in 0..8 {
                let stall = case.stall_by_phase[phase as usize];
                let expected = u64::from(form.before_port) + stall + PORT_CYCLE;
                assert_eq!(
                    cost_at_phase(form, case.port, phase),
                    expected,
                    "{} to port {:#06X} ({}), port cycle opening at phase +{phase}, must \
                     cost {} nominal plus a {stall} T-state stall",
                    form.name,
                    case.port,
                    case.rule,
                    u64::from(form.before_port) + PORT_CYCLE
                );
            }
        }
    }
}

#[test]
fn the_four_cases_are_distinct_and_neither_property_implies_the_other() {
    // The reason the rule has four rows. Being in contended memory and being the ULA's port
    // are independent, and at phase 0 all four land on different numbers — so a machine that
    // collapsed the rule to "the ULA contends its own port" (0x4001 would cost 0) or to "the
    // ULA contends contended addresses" (0x4000 and 0x4001 would agree) fails here.
    const PHASE: u32 = 0;
    let form = &FORMS[0];

    let costs: Vec<u64> = PORT_CASES
        .iter()
        .map(|case| cost_at_phase(form, case.port, PHASE))
        .collect();

    assert_eq!(
        costs,
        vec![11, 16, 17, 23],
        "at phase 0 the four cases cost 7 nominal + 4 port + a stall of 0, 5, 6 and 12"
    );

    let mut distinct = costs.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        PORT_CASES.len(),
        "two of the four cases collapsed onto one cost: {costs:?}"
    );
}

#[test]
fn a_port_cycle_outside_the_fetch_window_costs_nothing_extra() {
    // The control. I/O contention is a property of *when*, exactly as memory contention is:
    // the same four ports, at a position the ULA is not fetching in, must all cost the
    // instruction's nominal length. A model that stalled on the port's bits alone would pass
    // every assertion above and fail this one.
    //
    // 200 T-states before the window is far enough that the four accumulating charges of the
    // worst case — at most 24 T-states — cannot reach it.
    const BEFORE_DISPLAY: u32 = 200;

    for form in &FORMS {
        for case in &PORT_CASES {
            let mut machine = machine();
            advance_to(&mut machine, FIRST_CONTENDED_T_STATE - BEFORE_DISPLAY);
            arm(&mut machine, form, case.port);

            assert_eq!(
                cost_of_running(&mut machine, UNCONTENDED_CODE, 1),
                u64::from(form.before_port) + PORT_CYCLE,
                "{} to port {:#06X} must be free while the ULA is in the top border",
                form.name,
                case.port
            );
        }
    }
}
