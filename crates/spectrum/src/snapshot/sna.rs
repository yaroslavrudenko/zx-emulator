//! The `.sna` format: read only.
//!
//! # Why there is no writer
//!
//! **A `.sna` has no `PC` field.** The program counter lives on the *guest's stack*, at the
//! address `SP` points to, so a writer must push it — destroying two bytes of the machine's
//! RAM in order to record the machine's state. A save that modifies what it is saving is not
//! a save, and nothing in M6 needs one. Reading stays, because the milestone names the
//! format and because plenty of software is only distributed this way.
//!
//! # Three consequences of that design, recorded so they are not rediscovered as bugs
//!
//! | Property | Consequence |
//! |---|---|
//! | `PC` is popped from the RAM image at `SP`, then `SP` advances by two | the parser reads it out of its **own** image, so it needs no machine |
//! | `SP` below `0x4000`, or `0xFFFF` where the second byte wraps into ROM | those bytes are not in the file at all — [`Error::StackPointerOutsideRam`], not a guess |
//! | only `IFF2` is stored | `iff1 = iff2` is a **convention**, wrong only for a snapshot taken inside an NMI handler, which no `.sna` writer produces |
//! | no T-state counter | `frame_t_state = 0`, the top of the frame — which is inside the interrupt window, so a `.sna` takes an interrupt almost immediately. That is what other implementations do; it is a convention, not a measurement |
//!
//! The two stale bytes are left in place after `PC` is popped, which is what every other
//! implementation does: the snapshot's RAM genuinely contained them.

use ::z80::{CpuState, InterruptMode};

use super::reader::Reader;
use super::{Error, IMAGE_LEN_48K, RAM_BASE, Snapshot, store_image};
use crate::screen::Colour;

/// Bytes of header before the RAM image.
const HEADER_LEN: usize = 27;

/// The whole file: 27 bytes of header and a 48 KB image.
///
/// Named because it is also the check that refuses a **128K** `.sna`, which is longer. That
/// rejection is deliberate at M6 — a 128 snapshot carries banks this machine has no slot for
/// — and it arrives as [`Error::TrailingBytes`] naming offset 49179.
const FILE_LEN: usize = HEADER_LEN + IMAGE_LEN_48K;

const _: () = assert!(FILE_LEN == 49179);

/// Bit 2 of byte 19 holds `IFF2`. The rest of the byte has no defined meaning.
const IFF2_BIT: u8 = 0b100;

/// Bits of byte 26 that are a border colour; the format gives it the range 0–7.
const BORDER_MASK: u8 = 0b111;

/// Read a 48K `.sna` snapshot.
///
/// # Errors
///
/// [`Error`], naming the offset or the byte that failed. Every failure is a returned value:
/// this function does not panic on any input.
pub fn parse(bytes: &[u8]) -> Result<Snapshot, Error> {
    let mut reader = Reader::new(bytes);

    let i = reader.u8()?; // 0
    let hl_shadow = reader.u16_le()?; // 1 — the shadow set comes first, HL' DE' BC' AF'
    let de_shadow = reader.u16_le()?; // 3
    let bc_shadow = reader.u16_le()?; // 5
    let af_shadow = reader.u16_le()?; // 7
    let hl = reader.u16_le()?; // 9
    let de = reader.u16_le()?; // 11
    let bc = reader.u16_le()?; // 13
    let iy = reader.u16_le()?; // 15 — IY before IX, as in the .z80
    let ix = reader.u16_le()?; // 17
    let interrupt = reader.u8()?; // 19 — bit 2 is IFF2; there is no IFF1
    let r = reader.u8()?; // 20
    let af = reader.u16_le()?; // 21
    let stack_pointer = reader.u16_le()?; // 23 — points at PC, which is why it moves below
    let mode = reader.u8()?; // 25
    let border = reader.u8()?; // 26
    let image = reader.take(IMAGE_LEN_48K)?; // 27 — 0x4000..=0xFFFF, uncompressed
    reader.finish()?;

    let im =
        InterruptMode::try_from(mode).map_err(|_| Error::InvalidInterruptMode { value: mode })?;
    let pc = pop_program_counter(image, stack_pointer)?;
    let iff2 = interrupt & IFF2_BIT != 0;

    let mut snapshot = Snapshot::new(
        CpuState {
            af,
            bc,
            de,
            hl,
            af_shadow,
            bc_shadow,
            de_shadow,
            hl_shadow,
            ix,
            iy,
            // The pop happened, so the stack is two bytes shallower. `wrapping_add` because
            // an `SP` of 0xFFFE legitimately wraps to zero, exactly as it does on hardware.
            sp: stack_pointer.wrapping_add(2),
            pc,
            i,
            r,
            // Only IFF2 is stored. See the module documentation: this is a convention.
            iff1: iff2,
            iff2,
            im,
            // Neither format carries a halt flag; see `snapshot::UNPRESERVED`.
            halted: false,
            wz: 0,
            // Loading a state is a `POP AF`, so the latch must equal `F`.
            q: (af & 0xFF) as u8,
        },
        Colour::new(border & BORDER_MASK),
        // The format carries no frame position, so the machine restores at the top of the
        // frame. See the module documentation for what that costs.
        0,
    );
    store_image(&mut snapshot, image);
    Ok(snapshot)
}

