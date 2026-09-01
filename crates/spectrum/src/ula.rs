//! The ULA: the bus the Z80 sees, and the clock it runs against.
//!
//! One chip does four jobs that look unrelated and are not: it draws the screen, it stalls
//! the CPU, it reads the keyboard, and it raises the 50 Hz interrupt. Drawing and stalling
//! are the same mechanism seen from two sides — the address the ULA is fetching at T-state
//! `t` is exactly what makes a CPU wanting that bank wait.
//!
//! # Where a T-state is charged
//!
//! `crates/z80` calls the transfer method **first** and then ticks that cycle's T-states,
//! so when `fetch`/`read`/`write`/`in_port`/`out_port` runs, this clock stands at the
//! *start* of the machine cycle making the access. That is where a stall belongs: each
//! transfer prices its own contention **once**, and the ticks that follow are that cycle's
//! own T-states, already paid for. A tick arriving with no cycle outstanding is a standalone
//! internal cycle, and contends on its own account at its own frame position.
//!
//! Telling those two apart is a matter of counting, because every cycle's length is known
//! the moment it opens: an M1 fetch and a port access are four T-states, a memory read or
//! write three. That the machine can recognise an M1 fetch **at all** is what [`Bus::fetch`]
//! bought. Routed through one `read`, a four-T-state opcode fetch and a three-T-state read
//! followed by one internal cycle emit byte-identical streams — same address, same order,
//! same count — and this machine used to reconstruct the boundary by deferring a T-state and
//! seeing whether a fifth arrived. That guess was wrong for the read-modify-write family,
//! costing one contention point per execution; `docs/MACHINE.md` records the whole episode.
//!
//! # What is modelled and what is not
//!
//! | | |
//! |---|---|
//! | Memory contention | yes, per access, against the current slot map |
//! | Internal-cycle contention | yes, per T-state |
//! | I/O contention | yes, the four-case ULA port pattern |
//! | Keyboard | yes |
//! | Border colour | latched, but sampled once per frame — see [`crate::screen`] |
//! | Floating bus | **no** — an undecoded port reads [`FLOATING_BUS_BYTE`] |
//! | Speaker and `MIC` | **no** — bits 3 and 4 of a `0xFE` write are discarded |
//! | `EAR` input | **no** — bit 6 of a `0xFE` read idles low |
//!
//! The floating bus is the interesting omission. Everything needed to model it is already
//! here — the clock knows where in the fetch window it is — but the byte-to-phase mapping
//! is exactly the kind of claim `docs/MACHINE.md` says must not be guessed: there is no
//! oracle for it, and software that reads it (`Arkanoid`, `Aquaplane`) is M6's problem.
//! Returning a constant is wrong in a way that is *visible*; a plausible guess would be
//! wrong in a way that is not.

use z80::Bus;

use crate::keyboard::Keyboard;
use crate::memory::Memory;
use crate::screen::Colour;
use crate::timing::{self, Clock};

/// What the data bus reads as when nothing drives it.
///
/// A 48K floats high, which is why mode 0 and mode 1 interrupts behave alike on this
/// machine: `0xFF` decodes as `RST 38h`, the same address mode 1 vectors to.
pub const FLOATING_BUS_BYTE: u8 = 0xFF;

/// The ULA answers every port whose bit 0 is clear, and decodes no other address line.
///
/// This is why the port is written `0xFE` but responds at `0x7FFE`, `0xBFFE` and every
/// other even address — the high half is free for the keyboard to use as a row selector.
const ULA_PORT_SELECT: u16 = 0x0001;

/// Bits of a `0xFE` write that set the border colour.
const BORDER_MASK: u8 = 0x07;

/// Bits 5–7 of a `0xFE` read, which the keyboard does not drive.
///
/// Bits 5 and 7 float high. Bit 6 is the `EAR` input, low with nothing connected to the
/// tape socket — the state an issue 3 machine idles in. Software that detects issue 2
/// versus issue 3 does it by writing bit 3 or 4 and reading bit 6 back, which needs the
/// tape port M6 brings.
const UNDRIVEN_INPUT_BITS: u8 = 0xA0;

