//! The keyboard map, against literals that owe the map nothing.
//!
//! # Why this file is written the way it is
//!
//! `docs/STATUS.md`'s *The keyboard matrix was graded against itself* records the defect this
//! file is built to avoid. A test derived **both** the port it scanned **and** the value it
//! expected from the function under test; it was exhaustive over a 40 × 8 cross product and
//! **38 of the 40 keys could be rewired with the entire suite green**, because both sides of
//! every comparison moved together. The two that could not move were the two whose
//! expectations were literals. The conclusion recorded there is the one this file applies:
//! *a test whose expectation is computed by the subject is not a weak test; it is a tautology
//! with a cross product attached.*
//!
//! So every expectation below is a literal. [`EXPECTED_MEMBRANE`] and [`EXPECTED_CHORDS`] are
//! transcribed by hand and are never derived from [`keymap::BINDINGS`]; the only thing taken
//! from the subject is the *question* — which binding does this host key have — exactly as
//! `crates/spectrum/tests/keyboard_matrix.rs` calls `read` and compares to a literal.
//!
//! # Four properties, of three different strengths, kept apart
//!
//! 1. **The membrane map is a bijection.** Structural, and true or false independently of
//!    anyone's taste — forgetting `V` or binding `Q` twice is a defect in any keymap. Proven
//!    two ways that fail for different reasons: `a_membrane_binding_reaches_the_key_its_row_names`
//!    catches a **permutation**, and `the_membrane_bindings_hold_down_every_key_on_the_membrane`
//!    catches an **omission** — and neither catches the other. The second one goes through
//!    `Keyboard::read`, so *"covers the membrane"* is measured against the hardware model
//!    rather than counted against a list.
//! 2. **No host key is bound twice, and no hotkey shadows a membrane key.** Structural. This
//!    is the one that catches `F2` also typing a letter.
//! 3. **The chords name what the Spectrum's own legend names.** This half has a referent
//!    outside this repository: `DELETE` really is printed above `0`.
//! 4. **The forty specific assignments have not changed.** A design decision pinned against
//!    drift. `docs/MACHINE.md` ranks this class fourth in its verification plan and is clear
//!    about what it is worth — *"does not prove correctness — proves change"*. It is not
//!    dressed up as more.
//!
//! What none of this can see is whether the mapping is *good*. That is observation, and the
//! run is in the report rather than asserted here.

use std::collections::BTreeSet;

use frontend::keymap::{self, Binding, Hotkey};
use macroquad::input::KeyCode;
use spectrum::keyboard::{HALF_ROWS, KEYS_PER_HALF_ROW, RELEASED};
use spectrum::{Key, Keyboard};

/// The forty host keys that own a membrane key, written out by hand.
///
/// Not derived from [`keymap::BINDINGS`] in any part. Permuting two rows of the table under
/// test turns `a_membrane_binding_reaches_the_key_its_row_names` red, and that was measured
/// rather than assumed — the run is in the report.
const EXPECTED_MEMBRANE: [(KeyCode, Key); 40] = [
    (KeyCode::LeftShift, Key::CapsShift),
    (KeyCode::LeftControl, Key::SymbolShift),
    (KeyCode::Enter, Key::Enter),
    (KeyCode::Space, Key::Space),
    (KeyCode::A, Key::A),
    (KeyCode::B, Key::B),
    (KeyCode::C, Key::C),
    (KeyCode::D, Key::D),
    (KeyCode::E, Key::E),
    (KeyCode::F, Key::F),
    (KeyCode::G, Key::G),
    (KeyCode::H, Key::H),
    (KeyCode::I, Key::I),
    (KeyCode::J, Key::J),
    (KeyCode::K, Key::K),
    (KeyCode::L, Key::L),
    (KeyCode::M, Key::M),
    (KeyCode::N, Key::N),
    (KeyCode::O, Key::O),
    (KeyCode::P, Key::P),
    (KeyCode::Q, Key::Q),
    (KeyCode::R, Key::R),
    (KeyCode::S, Key::S),
    (KeyCode::T, Key::T),
    (KeyCode::U, Key::U),
    (KeyCode::V, Key::V),
    (KeyCode::W, Key::W),
    (KeyCode::X, Key::X),
    (KeyCode::Y, Key::Y),
    (KeyCode::Z, Key::Z),
    (KeyCode::Key0, Key::Num0),
    (KeyCode::Key1, Key::Num1),
    (KeyCode::Key2, Key::Num2),
    (KeyCode::Key3, Key::Num3),
    (KeyCode::Key4, Key::Num4),
    (KeyCode::Key5, Key::Num5),
    (KeyCode::Key6, Key::Num6),
    (KeyCode::Key7, Key::Num7),
    (KeyCode::Key8, Key::Num8),
    (KeyCode::Key9, Key::Num9),
];

