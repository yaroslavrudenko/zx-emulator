//! What the parsers do with files that are truncated, mutated, or hostile by construction.
//!
//! # Why this is a gate and not a nicety
//!
//! `crates/spectrum` builds with `panic = "abort"` in release, so a panic is not a
//! recoverable error — it kills the process, and `catch_unwind` is not available as a
//! backstop. A snapshot parser reads attacker-controlled lengths, counts and page numbers.
//! **This project has already shipped exactly that defect once**: `docs/STATUS.md` records a
//! `u8` T-state accumulator whose comment argued that no Z80 instruction exceeds 23
//! T-states, falsified the moment a run of `DD` prefixes became one instruction whose length
//! guest memory decides — turning guest memory *content* into a hard process abort.
//!
//! # Structure first, then behaviour — they are not substitutes
//!
//! `docs/M6.md` Decision 6 closes the class **by construction**, and that half is asserted in
//! `crates/spectrum/src/snapshot/mod.rs`: there is no indexing expression anywhere in the
//! module, no `unwrap`/`expect`/`panic!`, every file-derived quantity is widened or reduced,
//! and no allocation is ever sized from the file. Those are properties of the source and are
//! checked by reading it.
//!
//! This file asserts the behaviour that structure is supposed to produce. Structural
//! impossibility beats a passing test, so the structure is shown first — but a claim about
//! the source is a hypothesis about the binary until something runs it.

mod snapshot_common;

use snapshot_common::{page_of, sna_vector, v1_vector, v2_vector, v3_vector};
use spectrum::memory::BankIndex;
// Shadows the `z80` crate deliberately: nothing in this file needs the CPU.
use spectrum::snapshot::{Error, sna, z80};

// ---------------------------------------------------------------------------------------
// The exhaustive sweeps
// ---------------------------------------------------------------------------------------

/// One version 2/3 page block: a two-byte length, a page number, and 260 bytes of payload.
///
/// A uniform page is 64 runs of 255 and one of 64, four bytes each — which is why the
/// fixtures are small enough to sweep exhaustively.
const BLOCK_LEN: usize = 3 + 65 * 4;

#[test]
fn every_prefix_of_every_vector_returns_rather_than_aborting() {
    // Exhaustive over the axis that matters, deterministic, and it runs on every
    // `cargo test`. `docs/M6.md` Decision 6 asks for exactly this, and it is cheap only
    // because the fixtures were built to be small.
    //
    // **A strict prefix is not always an error, and pretending otherwise would be the
    // weaker test.** A version 2/3 file is a header followed by self-delimiting page blocks,
    // so a prefix ending on a block boundary is a well-formed snapshot carrying fewer banks.
    // Accepting that is a **ruling**: `Snapshot::bank` returns an `Option` precisely because
    // absence is representable, and every writer in circulation emits all three pages
    // anyway. What would change it is an observed file that omits a page and means something
    // by it. The exact boundaries are asserted, so an `Ok` anywhere else is a failure.
    for (what, file, header_len) in [
        // Version 1 is one fixed-length block plus a marker: no strict prefix is legal.
        ("v1", v1_vector(), None),
        ("v2", v2_vector(), Some(32 + 23)),
        ("v3", v3_vector(), Some(32 + 54)),
    ] {
        assert_eq!(
            file.len(),
            header_len.map_or(30 + 3 * 260 + 4, |header| header + 3 * BLOCK_LEN),
            "{what}: the transcribed vector is not the length its own structure implies"
        );
        for cut in 0..file.len() {
            let outcome = z80::parse(file.get(..cut).expect("a prefix of its own length"));
            let legal =
                header_len.is_some_and(|header| cut >= header && (cut - header) % BLOCK_LEN == 0);
            assert_eq!(
                outcome.is_ok(),
                legal,
                "{what} cut to {cut} bytes: a prefix is only legal on a page-block boundary"
            );
        }
    }

    // `.sna` is a fixed-length format, so **no** strict prefix of it is legal.
    let file = sna_vector();
    for cut in 0..file.len() {
        assert!(
            sna::parse(file.get(..cut).expect("a prefix")).is_err(),
            ".sna cut to {cut} bytes parsed as a whole snapshot"
        );
    }
}

