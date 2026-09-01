//! Gate: run Patrik Rak's `z80test` MEMPTR build **on the machine**, and read its verdict.
//!
//! # Why this file exists, and what M6 made possible
//!
//! `docs/STATUS.md` carried a row saying MEMPTR is written at exactly one site — `wz` set in
//! `indexed_address`, so `(IX+d)`/`(IY+d)` records it and `BIT n,(HL)` reads it back — that
//! every *other* hardware rule for the register is unimplemented, and that **nothing grades the
//! value**. FUSE has no MEMPTR column and neither exerciser reports on it. That row also named
//! the instrument: `testdata/README.md` documents `z80memptr.tap`, and `tape_corpus.rs` had
//! been sweeping it as **tape-format** corpus — parsing its blocks and checking their parity
//! bytes — without ever executing an instruction of it.
//!
//! **All three halves of that row are now out of date**, which is the normal fate of a row that
//! names its own settling condition: the rules are implemented, `Cpu::set_memptr` is the one
//! writer of twenty-eight sites, and this file plus `crates/z80/tests/memptr_rules.rs` grade the
//! value. The row was also wrong in both directions when it was current — it listed `RLD`/`RRD`
//! as unimplemented when they passed, and omitted `ADD/ADC/SBC HL,rr` when they failed.
//!
//! M6 is what made it runnable. `tape_rom_load.rs` established that this machine loads a `.tap`
//! through the **real ROM's own `LD-BYTES`**, reading the `EAR` bit, with every `IN A,($FE)`
//! contended by the four-case rule and no path that supplies a byte by any other route. That
//! was built as a milestone gate; it is also a general capability, and this is the first use of
//! it as one: **real Spectrum software run as a test oracle.**
//!
//! # A third shape of oracle
//!
//! This project already has two, and this is neither.
//!
//! | | what it compares | who wrote the expectation |
//! |---|---|---|
//! | `fuse_vectors.rs` | one instruction, state in and state out | FUSE's corpus |
//! | `zex_oracle.rs` | a CRC folded over millions of sequences, under CP/M | the exerciser's own tables |
//! | **this file** | a CRC folded over sequences, **on a Spectrum, reported to its screen** | the program's own tables |
//!
//! The difference from `zex_oracle.rs` is not the CPU work — it is everything around it.
//! `zexdoc` runs under a CP/M shell this project wrote, and its report arrives through a BDOS
//! call this project implements. **Nothing of the kind happens here.** The program is loaded
//! from a pulse train by the ROM, it runs on the whole machine, and it prints through the ROM's
//! own routines into the display file. The verdict is read back out of that display file by
//! matching character cells against the glyphs of the character set **in the machine's own
//! ROM** — `screen::read_text`, built for the boot gate for exactly this reason. So a subtly
//! wrong screen layout produces cells that match no glyph and read as `?` rather than quietly
//! resolving into a plausible verdict.
//!
//! # What is asserted
//!
//! **This section used to open *"this program fails here, and its failures are the point"*, and
//! that is no longer true — the program now passes all 160 groups.** The paragraph is rewritten
//! rather than deleted because the reasoning under it was sound and still governs: what the gate
//! pins is the *exact* verdict, whatever that verdict is, so neither an improvement nor a
//! regression can happen quietly. When the verdict was 45 failures that meant pinning 45
//! failures; a gate that had instead asserted success would have been red for the whole life of
//! the defect, which is how a gate gets `#[ignore]`d and then forgotten — this project's own
//! recorded failure, twice.
//!
//! So what is asserted is the **shape of the run** and the **exact verdict**, pinned:
//!
//! - the tape loaded — all four blocks, through `LD-BYTES`, off the `EAR` bit;
//! - the program ran and returned, rather than stopping early or hanging;
//! - **every one of its tests was seen**, by index, with none missing — see
//!   [`Transcript::missing`], which is what makes the transcript's completeness a *measurement*
//!   rather than a hope about the capture mechanism. It is checked against [`EXPECTED_TOTAL`]
//!   rather than against the summary line's own count, **because the passing summary has no
//!   count**: written the other way, this assertion would have switched itself off at the exact
//!   moment the run went green;
//! - the report is *readable* — a line this parser cannot read is a fault, not a pass;
//! - the verdict is exactly [`VERDICT`], and the failing groups are exactly [`FAILING_GROUPS`],
//!   which is now the empty set.
//!
//! That last pair is the working part, and it is not weaker for being green. Any group that
//! starts failing is named, and so is any that stops — the direction the gate fired in when the
//! rules landed, taking the pin from 45 groups to none.
//!
//! # The green was proven able to fail
//!
//! This gate passed on its first complete run, which is exactly what this project distrusts, so
//! two mutations were run against it in a scratch clone. Each had its occurrence count asserted
//! before the write, was re-read from disk afterwards, and was restored from held bytes with the
//! restore proved by `diff`.
//!
//! | Mutation | Result |
//! |---|---|
//! | delete the `wz` write that was then the only one — `crates/z80/src/instructions.rs:660` that day, and `fn indexed_address`'s `self.set_memptr(effective)` at `:795` since the single-writer refactor | **RED**, exit 101 — failing groups **45 → 143** |
//! | flip **one byte** of the tape's 14390: the `CODE` block's parity | **RED**, exit 101 — the ROM refuses the block, the run stops at [`LOAD_FAILED`] having read no verdict at all |
//!
//! The second is this gate's own proof that the program arrives through the loader: `.tap`
//! parity is checked by the **ROM**, not by anything here, so a single wrong byte 14 KB into a
//! pulse train is the whole difference between a verdict and no verdict.
//!
//! The first was run when `wz` had one write site and the pin was 45 failures. It carried a
//! finding beyond "the gate bites": stranding MEMPTR at zero made **17 then-failing groups start
//! passing**. That is why the caveat on [`FAILING_GROUPS`] is stated as strongly as it is, and
//! it is not retired by the run going green — a group's verdict was never a report on whether
//! its own rule is implemented.
//!
//! **Both mutations were re-run against the passing core**, since a gate pinned to success has a
//! different failure mode from one pinned to a failure count, and the earlier runs cannot speak
//! for it:
//!
//! | Mutation, against the passing core | Result |
//! |---|---|
//! | neutralise `Cpu::set_memptr` so every rule writes nothing | **RED**, exit 101 — **0 → 143** failing groups |
//! | drop only the block families' repeat rule — `repeat_block`'s `set_memptr` | **RED**, exit 101 — **0 → 8** failing, named below |
//!
//! The second is the sharper of the two, and its eight groups are worth reading rather than
//! counting: `087 LDIR`, `088 LDDR`, `089 LDIR->NOP'`, `090 LDDR->NOP'`, `093 CPIR`, `094 CPDR`
//! — and then `102 INIR->NOP'`, `103 INDR->NOP'`. **The first six are the rule Boo-boo and
//! Kladov measured in 2006; the last two are the correction David Banks traced on real parts
//! seventeen years later**, which their document explicitly denies. One line of this core
//! carries both, and removing it moves exactly those eight groups and nothing else.
//!
//! It also disposes of a guess made while writing this table: the six were predicted and the
//! eight were measured. Two groups more than expected is a small error to make in a comment and
//! exactly the size of error that, written down unmeasured, becomes a fact nobody rechecks.
//!
//! # What this program does not cover
//!
//! - **It is a CRC oracle, so it localises to a group and no further.** `LD (NN),A` failing
//!   says the folded result over that group's cases is wrong; it does not say which of MEMPTR's
//!   two bytes, or which addressing case, is responsible.
//! - **It grades MEMPTR only where MEMPTR is observable**, which on a Z80 means through the
//!   undocumented bits 3 and 5 that `BIT n,(HL)` takes from it. A rule whose effect never
//!   reaches those bits is invisible to it — as it is to every other instrument, here and
//!   elsewhere.
//! - **Four groups are `Skipped` by the program itself**: `SCF`/`CCF` in their NEC and ST
//!   variants, which are other manufacturers' parts. It skips them after its own probe, so
//!   they are counted and named rather than being silently absent.
//! - **It is not a timing test.** It runs on the whole contended machine — the loader alone
//!   performs thousands of contended port cycles — but nothing in its report is a function of
//!   T-states. `timing_oracle.rs` is what grades those.
//! - **Its `000 SELF TEST` reads the keyboard port** and expects `BF`, so that group is about
//!   the ULA's idle byte and not about the CPU at all. It is why [`ANSWER`] is released the
//!   moment the prompt clears.

