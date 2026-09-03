# Changelog

Notable changes to the published surface of **this workspace** — `crates/z80`, `crates/spectrum`
and `crates/frontend`. Milestones are recorded even before the first release, because each crate's
API is being frozen decision by decision and the moment each one stopped being free is worth
knowing.

> *This line named two crates while the second blockquote below records the deliberate decision to
> widen it to three, taken on 2026-09-01.* The reasoning moved and the scope sentence did not, so
> `crates/frontend`'s entries sat in a file that said it did not cover them — which is the exact
> shape `docs/STATUS.md` calls *"a row nobody re-reads keeps asserting, with the register's
> authority, a thing the code stopped doing"*. `crates/page` is deliberately **not** in the list:
> its whole surface is an FFI seam consumed by `crates/frontend` and nothing outside this
> workspace can call it, so a change there is a change to `crates/frontend`'s behaviour and is
> recorded as one.

**Every entry names its crate.** Entries written before M5 were `crates/z80` by definition and have
been labelled as such; nothing in them changed.

> **The scope was widened at M5, and the decision is recorded because it was a real choice.** This
> document opened by declaring itself a record of *"notable changes to the published surface of
> `crates/z80`"*, which was exactly right while `spectrum` was a stub. It is not one any more:
> `Spectrum`, `Ula`, `Memory`, `Keyboard`, `Key`, `Frame`, `Colour`, `screen::read_text`,
> `timing::Clock` and the rest are `pub`, `#![deny(missing_docs)]` is on, and `crates/frontend` is
> coming at M8 to consume them. There were two options and the other one was viable:
>
> - **Keep it z80-only and point elsewhere for `spectrum`.** The natural "elsewhere" is the coverage
>   table in `crates/spectrum/src/lib.rs`. **Rejected**, because that table records *what is graded
>   and by what* — it is an evidence register, not an API register, and it would answer "is this
>   tested?" for a reader asking "did this break?". Making one document serve both questions is how
>   the two open registers in `STATUS.md` and `ARCHITECTURE.md` came to disagree about four facts in
>   one session.
> - **Widen it.** Chosen. The stated purpose — *the moment each decision stopped being free* —
>   applies to `spectrum` identically and more urgently, because `spectrum` acquired its entire
>   surface in one commit while `z80` froze its own one decision at a time.
>
> The cost of widening is that a reader after a z80-only history has to skip entries. That is
> cheaper than a surface with no record at all, which is what `spectrum` had between `2157331` and
> this entry.

> **Widened again on 2026-09-01, to `crates/frontend`, and the reason the question came up is
> instructive.** `docs/M8.md` noticed that this file's opening line names `crates/z80` and
> `crates/spectrum` and that **`crates/frontend` is in neither list**, so M8's public additions
> would have had no home here — and it declined to settle it, on the ground that *"the same
> widening decision was taken deliberately once and should be taken deliberately again."* That was
> the right call and this is that second deliberate taking.
>
> **What forced it was not M8 but M7.** Extending `zx-shot` to photograph the 128 changed
> `frontend::media::start`'s **signature**, and with it the meaning of an existing command line:
> `zx a.rom b.rom` used to mean *"the last ROM wins"* and now means *"a 128, editor first"*. **A
> reinterpreted argument is the quiet kind of breaking change** — no signature a compiler can
> object to for the person typing it — which is precisely the category this file exists to make
> loud. A crate whose surface can break a user's habit without breaking their build needs a record
> more than one that cannot, not less.
>
> The argument against widening is the one the M5 entry already answered: the alternative
> "elsewhere" is `crates/frontend/src/lib.rs`'s gated / not-gated tables, and those are an
> **evidence** register, not an API register. Pointing a reader asking *"did this break?"* at a
> table answering *"is this tested?"* is the same conflation rejected above.

## Unreleased — audio, second wave · `crates/frontend` — the tape as loud as the beeper, and a browser page that stops shedding buffers

The follow-up to the entry below, closing the one audible defect it left open — S4, the tape's
loudness taken from the beeper — and removing the allocation churn its browser fix ran on. No
signature moved in either half: what changed is what `mix` computes, and what the page's
JavaScript shim does with a pushed frame — the latter recorded here because `crates/page` and
`web/` are the FFI seam `crates/frontend`'s behaviour ships through, per this file's own scope
note above.

### `crates/frontend` — S4: the tape leaves the shared mix denominator

**Changed.** The mix's one shared denominator — `FULL_SCALE`, every source divided by the same
total, so loudness given to the tape was taken from the games — is replaced by a tape-free
`GAME_SCALE`, and `TAPE_GAIN = 0.9` by a derived `TAPE_LEVEL` — the beeper's own level,
`BEEPER_GAIN / GAME_SCALE * HEADROOM`, 0.28125 exactly. The tape is **ruled equal to the
beeper** — the machine played its
tape through its own speaker, at the loudness the screech is remembered for — and the ruling is
one expression rather than two numbers, so a corrected resistor value moves both together. A
lone tape goes from **4.12% to 14.06%** of device full scale, measured at **14.3%** on a fresh
capture; a lone beeper sits at the same 14.06%. The worst-case five-source sum is **0.88125**,
proven under full scale at compile time, so there is **no limiter**; the thin-48K floor's margin
widens from 1.1% to 17.2%. The gate that asserted the old shape is inverted to
`the_tape_is_as_loud_as_the_beeper_at_the_same_level`, and the old quieter-than name is
registered in the deleted-on-purpose register rather than left citable. Both constants were
private, so the published surface is unchanged — what a consumer hears is not, by design.

