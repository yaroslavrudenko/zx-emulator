//! Which file is which, and what to do with each.
//!
//! Kept apart from the shell so that *"a `.z80` is a snapshot"* is a claim a headless test can
//! check, and so that reading bytes — the one operation a browser does differently — stays in
//! [`crate::host`] and never leaks in here. Nothing in this module performs I/O; every entry
//! point takes bytes that somebody else fetched.

use std::path::Path;

use spectrum::snapshot::{Snapshot, sna, z80};
use spectrum::tape::{tap, tzx};
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
    /// A `.tzx` cassette.
    ///
    /// A separate [`Kind`] from [`Kind::Tape`] and not a second name for it, for the same reason
    /// `.z80` and `.sna` are separate: the two are told apart by their **name**, and the
    /// converters take different arguments — `tzx::parse` needs the [`Model`](spectrum::Model),
    /// because a turbo loader's pulse lengths are counted in T-states and the two machines do not
    /// run at the same rate. Folding them together would mean sniffing the signature, which is a
    /// guess dressed as a fact.
    Tzx,
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
    ("tzx", Kind::Tzx),
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

/// Formats this emulator recognises by name and cannot load yet.
///
/// # A refusal that becomes support by deleting a row
///
/// Without this table a `.tzx` — which is what most commercial games actually ship as, because
/// `.tap` cannot represent a turbo loader at any speed — falls into the generic
/// *"not a .rom, .tap, .z80 or .sna"* message, which reads as *"this emulator is broken"*
/// rather than *"this format is not done yet"*. `docs/STATUS.md`'s standing complaint about
/// silence applies to a user-facing refusal as much as to a gate.
///
/// It is a **table** and not a branch in [`insert`] for one specific reason. `docs/M6.md`
/// Decision 5 chose a pulse train over a block list precisely so that `.tzx` would be a second
/// *converter* rather than a second tape engine, and `crates/spectrum` cashed that in as
/// `tape::tzx::parse`, producing the same `Tape` that `tap::parse` produces.
///
/// # The table is empty, and that is the point rather than a reason to delete it
///
/// `.tzx` landed on 2026-09-01 and its row came out. The mechanism stays: [`unsupported`] is
/// `pub`, [`accept`] still consults it before [`kind_of`], and the next format that is recognised
/// by name and not yet loadable — `.dsk`, `.szx`, `.trd` — becomes a legible refusal by adding
/// one row rather than by rediscovering why the generic message is not good enough.
///
/// **The prediction above was nearly right and it is worth recording that it undercounted.** It
/// said the cost was *"a row to [`EXTENSIONS`], one arm to [`insert`], and delete the row"*. It
/// was five: [`Kind`] gained a variant, [`verb`] needed an arm — which is the wildcard-free match
/// working exactly as designed, refusing to compile rather than picking a verb for a kind nobody
/// had thought about — and the sentence listing what is loadable appears **twice**, here and in
/// [`accept`]. A dispatch that costs three edits and a string that costs two is the honest shape,
/// and the string is the half that no compiler was ever going to catch.
const NOT_YET: &[(&str, &str)] = &[];

/// Why `path` cannot be loaded even though the format is a known one.
///
/// [`None`] means the extension is either loadable — ask [`kind_of`] — or not recognised at all,
/// which are different answers a caller should give different messages for.
#[must_use]
pub fn unsupported(path: &str) -> Option<&'static str> {
    let extension = Path::new(path).extension()?.to_str()?;
    NOT_YET
        .iter()
        .find(|(suffix, _)| suffix.eq_ignore_ascii_case(extension))
        .map(|&(_, reason)| reason)
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
        // The machine's own model, not a constant: `.tzx` speaks in T-states, and a 48K and a
        // 128 do not run at the same rate. `insert_tape` takes `&mut` while `model` takes `&`,
        // which the two-phase borrow allows in this position.
        Kind::Tzx => machine.insert_tape(tzx::parse(bytes, machine.model())?),
        Kind::Z80 => machine.restore(&z80::parse(bytes)?)?,
        Kind::Sna => machine.restore(&sna::parse(bytes)?)?,
        Kind::Rom => return Err(Error::RomAfterStart),
    }
    Ok(())
}

