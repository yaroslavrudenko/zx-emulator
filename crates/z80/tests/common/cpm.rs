//! A CP/M program shell: 64K of flat RAM, the program at `0x0100`, and the two BDOS calls
//! a CRC-checking exerciser needs.
//!
//! # Why this is not an extension of `machine.rs`
//!
//! The FUSE corpus and the `zex` exercisers are different *shapes* of oracle, not two sizes
//! of the same one.
//!
//! | | FUSE ([`super::machine`]) | `zex` (this module) |
//! |---|---|---|
//! | Unit of work | one instruction | one whole program, ~10⁸ instructions |
//! | Verdict from | comparing final state to a recorded expectation | the program's own CRC, printed as `OK` / `ERROR` |
//! | What it proves | registers, flags, memory, **and per-T-state bus addresses** | instruction *semantics* at scale, including sequences |
//! | Bus | logs every tick and transfer | records nothing — see below |
//!
//! Bending one into the other would give both a bus that is wrong for the other's job.
//!
//! # The bus deliberately records nothing
//!
//! [`super::machine::TestBus`] pushes one `u16` per T-state, which is exactly right for a
//! 30-T-state vector and catastrophic for a run of this length: at the measured ~10¹⁰
//! T-states that log alone would be tens of gigabytes. Contention is not what this gate
//! decides — FUSE owns that, per-T-state and per-address — so [`FlatBus::tick`] is empty
//! and the T-state total comes from what [`z80::Cpu::step`] returns.
//!
//! That total is **the core's own count, not an independent oracle**, and it is reported
//! rather than asserted. The oracle here is the CRC the program computes over its own
//! results, which no part of this harness can influence.
//!
//! # BDOS is trapped, not emulated
//!
//! CP/M is entered by `CALL 0x0005` with the function number in `C`. Rather than emulate
//! CP/M, `0x0005` holds a `RET` and the run loop notices `PC` arriving there, services the
//! call from the current register state, and lets the `RET` return. The exercisers use
//! exactly two functions — verified against the binary, which contains six `LD C,9` and one
//! `LD C,2` before its single `CALL 0x0005`.

use std::collections::BTreeMap;

use z80::{Bus, Cpu, CpuState, StepError};

/// The Z80 address space. Every `u16` is in range by construction.
pub const MEMORY_SIZE: usize = 0x1_0000;

/// Where CP/M loads a `.COM` image, and therefore the entry point.
pub const PROGRAM_ORIGIN: u16 = 0x0100;

/// The BDOS entry point. A program reaches CP/M by calling here.
pub const BDOS_ENTRY: u16 = 0x0005;

/// Warm boot. A finished program jumps here, which is this harness's termination signal —
/// not a crash.
pub const WARM_BOOT: u16 = 0x0000;

/// The initial stack pointer, and the value stored at `0x0006`.
///
/// `0x0006` is where CP/M keeps the BDOS entry address, which by convention is also the
/// first byte a program must not use — so programs read it as "top of memory" and set their
/// stack from it. `zexdoc` does exactly that (`LD HL,(0x0006)` / `LD SP,HL` are its first
/// two instructions), so this value must be sane whether or not anyone sets `SP` explicitly.
pub const INITIAL_SP: u16 = 0xF000;

/// BDOS function 2 — write the character in `E` to the console.
pub const BDOS_CONSOLE_OUT: u8 = 2;

/// BDOS function 9 — write the `$`-terminated string at `DE` to the console.
pub const BDOS_PRINT_STRING: u8 = 9;

const RET: u8 = 0xC9;
const STRING_TERMINATOR: u8 = b'$';

/// A `.COM` image that cannot be run, as opposed to one that runs and fails.
///
/// The realistic failure is not a subtly corrupt program: it is a fetch that saved an HTTP
/// error page under a `.com` name. Rejecting it here names the cause, instead of leaving a
/// mysterious immediate warm boot to be diagnosed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// A zero-byte file — the shape a failed download leaves behind.
    #[error("the image is empty")]
    Empty,
    /// Loading it at `0x0100` would run past the top of the address space.
    #[error(
        "the image is {len} bytes, which does not fit between {origin:#06x} and the top of \
         the 64K address space",
        origin = PROGRAM_ORIGIN
    )]
    TooLarge {
        /// The image's length in bytes.
        len: usize,
    },
}

