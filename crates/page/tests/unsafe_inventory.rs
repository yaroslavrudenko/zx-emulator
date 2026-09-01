//! The `unsafe` surface, counted — because *"small enough to read in one sitting"* is a claim.
//!
//! # This gate exists because clippy's cannot run here
//!
//! `Cargo.toml` turns on `clippy::undocumented_unsafe_blocks` and
//! `clippy::multiple_unsafe_ops_per_block`, both deny. Both are real and both are **blind on a
//! desktop host**: every `unsafe` block in `src/lib.rs` sits behind
//! `#[cfg(target_arch = "wasm32")]`, and a lint only fires on code the current target
//! compiles. So `cargo clippy` on macOS or Linux reports this crate clean whatever those
//! blocks contain, and the person reading the report has no way to tell that from a crate that
//! was actually checked.
//!
//! That is `docs/STATUS.md`'s recurring shape — *a gate that grades less than it appears to* —
//! and the remedy it prescribes is the one used here: **make the tool state what it covered,
//! and assert on that.** This file reads `src/lib.rs` as *text*, through `include_str!`, so
//! `cfg` is invisible to it and it sees every block on every target under an ordinary
//! `cargo test`.
//!
//! # What it asserts, and why a count is the right instrument
//!
//! `docs/M8.md` Decision 4's whole argument for a separate crate is that the exception is
//! *confined to a surface a person can read in one sitting*. That is a property of a **size**,
//! and a size that nothing checks is a size that grows. The numbers below are therefore
//! literals with a reason attached rather than a floor: if the next `unsafe` block is right,
//! this file is where the case for it gets written down, beside the constant it moves.
//!
//! *This sentence used to name **"a fourth `unsafe` block"** as the next one. It was written
//! when the surface was three blocks, it outlived that, and it did so inside the very test whose
//! job is to notice that number moving — an ordinal is just another copy of the count, and it
//! drifts exactly as any other copy does. The constants below are where the figures live; prose
//! that needs one names the constant instead of repeating its value.*
//!
//! **The general test — whether a figure or a list in prose is a second copy at all — is stated
//! once, and not here.** `README.md`'s *Engineering rules* owns it, as the third of three sibling
//! rules: *"does the surrounding sentence stay true when the list or the figure goes wrong?"*
//! Writing it out again in this file would be the defect it names, so what stays here is this
//! crate's own working of it. It deleted *"the three imports"* from `src/lib.rs`, which had gone
//! stale, in favour of *"every import declared below"*, which cannot. It kept the enumeration of
//! `unsafe` syntax forms in the same file, because that crate's *"sweep is over syntax"* is
//! unreadable without them. And it kept `Cargo.toml`'s *"3 runtime crates"*, a dated measurement
//! whose count and names are one fact.
//!
//! **What it cannot see:** whether the blocks are *correct*. A `// SAFETY:` comment is prose,
//! and prose asserting a guarantee is a hypothesis rather than a guarantee. This grades that
//! the comments exist and that the blocks have not multiplied, which is drift, not soundness.

/// The crate's whole source, at compile time.
///
/// `include_str!` rather than `std::fs::read_to_string`: the path is resolved relative to this
/// file at compile time, so the test cannot be made to pass — or fail — by the directory
/// `cargo test` happens to run from.
const SOURCE: &str = include_str!("../src/lib.rs");

/// One `unsafe { … }` block per FFI call, across both seams.
///
/// It grew when the browser gained a device, and growing is what this constant exists to make
/// visible: `docs/M8.md` Decision 4's whole case for a separate crate is that the exception
/// stays *readable in one sitting*, and a size nothing checks is a size that climbs.
const EXPECTED_BLOCKS: usize = 5;

/// How many of [`EXPECTED_BLOCKS`] are on the audio seam; the rest are the page's.
///
/// The division was prose — *"three for the argument and download seam, two for audio"* — above
/// a constant that only knew the total, so a block retargeted from one seam to the other left
/// the total right, the sentence wrong, and the suite green. It is a constant now, and the
/// page's share is its complement rather than a third figure that could disagree with the other
/// two. Attribution is by callee: a block reaching a `zx_audio_*` import is audio.
const EXPECTED_AUDIO_BLOCKS: usize = 2;

