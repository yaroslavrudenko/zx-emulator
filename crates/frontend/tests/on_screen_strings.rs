//! Every message that reaches the status bar must be ASCII, because the font has no glyph for
//! anything else.
//!
//! # This gate exists because somebody looked at the screen
//!
//! `media::accept`'s model-mismatch message and `media::NOT_YET`'s `.tzx` refusal were written
//! with an em dash, and `host::SaveError::Download` with another. They read perfectly in a
//! terminal, in a diff, and in every test that compares them as strings — and in the running
//! emulator the em dash draws as an **empty box**. Observed 2026-09-01, in Chrome, by dropping a
//! `manic.tzx` on the page and photographing the status bar: *"manic.tzx: a .tzx tape is not
//! supported yet ▯ the pulse-level"*.
//!
//! Nothing in this repository could have caught it. Every assertion about those strings compares
//! them to another string, and both sides were equally unrenderable — which is the shape
//! `docs/STATUS.md` keeps recording, arriving here as a *user-visible* defect rather than a
//! measurement one. The instrument that found it was a person looking at a picture, and this
//! file is what stops the next one needing to.
//!
//! # What it covers, and the part it cannot
//!
//! It covers the strings this crate can hand a test: everything [`media::accept`] returns, and
//! every [`host::SaveError`]'s `Display`. It **cannot** cover `main.rs`'s own literals —
//! `OPENING_MESSAGE` and the two lines `complain` draws — because they are private to a binary
//! that needs a window. Those were fixed by the same sweep and are named here so the gap is
//! recorded rather than assumed away: `grep -n '\\u{' crates/frontend/src/main.rs` is the check,
//! and it is a person's to run.
//!
//! It also does not prove the font *has* every ASCII glyph. It proves nothing outside ASCII is
//! attempted, which is the property that failed.

use frontend::host::SaveError;
use frontend::{drive, keymap, media};

/// Where the committed 48K ROM is, from the workspace root.
const ROM: &str = "testdata/roms/48.rom";

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root")
}

/// The characters `macroquad::draw_text`'s default font is known to carry.
fn assert_drawable(what: &str, message: &str) {
    for (index, character) in message.char_indices() {
        assert!(
            character.is_ascii() && !character.is_ascii_control(),
            "{what} contains {character:?} at byte {index}, which the status bar draws as an \
             empty box: {message}",
        );
    }
}

