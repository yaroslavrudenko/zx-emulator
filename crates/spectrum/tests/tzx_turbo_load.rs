//! A turbo block, read by a loader of our own, at timings the ROM cannot follow.
//!
//! # The gap this closes
//!
//! `tzx_rom_load.rs` said it plainly, and until this file existed it was true:
//!
//! > And **a genuinely turbo block** — one whose bits are faster than the ROM's — because the ROM
//! > cannot read one. Nothing in this repository can.
//!
//! That sentence made `.tzx`'s entire reason for existing ungraded. `crates/spectrum/src/tape`'s
//! own module documentation rests the whole format on it — *"a `.tap` file **cannot represent a
//! custom loader's tape at all**: it is block data with the ROM's standard timings implied, and
//! nothing in it can say 'this loader uses 700-T-state bits'"* — and then every gate in the suite
//! handed its `.tzx` to `LD-BYTES`, which only reads 855/1710. The pilot, sync and bit fields of
//! `ID 11` were therefore graded only at **the ROM's own values**, where they are indistinguishable
//! from constants — so a converter that ignored them and emitted the standard train would have
//! passed every gate in this repository that **runs a machine**.
//!
//! So this file supplies the missing half of the machine: **a loader we wrote**, running from RAM,
//! counting edges on `IN A,($FE)` itself, decoding a block at **pilot 1400 / sync 500 / bit0 500 /
//! bit1 1200** T-states against the ROM's 2168 / 667 / 855 / 1710. Every one of those five numbers
//! is a field in the file, and until now nothing in this workspace read them as anything but the
//! constants they coincide with.
//!
//! # What the ROM-driven gates could not see, measured
//!
//! The paragraph above is measured rather than argued — and the measurement corrected the sentence
//! it was taken to support. An earlier draft claimed such a converter *"would have passed every
//! tape gate in this repository"*, and that is **false**: `tzx_vectors.rs` compares against pulse
//! trains transcribed by hand from the format description, owes nothing to any loader, and catches
//! all three mutations below. The claim survives only in the narrower form it is now written in,
//! about the gates that grade by **running** something.
//!
//! Three mutations of `crates/spectrum/src/tape/tzx.rs`'s `ID 11` handler, taken in a scratch
//! clone on 2026-09-02 — the same practice `timing_oracle.rs` records — each one's landing proved
//! by diff and hash before its verdict was read, each restore proved clean afterwards:
//!
//! | mutation of the `ID 11` handler | this file | `tzx_vectors.rs` | `tzx_rom_load.rs` | `tzx_rom_timings.rs` |
//! |---|---|---|---|---|
//! | the bit-one field ignored, the ROM's 1710 emitted instead | **RED**, 5 of 16 | **RED**, 5 of 10 | GREEN | GREEN |
//! | the pilot and first-sync fields transposed | **RED**, 9 of 16 | **RED**, 4 of 10 | **RED**, 3 of 7 | **RED**, 2 of 5 |
//! | the pilot-count field ignored, the ROM's 3223 emitted instead | **RED**, 6 of 16 | **RED**, 5 of 10 | GREEN | GREEN |
//!
//! **Rows one and three are the gap, and they are exactly the gap the two ROM-driven files predict
//! for themselves.** Substituting the ROM's own value for a field is invisible to a gate whose
//! tape carries the ROM's own value — there is nothing to disagree with. The middle row is
//! different in kind: a transposition is wrong at *every* speed, so it breaks the standard tapes
//! too, and all four gates see it.
//!
//! **This file is therefore not the only thing that catches rows one and three, and claiming it
//! were would be the overclaim `docs/STATUS.md` catalogues.** `tzx_vectors.rs` catches them by
//! comparing a pulse array against literals a human wrote down. This file catches them by
//! **decoding the signal on a Z80** — through the ULA, the four-case I/O rule and the frame clock
//! — which is a different instrument reaching the same fields, and the only one of the two that
//! would notice if the pulses were right and the machine that reads them were wrong.
//!
//! # Why the loader's arithmetic is the machine's arithmetic
//!
//! The loader does not know what a T-state is. [`EDGE`](assemble) counts iterations of a six
//! instruction polling loop until bit 6 of port `0xFE` changes, and that count is the only number
//! it ever has. The loop is
//!
//! ```text
//! E1: INC B        4        so one poll costs 43 T-states of Z80 time, plus whatever
//!     RET Z        5        the ULA charges for the port read — which during the display
//!     IN A,($FE)  11        window is the four-case I/O rule's own delay, and is therefore
//!     AND $40      7        a function of the frame clock's absolute position.
//!     XOR C        4
//!     JR Z,E1     12
//! ```
//!
//! A half-period of `H` T-states therefore lands `B` at roughly `H / 50` once the ULA's share is
//! counted — 50 rather than 43 being **measured, by where the thresholds are observed to bite,
//! rather than hand-counted**; [`Thresholds::DEFAULT`] gives the three brackets that pin it. That
//! puts the control tape's three populations at about **26** for the 1400-T-state pilot, **23**
//! for the 1200-T-state one bit, and **9** for the 500-T-state sync and zero bit, and the loader's
//! two thresholds sit in the two gaps at 20 and 16.
//!
//! The gaps are narrower than they look, and that is a finding rather than a nuisance: the ULA's
//! contribution to the poll cost **varies across the frame**, because the four-case I/O rule
//! charges differently inside and outside the display window. A tape whose one bit lands near a
//! threshold therefore has some of its bits read one way and some the other, depending only on
//! where in the frame they fell. [`Outcome::SilentlyWrong`] is that happening, and it is why the
//! speed table below has a row that is neither a pass nor a clean failure.
//!
//! **That is why this grades the machine and not just the parser.** The counts are not read out of
//! the file; they are accumulated by a real Z80 executing real `IN` machine cycles through
//! [`spectrum::ula`], each contended by the four-case rule, against a level that is a function of
//! the pulse train's absolute position in the frame clock. Move any of it and the populations move
//! with it. `docs/M6.md` Decision 4's standing rule — nothing supplies a byte to the CPU by any
//! route other than a port read — is what makes that so, and it holds here by construction:
//! **this file never calls the ROM.** The machine is built on a blank ROM page, the loader is
//! entered with `PC` set into RAM, and [`Run::stayed_in_ram`] records whether it ever left, which
//! the gate below asserts it did not.
//!
//! # Why the expected checksum is a constant in the loader's own code
//!
//! Because the first version of this instrument scored a mutation **green**, and the reason is
//! worth more than the gate.
//!
//! That version put the expected `XOR` fold on the tape as a 6913th byte and had the loader
//! compare its computed fold against the byte it had just read. A payload of 6912 zeros folds to
//! zero, and the checksum byte *on that same tape* is also zero — so a loader that decoded nothing
//! at all, leaving `0x4000..0x5B00` as it found it, computed zero, read zero, and lit the border
//! green. The instrument reported success for a decode that never happened. **A gate that passes
//! on an all-zeros decode is the exact defect this repository has spent its whole history
//! cataloguing**, and it was one substitution away from being written down as a proof.
//!
//! Moving the constant into the loader's instruction stream closes it: [`EXPECTED_XOR`] appears in
//! the code as the immediate of a `CP`, it appears **nowhere on any tape**, and
//! [`an_all_zeros_decode_is_refused_rather_than_folding_to_the_expected_value`] is the permanent
//! negative control that says so. The constant is deliberately not zero, and
//! [`loading_screen`] closes the fold onto it with a single chosen attribute byte so that it
//! cannot become zero by accident.
//!
//! # The mutation record
//!
//! Three mutations, each applied to the tape the control run uses, each asserted to have changed
//! the file's bytes before its verdict was read. They are not a written record: every row below is
//! a `#[test]` in this file, so the instrument re-proves it can fail on every `cargo test`.
//!
//! | mutation of the control tape | the loader's border | what would be untested without it |
//! |---|---|---|
//! | one payload byte, `0xAA` → `0x55` | **red** — folded, compared, refused | that the fold is computed over what actually arrived |
//! | the one-bit half-period, 1200 → 500 | **red** — every bit now reads as a zero | that `ID 11`'s bit-one field drives the pulse train |
//! | the pilot half-period, 1400 → 300 | **blue** — never synced at all | that `ID 11`'s pilot field drives the pulse train |
//! | the payload replaced by 6912 zeros | **red** — see above | that the fold's expectation is not on the tape |
//!
//! The three border colours are distinct verdicts and not three flavours of failure: blue is the
//! loader still hunting for a pilot, yellow is synced and decoding, and only green and red are
//! reached after 6912 bytes have been folded. [`Verdict`] is the mapping.
//!
//! # How fast this machine will actually go, stated as what was measured
//!
//! The same loader, the same 6912-byte payload, the same instrument, at six data rates —
//! **measured on 2026-09-02**, each rate the ratio of the ROM's mean bit half-period
//! (`(855 + 1710) / 2 = 1282.5`) to the tape's. [`SPEEDS`] carries the table, and
//! [`the_loader_reads_every_rate_the_table_says_it_reads`] re-runs every row on every `cargo test`,
//! so this is a measurement the suite keeps taking rather than one it remembers:
//!
//! | rate | pilot / sync / bit0 / bit1 | with the loader as built | with its two thresholds moved |
//! |---|---|---|---|
//! | 1.50× | 1400 / 500 / 500 / 1200 | **read**, all 6912 bytes | — |
//! | 1.76× | 1200 / 450 / 450 / 1000 | **read** | — |
//! | 1.97× | 1100 / 400 / 400 / 900 | **read** | — |
//! | 2.08× | 1050 / 380 / 380 / 850 | **a green border over a wrong screen** — 1244 bytes damaged, and the fold agreed anyway | **read**, at `bit` 14 |
//! | 3.01× | 700 / 250 / 250 / 600 | **never synced** — blue | **read**, at `pilot_min` 11 and `bit` 8 |
//! | 3.56× | 600 / 220 / 220 / 500 | **never synced** — blue | **read**, at `pilot_min` 10 and `bit` 7 |
//!
//! **Every limit in the third column is the test loader's own, and the fourth column is the proof
//! and not the claim.** Each failing row is played back byte for byte unchanged, and the only thing
//! altered is an immediate inside the guest's own Z80 code; all three then read the whole screen
//! correctly. If any of it belonged to the emulator — to a rounding step in the pulse train, to a
//! floor on how short a pulse may be — then editing a constant in the guest could not have lifted
//! it. [`no_rate_in_the_table_is_refused_by_anything_but_the_loaders_own_two_constants`] is that
//! argument as a test.
//!
//! The two failure modes are localised to **different** constants, which is what makes the fourth
//! column more than a rescue: 2.08× is `bit` running out of resolution *after* a clean sync, and
//! the two fastest rows never reach a bit at all, because `pilot_min` runs out first. That is why
//! their border is blue and 2.08×'s is green.
//!
//! So the honest headline is not a ceiling but the absence of one: **at 3.56× the ROM's data rate —
//! the fastest tried — the emulator plays a tape that a guest loader reads byte-perfect**, and no
//! rate was found that it could not play. Half-periods are carried as exact [`u32`] T-state counts
//! with no rounding anywhere in the path, and the ceiling on how large a pulse train may grow is
//! 16,777,216 against the 786,432 a whole 48K tape needs.
//!
//! One row deserves reading twice. At 2.08× the loader **reported success on a corrupted screen**,
//! and it did so honestly: an `XOR` fold cannot see an even number of bits dropped from the same
//! position, and a threshold sitting inside a population is exactly what produces that. It is the
//! all-zeros hole from two sections above, met again from the other side, and it is why the gate
//! below asserts the arrived bytes as well as the border colour — see
//! [`a_fold_can_agree_while_the_screen_is_wrong`].
//!
//! # `ID 19`, and what a reader meets instead of a hang
//!
//! `ID 19 - Generalized Data Block` is the one turbo-relevant block this converter does not
//! implement. It is the format's general answer to the same problem `ID 11` solves specifically:
//! rather than a fixed pilot/sync/bit vocabulary it carries a **symbol alphabet** — a table of
//! pulse sequences, and then a stream of symbol indices — so playing it means a small machine of
//! its own.
//!
//! Not implementing it is a feature decision and is not made here. What is asserted here is the
//! behaviour a reader will actually meet, because it is the difference between a bug report and a
//! puzzle: a `.tzx` containing one is **refused by ID and byte offset**, loudly, at parse time —
//! not skipped, which would silently drop signal, and not hung on. `crates/spectrum/src/tape/mod.rs`
//! argues the refusal; [`a_generalized_data_block_is_refused_by_id_and_offset`] pins it next to the
//! turbo work, where somebody looking for it will be. **No file in this repository's corpus uses
//! one** — `tzx_corpus.rs` sweeps what is fetched, and every block the local `.tzx` games contain
//! is handled.

