//! Which address the CPU drives during an internal cycle — pinned **without the corpus**.
//!
//! # Why this exists when `fuse_vectors.rs` already checks it
//!
//! `testdata/fuse/` is gitignored. On a fresh clone the conformance gate skips with a
//! notice, and every property it alone protects silently loses its guard. The internal-cycle
//! address is exactly such a property: it leaves T-states, registers, flags and memory
//! completely unchanged, so *nothing else in the suite would notice it breaking*. A
//! mutation at `instructions.rs:429` proved that — it turned only `bus address` red, with
//! transfers byte-identical.
//!
//! The general shape, worth keeping past M1: **a corpus-dependent gate needs a
//! corpus-independent floor for the properties that would otherwise vanish on a clean
//! checkout.** These tests use only [`Machine`] and its `TestBus`, or the `M1Counter` bus
//! defined at the foot of this file, so they run everywhere, unconditionally.
//!
//! # Two questions, two buses
//!
//! The tests above ask *which address* the CPU drives during each T-state, and [`Machine`]
//! answers it. The tests below ask *which kind of machine cycle* a transfer opened —
//! specifically whether it was an M1 opcode fetch — which `TestBus` cannot answer, because
//! it takes the default `Bus::fetch` and so sees fetches and reads as the same call. That
//! second question arrived with `Bus::fetch` itself and is checked against `R`.
//!
//! # These are stronger than the vectors they back up
//!
//! The corpus runs with `I = 0x00` and `R = 0x01`, so `IR` is `0x0001` — which collides with
//! the program addresses at `0x0000`/`0x0001`/`0x0002`. In vector `10` the refresh address
//! and the displacement address are *both* `0x0002`, so that vector cannot tell them apart
//! at all. Here `I` and `R` are chosen so `IR` is `0x4006` and cannot be confused with any
//! program, operand or stack address — a core that used the wrong source fails these tests
//! even though it would pass the corpus.
//!
//! > **The last clause is true of most of the corpus and not of all of it — counted, 2026-09-01.**
//! > Ten vectors do separate `IR` from the program stream, and a core that used the wrong source
//! > would fail them: the eight `RST`s (`c7`, `cf`, `d7`, `df`, `e7`, `ef`, `f7`, `ff`) run at
//! > `PC` = `0x6d33`, so `c7`'s internal cycle at `4 MC 0001` is `IR` and could not be `PC`+1;
//! > and `ed57`/`ed5f` carry `I` = `0x1e`/`0xd7`, so their internal cycles land at `1e19`/`d7f5`.
//! > No vector carries both a non-zero `PC` and a non-zero `I`. So the honest version of the
//! > paragraph above is: **1325 of 1335 vectors are degenerate on this axis, ten are not, and
//! > these tests are stronger than the ten** — they separate `IR` from the program stream, the
//! > stack, *and* the operand addresses simultaneously, on every instruction rather than on ten.
//! > The claim being defended survives; the "would pass the corpus" flourish was too broad.

mod common;

use common::flags;
use common::machine::Machine;
use common::vectors::{MemoryBlock, Registers, Setup, State};
use z80::{Cpu, CpuState, InterruptMode};

/// Deliberately not `0x0000`, so a core that defaults an address to zero is caught.
const PROGRAM_START: u16 = 0x0100;
/// The address of the byte after the opcode: operands and displacements live here.
const FIRST_OPERAND: u16 = 0x0101;
/// The second operand of a three-byte instruction — `CALL`'s "last operand address".
const SECOND_OPERAND: u16 = 0x0102;

const STACK_TOP: u16 = 0x8000;
/// The stack grows down: the high byte of the return address lands here first.
const STACK_HIGH: u16 = 0x7FFF;
const STACK_LOW: u16 = 0x7FFE;

/// `I` and `R` chosen so `IR` shares no bits with any program or stack address above.
const INTERRUPT_VECTOR: u8 = 0x40;
const REFRESH: u8 = 0x05;
/// `IR` as seen *during* the instruction — the opcode fetch has already bumped `R` to 6.
const REFRESH_ADDRESS: u16 = 0x4006;

