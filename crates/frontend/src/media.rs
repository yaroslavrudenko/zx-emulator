//! Which file is which, and what to do with each.
//!
//! Kept apart from the shell so that *"a `.z80` is a snapshot"* is a claim a headless test can
//! check, and so that reading bytes — the one operation a browser does differently — stays in
//! [`crate::host`] and never leaks in here. Nothing in this module performs I/O; every entry
//! point takes bytes that somebody else fetched.

use std::path::Path;

use spectrum::snapshot::{Snapshot, sna, z80};
use spectrum::tape::tap;
use spectrum::{RomSizeError, Spectrum};

/// What a file named on the command line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// The 16 KB ROM the machine is built around.
    Rom,
    /// A `.tap` cassette.
    Tape,
    /// A `.z80` snapshot — the only format this emulator can also write.
    Z80,
    /// A `.sna` snapshot.
    Sna,
}

/// The extensions this emulator answers to.
///
/// `.z80` and `.sna` are separate [`Kind`]s rather than one `Snapshot` because the two
/// formats are told apart by their **name**, not their contents. Sniffing would work today —
/// a 48K `.sna` is exactly 49,179 bytes — and would be a guess dressed as a fact; the file
/// name is the thing that actually carries the answer.
const EXTENSIONS: &[(&str, Kind)] = &[
    ("rom", Kind::Rom),
    ("tap", Kind::Tape),
    ("z80", Kind::Z80),
    ("sna", Kind::Sna),
];

/// What `path`'s extension says it is, or `None` for anything else.
#[must_use]
pub fn kind_of(path: &str) -> Option<Kind> {
    let extension = Path::new(path).extension()?.to_str()?;
    EXTENSIONS
        .iter()
        .find(|(suffix, _)| suffix.eq_ignore_ascii_case(extension))
        .map(|&(_, kind)| kind)
}

/// Why a file could not be used.
///
/// Each variant wraps the error the format's own parser produced, through `#[from]`, so the
/// source chain reaches the offset that failed rather than stopping at *"bad snapshot"*.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The ROM was not exactly one 16 KB page.
    #[error("that is not a 16 KB ROM: {0}")]
    Rom(#[from] RomSizeError),

    /// The tape could not be parsed.
    #[error("that tape could not be read: {0}")]
    Tape(#[from] spectrum::tape::Error),

    /// The snapshot could not be parsed.
    #[error("that snapshot could not be read: {0}")]
    Snapshot(#[from] spectrum::snapshot::Error),

    /// The snapshot describes a different machine from the one running.
    ///
    /// A parse that succeeded and a restore that cannot happen are genuinely different
    /// failures, so this is its own variant rather than folded into [`Error::Snapshot`]: the
    /// file is fine and the machine is wrong, and a message saying the snapshot *could not be
    /// read* would send somebody looking for a corrupt file.
    ///
    /// > **This said *"unreachable today"*, and M7 made it reachable.** The reason given was
    /// > that *"[`start`] builds a 48K and both parsers currently produce 48K snapshots"*. The
    /// > first half stopped being true when `start` learned to build a 128 from a ROM pair —
    /// > a 48K snapshot handed to a running 128, or the reverse, is now an ordinary thing for a
    /// > command line to ask for. The variant did not change; the sentence about the world
    /// > around it did, which is the kind of comment `docs/STATUS.md` records as falsified at a
    /// > milestone boundary rather than decaying gradually.
    #[error("{0}")]
    Model(#[from] spectrum::ModelMismatch),

    /// The command line named a number of ROMs no machine is made of.
    ///
    /// One ROM is a 48K and two are a 128; there is no third machine here and no machine with
    /// none. Refused rather than resolved by a rule like *"use the first two"*, because a
    /// person who names three ROM files has made a mistake and the count is the only clue
    /// anything has about which of the three they meant.
    #[error("a machine is built from one ROM (48K) or two (128), not {given}")]
    RomCount {
        /// How many were named.
        given: usize,
    },

    /// A second ROM was named after the machine had already been built.
    ///
    /// Reachable from real input — `zx --rom a.rom b.rom` — and refused rather than ignored,
    /// because silently dropping a file somebody named on the command line is the worse of
    /// the two failures. A ROM is what the machine is made *of*; swapping it means building
    /// a different machine, which is what starting the emulator again does.
    #[error("a ROM cannot be loaded into a machine that is already running")]
    RomAfterStart,
}

/// Build the machine the ROM images describe: one is a 48K, two are a 128.
///
/// # The model is a function of the input, not of which function was called
///
/// This took a single `&[u8]` and built a 48K until M7 made a 128 constructible. The obvious
/// alternative was a second entry point — `start_128(editor, basic)` — and it is refused for the
/// reason `crates/spectrum/src/model.rs` gives about its own enum: *"the alternative to this is
/// three unrelated fields that can disagree with each other"*. Two constructors is the same
/// shape one level up. It would mean two places that decide what a machine is, and the caller
/// choosing between them **before** it has looked at what the user named — so the decision would
/// be duplicated in `zx`, in `zx-shot`, and in anything else that ever grows a command line.
///
/// One function whose behaviour follows the count keeps that decision in one place and makes it
/// checkable from a headless test, which is what `tests/media_dispatch.rs` does.
///
/// **The order of a pair is the order the ROMs page in**, which is the order `0x7FFD` bit 4
/// selects and the order the files are named: `128-0.rom` is the editor and pages in at reset,
/// `128-1.rom` is 48 BASIC. It is not inferred from the filenames — a caller that hands them
/// over the other way round gets a machine that boots 48 BASIC, which is a real configuration
/// and not this crate's business to second-guess.
///
/// # Errors
///
/// [`Error::Rom`] if any image is not exactly one 16 KB page, and [`Error::RomCount`] for a
/// count other than one or two.
pub fn start(roms: &[&[u8]]) -> Result<Spectrum, Error> {
    match roms {
        [rom] => Ok(Spectrum::new(rom)?),
        [editor, basic] => Ok(Spectrum::spectrum_128(editor, basic)?),
        other => Err(Error::RomCount { given: other.len() }),
    }
}

/// Hand `bytes` to a running machine.
///
/// A tape is inserted stopped — the shell's `F3` starts it, exactly as a person presses PLAY
/// after typing `LOAD ""`. Starting it here would mean the tape ran during the seconds the
/// ROM spends booting, and the loader would meet the middle of a block.
///
/// # Errors
///
/// [`Error::Tape`] or [`Error::Snapshot`] if the file is malformed, [`Error::Model`] if the
/// snapshot describes a different machine, [`Error::RomAfterStart`] for a ROM.
pub fn insert(machine: &mut Spectrum, kind: Kind, bytes: &[u8]) -> Result<(), Error> {
    match kind {
        Kind::Tape => machine.insert_tape(tap::parse(bytes)?),
        Kind::Z80 => machine.restore(&z80::parse(bytes)?)?,
        Kind::Sna => machine.restore(&sna::parse(bytes)?)?,
        Kind::Rom => return Err(Error::RomAfterStart),
    }
    Ok(())
}

/// The machine's state as a `.z80`.
///
/// `.z80` and not `.sna` because `crates/spectrum` has no `.sna` writer, and
/// [`spectrum::snapshot::sna`] explains why: the format cannot represent a machine that has
/// not pushed its `PC`, so writing one means corrupting the stack of the machine being saved.
#[must_use]
pub fn save(machine: &Spectrum) -> Vec<u8> {
    let state: Snapshot = machine.snapshot();
    z80::write(&state)
}