/// Hand `bytes` to the machine under the name `name`, and say what happened in one sentence.
///
/// # This is the one place all four byte sources meet
///
/// A command line, a URL's query string, a payload compiled in by [`crate::bundle`], and a file
/// dropped on the window all end here, with a name and some bytes, and none of them is told
/// which it is. That is the same argument that routes a query string into the `Vec<String>`
/// [`crate::host::partition`] already reads: a second way to decide *"what is this file and
/// what do we do with it"* is a second thing that can disagree.
///
/// # It lives in the library because the decision does
///
/// It was in `src/main.rs`, which that file's own header says is *"the untestable part, and it
/// is kept thin on purpose — everything with a decision in it … is in the library next door and
/// is reachable from `cargo test`."* Which verb applies to which extension, what an unsupported
/// format is told, and what a model mismatch says are all decisions, and none of them was
/// graded while they sat in a binary that needs a window.
///
/// # Saying what happened is the feature
///
/// A file that arrives and does nothing visible is indistinguishable from a broken build, and
/// the sharpest case is the one this shell now has: dropping a tape **appears to do nothing**,
/// because a tape is inserted stopped. [`insert`] says why — the loader would otherwise meet
/// the middle of a block — and that is only defensible if the machine says so. Hence the verb,
/// and hence the key that starts it.
///
/// Three failures are told apart rather than folded together, because they send a person to
/// three different places: a format we know and cannot load yet ([`unsupported`]); a format we
/// do not recognise at all; and a file that parsed and cannot be used.
pub fn accept(machine: &mut Spectrum, name: &str, bytes: &[u8]) -> String {
    if let Some(reason) = unsupported(name) {
        return format!("{name}: {reason}");
    }
    let Some(kind) = kind_of(name) else {
        return format!("{name}: not a .rom, .tap, .tzx, .z80 or .sna");
    };
    match insert(machine, kind, bytes) {
        Ok(()) => format!("{} {name}", verb(kind)),
        // **The library's answer and the frontend's genuinely differ here, and neither is a
        // bug.** [`Spectrum::restore`] refuses a 128 snapshot on a 48K, which is right for a
        // library: silently dropping five banks is the defect `docs/M6.md` refused for
        // duplicate pages, and a library that guesses is worse than one that declines. The
        // frontend's ideal answer is the opposite — build the machine the snapshot describes,
        // since the snapshot carries its own model — and it is **not reachable from here**,
        // because constructing a 128 needs two ROM images this process may never have been
        // given. Reusing the 48K ROM, or guessing which of two is the editor, would be
        // inventing a machine and calling it the user's. So the refusal stands, and the message
        // names the one thing that would fix it.
        Err(error @ Error::Model(_)) => {
            format!("{name}: {error} - restart naming the ROMs that machine needs")
        }
        Err(error) => format!("{name}: {error}"),
    }
}

/// What was done to the machine, in the words its own manual would use.
fn verb(kind: Kind) -> &'static str {
    match kind {
        // The one that must not be silent: [`insert`] deliberately inserts a tape stopped, so
        // the visible effect of dropping a tape is nothing at all until the tape is started.
        Kind::Tape | Kind::Tzx => "tape in the drive, press F3 to play:",
        Kind::Z80 | Kind::Sna => "snapshot restored from",
        // Unreachable at run time — [`insert`] refuses a ROM before returning `Ok` — and named
        // anyway, because that is what makes this match exhaustive.
        Kind::Rom => "built from",
        // **No wildcard arm, deliberately.** [`Kind`] is `#[non_exhaustive]`, which obliges a
        // wildcard in every *other* crate and obliges nothing inside this one — so here the
        // compiler checks that a kind added later has a verb, and a `_ => "loaded"` would be
        // precisely the arm that swallowed it silently. `crates/spectrum/src/keyboard.rs` makes
        // the same trade for `Key::position` and states the same reason.
    }
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
