//! What a game asks of the keymap that BASIC's editor never does.
//!
//! # Why the existing gates do not already cover this
//!
//! `tests/keymap_table.rs` is thorough about the *map*: forty host keys onto forty membrane
//! keys, a bijection proven two independent ways, every chord against the legend printed on a
//! real Spectrum. `crates/spectrum/tests/keyboard_matrix.rs` is thorough about the *membrane*:
//! the full 40 × 8 cross product against literal ports and bits.
//!
//! **Every one of those grades the map against a single keypress, and a game is a different
//! consumer.** It holds a direction and a jump together and expects both to read low in the
//! same scan; it reads the same port many times inside one frame; and it treats a key as a
//! *level* rather than an event. None of those is a claim about which key is which, so none of
//! them is caught by a table that is right about which key is which.
//!
//! # What was found already covered, and is therefore not duplicated here
//!
//! **A read that selects several half-rows at once must AND them.** That is a game asking *"is
//! any key down at all"* with `LD A,0; IN A,(0xFE)`, and it is already asserted next door:
//! `crates/spectrum/tests/keyboard_matrix.rs`'s `the_all_rows_scan_reports_any_key_down`, whose
//! own comment names the instruction sequence. That file's header also records the AND
//! explicitly — *"Reading several rows at once ANDs them, and that is asserted"* — so this file
//! cites it and stops, rather than adding a second copy of somebody else's gate.
//!
//! # What is still not covered by anything, stated so nobody reads a green as wider than it is
//!
//! **Ghosting and rollover are not modelled.** A real membrane has crosstalk when several keys
//! share a half-row and a column, and `crates/spectrum/tests/keyboard_matrix.rs` says so in its
//! own words: *"the membrane's real behaviour when two keys share a row and a column is not"*
//! modelled. A game that needs such a pair and misbehaves is meeting a **recorded gap**, not a
//! regression. Nothing in this file changes that, and the assertions below are deliberately
//! about keys in *different* half-rows, which is where the modelled behaviour is defined.
//!
//! **Whether a game responds at all.** No commercial game has ever run on this emulator. These
//! gates say the keymap delivers what a game would read; they cannot say a game reads it.

use std::collections::BTreeSet;

use frontend::keymap;
use macroquad::input::KeyCode;
use spectrum::joystick::Direction;
use spectrum::keyboard::{HALF_ROWS, RELEASED};
use spectrum::{Key, Keyboard};

/// Every membrane key, for asking a `Keyboard` what a binding actually pressed.
///
/// Written out rather than iterated, because `Key` has no iterator and adding one would be a
/// public item in a crate this file does not own.
const ALL_KEYS: [Key; 40] = [
    Key::CapsShift,
    Key::Z,
    Key::X,
    Key::C,
    Key::V,
    Key::A,
    Key::S,
    Key::D,
    Key::F,
    Key::G,
    Key::Q,
    Key::W,
    Key::E,
    Key::R,
    Key::T,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num0,
    Key::Num9,
    Key::Num8,
    Key::Num7,
    Key::Num6,
    Key::P,
    Key::O,
    Key::I,
    Key::U,
    Key::Y,
    Key::Enter,
    Key::L,
    Key::K,
    Key::J,
    Key::H,
    Key::Space,
    Key::SymbolShift,
    Key::M,
    Key::N,
    Key::B,
];

/// A port that selects exactly one half-row, holding that address line low.
fn select(half_row: usize) -> u16 {
    let high = !(1_u16 << half_row) & 0xFF;
    (high << 8) | 0x00FE
}

/// The port a game uses to ask *"is any key at all down"* — every address line low, so every
/// half-row contributes and the ULA ANDs them.
const ALL_ROWS: u16 = 0x00FE;

// The two literals this file compares against, taken from the hardware and not from the map.
// Half-row 5 is `P O I U Y` on address line A13, so `O` is bit 1. Half-row 7 is
// `SPACE SYMBOL-SHIFT M N B` on A15, so `SPACE` is bit 0. They are in different half-rows on
// purpose: that is the case a direction-plus-jump actually is, and it is the case the modelled
// membrane defines.

/// `O` — left in Manic Miner, and in most of its generation.
const O_HALF_ROW: usize = 5;
/// `O` is the second of `P O I U Y`.
const O_BIT: u8 = 0x02;

