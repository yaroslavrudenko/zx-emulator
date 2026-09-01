//! The `.z80` format: versions 1, 2 and 3 read, version 3 written.
//!
//! # Every offset here is transcribed, not remembered
//!
//! The header is irregular in ways that punish a plausible guess: `BC` is at offset 2 and
//! `HL` at 4, but `DE` is at **13**; `IY` is at 23 and `IX` at **25**, in that order; `A`
//! and `F` are separate bytes at 0 and 1 rather than an `AF` word. Each field below is
//! named and read in file order so that the code reads as the table it was taken from.
//!
//! # Why version 3 is what we write
//!
//! Two reasons, and neither is file size. **Version 3 is the only version that carries the
//! frame position**: version 1 has no T-state field at all, and the 23-byte additional
//! header a version 2 file declares covers offsets 32–54, stopping one byte short of the low
//! T-state word at 55. Writing either would make [`Snapshot::frame_t_state`] unrecoverable
//! through our own writer, so the round trip could not see it. And **compression gives the
//! decompressor a symmetric partner**, which is what makes
//! `decompress(compress(page)) == page` a property test rather than a handful of vectors.
//!
//! # The T-state counter is the highest-risk field in the design
//!
//! It is not a plain frame position: the frame is four quarters of 17472 T-states, the low
//! word counts *down* within the quarter, and the high byte is the quarter's index shifted by
//! three because it reads 3 at the top of the frame. `encode_t_states` in this module carries
//! the derivation, the format description's own sentence, and the two independent checks that
//! settle it — because its own exhaustive round trip provably cannot.

use ::z80::{CpuState, InterruptMode};

use super::reader::{Reader, Writer};
use super::rle;
use super::{
    BANKS_48K, Error, IMAGE_LEN_48K, Snapshot, bank_for_page, frame_position, store_image,
};
use crate::memory::{BankIndex, PAGE_SIZE};
use crate::screen::Colour;
use crate::timing::T_STATES_PER_FRAME;

/// Bytes in the header every version begins with.
const V1_HEADER_LEN: usize = 30;

/// The additional-header length that means version 2.
const V2_EXTRA_HEADER_LEN: u16 = 23;

/// The additional-header length version 3 normally declares.
const V3_EXTRA_HEADER_LEN: u16 = 54;

/// The additional-header length a version 3 file declares when it also carries the last
/// `OUT` to port `0x1FFD`, which only a +3 has.
const V3_EXTRA_HEADER_LEN_WITH_1FFD: u16 = 55;

/// Sound-chip registers the additional header reserves room for, at offsets 39–54.
const AY_REGISTER_COUNT: usize = 16;

/// The page length that means "16384 bytes, stored uncompressed".
///
/// It is **not** a length of 65535, and reading it as one is the kind of mistake that only
/// shows up on a page a compressor happened to leave raw.
const UNCOMPRESSED_PAGE: u16 = 0xFFFF;

/// Which version an additional-header length declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    /// Version 2.01: 23 bytes of additional header, and no frame position.
    V2,
    /// Version 3.0: 54 or 55 bytes, and a frame position.
    V3,
}

impl Version {
    /// The version `length` declares, from the format description's own three values.
    const fn from_extra_header_len(length: u16) -> Result<Self, Error> {
        match length {
            V2_EXTRA_HEADER_LEN => Ok(Self::V2),
            V3_EXTRA_HEADER_LEN | V3_EXTRA_HEADER_LEN_WITH_1FFD => Ok(Self::V3),
            other => Err(Error::UnsupportedVersion {
                extra_header_len: other,
            }),
        }
    }

    /// Whether hardware mode `mode` is a 48K **under this version's numbering**.
    ///
    /// The two versions renumbered the modes, and the overlap is a trap: mode 3 is a
    /// **128K** in version 2 and a *48K with a MGT interface* in version 3. Accepting 3
    /// unconditionally would load a 128 snapshot as a 48K and quietly lose five banks.
    ///
    /// Modes 1 and 3 are 48Ks with a peripheral whose ROM this crate does not model; their
    /// RAM is a 48K's RAM, so they load, and the peripheral is ignored rather than refused.
    const fn is_48k(self, mode: u8) -> bool {
        match self {
            // 0 = 48K, 1 = 48K + Interface I, 2 = SamRam, 3 = 128K, 4 = 128K + Interface I.
            Self::V2 => matches!(mode, 0 | 1),
            // 0 = 48K, 1 = 48K + Interface I, 2 = SamRam, 3 = 48K + M.G.T., 4 = 128K, ...
            Self::V3 => matches!(mode, 0 | 1 | 3),
        }
    }
}

