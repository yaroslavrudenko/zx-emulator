//! The address space: four 16 KB slots, each showing a ROM page or a RAM bank.
//!
//! This is `ARCHITECTURE.md` Decision 5 built as described, at M5 rather than at M7: the
//! 64 KB the Z80 sees is never a flat array, it is a **slot map** consulted on every
//! access. A 48K is then the configuration whose map never changes, and the 128's paging
//! port becomes a writer for `slots` rather than a rewrite of everything attached to a
//! flat array.
//!
//! # Contention is a property of the bank, not of the address
//!
//! On a 48K the two are indistinguishable — `0x4000–0x7FFF` is contended, and that range
//! is where the one contended bank happens to live. On a 128 they come apart: banks 1, 3,
//! 5 and 7 are contended in *whichever* slot they are paged into. [`Memory::is_contended`]
//! therefore asks the slot map, not the address, from the start. Getting this wrong is not
//! a bug that shows up as a crash; it shows up as a demo tearing three years later.
//!
//! # The bank index is provably in range
//!
//! `MACHINE.md` Decision 3 requires it and prices it: an unproven index measured **6.6 %**
//! at M1, and it is free to avoid. [`BankIndex`] and [`RomIndex`] mask on construction
//! *and* at the point of use, so every `[]` in this module indexes a fixed-size array with
//! a value the compiler can see is in range. The masks are only correct because the counts
//! are powers of two, which is asserted at compile time below rather than assumed.

use std::fmt;

/// Bytes in one memory page — the granularity slots, banks and ROMs are all expressed in.
pub const PAGE_SIZE: usize = 0x4000;

/// RAM banks a Spectrum 128 has. A 48K reaches three of them: 5, 2 and 0.
pub const BANK_COUNT: usize = 8;

/// ROM pages a Spectrum 128 has: the 128 editor and 48 BASIC.
///
/// A 48K loads its single ROM into page 0 and can never select page 1, because it has no
/// paging port to select it with. The page is sized in anyway so that M7 adds a writer for
/// `0x7FFD` and nothing else here moves.
pub const ROM_COUNT: usize = 2;

/// 16 KB slots the 64 KB address space is divided into.
pub const SLOT_COUNT: usize = 4;

// The masks below are only equivalent to a bounds check because these are powers of two.
// Asserted rather than assumed: a future `ROM_COUNT = 3` would silently start aliasing.
const _: () = assert!(BANK_COUNT.is_power_of_two());
const _: () = assert!(ROM_COUNT.is_power_of_two());
const _: () = assert!(SLOT_COUNT.is_power_of_two());
const _: () = assert!(PAGE_SIZE * SLOT_COUNT == 0x1_0000);

/// Bits of an address that select the slot.
const SLOT_SHIFT: u32 = 14;

/// Which RAM bank a slot shows.
///
/// A newtype rather than a `u8` so the value cannot arrive from arithmetic unmasked. The
/// mask is applied both on construction and again where the value indexes an array,
/// because it is the one at the point of use that the compiler turns into an elided
/// bounds check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankIndex(u8);

impl BankIndex {
    const MASK: u8 = (BANK_COUNT - 1) as u8;

    /// The bank `bank` selects, wrapping into range.
    ///
    /// Wrapping rather than rejecting: on a 128, bits 0–2 of port `0x7FFD` *are* the bank
    /// number, so every three-bit value is a legal selection and there is nothing to
    /// reject.
    #[must_use]
    pub const fn new(bank: u8) -> Self {
        Self(bank & Self::MASK)
    }

    /// The bank number, 0–7.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0 & Self::MASK
    }

    /// The bank number as an array index the compiler can prove is in range.
    const fn index(self) -> usize {
        (self.0 & Self::MASK) as usize
    }
}

/// Which ROM page a slot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RomIndex(u8);

impl RomIndex {
    const MASK: u8 = (ROM_COUNT - 1) as u8;

    /// The ROM page `rom` selects, wrapping into range.
    #[must_use]
    pub const fn new(rom: u8) -> Self {
        Self(rom & Self::MASK)
    }

    /// The ROM page number.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0 & Self::MASK
    }

    /// The page number as an array index the compiler can prove is in range.
    const fn index(self) -> usize {
        (self.0 & Self::MASK) as usize
    }
}

/// What one 16 KB slot of the address space currently shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// A ROM page. Writes to it are discarded, as they are on the hardware.
    Rom(RomIndex),
    /// A RAM bank.
    Bank(BankIndex),
}

