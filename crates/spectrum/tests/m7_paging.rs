//! Gate: the paging port `0x7FFD`, driven by a guest executing a real `OUT`.
//!
//! # What is graded here, and why it is not the same as [`spectrum::memory`]'s own tests
//!
//! `memory.rs` grades the **rule** — which bits mean what, that the lock absorbs, that all 256
//! values leave a coherent machine — by calling `write_paging_port` directly. That is the right
//! shape for the rule and it can say nothing about the **wiring**: whether a program executing
//! `OUT (C),A` actually reaches the port, through the CPU, the bus and the port decode.
//!
//! Every write here is therefore assembled as instructions and executed. The distinction is not
//! academic: `Ula::out_port`'s paging arm is a separate `if` from its border arm, added at M7,
//! and a decode that never fired would leave every test in `memory.rs` green.
//!
//! # What is **not** graded here
//!
//! - **The bit layout.** Transcribed from the World of Spectrum 128K reference and graded
//!   against the transcription. Nothing here could discover that bit 4 is really bit 5.
//! - **The partial decode.** *"Any port address with bits 1 and 15 reset"* is single-sourced;
//!   the family is asserted below, but only software reaching the port at some other address
//!   would grade the claim, and none is in reach.
//! - **Contention on the paging write itself.** `0x7FFD` lands in bank 5's address range, so it
//!   takes the contended-address I/O pattern; that is `io_contention.rs`'s subject and is
//!   unchanged by M7.

mod common;
mod m7_common;

use common::{machine, set_pc, write_program};
use m7_common::{BASIC_SEED, EDITOR_SEED, OUT_C_A_STEPS, machine_128, out_c_a, pattern_rom};
use spectrum::memory::{BankIndex, RomIndex, SLOT_COUNT, SPECTRUM_48K_SLOTS, Slot};
use spectrum::{Memory, Spectrum};

/// The canonical paging port address.
const PAGING_PORT: u16 = 0x7FFD;

/// Where the test programs are assembled: bank 2, which no paging value moves.
const PROGRAM: u16 = common::PROLOGUE;

/// Execute one `OUT (port),value` on `machine`.
fn page(machine: &mut Spectrum, port: u16, value: u8) {
    write_program(machine, PROGRAM, &out_c_a(port, value));
    set_pc(machine, PROGRAM);
    for _ in 0..OUT_C_A_STEPS {
        machine.step();
    }
}

/// The slot map as bank numbers, with `None` where a ROM sits.
fn map(machine: &Spectrum) -> [Option<u8>; SLOT_COUNT] {
    machine.memory().slots().map(|slot| match slot {
        Slot::Bank(bank) => Some(bank.get()),
        Slot::Rom(_) => None,
    })
}

#[test]
fn a_guest_out_reaches_the_paging_port_and_moves_the_bank_at_c000() {
    // The wiring assertion, and the one that would have been green in `memory.rs` with the
    // `out_port` arm missing entirely.
    let mut machine = machine_128();
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(0))
    );

    page(&mut machine, PAGING_PORT, 3);
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(3)),
        "an executed OUT must reach the port"
    );
}

/// Where a bank's signature goes: high in the `0xC000` slot, clear of [`PROGRAM`].
///
/// Not `0xC000` itself, and the reason is a property this file is about rather than an
/// inconvenience. The test program lives at `0x8000`, which is bank 2 at offset 0 — and when
/// bank 2 is the one paged into `0xC000`, `0xC000` **is** that same byte. A signature written
/// there is the program's first opcode, and the next `page()` call overwrites it. That is the
/// aliasing this gate exists to detect, arriving through the fixture instead of through the
/// model; `paging_bank_two_into_c000_aliases_the_program_at_8000` asserts it deliberately.
const SIGNATURE: u16 = 0xE000;