/// `SPACE` — jump.
const SPACE_HALF_ROW: usize = 7;
/// `SPACE` is the first of `SPACE SYMBOL-SHIFT M N B`.
const SPACE_BIT: u8 = 0x01;

#[test]
fn two_keys_held_together_both_read_low_in_the_same_scan() {
    // The defect this exists for would look like "the game ignores jump while I am moving", and
    // no single-key assertion anywhere can reach it: every one of them holds one key.
    let mut keyboard = Keyboard::new();
    keymap::apply(
        |code| code == KeyCode::O || code == KeyCode::Space,
        &mut keyboard,
    );

    assert_eq!(
        keyboard.read(select(O_HALF_ROW)),
        RELEASED & !O_BIT,
        "O should be the only key down in its half-row",
    );
    assert_eq!(
        keyboard.read(select(SPACE_HALF_ROW)),
        RELEASED & !SPACE_BIT,
        "SPACE should be the only key down in its half-row",
    );

    // And nothing else came down with them. A map that pressed a neighbour would pass both
    // assertions above and fail this one.
    for half_row in 0..HALF_ROWS {
        if half_row == O_HALF_ROW || half_row == SPACE_HALF_ROW {
            continue;
        }
        assert_eq!(
            keyboard.read(select(half_row)),
            RELEASED,
            "half-row {half_row} should be untouched by O and SPACE",
        );
    }

    // The way a game actually asks: one read, every row selected. Both bits must be low at once,
    // which is a strictly stronger statement than the two single-row reads above — those could
    // both pass on a keyboard that forgot one key between calls.
    assert_eq!(
        keyboard.read(ALL_ROWS),
        RELEASED & !O_BIT & !SPACE_BIT,
        "an all-rows scan should see both keys down together",
    );
}

#[test]
fn a_held_key_is_a_level_and_not_an_event() {
    // A game reads a key as a state, every frame, for as long as it is held. If `apply` were
    // edge-triggered — or if `release_all` ran after the presses rather than before them — a
    // held key would register once and then vanish, and the symptom would be a character that
    // takes one step per press instead of walking.
    let mut keyboard = Keyboard::new();
    let held = |code: KeyCode| code == KeyCode::O;

    for frame in 0..10 {
        keymap::apply(held, &mut keyboard);
        assert_eq!(
            keyboard.read(select(O_HALF_ROW)),
            RELEASED & !O_BIT,
            "O stopped reading as held on frame {frame}",
        );
    }

    // And it goes up exactly when the host says so, not a frame later.
    keymap::apply(|_| false, &mut keyboard);
    assert_eq!(keyboard.read(select(O_HALF_ROW)), RELEASED);
}

#[test]
fn a_port_read_many_times_within_one_frame_answers_the_same_every_time() {
    // A game polls in a tight loop, several half-rows per frame and often the same one twice.
    // `keymap::apply` runs once per frame and `Keyboard::read` must be a pure query of the
    // state it left — anything that mutated on read, or debounced, would make a game's polling
    // loop see a key flicker. Nothing here has ever been driven by a consumer that reads faster
    // than a person types.
    let mut keyboard = Keyboard::new();
    keymap::apply(
        |code| code == KeyCode::O || code == KeyCode::Space,
        &mut keyboard,
    );

    let first = keyboard.read(ALL_ROWS);
    for _ in 0..1000 {
        assert_eq!(
            keyboard.read(ALL_ROWS),
            first,
            "the same port answered differently within one frame",
        );
    }
    // Not a tautology against a constant: the value being repeated is the one both keys make.
    assert_eq!(first, RELEASED & !O_BIT & !SPACE_BIT);
}

#[test]
fn the_two_shift_aliases_do_not_leak_into_a_games_keys() {
    // `Tab` became a third alias for SYMBOL SHIFT at M8, for a browser's sake. SYMBOL SHIFT is
    // half-row 7 bit 1 — the *same half-row as SPACE*, which is the jump key in half the games
    // ever written. So the alias lands one bit away from the most-pressed key on the machine,
    // and this asserts the two do not touch each other.
    let mut keyboard = Keyboard::new();
    keymap::apply(|code| code == KeyCode::Space, &mut keyboard);
    assert_eq!(
        keyboard.read(select(SPACE_HALF_ROW)),
        RELEASED & !SPACE_BIT,
        "SPACE alone should not bring SYMBOL SHIFT down with it",
    );

    let mut keyboard = Keyboard::new();
    keymap::apply(|code| code == KeyCode::Tab, &mut keyboard);
    assert!(keyboard.is_pressed(Key::SymbolShift), "Tab is SYMBOL SHIFT");
    assert!(
        !keyboard.is_pressed(Key::Space),
        "Tab must not press SPACE, which shares its half-row",
    );
}

