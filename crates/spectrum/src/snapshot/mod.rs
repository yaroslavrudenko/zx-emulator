//! Snapshots: a machine's state as a value, and the two file formats that carry it.
//!
//! ```
//! use spectrum::snapshot::{Error, sna, z80};
//!
//! # fn load(bytes: &[u8]) -> Result<(), Error> {
//! let snapshot = z80::parse(bytes)?;           // or sna::parse
//! let canonical = z80::write(&snapshot);       // always version 3, compressed
//! assert_eq!(z80::parse(&canonical)?, snapshot);
//! # Ok(())
//! # }
//!
//! // Every failure is a returned value. This one needs no file, so it runs here: a
//! // four-byte "snapshot" is refused rather than indexing past the end of a header.
//! assert!(matches!(
//!     z80::parse(&[0x00; 4]),
//!     Err(Error::Truncated { offset: 4, .. })
//! ));
//! assert!(sna::parse(&[]).is_err());
//! ```
//!
//! # Everything here is pure
//!
//! `parse` is a function of its bytes and `write` is a function of a [`Snapshot`]. No
//! filesystem, no machine, no clock — which is what makes the whole module exhaustively
//! testable, fuzzable, and unchanged under WASM at M8. `docs/M6.md` Decision 1: the purity
//! comes from the signature, not from a crate boundary. Applying a [`Snapshot`] to a running
//! machine is the *other* half of M6 and lives with the machine.
//!
//! # The canonical representation is the machine's state, not a file
//!
//! `.z80` version 3 is the richest format and making it the internal representation is
//! tempting for exactly that reason. It is wrong: every quirk then leaks inward — the
//! "byte 12 = 255 means version 1" hack, the `PC = 0` sentinel that marks version 2 and 3,
//! the hardware-mode byte, the four-way split of the T-state counter. The machine would end
//! up storing a file. So there is one native type and the two formats are two lossy
//! encodings of it.
//!
//! **The memory image is keyed by bank, not by address**, and the reason is present-tense
//! rather than a bet on M7. Version 2 and 3 store pages by number, and the 48K's three page
//! numbers are neither contiguous nor derivable from the 128's rule: a 128 file numbers page
//! *N* as RAM bank *N − 3*, while a 48K file uses pages 4, 5 and 8 for `0x8000`, `0xC000`
//! and `0x4000` — banks 2, 0 and 5 in this crate's numbering, of which **only bank 5
//! satisfies *N − 3***. An address-keyed snapshot forces that table to exist twice, once in
//! the parser and once in the writer, with nothing tying them together and a
//! known-irregular rule to get wrong in both.
//!
//! # What no round trip can see
//!
//! `docs/M6.md` Decision 7. Three round trips exist and each one alone is a lie, because
//! every one of them compares a value against a value that came from the same code. A
//! **symmetric** error survives all three: a field permuted with another of the same width,
//! a field read from the wrong offset by both sides, or — the expensive one — a field
//! **dropped in both directions**, where the parser leaves it at its default and the writer
//! always emits that default. The round trip is then green because both sides agreed to
//! lose it. That is the M4 `q`-defaulting defect verbatim, and `docs/STATUS.md`'s finding
//! that *"zero is the one value that makes a positive false claim"* applies here unchanged.
//!
//! Two things break the symmetry and both are built:
//!
//! - **Hand-transcribed vectors**, in `crates/spectrum/tests/snapshot_vectors.rs`: a file
//!   written out byte by byte with each offset's meaning taken from the format description,
//!   and its expected [`Snapshot`] written out separately, field by field. Neither is
//!   derived from the other. `docs/STATUS.md` records what that pattern was worth for the
//!   keyboard: under a review that permuted the matrix, *"38 of the 40 keys moved and the
//!   suite stayed at 72 passed with a green boot gate"*.
//! - **A fixture whose fields are pairwise distinct and none of them zero**, which the test
//!   asserts about itself — otherwise a permutation is invisible even to a hand-written
//!   expectation, and a dropped field looks like a correct default.
//!
//! # What neither format carries
//!
//! [`UNPRESERVED`] names them, and it is a list rather than a sentence so that a test can
//! prove each one is genuinely dropped. A field that quietly joined it would otherwise look
//! exactly like a field that round-trips.

