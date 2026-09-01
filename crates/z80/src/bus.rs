//! The CPU's only window onto the outside world.
//!
//! The Z80 core owns no memory and no I/O. Everything it touches goes through [`Bus`],
//! which the machine crate implements over its own paged memory, ULA and ports. That is
//! the decision the rest of the design hangs on: the CPU stays a pure state machine that
//! can be tested against a flat 64K array, and the Spectrum's paging, contention and
//! floating bus live entirely on the other side of this trait.
//!
//! # Timing is part of the contract
//!
//! [`Bus::tick`] is called **inside** an instruction, once per machine cycle, not once
//! with the instruction's total at the end. Spectrum memory contention depends on *when
//! in the frame* an access happens, so a machine that only learned an instruction's total
//! duration could never place the stalls correctly.
//!
//! The core calls the transfer method first and then ticks that cycle's T-states, so an
//! implementation observes its own clock positioned at the **start** of the machine cycle
//! performing the access. A contention model reads the current T-state, works out the stall
//! the ULA would impose at that moment, and adds it to its own clock.
//!
//! The nominal durations the core uses are the published Z80 machine-cycle lengths, each
//! delivered as that many separate [`Bus::tick`] calls:
//!
//! | Machine cycle | T-states | Address driven | Transfer method |
//! |---|---|---|---|
//! | Opcode fetch (M1) | [`OPCODE_FETCH_T_STATES`] | `PC` | [`Bus::fetch`] |
//! | Memory read or write | [`MEMORY_ACCESS_T_STATES`] | the address transferred | [`Bus::read`] / [`Bus::write`] |
//! | I/O port read or write | [`PORT_ACCESS_T_STATES`] | the port | [`Bus::in_port`] / [`Bus::out_port`] |
//! | Internal operation | instruction-specific | `IR`, or the last address used | none |
//!
//! That last column is what makes a cycle's *length* knowable from the call stream. Without
//! it a three-T-state read followed by one internal cycle is indistinguishable from a
//! four-T-state opcode fetch — see [`Bus::fetch`], which exists for exactly that reason.
//!
//! The three lengths are **exported**, not merely tabulated, and the reason is that the
//! table above is not documentation of an internal choice — it is the **decoding key for
//! the call stream**. An implementation cannot honour the contract without them: to tell a
//! tick that belongs to the cycle a transfer just opened from a standalone internal cycle
//! that must contend on its own account, it has to know how long that cycle is. A
//! contention model that guesses instead has already been written here once, and
//! `docs/MACHINE.md` records what the guess cost. So these are part of what the trait
//! promises rather than an implementation detail a machine happens to need, and a machine
//! that had to re-transcribe them would be keeping a second copy of the crate's own
//! knowledge — with nothing to notice if the two ever disagreed.
//!
//! # Implementors should mark every method `#[inline]`
//!
//! [`crate::Cpu`] is generic over `B` and is never boxed, so these calls monomorphise —
//! but LLVM will not inline across a crate boundary unless the callee is either marked
//! `#[inline]` or reachable through LTO. Since `fetch`, `read` and `write` are the hottest
//! calls in the emulator, the annotation is worth the two seconds it costs.

/// T-states an opcode fetch (M1) occupies, delivered as that many [`Bus::tick`] calls.
///
/// One of the three published Z80 machine-cycle lengths that make a cycle's *length*
/// knowable from the call stream — see the module documentation for why they are part of
/// this trait's contract rather than an internal constant.
pub const OPCODE_FETCH_T_STATES: u8 = 4;

/// T-states a memory read or write cycle occupies.
///
/// See [`OPCODE_FETCH_T_STATES`].
pub const MEMORY_ACCESS_T_STATES: u8 = 3;

/// T-states an I/O port read or write cycle occupies.
///
/// See [`OPCODE_FETCH_T_STATES`].
pub const PORT_ACCESS_T_STATES: u8 = 4;

/// Memory and I/O as seen by the Z80, plus the clock the CPU advances as it runs.
///
/// Every method takes `&mut self`: reads are not pure on a real machine. A Spectrum's
/// floating bus returns different bytes depending on where the ULA's raster is, and
/// reading a contended address can itself cost time, so even `read` may need to mutate
/// the machine's state.
pub trait Bus {
    /// Read one byte from the address space.
    ///
    /// Called once per memory-read machine cycle, immediately before the matching
    /// [`Bus::tick`].
    fn read(&mut self, addr: u16) -> u8;

    /// Read the opcode byte of an M1 cycle.
    ///
    /// Defaults to [`Bus::read`], so implementing it is optional and every existing
    /// implementation keeps working unchanged.
    ///
    /// # Why it is a separate method
    ///
    /// M1 is the one machine cycle whose length cannot be inferred from the call stream. A
    /// write is three T-states and a port access is four, but a read is three for an operand
    /// and **four** for an opcode fetch. Routed through one method, `LD A,B` — a single
    /// four-T-state M1 cycle — and the read-modify half of `INC (HL)` — a three-T-state read
    /// then one internal cycle — emit byte-identical streams: one transfer callback followed
    /// by four ticks at the same address. A contention model owes **one** stall for the first
    /// and **two** for the second, and nothing in the stream says which it is looking at.
    ///
    /// # When it is called
    ///
    /// Once per M1 cycle that reads memory, which during [`crate::Cpu::step`] is also exactly
    /// once per `R` increment — prefix bytes included, since `DD`, `FD`, `CB` and `ED` are
    /// each their own M1 cycle with their own refresh.
    ///
    /// Three neighbours are deliberately **not** fetches:
    ///
    /// - a `DDCB`/`FDCB` instruction's displacement and opcode bytes, which the hardware
    ///   takes with ordinary three-T-state memory reads — `R` advances twice across those
    ///   four bytes, not four times;
    /// - every operand, data and stack read, which is what [`Bus::read`] now means
    ///   exclusively;
    /// - an interrupt acknowledge, whose byte comes from the device on the data bus rather
    ///   than from memory. That cycle refreshes `R` with no call here at all, and is the one
    ///   place the fetch-per-refresh correspondence does not hold.
    #[inline]
    fn fetch(&mut self, addr: u16) -> u8 {
        self.read(addr)
    }

    /// Write one byte to the address space.
    ///
    /// Called once per memory-write machine cycle, immediately before the matching
    /// [`Bus::tick`].
    fn write(&mut self, addr: u16, val: u8);

    /// Read one byte from an I/O port.
    ///
    /// The Z80 places the full 16 bits on the address bus. `IN A,(n)` forms the port as
    /// `A` in the high byte and the immediate operand in the low byte, which is why
    /// Spectrum peripherals decode on the high half as well as the low.
    fn in_port(&mut self, port: u16) -> u8;

    /// Write one byte to an I/O port.
    ///
    /// The same 16-bit port addressing applies as for [`Bus::in_port`].
    fn out_port(&mut self, port: u16, val: u8);

    /// Advance the machine's clock by exactly one T-state, with `addr` on the bus.
    ///
    /// Called once per T-state and never batched. Contention is a function of where in the
    /// frame an access falls, so seven separate one-T-state cycles are not the same thing
    /// as one seven-T-state block — batching them would collapse seven contention points
    /// into one and put every subsequent access at the wrong frame position.
    ///
    /// `addr` is the address the Z80 actually drives during that T-state. For the cycles
    /// that follow an opcode fetch that is the refresh address `IR`, which is why a
    /// contended machine cannot reconstruct it from the transfer addresses alone.
    fn tick(&mut self, addr: u16);
}
