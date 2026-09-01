//! The 40-key membrane, read through port `0xFE`.
//!
//! The keyboard is not a device with an address. It is eight half-rows of five keys wired
//! straight to the data bus, selected by the *high* half of the port address: `IN A,(0xFE)`
//! puts `A` on `A8–A15`, and every address line held **low** selects that half-row. The
//! five key bits come back low when pressed, and a scan with several rows selected returns
//! the AND of them — which is how `LD A,0; IN A,(0xFE)` tests "is any key down at all".
//!
//! Two consequences worth stating because they are easy to get wrong:
//!
//! - Selecting is active-low and pressing is active-low. Both inversions are real; only
//!   one of them cancelling is a bug.
//! - Bits 5–7 do not belong to the keyboard at all; [`crate::Ula`] supplies them.
//!
//! # A multi-half-row scan is a real thing software does, and here is the one that bites
//!
//! *"A scan with several rows selected returns the AND of them"* reads like a curiosity. It
//! is not, and the sharpest available demonstration came from disassembling a game rather
//! than from any test here: **Manic Miner reads `LD BC,0x7EFE` for its jump key**, and
//! `B = 0x7E` holds **A8 and A15 low together** — merging the `CAPS SHIFT` half-row with the
//! `B N M SYMBOL SHIFT SPACE` one.
//!
//! The consequence lands on anything mapping a host key to the machine's own cursor keys,
//! which are `CAPS SHIFT` with `5`–`8`: holding `CAPS SHIFT` to walk left makes Willy **jump
//! continuously**, because the game's merged scan cannot tell the two half-rows apart. That
//! is the machine behaving exactly correctly and a mapping being wrong, and it is the
//! strongest argument for a frontend reaching for [`crate::joystick`] — a *port*, which no
//! keyboard scan can touch — rather than for the membrane.

/// Half-rows the ULA scans.
pub const HALF_ROWS: usize = 8;

/// Keys wired to each half-row, and so the number of meaningful data bits.
pub const KEYS_PER_HALF_ROW: usize = 5;

/// The five key bits, all released.
pub const RELEASED: u8 = 0x1F;

const _: () = assert!(HALF_ROWS.is_power_of_two());
const _: () = assert!(RELEASED == (1 << KEYS_PER_HALF_ROW) - 1);

/// The address line that selects each half-row, in the order the half-rows are stored.
///
/// A table rather than a shift so the mapping from `A8`–`A15` to half-row is written down
/// once, in the one place it is used.
const HALF_ROW_SELECTORS: [u8; HALF_ROWS] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

/// A key on the 48K membrane.
///
/// Named for the legend on the key, not for what it produces: the Spectrum's `SYMBOL SHIFT`
/// and `CAPS SHIFT` change what every other key means, and encoding that here would put
/// the ROM's keyboard interpretation into the hardware model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Key {
    /// `CAPS SHIFT`, the left-hand shift.
    CapsShift,
    /// `Z`.
    Z,
    /// `X`.
    X,
    /// `C`.
    C,
    /// `V`.
    V,

    /// `A`.
    A,
    /// `S`.
    S,
    /// `D`.
    D,
    /// `F`.
    F,
    /// `G`.
    G,

    /// `Q`.
    Q,
    /// `W`.
    W,
    /// `E`.
    E,
    /// `R`.
    R,
    /// `T`.
    T,

    /// `1`.
    Num1,
    /// `2`.
    Num2,
    /// `3`.
    Num3,
    /// `4`.
    Num4,
    /// `5`.
    Num5,

    /// `0`.
    Num0,
    /// `9`.
    Num9,
    /// `8`.
    Num8,
    /// `7`.
    Num7,
    /// `6`.
    Num6,

    /// `P`.
    P,
    /// `O`.
    O,
    /// `I`.
    I,
    /// `U`.
    U,
    /// `Y`.
    Y,

    /// `ENTER`.
    Enter,
    /// `L`.
    L,
    /// `K`.
    K,
    /// `J`.
    J,
    /// `H`.
    H,

    /// `SPACE`, also `BREAK` with `CAPS SHIFT`.
    Space,
    /// `SYMBOL SHIFT`, the right-hand shift.
    SymbolShift,
    /// `M`.
    M,
    /// `N`.
    N,
    /// `B`.
    B,
}

