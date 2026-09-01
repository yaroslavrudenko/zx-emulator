//! The Kempston joystick: five switches on a port, and nothing else.
//!
//! # Why a port rather than the keyboard
//!
//! A Spectrum has no arrow keys, so games reach a joystick three ways: the **cursor keys**
//! (`CAPS SHIFT` with `5`–`8`), **arbitrary letters** chosen by the author, or **Kempston**,
//! which is an interface on the bus rather than anything on the membrane. Only the third can
//! be driven without colliding with something the game also reads on the keyboard — a
//! keyboard-based mapping has to know which keys the game itself uses, and nothing can know
//! that in general.
//!
//! # What is modelled and what is not
//!
//! | | |
//! |---|---|
//! | Five switches — four directions and fire | yes |
//! | Diagonals | yes, and they are just two switches held at once |
//! | Opposite directions held together | **yes, and deliberately** — the hardware has no interlock, so left-and-right at once is a state a real stick can be forced into and some games test for it |
//! | The unused top three bits | driven **low**, by construction — see [`UNUSED_BITS`] |
//! | A second joystick | **no.** Kempston is one port and one stick |
//! | Autofire, analogue sticks, Kempston mouse | **no** |
//!
//! # The port, and its decode, which a manufacturer's schematic settles
//!
//! The address is `0x1F`; the **decode** is `A5 = A6 = A7 = 0` with `/RD`, `/IORQ` and
//! `/M1 = 1`, from the Kempston Issue 4 (1989) schematic — so the window is the whole low byte
//! `0x00..=0x1F` and `A0`-`A4` are not wired at all. [`KEMPSTON_PORT_MASK`] carries the
//! sourcing, the `/M1` term that the folklore drops, and the reason a full `0x1F` compare is
//! *wrong* rather than merely narrow.
//!
//! # The Beta Disk shares this address, and the hardware solves it by disconnection
//!
//! The Beta Disk interface's FDC command/status register is at `0x1F` too. Real hardware does
//! not arbitrate: **the Beta gates `/IORQ` into a separate `IORQ EX` for its own rear
//! connector**, so a downstream card is cut off the I/O bus rather than contending with it —
//! and the Beta's own manual tells you to plug the joystick in there
//! (*"The connection opposite A is for adding your other interfaces, e.g. joystick."*).
//!
//! The Kempston itself is stateless and has no disable line; it would answer regardless. So
//! **nothing here needs to know about the Beta**, and when one is written the gating belongs
//! on its side. Recorded because the next person to add a disk interface will meet it.

/// The port a Kempston interface answers.
pub const KEMPSTON_PORT: u16 = 0x001F;

