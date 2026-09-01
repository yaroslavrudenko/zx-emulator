//! Which file is which, and what a running machine will accept.

use frontend::media::{self, Error, Kind};
use spectrum::memory::PAGE_SIZE;

#[test]
fn an_extension_names_the_format() {
    let cases = [
        ("48.rom", Some(Kind::Rom)),
        ("game.tap", Some(Kind::Tape)),
        ("save.z80", Some(Kind::Z80)),
        ("fire.sna", Some(Kind::Sna)),
        ("testdata/roms/128-0.rom", Some(Kind::Rom)),
        ("/absolute/path/to/a.tap", Some(Kind::Tape)),
        ("a.name.with.dots.z80", Some(Kind::Z80)),
    ];
    for (path, expected) in cases {
        assert_eq!(media::kind_of(path), expected, "{path}");
    }
}

#[test]
fn the_extension_is_matched_without_regard_to_case() {
    // `.TAP` and `.Z80` are how half the archives on the internet are named, and a
    // case-sensitive match would reject them with "not a file this emulator knows".
    for path in [
        "GAME.TAP",
        "Game.Tap",
        "SAVE.Z80",
        "Fire.SNA",
        "48.ROM",
        "MANIC.TZX",
        "Manic.Tzx",
    ] {
        assert!(media::kind_of(path).is_some(), "{path}");
    }
    assert_eq!(media::kind_of("GAME.TAP"), Some(Kind::Tape));
    assert_eq!(media::kind_of("Fire.SNA"), Some(Kind::Sna));
    assert_eq!(media::kind_of("MANIC.TZX"), Some(Kind::Tzx));
}

#[test]
fn anything_else_is_refused_rather_than_guessed_at() {
    for path in ["notes.txt", "archive.zip", "noextension", "", ".", "a."] {
        assert_eq!(media::kind_of(path), None, "{path:?}");
    }
}

#[test]
fn a_tzx_is_its_own_kind_and_not_a_tape() {
    // `game.tzx` used to be in the list above, because answering `Some(Tape)` for a format that
    // could not be read would have turned "unsupported format" into "corrupt tape". It reads now
    // — and the distinction it was protecting still matters, pointing the other way: `Tzx` must
    // not collapse into `Tape`, because `tzx::parse` needs the machine's `Model` and `tap::parse`
    // does not. A `.tzx` sent to the `.tap` converter would fail on the signature, and one sent
    // to the right converter with the wrong model would load and play at the wrong speed, which
    // is far worse than a refusal.
    assert_eq!(media::kind_of("game.tzx"), Some(Kind::Tzx));
    assert_ne!(media::kind_of("game.tzx"), media::kind_of("game.tap"));
}

#[test]
fn a_rom_of_the_wrong_size_is_refused_with_the_size_in_the_message() {
    let error = media::start(&[&[0; 100]]).expect_err("100 bytes is not a page");
    assert!(matches!(error, Error::Rom(_)), "{error:?}");
    // The source chain reaches the parser's own message rather than stopping at "bad ROM",
    // which is the whole reason these wrap with `#[from]`.
    assert!(
        !error.to_string().is_empty() && error.to_string().contains("16 KB"),
        "{error}",
    );
}

#[test]
fn a_page_sized_rom_starts_a_machine_at_the_top_of_frame_zero() {
    let machine = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    assert_eq!(machine.frames(), 0);
}

#[test]
fn the_number_of_roms_is_what_names_the_machine() {
    // The whole reason `start` takes a slice rather than existing twice. One ROM is a 48K, two
    // are a 128, and nothing else is a machine.
    //
    // Two assertions, and the second is the one with teeth.
    //
    // The model alone is not enough: a `spectrum_128` that ignored its second argument would
    // still report `Spectrum128`, so "it built a 128" and "it used both ROMs" are separate
    // claims. The two images are therefore given **different** first bytes, and the machine is
    // asked what is at `0x0000` — which is the editor ROM at reset, and is the assertion that
    // the pair went in in the order this crate documents rather than in the other one.
    let mut editor = [0_u8; PAGE_SIZE];
    let mut basic = [0_u8; PAGE_SIZE];
    editor[0] = 0xE0;
    basic[0] = 0xBA;

    let forty_eight = media::start(&[&editor[..]]).expect("one page-sized ROM");
    let one_two_eight = media::start(&[&editor[..], &basic[..]]).expect("two page-sized ROMs");

    assert_ne!(
        forty_eight.snapshot().model(),
        one_two_eight.snapshot().model(),
        "one ROM and two ROMs must not produce the same machine",
    );
    assert_eq!(
        one_two_eight.memory().read(0x0000),
        0xE0,
        "the first ROM named is the one paged in at reset",
    );
}