impl Key {
    /// Which half-row this key is on, and which data bit it drives.
    ///
    /// Written as one exhaustive match rather than a lookup table: the compiler then
    /// checks that every key has a position, and adding a key that is forgotten here will
    /// not compile.
    #[must_use]
    const fn position(self) -> (usize, u8) {
        match self {
            // A8 — CAPS SHIFT, Z, X, C, V
            Self::CapsShift => (0, 0x01),
            Self::Z => (0, 0x02),
            Self::X => (0, 0x04),
            Self::C => (0, 0x08),
            Self::V => (0, 0x10),

            // A9 — A, S, D, F, G
            Self::A => (1, 0x01),
            Self::S => (1, 0x02),
            Self::D => (1, 0x04),
            Self::F => (1, 0x08),
            Self::G => (1, 0x10),

            // A10 — Q, W, E, R, T
            Self::Q => (2, 0x01),
            Self::W => (2, 0x02),
            Self::E => (2, 0x04),
            Self::R => (2, 0x08),
            Self::T => (2, 0x10),

            // A11 — 1, 2, 3, 4, 5
            Self::Num1 => (3, 0x01),
            Self::Num2 => (3, 0x02),
            Self::Num3 => (3, 0x04),
            Self::Num4 => (3, 0x08),
            Self::Num5 => (3, 0x10),

            // A12 — 0, 9, 8, 7, 6. The digits run *backwards* on this half-row, because
            // the two number rows meet in the middle of the keyboard.
            Self::Num0 => (4, 0x01),
            Self::Num9 => (4, 0x02),
            Self::Num8 => (4, 0x04),
            Self::Num7 => (4, 0x08),
            Self::Num6 => (4, 0x10),

            // A13 — P, O, I, U, Y
            Self::P => (5, 0x01),
            Self::O => (5, 0x02),
            Self::I => (5, 0x04),
            Self::U => (5, 0x08),
            Self::Y => (5, 0x10),

            // A14 — ENTER, L, K, J, H
            Self::Enter => (6, 0x01),
            Self::L => (6, 0x02),
            Self::K => (6, 0x04),
            Self::J => (6, 0x08),
            Self::H => (6, 0x10),

            // A15 — SPACE, SYMBOL SHIFT, M, N, B
            Self::Space => (7, 0x01),
            Self::SymbolShift => (7, 0x02),
            Self::M => (7, 0x04),
            Self::N => (7, 0x08),
            Self::B => (7, 0x10),
        }
    }
}

/// Which keys are currently held down.
///
/// A bit **set** here means the key is down; the inversion to the active-low data bus
/// happens once, in [`Keyboard::read`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Keyboard {
    half_rows: [u8; HALF_ROWS],
}

impl Keyboard {
    /// A keyboard with nothing pressed.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            half_rows: [0; HALF_ROWS],
        }
    }

    /// Hold `key` down.
    pub const fn press(&mut self, key: Key) {
        let (row, bit) = key.position();
        // INVARIANT: `position` returns a half-row index, and the mask makes that provable
        // to the compiler as well as to the reader.
        self.half_rows[row & (HALF_ROWS - 1)] |= bit;
    }

    /// Let `key` up.
    pub const fn release(&mut self, key: Key) {
        let (row, bit) = key.position();
        // INVARIANT: as in `press`.
        self.half_rows[row & (HALF_ROWS - 1)] &= !bit;
    }

    /// Let every key up.
    pub const fn release_all(&mut self) {
        self.half_rows = [0; HALF_ROWS];
    }

    /// Whether `key` is currently held.
    #[must_use]
    pub const fn is_pressed(&self, key: Key) -> bool {
        let (row, bit) = key.position();
        self.half_rows[row & (HALF_ROWS - 1)] & bit != 0
    }

    /// The five key bits a read of `port` returns, active low.
    ///
    /// Only the high half of `port` is consulted; the low half is what selected the ULA in
    /// the first place. Every half-row whose address line is **low** contributes, and the
    /// results are ANDed — so a scan of several rows reports a key down if any selected
    /// row has one.
    #[inline]
    #[must_use]
    pub fn read(&self, port: u16) -> u8 {
        let selector = (port >> 8) as u8;
        let mut bits = RELEASED;
        for (line, pressed) in HALF_ROW_SELECTORS.iter().zip(&self.half_rows) {
            if selector & line == 0 {
                bits &= !pressed;
            }
        }
        bits & RELEASED
    }
}

