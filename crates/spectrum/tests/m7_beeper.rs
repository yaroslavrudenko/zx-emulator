//! Gate: the beeper — bit 4 of a `0xFE` write, driven by a guest executing real `OUT`s.
//!
//! # What is graded here
//!
//! `spectrum::audio`'s own unit tests grade the **rule**: that a sample is the weighted mean
//! of the window, that a short pulse survives attenuated, that setting the level it already
//! holds renders nothing. Every one of them calls `Audio::set_beeper` directly, and none of
//! them can say whether a program executing `OUT (0xFE),A` reaches it at all.
//!
//! That distinction is not academic, and it has a measured precedent one file over:
//! `m7_paging.rs` exists because `Ula::out_port`'s paging arm is a separate `if` and *"a
//! decode that never fired would leave every test in `memory.rs` green"*. The speaker's arm
//! is a third statement in the same function, and it was **absent for a whole milestone** —
//! `ula.rs:528` carried a comment calling itself *"an open finding, not a deferral"*. This
//! file is what closes it, and what would go red if it were reopened.
//!
//! # The expectation is derived before it is measured
//!
//! Everything below is a T-state count taken from the instruction lengths and the point in an
//! `OUT` at which the port cycle opens, **worked out in advance** and written into
//! [`EXPECTED_PERIOD`]. Nothing was run and then recorded — `docs/STATUS.md`'s standing rule
//! is that a number without its derivation is *"a claim wearing a measurement's clothes"*, and
//! its sharpest recorded instance is an agent adding a missing 5 T-state stall to an observed
//! 25 and reporting 30 where the answer was 26.
//!
//! # What is **not** graded here
//!
//! - **The beeper's amplitude, or its loudness against the AY.** A magnitude with no source;
//!   `spectrum::audio` says so and nothing here contradicts it.
//! - **The `0xFE` port's four output levels.** `MIC` shifting the speaker's level is not
//!   modelled — see `ula::MIC_BIT` — so this grades a two-level speaker, which is what the
//!   machine implements.
//! - **That it sounds like a Spectrum.** A human ear, `docs/M7.md`'s T4.
//! - **When in the frame the edges land relative to the display.** Nothing consults it.

mod common;
mod m7_common;

use common::{machine, with_cpu_state, write_program};
use m7_common::machine_128;
use spectrum::audio::{AMPLITUDE_MAX, SAMPLE_PERIOD_T_STATES};
use spectrum::{Sample, Spectrum};

/// Where the toggling program is assembled: bank 2 on a 48K, bank 0 on a 128 — uncontended on
/// both, so every instruction costs exactly its nominal length and the arithmetic below holds.
const PROGRAM: u16 = common::PROLOGUE;

/// `XOR n` — two bytes, seven T-states.
const XOR_N: u8 = 0xEE;

/// `LD B,n` and `LD C,n` — two bytes, seven T-states each, and pure padding here.
const LD_B_N: u8 = 0x06;
const LD_C_N: u8 = 0x0E;

/// `OUT (n),A` — two bytes, eleven T-states: an M1 fetch, an operand read, then the port
/// cycle. The port cycle is what calls `Bus::out_port`, and it opens **seven T-states into the
/// instruction**, which is the term the whole derivation below turns on.
const OUT_N_A: u8 = 0xD3;

/// T-states from the start of an `OUT (n),A` to the moment `Bus::out_port` is called.
const OUT_TO_PORT_CYCLE: u32 = 7;

/// The low half of the port an `OUT (n),A` names. `A` supplies the high half.
const ULA_PORT_LOW: u8 = 0xFE;

/// Bit 4 of a `0xFE` write: the speaker.
///
/// Written as a literal rather than imported, because it is the *expectation*. Taking it from
/// the crate would make this file agree with `ula.rs` by construction, which is the
/// keyboard-matrix tautology `docs/STATUS.md` records — *"a test whose expectation is computed
/// by the subject is not a weak test; it is a tautology"*.
const SPEAKER: u8 = 0x10;