// ---------------------------------------------------------------------------------------
// The arrow schemes, against a real game's own read
// ---------------------------------------------------------------------------------------

/// Manic Miner's jump check, decoded from its own code at `0x8C58`:
///
/// ```text
/// 8C58  LD BC,7EFEh  IN A,(C)  AND 1Fh  CP 1Fh  JR NZ,jump
/// ```
///
/// `B = 0x7E` pulls **A8 and A15 low at the same time**, so the read merges the `CAPS SHIFT`
/// half-row with the `SPACE` one and jumps if *anything* in either is down. That is the whole
/// reason `ARROW_SCHEMES` has both a `cursor` and a `5678` entry, and this constant is the
/// literal from the game rather than a number chosen here.
const MANIC_MINER_JUMP_PORT: u16 = 0x7EFE;

/// The scheme with `name`, or a panic naming what is there instead.
fn scheme(name: &str) -> &'static keymap::ArrowScheme {
    keymap::ARROW_SCHEMES
        .iter()
        .find(|candidate| candidate.name == name)
        .unwrap_or_else(|| {
            panic!(
                "no arrow scheme called {name:?}; there are {:?}",
                keymap::ARROW_SCHEMES
                    .iter()
                    .map(|s| s.name)
                    .collect::<Vec<_>>()
            )
        })
}

#[test]
fn the_game_scheme_does_not_press_caps_shift_and_the_cursor_scheme_does() {
    // **This is the defect the disassembly found, asserted.** Holding left under the `cursor`
    // scheme presses `CAPS SHIFT` as well as `5` — correct for BASIC, where the arrows *are*
    // that chord — and Manic Miner's merged-row read then sees a key down and jumps. A person
    // would see Willy walk left while jumping continuously and conclude the emulator was broken.
    let mut with_chord = Keyboard::new();
    let _ = keymap::apply_with(
        |code| code == KeyCode::Left,
        &mut with_chord,
        scheme("cursor (BASIC)"),
    );
    assert!(
        with_chord.is_pressed(Key::CapsShift),
        "the cursor scheme is the CAPS SHIFT chord and must stay that way for BASIC",
    );
    assert_ne!(
        with_chord.read(MANIC_MINER_JUMP_PORT) & 0x1F,
        0x1F,
        "the cursor scheme trips Manic Miner's jump read — which is why the default is not it",
    );

    let mut bare = Keyboard::new();
    let _ = keymap::apply_with(
        |code| code == KeyCode::Left,
        &mut bare,
        scheme("5678 + Kempston"),
    );
    assert!(
        !bare.is_pressed(Key::CapsShift),
        "the game scheme must press the bare digit, or it trips the jump read",
    );
    assert!(bare.is_pressed(Key::Num5), "left is key 5");
    assert_eq!(
        bare.read(MANIC_MINER_JUMP_PORT) & 0x1F,
        0x1F,
        "walking left under the game scheme must not read as a jump",
    );
}

#[test]
fn the_default_scheme_never_trips_the_merged_row_jump_read() {
    // **The gate that was missing, and its absence is the whole defect.**
    //
    // `the_game_scheme_does_not_press_caps_shift_and_the_cursor_scheme_does` above has asserted
    // since M8 that the cursor chord trips Manic Miner's jump read. It was green the entire time
    // the emulator shipped that chord *as the default*, because it grades a scheme **looked up by
    // name** and nothing graded the one the window actually starts on. A predicted hazard, gated,
    // and shipped anyway — so this asserts the property of `ARROW_SCHEMES[0]` itself, by index,
    // which is the thing `main.rs` reads.
    //
    // The owner's report was "whatever key I press, he jumps".
    for arrow in [KeyCode::Left, KeyCode::Down, KeyCode::Up, KeyCode::Right] {
        let mut keyboard = Keyboard::new();
        let _ = keymap::apply_with(
            |code| code == arrow,
            &mut keyboard,
            &keymap::ARROW_SCHEMES[0],
        );

        assert!(
            !keyboard.is_pressed(Key::CapsShift),
            "the default scheme presses CAPS SHIFT for {arrow:?}",
        );
        // The read the game actually performs, not a proxy for it: `B = 0x7E` pulls A8 and A15
        // low together, so this one port covers both half-rows the jump check merges.
        assert_eq!(
            keyboard.read(MANIC_MINER_JUMP_PORT) & 0x1F,
            0x1F,
            "the default scheme makes {arrow:?} read as a jump on Manic Miner",
        );
    }
}

