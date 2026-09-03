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
//! It is not a plain frame position: the frame is four quarters, the low word counts *down*
//! within the quarter, and the high byte is the quarter's index shifted by three because it
//! reads 3 at the top of the frame. `encode_t_states` in this module carries the derivation,
//! the format description's own sentence, and the two independent checks that settle it —
//! because its own exhaustive round trip provably cannot.
//!
//! **And the quarter is a different length on each machine** — 17472 T-states on a 48K, 17727
//! on a 128 — which this file had as one constant until the sentence's own parenthesis was
//! read properly. A 128's position was written against a 48K's quarters and read back the same
//! way, so every round trip agreed and every file disagreed with every other emulator.

use ::z80::{CpuState, InterruptMode};

use super::reader::{Reader, Writer};
use super::rle;
use super::{
    Error, IMAGE_LEN_48K, Snapshot, bank_for_page_of, frame_position, page_for_bank_128, pages_of,
    store_image,
};
use crate::ay::{self, Ay};
use crate::memory::{BankIndex, PAGE_SIZE};
use crate::model::Model;
use crate::screen::Colour;

/// Bytes in the header every version begins with.
const V1_HEADER_LEN: usize = 30;

/// The additional-header length that means version 2.
const V2_EXTRA_HEADER_LEN: u16 = 23;

/// The additional-header length version 3 normally declares.
const V3_EXTRA_HEADER_LEN: u16 = 54;

/// The additional-header length a version 3 file declares when it also carries the last
/// `OUT` to port `0x1FFD`, which only a +3 has.
const V3_EXTRA_HEADER_LEN_WITH_1FFD: u16 = 55;

/// Everything [`write`] emits before the first page block: the shared header, the two-byte
/// length field, and the additional header that field declares.
///
/// A widening `as` rather than `usize::from`, which is not a `const fn`; `u16` into `usize` is
/// lossless on every target this builds for, `wasm32` included.
const HEADER_LEN: usize = V1_HEADER_LEN + 2 + V3_EXTRA_HEADER_LEN as usize;

/// The largest a page block can be: the two-byte length, the page number, and a page stored
/// **uncompressed**. `write_page_block`'s fallback is what makes this a bound rather than a
/// guess — nothing it emits can exceed it, so it is what [`write`] sizes its buffer from.
const BLOCK_LEN: usize = 2 + 1 + PAGE_SIZE;

/// Sound-chip registers the additional header reserves room for, at offsets 39–54.
///
/// # Sixteen here, and the chip has fifteen
///
/// **The format reserves one byte more than the machine has a register for.** The 128's
/// AY-3-8912 is a cut-down `-8910` with one I/O port rather than two, so `R15` does not exist
/// — [`crate::ay`] carries the sourcing — while `.z80` version 3 reserves sixteen bytes
/// regardless. `docs/M7.md` Decision 6 names the hazard exactly: whatever a model puts in the
/// sixteenth *"round-trips perfectly and is invisible to every round trip"*, so it has to be
/// decided rather than left to fall out of an array length.
///
/// **The ruling, made with the chip and recorded here where the bytes are: the writer emits
/// zero at offset 54, and the reader discards it.** The byte describes a register the machine
/// does not have, so there is no value to record — and zero is already what this writer emits
/// for every other field describing absent hardware (offsets 35–38 on a 48K, and 58–85 on
/// both machines).
///
/// **It cannot be gated by a round trip** and must be gated by a transcribed vector, because
/// a round trip over a file *we* wrote compares our zero against our zero. `docs/M7.md` says
/// so and `crates/spectrum/tests/m7_ay_ports.rs` grades the chip half — that `R15` is absent
/// and a guest's read of it floats — from outside any round trip.
///
/// This constant stays at sixteen: it is the **format's** count and the format really does
/// reserve sixteen. [`crate::ay::REGISTER_COUNT`] is the chip's, and the two being different
/// numbers with different names is the point rather than an inconsistency.
const AY_REGISTER_COUNT: usize = 16;

