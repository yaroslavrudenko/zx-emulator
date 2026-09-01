//! Gate: bit 3 of `0x7FFD` changes what the ULA draws, and changes nothing else.
//!
//! # The three properties, and why each is a separate plausible wrong answer
//!
//! - **The ULA reads a bank, not an address.** A renderer that read `Memory::read(0x4000)` draws
//!   whatever the *slot map* shows at `0x4000` — which is bank 5, the screen a double-buffering
//!   program has just switched away from. It would show the buffer being drawn into, every
//!   frame, and it would pass every 48K test ever written.
//! - **Nothing else moves.** Bit 3 is not a paging bit: the slot map, what `Memory::read(0x4000)`
//!   returns, and which banks are contended are all unaffected. A model that paged bank 7 into
//!   slot 1 when the bit was set would render correctly and corrupt the machine.
//! - **The contended set does not follow the screen select.** Banks 1, 3, 5 and 7 are contended
//!   whichever one the ULA is drawing. A model that contended "the screen bank" is right about
//!   bank 5 and wrong about the other three — and every test that only ever selects bank 5
//!   agrees with it. That property is measured on the clock in `m7_contention.rs`; what is
//!   asserted here is the half this file can see, that selecting the shadow screen does not
//!   *change* the contended set.
//!
//! # What is **not** graded here
//!
//! **A mid-frame switch.** `render` takes the screen as it stands when it is called, so a
//! program that flips bit 3 partway down a frame is drawn as if the last value had applied all
//! frame — the same M5 boundary that makes multicolour effects unmodelled, unchanged by M7 and
//! named in [`spectrum::screen`].

mod common;
mod m7_common;

use common::{set_pc, write_program};
use m7_common::{OUT_C_A_STEPS, machine_128, out_c_a};
use spectrum::memory::BankIndex;
use spectrum::screen::{BORDER, CELL, DISPLAY_FILE, Frame, attribute_address, pixel_address};
use spectrum::{Colour, Spectrum};

const PAGING_PORT: u16 = 0x7FFD;
const PROGRAM: u16 = common::PROLOGUE;

/// Bit 3: clear draws bank 5, set draws bank 7.
const SCREEN_SELECT: u8 = 0x08;

/// The two banks the ULA can be wired to.
const NORMAL: u8 = 5;
const SHADOW: u8 = 7;

/// White ink on black paper, so a set bit is unambiguous.
const WHITE_INK: u8 = 0x07;

fn page(machine: &mut Spectrum, value: u8) {
    write_program(machine, PROGRAM, &out_c_a(PAGING_PORT, value));
    set_pc(machine, PROGRAM);
    for _ in 0..OUT_C_A_STEPS {
        machine.step();
    }
}

/// Put a lit pixel in the top-left cell of character column `column` of `bank`.
///
/// Written straight into the bank, because on a 128 the bank usually has no address — which is
/// the whole reason this gate exists.
fn mark(machine: &mut Spectrum, bank: u8, column: u8) {
    let page = machine.memory_mut().bank_mut(BankIndex::new(bank));
    let offset = |address: u16| usize::from(address - DISPLAY_FILE);
    page[offset(pixel_address(column, 0))] = 0x80;
    page[offset(attribute_address(column, 0))] = WHITE_INK;
}

/// Whether the leftmost pixel of character column `column` is lit in `frame`.
fn lit(frame: &Frame, column: usize) -> bool {
    frame.pixel(BORDER + column * CELL, BORDER) == Some(Colour::new(7))
}

/// Render `machine` into a fresh frame.
fn drawn(machine: &Spectrum) -> Frame {
    let mut frame = Frame::new();
    machine.render(&mut frame);
    frame
}

#[test]
fn selecting_the_shadow_screen_changes_what_is_drawn_and_nothing_else() {
    let mut machine = machine_128();
    mark(&mut machine, NORMAL, 0);
    mark(&mut machine, SHADOW, 1);

    let before_slots = machine.memory().slots();
    let before_4000 = machine.memory().read(0x4000);
    let before_contended: Vec<bool> = (0..4)
        .map(|slot| machine.memory().is_contended(slot * 0x4000))
        .collect();

    let normal = drawn(&machine);
    assert!(lit(&normal, 0), "bank 5's pixel");
    assert!(!lit(&normal, 1), "and not bank 7's");

    page(&mut machine, SCREEN_SELECT);

    let shadow = drawn(&machine);
    assert!(!lit(&shadow, 0), "bank 5's pixel must be gone");
    assert!(lit(&shadow, 1), "bank 7's must be drawn");

    // And nothing else moved.
    assert_eq!(machine.memory().slots(), before_slots, "the slot map");
    assert_eq!(machine.memory().read(0x4000), before_4000, "0x4000 reads");
    assert_eq!(
        (0..4)
            .map(|slot| machine.memory().is_contended(slot * 0x4000))
            .collect::<Vec<_>>(),
        before_contended,
        "which slots are contended"
    );
}