#[test]
fn mutating_any_single_byte_of_a_valid_file_never_aborts() {
    // The truncation sweep varies length; this varies *content* on the same axis the parser
    // trusts least. `0xED` is included because it is the compression escape, so it turns
    // ordinary payload into a run-length token wherever it lands.
    let (mut accepted, mut refused) = (0_u32, 0_u32);
    for byte in [0x00_u8, 0xFF, 0xED, 0x01] {
        for (what, original) in [("v1", v1_vector()), ("v3", v3_vector())] {
            for offset in 0..original.len() {
                let mut mutated = original.clone();
                if let Some(target) = mutated.get_mut(offset) {
                    *target = byte;
                }
                // The verdict is deliberately not asserted per mutation: some produce a
                // different but perfectly legal snapshot. What is asserted is that the call
                // *returns* — and, below, that the sweep reaches both outcomes, so it cannot
                // be a sweep that fails at byte zero every time and proves nothing.
                match z80::parse(&mutated) {
                    Ok(_) => accepted += 1,
                    Err(_) => refused += 1,
                }
                let _ = what;
            }
        }
    }
    assert!(
        accepted > 0,
        "no mutation produced a legal file; the sweep proves nothing"
    );
    assert!(
        refused > 0,
        "no mutation was refused; the parser accepts anything"
    );
}

proptest::proptest! {
    #[test]
    fn arbitrary_bytes_are_parsed_or_refused_but_never_abort(
        bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..2000)
    ) {
        let _ = z80::parse(&bytes);
        let _ = sna::parse(&bytes);
    }

    /// Random mutations of a valid file, which reach deeper than random bytes do: a random
    /// buffer almost never has a plausible header, so it is refused in the first thirty bytes
    /// and exercises none of the page machinery.
    #[test]
    fn mutations_of_a_valid_file_are_parsed_or_refused_but_never_abort(
        edits in proptest::collection::vec((0..875_usize, proptest::num::u8::ANY), 0..12)
    ) {
        let mut file = v3_vector();
        for (offset, byte) in edits {
            if let Some(target) = file.get_mut(offset) {
                *target = byte;
            }
        }
        let _ = z80::parse(&file);
    }
}

// ---------------------------------------------------------------------------------------
// `docs/M6.md` Decision 6's hostile-input table, one test per row
// ---------------------------------------------------------------------------------------

/// A version 3 file whose *first* page block carries `data` under page number `page`.
///
/// Everything else stays valid, so a failure names the row under test rather than the
/// fixture.
fn v3_with_first_block(page: u8, data: &[u8]) -> Vec<u8> {
    let full = v3_vector();
    let mut bytes = full.get(..86).expect("the version 3 header").to_vec();
    let length = u16::try_from(data.len()).expect("test data fits a length word");
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.push(page);
    bytes.extend_from_slice(data);
    // The two remaining pages, untouched.
    bytes.extend_from_slice(full.get(86 + 263..).expect("the last two blocks"));
    bytes
}

#[test]
fn a_run_that_overruns_the_page_is_refused() {
    // Sixty-five runs of 255 is 16575 bytes for a 16384-byte page, so the last one has
    // nowhere to go. The destination is fixed, so this is a cursor bound and not a length
    // check — nothing here can allocate what the file asked for.
    let mut data = Vec::new();
    for _ in 0..65 {
        data.extend_from_slice(&[0xED, 0xED, 255, 0x5A]);
    }
    assert!(matches!(
        z80::parse(&v3_with_first_block(8, &data)),
        Err(Error::PageOverrun {
            capacity: 16384,
            ..
        })
    ));
}

