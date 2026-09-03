# Status

A living record of where the project actually is — what is proven, what is measured, what is
open. Updated as work lands, not once at the start.

**Last updated:** 2026-09-01, closing M5 — and re-opened the same day, twice: once by
`tests/timing_oracle.rs`, which settled the item that had headed *Still ungraded* for two
milestones, and once by `tests/frame_boundary.rs` and `tests/block_interrupt.rs`, which closed two
properties of the **machine** that nothing had ever driven. **Then M6 merged** (`0d3e7ef`), and its
own eight open rows were added on the same date — see the M6 section, immediately below. **Then on
2026-09-02 `tests/tzx_turbo_load.rs` narrowed three of those eight and closed none**: a turbo
loader is graded, a turbo *game* still is not, and the difference between those two sentences is
what the M6 rows now carry. A fourth — the `EAR` sampling point — had its settling condition
*attempted* and is deliberately left as it stood, because a turbo loader running without failing
for that reason is an absence of a distinguishing failure and this document exists to refuse
reading one of those as evidence.

> **Also on 2026-09-02: the 128 was graded against hardware for the first time, and it was wrong.**
> `tests/timing_oracle.rs` now runs the 128 edition of Richard Butler's suite alongside the 48K
> one. The first run was red on **62 of 70** rows: `Timing::SPECTRUM_128` carried
> `first_contended_t_state: 14361` where the hardware wants **14362**, and a 32-T-state interrupt
> window where it wants one in **`33..=43`**. Both are corrected and all 71 of the file's rows are
> bit-exact. **The corpus had been on disk, hashed and documented, since 2026-09-01, read by
> nothing** — see *A corpus nothing reads is not evidence* near the end of this document, which is
> the part that generalises. `docs/MACHINE.md` carries the measurement.

