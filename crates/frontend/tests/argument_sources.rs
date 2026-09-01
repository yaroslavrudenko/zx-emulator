//! A URL and a command line, put through the same function and required to agree.
//!
//! # The gate is agreement, not parsing
//!
//! `docs/M8.md` Decision 3 rules that a browser's arguments become the same `Vec<String>` a
//! command line produces, and says what the alternative costs: *"A second argument source
//! consulted by a second code path is the divergence of Decision 1 in miniature: two ways to
//! say 'which ROM', which will eventually disagree about `--rom`, about repeats, about case."*
//!
//! So the discriminating claim is not *"the query parses"* — a table of literal expectations
//! would grade [`host::arguments_from_query`] against a second transcription of its own rules,
//! which is the tautology this repository has a name for. The claim is that **two sources
//! reach the same place**, and the only way to assert that is to run both through
//! [`host::partition`] and compare.
//!
//! That is why `partition` is public. It was private to `src/main.rs` and had no test at all,
//! while carrying a documented behaviour change — *"what a repeated `--rom` no longer does is
//! override"* — that nothing graded. Moving it is what makes this file possible and it should
//! have moved earlier.
//!
//! # What this cannot see
//!
//! That a browser hands over the query string this function is given. `page::query_string` is
//! an FFI call to a page's own script, there is no compiler on the far side of it, and nothing
//! under `cargo test` reaches a `window.location`. This file grades everything after that
//! string arrives; `web/README.md` records where the arrival itself was observed.

use frontend::host::{arguments_from_query, partition};

/// Any path will do: what matters is that both sides of every comparison get the same one.
///
/// Deliberately **not** the shell's `DEFAULT_ROM`. This file is grading how arguments are read,
/// and reaching into the binary for its policy would tie a test of one thing to a decision
/// about another.
const DEFAULT_ROM: &str = "roms/default.rom";

/// A query, and the command line a person would have typed to mean the same thing.
///
/// The right-hand column is written from *what a person would type*, not from what
/// `arguments_from_query` returns — which is the whole point. If the two columns were both
/// derived from the function under test, every row would pass under any implementation.
const AGREEMENT: &[(&str, &[&str])] = &[
    // The table `docs/M8.md` Decision 3 gives, in its own order.
    ("?rom=a.rom&tape=b.tap", &["a.rom", "b.tap"]),
    // Repeats accumulate. Two ROMs is a 128 rather than a person changing their mind, and the
    // query has to mean that too or the browser builds a different machine from the same words.
    ("?rom=a.rom&rom=c.rom", &["a.rom", "c.rom"]),
    ("?snapshot=x.z80", &["x.z80"]),
    ("", &[]),
    // The 128 ROM pair with a tape, which is the longest thing anybody will actually type.
    (
        "?rom=128-0.rom&rom=128-1.rom&tape=t.tap",
        &["128-0.rom", "128-1.rom", "t.tap"],
    ),
    // An extensionless ROM path. This is why the `rom` key emits `--rom` rather than the bare
    // value: without the flag `partition` cannot tell a ROM from a tape, because the only
    // evidence a path carries is its extension.
    ("?rom=roms/48", &["--rom", "roms/48"]),
    // Keys are matched the way extensions are — case-insensitively — so a URL typed in capitals
    // is not a different emulator.
    ("?ROM=a.rom", &["a.rom"]),
    // A bare word with no `=` is its own value.
    ("?x.tap", &["x.tap"]),
    // URL punctuation carrying no argument. All three mean "nothing was named", which is the
    // URL-launched case and must reach the default ROM rather than an empty path.
    ("?", &[]),
    ("?&&", &[]),
    ("?rom=", &[]),
    // A value containing the separator. Split on the first `=` only.
    ("?snapshot=a=b.z80", &["a=b.z80"]),
];

/// A literal command line as the `Vec<String>` `std::env::args` would have produced.
fn words(command_line: &[&str]) -> Vec<String> {
    command_line.iter().map(|&word| word.to_owned()).collect()
}

/// What the shell would build from a query string.
fn from_query(query: &str) -> (Vec<String>, Vec<String>) {
    partition(&arguments_from_query(query), DEFAULT_ROM)
}

/// What the shell would build from a command line.
fn from_command_line(command_line: &[&str]) -> (Vec<String>, Vec<String>) {
    partition(&words(command_line), DEFAULT_ROM)
}

#[test]
fn a_query_string_and_a_command_line_name_the_same_files() {
    for &(query, command_line) in AGREEMENT {
        assert_eq!(
            from_query(query),
            from_command_line(command_line),
            "{query:?} should mean the same as `zx {}`",
            command_line.join(" "),
        );
    }
}

#[test]
fn the_comparison_is_capable_of_failing() {
    // `docs/STATUS.md`: "every gate needs one assertion whose failure means 'I was not looking
    // at the thing'." Every row above is an equality, and an `arguments_from_query` that
    // returned the same thing for every input — or a `partition` that ignored its argument —
    // would satisfy all twelve. These two are the cases that must come out **different**.

    // Different ROMs are different machines.
    assert_ne!(
        from_query("?rom=a.rom"),
        from_command_line(&["b.rom"]),
        "two different ROM names must not partition the same way",
    );

    // The load-bearing one, and the reason the `rom` key emits a flag at all: without `--rom`,
    // an extensionless path is not a ROM, it is an unrecognised file, and the machine is built
    // from the default instead. If `arguments_from_query` ever stopped emitting the flag, every
    // equality row above would still pass — `a.rom` carries its own extension — and only this
    // assertion would notice.
    assert_ne!(
        from_query("?rom=roms/48"),
        from_command_line(&["roms/48"]),
        "an extensionless path is only a ROM when something says so",
    );
}

#[test]
fn the_table_covers_the_cases_it_claims_to() {
    // Pins the table's own shape, so that rows deleted in a merge do not quietly shrink the
    // gate to something that still reads as green.
    assert_eq!(AGREEMENT.len(), 12);

    // And that at least one row exercises each half of `partition`'s answer, so the whole table
    // cannot consist of cases that reach the default and nothing else.
    assert!(
        AGREEMENT
            .iter()
            .any(|&(query, _)| from_query(query).0 != vec![DEFAULT_ROM.to_owned()]),
        "no row names a ROM, so the ROM half of every comparison is the default on both sides",
    );
    assert!(
        AGREEMENT
            .iter()
            .any(|&(query, _)| !from_query(query).1.is_empty()),
        "no row names a tape or a snapshot, so the media half is empty on both sides",
    );
}

#[test]
fn a_query_is_read_as_the_arguments_a_command_line_would_have_carried() {
    // The three shapes `partition` cannot distinguish, asserted directly because it would
    // otherwise put all of them in the same place. These are literals about the *seam*, not
    // about the agreement, and they are kept in a separate test so the distinction survives.
    assert_eq!(
        arguments_from_query("?rom=a.rom&tape=b.tap"),
        vec!["--rom", "a.rom", "b.tap"],
    );
    assert_eq!(arguments_from_query("?snapshot=a=b.z80"), vec!["a=b.z80"]);
    // Not percent-decoded: the value is going back into an HTTP request, so `%20` has to stay
    // `%20` or the fetch asks for a path with a literal space in it. See
    // `host::arguments_from_query`.
    assert_eq!(
        arguments_from_query("?tape=games/a%20b.tap"),
        vec!["games/a%20b.tap"],
    );
    assert!(arguments_from_query("").is_empty());
}