#[test]
fn a_block_with_bytes_left_over_after_the_page_is_full_is_a_finding() {
    // The other side of the underrun ruling. The declared length said more than the page
    // needed, so either the length is wrong or the encoding is — and both are worth saying.
    let mut data = page_of(0x5A);
    data.extend_from_slice(&[0x99]);
    assert!(matches!(
        z80::parse(&v3_with_first_block(8, &data)),
        Err(Error::TrailingBytes { extra: 1, .. })
    ));
}

#[test]
fn a_run_length_of_zero_is_legal_and_consumes_its_four_bytes() {
    // Legal, emits nothing, and the loop still progresses — which is what makes termination
    // structural rather than argued.
    let mut data = vec![0xED, 0xED, 0x00, 0xFF];
    data.extend_from_slice(&page_of(0x5A));
    let snapshot = z80::parse(&v3_with_first_block(8, &data)).expect("a zero count is legal");
    assert!(
        snapshot
            .bank(BankIndex::new(5))
            .is_some_and(|page| page.iter().all(|&b| b == 0x5A))
    );
}

#[test]
fn an_escape_with_fewer_than_two_bytes_after_it_is_truncated() {
    for tail in [&[0xED, 0xED][..], &[0xED, 0xED, 0x05][..]] {
        assert!(
            matches!(
                z80::parse(&v3_with_first_block(8, tail)),
                Err(Error::Truncated { .. })
            ),
            "{tail:02X?}"
        );
    }
}

#[test]
fn a_block_that_ends_before_the_page_is_full_underruns() {
    // Strict rather than zero-filling, deliberately: a zero-filled tail is a wrong machine
    // that every round trip then agrees is right.
    assert!(matches!(
        z80::parse(&v3_with_first_block(8, &[0x01, 0x02, 0x03])),
        Err(Error::PageUnderrun {
            capacity: 16384,
            written: 3,
            ..
        })
    ));
}

#[test]
fn a_declared_page_length_longer_than_the_file_is_truncated() {
    let mut bytes = v3_vector().get(..86).expect("the header").to_vec();
    bytes.extend_from_slice(&0xFFFE_u16.to_le_bytes()); // just under the raw marker
    bytes.push(8);
    bytes.extend_from_slice(&[0x00; 4]); // and only four bytes actually present
    assert!(matches!(
        z80::parse(&bytes),
        Err(Error::Truncated {
            needed: 0xFFFE,
            available: 4,
            ..
        })
    ));
}

#[test]
fn a_page_length_of_ffff_means_sixteen_kilobytes_raw_and_not_a_length() {
    // The one value of the length word that is a *marker*. Reading it as 65535 would demand
    // a block the file cannot contain, so the failure mode is loud — but reading it as a
    // length at all is the mistake, and only a page a writer left raw would ever show it.
    let mut bytes = v3_vector().get(..86).expect("the header").to_vec();
    bytes.extend_from_slice(&0xFFFF_u16.to_le_bytes());
    bytes.push(8);
    bytes.extend_from_slice(&[0x77; 16384]);
    let snapshot = z80::parse(&bytes).expect("an uncompressed page is legal");
    assert!(
        snapshot
            .bank(BankIndex::new(5))
            .is_some_and(|page| page.iter().all(|&b| b == 0x77)),
        "0xFFFF must mean 16384 raw bytes"
    );
}

#[test]
fn a_page_number_a_48k_does_not_have_is_refused() {
    // The parse-don't-validate boundary: a `Snapshot` that reaches the machine cannot name a
    // bank the machine lacks, so the check is here rather than in the applier.
    for page in [0_u8, 1, 2, 3, 6, 7, 9, 10, 255] {
        assert_eq!(
            z80::parse(&v3_with_first_block(page, &page_of(0x5A))),
            Err(Error::UnknownPage { page }),
            "page {page}"
        );
    }
}