// The two counts must differ by exactly the one register the format has and the chip does
// not. An edit that quietly reconciled them would erase the distinction this file's comment
// exists to preserve.
const _: () = assert!(AY_REGISTER_COUNT == crate::ay::REGISTER_COUNT + 1);

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

    /// The machine hardware mode `mode` names **under this version's numbering**.
    ///
    /// The two versions renumbered the modes, and the overlap is the trap `docs/M6.md`
    /// flagged and `docs/M7.md` calls the highest-risk line in the parser: **mode 3 is a
    /// 128K in version 2 and a 48K with an M.G.T. interface in version 3.** Accepting it
    /// unconditionally loads a 128 snapshot as a 48K and quietly loses five banks.
    ///
    /// Modes 1 and 3 are 48Ks with a peripheral whose ROM this crate does not model; their
    /// RAM is a 48K's RAM, so they load and the peripheral is ignored rather than refused.
    /// The same goes for a 128 with a peripheral.
    ///
    /// # The version 3 128K number has a second witness, which it did not have when it was
    /// transcribed
    ///
    /// `docs/M7.md` warned: *"Transcribe it; do not infer it from the v2 value, because
    /// renumbering is the whole hazard."* It was transcribed from the format description by
    /// M6's implementer. It is now **corroborated from a file nobody here wrote**: the 128
    /// timing suite fetched at M7's step 0, `timing_tests-128k_v1.0.z80`, is a version 3 file
    /// carrying hardware mode **4**, and it is unmistakably a 128 program — its own BASIC
    /// prints `** MUST RUN IN 48k MODE **` and its header's `0x7FFD` is `0x30`. A number read
    /// out of a third party's file is a different lineage from a number read out of a
    /// document.
    const fn model(self, mode: u8) -> Option<Model> {
        match (self, mode) {
            // 0 = 48K, 1 = 48K + Interface I, 2 = SamRam, 3 = 128K, 4 = 128K + Interface I.
            (Self::V2, 0 | 1) => Some(Model::Spectrum48K),
            (Self::V2, 3 | 4) => Some(Model::Spectrum128),
            // 0 = 48K, 1 = 48K + Interface I, 2 = SamRam, 3 = 48K + M.G.T., 4 = 128K,
            // 5 = 128K + Interface I, 6 = 128K + M.G.T.
            (Self::V3, 0 | 1 | 3) => Some(Model::Spectrum48K),
            (Self::V3, 4..=6) => Some(Model::Spectrum128),
            // 2 is SamRam on both, which is a different machine rather than a peripheral.
            _ => None,
        }
    }

    /// The hardware-mode byte this version writes for `model`.
    ///
    /// Only version 3 is written, so only its numbering appears here — but the parameter is
    /// kept rather than assumed, because a writer that hard-coded a number would be the same
    /// silent mislabelling in the other direction.
    const fn hardware_mode(self, model: Model) -> u8 {
        // No `_` arm, deliberately. `Model` is `#[non_exhaustive]` to downstream crates and
        // exhaustive **here**, so a variant added later is a compile error in this function
        // rather than a machine silently written out as a 48K — which is the exact defect this
        // whole table exists to close, one model along.
        match (self, model) {
            (Self::V2 | Self::V3, Model::Spectrum48K) => 0,
            (Self::V2, Model::Spectrum128) => 3,
            (Self::V3, Model::Spectrum128) => 4,
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
    // **Sized once for the worst case rather than grown into.** `Vec::new()` here reached a
    // 128's ~130 KB across about eleven doublings, copying a quarter of a megabyte to write one
    // file; and `write_header`'s `reserve` covered only the header, which looked like it had
    // solved this and had not. The worst case is every page stored uncompressed, which is what
    // `write_page_block` falls back to, so nothing can exceed it. It is deliberately generous:
    // a compressible snapshot leaves the tail unused for the moment between here and the caller
    // writing the bytes out, and that is a better trade than eleven copies.
    let mut bytes = Vec::with_capacity(HEADER_LEN + pages_of(snapshot.model()).len() * BLOCK_LEN);
    write_header(&mut bytes, snapshot);
    // **Every bank the *model* has, not the three a 48K has.** This iterated `BANKS_48K`
    // unconditionally, so a 128 snapshot was written as a 48K file carrying three of its
    // eight banks — five lost, silently, with a header that claimed 48K so nothing
    // downstream could tell. It is the *"silent last-write-wins"* `docs/M6.md` refused for
    // duplicate pages, in the one direction `Spectrum::restore`'s `ModelMismatch` does not
    // cover: the refusal guards a 128 image entering a 48K machine, and nothing guarded a 128
    // machine leaving as a 48K file.
    for (bank, number) in pages_of(snapshot.model()) {
        if let Some(page) = snapshot.bank(bank) {
            write_page_block(&mut bytes, number, page);
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
    let model = version
        .model(hardware)
        .ok_or(Error::UnsupportedHardware { mode: hardware })?;

    // 35-54 describe hardware a 48K does not have. They are read by name rather than
    // skipped by a computed distance, so the code says what it is stepping over.
    let paging = extra.u8()?; // 35: last OUT to 0x7FFD, on a 128
    let _interface_1 = extra.u8()?; // 36: 0xFF if the Interface I ROM is paged
    let _emulation = extra.u8()?; // 37: R-register and LDIR emulation flags
    let ay_selected = extra.u8()?; // 38: last OUT to 0xFFFD
    // Fifteen of the sixteen: `take` borrows the bytes the header really carries, and the
    // sixteenth is stepped over by name below rather than silently included in a slice sized
    // to the format. `R15` is the one the chip does not have.
    let ay_registers: [u8; ay::REGISTER_COUNT] = extra
        .take(ay::REGISTER_COUNT)? // 39-53
        .try_into()
        .map_err(|_| Error::UnsupportedHardware { mode: hardware })?;
    extra.skip(AY_REGISTER_COUNT - ay::REGISTER_COUNT)?; // 54: `R15`, which does not exist

    let frame_t_state = match version {
        // The 23-byte additional header ends at offset 54, one byte short of the low word.
        Version::V2 => 0,
        Version::V3 => {
            let low = extra.u16_le()?; // 55
            let high = extra.u8()?; // 57
            // `model` and not a constant: the quarters this pair counts are a quarter of
            // *this* machine's frame, and the two machines' frames differ by 1020 T-states.
            decode_t_states(model, low, high)
        }
    };

    let mut cpu = header.cpu;
    cpu.pc = pc;
    let mut snapshot = Snapshot::new(cpu, header.border, frame_t_state);
    // The model has to be set **before** the page blocks, because which bank a page number
    // names depends on it — and getting that backwards is the five-lost-banks defect in a
    // different disguise.
    snapshot.set_model(model, paging_for(model, paging));
    if model.has_ay() {
        // Fifteen of the sixteen bytes. The sixteenth describes `R15`, which an AY-3-8912
        // does not have; `AY_REGISTER_COUNT`'s comment carries the ruling and the reason a
        // round trip cannot gate it.
        let mut ay = Ay::new();
        ay.restore(ay_selected, &ay_registers);
        snapshot.set_ay(&ay);
    }
    while !reader.is_empty() {
        read_page_block(&mut reader, &mut snapshot, model)?;
    }
    check_every_bank_is_present(&snapshot, model)?;
    Ok(snapshot)
}

/// Refuse a file that declares a machine and then does not carry all of its banks.
///
/// **The bank-set guard, redesigned as `docs/M7.md` Decision 7 says it must be.** M6 closed the
/// dropped-bank seam by comparing the parser's bank set against *the set the slot map exposes*,
/// and on a 128 that premise dissolves — banks legitimately exist outside the slot map, which is
/// what makes `Memory::bank` unavoidable. So the comparison is against the set the **model** has.
///
/// It is not a formality: a file claiming hardware mode 4 while carrying a 48K's page numbers
/// parses into a 128 snapshot holding three banks of eight, and a restore would then leave five
/// banks as whatever the target machine happened to have — the same silent half-load the
/// `ModelMismatch` refusal exists to prevent, arriving through the parser instead.
///
/// **Scoped to the machines whose loss nothing else can see, which is the whole argument.** On a
/// 48K all three banks are always addressable, so a missing one shows up the moment anything
/// reads that address — M6's slot-map guard is still the right instrument there and this pass
/// does not move it. On a 128 five of eight banks have no address at all, so a missing one is
/// invisible by construction. That asymmetry is exactly why `docs/M7.md` says the guard had to be
/// *redesigned* rather than kept, and it is why this is not simply applied to both.
///
/// It lives here rather than inline in `parse_v2_or_v3` because that function was doing five
/// jobs across ~88 lines, thirty-five of them this argument wrapped around eleven lines of code.
/// The seam was already named in prose before it was a function.
fn check_every_bank_is_present(snapshot: &Snapshot, model: Model) -> Result<(), Error> {
    // A whole-loop guard written as a whole-loop guard. This read
    // `model.banks().iter().filter(|_| model != Model::Spectrum48K)`, whose closure ignores its
    // item and tests a loop invariant — a reader sees `filter` and hunts for a per-item predicate
    // that is not there, and the machine evaluates it once per bank to learn the same thing
    // eight times.
    let must_be_present: &[u8] = match model {
        Model::Spectrum48K => &[],
        Model::Spectrum128 => model.banks(),
    };
    for &bank in must_be_present {
        let bank = BankIndex::new(bank);
        if snapshot.bank(bank).is_none() {
            // The page number straight out of the mapping, rather than a linear search of
            // `pages_of` behind a `map_or(0, …)`. That default was unreachable and it was also
            // **wrong if it were ever reached**: 0 is not a valid page number for either machine
            // — a 48K uses 4, 5 and 8, a 128 uses 3 to 10 — so a miss would have put an
            // impossible number in a message a person reads. It allocated a `Vec` per missing
            // bank to do it. This arm is 128-only, and `page_for_bank_128` is exactly what
            // `pages_of` consults, so the answer is total and there is nothing to default.
            return Err(Error::MissingPage {
                page: page_for_bank_128(bank),
            });
        }
    }
    Ok(())
}

/// The paging byte to record for `model`.
///
/// A 48K's `0x7FFD` byte in a file is meaningless — the machine has no paging port — so the
/// machine's own fixed value is used rather than whatever the file happened to carry. That
/// keeps a hostile 48K file from putting a `Snapshot` into a state `Spectrum::snapshot` would
/// never produce, which is what makes `snapshot(restore(s)) == s` exact rather than nearly so.
const fn paging_for(model: Model, from_file: u8) -> u8 {
    match model {
        Model::Spectrum48K => Model::Spectrum48K.paging_port_at_reset(),
        _ => from_file,
    }
}

/// Read one version 2/3 page block: a length word, a page number, and the data.
fn read_page_block(
    reader: &mut Reader<'_>,
    snapshot: &mut Snapshot,
    model: Model,
) -> Result<(), Error> {
    let length = reader.u16_le()?;
    let number = reader.u8()?;
    let bank = bank_for_page_of(model, number).ok_or(Error::UnknownPage { page: number })?;
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
    let model = snapshot.model();
    // The counter divides *this machine's* frame into quarters, and the two machines' frames
    // are different lengths — so the model is read here rather than at offset 34 alone.
    let (low_t_state, high_t_state) = encode_t_states(model, snapshot.frame_t_state);
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
    bytes.push(Version::V3.hardware_mode(model)); // 34
    // A 48K's byte 35 and its sound-chip bytes describe hardware it does not have, and zero
    // is what this writer emits for every such field — offsets 36, 37 and the whole of 58-85
    // included. A 128's are its real state.
    let has_ay = model.has_ay();
    bytes.push(if has_ay { snapshot.paging_port() } else { 0 }); // 35: last OUT to 0x7FFD
    bytes.push(0); // 36: no Interface I
    bytes.push(0); // 37: no R or LDIR emulation
    bytes.push(snapshot.ay().map_or(0, |ay| ay.selected)); // 38: last OUT to 0xFFFD
    // **The chip's fifteen, then the format's sixteenth, written as two statements because they
    // are two rulings.** A 16-iteration loop over `AY_REGISTER_COUNT` folded both into one
    // `unwrap_or(0)`, so the same zero meant *"this machine has no chip"* and *"this is `R15`,
    // which no machine has"* — and `R15` fell out of an array length, which is precisely what
    // `AY_REGISTER_COUNT`'s own comment says must be decided instead.
    let registers = snapshot
        .ay()
        .map_or([0; ay::REGISTER_COUNT], |ay| ay.registers);
    bytes.extend_from_slice(&registers); // 39-53
    bytes.push(0); // 54: `R15`, which the chip does not have
    bytes.extend_from_slice(&low_t_state.to_le_bytes()); // 55
    bytes.push(high_t_state); // 57
    // 58-85: Spectator, MGT, Multiface, the ROM/RAM flags and the joystick maps. All of it
    // describes hardware or an emulator preference, none of it is a 48K's state.
    bytes.extend_from_slice(&[0; TRAILING_EXTRA_HEADER_LEN]);
}

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

/// T-states in one quarter of `model`'s frame: 17472 on a 48K and 17727 on a 128.
///
/// **A function of the model and not a constant, because the sentence it comes from gives two
/// numbers.** This was `T_STATES_PER_FRAME / 4` — the 48K's — for every machine, so a 128's
/// frame position was encoded against a quarter **255 T-states too short**, and the error
/// accumulated one quarter at a time. Our own reader shared the constant, which is precisely
/// why every round trip stayed green while every file we wrote placed a 128 somewhere it had
/// not been, in anybody else's emulator.
const fn quarter_t_states(model: Model) -> u32 {
    model.timing().frame_t_states() / QUARTERS_PER_FRAME
}

// The format description says the low counter runs "from 17471 to 0 (17726 in 128K modes)",
// which is 17472 and 17727 values, and that four of them "make a total of 69888 (70908) T
// states per frame". Both halves have to hold, for both machines, or the encoding does not
// tile the frame — and the 128 row is the one that was missing.
const _: () = assert!(quarter_t_states(Model::Spectrum48K) == 17472);
const _: () = assert!(quarter_t_states(Model::Spectrum128) == 17727);
const _: () = assert!(quarter_t_states(Model::Spectrum48K) * QUARTERS_PER_FRAME == 69888);
const _: () = assert!(quarter_t_states(Model::Spectrum128) * QUARTERS_PER_FRAME == 70908);

/// Encode a frame position as the version 3 counter pair, `(low, high)`.
///
/// # The rule, quoted
///
/// > *"The hi T state counter counts up modulo 4. Just after the ULA generates its
/// > once-in-every-20-ms interrupt, it is 3, and is increased by one every 5 emulated
/// > milliseconds. In these 1/200s intervals, the low T state counter counts down from 17471
/// > to 0 (17726 in 128K modes), which make a total of 69888 (70908) T states per frame."*
///
/// So the frame is four quarters — of 17472 T-states on a 48K and **17727 on a 128** — the low
/// word counts **down** within the quarter, and the high byte is the quarter's index shifted by
/// three because it reads 3 at the top of the frame rather than 0.
///
/// # The parenthesis is the whole of the 128, and it was read past
///
/// Every number in that sentence comes in a pair, and this function took the first of each and
/// applied it to both machines: `T_STATES_PER_FRAME / 4`, the 48K's quarter, whatever
/// [`Snapshot::model`] said. Two things followed, and only one of them is visible from inside
/// this crate.
///
/// **A 128 position at or past 69888 was lost.** [`frame_position`] reduced into the 48K frame,
/// so the last 1020 T-states of a 128's frame — an ordinary place for a machine to be, a fifth
/// of the way through its last quarter — wrapped to the top of the frame. That one a round trip
/// *can* see, and `tests/snapshot_apply.rs` now does.
///
/// **Every other 128 position was misplaced for every reader but us**, by `255 x (quarter + 1)`
/// — computed over all 69888 surviving positions, that is exactly +255, +510, +765 or +1020
/// T-states, 17472 positions each, and never zero. Our writer and our reader shared the
/// constant, so `parse(write(s)) == s` was green for all of them; a conformant emulator
/// dividing the same file into 17727-T-state quarters reads a different position out of the
/// same two bytes. Nothing symmetric can see that, which is why the gate for it is the
/// hand-derived one below rather than the sweep.
///
/// # How this was settled, since its own round trip cannot settle it
///
/// An exhaustive sweep over every position of both frames — 69888 on a 48K and 70908 on a
/// 128, each machine graded against its own — pits this function against [`decode_t_states`]
/// and nothing else: if both halves encode the same wrong scheme it is green and every file we
/// write is unreadable elsewhere. Two things break that symmetry, and they agree:
///
/// 1. **Six positions per machine computed by hand from the sentence above**, not from this
///    code, and asserted in `the_counter_matches_the_format_descriptions_own_sentence`:
///    position 0 is *just after the interrupt*, so `high` is 3 and `low` is at the top of its
///    countdown — 17471 on a 48K and **17726 on a 128**; each of the three later quarter
///    boundaries begins the next 5 ms interval, so the countdown restarts at that top with
///    `high` advanced to 0, then 1, then 2; the frame's last T-state — 69887 or **70907** —
///    has `low` counted down to 0 with `high` at 2; and one T-state in is one step down from
///    the top. Both expected values are transcribed from the sentence's two halves by the test
///    module's `frame_of` and `top_of_the_countdown` rather than read out of
///    [`quarter_t_states`], so nothing here is graded by its own subject. That test then
///    asserts the two machines encode position 0 **differently** — the one claim a loop over a
///    single model structurally cannot contain, and the claim whose absence let a 128 divided
///    into a 48K's quarters pass every gate in this file.
/// 2. **`libspectrum`, the FUSE project's snapshot library**, computes
///    `low = quarter_states - (tstates % quarter_states) - 1` and
///    `high = ((tstates / quarter_states) + 3) % 4`, and reads back
///    `(((high + 1) % 4) + 1) * quarter_states - (low + 1)`. That is an **independent
///    implementation** of the same paragraph — with `quarter_states` a parameter of the machine
///    there, as it now is here — and the expressions below are algebraically identical to it.
///
/// Both are in the suite, and both were **proven able to fail** — on the narrower gate they
/// then were, which is the range that verdict belongs to. At M6 the sweep covered the 48K's
/// 69888 positions alone and the hand-worked list was three of them on that machine, and
/// shifting the high byte's origin from three to zero in *both* directions left that sweep
/// green and turned those two red. `docs/M6.md`'s mutation table records the run; nothing
/// records one taken since the gates grew the second machine.
///
/// **What does *not* settle it, measured rather than assumed:** the third-party corpus. Under
/// that same mutation `tests/snapshot_corpus.rs` stayed **green**, because everything it
/// asserts about a foreign file — that it parses, and that
/// `parse(write(parse(f))) == parse(f)` — is symmetric in exactly the way the formula is. A
/// foreign file proves the field is *readable*, not that our arithmetic on it is right. The
/// only instrument that would is a human loading one of our files in another emulator, which
/// is observation, is not automated, and has not been done.
const fn encode_t_states(model: Model, frame_t_state: u32) -> (u16, u8) {
    let quarter_states = quarter_t_states(model);
    let position = frame_position(model, frame_t_state);
    let quarter = position / quarter_states;
    // The low word counts down, so it is the room left in this quarter.
    let low = quarter_states - 1 - (position % quarter_states);
    // INVARIANT: `low <= 17726` and `quarter <= 3`, so both casts are lossless.
    (low as u16, ((quarter + 3) % QUARTERS_PER_FRAME) as u8)
}

/// Decode the version 3 counter pair back into a frame position on `model`.
///
/// Total over every `(low, high)` a file can hold, including the ones no conformant writer
/// produces: `high` is taken modulo 4 as the format describes, and a `low` beyond the
/// quarter's length is folded back into the frame rather than underflowing. Under
/// `overflow-checks = true` an underflow here would be an abort, so the addition below is
/// what makes the subtraction total rather than a comment promising it cannot happen.
///
/// **The model is a parameter for the same reason it is one in [`encode_t_states`], and this
/// is the end that made the defect invisible.** Both ends divided the frame into the 48K's
/// quarters, so they agreed with each other over every 128 file this crate produced and read
/// — the exact shape `docs/M6.md` Decision 7 says a round trip cannot see.
const fn decode_t_states(model: Model, low: u16, high: u8) -> u32 {
    let frame = model.timing().frame_t_states();
    let quarter_states = quarter_t_states(model);
    let quarter = ((high as u32) + 1) % QUARTERS_PER_FRAME;
    let count_up_from = quarter * quarter_states + (quarter_states - 1);
    // `count_up_from` is below the frame and `low` is at most 65535, which is below either
    // machine's frame, so one whole frame of headroom keeps the subtraction non-negative and
    // the remainder removes it again.
    (count_up_from + frame - (low as u32)) % frame
}

// The headroom argument above is only sound while a `u16` cannot exceed a frame. Both frames
// are comfortably over 65535, and a machine whose frame was not would need a different
// expression rather than a larger comment.
const _: () = assert!(Model::Spectrum48K.timing().frame_t_states() > u16::MAX as u32);
const _: () = assert!(Model::Spectrum128.timing().frame_t_states() > u16::MAX as u32);

#[cfg(test)]
mod tests {
    use super::*;

    /// Both machines, because the counter's geometry is per-model and every gate below was
    /// written when it was a constant.
    const MODELS: [Model; 2] = [Model::Spectrum48K, Model::Spectrum128];

    /// The frame length each model's counter divides, transcribed from the format
    /// description's *"69888 (70908) T states per frame"* rather than read from
    /// [`crate::timing`] — so a sweep over it is not bounded by the number it is grading.
    const fn frame_of(model: Model) -> u32 {
        match model {
            Model::Spectrum48K => 69888,
            Model::Spectrum128 => 70908,
        }
    }

    /// The value each model's low counter starts a quarter at, transcribed from the same
    /// sentence: *"counts down from 17471 to 0 (17726 in 128K modes)"*.
    ///
    /// **Read from the sentence and not from [`quarter_t_states`], which is the whole point.**
    /// A first cut at generalising these gates wrote `quarter_t_states(model) - 1` here, and
    /// that is the tautology `snapshot/mod.rs`'s own header warns about — an expectation
    /// computed by its subject, green for any quarter length the encoder happens to use.
    const fn top_of_the_countdown(model: Model) -> u16 {
        match model {
            Model::Spectrum48K => 17471,
            Model::Spectrum128 => 17726,
        }
    }

    // The two transcriptions have to tile: counting down from the top **to zero inclusive** is
    // one more value than the top, and four of those quarters are the frame.
    const _: () = assert!((top_of_the_countdown(Model::Spectrum48K) as u32 + 1) * 4 == 69888);
    const _: () = assert!((top_of_the_countdown(Model::Spectrum128) as u32 + 1) * 4 == 70908);

    #[test]
    fn the_counter_matches_the_format_descriptions_own_sentence() {
        // Six positions per machine, worked out by hand from the quoted paragraph with no
        // reference to `encode_t_states`. This is the only assertion in the file that can see
        // a wrong formula; the exhaustive sweep below cannot.
        //
        // **Every number in that sentence is a pair, and the second of each is the 128's.**
        for model in MODELS {
            let top = top_of_the_countdown(model);
            let quarter = frame_of(model) / QUARTERS_PER_FRAME;
            let cases = [
                // "Just after the ULA generates its once-in-every-20-ms interrupt, it is 3" —
                // and the low counter is at the top of its countdown.
                (0_u32, top, 3_u8),
                // "increased by one every 5 emulated milliseconds", counting up modulo 4, so
                // the second quarter reads 0 and its countdown restarts.
                (quarter, top, 0),
                (2 * quarter, top, 1),
                (3 * quarter, top, 2),
                // "counts down ... to 0": the last T-state of the frame is the bottom of the
                // fourth quarter's countdown.
                (frame_of(model) - 1, 0, 2),
                // One T-state into the frame is one step down from the top.
                (1, top - 1, 3),
            ];
            for (position, low, high) in cases {
                assert_eq!(
                    encode_t_states(model, position),
                    (low, high),
                    "{model} at frame position {position}"
                );
                assert_eq!(decode_t_states(model, low, high), position, "{model}");
            }
        }
        // The two rows must not coincide, or a 128 encoded as a 48K passes both of them —
        // which is exactly what happened, and what the loop above could not have caught while
        // it ran over one model.
        assert_ne!(
            encode_t_states(Model::Spectrum48K, 0),
            encode_t_states(Model::Spectrum128, 0)
        );
    }

    #[test]
    fn the_counter_agrees_with_libspectrum_over_the_whole_frame() {
        // The FUSE project's snapshot library, transcribed as its own expressions rather
        // than as a call to ours. An independent implementation of the same paragraph is a
        // stronger check than our own inverse, because it cannot share our misreading.
        //
        // Its `quarter_states` is a **parameter of the machine** there as here, which is the
        // detail this crate had flattened into a constant.
        for model in MODELS {
            for position in 0..frame_of(model) {
                let quarter_states = i64::from(frame_of(model)) / 4;
                let tstates = i64::from(position);
                let libspectrum_low = quarter_states - (tstates % quarter_states) - 1;
                let libspectrum_high = ((tstates / quarter_states) + 3) % 4;
                assert_eq!(
                    encode_t_states(model, position),
                    (libspectrum_low as u16, libspectrum_high as u8),
                    "{model} at frame position {position}"
                );

                let read_back =
                    (((libspectrum_high + 1) % 4) + 1) * quarter_states - (libspectrum_low + 1);
                assert_eq!(i64::from(position), read_back, "{model}");
            }
        }
    }

    #[test]
    fn the_counter_survives_every_frame_position() {
        // Exhaustive on the one axis there is, and cheap. It grades the pair against each
        // other and **cannot tell you the formula is right** — which is why the two tests
        // above exist and why this one is not the evidence.
        for model in MODELS {
            let top = top_of_the_countdown(model);
            for position in 0..frame_of(model) {
                let (low, high) = encode_t_states(model, position);
                assert_eq!(
                    decode_t_states(model, low, high),
                    position,
                    "{model} at position {position}"
                );
                assert!(low <= top, "{model} at {position} encoded low {low}");
                assert!(high < 4, "{model} at {position} encoded high {high}");
            }
        }
    }

    #[test]
    fn every_counter_a_file_can_hold_decodes_to_a_frame_position() {
        // Hostile input: `low` can be anything a `u16` holds and `high` anything a `u8`
        // does. Under `overflow-checks = true` the obvious subtraction aborts; this asserts
        // that it does not, over the whole space rather than over the legal part of it — and
        // over both machines, since the frame it lands inside is the model's.
        for model in MODELS {
            for high in 0..=u8::MAX {
                for low in 0..=u16::MAX {
                    let position = decode_t_states(model, low, high);
                    assert!(position < frame_of(model), "{model} low {low} high {high}");
                }
            }
        }
    }

    #[test]
    fn the_high_counter_is_read_modulo_four() {
        // "counts up modulo 4" — so a file holding 7 means the same as one holding 3.
        for model in MODELS {
            for high in 0..=u8::MAX {
                assert_eq!(
                    decode_t_states(model, 1234, high),
                    decode_t_states(model, 1234, high % 4),
                    "{model} high {high}"
                );
            }
        }
    }

    #[test]
    fn the_versions_disagree_about_hardware_mode_three() {
        // **The highest-risk line in this parser**, and the whole reason the mode table is
        // per-version. Mode 3 is a 128K in version 2 and a 48K with an M.G.T. interface in
        // version 3, so accepting it unconditionally loads a 128 snapshot as a 48K and loses
        // five banks — and mislabelling it in the *writer* loses them just as quietly, which
        // is the defect this table was extended to close.
        assert_eq!(Version::V2.model(3), Some(Model::Spectrum128));
        assert_eq!(Version::V3.model(3), Some(Model::Spectrum48K));

        // The full table, both versions, as two disjoint lists rather than a rule — because
        // the renumbering means there is no rule.
        for mode in [0, 1] {
            assert_eq!(Version::V2.model(mode), Some(Model::Spectrum48K));
            assert_eq!(Version::V3.model(mode), Some(Model::Spectrum48K));
        }
        for mode in [3, 4] {
            assert_eq!(
                Version::V2.model(mode),
                Some(Model::Spectrum128),
                "v2 {mode}"
            );
        }
        for mode in [4, 5, 6] {
            assert_eq!(
                Version::V3.model(mode),
                Some(Model::Spectrum128),
                "v3 {mode}"
            );
        }
        // SamRam on both, and everything above the table on neither.
        for mode in [2, 7, 9, 12, 255] {
            assert_eq!(Version::V2.model(mode), None, "v2 mode {mode}");
            assert_eq!(Version::V3.model(mode), None, "v3 mode {mode}");
        }
    }

    #[test]
    fn the_mode_the_writer_emits_is_the_mode_the_reader_reads_back() {
        // The defect, as its narrowest possible failing case: the writer emitted a constant
        // `0` for every model, so a 128 came back as a 48K. This is what would have caught it
        // before anything wrote a file.
        for model in [Model::Spectrum48K, Model::Spectrum128] {
            for version in [Version::V2, Version::V3] {
                assert_eq!(
                    version.model(version.hardware_mode(model)),
                    Some(model),
                    "{version:?} {model}"
                );
            }
        }
        // And the two versions really do disagree about which byte that is, which is what
        // makes a shared constant wrong rather than merely inelegant.
        assert_ne!(
            Version::V2.hardware_mode(Model::Spectrum128),
            Version::V3.hardware_mode(Model::Spectrum128)
        );
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
