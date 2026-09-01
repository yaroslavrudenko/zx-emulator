//! The M3 gate: run the `zexdoc` CP/M exerciser on the core and insist every one of its
//! test groups reports `OK`.
//!
//! # A different shape of oracle
//!
//! The FUSE corpus sets up a state, runs **one** instruction and compares the result. The
//! `zex` exercisers are self-checking `.COM` programs: they run millions of instruction
//! *sequences*, fold every result into a CRC, and compare that CRC against a value built
//! into the binary. So the verdict is not ours to compute — the program prints `OK` or
//! `ERROR` per group and we read it. Nothing in this file can make a wrong core look right,
//! because nothing in this file knows what any answer should be.
//!
//! That also means the assertions here have to be about the *shape of the run*, not just
//! its content. A program that printed three `OK`s and then jumped to warm boot has said
//! nothing about the other sixty-four, and "no line said ERROR" would pass it. So the group
//! **count** is asserted, and so is the terminating banner, and so is the reason the run
//! stopped.
//!
//! # Scope
//!
//! `zexdoc` only. `zexall` is M4: it adjudicates the `SCF`/`CCF` Q-latch rule, which
//! `docs/STATUS.md` records as deliberately unimplemented, so running it now would produce
//! a wall of expected failures and decide nothing. The scaffolding is shaped so that
//! turning it on is one `const` and one `#[test]`.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use common::cpm::{self, CpmMachine, Outcome};
use common::vectors;

// ---------------------------------------------------------------------------
// The exercisers
// ---------------------------------------------------------------------------

/// One CRC-checking exerciser: which file, and how many groups it must report.
struct Exerciser {
    /// File name inside `testdata/zex/`.
    file: &'static str,
    /// What the program is, for failure messages.
    description: &'static str,
    /// The number of test groups the program is published to contain.
    ///
    /// Asserted exactly, and the reason is the failure mode it exists to catch: a run that
    /// stops early still prints only `OK` lines, so "nothing said ERROR" is not evidence
    /// that anything was tested. A count is.
    groups: usize,
}

/// The documented-behaviour exerciser — the M3 gate.
///
/// **67** is `zexdoc`'s published group count, and it was re-derived from this exact
/// artifact rather than taken on trust: the program carries a null-terminated table of
/// 16-bit pointers immediately after the `JP 0` in its start routine, one per group, each
/// pointing at a 0x60-byte descriptor whose last 31 bytes are the `$`-terminated name
/// printed in the report. Walking that table yields 67 entries whose names are exactly the
/// canonical `zexdoc` list.
const ZEXDOC: Exerciser = Exerciser {
    file: "zexdoc.com",
    description: "the zexdoc exerciser",
    groups: 67,
};

/// The undocumented-behaviour exerciser — the M4 gate.
///
/// The **same program** as [`ZEXDOC`]: same 67 groups, same names, same order, same
/// instruction stream. The two binaries differ in 190 bytes — all 67 flag-mask bytes
/// (`0xc7`/`0xd7`/`0x53` become `0xff`) and 31 of the expected CRCs. That is why both runs
/// are asserted against one [`EXERCISER_SCALE`], and why `zexall` grades bits `zexdoc`
/// throws away — all 67 `zexdoc` masks have bits 3 and 5 clear.
///
/// It does **not** follow that `zexall` decides the Q latch. It does not; see claim 2.
///
/// # What its green proves, and the three claims must not blur into one
///
/// 1. **It does grade the undocumented `F3`/`F5` bits.** Established by controlled mutation
///    with a control, not by reading its source: forcing those bits to a constant `0` or
///    `0x28` makes `zexall` fail `<daa,cpl,scf,ccf>` while `zexdoc` stays 67/67, and a
///    control mutation of a *documented* bit (`SCF` not setting carry) fails **both** —
///    which is what proves the group is executed and graded rather than skipped.
///
/// 2. **It cannot separate the Q rule from `A & 0x28` — and the obvious explanation for why
///    is wrong.** The tempting story is that `zexall` restores `F` before each test, so
///    `Q == F` and `((Q ^ F) | A) & 0x28` collapses to `A & 0x28`. **Measured, that is
///    false:** instrumenting the core counted `SCF` and `CCF` executed 16,000 times each
///    with `Q != F` in **15,750 of them — 98.4 %**. It reaches the *shape* constantly.
///
///    What it never reaches is the *bit pattern*. The two rules differ exactly when
///    `((Q ^ F) & !A) & 0x28 != 0`, and across ~32,000 executions that condition held
///    **zero times**. Its 67/67 has accordingly been observed under three implementations —
///    `A & 0x28`, the Q rule, and a core whose latch was stuck at zero. **A verdict
///    identical under three rules is evidence for none of them.**
///
/// 3. **The entry latch is graded only by FUSE**, by exactly two vectors (`37_1`, `3f`),
///    and **mid-sequence `Q` behaviour has no oracle at all**. What would decide it is not
///    an exotic instruction sequence — per claim 2 that shape is everywhere — but a corpus
///    that varies `A` and `F` so bits 3 and 5 actually diverge. See `docs/STATUS.md` for
///    what an instrument would have to look like.
const ZEXALL: Exerciser = Exerciser {
    file: "zexall.com",
    description: "the zexall exerciser",
    groups: 67,
};

