//! The bus the vectors run on, and the **single point of contact** with the `z80` crate.
//!
//! # This file is the contract seam
//!
//! Every other module in `tests/` is free of any dependency on the CPU core: the parser,
//! the flag model, the independent ALU reference and the mismatch reporter are all pure
//! data. That is deliberate. This file, and only this file, names types and methods from
//! `z80`, so when the core's API changes, exactly one file needs editing.
//!
//! # What this harness needs from `z80`
//!
//! ```text
//! pub trait Bus {
//!     fn read(&mut self, addr: u16) -> u8;
//!     fn write(&mut self, addr: u16, val: u8);
//!     fn in_port(&mut self, port: u16) -> u8;
//!     fn out_port(&mut self, port: u16, val: u8);
//!     fn tick(&mut self, addr: u16);      // ONE T-state, with the address on the bus
//! }
//!
//! pub struct Cpu<B: Bus>;
//! impl<B: Bus> Cpu<B> {
//!     fn new(bus: B) -> Self;
//!     fn step(&mut self) -> u32;
//!     fn state(&self) -> CpuState;
//!     fn set_state(&mut self, state: CpuState);
//!     fn fault(&self) -> Option<StepError>;
//!     fn ei_pending(&self) -> bool;
//!     fn bus(&self) -> &B;
//! }
//! ```
//!
//! Only [`cpu_state`] and [`snapshot`] at the bottom of this file touch `CpuState`.
//!
//! # Why `tick` carries an address and is never batched
//!
//! A previous revision of the trait was `tick(&mut self, t_states: u8)`, and the core
//! charged a whole run of internal cycles with a single call. Both halves of that were
//! wrong for a Spectrum, and the corpus says so directly. Vector `09` (`ADD HL,BC`):
//!
//! ```text
//!  0 MC 0000       4 MR 0000 09
//!  4 MC 0001       5 MC 0001      6 MC 0001      7 MC 0001
//!  8 MC 0001       9 MC 0001     10 MC 0001
//! ```
//!
//! Seven **separate** one-T-state contention checks at `0x0001` — the `IR` pair, which is
//! what the Z80 parks on the address bus during internal cycles. ULA contention is a
//! function of `t mod 8`, so seven independent checks starting at T=4 are not the same
//! thing as one seven-T-state block starting at T=4, and no amount of arithmetic on a
//! batched total can recover the difference. An address per T-state is the minimum the
//! machine crate needs to model contention at all.

use z80::{Bus, Cpu, CpuState, InterruptMode, StepError};

use super::vectors::{EventKind, Registers, Setup, State, Transfer};

/// The Z80 address space. Every `u16` address is in range by construction, so indexing
/// this buffer cannot panic.
pub const MEMORY_SIZE: usize = 0x1_0000;

/// Upper bound on instructions executed per vector.
///
/// A core that reports no T-states would otherwise spin the run loop forever. A hanging
/// suite is strictly worse than a failing one: it gives no diagnosis and blocks CI, so the
/// loop is bounded and the overrun is reported as an ordinary failure.
/// The measured worst cases are 17 instructions un-prefixed and 16 for `edb0` (`LDIR`) at
/// 331 T-states, so this leaves roughly 4x headroom and is not expected to move at M2.
pub const MAX_STEPS_PER_VECTOR: u32 = 64;

/// 64K of RAM, the per-T-state address log, and the ordered list of bytes moved.
pub struct TestBus {
    memory: Vec<u8>,
    /// The address on the bus during each T-state, indexed **by T-state**. `tick` is
    /// called once per T-state, so pushing one entry per call keeps the index and the
    /// clock the same number — and makes the T-state total simply the length.
    tick_addresses: Vec<u16>,
    transfers: Vec<Transfer>,
}

impl Default for TestBus {
    fn default() -> Self {
        Self::new()
    }
}

impl TestBus {
    pub fn new() -> Self {
        Self {
            memory: vec![0; MEMORY_SIZE],
            tick_addresses: Vec::new(),
            transfers: Vec::new(),
        }
    }