/// The four T-states of an M1 opcode fetch — every expectation here spends them on `PC`.
///
/// # Only the first of the four is sourced. The other three are this core's shape
///
/// Each use below pairs this constant with [`PROGRAM_START`], which pins **all four**
/// T-states of the fetch to `PC`. The corpus pins **one**. Counted rather than assumed, on
/// 2026-09-01, with `grep -cE` over `testdata/fuse/tests.expected`: 1335 vectors, **1335
/// `MC` events at T=0**, and **zero `MC` events at T=1, T=2 or T=3** — which are the
/// interior of the M1 fetch that every vector begins with. Not one vector in the corpus
/// names an address inside a fetch.
///
/// So the corpus is exhaustive over 1335 opcodes and varies *nothing* on the axis *"which
/// T-state within this machine cycle"*. That is `docs/STATUS.md`'s **exhaustive on one
/// axis** lesson in the place it is easiest to miss: the cross product is over opcodes, and
/// the silent axis is inside a single cycle, so 1335 vectors have no more discriminating
/// power on it than one would.
///
/// `MEMORY_CYCLE` needs no such note. A read or write holds one address for all three of
/// its T-states, so pinning all three claims nothing the transfer has not already said. M1
/// is the one cycle whose address bus is documented to change mid-cycle, and therefore the
/// one place a constant address is a claim rather than a restatement.
///
/// **What sources the *other* three, then? Nothing in this repository.**
/// `common/report.rs` states as fact that a real Z80 drives `PC` for T1–T2 and the refresh
/// address for T3–T4 and cites no measurement; `docs/ARCHITECTURE.md` repeats it; this core
/// contradicts both by driving `PC` for all four. That disagreement, its evidence and what
/// would settle it are written up once, on `compare_contention` in `common/report.rs` —
/// read them there rather than re-deriving them here.
///
/// # Why the simplification is recorded here rather than fixed
///
/// **Measured, 2026-09-01**, in a scratch clone of `0d3e7ef` (never in the shared tree):
/// `Cpu::fetch_opcode` was mutated to drive `PC, PC, IR, IR` — the mutation verified by
/// `git diff` before any verdict was trusted — and the workspace re-run with
/// `cargo test -p z80 -p spectrum --no-fail-fast`. Baseline: 425 passed, 0 failed.
/// Mutated: 410 passed, **15 failed**, and the whole list is
///
/// - the **thirteen** address-stream tests in this file (the six `M1Counter` tests, which
///   count fetches rather than compare addresses, stay green);
/// - `every_t_state_reports_the_address_the_z80_drives` in `crates/z80/src/lib.rs`;
/// - `codegen.rs`'s `bounds_checks_in_the_execute_path_have_not_moved`, 7 → 8, an artefact
///   of the naive mutation's extra `refresh_address()` call and not a fact about M1.
///
/// **Nothing else moved.** `fuse_conformance_unprefixed` (290 vectors) and
/// `fuse_conformance_prefixed` (1045) stayed green — the corpus reading above, confirmed
/// mechanically over every vector rather than argued. `crates/spectrum/tests/timing_oracle.rs`
/// stayed green too: all 68 hardware rows unchanged, in the shipped configuration *and*
/// under the `INTERRUPT_T_STATES = 33` probe, where its three residual disagreements
/// (`group 3 contended` 42 against 41, `group 7 uncontended` 95 against 98, `group 34
/// uncontended` 42 against 44) came back **byte-identical** with the mutation applied.
///
/// The mechanism, read out of `crates/spectrum/src/ula.rs` rather than inferred: `Ula::tick`
/// consults its `address` argument **only** when no machine cycle is open, and a fetch opens
/// one four T-states long — so the address supplied on T2, T3 and T4 of an M1 is discarded
/// before it is looked at. `Ula` is the only `Bus` implementation outside tests and benches.
///
/// **This constant is therefore the single point where the simplification lives.** A
/// hardware-accurate M1 would split it into two T-states on [`PROGRAM_START`] and two on
/// [`REFRESH_ADDRESS`]; those thirteen expectations plus the one in `lib.rs` are the entire
/// list of what would have to change, and no machine's timing would change with them.
const OPCODE_FETCH: usize = 4;
/// The three T-states of a memory read or write, which hold one address throughout.
const MEMORY_CYCLE: usize = 3;