/// The instruction budget for one exerciser run.
///
/// Derived from measurement, not guessed: `zexdoc` completes in **5,764,169,610**
/// instructions (46,734,977,142 T-states) on this core, so this is ~1.4x that. A core with
/// a defect still executes the same program, so its instruction count can differ by a few
/// percent, not by half — the budget only has to separate "slightly different" from
/// "looping forever".
///
/// It exists for the same reason as `machine::MAX_STEPS_PER_VECTOR`, at a different scale:
/// a hung suite gives no diagnosis and blocks CI, while an exhausted budget fails with the
/// console attached, and the console names the last group the program reached.
const MAX_INSTRUCTIONS: u64 = 8_000_000_000;

/// The exact size of one exerciser run: `step()` calls and T-states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RunScale {
    instructions: u64,
    t_states: u64,
}

/// What **both** exercisers execute, to the single instruction.
///
/// `zexdoc` and `zexall` are the same program with different flag masks and expected CRCs,
/// so the instruction stream is identical by construction — the masks change what is folded
/// into a checksum, never what runs. Pinning one constant and asserting both runs against it
/// enforces that identity without a third run, and catches the case where the two drift
/// *together*, which comparing them to each other would not.
///
/// **A mismatch is a finding, not a stale constant to bump.** Two things can move it, and
/// they mean opposite things:
///
/// * The **T-state** total is a pure property of the program plus the core's cycle
///   accounting. It changing means the core's timing changed.
/// * The **instruction** total is `step()` calls, which is *also* a property of how this
///   core groups work into one step — a `DD`/`FD` prefix run is one `step`, so a change in
///   prefix handling legitimately moves it. That is worth knowing about either way.
///
/// Measured on this core; both exercisers reproduce it.
const EXERCISER_SCALE: RunScale = RunScale {
    instructions: 5_764_169_610,
    t_states: 46_734_977_142,
};

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// The M3 gate — documented behaviour.
///
/// # Why both gates are `#[ignore]`d when the FUSE gates are not
///
/// Measured on an Apple M3 Max: **~43 s each in release, and ~20 minutes each in the `dev`
/// profile `cargo test` uses by default** — 5.76 billion instructions at 133 M/s optimised
/// and 4.9 M/s unoptimised. The FUSE corpus is 1335 single instructions and finishes in
/// milliseconds either way; this is nine orders of magnitude more work, and putting forty
/// minutes into the default `cargo test -p z80` would make the fast gates unusable.
///
/// **An ignored gate that nothing runs is not a gate** — that is this project's own
/// recorded failure (`Z80_FUSE_REQUIRED` "appeared only in its own definition and a README
/// example"). The obligation therefore belongs to CI, in release, where the pair costs ~90 s:
///
/// ```sh
/// cargo test --release -p z80 --test zex_oracle -- --ignored --nocapture
/// ```
///
/// **It has not been discharged, and saying so is the point of this paragraph.** There is no
/// CI: `.github/` does not exist, the workflow that would install it is parked at `ci/ci.yml`
/// outside the directory that would run it, and `ci/README.md` opens by saying the repository
/// has none. So the command above is the whole mechanism — somebody types it — and every
/// verdict this file has ever produced was produced that way. The harness tests around them
/// are **not** ignored, so
/// `cargo test -p z80` still proves the CP/M shell, the report parser and every gate rule
/// on each run, without needing `testdata/` at all.
#[test]
#[ignore = "5.8 billion instructions: ~43 s in release, ~20 min in dev. Run it with \
            `cargo test --release -p z80 --test zex_oracle -- --ignored`. Nothing runs that \
            automatically: this repository has no CI, and `ci/README.md` says why."]