mod common;

use std::collections::BTreeMap;

use common::{elapsed, sinclair_rom, write_program};
use spectrum::memory::PAGE_SIZE;
use spectrum::screen::{ATTRIBUTE_FILE_LEN, DISPLAY_FILE_LEN, read_text};
use spectrum::tape::{Tape, tap};
use spectrum::{Key, Spectrum};
use z80::{CpuState, InterruptMode};

// ---------------------------------------------------------------------------------------
// The artifact
// ---------------------------------------------------------------------------------------

/// The program, as it sits on the tape.
///
/// Four blocks, and every field below was read out of this exact file rather than assumed from
/// the `.tap` conventions — `zex_oracle.rs` re-derived `zexdoc`'s group count from its artifact
/// for the same reason, and it is the same reason: a build that differs must fail here, naming
/// the cause, rather than silently loading something else.
///
/// | block | flag | data bytes | what |
/// |---|---|---|---|
/// | 0 | `0x00` | 17 | header: BASIC program `z80memptr`, autostart line 10 |
/// | 1 | `0xFF` | 42 | the BASIC loader |
/// | 2 | `0x00` | 17 | header: `CODE z80memptr`, 14298 bytes at `0x8000` |
/// | 3 | `0xFF` | 14298 | the program |
///
/// The BASIC loader decodes to `CLEAR 32767: LOAD "" CODE: CLS` and `RANDOMIZE USR 32768`,
/// which is what [`loader_stub`] does directly.
const TAPE: &str = "z80memptr.tap";

/// Where the tape's `CODE` block loads, from its own header.
const CODE_ORIGIN: u16 = 0x8000;

/// The data-byte counts of the four blocks, in order, as `LD-BYTES` wants them in `DE`.
///
/// A `.tap` block is a flag byte, this many data bytes, and a parity byte, so each of these is
/// the file's length word minus two.
const BLOCK_LENGTHS: [u16; 4] = [17, 42, 17, 14298];

/// The flag byte of each block: headers are under `0x80`, data blocks are `0xFF`.
const BLOCK_FLAGS: [u8; 4] = [0x00, 0xFF, 0x00, 0xFF];

/// Strings the program must contain, or it is not the program this harness can read.
///
/// The same guard as `zex_oracle.rs`'s image check — not a checksum, so a different genuine
/// build still runs, but a truncated download or an HTTP error page saved under a `.tap` name
/// fails here rather than reporting zero tests. These are also the literals the report parser
/// depends on, so a variant that reworded its output is caught where the wording matters.
const REQUIRED_MARKERS: [&str; 5] = [
    BANNER,
    SUMMARY_PREFIX,
    FAILED_SUFFIX,
    FAILED_MARKER,
    SKIPPED_MARKER,
];

// ---------------------------------------------------------------------------------------
// Where the harness puts things
// ---------------------------------------------------------------------------------------

/// `LD-BYTES`, the ROM's tape loader — the same entry point `tape_rom_load.rs` uses.
///
/// Entered with `IX` at the destination, `DE` the length, `A` the expected flag byte and carry
/// set for load-rather-than-verify; returns with carry set on success.
const LD_BYTES: u16 = 0x0556;

/// `CLS`, the ROM's clear-screen command.
///
/// Called because **the tape's own BASIC calls it** — `CLEAR 32767: LOAD "" CODE: CLS` — and
/// skipping it was measured to change the run, which is the only reason it is here rather than
/// being trimmed as ceremony. Without it the boot screen's copyright message is still on row 22
/// and the print position starts below it, so the program reaches the bottom of the screen
/// after twenty tests instead of after a full page, and the run stalls there.
const CLS: u16 = 0x0D6B;

/// Scratch for the two header blocks and the BASIC block, which are loaded and discarded.
///
/// They are loaded rather than skipped because the tape is a **signal**: there is no seek, and
/// the only way to reach block 3 is to let the ROM read blocks 0 to 2 off the `EAR` bit first.
/// So the whole tape passes through the loader, which is a stronger statement than loading the
/// one block this gate reads.
const SCRATCH: u16 = 0xBC00;

/// The loader stub. Above the program's last byte (`0xB7D9`) and clear of the scratch.
const STUB: u16 = 0xBD00;

