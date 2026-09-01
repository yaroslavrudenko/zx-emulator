//! Gate: an `NMI` arriving mid-loop, and the four rules that make it a different animal from
//! the frame interrupt.
//!
//! # Why this exists
//!
//! `docs/STATUS.md` lists `NMI` among the properties nothing drives through a machine, and
//! `block_interrupt.rs` names it in its own *not graded here* list: *"It is always accepted and
//! takes a different path."* Both halves of that sentence are ungraded claims, and each is a
//! rule the maskable interrupt does not share:
//!
//! | | frame interrupt | `NMI` |
//! |---|---|---|
//! | gated by `IFF1` | yes | **no** |
//! | gated by the ULA's 32 T-state window | yes | **no** — nothing on a 48K's board drives the line at all |
//! | vector | `0x0038`, or `(I << 8) \| 0xFF` | **`0x0066`**, fixed |
//! | flip-flops | both cleared | `IFF1` **copied into `IFF2`**, then cleared |
//! | returned from by | `RET` | **`RETN`**, which restores `IFF1` from `IFF2` |
//! | can be contended | **never** — see below | **yes** |
//!
//! Every row is a place where a plausible implementation of one would be wrong for the other:
//! an acceptance routed through the maskable path would decline with `IFF1` clear, would clear
//! `IFF2` and so make `RETN` restore the wrong value, and would land on the wrong vector.
//!
//! # It cannot be driven through `Spectrum`, and that is the machine being right
//!
//! There is no `Spectrum::nmi`, and there should not be. On a real 48K nothing on the board
//! drives `/NMI` — the pin goes to the edge connector, for an interface or a reset button to
//! pull — so a `Spectrum` as assembled here can *never* take one. This file therefore drives a
//! `Cpu<Ula>` directly, exactly as `io_contention.rs` does and for the same reason: it is the
//! real bus, the real clock and the real contention model, with the one event the machine has
//! no wire for supplied by the test. The consequence is worth stating plainly rather than
//! leaving implied: **nothing in the assembled machine exercises `Cpu::nmi` at all**, and this
//! is the only gate anywhere that drives it through a Spectrum bus.
//!
//! # The contended row is the sharp one
//!
//! `interrupt_contended_block.rs` establishes that a *frame* interrupt's acknowledge can never
//! be contended: the ULA's window is the frame's first 32 T-states and the first contended
//! T-state is 14335, so every acceptance happens in the top border. **That argument does not
//! transfer**, because it rests entirely on the window — and an `NMI` has none. It can be
//! raised at any T-state of the frame, including the middle of the display area, where its two
//! stack writes stall like anything else. This file drives exactly that, with a control at the
//! same frame position whose only difference is which bank the stack is in.
//!
//! # How the expected values were obtained
//!
//! The streams were recorded off a real `Cpu` through a bus that logs every transfer and tick,
//! in an out-of-tree scratch crate, with the recorder validated first against `INC (HL)`
//! decomposing as `pc:4, hl:3, hl:1, hl:3`:
//!
//! ```text
//!   NMI acknowledge   IC@ir:1 x5  MW@sp-1:3  MW@sp-2:3                        11
//!   RETN              M1@0066:4  M1@0067:4  MR@sp:3  MR@sp+1:3                14
//!   LDIR, repeating   M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x7        21
//!   LDIR, exit        M1@pc:4  M1@pc+1:4  MR@hl:3  MW@de:3  IC@de:1 x2        16
//! ```
//!
//! Two things there are not guessable and both matter. The acknowledge's five internal
//! T-states sit on **`IR`**, not on the stack — so with `I` at zero they are in the ROM page
//! and cannot stall, and only the two writes can. And `RETN` is a **two-M1** `ED`-page
//! instruction, which is why the refresh count below is `+2` for it and `+1` for the
//! acknowledge, the acknowledge being the one M1 cycle that refreshes without fetching.
//!
//! The stalls were then computed by a second implementation of the delay pattern, written from
//! the published figures with no sight of `crates/spectrum`, and agreed with the hand
//! arithmetic before the emulator was consulted:
//!
//! ```text
//!   raised at offset 14343, stack in the screen bank
//!     IC@ir  x5             14343 -> 14348   IR is in the ROM page, no stall
//!     MW@sp-1  opens 14348  column 13, pattern index 5: stall 1, then 3   -> 14352
//!     MW@sp-2  opens 14352  column 17, pattern index 1: stall 5, then 3   -> 14360
//!                                                          11 + 6  =  17
//!     RETN's two reads, from 14368 and 14376, stall 5 each  14 + 10 =  24
//! ```
//!
//! The second write's stall is priced at a column **the first one moved it to**, which is the
//! arithmetic that cannot be done by adding a correction to a total.
//!
//! # What is not graded here
//!
//! - **An `NMI` arriving while a frame interrupt is pending**, or one arriving inside the
//!   handler of the other. Both are real hardware situations and neither is driven.
//! - **An `NMI` escaping `HALT`.** `frame_interrupt.rs` grades the maskable case; nothing
//!   drives this one.
//! - **A handler at `0x0066` that does anything.** It is `RETN` and nothing else, because what
//!   is being graded is the acceptance and the return, not the routine.
//! - **The 48K ROM's own `0x0066`.** The fixture patches a `RETN` into a pattern ROM, so this
//!   file says nothing about what Sinclair's NMI routine does — which is a documented trap of
//!   its own, since the shipped 48K routine is famously unreachable in practice.

