//! The external instrument for `.tzx`: the ROM's **own** tape writer, measured.
//!
//! # Why a second oracle, when `tape_rom_timings.rs` already runs this routine
//!
//! Because it grades a different code path, and the difference is the whole point of `.tzx`.
//!
//! `tape_rom_timings.rs` grades `.tap`, whose five timings are **constants** in
//! `crates/spectrum/src/tape/tap.rs`, each derived by counting T-states through `SA-BYTES`. A
//! `.tzx` **turbo** block gets those same five numbers from *bytes in the file*, at five
//! offsets that the format description gives and that nothing else in this workspace has ever
//! read. A transposed pair of offsets, a big-endian word, a field read one byte early — none of
//! those is visible to any test built from a file this project also wrote, because the same
//! misreading would be on both sides of the comparison.
//!
//! So this file writes a turbo block **carrying the ROM's own timing values** and compares the
//! train it produces against the intervals the ROM's writer actually emits. Two
//! implementations, no shared code, no shared constants — the numbers travel from the format
//! description into a byte array here, through the converter's field offsets, and are compared
//! against a routine Sinclair shipped in 1982.
//!
//! It grades the standard-speed block the same way in the same run, which is the answer to
//! *"does a standard-speed `.tzx` agree with the `SA-BYTES` oracle"*: it does, by value, over
//! 3305 half-periods.
//!
//! # It is a deliberately separate harness, not a shared one
//!
//! The recording bus below is a second implementation of the one in `tape_rom_timings.rs`, and
//! that is a choice rather than an oversight. A shared harness would mean one bug in the
//! harness turning **both** oracles green at once, which is the failure mode `docs/STATUS.md`
//! catalogues under gates that verify nothing; and this project already duplicates its tape
//! fixtures on the same reasoning — `tape_signal.rs` repeats `IDLE_HALF_ROW` rather than share
//! it, *"because this gate must not go green by agreeing with a helper that moved"*.
//!
//! The two also state the ROM's irregularities **differently on purpose**: the `.tap` oracle
//! builds an expected train by applying named deviations, and this one asserts the *profile of
//! the differences* between the two trains. Same fact, two formulations, so an error in either
//! formulation shows up as a disagreement rather than as a shared blind spot.
//!
//! # What it does not grade
//!
//! The timings against **hardware**. The ROM defines what the standard values mean and it is
//! what every loader was written against, but nobody here has measured a real Spectrum. That
//! row stays in the ungraded list, exactly as it does for `.tap`.
//!
//! It also grades nothing about a *genuinely* turbo tape — a block whose bits are faster than
//! the ROM's — because the ROM cannot read one, and the ROM is this file's whole instrument.
//!
//! > That used to end *"Nothing in this repository can. That is T4."*, and the first sentence
//! > stopped being true when `tzx_turbo_load.rs` landed. It grades a turbo tape the way this file
//! > grades a standard one — by running a loader and measuring what it makes of the signal — with
//! > a loader written in this repository rather than Sinclair's, because no ROM will do it. What
//! > it cannot borrow from here is the *oracle*: the ROM's writer is code nobody here wrote, and
//! > there is no equivalent third party for turbo timings, so that gate rests on a signal this
//! > project generates and a loader this project wrote. The two files are therefore complementary
//! > rather than overlapping — this one has the better oracle, that one the wider reach — and
//! > **T4, a turbo *game* reaching its title screen, is still not done.**

use spectrum::Model;
use spectrum::memory::Memory;
use spectrum::tape::tzx;
use spectrum::timing::T_STATES_PER_FRAME;
use spectrum::ula::Ula;
use z80::{Bus, Cpu, CpuState};

/// `SA-BYTES`, the ROM's tape writer.
///
/// Entered with `IX` at the data, `DE` holding its length and `A` the flag byte. It performs its
/// own `DI`, and emits flag, data and parity as one block.
const SA_BYTES: u16 = 0x04C2;

/// Bit 3 of a `0xFE` write: the `MIC` output, which is what a tape recorder records.
const MIC_BIT: u8 = 0x08;