### `crates/frontend` — the browser page recycles its audio buffers through a transferable free-list

**Changed.** `web/zx_page.js` allocated a fresh buffer per pushed frame — ~3,834 bytes at
48 kHz, fifty times a second, ~187.5 KiB/s — and every one of them became garbage on the audio
thread, the one thread on the page with a hard render deadline. The page now pops a pooled
`ArrayBuffer`, copies the frame into it, and transfers it; `web/zx_audio_worklet.js` posts each
exhausted buffer straight back after its ring copy, and the pool parks at most four buffers
(`FREE_LIST_MAX`). Audio-thread-freed garbage goes to **zero in steady state**, at +16 lines of
code. An empty
pool never blocks and never drops — a miss allocates fresh, which is exactly the old behaviour —
and the worklet's own rule that `process` allocates nothing that outlives it stands as written:
the post-back runs in the port's message dispatch.

## Unreleased — audio · `crates/spectrum` + `crates/frontend` — the tape became audible and the clicking stopped

Two defects a person hears, reported together and fixed together. Neither was a wrong number: both
were a missing connection and a loop that was never closed.

### `crates/spectrum` — `Sample::tape`, the `EAR` line's own field

**Added.** `Sample` gains a third field, and the mix a fifth source. The `EAR` line already reached bit 6 of a `0xFE` read —
M6 closed that, and tapes loaded correctly because of it — but it reached **nothing else**, so a
machine that loaded a tape perfectly did it in silence. On real hardware the socket feeds the
amplifier as well as the ULA, which is why a loading Spectrum screeches and why a person can hear a
leader, a data block and a dropout without looking at the screen.

`Audio::set_tape` is driven from `Ula::advance` — the one place time passes, and therefore the one
place the tape's level can change — **on the tape's own edge rather than on every T-state**, so the
cost on a machine with no tape playing is the `bool` `Tape::advance` was already computing.

> **That sentence said *"the same compare-before-render shape `set_beeper` uses"*, and shipping it
> that way cost +23% of the emulator's speed.** `set_beeper` compares inside itself, which is right
> for a setter called thousands of times a second; `Ula::advance` runs **69,888 times per frame**,
> and Rust evaluates arguments *before* the call, so the timestamp — `Clock::t_states`, a 64-bit
> multiply-add that under this workspace's `overflow-checks = true` compiles to two multiplies and
> two branch-to-panic edges — ran on every elapsed T-state and was then thrown away by the guard
> inside the callee. Measured on `benches/frame.rs`: **+19% to +30% across all eleven cases, mean
> +23%**, on a machine with **no tape in the drive**.
>
> `Tape::advance` already knew when the level moved — `finish_pulse` is the only writer of it and
> runs right there — so it returns that, and the timestamp is computed only when there is something
> to timestamp. `Tape::advance` is `pub(crate)`, so the repair has no public-API delta. No benchmark
> in this workspace played a tape, which is why the regression shipped green; `benches/frame.rs`
> now has a `tape_playing_48k` case.

**`Sample` is `#[non_exhaustive]`, so the new field is not a breaking change**, and the crate still
mixes nothing: the tape is kept apart from the beeper for exactly the reason `docs/M8.md` Decision 9
keeps the beeper apart from the AY. Gated by `the_tape_reaches_the_speaker_and_not_only_the_ear_bit`
and `a_tape_that_was_never_started_is_silent` in `tests/tape_signal.rs`.

### `crates/frontend` — `Resampler::track`, and the end of the dropped frame

**Added**, and it replaces a ceiling that was doing real damage. The emulator paces to the machine's
50.08 Hz; a sound card consumes one second per second off a crystal that agrees with nothing. The
difference accumulates — observed in a browser as 210 ms of backlog after four minutes — so the
frontend used to stop feeding the device once the queue passed 100 ms.

**That is what a person heard as a tick every few seconds.** Declining to push a frame discards
20 ms out of the middle of a waveform, which is a discontinuity, which is a click, recurring on a
period set by the drift. The bound was correct and the mechanism was the wrong shape.

`track` closes the same loop by moving the output rate instead, by at most `MAX_CORRECTION` — 0.5%,
which is **8.6 cents** of pitch and reached only at the extremes of the queue. Nothing is discarded,
so there is no edge to hear. The target is half the buffer rather than its ceiling, because a queue
that is too shallow underruns and an underrun is silence.

`mix` carries the tape at `TAPE_GAIN`, and `FULL_SCALE` grew to match, so the five-source case still
cannot clip — gated by `five_sources_at_full_scale_do_not_exceed_the_headroom`, which was named for
four until this entry.

**Also added: `audio::queue_target`.** The setpoint was an expression in the frame loop —
`device_hz() * page::BUFFER_MILLISECONDS / 2000`, where `2000` fused a unit conversion to a ruling
— inside the one function this crate holds *"to plumbing"* because nothing in it can be graded. It
is a named `const fn` with a test now.