#[test]
fn every_bank_can_be_paged_in_and_holds_its_own_bytes() {
    // Eight banks, each written through the `0xC000` slot while it is there and read back
    // after the others have been. A model that aliased two banks passes any single-bank test.
    let mut machine = machine_128();
    for bank in 0..8_u8 {
        page(&mut machine, PAGING_PORT, bank);
        machine.memory_mut().write(SIGNATURE, 0xB0 + bank);
    }
    for bank in 0..8_u8 {
        page(&mut machine, PAGING_PORT, bank);
        assert_eq!(
            machine.memory().read(SIGNATURE),
            0xB0 + bank,
            "bank {bank} must not alias another"
        );
    }
}

#[test]
fn paging_bank_two_into_c000_aliases_the_program_at_8000() {
    // The consequence [`SIGNATURE`] exists for, asserted rather than merely avoided. Slot 2 is
    // wired to bank 2 on both machines and no port bit reaches it, so paging bank 2 into
    // `0xC000` puts the **same page** at two addresses at once. A model that copied pages
    // instead of aliasing them would pass every other test in this file.
    let mut machine = machine_128();
    page(&mut machine, PAGING_PORT, 2);
    machine.memory_mut().write(0xC123, 0x5A);
    assert_eq!(
        machine.memory().read(0x8123),
        0x5A,
        "one bank in two slots is one page, not two"
    );

    // And the reverse direction, which is what actually bit: a write through `0x8000` is
    // visible at `0xC000`.
    machine.memory_mut().write(0x8456, 0xA5);
    assert_eq!(machine.memory().read(0xC456), 0xA5);
}

#[test]
fn the_rom_select_bit_swaps_which_rom_answers_at_zero() {
    // Read as a byte rather than as a slot: "which ROM is selected" and "whose bytes appear at
    // 0x0000" are two claims, and only the second is what a guest experiences.
    let mut machine = machine_128();
    let editor = pattern_rom(EDITOR_SEED);
    let basic = pattern_rom(BASIC_SEED);
    assert_ne!(editor[0x1234], basic[0x1234], "the two ROMs must differ");

    assert_eq!(machine.memory().read(0x1234), editor[0x1234]);
    page(&mut machine, PAGING_PORT, 0x10);
    assert_eq!(machine.memory().read(0x1234), basic[0x1234]);
    page(&mut machine, PAGING_PORT, 0x00);
    assert_eq!(machine.memory().read(0x1234), editor[0x1234]);
}

#[test]
fn the_lock_is_absorbing_and_only_a_reset_clears_it() {
    let mut machine = machine_128();
    page(&mut machine, PAGING_PORT, 3);
    page(&mut machine, PAGING_PORT, 0x20 | 1);
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(1)),
        "the write that sets the lock still takes effect"
    );

    // Every value, through a real OUT, against a locked machine.
    for value in 0..=u8::MAX {
        page(&mut machine, PAGING_PORT, value);
        assert_eq!(
            machine.memory().slot_at(0xC000),
            Slot::Bank(BankIndex::new(1)),
            "value {value:#04X} got past the lock"
        );
    }

    machine.reset();
    assert_eq!(map(&machine), map(&machine_128()), "reset restores the map");
    page(&mut machine, PAGING_PORT, 4);
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(4)),
        "and the machine pages again"
    );
}

#[test]
fn reset_clears_the_lock_without_clearing_ram() {
    // The half of the reset rule that is easy to over-apply. `Ula::reset` gained a call to
    // `Memory::reset` at M7, and a `Memory::reset` that cleared RAM would pass every paging
    // assertion above.
    let mut machine = machine_128();
    page(&mut machine, PAGING_PORT, 7);
    machine.memory_mut().write(0xC000, 0xA5);
    page(&mut machine, PAGING_PORT, 0x20 | 7);

    machine.reset();
    page(&mut machine, PAGING_PORT, 7);
    assert_eq!(
        machine.memory().read(0xC000),
        0xA5,
        "a reset button does not clear RAM"
    );
}

