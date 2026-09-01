# Status

A living record of where the project actually is — what is proven, what is measured, what is
open. Updated as work lands, not once at the start.

**Last updated:** 2026-09-01, during M5.

> **The header said *"during M3"* while the document's top section was M4 and M5 had already
> landed.** It is corrected here rather than silently bumped, because the gap is the symptom of a
> real defect and not a typo: commit `2157331` shipped the whole M5 machine and **touched no file
> under `docs/`**. Its findings — including four open questions — went into a commit message and a
> crate doc comment instead. A commit message is not a register: it is never updated when an item
> closes, so an item recorded there is invisible the moment it stops being true. `ARCHITECTURE.md`
> already says the register lives here **and only here**; the corollary it did not spell out is that
> a milestone is not done until it has written to the register.

---

## Milestone M5 — the 48K boots — **SECTION INCOMPLETE, DO NOT READ AS A VERDICT**

> **This section is open.** The machine landed in commit `2157331` and the evidence that decides
> what it proves is **still being produced**: five gates are being written, and a cold review of the
> commit has returned REJECT with findings still in flight. What follows is the register of what M5
> opened — nothing more. **The measurements, the coverage table and the verdict are not here yet**,
> and their absence must not be read as their being favourable. A later pass writes them.

What is settled: the 48K boots to `© 1982 Sinclair Research Ltd` on frame 87, matched against
glyphs read from the ROM's own character set at `0x3D00` rather than a font table this crate wrote,
and 200 frames of boot run at 96× real time.

### What M5 opened — four items