#[test]
fn the_same_page_twice_is_refused_rather_than_last_write_wins() {
    // The alternative is a machine built silently from two different snapshots of one bank.
    let full = v3_vector();
    let mut bytes = full.get(..86 + 263).expect("header and one block").to_vec();
    bytes.extend_from_slice(full.get(86..86 + 263).expect("that same block again"));
    assert_eq!(z80::parse(&bytes), Err(Error::DuplicatePage { page: 8 }));
}

#[test]
fn a_128_mode_carrying_a_48ks_pages_is_refused_rather_than_half_loaded() {
    // The 128 modes are accepted now, so the question is what happens to a file that *claims*
    // to be a 128 and carries a 48K's three page numbers. Under the 128's page mapping those
    // are three banks of eight, and a restore would leave the other five as whatever the
    // target machine happened to hold — the same silent half-load `ModelMismatch` refuses,
    // arriving through the parser instead.
    //
    // It is refused by the bank-set guard rather than by the mode check, which is why the
    // error names a **page** and not a mode.
    let mut v3 = v3_vector();
    for mode in [4_u8, 5, 6] {
        if let Some(byte) = v3.get_mut(34) {
            *byte = mode;
        }
        assert_eq!(
            z80::parse(&v3),
            Err(Error::MissingPage { page: 3 }),
            "version 3, mode {mode}"
        );
    }
}

#[test]
fn a_hardware_mode_that_is_not_a_48k_is_refused() {
    // **This test's premise moved and it is worth saying how.** It was written when this
    // parser accepted 48K modes only, so modes 4, 5 and 6 were refused as *unsupported*.
    // They are now the version 3 **128K** modes and are read as 128s — which is the fix for a
    // writer that had been emitting mode 0 for a 128, silently turning every 128 snapshot into
    // a 48K file carrying three of its eight banks.
    //
    // So what this now grades is narrower and sharper: the modes that name **no machine this
    // crate is** are refused, and the version disagreement about mode 3 still holds. The 128
    // modes moved to the test below, where they are asserted to be *accepted as 128s* — a
    // stronger claim than the one they used to satisfy here.
    //
    // The two versions disagree about mode 3, which is the trap: a **128K** in version 2 and a
    // 48K with a MGT interface in version 3.
    let mut v3 = v3_vector();
    for mode in [2_u8, 7, 9, 12, 255] {
        if let Some(byte) = v3.get_mut(34) {
            *byte = mode;
        }
        assert_eq!(
            z80::parse(&v3),
            Err(Error::UnsupportedHardware { mode }),
            "version 3, mode {mode}"
        );
    }
    if let Some(byte) = v3.get_mut(34) {
        *byte = 3;
    }
    assert!(
        z80::parse(&v3).is_ok(),
        "mode 3 is a 48K + M.G.T. in version 3"
    );

    // **The trap itself, and the refusal is now for a sharper reason than it used to be.**
    // Version 2's mode 3 is a 128K, and this vector carries a 48K's three page numbers. It
    // used to be refused as an unsupported mode; it is now *recognised* as a 128 and refused
    // by the bank-set guard, because three banks of eight is a half-load. Either way it does
    // not load as a 48K, which is the claim — but the second refusal is the stronger one,
    // since it survives this parser learning about 128s and the first did not.
    let mut v2 = v2_vector();
    if let Some(byte) = v2.get_mut(34) {
        *byte = 3;
    }
    let parsed = z80::parse(&v2);
    assert_eq!(
        parsed,
        Err(Error::MissingPage { page: 3 }),
        "mode 3 is a 128K in version 2 and must not load as a 48K"
    );
    assert!(
        parsed.is_err(),
        "whatever the reason, it must not become a 48K snapshot"
    );
}

#[test]
fn an_additional_header_length_no_version_declares_is_refused() {
    let mut bytes = v3_vector();
    for length in [0_u16, 22, 24, 53, 56, 0xFFFF] {
        if let Some(field) = bytes.get_mut(30..32) {
            field.copy_from_slice(&length.to_le_bytes());
        }
        assert_eq!(
            z80::parse(&bytes),
            Err(Error::UnsupportedVersion {
                extra_header_len: length
            }),
            "length {length}"
        );
    }
}