/// Read a `.z80` snapshot of any version this crate supports.
///
/// # Errors
///
/// [`Error`], naming the offset or the byte that failed. Every failure is a returned value:
/// this function does not panic on any input, which
/// `crates/spectrum/tests/snapshot_hostile.rs` demonstrates by an exhaustive truncation
/// sweep and a property test over arbitrary bytes.
pub fn parse(bytes: &[u8]) -> Result<Snapshot, Error> {
    let mut reader = Reader::new(bytes);
    let header = Header::read(&mut reader)?;
    if header.pc == 0 {
        parse_v2_or_v3(&header, reader)
    } else {
        parse_v1(&header, reader)
    }
}

/// Serialise `snapshot` as a version 3 file with compressed pages.
///
/// The result is *canonical*: the same `Snapshot` always produces the same bytes, which is
/// what makes `write(parse(f)) == f` a meaningful check for an `f` this function produced.
/// It is **not** byte-identical to a foreign file that held the same state, and wanting it
/// to be would force [`Snapshot`] to carry bits that are not machine state — see
/// `docs/M6.md` Decision 7.
#[must_use]
pub fn write(snapshot: &Snapshot) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_header(&mut bytes, snapshot);
    for entry in &BANKS_48K {
        if let Some(page) = snapshot.bank(BankIndex::new(entry.bank)) {
            write_page_block(&mut bytes, entry.page, page);
        }
    }
    bytes
}

/// The 30 bytes every version starts with, decoded.
struct Header {
    /// The CPU state the header carries. `pc` is provisional: in version 2 and 3 the field
    /// at offset 6 is a sentinel and the real value lives at offset 32.
    cpu: CpuState,
    /// The word at offset 6. **Zero means version 2 or 3**, and is why this is kept apart.
    pc: u16,
    /// The border colour, from bits 1–3 of byte 12.
    border: Colour,
    /// Bit 5 of byte 12 — whether a version 1 memory block is run-length encoded.
    compressed: bool,
}

impl Header {
    /// Read the shared 30-byte header, in file order.
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let a = reader.u8()?; // 0
        let f = reader.u8()?; // 1
        let bc = reader.u16_le()?; // 2
        let hl = reader.u16_le()?; // 4
        let pc = reader.u16_le()?; // 6 — zero is the version 2/3 sentinel
        let sp = reader.u16_le()?; // 8
        let i = reader.u8()?; // 10
        let refresh = reader.u8()?; // 11 — bit 7 lives in byte 12 instead
        let flags = reader.u8()?; // 12
        let de = reader.u16_le()?; // 13 — *not* at 6; the pairs are not in register order
        let bc_shadow = reader.u16_le()?; // 15
        let de_shadow = reader.u16_le()?; // 17
        let hl_shadow = reader.u16_le()?; // 19
        let a_shadow = reader.u8()?; // 21
        let f_shadow = reader.u8()?; // 22
        let iy = reader.u16_le()?; // 23 — IY comes *before* IX
        let ix = reader.u16_le()?; // 25
        let iff1 = reader.u8()?; // 27 — 0 means DI, anything else means EI
        let iff2 = reader.u8()?; // 28
        let modes = reader.u8()?; // 29 — interrupt mode in bits 0-1

        // "If byte 12 is 255, it has to be regarded as being 1" — a compatibility hack for
        // files written by a version that used 255 as a field terminator.
        let flags = if flags == 0xFF { 0x01 } else { flags };

        let mode = modes & 0b11;
        let im = InterruptMode::try_from(mode)
            .map_err(|_| Error::InvalidInterruptMode { value: mode })?;

