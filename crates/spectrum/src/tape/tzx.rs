//! `.tzx` — the format that *can* carry a turbo loader, converted into a pulse train.
//!
//! # Why this is a converter and not a rewrite
//!
//! `docs/M6.md` Decision 5 chose a pulse train as the tape's internal form **for this**. A
//! block-list internal form would have made `.tzx` a rewrite of the tape subsystem; a pulse
//! train makes it a second converter with the [`Ula`](crate::Ula) side untouched. Nothing in
//! [`Tape`] changed to accommodate this module, which is the decision's payoff and the only
//! hard evidence in it.
//!
//! `.tap` is block data with the ROM's standard timings *implied* — nothing in it can say
//! *"this loader uses 700-T-state bits"* — so no `.tap` can carry a turbo loader at any speed,
//! and most commercial titles are turbo-loaded. `.tzx` carries per-block pilot, sync and bit
//! timings, which is precisely what `.tap` cannot express.
//!
//! # Every number here is transcribed from the format description
//!
//! The revision is **1.20, 19 Dec 2006** — the current one. Offsets, lengths and defaults below
//! are transcribed from it field by field and cross-checked against a second copy of the same
//! document; where the description contradicts itself, the contradiction is named at the
//! constant rather than resolved silently. `docs/M6.md` opened with a warning about its own
//! numbers and three of them were later found wrong, one of which *"a round trip could never
//! have caught"* — so nothing here is written from familiarity.
//!
//! Two of the description's own rules are load-bearing and easy to miss:
//!
//! - **A pause of zero duration is *completely ignored*** — *"the 'current pulse level' will NOT
//!   change in this case. This also applies to 'Data' blocks that have some pause duration
//!   included in them."* An emulator that emitted a zero-length silence instead would put a
//!   spurious edge between every such block and the next.
//! - **The level after a direct recording is the *last sample*, not its opposite** — where after
//!   every other signal block it *is* the opposite, *"so that a subsequent pulse will produce an
//!   edge"*. The two rules are one sentence apart in the description and they disagree on
//!   purpose. [`Signal::direct`](super::signal::Signal::direct) is where that is honoured.
//!
//! # What this module refuses, and why refusing beats skipping
//!
//! A metadata block is skippable, but **skipping requires knowing the block's length**, and a
//! block whose length cannot be determined is unskippable — that is the difference between a
//! parser that degrades and one that silently produces a wrong train. So:
//!
//! | Block | Verdict |
//! |---|---|
//! | `0x16`, `0x17` — the deprecated C64 blocks | **refused.** The description's length column says the body is the `DWORD` at offset 0, and the field description calls the same `DWORD` *"(extension rule)"*, which by the description's own definition **excludes** those four bytes. The two readings differ by four and nothing here can adjudicate them |
//! | `0x18` — CSW recording | **refused.** Its length *is* determinable, but its payload is a CSW v2 stream, optionally Z-RLE (deflate) compressed. Skipping it would drop signal silently |
//! | `0x19` — generalized data | **refused**, for the same reason: it carries signal, and playing it means a symbol-alphabet machine that is its own job |
//! | any other unrecognised ID | **refused.** Every ID the description defines is handled here, so an unknown one is either newer than 1.20 or not a `.tzx` at all |
//!
//! What would change any of those: an observed real file that is refused. That is the same
//! escape hatch `docs/M6.md` attaches to the snapshot parsers' strict rulings.
//!
//! # The two ceilings, and why one of them is not structural
//!
//! `docs/M6.md` Decision 6 requires that **no allocation is ever sized from the file** and that
//! every loop terminate. `.tzx` is the first parser here where neither is free:
//!
//! - **A loop block multiplies.** Three bytes can ask for a body to be replayed 65535 times,
//!   and a `Tape` is materialised — so a loop count times a block length is exactly an
//!   allocation sized from the file. The bound is
//!   [`MAX_PULSES`](super::signal::MAX_PULSES), it is checked on every push, and exceeding it
//!   is [`Error::TapeTooLong`] rather than a large allocation.
//! - **A jump block revisits.** *"Jump 0 = 'Loop Forever' - this should never happen"*, says the
//!   description, of a file it also permits. Progress is therefore **not** structural here, and
//!   that is the honest difference from [`crate::snapshot`], where every loop iteration consumes
//!   an input byte. It is a budget — [`MAX_BLOCKS_PLAYED`] — and reaching it is
//!   [`Error::TooManyBlocksPlayed`].
//!
//! The block **scan** is structural in the old sense: every iteration consumes at least the
//! block's own ID byte, so it terminates, and the index it builds holds one entry per block
//! present in the file. That is linear in the input's own length rather than derived from a
//! length field, which is the distinction Decision 6 is drawing.

use super::reader::Reader;
use super::signal::{Data, MAX_PULSES, Samples, Signal, SpeedData, UsedBits};
use super::{Error, Tape, tap};
use crate::Model;

// ---------------------------------------------------------------------------------------
// The header — transcribed from "TZX Header", length 10 bytes
// ---------------------------------------------------------------------------------------

/// Offset `0x00`, `ASCII[7]`: the signature.
const SIGNATURE: [u8; 7] = *b"ZXTape!";

/// Offset `0x07`, `BYTE`: *"End of text file marker"*, 26 decimal.
const END_OF_TEXT: u8 = 0x1A;

/// The only major revision this converter claims to handle.
///
/// *"To be able to use a TZX file, your program must be able to handle files of at least its
/// major version number."* The minor number is deliberately not checked: the same sentence
/// requires a 1.05-capable program to accept a 1.06 file *"even if it cannot handle all the
/// data in the file"*, and an unhandled block is refused by ID rather than by version.
const SUPPORTED_MAJOR: u8 = 1;

/// Bytes before the first block: the signature, the marker, and the two revision numbers.
const HEADER_LENGTH: usize = SIGNATURE.len() + 3;

const _: () = assert!(HEADER_LENGTH == 10);

// ---------------------------------------------------------------------------------------
// Block IDs — transcribed from the "TZX block ID list"
// ---------------------------------------------------------------------------------------

/// `ID 10 - Standard Speed Data Block`, body `[02,03]+04`.
const STANDARD_SPEED_DATA: u8 = 0x10;
/// `ID 11 - Turbo Speed Data Block`, body `[0F,10,11]+12`.
const TURBO_SPEED_DATA: u8 = 0x11;
/// `ID 12 - Pure Tone`, body `04`.
const PURE_TONE: u8 = 0x12;
/// `ID 13 - Pulse sequence`, body `[00]*02+01`.
const PULSE_SEQUENCE: u8 = 0x13;
/// `ID 14 - Pure Data Block`, body `[07,08,09]+0A`.
const PURE_DATA: u8 = 0x14;
/// `ID 15 - Direct Recording`, body `[05,06,07]+08`.
const DIRECT_RECORDING: u8 = 0x15;
/// `ID 16 - C64 ROM Type Data Block` — deprecated at 1.20, and its length is ambiguous.
const C64_ROM_DATA: u8 = 0x16;
/// `ID 17 - C64 Turbo Tape Data Block` — deprecated at 1.20, and its length is ambiguous.
const C64_TURBO_DATA: u8 = 0x17;
/// `ID 18 - CSW Recording`, body `[00,01,02,03]+04`. Length known; payload not decoded.
const CSW_RECORDING: u8 = 0x18;
/// `ID 19 - Generalized Data Block`, body `[00,01,02,03]+04`. Length known; not played.
const GENERALIZED_DATA: u8 = 0x19;
/// `ID 20 - Pause (silence) or 'Stop the Tape' command`, body `02`.
const PAUSE_OR_STOP: u8 = 0x20;
/// `ID 21 - Group start`, body `[00]+01`.
const GROUP_START: u8 = 0x21;
/// `ID 22 - Group end`, body `00`.
const GROUP_END: u8 = 0x22;
/// `ID 23 - Jump to block`, body `02`.
const JUMP_TO_BLOCK: u8 = 0x23;
/// `ID 24 - Loop start`, body `02`.
const LOOP_START: u8 = 0x24;
/// `ID 25 - Loop end`, body `00`.
const LOOP_END: u8 = 0x25;
/// `ID 26 - Call sequence`, body `[00,01]*02+02`.
const CALL_SEQUENCE: u8 = 0x26;
/// `ID 27 - Return from sequence`, body `00`.
const RETURN_FROM_SEQUENCE: u8 = 0x27;
/// `ID 28 - Select block`, body `[00,01]+02`.
const SELECT_BLOCK: u8 = 0x28;
/// `ID 2A - Stop the tape if in 48K mode`, body `04`.
const STOP_TAPE_IN_48K: u8 = 0x2A;
/// `ID 2B - Set signal level`, body `05`.
const SET_SIGNAL_LEVEL: u8 = 0x2B;
/// `ID 30 - Text description`, body `[00]+01`.
const TEXT_DESCRIPTION: u8 = 0x30;
/// `ID 31 - Message block`, body `[01]+02`.
const MESSAGE: u8 = 0x31;
/// `ID 32 - Archive info`, body `[00,01]+02`.
const ARCHIVE_INFO: u8 = 0x32;
/// `ID 33 - Hardware type`, body `[00]*03+01`.
const HARDWARE_TYPE: u8 = 0x33;
/// `ID 34 - Emulation info`, body `08` — deprecated at 1.20, fixed length, skippable.
const EMULATION_INFO: u8 = 0x34;
/// `ID 35 - Custom info block`, body `[10,11,12,13]+14`.
const CUSTOM_INFO: u8 = 0x35;
/// `ID 40 - Snapshot block`, body `[01,02,03]+04` — deprecated at 1.20, skippable.
const SNAPSHOT: u8 = 0x40;
/// `ID 5A - "Glue" block`, body `09` — *"Just skip these 9 bytes"*.
const GLUE: u8 = 0x5A;