/// The border bits of the same write, likewise transcribed rather than imported.
const BORDER: u8 = 0x07;

/// `NOP`, four T-states.
const NOP: u8 = 0x00;

/// `NOP`s that pad each block out to [`BLOCK_T_STATES`].
const PAD_NOPS: usize = 8;

/// Instructions in one block.
const BLOCK_STEPS: usize = 4 + PAD_NOPS;

/// T-states one block costs: `7 + 7 + 7 + 11 + 8 x 4`.
///
/// Chosen to be a whole number of sample periods so the sample stream is exactly periodic and
/// can be asserted by value rather than by a tolerance.
const BLOCK_T_STATES: u32 = 7 + 7 + 7 + 11 + PAD_NOPS as u32 * 4;

const _: () = assert!(BLOCK_T_STATES == 64);
const _: () = assert!(BLOCK_T_STATES.is_multiple_of(SAMPLE_PERIOD_T_STATES));

/// T-states from the start of a block to the edge it puts on the speaker.
///
/// Three seven-T-state instructions, then seven T-states into the `OUT`.
const EDGE_IN_BLOCK: u32 = 7 + 7 + 7 + OUT_TO_PORT_CYCLE;

const _: () = assert!(EDGE_IN_BLOCK == 28);

/// Samples one block produces.
const SAMPLES_PER_BLOCK: usize = (BLOCK_T_STATES / SAMPLE_PERIOD_T_STATES) as usize;

/// The beeper stream one full cycle of the program produces, **derived**.
///
/// Two blocks per cycle — one raising the speaker, one lowering it — so four samples, and each
/// one is the share of its 32 T-state window the speaker was high:
///
/// | sample | window | high during | share |
/// |---|---|---|---|
/// | 0 | `0..32` | `28..32` | 4/32 |
/// | 1 | `32..64` | all of it | 32/32 |
/// | 2 | `64..96` | `64..92` | 28/32 |
/// | 3 | `96..128` | none | 0 |
///
/// The first and third are the *same* edge seen from either side, which is why they sum to a
/// whole window and why a model that point-sampled instead of integrating would produce two
/// values from `{0, 65535}` here rather than these two.
const EXPECTED_PERIOD: [u16; 4] = [
    (AMPLITUDE_MAX as u32 * 4 / SAMPLE_PERIOD_T_STATES) as u16,
    AMPLITUDE_MAX,
    (AMPLITUDE_MAX as u32 * 28 / SAMPLE_PERIOD_T_STATES) as u16,
    0,
];

/// Blocks the program runs. Four full cycles of [`EXPECTED_PERIOD`].
const BLOCKS: usize = 8;

/// Assemble a program that flips `mask` in `A` and writes it to the ULA port, `BLOCKS` times.
///
/// `A` starts at zero, so a mask of [`SPEAKER`] alternates the speaker between low and high
/// and a mask of [`BORDER`] leaves the speaker at zero throughout — the positive case and its
/// negative control differ in **one byte** of the program.
fn toggling_program(mask: u8) -> Vec<u8> {
    let mut program = Vec::with_capacity(BLOCKS * (8 + PAD_NOPS));
    for _ in 0..BLOCKS {
        program.extend_from_slice(&[XOR_N, mask, LD_B_N, 0, LD_C_N, 0, OUT_N_A, ULA_PORT_LOW]);
        program.resize(program.len() + PAD_NOPS, NOP);
    }
    program
}