/// T-states an M1 opcode fetch occupies.
///
/// This and the two below are the published Z80 machine-cycle lengths, and they duplicate
/// `crates/z80`'s own constants, which that crate keeps private. Nothing compares the two
/// sets directly. What grades them is
/// `tests/contention_magnitude.rs::real_instructions_are_stalled_by_the_pattern_at_each_cycle_they_open`,
/// which runs real instructions through a real `Cpu<Ula>` against hand-derived totals — so a
/// wrong length here moves a figure there rather than passing silently.
const OPCODE_FETCH_CYCLE: u8 = 4;

/// T-states a memory read or write cycle occupies.
const MEMORY_CYCLE: u8 = 3;

/// T-states an I/O port cycle occupies.
const PORT_CYCLE: u8 = 4;

/// The bus the CPU is wired to: memory, ports, the keyboard, and the frame clock.
#[derive(Debug)]
pub struct Ula {
    memory: Memory,
    keyboard: Keyboard,
    clock: Clock,
    /// T-states of the machine cycle in progress whose contention is already charged.
    ///
    /// Set by whichever transfer opened the cycle and spent one per tick; a tick arriving
    /// with it at zero is a standalone internal cycle. It tracks the **CPU's** stream, so a
    /// caller driving this bus directly — a test reading a port, say — arms it without ever
    /// spending it. That costs nothing to such a caller, who is reading a value rather than
    /// measuring time, and the next transfer overwrites it.
    covered_t_states: u8,
    border: Colour,
}

impl Ula {
    /// A ULA at the start of frame zero, fronting `memory`.
    #[must_use]
    pub fn new(memory: Memory) -> Self {
        Self {
            memory,
            keyboard: Keyboard::new(),
            clock: Clock::new(),
            covered_t_states: 0,
            border: Colour::BLACK,
        }
    }

    /// Put the clock back to the start of frame zero and clear the border.
    ///
    /// Memory and the keyboard are left alone: a reset button does not clear RAM, and the
    /// ROM's own start-up clears what it relies on.
    pub fn reset(&mut self) {
        self.clock = Clock::new();
        self.covered_t_states = 0;
        self.border = Colour::BLACK;
    }

    /// The memory this ULA fronts.
    #[must_use]
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// The memory this ULA fronts, mutably.
    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }

    /// The keyboard.
    #[must_use]
    pub fn keyboard(&self) -> &Keyboard {
        &self.keyboard
    }

    /// The keyboard, mutably — how a frontend or a test presses a key.
    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        &mut self.keyboard
    }

    /// The frame clock.
    #[must_use]
    pub fn clock(&self) -> Clock {
        self.clock
    }

    /// The colour last written to the border.
    #[must_use]
    pub fn border(&self) -> Colour {
        self.border
    }

    /// Whether the ULA is holding `/INT` low right now.
    #[must_use]
    pub fn interrupt_asserted(&self) -> bool {
        self.clock.interrupt_asserted()
    }

    /// Charge the stall a contended access starting *now* at `address` would suffer.
    #[inline]
    fn contend(&mut self, address: u16) {
        if self.memory.is_contended(address) {
            self.clock
                .advance(timing::delay(self.clock.frame_t_state()));
        }
    }

    /// The stall an I/O cycle at `port` suffers, in T-states.
    ///
    /// Four cases, and they are not a simplification of one rule — the ULA contends
    /// because it owns the port *and* because the address happens to be in contended
    /// memory, and the two combine:
    ///
    /// | Address contended | ULA port | Pattern |
    /// |---|---|---|
    /// | no | no | `N:4` |
    /// | no | yes | `N:1, C:3` |
    /// | yes | no | `C:1, C:1, C:1, C:1` |
    /// | yes | yes | `C:1, C:3` |
    ///
    /// `C` applies the ULA's delay unconditionally: the address test is already the row.
    /// Each stall shifts the ones after it, which is why the offsets accumulate. The whole
    /// stall is charged before the cycle's four nominal T-states arrive as ticks; the
    /// clock ends in the same place, and nothing observes it in between.
    fn port_delay(&self, port: u16) -> u32 {
        let contended_address = self.memory.is_contended(port);
        let ula_port = port & ULA_PORT_SELECT == 0;
        match (contended_address, ula_port) {
            (false, false) => 0,
            (false, true) => self.delay_after(1),
            (true, true) => {
                let first = self.delay_after(0);
                first + self.delay_after(first + 1)
            }
            (true, false) => {
                let first = self.delay_after(0);
                let second = self.delay_after(first + 1);
                let third = self.delay_after(first + second + 2);
                let fourth = self.delay_after(first + second + third + 3);
                first + second + third + fourth
            }
        }
    }

    /// The ULA delay at the frame position `offset` T-states from now.
    #[inline]
    fn delay_after(&self, offset: u32) -> u32 {
        timing::delay(self.clock.ahead(offset))
    }

    /// Open a memory machine cycle of `t_states`, charging its contention.
    #[inline]
    fn begin_memory_cycle(&mut self, address: u16, t_states: u8) {
        self.contend(address);
        self.covered_t_states = t_states;
    }

    /// Open an I/O machine cycle, charging its contention.
    #[inline]
    fn begin_port_cycle(&mut self, port: u16) {
        self.clock.advance(self.port_delay(port));
        self.covered_t_states = PORT_CYCLE;
    }
}

