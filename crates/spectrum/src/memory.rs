//! The address space: four 16 KB slots, each showing a ROM page or a RAM bank.
//!
//! This is `ARCHITECTURE.md` Decision 5 built as described, at M5 rather than at M7: the
//! 64 KB the Z80 sees is never a flat array, it is a **slot map** consulted on every
//! access. A 48K is then the configuration whose map never changes, and the 128's paging
//! port becomes a writer for `slots` rather than a rewrite of everything attached to a
//! flat array.
//!
//! That prediction held, and M7 can say by how much. The whole memory half of the 128 is
//! **two fields and a handful of methods**: the [`Slot`] indirection, the per-bank contention
//! array and the second ROM page were all already here and all already had tests.
//!
//! # A 48K is the machine that powered on with the lock already set
//!
//! `M7.md` Decision 1's finding, and it is the reason nothing in this module asks which
//! machine it is in order to decide whether a write to `0x7FFD` may page. Derive a slot map
//! from a port value the way a 128 does and ask which value produces a 48K's map:
//!
//! ```text
//!   0x20 = 0b0010_0000
//!          bits 0-2 = 000  ->  bank 0 at 0xC000     matches
//!          bit  3   = 0    ->  screen is bank 5     matches
//!          bit  4   = 0    ->  ROM page 0           matches
//!          bit  5   = 1    ->  paging locked        matches: a 48K cannot page
//! ```
//!
//! **A 48K's memory map is exactly what port value `0x20` derives, and its inability to page
//! is exactly the lock bit already being set.** `Memory::write_paging_port` returns early on
//! the lock and asks nothing else, so a 48K absorbs a `0x7FFD` write with no model check
//! anywhere. That equation is asserted at compile time below rather than left as prose:
//! `slots_for(0x20)` is checked against [`SPECTRUM_48K_SLOTS`], which is transcribed
//! independently from the published 48K map.
//!
//! What the port byte cannot derive is the rest of the machine — which banks are contended,
//! how long a frame is, and which banks exist at all. [`crate::Model`] is where those live.
//!
//! # One representation, derived once
//!
//! [`Memory`] stores the **port byte** and derives `slots`, the screen bank and the lock from
//! it. It never stores both. Keeping a `paging_locked: bool` and a `screen_bank: BankIndex`
//! alongside the byte would be three representations of one datum that can disagree, and a
//! snapshot writer would then have to choose which to serialise. `slots` is a derived cache,
//! rebuilt by the one function that reads the byte, so the bit layout is written down once.
//!
//! # Contention is a property of the bank, not of the address
//!
//! On a 48K the two are indistinguishable — `0x4000–0x7FFF` is contended, and that range
//! is where the one contended bank happens to live. On a 128 they come apart: banks 1, 3,
//! 5 and 7 are contended in *whichever* slot they are paged into. [`Memory::is_contended`]
//! therefore asks the slot map, not the address, from the start. Getting this wrong is not
//! a bug that shows up as a crash; it shows up as a demo tearing three years later.
//!
//! **That is why M7 cost nothing on the hot path.** Through M8, `is_contended` was
//! byte-for-byte the function it was at M5 — the only thing the 128 changed was the contents
//! of the `contended` array it read. *(That sentence was present tense until the audio-glitch
//! pass measured what it was defending: two dependent loads and two branches, on every memory
//! access, for an answer that moves only on a `0x7FFD` write. `is_contended` now reads
//! `contended_slots`, a third derived cache rebuilt in the same breath as `slots`, and the
//! property this section argues survives the fold: the cache is derived from the slot map, so
//! contention still follows the bank into whichever slot pages it.)*
//!
//! # The bank index is provably in range
//!
//! [`BankIndex`] and [`RomIndex`] mask on construction *and* at the point of use, so every
//! `[]` in this module indexes a fixed-size array with a value the compiler can see is in
//! range. The masks are only correct because the counts are powers of two, which is
//! asserted at compile time below rather than assumed.
//!
//! **The reason is clarity, and it is no longer speed.** This paragraph used to price the
//! decision — *"an unproven index measured **6.6 %** at M1"* — and that figure is
//! **falsified**. `benches/step.rs` now measures the two variants against each other
//! instead of quoting a number: one bus, one line of difference, and the masked variant
//! came out *slower* in three runs of four, with the spread inside a single variant larger
//! than the gap between them. The bounds checks the masking removes are real — they are
//! visible in the emitted assembly, present in the unmasked instantiation and absent from
//! the masked one — and on this core they cost nothing measurable. So nobody should read
//! this module and expect the masks to buy time at M7; they buy a `[]` that cannot panic,
//! which is worth having on its own. The measurement is in `docs/ARCHITECTURE.md`.
//!
//! **They now buy something else as well, and it is the reason they matter more at M7 than at
//! M5.** Bits 0–2 of `0x7FFD` *are* the bank number, so every three-bit value a guest can
//! write is a legal selection with nothing to reject — and this crate builds with
//! `panic = "abort"` in release, where a guest-triggered panic kills the process. All 256 port
//! values are legal and none of them can panic, structurally rather than by testing, because
//! the masking makes the illegal value unrepresentable. `all_256_paging_port_values_leave_a_coherent_machine`
//! is the exhaustive check that the rest of the machine agrees.

