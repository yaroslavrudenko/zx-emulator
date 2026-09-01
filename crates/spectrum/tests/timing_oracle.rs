//! Gate: the whole contention model against **T-state counts measured on real Spectrums**.
//!
//! This is `docs/MACHINE.md`'s verification item 2 — *"a known-timing test program … a number
//! to compare, not a picture to squint at"* — and it is the first thing in `crates/spectrum`
//! whose expectations were not written by this project.
//!
//! # What every other contention gate cannot do
//!
//! `contention_magnitude.rs`, `io_contention.rs`, `block_contention.rs` and
//! `prefix_chain_contention.rs` all assert **relative to**
//! [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE], and each of them
//! says so in its own header. They are exact, they are hand-derived, and every one of them
//! survives that constant being wrong, because they position the machine *by* it. The delay
//! pattern `[6,5,4,3,2,1,0,0]` and the four-case I/O rule are in the same position: they are
//! the emulator community's figures, transcribed here as expectations, so a gate written from
//! them cannot discover that they are wrong.
//!
//! This file positions the machine by the **interrupt** instead, and then lets a program the
//! Spectrum itself runs count how much work fits into one frame. Nothing here is expressed in
//! terms of 14335, of the pattern, or of the four cases. Every expected number is read out of
//! the corpus.
//!
//! # The corpus, and why it is an oracle rather than a second opinion
//!
//! `timing_tests_48k_v1.0.z80` is Richard Butler's 48K timing test suite (ZXSpectrum4.net,
//! 2010). It is a `.z80` snapshot carrying a BASIC front end, 34 machine-code test groups
//! covering the documented instruction set, and — the part that matters — **two tables of
//! expected results, at `0xE200` and `0xE400`**.
//!
//! Each test group is a block of instructions executed in a loop until the frame interrupt
//! fires. The interrupt handler records three numbers: the refresh register `R`, the number of
//! loop iterations completed, and the stack pointer. Every group is run twice — once from
//! uncontended memory where it sits, and once copied to `0x5B00`, in the screen bank, where
//! the ULA charges it. **The measured quantity is therefore how much a whole frame of
//! contention costs**, integrated over ~10<sup>2</sup>–10<sup>3</sup> iterations, which is a
//! quantity no assertion in this workspace has ever compared against anything external.
//!
//! The authors state the provenance in as many words: *"In order to get the correct results we
//! ran the tests on real Spectrums"*, and their results database is explicitly hardware-only —
//! *"Only submit results from genuine hardware no emulators!"*. **Twenty-eight machines have been
//! submitted by nine independent people; twenty-five of them classify, 17 as `TYPE1` and 8 as
//! `TYPE2`, spanning board issues 2, 3, 3B, 4A, 4B and 6A.** The three that do not are two Inves
//! Spanish clones — a machine with no contended memory at all — and one issue 1 board recorded as
//! returning zeros. The counts were parsed from the results table rather than read off it.
//!
//! **That is the anti-circularity argument, and it is worth stating precisely, because a
//! community test suite is usually somebody else's derivation.** The authors also say their own
//! emulator *"works perfectly and passes all the tests"* — which, read alone, is exactly the
//! shape that would make this a circular oracle. It is not, for a reason that does not depend
//! on believing them: the tables encode **two** hardware behaviours, and the machines in the
//! results database sort into those two classes rather than scattering. A table fitted to an
//! emulator has no reason to predict a second class of machine it does not implement, and no
//! reason for twenty-five real machines to sort cleanly into the two.
//!
//! # Early and late timing — what this corpus can and cannot settle
//!
//! The two tables are the suite's own `TYPE1 (Early)` and `TYPE2 (Late)`, and the strings are
//! in the snapshot's BASIC. They differ by one T-state in when the display's contention window
//! opens relative to the interrupt, and **69 of the 73 populated rows differ between them**, so
//! the difference is not subtle from the outside.
//!
//! So the corpus **cannot** tell this project whether `FIRST_CONTENDED_T_STATE` should be
//! 14335 or one more: it has an answer for both, because real Spectrums have both. What it
//! **can** do, and what nothing else here does, is:
//!
//! - **refuse a machine that is neither.** Measured, not argued: moving the window to 14336 while
//!   leaving the interrupt alone scores 7 mismatches against `TYPE1` and **64 against `TYPE2`** —
//!   it does not become the other machine, it stops being either. The mutation table is in
//!   `docs/MACHINE.md`;
//! - say **which** of the two real behaviours this emulator reproduces, as a fact rather than
//!   an intention;
//! - grade the pattern, the phase and the I/O rule **jointly**, integrated over a frame,
//!   against numbers this project did not write.
//!
//! > **Two of the three bullets above are about the 68 graded rows, and the detection run does
//! > none of that work.** An earlier draft said the tables' 120-T-state gap in `R` on the
//! > detection run meant "a model whose pattern or four-case rule is wrong matches nothing", and
//! > that is false: the detection group is `JP (HL)` in **uncontended** memory, so contention
//! > cannot reach it. It was disproved by this file's own mutation run — at 14336 the detection
//! > still reports `TYPE1` and only the contended rows move. The sentence is recorded rather than
//! > deleted because it is the shape `docs/STATUS.md` catalogues twice already: an argument that
//! > names real quantities, predicts the right verdict, and identifies the wrong cause.
//!
//! The authors add a caveat that is itself a finding: a *cold* machine can report late and then
//! report early once warm. The one T-state is not a stable identity of a board, so "which is
//! correct" may not be a question with a single hardware answer — see `docs/MACHINE.md`.
//!
//! # What is not graded here
//!
//! - **Test groups 35–37.** They need a 48K floating bus, which this machine does not model:
//!   [`FLOATING_BUS_BYTE`][spectrum::FLOATING_BUS_BYTE] is a constant. They are counted and
//!   named as skipped rather than dropped.
//! - **The interrupt window's *length*.** The program only needs the interrupt to be accepted
//!   at the top of the frame; 32 T-states versus 24 would not move a number here.
//! - **Anything about a 128.** The suite has a 128 edition; this is the 48K one.
//!
//! # Absence
//!
//! Through `crates/testsupport` unchanged, exactly as every other corpus in this workspace:
//! present, it runs; absent, the gate **fails** naming the fetch; absent with
//! `ZX_CORPUS_ALLOW_MISSING`, it skips; and that opt-out is refused under `CI`.
//!
//! ```sh
//! mkdir -p testdata/timing
//! curl -fSL -o testdata/timing/timing_tests_48k_v1.0.z80 \
//!   https://raw.githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.EmulatorTestSuites.ZXSpectrum/Timing/timing_tests_48k_v1.0.z80
//! ```
//!
//! Provenance, licensing and the SHA-256 are in `docs/MACHINE.md`.