/// Where the stub parks if any block fails to load, so `PC` alone tells the two apart.
const LOAD_FAILED: u16 = 0xBE00;

/// Where the stub parks if the program returns, as it would to BASIC after `USR`.
const RETURNED: u16 = 0xBE80;

/// The stub's stack. Uncontended, below the parks, and clear of the program.
const STACK: u16 = 0xBFF0;

// ---------------------------------------------------------------------------------------
// Budgets, both measured
// ---------------------------------------------------------------------------------------

/// Frames to let the ROM start up before the stub takes over.
///
/// `boot.rs` measures the copyright message appearing on frame 87. This is comfortably past it,
/// so the system variables, the channels and the display file are all as the ROM leaves them —
/// which is the environment `RANDOMIZE USR 32768` would have given the program, and which its
/// printing depends on.
const BOOT_FRAMES: u64 = 120;

/// Instructions between screen samples.
///
/// About 20,000 T-states, under a third of a frame. Fine enough that the scroll prompt is never
/// stepped past — though the prompt persists until answered, so that is a margin rather than a
/// requirement — and coarse enough that the sampling costs less than the emulation it watches.
const STEPS_PER_SAMPLE: u32 = 2_000;

/// T-states to allow for the whole run: the load and the program together.
///
/// **Measured, not guessed.** A complete run costs **910,573,713** T-states, of which
/// **309,241,005** — a third of it — is the tape passing through the loader in emulated real
/// time. (That second figure is not an estimate: it is where the run stops when the tape's
/// parity byte is corrupted, which is the moment the last block finishes arriving.) This is
/// ~4x the total. The budget only has to separate "a bit different" from "looping forever": a
/// core with a defect executes the same program and can differ by a few percent, not by a
/// factor — and the mutation that deletes MEMPTR entirely still completes well inside it.
///
/// The total was **921,998,240** while 45 groups failed, and the difference is the program
/// printing fewer lines, not running less: a failure costs an extra CRC line, which costs
/// screenfuls, which cost scroll prompts — nine then against seven now. Worth recording only
/// because a T-state total that moves when no timing changed looks like a defect until the
/// cause is named.
const BUDGET: u64 = 4_000_000_000;

// ---------------------------------------------------------------------------------------
// The stub: what BASIC would have done
// ---------------------------------------------------------------------------------------

/// Load all four blocks through `LD-BYTES`, clear the screen, then call the program.
///
/// `DI` after every `CALL` on purpose: `SA/LD-RET` executes `EI` on the way out, and the ROM's
/// own `LOAD` holds interrupts off across a load for the same reason — an interrupt taken
/// between blocks would run the ROM's frame handler in the middle of its own edge detection.
/// `DI` does not touch the flags, so the carry `LD-BYTES` returns survives it.
///
/// Interrupts are then enabled and `IM 1` selected before the call, because that is the state
/// `RANDOMIZE USR` hands a program from BASIC — and because the ROM's keyboard scan runs on the
/// frame interrupt, so without it the scroll prompt could never be answered.
fn loader_stub() -> Vec<u8> {
    let mut code = vec![0xF3]; // DI

    for (index, (&length, &flag)) in BLOCK_LENGTHS.iter().zip(&BLOCK_FLAGS).enumerate() {
        let destination = if index == 3 { CODE_ORIGIN } else { SCRATCH };
        code.extend([0xDD, 0x21]); // LD IX,nn
        code.extend(destination.to_le_bytes());
        code.push(0x11); // LD DE,nn
        code.extend(length.to_le_bytes());
        code.extend([0x3E, flag]); // LD A,flag
        code.push(0x37); // SCF — load rather than verify
        code.push(0xCD); // CALL LD-BYTES
        code.extend(LD_BYTES.to_le_bytes());
        code.push(0xF3); // DI — undo SA/LD-RET's EI
        code.push(0xD2); // JP NC,LOAD_FAILED — carry clear means this block failed
        code.extend(LOAD_FAILED.to_le_bytes());
    }

    code.push(0xCD); // CALL CLS — what the tape's own BASIC does before RANDOMIZE USR
    code.extend(CLS.to_le_bytes());
    code.extend([0xED, 0x56]); // IM 1
    code.push(0xFB); // EI
    code.push(0xCD); // CALL CODE_ORIGIN
    code.extend(CODE_ORIGIN.to_le_bytes());
    code.push(0xF3); // DI
    code.push(0xC3); // JP RETURNED
    code.extend(RETURNED.to_le_bytes());
    code
}

/// `JP <here>` — a one-instruction park loop, so an address is a verdict.
fn park(at: u16) -> Vec<u8> {
    let mut code = vec![0xC3];
    code.extend(at.to_le_bytes());
    code
}

/// A booted 48K holding the stub and the tape, positioned to run the stub.
fn machine_with(rom: &[u8], tape: Tape) -> Spectrum {
    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");

    // The ROM starts up first: the program prints through the ROM's routines, and those need
    // the system variables and channels the start-up sequence builds.
    machine.run_frames(BOOT_FRAMES);

    write_program(&mut machine, STUB, &loader_stub());
    write_program(&mut machine, LOAD_FAILED, &park(LOAD_FAILED));
    write_program(&mut machine, RETURNED, &park(RETURNED));

    machine.set_cpu_state(CpuState {
        pc: STUB,
        sp: STACK,
        iff1: false,
        iff2: false,
        im: InterruptMode::Mode1,
        ..machine.cpu_state()
    });
    machine.insert_tape(tape);
    machine.tape_mut().play();
    machine
}

// ---------------------------------------------------------------------------------------
// Reading a screen that pages
// ---------------------------------------------------------------------------------------

/// The ROM's paging prompt, which is also this gate's capture mechanism.
///
/// The program prints through the ROM and never writes `SCR-CT` — grepped for, in the loaded
/// image, rather than assumed — so once the screen is full the ROM asks `scroll?` and waits for
/// a key, on this machine exactly as on a real one. A user presses a key, so this does.
///
/// It is worth being precise about why that is a **feature** rather than an obstacle worked
/// around, because the first working draft of this file did it the other way and was wrong.
/// That draft sampled the display continuously and reconstructed the scrollback by finding the
/// shift that explained each new screen as the old one moved up. Two things defeated it, and
/// both are visible in its output: a sample can land in the middle of the ROM writing a
/// character cell, so cells hold half a glyph and read as `?`; and when several lines move at
/// once no small shift matches, while a shift of 23 matches trivially through the blank lines,
/// so it invented history instead of losing it silently.
///
/// **At the prompt the machine is quiescent** — parked in the ROM's key wait — so the screen is
/// whole and no cell is half-written. And between two prompts the ROM prints exactly one
/// screenful, so consecutive pages are contiguous with no gap. The program's own pagination is
/// a more exact transcript than any heuristic, and [`Transcript::missing`] proves it was
/// complete rather than assuming it.
const SCROLL_PROMPT: &str = "scroll?";

