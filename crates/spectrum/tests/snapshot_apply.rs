//! The applier: `Spectrum::restore` and `Spectrum::snapshot`, and what grades them.
//!
//! # R1, and why it is the strongest and the cheapest instrument here
//!
//! `docs/M6.md` Decision 7 decomposes the milestone's round trips into three. This file owns
//! the first and the third:
//!
//! | | Round trip | Grades |
//! |---|---|---|
//! | **R1** | `snapshot(restore(s)) == s` | the applier: every field reaching the machine and coming back |
//! | R2 | `parse(write(s)) == s` | the codec — `snapshot_vectors.rs` and `snapshot_hostile.rs` own it |
//! | **R3** | `write(snapshot(restore(parse(f)))) == f` | that R1 and R2 compose, end to end, over bytes |
//!
//! R1 needs no corpus and no ROM: a `Snapshot`, a bare machine, and a comparison. That is what
//! makes it the test that catches `restore` forgetting a field.
//!
//! # What R1 cannot see — measured by symmetric mutation, not argued
//!
//! Every round trip compares a value against a value that came from the same code, so a
//! **symmetric** error survives. Three were mutated into the applier, each verified present in
//! the file before its verdict was trusted and each restored from a byte-level backup:
//!
//! | Mutation | R1 | R3 | What did see it |
//! |---|---|---|---|
//! | The bank→address map inverted in **both** `restore` and `snapshot` | **green** | **green** | `a_restored_bank_lands_where_the_48k_memory_map_says` |
//! | `restore` routed through `Bus::out_port` instead of `Ula::set_border` | **green** | **green** | `a_restore_leaves_no_machine_cycle_half_open` and `a_restore_does_not_move_the_tape` |
//! | `restore` rewinding the frame counter | **green** | **green** | `a_restore_does_not_rewind_the_machines_uptime` |
//!
//! The first is the M4 `q`-defaulting defect in a new shape: two halves agreeing to lose the
//! same thing, and a green run because they agreed.
//!
//! **One class R1 is structurally immune to, which is why it is stronger than R2 rather than
//! merely different.** R1's two halves do not share a field map: `restore` writes through
//! `Cpu::set_state` and `snapshot` reads through `Cpu::state`, so there is no per-field mapping
//! in this crate for a permutation to live in. A permuted or dropped CPU field would have to be
//! permuted or dropped inside `crates/z80`, where `tests/cpu_state.rs` grades it. What R1 owns
//! alone is the three fields that are **not** the CPU's — the border, the frame position and
//! the banks — and each has an assertion below that owes nothing to the round trip.
//!
//! # The fixture is hand-transcribed, and it checks itself
//!
//! It is `snapshot_common`'s version 3 vector, whose bytes were written from the format
//! description one offset at a time and whose expected values were written separately. Two
//! fields no format carries are set on top of it, because a field the fixture leaves at its
//! default is a field the round trip cannot see. `the_fixture_can_separate_a_permutation`
//! asserts that every value is distinct and none is zero — otherwise a permuted read is
//! invisible even to a hand-written expectation, and `docs/STATUS.md`'s *"zero is the one value
//! that makes a positive false claim"* applies unchanged.

mod snapshot_common;

use spectrum::memory::{BankIndex, PAGE_SIZE, Slot};
use spectrum::snapshot::{Snapshot, z80};
use spectrum::tape::Tape;
use spectrum::timing::{FIRST_CONTENDED_T_STATE, T_STATES_PER_FRAME};
use spectrum::{Colour, Spectrum};
// `spectrum::snapshot::z80` is the *format*; `::z80` is the CPU crate. Both are named `z80`
// and this file needs items from each, so the crate is always reached by an absolute path.
use ::z80::Bus;

/// A ROM of `NOP`s.
///
/// The applier never touches ROM — no format carries one — so this gate needs no corpus, and a
/// gate that runs on a bare clone is worth more than one that does not.
fn machine() -> Spectrum {
    Spectrum::new(&[0x00; PAGE_SIZE]).expect("a page-sized ROM")
}

/// The fixture: the transcribed version 3 vector, plus the two fields it cannot carry.
///
/// `wz` and `halted` are set here because **no snapshot format has them** —
/// `snapshot::UNPRESERVED` names both — so a fixture built only from a file would leave them
/// at zero and false, and R1 would then be blind to an applier that dropped them. They travel
/// through `CpuState`, and this is what makes the round trip say so.
fn fixture() -> Snapshot {
    let mut snapshot =
        z80::parse(&snapshot_common::v3_vector()).expect("the transcribed vector parses");
    snapshot.cpu.halted = true;
    snapshot.cpu.wz = 0x0F1E;
    snapshot
}

