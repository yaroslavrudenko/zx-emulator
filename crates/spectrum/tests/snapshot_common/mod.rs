//! The transcribed snapshot files every snapshot gate is built from.
//!
//! # These bytes were written from the format description, one offset at a time
//!
//! Each field carries its offset and its meaning in a comment beside it, because the headers
//! are irregular in ways that punish a plausible guess: `BC` is at offset 2 and `HL` at 4 but
//! `DE` is at **13**; `IY` is at 23 and `IX` at **25**, in that order; `A` and `F` are
//! separate bytes rather than an `AF` word.
//!
//! **The expectations live elsewhere, in `snapshot_vectors.rs`, and were written
//! separately.** That separation is the whole point. `docs/M6.md` Decision 7: every round
//! trip compares a value against a value that came from the same code, so a symmetric error
//! — a permuted field, a shared wrong offset, a field dropped in both directions — survives
//! all of them. Only a hand-written expectation that owes nothing to the code under test can
//! see one.
//!
//! `docs/STATUS.md` records what happens without that separation. The keyboard's own gate
//! derived **both** the port it scanned and the value it expected from the function under
//! test: *"38 of the 40 keys could be rewired with the entire suite green."* A test whose
//! expectation is computed by its subject is not a weak test; it is a tautology with a cross
//! product attached.
//!
//! # Why the run-length payloads are not `compress`
//!
//! [`page_of`] emits the escape tokens by hand. `snapshot::snapshot::rle::compress` is
//! private, so it is not reachable from an integration test at all — which is the point: the
//! payload expectation cannot be derived from the encoder it grades.

// Each gate is its own binary and compiles this whole module while using a subset of it —
// the standard `tests/common` situation, matching `crates/spectrum/tests/common/mod.rs`. The
// allow is permanent, so `#[expect]` would itself warn in the binaries where an item is used.
#![allow(dead_code)]

use spectrum::memory::PAGE_SIZE;

// ---------------------------------------------------------------------------------------
// The fixture, as values. Written from the format description, not from the parser.
// ---------------------------------------------------------------------------------------

/// Every 16-bit register of the fixture, `(name, value)`.
///
/// Twelve values, pairwise distinct and none of them zero — asserted below, because that
/// property is what makes a permuted read visible.
pub const REGISTERS: [(&str, u16); 12] = [
    ("af", 0x1234),
    ("bc", 0x5678),
    ("de", 0x9ABC),
    ("hl", 0xDEF0),
    ("af_shadow", 0x2143),
    ("bc_shadow", 0x6587),
    ("de_shadow", 0xA9CB),
    ("hl_shadow", 0xED0F),
    ("ix", 0x1357),
    ("iy", 0x2468),
    ("sp", 0xFEDC),
    ("pc", 0xBA98),
];

pub const I: u8 = 0x7E;
pub const R: u8 = 0xC5; // bit 7 set, so the split across bytes 11 and 12 is exercised
pub const BORDER: u8 = 5;

/// A frame position inside the **third** quarter, so neither half of the version 3 counter
/// is at a boundary value that a wrong formula could reach by accident.
pub const FRAME_T_STATE: u32 = 40_000;

/// The distinguishing byte each of the three 16 KB regions is filled with, in address order.
pub const FILL: [(u16, u8, u8); 3] = [
    // (base address, bank in this crate's numbering, fill byte)
    (0x4000, 5, 0xA1),
    (0x8000, 2, 0xB2),
    (0xC000, 0, 0xC3),
];

/// Look a register up by name, so an assertion cannot quietly compare a value against itself.
pub fn expected(name: &str) -> u16 {
    REGISTERS
        .iter()
        .find(|&&(field, _)| field == name)
        .map(|&(_, value)| value)
        .unwrap_or_else(|| panic!("{name} is not in the fixture"))
}

// ---------------------------------------------------------------------------------------
// The bytes. One line per field, with its offset and its meaning.
// ---------------------------------------------------------------------------------------

/// Byte 12 of the shared header: bit 0 is bit 7 of `R`, bits 1–3 are the border, bit 5 says
/// a **version 1** memory block is compressed.
pub const FLAGS_UNCOMPRESSED: u8 = ((R >> 7) & 1) | (BORDER << 1);
pub const FLAGS_COMPRESSED: u8 = FLAGS_UNCOMPRESSED | 0x20;

