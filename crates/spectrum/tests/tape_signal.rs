//! The signal itself: the pulse train a `.tap` becomes, and the `EAR` bit it drives.
//!
//! # Three instruments, and none of them is a round trip
//!
//! `docs/M6.md` Decision 5's gate, in the order it names them:
//!
//! 1. **The exact pulse train of a one-byte block, written out by hand.** Every half-period
//!    after the pilot tone is a literal here, taken from the format's own rule — most
//!    significant bit first, two equal half-periods per bit, 855 for a zero and 1710 for a
//!    one — and not from the encoder. `tape_rom_timings.rs` grades the pilot's length and
//!    period against the ROM's own writer, which is the one part of the train too long to
//!    write down.
//! 2. **A decoder written in this file**, which reads a train back into bytes using the
//!    threshold rule the ROM's loader uses. It owes nothing to `tape::tap`, so
//!    `decode(parse(f)) == f` is a check of the encoder against an independent reader rather
//!    than against its own inverse — and it runs over arbitrary payloads as a `proptest`.
//! 3. **`IN A,(0xFE)` still returns `0xBF` with no tape**, which is M5's behaviour and is
//!    pinned from outside this crate by `keyboard_matrix.rs` across the whole membrane.
//!
//! # And one the design turns on
//!
//! `the_tape_advances_by_contention_as_well_as_by_ticks`. Contention moves the clock outside
//! any `Bus::tick`, so a tape driven from `tick` alone would run slow by exactly the
//! contention a loader suffers — silently, because nothing else would move. It is the reason
//! `Ula::advance` exists.

use spectrum::tape::{Tape, tap};
use spectrum::timing::{FIRST_CONTENDED_T_STATE, T_STATES_PER_FRAME};
use spectrum::{Colour, Spectrum};
use z80::{Bus, CpuState};

/// Bit 6 of a `0xFE` read: the `EAR` input.
const EAR_BIT: u8 = 0x40;

/// What a real `IN A,(0xFE)` returns with no key held and nothing on the tape socket.
///
/// The same literal `crates/spectrum/tests/common/mod.rs` has pinned since M5, repeated here
/// rather than shared because this gate must not go green by agreeing with a helper that
/// moved. Bits 5 and 7 float high; bit 6 is the `EAR` input, low; bits 0–4 are the keyboard's
/// five, high when nothing is pressed.
const IDLE_HALF_ROW: u8 = 0xBF;

/// Half-period of a zero bit, from the format's own timings.
const BIT_ZERO: u32 = 855;

/// Half-period of a one bit: twice a zero, which is what lets one threshold separate them.
const BIT_ONE: u32 = 1710;

/// The two sync half-periods that follow the pilot tone.
const SYNC: [u32; 2] = [667, 735];

/// The pilot tone's half-period.
const PILOT: u32 = 2168;

/// A ROM of `NOP`s: every instruction is one uncontended M1 fetch, exactly four T-states.
const NOP_T_STATES: u32 = 4;

fn machine() -> Spectrum {
    Spectrum::new(&[0x00; spectrum::memory::PAGE_SIZE]).expect("a page-sized ROM")
}

/// A `.tap` file holding one data block: a length word, the flag, `payload`, and its parity.
fn tap_file(flag: u8, payload: &[u8]) -> Vec<u8> {
    let parity = payload.iter().fold(flag, |sum, &byte| sum ^ byte);
    let length = u16::try_from(payload.len() + 2).expect("a short block");
    let mut file = length.to_le_bytes().to_vec();
    file.push(flag);
    file.extend_from_slice(payload);
    file.push(parity);
    file
}

// ---------------------------------------------------------------------------------------
// 1 — the train, written out by hand
// ---------------------------------------------------------------------------------------

/// The half-periods one byte contributes, most significant bit first.
///
/// Written from the rule rather than produced by the encoder: this function is the
/// expectation, so it must not call anything in `tape::tap`.
fn expected_byte(byte: u8) -> Vec<u32> {
    let mut pulses = Vec::new();
    for bit in (0..8).rev() {
        let length = if byte & (1 << bit) == 0 {
            BIT_ZERO
        } else {
            BIT_ONE
        };
        pulses.extend([length, length]);
    }
    pulses
}