**Signature note, taken while it was still free.** `Resampler::track` was added in this same entry
taking `queued: i32`, carrying `page::audio_push`'s `-1` sentinel into a public signature in a
different crate; it takes `Option<u32>` before either ships. No released version had the `i32`
form.

**Also added: `media::DEFAULT_ROM` and `media::load_named`** — the same commit's de-duplication
half rather than its audio half, recorded because an unrecorded `pub` item is exactly what
widening this file to `crates/frontend` was for. The constant was declared twice, doc comment and
all, once in each binary; the function is the recognise-load-or-say-why-not step `zx`'s drop path
already owned and `zx-shot` now shares — which also changes what `zx-shot` prints for a 128
snapshot on a 48K: `load_named`'s `Error::Model` arm names the ROMs that machine needs, where the
binary's own copy of the decision had no such arm.

**What is still not graded: that any of it sounds right.** Every assertion added here is arithmetic
on a buffer. `crates/frontend/src/audio.rs`'s own table has said so since M8 and it is still the row
to read twice — a person listening is the only instrument for this, and no gate substitutes for one.

## Unreleased — MEMPTR · `crates/z80` — an address latch that was nearly always zero

### Changed

- **`CpuState::wz` now carries a real address after almost every instruction**, where it was
  written by exactly one site and was in practice zero. The one site was the `(IX+d)`/`(IY+d)`
  effective-address computation; there are now twenty-eight, all behind the single writer
  `Cpu::set_memptr` — every absolute load and store, the accumulator stores' three-instruction
  quirk, the 16-bit arithmetic, both I/O groups, every jump, call, return and restart,
  `EX (SP),HL`, `RLD`/`RRD`, all four block families including their repeat, and interrupt and
  NMI acceptance. The count is a command, not a claim:
  `grep -c 'self\.set_memptr(' crates/z80/src/instructions.rs crates/z80/src/lib.rs` reported
  26 and 2 on 2026-09-01.

- **`BIT n,(HL)`'s undocumented flag bits 3 and 5 change observably**, which is the half of this
  a consumer meets without ever reading `wz`. Both memory forms of `BIT` take those two bits
  from `wz`'s high byte. For the `(IX+d)` form that was already the instruction's own effective
  address; for the `(HL)` form it was whatever the last *indexed* access had left, which in a
  program touching nothing indexed is zero. `F` after `BIT n,(HL)` is therefore a different
  value than it was, and `CpuState::af` reports the difference.

### Why this is a change and not a break, and not an addition either

The ruling is written out because neither of the easy answers is right, and because this file
has already ruled once that a change with no compiler-visible signature can still be breaking.

- **It is not breaking in the mechanical sense.** No item is added, removed, renamed or
  retyped; `CpuState` has exactly the fields it had. Everything that compiled still compiles,
  and `cargo-semver-checks` has nothing to report. That is necessary and not sufficient — the
  M7 entry above turns on precisely the case where it is not sufficient.

- **It is not an addition.** `wz` existed, `BIT n,(HL)`'s bits 3 and 5 existed, and both had
  values before. What changed is *which* values. An `Added` entry claims a reader gains
  something and loses nothing, and a reader who could previously assume `wz == 0` has lost that.

- **It does not meet this file's own bar for the quiet breaking change, and the difference is
  which side of the interface moved.** M7 reinterpreted the **caller's input**: `zx a.rom b.rom`
  means something else now, so a habit that was correct became incorrect and the user is the one
  who breaks. Here the caller's input is untouched. What moved is the **core's answer**, from
  wrong to right — and against a contract this crate had already written down. `CpuState::wz`'s
  own doc comment says a snapshot *must* carry it because *"`BIT n,(HL)` executed just after a
  load is restored can report bits the loading machine never computed"*; that sentence describes
  a live latch and was published while the latch was inert. `docs/STATUS.md` carried the gap as
  an open row with a settling condition. **Delivering a fix the documentation already promised
  is keeping a contract, not breaking one**, and filing it as breaking would put a defect and a
  reinterpretation in the same category.

### One consequence, named rather than waved past

`crates/spectrum`'s `snapshot::UNPRESERVED` records that no `.z80` or `.sna` format carries
`wz`, so the parser sets zero — and adds that it is *"observable only through the undocumented
flag bits of a `BIT n,(IX+d)` executed before anything else sets it."* **That second half was
true when it was written and is not now.** It was free while `wz` was nearly always zero in the
running core as well as after a load: dropping a field that was zero anyway costs nothing
observable. It is not free now. A running core maintains the latch, a load resets it, and the
first `BIT n,(HL)` after the load can therefore report bits the saving machine never had —
which is exactly the hazard `CpuState::wz`'s doc names, made reachable by this change rather
than by anything in the snapshot layer. The snapshot layer is unchanged and its round trip is
still green, because a field dropped in both directions is what that suite is built to catch;
what has changed is the cost of the drop. Flagged here, not fixed here.

## Unreleased — M7 · `crates/frontend` — the shell learned there are two machines

### Changed — and one of these can break a command line without breaking a build

