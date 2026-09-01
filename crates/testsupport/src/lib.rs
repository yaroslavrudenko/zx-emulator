//! Where the corpora live, and what happens when one of them is missing.
//!
//! Every gate in this workspace is backed by data that is **fetched rather than committed**
//! — the FUSE vectors, the `zex` exercisers, and (by policy, though it happens to be
//! committed) the Sinclair ROM. So every gate faces the same question, and it has exactly
//! one right answer:
//!
//! | | |
//! |---|---|
//! | present | it runs |
//! | absent | **the gate fails**, naming the fetch instructions |
//! | absent, [`ALLOW_MISSING_CORPUS_ENV`] set | the gate skips, printing why |
//! | absent, opt-out set, **and `CI` set** | **refused** |
//! | an obsolete spelling set at all | **refused** |
//!
//! # Why this is a crate rather than a module
//!
//! It used to live in `crates/z80/tests/common/vectors.rs`, which put it inside one crate's
//! integration-test tree and therefore out of reach of every other crate's. `crates/spectrum`
//! needs the identical rule for the Sinclair ROM, and `testdata/README.md` already promised
//! it — *"the same rule as every other corpus, through the same environment variable — there
//! is one convention here, not one per corpus"* — while no code implemented it on that side.
//! A shared crate is what makes that sentence true rather than aspirational, and it is the
//! only way the two cannot drift apart on the one question that decides whether a green run
//! means anything.
//!
//! # Why the decision is a pure function
//!
//! [`absence_verdict`] and [`parse_boolean`] take their inputs as arguments and return a
//! value; [`skip_absent_corpus`] is the thin shell that reads the environment and acts on
//! them. That split is not decoration, and it is not only the pattern `docs/STATUS.md`
//! already recommends — under edition 2024 `std::env::set_var` is `unsafe`, and every crate
//! here is `unsafe_code = "forbid"`, so a test **cannot** set an environment variable to
//! exercise the shell in-process. Without the split, the rules would be reachable only by
//! spawning a subprocess, which is exactly the sort of proof nobody re-runs. With it, every
//! rule has a failing case that costs microseconds on every `cargo test`, corpus or no
//! corpus.
//!
//! The shell itself is still exercised, from outside: see `testdata/README.md`.

use std::path::{Path, PathBuf};

/// The **deliberate opt-out** for a checkout that has no corpus.
///
/// The default is that every corpus is required. It used to be the other way around, and
/// that was a hole: with `testdata/fuse` moved aside, `cargo test` exited 0 with the same 87
/// tests and zero failures, because libtest captures stdout on success and the skip notice
/// was a `println!`. Five tests verified nothing and the test count did not even change.
/// Absence has to move the pass/fail surface, not a captured line of text.
///
/// # The name
///
/// It was `Z80_FUSE_ALLOW_MISSING` while it lived in the `z80` crate. That name was already
/// understating its reach — it governs the `zex` gate too — and moving here, where it also
/// governs the Spectrum ROM, would have made it plainly false. The harm is concrete rather
/// than cosmetic: a developer who sets a variable named for one crate and one corpus, in
/// order to work without *that* corpus, silently disarms every other gate as well. The old
/// spelling is now refused outright; see [`reject_obsolete_env`].
pub const ALLOW_MISSING_CORPUS_ENV: &str = "ZX_CORPUS_ALLOW_MISSING";

/// A variable that used to mean something here and no longer does.
struct Obsolete {
    name: &'static str,
    why: &'static str,
}

/// Every spelling that is refused rather than ignored.
///
/// Being *set* is a hard error, never a no-op. A variable that a CI file still exports and
/// the code silently ignores is precisely the failure this whole module exists to catch —
/// `docs/STATUS.md` records `Z80_FUSE_REQUIRED` as *"a guard that exists solely in a file
/// nobody runs"* — so each migration refuses to be silent about itself.
const OBSOLETE_ENV: [Obsolete; 2] = [
    Obsolete {
        name: "Z80_FUSE_REQUIRED",
        why: "The corpus is required by default, so nothing needs to ask for it.",
    },
    Obsolete {
        name: "Z80_FUSE_ALLOW_MISSING",
        why: "The opt-out is no longer specific to the z80 crate or to the FUSE corpus: it \
              governs the zex exercisers and the Sinclair ROM as well, and a name claiming \
              otherwise understates what setting it turns off.",
    },
];