use spectrum::Spectrum;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// Where the snapshot lives under `testdata/`.
const CORPUS: [&str; 2] = ["timing", "timing_tests_48k_v1.0.z80"];

/// Where the committed Sinclair ROM lives under `testdata/`.
const ROM: [&str; 2] = ["roms", "48.rom"];

/// The fetch instructions, repeated in the failure message a developer will actually read.
const FETCH: &str = "curl -fSL -o testdata/timing/timing_tests_48k_v1.0.z80 https://raw.\
                     githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.\
                     EmulatorTestSuites.ZXSpectrum/Timing/timing_tests_48k_v1.0.z80";

/// Read a corpus file, or `None` having applied the shared absence policy.
///
/// `fetch` is printed **before** the policy is consulted, because on the undeclared-absence
/// path the policy panics and nothing after it runs. It is the one thing a developer meeting
/// this failure actually needs, and the shared message can only point at
/// `testdata/README.md`.
fn corpus_file(what: &str, parts: [&str; 2], fetch: Option<&str>) -> Option<Vec<u8>> {
    // Unconditionally, not only on the absent path: an obsolete spelling must be an error in
    // *every* gate, or a CI file still exporting one is silently ignored by whichever gate
    // happens to find its corpus present.
    testsupport::reject_obsolete_env();

    let path: PathBuf = testsupport::testdata_dir().join(parts[0]).join(parts[1]);
    if !path.is_file() {
        if let Some(fetch) = fetch {
            println!(
                "{what} is fetched on demand:\n  mkdir -p testdata/{}\n  {fetch}",
                parts[0]
            );
        }
        testsupport::skip_absent_corpus(what, &path);
        return None;
    }
    Some(std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display())))
}