/// Read the word `sp` points at out of the snapshot's own RAM image.
///
/// # Errors
///
/// [`Error::StackPointerOutsideRam`] when either byte lies outside `0x4000`–`0xFFFF`: below
/// the image, where the address is ROM that a `.sna` does not carry, or at `SP = 0xFFFF`
/// where the second byte wraps past the top into ROM. Those bytes are not in the file at
/// all, so there is nothing to guess and guessing zero would be a machine that jumps
/// somewhere the original never did.
fn pop_program_counter(image: &[u8], sp: u16) -> Result<u16, Error> {
    let outside = Error::StackPointerOutsideRam { sp };
    let offset = usize::from(sp.checked_sub(RAM_BASE).ok_or(outside)?);
    // Through a `Reader` rather than a slice index, for the reason `snapshot::reader` gives:
    // this offset comes from the file.
    let mut stack = Reader::new(image);
    stack.skip(offset).map_err(|_| outside)?;
    stack.u16_le().map_err(|_| outside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{BankIndex, PAGE_SIZE};

    /// A well-formed `.sna` whose every header field differs from every other.
    ///
    /// Built here rather than transcribed, because the transcribed vector lives in
    /// `crates/spectrum/tests/snapshot_vectors.rs` where its expectation can be written
    /// independently of this file. These tests are about the *rules* — the stack pop, the
    /// length check — and use a fixture that is convenient rather than probative.
    fn well_formed(sp: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; FILE_LEN];
        if let Some(header) = bytes.get_mut(..HEADER_LEN) {
            header.copy_from_slice(&[
                0x3F, // 0: I
                0x11, 0x22, // 1: HL'
                0x33, 0x44, // 3: DE'
                0x55, 0x66, // 5: BC'
                0x77, 0x88, // 7: AF'
                0x99, 0xAA, // 9: HL
                0xBB, 0xCC, // 11: DE
                0xDD, 0xEE, // 13: BC
                0x3A, 0x5C, // 15: IY
                0x12, 0x34,     // 17: IX
                IFF2_BIT, // 19: interrupts enabled
                0x7E,     // 20: R
                0x56, 0x78, // 21: AF
                0x00, 0x00, // 23: SP, overwritten below
                0x01, // 25: IM 1
                0x05, // 26: border
            ]);
        }
        if let Some(field) = bytes.get_mut(23..25) {
            field.copy_from_slice(&sp.to_le_bytes());
        }
        bytes
    }

    /// Write `value` at guest address `address` in a `.sna`'s RAM image.
    fn poke(bytes: &mut [u8], address: u16, value: u16) {
        let at = HEADER_LEN + usize::from(address - RAM_BASE);
        if let Some(field) = bytes.get_mut(at..at + 2) {
            field.copy_from_slice(&value.to_le_bytes());
        }
    }

    #[test]
    fn the_program_counter_comes_off_the_guests_own_stack() {
        let mut bytes = well_formed(0x8000);
        poke(&mut bytes, 0x8000, 0x1234);
        let snapshot = parse(&bytes).expect("a well-formed .sna");
        assert_eq!(snapshot.cpu.pc, 0x1234, "PC is popped, never a field");
        assert_eq!(snapshot.cpu.sp, 0x8002, "and the stack is two shallower");
    }

    #[test]
    fn the_two_stale_stack_bytes_stay_in_ram() {
        // What every other implementation does, and it is right: the snapshot's RAM
        // genuinely contained them. Zeroing them would be inventing a machine.
        let mut bytes = well_formed(0x8000);
        poke(&mut bytes, 0x8000, 0x1234);
        let snapshot = parse(&bytes).expect("a well-formed .sna");
        let bank = snapshot.bank(BankIndex::new(2)).expect("0x8000 is bank 2");
        assert_eq!((bank[0], bank[1]), (0x34, 0x12));
    }

    #[test]
    fn a_stack_pointer_outside_the_ram_image_is_refused_rather_than_guessed() {
        for sp in [0x0000, 0x0001, 0x3FFE, 0x3FFF, 0xFFFF] {
            assert_eq!(
                parse(&well_formed(sp)),
                Err(Error::StackPointerOutsideRam { sp }),
                "SP {sp:#06X}"
            );
        }
    }

    #[test]
    fn the_last_two_bytes_of_ram_are_a_legal_stack() {
        // The boundary on the other side: 0xFFFE is the highest SP whose two bytes are both
        // in the image, and 0xFFFF is the first that is not. Off by one here would either
        // refuse a legal file or read a byte the file does not contain.
        let mut bytes = well_formed(0xFFFE);
        poke(&mut bytes, 0xFFFE, 0xBEEF);
        let snapshot = parse(&bytes).expect("SP = 0xFFFE is legal");
        assert_eq!(snapshot.cpu.pc, 0xBEEF);
        assert_eq!(snapshot.cpu.sp, 0x0000, "the stack wraps, as on hardware");
    }

    #[test]
    fn only_iff2_is_stored_and_iff1_follows_it() {
        for (byte, expected) in [(0x00, false), (IFF2_BIT, true), (0xFF, true), (0xFB, false)] {
            let mut bytes = well_formed(0x8000);
            if let Some(field) = bytes.get_mut(19) {
                *field = byte;
            }
            let snapshot = parse(&bytes).expect("a well-formed .sna");
            assert_eq!(snapshot.cpu.iff2, expected, "byte 19 = {byte:#04X}");
            assert_eq!(
                snapshot.cpu.iff1, expected,
                "iff1 = iff2 is the convention this format forces"
            );
        }
    }

    #[test]
    fn a_file_of_the_wrong_length_is_refused_at_both_ends() {
        let bytes = well_formed(0x8000);
        assert!(matches!(
            parse(&bytes[..FILE_LEN - 1]),
            Err(Error::Truncated { .. })
        ));

        // A 128K .sna is a 48K one with more after it. Rejected at M6, and the error names
        // the offset so the reason is findable.
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(
            parse(&longer),
            Err(Error::TrailingBytes {
                offset: FILE_LEN,
                extra: 1
            })
        );
    }

    #[test]
    fn an_interrupt_mode_the_z80_does_not_have_is_refused() {
        for mode in [3_u8, 4, 255] {
            let mut bytes = well_formed(0x8000);
            if let Some(field) = bytes.get_mut(25) {
                *field = mode;
            }
            assert_eq!(
                parse(&bytes),
                Err(Error::InvalidInterruptMode { value: mode }),
                "mode {mode}"
            );
        }
    }

    #[test]
    fn the_image_lands_in_the_three_banks_in_address_order() {
        let mut bytes = well_formed(0x8000);
        poke(&mut bytes, 0x8000, 0x0000);
        // One distinguishing byte at the base of each 16 KB region.
        for (address, value) in [(0x4000_u16, 0xA1_u8), (0x8000, 0xB2), (0xC000, 0xC3)] {
            if let Some(field) = bytes.get_mut(HEADER_LEN + usize::from(address - RAM_BASE)) {
                *field = value;
            }
        }
        let snapshot = parse(&bytes).expect("a well-formed .sna");
        assert_eq!(snapshot.bank(BankIndex::new(5)).map(|p| p[0]), Some(0xA1));
        assert_eq!(snapshot.bank(BankIndex::new(2)).map(|p| p[0]), Some(0xB2));
        assert_eq!(snapshot.bank(BankIndex::new(0)).map(|p| p[0]), Some(0xC3));
        assert_eq!(snapshot.banks().count(), 3);
        assert_eq!(
            snapshot.bank(BankIndex::new(5)).map(|p| p.len()),
            Some(PAGE_SIZE)
        );
    }

    #[test]
    fn the_border_is_masked_to_the_range_the_format_gives_it() {
        for (byte, expected) in [(0_u8, 0_u8), (7, 7), (0xFF, 7), (0x0A, 2)] {
            let mut bytes = well_formed(0x8000);
            if let Some(field) = bytes.get_mut(26) {
                *field = byte;
            }
            let snapshot = parse(&bytes).expect("a well-formed .sna");
            assert_eq!(snapshot.border.index(), expected, "byte 26 = {byte:#04X}");
        }
    }

    #[test]
    fn a_sna_restores_at_the_top_of_the_frame() {
        // A convention rather than a measurement, and the one place it is asserted so that
        // changing it is a deliberate act. The top of the frame is inside the interrupt
        // window, so a `.sna` takes an interrupt almost immediately.
        let snapshot = parse(&well_formed(0x8000)).expect("a well-formed .sna");
        assert_eq!(snapshot.frame_t_state, 0);
    }
}
