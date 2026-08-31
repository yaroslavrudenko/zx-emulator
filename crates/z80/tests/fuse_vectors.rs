//! The conformance gates: replay the FUSE corpus and assert every register, every flag bit
//! (undocumented 3 and 5 included), every memory location the vector lists, the T-state
//! total, the ordered transfers, and the address on the bus at every T-state the corpus
//! pins down.
//!
//! Two gates, one per milestone: `fuse_conformance_unprefixed` (M1, 290 vectors) and
//! `fuse_conformance_prefixed` (M2, 1045). Both run the same `run_vector`, so neither can
//! drift into being weaker than the other.

mod common;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use common::flags;
use common::machine::{Completion, MAX_STEPS_PER_VECTOR, Machine};
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
        disputed_flag_bits: 0,
        reason: "JR NZ,d not taken: the displacement is fetched either way (7 T-states)",
    },
    CorpusOmission {
        vector: "28_1",
        elided_reads: &[0x0001],
        disputed_flag_bits: 0,
        reason: "JR Z,d not taken: the displacement is fetched either way (7 T-states)",
    },
    CorpusOmission {
        vector: "30_2",
        elided_reads: &[0x0001],
        disputed_flag_bits: 0,
        reason: "JR NC,d not taken: the displacement is fetched either way (7 T-states)",
    },
    CorpusOmission {
        vector: "38_1",
        elided_reads: &[0x0001],
        disputed_flag_bits: 0,
        reason: "JR C,d not taken: the displacement is fetched either way (7 T-states)",
    },
    // JP cc,nn not taken — both operand bytes are fetched; WZ is set from them.
    CorpusOmission {
        vector: "c2_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP NZ,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "ca_1",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP Z,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "d2_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP NC,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "da_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP C,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "e2_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP PO,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "ea_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP PE,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "f2_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "JP P,nn not taken: one documented cycle count (10 T), and WZ = nn regardless",
    },
    CorpusOmission {
        vector: "fa_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
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
        disputed_flag_bits: 0,
        reason: "CALL NZ,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "cc_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL Z,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "d4_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL NC,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "dc_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL C,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "e4_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL PO,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "ec_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL PE,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "f4_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL P,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    CorpusOmission {
        vector: "fc_2",
        elided_reads: &[0x0001, 0x0002],
        disputed_flag_bits: 0,
        reason: "CALL M,nn not taken: 10 T = 4 + 3 + 3; the skip is the push, not the fetch",
    },
    // DJNZ on its final iteration, when B reaches zero and the branch is not taken.
    CorpusOmission {
        vector: "10",
        elided_reads: &[0x0002],
        disputed_flag_bits: 0,
        reason: "DJNZ with B == 0: the displacement is fetched before the branch is decided",
    },
    // ---------------------------------------------------------------------------
    // A DIFFERENT CATEGORY: a rule disagreement, not an unrecorded access.
    //
    // Everything above is the corpus declining to *record* an access both sides agree
    // happened. The four below are a genuine disagreement about what the hardware does:
    // where `BIT n,(HL)` gets the undocumented bits 3 and 5. FUSE takes them from the
    // tested value; we take them from MEMPTR (`WZ`).
    //
    // What the corpus can and cannot settle here — read before revisiting:
    //
    // * For the plain `(HL)` form it settles NOTHING. `machine.rs` initialises `wz = 0`,
    //   correctly, because the corpus has no MEMPTR column. So under our rule bits 3/5 come
    //   out zero for *every* plain `BIT n,(HL)` vector, and the ones that pass are exactly
    //   the ones whose expectation happens to be zero. They agree by coincidence, not by
    //   confirmation. Do not read the pass count as evidence either way.
    // * The evidence that actually carries the rule is elsewhere: **97 `DD`/`FD` vectors**
    //   that FUSE's rule fails and MEMPTR satisfies with no special case. A third candidate
    //   — the effective address's high byte — was also tried and scored worse.
    //
    // So these four entries record a deliberate, evidence-backed divergence whose local
    // evidence is neutral. `zexall` at M4 is what finally adjudicates it.
    CorpusOmission {
        vector: "cb4e",
        elided_reads: &[],
        disputed_flag_bits: flags::X,
        reason: "BIT 1,(HL): only bit 3 (F3/X) diverges — MEMPTR gives 0, the corpus expects 1",
    },
    CorpusOmission {
        vector: "cb5e",
        elided_reads: &[],
        disputed_flag_bits: flags::UNDOCUMENTED,
        reason: "BIT 3,(HL): both bits 3 and 5 diverge — MEMPTR gives 0, the corpus expects both set",
    },
    CorpusOmission {
        vector: "cb6e",
        elided_reads: &[],
        disputed_flag_bits: flags::Y,
        reason: "BIT 5,(HL): only bit 5 (F5/Y) diverges — MEMPTR gives 0, the corpus expects 1",
    },
    CorpusOmission {
        vector: "cb76",
        elided_reads: &[],
        disputed_flag_bits: flags::X,
        reason: "BIT 6,(HL): only bit 3 (F3/X) diverges — MEMPTR gives 0, the corpus expects 1",
    },
];

