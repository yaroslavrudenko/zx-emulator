//! Expectations for the transcribed snapshot files, written independently of the code.
//!
//! The bytes live in [`snapshot_common`]; the expectations live here. Neither is derived from
//! the other, and neither is derived from `crates/spectrum/src/snapshot`. `docs/M6.md`
//! Decision 7 is why: a round trip cannot see a field permuted with another of the same
//! width, a field read from the wrong offset by both sides, or a field dropped in both
//! directions — and this file is one of the two things that can.
//!
//! The other is [`the_fixture_can_separate_a_permutation_from_a_correct_read`], which asserts
//! that the fixture's own fields are pairwise distinct and none of them zero. Without it a
//! permutation is invisible even to a hand-written expectation, and a dropped field looks
//! like a correct default.

mod snapshot_common;

use snapshot_common::{
    BORDER, FILL, FRAME_T_STATE, I, R, REGISTERS, expected, sna_vector, v1_vector, v2_vector,
    v3_vector,
};
use spectrum::memory::BankIndex;
use spectrum::snapshot::{self, sna};
use z80::InterruptMode;

// ---------------------------------------------------------------------------------------
// The expectations, asserted field by field.
// ---------------------------------------------------------------------------------------

/// Assert every register of a parsed snapshot against the fixture, by name.
fn assert_registers(snapshot: &snapshot::Snapshot, what: &str) {
    let cpu = &snapshot.cpu;
    for (name, value) in [
        ("af", cpu.af),
        ("bc", cpu.bc),
        ("de", cpu.de),
        ("hl", cpu.hl),
        ("af_shadow", cpu.af_shadow),
        ("bc_shadow", cpu.bc_shadow),
        ("de_shadow", cpu.de_shadow),
        ("hl_shadow", cpu.hl_shadow),
        ("ix", cpu.ix),
        ("iy", cpu.iy),
        ("sp", cpu.sp),
        ("pc", cpu.pc),
    ] {
        assert_eq!(value, expected(name), "{what}: {name}");
    }
    assert_eq!(cpu.i, I, "{what}: I");
    assert_eq!(
        cpu.r, R,
        "{what}: R — seven bits at offset 11 and the eighth in bit 0 of byte 12"
    );
    assert_eq!(cpu.im, InterruptMode::Mode2, "{what}: interrupt mode");
    assert_eq!(snapshot.border.index(), BORDER, "{what}: border");
}

/// Assert the memory image, region by region.
fn assert_memory(snapshot: &snapshot::Snapshot, what: &str) {
    for &(address, bank, byte) in &FILL {
        let page = snapshot
            .bank(BankIndex::new(bank))
            .unwrap_or_else(|| panic!("{what}: bank {bank} ({address:#06X}) is missing"));
        assert!(
            page.iter().all(|&b| b == byte),
            "{what}: bank {bank} should be all {byte:#04X}"
        );
    }
    assert_eq!(
        snapshot.bank(BankIndex::new(1)),
        None,
        "{what}: a 48K carries three banks and no more"
    );
}

#[test]
fn the_fixture_can_separate_a_permutation_from_a_correct_read() {
    // The assertion the fixture makes about *itself*. Without it, a parser that swapped `de`
    // and `hl` would still satisfy every expectation above if the two happened to be equal,
    // and a parser that dropped a field would look like one reading a correct default.
    for &(name, value) in &REGISTERS {
        assert_ne!(
            value, 0,
            "{name} must not be zero: zero is the one value that \
                              makes a dropped field look like a correct read"
        );
    }
    for (index, &(name, value)) in REGISTERS.iter().enumerate() {
        for &(other_name, other) in REGISTERS.iter().skip(index + 1) {
            assert_ne!(
                value, other,
                "{name} and {other_name} are equal, so a permutation of the two is invisible"
            );
        }
    }
    assert_ne!(I, 0);
    assert_ne!(R, 0);
    assert_ne!(I, R);
    assert_ne!(BORDER, 0);
    assert_ne!(FRAME_T_STATE, 0);
    assert_eq!(
        FRAME_T_STATE, 40_000,
        "the version 3 vector's counter bytes were derived by hand from this value; \
         changing it makes that derivation stale rather than wrong-looking"
    );
    assert_ne!(
        FRAME_T_STATE % (69888 / 4),
        0,
        "the frame position must not sit on a quarter boundary, where a wrong T-state \
         formula could still produce the right low word"
    );
    let fills: Vec<u8> = FILL.iter().map(|&(_, _, byte)| byte).collect();
    for (index, byte) in fills.iter().enumerate() {
        assert!(
            !fills.iter().skip(index + 1).any(|other| other == byte),
            "the three regions must be distinguishable, or a mis-mapped bank is invisible"
        );
    }
}