#[test]
fn the_ula_reads_the_bank_and_not_the_address() {
    // The sharpest form of the first property. Bank 7 is selected as the screen and paged into
    // **no slot at all**, so there is no address at which its bytes can be read — and it must
    // still be what appears on the display.
    let mut machine = machine_128();
    mark(&mut machine, SHADOW, 3);
    page(&mut machine, SCREEN_SELECT); // bits 0-2 are zero: bank 0 is at 0xC000

    assert_eq!(
        machine.memory().screen_bank(),
        BankIndex::new(SHADOW),
        "the ULA is wired to bank 7"
    );
    for slot in 0..4_u16 {
        assert_ne!(
            machine.memory().slot_at(slot * 0x4000),
            spectrum::memory::Slot::Bank(BankIndex::new(SHADOW)),
            "bank 7 must be paged out for this to mean anything"
        );
    }
    assert!(lit(&drawn(&machine), 3), "and it is still on the display");
}

#[test]
fn a_write_through_c000_reaches_the_shadow_screen_when_bank_seven_is_paged_there() {
    // The reason the shadow screen exists, end to end: a program pages bank 7 in, draws into it
    // through ordinary addresses while the ULA is still showing bank 5, then flips bit 3.
    let mut machine = machine_128();
    page(&mut machine, SHADOW); // bank 7 at 0xC000, screen still bank 5

    // The screen bank appears at 0x4000 when it is paged there, so an address within it is the
    // display-file address rebased onto whichever slot it is actually in.
    let rebased = |address: u16| 0xC000 + (address - DISPLAY_FILE);
    machine
        .memory_mut()
        .write(rebased(pixel_address(4, 0)), 0x80);
    machine
        .memory_mut()
        .write(rebased(attribute_address(4, 0)), WHITE_INK);

    assert!(
        !lit(&drawn(&machine), 4),
        "nothing is visible while bank 5 is the screen"
    );

    page(&mut machine, SCREEN_SELECT | SHADOW);
    assert!(lit(&drawn(&machine), 4), "and the flip shows it");
}

#[test]
fn the_shadow_screen_survives_being_paged_out_and_back() {
    // A bank with no address is still a bank. This is the property `Memory::bank` exists for,
    // seen from the screen's side.
    let mut machine = machine_128();
    mark(&mut machine, SHADOW, 5);
    page(&mut machine, SCREEN_SELECT | SHADOW);
    assert!(lit(&drawn(&machine), 5));

    page(&mut machine, SCREEN_SELECT | 2); // bank 2 at 0xC000; bank 7 has no address
    assert!(lit(&drawn(&machine), 5), "and it is unchanged");
}

#[test]
fn a_48k_has_no_shadow_screen_and_cannot_be_given_one() {
    // Bit 3 is absorbed like every other bit, so a 48K's ULA is wired to bank 5 permanently.
    // Written as a screen assertion rather than a `screen_bank()` one, because "which bank" and
    // "what is on the display" are two claims and only the second is what a user sees.
    let mut machine = common::machine();
    mark(&mut machine, NORMAL, 0);
    mark(&mut machine, SHADOW, 1);

    for value in [SCREEN_SELECT, 0xFF, SCREEN_SELECT | SHADOW] {
        page(&mut machine, value);
        let frame = drawn(&machine);
        assert!(
            lit(&frame, 0),
            "value {value:#04X}: bank 5 is still the screen"
        );
        assert!(
            !lit(&frame, 1),
            "value {value:#04X}: bank 7 must not appear"
        );
    }
}

#[test]
fn reading_the_screen_as_text_follows_the_same_bank_the_renderer_does() {
    // `read_text` is what the boot gates assert against. If it followed the slot map while
    // `render` followed the port, the 128's shadow screen would be half-modelled and both
    // halves would look green from their own side.
    let mut machine = machine_128();
    mark(&mut machine, SHADOW, 0);

    let before = spectrum::screen::read_text(machine.memory());
    page(&mut machine, SCREEN_SELECT);
    let after = spectrum::screen::read_text(machine.memory());
    assert_ne!(before[0], after[0], "the text view must follow bit 3");
    assert!(before[0].starts_with(' '), "bank 5 is blank");
    assert!(!after[0].starts_with(' '), "bank 7 is not");
}