/// 64K of RAM and nothing else — no tick log, no transfer log.
pub struct FlatBus {
    memory: Vec<u8>,
    port_accesses: u64,
}

impl Default for FlatBus {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatBus {
    pub fn new() -> Self {
        Self {
            memory: vec![0; MEMORY_SIZE],
            port_accesses: 0,
        }
    }

    /// Read without ticking — for the BDOS string walk and for assertions.
    pub fn peek(&self, addr: u16) -> u8 {
        self.memory[usize::from(addr)]
    }

    /// Write without ticking — for laying out the program and CP/M's page zero.
    pub fn poke(&mut self, addr: u16, value: u8) {
        self.memory[usize::from(addr)] = value;
    }

    /// How many `IN`/`OUT` instructions reached the bus.
    ///
    /// A CP/M exerciser performs none. A non-zero count therefore means the core decoded
    /// something as an I/O instruction that is not one, which is a real defect that the
    /// CRC alone might not localise.
    pub fn port_accesses(&self) -> u64 {
        self.port_accesses
    }

    /// Copy a `.COM` image to [`PROGRAM_ORIGIN`].
    pub fn load_program(&mut self, image: &[u8]) -> Result<(), ImageError> {
        if image.is_empty() {
            return Err(ImageError::Empty);
        }
        let origin = usize::from(PROGRAM_ORIGIN);
        if origin + image.len() > MEMORY_SIZE {
            return Err(ImageError::TooLarge { len: image.len() });
        }
        self.memory[origin..origin + image.len()].copy_from_slice(image);
        Ok(())
    }
}

impl Bus for FlatBus {
    fn read(&mut self, addr: u16) -> u8 {
        self.memory[usize::from(addr)]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.memory[usize::from(addr)] = val;
    }

    /// An unattached Z80 `IN` reads the high half of the address bus — the same rule
    /// [`super::machine::TestBus`] derives from the corpus.
    fn in_port(&mut self, port: u16) -> u8 {
        self.port_accesses += 1;
        (port >> 8) as u8
    }

    fn out_port(&mut self, _port: u16, _val: u8) {
        self.port_accesses += 1;
    }

    /// Empty on purpose. See the module documentation.
    fn tick(&mut self, _addr: u16) {}
}

/// Why the run loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// `PC` reached [`WARM_BOOT`] — the program finished, which is the only success.
    WarmBoot,
    /// The instruction budget ran out. The program is looping, so the run is a failure
    /// with a diagnosis rather than a hung suite with none.
    InstructionLimit,
    /// The core had a fault recorded when the program warm-booted.
    ///
    /// # This is a formality, not a guard, and the distinction is the point
    ///
    /// `Cpu::begin_operation` clears the fault at the start of **every** `step`, so a check
    /// made after the loop can only ever observe a fault left by the *final* instruction.
    /// It is not a sweep of the run, and calling it one would be this project's recurring
    /// defect: a comment claiming a protection the code does not provide.
    ///
    /// It is kept anyway, and not moved into the loop, for two reasons that point the same
    /// way. Nothing here can set a fault at all — the only condition that does is an accepted
    /// mode-0 interrupt, and this harness never offers one — so an in-loop check would cost
    /// 5.8 billion branches to catch a condition that cannot arise. And an outcome type with
    /// no way to say "the core complained" is how a complaint gets silently dropped if that
    /// ever changes.
    Fault(StepError),
}

/// A CPU wired to a [`FlatBus`], with CP/M's page zero laid out and a program loaded.
pub struct CpmMachine {
    cpu: Cpu<FlatBus>,
    console: String,
    bdos_calls: BTreeMap<u8, u64>,
    instructions: u64,
    t_states: u64,
}