#[test]
fn a_transcribed_version_1_file_reads_as_the_state_it_encodes() {
    let snapshot = snapshot::z80::parse(&v1_vector()).expect("the transcribed version 1 vector");
    assert_registers(&snapshot, "v1");
    assert_memory(&snapshot, "v1");
    assert!(snapshot.cpu.iff1, "v1: byte 27 is non-zero");
    assert!(!snapshot.cpu.iff2, "v1: byte 28 is zero");
    assert_eq!(
        snapshot.frame_t_state, 0,
        "version 1 has no T-state field, so the frame position is the top of the frame"
    );
}

#[test]
fn a_transcribed_version_2_file_reads_as_the_state_it_encodes() {
    let snapshot = snapshot::z80::parse(&v2_vector()).expect("the transcribed version 2 vector");
    assert_registers(&snapshot, "v2");
    assert_memory(&snapshot, "v2");
    assert_eq!(
        snapshot.frame_t_state, 0,
        "the 23-byte additional header ends at offset 54, one byte short of the counter"
    );
}

#[test]
fn a_transcribed_version_3_file_reads_as_the_state_it_encodes() {
    let snapshot = snapshot::z80::parse(&v3_vector()).expect("the transcribed version 3 vector");
    assert_registers(&snapshot, "v3");
    assert_memory(&snapshot, "v3");
    assert!(snapshot.cpu.iff1);
    assert!(!snapshot.cpu.iff2);
    assert_eq!(
        snapshot.frame_t_state, FRAME_T_STATE,
        "the counter at offsets 55-57, decoded: quarter 1 of the high byte and 12415 left \
         on the low word"
    );
}

#[test]
fn a_transcribed_sna_reads_as_the_state_it_encodes() {
    let snapshot = sna::parse(&sna_vector()).expect("the transcribed .sna vector");
    let cpu = &snapshot.cpu;
    // Every register the format stores as a field. `sp` is absent from this list on purpose:
    // it moves, because the pop happened.
    for (name, value) in [
        ("af", cpu.af),
        ("bc", cpu.bc),
        ("de", cpu.de),
        ("hl", cpu.hl),
        ("af_shadow", cpu.af_shadow),
        ("bc_shadow", cpu.bc_shadow),
        ("de_shadow", cpu.de_shadow),
        ("hl_shadow", cpu.hl_shadow),
        ("ix", cpu.ix),
        ("iy", cpu.iy),
    ] {
        assert_eq!(value, expected(name), ".sna: {name}");
    }
    assert_eq!(snapshot.cpu.i, I);
    assert_eq!(
        snapshot.cpu.r, R,
        ".sna stores all eight bits of R in one byte"
    );
    assert_eq!(snapshot.border.index(), BORDER);
    assert_eq!(snapshot.cpu.im, InterruptMode::Mode2);

    // The defining property of the format: PC is not a field, it is on the guest's stack.
    assert_eq!(snapshot.cpu.pc, expected("pc"), "PC popped from 0x8000");
    assert_eq!(snapshot.cpu.sp, 0x8002, "and SP advanced past it");
    assert!(
        snapshot.cpu.iff1 && snapshot.cpu.iff2,
        "only IFF2 is stored; iff1 = iff2 is the convention the format forces"
    );
    assert_eq!(snapshot.frame_t_state, 0, "no T-state counter exists");
}

