//! The external instrument for the tape: the ROM's **own** tape writer, measured.
//!
//! # Why this exists
//!
//! `crates/spectrum/src/tape/tap.rs` carries five timing constants — the pilot half-period,
//! the two sync half-periods and the two bit half-periods — plus two pilot **counts**. Every
//! one of them was derived by counting T-states through `SA-BYTES`, the 48K ROM's tape
//! writer, and a derivation is an argument until something independent agrees with it.
//!
//! `docs/STATUS.md` records the class: *"an algebraic argument is exactly as strong as its
//! weakest premise"*. Hand-counting `DJNZ` iterations across a page of someone else's machine
//! code is exactly the shape of argument that reads as certain and is one transcription error
//! away from being wrong.
//!
//! So this gate does not check the constants against themselves. It **runs the ROM's writer
//! on the real machine and measures the intervals between the edges it puts on the `MIC`
//! line**, then compares them to the pulse train
//! [`tape::tap::parse`](spectrum::tape::tap::parse) produces for the same bytes. The ROM is
//! code this project did not write, it is what "the standard timings" means, and it is
//! committed — so this is an oracle that runs on every `cargo test`, not an observation.
//!
//! # What the measurement found that the derivation did not predict
//!
//! Two things, both real and both recorded here rather than smoothed over.
//!
//! **The ROM's own writer does not emit a uniform train.** Within a byte every half-period is
//! exactly 855 or 1710, and the derivation holds. But the interval that *spans a byte
//! boundary* is **three T-states shorter**, because the `LD B,$31` at `0x052D` compensates
//! for the per-byte housekeeping — `DEC DE`, `INC IX`, the `IN A,($FE)` break check and the
//! re-entry through `SA-LOOP` — and lands three T-states under. The first half-period after
//! the sync is **one T-state longer**, for the mirror-image reason: it is entered with
//! `B = 0x3B` from `LD BC,$3B0E` rather than with the loop's own `0x3E`. Neither deviation is
//! a defect in the ROM or in our converter: a `.tap` carries no timings at all, so every
//! player in the world emits the idealised values, and the loader's thresholds are hundreds
//! of T-states wide. They are asserted here **by value**, because a deviation that is
//! measured and named is a fact and a deviation that is tolerated by a range check is a place
//! for a future error to hide.
//!
//! **The ULA contends the writer's own `OUT`s.** Port `0xFE` is a ULA port, so every edge
//! during the display window is delayed by 0–6 T-states, and the emitted signal on real
//! hardware genuinely jitters by that much. The measurement removes it — see
//! [`Recorder::nominal_now`] — rather than widening the assertions to hide it, so the
//! comparison below is an equality and not a tolerance.
//!
//! # What it grades, and what it does not
//!
//! It grades the **converter**: the pilot period, the pilot length for both flag classes, the
//! sync pair, and the two bit periods, in order and by value.
//!
//! It does **not** grade the timings against *hardware*. The ROM defines what a `.tap` means
//! and it is what every loader was written against, but nobody here has measured a real
//! Spectrum. That row stays in the ungraded list.
//!
//! It also does not grade the **loader**; `tape_rom_load.rs` does that, and the two are
//! deliberately separate instruments. This one would still pass with the `EAR` bit wired to
//! nothing, because `SA-BYTES` only writes.
//!
//! # The recording bus
//!
//! `Ula` is a public type implementing a public trait, so a test can put its own `Bus` in
//! front of it and see every transfer. That is the whole mechanism, and no production code is
//! modelled for the test's benefit: `MIC` is still discarded by the machine, exactly as
//! `docs/M6.md` says it is until tape *saving* is built.

use spectrum::memory::Memory;
use spectrum::tape::tap;
use spectrum::timing::T_STATES_PER_FRAME;
use spectrum::ula::Ula;
use z80::{
    Bus, Cpu, CpuState, MEMORY_ACCESS_T_STATES, OPCODE_FETCH_T_STATES, PORT_ACCESS_T_STATES,
};

/// `SA-BYTES`, the ROM's tape writer.
///
/// Entered with `IX` at the data, `DE` holding its length and `A` the flag byte. It performs
/// its own `DI`, and emits flag, data and parity as one block.
const SA_BYTES: u16 = 0x04C2;

/// Bit 3 of a `0xFE` write: the `MIC` output, which is what a tape recorder records.
const MIC_BIT: u8 = 0x08;

/// Where the block being written lives. Uncontended RAM, so nothing here depends on it.
const DATA: u16 = 0x8000;

/// The stack `SA-BYTES` pushes its return address onto.
const STACK: u16 = 0x7F00;