// ---------------------------------------------------------------------------------------
// Fixed body sizes and prefix offsets, transcribed field by field
// ---------------------------------------------------------------------------------------

/// `ID 10`: `0x00` pause, `0x02` length — so the length word is two bytes in, and the fixed
/// part is the four bytes before the data.
const STANDARD_LENGTH_AT: usize = 0x02;
const STANDARD_FIXED: usize = 0x04;

/// `ID 11`: `0x0F` is the `BYTE[3]` length and `0x12` is the data.
const TURBO_LENGTH_AT: usize = 0x0F;
const TURBO_FIXED: usize = 0x12;

/// `ID 14`: `0x07` is the `BYTE[3]` length and `0x0A` is the data.
const PURE_DATA_LENGTH_AT: usize = 0x07;
const PURE_DATA_FIXED: usize = 0x0A;

/// `ID 15`: `0x05` is the `BYTE[3]` length and `0x08` is the samples.
const DIRECT_LENGTH_AT: usize = 0x05;
const DIRECT_FIXED: usize = 0x08;

/// `ID 35`: the identification string is `CHAR[10]` and the `DWORD` that follows it sits at
/// offset `0x10`, so the string is **sixteen** bytes and not ten.
///
/// The description's array notation is decimal elsewhere — `BYTE[3]` really is three bytes,
/// which its own offsets confirm — so this one entry is written in hex. Two independent numbers
/// in the same table settle it: the `DWORD`'s offset `0x10`, and the length column's `+14`,
/// which is `0x10 + 4`. It is transcribed as sixteen on the strength of those two rather than
/// on the strength of the array's spelling.
const CUSTOM_INFO_ID_LENGTH: usize = 0x10;
const CUSTOM_INFO_FIXED: usize = 0x14;

const _: () = assert!(CUSTOM_INFO_FIXED == CUSTOM_INFO_ID_LENGTH + 4);

/// `ID 31`: the text length is the second byte, after the display time.
const MESSAGE_LENGTH_AT: usize = 0x01;

/// `ID 40`: the snapshot's `BYTE[3]` length follows the one-byte type.
const SNAPSHOT_LENGTH_AT: usize = 0x01;

/// Body lengths that are a constant, straight from the description's length column.
const PURE_TONE_BODY: usize = 0x04;
const PAUSE_BODY: usize = 0x02;
const JUMP_BODY: usize = 0x02;
const LOOP_START_BODY: usize = 0x02;
const EMPTY_BODY: usize = 0x00;
const EMULATION_INFO_BODY: usize = 0x08;
const GLUE_BODY: usize = 0x09;

/// The `DWORD` that opens every block added after revision 1.10.
///
/// *"ALL custom blocks that will be added after version 1.10 will have the length of the block
/// in first 4 bytes (long word) after the ID (this length does not include these 4 length
/// bytes)."* `ID 18`, `ID 19`, `ID 2A` and `ID 2B` follow it, and reading their bodies as
/// `4 + L` reproduces the fixed lengths their own length columns give — `04` for `ID 2A` at
/// `L = 0` and `05` for `ID 2B` at `L = 1` — which is what makes the general reading safe to
/// apply to them rather than a second transcription to get wrong.
const EXTENSION_LENGTH_BYTES: usize = 4;

// ---------------------------------------------------------------------------------------
// Timings
// ---------------------------------------------------------------------------------------

/// T-states in one millisecond, which is the unit every `.tzx` pause is written in.
///
/// *"The timings are given in Z80 clock ticks (T states) unless otherwise stated. 1 T state =
/// (1/3500000)s"*, and *"The ZXTape format has ALL timings written according to a 3.5MHz clock
/// (the standard Spectrum 16/48K clock)"*.
///
/// So this is the format's own clock and not the machine's frame rate. It is deliberately
/// **not** derived from [`T_STATES_PER_FRAME`](crate::timing::T_STATES_PER_FRAME) the way
/// [`tap`]'s inter-block gap is: a `.tap` carries no durations at all, so its gap is ours to
/// choose and is worth expressing in the machine's own terms; a `.tzx` pause is a number in the
/// file, written against 3.5 MHz by whoever recorded it.
const T_STATES_PER_MILLISECOND: u32 = 3500;

/// The opening stretch of a pause, held at the level the last edge left behind.
///
/// *"To ensure that the last edge produced is properly finished there should be at least 1 ms.
/// pause of the opposite level and only after that the pulse should go to 'low'."*
const PAUSE_SETTLE_MILLISECONDS: u32 = 1;

/// The line's resting level, which is where a pause leaves it.
///
/// *"At the end of a 'Pause' block the 'current pulse level' is low"*, and *"An emulator should
/// put the 'current pulse level' to 'low' when starting to play a TZX file"* — which is the
/// state [`Tape::new`] already starts in.
const LOW: bool = false;

/// The largest pause a `.tzx` can ask for, in T-states, proving the arithmetic cannot overflow.
const _: () = assert!((u16::MAX as u64) * (T_STATES_PER_MILLISECOND as u64) < u32::MAX as u64);

// ---------------------------------------------------------------------------------------
// Ceilings
// ---------------------------------------------------------------------------------------

/// How many block executions a file may ask for before the conversion is refused.
///
/// **This bounds termination, not memory** — [`MAX_PULSES`](super::signal::MAX_PULSES) bounds
/// memory. The two are separate because a block can be executed without emitting anything: a
/// jump to itself, or a loop over a group start and end pair, spins forever while allocating
/// nothing, so the pulse ceiling would never fire.
///
/// **It is the pulse ceiling, and that equality is derived from both sides.** Below it, this
/// budget could refuse a file the pulse ceiling permits — a loop body of sixteen blocks each
/// emitting one half-period stays well inside the tape's size limit while executing over a
/// million blocks, and refusing that would be this bound overruling the one that was actually
/// reasoned about. Above it, it buys nothing: a file can only exceed the pulse ceiling's worth
/// of executions by being almost entirely silent, since any block that emits counts against the
/// other bound too. So the two meet exactly, and neither is a number picked to look round.
const MAX_BLOCKS_PLAYED: usize = MAX_PULSES;

// ---------------------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------------------