fn zexdoc_conformance() {
    run_exerciser(&ZEXDOC);
}

/// The M4 gate — undocumented behaviour.
///
/// M4 was written as *"undocumented flags — `zexall` passes"*. It already did, on the first
/// run, which turned the milestone from an implementation task into an evidence one: make it
/// a committed gate and state precisely what its green does and does not prove. Those three
/// claims are on [`ZEXALL`], and they are deliberately kept apart, because the tempting
/// summary — "`zexall` passes, so the undocumented flags are right" — is true of the first
/// claim and false of the second.
#[test]
#[ignore = "5.8 billion instructions: ~43 s in release, ~20 min in dev. Run it with \
            `cargo test --release -p z80 --test zex_oracle -- --ignored`. Nothing runs that \
            automatically: this repository has no CI, and `ci/README.md` says why."]
fn zexall_conformance() {
    run_exerciser(&ZEXALL);
}

fn run_exerciser(exerciser: &Exerciser) {
    vectors::reject_obsolete_env();

    let dir = vectors::testdata_dir().join("zex");
    let path = dir.join(exerciser.file);
    let image = match std::fs::read(&path) {
        Ok(image) => image,
        // Absent is a skip only when the absence has been declared; unreadable for any
        // other reason is a real failure and must not be laundered into one.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            vectors::skip_absent_corpus(exerciser.description, &path);
            return;
        }
        Err(err) => panic!("cannot read {}: {err}", path.display()),
    };

    assert_image_is_an_exerciser(exerciser, &path, &image);

    let mut machine = CpmMachine::load(&image)
        .unwrap_or_else(|err| panic!("{} at {}: {err}", exerciser.description, path.display()));

    let started = Instant::now();
    let outcome = machine.run(MAX_INSTRUCTIONS);
    let elapsed = started.elapsed();

    let report = parse_console(machine.console());
    report_run(exerciser, &machine, outcome, elapsed, &report);

    let scale = RunScale {
        instructions: machine.instructions(),
        t_states: machine.t_states(),
    };

    let mut faults = faults(exerciser, outcome, &report, machine.port_accesses());
    faults.extend(scale_fault(scale));
    assert!(
        faults.is_empty(),
        "{} did not pass the gate:\n{}\n{}",
        exerciser.description,
        faults.join("\n"),
        diagnostics(&machine),
    );
}

/// Whether this run executed the instruction stream both exercisers are pinned to.
///
/// Separate from [`faults`] because it is not a property of the printed report — it is a
/// cross-run invariant, and it is what makes "`zexall` runs the same stream as `zexdoc`" an
/// asserted fact rather than a claim in a comment.
fn scale_fault(scale: RunScale) -> Option<String> {
    (scale != EXERCISER_SCALE).then(|| {
        format!(
            "the run executed {} instructions / {} T-states, but both exercisers are pinned \
             to {} / {}. See EXERCISER_SCALE — this is a finding about the core or the \
             artifact, not a constant to bump.",
            scale.instructions,
            scale.t_states,
            EXERCISER_SCALE.instructions,
            EXERCISER_SCALE.t_states,
        )
    })
}