#[test]
fn a_version_1_block_without_its_end_marker_is_refused() {
    let full = v1_vector();
    let body = full.len() - 4;

    // The marker is there but wrong.
    let mut wrong = full.clone();
    if let Some(field) = wrong.get_mut(body..) {
        field.copy_from_slice(&[0x00, 0xED, 0xED, 0x01]);
    }
    assert_eq!(
        z80::parse(&wrong),
        Err(Error::MissingEndMarker { offset: body })
    );

    // The marker is not there at all.
    assert!(matches!(
        z80::parse(full.get(..body).expect("the body")),
        Err(Error::Truncated { .. })
    ));
}

#[test]
fn bytes_after_the_end_of_a_snapshot_are_a_finding() {
    // Strict, with a stated escape hatch: an observed real-world file that is legitimately
    // longer is what would change this ruling. A 128K `.sna` is exactly such a file at M7.
    for (what, mut file) in [("v1", v1_vector()), ("sna", sna_vector())] {
        let length = file.len();
        file.push(0x00);
        let outcome = if what == "sna" {
            sna::parse(&file)
        } else {
            z80::parse(&file)
        };
        assert_eq!(
            outcome,
            Err(Error::TrailingBytes {
                offset: length,
                extra: 1
            }),
            "{what}"
        );
    }
}

#[test]
fn an_interrupt_mode_the_z80_does_not_have_is_refused() {
    let mut bytes = v3_vector();
    if let Some(byte) = bytes.get_mut(29) {
        *byte = 0b11; // bits 0-1 hold the mode, and 3 is not a mode
    }
    assert_eq!(
        z80::parse(&bytes),
        Err(Error::InvalidInterruptMode { value: 3 })
    );
}

#[test]
fn the_byte_twelve_compatibility_hack_is_honoured() {
    // "If byte 12 is 255, it has to be regarded as being 1" — so the border is 0, bit 7 of
    // `R` is set, and the version 1 block is *not* compressed. Getting this wrong turns a
    // whole class of old files into garbage rather than into an error.
    let mut bytes = v1_vector();
    if let Some(byte) = bytes.get_mut(12) {
        *byte = 0xFF;
    }
    // The image is still run-length encoded, so with the flag read as 1 the block is taken
    // as a raw 49152-byte image — which this 784-byte one is not.
    assert!(
        matches!(
            z80::parse(&bytes),
            Err(Error::Truncated { needed: 49152, .. })
        ),
        "byte 12 = 255 must be read as 1, which clears the compression flag"
    );

    // And with a genuinely uncompressed image it reads clean, with border 0 and R bit 7 set.
    let mut raw = bytes.get(..30).expect("the header").to_vec();
    raw.extend_from_slice(&[0x42; 49152]);
    let snapshot = z80::parse(&raw).expect("an uncompressed version 1 file");
    assert_eq!(snapshot.border.index(), 0, "bits 1-3 of 0x01 are zero");
    assert_eq!(snapshot.cpu.r & 0x80, 0x80, "bit 0 of 0x01 is bit 7 of R");
}

#[test]
fn a_stack_pointer_outside_the_ram_image_is_refused() {
    let mut bytes = sna_vector();
    for sp in [0x0000_u16, 0x3FFF, 0xFFFF] {
        if let Some(field) = bytes.get_mut(23..25) {
            field.copy_from_slice(&sp.to_le_bytes());
        }
        assert_eq!(
            sna::parse(&bytes),
            Err(Error::StackPointerOutsideRam { sp }),
            "SP {sp:#06X}"
        );
    }
}

#[test]
fn an_empty_file_is_refused_by_both_parsers() {
    assert!(matches!(
        z80::parse(&[]),
        Err(Error::Truncated { offset: 0, .. })
    ));
    assert!(matches!(
        sna::parse(&[]),
        Err(Error::Truncated { offset: 0, .. })
    ));
}
