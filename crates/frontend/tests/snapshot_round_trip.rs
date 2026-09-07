//! A tape loaded, saved as a `.z80`, and restored: the same machine, not a picture of one.
//!
//! # What `zx-shot --snapshot` claims, and what would make the claim false
//!
//! A cassette is minutes of emulated time — `docs/RUNNING.md` puts *Manic Miner*'s load at some
//! nine thousand frames and *R-Type*'s at twenty-eight thousand — and `--snapshot` exists to spend
//! that once. The claim it makes is not *"here is a file"*; it is **that the file is the machine**,
//! so that opening it is the same act as having waited.
//!
//! There are two ways for that to be false, and only the second is interesting:
//!
//! - **The picture does not survive.** Caught by comparing the restored machine's frame to the
//!   frame the run itself rendered, byte for byte, through the buffer the window uploads.
//! - **Only the picture survives.** A `.z80` that carried the display file and nothing else would
//!   pass that comparison completely and be worthless — it would open on a photograph of *Manic
//!   Miner* and then sit there, because the Z80's registers, its interrupt mode, its stack and the
//!   game's own variables never arrived. **`the_restored_machine_goes_on_being_the_game` is the
//!   assertion that costs that mutation its pass**: both machines are run two hundred further
//!   frames and must still agree, which they can only do by executing the same code from the same
//!   state.
//!
//! The second test is why this file runs the game on rather than stopping at the snapshot, and the
//! control inside it is why *"both machines are frozen"* cannot satisfy it either.
//!
//! **Neither test subsumes the other, and that was measured rather than assumed.** Two mutations
//! were made to `crates/spectrum/src/snapshot/z80.rs`'s writer on 2026-09-04, each removing one
//! field from the file, and each reddened exactly one of the two tests:
//!
//! | mutation | `the_cover_is_the_cartridge_seen` | `the_restored_machine_goes_on_being_the_game` |
//! |---|---|---|
//! | `PC` written as zero | passes — the display file is intact | **fails**, 176,623 bytes apart |
//! | border written as zero | **fails**, 32,768 bytes apart | passes |
//!
//! The second row is the surprising one and it is the reason the first test cannot be deleted:
//! *Manic Miner* sets the border itself, every frame, so a restored machine with the wrong border
//! **repairs it within four seconds** and the two machines agree again. A snapshot that lost the
//! border would therefore leave a cartridge that opens on the wrong picture and then corrects
//! itself, which nothing but a comparison at the instant of restoration can see.
//!
//! # Why the load is real, and what that costs
//!
//! Nothing here injects a game into memory. The ROM types `LOAD ""` through
//! [`frontend::keymap::apply`] — the window's own keyboard path, with the same script every tape
//! command in `docs/images/README.md` uses — PLAY is pressed on the drive, and the loader counts
//! edges off the pulse train for the whole cassette. That is roughly **twelve thousand frames**,
//! which is a few seconds in a release build and around half a minute in the debug profile
//! `cargo test` uses. It is the price of grading a snapshot of a machine that genuinely arrived
//! where it says it did, and the alternative — a synthesised machine state — would grade the
//! synthesiser.
//!
//! # Why here and not in `zx-shot`'s own `mod tests`
//!
//! That module exists for what an integration test cannot reach: `MEDIA_FORMATS` and the `--hold`
//! edges are private to a binary. Nothing in this file is. [`frontend::media::save`] is the whole
//! of `--snapshot`, [`frontend::media::load_named`] is the whole of reading one back, and both are
//! library functions the window also calls — so grading them here grades the flag without linking
//! the binary, and without a second copy of the frame loop to drift from the first.
//!
//! What is **not** graded here: that `zx-shot` writes the file to the path `--snapshot` names.
//! That is `host::save`, which `tests/byte_sources.rs` already owns, and re-asserting it would be
//! grading the same function twice under a different name.

use std::collections::BTreeSet;

use frontend::palette::{self, CHANNELS, RGBA_BYTES};
use frontend::{keymap, media};
use macroquad::input::KeyCode;
use spectrum::{Frame, Model, Spectrum};

/// The game this file loads, under `testdata/games/`.
///
/// *Manic Miner* because it is the shortest full load in the corpus and because
/// `docs/images/README.md` publishes the exact command that reaches its first cavern, so the
/// numbers below are that page's readings rather than a sweep repeated here.
const GAME: &str = "ManicMiner.tap";