#[test]
fn the_three_z80_versions_agree_about_everything_they_all_carry() {
    // Three encodings, one machine. A version-specific defect — a wrong additional-header
    // offset, say — moves one of these and not the others.
    let v1 = snapshot::z80::parse(&v1_vector()).expect("v1");
    let v2 = snapshot::z80::parse(&v2_vector()).expect("v2");
    let v3 = snapshot::z80::parse(&v3_vector()).expect("v3");
    assert_eq!(v1.cpu, v2.cpu, "v1 and v2 describe the same CPU");
    assert_eq!(v2.cpu, v3.cpu, "v2 and v3 describe the same CPU");
    assert_eq!(v1.border, v3.border);
    for &(_, bank, _) in &FILL {
        let index = BankIndex::new(bank);
        assert_eq!(v1.bank(index), v3.bank(index), "bank {bank}");
        assert_eq!(v2.bank(index), v3.bank(index), "bank {bank}");
    }
    // The one thing they do *not* agree about, and it is the reason the writer emits v3.
    assert_eq!((v1.frame_t_state, v2.frame_t_state), (0, 0));
    assert_eq!(v3.frame_t_state, FRAME_T_STATE);
}

#[test]
fn the_two_formats_agree_about_the_state_they_both_carry() {
    // Cross-format agreement, which `docs/M6.md`'s implementation order names as `.sna`'s
    // gate. `.sna` has no round trip of its own — there is no writer — so this is what
    // grades its offsets against something other than itself.
    let from_sna = sna::parse(&sna_vector()).expect(".sna");
    let from_z80 = snapshot::z80::parse(&v3_vector()).expect(".z80 v3");

    assert_eq!(from_sna.cpu.af, from_z80.cpu.af);
    assert_eq!(from_sna.cpu.bc, from_z80.cpu.bc);
    assert_eq!(from_sna.cpu.de, from_z80.cpu.de);
    assert_eq!(from_sna.cpu.hl, from_z80.cpu.hl);
    assert_eq!(from_sna.cpu.af_shadow, from_z80.cpu.af_shadow);
    assert_eq!(from_sna.cpu.bc_shadow, from_z80.cpu.bc_shadow);
    assert_eq!(from_sna.cpu.de_shadow, from_z80.cpu.de_shadow);
    assert_eq!(from_sna.cpu.hl_shadow, from_z80.cpu.hl_shadow);
    assert_eq!(from_sna.cpu.ix, from_z80.cpu.ix);
    assert_eq!(from_sna.cpu.iy, from_z80.cpu.iy);
    assert_eq!(from_sna.cpu.i, from_z80.cpu.i);
    assert_eq!(from_sna.cpu.r, from_z80.cpu.r);
    assert_eq!(from_sna.cpu.pc, from_z80.cpu.pc);
    assert_eq!(from_sna.cpu.im, from_z80.cpu.im);
    assert_eq!(from_sna.border, from_z80.border);

    // Where they legitimately differ, and why — stated rather than skipped over.
    assert_eq!(
        from_sna.cpu.sp, 0x8002,
        ".sna: SP advanced past the popped PC"
    );
    assert_eq!(
        from_z80.cpu.sp,
        expected("sp"),
        ".z80: SP is a field at offset 8"
    );
    for &(address, bank, _) in &FILL {
        let index = BankIndex::new(bank);
        if address == 0x8000 {
            // The `.sna`'s stack sits at 0x8000, so this bank legitimately holds the two
            // stale bytes `PC` was popped from — which the format says to leave in place,
            // because the snapshot's RAM genuinely contained them.
            let page = from_sna.bank(index).expect("the .sna carries this bank");
            assert_eq!(
                u16::from_le_bytes([page[0], page[1]]),
                expected("pc"),
                "the popped PC is still in RAM where it was"
            );
            continue;
        }
        assert_eq!(from_sna.bank(index), from_z80.bank(index), "bank {bank}");
    }
}