/// Where the block being written lives. Uncontended RAM, so nothing here depends on it.
const DATA: u16 = 0x8000;

/// The stack `SA-BYTES` pushes its return address onto.
const STACK: u16 = 0x7F00;

/// The flag byte, and the bytes after it.
///
/// `0x00` and `0xFF` make a whole byte of each bit value, so a converter emitting one length for
/// both would produce a train of the right shape and the wrong values; `0xA5` alternates, so a
/// converter that got the bit **order** backwards would still differ from its own mirror image.
const FLAG: u8 = 0xFF;
const PAYLOAD: [u8; 3] = [0x00, 0xFF, 0xA5];

/// The ten-byte `.tzx` header: `"ZXTape!"`, the end-of-text marker, and revision 1.20.
const HEADER: [u8; 10] = [b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14];

/// The ROM's standard timings, **as they will be written into a turbo block's bytes**.
///
/// Transcribed from the format description's own curly-bracket defaults for `ID 11` — *"Length
/// of PILOT pulse {2168}"*, *"SYNC first pulse {667}"*, *"SYNC second pulse {735}"*, *"ZERO bit
/// pulse {855}"*, *"ONE bit pulse {1710}"*, *"Length of PILOT tone (number of pulses) {8063
/// header (flag<128), 3223 data (flag>=128)}"* — and **not** imported from
/// `spectrum::tape::tap`, which is what makes the comparison below an independent one.
const PILOT: u16 = 2168;
const SYNC_FIRST: u16 = 667;
const SYNC_SECOND: u16 = 735;
const BIT_ZERO: u16 = 855;
const BIT_ONE: u16 = 1710;
const DATA_PILOT_PULSES: u16 = 3223;

/// Half-periods one byte contributes: two per bit.
const PULSES_PER_BYTE: usize = 16;

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
    /// Port `0xFE` is a ULA port, so the writer's own edges are delayed by 0–6 T-states each
    /// while the display is being drawn. That is what the hardware does and is not what a
    /// `.tzx` describes, so removing it makes the comparison an **equality** rather than a
    /// tolerance — and a tolerance is where a future error hides.
    ///
    /// Not circular: it uses the contention model to subtract contention, and that model is
    /// graded by four gates of its own in this directory.
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
        if port & 1 == 0 {
            let mic = value & MIC_BIT != 0;
            // The first write is an edge whether or not the level moved: it is where the
            // routine takes the line over, and it opens the first half-period.
            if self.edges.is_empty() || mic != self.mic {
                self.mic = mic;
                self.edges.push(self.nominal_now());
            }
        }
        self.charge(0, |ula| ula.out_port(port, value));
    }

    fn tick(&mut self, address: u16) {
        self.charge(1, |ula| ula.tick(address));
    }
}