/// Both corpora this gate needs, or `None` if either is absent.
fn corpora() -> Option<(Vec<u8>, Vec<u8>)> {
    let snapshot = corpus_file(
        "Richard Butler's 48K timing test suite (timing_tests_48k_v1.0.z80)",
        CORPUS,
        Some(FETCH),
    )?;
    let rom = corpus_file("the Sinclair 48K ROM", ROM, None)?;
    Some((snapshot, rom))
}

// ---------------------------------------------------------------------------
// Reading the snapshot
//
// Deliberately not through `spectrum::snapshot`. Two reasons, and the second is the load
// bearing one. This file must be able to run against a machine whose snapshot module is
// mid-change, and — more importantly — an oracle that reaches its expectations through the
// crate under test has one more way to agree with it than it should. Sixty lines of
// run-length decoding is a cheap price for the expectations arriving from outside.
// ---------------------------------------------------------------------------

/// The whole 64 KB address space as a snapshot describes it.
type Image = [u8; 0x1_0000];

/// Where a `.z80` version 2/3 page number lands in a 48K address space.
///
/// Pages 0–3 are the 128's ROM and low banks and cannot appear in a 48K file.
const fn page_base(page: u8) -> Option<usize> {
    match page {
        4 => Some(0x8000),
        5 => Some(0xC000),
        8 => Some(0x4000),
        _ => None,
    }
}

/// Decode the version 2/3 `.z80` in `file` into a 64 KB image, RAM only.
///
/// Structure is validated rather than a checksum pinned — the same choice `testdata/README.md`
/// records for the FUSE vectors and the `zex` exercisers, and for the same reason: any genuine
/// copy of the file must work, and what the gate actually depends on is the content, which
/// [`assert_is_the_timing_suite`] checks directly.
fn load_z80(file: &[u8]) -> Image {
    assert!(file.len() > 34, "a .z80 header is at least 30 bytes");
    assert_eq!(
        u16::from_le_bytes([file[6], file[7]]),
        0,
        "expected a version 2/3 .z80, whose version-1 PC field is zero"
    );
    let extra = usize::from(u16::from_le_bytes([file[30], file[31]]));
    assert!(
        matches!(extra, 23 | 54 | 55),
        "unexpected additional header length {extra}"
    );
    assert_eq!(
        u16::from_le_bytes([file[32], file[33]]),
        0xC000,
        "the suite's entry point is 0xC000"
    );
    assert_eq!(file[34], 0, "expected hardware mode 0 — a 48K");

    let mut image = [0_u8; 0x1_0000];
    let mut seen = [false; 3];
    let mut at = 32 + extra;
    while at + 3 <= file.len() {
        let length = usize::from(u16::from_le_bytes([file[at], file[at + 1]]));
        let page = file[at + 2];
        at += 3;

        let base = page_base(page).unwrap_or_else(|| panic!("page {page} is not a 48K page"));
        let index = match page {
            4 => 0,
            5 => 1,
            _ => 2,
        };
        assert!(!seen[index], "page {page} appears twice");
        seen[index] = true;

        if length == 0xFFFF {
            let end = at + 0x4000;
            assert!(end <= file.len(), "page {page} is truncated");
            image[base..base + 0x4000].copy_from_slice(&file[at..end]);
            at = end;
        } else {
            let end = at + length;
            assert!(end <= file.len(), "page {page} is truncated");
            decompress_page(&file[at..end], &mut image[base..base + 0x4000], page);
            at = end;
        }
    }
    assert_eq!(at, file.len(), "trailing bytes after the last page");
    assert_eq!(seen, [true; 3], "a 48K snapshot carries pages 4, 5 and 8");
    image
}

