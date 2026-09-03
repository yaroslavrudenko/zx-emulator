//! Every gate this repository names in prose is a gate that exists.
//!
//! # The defect
//!
//! `crates/frontend/build.rs` said, in the doc comment above the list it duplicates, that
//! *"the copy is kept honest by `tests/bundled_extensions.rs`"*. There was no such file for
//! a fortnight, and the sentence cost exactly what a false gate costs: a format was added to
//! the list and the paragraph beneath it went on predicting that format's arrival, because a
//! reader who is told a thing is checked stops checking it. That is one instance of a family
//! — a test file that was never written, a test function under a name that does not exist —
//! and the family reproduces faster than it is fixed by hand, which is why this is a gate
//! rather than a sweep.
//!
//! # What it grades
//!
//! Two populations, both drawn from every `.md` and `.rs` file in the repository:
//!
//! - **Backticked names shaped like a test in this project** must be a function that exists.
//! - **Backticked paths under a `tests/` directory** must be a file that exists.
//!
//! Nothing here asks whether the named test *checks what the sentence says it checks*. That
//! is the harder claim and no walk of the text can reach it. This grades the cheap half, and
//! the cheap half is the half that was wrong.
//!
//! # Why the population is named by shape rather than by a list
//!
//! A list of "the names worth checking" is itself a citation, and would rot the same way.
//! The shape is [`FEWEST_SEGMENTS`] underscore-separated lowercase segments, which is what
//! this project's test names look like and what nothing else here looks like — and the floor
//! is not a taste, it is measured, by
//! [`the_shape_this_gate_looks_for_is_the_shape_this_repository_writes`] below.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{Document, Repository, backtick_spans};

// ---------------------------------------------------------------------------------------
// The population
// ---------------------------------------------------------------------------------------

/// The fewest underscore-separated segments a name must have to be graded as a test name.
///
/// **Measured, not chosen.** Against the tree as it stood, every backticked snake_case token
/// that named something outside this repository — `split_at_checked`, `panic_bounds_check`,
/// `miniquad_add_plugin`, `js_unwrap_to_buf`, `get_dropped_files`, twenty-six more — had
/// **four segments or fewer**, and every one that named a test of ours had seven or more. The
/// gap between four and seven was empty, so a floor of five sat in it with room on both sides
/// rather than on top of the data.
///
/// # The gap has since closed from both directions, and the floor is now load-bearing
///
/// That paragraph is kept because it is how the number was arrived at, but the tree it
/// describes is gone and the argument it makes no longer holds. Swept again on 2026-09-03:
/// the repository's own cited names now reach down to **five** segments and are common at
/// six, and on the other side at least one outsider has reached five as well —
/// `clippy::multiple_unsafe_ops_per_block`, whose lint name written without its `clippy::`
/// qualifier is a five-segment lowercase token this floor cannot tell from a test of ours.
///
/// So five is no longer a comfortable seat in an empty gap. It is the boundary itself, and
/// both directions off it now cost something real: raising it drops citations that name live
/// tests, and lowering it admits library and lint names. That is a **stronger** reason not to
/// move it than the original one, not a weaker one — the number stopped being arbitrary and
/// started being measured by the data on either side of it.
///
/// The count of this repository's own tests the floor admits is deliberately not written
/// here any more. It was — as a pair of literals — and both had drifted by more than a
/// hundred before anyone noticed, inside the file whose subject is exactly that. The figure
/// is derived on every run instead, by
/// [`the_shape_this_gate_looks_for_is_the_shape_this_repository_writes`], which prints what it
/// measured when it fails. Raising this floor to silence a finding is the obvious way to
/// disarm the gate, and that assertion is what makes it impossible to do quietly.
const FEWEST_SEGMENTS: usize = 5;

