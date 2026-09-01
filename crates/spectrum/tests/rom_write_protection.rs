//! Gate: a write into a ROM slot changes nothing a later read can see.
//!
//! # Why this exists
//!
//! Making the ROM slot writable left the boot gate **green**. That is not surprising once
//! stated — the 48K ROM's own routines write through pointers that can legally address ROM,
//! and every one of those writes is discarded on the hardware, so a machine that honoured
//! them would corrupt itself only where the ROM happens to write over code it later
//! executes. The copyright message is drawn long before that matters.
//!
//! # What is graded here
//!
//! - Every byte of the ROM page survives a write aimed at every address in it.
//! - Writability follows the **slot map**, not the address range. `ARCHITECTURE.md`
//!   Decision 5 makes 48K a *configuration* of a paged machine, so "below `0x4000`" is a
//!   fact about this configuration and not the rule; the rule is "the slot shows a ROM
//!   page". A gate written against the address range would keep passing on a 128 whose
//!   paging port had just put a ROM page somewhere else.
//! - All three write paths: straight to the address space, through the bus, and through an
//!   executing `LD (nn),A`.
//! - A discarded write reaches no RAM bank either — discarding is not aliasing.
//!
//! # What is not graded here
//!
//! **That the ROM slot can ever be anything else.** A 48K has no paging port, so nothing in
//! this crate can page a RAM bank into slot 0 or a ROM page into slot 2, and the slot-driven
//! assertions below therefore run against exactly one map today. They are written that way
//! because M7 supplies the second map, not because two are being exercised now.

mod common;

use common::{
    UNCONTENDED_CODE, machine, pattern_rom, set_pc, write_program, write_through_the_bus,
};
use spectrum::memory::{PAGE_SIZE, SLOT_COUNT, Slot};

/// An address in the ROM page, clear of the reset and interrupt vectors.
const IN_ROM: u16 = 0x1234;

/// A byte no ROM address in [`pattern_rom`] holds, so "the write landed" is unambiguous.
///
/// The pattern is `address & 0xFF`, so address `0x1234` holds `0x34`; `0xA5` is neither that
/// nor the `0x00` an untouched RAM byte holds.
const INTRUDER: u8 = 0xA5;

/// One marker per RAM slot, so a write that leaked out of the ROM page has somewhere to show.
const RAM_MARKERS: [(u16, u8); 3] = [(0x4000, 0x11), (0x8000, 0x22), (0xC000, 0x33)];

#[test]
fn every_byte_of_the_rom_page_survives_a_write_to_every_address() {
    let mut machine = machine();
    let expected = pattern_rom();

    for address in 0..PAGE_SIZE {
        let address = u16::try_from(address).expect("a ROM page is smaller than 64 KB");
        let resident = machine.memory().read(address);
        machine.memory_mut().write(address, !resident);
    }

    let observed: Vec<u8> = (0..PAGE_SIZE)
        .map(|address| {
            machine
                .memory()
                .read(u16::try_from(address).expect("a ROM page is smaller than 64 KB"))
        })
        .collect();

    let first_change = observed
        .iter()
        .zip(&expected)
        .position(|(seen, want)| seen != want);
    assert_eq!(
        first_change, None,
        "a write into the ROM slot was honoured: the ROM page differs from the image it was \
         built with, first at offset {first_change:?}"
    );
}