    /// Write a vector's initial memory image into the RAM.
    pub fn load(&mut self, setup: &Setup) {
        for block in &setup.memory {
            for (addr, byte) in block.addresses() {
                self.memory[usize::from(addr)] = byte;
            }
        }
    }

    /// Read without ticking or recording — for assertions after the run.
    pub fn peek(&self, addr: u16) -> u8 {
        self.memory[usize::from(addr)]
    }

    /// Total T-states, which is exactly the number of `tick` calls.
    pub fn t_states(&self) -> u32 {
        self.tick_addresses.len() as u32
    }

    /// The address on the bus at each T-state, indexed by T-state.
    pub fn tick_addresses(&self) -> &[u16] {
        &self.tick_addresses
    }

    /// Every byte moved, in order.
    pub fn transfers(&self) -> &[Transfer] {
        &self.transfers
    }

    fn record(&mut self, kind: EventKind, addr: u16, data: u8) {
        self.transfers.push(Transfer { kind, addr, data });
    }
}

impl Bus for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        let value = self.memory[usize::from(addr)];
        self.record(EventKind::MemoryRead, addr, value);
        value
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.memory[usize::from(addr)] = val;
        self.record(EventKind::MemoryWrite, addr, val);
    }

    /// An unattached Z80 `IN` reads the high half of the address bus.
    ///
    /// Derived from the corpus, not copied from another emulator: every one of the 36 `PR`
    /// events in `tests.expected` carries `data == port >> 8`. `tests/fuse_format.rs`
    /// verifies that over the whole corpus rather than trusting it.
    fn in_port(&mut self, port: u16) -> u8 {
        let value = (port >> 8) as u8;
        self.record(EventKind::PortRead, port, value);
        value
    }

    fn out_port(&mut self, port: u16, val: u8) {
        self.record(EventKind::PortWrite, port, val);
    }

    /// One T-state elapses with `addr` on the bus.
    fn tick(&mut self, addr: u16) {
        self.tick_addresses.push(addr);
    }
}

/// Why a vector's run loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// The T-state budget was reached — the normal outcome.
    ReachedBudget,
    /// [`MAX_STEPS_PER_VECTOR`] instructions ran without reaching the budget, meaning the
    /// core is reporting too few T-states per instruction (often none at all).
    StepLimit,
}

/// A CPU wired to a [`TestBus`], loaded with one vector.
pub struct Machine {
    cpu: Cpu<TestBus>,
    reported_t_states: u32,
}

impl Machine {
    /// Build a machine in the vector's initial state.
    pub fn load(setup: &Setup) -> Self {
        let mut bus = TestBus::new();
        bus.load(setup);
        let mut cpu = Cpu::new(bus);
        cpu.set_state(cpu_state(&setup.registers, &setup.state));
        Self {
            cpu,
            reported_t_states: 0,
        }
    }

    /// Run instructions until the T-state budget is reached, always executing at least
    /// one — the corpus's own convention, since its budget for a single-instruction vector
    /// is typically far below the instruction's real cost.
    pub fn run(&mut self, until_t_states: u32) -> Completion {
        for _ in 0..MAX_STEPS_PER_VECTOR {
            self.step();
            if self.bus().t_states() >= until_t_states {
                return Completion::ReachedBudget;
            }
        }
        Completion::StepLimit
    }

    /// Execute exactly one instruction, returning the T-states it reported.
    pub fn step(&mut self) -> u32 {
        let t_states = self.cpu.step();
        self.reported_t_states += t_states;
        t_states
    }

