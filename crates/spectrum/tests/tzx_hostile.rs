//! `.tzx` as untrusted bytes: the parser must return, always, on anything.
//!
//! # Why this is not a style preference
//!
//! `crates/spectrum` builds with `panic = "abort"` in release. A panic on a malformed tape is
//! therefore **not** a recoverable error — it kills the process, and `catch_unwind` is not
//! available as a backstop. `docs/M6.md` Decision 6 sets the requirement accordingly: not *"do
//! not panic on the inputs we tested"* but **remove the constructs that can panic**, and this
//! project has shipped a guest-triggered abort once already.
//!
//! Structure and behaviour are graded separately, and they are not substitutes:
//!
//! | | |
//! |---|---|
//! | **structure** | `tape::tests::there_is_no_indexing_anywhere_in_the_tape_module` and `nothing_in_the_tape_module_can_panic_on_purpose`, in the crate's own test tree — a scanner over the production half of all five source files, with its own positive **and** negative cases |
//! | **behaviour** | this file: an exhaustive sweep over every block ID, a truncation sweep, single-byte mutations, and two `proptest`s over arbitrary bytes |
//!
//! # The two ceilings get their own sweeps
//!
//! `.tzx` is the first parser in this workspace where "no allocation is ever sized from the
//! file" and "every loop terminates" both cost something. A loop block multiplies a body by up
//! to 65535 while costing three bytes, and a jump block can revisit forever without consuming
//! any input. Both are bounded, and **the bounds are graded by files that reach them** rather
//! than by the prose that describes them.

use spectrum::Model;
use spectrum::tape::{Error, tzx};

/// The ten-byte `.tzx` header: `"ZXTape!"`, the end-of-text marker, and revision 1.20.
const HEADER: [u8; 10] = [b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14];

/// Both models, because one block's meaning depends on which machine is playing and a hostile
/// file must be refused the same way on either.
const MODELS: [Model; 2] = [Model::Spectrum48K, Model::Spectrum128];

fn file(blocks: &[&[u8]]) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    for block in blocks {
        bytes.extend_from_slice(block);
    }
    bytes
}

/// A file exercising one block of every shape the converter reads a length or a count from.
fn representative_file() -> Vec<u8> {
    file(&[
        &[0x10, 0x00, 0x00, 0x03, 0x00, 0xFF, 0x2A, 0xD5], // standard speed
        &[
            0x11, 0xE8, 0x03, 0x2C, 0x01, 0x90, 0x01, 0xF4, 0x01, 0xBC, 0x02, 0x02, 0x00, 0x08,
            0x01, 0x00, 0x01, 0x00, 0x00, 0x5A,
        ], // turbo speed
        &[0x12, 0x10, 0x00, 0x02, 0x00],                   // pure tone
        &[0x13, 0x02, 0x11, 0x00, 0x22, 0x00],             // pulse sequence
        &[
            0x14, 0x0A, 0x00, 0x14, 0x00, 0x08, 0x01, 0x00, 0x01, 0x00, 0x00, 0xA5,
        ], // pure data
        &[0x15, 0x4F, 0x00, 0x01, 0x00, 0x08, 0x01, 0x00, 0x00, 0xC3], // direct recording
        &[0x20, 0x01, 0x00],                               // pause
        &[0x21, 0x02, b'h', b'i'],                         // group start
        &[0x22],                                           // group end
        &[0x2B, 0x01, 0x00, 0x00, 0x00, 0x01],             // set signal level
        &[0x30, 0x02, b'o', b'k'],                         // text description
        &[0x32, 0x04, 0x00, 0x01, 0x00, 0x01, b'x'],       // archive info
        &[0x33, 0x01, 0x00, 0x01, 0x00],                   // hardware type
        &[0x5A, b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14], // glue
    ])
}

#[test]
fn the_representative_file_actually_parses() {
    // The positive control for every sweep below. A fixture that did not parse would make the
    // truncation and mutation sweeps trivially green — every variant would be an error because
    // the original already was — which is this project's recurring failure in its cheapest
    // form: a count of zero and an absence of the subject are the same observation.
    let bytes = representative_file();
    for model in MODELS {
        let tape = tzx::parse(&bytes, model).unwrap_or_else(|error| {
            panic!("the representative file must parse, and it failed with {error}")
        });
        assert!(
            tape.pulses().len() > 3000,
            "it must carry real signal, not just structure"
        );
    }
}

