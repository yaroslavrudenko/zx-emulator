//! A cycle-accurate Zilog Z80 core.
//!
//! The CPU owns no memory and no I/O. Everything it touches goes through the [`Bus`]
//! trait, which the machine crate implements; see `docs/ARCHITECTURE.md` for why that
//! separation is the decision the rest of the emulator hangs on.
//!
//! ```
//! use z80::{Bus, Cpu, CpuState};
//!
//! # struct FlatRam([u8; 0x1_0000]);
//! # impl Bus for FlatRam {
//! #     fn read(&mut self, addr: u16) -> u8 { self.0[usize::from(addr)] }
//! #     fn write(&mut self, addr: u16, val: u8) { self.0[usize::from(addr)] = val; }
//! #     fn in_port(&mut self, _port: u16) -> u8 { 0xFF }
//! #     fn out_port(&mut self, _port: u16, _val: u8) {}
//! #     fn tick(&mut self, _addr: u16) {}
//! # }
//! let mut cpu = Cpu::new(FlatRam([0; 0x1_0000]));
//! cpu.set_state(CpuState { pc: 0x8000, sp: 0xFF00, ..CpuState::default() });
//!
//! let t_states = cpu.step();
//! assert_eq!(cpu.state().pc, 0x8001); // a NOP out of blank memory
//! assert_eq!(t_states, 4);
//! ```
//!
//! # Scope
//!
//! This is milestone M1: the un-prefixed opcodes, `0x00`–`0xFF` less the four prefix
//! bytes. The `CB`, `DD`, `ED` and `FD` prefixes neither panic nor silently do nothing —
//! both of which would turn a missing instruction into a bug that takes a week to find.
//! They record a [`StepError`] readable through [`Cpu::fault`].
//!
//! Interrupt acceptance *is* implemented ([`Cpu::interrupt`], [`Cpu::nmi`]), because
//! leaving `HALT` requires pushing a return address and dispatching through the interrupt
//! mode — work only the core can do. Deciding *when* to accept still belongs to the
//! machine, which is what owns the interrupt line.
//!
//! # Timing
//!
//! T-states are charged where they are spent. Every memory and I/O access calls
//! [`Bus::tick`] for its own machine cycle, and cycles in which the Z80 computes rather
//! than transfers are charged explicitly. [`Cpu::step`] returns the total as a
//! convenience, but the authoritative account is the sequence of `tick` calls.
//!
//! # Known deviations from real silicon
//!
//! - **`SCF`/`CCF` undocumented bits.** They are taken from the accumulator. A real NMOS
//!   Z80 ORs in the flag latch left by the previous instruction — the latch is recorded
//!   here as [`CpuState::q`], but the rule that consumes it is not implemented, because
//!   `zexall` at M4 adjudicates two genuinely contested cases (`POP AF` and `EX AF,AF'`).
//! - **`MEMPTR`** ([`CpuState::wz`]) is carried through snapshots but never updated; it
//!   becomes observable only through `BIT n,(HL)` and the `ED` block operations, both M2.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

mod bus;
mod decode;
mod instructions;
mod registers;

pub(crate) mod flags;

pub use bus::Bus;

use registers::{Registers, index, pair};

/// T-states in an opcode fetch (M1).
const OPCODE_FETCH: u8 = 4;

/// T-states in a memory read or write.
const MEMORY_ACCESS: u8 = 3;

/// T-states in an I/O port read or write.
const PORT_ACCESS: u8 = 4;

/// T-states in a maskable-interrupt acknowledge cycle: M1 stretched by two wait states.
const INTERRUPT_ACKNOWLEDGE: u8 = 7;

/// T-states in a non-maskable-interrupt acknowledge cycle.
const NMI_ACKNOWLEDGE: u8 = 5;

/// Where a non-maskable interrupt always vectors.
const NMI_VECTOR: u16 = 0x0066;

/// Where a mode 1 interrupt always vectors — the Spectrum's 50 Hz frame interrupt.
const MODE_1_VECTOR: u16 = 0x0038;

/// A condition that stopped an instruction short.
///
/// This is a permanent diagnostic channel, not M1 scaffolding. The prefix variant
/// disappears when M2 implements the prefixes, but an `ED` opcode with no defined meaning
/// can still be reached at any milestone, and a machine wants somewhere to report it other
/// than a panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum StepError {
    /// A `CB`, `DD`, `ED` or `FD` prefix was fetched. Those instruction sets land in M2.
    ///
    /// The prefix's own opcode fetch has already happened by the time this is recorded:
    /// four T-states are charged, `PC` has advanced past the prefix byte and `R` has been
    /// incremented, exactly as on hardware. `address` names the prefix byte itself.
    #[error("opcode prefix {prefix:#04X} at {address:#06X} is not implemented yet")]
    UnsupportedPrefix {
        /// The prefix byte that was fetched.
        prefix: u8,
        /// The address the prefix byte was fetched from.
        address: u16,
    },

    /// A mode 0 interrupt supplied a byte this core cannot execute.
    ///
    /// In mode 0 the interrupting device places an instruction on the data bus. Only the
    /// `RST` family is supported, which is what real hardware puts there — and what a
    /// Spectrum's floating bus produces, since `0xFF` is `RST 38h`.
    #[error("interrupt mode 0 supplied {opcode:#04X}, which is not an RST instruction")]
    UnsupportedInterruptOpcode {
        /// The byte the device placed on the bus.
        opcode: u8,
    },
}

/// A value outside the three interrupt modes the Z80 defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("interrupt mode {0} is not one of 0, 1 or 2")]
pub struct InvalidInterruptMode(
    /// The rejected value.
    pub u8,
);

/// How the Z80 responds when it accepts an interrupt.
///
/// Modelled as an enum rather than a raw byte so that the machine's dispatch can match
/// exhaustively: there is no fourth mode, and code that handles interrupts should not need
/// a fallback arm for a value the hardware cannot hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InterruptMode {
    /// Mode 0 — the interrupting device puts an instruction on the data bus and the CPU
    /// executes it. The reset default.
    #[default]
    Mode0,
    /// Mode 1 — a restart to `0x0038`, ignoring the device entirely. This is what the
    /// ZX Spectrum's 50 Hz frame interrupt uses.
    Mode1,
    /// Mode 2 — an indirect vector, formed from `I` as the high byte and a byte the device
    /// supplies as the low byte.
    Mode2,
}

impl TryFrom<u8> for InterruptMode {
    type Error = InvalidInterruptMode;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Mode0),
            1 => Ok(Self::Mode1),
            2 => Ok(Self::Mode2),
            other => Err(InvalidInterruptMode(other)),
        }
    }
}

impl From<InterruptMode> for u8 {
    fn from(mode: InterruptMode) -> Self {
        match mode {
            InterruptMode::Mode0 => 0,
            InterruptMode::Mode1 => 1,
            InterruptMode::Mode2 => 2,
        }
    }
}

/// The complete architectural state of the CPU.
///
/// One struct rather than a wall of accessors, because this is not a convenience for
/// tests: it is precisely what a `.z80` or `.sna` snapshot file carries, so loading and
/// saving a snapshot is [`Cpu::set_state`] and [`Cpu::state`] with a serialiser either
/// side. Every combination of these values is a state a real Z80 can be in, so the fields
/// are public and carry no invariants to protect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuState {
    /// Accumulator and flags.
    pub af: u16,
    /// The `BC` pair.
    pub bc: u16,
    /// The `DE` pair.
    pub de: u16,
    /// The `HL` pair.
    pub hl: u16,
    /// The shadow accumulator and flags, reached by `EX AF,AF'`.
    pub af_shadow: u16,
    /// The shadow `BC'` pair, reached by `EXX`.
    pub bc_shadow: u16,
    /// The shadow `DE'` pair, reached by `EXX`.
    pub de_shadow: u16,
    /// The shadow `HL'` pair, reached by `EXX`.
    pub hl_shadow: u16,
    /// The `IX` index register.
    pub ix: u16,
    /// The `IY` index register.
    pub iy: u16,
    /// The stack pointer.
    pub sp: u16,
    /// The program counter.
    pub pc: u16,
    /// The interrupt vector register, supplying the high byte of a mode 2 vector.
    pub i: u8,
    /// The memory refresh register. Bits 0–6 increment on every opcode fetch and bit 7
    /// does not, which is why software uses it as a cheap source of pseudo-randomness.
    pub r: u8,
    /// Whether maskable interrupts are enabled.
    pub iff1: bool,
    /// The saved copy of `iff1` that a non-maskable interrupt preserves and `RETN`
    /// restores.
    pub iff2: bool,
    /// How an accepted interrupt is dispatched.
    pub im: InterruptMode,
    /// Whether the CPU is halted.
    ///
    /// Nothing enforces that `pc` names the `HALT` opcode the CPU stopped on, and
    /// [`Cpu::interrupt`] resumes at `pc + 1` — correct only if it does. Legitimate
    /// `.z80` and `.sna` snapshots always satisfy that, because `HALT` holds `PC` on
    /// itself; a hand-built state that sets this flag with `pc` pointing elsewhere will
    /// resume one byte past whatever it does point at.
    pub halted: bool,
    /// `MEMPTR` (often written `WZ`) — an internal address latch with no instruction that
    /// reads it directly.
    ///
    /// It leaks into observable behaviour through the undocumented flag bits of
    /// `BIT n,(HL)` and the `ED` block operations, which is why `zexall` can see it and
    /// why a snapshot must carry it. Nothing in the un-prefixed set updates it, so it stays
    /// zero until M2.
    pub wz: u16,
    /// The flag latch — the value most recently written to `F`, or zero if the last
    /// instruction wrote no flags.
    ///
    /// A real NMOS Z80 ORs this into the undocumented bits of `SCF` and `CCF`. The latch is
    /// maintained here so the rule is a two-line change when `zexall` settles it at M4; the
    /// rule itself is deliberately not implemented yet.
    pub q: u8,
}

