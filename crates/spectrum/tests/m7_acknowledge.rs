//! Gate: an interrupt acknowledge is **one** machine cycle, not `t_states` internal ones.
//!
//! # Why this gate could have existed since M5, and why nobody wrote it
//!
//! `ula.rs` recorded the defect and declined to fix it, on the grounds that *"no test can
//! currently tell the two models apart"*. That was true of **software running on the machine**
//! and false of **a test driving the bus** — `Cpu::new`, `Cpu::nmi`, `Cpu::interrupt` and
//! `Ula::new` have all been public since M5. This file is the test that was available the whole
//! time.
//!
//! It therefore goes through a raw [`z80::Cpu<Ula>`] rather than through [`spectrum::Spectrum`],
//! and that is not a convenience. `Spectrum::step` offers an interrupt only while the ULA is
//! asserting one, so the machine's own frame loop **cannot** reach a contended acknowledge; the
//! CPU's entry points can.
//!
//! # What this pins, and what it cannot grade
//!
//! It pins the **model**: one contention charge, not `t_states` of them. It cannot **grade** it,
//! and nothing can — which is a stronger statement than "it is currently ungraded" and is the
//! reason it is worth writing down.
//!
//! - **On a 48K it is unreachable by construction.** `/INT` is held for the frame's first 32
//!   T-states, contention begins at 14335, and acceptance cannot be deferred past the window's
//!   end. Every accepted interrupt lands in the top border.
//! - **On a 128 the same argument holds**, re-derived on its own numbers rather than inherited:
//!   the window still opens at frame T-state 0 and contention begins at **14362**. The conclusion
//!   never depended on the figure — it would have to be under about forty. *(This read 14361 and
//!   called it "that disputed figure". `tests/timing_oracle.rs`'s 128 edition settled it on
//!   2026-09-02 at 14362, uniquely; the margin the argument rests on does not notice one
//!   T-state, which is what the sentence was for.)*
//! - **`NMI` is the exception, and it has no hardware source on either machine.** Nothing on a
//!   Spectrum drives `/NMI`, so no guest reaches this either; a test does, because `Cpu::nmi` is
//!   a public entry point.
//!
//! So what says the model is right is the hardware rule, which `docs/Z80-REFERENCE.md` already
//! states in this repository's own words: the Z80 asserts `/M1` with `/IORQ` in place of
//! `/MREQ`, one machine cycle, therefore one contention point.
//!
//! # Every expectation is derived from the pattern first
//!
//! Including the **old** model's, so that each assertion names the number it is separating
//! itself from. A test that only knew the right answer could not say whether it was capable of
//! seeing the wrong one.

mod common;
mod m7_common;

use m7_common::{machine_128, pattern_rom};
use spectrum::memory::PAGE_SIZE;
use spectrum::timing::Timing;
use spectrum::{Memory, Ula};
use z80::{Bus, Cpu, CpuState, InterruptMode};

/// `NOP`, and the uncontended bank the positioning sled runs in.
const NOP: u8 = 0x00;
const SLED: u16 = 0x8000;

/// A stack in bank 2 — uncontended on both machines, and **not** in the pageable slot.
///
/// `0xFF00` would sit in slot 3, which is where the contended bank is paged for these
/// measurements, so the two stack writes would contend as well and the acknowledge would stop
/// being the only variable.
const STACK: u16 = 0xA000;

/// T-states the two stack writes cost, uncontended.
const PUSH_T_STATES: u32 = 3 + 3;

/// Where the positioning sled lands: the first multiple of four at or past the 48K's window.
///
/// A whole number of `NOP`s, so the landing is arithmetic rather than a composition puzzle, and
/// one T-state into the contended window rather than exactly on it — which makes the pattern
/// start at 5 instead of 6 and is therefore a slightly less forgiving phase to be wrong at.
const TARGET: u32 = 14336;

const _: () = assert!(TARGET.is_multiple_of(4));
const _: () = assert!(TARGET > Timing::SPECTRUM_48K.first_contended_t_state());

/// A 48K whose `IR` lands in the contended screen bank, positioned at [`TARGET`].
fn forty_eight_k(i: u8) -> Cpu<Ula> {
    let memory = Memory::spectrum_48k(&pattern_rom(0)).expect("a page-sized ROM");
    positioned(Cpu::new(Ula::new(memory)), i)
}

/// A 128 with `bank` paged into `0xC000`, positioned at [`TARGET`].
fn one_two_eight(bank: u8, i: u8) -> Cpu<Ula> {
    let mut machine = machine_128();
    machine.ula_mut().out_port(0x7FFD, bank);
    let memory = std::mem::replace(
        machine.memory_mut(),
        Memory::spectrum_48k(&[0; PAGE_SIZE]).expect("a page-sized ROM"),
    );
    positioned(Cpu::new(Ula::new(memory)), i)
}

