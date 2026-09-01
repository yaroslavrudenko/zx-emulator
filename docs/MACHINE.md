# The machine — M5 onward

Design for `crates/spectrum`. The CPU is finished and graded by three oracles; this document is
about everything around it, where the verification story is completely different and worse.

## The honest starting point: there is no oracle here

M1–M4 had `OK` or not. FUSE grades an instruction, `zexdoc` and `zexall` grade the processor, and
a mutation either dies or does not. **None of that exists for a machine.**

Contention correctness, floating-bus values, screen timing and keyboard behaviour are verified
against *known-demanding software* — a demo tears or it does not, a loader works or it does not.
That is observation, and this project has a rule about the difference: `STATUS.md` says the two
modes must not be mixed by accident. M5 enters observation mode deliberately.

So the design goal is not "make it work". It is **make what is wrong visible**, because nothing
else will tell us.

> **CORRECTION — the heading is now wrong for one of the four things it names, and only for a 48K.**
> `crates/spectrum/tests/timing_oracle.rs` landed on 2026-09-01 and grades the 48K contention model
> against 68 hardware-measured numbers this project did not write. *The Timing Oracle*, below,
> carries the full account. Two of the sentences above therefore need their scope narrowed rather
> than deleted:
>
> - *"None of that exists for a machine"* — **it does now, for 48K contention.** A mutation either
>   dies or does not, and thirteen were run against it.
> - *"Contention correctness … verified against known-demanding software … that is observation"* —
>   **48K contention is no longer graded by observation.** Floating-bus values, screen timing and
>   keyboard behaviour still are, and the sentence remains true of them.
>
> **What is established is narrower than "the contention model is right", and the difference
> matters.** Precisely: **the first contended T-state falls exactly 14335 T-states after `/INT`**,
> given that this machine asserts `/INT` at frame T-state 0. Three things that reading does *not*
> establish, each demonstrated by a green mutation rather than argued:
>
> - **The frame's origin is still a convention.** Moving `/INT` one T-state later *and* the window
>   to 14336 leaves the gate green. The corpus grades the interval, not either endpoint.
> - **The 64-line pre-display count closes only as a consequence.** `64 × 224 − 1 = 14335` is the
>   derivation and its *product* is measured; the 64 and the 224 are not observed independently, so
>   a compensating pair of errors survives.
> - **The interrupt window's *length* is still ungraded.** Cutting `INTERRUPT_T_STATES` from 32 to
>   24 leaves the gate green.
>
> And it says nothing whatever about a 128 — see [`M7.md`](M7.md), which is where the question of
> whether an equivalent 128 oracle exists is taken.

## Decision 1 — the machine owns the clock, the CPU does not

`Cpu::step()` returns T-states, but that number is **not** the frame clock. The bus is:

```rust
fn tick(&mut self, addr: u16);   // one T-state, with the address the Z80 drives
```

Contention *adds* T-states, and it adds them on the machine's side. This was measured at M1: with
a contended bus, `step()`'s return was identical to the flat-bus run while the bus's own clock
diverged. So:

> **The frame is driven by the bus's tick count, never by summing `step()`.**

A machine that adds up `step()` returns will run a frame that is correct in instruction count and
wrong in time, and every timing-sensitive effect will be subtly off with nothing failing.

## Decision 2 — one step can overrun the frame budget

There is no small maximum for a single instruction. A run of `DD`/`FD` prefix bytes is **one
instruction**, four T-states per prefix, and guest memory decides how long the run is. The frame
loop must handle a step that carries it past the interrupt point rather than assuming it can stop
exactly on 69888.

## Decision 3 — paged memory from the start, 48K as a configuration

Already settled in `ARCHITECTURE.md` Decision 5 and unchanged: slots → banks, with 48K as the
special case where paging is locked. The 128's contention is a property of **which bank is paged
in**, not of the address range, so the check happens per access against the current slot map.

`Memory` must keep the bank index **provably in range** — a newtype or a mask at the point of use.
Measured at M1: an unproven index costs 6.6 % in bounds checks, and it is free to avoid.

> **The 6.6 % is falsified, and the sentence is left standing because the decision it justified
> has shipped and someone will come looking for the number.** The decision itself is unchanged —
> `BankIndex`/`RomIndex` mask on construction and at the point of use, and
> `crates/spectrum/src/memory.rs` builds on that. What is wrong is its stated **price**.
>
> `benches/step.rs` now *measures* the difference instead of quoting it: `PagedRam<MASKED>` is
> one bus with one line of difference — `banks[self.slots[slot]]` against
> `banks[self.slots[slot] & 3]`. The masking does what it claims, verified in the emitted
> assembly: all four `panic_bounds_check` sites on the access paths sit in the `MASKED = false`
> instantiation and the masked one has none. **And it buys nothing measurable.** Over four runs
> the masked variant was *slower* in three; the spread within one variant (±1.4 %) is larger than
> the difference between them and the sign does not hold. A 6.6 % effect would be ~10 µs on a
> ~148 µs workload and it is not there. Numbers, command and dates are in
> [`ARCHITECTURE.md`](ARCHITECTURE.md)'s *Measured* section and **only** there.
>
> So *"it is free to avoid"* survives and *"an unproven index costs 6.6 %"* does not. The bank
> index stays provable because a masked index is clearer and cannot be got wrong, not because it
> was bought with a measured 6.6 %. **Nobody should spend a newtype at M7 expecting to buy back
> time.** The original claim's method was never recorded, which is why it could only be re-taken
> rather than defended — the class is written up in [`STATUS.md`](STATUS.md).

