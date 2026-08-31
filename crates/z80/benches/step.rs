//! Throughput benchmark for the core's execute path.
//!
//! `docs/ARCHITECTURE.md` says performance is a non-goal: a 3.5 MHz Z80 needs about 70,000
//! T-states per frame, and any modern machine clears that by orders of magnitude. This
//! benchmark exists to keep it that way rather than to chase a number — the figure that
//! matters is the **realtime multiple**, and the useful question is whether it moves when
//! M2 quadruples the opcode count and M7 puts contention on every access.
//!
//! Two buses are measured because they bracket the real machine:
//!
//! - `FlatRam` — the cost of the core alone, with a bus that does nothing.
//! - `PagedRam` — an M7-shaped bus: a slot lookup per access, and per T-state a bank
//!   lookup on the driven address plus a stall calculation for contended banks. This is
//!   the one that predicts the shipped emulator, and the one whose cost the per-T-state
//!   `Bus::tick` contract makes visible.
//!
//! Run with `cargo bench -p z80`. The counter column reports **T-states per second**, so
//! the realtime multiple is that figure divided by the Z80's 3.5 MHz — no arithmetic on
//! the reader's part, and no estimate of "average T-states per instruction" to get wrong.

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
use z80::{Bus, Cpu, CpuState};

fn main() {
    divan::main();
}

/// Instructions executed per benchmark iteration.
const STEPS_PER_ITERATION: usize = 10_000;

/// The Z80 address space.
const ADDRESS_SPACE: usize = 0x1_0000;

/// Size of one memory bank on a 128K machine.
const BANK_SIZE: usize = 0x4000;

/// A hand-assembled un-prefixed program doing what real Spectrum code does: read memory,
/// ALU, write memory, walk pointers, call a subroutine that pushes and pops, compare, and
/// branch both ways. `HL` and `DE` are reset every outer pass so it can never walk into its
/// own code.
///
/// ```text
///   8000: 21 00 40   LD HL,0x4000
///   8003: 11 00 50   LD DE,0x5000
///   8006: 06 20      LD B,0x20
///   8008: 0E 00      LD C,0x00
///   800A: 7E         LD A,(HL)     <- inner loop
///   800B: 23         INC HL
///   800C: 81         ADD A,C
///   800D: 12         LD (DE),A
///   800E: 13         INC DE
///   800F: CD 20 80   CALL 0x8020
///   8012: 0C         INC C
///   8013: FE 40      CP 0x40
///   8015: 20 01      JR NZ,+1
///   8017: 00         NOP
///   8018: 10 F0      DJNZ inner
///   801A: C3 00 80   JP 0x8000
/// ```
const PROGRAM: &[u8] = &[
    0x21, 0x00, 0x40, // LD HL,4000
    0x11, 0x00, 0x50, // LD DE,5000
    0x06, 0x20, // LD B,20
    0x0E, 0x00, // LD C,00
    0x7E, // LD A,(HL)
    0x23, // INC HL
    0x81, // ADD A,C
    0x12, // LD (DE),A
    0x13, // INC DE
    0xCD, 0x20, 0x80, // CALL 8020
    0x0C, // INC C
    0xFE, 0x40, // CP 40
    0x20, 0x01, // JR NZ,+1
    0x00, // NOP
    0x10, 0xF0, // DJNZ -16
    0xC3, 0x00, 0x80, // JP 8000
];

/// `PUSH BC / PUSH HL / INC A / DEC A / POP HL / POP BC / RET`.
const SUBROUTINE: &[u8] = &[0xC5, 0xE5, 0x3C, 0x3D, 0xE1, 0xC1, 0xC9];

/// Where [`PROGRAM`] is assembled.
const PROGRAM_BASE: u16 = 0x8000;

/// Where [`SUBROUTINE`] is assembled.
const SUBROUTINE_BASE: u16 = 0x8020;

/// Well clear of both, so pushes never overwrite code.
const INITIAL_SP: u16 = 0x7FF0;

/// Load the workload into a freshly built address space.
fn load(memory: &mut [u8]) {
    let program_base = usize::from(PROGRAM_BASE);
    let subroutine_base = usize::from(SUBROUTINE_BASE);
    memory[program_base..program_base + PROGRAM.len()].copy_from_slice(PROGRAM);
    memory[subroutine_base..subroutine_base + SUBROUTINE.len()].copy_from_slice(SUBROUTINE);
}

/// The state the workload starts from.
fn start_state() -> CpuState {
    CpuState {
        pc: PROGRAM_BASE,
        sp: INITIAL_SP,
        ..CpuState::default()
    }
}

/// A bus that does nothing but hold bytes — the cost of the core alone.
struct FlatRam {
    memory: Vec<u8>,
    t_states: u64,
}