/// Expand one run-length encoded page. `ED ED count value` is a run; everything else is literal.
fn decompress_page(block: &[u8], page: &mut [u8], number: u8) {
    let mut out = 0_usize;
    let mut at = 0_usize;
    while at < block.len() {
        if at + 3 < block.len() && block[at] == 0xED && block[at + 1] == 0xED {
            let count = usize::from(block[at + 2]);
            assert!(
                out + count <= page.len(),
                "a run overflows page {number} at offset {out}"
            );
            page[out..out + count].fill(block[at + 3]);
            out += count;
            at += 4;
        } else {
            assert!(out < page.len(), "page {number} overflows at offset {out}");
            page[out] = block[at];
            out += 1;
            at += 1;
        }
    }
    assert_eq!(out, page.len(), "page {number} decoded to {out} bytes");
}

// ---------------------------------------------------------------------------
// The suite's own layout, all of it read out of the image
// ---------------------------------------------------------------------------

/// The three numbers the suite's interrupt handler records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reading {
    /// The Z80 refresh register at the moment the interrupt was taken.
    refresh: u8,
    /// Loop iterations the test group completed before the interrupt.
    iterations: u16,
    /// The stack pointer at the moment the interrupt was taken.
    stack_pointer: u16,
}

impl std::fmt::Display for Reading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "R={:3} loop={:5} sp={:5}",
            self.refresh, self.iterations, self.stack_pointer
        )
    }
}

/// Where the running program leaves its measurement.
const RESULT: u16 = 0xEF00;

/// Base of the `TYPE1 (Early)` expectation table.
const EARLY_TABLE: u16 = 0xE200;

/// The `TYPE2 (Late)` table sits this far above it.
const LATE_TABLE_OFFSET: u16 = 512;

/// Bytes per test group in a table: uncontended at +0, contended at +5.
const TABLE_STRIDE: u16 = 10;

/// The suite's own name for a machine's one-T-state timing class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimingType {
    /// `TYPE1 (Early)` — the table at [`EARLY_TABLE`].
    Early,
    /// `TYPE2 (Late)` — the table 512 bytes above it.
    Late,
}

impl TimingType {
    /// The BASIC line the suite prints when it detects this class, verbatim from the snapshot.
    const fn banner(self) -> &'static [u8] {
        match self {
            Self::Early => b"TYPE1 (Early) timings detected.",
            Self::Late => b"TYPE2 (Late) timings detected.",
        }
    }

    const fn table(self) -> u16 {
        match self {
            Self::Early => EARLY_TABLE,
            Self::Late => EARLY_TABLE + LATE_TABLE_OFFSET,
        }
    }
}

/// One row of an expectation table.
fn expected(image: &Image, timing: TimingType, group: u8, contended: bool) -> Reading {
    let at = timing.table() + u16::from(group) * TABLE_STRIDE + if contended { 5 } else { 0 };
    read_reading(image, at)
}

/// Three numbers in the suite's own layout: a byte, then two little-endian words.
fn read_reading(image: &Image, at: u16) -> Reading {
    let byte = |offset: u16| image[usize::from(at.wrapping_add(offset))];
    Reading {
        refresh: byte(0),
        iterations: u16::from_le_bytes([byte(1), byte(2)]),
        stack_pointer: u16::from_le_bytes([byte(3), byte(4)]),
    }
}

// ---------------------------------------------------------------------------
// Running one test group on the machine
// ---------------------------------------------------------------------------

/// `RANDOMIZE USR 49152`, entered after the ROM has already put the address in `BC`.
///
/// The three instructions at `0x34B6` are `LD HL,0x2D2B` / `PUSH HL` / `PUSH BC` / `RET`, so
/// entering here jumps to `BC` with the ROM's own continuation on the stack — which is how the
/// suite is started from BASIC, and therefore the state it was measured in.
const USR_ENTRY: u16 = 0x34B6;

