//! The one instrument that grades our *reading* of the formats rather than our consistency
//! with ourselves.
//!
//! # Why a third-party file is a different class of evidence
//!
//! Every round trip in this milestone compares a value against a value that came from the
//! same code, so a symmetric misreading — a field permuted, an offset shared by both sides,
//! a field dropped in both directions — survives all of them. The hand-transcribed vectors in
//! `snapshot_vectors.rs` break that symmetry using the format *description*. A file somebody
//! else's emulator wrote breaks it using an independent *implementation* of that description,
//! which is a materially different thing and the only external oracle the machine layer has.
//!
//! `docs/M6.md` calls this out as the thing M6 has and M5 did not: *"a snapshot format is an
//! interoperation contract with dozens of independent implementations"*.
//!
//! # It is a `#[test]`, and that sentence is load-bearing
//!
//! Not an example, not a binary, not a `main` that prints a verdict. `docs/STATUS.md` records
//! three gates that ran nowhere, the last of which — the M5 boot gate as an `examples/`
//! binary — it calls *"the worst form so far"*, because `cargo test` builds an example
//! without ever calling its `main`.
//!
//! # Absence
//!
//! Through `crates/testsupport` unchanged — one convention for every corpus in the
//! workspace, not one per corpus. Present, it runs; absent, the gate **fails** naming the
//! fetch instructions; absent with `ZX_CORPUS_ALLOW_MISSING`, it skips; and that opt-out is
//! **refused under `CI`**, because an opt-out for a human on a fresh clone must never decide
//! what a pipeline verifies.

use spectrum::snapshot::{Error, sna, z80};
use std::path::{Path, PathBuf};

/// What a snapshot file turned out to be.
#[derive(Debug, Default)]
struct Tally {
    /// Files that parsed into a `Snapshot`.
    parsed: usize,
    /// Files refused as hardware this milestone does not model.
    ///
    /// Counted rather than failed, because a 128 snapshot in the directory is an **M7
    /// boundary and not a defect**. Counted rather than ignored, because a directory holding
    /// nothing else must not read as a pass — which is the whole lesson of
    /// `docs/STATUS.md`'s corpus-absence findings: *make the tool state what it covered, and
    /// assert on that, rather than on its verdict*.
    unsupported: usize,
}

/// `testdata/snapshots`, or `None` when the corpus is absent under the shared policy.
fn corpus_dir() -> Option<PathBuf> {
    // Unconditionally, not only on the absent path: an obsolete spelling must be an error in
    // *every* gate, or a CI file still exporting one is silently ignored by whichever gate
    // happens to find its corpus present.
    testsupport::reject_obsolete_env();

    let path = testsupport::testdata_dir().join("snapshots");
    if !path.is_dir() {
        testsupport::skip_absent_corpus("third-party snapshots", &path);
        return None;
    }
    Some(path)
}

/// Every `.z80` and `.sna` in the corpus directory, sorted so a failure is reproducible.
fn snapshot_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", dir.display()))
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("z80") || extension.eq_ignore_ascii_case("sna")
                })
        })
        .collect();
    files.sort();
    files
}

/// Whether `path` names a `.z80`.
fn is_z80(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("z80"))
}

#[test]
fn every_third_party_snapshot_parses_and_survives_our_own_encoder() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    let files = snapshot_files(&dir);
    assert!(
        !files.is_empty(),
        "{} exists but holds no .z80 or .sna files — see testdata/README.md",
        dir.display()
    );

    let mut tally = Tally::default();
    for path in &files {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
        let name = path.display();

        let parsed = if is_z80(path) {
            z80::parse(&bytes)
        } else {
            sna::parse(&bytes)
        };
        let snapshot = match parsed {
            Ok(snapshot) => snapshot,
            Err(Error::UnsupportedHardware { mode }) => {
                println!("{name}: hardware mode {mode} is an M7 machine, not a 48K");
                tally.unsupported += 1;
                continue;
            }
            Err(error) => panic!("{name} did not parse: {error}"),
        };
        tally.parsed += 1;

        // **Not** `write(parse(f)) == f`. Byte-identity over a foreign file is neither
        // achievable nor desirable — `docs/M6.md` Decision 7 — because a version 1 file
        // re-serialised as version 3 is a different byte string for the same machine, and
        // because a foreign header carries bits that are not machine state. What *is*
        // meaningful is that everything the foreign file gave us survives our own encoder.
        let reparsed = z80::parse(&z80::write(&snapshot))
            .unwrap_or_else(|error| panic!("{name}: our own output did not re-parse: {error}"));
        assert_eq!(
            reparsed, snapshot,
            "{name}: a field the foreign file carried was lost by our writer"
        );

        // A 48K snapshot carries three banks, and a file that parsed with fewer means either
        // the file is unusual or we stopped reading early.
        assert_eq!(
            [5_u8, 2, 0]
                .into_iter()
                .filter(|&bank| snapshot
                    .bank(spectrum::memory::BankIndex::new(bank))
                    .is_some())
                .count(),
            3,
            "{name}: a 48K snapshot should carry banks 5, 2 and 0"
        );
    }

    println!(
        "third-party snapshots: {} parsed, {} refused as non-48K hardware, {} files in {}",
        tally.parsed,
        tally.unsupported,
        files.len(),
        dir.display()
    );
    assert!(
        tally.parsed > 0,
        "{} holds {} snapshot files and not one of them was a 48K this milestone reads — \
         the sweep verified nothing",
        dir.display(),
        files.len()
    );
}