#[test]
fn add_hl_ss_spends_its_seven_internal_t_states_on_ir() {
    // `ADD HL,BC` — 11 T: a 4-T fetch, then a 16-bit add occupying two internal machine
    // cycles, all seven T-states with the refresh address on the bus.
    assert_eq!(
        cycles(&[(PROGRAM_START, OPCODE_FETCH), (REFRESH_ADDRESS, 7)]),
        run(&[0x09], registers()),
        "ADD HL,BC must drive IR through all seven internal T-states, not the last address \
         it fetched from",
    );
}

#[test]
fn rst_spends_its_internal_t_state_on_ir() {
    // `RST 38` — 11 T: fetch, one internal T-state, then two stack writes.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (STACK_HIGH, MEMORY_CYCLE),
            (STACK_LOW, MEMORY_CYCLE),
        ]),
        run(&[0xFF], registers()),
        "RST's internal cycle sits on IR",
    );
}

/// The counterpart to the test above, and the reason both are needed.
///
/// `RST` and `CALL` share their push-and-jump implementation, yet the corpus insists their
/// internal cycles sit on *different* addresses. Nothing but the trace would have predicted
/// that; with `IR` at `0x4006` and the last operand at `0x0102`, giving either instruction
/// the other's rule fails here immediately.
#[test]
fn call_spends_its_internal_t_state_on_the_last_operand_address() {
    // `CALL 0x1234` — 17 T: fetch, two operand reads, one internal T-state, two writes.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (FIRST_OPERAND, MEMORY_CYCLE),
            (SECOND_OPERAND, MEMORY_CYCLE),
            (SECOND_OPERAND, 1),
            (STACK_HIGH, MEMORY_CYCLE),
            (STACK_LOW, MEMORY_CYCLE),
        ]),
        run(&[0xCD, 0x34, 0x12], registers()),
        "CALL's internal cycle sits on the last operand address, not on IR",
    );
}

/// `DJNZ` drives two different addresses inside one instruction.
///
/// The corpus cannot demonstrate this: in vector `10` the refresh address and the
/// displacement address are both `0x0002`, so the two are indistinguishable there. With
/// `IR` at `0x4006` they are not.
#[test]
fn djnz_taken_uses_ir_for_the_m1_extra_t_state_and_the_displacement_address_for_the_add() {
    // `DJNZ -3` with B = 2, so the branch is taken — 13 T: a 5-T M1 (4 + 1 extra), a 3-T
    // displacement read, then a 5-T internal add.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (FIRST_OPERAND, MEMORY_CYCLE),
            (FIRST_OPERAND, 5),
        ]),
        run(&[0x10, 0xFD], counting_down_from(2)),
        "DJNZ's extra M1 T-state belongs to IR; its five internal T-states belong to the \
         displacement byte's own address",
    );
}

#[test]
fn djnz_not_taken_still_reads_the_displacement() {
    // B = 1 decrements to zero, so the branch is not taken — 8 T, and the five internal
    // T-states of the add disappear. The displacement read does not: this is the behaviour
    // the corpus declines to record and `CORPUS_OMISSIONS` documents.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (FIRST_OPERAND, MEMORY_CYCLE),
        ]),
        run(&[0x10, 0xFD], counting_down_from(1)),
        "a not-taken DJNZ still fetches its displacement — only the internal add is skipped",
    );
}

#[test]
fn push_spends_its_internal_t_state_on_ir() {
    // `PUSH BC` — 11 T: a 5-T M1 (4 + 1 extra), then two stack writes.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (STACK_HIGH, MEMORY_CYCLE),
            (STACK_LOW, MEMORY_CYCLE),
        ]),
        run(&[0xC5], registers()),
        "PUSH's extra M1 T-state sits on IR",
    );
}

#[test]
fn inc_ss_spends_its_two_internal_t_states_on_ir() {
    // `INC BC` — 6 T: a 4-T fetch, then a 2-T internal increment of the pair.
    assert_eq!(
        cycles(&[(PROGRAM_START, OPCODE_FETCH), (REFRESH_ADDRESS, 2)]),
        run(&[0x03], registers()),
        "the 16-bit increment's two internal T-states sit on IR",
    );
}