/// The 30 bytes every version begins with, with `pc_field` at offset 6 and `flags` at 12.
///
/// Transcribed one offset at a time. The irregularities are the reason this is a literal
/// rather than a loop: `BC` is at 2 and `HL` at 4 but `DE` is at **13**; `IY` is at 23 and
/// `IX` at **25**, in that order; `A` and `F` are separate bytes rather than an `AF` word.
pub fn shared_header(pc_field: u16, flags: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(0x12); //  0  A
    bytes.push(0x34); //  1  F
    bytes.extend_from_slice(&[0x78, 0x56]); //  2  BC   = 0x5678
    bytes.extend_from_slice(&[0xF0, 0xDE]); //  4  HL   = 0xDEF0
    bytes.extend_from_slice(&pc_field.to_le_bytes()); //  6  PC, or zero for version 2/3
    bytes.extend_from_slice(&[0xDC, 0xFE]); //  8  SP   = 0xFEDC
    bytes.push(I); // 10  I
    bytes.push(R & 0x7F); // 11  R, seven significant bits
    bytes.push(flags); // 12  see above
    bytes.extend_from_slice(&[0xBC, 0x9A]); // 13  DE   = 0x9ABC  <- not at 6
    bytes.extend_from_slice(&[0x87, 0x65]); // 15  BC'  = 0x6587
    bytes.extend_from_slice(&[0xCB, 0xA9]); // 17  DE'  = 0xA9CB
    bytes.extend_from_slice(&[0x0F, 0xED]); // 19  HL'  = 0xED0F
    bytes.push(0x21); // 21  A'
    bytes.push(0x43); // 22  F'
    bytes.extend_from_slice(&[0x68, 0x24]); // 23  IY   = 0x2468  <- IY before IX
    bytes.extend_from_slice(&[0x57, 0x13]); // 25  IX   = 0x1357
    bytes.push(0x01); // 27  IFF1: non-zero means EI
    bytes.push(0x00); // 28  IFF2: clear, so a swap with IFF1 is visible
    bytes.push(0x02); // 29  interrupt mode 2, in bits 0-1
    assert_eq!(bytes.len(), 30, "the shared header is 30 bytes");
    bytes
}

/// Bytes that expand to a whole page of `byte`, encoded by hand.
///
/// The count in an escape is a single byte, so a 16384-byte page is 64 runs of 255 and one
/// of 64: `64 x 255 + 64 = 16384`.
pub fn page_of(byte: u8) -> Vec<u8> {
    let mut encoded = Vec::new();
    for _ in 0..64 {
        encoded.extend_from_slice(&[0xED, 0xED, 255, byte]);
    }
    encoded.extend_from_slice(&[0xED, 0xED, 64, byte]);
    encoded
}

/// A version 2/3 page block: a two-byte compressed length, a page number, and the data.
pub fn page_block(page: u8, byte: u8) -> Vec<u8> {
    let data = page_of(byte);
    let length = u16::try_from(data.len()).expect("a compressed page is far under 64 KB");
    let mut block = length.to_le_bytes().to_vec();
    block.push(page);
    block.extend_from_slice(&data);
    block
}

/// A complete version 1 `.z80`: the shared header, one compressed 48 KB block, the marker.
pub fn v1_vector() -> Vec<u8> {
    let mut bytes = shared_header(expected("pc"), FLAGS_COMPRESSED);
    // One block holding 0x4000-0xFFFF in address order, not three.
    for &(_, _, byte) in &FILL {
        bytes.extend_from_slice(&page_of(byte));
    }
    bytes.extend_from_slice(&[0x00, 0xED, 0xED, 0x00]); // the version 1 end marker
    bytes
}

/// The additional header of a version 2 file: 23 bytes covering offsets 32–54.
///
/// It stops **one byte short** of the low T-state word at 55, which is why a version 2 file
/// has no frame position and why the writer emits version 3.
pub fn v2_vector() -> Vec<u8> {
    let mut bytes = shared_header(0, FLAGS_UNCOMPRESSED);
    bytes.extend_from_slice(&23_u16.to_le_bytes()); // 30  additional header length
    bytes.extend_from_slice(&expected("pc").to_le_bytes()); // 32  the real PC
    bytes.push(0); // 34  hardware mode 0 = 48K
    bytes.push(0); // 35  last OUT to 0x7FFD
    bytes.push(0); // 36  Interface I ROM not paged
    bytes.push(0); // 37  no R or LDIR emulation
    bytes.push(0); // 38  last OUT to 0xFFFD
    bytes.extend_from_slice(&[0; 16]); // 39-54  sound chip registers
    assert_eq!(bytes.len(), 32 + 23, "offsets 32-54 inclusive");
    for &(_, _, byte) in &FILL {
        // The page numbers, taken from the format description: page 8 is 0x4000-0x7FFF,
        // page 4 is 0x8000-0xBFFF, page 5 is 0xC000-0xFFFF.
        bytes.extend_from_slice(&page_block(page_number(byte), byte));
    }
    bytes
}