mod common;

use common::pattern_rom;
use spectrum::memory::PAGE_SIZE;
use spectrum::{Memory, Ula};
use z80::{Cpu, CpuState};

// ---------------------------------------------------------------------------
// The published costs, written here as expectations
// ---------------------------------------------------------------------------

/// T-states one 48K frame lasts.
const FRAME_T_STATES: u64 = 69_888;

/// T-states a repeating `LDIR` pass costs.
const REPEATING_PASS: u64 = 21;

/// T-states the pass that exhausts `BC` costs.
const EXIT_PASS: u64 = 16;

/// T-states an `NMI` acknowledge costs with nothing stalling it.
const ACKNOWLEDGE: u64 = 11;

/// T-states `RETN` costs with nothing stalling it.
const RETN_T_STATES: u64 = 14;

/// Where an `NMI` vectors. Fixed in the silicon; no register selects it.
const NMI_VECTOR: u16 = 0x0066;

/// `RETN`.
const RETN: [u8; 2] = [0xED, 0x45];

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// Bytes the loop moves.
const BLOCK_LEN: usize = 1000;

/// `BC` on entry.
const BC_START: u16 = BLOCK_LEN as u16;

/// T-states the loop costs on its own.
const NOMINAL: u64 = (BC_START as u64 - 1) * REPEATING_PASS + EXIT_PASS;

/// `R` on entry, with bit 7 set — the latch only `LD R,A` moves.
const REFRESH_WITH_THE_LATCH_SET: u8 = 0x80;

/// M1 cycles the run performs: two per pass, one for the acknowledge, two for `RETN`.
const M1_CYCLES: u32 = 2 * BC_START as u32 + 1 + 2;

/// One place to raise the `NMI`.
struct Case {
    name: &'static str,
    /// Passes to let the loop run first. The loop is uncontended, so the frame position this
    /// lands on is exactly `passes * 21`.
    passes_before: u64,
    /// Where the stack lives.
    stack: u16,
    /// What the acknowledge costs on the clock.
    acknowledge: u64,
    /// And what `RETN` costs on it.
    retn: u64,
}

impl Case {
    /// The frame position the `NMI` is raised at.
    const fn raised_at(&self) -> u32 {
        (self.passes_before * REPEATING_PASS) as u32
    }

    /// `BC` as the acknowledge must find it.
    const fn remaining(&self) -> u16 {
        BC_START - self.passes_before as u16
    }

    /// Where the machine stands when the whole run finishes.
    const fn end(&self) -> (u64, u32) {
        (0, (NOMINAL + self.acknowledge + self.retn) as u32)
    }
}

/// A stack in RAM a 48K never contends.
const STACK_FREE: u16 = 0xFF00;

/// And one in the screen bank, the only bank it does.
const STACK_CONTENDED: u16 = 0x7000;

