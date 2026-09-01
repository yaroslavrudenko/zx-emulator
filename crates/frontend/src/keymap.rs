//! A modern keyboard onto the 40-key membrane.
//!
//! # This is a design problem, not a lookup
//!
//! The membrane has forty keys and a PC keyboard has about a hundred, so the obvious move is
//! a forty-row table and a shrug at the rest. That table is the prettiest one and it is not
//! the useful one, because the sets are not nested — each keyboard has keys the other does
//! not, in both directions:
//!
//! - The Spectrum has **`SYMBOL SHIFT`**, which nothing on a PC keyboard is.
//! - The Spectrum has **no** `Backspace`, no arrow keys, no `Escape`, and no punctuation
//!   keys at all. All of those are *shifted* combinations, printed on the keys in red and
//!   green, and a person who reaches for `Backspace` and gets nothing concludes the emulator
//!   is broken rather than that Sinclair was saving money on keys.
//! - A PC keyboard has two `Shift`s and two `Ctrl`s, and a person uses whichever hand is
//!   free.
//!
//! So the map is not `host key -> membrane key`. It is `host key -> what the membrane needs
//! held down`, and that is what [`Binding`] carries.
//!
//! # The rule that decides which chords exist
//!
//! **A chord exists exactly where the PC key's own printed meaning matches the Spectrum
//! legend's meaning.** `Backspace` deletes and `CAPS SHIFT`+`0` is `DELETE`, so that chord
//! exists. `,` is `,` and `SYMBOL SHIFT`+`N` is `,`, so that one exists.
//!
//! The rule earns its place by what it *excludes*. `CAPS SHIFT`+`1` is `EDIT`, and there is
//! no PC key that means "edit" — binding `Tab` to it would be an invention rather than a
//! translation, and inventions are what make a keymap feel arbitrary. `SYMBOL SHIFT`+`(` is
//! reachable, but `(` is a *shifted* key on a PC, and binding an unshifted host key to it
//! would put the two keyboards' shift states into disagreement. Both are left out. They
//! remain typeable the way the hardware types them, by holding the mapped `CAPS SHIFT` or
//! `SYMBOL SHIFT` and pressing the digit.
//!
//! ## That last sentence is a fallback, and a browser destroys it
//!
//! **A fallback is only a fallback on the platform it was reasoned about**, and this one was
//! reasoned about on a desktop. On a Spectrum `(` is `SYMBOL SHIFT`+`8` and `)` is
//! `SYMBOL SHIFT`+`9`. With `Ctrl` mapped to `SYMBOL SHIFT`, those are `Ctrl+8` and `Ctrl+9`,
//! which **Chrome uses to switch browser tabs and does not offer to the page at all**. Not
//! "offers and then overrides" — the second class of browser shortcut is consumed by the
//! chrome, so `preventDefault` has nothing to cancel. See [`crate::host`] for the two classes.
//!
//! So the design is not inconvenienced at its edges: **it is broken at the exact point where it
//! chose to lean on the hardware**, and it is broken for the parentheses, which no BASIC
//! program avoids. And the failure is silent — a tab switches and the emulator looks like it
//! ignored a keystroke.
//!
//! **The general shape, which is why this is written here rather than in a browser note.** An
//! excluded case is normally free: you exclude it *because* something else already covers it.
//! That coverage is an assumption about the platform, it is invisible in the table, and it is
//! the first thing a new target takes away. When a rule's exclusions rest on a fallback, the
//! fallback is part of the rule and moves to a new platform with it — or fails to.
//!
//! `docs/M8.md` Decision 2 takes the ruling: `Ctrl` stays, and `Tab` becomes a **third alias**
//! for `SYMBOL SHIFT` — one row in [`BINDINGS`], on both targets, covered by the existing
//! `tests/keymap_table.rs` without editing it. It does not remove the sharp edge; it provides a
//! way around one, and a person has to be told the way exists.
//!
//! Which host keys carry the two shifts is the one genuinely free choice, and it follows the
//! convention every Spectrum emulator has used for thirty years — `Shift` is `CAPS SHIFT`,
//! `Ctrl` is `SYMBOL SHIFT`. The argument for it is not aesthetic: a person who has used any
//! other emulator already has the muscle memory, and a mapping nobody has to learn is worth
//! more than a mapping that is easier to justify from first principles.
//!
//! # The founding rule is already false on most backends, and it is not fixable here
//!
//! *"The PC key's own printed meaning"* is what is printed on **the user's** keyboard. What
//! this table actually receives is a [`KeyCode`], and `miniquad` derives that differently on
//! each backend. Read out of the pinned `miniquad-0.4.11` source on 2026-09-01:
//!
//! | Backend | A [`KeyCode`] is derived from | Follows the layout? |
//! |---|---|---|
//! | Browser (`js/gl.js:1215`) | `into_sapp_keycode(event.code)` | **No** — `KeyboardEvent.code` is the *physical* key, named after a US board |
//! | Windows (`src/native/windows.rs:694`) | `HIWORD(lparam) & 0x1FF` — the **scan code** — through `windows/keycodes.rs:3` | **No** — set-1 scan codes, *"same as GLFW"* |
//! | macOS (`src/native/apple/apple_util.rs:154`) | `NSEvent.keyCode` | **No** — a positional virtual-key code |
//! | Linux X11 (`src/native/linux_x11/keycodes.rs:10`) | `XGetKeyboardMapping`, then `keysyms[0]` | **Yes** — a keysym is what the current layout says the key *means* |
//! | Linux Wayland (`src/native/linux_wayland/keycodes.rs`) | an `xkb` keysym | **Yes**, and its own header notes the keysym is modifier-dependent |
//!
//! **So on a French AZERTY board the physical key printed `A` is `KeyCode::Q` in a browser, on
//! Windows and on macOS, and `KeyCode::A` under X11 and Wayland** — one keypress, one machine,
//! two answers, and this table is then correctly consulted and correctly produces two different
//! membrane keys.
//!
//! Four things follow, and the third is the one that inverts the obvious reading:
//!
//! - **It is not fixable in this file.** The table is downstream of the divergence: by the time
//!   a [`KeyCode`] arrives, the information that would separate the two cases is gone.
//! - **It is invisible to every US-layout user**, which is everyone who can test it here. On a
//!   US board every backend agrees and every chord in the table is right.
//! - **The layout-following backends are the ones that honour the rule, and they are the
//!   minority.** X11 and Wayland report what the key *means* under the current layout, which is
//!   exactly what *"the PC key's own printed meaning"* asks for. The browser, Windows and macOS
//!   report where the key *is*. So on an AZERTY board the key printed `,` produces
//!   `SYMBOL SHIFT`+`N` — a comma, correctly — under X11, and produces the membrane `M` on the
//!   other three. **The founding rule is not "weakened in the browser"; it holds on two
//!   backends and fails on three, and one of the three is the desktop this was developed on.**
//! - **It is `observed`, and nobody here has observed it.** The mapping tables were read; a
//!   non-US keyboard was not pressed. `docs/STATUS.md` carries the open row and names what
//!   settles it: one person on AZERTY or QWERTZ, pressing one key, in both builds.
//!
//! # A known limit, stated rather than discovered
//!
//! Holding physical `Shift` *and* physical `,` presses `CAPS SHIFT` **and** `SYMBOL SHIFT`
//! **and** `N` — which the machine reads as extended mode, not as `<`. The two keyboards'
//! shift states are genuinely different things and this map does not reconcile them; it
//! translates one key at a time. Producing `<` is `SYMBOL SHIFT`+`R`, the way the hardware
//! does it. Fixing this properly means intercepting the host's *character* rather than its
//! keys, which is a different design and a much larger one.
//!
//! # What the gate is worth, precisely
//!
//! `crates/spectrum`'s own `tests/keyboard_matrix.rs` pins the membrane against the
//! hardware, and `docs/STATUS.md` records why: the previous test derived both its input and
//! its expectation from the function under test, and **38 of the 40 keys could be rewired
//! with the suite green**. The two that could not were the two whose expectations were
//! literals.
//!
//! `tests/keymap_table.rs` is written to that lesson, and its power is split three ways
//! rather than claimed as one:
//!
//! - **The bijection is structural.** Forty host keys onto forty membrane keys, none missed
//!   and none doubled, is true or false independently of anyone's taste. Forgetting to bind
//!   `V`, or binding `Q` twice, is a defect in any keymap whatever.
//! - **The chord targets have an external referent.** `DELETE` is printed above `0` on a real
//!   Spectrum. That half of the literal table can be checked against a photograph.
//! - **The forty specific assignments are a design decision pinned against drift.** They are
//!   literals, so permuting the table turns the test red — but the literals were transcribed
//!   from this file's own choices, so what they grade is *change*, not *correctness*.
//!   `docs/MACHINE.md` ranks that exact class fourth in its verification plan and says what
//!   it is worth: *"does not prove correctness — proves change, which is what catches a
//!   regression once something works."* It is not dressed up as more than that.