/// Run `cpu` to exactly [`TARGET`] out of uncontended memory, then point `IR` via `i`.
///
/// A sled of `NOP`s in bank 2: uncontended on both machines and untouched by any paging value,
/// so each costs exactly four T-states and the landing is a multiplication. It is **asserted**,
/// not assumed — if contention ever reached bank 2, every figure below would silently shift.
fn positioned(mut cpu: Cpu<Ula>, i: u8) -> Cpu<Ula> {
    let sled = usize::try_from(TARGET / 4).expect("a sled that fits in memory");
    for offset in 0..sled {
        let address = SLED + u16::try_from(offset).expect("a sled inside one bank");
        cpu.bus_mut().memory_mut().write(address, NOP);
    }
    cpu.set_state(CpuState {
        pc: SLED,
        ..CpuState::default()
    });
    for _ in 0..sled {
        cpu.step();
    }
    assert_eq!(
        cpu.bus().clock().frame_t_state(),
        TARGET,
        "the sled must land exactly on the target out of uncontended memory"
    );

    cpu.set_state(CpuState {
        i,
        sp: STACK,
        iff1: true,
        iff2: true,
        im: InterruptMode::Mode1,
        pc: cpu.state().pc,
        ..CpuState::default()
    });
    cpu
}

/// What the clock charged for `operation`, which is the only account that can see a stall.
///
/// `Cpu::interrupt` and `Cpu::nmi` both **return** a T-state figure, and that figure is the
/// CPU's nominal charge — contention is added on the bus's side, where a return value cannot
/// see it. An assertion of the form `accepted == ACKNOWLEDGE` pins a nominal length and can
/// never witness the thing this file is about.
fn charged(cpu: &mut Cpu<Ula>, operation: impl FnOnce(&mut Cpu<Ula>)) -> u32 {
    let before = cpu.bus().clock().frame_t_state();
    operation(cpu);
    cpu.bus().clock().frame_t_state() - before
}

#[test]
fn an_nmi_with_a_contended_refresh_address_is_charged_one_stall_and_not_five() {
    // Derived at +1 into the 48K's window, so the pattern starts at 5:
    //
    //   one cycle    stall delay(+1) = 5, then 5 covered T-states, then the two pushes
    //                5 + 5 + 6                                            = 16
    //
    //   five cycles  t1 at +1  -> 5, +1  -> +7
    //                t2 at +7  -> 0, +1  -> +8
    //                t3 at +8  -> 6, +1  -> +15
    //                t4 at +15 -> 0, +1  -> +16
    //                t5 at +16 -> 6, +1  -> +23   then the two pushes    = 29
    const ONE_CYCLE: u32 = 5 + 5 + PUSH_T_STATES;
    const FIVE_CYCLES: u32 = 23 + PUSH_T_STATES;
    assert_eq!((ONE_CYCLE, FIVE_CYCLES), (16, 29));

    // `I = 0x40` puts `IR` in 0x40xx — bank 5, the contended one — for every value of `R`.
    let mut cpu = forty_eight_k(0x40);
    let charge = charged(&mut cpu, |cpu| {
        cpu.nmi();
    });
    assert_eq!(charge, ONE_CYCLE);
    // Named rather than merely implied: this is the figure the fix moved away from, and
    // asserting the negative is what says the gate can see the wrong answer as well as the
    // right one.
    assert_ne!(charge, FIVE_CYCLES, "the per-T-state model must be refuted");
}

#[test]
fn an_accepted_interrupt_with_a_contended_refresh_address_is_charged_one_stall_and_not_seven() {
    //   one cycle     5 + 7 + 6                                           = 18
    //   seven cycles  t1..t5 as above -> +23
    //                 t6 at +23 -> 0, +1 -> +24
    //                 t7 at +24 -> 6, +1 -> +31   then the two pushes     = 37
    const ONE_CYCLE: u32 = 5 + 7 + PUSH_T_STATES;
    const SEVEN_CYCLES: u32 = 31 + PUSH_T_STATES;
    assert_eq!((ONE_CYCLE, SEVEN_CYCLES), (18, 37));

    let mut cpu = forty_eight_k(0x40);
    let charge = charged(&mut cpu, |cpu| {
        assert_ne!(cpu.interrupt(0xFF), 0, "the offer must be accepted");
    });
    assert_eq!(charge, ONE_CYCLE);
    assert_ne!(
        charge, SEVEN_CYCLES,
        "the per-T-state model must be refuted"
    );
    assert_eq!(cpu.state().pc, 0x0038, "mode 1 vectors to 0x0038");
}