> **M7's memory half boots, and `crates/frontend` enters this register for the first time.**
> Added 2026-09-01. The 128 pages through `0x7FFD`, carries both ROMs, contends per bank, has the
> shadow screen, and `Timing` is a value rather than a constant. **Verified here by running it, not
> by relay:** `zx-shot`, extended in the same pass to build a 128 through the same public API a
> consumer uses, photographs the menu with all five entries drawn; three `CAPS SHIFT`+`6` presses
> through the real keymap move the highlight to *48 BASIC*; and selecting it reaches
> `© 1982 Sinclair Research Ltd`. **The year changing is what makes that a claim about which ROM is
> executing** rather than about something being drawn — `128-0.rom` carries *1986* and `128-1.rom`
> carries *1982*, checked by offset in the committed bytes. Both 128 ROMs were gitignored and are
> now named in `.gitignore`, with their sizes, SHA-1s, CRC-32s and SHA-256s recorded in
> `testdata/README.md` **from the committed bytes rather than transcribed**. M7's own write-up is
> its milestone's to make; this note exists so the register is not last to hear again.
>
> The `crates/frontend` rows are in [the authoritative register](#open--the-authoritative-register)
> and the reason they were missing for six milestones is in the note above them.

> **M6 merged before this document said anything about it, which is the defect this document is
> named after.** `STATUS.md` records that *"a milestone is not done until it has written to the
> register"*, having watched commit `2157331` ship the whole M5 machine while touching no file under
> `docs/`. M6 did better — its design document carries two long implementer reports — but a design
> document is not the register either: it is never revisited when an item closes, so an item
> recorded only there is invisible the moment it stops being true. The M6 section below is that
> write-up, taken from the implementers' reports and **not softened**.

> **Then the three rows nobody had re-audited for three milestones were audited, and all three
> were wrong.** The register carried a standing note — repeated at M5 and again at M6 — that
> *resolved-target refactor*, *`WZ` / MEMPTR* and *contention within a cycle* had been carried
> forward unchecked. The fourth pass checked them against the crate. **All three left the Open
> table**: two were closed by code that had landed milestones earlier, one was closed and replaced
> by a narrower row, and **two of the three were not stale but false** — asserting, with the
> register's authority, things the code had stopped doing. The audit and what it implies for this
> process are under [Open — the authoritative register](#open--the-authoritative-register); the
> evidence is in the Closed table beneath it. The same pass corrected four test doc-comments that
> still said the timing oracle did not exist, and listed the copies it found in files it did not
> own rather than editing them.

> **The header said *"during M3"* while the document's top section was M4 and M5 had already
> landed.** It is corrected here rather than silently bumped, because the gap is the symptom of a
> real defect and not a typo: commit `2157331` shipped the whole M5 machine and **touched no file
> under `docs/`**. Its findings — including four open questions — went into a commit message and a
> crate doc comment instead. A commit message is not a register: it is never updated when an item
> closes, so an item recorded there is invisible the moment it stops being true. `ARCHITECTURE.md`
> already says the register lives here **and only here**; the corollary it did not spell out is that
> a milestone is not done until it has written to the register.

---

## Milestone M6 — snapshots, and a tape the real ROM loads

**Merged as PR #6 (`0d3e7ef`).** The gate is **T1 + T2 + T3**, and *"a real game runs"* — which is
what `MACHINE.md`'s milestone table asked for when M6 was designed — is **T4**, observation, and it
runs nowhere. That is not a shortfall discovered late: `M6.md` Decision 8 argued it before any code
was written, on the ground this document supplies three times over — **a gate whose corpus is
absent by default is a gate that runs nowhere**, and a repository that may not carry games cannot
carry that corpus. The milestone did not weaken its gate to fit; it moved the gate to the part that
can be committed and then wrote down, precisely, what the residue is. That residue is the
*Still ungraded* list below, and it is the milestone's deliverable rather than its apology.

**The register had not caught up.** This section is that catch-up: what M6 closed, what it opened,
and two findings that were measurements rather than opinions. The rows it opens are in the
[authoritative register](#open--the-authoritative-register), in the M1 section, because there is
one open register in this project — a second table in the newest section is exactly the duplication
that let two documents disagree about four facts in one session.

### What M6 closed

**The snapshot round trips, and the measured statement of what breaks their symmetry.** Three round
trips exist and each grades a different thing — R1 `snapshot(restore(s)) == s` over a `Snapshot`
built in code, R2 `parse(write(s)) == s`, R3 `write(parse(f)) == f` over a file in our own canonical
encoding. `M6.md` Decision 7 predicted that **all three are blind to a symmetric error**, and the
prediction is now a measurement rather than an argument. Two symmetric mutations, each verified
present in the file before its verdict was trusted — occurrence count asserted before the write, the
file re-read after, and the restore made from a byte-level backup and `diff`ed rather than from
`git checkout --`:

**Both rows are M6's, and so is every range in them.** They were taken against the gates as M6 left
them — the milestone that merged as `0d3e7ef` on 2026-09-01 — and the `.z80` frame-position counter
was a 48K-only function then: its sweep covered that machine's 69888 positions and no others, and
the six hand-worked positions are six **on a 48K**. M7 made the quarter a function of the model and
widened both halves — each machine swept over its own frame, 69888 **and** 70908; six hand-worked
positions **per machine**; and an assertion that the two machines encode frame position 0
differently, which a loop over a single model structurally cannot contain. **Nothing records a
mutation taken since.** So the verdicts below are left at the gate they were taken on rather than
restated against the wider one: a mutation verdict carries its gate, and re-stating one against a
range it never ran on asserts something nobody measured, which is worse than a stale number.
`crates/spectrum/src/snapshot/z80.rs`'s `encode_t_states` draws the same boundary from the code's
side, and `docs/M6.md`'s own copy of this table carries the same framing.

| Symmetric mutation | Every round trip | What went red |
|---|---|---|
| `HL` and `DE` permuted in the parser **and** the writer | **green** — R2, R3 **and** the third-party corpus sweep | the three hand-transcribed `.z80` vectors; the hand-built `.sna`/`.z80` cross-format pair; the **third-party** `fire.z80`/`fire.sna` pair |
| The v3 T-state high byte's origin shifted from 3 to 0, both directions | **green** — including the exhaustive sweep over **all 69888** frame positions | six positions derived by hand from the format description's own sentence; `libspectrum`'s independent expressions, transcribed; the transcribed version 3 vector |

Nine further mutations were run against the applier and the tape. **R1 and R3 are green on every one
of the nine.** So the closure is not "the round trips work"; it is the sharper statement they were
built to support: **only an expectation that owes nothing to the code under test sees a symmetric
error**, and a foreign file proves a field is *readable* rather than that our arithmetic on it is
right. One instrument the design did not anticipate turned out to matter as much as either — **a
`.z80` and a `.sna` that a third party saved from one machine.** `.sna` has no writer here (a `.sna`
writer must push `PC` onto the guest's stack and destroy two bytes of the RAM it is recording), so
it has no round trip at all, and that pair is its only external grading. It earned its place on its
first run by catching a wrong **expectation** about `SP` — the writer pushed, so a correct pop
restores the *same* `SP` the `.z80` carries, not one two higher. The parser was right; the test was
wrong; nothing built from our own code could have said so.

**The tape's signal-level model, gated through the real ROM.** `M6.md` Decision 4 rejected the ROM
trap — watch for `PC` at `0x0556` and inject the block — on the ground that the gate would then
grade the trap: it bypasses the ULA, the contention model, the frame clock, the interrupt window and
the port decoding, which is every part of the machine M5 could not grade. What shipped instead is a
pulse train read through bit 6 of port `0xFE`, and three things grade it:

- **T2** — the real ROM's own `LD-BYTES` loads a synthetic tape through the `EAR` bit, on the real
  machine, with only the committed ROM as corpus.
- **T3** — a program **we wrote**, stored as a `.tap`, loaded by that same ROM routine and then
  **executed**. It adds `0x1234` and `0x1111` and stores `0x2345` — a value the test asserts appears
  **nowhere in the tape's own bytes** — so *"the data arrived"* and *"it ran"* are two claims and
  not one. Everything in between is the machine: thousands of `IN A,($FE)` port cycles, each
  contended by the four-case I/O rule, each reading a level that is a function of the frame clock's
  absolute position.
- **`no_shortcut_exists_past_the_ear_bit`** — the same tape, the same stub, the same budget, motor
  **stopped**. The loader spins in `LD-START` waiting for an edge that never comes, nothing reaches
  the destination, and the machine is asserted to be still inside the ROM, which separates *waiting*
  from *wandered off*. Both otherwise look like an expired budget. This is the standing assertion
  Decision 4 asks for, so a shortcut cannot be added later and pass quietly.

**The parser's panic class, closed structurally rather than tested away.** The crate builds with
`panic = "abort"`, so a panic on a hostile file is not a recoverable error — it kills the process,
and `catch_unwind` is not available as a backstop. With `unsafe_code = "forbid"` there are three
panic sources a hostile file can reach, and each is closed by construction:

| Source | How it is closed | What enforces it |
|---|---|---|
| slice indexing | **there is no indexing anywhere in the module.** Every byte moves through `Reader` or `Writer`, whose slice operations are total by signature | `there_is_no_indexing_anywhere_in_the_snapshot_module`, a scanner over the production half of all **five** source files, listed rather than globbed *"because a file that quietly stopped being scanned would be indistinguishable from a file with nothing to find"* |
| explicit `panic!` / `unwrap` / `expect` / `todo!` / `unimplemented!` / `unreachable!` | none in the production half | `nothing_in_the_snapshot_module_can_panic_on_purpose`, six forbidden constructs |
| unbounded allocation | **no allocation is ever sized from the file.** Decompression fills a fixed `[u8; PAGE_SIZE]` through a write cursor; a write past the end is `PageOverrun`, not a `Vec` growing to whatever the file asked for | the type, and the hostile-input table as one test per row |

**The scanner has its own positive *and* negative cases, and that is what stops it being a
tautology.** `the_indexing_scanner_can_tell_an_index_from_an_array_type` requires five expressions
to be *found* — `self.banks[index]`, `bytes[..2]`, `value.to_le_bytes()[0]`, `grid[y][x]`,
`PAGE[0]` — and seven to be *ignored*, including `[u8; 2]`, `Box<[u8; PAGE_SIZE]>`, `#[derive(..)]`,
a commented-out index, and a doc link. Its own comment states the reason: *"the gate below is only
worth running if this function distinguishes the two… Without them it would be a scanner that finds
nothing, asserting that nothing is there."* That is this document's own recurring failure —
**a count of zero and an absence of the subject are the same observation** — anticipated at the
point where it would have bitten, instead of being catalogued afterwards.

**Structural impossibility and behavioural sweeps are not substitutes for each other**, and both
shipped: `tests/snapshot_hostile.rs` carries an exhaustive truncation sweep (`parse(&file[..k])` for
every `k`), a single-byte mutation sweep, two `proptest`s, and one case per row of `M6.md`'s
hostile-input table.

### Still ungraded — the deliverable, and it is not softened

`MACHINE.md` asks for this list explicitly, and this document records that *"reporting the absence
of a distinguishing test as evidence of correctness is the failure this project keeps catching"*.
Four items, and the labels on them are load-bearing.

- **Tape pulse timings are graded against *the ROM*, not against hardware — a different claim, and
  it must be labelled as one.** `tests/tape_rom_timings.rs` runs **`SA-BYTES`, the ROM's own tape
  writer**, on the real machine behind a recording bus, and compares the intervals between the edges
  it puts on the `MIC` line against our converter's train for the same bytes: **3305 half-periods,
  two implementations, no shared code, compared by value and in order**, with the ULA's contention
  of the writer's own `OUT`s subtracted so the comparison is an equality rather than a range. That
  is a real oracle and it runs on every `cargo test` — the ROM is code this project did not write,
  it is committed, and its timings *are* what a `.tap` means. **It is not a hardware measurement.**
  That 2168 / 667 / 735 / 855 / 1710 are what a real Spectrum emits is **unverified**: nobody here
  has put a scope on one, and no gate in this repository could. What the ROM oracle does establish
  is that our numbers agree with the numbers every loader was written against, which is a narrower
  and still valuable thing. It also has the ROM's own irregularities in it, asserted by value rather
  than absorbed into a tolerance — the byte-boundary interval is three T-states short, the
  first half-period after the sync is one T-state long, and the boundary before the parity byte is a
  third case at one short.
- **Contention during a load is exercised on every port read and graded by nothing.** The loader
  writes into contended RAM and performs thousands of contended port cycles, so the model is
  *running*; nothing asserts it. Measured rather than assumed: the mutation *"the tape stops seeing
  contention stalls"* reddens the two contention-vs-tape gates and **leaves the ROM load gates
  green**, because the ROM's loader does not depend on the contention it suffers. What would grade
  it is a loader whose margins are tight enough to fail when the stalls are wrong — which is a turbo
  loader, which needs `.tzx`, which is the row below. **Both have since arrived, and the row
  narrows rather than closes.** `tests/tzx_turbo_load.rs` runs a loader whose margins are exactly
  that tight: at 2.08× the ROM's data rate a bit's poll count lands *on* its threshold, and the
  four-case I/O rule moves the cost of a poll across the frame, so which way a bit reads depends on
  where in the frame it fell. The contention model is therefore no longer merely *exercised* by a
  tape gate — one gate's observed behaviour is produced by it. It is still not *asserted* against
  it: **no mutation of the stalls has been taken against that gate**, and until one is, this row is
  open on a sharper condition than it had.
- **T4 runs nowhere, and what it is the only tier for has narrowed.** It was recorded here as the
  only tier that grades a turbo loader; since 2026-09-02 that is a turbo **game** rather than a
  turbo loader, because `tests/tzx_turbo_load.rs` grades the format with a tape and a loader this
  project wrote — see the `.tzx` row below. It remains the only
  tier that grades a program written by somebody who did not know how this emulator works, and the
  only instrument that would grade our **arithmetic** on a format's fields rather than merely our
  ability to read them back — that last was measured, not assumed: under the symmetric T-state
  mutation the entire third-party corpus sweep stayed **green**. Loading one of our own `.z80` files
  in a third-party emulator is what settles it, it is observation, and it is **not done**. Nothing
  shrinks this to zero. What is required instead is that each run be **recorded** — file, SHA-256,
  outcome, date — in this document.
- **`.tzx` was the first thing to add if T4 was ever attempted. It landed 2026-09-01, the turbo
  gate followed on 2026-09-02, and what this row carries now is a distinction rather than an
  absence: the turbo *format* is graded, and a turbo *game* is not.** The premise stands — `.tap` is
  block data with the ROM's standard timings *implied*, nothing in the format can say *"this loader
  uses 700-T-state bits"*, no `.tap` can carry a turbo loader at any speed, and most commercial
  titles are turbo-loaded — and the cost was already paid down by the tape's internal form being a
  **pulse train** rather than a block list (`M6.md` Decision 5), so `.tzx` was a second converter
  with the ULA side untouched.

  What was missing after it landed was the other end of the wire. Every gate that ran a *machine*
  handed its `.tzx` to `LD-BYTES`, which reads only 855 and 1710, so `ID 11`'s pilot, sync and bit
  fields were graded **only at the ROM's own values**, where they are indistinguishable from
  constants.
  `tests/tzx_turbo_load.rs` supplies the missing half: a **124-byte hand-assembled Z80 loader**
  running from RAM on a blank ROM page, counting edges on `IN A,($FE)` itself, decoding a block at
  pilot 1400 / sync 500 / bit0 500 / bit1 1200 against the ROM's 2168 / 667 / 855 / 1710, and never
  calling the ROM at all. Three further encodings of the same signal — `ID 12 + 13 + 14`, an `ID 15`
  direct recording, and an `ID 30 / 21 / 23` tape reached only by following a jump — produce one
  identical **112,594-pulse** train, and four encodings sharing no field layout cannot be wrong the
  same way by accident.

  **Two measurements are worth carrying into this register rather than leaving in the gate.** There
  is **no ceiling**: at **3.56×** the ROM's data rate — the fastest tried — the emulator plays a
  tape a guest loader reads byte-perfect, and every failure met was the test loader's own
  resolution, proved by replaying each failing rate byte for byte unchanged and lifting it with an
  immediate inside the guest's Z80 code. And **2.08× fails silently**, which is the finding: the
  loader painted its border **green over 1244 wrong bytes**, because a one bit's poll count lands on
  its threshold, the ULA's I/O contention moves the poll cost across the frame, bits drop from the
  same position in scattered bytes, and an even number of them cancels out of an `XOR` fold. That is
  the all-zeros hole this document keeps cataloguing, met from the other side, and the gate asserts
  it **structurally** — *the fold agreed and the bytes did not* — rather than by the 1244, which is a
  property of that payload landing on that frame.

  **The mutation matrix is a statement about instruments and belongs in this register for that
  reason.** Against a deliberately broken `ID 11` handler in an isolated clone, ignoring the file's
  bit-one length or its pilot count reddens `tzx_turbo_load` and `tzx_vectors` while **`tzx_rom_load`
  and `tzx_rom_timings` stay green**. Substituting the ROM's own value for a field is invisible to
  any gate whose tape carries the ROM's own value, so **every gate here that grades by running a
  machine with the ROM in it was blind to those two**, and until this gate existed the only
  instrument that caught them was the vector-level one — literals a human transcribed from the
  format description. The generalisation *"a gate that runs a machine was blind"* is the tempting
  one and it is wrong: `tzx_turbo_load` runs a machine too, and reddens on all three rows. What
  distinguishes it is whose loader is reading. Transposing the pilot and sync
  fields is wrong at *every* speed and all four see it. A machine-level gate reaches further than a
  vector and sees only what its own loader is sensitive to; neither replaces the other.

  What is still graded by nothing is a turbo **game**, and it is a corpus problem rather than a
  capability one: `testdata/games/` is gitignored, no commercial turbo-loaded title is present, and
  none can be committed.

**Five further items M6 hands over, so that the four above are not read as the whole list.** Four
become register rows: `CpuState::wz` is destroyed by every snapshot load and no format
`MACHINE.md` names carries it; the FLASH phase across a load is carried by nothing, so a snapshot
taken mid-flash renders inverted for up to 16 frames; `.sna` restores at `frame_t_state = 0` — inside
the interrupt window — **by convention**, not by measurement; and the `EAR` sampling point within an
`IN` cycle is approximated to the start of the cycle, up to four T-states early, along with issue 2 /
issue 3 `EAR` readback, which is not modelled at all. **Eight new rows in total, four and four.**

The fifth gets **no new row on purpose**: `CpuState::q` is derived at parse from `F` and has **no
oracle**, exactly as it had none at M4 — and *"the flag latch has almost no instrument"* is already
a row in this register, from M4. Adding a second row saying the same thing about the same field
would make the register look as though M6 had discovered something it had not. **A milestone that
re-encounters an open item has not opened one**, and the register should say so by not growing.

### Two findings that were measurements, not opinions

**`M6.md`'s stated reason for `set_border` over `out_port` was falsified after implementation, and
the reason that replaced it is a different mechanism.** The design says routing a restore through
`out_port` *"would leave `Ula`'s `covered_t_states` armed at 4, so the first four ticks of the
**next instruction** would be charged as an open cycle and skip their contention."* **The
"next instruction" clause is false.** Every instruction begins with an opcode fetch, and
`begin_memory_cycle` assigns `covered_t_states` **unconditionally** — so a stale 4 is overwritten
before it can ever be spent, and nothing reachable through `Spectrum::step` can see it. Two things
do survive, and a mutation table separates them from the claim:

- **A bare-tick sequence.** The interrupt acknowledge is seven `Bus::tick` calls with **no transfer
  between them** to reset the count, so a stale 4 would silently cover four of them.
  `a_restore_leaves_no_machine_cycle_half_open` drives one tick directly, which is the smallest
  thing that can see it.
- **The tape moves.** A port cycle charges a contention stall, and the tape advances with the clock,
  so a restore through the bus **moves the head**. That one survives whatever order the setters run
  in, which makes it the more durable of the two.

So the setter is still unavoidable and the reason is still concrete — it is simply not the reason
that was written down. **The sharp part is what it took to see it:**
`a_restore_does_not_move_the_tape` **could not fail** until the machine's clock was inside the
display window when `restore` ran, because a ULA port access is only stalled there. The first
version of that test was green under the mutation it exists for. **A test's position in the frame is
part of its failing case**, and a gate written at an arbitrary clock position may be testing at the
one phase where its subject has no effect — which is the *exhaustive on one axis* lesson arriving in
a form this document had not recorded: not too few cases, but one case chosen where the quantity is
zero.

**A hand derivation lost to a measurement by exactly two T-states, and the loss was legible.** The
boundary before the parity byte in the ROM's own tape writer was **derived** as an ordinary byte
boundary — three T-states short — and **measured** at one short. `SA-PARITY` reaches the bit loop by
a path two T-states longer than `LD L,(IX+0)`, and that accounts for the difference exactly. The
derivation was wrong, and it was wrong *legibly*, which is the whole value of having written it down
before measuring: a wrong number with a stated mechanism can be reconciled, and a wrong number
without one can only be replaced.

**The class, which is the reusable half: a derivation of *another program's* behaviour is a
hypothesis until that program is run.** This is the same shape as the M3 `zexall` argument recorded
below under *Reaching for proof where you have measurement* — *"an algebraic argument is exactly as
strong as its weakest premise, and the premise there was a plausible claim about another program's
internals, asserted rather than measured"* — and it is worth having a second instance, because the
first one is easy to read as a story about that particular exerciser. It is not. **Derive first,
then measure, then reconcile**; the derivation earns its keep by making the disagreement
interpretable, not by being right.

---

## Milestone M5 — the 48K boots, and gates that can fail

The 48K boots to `© 1982 Sinclair Research Ltd` on **frame 87**, matched against glyphs read from
the ROM's own character set at `0x3D00` rather than a font table this crate wrote, and 200 frames of
boot run at **96× real time**.

> **That last figure is unmeasured and is marked so rather than re-taken.** It carries no command,
> no date, no host and no statement of what else the machine was doing — which is exactly what this
> document says, four hundred lines below, makes a number *"not a measurement at all: a claim
> wearing a measurement's clothes"*, and what `ARCHITECTURE.md` requires of every row in its
> *Measured* section. It sits one line under a removed *SECTION INCOMPLETE* banner in a commit
> whose whole thesis is that defect, which is the point worth noticing: **the sweep covered the
> section being corrected and not the sentence introducing it.**
>
> It is **not** re-measured here, deliberately. Three agents are compiling in this tree, and a
> throughput figure taken under that load would be worth less than the hole — a wrong number with
> a method reads as authoritative in a way that no number does not. Whoever takes it should state
> the command, the profile, the host and the load alongside it.
>
> **Two data points arrived anyway, and they are recorded with their conditions rather than
> promoted.** `cargo run --release -p spectrum --example boot -- testdata/roms/48.rom` is part of
> this project's standing verification set, and on 2026-09-01 it printed *"13977600 T-states in
> 0.036s — **112x** real time"* and, half an hour later on the same tree, **114x** — both while
> three agents were compiling. Those are **lower bounds taken under unstated load**, not a
> replacement figure. What they settle is only this: the quantity moves by 2 % between two runs of
> one command on one host within one hour, and by ~17 % from the recorded 96, so **96 was never
> reproducible in the sense the *Measured* discipline means.** A number that drifts that far
> without anyone characterising the conditions is not a measurement of the emulator; it is a
> measurement of whatever else the machine was doing.

> **This section opened under a banner reading *SECTION INCOMPLETE, DO NOT READ AS A VERDICT*,
> and the banner is removed here rather than left standing.** It was correct when written: the
> machine had landed in `2157331` and the measurements that decide what it proves were still being
> taken, so the sentence above was the only thing in the section that was settled. They are in. What
> follows is the verdict the banner was holding a place for.

The milestone landed in two commits, and **the second falsified the first one's premise.**
`2157331` shipped the machine and a table of what the boot gate covers. `bf8414d` shipped six real
gates — and writing them established that the first commit's table had been measured against
something that nothing runs.

### The gates of that pass, and ten mutations that turn them red

At `2157331` there was no `crates/spectrum/tests/` directory at all. There were **seven** gates in
it when this table was written, and the crate's own coverage table in `crates/spectrum/src/lib.rs`
is the per-property view of the same set:

> **Deliberately no live count.** The directory has grown past seven while this section was being
> written, and it is still growing — `ls -1 crates/spectrum/tests/*.rs | wc -l` gives a different
> answer from one hour to the next while three agents are landing gates. A number that goes stale
> between being written and being read is worse than no number: the list below names the gates it
> is about, `crates/spectrum/src/lib.rs` carries the per-property table, and the directory listing
> is the only thing that is ever current.

| Gate | What it grades |
|---|---|
| `tests/boot.rs` | the ROM reaching the copyright message, **and the frame it lands on** — the one number that discriminated, which the old example printed and never asserted |
| `tests/frame_interrupt.rs` | the 50 Hz line, its window, acceptance against `IFF1` × position, the `HALT` escape, and the real ROM's own `FRAMES` counter advancing once per frame |
| `tests/keyboard_matrix.rs` | the full 40 key × 8 half-row cross product, against a membrane table written independently of `keyboard`'s own map, plus two absolute anchors |
| `tests/rom_write_protection.rs` | every address of the ROM page, through all three write paths, driven by the **slot map** rather than by an address range |
| `tests/contention_magnitude.rs` | the same instruction one bank apart, per phase and over a run, through a real `Cpu<Ula>` — and **the whole read-modify-write family**, not one member: `INC (HL)` 26/19, `RLC (HL)` 34/27, `INC (IX+d)` 58/51, `RLC (IX+d)` 58/51, `EX (SP),HL` 48/41, plus an internal run on a **contended `IR`** |
| `tests/contention_phase.rs` | `timing::FIRST_CONTENDED_T_STATE`, pinned to the frame's structure |
| `tests/io_contention.rs` | the four-case I/O rule through a **real `Cpu<Ula>`** — `IN A,(n)`, `OUT (n),A` and `IN A,(C)` × four ports × eight phases, 96 assertions, every expected value derived from the published rule before the emulator was measured |

Ten mutations were run against them. **Every mutation's landing was verified before its verdict was
trusted** — occurrence count asserted before the write, file re-read after — which is this project's
standing rule and the reason these are measurements rather than hopes. The table below is the run
against the **first six**; `io_contention.rs` landed after it, and its own admission ticket is the
hole it found on the way in — see *Still ungraded*, where it closes a row.

| Mutation | Verdict |
|---|---|
| `/INT` never asserted | `frame_interrupt`, **6 of 6** |
| Keyboard reports every key held | `keyboard_matrix`, **6 of 8** |
| Keyboard matrix permuted | `keyboard_matrix`, **3 of 8** |
| ROM slot made writable | `rom_write_protection`, **4 of 6** |
| Contention phase off by one (14335 → 14334) | `contention_phase` — **the only failure in the workspace** |
| `pixel_address` line reduction removed | `screen`, exhaustive over all 65,536 pairs |
| `Ula::fetch` removed, so M1 falls back to `read` | **7 failures** |
| `MEMORY_CYCLE` 3 → 2 | **8 failures** |
| Internal cycles never contend | **5 failures** |
| Contention removed entirely | **14 failures** |

> **That table is the record of one pass and the directory has grown past it.** The row
> `contention phase off by one (14335 → 14334)` in particular says *"the only failure in the
> workspace"*, and re-measured on the current tree it is **RED 3** —
> `contention_phase.rs`, `timing_oracle.rs` and `frame_boundary.rs`. The old figure was correct
> when taken and is left as it stands, because *"the only failure in the workspace"* is a claim
> about a run, and naming the wrong run makes it wrong in exactly the way this document's own M5
> section is about. Whoever quotes a verdict from this table should re-take it first.

### Two more gates: the frame boundary, and a block instruction that is actually interrupted

Two properties of the **machine** rather than of an instruction had no gate in any form. Both are
load-bearing, both were named in the documents as design decisions with nothing behind them, and
both are now driven through a real `Spectrum`.

| Gate | What it grades |
|---|---|
| `tests/frame_boundary.rs` | the clock rolling over **inside** an instruction: the frame counter and the offset as a **pair**, an instruction landing exactly on 69888, **two** crossings in one `step()`, contention priced at the same frame position in frames 0, 1 and 2, one instruction priced free-then-contended across the wrap, and an overshoot past the 32-T-state window missing that frame's interrupt entirely |
| `tests/block_interrupt.rs` | an 8192-byte `LDIR`/`LDDR` interrupted mid-loop by the machine's own 50 Hz interrupt: the two acceptances it takes, the derived iteration each lands on, the `BC` still remaining, the instruction's **own** address on the stack, the resumed loop finishing the copy byte-exact, and `R` counting the acknowledge's refresh |

**Everything positioned past frame zero needed a mechanism first, and its absence is why these
were ungraded rather than merely unwritten.** `common::advance_to` assembles one straight-line
prologue, and a frame is some seventeen thousand instructions away — more than the bank it
assembles into. So the far side of the first frame boundary was *unreachable*, and every gate in
that directory measures inside frame zero. `common::advance_to_absolute` reaches it by repeating a
fixed-cost sled instead of assembling a longer one.

Eleven mutations were run against the pair, each landed and each restored from bytes held by the
driver. Counts are failing tests across the whole workspace under `--no-fail-fast`:

| Mutation | Workspace | Reddens the new gates by |
|---|---|---|
| Rollover tests `>` rather than `>=` | RED 8 | the exact landing, the NOP equivalence, the `HALT`, and an acceptance point |
| Contention priced from time since power-on | RED 4 | both pricing rows |
| The interrupt offered *after* the instruction | RED 6 | the overshoot, the `HALT`, the acceptance points |
| `INTERRUPT_T_STATES` 32 → 33 | RED 4 | the overshoot |
| **`INTERRUPT_T_STATES` 32 → 24** | **RED 1** | **the overshoot — sole witness in the workspace** |
| **A 16-bit T-state accumulator** | **RED 1** | **the two-wrap chain — sole witness** |
| `LDIR` runs its whole loop inside one `step()` | RED 15 | all three behavioural block gates |
| `acknowledge` no longer increments `R` | RED 4 | the refresh count |
| `FIRST_CONTENDED_T_STATE` → 14334 / → 14336 | RED 3 each | the free-then-contended chain, both times |
| Rollover discards the overshoot | RED 3 | **nothing** — and that is a property, see below |
| Rollover `if` rather than `while` | RED 3 | **nothing** — same reason |

Two of those are sole witnesses, and both are worth naming because a gate with no unique failing
case is decoration. **A 16-bit T-state accumulator** is the next size up from the `u8` that once
aborted this process on a legal instruction stream; 131,072 T-states in one `step()` — a
32,767-prefix chain filling every byte of uncontended RAM — is the only measurement in the
workspace large enough to see it. **`INTERRUPT_T_STATES` at 24** is the one the window's own gate
could not see, and is written up below.

### The premise those gates were written from was wrong, and that is the most useful thing here

`2157331` reported five mutations as green **under the boot gate**. They were green under the boot
*example* — `crates/spectrum/examples/boot.rs`, which `cargo test` builds and never runs, because
`main` is never called.

Re-run at `2157331` against the pre-gate lib target — baseline 72 passed, no `tests/` directory —
with each mutation's landing asserted before its verdict, **four of the five were already red** from
unit tests inside `src`:

| Mutation | Verdict, pre-gate | Failing tests |
|---|---|---|
| `/INT` never asserted | **RED** | 5 |
| Keyboard reports every key held | **RED** | 7 |
| ROM slot made writable | **RED** | 1 |
| Contention removed entirely | **RED** | 13 |
| Contention phase off by one | **GREEN** | — |

**One survivor from the original five.** The keyboard-matrix permutation also survived, and it is
the other reason `keyboard_matrix.rs` exists — but it **was never among the five**; a cold review
found it separately.

So the honest before-picture is not "five things were ungraded". It is this:

> The machine's behaviour was tested **in isolation** and never **through the machine**, and the
> contention phase and the matrix wiring had no gate in any form.

That is a better finding than the one the milestone set out to report, and it is only visible
because the verdict was re-taken against the whole workspace instead of inherited. A coverage table
is a claim about a *run*, and naming the wrong run makes every row in it wrong at once.

> **This paragraph said *"three of the five"* and *"only two survived"*, and both were wrong.** It
> is corrected in place rather than adjusted quietly, because the error is not a typo and its
> shape is the useful part. The per-mutation numbers were never in doubt — 5, 7, 1 and 13 failing
> tests, sitting in a table directly beneath the summary that miscounted them. The "three" was a
> summary sentence nobody re-added, and the "two survivors" came from counting the phase mutation
> **plus** the keyboard permutation, which was not one of the five being summarised. The figure
> then propagated into three documents, where it read as settled because three files agreed.
> The class is written up under *A derived figure repeated across documents acquires authority it
> never earned*, below.

### The keyboard matrix was graded against itself

`every_key_is_visible_to_a_scan_of_its_own_half_row` derived **both** the port it scanned **and** the
value it expected from `Key::position()` — the function under test. It proved `read()` consistent
with `position()` and could not see whether `position()` matched the hardware. Only `X` and `ENTER`
were pinned absolutely.

Measured: **38 of the 40 keys could be rewired with the entire suite green.** The wiring was in fact
correct. **The defect was the evidence**, and it is now a literal 40-row table that owes nothing to
the code under test.

**The 38 is not an arbitrary number, and re-deriving it rather than inheriting it is what makes the
point sharp.** The two keys that could *not* be moved are exactly the two whose expectations were
**literals** — `X` at `(0, 0x04)` and `ENTER` at `(6, 0x01)`. Every other key's expectation was
computed by the function under test, so it moved with the function and the comparison stayed true.
Two literal rows out of forty were the entire discriminating power of the suite. That is the
cleanest available demonstration that **a literal table is different in kind from a consistency
check, not merely stricter** — the consistency check has *no* failing case for the property it
appears to test, and forty of them have no more power than one.

This is the *exhaustive on one axis* lesson in a new shape. That test was exhaustive over the
40 × 8 cross product and varied nothing on the axis that mattered, because both sides of every
comparison came from one source. A test whose expectation is computed by the subject is not a weak
test; it is a tautology with a cross product attached.

### `Bus::fetch` landed, and the heuristic it replaces is deleted

`crates/z80/src/bus.rs` gained a defaulted `fetch`, `Ula` implements it, and
`crates/spectrum/src/machine_cycle.rs` — the 312-line file that reconstructed machine-cycle
boundaries from the tick stream by deferral — is **deleted**. Measured on the change:

| | |
|---|---|
| Production LOC | **−161** |
| Branches on the `Bus::tick` path | **−8** |
| Nesting depth on that path | **−2** |
| Public API change, `crates/z80` | **none** — `fetch` is additive and defaulted |
| Public API change, `crates/spectrum` | `pub mod machine_cycle` removed, and `Clock::advance` with it. Both were surface that could only be misused: no public path returned a `MachineCycle`, and `advance` was a `Copy`-on-a-temporary **no-op**. Recorded in [`CHANGELOG.md`](../CHANGELOG.md) |

**What of that table this pass re-derived, and what it did not.** The deleted file's production half
is **191 lines** — `git show 2157331:crates/spectrum/src/machine_cycle.rs` runs production code to
line 191, where `#[cfg(test)]` begins — and that was checked here. The **−161 net**, the branch count
and the nesting depth come from the implementer's measurement of the change and were **not
re-derived line by line in this pass**; whoever wants them independent should re-run the count across
`machine_cycle.rs` and `ula.rs` between `2157331` and `bf8414d`.

The full account of *why* the reconstruction existed and what it cost is in
[`MACHINE.md`](MACHINE.md), and the two hardware rulings `fetch` forced are in
[`Z80-REFERENCE.md`](Z80-REFERENCE.md).

### Still ungraded — a deliverable, not an apology

This list is the answer to `MACHINE.md`'s instruction to write down which properties nothing covers,
rather than inferring correctness from the absence of a failing test. It is **not** a to-do list
awaiting apology; it is the shape of what M5's green means.

- ~~**`timing::FIRST_CONTENDED_T_STATE` has no oracle.** It is pinned to `64 × 224 − 1`, a
  **derivation** from documented frame structure — not a measurement. Nothing compares it to
  hardware, and **an issue 2 machine is one T-state earlier and would pass the gate identically.**
  `contention_phase` pins it against drift; it does not establish it.~~
  **Closed by `tests/timing_oracle.rs`.** The row's evidence, its sixteen mutations and its
  precise scope are in the **Closed** table; the anti-circularity reasoning, which is the reusable
  part, is under *Is this oracle circular?*. Two things about the struck text are worth keeping
  visible rather than deleting: *"an issue 2 machine is one T-state earlier"* is **the wrong
  axis** — the suite's own data has the early/late split spanning six board issues with three of
  them in **both** classes, and one machine reporting Late cold and Early warm — and what the
  oracle closes is the **interval from `/INT` to the first contended T-state**, not the constant
  standing alone.
- ~~**The 64-line pre-display count is taken on trust.** It is the input to that derivation, and it
  has the same status.~~ **Closed only as a consequence, which is a weaker thing and is recorded
  as such.** The oracle measures the interval's *product*, not its factors: a pre-display count of
  63 with a line of 227.5 T-states is not a machine, but any compensating pair that still lands on
  14335 would pass identically. What is established is the total; `64 × 224` remains the reading of
  it that the documented frame structure supplies.
- ~~**The interrupt window's *length* is still not measured — and it is now *demonstrated* that the
  oracle cannot measure it.** `INTERRUPT_T_STATES` moved 32 → 24 leaves `timing_oracle.rs` green;
  measured here, that mutation reddens **exactly one test in the whole workspace**, and it is
  `frame_boundary.rs`'s `an_overshoot_past_the_interrupt_window_misses_that_frames_interrupt`. So
  the row narrows rather than closes: the window is now pinned against drift **on both edges**,
  by two literals rather than by a value derived from the constant, and nothing anywhere compares
  32 to hardware.~~ What the two-sided pin replaced is written up under *A window graded against
  its own constant*, below.
  **Closed by `tests/timing_oracle.rs`, and the word *demonstrated* is the part that was wrong.**
  A single mutation is one sample, and 24 turned out to sit **inside** the band the oracle is
  genuinely insensitive to; the sweep over 1–65 pins the constant to **`17..=32`**. Calling one
  sample a demonstration of insensitivity is the same error this document catalogues elsewhere as
  an argument that names real quantities, predicts the right verdict, and identifies the wrong
  cause — and it is worth keeping visible for that reason rather than deleting. The evidence, the
  two derived-before-measured edges, and the nested-interrupt mechanism that explains why 24 was
  invisible are in the **Closed** table.
- ~~**I/O contention is graded against a hand-written tick stream**, not through a real `Cpu<Ula>`.~~
  **Closed by `tests/io_contention.rs`** — three instruction forms (`IN A,(n)`, `OUT (n),A`,
  `IN A,(C)`) × four ports × eight phases through a real `Cpu<Ula>`, 96 assertions, every expected
  value derived from the published rule before the emulator was measured. **It found a real hole
  on the way in:** `ula.rs`'s existing four-case unit test only ever exercises phase 0, and at
  phase 0 the second `C` term of the contended-ULA-port case is zero — so deleting that term left
  the unit test green. That is the *exhaustive on one axis* lesson again, in the smallest possible
  form: four cases enumerated, one phase.
- ~~**The read-modify-write family is gated by one member.**~~ **Closed.** `RLC (HL)` (34/27),
  `INC (IX+d)` (58/51), `RLC (IX+d)` (58/51) and `EX (SP),HL` (48/41) are now gated alongside
  `INC (HL)`'s 26/19, and `the_index_computation_is_charged_on_the_displacement_address` separates
  the two indexed forms from each other by moving the operand out of the contended bank, so
  **where** the index-computation T-states are spent is observed rather than asserted. "By
  construction" was an argument; it is now a verdict.
- **Floating bus, progressive drawing (multicolour, border stripes) and keyboard ghosting are not
  modelled**, so they are not gradeable rather than ungraded. ~~The oracle's groups 35–37 are
  excluded **by name** for exactly that reason, so its green says nothing about them.~~

  > **Corrected 2026-09-01: the excluded groups are 36 and 37. Group 35 is graded, and the reason
  > it can be is a coupling rather than a modelling success.** The three are **one program** — the
  > suite's dispatch table sends all of them to `0xC91D` — differing only in how many rows of text
  > the BASIC prints first (`l` = 0, 13, 21). Inside the loop, `IN B,(C)` puts the value it reads
  > into the port's **own high byte**, so what is on the screen decides whether the four
  > `IN r,(C)` after it are contended. That is why 36 and 37 cannot be graded here, and it is
  > measured rather than assumed: group 36 run with the screen filled with `0x00`, with `0xFF` and
  > with a text pattern gives **identical** readings, which a constant floating bus predicts and
  > hardware cannot do.
  >
  > **Group 35 matches both its hardware rows bit-exactly — because `FLOATING_BUS_BYTE` is `0xFF`,
  > which lies outside the contended range `0x4000..=0x7FFF`**, so those four bus-dependent cycles
  > are never charged and the only contention the row measures is the part that does not depend on
  > the bus. Two consequences, both of which belong to this row rather than to the gate:
  >
  > - **Whoever implements a floating bus should take group 35's two rows as part of that work's
  >   acceptance criterion, not as a pre-existing gate to keep green.** A model returning the byte
  >   the ULA is fetching returns display bytes inside `0x40..=0x7F` for ordinary screen content,
  >   so the row can redden for a reason that has nothing to do with contention — and the next
  >   reader would have every reason to misread that as a regression.
  > - **It is not established why hardware agrees with a constant `0xFF` here.** Both rows were
  >   measured with text on screen whose glyph bytes fall inside `0x40..=0x7F` (`Z` is `0x7E`), and
  >   the *uncontended* row is the harder case: the BASIC runs it **before** its own `CLS`, and the
  >   suite never clears between tests, so it is taken with thirty-four tests' worth of report text
  >   on display — and being uncontended buys it nothing, because I/O contention is a function of
  >   the port address, not of where the code sits. Recorded as open in
  >   `crates/spectrum/tests/timing_oracle.rs`'s *Group 35* section. **It is the gap that would
  >   make grading that row safe rather than merely green.**
- **The frame's origin is a convention, not a measurement.** We assert `/INT` at frame T-state 0
  and the oracle grades the interval from there; moving the origin and the window together — `/INT`
  one T-state later *and* `FIRST_CONTENDED_T_STATE` at 14336 — leaves `timing_oracle.rs` green.
  Measured here rather than inherited: that mutation is RED 15 across the workspace and
  `every_instruction_group_matches_the_hardware_table_contended_and_not` is **not among the
  fifteen**. The convention is pinned against drift by fifteen tests and established by none.
- ~~**An instruction driven across the frame boundary.** Nothing grades that the clock rolls over
  correctly mid-instruction, that contention is priced correctly on the far side, or that one
  instruction can advance the frame counter more than once.~~ **Closed by
  `tests/frame_boundary.rs`.** Seven cases, eight mutations, and two of them reddening that file
  and nothing else in the workspace — see the gate table below.
- ~~**A block instruction actually interrupted mid-loop.** `block_contention.rs` grades the rewind,
  which *shows* the loop is interruptible; nothing interrupts one.~~ **Closed by
  `tests/block_interrupt.rs`**, which drives an 8192-byte `LDIR` across two frame boundaries and
  grades the two acceptances it takes.
- **An interrupt arriving mid-loop while the loop is being *contended*.** `block_interrupt.rs`
  runs wholly uncontended on purpose, so the iteration an interrupt lands on is arithmetic rather
  than a simulation of the model `block_contention.rs` grades. The contended case is M7's shape.
- ~~**The four-case I/O rule's fourth term.** A mutation dropping it leaves the oracle green,
  because its groups 1–34 never reach a contended non-ULA port. `io_contention.rs` remains its
  only gate, and that is now a measured statement rather than an assumption.~~
  **Closed 2026-09-01 by extending `tests/timing_oracle.rs` from 34 groups to 35** — 68 hardware
  rows to **70**.

  > **The struck row's measurement was right and its *scope* was wrong, which is the interesting
  > part.** Groups 1–34 genuinely never reach a contended non-ULA port. But *"the suite's groups"*
  > was the oracle's **then-range** spoken of as a property of the corpus, and group 35 was
  > outside it only because it sits in the same three-group block as the two that really do need a
  > floating bus. **Group 35 reaches the term directly**: its loop addresses `(C<<8)|C` with `C` a
  > counter advancing once per iteration from zero, so it sweeps every high byte and both parities
  > of A0, and every *odd* `C` in `0x41..=0x7F` is exactly `C:1, C:1, C:1, C:1`.
  >
  > Re-measured in a scratch clone on the day of the change — the same mutations against the two
  > ranges, each landing asserted before its verdict was read and each restore checked against
  > held bytes:
  >
  > | mutation of the `(true, false)` arm | oracle at 34 groups (68 rows) | oracle at 35 groups (70 rows) | `io_contention.rs` |
  > |---|---|---|---|
  > | deleted — `(true, false) => 0` | **GREEN**, 3 passed | **RED**, 2 of 70 — and both are group 35's | **RED**, 2 of 3 |
  > | weakened to the two-stall shape | **GREEN**, 3 passed | **RED**, 2 of 70 — both group 35's | **RED**, 2 of 3 |
  > | the fourth term alone dropped | **GREEN** — the other 68 rows unmoved | **RED**, 1 of 70 — group 35 uncontended only | **RED**, 1 of 3 |
  >
  > The right-hand columns are what makes this an extension worth having rather than two more
  > green rows: **the mutation is invisible at 34 groups and caught at 35, and no other row moves**
  > — the disagreement is exactly the two rows that were added, which is also the evidence that
  > the other 68 are untouched by the change.
  >
  > **Three edits, not two, and the third is the one that grades the fourth term specifically.**
  > This table carried the first two for a while and `io_contention.rs` carried a different pair,
  > so read against each other the two records looked like a disagreement about what the fourth
  > term is worth. They were both right and both partial. Dropping the term alone is the smallest
  > verdict of the three because the first two change the arm's cost at every group position while
  > this one changes it at **one position in eight** — a row that is easy to mistake for a weaker
  > version of its neighbours and is in fact the only one of them aimed at the term. The counting
  > standard that settles it is [`MACHINE.md`](MACHINE.md)'s: **a mutation is an edit, not a
  > verdict and not a constant**, which is what makes this arm three and not one.
  >
  > So the term now has an **external** check as well as this project's own, and the two are
  > complementary rather than redundant: `io_contention.rs` is exhaustive over four ports × eight
  > phases with every expectation derived by hand from the published rule, while the oracle's row
  > is integrated over a whole frame against a number this project did not write. **Group 35 also
  > reaches `OUT (C),r`**, which `io_contention.rs` names in its own *what is not graded here* —
  > six of them are in the loop. **The oracle's row carries a coupling `io_contention.rs` does
  > not**; it is the floating-bus row above, and it must be read before this closure is relied on.

The struck rows are kept with their closures attached rather than deleted, for the reason the
Closed table exists: **a row that vanished is indistinguishable from a row nobody re-read.** The
remainder are the shape of what M5's green still means — and the list is longer than it was, which
is the intended direction: closing four items surfaced four sharper ones underneath them.

### What M5 opened — all four closed, and one of them into three narrower rows

They are **not listed in full here.** There is one open register in this project and it is
*[Open — the authoritative register](#open--the-authoritative-register)*, in the M1 section; a
second table in the newest section is precisely the duplication that let two documents disagree
about four facts in one session. M5 opened four items; three left the register into the Closed table
when the milestone closed, and the fourth has since followed them:

- ~~**Open:** `timing::FIRST_CONTENDED_T_STATE = 14335` has **no oracle**. Unchanged.~~
  **Closed by `tests/timing_oracle.rs`**, with the same evidence and the same scope as the struck
  row under *Still ungraded* above and the row in the **Closed** table below. This bullet is
  **the third recurrence of this document's own propagation defect**, and it is left struck rather
  than deleted for that reason: the *Still ungraded* row was struck and the Closed entry written
  in the same pass that left this copy standing, so for a while one section of this file said the
  item was closed and another said it was open and unchanged. The standing rule that a correction
  is not landed until every other copy of the corrected claim has been grepped for is in this
  document because this document keeps breaking it.

  > **State the scope, because the obvious reading is too strong.** What is established is that
  > **the first contended T-state falls exactly 14335 T-states after `/INT`** — given that this
  > machine asserts `/INT` at frame T-state 0. Three things stay open and each is its own row in
  > the authoritative register rather than a footnote here: the frame's **origin** is a convention
  > (moving `/INT` and the window together leaves the oracle green), the interrupt window's
  > **length** is unmeasured (32 → 24 leaves the oracle green), and the `64 × 224` **factorisation**
  > is not measured — its *product* is, and any compensating pair landing on 14335 would pass
  > identically.
  >
  > **The two green mutations named in that sentence are carried from the rows above, not re-taken
  > here.** They were measured by the pass that landed the oracle and are recorded under *Still
  > ungraded* and in the authoritative register; this bullet quotes them and did not re-run them,
  > which is stated so that a reader does not count one measurement twice. What **was** re-taken
  > for this bullet is only that the gate is green at all: `cargo test -p spectrum --test
  > timing_oracle` on 2026-09-01 reports 3 passed, 0 failed (exit status captured before any pipe).
- Closed: the read-modify-write contention residual — `Ula` implements `fetch`.
- Closed: five mutations leave the boot gate green — the boot *example*, and **four of the five**
  were already red (5, 7, 1 and 13 failing tests) against the pre-gate lib target.
- Closed: nothing runs the boot gate — `crates/spectrum/tests/boot.rs` runs it.

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

> **Still true of *oracles*, and no longer true of *gates* — the two are worth keeping apart,
> because this row is quoted as though it said the second.** Nothing external grades an interrupt,
> and nothing at M5 changed that. What changed is that acceptance is now driven **through the
> machine** rather than only in `crates/z80`'s unit tests: `tests/frame_interrupt.rs` grades the
> line and the `HALT` escape, and `tests/block_interrupt.rs` grades acceptance **inside a
> repeating instruction** — the derived iteration it lands on, the `BC` still remaining, the
> instruction's own address pushed, and the acknowledge's refresh visible in `R`. Mode 2's
> vector-table read is exercised there too, by both new gates.
>
> `NMI`, `RETN`, mode 0 and the `EI` deferral remain unit-tested only, and the *oracle* column of
> the table above is unchanged: **❌, ❌, ❌**.

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
| Wall clock, release | **43.1 s** — ~308x real time at 3.5 MHz. *Written as "within 7 % of `benches/step.rs`'s 329x". The 329x is now **unresolved**: the same bench re-run on this tree gives 296–308×, and this row's own 308x — a completely independent workload of 5.8 × 10⁹ real instructions — lands on that figure rather than on 329. Two measurements agreeing is not proof, but the agreement is with 306, not with 329. See [`ARCHITECTURE.md`](ARCHITECTURE.md)* |
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

> **The "16 passing tests" is 19, and it is labelled as measured, so it is re-measured here.**
> `cargo test -p z80 --test zex_oracle` reports **19 passed, 2 ignored** on 2026-09-01 — the file
> holds 21 `#[test]`s of which `zexdoc_conformance` and `zexall_conformance` carry `#[ignore]`.
> The argument survives untouched: the command still exits 0 without ever looking for the
> exerciser, which is the whole point. Only the integer was stale.
>
> **What was *not* re-taken, and the distinction matters:** this run had the corpus **present**,
> because moving `testdata/` aside while three agents are running their own suites would break
> theirs. The original claim is about the corpus-absent case, and its number is the count of
> runnable tests either way. State it that way rather than re-quoting a figure taken under
> conditions nobody recorded.

### What the harness learned from the M2 review

The verdict is a pure function over a parsed report rather than a chain of inline `assert!`s.
The reason generalises beyond this file: an inline assertion inside a 43-second run can only be
proven to bite by a manual mutation nobody repeats, whereas the same rules as a function have
**one failing case each, running in microseconds on every `cargo test`**, corpus or no corpus.

Six tests cover the verdict rules and ten cover the CP/M shell and the report parser — sixteen
in all, none of them `#[ignore]`d, none of them needing `testdata/`. The one that matters most
is `a_run_that_stopped_early_is_a_fault_even_though_every_line_said_ok`, because a truncated run
prints nothing but `OK` lines and "did any line say ERROR?" passes it.

> **Counted on 2026-09-01 from the test list rather than from memory: it is nine and ten —
> nineteen in all.** The nine verdict-rule tests run from
> `the_pinned_instruction_stream_is_accepted` to `a_port_access_is_a_fault`; the ten shell and
> parser tests from `both_bdos_functions_reach_the_console` to
> `the_report_parser_ignores_the_banner_and_the_footer`. The sentence's claim — none `#[ignore]`d,
> none needing `testdata/` — is unchanged and still true.
>
> **The same stale sixteen sits in `testdata/README.md`, which is another agent's file and is
> routed separately.** That is the whole content of the propagation lesson below: one derived
> figure, two files, and the correction only lands in the one being read.
>
> > **Routed and landed, 2026-09-01.** `testdata/README.md` no longer carries a number there: it
> > carries the two commands (`grep -c '#\[test\]'` and `grep -c '#\[ignore'` over
> > `crates/z80/tests/zex_oracle.rs`, giving 21 and 2 on that date) and a note recording that the
> > figure had been stale in two files simultaneously. **The gap between "routed separately" and
> > "routed" was the whole defect**, and it lasted as long as it did because the sentence naming it
> > read like an action. Naming a thing as owned elsewhere is a hand-off only if somebody takes the
> > other end.

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

> **Both numbers are unreproducible, and the reason is written three subsections below this one
> in this same file.** 507× / 294× were measured against a `Bus::tick` that took a **batch**
> — and *Hardening round*, below, records that batching was removed at M1 because it discards
> 88 of the corpus's 166 internal contention points. **The headline therefore describes a bus
> contract this milestone deleted**, and it has been quoted as M1's result ever since. There is
> no tree in this repository it can be re-run on.
>
> What replaced it, re-measured on 2026-09-01 across all five milestone trees: the per-T-state
> flat bus runs at **~306×** and the paged, contended bus at **~160×**. The full verdict table,
> the load the machine was under, and the command are in
> [`ARCHITECTURE.md`](ARCHITECTURE.md)'s *Measured* section and **only** there.
>
> **The last sentence survives its own evidence, and that is the point worth keeping.** At 160×
> the paged bus spends ~125 µs of a 20,000 µs frame — **0.6 %** rather than 0.34 %. The policy
> was never close to the margin, so "optimise nothing" is exactly as safe against the real
> number as against the wrong one. A conclusion that is robust to its premise being off by
> nearly 2× is worth marking as such, because the next one may not be.

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
M5 section above lists its item *names* and defers here for their state, for the same reason. M5
opened four; three left this table for the Closed one below when the milestone closed, and the first
row is the survivor.

> **The survivor has since closed, and closing it opened three narrower rows in its place.** The
> row read *"`timing::FIRST_CONTENDED_T_STATE = 14335` has no oracle"*; `tests/timing_oracle.rs`
> settled it against measured hardware, and settling it made visible that the constant was never
> the whole claim. What the oracle grades is an **interval** — from `/INT` to the first contended
> T-state — so the frame's origin, the window's length, and the `64 × 224` factorisation of that
> interval each had to be stated separately, and each is still open. The three rows at the top of
> this table are those, and the closed row is in the Closed table with its scope and the three
> mutations that bound it. **A row that closes into three narrower rows is the normal case, not a
> setback**: what it means is that the original row was carrying more than one claim.

> **M6 merged and added eight rows, which now head this table.** Four of them are the milestone's
> stated deliverable — the tape's timings graded against the **ROM** rather than against hardware,
> contention during a load exercised and graded by nothing, **T4 running nowhere**, and the absence
> of `.tzx` — and four more come from `M6.md`'s own hand-over table. **Three of those rows were
> renamed on 2026-09-02 rather than closed**, when `tests/tzx_turbo_load.rs` made the turbo *format*
> graded and left the turbo *game* where it was; the `.tzx` row in particular now reads *no turbo
> **game** is graded*, which is where a reader looking for the old title should go. The account of
> each lives in
> the M6 section above and the settling condition lives here; that split is the one the M5 rows
> already use, and it exists so that a reader looking for *what is open* finds one table rather than
> three narratives.
>
> **M6 opened more rows than it closed, and that is the intended direction rather than a setback** —
> the same shape the note above records for the timing oracle. What M6 *closed* — the round trips
> and the measured statement of what breaks their symmetry, the tape's signal-level model gated
> through the real ROM, and the parser's panic class closed **structurally** — were never rows in
> this table, because each was opened and settled inside one milestone. They are recorded in the M6
> section rather than in the Closed table below, because that table's rule is that **an item leaves
> the Open table only into it**, and back-dating Open rows so that closures have somewhere to land
> would make both tables fiction.

> **`crates/frontend` enters this register for the first time, and its absence was the defect
> this document is named after applied to a whole crate.** Added 2026-09-01. The crate is a
> workspace member; it has its own gates, its own explicit *not gated* table in `src/lib.rs`
> written in this document's own format, and until now **this register mentioned it nowhere** —
> `grep -n -i frontend docs/STATUS.md` matched nothing. `MACHINE.md`'s verification plan says the
> same of itself, in as many words, and points here.
>
> **Why that is worse than it sounds, and not merely an omission.** This document's standing rule
> is that *"a milestone is not done until it has written to the register"*, and its stated reason
> is that anything recorded only in a design document *"is never revisited when an item closes, so
> an item recorded only there is invisible the moment it stops being true."* The frontend's
> ungraded list was recorded in a **crate doc comment** — which has the same property, plus one
> worse: `lib.rs:26` had been asserting a **figure about this register that this register does not
> contain** ever since it was written, and no reader of either file was positioned to notice,
> because the two were never read together. **A crate outside the register can contradict it
> without anybody being in a position to see the contradiction.**
>
> The five rows below are the crate as it stands today plus the two facts `M8.md` established
> about the world it will run in. They are stated as what nothing covers, not as work owed.

| Item | State | Settled by |
|---|---|---|
| **M6** — tape pulse timings are graded against **the ROM**, not against hardware | `tests/tape_rom_timings.rs` compares 3305 half-periods against the ROM's own `SA-BYTES`, by value and in order, with the ULA's contention of the writer's `OUT`s subtracted. That is an oracle and it is **not a hardware measurement**: that 2168 / 667 / 735 / 855 / 1710 are what a real Spectrum emits is **unverified** | An oscilloscope on a real Spectrum, or a tape-timing corpus with hardware provenance of the kind `testdata/timing/` has. **Nobody here has taken one, and no gate in this repository could** |
| **M6** — contention during a tape load is exercised and graded by nothing | The loader performs thousands of contended port cycles and writes into contended RAM, so the model runs. Measured: the mutation *"the tape stops seeing contention stalls"* reddens the two contention-vs-tape gates and leaves the **ROM load gates green** — the ROM's loader does not depend on the contention it suffers | **A mutation of the stalls taken against `tests/tzx_turbo_load.rs`.** The loader that row used to ask for now exists and its margins are that tight: its 2.08× row is *produced* by the four-case I/O rule moving the cost of a poll across the frame. So the contention model is no longer only exercised by a tape gate — it decides one gate's observed behaviour. Nothing has yet been mutated against it, which is why this is a settling condition and not a closure |
| **M6** — **T4 runs nowhere** | The only tier that grades a turbo **game**, a program written by somebody who did not know how this emulator works, or our **arithmetic** on a format's fields. It read *"a turbo loader"* until 2026-09-02; a turbo loader is graded by `tests/tzx_turbo_load.rs`, and what cannot be committed is the game. That last is measured, not assumed: under a symmetric mutation of the T-state formula the whole third-party corpus sweep stayed green | Nothing, structurally — a repository that may not carry games cannot commit the corpus. What is required instead: each run **recorded** here with file, SHA-256, outcome and date. Loading one of our own `.z80` files in a third-party emulator is the cheapest half and is **not done** |
| **M6** — no turbo **game** is graded, and none can be committed | This row read *"`.tzx` is absent, and therefore turbo loaders are"*. `.tzx` landed 2026-09-01 and `tests/tzx_turbo_load.rs` graded the format on 2026-09-02 — sixteen tests, a 124-byte loader of our own on a blank ROM page decoding `ID 11` at pilot 1400 / sync 500 / bit0 500 / bit1 1200, four encodings of one 112,594-pulse signal, **no ceiling found at 3.56×** the ROM's data rate, and a **silent** failure at 2.08× (a green border over 1244 wrong bytes) asserted structurally. The premise is unchanged: `.tap` names no timing, so it carries no turbo loader at any speed, and most commercial titles are turbo-loaded | Nothing available, and it is a **corpus** problem rather than a capability one: `testdata/games/` is gitignored, no commercial turbo-loaded title is present, and none may be redistributed. This is the same wall as the T4 row and closes with it or not at all |
| **M6** — `CpuState::wz` is destroyed by every snapshot load | No format `MACHINE.md` names carries it; both parsers set zero. Observable only through the undocumented flag bits of a `BIT n,(IX+d)` executed before anything else writes it | Nothing available. `.szx` carries it, and M7 is where that would be reconsidered — `M6.md` Decision 9 defers it on YAGNI grounds that the field does not disturb |
| **M6** — `.sna` restores at `frame_t_state = 0` **by convention** | Which is inside the interrupt window, so a `.sna` load takes an interrupt almost immediately. The format carries no T-state counter; this is what other implementations do and what most `.sna` files expect. A convention, not a measurement | A `.sna` observed to misbehave, or a format description that says otherwise |
| **M6** — the FLASH phase across a load | **Nothing.** No format carries it, and this model derives it from `frames()`, which `restore` deliberately leaves alone so that uptime means one thing. A snapshot taken mid-flash renders inverted for up to 16 frames after loading | A user noticing the flash jump. It is also the thing that would decide `M6.md`'s open question of whether `restore` should reset `frames()` |
| **M6** — the `EAR` sampling point within an `IN` cycle | Approximated to the start of the cycle: `Ula::in_port` runs after the contention stall is charged and before the cycle's four nominal ticks, so the level is sampled **up to four T-states early**. Far inside the ROM's tolerance, which distinguishes 855 from 1710 | A specific turbo loader failing. **One now runs** — `tests/tzx_turbo_load.rs`, against half-periods as short as 220 T-states — **and has not failed for this reason**: every limit it met was traced to a constant in its own Z80 code. That is the absence of a distinguishing failure, which this document's own standing rule forbids reading as evidence, so the row is unchanged and its condition has merely been attempted. Same for issue 2 / issue 3 `EAR` readback, which is not modelled at all |
| **M5** — the frame's **origin** is a convention | We assert `/INT` at frame T-state 0 and everything is measured from there. `tests/timing_oracle.rs` grades the *interval* between `/INT` and the first contended T-state, so moving both together leaves it green — measured, not argued. Fifteen other tests pin the convention against drift and none establishes it | Hardware that reports where `/INT` falls relative to something the emulator does not also define |
| **M5** — a `TYPE2` machine is **not** this machine with a 33-T-state window | The successor to the closed interrupt-window row below, which the sweep settled to a band. Three of the 68 rows resist at window 33: `group 3` contended (`R` 42 against 41), `group 7` uncontended (95 against 98), `group 34` uncontended (42 against 44). **`group 7` locates the residue**: its body ends `DI`/`EI`/`JR`, so its acceptance points are quantised, and the arithmetic puts an instruction boundary **exactly on** the frame's interrupt T-state — this machine takes the interrupt there and the hardware `TYPE2` machine did not. It cannot be reconciled with the detection row under any single integer-T-state change, because the two want **opposite tie-breaks**. So some second-order difference remains at that edge | A `TYPE2` machine's full 73-row hardware submission, or a Z80 interrupt-sampling model finer than one T-state. Neither is in hand, and neither is needed for the detection row, which is bit-exact |
| **M5** — the interrupt window's length is pinned to a **band**, not a point | Closed as "ungraded" below, but not to a single value: the oracle is green across **`17..=32`** and cannot separate those sixteen. 32 is the community's figure and is what this crate uses. **One boundary in the sweep is unexplained and is recorded rather than smoothed away**: at windows **14–16** the detection group still reads `TYPE1` while contended rows disagree, so something in the contended groups is sensitive to a short window and this gate does not say what. A boundary nobody predicted is worth more than one that was | A timing program that varies when it enables interrupts *within* the window — the band's interior is invisible to a suite that only needs the interrupt accepted at the top of the frame |
| **M5** — the 64-line pre-display count's **factors** | Its *product* is now measured — see the Closed table. Any compensating pair that still lands on 14335 would pass identically | Nothing available; it is the documented frame structure's reading of a measured total |
| The flag latch has almost no instrument | Two FUSE vectors (`37_1`, `3f`) are the **only** gate that can see it. `zexdoc` masks the bits off; `zexall` passes against three different rules including a stuck-at-zero latch | A corpus with a flag-setter → no-flag instruction → `SCF` sequence. Neither existing corpus has one |
| CI does not run the M3 gate | The workflow lives at **`ci/ci.yml`**, outside `.github/`, and `ci/README.md` beside it gives the one-line install and names the credential it needs. It is there because a push whose diff touches `.github/workflows/**` is refused by a token without `workflow` scope — the refusal is on the *path*, not on the branch, the content or the author. The copy that sat on the `ci-pending` branch was checked against `main` before being moved there and **would not have run**: it fetched the FUSE corpus from a mirror that is not the one `testdata/README.md` names and is not byte-identical to it, it named `Z80_FUSE_ALLOW_MISSING` as the live opt-out when both that spelling and `Z80_FUSE_REQUIRED` are now hard errors, it installed no ALSA headers so `crates/page` could not build on a Linux runner at all, and it ran none of `zexdoc`, `zexall` or the MEMPTR exerciser — the three `#[ignore]`d gates. **Those three used to end their `#[ignore]` reason with *"CI runs it that way"*, and that sentence was read by every developer who ran `cargo test` and skipped them.** It now names the command and says plainly that nothing runs it, which is the same correction as this row applied where a reader actually meets it: a register entry recording that CI is absent does not help someone who is being told, at the point of the skip, that it is present | Someone with `workflow` scope running `cp ci/ci.yml .github/workflows/ci.yml` and pushing it. Until then the gate is verified locally and enforced nowhere, which is the `Z80_FUSE_REQUIRED` defect again |
| **A green MEMPTR oracle is not 160 correct rules, and the gap has a number** | Every un-prefixed rule is implemented and `tests/memptr_oracle.rs` reads **0 of 160 groups failing** — but MEMPTR is observable only through bits 3 and 5 of `BIT n,(HL)`, so the exerciser grades each group's *aggregate* and cannot see a compensating pair of errors inside one. Two measurements bound it, both recorded at `FAILING_GROUPS` in that file rather than re-derived here: breaking the accumulator-store quirk — one line, three instructions wrong — reddens **four** `memptr_rules.rs` tests but only **two** oracle groups, so **`104 OUT (N),A` passes with its rule broken**; and groups **`113 JP (HL)` and `114 JP (XY)` failed while already correct**, needing no rule at all, and came green when the instructions the exerciser *sets up* with were fixed. **A group's verdict is not a report on its own rule, in either direction.** Separately, **`OTIR`/`OTDR` are covered by the repeat rule and graded by nothing** — the exerciser's self-overwriting `->NOP'` trick has no output counterpart, so no group reaches their repeat path; `crates/z80/src/instructions.rs` carries that caveat at `otir`/`otdr` | Nothing closes the first half: it is a property of a two-bit probe folded into a CRC, and `memptr_rules.rs` — 30 tests, one rule each, asserting the full sixteen-bit result — is the mitigation rather than the cure. Neither instrument subsumes the other, which is why both exist: unit tests cannot catch a rule nobody thought to write, and the oracle cannot say which rule is wrong. The `OTIR`/`OTDR` half needs a test program able to grade an output block repeat. **A third instance, measured 2026-09-01 and sharper than either of the two above: the whole `RET` family's rule is graded by NOTHING.** Deleting all three of its write sites — `return_from_interrupt`, `return_unconditional`, `return_conditional`, the only three of the six `set_memptr(target)` calls that are returns — leaves `memptr_rules.rs` at **30/30** *and* the exerciser at **`all tests passed.`**. Run in an isolated clone, the deletion proven by diff, with a positive control in the same clone (dropping the repeat rewind still moved 0 → 8) to show the harness bites. So groups `120`–`124` were among the 45 that came green **without their own rule being what fixed them** — the `JP (HL)` phenomenon again, and the reason *45 → 0* must not be read as 160 rules independently confirmed |
| **The I/O block repeat rule diverges from its own cited primary source, deliberately** | Boo-boo and Kladov's 2006 document — the source every other rule in this core is quoted from — exempts the I/O block forms: *"INIR — exactly as INI on each execution"*. **That is wrong.** David Banks traced the repeat's extra M-cycle on real parts (one Zilog NMOS, two NEC NMOS), MAME adopted it, and Patrik Rak's exerciser carries CRCs expecting it, so the uniform rule is implemented on that evidence — which collapsed a per-family flag into a constant and let it be deleted. The mutation confirming it, and the two groups it moves that the 2006 document denies, are recorded in `tests/memptr_oracle.rs`'s module doc rather than restated here | Nothing pending. Recorded because a deliberate divergence from a cited source reads as a defect to whoever finds it next, so the reason has to be reachable from the divergence. **Also not modelled:** the **KP1858BM1 / T34BM1** clones write **zero** into MEMPTR's high byte where an NMOS part writes `A` — exactly `LD (nn),A`, `LD (rp),A` and `OUT (n),A`. Modelling them needs a part-variant switch on the public API, and nothing in this emulator asks for one |
| **`crates/frontend`** — which membrane key a **non-US keyboard** produces differs between `miniquad`'s backends | Read out of the pinned `miniquad-0.4.11` source on 2026-09-01. The browser (`js/gl.js:1215`, `event.code`), Windows (`windows.rs:694`, `HIWORD(lparam) & 0x1FF` — a **scan code**, not the virtual-key code) and macOS (`apple_util.rs:154`, `NSEvent.keyCode`) all key off the **physical position**; X11 (`linux_x11/keycodes.rs:10`, `keysyms[0]`) and Wayland key off the **layout's keysym**. On AZERTY the key printed `A` is `KeyCode::Q` on the first three and `KeyCode::A` on the last two. **Not fixable in `keymap.rs`** — the table is downstream, and by the time a `KeyCode` arrives the distinguishing information is gone. It is `derived` from five mapping tables; **no key has been pressed** | One person on AZERTY or QWERTZ pressing one key, in a browser and on Linux. **Nobody here can perform it**, and no US-layout tester can see it: on a US board all five backends agree |
| **`crates/frontend`** — which `Ctrl` chords a browser lets the page cancel | `docs/M8.md` Decision 2 turns on it and its class is **derived**, written from familiarity. What is not in doubt is that the uncancellable class is non-empty on every desktop browser, which is all the design needs; what is in doubt is every specific row. **`Ctrl+8`/`Ctrl+9` are the load-bearing ones** — they are `SYMBOL SHIFT`+`8`/`9`, i.e. `(` and `)`, which no BASIC program avoids | A person at a physical keyboard, in Chrome and Firefox, on three operating systems, on a page that logs `keydown` and reports whether `preventDefault` suppressed the browser's action. **Explicitly not settleable by any automation available here** — see *A verification that cannot ask its own question*, below |
| **`crates/frontend`** — `wasm32` is neither built nor run | The crate is *designed* to be portable and the design is structural rather than asserted: `grep -rn "cfg(target" crates/frontend/src/ crates/spectrum/src/ crates/z80/src/` returns nothing, so the code `cargo test` runs and the code a browser would run are the same code. That is a real and useful property **and it is not a build**. ~~`host.rs`'s claim that both non-portable functions *"compile for `wasm32-unknown-unknown` as they stand"* is its author's and has been re-derived by nobody.~~ **Built, linked and run on 2026-09-01** — `crates/frontend/src/lib.rs` and `web/README.md` both record it, and this row did not move when they did. What survives is narrower and is the half that was always the gap: one browser, one operating system, one machine, one afternoon, and **`web/gate.sh` grades a compiler, a linker and a module's import table without observing a single pixel**. There are also now **four** target-gated entry points in `crates/page` rather than two, and the audio pair is called from `main.rs` rather than through `host.rs`, so the seam is no longer enumerable from one file | ~~`cargo check --target wasm32-unknown-unknown -p frontend`, then `cargo build`.~~ Both run. What is left needs a second browser, a second operating system, and a person: the same condition as the rows below |
| **`crates/frontend`** — what the crate's own *not gated* table lists | *"Whether it looks right"*, *"whether it is pleasant to type on"*, *"whether the motion is smooth"*, *"that the window opens at all"*. `src/lib.rs` carries them in this document's format and does not soften them. **These are not open questions awaiting a technique** — `M8.md` puts it exactly: the thing the frontend adds is *"a browser, a GPU, a keyboard and a person, and none of those four is gradeable by anything in a repository"* | Nothing, structurally, for the first three. A person, recorded — browser, version, operating system, what was typed, what happened, date — exactly as M6's T4 requires. **An unrecorded observation is indistinguishable from one nobody made** |
| **`crates/frontend`** — the `.wasm` payload size, and what `opt-level = "z"` costs in frame rate | `[profile.release]` is `opt-level = 3`, `lto = "fat"`, `codegen-units = 1` — tuned for a native binary where payload is free. **Neither number has been measured** | Building both and measuring both. Neither has been done, and the figure must be published with its command and date rather than as an integer — `MACHINE.md` has three stale integers on record and the fix was the same every time |
| **`crates/frontend`** — the audio queue's **setpoint** is a ruling nothing measures | `audio::queue_target` steers the device's backlog at **half** of `page::BUFFER_MILLISECONDS`. Its arithmetic is asserted (`the_setpoint_is_half_the_device_buffer`) and the *choice* of one half is not: a quarter biases the loop toward underrunning, three quarters toward latency, and no instrument here distinguishes them. It was an expression in the frame loop — `* BUFFER_MILLISECONDS / 2000`, a unit conversion and a policy multiplied together — which is why it had no test at all until 2026-09-03 | A person with speakers, comparing the three. This is the same wall as every other audio row: **this environment has no audio device** |
| **`crates/frontend`** — the loop's feedback signal is **stale in a browser and exact on the desktop** | `Resampler::track` runs once per emulated frame, every 20 ms. On the desktop the depth is measured inside the lock that inserted the samples. In a browser it comes back from the worklet's periodic report — `REPORT_EVERY = 8` render quanta, ~21.3 ms at 48 kHz — plus a `postMessage` hop. **8 and not 16 because at 16 the refresh was ~42.7 ms against a 20 ms control period and a 50 ms setpoint**, i.e. the loop applied the same error twice before seeing its own effect, which is the textbook shape of a limit cycle and would be heard as pitch wobble. The *systematic* half is closed — `zx_audio_push` now reports `pushed − consumed`, so the chunk being handed over is counted — and the residual is the worklet's drain since its last report, which can only make the answer too large | A browser, an ear, and a tone held long enough to hear warble on. Arithmetic bounds it; nothing here has listened to it |
| **`crates/frontend`** — the browser's audio bound is a **backstop plus a ring**, and only the ring is on the audio thread | The frame loop refuses to push past twice the setpoint — a full `BUFFER_MILLISECONDS` — on both targets, and each device drops below that: `desktop::push` at its ring's capacity, `zx_audio_worklet.js` by advancing its read index when its 16384-sample ring fills. **The browser had no bound at all between 2026-09-02 and 2026-09-03**: the frame-loop ceiling was removed in favour of the rate loop and the worklet was an unbounded array, so a machine running faster than the loop's 0.5% authority grew latency and `Float32Array`s for as long as the tab lived. Restored as a backstop rather than as the mechanism, which is the distinction the removal got right and the deletion got wrong | Nothing pending for the bound itself. What is **not** graded is the worklet: it is JavaScript, and no test in this repository executes any |
| **The M1 fetch drives `PC` for all four T-states, where hardware drives `IR` for two** | The successor to the closed *contention within a cycle* row below. `fetch_opcode` (`crates/z80/src/lib.rs`) charges `internal_cycles(pc, OPCODE_FETCH_T_STATES)`, so `PC` sits on the bus for T1–T4. A real Z80 drives `PC` for T1–T2 and the **refresh address for T3–T4** — stated in `ARCHITECTURE.md:113-115` and in `crates/z80/tests/common/report.rs:158-160`. On a 48K this is a live quantity, not a curiosity: `IR` usually points into contended RAM. **Nothing grades it, and one gate now asserts the simplification** — see the contradiction recorded beneath the Closed table | Hardware, or a corpus that contends inside M1. The FUSE corpus **cannot**: `report.rs:148-150` records that it contends only at cycle starts, so it does not adjudicate T3–T4 |

**Not audited in this pass:** *resolved-target refactor*, *`WZ` / MEMPTR* and *contention within a
cycle* were carried forward as written and were **not** re-checked against the crate. `WZ` in
particular has passed the milestone its row names as its settling condition, so treat its state as
unverified rather than current. Whoever next opens this register should re-derive all three; the
`panic_bounds_check` correction in `ARCHITECTURE.md` is what an unaudited carry-forward costs.

**The M5-closing pass did not re-check them either**, and says so rather than letting a second
silent carry-forward look like a second audit. The three M5 rows that left the register in that pass
were each re-derived against the crate; these three were not touched, and the note above now applies
to two consecutive passes.

**The M6 pass did not re-check them either — three consecutive passes now.** It is said again rather
than dropped, because three silent carry-forwards in a row is the point at which the note stops
being a caveat and starts being the finding: **nothing in this process re-audits a row that nobody
is currently working on**, and the rows most likely to have rotted are exactly the ones nobody is
working on. What M6 *did* produce is a neighbour for one of them. The `WZ` / MEMPTR row says it is
*"carried in `CpuState`, never written"*; M6 adds that **every snapshot load destroys it** — no
format `MACHINE.md` names carries the field, so both parsers set zero — which is a fact about a
different subject and settles nothing about the row. Two adjacent rows about one field is a signal
that the original was carrying more than one claim, and whoever next opens this register should
start there.

**The fourth pass audited all three, and every one of them was rotten.** The three notes above are
left standing because they are the record of the defect, and the defect turned out to be worse than
they feared: it was not that the rows *might* have gone stale, it is that **all three had, and two
of them were false rather than merely out of date**.

- *Resolved-target refactor* — **closed, and git dates the closure to M2, the milestone the row
  itself named as its deadline.** It describes three functions; `read_operand` and `write_operand`
  exist in **no crate** — 5 occurrences at M1 (`01c93ac`), none at M2 (`8021b4f`), where
  `enum Target` already sits at `decode.rs:146`, the line it is on today. A row can go stale enough
  to name nothing, and this one did so on the day it came due.
- *`WZ` / MEMPTR* — **its central claim was false.** *"Never written"* was contradicted by
  `fn indexed_address` (`instructions.rs:795`, `self.set_memptr(effective);`), by
  `crates/z80/src/lib.rs:1484` asserting the written value
  (`assert_eq!(cpu.state().wz, 0xA381, "the indexed access recorded MEMPTR")`), by four
  `CORPUS_OMISSIONS` whose `reason` fields read *"MEMPTR gives 0, the corpus expects…"*
  (`crates/z80/tests/fuse_vectors.rs:213`, `:219`, `:225`, `:231`) and which exist *because* it is
  written, by the crate's own `//!` block at `lib.rs:69-72`, and by `CHANGELOG.md:499` announcing
  the change — *"`MEMPTR`/`WZ` is now **live rather than inert**"*. Every one of those contradicts
  the row, and the register outlasted all of them.

  **The crate is not divided against itself, and the claim that it was is the more useful
  correction.** This bullet used to end by naming a holdout — *"`lib.rs:326` still carries the old
  sentence"* — and there is no such sentence: `lib.rs:326` is a blank line, and the nearest prose
  about the field, the `//!` block at `:69`, says the **opposite** of what it was accused of
  saying. So a reader was being sent to repair something already right, which is the failure mode
  the phantom-guardian family usually produces in its milder form and is not milder at all: a
  false accusation costs a reader the time to disprove it, and a plausible one gets acted on.
- *Contention within a cycle* — **its premise was false and its remedy already existed.** The gate
  the settling condition asked for is `every_t_state_reports_the_address_the_z80_drives`, and it
  pins whole runs rather than cycle starts.

**The number worth carrying forward is that two of three were false, not stale.** The note above
predicted rot; what it found was contradiction. A row nobody re-reads does not drift gently out of
date — it keeps asserting, with the register's authority, a thing the code stopped doing. The
milestone-shaped process this document describes has no step that re-opens a row nobody is working
on, and writing the caveat four times has not supplied one. **What would: give every row a claim
that can be mechanically checked** — the `read_operand` row would have been caught by a grep in a
second, and `WZ` by a `rg 'self\.wz\s*='` returning a hit where the row says there is none.

### Closed — items that left the register, and what closed each one

An item leaves the Open table only into this one, with its evidence. A row that simply disappears
is indistinguishable from a row nobody re-read.

| Item | What it was | What closed it |
|---|---|---|
| **M1 fetch vs operand read** | *"Not blocking. Contention depends on address and `t mod 8`, both of which the machine has. A defaulted `fn fetch(&mut self, addr) -> u8 { self.read(addr) }` is non-breaking whenever a debugger or a precise floating-bus model wants it."* | **The reasoning was wrong and M5 measured it.** `LD A,B` and the read-modify half of `INC (HL)` emit byte-identical streams — `read(addr)` then four `tick(addr)` — while owing one contention point and two respectively, so address and phase are not sufficient however true it is that contention depends on them. `crates/spectrum/src/machine_cycle.rs` had to reconstruct the boundaries by deferral, at a residual of one contention point on the read-modify-write family — **an isolated stall of 0–6 T-states, which is not the same quantity as the observable error; see the correction below the table.** `Bus::fetch` landed in `crates/z80/src/bus.rs`, defaulted, with every M1 opcode fetch routed through it. **Both halves are now closed.** `crates/spectrum/src/ula.rs:766` — `fn fetch(&mut self, address: u16) -> u8` — implements it, `machine_cycle.rs` is deleted, and the residual is gone rather than pinned: with every cycle's length disclosed the moment it opens, there is nothing left to reconstruct. Full account in [`MACHINE.md`](MACHINE.md); the two rulings it forced are in [`Z80-REFERENCE.md`](Z80-REFERENCE.md) |
| **M5 — read-modify-write contention residual** | *"One contention point, 0–6 T-states, per instruction that performs exactly one internal cycle at the address it just read. Pinned by a test that asserts the loss rather than hiding it. **Now closable**: `Bus::fetch` has landed in the CPU."* | `Ula` implemented `fetch`. Closed by the row above and with it — the two were one item split across two tables, which is itself worth noticing: the Open row's settling condition named `Ula` implementing `fetch`, and it does. `machine_cycle.rs` and its residual-pinning test are deleted together. The **quantity** the heuristic lost was one contention point; the **error** it produced was 0 or 1 T-state — see the correction below |
| **M5 — five mutations leave the boot gate green** | *"`/INT` never asserted; the keyboard reporting every key held; the ROM slot made writable; contention removed entirely; contention phase off by one… Until they land, the boot gate grades the memory map's read side and the screen, and nothing else."* | **Closed by being re-measured, and the re-measurement changed the finding.** Those verdicts were taken against the boot *example*, which nothing runs. Re-measured at `2157331` against the pre-gate lib target, **four of the five were already red** from unit tests inside `src` — 5, 7, 1 and 13 failing tests — and **the contention-phase mutation is the only survivor of the five**. (A keyboard-matrix permutation also survives and is why `keyboard_matrix.rs` exists, but it was never one of the five; this row said "three" and "two survivors" and both were wrong — see the M5 section.) Seven gates now exist and ten mutations turn them red — the tables are in the M5 section above |
| **M5 — nothing runs the boot gate** | *"It is `crates/spectrum/examples/boot.rs`; `cargo test` builds an example without calling `main`. Deleting the committed ROM left the suite at 72 passed."* | `crates/spectrum/tests/boot.rs` exists and runs the ROM under `cargo test`, asserting both the message **and the frame it appears on** — the one number the example printed and never checked. The example remains, as an example. See *A gate that nothing runs, for the third time*, which this closes |
| **`Q` latch — latch lifecycle** | *"`((q ^ f) \| a) & 0x28` has landed, but `begin_operation()` zeroes `q` before `SCF`/`CCF` read it… **FUSE is red: 288/290**… **Blocks M3**."* | The `q_prev` fix landed in `crates/z80/src`: `begin_operation` now assigns `self.q_prev = self.q` before clearing, and both `SCF`/`CCF` call sites read `q_prev`. Re-measured here rather than inherited — `cargo test -p z80 --test fuse_vectors` reports **290 executed, 290 passed, 0 failed** and **1045 executed, 1045 passed, 0 failed**. The row had been red in the register through two merged milestones. **What it does *not* close** is the row above it: the rule is still graded by two FUSE vectors and nothing else |
| **`Cpu<B: Bus>` struct-level bound** | *"Downstream types naming `Cpu<Ula>` must carry `where Ula: Bus`… Removable at any time."* | Removed. The declaration is `pub struct Cpu<B>` |
| **M5 — `timing::FIRST_CONTENDED_T_STATE = 14335` has no oracle** | *"Pinned to `64 × 224 − 1`, a derivation from documented frame structure. Nothing measures it against hardware… `contention_phase` pins it against drift; it does not establish it."* The row also said **an issue 2 machine is one T-state earlier**, which is on the wrong axis and is corrected below | **`tests/timing_oracle.rs`** — Richard Butler's 48K timing suite, a `.z80` snapshot carrying 37 machine-code groups — 34 over the documented instruction set, and three sharing one program — and **two** expected-result tables measured on real Spectrums, run contended and uncontended: 70 hardware rows, 0 disagreements, and the machine reproduces `TYPE1 (Early)`. **Green on the first run, so thirteen mutations were run against it**, each landed and each restored from held bytes, and **three more arrived with group 35 — sixteen in total.** The thirteen is left as it was measured: it is the count taken that day against the 68-row range, and re-stating a dated verdict against a range it never ran on asserts something nobody measured. What grew is the live figure, and it grew by three rather than one because the `(true, false)` arm was edited three separate ways — deleted, weakened to the two-stall shape, and the fourth term dropped alone — which [`MACHINE.md`](MACHINE.md)'s standard counts as three mutations, *an edit, not a verdict and not a constant*. `FIRST_CONTENDED_T_STATE` at 14333/14334/14336/14337 mismatches 13/8/7/7 hardware rows and only 14335 is clean; at 14336 the machine scores **7 against Early and 64 against Late — it does not become the other machine, it stops being either.** Deleting the delay pattern is 38 rows, `fetch` as a 3-T read 29, internals never contending 34 — all of them taken against the 68-row range that preceded group 35, and carried rather than re-run, because a mutation verdict carries the gate it was taken on. **The scope is narrower than the row's disappearance suggests and is stated in the three new Open rows above:** what is measured is the *interval* from `/INT` to the first contended T-state, because moving the origin and the window together leaves the oracle green |
| **M5 — the interrupt window's *length* is ungraded** | *"32 T-states. `tests/frame_boundary.rs` pins **both** edges against drift with two literals (31 accepted, 32 missed) and is the only test in the workspace that 32 → 24 reddens. The oracle is green at 24, so it cannot settle this."* | **`tests/timing_oracle.rs` — and the row's own evidence was one sample mistaken for a band.** 32 → 24 leaving the oracle green was read as *"the oracle cannot measure this"*. Sweeping the window over **1–65** with the contention constant fixed shows what it actually was: the green band is **`17..=32`**, and 24 sits inside it. Below 17 contended rows disagree; at **33** the machine's whole classification flips from `TYPE1 (Early)` to `TYPE2 (Late)` and **65 of the 68 graded rows follow**; from **44** the suite stops terminating. **Both edges were derived from the suite's own code *before* the sweep ran, and both measured exactly there.** The mechanism is the closure's real content: the suite installs a handler beginning `INC C` / `EI` / `RET C`, and because **`EI` does not re-enable interrupts until after the following instruction**, the earliest the CPU can see `/INT` again is `19 + 4 + 4 + 5 = 32` T-states after accepting the last one — *exactly* the far edge of a 32-T-state window. If that tie resolves as "still asserted", a second, nested interrupt is taken, eight fewer four-T-state iterations fit, and `2 − 8 ≡ 122 (mod 128)`: the tables' 120 gap. **That retires a puzzle rather than merely correcting it.** The suite's authors record a board reporting Late when cold and Early once warm, and issues 3B, 4B and 6A in *both* classes — nonsense as "one T-state of contention", exactly what you would predict as an **edge race in the interrupt**. Any window shorter than the tie point behaves identically, which is why 24 was invisible. Two things stay open in the table above: the three rows that resist at 33, and the unexplained **14–16** boundary |
| **M1 — resolved-target refactor** | *"`read_operand`, `write_operand` and `tick_read_modify_delay` each recompute `pair(base)` independently. Free for `(HL)`; for `(IX+d)` the displacement must be fetched once and the addition charged once."* Settled by: *"**M2's opening move.** Needs a `Register(RegIndex) \| Memory(u16)` computed once and threaded"* | **Landed, in exactly the shape the settling condition names.** `crates/z80/src/decode.rs:160` is `pub(crate) enum Target { Register(RegIndex), Memory(u16) }`. `instructions.rs:831 fn resolve` is the single point that determines a memory operand's address: the displaced branch calls `fetch_signed_byte()` **once** and `indexed_address` **once**, and `fn indexed_address` (`:848`) performs the addition once. `fn read_target` (`:877`), `fn write_target` (`:885`) and `fn tick_read_modify_delay` (`:927`) all take `Target` **by value**; `fn increment_operand` (`:1169`) threads one `Target` through read → delay → write, and `fn execute_cb` (`:725`) does the same for `DDCB`. `fn tick_index_computation` (`:870`) charges the addition once, at four sites each guarded by `matches!(target, Target::Memory(_))`. **Each of those eight carries its declaration rather than a bare number, and that is a repair rather than a flourish:** the row previously offered `:638`, `:655`, `:677`, `:685`, `:693`, `:703`, `:912` and `:543`, every one of them stale by between 133 and 170 lines and every one landing on unrelated code — an evidentiary apparatus that had drifted off its subject entirely inside a row whose own thesis is that a claim must be mechanically checkable. A line number paired with `fn resolve` can be checked by grep; a line number alone can only be believed. **The decisive check is negative, and git dates it exactly.** `rg '\bread_operand\b\|\bwrite_operand\b'` across the tree matches **nothing in any crate** — every hit is prose in this document. `git grep -c read_operand 01c93ac -- crates/z80/src/` gives **5 occurrences in `instructions.rs` at M1**; the same grep at `8021b4f` — **M2** — gives **none**, and `git grep 'enum Target' 8021b4f` puts it at `crates/z80/src/decode.rs:146` — **the line it occupied then**, fourteen above the 160 it occupies today, and the drift is the point rather than a flaw in the check: what git found is the *declaration*, and a declaration is what a grep finds again at whatever line it has moved to. So the refactor landed in the very milestone the row named as its settling condition, *"M2's opening move"*, and the row then survived M2, M3, M4, M5 and M6 unchanged. **Five milestones describing three functions that had already been deleted when the row's own deadline arrived** |
| **M1 — contention within a cycle** | *"Only cycle *starts* are pinned; nothing asserts the address holds constant across a cycle's remaining T-states. It does — but by implementation, not by gate."* Settled by: *"One assertion over `tick_addresses` between consecutive transfers, if it earns its place"* | **The gate exists and the row's premise was false.** `crates/z80/src/lib.rs:1055`, `every_t_state_reports_the_address_the_z80_drives`, asserts the **entire per-T-state address stream** — one `assert_eq!` over the whole `Vec<u16>` — for seven instructions: `ADD HL,BC`, `INC BC`, `JR e`, `CALL nn`, `RST 38h`, `EX (SP),HL`, `OUT (n),A`. It is written as *runs*, e.g. `stream(&[(0x0000, 4), (0x0001, 7)])`, so within-cycle constancy is what it pins, not cycle starts. That is precisely the assertion the settling condition asked for. What *is* start-only is the **FUSE corpus comparison** (`crates/z80/tests/common/report.rs:168`), deliberately and correctly — see the contradiction below, which is the sharper item this closure exposed |
| **MEMPTR — nothing grades the *value*** | *"And nothing grades the value: FUSE has no MEMPTR column. The instrument exists, is documented, and is neither committed nor run — `tape_corpus.rs` sweeps `z80memptr.tap` as tape-format corpus (pulse-train decode + parity) and **never executes it on the CPU**. Running it through the ROM loader the way M6's T3 runs a `.tap` is what would settle it."* | **`tests/memptr_oracle.rs`, which is exactly the settling condition the row named.** Patrik Rak's `z80test` v1.2a MEMPTR build is loaded from `z80memptr.tap` by the **real ROM's `LD-BYTES`** off the `EAR` bit — all four blocks, 309,241,005 T-states of emulated tape out of a 921,998,240-T-state run — then run on the whole machine, printing through the ROM into the display file, with the verdict read back by `screen::read_text` against the glyphs of the machine's **own** character set. It reports **`Result: 045 of 160 tests failed.`**, and the 45 are pinned **by name** so a rule landing is a red gate rather than a silent improvement. **Green was proven able to fail, by two mutations, each counted before the write and re-read after it, each restored from held bytes.** Deleting the sole `wz` write — `instructions.rs:660` on the day it was measured, which is `fn indexed_address`'s `self.set_memptr(effective)` at `:795` since the single-writer refactor — takes the failing count **45 → 143** (exit 101). Flipping **one byte** of the tape's 14390 — the CODE block's parity — makes the ROM refuse the block and the gate stops at `LOAD_FAILED` before reading any verdict (exit 101), which is this gate's own proof that the program arrives through the loader and not by some other route. **What it does *not* close is the row above it:** the rules are still unimplemented, and *a passing group is not a correct rule* — the first mutation demonstrates that directly, since stranding MEMPTR at zero made **17 currently-failing groups start passing**. A verdict identical under several implementations is evidence for none of them, which is `zex_oracle.rs`'s claim 2 in a second setting. **Since superseded, and the stale half is the clause about the row above:** *"the rules are still unimplemented"* was true when written and is not now — that row closed on 2026-09-01 and is two rows below. The verdict quoted here, `045 of 160`, is this pass's record and not a live figure; the gate reads `all tests passed.` today. The caveat this row ends on did **not** expire with the defect — it is restated, with a number against it, in the Open row *A green MEMPTR oracle is not 160 correct rules* |
| **M1 — `WZ` / MEMPTR** | *"Carried in `CpuState`, never written."* Settled by: *"M4, when `BIT n,(HL)` first makes it observable"* | **The row's claim was false and its settling condition is met — but only for one of MEMPTR's rules, so it closes into the narrower Open row above rather than outright.** `fn indexed_address` (`crates/z80/src/instructions.rs:853`) writes it on every indexed effective-address computation — `self.wz = effective;` when this row was written, `self.set_memptr(effective);` since the single-writer refactor — and `fn test_bit` (`:775`) reads the high byte back for **both** memory forms of `BIT n,s`. It is observable and *observed*: `crates/z80/tests/fuse_vectors.rs` carries four `CORPUS_OMISSIONS` for `BIT 1/3/5/6,(HL)` at `:213`, `:219`, `:225` and `:231`, each with the stated reason *"MEMPTR gives 0, the corpus expects…"*, and `crates/z80/src/lib.rs:1527` asserts `wz == 0xA381` after an indexed access. `CHANGELOG.md:499` recorded the change — *"`MEMPTR`/`WZ` is now **live rather than inert**"* — and the register never caught up. The residue is that **one** site writes it and nothing grades its value |
| **MEMPTR is written at exactly one site** | *"`rg 'self\.wz\s*=' crates/z80/src` still returns **exactly one hit**, so every *other* hardware rule — `LD (nn),A`, `JP nn`, `CALL`, `RET`, `EX (SP),HL`, the repeating block ops, `IN`/`OUT`, `ADD/ADC/SBC HL,rr` — is **unimplemented**"*, at a measured cost of **45 of 160** groups | **Every un-prefixed rule in Boo-boo and Kladov is implemented**, behind the single writer `Cpu::set_memptr`. `grep -rn 'self\.set_memptr(' crates/z80/src \| wc -l` returned **28** call sites on 2026-09-01 — 26 in `instructions.rs`, 2 in `lib.rs` for interrupt acceptance and NMI — and `self.wz =` appears nowhere but `set_memptr` and `set_state`. `tests/memptr_oracle.rs` moved **45 of 160 → 0 of 160**, printing `Result: all tests passed.`, and `tests/memptr_rules.rs` grades each rule's exact sixteen-bit result on its own. **The row's self-contradiction closed with it:** `lib.rs` no longer documents the field as *"not yet updated by any instruction"*. What did **not** close is the two narrower Open rows this opened — the oracle's blind spot, `OTIR`/`OTDR`, and the clone variant |
| **`crates/spectrum`** — sound is on `Ula::tick`'s path now, at one `bool`, and the figure is a benchmark rather than a claim | *"A **numeric** assertion. `benches/frame.rs`'s hot-path claim is still prose, and prose does not go red — the next regression lands on whatever baseline the last one left. A divan threshold against a recorded median, run by something, is what closes it, and it is blocked on the same missing CI as the row above"* | **`web/gate.sh`'s PERF step, on 2026-09-03 — and the estimator is deliberately not the one this row asked for.** The step holds `quiet_48k` — one frame of `NOP`s with nobody touching sound, the invariant the +23 % regression broke — to a recorded ceiling: the lowest per-run `fastest` of three bench runs must stay under **138.7 µs × 1.15**, the floor recorded 2026-09-03 (`hw.model` Mac15,9, rustc 1.98.0), bit-stable across seven undisturbed runs in two independent sessions. Not the *median* the row named, and the derivation at the step records why: under a sustained load of 11, three consecutive runs' medians all sat at exactly ×1.10 of baseline while the same runs' floors stayed at 131.9–142.2 µs — the floor is the estimator that survives load, and the ceiling it is held to still catches a P1-class regression, +19.6 % on this case, reproduced on the floor. The step was proven able to fail before it was trusted — three mutations, each taken and each restored — and on any machine that is not the baseline's it **skips and is counted as an unanswered question**, so the verdict cannot say *green* for a question it did not put. **What did *not* close is the "run by something" clause, and it is narrower than the missing-CI row alone carries.** Nothing runs `gate.sh` automatically — the Open row *CI does not run the M3 gate* stands unchanged, `ci/ci.yml` uninstalled for want of `workflow` scope — and installing it would still not run *this number*: the workflow's gate job runs on `ubuntu-latest`, which is not the baseline's machine, so the PERF step would skip there, counted. Automating it needs a runner that is the baseline's hardware, or a baseline recorded on the runner's own — until then the instrument is pre-push, run by whoever runs the gate |

> **What of that oracle row this pass derived, and what it carried across.** The **workspace**
> verdicts were re-taken here: `FIRST_CONTENDED_T_STATE` at 14334 and at 14336 each redden
> **three** tests — `contention_phase.rs`, `timing_oracle.rs` and `frame_boundary.rs` — and the
> combined origin shift (`/INT` one T-state later *and* the window at 14336) reddens **fifteen**
> with `timing_oracle.rs` **not among them**, which is the measurement the row's scope rests on.
> The **per-row hardware mismatch counts** — 13/8/7/7, 38, 29, 34 — are the oracle's own internal
> figures and were taken by whoever wired that gate; they are recorded here as theirs, not
> re-derived. Do not read the two columns as the same quantity: one counts failing *tests*, the
> other counts disagreeing *hardware rows inside one test*, and the earlier draft of this note
> conflated them.
>
> **Correction — *"an issue 2 machine is one T-state earlier"* is the wrong axis, and it was
> repeated in three places.** The suite's own results database has the early/late split spanning
> board issues 2, 3, 3B, 4A, 4B and 6A, with **3B, 4B and 6A appearing in both classes** — and the
> authors record one machine reporting Late when cold and Early once warm. So the difference is
> not a property of the board revision at all. The sentence stood in this document twice (the
> *Still ungraded* row and the Open register row, both now struck) and still stands in
> `crates/spectrum/src/timing.rs`, which is another agent's file and is routed separately.
> Choosing `TYPE1` is choosing a **behaviour** that real machines exhibit, not choosing an issue
> number.

> **Correction — "one contention point (0–6 T-states)" conflated two different quantities, and the
> conflation cost a wrong number.** That phrasing appeared in this register, in `CHANGELOG.md` and in
> the M5 commit message, and it reads as though the heuristic's error was up to six T-states. It is
> not. **0–6 is the *isolated stall*** — what `delay(t)` charges a single cycle, taken alone. **The
> *net observable error* on `INC (HL)`, swept over all 448 start positions, was 0 or 1 T-state and
> never more**, because dropping a stall opens the following cycle earlier, where the pattern
> usually charges most of it straight back.
>
> This is not cosmetic. Two agents independently derived `INC (HL)`'s contended cost and got 26 and
> 30; the 30 came from taking the observed 25 and adding "the missing 5", which is exactly the
> arithmetic the 0–6 phrasing invites. The full account is *A missing stall cannot be added to a
> total*, below. The wording is corrected wherever it appears rather than deleted, because a reader
> who has already acted on it needs to find out that they did.
>
> **It cannot be corrected in `2157331`'s commit message**, which is the second time this document
> has had to say that about the same commit. A commit message is not a register: the 0–6 phrasing
> stands there permanently, in the same file that named the boot gate's coverage against a run that
> never happened. Anyone reading the M5 commits should read this section alongside them.

> **Closing *contention within a cycle* exposed a contradiction between two files in one crate, and
> it is the more interesting half of that closure.** `crates/z80/tests/common/report.rs:152` carries
> a heading — *"# Do not 'close' that gap by asserting the address holds constant across a cycle"* —
> under which it argues that such an assertion *"would pass today"* but **"must not be added"**,
> because *"a real Z80 drives `PC` for T1–T2 of an opcode fetch and the **refresh address for
> T3–T4**… This core currently drives `PC` for all four"*, so the assertion *"would not be testing
> the Z80; it would be freezing our present simplification into the gate, and would fire as a
> **false failure** the day somebody makes M1 hardware-accurate."*
>
> **`crates/z80/src/lib.rs:1055` makes that assertion.** `every_t_state_reports_the_address_the_z80_drives`
> opens `ADD HL,BC` with `stream(&[(0x0000, 4), (0x0001, 7)])` — `PC` across all four M1 T-states,
> then `IR` across the seven internal ones. The same test therefore *knows* the refresh address
> (`0x0001` is `IR` after `increment_r`) and drives `PC` through the two T-states where hardware
> would carry it. Its **name** asserts it reports "the address the Z80 drives"; for T3–T4 of M1 it
> reports the address this core drives.
>
> **Both readings are defensible and that is the point** — this is not a bug report. `report.rs` is
> refusing to bake the simplification into the *corpus comparison*, which grades us against an
> external oracle; `lib.rs` is pinning the *current* stream so it cannot change unnoticed, which is
> what a characterisation test is for. What is not defensible is that neither file mentions the
> other, so the second one silently does the thing the first one forbids by name, and the register
> recorded the gap as unclosed while the assertion had been sitting in `src` all along.
>
> **Verified here, not inherited:** `fetch_opcode` (`crates/z80/src/lib.rs`) calls
> `internal_cycles(address, OPCODE_FETCH_T_STATES)` with `address = self.regs.pc()`, which is the
> mechanism both files describe. **Not verified here:** that hardware drives `IR` on T3–T4 — that is
> read from `ARCHITECTURE.md:113-115` and `report.rs:158-160`, both of which state it without a
> measurement, and neither of which this pass could check against a Z80. Whoever has a datasheet or
> a logic analyser is the person to settle it. `crates/z80/**` is another agent's tree and **nothing
> in it was edited by this pass.**

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
  **The headroom is ~160×, not 500×** — the 500 is the unreproducible batched-tick figure
  corrected under *Measured*, above, and the number that governs a frame loop is the paged,
  contended bus. The ruling is unchanged and the arithmetic still favours it by three orders of
  magnitude; the figure is corrected because leaving a falsified number inside a live
  justification is how it gets quoted again.
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

> **That sentence is false on this branch and the register two hundred lines above it has said so
> the whole time.** Verified on 2026-09-01: `ls .github` reports no such directory, and
> `git ls-tree -r --name-only HEAD -- .github` is empty at `b3e89ad`. The workflow **is** committed
> — on the branch **`ci-pending`**, at `1604af7` (*"the workflow, parked on a branch until the token
> has workflow scope"*) and `55224e0` — and it is not reachable from `m5-followup` and has never
> run. So the accurate statement is: *the workflow is written, reviewed and parked; no pipeline
> executes it; the M3 gate remains verified locally and enforced nowhere.*
>
> **The shape is worse than the wrong sentence.** This document's own headline defect is *"an
> `#[ignore]`d gate that no pipeline executes is not a gate"*, and here the remedy paragraph
> asserts the remedy shipped while the Open register, `:528` and the row in *Open — the
> authoritative register*, all say it did not. Three places, one fact, and the one a reader reaches
> first is the one that is wrong. A remedy sentence is a **claim about the present**; the register
> is the record of it; when they disagree the register wins, and the remedy sentence should have
> deferred to it rather than restated it in the past tense.
>
> Corrected here rather than deleted, because a reader who acted on *"CI now exists"* — by not
> checking whether their change was gated anywhere — needs to find out that they did.
>
> > **The location above has since gone stale, and it is corrected rather than rewritten because
> > it is a pointer somebody would follow.** The `ci-pending` branch is deleted; the workflow is
> > at **`ci/ci.yml`** with `ci/README.md` beside it, which says why it cannot live in
> > `.github/workflows/` and gives the command that installs it. Nothing else in this note
> > changes: no pipeline executes it, and the M3 gate is still verified locally and enforced
> > nowhere.
> >
> > What the move surfaced is worth more than the new path. The parked file was checked against
> > `main` before it was moved and **it would not have run** — its FUSE mirror was not the one
> > `testdata/README.md` names, its opt-out variable had been renamed and the old spelling made
> > an error, its runner was missing a system library `crates/page` cannot build without, and it
> > ran none of the three `#[ignore]`d gates. **That is the cost of parking a file, and it is a
> > different cost from the missing scope.** A branch that cannot be pushed does not hold still;
> > it rots against a tree that keeps moving, and every day it waits the thing it will eventually
> > install is less true. Worth separating from the scope problem, because only one of the two is
> > fixed by finding the right token.

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

### A measurement with no recorded method cannot be defended, only re-taken

`ARCHITECTURE.md` carried *"`overflow-checks = true` costs 5 % on the core, measured at M1"*.
Re-run on 2026-09-01 against all five milestone trees, the flat bus costs **36–42 %** more with
the checks on — **including on M1 itself, which measures +40.6 %.**

**That is not decay, and the distinction from the `panic_bounds_check` row is the whole content
of this entry.** That row was true when written and **falsified at a nameable commit**; a bisect
found M2, and "it broke at M2" is now a fact about M2. This row **does not reproduce on the tree
it was written on**, so there is no commit to name and nothing to bisect. The workload cannot
absorb the difference either: `benches/step.rs` is byte-identical in all five milestone trees and
in the working tree, so the thing being measured really was held constant.

So the honest verdict is *cannot be reproduced*, not *falsified at milestone N* — and the reason
nobody could tell those two apart for four milestones is that **the method was never written
down.** Five per cent of what, measured how, on which bus, on which host, under what load? Every
one of those changes the answer by more than five per cent, and the row recorded none of them. A
number without its method is not a weak measurement. It is not a measurement at all: it is a
claim wearing a measurement's clothes, and it cannot be argued with, only replaced.

**The remedy adopted is the cheapest thing that could work: every row in that section now carries
the command that produced it, the date it was last run, and what enforces it.** That is not
bookkeeping. Re-running has to be cheaper than re-deriving or it does not happen — which is
exactly how an explicit instruction to *"re-run after M2"* went unexecuted through three
milestones while looking green.

The rule the number was attached to — `overflow-checks = true` in release, so debug and release
agree and every wrap is an explicit `wrapping_*` — is a **correctness** decision and is untouched
by any of this. Only its price was wrong. Note the second column of that re-measurement, because
it is the one that matters for M7: on the paged bus, the shape the real machine has, the checks
cost **3–8 %**.

### A count is a property of the probe, and the probe is part of the claim

Four rows of the *Measured* table turned out to be neither true nor false but **under-specified**,
and they only look like different bugs. Indirect branches are **0** in a whole-program build and
**1** in the `Cpu<Ula>` library object. `Ula::tick` is inlined by the *link*, not by its
`#[inline]`, so "no out-of-line bus method" holds for one subject and fails for the other.
*"`Bus::read` compiles to one instruction"* names no bus at all, and the two candidates differ by
a page of paging logic — that row is **refused rather than re-pinned**, because there is nothing
to pin.

And the headline case: **three separate probes have produced 15, 11, 10 and 7 for "the bounds
checks".** Two independent bisects, using two different probes, agree exactly on **when** the row
broke — M2 — and disagree on **the integer**. *The falsification date reproduces; the number does
not.*

Settling why M2 broke it produced two more facts about that integer, and both make the subject
line load-bearing rather than decorative:

- **It counts instructions, not checks.** The gate counts `bl …panic_bounds_check` call sites, and
  LLVM tail-merges cold blocks: in one variant, **five branches reach three call sites**. The
  number is exact and reproducible and it is not the number of bounds checks — and it never
  claimed to be, which is only a defence if the row says so.
- **It is a property of the inliner.** Holding the probe, the toolchain and the source semantics
  fixed and varying nothing but inlining decisions, the same M2 core measures **0, 3, 5, 7 and
  10**.

**So a bare integer in a table is not a measurement; it is a measurement with its subject
deleted.** The gate that now pins the deterministic rows therefore carries its own probe *in its
own source* and rebuilds it, so the subject cannot drift away from the number the way prose does.

> **The same defect has a second form, and it is the more dangerous one.** *Why* M2 broke the row
> was on record as an inference: *"the prefix decoder now produces a runtime operand field plus a
> `DD`/`FD` base where M1 had constants."* Measured, it is half right and half describing a change
> that never happened. `Operand::source`/`destination` indexed their table with a runtime opcode
> **at M1 too**, so the operand field was never a constant. And the `DD`/`FD` base — which the
> surviving checks genuinely do index with — produces **none of them on its own**: grafting M2's
> entire prefix-consuming `dispatch` onto the M1 core leaves the count at **0**. The mechanism is
> an inliner decision, and the compiler states it in as many words. Details, trees and the part
> that is still unsettled are in [`ARCHITECTURE.md`](ARCHITECTURE.md) and **only** there.
>
> The class: **an inference that identifies the right operand and calls it the cause reads exactly
> like an explanation.** It survives casual checking, because everything it names is really
> there. This document already records the same shape twice for the two `zexall` guesses that
> *predicted the right verdict for the wrong reason*; the new instance is that it happened inside
> a row that was **explicitly labelled inferred, not measured** — and the label did not stop the
> inference being cited as the reason for four milestones. **Labelling an inference is not a
> substitute for taking it down.**

### A gate can pass vacuously, and only a positive control catches it

The codegen gate's first run was **green on four assertions that measured nothing.** The probe
was built with `--emit=asm` but without `link`, so fat LTO never ran, the core stayed in its own
rlib, and every "this count is zero" assertion read a triumphant zero off an artifact that did
not contain the code under test.

Nothing in those assertions could have caught it. Each was correctly written, each was checking
the right property, and each was checking it in a file where the subject was absent. **A count of
zero and an absence of the subject are the same observation.**

What caught it was a **positive control**: one assertion whose only job is to fail when the probe
stops exercising the core. `the_probe_actually_exercises_the_core` requires the artifact to carry
at least 1,000 source locations naming `crates/z80/src`. It earned its keep on its first run. The
floor is deliberately loose — measured **1427 at M5** and **618 at M1** — because it only has to
separate *present* from *absent*, and a probe that stops driving the core reads **0**. The gap it
must span is three orders of magnitude, not a few per cent, which is why a loose floor is the
right shape here and a tight one would just be a second thing to re-pin.

**This is a distinct mechanism from the harness recorded above as reporting green while verifying
nothing, and that is why it gets its own entry rather than a cross-reference.** There, the
*inputs* were missing and five tests silently skipped. Here every test ran and every test
measured — the wrong artifact. The family is the same and the remedy is not interchangeable: a
corpus guard cannot detect an empty artifact, and an assertion about the subject's presence cannot
detect a missing corpus. Stated generally, and it is cheap: **every gate needs one assertion whose
failure means "I was not looking at the thing".**

### Deleting an assertion because it cannot fail is right, and it looks wrong on a diff

`ARCHITECTURE.md` claims the execute path allocates nothing, and the obvious gate is to count
allocator call sites attributable to `crates/z80/src` in the probe's assembly. It was written.
Then it was mutated to prove it bites — `Cpu::step` made to build a `Vec` on every call — and it
**still measured zero**, because the `bl` to the allocator lives inside `alloc`'s own code and
carries `alloc`'s source location, not the caller's. **The assertion could not have failed for
any mutation of the core.**

It was deleted, and what replaced it is strictly stronger. `crates/z80` is
`#![cfg_attr(not(test), no_std)]` with no `extern crate alloc`, so in every non-test build
**allocation does not compile**; the gate asserts those two structural facts. That is a property
of all builds rather than an observation about one — and it is exactly what the mutation had to
destroy before it could allocate at all, which is the evidence that the replacement is sensitive
where the count was not.

**This is recorded as a method rather than a lesson, because on a diff it reads as coverage going
down**: an assertion removed, the test count falling by one, a reviewer's instinct to object. The
rule that makes it right is one this document already lives by from the other direction — *a test
that does not go red on the original defect is decoration.* **A test's value is its failing case,
so a test with no reachable failing case has none, and keeping it costs more than deleting it**,
because a green that cannot go red is indistinguishable from a green that could.

The one thing that must not happen is deleting it *quietly*. The account of what was tried, why it
was insensitive, and what replaced it lives in the gate's own doc comment, next to the assertion
that took its place — so the next reader finds out that the count was attempted and does not
propose it again as an improvement.

### A derived figure repeated across documents acquires authority it never earned

*"Three of the five mutations were already red, and two survived."* It was **four**, and **one** of
the five survived. The correction is in the M5 section; this entry is about how the wrong number
lived, because that part is not about the machine.

**The evidence was never missing and was never in dispute.** The per-mutation failing-test
counts — 5, 7, 1 and 13 — were correct throughout, and they sat in a table directly beneath the
sentence that miscounted them. Adding four numbers is not an investigation. **Nobody added them,**
because by then the sentence had been quoted into three documents, and three files saying the same
thing reads exactly like corroboration. It was not corroboration: it was one derivation copied
three times, and copying a conclusion is the one operation that cannot detect an error in it.

Two things sharpen this past "check your arithmetic".

**First, it is the failure mode of a rule this project is otherwise right about.** *One register,
one owner* — a fact lives in one place and everything else links to it — is why `ARCHITECTURE.md`
refuses to duplicate the open register and why measurements name a single home. That rule governs
**state**. A *derived summary* is different in kind: copying it duplicates a conclusion while
leaving its inputs behind, so the copy can no longer be checked where it sits. Deferring to a
source stays safe; carrying a number away from its derivation does not.

**Second, one of the three documents knew.** `MACHINE.md` said, in as many words, *"which three
were already red is not recorded here and is not derived here"* — and printed the one-line command
that would settle it. The hedge was accurate, honest, and completely ineffective: the figure was
quoted for three documents anyway, hedge and all, because a label travels with a claim and a
correction does not happen unless somebody runs the command. **Naming what would settle a claim is
not a substitute for settling it, and when the cost is one command the hedge is the more expensive
option.** This document already records the identical failure for a *measurement* — an unenforced
instruction to re-run after M2, which named the exact milestone and was never executed. It is the
same defect applied to arithmetic.

So: **re-derive rather than cite, whenever the derivation is cheaper than the citation is
durable.** And treat agreement across documents as evidence of copying until the derivations are
shown to have been independent.

### A mutation that survives is a question, not a result

A mutation that made only every *other* T-state of a contended internal run contend reddened
**nothing**. Both available reflexes are wrong here. Dismissing it ("equivalent mutant, moving on")
throws away a finding; reaching straight for a new test writes a gate without knowing what it is
gating.

What was done instead was to **derive why it survived**, and the answer is a fact about the
hardware rather than a gap in the suite. A contended one-T-state internal cycle at group position
`k ≤ 6` stalls `6 − k` T-states and therefore lands on position **7**, where the stall is zero — so
a contended internal run **strictly alternates charged, free, charged, free**, and the mutation
skips exactly the free ones. It is observationally identical to the original across all 120 start
columns of a display line. There was nothing to catch.

**Deriving it is also what located the one shape where the mutation *can* bite**: an internal run
that begins on position 7 — which requires arriving from an uncontended cycle onto a contended
address, which happens only when the internal cycles ride the refresh address `IR` rather than the
address of the last transfer. `an_internal_run_on_a_contended_refresh_address_is_charged_at_every_t_state`
is that case, and writing it paid for something nobody had asked for: **nothing in the suite had
ever established that internals on a contended `IR` are charged at all.** M7 makes that case
routine, because a 128 contends banks in whichever slot they are paged into.

**The class: a surviving mutation is either a hole in the suite or a fact about the system, and
the two are indistinguishable from the outside — both are a green run.** Telling them apart
requires deriving the reason, and the derivation is the deliverable: it either yields a gate that
closes a real gap, or it yields a property of the machine worth writing down. *"It survived"* is
the beginning of the investigation and never its result — which is the same rule this project
already applies at the other end of a mutation run, where *"it landed"* must be proven before a
verdict is trusted.

> **A second instance, and it landed on the same side.** Two mutations of `Clock::advance` — the
> rollover discarding its overshoot (`frame_t_state = 0` for `-= T_STATES_PER_FRAME`), and `if`
> for `while` — reddened `timing.rs`'s own unit tests and **nothing** in
> `crates/spectrum/tests/`, including the file written specifically to drive instructions across
> the boundary. Derived rather than dismissed, the reason is a fact about the machine: **every
> multi-T-state `advance` on a 48K is a contention stall, contention exists only between 14335 and
> 57342, and the largest stall is six — so `advance` can never cross a frame boundary by more than
> one T-state through the machine.** At a step of one, `= 0` and `-= 69888` are the same
> assignment and `if` and `while` are the same loop. There is nothing to catch, and a gate written
> to catch it would have to call `Clock::advance` directly, which is what the unit tests already
> do.
>
> It is the same reason that shapes `frame_boundary.rs` end to end: **around a frame boundary a
> 48K is always in the border**, so the wrap can never fall inside the fetch window and no
> instruction can straddle it with contention live on either side. One derivation, two
> consequences — which is the tell that it is a property rather than a coincidence.

### A window graded against its own constant

`INTERRUPT_T_STATES` moved from 32 to 24 reddened **one test in the entire workspace**, and it was
a newly written one. The window's own gate —
`frame_interrupt.rs`'s `the_line_is_held_across_the_whole_window_and_drops_at_its_end` — stayed
green, and so did `timing_oracle.rs`.

It stayed green because it derives **both** the positions it samples and the value it expects from
the constant under test: it steps `while frame_t_state() < INTERRUPT_T_STATES`, asserts `/INT` at
each, then asserts the machine lands exactly on `INTERRUPT_T_STATES`. Every clause moves with the
constant. It is the keyboard-matrix tautology again — *a test whose expectation is computed by the
subject is not a weak test; it is a tautology with a cross product attached* — this time with a
loop rather than a cross product.

**And the near-miss is the sharp part.** The same mutation in the *other* direction, 32 → 33, does
redden it. Not because the test can see the window move, but because 33 is not a multiple of four,
so its `NOP`s can no longer land exactly on it and the final `assert_eq!` fails on arithmetic. A
one-sided pin, held by an accident of divisibility, reading in a run log exactly like a two-sided
one.

What replaced it is two literals: an instruction that overshoots the top of a frame by **31**
T-states must have its interrupt accepted at once, and one that overshoots by **32** must miss that
frame's interrupt entirely and wait for the next. Neither mentions `INTERRUPT_T_STATES`. That is
the difference in kind this document already records for the keyboard: **a literal table has a
failing case for the property it appears to test, and a consistency check has none.**

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

> **The last clause stopped being true at M2 and the fix is in the very file the table is about.**
> `crates/z80/tests/alu_flags.rs:34` carries
> `const ENTRY_FLAG_GRID: [u8; 4] = [0x00, flags::C, !flags::C, 0xFF];`, and its own doc names this
> exact case — *"a `CP` ORing entry bit 5 for exactly `a=0x3C, operand=0x17` survived that
> revision … Four values fix it"*. `git log -1 -- crates/z80/tests/alu_flags.rs` is `8021b4f`,
> which is the M2 merge, so the entry-flag axis was restored **three milestones** before this
> sentence was last read past.
>
> **The table above is honest as history and the concluding sentence is not, and the difference is
> the whole lesson.** *"The deleted boundary test caught it and the sweep did not"* is a verdict
> about a run that happened; *"it is invisible to every gate we have"* is a claim about the
> present tense, and the present tense is what rots. A finding written in the past tense stays
> true for ever; the same finding generalised into a standing claim acquires an expiry date that
> nothing enforces. Prefer the past tense unless the standing claim is worth a gate of its own.

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

> **This paragraph is the only place in this document that counts anything in this family, and six
> other files have been citing it — several of them wrongly. Written down here so the next one has
> a referent.** Added 2026-09-01, after a figure that came from nowhere was found in two source
> files and traced.
>
> **What is counted: three, and they are these.**
>
> | # | The gate | The form |
> |---|---|---|
> | 1 | The M3 `zexdoc` job | an `#[ignore]`d `#[test]` — invisible by default, but a test, reachable with `--ignored`, listed as skipped |
> | 2 | `.github/workflows/ci.yml` | written, reviewed, and parked on a branch that cannot be pushed. Still open, in the register above |
> | 3 | The M5 boot gate | `crates/spectrum/examples/boot.rs`. **An example is not a test in any form**: no flag reaches it, no listing shows it, its absence from a run leaves no trace. Closed — `tests/boot.rs` landed |
>
> **What is *not* counted, and this is where every consumer has gone wrong.** These three are
> gates that **nothing ran**. They are not *"occasions where this project shipped evidence that
> graded less than it appeared to"*, which is the phrase five documents wrap around the number.
> That broader family is recorded in this document repeatedly and **counted nowhere** — a harness
> reporting green while verifying nothing (*The harness was reviewed, and it could report green
> while verifying nothing*), a codegen gate passing vacuously on an artefact that did not contain
> the subject (*A gate can pass vacuously, and only a positive control catches it*), the keyboard
> matrix graded against itself (*The keyboard matrix was graded against itself*), an interrupt
> window graded against its own constant (*A window graded against its own constant*), an
> exhaustive sweep narrower than the sample it replaced (*Exhaustive on one axis can be weaker
> than a sample on another*). Five instances are visible from that list alone, which is presumably
> where a *"five"* came from — but nobody derived it, the five are not a closed set, and this
> document has never claimed one.
>
> **Those five are named by section and not by line, and the reason is the subject of this whole
> note.** They were written as `:1200`, `:1366`, `:413`, `:1504` and `:1530`, and **not one of the
> five still resolved** when they were followed — four of them landed more than a hundred lines
> from the section they claimed, on prose about an unrelated subject. A bare line number carries
> no redundancy: nothing inside it can be compared against what it points at, so it cannot fail
> loudly, only drift silently. A section title can be grepped, and a wrong one announces itself.
> **No replacement numbers are given here on purpose** — this document is edited in the middle far
> more often than at the end, so any five line numbers written into this paragraph would be wrong
> again by the next pass, including the pass that wrote them. The titles above are the citation.
>
> **The phrase everybody quotes is not this document's.** *"The worst form so far"* is
> [`MACHINE.md`](MACHINE.md)`:132`'s. On 2026-09-01, **before this note existed**,
> `grep -n -i worst docs/STATUS.md` matched **nothing**; it now matches only this note and the
> ones like it, which is stated that way because otherwise the sentence falsifies itself the
> moment it is written down. Four files attribute the phrase here. The way to re-check it is
> `grep -n -i worst docs/STATUS.md` and to read what it returns rather than counting it — every
> live hit should be a correction *about* the phrase, and none should be this document using it.
>
> **So the rule for anyone citing this:** the count is **three**, it counts *gates that nothing
> ran*, and if the sentence you are writing says *"graded less than it appeared to"* then it is
> about the broader family and **must not carry an integer**, because there is not one to carry.
> `crates/frontend/src/lib.rs` and `src/bin/zx-shot.rs` said *"five"*; `M6.md` (twice), `M7.md`,
> `crates/spectrum/src/tape/mod.rs`, `tape_corpus.rs` and `snapshot_corpus.rs` said *"three"* with
> the wrong noun or the wrong attribution or both.
>
> **Two sentences elsewhere carry it correctly, and they are the ones to copy.** `ARCHITECTURE.md`
> says *"a gate that runs nowhere, which `STATUS.md` records three times already"* — in its M6 row,
> beside the T4 tier it declines to gate on — and `README.md` says the same thing in its own words,
> *"a milestone gated on it would be a gate that runs nowhere — which `docs/STATUS.md` records this
> project shipping three times already"*. Right count, right family, right noun, in both.
>
> **The exemplars were right and the citation of them was not, which is the sharper half.** This
> note used to send readers to ~~`docs/ARCHITECTURE.md:362`~~ and ~~`README.md:101`~~ for those
> sentences, and **neither coordinate ever held one**: at the commit that introduced the pair,
> `:362` was a sentence about a design's two halves and `:101` was a **blank line** — a dated claim
> about a commit, which is why drift cannot touch it. The phrase it put in their mouths,
> *"three gates that ran nowhere"*, is not theirs either — `rg 'gates that ran
> nowhere'` returns `docs/M6.md` and `crates/spectrum/tests/snapshot_corpus.rs`, which are two of
> the files listed a paragraph above as needing correction. **Those two coordinates carried line
> numbers until now, and one of them had already drifted** — the `M6.md` hit moved from `:649` to
> `:672` — inside the paragraph whose subject is that exact failure. The command is printed beside
> them, so the digits bought nothing and rotted anyway; running it is the citation. So this note acquitted two
> sentences it had not read, in words borrowed from the files it was convicting, at addresses that
> pointed at nothing. **An acquittal is the citation nobody re-checks**, which is why it survived
> longer than any of the accusations around it: a wrong finding invites an argument, and a wrong
> exoneration closes one.
>
> **Both coordinates were re-derived, and what they hold is not the point — which is the correction
> this paragraph needed itself.** It used to rest its case on ~~`README.md:101`~~ being *"still a
> blank line"*, and that sentence was true when written and false by the time it was next read: the
> line gained content under an unrelated edit and now carries prose about the ROM's `LD-BYTES`
> toggling the border. ~~`docs/ARCHITECTURE.md:362`~~ is a `regs: [u8; 26]` field declaration, a
> third thing again. **The argument never needed either observation.** What convicts the citation is
> that neither coordinate ever named the sentence it claimed to name, and nothing a later occupant
> does can change that: the two exemplars are found today by their words — `ARCHITECTURE.md`'s
> *"a gate that runs nowhere, which `STATUS.md` records three times already"* and `README.md`'s
> *"records this project shipping three times already"* — and each sits hundreds of lines from the
> address it was given. **That is why the two exemplars above are quoted and not addressed.** A
> citation carrying a quoted fragment or a function name is re-found by `rg` after any amount of
> drift; one carrying only a coordinate can only be believed, and repointing it at whatever occupies
> the line today is how a wrong exoneration gets re-issued.
>
> **A specimen rots in the opposite direction from a citation, and nothing here had recorded that
> shape.** A coordinate quoted *as an example of a broken pointer* carries a claim of its own —
> *nothing lives here* — and it is falsified not by its target moving away but by something moving
> **in**. The citation gate is structurally blind to it, because the gate's entire question is
> whether a cited line has content: a specimen going wrong turns the gate **red → green**, by the
> same non-event that turns a good citation bad, running backwards. Catching it would need a second
> mark meaning *"I assert this coordinate is empty"*, and the gate's author declined to invent one
> on a single observation, on the ground that **a mark nobody has the habit of writing is a mark
> nobody writes**. That restraint is right, and it is why this paragraph is repaired in prose rather
> than by a mechanism: an argument that does not depend on its specimen cannot be falsified by one.
> What the gate does honour is **strikethrough** — a struck `` `path:NNN` `` — as the specimen mark,
> and that mark is self-grading by construction: the mark *is* the text, so it travels when the file
> grows, and the edit that stops something being a specimen removes the mark in the same keystroke
> and puts the citation back under the gate. A registry kept beside the prose could never have that
> property. The dead coordinates above are struck for that reason — it tells a reader at a glance
> what they are, and stops the gate accusing them. The general form is at the end of this document,
> under *A hazard named as future work is optional*.

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

> **Closed.** The entry above said *"a real test is in flight in `crates/spectrum/tests/`. This
> entry stands until it lands, and the next docs pass closes it — the register does not get to
> record a fix before the fix exists."* This is that pass, and it landed:
> `crates/spectrum/tests/boot.rs` runs the ROM under `cargo test` and asserts the message **and the
> frame it appears on**. The example remains, as an example.
>
> **The finding survives its own fix, and is the more useful half.** Rerunning the five mutation
> verdicts against the real gate is what showed that **four** of them had been red all along, in
> unit tests inside `src` — so the coverage table in `2157331` was not merely optimistic, it was
> describing a run that never happened. **A coverage claim names a run, and naming the wrong run
> makes every row wrong at once**, in both directions: four rows understated the suite and one
> overstated it. *(This passage said "three" and "two" until the verdicts were re-derived rather
> than re-quoted; see the M5 section.)*

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

### A missing stall cannot be added to a total, because every stall shifts the ones after it

Two agents independently measured `INC (HL)`'s contended cost at phase 0 and disagreed: **26 and
30.** Both had the same observation to work from — the retired heuristic produced **25** — and both
knew it was short by one contention point.

The 30 came from adding the missing point back: the lost stall was worth 5 T-states at that
position, so 25 + 5 = 30. The arithmetic is right and the answer is wrong. **Dropping that stall did
not merely subtract 5 — it opened the following write four T-states early**, at +18 instead of +23,
where the delay pattern charges 4 rather than 0. Four of the five came straight back. The *quantity*
lost was one contention point; the *error* was one T-state.

It was settled by refusing to reason about the delta at all: a recording bus attached to a real
`Cpu`, the instruction decomposed into its four machine cycles — **`pc:4, hl:3, hl:1, hl:3`** — and
the answer re-derived by a second implementation written only from the published delay rule, with no
sight of the first. **26 at phase 0, 19 at phase 7.** The per-cycle table is in
[`MACHINE.md`](MACHINE.md) and only there.

**The class: in a system where a cost shifts what follows it, a delta is not additive, and reasoning
about one in isolation gives a confidently wrong number.** Not an uncertain number — a specific,
plausible, defensible one, which is what makes it expensive. The tell is that the two derivations
differed by *almost exactly* the missing quantity; that near-match reads as confirmation and is
actually the signature of the mistake.

Two further things generalise. **The wrong number was made plausible by this project's own
documentation** — the residual was recorded as *"one contention point (0–6 T-states)"*, which
conflates the isolated stall with the observable error and invites precisely the addition that
produced 30. It is corrected above. And **the fix was to go back to the mechanism**: the machine
cycles are observable, they were recorded rather than read off the source, and the second derivation
was independent by construction. Where a quantity can be recomputed from first principles, that
outranks adjusting an old one.

### A restore that discards real work is the same defect as a mutation that never lands, and quieter

A mutation driver restored the file it had mutated with `git checkout -- <file>` — and reverted a
real, uncommitted edit an agent had made to that same file minutes earlier. It was caught, and the
driver was rewritten to back up the working copy before mutating and restore from that backup
afterwards.

**The class is already half-written in this document.** The standing rule is that *a failed edit and
an unbreakable guard produce the same exit code*, so every mutation's landing is verified before its
verdict is trusted. The restore is the same hazard at the other end of the run: **both produce a run
whose result describes a tree nobody intended**, and the restore is worse, because a failed mutation
merely wastes the run while a destructive restore silently deletes work that was never part of the
experiment.

The specific trap is that `git checkout --` means "discard my changes to this path" and the driver
means "undo *my* change to this path". Those coincide only when the driver's change is the sole
uncommitted change to that file, which is an assumption about everything else happening in the tree
— and it is false whenever anyone else is working in it. **A restore must be scoped to what the
process itself did**, never to what the index happens to hold.

### A harness reported eight survivals while every one of them exited 101

The most dangerous shape a test tool can take is not a wrong verdict. It is **"the suite went red
and I could not read how" printing as "the guard held"** — and the tool that did it here was the
mutation driver, built to enforce exactly that rule against everything else.

Eight mutations, all reported **SURVIVED**. All eight had in fact reddened multiple targets, and
all eight had exited **101**. The driver used `subprocess.run(capture_output=True)`, which routes
stdout and stderr into two separate pipes; `cargo` prints `Running <target>` on **stderr** and
`test result: FAILED` on **stdout**. Concatenating the two afterwards destroyed the interleaving
the parser walked, so it matched nothing, found zero failures, and called that a survival.

**The fix that matters is not the parser.** Merging the streams into one pipe is a one-line change
and it is the smaller half. The load-bearing change is the assertion the driver now makes about
itself: **a non-zero exit with nothing parsed raises, and never reports a survival.** So does a
zero exit with parsed failures. A run the tool could not read is a *failed run*, not a green one,
and the distinction has to be encoded rather than remembered — because the two are identical from
the outside, which is the whole reason this document keeps writing the same sentence about exit
codes.

This is the project's own standing warning about tooling manufacturing false evidence, committed
by the tool built to enforce it. That is not irony worth enjoying; it is the reason the rule needs
a mechanism. Three instances are now recorded within these pages — a corpus-absent suite exiting 0
with 87 passing tests, `cargo test` short-circuiting before the integration gates, and a mutation
driver reading its own output wrong — and in every one the tool answered a **narrower question than
the caller asked**, in a form indistinguishable from the wider one.

### `cargo test` without `--no-fail-fast` answers a different question than the one asked

`cargo test` stops after the first target that fails. The integration gates in
`crates/spectrum/tests/` are separate targets from the lib's unit tests, so a mutation that reddens a
unit test **prevents every one of the integration gates from running at all** — and the output of "the gates did not run"
is indistinguishable from "the gates passed". One mutation run was invalidated by this before it was
caught, and it is why the mutation table above reports failure *counts* across the workspace rather
than a bare red/green.

**This belongs with the two failures already catalogued here**, and it is the same family as both:
*"with `testdata/fuse` absent, `cargo test -p z80` exited 0 with 87 passing tests"*, and *"a
truncated run prints nothing but `OK` lines"*. In each case a harness reported green while verifying
nothing, and in each case the green was **correct as an answer to a different question** — did any
target fail before we stopped, rather than did every gate run and pass.

The class generalises past `cargo`. Any tool that short-circuits, samples, or filters silently is
answering a narrower question than the caller asked, and its answer looks exactly like the wider
one. The shell has the same hazard in a smaller package: `cmd | tail; echo $?` reports *`tail`'s*
exit status, not `cmd`'s, so a gate written that way reads green whatever `cmd` did. That one is
recorded here for the first time; the remedy is identical to the one this project already uses for
its corpus guards. **Make the tool state what it covered, and assert on that, rather than on its
verdict.** A count of executed targets, or of executed vectors, is checkable; an exit code is not.

### The interrupt acknowledge is priced per T-state, and it was written down rather than fixed

`crates/spectrum/src/ula.rs` charges every machine cycle's contention **once**, at the moment the
cycle opens, because every cycle reaches the bus as a transfer callback followed by that cycle's
own ticks. **The interrupt acknowledge has no transfer callback** — it reads no memory, asserting
`/IORQ` in place of `/MREQ` — so `crates/z80` delivers it as seven bare `Bus::tick` calls at the
refresh address, and `Ula::tick` treats each as a standalone internal cycle contending on its own
account. The hardware performs **one** machine cycle there: M1 stretched by two wait states. On a
contended `IR` the model would charge seven stalls where it owes one.

**It is not fixed, and the reason it is not fixed is the finding.** On a 48K it cannot be observed
at all: the ULA holds `/INT` low for the frame's first 32 T-states and contention does not begin
until 14335, so an accepted interrupt and its acknowledge always land in the top border, where
every stall is zero whatever the address. **No test can distinguish the two models**, so a fix
would be an unverifiable guess and a gate for it would assert a number nothing produces. That is
the same class this document records for the floating bus: *returning a constant is wrong in a way
that is visible; a plausible guess would be wrong in a way that is not.*

The general rule, which is the part worth carrying: **when a defect is real, understood, and
currently unobservable, the correct action is to write it down where the register lives — not to
fix it, and not to leave it in a head.** A guess shipped into a model becomes indistinguishable
from a measurement the moment the next person reads the code; a recorded item stays an item. This
project's register exists precisely so that a known-and-unfixable thing cannot quietly become a
forgotten one.

**M7 is when it stops being unobservable**, and that is why it is recorded now rather than when it
bites: the 128's frame geometry differs — 70908 T-states, a different interrupt position — and it
contends banks in whichever slot they are paged into, so an acknowledge on a contended `IR` becomes
reachable. Whoever does that work should arrive at this already knowing, rather than rediscovering
a seven-fold overcharge as an unexplained timing error.

### Is this oracle circular? — the shape of the answer, which will be needed again

`tests/timing_oracle.rs` is the first thing in `crates/spectrum` whose expectations this project did
not write, and the obvious objection is the right one to raise: **the suite's author writes his own
emulator, and his page says it passes all the tests.** Read alone, that is exactly the shape that
would make the corpus worthless — a table fitted to an emulator, graded against an emulator.

Three things answer it, and **none of them requires believing the author**:

1. The expectations are attributed to runs on real Spectrums, in as many words.
2. The results database is hardware-only **by policy** — *"Only submit results from genuine
   hardware no emulators!"*
3. **Decisively: the file carries two tables, and twenty-five of twenty-eight submitted machines,
   from nine independent people, sort cleanly into exactly those two classes.** A table fitted to
   one emulator has no reason to predict a *second* class of machine that emulator does not
   implement, and no reason for twenty-five real boards to fall into the two rather than scatter.

**It is the third that carries the weight, and the reason generalises past this corpus.** The first
two are assertions by the party whose independence is in question — worth having, worth nothing on
their own. The third is a **structural** property of the artefact: it predicts something the
circular hypothesis gives no account of. That is the question to ask of the next external oracle
this project reaches for — at M7 for the 128, and for anything that grades the AY: *not "did they
say it came from hardware", but "what does this artefact predict that a fitted table would have no
reason to predict?"*

And the answer's limits are as reusable as the answer. The corpus **cannot** say whether 14335 or
14336 is right, because real Spectrums are both; what it can do is **refuse a machine that is
neither** — and that turned out to be the sharper instrument, because it is the one every emulator
bug actually trips.

### A correction is not landed until you have grepped for every other copy

This document already records that *a derived figure repeated across documents acquires authority
it never earned*, and prescribes **re-derive rather than cite**. That rule is right and it is
incomplete: it tells you what to do when you *find* a figure, and says nothing about what to do
when you *fix* one.

Both defect classes the previous documentation pass set out to eliminate **recurred inside the
commit that names them**:

- The mutation-count correction visited `STATUS.md`, `ARCHITECTURE.md` and `MACHINE.md` — and left
  `README.md`, the repository's front door and the only document a newcomer reads first, carrying
  the pre-correction figure. The section diagnosing the propagation missed a copy of the thing
  propagating, and the copy it missed was the most-read one.
- *"Sixteen tests"* was stale in `STATUS.md` **and** `testdata/README.md` simultaneously, and the
  pass corrected neither.

> **Checked rather than repeated: the first bullet is history, not the present.** `README.md`
> carries the corrected figure today — *"four were already red and one survived"* — under a
> correction block that reaches this same rule independently and reports its own sweep as **not
> coming back clean**, naming `docs/MACHINE.md:358` as the remaining copy. Two agents arriving at
> the same rule from opposite ends of the same defect is the strongest evidence for it there is;
> a third repeating the *finding* without re-checking whether it still held would have been the
> defect itself, one level up.

**The enforcement half, which costs seconds:** when a figure is corrected, `grep` for it across
`docs/`, `README.md`, `CHANGELOG.md` and `testdata/` **before publishing**, and fix or route every
instance. Not "check the related documents" — grep, because the instinct for which documents are
related is exactly what failed both times.

**And grep the claim's *noun*, not the figure — the rule above says *grep for it*, and *it* was the
wrong operand.** Measured on the `.z80` frame-position gate, whose count has now been corrected four
times with every round reporting a clean sweep: each round searched for the number it was replacing,
and a search for a figure returns only the copies that already agree with it. The survivors said
*three* where the new figure was *six*, or *six* with no per-machine qualifier — invisible to a
search for either number. Searching the half of the sentence a renumbering leaves alone —
*"derived by hand"*, *"exhaustive sweep"*, *"by hand from the format"* — returned every copy in one
pass. This is the anchoring rule below (*a hazard named as future work is optional*) applied to a
search rather than to a citation: the noun is the shape, the number is the thing that moves.

The deeper point is that *re-derive rather than cite* is a rule about **the document set**, not
about one file. A correction applied file-by-file leaves the corpus internally inconsistent, and an
inconsistent corpus is worse than a uniformly wrong one: a reader who checks two sources and finds
them agreeing stops looking, and a reader who finds them disagreeing has no way to tell which is
current. This pass found four such disagreements — a CI remedy contradicting its own register two
hundred lines away, a present-tense claim about gate coverage falsified three milestones earlier, a
count stale in two files, and a throughput figure with no method in the commit whose thesis is that
defect — and every one of them was cheaper to grep for than to argue about.

> **The register-audit pass ran that sweep and it did not come back clean, so the residue is
> written down rather than described as handled.** Correcting four test doc-comments about the
> timing oracle's scope meant grepping `docs/`, `README.md`, `CHANGELOG.md`, `testdata/` and the
> `//!` blocks for every other copy of *"no oracle"* near `FIRST_CONTENDED_T_STATE`. Most of the
> corpus is current — `crates/spectrum/src/timing.rs:54` opens with *"**The oracle exists**"*,
> and `MACHINE.md`, `ARCHITECTURE.md`, `M6.md`, `README.md` and `testdata/README.md` all carry the
> interval-scoped statement. **Four live copies remain, all in files that pass did not own, and
> they are listed here so the next reader finds them rather than rediscovering them:**
>
> **Update, same day — three of the four are now fixed, and the list is why.** Two were corrected
> by the agent that owns `contention_phase.rs`, working from this table; the third had already
> been fixed by its own owner. Only the `M7.md` row survives. That is the argument for listing a
> defect you cannot fix instead of only reporting it: a named `file:line` gets closed by whoever
> next owns the file, and an unnamed one is rediscovered.
>
> | File | What it still says |
> |---|---|
> | ~~`crates/spectrum/tests/contention_phase.rs:5-11`~~ | ~~*"The absolute phase remains unverified against any external oracle. Nothing in this file, and nothing anywhere in this project, measures `FIRST_CONTENDED_T_STATE` against a real machine… `docs/MACHINE.md` names such a program as verification item 2 **and it is not written**"* — and `timing_oracle.rs:3` says it **is** item 2. This is the strongest surviving copy, and it is the header of the phase gate itself~~ **FIXED.** The header now opens with the superseded text quoted, both halves corrected, and the oracle's scope stated narrowly: what is established is the **interval** — the first contended T-state falls exactly 14335 after `/INT` — given the frame origin this machine asserts, which stays a convention |
> | ~~`crates/spectrum/tests/contention_phase.rs:9-11`~~ | ~~*"An issue 2 Spectrum is one T-state earlier than an issue 3"* — the **wrong-axis** sentence this document already corrected in two places and `timing.rs:52-58` corrected in a third. This is its last live copy~~ **FIXED, and it was indeed the last.** `rg 'T-state earlier'` now returns only quoted-and-corrected copies. The replacement does not merely say "wrong axis": it names the **mechanism** the oracle since established — an edge race in the interrupt, not a property of the board |
> | ~~`crates/spectrum/src/lib.rs:109`~~ | ~~the crate coverage table: *"`tests/contention_phase.rs` pins it to the frame's structure. **No oracle.**"*  — and the table has no row for `tests/timing_oracle.rs` at all~~ **FIXED by that file's owner**, independently and before this pass reached it: `:137` now carries the `timing_oracle.rs` row and `:139` the interrupt-window band. Recorded because it is the good outcome of listing a defect rather than only fixing it |
> | `docs/M7.md:1466-1467` | *"`STATUS.md` has not been written to since M5 — its **first open row still says `FIRST_CONTENDED_T_STATE` has no oracle**"* — a claim **about this file** that was true when written and is not now |
>
> The last row is the sharpest, and it is a new shape: **a document making a stale claim about
> another document's staleness.** `M7.md` is right that a register nobody re-reads goes wrong, and
> is itself the example. It also means the two files corroborate each other's *previous* states to
> anyone checking both — the failure this document names two sections above — so the note above is
> written **from** `M7.md:1466` as it reads today, not from memory of what it said.
>
> **A fifth copy was found and had been fixed by the time it was checked, which is worth recording
> as a method note rather than dropped.** The sweep reported `crates/spectrum/src/timing.rs:85` as
> reading *"See the module documentation: unverified"*. Re-read from the file forty minutes later,
> `timing.rs` contains **no occurrence of "unverified" at all**; the constant sits under
> *"Hardware-graded as an interval from `/INT`"*, and the file's mtime had moved. Another
> agent corrected it mid-session. The copy was real, the report was accurate when taken, and
> publishing it unchecked would have put a false claim in this table — **a sweep result is a
> document too, and this document's own rule is to verify against the code and not against a
> document.** The rule earned its keep on the one row where it was tested.

> **This rule has a sibling, it lives in `README.md`, and this paragraph is a pointer rather than
> a second copy.** The rule above is about a citation that *went* wrong; it says nothing about one
> that was false the day it was written. A twelve-citation audit classified each at its
> introducing commit — `git log -S`, not inference — and split **five wrong-when-written against
> seven broken-by-drift**. Only the drift half is reachable by anchoring and grepping, because
> only it has a checkable subject; each of the five named something that had never existed, so no
> citation format would have caught them. The other rule — *a citation is not written until its
> target has been read* — is therefore stated once, in `README.md`'s **Engineering rules**,
> alongside its evidence: `docs/ARCHITECTURE.md` promoting
> `crates/frontend/tests/portability.rs`'s `this_crate_still_forbids_unsafe` from one crate to
> four, written **during** a pass that was correcting a different false citation. Two files, one
> rule each, and a link between them. Writing it out in both is the defect this section is about.

### A verification that cannot ask its own question

`M8.md` Decision 2 needs to know which `Ctrl` chords a browser lets a page cancel. The obvious
instrument is browser automation: open a page that logs `keydown`, inject `Ctrl+P`, see what
happens. **That check is invalid, and it is invalid in a way that returns green.**

A key injected through the Chrome DevTools Protocol — `Input.dispatchKeyEvent`, which is what every
automation tool in this environment ultimately calls — is delivered **to the page** and does not
traverse the browser's own shortcut layer. So an injected `Ctrl+P` arrives as an ordinary
cancellable `keydown` **whether or not a real `Ctrl+P` would open the print dialog**. Injecting a
`KeyboardEvent` from page script is worse: `code` is whatever the script says.

**This is a distinct failure shape from the ones already recorded here, and the distinction is the
entry.** The catalogue above has two kinds. A gate that *nothing runs* produces no observation at
all. A gate that *passes vacuously* runs, and measures the wrong artefact — the codegen probe read
a real file that did not contain the subject. **This third kind runs, measures the right artefact,
and asks a different question than the one the caller meant** — the tool answers *"is this event
cancellable when delivered to the page?"* with perfect accuracy, and the caller wanted *"is this
event delivered to the page?"*

That makes it the most dangerous of the three, because the two previous kinds are catchable by the
remedies already adopted — a corpus guard, and *"every gate needs one assertion whose failure means
'I was not looking at the thing'"*. **Neither works here.** The subject is present, the run is
real, and a positive control would confirm the injection arrived, which is exactly the thing that
is true and irrelevant. There is no assertion that fails, because the apparatus is working
perfectly.

**What separates it from a gauge, since it superficially resembles one:** a gauge has no fact of
the matter. Here there is a fact — a real `Ctrl+8` either reaches the page or does not — and the
instrument simply cannot reach it. So it is an unobservable defect, and unobservable defects are
resolvable by evidence from **outside** the machine. The evidence is a person at a physical
keyboard, and that is in the register above as the settling condition.

**The general rule: before trusting an instrument, state the question it actually answers and
compare it, in words, to the question you asked.** Where the instrument's mechanism bypasses the
layer under test, no amount of care in the assertions recovers it — the check must be refused, and
refusing it is a result. `M8.md` records this in its preamble rather than in a footnote, which is
the right place for it, because a reader who reaches Decision 2 without it will write the automated
check and believe the answer.

### A fallback is only a fallback on the platform it was reasoned about

`crates/frontend/src/keymap.rs` states the rule that decides which chords exist, and earns it by
what it *excludes*: `SYMBOL SHIFT`+`(` is left unbound because `(` is a shifted key on a PC and
binding an unshifted host key to it would put the two keyboards' shift states into disagreement.
The exclusion is safe, the file says, because those combinations *"remain typeable the way the
hardware types them, by holding the mapped `CAPS SHIFT` or `SYMBOL SHIFT` and pressing the digit."*

That is `Ctrl+8` and `Ctrl+9`. **Chrome uses both to switch browser tabs and does not offer them to
the page at all** — not "offers and then overrides", so `preventDefault` has nothing to cancel.

**The design is not inconvenienced at its edges; it is broken at the exact point where it chose to
lean on the hardware**, and it is broken for the parentheses, which no BASIC program avoids.

**The class, which is what makes this worth a section rather than a bug report.** An excluded case
is normally free — you exclude it *because* something else already covers it. That coverage is a
**fallback**, and a fallback is a claim about the platform. It is invisible in the table, it is
never gated, and it is the first thing a new target takes away. **So when a rule's exclusions rest
on a fallback, the fallback is part of the rule and travels with it — or fails to.** The
transferable habit: when a design says *"and X is still reachable by other means"*, write down
**which** means, because that sentence is the load-bearing one and it is the one nobody re-reads
when the target changes.

It is the same shape as *"a coverage claim names a run"*, one level up: an exclusion claim names a
**platform**, and naming the wrong platform makes the whole exclusion wrong at once.

### The answer was in a vendored file nobody opened

`host.rs` listed what a real WASM build would still need, and one item read *"a browser reserves
some of the keys `keymap` binds — `F5` reloads the page."*

**It does not, and it never was going to.** `miniquad-0.4.11` — the version `Cargo.lock` pins —
installs `canvas.onkeydown` and calls `event.preventDefault()` on sapp keycodes 32 (`Space`), 39
(`'`), 47 (`/`), 258 (`Tab`), 259 (`Backspace`), 262–265 (the arrows) and **290–299 (`F1`–`F10`)**.
`F5` is 294. Read at `js/gl.js:1214-1229`, with `case "F5": return 294;` at `gl.js:539`, on
2026-09-01. Every key `crates/frontend` binds in that range is covered, and `F11` (300) and `F12`
(301) are deliberately **excluded**, so fullscreen and the developer tools stay with the browser —
which is evidence the list was designed rather than accumulated.

**This is the cheapest defect in this document and the lesson is proportionally blunt: a claim
about a *dependency* was made as a claim about the *platform*.** The dependency is pinned,
vendored, on this machine, and greppable. Reasoning about what browsers do is hard and
unfalsifiable here; reading the file that sits between us and the browser took one command.

It is `crates/z80`'s corpus lesson arriving from the other direction — *"reading the corpus beat
reasoning from the specification"*, five times on that crate — with the twist that here there was
no specification at all, only familiarity. **Where a vendored artefact sits between this project
and a platform, it is the artefact that decides, and it is the first thing to read.**

### A gauge has no fact of the matter; an unobservable defect has a fact that nothing reaches

[`MACHINE.md`](MACHINE.md)'s rewritten verification plan draws this distinction and it is the
sharpest thing in that document, so it is recorded here as well — deliberately, and not as a
duplicated fact. **What lives there is the derivation** (that sampling `/INT` at the instruction
boundary with contention at 14335, and at the instruction's last T-state with contention at 14336,
are the same machine — agreeing on all 68 rows and on the detection row). **What is recorded here
is the rule that follows**, because this is the register a person consults when deciding whether to
commission a test, and that is the moment the distinction pays.

| | A gauge | An unobservable defect |
|---|---|---|
| Is there a fact of the matter? | **No.** Two descriptions, one machine | **Yes.** One of the two is wrong |
| What a test would find | Nothing, ever, by construction | Nothing *yet*, for a stated reason |
| Correct action | Pick a convention, record that it is one, stop | Write it down where the register lives; fix it if a rule decides it |
| Can evidence resolve it? | **No.** No evidence exists that could | **Yes** — but only evidence from *outside* the machine |

**They look identical from a run log and they are opposites.** Both present as a test that stays
green under a change you expected to break it. Reading that green as "the model is right" is wrong
in both cases and wrong in two different directions: for a gauge there was nothing to be right
about, and for an unobservable defect something is wrong and the instrument cannot see it.

**Two entries in this document are now instances, and neither was recognised as one at the time.**
The `/INT`-sample-point pair is a gauge, and the acknowledge was *mistaken for* a gauge and was
actually the second kind — *"no test can distinguish the two models"* was true of software running
on the machine and false of a test driving the bus, which `M7.md` Decision 5 then settled on a
documented hardware rule. **Asking which kind you have, first, is what separates a note in the
register from a week of test design.**

The third kind is the one directly above: an instrument whose *mechanism* bypasses the layer under
test. A gauge cannot be resolved by any evidence; an unobservable defect can be resolved by
evidence from outside; **a bypassed layer can be resolved by evidence the tool at hand cannot
produce** — and only the last of the three is fixed by reaching for a different tool.

### A primary source and the silicon disagree, and the implementation follows the silicon

M7's per-bank contention has a documented answer and a true one, and they are different. The
Sinclair *Servicing Manual* §4.11 documents contended pages **4–7**. The machine contends **1, 3,
5, 7** — a HAL10H8/PCB defect that Amstrad corrected only in the +2A/+3.

**The implementation follows the silicon and writes the disagreement down rather than overriding
it silently**, which is the same ruling `crates/z80` makes about the FUSE corpus in reverse: *"the
core models the Z80, the harness models the corpus, including its limitations."* Here the *machine*
is the authority and the *manual* is the description, so the manual is the thing that gets a note.

**The sharp part is what would have hidden it.** The two rules **agree on banks 5 and 7** — and
bank 5 is the screen. So a model built from the manual would pass anything that exercised only the
display file, which is most of what an emulator is casually tested with. The disagreement lives
entirely in banks that ordinary software touches only once it is paging. **That is why the
contention gate sweeps all eight banks rather than checking the screen and generalising**, and it
is a second instance of *exhaustive on one axis can be weaker than a sample on another*: a test of
the screen bank is exhaustive over everything a 48K has and blind to the axis the 128 adds.

The general rule, and it is not "distrust manuals": **when a primary source and the artefact
disagree, record both and say which one the code follows.** A model that silently matches the
source is unfalsifiable against the hardware; a model that silently matches the hardware leaves the
next reader to rediscover why the manual is wrong.

### A hazard named as future work is optional; a shape that refuses the wrong version is not

`M7.md` Decision 4 predicted the `.z80` T-state defect in writing — the file, the constant, the
reason the assertion would stay green, the test that had to change, and `17727` printed in its own
parenthesis — and it shipped anyway. The instance, its arithmetic and its measured cost live there,
under *"Prose can name a defect; only a type or a gate can refuse it"*, and are not repeated here.
What belongs in this register is the diagnosis, because it is not carelessness and it is not about
snapshots.

The instruction asked for a **task** — a test that *"must become exhaustive over both frame
lengths"* — and a task is satisfied by a reader who agrees with it and then does something else,
with **nothing going red in between**: the defect was invisible to every gate that existed, because
our reader shared our writer's misreading, so `parse(write(s)) == s` stayed green while every file
we wrote disagreed with every other emulator. There was no moment at which skipping it had a cost.
The two instructions in the same document that needed no such paragraph asked for a **shape**
instead — Decision 1's `paging_port_at_reset` is a field that must be assigned, Decision 2's
`BankIndex::new` is a constructor that masks — and neither requires anyone to remember anything. So
does the fix that finally landed: `quarter_t_states(model)` takes the model as an argument, so a
caller cannot fail to supply one, and `MODELS` is an array the counter gates iterate rather than a
reminder to add a second case. **Where a hazard is identified, spend the sentence on the shape that
makes the wrong version unwriteable, not on the task of writing the right one.**

It has a sibling above — *an unenforced instruction to re-measure is the same defect as an unrun
gate*, under *Comments rot at milestone boundaries* — and the remedies differ in a way worth keeping
apart: that one is closed by making compliance cheaper than re-deriving, this one by leaving nothing
to comply with.

**The same distinction decides how a citation is written, and there is a measurement for it.** A
bare `file:line` asks a future reader to keep it true; an anchor — a function name, a constant, a
quoted fragment — is a shape that survives the file moving, because `rg` re-finds it. A citation
gate run this evening found **10** of this repository's 122 resolvable line citations carrying one,
and `docs/M7.md` grew from 1984 to 2022 lines inside a single session, turning a citation into it
red with nobody touching either the citing sentence or the cited one. No convention about *where* a
citation lives can fix that; only removing the line number can.

### A corpus nothing reads is not evidence, and having it makes it look like evidence

On 2026-09-02 the 128's timing constants were graded for the first time and **two of them were
wrong**: `first_contended_t_state` read 14361 where the hardware wants **14362**, and the interrupt
window read 32 where the hardware wants a value in **`33..=43`**. The machine was red on **62 of 70**
rows. The correction is in [`MACHINE.md`](MACHINE.md) and in `crates/spectrum/src/timing.rs`; what
belongs here is why it took a milestone, because none of the six reasons is about a 128.

**1. The corpus was already on disk.** `timing_tests-128k_v1.0.z80` was fetched, hashed,
licence-checked and written up in three documents on 2026-09-01 — and **read by nothing** until the
following day, while the constants it refutes were being defended at length in prose four files
away. `timing_oracle.rs` said *"Nothing here reads it"* in its own module documentation, accurately,
for the whole of that time. This register already carries the sibling shape — *an `#[ignore]`d gate
that no pipeline executes is not a gate* — and this is the same defect with the gate missing
entirely rather than skipped. **Running the corpus you already have outranks any argument about what
it could show.**

**2. A missing results page was allowed to stand in for a missing corpus.** The argument that
retired the question read: there is no 128 results database, so the anti-circularity leg the 48K
enjoys has no 128 counterpart, so the 128 cannot be graded. Every clause of that is true and the
conclusion does not follow — because **the 48K's grading never came from the database either.** That
page publishes eight categorical columns and *no numbers at all*; it supplies the argument that the
tables are not one author's emulator, not the tables. The expectations have always come from inside
the file, and the 128 file has such a table. **When an argument concludes that something cannot be
measured, check that it is talking about the instrument and not about the instrument's pedigree.**

**3. A census counts copies, not witnesses.** The window was held at 32 because implementations split
*three to two* — ZEsarUX, rustzx and MAME against Fuse and JSpeccy. The three were wrong. The same
split recurs on the contention offset, and the "two documents" behind 14361 were **one text**: the
Sinclair Wiki's oldest revision is a logged copy of the World of Spectrum FAQ, by the same author who
wrote Fuse. Counting agreeing artefacts measures how often a number was copied, which is not
evidence about the number.

**4. A derivation predicted the answer and a vote overruled it.** The band `33..=43` was *derived*
from the suite's own instruction stream before anything ran, printed in the constant's own comment,
and correctly labelled a prediction to test. Hardware landed on it, both edges, to the T-state. The
error was not the reasoning but the decision taken against it — and the decision was defensible on
the day, because a derivation held out of a shipped value is the right call **while nothing has run
it**. It stopped being the right call the moment something could.

**5. When a source's figure and its own derivation disagree, it has told you its error bar.** The
FAQ states 14361 and its own `63 × 228` geometry implies 14363. **The hardware is exactly between,
at 14362** — which is what a two-T-state internal inconsistency normally means, and which neither
number alone could say. The file that quoted the source recorded the figure and not the
disagreement. **Transcribe the contradiction, not the conclusion.**

**6. The lineage carried the right number under the wrong name.** 14362 is not a new number: Fuse and
rustzx already store it, as `top_left_pixel`, and subtract one. Six implementations do. So the value
the hardware wants was inside the lineage the whole time, one conversion away, and this project
**inherited the conversion rather than the number**. The documents and the implementations were never
disagreeing about a quantity — they were agreeing about a quantity and disagreeing about which event
it names. **A figure and its coordinate system travel together or they do not travel.**

**And a seventh that is about this register rather than about the constants: none of it was ever an
open row.** The 128's numbers were argued over for a milestone in a doc comment, with an evidence
table, a census, and a falsifiable prediction — everything the Open table exists to hold — and the
Open table never heard of them. This document is named after that failure and still took it. **An
argument long enough to need an evidence table is an item, and belongs where items live.**

## How this project is verified

Three tiers, and the distinction between them is the point:

1. **An external oracle decides correctness.** FUSE vectors now, `zexdoc` at M3, `zexall` at
   M4. Not opinion, not self-assessment — `OK` or not.
2. **Regression tests must be proven to bite.** A test that does not go red on the original
   defect is decoration. Every mutation is verified to have *landed in the file* before its
   verdict is trusted, because a failed edit and an unbreakable guard produce the same exit code.
   **M5 adds the two ends of that rule that were missing.** The *restore* must be scoped to what the
   mutation process itself changed — `git checkout --` discards concurrent uncommitted work and
   produces the same kind of untrustworthy tree. And the *runner* must be made to run everything:
   `cargo test` without `--no-fail-fast` stops at the first failing target, so the integration gates
   never execute, which is indistinguishable from their passing. Both are written up above.
   **A third end has since been found, and it is the driver reading its own output.** Splitting
   `cargo`'s stdout and stderr into separate pipes destroys the interleaving a parser walks, and
   the driver reported eight survivals against eight runs that had every one exited 101. So: merge
   the streams, and make an unreadable run **raise** rather than resolve to a verdict. Written up
   under *A harness reported eight survivals while every one of them exited 101*.
3. **Claims about the emitted code are checked in the emitted code.** "It monomorphises" and
   "it lowers to a jump table" are assertions until someone reads the assembly.

Machine-level timing (contention, floating bus) had no such oracle and was verified against
known-demanding software. That is observation, and it is labelled as observation. **M5 is the first
milestone to run in that mode, and what it produced instead of an oracle is the *ungraded* list —
the properties nothing covers, written down rather than inferred.** That list is a deliverable of
the milestone; see *Still ungraded* in the M5 section.

> **Tier 1 now reaches the machine, and that changes the sentence above rather than replacing it.**
> `tests/timing_oracle.rs` grades contention against T-state counts measured on real Spectrums, so
> *"machine-level timing has no such oracle"* is past tense for the memory and I/O patterns and
> present tense for everything else in that clause — the floating bus is still unmodelled and
> therefore ungradeable, progressive drawing likewise, and the interrupt window's length is now
> *demonstrated* to be beyond the oracle's reach rather than merely unmeasured.
>
> **And the *ungraded* list survived being right.** The item that headed it for two milestones did
> not close by someone deciding it was fine; it closed because the list named exactly what would
> settle it — *"a known-timing test program that reports measured T-state counts"* — and somebody
> went and found one. That is what the list is for, and it is the argument for keeping it when it
> is uncomfortable: **a well-formed open item is a search query.** It closed into three narrower
> items, all of them still open, which is the normal shape of progress here and not a disappointment.

---

## Next — M2, costed

> **Historical. This section was written before M2 and is kept as it stood.** M2, M3, M4 and M5 have
> all merged since; nothing here describes what is next. It is left in place rather than deleted
> because the costing turned out to be accurate and the reasoning is reusable — but a reader
> scanning for the current plan should stop at the M5 section and the Open register, not here.

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
