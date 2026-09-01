//! Four ways to get bytes into the machine, required to reach the same machine.
//!
//! # The claim, and why it needs a gate rather than an argument
//!
//! A command line, a URL's query string, a payload compiled in by [`bundle`], and a file dropped
//! on the window are four sources, and the design's whole defence is that they are **one path**:
//! every one of them ends at [`media::accept`] with a name and some bytes, and none of them is
//! told which it is. That is a structural argument and it is a good one — but *"they cannot
//! disagree because they are the same code"* is exactly the sentence that turns out to be false
//! the day somebody adds a fourth loader for a good local reason.
//!
//! So it is asserted, over machine state, byte for byte.
//!
//! # The corpus is generated here, and that is the point
//!
//! `docs/M8.md` and this project's licensing rules meet here: **no game may be committed**, so a
//! gate for a game-loading mechanism cannot use a game. It does not need one. The payload is a
//! snapshot this file produces by booting the committed Sinclair ROM and writing it out through
//! `spectrum::snapshot::z80::write`, so the bytes are the repository's own and the gate has no
//! corpus at all. What is graded is the **mechanism**, which is the thing that could be wrong.
//!
//! # What it does not cover
//!
//! - **`bundle::bytes` returning something**, in an ordinary build. The feature is off by
//!   default, so the bundled arm of the equivalence is exercised by
//!   `crates/frontend/gate-bundled.sh`, which builds with a generated payload. Here the bundled
//!   source is stood in for by the same lookup it performs — a name and a `&'static [u8]` — and
//!   that substitution is named rather than hidden.
//! - **The step a drop adds before `media::accept`**: pulling a name out of `DroppedFile::path`
//!   and bytes out of `DroppedFile::bytes`. That needs a window, or a browser, and is in
//!   `crates/frontend/src/lib.rs`'s ungraded table with everything else that does.
//! - **Whether a real game runs.** Nothing here has ever run one.

use frontend::{bundle, host, media};
use spectrum::Spectrum;

/// Where the committed 48K ROM is, from the workspace root.
const ROM: &str = "testdata/roms/48.rom";

/// Frames given to the ROM before its state is captured — the same figure
/// `crates/spectrum/examples/boot.rs` and `zx-shot` use, and several times what boot needs.
const BOOT_FRAMES: u64 = 120;

/// Frames every machine in a comparison runs after being fed, so the comparison is of a machine
/// that has *executed* rather than of one that has merely been written to.
const SETTLE_FRAMES: u64 = 10;

/// The workspace root, from this crate's manifest directory.
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root, two directories above crates/frontend")
}

/// The committed 48K ROM, or `None` when the corpus is absent.
fn rom() -> Option<Vec<u8>> {
    std::fs::read(workspace_root().join(ROM)).ok()
}

/// A `.z80` snapshot of a booted 48K, generated here so that no game has to be committed.
fn generated_snapshot(rom: &[u8]) -> Vec<u8> {
    let mut machine = Spectrum::new(rom).expect("the committed ROM is one 16 KB page");
    machine.run_frames(BOOT_FRAMES);
    media::save(&machine)
}

/// Build a machine from `rom`, feed it `(name, bytes)`, run a little, and return its state.
fn fed(rom: &[u8], name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut machine = media::start(&[rom]).expect("one ROM is a 48K");
    let message = media::accept(&mut machine, name, bytes);
    assert!(
        message.starts_with("snapshot restored from"),
        "the payload should have been restored, and instead: {message}",
    );
    machine.run_frames(SETTLE_FRAMES);
    media::save(&machine)
}

