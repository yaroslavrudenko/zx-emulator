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
use spectrum::joystick::Direction;
use spectrum::{Joystick, Key, Keyboard};

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
    // A **third** alias for `SYMBOL SHIFT`, and the only row in this table that is here
    // because of a browser. `docs/M8.md` Decision 2: `Ctrl`+`8` and `Ctrl`+`9` are `(` and
    // `)`, a browser keeps them for switching tabs and does not offer them to the page, and
    // `preventDefault` cannot reach a shortcut the page never receives. `Tab` is unbound in
    // both tables, sits under the left little finger next to `Q`, and — the reason it is
    // `Tab` and not some other spare key — miniquad's own `gl.js` already `preventDefault`s
    // sapp keycode 258 with the comment `// tab - for UI`, so the browser's own use of it
    // (moving focus off the canvas, which would end keyboard input for the session) is
    // suppressed by the shipped bundle. `Ctrl`+`Tab` stays the browser's, which is correct:
    // that is a browser action and not a combination anybody types on a Spectrum.
    (KeyCode::Tab, Binding::Alias(Key::SymbolShift)),
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

/// The four host keys an [`ArrowScheme`] redirects.
///
/// In [`BINDINGS`] order, so that [`apply_with`] can match a scheme's fields to them positionally
/// without a second table saying which is which.
const ARROW_KEYS: [KeyCode; 4] = [KeyCode::Left, KeyCode::Down, KeyCode::Up, KeyCode::Right];

/// The Kempston directions the four [`ARROW_KEYS`] drive, in the same order.
///
/// Positional, so the two arrays cannot drift into disagreement without the compiler noticing
/// the length.
const ARROW_DIRECTIONS: [Direction; 4] = [
    Direction::Left,
    Direction::Down,
    Direction::Up,
    Direction::Right,
];

/// What the cursor keys press on the machine.
///
/// # A PC's arrows cannot map to one thing and be right, and that is a fact about the games
///
/// **The Spectrum has no arrow keys.** Nothing is printed on the membrane that means *move
/// left*, so there is no printed meaning for the founding rule to follow, and games do not
/// agree. Three camps, and this project has evidence for two of them:
///
/// - **The cursor keys** — physically `CAPS SHIFT` + `5`/`6`/`7`/`8`, which is what the arrows
///   printed on those keys mean. Every game advertising *"cursor"* control reads these.
/// - **Arbitrary letters**, chosen per game. **Manic Miner is here**: it reads `Q`–`P` for left
///   and right and the bottom row for jump, read out of the game's own text at `0x9D31` rather
///   than recalled. So the game the owner actually wants to play is **unreachable** through the
///   cursor chord, whatever else is true.
/// - **A Kempston joystick**, which is a separate port rather than the membrane — and which
///   ~~`crates/spectrum` does not have at all~~ **`crates/spectrum` has**: `spectrum::joystick`,
///   reached through `Spectrum::joystick_mut`, imported at the top of *this* file and pressed by
///   [`ArrowTarget::Kempston`] below. The parenthetical was written when the three camps were
///   first set out and the port did not exist; it survived the port's arrival, and went on
///   denying a device this very module drives.
///
/// **So a single fixed mapping is wrong for some large fraction of any collection**, and the
/// two ways of hiding that are both worse than admitting it: binding the arrows to the cursor
/// chord alone leaves Manic Miner dead, and pressing *both* the cursor chord and a letter set
/// at once — which several emulators do — means a game that happens to read `6` for something
/// else sees it held down every time somebody walks left.
///
/// The honest answer is the one `docs/M8.md` Decision 13 takes: **the arrows are a choice, the
/// choice is one keystroke away, and the current choice is on the screen.** That is not a
/// cop-out; the alternative is a mapping that silently does the wrong thing on half the games.
///
/// # It is still one table and one path
///
/// A scheme replaces what four host keys do and nothing else. [`apply_with`] is the only
/// function that presses anything, [`BINDINGS`] is still the only place the other fifty-three
/// rows live, and both targets run the same code — the property that made four byte sources one
/// `partition` and drag-and-drop one implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrowScheme {
    /// What to show a person who wants to know what the arrows currently do.
    ///
    /// **It names the audience, not just the keys, and that is the fix for a real defect.** The
    /// first version of this table called the `CAPS SHIFT` chord `"cursor"` — which is correct
    /// terminology and was read by the owner as *"the arrow keys"*, i.e. as the general answer
    /// rather than as the editor's. He was looking straight at the name of the thing that was
    /// wrong and it told him nothing. A name that says *who a scheme is for* is what the status
    /// bar is worth having.
    pub name: &'static str,

    /// One line saying what this scheme sends and who it suits, shown when `F7` selects it.
    ///
    /// Separate from [`ArrowScheme::name`] because the two are read at different moments: the
    /// name sits on the status bar every frame and has to be short, and this is shown once, at
    /// the instant somebody asked the question by pressing the key.
    ///
    /// **ASCII only** — `tests/on_screen_strings.rs` records why: the status bar's font draws
    /// anything else as an empty box, and that reached a user before a gate did.
    pub hint: &'static str,

    /// What the four arrow keys reach.
    pub target: ArrowTarget,
}