## Decision 4 — the ULA is the second clock, and it is the hard part

The ULA both *draws* and *stalls*. Those are one mechanism seen from two sides: the screen address
it is fetching at T-state `t` is exactly what makes the CPU wait if the CPU wants that bank.

| | 48K | 128 |
|---|---|---|
| T-states per frame | 69888 | 70908 |
| Interrupt | 50 Hz, at the frame start | same |
| Contended | `0x4000–0x7FFF` | banks 1, 3, 5, 7 in **any** slot |
| Screen | bank 5 | bank 5 or shadow bank 7 |

The contention pattern is a function of `t mod 8` over the 128 T-states of each display line's
active window, across 192 lines. The delay table is small; getting the *phase* right is the work.

## Verification plan, such as it is

Ranked by how much they actually prove:

1. **Boots to `© 1982 Sinclair Research Ltd`.** Binary and cheap. It was ranked first here on the
   claim that it *"exercises the memory map, the interrupt, the keyboard scan and the screen in one
   go"*. **M5 measured that claim and it is wrong**, so it is corrected rather than quietly
   narrowed. Corrupting the screen layout turns the gate RED; **five** mutations leave it green —
   `/INT` never asserted, the keyboard reporting every key held, the ROM slot made writable,
   contention removed entirely, and the contention phase off by one (byte-identical output).
   Positive panic-probes confirm the keyboard read and the interrupt acceptance are *executed*;
   they are simply not *graded*. It grades the memory map's read side and the screen. That is all.

   **Worse, at commit `2157331` nothing ran it.** It is `crates/spectrum/examples/boot.rs`, and
   `cargo test` builds an example without ever calling `main`. Deleting the committed ROM left the
   suite at 72 passed. This is the third time this project has shipped a gate that no pipeline
   executes — `STATUS.md` records the other two — and it is the worst form so far, because the M3
   gate was at least an `#[ignore]`d `#[test]` reachable with `--ignored`, while an example is not a
   test in any form. **That test has since landed** as `crates/spectrum/tests/boot.rs`, which runs
   the ROM under `cargo test` and asserts both the message and the frame it appears on. The example
   remains, as an example; it is no longer the gate. The mutation verdicts above were measured
   before it landed and are left as they were recorded — they are what the *boot gate* proves, and
   `crates/spectrum/src/lib.rs` carries the current coverage table.

   **What those verdicts are worth was then re-measured, and the finding got smaller and
   sharper.** Re-run at `2157331` against the pre-gate lib target — baseline 72 passed, no
   `tests/` directory, each mutation's landing asserted before its verdict — **four of the five
   were already red** from unit tests inside `src`, which an example nothing calls never involved:

   | Mutation | Verdict, pre-gate | Failing tests |
   |---|---|---|
   | `/INT` never asserted | **RED** | 5 |
   | Keyboard reports every key held | **RED** | 7 |
   | ROM slot made writable | **RED** | 1 |
   | Contention removed entirely | **RED** | 13 |
   | Contention phase off by one | **GREEN** | — |

   **The contention phase is the only survivor of the five.** A permutation of the keyboard
   matrix survives too and is the other reason `keyboard_matrix.rs` exists, but it **is not among
   the five above**; a cold review found it separately. So the honest before-picture is not "five
   properties were ungraded" but this: **the machine's behaviour was tested in isolation and never
   through the machine**, and the contention phase and the matrix wiring had no gate in any form.
   That is a smaller claim than the five green rows suggest and a more precise one, and it is what
   makes the two surviving mutations the significant ones —
   `crates/spectrum/tests/contention_phase.rs` and `keyboard_matrix.rs` exist because nothing
   anywhere could see those two properties.

   > **This passage said *"three of the five"*, *"only two mutations survived"*, and — worst —
   > *"which three were already red is not recorded here and is not derived here",* naming the
   > command that would derive it.** All three sentences are replaced above by the derivation
   > itself. The hedge is the part worth noticing: it correctly identified that the figure was
   > un-derived, printed the one-line command that settles it, and then the figure was quoted for
   > three documents anyway. **Naming what would settle a claim is not a substitute for settling
   > it**, particularly when the cost is one command. `STATUS.md` records the class.

   `STATUS.md`'s M5 section carries the full account, and the ten mutations that turn today's
   seven gates red.
2. **A known-timing test program** — the Spectrum community's contention test suites report
   measured T-state counts for the machine to print. That is the closest thing to an oracle
   available, and it is a real one: a number to compare, not a picture to squint at.

   **Written, and it landed green on its first run.** `crates/spectrum/tests/timing_oracle.rs`
   runs Richard Butler's 48K timing test suite on the machine and compares 68 hardware-measured
   numbers. The corpus, why it is not circular, what its green establishes and — the part that
   matters more — the three things it leaves ungraded are in *The timing oracle*, below. The
   headline: **`FIRST_CONTENDED_T_STATE = 14335` survived**, and it is the only value in
   ±2 that does.