impl Default for CpuState {
    /// The state a Z80 is in after `RESET`.
    ///
    /// `PC`, `I` and `R` are cleared, interrupts are disabled and mode 0 is selected. `AF`
    /// and `SP` come up holding `0xFFFF`, which is worth reproducing rather than zeroing: a
    /// program that reads `SP` before setting it then sees what it would see on real
    /// hardware.
    fn default() -> Self {
        Self {
            af: u16::MAX,
            bc: 0,
            de: 0,
            hl: 0,
            af_shadow: 0,
            bc_shadow: 0,
            de_shadow: 0,
            hl_shadow: 0,
            ix: 0,
            iy: 0,
            sp: u16::MAX,
            pc: 0,
            i: 0,
            r: 0,
            iff1: false,
            iff2: false,
            im: InterruptMode::Mode0,
            halted: false,
            wz: 0,
            q: 0,
        }
    }
}

/// The interrupt system's live state: two flip-flops, the mode, and `EI`'s deferral.
///
/// `ei_pending` is not part of [`CpuState`] because it is not architectural state a
/// snapshot format records — it is a one-instruction window internal to the core.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct InterruptState {
    iff1: bool,
    iff2: bool,
    mode: InterruptMode,
    ei_pending: bool,
}

/// A Z80 CPU wired to a bus.
///
/// Generic over the bus rather than holding a trait object: the core is monomorphised into
/// its machine, so there is no indirect call on the execute path and the bus accessors
/// inline.
#[derive(Debug, Clone)]
pub struct Cpu<B: Bus> {
    regs: Registers,
    bus: B,
    interrupts: InterruptState,
    halted: bool,
    /// `MEMPTR`. Carried through snapshots; not yet updated by any instruction.
    wz: u16,
    /// The flag latch. See [`CpuState::q`].
    q: u8,
    /// T-states charged during the instruction currently executing.
    t_states: u8,
    /// What stopped the most recent [`Cpu::step`] short, if anything.
    fault: Option<StepError>,
}

/// Where an accepted interrupt vectors.
enum InterruptDispatch {
    /// Modes 0 and 1 both jump to an address known before the sequence starts.
    Fixed(u16),
    /// Mode 2 reads its target from the table `I` points at — after the return address is
    /// pushed, so it cannot be resolved in advance.
    Vectored(u16),
}

/// The address an `RST` opcode restarts to, or `None` if the byte is not an `RST`.
///
/// `RST` is `11 ttt 111`, and `ttt` is already scaled by eight in place.
fn restart_target(opcode: u8) -> Option<u16> {
    /// The bits that must all be set for the encoding to be an `RST`.
    const RESTART_PATTERN: u8 = 0xC7;
    /// Bits 5–3.
    const TARGET_MASK: u8 = 0x38;

    (opcode & RESTART_PATTERN == RESTART_PATTERN).then(|| u16::from(opcode & TARGET_MASK))
}

impl<B: Bus> Cpu<B> {
    /// Build a CPU in its post-reset state, taking ownership of the bus.
    pub fn new(bus: B) -> Self {
        let mut cpu = Self {
            regs: Registers::default(),
            bus,
            interrupts: InterruptState::default(),
            halted: false,
            wz: 0,
            q: 0,
            t_states: 0,
            fault: None,
        };
        cpu.set_state(CpuState::default());
        cpu
    }

    /// Execute exactly one instruction and return the T-states it took.
    ///
    /// The bus has already been ticked for each of those T-states, machine cycle by machine
    /// cycle, by the time this returns; the total is a convenience for callers that do not
    /// track time themselves.
    ///
    /// While the CPU is halted this performs one M1 cycle and no instruction: `PC` stays on
    /// the `HALT` opcode and `R` keeps advancing, exactly as the hardware does. Use
    /// [`Cpu::interrupt`] or [`Cpu::nmi`] to resume.
    ///
    /// An opcode this milestone does not implement is not a panic and not a silent no-op:
    /// the T-states genuinely consumed are returned and the reason is recorded in
    /// [`Cpu::fault`], which a caller that cares can check after each step.
    pub fn step(&mut self) -> u32 {
        self.begin_operation();

        if self.halted {
            self.halt_cycle();
            return u32::from(self.t_states);
        }

        let opcode = self.fetch_opcode();
        // `Result::err` keeps the diagnosis and discards the unit success — the error is
        // recorded for the caller, never dropped.
        self.fault = self.execute(opcode, pair::HL).err();

        u32::from(self.t_states)
    }

    /// Offer a maskable interrupt, returning the T-states it consumed.
    ///
    /// Returns zero — having done nothing — when the CPU will not accept one, which is the
    /// case while `iff1` is clear or while `EI`'s one-instruction deferral is still open.
    /// Putting that test here rather than in the machine keeps the acceptance rule in one
    /// place; a machine can call this unconditionally whenever its interrupt line is
    /// asserted.
    ///
    /// `data` is the byte the interrupting device places on the data bus. Mode 1 ignores
    /// it, mode 2 uses it as the low half of the vector address, and mode 0 executes it as
    /// an instruction. A Spectrum's bus floats to `0xFF`, which is `RST 38h` — the reason
    /// modes 0 and 1 behave alike on that machine.
    pub fn interrupt(&mut self, data: u8) -> u32 {
        // Every reason to decline is settled before anything changes. An interrupt is
        // accepted whole or not at all: a half-accepted one clears `iff1`, strands a
        // return address on the stack and leaves `PC` unmoved, so the interrupted
        // instruction runs again and each retry leaks two more bytes of stack — driven by
        // a device the machine does not control.
        if !self.interrupts.iff1 || self.interrupts.ei_pending {
            return 0;
        }
        let Some(dispatch) = self.resolve_dispatch(data) else {
            self.fault = Some(StepError::UnsupportedInterruptOpcode { opcode: data });
            return 0;
        };

        self.begin_operation();
        self.leave_halt();
        // Accepting a maskable interrupt disables both flip-flops; `EI` in the handler is
        // what re-enables them.
        self.interrupts.iff1 = false;
        self.interrupts.iff2 = false;
        self.acknowledge(INTERRUPT_ACKNOWLEDGE);

        let return_address = self.regs.pc();
        self.push_word(return_address);

        match dispatch {
            InterruptDispatch::Fixed(target) => self.regs.set_pc(target),
            InterruptDispatch::Vectored(pointer) => {
                let low = self.read_byte(pointer);
                let high = self.read_byte(pointer.wrapping_add(1));
                self.regs.set_pc(u16::from_le_bytes([low, high]));
            }
        }

        u32::from(self.t_states)
    }

    /// Raise a non-maskable interrupt, returning the T-states it consumed.
    ///
    /// An NMI is always accepted. It saves `iff1` into `iff2` so that `RETN` can restore
    /// it, then vectors to `0x0066`.
    pub fn nmi(&mut self) -> u32 {
        self.begin_operation();
        self.leave_halt();
        self.interrupts.iff2 = self.interrupts.iff1;
        self.interrupts.iff1 = false;
        self.acknowledge(NMI_ACKNOWLEDGE);

        let return_address = self.regs.pc();
        self.push_word(return_address);
        self.regs.set_pc(NMI_VECTOR);

        u32::from(self.t_states)
    }

    /// What stopped the most recent operation short, if anything.
    ///
    /// Cleared at the start of every operation the CPU actually performs — a step, an
    /// accepted interrupt, an NMI — so it always describes the latest one. An interrupt
    /// offer that is *declined* performs no work and leaves the previous value in place,
    /// except in mode 0, where the reason for declining is itself recorded here.
    ///
    /// It is diagnostic, not architectural: [`Cpu::set_state`] does not touch it, because
    /// loading a snapshot is not an operation and no snapshot format carries a fault.
    pub fn fault(&self) -> Option<StepError> {
        self.fault
    }