impl FlatRam {
    fn new() -> Self {
        let mut memory = vec![0; ADDRESS_SPACE];
        load(&mut memory);
        Self {
            memory,
            t_states: 0,
        }
    }
}

impl Bus for FlatRam {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        self.memory[usize::from(addr)]
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        self.memory[usize::from(addr)] = val;
    }

    #[inline]
    fn in_port(&mut self, _port: u16) -> u8 {
        0xFF
    }

    #[inline]
    fn out_port(&mut self, _port: u16, _val: u8) {}

    #[inline]
    fn tick(&mut self, _addr: u16) {
        self.t_states += 1;
    }
}

/// An M7-shaped bus: every access goes through a slot lookup, and every tick advances a
/// frame position and computes a contention delay for contended banks.
///
/// This is not the real machine — the real one is M5 and M7 work — but it has the same
/// per-access and per-tick shape, so it is the honest number to watch as the core grows.
struct PagedRam {
    banks: [Vec<u8>; 4],
    slots: [usize; 4],
    contended: [bool; 4],
    t_states: u64,
    frame_position: u32,
}

/// T-states in one 128K frame.
const FRAME_T_STATES: u32 = 70_908;

/// The ULA's eight-T-state stall pattern, indexed by frame position.
const CONTENTION_PATTERN: [u8; 8] = [6, 5, 4, 3, 2, 1, 0, 0];

impl PagedRam {
    fn new() -> Self {
        let mut flat = vec![0; ADDRESS_SPACE];
        load(&mut flat);
        let banks = [
            flat[0..BANK_SIZE].to_vec(),
            flat[BANK_SIZE..2 * BANK_SIZE].to_vec(),
            flat[2 * BANK_SIZE..3 * BANK_SIZE].to_vec(),
            flat[3 * BANK_SIZE..].to_vec(),
        ];
        Self {
            banks,
            slots: [0, 1, 2, 3],
            // On a 128K the contended banks are 1, 3, 5 and 7; here slots 1 and 3 stand in.
            contended: [false, true, false, true],
            t_states: 0,
            frame_position: 0,
        }
    }

    #[inline]
    fn locate(&self, addr: u16) -> (usize, usize) {
        let address = usize::from(addr);
        let slot = address / BANK_SIZE;
        (self.slots[slot], address % BANK_SIZE)
    }
}

impl Bus for PagedRam {
    #[inline]
    fn read(&mut self, addr: u16) -> u8 {
        let (bank, offset) = self.locate(addr);
        self.banks[bank][offset]
    }

    #[inline]
    fn write(&mut self, addr: u16, val: u8) {
        let (bank, offset) = self.locate(addr);
        self.banks[bank][offset] = val;
    }

    #[inline]
    fn in_port(&mut self, _port: u16) -> u8 {
        0xFF
    }

    #[inline]
    fn out_port(&mut self, _port: u16, _val: u8) {}

    /// Per T-state, this is where an M7 machine does its contention arithmetic: work out
    /// which bank the driven address lands in and, if that bank is contended, how many
    /// T-states the ULA would stall for at this frame position.
    #[inline]
    fn tick(&mut self, addr: u16) {
        self.t_states += 1;
        self.frame_position = (self.frame_position + 1) % FRAME_T_STATES;
        let slot = usize::from(addr) / BANK_SIZE;
        if self.contended[slot] {
            let _delay = black_box(CONTENTION_PATTERN[(self.frame_position % 8) as usize]);
        }
    }
}

/// T-states one iteration executes.
///
/// The workload is deterministic and always starts from the same state, so this is a
/// constant — measured by running it rather than estimated from an instruction mix.
fn t_states_per_iteration() -> u64 {
    let mut cpu = Cpu::new(FlatRam::new());
    cpu.set_state(start_state());
    for _ in 0..STEPS_PER_ITERATION {
        cpu.step();
    }
    cpu.bus().t_states
}

#[divan::bench]
fn flat_bus(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(t_states_per_iteration()))
        .with_inputs(|| {
            let mut cpu = Cpu::new(FlatRam::new());
            cpu.set_state(start_state());
            cpu
        })
        .bench_local_values(|mut cpu| {
            for _ in 0..STEPS_PER_ITERATION {
                black_box(cpu.step());
            }
            black_box(cpu.bus().t_states)
        });
}

#[divan::bench]
fn paged_contended_bus(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(t_states_per_iteration()))
        .with_inputs(|| {
            let mut cpu = Cpu::new(PagedRam::new());
            cpu.set_state(start_state());
            cpu
        })
        .bench_local_values(|mut cpu| {
            for _ in 0..STEPS_PER_ITERATION {
                black_box(cpu.step());
            }
            black_box(cpu.bus().t_states)
        });
}
