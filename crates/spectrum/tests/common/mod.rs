//! Shared fixtures for the machine gates — the ONE home for every helper they use.
//!
//! # What these gates are for
//!
//! `crates/spectrum/src` carries unit tests for each part in isolation, and the boot gate
//! carries the whole machine. Neither is what these files are: **the boot gate was measured,
//! by mutation, to grade almost nothing.** `/INT` never asserted, a keyboard reporting every
//! key held, a writable ROM slot, contention deleted outright, and contention off by one
//! T-state all left the ROM reaching `© 1982 Sinclair Research Ltd` — the last of them
//! producing byte-identical output. Only a corrupted screen address layout turned it red.
//!
//! So these gates assert those five properties **through the machine** — through `Spectrum`,
//! its CPU, and its bus — rather than through the part that implements them. Each file names
//! in its own documentation what it grades and what remains ungraded, because
//! `docs/MACHINE.md` asks for the second list explicitly and this project's recorded failure
//! is reporting the absence of a distinguishing test as evidence of correctness.
//!
//! # Positioning the clock
//!
//! Contention and the interrupt window are both functions of *where in the frame* an access
//! falls, so nearly everything here needs the machine at an exact `frame_t_state`. There is
//! no public setter for the clock and there should not be — the clock is the bus's, and a
//! test that could move it directly would not be testing the machine. [`advance_to`] instead
//! *runs* the machine to the position, out of uncontended memory where every instruction
//! costs its nominal length.

// Each gate is its own binary and compiles this whole module while using a subset of it —
// the standard `tests/common` situation, matching `crates/z80/tests/common/mod.rs`. The
// allow is permanent, so `#[expect]` would itself warn in the binaries where an item is used.
#![allow(dead_code)]

use spectrum::keyboard::{HALF_ROWS, KEYS_PER_HALF_ROW};
use spectrum::memory::PAGE_SIZE;
use spectrum::timing::T_STATES_PER_FRAME;
use spectrum::{Key, Spectrum};
use z80::{Bus, CpuState, InterruptMode};

// ---------------------------------------------------------------------------
// Where things live in the address space
// ---------------------------------------------------------------------------

/// Where [`advance_to`] assembles its clock-positioning prologue.
///
/// Slot 2 — RAM bank 2, which a 48K never contends — so every instruction in the prologue
/// costs exactly its nominal length and the landing position is arithmetic rather than a
/// simulation of the contention model the tests are trying to grade.
pub const PROLOGUE: u16 = 0x8000;

/// Where [`advance_to_absolute`] assembles the sled it re-runs to cross whole frames.
///
/// Also slot 2, and clear of [`PROLOGUE`]'s reach: the fine half of that positioning is never
/// more than two sleds long, so its prologue cannot grow past ~1 KB from `0x8000`.
pub const SLED: u16 = 0x8800;

/// `NOP`s in that sled.
const SLED_NOPS: u64 = 256;

/// T-states one lap of it costs, out of uncontended memory.
const SLED_T_STATES: u64 = SLED_NOPS * NOP_T_STATES as u64;

/// Uncontended RAM for a program under test, clear of the prologue.
///
/// Slot 3, RAM bank 0. The prologue can reach ~3.6 KB from [`PROLOGUE`] and must never
/// collide with the program a gate is measuring.
pub const UNCONTENDED_CODE: u16 = 0xC000;

/// Contended RAM for the same program: the screen bank.
///
/// Bank 5 in slot 1 is the only bank a 48K contends, so this address and
/// [`UNCONTENDED_CODE`] differ in exactly one property — which is what makes a difference
/// between two runs attributable to contention and to nothing else.
pub const CONTENDED_CODE: u16 = 0x4000;

// ---------------------------------------------------------------------------
// Instructions the fixtures assemble, with their uncontended cost
// ---------------------------------------------------------------------------

/// `NOP`: one M1 opcode fetch, one byte, four T-states.
pub const NOP: u8 = 0x00;

/// T-states a [`NOP`] costs when fetched from uncontended memory.
pub const NOP_T_STATES: u32 = 4;

/// `HALT`: the CPU stops fetching instructions until an interrupt arrives.
pub const HALT: u8 = 0x76;

/// `LD A,0`: an M1 fetch and an operand read — two bytes, seven T-states.
const LD_A_0: [u8; 2] = [0x3E, 0x00];

/// T-states [`LD_A_0`] costs when fetched from uncontended memory.
const LD_A_0_T_STATES: u32 = 7;