/// The key pressed to answer it.
///
/// **Not `SPACE`, and this is not a free choice.** At its prompt the ROM treats `SPACE`, `BREAK`
/// and `STOP` as "abort", so pressing one would end the run and look like the program stopping
/// early — which the group count would then have to catch. `ENTER` is a plain continue.
///
/// It is pressed **only while the prompt is showing**, which matters for a reason the program
/// itself supplies: its group `000 SELF TEST` does an `IN A,($FE)` and expects `BF`, the idle
/// keyboard byte. A key held down across the run would fail the program's own self-test, and
/// the gate would then be grading the harness.
const ANSWER: Key = Key::Enter;

/// One line of the program's report.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestLine {
    name: String,
    outcome: Outcome,
}

/// What the program said about one of its tests.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Ok,
    /// The program skipped the group after its own probe — the NEC and ST `SCF`/`CCF` variants.
    Skipped,
    /// The group's CRC did not match. This is the punch list a MEMPTR fix works from.
    Failed {
        crc: String,
        expected: String,
    },
    /// A line ending in `FAILED` whose CRC line this parser could not read, or a line that
    /// looks like a test line and is not readable as one. **Kept rather than dropped**: a
    /// report we cannot read is not a report that passed.
    Unreadable(String),
}

/// Everything the program printed, keyed by its own test index.
///
/// A `BTreeMap` rather than a list, and the index rather than the order, because the same page
/// can be captured twice — the screen is sampled repeatedly while the prompt is up — and
/// because the index is what makes [`missing`][Self::missing] a completeness proof rather than
/// a count.
#[derive(Debug, Default)]
struct Transcript {
    tests: BTreeMap<u32, TestLine>,
    summary: Option<Summary>,
    /// Every page as captured, for the failure message.
    pages: Vec<Vec<String>>,
}

impl Transcript {
    /// Fold one captured screen into the transcript.
    ///
    /// A `CRC:` line belongs to the failing test printed immediately above it, so the most
    /// recent index on **this page** carries it. Tracked within a page rather than across
    /// pages: a page boundary can fall between a `FAILED` line and its `CRC:` line, and
    /// attaching that orphan to whatever index happened to be last would be a fabrication. The
    /// orphan instead leaves its test as [`Outcome::Unreadable`], which is a fault.
    fn absorb(&mut self, screen: Vec<String>) {
        let mut latest = None;
        for line in &screen {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(SUMMARY_PREFIX)
                && let Some(summary) = read_summary(rest)
            {
                self.summary = Some(summary);
            } else if let Some((index, test)) = read_test_line(line) {
                latest = Some(index);
                self.tests.entry(index).or_insert(test);
            } else if let Some((crc, expected)) = read_crc_line(line)
                && let Some(entry) = latest.and_then(|index| self.tests.get_mut(&index))
                && matches!(entry.outcome, Outcome::Unreadable(_))
            {
                entry.outcome = Outcome::Failed { crc, expected };
            }
        }
        if !self.pages.contains(&screen) {
            self.pages.push(screen);
        }
    }

    /// The test indices the program's own total says exist but which were never seen.
    ///
    /// **This is what makes the capture a measurement.** The program numbers its tests, and its
    /// summary says how many there are, so the two together prove the transcript is complete
    /// without this file having to trust the paging argument above. If the ROM's scroll counter
    /// ever let more lines past than a screen holds, this is what would say so.
    fn missing(&self, total: u32) -> Vec<u32> {
        (0..total)
            .filter(|index| !self.tests.contains_key(index))
            .collect()
    }

    /// Every test the program reported a failure for, as `index name`.
    fn failing(&self) -> Vec<String> {
        self.tests
            .iter()
            .filter(|(_, test)| !matches!(test.outcome, Outcome::Ok | Outcome::Skipped))
            .map(|(index, test)| format!("{index:03} {}", test.name))
            .collect()
    }

    /// Every line this parser could not read, which is a fault rather than a gap.
    fn unreadable(&self) -> Vec<String> {
        self.tests
            .iter()
            .filter_map(|(index, test)| match &test.outcome {
                Outcome::Unreadable(line) => Some(format!("{index:03} {}: {line:?}", test.name)),
                _ => None,
            })
            .collect()
    }

