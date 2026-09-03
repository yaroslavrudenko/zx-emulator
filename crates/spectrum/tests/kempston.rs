//! Gate: the Kempston joystick, read by a guest executing a real `IN`.
//!
//! # What is graded, and at what class
//!
//! | | Class | |
//! |---|---|---|
//! | A guest's `IN A,(0x1F)` reaches the interface | **proven** | `joystick`'s own tests call `Joystick::read` directly and cannot see the port decode or the bus |
//! | The bit layout | **derived** — transcribed, and graded against the transcription | Every source agrees on it; a permutation would send a game the wrong way and nothing else here would fail |
//! | Active high, against the keyboard's active low | **proven** | A model with the convention inverted reads `0x1F` idle: every direction held, forever |
//! | The joystick does not disturb the keyboard, or the keyboard the joystick | **proven** | They are different ports and the whole point of using Kempston is that they cannot collide |
//!
//! # What nothing here grades
//!
//! - **The decode, as a *behaviour*.** The decode itself is no longer unsourced — the Kempston
//!   Issue 4 (1989) schematic gives `A5 = A6 = A7 = 0` and `spectrum::joystick` transcribes it —
//!   but the assertions below grade this crate against that transcription and could not discover
//!   the transcription is wrong. *(This row read **"No source for it was found"** and described
//!   `KEMPSTON_PORT_MASK` as matching *"the canonical address's low byte"* while claiming
//!   *"nothing about address lines"*. Both were already false of the constant beside them —
//!   `0x00E0` is not `0x00FF`, and a mask **is** a claim about address lines. This was the
//!   fourth copy of that sentence in the repository and the one no review listed; it was found
//!   by grepping for the pattern rather than by being pointed at.)*
//! - **What the unused top three bits read as, as a *behaviour*.** The same schematic sources
//!   them — D5 through an inverting `74LS366`, D6 and D7 pulled low through two `1N4148`s — so
//!   zero is transcribed rather than assumed; what nothing here grades is the transcription.
//! - **That any real game responds to it.** That is T4 — a person, a game, and a look.

mod common;

use common::{machine, set_pc, write_program};
use spectrum::joystick::{Direction, KEMPSTON_PORT};
use spectrum::{Key, Spectrum};
use z80::Bus;

/// Where the reading program is assembled.
const PROGRAM: u16 = common::PROLOGUE;

/// `IN A,(n)` — two bytes, eleven T-states.
const IN_A_N: u8 = 0xDB;

/// The five bits, **transcribed** rather than imported. Taking them from the crate would make
/// this file agree with `joystick.rs` by construction, which is the tautology
/// `docs/STATUS.md` records the keyboard matrix once being.
const RIGHT: u8 = 0x01;
const LEFT: u8 = 0x02;
const DOWN: u8 = 0x04;
const UP: u8 = 0x08;
const FIRE: u8 = 0x10;

/// Read the joystick port the way a game does, and report what landed in `A`.
fn read_joystick(machine: &mut Spectrum) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the port's low byte is what `IN A,(n)` encodes, and it is 0x1F"
    )]
    let low = KEMPSTON_PORT as u8;
    write_program(machine, PROGRAM, &[IN_A_N, low]);
    set_pc(machine, PROGRAM);
    machine.step();
    (machine.cpu_state().af >> 8) as u8
}

#[test]
fn a_guest_in_reaches_the_joystick_and_an_idle_stick_reads_zero() {
    // The wiring, and the assertion that would be green with the port arm missing entirely —
    // an undecoded port on this machine reads `0xFF`, which is every direction held at once.
    let mut machine = machine();
    assert_eq!(
        read_joystick(&mut machine),
        0x00,
        "an idle Kempston reads zero, not the floating bus"
    );
}

#[test]
fn each_switch_sets_the_bit_the_published_layout_gives_it() {
    // Literals against literals. A permutation of two directions is invisible to every other
    // test in this repository and sends a game the wrong way.
    for (direction, bit) in [
        (Direction::Right, RIGHT),
        (Direction::Left, LEFT),
        (Direction::Down, DOWN),
        (Direction::Up, UP),
        (Direction::Fire, FIRE),
    ] {
        let mut machine = machine();
        machine.joystick_mut().press(direction);
        assert_eq!(read_joystick(&mut machine), bit, "{direction:?}");
    }
}

#[test]
fn a_diagonal_with_fire_is_three_bits_at_once() {
    let mut machine = machine();
    for direction in [Direction::Up, Direction::Right, Direction::Fire] {
        machine.joystick_mut().press(direction);
    }
    assert_eq!(read_joystick(&mut machine), UP | RIGHT | FIRE);

    machine.joystick_mut().release(Direction::Fire);
    assert_eq!(read_joystick(&mut machine), UP | RIGHT);
    machine.joystick_mut().release_all();
    assert_eq!(read_joystick(&mut machine), 0x00);
}