/// What an [`ArrowScheme`] drives.
///
/// Two variants because a Kempston joystick is **not the membrane**: it is a separate port, read
/// with `IN A,(31)` rather than by pulling an address line low. That is what makes it the
/// cleanest arrow target available — a game reading the joystick cannot collide with anything
/// another game reads on the keyboard, and vice versa — and it is also why it cannot be
/// expressed as a [`Binding`].
///
/// It is still one input path: [`apply_with`] is the only function that presses anything, it
/// rebuilds **both** devices from scratch every frame, and the variant only decides which of the
/// two it writes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowTarget {
    /// Four membrane bindings, in the order of [`ARROW_KEYS`]: left, down, up, right.
    Membrane([Binding; 4]),

    /// The Kempston port.
    ///
    /// **Active high, and with no interlock** — the hardware allows left and right at once, so
    /// nothing here prevents it either. `release_all` every frame is what stops a direction
    /// sticking when the window loses focus, which is the same discipline `apply` already
    /// applies to the membrane and for the same reason: a stuck direction is indistinguishable
    /// from a broken emulator.
    Kempston,

    /// Four membrane bindings **and** the Kempston port, from the same keypress.
    ///
    /// # Why driving two devices at once is safe here and is not safe on the membrane
    ///
    /// This module already refuses to press *two membrane mappings* at once, and
    /// [`apply_with`] says why in as many words: a game that happens to read `6` for something
    /// else would see it held down every time somebody walks left. **That argument is about
    /// two things sharing one device, and it does not carry over to this variant**, because
    /// Kempston is not the membrane — it is a separate port, read with `IN A,(31)` rather than
    /// by pulling an address line low. `crate::keymap`'s own [`ArrowTarget::Kempston`] doc and
    /// `spectrum::joystick`'s header both state the property this rests on: *a game reading the
    /// joystick cannot collide with anything another game reads on the keyboard, and vice
    /// versa*.
    ///
    /// So the port half is collision-free **by construction**, and the risk of the pair is
    /// exactly the risk of its membrane half alone — which is the argument for combining rather
    /// than choosing. `tests/keymap_under_a_game.rs` asserts the two halves stay in their own
    /// devices.
    ///
    /// # It is a superset, measured rather than argued
    ///
    /// Manic Miner, run from `testdata/games/ManicMiner.tap` on 2026-09-01, reads **both**: the
    /// bare digit and the port produce byte-identical machine state after an identical hold, and
    /// discarding the joystick instead of handing it to the machine makes the port branch
    /// identical to holding nothing at all. A game reads one or the other, so a scheme that
    /// sends both reaches strictly more titles than either half — at no cost on a game that
    /// reads only one, which is what "identical" above measures.
    Both([Binding; 4]),
}

/// A chord under `CAPS SHIFT`, which is how the machine's own cursor keys are typed.
const fn cursor(key: Key) -> Binding {
    Binding::Chord {
        shift: Key::CapsShift,
        key,
    }
}