    /// The captured pages as one block of text, for a failure message.
    fn text(&self) -> String {
        self.pages
            .iter()
            .map(|page| {
                page.iter()
                    .map(|line| line.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n--- page ---\n")
    }
}

/// A cheap fingerprint of the screen bank, so `read_text` runs only when the screen moved.
///
/// `read_text` matches 768 cells against 96 glyphs; running it on every sample for the whole
/// run would cost more than the emulation. The display is unchanged for the entire load — the
/// ROM flashes the *border* — so this skips almost all of it.
fn screen_fingerprint(machine: &Spectrum) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let page: &[u8; PAGE_SIZE] = machine.memory().bank(machine.memory().screen_bank());
    // Eight bytes at a time: folding it byte-wise was measured to cost more than the emulation.
    // `as_chunks` rather than `chunks_exact` for the same reason `tape_corpus.rs` uses it — the
    // length is a constant, so the remainder is known empty at compile time and there is no
    // fallible conversion left in the loop.
    page[..DISPLAY_FILE_LEN + ATTRIBUTE_FILE_LEN]
        .as_chunks::<{ size_of::<u64>() }>()
        .0
        .iter()
        .fold(FNV_OFFSET, |hash, chunk| {
            (hash ^ u64::from_le_bytes(*chunk)).wrapping_mul(FNV_PRIME)
        })
}
// ---------------------------------------------------------------------------------------
// Reading the program's report
// ---------------------------------------------------------------------------------------

const BANNER: &str = "Z80 MEMPTR test";
const SUMMARY_PREFIX: &str = "Result: ";
const ALL_PASSED: &str = "all tests passed.";
const FAILED_SUFFIX: &str = " tests failed.";
const OK_MARKER: &str = "OK";
const SKIPPED_MARKER: &str = "Skipped";
const FAILED_MARKER: &str = "FAILED";
const CRC_PREFIX: &str = "CRC:";
const EXPECTED_MARKER: &str = "Expected:";

/// The width of the program's test index — `000` through `159`.
const INDEX_WIDTH: usize = 3;

/// The final line's reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Summary {
    AllPassed,
    /// `Result: NNN of NNN tests failed.`
    Failed {
        failed: u32,
        total: u32,
    },
}

/// Read one report line: `NNN name<padding>VERDICT`.
///
/// Returns `None` for a line that is not a test line at all. A line that *is* one — it opens
/// with the program's index — but whose verdict this parser does not know becomes
/// [`Outcome::Unreadable`] rather than `None`, so it cannot be quietly skipped.
fn read_test_line(line: &str) -> Option<(u32, TestLine)> {
    let (index, rest) = line.split_at_checked(INDEX_WIDTH)?;
    let index: u32 = index.parse().ok()?;
    let rest = rest.strip_prefix(' ')?;

    let (name, outcome) = if let Some(name) = rest.strip_suffix(OK_MARKER) {
        (name, Outcome::Ok)
    } else if let Some(name) = rest.strip_suffix(SKIPPED_MARKER) {
        (name, Outcome::Skipped)
    } else if let Some(name) = rest.strip_suffix(FAILED_MARKER) {
        // The CRC arrives on the next line; until it does this stays unreadable, so a `FAILED`
        // whose detail never came cannot read as a clean failure.
        (name, Outcome::Unreadable(line.to_owned()))
    } else {
        (rest, Outcome::Unreadable(line.to_owned()))
    };

    Some((
        index,
        TestLine {
            name: name.trim().to_owned(),
            outcome,
        },
    ))
}

/// Read a `CRC:xxxxxxxx   Expected:xxxxxxxx` line.
fn read_crc_line(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix(CRC_PREFIX)?;
    let (crc, expected) = rest.split_once(EXPECTED_MARKER)?;
    Some((crc.trim().to_owned(), expected.trim().to_owned()))
}

/// Read the tail of the summary line: the all-passed wording, or `NNN of NNN tests failed.`
fn read_summary(rest: &str) -> Option<Summary> {
    if rest.starts_with(ALL_PASSED) {
        return Some(Summary::AllPassed);
    }
    let counts = rest.strip_suffix(FAILED_SUFFIX)?;
    let (failed, total) = counts.split_once(" of ")?;
    Some(Summary::Failed {
        failed: failed.trim().parse().ok()?,
        total: total.trim().parse().ok()?,
    })
}

// ---------------------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------------------

/// How many tests the program runs, pinned independently of its verdict.
///
/// **It is a separate constant precisely because the passing summary does not carry one.** The
/// program prints `all tests passed.` with no count, so once this core started passing, a
/// completeness check written against the summary's own `total` would have switched itself off
/// — silently, at the exact moment the gate had least else to assert. That is the defect
/// `docs/STATUS.md` records the FUSE harness having: a gate that verifies nothing while looking
/// like it verifies something. [`faults`] checks [`Transcript::missing`] against this number on
/// every run, pass or fail.
const EXPECTED_TOTAL: u32 = 160;

/// What this core scores today, pinned.
///
/// **A mismatch is a finding, not a constant to bump.** This core now passes every group, so
/// any movement at all is a regression, and the failure message names the groups that moved.
const VERDICT: Summary = Summary::AllPassed;

/// The groups that fail today, in the program's own numbering — now none.
///
/// Named rather than counted, for the reason `zex_oracle.rs` asserts a group count: a count
/// alone cannot tell "three fixed and three broken" from "no change". Empty, it says the
/// stronger thing — that *no* group may fail — and [`faults`] still reports any that do by name.
///
/// # What a green run here does and does not establish
///
/// It was 45 groups until the rules landed, and this comment previously argued that the then-111
/// passing groups were not 111 correct rules, *"because a group passes when the program's folded
/// CRC did not distinguish"*. **That argument survives its own success and is the more important
/// half now.** MEMPTR is observable only through bits 3 and 5 of `BIT n,(HL)` — two bits per
/// probe — so this exerciser grades the *aggregate* of a rule set, group by group, and a
/// compensating pair of errors inside one group is a thing it cannot see.
///
/// So this is one of two instruments, not the instrument. `crates/z80/tests/memptr_rules.rs`
/// asserts each rule's exact sixteen-bit result on its own, which is what localises a defect to
/// a rule; this file is what proves those rules add up to the behaviour a program written by
/// somebody else, against real silicon, expects. Neither subsumes the other: the unit tests
/// cannot catch a rule nobody thought to write, and this cannot say which rule is wrong.
///
/// **And the gap is measured, not merely conceded.** Breaking the accumulator-store quirk — one
/// line, three instructions wrong — turns four of those unit tests red but only **two** groups
/// here, `141` and `143`. `104 OUT (N),A` passes with its rule broken, because two bits folded
/// into a CRC do not distinguish that particular wrong value. That is this caveat with a number
/// against it.
///
/// # What the 45 turned out to be
///
/// Recorded because the shape of the answer is the useful part. The failing set carried three
/// asymmetries, all three of which paid out, and one of them not in the direction it pointed:
///
/// 1. **The `LD` family split by direction** — `LD (NN),A` and `LD ([BC,DE]),A` failed while
///    their loads passed, which is exactly the set taking the *"`MEMPTR_hi` = A"* quirk. See
///    `accumulator_store_memptr` in `crates/z80/src/instructions.rs`.
/// 2. **The block family split by repetition** — the rewind is its own rule, `MEMPTR = PC + 1`.
/// 3. **`JP (HL)` and `JP (XY)` failed while needing no rule at all.** They are the family's one
///    documented *exception* and the core already left `MEMPTR` alone for them; they failed
///    because the instructions the exerciser sets up *with* were wrong, and came green with no
///    change to `jump_to_pair`. A CRC oracle localises to a group and no further, and this is
///    what that limitation looks like from the inside — a group can fail for a rule that is not
///    its own.
const FAILING_GROUPS: &[&str] = &[];

#[test]
#[ignore = "loads a 14 KB tape through the ROM in emulated real time and then runs a 160-group \
            exerciser: 127,406,000 instructions / 910,573,713 T-states. Those two are \
            deterministic and reproduce exactly; the wall clock is not, so it carries its \
            conditions — 2.66 s in release and 24.25 s in the dev profile cargo test uses by \
            default, Apple M3 Max, nothing else running, 2026-09-01. Run it with \
            `cargo test --release -p spectrum --test memptr_oracle -- --ignored`. Nothing runs \
            that automatically: this repository has no CI, and `ci/README.md` says why."]
fn the_memptr_exerciser_reports_the_verdict_this_core_earns() {
    let Some(rom) = sinclair_rom() else {
        return;
    };
    let Some(bytes) = tape_bytes() else {
        return;
    };

    let tape = tap::parse(&bytes).unwrap_or_else(|err| panic!("{TAPE} did not parse: {err}"));
    let mut machine = machine_with(&rom, tape);

    let start = elapsed(&machine);
    let mut transcript = Transcript::default();
    let mut fingerprint = 0;
    let mut instructions = 0_u64;
    let mut prompts = 0_u32;

    // Stepped in blocks rather than one at a time with a check after each: the checks cost more
    // than the emulation otherwise, and every stopping condition here is a park loop or a
    // prompt that persists until answered, so nothing can be stepped past.
    while elapsed(&machine) - start < BUDGET {
        for _ in 0..STEPS_PER_SAMPLE {
            machine.step();
        }
        instructions += u64::from(STEPS_PER_SAMPLE);

        let current = screen_fingerprint(&machine);
        if current != fingerprint {
            fingerprint = current;
            let screen = read_text(machine.memory());
            let prompting = screen
                .last()
                .is_some_and(|line| line.contains(SCROLL_PROMPT));
            if prompting {
                // Quiescent: the machine is parked in the ROM's key wait, so this page is whole.
                transcript.absorb(screen);
                if !machine.keyboard().is_pressed(ANSWER) {
                    prompts += 1;
                }
                machine.keyboard_mut().press(ANSWER);
            } else {
                machine.keyboard_mut().release(ANSWER);
            }
        }

        let pc = machine.cpu_state().pc;
        if pc == LOAD_FAILED || pc == RETURNED {
            break;
        }
    }

    // The last page never prompts — the program finishes before the screen fills again — so it
    // is captured here rather than in the loop.
    transcript.absorb(read_text(machine.memory()));

    let t_states = elapsed(&machine) - start;
    let pc = machine.cpu_state().pc;

    // Printed on success as well as on failure. A passing run that prints nothing is
    // indistinguishable from a run that verified nothing, which is the exact defect
    // `docs/STATUS.md` records — and this list is the punch list a MEMPTR fix works from.
    println!("--- pages ---\n{}\n--- end pages ---", transcript.text());
    println!("--- failing groups ---");
    for group in transcript.failing() {
        println!("{group}");
    }
    println!(
        "--- {prompts} scroll prompts answered, {} tests seen, {} failing, summary {:?} ---",
        transcript.tests.len(),
        transcript.failing().len(),
        transcript.summary,
    );
    println!(
        "--- {instructions} instructions / {t_states} T-states, final PC {pc:#06X}, fault {:?} ---",
        machine.fault(),
    );

    let mut faults = faults(&transcript, pc);
    if !faults.is_empty() {
        faults.push(format!("\nthe pages read:\n{}", transcript.text()));
    }
    assert!(
        faults.is_empty(),
        "the MEMPTR exerciser did not pass this gate:\n{}",
        faults.join("\n"),
    );
}

/// Every reason this run fails the gate, or an empty list.
///
/// A pure function over the parsed report for the same reason `zex_oracle.rs` collects its
/// faults instead of asserting inline: a chain of `assert!`s short-circuits, so a run with a
/// changed verdict *and* a truncated report costs a second run to learn the second thing — and,
/// more importantly, inline assertions could only be exercised by running the whole exerciser,
/// which makes proving they bite a manual mutation nobody repeats. Every rule below has its own
/// failing case in a test that runs in microseconds on every `cargo test`.
fn faults(transcript: &Transcript, pc: u16) -> Vec<String> {
    let mut faults = Vec::new();

    if pc == LOAD_FAILED {
        faults.push(
            "a tape block did not load: LD-BYTES returned with carry clear, so the ROM refused \
             a block. Nothing below this line means anything."
                .to_owned(),
        );
        return faults;
    }
    if pc != RETURNED {
        faults.push(format!(
            "the program did not run to completion: it parked at {pc:#06X} rather than \
             returning to {RETURNED:#06X}, so the budget ran out with it still running",
        ));
    }

    let Some(summary) = transcript.summary else {
        faults.push(
            "the program never printed a readable summary line, so there is no verdict to \
             read. A run that stops early prints nothing but OK lines."
                .to_owned(),
        );
        return faults;
    };

    // A line the parser could not read is a fault in its own right. Unlike a failing group,
    // which is expected here, an unreadable one means the verdict below is not evidence.
    for line in transcript.unreadable() {
        faults.push(format!("UNREADABLE REPORT LINE {line}"));
    }

    // Checked against [`EXPECTED_TOTAL`] rather than against the summary's own count, because
    // the passing summary has no count — see EXPECTED_TOTAL. A run that stopped after ten
    // groups and then printed `all tests passed.` would otherwise satisfy every other rule here.
    let missing = transcript.missing(EXPECTED_TOTAL);
    if !missing.is_empty() {
        faults.push(format!(
            "the program runs {EXPECTED_TOTAL} tests and {} were never seen: {missing:?}. The \
             transcript is incomplete, so no verdict below it can be trusted.",
            missing.len(),
        ));
    }
    if let Summary::Failed { total, .. } = summary
        && total != EXPECTED_TOTAL
    {
        faults.push(format!(
            "the program reports {total} tests where this gate expects {EXPECTED_TOTAL}. That \
             is a different build of the exerciser, not a different result from ours.",
        ));
    }

    if summary != VERDICT {
        faults.push(format!(
            "the program reported {summary:?}, but this core is pinned to {VERDICT:?}. See \
             VERDICT — this is a finding about the core, not a constant to bump.",
        ));
    }

    let failing = transcript.failing();
    if failing != FAILING_GROUPS {
        let fixed: Vec<_> = FAILING_GROUPS
            .iter()
            .filter(|group| !failing.iter().any(|seen| seen == *group))
            .collect();
        let broken: Vec<_> = failing
            .iter()
            .filter(|group| !FAILING_GROUPS.contains(&group.as_str()))
            .collect();
        faults.push(format!(
            "the failing set moved, which is what this gate is for.\n  \
             NO LONGER FAILING (a MEMPTR rule was implemented — update FAILING_GROUPS and \
             VERDICT deliberately, having seen this): {fixed:#?}\n  \
             NEWLY FAILING (a regression): {broken:#?}",
        ));
    }

    faults
}

/// The gate's own claim that the tape is the artifact it was written against.
#[test]
fn the_tape_is_the_program_this_gate_expects() {
    let Some(bytes) = tape_bytes() else {
        return;
    };

    let mut lengths = Vec::new();
    let mut flags = Vec::new();
    let mut offset = 0;
    while offset + 2 <= bytes.len() {
        let length = usize::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
        offset += 2;
        assert!(offset + length <= bytes.len(), "{TAPE} is truncated");
        flags.push(bytes[offset]);
        lengths.push(u16::try_from(length - 2).expect("a block that fits a length word"));
        offset += length;
    }
    assert_eq!(
        offset,
        bytes.len(),
        "trailing bytes after the last block of {TAPE}"
    );

    assert_eq!(BLOCK_LENGTHS.to_vec(), lengths, "{TAPE}'s block lengths");
    assert_eq!(BLOCK_FLAGS.to_vec(), flags, "{TAPE}'s block flags");

    for marker in REQUIRED_MARKERS {
        assert!(
            bytes
                .windows(marker.len())
                .any(|window| window == marker.as_bytes()),
            "{TAPE} does not contain {marker:?}, so it is not the MEMPTR exerciser — or it is a \
             build whose report this harness cannot read",
        );
    }
}

/// `testdata/tapes/z80memptr.tap`, or `None` when the shared policy says this run may skip it.
fn tape_bytes() -> Option<Vec<u8>> {
    testsupport::reject_obsolete_env();
    let path = testsupport::testdata_dir().join("tapes").join(TAPE);
    if !path.is_file() {
        testsupport::skip_absent_corpus("the MEMPTR exerciser tape", &path);
        return None;
    }
    Some(std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display())))
}