/// Where the machine code the suite runs begins.
const PROGRAM: u16 = 0xC000;

/// Where it stops. The bytes there are `DI / LD A,0x3F / LD I,A / IM 1 / EI / RET` — the
/// suite handing the machine back to BASIC, and the last moment its measurement is intact.
const STOP: u16 = 0xBC28;

/// The suite reads its test number from here, exactly as the BASIC front end pokes it.
const TEST_NUMBER: u16 = 40000;

/// Non-zero here runs the group from `0x5B00`, in the contended bank, instead of where it sits.
const CONTENDED_FLAG: u16 = 40002;

/// Stack pointer the BASIC front end leaves before `USR`.
const ENTRY_SP: u16 = 0xFFFE;

/// A ceiling that is far more than the two frames a group needs, and far less than forever.
const T_STATE_CEILING: u64 = 10_000_000;

/// Run one test group and return what its interrupt handler recorded.
///
/// The two-machine dance is the faithful part rather than a convenience. The suite's numbers are
/// a count of work completed before an interrupt, so they are only meaningful if the program
/// starts where the hardware starts it: at the **top of a frame**. Running the ROM's four-
/// instruction `USR` prologue moves the clock off zero, so the prologue runs on one machine and
/// its result — RAM and registers — is installed into a second machine that is still at frame
/// zero, T-state zero. A fresh [`Spectrum`] is the only thing in this crate's public surface that
/// is definitionally there, which is why the second machine is built rather than rewound.
fn run_group(rom: &[u8], image: &Image, group: u8, contended: bool) -> Reading {
    let mut prologue = machine_holding(rom, image);
    prologue.memory_mut().write(TEST_NUMBER, group);
    prologue
        .memory_mut()
        .write(CONTENDED_FLAG, u8::from(contended));

    let mut state = prologue.cpu_state();
    state.pc = USR_ENTRY;
    state.bc = PROGRAM;
    state.sp = ENTRY_SP;
    prologue.set_cpu_state(state);

    // `LD HL,nn` / `PUSH HL` / `PUSH BC` / `RET`. Bounded well above four so a ROM that does
    // something else fails with a position rather than hanging.
    for _ in 0..16 {
        if prologue.cpu_state().pc == PROGRAM {
            break;
        }
        prologue.step();
    }
    assert_eq!(
        prologue.cpu_state().pc,
        PROGRAM,
        "the ROM's USR prologue did not reach {PROGRAM:#06X}"
    );

    let mut ram = [0_u8; 0xC000];
    for (offset, byte) in ram.iter_mut().enumerate() {
        *byte = prologue
            .memory()
            .read(0x4000 + u16::try_from(offset).expect("48K of RAM"));
    }

    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");
    for (offset, byte) in ram.iter().enumerate() {
        machine
            .memory_mut()
            .write(0x4000 + u16::try_from(offset).expect("48K of RAM"), *byte);
    }
    machine.set_cpu_state(prologue.cpu_state());
    assert_eq!(
        (machine.frames(), machine.frame_t_state()),
        (0, 0),
        "the measurement must start at the top of a frame"
    );

    while machine.cpu_state().pc != STOP {
        machine.step();
        let elapsed = machine.frames() * u64::from(spectrum::timing::T_STATES_PER_FRAME)
            + u64::from(machine.frame_t_state());
        assert!(
            elapsed <= T_STATE_CEILING,
            "test group {group} ({}) ran {elapsed} T-states without reaching {STOP:#06X}; \
             PC={:#06X}",
            label(contended),
            machine.cpu_state().pc
        );
    }
    assert_eq!(
        machine.fault(),
        None,
        "a Spectrum cannot fault: its bus floats to 0xFF, which is RST 38h"
    );

    read_reading_from(&machine, RESULT)
}