/// Every arrow mapping, in the order `F7` cycles them.
///
/// # These are read out of the games, not chosen
///
/// Six games from the owner's own collection were disassembled on 2026-09-01 — the movement
/// routines decoded from memory, not the instruction text, because **the instruction text is
/// wrong about at least one of them**. What that found:
///
/// | Game | What it reads | How that was established |
/// |---|---|---|
/// | **Manic Miner** | Left `Q E T U O`, right `W R Y I P` — *interleaved*, not two halves — **and plain `5`/`8`**, and `7`/`0` for jump, and the whole bottom row for jump | Decoded at `0x8BFB`–`0x8C7F`, byte-identical across five images |
/// | **Cybernoid II** | Defaults `O` left, `P` right, `Q` up, `SPACE` fire; redefinable | Four decoded key slots at `0x2181`–`0x219F` |
/// | **Cybernoid I** | Same engine, same menu; this snapshot's slots all read `6` | Decoded; the defaults are not recoverable from it |
/// | **Exolon** | Redefinable; one default readable, key `O` | Decoded at `0x2056` |
/// | **Mario Bros** | **Nothing fixed.** It scans all eight half-rows into a buffer and redefines | Decoded at `0xB79F` |
/// | **Batty** | Nothing readable — body is compressed, 7.97 bits/byte | Measured entropy |
///
/// **Three of the six read no fixed keys at all**, which settles the design question on its own:
/// there is no single mapping to find, and the emulator's job is to be able to deliver *any*
/// key so that a game's own redefine menu works.
///
/// # The default was chosen against the wrong thing, and this records the correction
///
/// The machine's own cursor keys are `CAPS SHIFT` + `5`/`6`/`7`/`8`, and that is what the arrows
/// must do in BASIC — it is the printed meaning, and it is what [`BINDINGS`] has always said.
///
/// **But a game reading "cursor keys" reads the bare digit**, and pressing the chord presses
/// `CAPS SHIFT` as well. For Manic Miner that is not a harmless extra: its jump routine is
/// `LD BC,7EFEh : IN A,(C)`, and `B = 0x7E` pulls A8 and A15 low **together**, merging the row
/// that contains `CAPS SHIFT` into the read. Holding the cursor chord to walk therefore makes
/// Willy jump.
///
/// **All of that was written here, in this file, before the defect shipped — and the chord was
/// still made the default.** The reason given was that it *"is what `BINDINGS` says, so a build
/// that never touches `F7` behaves exactly as it did before schemes existed"*. That reasoning
/// weighed continuity with a previous build against being able to play, and continuity is not
/// something anybody asked for. The owner asked for the cursor keys to be the way he plays
/// games; the first thing he reported was *"whatever key I press, he jumps"*.
///
/// **The lesson is not "we forgot".** The hazard was identified, documented in full, and gated
/// against — `tests/keymap_under_a_game.rs` asserted that the cursor scheme trips the jump read
/// **and shipped that as the default anyway**, because the gate graded the scheme rather than
/// the choice of default. A predicted hazard placed behind a key the user has to know to press
/// is an unshipped fix. So the default is now the scheme that works in a game, and the chord is
/// one `F7` away for the editor.
///
/// Measured on 2026-09-01, from `.agent-workspace/manic-miner/probes/mm-from-tape.z80`, two runs
/// of the same deterministic machine over the same 12 frames, one with an arrow held:
///
/// | Scheme | `Right` held | What moved |
/// |---|---|---|
/// | `cursor` | walks **and jumps** | the same three bytes `SPACE` moves, every time |
/// | this default | walks | the walk bytes only; the jump bytes untouched |
///
/// # Why the default sends the digits *and* the port
///
/// A game reads the keyboard or it reads Kempston, and nothing can know which in advance —
/// three of the six games disassembled read no fixed keys at all. The port cannot collide with
/// the membrane ([`ArrowTarget::Both`] sets out why), so sending both covers strictly more
/// titles than either half at no cost to the other. Manic Miner reads both, and the two produce
/// identical state.
///
/// **What was rejected:** Kempston *alone* as the default. It collides with nothing, but it
/// reaches nothing on a game that does not read the port — which is a control that silently does
/// the wrong thing wearing the other mask, and this project has already fixed a drop that
/// silently did nothing for that exact reason. It stays in the cycle for the game that needs the
/// port without the digits.
pub const ARROW_SCHEMES: &[ArrowScheme] = &[
    ArrowScheme {
        // **The default.** The digits without `CAPS SHIFT`, so nothing trips a merged-row jump
        // read, plus the port, which no keyboard read can see. `5` left, `8` right, `7` jump on
        // Manic Miner; the port for everything that offers a joystick.
        name: "5678 + Kempston",
        hint: "arrows send 5/6/7/8 and the Kempston port - what most games read",
        target: ArrowTarget::Both([
            Binding::Membrane(Key::Num5),
            Binding::Membrane(Key::Num6),
            Binding::Membrane(Key::Num7),
            Binding::Membrane(Key::Num8),
        ]),
    },
    CURSOR_KEYS,
    ArrowScheme {
        // Cybernoid II's decoded defaults, and the commonest arbitrary set there is. `O` and
        // `P` are also in Manic Miner's left and right groups respectively, so this plays that
        // too — `Q` is the one to avoid there, because Manic Miner reads `Q` as **left**.
        name: "QAOP",
        hint: "arrows send Q/A/O/P - the commonest hand-picked set",
        target: ArrowTarget::Membrane([
            Binding::Membrane(Key::O),
            Binding::Membrane(Key::A),
            Binding::Membrane(Key::Q),
            Binding::Membrane(Key::P),
        ]),
    },
    ArrowScheme {
        // Sinclair joystick 1 as plain keys: 6 left, 7 right, 8 down, 9 up. Both Cybernoid
        // games and Exolon offer an "INTERFACE 2" option that reads exactly these.
        name: "Sinclair 1",
        hint: "arrows send 6/7/8/9 - for a game offering INTERFACE 2",
        target: ArrowTarget::Membrane([
            Binding::Membrane(Key::Num6),
            Binding::Membrane(Key::Num8),
            Binding::Membrane(Key::Num9),
            Binding::Membrane(Key::Num7),
        ]),
    },
    ArrowScheme {
        // Sinclair joystick 2: 1 left, 2 right, 3 down, 4 up.
        name: "Sinclair 2",
        hint: "arrows send 1/2/3/4 - INTERFACE 2, second stick",
        target: ArrowTarget::Membrane([
            Binding::Membrane(Key::Num1),
            Binding::Membrane(Key::Num3),
            Binding::Membrane(Key::Num4),
            Binding::Membrane(Key::Num2),
        ]),
    },
    ArrowScheme {
        // The port on its own. The default already includes it; this is here for the game that
        // reads the port *and* reads a bare digit for something else, where the digits have to
        // go. Last in the cycle because it reaches nothing on a game that ignores the port.
        name: "Kempston only",
        hint: "arrows send the Kempston port and no key at all",
        target: ArrowTarget::Kempston,
    },
];

