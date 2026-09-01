//! Gate **T2**: the real 128 ROMs boot to the 128's own menu, and *48 BASIC* is reached
//! **from the keyboard, through the second ROM**.
//!
//! # Why this is the 128's equivalent of the boot gate, and why it grades more
//!
//! `tests/boot.rs` measured what reaching `© 1982 Sinclair Research Ltd` on a 48K was worth: by
//! mutation, it graded the memory map and the screen and almost nothing else. Four of its five
//! probes left it green.
//!
//! This gate reaches the same class of verdict for the 128 and then does something the 48K's
//! cannot: **it selects a menu entry.** That single motion exercises the ULA's keyboard read,
//! the ROM-select bit of `0x7FFD`, the second ROM page, and the paging lock the ROM sets on its
//! way into 48K mode — and it ends on a string this project already knows how to assert against
//! glyphs from the ROM's own character set. The two copyright messages differ by their **year**,
//! so arriving at the 1982 one is a claim that the *other* ROM is executing, not merely that
//! something was drawn.
//!
//! # The frame numbers are change detectors, not hardware figures
//!
//! Exactly as `tests/boot.rs` says of its 87: nothing here establishes that a real 128 reaches
//! its menu on frame 57. A legitimate change to the machine's timing will move these, and the
//! right response is to re-measure and update them **having seen them move** — which is the one
//! thing an unasserted printout never forces.
//!
//! # What is **not** graded here
//!
//! - **The ROM images themselves.** Their provenance is bytes-and-hashes work, not this gate's.
//! - **Key-repeat and debounce timing.** `tap` holds a key for four frames because that is
//!   comfortably enough; nothing measures what the minimum is.
//! - **Anything about contention.** The 128's timing constants could be wrong by a wide margin
//!   and this gate would still be green — the ROM's start-up is a sequence of instructions, not
//!   of deadlines. That is the same finding `tests/boot.rs` recorded for the 48K, where deleting
//!   contention entirely left the message appearing and only moved the frame it appeared on.

mod common;
mod m7_common;

use m7_common::{screen_text, sinclair_rom_128, tap};
use spectrum::memory::{BankIndex, RomIndex, Slot};
use spectrum::{Key, Spectrum};
use z80::Bus;

/// What the 128's own editor ROM prints. The **year** is what makes it the 128's.
const COPYRIGHT_128: &str = "\u{a9} 1986 Sinclair Research Ltd";

/// What the 48 BASIC ROM prints — the same string the 48K boot gate asserts.
const COPYRIGHT_48: &str = "\u{a9} 1982 Sinclair Research Ltd";

/// The menu, top to bottom, as the ROM draws it.
const MENU: [&str; 5] = [
    "Tape Loader",
    "128 BASIC",
    "Calculator",
    "48 BASIC",
    "Tape Tester",
];

/// The character row each [`MENU`] entry is drawn on.
const FIRST_MENU_ROW: usize = 8;

/// Which [`MENU`] entry selects 48 BASIC, and therefore how far the highlight must move.
const BASIC_48: usize = 3;

/// The frame the 128's copyright message is **complete** on.
///
/// Not the frame the first glyph of it appears on, which is 57 — the ROM draws the line
/// character by character and `© 1986 Sin` is on screen a frame before the whole string is. The
/// distinction is worth the sentence because it is the sort of thing that makes a re-measured
/// number look like a regression: a gate matching a prefix and a gate matching the string
/// legitimately disagree by one.
const COPYRIGHT_FRAME: u64 = 58;

/// The frame the menu is fully drawn by.
const MENU_FRAME: u64 = 120;

/// Frames to allow after selecting *48 BASIC* before its message must be up.
const HANDOVER_FRAMES: u64 = 200;

/// A booted 128 and its 48 BASIC ROM image, or `None` when the corpus policy says to skip.
fn booted() -> Option<(Spectrum, Vec<u8>)> {
    let (editor, basic) = sinclair_rom_128()?;
    let mut machine = Spectrum::spectrum_128(&editor, &basic).expect("two page-sized ROMs");
    machine.run_frames(MENU_FRAME);
    Some((machine, basic))
}

