# Status

A living record of where the project actually is — what is proven, what is measured, what is
open. Updated as work lands, not once at the start.

**Last updated:** 2026-08-31, during M2.

---

## Milestone M3 — `zexdoc`

In progress. This is the first oracle that grades the **processor**, not an instruction.

The difference is structural, not one of degree. FUSE vectors set up a state, run one instruction
and compare — so a defect is localised for you, and the failure names the opcode. `zexdoc` is a
CP/M `.COM` binary that runs on the order of a hundred million instructions, computes a CRC over
the results and prints `OK` or `ERROR` per test group. It catches what per-instruction vectors
structurally cannot: an error that only appears in a *sequence*, which is exactly the shape of the
`SCF`/`CCF` Q latch this project has been deferring since M1.

It needs its own scaffolding rather than an extension of the vector harness — 64 KB of RAM, the
program at `0x0100`, two BDOS calls trapped at `0x0005` for console output, and warm boot at
`0x0000` as the termination signal.

`zexall` is M4 and stays out of M3 deliberately: it grades the undocumented flags, the Q latch is
knowingly unimplemented, and running it now would produce a wall of expected failures that teaches
nothing.

## Milestone M2 — the four prefixes

| Prefix | Vectors | State |
|---|---|---|
| `DD` | 343 / 343 | ✅ |
| `FD` | 341 / 341 | ✅ |
| `ED` | 97 / 97 | ✅ all eight repeating block forms passed first run |
| `CB` | 260 / 264 | 4 outstanding **by ruling, not defect** — see below |
| M1 un-prefixed | 290 / 290 | unchanged throughout M2 |

**1041 of 1045 prefixed vectors.** Implementation complete; under cold review.

### The `BIT n,(HL)` ruling

FUSE takes its undocumented bits 3/5 from the **tested value**; we take them from **MEMPTR**.
We are right, and the evidence is the shape of the discovery rather than an argument: the
effective address was first hard-coded for `BIT n,(IX+d)`, which fixed `DDCB` and broke nothing.
Then MEMPTR turned out to be the real rule — and `BIT n,(IX+d)` **fell out with no special case**
while plain `CB` went 256 → 260. **A rule that explains more than it was fitted to.** The corpus
needs two unrelated rules where the hardware has one. `zexall` at M4 adjudicates.

### What M2 removed