/// The slot map of a 48K: ROM, then banks 5, 2 and 0.
///
/// The bank numbers are the ones a 128 would use for the same three 16 KB regions, which
/// is what makes a 48K a configuration of the larger machine rather than a different
/// design. Bank 5 holds the screen.
pub const SPECTRUM_48K_SLOTS: [Slot; SLOT_COUNT] = [
    Slot::Rom(RomIndex::new(0)),
    Slot::Bank(BankIndex::new(5)),
    Slot::Bank(BankIndex::new(2)),
    Slot::Bank(BankIndex::new(0)),
];

/// Which banks the ULA contends on a 48K.
///
/// Only bank 5, the one the screen lives in and the only one this machine reaches through
/// a contended slot. A 128 sets banks 1, 3, 5 and 7; marking those here would be a claim
/// about hardware a 48K does not have.
const SPECTRUM_48K_CONTENDED_BANKS: [bool; BANK_COUNT] =
    [false, false, false, false, false, true, false, false];

/// A ROM image that is not one page long.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a Spectrum ROM image is {expected} bytes; this one is {actual}")]
pub struct RomSizeError {
    /// Bytes a ROM page holds.
    pub expected: usize,
    /// Bytes the supplied image held.
    pub actual: usize,
}

/// The Z80's 64 KB address space, as slots onto ROM pages and RAM banks.
pub struct Memory {
    ram: Box<[[u8; PAGE_SIZE]; BANK_COUNT]>,
    rom: Box<[[u8; PAGE_SIZE]; ROM_COUNT]>,
    slots: [Slot; SLOT_COUNT],
    contended: [bool; BANK_COUNT],
}

impl Memory {
    /// A 48K's memory: one ROM page and the fixed slot map, with RAM cleared.
    ///
    /// Real RAM powers up holding garbage rather than zeros, and the ROM's own start-up
    /// clears what it relies on. Zeroing is chosen for determinism: a frame hash means
    /// nothing if the machine starts from a different state each run.
    ///
    /// # Errors
    ///
    /// [`RomSizeError`] if `rom` is not exactly [`PAGE_SIZE`] bytes.
    pub fn spectrum_48k(rom: &[u8]) -> Result<Self, RomSizeError> {
        let page: &[u8; PAGE_SIZE] = rom.try_into().map_err(|_| RomSizeError {
            expected: PAGE_SIZE,
            actual: rom.len(),
        })?;

        let mut memory = Self {
            ram: Box::new([[0; PAGE_SIZE]; BANK_COUNT]),
            rom: Box::new([[0; PAGE_SIZE]; ROM_COUNT]),
            slots: SPECTRUM_48K_SLOTS,
            contended: SPECTRUM_48K_CONTENDED_BANKS,
        };
        memory.rom[RomIndex::new(0).index()] = *page;
        Ok(memory)
    }

    /// The byte at `address`, through whatever the slot map currently shows there.
    #[inline]
    #[must_use]
    pub fn read(&self, address: u16) -> u8 {
        let (slot, offset) = split(address);
        match self.slots[slot] {
            Slot::Rom(rom) => self.rom[rom.index()][offset],
            Slot::Bank(bank) => self.ram[bank.index()][offset],
        }
    }

    /// Store `value` at `address`, discarding writes that land in a ROM slot.
    ///
    /// Discarding rather than rejecting is the hardware's behaviour, and software relies on
    /// it: the ROM's own routines write through pointers that can legally address ROM.
    #[inline]
    pub fn write(&mut self, address: u16, value: u8) {
        let (slot, offset) = split(address);
        if let Slot::Bank(bank) = self.slots[slot] {
            self.ram[bank.index()][offset] = value;
        }
    }

    /// Whether the ULA contends accesses to `address` at this moment's slot map.
    #[inline]
    #[must_use]
    pub fn is_contended(&self, address: u16) -> bool {
        let (slot, _) = split(address);
        match self.slots[slot] {
            Slot::Rom(_) => false,
            Slot::Bank(bank) => self.contended[bank.index()],
        }
    }

    /// What the slot covering `address` currently shows.
    #[must_use]
    pub fn slot_at(&self, address: u16) -> Slot {
        let (slot, _) = split(address);
        self.slots[slot]
    }

    /// The whole slot map, in address order.
    #[must_use]
    pub fn slots(&self) -> [Slot; SLOT_COUNT] {
        self.slots
    }
}