/// Bytes to record.
///
/// Three, and chosen rather than arbitrary: `0x00` and `0xFF` make a whole byte of each bit
/// value, so a converter that emitted one length for both would produce a train of the right
/// shape and the wrong values; `0xA5` alternates, so a converter that got the bit **order**
/// backwards would still differ from its own mirror image.
const PAYLOAD: [u8; 3] = [0x00, 0xFF, 0xA5];

/// The ULA's largest I/O stall, from the published delay pattern's first entry.
const MAX_ULA_STALL: u64 = 6;

/// A ULA in front of a recorder, so the test sees every `OUT` with the clock's own time.
struct Recorder {
    ula: Ula,
    /// Contention-free T-states at which the `MIC` level changed, in order.
    edges: Vec<u64>,
    /// The level it changed to.
    mic: bool,
    /// Every T-state the ULA has charged as a stall rather than as a machine cycle's own.
    stalls: u64,
}

impl Recorder {
    fn new(rom: &[u8]) -> Self {
        Self {
            ula: Ula::new(Memory::spectrum_48k(rom).expect("the 48K ROM is one page")),
            edges: Vec::new(),
            mic: false,
            stalls: 0,
        }
    }

    /// T-states since power-on. Absolute rather than frame-relative, because a frame boundary
    /// inside the pilot tone would otherwise make one interval look negative.
    fn now(&self) -> u64 {
        let clock = self.ula.clock();
        clock.frames() * u64::from(T_STATES_PER_FRAME) + u64::from(clock.frame_t_state())
    }

    /// The same clock with every contention stall removed.
    ///
    /// The ULA contends port `0xFE`, so the writer's own edges are delayed by 0–6 T-states
    /// each while the display is being drawn — which is what the hardware does and is not
    /// what a `.tap` describes. Removing it makes the comparison against the converter an
    /// **equality** rather than a tolerance, and a tolerance is where a future error hides.
    ///
    /// This is not circular. It uses the contention model to subtract contention, and that
    /// model is graded by four gates of its own in this directory — `contention_magnitude`,
    /// `contention_phase`, `io_contention` and `block_contention`. What is left is the pure
    /// Z80 instruction timing of the ROM's loop, which is the quantity `tap.rs`'s constants
    /// claim to be.
    fn nominal_now(&self) -> u64 {
        self.now() - self.stalls
    }

    /// Run `body` and add whatever it charged beyond `nominal` to the stall account.
    fn charge<T>(&mut self, nominal: u64, body: impl FnOnce(&mut Ula) -> T) -> T {
        let before = self.now();
        let value = body(&mut self.ula);
        self.stalls += self.now() - before - nominal;
        value
    }
}

impl Bus for Recorder {
    fn fetch(&mut self, address: u16) -> u8 {
        // A transfer ticks nothing itself: its own T-states arrive afterwards as `tick`
        // calls, so everything the clock moves here is stall.
        self.charge(0, |ula| ula.fetch(address))
    }

    fn read(&mut self, address: u16) -> u8 {
        self.charge(0, |ula| ula.read(address))
    }

    fn write(&mut self, address: u16, value: u8) {
        self.charge(0, |ula| ula.write(address, value));
    }

    fn in_port(&mut self, port: u16) -> u8 {
        self.charge(0, |ula| ula.in_port(port))
    }

    fn out_port(&mut self, port: u16, value: u8) {
        // The ULA answers every even port; the high half is the keyboard's row selector.
        if port & 1 == 0 {
            let mic = value & MIC_BIT != 0;
            // The first write is an edge whether or not the level moved: it is where the
            // routine takes the line over, and it opens the first half-period. Without this
            // the pilot's opening `OUT` — which writes the level the line already idles at —
            // goes unrecorded and every count below comes out one short.
            if self.edges.is_empty() || mic != self.mic {
                self.mic = mic;
                self.edges.push(self.nominal_now());
            }
        }
        self.charge(0, |ula| ula.out_port(port, value));
    }

    fn tick(&mut self, address: u16) {
        // One nominal T-state, plus whatever a standalone internal cycle contends.
        self.charge(1, |ula| ula.tick(address));
    }
}

// The nominal machine-cycle lengths this file reasons about are `crates/z80`'s own, so a
// change there is a compile error here rather than a silently wrong subtraction.
const _: () = assert!(OPCODE_FETCH_T_STATES == 4);
const _: () = assert!(MEMORY_ACCESS_T_STATES == 3);
const _: () = assert!(PORT_ACCESS_T_STATES == 4);