impl Bus for Ula {
    #[inline]
    fn fetch(&mut self, address: u16) -> u8 {
        self.begin_memory_cycle(address, OPCODE_FETCH_CYCLE);
        self.memory.read(address)
    }

    #[inline]
    fn read(&mut self, address: u16) -> u8 {
        self.begin_memory_cycle(address, MEMORY_CYCLE);
        self.memory.read(address)
    }

    #[inline]
    fn write(&mut self, address: u16, value: u8) {
        self.begin_memory_cycle(address, MEMORY_CYCLE);
        self.memory.write(address, value);
    }

    #[inline]
    fn in_port(&mut self, port: u16) -> u8 {
        self.begin_port_cycle(port);
        if port & ULA_PORT_SELECT != 0 {
            return FLOATING_BUS_BYTE;
        }
        self.keyboard.read(port) | UNDRIVEN_INPUT_BITS
    }

    #[inline]
    fn out_port(&mut self, port: u16, value: u8) {
        self.begin_port_cycle(port);
        if port & ULA_PORT_SELECT == 0 {
            // Bits 3 and 4 are `MIC` and the speaker. Discarded until M6 and M8 want them.
            self.border = Colour::new(value & BORDER_MASK);
        }
    }

    #[inline]
    fn tick(&mut self, address: u16) {
        match self.covered_t_states.checked_sub(1) {
            // Inside an open cycle: its contention was charged when the cycle opened.
            Some(remaining) => self.covered_t_states = remaining,
            // Nothing open: a standalone internal cycle, contending on its own account.
            None => self.contend(address),
        }
        self.clock.advance(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::Key;
    use crate::memory::PAGE_SIZE;
    use crate::timing::{FIRST_CONTENDED_T_STATE, T_STATES_PER_FRAME};

    /// An uncontended address in the top slot, and a contended one in the screen bank.
    const UNCONTENDED: u16 = 0x8000;
    const CONTENDED: u16 = 0x4000;

    fn ula() -> Ula {
        Ula::new(Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM"))
    }

    /// Move the clock to `frame_t_state` without going through the bus.
    fn at(frame_t_state: u32) -> Ula {
        let mut ula = ula();
        ula.clock.advance(frame_t_state);
        ula
    }

    /// One memory-read machine cycle as the core issues it: the transfer, then three ticks.
    fn read_cycle(ula: &mut Ula, address: u16) {
        ula.read(address);
        for _ in 0..3 {
            ula.tick(address);
        }
    }

    #[test]
    fn an_uncontended_read_costs_exactly_its_three_t_states() {
        let mut ula = at(FIRST_CONTENDED_T_STATE);
        read_cycle(&mut ula, UNCONTENDED);
        assert_eq!(ula.clock.frame_t_state(), FIRST_CONTENDED_T_STATE + 3);
    }

    #[test]
    fn a_contended_read_is_stalled_by_the_pattern_at_its_start() {
        for (offset, expected) in [(0, 6), (1, 5), (5, 1), (6, 0), (7, 0)] {
            let start = FIRST_CONTENDED_T_STATE + offset;
            let mut ula = at(start);
            read_cycle(&mut ula, CONTENDED);
            assert_eq!(
                ula.clock.frame_t_state(),
                start + expected + 3,
                "a read starting at +{offset} should stall {expected}"
            );
        }
    }

    #[test]
    fn contention_outside_the_fetch_window_is_free() {
        let mut ula = at(FIRST_CONTENDED_T_STATE - 1);
        read_cycle(&mut ula, CONTENDED);
        assert_eq!(ula.clock.frame_t_state(), FIRST_CONTENDED_T_STATE + 2);
    }

    #[test]
    fn an_opcode_fetch_contends_once_and_not_four_times() {
        // Four ticks at the fetch address are one machine cycle, not four internal ones.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.fetch(CONTENDED);
        for _ in 0..4 {
            ula.tick(CONTENDED);
        }
        assert_eq!(
            ula.clock.frame_t_state(),
            start + 6 + 4,
            "delay(+0)=6 for the fetch's stall, then its four T-states and nothing else"
        );
    }

    #[test]
    fn a_fetch_and_a_read_charge_the_same_tick_stream_differently() {
        // What `Bus::fetch` bought, as the one assertion that could not be written before
        // it existed. Both runs are a transfer followed by four ticks at one address —
        // byte-identical streams. An M1 cycle is four T-states, so all four are covered; a
        // memory read is three, so the fourth tick is an internal cycle and contends.
        let start = FIRST_CONTENDED_T_STATE;

        let mut fetched = at(start);
        fetched.fetch(CONTENDED);
        for _ in 0..4 {
            fetched.tick(CONTENDED);
        }
        assert_eq!(fetched.clock.frame_t_state(), start + 6 + 4);

        let mut read = at(start);
        read.read(CONTENDED);
        for _ in 0..4 {
            read.tick(CONTENDED);
        }
        //   read   at +0 -> delay(0)=6, clock +6, three T-states -> +9
        //   tick 4 at +9 -> internal, delay(9)=5, clock +14, +1 -> +15
        assert_eq!(
            read.clock.frame_t_state(),
            start + 15,
            "the read's fourth tick is an internal cycle and must be charged"
        );
    }

    #[test]
    fn internal_cycles_after_a_read_each_contend() {
        // The `JR` shape: read the displacement, then five internal cycles at the same
        // address. Six contention points, priced at six successive positions.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.read(CONTENDED);
        for _ in 0..8 {
            ula.tick(CONTENDED);
        }

        // Priced by hand from the pattern, one stall at a time:
        //   read   at +0  -> 6, clock +6, three T-states -> +9
        //   tick 4 at +9  -> (9)%8=1 -> 5, clock +14, +1 -> +15
        //   tick 5 at +15 -> (15)%8=7 -> 0, +1 -> +16
        //   tick 6 at +16 -> 6, clock +22, +1 -> +23
        //   tick 7 at +23 -> (23)%8=7 -> 0, +1 -> +24
        //   tick 8 at +24 -> 6, +1 -> +31
        assert_eq!(ula.clock.frame_t_state(), start + 31);
    }

    #[test]
    fn internal_cycles_on_an_uncontended_refresh_address_are_free() {
        // `ADD HL,BC` executing from contended memory: the fetch stalls, the seven
        // internal cycles sit on the refresh address, which points into the ROM.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.fetch(CONTENDED);
        for _ in 0..4 {
            ula.tick(CONTENDED);
        }
        for _ in 0..7 {
            ula.tick(0x3F00);
        }
        assert_eq!(ula.clock.frame_t_state(), start + 6 + 11);
    }

    #[test]
    fn a_write_cycle_is_three_t_states_and_its_internal_cycles_are_charged() {
        // `LDIR`: write, then two internal cycles at the write address. A write is never
        // four T-states, so nothing here is deferred.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.write(CONTENDED, 0);
        for _ in 0..5 {
            ula.tick(CONTENDED);
        }
        //   write  at +0 -> 6, clock +6, three T-states -> +9
        //   tick 4 at +9 -> (9)%8=1 -> 5, clock +14, +1 -> +15
        //   tick 5 at +15 -> (15)%8=7 -> 0, +1 -> +16
        assert_eq!(ula.clock.frame_t_state(), start + 16);
    }

    #[test]
    fn the_read_modify_write_internal_cycle_is_charged() {
        // The half of `INC (HL)` that this machine used to get wrong: a read, one internal
        // cycle at the address just read, then the write-back. Under the deferral heuristic
        // that internal cycle was indistinguishable from an opcode fetch's fourth T-state
        // and its stall was dropped, which cost one contention point on every execution of
        // the whole read-modify-write family. All three cycles must now be charged.
        let start = FIRST_CONTENDED_T_STATE;
        let mut ula = at(start);
        ula.read(CONTENDED);
        for _ in 0..3 {
            ula.tick(CONTENDED);
        }
        ula.tick(CONTENDED);
        ula.write(CONTENDED, 0);
        for _ in 0..3 {
            ula.tick(CONTENDED);
        }
        //   read     at +0  -> delay(0)=6, clock +6, three T-states -> +9
        //   internal at +9  -> delay(9)=5, clock +14, one T-state   -> +15
        //   write    at +15 -> delay(15)=0, three T-states          -> +18
        //
        // The heuristic reached +17 by dropping the stall at +9 and then charging the write
        // four T-states early at +10, where the pattern happens to stall 4 — so the visible
        // error was one T-state, not the five the missing stall suggests. Each stall shifts
        // the ones after it, which is why a missing one cannot be added to the total.
        assert_eq!(ula.clock.frame_t_state(), start + 18);
    }

    #[test]
    fn the_four_port_contention_cases_are_all_distinct() {
        // Priced by hand from the published table, at a frame position where the pattern
        // starts at 6. `C:n` stalls, then n T-states pass, so each stall shifts the next.
        //
        //   N:4                  0x8001  0
        //   N:1, C:3             0x8000  delay(+1)=5
        //   C:1, C:3             0x4000  delay(+0)=6, then delay(+7)=0
        //   C:1, C:1, C:1, C:1   0x4001  6, delay(+7)=0, delay(+8)=6, delay(+15)=0
        let start = FIRST_CONTENDED_T_STATE;
        let cases = [(0x8001_u16, 0_u32), (0x8000, 5), (0x4000, 6), (0x4001, 12)];
        for (port, stall) in cases {
            let mut ula = at(start);
            ula.in_port(port);
            for _ in 0..4 {
                ula.tick(port);
            }
            assert_eq!(
                ula.clock.frame_t_state(),
                start + stall + 4,
                "port {port:#06X} should stall {stall} T-states"
            );
        }
    }

    #[test]
    fn reading_the_keyboard_port_reports_the_selected_half_row() {
        let mut ula = ula();
        ula.keyboard_mut().press(Key::Enter);
        assert_eq!(ula.in_port(0xBFFE), UNDRIVEN_INPUT_BITS | 0x1E);
        assert_eq!(ula.in_port(0xFEFE), UNDRIVEN_INPUT_BITS | 0x1F);
    }

    #[test]
    fn an_undecoded_port_reads_as_a_floating_bus() {
        let mut ula = ula();
        assert_eq!(ula.in_port(0xFFFF), FLOATING_BUS_BYTE);
    }

    #[test]
    fn writing_the_ula_port_latches_the_border_colour() {
        let mut ula = ula();
        assert_eq!(ula.border(), Colour::BLACK);
        // Bits above 2 are MIC, the speaker, and nothing.
        ula.out_port(0x00FE, 0xF8 | 5);
        assert_eq!(ula.border(), Colour::new(5));
    }

    #[test]
    fn writing_a_port_the_ula_does_not_own_leaves_the_border_alone() {
        let mut ula = ula();
        ula.out_port(0x00FE, 2);
        ula.out_port(0x7FFD, 5);
        assert_eq!(ula.border(), Colour::new(2));
    }

    #[test]
    fn the_clock_is_the_bus_and_it_rolls_over_mid_instruction() {
        // MACHINE.md Decision 2: nothing stops a machine cycle on a frame boundary.
        let mut ula = at(T_STATES_PER_FRAME - 2);
        read_cycle(&mut ula, UNCONTENDED);
        assert_eq!(ula.clock.frames(), 1);
        assert_eq!(ula.clock.frame_t_state(), 1);
    }

    #[test]
    fn reset_returns_the_clock_to_the_start_without_disturbing_memory() {
        let mut ula = ula();
        ula.memory_mut().write(0x8000, 0xA5);
        ula.clock.advance(1234);
        ula.out_port(0x00FE, 3);
        ula.reset();
        assert_eq!(ula.clock, Clock::new());
        assert_eq!(ula.border(), Colour::BLACK);
        assert_eq!(ula.memory().read(0x8000), 0xA5);
    }
}
