//! The register file, stored as one flat byte array.
//!
//! # Why an array and not named fields
//!
//! The `DD` and `FD` prefixes substitute `IX` or `IY` for `HL` in the instruction that
//! follows them. With named fields every `HL`-touching handler would need a branch to ask
//! which register it is really operating on — a branch repeated across the whole
//! `LD`/`ALU`/`INC`/`DEC` set, each copy a place to get it wrong.
//!
//! Laid out as an array, the substitution becomes a **constant index offset**: `HL`, `IX`
//! and `IY` are three [`PairBase`] values, and one handler serves all three with no `if`
//! at all. That base is threaded from the decoder as a parameter, so M2 implements the
//! prefixes by passing a different constant rather than by editing any handler.
//!
//! # Layout
//!
//! ```text
//!  0  1   2  3  4  5  6  7    8   9   10 11 12 13 14 15
//!  A  F   B  C  D  E  H  L    A'  F'  B' C' D' E' H' L'
//!
//!  16  17   18  19   20  21   22 23   24  25
//!  IXh IXl  IYh IYl  SPh SPl  I  R    PCh PCl
//! ```
//!
//! Every 16-bit pair is stored **high byte first at an even index**, so a pair is named by
//! the index of its high byte and read with one `u16::from_be_bytes`. The main set `B..L`
//! sits contiguously opposite its shadow `B'..L'`, which makes `EXX` a short loop over a
//! fixed offset rather than six hand-written swaps.
//!
//! # Why the indices are newtypes
//!
//! [`RegIndex`] and [`PairBase`] wrap a `usize` whose only constructors are the constants
//! in [`index`] and [`pair`], plus [`PairBase::high`] and [`PairBase::low`]. No arbitrary
//! integer can reach the array, so the bound is enforced by construction rather than by a
//! comment — which matters because the base is now a runtime value threaded from the
//! decoder, and a comment asserting "this is always in range" would no longer be checking
//! anything.

/// Number of bytes in the register array.
const REGISTER_COUNT: usize = 26;

/// Bytes spanned by `BC`, `DE` and `HL` together — the block `EXX` exchanges.
const MAIN_PAIR_BLOCK: usize = 6;

/// `R` is a 7-bit counter: the M1 increment wraps within bits 0–6 and never disturbs
/// bit 7, which only a `LD R,A` can change.
const REFRESH_COUNTER_MASK: u8 = 0x7F;

/// The index of one 8-bit register.
///
/// Constructible only from the constants in [`index`] and from [`PairBase::high`] /
/// [`PairBase::low`], all of which are in range by inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegIndex(usize);

/// The base index of a 16-bit pair — the index of its high byte.
///
/// This is the type the `DD`/`FD` substitution operates on: `HL`, `IX` and `IY` differ
/// only in the value carried here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PairBase(usize);

impl PairBase {
    /// The pair's high byte — `H` for `HL`, `IXh` for `IX`.
    pub(crate) const fn high(self) -> RegIndex {
        RegIndex(self.0)
    }

    /// The pair's low byte — `L` for `HL`, `IXl` for `IX`.
    pub(crate) const fn low(self) -> RegIndex {
        RegIndex(self.0 + 1)
    }
}

/// Indices of the 8-bit registers that no prefix ever substitutes.
///
/// `H` and `L` are deliberately absent: they are reached through [`PairBase::high`] and
/// [`PairBase::low`] precisely because a `DD`/`FD` prefix moves them and these do not.
pub(crate) mod index {
    use super::RegIndex;

    /// Accumulator.
    pub(crate) const A: RegIndex = RegIndex(0);
    /// Flags.
    pub(crate) const F: RegIndex = RegIndex(1);
    /// `B`.
    pub(crate) const B: RegIndex = RegIndex(2);
    /// `C`.
    pub(crate) const C: RegIndex = RegIndex(3);
    /// `D`.
    pub(crate) const D: RegIndex = RegIndex(4);
    /// `E`.
    pub(crate) const E: RegIndex = RegIndex(5);
    /// Interrupt vector register.
    pub(crate) const I: RegIndex = RegIndex(22);
    /// Memory refresh register.
    pub(crate) const R: RegIndex = RegIndex(23);
}

/// Base indices of the 16-bit pairs.
pub(crate) mod pair {
    use super::PairBase;

    /// Accumulator and flags.
    pub(crate) const AF: PairBase = PairBase(0);
    /// `BC`.
    pub(crate) const BC: PairBase = PairBase(2);
    /// `DE`.
    pub(crate) const DE: PairBase = PairBase(4);
    /// `HL` — the pair a `DD`/`FD` prefix replaces with [`IX`] or [`IY`].
    pub(crate) const HL: PairBase = PairBase(6);
    /// Shadow `AF'`.
    pub(crate) const AF_SHADOW: PairBase = PairBase(8);
    /// Shadow `BC'`, and the base of the contiguous `B'C'D'E'H'L'` block.
    pub(crate) const BC_SHADOW: PairBase = PairBase(10);
    /// Shadow `DE'`.
    pub(crate) const DE_SHADOW: PairBase = PairBase(12);
    /// Shadow `HL'`.
    pub(crate) const HL_SHADOW: PairBase = PairBase(14);
    /// `IX`.
    pub(crate) const IX: PairBase = PairBase(16);
    /// `IY`.
    pub(crate) const IY: PairBase = PairBase(18);
    /// Stack pointer.
    pub(crate) const SP: PairBase = PairBase(20);
    /// Program counter.
    pub(crate) const PC: PairBase = PairBase(24);
}