use macroquad::input::KeyCode;
use spectrum::{Key, Keyboard};

/// What one host key does to the membrane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// The one host key that owns this membrane key.
    ///
    /// Exactly forty of these exist and their [`Key`]s are all forty of the membrane's, which
    /// is the bijection `tests/keymap_table.rs` asserts.
    Membrane(Key),

    /// A second host key for a membrane key some [`Binding::Membrane`] entry already owns.
    ///
    /// Only the two shifts have one. A PC keyboard carries two `Shift`s and two `Ctrl`s and a
    /// person reaches for whichever hand is free, so binding only the left-hand pair would be
    /// letting the shape of the gate decide the shape of the product. Kept as its own variant
    /// rather than as a second `Membrane` row so that *"the membrane map is a bijection"*
    /// stays a property with no exceptions to name — a gate that has to list its own
    /// exceptions has already lost most of its power.
    Alias(Key),

    /// A host key the membrane has no equivalent for, produced the way the Spectrum's own
    /// legend produces it.
    Chord {
        /// `CAPS SHIFT` or `SYMBOL SHIFT` — the shift the legend prints this under.
        shift: Key,
        /// The key held with it.
        key: Key,
    },
}

impl Binding {
    /// Hold whatever this binding names.
    fn press(self, keyboard: &mut Keyboard) {
        match self {
            Self::Membrane(key) | Self::Alias(key) => keyboard.press(key),
            Self::Chord { shift, key } => {
                keyboard.press(shift);
                keyboard.press(key);
            }
        }
    }
}