    /// The complete architectural state.
    pub fn state(&self) -> CpuState {
        CpuState {
            af: self.regs.pair(pair::AF),
            bc: self.regs.pair(pair::BC),
            de: self.regs.pair(pair::DE),
            hl: self.regs.pair(pair::HL),
            af_shadow: self.regs.pair(pair::AF_SHADOW),
            bc_shadow: self.regs.pair(pair::BC_SHADOW),
            de_shadow: self.regs.pair(pair::DE_SHADOW),
            hl_shadow: self.regs.pair(pair::HL_SHADOW),
            ix: self.regs.pair(pair::IX),
            iy: self.regs.pair(pair::IY),
            sp: self.regs.sp(),
            pc: self.regs.pc(),
            i: self.regs.get(index::I),
            r: self.regs.get(index::R),
            iff1: self.interrupts.iff1,
            iff2: self.interrupts.iff2,
            im: self.interrupts.mode,
            halted: self.halted,
            wz: self.wz,
            q: self.q,
        }
    }

    /// Replace the complete architectural state.
    ///
    /// `EI`'s one-instruction deferral is cleared: it belongs to an instruction that is no
    /// longer running, and carrying it across a snapshot load would suppress the first
    /// interrupt after the load.
    pub fn set_state(&mut self, state: CpuState) {
        self.regs.set_pair(pair::AF, state.af);
        self.regs.set_pair(pair::BC, state.bc);
        self.regs.set_pair(pair::DE, state.de);
        self.regs.set_pair(pair::HL, state.hl);
        self.regs.set_pair(pair::AF_SHADOW, state.af_shadow);
        self.regs.set_pair(pair::BC_SHADOW, state.bc_shadow);
        self.regs.set_pair(pair::DE_SHADOW, state.de_shadow);
        self.regs.set_pair(pair::HL_SHADOW, state.hl_shadow);
        self.regs.set_pair(pair::IX, state.ix);
        self.regs.set_pair(pair::IY, state.iy);
        self.regs.set_sp(state.sp);
        self.regs.set_pc(state.pc);
        self.regs.set(index::I, state.i);
        self.regs.set(index::R, state.r);
        self.interrupts.iff1 = state.iff1;
        self.interrupts.iff2 = state.iff2;
        self.interrupts.mode = state.im;
        self.interrupts.ei_pending = false;
        self.halted = state.halted;
        self.wz = state.wz;
        self.q = state.q;
    }

    /// Whether an interrupt must not be accepted yet because `EI` has just run.
    ///
    /// True from the moment `EI` completes until the instruction after it completes.
    /// [`Cpu::interrupt`] already honours this; the accessor exists so a machine can see
    /// why an offer was declined.
    pub fn ei_pending(&self) -> bool {
        self.interrupts.ei_pending
    }

    /// The bus this CPU owns.
    pub fn bus(&self) -> &B {
        &self.bus
    }

    /// The bus this CPU owns, mutably — how a machine reaches its own memory and
    /// peripherals once the CPU has taken ownership of them.
    pub fn bus_mut(&mut self) -> &mut B {
        &mut self.bus
    }

    // -----------------------------------------------------------------------------
    // Interrupt mechanics
    // -----------------------------------------------------------------------------

    /// Reset the bookkeeping that belongs to one operation rather than to the machine.
    ///
    /// `step`, `interrupt` and `nmi` all begin here so a fourth entry point cannot inherit
    /// a stale T-state count, a stale fault, an expired `EI` deferral or a flag latch left
    /// by the previous instruction. Three separate lines in three places is exactly how
    /// the first two of those came to be missed.
    ///
    /// `interrupt` reads `ei_pending` *before* calling this, because for that one entry
    /// point the deferral is an input to the accept/decline decision, not just state to
    /// clear.
    fn begin_operation(&mut self) {
        self.t_states = 0;
        self.interrupts.ei_pending = false;
        self.q = 0;
        self.fault = None;
    }

    /// Resume from `HALT`, if halted.
    ///
    /// `HALT` holds `PC` on its own opcode, so an accepted interrupt must step past it —
    /// otherwise the handler would return straight into the `HALT` and stop again.
    fn leave_halt(&mut self) {
        if self.halted {
            self.halted = false;
            self.regs.advance_pc();
        }
    }

    /// The interrupt acknowledge cycle: an M1 cycle, so it refreshes `R` like any other.
    ///
    /// The Z80 drives no fetch address here — the device supplies the byte — so the bus
    /// carries the refresh address, as it does through the rest of any M1 cycle.
    fn acknowledge(&mut self, t_states: u8) {
        self.regs.increment_r();
        let refresh = self.regs.refresh_address();
        self.internal_cycles(refresh, t_states);
    }

    /// Where an accepted interrupt will vector, resolved before any state changes.
    ///
    /// `None` means the offer must be declined — only mode 0 can fail, and only because
    /// the device supplied a byte this core cannot execute.
    fn resolve_dispatch(&self, data: u8) -> Option<InterruptDispatch> {
        match self.interrupts.mode {
            // Mode 0 executes the byte the device places on the bus. Only `RST` is
            // supported, which is what real hardware puts there — a Spectrum's floating
            // bus reads `0xFF`, which is `RST 38h`.
            InterruptMode::Mode0 => restart_target(data).map(InterruptDispatch::Fixed),
            InterruptMode::Mode1 => Some(InterruptDispatch::Fixed(MODE_1_VECTOR)),
            InterruptMode::Mode2 => Some(InterruptDispatch::Vectored(u16::from_be_bytes([
                self.regs.get(index::I),
                data,
            ]))),
        }
    }

    /// One machine cycle of a halted CPU.
    ///
    /// A halted Z80 has not stopped: it keeps issuing M1 cycles, refreshing memory as it
    /// goes, and executes an internal `NOP` each time. `PC` stays on the `HALT` opcode, so
    /// the byte fetched is that opcode and is deliberately discarded.
    fn halt_cycle(&mut self) {
        let address = self.regs.pc();
        self.bus.read(address);
        self.regs.increment_r();
        self.internal_cycles(address, OPCODE_FETCH);
    }

    // -----------------------------------------------------------------------------
    // Bus access
    //
    // Each wrapper performs the transfer and then charges its machine cycle, so an
    // implementation of `Bus` sees its clock positioned at the start of the cycle that is
    // accessing it. `#[inline]` is not decoration: without it LLVM will not inline these
    // across the crate boundary into the machine.
    // -----------------------------------------------------------------------------

    /// Advance the clock by one T-state with `address` on the bus.
    #[inline]
    fn tick(&mut self, address: u16) {
        // INVARIANT: the longest Z80 instruction is 23 T-states and the longest interrupt
        // acknowledge sequence 19, so a per-instruction accumulator cannot overflow a byte.
        // `overflow-checks = true` is set in every profile, so a mistake here is a loud
        // panic rather than a silent wrap.
        self.t_states += 1;
        self.bus.tick(address);
    }

    /// Charge one machine cycle: `count` T-states, each with `address` on the bus.
    ///
    /// Never a single batched call. The ULA contends per T-state, so a run of internal
    /// cycles has to reach the machine as a run.
    #[inline]
    fn internal_cycles(&mut self, address: u16, count: u8) {
        for _ in 0..count {
            self.tick(address);
        }
    }

    /// Read one byte, charging a memory-read cycle.
    #[inline]
    fn read_byte(&mut self, address: u16) -> u8 {
        let value = self.bus.read(address);
        self.internal_cycles(address, MEMORY_ACCESS);
        value
    }

    /// Write one byte, charging a memory-write cycle.
    #[inline]
    fn write_byte(&mut self, address: u16, value: u8) {
        self.bus.write(address, value);
        self.internal_cycles(address, MEMORY_ACCESS);
    }

    /// Read one byte from a port, charging an I/O cycle.
    #[inline]
    fn read_port(&mut self, port: u16) -> u8 {
        let value = self.bus.in_port(port);
        self.internal_cycles(port, PORT_ACCESS);
        value
    }

    /// Write one byte to a port, charging an I/O cycle.
    #[inline]
    fn write_port(&mut self, port: u16, value: u8) {
        self.bus.out_port(port, value);
        self.internal_cycles(port, PORT_ACCESS);
    }

    /// Fetch the next opcode: an M1 cycle, which also advances `PC` and refreshes `R`.
    ///
    /// `R` is incremented before the cycle is charged because the refresh address the Z80
    /// drives during and after M1 carries the *post*-increment value.
    #[inline]
    fn fetch_opcode(&mut self) -> u8 {
        let address = self.regs.pc();
        let opcode = self.bus.read(address);
        self.regs.increment_r();
        self.internal_cycles(address, OPCODE_FETCH);
        self.regs.advance_pc();
        opcode
    }