#[test]
fn jr_spends_its_five_internal_t_states_on_the_displacement_address() {
    // `JR +0` — 12 T: fetch, a 3-T displacement read, then a 5-T internal add. Like DJNZ
    // and unlike everything above, the add is charged to the displacement byte's address.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (FIRST_OPERAND, MEMORY_CYCLE),
            (FIRST_OPERAND, 5),
        ]),
        run(&[0x18, 0x00], registers()),
        "JR's internal add is charged to the displacement address, not to IR",
    );
}

/// `RET cc` in both directions — the taken path pops, the not-taken path stops after the
/// M1 extra T-state, and both charge that extra T-state to `IR`.
#[test]
fn ret_cc_spends_its_internal_t_state_on_ir_whether_or_not_it_returns() {
    // `RET NZ` taken (Z clear) — 11 T: 5-T M1, then two stack reads.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (REFRESH_ADDRESS, 1),
            (STACK_TOP, MEMORY_CYCLE),
            (STACK_TOP + 1, MEMORY_CYCLE),
        ]),
        run(&[0xC0], with_flags(0x00)),
        "RET NZ taken: the extra M1 T-state is on IR, then it pops",
    );

    // `RET NZ` not taken (Z set) — 5 T, and nothing is popped.
    assert_eq!(
        cycles(&[(PROGRAM_START, OPCODE_FETCH), (REFRESH_ADDRESS, 1)]),
        run(&[0xC0], with_flags(flags::Z)),
        "RET NZ not taken: 5 T-states and no stack access at all",
    );
}

/// `EX (SP),HL` — the only instruction that drives **stack** addresses during its internal
/// cycles, and it drives two different ones.
#[test]
fn ex_sp_hl_drives_the_stack_addresses_through_its_internal_cycles() {
    // 19 T: fetch, two stack reads, 1 internal on SP+1, two stack writes, 2 internal on SP.
    // Corpus vector `e3` with SP = 0x0373 shows the shape: the single internal T-state
    // belongs to the high half just read, the trailing pair to the low half just written.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (STACK_TOP, MEMORY_CYCLE),
            (STACK_TOP + 1, MEMORY_CYCLE),
            (STACK_TOP + 1, 1),
            (STACK_TOP + 1, MEMORY_CYCLE),
            (STACK_TOP, MEMORY_CYCLE),
            (STACK_TOP, 2),
        ]),
        run(&[0xE3], registers()),
        "EX (SP),HL: one internal T-state on SP+1, two on SP",
    );
}

/// `LDIR` mid-run — the repeat mechanism, and the newest code in the core.
///
/// M2 added six block-instruction internal-cycle sites and the corpus is the only thing
/// that touched any of them. These two tests are the corpus-independent floor for the most
/// distinctive of them: the five extra T-states that make the instruction repeat.
#[test]
fn ldir_charges_its_repeat_cycles_to_the_destination_address() {
    // 21 T while BC != 0 after the copy: ED fetch, B0 fetch, read (HL), write (DE), then
    // seven internal T-states — two for the copy, five for the repeat — all on DE.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (PROGRAM_START + 1, OPCODE_FETCH),
            (BLOCK_SOURCE, MEMORY_CYCLE),
            (BLOCK_DESTINATION, MEMORY_CYCLE),
            (BLOCK_DESTINATION, 7),
        ]),
        run(&[0xED, 0xB0], block_copy(2)),
        "a repeating LDIR spends all seven internal T-states on DE",
    );
}

#[test]
fn ldir_on_its_final_iteration_drops_the_five_repeat_cycles() {
    // 16 T when BC reaches zero: the same shape with only the copy's two internal T-states.
    // This is the difference the repeat mechanism has to get right.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (PROGRAM_START + 1, OPCODE_FETCH),
            (BLOCK_SOURCE, MEMORY_CYCLE),
            (BLOCK_DESTINATION, MEMORY_CYCLE),
            (BLOCK_DESTINATION, 2),
        ]),
        run(&[0xED, 0xB0], block_copy(1)),
        "the last LDIR iteration is 16 T, not 21 — no repeat cycles",
    );
}

