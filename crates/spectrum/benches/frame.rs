//! What a frame of the **real** machine costs, and what M7's sound half added to it.
//!
//! # Why this exists, and the mistake it replaces
//!
//! `docs/M7.md` Decision 3 raised the cost of turning the contention constants into fields
//! and named `cargo bench -p z80 --bench step` as the command to measure it. Then it checked,
//! and corrected itself in the open:
//!
//! > *"**This named `cargo bench -p z80 --bench step` as the command, and that command cannot
//! > see this change at all.** … the bench measures a CPU against a bus this milestone does
//! > not touch, and **there is no benchmark in this workspace that exercises
//! > `spectrum::Ula`** … So the honest statement is that the command to measure this does not
//! > exist, and naming one that does exist but measures something else is worse than naming
//! > none: it is checkable, it runs, it produces numbers, and every one of them is about a
//! > different bus."*
//!
//! It then said exactly what would settle it: *"a bench in `crates/spectrum` that steps the
//! real `Ula` over a frame, which is the same shape `step.rs` already has and does not exist
//! yet."* This is that bench. It is written because M7's sound half makes a claim about the
//! hot path — that it adds nothing to it — and a performance claim with no command behind it
//! is the same defect one milestone later.
//!
//! # What each case isolates
//!
//! Every case runs **one real frame** of a real `Spectrum`: a real `Cpu<Ula>`, real
//! contention, the real slot map. They differ only in what the guest does and whether
//! anything drains the audio, so a difference between two rows is attributable to that and
//! to nothing else.
//!
//! | Case | What it adds to the row above it |
//! |---|---|
//! | `quiet_48k` / `quiet_128` | the baseline: a frame of `NOP`s out of uncontended RAM, nobody touching sound |
//! | `contended_48k` | the same frame out of the screen bank, so every fetch is stalled |
//! | `drained_48k` / `drained_128` | one `take_samples` per frame — **the whole cost of sound for a silent machine**, since nothing wrote a sound port |
//! | `beeper_48k` | a guest toggling the speaker every 18 T-states, drained once |
//! | `ay_128` | a guest writing an AY register every 29 T-states, drained once, with three tones and an envelope running |
//! | `tape_reads_48k` | a guest reading `0xFE` every 11 T-states — the only case that exercises the **`IN`** path at all, and what `Spectrum::ear_reads`'s counter is priced against |
//! | `border_48k` | a guest flipping the **border** every 18 T-states — about 3900 bands' worth in a frame |
//! | `quiet_rendered_48k` / `border_rendered_48k` | the same frames **plus a `render`**, which is the only thing that walks the border record. The gap between the pair is what drawing bands costs over drawing one colour |
//!
//! `beeper_48k`, `ay_128` and `border_48k` are deliberately far heavier than real software: a
//! music driver writes the AY about fourteen times per **frame**, not two thousand times, and
//! a tape load changes the border 25 to 30 times. They are upper bounds, not workloads.
//!
//! # What it measured, on 2026-09-01
//!
//! Medians on this machine, against a 20,000 µs frame budget. **Re-run it rather than quoting
//! it** — `docs/STATUS.md`'s standing rule is that a bare number is a measurement with its
//! subject deleted, and these carry their command and their date for that reason.
//!
//! | Case | Median | Over `quiet_48k` | Share of a frame |
//! |---|---|---|---|
//! | `quiet_48k` | 148.0 µs | — | 0.74 % |
//! | `quiet_rendered_48k` | 151.7 µs | +3.7 µs | 0.76 % |
//! | `drained_48k` | 154.4 µs | +6.4 µs | 0.77 % |
//! | `border_48k` | 167.1 µs | +19.1 µs | 0.84 % |
//! | `border_rendered_48k` | 174.3 µs | +26.3 µs | 0.87 % |
//! | `beeper_48k` | 197.4 µs | +49.4 µs | 0.99 % |
//!
//! Two readings worth stating rather than leaving to arithmetic. **`quiet_48k` did not move
//! when sound and the border record landed** — 148.0 µs before and after, which is the
//! measured form of the claim that neither is on the hot path. And **drawing bands rather
//! than one colour costs 3.5 µs**: `border_rendered_48k − border_48k` is 7.2 µs against
//! `quiet_rendered_48k − quiet_48k`'s 3.7 µs, so the record itself is the difference.
//!
//! # Reading it
//!
//! The counter column reports **T-states per second**, so the realtime multiple is that figure
//! divided by the machine's own clock — 3.5 MHz for a 48K and 3.5469 MHz for a 128. No
//! arithmetic about "average T-states per instruction" and no estimate to get wrong.
//!
//! Run with `cargo bench -p spectrum`.

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
use spectrum::memory::PAGE_SIZE;
use spectrum::{Frame, Spectrum, timing::Timing};