/// The three raisings.
///
/// The second and third are the pair that isolates contention: same instruction, same frame
/// position, same everything but which bank the stack is in. The first is the ordinary case,
/// in the top border where a frame interrupt's acknowledge would also live.
///
/// 683 passes is 14343 T-states, which is eight past the first contended T-state — inside the
/// display area, and far enough in that both of the acknowledge's writes land there.
const CASES: [Case; 3] = [
    Case {
        name: "top border, stack in free RAM",
        passes_before: 100,
        stack: STACK_FREE,
        acknowledge: ACKNOWLEDGE,
        retn: RETN_T_STATES,
    },
    Case {
        name: "display area, stack in the screen bank",
        passes_before: 683,
        stack: STACK_CONTENDED,
        acknowledge: 17,
        retn: 24,
    },
    Case {
        name: "display area, stack in free RAM",
        passes_before: 683,
        stack: STACK_FREE,
        acknowledge: ACKNOWLEDGE,
        retn: RETN_T_STATES,
    },
];

// ---------------------------------------------------------------------------
// Where everything lives
// ---------------------------------------------------------------------------

/// The source block, uncontended, so the loop costs a flat 21 wherever in the frame it runs.
const SOURCE: u16 = 0x9000;

/// The destination, also uncontended.
const DESTINATION: u16 = 0xA000;

/// The block instruction under test.
const CODE: u16 = 0x9800;
const LDIR: [u8; 2] = [0xED, 0xB0];

/// A byte written either side of both blocks, and asserted to survive.
const GUARD: u8 = 0x5A;

/// Steps to allow before concluding the loop is not going to finish.
const STEP_BUDGET: usize = 4_000;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A ROM whose `0x0066` is a `RETN` and whose every other byte is the pattern one.
///
/// The vector is in the ROM page and the page is write-protected — which
/// `rom_write_protection.rs` grades — so the handler cannot be poked in afterwards. Patching
/// the image is the only way, and it is the honest one: it is what a machine with a different
/// ROM would have.
fn rom_with_a_returning_nmi_handler() -> Vec<u8> {
    let mut rom = pattern_rom();
    rom[NMI_VECTOR as usize..NMI_VECTOR as usize + RETN.len()].copy_from_slice(&RETN);
    assert_eq!(rom.len(), PAGE_SIZE, "a page-sized ROM");
    rom
}

/// The block to copy: a period-251 ramp, so any shift of the copy is visible everywhere.
fn source_bytes() -> Vec<u8> {
    (0..BLOCK_LEN).map(|i| (i % 251) as u8).collect()
}

/// The four bytes that must survive.
const fn guards() -> [u16; 4] {
    [
        SOURCE - 1,
        SOURCE + BC_START,
        DESTINATION - 1,
        DESTINATION + BC_START,
    ]
}

/// Store `bytes` at `at`, through the machine's own memory.
fn poke(cpu: &mut Cpu<Ula>, at: u16, bytes: &[u8]) {
    for (offset, byte) in bytes.iter().enumerate() {
        let target = at
            .checked_add(u16::try_from(offset).expect("a block shorter than the address space"))
            .expect("a block that does not run off the top of memory");
        cpu.bus_mut().memory_mut().write(target, *byte);
    }
}

/// T-states elapsed since power-on.
fn elapsed(cpu: &Cpu<Ula>) -> u64 {
    let clock = cpu.bus().clock();
    clock.frames() * FRAME_T_STATES + u64::from(clock.frame_t_state())
}

/// Where the machine stands, as a frame and an offset.
fn position(cpu: &Cpu<Ula>) -> (u64, u32) {
    let clock = cpu.bus().clock();
    (clock.frames(), clock.frame_t_state())
}