- **`media::start`** now takes `&[&[u8]]` rather than `&[u8]`, and **the count of ROM images is
  what names the machine**: one is a 48K, two are a 128 with the first paging in at reset. A
  second constructor (`start_128`) was rejected — it would put the decision *"which machine is
  this"* in every caller, before any of them had looked at what the user named.
- **`media::Error::RomCount { given }`** added. `Error` is `#[non_exhaustive]`, so this is not
  breaking.
- **`zx` and `zx-shot` accumulate `--rom` rather than overriding.** This is the behaviour change
  above, seen from the command line: a repeated `--rom` used to be a correction and is now an
  addition, and a third is an error rather than a silently dropped file.
- **`zx-shot --settle N`** added — frames run after the last key. The default suits a line of
  BASIC and not a ROM re-page: selecting *48 BASIC* from the 128 menu photographs as a black
  rectangle at the default, because the machine is genuinely mid-clear.

### Not changed, and recorded so it is not proposed as a tidy-up

- **`crates/frontend`'s `unsafe_code = "forbid"` stays.** `docs/M8.md` Decision 4 left the choice
  open; it is taken there. The wasm download glue goes in a small crate of its own rather than
  relaxing this crate's posture — `forbid` is not `deny`, and a crate that takes files from
  strangers keeps the structural guarantee.

## Unreleased — M6 · `crates/spectrum` — the applier and the tape

### Added — `pub mod snapshot`

- **`snapshot::Snapshot`** (`pub cpu: z80::CpuState`, `pub border: Colour`,
  `pub frame_t_state: u32`; the bank array **private**), re-exported as `spectrum::Snapshot`.
- **`Snapshot::bank(BankIndex) -> Option<&[u8; PAGE_SIZE]>`**.
- **`snapshot::Error`** (`#[non_exhaustive]`, `Copy`, allocation-free), whose every variant names
  the byte or offset that failed.
- **`snapshot::z80::parse` / `snapshot::z80::write`**, and **`snapshot::sna::parse`**.
  **There is no `sna::write`**, deliberately and permanently until something needs one: a `.sna`
  writer must push `PC` onto the guest's stack, destroying two bytes of the RAM it is recording, and
  a save that modifies what it is saving is not a save.

> **This entry was missing, and its absence is the exact defect this file exists to prevent.**
> `pub mod snapshot` and the six items above shipped in the M6 merge with **no changelog entry at
> all**, while the applier and the tape below were written up at length. A published surface with no
> record is what `spectrum` had between `2157331` and the M5 entry, and the widening note at the top
> of this file argues that case; the same gap reopened one milestone later inside the milestone that
> opened the module. Added on 2026-09-01 from the crate rather than from the design document,
> because the design document is where the items were *proposed* and the crate is where they are.

**The parser's fields are public because there is no invariant to protect**, following `CpuState`'s
own precedent: `Colour::new` wraps into range and an out-of-range `frame_t_state` is absorbed by the
clock's documented rollover. **The banks are private** because *which* bank indices are meaningful is
a property of the model, and the parser is where that is decided — a `Snapshot` that reaches
`restore` cannot name a bank the machine lacks, which is a parse-don't-validate boundary rather than
a style choice.

**`Error` is the module's whole hostile-input contract, and it is closed structurally rather than by
care.** The crate builds with `panic = "abort"`, so a panic on a malformed file kills the process and
`catch_unwind` is not a backstop. There is no slice indexing anywhere in the module — every byte
moves through one `Reader`/`Writer` pair — no `unwrap`/`expect`/`panic!` in the production half, and
**no allocation is ever sized from the file**. All three are asserted by tests over the module's own
source, and the indexing scanner carries positive *and* negative cases so it cannot pass by finding
nothing. `docs/STATUS.md`'s M6 section has the account.

### Added — `Spectrum::snapshot()` and `Spectrum::restore()`

- **`Spectrum::snapshot(&self) -> Snapshot`** and **`Spectrum::restore(&mut self, &Snapshot)`**.

**`Ula`, `Memory`, `Clock` and `CpuState` gained no public items**, and that is the whole of
`docs/M6.md` Decision 1's argument rather than a happy outcome. A snapshot applier needs three
things the machine does not otherwise expose — set the border without performing an `OUT`, set the
frame position without adding elapsed time, and write RAM bank by bank. Placed in a separate crate
they would have had to reach through `spectrum`'s **public** surface, making all three permanent
public API for one in-workspace caller. As modules inside the crate they are `pub(crate)`:
`Ula::set_border`, `Ula::set_frame_t_state`, `Ula::insert_tape`, `Ula::tape_mut`,
`Clock::set_frame_t_state`, and a private `Ula::advance`.

`Memory::bank` / `bank_mut` are **still absent**, and stay absent. The three banks a 48K snapshot
carries are exactly the three its slot map exposes, so `restore` reaches them through the already
public `Memory::slots()` — which also means the code is derived from the slot map rather than from
the 48K's particular one, and will not need rewriting when M7 moves it. They become unavoidable at
M7, where a 128 snapshot carries banks that are paged out and therefore have no address at all;
adding them now would be a semver commitment with no caller.