mod reader;
mod rle;

pub mod sna;
pub mod z80;

use std::fmt;

use crate::memory::{BANK_COUNT, BankIndex, PAGE_SIZE};
use crate::model::Model;
use crate::screen::Colour;
use crate::timing::T_STATES_PER_FRAME;

/// A snapshot of one machine offered to another.
///
/// **Both directions are refused, and the symmetry is the point.** A 128 image restored into a
/// 48K has five banks with nowhere to go, and dropping them silently is the *"silent
/// last-write-wins"* `docs/M6.md` refused for duplicate pages. The other direction is broken
/// too and much easier to talk yourself out of: a 48K image carries no paging byte, so
/// restoring one into a 128 leaves 48K code running against the **128 editor ROM** — a machine
/// that looks loaded and executes the wrong ROM.
///
/// A 128 *can* legitimately host a 48K image — that is what the ROM's own **48 BASIC** menu
/// entry does, and it is one paging value. Making [`crate::Spectrum::restore`] do it
/// automatically is deliberately **not** built: it is a second meaning for one method, it needs
/// a rule for what the un-restored five banks contain, and no caller wants it. Anyone who does
/// should add a separate entry point rather than weaken this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a {snapshot} snapshot cannot be restored into a {machine}")]
#[non_exhaustive]
pub struct ModelMismatch {
    /// The machine the snapshot describes.
    pub snapshot: Model,
    /// The machine it was offered to.
    pub machine: Model,
}

/// A field of [`CpuState`](::z80::CpuState) that no snapshot format this crate reads can
/// carry, and what happens to it instead.
///
/// This exists because *"a field dropped in both directions"* is the one defect a round trip
/// cannot see, so the drops are enumerated and each one is asserted to be real by
/// `crates/spectrum/tests/snapshot_vectors.rs`. A field that silently joined this list
/// would look identical to a field that survives.
pub const UNPRESERVED: &[(&str, &str)] = &[
    (
        "wz",
        "MEMPTR. No format names it; the parser sets zero. Observable only through the \
         undocumented flag bits of a `BIT n,(IX+d)` executed before anything else sets it. \
         `.szx` carries it, and M7 is where that would be reconsidered.",
    ),
    (
        "q",
        "The flag latch. Derived at parse from `F` rather than defaulted to zero, which is \
         the ruling `docs/STATUS.md` already made: loading a state is a `POP AF`, so the \
         load is the last thing that wrote `F` and the latch must equal it.",
    ),
    (
        "halted",
        "Neither format has a halt flag. The parser clears it. A snapshot taken on a `HALT` \
         resumes by re-executing the `HALT`, because `PC` holds itself there — which is the \
         same place the machine would have been, one instruction earlier.",
    ),
];

