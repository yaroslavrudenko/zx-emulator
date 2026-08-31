//! The M1 conformance gate: replay the FUSE corpus's un-prefixed vectors and assert
//! every register, every flag bit (undocumented 3 and 5 included), every memory location
//! the vector lists, and the T-state total.
//!
//! Prefixed opcodes (`CB` / `ED` / `DD` / `FD`) are M2 work. They are **counted and
//! printed**, never silently dropped — a suite that quietly ignores three quarters of its
//! corpus is indistinguishable from one that passes it.

mod common;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use common::machine::{Completion, Machine};
use common::report::{self, CorpusOmission, Mismatch};
use common::vectors::{self, Transfer, Vector};

/// Vectors where the corpus omits a memory read the hardware genuinely performs.
///
/// # Why the harness carries this and not the core
///
/// The core models the Z80; the harness models the corpus, **including its known
/// limitations**. FUSE contends the operand addresses of a not-taken `JP cc` / `JR cc` /
/// `DJNZ` without recording a read — internally it puts the address on the bus but never
/// asserts MREQ. The Z80 does read. Five pieces of evidence, strongest first:
///
/// 1. **Zilog documents `JP cc,nn` as a single cycle count (10 T).** `CALL cc` is
///    documented 17/10 and `RET cc` 11/5. A machine cycle that does not happen changes the
///    count; `JP cc` has no second count to change to.
/// 2. **`PC` still advances by 3**, and on the Z80 `PC` increments *as part of* the
///    operand-fetch machine cycle — there is no separate "skip two bytes" mechanism.
/// 3. **MEMPTR:** `JP cc,nn` sets `WZ = nn` whether or not the branch is taken. That is a
///    hardware *measurement* (from the `BIT n,(HL)` experiments), not documentation — and
///    `WZ` cannot take a value the CPU never read.
/// 4. **The corpus's own asymmetry is the tell:** `CALL cc` genuinely differs in machine
///    cycles when not taken, `JP cc` does not — yet FUSE encodes both the same way.
/// 5. `crates/z80/src/instructions.rs:493-496` already documented the operand as fetched
///    either way; only the body disagreed.
///
/// `CALL cc` is deliberately **absent** from this list: its skip is real (vector `c4_2`
/// leaves `SP` untouched), so the core skips too and the corpus matches with no exception.
///
/// # This list cannot rot
///
/// Each entry names the exact addresses it permits, and everything the corpus *does*
/// record must still match in order and content. If an entry ever becomes unnecessary,
/// [`report::compare_transfers_allowing`] fails with "DELETE it" rather than passing
/// quietly — a suppression nobody needs is a suppression nobody notices.
const CORPUS_OMISSIONS: &[CorpusOmission] = &[
    // JR cc,d not taken — the displacement byte is fetched, then discarded.
    CorpusOmission {
        vector: "20_2",
        elided_reads: &[0x0001],
        reason: "JR NZ,d not taken: the displacement is fetched either way (7 T-states)",
    },
    CorpusOmission {
        vector: "28_1",
        elided_reads: &[0x0001],
        reason: "JR Z,d not taken: the displacement is fetched either way (7 T-states)",
    },
    CorpusOmission {
        vector: "30_2",
        elided_reads: &[0x0001],
        reason: "JR NC,d not taken: the displacement is fetched either way (7 T-states)",
    },
    CorpusOmission {
        vector: "38_1",
        elided_reads: &[0x0001],
        reason: "JR C,d not taken: the displacement is fetched either way (7 T-states)",
    },
    // JP cc,nn not taken — both operand bytes are fetched; WZ is set from them.
    CorpusOmission {
        vector: "c2_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP NZ,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "ca_1",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP Z,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "d2_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP NC,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "da_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP C,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "e2_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP PO,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "ea_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP PE,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "f2_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP P,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "fa_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "JP M,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    // CALL cc,nn not taken — 10 T-states is exactly 4 (fetch) + 3 + 3 (both operands).
    //
    // What CALL cc genuinely skips when not taken is the PUSH, not the operand fetch:
    // 17 T taken is 4 + 3 + 4 + 3 + 3, and the seven that disappear are the extra internal
    // T-state and the two stack writes. Vector `c4_2` pins that down — it expects `SP`
    // unchanged at 0x5698 and no `MW` at all, and the core matches on both. The operand
    // reads are the same corpus artefact as `JP cc`; the missing push is real behaviour.
    CorpusOmission {
        vector: "c4_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL NZ,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "cc_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL Z,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "d4_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL NC,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "dc_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL C,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "e4_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL PO,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "ec_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL PE,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "f4_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL P,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "fc_2",
        elided_reads: &[0x0001, 0x0002],
        reason: "CALL M,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    // DJNZ on its final iteration, when B reaches zero and the branch is not taken.
    CorpusOmission {
        vector: "10",
        elided_reads: &[0x0002],
        reason: "DJNZ with B == 0: the displacement is fetched before the branch is decided",
    },
];

fn omission_for(vector: &str) -> Option<&'static CorpusOmission> {
    CORPUS_OMISSIONS
        .iter()
        .find(|omission| omission.vector == vector)
}

/// The other half of "this list cannot rot": every declared omission must name a vector
/// that actually runs.
///
/// A typo, or a vector that leaves the corpus on a re-fetch, would otherwise sit in the
/// list forever — suppressing nothing, and telling every future reader that the case is
/// handled when it is not. (The complementary check, an omission that is no longer needed,
/// lives in `report::compare_transfers_allowing`.)
fn assert_omissions_are_live(m1: &[Vector]) {
    for omission in CORPUS_OMISSIONS {
        assert!(
            m1.iter().any(|vector| vector.name() == omission.vector),
            "CORPUS_OMISSIONS names vector {:?}, which is not in the executed M1 set — the \
             name is wrong, or the vector has left the corpus. Reason on file: {}",
            omission.vector,
            omission.reason,
        );
    }
    println!(
        "  corpus omissions declared:  {} (documented reads the corpus elides)",
        CORPUS_OMISSIONS.len()
    );
}

/// A floor on the number of un-prefixed vectors, not the exact count.
///
/// The point is to catch a truncated, empty or half-downloaded corpus — the failure mode
/// where `cargo test` exits 0 having verified nothing. A floor does that without breaking
/// when the corpus is re-fetched at a different revision. The exact counts are printed on
/// every run so drift stays visible.
const MIN_M1_VECTORS: usize = 250;

/// Full diagnostics are printed for this many failures. The one-line punch list above
/// them always covers every failing vector, so nothing is dropped.
const DETAILED_FAILURES: usize = 20;

struct Failure<'a> {
    vector: &'a Vector,
    mismatches: Vec<Mismatch>,
    transfers: Vec<Transfer>,
    tick_addresses: Vec<u16>,
}