/// Run `SA-BYTES` over `payload` with `flag`, and return the first `wanted` half-periods it
/// emits, contention removed.
fn rom_emitted_pulses(rom: &[u8], flag: u8, payload: &[u8], wanted: usize) -> Vec<u32> {
    let mut cpu = Cpu::new(Recorder::new(rom));
    for (offset, &byte) in (0..).zip(payload) {
        cpu.bus_mut().ula.memory_mut().write(DATA + offset, byte);
    }

    cpu.set_state(CpuState {
        pc: SA_BYTES,
        ix: DATA,
        de: u16::try_from(payload.len()).expect("a short payload"),
        af: u16::from(flag) << 8,
        sp: STACK,
        // `SA-BYTES` performs its own `DI`, but not on its first instruction. With interrupts
        // already off, nothing can land between entry and that `DI`.
        iff1: false,
        iff2: false,
        ..CpuState::default()
    });

    // Bounded by the signal itself rather than by a T-state budget, so the run stops on the
    // edge we asked for and never records the extra one `SA/LD-RET` produces when it restores
    // the border on the way out.
    let budget = u64::from(u32::MAX);
    while cpu.bus().edges.len() <= wanted && cpu.bus().now() < budget {
        cpu.step();
    }
    assert!(
        cpu.bus().edges.len() > wanted,
        "the writer stopped after {} edges, short of the {wanted} asked for",
        cpu.bus().edges.len()
    );

    cpu.bus()
        .edges
        .windows(2)
        .take(wanted)
        .map(|pair| match pair {
            [before, after] => u32::try_from(after - before).expect("an interval under 4 GT"),
            _ => unreachable!("windows(2) yields pairs"),
        })
        .collect()
}

/// A `.tap` file holding one block: a length word, the flag, the payload, and its parity.
fn tap_file(flag: u8, payload: &[u8]) -> Vec<u8> {
    let parity = payload.iter().fold(flag, |sum, &byte| sum ^ byte);
    let length = u16::try_from(payload.len() + 2).expect("a short block");
    let mut file = length.to_le_bytes().to_vec();
    file.push(flag);
    file.extend_from_slice(payload);
    file.push(parity);
    file
}

fn sinclair_rom() -> Option<Vec<u8>> {
    let path = testsupport::testdata_dir().join("roms").join("48.rom");
    match std::fs::read(&path) {
        Ok(rom) => Some(rom),
        Err(_) => {
            testsupport::skip_absent_corpus("the Sinclair 48K ROM", &path);
            None
        }
    }
}

/// Everything the converter emits for one block except the trailing silence.
///
/// `SA-BYTES` returns after the block's last edge and emits nothing more, so the pause our
/// converter appends has no counterpart — a difference in what the two are *for* rather than
/// a disagreement about a timing.
fn converter_pulses(flag: u8, payload: &[u8]) -> Vec<u32> {
    let tape = tap::parse(&tap_file(flag, payload)).expect("a well-formed block");
    tape.pulses()
        .split_last()
        .map_or_else(Vec::new, |(_pause, head)| head.to_vec())
}

/// Half-periods one byte contributes: two per bit.
const PULSES_PER_BYTE: usize = 16;

/// How far the ROM's own half-period differs from the idealised one, at each place it does.
///
/// Every entry is derived by counting T-states through the path `SA-BYTES` takes to reach the
/// next `OUT`, and every one of them was **predicted before it was measured** — which is the
/// order this project asks for: derive the expected value from the hardware, then measure.
/// The parity byte's entry is the one that was got wrong first time, at −3 rather than −1, and
/// the measurement is what named the missing two T-states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deviation {
    /// The first half-period after the sync, entered at `SA-BIT-1` with `B = 0x3B` from
    /// `LD BC,$3B0E` rather than with the loop's own `0x3E`. One `DJNZ` iteration is 13
    /// T-states and three of the four instructions it replaces are gone, so it runs **one
    /// T-state long**.
    FirstBitAfterSync,
    /// A half-period spanning a byte boundary. `LD B,$31` at `0x052D` compensates for
    /// `DEC DE`, `INC IX`, the `IN A,($FE)` break check and the re-entry through `SA-LOOP`,
    /// and lands **three T-states under**.
    ByteBoundary,
    /// The boundary before the **parity** byte, which reaches the bit loop through
    /// `SA-PARITY` instead: `JR Z` taken (12) plus `LD L,H` (4) plus `JR SA-LOOP-P` (12),
    /// against `JR Z` not taken (7) plus `LD L,(IX+0)` (19). Two T-states more than the
    /// ordinary boundary, so **one T-state under** rather than three.
    ParityBoundary,
}

impl Deviation {
    /// The ROM's half-period, given the idealised one.
    const fn apply(self, pulse: u32) -> u32 {
        match self {
            Self::FirstBitAfterSync => pulse + 1,
            Self::ByteBoundary => pulse - 3,
            Self::ParityBoundary => pulse - 1,
        }
    }
}

