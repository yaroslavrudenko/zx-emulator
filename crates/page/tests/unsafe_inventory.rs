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

/// The lines of `SOURCE` that are code rather than commentary or blank.
///
/// Doc comments in this crate discuss `unsafe` constantly — that is most of what they are for
/// — so a naive substring count over the whole file would grade the prose. Every line whose
/// first non-blank characters are `//` is dropped, which covers `//`, `///` and `//!` in one
/// rule.
///
/// *Blank lines used to survive this filter*, which mattered in exactly one place and mattered
/// there quietly: the anti-vacuity floor below counts what this returns, so a fifth of the
/// figure it was checking was whitespace. A floor met partly by blank lines is a floor that has
/// already been lowered without anyone deciding to.
fn code_lines() -> Vec<&'static str> {
    SOURCE
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .collect()
}

/// `SOURCE` with every comment line blanked, so byte offsets still map to the original lines.
///
/// The block scan below needs to match braces across lines, which a `Vec` of surviving lines
/// cannot do — and it must not match a brace inside the prose, which is most of this file.
/// Blanking rather than deleting keeps [`Block::line`] honest without a second index.
fn code_only() -> String {
    SOURCE
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<&str>>()
        .join("\n")
}

/// One `unsafe { … }` block, found by matching its braces.
struct Block {
    /// The 1-based line in `SOURCE` that the `unsafe` keyword is on.
    line: usize,
    /// Everything between the braces, comments blanked.
    body: String,
}

/// Every `unsafe { … }` block in `SOURCE`.
///
/// # Why a brace-matched scan and not a line filter
///
/// This inventory counted **lines containing `unsafe {`**, and that turned out to be a different
/// quantity from the one its own constants describe, in three ways at once:
///
/// 1. [`EXPECTED_BLOCKS`] documents *"one `unsafe { … }` block per FFI call"*, and a line count
///    cannot see inside a block — so a block containing **two** FFI calls counted as one and the
///    stated invariant was asserted by nothing. The only instrument that would have caught it is
///    `clippy::multiple_unsafe_ops_per_block`, which this file's own header argues is blind on a
///    desktop host. The gate rested on the lint it was written to replace.
/// 2. A block whose brace lands on the *next* line — which is what `rustfmt` does the moment a
///    call gets long enough — was invisible to every test here, including the `SAFETY:` one.
/// 3. Attribution to a seam required the callee's name on the **same physical line** as
///    `unsafe {`. `src/lib.rs`'s `zx_offer_download` call is exactly 100 characters, which is
///    `rustfmt`'s `max_width`: one more argument character and it wraps, `blocks` stays 5,
///    `audio` drops to 0, and the assertion fires with the message *"the total above is still
///    right, so a block has moved between the seams"* — a confident, specific and completely
///    false diagnosis. A gate that lies about **why** it failed is worse than one that does not
///    fire, because it sends the next person to the wrong file.
///
/// Matching the braces answers all three. The body is then a piece of text a test can ask
/// questions of, rather than a line a test can only pattern-match.
///
/// # What it does not parse
///
/// Braces inside string literals. There are none inside an `unsafe` block here, and a runaway
/// match would change the block count, which is asserted — so the limitation fails loudly rather
/// than silently. Trailing `// …` comments after code are not blanked either, and
/// [`no_trailing_comment_hides_the_word_this_gate_scans_for`] is the assertion that says so.
fn unsafe_blocks() -> Vec<Block> {
    const KEYWORD: &str = "unsafe";
    let code = code_only();
    let mut blocks = Vec::new();
    let mut cursor = 0_usize;
    while let Some(offset) = code[cursor..].find(KEYWORD) {
        let start = cursor + offset;
        cursor = start + KEYWORD.len();
        if !is_whole_word(&code, start, KEYWORD.len()) {
            continue;
        }
        // `unsafe extern`, `unsafe fn`, `unsafe impl` and `#[unsafe(` are the other syntax forms,
        // counted separately and on purpose — only a block has a brace next.
        let Some(open) = next_visible(&code, cursor).filter(|&at| code.as_bytes()[at] == b'{')
        else {
            continue;
        };
        let Some(close) = closing_brace(&code, open) else {
            continue;
        };
        blocks.push(Block {
            line: code[..start].matches('\n').count() + 1,
            body: code[open + 1..close].to_owned(),
        });
        cursor = close;
    }
    blocks
}