/// Read a `.tzx` file into the signal it describes.
///
/// `model` decides one block: `ID 2A` stops the tape **only** on a 48K, and this machine has
/// both models. Getting it wrong is not symmetric — wrongly continuing past a stop is mostly
/// harmless, because the loader has stopped listening by then, while wrongly stopping truncates
/// a 128 multiload at its first level — so the machine is an input to the conversion rather than
/// a default to pick.
///
/// # Errors
///
/// [`Error`], naming the offset that failed. Every failure is a returned value: this function
/// does not panic on any input, which is a property of its construction rather than of the
/// inputs it has seen. There is no indexing expression in `tape/`, every quantity derived from
/// the file is widened or refused rather than wrapped, and the two ceilings above make an
/// oversized or non-terminating file an [`Error`] rather than an allocation or a hang.
pub fn parse(bytes: &[u8], model: Model) -> Result<Tape, Error> {
    let blocks = scan(bytes)?;
    let pulses = Player::new(&blocks, model).run()?;
    Ok(Tape::new(pulses))
}

/// One block, located: its ID, where it starts, and exactly the bytes the format says it owns.
struct Block<'a> {
    /// The ID byte.
    id: u8,
    /// Absolute file offset of the ID byte.
    offset: usize,
    /// The bytes after the ID byte, as long as the format says the block is.
    body: &'a [u8],
}

impl Block<'_> {
    /// Absolute file offset of the first body byte.
    const fn body_offset(&self) -> usize {
        self.offset.saturating_add(1)
    }

    /// A cursor over this block's body, reporting absolute file offsets.
    const fn reader(&self) -> Reader<'_> {
        Reader::at(self.body, self.body_offset())
    }
}

/// Split the file into blocks, determining every one's length.
///
/// This is the parse-don't-validate boundary for the file's **structure**: either every block's
/// extent is known, or the file is refused. Nothing downstream has to wonder where a block ends.
///
/// Termination is structural — each iteration consumes at least the ID byte — and the index is
/// bounded by the file's own length rather than by a length field, since a block cannot be
/// shorter than its ID.
fn scan(bytes: &[u8]) -> Result<Vec<Block<'_>>, Error> {
    let mut reader = Reader::at(bytes, 0);

    let signature = reader.take(SIGNATURE.len())?;
    let marker = reader.u8()?;
    if signature != SIGNATURE || marker != END_OF_TEXT {
        return Err(Error::NotATzxFile);
    }
    let major = reader.u8()?;
    let minor = reader.u8()?;
    if major != SUPPORTED_MAJOR {
        return Err(Error::UnsupportedVersion { major, minor });
    }

    let mut blocks = Vec::new();
    while !reader.is_empty() {
        let offset = reader.offset();
        let id = reader.u8()?;
        let length = measure(id, &mut reader.body_probe(), offset)?;
        let body = reader.take(length)?;
        blocks.push(Block { id, offset, body });
    }
    Ok(blocks)
}

impl<'a> Reader<'a> {
    /// A throwaway cursor over what is left, for measuring a block before taking it.
    fn body_probe(&self) -> Self {
        Self::at(self.rest(), self.offset())
    }
}

/// A body length: a fixed part plus a length the file supplied.
///
/// Saturating rather than checked, and that is not a shortcut. [`Reader::take`] refuses any
/// count larger than the bytes that remain, and `usize::MAX` is larger than any slice, so a
/// length too large to represent is refused by the same comparison that refuses one that is
/// merely too big — and the error still names the offset. This is reachable rather than
/// theoretical: `usize` is 32 bits on `wasm32-unknown-unknown`, which this workspace targets,
/// and `ID 18`, `ID 19` and `ID 35` all carry a full `DWORD` length.
fn body_length(fixed: usize, declared: u32) -> usize {
    usize::try_from(declared)
        .unwrap_or(usize::MAX)
        .saturating_add(fixed)
}

/// How long the body of a block with this `id` is, measured from `probe`.
///
/// `offset` names the ID byte, so a refusal points at the block rather than at its body.
fn measure(id: u8, probe: &mut Reader<'_>, offset: usize) -> Result<usize, Error> {
    let length = match id {
        STANDARD_SPEED_DATA => {
            probe.skip(STANDARD_LENGTH_AT)?;
            body_length(STANDARD_FIXED, u32::from(probe.u16_le()?))
        }
        TURBO_SPEED_DATA => {
            probe.skip(TURBO_LENGTH_AT)?;
            body_length(TURBO_FIXED, probe.u24_le()?)
        }
        PURE_TONE => PURE_TONE_BODY,
        // "Up to 255 pulses can be stored in this block": a count byte, then that many WORDs.
        PULSE_SEQUENCE => body_length(1, u32::from(probe.u8()?) * 2),
        PURE_DATA => {
            probe.skip(PURE_DATA_LENGTH_AT)?;
            body_length(PURE_DATA_FIXED, probe.u24_le()?)
        }
        DIRECT_RECORDING => {
            probe.skip(DIRECT_LENGTH_AT)?;
            body_length(DIRECT_FIXED, probe.u24_le()?)
        }
        CSW_RECORDING | GENERALIZED_DATA | STOP_TAPE_IN_48K | SET_SIGNAL_LEVEL => {
            body_length(EXTENSION_LENGTH_BYTES, probe.u32_le()?)
        }
        PAUSE_OR_STOP => PAUSE_BODY,
        JUMP_TO_BLOCK => JUMP_BODY,
        LOOP_START => LOOP_START_BODY,
        GROUP_END | LOOP_END | RETURN_FROM_SEQUENCE => EMPTY_BODY,
        // A count word, then that many signed relative block offsets.
        CALL_SEQUENCE => body_length(2, u32::from(probe.u16_le()?) * 2),
        // "Length of the whole block (without these two bytes)".
        SELECT_BLOCK | ARCHIVE_INFO => body_length(2, u32::from(probe.u16_le()?)),
        GROUP_START | TEXT_DESCRIPTION => body_length(1, u32::from(probe.u8()?)),
        MESSAGE => {
            probe.skip(MESSAGE_LENGTH_AT)?;
            body_length(2, u32::from(probe.u8()?))
        }
        // A count byte, then that many three-byte HWINFO records.
        HARDWARE_TYPE => body_length(1, u32::from(probe.u8()?) * 3),
        EMULATION_INFO => EMULATION_INFO_BODY,
        CUSTOM_INFO => {
            probe.skip(CUSTOM_INFO_ID_LENGTH)?;
            body_length(CUSTOM_INFO_FIXED, probe.u32_le()?)
        }
        SNAPSHOT => {
            probe.skip(SNAPSHOT_LENGTH_AT)?;
            body_length(EXTENSION_LENGTH_BYTES, probe.u24_le()?)
        }
        GLUE => GLUE_BODY,
        C64_ROM_DATA | C64_TURBO_DATA => return Err(Error::UnplayableBlock { offset, id }),
        _ => return Err(Error::UnknownBlock { offset, id }),
    };
    Ok(length)
}

// ---------------------------------------------------------------------------------------
// Playing — the block list as a program
// ---------------------------------------------------------------------------------------

/// Where to go after a block.
enum Step {
    /// On to the block after this one.
    Next,
    /// On to the block at this index.
    Goto(usize),
    /// The tape ends here.
    Stop,
}

/// A loop being replayed: `ID 24` opened it and `ID 25` closes it.
struct Repeat {
    /// Index of the first block inside the loop.
    start: usize,
    /// Passes still to complete, including the one running now.
    remaining: u16,
}

/// A call sequence being executed: `ID 26` opened it and `ID 27` returns from it.
struct Call {
    /// Index of the `ID 26` block, which is what its offsets are relative to.
    block: usize,
    /// Which of its calls is running.
    index: usize,
}

/// The control flow a `.tzx` can express, which is one loop and one call sequence.
///
/// *"For simplicity reasons don't nest loop blocks!"* and *"The 'nesting' of call blocks is also
/// not allowed"* — so one register each, and a file that nests anyway is refused by name rather
/// than accepted into a deeper structure the format does not describe.
struct Flow {
    /// The loop in progress.
    repeat: Option<Repeat>,
    /// The call sequence in progress.
    call: Option<Call>,
}