/// The same three numbers, read out of a running machine rather than out of the image.
fn read_reading_from(machine: &Spectrum, at: u16) -> Reading {
    let byte = |offset: u16| machine.memory().read(at.wrapping_add(offset));
    Reading {
        refresh: byte(0),
        iterations: u16::from_le_bytes([byte(1), byte(2)]),
        stack_pointer: u16::from_le_bytes([byte(3), byte(4)]),
    }
}

/// A machine holding the ROM and the snapshot's RAM, at the top of frame zero.
fn machine_holding(rom: &[u8], image: &Image) -> Spectrum {
    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");
    for address in 0x4000..=0xFFFF_u32 {
        let address = u16::try_from(address).expect("inside the address space");
        machine.memory_mut().write(address, image[address as usize]);
    }
    machine
}

const fn label(contended: bool) -> &'static str {
    if contended {
        "contended"
    } else {
        "uncontended"
    }
}

// ---------------------------------------------------------------------------
// What is graded
// ---------------------------------------------------------------------------

/// The group the suite uses to decide which of the two machines it is running on.
///
/// It is not one of the 34 instruction groups: it is `JP (HL)` jumping to itself out of
/// uncontended memory, so it measures **only** where the interrupt falls, at four T-states and
/// one `R` increment per iteration.
const DETECTION_GROUP: u8 = 0;

/// The instruction groups, `1..=GROUPS`.
const GROUPS: u8 = 34;

/// Groups the suite carries that this machine cannot run.
///
/// 35, 36 and 37 read the floating bus. Named rather than silently skipped, because
/// `docs/STATUS.md` records what a silently narrowed gate costs.
const NEEDS_FLOATING_BUS: [u8; 3] = [35, 36, 37];

#[test]
fn the_corpus_is_the_timing_suite_and_carries_two_distinct_hardware_tables() {
    // The positive control. Every number this file asserts is read out of the loaded image, so
    // a wrong, renamed or truncated file would supply its own expectations and the gate would
    // agree with itself — this project's recorded "a count of zero and an absence of the
    // subject are the same observation", in the one shape that fits a data-driven oracle.
    let Some((snapshot, _)) = corpora() else {
        return;
    };
    let image = load_z80(&snapshot);

    for timing in [TimingType::Early, TimingType::Late] {
        assert!(
            image
                .windows(timing.banner().len())
                .any(|window| window == timing.banner()),
            "the snapshot must contain the suite's own {:?} banner",
            std::str::from_utf8(timing.banner()).expect("ASCII"),
        );
    }

    let mut differing = 0_usize;
    let mut populated = 0_usize;
    for group in 0..=GROUPS {
        for contended in [false, true] {
            let early = expected(&image, TimingType::Early, group, contended);
            let late = expected(&image, TimingType::Late, group, contended);
            let empty = Reading {
                refresh: 0,
                iterations: 0,
                stack_pointer: 0,
            };
            if early == empty && late == empty {
                continue;
            }
            populated += 1;
            if early != late {
                differing += 1;
            }
        }
    }
    assert!(
        populated >= 2 * usize::from(GROUPS),
        "only {populated} table rows are populated; the tables are not the suite's"
    );
    assert!(
        differing * 10 >= populated * 9,
        "the two tables should differ on almost every row ({differing} of {populated}); if \
         they agree, this gate cannot see the one T-state that separates the two machines"
    );
}