        Ok(Self {
            cpu: CpuState {
                af: word(a, f),
                bc,
                de,
                hl,
                af_shadow: word(a_shadow, f_shadow),
                bc_shadow,
                de_shadow,
                hl_shadow,
                ix,
                iy,
                sp,
                pc,
                i,
                // Bit 7 of `R` is bit 0 of byte 12, because byte 11 was documented as
                // holding only seven significant bits. Forgetting this bit is the archetype
                // of a defect a symmetric round trip hides: a parser that drops it and a
                // writer that never sets it agree perfectly and lose the top bit of `R`.
                r: (refresh & 0x7F) | ((flags & 0x01) << 7),
                iff1: iff1 != 0,
                iff2: iff2 != 0,
                im,
                // Neither format carries a halt flag; see `snapshot::UNPRESERVED`.
                halted: false,
                wz: 0,
                // Loading a state is a `POP AF`, so the latch must equal `F`.
                q: f,
            },
            pc,
            border: Colour::new((flags >> 1) & 0b111),
            compressed: flags & 0x20 != 0,
        })
    }
}

/// Assemble a 16-bit register from the two bytes the header stores apart.
const fn word(high: u8, low: u8) -> u16 {
    ((high as u16) << 8) | (low as u16)
}

/// Read the memory of a version 1 file: one block holding all 48 KB in address order.
fn parse_v1(header: &Header, mut reader: Reader<'_>) -> Result<Snapshot, Error> {
    let mut image = Box::new([0_u8; IMAGE_LEN_48K]);
    let block_offset = reader.offset();

    if header.compressed {
        let mut writer = Writer::new(image.as_mut_slice());
        rle::expand(&mut reader, &mut writer, block_offset)?;

        let marker_offset = reader.offset();
        if reader.take(rle::V1_END_MARKER.len())? != rle::V1_END_MARKER {
            return Err(Error::MissingEndMarker {
                offset: marker_offset,
            });
        }
    } else {
        image.copy_from_slice(reader.take(IMAGE_LEN_48K)?);
    }
    // Strict, and the same ruling as a short decompression: bytes we cannot explain mean we
    // have misparsed something, and saying so beats loading a machine that might be wrong.
    reader.finish()?;

    // Version 1 has no T-state field, so the frame position is the top of the frame.
    let mut snapshot = Snapshot::new(header.cpu, header.border, 0);
    store_image(&mut snapshot, image.as_slice());
    Ok(snapshot)
}

/// Read the additional header and the page blocks of a version 2 or 3 file.
fn parse_v2_or_v3(header: &Header, mut reader: Reader<'_>) -> Result<Snapshot, Error> {
    let extra_len = reader.u16_le()?; // 30
    let version = Version::from_extra_header_len(extra_len)?;
    let extra_offset = reader.offset();
    let mut extra = Reader::at(reader.take(usize::from(extra_len))?, extra_offset);

    let pc = extra.u16_le()?; // 32 — the real program counter
    let hardware = extra.u8()?; // 34
    if !version.is_48k(hardware) {
        return Err(Error::UnsupportedHardware { mode: hardware });
    }

    // 35-54 describe hardware a 48K does not have. They are read by name rather than
    // skipped by a computed distance, so the code says what it is stepping over.
    let _paging = extra.u8()?; // 35: last OUT to 0x7FFD, on a 128
    let _interface_1 = extra.u8()?; // 36: 0xFF if the Interface I ROM is paged
    let _emulation = extra.u8()?; // 37: R-register and LDIR emulation flags
    let _ay_selected = extra.u8()?; // 38: last OUT to 0xFFFD
    extra.skip(AY_REGISTER_COUNT)?; // 39-54: the sound chip's registers

    let frame_t_state = match version {
        // The 23-byte additional header ends at offset 54, one byte short of the low word.
        Version::V2 => 0,
        Version::V3 => {
            let low = extra.u16_le()?; // 55
            let high = extra.u8()?; // 57
            decode_t_states(low, high)
        }
    };

    let mut cpu = header.cpu;
    cpu.pc = pc;
    let mut snapshot = Snapshot::new(cpu, header.border, frame_t_state);
    while !reader.is_empty() {
        read_page_block(&mut reader, &mut snapshot)?;
    }
    Ok(snapshot)
}