/// The largest T-state total that four and seven cannot compose.
///
/// `gcd(4, 7) = 1`, so all but finitely many totals are reachable as `4a + 7b`; the largest
/// that is not is their Frobenius number, `4 * 7 - 4 - 7`. Written out rather than inlined
/// because it is the exact reason [`advance_to`] has a lower bound at all.
const UNREACHABLE_ABOVE: u32 = NOP_T_STATES * LD_A_0_T_STATES - NOP_T_STATES - LD_A_0_T_STATES;

// ---------------------------------------------------------------------------
// Machines
// ---------------------------------------------------------------------------

/// A 16 KB ROM image whose every byte is its own low address byte.
///
/// A recognisable pattern rather than zeros, so a ROM byte, an unwritten RAM byte and a byte
/// a test wrote are three visibly different things. Nothing executes it — every gate here
/// sets `PC` into RAM first.
#[must_use]
pub fn pattern_rom() -> Vec<u8> {
    (0..PAGE_SIZE)
        .map(|address| (address & 0xFF) as u8)
        .collect()
}

/// A 48K holding [`pattern_rom`], at the start of frame zero with interrupts disabled.
#[must_use]
pub fn machine() -> Spectrum {
    Spectrum::new(&pattern_rom()).expect("a page-sized ROM")
}

/// The real Sinclair 48K ROM, or `None` when the shared policy says this run may skip it.
///
/// The one gate here that uses it needs `testdata/roms/48.rom`, and absence follows the same
/// rule as every other corpus in this workspace — see `crates/testsupport`. That rule is why
/// this returns an `Option` rather than reading the file directly: a missing ROM must move
/// the pass/fail surface, never print a notice libtest then captures.
#[must_use]
pub fn sinclair_rom() -> Option<Vec<u8>> {
    // Unconditionally, not only on the absent path: an obsolete spelling must be an error in
    // *every* gate, or a CI file still exporting one is silently ignored by whichever gate
    // happens to find its corpus present. This mirrors `vectors::corpus_or_skip`.
    testsupport::reject_obsolete_env();

    let path = testsupport::testdata_dir().join("roms").join("48.rom");
    if !path.is_file() {
        testsupport::skip_absent_corpus("the Sinclair 48K ROM", &path);
        return None;
    }
    Some(std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display())))
}

// ---------------------------------------------------------------------------
// Driving a machine
// ---------------------------------------------------------------------------

/// T-states elapsed since power-on, across frame boundaries.
///
/// The machine reports its position as a frame count and an offset because everything that
/// consults the clock is frame-relative; a measurement that may cross a boundary wants the
/// absolute figure.
#[must_use]
pub fn elapsed(machine: &Spectrum) -> u64 {
    machine.frames() * u64::from(T_STATES_PER_FRAME) + u64::from(machine.frame_t_state())
}

/// Store `bytes` starting at `address`.
///
/// Goes through the machine's own memory, so a caller that aims this at a ROM slot gets the
/// hardware's behaviour — the write is discarded — rather than a test-only back door.
pub fn write_program(machine: &mut Spectrum, address: u16, bytes: &[u8]) {
    for (offset, byte) in bytes.iter().enumerate() {
        let target = address
            .checked_add(u16::try_from(offset).expect("a program shorter than the address space"))
            .expect("a program that does not run off the top of memory");
        machine.memory_mut().write(target, *byte);
    }
}

/// Edit the CPU's registers in place.
///
/// The one place the read-modify-write of a [`CpuState`] is written. `spectrum` exposes the
/// register file only as a whole snapshot — `crates/z80` keeps `Registers` private, which is
/// the point of the crate boundary — so every gate that needs one register has to fetch all
/// twenty fields, change one, and put them back. Doing that inline is three lines each time
/// and was already copied four ways across these gates before this existed.
pub fn with_cpu_state(machine: &mut Spectrum, edit: impl FnOnce(&mut CpuState)) {
    let mut state = machine.cpu_state();
    edit(&mut state);
    machine.set_cpu_state(state);
}

/// Point `PC` at `address`, leaving every other register alone.
pub fn set_pc(machine: &mut Spectrum, address: u16) {
    with_cpu_state(machine, |state| state.pc = address);
}

