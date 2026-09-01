//! `build.rs`'s `LOADABLE` and [`media::EXTENSIONS`] are the same list, and nothing but this
//! file can say so.
//!
//! # The duplication is forced, and it was unguarded for a fortnight while claiming not to be
//!
//! A build script runs *before* the crate it builds. It cannot call [`media::kind_of`], it cannot
//! link this library, and there is no third place to put the list that both could reach without
//! inventing a fourth crate for one array. So `build.rs` carries a copy, and `build.rs` said —
//! in the doc comment directly above that copy — that *"the copy is kept honest by
//! `tests/bundled_extensions.rs`, which reads **this file as text** … That is the shape this
//! project prefers when duplication is genuinely forced: not a promise, a gate."*
//!
//! **There was no such file.** `rg bundled_extensions` over the whole repository returned that one
//! line, citing itself. The sentence was the promise it was contrasting itself against, and it
//! cost exactly what a false gate costs: `tzx` was added to `LOADABLE` on 2026-09-01 and the
//! paragraph beneath it went on predicting `.tzx`'s arrival, because **a reader who is told a
//! thing is checked stops checking it**. An ungated file is a known risk; a file that says it is
//! gated is an unknown one.
//!
//! # What this grades, and what it cannot
//!
//! It reads `build.rs` as **text**, which is the weakest instrument in this repository and is used
//! here only because it is the sole instrument available: the alternative is not a better test,
//! it is no test. Everything on the other side of the comparison is the real linked value.
//!
//! It cannot see the build script *run* — `gate-bundled.sh` does that, by pointing
//! `ZX_BUNDLE_MEDIA` at a format the emulator cannot load and requiring the build to refuse. The
//! two are complements: that gate proves the check fires, this one proves it fires on the right
//! set.

use std::collections::BTreeSet;

use frontend::media::{self, Kind};