/// T-states the ULA stalls a contended access that starts on [`FIRST_CONTENDED_T_STATE`].
///
/// The first entry of the published delay pattern, written as a literal rather than read from
/// `timing::delay`, so this file's expectation is not computed by the code it is checking.
const STALL_AT_PHASE_ZERO: u32 = 6;

// ---------------------------------------------------------------------------------------
// The fixture, checked against itself before anything is built on it
// ---------------------------------------------------------------------------------------

#[test]
fn the_fixture_can_separate_a_permutation() {
    // Without this a permuted field is invisible to every assertion below: two fields holding
    // the same value swap identically, and a field left at its default looks restored.
    let snapshot = fixture();
    let cpu = &snapshot.cpu;
    let values: Vec<(&str, u32)> = vec![
        ("af", u32::from(cpu.af)),
        ("bc", u32::from(cpu.bc)),
        ("de", u32::from(cpu.de)),
        ("hl", u32::from(cpu.hl)),
        ("af_shadow", u32::from(cpu.af_shadow)),
        ("bc_shadow", u32::from(cpu.bc_shadow)),
        ("de_shadow", u32::from(cpu.de_shadow)),
        ("hl_shadow", u32::from(cpu.hl_shadow)),
        ("ix", u32::from(cpu.ix)),
        ("iy", u32::from(cpu.iy)),
        ("sp", u32::from(cpu.sp)),
        ("pc", u32::from(cpu.pc)),
        ("i", u32::from(cpu.i)),
        ("r", u32::from(cpu.r)),
        ("wz", u32::from(cpu.wz)),
        ("q", u32::from(cpu.q)),
        ("border", u32::from(snapshot.border.index())),
        ("frame_t_state", snapshot.frame_t_state),
    ];
    for &(name, value) in &values {
        assert_ne!(value, 0, "{name} is zero, so a dropped {name} is invisible");
    }
    for (index, &(name, value)) in values.iter().enumerate() {
        for &(other, other_value) in values.iter().skip(index + 1) {
            assert_ne!(
                value, other_value,
                "{name} and {other} are both {value:#X}, so swapping them is invisible"
            );
        }
    }
    // The two booleans have to differ from each other for the same reason, and `.sna` makes
    // exactly this mistake legitimately — it stores only `IFF2` and infers `IFF1` from it.
    assert!(cpu.iff1);
    assert!(!cpu.iff2);
    assert!(cpu.halted);
    // ...and every field must differ from what a fresh machine holds, or a `restore` that did
    // nothing at all would pass.
    let fresh = machine().snapshot();
    assert_ne!(fresh.cpu, snapshot.cpu);
    assert_ne!(fresh.border, snapshot.border);
    assert_ne!(fresh.frame_t_state, snapshot.frame_t_state);
}

#[test]
fn the_fixture_carries_every_bank_the_machine_exposes() {
    // The seam `docs/M6.md` Decision 2 names: a bank present in a snapshot and absent from the
    // slot map is silently dropped by `restore`. Asserted against the machine rather than
    // reasoned about, from both ends — the fixture's banks and the machine's.
    let carried: Vec<u8> = fixture().banks_carried();
    let exposed: Vec<u8> = exposed_banks();
    assert_eq!(carried, exposed, "the snapshot and the machine must agree");
    assert_eq!(carried.len(), 3, "a 48K exposes three RAM banks");
}

/// The bank numbers the 48K slot map shows, ascending.
fn exposed_banks() -> Vec<u8> {
    let mut banks: Vec<u8> = machine()
        .memory()
        .slots()
        .into_iter()
        .filter_map(|slot| match slot {
            Slot::Bank(bank) => Some(bank.get()),
            Slot::Rom(_) => None,
        })
        .collect();
    banks.sort_unstable();
    banks
}

/// The bank numbers a snapshot carries, ascending — through the one public accessor there is.
trait BanksCarried {
    fn banks_carried(&self) -> Vec<u8>;
}

impl BanksCarried for Snapshot {
    fn banks_carried(&self) -> Vec<u8> {
        (0..u8::try_from(spectrum::memory::BANK_COUNT).expect("eight banks"))
            .filter(|&bank| self.bank(BankIndex::new(bank)).is_some())
            .collect()
    }
}

// ---------------------------------------------------------------------------------------
// R1 — the applier
// ---------------------------------------------------------------------------------------