/// Every block ID the format description defines, transcribed from its own "TZX block ID list"
/// plus the four it lists under "Deprecated blocks".
const DEFINED_IDS: [u8; 29] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, // the blocks that carry signal
    0x16, 0x17, 0x18, 0x19, // C64 pair, CSW, generalized
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2A, 0x2B, // structure
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x40, 0x5A, // metadata and deprecated
];

#[test]
fn every_id_the_format_defines_is_recognised_and_no_other_is() {
    // The claim the module's refusal policy rests on: *"Every ID the description defines is
    // handled here, so an unrecognised one is either newer than revision 1.20 or not a `.tzx`
    // at all."* That sentence is only safe if it is true, so it is a test rather than a
    // comment — an ID the converter forgot would be refused as unknown, and a real file
    // carrying it would be refused with it.
    //
    // The undefined half needs no body at all: an unrecognised ID is refused before its length
    // is ever consulted, which is the whole point of refusing rather than guessing.
    let mut recognised = 0_usize;
    let mut unknown = 0_usize;

    for id in 0..=u8::MAX {
        let defined = DEFINED_IDS.contains(&id);
        // A body of group-end blocks: each is one byte with no body of its own, so whatever
        // the block under test does not consume parses cleanly instead of inventing new IDs.
        let mut block = vec![id];
        block.extend(std::iter::repeat_n(0x22, 64));
        let bytes = file(&[&block]);

        for model in MODELS {
            let verdict = tzx::parse(&bytes, model);
            let is_unknown =
                matches!(verdict, Err(Error::UnknownBlock { id: named, .. }) if named == id);
            assert_eq!(
                is_unknown, !defined,
                "block ID {id:#04X}: defined={defined}, but the parse said {verdict:?}"
            );
            if defined {
                recognised += 1;
            } else {
                unknown += 1;
            }
        }
    }

    // Both halves must be non-empty, or one of the two branches above is asserting nothing.
    assert_eq!(recognised, DEFINED_IDS.len() * MODELS.len());
    assert_eq!(unknown, (256 - DEFINED_IDS.len()) * MODELS.len());
}

#[test]
fn a_refusal_names_a_byte_that_is_really_there() {
    // An error a user cannot act on is a stack trace with better formatting, so every refusal
    // carries an offset and an ID — and the two have to agree with the file. Checked across
    // every ID rather than for one hand-picked case, because an offset that is right for the
    // first block and wrong for a later one is exactly the kind of defect a single case misses.
    for id in 0..=u8::MAX {
        let mut block = vec![id];
        block.extend(std::iter::repeat_n(0x22, 64));
        let bytes = file(&[&[0x12, 0x10, 0x00, 0x01, 0x00], &block]);

        for model in MODELS {
            if let Err(
                Error::UnknownBlock { id: named, offset }
                | Error::UnplayableBlock { id: named, offset }
                | Error::MisplacedBlock { id: named, offset },
            ) = tzx::parse(&bytes, model)
            {
                assert_eq!(
                    bytes.get(offset).copied(),
                    Some(named),
                    "block ID {id:#04X}: the error named {named:#04X} at offset {offset}, \
                     where the file holds something else"
                );
            }
        }
    }
}

#[test]
fn every_prefix_of_the_representative_file_returns() {
    // The exhaustive truncation sweep. Unlike a snapshot, a `.tzx` prefix that ends on a block
    // boundary is a valid shorter tape, so the assertion here is that the parser **returns** —
    // `tape::tzx::tests::a_prefix_parses_exactly_when_it_ends_on_a_block_boundary` is the
    // sharper one that says exactly which prefixes are valid, and the two are complementary.
    let bytes = representative_file();
    for k in 0..=bytes.len() {
        let prefix = bytes.get(..k).expect("k <= len");
        for model in MODELS {
            let _ = tzx::parse(prefix, model);
        }
    }
}

#[test]
fn every_single_byte_mutation_of_the_representative_file_returns() {
    // A byte at a time, every position, every value. This is where a length field becomes
    // enormous, a block ID becomes one that does not exist, a used-bits count leaves its range,
    // and a jump offset points off the end — each of which is a separate refusal path, and none
    // of which may be an abort.
    let original = representative_file();
    for index in 0..original.len() {
        for value in [0x00_u8, 0x01, 0x7F, 0x80, 0xFE, 0xFF] {
            let mut mutated = original.clone();
            let byte = mutated.get_mut(index).expect("index < len");
            if *byte == value {
                continue;
            }
            *byte = value;
            for model in MODELS {
                let _ = tzx::parse(&mutated, model);
            }
        }
    }
}