/// Whether the `length` bytes at `at` are a token rather than part of a longer identifier.
fn is_whole_word(text: &str, at: usize, length: usize) -> bool {
    let boundary =
        |byte: Option<&u8>| !byte.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    boundary(
        at.checked_sub(1)
            .and_then(|before| text.as_bytes().get(before)),
    ) && boundary(text.as_bytes().get(at + length))
}

/// The offset of the next non-whitespace byte at or after `from`.
fn next_visible(text: &str, from: usize) -> Option<usize> {
    text.as_bytes()[from..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|offset| from + offset)
}

/// The offset of the `}` that closes the `{` at `open`.
fn closing_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, byte) in text.as_bytes().iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// The `zx_*` functions `SOURCE` declares in its `unsafe extern` blocks.
///
/// Derived rather than listed. A hardcoded roster here would be a second copy of the crate's
/// import set — the defect this whole file exists to catch, committed by the catcher.
fn declared_imports() -> Vec<&'static str> {
    code_lines()
        .iter()
        .filter_map(|line| line.strip_prefix("fn "))
        .filter(|rest| rest.starts_with("zx_"))
        .filter_map(|rest| rest.split_once('(').map(|(name, _)| name))
        .collect()
}

/// How many calls to a declared import `body` makes.
fn ffi_calls(body: &str, imports: &[&str]) -> usize {
    imports
        .iter()
        .map(|name| body.matches(&format!("{name}(")).count())
        .sum()
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
    let blocks = unsafe_blocks();
    let audio = blocks
        .iter()
        .filter(|block| block.body.contains("zx_audio_"))
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
        blocks.len(),
        EXPECTED_BLOCKS,
        "the crate has {} `unsafe` blocks and is documented as having {EXPECTED_BLOCKS}",
        blocks.len(),
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
fn every_unsafe_block_contains_exactly_one_ffi_call() {
    // [`EXPECTED_BLOCKS`] says *"one `unsafe { … }` block per FFI call"*, and until this test
    // existed nothing said whether that was true — the count could not see inside a block, so
    // two calls in one block read as one block and the sentence stayed green. This is what
    // `clippy::multiple_unsafe_ops_per_block` would assert if it could see these blocks on a
    // desktop host, which this file's own header explains it cannot.
    let imports = declared_imports();
    assert_eq!(
        imports.len(),
        EXPECTED_BLOCKS,
        "the crate declares {} imports and {EXPECTED_BLOCKS} blocks; one block per FFI call is \
         the invariant, so these two figures move together or the invariant is what changed",
        imports.len(),
    );
    for block in unsafe_blocks() {
        let calls = ffi_calls(&block.body, &imports);
        assert_eq!(
            calls, 1,
            "the `unsafe` block on line {} makes {calls} FFI calls and the invariant is one — \
             a block with two is one `// SAFETY:` comment discharging two obligations, which is \
             the shape `clippy::multiple_unsafe_ops_per_block` exists to refuse",
            block.line,
        );
    }
}

#[test]
fn every_unsafe_block_is_preceded_by_a_safety_comment() {
    // What `clippy::undocumented_unsafe_blocks` would do, if it could see these blocks on this
    // target. Kept as well as the lint rather than instead of it: the lint is the better
    // instrument where it applies, because it understands the syntax rather than the text.
    let lines: Vec<&str> = SOURCE.lines().map(str::trim).collect();
    let blocks = unsafe_blocks();

    for block in &blocks {
        let above = block.line - 1;
        let documented = lines[..above]
            .iter()
            .rev()
            .take_while(|line| line.starts_with("//") || line.is_empty())
            .any(|line| line.starts_with("// SAFETY:"));
        assert!(
            documented,
            "the `unsafe` block on line {} has no `// SAFETY:` comment above it",
            block.line,
        );
    }
    assert_eq!(
        blocks.len(),
        EXPECTED_BLOCKS,
        "the scan found {} blocks to check and the inventory says {EXPECTED_BLOCKS}",
        blocks.len(),
    );
}

#[test]
fn no_trailing_comment_hides_the_word_this_gate_scans_for() {
    // [`code_only`] blanks whole comment lines and leaves a trailing `// …` after code in place,
    // so a trailing comment containing the word would be scanned as if it were code. There are
    // none. Asserted rather than assumed, because the assumption is invisible at the point where
    // it matters and this is the one line that makes it checkable.
    for (index, line) in SOURCE.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some((_, comment)) = trimmed.split_once("//") else {
            continue;
        };
        assert!(
            !comment.contains("unsafe"),
            "line {} carries code and a trailing comment mentioning `unsafe`, which the block \
             scan would read as code: {line}",
            index + 1,
        );
    }
}