#[test]
fn r1_a_snapshot_survives_a_trip_through_the_machine() {
    // `snapshot(restore(s)) == s`. Format-free, corpus-free, and the test that catches
    // `restore` forgetting a field.
    let snapshot = fixture();
    let mut machine = machine();
    machine
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(machine.snapshot(), snapshot);
}

#[test]
fn r1_holds_from_a_machine_that_was_already_running() {
    // The same trip onto a machine whose every field already differs from the fixture's, so a
    // `restore` that skipped a field leaves the *old* value rather than a default — which is
    // the case a fresh machine cannot distinguish from a correct restore of a zero.
    let mut machine = machine();
    machine.run_frames(3);
    for address in [0x4000_u16, 0x8000, 0xC000] {
        machine.memory_mut().write(address, 0x5A);
    }
    machine.ula_mut().out_port(0x00FE, 2);

    let snapshot = fixture();
    machine
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(machine.snapshot(), snapshot);
}

#[test]
fn restoring_twice_is_the_same_as_restoring_once() {
    // Idempotence, which is what a load has to be: `docs/M6.md`'s ruling that `restore` steps
    // nothing and charges no contention would be false if a second application moved anything.
    let snapshot = fixture();
    let mut once = machine();
    once.restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    let mut twice = machine();
    twice
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    twice
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(once.snapshot(), twice.snapshot());
}

// ---------------------------------------------------------------------------------------
// R3 — the applier and the codec compose, over bytes
// ---------------------------------------------------------------------------------------

#[test]
fn r3_a_canonical_file_survives_a_trip_through_the_machine() {
    // `write(snapshot(restore(parse(f)))) == f`, for an `f` our own writer produced — which is
    // what makes byte-identity the right comparison rather than an over-claim. A foreign file
    // in a foreign encoding re-serialises to different bytes for the same machine, and that is
    // correct behaviour; `docs/M6.md` Decision 7 says so and `snapshot_corpus.rs` is where a
    // foreign file is graded.
    let file = z80::write(&fixture());
    let parsed = z80::parse(&file).expect("our own writer produces a readable file");
    let mut machine = machine();
    machine
        .restore(&parsed)
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(z80::write(&machine.snapshot()), file);
}

// ---------------------------------------------------------------------------------------
// The three things R1 and R3 are both blind to
// ---------------------------------------------------------------------------------------

#[test]
fn a_restored_bank_lands_where_the_48k_memory_map_says() {
    // The asymmetric anchor for the bank→address mapping. Both halves of the applier consult
    // the same slot map, so inverting it inverts both and every round trip stays green — the
    // measured mutation in this file's documentation. These addresses are the 48K's published
    // map, written in `snapshot_common` beside the page numbers they came from, and they owe
    // nothing to `crates/spectrum/src/lib.rs`.
    let mut machine = machine();
    machine
        .restore(&fixture())
        .expect("both machines are 48K, so a restore cannot be refused");
    for (base, bank, fill) in snapshot_common::FILL {
        assert_eq!(
            machine.memory().read(base),
            fill,
            "bank {bank} should be visible at {base:#06X}"
        );
        let last = base.wrapping_add(u16::try_from(PAGE_SIZE - 1).expect("a page fits"));
        assert_eq!(machine.memory().read(last), fill, "and to the end of it");
    }
    // ...and the ROM is untouched, because no format carries one.
    assert_eq!(machine.memory().read(0x0000), 0x00);
}

#[test]
fn a_restore_does_not_rewind_the_machines_uptime() {
    // `frames()` is the machine's uptime since power-on: the boot gate asserts on it and the
    // FLASH phase derives from it, so rewinding it on a load would make one number mean two
    // things. No format carries a frame count, so this is a **convention** rather than a
    // measurement — and its cost is that a snapshot taken mid-flash renders inverted for up to
    // `screen::FLASH_FRAMES` frames after loading.
    let mut machine = machine();
    machine.run_frames(7);
    assert_eq!(machine.frames(), 7);
    machine
        .restore(&fixture())
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(machine.frames(), 7, "a load is not a reset");
    assert_eq!(
        machine.frame_t_state(),
        fixture().frame_t_state,
        "but the position within the frame is the snapshot's"
    );
}