/// `<workspace>/testdata` — the root every fetched-on-demand corpus lives under.
///
/// One computation of the workspace-relative path, so no two gates can end up disagreeing
/// about where `testdata` is. This crate sits at `crates/testsupport`, the same depth as
/// every other crate, so the two `..` steps are the same ones this function has always
/// taken; `the_testdata_root_resolves_to_the_workspace` is what keeps that true if the crate
/// ever moves.
#[must_use]
pub fn testdata_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("testdata")
}

/// What the shared policy decides when a corpus is not on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsenceVerdict {
    /// Fail: nobody declared that this run may proceed without the corpus.
    Undeclared,
    /// Fail: the opt-out is set, but so is `CI`.
    ///
    /// The opt-out is for a human on a fresh clone and for nobody else. Without this it
    /// reproduces the original hole exactly — corpus absent, flag set, suite exits 0 with
    /// the full test count and the notice captured, indistinguishable from a verified run.
    /// The realistic route there is not a fork; it is somebody exporting the variable once
    /// to get a checkout building and never unsetting it.
    RefusedUnderCi,
    /// Skip, printing why.
    Skip,
}

/// The whole absence decision, as a pure function of its two inputs.
///
/// `declared` is whether [`ALLOW_MISSING_CORPUS_ENV`] is set to a true value; `under_ci` is
/// whether `CI` is set to anything at all.
#[must_use]
pub const fn absence_verdict(declared: bool, under_ci: bool) -> AbsenceVerdict {
    match (declared, under_ci) {
        (false, _) => AbsenceVerdict::Undeclared,
        (true, true) => AbsenceVerdict::RefusedUnderCi,
        (true, false) => AbsenceVerdict::Skip,
    }
}

/// Interpret the value of a boolean environment variable, or `None` if it is not one.
///
/// Only `"1"` used to be honoured, which meant `true`, `TRUE`, `yes` and `on` all disarmed
/// the flag silently — and `true` is the natural YAML spelling, exactly how an unquoted
/// boolean is serialised into the environment. A variable that is set but unrecognised is a
/// configuration error, never a silent `false`, which is why this returns `None` rather than
/// defaulting.
#[must_use]
pub fn parse_boolean(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Read a boolean environment variable, refusing anything ambiguous.
fn boolean_env(name: &str) -> bool {
    let raw = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return false,
        Err(std::env::VarError::NotUnicode(value)) => {
            panic!("{name} is set to a non-Unicode value ({value:?}); expected a boolean")
        }
    };
    parse_boolean(&raw).unwrap_or_else(|| {
        panic!(
            "{name}={raw:?} is not a recognised boolean. Use one of \
             1/true/yes/on or 0/false/no/off (case-insensitive)."
        )
    })
}

/// Refuse every obsolete spelling, in **every** gate rather than only the one that renamed
/// it.
///
/// Called first by every entry point here, so that a CI file still exporting an old name
/// cannot be silently ignored by whichever gate happens not to check.
///
/// # Panics
///
/// If any obsolete spelling is set to anything at all.
pub fn reject_obsolete_env() {
    for obsolete in &OBSOLETE_ENV {
        assert!(
            std::env::var_os(obsolete.name).is_none(),
            "{} is obsolete and no longer read. {}\nTo allow a run without a corpus, set \
             {ALLOW_MISSING_CORPUS_ENV}=1 instead.",
            obsolete.name,
            obsolete.why,
        );
    }
}