#[test]
fn a_one_byte_blocks_train_is_the_one_written_out_by_hand() {
    // The whole non-pilot train as literals. `0x2A` and its parity `0xD5` are bit-complements
    // of each other in seven of eight positions, so a converter that emitted the bits in the
    // wrong order, or that got the two lengths the wrong way round, produces a train that
    // differs from this one rather than a plausible variant of it.
    let tape = tap::parse(&tap_file(0xFF, &[0x2A])).expect("a well-formed block");
    let pulses = tape.pulses();

    let pilot = pulses.iter().take_while(|&&p| p == PILOT).count();
    let tail: Vec<u32> = pulses.iter().skip(pilot).copied().collect();

    let mut expected = SYNC.to_vec();
    // 0xFF — the data-block flag: eight one bits.
    expected.extend([BIT_ONE; 16]);
    // 0x2A = 0b0010_1010.
    expected.extend([
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ONE, BIT_ONE, // 1
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ONE, BIT_ONE, // 1
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ONE, BIT_ONE, // 1
        BIT_ZERO, BIT_ZERO, // 0
    ]);
    // The parity byte, 0xFF ^ 0x2A = 0xD5 = 0b1101_0101.
    expected.extend([
        BIT_ONE, BIT_ONE, // 1
        BIT_ONE, BIT_ONE, // 1
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ONE, BIT_ONE, // 1
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ONE, BIT_ONE, // 1
        BIT_ZERO, BIT_ZERO, // 0
        BIT_ONE, BIT_ONE, // 1
    ]);
    // The trailing silence: one second at the machine's own frame rate.
    expected.push(T_STATES_PER_FRAME * 50);

    assert_eq!(tail, expected);
    assert_eq!(
        expected_byte(0x2A),
        tail.get(18..34)
            .expect("the byte follows the sync and the flag"),
        "and the hand-written table agrees with the hand-written rule"
    );
}

#[test]
fn the_flag_byte_changes_the_pilot_and_nothing_about_the_payload() {
    // The `BIT 7,A` rule, from this side. The two blocks carry the same payload byte, and its
    // sixteen half-periods must be identical — while the pilot length is not.
    //
    // The flag byte and the **parity** byte both differ, and that is not a defect in the
    // converter: parity is the XOR of the flag and the data, so changing the flag changes it.
    // Comparing whole trains would have made this test fail for a correct reason, which is why
    // it names the region it means.
    const SYNC_AND_FLAG: usize = 2 + 16;
    const PAYLOAD_HALF_PERIODS: usize = 16;

    let payload = [0x2A];
    let header = tap::parse(&tap_file(0x00, &payload)).expect("a header block");
    let data = tap::parse(&tap_file(0x80, &payload)).expect("a data block");

    let payload_region = |tape: &Tape| -> Vec<u32> {
        tape.pulses()
            .iter()
            .skip_while(|&&pulse| pulse == PILOT)
            .skip(SYNC_AND_FLAG)
            .take(PAYLOAD_HALF_PERIODS)
            .copied()
            .collect()
    };
    assert_eq!(payload_region(&header), expected_byte(0x2A));
    assert_eq!(payload_region(&header), payload_region(&data));

    let pilot = |tape: &Tape| tape.pulses().iter().filter(|&&p| p == PILOT).count();
    assert!(
        pilot(&header) > pilot(&data),
        "a header's pilot tone is the longer of the two"
    );
}

// ---------------------------------------------------------------------------------------
// 2 — a decoder this file owns
// ---------------------------------------------------------------------------------------

/// Read the first block of a pulse train back into the bytes it carries.
///
/// Written from the loader's rule and **not** from `tape::tap`: skip the pilot tone, which is
/// the only thing longer than a one bit; skip the two sync half-periods, which is how the
/// loader knows the data has started; then take half-periods in pairs and call each pair a bit
/// by comparing it to the midpoint of the two lengths, which is what the ROM's `LD-EDGE`
/// comparison amounts to. The threshold is derived from the two published lengths rather than
/// imported, so a decoder that agreed with a wrong encoder would have to agree by accident.
///
/// It stops at the trailing silence, so on a multi-block tape it reads the first block. That
/// is all any caller here wants and it keeps the reader as simple as the rule it encodes.
fn decode(pulses: &[u32]) -> Vec<u8> {
    const THRESHOLD: u32 = (BIT_ZERO + BIT_ONE) / 2;
    /// The sync pair is two half-periods, and it is shorter than a bit rather than longer —
    /// so it has to be skipped by position. Letting it through decodes two extra zero bits
    /// and shifts every byte after it, which is exactly what this decoder did first time out.
    const SYNC_PULSES: usize = 2;

    let bits: Vec<bool> = pulses
        .iter()
        .skip_while(|&&pulse| pulse > BIT_ONE)
        .skip(SYNC_PULSES)
        .take_while(|&&pulse| pulse <= BIT_ONE)
        .step_by(2)
        .map(|&pulse| pulse > THRESHOLD)
        .collect();
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|byte| {
            byte.iter()
                .fold(0_u8, |value, &bit| (value << 1) | u8::from(bit))
        })
        .collect()
}