    /// Fetch one operand byte from the instruction stream.
    #[inline]
    fn fetch_byte(&mut self) -> u8 {
        let address = self.regs.pc();
        let value = self.read_byte(address);
        self.regs.advance_pc();
        value
    }

    /// Fetch one operand byte as a signed relative displacement.
    #[inline]
    fn fetch_signed_byte(&mut self) -> i8 {
        i8::from_ne_bytes([self.fetch_byte()])
    }

    /// Fetch a 16-bit operand. The Z80 stores words low byte first.
    #[inline]
    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte();
        let high = self.fetch_byte();
        u16::from_le_bytes([low, high])
    }

    /// Write `F` and record the flag latch.
    ///
    /// Every instruction that computes flags goes through here rather than touching the
    /// register directly, so the latch has exactly one writer.
    #[inline]
    fn write_flags(&mut self, flags: u8) {
        self.regs.set_f(flags);
        self.q = flags;
    }

    /// The carry flag, which several instruction classes take as an input.
    fn carry_flag(&self) -> bool {
        (self.regs.f() & flags::CARRY) != 0
    }

    /// Build the fault for a prefix byte this milestone does not implement.
    fn unsupported_prefix(&self, prefix: u8) -> StepError {
        StepError::UnsupportedPrefix {
            prefix,
            // The prefix's own fetch has already advanced PC past it.
            address: self.regs.pc().wrapping_sub(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte every port read returns, distinctive enough to be recognisable.
    const PORT_INPUT: u8 = 0x5A;

    /// 64K of RAM and a running T-state total — the least a Z80 needs to run.
    ///
    /// The conformance suites in `tests/` drive the core through their own bus; this one
    /// exists so the core's *internal* accounting can be asserted without a corpus on disk.
    struct Ram {
        memory: Vec<u8>,
        t_states: u32,
        /// The address driven during each T-state, in order — the stream a contended
        /// machine sees.
        tick_addresses: Vec<u16>,
        /// Every address MREQ was actually asserted for, in order.
        reads: Vec<u16>,
        last_port: Option<u16>,
        last_port_write: Option<u8>,
    }

    impl Ram {
        fn new(program: &[u8]) -> Self {
            let mut memory = vec![0; 0x1_0000];
            memory[..program.len()].copy_from_slice(program);
            Self {
                memory,
                t_states: 0,
                tick_addresses: Vec::new(),
                reads: Vec::new(),
                last_port: None,
                last_port_write: None,
            }
        }

        fn peek(&self, address: u16) -> u8 {
            self.memory[usize::from(address)]
        }
    }

    impl Bus for Ram {
        fn read(&mut self, addr: u16) -> u8 {
            self.reads.push(addr);
            self.memory[usize::from(addr)]
        }

        fn write(&mut self, addr: u16, val: u8) {
            self.memory[usize::from(addr)] = val;
        }

        fn in_port(&mut self, port: u16) -> u8 {
            self.last_port = Some(port);
            PORT_INPUT
        }

        fn out_port(&mut self, port: u16, val: u8) {
            self.last_port = Some(port);
            self.last_port_write = Some(val);
        }

        fn tick(&mut self, addr: u16) {
            self.t_states += 1;
            self.tick_addresses.push(addr);
        }
    }

    /// Assemble `program` at address zero and run one instruction from `state`.
    fn run_one(program: &[u8], state: CpuState) -> (u32, Cpu<Ram>) {
        let mut cpu = Cpu::new(Ram::new(program));
        cpu.set_state(state);
        let t_states = cpu.step();
        // The returned total and the total the bus accumulated must agree, or the core is
        // reporting time it never charged.
        assert_eq!(
            t_states,
            cpu.bus().t_states,
            "reported total vs ticked total"
        );
        (t_states, cpu)
    }

    /// A starting state with a usable stack and all flags clear.
    fn ready() -> CpuState {
        CpuState {
            af: 0,
            sp: 0xFFFF,
            ..CpuState::default()
        }
    }

    /// [`ready`] with a specific `F`.
    fn with_flags(f: u8) -> CpuState {
        CpuState {
            af: u16::from(f),
            ..ready()
        }
    }

    /// The accumulator and flags after one instruction.
    fn accumulator_and_flags(program: &[u8], state: CpuState) -> (u8, u8) {
        let (_, cpu) = run_one(program, state);
        let [a, f] = cpu.state().af.to_be_bytes();
        (a, f)
    }

    #[test]
    fn instruction_timings_match_the_published_machine_cycles() {
        let zero = ready();
        let taken = with_flags(0);
        let not_taken = with_flags(flags::ZERO);
        let counter_two = CpuState {
            bc: 0x0200,
            ..ready()
        };
        let counter_one = CpuState {
            bc: 0x0100,
            ..ready()
        };

        let cases: &[(&str, &[u8], CpuState, u32)] = &[
            ("NOP", &[0x00], zero, 4),
            ("LD B,n", &[0x06, 0x42], zero, 7),
            ("LD B,(HL)", &[0x46], zero, 7),
            ("LD (HL),B", &[0x70], zero, 7),
            ("LD (HL),n", &[0x36, 0x42], zero, 10),
            ("INC B", &[0x04], zero, 4),
            ("INC (HL)", &[0x34], zero, 11),
            ("DEC (HL)", &[0x35], zero, 11),
            ("LD BC,nn", &[0x01, 0x34, 0x12], zero, 10),
            ("LD (BC),A", &[0x02], zero, 7),
            ("LD A,(DE)", &[0x1A], zero, 7),
            ("LD (nn),A", &[0x32, 0x00, 0x40], zero, 13),
            ("LD A,(nn)", &[0x3A, 0x00, 0x40], zero, 13),
            ("LD (nn),HL", &[0x22, 0x00, 0x40], zero, 16),
            ("LD HL,(nn)", &[0x2A, 0x00, 0x40], zero, 16),
            ("INC BC", &[0x03], zero, 6),
            ("DEC SP", &[0x3B], zero, 6),
            ("ADD HL,BC", &[0x09], zero, 11),
            ("LD SP,HL", &[0xF9], zero, 6),
            ("PUSH BC", &[0xC5], zero, 11),
            ("POP BC", &[0xC1], zero, 10),
            ("ADD A,B", &[0x80], zero, 4),
            ("ADD A,(HL)", &[0x86], zero, 7),
            ("ADD A,n", &[0xC6, 0x01], zero, 7),
            ("JP nn", &[0xC3, 0x00, 0x40], zero, 10),
            // The absolute conditional jump fetches its operand either way, so unlike the
            // relative form it costs the same whether or not it branches.
            ("JP NZ,nn taken", &[0xC2, 0x00, 0x40], taken, 10),
            ("JP NZ,nn not taken", &[0xC2, 0x00, 0x40], not_taken, 10),
            ("JP (HL)", &[0xE9], zero, 4),
            ("JR e", &[0x18, 0x02], zero, 12),
            ("JR NZ,e taken", &[0x20, 0x02], taken, 12),
            ("JR NZ,e not taken", &[0x20, 0x02], not_taken, 7),
            ("DJNZ taken", &[0x10, 0x02], counter_two, 13),
            ("DJNZ not taken", &[0x10, 0x02], counter_one, 8),
            ("CALL nn", &[0xCD, 0x00, 0x40], zero, 17),
            ("CALL NZ,nn taken", &[0xC4, 0x00, 0x40], taken, 17),
            ("CALL NZ,nn not taken", &[0xC4, 0x00, 0x40], not_taken, 10),
            ("RET", &[0xC9], zero, 10),
            ("RET NZ taken", &[0xC0], taken, 11),
            ("RET NZ not taken", &[0xC0], not_taken, 5),
            ("RST 38h", &[0xFF], zero, 11),
            ("IN A,(n)", &[0xDB, 0xFE], zero, 11),
            ("OUT (n),A", &[0xD3, 0xFE], zero, 11),
            ("EX (SP),HL", &[0xE3], zero, 19),
            ("EX DE,HL", &[0xEB], zero, 4),
            ("EX AF,AF'", &[0x08], zero, 4),
            ("EXX", &[0xD9], zero, 4),
            ("HALT", &[0x76], zero, 4),
            ("DAA", &[0x27], zero, 4),
            ("CPL", &[0x2F], zero, 4),
            ("SCF", &[0x37], zero, 4),
            ("CCF", &[0x3F], zero, 4),
            ("RLCA", &[0x07], zero, 4),
            ("DI", &[0xF3], zero, 4),
            ("EI", &[0xFB], zero, 4),
            // A prefix still costs its own opcode fetch, exactly as on hardware.
            ("CB prefix", &[0xCB], zero, 4),
        ];

        for (name, program, state, expected) in cases {
            let (t_states, _) = run_one(program, *state);
            assert_eq!(t_states, *expected, "{name}");
        }
    }

    #[test]
    fn every_t_state_reports_the_address_the_z80_drives() {
        // The FUSE corpus's `MC` events are plain T-state indices: the event at time T names
        // the address on the bus during T-state T. Each expectation below is transcribed
        // from the named vector, and each `run` is one machine cycle's worth of T-states.
        //
        // This is what a batched tick cannot express. Seven T-states of `ADD HL,BC` are
        // seven separate contention points on `IR`, not one seven-T-state block, and the
        // machine could never recover `IR` from the transfer addresses because no transfer
        // ever names it.
        fn run(address: u16, count: usize) -> Vec<u16> {
            core::iter::repeat_n(address, count).collect()
        }

        fn stream(runs: &[(u16, usize)]) -> Vec<u16> {
            runs.iter().flat_map(|&(a, n)| run(a, n)).collect()
        }

        // `ADD HL,BC` — corpus `09`: M1 on PC, then seven on IR.
        let (_, cpu) = run_one(&[0x09], ready());
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[(0x0000, 4), (0x0001, 7)]),
            "ADD HL,BC"
        );

        // `INC BC` — corpus `03`: M1 on PC, then two on IR.
        let (_, cpu) = run_one(&[0x03], ready());
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[(0x0000, 4), (0x0001, 2)]),
            "INC BC"
        );

        // `JR e` — corpus `18`: the five computation T-states hold the displacement byte's
        // own address, not IR.
        let (_, cpu) = run_one(&[0x18, 0x40], ready());
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[(0x0000, 4), (0x0001, 3), (0x0001, 5)]),
            "JR e"
        );

        // `CALL nn` — corpus `cd`: the internal T-state holds the last operand's address.
        let state = CpuState {
            sp: 0xB07D,
            ..ready()
        };
        let (_, cpu) = run_one(&[0xCD, 0x5D, 0x3A], state);
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[
                (0x0000, 4),
                (0x0001, 3),
                (0x0002, 3),
                (0x0002, 1),
                (0xB07C, 3),
                (0xB07B, 3),
            ]),
            "CALL nn"
        );

        // `RST 38h` — corpus `ff`: no operands, so the same shared cycle holds IR instead.
        // `CALL` and `RST` share one implementation and still differ here.
        let state = CpuState {
            sp: 0x5507,
            ..ready()
        };
        let (_, cpu) = run_one(&[0xFF], state);
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[(0x0000, 4), (0x0001, 1), (0x5506, 3), (0x5505, 3)]),
            "RST 38h"
        );

        // `EX (SP),HL` — corpus `e3`: the two internal runs hold the two stack addresses.
        let state = CpuState {
            sp: 0x0373,
            hl: 0x224D,
            ..ready()
        };
        let (_, cpu) = run_one(&[0xE3], state);
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[
                (0x0000, 4),
                (0x0373, 3),
                (0x0374, 3),
                (0x0374, 1),
                (0x0374, 3),
                (0x0373, 3),
                (0x0373, 2),
            ]),
            "EX (SP),HL"
        );

        // `OUT (n),A` — corpus `d3`: the port cycle is four T-states on the full 16-bit
        // port address.
        let state = CpuState {
            af: 0xA200,
            ..ready()
        };
        let (_, cpu) = run_one(&[0xD3, 0xEC], state);
        assert_eq!(
            cpu.bus().tick_addresses,
            stream(&[(0x0000, 4), (0x0001, 3), (0xA2EC, 4)]),
            "OUT (n),A"
        );
    }

    #[test]
    fn a_conditional_branch_that_is_not_taken_still_fetches_its_operands() {
        // `JP cc` has a single documented cycle count (10 T) where `CALL cc` has two
        // (17/10) and `RET cc` has two (11/5) — a machine cycle that does not happen
        // changes the count, so `JP cc`'s operand fetch is a real cycle on both paths.
        // `PC` also advances by three, and on the Z80 `PC` increments as part of that very
        // cycle. The FUSE trace elides the transfer; the harness carries an exception list
        // for those vectors.
        let taken = with_flags(0); // Z clear -> NZ holds
        let not_taken = with_flags(flags::ZERO);

        let (t_taken, cpu) = run_one(&[0xC2, 0x1B, 0xE1], taken);
        assert_eq!(cpu.bus().reads, [0x0000, 0x0001, 0x0002]);
        assert_eq!(cpu.state().pc, 0xE11B);

        let (t_not_taken, cpu) = run_one(&[0xC2, 0x1B, 0xE1], not_taken);
        assert_eq!(
            cpu.bus().reads,
            [0x0000, 0x0001, 0x0002],
            "the operands are fetched either way"
        );
        assert_eq!(cpu.state().pc, 0x0003);
        assert_eq!(t_taken, t_not_taken, "and both paths cost the same");

        // `CALL cc` is the case that genuinely loses machine cycles: the operands are
        // still fetched, but nothing is pushed and `PC` does not move — corpus `c4_2`.
        let (t_states, cpu) = run_one(&[0xC4, 0x61, 0x9C], not_taken);
        assert_eq!(cpu.bus().reads, [0x0000, 0x0001, 0x0002]);
        assert_eq!(cpu.state().pc, 0x0003);
        assert_eq!(cpu.state().sp, 0xFFFF, "nothing was pushed");
        assert_eq!(t_states, 10, "against 17 when taken");

        // `JR cc` skips only the five-T-state address computation.
        let (t_states, cpu) = run_one(&[0x20, 0x40], not_taken);
        assert_eq!(cpu.bus().reads, [0x0000, 0x0001]);
        assert_eq!(cpu.state().pc, 0x0002);
        assert_eq!(t_states, 7);

        // `DJNZ` on its final pass, likewise.
        let last_pass = CpuState {
            bc: 0x0100,
            ..ready()
        };
        let (t_states, cpu) = run_one(&[0x10, 0xFD], last_pass);
        assert_eq!(cpu.bus().reads, [0x0000, 0x0001]);
        assert_eq!(cpu.state().bc, 0x0000);
        assert_eq!(cpu.state().pc, 0x0002);
        assert_eq!(t_states, 8);
    }

    #[test]
    fn a_mode_0_interrupt_the_core_cannot_execute_changes_nothing_at_all() {
        // Accept-or-decline has to be atomic. Half-accepting would clear `iff1`, strand a
        // return address and leave `PC` unmoved, so the interrupted instruction would run
        // again and every retry would leak two more bytes of stack.
        let before = CpuState {
            iff1: true,
            iff2: true,
            im: InterruptMode::Mode0,
            sp: 0xFF00,
            pc: 0x8000,
            ..ready()
        };
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(before);

        let t_states = cpu.interrupt(0x00); // NOP — not an RST

        assert_eq!(t_states, 0, "nothing charged");
        assert_eq!(cpu.state(), before, "and not one field moved");
        assert!(cpu.bus().reads.is_empty(), "no transfer");
        assert_eq!(cpu.bus().t_states, 0, "the bus saw no cycles");
        assert_eq!(
            cpu.fault(),
            Some(StepError::UnsupportedInterruptOpcode { opcode: 0x00 }),
            "but the reason is reported"
        );
    }

    #[test]
    fn every_entry_point_clears_the_previous_operations_bookkeeping() {
        // `fault` is per-operation state, so each entry point must reset it. Three of them
        // now share one `begin_operation`, because adding the line separately to each is
        // how two of the three came to be missing it.
        let armed = CpuState {
            iff1: true,
            im: InterruptMode::Mode1,
            sp: 0xFF00,
            ..ready()
        };

        let mut cpu = Cpu::new(Ram::new(&[0xCB]));
        cpu.set_state(armed);
        cpu.step();
        assert!(cpu.fault().is_some(), "the prefix faulted");
        cpu.interrupt(0xFF);
        assert!(cpu.fault().is_none(), "an accepted interrupt clears it");

        let mut cpu = Cpu::new(Ram::new(&[0xCB]));
        cpu.set_state(armed);
        cpu.step();
        assert!(cpu.fault().is_some());
        cpu.nmi();
        assert!(cpu.fault().is_none(), "and so does an NMI");
    }

    #[test]
    fn every_unprefixed_opcode_is_decoded() {
        for opcode in 0..=u8::MAX {
            let (t_states, cpu) = run_one(&[opcode, 0x00, 0x00], ready());
            let is_prefix = matches!(opcode, 0xCB | 0xDD | 0xED | 0xFD);
            assert_eq!(
                cpu.fault().is_some(),
                is_prefix,
                "opcode {opcode:#04X} fault state"
            );
            // Even the shortest instruction spends its opcode fetch, so a zero total would
            // mean an arm that silently did nothing.
            assert!(
                t_states >= 4,
                "opcode {opcode:#04X} took {t_states} T-states"
            );
        }
    }

    #[test]
    fn prefixes_report_the_byte_and_its_address() {
        for prefix in [0xCB_u8, 0xDD, 0xED, 0xFD] {
            let state = CpuState {
                pc: 0x1234,
                ..ready()
            };
            let mut cpu = Cpu::new(Ram::new(&[]));
            cpu.bus_mut().memory[0x1234] = prefix;
            cpu.set_state(state);
            cpu.step();

            assert_eq!(
                cpu.fault(),
                Some(StepError::UnsupportedPrefix {
                    prefix,
                    address: 0x1234,
                })
            );
        }
    }

    #[test]
    fn a_fault_is_cleared_by_the_next_successful_step() {
        let mut cpu = Cpu::new(Ram::new(&[0xCB, 0x00]));
        cpu.set_state(ready());

        cpu.step();
        assert!(cpu.fault().is_some(), "prefix should fault");
        cpu.step();
        assert!(cpu.fault().is_none(), "NOP should clear the fault");
    }

    #[test]
    fn refresh_register_wraps_in_seven_bits() {
        // Bit 7 is a latch that only `LD R,A` can change; the M1 increment must roll 0x7F
        // round to 0x00 without touching it.
        let (_, cpu) = run_one(&[0x00], CpuState { r: 0x7F, ..ready() });
        assert_eq!(cpu.state().r, 0x00);

        let (_, cpu) = run_one(&[0x00], CpuState { r: 0xFF, ..ready() });
        assert_eq!(cpu.state().r, 0x80, "bit 7 must survive the wrap");
    }

    #[test]
    fn halt_holds_the_program_counter_and_keeps_refreshing() {
        let mut cpu = Cpu::new(Ram::new(&[0x76]));
        cpu.set_state(ready());

        cpu.step();
        let after_first = cpu.state();
        assert!(after_first.halted);
        assert_eq!(after_first.pc, 0x0000, "PC must not advance past HALT");

        cpu.step();
        let after_second = cpu.state();
        assert_eq!(after_second.pc, 0x0000, "PC still held");
        assert_eq!(
            after_second.r,
            after_first.r.wrapping_add(1),
            "halted M1 cycles still refresh"
        );
    }

    #[test]
    fn halted_is_authoritative_not_a_shadow_of_the_program_counter() {
        // Setting the flag through a snapshot must stop the CPU even though PC points at
        // an ordinary instruction, and clearing it must resume execution of that same
        // instruction. If `halted` were merely a record of "PC was rewound", neither would
        // hold.
        let mut cpu = Cpu::new(Ram::new(&[0x3C])); // INC A
        cpu.set_state(CpuState {
            halted: true,
            ..ready()
        });

        let t_states = cpu.step();
        assert_eq!(t_states, 4, "a halted step is one M1 cycle");
        assert_eq!(cpu.state().pc, 0x0000, "PC does not move while halted");
        assert_eq!(cpu.state().af.to_be_bytes()[0], 0x00, "INC A did not run");

        cpu.set_state(CpuState {
            halted: false,
            ..cpu.state()
        });
        cpu.step();
        assert_eq!(cpu.state().af.to_be_bytes()[0], 0x01, "INC A ran on resume");
    }

    #[test]
    fn ei_defers_interrupt_acceptance_by_one_instruction() {
        let mut cpu = Cpu::new(Ram::new(&[0xFB, 0x00, 0x00]));
        cpu.set_state(ready());

        cpu.step();
        assert!(cpu.state().iff1, "EI enables interrupts");
        assert!(
            cpu.ei_pending(),
            "acceptance is deferred over the next instruction"
        );

        cpu.step();
        assert!(
            !cpu.ei_pending(),
            "the deferral expires after one instruction"
        );
        assert!(cpu.state().iff1, "but interrupts stay enabled");
    }

    #[test]
    fn loading_a_snapshot_clears_the_ei_deferral() {
        // Otherwise a snapshot taken immediately after `EI` would suppress the first
        // interrupt of the frame it is restored into.
        let mut cpu = Cpu::new(Ram::new(&[0xFB]));
        cpu.set_state(ready());
        cpu.step();
        assert!(cpu.ei_pending());

        cpu.set_state(cpu.state());
        assert!(
            !cpu.ei_pending(),
            "the deferral does not survive a state load"
        );
    }

    #[test]
    fn di_clears_both_flip_flops() {
        let state = CpuState {
            iff1: true,
            iff2: true,
            ..ready()
        };
        let (_, cpu) = run_one(&[0xF3], state);
        assert!(!cpu.state().iff1);
        assert!(!cpu.state().iff2);
    }

    #[test]
    fn a_maskable_interrupt_is_declined_unless_it_can_be_accepted() {
        // Disabled.
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(ready());
        assert_eq!(cpu.interrupt(0xFF), 0, "declined while IFF1 is clear");
        assert_eq!(cpu.state().pc, 0x0000, "and nothing happened");

        // Enabled, but `EI`'s deferral is still open.
        let mut cpu = Cpu::new(Ram::new(&[0xFB]));
        cpu.set_state(ready());
        cpu.step();
        assert_eq!(cpu.interrupt(0xFF), 0, "declined during the EI deferral");
    }

    #[test]
    fn mode_1_interrupt_vectors_to_0x0038() {
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(CpuState {
            iff1: true,
            iff2: true,
            im: InterruptMode::Mode1,
            sp: 0xFFFF,
            pc: 0x1234,
            ..ready()
        });

        let t_states = cpu.interrupt(0xFF);
        let after = cpu.state();

        assert_eq!(t_states, 13, "acknowledge (7) plus two pushes (3 + 3)");
        assert_eq!(after.pc, 0x0038);
        assert_eq!(after.sp, 0xFFFD);
        assert_eq!(cpu.bus().peek(0xFFFE), 0x12, "return address pushed");
        assert_eq!(cpu.bus().peek(0xFFFD), 0x34);
        assert!(!after.iff1, "acceptance disables both flip-flops");
        assert!(!after.iff2);
    }

    #[test]
    fn mode_2_interrupt_reads_its_vector_from_the_table() {
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        // I = 0x80 and the device supplies 0x40, so the vector lives at 0x8040.
        cpu.bus_mut().memory[0x8040] = 0x21;
        cpu.bus_mut().memory[0x8041] = 0x43;
        cpu.set_state(CpuState {
            iff1: true,
            im: InterruptMode::Mode2,
            i: 0x80,
            sp: 0xFFFF,
            pc: 0x1234,
            ..ready()
        });

        let t_states = cpu.interrupt(0x40);

        assert_eq!(t_states, 19, "mode 2 adds a vector read (3 + 3)");
        assert_eq!(cpu.state().pc, 0x4321, "little-endian vector");
    }

    #[test]
    fn mode_0_executes_the_supplied_restart_and_faults_on_anything_else() {
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(CpuState {
            iff1: true,
            im: InterruptMode::Mode0,
            sp: 0xFFFF,
            pc: 0x1234,
            ..ready()
        });
        // 0xFF is RST 38h — what a Spectrum's floating bus supplies.
        assert_eq!(cpu.interrupt(0xFF), 13);
        assert_eq!(cpu.state().pc, 0x0038);
        assert!(cpu.fault().is_none());

        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(CpuState {
            iff1: true,
            im: InterruptMode::Mode0,
            sp: 0xFFFF,
            ..ready()
        });
        cpu.interrupt(0x00); // NOP is not an RST
        assert_eq!(
            cpu.fault(),
            Some(StepError::UnsupportedInterruptOpcode { opcode: 0x00 })
        );
    }

    #[test]
    fn an_accepted_interrupt_resumes_from_halt_and_returns_past_it() {
        // The return address must be HALT + 1, or the handler's RET would drop the CPU
        // straight back into the HALT it just left.
        let mut cpu = Cpu::new(Ram::new(&[0x76])); // HALT at 0x0000
        cpu.set_state(CpuState {
            iff1: true,
            im: InterruptMode::Mode1,
            sp: 0xFFFF,
            ..ready()
        });

        cpu.step();
        assert!(cpu.state().halted);
        assert_eq!(cpu.state().pc, 0x0000);

        cpu.interrupt(0xFF);
        let after = cpu.state();
        assert!(!after.halted, "the interrupt resumes the CPU");
        assert_eq!(after.pc, 0x0038);
        assert_eq!(cpu.bus().peek(0xFFFD), 0x01, "returns to HALT + 1");
        assert_eq!(cpu.bus().peek(0xFFFE), 0x00);
    }

    #[test]
    fn a_non_maskable_interrupt_is_always_accepted_and_saves_iff1() {
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(CpuState {
            iff1: true,
            iff2: false,
            sp: 0xFFFF,
            pc: 0x1234,
            ..ready()
        });

        let t_states = cpu.nmi();
        let after = cpu.state();

        assert_eq!(t_states, 11, "acknowledge (5) plus two pushes (3 + 3)");
        assert_eq!(after.pc, NMI_VECTOR);
        assert!(!after.iff1);
        assert!(after.iff2, "IFF1 is saved into IFF2 for RETN");

        // And it does not consult IFF1 the way a maskable interrupt does.
        let mut cpu = Cpu::new(Ram::new(&[0x00]));
        cpu.set_state(ready());
        assert_eq!(cpu.nmi(), 11, "accepted even with interrupts disabled");
    }

    #[test]
    fn the_flag_latch_records_the_last_written_flags() {
        // The latch M4 needs for SCF/CCF. It is maintained but not yet consumed.
        let state = CpuState {
            af: 0x0100,
            bc: 0x0100,
            ..ready()
        };
        let (_, cpu) = run_one(&[0x80], state); // ADD A,B
        let after = cpu.state();
        let [_, f] = after.af.to_be_bytes();
        assert_eq!(after.q, f, "an ALU op latches the flags it wrote");

        // An instruction that writes no flags leaves the latch clear.
        let (_, cpu) = run_one(&[0x00], state); // NOP
        assert_eq!(cpu.state().q, 0);

        let (_, cpu) = run_one(&[0x41], state); // LD B,C
        assert_eq!(cpu.state().q, 0);
    }

    #[test]
    fn push_writes_high_byte_first_and_grows_downwards() {
        let state = CpuState {
            bc: 0x1234,
            sp: 0xFFFF,
            ..ready()
        };
        let (_, cpu) = run_one(&[0xC5], state);

        assert_eq!(cpu.state().sp, 0xFFFD);
        assert_eq!(
            cpu.bus().peek(0xFFFE),
            0x12,
            "high byte at the higher address"
        );
        assert_eq!(
            cpu.bus().peek(0xFFFD),
            0x34,
            "low byte at the lower address"
        );
    }

    #[test]
    fn call_pushes_the_return_address_and_ret_recovers_it() {
        let mut cpu = Cpu::new(Ram::new(&[0xCD, 0x06, 0x00]));
        cpu.bus_mut().memory[0x0006] = 0xC9; // RET
        cpu.set_state(ready());

        cpu.step();
        let after_call = cpu.state();
        assert_eq!(after_call.pc, 0x0006);
        assert_eq!(after_call.sp, 0xFFFD);
        assert_eq!(cpu.bus().peek(0xFFFE), 0x00);
        assert_eq!(
            cpu.bus().peek(0xFFFD),
            0x03,
            "the address after the CALL operands"
        );

        cpu.step();
        let after_ret = cpu.state();
        assert_eq!(after_ret.pc, 0x0003);
        assert_eq!(after_ret.sp, 0xFFFF);
    }

    #[test]
    fn rst_calls_its_page_zero_vector() {
        // RST 38h — the vector the Spectrum's frame interrupt lands on.
        let (_, cpu) = run_one(&[0xFF], ready());
        assert_eq!(cpu.state().pc, 0x0038);
        assert_eq!(cpu.state().sp, 0xFFFD);
    }

    #[test]
    fn djnz_decrements_b_without_touching_flags() {
        let state = CpuState {
            bc: 0x0200,
            af: u16::from(flags::ZERO | flags::CARRY),
            ..ready()
        };
        let (_, cpu) = run_one(&[0x10, 0x02], state);
        let after = cpu.state();

        assert_eq!(after.bc, 0x0100, "B decremented, C untouched");
        assert_eq!(after.pc, 0x0004, "branch taken: PC + displacement");
        let [_, f] = after.af.to_be_bytes();
        assert_eq!(f, flags::ZERO | flags::CARRY, "DJNZ affects no flags");
    }

    #[test]
    fn jr_displacement_is_signed_and_measured_from_after_the_operand() {
        // JR -2 is the idiomatic tight spin: it jumps back onto itself.
        let (_, cpu) = run_one(&[0x18, 0xFE], ready());
        assert_eq!(cpu.state().pc, 0x0000);
    }

    #[test]
    fn exchange_stack_hl_swaps_both_ways() {
        let mut cpu = Cpu::new(Ram::new(&[0xE3]));
        cpu.bus_mut().memory[0x8000] = 0x34;
        cpu.bus_mut().memory[0x8001] = 0x12;
        cpu.set_state(CpuState {
            hl: 0xABCD,
            sp: 0x8000,
            ..ready()
        });

        cpu.step();
        assert_eq!(cpu.state().hl, 0x1234);
        assert_eq!(cpu.bus().peek(0x8000), 0xCD);
        assert_eq!(cpu.bus().peek(0x8001), 0xAB);
        assert_eq!(cpu.state().sp, 0x8000, "SP is unchanged");
    }

    #[test]
    fn exx_leaves_af_alone_and_ex_af_leaves_the_rest_alone() {
        let state = CpuState {
            af: 0x1111,
            bc: 0x2222,
            af_shadow: 0x3333,
            bc_shadow: 0x4444,
            ..ready()
        };

        let (_, cpu) = run_one(&[0xD9], state); // EXX
        assert_eq!(cpu.state().bc, 0x4444);
        assert_eq!(cpu.state().bc_shadow, 0x2222);
        assert_eq!(cpu.state().af, 0x1111, "EXX does not touch AF");

        let (_, cpu) = run_one(&[0x08], state); // EX AF,AF'
        assert_eq!(cpu.state().af, 0x3333);
        assert_eq!(cpu.state().af_shadow, 0x1111);
        assert_eq!(cpu.state().bc, 0x2222, "EX AF,AF' does not touch BC");
    }

    #[test]
    fn xor_a_clears_the_accumulator_and_or_a_leaves_it() {
        // `XOR A` is the idiomatic way to zero the accumulator, and it is the sharpest
        // possible check on the ALU operation field: it must yield 0x00 for every input, so
        // if the decoder reaches `OR` instead the accumulator comes back untouched. The two
        // operations are adjacent in the field (XOR is 5, OR is 6).
        for accumulator in [0x00_u8, 0x01, 0x5A, 0xF5, 0xFF] {
            let state = CpuState {
                af: u16::from(accumulator) << 8,
                ..ready()
            };

            let (a, _) = accumulator_and_flags(&[0xAF], state); // XOR A
            assert_eq!(a, 0x00, "XOR A with A={accumulator:#04X}");

            let (a, _) = accumulator_and_flags(&[0xB7], state); // OR A
            assert_eq!(a, accumulator, "OR A with A={accumulator:#04X}");
        }
    }

    #[test]
    fn the_alu_operation_field_selects_the_right_operation() {
        // Locks all eight encodings of the `ooo` field in both `10 ooo rrr` (register) and
        // `11 ooo 110` (immediate) form. A=0x3E, operand=0xD0 gives a distinct result for
        // every operation, so any transposition in the table shows up here.
        const A: u8 = 0x3E;
        const OPERAND: u8 = 0xD0;

        // (field, name, register opcode `op A,B`, immediate opcode `op n`, expected A)
        let cases: &[(u8, &str, u8, u8, u8)] = &[
            (0, "ADD", 0x80, 0xC6, 0x0E),
            (1, "ADC", 0x88, 0xCE, 0x0E),
            (2, "SUB", 0x90, 0xD6, 0x6E),
            (3, "SBC", 0x98, 0xDE, 0x6E),
            (4, "AND", 0xA0, 0xE6, 0x10),
            (5, "XOR", 0xA8, 0xEE, 0xEE),
            (6, "OR", 0xB0, 0xF6, 0xFE),
            (7, "CP", 0xB8, 0xFE, A), // CP discards its result
        ];

        for &(field, name, register_opcode, immediate_opcode, expected) in cases {
            assert_eq!(
                (register_opcode >> 3) & 0x07,
                field,
                "{name} register opcode encodes field {field}"
            );
            assert_eq!(
                (immediate_opcode >> 3) & 0x07,
                field,
                "{name} immediate opcode encodes field {field}"
            );

            let state = CpuState {
                af: u16::from(A) << 8,
                bc: u16::from(OPERAND) << 8,
                ..ready()
            };

            let (a, _) = accumulator_and_flags(&[register_opcode], state);
            assert_eq!(a, expected, "{name} A,B");

            let (a, _) = accumulator_and_flags(&[immediate_opcode, OPERAND], state);
            assert_eq!(a, expected, "{name} n");
        }
    }

    #[test]
    fn cp_takes_the_undocumented_bits_from_the_operand() {
        // CP discards its result, so bits 3 and 5 come from the value compared against.
        // 0x28 is exactly those two bits, and the difference 0xD8 carries only bit 3 — so a
        // core that copied them from the result would drop bit 5 here.
        let state = CpuState {
            af: 0x0000,
            bc: 0x2800,
            ..ready()
        };
        let (a, f) = accumulator_and_flags(&[0xB8], state); // CP B

        assert_eq!(a, 0x00, "CP leaves the accumulator alone");
        assert_eq!(f & (flags::BIT3 | flags::BIT5), 0x28, "from the operand");
        assert_eq!(
            f,
            flags::SIGN
                | flags::BIT5
                | flags::HALF_CARRY
                | flags::BIT3
                | flags::ADD_SUBTRACT
                | flags::CARRY
        );
    }

    #[test]
    fn add_reports_half_carry_and_signed_overflow() {
        // 0x0F + 0x01 carries out of bit 3 but not out of bit 7.
        let state = CpuState {
            af: 0x0F00,
            bc: 0x0100,
            ..ready()
        };
        let (a, f) = accumulator_and_flags(&[0x80], state); // ADD A,B
        assert_eq!(a, 0x10);
        assert_eq!(f, flags::HALF_CARRY);

        // 0x7F + 0x01 crosses the signed boundary: sign set, overflow set.
        let state = CpuState {
            af: 0x7F00,
            bc: 0x0100,
            ..ready()
        };
        let (a, f) = accumulator_and_flags(&[0x80], state);
        assert_eq!(a, 0x80);
        assert_eq!(f, flags::SIGN | flags::HALF_CARRY | flags::PARITY_OVERFLOW);
    }

    #[test]
    fn inc_and_dec_preserve_the_carry_flag() {
        // This is what separates INC from ADD A,1 and makes INC usable inside a multi-byte
        // addition.
        let state = CpuState {
            bc: 0x0000,
            af: u16::from(flags::CARRY),
            ..ready()
        };
        let (_, cpu) = run_one(&[0x04], state); // INC B
        let [_, f] = cpu.state().af.to_be_bytes();
        assert_eq!(f & flags::CARRY, flags::CARRY, "carry survives INC");

        let (_, cpu) = run_one(&[0x05], state); // DEC B
        let [_, f] = cpu.state().af.to_be_bytes();
        assert_eq!(f & flags::CARRY, flags::CARRY, "carry survives DEC");
        assert_eq!(f & flags::ADD_SUBTRACT, flags::ADD_SUBTRACT, "DEC sets N");
    }

    #[test]
    fn add_hl_leaves_sign_zero_and_overflow_untouched() {
        let preserved = flags::SIGN | flags::ZERO | flags::PARITY_OVERFLOW;
        let state = CpuState {
            af: u16::from(preserved),
            hl: 0x0000,
            bc: 0x0001,
            ..ready()
        };
        let (_, cpu) = run_one(&[0x09], state); // ADD HL,BC
        let [_, f] = cpu.state().af.to_be_bytes();

        assert_eq!(cpu.state().hl, 0x0001);
        assert_eq!(f, preserved, "only H, N and C are defined by ADD HL,ss");
    }

    #[test]
    fn add_hl_hl_doubles_the_pair() {
        // The encoding that names the same pair twice — and the one `DD 29` turns into
        // `ADD IX,IX`, so it is the case the base substitution has to get right in two
        // positions at once.
        let state = CpuState {
            hl: 0x1234,
            ..ready()
        };
        let (_, cpu) = run_one(&[0x29], state);
        assert_eq!(cpu.state().hl, 0x2468);
    }

    #[test]
    fn accumulator_rotates_leave_sign_zero_and_parity_untouched() {
        // The distinguishing behaviour against the CB-prefixed rotates: the zero flag stays
        // set even though the result is non-zero.
        let preserved = flags::SIGN | flags::ZERO;
        let state = CpuState {
            af: 0x8000 | u16::from(preserved),
            ..ready()
        };
        let (a, f) = accumulator_and_flags(&[0x07], state); // RLCA

        assert_eq!(a, 0x01);
        assert_eq!(f, preserved | flags::CARRY);
    }

    #[test]
    fn daa_renormalises_a_nibble_that_overflowed_bcd_range() {
        // 0x0A is not valid BCD; after an addition it must become 0x10.
        let state = CpuState {
            af: 0x0A00,
            ..ready()
        };
        let (a, f) = accumulator_and_flags(&[0x27], state);

        assert_eq!(a, 0x10);
        assert_eq!(
            f,
            flags::HALF_CARRY,
            "the correction itself carried out of bit 3"
        );
    }

    #[test]
    fn daa_subtracts_when_the_previous_operation_did() {
        // With N and H set, DAA corrects downwards: 0x0F - 0x06 = 0x09.
        let state = CpuState {
            af: 0x0F00 | u16::from(flags::ADD_SUBTRACT | flags::HALF_CARRY),
            ..ready()
        };
        let (a, f) = accumulator_and_flags(&[0x27], state);

        assert_eq!(a, 0x09);
        assert_eq!(
            f & flags::ADD_SUBTRACT,
            flags::ADD_SUBTRACT,
            "N is preserved"
        );
        assert_eq!(f & flags::CARRY, 0);
    }

    #[test]
    fn scf_and_ccf_take_the_undocumented_bits_from_the_accumulator() {
        let state = CpuState {
            af: 0x2800,
            ..ready()
        };
        let (_, f) = accumulator_and_flags(&[0x37], state); // SCF
        assert_eq!(f, flags::BIT5 | flags::BIT3 | flags::CARRY);

        // CCF moves the old carry into the half-carry and inverts it.
        let state = CpuState {
            af: 0x2800 | u16::from(flags::CARRY),
            ..ready()
        };
        let (_, f) = accumulator_and_flags(&[0x3F], state);
        assert_eq!(f, flags::BIT5 | flags::HALF_CARRY | flags::BIT3);
    }

    #[test]
    fn in_and_out_put_the_accumulator_on_the_high_half_of_the_port() {
        // Both forms address port `A * 256 + n`, which is why Spectrum peripherals decode
        // the high half of the address bus as well as the low — port 0xFE reaches the ULA
        // whatever the accumulator holds, but the keyboard row is selected by that half.
        let state = CpuState {
            af: 0x7F00 | u16::from(flags::ZERO),
            ..ready()
        };

        let (_, cpu) = run_one(&[0xDB, 0xFE], state); // IN A,(0xFE)
        let [a, f] = cpu.state().af.to_be_bytes();
        assert_eq!(cpu.bus().last_port, Some(0x7FFE));
        assert_eq!(a, PORT_INPUT);
        assert_eq!(f, flags::ZERO, "IN A,(n) affects no flags");

        let (_, cpu) = run_one(&[0xD3, 0xFE], state); // OUT (0xFE),A
        assert_eq!(cpu.bus().last_port, Some(0x7FFE));
        assert_eq!(cpu.bus().last_port_write, Some(0x7F), "OUT writes A");
    }

    #[test]
    fn interrupt_mode_round_trips_through_its_byte_encoding() {
        for (byte, mode) in [
            (0, InterruptMode::Mode0),
            (1, InterruptMode::Mode1),
            (2, InterruptMode::Mode2),
        ] {
            assert_eq!(InterruptMode::try_from(byte), Ok(mode));
            assert_eq!(u8::from(mode), byte);
        }
        assert_eq!(InterruptMode::try_from(3), Err(InvalidInterruptMode(3)));
    }

    #[test]
    fn reset_state_matches_real_hardware() {
        let cpu = Cpu::new(Ram::new(&[]));
        let state = cpu.state();

        assert_eq!(state.af, 0xFFFF);
        assert_eq!(state.sp, 0xFFFF);
        assert_eq!(state.pc, 0x0000);
        assert_eq!(state.i, 0);
        assert_eq!(state.r, 0);
        assert!(!state.iff1);
        assert!(!state.iff2);
        assert_eq!(state.im, InterruptMode::Mode0);
        assert!(!state.halted);
        assert_eq!(state.wz, 0);
        assert_eq!(state.q, 0);
    }

    #[test]
    fn state_round_trips() {
        let state = CpuState {
            af: 0x0102,
            bc: 0x0304,
            de: 0x0506,
            hl: 0x0708,
            af_shadow: 0x090A,
            bc_shadow: 0x0B0C,
            de_shadow: 0x0D0E,
            hl_shadow: 0x0F10,
            ix: 0x1112,
            iy: 0x1314,
            sp: 0x1516,
            pc: 0x1718,
            i: 0x19,
            r: 0x1A,
            iff1: true,
            iff2: true,
            im: InterruptMode::Mode2,
            halted: true,
            wz: 0x1B1C,
            q: 0x1D,
        };

        let mut cpu = Cpu::new(Ram::new(&[]));
        cpu.set_state(state);
        assert_eq!(cpu.state(), state);
    }
}