/// The arrows as the machine's own: `CAPS SHIFT` with `5`/`6`/`7`/`8`.
///
/// # Named, rather than being whichever row of [`ARROW_SCHEMES`] happens to be first
///
/// This is the one scheme that is not a preference: it is what the legend prints on those keys,
/// what [`BINDINGS`] says, and what [`apply`] must keep doing. `zx-shot` types BASIC through
/// `apply`, and `tests/keymap_table.rs` grades the table through it — so `apply` has to mean
/// *the table*, permanently, and not *the current default*, which is a thing that changes.
///
/// Those two were the same value until the default moved, and a constant that is only correct
/// while two unrelated decisions happen to agree is a defect waiting for the next edit. One
/// definition, referenced from both places, so they cannot drift apart.
const CURSOR_KEYS: ArrowScheme = ArrowScheme {
    name: "cursor (BASIC)",
    hint: "arrows send CAPS SHIFT + 5/6/7/8 - the editor's cursor keys",
    target: ArrowTarget::Membrane([
        cursor(Key::Num5),
        cursor(Key::Num6),
        cursor(Key::Num7),
        cursor(Key::Num8),
    ]),
};

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
/// > **That addition landed, and it was a second feature that asked for it rather than this
/// > one.** [`spectrum::tape::Tape::is_playing`] exists, and its own documentation carries the
/// > argument above as the reason. What needed it was [`crate::pacing::Rung::Automatic`], which
/// > has to ask the drive **every tick** — a tape that stops itself at the end of its train is
/// > exactly the *"the owner can change it behind your back"* case, one layer up.
/// >
/// > **The three keys stay three keys.** Nothing about a toggle got better: `F3`/`F4`/`F5` are
/// > still an edge each, still a row of [`HOTKEYS`] `tests/keymap_table.rs` grades as a table,
/// > and collapsing two of them would buy one keystroke and cost the ability to say *stop* to a
/// > machine whose drive has already stopped itself. The accessor was the cheap half of that old
/// > trade; the toggle was the expensive half, and only the cheap half was worth having.
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
    /// Point the arrow keys at the next mapping in [`ARROW_SCHEMES`].
    ///
    /// A runtime choice rather than a build-time one because the right answer is a property of
    /// the **game**, and one artefact is meant to run more than one game.
    CycleArrows,
    /// Run the machine at the next rung in [`crate::pacing::RUNGS`].
    ///
    /// # A key that cycles, and not a key that is held
    ///
    /// A held key is the nicer gesture — press to skip a load, release when the game arrives —
    /// and it was rejected on where it would have had to live. Everything in [`HOTKEYS`] is an
    /// **edge**, read once through `is_key_pressed` by a loop `tests/keymap_table.rs` can grade
    /// as a table; a held modifier is a **level**, read through `is_key_down`, and the only place
    /// to put that is `main.rs` — the file whose own header says it is *"the untestable part, and
    /// it is kept thin on purpose"*. It would have been a second input mechanism, in the one file
    /// no test can reach, to save one keystroke. The cycle costs four presses to reach the top
    /// rung and one to come back — which is why [`crate::pacing::RUNGS`] widened its *step* rather
    /// than growing rungs when the measured ceiling turned out to be 98× — and every part of it is
    /// reachable from `cargo test`.
    ///
    /// # The rung that decides is a rung of this cycle, and that is what makes it safe
    ///
    /// [`crate::pacing::Rung::Automatic`] runs the machine flat out while a tape is moving. As
    /// anything other than a rung — a modifier, a rule that fires on PLAY, a preference — it
    /// would be a **second input mechanism** deciding the speed, which is the same objection this
    /// key already answered above about a held modifier, arriving with an extra failure mode:
    /// somebody parked at 1× to watch the loading stripes would be overtaken by it. As the last
    /// rung of one cycle there is nothing to reconcile. One key sets the speed, four presses
    /// reach the rung that decides, and a fifth is real time again.
    CycleSpeed,
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
    (KeyCode::F7, Hotkey::CycleArrows),
    (KeyCode::F8, Hotkey::CycleSpeed),
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
pub fn apply(is_down: impl FnMut(KeyCode) -> bool, keyboard: &mut Keyboard) {
    // **[`CURSOR_KEYS`], not `ARROW_SCHEMES[0]`, and the difference is the whole point.** This
    // function means *the [`BINDINGS`] table*: `zx-shot` types BASIC through it, and
    // `tests/keymap_table.rs` grades the table through it. `ARROW_SCHEMES[0]` means *the
    // window's current default*, which moved the day a game turned out to need a different one.
    // Pointing this at the default made those two decisions the same decision, so changing the
    // one silently changed the other — in a binary this file does not own.
    //
    // `CURSOR_KEYS` is a membrane scheme, so the returned joystick is always released and
    // discarding it loses nothing. A caller wanting a scheme uses [`apply_with`].
    let _ = apply_with(is_down, keyboard, &CURSOR_KEYS);
}