#[test]
fn the_decoder_can_tell_a_wrong_train_from_a_right_one() {
    // The decoder is only worth running if it discriminates, so it has its own failing cases
    // before anything is built on it. Without them it would be a reader that returns whatever
    // it is given, asserting that the encoder is right.
    // `0x2A` rather than `0xA5`: a byte that is **not** its own bit-reversal, or the
    // wrong-order case below would pass against a decoder that read the bits backwards.
    const BYTE: u8 = 0x2A;
    assert_ne!(
        BYTE,
        BYTE.reverse_bits(),
        "the fixture must not be a palindrome"
    );

    let mut train = vec![PILOT, PILOT, SYNC[0], SYNC[1]];
    train.extend(expected_byte(BYTE));
    assert_eq!(decode(&train), vec![BYTE]);

    let mut reversed = vec![PILOT, PILOT, SYNC[0], SYNC[1]];
    for pulse in expected_byte(BYTE).as_chunks::<2>().0.iter().rev() {
        reversed.extend_from_slice(pulse);
    }
    assert_eq!(decode(&reversed), vec![BYTE.reverse_bits()]);

    let mut swapped = vec![PILOT, PILOT, SYNC[0], SYNC[1]];
    for &pulse in &expected_byte(BYTE) {
        swapped.push(if pulse == BIT_ZERO { BIT_ONE } else { BIT_ZERO });
    }
    assert_eq!(decode(&swapped), vec![!BYTE]);
}

#[test]
fn every_block_decodes_back_to_the_bytes_it_was_built_from() {
    let payload = [0x00, 0xFF, 0x2A, 0xD5, 0x01, 0x80];
    for flag in [0x00_u8, 0xFF] {
        let tape = tap::parse(&tap_file(flag, &payload)).expect("a well-formed block");
        let mut expected = vec![flag];
        expected.extend_from_slice(&payload);
        expected.push(payload.iter().fold(flag, |sum, &byte| sum ^ byte));
        assert_eq!(decode(tape.pulses()), expected, "flag {flag:#04X}");
    }
}

// ---------------------------------------------------------------------------------------
// 3 — the wiring: the signal reaches bit 6 of a real port read
// ---------------------------------------------------------------------------------------

#[test]
fn with_no_tape_a_port_read_is_unchanged_from_m5() {
    // The one assertion this milestone must not break. `keyboard_matrix.rs` pins the same
    // literal across the full 40 key x 8 half-row cross product, so this is the local echo of
    // a gate that lives elsewhere and is stronger.
    let mut machine = machine();
    assert_eq!(machine.ula_mut().in_port(0xFEFE), IDLE_HALF_ROW);
    assert_eq!(
        IDLE_HALF_ROW & EAR_BIT,
        0,
        "bit 6 is the EAR input, and it is low"
    );

    // ...and an *empty* tape in the drive is the same thing, because a drive with no cassette
    // and a cassette with nothing on it drive the line identically.
    machine.insert_tape(Tape::new(Vec::new()));
    machine.tape_mut().play();
    machine.run_frames(2);
    assert_eq!(machine.ula_mut().in_port(0xFEFE), IDLE_HALF_ROW);
}

#[test]
fn the_ear_bit_follows_a_square_wave_through_the_running_machine() {
    // The signal path end to end: a train built by hand, time advanced by real instructions,
    // and the level read out of a real port cycle. Nothing here goes through `.tap`.
    const HALF_PERIOD: u32 = 100;
    const NOPS_PER_HALF_PERIOD: u64 = (HALF_PERIOD / NOP_T_STATES) as u64;

    let mut machine = machine();
    machine.insert_tape(Tape::new(vec![HALF_PERIOD; 6]));
    machine.tape_mut().play();

    let mut observed = Vec::new();
    for _ in 0..6 {
        observed.push(machine.ula_mut().in_port(0xFEFE) & EAR_BIT != 0);
        for _ in 0..NOPS_PER_HALF_PERIOD {
            machine.step();
        }
    }
    assert_eq!(observed, [false, true, false, true, false, true]);
    // The port read itself must not have moved the clock here, or the sampling above would be
    // drifting: outside the display's fetch window a ULA port access stalls nothing.
    assert!(machine.frame_t_state() < FIRST_CONTENDED_T_STATE);
}