/// The literal `build.rs` text, read from the package this test belongs to.
fn build_script() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// The string literals of the `&[...]` that `declaration` introduces.
///
/// Anchored on `= &[` rather than on the first `[`, because the type is written `&[&str]` and a
/// bracket-counting parser would stop inside it. It ends at `];`, which is the one sequence a
/// string in this list cannot contain.
///
/// A parser rather than a regular expression, and a deliberately shallow one: its job is to fail
/// loudly on anything it does not understand, not to survive a reformatting nobody has proposed.
fn string_literals(source: &str, declaration: &str) -> BTreeSet<String> {
    let start = source
        .find(declaration)
        .unwrap_or_else(|| panic!("build.rs no longer declares `{declaration}`"));
    let rest = &source[start..];
    let open = rest
        .find("= &[")
        .unwrap_or_else(|| panic!("`{declaration}` is no longer written as `= &[...]`"))
        + "= &[".len();
    let close = rest[open..]
        .find("];")
        .unwrap_or_else(|| panic!("`{declaration}`'s array is not closed with `];`"))
        + open;

    // Every second field of a split on `"` is the inside of a literal: `a "b" c "d"` splits to
    // `a `, `b`, ` c `, `d`, so skipping the first and taking every other one is exactly the
    // quoted text.
    rest[open..close]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// What `media` answers to, as bare extensions.
fn recognised() -> BTreeSet<String> {
    media::EXTENSIONS
        .iter()
        .map(|&(extension, _)| extension.to_owned())
        .collect()
}

#[test]
fn the_build_scripts_list_is_exactly_the_one_the_emulator_loads() {
    let embeddable = string_literals(&build_script(), "const LOADABLE");
    let loadable = recognised();

    // Both directions, and the asymmetry is the whole reason. An entry **missing** from `build.rs`
    // refuses a payload the emulator could have run — annoying, and visible the moment somebody
    // tries. An **extra** entry is the dangerous one: it embeds bytes nothing can parse, and the
    // failure surfaces as a standalone binary that starts, shows nothing, and looks like a broken
    // emulator on a machine the person who built it does not have. `build.rs`'s own header calls
    // that "the outcome this check exists to prevent".
    assert_eq!(
        embeddable,
        loadable,
        "build.rs's LOADABLE and media::EXTENSIONS have drifted apart\n  \
         build.rs embeds:   {embeddable:?}\n  \
         media can load:    {loadable:?}\n  \
         only in build.rs:  {:?}\n  \
         only in media:     {:?}",
        embeddable.difference(&loadable).collect::<Vec<_>>(),
        loadable.difference(&embeddable).collect::<Vec<_>>(),
    );

    // The vacuous pass this repository keeps writing counters against: an anchor that matched an
    // empty array, or a `media::EXTENSIONS` that lost its rows, would make the two sets equal by
    // both being empty.
    assert!(
        embeddable.len() >= 5,
        "only {} extensions on either side — one of the two lists has collapsed",
        embeddable.len(),
    );
}

#[test]
fn every_extension_the_build_script_embeds_is_one_kind_of_recognises() {
    // `build.rs`'s original prescription, kept because set equality is not quite the same claim:
    // two lists can agree on a typo. This one asks the *function* that will be handed the payload
    // at run time, which is the thing that actually has to say yes.
    let mut checked = 0;
    for extension in string_literals(&build_script(), "const LOADABLE") {
        let kind = media::kind_of(&format!("payload.{extension}"));
        assert!(
            kind.is_some(),
            "build.rs would embed a .{extension} and media::kind_of does not know it, so the \
             binary would start and be unable to read its own payload",
        );
        // And nothing here is a format we recognise by name but cannot parse: `media::unsupported`
        // is the table that turns such a thing into a legible refusal, and a build embedding one
        // would be shipping that refusal instead of a game.
        assert!(
            media::unsupported(&format!("payload.{extension}")).is_none(),
            "build.rs would embed a .{extension}, which media::NOT_YET says cannot be loaded",
        );
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} extensions were checked");
}

#[test]
fn a_rom_is_embeddable_and_is_not_something_a_running_machine_accepts() {
    // The one row where the two lists mean different things, asserted rather than left implicit.
    // `build.rs` embeds a `.rom` because a standalone needs a machine to be built *from*;
    // `media::insert` refuses one, because a machine that is already running cannot be rebuilt.
    // Every user-facing sentence in this crate has to make that distinction, and it is worth one
    // assertion that the distinction is real rather than a habit.
    assert!(
        string_literals(&build_script(), "const LOADABLE").contains("rom"),
        "a standalone with no ROM has no machine to build",
    );
    assert_eq!(media::kind_of("a.rom"), Some(Kind::Rom));
}

#[test]
fn the_check_is_capable_of_failing() {
    // A positive control, for the same reason `tests/on_screen_strings.rs` carries one: the
    // comparison above is otherwise a function that has only ever been observed to say yes, and
    // the defect this file exists for — a gate that was cited and never written — is precisely a
    // check nobody had watched fail.
    //
    // It is run against a **synthetic declaration** rather than against `build.rs` itself, which
    // makes it hermetic in both directions: it edits no real file, and it does not quietly start
    // passing for a new reason when somebody reorders the real array or renames an entry.
    //
    // Two claims, and the first is the one a control usually forgets. A parser that returned the
    // empty set for everything would make the comparison in the test above vacuously true, so this
    // begins by proving the parser can say **yes** — that it reads a correct list correctly — and
    // only then that it says **no**.
    let complete = declaration(&recognised());
    assert_eq!(
        string_literals(&complete, "const LOADABLE"),
        recognised(),
        "the parser cannot read a list it should accept, so a green above means nothing",
    );

    // And now with one entry gone: the exact drift that happened, which is a format the emulator
    // learned to load and the build script never heard about.
    for absent in recognised() {
        let mut short = recognised();
        short.remove(&absent);
        assert_ne!(
            string_literals(&declaration(&short), "const LOADABLE"),
            recognised(),
            "a LOADABLE missing .{absent} still matched media::EXTENSIONS, so this gate would \
             not have caught the drift it was written for",
        );
    }
}

/// A `LOADABLE` declaration carrying exactly `extensions`, for the control above to read back.
///
/// Written in the same shape as the real one — `= &[` … `];` — because that shape is what
/// [`string_literals`] anchors on, and a control that fed the parser something easier would be
/// grading a parser nobody runs.
fn declaration(extensions: &BTreeSet<String>) -> String {
    let body = extensions
        .iter()
        .map(|extension| format!("{extension:?}, "))
        .collect::<String>();
    format!("const LOADABLE: &[&str] = &[{body}];")
}