/// A `Cpu<Ula>` at the top of frame zero, loaded with the loop and `case`'s stack.
///
/// `IFF1` is left **clear**, which is the whole point: every acceptance this file grades
/// happens with maskable interrupts disabled, so nothing here can be a maskable acceptance
/// wearing an `NMI`'s name.
fn loaded(case: &Case) -> Cpu<Ula> {
    let memory = Memory::spectrum_48k(&rom_with_a_returning_nmi_handler()).expect("one page");
    let mut cpu = Cpu::new(Ula::new(memory));

    poke(&mut cpu, SOURCE, &source_bytes());
    poke(&mut cpu, DESTINATION, &vec![0; BLOCK_LEN]);
    for guard in guards() {
        poke(&mut cpu, guard, &[GUARD]);
    }
    poke(&mut cpu, CODE, &LDIR);

    cpu.set_state(CpuState {
        pc: CODE,
        hl: SOURCE,
        de: DESTINATION,
        bc: BC_START,
        sp: case.stack,
        r: REFRESH_WITH_THE_LATCH_SET,
        // Zero, so the acknowledge's five internal T-states sit on an `IR` in the ROM page and
        // cannot stall. A contended `I` would add a stall this file's arithmetic does not carry.
        i: 0,
        // `iff1` among them: the default is clear, which is the state every gate here needs.
        ..CpuState::default()
    });
    cpu
}

/// Run `passes` iterations of the loop.
fn run_passes(cpu: &mut Cpu<Ula>, passes: u64) {
    for _ in 0..passes {
        cpu.step();
    }
}

/// Run until the loop leaves, and report where it ended.
fn run_to_completion(cpu: &mut Cpu<Ula>) -> (u64, u32) {
    for _ in 0..STEP_BUDGET {
        if cpu.state().pc == CODE + 2 && cpu.state().bc == 0 {
            return position(cpu);
        }
        cpu.step();
    }
    panic!("the loop did not finish within {STEP_BUDGET} steps");
}