#[test]
fn the_default_scheme_reaches_the_membrane_and_the_port_from_one_press() {
    // The default sends the bare digits **and** the Kempston port, because a game reads one or
    // the other and nothing can know which — three of the six games disassembled read no fixed
    // keys at all. This asserts both halves arrive and that each stays in its own device: the
    // whole safety argument for combining them is that a port cannot collide with a key.
    let mut keyboard = Keyboard::new();
    let joystick = keymap::apply_with(
        |code| code == KeyCode::Right,
        &mut keyboard,
        &keymap::ARROW_SCHEMES[0],
    );

    assert!(
        keyboard.is_pressed(Key::Num8),
        "right should reach the membrane as the bare digit 8",
    );
    assert!(
        joystick.is_pressed(Direction::Right),
        "right should reach the Kempston port as well",
    );

    // And the port half touched nothing else on the membrane. A `Both` that leaked into a third
    // key would look exactly like a working scheme until some game read that key.
    for half_row in 0..HALF_ROWS {
        let expected = if half_row == 4 {
            RELEASED & !0x04
        } else {
            RELEASED
        };
        assert_eq!(
            keyboard.read(select(half_row)),
            expected,
            "half-row {half_row} should carry only the bare 8",
        );
    }
}

#[test]
fn apply_still_means_the_bindings_table_whatever_the_default_becomes() {
    // **`apply` is not the default and must never be pointed at it again.** `zx-shot` types BASIC
    // through this function, and `tests/keymap_table.rs` grades `BINDINGS` through it — so it has
    // to keep meaning *the printed legend*, permanently. It used to delegate to
    // `ARROW_SCHEMES[0]`, which made "what the table says" and "what the window starts on" the
    // same decision; moving the default then silently changed a binary this crate does not own.
    //
    // The expectations are the literals from the machine's own keys — the arrows printed on
    // `5`-`8`, held under `CAPS SHIFT` — and are not read from any scheme.
    for (arrow, digit) in [
        (KeyCode::Left, Key::Num5),
        (KeyCode::Down, Key::Num6),
        (KeyCode::Up, Key::Num7),
        (KeyCode::Right, Key::Num8),
    ] {
        let mut keyboard = Keyboard::new();
        keymap::apply(|code| code == arrow, &mut keyboard);
        assert!(
            keyboard.is_pressed(Key::CapsShift),
            "{arrow:?} through `apply` must still be the CAPS SHIFT chord the legend prints",
        );
        assert!(
            keyboard.is_pressed(digit),
            "{arrow:?} through `apply` must still press {digit:?}",
        );
    }
}

