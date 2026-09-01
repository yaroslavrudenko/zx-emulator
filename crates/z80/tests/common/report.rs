//! Turning a failed vector into a sentence a human can act on.
//!
//! The bar this module exists to meet: a failure must name the opcode, the exact field
//! that diverged, and — when the field is `F` — the individual flag bits, including the
//! undocumented 3 and 5. "assertion failed: left == right" on a 16-bit register tells you
//! nothing about which of the eight flags moved.

use std::fmt::Write as _;

use super::flags;
use super::vectors::{EventKind, Expectation, MemoryBlock, Registers, State, Transfer, Vector};

/// One diverging field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    pub field: String,
    pub expected: String,
    pub actual: String,
    /// Extra explanation, such as which individual flag bits differ.
    pub note: Option<String>,
}

impl Mismatch {
    pub fn new(
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
            note: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

// ---------------------------------------------------------------------------
// State comparisons
// ---------------------------------------------------------------------------

/// Compare every register the vector specifies.
///
/// `AF` and `AF'` are decomposed into accumulator plus named flag bits, because a bare
/// `5601 != 5600` hides the fact that only the undocumented bit 3 moved.
pub fn compare_registers(
    expected: &Registers,
    actual: &Registers,
    disputed_flag_bits: u8,
) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();
    for ((name, want), (_, got)) in expected.named().into_iter().zip(actual.named()) {
        // Disputed bits are excused for `AF` only, and only the named bits: the
        // accumulator and every other flag still have to match exactly.
        let excused = if name == "AF" {
            u16::from(disputed_flag_bits)
        } else {
            0
        };
        if want & !excused == got & !excused {
            continue;
        }
        let mismatch = Mismatch::new(name, format!("{want:04x}"), format!("{got:04x}"));
        mismatches.push(if name.starts_with("AF") {
            let (want_f, got_f) = (want as u8, got as u8);
            mismatch.with_note(format!(
                "A: {:02x} -> {:02x}; F: {:02x} -> {:02x}\n    expected flags {}\n    actual   flags {}\n    differing flags: {}",
                want >> 8,
                got >> 8,
                want_f,
                got_f,
                flags::describe(want_f),
                flags::describe(got_f),
                describe_flag_differences(want_f, got_f),
            ))
        } else {
            mismatch
        });
    }
    mismatches
}

fn describe_flag_differences(expected: u8, actual: u8) -> String {
    let differing = flags::differences(expected, actual);
    if differing.is_empty() {
        String::from("none (only the accumulator differs)")
    } else {
        differing.join(", ")
    }
}

/// Compare `I`, `R`, `IFF1`, `IFF2`, `IM` and the halt flag.
pub fn compare_state(expected: &State, actual: &State) -> Vec<Mismatch> {
    expected
        .named()
        .into_iter()
        .zip(actual.named())
        .filter(|((_, want), (_, got))| want != got)
        .map(|((name, want), (_, got))| Mismatch::new(name, want.to_string(), got.to_string()))
        .collect()
}

/// Compare the T-state total.
pub fn compare_t_states(expected: u32, actual: u32) -> Option<Mismatch> {
    (expected != actual).then(|| {
        Mismatch::new("T-states", expected.to_string(), actual.to_string()).with_note(format!(
            "off by {}{}",
            if actual > expected { "+" } else { "-" },
            actual.abs_diff(expected)
        ))
    })
}

/// Compare every memory location the expectation lists, one finding per address.
pub fn compare_memory(expected: &[MemoryBlock], read: impl Fn(u16) -> u8) -> Vec<Mismatch> {
    expected
        .iter()
        .flat_map(MemoryBlock::addresses)
        .filter_map(|(addr, want)| {
            let got = read(addr);
            (want != got).then(|| {
                Mismatch::new(
                    format!("memory[{addr:04x}]"),
                    format!("{want:02x}"),
                    format!("{got:02x}"),
                )
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Bus-trace comparisons
// ---------------------------------------------------------------------------

/// Compare the address on the bus at every T-state the corpus pins down.
///
/// This is the check the per-T-state `Bus::tick(addr)` signature exists to make possible.
/// A core that batches a run of internal cycles into one call cannot satisfy it: the
/// corpus asks what was on the bus at T=4, T=5, T=6 and so on individually, and a batched
/// tick has no answer for any of them.
///
/// Only the T-states the corpus actually names are asserted. An `MC` at the start of a
/// four-T-state fetch says nothing about that fetch's remaining three T-states, and this
/// comparison invents no claim the corpus does not make.
///
/// The gap is now counted rather than estimated. Over `testdata/fuse/tests.expected`,
/// 2026-09-01: **20,383 T-states across 1335 vectors, 7637 of them pinned** by an `MC` or
/// `PC` event — so **62.5% go unpinned**, and *"roughly two thirds"* below is right.
///
/// # Do not "close" that gap by asserting the address holds constant across a cycle
///
/// It is tempting, since roughly two thirds of T-states go unpinned, to add an assertion
/// that the address stays put between consecutive transfers. It would pass today. **It
/// must not be added**, and the reason is not caution but correctness:
///
/// A real Z80 drives `PC` for T1–T2 of an opcode fetch and the **refresh address for
/// T3–T4** — the address bus changes mid-cycle. This core currently drives `PC` for all
/// four. The corpus does not adjudicate that, because it contends only at the cycle start.
/// So such an assertion would not be testing the Z80; it would be freezing our present
/// simplification into the gate, and would fire as a *false failure* the day somebody makes
/// M1 hardware-accurate.
///
/// The rule this is an instance of: **the gate asserts the chip, never our convenience.**
/// The same principle, applied in the other direction, is why `CORPUS_OMISSIONS` exists —
/// there the corpus was wrong about the chip, and the harness bent rather than the core.
///
/// # Three corrections to the section above, 2026-09-01
///
/// **The rule stands, this function stays as it is, and the paragraph enforcing it is right
/// about the chip.** What was wrong was its *status* and its *consequence*: the hardware
/// sentence was stated as a fact with nothing behind it, the assertion it forbids had already
/// been written in fourteen other places, and the failure it predicts is not the kind it says.
/// The last of those is the one that changes what should be done.
///
/// ## 1. *"A real Z80 drives …"* was asserted here as a fact with no source. It now has one,
/// and it is **proven**
///
/// It was sourced nowhere: not in `docs/Z80-REFERENCE.md`, where hardware rules live and
/// which said only that an M1 cycle has *"the address driven from `PC`"* without resolving it
/// per T-state; not in `docs/ARCHITECTURE.md`, which repeated it in the same unlabelled form;
/// not here. The project's own vocabulary — *proven / measured / derived / observed* — had no
/// row for it, which is the tell. **A gate whose justification is an unlabelled claim is one
/// rung below the thing it is guarding against**, and this guard was written in the belief
/// that it was the other way round.
///
/// The claim is now settled, from the vendor. *Z80 CPU User Manual*, Zilog, **UM008011-0816**,
/// `https://www.zilog.com/docs/z80/um0080.pdf`, fetched 2026-09-01, SHA-256
/// `e3c83da5a5d8e372364c20fa53665e6fbb165ec6ac38c8c1eebc359603447b5e`. Section *Instruction
/// Fetch*, verbatim:
///
/// > The Program Counter is placed on the address bus at the beginning of the M1 cycle. …
/// > Clock states T3 and T4 of a fetch cycle are used to refresh dynamic memories. … **During
/// > T3 and T4, the lower seven bits of the address bus contain a memory refresh address** and
/// > the RFSH signal becomes active … To prevent data from different memory segments from
/// > being gated onto the data bus, an RD signal is not generated during this refresh period.
/// > … the refresh address is only guaranteed to be stable during the MREQ period.
///
/// Figure 5, *Instruction Op Code Fetch*, draws the `A15–A0` row as two fields with the
/// boundary between T2 and T3: **`PC`** then **`Refresh Address`**. And, separately, under
/// *Memory Refresh (R) Register*: *"During refresh, the contents of the I Register are placed
/// on the upper eight bits of the address bus."*
///
/// So the sentence in the section above is **right**, and the manual refines it in two ways
/// worth carrying:
///
/// - **`A8–A15` = `I` is proven, but by a different sentence from the one about T3–T4.** The
///   timing text guarantees only the *lower seven bits*; the upper eight come from the `R`
///   register's own description. `A7` — `R`'s eighth bit, the latch `LD R,A` writes — is
///   **derived**: the manual says the counter's data *"is sent out on the lower portion of the
///   address bus"* and guarantees seven of it. `{I, R}` as a whole 16-bit value is therefore
///   *proven for 15 bits and derived for one*, and `Registers::refresh_address` composes all
///   sixteen.
/// - **It is not claimed to be stable for the whole of T3–T4** — only *"during the MREQ
///   period"*. A model that pins an address to every T-state of the refresh half would be
///   claiming more than Zilog does.
///
/// ## 2. The assertion this forbids is already in the tree, in **fourteen** places
///
/// *"It must not be added"* was written as though it had not been. It had. Not in this
/// function — `compare_contention` is clean and stays clean — but:
///
/// - `crates/z80/tests/bus_timing.rs` pairs `OPCODE_FETCH` with `PROGRAM_START` in
///   **thirteen** address-stream expectations, each pinning all four M1 T-states to `PC`;
/// - `crates/z80/src/lib.rs`'s `every_t_state_reports_the_address_the_z80_drives` pins the
///   whole stream for seven instructions, M1 interiors included.
///
/// That count is not a grep. It is the set of tests that went red when `Cpu::fetch_opcode`
/// was mutated to drive `PC, PC, IR, IR` (below), which is the only way to enumerate it
/// without missing one.
///
/// The second name is the sharper problem, and with UM0080 in hand it is no longer merely
/// unsourced — it is **contradicted**. A test called *"every T-state reports the address the
/// Z80 drives"* reports, for two T-states in every four, the address **this core** drives,
/// which Figure 5 says is not the one the Z80 drives. Its expectation for those T-states came
/// from the implementation it is grading. `docs/STATUS.md` catalogues that exact shape:
/// *a test whose expectation is computed by the subject is not a weak test; it is a tautology
/// with a cross product attached*. Here the cross product is seven instructions and the
/// tautology is two T-states wide.
///
/// The test is still worth having — it is the only thing in the workspace that would notice
/// the M1 interior changing at all, which is precisely what the mutation below demonstrates.
/// What it needs is a name and a comment that claim what it checks. Neither file is mine to
/// change; both are reported.
///
/// ## 3. It would not fire as a *false* failure — and the failure would cost nothing
///
/// This is the correction that matters, because it is the one that changes what should be
/// done about the other two. The claim was that such an assertion *"would fire as a false
/// failure the day somebody makes M1 hardware-accurate"*. It would fire, and the failure
/// would be **true**: the expectations would be stale and would need updating, which is what
/// a change detector is for. Calling a true failure false is what turns a gate into a
/// nuisance and gets it deleted.
///
/// **And nothing else would move. Measured, 2026-09-01**, in a scratch clone of `0d3e7ef`,
/// never in the shared tree: `Cpu::fetch_opcode` mutated to drive `PC, PC, IR, IR`, the
/// mutation confirmed present by `git diff` before any verdict was read, then
/// `cargo test -p z80 -p spectrum --no-fail-fast` — **425 passed / 0 failed** before,
/// **410 passed / 15 failed** after, and reverting restored 425/0 exactly.
///
/// The fifteen are the fourteen above plus `codegen.rs`'s
/// `bounds_checks_in_the_execute_path_have_not_moved` (7 → 8), which is an artefact of the
/// naive mutation's extra `refresh_address()` call and says nothing about M1. Everything
/// that grades behaviour stayed green:
///
/// - `fuse_conformance_unprefixed` (290 vectors) and `fuse_conformance_prefixed` (1045),
///   both **unchanged** — so the corpus genuinely cannot see it, over every vector rather
///   than over the three that were read by hand;
/// - `crates/spectrum/tests/timing_oracle.rs`, all **68 hardware rows unchanged**, in the
///   shipped configuration *and* under an `INTERRUPT_T_STATES = 33` probe where its three
///   residual disagreements came back byte-identical with the mutation applied.
///
/// **Why the corpus cannot see it, stated exactly.** `tests.expected` carries 1335 vectors,
/// **1335 `MC` events at T=0 and zero at T=1, T=2 or T=3** — the interior of the M1 fetch
/// every vector opens with. The two vectors with a non-zero `I` show what the corpus *does*
/// resolve: `ed57` (`I`=`0x1e`, `R`=`0x17`) records `0 MC 0000`, `4 MC 0001`, `8 MC 1e19`.
/// The `MC` opening the **second** M1 is at `0001` — `PC`, not `IR`, which would have been
/// `1e18` — so the corpus does pin the *first* T-state of a fetch to `PC` and is decisive
/// about it. The internal cycle at T=8 is at `1e19`, unmistakably `{I, R}`. Between them,
/// the fetch's own T2–T4: never named, in any vector.
///
/// **Why the machine cannot see it either.** `Ula::tick` consults its `address` argument
/// only when no machine cycle is open; a fetch opens one four T-states long, so the address
/// supplied on T2–T4 of an M1 is discarded before it is read. `Ula` is the only `Bus`
/// implementation outside tests and benches. Contention is priced **once per machine cycle,
/// at the address the cycle opens on** — so M1's second half is never separately contended
/// on *any* address, whatever is driven there.
///
/// ## What this leaves, and it is a better answer than "one of the two files is wrong"
///
/// The section above is **right about the hardware and wrong about the consequence**. The
/// address bus almost certainly does change mid-M1; asserting that it does not is still
/// wrong in principle and still must not go into this function; and it is nonetheless
/// **unobservable in this emulator**, so the fourteen assertions elsewhere are sound as
/// change detectors even though their names claim more than they check.
///
/// The right disposition follows from that and is deliberately not a code change: rename
/// or re-document the two offenders so they claim what they check, and leave the core alone
/// until something can grade it. A "fix" to `fetch_opcode` today would be an unverifiable
/// guess whose gate asserts a number nothing produces — the same reasoning, and the same
/// wording, that `crates/spectrum/src/ula.rs` used to leave the interrupt-acknowledge shape
/// standing until an oracle arrived for it.
///
/// ## And the divergence cannot cost a T-state on the hardware either — same manual
///
/// This is stronger than *"unobservable in this emulator"*, it is not an argument from our
/// contention model, and it is the reason the disposition above is safe rather than merely
/// convenient. A Spectrum charges contention by holding the Z80's `/WAIT` line, and UM0080's
/// *T Cycle* section says exactly when the CPU looks at it:
///
/// > During T2 and every subsequent automatic WAIT state (TW), the CPU samples the WAIT line
/// > with the falling edge of the clock. If the WAIT line is active at this time, another
/// > WAIT state is entered during the following cycle.
///
/// **T2 and TW. Not T3, not T4.** Wait states are inserted between T2 and T3, which is
/// before the refresh address reaches the bus at all. So the address driven during T3–T4 of
/// an M1 **cannot** lengthen that M1, whatever it is and whoever is snooping it: by the time
/// the ULA could react to it the CPU has stopped asking. Contention charged once per machine
/// cycle at the address the cycle opens on is not this emulator's simplification of the
/// hardware — for M1 it is the only thing the hardware can do.
///
/// Which closes the question that started this: **`PC`-for-all-four and `PC,PC,IR,IR` are not
/// two behaviours, they are one behaviour in two coordinate systems.** No `Bus` that models a
/// real Z80 machine can separate them, because the pin that would have to carry the
/// difference is not being read. Measuring 68 hardware rows and 1335 vectors unchanged was
/// the empirical half of that; this is why the empirical half had to come out that way.
///
/// The residue — what a refresh address in contended RAM *does* do on real hardware — is not
/// a timing effect at all. `I` in `0x40..=0x7F` points the refresh strobe at the screen bank
/// and produces visible "snow", which is the ULA's *display* fetch being disturbed, not the
/// CPU being stalled. That is outside anything this core models, and it is the one thing left
/// that a divergence here could ever be observed by. `I` is `0x3F` under the 48K ROM — 399,992
/// of 400,000 sampled instructions, measured 2026-09-01 — and an `IM 2` table is
/// conventionally placed high, so the configuration is rare as well as inert.
pub fn compare_contention(expected: &Expectation, tick_addresses: &[u16]) -> Vec<Mismatch> {
    expected
        .contention_points()
        .filter_map(|(t_state, want)| {
            let field = format!("bus address at T{t_state}");
            match tick_addresses.get(t_state as usize) {
                Some(&got) if got == want => None,
                Some(&got) => Some(Mismatch::new(
                    field,
                    format!("{want:04x}"),
                    format!("{got:04x}"),
                )),
                None => Some(
                    Mismatch::new(field, format!("{want:04x}"), "no tick at that T-state")
                        .with_note(format!(
                            "the core reported {} T-state tick(s) in total; Bus::tick must be \
                             called once per T-state and never batched",
                            tick_addresses.len()
                        )),
                ),
            }
        })
        .collect()
}

/// A vector where the corpus omits a memory read the hardware genuinely performs.
///
/// The corpus is an oracle, not scripture. Where it and the Z80 disagree, the core follows
/// the Z80 and the *harness* carries the difference — bending the core to match another
/// emulator's bookkeeping would be fixing the wrong artefact.
///
/// An omission is a scalpel, never a blanket: it permits exactly the listed reads, at the
/// listed addresses, and everything the corpus *does* record must still match in order and
/// in content. It also cannot rot — see [`compare_transfers_allowing`], which fails when a
/// listed omission turns out to be unnecessary.
#[derive(Debug, Clone, Copy)]
pub struct CorpusOmission {
    /// The vector's name in the corpus.
    pub vector: &'static str,
    /// Addresses the corpus contends but records no read for, though the CPU reads them.
    ///
    /// Empty when the disagreement is not about transfers at all — see
    /// [`Self::disputed_flag_bits`].
    pub elided_reads: &'static [u16],
    /// Bits of `F` on which the corpus and the core follow **different rules**.
    ///
    /// A different category from [`Self::elided_reads`], and worth keeping distinct: an
    /// elided read is the corpus declining to *record* something both agree happened, while
    /// a disputed flag bit is a genuine disagreement about what the hardware does. Only the
    /// named bits are excused; the accumulator and every other flag must still match
    /// exactly, and the bits must actually differ or the entry is dead.
    pub disputed_flag_bits: u8,
    /// Why the core and the corpus differ, in enough detail to re-litigate later.
    pub reason: &'static str,
}

/// Compare the bytes moved, in order, allowing exactly the reads an omission declares.
///
/// Everything the corpus records must still appear, in order, byte for byte — the
/// omission only permits *additional* reads, and only at the declared addresses.
pub fn compare_transfers_allowing(
    expected: &[Transfer],
    actual: &[Transfer],
    omission: &CorpusOmission,
) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();

    // Greedy subsequence match: every recorded transfer must still appear, in order.
    // Whatever the corpus did not account for falls out as an extra.
    let mut extras: Vec<Transfer> = Vec::new();
    let mut wanted = expected.iter().peekable();
    for got in actual {
        if wanted.peek().is_some_and(|want| *want == got) {
            wanted.next();
        } else {
            extras.push(*got);
        }
    }
    for missing in wanted {
        mismatches.push(
            Mismatch::new("transfer", missing.to_string(), "never happened").with_note(
                "an omission permits EXTRA reads; it never excuses a transfer the corpus \
                 recorded from going missing",
            ),
        );
    }

    // The extras must be exactly the declared elided reads — no more, no fewer.
    //
    // Compared as (kind, address) over EVERY extra, not just the reads. Filtering to
    // `MemoryRead` first was a hole: a spurious `MW`, `PW` or `PR` on any of the 21
    // omission vectors was dropped before the comparison and vanished — two invented stack
    // writes compared equal to nothing. It also split the two guards apart, because the
    // "DELETE it" check keyed on all extras while the comparison keyed on reads only, so a
    // single non-read extra suppressed the rot-guard and misattributed the failure to the
    // declaration. One key for both, and that second bug cannot recur.
    let mut found: Vec<(EventKind, u16)> = extras
        .iter()
        .map(|transfer| (transfer.kind, transfer.addr))
        .collect();
    found.sort_unstable();
    let mut declared: Vec<(EventKind, u16)> = omission
        .elided_reads
        .iter()
        .map(|addr| (EventKind::MemoryRead, *addr))
        .collect();
    declared.sort_unstable();

    if extras.is_empty() {
        mismatches.push(
            Mismatch::new(
                "corpus omission",
                format!("vector {:?} to need it", omission.vector),
                "the core matched the corpus exactly",
            )
            .with_note(
                "this omission is no longer necessary — DELETE it from CORPUS_OMISSIONS. A \
                 suppression nobody needs is a suppression nobody notices, and it will hide \
                 the next real divergence at this opcode",
            ),
        );
    } else if found != declared {
        mismatches.push(
            Mismatch::new(
                "corpus omission",
                render_accesses(&declared),
                render_accesses(&found),
            )
            .with_note(format!(
                "the omission for {:?} declares exactly which accesses the corpus elides, \
                 and they must all be reads; the core made a different set. Reason on \
                 file: {}",
                omission.vector, omission.reason,
            )),
        );
    }

    mismatches
}

/// The rot-guard for [`CorpusOmission::disputed_flag_bits`].
///
/// **Every named bit must actually differ — not merely one of them.** The weaker reading
/// fired only when the whole declaration was dead, so declaring both undocumented bits when
/// only one diverges was invisible: the other was excused for nothing, permanently, and the
/// guard reported success. That is the identical "a suppression nobody needs is a
/// suppression nobody notices" case the guard exists to prevent, one level down — per-bit
/// rather than per-vector.
///
/// Measured on the four `BIT n,(HL)` entries when they all declared both bits: `cb4e` and
/// `cb76` diverge on bit 3 alone, `cb6e` on bit 5 alone. Three of four were over-declared
/// and nothing said so.
pub fn compare_disputed_flags(
    omission: &CorpusOmission,
    expected_f: u8,
    actual_f: u8,
) -> Vec<Mismatch> {
    let disputed = omission.disputed_flag_bits;
    let differing = (expected_f ^ actual_f) & disputed;
    if disputed == 0 || differing == disputed {
        return Vec::new();
    }

    let superfluous = disputed & !differing;
    vec![
        Mismatch::new(
            "disputed flag bits",
            format!(
                "every declared bit to differ: {}",
                flags::describe(disputed)
            ),
            format!(
                "these agree and are excused for nothing: {}",
                render_mask(superfluous)
            ),
        )
        .with_note(format!(
            "NARROW the declaration for {:?} to the bits that genuinely diverge, or DELETE it \
             if none do. An excused bit that already agrees suppresses nothing today and \
             hides the next real divergence at that opcode. Reason on file: {}",
            omission.vector, omission.reason,
        )),
    ]
}

/// Name the set bits of a mask: `Y(bit5), X(bit3)`.
fn render_mask(mask: u8) -> String {
    let named: Vec<&str> = flags::BITS
        .iter()
        .filter(|(_, bit)| mask & bit != 0)
        .map(|(name, _)| *name)
        .collect();
    if named.is_empty() {
        String::from("(none)")
    } else {
        named.join(", ")
    }
}

/// Compare the bytes moved, in order, ignoring timestamps.
pub fn compare_transfers(expected: &[Transfer], actual: &[Transfer]) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();
    for (index, want) in expected.iter().enumerate() {
        match actual.get(index) {
            Some(got) if got == want => {}
            Some(got) => mismatches.push(Mismatch::new(
                format!("transfer #{index}"),
                want.to_string(),
                got.to_string(),
            )),
            None => mismatches.push(Mismatch::new(
                format!("transfer #{index}"),
                want.to_string(),
                "no transfer at all",
            )),
        }
    }
    for (index, got) in actual.iter().enumerate().skip(expected.len()) {
        mismatches.push(Mismatch::new(
            format!("transfer #{index}"),
            "no transfer",
            got.to_string(),
        ));
    }
    mismatches
}

/// Every check for one vector, in the order a reader wants them.
pub fn compare(
    expected: &Expectation,
    actual_registers: &Registers,
    actual_state: &State,
    actual_transfers: &[Transfer],
    tick_addresses: &[u16],
    omission: Option<&CorpusOmission>,
    read: impl Fn(u16) -> u8,
) -> Vec<Mismatch> {
    let disputed = omission.map_or(0, |omission| omission.disputed_flag_bits);
    let mut mismatches = compare_registers(&expected.registers, actual_registers, disputed);
    if let Some(omission) = omission {
        mismatches.extend(compare_disputed_flags(
            omission,
            expected.registers.f(),
            actual_registers.f(),
        ));
    }
    mismatches.extend(compare_state(&expected.state, actual_state));
    mismatches.extend(compare_t_states(
        expected.state.t_states,
        actual_state.t_states,
    ));
    mismatches.extend(compare_memory(&expected.memory, read));
    // The relaxing comparator is used only when there is something to relax. An omission
    // that declares no elided reads gets the strict comparison, so its "DELETE it" guard
    // cannot misfire on a vector whose disagreement was never about transfers.
    mismatches.extend(match omission {
        Some(omission) if !omission.elided_reads.is_empty() => {
            compare_transfers_allowing(&expected.transfers(), actual_transfers, omission)
        }
        _ => compare_transfers(&expected.transfers(), actual_transfers),
    });
    // The contention half is never relaxed. An omission concerns whether MREQ is asserted
    // during a cycle; it says nothing about which address is on the bus, and the corpus is
    // right about that even where it declines to record the read.
    mismatches.extend(compare_contention(expected, tick_addresses));
    mismatches
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render the per-T-state address log run-length encoded: `T0-3 0000, T4-10 0001`.
///
/// Run-length is the readable form for this data *and* the diagnostic one: a core that
/// batches internal cycles shows up as a run that is too short, right next to the run the
/// corpus expected.
pub fn render_tick_addresses(tick_addresses: &[u16]) -> String {
    if tick_addresses.is_empty() {
        return String::from("(no ticks)");
    }
    let mut out = String::new();
    let mut start = 0usize;
    for index in 1..=tick_addresses.len() {
        let ended = index == tick_addresses.len() || tick_addresses[index] != tick_addresses[start];
        if !ended {
            continue;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        let last = index - 1;
        if last == start {
            let _ = write!(out, "T{start} {:04x}", tick_addresses[start]);
        } else {
            let _ = write!(out, "T{start}-{last} {:04x}", tick_addresses[start]);
        }
        start = index;
    }
    out
}

/// Render the corpus's contention points in the same shape, for side-by-side reading.
pub fn render_contention_points(expected: &Expectation) -> String {
    let points: Vec<String> = expected
        .contention_points()
        .map(|(t_state, addr)| format!("T{t_state} {addr:04x}"))
        .collect();
    if points.is_empty() {
        String::from("(none)")
    } else {
        points.join(", ")
    }
}

/// Render `(kind, address)` pairs as `MR 0001, MW 7fff`.
fn render_accesses(accesses: &[(EventKind, u16)]) -> String {
    if accesses.is_empty() {
        return String::from("(none)");
    }
    accesses
        .iter()
        .map(|(kind, addr)| format!("{} {addr:04x}", kind.token()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a transfer list.
pub fn render_transfers(transfers: &[Transfer]) -> String {
    if transfers.is_empty() {
        return String::from("(none)");
    }
    transfers
        .iter()
        .map(Transfer::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The full failure report for one vector.
pub fn render_failure(
    vector: &Vector,
    mismatches: &[Mismatch],
    actual_transfers: &[Transfer],
    tick_addresses: &[u16],
) -> String {
    let opcode = vector.setup.opcode_at_pc();
    let mut out = format!(
        "vector {name:?} (opcode {opcode:02x} at PC {pc:04x}) — {count} mismatch(es)\n",
        name = vector.name(),
        pc = vector.setup.registers.pc,
        count = mismatches.len(),
    );
    for mismatch in mismatches {
        let _ = writeln!(
            out,
            "  {field}: expected {expected}, got {actual}",
            field = mismatch.field,
            expected = mismatch.expected,
            actual = mismatch.actual,
        );
        if let Some(note) = &mismatch.note {
            let _ = writeln!(out, "    {note}");
        }
    }
    let _ = writeln!(
        out,
        "  expected transfers: {}",
        render_transfers(&vector.expected.transfers())
    );
    let _ = writeln!(
        out,
        "  actual   transfers: {}",
        render_transfers(actual_transfers)
    );
    let _ = writeln!(
        out,
        "  corpus contention points: {}",
        render_contention_points(&vector.expected)
    );
    let _ = writeln!(
        out,
        "  actual bus address per T-state: {}",
        render_tick_addresses(tick_addresses)
    );
    out
}