use spectrum::Spectrum;
use spectrum::tape::{Tape, tzx};
use spectrum::{Model, timing::T_STATES_PER_FRAME};
use z80::CpuState;

// ---------------------------------------------------------------------------------------
// The payload: a screen, and a fold that cannot be the degenerate value
// ---------------------------------------------------------------------------------------

/// The bitmap: 32 bytes across, 192 pixel rows, in the ULA's own non-linear order.
const BITMAP_BYTES: usize = 6144;
/// The attribute file: one byte per character cell, 32 across and 24 down.
const ATTRIBUTE_BYTES: usize = 768;
/// A whole Spectrum screen, which is what `0x4000..0x5B00` holds.
const SCREEN_BYTES: usize = BITMAP_BYTES + ATTRIBUTE_BYTES;

/// The fold the loader is built to expect, carried as an immediate **in its own instructions**.
///
/// Not zero, and that is the point rather than a detail: 6912 zero bytes fold to zero, so a zero
/// expectation is satisfied by a decode that never happened. See this file's header.
const EXPECTED_XOR: u8 = 0x5A;

/// Eight distinct bitmap bytes, drawn as diagonal bands so that no two neighbours are equal.
///
/// None of them is `0xFF`, and [`the_payload_cannot_counterfeit_a_pilot_tone`] is why that matters:
/// sixteen consecutive `0xFF` bytes are 256 consecutive long half-periods, which is exactly the run
/// the loader accepts as a pilot tone. A payload able to counterfeit one would make the pilot
/// mutation below prove nothing.
const STRIPES: [u8; 8] = [0x81, 0xC3, 0x66, 0x3C, 0x18, 0x24, 0x42, 0x99];

/// A screen: diagonal bands over the bitmap, a colour lattice over the attributes.
///
/// The last attribute byte — the bottom-right character cell — is **chosen** rather than drawn, so
/// that the whole screen folds to [`EXPECTED_XOR`]. Every other candidate for this job cancels:
/// 6912 is 27 whole periods of 256, so any pattern that is a bijection over a byte, and any
/// per-row constant across an even row, folds to zero by symmetry. One deliberate byte is cheaper
/// than a pattern contrived to avoid that, and it is checkable at compile time.
const fn loading_screen() -> [u8; SCREEN_BYTES] {
    let mut screen = [0u8; SCREEN_BYTES];
    let mut fold = 0u8;
    let mut index = 0;

    // The bitmap. The screen address is not linear: byte `third*2048 + line*256 + row*32 + column`
    // is pixel row `third*64 + row*8 + line`, which is why a Spectrum screen loads in thirds.
    while index < BITMAP_BYTES {
        let column = index % 32;
        let row = (index / 32) % 8;
        let line = (index / 256) % 8;
        let third = index / 2048;
        let y = third * 64 + row * 8 + line;
        screen[index] = STRIPES[(y + column) % STRIPES.len()];
        fold ^= screen[index];
        index += 1;
    }

    // The attribute file: ink in bits 0-2, paper in bits 3-5, so this is a lattice of colours.
    while index < SCREEN_BYTES - 1 {
        let cell = index - BITMAP_BYTES;
        let ink = (cell / 32) % 8;
        let paper = (cell % 32) % 8;
        screen[index] = (ink | (paper << 3)) as u8;
        fold ^= screen[index];
        index += 1;
    }

    screen[SCREEN_BYTES - 1] = fold ^ EXPECTED_XOR;
    screen
}

/// The screen every tape in this file carries, unless the test is mutating it.
const SCREEN: [u8; SCREEN_BYTES] = loading_screen();

/// The `XOR` of every byte, which is the only checksum the loader computes.
const fn xor_fold(bytes: &[u8]) -> u8 {
    let mut fold = 0u8;
    let mut index = 0;
    while index < bytes.len() {
        fold ^= bytes[index];
        index += 1;
    }
    fold
}

const _: () = assert!(
    xor_fold(&SCREEN) == EXPECTED_XOR,
    "the loader's baked-in constant must be the fold of the screen it is given"
);
const _: () = assert!(
    EXPECTED_XOR != 0,
    "a zero expectation is satisfied by an all-zeros decode, which is the hole this file records"
);

// ---------------------------------------------------------------------------------------
// The tape: `ID 11` with every timing named
// ---------------------------------------------------------------------------------------

/// The five half-periods and the pilot count that `ID 11` carries as fields.
///
/// A `.tap` has none of these; it is why `.tzx` exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Timings {
    pilot: u16,
    pilot_pulses: u16,
    sync_first: u16,
    sync_second: u16,
    bit_zero: u16,
    bit_one: u16,
}

/// Genuinely turbo: every value below the ROM's, and the bit rate about half again as fast.
///
/// The sync pair is a *pair of equal* half-periods, which the ROM's 667/735 never is — so this is
/// not the standard sequence at a different speed, it is a different sequence.
const TURBO: Timings = Timings {
    pilot: 1400,
    pilot_pulses: 2000,
    sync_first: 500,
    sync_second: 500,
    bit_zero: 500,
    bit_one: 1200,
};

/// The ROM's own values, for the contrast the header draws. Never played here.
const ROM_PILOT: u32 = 2168;
const ROM_SYNC_FIRST: u32 = 667;
const ROM_SYNC_SECOND: u32 = 735;
const ROM_BIT_ZERO: u32 = 855;
const ROM_BIT_ONE: u32 = 1710;