use std::fmt;

use crate::model::Model;

/// Bytes in one memory page — the granularity slots, banks and ROMs are all expressed in.
pub const PAGE_SIZE: usize = 0x4000;

/// RAM banks a Spectrum 128 has. A 48K reaches three of them: 5, 2 and 0.
pub const BANK_COUNT: usize = 8;

/// ROM pages a Spectrum 128 has: the 128 editor and 48 BASIC.
///
/// A 48K loads its single ROM into page 0 and can never select page 1, because its paging
/// port is locked from power-on and bit 4 is what would select it. The page was sized in at
/// M5 so that M7 would add a writer for `0x7FFD` and nothing else here would move; it did.
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

/// Bits 0–2 of `0x7FFD`: the RAM bank paged into `0xC000`.
const PAGING_BANK: u8 = 0x07;

/// Bit 3 of `0x7FFD`: clear selects bank 5 as the screen, set selects the shadow bank 7.
const PAGING_SCREEN: u8 = 0x08;

/// Bit 4 of `0x7FFD`: clear selects the 128 editor ROM, set selects 48 BASIC.
const PAGING_ROM: u8 = 0x10;

/// Bit 5 of `0x7FFD`: paging disable.
///
/// From the World of Spectrum *128K Technical Information* reference, verbatim: *"If set,
/// memory paging will be disabled and further output to this port will be ignored until the
/// computer is reset."* Absorbing, and the only way out is a hard reset — which is what
/// [`Memory::reset`] is for.
const PAGING_LOCK: u8 = 0x20;

// Bits 6 and 7 are unused on a 128 — the +2A/+3 put a second paging port at `0x1FFD` instead,
// and that machine is out of scope. Nothing masks them off, because nothing reads them: a
// value with them set derives exactly the same map as the value without.
const _: () = assert!(PAGING_BANK | PAGING_SCREEN | PAGING_ROM | PAGING_LOCK == 0x3F);

/// The bank the screen lives in with bit 3 of `0x7FFD` clear — and a 48K's only screen.
const NORMAL_SCREEN_BANK: u8 = 5;

/// The bank the screen lives in with bit 3 set: the 128's shadow screen.
const SHADOW_SCREEN_BANK: u8 = 7;

/// The bank wired to `0x4000` on both machines, whatever the paging port says.
const FIXED_BANK_4000: u8 = 5;

/// The bank wired to `0x8000` on both machines.
const FIXED_BANK_8000: u8 = 2;

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
///
/// **Transcribed from the published 48K map, and never computed.** That independence is the
/// whole value of it now: `slots_for` derives the same map from a port value by a completely
/// different route, and the compile-time assertion below compares the two. A single wrong bit
/// in the derivation is caught by a table nothing derived.
pub const SPECTRUM_48K_SLOTS: [Slot; SLOT_COUNT] = [
    Slot::Rom(RomIndex::new(0)),
    Slot::Bank(BankIndex::new(5)),
    Slot::Bank(BankIndex::new(2)),
    Slot::Bank(BankIndex::new(0)),
];