/// Run the program on `machine` from a standing start and hand back what it emitted.
///
/// The machine must be **fresh**: the derivation puts the first block at frame T-state zero,
/// so a machine that had already run would place every edge somewhere else and the expected
/// stream would be a different one that this file has not derived.
fn run_toggling(machine: &mut Spectrum, mask: u8) -> Vec<Sample> {
    assert_eq!(
        (machine.frames(), machine.frame_t_state()),
        (0, 0),
        "the derivation places the first edge at T-state {EDGE_IN_BLOCK} of frame zero"
    );
    // `A` decides what the first `XOR` produces, so it is set rather than inherited. This does
    // not move the clock: it is a register assignment, not a machine cycle.
    with_cpu_state(machine, |state| state.af = 0);
    write_program(machine, PROGRAM, &toggling_program(mask));
    common::set_pc(machine, PROGRAM);
    for _ in 0..BLOCKS * BLOCK_STEPS {
        machine.step();
    }
    assert_eq!(
        machine.frame_t_state(),
        BLOCKS as u32 * BLOCK_T_STATES,
        "the program must cost exactly what its instruction lengths say, or every edge \
         position below is measured against the wrong clock"
    );
    machine.take_samples().to_vec()
}

#[test]
fn a_guest_out_reaches_the_speaker_and_the_stream_is_the_derived_one() {
    // The wiring assertion **and** the timing one in a single comparison. It is stated by
    // value rather than as "the beeper varies", because a stream that varied at the wrong
    // rate, or with the edges on the wrong side of a window, would pass the weaker version.
    let mut machine = machine();
    let samples = run_toggling(&mut machine, SPEAKER);

    assert_eq!(samples.len(), BLOCKS * SAMPLES_PER_BLOCK);
    let beeper: Vec<u16> = samples.iter().map(|sample| sample.beeper).collect();
    let expected: Vec<u16> = EXPECTED_PERIOD
        .iter()
        .copied()
        .cycle()
        .take(beeper.len())
        .collect();
    assert_eq!(beeper, expected);
}

#[test]
fn writing_only_the_border_bits_leaves_the_speaker_where_it_was() {
    // **The negative control, and it is the half that makes the test above mean something.**
    // The two programs differ in one byte. Without this, a model that drove the speaker from
    // *any* write to `0xFE` — or from the wrong bit of it — would pass the positive case
    // exactly as well.
    let mut machine = machine();
    let samples = run_toggling(&mut machine, BORDER);

    assert!(
        samples.iter().all(|sample| sample.beeper == 0),
        "a border-only program must produce silence"
    );
    // And the writes really did happen, so this is not a program that did nothing: the border
    // ends on the value the last `XOR` left in `A`. An even number of blocks returns it to 0,
    // so the check is that it *moved* during the run, which the odd blocks did.
    assert_eq!(samples.len(), BLOCKS * SAMPLES_PER_BLOCK);
    assert_eq!(
        machine.border().index(),
        0,
        "eight flips end where they began"
    );

    let mut odd = common::machine();
    with_cpu_state(&mut odd, |state| state.af = 0);
    write_program(&mut odd, PROGRAM, &toggling_program(BORDER));
    common::set_pc(&mut odd, PROGRAM);
    for _ in 0..BLOCK_STEPS {
        odd.step();
    }
    assert_eq!(odd.border().index(), BORDER, "one flip moved the border");
    assert!(
        odd.take_samples().iter().all(|sample| sample.beeper == 0),
        "and it still made no sound"
    );
}

#[test]
fn the_speaker_and_the_border_come_out_of_the_same_write_without_disturbing_each_other() {
    // Bits 0-2 and bit 4 of one byte. A model that masked either of them wrongly would move
    // the other, and a guest writing a border colour would click.
    let mut machine = machine();
    with_cpu_state(&mut machine, |state| state.af = 0);
    write_program(
        &mut machine,
        PROGRAM,
        &[XOR_N, SPEAKER | 5, OUT_N_A, ULA_PORT_LOW],
    );
    common::set_pc(&mut machine, PROGRAM);
    machine.step();
    machine.step();

    assert_eq!(machine.border().index(), 5, "the border took bits 0-2");
    machine.run_frame();
    let samples = machine.take_samples();
    assert!(
        samples
            .iter()
            .rev()
            .take(16)
            .all(|s| s.beeper == AMPLITUDE_MAX),
        "and the speaker took bit 4 and stayed high"
    );
}