#[test]
fn writability_follows_the_slot_map_and_not_the_address_range() {
    // Driven by what each slot *shows*, so the expectation would follow the map rather than
    // the address if a machine ever paged a ROM page somewhere other than slot 0.
    let mut machine = machine();

    for slot in 0..SLOT_COUNT {
        let base = u16::try_from(slot * PAGE_SIZE).expect("four slots span 64 KB");
        for offset in [0, 1, u16::try_from(PAGE_SIZE / 2).unwrap(), 0x3FFF] {
            let address = base + offset;
            let resident = machine.memory().read(address);
            let intruder = !resident;
            machine.memory_mut().write(address, intruder);
            let observed = machine.memory().read(address);

            match machine.memory().slot_at(address) {
                Slot::Rom(rom) => assert_eq!(
                    observed,
                    resident,
                    "{address:#06X} is in slot {slot}, which shows ROM page {}: the write \
                     must be discarded, not honoured",
                    rom.get()
                ),
                Slot::Bank(bank) => assert_eq!(
                    observed,
                    intruder,
                    "{address:#06X} is in slot {slot}, which shows RAM bank {}: the write \
                     must land. A gate where nothing is writable proves nothing about ROM",
                    bank.get()
                ),
            }
        }
    }
}

#[test]
fn slot_at_agrees_with_the_slot_map_across_the_whole_address_space() {
    // The indirection itself: a read or a write consults `slots[address >> 14]`, and this is
    // the assertion that the per-address answer and the whole map are the same answer.
    let machine = machine();
    let map = machine.memory().slots();

    for (slot, shown) in map.iter().enumerate() {
        let base = u16::try_from(slot * PAGE_SIZE).expect("four slots span 64 KB");
        for offset in [0, 1, 0x2000, 0x3FFF] {
            assert_eq!(
                machine.memory().slot_at(base + offset),
                *shown,
                "{:#06X} must resolve through slot {slot}",
                base + offset
            );
        }
    }

    assert!(
        matches!(map[0], Slot::Rom(_)),
        "a 48K shows its single ROM page in the bottom slot"
    );
    assert!(
        map[1..].iter().all(|slot| matches!(slot, Slot::Bank(_))),
        "the other three slots are RAM"
    );
}

#[test]
fn a_write_through_the_bus_into_rom_is_discarded() {
    // The path an executing instruction takes, which is not the path the assertions above
    // take. Both must honour the protection; a gate covering one leaves the other free.
    let mut machine = machine();
    let resident = machine.memory().read(IN_ROM);

    write_through_the_bus(&mut machine, IN_ROM, INTRUDER);

    assert_eq!(
        machine.memory().read(IN_ROM),
        resident,
        "a write through the ULA reached the ROM page"
    );
}

#[test]
fn a_write_the_cpu_performs_into_rom_is_discarded() {
    // `LD A,0xA5` then `LD (0x1234),A`, executed out of RAM. The whole path: decode, the
    // bus's write, the slot map.
    let mut machine = machine();
    let resident = machine.memory().read(IN_ROM);
    let [low, high] = IN_ROM.to_le_bytes();
    write_program(
        &mut machine,
        UNCONTENDED_CODE,
        &[0x3E, INTRUDER, 0x32, low, high],
    );
    set_pc(&mut machine, UNCONTENDED_CODE);

    machine.step();
    machine.step();

    assert_eq!(
        machine.cpu_state().af >> 8,
        u16::from(INTRUDER),
        "the program did not run as written: A should hold the byte it was about to store"
    );
    assert_eq!(
        machine.memory().read(IN_ROM),
        resident,
        "a `LD (nn),A` into the ROM slot was honoured"
    );
    assert_eq!(machine.fault(), None);
}

#[test]
fn a_discarded_write_reaches_no_ram_bank_either() {
    // Discarding is not aliasing. A slot map that quietly redirected ROM writes into a RAM
    // bank would pass every assertion above and corrupt memory the guest owns.
    let mut machine = machine();
    for (address, marker) in RAM_MARKERS {
        machine.memory_mut().write(address, marker);
    }

    for address in (0..PAGE_SIZE).step_by(64) {
        let address = u16::try_from(address).expect("a ROM page is smaller than 64 KB");
        machine.memory_mut().write(address, INTRUDER);
        write_through_the_bus(&mut machine, address, INTRUDER);
    }

    for (address, marker) in RAM_MARKERS {
        assert_eq!(
            machine.memory().read(address),
            marker,
            "a write aimed at the ROM page changed RAM at {address:#06X}"
        );
    }
}