/// The slot map port value `paging_port` selects.
///
/// The **one** place `0x7FFD`'s bit layout is written down. Slots 1 and 2 are wired to banks 5
/// and 2 on both machines and no port bit reaches them; slot 0 takes its ROM page from bit 4
/// and slot 3 its bank from bits 0–2.
///
/// Total by construction: every three-bit value is a bank and every one-bit value is a ROM
/// page, so there is no value of `paging_port` this cannot answer for.
const fn slots_for(paging_port: u8) -> [Slot; SLOT_COUNT] {
    [
        Slot::Rom(RomIndex::new((paging_port & PAGING_ROM) >> 4)),
        Slot::Bank(BankIndex::new(FIXED_BANK_4000)),
        Slot::Bank(BankIndex::new(FIXED_BANK_8000)),
        Slot::Bank(BankIndex::new(paging_port & PAGING_BANK)),
    ]
}

/// Whether two slots show the same thing.
///
/// Exists so the equation below can be asserted at **compile time**: `Slot`'s derived
/// `PartialEq` is not `const`, and this claim is too load-bearing to leave to a test that
/// could be deleted.
const fn slot_eq(a: Slot, b: Slot) -> bool {
    match (a, b) {
        (Slot::Rom(left), Slot::Rom(right)) => left.get() == right.get(),
        (Slot::Bank(left), Slot::Bank(right)) => left.get() == right.get(),
        _ => false,
    }
}

/// Whether two slot maps agree, slot for slot.
const fn slot_maps_eq(a: [Slot; SLOT_COUNT], b: [Slot; SLOT_COUNT]) -> bool {
    let mut slot = 0;
    while slot < SLOT_COUNT {
        if !slot_eq(a[slot], b[slot]) {
            return false;
        }
        slot += 1;
    }
    true
}

// **`M7.md` Decision 1's central claim, as a build failure rather than a paragraph.** The
// left side derives a 48K's map from a port byte using the 128's rule; the right side is the
// published 48K map, transcribed independently. If they ever disagree, this crate does not
// compile.
const _: () = assert!(slot_maps_eq(
    slots_for(Model::Spectrum48K.paging_port_at_reset()),
    SPECTRUM_48K_SLOTS
));