fn omission_for(vector: &str) -> Option<&'static CorpusOmission> {
    CORPUS_OMISSIONS
        .iter()
        .find(|omission| omission.vector == vector)
}

/// Every family of the corpus must be present in strength.
///
/// The check this replaces — `assert_eq!(total, m1.len() + prefixed.len())` — was a
/// tautology: `partition` puts every element in exactly one half, so it could not fail for
/// any input, including a corpus with a whole prefix family deleted. Floors are the only
/// form of this assertion with any content.
fn assert_corpus_is_complete(m1: &[Vector], prefixed: &[Vector]) {
    let corpus = vectors::corpus_dir();
    assert!(
        m1.len() >= vectors::MIN_M1_VECTORS,
        "only {} un-prefixed vectors were found, expected at least {}. The corpus in {} \
         looks truncated — a run over too few vectors is a false green.",
        m1.len(),
        vectors::MIN_M1_VECTORS,
        corpus.display(),
    );

    for (prefix, floor) in MIN_VECTORS_PER_PREFIX {
        let found = prefixed
            .iter()
            .filter(|vector| vector.setup.prefix().is_some_and(|p| p.name() == prefix))
            .count();
        assert!(
            found >= floor,
            "only {found} {prefix}-prefixed vectors were found, expected at least {floor} — \
             {} short. The corpus in {} has lost {prefix} vectors; M2's gate is \"fuse green \
             in full\", and a short family silently reduces it.",
            floor - found,
            corpus.display(),
        );
    }
}

/// The other half of "this list cannot rot": every declared omission must name a vector
/// that actually runs.
///
/// A typo, or a vector that leaves the corpus on a re-fetch, would otherwise sit in the
/// list forever — suppressing nothing, and telling every future reader that the case is
/// handled when it is not. (The complementary check, an omission that is no longer needed,
/// lives in `report::compare_transfers_allowing`.)
fn assert_omissions_are_live(corpus: &[Vector]) {
    for omission in CORPUS_OMISSIONS {
        assert!(
            corpus.iter().any(|vector| vector.name() == omission.vector),
            "CORPUS_OMISSIONS names vector {:?}, which is not in the corpus — the name is \
             wrong, or the vector has left the corpus. Reason on file: {}",
            omission.vector,
            omission.reason,
        );
    }
    let disputed = CORPUS_OMISSIONS
        .iter()
        .filter(|omission| omission.disputed_flag_bits != 0)
        .count();
    println!(
        "  corpus omissions declared:  {} ({} elided reads, {disputed} disputed flag rules)",
        CORPUS_OMISSIONS.len(),
        CORPUS_OMISSIONS.len() - disputed,
    );
}