/// Read one version 2/3 page block: a length word, a page number, and the data.
fn read_page_block(reader: &mut Reader<'_>, snapshot: &mut Snapshot) -> Result<(), Error> {
    let length = reader.u16_le()?;
    let number = reader.u8()?;
    let bank = bank_for_page(number).ok_or(Error::UnknownPage { page: number })?;
    // The alternative is a silent last-write-wins, which loads a machine built from two
    // different snapshots of the same bank.
    if snapshot.bank(bank).is_some() {
        return Err(Error::DuplicatePage { page: number });
    }

    let data_offset = reader.offset();
    let mut page = Box::new([0_u8; PAGE_SIZE]);
    if length == UNCOMPRESSED_PAGE {
        page.copy_from_slice(reader.take(PAGE_SIZE)?);
    } else {
        let mut block = Reader::at(reader.take(usize::from(length))?, data_offset);
        let mut writer = Writer::new(page.as_mut_slice());
        rle::expand(&mut block, &mut writer, data_offset)?;
        block.finish()?;
    }
    snapshot.set_bank(bank, page);
    Ok(())
}

/// Emit the 30-byte header and the 54-byte version 3 additional header.
fn write_header(bytes: &mut Vec<u8>, snapshot: &Snapshot) {
    let cpu = &snapshot.cpu;
    let (low_t_state, high_t_state) = encode_t_states(snapshot.frame_t_state);
    // The shared header, the two-byte length field, and the additional header it declares.
    bytes.reserve(V1_HEADER_LEN + 2 + usize::from(V3_EXTRA_HEADER_LEN));

    bytes.push(high_byte(cpu.af)); // 0: A
    bytes.push(low_byte(cpu.af)); // 1: F
    bytes.extend_from_slice(&cpu.bc.to_le_bytes()); // 2
    bytes.extend_from_slice(&cpu.hl.to_le_bytes()); // 4
    bytes.extend_from_slice(&0_u16.to_le_bytes()); // 6: the version 2/3 sentinel
    bytes.extend_from_slice(&cpu.sp.to_le_bytes()); // 8
    bytes.push(cpu.i); // 10
    bytes.push(cpu.r & 0x7F); // 11: seven significant bits
    // 12: bit 0 is the eighth bit of `R`, bits 1-3 the border. Bit 5 says a *version 1*
    // block is compressed, and stays clear: from version 2 on, each page declares its own
    // length and this reader does not consult the flag either.
    bytes.push(((cpu.r >> 7) & 0x01) | (snapshot.border.index() << 1));
    bytes.extend_from_slice(&cpu.de.to_le_bytes()); // 13
    bytes.extend_from_slice(&cpu.bc_shadow.to_le_bytes()); // 15
    bytes.extend_from_slice(&cpu.de_shadow.to_le_bytes()); // 17
    bytes.extend_from_slice(&cpu.hl_shadow.to_le_bytes()); // 19
    bytes.push(high_byte(cpu.af_shadow)); // 21: A'
    bytes.push(low_byte(cpu.af_shadow)); // 22: F'
    bytes.extend_from_slice(&cpu.iy.to_le_bytes()); // 23: IY first
    bytes.extend_from_slice(&cpu.ix.to_le_bytes()); // 25
    bytes.push(u8::from(cpu.iff1)); // 27
    bytes.push(u8::from(cpu.iff2)); // 28
    bytes.push(u8::from(cpu.im)); // 29: mode in bits 0-1, the rest emulator preferences

    bytes.extend_from_slice(&V3_EXTRA_HEADER_LEN.to_le_bytes()); // 30
    bytes.extend_from_slice(&cpu.pc.to_le_bytes()); // 32
    bytes.push(HARDWARE_MODE_48K); // 34
    bytes.push(0); // 35: last OUT to 0x7FFD — a 48K has no paging port
    bytes.push(0); // 36: no Interface I
    bytes.push(0); // 37: no R or LDIR emulation
    bytes.push(0); // 38: last OUT to 0xFFFD — a 48K has no sound chip
    bytes.extend_from_slice(&[0; AY_REGISTER_COUNT]); // 39-54
    bytes.extend_from_slice(&low_t_state.to_le_bytes()); // 55
    bytes.push(high_t_state); // 57
    // 58-85: Spectator, MGT, Multiface, the ROM/RAM flags and the joystick maps. All of it
    // describes hardware or an emulator preference, none of it is a 48K's state.
    bytes.extend_from_slice(&[0; TRAILING_EXTRA_HEADER_LEN]);
}

/// Hardware mode 0, in both version 2's and version 3's numbering: a plain 48K.
const HARDWARE_MODE_48K: u8 = 0;