`StepError::UnsupportedPrefix` became unreachable — all four prefixes are handled, and unassigned
`ED` encodings are defined two-byte NOPs. The justification for keeping it (*"an M2+ core can
still fault on an undefined `ED` opcode"*) was simply false.

Removing it exposed the same lie one level down: `execute`, `execute_cb`, `execute_ed` and
`dispatch` all returned `Result<(), StepError>` with an **unconstructible `Err`**, and `step()`
unwrapped a `None` that was always `None`. A signature claiming a failure mode that does not exist
is the type-level form of a comment claiming a protection that does not exist — the class this
project keeps finding. All four now return `()`.

`StepError` and `fault()` remain, re-scoped: the mode-0 device byte is the one genuine runtime
condition, and it earns the type on its own.

## Milestone M1 — Z80 core, un-prefixed opcodes

### Proven

| | |
|---|---|
| **FUSE conformance, un-prefixed** | **290 / 290 pass** |
| Divergence in T-state totals, `PC`, `R`, `I`, `IFF1/2`, `IM`, memory, or any register but `AF` | **none** |
| Crate tests | 36 lib + doctests, green in dev **and** release (`overflow-checks = true`) |
| `clippy --all-targets -- -D warnings`, `fmt`, `cargo doc` | clean, 0 warnings |
| Out of M1 scope, counted not ignored | 1045 prefixed vectors — `DD` 343, `FD` 341, `CB` 264, `ED` 97 |

Two defects were found by the oracle and fixed. Both now carry regression tests that were
**proven to go red** on the original defect before being trusted:

- The `AluOp` table had `Or` and `Xor` transposed. The hardware field encodes `101`=XOR,
  `110`=OR. Caught by vector `af`: `XOR A` must yield `0x00` and returned `A | A`.
- `DAA` suppressed the magnitude corrections on the `N=1` path. The rule — in this repo's own
  `Z80-REFERENCE.md` — applies on both paths; only the direction differs.

The second one is why an external oracle earns its keep: the wrong behaviour was defended by a
plausible comment and would have shipped.

### Measured

See *Measured* in [`ARCHITECTURE.md`](ARCHITECTURE.md). Headline: **507× real-time** on a flat
bus, **294×** on a bus shaped the way M7 will be — 0.34 % of a frame budget. The performance
policy stays "optimise nothing", now backed by a number rather than an assumption.

### Hardening round — what a cold review found after the oracle was green

290/290 said the arithmetic was right. It said nothing about four decisions frozen in the public
API, which is what a reviewer is for. All are now implemented; the trace gate is what proves the
first one stays fixed.

| Item | Why it mattered | State |
|---|---|---|
| `Bus::tick` batched machine cycles and carried no address | Batching discards **88 of the corpus's 166** internal contention points. And the machine can track its own transfers but can never learn `IR` — which is what sits on the bus during the internal cycles of `ADD HL,ss`, `JR`, `DJNZ`, `CALL`, `PUSH` | fixed; trace asserted |
| No way to accept an interrupt | `interrupt()` / `nmi()` did not exist, and `set_state` cannot write memory or tick the bus, so there was no route out of `HALT`. M5 could not have booted | fixed; `halted` now drives `step()` and the acceptance rule lives in one place |
| `set_state` left `ei_pending` stale | A snapshot loaded just after `EI` dropped that frame's interrupt | fixed |
| `WZ` / `Q` absent from `CpuState` | Adding public fields is free with zero consumers and breaking with one | fixed; the `Q` plumbing landed, the rule waits for `zexall` |
| `hl_base` unbuilt | Decision 2's central mechanism | fixed; `base` threaded, so `DD 29` becomes `ADD IX,IX` with no new code |

Three findings the corpus produced that no amount of reading the spec would have: `RST`'s
internal cycle sits on `IR` while `CALL`'s sits on the last operand address, though both share
one handler; and `DJNZ` uses two different addresses in one instruction.

### Open — the authoritative register

This table is the single source for what is open. `ARCHITECTURE.md` links here and does not
duplicate it: the two were briefly kept in parallel and disagreed about four facts within one
session, which is the same failure mode that let the `tick` contract survive unchallenged.

| Item | State | Settled by |
|---|---|---|
| `Q` latch | Plumbing landed — `write_flags` is the single F writer, `q` cleared per step. The **rule** is not implemented; `SCF`/`CCF` use `F3/F5 = A & 0x28` | **M4, `zexall`.** FUSE's single-instruction vectors structurally cannot decide it: Q is defined by an instruction *sequence*. The two contested cases are `POP AF` and `EX AF,AF'` |
| `WZ` / MEMPTR | Carried in `CpuState`, never written | M4, when `BIT n,(HL)` first makes it observable |
| Resolved-target refactor | `read_operand`, `write_operand` and `tick_read_modify_delay` each recompute `pair(base)` independently. Free for `(HL)`; for `(IX+d)` the displacement must be fetched once and the addition charged once | **M2's opening move.** Needs a `Register(RegIndex) \| Memory(u16)` computed once and threaded |
| `Cpu<B: Bus>` struct-level bound | Downstream types naming `Cpu<Ula>` must carry `where Ula: Bus`; the fields need no bound to be well-formed | Removable at any time — non-breaking, but touches every signature written meanwhile |
| M1 fetch vs operand read | The machine cannot tell an M1 opcode fetch from an operand read; both arrive as `Bus::read` | Not blocking. Contention depends on address and `t mod 8`, both of which the machine has. A defaulted `fn fetch(&mut self, addr) -> u8 { self.read(addr) }` is non-breaking whenever a debugger or a precise floating-bus model wants it |
| Contention within a cycle | Only cycle *starts* are pinned; nothing asserts the address holds constant across a cycle's remaining T-states. It does — but by implementation, not by gate | One assertion over `tick_addresses` between consecutive transfers, if it earns its place |

### Where the corpus is not an oracle

FUSE elides the operand read on a not-taken `JP cc` / `JR cc` / `DJNZ`. The core reads them, and
the harness carries a **documented exception list** naming each vector.

The core is right, on five independent grounds: Zilog documents `JP cc,nn` with a **single** cycle
count (10 T) where `CALL cc` and `RET cc` have two, and a machine cycle that does not happen
changes the count; `PC` still advances by 3, and on the Z80 `PC` increments as part of the
operand-fetch cycle; MEMPTR sets `WZ = nn` regardless of the condition, which is a hardware
measurement from `BIT n,(HL)` experiments rather than documentation, and `WZ` cannot be loaded
from bytes never read; the corpus treats `CALL cc` — where the machine cycles genuinely differ —
identically to `JP cc`, where they do not, which is the signature of a bookkeeping convention; and
this crate's own doc comment on `jump_conditional` said so before the code did.

The general rule this establishes: **the core models the Z80, the harness models the corpus,
including its limitations.** Bending the core to match an emulator's bookkeeping would be fitting,
not fixing.

### Deliberately not changed

Recorded so they are not re-litigated:

- `#[allow(dead_code)]` on `flags::prefixed` is correct. `#[expect]` was tested and **breaks**
  the `-D warnings` gate: under `cfg(test)` every item is used, so the expectation goes
  unfulfilled and is promoted to an error.
- `Operand::register_index`'s `match` compiles to an 11-instruction cascade. Both proposed
  replacements trade compile-time exhaustiveness for ~10 instructions against 500× headroom.
- Big-endian pair storage costs ~2 instructions per 16-bit access. Priced, deliberate, kept —
  flipping it would be a large cosmetic diff for no measurable gain.

---

## The harness was reviewed, and it could report green while verifying nothing

A cold review of `crates/z80/tests/` — the code that decides whether the core is correct — found
two CRITICAL and six HIGH defects, **all in the lenient direction**, each reproduced with a
working probe. The headline, and the reason this section exists rather than a line in a table:

**With `testdata/fuse` absent, `cargo test -p z80` exited 0 with 87 passing tests — byte-for-byte
indistinguishable from a full green run.** Five tests verified nothing, and the word `SKIPPING`
never appeared, because libtest captures stdout for passing tests. The test *count did not change*.

The guard against this existed and was deployed nowhere: there was no CI, and `Z80_FUSE_REQUIRED`
appeared only in its own definition and a README example. Worse, it honoured the literal string
`"1"` only — `true`, `yes` and `on` silently disarmed it, and `true` is precisely how a GitHub
Actions `env:` block serialises an unquoted boolean.

This is the project's own doctrine turned on itself. `STATUS.md` already said it: *a failed edit
and an unbreakable guard produce the same exit code.*

### The class, which matters more than the instances

The reviewer's summary is worth keeping verbatim in spirit: **the most dangerous defect was not a
bug in a comparison — it was a comment.** Three places asserted a protection the code did not
provide — the "two independent accounts" T-state cross-check was one counter read twice
(`t_states += 1` and `bus.tick(addr)` are adjacent lines); an omission "permits exactly the listed
reads" while silently accepting extra writes and port accesses; "every vector counted, never
silently dropped" was a tautology over `partition`, and deleting all 264 `CB` vectors left the
suite green.

Each was written persuasively enough that a reader stops looking. That is the same failure mode
recorded above for `DAA` — *"the wrong behaviour was defended by a plausible comment and would have
shipped"* — recurring inside the file whose job is to catch it.

**The remedy adopted: a test per documented claim.** Prose asserting a guarantee is not a
guarantee; it is a hypothesis that needs its own failing case.

CI now exists (`.github/workflows/ci.yml`), fetches the corpus, and carries a second job whose
entire purpose is to assert that the conformance gate **refuses to pass** when the corpus is
absent.

### Comments rot at milestone boundaries, so the sweep belongs in "done"

Three consecutive reviews produced findings from stale doc comments, and the third had a **live
panic** attached: `t_states` was a `u8` because a comment argued that the longest Z80 instruction
is 23 T-states — true when written, and falsified the moment M2's `dispatch` made a run of `DD`
prefixes into *one* instruction whose length guest memory decides. The comment's own safety
argument became the defect, and the "loud panic rather than a silent wrap" it promised turned out
to be reporting a **legal instruction stream**.

The mechanism is worth stating because it tells you *when* to look: **every one of those comments
was true when written.** They do not decay gradually — they are falsified at milestone boundaries,
because that is when the claims they encode stop holding. Which is exactly when the sweep should
run.

**So a doc-comment sweep is part of a milestone's definition of done**, alongside the gate sweep.
Not a periodic tidy: a step, performed before the milestone is reported.

### Exhaustive on one axis can be weaker than a sample on another

The harness's ALU test was a 256-case proptest; it was replaced with an exhaustive sweep of all
1,048,576 operand pairs. That reads as unambiguous strengthening, and it was approved as such.

It was not. The sweep is exhaustive on the **operand** axis and **narrower than what it replaced**
on the **entry-flag** axis — the old `BOUNDARY_FLAGS = [0x00, 0xff]` was deleted, and its own
comment had said exactly why it existed: *"so every test also proves the instruction overwrites the
bits it owns and preserves the ones it does not."*

Two mutations, each proven to have landed before its verdict was trusted:

| Mutation | Exhaustive sweep | The deleted boundary test | Proptest | FUSE |
|---|---|---|---|---|
| `inc8` wrongly preserves entry `H` | **passes** | **fails on case 1** | fails | — |
| `CP` ORs entry bit 5 for one operand pair | **passes** | — | passes | **290/290 passes** |

The second is the shape of the register-`Q` behaviour this project defers to M4 — a leak of entry
`F` into a result — and it is invisible to every gate we have.

**More cases is not more coverage.** A count is a property of the loop; coverage is a property of
which *dimensions* vary. When replacing a sample with an enumeration, the question to ask is not
"how many more cases" but "which axis did the old test vary that the new one holds constant".

The comment also said the remaining flag bits were *"covered"* by the proptest. They are
**sampled** — 256 draws over a 2²⁴ joint space. That is the same pattern this document already
records twice: prose asserting a protection the mechanism does not provide, this time inside the
file whose entire claim is exhaustiveness.

## How this project is verified

Three tiers, and the distinction between them is the point:

1. **An external oracle decides correctness.** FUSE vectors now, `zexdoc` at M3, `zexall` at
   M4. Not opinion, not self-assessment — `OK` or not.
2. **Regression tests must be proven to bite.** A test that does not go red on the original
   defect is decoration. Every mutation is verified to have *landed in the file* before its
   verdict is trusted, because a failed edit and an unbreakable guard produce the same exit code.
3. **Claims about the emitted code are checked in the emitted code.** "It monomorphises" and
   "it lowers to a jump table" are assertions until someone reads the assembly.

Machine-level timing (contention, floating bus) has no such oracle and is verified against
known-demanding software. That is observation, and it is labelled as observation.

---

## Next — M2, costed

The 1045 prefixed vectors were run once as reconnaissance. **1043 fail**, and the shape of the
failures turns "implement four prefixes" into a sequence:

| Category | CB | DD | ED | FD | Total |
|---|---|---|---|---|---|
| decode (fault) | 264 | 341 | 89 | 341 | 1035 |
| registers / timing / contention / transfers | 264 | 341 | 97 | 341 | 1043 |
| flags | 139 | 179 | 57 | 179 | 554 |
| memory | 14 | 150 | 20 | 144 | 328 |

Three things decide the order of work:

1. **`DD`/`FD` are 684 vectors — 65% of M2 — and carry 294 of the 328 memory failures.** That
   concentration *is* the `(IX+d)` displacement path. The resolved-target refactor is therefore
   not housekeeping: it pays for two thirds of the corpus, and it goes first.
2. **Two `DD` vectors already pass** — `dd00` and `ddfd00`, both prefix-chain cases. The rule that
   each prefix is its own instruction with its own `R` increment is already correct.
3. **Eight `ED` vectors fail with no fault recorded** — `edb0`–`edb3`, `edb8`–`edbb`, i.e.
   `LDIR`/`CPIR`/`INIR`/`OTIR` and their decrementing twins. They are the only `ED` work that is
   not uniform decode, because they repeat. The harness's `MAX_STEPS_PER_VECTOR = 64` is
   comfortable at M1 (the longest is 17) and must be re-derived when they land, rather than
   discovered as a spurious step-limit failure.

The traps themselves are catalogued in [`Z80-REFERENCE.md`](Z80-REFERENCE.md): `DDCB`/`FDCB` put
the displacement byte *before* the opcode, prefix chains each cost their own M1 fetch and `R`
increment, and the `HL`→`IX`/`IY` substitution is asymmetric.

**One gap to close first: the harness has never been reviewed by anyone but its author.** The cold
review deliberately scoped `crates/z80/tests/` out, so the code that decides whether the core is
correct has had no independent eye. That is the wrong asymmetry to carry into M2, where the
harness grows a repeat mechanism and a new step cap.