// ---------------------------------------------------------------------------------------
// The harness's own claims, each with its own failing case
// ---------------------------------------------------------------------------------------
//
// `docs/STATUS.md` records why these exist: prose asserting a guarantee is a hypothesis, and
// this project has already shipped three comments that claimed a protection the code did not
// provide. Every one of these runs without the corpus, so a fresh clone still proves the report
// parser works even when it cannot prove the CPU does.

/// A page as the program prints one, padded to nothing in particular — the parser reads lines.
fn page(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_owned()).collect()
}

#[test]
fn a_report_line_is_read_in_each_of_its_three_forms() {
    assert_eq!(
        Some((
            9,
            TestLine {
                name: "DAA".to_owned(),
                outcome: Outcome::Ok
            }
        )),
        read_test_line("009 DAA                       OK")
    );
    assert_eq!(
        Some((
            3,
            TestLine {
                name: "SCF (NEC)".to_owned(),
                outcome: Outcome::Skipped
            }
        )),
        read_test_line("003 SCF (NEC)            Skipped")
    );
    let (index, failed) =
        read_test_line("143 LD (NN),A             FAILED").expect("a failing line");
    assert_eq!(143, index);
    assert_eq!("LD (NN),A", failed.name);
    assert!(matches!(failed.outcome, Outcome::Unreadable(_)));
}