/// Address bits the decode consults: **A5, A6 and A7**, all low.
///
/// # Primary, from the manufacturer's own schematic
///
/// This crate first matched the canonical address's low byte and said plainly that it had no
/// source for a decode. It has one now — the **Kempston Joystick Interface, Issue 4 (1989)**
/// schematic, which shows a `SN74LS138` with `A5`, `A6`, `A7` on its three select inputs and
/// the strobes on its enables:
///
/// ```text
///   Y0 asserts  <=>  A7 = A6 = A5 = 0  AND  /RD = 0  AND  /IORQ = 0  AND  /M1 = 1
/// ```
///
/// `Y0` drives the output enables of the `SN74LS366` that puts the switches on the bus. So the
/// window is the low byte `0x00..=0x1F`, and **`A0`-`A4` are not wired to anything** — the
/// canonical `0x1F` is one member of a family of thirty-two, and `IN A,(0x00)` reads the
/// joystick on real hardware. A full low-byte compare, which this crate had, is *wrong* rather
/// than merely narrow.
///
/// Corroborated independently by a KiCad redraw of the **Cheetah** Kempston-compatible
/// interface — different author, different manufacturer, same two chips, and a complete net
/// list containing exactly `A5`, `A6`, `A7` among the address lines.
///
/// # The `/M1` term is the half the folklore drops, and this emulator satisfies it by
/// construction
///
/// Cheap clones that decode `A5` alone **and ignore the read strobe** drive the data bus during
/// the Z80's interrupt-acknowledge cycle — `/IORQ` is asserted there with neither `/RD` nor
/// `/WR` — and the CPU takes the joystick's byte as its IM 2 vector. The defect is recorded by
/// a named engineer, Miguel Angel Rodriguez, in ZEsarUX's source.
///
/// **It cannot happen here, and not because of anything this module does.** An interrupt
/// acknowledge reaches this bus through `z80::Bus::acknowledge`, which M7 added for the
/// contention charge; it never calls `in_port`. The `/RD` and `/M1` terms are therefore
/// satisfied by the shape of the `Bus` trait rather than by a mask — which is worth writing
/// down, because a future bus that routed an acknowledge through `in_port` would reintroduce
/// a hardware defect this machine currently cannot have.
///
/// # What the two "competing claims" turned out to be
///
/// *"A5 only"* and *"A5, A6, A7"* are not two traditions with two sources. They are **one
/// self-contradicting page**: the World of Spectrum ports FAQ gives `---- ---- 000- ----` in
/// its prose row and `0x20  /* ---- ---- --0- ---- */` in a `#define` one line below, the two
/// halves by different authors and never reconciled. Everything else in circulation is that
/// page restated.
///
/// **A5-only is not folklore either — it is right for other hardware**, and the only
/// hardware-verified decode in the whole record is one of those: Fuse changed its **Timex
/// TC2048** mask to `A5` alone in 2007 with the commit message *"Correct joystick port mask on
/// TC2048 (verified on real hardware)"*. That is a Timex clone's built-in port, not a Kempston,
/// and Fuse's own Kempston mask has no cited verification anywhere.
///
/// # And a caution about which board this is
///
/// The **1984** Kempston is a different circuit — a `74LS541` and a `4071` where the 1989 board
/// has a `74LS138` and a `74LS366`. A `4071` is OR-only and a `74LS541` is non-inverting, so
/// that board has no inverter and can only test address lines for being **low**; a full
/// `0x1F` compare is physically impossible on it. Its exact subset was **not traced** and
/// nothing in the literature describes it. This constant follows the board that has a
/// schematic.
pub const KEMPSTON_PORT_MASK: u16 = 0x00E0;

/// The value the masked address must equal: A5, A6 and A7 all low.
pub const KEMPSTON_PORT_SELECT: u16 = 0x0000;

const _: () = assert!(KEMPSTON_PORT & KEMPSTON_PORT_MASK == KEMPSTON_PORT_SELECT);

/// Bits 5-7 of a read, which the interface drives **low**.
///
/// # Zero by construction, and the schematic says how
///
/// This crate first recorded these as *"zero here, and no source establishes it"*, with a
/// floating alternative named as the real possibility. The schematic settles it, and the
/// mechanism is more deliberate than a convention:
///
/// - **D5** is the sixth buffer of the `74LS366`, its input tied to `+5V` and the part
///   inverting — a hard `0`.
/// - **D6** and **D7** are pulled low through two `1N4148` diodes to the decode strobe.
///
/// So a genuine Kempston's byte is **fully defined on all eight bits** and an idle read is
/// `0x00`. Kempston's own instruction leaflet states the polarity outright, and it is the
/// inverse of everything else on the machine: *"Note that all bits are normally high (1) and
/// go low (0) when the joystick is moved, except the Kempston IN 31 standard where all bits
/// are normally low (0) and go high (1) when the joystick is moved."*
///
/// **The exception, named because it is real and is not this machine:** Scorpion-family clones
/// merge the floppy controller's `DRQ`/`INTRQ` into bits 5-7 of the same read. That is a
/// different machine, and `docs/M7.md` Decision 10 keeps those out of scope.
pub const UNUSED_BITS: u8 = 0xE0;