    /// The sum of what [`Cpu::step`] returned.
    ///
    /// This used to be described as "a second, independent account" of the T-state total,
    /// to be cross-checked against the bus's tick count. **It is not, and that claim was
    /// worse than the missing check** — it stopped anyone from adding a real one. In the
    /// core, `self.t_states += 1` and `self.bus.tick(address)` are adjacent lines and
    /// `step` returns that same counter, so comparing the two compares one number with
    /// itself and can never fail.
    ///
    /// The genuine independent oracle is the corpus: its `tstates` column is derived from
    /// the published machine-cycle lengths, not from our core. So this value is asserted
    /// against *that*, alongside the bus count, in `fuse_vectors.rs` — two claims each
    /// measured against an outside authority, rather than against each other.
    pub fn reported_t_states(&self) -> u32 {
        self.reported_t_states
    }

    /// Any error the core recorded during the run — at M1, an unimplemented prefix.
    pub fn fault(&self) -> Option<StepError> {
        self.cpu.fault()
    }

    /// Whether an `EI` has opened its one-instruction interrupt-delay window.
    pub fn ei_pending(&self) -> bool {
        self.cpu.ei_pending()
    }

    /// Replace the whole machine state, as a snapshot load would.
    pub fn set_state(&mut self, registers: &Registers, state: &State) {
        self.cpu.set_state(cpu_state(registers, state));
    }

    /// The final register and machine state. `State::t_states` carries the T-state total,
    /// so the result has the same shape as a parsed expectation.
    pub fn snapshot(&self) -> (Registers, State) {
        snapshot(&self.cpu)
    }

    pub fn read_memory(&self, addr: u16) -> u8 {
        self.bus().peek(addr)
    }

    /// The address on the bus at each T-state, indexed by T-state.
    pub fn tick_addresses(&self) -> &[u16] {
        self.bus().tick_addresses()
    }

    /// Every byte the instruction moved, in order.
    pub fn transfers(&self) -> &[Transfer] {
        self.bus().transfers()
    }

    fn bus(&self) -> &TestBus {
        self.cpu.bus()
    }
}

// ---------------------------------------------------------------------------
// The state seam — the only place the core's `CpuState` appears
// ---------------------------------------------------------------------------

/// Corpus vector -> the core's public state struct.
fn cpu_state(registers: &Registers, state: &State) -> CpuState {
    CpuState {
        af: registers.af,
        bc: registers.bc,
        de: registers.de,
        hl: registers.hl,
        af_shadow: registers.af_shadow,
        bc_shadow: registers.bc_shadow,
        de_shadow: registers.de_shadow,
        hl_shadow: registers.hl_shadow,
        ix: registers.ix,
        iy: registers.iy,
        sp: registers.sp,
        pc: registers.pc,
        i: state.i,
        r: state.r,
        iff1: state.iff1,
        iff2: state.iff2,
        // The parser rejects any interrupt mode outside 0..=2, so this cannot fail on a
        // corpus that loaded successfully.
        im: InterruptMode::try_from(state.im)
            .expect("the corpus parser accepts only interrupt modes 0, 1 and 2"),
        halted: state.halted,
        // `wz` (MEMPTR) and `q` (the SCF/CCF flag latch) have no column in the corpus —
        // FUSE's format predates both being understood. Defaulting them is the honest
        // reading: the vectors neither set nor check them, so the harness must not invent
        // values. Their real oracle is zexall at M4.
        ..CpuState::default()
    }
}

/// The core's public state struct -> the shape the corpus comparison speaks.
fn snapshot(cpu: &Cpu<TestBus>) -> (Registers, State) {
    let state = cpu.state();
    let registers = Registers {
        af: state.af,
        bc: state.bc,
        de: state.de,
        hl: state.hl,
        af_shadow: state.af_shadow,
        bc_shadow: state.bc_shadow,
        de_shadow: state.de_shadow,
        hl_shadow: state.hl_shadow,
        ix: state.ix,
        iy: state.iy,
        sp: state.sp,
        pc: state.pc,
    };
    let machine_state = State {
        i: state.i,
        r: state.r,
        iff1: state.iff1,
        iff2: state.iff2,
        im: u8::from(state.im),
        halted: state.halted,
        t_states: cpu.bus().t_states(),
    };
    (registers, machine_state)
}