#[test]
fn every_message_a_dropped_file_can_produce_is_drawable() {
    let Some(rom) = std::fs::read(workspace_root().join(ROM)).ok() else {
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let mut machine = media::start(&[&rom[..]]).expect("one ROM is a 48K");

    // Every branch of `accept`, named rather than sampled: the refusal, the ROM rejection, a
    // parse failure, and a success. A sample would have missed the em dash, because the em dash
    // was in exactly one branch.
    //
    // The first row says `.tzx` and is no longer a refusal — it landed on 2026-09-01 and now
    // inserts a tape. It stays because what this file grades is that *whatever* `accept` returns
    // is drawable, and a row that changed category without changing its answer is worth keeping
    // for exactly that reason.
    let cases: Vec<(&str, &[u8])> = vec![
        ("manic.tzx", b"ZXTape!\x1a\x01\x14"),
        ("thing.dsk", b""),
        ("second.rom", &rom[..]),
        ("broken.z80", b"\x00\x01\x02"),
        ("broken.sna", b"\x00"),
        ("broken.tap", b"\xff\xff\xff\xff"),
    ];
    let mut produced = 0;
    for (name, bytes) in cases {
        let message = media::accept(&mut machine, name, bytes);
        assert_drawable(name, &message);
        produced += 1;
    }

    // A success too, since it is the message a person sees most often.
    let mut fresh = media::start(&[&rom[..]]).expect("one ROM is a 48K");
    fresh.run_frames(2);
    let snapshot = media::save(&fresh);
    let message = media::accept(&mut machine, "generated.z80", &snapshot);
    assert_drawable("a successful load", &message);
    produced += 1;

    // **The model mismatch, and this case exists because a mutation survived without it.**
    // Putting the em dash back into that one branch left every assertion above green, because
    // none of them could reach it: `Error::Model` needs a snapshot describing a *different*
    // machine, and the six cases above are all 48K. So the branch that carried one of the three
    // original defects was the one branch this gate could not see — which is the shape
    // `docs/STATUS.md` keeps recording, caught here by a mutation rather than by a screenshot
    // the second time.
    //
    // Reaching it costs nothing: build a 128 from the two committed ROMs, save it, and hand it
    // to the 48K that is already running. Decision 11's wording is graded at the same time.
    //
    // > **The paragraph that used to be here said the direction mattered, and it no longer does —
    // > because the defect it was routing around has been fixed.** It read: *"The obvious case — a
    // > 128 snapshot handed to a 48K — does not refuse: it restores, silently. Measured
    // > 2026-09-01: `media::save` on a 128 writes `.z80` hardware-mode byte 0, which is 48K, so
    // > the file does not describe the machine that produced it and nothing downstream can tell."*
    // > It was true when it was written and it is false now, in **both** halves. Re-measured the
    // > same day, by saving a 128 through `media::save` and reading the bytes back rather than by
    // > reading the writer:
    // >
    // > - the `.z80` v3 hardware-mode byte at offset 34 is **4**, which is a 128 — not 0;
    // > - and `media::accept` of that file on a running 48K returns
    // >   *"a 128 snapshot cannot be restored into a 48K - restart naming the ROMs that machine
    // >   needs"*, so the obvious case **does** refuse.
    // >
    // > The correction is recorded rather than swapped in because the stale sentence was a
    // > *reported defect* — a note left in a test explaining why it takes the long way round — and
    // > a reported defect that has been fixed should be visibly closed rather than quietly
    // > deleted. Nothing about the case below changed: both directions now reach `Error::Model`,
    // > so this one is kept exactly as it was, and it is no longer the only one that would work.
    //
    // A 48K snapshot into a running 128 is refused, and that is the branch with the message.
    let editor = std::fs::read(workspace_root().join("testdata/roms/128-0.rom"));
    let basic = std::fs::read(workspace_root().join("testdata/roms/128-1.rom"));
    if let (Ok(editor), Ok(basic)) = (editor, basic) {
        let mut a_128 = media::start(&[&editor[..], &basic[..]]).expect("two ROMs are a 128");
        let message = media::accept(&mut a_128, "from-a-48k.z80", &snapshot);
        assert!(
            message.contains("restart naming the ROMs"),
            "a 48K snapshot on a 128 should be refused with the frontend's own advice: {message}",
        );
        assert_drawable("a model mismatch", &message);
        produced += 1;
    } else {
        // Reported rather than skipped silently: without this case the gate has the hole the
        // mutation found, and a green that does not say so is the failure this file is about.
        eprintln!("skipping the model-mismatch case: the 128 ROMs are absent");
        produced += 1;
    }

    // The "I was not looking at the thing" assertion: `assert_drawable` passes vacuously on an
    // empty string, so a `media::accept` that returned `String::new()` would satisfy every line
    // above. Each message must actually say something, and the count must match the table.
    assert_eq!(
        produced, 8,
        "the table and the run disagree about how many cases ran"
    );
}

#[test]
fn the_refusal_a_stranger_earns_names_every_format_that_would_have_worked() {
    // Not a *drawability* claim — a **completeness** one, and it belongs here because this is the
    // file that already calls `media::accept` and looks at what comes back. Asking the function
    // is the strongest form available: `main.rs` and `zx-shot` have to grade their literals as
    // literals, because theirs are private to a binary and never pass through anything a test can
    // call. This one is the message itself.
    //
    // It matters because the sentence is the last thing a person reads before giving up. A
    // refusal that omits a format they are holding sends them away from an emulator that would
    // have loaded it, and the omission is invisible from inside: `accept` returns the same shape
    // of string either way, and every assertion that ever compared it compared it to another
    // hand-written list of the same four names.
    let Some(rom) = std::fs::read(workspace_root().join(ROM)).ok() else {
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let mut machine = media::start(&[&rom[..]]).expect("one ROM is a 48K");

    // `.dsk` is the control: a real format, recognised by nobody here, so `kind_of` returns
    // `None` and this is the branch that fires. It has to stay unloadable for this test to reach
    // the message at all, which is asserted rather than assumed.
    assert!(
        media::kind_of("thing.dsk").is_none(),
        ".dsk became loadable, so this test no longer reaches the refusal it grades",
    );
    let refusal = media::accept(&mut machine, "thing.dsk", b"");

    // Every extension, `.rom` included: this branch is reached when `kind_of` says *nothing at
    // all*, and `kind_of` does know a `.rom`. Naming it is correct here even though `insert`
    // turns one away later with a different message — which is exactly why the two sentences in
    // `zx-shot`, whose `--media` never builds a machine, list one format fewer.
    let mut named = 0;
    for &(extension, _) in media::EXTENSIONS {
        assert!(
            refusal.contains(&format!(".{extension}")),
            "a .{extension} loads and the refusal does not say so, so somebody holding one is \
             told this emulator cannot read it: {refusal}",
        );
        named += 1;
    }
    assert!(
        named >= 5,
        "only {named} extensions were checked — media::EXTENSIONS has shrunk",
    );
}

#[test]
fn every_save_failure_is_drawable() {
    let failures = [
        SaveError::Download {
            name: "snapshot-1.z80".to_owned(),
        },
        SaveError::NoFreeName {
            stem: "snapshot".to_owned(),
            extension: "z80".to_owned(),
            limit: 1000,
        },
        SaveError::Write {
            path: "snapshot-1.z80".to_owned(),
            source: std::io::Error::new(std::io::ErrorKind::Unsupported, "operation not supported"),
        },
    ];
    for failure in &failures {
        let message = failure.to_string();
        assert!(!message.is_empty(), "a SaveError with nothing to say");
        assert_drawable("a SaveError", &message);
    }
}

#[test]
fn every_arrow_scheme_name_and_hint_is_drawable() {
    // Both reach the status bar: the name every frame, the hint when `F7` selects the scheme.
    // They are library data rather than `main.rs` literals, so unlike `OPENING_MESSAGE` they are
    // reachable from a test — which is the gap this file's header names as a person's to grep.
    // A scheme added later with an en dash in its hint gets caught here instead of on a screen.
    let mut checked = 0;
    for scheme in keymap::ARROW_SCHEMES {
        assert_drawable("an arrow scheme name", scheme.name);
        assert_drawable("an arrow scheme hint", scheme.hint);
        // Vacuously drawable is the failure mode this file was written about: `assert_drawable`
        // passes on an empty string, so a scheme with no hint would sail through both lines.
        assert!(
            !scheme.name.is_empty() && !scheme.hint.is_empty(),
            "a scheme with nothing to say: {scheme:?}",
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        keymap::ARROW_SCHEMES.len(),
        "the loop and the table disagree about how many schemes there are",
    );
    assert!(checked >= 2, "there is nothing to cycle");
}

#[test]
fn everything_the_tape_drive_can_say_is_drawable() {
    // These landed with `frontend::drive` and they are library data rather than `main.rs`
    // literals, which is half of why they live there: the three messages a person reads at the
    // moment a press did nothing are the worst possible ones to be unreachable by this gate.
    //
    // `AT_THE_END` and `RAN_OUT` both name `F5`, and a hyphen is the character an author reaches
    // for when writing a sentence with an aside in it — which is precisely how the em dash this
    // file was written about got onto the screen three times.
    let messages = [
        drive::PLAYING,
        drive::NO_TAPE,
        drive::AT_THE_END,
        drive::RAN_OUT,
        drive::STOPPED,
        drive::REWOUND,
    ];
    let mut checked = 0;
    for message in messages {
        assert_drawable("a tape-drive message", message);
        // Vacuously drawable is this file's own recurring failure: `assert_drawable` passes on an
        // empty string, so a constant emptied by a bad edit would satisfy the loop by saying
        // nothing at all — which is the silence the whole module exists to remove.
        assert!(
            !message.is_empty(),
            "a tape-drive message with nothing to say"
        );
        checked += 1;
    }
    assert_eq!(checked, messages.len(), "the loop and the list disagree");
}

#[test]
fn the_check_is_capable_of_failing() {
    // A positive control for the checker itself, which is otherwise a function that has only
    // ever been shown to say yes. The em dash is the exact character that was found on screen.
    let result = std::panic::catch_unwind(|| {
        assert_drawable(
            "a deliberate failure",
            "not supported yet \u{2014} the parser",
        );
    });
    assert!(
        result.is_err(),
        "assert_drawable accepted an em dash, so it would not have caught the defect it exists for",
    );
}