/// The ROM's mean bit half-period, doubled so the ratios below stay in integers.
///
/// `(855 + 1710) / 2` is 1282.5, which is why every rate in [`SPEEDS`] is written as a pair of
/// integers rather than as a float: `2 * (855 + 1710) / 2` over `2 * (zero + one) / 2` is
/// `(855 + 1710)` over `(zero + one)`, and no rounding enters the comparison.
const ROM_BIT_SUM: u32 = ROM_BIT_ZERO + ROM_BIT_ONE;

/// The pause after the block, in milliseconds. Long enough to be a real gap, short enough to cost
/// nothing: the decode is finished before it starts.
const PAUSE_MS: u16 = 100;

/// The ten-byte `.tzx` header: `"ZXTape!"`, the end-of-text marker, and revision 1.20.
const HEADER: [u8; 10] = [b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14];

/// A three-byte little-endian length, which is how `ID 11`, `ID 14` and `ID 15` size their bodies.
fn length_24(bytes: &[u8]) -> Vec<u8> {
    let length = u32::try_from(bytes.len()).expect("a screen-sized block");
    length
        .to_le_bytes()
        .get(..3)
        .expect("a 24-bit length")
        .to_vec()
}

/// A `.tzx` holding one `ID 11 - Turbo Speed Data Block`, field by field from the description.
///
/// Unlike `tzx_rom_load.rs`'s namesake this carries no flag byte and no parity byte: those belong
/// to the ROM's block format, and a loader that is not the ROM's owes them nothing. The payload is
/// the screen and only the screen.
fn turbo_speed(timings: Timings, payload: &[u8]) -> Vec<u8> {
    let mut file = HEADER.to_vec();
    file.push(0x11);
    file.extend(timings.pilot.to_le_bytes()); //        0x00 WORD pilot half-period
    file.extend(timings.sync_first.to_le_bytes()); //   0x02 WORD first sync half-period
    file.extend(timings.sync_second.to_le_bytes()); //  0x04 WORD second sync half-period
    file.extend(timings.bit_zero.to_le_bytes()); //     0x06 WORD a zero bit's half-period
    file.extend(timings.bit_one.to_le_bytes()); //      0x08 WORD a one bit's half-period
    file.extend(timings.pilot_pulses.to_le_bytes()); // 0x0A WORD how many pilot pulses
    file.push(8); //                                    0x0C BYTE used bits in the last byte
    file.extend(PAUSE_MS.to_le_bytes()); //             0x0D WORD pause after the block, ms
    file.extend(length_24(payload)); //                 0x0F BYTE[3] payload length
    file.extend_from_slice(payload); //                 0x12 BYTE[N]
    file
}

/// The same signal written as `ID 12` (pure tone) + `ID 13` (pulse sequence) + `ID 14` (pure data).
///
/// This is the shape a real turbo loader's tape actually has, because a non-standard sync pair can
/// only be written down as its own block. `tzx_rom_load.rs` grades the three primitives at the
/// ROM's values, where a converter that ignored their fields would still pass; here they carry
/// turbo values and nothing in the ROM can read the result.
fn assembled_from_primitives(timings: Timings, payload: &[u8]) -> Vec<u8> {
    let mut file = HEADER.to_vec();

    // ID 12 - Pure Tone: 0x00 WORD one pulse's length, 0x02 WORD how many.
    file.push(0x12);
    file.extend(timings.pilot.to_le_bytes());
    file.extend(timings.pilot_pulses.to_le_bytes());

    // ID 13 - Pulse sequence: 0x00 BYTE how many pulses, 0x01 WORD[N] their lengths.
    file.push(0x13);
    file.push(2);
    file.extend(timings.sync_first.to_le_bytes());
    file.extend(timings.sync_second.to_le_bytes());

    // ID 14 - Pure Data Block: 0x00 WORD zero bit, 0x02 WORD one bit, 0x04 BYTE used bits,
    // 0x05 WORD pause, 0x07 BYTE[3] length, 0x0A BYTE[N] data.
    file.push(0x14);
    file.extend(timings.bit_zero.to_le_bytes());
    file.extend(timings.bit_one.to_le_bytes());
    file.push(8);
    file.extend(PAUSE_MS.to_le_bytes());
    file.extend(length_24(payload));
    file.extend_from_slice(payload);
    file
}

/// How long one sample of a direct recording lasts, in T-states.
///
/// 100 divides every half-period in [`TURBO`] exactly — 1400, 500 and 1200 are 14, 5 and 12
/// samples — so the recording can reconstruct the train without rounding. That is a property of
/// these numbers and is asserted rather than assumed.
const SAMPLE_T_STATES: u16 = 100;

/// The same signal again, as `ID 15 - Direct Recording`: raw `EAR` samples, one bit each.
///
/// No pilot field, no sync field, no bit field — nothing but levels and a sample rate. If the
/// train this produces is the same train as `ID 11`'s, then `ID 11`'s five fields were read
/// correctly, because there is no shared code path by which both could be wrong the same way.
fn direct_recording(timings: Timings, payload: &[u8]) -> Vec<u8> {
    /// Hold the current level for one half-period's worth of samples, then flip it.
    fn hold(half_period: u16, level: &mut bool, samples: &mut Vec<bool>) {
        assert_eq!(
            half_period % SAMPLE_T_STATES,
            0,
            "{half_period} T-states is not a whole number of {SAMPLE_T_STATES}-T-state samples"
        );
        for _ in 0..half_period / SAMPLE_T_STATES {
            samples.push(*level);
        }
        *level = !*level;
    }

    let mut samples: Vec<bool> = Vec::new();
    let mut level = false;
    for _ in 0..timings.pilot_pulses {
        hold(timings.pilot, &mut level, &mut samples);
    }
    hold(timings.sync_first, &mut level, &mut samples);
    hold(timings.sync_second, &mut level, &mut samples);
    for &byte in payload {
        for bit in (0..8).rev() {
            let half = if byte & (1 << bit) == 0 {
                timings.bit_zero
            } else {
                timings.bit_one
            };
            hold(half, &mut level, &mut samples);
            hold(half, &mut level, &mut samples);
        }
    }

    let used_bits = match samples.len() % 8 {
        0 => 8,
        remainder => remainder,
    };
    let mut bits = Vec::with_capacity(samples.len().div_ceil(8));
    for chunk in samples.chunks(8) {
        let mut byte = 0u8;
        for (position, &sample) in chunk.iter().enumerate() {
            if sample {
                byte |= 0x80 >> position;
            }
        }
        bits.push(byte);
    }

    let mut file = HEADER.to_vec();
    file.push(0x15);
    file.extend(SAMPLE_T_STATES.to_le_bytes()); // 0x00 WORD T-states per sample
    file.extend(PAUSE_MS.to_le_bytes()); //        0x02 WORD pause after the block, ms
    file.push(u8::try_from(used_bits).expect("one to eight")); // 0x04 BYTE used bits, last byte
    file.extend(length_24(&bits)); //              0x05 BYTE[3]
    file.extend(bits); //                          0x08 BYTE[N]
    file
}

/// The same signal a fourth time, reached only by following the format's control-flow blocks.
///
/// A text description and a group wrap it, and an `ID 23` jump steps **over a decoy pure tone**
/// whose pulse length appears nowhere else. If the jump were ignored the decoy's pulses would be
/// in the train and the comparison would fail; if the jump over-shot, the pilot would be. That is
/// what makes this a test of the jump rather than of the fact that a file parsed.
const DECOY_PULSE: u16 = 9999;
const DECOY_PULSES: u16 = 64;

/// The `ID 30` text and the `ID 21` group name that wrap the signal. Neither carries a pulse.
const DESCRIPTION: &[u8] = b"turbo, behind a jump";
const GROUP: &[u8] = b"turbo";

fn reached_by_control_flow(timings: Timings, payload: &[u8]) -> Vec<u8> {
    let description = DESCRIPTION;
    let group = GROUP;
    let mut file = HEADER.to_vec();

    // ID 30 - Text description: 0x00 BYTE length, 0x01 CHAR[N].
    file.push(0x30);
    file.push(u8::try_from(description.len()).expect("a short description"));
    file.extend_from_slice(description);

    // ID 21 - Group start: 0x00 BYTE length, 0x01 CHAR[N].
    file.push(0x21);
    file.push(u8::try_from(group.len()).expect("a short name"));
    file.extend_from_slice(group);

    // ID 23 - Jump to block: 0x00 WORD a signed relative block offset, where 1 is the next block.
    // 2 therefore steps over exactly one block, which is the decoy that follows.
    file.push(0x23);
    file.extend(2_u16.to_le_bytes());

    // ID 12 - Pure Tone, the decoy. Never played, and its length is unlike every other here.
    file.push(0x12);
    file.extend(DECOY_PULSE.to_le_bytes());
    file.extend(DECOY_PULSES.to_le_bytes());

    // The signal itself, as the three primitives, minus their own header.
    file.extend_from_slice(
        assembled_from_primitives(timings, payload)
            .get(HEADER.len()..)
            .expect("a header and a body"),
    );

    file.push(0x22); // ID 22 - Group end
    file
}