/// The Z80 register file.
///
/// The array starts zeroed; the post-reset values a real Z80 powers up with are defined
/// once, as [`crate::CpuState::default`], so the reset state has a single home rather than
/// one here and another in the snapshot type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Registers {
    regs: [u8; REGISTER_COUNT],
}

impl Registers {
    /// Read one 8-bit register.
    pub(crate) fn get(&self, index: RegIndex) -> u8 {
        self.regs[index.0]
    }

    /// Write one 8-bit register.
    pub(crate) fn set(&mut self, index: RegIndex, value: u8) {
        self.regs[index.0] = value;
    }

    /// Read a 16-bit pair.
    pub(crate) fn pair(&self, base: PairBase) -> u16 {
        u16::from_be_bytes([self.get(base.high()), self.get(base.low())])
    }

    /// Write a 16-bit pair.
    pub(crate) fn set_pair(&mut self, base: PairBase, value: u16) {
        let [high, low] = value.to_be_bytes();
        self.set(base.high(), high);
        self.set(base.low(), low);
    }

    /// The accumulator.
    pub(crate) fn a(&self) -> u8 {
        self.get(index::A)
    }

    /// Set the accumulator.
    pub(crate) fn set_a(&mut self, value: u8) {
        self.set(index::A, value);
    }

    /// The flags register.
    pub(crate) fn f(&self) -> u8 {
        self.get(index::F)
    }

    /// Set the flags register.
    ///
    /// Prefer [`crate::Cpu::write_flags`] from instruction handlers: it also records the
    /// flag latch that `SCF` and `CCF` read back.
    pub(crate) fn set_f(&mut self, value: u8) {
        self.set(index::F, value);
    }

    /// The stack pointer.
    pub(crate) fn sp(&self) -> u16 {
        self.pair(pair::SP)
    }

    /// Set the stack pointer.
    pub(crate) fn set_sp(&mut self, value: u16) {
        self.set_pair(pair::SP, value);
    }

    /// The program counter.
    pub(crate) fn pc(&self) -> u16 {
        self.pair(pair::PC)
    }

    /// Set the program counter.
    pub(crate) fn set_pc(&mut self, value: u16) {
        self.set_pair(pair::PC, value);
    }

    /// Advance `PC` by one, wrapping at the top of the address space.
    pub(crate) fn advance_pc(&mut self) {
        self.set_pc(self.pc().wrapping_add(1));
    }

    /// The refresh address the Z80 drives during the cycles that follow an opcode fetch:
    /// `I` in the high byte, `R` in the low.
    ///
    /// A machine cannot reconstruct this from transfer addresses, which is why it has to
    /// reach the bus through [`crate::Bus::tick`].
    pub(crate) fn refresh_address(&self) -> u16 {
        u16::from_be_bytes([self.get(index::I), self.get(index::R)])
    }

    /// Increment `R` the way an opcode fetch does.
    ///
    /// The refresh counter is only 7 bits wide. Bit 7 is a latch that survives every M1
    /// cycle and changes only when software writes it with `LD R,A` — which is exactly why
    /// programs can use `R` as a cheap pseudo-random source and why copy protection can
    /// use it as a fingerprint.
    pub(crate) fn increment_r(&mut self) {
        let refresh = self.get(index::R);
        let incremented =
            (refresh.wrapping_add(1) & REFRESH_COUNTER_MASK) | (refresh & !REFRESH_COUNTER_MASK);
        self.set(index::R, incremented);
    }

    /// `EX AF,AF'` — exchange the accumulator and flags with their shadows.
    pub(crate) fn exchange_af(&mut self) {
        self.regs.swap(pair::AF.0, pair::AF_SHADOW.0);
        self.regs.swap(pair::AF.0 + 1, pair::AF_SHADOW.0 + 1);
    }

    /// `EXX` — exchange `BC`, `DE` and `HL` with their shadows in one operation.
    ///
    /// The main and shadow blocks are contiguous and identically ordered, so this is the
    /// same fixed offset applied six times.
    pub(crate) fn exchange_shadow_set(&mut self) {
        for offset in 0..MAIN_PAIR_BLOCK {
            self.regs
                .swap(pair::BC.0 + offset, pair::BC_SHADOW.0 + offset);
        }
    }

    /// `EX DE,HL` — exchange the two pairs in place.
    ///
    /// Note that this always names the *main* `HL`, never `IX` or `IY`: `DD EB` is
    /// `EX DE,HL`, not `EX DE,IX`. It is one of the few `HL` instructions the prefixes do
    /// not touch, which is why it takes no base.
    pub(crate) fn exchange_de_hl(&mut self) {
        self.regs.swap(pair::DE.0, pair::HL.0);
        self.regs.swap(pair::DE.0 + 1, pair::HL.0 + 1);
    }
}