/// The extra host keys for the two shifts: the right-hand pair, and `Tab`.
///
/// > **`docs/M8.md` Decision 2 says adding `Tab` here is *"covered by gates that exist, without
/// > editing them"*, and that is wrong — this file had to be edited, and the edit is the
/// > gate working.** The claim was true of five of the seven tests in this file:
/// > `an_alias_targets_a_key_some_membrane_binding_already_owns` iterates this literal table,
/// > so a row nothing added here is simply not checked, and the bijection tests filter
/// > `Binding::Alias` out by construction. It is false of
/// > `the_table_holds_the_three_kinds_and_nothing_else`, which pins
/// > `BINDINGS.len()` against `EXPECTED_MEMBRANE + EXPECTED_ALIASES + EXPECTED_CHORDS` — so a
/// > row added to the subject and not to this table turns it **red**.
/// >
/// > That is exactly what that test was written for, in its own words: *"an assertion whose
/// > failure means 'I was not looking at the thing'"*. The design document reasoned about
/// > which tests would *pass* and missed the one whose whole job is to notice a table it has
/// > not been told about. **A gate that must be edited to accept a change is not a gate that
/// > failed to cover it**; it is the only kind that could have caught an alias added by
/// > accident, and the correction is recorded in `docs/M8.md` rather than absorbed by quietly
/// > bumping a number here.
const EXPECTED_ALIASES: [(KeyCode, Key); 3] = [
    (KeyCode::RightShift, Key::CapsShift),
    (KeyCode::RightControl, Key::SymbolShift),
    (KeyCode::Tab, Key::SymbolShift),
];

/// Every chord, as `(host key, shift, key)`.
///
/// Transcribed from the legends printed on a real 48K's keys, which is what makes this table
/// different in kind from the one above: `DELETE` above `0`, the arrows on `5`–`8`, `BREAK`
/// on `SPACE`, and the red symbols under the letters are facts about a physical object.
const EXPECTED_CHORDS: [(KeyCode, Key, Key); 14] = [
    (KeyCode::Backspace, Key::CapsShift, Key::Num0), // DELETE
    (KeyCode::Left, Key::CapsShift, Key::Num5),      // <-
    (KeyCode::Down, Key::CapsShift, Key::Num6),      // v
    (KeyCode::Up, Key::CapsShift, Key::Num7),        // ^
    (KeyCode::Right, Key::CapsShift, Key::Num8),     // ->
    (KeyCode::CapsLock, Key::CapsShift, Key::Num2),  // CAPS LOCK
    (KeyCode::Escape, Key::CapsShift, Key::Space),   // BREAK
    (KeyCode::Comma, Key::SymbolShift, Key::N),      // ,
    (KeyCode::Period, Key::SymbolShift, Key::M),     // .
    (KeyCode::Semicolon, Key::SymbolShift, Key::O),  // ;
    (KeyCode::Apostrophe, Key::SymbolShift, Key::Num7), // '
    (KeyCode::Minus, Key::SymbolShift, Key::J),      // -
    (KeyCode::Equal, Key::SymbolShift, Key::L),      // =
    (KeyCode::Slash, Key::SymbolShift, Key::V),      // /
];

/// Every hotkey, written out by hand.
const EXPECTED_HOTKEYS: [(KeyCode, Hotkey); 7] = [
    (KeyCode::F1, Hotkey::ToggleStatus),
    (KeyCode::F2, Hotkey::SaveSnapshot),
    (KeyCode::F3, Hotkey::PlayTape),
    (KeyCode::F4, Hotkey::StopTape),
    (KeyCode::F5, Hotkey::RewindTape),
    (KeyCode::F6, Hotkey::Reset),
    // The arrow keys are a *choice* — the Spectrum has none, and games disagree about what to
    // read — so the choice needs a key. `docs/M8.md` Decision 13, and `keymap::ArrowScheme`.
    (KeyCode::F7, Hotkey::CycleArrows),
];