#[test]
fn fuse_conformance_unprefixed() {
    let Some(corpus) = vectors::corpus_or_skip() else {
        return;
    };

    let total = corpus.len();
    let (m1, prefixed): (Vec<Vector>, Vec<Vector>) =
        corpus.into_iter().partition(Vector::is_m1_scope);

    print_scope_census(total, &m1, &prefixed);

    assert_eq!(
        total,
        m1.len() + prefixed.len(),
        "every vector must be either executed or explicitly skipped"
    );
    assert!(
        m1.len() >= MIN_M1_VECTORS,
        "only {} un-prefixed vectors were found, expected at least {MIN_M1_VECTORS}. \
         The corpus in {} looks truncated or incomplete — a run over too few vectors is a \
         false green.",
        m1.len(),
        vectors::corpus_dir().display(),
    );
    assert_omissions_are_live(&m1);

    let failures: Vec<Failure<'_>> = m1.iter().filter_map(run_vector).collect();

    println!(
        "M1 conformance: {executed} executed, {passed} passed, {failed} failed",
        executed = m1.len(),
        passed = m1.len() - failures.len(),
        failed = failures.len(),
    );

    assert!(failures.is_empty(), "{}", render_report(&failures));
}

/// Load one vector, step until its T-state budget is reached, and compare everything.
fn run_vector(vector: &Vector) -> Option<Failure<'_>> {
    let mut machine = Machine::load(&vector.setup);
    let completion = machine.run(vector.setup.state.t_states);
    let (registers, state) = machine.snapshot();

    let mut mismatches = report::compare(
        &vector.expected,
        &registers,
        &state,
        machine.transfers(),
        machine.tick_addresses(),
        omission_for(vector.name()),
        |addr| machine.read_memory(addr),
    );

    if completion == Completion::StepLimit {
        mismatches.insert(
            0,
            Mismatch::new("run", "reach the T-state budget", "hit the step limit").with_note(
                "the core reported too few T-states per instruction (often zero), so the \
                 run loop never reached the budget",
            ),
        );
    }

    // Two independent accounts of the same quantity — the core's own arithmetic against
    // the sum of what it actually charged the bus.
    if machine.reported_t_states() != state.t_states {
        mismatches.push(
            Mismatch::new(
                "T-states (step vs tick)",
                machine.reported_t_states().to_string(),
                state.t_states.to_string(),
            )
            .with_note(
                "Cpu::step returned a different total from what the bus accumulated through \
                 Bus::tick, so the instruction charged the machine for a different number of \
                 T-states than it claims to have taken",
            ),
        );
    }

    if let Some(fault) = machine.fault() {
        mismatches.push(
            Mismatch::new("fault", "none", format!("{fault:?}")).with_note(fault.to_string()),
        );
    }

    (!mismatches.is_empty()).then(|| Failure {
        vector,
        mismatches,
        transfers: machine.transfers().to_vec(),
        tick_addresses: machine.tick_addresses().to_vec(),
    })
}