/// [`apply`], with the arrow keys pointed at `scheme` instead of at their [`BINDINGS`] rows.
///
/// The four host keys in [`ARROW_KEYS`] take their binding from `scheme`; every other row comes
/// from `BINDINGS` exactly as before. **They are replaced rather than added to**: pressing both
/// the cursor chord and a letter set is what makes a game that reads `6` for something else see
/// it held down every time somebody walks left, and that failure is silent and looks like a bug
/// in the game.
///
/// [`CURSOR_KEYS`] reproduces the `BINDINGS` rows exactly, so calling this with it is the same
/// thing as calling [`apply`] — which is what lets every existing gate keep grading the table
/// rather than the scheme.
pub fn apply_with(
    mut is_down: impl FnMut(KeyCode) -> bool,
    keyboard: &mut Keyboard,
    scheme: &ArrowScheme,
) -> Joystick {
    // **The joystick is returned rather than mutated, and that is not a style choice.**
    // `Spectrum` hands out `keyboard_mut` and `joystick_mut` separately, so a function taking
    // both would need two simultaneous `&mut` borrows of the same machine and would not
    // compile. Building a fresh `Joystick` and handing it back is also the more honest shape:
    // *rebuilt from the host's state every frame* is literally what a returned value is.
    //
    // It matters more here than for the membrane. Kempston is **active high** and has no
    // interlock, so a direction the host stops reporting — a lost window focus, a backgrounded
    // browser tab — would stay pressed for the rest of the session under any design that
    // tracked edges. `Joystick::default()` is every direction released.
    keyboard.release_all();
    let mut joystick = Joystick::default();

    for &(code, binding) in BINDINGS {
        if !is_down(code) {
            continue;
        }
        let arrow = ARROW_KEYS.iter().position(|&key| key == code);
        match (arrow, scheme.target) {
            (Some(index), ArrowTarget::Membrane(bindings)) => bindings[index].press(keyboard),
            (Some(index), ArrowTarget::Kempston) => joystick.press(ARROW_DIRECTIONS[index]),
            // Two devices, one keypress. Safe only because they *are* two devices: see
            // [`ArrowTarget::Both`] for why this is not the both-at-once the paragraph above
            // refuses, which is two mappings sharing the membrane.
            (Some(index), ArrowTarget::Both(bindings)) => {
                bindings[index].press(keyboard);
                joystick.press(ARROW_DIRECTIONS[index]);
            }
            (None, _) => binding.press(keyboard),
        }
    }
    joystick
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