#[test]
fn an_uncontended_refresh_address_charges_exactly_what_it_always_did() {
    // The regression half, and the reason the fix moved no existing figure: with `IR` in the
    // ROM there is no stall to charge once or five times, so both models agree exactly. Every
    // 48K gate that touches an interrupt is in this case, which is why none of them moved.
    let mut cpu = forty_eight_k(0x00);
    assert_eq!(
        charged(&mut cpu, |cpu| {
            cpu.nmi();
        }),
        5 + PUSH_T_STATES
    );

    let mut cpu = forty_eight_k(0x00);
    let charge = charged(&mut cpu, |cpu| {
        cpu.interrupt(0xFF);
    });
    assert_eq!(charge, 7 + PUSH_T_STATES);
}

#[test]
fn a_128_reaches_the_same_case_by_paging_rather_than_by_arrangement() {
    // `docs/STATUS.md` says M7 makes the contended-refresh case routine. Here is the routine
    // version: `I = 0xC0` puts `IR` in the **pageable** slot, so which bank is there decides
    // whether the acknowledge contends — the same instruction, the same address, one paging
    // value apart.
    //
    // The 128's window opens 26 T-states later than the 48K's, so at TARGET it has not opened
    // and nothing contends yet. That is the point: the *mechanism* is what is being separated,
    // and the two banks must differ only if contention is live.
    assert!(TARGET < Timing::SPECTRUM_128.first_contended_t_state());
    for bank in [0_u8, 1] {
        let mut cpu = one_two_eight(bank, 0xC0);
        assert_eq!(
            charged(&mut cpu, |cpu| {
                cpu.nmi();
            }),
            5 + PUSH_T_STATES,
            "a 128 does not contend at {TARGET}, whichever bank {bank} is paged in"
        );
    }
}

#[test]
fn no_accepted_interrupt_on_either_machine_can_reach_a_contended_t_state() {
    // The structural claim `ula.rs` makes, as arithmetic rather than as prose — and re-derived
    // on each machine's own geometry rather than inherited from the 48K, because the 128 is
    // exactly the machine the old comment guessed might close the gap.
    //
    // Acceptance is offered only while the ULA holds `/INT`, and cannot be deferred past the
    // window's end: if the line has dropped, no offer is made. So the latest an acknowledge can
    // begin is the window's last T-state plus the longest instruction that could be in flight.
    // The Z80's longest is 23 T-states; a run of `DD`/`FD` prefixes is unbounded, but every
    // prefix is a fetch and an interrupt is offered before each instruction, so the bound that
    // matters is one instruction.
    const LONGEST_INSTRUCTION: u32 = 23;

    for (name, timing) in [("48K", Timing::SPECTRUM_48K), ("128", Timing::SPECTRUM_128)] {
        let latest = timing.interrupt_t_states() + LONGEST_INSTRUCTION;
        assert!(
            latest < timing.first_contended_t_state(),
            "{name}: an acknowledge could reach {latest}, and contention starts at {}",
            timing.first_contended_t_state()
        );

        // And the margin, so that the conclusion is visibly insensitive to the offset. Written
        // when that offset was "the one figure this milestone could not establish"; the 128
        // edition of `timing_oracle.rs` established it on 2026-09-02, and the assertion is worth
        // keeping unchanged because what it grades is the *margin* — the offset would have to be
        // wrong by three orders of magnitude for the argument to fail, and it moved by one.
        assert!(
            timing.first_contended_t_state() > 40 * latest,
            "{name}: the conclusion should not be close"
        );
    }
}

#[test]
fn the_acknowledge_leaves_no_machine_cycle_half_open() {
    // `covered_t_states` is armed by the acknowledge and must be spent exactly by its own
    // ticks. If it were armed for more than it spends, the next bare ticks would skip their
    // contention — which is the failure `set_border` documents from the other direction.
    //
    // Measured as: the instruction *after* an accepted interrupt costs exactly its own nominal
    // length, out of uncontended ROM.
    //
    // Which instruction that is has to be derived rather than assumed, and getting it wrong is
    // how this assertion first read `4`. The fixture's ROM is `pattern_rom(0)`, whose byte at
    // address `a` is `a & 0xFF` — so `0x0038` holds `0x38`, which is `JR C,e`. `F` is therefore
    // what decides its length, and `CpuState::default()` powers up with `AF = 0xFFFF`, carry
    // **set**, so it would be taken and cost 12. Carry is cleared here so the answer is the
    // unambiguous one: not taken, seven T-states, all of it in uncontended ROM.
    let mut cpu = forty_eight_k(0x40);
    assert_eq!(
        cpu.bus().memory().read(0x0038),
        0x38,
        "the pattern ROM puts JR C,e at the mode 1 vector"
    );
    cpu.set_state(CpuState {
        af: 0x0000,
        ..cpu.state()
    });

    cpu.interrupt(0xFF);
    assert_eq!(cpu.state().pc, 0x0038);

    let before = cpu.bus().clock().frame_t_state();
    cpu.step();
    assert_eq!(
        cpu.bus().clock().frame_t_state() - before,
        7,
        "an untaken JR out of uncontended ROM costs seven and nothing else"
    );
}
