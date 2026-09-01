//! The external check on our reading of the `.tap` format: tapes a third party recorded.
//!
//! # What a foreign tape grades that nothing else here can
//!
//! `tape_signal.rs` grades the converter against a decoder this project wrote, and
//! `tape_rom_timings.rs` grades its **timings** against the ROM's own writer. Neither of them
//! grades the **file format**: both build their `.tap` bytes in the test, so a shared
//! misreading of the block framing — a length word taken as big-endian, the flag byte counted
//! outside the length, an off-by-one in where a block ends — would be invisible to both,
//! because the same misreading would be on each side of the comparison.
//!
//! A file somebody else's tool wrote breaks that symmetry, and it does it with an assertion
//! that owes nothing to us at all: **the parity byte**. Every block on a real tape ends with
//! the XOR of every byte before it, and that rule was applied by whoever recorded the tape.
//! So if this crate splits a 14300-byte block one byte wrong, or emits its bits in the wrong
//! order, or skips the wrong number of sync pulses, the bytes that come back out of the pulse
//! train **fail somebody else's checksum**. That is the same class of instrument as the
//! third-party snapshots in `snapshot_corpus.rs`, and it is the reason both exist.
//!
//! # It is a `#[test]`, and a directory sweep
//!
//! Not an example, not a `main` that prints a verdict — `docs/STATUS.md` records three gates
//! that ran nowhere and calls an `examples/` binary *"the worst form so far"*. And a sweep
//! rather than a list of named files, so a user's own tapes are graded too.
//!
//! # Absence
//!
//! Through `crates/testsupport` unchanged: present, it runs; absent, the gate **fails** naming
//! the fetch; absent under `ZX_CORPUS_ALLOW_MISSING`, it skips; and that opt-out is **refused
//! under `CI`**. One convention for every corpus in the workspace, not one per corpus.

use spectrum::tape::tap;
use std::path::{Path, PathBuf};

/// Half-period of a zero bit, and of a one bit — the two lengths a decoder separates.
const BIT_ZERO: u32 = 855;
const BIT_ONE: u32 = 1710;

/// The fetch instructions, repeated where a developer will actually read them.
const FETCH: &str = "mkdir -p testdata/tapes && cd testdata/tapes && base=https://raw.\
                     githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.\
                     EmulatorTestSuites.Z80/Program && curl -fsSL -O $base/Raxoft/V1_2A/\
                     z80doc.tap -O $base/Raxoft/V1_2A/z80memptr.tap -O $base/MarkWoodmass/\
                     z80tests.tap";

/// `testdata/tapes`, or `None` when the corpus is absent under the shared policy.
fn corpus_dir() -> Option<PathBuf> {
    // Unconditionally rather than only on the absent path: an obsolete spelling must be an
    // error in *every* gate, or a CI file still exporting one is silently ignored by whichever
    // gate happens to find its corpus present.
    testsupport::reject_obsolete_env();

    let path = testsupport::testdata_dir().join("tapes");
    if !path.is_dir() {
        println!("third-party tapes are fetched on demand:\n  {FETCH}");
        testsupport::skip_absent_corpus("third-party .tap tapes", &path);
        return None;
    }
    Some(path)
}

/// Every `.tap` in the corpus directory, in a stable order.
fn tapes(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("the corpus directory was just confirmed to exist")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "tap"))
        .collect();
    files.sort();
    files
}

/// The blocks a `.tap` file holds, read **without** `tape::tap`.
///
/// A two-byte little-endian length and then that many bytes, repeated. Written here so the
/// comparison below is against an independent reading of the file rather than against the one
/// under test.
fn blocks_in_file(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut blocks = Vec::new();
    let mut offset = 0;
    while offset + 2 <= bytes.len() {
        let length = usize::from(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
        offset += 2;
        let end = offset + length;
        assert!(end <= bytes.len(), "the corpus file is truncated");
        blocks.push(bytes[offset..end].to_vec());
        offset = end;
    }
    assert_eq!(offset, bytes.len(), "trailing bytes after the last block");
    blocks
}

/// Read every block back out of a pulse train.
///
/// The loader's rule, applied repeatedly: skip everything longer than a one bit — which is the
/// pilot tone and the inter-block silence — then skip the two sync half-periods, then take
/// half-periods in pairs until the next thing that is too long. The threshold is the midpoint
/// of the two published bit lengths.
fn decode_blocks(pulses: &[u32]) -> Vec<Vec<u8>> {
    const SYNC_PULSES: usize = 2;
    let mut blocks = Vec::new();
    let mut rest = pulses;

    while let Some(start) = rest.iter().position(|&pulse| pulse <= BIT_ONE) {
        rest = &rest[start..];
        if rest.len() < SYNC_PULSES {
            break;
        }
        rest = &rest[SYNC_PULSES..];
        let end = rest
            .iter()
            .position(|&pulse| pulse > BIT_ONE)
            .unwrap_or(rest.len());
        let (data, tail) = rest.split_at(end);
        blocks.push(bits_to_bytes(data));
        rest = tail;
    }
    blocks
}

/// Two half-periods per bit, most significant bit first.
fn bits_to_bytes(pulses: &[u32]) -> Vec<u8> {
    const THRESHOLD: u32 = (BIT_ZERO + BIT_ONE) / 2;
    let bits: Vec<bool> = pulses
        .iter()
        .step_by(2)
        .map(|&pulse| pulse > THRESHOLD)
        .collect();
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|byte| {
            byte.iter()
                .fold(0_u8, |value, &bit| (value << 1) | u8::from(bit))
        })
        .collect()
}