/// Enable both interrupt flip-flops, select `mode`, and give the CPU a usable stack.
///
/// Call this **after** [`advance_to`]: positioning runs the machine past the interrupt
/// window at the top of the frame, and a machine with interrupts already on would vector out
/// of the prologue instead of finishing it.
pub fn enable_interrupts(machine: &mut Spectrum, mode: InterruptMode) {
    with_cpu_state(machine, |state| {
        state.iff1 = true;
        state.iff2 = true;
        state.im = mode;
        state.sp = 0xFF00;
    });
}

/// Run a fresh machine to exactly `target` T-states into frame zero.
///
/// Assembles a prologue of `NOP`s and `LD A,0`s in uncontended RAM and executes it. Four and
/// seven are coprime, so every total above [`UNREACHABLE_ABOVE`] is composable — and the two
/// instruction lengths are the two the Z80 happens to offer at the bottom of its range, which
/// is why the bound is what it is rather than a chosen convenience.
///
/// The landing position is **asserted**, not assumed. If contention ever reached bank 2, or
/// an instruction's nominal length changed, the prologue would silently land somewhere else
/// and every measurement taken from it would be quietly wrong.
///
/// # Panics
///
/// If `machine` has already run, or if `target` is a total four and seven cannot compose.
pub fn advance_to(machine: &mut Spectrum, target: u32) {
    assert_eq!(
        elapsed(machine),
        0,
        "advance_to positions a machine that has not run yet"
    );
    if target == 0 {
        return;
    }
    assert!(
        target > UNREACHABLE_ABOVE,
        "{target} is at or below {UNREACHABLE_ABOVE}, the largest total four and seven \
         cannot compose, so no prologue of 4- and 7-T-state instructions lands on it \
         exactly; a gate needing an offset this small steps the machine instead"
    );

    // 4a + 7b == target. Modulo 4 that is 3b ≡ target, and 3 is its own inverse mod 4, so b
    // is pinned first and a follows exactly.
    let long = (3 * target) % 4;
    let short = (target - LD_A_0_T_STATES * long) / NOP_T_STATES;

    let mut program = Vec::with_capacity(LD_A_0.len() * long as usize + short as usize);
    for _ in 0..long {
        program.extend_from_slice(&LD_A_0);
    }
    program.resize(program.len() + short as usize, NOP);
    write_program(machine, PROLOGUE, &program);

    set_pc(machine, PROLOGUE);
    for _ in 0..(long + short) {
        machine.step();
    }

    assert_eq!(
        elapsed(machine),
        u64::from(target),
        "the prologue must land exactly on the requested frame position: {long} x LD A,0 \
         and {short} x NOP out of uncontended RAM should cost {target} T-states"
    );
}

/// Run a fresh machine to exactly `target` T-states after power-on, however many frames away.
///
/// [`advance_to`] assembles **one straight-line prologue**, so its reach is bounded by the RAM
/// it can write: a frame is [`T_STATES_PER_FRAME`] T-states and would need some seventeen
/// thousand instructions, which is more than the 16 KB bank it assembles into. Everything at
/// or beyond a frame boundary is therefore out of its reach — which is why every gate in this
/// directory before `frame_boundary.rs` measured inside frame zero.
///
/// This closes the gap by **repeating** a fixed-cost sled rather than by assembling a longer
/// one: [`SLED_NOPS`] `NOP`s at [`SLED`], executed as that many steps. Out of bank 2 nothing
/// contends, so each lap costs exactly `4 * SLED_NOPS` T-states and the landing stays
/// arithmetic rather than a simulation of the contention model these gates are grading.
///
/// The fine positioning runs **first**, so [`advance_to`]'s own landing assertion still fires
/// against an unrun machine; the laps then carry the remainder, which is always a whole
/// multiple of the sled. The final position is asserted here as well, for the same reason
/// [`advance_to`] asserts its own.
///
/// Interrupts must still be enabled *after* this returns: the laps run through the interrupt
/// window at the top of every frame they cross, and a machine with `iff1` set would vector out
/// of the sled instead of finishing it.
///
/// # Panics
///
/// If `machine` has already run, or if the machine does not land exactly on `target`.
pub fn advance_to_absolute(machine: &mut Spectrum, target: u64) {
    assert_eq!(
        elapsed(machine),
        0,
        "advance_to_absolute positions a machine that has not run yet"
    );

    let mut laps = target / SLED_T_STATES;
    let mut remainder = target - laps * SLED_T_STATES;
    // A remainder inside the Frobenius gap is not composable, so borrow a whole lap: the
    // prologue then assembles `remainder + SLED_T_STATES`, which is far above the bound.
    if laps > 0 && remainder <= u64::from(UNREACHABLE_ABOVE) {
        laps -= 1;
        remainder += SLED_T_STATES;
    }

    advance_to(
        machine,
        u32::try_from(remainder).expect("a remainder below two sleds"),
    );

    if laps > 0 {
        write_program(machine, SLED, &vec![NOP; SLED_NOPS as usize]);
        for _ in 0..laps {
            set_pc(machine, SLED);
            for _ in 0..SLED_NOPS {
                machine.step();
            }
        }
    }

    assert_eq!(
        elapsed(machine),
        target,
        "positioning must land exactly on {target}: {remainder} T-states of prologue and \
         {laps} sleds of {SLED_T_STATES}"
    );
}

