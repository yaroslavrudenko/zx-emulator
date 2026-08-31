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
//! | Machine cycle | T-states | Address driven |
//! |---|---|---|
//! | Opcode fetch (M1) | 4 | `PC` |
//! | Memory read or write | 3 | the address transferred |
//! | I/O port read or write | 4 | the port |
//! | Internal operation | instruction-specific | `IR`, or the last address used |
//!
//! # Implementors should mark every method `#[inline]`
//!
//! [`crate::Cpu`] is generic over `B` and is never boxed, so these calls monomorphise —
//! but LLVM will not inline across a crate boundary unless the callee is either marked
//! `#[inline]` or reachable through LTO. Since `read` and `write` are the hottest calls
//! in the emulator, the annotation is worth the two seconds it costs.

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