/// The percentage of this repository's own test names the floor must keep admitting.
///
/// Not 100: a handful of tests are genuinely short (`state_round_trips`,
/// `every_pixel_is_opaque`), and a gate that forbade a short name would be legislating style
/// rather than catching rot. The margin between this and what the floor actually admits is
/// what leaves room for a few more of those and none at all for raising the floor.
///
/// The handful was written here as **nine**, and was thirteen by the time anybody counted.
/// The literal is gone for the same reason [`FEWEST_SEGMENTS`]'s is: the live figure is the
/// difference between the two counts
/// [`the_shape_this_gate_looks_for_is_the_shape_this_repository_writes`] measures and prints
/// when it fails, so a sentence here restating it can only be a second copy waiting to
/// disagree with the first.
const NAMES_THE_FLOOR_MUST_ADMIT: usize = 95;

/// A name the prose discusses in the past tense.
struct Removed {
    /// The name as it is written in the sentences that discuss it.
    name: &'static str,
    /// Why it is gone, in the words of the file that removed it.
    why: &'static str,
}

/// Names this repository deliberately talks about and deliberately does not have.
///
/// The entries are not exemptions, they are *records*: a deleted test whose deletion is
/// argued in four places is a thing the documents must be able to name. What makes the list
/// safe to have is that it is graded from both ends —
/// [`every_deleted_name_is_still_absent_and_still_discussed`] fails an entry that has come
/// back and an entry nobody mentions any more — so it cannot become the quiet place a real
/// finding goes to be silenced.
const DELETED_ON_PURPOSE: [Removed; 4] = [
    Removed {
        name: "every_key_is_visible_to_a_scan_of_its_own_half_row",
        why: "Removed at M6. It derived both the port it scanned and the value it expected from \
              Key::position(), the function under test, so it proved read() and position() \
              agree and could say nothing about whether either matched hardware — under a \
              review that permuted the matrix, 38 of 40 keys moved and the suite stayed green. \
              crates/spectrum/tests/keyboard_matrix.rs replaced it with a literal table. The \
              four sentences that name it all discuss it in the past tense, and they have to be \
              able to.",
    },
    Removed {
        name: "a_stopped_tape_is_silent",
        why: "Renamed to a_tape_that_was_never_started_is_silent when the tape reached the \
              speaker: the name was broader than its fixture. Tape::new leaves the level low \
              and the motor off, so what it graded was a cassette that has never been played — \
              not one that was played and then stopped, which is a different state and is not \
              silent: Tape::stop holds the signal where it stands, so a tape stopped on a high \
              half-period holds the line high. crates/spectrum/tests/tape_signal.rs argues the \
              rename where the test lives, grades the stopped-on-high state with two gates of \
              its own, and recounts the old name in the past tense — the sentence this entry \
              exists to keep legal.",
    },
    Removed {
        name: "five_sources_at_full_scale_do_not_exceed_the_headroom",
        why: "Renamed to five_sources_at_full_scale_stay_under_the_device_range after the S4 \
              ruling took the tape out of the shared denominator: the five-source sum now \
              deliberately exceeds HEADROOM (0.88125 against 0.6) and what the assertion holds \
              is the device's full scale, so the old name promised a property the mix no longer \
              has. crates/frontend/tests/audio_resampling.rs argues the rename where the gate \
              lives; the historical mentions recount the old name in the past tense — the \
              sentences this entry keeps legal.",
    },
    Removed {
        name: "the_tape_is_quieter_than_the_beeper_at_the_same_level",
        why: "Inverted to the_tape_is_as_loud_as_the_beeper_at_the_same_level by the S4 \
              ruling: quieter was never ruled, it was what the shared FULL_SCALE denominator \
              left over once the thin-48K floor capped TAPE_GAIN, and the constant's own doc \
              called the result structural rather than chosen. S4 took the tape out of the \
              shared scale and derived TAPE_LEVEL from the beeper's own level — the machine \
              played its tape through its own speaker, as loud as the games it ran. \
              crates/frontend/tests/audio_resampling.rs argues the inversion where the test \
              lives and names the old gate in the past tense — the sentence this entry keeps \
              legal.",
    },
];

// ---------------------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------------------