/// Additional-header bytes after the T-state counter, offsets 58–85.
const TRAILING_EXTRA_HEADER_LEN: usize = 28;

/// Emit one page block: the compressed length (or the raw marker), the page number, the data.
fn write_page_block(bytes: &mut Vec<u8>, page: u8, contents: &[u8; PAGE_SIZE]) {
    let compressed = rle::compress(contents);
    // A page whose encoding is no smaller than the page is stored raw. That is not only a
    // size choice: it also keeps the declared length below `UNCOMPRESSED_PAGE`, so the two
    // meanings of the length word can never collide.
    if compressed.len() < PAGE_SIZE {
        // INVARIANT: below `PAGE_SIZE` (0x4000), so the cast is lossless.
        bytes.extend_from_slice(&(compressed.len() as u16).to_le_bytes());
        bytes.push(page);
        bytes.extend_from_slice(&compressed);
    } else {
        bytes.extend_from_slice(&UNCOMPRESSED_PAGE.to_le_bytes());
        bytes.push(page);
        bytes.extend_from_slice(contents);
    }
}

/// The high byte of a register pair.
const fn high_byte(pair: u16) -> u8 {
    (pair >> 8) as u8
}

/// The low byte of a register pair.
const fn low_byte(pair: u16) -> u8 {
    (pair & 0xFF) as u8
}

/// Quarters of a frame the T-state counter divides the frame into.
const QUARTERS_PER_FRAME: u32 = 4;

/// T-states in one quarter of a frame: 17472 on a 48K.
const QUARTER_T_STATES: u32 = T_STATES_PER_FRAME / QUARTERS_PER_FRAME;

// The format description says the low counter runs "from 17471 to 0", which is 17472 values,
// and that four of them "make a total of 69888 T states per frame". Both halves have to hold
// or the encoding does not tile the frame.
const _: () = assert!(QUARTER_T_STATES * QUARTERS_PER_FRAME == T_STATES_PER_FRAME);
const _: () = assert!(QUARTER_T_STATES == 17472);

/// Encode a frame position as the version 3 counter pair, `(low, high)`.
///
/// # The rule, quoted
///
/// > *"The hi T state counter counts up modulo 4. Just after the ULA generates its
/// > once-in-every-20-ms interrupt, it is 3, and is increased by one every 5 emulated
/// > milliseconds. In these 1/200s intervals, the low T state counter counts down from 17471
/// > to 0 (17726 in 128K modes), which make a total of 69888 (70908) T states per frame."*
///
/// So the frame is four quarters of 17472 T-states; the low word counts **down** within the
/// quarter, and the high byte is the quarter's index shifted by three because it reads 3 at
/// the top of the frame rather than 0.
///
/// # How this was settled, since its own round trip cannot settle it
///
/// An exhaustive sweep of all 69888 positions grades this function against
/// [`decode_t_states`] and nothing else — if both halves encode the same wrong scheme it is
/// green and every file we write is unreadable elsewhere. Two things break that symmetry, and
/// they agree:
///
/// 1. **Three positions computed by hand from the sentence above**, not from this code, and
///    asserted in `the_counter_matches_the_format_descriptions_own_sentence`: position 0 is
///    *just after the interrupt*, so `high` is 3 and `low` is at the top of its countdown,
///    17471; position 17472 begins the next 5 ms interval, so `high` has advanced to 0; and
///    position 69887 is the last T-state, so `low` has counted down to 0 with `high` at 2.
/// 2. **`libspectrum`, the FUSE project's snapshot library**, computes
///    `low = quarter_states - (tstates % quarter_states) - 1` and
///    `high = ((tstates / quarter_states) + 3) % 4`, and reads back
///    `(((high + 1) % 4) + 1) * quarter_states - (low + 1)`. That is an **independent
///    implementation** of the same paragraph, and the expressions below are algebraically
///    identical to it.
///
/// Both are in the suite, and both were **proven able to fail**: shifting the high byte's
/// origin from three to zero in *both* directions leaves the exhaustive sweep green over all
/// 69888 positions and turns those two red.
///
/// **What does *not* settle it, measured rather than assumed:** the third-party corpus. Under
/// that same mutation `tests/snapshot_corpus.rs` stayed **green**, because everything it
/// asserts about a foreign file — that it parses, and that
/// `parse(write(parse(f))) == parse(f)` — is symmetric in exactly the way the formula is. A
/// foreign file proves the field is *readable*, not that our arithmetic on it is right. The
/// only instrument that would is a human loading one of our files in another emulator, which
/// is observation, is not automated, and has not been done.
const fn encode_t_states(frame_t_state: u32) -> (u16, u8) {
    let position = frame_position(frame_t_state);
    let quarter = position / QUARTER_T_STATES;
    // The low word counts down, so it is the room left in this quarter.
    let low = QUARTER_T_STATES - 1 - (position % QUARTER_T_STATES);
    // INVARIANT: `low <= 17471` and `quarter <= 3`, so both casts are lossless.
    (low as u16, ((quarter + 3) % QUARTERS_PER_FRAME) as u8)
}