#[test]
fn every_tape_in_the_corpus_survives_the_trip_through_a_pulse_train() {
    let Some(dir) = corpus_dir() else {
        return;
    };
    let files = tapes(&dir);
    assert!(
        !files.is_empty(),
        "{} holds no .tap files. A directory that exists and is empty must not read as a \
         pass — see docs/STATUS.md on corpora that verify nothing.\n  {FETCH}",
        dir.display()
    );

    let mut blocks_checked = 0_usize;
    let mut bytes_checked = 0_usize;
    for path in &files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let bytes = std::fs::read(path).expect("a corpus file that was just listed");
        let expected = blocks_in_file(&bytes);
        assert!(!expected.is_empty(), "{name} holds no blocks");

        let tape = tap::parse(&bytes).unwrap_or_else(|err| panic!("{name} did not parse: {err}"));
        let decoded = decode_blocks(tape.pulses());

        assert_eq!(
            decoded.len(),
            expected.len(),
            "{name}: the signal carries {} blocks and the file holds {}",
            decoded.len(),
            expected.len()
        );
        for (index, (got, want)) in decoded.iter().zip(&expected).enumerate() {
            assert_eq!(
                got.len(),
                want.len(),
                "{name} block {index}: recovered {} bytes of {}",
                got.len(),
                want.len()
            );
            assert!(
                got == want,
                "{name} block {index}: the bytes that came back are not the bytes on the tape"
            );
            blocks_checked += 1;
            bytes_checked += want.len();
        }
    }

    // Say what was covered rather than only that nothing failed. `docs/STATUS.md`: *"make the
    // tool state what it covered, and assert on that, rather than on its verdict"*.
    println!(
        "tape corpus: {} files, {blocks_checked} blocks, {bytes_checked} bytes",
        files.len()
    );
    assert!(blocks_checked >= files.len(), "every file must contribute");
}

#[test]
fn the_recovered_blocks_satisfy_somebody_elses_checksum() {
    // **The assertion that owes nothing to this project.** Every block on a real tape ends with
    // the XOR of the bytes before it, applied by whoever recorded it. A block split one byte
    // wrong, a bit order reversed, or the wrong number of sync pulses skipped all produce bytes
    // that fail this — over blocks of 14300 bytes, not over a fixture we chose.
    let Some(dir) = corpus_dir() else {
        return;
    };
    let mut checked = 0_usize;
    for path in tapes(&dir) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let bytes = std::fs::read(&path).expect("a corpus file that was just listed");
        let tape = tap::parse(&bytes).unwrap_or_else(|err| panic!("{name} did not parse: {err}"));

        for (index, block) in decode_blocks(tape.pulses()).iter().enumerate() {
            let Some((&parity, body)) = block.split_last() else {
                panic!("{name} block {index} is empty");
            };
            assert_eq!(
                body.iter().fold(0_u8, |sum, &byte| sum ^ byte),
                parity,
                "{name} block {index}: the parity byte does not match the {} bytes before it",
                body.len()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no block was checked");
    println!("tape corpus: {checked} parity bytes verified");
}

#[test]
fn a_real_tape_alternates_headers_and_data_blocks() {
    // The structural claim the format makes and our converter acts on: a block whose flag byte
    // is under 128 is a header and gets the longer pilot tone. Every tape recorded by the ROM's
    // own SAVE is header/data pairs, and the headers are 19 bytes — a flag, seventeen of
    // description, and the parity. This is what makes the flag-bit branch in `tap::pilot_pulses`
    // exercised by real data rather than only by a fixture that chose its own flags.
    const HEADER_BLOCK_LEN: usize = 19;
    let Some(dir) = corpus_dir() else {
        return;
    };
    let mut headers = 0_usize;
    let mut data = 0_usize;
    for path in tapes(&dir) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let bytes = std::fs::read(&path).expect("a corpus file that was just listed");
        for (index, block) in blocks_in_file(&bytes).iter().enumerate() {
            let Some(&flag) = block.first() else {
                panic!("{name} block {index} is empty");
            };
            if flag < 0x80 {
                assert_eq!(
                    block.len(),
                    HEADER_BLOCK_LEN,
                    "{name} block {index} has a header's flag and is not 19 bytes"
                );
                headers += 1;
            } else {
                data += 1;
            }
        }
    }
    assert!(headers > 0, "no header block in the corpus");
    assert!(data > 0, "no data block in the corpus");
    println!("tape corpus: {headers} header blocks, {data} data blocks");
}