/// What the table under test says about `code`.
///
/// The *question* comes from the subject; every *answer* compared against it is a literal.
fn binding_for(code: KeyCode) -> Option<Binding> {
    keymap::BINDINGS
        .iter()
        .find(|&&(bound, _)| bound == code)
        .map(|&(_, binding)| binding)
}

/// A port that selects exactly one half-row, holding that address line low.
fn select(half_row: usize) -> u16 {
    let high = !(1_u16 << half_row) & 0xFF;
    (high << 8) | 0x00FE
}

// ---------------------------------------------------------------------------------------
// 1. The bijection — caught two ways, for two different failures
// ---------------------------------------------------------------------------------------

#[test]
fn a_membrane_binding_reaches_the_key_its_row_names() {
    // This is the one that catches a **permutation**. `the_membrane_bindings_hold_down_every_key`
    // below cannot: swapping two rows still presses all forty keys.
    for (code, key) in EXPECTED_MEMBRANE {
        assert_eq!(
            binding_for(code),
            Some(Binding::Membrane(key)),
            "{code:?} should own {key:?} on the membrane",
        );
    }
}

#[test]
fn the_membrane_bindings_hold_down_every_key_on_the_membrane() {
    // This is the one that catches an **omission**, and it does it through the hardware model
    // rather than by counting a list: with all forty host keys held, every one of the eight
    // half-rows must read all five bits low. A membrane key nothing binds leaves its bit
    // high, and `Keyboard::read` is graded against literals next door.
    let codes: Vec<KeyCode> = EXPECTED_MEMBRANE.iter().map(|&(code, _)| code).collect();
    let mut keyboard = Keyboard::new();
    keymap::apply(|code| codes.contains(&code), &mut keyboard);

    for half_row in 0..HALF_ROWS {
        assert_eq!(
            keyboard.read(select(half_row)),
            0x00,
            "half-row {half_row} has a key no host key holds down",
        );
    }
}

#[test]
fn the_membrane_map_is_a_bijection() {
    let membrane: Vec<Key> = keymap::BINDINGS
        .iter()
        .filter_map(|&(_, binding)| match binding {
            Binding::Membrane(key) => Some(key),
            Binding::Alias(_) | Binding::Chord { .. } => None,
        })
        .collect();

    // The forty is the membrane's own shape rather than a number written here: eight
    // half-rows of five keys is what `crates/spectrum/src/keyboard.rs` says the hardware is.
    assert_eq!(
        membrane.len(),
        HALF_ROWS * KEYS_PER_HALF_ROW,
        "the membrane has {} keys and {} bindings claim one",
        HALF_ROWS * KEYS_PER_HALF_ROW,
        membrane.len(),
    );
    let distinct: BTreeSet<Key> = membrane.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        membrane.len(),
        "two host keys claim to own the same membrane key",
    );
    assert_eq!(
        EXPECTED_MEMBRANE.len(),
        HALF_ROWS * KEYS_PER_HALF_ROW,
        "this test's own literal table has drifted from the membrane's shape",
    );
}

// ---------------------------------------------------------------------------------------
// 2. Nothing is bound twice, and nothing is shadowed
// ---------------------------------------------------------------------------------------

#[test]
fn no_host_key_is_bound_twice() {
    let mut seen = BTreeSet::new();
    for &(code, binding) in keymap::BINDINGS {
        assert!(
            seen.insert(code as u32),
            "{code:?} appears twice; the second entry ({binding:?}) is unreachable",
        );
    }
    assert_eq!(seen.len(), keymap::BINDINGS.len());
}

#[test]
fn no_hotkey_shadows_a_bound_key() {
    // The defect this exists for is a hotkey that also types something — saving a snapshot
    // mid-sentence and finding a stray letter in the listing.
    for &(hotkey_code, action) in keymap::HOTKEYS {
        assert_eq!(
            binding_for(hotkey_code),
            None,
            "{hotkey_code:?} is both {action:?} and a key on the machine",
        );
    }
}

#[test]
fn the_hotkeys_are_the_ones_written_down() {
    assert_eq!(keymap::HOTKEYS.len(), EXPECTED_HOTKEYS.len());
    for (code, action) in EXPECTED_HOTKEYS {
        assert!(
            keymap::HOTKEYS.contains(&(code, action)),
            "{code:?} should be {action:?}",
        );
    }
}

// ---------------------------------------------------------------------------------------
// 3. The aliases and the chords
// ---------------------------------------------------------------------------------------