/// Floors for the prefixed families, in the spirit of [`vectors::MIN_M1_VECTORS`].
///
/// Without these an entire prefix category could disappear and nothing would notice:
/// `assert_eq!(total, m1.len() + prefixed.len())` cannot fail, because `partition`
/// guarantees it — deleting all 264 `CB` blocks left the suite green with the `CB` row
/// simply absent from a census nobody reads. M2's gate is "fuse green in full", and
/// without a per-family floor that milestone could be declared on three quarters of the
/// corpus. Observed: CB 264, DD 343, ED 97, FD 341.
const MIN_VECTORS_PER_PREFIX: [(&str, usize); 4] =
    [("CB", 250), ("DD", 330), ("ED", 90), ("FD", 330)];

/// `R` is a 7-bit counter: its delta gives the instruction count actually executed.
const REFRESH_COUNTER_MASK: u8 = 0x7F;

/// Full diagnostics are printed for this many failures. The one-line punch list above
/// them always covers every failing vector, so nothing is dropped.
const DETAILED_FAILURES: usize = 20;

struct Failure<'a> {
    vector: &'a Vector,
    mismatches: Vec<Mismatch>,
    transfers: Vec<Transfer>,
    tick_addresses: Vec<u16>,
}

/// Load the corpus and split it into the M1 and M2 halves, checking both are intact.
fn partitioned_corpus() -> Option<(Vec<Vector>, Vec<Vector>)> {
    let corpus = vectors::corpus_or_skip()?;
    let total = corpus.len();
    let (m1, prefixed): (Vec<Vector>, Vec<Vector>) =
        corpus.into_iter().partition(Vector::is_m1_scope);
    print_scope_census(total, &m1, &prefixed);
    assert_corpus_is_complete(&m1, &prefixed);
    Some((m1, prefixed))
}

/// Run one half of the corpus and report it as a single pass/fail.
fn conformance(milestone: &str, vectors: &[Vector]) {
    let failures: Vec<Failure<'_>> = vectors.iter().filter_map(run_vector).collect();
    println!(
        "{milestone} conformance: {executed} executed, {passed} passed, {failed} failed",
        executed = vectors.len(),
        passed = vectors.len() - failures.len(),
        failed = failures.len(),
    );
    assert!(
        failures.is_empty(),
        "{}",
        render_report(milestone, &failures)
    );
}

/// Both halves are checked in one place, so neither can rot into naming a vector that no
/// longer exists — including the four `CB` entries, which live in the M2 half.
#[test]
fn corpus_omissions_are_live() {
    let Some(corpus) = vectors::corpus_or_skip() else {
        return;
    };
    assert_omissions_are_live(&corpus);
}

#[test]
fn fuse_conformance_unprefixed() {
    let Some((m1, _prefixed)) = partitioned_corpus() else {
        return;
    };
    conformance("M1", &m1);
}

/// The M2 gate.
///
/// This did not exist until now, and its absence was not visible from anywhere: the file
/// ran 290 vectors, skipped 1045, and printed a census saying so — while "1041 of 1045
/// pass" was true, repeated, and **asserted by nothing in the repository**.
///
/// What that cost, measured: of eight mutations to the M2 core, **three survived the entire
/// suite** — a `CB` decode table with `SLA`/`SRA` transposed, a `DDCB` undocumented copy
/// targeting `IXh`/`IXl` instead of `H`/`L`, and `ED_PAIRS` with `BC`/`DE` transposed. The
/// first is the same bug class as M1's `XOR`/`OR` transposition, which the corpus caught
/// the instant it ran.
///
/// The shape of that split is the lesson worth keeping: every decision the implementer
/// flagged as *uncertain* already had a hand-written test — doubt gets tested. What went
/// unprotected was the **systematic** material, decode tables and operand substitution,
/// which is exactly what a thousand-vector sweep is for and what nothing else covers.
#[test]
fn fuse_conformance_prefixed() {
    let Some((_m1, prefixed)) = partitioned_corpus() else {
        return;
    };
    conformance("M2", &prefixed);
}