/// Whether `token` is shaped like one of this project's test names.
fn is_test_name_shaped(token: &str) -> bool {
    let segments: Vec<&str> = token.split('_').collect();
    let starts_with_a_letter = token.chars().next().is_some_and(|c| c.is_ascii_lowercase());
    let every_segment_is_plain = segments
        .iter()
        .all(|segment| !segment.is_empty() && segment.bytes().all(is_lower_or_digit));
    segments.len() >= FEWEST_SEGMENTS && starts_with_a_letter && every_segment_is_plain
}

fn is_lower_or_digit(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

/// Whether `token` is a path under some `tests/` directory.
///
/// A `tests/` path is the population worth grading because it is always a citation of a
/// **gate** — the exact class of claim this file exists for. A `src/` path names code, and
/// code is not promising anything.
fn is_test_file_path(token: &str) -> bool {
    token.ends_with(".rs")
        && (token.starts_with("tests/") || token.contains("/tests/"))
        && token.bytes().all(common::is_path_byte)
}

/// Every backticked name in `text` that is shaped like a test, with its line number.
///
/// A trailing `()` is stripped first, because `some_long_test_name()` in a sentence is the
/// same citation as `some_long_test_name` and it would be strange for the gate to reach one
/// and not the other.
fn cited_names(text: &str) -> Vec<(usize, String)> {
    cited(text, is_test_name_shaped, |body| {
        body.strip_suffix("()").unwrap_or(body)
    })
}

/// Every backticked `tests/…` path in `text`, with its line number.
fn cited_test_paths(text: &str) -> Vec<(usize, String)> {
    cited(text, is_test_file_path, |body| body)
}

/// Every backticked span `wanted` accepts, paired with the line it sat on.
///
/// # Three parameters and an identity closure, kept on purpose
///
/// A tidier shape was written here and reverted, so that nobody spends an afternoon
/// re-deriving it. Folding `wanted` and `trim` into one `fn(&str) -> Option<&str>` per
/// population does remove a parameter and the `|body| body` — and costs **four lines**,
/// because each population then needs a named adapter of its own to do the folding.
///
/// The rule this project measures a change against is that a diff which adds lines has to buy
/// something other than taste, and a parameter count is taste. The identity closure is the
/// visible price of a shape that is otherwise honest about what it does: one loop, one
/// predicate, one normalisation.
fn cited(text: &str, wanted: fn(&str) -> bool, trim: fn(&str) -> &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for span in backtick_spans(line) {
            let token = trim(span.body);
            if wanted(token) {
                found.push((index + 1, token.to_owned()));
            }
        }
    }
    found
}

/// Every name this repository defines as a function or a macro.
///
/// A definition, not a mention: the name must be followed by `(` or `<`, and the line must
/// not be a comment. Both filters point the same way — a doc comment reading `fn resolve`
/// is prose *about* a function, and counting it as a definition would let a citation
/// validate itself.
fn defined_names(documents: &[Document]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for document in documents.iter().filter(|d| d.path.ends_with(".rs")) {
        for line in document.text.lines() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            names.extend(definitions_on(line, "fn ", &['(', '<']));
            names.extend(definitions_on(line, "macro_rules! ", &['{', '(']));
        }
    }
    names
}

/// The names introduced on one line by `keyword`, each of which must be followed by one of
/// `openers`.
fn definitions_on(line: &str, keyword: &str, openers: &[char]) -> Vec<String> {
    let mut names = Vec::new();
    let mut consumed = 0;
    while let Some(offset) = line[consumed..].find(keyword) {
        let at = consumed + offset;
        let after = &line[at + keyword.len()..];
        consumed = at + keyword.len();
        let preceded_by_a_boundary = line[..at]
            .bytes()
            .next_back()
            .is_none_or(|byte| !common::is_identifier_byte(byte));
        if !preceded_by_a_boundary {
            continue;
        }
        let name: String = after
            .bytes()
            .take_while(|&byte| common::is_identifier_byte(byte))
            .map(char::from)
            .collect();
        let opens = after[name.len()..]
            .trim_start()
            .starts_with(|c: char| openers.contains(&c));
        if !name.is_empty() && opens {
            names.push(name);
        }
    }
    names
}