/// Run `steps` instructions from `address` and report what the machine's clock charged.
///
/// The clock, never the sum of what `step` returns: `docs/MACHINE.md` Decision 1 is a
/// measurement, and contention is added on the bus's side where `step`'s return cannot see
/// it.
pub fn cost_of_running(machine: &mut Spectrum, address: u16, steps: usize) -> u64 {
    set_pc(machine, address);
    let before = elapsed(machine);
    for _ in 0..steps {
        machine.step();
    }
    elapsed(machine) - before
}

// ---------------------------------------------------------------------------
// Watching a run take interrupts
// ---------------------------------------------------------------------------

/// One accepted interrupt, as it looks from outside the CPU.
///
/// Everything here is sampled at the moment of acceptance rather than reconstructed
/// afterwards, which matters for the first two fields: the acknowledge moves the clock, so a
/// position read *after* the step is the position the handler starts at and not the position
/// the offer was made at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedInterrupt {
    /// The frame the offer was made in.
    pub frame: u64,
    /// How far into that frame — necessarily inside the ULA's window.
    pub offset: u32,
    /// `BC` exactly as the last completed iteration left it.
    pub bc: u16,
    /// The address the acknowledge pushed.
    pub return_address: u16,
    /// T-states the acknowledge itself charged.
    pub charged: u32,
}

/// What one run of a repeating instruction produced.
#[derive(Debug)]
pub struct InterruptedRun {
    /// Every interrupt taken, in order.
    pub accepted: Vec<AcceptedInterrupt>,
    /// T-states the whole run cost, taken from the clock rather than from `step`'s return.
    pub cost: u64,
    /// Where the machine stands afterwards, as a frame and an offset.
    pub end: (u64, u32),
}

/// Step until `finished`, recording every interrupt taken on the way.
///
/// An acceptance is a step after which `PC` is `handler`: [`z80::Cpu::interrupt`] executes no
/// instruction of its own, so the register file is exactly as the last completed iteration
/// left it — which is the state a gate on an interrupted block instruction is about.
///
/// The predicate rather than a fixed "the counter reached zero" test, because the four block
/// families do not agree on what the counter is: the transfers and the compares count `BC`
/// down to zero, and the input and output families count `B` while leaving `C` alone. What
/// they do agree on is that only the pass which exhausts the counter steps `PC` past the
/// instruction, so every caller's predicate is some reading of `PC`.
///
/// # Panics
///
/// If `budget` steps pass without `finished` returning true — a hang would otherwise be
/// indistinguishable from a slow test.
pub fn run_recording_interrupts(
    machine: &mut Spectrum,
    handler: u16,
    budget: usize,
    finished: impl Fn(&CpuState) -> bool,
) -> InterruptedRun {
    let before = elapsed(machine);
    let mut accepted = Vec::new();

    for _ in 0..budget {
        // Taken before the step: see `AcceptedInterrupt`.
        let offered_at = (machine.frames(), machine.frame_t_state());
        let charged = machine.step();
        let state = machine.cpu_state();
        if state.pc == handler {
            accepted.push(AcceptedInterrupt {
                frame: offered_at.0,
                offset: offered_at.1,
                bc: state.bc,
                return_address: u16::from_le_bytes([
                    machine.memory().read(state.sp),
                    machine.memory().read(state.sp.wrapping_add(1)),
                ]),
                charged,
            });
        }
        if finished(&state) {
            return InterruptedRun {
                accepted,
                cost: elapsed(machine) - before,
                end: (machine.frames(), machine.frame_t_state()),
            };
        }
    }
    panic!("the instruction under test did not finish within {budget} steps");
}

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

/// Read an I/O port through the bus, exactly as the CPU's `IN` does.
pub fn read_port(machine: &mut Spectrum, port: u16) -> u8 {
    machine.ula_mut().in_port(port)
}