/// The contention [`Memory::is_contended`] answers for each slot of `slots`.
///
/// The third derived cache, and the reason it exists is where its two callers sit:
/// `is_contended` runs on every memory access — the call site in `crates/spectrum/src/ula.rs`
/// is on what that file calls the hottest line in the emulator — while the inputs, the slot
/// map and the per-bank contention table, move only on a `0x7FFD` write. So the bank lookup
/// and its two branches are paid at the write, once, and the access path reads one `bool`.
fn contended_slots_for(
    slots: [Slot; SLOT_COUNT],
    contended: [bool; BANK_COUNT],
) -> [bool; SLOT_COUNT] {
    slots.map(|slot| match slot {
        Slot::Rom(_) => false,
        Slot::Bank(bank) => contended[bank.index()],
    })
}

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
    /// A derived cache of [`slots_for`]`(paging_port)`, never assigned from anywhere else.
    slots: [Slot; SLOT_COUNT],
    /// A derived cache of [`Model::contended_banks`], fixed for this machine's lifetime.
    contended: [bool; BANK_COUNT],
    /// A derived cache of [`contended_slots_for`]`(slots, contended)`, rebuilt in the same
    /// breath as `slots` and read by [`Memory::is_contended`] on every memory access.
    contended_slots: [bool; SLOT_COUNT],
    model: Model,
    /// The last value written to `0x7FFD` — the SSOT for the map, the screen and the lock.
    paging_port: u8,
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
        let mut memory = Self::blank(Model::Spectrum48K);
        memory.load_rom(RomIndex::new(0), rom)?;
        Ok(memory)
    }

    /// A 128's memory: two ROM pages, eight banks, and paging live.
    ///
    /// `editor` is the 128's own ROM — page 0, the one it resets into — and `basic` is the 48
    /// BASIC ROM the menu's *48 BASIC* entry selects by setting bit 4 of `0x7FFD`. They are
    /// separate arguments rather than one 32 KB image because the two pages are separate files
    /// in every distribution, and splitting a combined image is a step at which the halves can
    /// be swapped with nothing to notice.
    ///
    /// # Errors
    ///
    /// [`RomSizeError`] if either image is not exactly [`PAGE_SIZE`] bytes.
    pub fn spectrum_128(editor: &[u8], basic: &[u8]) -> Result<Self, RomSizeError> {
        let mut memory = Self::blank(Model::Spectrum128);
        memory.load_rom(RomIndex::new(0), editor)?;
        memory.load_rom(RomIndex::new(1), basic)?;
        Ok(memory)
    }

    /// Cleared RAM, empty ROM pages, and `model`'s power-on paging state.
    fn blank(model: Model) -> Self {
        let paging_port = model.paging_port_at_reset();
        let slots = slots_for(paging_port);
        let contended = model.contended_banks();
        Self {
            ram: Box::new([[0; PAGE_SIZE]; BANK_COUNT]),
            rom: Box::new([[0; PAGE_SIZE]; ROM_COUNT]),
            slots,
            contended,
            contended_slots: contended_slots_for(slots, contended),
            model,
            paging_port,
        }
    }

    /// Copy `image` into ROM page `page`.
    fn load_rom(&mut self, page: RomIndex, image: &[u8]) -> Result<(), RomSizeError> {
        let image: &[u8; PAGE_SIZE] = image.try_into().map_err(|_| RomSizeError {
            expected: PAGE_SIZE,
            actual: image.len(),
        })?;
        self.rom[page.index()] = *image;
        Ok(())
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
        self.contended_slots[slot]
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

    /// One RAM bank's contents, whether or not the slot map currently shows it anywhere.
    ///
    /// **The point is the second half of that sentence.** On a 48K every bank has an address
    /// and this is a slower spelling of [`Memory::read`]; on a 128 five of the eight banks
    /// typically have no address at all, and three separate callers need them anyway:
    ///
    /// - the snapshot **writer**, which would otherwise capture three banks of eight;
    /// - the snapshot **applier**, which would otherwise silently drop five — the defect
    ///   `docs/M6.md` refused for duplicate pages;
    /// - [`crate::screen::render`], because the ULA draws from bank 5 or bank 7 **directly**
    ///   rather than through the slot map.
    #[must_use]
    pub fn bank(&self, bank: BankIndex) -> &[u8; PAGE_SIZE] {
        &self.ram[bank.index()]
    }

    /// One RAM bank's contents, mutably.
    ///
    /// See [`Memory::bank`]. This is RAM by construction — a `BankIndex` cannot name a ROM
    /// page — so there is no write protection to bypass and none is bypassed.
    pub fn bank_mut(&mut self, bank: BankIndex) -> &mut [u8; PAGE_SIZE] {
        &mut self.ram[bank.index()]
    }

    /// The bank the **ULA** is drawing the screen from.
    ///
    /// Bank 5 unless bit 3 of `0x7FFD` selects the shadow screen, which only a 128 can do.
    /// This is not the same question as "what is at `0x4000`": the ULA reaches its screen
    /// directly and does not go through the slot map, so a 128 can display bank 7 while bank 7
    /// is paged into `0xC000`, into no slot at all, or into both.
    #[must_use]
    pub fn screen_bank(&self) -> BankIndex {
        BankIndex::new(if self.paging_port & PAGING_SCREEN == 0 {
            NORMAL_SCREEN_BANK
        } else {
            SHADOW_SCREEN_BANK
        })
    }

    /// Which machine this is.
    #[must_use]
    pub(crate) fn model(&self) -> Model {
        self.model
    }

    /// The last value written to `0x7FFD`.
    #[must_use]
    pub(crate) fn paging_port(&self) -> u8 {
        self.paging_port
    }

    /// A guest's write to `0x7FFD`, honouring the lock.
    ///
    /// **A 48K absorbs every one of these and there is no model check**: it powers on with the
    /// lock bit already set, so the early return is the whole of its "cannot page" behaviour.
    /// That is `M7.md` Decision 1's equation doing the work rather than a branch on the model.
    ///
    /// All 256 values are legal. Bits 0–2 *are* the bank number and bit 4 *is* the ROM page,
    /// so there is nothing to reject and no error to return; bits 6 and 7 are unused on a 128
    /// and are simply not read.
    pub(crate) fn write_paging_port(&mut self, value: u8) {
        if self.paging_port & PAGING_LOCK != 0 {
            return;
        }
        self.set_paging_port(value);
    }

    /// Set the paging port **without honouring the lock** — the snapshot applier's route.
    ///
    /// The same shape as [`crate::Ula::set_border`] and for a directly analogous reason: a
    /// restore is not a machine cycle, and routing state-setting through the guest-facing path
    /// gets the guest-facing behaviour. Here that behaviour is specifically wrong, and quietly:
    /// the machine being restored *into* may already be locked from whatever it was running
    /// before, so [`Memory::write_paging_port`] would discard the snapshot's map and leave
    /// every field a round trip compares still matching.
    pub(crate) fn set_paging_port(&mut self, value: u8) {
        self.paging_port = value;
        self.slots = slots_for(value);
        self.contended_slots = contended_slots_for(self.slots, self.contended);
    }

    /// The reset button, as far as memory is concerned: the power-on map, lock cleared.
    ///
    /// **RAM is untouched**, which is the hardware's behaviour and was already this crate's
    /// position — *"a reset button does not clear RAM, does not lift the keys, and does not
    /// rewind a cassette."* What that sentence did not cover, because a 48K has no such state,
    /// is the paging lock: clearing it is the **only** way a locked 128 ever pages again.
    ///
    /// A no-op on a 48K by construction — its reset value is `0x20`, which is already the
    /// value — so `reset_returns_the_clock_to_the_start_without_disturbing_memory` keeps
    /// meaning exactly what it meant.
    pub(crate) fn reset(&mut self) {
        self.set_paging_port(self.model.paging_port_at_reset());
    }
}