/// Every reason this run fails the gate, or an empty list.
///
/// # Why this returns a list instead of asserting
///
/// Two reasons, and the second is the one that matters.
///
/// A chain of `assert!`s short-circuits, so a run with a CRC mismatch *and* a truncated
/// report tells you about one of them and costs 43 s to learn the next. Collecting says
/// everything the first time.
///
/// More importantly, inline assertions can only be exercised by running the whole
/// exerciser, which makes proving that they bite a 43-second manual mutation nobody repeats.
/// As a pure function over a parsed report, each rule below has its own failing case in a
/// test that runs in microseconds and runs on every `cargo test`. `docs/STATUS.md` records
/// what happens otherwise: prose asserting a guarantee is a hypothesis, and this project has
/// already shipped three comments that claimed a protection the code did not provide.
fn faults(
    exerciser: &Exerciser,
    outcome: Outcome,
    report: &ConsoleReport,
    port_accesses: u64,
) -> Vec<String> {
    let mut faults = Vec::new();

    if outcome != Outcome::WarmBoot {
        faults.push(format!(
            "the program did not run to completion: {outcome:?} rather than {:?}",
            Outcome::WarmBoot,
        ));
    }

    // The punch list, first: this is what an implementer works from.
    for group in &report.groups {
        if group.outcome != GroupOutcome::Ok {
            faults.push(format!("FAILING GROUP {group}"));
        }
    }

    if report.groups.len() != exerciser.groups {
        faults.push(format!(
            "{} test groups were reported, but {} has {}. A run that stops early prints \
             nothing but OK lines, so the count is what catches it.",
            report.groups.len(),
            exerciser.description,
            exerciser.groups,
        ));
    }

    if !report.complete {
        faults.push(format!(
            "{COMPLETE_MARKER:?} was never printed, so the program did not reach its own end",
        ));
    }

    if port_accesses != 0 {
        faults.push(format!(
            "{port_accesses} port access(es) reached the bus. A CP/M exerciser performs \
             none, so the core decoded something as an I/O instruction that is not one.",
        ));
    }

    faults
}

/// Reject an image that cannot be the program we think it is.
///
/// Not a checksum — a different but genuine build of `zexdoc` should still run, exactly as
/// `testdata/README.md` says of the FUSE files ("the harness validates their structure on
/// load rather than trusting it"). What this catches is the realistic failure: a fetch that
/// saved an HTTP error page, or half a file, under a `.com` name. It also pins the four
/// literals [`parse_console`] depends on to the artifact itself, so a variant that reworded
/// its report fails here, naming the cause, instead of silently reporting zero groups.
fn assert_image_is_an_exerciser(exerciser: &Exerciser, path: &Path, image: &[u8]) {
    for marker in [BANNER_MARKER, COMPLETE_MARKER, OK_MARKER, ERROR_MARKER] {
        assert!(
            image
                .windows(marker.len())
                .any(|window| window == marker.as_bytes()),
            "{} at {} does not contain {marker:?}, so it is not {} — or it is a build whose \
             report this harness cannot read.",
            exerciser.description,
            path.display(),
            exerciser.description,
        );
    }
}

/// Everything worth knowing when an assertion fails, in one block.
fn diagnostics(machine: &CpmMachine) -> String {
    format!(
        "--- console ({} bytes) ---\n{}\n--- end console ---\n\
         instructions: {}, T-states: {}, BDOS calls: {:?}, port accesses: {}",
        machine.console().len(),
        machine.console(),
        machine.instructions(),
        machine.t_states(),
        machine.bdos_calls(),
        machine.port_accesses(),
    )
}

/// Print what the run did — on success as well as on failure.
///
/// A passing run that prints nothing is indistinguishable from a run that verified nothing,
/// which is the exact defect `docs/STATUS.md` records: `cargo test` once exited 0 with 87
/// passing tests while five of them checked an absent corpus. So the console the verdict
/// was read from is printed verbatim, and it is the evidence. libtest captures this unless
/// `--nocapture` is passed, which is why CI passes it.
///
/// The throughput figures are **reported, never asserted**. A threshold here would fail on
/// a loaded CI runner and teach nobody anything; `benches/step.rs` is where throughput is
/// measured properly.
fn report_run(
    exerciser: &Exerciser,
    machine: &CpmMachine,
    outcome: Outcome,
    elapsed: Duration,
    report: &ConsoleReport,
) {
    let seconds = elapsed.as_secs_f64();
    // A Z80 in a 48K Spectrum runs at 3.5 MHz. Comparing against that turns the T-state
    // total into the only figure that means anything to this project.
    let real_time = machine.t_states() as f64 / (seconds * 3_500_000.0);
    let failed = report
        .groups
        .iter()
        .filter(|group| group.outcome != GroupOutcome::Ok)
        .count();

    println!(
        "--- {} console ---\n{}",
        exerciser.description,
        machine.console()
    );
    println!(
        "--- {}: {outcome:?}, {} group(s) reported, {failed} failing, complete={} ---",
        exerciser.description,
        report.groups.len(),
        report.complete,
    );
    println!(
        "--- {} instructions / {} T-states in {:.2}s \
         ({:.0} instructions/s, {real_time:.0}x real time at 3.5 MHz) ---",
        machine.instructions(),
        machine.t_states(),
        seconds,
        machine.instructions() as f64 / seconds,
    );
}