#[test]
fn the_machine_reproduces_one_of_the_two_measured_hardware_timings() {
    // The detection group is `JP (HL)` jumping to itself in **uncontended** memory, so what this
    // grades is where the interrupt falls relative to an instruction boundary — and nothing about
    // contention, which cannot reach it. The two hardware answers are `R` = 2 and `R` = 122
    // against an identical loop count and stack pointer, so a machine whose interrupt lands
    // anywhere else matches neither. What it does *not* do is grade the contention phase; that is
    // the sixty-eight rows in the test below.
    let Some((snapshot, rom)) = corpora() else {
        return;
    };
    let image = load_z80(&snapshot);

    let actual = run_group(&rom, &image, DETECTION_GROUP, false);
    let early = expected(&image, TimingType::Early, DETECTION_GROUP, false);
    let late = expected(&image, TimingType::Late, DETECTION_GROUP, false);

    let detected = if actual == early {
        TimingType::Early
    } else if actual == late {
        TimingType::Late
    } else {
        panic!(
            "this machine is neither of the two real behaviours the suite has ever seen.\n  \
             measured here : {actual}\n  \
             TYPE1 (Early) : {early}\n  \
             TYPE2 (Late)  : {late}"
        )
    };

    // Pinned, so that a change of class is loud rather than absorbed. Which class this machine
    // lands in is a *measurement of the emulator*, not a target: it follows from
    // `timing::FIRST_CONTENDED_T_STATE` and from where the interrupt is asserted.
    assert_eq!(
        detected,
        TimingType::Early,
        "the machine changed timing class; see docs/MACHINE.md before updating this"
    );
}

#[test]
fn every_instruction_group_matches_the_hardware_table_contended_and_not() {
    // Sixty-eight numbers, each of them a whole frame's worth of contention integrated over
    // hundreds of iterations, and not one of them derived from `FIRST_CONTENDED_T_STATE`, from
    // the delay pattern, or from the four-case I/O rule.
    let Some((snapshot, rom)) = corpora() else {
        return;
    };
    let image = load_z80(&snapshot);

    let detection = run_group(&rom, &image, DETECTION_GROUP, false);
    let timing = if detection == expected(&image, TimingType::Early, DETECTION_GROUP, false) {
        TimingType::Early
    } else {
        TimingType::Late
    };

    let mut failures: Vec<String> = Vec::new();
    let mut against_early = 0_usize;
    let mut against_late = 0_usize;
    let mut compared = 0_usize;
    let mut readings: Vec<Reading> = Vec::new();
    for group in 1..=GROUPS {
        assert!(
            !NEEDS_FLOATING_BUS.contains(&group),
            "group {group} needs a floating bus and must not be in the graded range"
        );
        for contended in [false, true] {
            let want = expected(&image, timing, group, contended);
            let got = run_group(&rom, &image, group, contended);
            readings.push(got);
            compared += 1;
            against_early +=
                usize::from(got != expected(&image, TimingType::Early, group, contended));
            against_late +=
                usize::from(got != expected(&image, TimingType::Late, group, contended));
            if got != want {
                failures.push(format!(
                    "  group {group:2} {:<11}  want {want}   got {got}",
                    label(contended)
                ));
            }
        }
    }

    assert_eq!(
        compared,
        2 * usize::from(GROUPS),
        "every group must be run in both forms"
    );

    // The assertion whose failure means "I was not looking at the thing". Every number below
    // is read out of the guest's own RAM at `RESULT`, so a run in which the program never
    // executed — a prologue that landed somewhere else, a stop address reached immediately —
    // would report whatever was already there, identically, sixty-eight times, and the
    // comparison above would be a comparison of one stale word against a table. The floor is
    // deliberately loose: this only has to separate *ran* from *did not run*, and the observed
    // spread is 58 distinct iteration counts across the 68 rows.
    let mut distinct: Vec<u16> = readings.iter().map(|r| r.iterations).collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(
        distinct.len() >= 20,
        "only {} distinct loop counts across {compared} groups — the test program is not \
         running",
        distinct.len()
    );

    // Both counts, always, because the interesting failure is not "we drifted off our table"
    // but "we are between the two real machines" — a shape that is invisible if only the
    // detected table is reported, and one a wrong contention phase produces exactly.
    assert!(
        failures.is_empty(),
        "{} of {compared} groups disagree with the {timing:?} hardware table.\n{}\n\
         Mismatches against TYPE1 (Early): {against_early} of {compared}; against TYPE2 \
         (Late): {against_late} of {compared}. A machine that is genuinely one of the two \
         scores zero against that one.\n\
         Groups {NEEDS_FLOATING_BUS:?} are excluded: they read the floating bus, which this \
         machine does not model.",
        failures.len(),
        failures.join("\n")
    );
}