#[test]
fn a_restore_leaves_no_machine_cycle_half_open() {
    // Why `Ula::set_border` exists rather than a route through `Bus::out_port`. A port cycle
    // arms the ULA's "this many T-states are already paid for" counter at four, and a restore
    // that opened one would leave it armed with no cycle to spend it — so the next four bare
    // ticks would be charged as an open cycle's own and skip their contention.
    //
    // Bare ticks rather than an instruction, deliberately: an instruction begins with a fetch,
    // which overwrites the counter, so `Spectrum::step` cannot see this. What can is the
    // interrupt acknowledge, which `crates/z80` delivers as seven bare `tick` calls with no
    // transfer to reset the count — and a test driving the bus directly, which is this.
    let mut snapshot = fixture();
    snapshot.frame_t_state = FIRST_CONTENDED_T_STATE;
    let mut machine = machine();
    machine
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");

    machine.ula_mut().tick(0x4000);
    assert_eq!(
        machine.frame_t_state(),
        FIRST_CONTENDED_T_STATE + STALL_AT_PHASE_ZERO + 1,
        "a tick after a restore must be a standalone internal cycle and contend on its own \
         account; if it cost one T-state, a cycle was left half-open"
    );
}

#[test]
fn a_restore_does_not_move_the_tape() {
    // The second consequence of routing a restore through the bus: a port cycle advances the
    // clock by its contention stall, and the tape advances with the clock. Restoring a
    // snapshot is not elapsed time.
    //
    // **The clock has to be inside the display window when `restore` runs**, and getting that
    // wrong is what made the first version of this test unable to fail. A ULA port access is
    // only stalled during the display's fetch window, and `restore` sets the border *before*
    // it sets the frame position — so a machine whose clock is at the top of the frame charges
    // a stall of zero, the tape does not move, and the mutation is invisible. Measured, not
    // reasoned: mutation B turned exactly one gate red until this test was positioned.
    //
    // The first restore is what positions it; the second is the one under test.
    let mut snapshot = fixture();
    snapshot.frame_t_state = FIRST_CONTENDED_T_STATE;
    let mut machine = machine();
    machine
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(machine.frame_t_state(), FIRST_CONTENDED_T_STATE);

    machine.insert_tape(Tape::new(vec![1; 8]));
    machine.tape_mut().play();
    machine
        .restore(&snapshot)
        .expect("both machines are 48K, so a restore cannot be refused");
    assert!(
        !machine.tape_mut().level(),
        "the head moved during a restore, so something charged elapsed time"
    );
}

#[test]
fn a_restore_does_not_eject_the_tape() {
    // Loading a snapshot does not take the cassette out of the drive, for the same reason
    // `Ula::reset` does not rewind it.
    let mut machine = machine();
    machine.insert_tape(Tape::new(vec![4, 4]));
    machine
        .restore(&fixture())
        .expect("both machines are 48K, so a restore cannot be refused");
    assert_eq!(machine.tape_mut().pulses(), &[4, 4]);
}

// ---------------------------------------------------------------------------------------
// The contract at its edges
// ---------------------------------------------------------------------------------------

#[test]
fn a_frame_position_at_or_past_the_frame_length_rolls_over() {
    // `Snapshot`'s documented contract: values at or above `T_STATES_PER_FRAME` roll over when
    // applied, as the clock does everywhere else. Both parsers already reduce into range, so
    // this is the promise a `Snapshot` built in code relies on.
    let mut snapshot = fixture();
    for (given, expected) in [
        (T_STATES_PER_FRAME, 0),
        (T_STATES_PER_FRAME + 7, 7),
        (u32::MAX, u32::MAX % T_STATES_PER_FRAME),
    ] {
        snapshot.frame_t_state = given;
        let mut machine = machine();
        machine
            .restore(&snapshot)
            .expect("both machines are 48K, so a restore cannot be refused");
        assert_eq!(machine.frame_t_state(), expected, "given {given}");
        assert_eq!(machine.frames(), 0, "rolling over is not advancing");
    }
}

#[test]
fn a_snapshot_of_a_fresh_machine_is_the_machine_it_came_from() {
    // The degenerate case, and the one that would catch `snapshot` inventing a value: a
    // machine that has done nothing must snapshot to zeros, a black border, and three banks of
    // zeroed RAM.
    let snapshot = machine().snapshot();
    assert_eq!(snapshot.border, Colour::BLACK);
    assert_eq!(snapshot.frame_t_state, 0);
    assert_eq!(snapshot.cpu, ::z80::CpuState::default());
    for bank in snapshot.banks_carried() {
        let page = snapshot
            .bank(BankIndex::new(bank))
            .expect("the bank was just listed");
        assert!(page.iter().all(|&byte| byte == 0), "bank {bank}");
    }
}