/// One of the five switches.
///
/// An enum rather than a bitmask in the public API for the same reason [`crate::Key`] is one:
/// a caller says what it means and cannot name a bit that is not a switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    /// Bit 0.
    Right,
    /// Bit 1.
    Left,
    /// Bit 2.
    Down,
    /// Bit 3.
    Up,
    /// Bit 4.
    Fire,
}

impl Direction {
    /// Every switch, in bit order.
    pub const ALL: [Self; 5] = [Self::Right, Self::Left, Self::Down, Self::Up, Self::Fire];

    /// The bit this switch sets in a port read.
    ///
    /// **Transcribed**, and the layout is the one thing about Kempston that every source
    /// agrees on: right, left, down, up, fire, from bit 0 up. A model that permuted two of
    /// them would send a game the wrong way and nothing in this repository would fail, which
    /// is why `crates/spectrum/tests/kempston.rs` asserts the five literals rather than
    /// deriving them from this function.
    #[must_use]
    pub const fn bit(self) -> u8 {
        match self {
            Self::Right => 0x01,
            Self::Left => 0x02,
            Self::Down => 0x04,
            Self::Up => 0x08,
            Self::Fire => 0x10,
        }
    }
}

/// Which of the five switches are held.
///
/// **Active high**, unlike the keyboard's membrane: a Kempston read returns a `1` for each
/// direction held, where a `0xFE` read returns a `0` for each key. Getting that backwards
/// gives a machine whose joystick is permanently pushed in every direction at once, which is
/// why the two conventions are named beside each other here rather than left to be recalled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Joystick {
    held: u8,
}

impl Joystick {
    /// A joystick with nothing held.
    #[must_use]
    pub const fn new() -> Self {
        Self { held: 0 }
    }

    /// Hold `direction`.
    ///
    /// Holding two opposite directions at once is allowed, because the hardware allows it:
    /// there is no interlock in a switch box, and a stick can be forced. Some games check for
    /// it as a cheat or a debug key, so refusing it here would model a machine nobody built.
    pub const fn press(&mut self, direction: Direction) {
        self.held |= direction.bit();
    }

    /// Release `direction`.
    pub const fn release(&mut self, direction: Direction) {
        self.held &= !direction.bit();
    }

    /// Release everything.
    ///
    /// What a frontend calls when it loses focus, so a held direction does not stick.
    pub const fn release_all(&mut self) {
        self.held = 0;
    }

    /// Whether `direction` is held.
    #[must_use]
    pub const fn is_pressed(self, direction: Direction) -> bool {
        self.held & direction.bit() != 0
    }

    /// The byte a read of [`KEMPSTON_PORT`] returns.
    #[must_use]
    pub const fn read(self) -> u8 {
        self.held & !UNUSED_BITS
    }
}