/// Run `SA-BYTES` over the payload and return the first `wanted` half-periods it emits.
fn rom_emitted_pulses(rom: &[u8], wanted: usize) -> Vec<u32> {
    let mut cpu = Cpu::new(Recorder::new(rom));
    for (offset, &byte) in (0..).zip(&PAYLOAD) {
        cpu.bus_mut().ula.memory_mut().write(DATA + offset, byte);
    }

    cpu.set_state(CpuState {
        pc: SA_BYTES,
        ix: DATA,
        de: u16::try_from(PAYLOAD.len()).expect("a short payload"),
        af: u16::from(FLAG) << 8,
        sp: STACK,
        // `SA-BYTES` performs its own `DI`, but not on its first instruction.
        iff1: false,
        iff2: false,
        ..CpuState::default()
    });

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

/// The block's bytes as they go on tape: the flag, the payload, and the parity byte.
fn block_data() -> Vec<u8> {
    let mut data = vec![FLAG];
    data.extend_from_slice(&PAYLOAD);
    data.push(PAYLOAD.iter().fold(FLAG, |sum, &byte| sum ^ byte));
    data
}

/// A `.tzx` holding one `ID 10 - Standard Speed Data Block`, with no pause after it.
///
/// Body: `0x00` WORD pause, `0x02` WORD length, `0x04` the data.
fn standard_speed_file() -> Vec<u8> {
    let data = block_data();
    let mut file = HEADER.to_vec();
    file.push(0x10);
    file.extend(0_u16.to_le_bytes());
    file.extend(
        u16::try_from(data.len())
            .expect("a short block")
            .to_le_bytes(),
    );
    file.extend(data);
    file
}

/// A `.tzx` holding one `ID 11 - Turbo Speed Data Block` carrying the ROM's own values.
///
/// Every field is written at the offset the format description gives it, and every number comes
/// from the description's curly-bracket defaults rather than from this crate's constants.
fn turbo_speed_file() -> Vec<u8> {
    let data = block_data();
    let mut file = HEADER.to_vec();
    file.push(0x11);
    file.extend(PILOT.to_le_bytes()); //         0x00 WORD    pilot pulse
    file.extend(SYNC_FIRST.to_le_bytes()); //    0x02 WORD    sync first
    file.extend(SYNC_SECOND.to_le_bytes()); //   0x04 WORD    sync second
    file.extend(BIT_ZERO.to_le_bytes()); //      0x06 WORD    zero bit
    file.extend(BIT_ONE.to_le_bytes()); //       0x08 WORD    one bit
    file.extend(DATA_PILOT_PULSES.to_le_bytes()); // 0x0A WORD pilot tone length, in pulses
    file.push(8); //                             0x0C BYTE    used bits in the last byte
    file.extend(0_u16.to_le_bytes()); //         0x0D WORD    pause after this block, ms
    let length = u32::try_from(data.len()).expect("a short block");
    file.extend(length.to_le_bytes().get(..3).expect("a 24-bit length")); // 0x0F BYTE[3]
    file.extend(data); //                        0x12 BYTE[N] data
    file
}

/// The train a `.tzx` file produces.
fn converted(file: &[u8]) -> Vec<u32> {
    tzx::parse(file, Model::Spectrum48K)
        .expect("a well-formed .tzx")
        .pulses()
        .to_vec()
}

/// How far the ROM's own half-period differs from the idealised one, at each place it does.
///
/// Every entry is a property of `SA-BYTES` rather than of any converter, and each is quoted
/// from the derivation `crates/spectrum/src/tape/tap.rs` and `tape_rom_timings.rs` carry:
///
/// - the **first half-period after the sync** runs one T-state long, because the bit loop is
///   entered with `B = 0x3B` from `LD BC,$3B0E` rather than with the loop's own `0x3E`;
/// - a half-period spanning a **byte boundary** runs three T-states short, because `LD B,$31`
///   at `0x052D` under-compensates the per-byte housekeeping;
/// - the boundary **before the parity byte** runs one short rather than three, because
///   `SA-PARITY` reaches the bit loop by a path two T-states longer than `LD L,(IX+0)`.
///
/// Stated here as the expected *difference* between the two trains rather than as a
/// transformation that builds one from the other, which is the same fact in the other
/// direction — so a mistake in either formulation is a disagreement between the two files
/// rather than a blind spot they share.
fn expected_differences(pilot_pulses: usize, bytes: usize) -> Vec<(usize, i64)> {
    let first_bit = pilot_pulses + 2;
    let parity_boundary = first_bit + (bytes - 1) * PULSES_PER_BYTE;

    let mut differences = vec![(first_bit, 1_i64)];
    for byte in 1..bytes {
        let index = first_bit + byte * PULSES_PER_BYTE;
        differences.push((index, if index == parity_boundary { -1 } else { -3 }));
    }
    differences.sort_unstable();
    differences
}

/// Where the pilot tone stops in a train: the first pulse that is not the opening one.
fn pilot_length(train: &[u32]) -> usize {
    let period = train.first().copied().unwrap_or_default();
    train.iter().take_while(|&&pulse| pulse == period).count()
}

/// Every index at which the two trains differ, and by how much.
fn differences(measured: &[u32], ours: &[u32]) -> Vec<(usize, i64)> {
    measured
        .iter()
        .zip(ours)
        .enumerate()
        .filter(|(_, (rom, converted))| rom != converted)
        .map(|(index, (&rom, &converted))| (index, i64::from(rom) - i64::from(converted)))
        .collect()
}

#[test]
fn a_standard_speed_block_emits_the_train_the_rom_writes() {
    // The answer to "does a standard-speed `.tzx` agree with the `SA-BYTES` oracle": 3305
    // half-periods, two implementations, compared by value and in order, with the ROM's three
    // known irregularities asserted by name rather than absorbed into a tolerance.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let ours = converted(&standard_speed_file());
    let measured = rom_emitted_pulses(&rom, ours.len());

    assert_eq!(
        ours.len(),
        3305,
        "a data pilot, the sync pair, and five bytes"
    );
    assert_eq!(
        differences(&measured, &ours),
        expected_differences(pilot_length(&ours), block_data().len())
    );
}

#[test]
fn a_turbo_block_carrying_the_roms_values_emits_the_train_the_rom_writes() {
    // **The strong instrument.** Every number in this train came out of five `WORD`s and one
    // `BYTE[3]` in a file, at offsets transcribed from the format description — no constant of
    // this crate's is involved on the converter's side of the comparison. A transposed pair of
    // offsets, a big-endian word, or a field read one byte early all produce a train that
    // disagrees with the ROM's, and there is no shared code for the two to agree wrongly
    // through.
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let ours = converted(&turbo_speed_file());
    let measured = rom_emitted_pulses(&rom, ours.len());

    assert_eq!(
        differences(&measured, &ours),
        expected_differences(pilot_length(&ours), block_data().len())
    );
}

#[test]
fn the_two_tzx_block_kinds_agree_with_each_other() {
    // A standard-speed block *is* a turbo block at the ROM's values, which is what the format
    // says: "This block must be replayed with the standard Spectrum ROM timing values - see the
    // values in curly brackets in block ID 11". The two reach the train by different routes —
    // one from this crate's constants, one from bytes in the file — so agreeing is a fact about
    // both, and it is what would go red if either route drifted.
    assert_eq!(
        converted(&standard_speed_file()),
        converted(&turbo_speed_file())
    );
}

#[test]
fn the_difference_profile_is_not_vacuous() {
    // The positive control for the comparison above. `differences` returning an empty list
    // would make both gates pass by finding nothing, which is this project's recurring failure:
    // a count of zero and an absence of the subject are the same observation. So the profile
    // must be non-empty, must name the places the ROM is documented to deviate, and must
    // disagree with a train that has been altered.
    let ours = converted(&standard_speed_file());
    let profile = expected_differences(pilot_length(&ours), block_data().len());
    assert_eq!(
        profile.len(),
        block_data().len(),
        "one per byte boundary, plus the first bit"
    );
    assert!(profile.iter().any(|&(_, delta)| delta == 1));
    assert!(profile.iter().any(|&(_, delta)| delta == -3));
    assert_eq!(
        profile.iter().filter(|&&(_, delta)| delta == -1).count(),
        1,
        "exactly one parity boundary"
    );

    let mut altered = ours.clone();
    let first = altered.first_mut().expect("a non-empty train");
    *first += 1;
    assert_eq!(
        differences(&altered, &ours),
        vec![(0, 1)],
        "a train that differs must be reported as differing"
    );
}

#[test]
fn the_contention_the_measurement_removes_is_real_and_bounded() {
    // A positive control for `nominal_now`. If the subtraction were a no-op the assertions above
    // would be measuring a contended clock and would have had to be loosened to pass; if it
    // over-subtracted they would be measuring something shorter than the machine ever ran.
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
    while cpu.bus().now() < 200_000 {
        cpu.step();
    }

    let recorder = cpu.bus();
    assert!(
        recorder.stalls > 0,
        "the writer ran 200,000 T-states without the ULA contending one port access, so the \
         correction is measuring nothing"
    );
    assert!(recorder.nominal_now() < recorder.now());
    let intervals = recorder.edges.windows(2).count();
    assert!(
        recorder.stalls <= MAX_ULA_STALL * intervals as u64,
        "{} T-states of stall across {intervals} edges is more than the pattern can charge",
        recorder.stalls
    );
}