#[test]
fn a_line_that_is_not_a_report_line_is_not_read_as_one() {
    assert_eq!(None, read_test_line("Result: 045 of 160 tests failed."));
    assert_eq!(None, read_test_line(""));
    assert_eq!(None, read_test_line("Z80 MEMPTR test"));
    // The `?` a cell that matches no ROM glyph reads as — a screen sampled mid-write, or a
    // wrong screen layout. It must not parse as a test.
    assert_eq!(None, read_test_line("0?9 DAA                       OK"));
}

#[test]
fn a_failing_test_is_only_complete_once_its_crc_line_arrives() {
    let mut transcript = Transcript::default();
    transcript.absorb(page(&["143 LD (NN),A             FAILED"]));
    assert_eq!(
        vec!["143 LD (NN),A: \"143 LD (NN),A             FAILED\"".to_owned()],
        transcript.unreadable(),
        "a FAILED line whose detail never came must not read as a clean failure",
    );

    let mut transcript = Transcript::default();
    transcript.absorb(page(&[
        "143 LD (NN),A             FAILED",
        "CRC:34A28C78   Expected:F6AE8C1D",
    ]));
    assert!(transcript.unreadable().is_empty());
    assert_eq!(vec!["143 LD (NN),A".to_owned()], transcript.failing());
}

#[test]
fn a_crc_line_orphaned_by_a_page_break_is_not_attached_to_the_wrong_test() {
    // The page boundary falls between the FAILED line and its CRC. Attaching that orphan to
    // whichever index happened to be last on the next page would be a fabrication, so the test
    // stays unreadable — which is a fault.
    let mut transcript = Transcript::default();
    transcript.absorb(page(&["143 LD (NN),A             FAILED"]));
    transcript.absorb(page(&[
        "CRC:34A28C78   Expected:F6AE8C1D",
        "144 LD RR,NN                  OK",
    ]));

    assert_eq!(
        Some(&TestLine {
            name: "LD RR,NN".to_owned(),
            outcome: Outcome::Ok
        }),
        transcript.tests.get(&144),
        "the orphan must not have overwritten the next page's first test",
    );
    assert_eq!(1, transcript.unreadable().len(), "{transcript:#?}");
}

#[test]
fn a_missing_test_index_is_reported_rather_than_counted_over() {
    let mut transcript = Transcript::default();
    transcript.absorb(page(&[
        "000 SELF TEST                 OK",
        "002 CCF                       OK",
    ]));

    assert_eq!(vec![1, 3], transcript.missing(4));
    assert!(transcript.missing(3).contains(&1));
}

#[test]
fn the_same_page_captured_twice_is_absorbed_once() {
    // The screen is sampled repeatedly while the prompt is up, so this is the normal case
    // rather than an edge one.
    let mut transcript = Transcript::default();
    let screen = page(&["000 SELF TEST                 OK"]);
    transcript.absorb(screen.clone());
    transcript.absorb(screen);

    assert_eq!(1, transcript.tests.len());
    assert_eq!(1, transcript.pages.len());
}