#[test]
fn a_pathological_loop_is_refused_by_one_of_the_two_ceilings() {
    // Every combination of a big loop count and a big body, so both bounds are reached by real
    // files rather than described in prose. A loop over a block that emits hits the tape
    // ceiling; a loop over blocks that do not hits the block budget; and either way the answer
    // is a returned error rather than a hang or a multi-gigabyte allocation.
    let tone = |count: u16| {
        let mut block = vec![0x12_u8, 0x01, 0x00];
        block.extend(count.to_le_bytes());
        block
    };
    let cases: [(&str, Vec<u8>); 3] = [
        (
            "65535 passes over a 65535-pulse tone",
            file(&[&[0x24, 0xFF, 0xFF], &tone(0xFFFF), &[0x25]]),
        ),
        (
            "a loop whose body jumps back into itself",
            file(&[&[0x24, 0xFF, 0xFF], &[0x23, 0x00, 0x00], &[0x25]]),
        ),
        ("a jump to itself", file(&[&[0x23, 0x00, 0x00]])),
    ];

    for (name, bytes) in cases {
        assert!(
            bytes.len() < 40,
            "{name}: a small file demanding a large one"
        );
        for model in MODELS {
            match tzx::parse(&bytes, model) {
                Err(Error::TapeTooLong { .. } | Error::TooManyBlocksPlayed { .. }) => {}
                other => panic!("{name}: expected a ceiling, got {other:?}"),
            }
        }
    }
}

#[test]
fn a_declared_length_larger_than_the_file_is_truncated_rather_than_allocated() {
    // The three width classes a `.tzx` length comes in — a WORD, a BYTE[3] and a DWORD — each
    // set to its maximum with nothing behind it. The DWORD case is the one that does not fit in
    // a `usize` on a 32-bit target, which this workspace has: `wasm32-unknown-unknown`.
    let cases: [(&str, Vec<u8>); 3] = [
        ("ID 10, a WORD length", vec![0x10, 0x00, 0x00, 0xFF, 0xFF]),
        (
            "ID 11, a BYTE[3] length",
            vec![
                0x11, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x01, 0x00, 0x08,
                0x00, 0x00, 0xFF, 0xFF, 0xFF,
            ],
        ),
        ("ID 35, a DWORD length", {
            let mut block = vec![0x35_u8];
            block.extend(std::iter::repeat_n(b'x', 16));
            block.extend([0xFF, 0xFF, 0xFF, 0xFF]);
            block
        }),
    ];

    for (name, block) in cases {
        let bytes = file(&[&block]);
        for model in MODELS {
            assert!(
                matches!(tzx::parse(&bytes, model), Err(Error::Truncated { .. })),
                "{name} must be refused as truncated"
            );
        }
    }
}

proptest::proptest! {
    #[test]
    fn arbitrary_bytes_never_panic(bytes: Vec<u8>) {
        // `docs/M6.md` Decision 6's committed substitute for a fuzzer, which needs nightly and
        // a corpus that grows outside the repository. It asserts only that `parse` returns.
        for model in MODELS {
            let _ = tzx::parse(&bytes, model);
        }
    }

    #[test]
    fn arbitrary_bytes_behind_a_valid_header_never_panic(tail: Vec<u8>) {
        // The sharper half: random bytes almost never begin with `ZXTape!`, so the test above
        // spends nearly all its budget on `NotATzxFile` and reaches no block at all. Putting a
        // real header in front is what makes the block dispatch the thing being fuzzed.
        let mut bytes = HEADER.to_vec();
        bytes.extend_from_slice(&tail);
        for model in MODELS {
            let _ = tzx::parse(&bytes, model);
        }
    }

    #[test]
    fn arbitrary_mutations_of_a_valid_file_never_panic(
        indices in proptest::collection::vec(0_usize..160, 1..8),
        values in proptest::collection::vec(0_u8..=255, 1..8),
    ) {
        let mut bytes = representative_file();
        for (&index, &value) in indices.iter().zip(&values) {
            if let Some(byte) = bytes.get_mut(index) {
                *byte = value;
            }
        }
        for model in MODELS {
            let _ = tzx::parse(&bytes, model);
        }
    }
}
