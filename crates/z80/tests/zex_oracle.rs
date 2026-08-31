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

// M4 turns this on. `zexall` has the same 67 groups and the same output format; it differs
// only in the flag masks it applies, which is why it can decide the Q latch and `zexdoc`
// cannot.
//
// const ZEXALL: Exerciser = Exerciser {
//     file: "zexall.com",
//     description: "the zexall exerciser",
//     groups: 67,
// };

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

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// The M3 gate.
///
/// # Why this one test is `#[ignore]`d when the FUSE gates are not
///
/// Measured on an Apple M3 Max: **43 s in release, and 20 minutes in the `dev` profile
/// `cargo test` uses by default** — 5.76 billion instructions at 133 M/s optimised and
/// 4.9 M/s unoptimised. The FUSE corpus is 1335 single instructions and finishes in
/// milliseconds either way; this is nine orders of magnitude more work, and putting twenty
/// minutes into the default `cargo test -p z80` would make the fast gates unusable.
///
/// **An ignored gate that nothing runs is not a gate** — that is this project's own
/// recorded failure (`Z80_FUSE_REQUIRED` "appeared only in its own definition and a README
/// example"). So the obligation moves to CI, in release, where it costs 43 s:
///
/// ```sh
/// cargo test --release -p z80 --test zex_oracle -- --ignored --nocapture
/// ```
///
/// Run it locally the same way. The ten harness tests around it are **not** ignored, so
/// `cargo test -p z80` still proves the CP/M shell and the report parser work on every run.
#[test]
#[ignore = "5.8 billion instructions: 43 s in release, ~20 min in dev. Run with --release \
            -- --ignored; CI runs it that way."]
fn zexdoc_conformance() {
    run_exerciser(&ZEXDOC);
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

    let faults = faults(exerciser, outcome, &report, machine.port_accesses());
    assert!(
        faults.is_empty(),
        "{} did not pass the gate:\n{}\n{}",
        exerciser.description,
        faults.join("\n"),
        diagnostics(&machine),
    );
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