#[cfg(test)]
mod tests {
    //! # One test used to live here and has been removed
    //!
    //! `every_key_is_visible_to_a_scan_of_its_own_half_row` pressed each of the 40 keys and
    //! asserted it read low — on a port and against a bit **both derived from
    //! [`Key::position`], the function under test**. It therefore proved [`Keyboard::read`]
    //! consistent with `position`, and could say nothing about whether either matched the
    //! hardware. It is gone rather than kept alongside `tests/keyboard_matrix.rs`, which
    //! pins all 40 keys against a literal table of ports and bits that owes the crate
    //! nothing, and so proves strictly more.
    //!
    //! **That it was blind is measured, not argued.** Rotating the five bits of half-row 0
    //! — `CAPS SHIFT` `0x01`->`0x02`, `Z` `0x02`->`0x04`, `X` `0x04`->`0x08`,
    //! `C` `0x08`->`0x10`, `V` `0x10`->`0x01`, five keys rewired, distinctness preserved —
    //! left it **green**. The mutation was caught, at the commit that predates
    //! `tests/`, by exactly one test: `releasing_a_key_does_not_release_its_neighbours`,
    //! which happens to assert a **literal** `!0x04` for `X`. That literal, and the one in
    //! `a_key_reads_low_only_on_its_own_half_row` for `ENTER`, are the two anchors that make
    //! the surviving tests here worth keeping, and they are why the permutation a cold
    //! review found could move **38** of the 40 keys and not 40: those two could not move.
    //!
    //! The general form is in `docs/STATUS.md` — *a test whose expectation is computed by
    //! the subject is not a weak test; it is a tautology with a cross product attached*. The
    //! cross product is not the fix. Getting the other side of the comparison from somewhere
    //! the subject cannot reach is, and that is what the literal table does.

    use super::*;

    /// Every key, so exhaustiveness properties can be asserted over the whole membrane.
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

    #[test]
    fn nothing_pressed_reads_as_all_ones() {
        let keyboard = Keyboard::new();
        for port in [0xFEFE, 0xFDFE, 0x7FFE, 0x00FE] {
            assert_eq!(keyboard.read(port), RELEASED, "port {port:#06X}");
        }
    }

    #[test]
    fn a_key_reads_low_only_on_its_own_half_row() {
        let mut keyboard = Keyboard::new();
        keyboard.press(Key::Enter);
        assert_eq!(
            keyboard.read(0xBFFE),
            RELEASED & !0x01,
            "ENTER is on A14 bit 0"
        );
        assert_eq!(keyboard.read(0xFEFE), RELEASED, "and on no other half-row");
    }

    #[test]
    fn selecting_every_half_row_reports_any_key_down() {
        // `LD A,0; IN A,(0xFE)` — the "is anything pressed" scan the ROM uses.
        let mut keyboard = Keyboard::new();
        assert_eq!(keyboard.read(0x00FE), RELEASED);
        keyboard.press(Key::Space);
        assert_eq!(keyboard.read(0x00FE), RELEASED & !0x01);
    }

    #[test]
    fn releasing_a_key_does_not_release_its_neighbours() {
        let mut keyboard = Keyboard::new();
        keyboard.press(Key::Z);
        keyboard.press(Key::X);
        keyboard.release(Key::Z);
        assert!(!keyboard.is_pressed(Key::Z));
        assert!(keyboard.is_pressed(Key::X));
        assert_eq!(keyboard.read(0xFEFE), RELEASED & !0x04);
    }

    #[test]
    fn every_key_has_its_own_position() {
        let mut seen = Vec::with_capacity(ALL_KEYS.len());
        for key in ALL_KEYS {
            let position = key.position();
            assert!(
                !seen.contains(&position),
                "{key:?} shares half-row/bit {position:?} with an earlier key"
            );
            assert!(
                position.0 < HALF_ROWS,
                "{key:?} is on a half-row that does not exist"
            );
            assert!(
                position.1 & !RELEASED == 0,
                "{key:?} drives a bit the keyboard does not own"
            );
            seen.push(position);
        }
        assert_eq!(seen.len(), HALF_ROWS * KEYS_PER_HALF_ROW);
    }

    #[test]
    fn release_all_clears_every_half_row() {
        let mut keyboard = Keyboard::new();
        for key in ALL_KEYS {
            keyboard.press(key);
        }
        keyboard.release_all();
        assert_eq!(keyboard.read(0x00FE), RELEASED);
    }
}