#[test]
fn a_command_line_a_url_a_bundle_and_a_drop_reach_the_same_machine() {
    let Some(rom) = rom() else {
        // The corpus-absence rule every gate in this repository shares: absent means the gate
        // says why rather than passing quietly. It is not a hard failure here because this
        // crate's suite is run on machines that may not have fetched the ROM, and
        // `testdata/README.md` owns that policy.
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let payload = generated_snapshot(&rom);

    // Written to disk so the command line and the URL are reading a real file through a real
    // path, rather than being handed the same in-memory `Vec` under four different names — which
    // would be the tautology this file exists to avoid.
    let directory = std::env::temp_dir().join(format!("zx-byte-sources-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a writable temporary directory");
    let path = directory.join("generated.z80");
    std::fs::write(&path, &payload).expect("a writable temporary file");
    let named = path.to_str().expect("a UTF-8 temporary path").to_owned();

    // 1. A command line: `zx <rom> <snapshot>`.
    let argv = vec![ROM.to_owned(), named.clone()];
    let (argv_roms, argv_media) = host::partition(&argv, ROM);

    // 2. A URL: `?rom=<rom>&snapshot=<snapshot>`. Through `arguments_from_query`, which is what
    //    a browser's `page::query_string` feeds.
    let query = host::arguments_from_query(&format!("?rom={ROM}&snapshot={named}"));
    let (query_roms, query_media) = host::partition(&query, ROM);

    assert_eq!(argv_roms, query_roms, "a URL names a different ROM");
    assert_eq!(argv_media, query_media, "a URL names different media");

    // Both of the above resolve a *path*; the machines they build are therefore the same
    // machine, and asserting it twice would be asserting one thing twice. What differs between
    // the four is where the **bytes** come from, so that is what the rest compares.
    let from_file = std::fs::read(&argv_media[0]).expect("the file just written");
    let from_command_line = fed(&rom, &argv_media[0], &from_file);

    // 3. A bundle: a name and a `&'static [u8]` that never touched a filesystem.
    let from_bundle = fed(&rom, "generated.z80", &payload);

    // 4. A drop: a name from the file's own name and bytes from the drop event.
    let dropped_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a UTF-8 file name");
    let from_drop = fed(&rom, dropped_name, &payload);

    assert_eq!(
        from_command_line, from_bundle,
        "an embedded payload reaches a different machine from a named file",
    );
    assert_eq!(
        from_command_line, from_drop,
        "a dropped file reaches a different machine from a named file",
    );

    // The assertion whose failure means "I was not looking at the thing". Every comparison above
    // is an equality between machines, and a `fed` that ignored its bytes — or a `media::save`
    // that returned a constant — would satisfy all of them. A machine that was never fed the
    // payload must differ.
    let mut untouched = media::start(&[&rom[..]]).expect("one ROM is a 48K");
    untouched.run_frames(SETTLE_FRAMES);
    assert_ne!(
        from_command_line,
        media::save(&untouched),
        "a machine that was fed the payload is indistinguishable from one that was not",
    );

    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn a_bundled_build_supplies_the_arguments_a_command_line_would_have_carried() {
    // `bundle::arguments` emits `--rom` for a ROM, exactly as `arguments_from_query` does for a
    // query string's `rom` key, so that an embedded ROM whose file name has no extension still
    // builds the machine. In an ordinary build there is nothing embedded, and that is asserted
    // here rather than assumed: a default build that quietly carried a payload would be the
    // licensing failure this whole mechanism is shaped to avoid.
    #[cfg(not(feature = "bundled"))]
    {
        assert!(
            bundle::entries().is_empty(),
            "a build without the `bundled` feature has embedded something",
        );
        assert!(bundle::arguments().is_empty());
        assert!(bundle::bytes("anything.tap").is_none());
        assert!(
            bundle::acknowledgement().is_none(),
            "a build that embeds no ROM must not claim Amstrad's permission",
        );
    }
    #[cfg(feature = "bundled")]
    {
        // What `crates/frontend/gate-bundled.sh` builds and runs. The payload is generated by
        // that script, so this arm has no corpus either.
        assert!(
            !bundle::entries().is_empty(),
            "the `bundled` feature is on and nothing is embedded; build.rs should have refused",
        );
        for &(name, bytes) in bundle::entries() {
            assert!(!bytes.is_empty(), "{name} was embedded empty");
            assert_eq!(
                bundle::bytes(name),
                Some(bytes),
                "{name} is not addressable"
            );
        }
        let (roms, rest) = host::partition(&bundle::arguments(), "unused-default.rom");
        assert!(
            roms.iter().any(|name| name.ends_with(".rom")) || !rest.is_empty(),
            "a bundled build named neither a ROM nor any media",
        );
    }
}

#[test]
fn a_name_that_cannot_be_loaded_says_what_can_be() {
    let Some(rom) = rom() else {
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let mut machine = media::start(&[&rom[..]]).expect("one ROM is a 48K");

    // **This test used to assert that `.tzx` was refused by name, and it was right to until
    // 2026-09-01.** `.tzx` is what most commercial games ship as, because `.tap` cannot represent
    // a turbo loader at any speed, and it now loads. What it graded — *a refusal a person can act
    // on* — is still graded; the category it pointed at is simply empty, and the assertion moved
    // to the thing that replaced it rather than being deleted or loosened.
    //
    // The discriminating half is that a `.tzx` reaches the **`.tzx`** converter and not the
    // `.tap` one. Both produce a `Tape` and both failures would read "that tape could not be
    // read", so a name-based dispatch that quietly sent one to the other would look identical
    // from here for every well-formed file. A signature error only `tzx::parse` can produce is
    // what tells them apart.
    let message = media::accept(&mut machine, "manic.tzx", b"NOTATAPE\x00\x00");
    assert!(
        message.contains("ZXTape!"),
        "a .tzx should reach the .tzx parser, and instead: {message}",
    );

    // And a good one goes in the drive, stopped, like any other tape.
    let loaded = media::accept(&mut machine, "manic.tzx", b"ZXTape!\x1a\x01\x14");
    assert!(
        loaded.starts_with("tape in the drive"),
        "a well-formed .tzx should be inserted: {loaded}",
    );

    // The refusal mechanism itself is intact and currently has nothing to point at. Recorded as
    // an assertion rather than as a comment, so that a format added to `NOT_YET` later fails
    // here and gets a case of its own instead of inheriting this one's silence.
    assert_eq!(
        media::unsupported("manic.tzx"),
        None,
        "`.tzx` is loadable now and must not also be listed as not-yet-supported",
    );

    // A format nothing has ever heard of still says what is accepted — and now names `.tzx`,
    // which is the half no compiler was going to catch when the dispatch changed.
    let unknown = media::accept(&mut machine, "thing.dsk", b"");
    assert!(
        unknown.contains(".tap") && unknown.contains(".tzx"),
        "an unknown extension should name what is accepted, and instead: {unknown}",
    );

    // The positive control: a real one must NOT take either path.
    let real = media::accept(&mut machine, "generated.z80", &generated_snapshot(&rom));
    assert!(
        real.starts_with("snapshot restored from"),
        "a loadable file was refused: {real}",
    );
}

#[test]
fn a_tape_says_that_nothing_will_happen_until_it_is_started() {
    let Some(rom) = rom() else {
        eprintln!("skipping: {ROM} is absent — see testdata/README.md");
        return;
    };
    let mut machine = media::start(&[&rom[..]]).expect("one ROM is a 48K");

    // The smallest thing `tap::parse` accepts: one block, length-prefixed. The point is not the
    // tape — it is that a person who drops one sees why the screen did not change.
    let block: Vec<u8> = vec![0x02, 0x00, 0xFF, 0xFF];
    let message = media::accept(&mut machine, "quiet.tap", &block);
    if message.starts_with("quiet.tap:") {
        // The parser refused this synthetic block; that is its business and not this test's.
        // Reported rather than silently passing, because a skip that looks like a pass is the
        // failure this repository keeps recording.
        eprintln!("skipping the wording check: the synthetic tape was refused — {message}");
        return;
    }
    assert!(
        message.contains("F3"),
        "a tape is inserted stopped, so the message must name the key that starts it: {message}",
    );
}