#[test]
fn a_version_3_file_written_from_a_parsed_one_is_byte_identical() {
    // R3: `write(parse(f)) == f` for an `f` already in our canonical form. It grades that R1
    // and R2 compose, and it is meaningful *only* because the vector was written in that
    // canonical form deliberately — byte-identity over a foreign file is neither achievable
    // nor desirable, per `docs/M6.md` Decision 7.
    let file = v3_vector();
    let snapshot = snapshot::z80::parse(&file).expect("the transcribed version 3 vector");
    let written = snapshot::z80::write(&snapshot);
    assert_eq!(
        written.len(),
        file.len(),
        "the re-written file changed length"
    );
    let first_difference = written
        .iter()
        .zip(&file)
        .position(|(left, right)| left != right);
    assert_eq!(
        first_difference,
        None,
        "first differing offset; written {:02X?} vs transcribed {:02X?}",
        first_difference.and_then(|at| written.get(at)),
        first_difference.and_then(|at| file.get(at)),
    );
}

#[test]
fn a_snapshot_survives_being_written_and_read_back() {
    // R2: `parse(write(s)) == s`. It grades the codec — header offsets, page framing,
    // compression — and it is blind to any offset `parse` and `write` get *equally* wrong,
    // which is what the transcribed vectors above are for.
    for (what, file) in [
        ("v1", v1_vector()),
        ("v2", v2_vector()),
        ("v3", v3_vector()),
        ("sna", sna_vector()),
    ] {
        let original = if what == "sna" {
            sna::parse(&file).expect("sna")
        } else {
            snapshot::z80::parse(&file).expect("z80")
        };
        let reparsed =
            snapshot::z80::parse(&snapshot::z80::write(&original)).expect("our own output");
        assert_eq!(reparsed, original, "{what} did not survive the round trip");
    }
}

#[test]
fn every_field_no_format_carries_is_proven_to_be_dropped() {
    // The defect a round trip cannot see is a field dropped in *both* directions: the parser
    // leaves it at its default and the writer always emits that default, so the comparison
    // is green because both sides agreed to lose it. `docs/STATUS.md` records this exact
    // shape as the M4 `q` defect — "zero is the one value that makes a positive false
    // claim". So the drops are enumerated in `snapshot::UNPRESERVED` and each one is checked
    // here against the value the list says it takes.
    let names: Vec<&str> = snapshot::UNPRESERVED
        .iter()
        .map(|&(name, _)| name)
        .collect();
    assert_eq!(
        names,
        ["wz", "q", "halted"],
        "a field that quietly joined this list would look exactly like one that survives"
    );

    for (what, snapshot) in [
        ("v1", snapshot::z80::parse(&v1_vector()).expect("v1")),
        ("v3", snapshot::z80::parse(&v3_vector()).expect("v3")),
        ("sna", sna::parse(&sna_vector()).expect("sna")),
    ] {
        assert_eq!(snapshot.cpu.wz, 0, "{what}: no format names MEMPTR");
        assert!(!snapshot.cpu.halted, "{what}: no format has a halt flag");
        assert_eq!(
            snapshot.cpu.q,
            (snapshot.cpu.af & 0xFF) as u8,
            "{what}: the latch is derived from F, not defaulted to zero — loading a state \
             is a POP AF, so the load is the last thing that wrote F"
        );
    }

    // And the derivation is not vacuous: `F` is non-zero in the fixture, so a parser that
    // defaulted `q` to zero would fail the assertion above rather than agreeing with it.
    assert_ne!(
        expected("af") & 0xFF,
        0,
        "F must be non-zero for that check to bite"
    );
}
