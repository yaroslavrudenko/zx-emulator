//! Gate: a half-row read reports the keys that are down — and only those, in the right place.
//!
//! # Why this exists
//!
//! Two separate mutations, both of which used to survive everything.
//!
//! **A keyboard reporting every key held** left the boot gate green. A positive probe
//! confirmed the ROM does read the keyboard during start-up, so the scan is executed; it is
//! simply not graded. The ROM's `KEY-SCAN` rejects a scan showing more than two keys down as
//! noise, so "everything pressed" and "nothing pressed" reach it as the same verdict — which
//! is also why the real ROM is *not* used as the oracle here: it cannot tell the two apart.
//!
//! **A permuted matrix** left the whole suite green. `keyboard.rs`'s own
//! `every_key_is_visible_to_a_scan_of_its_own_half_row` derives both the port it scans and the
//! value it expects from `Key::position()`, the function under test, so it proves `read()` and
//! `position()` agree and nothing more. Under a review that swapped half-rows, rotated six of
//! them, and rotated bits inside two more, **38 of the 40 keys moved** and the suite stayed at
//! 72 passed. The wiring was in fact correct; the evidence was not.
//!
//! Those are different failures — a gate that catches the first can still miss the second —
//! so both are covered, and the fixture the second needs is [`MEMBRANE`], written from the
//! published matrix with literal ports.
//!
//! # What is graded here
//!
//! The full cross product — **40 keys × 8 half-rows**, 320 reads against literal ports. For
//! each key: it reads low on its own bit of its own half-row, and every other bit of that row,
//! and every bit of the other seven rows, reads **released**. The second half is what the
//! "everything held" mutation dies on; the literal ports are what the permutation dies on.
//!
//! Also: the two absolute anchors, the bits the keyboard does not drive, the all-rows scan the
//! ROM uses to ask "is anything down at all", release, and the whole path through a real
//! `IN A,(0xFE)`.
//!
//! # What is not graded here
//!
//! **Key rollover and ghosting.** Reading several rows at once ANDs them, and that is
//! asserted; the membrane's real behaviour when two keys share a row and a column is not
//! modelled and so cannot be graded. Nothing else in this crate models it either — that is a
//! note about the model, not about this test.

mod common;

use common::{
    ALL_HALF_ROWS_PORT, ANCHORS, IDLE_HALF_ROW, MEMBRANE, UNCONTENDED_CODE, UNDRIVEN_BITS,
    half_row_keys, half_row_port, machine, read_port, set_pc, write_program,
};
use spectrum::Key;
use spectrum::keyboard::{HALF_ROWS, KEYS_PER_HALF_ROW};

/// The value bits 5–7 always hold: 5 and 7 float high, 6 is `EAR`, low with no tape.
const UNDRIVEN_VALUE: u8 = 0xA0;

/// Every key on the membrane, in table order.
fn all_keys() -> impl Iterator<Item = (usize, usize, Key)> {
    MEMBRANE
        .into_iter()
        .enumerate()
        .flat_map(|(row, (_, keys))| {
            keys.into_iter()
                .enumerate()
                .map(move |(bit, key)| (row, bit, key))
        })
}

#[test]
fn the_membrane_fixture_is_forty_distinct_keys_on_eight_distinct_ports() {
    // The fixture is the oracle the rest of this file grades against, so it is itself
    // checked. A duplicated key would make one key's assertions vacuously agree with
    // another's, and a port with two address lines low would select two half-rows at once and
    // quietly weaken every "reads released" assertion below.
    let mut seen: Vec<Key> = Vec::with_capacity(HALF_ROWS * KEYS_PER_HALF_ROW);
    for (_, _, key) in all_keys() {
        assert!(
            !seen.contains(&key),
            "{key:?} appears twice in the membrane"
        );
        seen.push(key);
    }
    assert_eq!(seen.len(), 40, "the 48K membrane has forty keys");

    let mut ports: Vec<u16> = MEMBRANE.iter().map(|(port, _)| *port).collect();
    for port in &ports {
        assert_eq!(port & 0x00FF, 0x00FE, "{port:#06X} does not select the ULA");
        assert_eq!(
            (port >> 8).count_zeros(),
            8 + 1,
            "{port:#06X} must hold exactly one of A8-A15 low"
        );
    }
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(ports.len(), HALF_ROWS, "two rows share a port");
}

#[test]
fn the_two_absolute_anchors_are_where_the_literature_puts_them() {
    // X at 0xFEFE bit 2 and ENTER at 0xBFFE bit 0 are the only two keys this project can
    // check against a concrete published (port, bit) pair rather than against a diagram it
    // transcribed itself. If the whole matrix were transcribed wrongly in one consistent
    // motion, these are the two points it could not slide past.
    for (key, port, bit) in ANCHORS {
        let mut machine = machine();
        machine.keyboard_mut().press(key);
        assert_eq!(
            read_port(&mut machine, port),
            IDLE_HALF_ROW & !bit,
            "{key:?} must read low on bit {bit:#04X} of port {port:#06X}"
        );
    }
}