fn main() {
    divan::main();
}

/// Uncontended RAM on both machines: bank 2 on a 48K, and bank 2 on a 128 at reset.
const UNCONTENDED: u16 = 0x8000;

/// The screen bank, contended on both machines.
const CONTENDED: u16 = 0x4000;

/// A ROM of `NOP`s. Nothing here executes it; every case sets `PC` into RAM.
fn rom() -> [u8; PAGE_SIZE] {
    [0x00; PAGE_SIZE]
}

fn forty_eight() -> Spectrum {
    Spectrum::new(&rom()).expect("a page-sized ROM")
}

fn one_two_eight() -> Spectrum {
    Spectrum::spectrum_128(&rom(), &rom()).expect("two page-sized ROMs")
}

/// Fill the 16 KB from `address` with `program`, repeated.
///
/// Repeated rather than run in a loop: a `JP` back to the top would put a branch in the
/// measurement, and the point is to measure the bus rather than the branch predictor.
fn load(machine: &mut Spectrum, address: u16, program: &[u8]) {
    let mut offset = 0_usize;
    while offset < PAGE_SIZE {
        for &byte in program {
            let Ok(target) = u16::try_from(usize::from(address) + offset) else {
                return;
            };
            machine.memory_mut().write(target, byte);
            offset += 1;
            if offset >= PAGE_SIZE {
                return;
            }
        }
    }
}

/// Point `PC` at `address`.
fn set_pc(machine: &mut Spectrum, address: u16) {
    let mut state = machine.cpu_state();
    state.pc = address;
    machine.set_cpu_state(state);
}

/// Run one frame from `address`, optionally draining the audio, and report the T-states.
fn one_frame(machine: &mut Spectrum, address: u16, drain: bool) -> u64 {
    set_pc(machine, address);
    let before = machine.ula().clock().t_states();
    machine.run_frame();
    if drain {
        black_box(machine.take_samples().len());
    }
    machine.ula().clock().t_states() - before
}

/// One benchmark case: build a machine, load a program, and run frames off it.
fn case(
    bencher: Bencher<'_, '_>,
    build: fn() -> Spectrum,
    address: u16,
    program: &[u8],
    drain: bool,
    frame_t_states: u32,
) {
    bencher
        .counter(ItemsCount::new(frame_t_states))
        .with_inputs(|| {
            let mut machine = build();
            load(&mut machine, address, program);
            machine
        })
        .bench_local_values(|mut machine| {
            black_box(one_frame(&mut machine, address, drain));
            machine
        });
}

/// `NOP`.
const NOPS: &[u8] = &[0x00];

/// `XOR 0x10 : OUT (0xFE),A` — the speaker flipped every eighteen T-states.
const BEEPER: &[u8] = &[0xEE, 0x10, 0xD3, 0xFE];

/// `XOR 0x07 : OUT (0xFE),A` — the **border** flipped every eighteen T-states.
///
/// About 3900 border changes in a frame, against the 25 to 30 a tape load makes. An upper
/// bound on what a border-multicolour demo costs, not a workload.
const BORDER: &[u8] = &[0xEE, 0x07, 0xD3, 0xFE];

/// `IN A,(0xFE)` — the tape port read as hard as a loader reads it, and harder.
///
/// **The row `Spectrum::ear_reads` is measured against**, and the reason it exists: every case
/// above writes ports and none of them reads one, so a cost on the `IN` path is invisible to all
/// of them. This one is nothing but that path — an `IN` every eleven T-states, about six thousand
/// a frame against the 682 a running ROM loader makes — so whatever the counter costs shows up
/// here magnified roughly ninefold, and a figure that is lost in the noise here is lost in the
/// noise everywhere.
const TAPE_READS: &[u8] = &[0xDB, 0xFE];

/// `LD BC,0xBFFD : LD A,n : OUT (C),A` — an AY data write every twenty-nine T-states.
///
/// The register the writes land in is whatever `setup_ay` last latched, which is the volume
/// of channel A. That is what a driver's hottest inner loop looks like and it is what makes
/// this the expensive case: every one of these forces the generator to catch up.
const AY_WRITES: &[u8] = &[0x01, 0xFD, 0xBF, 0x3E, 0x0F, 0xED, 0x79];

const FORTY_EIGHT_FRAME: u32 = 69_888;
const ONE_TWO_EIGHT_FRAME: u32 = 70_908;