#[test]
fn the_tape_advances_by_contention_as_well_as_by_ticks() {
    // The property `Ula::advance` exists for, asserted through the machine and **on the tape**
    // rather than on the clock. The same 32 `NOP`s run from the contended bank and from an
    // uncontended one; the instructions are identical, so the only difference between the runs
    // is the contention — and the tape has to see it.
    //
    // A tape driven from `Bus::tick` alone would land in the same place in both runs and would
    // run slow by exactly the contention a real loader suffers, silently, because nothing else
    // would move.

    /// Measured: 32 `NOP`s cost 128 T-states out of the contended bank and 194 in it, starting
    /// on `FIRST_CONTENDED_T_STATE`. A half-period between the two flips the signal in the
    /// contended run and not in the other, which is what makes the two runs distinguishable
    /// **by the tape** rather than by the clock.
    const HALF_PERIOD: u32 = 160;
    const UNCONTENDED_COST: u32 = 32 * NOP_T_STATES;
    const CONTENDED_COST: u32 = 194;

    // The half-period has to fall strictly between the two costs or the two runs cannot be
    // told apart by the tape at all. Asserted at compile time, so a future edit to any of the
    // three numbers is a build error rather than a test that quietly stops discriminating.
    const _: () = assert!(UNCONTENDED_COST < HALF_PERIOD && HALF_PERIOD < CONTENDED_COST);

    let mut outcomes = Vec::new();
    for code in [0x4000_u16, 0x8000] {
        let mut machine = machine();
        // A page of `NOP`s at `code`, and the clock parked inside the display window where a
        // contended fetch actually stalls.
        for offset in 0..256 {
            machine.memory_mut().write(code + offset, 0x00);
        }
        let mut snapshot = machine.snapshot();
        snapshot.cpu = CpuState {
            pc: code,
            ..CpuState::default()
        };
        snapshot.frame_t_state = FIRST_CONTENDED_T_STATE;
        snapshot.border = Colour::BLACK;
        machine.restore(&snapshot);

        machine.insert_tape(Tape::new(vec![HALF_PERIOD; 4]));
        machine.tape_mut().play();
        let start = machine.frame_t_state();
        for _ in 0..32 {
            machine.step();
        }
        outcomes.push((machine.frame_t_state() - start, machine.tape_mut().level()));
    }

    let [
        (contended_cost, contended_level),
        (uncontended_cost, uncontended_level),
    ] = outcomes.as_slice()
    else {
        unreachable!("two runs")
    };

    // The clock, first, so a failure below says which half moved.
    assert_eq!(
        *uncontended_cost, UNCONTENDED_COST,
        "an uncontended NOP is exactly four T-states"
    );
    assert_eq!(
        *contended_cost, CONTENDED_COST,
        "the contended cost moved; re-derive HALF_PERIOD before trusting the levels below"
    );
    // ...and then the tape, which is the assertion this test is named for.
    assert!(
        !uncontended_level,
        "128 T-states is less than one half-period, so the signal must not have flipped"
    );
    assert!(
        *contended_level,
        "194 T-states is more than one half-period, so the signal must have flipped — if it \
         did not, the tape is being driven by ticks alone and never sees a contention stall"
    );
}

// ---------------------------------------------------------------------------------------
// The encoder against an independent reader, over arbitrary bytes
// ---------------------------------------------------------------------------------------

proptest::proptest! {
    #[test]
    fn any_block_decodes_back_to_itself(payload: Vec<u8>, flag: u8) {
        // `decode` is written from the format's rule and shares no code with the encoder, so
        // this is not `f(f_inverse(x))`. Its blind spot is a rule both readings share — the
        // bit *order*, say — which is what the hand-written literal train above closes.
        let tape = tap::parse(&tap_file(flag, &payload)).expect("a well-formed block");
        let mut expected = vec![flag];
        expected.extend_from_slice(&payload);
        expected.push(payload.iter().fold(flag, |sum, &byte| sum ^ byte));
        proptest::prop_assert_eq!(decode(tape.pulses()), expected);
    }
}