#[test]
fn an_alias_targets_a_key_some_membrane_binding_already_owns() {
    // An alias whose target nothing owns would be a membrane key with two host keys and no
    // primary, which is the bijection quietly broken through the back door.
    let owned: BTreeSet<Key> = EXPECTED_MEMBRANE.iter().map(|&(_, key)| key).collect();
    for (code, key) in EXPECTED_ALIASES {
        assert_eq!(binding_for(code), Some(Binding::Alias(key)));
        assert!(owned.contains(&key), "{key:?} has an alias but no owner");
    }
}

#[test]
fn every_chord_names_the_combination_the_spectrum_prints_on_the_key() {
    for (code, shift, key) in EXPECTED_CHORDS {
        assert_eq!(
            binding_for(code),
            Some(Binding::Chord { shift, key }),
            "{code:?} should be {shift:?} + {key:?}",
        );
    }
}

#[test]
fn every_chord_is_held_under_one_of_the_two_shifts() {
    // A chord under anything but a shift would be a combination the machine cannot produce,
    // and it would look perfectly reasonable in the table.
    for &(code, binding) in keymap::BINDINGS {
        if let Binding::Chord { shift, .. } = binding {
            assert!(
                shift == Key::CapsShift || shift == Key::SymbolShift,
                "{code:?} is held under {shift:?}, which is not a shift",
            );
        }
    }
}

#[test]
fn the_table_holds_the_three_kinds_and_nothing_else() {
    // A positive control in the sense `docs/STATUS.md` means: an assertion whose failure says
    // "I was not looking at the thing". If `BINDINGS` were emptied or truncated, every
    // `contains`-shaped assertion above would still have something to say, but this one
    // pins the totals, so the tables and the subject cannot silently diverge in size.
    let expected = EXPECTED_MEMBRANE.len() + EXPECTED_ALIASES.len() + EXPECTED_CHORDS.len();
    assert_eq!(
        keymap::BINDINGS.len(),
        expected,
        "the table under test has {} entries and this file accounts for {expected}",
        keymap::BINDINGS.len(),
    );
}

// ---------------------------------------------------------------------------------------
// 4. Behaviour, through the seam `apply` exists for
// ---------------------------------------------------------------------------------------

#[test]
fn holding_a_chord_key_presses_both_of_its_membrane_keys_and_no_others() {
    let mut keyboard = Keyboard::new();
    keymap::apply(|code| code == KeyCode::Comma, &mut keyboard);

    assert!(
        keyboard.is_pressed(Key::SymbolShift),
        "SYMBOL SHIFT is down"
    );
    assert!(keyboard.is_pressed(Key::N), "N is down");

    // Literal, and the half-row is the hardware's: SPACE, SYMBOL SHIFT, M, N, B are A15's
    // five bits in that order, so SYMBOL SHIFT is 0x02 and N is 0x08.
    assert_eq!(keyboard.read(select(7)), RELEASED & !0x02 & !0x08);
    for other in [0, 1, 2, 3, 4, 5, 6] {
        assert_eq!(
            keyboard.read(select(other)),
            RELEASED,
            "half-row {other} should be untouched by a comma",
        );
    }
}

#[test]
fn releasing_the_host_key_releases_the_whole_chord() {
    // The reason `apply` rebuilds rather than tracks edges: a chord that half-releases leaves
    // a shift stuck down, and a stuck CAPS SHIFT is indistinguishable from a broken machine.
    let mut keyboard = Keyboard::new();
    keymap::apply(|code| code == KeyCode::Comma, &mut keyboard);
    keymap::apply(|_| false, &mut keyboard);

    assert!(!keyboard.is_pressed(Key::SymbolShift));
    assert!(!keyboard.is_pressed(Key::N));
    for half_row in 0..HALF_ROWS {
        assert_eq!(keyboard.read(select(half_row)), RELEASED);
    }
}

#[test]
fn either_shift_key_reaches_the_same_membrane_key() {
    for code in [KeyCode::LeftShift, KeyCode::RightShift] {
        let mut keyboard = Keyboard::new();
        keymap::apply(|pressed| pressed == code, &mut keyboard);
        assert!(
            keyboard.is_pressed(Key::CapsShift),
            "{code:?} should reach CAPS SHIFT",
        );
        // CAPS SHIFT is A8 bit 0 — a literal from the hardware, not from the map.
        assert_eq!(keyboard.read(select(0)), RELEASED & !0x01);
    }
}