/// Every `#[test]` function's name.
///
/// The attribute and the signature are separated by anything from nothing to a four-line
/// `#[ignore = "…"]`, so the search runs forward a bounded distance rather than assuming
/// adjacency.
///
/// # A recorded limit, rather than one nobody has looked at
///
/// The attribute is matched as a **line of its own**, and the **first** `fn` after it is the
/// one taken. Both are narrower than Rust allows. An attribute sharing its line with anything
/// else, and two `fn` keywords on one signature line, are shapes this repository does not
/// write — swept while this note was added, every `#[test]` in the tree is the attribute
/// alone — so widening the scan would be inventing a population to grade.
///
/// It is written down rather than left implicit because a scan that silently declines an
/// input is the failure this whole directory hunts, and because the alternative reading is
/// available and wrong: the narrowness is a bet on a convention, not a property of Rust. What
/// backs the bet is that breaking it costs names, and losing names drives
/// [`the_shape_this_gate_looks_for_is_the_shape_this_repository_writes`] *down* against its
/// floor — the safe direction, since a scan that finds fewer tests reddens rather than
/// quietly grading less.
///
/// No count is given here on purpose. One was, for the length of an afternoon, and the tree
/// gained eleven tests before the paragraph was finished — which is the whole subject of
/// [`FEWEST_SEGMENTS`]'s own correction two screens up.
fn test_names(documents: &[Document]) -> Vec<String> {
    /// The most lines an attribute block between `#[test]` and its `fn` has ever taken here.
    const ATTRIBUTE_ROOM: usize = 12;

    let mut names = Vec::new();
    for document in documents.iter().filter(|d| d.path.ends_with(".rs")) {
        let lines: Vec<&str> = document.text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if line.trim() != "#[test]" {
                continue;
            }
            let room = lines[index + 1..].iter().take(ATTRIBUTE_ROOM);
            names.extend(
                room.filter_map(|l| definitions_on(l, "fn ", &['(', '<']).into_iter().next())
                    .take(1),
            );
        }
    }
    names
}

// ---------------------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------------------

#[test]
fn every_cited_test_name_is_a_function_that_exists() {
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();
    let defined = defined_names(repository.documents());
    let excused: BTreeMap<&str, &str> = DELETED_ON_PURPOSE
        .iter()
        .map(|removed| (removed.name, removed.why))
        .collect();

    let mut graded = 0;
    let mut phantom = Vec::new();
    for document in repository.documents() {
        for (line, name) in cited_names(&document.text) {
            if excused.contains_key(name.as_str()) {
                continue;
            }
            graded += 1;
            if !defined.contains(&name) {
                phantom.push(format!(
                    "{}:{line} names `{name}`{}",
                    document.path,
                    nearest_relative(&name, &defined),
                ));
            }
        }
    }

    // The vacuous pass this repository keeps writing counters against: an extractor that
    // matched nothing, or a `defined` set that swallowed everything, would make the loop
    // above silent for a reason that has nothing to do with the citations being right.
    assert!(
        graded >= 100,
        "only {graded} names were graded; the extractor is not reading the documents",
    );
    assert!(
        phantom.is_empty(),
        "{} citation(s) name a test this repository does not have:\n  {}\n\n\
         A sentence that says a thing is checked is why nobody checks it. Either write the \
         test, correct the name, or — if it was removed on purpose — record it in \
         DELETED_ON_PURPOSE with the reason, which is graded too.",
        phantom.len(),
        phantom.join("\n  "),
    );
}