/// Every host key that reaches the machine.
///
/// A flat slice rather than a `match`, because the frame loop needs to *enumerate* the host
/// keys it should ask about — [`macroquad::input::is_key_down`] answers one key at a time and
/// there is no "what is held" query to iterate instead. The slice is the one place the map is
/// written down, so the table the loop walks and the table the test reads are the same bytes.
pub const BINDINGS: &[(KeyCode, Binding)] = &[
    // ---- the two shifts ----------------------------------------------------------------
    (KeyCode::LeftShift, Binding::Membrane(Key::CapsShift)),
    (KeyCode::LeftControl, Binding::Membrane(Key::SymbolShift)),
    (KeyCode::RightShift, Binding::Alias(Key::CapsShift)),
    (KeyCode::RightControl, Binding::Alias(Key::SymbolShift)),
    // ---- the two keys that are their own legend ----------------------------------------
    (KeyCode::Enter, Binding::Membrane(Key::Enter)),
    (KeyCode::Space, Binding::Membrane(Key::Space)),
    // ---- letters -----------------------------------------------------------------------
    (KeyCode::A, Binding::Membrane(Key::A)),
    (KeyCode::B, Binding::Membrane(Key::B)),
    (KeyCode::C, Binding::Membrane(Key::C)),
    (KeyCode::D, Binding::Membrane(Key::D)),
    (KeyCode::E, Binding::Membrane(Key::E)),
    (KeyCode::F, Binding::Membrane(Key::F)),
    (KeyCode::G, Binding::Membrane(Key::G)),
    (KeyCode::H, Binding::Membrane(Key::H)),
    (KeyCode::I, Binding::Membrane(Key::I)),
    (KeyCode::J, Binding::Membrane(Key::J)),
    (KeyCode::K, Binding::Membrane(Key::K)),
    (KeyCode::L, Binding::Membrane(Key::L)),
    (KeyCode::M, Binding::Membrane(Key::M)),
    (KeyCode::N, Binding::Membrane(Key::N)),
    (KeyCode::O, Binding::Membrane(Key::O)),
    (KeyCode::P, Binding::Membrane(Key::P)),
    (KeyCode::Q, Binding::Membrane(Key::Q)),
    (KeyCode::R, Binding::Membrane(Key::R)),
    (KeyCode::S, Binding::Membrane(Key::S)),
    (KeyCode::T, Binding::Membrane(Key::T)),
    (KeyCode::U, Binding::Membrane(Key::U)),
    (KeyCode::V, Binding::Membrane(Key::V)),
    (KeyCode::W, Binding::Membrane(Key::W)),
    (KeyCode::X, Binding::Membrane(Key::X)),
    (KeyCode::Y, Binding::Membrane(Key::Y)),
    (KeyCode::Z, Binding::Membrane(Key::Z)),
    // ---- digits ------------------------------------------------------------------------
    (KeyCode::Key0, Binding::Membrane(Key::Num0)),
    (KeyCode::Key1, Binding::Membrane(Key::Num1)),
    (KeyCode::Key2, Binding::Membrane(Key::Num2)),
    (KeyCode::Key3, Binding::Membrane(Key::Num3)),
    (KeyCode::Key4, Binding::Membrane(Key::Num4)),
    (KeyCode::Key5, Binding::Membrane(Key::Num5)),
    (KeyCode::Key6, Binding::Membrane(Key::Num6)),
    (KeyCode::Key7, Binding::Membrane(Key::Num7)),
    (KeyCode::Key8, Binding::Membrane(Key::Num8)),
    (KeyCode::Key9, Binding::Membrane(Key::Num9)),
    // ---- CAPS SHIFT chords, from the red legends ---------------------------------------
    // Every one of these is a PC key whose own meaning is what the legend says.
    (
        KeyCode::Backspace,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Num0,
        },
    ), // DELETE
    (
        KeyCode::Left,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Num5,
        },
    ), // <-
    (
        KeyCode::Down,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Num6,
        },
    ), // v
    (
        KeyCode::Up,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Num7,
        },
    ), // ^
    (
        KeyCode::Right,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Num8,
        },
    ), // ->
    (
        KeyCode::CapsLock,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Num2,
        },
    ), // CAPS LOCK
    (
        KeyCode::Escape,
        Binding::Chord {
            shift: Key::CapsShift,
            key: Key::Space,
        },
    ), // BREAK
    // ---- SYMBOL SHIFT chords, from the red symbols under the letters -------------------
    // Exactly the punctuation that is unshifted on a US keyboard and present on the
    // Spectrum. `(`, `)`, `"` and the rest are shifted on a PC and are left to the hardware
    // route, so that the two keyboards' shift states never have to agree.
    (
        KeyCode::Comma,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::N,
        },
    ), // ,
    (
        KeyCode::Period,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::M,
        },
    ), // .
    (
        KeyCode::Semicolon,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::O,
        },
    ), // ;
    (
        KeyCode::Apostrophe,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::Num7,
        },
    ), // '
    (
        KeyCode::Minus,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::J,
        },
    ), // -
    (
        KeyCode::Equal,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::L,
        },
    ), // =
    (
        KeyCode::Slash,
        Binding::Chord {
            shift: Key::SymbolShift,
            key: Key::V,
        },
    ), // /
];