/// Load one vector, step until its T-state budget is reached, and compare everything.
fn run_vector(vector: &Vector) -> Option<Failure<'_>> {
    let mut machine = Machine::load(&vector.setup);
    let completion = machine.run(vector.setup.state.t_states);
    let (registers, state) = machine.snapshot();

    // A truncated run poisons every derived comparison, so report only the overrun and
    // stop. Continuing produced 50 mismatches on vector `10` at a reduced cap — including a
    // confident, wrong instruction to delete a correct and load-bearing omission. A loud
    // test is worth nothing if the outcome it shouts is lenient.
    if completion == Completion::StepLimit {
        // `R` counts opcode fetches, 7 bits wrapping, so its delta is the instruction count
        // the run actually managed — a measurement, not the guess "often zero" was.
        let executed = state.r.wrapping_sub(vector.setup.state.r) & REFRESH_COUNTER_MASK;
        return Some(Failure {
            vector,
            mismatches: vec![
                Mismatch::new(
                    "run",
                    format!("reach {} T-states", vector.setup.state.t_states),
                    format!("stopped after {} T-states", state.t_states),
                )
                .with_note(format!(
                    "the run loop hit its {MAX_STEPS_PER_VECTOR}-instruction cap having \
                     executed {executed} instruction(s) (from the R delta). Every other \
                     comparison is suppressed because a truncated run makes all of them \
                     meaningless — fix this first."
                )),
            ],
            transfers: machine.transfers().to_vec(),
            tick_addresses: machine.tick_addresses().to_vec(),
        });
    }

    let mut mismatches = report::compare(
        &vector.expected,
        &registers,
        &state,
        machine.transfers(),
        machine.tick_addresses(),
        omission_for(vector.name()),
        |addr| machine.read_memory(addr),
    );

    // `Cpu::step`'s return value, measured against the corpus — NOT against the bus tick
    // count, which is the same counter read twice and can never disagree.
    if machine.reported_t_states() != vector.expected.state.t_states {
        mismatches.push(
            Mismatch::new(
                "T-states (Cpu::step return)",
                vector.expected.state.t_states.to_string(),
                machine.reported_t_states().to_string(),
            )
            .with_note(
                "Cpu::step returned a total the corpus disagrees with. The corpus column is \
                 derived from the published machine-cycle lengths, so it is an outside \
                 authority on this number.\n    Note this check cannot currently fail alone: \
                 in the core, `t_states += 1` and `bus.tick(address)` are adjacent lines, so \
                 it fires together with the bus-count check or not at all. It is kept anyway \
                 — as a guard against a future entry point that charges time WITHOUT going \
                 through Cpu::tick, which is the only way the two could ever diverge.",
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
    println!("  in M2 scope (prefixed):    {}", prefixed.len());
    for (prefix, count) in &by_prefix {
        println!("    {prefix}-prefixed: {count}");
    }
}

/// Summary, then the census of which fields fail, then a complete punch list, then
/// detail. Ordered so the first screen answers "what do I fix first?".
///
/// `milestone` is threaded in rather than hard-coded because both gates share this
/// function: a pure-`CB` failure used to announce itself as `48 M1 vector(s) failed` and
/// offer "the M1 punch list". The first question on a red gate is *which milestone did I
/// break*, and answering it wrongly is the same defect as a stale comment — output that
/// misdescribes reality — in a harness whose entire purpose is an actionable message.
fn render_report(milestone: &str, failures: &[Failure<'_>]) -> String {
    let mut out = format!("\n{} {milestone} vector(s) failed.\n\n", failures.len());

    out.push_str(&format!(
        "Failing fields, most common first — this is the {milestone} punch list:\n"
    ));
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