fn tape_from(file: &[u8]) -> Tape {
    tzx::parse(file, Model::Spectrum48K).expect("a well-formed .tzx")
}

// ---------------------------------------------------------------------------------------
// The loader: 124 bytes of Z80, assembled here so every one of them is checkable
// ---------------------------------------------------------------------------------------

/// Where the loader is placed: uncontended RAM, clear of the screen it decodes into.
const LOADER: u16 = 0x8000;
/// Its stack. Contended, deliberately — see [`assemble`].
const STACK: u16 = 0x7FF0;
/// The first address that is RAM rather than ROM on a 48K.
///
/// The screen happens to start here too, which is a fact about the machine and not a coincidence
/// worth collapsing: `PC` below this is in the ROM, and this file's loader never goes there.
const FIRST_RAM_ADDRESS: u16 = 0x4000;
/// Where the decoded screen goes, which is the real screen.
const SCREEN_BASE: u16 = FIRST_RAM_ADDRESS;
/// One past its end: `0x4000 + 6912`. The decode loop stops when `H` reaches the high byte.
const SCREEN_END: u16 = 0x5B00;

/// The loader's two decision constants, both counts of polls rather than T-states.
///
/// They are the *whole* of its resolution: `pilot_min` decides whether a half-period is long
/// enough to belong to a pilot tone, and `bit` decides whether one is long enough to be a one bit.
/// Every failure in [`SPEEDS`] is one of these two running out, which is the point
/// [`no_rate_in_the_table_is_refused_by_anything_but_the_loaders_own_two_constants`] makes by
/// moving them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Thresholds {
    pilot_min: u8,
    bit: u8,
}

impl Thresholds {
    /// What the loader is normally built with.
    ///
    /// The populations that have to be separated are, in polls: a 1400-T-state pilot near 26, a
    /// 1200-T-state one bit near 23, and a 500-T-state zero bit near 9. 20 and 16 sit in the two
    /// gaps. Those figures follow from an effective poll cost near **50 T-states** — the 43 of
    /// Z80 time listed in this file's header, plus the ULA's I/O contention on `IN A,($FE)` — and
    /// that cost is not asserted from a hand count but bracketed by three measured boundaries:
    ///
    /// - a 500-T-state zero bit is still read as a zero at `bit` = 12 and is read as a **one** at
    ///   `bit` = 10, so its poll count is between 10 and 12, putting the cost near `500 / 11`;
    /// - an 850-T-state one bit sits exactly on `bit` = 16 (see [`Outcome::SilentlyWrong`]),
    ///   putting the cost near `850 / 16`;
    /// - a 700-T-state pilot fails `pilot_min` = 20 and a 1050-T-state one passes it, putting the
    ///   cost between `700 / 20` and `1050 / 20`.
    ///
    /// All three brackets contain 50, and no hand-counted figure was needed to get there.
    const DEFAULT: Self = Self {
        pilot_min: 20,
        bit: 16,
    };
}

/// How many consecutive long half-periods the loader insists on before it believes a pilot.
///
/// 256, counted in `D`, which is what makes `INC D` / `JR NZ` a loop of exactly that many — so a
/// stray long pulse in the data cannot be mistaken for a pilot tone.
const PILOT_RUN: usize = 256;

/// The border colours the loader paints, which are its only output.
///
/// Distinct verdicts rather than degrees of failure: blue and yellow are reached *before* any
/// fold, so they say the loader never got far enough to have an opinion about the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Blue. Still hunting for a pilot tone, and never found one.
    Hunting,
    /// Yellow. Synced, and decoding, but it never finished the screen.
    Decoding,
    /// Green. 6912 bytes decoded and folded to the constant in the loader's own code.
    Loaded,
    /// Red. 6912 bytes decoded, and they folded to something else.
    Corrupt,
}

impl Verdict {
    const HUNTING: u8 = 1; // blue
    const DECODING: u8 = 6; // yellow
    const LOADED: u8 = 4; // green
    const CORRUPT: u8 = 2; // red

    fn of(machine: &Spectrum) -> Self {
        match machine.border().index() {
            Self::HUNTING => Self::Hunting,
            Self::DECODING => Self::Decoding,
            Self::LOADED => Self::Loaded,
            Self::CORRUPT => Self::Corrupt,
            other => panic!("the loader painted {other}, which it has no instruction to paint"),
        }
    }
}

/// A two-pass assembler over the handful of `JR`s and `CALL`s the loader needs.
///
/// It exists so that the loader can be read as instructions rather than as an array of hex. Every
/// `emit` below carries its mnemonic, so a reader can check the opcode against a Z80 table without
/// running anything; the assembler's only job is the arithmetic on the jump displacements, which is
/// the part a human gets wrong.
struct Assembler {
    origin: u16,
    code: Vec<u8>,
    labels: Vec<(&'static str, u16)>,
    patches: Vec<Patch>,
}

struct Patch {
    /// The index in `code` of the displacement or address byte to fill in.
    at: usize,
    label: &'static str,
    /// `Some(pc)` for a relative jump, holding the address the displacement counts from.
    relative_to: Option<u16>,
}

impl Assembler {
    fn new(origin: u16) -> Self {
        Self {
            origin,
            code: Vec::new(),
            labels: Vec::new(),
            patches: Vec::new(),
        }
    }

    /// The address the next byte will land at.
    fn here(&self) -> u16 {
        self.origin + u16::try_from(self.code.len()).expect("a loader of a few hundred bytes")
    }

    fn label(&mut self, name: &'static str) {
        let here = self.here();
        self.labels.push((name, here));
    }