/// Something the *emulator* does, as opposed to something the machine does.
///
/// # The tape is three keys and not a toggle, which is a finding rather than a preference
///
/// [`spectrum::tape::Tape`] exposes `play`, `stop` and `rewind` but nothing that reports
/// whether it is **running**. A toggle therefore has to keep its own flag — and that flag
/// goes wrong on its own, because the tape stops itself: `play` on a wound-off tape is
/// documented to leave it stopped, and playback clears the same private field when it reaches
/// the end. The shell's flag would then say *playing* while the drive said *stopped*, and the
/// next press would appear to do nothing.
///
/// Shadowing state that the owner can change behind your back is how a frontend acquires a
/// bug nothing can see, so it is not done here. Three explicit keys are slightly less elegant
/// and always tell the truth. The smallest addition that would allow a toggle is named in the
/// report: `pub const fn is_playing(&self) -> bool` on `Tape`.
///
/// # Deliberately **not** `#[non_exhaustive]`
///
/// Its siblings in `crates/spectrum` are, and they should be — they are a published surface
/// with consumers outside this workspace. This one is matched exhaustively by the binary
/// beside it and by nothing else, and marking it non-exhaustive forces that match to carry a
/// wildcard arm: precisely the arm that would swallow a newly added hotkey and leave it
/// silently doing nothing. `crates/spectrum/src/keyboard.rs` makes the same trade for
/// `Key::position` and states the reason — *"the compiler then checks that every key has a
/// position, and adding a key that is forgotten here will not compile."*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    /// Show or hide the pacing readout.
    ToggleStatus,
    /// Write the machine's state out as a `.z80`.
    SaveSnapshot,
    /// Start the tape.
    PlayTape,
    /// Stop the tape where it stands.
    StopTape,
    /// Wind the tape back to the start.
    RewindTape,
    /// Press the reset button.
    Reset,
}