/// The block list, being turned into a signal.
struct Player<'a> {
    /// Every block in the file, in file order.
    blocks: &'a [Block<'a>],
    /// Which machine will play it — `ID 2A`'s only input.
    model: Model,
    /// The train being built.
    signal: Signal,
    /// Where the loop and call registers stand.
    flow: Flow,
}

impl<'a> Player<'a> {
    fn new(blocks: &'a [Block<'a>], model: Model) -> Self {
        Self {
            blocks,
            model,
            signal: Signal::new(),
            flow: Flow {
                repeat: None,
                call: None,
            },
        }
    }

    /// Play every block the control flow reaches, and return the train.
    fn run(mut self) -> Result<Vec<u32>, Error> {
        let mut index = 0_usize;
        let mut played = 0_usize;

        while let Some(block) = self.blocks.get(index) {
            played += 1;
            if played > MAX_BLOCKS_PLAYED {
                return Err(Error::TooManyBlocksPlayed {
                    offset: block.offset,
                    limit: MAX_BLOCKS_PLAYED,
                });
            }
            match self.execute(block, index)? {
                Step::Next => index += 1,
                Step::Goto(target) => index = target,
                Step::Stop => break,
            }
        }
        Ok(self.signal.into_pulses())
    }

    /// Play one block.
    ///
    /// A dispatch table and nothing else: every arm is one named call, so the shape of the
    /// format is readable here and the work is where its name says it is.
    fn execute(&mut self, block: &Block<'_>, index: usize) -> Result<Step, Error> {
        match block.id {
            STANDARD_SPEED_DATA => self.standard_speed(block),
            TURBO_SPEED_DATA => self.turbo_speed(block),
            PURE_TONE => self.pure_tone(block),
            PULSE_SEQUENCE => self.pulse_sequence(block),
            PURE_DATA => self.pure_data(block),
            DIRECT_RECORDING => self.direct_recording(block),
            PAUSE_OR_STOP => self.pause_or_stop(block),
            JUMP_TO_BLOCK => self.jump(block, index),
            LOOP_START => self.loop_start(block, index),
            LOOP_END => self.loop_end(block),
            CALL_SEQUENCE => self.call_sequence(block, index),
            RETURN_FROM_SEQUENCE => self.return_from_sequence(block),
            STOP_TAPE_IN_48K => Ok(self.stop_in_48k()),
            SET_SIGNAL_LEVEL => self.set_signal_level(block),
            CSW_RECORDING | GENERALIZED_DATA => Err(Error::UnplayableBlock {
                offset: block.offset,
                id: block.id,
            }),
            // Everything else is metadata or presentation: a group's name, a description, a
            // message, the archive and hardware tables, a custom or deprecated payload, the
            // glue between two concatenated files — and `ID 28`, which asks for a menu this
            // emulator does not have and which therefore falls through to the next block, as
            // it does in any player without one. None of them changes the signal.
            _ => Ok(Step::Next),
        }
    }

    // -- the blocks that carry signal --------------------------------------------------

    /// `ID 10`: a `.tap` block, with the ROM's timings and a pause in milliseconds.
    ///
    /// *"This block must be replayed with the standard Spectrum ROM timing values - see the
    /// values in curly brackets in block ID 11"*, and *"The pilot tone consists in 8063 pulses
    /// if the first data byte (flag byte) is < 128, 3223 otherwise"*. Both come from
    /// [`tap`], which derives each of them by counting T-states through the ROM's own
    /// `SA-BYTES` and has them graded against that routine on every `cargo test`. Restating
    /// them here would be a second copy of a number to get wrong.
    fn standard_speed(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let pause = body.u16_le()?;
        let length = usize::from(body.u16_le()?);
        let data = body.take(length)?;

        let Some((&flag, _)) = data.split_first() else {
            return Err(Error::EmptyBlock {
                offset: block.offset,
            });
        };
        self.signal.speed_data(
            &SpeedData {
                pilot: tap::PILOT_PULSE,
                pilot_pulses: tap::pilot_pulses(flag),
                sync_first: tap::SYNC_FIRST,
                sync_second: tap::SYNC_SECOND,
                data: Data {
                    bytes: data,
                    used_bits: UsedBits::ALL,
                    zero: tap::BIT_ZERO,
                    one: tap::BIT_ONE,
                },
            },
            block.offset,
        )?;
        self.pause(pause, block.offset)
    }

    /// `ID 11`: the same shape with every number read from the file.
    ///
    /// This is the block `.tap` cannot express and the reason this module exists.
    fn turbo_speed(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let pilot = u32::from(body.u16_le()?);
        let sync_first = u32::from(body.u16_le()?);
        let sync_second = u32::from(body.u16_le()?);
        let zero = u32::from(body.u16_le()?);
        let one = u32::from(body.u16_le()?);
        let pilot_pulses = usize::from(body.u16_le()?);
        let used_bits = self.used_bits(&mut body)?;
        let pause = body.u16_le()?;
        let length = body.u24_le()?;
        let data = body.take(body_length(0, length))?;

        self.signal.speed_data(
            &SpeedData {
                pilot,
                pilot_pulses,
                sync_first,
                sync_second,
                data: Data {
                    bytes: data,
                    used_bits,
                    zero,
                    one,
                },
            },
            block.offset,
        )?;
        self.pause(pause, block.offset)
    }

    /// `ID 12`: *"a tone which is basically the same as the pilot tone in the ID 10, ID 11
    /// blocks"* — one length, and how many half-periods of it.
    fn pure_tone(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let length = u32::from(body.u16_le()?);
        let count = usize::from(body.u16_le()?);
        self.signal.tone(length, count, block.offset)?;
        Ok(Step::Next)
    }

    /// `ID 13`: *"N pulses, each having its own timing"* — a non-standard sync tone.
    fn pulse_sequence(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let count = body.u8()?;
        for _ in 0..count {
            let length = u32::from(body.u16_le()?);
            self.signal.pulse(length, block.offset)?;
        }
        Ok(Step::Next)
    }

    /// `ID 14`: *"the same as in the turbo loading data block, except that it has no pilot or
    /// sync pulses"*.
    fn pure_data(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let zero = u32::from(body.u16_le()?);
        let one = u32::from(body.u16_le()?);
        let used_bits = self.used_bits(&mut body)?;
        let pause = body.u16_le()?;
        let length = body.u24_le()?;
        let data = body.take(body_length(0, length))?;

        self.signal.data(
            &Data {
                bytes: data,
                used_bits,
                zero,
                one,
            },
            block.offset,
        )?;
        self.pause(pause, block.offset)
    }

    /// `ID 15`: levels rather than half-periods — *"Each bit represents a state on the EAR
    /// port (i.e. one sample). MSb is played first."*
    fn direct_recording(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let t_states_per_sample = u32::from(body.u16_le()?);
        let pause = body.u16_le()?;
        let used_bits = self.used_bits(&mut body)?;
        let length = body.u24_le()?;
        let samples = body.take(body_length(0, length))?;

        self.signal.direct(
            &Samples {
                bytes: samples,
                used_bits,
                t_states_per_sample,
            },
            block.offset,
        )?;
        self.pause(pause, block.offset)
    }

    /// `ID 20`: a silence, or — at zero — *"STOP THE TAPE"*.
    fn pause_or_stop(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        let milliseconds = body.u16_le()?;
        if milliseconds == 0 {
            return Ok(Step::Stop);
        }
        self.pause(milliseconds, block.offset)
    }

    /// `ID 2B`: *"sets the current signal level to the specified value (high or low)"*, for
    /// *"custom loaders which are level-sensitive"*.
    fn set_signal_level(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let mut body = block.reader();
        body.skip(EXTENSION_LENGTH_BYTES)?;
        let level = body.u8()?;
        self.signal.set_level(level != 0, block.offset)?;
        Ok(Step::Next)
    }

    // -- the blocks that decide what is played -----------------------------------------

    /// `ID 23`: *"jump from one block to another within the file"*, by a signed block count.
    ///
    /// *"All blocks are included in the block count!"* and the offset is relative to the jump
    /// block itself: *"Jump 1 = 'Go to the next block'"*.
    fn jump(&mut self, block: &Block<'_>, index: usize) -> Result<Step, Error> {
        let mut body = block.reader();
        let relative = body.u16_le()?;
        self.target(index, relative, block.offset).map(Step::Goto)
    }

    /// `ID 24`: *"If you have a sequence of identical blocks... this block is the same as the
    /// FOR statement in BASIC."*
    fn loop_start(&mut self, block: &Block<'_>, index: usize) -> Result<Step, Error> {
        if self.flow.repeat.is_some() {
            return Err(self.misplaced(block));
        }
        let mut body = block.reader();
        let remaining = body.u16_le()?;
        self.flow.repeat = Some(Repeat {
            start: index.saturating_add(1),
            remaining,
        });
        Ok(Step::Next)
    }

    /// `ID 25`: *"jump back to the start of the loop if it hasn't been run for the specified
    /// number of times"*.
    ///
    /// A count of zero or one plays the body once, which is the only total reading of a field
    /// the description says is *"greater than 1"* without saying what a smaller value means.
    fn loop_end(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let Some(repeat) = self.flow.repeat.as_mut() else {
            return Err(self.misplaced(block));
        };
        repeat.remaining = repeat.remaining.saturating_sub(1);
        if repeat.remaining > 0 {
            return Ok(Step::Goto(repeat.start));
        }
        self.flow.repeat = None;
        Ok(Step::Next)
    }

    /// `ID 26`: *"executes a sequence of blocks that are somewhere else and then goes back to
    /// the next block"*.
    fn call_sequence(&mut self, block: &Block<'_>, index: usize) -> Result<Step, Error> {
        if self.flow.call.is_some() {
            return Err(self.misplaced(block));
        }
        let mut body = block.reader();
        if body.u16_le()? == 0 {
            return Ok(Step::Next);
        }
        let relative = body.u16_le()?;
        let target = self.target(index, relative, block.offset)?;
        self.flow.call = Some(Call {
            block: index,
            index: 0,
        });
        Ok(Step::Goto(target))
    }

    /// `ID 27`: *"The next block played will be the block after the last CALL block (or the
    /// next Call, if the Call block had multiple calls)."*
    fn return_from_sequence(&mut self, block: &Block<'_>) -> Result<Step, Error> {
        let Some(call) = self.flow.call.as_ref() else {
            return Err(self.misplaced(block));
        };
        let (caller, next) = (call.block, call.index.saturating_add(1));
        let Some(source) = self.blocks.get(caller) else {
            return Err(self.misplaced(block));
        };

        let mut body = source.reader();
        let count = usize::from(body.u16_le()?);
        if next >= count {
            self.flow.call = None;
            return Ok(Step::Goto(caller.saturating_add(1)));
        }
        body.skip(next.saturating_mul(2))?;
        let relative = body.u16_le()?;
        let target = self.target(caller, relative, source.offset)?;
        self.flow.call = Some(Call {
            block: caller,
            index: next,
        });
        Ok(Step::Goto(target))
    }

    /// `ID 2A`: *"the tape will stop ONLY if the machine is an 48K Spectrum"*.
    ///
    /// The wildcard is deliberate: the description says *only* a 48K stops, so a machine this
    /// crate grows later plays on, which is the reading that stays right without being revisited.
    fn stop_in_48k(&self) -> Step {
        match self.model {
            Model::Spectrum48K => Step::Stop,
            _ => Step::Next,
        }
    }

    // -- shared -------------------------------------------------------------------------

    /// A silence, and the edge that precedes it.
    ///
    /// *"A 'Pause' block consists of a 'low' pulse level of some duration. To ensure that the
    /// last edge produced is properly finished there should be at least 1 ms. pause of the
    /// opposite level and only after that the pulse should go to 'low'. At the end of a 'Pause'
    /// block the 'current pulse level' is low... A 'Pause' block of zero duration is completely
    /// ignored, so the 'current pulse level' will NOT change in this case."*
    ///
    /// All four sentences are load-bearing, and the last one is why this returns before
    /// emitting anything at zero rather than emitting a zero-length silence.
    ///
    /// **The settle exists only when the line is not already low.** That is not an
    /// optimisation: the settle's job is *"to ensure that the last edge produced is properly
    /// finished"*, and a line already resting low has no edge outstanding — the block before it
    /// was a pause, or a direct recording that ended low, or nothing at all. Emitting one
    /// anyway is harmless in level terms, because the train's own closing flip and the
    /// correcting edge cancel in zero time, but it puts two half-periods into every pause that
    /// mean nothing, and a train that can be written out by hand is worth more than that.
    fn pause(&mut self, milliseconds: u16, at: usize) -> Result<Step, Error> {
        if milliseconds == 0 {
            return Ok(Step::Next);
        }
        let mut remaining = u32::from(milliseconds) * T_STATES_PER_MILLISECOND;

        if self.signal.level() != LOW {
            let settle = (PAUSE_SETTLE_MILLISECONDS * T_STATES_PER_MILLISECOND).min(remaining);
            self.signal.pulse(settle, at)?;
            remaining -= settle;
        }
        if remaining > 0 {
            self.signal.pulse(remaining, at)?;
        }
        self.signal.set_level(LOW, at)?;
        Ok(Step::Next)
    }

    /// The `Used bits in the last byte` field, refused if it is outside the format's `1-8`.
    fn used_bits(&self, body: &mut Reader<'_>) -> Result<UsedBits, Error> {
        let offset = body.offset();
        let bits = body.u8()?;
        UsedBits::new(bits).ok_or(Error::UsedBitsOutOfRange { offset, bits })
    }

    /// The block index `relative` blocks from `from`, refused if it leaves the file.
    ///
    /// The value is *"a signed short word"*, so a backward jump is a large `u16` and reading it
    /// unsigned would send every one of them off the end. Landing exactly on the block after the
    /// last is legal and simply ends the tape.
    fn target(&self, from: usize, relative: u16, at: usize) -> Result<usize, Error> {
        let to = i64::try_from(from)
            .unwrap_or(i64::MAX)
            .saturating_add(i64::from(relative as i16));
        match usize::try_from(to) {
            Ok(index) if index <= self.blocks.len() => Ok(index),
            _ => Err(Error::JumpOutOfRange {
                offset: at,
                blocks: self.blocks.len(),
                target: to,
            }),
        }
    }

    /// A structural block where the format does not allow one.
    const fn misplaced(&self, block: &Block<'_>) -> Error {
        Error::MisplacedBlock {
            offset: block.offset,
            id: block.id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file header, so a fixture is a `.tzx` rather than a byte string that resembles one.
    fn header() -> Vec<u8> {
        let mut file = SIGNATURE.to_vec();
        file.extend([END_OF_TEXT, SUPPORTED_MAJOR, 20]);
        file
    }

    fn file(blocks: &[&[u8]]) -> Vec<u8> {
        let mut bytes = header();
        for block in blocks {
            bytes.extend_from_slice(block);
        }
        bytes
    }

    fn pulses(blocks: &[&[u8]]) -> Vec<u32> {
        parse(&file(blocks), Model::Spectrum48K)
            .expect("a well-formed file")
            .pulses()
            .to_vec()
    }

    /// One millisecond of the 48K's clock, written out here rather than taken from production.
    ///
    /// The three pause tests below assert the lengths a pause emits, and they used to assert them
    /// against `T_STATES_PER_MILLISECOND` itself — which made them agree with that constant
    /// whatever it held, including zero. `tap.rs` has exactly the independent anchor they lacked
    /// (`PAUSE_T_STATES == 69_888 * 50`), and this is the same move: the machine's clock is
    /// 3.5 MHz, so a millisecond of it is a thousandth of that, and the arithmetic is done here.
    const ONE_MILLISECOND: u32 = 3_500_000 / 1_000;

    #[test]
    fn a_millisecond_is_a_thousandth_of_the_machines_clock() {
        // The anchor's own grading, against the machine's constant rather than against the tape
        // module's — two independent sources for one number, neither of them the subject.
        assert_eq!(ONE_MILLISECOND, 3_500);
        assert_eq!(
            ONE_MILLISECOND,
            crate::timing::Timing::SPECTRUM_48K.cpu_hz() / 1_000
        );
        assert_eq!(
            T_STATES_PER_MILLISECOND, ONE_MILLISECOND,
            "the production constant must agree with the clock it claims to come from"
        );
    }

    #[test]
    fn the_header_is_the_one_from_the_format_description() {
        // Ten bytes, written out here rather than taken from the constants above, so a
        // constant that drifted would disagree with a literal rather than with itself.
        assert_eq!(
            header(),
            vec![b'Z', b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 1, 20]
        );
        assert_eq!(header().len(), HEADER_LENGTH);
        assert!(
            parse(&header(), Model::Spectrum48K).is_ok(),
            "no blocks is a blank tape"
        );
    }

    #[test]
    fn a_file_that_is_not_a_tzx_is_refused_rather_than_guessed_at() {
        assert_eq!(
            parse(b"", Model::Spectrum48K),
            Err(Error::Truncated {
                offset: 0,
                needed: 7,
                available: 0
            })
        );
        let mut wrong = header();
        wrong.clear();
        wrong.extend(b"ZXTape?\x1A\x01\x14");
        assert_eq!(parse(&wrong, Model::Spectrum48K), Err(Error::NotATzxFile));

        let mut no_marker = header();
        no_marker.clear();
        no_marker.extend(b"ZXTape!\x00\x01\x14");
        assert_eq!(
            parse(&no_marker, Model::Spectrum48K),
            Err(Error::NotATzxFile)
        );
    }

    #[test]
    fn a_newer_major_revision_is_refused_and_a_newer_minor_one_is_not() {
        // "your program must be able to handle files of at least its major version number...
        // If your program can handle (say) version 1.05 and you encounter a file with version
        // number 1.06, your program must be able to handle it".
        let mut future = header();
        future.clear();
        future.extend(b"ZXTape!\x1A\x02\x00");
        assert_eq!(
            parse(&future, Model::Spectrum48K),
            Err(Error::UnsupportedVersion { major: 2, minor: 0 })
        );

        let mut newer_minor = header();
        newer_minor.clear();
        newer_minor.extend(b"ZXTape!\x1A\x01\xFF");
        assert!(parse(&newer_minor, Model::Spectrum48K).is_ok());
    }

    #[test]
    fn a_pause_finishes_the_last_edge_and_then_goes_low() {
        // The format's four sentences, as a train, in the case where there is an edge to
        // finish. A pure tone of one half-period leaves the line high, so the pause holds it
        // high for 1 ms — and that half-period's own closing flip is what takes the line low,
        // which is why no instant edge appears between the two.
        //
        // **This expectation was derived before it was measured and it was wrong**, in exactly
        // that place: an instant edge was predicted after the settle. The measurement disagreed
        // by one pulse and the reason was legible — a half-period ends by flipping, so a settle
        // that begins high ends low on its own. Recorded here rather than quietly corrected.
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let pause = [PAUSE_OR_STOP, 0x02, 0x00];
        assert_eq!(
            pulses(&[&tone, &pause]),
            vec![
                0x10,            // the tone: one half-period, and the line is now high
                ONE_MILLISECOND, // 1 ms of that level, ending with the flip down to low
                ONE_MILLISECOND, // the remaining 1 ms, low
                0,               // an instant edge back to low, where a pause ends
            ]
        );
    }

    #[test]
    fn a_pause_with_no_edge_outstanding_is_one_unbroken_silence() {
        // The other branch, and the reason the settle is conditional. Nothing has driven the
        // line, so it is already low and there is no last edge to finish: the whole pause is
        // one half-period rather than a settle and a remainder.
        let pause = [PAUSE_OR_STOP, 0x02, 0x00];
        assert_eq!(pulses(&[&pause]), vec![2 * ONE_MILLISECOND, 0]);
    }

    #[test]
    fn a_one_millisecond_pause_has_no_remainder_to_emit() {
        // The boundary of the settle's `min`: a 1 ms pause after an outstanding edge is all
        // settle and no silence, and it must not emit a zero-length pulse nobody asked for —
        // the settle already ended low, so there is no polarity left to correct.
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let pause = [PAUSE_OR_STOP, 0x01, 0x00];
        assert_eq!(pulses(&[&tone, &pause]), vec![0x10, ONE_MILLISECOND]);
    }

    #[test]
    fn a_pause_of_zero_is_completely_ignored() {
        // "A 'Pause' block of zero duration is completely ignored, so the 'current pulse level'
        // will NOT change in this case." As an `ID 20` it means stop; inside a data block it
        // means emit nothing, and this is the data-block half.
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let no_pause_data = [
            PURE_DATA, 0x0A, 0x00, 0x14, 0x00, 0x08, 0x00, 0x00, 0x01, 0x00, 0x00, 0xFF,
        ];
        let train = pulses(&[&tone, &no_pause_data]);
        assert_eq!(
            train.len(),
            1 + 16,
            "the tone, the byte, and no silence at all"
        );
    }

    #[test]
    fn a_direct_recordings_two_words_are_a_rate_then_a_pause_and_not_the_reverse() {
        // **`ID 15` had no value assertion through `parse` at all.** It appeared twice in the
        // suite: once in the truncation sweep, which only asks whether a prefix parses, and once
        // in `tzx_hostile.rs`, which only asks that nothing panics. So `direct_recording`'s two
        // adjacent `u16` reads — `TS per sample` at body offset 0, `pause` at offset 2 — could be
        // transposed and the whole workspace would stay green. That is the permutation
        // `docs/M6.md` Decision 7 names as the one a round trip cannot see, on the block whose
        // level rule was already found designed, documented and unimplemented once.
        //
        // The expectation is written out from the format description, not from the code: the
        // samples are levels, so each **run** of equal ones is a half-period.
        const RATE: u32 = 0x4F; // 79 T-states per sample — the body's first word
        const PAUSE_MS: u32 = 3; // milliseconds — the body's second word
        let recording = [
            DIRECT_RECORDING,
            0x4F,
            0x00, // TS per sample = 79
            0x03,
            0x00, // pause = 3 ms
            0x08, // all eight bits of the last byte
            0x01,
            0x00,
            0x00, // one byte of samples
            0xA5, // 1010_0101 — runs of 1,1,1,2,1,1,1, opening high and closing high
        ];

        assert_eq!(
            pulses(&[&recording]),
            vec![
                0,               // the first sample is high and the line rests low: an instant edge
                RATE,            // 1
                RATE,            // 0
                RATE,            // 1
                2 * RATE,        // 00
                RATE,            // 1
                RATE,            // 0
                RATE,            // 1
                0, // the last sample is high, so the closing flip is cancelled in zero time
                ONE_MILLISECOND, // the pause settles the outstanding edge...
                (PAUSE_MS - 1) * ONE_MILLISECOND, // ...and the rest of it is silence
                0, // and a pause ends low
            ],
            "a rate of 79 and a pause of 3 ms; transposed they would be 3 and 79"
        );

        // The two numbers must not be interchangeable, or the assertion above grades their sum
        // rather than their order. 79 T-states and 3 ms differ by four orders of magnitude, and
        // this says so rather than leaving it to be read off the fixture.
        const {
            assert!(RATE != PAUSE_MS);
            assert!(PAUSE_MS * ONE_MILLISECOND > RATE * u8::BITS);
        }
    }

    #[test]
    fn a_zero_pause_block_stops_the_tape() {
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let stop = [PAUSE_OR_STOP, 0x00, 0x00];
        assert_eq!(
            pulses(&[&tone, &stop, &tone]),
            vec![0x10],
            "nothing after the stop reaches the train"
        );
    }

    #[test]
    fn the_48k_stop_block_depends_on_the_machine() {
        // The one block whose meaning is a function of the model, and the reason `parse` takes
        // one. Getting it wrong is not symmetric: a 128 that stopped here would lose every
        // level of a multiload after the first.
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let stop = [STOP_TAPE_IN_48K, 0x00, 0x00, 0x00, 0x00];
        let bytes = file(&[&tone, &stop, &tone]);

        let on_48k = parse(&bytes, Model::Spectrum48K).expect("a well-formed file");
        assert_eq!(on_48k.pulses(), &[0x10]);

        let on_128 = parse(&bytes, Model::Spectrum128).expect("a well-formed file");
        assert_eq!(
            on_128.pulses(),
            &[0x10, 0x10],
            "a 128 plays straight through"
        );
    }

    #[test]
    fn a_set_signal_level_block_forces_the_polarity() {
        let high = [SET_SIGNAL_LEVEL, 0x01, 0x00, 0x00, 0x00, 0x01];
        let low = [SET_SIGNAL_LEVEL, 0x01, 0x00, 0x00, 0x00, 0x00];
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];

        // The line rests low, so forcing it low emits nothing and forcing it high emits an edge.
        assert_eq!(pulses(&[&low, &tone]), vec![0x10]);
        assert_eq!(pulses(&[&high, &tone]), vec![0, 0x10]);
        // ...and after one half-period the line is high, so the two swap over.
        assert_eq!(pulses(&[&tone, &high]), vec![0x10]);
        assert_eq!(pulses(&[&tone, &low]), vec![0x10, 0]);
    }

    #[test]
    fn a_pulse_sequence_is_its_own_lengths_in_order() {
        // Three distinct, non-zero lengths, so an order reversal or a dropped pulse is visible
        // rather than plausible.
        let sequence = [PULSE_SEQUENCE, 0x03, 0x11, 0x00, 0x22, 0x00, 0x33, 0x00];
        assert_eq!(pulses(&[&sequence]), vec![0x11, 0x22, 0x33]);
    }

    #[test]
    fn a_loop_replays_its_body_the_stated_number_of_times() {
        let start = |count: u8| [LOOP_START, count, 0x00];
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let end = [LOOP_END];

        assert_eq!(pulses(&[&start(3), &tone, &end]), vec![0x10; 3]);
        assert_eq!(pulses(&[&start(1), &tone, &end]), vec![0x10; 1]);
        assert_eq!(
            pulses(&[&start(0), &tone, &end]),
            vec![0x10; 1],
            "a count below the format's minimum plays the body once"
        );
    }

    #[test]
    fn a_jump_is_relative_to_the_jump_block_and_is_signed() {
        // "Jump 1 = 'Go to the next block'... Jump 2 = 'Skip one block'... Jump -1 = 'Go to the
        // previous block'". The two tones differ so the arithmetic is visible.
        let first = [PURE_TONE, 0x11, 0x00, 0x01, 0x00];
        let second = [PURE_TONE, 0x22, 0x00, 0x01, 0x00];
        let skip_one = [JUMP_TO_BLOCK, 0x02, 0x00];
        assert_eq!(pulses(&[&skip_one, &first, &second]), vec![0x22]);

        let next = [JUMP_TO_BLOCK, 0x01, 0x00];
        assert_eq!(pulses(&[&next, &first, &second]), vec![0x11, 0x22]);

        // A backward jump is a **negative signed word**, which read unsigned would be 65534 and
        // land off the end of the file. Laid out so both directions are load-bearing:
        //
        //   0  tone 0x11        1  jump +2  ->  3
        //   2  stop             3  tone 0x33      4  jump -2  ->  2
        //
        // A forward jump that landed anywhere but 3 would either stop the tape early or replay
        // the tone; a backward jump that landed on 3 rather than 2 would never terminate.
        let stop = [PAUSE_OR_STOP, 0x00, 0x00];
        let third = [PURE_TONE, 0x33, 0x00, 0x01, 0x00];
        let forward = [JUMP_TO_BLOCK, 0x02, 0x00];
        let backward = [JUMP_TO_BLOCK, 0xFE, 0xFF];
        assert_eq!(
            pulses(&[&first, &forward, &stop, &third, &backward]),
            vec![0x11, 0x33]
        );
    }

    #[test]
    fn a_jump_off_either_end_of_the_file_is_refused() {
        let back = [JUMP_TO_BLOCK, 0xFF, 0xFF];
        assert_eq!(
            parse(&file(&[&back]), Model::Spectrum48K),
            Err(Error::JumpOutOfRange {
                offset: HEADER_LENGTH,
                blocks: 1,
                target: -1
            })
        );
        let far = [JUMP_TO_BLOCK, 0x10, 0x00];
        assert_eq!(
            parse(&file(&[&far]), Model::Spectrum48K),
            Err(Error::JumpOutOfRange {
                offset: HEADER_LENGTH,
                blocks: 1,
                target: 16
            })
        );
        // ...but landing exactly one past the last block is the tape ending, not an error.
        let past_the_end = [JUMP_TO_BLOCK, 0x01, 0x00];
        assert!(parse(&file(&[&past_the_end]), Model::Spectrum48K).is_ok());
    }

    #[test]
    fn a_call_sequence_visits_each_target_and_comes_back() {
        // "It basically executes a sequence of blocks that are somewhere else and then goes
        // back to the next block." Laid out with the indices written down, because the offsets
        // are relative to the call block and getting that wrong is the whole risk:
        //
        //   0  call +3, +5     1  tone 0xAA     2  stop
        //   3  tone 0x22       4  return        5  tone 0x33     6  return
        //
        // Three distinct tones, so the order is visible in the train rather than only its
        // length: the two subroutines run in the order the call block lists them, and only
        // then does execution resume at block 1.
        let call = [CALL_SEQUENCE, 0x02, 0x00, 0x03, 0x00, 0x05, 0x00];
        let after = [PURE_TONE, 0xAA, 0x00, 0x01, 0x00];
        let stop = [PAUSE_OR_STOP, 0x00, 0x00];
        let first = [PURE_TONE, 0x22, 0x00, 0x01, 0x00];
        let second = [PURE_TONE, 0x33, 0x00, 0x01, 0x00];
        let ret = [RETURN_FROM_SEQUENCE];

        assert_eq!(
            pulses(&[&call, &after, &stop, &first, &ret, &second, &ret]),
            vec![0x22, 0x33, 0xAA]
        );
    }

    #[test]
    fn a_misplaced_structural_block_is_refused_by_name() {
        for (block, id) in [
            (vec![LOOP_END], LOOP_END),
            (vec![RETURN_FROM_SEQUENCE], RETURN_FROM_SEQUENCE),
        ] {
            assert_eq!(
                parse(&file(&[&block]), Model::Spectrum48K),
                Err(Error::MisplacedBlock {
                    offset: HEADER_LENGTH,
                    id
                })
            );
        }

        // Nesting, which the description forbids in both cases.
        let start = [LOOP_START, 0x02, 0x00];
        assert_eq!(
            parse(&file(&[&start, &start]), Model::Spectrum48K),
            Err(Error::MisplacedBlock {
                offset: HEADER_LENGTH + start.len(),
                id: LOOP_START
            })
        );
    }

    #[test]
    fn a_group_or_a_description_changes_nothing_about_the_train() {
        // The metadata blocks, each with a length that has to be right or the block after it
        // would be misparsed. A description's text is deliberately the bytes of another block
        // header, so a length read as zero would play it.
        let tone = [PURE_TONE, 0x10, 0x00, 0x01, 0x00];
        let group_start = [GROUP_START, 0x04, b'n', b'a', b'm', b'e'];
        let group_end = [GROUP_END];
        let description = [TEXT_DESCRIPTION, 0x05, PURE_TONE, 0xFF, 0x00, 0xFF, 0x00];
        let message = [MESSAGE, 0x05, 0x02, b'h', b'i'];
        let glue = [GLUE, b'X', b'T', b'a', b'p', b'e', b'!', 0x1A, 0x01, 0x14];

        assert_eq!(
            pulses(&[
                &group_start,
                &description,
                &message,
                &glue,
                &tone,
                &group_end
            ]),
            vec![0x10]
        );
    }

    #[test]
    fn a_block_whose_length_cannot_be_determined_is_refused_rather_than_skipped() {
        // The C64 blocks: the description's length column and its own field description
        // disagree by four bytes about whether the opening DWORD counts itself. Skipping by
        // either reading would silently misplace every block after it.
        for id in [C64_ROM_DATA, C64_TURBO_DATA] {
            let block = [id, 0x08, 0x00, 0x00, 0x00];
            assert_eq!(
                parse(&file(&[&block]), Model::Spectrum48K),
                Err(Error::UnplayableBlock {
                    offset: HEADER_LENGTH,
                    id
                })
            );
        }
    }

    #[test]
    fn a_block_that_carries_signal_we_cannot_play_is_refused_rather_than_skipped() {
        // `ID 18` and `ID 19` have knowable lengths, so they *could* be skipped — and skipping
        // them would drop signal and produce a tape that fails to load with no explanation.
        for id in [CSW_RECORDING, GENERALIZED_DATA] {
            let block = [id, 0x00, 0x00, 0x00, 0x00];
            assert_eq!(
                parse(&file(&[&block]), Model::Spectrum48K),
                Err(Error::UnplayableBlock {
                    offset: HEADER_LENGTH,
                    id
                })
            );
        }
    }

    #[test]
    fn an_unknown_block_id_is_refused() {
        // Every ID the description defines is handled, so an unrecognised one is either newer
        // than revision 1.20 or not a `.tzx` at all. Guessing that it follows the extension
        // rule would skip an arbitrary span and produce a wrong train in silence.
        let block = [0x99_u8, 0x00, 0x00];
        assert_eq!(
            parse(&file(&[&block]), Model::Spectrum48K),
            Err(Error::UnknownBlock {
                offset: HEADER_LENGTH,
                id: 0x99
            })
        );
    }

    #[test]
    fn a_used_bits_count_outside_the_formats_range_is_refused() {
        for bits in [0x00_u8, 0x09, 0xFF] {
            let block = [
                PURE_DATA, 0x0A, 0x00, 0x14, 0x00, bits, 0x00, 0x00, 0x01, 0x00, 0x00, 0xFF,
            ];
            assert_eq!(
                parse(&file(&[&block]), Model::Spectrum48K),
                Err(Error::UsedBitsOutOfRange {
                    offset: HEADER_LENGTH + 1 + 0x04,
                    bits
                })
            );
        }
    }

    #[test]
    fn an_empty_standard_speed_block_has_no_flag_byte_to_choose_a_pilot_with() {
        // The same ruling `tap::parse` makes, for the same reason: the pilot tone's length is a
        // function of the flag byte, so a block without one cannot become a signal.
        let block = [STANDARD_SPEED_DATA, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            parse(&file(&[&block]), Model::Spectrum48K),
            Err(Error::EmptyBlock {
                offset: HEADER_LENGTH
            })
        );
    }

    #[test]
    fn a_short_loop_over_silent_blocks_finishes_rather_than_being_refused() {
        // The negative control for the budget, and the reason it is the pulse ceiling rather
        // than something smaller. A 65535-times loop over two silent blocks is 196,606 block
        // executions — a lot, and legal, and it must complete. The budget only exists for files
        // that never finish at all.
        let start = [LOOP_START, 0xFF, 0xFF];
        let group_start = [GROUP_START, 0x00];
        let group_end = [GROUP_END];
        let end = [LOOP_END];
        let tape = parse(
            &file(&[&start, &group_start, &group_end, &end]),
            Model::Spectrum48K,
        )
        .expect("a legal loop, however tedious");
        assert_eq!(tape.pulses(), &[] as &[u32], "and it emitted nothing");
    }

    #[test]
    fn a_loop_that_never_finishes_is_bounded_by_the_budget() {
        // The case the pulse ceiling cannot catch, because nothing is emitted. A loop end with
        // its own body inside it re-enters the same loop register forever, so no pass ever
        // completes. Without the budget this hangs; with it, it is an error naming the limit.
        let start = [LOOP_START, 0xFF, 0xFF];
        let back = [JUMP_TO_BLOCK, 0x00, 0x00];
        let end = [LOOP_END];
        assert_eq!(
            parse(&file(&[&start, &back, &end]), Model::Spectrum48K),
            Err(Error::TooManyBlocksPlayed {
                offset: HEADER_LENGTH + start.len(),
                limit: MAX_BLOCKS_PLAYED
            })
        );
    }

    #[test]
    fn a_jump_to_itself_terminates_rather_than_hanging() {
        // "Jump 0 = 'Loop Forever' - this should never happen", says the description, of a file
        // it nonetheless permits. Progress is not structural here, so this is what bounds it.
        let forever = [JUMP_TO_BLOCK, 0x00, 0x00];
        assert_eq!(
            parse(&file(&[&forever]), Model::Spectrum48K),
            Err(Error::TooManyBlocksPlayed {
                offset: HEADER_LENGTH,
                limit: MAX_BLOCKS_PLAYED
            })
        );
    }

    #[test]
    fn a_loop_that_multiplies_a_block_is_refused_rather_than_allocated() {
        // Three bytes of loop block asking for a 65535-times replay of a tone that is itself
        // 65535 half-periods: 4.2 billion pulses, or 17 GB, from a 17-byte file. This is the
        // allocation-sized-from-the-file construct, and it is a returned error.
        let start = [LOOP_START, 0xFF, 0xFF];
        let tone = [PURE_TONE, 0x01, 0x00, 0xFF, 0xFF];
        let end = [LOOP_END];
        let bytes = file(&[&start, &tone, &end]);
        assert!(bytes.len() < 32, "a small file demanding a large train");

        assert_eq!(
            parse(&bytes, Model::Spectrum48K),
            Err(Error::TapeTooLong {
                offset: HEADER_LENGTH + start.len(),
                limit: MAX_PULSES
            })
        );
    }

    #[test]
    fn a_prefix_parses_exactly_when_it_ends_on_a_block_boundary() {
        // The exhaustive truncation sweep `docs/M6.md` Decision 6 asks for, over a file that
        // exercises one block of every shape this module reads a length from. Exhaustive over
        // the axis that matters, and cheap because the fixture is small.
        //
        // **It asserts more than "nothing panicked", and it has to.** A truncated `.z80` is
        // never a valid `.z80`, so the snapshot sweep can demand `Err` at every length. A
        // `.tzx` is a header followed by self-delimiting blocks, so a prefix that stops exactly
        // between two blocks **is** a well-formed shorter tape — the header alone is a blank
        // one. Demanding `Err` everywhere would therefore have been wrong, and demanding merely
        // "it returned" would have passed against a parser that accepted every prefix. So the
        // boundaries are computed from the fixture's own construction and the verdict is
        // asserted to match them exactly, in both directions.
        let blocks: [&[u8]; 5] = [
            &[PURE_TONE, 0x10, 0x00, 0x01, 0x00],
            &[PULSE_SEQUENCE, 0x02, 0x11, 0x00, 0x22, 0x00],
            &[
                PURE_DATA, 0x0A, 0x00, 0x14, 0x00, 0x08, 0x01, 0x00, 0x01, 0x00, 0x00, 0x5A,
            ],
            &[
                DIRECT_RECORDING,
                0x4F,
                0x00,
                0x01,
                0x00,
                0x08,
                0x01,
                0x00,
                0x00,
                0xA5,
            ],
            &[TEXT_DESCRIPTION, 0x02, b'h', b'i'],
        ];
        let bytes = file(&blocks);

        let mut boundaries = vec![HEADER_LENGTH];
        for block in blocks {
            let last = boundaries.last().copied().unwrap_or(HEADER_LENGTH);
            boundaries.push(last + block.len());
        }
        assert_eq!(
            boundaries.last(),
            Some(&bytes.len()),
            "the boundaries must account for every byte, or the sweep below grades nothing"
        );

        for k in 0..=bytes.len() {
            let prefix = bytes.get(..k).expect("k <= len");
            let parsed = parse(prefix, Model::Spectrum48K).is_ok();
            assert_eq!(
                parsed,
                boundaries.contains(&k),
                "a {k}-byte prefix of a {}-byte file parsed: {parsed}",
                bytes.len()
            );
        }
    }
}