#[test]
fn a_48k_absorbs_every_paging_write_a_guest_can_make() {
    // `M7.md` Decision 1's equation, through the machine: a 48K needs no model check because
    // it powers on with the lock bit set. Exhaustive over the value axis, because it is 256
    // wide and this is the property that keeps every existing 48K gate meaning what it meant.
    let mut machine = machine();
    let before = map(&machine);
    assert_eq!(machine.memory().slots(), SPECTRUM_48K_SLOTS);

    for value in 0..=u8::MAX {
        page(&mut machine, PAGING_PORT, value);
        assert_eq!(
            map(&machine),
            before,
            "value {value:#04X} took effect on a 48K"
        );
        assert_eq!(
            machine.memory().screen_bank(),
            BankIndex::new(5),
            "value {value:#04X} moved a 48K's screen"
        );
        assert_eq!(
            machine.memory().slot_at(0x0000),
            Slot::Rom(RomIndex::new(0)),
            "value {value:#04X} selected a second ROM a 48K does not have"
        );
    }
}

#[test]
fn a_48k_still_reads_its_own_rom_after_every_paging_write() {
    // The consequence a slot comparison would miss: a 48K has one ROM image loaded and page 1
    // is blank, so a paging write that *did* take effect would read zeros rather than the ROM.
    let rom = common::pattern_rom();
    let mut machine = Spectrum::new(&rom).expect("a page-sized ROM");
    for value in [0x00_u8, 0x10, 0x30, 0xFF] {
        page(&mut machine, PAGING_PORT, value);
        assert_eq!(
            machine.memory().read(0x1234),
            rom[0x1234],
            "value {value:#04X}"
        );
    }
}

#[test]
fn the_port_answers_across_its_decoded_family_and_not_outside_it() {
    // The published decode is *"any port address with bits 1 and 15 reset"*. Transcribed, and
    // graded here against the transcription — which is all anything in reach can do, and is
    // said plainly in this file's header rather than implied by a green test.
    //
    // The negative half is the half that matters: an equality-based decode passes every
    // address in the first list and fails every address in the second.
    let responds = [0x7FFD_u16, 0x7FFC, 0x0000, 0x1234, 0x00FC, 0x7D00];
    let ignores = [0x8000_u16, 0xFFFD, 0xBFFD, 0x7FFF, 0x0002, 0x00FE];

    for port in responds {
        let mut machine = machine_128();
        page(&mut machine, port, 5);
        assert_eq!(
            machine.memory().slot_at(0xC000),
            Slot::Bank(BankIndex::new(5)),
            "{port:#06X} has A15 and A1 reset, so the paging port must answer"
        );
    }
    for port in ignores {
        let mut machine = machine_128();
        page(&mut machine, port, 5);
        assert_eq!(
            machine.memory().slot_at(0xC000),
            Slot::Bank(BankIndex::new(0)),
            "{port:#06X} has A15 or A1 set, so the paging port must not answer"
        );
    }
}

#[test]
fn a_paging_write_and_a_border_write_do_not_suppress_each_other() {
    // Why `out_port` is two independent `if`s. The two decodes select on disjoint address
    // lines, so an address with A0, A1 and A15 all reset is claimed by both devices — and a
    // `match` on the port would silently pick one.
    //
    // The *canonical* addresses do not overlap: `0x00FE` has A1 set and `0x7FFD` has A0 set.
    // So this needs an address in the intersection, and `0x00FC` is one.
    let mut machine = machine_128();
    assert_eq!(machine.border(), spectrum::Colour::new(0));

    page(&mut machine, 0x00FC, 0x06);
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(6)),
        "bits 0-2 must reach the paging port"
    );
    assert_eq!(
        machine.border(),
        spectrum::Colour::new(6),
        "and bits 0-2 must reach the border, from the same write"
    );
}