3. **A snapshot round trip.** Load a `.z80`, run zero frames, save, compare. Byte-identical or the
   state model is wrong. Cheap, deterministic, and independent of timing.
4. **Frame-hash regression.** Render frame N of a fixed program and hash it. Does not prove
   correctness — proves *change*, which is what catches a regression once something works.
5. **Known-demanding software.** Multicolour demos, Nirvana-engine programs, a tape loader. This is
   observation and must be labelled as such.

**Write down which of these covers what, and — as with the CPU — which properties nothing covers.**
The M3 lesson stands: reporting the absence of a distinguishing test as evidence of correctness is
the failure this project keeps catching.

## The timing oracle

Verification item 2, taken. This section says what was surveyed, what was chosen, what it proves,
and what it does not — in that order, because the last part is the one that gets skipped.

### What exists, and what tier each candidate reaches

Ranked by what they could actually settle for a 48K, using this project's own vocabulary.

| Candidate | What it is | Self-checking? | Tier |
|---|---|---|---|
| **[ZXSpectrum4.net 48K timing tests](https://www.zxspectrum4.net/op_timing.php)** (Richard Butler, 2010) | A `.z80` snapshot: 34 instruction groups run in a loop until the frame interrupt, each in contended and uncontended memory, against two tables of expected `R` / loop-count / `SP` carried inside the file | **Yes** — three numbers per group, compared against the file's own tables | **Measured.** The authors state *"In order to get the correct results we ran the tests on real Spectrums"*, and 28 genuine machines from 9 independent submitters are on record, 25 of which classify into the file's two tables |
| [MrKWatkins/EmulatorTestSuites](https://github.com/MrKWatkins/EmulatorTestSuites) | The same suite repackaged as a C# NuGet library with a harness interface, GPL-3.0 | Yes — it *is* the suite above | Same tier; it is a **redistribution**, not a second opinion. This is where the file is fetched from, because the original download link 404s |
| [Chris Smith, *The ZX Spectrum ULA*](http://www.zxdesign.info) (2010) | Silicon-level reverse engineering of the ULA, with the contention mechanism derived from the actual gate structure | No — it is a **book** | Would be **proven**, and is unusable as a gate. `zxdesign.info` also refused connection on both HTTP and HTTPS throughout this work |
| [Sinclair Wiki, *Contended memory*](https://sinclair.wiki.zxnet.co.uk/wiki/Contended_memory) | The community's reference page: *"14335 or 14336"*, pattern `6,5,4,3,2,1,0,0` | No | **Derived**, and it is *this project's own source*. Grading against it would be circular by construction |
| ULA/floating-bus tests (*Floating Spy*, *ULA 48 Simple Test*, *Test Program*), catalogued in [redcode/ZXSpectrum's test list](https://github.com/redcode/ZXSpectrum/wiki/Tests) | Mostly on-screen patterns | Partly | **Observed**, and most need a floating bus this machine does not model |

The ZXSpectrum4.net suite is the only candidate that is simultaneously runnable, self-checking,
48K, and anchored to hardware. It is the one used.

### The circularity check, and how it was made

**A community test suite is normally somebody else's derivation, and the worst available outcome
here would be an oracle whose expected values came from an emulator** — the hole would look closed
while nothing had closed. The author of this suite writes his own emulator, and the page says in
so many words that *"the emulator works perfectly and passes all the tests"*. Read alone, that is
exactly the shape of a circular oracle.

Three things say it is not, and none of them requires taking the author's word:

- The expectations are attributed to hardware directly — *"In order to get the correct results we
  ran the tests on real Spectrums"* — and the results database is hardware-only by policy:
  *"Only submit results from genuine hardware no emulators!"*.
- [Twenty-eight machines](https://www.zxspectrum4.net/downloads/spectrum48k_timing_results.htm) are
  on that page, submitted by **nine** named people (Miguel Angel Rodriguez Jodar 10, José Leandro
  5, Richard Butler 4, Hernán Álvarez 2, Arda Erdikmen 2, Tihomir Šantek 2, Mark Woodmass, Jaime
  Tejedor, "micky"), with ULA and CPU part numbers recorded.
- **The file carries two tables, and the real machines sort into exactly those two classes** —
  **17 report `TYPE1`, 8 report `TYPE2`**, across board issues 2, 3, 3B, 4A, 4B and 6A. Only three
  entries classify as neither: two Inves Spanish clones, which have no contended memory at all, and
  one issue 1 board recorded as returning zeros. A table fitted to a single emulator has no reason
  to predict a second class of machine, and no reason for twenty-five real machines to sort cleanly
  into the two.

  > These counts were **parsed** out of the results table, not read off it. A first pass by eye
  > gave "25 machines, 8 submitters, issues 1–6A", and all three were wrong: 25 is the number that
  > *classifies*, not the number submitted; there is a ninth submitter; and issue 1 appears only
  > among the unclassified. Three wrong numbers in one sentence, from a table small enough to read,
  > is why the parse exists.
  >
  > **Two further readings were taken during M7's oracle survey. Both disagree with the parse, both
  > disagree with each other, and the parse stands.**
  >
  > | Reading | Machines | Submitters | `TYPE1` | `TYPE2` | Other |
  > |---|---|---|---|---|---|
  > | **the parse, above** | **28** | **9** | **17** | **8** | 3 |
  > | M7 survey, direct fetch | 26 | 9 | 14 | 9 | 2 |
  > | M7 survey, second reader | 24 | 9 | 10 | 9 | 2 |
  >
  > **Only the nine submitters is stable.** The first new reading is not even internally consistent
  > — 14 + 9 + 2 = 25 against its own 26 — and the second accounts for 21 of the 24 it claims.
  >
  > **This is recorded rather than acted on, and the distinction matters.** Both new readings are
  > eyeball readings of an HTML table, which is exactly the method that produced the three wrong
  > numbers in the paragraph above; a parse beat it then and outranks it now. Two weak readings
  > disagreeing with a strong one — *and with each other* — is evidence about the readings, not
  > about the table. **What would settle it is re-running the parse.** If that ever disagrees with
  > 17/8, *that* is the finding and this note is where it should land.
  >
  > What none of the three disturbs is the **argument**: two expected-result tables, a couple of
  > dozen genuine machines, nine independent submitters, sorting into the two classes with a small
  > unclassified remainder. The non-circularity claim rests on that shape, and the shape is the one
  > thing every reading agrees on.

That is corroboration by independent hardware, which is what moves this from *derived* to
**measured**. It is not *proven*: nobody has re-derived these numbers from the ULA's gates, and the
suite's own coverage is what it is.

### The corpus

| | |
|---|---|
| File | `testdata/timing/timing_tests_48k_v1.0.z80` |
| Bytes | 10883 |
| SHA-256 | `1e66230a7b23737294f35d2778b8384ce3f81412b98883d35e564091377382af` |
| Format | `.z80` **version 2**, 48K (additional header 23, hardware mode 0), entry `PC = 0xC000` |
| Origin | Richard Butler / ZXSpectrum4.net, 2010. Fetched from the [MrKWatkins mirror](https://github.com/MrKWatkins/EmulatorTestSuites) (GPL-3.0) because `zxspectrum4.net/downloads/timing_tests/` now 404s |
| Redistributed here | **No.** Gitignored and fetched on demand, exactly like the FUSE vectors, the `zex` exercisers and the third-party snapshots |

```sh
mkdir -p testdata/timing
curl -fSL -o testdata/timing/timing_tests_48k_v1.0.z80 \
  https://raw.githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.EmulatorTestSuites.ZXSpectrum/Timing/timing_tests_48k_v1.0.z80
shasum -a 256 testdata/timing/timing_tests_48k_v1.0.z80
```

Absence goes through `crates/testsupport` unchanged — present it runs, absent the gate **fails**
naming the fetch, absent with `ZX_CORPUS_ALLOW_MISSING` it skips, and that opt-out is **refused
under `CI`**. All five rows including the obsolete-spelling refusal were exercised on 2026-09-01 by
moving the directory aside; this table is checked rather than asserted, because
[`testdata/README.md`](../testdata/README.md) has carried a fictional one before.

> **`testdata/README.md` does not yet have a section for this corpus**, and that file is owned
> elsewhere. Until it does, the fetch command lives here, in the gate's own module documentation,
> and in the failure message the gate prints — which is the one a developer actually reads.

### What it does, and why it can see what the other gates cannot

`contention_magnitude.rs`, `io_contention.rs`, `block_contention.rs` and
`prefix_chain_contention.rs` are exact, hand-derived, and **all of them position the machine by
`FIRST_CONTENDED_T_STATE`**, so all of them survive it being wrong — each says so in its own
header. The delay pattern and the four-case I/O rule are in the same position: transcribed from the
community, so a gate written from them cannot discover that they are wrong.

This gate positions the machine by the **interrupt**, and then lets a program the Spectrum itself
runs count how much work fits in a frame. Nothing in it is expressed in terms of 14335, the
pattern, or the four cases; every expected number is read out of the corpus.

**Result, 2026-09-01: 3 tests, 68 graded rows, green on the first run.** The machine reproduces
`TYPE1 (Early)` — the majority class, 17 of the 25 machines that classify — with **0 of 68**
disagreements. Groups 35–37 are excluded and named: they read the floating bus, which this machine
does not model.

One of those 68 is worth naming. **Group 27 is `BIT (HL); RES (HL); SET (HL); INC (HL); DEC (HL)`,
and it is graded in contended memory over 143 iterations.** That is the read-modify-write family
whose price two derivations disagreed about at M5 — 26 against 30 for `INC (HL)` at phase 0 — and
it now agrees with hardware, integrated over a frame. It does not adjudicate 26 against 30 directly,
because the number it compares is a loop count and not one instruction's cost; what it does is put
the mechanism those figures come from in front of an external number for the first time. The two
mutations that break that mechanism — `Ula::fetch` charging a 3-T read, and internal cycles never
contending — are red here at 29 and 34 of 68.

### The green was then proven able to fail

A green first run is exactly what this project distrusts, so thirteen mutations were run against
the gate. **Every one had its landing verified before its verdict was trusted** — occurrence count
asserted, file re-read after the write, and restored from held bytes with the SHA-256 compared.

| Mutation | Oracle |
|---|---|
| `FIRST_CONTENDED_T_STATE` 14335 → 14333 | **RED**, 13 of 68 |
| → 14334 | **RED**, 8 of 68 |
| → 14336 | **RED**, 7 of 68 — and **64 of 68 against the *other* table**, so it is not "the late machine", it is neither machine |
| → 14337 | **RED**, 7 of 68 |
| → 14361 (the 128's figure) | **RED**, 23 of 68 |
| `DELAY_PATTERN` last slot `0` → `1` | **RED**, 14 of 68 |
| `DELAY_PATTERN` all zero — contention removed | **RED**, 38 of 68 |
| `Ula::fetch` charges a 3-T read instead of an M1 cycle | **RED**, 29 of 68 |
| Standalone internal cycles never contend | **RED**, 34 of 68 |
| I/O case `N:1, C:3` → no stall | **RED**, 4 of 68 |
| I/O case `C:1,C:1,C:1,C:1` — fourth term dropped | **GREEN** |
| `INTERRUPT_T_STATES` 32 → 24 | **GREEN** |
| Interrupt asserted one T-state later **and** the window moved to 14336 | **GREEN** |

**14335 is a unique optimum over ±2.** That is the finding this project has been waiting two
milestones for, and it is the first time any number in `timing.rs` has been compared against a
measurement taken outside this project. The frame length has always been checked against a
*published* figure, which is a transcription, not an oracle.

### What the green establishes — precisely, because the obvious reading is too strong

The last mutation in that table is the important one and it is deliberately last. Moving the
interrupt one T-state later *and* the contention window one T-state later leaves the gate **green**.
So the oracle grades the **interval** — the distance from `/INT` being asserted to the first
contended T-state — and not the constant on its own.

> **What is established is that the first contended T-state falls exactly 14335 T-states after
> `/INT`.** `FIRST_CONTENDED_T_STATE = 14335` is correct *given* that this machine asserts `/INT`
> at frame T-state 0. A machine that asserted it one T-state later and set the window to 14336
> would be equally right, and this corpus cannot tell them apart. The constant is now anchored;
> the **origin of the frame** is still a convention.

The three open rows this bears on in `STATUS.md` change status, and only one of them closes
cleanly:

- **`FIRST_CONTENDED_T_STATE` has no oracle** — **closed**, with the qualification above.
- **The 64-line pre-display count is taken on trust** — **closed as a consequence, not
  separately.** 64 × 224 − 1 = 14335 is the derivation, and its *product* is now measured. Nothing
  here observes the 64 and the 224 independently, so a compensating pair of errors would survive.
  That is a much smaller gap than an unmeasured product, and it is not nothing.
- **The interrupt window's *length* is pinned against drift, never measured** — **still open, and
  now demonstrated rather than asserted.** Cutting it from 32 T-states to 24 leaves this gate
  green. The test program only needs the interrupt to be *accepted* near the top of the frame; how
  long the line is held is invisible to it.

### What it does not grade

Named rather than inferred, per the instruction at the top of the verification plan.

- **The floating bus.** Groups 35–37 exist, need it, and are excluded by name. They are also the
  suite's *port* I/O timing groups, which is why the next row is what it is.
- **The four-case I/O rule's fourth term.** Dropping it entirely leaves the gate green: a contended
  address that is *not* the ULA's port (`C:1,C:1,C:1,C:1`) is not reached by groups 1–34.
  `io_contention.rs` remains the only gate on that case, and it is hand-derived from the published
  rule. The `N:1, C:3` case *is* graded here, at 4 of 68.
- **`DEC`, the `IY` forms, and every instruction-level figure.** This gate integrates a frame; it
  cannot attribute a discrepancy to an instruction. `contention_magnitude.rs` is still what does
  that, and the two are complementary rather than redundant — one is exact and unanchored, the
  other is anchored and coarse.
- **Anything about a 128.** The site states that 128K and +2 (Grey) editions of these tests
  exist. Neither was fetched or verified here; M7 should.

  > **M7 took it, and the answer is no — with the shortfall on the decisive leg rather than on
  > availability.** The full survey is in [`M7.md`](M7.md) Decision 8; the two findings that belong
  > here:
  >
  > - **The 128 file could not be obtained.** `zxspectrum4.net/downloads/timing_tests/` now
  >   **302-redirects to the site root** — the whole download directory is gone, not merely the one
  >   file *The corpus* already records as 404ing — and the MrKWatkins mirror carries **only**
  >   `timing_tests_48k_v1.0.z80`.
  > - **There is no 128 results database, and that is the part looking harder cannot fix.** The
  >   site's only results page is `spectrum48k_timing_results.htm`. So the argument that moved
  >   *this* corpus from *derived* to **measured** — 25 of 28 real machines from 9 submitters
  >   sorting into the file's two expected classes — **has no 128 counterpart**, and a recovered
  >   128 file would still rest on the author's own account of his own process, which is precisely
  >   what that third leg exists to avoid relying on. `M7.md` argues the gap may be structural
  >   rather than an oversight: the two tables exist *because* real 48Ks come in two contention
  >   classes, and a machine without such a split gives its suite nothing to predict.
  >
  > So the 128's first-contended figure, its 228-T-state line and its delay pattern remain
  > **transcribed from a derived source** — the World of Spectrum *128K Technical Information*
  > page. That is the state the 48K was in until this section was written, and saying so plainly
  > matters, because the nearest hazard is letting the 48K's new oracle lend the 128 an authority
  > it has not earned.

### The one T-state is not a property of a board, which is a finding in itself

The suite's own page records that a **cold** machine can report `TYPE2 (Late)` and then report
`TYPE1 (Early)` once it has warmed up — they rewrote the detection to stream its numbers on screen
so the wavering could be watched. The hardware table bears this out from the other direction: board
issue does not predict type. **Issue 3B, 4B and 6A boards appear in both classes**, and Mark
Woodmass's issue 2 board with a replacement issue-3 ULA reports `TYPE1`.

So *"which of 14335 and 14336 is correct"* may not be a question with a single hardware answer, and
`timing.rs`'s note that *"an issue 2 machine is one T-state earlier"* is the wrong axis: the
community's early/late split is not issue 2 versus issue 3. Choosing `TYPE1` is choosing the
majority behaviour of real machines, which is the right default and is now a *decision* rather than
an accident.

## Milestones

| | Goal | Gate |
|---|---|---|
| **M5** | 48K: paged memory, ULA screen, keyboard, 50 Hz interrupt | **the gates in `crates/spectrum/tests/`** — boot **and the frame it lands on**, the 50 Hz interrupt line, the 40 × 8 keyboard matrix, ROM write protection through all three write paths, contention magnitude (the whole read-modify-write family), contention phase, and the four-case I/O rule through a real `Cpu<Ula>`. **Count them, do not quote a number:** `ls -1 crates/spectrum/tests/*.rs \| wc -l` |
| M6 | `.z80`/`.sna` snapshots, `.tap` tape | **T1 + T2 + T3** ([`M6.md`](M6.md) Decision 8) — the round trips, hand-transcribed vectors and hostile-input sweeps; the real ROM's `LD-BYTES` loading a synthetic tape through the `EAR` bit; and **a program we wrote, loaded from tape by the ROM and executed**. *"A real game runs"* is **T4**, observation, and cannot be automated in a repository that may not carry games |
| M7 | 128: paging, second ROM, AY-3-8912, per-bank contention | **T1 + T2 + T3** ([`M7.md`](M7.md) Decision 5) — the paging port exhaustively, the AY's checkable state machine, the round trips; the real 128 ROMs reaching their own menu and reaching the 48K message through the second ROM; and **a program we wrote that a 48K gets demonstrably wrong**. *"128-only software runs"* is **T4** |
| M8 | WASM + macroquad | playable from a URL |

> **The M5 row said *"boots to the copyright message"*, and that is corrected here rather than
> quietly widened.** It was the whole gate at commit `2157331` — and this document's own
> verification plan, three sections above, records what M5 then measured about it: the boot check
> grades the memory map's read side and the screen, **four of five mutations were already red and
> exactly one survived**, and at that
> commit **nothing ran it at all**, because it was an example. `bf8414d` shipped six real gates
> and ten mutations proven to turn them red, and `io_contention.rs` has since made it seven —
> closing one of the *Still ungraded* rows in the process. This was the last line in the file
> still implying
> the single-gate picture, which is precisely how a corrected finding gets un-corrected: the
> narrative section is rewritten and the summary table, which is what people actually read, is
> not. [`STATUS.md`](STATUS.md)'s M5 section carries the gate table and the mutation verdicts.

> **Three further corrections to that table, and the first is the one worth reading twice.**
>
> **1. The block above said *"five mutations left it green"* — and it was written *after* that
> figure was known wrong.** The measurement is four sections above it, in this same file: re-run at
> `2157331` against the pre-gate lib target, **four of the five were already red** (5, 7, 1 and 13
> failing tests) and **the contention phase is the only survivor**. So a passage whose entire job
> was correcting one error carried a second, uncorrected one through the correction. That is not a
> new failure class — it is [`STATUS.md`](STATUS.md)'s *"a derived figure repeated across documents
> acquires authority it never earned"*, arriving inside its own remedy. **The narrative was fixed
> and the summary was not, which is the exact sentence the block itself ends on.**
>
> **2. The M5 row said *"seven gates"*, and a bare integer in another crate's test directory is a
> claim that rots on someone else's commit.** It was seven when written. `ls -1
> crates/spectrum/tests/*.rs | wc -l` returned **13** and then **14** within minutes during a cold
> review as agents landed tests; **16** when this correction was begun; and **19** about twenty
> minutes later, before it was finished (all 2026-09-01). **The integer changed twice while the
> sentence correcting it was being written**, which is a better argument for the rule than the rule
> is. The row now carries the command instead of an answer. Three stale integers have already been found in
> this document set — the 6.6 % bank index, the 329×/145× throughput pair, and the bounds-check
> count that measured 7, 10, 11 and 15 under four probes — and the fix in every case was the same:
> **publish the command, not the number.** [`ARCHITECTURE.md`](ARCHITECTURE.md)'s *Measured*
> section makes the same point at length and its own text names the temptation: *"a live count of
> files in another crate's test directory is a claim that rots on someone else's commit."*
>
> **3. The M6 and M7 rows said *"a real game runs"* and *"128-only software runs"*.** Both are
> tier **T4** — observation, manual, corpus-dependent, and unautomatable in a repository that may
> not carry games. [`M6.md`](M6.md) Decision 8 settled M6's real gate as **T1 + T2 + T3** and its
> implementer flagged that *"`MACHINE.md`'s and `ARCHITECTURE.md`'s milestone tables still carry
> the old wording — neither file is the implementer's to edit"*. This is that edit, for M6, and
> the same edit made for M7 at the same time so the next milestone does not inherit the same
> mismatch. **`ARCHITECTURE.md`'s copy of the table is owned elsewhere and still carries the old
> wording for all three rows.**

Contention lands at M5 for the 48K and is extended at M7. It is not deferred: the entire `tick`
contract exists for it, and a machine built without it would have to be rebuilt rather than
extended.

## What the CPU already gives the machine

- `Cpu::interrupt(data) -> u32` and `nmi()`. `interrupt` **declines** — returns 0, changes
  nothing — while `iff1` is clear or the `EI` window is open, so the acceptance rule lives in one
  place and the machine does not reimplement it.
- `Cpu::state()` / `set_state()` over a `CpuState` carrying everything a `.z80` snapshot needs,
  including `wz` and `q`. This is also the **only** route to `PC`.
- `Cpu::fault()` — a mode-0 device byte with no defined meaning is the one genuine runtime fault.
- `Cpu::ei_pending()` — why an interrupt offer was declined, without reimplementing the rule.
- `Cpu::bus()` / `bus_mut()` — how a machine reaches its own memory and peripherals after the CPU
  has taken ownership of them.

> **Correction.** This list said `Cpu::pc()` for cheap loop checks. **There is no such method and
> there never was one** — `Registers::pc` is `pub(crate)`, because the CPU owns its register file
> and the whole point of the crate boundary is that the machine cannot reach into it. The line was
> written from the design's intent rather than from the crate, and nothing checked it.
>
> What exists is `cpu.state().pc`, which builds an entire `CpuState` — twenty fields, fourteen of
> them read back out of the `[u8; 26]` array — every time it is called. That is correct and it is what
> `spectrum` uses, wrapped as `Spectrum::cpu_state()`. It is a **snapshot**, not the cheap
> per-iteration read the old line implied, and a frame loop that wants to watch `PC` should know
> that before putting it inside one.

## What it did not give — the M1-fetch question, answered wrong and then measured

**Resolved at M5.** This section used to close the document with an argument, and the argument was
wrong. It is kept verbatim, because how it was wrong is more useful than the correction:

> The machine **cannot distinguish an M1 opcode fetch from an operand read** — both arrive as
> `Bus::read`. **Contention is unaffected, since the delay depends on address and `t mod 8` and
> the machine has both.** It matters only for a debugger or a precise floating-bus model, and the
> fix is non-breaking whenever it is wanted: a defaulted
> `fn fetch(&mut self, addr) -> u8 { self.read(addr) }`.

**The bolded sentence is false.** Its first clause is a true fact about contention. The conclusion
does not follow from it, and M5 produced the counter-example.

`LD A,B` is one M1 cycle: four T-states. The read-modify half of `INC (HL)` is a three-T-state
memory read followed by one internal cycle. Routed through a single `read`, the two emit
**byte-identical call streams** — `read(addr)` followed by four `tick(addr)`: same address, same
order, same count. One contention point is correct for the first and **two** for the second, and no
amount of address-and-`t mod 8` reasoning separates them, because the machine never received the
input that distinguishes them.

**The defect is not the arithmetic, it is the question that was never asked.** The argument reasoned
from *what contention depends on* — address and phase, both of which the machine has — and never
asked *whether the machine can observe the quantity being charged*, which is the number of cycles,
not their addresses. That is the same shape as the failure `STATUS.md` records at M3: reporting the
absence of a distinguishing test as evidence of correctness. Here it was the absence of a
distinguishing **signal**, argued into a claim that the signal did not matter.

### What the machine did instead, and what it cost

`crates/spectrum/src/machine_cycle.rs` reconstructed cycle boundaries from the stream alone. A
transfer opened a cycle; any `tick` outside such a window was a standalone internal cycle; and the
fourth tick after a `read` was **deferred**, because an opcode fetch is exactly four T-states. A
*fifth* tick at the same address proved the run was a three-T-state read followed by internal
cycles, and the deferred T-state was then charged at the frame position it actually occupied. If no
fifth tick arrived it was dropped, which is right for an M1 fetch.

The residual was exact, and pinned by a test rather than hidden: **one contention point — 0 to 6
T-states, and only on a contended address — per execution of an instruction that performs exactly
one internal cycle at the address it has just read.** That is the read-modify-write family:
`INC`/`DEC (HL)` and `(IX+d)`/`(IY+d)`, the `CB` bit/rotate/shift group on memory, and
`EX (SP),HL`/`IX`/`IY`. Longer internal runs resolved, and so did every opcode fetch.

> **"0 to 6" is the *isolated stall*, not the error — read it with the correction two sections
> below before acting on it.** The sentence above is left as it was recorded, because this project
> corrects loudly rather than deleting quietly and because a reader who has already acted on that
> number needs to find out. But 0–6 is what `delay(t)` charges a single cycle *taken alone*, and
> that is a different quantity from what the heuristic actually got wrong. **The net observable
> error on `INC (HL)`, swept over all 448 start positions, was 0 or 1 T-state and never more** —
> dropping a stall opens the following cycle earlier, where the pattern charges most of it
> straight back. Conflating the two is not cosmetic: it is what made 30 a plausible answer for
> `INC (HL)`'s contended cost at phase 0, where the machine cycles give 26. See *The machine has
> taken it, and the deferral is retired*, below, and *A missing stall cannot be added to a total*
> in [`STATUS.md`](STATUS.md).

### The fix landed in the CPU

`crates/z80/src/bus.rs` carries the sixth method, defaulted exactly as the quoted paragraph
proposed, and **every M1 opcode fetch in the core routes through it** — the un-prefixed opcode, each
`DD`/`FD`/`CB`/`ED` prefix byte, and the opcode on the `CB` and `ED` pages. `DDCB`/`FDCB` operand
bytes do not: they are ordinary memory reads, which is why `R` advances twice across those four
bytes. Two further neighbours are deliberately not fetches — a halted CPU's discarded byte **is**
one, an interrupt acknowledge is **not** — and because those are hardware rules they are stated in
[`Z80-REFERENCE.md`](Z80-REFERENCE.md), together with the invariant they qualify.

### The machine has taken it, and the deferral is retired

`Ula` implements `fetch`, and `machine_cycle.rs` is **deleted**. With every cycle's length known the
moment it opens — four T-states for an M1 fetch or a port access, three for a memory read or write —
there is nothing left to reconstruct. What survives is a single `u8` on `Ula`: the count of T-states
the open cycle has already paid contention for, spent one per `tick`, and a tick arriving with it at
zero is a standalone internal cycle that contends on its own account. The four-case I/O contention
rule is untouched — it was never part of the heuristic and reads the clock, not the cycle.

Two things about the arithmetic are worth recording, because two independent readers got the second
one wrong before it was checked against the hardware.

**`INC (HL)` decomposes into four machine cycles, `pc:4, hl:3, hl:1, hl:3`** — verified by a
recording bus driven from a real `Cpu`, not read off the source. Each contends once, at the address
it drives, at the moment it opens. Starting on `FIRST_CONTENDED_T_STATE`:

| cycle | at | stall | ends |
|---|---|---|---|
| fetch `0x4000` | +0 | `delay(0)` = 6 | +10 |
| read `0x4100` | +10 | `delay(10)` = 4 | +17 |
| internal `0x4100` | +17 | `delay(17)` = 5 | +23 |
| write `0x4100` | +23 | `delay(23)` = 0 | +26 |

**26 T-states, and 19 at phase 7.** The heuristic produced 25 and 18.

**The lost *quantity* was one contention point; the visible *error* was one T-state, not five.**
Those are different numbers and the difference is the whole subtlety. Dropping the stall at +17 did
not simply subtract 5: it opened the write four T-states early, at +18, where the pattern stalls 4
rather than 0, so four of the five came straight back. **A missing stall cannot be added to a total,
because every stall shifts the ones after it** — the same property `ula.rs` already spells out for
the four-case I/O pattern, applied one cycle at a time. Anyone re-deriving this from a claimed
residual rather than from the machine cycles will land on 30, and 30 is wrong.

Phase 0 is not a lucky case: **swept over all 448 start positions, the heuristic's total for
`INC (HL)` was wrong by 0 or 1 T-state and never more.** So the recorded residual's "0 to 6" is not
an error bound at any phase — it is the isolated stall, and the two quantities should never be
written as one number. The sweep is carried here from [`STATUS.md`](STATUS.md) and
[`CHANGELOG.md`](../CHANGELOG.md), which both record it; **it was not re-derived in this pass**,
and it is no longer reproducible from the crate, because comparing the two totals requires the
heuristic that `machine_cycle.rs` held and that file is deleted. Whoever wants it independent must
re-implement the deferral against the current `Ula` and re-sweep.

The change is a correctness fix and is justified on that ground alone; `ARCHITECTURE.md`'s position
that performance is a non-goal is not in tension with it. Measured against the real ROM, the boot
run reaches the copyright message on **frame 87 either way** — but on 658,144 instructions rather
than 658,277, so the changed path is genuinely exercised during boot and the effect is 0.02 %, far
below what moves a frame boundary. (Removing contention *entirely* changes instructions-per-frame by
roughly 20 % and does move it, 87 → 85.)