#[test]
fn a_count_of_roms_no_machine_is_made_of_is_refused_with_the_count_in_it() {
    // Zero and three, which are the two ways to get here: a caller that built an empty list,
    // and a person who named one ROM too many. Neither is resolved by a rule — "use the first
    // two" would build a machine out of a typo.
    for given in [0_usize, 3, 4] {
        let images = vec![&[0_u8; PAGE_SIZE][..]; given];
        let error = media::start(&images).expect_err("not one ROM and not two");
        assert!(
            matches!(error, Error::RomCount { given: n } if n == given),
            "{given}: {error:?}",
        );
        assert!(error.to_string().contains(&given.to_string()), "{error}");
    }
}

#[test]
fn a_wrong_sized_rom_in_a_pair_is_refused_in_either_position() {
    // Both positions, because a constructor that validates only its first argument passes a
    // single-position test and ships a machine with half a ROM in it.
    for (first, second) in [
        (&[0_u8; PAGE_SIZE][..], &[0_u8; 100][..]),
        (&[0_u8; 100][..], &[0_u8; PAGE_SIZE][..]),
    ] {
        let error = media::start(&[first, second]).expect_err("100 bytes is not a page");
        assert!(matches!(error, Error::Rom(_)), "{error:?}");
    }
}

#[test]
fn a_malformed_tape_is_refused() {
    let mut machine = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    // A length word promising three bytes with none behind it.
    let error = media::insert(&mut machine, Kind::Tape, &[0x03, 0x00])
        .expect_err("a truncated block is not a tape");
    assert!(matches!(error, Error::Tape(_)), "{error:?}");
}

#[test]
fn a_malformed_snapshot_is_refused_in_both_formats() {
    let mut machine = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    for kind in [Kind::Z80, Kind::Sna] {
        let error = media::insert(&mut machine, kind, &[0x00, 0x01, 0x02])
            .expect_err("three bytes is not a snapshot");
        assert!(matches!(error, Error::Snapshot(_)), "{kind:?}: {error:?}");
    }
}

#[test]
fn a_rom_named_after_the_machine_is_running_is_refused_rather_than_ignored() {
    // This comment used to say "reachable from real input — `zx --rom a.rom b.rom`", and that
    // was never true: `partition` routes **every** `.rom` argument into the ROM list and none of
    // them into the media list, so no command line has ever reached `insert` with `Kind::Rom`.
    // Checked against `main.rs` rather than reasoned about. What *is* reachable is a `.rom`
    // dropped onto a running window — `docs/M8.md` Decision 5 — which is a likelier gesture than
    // the command line ever was. Silently dropping a file somebody handed over is still the
    // worse of the two failures, so the variant and this gate stand; only the claim about how
    // you get here was wrong.
    let mut machine = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    let error = media::insert(&mut machine, Kind::Rom, &[0; PAGE_SIZE])
        .expect_err("a ROM is not something a running machine takes");
    assert!(matches!(error, Error::RomAfterStart), "{error:?}");
}

#[test]
fn a_valid_tape_goes_into_the_drive() {
    let mut machine = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    assert!(
        machine.tape_mut().pulses().is_empty(),
        "the drive starts empty"
    );

    // The one-block tape from `spectrum::tape`'s own doc example: a length word, a flag byte,
    // one byte of data, and a checksum.
    media::insert(&mut machine, Kind::Tape, &[0x03, 0x00, 0xFF, 0x2A, 0xD5])
        .expect("a well-formed one-block tape");
    assert!(
        machine.tape_mut().pulses().len() > 3000,
        "a data block opens with 3223 pilot pulses",
    );
}

#[test]
fn a_saved_snapshot_reads_back_as_the_machine_that_wrote_it() {
    // Not a round-trip gate — `crates/spectrum` has several of those and they are stronger
    // than anything here. This asserts the far narrower thing this crate is responsible for:
    // that `save` produces bytes `insert` accepts, so `F2` and a later command line agree.
    let mut machine = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    machine.run_frames(3);
    machine.memory_mut().write(0x8000, 0xA5);

    let saved = media::save(&machine);
    let mut restored = media::start(&[&[0; PAGE_SIZE]]).expect("a page-sized ROM");
    media::insert(&mut restored, Kind::Z80, &saved).expect("our own snapshot");

    assert_eq!(restored.memory().read(0x8000), 0xA5);

    // The whole CPU state **except** the fields `spectrum::snapshot::UNPRESERVED` names, which
    // is `wz`, `q` and `halted`. This assertion first compared the states whole, and it failed
    // on `q` — correctly. No format carries these three; `crates/spectrum` enumerates them
    // precisely because *"a field dropped in both directions is the one defect a round trip is
    // green for"*, and each is proven dropped by its own gate next door. Comparing whole
    // states here was asserting something the `.z80` format cannot deliver, so the test was
    // wrong rather than the code, and it is narrowed rather than deleted: the other seventeen
    // fields are still compared.
    let after = restored.cpu_state();
    let mut expected = machine.cpu_state();
    expected.wz = after.wz;
    expected.q = after.q;
    expected.halted = after.halted;
    assert_eq!(after, expected);

    // If a fourth field ever joins that list, the exclusion above silently stops covering it.
    assert_eq!(
        spectrum::snapshot::UNPRESERVED.len(),
        3,
        "UNPRESERVED has changed; this test excludes exactly wz, q and halted",
    );
}