/// `LOAD ""`, as `docs/images/README.md` publishes it and as `zx-shot --keys` would be given it.
///
/// `J` is the whole keyword, because a 48K starts a line in `K` mode; each `LeftControl+P` is one
/// `SYMBOL SHIFT`+`P`, which is a quote.
const LOAD: &str = "J;LeftControl+P;LeftControl+P;Enter";

/// Frames each key is held, and then released.
///
/// Ten, which is `zx-shot`'s own default and what the gallery's *Central Cavern* command uses by
/// not passing `--hold`. Clear of both measured edges: under five the ROM misses the tap, over
/// thirty-five the editor types the script twice.
const HOLD: usize = 10;

/// Frames run after `ENTER`, over the whole cassette and into the game.
///
/// `docs/images/README.md`'s reading for its `central-cavern.png`, unchanged. It is a property of
/// this dump of this game and transfers nowhere.
const SETTLE: usize = 11_750;

/// Frames both machines are run on after the round trip, before they are compared again.
///
/// Four seconds of emulated time. Long enough that *Manic Miner*'s air bar has visibly drained and
/// its cavern has animated, which is what lets the *"the picture moved"* control below be an
/// assertion rather than a hope.
const CARRY_ON: usize = 200;

/// Colours a frame must contain before it is allowed to be evidence of anything.
///
/// A black screen compares equal to another black screen, and a machine that crashed into its own
/// display file would satisfy every equality in this file. Four is chosen to be unarguable: a
/// blank screen is one colour, a blank screen inside a border is two.
const DISTINCT_COLOURS: usize = 4;

/// The frame the window would upload, for the machine as it stands.
fn photograph(machine: &Spectrum) -> Box<[u8; RGBA_BYTES]> {
    let mut frame = Frame::new();
    machine.render(&mut frame);
    let mut rgba = palette::buffer();
    palette::write_rgba(&frame, &mut rgba);
    rgba
}

/// How many distinct RGBA pixels a frame holds.
///
/// The stride is [`palette::CHANNELS`] rather than a `4`, so a frame that grew a channel would
/// stop compiling here instead of counting pixels that are not pixels.
fn colours(rgba: &[u8; RGBA_BYTES]) -> usize {
    let (pixels, rest) = rgba.as_chunks::<CHANNELS>();
    assert!(rest.is_empty(), "a frame is a whole number of pixels");
    pixels.iter().collect::<BTreeSet<_>>().len()
}

/// Where two frames first disagree, and how widely, in a sentence a failure can print.
///
/// A bare `assert_eq!` on 327,680 bytes prints 327,680 bytes twice, which is not a diagnosis. The
/// count separates the two failures that matter: one wrong pixel is a rendering difference, and
/// tens of thousands is a machine that did not arrive.
fn difference(left: &[u8; RGBA_BYTES], right: &[u8; RGBA_BYTES]) -> Option<String> {
    let differing = left
        .iter()
        .zip(right.iter())
        .filter(|(a, b)| a != b)
        .count();
    let first = left.iter().zip(right.iter()).position(|(a, b)| a != b)?;
    Some(format!(
        "{differing} of {RGBA_BYTES} bytes differ, the first at byte {first} \
         (pixel {}): {:#04x} became {:#04x}",
        first / 4,
        left[first],
        right[first],
    ))
}

/// Hold `tap` for [`HOLD`] frames and release it for the same, through the real keymap.
///
/// The keyboard is re-applied **between** frames, exactly as the window re-polls the host between
/// them and exactly as `zx-shot`'s own `press` does. Collapsing it into one `apply` and a run of
/// frames would be a different thing from holding a key.
fn press(machine: &mut Spectrum, tap: &[KeyCode]) {
    for _ in 0..HOLD {
        keymap::apply(|code| tap.contains(&code), machine.keyboard_mut());
        machine.run_frame();
    }
    for _ in 0..HOLD {
        keymap::apply(|_| false, machine.keyboard_mut());
        machine.run_frame();
    }
}