/// The address the acknowledge pushed.
fn pushed(cpu: &Cpu<Ula>) -> u16 {
    let sp = cpu.state().sp;
    u16::from_le_bytes([
        cpu.bus().memory().read(sp),
        cpu.bus().memory().read(sp.wrapping_add(1)),
    ])
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

#[test]
fn an_nmi_is_accepted_mid_loop_with_maskable_interrupts_disabled_and_no_line_asserted() {
    // The discriminating gate, and the one an implementation that routed `NMI` through the
    // maskable path could not pass: at this position the machine is offering nothing and the
    // CPU would decline anything it were offered, and the `NMI` is taken regardless.
    for case in &CASES {
        let mut cpu = loaded(case);
        run_passes(&mut cpu, case.passes_before);

        assert_eq!(
            position(&cpu),
            (0, case.raised_at()),
            "{}: {} uncontended passes at {REPEATING_PASS} T-states must land exactly there",
            case.name,
            case.passes_before
        );
        assert!(
            !cpu.bus().interrupt_asserted(),
            "{}: the ULA must not be holding /INT low here, or the contrast below is with \
             nothing",
            case.name
        );

        // A maskable offer made at this exact moment changes nothing at all.
        let before = cpu.state();
        assert_eq!(
            cpu.interrupt(0xFF),
            0,
            "{}: with IFF1 clear the CPU must decline a maskable interrupt",
            case.name
        );
        assert_eq!(
            (cpu.state().pc, cpu.state().sp, cpu.state().bc),
            (before.pc, before.sp, before.bc),
            "{}: and a declined offer must leave the loop exactly as it was",
            case.name
        );

        let charged = cpu.nmi();
        let after = cpu.state();

        assert_eq!(
            after.pc, NMI_VECTOR,
            "{}: an NMI vectors to {NMI_VECTOR:#06X}, fixed — not through I, and not to \
             0x0038",
            case.name
        );
        assert_eq!(
            pushed(&cpu),
            CODE,
            "{}: it must push the instruction's **own** address. {:#06X} is where it would \
             return to if the loop had already exited, and the loop would never finish",
            case.name,
            CODE + 2
        );
        assert_eq!(
            after.bc,
            case.remaining(),
            "{}: {} of the {BC_START} iterations are complete and the rest are still to run",
            case.name,
            case.passes_before
        );
        assert_eq!(
            after.sp,
            case.stack - 2,
            "{}: two bytes of return address, and no more",
            case.name
        );
        assert_eq!(
            u64::from(charged),
            ACKNOWLEDGE,
            "{}: the CPU's own charge for an NMI acknowledge is {ACKNOWLEDGE} T-states \
             whatever the bus does — contention is added on the bus's side, where this number \
             cannot see it",
            case.name
        );
        assert_eq!(
            after.r,
            REFRESH_WITH_THE_LATCH_SET
                | u8::try_from((2 * case.passes_before + 1) % 128).expect("seven bits"),
            "{}: two M1 cycles per pass, and one more for the acknowledge — which refreshes \
             without fetching, and is the only M1 cycle that does",
            case.name
        );
        assert_eq!(cpu.fault(), None, "{}", case.name);
    }
}

#[test]
fn an_nmi_saves_iff1_in_iff2_and_retn_gives_it_back() {
    // The flip-flop rule, which is where `NMI` and the frame interrupt genuinely differ rather
    // than merely arriving by different routes: acceptance **copies** `IFF1` into `IFF2` and
    // then clears it, so `RETN` can put it back. A path that cleared both — which is what the
    // maskable acceptance correctly does — would leave `RETN` restoring a zero, and every
    // interrupt after the first `NMI` would be silently lost.
    //
    // Both entry values, because "copied" and "cleared" agree when `IFF1` is already false.
    for iff1 in [true, false] {
        let case = &CASES[0];
        let mut cpu = loaded(case);
        run_passes(&mut cpu, case.passes_before);
        let mut state = cpu.state();
        state.iff1 = iff1;
        // The opposite of `iff1`, so a shadow that was merely left alone is not mistaken for
        // one that was written.
        state.iff2 = !iff1;
        cpu.set_state(state);

        cpu.nmi();
        let accepted = cpu.state();
        assert!(
            !accepted.iff1,
            "iff1 = {iff1}: acceptance must disable maskable interrupts"
        );
        assert_eq!(
            accepted.iff2, iff1,
            "iff1 = {iff1}: and must copy the old IFF1 into IFF2, not clear it — the shadow \
             was {} on the way in, so a value of {iff1} can only have been written here",
            !iff1
        );

        cpu.step();
        let returned = cpu.state();
        assert_eq!(
            returned.pc, CODE,
            "iff1 = {iff1}: RETN must return to the instruction's own address"
        );
        assert_eq!(
            returned.iff1, iff1,
            "iff1 = {iff1}: and must restore IFF1 from IFF2, giving back exactly what the \
             acceptance saved"
        );
    }
}

#[test]
fn an_nmi_can_be_contended_and_the_frame_interrupt_never_can() {
    // The property `interrupt_contended_block.rs` cannot reach. Its argument that an
    // acknowledge is never contended rests wholly on the ULA's window sitting before the
    // display area — and an NMI has no window, so the argument does not transfer and the
    // opposite is true.
    //
    // The second and third cases are the measurement: same instruction, same frame position,
    // same registers, and the stack one bank apart.
    for case in &CASES {
        let mut cpu = loaded(case);
        run_passes(&mut cpu, case.passes_before);

        let before = elapsed(&cpu);
        cpu.nmi();
        let acknowledge = elapsed(&cpu) - before;
        cpu.step();
        let retn = elapsed(&cpu) - before - acknowledge;

        assert_eq!(
            acknowledge, case.acknowledge,
            "{}: the acknowledge's two stack writes are the only cycles that can stall — its \
             five internal T-states are on IR, in the ROM page",
            case.name
        );
        assert_eq!(
            retn, case.retn,
            "{}: and RETN's two stack reads are the only cycles that can stall in it",
            case.name
        );
    }

    let contended = &CASES[1];
    let free = &CASES[2];
    assert_eq!(
        contended.raised_at(),
        free.raised_at(),
        "the pair must differ in the stack's bank and in nothing else"
    );
    assert!(
        contended.acknowledge > free.acknowledge && contended.retn > free.retn,
        "an NMI raised inside the display area onto a contended stack must cost more than the \
         same NMI onto a free one — {} against {} for the acknowledge, {} against {} for RETN",
        contended.acknowledge,
        free.acknowledge,
        contended.retn,
        free.retn
    );
}

#[test]
fn the_loop_resumes_after_the_handler_and_finishes_the_copy_exactly() {
    let expected = source_bytes();

    for case in &CASES {
        let mut cpu = loaded(case);
        let start = elapsed(&cpu);
        run_passes(&mut cpu, case.passes_before);
        cpu.nmi();
        let end = run_to_completion(&mut cpu);
        let state = cpu.state();

        assert_eq!(state.bc, 0, "{}: the counter must be exhausted", case.name);
        assert_eq!(
            state.pc,
            CODE + 2,
            "{}: only the pass that exhausts BC steps past the instruction",
            case.name
        );
        assert_eq!(
            (state.hl, state.de),
            (SOURCE + BC_START, DESTINATION + BC_START),
            "{}: both pointers must have walked the whole block",
            case.name
        );
        assert_eq!(
            state.sp, case.stack,
            "{}: RETN must have popped the return address again",
            case.name
        );
        assert_eq!(
            end,
            case.end(),
            "{}: the run must end in frame {} at offset {} — {NOMINAL} of loop, {} of \
             acknowledge and {} of RETN, and not one T-state of anything else",
            case.name,
            case.end().0,
            case.end().1,
            case.acknowledge,
            case.retn
        );
        assert_eq!(
            elapsed(&cpu) - start,
            NOMINAL + case.acknowledge + case.retn,
            "{}: stated as a cost as well as a position, which is the form that survives the \
             run having started somewhere other than zero",
            case.name
        );
        assert_eq!(
            state.r,
            REFRESH_WITH_THE_LATCH_SET
                | u8::try_from(M1_CYCLES % 128).expect("a seven-bit refresh counter"),
            "{}: {M1_CYCLES} M1 cycles — two per pass, one for the acknowledge and two for \
             RETN, which is an ED-page instruction and fetches twice",
            case.name
        );

        for (offset, want) in expected.iter().enumerate() {
            let address = DESTINATION + u16::try_from(offset).expect("a block in range");
            assert_eq!(
                cpu.bus().memory().read(address),
                *want,
                "{}: {address:#06X} must hold the byte copied to it",
                case.name
            );
        }
        for guard in guards() {
            assert_eq!(
                cpu.bus().memory().read(guard),
                GUARD,
                "{}: {guard:#06X} is outside both blocks and must be untouched — a pass run \
                 long or short around the acceptance would step onto it",
                case.name
            );
        }
    }
}

#[test]
fn the_layout_is_what_the_derivation_assumes() {
    // The control for this file's premises rather than for the emulator.
    let cpu = loaded(&CASES[0]);
    let memory = cpu.bus().memory();

    assert!(
        memory.is_contended(STACK_CONTENDED),
        "{STACK_CONTENDED:#06X} must be in the bank a 48K contends, or the contended case \
         measures nothing"
    );
    for region in [
        SOURCE,
        SOURCE + BC_START - 1,
        DESTINATION,
        DESTINATION + BC_START - 1,
        CODE,
        STACK_FREE - 2,
        NMI_VECTOR,
    ] {
        assert!(
            !memory.is_contended(region),
            "{region:#06X} must be in a bank a 48K never contends, or the loop does not cost a \
             flat {REPEATING_PASS} and every position in this file moves"
        );
    }
    assert_eq!(
        memory.read(NMI_VECTOR),
        RETN[0],
        "the patched ROM must actually carry the handler — writing it after construction is \
         impossible, because the ROM page is write-protected"
    );

    // The second and third cases must genuinely be inside the display area, and the first
    // genuinely outside it, or the file's whole contrast is between two identical situations.
    const FIRST_CONTENDED: u32 = 14_335;
    assert!(
        CASES[0].raised_at() < FIRST_CONTENDED,
        "the first case must be raised in the top border"
    );
    for case in [&CASES[1], &CASES[2]] {
        assert!(
            case.raised_at() >= FIRST_CONTENDED,
            "{}: must be raised inside the display area",
            case.name
        );
    }

    let mut spans = [
        (SOURCE - 1, BC_START + 2),
        (DESTINATION - 1, BC_START + 2),
        (CODE, 2),
        (STACK_CONTENDED - 2, 2),
    ];
    spans.sort_unstable();
    for pair in spans.windows(2) {
        let (start, len) = pair[0];
        assert!(
            start + len <= pair[1].0,
            "{start:#06X}+{len} runs into {:#06X}",
            pair[1].0
        );
    }
}