**Three things deliberately survive a restore**, each a convention rather than a measurement,
because no format carries the field: `Spectrum::frames()` (the machine's uptime — the boot gate
asserts on it and the FLASH phase derives from it), the ROM, and the tape.

### Added — `pub mod tape`

- **`tape::Tape`**, with `new`, `play`, `stop`, `rewind`, `level` and `pulses`.
- **`tape::Error`** (`#[non_exhaustive]`, `Copy`, allocation-free — matching `snapshot::Error`,
  `RomSizeError` and `z80::StepError`).
- **`tape::tap::parse`**.
- **`Spectrum::insert_tape()`** and **`Spectrum::tape_mut()`**.

**The tape is a signal, not a ROM trap**, which is `docs/M6.md` Decision 4 and the largest decision
in the milestone. `Tape` holds a **pulse train** — half-period lengths in T-states — rather than a
block list, so `.tzx` becomes a second converter at M7 rather than a rewrite of the ULA side.

`Tape::new` and `Tape::pulses` are public where the design implied only `play`/`stop`/`rewind`. The
reason is the gates: an expectation computed by its own subject is a tautology, so the gate that
grades the `.tap` converter has to build a train by hand and read one back with a decoder it owns.

`Spectrum::tape_mut` returns `&mut Tape` rather than `Option<&mut Tape>`: a drive with no cassette
and a cassette with nothing on it drive the `EAR` line identically, so there is no state to
distinguish and no option to unwrap.

**Deliberately absent, and non-breaking to add later:** `Tape::is_finished`, `Tape::position`,
`Spectrum::eject_tape`, `tape::tzx`, `sna::write`, `Memory::bank` / `bank_mut`.

### Changed — behaviour, not signatures

- **Bit 6 of a read from port `0xFE` is now driven by the tape.** With no tape inserted it still
  reads **low**, so `IN A,(0xFE)` on an idle machine still returns the `0xBF` that
  `crates/spectrum/tests/keyboard_matrix.rs` has pinned across the full membrane since M5. No
  constant changed and no signature moved; `ula.rs`'s documented table row went from **no** to
  **yes**.
- **`Ula::reset` still does not rewind the tape.** Pressing reset does not rewind a cassette.

### Note for machine authors

Every `Clock::advance` call site in `ula.rs` now routes through one private `Ula::advance` that
moves the clock **and** the tape. Contention advances the clock outside any `Bus::tick`, so a tape
driven from `tick` alone would run slow by exactly the contention a loader suffers — silently.
Measured: 32 `NOP`s cost 128 T-states out of the contended bank and 194 in it.

## Unreleased — M5 follow-up · `crates/z80` — the machine-cycle lengths become contract

### Added

- **`z80::OPCODE_FETCH_T_STATES`, `z80::MEMORY_ACCESS_T_STATES`, `z80::PORT_ACCESS_T_STATES`**
  (`pub const u8` = 4, 3, 4), defined in `bus` and re-exported from the crate root. Additive and
  non-breaking; permanent, because a published constant cannot be withdrawn without a major bump.

**Why these are contract and not implementation detail, which is the whole of the decision.** The
`Bus` documentation already published this table in prose — it had to, because an implementation
cannot honour the trait without it. `Bus::tick` is called once per T-state and carries no cycle
boundary, so the only way to tell a tick that belongs to the cycle a transfer just opened from a
standalone internal cycle that must contend on its own account is to **count**, and counting needs
the length. These three numbers are the decoding key for the call stream, not a number the machine
happens to want. A crate that publishes the key in prose and keeps the machine-readable form private
is asking every implementor to re-transcribe it.

`crates/spectrum` was doing exactly that. Its `ula.rs` held `OPCODE_FETCH_CYCLE`/`MEMORY_CYCLE`/
`PORT_CYCLE` with the same three values, and `contention_magnitude.rs` recorded the consequence in
its *what is not graded here*: *"those two sets of constants are duplicates that no gate compares, so
if they diverged every contended access would be charged wrongly."* The gate it offered instead was
that a divergence would move a hand-derived total — **a consequence, not a comparison**. The
duplicates are deleted and `Ula` consumes the exported constants, so a divergence is no longer
something to detect: it is unrepresentable.

**What is deliberately *not* exported, because the boundary is the argument.** `INTERRUPT_ACKNOWLEDGE`
and `NMI_ACKNOWLEDGE` stay private. The three above are public because each corresponds to a `Bus`
transfer callback, so an implementation can *recognise* the cycle the number measures. An acknowledge
has no callback — it reads no memory — so a machine has nothing to attach the length to, and
exporting it would hand out a number a `Bus` cannot act on.

**What still grades the values.** Removing the duplication removes the risk of two implementations
disagreeing; it does nothing about both being wrong together, and one definition means a wrong value
now moves the core's accounting and the ULA's in lockstep. So the expectations stay independent:
`contention_magnitude.rs`'s `nominal` column, `io_contention.rs`'s `PORT_CYCLE`, `block_contention.rs`'s
16 and 21, and `crates/z80/tests/bus_timing.rs`'s own `MEMORY_CYCLE` are all written from the
published Z80 figures and none of them import these constants.

### Note for machine authors