// The five switches must not reach the bits the interface does not drive, or a direction
// would read back as noise in the top of the byte.
const _: () = assert!(Direction::Right.bit() & UNUSED_BITS == 0);
const _: () = assert!(Direction::Fire.bit() & UNUSED_BITS == 0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_switches_are_the_published_bits() {
        // Literals, not derived from `Direction::bit`, because that function is the subject.
        // A permutation of two directions sends a game the wrong way and nothing else here
        // would notice.
        assert_eq!(Direction::Right.bit(), 0x01);
        assert_eq!(Direction::Left.bit(), 0x02);
        assert_eq!(Direction::Down.bit(), 0x04);
        assert_eq!(Direction::Up.bit(), 0x08);
        assert_eq!(Direction::Fire.bit(), 0x10);
    }

    #[test]
    fn every_switch_has_its_own_bit_and_none_of_them_is_a_spare() {
        let mut seen = 0_u8;
        for direction in Direction::ALL {
            assert_eq!(seen & direction.bit(), 0, "{direction:?} shares a bit");
            seen |= direction.bit();
        }
        assert_eq!(
            seen, !UNUSED_BITS,
            "the five switches fill the five low bits"
        );
        assert_eq!(Direction::ALL.len(), 5);
    }

    #[test]
    fn an_idle_joystick_reads_zero_and_a_held_one_reads_its_bit() {
        // Active high, which is the opposite of the keyboard's membrane. A model with the
        // convention inverted reads `0x1F` idle — every direction held at once, forever.
        let mut joystick = Joystick::new();
        assert_eq!(joystick.read(), 0x00);
        for direction in Direction::ALL {
            joystick.press(direction);
            assert_eq!(joystick.read(), direction.bit(), "{direction:?}");
            assert!(joystick.is_pressed(direction));
            joystick.release(direction);
            assert_eq!(joystick.read(), 0x00, "{direction:?} released");
        }
    }

    #[test]
    fn a_diagonal_is_two_switches_and_opposites_are_allowed() {
        let mut joystick = Joystick::new();
        joystick.press(Direction::Up);
        joystick.press(Direction::Right);
        assert_eq!(joystick.read(), 0x08 | 0x01);

        // No interlock, because a switch box has none and some games test for it.
        joystick.press(Direction::Left);
        assert_eq!(joystick.read(), 0x08 | 0x01 | 0x02);
        assert!(joystick.is_pressed(Direction::Left) && joystick.is_pressed(Direction::Right));
    }

    #[test]
    fn releasing_everything_leaves_nothing_held() {
        let mut joystick = Joystick::new();
        for direction in Direction::ALL {
            joystick.press(direction);
        }
        assert_eq!(joystick.read(), 0x1F);
        joystick.release_all();
        assert_eq!(joystick.read(), 0x00);
        assert_eq!(joystick, Joystick::new());
    }

    #[test]
    fn a_read_never_sets_a_bit_the_interface_does_not_drive() {
        // The top three bits are the interface's, not the switches'. Whatever they are chosen
        // to read as, a direction must never reach them.
        let mut joystick = Joystick::new();
        for direction in Direction::ALL {
            joystick.press(direction);
        }
        assert_eq!(joystick.read() & UNUSED_BITS, 0);
    }

    #[test]
    fn the_decode_is_three_address_lines_and_the_family_is_thirty_two_wide() {
        // **The schematic's window, asserted as a family rather than as an address.** A first
        // cut matched the whole low byte, which is not merely narrow but wrong: `A0`-`A4` are
        // not wired to anything on the board, so `IN A,(0x00)` reads the joystick on real
        // hardware exactly as `IN A,(0x1F)` does.
        let answered = (0..=0xFF_u16)
            .filter(|port| port & KEMPSTON_PORT_MASK == KEMPSTON_PORT_SELECT)
            .count();
        assert_eq!(answered, 32, "A5, A6 and A7 low leaves five lines free");
        for port in [0x0000_u16, 0x0001, 0x001E, KEMPSTON_PORT] {
            assert_eq!(
                port & KEMPSTON_PORT_MASK,
                KEMPSTON_PORT_SELECT,
                "{port:#06X}"
            );
        }

        // The high half is whatever the program had lying around — `IN A,(n)` puts `A` there
        // and `IN A,(C)` puts `B` — so the decode must not consult it.
        for high in [0x00_u16, 0x12, 0xFF] {
            let port = (high << 8) | KEMPSTON_PORT;
            assert_eq!(
                port & KEMPSTON_PORT_MASK,
                KEMPSTON_PORT_SELECT,
                "{port:#06X}"
            );
        }

        // And every port this machine's other devices actually answer stays outside it, which
        // is what makes the wider decode safe here rather than merely correct.
        for port in [0x00FE_u16, 0x7FFD, 0xFFFD, 0xBFFD, 0x003F, 0x00DF] {
            assert_ne!(
                port & KEMPSTON_PORT_MASK,
                KEMPSTON_PORT_SELECT,
                "{port:#06X}"
            );
        }
    }
}