#[test]
fn lddr_matches_ldir_cycle_for_cycle() {
    // LDDR differs from LDIR only in which way HL and DE move afterwards; the machine
    // cycles of a single iteration are identical, so the floor is too.
    assert_eq!(
        cycles(&[
            (PROGRAM_START, OPCODE_FETCH),
            (PROGRAM_START + 1, OPCODE_FETCH),
            (BLOCK_SOURCE, MEMORY_CYCLE),
            (BLOCK_DESTINATION, MEMORY_CYCLE),
            (BLOCK_DESTINATION, 7),
        ]),
        run(&[0xED, 0xB8], block_copy(2)),
        "a repeating LDDR has the same cycle shape as LDIR",
    );
}

// ---------------------------------------------------------------------------
// M1 opcode fetches, counted against `R`
//
// `Bus::fetch` splits the M1 opcode fetch out of `Bus::read` so a machine can tell a
// four-T-state fetch from a three-T-state read followed by an internal cycle. Verifying
// that split by counting call sites by eye is exactly the kind of evidence this project
// keeps catching itself accepting, so it is verified against an oracle instead:
//
//     within one `step()`, the number of `Bus::fetch` calls equals the number of `R`
//     increments.
//
// `R` is graded independently and heavily — 290/290 un-prefixed and 1045/1045 prefixed FUSE
// vectors, plus `zexall` — so this anchors the new method to something already proven. It
// also bites in both directions: a fetch left on `read` drops one side of the equation, and
// an operand read promoted to `fetch` inflates the other.
//
// The one exception is an interrupt acknowledge, which refreshes without fetching because
// its byte comes from the device rather than from memory. It gets its own test rather than
// a footnote.
// ---------------------------------------------------------------------------

#[test]
fn an_unprefixed_instruction_fetches_its_opcode_and_reads_its_operands() {
    // `NOP` is the floor: one M1 cycle and no memory cycle of any other kind.
    let nop = tally(&[0x00], started(), 1);
    assert_eq!(
        (1, 0),
        (nop.fetches, nop.reads),
        "NOP is one fetch and nothing else",
    );
    assert_refresh_tracks_fetches(&nop, "NOP");

    // `LD BC,nn` puts both kinds in one instruction: the M1 cycle, then two three-T-state
    // operand reads that must stay on `read`. If the split were wrong in the obvious
    // direction — everything through `fetch` — this is the case that says so.
    let load = tally(&[0x01, 0x34, 0x12], started(), 1);
    assert_eq!(
        (1, 2),
        (load.fetches, load.reads),
        "LD BC,nn is one fetch and two operand reads",
    );
    assert_refresh_tracks_fetches(&load, "LD BC,nn");
}

#[test]
fn every_prefix_in_a_long_run_is_its_own_m1_fetch() {
    // The same shape as `lib.rs`'s `a_long_prefix_run_does_not_overflow_the_t_state_count`,
    // asked a different question. That test pins T-states, `PC` and `R` against a byte-wide
    // accumulator overflowing on a legal instruction stream; this one pins that all 301 M1
    // cycles reached `Bus::fetch`, and that not one of them arrived as a read.
    const PREFIX_RUN: usize = 300;
    let mut program = vec![0xDD; PREFIX_RUN];
    program.push(0x00); // the NOP the run finally prefixes

    // Every byte of this program is its own M1 cycle — which is the claim under test, so it
    // is derived from the program rather than restated as a second literal.
    let m1_cycles = u32::try_from(program.len()).expect("301 fits in a u32");
    let run = tally(&program, started(), 1);

    assert_eq!(
        (m1_cycles, 0),
        (run.fetches, run.reads),
        "300 prefixes and the NOP they prefix are 301 M1 fetches and no read at all",
    );
    assert_refresh_tracks_fetches(&run, "a 300-prefix run");
    assert_eq!(
        45, run.refresh.1,
        "301 increments of a seven-bit counter from zero",
    );
}