/// Why a snapshot could not be read.
///
/// `Copy` and allocation-free, matching [`RomSizeError`](crate::RomSizeError),
/// [`StepError`](::z80::StepError) and
/// [`InvalidInterruptMode`](::z80::InvalidInterruptMode), the three error types this
/// workspace already has. Every variant names the byte or the offset that failed, because a
/// parse error a user cannot act on is a stack trace with better formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The file ended in the middle of a field.
    #[error("truncated at offset {offset}: {needed} bytes needed, {available} available")]
    Truncated {
        /// Where the field that did not fit begins.
        offset: usize,
        /// How many bytes it needed.
        needed: usize,
        /// How many were left.
        available: usize,
    },

    /// Bytes follow the end of the snapshot.
    #[error("{extra} unread bytes follow the snapshot at offset {offset}")]
    TrailingBytes {
        /// Where the unread bytes begin.
        offset: usize,
        /// How many there are.
        extra: usize,
    },

    /// The `.z80` additional-header length names no version this reads.
    #[error(
        "additional header length {extra_header_len} is not 23 (version 2), 54 or 55 \
         (version 3)"
    )]
    UnsupportedVersion {
        /// The length word at offset 30.
        extra_header_len: u16,
    },

    /// The `.z80` hardware-mode byte names a machine this crate is not.
    #[error("hardware mode {mode} is not a 48K; the 128 modes arrive at M7")]
    UnsupportedHardware {
        /// The mode byte at offset 34.
        mode: u8,
    },

    /// A `.z80` page block names a page a 48K does not have.
    #[error("page {page} is not one a 48K snapshot carries (4, 5 and 8 are)")]
    UnknownPage {
        /// The page number byte.
        page: u8,
    },

    /// A `.z80` page appears more than once.
    #[error("page {page} appears more than once")]
    DuplicatePage {
        /// The page number byte.
        page: u8,
    },

    /// A compressed block expands past the end of the region it fills.
    #[error(
        "the compressed block at offset {offset} expands past the end of its {capacity} \
         bytes"
    )]
    PageOverrun {
        /// Where the block begins.
        offset: usize,
        /// How much room it had.
        capacity: usize,
    },

    /// A compressed block ran out before filling the region.
    ///
    /// Strict rather than zero-filling, deliberately: a zero-filled tail is a wrong machine
    /// that every round trip then agrees is right. An observed real-world file that is
    /// legitimately short is what would change this ruling.
    #[error("the compressed block at offset {offset} filled only {written} of {capacity} bytes")]
    PageUnderrun {
        /// Where the block begins.
        offset: usize,
        /// How much room it had.
        capacity: usize,
        /// How much it filled.
        written: usize,
    },

    /// A version 1 memory block is not followed by `00 ED ED 00`.
    #[error("the version 1 memory block at offset {offset} has no 00 ED ED 00 end marker")]
    MissingEndMarker {
        /// Where the four bytes that should have been the marker begin.
        offset: usize,
    },

    /// The interrupt-mode field names a mode the Z80 does not have.
    #[error("interrupt mode {value} does not exist")]
    InvalidInterruptMode {
        /// The rejected value.
        value: u8,
    },

    /// A `.sna` keeps `PC` on the guest's stack, and this one points outside the RAM image.
    #[error(
        "SP is {sp:#06X}; a .sna keeps PC on the stack, so both bytes must lie in the \
         0x4000-0xFFFF image"
    )]
    StackPointerOutsideRam {
        /// The stack pointer the header held.
        sp: u16,
    },
}

/// Everything a snapshot restores, in the machine's own terms.
///
/// Three fields are public because there is no invariant to protect, following
/// [`CpuState`](::z80::CpuState)'s own precedent — *"every combination of these values is a
/// state a real Z80 can be in"*. [`Colour::new`] wraps into range, and a `frame_t_state` at
/// or above [`T_STATES_PER_FRAME`] is absorbed by the clock's documented rollover, so
/// neither can hold a value that means nothing.
///
/// The banks are private because *which* bank indices are meaningful is a property of the
/// model, and the parser is where that is decided: a `Snapshot` that reaches the machine
/// cannot name a bank the machine lacks.
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The CPU, exactly as [`z80::Cpu::state`](::z80::Cpu::state) reports it.
    pub cpu: ::z80::CpuState,
    /// The colour last written to the border.
    pub border: Colour,
    /// T-states into the frame.
    ///
    /// Values at or above [`T_STATES_PER_FRAME`] roll over when applied, as the clock does
    /// everywhere else. Both parsers reduce into range anyway, so a hostile file cannot put
    /// a number here that means nothing.
    pub frame_t_state: u32,
    /// One page per RAM bank the file carried, `None` for a bank it did not.
    banks: [Option<Box<[u8; PAGE_SIZE]>>; BANK_COUNT],
    /// Which machine this describes.
    ///
    /// Private and read-only from outside, for the same reason `banks` is: which bank indices
    /// are meaningful is a property of the model, so a caller able to relabel a 48K snapshot
    /// as a 128 could hand [`crate::Spectrum::restore`] a value whose bank set and whose claim
    /// disagree — and the refusal that exists to catch exactly that would pass it.
    model: Model,
    /// The `0x7FFD` value the machine stood at, which is its whole memory map.
    ///
    /// Meaningless on a 48K beyond its fixed `0x20`, and the entire arrangement of the eight
    /// banks on a 128 — so a `Snapshot` carrying eight banks without it would describe a
    /// machine it could not restore.
    paging_port: u8,
}