impl fmt::Debug for Memory {
    /// Deliberately not derived: a derived `Debug` prints 160 KB of page contents, which
    /// makes every failing assertion involving a machine unreadable.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memory")
            .field("model", &self.model)
            .field("paging_port", &format_args!("{:#04X}", self.paging_port))
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

    fn pattern(seed: u8) -> Vec<u8> {
        (0..PAGE_SIZE).map(|i| (i & 0xFF) as u8 ^ seed).collect()
    }

    fn memory() -> Memory {
        Memory::spectrum_48k(&pattern(0)).expect("a page-sized ROM")
    }

    fn memory_128() -> Memory {
        Memory::spectrum_128(&pattern(0), &pattern(0xFF)).expect("two page-sized ROMs")
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
    fn a_128_rejects_a_wrong_sized_image_in_either_position() {
        // Both arguments, because a constructor that only checked the first would accept a
        // machine whose 48 BASIC ROM was whatever the page happened to hold.
        assert!(Memory::spectrum_128(&[0; 16], &pattern(0)).is_err());
        assert!(Memory::spectrum_128(&pattern(0), &[0; 16]).is_err());
        assert!(Memory::spectrum_128(&pattern(0), &pattern(1)).is_ok());
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
        //
        // At M5 this reached in and assigned `slots` directly, because there was no way for a
        // 48K to page. There is now, so it does it the way the hardware would — and the fact
        // that the two routes agree is itself the M7 claim.
        let mut memory = memory_128();
        assert!(memory.is_contended(0x4000), "bank 5 is contended");
        assert!(!memory.is_contended(0xC000), "bank 0 is not");
        memory.write_paging_port(5);
        assert!(
            memory.is_contended(0xC000),
            "bank 5 paged into 0xC000 must bring its contention with it"
        );
        memory.write_paging_port(2);
        assert!(!memory.is_contended(0xC000), "bank 2 is not contended");
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
        assert!(rendered.contains("paging_port"), "the SSOT must be visible");
    }

    // -----------------------------------------------------------------------------
    // The paging port
    // -----------------------------------------------------------------------------

    #[test]
    fn a_48k_derives_the_published_slot_map_from_its_reset_port_value() {
        // The compile-time assertion above is the real gate. This is what names it in a
        // failure message, and it checks the machine as built rather than the function.
        let memory = memory();
        assert_eq!(memory.paging_port(), 0x20);
        assert_eq!(memory.slots(), SPECTRUM_48K_SLOTS);
    }

    #[test]
    fn a_48k_absorbs_every_write_to_the_paging_port() {
        // No model check anywhere makes this true — the lock bit does. Exhaustive because
        // the axis is 256 values wide and checking it costs microseconds.
        let mut memory = memory();
        for value in 0..=u8::MAX {
            memory.write_paging_port(value);
            assert_eq!(memory.paging_port(), 0x20, "value {value:#04X} took effect");
            assert_eq!(memory.slots(), SPECTRUM_48K_SLOTS, "value {value:#04X}");
            assert_eq!(memory.screen_bank(), BankIndex::new(NORMAL_SCREEN_BANK));
        }
    }

    #[test]
    fn a_128_pages_the_bank_that_bits_zero_to_two_name() {
        let mut memory = memory_128();
        for bank in 0..8_u8 {
            memory.write_paging_port(bank);
            assert_eq!(
                memory.slot_at(0xC000),
                Slot::Bank(BankIndex::new(bank)),
                "bank {bank}"
            );
            // The other three slots are wired and no port bit reaches them.
            assert_eq!(memory.slot_at(0x4000), Slot::Bank(BankIndex::new(5)));
            assert_eq!(memory.slot_at(0x8000), Slot::Bank(BankIndex::new(2)));
        }
    }

    #[test]
    fn a_128_selects_its_rom_with_bit_four() {
        // Written as a read of an actual byte rather than of the slot, because "which ROM is
        // selected" and "which ROM's bytes appear at 0x0000" are the two things that must
        // agree and a slot comparison would only check the first.
        let mut memory = memory_128();
        assert_eq!(memory.slot_at(0x0000), Slot::Rom(RomIndex::new(0)));
        assert_eq!(memory.read(0x0001), pattern(0)[1]);

        memory.write_paging_port(PAGING_ROM);
        assert_eq!(memory.slot_at(0x0000), Slot::Rom(RomIndex::new(1)));
        assert_eq!(memory.read(0x0001), pattern(0xFF)[1]);
        assert_ne!(pattern(0)[1], pattern(0xFF)[1], "the two ROMs must differ");
    }

    #[test]
    fn a_128_selects_its_screen_with_bit_three_and_nothing_else_moves() {
        // `M7.md` Decision 3's first gated property: bit 3 changes what the ULA draws from,
        // and changes neither the slot map nor what an address reads.
        let mut memory = memory_128();
        memory.write_paging_port(0);
        let before = memory.slots();
        assert_eq!(memory.screen_bank(), BankIndex::new(5));

        memory.write_paging_port(PAGING_SCREEN);
        assert_eq!(memory.screen_bank(), BankIndex::new(7));
        assert_eq!(memory.slots(), before, "the slot map must not move");
        assert!(
            memory.is_contended(0x4000),
            "0x4000 is still bank 5 and still contended"
        );
    }

    #[test]
    fn the_contended_set_does_not_follow_the_screen_select() {
        // `M7.md` Decision 3's second gated property, and the one a naive model gets wrong in
        // a way that passes every bank-5 test: banks 1, 3, 5 and 7 are contended whichever
        // one the ULA happens to be drawing.
        let mut memory = memory_128();
        for screen in [0, PAGING_SCREEN] {
            for bank in 0..8_u8 {
                memory.write_paging_port(screen | bank);
                assert_eq!(
                    memory.is_contended(0xC000),
                    bank % 2 == 1,
                    "bank {bank} with screen bit {screen:#04X}"
                );
            }
        }
    }

    #[test]
    fn the_lock_is_absorbing_until_a_reset() {
        let mut memory = memory_128();
        memory.write_paging_port(3);
        assert_eq!(memory.slot_at(0xC000), Slot::Bank(BankIndex::new(3)));

        memory.write_paging_port(PAGING_LOCK | 1);
        assert_eq!(
            memory.slot_at(0xC000),
            Slot::Bank(BankIndex::new(1)),
            "the write that sets the lock still takes effect"
        );

        for value in 0..=u8::MAX {
            memory.write_paging_port(value);
            assert_eq!(
                memory.slot_at(0xC000),
                Slot::Bank(BankIndex::new(1)),
                "value {value:#04X} got past the lock"
            );
        }

        memory.reset();
        assert_eq!(memory.paging_port(), 0x00, "reset clears the lock");
        memory.write_paging_port(4);
        assert_eq!(memory.slot_at(0xC000), Slot::Bank(BankIndex::new(4)));
    }

    #[test]
    fn the_lock_bypassing_setter_reaches_a_locked_machine() {
        // The applier's route. A restore into a machine that locked itself is exactly the
        // case `write_paging_port` gets silently wrong, and every field a round trip compares
        // would still match.
        let mut memory = memory_128();
        memory.write_paging_port(PAGING_LOCK);
        memory.write_paging_port(6);
        assert_eq!(memory.slot_at(0xC000), Slot::Bank(BankIndex::new(0)));

        memory.set_paging_port(PAGING_LOCK | PAGING_SCREEN | 6);
        assert_eq!(memory.slot_at(0xC000), Slot::Bank(BankIndex::new(6)));
        assert_eq!(memory.screen_bank(), BankIndex::new(7));
        assert_eq!(
            memory.paging_port(),
            PAGING_LOCK | PAGING_SCREEN | 6,
            "and the restored machine is locked again, as its snapshot said"
        );
    }

    #[test]
    fn reset_restores_the_power_on_map_and_leaves_ram_alone() {
        let mut memory = memory_128();
        memory.write_paging_port(7);
        memory.write(0xC000, 0xA5);
        memory.write_paging_port(PAGING_ROM | PAGING_SCREEN | PAGING_LOCK | 3);

        memory.reset();
        assert_eq!(memory.paging_port(), 0x00);
        assert_eq!(
            memory.slots(),
            SPECTRUM_48K_SLOTS,
            "a 128 resets to this map too"
        );
        assert_eq!(memory.screen_bank(), BankIndex::new(5));
        assert_eq!(
            memory.bank(BankIndex::new(7))[0],
            0xA5,
            "reset does not clear RAM"
        );
    }

    #[test]
    fn a_48k_reset_is_a_no_op_by_construction() {
        let mut memory = memory();
        memory.write(0x8000, 0xA5);
        let before = memory.slots();
        memory.reset();
        assert_eq!(memory.slots(), before);
        assert_eq!(memory.paging_port(), 0x20);
        assert_eq!(memory.read(0x8000), 0xA5);
    }

    #[test]
    fn all_256_paging_port_values_leave_a_coherent_machine() {
        // The exhaustive check `M7.md` Decision 2 asks for. This crate builds with
        // `panic = "abort"` in release and the paging port is guest-controlled input, so
        // "no value is hostile" has to be a property rather than a hope. Cheap, and
        // exhaustive on the axis that matters.
        for value in 0..=u8::MAX {
            let mut memory = memory_128();
            memory.set_paging_port(value);

            // Every slot resolves, every address reads, and the map matches the byte.
            assert_eq!(memory.slots(), slots_for(value), "value {value:#04X}");
            assert_eq!(
                memory.slot_at(0xC000),
                Slot::Bank(BankIndex::new(value & PAGING_BANK))
            );
            assert_eq!(
                memory.slot_at(0x0000),
                Slot::Rom(RomIndex::new((value & PAGING_ROM) >> 4))
            );
            let screen = memory.screen_bank().get();
            assert!(screen == NORMAL_SCREEN_BANK || screen == SHADOW_SCREEN_BANK);

            // One address in each slot, read and written, so nothing indexes out of range.
            for address in [0x0000_u16, 0x4000, 0x8000, 0xC000, 0xFFFF] {
                memory.write(address, 0x5A);
                let _ = memory.read(address);
                let _ = memory.is_contended(address);
            }
        }
    }

    #[test]
    fn bits_six_and_seven_are_not_decoded() {
        // Unused on a 128 — the +2A/+3 puts a second port at 0x1FFD instead. A value with
        // them set must derive exactly the map the value without them does.
        for value in 0..0x40_u8 {
            assert!(slot_maps_eq(slots_for(value), slots_for(value | 0xC0)));
        }
    }

    // -----------------------------------------------------------------------------
    // Banks without addresses
    // -----------------------------------------------------------------------------

    #[test]
    fn a_bank_agrees_with_read_wherever_the_slot_map_exposes_it() {
        // The two routes to a byte must not diverge, or the snapshot writer and the machine
        // would disagree about what is in memory.
        let mut memory = memory_128();
        for bank in 0..8_u8 {
            memory.write_paging_port(bank);
            memory.write(0xC000, 0x10 + bank);
            memory.write(0xFFFF, 0x20 + bank);
            assert_eq!(memory.bank(BankIndex::new(bank))[0], 0x10 + bank);
            assert_eq!(
                memory.bank(BankIndex::new(bank))[PAGE_SIZE - 1],
                0x20 + bank
            );
        }
        // And the banks wired to fixed slots, through their own addresses.
        memory.write(0x4000, 0xE5);
        memory.write(0x8000, 0xE2);
        assert_eq!(memory.bank(BankIndex::new(5))[0], 0xE5);
        assert_eq!(memory.bank(BankIndex::new(2))[0], 0xE2);
    }

    #[test]
    fn a_bank_with_no_address_is_still_reachable_and_still_distinct() {
        // The case `Memory::bank` exists for: on a 128 five banks have no address at any
        // moment, and a snapshot has to carry them anyway.
        let mut memory = memory_128();
        for bank in 0..8_u8 {
            memory.bank_mut(BankIndex::new(bank))[0] = 0xB0 + bank;
        }
        // Paged out — bank 0 is at 0xC000 and the other six are nowhere.
        memory.write_paging_port(0);
        for bank in 0..8_u8 {
            assert_eq!(
                memory.bank(BankIndex::new(bank))[0],
                0xB0 + bank,
                "bank {bank} must not alias another"
            );
        }
        assert_eq!(
            memory.read(0xC000),
            0xB0,
            "bank 0 is the one with an address"
        );
    }

    #[test]
    fn a_bank_write_is_visible_through_the_address_that_shows_it() {
        let mut memory = memory_128();
        memory.write_paging_port(7);
        memory.bank_mut(BankIndex::new(7))[0x1234] = 0x99;
        assert_eq!(memory.read(0xD234), 0x99);
    }

    #[test]
    fn a_bank_index_out_of_the_models_range_still_names_a_bank() {
        // Total by signature: `bank` cannot fail and cannot panic, whatever a caller passes.
        let memory = memory();
        assert_eq!(memory.bank(BankIndex::new(0xFF)).len(), PAGE_SIZE);
    }

    #[test]
    fn the_model_is_what_the_constructor_said() {
        assert_eq!(memory().model(), Model::Spectrum48K);
        assert_eq!(memory_128().model(), Model::Spectrum128);
    }
}