    fn emit(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    /// A two-byte relative jump: `opcode` then a displacement counted from the following byte.
    fn jr(&mut self, opcode: u8, label: &'static str) {
        self.emit(&[opcode, 0]);
        self.patches.push(Patch {
            at: self.code.len() - 1,
            label,
            relative_to: Some(self.here()),
        });
    }

    /// `CALL nn`, whose operand is absolute.
    fn call(&mut self, label: &'static str) {
        self.emit(&[0xCD, 0, 0]);
        self.patches.push(Patch {
            at: self.code.len() - 2,
            label,
            relative_to: None,
        });
    }

    fn address_of(&self, label: &str) -> u16 {
        self.labels
            .iter()
            .find(|(name, _)| *name == label)
            .map(|&(_, address)| address)
            .unwrap_or_else(|| panic!("the loader has no label {label}"))
    }

    fn finish(mut self) -> Program {
        for patch in &self.patches {
            let target = self
                .labels
                .iter()
                .find(|(name, _)| *name == patch.label)
                .map(|&(_, address)| address)
                .unwrap_or_else(|| panic!("the loader has no label {}", patch.label));
            match patch.relative_to {
                None => {
                    let [low, high] = target.to_le_bytes();
                    self.code[patch.at] = low;
                    self.code[patch.at + 1] = high;
                }
                Some(from) => {
                    let displacement = i32::from(target) - i32::from(from);
                    let displacement = i8::try_from(displacement).unwrap_or_else(|_| {
                        panic!(
                            "{} is {displacement} away, which no JR reaches",
                            patch.label
                        )
                    });
                    self.code[patch.at] = displacement.to_le_bytes()[0];
                }
            }
        }
        Program {
            stop: self.address_of("STOP"),
            code: self.code,
        }
    }
}

/// The assembled loader and the one address a caller needs to watch.
struct Program {
    code: Vec<u8>,
    /// The `JR` to itself the loader parks in once it has painted its verdict.
    stop: u16,
}

/// The loader, instruction by instruction.
///
/// It is a real turbo loader in miniature and does what the commercial ones do: measure the
/// interval between `EAR` transitions by counting a polling loop, insist on a long run of long
/// intervals before believing a pilot tone, take the shorter sync pair as the start of data, then
/// read bits by comparing each interval against a threshold.
///
/// Three details are deliberate rather than incidental:
///
/// - **`DI` first, and interrupts never come back on.** Nothing in this file may depend on the
///   ROM, and an interrupt would enter it.
/// - **The stack is at `0x7FF0`, which is contended**, and the decode target is `0x4000`, which is
///   contended too. Both were left that way on purpose: it means the `PUSH AF`/`POP AF` inside a
///   bit measurement and the `LD (HL),E` between bytes are charged the ULA's contention, so the
///   decode is happening *through* the contention model rather than beside it. It is safe because
///   both fall between measurements rather than inside one — only the first half-period of a bit is
///   measured, and `EDGE` waits for a level change rather than for a deadline, so a few T-states of
///   delay cannot desynchronise it.
/// - **`RET Z` in the polling loop is the only timeout there is.** `B` wraps to zero after 256
///   polls and the routine gives up, which is what stops a finished tape from hanging the machine.
fn assemble(thresholds: Thresholds) -> Program {
    let Thresholds { pilot_min, bit } = thresholds;
    let mut asm = Assembler::new(LOADER);

    asm.emit(&[0xF3]); //                             DI
    asm.emit(&[0x31]); //                             LD SP,nn
    asm.emit(&STACK.to_le_bytes());
    asm.emit(&[0x3E, Verdict::HUNTING]); //           LD A,1
    asm.emit(&[0xD3, 0xFE]); //                       OUT ($FE),A    border blue: hunting
    asm.emit(&[0x0E, 0x00]); //                       LD C,0         C holds the last EAR level

    // Hunt for a pilot tone: a run of PILOT_RUN consecutive long half-periods.
    asm.label("PILOT");
    asm.call("EDGE"); //                              CALL EDGE
    asm.emit(&[0x78]); //                             LD A,B
    asm.emit(&[0xFE, pilot_min]); //                  CP PILOT_MIN
    asm.jr(0x38, "PILOT"); //                         JR C,PILOT     too short: keep hunting
    asm.emit(&[0x16, 0x00]); //                       LD D,0         D counts the run, mod 256
    asm.label("PLOOP");
    asm.call("EDGE"); //                              CALL EDGE
    asm.emit(&[0x78]); //                             LD A,B
    asm.emit(&[0xFE, pilot_min]); //                  CP PILOT_MIN
    asm.jr(0x38, "PILOT"); //                         JR C,PILOT     the run broke: start over
    asm.emit(&[0x14]); //                             INC D
    asm.jr(0x20, "PLOOP"); //                         JR NZ,PLOOP    until D wraps: PILOT_RUN of them

    // The sync pair is the first half-period *shorter* than a pilot's.
    asm.label("SYNC");
    asm.call("EDGE"); //                              CALL EDGE
    asm.emit(&[0x78]); //                             LD A,B
    asm.emit(&[0xFE, pilot_min]); //                  CP PILOT_MIN
    asm.jr(0x30, "SYNC"); //                          JR NC,SYNC     still pilot: keep waiting
    asm.call("EDGE"); //                              CALL EDGE      consume the second sync half
    asm.emit(&[0x3E, Verdict::DECODING]); //          LD A,6
    asm.emit(&[0xD3, 0xFE]); //                       OUT ($FE),A    border yellow: decoding

    // Decode SCREEN_BYTES bytes straight into the screen.
    asm.emit(&[0x21]); //                             LD HL,nn
    asm.emit(&SCREEN_BASE.to_le_bytes());
    asm.label("NEXT");
    asm.call("RDBYTE"); //                            CALL RDBYTE
    asm.emit(&[0x73]); //                             LD (HL),E
    asm.emit(&[0x23]); //                             INC HL
    asm.emit(&[0x7C]); //                             LD A,H
    asm.emit(&[0xFE, SCREEN_END.to_le_bytes()[1]]); // CP $5B
    asm.jr(0x20, "NEXT"); //                          JR NZ,NEXT

    // Fold what arrived, and compare against the constant carried in this instruction stream.
    asm.emit(&[0x21]); //                             LD HL,nn
    asm.emit(&SCREEN_BASE.to_le_bytes());
    asm.emit(&[0x01]); //                             LD BC,nn
    asm.emit(&u16::try_from(SCREEN_BYTES).expect("a screen").to_le_bytes());
    asm.emit(&[0x1E, 0x00]); //                       LD E,0
    asm.label("CKS");
    asm.emit(&[0x7E]); //                             LD A,(HL)
    asm.emit(&[0xAB]); //                             XOR E
    asm.emit(&[0x5F]); //                             LD E,A
    asm.emit(&[0x23]); //                             INC HL
    asm.emit(&[0x0B]); //                             DEC BC
    asm.emit(&[0x78]); //                             LD A,B
    asm.emit(&[0xB1]); //                             OR C
    asm.jr(0x20, "CKS"); //                           JR NZ,CKS
    asm.emit(&[0x7B]); //                             LD A,E
    asm.emit(&[0xFE, EXPECTED_XOR]); //               CP EXPECTED_XOR   <- the constant, in the code
    asm.emit(&[0x3E, Verdict::LOADED]); //            LD A,4
    asm.jr(0x28, "OK"); //                            JR Z,OK
    asm.emit(&[0x3E, Verdict::CORRUPT]); //           LD A,2
    asm.label("OK");
    asm.emit(&[0xD3, 0xFE]); //                       OUT ($FE),A    green or red
    asm.label("STOP");
    asm.jr(0x18, "STOP"); //                          JR STOP

    // EDGE: poll until bit 6 of port 0xFE differs from C, leaving the poll count in B.
    asm.label("EDGE");
    asm.emit(&[0x06, 0x00]); //                       LD B,0
    asm.label("E1");
    asm.emit(&[0x04]); //                             INC B
    asm.emit(&[0xC8]); //                             RET Z          B wrapped: no edge came
    asm.emit(&[0xDB, 0xFE]); //                       IN A,($FE)
    asm.emit(&[0xE6, 0x40]); //                       AND $40        the EAR bit
    asm.emit(&[0xA9]); //                             XOR C
    asm.jr(0x28, "E1"); //                            JR Z,E1        unchanged: keep counting
    asm.emit(&[0x79]); //                             LD A,C
    asm.emit(&[0xEE, 0x40]); //                       XOR $40
    asm.emit(&[0x4F]); //                             LD C,A         remember the new level
    asm.emit(&[0xC9]); //                             RET

    // RDBYTE: one byte into E, most significant bit first.
    asm.label("RDBYTE");
    asm.emit(&[0x1E, 0x01]); //                       LD E,1         a sentinel, shifted out after 8
    asm.label("RDBIT");
    asm.call("EDGE"); //                              CALL EDGE      measure the first half-period
    asm.emit(&[0x78]); //                             LD A,B
    asm.emit(&[0xF5]); //                             PUSH AF        keep it across the second
    asm.call("EDGE"); //                              CALL EDGE      and burn the second half
    asm.emit(&[0xF1]); //                             POP AF
    asm.emit(&[0xFE, bit]); //                        CP BIT_THRESH  carry set iff A < threshold
    asm.emit(&[0x3F]); //                             CCF            so carry now means 'a one'
    asm.emit(&[0xCB, 0x13]); //                       RL E           shift it in, sentinel out
    asm.jr(0x30, "RDBIT"); //                         JR NC,RDBIT    sentinel still inside: go again
    asm.emit(&[0xC9]); //                             RET

    asm.finish()
}

/// How many bytes the loader assembles to.
///
/// Named because it moved: the first version of this loader was **130** bytes, and six of them
/// were the flaw — a `CALL RDBYTE` that read a checksum byte off the tape and a `LD HL,$5B00` /
/// `CP (HL)` that compared against it. Replacing those with a `CP` against an immediate is what
/// made the all-zeros decode impossible, and it is also what made the loader shorter.
const LOADER_BYTES: usize = 124;

// ---------------------------------------------------------------------------------------
// Running it
// ---------------------------------------------------------------------------------------

/// T-states to allow. The control tape is a 2000-pulse pilot plus 6912 bytes at roughly 1700
/// T-states a bit — about 97 million — so this is enough to play the whole of it and a third again.
const BUDGET: u64 = 130_000_000;

/// A blank ROM page.
///
/// The loader never enters it: it is reached only by an interrupt or by a wild jump, the first is
/// disabled before anything else happens and the second would be a failure this file wants to see
/// rather than survive. Using a blank page rather than the committed Sinclair ROM is what makes
/// this gate need **no corpus at all** — `testdata/**` is gitignored, so a gate that read from it
/// could never run on a clean checkout.
fn blank_rom() -> [u8; 16 * 1024] {
    [0; 16 * 1024]
}

fn write_bytes(machine: &mut Spectrum, at: u16, bytes: &[u8]) {
    for (offset, &byte) in (0..).zip(bytes) {
        machine.memory_mut().write(at.wrapping_add(offset), byte);
    }
}

fn read_bytes(machine: &Spectrum, at: u16, length: usize) -> Vec<u8> {
    (0..length)
        .map(|offset| {
            let offset = u16::try_from(offset).expect("a screen-sized read");
            machine.memory().read(at.wrapping_add(offset))
        })
        .collect()
}

fn elapsed(machine: &Spectrum) -> u64 {
    machine.frames() * u64::from(T_STATES_PER_FRAME) + u64::from(machine.frame_t_state())
}

/// What one run of the loader against one tape produced.
struct Run {
    machine: Spectrum,
    verdict: Verdict,
    /// Whether `PC` ever left RAM. It must not: this file does not use the ROM.
    stayed_in_ram: bool,
}

impl Run {
    /// The screen as it stands in memory, which is what the loader actually wrote.
    fn screen(&self) -> Vec<u8> {
        read_bytes(&self.machine, SCREEN_BASE, SCREEN_BYTES)
    }
}

/// Load `tape` with the loader as it is normally built.
fn run(tape: Tape) -> Run {
    run_with(tape, Thresholds::DEFAULT)
}

/// Load `tape` with a loader built to `thresholds` rather than to the usual ones.
///
/// Both thresholds are the loader's own, so this is the seam that separates *"the emulator cannot
/// play this tape"* from *"these six instructions cannot resolve it"* — see
/// [`no_rate_in_the_table_is_refused_by_anything_but_the_loaders_own_two_constants`], which is the
/// only caller that passes anything but [`Thresholds::DEFAULT`]. Nothing about the machine or the
/// tape changes between the two calls.
fn run_with(tape: Tape, thresholds: Thresholds) -> Run {
    let program = assemble(thresholds);
    assert_eq!(
        program.code.len(),
        LOADER_BYTES,
        "the loader changed size; if that is intended, say so in LOADER_BYTES' documentation"
    );

    let mut machine = Spectrum::new(&blank_rom()).expect("a blank page is still one page");
    write_bytes(&mut machine, LOADER, &program.code);
    machine.set_cpu_state(CpuState {
        pc: LOADER,
        sp: STACK,
        iff1: false,
        iff2: false,
        ..CpuState::default()
    });
    machine.insert_tape(tape);
    machine.tape_mut().play();

    let start = elapsed(&machine);
    let mut stayed_in_ram = true;
    while elapsed(&machine) - start < BUDGET {
        let pc = machine.cpu_state().pc;
        stayed_in_ram &= pc >= FIRST_RAM_ADDRESS;
        if pc == program.stop {
            break;
        }
        machine.step();
    }

    let verdict = Verdict::of(&machine);
    Run {
        machine,
        verdict,
        stayed_in_ram,
    }
}

// ---------------------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------------------

#[test]
fn a_turbo_block_the_rom_cannot_read_is_decoded_by_a_loader_of_our_own() {
    // The milestone this file exists for. Every timing in the tape is below the ROM's, the sync
    // pair is a shape the ROM never emits, and a whole 6912-byte screen arrives byte for byte.
    //
    // Two assertions rather than one, and the pair is the point: the **border** is the loader's own
    // verdict, reached by folding what it decoded against a constant in its own instruction stream,
    // and the **byte comparison** is this file's, owing nothing to the loader's arithmetic. A fold
    // is a weak equality — it cannot see two bytes swapped — so the fold alone would not do; and
    // the byte comparison alone would not exercise the mechanism every mutation below moves.
    let file = turbo_speed(TURBO, &SCREEN);
    let result = run(tape_from(&file));

    assert_eq!(
        result.verdict,
        Verdict::Loaded,
        "the loader did not report a clean decode"
    );
    assert_eq!(
        result.screen(),
        SCREEN,
        "the screen in memory is not the screen on the tape"
    );
    assert!(
        result.stayed_in_ram,
        "the loader entered the ROM, so this run was not the loader's own work"
    );
    assert_eq!(result.machine.fault(), None);
}

#[test]
fn the_tape_this_gate_plays_is_genuinely_faster_than_the_rom_can_follow() {
    // The positive control for the claim in the file's name, kept separate from the run above so
    // that "it loaded" and "it was turbo" are two findings and not one. Without this, a tape that
    // had quietly been given the ROM's own timings would still pass everything below.
    assert!(
        u32::from(TURBO.bit_zero) < ROM_BIT_ZERO && u32::from(TURBO.bit_one) < ROM_BIT_ONE,
        "the bits must be shorter than the ROM's, or nothing here is turbo"
    );
    assert!(
        u32::from(TURBO.pilot) < ROM_PILOT,
        "the pilot must be shorter than the ROM's too"
    );
    // And it is not merely the standard sequence played faster: the ROM's sync pair is two
    // *different* half-periods, 667 and 735, and this one is two equal ones.
    assert_eq!(
        TURBO.sync_first, TURBO.sync_second,
        "this tape's sync pair is two equal half-periods"
    );
    assert_ne!(
        (u32::from(TURBO.sync_first), u32::from(TURBO.sync_second)),
        (ROM_SYNC_FIRST, ROM_SYNC_SECOND),
        "and it is therefore a shape the ROM never emits"
    );
}

#[test]
fn the_payload_cannot_counterfeit_a_pilot_tone() {
    // The loader accepts 256 consecutive long half-periods as a pilot. A run of sixteen 0xFF bytes
    // is 128 one-bits, which is 256 long half-periods — so a payload containing one could sync the
    // loader by itself and the pilot mutation below would prove nothing about the pilot field.
    assert!(
        !SCREEN.contains(&0xFF),
        "the screen must contain no 0xFF byte, so no run of them can counterfeit a pilot"
    );

    // The precise statement, since a run of one bits is what matters and a run of equal bytes is
    // only a proxy for it: every one bit is two long half-periods, so the loader needs
    // `PILOT_RUN / 2` consecutive one bits before it would believe the payload was a pilot tone.
    let (longest, _) = SCREEN
        .iter()
        .flat_map(|byte| (0..8).rev().map(move |bit| byte & (1 << bit) != 0))
        .fold((0_usize, 0_usize), |(longest, current), is_one| {
            let current = if is_one { current + 1 } else { 0 };
            (longest.max(current), current)
        });
    assert!(
        longest * 2 < PILOT_RUN,
        "the longest run of one bits is {longest}, which is {} long half-periods and too close to \
         the {PILOT_RUN} the loader accepts as a pilot",
        longest * 2
    );
}

// ---------------------------------------------------------------------------------------
// The negative controls — the mutation table, as tests
// ---------------------------------------------------------------------------------------

/// Which byte of the payload the first mutation moves. Chosen inside the bitmap, well past the
/// point at which a loader that had merely started would still be reading.
const MUTATED_BYTE: usize = 3000;

#[test]
fn one_wrong_payload_byte_is_caught_by_the_fold() {
    // Mutation 1. The fold is computed over what actually arrived, so a single byte that is not
    // what the tape carried must reach the loader's comparison and fail it.
    let mut payload = SCREEN;
    let before = payload[MUTATED_BYTE];
    payload[MUTATED_BYTE] ^= 0xFF;
    assert_ne!(
        before, payload[MUTATED_BYTE],
        "the mutation did not land, so what follows would prove nothing"
    );
    assert_ne!(xor_fold(&payload), EXPECTED_XOR);

    let result = run(tape_from(&turbo_speed(TURBO, &payload)));
    assert_eq!(
        result.verdict,
        Verdict::Corrupt,
        "one wrong byte in 6912 went unnoticed"
    );
    // And it is wrong in exactly the place it was made wrong: the loader read the whole screen and
    // objected to its contents, rather than falling over somewhere and objecting to that.
    assert_eq!(result.screen(), payload);
}

#[test]
fn the_files_one_bit_length_is_what_drives_the_pulse_train() {
    // Mutation 2. `ID 11`'s bit-one field set equal to its bit-zero field: the tape now carries the
    // same bytes, at a pilot and sync the loader still syncs on, but with no way to tell a one from
    // a zero. Every bit reads as a zero, so a screen of zeros arrives and folds to zero.
    //
    // This is the assertion that the field is *read*. A converter that ignored it and emitted 1710
    // would produce an identical file and a green run.
    let mutated = Timings {
        bit_one: TURBO.bit_zero,
        ..TURBO
    };
    assert_ne!(
        turbo_speed(TURBO, &SCREEN),
        turbo_speed(mutated, &SCREEN),
        "the mutation did not land, so what follows would prove nothing"
    );

    let result = run(tape_from(&turbo_speed(mutated, &SCREEN)));
    assert_eq!(
        result.verdict,
        Verdict::Corrupt,
        "the bit-one field made no difference to the signal"
    );
    assert!(
        result.screen().iter().all(|&byte| byte == 0),
        "with both bit lengths equal every bit must read as a zero"
    );
}

#[test]
fn the_files_pilot_length_is_what_the_loader_syncs_on() {
    // Mutation 3, and its verdict is a different colour from the other two on purpose. The pilot
    // half-period is shortened below the loader's threshold, so the run of long edges it waits for
    // never happens: it never syncs, never paints yellow, and is still hunting when the tape ends.
    //
    // Blue rather than red is what makes this a test of the *pilot* field specifically. A red would
    // mean it had synced and decoded badly, which is a different failure.
    let mutated = Timings {
        pilot: 300,
        ..TURBO
    };
    assert_ne!(
        turbo_speed(TURBO, &SCREEN),
        turbo_speed(mutated, &SCREEN),
        "the mutation did not land, so what follows would prove nothing"
    );

    let result = run(tape_from(&turbo_speed(mutated, &SCREEN)));
    assert_eq!(
        result.verdict,
        Verdict::Hunting,
        "the loader synced on a pilot tone it should not have recognised"
    );
    assert!(
        result.screen().iter().all(|&byte| byte == 0),
        "nothing may be written before the loader has synced"
    );
}

#[test]
fn an_all_zeros_decode_is_refused_rather_than_folding_to_the_expected_value() {
    // The flaw this file's header records, made permanent. The tape carries 6912 zero bytes, so
    // the screen the loader writes is byte-identical to the screen it would have if it had decoded
    // **nothing at all** — and the fold of that screen is zero.
    //
    // The first version of this instrument scored green here, because the expected fold was on the
    // tape and was also zero. It scores red now because the expectation is an immediate inside the
    // loader's own instructions, and 0x5A is not 0.
    let zeros = [0u8; SCREEN_BYTES];
    assert_eq!(xor_fold(&zeros), 0, "an all-zeros decode folds to zero");
    assert_ne!(
        EXPECTED_XOR, 0,
        "and the expectation must not, or this test is the hole rather than the gate"
    );

    let result = run(tape_from(&turbo_speed(TURBO, &zeros)));
    assert_eq!(
        result.verdict,
        Verdict::Corrupt,
        "a decode indistinguishable from no decode at all was accepted"
    );
}

#[test]
fn the_expected_fold_appears_in_the_loaders_code_and_on_no_tape() {
    // The structural half of the same argument, and the cheap one: wherever the constant lives, it
    // must not be somewhere the tape can reach. `CP EXPECTED_XOR` is `FE 5A`, and it is in the
    // instruction stream; the tape carries the screen and nothing else.
    let program = assemble(Thresholds::DEFAULT);
    assert!(
        program
            .code
            .windows(2)
            .any(|pair| pair == [0xFE, EXPECTED_XOR]),
        "the loader does not compare against the constant it is supposed to carry"
    );

    let file = turbo_speed(TURBO, &SCREEN);
    let payload_starts = file.len() - SCREEN_BYTES;
    assert_eq!(
        file.get(payload_starts..),
        Some(&SCREEN[..]),
        "the tape's payload must be the screen and nothing appended to it"
    );
}

// ---------------------------------------------------------------------------------------
// The same signal, written four ways
// ---------------------------------------------------------------------------------------

/// How many pulses the signal proper is: the pilot tone, the sync pair, and two half-periods per
/// bit of every byte. The trailing pause is deliberately not counted — see
/// [`four_encodings_of_one_turbo_signal_produce_one_signal`].
fn signal_pulses(timings: Timings) -> usize {
    usize::from(timings.pilot_pulses) + 2 + 16 * SCREEN_BYTES
}

#[test]
fn four_encodings_of_one_turbo_signal_produce_one_signal() {
    // `ID 11` is one way to write a turbo block down; the format has three others, and a converter
    // that read any of their fields wrongly would produce a different train. Comparing the trains
    // is a far stronger equality than comparing what the loader made of them — two tapes that both
    // decode correctly can still differ, and the loader's thresholds are wide enough to hide it.
    //
    // The four routes share no field layout at all: `ID 11` names five half-periods in one block,
    // `ID 12`/`ID 13`/`ID 14` split them across three, `ID 15` names none of them and carries raw
    // levels at a fixed sample rate, and the fourth reaches the same three primitives only by
    // following a jump. There is no single mistake that produces the same wrong answer four times.
    let signal = signal_pulses(TURBO);
    let control = tape_from(&turbo_speed(TURBO, &SCREEN));

    for (name, file) in [
        (
            "ID 12 + ID 13 + ID 14",
            assembled_from_primitives(TURBO, &SCREEN),
        ),
        ("ID 15, direct recording", direct_recording(TURBO, &SCREEN)),
        (
            "ID 30 + ID 21 + ID 23 over a decoy",
            reached_by_control_flow(TURBO, &SCREEN),
        ),
    ] {
        let tape = tape_from(&file);
        // Named rather than sliced blind: two `None`s compare equal, so an encoding that produced
        // a train too short to hold the signal would otherwise pass by being empty.
        let (theirs, ours) = (tape.pulses().get(..signal), control.pulses().get(..signal));
        assert!(
            theirs.is_some() && ours.is_some(),
            "{name} produced only {} pulses, which is short of the {signal} the signal needs",
            tape.pulses().len()
        );
        assert_eq!(theirs, ours, "{name} is not the same signal as ID 11");
    }
}

#[test]
fn only_the_trailing_pause_tells_the_four_encodings_apart() {
    // The honest remainder of the comparison above, which stops at the end of the signal. The four
    // files agree on every one of the 112594 pulses that carry data and disagree after it, and the
    // disagreement is `ID 15`'s alone — the other two are pulse-identical to `ID 11` all the way to
    // the end.
    //
    // It is not a defect and it is documented in `crates/spectrum/src/tape/tzx.rs`, which is why
    // it is asserted by value rather than tolerated by trimming: *"the level after a direct
    // recording is the **last sample**, not its opposite — where after every other signal block it
    // *is* the opposite, so that a subsequent pulse will produce an edge"*. Honouring that rule
    // puts a zero-length edge at the boundary, and the pause that follows is written as one
    // millisecond of the standing level and then the rest, rather than as a single span.
    let signal = signal_pulses(TURBO);
    let control = tape_from(&turbo_speed(TURBO, &SCREEN));
    let direct = tape_from(&direct_recording(TURBO, &SCREEN));

    assert_eq!(
        signal, 112_594,
        "2000 pilot pulses, a sync pair, and 6912 bytes"
    );
    assert_eq!(
        control.pulses().get(signal..),
        Some(&[350_000, 0][..]),
        "100ms at 3.5MHz, written as one span"
    );
    assert_eq!(
        direct.pulses().get(signal..),
        Some(&[0, 3_500, 346_500, 0][..]),
        "the level rule's zero-length edge, then the same 100ms split at one millisecond"
    );

    // "The disagreement is ID 15's alone" is a claim, so here it is as an assertion: the other two
    // routes are identical to `ID 11` over the *whole* train, pause included, and not merely over
    // the signal the test above compares.
    for (name, file) in [
        (
            "ID 12 + ID 13 + ID 14",
            assembled_from_primitives(TURBO, &SCREEN),
        ),
        (
            "ID 30 + ID 21 + ID 23 over a decoy",
            reached_by_control_flow(TURBO, &SCREEN),
        ),
    ] {
        assert_eq!(
            tape_from(&file).pulses(),
            control.pulses(),
            "{name} differs from ID 11 somewhere, and only ID 15 is supposed to"
        );
    }

    // And both routes really are the same duration, which is the thing that would matter to a
    // machine still reading: the pause is 100ms either way, however it is written down.
    let pause: u32 = control.pulses()[signal..].iter().sum();
    assert_eq!(pause, direct.pulses()[signal..].iter().sum::<u32>());
    assert_eq!(
        pause,
        u32::from(PAUSE_MS) * 3_500,
        "3.5 million T-states a second is 3500 a millisecond"
    );
}

#[test]
fn the_control_tapes_pulse_train_is_the_length_the_arithmetic_predicts() {
    // The non-vacuity floor for the comparison above: four things being equal to each other says
    // nothing if all four are empty. This is what the train must be, counted from the format's own
    // description rather than from the converter — a pilot pulse per `pilot_pulses`, the sync pair,
    // and two half-periods per bit of every byte, and then the two the pause adds.
    let control = tape_from(&turbo_speed(TURBO, &SCREEN));
    assert_eq!(
        control.pulses().len(),
        signal_pulses(TURBO) + 2,
        "a pilot tone, a sync pair, sixteen half-periods per byte, and the pause"
    );

    // And it is made of the half-periods the file names, not of some default: the pilot's own
    // length appears exactly as many times as the file asks for.
    let pilots = control
        .pulses()
        .iter()
        .filter(|&&pulse| pulse == u32::from(TURBO.pilot))
        .count();
    assert_eq!(pilots, usize::from(TURBO.pilot_pulses));
    assert!(
        !control.pulses().contains(&ROM_BIT_ONE),
        "the ROM's own bit length must appear nowhere in a turbo train"
    );
}

#[test]
fn a_jump_that_was_ignored_would_put_the_decoys_pulses_in_the_train() {
    // The positive control for the control-flow route: the decoy really is in the file, really is
    // signal, and really would be audible if `ID 23` were treated as metadata and skipped over.
    let file = reached_by_control_flow(TURBO, &SCREEN);
    assert!(
        file.windows(2)
            .any(|pair| pair == DECOY_PULSE.to_le_bytes()),
        "the decoy's pulse length is not in the file, so jumping over it proves nothing"
    );

    let mut without_the_jump = file.clone();
    let jump = without_the_jump
        .windows(3)
        .position(|window| window == [0x23, 0x02, 0x00])
        .expect("the file contains the jump this test is about");
    // The jump lives in the metadata prefix, ahead of the decoy and of the payload. Saying so is
    // not pedantry: those three bytes could occur inside 6912 bytes of screen, and a search that
    // found one there would delete a byte of the picture and call it a control.
    assert_eq!(
        jump,
        HEADER.len() + 2 + DESCRIPTION.len() + 2 + GROUP.len(),
        "the jump was not where the file puts it, so this found three bytes of something else"
    );
    without_the_jump.splice(jump..jump + 3, []);

    let played = tape_from(&without_the_jump);
    let control = tape_from(&turbo_speed(TURBO, &SCREEN));
    assert_eq!(
        played.pulses().len(),
        control.pulses().len() + usize::from(DECOY_PULSES),
        "removing the jump must let the decoy's pulses into the train"
    );
}

// ---------------------------------------------------------------------------------------
// How fast it will go, and whose ceiling that is
// ---------------------------------------------------------------------------------------

/// What one whole run amounted to: the loader's own verdict **and** whether it was right.
///
/// The pair is not redundant, and [`Outcome::SilentlyWrong`] is why. A `XOR` fold cannot see an
/// even number of bits dropped from the same position, so a loader can be confidently, visibly
/// wrong — and a table that recorded only the border colour would have written that down as a
/// success. This is the same lesson as the all-zeros hole in this file's header, met a second time
/// from the other direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Green, and the screen in memory is the screen on the tape. The only good result.
    Read,
    /// **Green, and it is not.** The fold agreed and the bytes did not.
    SilentlyWrong,
    /// Red: decoded a whole screen, folded it, and refused it.
    Refused,
    /// Blue: never recognised a pilot tone, so it never began.
    NeverSynced,
    /// Yellow: synced, and then never finished a screen inside the budget.
    Stalled,
}

impl Outcome {
    fn of(result: &Run) -> Self {
        match result.verdict {
            Verdict::Hunting => Self::NeverSynced,
            Verdict::Decoding => Self::Stalled,
            Verdict::Corrupt => Self::Refused,
            Verdict::Loaded if result.screen() == SCREEN => Self::Read,
            Verdict::Loaded => Self::SilentlyWrong,
        }
    }
}

/// The six data rates the header's table reports, slowest first.
///
/// Each row is a whole set of timings rather than a scale factor, because that is what a real
/// turbo tape carries: the pilot, the sync and the two bit lengths are independent fields, and a
/// loader has to cope with all four moving at once.
const SPEEDS: [(Timings, Outcome); 6] = [
    (TURBO, Outcome::Read), //                             1.50x
    (
        Timings {
            pilot: 1200,
            pilot_pulses: 2200,
            sync_first: 450,
            sync_second: 450,
            bit_zero: 450,
            bit_one: 1000,
        },
        Outcome::Read, //                                  1.76x
    ),
    (
        Timings {
            pilot: 1100,
            pilot_pulses: 2400,
            sync_first: 400,
            sync_second: 400,
            bit_zero: 400,
            bit_one: 900,
        },
        Outcome::Read, //                                  1.97x
    ),
    (
        Timings {
            pilot: 1050,
            pilot_pulses: 2500,
            sync_first: 380,
            sync_second: 380,
            bit_zero: 380,
            bit_one: 850,
        },
        Outcome::SilentlyWrong, //                         2.08x — the interesting row
    ),
    (
        Timings {
            pilot: 700,
            pilot_pulses: 3000,
            sync_first: 250,
            sync_second: 250,
            bit_zero: 250,
            bit_one: 600,
        },
        Outcome::NeverSynced, //                           3.01x
    ),
    (
        Timings {
            pilot: 600,
            pilot_pulses: 3500,
            sync_first: 220,
            sync_second: 220,
            bit_zero: 220,
            bit_one: 500,
        },
        Outcome::NeverSynced, //                           3.56x
    ),
];

/// How much faster than the ROM a set of timings carries data, in hundredths.
///
/// Integer arithmetic on purpose: the ratio is `(855 + 1710) / (zero + one)`, and multiplying by
/// 100 before dividing keeps it exact rather than nearly right.
fn rate_against_the_rom(timings: Timings) -> u32 {
    ROM_BIT_SUM * 100 / (u32::from(timings.bit_zero) + u32::from(timings.bit_one))
}

#[test]
fn the_loader_reads_every_rate_the_table_says_it_reads() {
    // The header's speed table, re-measured rather than recalled, and asserted as a whole outcome
    // rather than as a border colour — because one of the six rows is a green border over a wrong
    // screen, and a gate that graded the colour would have called that a pass.
    for (timings, expected) in SPEEDS {
        let rate = rate_against_the_rom(timings);
        let result = run(tape_from(&turbo_speed(timings, &SCREEN)));
        assert_eq!(
            Outcome::of(&result),
            expected,
            "at {}.{:02}x the ROM's data rate",
            rate / 100,
            rate % 100
        );
    }
}

#[test]
fn a_fold_can_agree_while_the_screen_is_wrong() {
    // The row above that is worth its own test, and the most useful thing this file found.
    //
    // At 2.08x a one bit's poll count lands *on* the loader's threshold, so whether a given bit
    // reads as a one depends on where in the frame it fell — the ULA's I/O contention moves the
    // poll cost by a few T-states across the display window. Bits are therefore dropped from the
    // same position in scattered bytes, each dropping the same 0x80 out of the fold, and an even
    // number of them cancels. The loader folds, agrees with itself, and paints green over a
    // screen that is visibly wrong.
    //
    // This is asserted structurally rather than by the count of damaged bytes: the claim is "the
    // fold agreed and the bytes did not", which is a property of `XOR`, and not "1244 bytes were
    // damaged", which is a property of this particular payload landing on this particular frame.
    let (timings, expected) = SPEEDS[MARGINAL_SPEED];
    assert_eq!(expected, Outcome::SilentlyWrong, "the table's marginal row");

    let result = run(tape_from(&turbo_speed(timings, &SCREEN)));
    let arrived = result.screen();
    assert_ne!(arrived, SCREEN, "this row is supposed to arrive damaged");
    assert_eq!(
        xor_fold(&arrived),
        EXPECTED_XOR,
        "and the fold is supposed to have missed it, which is what makes the border green"
    );
    assert_eq!(result.verdict, Verdict::Loaded);
}

#[test]
fn no_rate_in_the_table_is_refused_by_anything_but_the_loaders_own_two_constants() {
    // The claim the header makes about *whose* limit the table's failures are, turned into a
    // measurement. Every failing row is played again byte for byte unchanged; the only thing that
    // moves is one or both immediates inside the guest's own code. If any of these limits belonged
    // to the emulator — to a rounding step in the pulse train, to a resolution floor, to a ceiling
    // on how short a pulse may be — then editing a constant in the Z80 program could not lift it.
    //
    // It lifts all three. The two failure modes are also localised to different constants, which
    // is what makes this more than a rescue: 2.08x is the *bit* threshold running out, and the two
    // fastest rows never get that far, because `pilot_min` runs out first and they never sync.
    for (index, rescue) in RESCUES {
        let (timings, refused) = SPEEDS[index];
        assert_ne!(refused, Outcome::Read, "this row is supposed to fail");

        let file = turbo_speed(timings, &SCREEN);
        let rescued = run_with(tape_from(&file), rescue);
        assert_eq!(
            Outcome::of(&rescued),
            Outcome::Read,
            "at {} percent of the ROM's rate, {rescue:?} did not rescue a tape the emulator had \
             played identically both times",
            rate_against_the_rom(timings)
        );
    }
}

/// Which row of [`SPEEDS`] is the one the loader gets wrong without noticing.
const MARGINAL_SPEED: usize = 3;

/// For each failing row of [`SPEEDS`], thresholds that read it. Measured, not chosen.
///
/// The middle failure needs only `bit` moved, because the loader synced perfectly well and merely
/// misread bits. The two fastest need `pilot_min` moved as well, because they never synced at all
/// — a different constant, and the reason their border is blue rather than green.
const RESCUES: [(usize, Thresholds); 3] = [
    // 2.08x: it synced perfectly well and merely misread bits, so only `bit` moves.
    (
        3,
        Thresholds {
            pilot_min: 20,
            bit: 14,
        },
    ),
    // 3.01x and 3.56x never synced, so `pilot_min` has to come down before `bit` can matter at
    // all. Both were swept rather than guessed: 3.01x reads at every `pilot_min` from 12 down and
    // every `bit` from 9 down, and 3.56x at every `pilot_min` from 11 down and every `bit` from 7
    // down. The pairs below are the middle of each measured region, not its edge.
    (
        4,
        Thresholds {
            pilot_min: 11,
            bit: 8,
        },
    ),
    (
        5,
        Thresholds {
            pilot_min: 10,
            bit: 7,
        },
    ),
];

// ---------------------------------------------------------------------------------------
// `ID 19`, which is not implemented, and says so
// ---------------------------------------------------------------------------------------

#[test]
fn a_generalized_data_block_is_refused_by_id_and_offset() {
    // Not a request for `ID 19` to be implemented — that is a feature decision and is not made
    // here. This pins the behaviour a reader will meet if they hand this emulator a `.tzx` that
    // uses one: a named refusal at parse time, carrying the block's ID and its byte offset, rather
    // than a silent skip that drops signal or a machine that sits waiting for an edge.
    //
    // The block's length *is* knowable — a DWORD at offset 0 — which is exactly why refusing is a
    // decision rather than a necessity, and why it is worth asserting.
    let mut file = HEADER.to_vec();
    file.push(0x19);
    file.extend(4_u32.to_le_bytes()); // 0x00 DWORD block length, without these four bytes
    file.extend([0; 4]);

    assert_eq!(
        tzx::parse(&file, Model::Spectrum48K),
        Err(spectrum::tape::Error::UnplayableBlock {
            offset: HEADER.len(),
            id: 0x19,
        }),
        "a generalized data block must be refused by name, not skipped and not played"
    );
}