/// Whether any line of the screen contains `needle`.
fn shows(machine: &Spectrum, font: &[u8], needle: &str) -> bool {
    screen_text(machine, font)
        .iter()
        .any(|line| line.contains(needle))
}

/// The attribute byte of the tenth column of character row `row`.
///
/// The menu's highlight is a colour inversion, so it is invisible to a glyph reader — which is
/// why "which entry is selected" has to be asked of the attribute file rather than of the text.
fn attribute(machine: &Spectrum, row: usize) -> u8 {
    let page = machine.memory().bank(machine.memory().screen_bank());
    let address = spectrum::screen::attribute_address(9, u8::try_from(row).expect("24 rows"));
    page[usize::from(address - spectrum::screen::DISPLAY_FILE)]
}

/// Which menu entry is highlighted, if exactly one is.
fn highlighted(machine: &Spectrum) -> Option<usize> {
    let rows: Vec<u8> = (0..MENU.len())
        .map(|entry| attribute(machine, FIRST_MENU_ROW + entry))
        .collect();
    let odd_one_out: Vec<usize> = (0..rows.len())
        .filter(|&entry| rows.iter().filter(|&&other| other == rows[entry]).count() == 1)
        .collect();
    match odd_one_out.as_slice() {
        [entry] => Some(*entry),
        _ => None,
    }
}

#[test]
fn the_128_boots_to_its_own_copyright_message_on_a_known_frame() {
    let Some((editor, basic)) = sinclair_rom_128() else {
        return;
    };
    let mut machine = Spectrum::spectrum_128(&editor, &basic).expect("two page-sized ROMs");

    let mut appeared = None;
    for _ in 0..MENU_FRAME {
        machine.run_frame();
        if appeared.is_none() && shows(&machine, &basic, COPYRIGHT_128) {
            appeared = Some(machine.frames());
        }
    }

    assert_eq!(
        appeared,
        Some(COPYRIGHT_FRAME),
        "the 128 must reach {COPYRIGHT_128:?}, and take the same number of frames to do it. \
         The screen as it stands:\n{}",
        screen_text(&machine, &basic).join("\n")
    );
    assert_eq!(machine.fault(), None);
}

#[test]
fn the_menu_is_drawn_with_all_five_entries() {
    let Some((machine, basic)) = booted() else {
        return;
    };
    let text = screen_text(&machine, &basic);
    for (entry, expected) in MENU.iter().enumerate() {
        assert!(
            text[FIRST_MENU_ROW + entry].contains(expected),
            "row {} should read {expected:?}, and reads {:?}",
            FIRST_MENU_ROW + entry,
            text[FIRST_MENU_ROW + entry]
        );
    }
    assert!(shows(&machine, &basic, COPYRIGHT_128));
}

#[test]
fn the_machine_is_still_running_its_interrupt_loop_at_the_menu() {
    // The message being on screen is a snapshot; this is the assertion that the machine reached
    // it and did not then wedge. `HALT` with interrupts off, or interrupts never accepted
    // again, would both leave the screen looking perfect.
    let Some((machine, _)) = booted() else {
        return;
    };
    let state = machine.cpu_state();
    assert!(state.iff1, "the menu loop runs with interrupts enabled");
    assert!(!state.halted, "the machine must not be wedged in HALT");
    assert_eq!(machine.frames(), MENU_FRAME);
}

#[test]
fn the_menu_starts_on_the_first_entry_and_the_keyboard_moves_the_highlight() {
    let Some((mut machine, _)) = booted() else {
        return;
    };
    assert_eq!(highlighted(&machine), Some(0), "Tape Loader is selected");

    for (entry, name) in MENU.iter().enumerate().skip(1) {
        tap(&mut machine, &[Key::CapsShift, Key::Num6]);
        assert_eq!(
            highlighted(&machine),
            Some(entry),
            "the highlight should be on {name:?}"
        );
    }
}