impl CpmMachine {
    /// Build a machine with `image` loaded at [`PROGRAM_ORIGIN`] and ready to run.
    pub fn load(image: &[u8]) -> Result<Self, ImageError> {
        let mut bus = FlatBus::new();
        bus.load_program(image)?;

        // Page zero, the two entries a `.COM` program is entitled to assume.
        //
        // `0x0000` is left as `NOP` rather than given a `JP`: the run loop treats arriving
        // there as termination and never executes it, and putting a real instruction there
        // would only hide a failure to notice.
        bus.poke(BDOS_ENTRY, RET);
        bus.poke(BDOS_ENTRY + 1, (INITIAL_SP & 0xFF) as u8);
        bus.poke(BDOS_ENTRY + 2, (INITIAL_SP >> 8) as u8);

        let mut cpu = Cpu::new(bus);
        cpu.set_state(CpuState {
            pc: PROGRAM_ORIGIN,
            sp: INITIAL_SP,
            ..CpuState::default()
        });

        Ok(Self {
            cpu,
            console: String::new(),
            bdos_calls: BTreeMap::new(),
            instructions: 0,
            t_states: 0,
        })
    }

    /// Run until the program warm-boots, or until `max_instructions` have executed.
    ///
    /// The budget is what stops a broken core from hanging the suite. A hang gives no
    /// diagnosis and blocks CI; an exhausted budget gives the console output so far, which
    /// names the last test group the program reached.
    pub fn run(&mut self, max_instructions: u64) -> Outcome {
        while self.instructions < max_instructions {
            let pc = self.cpu.state().pc;
            if pc == WARM_BOOT {
                return self.finished();
            }
            if pc == BDOS_ENTRY {
                self.service_bdos();
            }
            self.t_states += u64::from(self.cpu.step());
            self.instructions += 1;
        }
        Outcome::InstructionLimit
    }

    /// The console text the program produced.
    pub fn console(&self) -> &str {
        &self.console
    }

    /// How many instructions ran.
    pub fn instructions(&self) -> u64 {
        self.instructions
    }

    /// The T-state total, summed from what [`z80::Cpu::step`] returned.
    ///
    /// **Not an independent account** of anything — it is the core's own counter, reported
    /// because the scale is interesting and never asserted against.
    pub fn t_states(&self) -> u64 {
        self.t_states
    }

    /// How many `IN`/`OUT` instructions reached the bus.
    pub fn port_accesses(&self) -> u64 {
        self.cpu.bus().port_accesses()
    }

    /// BDOS function number -> how many times it was called.
    pub fn bdos_calls(&self) -> &BTreeMap<u8, u64> {
        &self.bdos_calls
    }

    /// The verdict at warm boot. See [`Outcome::Fault`] for what this can and cannot see.
    fn finished(&self) -> Outcome {
        match self.cpu.fault() {
            Some(fault) => Outcome::Fault(fault),
            None => Outcome::WarmBoot,
        }
    }

    /// Service the call `PC` has just arrived at. The `RET` at [`BDOS_ENTRY`] returns.
    fn service_bdos(&mut self) {
        let state = self.cpu.state();
        // C is the function number, E the character argument, DE the string address.
        let function = state.bc as u8;
        *self.bdos_calls.entry(function).or_default() += 1;
        match function {
            BDOS_CONSOLE_OUT => self.console.push(char::from(state.de as u8)),
            BDOS_PRINT_STRING => self.print_string(state.de),
            // Every other function is ignored, deliberately: the exercisers call none, and
            // the count above records it if that ever stops being true.
            _ => {}
        }
    }

    fn print_string(&mut self, start: u16) {
        let mut addr = start;
        // Bounded by the address space: an unterminated string would otherwise wrap
        // forever, turning a corpus defect into a hung suite.
        for _ in 0..MEMORY_SIZE {
            let byte = self.cpu.bus().peek(addr);
            if byte == STRING_TERMINATOR {
                return;
            }
            self.console.push(char::from(byte));
            addr = addr.wrapping_add(1);
        }
        panic!(
            "BDOS function {BDOS_PRINT_STRING}: the string at {start:#06x} has no \
             {:?} terminator anywhere in the 64K address space",
            char::from(STRING_TERMINATOR)
        );
    }
}
