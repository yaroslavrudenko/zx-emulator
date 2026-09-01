//! The absence this milestone's whole gate rests on.
//!
//! # Why an absence is worth a test
//!
//! `docs/M8.md` Decision 7 grades six headless modules as behaving identically on both targets,
//! and its evidence class is **proven** rather than *measured* for one reason:
//!
//! > *"**Structurally.** `grep -rn "cfg(target" crates/frontend/src/` returns nothing, so the
//! > host tests and the browser run the same code. There is no run in which it can be false."*
//!
//! That is the strongest kind of claim this project makes — `docs/STATUS.md` prefers it
//! wherever it is available, on the grounds that *"a property of every build beats an
//! observation about one"* — and it is the **only** thing that makes a host-run test say
//! anything at all about a `wasm32` build. Every other frontend gate runs on this machine's
//! architecture. Their relevance to a browser is entirely downstream of this sentence.
//!
//! A claim that load-bearing, resting on a `grep` in a document, is a claim that lasts until
//! the first `#[cfg]` somebody adds for a good local reason. So it is asserted here, where a
//! change that falsifies it turns something red in the same run that introduced it.
//!
//! # Scope, and why it is narrower than the document's grep
//!
//! `docs/M8.md`'s command sweeps `crates/frontend/src/`, `crates/spectrum/src/` and
//! `crates/z80/src/`. This test sweeps **only this crate**. Asserting a property of a sibling
//! crate's source from here would make another crate's ordinary refactor turn this crate's
//! suite red, which is a coupling a test should not create — and it would be asserting it in
//! the wrong place, since the two machine crates are already `#![no_std]`-shaped and are proven
//! to build for `wasm32` by the build itself.
//!
//! # What it cannot see
//!
//! Everything that is *not* `#[cfg]`. A `#[cfg]`-free crate can still behave differently on two
//! targets — through a dependency, through pointer width, through `f32` rounding, through
//! whatever a browser's WebGL does with a texture upload. This grades that **this crate does
//! not branch on the target**, which is a real and narrow property, and not that the two
//! targets agree.

use std::path::{Path, PathBuf};

/// The needle. Catches `cfg(target_arch`, `cfg(target_os`, `cfg(target_family` and every other
/// spelling in one string, which is exactly what the document's own `grep` matches.
const TARGET_CONDITIONAL: &str = "cfg(target";

/// The fewest `.rs` files this crate can plausibly have.
///
/// A floor rather than an equality, for the reason `docs/STATUS.md` gives about the codegen
/// probe's own floor: it only has to separate *present* from *absent*, and a walk that found
/// nothing reads as **0**. `src/` held ten files when this was written; the floor is set well
/// below that so an ordinary refactor does not have to touch this number, and well above zero
/// so a broken walk cannot pass.
const FEWEST_SOURCES: usize = 6;

/// Every `.rs` file under `directory`, recursively.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("a readable directory entry").path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|suffix| suffix == "rs") {
            found.push(path);
        }
    }
    found
}

/// This crate's `src/`.
fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn nothing_in_this_crate_branches_on_the_target() {
    let files = sources(&source_root());

    // The "I was not looking at the thing" assertion, and it is not optional here: this test's
    // verdict is an *absence*, so a walk that visited nothing produces a confident green for a
    // question it never asked. `docs/STATUS.md`: "a count of zero and an absence of the subject
    // are the same observation."
    assert!(
        files.len() >= FEWEST_SOURCES,
        "the walk found {} source files under {}; it is not looking at the crate",
        files.len(),
        source_root().display(),
    );

    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        for (number, line) in text.lines().enumerate() {
            // Comments are skipped, and the reason is that this gate caught its own author.
            // Adding a row to `src/lib.rs`'s coverage table that *described* this test — and
            // therefore quoted the string it looks for — turned it red. A commented-out `cfg`
            // is not code, and a documentation table that cannot name the thing it documents
            // is a gate that makes the codebase worse.
            //
            // It costs nothing here: an attribute is not a comment, so
            // `#[cfg(target_os = "macos")]` on a real line is still caught. That was
            // re-confirmed by mutation after this filter was added, rather than assumed —
            // a filter is exactly the kind of change that can quietly make a gate unable to
            // fail, which is `docs/STATUS.md`'s standing warning about greens that cannot go
            // red.
            if line.trim_start().starts_with("//") {
                continue;
            }
            assert!(
                !line.contains(TARGET_CONDITIONAL),
                "{}:{} branches on the target: {}\n\
                 The browser build's whole claim to the host tests is that there is no such \
                 line. If this one is right, `docs/M8.md` Decision 7's T1 tier has to be \
                 re-argued, not just this assertion relaxed.",
                path.display(),
                number + 1,
                line.trim(),
            );
        }
    }
}

#[test]
fn the_target_conditional_code_is_where_it_was_moved_to() {
    // The other half of the claim, and the half that makes the first half mean something. This
    // crate has no `#[cfg]` *because* the two non-portable calls were moved into `crates/page`,
    // not because the browser needs no different behaviour. Without this assertion, deleting
    // `crates/page` entirely would leave the test above greener than ever.
    //
    // Reading a sibling crate is exactly what the test above declines to do, and the difference
    // is the direction: asserting that another crate does **not** contain something couples this
    // suite to that crate's every edit, and asserting that it **does** contain the thing this
    // crate delegated to it is a statement about this crate's own design.
    let glue = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../page/src/lib.rs")
        .canonicalize()
        .expect("crates/page/src/lib.rs, which crates/frontend depends on");
    let text = std::fs::read_to_string(&glue).expect("a readable crates/page/src/lib.rs");

    assert!(
        text.contains(TARGET_CONDITIONAL),
        "{} has no target-conditional code, so this crate's freedom from it means nothing",
        glue.display(),
    );
    assert!(
        text.contains("unsafe"),
        "{} was supposed to be where the `unsafe` went",
        glue.display(),
    );
}

#[test]
fn this_crate_still_forbids_unsafe() {
    // `docs/M8.md` Decision 4 spent a page on keeping `unsafe_code = "forbid"` here, and the
    // whole argument for a separate crate collapses the moment somebody relaxes this line to
    // `deny` to make one thing work. A manifest is not usually a thing to assert about; this
    // one is, because the alternative to `forbid` is not a compile error but a review nobody
    // is scheduled to perform.
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains(r#"unsafe_code = "forbid""#),
        "crates/frontend no longer forbids unsafe; `docs/M8.md` Decision 4 says what that costs",
    );
}