#[test]
fn every_scheme_binds_four_distinct_membrane_keys() {
    // A scheme whose left and right land on the same membrane key would make a game see both
    // directions at once, which reads as a stuck sprite rather than as a mapping error. And
    // Manic Miner is the case that makes this non-theoretical: its left group is `Q E T U O`
    // and its right group `W R Y I P`, *interleaved* across the top row — so a scheme built by
    // taking "the left half of the row" would press one of each.
    const ARROWS: [KeyCode; 4] = [KeyCode::Left, KeyCode::Down, KeyCode::Up, KeyCode::Right];
    for scheme in keymap::ARROW_SCHEMES {
        let mut seen = BTreeSet::new();
        for (index, arrow) in ARROWS.into_iter().enumerate() {
            // Through `apply_with`, which is the path the window uses. `Binding::press` is
            // private and should stay that way: a test reaching past the public seam would be
            // grading a function the shell never calls.
            let mut keyboard = Keyboard::new();
            let joystick = keymap::apply_with(|code| code == arrow, &mut keyboard, scheme);

            // A Kempston scheme touches no membrane key at all — that is the point of it, and
            // it is why the identity of a direction is *whatever the machine ended up with*
            // rather than a list of keys. Reading both devices keeps one loop over both kinds
            // and stops a scheme that presses nothing anywhere from passing quietly.
            let mut pressed: Vec<String> = ALL_KEYS
                .iter()
                .copied()
                .filter(|&key| keyboard.is_pressed(key))
                .map(|key| format!("{key:?}"))
                .collect();
            pressed.extend(
                Direction::ALL
                    .into_iter()
                    .filter(|&direction| joystick.is_pressed(direction))
                    .map(|direction| format!("joystick {direction:?}")),
            );

            assert!(
                !pressed.is_empty(),
                "{}'s direction {index} presses nothing at all",
                scheme.name,
            );
            assert!(
                seen.insert(pressed.clone()),
                "{}'s direction {index} presses {pressed:?}, which another direction also does",
                scheme.name,
            );
        }
    }
    // The "I was not looking at the thing" assertion: an empty table would satisfy the loop.
    assert!(
        keymap::ARROW_SCHEMES.len() >= 2,
        "there is nothing to cycle"
    );
}

#[test]
fn the_cursor_scheme_reproduces_the_table() {
    // `apply` is the `BINDINGS` rows and the cursor scheme is the same four chords, so the two
    // must agree — or `F7`ing to `cursor (BASIC)` would give a *different* editor from the one
    // every gate in `tests/keymap_table.rs` grades.
    //
    // > This compared `apply` against `ARROW_SCHEMES[0]` until the default moved off the cursor
    // > chord, at which point the comparison became a tautology in the wrong direction: it would
    // > have gone red for the *correct* change and stayed green for a cursor scheme that drifted
    // > away from the table. It is pinned to the cursor scheme by name now, which is the thing
    // > the claim was always about.
    for code in [KeyCode::Left, KeyCode::Down, KeyCode::Up, KeyCode::Right] {
        let mut through_apply = Keyboard::new();
        keymap::apply(|pressed| pressed == code, &mut through_apply);

        let mut through_scheme = Keyboard::new();
        let _ = keymap::apply_with(
            |pressed| pressed == code,
            &mut through_scheme,
            scheme("cursor (BASIC)"),
        );

        for half_row in 0..HALF_ROWS {
            assert_eq!(
                through_apply.read(select(half_row)),
                through_scheme.read(select(half_row)),
                "{code:?} differs between `apply` and the cursor scheme on half-row {half_row}",
            );
        }
    }
}

#[test]
fn the_kempston_scheme_touches_no_membrane_key_and_releases_on_focus_loss() {
    // **The whole reason a joystick scheme is worth having**: it is a port, not the membrane, so
    // a game reading the keyboard cannot see it and a game reading the joystick cannot be
    // confused by a keypress. Every other scheme has to be chosen against what a particular
    // game reads; this one collides with nothing.
    let kempston = scheme("Kempston only");

    let mut keyboard = Keyboard::new();
    let joystick = keymap::apply_with(|code| code == KeyCode::Up, &mut keyboard, kempston);

    assert!(
        joystick.is_pressed(Direction::Up),
        "up should reach the port"
    );
    for half_row in 0..HALF_ROWS {
        assert_eq!(
            keyboard.read(select(half_row)),
            RELEASED,
            "half-row {half_row} moved; a joystick must not touch the membrane",
        );
    }

    // Focus loss. Kempston is **active high with no interlock**, so a direction that is not
    // rebuilt stays pressed forever — the exact defect the per-frame keyboard rebuild was
    // designed against, in the device where it is easier to get wrong.
    let released = keymap::apply_with(|_| false, &mut keyboard, kempston);
    for direction in Direction::ALL {
        assert!(
            !released.is_pressed(direction),
            "{direction:?} survived a frame with nothing held",
        );
    }

    // And the opposite directions really can be held together, because the hardware has no
    // interlock and this must not invent one.
    let both = keymap::apply_with(
        |code| code == KeyCode::Left || code == KeyCode::Right,
        &mut keyboard,
        kempston,
    );
    assert!(both.is_pressed(Direction::Left) && both.is_pressed(Direction::Right));
}