/// The nearest name that could have been meant, when there is an obvious one.
///
/// Both hits this gate found on the day it was written were **truncations** — a name copied
/// as far as the end of a thought and no further — so the failure message says so rather
/// than leaving the reader to grep. When nothing is close it says nothing, which is the
/// honest answer more often than a guess would be.
///
/// # "Close" needed a floor, because a short name is a prefix of everything
///
/// The prefix test alone had no notion of how much of the name had to match, and this
/// repository defines a function called `a`. So a citation of a genuinely missing test came
/// back reported as *"did you mean `a`?"* — a confident, useless suggestion attached to a
/// true finding, which is the shape [`crate`]'s sibling `cited_lines.rs` spends a page
/// arguing a gate must never produce. Candidates are now limited to names the gate would
/// itself grade: a suggestion that is not test-shaped is not a truncation of a test name, it
/// is a coincidence of spelling.
fn nearest_relative(name: &str, defined: &BTreeSet<String>) -> String {
    let candidates: Vec<&String> = defined
        .iter()
        .filter(|known| is_test_name_shaped(known))
        .filter(|known| known.starts_with(name) || name.starts_with(known.as_str()))
        .collect();
    match candidates.as_slice() {
        [only] => format!(" — did you mean `{only}`?"),
        _ => String::new(),
    }
}

#[test]
fn every_cited_test_file_is_a_file_that_exists() {
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();

    let mut graded = 0;
    let mut phantom = Vec::new();
    for document in repository.documents() {
        for (line, path) in cited_test_paths(&document.text) {
            graded += 1;
            if repository.resolve(&document.path, &path) == common::Resolution::Unknown {
                phantom.push(format!("{}:{line} names `{path}`", document.path));
            }
        }
    }

    assert!(
        graded >= 150,
        "only {graded} test-file paths were graded; the extractor is not reading the documents",
    );
    assert!(
        phantom.is_empty(),
        "{} citation(s) name a test file this repository does not have:\n  {}\n\n\
         `crates/frontend/build.rs` carried one of these for a fortnight while claiming the \
         opposite; the file it named was written the day this gate was.",
        phantom.len(),
        phantom.join("\n  "),
    );
}

#[test]
fn the_shape_this_gate_looks_for_is_the_shape_this_repository_writes() {
    // The floor's own gate. `FEWEST_SEGMENTS` is the one number here that can be moved to
    // make a finding go away, and moving it far enough to matter stops the shape describing
    // this project's tests — which is a claim about the repository, checkable against the
    // repository, and therefore the thing to assert rather than the number.
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();
    let names = test_names(repository.documents());

    assert!(
        names.len() >= 800,
        "found only {} #[test] functions; the collector is not reading the suite",
        names.len(),
    );
    let admitted = names
        .iter()
        .filter(|name| is_test_name_shaped(name))
        .count();
    assert!(
        admitted * 100 >= names.len() * NAMES_THE_FLOOR_MUST_ADMIT,
        "a floor of {FEWEST_SEGMENTS} segments admits {admitted} of {} test names ({}%), \
         below the {NAMES_THE_FLOOR_MUST_ADMIT}% this gate needs to be looking at the tests \
         it grades. Either the floor was raised to silence a finding, or this project's \
         naming has changed and the measurement behind the floor needs re-running.",
        names.len(),
        admitted * 100 / names.len(),
    );
}

#[test]
fn every_deleted_name_is_still_absent_and_still_discussed() {
    // An allowlist that grows without friction is how a gate dies, so this one is graded
    // from both ends. An entry whose test came back is an exemption that now hides a real
    // definition; an entry nobody mentions any more is a record of nothing, and leaving it
    // there is how the list becomes a place to put findings.
    let repository = Repository::walk();
    repository.assert_the_walk_found_the_repository();
    let defined = defined_names(repository.documents());

    for removed in &DELETED_ON_PURPOSE {
        assert!(
            !defined.contains(removed.name),
            "`{}` is on DELETED_ON_PURPOSE and exists again. {}",
            removed.name,
            removed.why,
        );
        let mentions = repository
            .documents()
            .iter()
            .filter(|document| document.text.contains(removed.name))
            .count();
        assert!(
            mentions >= 2,
            "`{}` is on DELETED_ON_PURPOSE but only {mentions} file(s) mention it — this \
             file being one of them. The entry exists so the documents can name it; if they \
             no longer do, delete the entry.",
            removed.name,
        );
    }
}