/// A 48K that has loaded [`GAME`] from its cassette, or `None` when the corpus is not here.
///
/// The sequence is `zx-shot`'s, in its order and for its reasons: boot, type, **then** PLAY —
/// a drive started before the ROM has finished booting has the loader meeting the middle of a
/// block, which is why `media::insert` puts a tape in stopped.
fn machine_that_loaded_the_game() -> Option<Spectrum> {
    let roms = testsupport::testdata_dir().join("roms").join("48.rom");
    let games = testsupport::testdata_dir().join("games").join(GAME);
    let (Ok(rom), Ok(tape)) = (std::fs::read(&roms), std::fs::read(&games)) else {
        testsupport::skip_absent_corpus("the 48K ROM and a Manic Miner cassette", &games);
        return None;
    };

    let mut machine = media::start(&[&rom]).expect("one ROM is a 48K");
    assert_eq!(
        media::load_named(&mut machine, GAME, &tape).expect("a .tap this emulator can read"),
        media::Kind::Tape,
        "{GAME} stopped being a cassette, so this file is no longer loading one",
    );

    for _ in 0..120 {
        machine.run_frame();
    }
    for name in LOAD.split(';') {
        let tap: Vec<KeyCode> = name
            .split('+')
            .map(|key| keymap::code_named(key).expect("a key the window can press"))
            .collect();
        press(&mut machine, &tap);
    }
    machine.tape_mut().play();
    for _ in 0..SETTLE {
        machine.run_frame();
    }

    // A fault is a finding rather than a condition to handle, and it has to be checked before the
    // frames below are read: a machine that stopped would supply a very stable picture.
    assert!(
        machine.fault().is_none(),
        "the machine faulted while loading {GAME}, so nothing photographed after it means anything",
    );
    Some(machine)
}

/// Restore `bytes` into a machine built from the same ROM, the way a person opening a cartridge
/// would.
fn machine_restored_from(bytes: &[u8]) -> Spectrum {
    let rom = std::fs::read(testsupport::testdata_dir().join("roms").join("48.rom"))
        .expect("the ROM that was read a moment ago");
    let mut machine = media::start(&[&rom]).expect("one ROM is a 48K");
    assert_eq!(
        media::load_named(&mut machine, "cartridge.z80", bytes)
            .expect("the .z80 this crate just wrote"),
        media::Kind::Z80,
        "what `media::save` writes is no longer what `media::load_named` calls a snapshot",
    );
    machine
}

#[test]
fn the_cover_is_the_cartridge_seen() {
    let Some(loaded) = machine_that_loaded_the_game() else {
        return;
    };

    let played = photograph(&loaded);
    // The premise, before anything is compared to anything. A cassette that loaded nothing leaves
    // a machine at the `LOAD ""` prompt, which is a perfectly reproducible picture and would
    // satisfy every equality below.
    assert_eq!(
        loaded.model(),
        Model::Spectrum48K,
        "this file loads a 48K game and the machine says it is not a 48K",
    );
    assert!(
        colours(&played) >= DISTINCT_COLOURS,
        "the frame after the load holds {} colours, which is not a game: {SETTLE} frames of \
         cassette reached a blank screen",
        colours(&played),
    );

    let cartridge = media::save(&loaded);
    let restored = photograph(&machine_restored_from(&cartridge));

    assert!(
        difference(&played, &restored).is_none(),
        "the cartridge does not open on the picture it was taken from: {}",
        difference(&played, &restored).unwrap_or_default(),
    );
}

#[test]
fn the_restored_machine_goes_on_being_the_game() {
    let Some(mut loaded) = machine_that_loaded_the_game() else {
        return;
    };

    let cartridge = media::save(&loaded);
    let before = photograph(&loaded);
    let mut restored = machine_restored_from(&cartridge);

    for _ in 0..CARRY_ON {
        loaded.run_frame();
        restored.run_frame();
    }
    let carried_on = photograph(&loaded);
    let and_so_did_the_copy = photograph(&restored);

    // The control, and the whole reason this test is not a second copy of the one above: if the
    // game were not running, both machines would still agree and would agree about nothing. Manic
    // Miner's air bar drains every frame, so four seconds cannot leave the screen where it was.
    assert!(
        difference(&before, &carried_on).is_some(),
        "{CARRY_ON} frames changed nothing on the original machine, so the comparison below \
         would hold for a pair of frozen machines and grades nothing",
    );

    assert!(
        difference(&carried_on, &and_so_did_the_copy).is_none(),
        "the two machines diverged after {CARRY_ON} frames, so the snapshot carried the screen \
         and not the machine: {}",
        difference(&carried_on, &and_so_did_the_copy).unwrap_or_default(),
    );
    assert!(
        restored.fault().is_none(),
        "the restored machine faulted while being run on, so it was never the same machine",
    );
}