#[test]
fn the_beeper_is_the_same_instrument_on_both_machines() {
    // The beeper is a ULA feature and the ULA is the same chip, so the identical program must
    // produce the identical stream on a 128 — whose frame is 1020 T-states longer and whose
    // audio also carries a sound chip. If the two ever diverged, it would be the AY leaking
    // into the beeper's field, which is exactly the separation `docs/M7.md` Decision 6 asks
    // for.
    let mut forty_eight = machine();
    let mut one_two_eight = machine_128();
    let a = run_toggling(&mut forty_eight, SPEAKER);
    let b = run_toggling(&mut one_two_eight, SPEAKER);

    let beeper =
        |samples: &[Sample]| -> Vec<u16> { samples.iter().map(|sample| sample.beeper).collect() };
    assert_eq!(beeper(&a), beeper(&b));
    assert!(
        a.iter().all(|s| s.channels == [0; 3]),
        "a 48K has no sound chip and must emit silence on every channel"
    );
    assert!(
        b.iter().all(|s| s.channels == [0; 3]),
        "and a 128 whose chip nobody wrote to must too"
    );
}

#[test]
fn a_consumer_draining_once_a_frame_never_loses_a_sample() {
    // The property `SAMPLE_CAPACITY` is sized for, asserted through the machine rather than
    // through the buffer: run real frames of a program that is writing the speaker constantly
    // and check nothing was dropped.
    let mut machine = machine();
    with_cpu_state(&mut machine, |state| state.af = 0);
    write_program(&mut machine, PROGRAM, &toggling_program(SPEAKER));

    let mut samples = 0_usize;
    for _ in 0..4 {
        common::set_pc(&mut machine, PROGRAM);
        machine.run_frame();
        samples += machine.take_samples().len();
    }
    assert_eq!(machine.dropped_samples(), 0);
    assert!(samples > 8000, "four frames should be about 8736 samples");

    // And a consumer that does *not* drain is told, rather than left to wonder.
    //
    // **The first version of this assertion was wrong, and deriving why is the useful half.**
    // It ran three more frames without taking and expected an overrun, on the reasoning that
    // three frames of samples do not fit a buffer holding two. They did fit, because the
    // generator had not produced three frames of them: it runs **only when asked**, and the
    // program here stops writing the speaker after 512 T-states, so each frame's remaining
    // 69376 T-states stayed unrendered until the *next* frame's first write pulled them
    // through. Three frames of running produce a little under two frames of rendered samples,
    // and 4384 fits in 4432 with 48 to spare.
    //
    // That is a property of the design rather than an accident, and it is the same property
    // that keeps sound off the hot path: nothing is generated until something observes it. So
    // the overrun has to be forced by a consumer that *asks* after letting four frames go by,
    // which is what a consumer that missed its deadline actually looks like.
    for _ in 0..4 {
        common::set_pc(&mut machine, PROGRAM);
        machine.run_frame();
    }
    let late = machine.take_samples().len();
    assert_eq!(
        late,
        spectrum::audio::SAMPLE_CAPACITY,
        "a late consumer gets a full buffer and no more"
    );
    assert!(
        machine.dropped_samples() > 4_000,
        "and is told how much it missed: four frames is about 8736 samples into a buffer \
         holding {}",
        spectrum::audio::SAMPLE_CAPACITY
    );
}

#[test]
fn a_reset_silences_the_speaker() {
    // The reset line reaches the ULA's output latch. A model that left the speaker high would
    // leave a machine screaming after the reset button.
    let mut machine = machine();
    with_cpu_state(&mut machine, |state| state.af = 0);
    write_program(
        &mut machine,
        PROGRAM,
        &[XOR_N, SPEAKER, OUT_N_A, ULA_PORT_LOW],
    );
    common::set_pc(&mut machine, PROGRAM);
    machine.step();
    machine.step();

    machine.reset();
    machine.run_frame();
    assert!(
        machine.take_samples().iter().all(|s| s.beeper == 0),
        "a reset must leave the speaker low"
    );
}