`crates/z80/tests/codegen.rs` pins exact codegen counts and was checked across this change: all seven
assertions hold unmoved. `const` items emit no code, so adding three public ones is invisible to the
compiler — which was the prediction, and it was checked rather than assumed.

## Unreleased — M5 · `crates/spectrum` — the machine, and a second published surface

### Added — the whole of it, in one commit

`crates/spectrum` went from a stub to a 48K in `2157331`, so its entire published surface arrived at
once rather than a decision at a time. Recorded here as a single entry because that is what
happened, not because the items are minor:

- **`Spectrum`** — the machine. `new`, `reset`, `step`, `run_frame`, `run_frames`, `frames`,
  `frame_t_state`, `render`, `cpu_state`, `set_cpu_state`, `fault`, `ula`/`ula_mut`,
  `memory`/`memory_mut`, `keyboard`/`keyboard_mut`, `border`.
- **`Ula`** — the bus the CPU is instantiated over: `new`, `reset`, `memory`/`memory_mut`,
  `keyboard`/`keyboard_mut`, `clock`, `border`, `interrupt_asserted`, and `FLOATING_BUS_BYTE`. It
  implements **all six** `z80::Bus` methods, `fetch` included — the defaulted one is overridden here
  rather than inherited, which is what retires the machine-cycle heuristic below.
- **`Memory`** — slots to banks, with 48K as the locked configuration: `spectrum_48k`, `read`,
  `write`, `is_contended`, `slot_at`, `slots`, plus `Slot`, `BankIndex`, `RomIndex`,
  `SPECTRUM_48K_SLOTS`, `RomSizeError` and the four size constants.
- **`Keyboard`** and **`Key`** — `press`, `release`, `release_all`, `is_pressed`, `read`, with
  `HALF_ROWS`, `KEYS_PER_HALF_ROW` and `RELEASED`.
- **`screen`** — `Frame`, `Colour`, `Attribute`, `render`, `flash_phase`, `pixel_address`,
  `attribute_address`, **`read_text`**, and the display/frame geometry constants.
- **`timing`** — `Clock`, `delay`, `T_STATES_PER_FRAME`, `T_STATES_PER_LINE`, `LINES_PER_FRAME`,
  `INTERRUPT_T_STATES`, `FIRST_CONTENDED_T_STATE`.

### Removed

- **`pub mod machine_cycle`, and with it the whole reconstruction heuristic.** It exported a type no
  caller could obtain — nothing public returned one — while letting any caller *construct* one and
  drive it into a state the real bus never produces. A public type reachable only by building it
  wrongly is a surface that can only be misused, and it is gone: `Bus::fetch` states outright what
  the deferral existed to infer. **−161 production LOC, −8 branches and −2 nesting levels on the
  `Bus::tick` path.** Nothing on `crates/z80`'s side changed to make room for it — `fetch` there is
  additive and defaulted — so this removal is `spectrum`'s alone. The −161 and the branch and
  nesting counts are the implementer's measurement of the change; the deleted file's own production
  half is 191 lines, which is the part re-derived independently.

- **`Clock::advance` left the published surface**, narrowed to `pub(crate)` rather than deleted —
  every real caller is inside this crate. As a *public* method it was a **no-op**: `Clock` is `Copy`
  and `Ula::clock` returns by value, so an outside `ula().clock().advance(n)` auto-refs a temporary,
  compiles clean, and advances nothing. A published method that silently does nothing is worse than
  an absent one, because a caller reads the name and stops looking — the same failure class this
  project records as *"the most dangerous defect was not a bug in a comparison, it was a comment"*.
  `crates/spectrum/src/timing.rs` — `const CONTENDED_T_STATES_PER_LINE` — now carries that
  reasoning at the declaration.

### Note for machine authors

**`Spectrum::step` returns T-states and that number is not the clock.** Contention is added on the
bus's side and is not included in it. Use `Spectrum::frame_t_state` for time; `step`'s return is
useful only for asserting an instruction's nominal length. A frame loop that sums `step` returns
gets the instruction count right and the time wrong, with nothing failing — which is why
`run_frame` watches the frame *counter* rather than a T-state budget.

**`Spectrum::render` is a snapshot, not a record of what the ULA drew.** Progressive drawing —
multicolour, border stripes — is not modelled. Nor are the floating bus or keyboard ghosting. The
per-property list of what is and is not graded lives in `crates/spectrum/src/lib.rs`.

## Unreleased — M5 · `crates/z80` — `Bus::fetch`, the M1 opcode fetch

### Added