/// Decode the version 3 counter pair back into a frame position.
///
/// Total over every `(low, high)` a file can hold, including the ones no conformant writer
/// produces: `high` is taken modulo 4 as the format describes, and a `low` beyond the
/// quarter's length is folded back into the frame rather than underflowing. Under
/// `overflow-checks = true` an underflow here would be an abort, so the addition below is
/// what makes the subtraction total rather than a comment promising it cannot happen.
const fn decode_t_states(low: u16, high: u8) -> u32 {
    let quarter = ((high as u32) + 1) % QUARTERS_PER_FRAME;
    let count_up_from = quarter * QUARTER_T_STATES + (QUARTER_T_STATES - 1);
    // `count_up_from` is at most 69887 and `low` at most 65535, so one whole frame of
    // headroom is enough for the subtraction to stay non-negative, and the remainder
    // removes it again.
    (count_up_from + T_STATES_PER_FRAME - (low as u32)) % T_STATES_PER_FRAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counter_matches_the_format_descriptions_own_sentence() {
        // Three positions worked out by hand from the quoted paragraph, with no reference to
        // `encode_t_states`. This is the only assertion in the file that can see a wrong
        // formula; the exhaustive sweep below cannot.
        let cases = [
            // "Just after the ULA generates its once-in-every-20-ms interrupt, it is 3" —
            // and the low counter is at the top of its countdown.
            (0_u32, 17471_u16, 3_u8),
            // "increased by one every 5 emulated milliseconds", counting up modulo 4, so the
            // second quarter reads 0 and its countdown restarts.
            (QUARTER_T_STATES, 17471, 0),
            (2 * QUARTER_T_STATES, 17471, 1),
            (3 * QUARTER_T_STATES, 17471, 2),
            // "counts down from 17471 to 0": the last T-state of the frame is the bottom of
            // the fourth quarter's countdown.
            (T_STATES_PER_FRAME - 1, 0, 2),
            // One T-state into the frame is one step down from the top.
            (1, 17470, 3),
        ];
        for (position, low, high) in cases {
            assert_eq!(
                encode_t_states(position),
                (low, high),
                "frame position {position}"
            );
            assert_eq!(decode_t_states(low, high), position);
        }
    }

    #[test]
    fn the_counter_agrees_with_libspectrum_over_the_whole_frame() {
        // The FUSE project's snapshot library, transcribed as its own expressions rather
        // than as a call to ours. An independent implementation of the same paragraph is a
        // stronger check than our own inverse, because it cannot share our misreading.
        for position in 0..T_STATES_PER_FRAME {
            let quarter_states = i64::from(T_STATES_PER_FRAME) / 4;
            let tstates = i64::from(position);
            let libspectrum_low = quarter_states - (tstates % quarter_states) - 1;
            let libspectrum_high = ((tstates / quarter_states) + 3) % 4;
            assert_eq!(
                encode_t_states(position),
                (libspectrum_low as u16, libspectrum_high as u8),
                "frame position {position}"
            );

            let read_back =
                (((libspectrum_high + 1) % 4) + 1) * quarter_states - (libspectrum_low + 1);
            assert_eq!(i64::from(position), read_back);
        }
    }

    #[test]
    fn the_counter_survives_every_frame_position() {
        // Exhaustive on the one axis there is, and cheap. It grades the pair against each
        // other and **cannot tell you the formula is right** — which is why the two tests
        // above exist and why this one is not the evidence.
        for position in 0..T_STATES_PER_FRAME {
            let (low, high) = encode_t_states(position);
            assert_eq!(decode_t_states(low, high), position, "position {position}");
            assert!(low <= 17471, "position {position} encoded low {low}");
            assert!(high < 4, "position {position} encoded high {high}");
        }
    }

    #[test]
    fn every_counter_a_file_can_hold_decodes_to_a_frame_position() {
        // Hostile input: `low` can be anything a `u16` holds and `high` anything a `u8`
        // does. Under `overflow-checks = true` the obvious subtraction aborts; this asserts
        // that it does not, over the whole space rather than over the legal part of it.
        for high in 0..=u8::MAX {
            for low in 0..=u16::MAX {
                let position = decode_t_states(low, high);
                assert!(position < T_STATES_PER_FRAME, "low {low} high {high}");
            }
        }
    }

    #[test]
    fn the_high_counter_is_read_modulo_four() {
        // "counts up modulo 4" — so a file holding 7 means the same as one holding 3.
        for high in 0..=u8::MAX {
            assert_eq!(
                decode_t_states(1234, high),
                decode_t_states(1234, high % 4),
                "high {high}"
            );
        }
    }

    #[test]
    fn the_versions_disagree_about_hardware_mode_three() {
        // Mode 3 is a 128K in version 2 and a 48K with a MGT interface in version 3.
        // Accepting it unconditionally would load a 128 snapshot as a 48K.
        assert!(!Version::V2.is_48k(3), "mode 3 is a 128K in version 2");
        assert!(
            Version::V3.is_48k(3),
            "mode 3 is a 48K + M.G.T. in version 3"
        );
        for mode in [0, 1] {
            assert!(Version::V2.is_48k(mode));
            assert!(Version::V3.is_48k(mode));
        }
        for mode in [2, 4, 5, 6, 7, 9, 12, 255] {
            assert!(!Version::V2.is_48k(mode), "v2 mode {mode}");
            assert!(!Version::V3.is_48k(mode), "v3 mode {mode}");
        }
    }

    #[test]
    fn the_additional_header_length_names_the_version() {
        assert_eq!(Version::from_extra_header_len(23), Ok(Version::V2));
        assert_eq!(Version::from_extra_header_len(54), Ok(Version::V3));
        assert_eq!(Version::from_extra_header_len(55), Ok(Version::V3));
        for length in [0, 1, 22, 24, 53, 56, 0xFFFF] {
            assert_eq!(
                Version::from_extra_header_len(length),
                Err(Error::UnsupportedVersion {
                    extra_header_len: length
                })
            );
        }
    }

    #[test]
    fn the_written_header_is_the_length_the_format_describes() {
        // A version 3 header is 30 shared bytes, a 2-byte length word, and 54 more.
        let snapshot = Snapshot::new(CpuState::default(), Colour::BLACK, 0);
        let bytes = write(&snapshot);
        assert_eq!(
            bytes.len(),
            V1_HEADER_LEN + 2 + usize::from(V3_EXTRA_HEADER_LEN),
            "a snapshot with no banks is exactly its header"
        );
        // Every declared byte must be accounted for by a named field, or the writer has
        // silently shifted everything after the gap.
        const PC: usize = 2; // 32-33
        const SINGLE_BYTES: usize = 5; // 34 hardware, 35 paging, 36 If.1, 37 flags, 38 AY select
        const T_STATE_COUNTER: usize = 3; // 55-56 low, 57 high
        assert_eq!(
            PC + SINGLE_BYTES + AY_REGISTER_COUNT + T_STATE_COUNTER + TRAILING_EXTRA_HEADER_LEN,
            usize::from(V3_EXTRA_HEADER_LEN),
        );
    }

    #[test]
    fn the_t_state_counter_is_written_at_offsets_fifty_five_to_fifty_seven() {
        // The accounting above says the fields add up; this says where one of them landed.
        // The offsets are the format description's, and the value is the one
        // `the_counter_matches_the_format_descriptions_own_sentence` derives by hand.
        let mut snapshot = Snapshot::new(CpuState::default(), Colour::BLACK, 0);
        snapshot.frame_t_state = 1;
        let bytes = write(&snapshot);
        assert_eq!(bytes.get(55..58), Some(&[0x3E, 0x44, 0x03][..]));
        assert_eq!(
            u16::from_le_bytes([0x3E, 0x44]),
            17470,
            "one step down from 17471"
        );
    }
}