// The frame lengths are written out here so a change to either shows up as a disagreement
// rather than as a silently rescaled throughput figure.
const _: () = assert!(Timing::SPECTRUM_48K.frame_t_states() == FORTY_EIGHT_FRAME);
const _: () = assert!(Timing::SPECTRUM_128.frame_t_states() == ONE_TWO_EIGHT_FRAME);

#[divan::bench]
fn quiet_48k(bencher: Bencher) {
    case(
        bencher,
        forty_eight,
        UNCONTENDED,
        NOPS,
        false,
        FORTY_EIGHT_FRAME,
    );
}

#[divan::bench]
fn quiet_128(bencher: Bencher) {
    case(
        bencher,
        one_two_eight,
        UNCONTENDED,
        NOPS,
        false,
        ONE_TWO_EIGHT_FRAME,
    );
}

#[divan::bench]
fn contended_48k(bencher: Bencher) {
    case(
        bencher,
        forty_eight,
        CONTENDED,
        NOPS,
        false,
        FORTY_EIGHT_FRAME,
    );
}

#[divan::bench]
fn drained_48k(bencher: Bencher) {
    case(
        bencher,
        forty_eight,
        UNCONTENDED,
        NOPS,
        true,
        FORTY_EIGHT_FRAME,
    );
}

#[divan::bench]
fn drained_128(bencher: Bencher) {
    case(
        bencher,
        one_two_eight,
        UNCONTENDED,
        NOPS,
        true,
        ONE_TWO_EIGHT_FRAME,
    );
}

#[divan::bench]
fn beeper_48k(bencher: Bencher) {
    case(
        bencher,
        forty_eight,
        UNCONTENDED,
        BEEPER,
        true,
        FORTY_EIGHT_FRAME,
    );
}

#[divan::bench]
fn tape_reads_48k(bencher: Bencher) {
    case(
        bencher,
        forty_eight,
        UNCONTENDED,
        TAPE_READS,
        false,
        FORTY_EIGHT_FRAME,
    );
}

#[divan::bench]
fn border_48k(bencher: Bencher) {
    case(
        bencher,
        forty_eight,
        UNCONTENDED,
        BORDER,
        false,
        FORTY_EIGHT_FRAME,
    );
}

/// A frame **and the render of it**, which is the only case that walks the border record.
///
/// Every other row above measures the machine; this one measures the machine plus what a
/// frontend does with it. The difference between it and `border_48k` is the whole cost of
/// drawing the bands rather than one colour.
#[divan::bench]
fn border_rendered_48k(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(FORTY_EIGHT_FRAME))
        .with_inputs(|| {
            let mut machine = forty_eight();
            load(&mut machine, UNCONTENDED, BORDER);
            (machine, Frame::new())
        })
        .bench_local_values(|(mut machine, mut frame)| {
            black_box(one_frame(&mut machine, UNCONTENDED, false));
            machine.render(&mut frame);
            (machine, frame)
        });
}

/// The same render with a border nobody wrote, so the two rows bracket the record's cost.
#[divan::bench]
fn quiet_rendered_48k(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(FORTY_EIGHT_FRAME))
        .with_inputs(|| {
            let mut machine = forty_eight();
            load(&mut machine, UNCONTENDED, NOPS);
            (machine, Frame::new())
        })
        .bench_local_values(|(mut machine, mut frame)| {
            black_box(one_frame(&mut machine, UNCONTENDED, false));
            machine.render(&mut frame);
            (machine, frame)
        });
}

#[divan::bench]
fn ay_128(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(ONE_TWO_EIGHT_FRAME))
        .with_inputs(|| {
            let mut machine = one_two_eight();
            setup_ay(&mut machine);
            load(&mut machine, UNCONTENDED, AY_WRITES);
            machine
        })
        .bench_local_values(|mut machine| {
            black_box(one_frame(&mut machine, UNCONTENDED, true));
            machine
        });
}

/// Put three tones, noise and an envelope on the chip, so the generator has work to do.
///
/// Through the bus rather than through an assembled program, because setting the chip up is
/// not what is being measured — the frame that follows is.
fn setup_ay(machine: &mut Spectrum) {
    use z80::Bus;
    for (register, value) in [
        (0_u8, 0x55_u8),
        (1, 0x01),
        (2, 0xA0),
        (4, 0x11),
        (6, 0x07),
        (7, 0b0011_0110),
        (8, 0x10),
        (9, 0x0F),
        (10, 0x0F),
        (11, 0x08),
        (13, 0x0A),
    ] {
        machine.ula_mut().out_port(0xFFFD, register);
        machine.ula_mut().out_port(0xBFFD, value);
    }
    // Leave the latch on channel A's volume, which is what `AY_WRITES` then hammers.
    machine.ula_mut().out_port(0xFFFD, 8);
}