- **`Bus::fetch(&mut self, addr: u16) -> u8`**, defaulted to `Bus::read`. Non-breaking: every
  existing implementation compiles and behaves identically without touching it, and `spectrum`
  was left unmodified **at the moment it landed** as the proof — `cargo test -p spectrum` reported
  98 passed, 0 failed on the untouched machine. It has since opted in, which is the point of a
  defaulted method: non-breaking on arrival, load-bearing when adopted.

  It exists because M1 is the one machine cycle whose **length** a machine cannot infer from
  the call stream. A write is three T-states and a port access is four, but a read is three for
  an operand and four for an opcode fetch — so `LD A,B` (one M1 cycle) and the read-modify half
  of `INC (HL)` (a three-T-state read, then an internal cycle) emitted **byte-identical**
  streams: one transfer callback followed by four ticks at the same address. A contention model
  owes one stall for the first and two for the second, and nothing in the stream said which.

  This was not a speculative addition. **`crates/spectrum/src/machine_cycle.rs` reconstructed**
  cycle boundaries by deferring the fourth tick until a fifth disclosed the shape, at a residual of
  exactly one contention point on the read-modify-write family (`INC`/`DEC (HL)` and `(IX+d)`, the
  `CB` operations on memory, `EX (SP),HL`), pinned by a test of its own. `fetch` removes the
  ambiguity at the source rather than reconstructing it downstream, and **the machine has taken it:
  `Ula` implements `fetch`, `machine_cycle.rs` is deleted, and the residual is gone rather than
  pinned.**

  > **Correction, and it is not a tidy-up.** This paragraph described the file in the present tense
  > after it had been deleted, and it described the residual as *"exactly one contention point — 0
  > to 6 T-states"*. **That phrasing conflates two different quantities.** 0–6 is the *isolated
  > stall* a single cycle would be charged, taken alone. The *net observable error* is smaller and
  > differently shaped: swept over all 448 start positions, `INC (HL)`'s total was wrong by 0 or 1
  > T-state and never more, because dropping a stall opens the next cycle earlier, where the pattern
  > charges most of it straight back. Read as an error bound, "0 to 6" invites adding the missing
  > stall to an observed total — which two independent derivations did, landing on 30 T-states where
  > the machine cycles give 26. `docs/STATUS.md` carries the full account under *A missing stall
  > cannot be added to a total*.

- **The rule, for implementors:** `fetch` is called once per M1 cycle that reads memory, which
  during `step()` is also exactly once per `R` increment — prefix bytes included, since `DD`,
  `FD`, `CB` and `ED` are each their own M1 cycle. Everything else stays on `read`: every
  operand, data and stack access, and specifically the `DDCB`/`FDCB` displacement and opcode
  bytes, which the hardware takes as ordinary three-T-state reads and which is why `R` advances
  twice across that four-byte instruction rather than four times.

### Two rulings this forced, both settled by what the hardware does to `R`

- **A halted CPU's discarded byte is a `fetch`.** A halted Z80 has not stopped: it keeps issuing
  M1 cycles and executing an internal `NOP`. The Z80 cannot refresh without an M1 cycle and this
  cycle refreshes, so it is an M1 cycle — four T-states, address driven from `PC` — differing
  from any other opcode fetch only in that the byte is thrown away.

- **An interrupt acknowledge is *not* a `fetch`, and is the one exception to the rule above.** It
  reads no memory: `/IORQ` replaces `/MREQ` and the device answers on the data bus, which in
  mode 0 *is* the instruction and reaches the core as `interrupt`'s `data` argument. Calling
  `fetch` there would name an address the machine would be entitled to contend and to serve from
  its memory map. So an acknowledge refreshes `R` without fetching, and **fetch-per-refresh is
  exact across `step()` and off by one for each accepted interrupt.** Anything reconstructing M1
  cycles from the bus alone must add those itself.

### The invariant those rulings force, stated with its scope

The tempting rule is *one `fetch` per `R` increment*. It is nearly true, it is why the method can be
described in one line, and it is wrong as stated. The exact form:

> `R` increments **once per M1 cycle**. `fetch` fires **once per M1 cycle that reads memory**. The
> interrupt acknowledge is the only cycle that is neither — it refreshes without fetching. So the
> correspondence is **exact across `step()`**, where a frame loop spends all of its time, and **off
> by one per accepted interrupt or NMI**.

A halted CPU's cycle keeps the count, because it does read memory — it re-fetches the `HALT` opcode
and discards the byte. The acknowledge does not, because `/IORQ` replaces `/MREQ` and no address is
presented to memory at all.

**The exception was not found by thinking harder about the rule; it was found by trying to write its
test**, at which point the acknowledge path had to be given a verdict and refused to fit. An
invariant asserted has no scope; an invariant tested acquires one.

### Note for machine authors

The correspondence above is also the gate. `crates/z80/tests/bus_timing.rs` asserts
**one `Bus::fetch` call per `R` increment** across an un-prefixed instruction, a 300-byte prefix
run, a `DDCB` form, an `ED` block instruction mid-repeat and `HALT`, with the interrupt exception
tested rather than left as a footnote. `R` is independently graded by 290/290 and 1045/1045 FUSE
vectors and by `zexall`, so the new method is anchored to something already proven rather than to
a hand-count of call sites — and the check bites in both directions, since a fetch left on `read`
drops one side of the equation while an operand read promoted to `fetch` inflates the other.

**On the machine's side the adoption is graded too**, which is the part a defaulted method usually
leaves unmeasured: removing `Ula`'s `fetch` implementation — so the machine silently falls back to
the default and treats every opcode fetch as a three-T-state read — turns **7 tests red**. Opting in
is therefore load-bearing rather than decorative, and opting back out cannot happen quietly.

## Unreleased — M3 · `crates/z80` — `zexdoc`