/// The ROM's train for the same block: the converter's, with the deviations applied.
///
/// Written as a transformation of the ideal train rather than as a second literal train,
/// because the *claim* being made is precisely that the two differ in these places and nowhere
/// else. A literal train would also be 3305 numbers nobody could check.
fn expected_rom_train(converted: &[u32], bit_region: usize, bytes: usize) -> Vec<u32> {
    let parity_boundary = bytes.saturating_sub(1) * PULSES_PER_BYTE;
    converted
        .iter()
        .zip(0..)
        .map(|(&pulse, index): (&u32, usize)| {
            match index.checked_sub(bit_region) {
                // Still in the pilot tone or the sync pair, where the ROM is exact.
                None => pulse,
                Some(0) => Deviation::FirstBitAfterSync.apply(pulse),
                Some(bit) if bit == parity_boundary => Deviation::ParityBoundary.apply(pulse),
                Some(bit) if bit % PULSES_PER_BYTE == 0 => Deviation::ByteBoundary.apply(pulse),
                Some(_) => pulse,
            }
        })
        .collect()
}

/// Where the pilot tone stops in a train: the first pulse that is not the opening one.
fn pilot_length(train: &[u32]) -> usize {
    let period = train.first().copied().unwrap_or_default();
    train.iter().take_while(|&&pulse| pulse == period).count()
}

#[test]
fn the_rom_emits_the_train_our_converter_produces() {
    // The whole gate in one comparison: 3305 half-periods for a data block, produced by two
    // implementations that share no code, asserted equal in order and by value — with the two
    // measured deviations applied by name rather than absorbed into a tolerance.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    // A data block: 3223 pilot pulses rather than a header's 8063, which keeps the run short.
    // `the_pilot_is_longer_for_a_header_than_for_a_data_block` grades the other branch.
    let flag = 0xFF;
    let converted = converter_pulses(flag, &PAYLOAD);
    let measured = rom_emitted_pulses(&rom, flag, &PAYLOAD, converted.len());
    // Two sync pulses separate the pilot from the first bit; the block is the flag byte, the
    // payload and the parity byte.
    let expected = expected_rom_train(&converted, pilot_length(&converted) + 2, PAYLOAD.len() + 2);

    let divergences: Vec<(usize, u32, u32)> = measured
        .iter()
        .zip(&expected)
        .zip(0..)
        .filter(|((rom, ours), _)| rom != ours)
        .map(|((&rom, &ours), index)| (index, rom, ours))
        .collect();
    assert_eq!(
        divergences,
        Vec::new(),
        "the ROM's writer and our converter disagree, as (index, ROM, expected)"
    );
    assert_eq!(measured.len(), expected.len());
}

#[test]
fn the_pilot_tone_is_exactly_our_pilot_period_and_our_pilot_length() {
    // The two constants a wrong hand-count would most plausibly get wrong, graded on their
    // own so a failure names which. `BIT 7,A` in `SA-BYTES` chooses between two lengths that
    // are not derivable from each other, so a converter using one for both would pass every
    // test built from a single flag class.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    for flag in [0x00_u8, 0xFF] {
        let converted = converter_pulses(flag, &PAYLOAD);
        let pilot = pilot_length(&converted);
        let measured = rom_emitted_pulses(&rom, flag, &PAYLOAD, pilot + 2);

        assert_eq!(
            pilot_length(&measured),
            pilot,
            "flag {flag:#04X}: the pilot tone is the wrong number of half-periods"
        );
        assert_eq!(
            measured.get(..pilot),
            converted.get(..pilot),
            "flag {flag:#04X}: the pilot half-period is the wrong length"
        );
        assert_eq!(
            measured.get(pilot..pilot + 2),
            converted.get(pilot..pilot + 2),
            "flag {flag:#04X}: the sync pair is wrong"
        );
    }
}

#[test]
fn the_contention_the_measurement_removes_is_real_and_bounded() {
    // A positive control for `nominal_now`. If the subtraction were a no-op the assertions
    // above would be measuring a contended clock and would have to have been loosened to
    // pass; if it over-subtracted they would be measuring something shorter than the machine
    // ever ran. So: the raw clock must be *ahead* of the corrected one by a real amount, and
    // every recorded stall must be inside the published pattern's range.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let mut cpu = Cpu::new(Recorder::new(&rom));
    cpu.set_state(CpuState {
        pc: SA_BYTES,
        ix: DATA,
        de: 1,
        af: 0xFF00,
        sp: STACK,
        ..CpuState::default()
    });
    // Long enough to cross into the display window, where port 0xFE is contended.
    while cpu.bus().now() < 200_000 {
        cpu.step();
    }

    let recorder = cpu.bus();
    assert!(
        recorder.stalls > 0,
        "the writer ran 200,000 T-states without the ULA contending one port access, so the \
         correction below is measuring nothing"
    );
    assert!(recorder.nominal_now() < recorder.now());
    let intervals = recorder.edges.windows(2).count();
    assert!(
        recorder.stalls <= MAX_ULA_STALL * intervals as u64,
        "{} T-states of stall across {intervals} edges is more than the pattern can charge",
        recorder.stalls
    );
}