/// The shared policy for "the corpus this gate needs is not there".
///
/// **Panics unless the absence has been explicitly declared**, and refuses the declaration
/// under CI. `what` names the corpus in the failure message and `location` says where it was
/// looked for; everything else is identical for every gate, which is the point.
///
/// # Panics
///
/// Per [`absence_verdict`]: when the absence is undeclared, and when it is declared under
/// CI. Also when an obsolete variable is set, or when the opt-out holds a value that is not
/// a recognised boolean.
pub fn skip_absent_corpus(what: &str, location: &Path) {
    reject_obsolete_env();

    let declared = boolean_env(ALLOW_MISSING_CORPUS_ENV);
    match absence_verdict(declared, std::env::var_os("CI").is_some()) {
        AbsenceVerdict::Undeclared => panic!(
            "{what} was not found in {}.\n\
             It is gitignored and fetched on demand — see testdata/README.md.\n\
             If you genuinely mean to run without it, set {ALLOW_MISSING_CORPUS_ENV}=1; that \
             turns every corpus-backed assertion off, so it is refused under CI.",
            location.display(),
        ),
        AbsenceVerdict::RefusedUnderCi => panic!(
            "{ALLOW_MISSING_CORPUS_ENV} is set and so is CI. The opt-out exists so a \
             developer can work on a checkout without a corpus; it must never decide what a \
             pipeline verifies. Fetch {what} in CI — see testdata/README.md.",
        ),
        AbsenceVerdict::Skip => {
            println!("SKIPPING {what}: not present, and {ALLOW_MISSING_CORPUS_ENV} is set.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_absence_rules_are_exhaustive_over_their_two_inputs() {
        // Four rows because there are four inputs, not because four looked like enough.
        // The row that matters is the third: a declared absence under CI is *refused*, which
        // is the difference between an opt-out for a human and an opt-out that quietly
        // decides what a pipeline verifies.
        let cases = [
            (false, false, AbsenceVerdict::Undeclared),
            (false, true, AbsenceVerdict::Undeclared),
            (true, true, AbsenceVerdict::RefusedUnderCi),
            (true, false, AbsenceVerdict::Skip),
        ];
        for (declared, under_ci, expected) in cases {
            assert_eq!(
                absence_verdict(declared, under_ci),
                expected,
                "declared={declared} under_ci={under_ci}"
            );
        }
    }

    #[test]
    fn an_undeclared_absence_is_never_a_skip() {
        // The property, stated separately from the table above: whatever else changes, a
        // corpus that is simply missing must move the pass/fail surface.
        for under_ci in [false, true] {
            assert_ne!(absence_verdict(false, under_ci), AbsenceVerdict::Skip);
        }
    }

    #[test]
    fn every_yaml_spelling_of_a_boolean_is_honoured() {
        // `true` is how an unquoted YAML boolean serialises into the environment, and
        // honouring only "1" is how the flag used to be disarmed silently.
        for raw in ["1", "true", "TRUE", "True", "yes", "on", " true ", "\tON\n"] {
            assert_eq!(parse_boolean(raw), Some(true), "{raw:?}");
        }
        for raw in ["0", "false", "FALSE", "no", "off", " 0 "] {
            assert_eq!(parse_boolean(raw), Some(false), "{raw:?}");
        }
    }

    #[test]
    fn an_unrecognised_value_is_a_configuration_error_not_a_silent_false() {
        for raw in ["", "2", "maybe", "y", "n", "TRUEISH", "1 0"] {
            assert_eq!(
                parse_boolean(raw),
                None,
                "{raw:?} must be rejected rather than read as false"
            );
        }
    }

    #[test]
    fn both_obsolete_spellings_are_refused_and_the_current_one_is_not() {
        let names: Vec<&str> = OBSOLETE_ENV.iter().map(|o| o.name).collect();
        assert!(names.contains(&"Z80_FUSE_REQUIRED"));
        assert!(
            names.contains(&"Z80_FUSE_ALLOW_MISSING"),
            "the pre-rename opt-out must be refused loudly, not silently ignored — a stale \
             CI config setting it would otherwise disarm every gate while looking armed"
        );
        assert!(
            !names.contains(&ALLOW_MISSING_CORPUS_ENV),
            "the current spelling cannot also be obsolete"
        );
    }

    #[test]
    fn the_testdata_root_resolves_to_the_workspace() {
        // `testdata/README.md` is committed — the one file in that tree `.gitignore`
        // un-ignores besides the ROMs — so this is a real check that the relative path is
        // right, and it goes red if this crate is ever moved to a different depth.
        let root = testdata_dir();
        assert!(
            root.join("README.md").is_file(),
            "{} does not look like the workspace testdata root",
            root.display()
        );
    }
}