impl Snapshot {
    /// A snapshot with the given CPU, border and frame position, and no memory at all.
    ///
    /// A **48K** unless [`Snapshot::set_model`] says otherwise, which is what both parsers
    /// currently produce and what every caller before M7 meant.
    pub(crate) fn new(cpu: ::z80::CpuState, border: Colour, frame_t_state: u32) -> Self {
        const DEFAULT_MODEL: Model = Model::Spectrum48K;
        Self {
            cpu,
            border,
            frame_t_state,
            banks: [const { None }; BANK_COUNT],
            model: DEFAULT_MODEL,
            paging_port: DEFAULT_MODEL.paging_port_at_reset(),
        }
    }

    /// Which machine this snapshot describes.
    #[must_use]
    pub fn model(&self) -> Model {
        self.model
    }

    /// The `0x7FFD` value the machine stood at.
    pub(crate) fn paging_port(&self) -> u8 {
        self.paging_port
    }

    /// Record which machine this describes, and the memory map it stood in.
    ///
    /// One setter for both, because they are one fact: a model without its paging byte cannot
    /// arrange a 128's banks, and a paging byte without its model cannot be checked against the
    /// machine it is offered to. Splitting them is how a `Snapshot` comes to hold a 128's map
    /// and a 48K's label.
    pub(crate) fn set_model(&mut self, model: Model, paging_port: u8) {
        self.model = model;
        self.paging_port = paging_port;
    }

    /// The contents of `bank`, or `None` if the snapshot does not carry it.
    ///
    /// Public because a failing comparison must be able to say *where*, and because reading
    /// RAM out of a snapshot without starting a machine is what a debugger does.
    #[must_use]
    pub fn bank(&self, bank: BankIndex) -> Option<&[u8; PAGE_SIZE]> {
        self.banks.get(usize::from(bank.get()))?.as_deref()
    }

    /// Store `page` as the contents of `bank`, replacing whatever was there.
    pub(crate) fn set_bank(&mut self, bank: BankIndex, page: Box<[u8; PAGE_SIZE]>) {
        // `BankIndex` is masked into `0..BANK_COUNT` on construction, so this always finds a
        // slot; `get_mut` is used rather than an index so that does not have to be trusted.
        if let Some(slot) = self.banks.get_mut(usize::from(bank.get())) {
            *slot = Some(page);
        }
    }

    /// Every bank the snapshot carries, in ascending bank order.
    ///
    /// This is what the writer emits from and what the applier will restore from — one
    /// iteration order, so a bank cannot be carried by one and missed by the other.
    pub(crate) fn banks(&self) -> impl Iterator<Item = (BankIndex, &[u8; PAGE_SIZE])> {
        self.banks
            .iter()
            .zip(0..)
            .filter_map(|(page, index)| Some((BankIndex::new(index), page.as_deref()?)))
    }
}