#[test]
fn the_canonical_addresses_reach_exactly_one_device_each() {
    // The sharper fact the compile-time assertions in `ula.rs` established: the two families
    // overlap and their published members do not. Worth a gate of its own, because it is what
    // makes the `if`/`if` shape invisible to ordinary software — and therefore what makes the
    // previous test the only thing that grades it.
    let mut machine = machine_128();
    page(&mut machine, 0x00FE, 0x07);
    assert_eq!(machine.border(), spectrum::Colour::new(7));
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(0)),
        "0x00FE has A1 set: it is not a paging write"
    );

    let mut machine = machine_128();
    page(&mut machine, 0x7FFD, 0x03);
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(3))
    );
    assert_eq!(
        machine.border(),
        spectrum::Colour::new(0),
        "0x7FFD has A0 set: it is not a border write"
    );
}

#[test]
fn the_slot_map_a_128_resets_to_is_the_48ks_published_map() {
    // `M7.md` Decision 1's equation, from the other end: a 128 at `0x7FFD = 0x00` and a 48K
    // have the *same* map. That is what makes "48K is a configuration of the 128" checkable
    // rather than a slogan, and it is asserted at compile time in `memory.rs` as well.
    let one_two_eight = machine_128();
    let forty_eight = machine();
    assert_eq!(one_two_eight.memory().slots(), forty_eight.memory().slots());
    assert_eq!(one_two_eight.memory().slots(), SPECTRUM_48K_SLOTS);
    assert_eq!(
        one_two_eight.memory().screen_bank(),
        forty_eight.memory().screen_bank()
    );
}

#[test]
fn no_paging_value_can_make_a_machine_incoherent() {
    // This crate builds with `panic = "abort"` in release, and the paging port is
    // guest-controlled input by definition: any of 256 values, at any moment, from any
    // program. So "no value is hostile" has to be a property rather than a hope, and it is
    // asserted through a real `OUT` here as well as against the rule in `memory.rs`.
    for value in 0..=u8::MAX {
        let mut machine = machine_128();
        page(&mut machine, PAGING_PORT, value);

        // Every address in every slot, read and written, plus a render — which reaches the
        // screen bank, the one indirection a wild value could push out of range.
        for address in [0x0000_u16, 0x3FFF, 0x4000, 0x8000, 0xC000, 0xFFFF] {
            machine.memory_mut().write(address, 0x5A);
            let _ = machine.memory().read(address);
        }
        let mut frame = spectrum::Frame::new();
        machine.render(&mut frame);
        assert_eq!(machine.fault(), None, "value {value:#04X}");
    }
}

#[test]
fn the_two_rom_images_land_in_the_order_the_constructor_names() {
    // A `Memory::spectrum_128` that loaded the same image into both pages, or swapped them,
    // would pass every test above that only ever compares a slot rather than a byte. Checked
    // through the machine, both ways round, with images that differ at every address.
    let mut machine = Spectrum::spectrum_128(&pattern_rom(0x11), &pattern_rom(0x22))
        .expect("two page-sized ROMs");
    assert_eq!(machine.memory().read(0x0000), 0x11, "the editor is page 0");
    page(&mut machine, PAGING_PORT, 0x10);
    assert_eq!(machine.memory().read(0x0000), 0x22, "48 BASIC is page 1");

    let mut swapped = Spectrum::spectrum_128(&pattern_rom(0x22), &pattern_rom(0x11))
        .expect("two page-sized ROMs");
    assert_eq!(swapped.memory().read(0x0000), 0x22);
    page(&mut swapped, PAGING_PORT, 0x10);
    assert_eq!(swapped.memory().read(0x0000), 0x11);
}

#[test]
fn a_wrong_sized_rom_is_refused_in_either_position() {
    assert!(Spectrum::spectrum_128(&[0; 16], &pattern_rom(0)).is_err());
    assert!(Spectrum::spectrum_128(&pattern_rom(0), &[0; 16]).is_err());
    let _: Memory = Memory::spectrum_128(&pattern_rom(0), &pattern_rom(1)).expect("two ROMs");
}