fn print_scope_census(total: usize, m1: &[Vector], prefixed: &[Vector]) {
    let mut by_prefix: BTreeMap<&'static str, usize> = BTreeMap::new();
    for prefix in prefixed.iter().filter_map(|vector| vector.setup.prefix()) {
        *by_prefix.entry(prefix.name()).or_default() += 1;
    }

    println!(
        "FUSE corpus: {total} vectors from {}",
        vectors::corpus_dir().display()
    );
    println!("  in M1 scope (un-prefixed): {}", m1.len());
    println!("  skipped as M2 scope:       {}", prefixed.len());
    for (prefix, count) in &by_prefix {
        println!("    {prefix}-prefixed: {count}");
    }
}

/// Summary, then the census of which fields fail, then a complete punch list, then
/// detail. Ordered so the first screen answers "what do I fix first?".
fn render_report(failures: &[Failure<'_>]) -> String {
    let mut out = format!("\n{} M1 vector(s) failed.\n\n", failures.len());

    out.push_str("Failing fields, most common first — this is the M1 punch list:\n");
    for (field, count) in field_census(failures) {
        out.push_str(&format!("  {count:>4} x {field}\n"));
    }

    out.push_str("\nEvery failing vector:\n");
    for failure in failures {
        let fields: Vec<&str> = failure
            .mismatches
            .iter()
            .map(|mismatch| mismatch.field.as_str())
            .collect();
        out.push_str(&format!(
            "  {name:<8} opcode {opcode:02x}  {count} mismatch(es): {fields}\n",
            name = failure.vector.name(),
            opcode = failure.vector.setup.opcode_at_pc(),
            count = failure.mismatches.len(),
            fields = fields.join(", "),
        ));
    }

    let shown = failures.len().min(DETAILED_FAILURES);
    out.push_str(&format!(
        "\nDetail for the first {shown} of {} (the list above is complete):\n\n",
        failures.len()
    ));
    for failure in failures.iter().take(DETAILED_FAILURES) {
        out.push_str(&report::render_failure(
            failure.vector,
            &failure.mismatches,
            &failure.transfers,
            &failure.tick_addresses,
        ));
        out.push('\n');
    }
    out
}

/// Collapse a field name to its category, so the census counts *kinds* of failure rather
/// than instances: `memory[1234]` -> `memory`, `bus address at T7` -> `bus address`,
/// `transfer #2` -> `transfer`. Without this, one broken instruction with eleven internal
/// T-states would contribute eleven distinct rows and bury the ranking it exists to give.
fn field_category(field: &str) -> &str {
    for delimiter in ["[", " at T", " #"] {
        if let Some((head, _)) = field.split_once(delimiter) {
            return head;
        }
    }
    field
}

/// How many vectors each kind of field broke in. Memory addresses collapse to `memory`
/// so one bad `LD (nn),A` does not drown the census in 64 distinct entries.
fn field_census(failures: &[Failure<'_>]) -> Vec<(String, usize)> {
    let mut census: BTreeMap<String, usize> = BTreeMap::new();
    for failure in failures {
        // Deduplicated within a vector: an instruction with seven wrong internal T-states
        // is ONE vector with a bus-address problem, not seven. The ranking answers "how
        // many instructions does this category break", which is what picks the next fix.
        let categories: BTreeSet<&str> = failure
            .mismatches
            .iter()
            .map(|mismatch| field_category(&mismatch.field))
            .collect();
        for category in categories {
            *census.entry(category.to_owned()).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = census.into_iter().collect();
    ranked.sort_by_key(|(field, count)| (Reverse(*count), field.clone()));
    ranked
}