### The published surface did not change, and that is the result

M3 added no types, no methods, no variants, no fields. The milestone was a **gate**, not a
feature: `zexdoc` — a self-checking CP/M binary that runs 5,764,169,610 instructions and
compares CRCs it computes itself against values built into its own image — reports `OK` for
**all 67 test groups**, first run, with no change to `crates/z80/src`.

That is the strongest statement this crate has been able to make so far. FUSE proves each
instruction in isolation; `zexdoc` proves they still hold up in *sequences*, billions deep,
where a wrong flag bit poisons a CRC thousands of instructions after the mistake.

### Note for machine authors

**Throughput at scale is now measured rather than extrapolated.** 46,734,977,142 T-states in
43.1 s on an Apple M3 Max — **~308x real time** for a 3.5 MHz Z80, on a flat 64K bus with a
no-op `tick`.

> **Correction.** This entry went on to say *"within 7 % of `benches/step.rs`'s 329x, so the
> benchmark's figure holds"*, and it claims the wrong thing corroborated the wrong thing.
> The **329x is unresolved**: re-run repeatedly it gives **296–308x**, and a loaded machine
> can only make a throughput figure *smaller*, never larger — so there is no reading of the
> load under which 306 is evidence for 329. What the 308 above corroborates is ~306, on a
> completely independent workload of 5.8 × 10⁹ real instructions, which is a genuinely
> useful agreement and is not the one that was claimed. See `docs/ARCHITECTURE.md`'s
> *Measured* section, which owns the verdict table.

The number that matters for a frame loop is the other one in that pair: a `dev`-profile build
runs the same work at ~4.9 M instructions/s, **27x slower**. Anything scheduling `Cpu::step`
against a wall clock must be built in release.

## Unreleased — M2 · `crates/z80` — the four prefixes

### Breaking

- **Removed `StepError::UnsupportedPrefix`.** It became unreachable: all four prefixes are decoded
  and unassigned `ED` encodings are two-byte NOPs by hardware rule, so nothing in `step()` can
  fault. `#[non_exhaustive]` licenses *adding* variants, not removing them, so this is the crate's
  first API-breaking change — recorded here rather than left silent, even though `spectrum` is
  still a stub and the practical cost today is zero.

  Removing it exposed the same thing one level down: `execute`, `execute_cb`, `execute_ed` and
  `dispatch` all returned `Result<(), StepError>` with an **unconstructible `Err`**, and `step()`
  unwrapped a `None` that was always `None`. All four now return `()`.

- **`Cpu<B>` no longer carries the `Bus` bound on the struct definition.** The bound lives on the
  `impl` blocks, where it belongs; the field types never needed it. Downstream types that merely
  *name* `Cpu<Ula>` no longer have to repeat `where Ula: Bus`. Enforced by a compile-time guard
  rather than a comment — `DownstreamHolder<B>` does not compile if the bound returns.

### Fixed

- **A run of 63 or more `DD`/`FD` prefix bytes panicked inside `step()`.** `t_states` was a `u8`,
  and `overflow-checks = true` in every profile turned the overflow into a panic — with
  `panic = "abort"` in release, a hard process abort **from guest memory content**. Widened to
  `u32`, which `step()` already returned.

  The cause was a comment that was true when written: it argued the accumulator was safe *because*
  the longest instruction is 23 T-states, which `dispatch` falsified the moment a prefix run became
  one instruction. A pinned test now uses 300 prefixes and asserts 1204 T-states, `PC = 301` and
  `R = 45`, so it proves each prefix is a real M1 fetch rather than merely that nothing crashes.

### Added

- All four prefixes: `CB`, `DD`, `FD`, `ED`, including the `DDCB`/`FDCB` forms with the
  displacement byte *before* the opcode, and the seven undocumented encodings that copy the result
  to a register as well as to `(IX+d)`.
- The `ED` block instructions, repeating and non-repeating. Their repeat is `PC -= 2` with **one
  `step()` per iteration**, so a 64 KB `LDIR` stays interruptible — which is what lets it coexist
  with a 50 Hz frame interrupt at M5.
- `MEMPTR`/`WZ` is now **live rather than inert**: every indexed access sets it and `BIT` reads it
  back. It is the mechanism behind the `BIT n,(IX+d)` flag rule.

### Note for machine authors

**A single `step()` is not bounded.** A prefix run is one instruction and guest memory decides its
length, so a frame loop must treat one step as able to overrun its remaining budget. There is no
small maximum.

## Unreleased — M1 · `crates/z80` — the un-prefixed opcodes

### Added

- `Bus`, `Cpu<B>`, `CpuState`, `InterruptMode`, `StepError` and the un-prefixed instruction set.
- `Bus::tick(&mut self, addr: u16)` — one call per T-state, never batched, with the address the Z80
  actually drives. Contention is a function of `t mod 8`, so N separate cycles are not one N-cycle
  block; and a machine can track its own transfers but can never learn `IR`, which is what sits on
  the bus during the internal cycles of `ADD HL,ss`, `JR`, `DJNZ`, `CALL` and `PUSH`.
- `Cpu::interrupt` and `Cpu::nmi`. Without them there was no route out of `HALT` at all.