#[test]
fn the_summary_line_is_read_both_ways() {
    assert_eq!(Some(Summary::AllPassed), read_summary(ALL_PASSED));
    assert_eq!(
        Some(Summary::Failed {
            failed: 45,
            total: 160
        }),
        read_summary("045 of 160 tests failed.")
    );
}

#[test]
fn a_summary_line_this_parser_cannot_read_is_not_a_pass() {
    assert_eq!(None, read_summary("something went wrong"));
    assert_eq!(None, read_summary("many of some tests failed."));
    assert_eq!(None, read_summary("045 of 160 tests failed"));
}

// --- one failing case per gate rule, so none of them can be decorative ---

/// A transcript that passes the gate exactly: every pinned group failing, the rest `OK`.
///
/// Built from [`FAILING_GROUPS`] and [`EXPECTED_TOTAL`] rather than from literals, so it cannot
/// drift away from them. With the pinned set empty it is simply a wholly passing run, and it
/// keeps working unchanged if a regression ever has to be pinned here again.
fn passing_transcript() -> Transcript {
    let mut transcript = Transcript::default();
    let lines: Vec<String> = (0..EXPECTED_TOTAL)
        .map(|index| {
            match FAILING_GROUPS
                .iter()
                .find(|group| group.starts_with(&format!("{index:03} ")))
            {
                Some(group) => {
                    let name = &group[INDEX_WIDTH + 1..];
                    format!("{index:03} {name} FAILED\nCRC:00000000   Expected:11111111")
                }
                None => format!("{index:03} group {index} OK"),
            }
        })
        .collect();
    transcript.absorb(lines.join("\n").lines().map(str::to_owned).collect());
    transcript.summary = Some(VERDICT);
    transcript
}

#[test]
fn the_pinned_verdict_and_failing_set_agree_with_each_other() {
    // The two constants are written by hand from one run, so nothing but this stops them
    // disagreeing — a count of 45 beside a list of 44 would make every future failure
    // unreadable. It held while the pin was a failing set and it still has to hold now that
    // the two agree on zero, which is why it is repointed rather than deleted: if a regression
    // is ever pinned here, this is what stops the count and the list drifting apart again.
    let expected_failures = match VERDICT {
        Summary::AllPassed => 0,
        Summary::Failed { failed, total } => {
            assert!(
                failed < total,
                "a run where everything fails is not a verdict"
            );
            assert_eq!(total, EXPECTED_TOTAL, "the pin must name the same build");
            failed as usize
        }
    };
    assert_eq!(
        expected_failures,
        FAILING_GROUPS.len(),
        "VERDICT expects {expected_failures} failing group(s) and FAILING_GROUPS names {}",
        FAILING_GROUPS.len(),
    );
    let mut sorted = FAILING_GROUPS.to_vec();
    sorted.sort_unstable();
    assert_eq!(
        sorted, FAILING_GROUPS,
        "FAILING_GROUPS must be in index order"
    );
}

#[test]
fn the_run_this_gate_is_pinned_to_has_no_faults() {
    assert_eq!(
        Vec::<String>::new(),
        faults(&passing_transcript(), RETURNED)
    );
}

#[test]
fn a_load_failure_stops_the_report_being_read_at_all() {
    let faults = faults(&Transcript::default(), LOAD_FAILED);
    assert_eq!(1, faults.len(), "{faults:#?}");
    assert!(faults[0].contains("did not load"), "{faults:#?}");
}

#[test]
fn a_run_that_never_returned_is_a_fault() {
    let faults = faults(&passing_transcript(), 0x1234);
    assert!(
        faults.iter().any(|fault| fault.contains("0x1234")),
        "{faults:#?}"
    );
}

#[test]
fn a_run_with_no_summary_is_a_fault_even_though_no_line_said_failed() {
    // The scenario the summary assertion exists for: every line that exists says OK.
    let mut transcript = Transcript::default();
    transcript.absorb(page(&["000 SELF TEST                 OK"]));

    let faults = faults(&transcript, RETURNED);

    assert_eq!(1, faults.len(), "{faults:#?}");
    assert!(faults[0].contains("readable summary"), "{faults:#?}");
}

#[test]
fn an_incomplete_transcript_is_a_fault_even_when_every_line_seen_says_ok() {
    // The rule that had to survive the core starting to pass. Before, the completeness check
    // read its `total` out of a `Failed` summary; a passing run carries no total, so written
    // that way this assertion would have quietly stopped running — and a truncated transcript
    // whose visible lines all say OK is exactly what it exists to catch.
    let mut transcript = passing_transcript();
    let last = EXPECTED_TOTAL - 1;
    transcript.tests.remove(&last);

    let faults = faults(&transcript, RETURNED);

    assert!(
        faults
            .iter()
            .any(|fault| fault.contains("never seen") && fault.contains(&last.to_string())),
        "{faults:#?}"
    );
}

#[test]
fn a_changed_verdict_is_a_fault_that_says_it_is_not_a_constant_to_bump() {
    let mut transcript = passing_transcript();
    transcript.summary = Some(Summary::Failed {
        failed: 1,
        total: EXPECTED_TOTAL,
    });

    let faults = faults(&transcript, RETURNED);

    assert!(
        faults
            .iter()
            .any(|fault| fault.contains("not a constant to bump")),
        "{faults:#?}"
    );
}

#[test]
fn a_regression_is_a_fault_that_names_the_group_that_broke() {
    // The outcome this gate now exists to notice. It used to assert the mirror image — that a
    // rule *landing* is reported, naming the group fixed — and that direction fired for real:
    // it is how the 45 pinned groups were taken down. With nothing left failing, the same
    // machinery guards the other way, and the message must still name the group rather than
    // only the count.
    let mut transcript = passing_transcript();
    let broken = 0;
    transcript
        .tests
        .get_mut(&broken)
        .expect("the group")
        .outcome = Outcome::Failed {
        crc: "00000000".to_owned(),
        expected: "11111111".to_owned(),
    };

    let faults = faults(&transcript, RETURNED);

    assert!(
        faults
            .iter()
            .any(|fault| fault.contains("NEWLY FAILING") && fault.contains("000 group 0")),
        "{faults:#?}"
    );
}