/// Write to memory through the **bus**, the path an executing instruction takes.
///
/// Distinct from [`write_program`], which goes straight to the address space. Both must
/// honour ROM write-protection, and a gate that only checked one of them would leave the
/// other free to differ.
pub fn write_through_the_bus(machine: &mut Spectrum, address: u16, value: u8) {
    machine.ula_mut().write(address, value);
}

// ---------------------------------------------------------------------------
// The keyboard membrane
// ---------------------------------------------------------------------------

/// The byte a half-row read returns with nothing pressed on it.
///
/// The five key bits idle high, and the ULA supplies bits 5–7 that the keyboard does not
/// drive: bits 5 and 7 float high, and bit 6 is the `EAR` input, low with nothing plugged
/// into the tape socket. Written as the literal a real `IN A,(0xFE)` returns rather than
/// composed from the crate's own constants, because it is the expectation.
pub const IDLE_HALF_ROW: u8 = 0xBF;

/// The bits of a half-row read that the keyboard does not drive.
pub const UNDRIVEN_BITS: u8 = 0xE0;

/// The 40-key membrane, as the published 48K matrix gives it — **not** as the crate models
/// it.
///
/// Each row is a **literal port** and the five keys on it in **data-bit order**, bit 0 first.
/// Nothing here is computed from `spectrum`: not the port, not the bit, not the grouping. A
/// key's position in this table *is* its claimed wiring, and the ports are written out so a
/// reader can check all eight against the published matrix by eye.
///
/// That independence is the entire value of the table, and it was measured to be missing
/// before it existed. `keyboard.rs`'s own `every_key_is_visible_to_a_scan_of_its_own_half_row`
/// derives **both** the port it scans and the value it expects from `Key::position()`, the
/// function under test — so it proves `read()` and `position()` agree and can say nothing
/// about whether either matches hardware. Under a review that swapped half-rows, then rotated
/// six of them, then rotated bits inside two more, **38 of the 40 keys moved** and the suite
/// stayed at 72 passed with a green boot gate.
///
/// The digits run backwards on the `0xEFFE` row because the two number rows meet in the
/// middle of the keyboard. That is a real property of the membrane, and exactly the sort of
/// thing a derived table would get "right" in both places at once.
pub const MEMBRANE: [(u16, [Key; KEYS_PER_HALF_ROW]); HALF_ROWS] = [
    (0xFEFE, [Key::CapsShift, Key::Z, Key::X, Key::C, Key::V]),
    (0xFDFE, [Key::A, Key::S, Key::D, Key::F, Key::G]),
    (0xFBFE, [Key::Q, Key::W, Key::E, Key::R, Key::T]),
    (
        0xF7FE,
        [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5],
    ),
    (
        0xEFFE,
        [Key::Num0, Key::Num9, Key::Num8, Key::Num7, Key::Num6],
    ),
    (0xDFFE, [Key::P, Key::O, Key::I, Key::U, Key::Y]),
    (0xBFFE, [Key::Enter, Key::L, Key::K, Key::J, Key::H]),
    (
        0x7FFE,
        [Key::Space, Key::SymbolShift, Key::M, Key::N, Key::B],
    ),
];

/// The two keys whose port and bit are documented outside this project altogether.
///
/// `X` at `0xFEFE` bit 2 and `ENTER` at `0xBFFE` bit 0 are the only absolute anchors that
/// appear in the Spectrum literature as concrete `(port, bit)` pairs rather than as a
/// diagram. Asserted separately from [`MEMBRANE`] so that a table transcribed wrongly in one
/// consistent motion still has two points it cannot slide past.
pub const ANCHORS: [(Key, u16, u8); 2] = [(Key::X, 0xFEFE, 0x04), (Key::Enter, 0xBFFE, 0x01)];

/// The port that selects exactly the half-row at `row` in [`MEMBRANE`], and no other.
#[must_use]
pub fn half_row_port(row: usize) -> u16 {
    MEMBRANE[row].0
}

/// The keys on the half-row at `row` in [`MEMBRANE`], in data-bit order.
#[must_use]
pub fn half_row_keys(row: usize) -> [Key; KEYS_PER_HALF_ROW] {
    MEMBRANE[row].1
}

/// A port that selects **every** half-row at once — the ROM's "is anything pressed" scan.
pub const ALL_HALF_ROWS_PORT: u16 = 0x00FE;