#[test]
fn the_joystick_and_the_keyboard_cannot_collide() {
    // **The reason a frontend maps arrow keys to Kempston rather than to the membrane.** They
    // are different ports, so a game reading both sees each one unaffected by the other — and
    // a keyboard mapping would have to know which keys the game itself uses, which nothing
    // can know in general.
    let mut machine = machine();
    machine.joystick_mut().press(Direction::Up);
    machine.keyboard_mut().press(Key::Q);

    assert_eq!(read_joystick(&mut machine), UP, "the key did not reach it");
    // And the keyboard still reports its own key, through its own port. `0xFB` selects the
    // half-row `Q W E R T`, and a held key reads as a **zero** — the opposite convention.
    let membrane = machine.ula_mut().in_port(0xFBFE);
    assert_eq!(
        membrane & 0x01,
        0,
        "Q is held, and the membrane is active low"
    );
    assert_ne!(membrane, UP, "and it is not the joystick's byte");
}

#[test]
fn the_two_conventions_are_opposite_and_that_is_the_point() {
    // Stated as a single failing case because it is the classic defect: a Kempston modelled
    // active-low reads `0x1F` with nothing held, which every game reads as all four directions
    // and fire at once.
    let mut idle = machine();
    let idle_stick = read_joystick(&mut idle);
    let idle_keys = idle.ula_mut().in_port(0xFEFE);

    assert_eq!(idle_stick, 0x00, "an idle joystick is all zeros");
    assert_eq!(idle_keys & 0x1F, 0x1F, "an idle half-row is all ones");
    assert_ne!(
        idle_stick & 0x1F,
        idle_keys & 0x1F,
        "the two ports idle at opposite values, which is what makes the convention checkable"
    );
}

#[test]
fn the_joystick_answers_on_both_machines() {
    // It is an interface on the bus, not a feature of either ROM, so a 128 has it on the same
    // terms. A model that had hung it off the 48K's path would fail here and nowhere else.
    let editor = vec![0_u8; spectrum::memory::PAGE_SIZE];
    let mut one_two_eight = Spectrum::spectrum_128(&editor, &editor).expect("two page-sized ROMs");
    one_two_eight.joystick_mut().press(Direction::Left);
    assert_eq!(read_joystick(&mut one_two_eight), LEFT);
}

#[test]
fn a_reset_does_not_release_the_stick() {
    // Pressing reset does not let go of a joystick, for the same reason it does not lift the
    // keys or rewind a cassette — `Ula::reset` documents the rule and this is the joystick's
    // instance of it.
    let mut machine = machine();
    machine.joystick_mut().press(Direction::Fire);
    machine.reset();
    assert_eq!(read_joystick(&mut machine), FIRE);
}

#[test]
fn the_whole_family_answers_and_the_keyboards_own_ports_do_not() {
    // **The decode as a family, through a real `IN`.** `A0`-`A4` reach nothing on the board, so
    // every port from `0x00` to `0x1F` is the joystick's — and a model matching the canonical
    // address exactly would answer one of the thirty-two and float the rest.
    let mut machine = machine();
    machine.joystick_mut().press(Direction::Fire);
    for low in [0x00_u8, 0x01, 0x0F, 0x1E, 0x1F] {
        write_program(&mut machine, PROGRAM, &[IN_A_N, low]);
        set_pc(&mut machine, PROGRAM);
        machine.step();
        assert_eq!(
            (machine.cpu_state().af >> 8) as u8,
            FIRE,
            "port {low:#04X} is inside the window"
        );
    }
    // And one line outside it is not.
    for low in [0x20_u8, 0x3F, 0xDF, 0xFE] {
        write_program(&mut machine, PROGRAM, &[IN_A_N, low]);
        set_pc(&mut machine, PROGRAM);
        machine.step();
        assert_ne!(
            (machine.cpu_state().af >> 8) as u8,
            FIRE,
            "port {low:#04X} is outside it"
        );
    }
}

#[test]
fn where_the_joystick_and_the_ula_collide_the_joystick_answers() {
    // **A ruling, pinned so it is not silently reversed.** An *even* port below `0x20` is
    // claimed by both — the ULA decodes `A0` alone — and on hardware a fitted Kempston and the
    // ULA would both drive the bus. The narrower decode wins here, as it does for the AY.
    //
    // Nothing ordinary reaches it: every keyboard scan is `0xFE` with a high half, and
    // `0xFE`'s `A5` is set.
    let mut machine = machine();
    machine.joystick_mut().press(Direction::Down);
    machine.keyboard_mut().press(Key::Enter);

    assert_eq!(
        machine.ula_mut().in_port(0x001E),
        DOWN,
        "an even port inside the window is the joystick's"
    );
    // While the keyboard's own port, one address line away, is untouched.
    assert_ne!(
        machine.ula_mut().in_port(0xBFFE) & 0x01,
        0x01,
        "ENTER is held"
    );
}