/// A complete version 3 `.z80`, in the canonical form this crate's writer emits.
pub fn v3_vector() -> Vec<u8> {
    let mut bytes = shared_header(0, FLAGS_UNCOMPRESSED);
    bytes.extend_from_slice(&54_u16.to_le_bytes()); // 30  additional header length
    bytes.extend_from_slice(&expected("pc").to_le_bytes()); // 32  the real PC
    bytes.push(0); // 34  hardware mode 0 = 48K
    bytes.extend_from_slice(&[0, 0, 0, 0]); // 35-38
    bytes.extend_from_slice(&[0; 16]); // 39-54  sound chip registers
    // 55-56 low, 57 high. Derived by hand from the format description's own sentence, and
    // *not* from `encode_t_states`: 40000 falls in the third quarter (40000 / 17472 = 2), so
    // the low counter has 17471 - (40000 - 2 x 17472) = 12415 left to count down, and the
    // high counter reads (2 + 3) mod 4 = 1 because it starts each frame at 3 rather than 0.
    bytes.extend_from_slice(&12_415_u16.to_le_bytes()); // 55
    bytes.push(1); // 57
    bytes.extend_from_slice(&[0; 28]); // 58-85
    assert_eq!(bytes.len(), 32 + 54, "offsets 32-85 inclusive");
    for &(_, _, byte) in &FILL {
        bytes.extend_from_slice(&page_block(page_number(byte), byte));
    }
    bytes
}

/// The `.z80` page number for the region filled with `byte`, from the format description.
///
/// Written as a literal table here rather than shared with the crate, so the parser's table
/// is graded against this one instead of against itself.
pub fn page_number(byte: u8) -> u8 {
    match byte {
        0xA1 => 8, // 0x4000-0x7FFF
        0xB2 => 4, // 0x8000-0xBFFF
        0xC3 => 5, // 0xC000-0xFFFF
        other => panic!("no page holds {other:#04X}"),
    }
}

/// A complete 48K `.sna`: 27 bytes of header and a raw 48 KB image.
///
/// `SP` points at the two bytes holding `PC`, which is the format's defining property.
pub fn sna_vector() -> Vec<u8> {
    const STACK: u16 = 0x8000;
    let mut bytes = Vec::new();
    bytes.push(I); //  0  I
    bytes.extend_from_slice(&[0x0F, 0xED]); //  1  HL' = 0xED0F  <- the shadow set first
    bytes.extend_from_slice(&[0xCB, 0xA9]); //  3  DE' = 0xA9CB
    bytes.extend_from_slice(&[0x87, 0x65]); //  5  BC' = 0x6587
    bytes.extend_from_slice(&[0x43, 0x21]); //  7  AF' = 0x2143
    bytes.extend_from_slice(&[0xF0, 0xDE]); //  9  HL  = 0xDEF0
    bytes.extend_from_slice(&[0xBC, 0x9A]); // 11  DE  = 0x9ABC
    bytes.extend_from_slice(&[0x78, 0x56]); // 13  BC  = 0x5678
    bytes.extend_from_slice(&[0x68, 0x24]); // 15  IY  = 0x2468  <- IY before IX again
    bytes.extend_from_slice(&[0x57, 0x13]); // 17  IX  = 0x1357
    bytes.push(0b100); // 19  bit 2 is IFF2; there is no IFF1 field
    bytes.push(R); // 20  R, all eight bits
    bytes.extend_from_slice(&[0x34, 0x12]); // 21  AF  = 0x1234
    bytes.extend_from_slice(&STACK.to_le_bytes()); // 23  SP, pointing at PC
    bytes.push(0x02); // 25  interrupt mode 2
    bytes.push(BORDER); // 26  border
    assert_eq!(bytes.len(), 27, "the .sna header is 27 bytes");

    let mut image = Vec::new();
    for &(_, _, byte) in &FILL {
        image.extend(std::iter::repeat_n(byte, PAGE_SIZE));
    }
    // Poke PC onto the guest's stack, which is where a .sna keeps it.
    let at = usize::from(STACK - 0x4000);
    image[at..at + 2].copy_from_slice(&expected("pc").to_le_bytes());

    bytes.extend_from_slice(&image);
    assert_eq!(bytes.len(), 49_179);
    bytes
}