#[test]
fn selecting_48_basic_from_the_keyboard_reaches_the_second_rom() {
    // **The gate.** Every step is a thing the machine does rather than a thing the test does:
    // the highlight moves because the ULA's keyboard read reports the key, the ROM changes
    // because the editor writes bit 4 of `0x7FFD`, and the message appears because the second
    // ROM is executing.
    let Some((mut machine, basic)) = booted() else {
        return;
    };
    assert_eq!(
        machine.memory().slot_at(0x0000),
        Slot::Rom(RomIndex::new(0))
    );

    for _ in 0..BASIC_48 {
        tap(&mut machine, &[Key::CapsShift, Key::Num6]);
    }
    assert_eq!(highlighted(&machine), Some(BASIC_48), "48 BASIC");

    tap(&mut machine, &[Key::Enter]);
    let mut appeared = None;
    for _ in 0..HANDOVER_FRAMES {
        machine.run_frame();
        if appeared.is_none() && shows(&machine, &basic, COPYRIGHT_48) {
            appeared = Some(machine.frames());
        }
    }

    assert!(
        appeared.is_some(),
        "48 BASIC must print {COPYRIGHT_48:?}. The screen as it stands:\n{}",
        screen_text(&machine, &basic).join("\n")
    );
    assert!(
        !shows(&machine, &basic, COPYRIGHT_128),
        "and the 128's own message must be gone"
    );
    assert_eq!(machine.fault(), None);
}

#[test]
fn the_machine_that_48_basic_leaves_behind_is_a_48ks_memory_map() {
    // What "48 BASIC" means to the hardware, and it is the whole of `M7.md` Decision 1's
    // equation arriving from the other direction: the ROM puts the machine into exactly the
    // configuration a 48K powers on in — second ROM, bank 0 at 0xC000, screen bank 5, **paging
    // locked**. That last one is the interesting assertion, because it is the state a real 48K
    // is permanently in.
    let Some((mut machine, basic)) = booted() else {
        return;
    };
    for _ in 0..BASIC_48 {
        tap(&mut machine, &[Key::CapsShift, Key::Num6]);
    }
    tap(&mut machine, &[Key::Enter]);
    machine.run_frames(HANDOVER_FRAMES);
    assert!(shows(&machine, &basic, COPYRIGHT_48));

    assert_eq!(
        machine.memory().slot_at(0x0000),
        Slot::Rom(RomIndex::new(1)),
        "48 BASIC is ROM page 1"
    );
    assert_eq!(
        machine.memory().slot_at(0xC000),
        Slot::Bank(BankIndex::new(0))
    );
    assert_eq!(machine.memory().screen_bank(), BankIndex::new(5));

    // **The RAM half of the map is a 48K's exactly; the ROM half necessarily is not**, and the
    // difference is worth asserting rather than glossing. A 48K's single image lives in ROM page
    // 0 because that is the only page it has; the 128 holds the same code in page **1**, because
    // page 0 is its own editor. So `0x7FFD = 0x30` reproduces a 48K's *behaviour* — same banks,
    // same screen, paging locked — while pointing at a different page number, and an assertion
    // that expected `SPECTRUM_48K_SLOTS` outright would be wrong for a reason that says nothing
    // about the machine.
    let banks: Vec<Slot> = machine.memory().slots()[1..].to_vec();
    let expected: Vec<Slot> = spectrum::memory::SPECTRUM_48K_SLOTS[1..].to_vec();
    assert_eq!(banks, expected, "the three RAM slots are a 48K's, exactly");
    assert_ne!(
        machine.memory().slots()[0],
        spectrum::memory::SPECTRUM_48K_SLOTS[0],
        "and the ROM slot is the one thing that cannot be"
    );

    // And the lock the ROM set is real: a guest write to the paging port is now absorbed, on a
    // machine that was paging freely a moment ago.
    let before = machine.memory().slots();
    machine.ula_mut().out_port(0x7FFD, 0x07);
    assert_eq!(
        machine.memory().slots(),
        before,
        "the ROM locked paging on its way into 48K mode"
    );
}

#[test]
fn the_two_copyright_messages_differ_by_the_year_and_that_is_what_makes_this_a_rom_test() {
    // Stated as an assertion because the whole gate rests on it: if the two ROMs printed the
    // same string, reaching it would say nothing about which one ran.
    assert_ne!(COPYRIGHT_128, COPYRIGHT_48);
    assert_eq!(COPYRIGHT_128.replace("1986", "1982"), COPYRIGHT_48);
}