They are **not listed here.** There is one open register in this project and it is
*[Open — the authoritative register](#open--the-authoritative-register)*, in the M1 section; a
second table in the newest section is precisely the duplication that let two documents disagree
about four facts in one session. The four items now live there, in one place, with their evidence:

- `timing::FIRST_CONTENDED_T_STATE = 14335` has **no oracle**.
- The read-modify-write contention residual — now closable, and not closed.
- **Five mutations leave the boot gate green.**
- **Nothing runs the boot gate.**

---

## Milestone M4 — `zexall`, and what a green oracle is worth

M4 was written as *"undocumented flags — `zexall` passes"*. It already passed, on the first
run it was ever given. So the milestone became an **evidence** task rather than an
implementation one: make it a committed gate, and state precisely what its green does and
does not prove.

**Both exercisers now report 67/67**, and they are asserted to execute the *same instruction
stream* — 5,764,169,610 instructions and 46,734,977,142 T-states each, **identical to the
single instruction**. That is not a coincidence to note in passing, it is a pinned constant
(`EXERCISER_SCALE`): `zexdoc` and `zexall` are the same program differing in 190 bytes, all of
them flag masks and expected CRCs, so a divergence would mean something moved underneath both.
`libtest` runs the pair concurrently, so together they still cost ~43 s.

Pinning the total also gives the `zex` path a **coarse timing assertion** it did not have —
an aggregate T-state count over 5.8 billion instructions. It would catch a systematic cycle
error; it cannot catch a per-instruction one that cancels out.

### What `zexall`'s green proves — three claims that must not blur into one

1. **It does grade the undocumented `F3`/`F5` bits.** Not read off its source — established by
   controlled mutation *with a control*. Forcing those bits to a constant `0` or `0x28` fails
   `<daa,cpl,scf,ccf>` under `zexall` while `zexdoc` stays 67/67; a control mutation of a
   **documented** bit (`SCF` not setting carry) fails **both**, which is what proves the group
   is executed and graded rather than skipped. Confirmed structurally too: all 67 of `zexdoc`'s
   masks (`0xc7`/`0xd7`/`0x53`) have bits 3 and 5 clear, and all 67 of `zexall`'s are `0xff`.

2. **It cannot separate the Q rule from `A & 0x28`.** Its 67/67 has been observed under
   **three different implementations** — `A & 0x28`, the Q rule, and a core whose latch was
   stuck at zero. **A verdict identical under three rules is evidence for none of them.**

   The reason is *not* the one this document gave for a while. `zexall` does **not** keep
   `Q == F`; it reaches `Q ≠ F` in 98.4 % of `SCF`/`CCF` executions. It reaches the *shape*
   constantly and never the *bit pattern* — the rules differ iff `((Q ^ F) & ~A) & 0x28 ≠ 0`,
   which held zero times in ~32,000 executions. **The counts and the full account live in
   *Reaching for proof where you have measurement*, below, and only there** — this is a
   summary that defers to it, because two copies of one measurement is how this document's
   own recorded failure started.

   Claims 1 and 2 must be held apart: `zexall` **is** sensitive to `F3`/`F5`, and it **cannot**
   separate two rules that agree everywhere it looks. The first does not make the second false.

3. **The entry latch is graded only by FUSE**, by exactly two vectors, and **mid-sequence `Q`
   has no oracle at all.**

### Coverage — what each oracle sees, and what nothing sees

The last column is the useful one.

| Property | FUSE | `zexdoc` | `zexall` | Covered by |
|---|---|---|---|---|
| Documented flags & results, per instruction | ✅ 1335 vectors | ✅ | ✅ | all three |
| Instruction semantics in **long sequences** | ❌ one instruction per vector | ✅ 5.8 × 10⁹ | ✅ | `zex` only |
| Undocumented `F3`/`F5` on ordinary results | ✅ | ❌ masked off in all 67 groups | ✅ | FUSE + `zexall` |
| `SCF`/`CCF` undocumented bits, **entry latch** (`Q == F`) | ⚠️ `37_1` and `3f` **only** | ❌ masked | ❌ never varies `A`/`F` enough to separate the rules | **two vectors** |
| `SCF`/`CCF` where the **Q rule and `A & 0x28` disagree** | ❌ | ❌ | ❌ reaches `Q ≠ F` 98.4 % of the time, but `((Q ^ F) & ~A) & 0x28 ≠ 0` **zero** times | **nothing** |
| Per-instruction T-state totals | ✅ | ❌ | ❌ | FUSE only |
| Aggregate T-state total over 5.8 × 10⁹ instructions | — | ✅ pinned | ✅ pinned | `zex` only |
| Ordered bus transfers (`MR`/`MW`/`PR`/`PW`) | ✅ | ❌ | ❌ | FUSE only |
| **Per-T-state bus addresses — contention** | ✅ 166 points | ❌ `tick` is a no-op | ❌ | **FUSE only** |
| Interrupt acceptance, `IM 0/1/2`, `NMI`, `RETN` | ❌ no vector injects one | ❌ | ❌ | **nothing** |

### Two gaps worth naming before they are rediscovered

**Contention is not covered on the `zex` path, and that is deliberate.**
`cpm::FlatBus::tick` is an empty function. `machine::TestBus` pushes one `u16` per T-state,
which is right for a 30-T-state vector and impossible here: at 46.7 billion T-states that log
alone would be ~93 GB. So the two harnesses share no bus on purpose. The consequence is
precise: **the 5.8-billion-instruction path verifies instruction semantics and nothing about
timing except the aggregate total.** FUSE remains the only per-T-state, per-address oracle,
and it covers 1335 single instructions. **M7's contention work cannot lean on the `zex`
gates at all** — writing this down now is cheaper than rediscovering it then.

**Interrupts have no oracle.** No FUSE vector injects one and no exerciser generates one, so
`Cpu::interrupt`, `Cpu::nmi`, the three interrupt modes, `RETN`, and the `EI` one-instruction
deferral are verified by unit tests in `crates/z80/src` and by nothing external. That is a
different class of evidence from everything above, and M5 is where it starts to matter.

## Milestone M3 — `zexdoc`

**All 67 test groups report `OK`, first run, with no change to `crates/z80/src`.**

`zexdoc` is a different shape of oracle from FUSE and that is the point of it. FUSE sets up a
state, runs one instruction and compares; `zexdoc` runs **5,764,169,610 instructions**, folds
every result into CRCs, and compares them against values built into its own image. It proves
the instructions still hold up in *sequences* billions deep, where a wrong flag bit poisons a
checksum thousands of instructions after the mistake that caused it.

| | |
|---|---|
| Groups reporting `OK` | **67 / 67** |
| Instructions / T-states | 5,764,169,610 / 46,734,977,142 |
| Wall clock, release | **43.1 s** — ~308x real time at 3.5 MHz, within 7 % of `benches/step.rs`'s 329x |
| Wall clock, `dev` profile | **~20 minutes** — 27x slower, which is why the gate is `#[ignore]`d and release-only |
| Port accesses | 0, asserted — a CP/M exerciser performs none, so any would mean an `IN`/`OUT` misdecode |

### The gate had to be proven able to fail, and one attempt to prove it did not work

A green run proves nothing until the run is proven. Two things were caught by taking that
seriously rather than by being careful:

- **The group count was nearly wrong in the harness's favour.** A first derivation scanned the
  binary for printable strings and found **65**; it missed two names padded with fewer than
  four dots. The count now comes from walking `zexdoc`'s own descriptor table — 67 entries at a
  0x60 stride, following the `JP 0` in its start routine. **A gate pinned at 65 would have
  failed a correct core**, and it would have looked like a CPU defect.
- **The obvious way to prove the `ERROR` path did not work.** Running `zexall`, expecting it to
  fail, returned 67/67 — so it proved nothing. What *did* work was corrupting one expected CRC
  inside a copy of `zexdoc.com` itself: the gate went red with
  `CRC expected deadbeef, found f8b4eaa9`, and the `found` value being the *correct* CRC is
  what shows the core was right and only the expectation was poisoned. Restored byte-identical
  afterwards, verified by SHA-256, and green returned.

### The `zexall` question — the experiment, and what it does and does not settle

`zexall` also reports 67/67 today. That is surprising, because this document has said since M1
that the Q latch is unimplemented and `SCF`/`CCF` take `F3/F5 = A & 0x28`. The question worth
answering is not "is this `zexall` build genuine" but **does `zexall` grade those bits at all** —
so the rule was mutated in a scratch copy of `src/` and both exercisers re-run against each
mutation. Every mutation was verified present in the file before its verdict was trusted.

| Mutation to `flags::scf`/`ccf` | `zexdoc` | `zexall` |
|---|---|---|
| **A** — `F3/F5` always `0` | 67/67 OK | **FAIL** `<daa,cpl,scf,ccf>`: expected `6d2dd213`, found `c4ab71f0` |
| **B** — `F3/F5` always `0x28` | 67/67 OK | **FAIL** same group, found `f14add2d` |
| **C** — control: `SCF` does not set carry | **FAIL** expected `9b4ba675`, found `d99ebf0e` | **FAIL** found `2ff8cb68` |

Three facts follow, and the control is what makes them facts rather than inferences. **C** is a
*documented* bit, and it fails under both — so the group really is executed, really is graded by
both binaries, and the mutation mechanism genuinely reaches live code. Against that baseline:

1. **`zexall` grades the undocumented `F3`/`F5` bits of `SCF`/`CCF`.** Two different wrong
   values are both caught, with different CRCs.
2. **`zexdoc` does not grade them** — its mask for this group is `0xd7`, which has bits 3 and 5
   clear. A and B are invisible to it, exactly as the masks predict.
3. **So the current rule passes `zexall` on merit**, not by not being looked at.

**What this does *not* settle, and the distinction matters.** That `zexall` grades the bits is
not the same as `zexall` discriminating the *Q rule* from the simpler `A & 0x28` rule. The
earlier hypothesis — that `zexall`'s harness restores flags with `POP AF` / `EX AF,AF'`
immediately before each tested instruction, which are precisely the two cases this document
already names as contested, so `Q` would be zero and both rules would agree — remains
**unverified**. It is consistent with every number above, and it would explain passing on merit
without the rule being implemented.

**The practical consequence for M4 is immediate:** its stated premise — that `zexall` fails
until `Q` lands — is false for this build. `<daa,cpl,scf,ccf>` passes today.

### The Q rule then landed, and FUSE caught a defect no other gate sees

The rule shipped as `((q ^ f) | a) & 0x28`, which collapses to `a & 0x28` whenever `q == f` —
so it should have been invisible to every existing gate. It was not. **FUSE went from 290/290
to 288/290**, and the two failures are `37_1` and `3f`, the only vectors that can see the
latch at all.

The harness was half the cause and is fixed here: `cpu_state()` defaulted `q` to zero, but
**loading a state is a `POP AF`** — the load is the last thing that wrote `F`, so the latch
must equal it. Zero is the one value that makes a positive false claim. `q` is now set from
`F`; `wz` stays defaulted, because the corpus genuinely carries no MEMPTR column.

That alone did not fix it, and the reason is a **defect in the core**. `begin_operation()`
runs `self.q = 0` at the start of every `step()`, and `SCF`/`CCF` read that same field — so
the latch they see is always zero, whatever was loaded or whatever the previous instruction
wrote. The shipped rule therefore evaluates `(f | a) & 0x28`, which is neither the Q rule nor
the `a & 0x28` it replaced.

Measured, each mutation verified present in the file before its verdict was trusted:

| Core | FUSE | `zexdoc` | `zexall` |
|---|---|---|---|
| Pre-Q (`a & 0x28`) | 290/290 | 67/67 | 67/67 |
| **As shipped** (latch stuck at 0) | **288/290** | 67/67 | 67/67 |
| Shipped **+ `q_prev`** (below) | **290/290** | 67/67 | 67/67 |

The fix is to keep the previous instruction's latch instead of destroying it — add a
`q_prev: u8`, make `begin_operation` do `self.q_prev = self.q;` *before* `self.q = 0;`, and
have the two `SCF`/`CCF` call sites read `q_prev`. Proven in a scratch copy: 290/290 and
1045/1045. **`crates/z80/src` is owned elsewhere and was not modified.**

### The instrument problem, which the table above makes concrete

**`zexall` did not catch this.** It passes 67/67 against a core whose latch is stuck at zero
*and* against a correct one — and it also passed against pre-Q `a & 0x28`. Three different
rules, one verdict. Yet `zexall` is not blind to these bits in general: forcing them to a
constant `0` or `0x28` does make it fail.

> **Superseded by measurement at M4, and the correction is worth keeping visible.** This
> paragraph originally guessed that `F`'s bits 3 and 5 must be *clear* in the sequences
> `zexall` exercises, and a companion claim guessed that `Q == F` throughout. Instrumenting
> the core disproved the second outright — `Q ≠ F` in 98.4 % of `SCF`/`CCF` executions — and
> replaced the first with the exact condition: the rules differ iff `((Q ^ F) & ~A) & 0x28 ≠ 0`,
> which held **zero times** in ~32,000 executions. See the M4 section. Two guesses that
> *predicted the right verdict for the wrong reason* is precisely the failure this document
> keeps cataloguing, so the guess is left here with its correction attached rather than
> quietly overwritten.

So as of M4 the position is: **two FUSE vectors are the only gate in this project that can see
the flag latch at all.** And the search for a better instrument is narrower than it looked: it
does **not** need an exotic instruction sequence — per the measurement, `Q ≠ F` is everywhere.
It needs a corpus that **varies `A` and `F` so bits 3 and 5 actually diverge**. That is a much
easier thing to find, which is the useful half of this finding.

### The gate runs nowhere unless CI runs it

An `#[ignore]`d gate that no pipeline executes is not a gate. It is the same defect as
`Z80_FUSE_REQUIRED`, which this document already records as having "appeared only in its own
definition and a README example" — a guard that exists solely in a file nobody runs.

`.github/workflows/ci.yml` therefore gained a `zexdoc` job, and `guard-must-be-armed` gained a
matching corpus-absent check. **`--ignored` is load-bearing in both**, and this was measured
rather than assumed: with `testdata/zex` moved aside, `cargo test -p z80 --test zex_oracle`
exits **0 with 16 passing tests**, never looking for the exerciser — while the same command
with `--ignored` exits 101. A CI step written without the flag would assert nothing and look
identical to one that asserts everything.

> **This has not shipped.** The workflow file is written and correct, but the session token
> lacks `workflow` scope, so `.github/` cannot be pushed. Until someone with that scope pushes
> it, **the M3 gate is verified locally and enforced nowhere.**

### What the harness learned from the M2 review

The verdict is a pure function over a parsed report rather than a chain of inline `assert!`s.
The reason generalises beyond this file: an inline assertion inside a 43-second run can only be
proven to bite by a manual mutation nobody repeats, whereas the same rules as a function have
**one failing case each, running in microseconds on every `cargo test`**, corpus or no corpus.

Six tests cover the verdict rules and ten cover the CP/M shell and the report parser — sixteen
in all, none of them `#[ignore]`d, none of them needing `testdata/`. The one that matters most
is `a_run_that_stopped_early_is_a_fault_even_though_every_line_said_ok`, because a truncated run
prints nothing but `OK` lines and "did any line say ERROR?" passes it.

---

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

This table is the single source for what is open, **across all milestones** — it sits in the M1
section for historical reasons, not because it is scoped to M1. `ARCHITECTURE.md` links here and
does not duplicate it: the two were briefly kept in parallel and disagreed about four facts within
one session, which is the same failure mode that let the `tick` contract survive unchallenged. The
M5 section above lists its four item *names* and defers here for their state, for the same reason.

| Item | State | Settled by |
|---|---|---|
| **M5** — `timing::FIRST_CONTENDED_T_STATE = 14335` has **no oracle** | 14334 produces byte-identical output, so nothing in this repository currently distinguishes the two. Recorded as an open question rather than a settled constant | A known-timing test program that reports measured T-state counts — item 2 of `MACHINE.md`'s verification plan, and the only item there that is an oracle rather than an observation |
| **M5** — read-modify-write contention residual | One contention point, 0–6 T-states, per instruction that performs exactly one internal cycle at the address it just read. Pinned by a test that asserts the loss rather than hiding it. **Now closable**: `Bus::fetch` has landed in the CPU | `Ula` implementing `fetch`. `crates/spectrum/src/ula.rs` still inherits the default, and its own test records that it should go red when that changes |
| **M5** — five mutations leave the boot gate green | `/INT` never asserted; the keyboard reporting every key held; the ROM slot made writable; contention removed entirely; contention phase off by one (byte-identical output). Positive panic-probes confirm the keyboard read and the interrupt acceptance are *executed* — they are simply not *graded* | Five targeted gates, **in flight**. Until they land, the boot gate grades the memory map's read side and the screen, and nothing else |
| **M5** — nothing runs the boot gate | It is `crates/spectrum/examples/boot.rs`; `cargo test` builds an example without calling `main`. Deleting the committed ROM left the suite at 72 passed | A real `#[test]`, **in flight**. See *A gate that nothing runs, for the third time* |
| The flag latch has almost no instrument | Two FUSE vectors (`37_1`, `3f`) are the **only** gate that can see it. `zexdoc` masks the bits off; `zexall` passes against three different rules including a stuck-at-zero latch | A corpus with a flag-setter → no-flag instruction → `SCF` sequence. Neither existing corpus has one |
| CI does not run the M3 gate | `.github/workflows/ci.yml` has the `zexdoc` job written, but `.github/` cannot be pushed from the session that wrote it — the token lacks `workflow` scope | Someone with `workflow` scope pushing it. Until then the gate is verified locally and enforced nowhere, which is the `Z80_FUSE_REQUIRED` defect again |
| `WZ` / MEMPTR | Carried in `CpuState`, never written | M4, when `BIT n,(HL)` first makes it observable |
| Resolved-target refactor | `read_operand`, `write_operand` and `tick_read_modify_delay` each recompute `pair(base)` independently. Free for `(HL)`; for `(IX+d)` the displacement must be fetched once and the addition charged once | **M2's opening move.** Needs a `Register(RegIndex) \| Memory(u16)` computed once and threaded |
| Contention within a cycle | Only cycle *starts* are pinned; nothing asserts the address holds constant across a cycle's remaining T-states. It does — but by implementation, not by gate | One assertion over `tick_addresses` between consecutive transfers, if it earns its place |

**Not audited in this pass:** *resolved-target refactor*, *`WZ` / MEMPTR* and *contention within a
cycle* were carried forward as written and were **not** re-checked against the crate. `WZ` in
particular has passed the milestone its row names as its settling condition, so treat its state as
unverified rather than current. Whoever next opens this register should re-derive all three; the
`panic_bounds_check` correction in `ARCHITECTURE.md` is what an unaudited carry-forward costs.

### Closed — items that left the register, and what closed each one

An item leaves the Open table only into this one, with its evidence. A row that simply disappears
is indistinguishable from a row nobody re-read.

| Item | What it was | What closed it |
|---|---|---|
| **M1 fetch vs operand read** | *"Not blocking. Contention depends on address and `t mod 8`, both of which the machine has. A defaulted `fn fetch(&mut self, addr) -> u8 { self.read(addr) }` is non-breaking whenever a debugger or a precise floating-bus model wants it."* | **The reasoning was wrong and M5 measured it.** `LD A,B` and the read-modify half of `INC (HL)` emit byte-identical streams — `read(addr)` then four `tick(addr)` — while owing one contention point and two respectively, so address and phase are not sufficient however true it is that contention depends on them. `crates/spectrum/src/machine_cycle.rs` had to reconstruct the boundaries by deferral, at a residual of one contention point (0–6 T-states) on the read-modify-write family. `Bus::fetch` has since landed in `crates/z80/src/bus.rs`, defaulted, with every M1 opcode fetch routed through it. **The CPU half is closed; the machine half is not** — `Ula` still inherits the default, and the residual is the second row of the Open table above. Full account in [`MACHINE.md`](MACHINE.md); the two rulings it forced are in [`Z80-REFERENCE.md`](Z80-REFERENCE.md) |
| **`Q` latch — latch lifecycle** | *"`((q ^ f) \| a) & 0x28` has landed, but `begin_operation()` zeroes `q` before `SCF`/`CCF` read it… **FUSE is red: 288/290**… **Blocks M3**."* | The `q_prev` fix landed in `crates/z80/src`: `begin_operation` now assigns `self.q_prev = self.q` before clearing, and both `SCF`/`CCF` call sites read `q_prev`. Re-measured here rather than inherited — `cargo test -p z80 --test fuse_vectors` reports **290 executed, 290 passed, 0 failed** and **1045 executed, 1045 passed, 0 failed**. The row had been red in the register through two merged milestones. **What it does *not* close** is the row above it: the rule is still graded by two FUSE vectors and nothing else |
| **`Cpu<B: Bus>` struct-level bound** | *"Downstream types naming `Cpu<Ula>` must carry `where Ula: Bus`… Removable at any time."* | Removed. The declaration is `pub struct Cpu<B>` |

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

> **A fourth instance, found at M5, and this one was in a *measurement*.** `ARCHITECTURE.md`'s
> *Measured* section claimed register indexing was proven in range; it was true at M1 and falsified
> at M2, and survived three further milestones. The numbers and the bisect live there and **only**
> there. Two things generalise. First, the sweep must cover **measured rows, not only prose** — a
> number reads as more durable than a sentence and is not. Second, that section carried an explicit
> instruction to *"re-run after M2"*, naming the exact milestone that broke it, and nothing enforced
> it. **An unenforced instruction to re-measure is the same defect as an unrun gate**, and it failed
> the same way: silently, while looking green. The remedy adopted is the one already used for the
> gates — every re-measured row now ships with the command that produced it, so re-running is
> cheaper than re-deriving.

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

### Reaching for proof where you have measurement

The M3 justification for shipping an unverifiable rule claimed `zexall` cannot distinguish the Q
rule from `A & 0x28` **provably**: it restores `F` before each test, so `Q == F`, so the expression
collapses. The algebra was verified exhaustively over all 65,536 `(a, f)` pairs and is correct.

**The premise was false.** A cold review instrumented the core and counted every `SCF`/`CCF` across
a full run:

```
SCF: executed=16000  q!=f=15750  rule-would-differ=0
CCF: executed=16000  q!=f=15750  rule-would-differ=0
```

`Q ≠ F` in **98.4 %** of executions. `zexall` does not run inside the collapse region at all.

The conclusion survives — three full runs, including one with the rule deleted and one with the
latch stuck at zero, all report 67/67 — but for a different reason. The rules differ iff
`((Q ^ F) & ~A) & 0x28 ≠ 0`: a bit of `Q ^ F` set in position 3 or 5 **where `A` has it clear**.
Over ~32,000 executions that held zero times. The exerciser reaches the *shape* constantly and
never the *bit pattern*.

**Three consequences, and the third is the expensive one:**

1. The claim that no corpus generates the required sequence shape is also false. `q=00, f=04` is
   that shape, 15,750 times.
2. The measurement is **stronger than the proof it replaced** — 32,000 executions with zero
   observable divergence, plus the exact condition that would change the answer. A reader can act on
   that; nobody can act on a proof whose premise is wrong.
3. **It misdirected the search.** An instrument that would decide the rule does not need a special
   sequence — it needs one that **varies `A` and `F` so bits 3 and 5 diverge**. A much easier thing
   to find, and the old wording sent the next person after the wrong one.

The general rule: **an algebraic argument is exactly as strong as its weakest premise**, and the
premise here was a plausible claim about another program's internals, asserted rather than measured.
Where a measurement is available, it outranks a proof about someone else's code.

### A gate that nothing runs, for the third time — and the form got worse

This document already records the pattern twice: *"The gate runs nowhere unless CI runs it"* for the
M3 `zexdoc` job, and *"verified locally and enforced nowhere"* for the workflow that cannot be
pushed. M5 produced the third instance and the most complete one.

The boot gate — the thing `MACHINE.md` ranks **first** in its verification plan — is
`crates/spectrum/examples/boot.rs`. **`cargo test` builds an example and never calls its `main`.**
The cold review of commit `2157331` deleted the committed ROM and the suite stayed at 72 passed;
there was no `crates/spectrum/tests/` directory at all.

**The escalation is the finding.** The M3 gate was an `#[ignore]`d `#[test]` — invisible by default,
but a test, reachable with `cargo test -- --ignored`, and discoverable by anyone listing the suite.
An example is not a test in any form: no flag reaches it, no listing shows it as skipped, and its
absence from a run leaves no trace. The earlier instances were gates that were *not scheduled*; this
one was never a gate.

It is also the same failure the harness review names as the most dangerous — *"the most dangerous
defect was not a bug in a comparison, it was a comment"*. The commit message describes what the gate
covers, in a table, measured by mutation. Every word of that is accurate about what the example
*would* grade **if anything ran it**, and nothing in it says that nothing does.

**Being written now, not fixed.** A real test is in flight in `crates/spectrum/tests/`. This entry
stands until it lands, and the next docs pass closes it — the register does not get to record a fix
before the fix exists.

### An invariant that looked universal, and the test that found its scope

`Bus::fetch` arrived with an obvious companion rule: **one `fetch` per `R` increment.** It is nearly
true, it is the reason the method can be described in one line, and it is wrong as stated.

`R` increments once per **M1 cycle**. `fetch` is called once per M1 cycle **that reads memory**. Two
M1 cycles are unusual and only one of them breaks the correspondence. A halted CPU's cycle *does*
read memory — it fetches the `HALT` opcode again and throws it away — so it keeps the count. An
interrupt acknowledge asserts `/IORQ` instead of `/MREQ`, reads no memory at all, and therefore
refreshes without fetching. The invariant is exact across `step()`, where a frame loop spends all of
its time, and off by one per accepted interrupt or NMI.
The hardware rules themselves are in [`Z80-REFERENCE.md`](Z80-REFERENCE.md), where hardware rules
live.

**The lesson is small and it is about method, not about the Z80.** The exception was not found by
thinking harder about the rule. It was found by **trying to write its test** — at which point the
acknowledge path had to be given a verdict and refused to fit. `bus_timing.rs`'s
`an_interrupt_acknowledge_refreshes_without_fetching` now exists so the exception cannot quietly
become a bug: routing the acknowledge through `fetch` would read as a tidy-up and would charge a
memory cycle the hardware never performs.

Stated generally, and it is the cheap half of every other lesson in this document: **an invariant
asserted has no scope; an invariant tested acquires one.** The cost of finding out which is one
test.

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