// ---------------------------------------------------------------------------------------
// The positive control
// ---------------------------------------------------------------------------------------

#[test]
fn the_name_gate_is_capable_of_failing() {
    // Run against synthetic text rather than against the repository, for the reason
    // `crates/frontend/tests/bundled_extensions.rs` gives about its own control: it edits no
    // real file, and it cannot quietly start passing for a new reason when somebody renames
    // something. The defect this whole file exists for is a check nobody had watched fail.
    //
    // Two claims, and the first is the one a control usually forgets. An extractor that
    // returned nothing would make every gate above vacuously green, so this begins by
    // proving it says **yes**.
    let real = "prose about `every_deleted_name_is_still_absent_and_still_discussed` and \
                `crates/testsupport/tests/cited_names.rs`";
    assert_eq!(
        cited_names(real),
        vec![(
            1,
            "every_deleted_name_is_still_absent_and_still_discussed".to_owned()
        )],
        "the extractor cannot read a citation it should accept, so a green above means nothing",
    );
    assert_eq!(
        cited_test_paths(real),
        vec![(1, "crates/testsupport/tests/cited_names.rs".to_owned())],
    );

    // And now the two failures, which are the exact shapes found in the tree: a name that
    // was copied as far as the end of a thought, and a file that was promised and never
    // written.
    //
    // Both phantoms are **assembled** rather than written out, so that this file does not
    // itself contain a backticked citation to something that does not exist. That is not
    // fastidiousness: written as literals, they were extracted from this file's own source
    // by the gate above, which reported them on its first run against the tree. A control
    // that fails the thing it is a control for is worse than no control.
    let repository = Repository::walk();
    let defined = defined_names(repository.documents());
    let phantom_name = "the_name_gate_is_capable_of";
    let truncated = cited_names(&format!("`{phantom_name}` grades the other branch"));
    let [(_, name)] = truncated.as_slice() else {
        panic!("the extractor stopped seeing a truncated name");
    };
    assert!(
        !defined.contains(name),
        "the synthetic truncation exists for real"
    );
    assert_eq!(
        nearest_relative(name, &defined),
        " — did you mean `the_name_gate_is_capable_of_failing`?",
        "a truncation must be reported as one; that is what both real hits were",
    );

    let phantom_file = "crates/testsupport/tests/no_such_gate.rs";
    let promised = cited_test_paths(&format!("kept honest by `{phantom_file}`"));
    let [(_, path)] = promised.as_slice() else {
        panic!("the extractor stopped seeing a cited test file");
    };
    assert_eq!(
        repository.resolve("docs/STATUS.md", path),
        common::Resolution::Unknown,
        "a promised-and-never-written gate must not resolve",
    );
}

#[test]
fn the_shape_test_separates_a_test_name_from_everything_else() {
    // The measurement the floor rests on, kept as an assertion so the numbers in
    // `FEWEST_SEGMENTS`'s comment are reproducible rather than remembered. The left column
    // is what the tree's own snake_case tokens looked like when they named something outside
    // this repository; the right is what they looked like when they named a test of ours.
    for outsider in [
        "split_at_checked",
        "panic_bounds_check",
        "miniquad_add_plugin",
        "get_dropped_files",
        "js_unwrap_to_buf",
        "add_missing_functions_stabs",
        "panic_const_add_overflow",
    ] {
        assert!(
            !is_test_name_shaped(outsider),
            "{outsider} must not be graded"
        );
    }
    for ours in [
        "the_membrane_bindings_hold_down_every_key",
        "paging_bank_two_into_c000_aliases_the_program",
        "every_key_is_visible_to_a_scan_of_its_own_half_row",
    ] {
        assert!(is_test_name_shaped(ours), "{ours} must be graded");
    }
    // And the shape is a shape, not a spell-check: anything with a capital, a `::` or a
    // call in it is a type, a path or an expression, and the gate has no business grading it.
    for other in ["Keyboard", "Key::position", "read()", "T_STATES", "wz"] {
        assert!(!is_test_name_shaped(other), "{other} must not be graded");
    }
}