#[test]
fn a_third_party_pair_of_the_same_state_reads_the_same_through_both_parsers() {
    // The strongest single assertion available here, and it needs no expectation of ours at
    // all: when a third party saved one machine in both formats, the two files are an
    // *independent* claim that a particular `.z80` and a particular `.sna` describe the same
    // state. Our two parsers must agree with that claim, and a wrong offset in either one
    // breaks it — including offsets no hand-written vector happened to vary.
    //
    // `.sna` has no writer and therefore no round trip of its own, so this is the only
    // external instrument that grades it at all.
    let Some(dir) = corpus_dir() else {
        return;
    };

    let mut pairs = 0;
    for path in snapshot_files(&dir) {
        if !is_z80(&path) {
            continue;
        }
        let partner = path.with_extension("sna");
        if !partner.is_file() {
            continue;
        }
        pairs += 1;

        let from_z80 = z80::parse(&std::fs::read(&path).expect("readable"))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let from_sna = sna::parse(&std::fs::read(&partner).expect("readable"))
            .unwrap_or_else(|error| panic!("{}: {error}", partner.display()));
        let what = path.display();

        // Everything both formats carry as a field.
        assert_eq!(from_z80.cpu.af, from_sna.cpu.af, "{what}: AF");
        assert_eq!(from_z80.cpu.bc, from_sna.cpu.bc, "{what}: BC");
        assert_eq!(from_z80.cpu.de, from_sna.cpu.de, "{what}: DE");
        assert_eq!(from_z80.cpu.hl, from_sna.cpu.hl, "{what}: HL");
        assert_eq!(
            from_z80.cpu.af_shadow, from_sna.cpu.af_shadow,
            "{what}: AF'"
        );
        assert_eq!(
            from_z80.cpu.bc_shadow, from_sna.cpu.bc_shadow,
            "{what}: BC'"
        );
        assert_eq!(
            from_z80.cpu.de_shadow, from_sna.cpu.de_shadow,
            "{what}: DE'"
        );
        assert_eq!(
            from_z80.cpu.hl_shadow, from_sna.cpu.hl_shadow,
            "{what}: HL'"
        );
        assert_eq!(from_z80.cpu.ix, from_sna.cpu.ix, "{what}: IX");
        assert_eq!(from_z80.cpu.iy, from_sna.cpu.iy, "{what}: IY");
        assert_eq!(from_z80.cpu.i, from_sna.cpu.i, "{what}: I");
        assert_eq!(from_z80.cpu.im, from_sna.cpu.im, "{what}: interrupt mode");
        assert_eq!(from_z80.border, from_sna.border, "{what}: border");

        // And the one field the `.sna` does not have: `PC` is popped off the guest's stack,
        // which is the format's defining property. If our pop reads the wrong two bytes this
        // is where it shows, against a value a third party put in a `.z80` field.
        assert_eq!(
            from_z80.cpu.pc, from_sna.cpu.pc,
            "{what}: the PC popped from the .sna's stack must equal the .z80's PC field"
        );
        // And `SP` must come back **equal**, which is the property the pop exists to create
        // and is worth stating carefully because the obvious guess is wrong. A `.sna` writer
        // *pushes* `PC`, so its stored `SP` is two **below** the machine's; the parser pops
        // and adds two, restoring the original. So a correct reader lands on the same `SP`
        // the `.z80` carries as a plain field — not two above it.
        //
        // This assertion was written the other way round first, and this corpus is what
        // corrected it: `fire.z80` holds SP = 0x7FE6 and `fire.sna` holds 0x7FE4. An
        // independent implementation disagreeing with an expectation is exactly what an
        // external oracle is for, and it grades the `wrapping_add` that nothing else can.
        assert_eq!(
            from_z80.cpu.sp, from_sna.cpu.sp,
            "{what}: the .sna writer pushed PC, so a correct pop restores the same SP"
        );
    }

    println!("third-party cross-format pairs checked: {pairs}");
    assert!(
        pairs > 0,
        "{} holds no `<name>.z80` / `<name>.sna` pair, so nothing graded `.sna`'s offsets \
         against an independent implementation — see testdata/README.md, which names one",
        dir.display()
    );
}