// ---------------------------------------------------------------------------
// Reading the exerciser's report
// ---------------------------------------------------------------------------

/// The literals below are the exerciser's own message strings, and they are asserted to be
/// present in the image before a run starts — see [`assert_image_is_an_exerciser`].
const BANNER_MARKER: &str = "Z80 instruction exerciser";
const COMPLETE_MARKER: &str = "Tests complete";
const OK_MARKER: &str = "  OK";
const ERROR_MARKER: &str = "  ERROR **** crc expected:";
const FOUND_MARKER: &str = " found:";

#[derive(Debug, Clone, PartialEq, Eq)]
enum GroupOutcome {
    Ok,
    /// The group's CRC did not match. This is the punch list an implementer works from.
    Crc {
        expected: String,
        found: String,
    },
    /// The line said `ERROR` in a form this parser does not know. Kept verbatim rather
    /// than dropped, because a report we cannot read is not a report that passed.
    Unreadable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupReport {
    name: String,
    outcome: GroupOutcome,
}

impl std::fmt::Display for GroupReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.outcome {
            GroupOutcome::Ok => write!(f, "{}: OK", self.name),
            GroupOutcome::Crc { expected, found } => {
                write!(f, "{}: CRC expected {expected}, found {found}", self.name)
            }
            GroupOutcome::Unreadable(line) => {
                write!(f, "{}: unreadable report line {line:?}", self.name)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConsoleReport {
    groups: Vec<GroupReport>,
    complete: bool,
}

/// Turn the console text into one entry per test group.
///
/// The exerciser terminates every line with **LF then CR**, in that order — which is why
/// the carriage returns are dropped before splitting rather than trimmed afterwards: a
/// naive split on LF leaves a stray CR at the *start* of each following line, and
/// `strip_suffix("  OK")` would still match, so the bug would be invisible until a name
/// comparison went wrong.
fn parse_console(console: &str) -> ConsoleReport {
    let text: String = console.chars().filter(|c| *c != '\r').collect();

    let groups = text
        .lines()
        .filter_map(|line| {
            if let Some(name) = line.strip_suffix(OK_MARKER) {
                return Some(GroupReport {
                    name: group_name(name),
                    outcome: GroupOutcome::Ok,
                });
            }
            let (name, detail) = line.split_once(ERROR_MARKER)?;
            let outcome = match detail.split_once(FOUND_MARKER) {
                Some((expected, found)) => GroupOutcome::Crc {
                    expected: expected.trim().to_owned(),
                    found: found.trim().to_owned(),
                },
                None => GroupOutcome::Unreadable(line.to_owned()),
            };
            Some(GroupReport {
                name: group_name(name),
                outcome,
            })
        })
        .collect();

    ConsoleReport {
        groups,
        complete: text.contains(COMPLETE_MARKER),
    }
}

/// Strip the dot padding the exerciser uses to align its report into a column.
fn group_name(raw: &str) -> String {
    raw.trim().trim_end_matches('.').trim_end().to_owned()
}

// ---------------------------------------------------------------------------
// The harness's own claims, each with its own failing case
// ---------------------------------------------------------------------------
//
// `docs/STATUS.md` records why these exist: prose asserting a guarantee is a hypothesis,
// and this project has already shipped three comments that claimed a protection the code
// did not provide. Every one of these runs without the corpus, so a fresh clone still
// proves the shell works even when it cannot prove the CPU does.

/// A hand-assembled CP/M program: print `HI` with function 9, `!` with function 2, warm boot.
const HELLO: &[u8] = &[
    0x0E, 0x09, // 0100  LD C,9
    0x11, 0x12, 0x01, // 0102  LD DE,0x0112
    0xCD, 0x05, 0x00, // 0105  CALL 0x0005
    0x0E, 0x02, // 0108  LD C,2
    0x1E, 0x21, // 010A  LD E,'!'
    0xCD, 0x05, 0x00, // 010C  CALL 0x0005
    0xC3, 0x00, 0x00, // 010F  JP 0x0000
    b'H', b'I', b'$', // 0112  the function 9 string
];

#[test]
fn both_bdos_functions_reach_the_console() {
    let mut machine = CpmMachine::load(HELLO).expect("the fixture is a valid image");

    assert_eq!(Outcome::WarmBoot, machine.run(1_000));

    assert_eq!("HI!", machine.console());
    assert_eq!(
        &BTreeMap::from([(cpm::BDOS_PRINT_STRING, 1), (cpm::BDOS_CONSOLE_OUT, 1)]),
        machine.bdos_calls(),
    );
}

#[test]
fn the_stack_is_usable_without_the_program_setting_it() {
    // `HELLO` never touches SP, so its two `CALL`s prove the initial SP and the word at
    // 0x0006 are both sane. zexdoc reads 0x0006 and sets SP from it before doing anything
    // else, so a zero there would have it pushing through the top of memory.
    let mut machine = CpmMachine::load(HELLO).expect("the fixture is a valid image");
    machine.run(1_000);
    assert_eq!("HI!", machine.console());
}

#[test]
fn a_program_that_never_finishes_is_a_failure_not_a_hang() {
    // JP 0x0100 — a tight infinite loop that never reaches warm boot.
    let mut machine = CpmMachine::load(&[0xC3, 0x00, 0x01]).expect("valid image");
    assert_eq!(Outcome::InstructionLimit, machine.run(1_000));
    assert_eq!(1_000, machine.instructions());
}

#[test]
fn port_accesses_are_counted() {
    // OUT (0xFE),A ; IN A,(0xFE) ; JP 0
    let mut machine =
        CpmMachine::load(&[0xD3, 0xFE, 0xDB, 0xFE, 0xC3, 0x00, 0x00]).expect("valid image");
    assert_eq!(Outcome::WarmBoot, machine.run(1_000));
    assert_eq!(2, machine.port_accesses());
}

#[test]
#[should_panic(expected = "has no '$' terminator")]
fn an_unterminated_bdos_string_is_loud() {
    // LD C,9 ; LD DE,0x8000 ; CALL 5 — 0x8000 is untouched RAM, so the scan wraps the whole
    // address space without meeting a '$'. Without the bound in `print_string` this hangs.
    let mut machine =
        CpmMachine::load(&[0x0E, 0x09, 0x11, 0x00, 0x80, 0xCD, 0x05, 0x00]).expect("valid image");
    machine.run(1_000);
}

#[test]
fn an_empty_image_is_rejected() {
    assert_eq!(Some(cpm::ImageError::Empty), CpmMachine::load(&[]).err());
}

#[test]
fn an_oversized_image_is_rejected() {
    let too_big = vec![0u8; cpm::MEMORY_SIZE - usize::from(cpm::PROGRAM_ORIGIN) + 1];
    assert_eq!(
        Some(cpm::ImageError::TooLarge { len: too_big.len() }),
        CpmMachine::load(&too_big).err(),
    );
}

#[test]
fn the_report_parser_reads_ok_and_crc_lines() {
    // The exerciser's real byte sequence: LF then CR, in that order.
    let console = "Z80 instruction exerciser\n\r\
                   <adc,sbc> hl,<bc,de,hl,sp>....  OK\n\r\
                   add hl,<bc,de,hl,sp>..........  ERROR **** crc expected:89fdb635 found:deadbeef\n\r\
                   Tests complete\n\r";

    let report = parse_console(console);

    assert_eq!(
        vec![
            GroupReport {
                name: "<adc,sbc> hl,<bc,de,hl,sp>".to_owned(),
                outcome: GroupOutcome::Ok,
            },
            GroupReport {
                name: "add hl,<bc,de,hl,sp>".to_owned(),
                outcome: GroupOutcome::Crc {
                    expected: "89fdb635".to_owned(),
                    found: "deadbeef".to_owned(),
                },
            },
        ],
        report.groups,
    );
    assert!(report.complete);
}

#[test]
fn the_report_parser_sees_a_run_that_stopped_early() {
    // Three OKs and nothing else. Every line that exists says OK, which is precisely why
    // "no line said ERROR" is not a passing condition and the count assertion exists.
    let console = "Z80 instruction exerciser\n\ra....  OK\n\rb....  OK\n\rc....  OK\n\r";

    let report = parse_console(console);

    assert_eq!(3, report.groups.len());
    assert!(report.groups.iter().all(|g| g.outcome == GroupOutcome::Ok));
    assert!(
        !report.complete,
        "an early stop must not be mistaken for a finished run",
    );
}

#[test]
fn the_report_parser_ignores_the_banner_and_the_footer() {
    let report = parse_console("Z80 instruction exerciser\n\rTests complete\n\r");
    assert!(report.groups.is_empty());
    assert!(report.complete);
}

// --- one failing case per gate rule, so none of them can be decorative ---

/// A report of `count` groups, every one `OK`, ending with the completion banner.
fn clean_report(count: usize) -> ConsoleReport {
    ConsoleReport {
        groups: (0..count)
            .map(|index| GroupReport {
                name: format!("group {index}"),
                outcome: GroupOutcome::Ok,
            })
            .collect(),
        complete: true,
    }
}

#[test]
fn the_pinned_instruction_stream_is_accepted() {
    assert_eq!(None, scale_fault(EXERCISER_SCALE));
}

#[test]
fn a_diverging_instruction_stream_is_a_fault() {
    // One instruction out is enough. The two exercisers are the same program, so any
    // divergence at all means something moved underneath them.
    let drifted = RunScale {
        instructions: EXERCISER_SCALE.instructions + 1,
        ..EXERCISER_SCALE
    };

    let fault = scale_fault(drifted).expect("a divergent stream must be a fault");

    assert!(fault.contains("not a constant to bump"), "{fault}");
}

#[test]
fn a_diverging_t_state_total_is_a_fault() {
    let drifted = RunScale {
        t_states: EXERCISER_SCALE.t_states - 1,
        ..EXERCISER_SCALE
    };

    assert!(scale_fault(drifted).is_some());
}

#[test]
fn a_clean_run_has_no_faults() {
    let faults = faults(&ZEXDOC, Outcome::WarmBoot, &clean_report(ZEXDOC.groups), 0);
    assert_eq!(Vec::<String>::new(), faults);
}

#[test]
fn a_run_that_stopped_early_is_a_fault_even_though_every_line_said_ok() {
    // The scenario the group count exists for: three OKs, no ERROR anywhere, warm boot
    // reached. Everything a naive "did any line say ERROR?" check looks at is clean.
    let mut report = clean_report(3);
    report.complete = false;

    let faults = faults(&ZEXDOC, Outcome::WarmBoot, &report, 0);

    assert_eq!(2, faults.len(), "{faults:#?}");
    assert!(
        faults[0].contains("3 test groups were reported"),
        "{faults:#?}"
    );
    assert!(faults[1].contains("Tests complete"), "{faults:#?}");
}

#[test]
fn a_crc_mismatch_names_the_group_and_both_crcs() {
    let mut report = clean_report(ZEXDOC.groups);
    report.groups[12] = GroupReport {
        name: "<daa,cpl,scf,ccf>".to_owned(),
        outcome: GroupOutcome::Crc {
            expected: "9b4ba675".to_owned(),
            found: "deadbeef".to_owned(),
        },
    };

    let faults = faults(&ZEXDOC, Outcome::WarmBoot, &report, 0);

    assert_eq!(
        vec!["FAILING GROUP <daa,cpl,scf,ccf>: CRC expected 9b4ba675, found deadbeef".to_owned()],
        faults,
    );
}

#[test]
fn an_unreadable_error_line_is_a_fault_rather_than_a_pass() {
    let mut report = clean_report(ZEXDOC.groups);
    report.groups[0].outcome = GroupOutcome::Unreadable("something went wrong".to_owned());

    let faults = faults(&ZEXDOC, Outcome::WarmBoot, &report, 0);

    assert_eq!(1, faults.len(), "{faults:#?}");
    assert!(faults[0].starts_with("FAILING GROUP"), "{faults:#?}");
}

#[test]
fn an_exhausted_instruction_budget_is_a_fault() {
    let faults = faults(
        &ZEXDOC,
        Outcome::InstructionLimit,
        &clean_report(ZEXDOC.groups),
        0,
    );

    assert_eq!(1, faults.len(), "{faults:#?}");
    assert!(faults[0].contains("InstructionLimit"), "{faults:#?}");
}

#[test]
fn a_port_access_is_a_fault() {
    let faults = faults(&ZEXDOC, Outcome::WarmBoot, &clean_report(ZEXDOC.groups), 1);

    assert_eq!(1, faults.len(), "{faults:#?}");
    assert!(faults[0].contains("1 port access"), "{faults:#?}");
}