#[test]
fn nothing_pressed_reads_every_half_row_as_released() {
    let mut machine = machine();

    for row in 0..HALF_ROWS {
        let port = half_row_port(row);
        assert_eq!(
            read_port(&mut machine, port),
            IDLE_HALF_ROW,
            "half-row {port:#06X} reports a key down with nothing pressed"
        );
    }
    assert_eq!(
        read_port(&mut machine, ALL_HALF_ROWS_PORT),
        IDLE_HALF_ROW,
        "the all-rows scan reports a key down with nothing pressed"
    );
}

#[test]
fn a_pressed_key_reads_low_on_its_own_bit_and_every_other_key_reads_released() {
    // The 320-read cross product, against literal ports. Two independent things die here: a
    // keyboard reporting everything held fails the `scan != row` branch, and a permuted matrix
    // fails the `scan == row` branch because the port it is scanned on is not derived from the
    // map under test.
    for (row, bit, key) in all_keys() {
        let mut machine = machine();
        machine.keyboard_mut().press(key);
        let pressed_bit = 1_u8 << bit;

        for scan in 0..HALF_ROWS {
            let port = half_row_port(scan);
            let observed = read_port(&mut machine, port);
            let expected = if scan == row {
                IDLE_HALF_ROW & !pressed_bit
            } else {
                IDLE_HALF_ROW
            };
            assert_eq!(
                observed,
                expected,
                "with only {key:?} down, port {port:#06X} read {observed:#04X} and should have \
                 read {expected:#04X} — the published matrix puts {key:?} on {:#06X} bit {bit}",
                half_row_port(row)
            );
        }
    }
}

#[test]
fn the_all_rows_scan_reports_any_key_down() {
    // `LD A,0; IN A,(0xFE)` — every address line low, so every half-row contributes and the
    // results are ANDed. This is how the ROM asks whether anything is pressed at all.
    for (row, bit, key) in all_keys() {
        let mut machine = machine();
        machine.keyboard_mut().press(key);
        assert_eq!(
            read_port(&mut machine, ALL_HALF_ROWS_PORT),
            IDLE_HALF_ROW & !(1_u8 << bit),
            "{key:?} (port {:#06X}, bit {bit}) was invisible to the all-rows scan",
            half_row_port(row)
        );
    }
}

#[test]
fn releasing_a_key_returns_its_half_row_to_released() {
    for (row, _, key) in all_keys() {
        let mut machine = machine();
        machine.keyboard_mut().press(key);
        machine.keyboard_mut().release(key);
        assert_eq!(
            read_port(&mut machine, half_row_port(row)),
            IDLE_HALF_ROW,
            "{key:?} still reads down after being released"
        );
    }
}

#[test]
fn the_bits_the_keyboard_does_not_drive_never_move() {
    // Bits 5 and 7 float high and bit 6 is the `EAR` input. They belong to the ULA, not the
    // membrane, and no combination of keys may disturb them.
    let mut machine = machine();
    for (_, _, key) in all_keys() {
        machine.keyboard_mut().press(key);
    }

    for row in 0..HALF_ROWS {
        let port = half_row_port(row);
        let observed = read_port(&mut machine, port);
        assert_eq!(
            observed & UNDRIVEN_BITS,
            UNDRIVEN_VALUE,
            "port {port:#06X} read {observed:#04X}: bits 5-7 are the ULA's, and bit 6 (EAR) \
             idles low with nothing in the tape socket"
        );
    }
}

#[test]
fn the_cpu_reads_the_selected_half_row_through_in_a_n() {
    // The whole path, one row at a time: `LD A,<row selector>` then `IN A,(0xFE)`, which puts
    // `A` on the high half of the port. Grades the decode, the port dispatch and the ULA's
    // undriven bits together — none of which the assertions above go through.
    const BIT: usize = 2;

    for row in 0..HALF_ROWS {
        let port = half_row_port(row);
        let key = half_row_keys(row)[BIT];
        let mut machine = machine();
        machine.keyboard_mut().press(key);

        let selector = (port >> 8) as u8;
        write_program(
            &mut machine,
            UNCONTENDED_CODE,
            &[0x3E, selector, 0xDB, 0xFE],
        );
        set_pc(&mut machine, UNCONTENDED_CODE);
        machine.step();
        machine.step();

        let accumulator = (machine.cpu_state().af >> 8) as u8;
        assert_eq!(
            accumulator,
            IDLE_HALF_ROW & !(1_u8 << BIT),
            "IN A,(0xFE) with A={selector:#04X} should report {key:?} down on port {port:#06X}"
        );
        assert_eq!(machine.fault(), None);
    }
}
