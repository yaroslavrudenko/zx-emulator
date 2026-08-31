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