impl fmt::Debug for Memory {
    /// Deliberately not derived: a derived `Debug` prints 160 KB of page contents, which
    /// makes every failing assertion involving a machine unreadable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memory")
            .field("slots", &self.slots)
            .field("contended", &self.contended)
            .finish_non_exhaustive()
    }
}

/// Split an address into the slot it selects and the offset within that slot's page.
///
/// Both halves are masked so the compiler can see they index their arrays in range; the
/// shift already guarantees the slot for a `u16`, but the mask survives a change of
/// [`SLOT_SHIFT`] and costs nothing.
#[inline]
const fn split(address: u16) -> (usize, usize) {
    let slot = ((address >> SLOT_SHIFT) as usize) & (SLOT_COUNT - 1);
    let offset = (address as usize) & (PAGE_SIZE - 1);
    (slot, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Memory {
        let rom: Vec<u8> = (0..PAGE_SIZE).map(|i| (i & 0xFF) as u8).collect();
        Memory::spectrum_48k(&rom).expect("a page-sized ROM")
    }

    #[test]
    fn a_rom_image_of_the_wrong_length_is_rejected() {
        let err = Memory::spectrum_48k(&[0; 16]).expect_err("16 bytes is not a ROM");
        assert_eq!(
            err,
            RomSizeError {
                expected: PAGE_SIZE,
                actual: 16
            }
        );
    }

    #[test]
    fn the_rom_is_visible_in_the_bottom_slot() {
        let memory = memory();
        assert_eq!(memory.read(0x0000), 0x00);
        assert_eq!(memory.read(0x1234), 0x34);
        assert_eq!(memory.read(0x3FFF), 0xFF);
    }

    #[test]
    fn writes_to_the_rom_slot_are_discarded_rather_than_rejected() {
        let mut memory = memory();
        memory.write(0x1234, 0xA5);
        assert_eq!(memory.read(0x1234), 0x34);
    }

    #[test]
    fn each_ram_slot_is_a_distinct_bank() {
        let mut memory = memory();
        memory.write(0x4000, 1);
        memory.write(0x8000, 2);
        memory.write(0xC000, 3);
        assert_eq!(
            (
                memory.read(0x4000),
                memory.read(0x8000),
                memory.read(0xC000)
            ),
            (1, 2, 3),
            "the three RAM slots must not alias each other"
        );
    }

    #[test]
    fn only_the_screen_bank_is_contended_on_a_48k() {
        let memory = memory();
        assert!(!memory.is_contended(0x0000), "ROM is never contended");
        assert!(!memory.is_contended(0x3FFF));
        assert!(memory.is_contended(0x4000));
        assert!(memory.is_contended(0x7FFF));
        assert!(!memory.is_contended(0x8000));
        assert!(!memory.is_contended(0xFFFF));
    }

    #[test]
    fn contention_follows_the_bank_and_not_the_address_range() {
        // The property M7 depends on, asserted at M5 while it is still cheap to fix: move
        // the contended bank into an uncontended slot and contention must move with it.
        let mut memory = memory();
        memory.slots = [
            Slot::Rom(RomIndex::new(0)),
            Slot::Bank(BankIndex::new(2)),
            Slot::Bank(BankIndex::new(5)),
            Slot::Bank(BankIndex::new(0)),
        ];
        assert!(!memory.is_contended(0x4000), "bank 2 is not contended");
        assert!(memory.is_contended(0x8000), "bank 5 still is");
    }

    #[test]
    fn a_bank_index_wraps_into_range_rather_than_panicking() {
        assert_eq!(BankIndex::new(9).get(), 1);
        assert_eq!(BankIndex::new(0xFF).get(), 7);
        assert_eq!(RomIndex::new(0xFF).get(), 1);
    }

    #[test]
    fn the_slot_map_covers_the_whole_address_space_without_overlap() {
        assert_eq!(split(0x0000), (0, 0x0000));
        assert_eq!(split(0x3FFF), (0, 0x3FFF));
        assert_eq!(split(0x4000), (1, 0x0000));
        assert_eq!(split(0xFFFF), (3, 0x3FFF));
    }

    #[test]
    fn debug_does_not_print_the_page_contents() {
        let rendered = format!("{:?}", memory());
        assert!(
            rendered.len() < 400,
            "Debug printed {} bytes",
            rendered.len()
        );
        assert!(rendered.contains("slots"));
    }
}