impl fmt::Debug for Snapshot {
    /// Deliberately not derived, for the reason [`Memory`](crate::Memory) already had:
    /// *"a derived `Debug` prints 160 KB of page contents, which makes every failing
    /// assertion involving a machine unreadable."*
    ///
    /// A per-bank digest instead, so an `assert_eq!` failure names **which bank** diverged
    /// rather than emitting 96 KB of hex. The digest is a readability aid and not a
    /// comparison: `PartialEq` compares the pages themselves.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let banks: Vec<String> = self
            .banks()
            .map(|(bank, page)| format!("{}:{:016x}", bank.get(), digest(page)))
            .collect();
        f.debug_struct("Snapshot")
            .field("model", &self.model)
            .field("paging_port", &format_args!("{:#04X}", self.paging_port))
            .field("cpu", &self.cpu)
            .field("border", &self.border)
            .field("frame_t_state", &self.frame_t_state)
            .field("banks", &banks)
            .finish()
    }
}

/// FNV-1a over a page, for [`Snapshot`]'s `Debug`.
///
/// A sum would collide between a page and its own permutation, which is precisely the case
/// a failing assertion needs to distinguish.
fn digest(page: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    page.iter().fold(OFFSET_BASIS, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

/// Reduce a file-derived frame position into range.
///
/// Both parsers end here, so `frame_t_state` is a frame position by construction rather than
/// by the file's good behaviour.
const fn frame_position(t_state: u32) -> u32 {
    t_state % T_STATES_PER_FRAME
}

/// Bytes of RAM a 48K holds: `0x4000`–`0xFFFF`.
///
/// Both formats have an address-ordered image of exactly this length — a `.z80` version 1
/// file holds it as one compressed block, and a `.sna` holds it raw after its header.
const IMAGE_LEN_48K: usize = 3 * PAGE_SIZE;

/// The lowest address a 48K's RAM covers. Below it is ROM, which neither format carries.
const RAM_BASE: u16 = 0x4000;

/// One of the three RAM banks a 48K snapshot carries, and how each format names it.
struct Bank48K {
    /// The `.z80` version 2/3 page number this bank is stored under.
    ///
    /// A `.sna` has no page numbers — it is address-ordered — so this field is `.z80`'s
    /// alone. It lives in the same table anyway, because "which 16 KB is which bank, and
    /// what each format calls it" is *one* piece of knowledge and splitting it across two
    /// tables is how the two halves come to disagree.
    page: u8,
    /// The bank number in this crate's numbering — [`crate::memory`]'s, not the file's.
    bank: u8,
}

/// The three banks a 48K carries, in the **address order** both formats store them in.
///
/// Transcribed from the format description and never computed, because this is exactly the
/// shape that invites a formula. The 128's rule is *page N is bank N − 3*, and it is right
/// for **one** of these three entries: page 8 is bank 5, but page 4 is bank 2 (not 1) and
/// page 5 is bank 0 (not 2). A formula that is right a third of the time passes any test
/// built from itself, which is why this is a table and why
/// `the_bank_set_matches_what_the_48k_slot_map_exposes` compares it against the machine
/// rather than against itself.
///
/// From the format description: *"In 48K mode, pages 4,5 and 8 are saved"*, with page 4 at
/// `8000-bfff`, page 5 at `c000-ffff` and page 8 at `4000-7fff`.
const BANKS_48K: [Bank48K; 3] = [
    // 0x4000-0x7FFF — the screen bank, and the only contended one.
    Bank48K { page: 8, bank: 5 },
    // 0x8000-0xBFFF
    Bank48K { page: 4, bank: 2 },
    // 0xC000-0xFFFF
    Bank48K { page: 5, bank: 0 },
];

/// The bank a `.z80` version 2/3 page number names, or `None` if a 48K has no such page.
fn bank_for_page(page: u8) -> Option<BankIndex> {
    BANKS_48K
        .iter()
        .find(|entry| entry.page == page)
        .map(|entry| BankIndex::new(entry.bank))
}

/// Store an address-ordered 48 KB image into `snapshot`, bank by bank.
///
/// One splitter for both formats, so a `.z80` version 1 file and a `.sna` cannot come to
/// disagree about which 16 KB is which bank — which is the whole reason `BANKS_48K` is
/// shared rather than transcribed twice.
///
/// A short image stores only the banks it covers, because `as_chunks` yields only whole
/// pages and leaves any remainder aside; both callers hand over exactly [`IMAGE_LEN_48K`]
/// bytes, so there is never a remainder in practice.
fn store_image(snapshot: &mut Snapshot, image: &[u8]) {
    let (pages, _remainder) = image.as_chunks::<PAGE_SIZE>();
    for (entry, page) in BANKS_48K.iter().zip(pages) {
        snapshot.set_bank(BankIndex::new(entry.bank), Box::new(*page));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every production source file of this module.
    ///
    /// Listed rather than globbed because a file that quietly stopped being scanned would
    /// be indistinguishable from a file with nothing to find.
    const SOURCES: [(&str, &str); 5] = [
        ("snapshot/mod.rs", include_str!("mod.rs")),
        ("snapshot/reader.rs", include_str!("reader.rs")),
        ("snapshot/rle.rs", include_str!("rle.rs")),
        ("snapshot/z80.rs", include_str!("z80.rs")),
        ("snapshot/sna.rs", include_str!("sna.rs")),
    ];

    /// The production half of `source` — everything above its `#[cfg(test)]` module.
    fn production(source: &str) -> &str {
        source
            .split_once("\n#[cfg(test)]")
            .map_or(source, |(head, _)| head)
    }

    /// Lines holding an index expression, as `(line number, line)`.
    ///
    /// An index expression is a `[` **immediately** preceded by an identifier character, a
    /// `)` or a `]` — which is what `a[i]`, `f()[i]` and `a[i][j]` look like and what
    /// `[u8; N]`, `&[u8]`, `Box<[u8; N]>` and `#[derive(..)]` do not. Comments are stripped
    /// first, so a doc link like ``[`Reader::take`]`` cannot be mistaken for one.
    fn indexing_sites(source: &str) -> Vec<(usize, String)> {
        source
            .lines()
            .enumerate()
            .filter_map(|(number, line)| {
                let code = line.split("//").next().unwrap_or(line);
                let bytes: Vec<char> = code.chars().collect();
                let indexed = bytes.windows(2).any(|pair| {
                    matches!(pair, [before, '['] if before.is_alphanumeric()
                        || *before == '_'
                        || *before == ')'
                        || *before == ']')
                });
                indexed.then(|| (number + 1, code.trim().to_owned()))
            })
            .collect()
    }

    #[test]
    fn the_indexing_scanner_can_tell_an_index_from_an_array_type() {
        // The gate below is only worth running if this function distinguishes the two, so
        // it has its own failing cases. Without them it would be a scanner that finds
        // nothing, asserting that nothing is there.
        for indexing in [
            "self.banks[index]",
            "let x = bytes[..2];",
            "value.to_le_bytes()[0]",
            "grid[y][x] = 1;",
            "PAGE[0]",
        ] {
            assert_eq!(
                indexing_sites(indexing).len(),
                1,
                "{indexing:?} is an index expression"
            );
        }
        for innocent in [
            "fn f(bytes: &[u8]) -> [u8; 2] {",
            "banks: [Option<Box<[u8; PAGE_SIZE]>>; BANK_COUNT],",
            "#[derive(Debug, Clone, Copy)]",
            "encoded.extend_from_slice(&[ESCAPE, ESCAPE, count, byte]);",
            "const SLOTS: [Slot; 4] = [Slot::Rom, Slot::Bank];",
            "// self.banks[index] in a comment",
            "/// A doc link to [`Reader::take`] and a page[0] mention",
        ] {
            assert_eq!(
                indexing_sites(innocent),
                Vec::<(usize, String)>::new(),
                "{innocent:?} is not an index expression"
            );
        }
    }

    #[test]
    fn there_is_no_indexing_anywhere_in_the_snapshot_module() {
        // `docs/M6.md` Decision 6, as a property of the source rather than a sentence in a
        // doc comment. Slice indexing is one of the three panic sources a hostile file can
        // reach in safe Rust, and this module closes it by not containing the construct:
        // every byte moves through `Reader` or `Writer`, whose four slice operations are
        // total by signature.
        //
        // Structural impossibility beats a passing test, so this asserts the structure. The
        // truncation sweep and the property test in `tests/snapshot_hostile.rs` assert the
        // behaviour, and the two are not substitutes for each other.
        for (name, source) in SOURCES {
            assert_eq!(
                indexing_sites(production(source)),
                Vec::<(usize, String)>::new(),
                "{name} indexes a slice; route it through Reader or Writer instead"
            );
        }
    }

    #[test]
    fn nothing_in_the_snapshot_module_can_panic_on_purpose() {
        // The one item on Decision 6's list that a grep enforces rather than a type. The
        // production half only: tests unwrap freely, and that is what tests are for.
        const FORBIDDEN: [&str; 6] = [
            ".unwrap()",
            ".expect(",
            "panic!(",
            "todo!(",
            "unimplemented!(",
            "unreachable!(",
        ];
        for (name, source) in SOURCES {
            let code = production(source);
            for (number, line) in code.lines().enumerate() {
                let statement = line.split("//").next().unwrap_or(line);
                for forbidden in FORBIDDEN {
                    assert!(
                        !statement.contains(forbidden),
                        "{name}:{} uses {forbidden}: {}",
                        number + 1,
                        statement.trim()
                    );
                }
            }
        }
    }

    #[test]
    fn every_unpreserved_field_is_named_once_and_explained() {
        let names: Vec<&str> = UNPRESERVED.iter().map(|&(name, _)| name).collect();
        assert_eq!(
            names,
            ["wz", "q", "halted"],
            "the list is what `tests/snapshot_vectors.rs` proves; changing it changes what \
             the round trip is allowed to be blind to"
        );
        for &(name, why) in UNPRESERVED {
            assert!(why.len() > 40, "{name} needs a reason, not a label");
        }
    }

    #[test]
    fn a_frame_position_is_always_inside_a_frame() {
        assert_eq!(frame_position(0), 0);
        assert_eq!(
            frame_position(T_STATES_PER_FRAME - 1),
            T_STATES_PER_FRAME - 1
        );
        assert_eq!(frame_position(T_STATES_PER_FRAME), 0);
        assert_eq!(frame_position(u32::MAX), u32::MAX % T_STATES_PER_FRAME);
    }

    #[test]
    fn banks_go_in_and_come_back_out_by_index() {
        let mut snapshot = Snapshot::new(::z80::CpuState::default(), Colour::new(3), 7);
        assert_eq!(snapshot.bank(BankIndex::new(5)), None);
        assert_eq!(snapshot.banks().count(), 0);

        snapshot.set_bank(BankIndex::new(5), Box::new([0xA5; PAGE_SIZE]));
        snapshot.set_bank(BankIndex::new(0), Box::new([0x5A; PAGE_SIZE]));

        assert_eq!(snapshot.bank(BankIndex::new(5)).map(|p| p[0]), Some(0xA5));
        assert_eq!(snapshot.bank(BankIndex::new(0)).map(|p| p[0]), Some(0x5A));
        assert_eq!(snapshot.bank(BankIndex::new(2)), None);
        let seen: Vec<u8> = snapshot.banks().map(|(bank, _)| bank.get()).collect();
        assert_eq!(
            seen,
            [0, 5],
            "ascending bank order, so the two consumers agree"
        );
    }

    #[test]
    fn debug_names_which_bank_diverged_without_printing_it() {
        let mut snapshot = Snapshot::new(::z80::CpuState::default(), Colour::BLACK, 0);
        snapshot.set_bank(BankIndex::new(5), Box::new([0x00; PAGE_SIZE]));
        let rendered = format!("{snapshot:?}");
        assert!(
            rendered.len() < 900,
            "Debug printed {} bytes",
            rendered.len()
        );
        assert!(rendered.contains("frame_t_state"));
        assert!(rendered.contains("\"5:"), "the bank number has to appear");
    }

    #[test]
    fn the_bank_set_matches_what_the_48k_slot_map_exposes() {
        // The seam `docs/M6.md` names: a bank present in a snapshot and absent from the slot
        // map would be silently dropped when it is applied. Asserted against the machine
        // rather than reasoned about, and it goes red the moment either side moves.
        use crate::memory::{Memory, Slot};

        let memory = Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM");
        let mut exposed: Vec<u8> = memory
            .slots()
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Bank(bank) => Some(bank.get()),
                Slot::Rom(_) => None,
            })
            .collect();
        let mut carried: Vec<u8> = BANKS_48K.iter().map(|entry| entry.bank).collect();
        exposed.sort_unstable();
        carried.sort_unstable();
        assert_eq!(carried, exposed, "the parser and the machine must agree");
    }

    #[test]
    fn the_page_numbers_are_the_ones_the_format_description_names() {
        // Transcribed: "In 48K mode, pages 4,5 and 8 are saved", page 4 at 8000-bfff,
        // page 5 at c000-ffff, page 8 at 4000-7fff. `BANKS_48K` is in address order, so the
        // page numbers in that order are 8, 4, 5 — which is *not* ascending, and that is the
        // point: the mapping has no formula.
        let pages: Vec<u8> = BANKS_48K.iter().map(|entry| entry.page).collect();
        assert_eq!(pages, [8, 4, 5]);
        assert_eq!(bank_for_page(8).map(BankIndex::get), Some(5));
        assert_eq!(bank_for_page(4).map(BankIndex::get), Some(2));
        assert_eq!(bank_for_page(5).map(BankIndex::get), Some(0));
        for absent in [0, 1, 2, 3, 6, 7, 9, 10, 11, 255] {
            assert_eq!(bank_for_page(absent), None, "page {absent}");
        }
    }

    #[test]
    fn only_one_of_the_three_pages_obeys_the_hundred_and_twenty_eights_rule() {
        // Stated as an assertion because it is the trap: `page = bank + 3` is right for the
        // 128 and for exactly one of these three, and a formula that is right a third of the
        // time passes any test built from itself.
        let obedient = BANKS_48K
            .iter()
            .filter(|entry| entry.page == entry.bank + 3)
            .count();
        assert_eq!(obedient, 1, "only page 8 / bank 5 satisfies N - 3");
    }

    #[test]
    fn an_address_ordered_image_lands_in_address_order() {
        // The splitter both formats share. A `.z80` version 1 block and a `.sna` image are
        // the same bytes in the same order, so this is the one place the order is decided.
        let mut image = vec![0_u8; IMAGE_LEN_48K];
        for (region, value) in [(0_usize, 0xA1_u8), (1, 0xB2), (2, 0xC3)] {
            if let Some(byte) = image.get_mut(region * PAGE_SIZE) {
                *byte = value;
            }
        }
        let mut snapshot = Snapshot::new(::z80::CpuState::default(), Colour::BLACK, 0);
        store_image(&mut snapshot, &image);
        assert_eq!(snapshot.bank(BankIndex::new(5)).map(|p| p[0]), Some(0xA1));
        assert_eq!(snapshot.bank(BankIndex::new(2)).map(|p| p[0]), Some(0xB2));
        assert_eq!(snapshot.bank(BankIndex::new(0)).map(|p| p[0]), Some(0xC3));
    }

    #[test]
    fn the_digest_separates_a_page_from_its_own_permutation() {
        // Which a sum would not, and naming the wrong bank in a failure message is worse
        // than naming none.
        assert_ne!(digest(&[1, 2, 3]), digest(&[3, 2, 1]));
        assert_ne!(digest(&[0; 16]), digest(&[0; 17]));
    }
}