/// One `unsafe extern "C"` block per seam: the page, and the audio device.
///
/// Kept apart rather than merged so that each carries its own `#[link(wasm_import_module)]` and
/// its own explanation, and so that deleting audio would delete a block rather than edit one.
const EXPECTED_EXTERN_BLOCKS: usize = 2;

/// `#[unsafe(no_mangle)]`, on the plugin version export.
///
/// Counted separately from the blocks because it is a different obligation: a block promises
/// something about memory, and `no_mangle` promises something about the **linker** — that no
/// other symbol in the artefact carries the name. An inventory that folded them together would
/// let one be swapped for the other without the total moving.
const EXPECTED_UNSAFE_ATTRIBUTES: usize = 1;

/// The lines of `SOURCE` that are code rather than commentary.
///
/// Doc comments in this crate discuss `unsafe` constantly — that is most of what they are for
/// — so a naive substring count over the whole file would grade the prose. Every line whose
/// first non-blank characters are `//` is dropped, which covers `//`, `///` and `//!` in one
/// rule.
fn code_lines() -> Vec<&'static str> {
    SOURCE
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect()
}

#[test]
fn the_source_under_inventory_is_the_crate_and_not_an_empty_string() {
    // `docs/STATUS.md`: "every gate needs one assertion whose failure means 'I was not looking
    // at the thing'." A count of zero and an absence of the subject are the same observation,
    // and every assertion below is a count — so without this one, an `include_str!` pointed at
    // the wrong file, or a comment filter that ate the whole source, would read as a crate
    // with no `unsafe` in it and pass three tests.
    let code = code_lines();
    assert!(
        code.len() > 50,
        "only {} code lines survived the comment filter; it is eating the subject",
        code.len(),
    );
    for anchor in [
        "pub const fn handoff",
        "fn zx_offer_download",
        "fn zx_query_copy",
    ] {
        assert!(
            SOURCE.contains(anchor),
            "{anchor} is missing; this is not the file the inventory is about",
        );
    }
}

#[test]
fn the_unsafe_surface_is_the_size_it_is_documented_to_be() {
    let code = code_lines();

    let blocks = code.iter().filter(|line| line.contains("unsafe {")).count();
    let audio = code
        .iter()
        .filter(|line| line.contains("unsafe {") && line.contains("zx_audio_"))
        .count();
    let externs = code
        .iter()
        .filter(|line| line.contains("unsafe extern"))
        .count();
    let attributes = code
        .iter()
        .filter(|line| line.contains("#[unsafe("))
        .count();

    assert_eq!(
        blocks, EXPECTED_BLOCKS,
        "the crate has {blocks} `unsafe` blocks and is documented as having {EXPECTED_BLOCKS}",
    );
    assert_eq!(
        audio, EXPECTED_AUDIO_BLOCKS,
        "{audio} of them are on the audio seam and the inventory says {EXPECTED_AUDIO_BLOCKS}; \
         the total above is still right, so a block has moved between the seams",
    );
    assert_eq!(externs, EXPECTED_EXTERN_BLOCKS);
    assert_eq!(attributes, EXPECTED_UNSAFE_ATTRIBUTES);
}

#[test]
fn every_unsafe_block_is_preceded_by_a_safety_comment() {
    // What `clippy::undocumented_unsafe_blocks` would do, if it could see these blocks on this
    // target. Kept as well as the lint rather than instead of it: the lint is the better
    // instrument where it applies, because it understands the syntax rather than the text.
    let lines: Vec<&str> = SOURCE.lines().map(str::trim).collect();

    let mut checked = 0_usize;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("//") || !line.contains("unsafe {") {
            continue;
        }
        checked += 1;
        let documented = lines[..index]
            .iter()
            .rev()
            .take_while(|above| above.starts_with("//") || above.is_empty())
            .any(|above| above.starts_with("// SAFETY:"));
        assert!(
            documented,
            "the `unsafe` block on line {} has no `// SAFETY:` comment above it",
            index + 1,
        );
    }
    assert_eq!(
        checked, EXPECTED_BLOCKS,
        "the scan found {checked} blocks to check and the inventory says {EXPECTED_BLOCKS}",
    );
}