#[test]
fn the_indexed_cb_displacement_and_opcode_are_reads_not_fetches() {
    // `DD CB d 06` is `RLC (IX+d)`: four bytes, of which only the first two are M1 cycles.
    // The displacement and the operation byte arrive as ordinary three-T-state memory reads,
    // which is why `R` advances **twice** across a four-byte instruction rather than four
    // times. Promoting either of them to `fetch` would refresh twice too often, and would
    // also tell a contention model to charge them as four-T-state cycles.
    let state = CpuState {
        ix: INDEX_BASE,
        ..started()
    };
    let indexed = tally(&[0xDD, 0xCB, 0x02, 0x06], state, 1);

    assert_eq!(2, indexed.fetches, "only DD and CB are M1 cycles");
    assert_eq!(
        3, indexed.reads,
        "the displacement, the operation byte, and (IX+d) itself",
    );
    assert_refresh_tracks_fetches(&indexed, "RLC (IX+d)");
    assert_eq!(
        2, indexed.refresh.1,
        "R advances twice across the four bytes, not four times",
    );
}

#[test]
fn a_repeating_block_instruction_refetches_its_own_opcode_each_pass() {
    // `LDIR` repeats by rewinding `PC` two bytes and returning, one `step()` per iteration,
    // so every pass is a fresh `ED B0` — two M1 cycles, and `R` advancing by two. That is
    // what keeps a 64 KB copy interruptible, and therefore what lets it coexist with a
    // 50 Hz frame interrupt; here it is visible as two passes costing four fetches.
    let copying = CpuState {
        hl: BLOCK_SOURCE,
        de: BLOCK_DESTINATION,
        bc: 2,
        ..started()
    };

    let first = tally(&[0xED, 0xB0], copying, 1);
    assert_eq!(2, first.fetches, "ED and B0 are each their own M1 cycle");
    assert_eq!(1, first.reads, "and the copy reads one byte from (HL)");
    assert_refresh_tracks_fetches(&first, "LDIR, mid-repeat");
    assert_eq!(
        PROGRAM_START, first.pc,
        "PC is rewound onto the instruction, which is what makes the next pass re-fetch it",
    );

    let both = tally(&[0xED, 0xB0], copying, 2);
    assert_eq!(
        4, both.fetches,
        "the second pass re-fetches the same two bytes"
    );
    assert_refresh_tracks_fetches(&both, "LDIR, two passes");
    assert_eq!(4, both.refresh.1, "R advances by two per iteration");
}

#[test]
fn a_halted_cpu_issues_a_real_m1_fetch_every_cycle() {
    // The ruling this change had to make, and `R` is what settles it. A halted Z80 has not
    // stopped: it keeps issuing M1 cycles and executing an internal NOP. The Z80 has no way
    // to refresh without an M1 cycle, and a halted cycle refreshes — so it *is* an M1 cycle,
    // differing from any other opcode fetch only in that the byte is discarded. Calling it a
    // read would ask a contention model to charge three T-states plus an internal cycle for
    // what the hardware spends as one four-T-state fetch.
    //
    // Three steps: the `HALT` itself, then two halted cycles. `PC` never leaves the opcode,
    // so all three fetch the same address.
    let halting = tally(&[0x76], started(), 3);

    assert_eq!(
        3, halting.fetches,
        "the HALT and the two halted cycles are three M1 cycles",
    );
    assert_eq!(0, halting.reads, "a halted CPU reads nothing");
    assert_refresh_tracks_fetches(&halting, "HALT");
    assert_eq!(
        PROGRAM_START, halting.pc,
        "and PC stays on the HALT opcode throughout",
    );
}