/// Host keys the emulator keeps for itself.
///
/// Function keys, deliberately: `tests/keymap_table.rs` asserts that nothing here also
/// appears in [`BINDINGS`], and the cheapest way to keep that true under later edits is to
/// take them from a range the membrane has no claim on at all. A hotkey that also typed a
/// letter would be the kind of defect that only shows up when somebody saves a snapshot
/// mid-sentence.
pub const HOTKEYS: &[(KeyCode, Hotkey)] = &[
    (KeyCode::F1, Hotkey::ToggleStatus),
    (KeyCode::F2, Hotkey::SaveSnapshot),
    (KeyCode::F3, Hotkey::PlayTape),
    (KeyCode::F4, Hotkey::StopTape),
    (KeyCode::F5, Hotkey::RewindTape),
    (KeyCode::F6, Hotkey::Reset),
];

/// Set `keyboard` to exactly what `is_down` reports, and nothing else.
///
/// Everything is released first and then re-pressed from scratch, rather than tracking
/// edges. That is not laziness: a held key that the host stops reporting — because the window
/// lost focus, or because the host swallowed it under its own modifier — leaves a key stuck
/// down forever under an edge-tracking model, and a stuck `CAPS SHIFT` is indistinguishable
/// from a broken emulator. Rebuilding the state each frame makes the machine's keyboard a
/// pure function of the host's, so there is no state to get out of step. It costs one pass
/// over [`BINDINGS`] per frame, on a machine that is already running 70,000 T-states in the
/// same tick.
///
/// `is_down` is a parameter rather than a direct call to [`macroquad::input::is_key_down`] so
/// that this function — the one piece of the keymap with behaviour rather than data — can be
/// driven from a test without a window. The gate that presses `,` and asserts `SYMBOL SHIFT`
/// and `N` come down together exists because of this seam.
pub fn apply(mut is_down: impl FnMut(KeyCode) -> bool, keyboard: &mut Keyboard) {
    keyboard.release_all();
    for &(code, binding) in BINDINGS {
        if is_down(code) {
            binding.press(keyboard);
        }
    }
}

/// The bound host key `name` refers to, matched case-insensitively.
///
/// Names are [`KeyCode`]'s own `Debug` spelling — `A`, `Key2`, `LeftControl`, `Backspace` —
/// so there is no second table of names to fall out of step with the first. The search is
/// over [`BINDINGS`] alone, which is a deliberate restriction rather than an oversight: it
/// means a caller driving the machine by name **cannot press a key the window could not
/// press**, and cannot reach a hotkey, which is an emulator control and not a key on the
/// machine at all.
///
/// This exists for `zx-shot`, which turns a written key script into taps on the membrane.
#[must_use]
pub fn code_named(name: &str) -> Option<KeyCode> {
    BINDINGS
        .iter()
        .map(|&(code, _)| code)
        .find(|code| format!("{code:?}").eq_ignore_ascii_case(name))
}