#[test]
fn an_interrupt_acknowledge_refreshes_without_fetching() {
    // The one exception, tested rather than left as a footnote. An acknowledge is an M1
    // cycle — it refreshes `R` — but it reads no memory: the Z80 asserts `/IORQ` in place of
    // `/MREQ` and the device answers on the data bus, which reaches this core as
    // `interrupt`'s argument rather than through the `Bus`. Reporting it as a fetch would
    // name an address the machine would be entitled to contend and to serve from its memory
    // map, so the correct call count here is zero.
    //
    // The consequence for a machine: fetch-per-refresh is exact across `step()`, and off by
    // one for each accepted interrupt. Anything reconstructing M1 cycles from the bus alone
    // must add those itself.
    let mut cpu = Cpu::new(M1Counter::new(&[0x00]));
    cpu.set_state(CpuState {
        iff1: true,
        im: InterruptMode::Mode1,
        ..started()
    });

    let t_states = cpu.interrupt(FLOATING_BUS_BYTE);

    assert_eq!(
        13, t_states,
        "a seven-T-state acknowledge and a six-T-state push"
    );
    assert_eq!(0, cpu.bus().fetches, "nothing was fetched");
    assert_eq!(
        0,
        cpu.bus().reads,
        "and nothing read either: mode 1's vector is a constant",
    );
    assert_eq!(
        1,
        cpu.state().r,
        "yet R advanced, because the acknowledge is still an M1 cycle",
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Expand `(address, t_states)` runs into the flat per-T-state address log the bus records.
///
/// Written as runs because that is how the hardware behaves — one address held for a whole
/// machine cycle — and because it keeps each expectation readable as a cycle breakdown
/// rather than a wall of repeated literals.
fn cycles(runs: &[(u16, usize)]) -> Vec<u16> {
    runs.iter()
        .flat_map(|(address, t_states)| std::iter::repeat_n(*address, *t_states))
        .collect()
}

fn registers() -> Registers {
    Registers {
        pc: PROGRAM_START,
        sp: STACK_TOP,
        ..Registers::default()
    }
}

/// `B` is the high byte of `BC`, and it is what `DJNZ` counts down.
fn counting_down_from(b: u8) -> Registers {
    Registers {
        bc: u16::from(b) << 8,
        ..registers()
    }
}

/// Source and destination for the block-copy tests, well away from the program and the
/// stack so a wrong address cannot coincide with a right one.
const BLOCK_SOURCE: u16 = 0x5000;
const BLOCK_DESTINATION: u16 = 0x6000;

/// `HL` -> `DE`, `BC` bytes to go. `BC == 1` makes the next copy the last one.
fn block_copy(remaining: u16) -> Registers {
    Registers {
        hl: BLOCK_SOURCE,
        de: BLOCK_DESTINATION,
        bc: remaining,
        ..registers()
    }
}

/// `F` is the low byte of `AF`, and it is what the conditional instructions branch on.
fn with_flags(f: u8) -> Registers {
    Registers {
        af: u16::from(f),
        ..registers()
    }
}

/// Execute exactly one instruction and return the address the bus saw at each T-state.
fn run(bytes: &[u8], registers: Registers) -> Vec<u16> {
    let setup = Setup {
        name: String::from("bus_timing"),
        registers,
        state: State {
            i: INTERRUPT_VECTOR,
            r: REFRESH,
            ..State::default()
        },
        memory: vec![MemoryBlock {
            start: PROGRAM_START,
            bytes: bytes.to_vec(),
        }],
    };
    let mut machine = Machine::load(&setup);
    machine.step();
    // Every test in this file rests on the instruction actually having executed. A core
    // that recorded an unimplemented-opcode fault would still produce a tick log, and a
    // short one could coincidentally match a short expectation — so the fault is checked
    // here, once, rather than assumed nine times.
    assert_eq!(
        None,
        machine.fault(),
        "the core faulted instead of executing {bytes:02x?}",
    );
    machine.tick_addresses().to_vec()
}

// ---------------------------------------------------------------------------
// Fixtures for the M1-fetch tests
// ---------------------------------------------------------------------------

/// The index register's base, chosen to collide with no program, stack or block address.
const INDEX_BASE: u16 = 0x7000;

/// What a Spectrum's undriven data bus reads as, which is also `RST 38h`.
const FLOATING_BUS_BYTE: u8 = 0xFF;

/// `R` counts in its low seven bits; bit 7 keeps whatever `LD R,A` last put there.
const REFRESH_COUNTER: u8 = 0x7F;
/// Which is why it wraps after this many M1 cycles rather than after 256.
const REFRESH_PERIOD: u32 = 128;

/// A bus that counts M1 opcode fetches apart from every other memory read.
///
/// `common::machine::TestBus` cannot answer that question, and not by oversight: it takes
/// the default `Bus::fetch`, which forwards to `read`, so the two arrive there
/// indistinguishably — the very ambiguity the method exists to remove. Telling them apart
/// needs a bus that overrides it, and this one does that and nothing else.
struct M1Counter {
    memory: Vec<u8>,
    /// `Bus::fetch` calls — M1 opcode fetches.
    fetches: u32,
    /// `Bus::read` calls — operand, data and stack reads, and after this change nothing
    /// else.
    reads: u32,
}

impl M1Counter {
    /// 64K of RAM with `program` loaded at [`PROGRAM_START`].
    fn new(program: &[u8]) -> Self {
        let mut memory = vec![0; 0x1_0000];
        let start = usize::from(PROGRAM_START);
        memory[start..start + program.len()].copy_from_slice(program);
        Self {
            memory,
            fetches: 0,
            reads: 0,
        }
    }
}

impl z80::Bus for M1Counter {
    fn read(&mut self, addr: u16) -> u8 {
        self.reads += 1;
        self.memory[usize::from(addr)]
    }

    /// Deliberately not `self.read(addr)`: that is the default body this override replaces,
    /// and delegating to it would count every fetch on the read side as well.
    fn fetch(&mut self, addr: u16) -> u8 {
        self.fetches += 1;
        self.memory[usize::from(addr)]
    }

    fn write(&mut self, addr: u16, val: u8) {
        self.memory[usize::from(addr)] = val;
    }

    fn in_port(&mut self, _port: u16) -> u8 {
        FLOATING_BUS_BYTE
    }

    fn out_port(&mut self, _port: u16, _val: u8) {}

    fn tick(&mut self, _addr: u16) {}
}

/// What a run of `step()`s did, split by machine-cycle kind.
struct Tally {
    fetches: u32,
    reads: u32,
    /// `R` before the run, and after it.
    refresh: (u8, u8),
    /// `PC` after the run.
    pc: u16,
}

impl Tally {
    /// `R` as it would stand if every M1 fetch — and nothing else — had refreshed it.
    ///
    /// This is the oracle, not a restatement of the code under test: the arithmetic is the
    /// documented behaviour of the refresh register, and the only input taken from the run
    /// is the number of `Bus::fetch` calls.
    fn refresh_implied_by_fetches(&self) -> u8 {
        let (before, _) = self.refresh;
        let counted = u8::try_from(self.fetches % REFRESH_PERIOD)
            .expect("a remainder below 128 fits in a u8");
        (before & !REFRESH_COUNTER) | (before.wrapping_add(counted) & REFRESH_COUNTER)
    }
}

/// The invariant: one `R` increment per M1 fetch, and no other source of either.
#[track_caller]
fn assert_refresh_tracks_fetches(tally: &Tally, case: &str) {
    assert_eq!(
        tally.refresh_implied_by_fetches(),
        tally.refresh.1,
        "{case}: {} fetches, so R must have gone {:#04X} -> {:#04X}",
        tally.fetches,
        tally.refresh.0,
        tally.refresh_implied_by_fetches(),
    );
}

/// The state the M1-fetch tests start from: `PC` at [`PROGRAM_START`], a usable stack, and
/// `R` at zero so an advance reads as a count.
fn started() -> CpuState {
    CpuState {
        pc: PROGRAM_START,
        sp: STACK_TOP,
        af: 0,
        ..CpuState::default()
    }
}

/// Execute `steps` instructions from `state` and report the fetch/read split.
fn tally(program: &[u8], state: CpuState, steps: usize) -> Tally {
    let mut cpu = Cpu::new(M1Counter::new(program));
    cpu.set_state(state);
    let before = cpu.state().r;

    for _ in 0..steps {
        cpu.step();
    }

    // As in `run` above: a faulted core still produces counts, and a short one could
    // coincidentally match a short expectation.
    assert_eq!(
        None,
        cpu.fault(),
        "the core faulted instead of executing {program:02x?}",
    );
    Tally {
        fetches: cpu.bus().fetches,
        reads: cpu.bus().reads,
        refresh: (before, cpu.state().r),
        pc: cpu.state().pc,
    }
}
