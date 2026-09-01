# zx-emulator

A **ZX Spectrum 48K and 128** emulator written from scratch in Rust.

Not a port. Not a translation of an existing emulator. The CPU is implemented from the Z80
hardware specification and from our own architecture; correctness is then proven against
public conformance suites rather than against somebody else's source.

**The bar:** the emulator is not "done" until `zexall` passes — including the undocumented
flag behaviour that most emulators quietly skip.

> **What that green proves is narrower than the sentence reads, and this repository measured the
> gap rather than assuming it.** `zexall` reports 67/67 (M4), and it genuinely **is** sensitive to
> the undocumented `F3`/`F5` bits — forcing them to a constant `0` or `0x28` fails it, while a
> control mutation of a *documented* bit fails both exercisers, which is what proves the group is
> executed rather than skipped. But `zexall` **cannot decide the rule behind those bits**: the same
> 67/67 has been observed under three different implementations, including one whose flag latch was
> stuck at zero. Two FUSE vectors are the only gate in this project that can see that latch at all.
>
> So the bar is necessary and not sufficient, and the tempting one-line summary — *"`zexall`
> passes, so the undocumented flags are right"* — is true of the first claim and false of the
> second. [`docs/STATUS.md`](docs/STATUS.md) holds the coverage table that keeps them apart.

---

## Why this project

Emulating a Z80 is a rare kind of engineering problem: the correct answer is not a matter
of opinion. `zexall` either prints `OK` for all 67 tests or it does not. That makes it an
honest way to exercise the parts of Rust that matter — exhaustive `match` over an
instruction set, types that make an invalid state unrepresentable, and property tests over
arithmetic — with an external oracle instead of self-assessment.

---

## Status

| Milestone | Goal | Gate | State |
|---|---|---|---|
| M1 | Registers, flags, un-prefixed opcodes | FUSE vectors green for un-prefixed | **290/290** — merged |
| M2 | `CB` / `ED` / `DD` / `FD` prefixes | FUSE vectors green in full | **1045/1045** — merged |
| M3 | Documented behaviour | `zexdoc` passes | **67/67 first run** — merged |
| M4 | Undocumented flags | **`zexall` passes** — CPU complete | **67/67**, made a gate with its limits stated — merged |
| **M5** | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | boots to `© 1982 Sinclair Research Ltd` | the machine boots, **on frame 87**, and **the gate work landed**: `crates/spectrum/tests/boot.rs` is a real `#[test]` that runs the ROM under `cargo test` and asserts the frame, alongside the other integration gates in that directory. Of the five mutations, **four were already red and one survived**. See [`docs/STATUS.md`](docs/STATUS.md) for what M5's green does and does not mean |
| **M6** | Snapshots (`.z80` / `.sna`) and tape (`.tap`) | **T1 + T2 + T3** — *not* "a real game runs" | **merged.** A program written here, stored as a `.tap`, loaded by the real ROM's own `LD-BYTES` through the `EAR` bit, and executed — computing a value asserted to appear **nowhere in its own bytes**, so that *"the data arrived"* and *"it ran"* are separate claims. Design in [`docs/M6.md`](docs/M6.md); what it opened and closed is in [`docs/STATUS.md`](docs/STATUS.md) |
| M7 | 128: paging, second ROM, AY-3-8912, **the beeper**, per-bank contention | **T1 + T2 + T3** — *not* "128-only software runs" | design in [`docs/M7.md`](docs/M7.md), in flight. **The memory half boots:** the 128 reaches its own `© 1986` copyright, draws all five menu entries, the highlight moves under `CAPS SHIFT`+`6`, and selecting *48 BASIC* reaches the `© 1982` message through ROM page 1 — the year changing is what makes that a claim about which ROM is executing |
| M8 | WASM build | playable from a URL — the browser, **not** the sound | design in [`docs/M8.md`](docs/M8.md) |

> **Two corrections to the rows above, both made 2026-09-01 rather than left to be inferred from
> the design documents.**
>
> **The beeper joined M7 and left M8.** `docs/M6.md` had assigned *Sound — the speaker bit of a
> `0xFE` write* to M8, and `docs/M7.md` said in three places that M8 owned the audio device and
> the beeper with it. **The AY is a 128 device and the beeper is a ULA feature, so both are the
> machine's** — decoded by the same `Ula::out_port` that already takes the border out of the same
> byte — and `crates/spectrum` is where the machine is modelled. **What M8 owns is routing audio
> the machine already produces to a browser's audio device**: the mix, the resampling, the device.
> Four rows were corrected with the originals struck; `docs/M8.md` Decision 9 carries the ruling.
>
> **M8's gate is a build gate and the row's *"playable from a URL"* cannot be one.** *Playable* is
> not a property of an artefact — it is a property of a browser rendering a canvas, a GPU
> compositing it, a keyboard delivering keys and a person forming an opinion — so no corpus and no
> licence makes it automatable. This is the **third** milestone row corrected this way, and the
> reason differs from M6's and M7's: theirs were unautomatable because a corpus could not be
> committed, which another repository could fix, and M8's is unautomatable structurally.

> **Correction — the M5 row said *"the gate work is unfinished: five mutations leave it green and
> nothing yet runs it"*, and the repository falsifies both halves.** It is corrected loudly rather
> than quietly bumped, because how it survived is worth more than the row.
>
> *"Nothing yet runs it"* was already false when it was written. `crates/spectrum/tests/boot.rs` is
> a committed `#[test]`, landed with the M5 gates; `docs/STATUS.md` closes the item in as many
> words — *"nothing runs the boot gate — `crates/spectrum/tests/boot.rs` runs it."*
>
> *"Five mutations leave it green"* was never a verdict about a run that happened. Those mutations
> were graded against `crates/spectrum/examples/boot.rs`, which `cargo test` **builds and never
> calls**. Re-measured against the pre-gate lib target, **four of the five were already red** — 5,
> 7, 1 and 13 failing unit tests inside `src` — and **one survived**, the contention-phase
> off-by-one.
>
> **The propagation is the finding.** The pass that fixed the figure gives a whole section to *"a
> derived figure repeated across documents acquires authority it never earned"*, and states that
> the wrong number had spread into three documents. `docs/STATUS.md` and `docs/MACHINE.md` each
> carry a correction of it. This file did not — the repository's front door, and the only document
> a newcomer reads first. **The section diagnosing the propagation missed a copy of the thing
> propagating, and the copy it missed was the most-read one.**
>
> The rule that follows is cheap and absolute: *a correction is not landed until you have grepped
> for every other copy of what you corrected.* Every fact in these documents costs seconds to
> sweep across `docs/`, `README.md`, `CHANGELOG.md` and `testdata/`. **That sweep was run for this
> correction and did not come back clean**: `docs/MACHINE.md:358` still asserts *"five mutations
> left it green"* as fact, inside a correction block written after the figure was known to be
> wrong. It is left for that file's owner rather than edited from here, and it is named so it
> cannot go quiet again.
>
> **The row above deliberately carries no gate count**, and that is not vagueness — it is the same
> lesson applied one step earlier. `ls -1 crates/spectrum/tests/*.rs | wc -l` returned **13** on
> 2026-09-01 while this note was being written, and **14** a few minutes later, before it was
> finished; 6 of them are committed and the rest landed that same day. A bare integer here would
> have been stale before the paragraph containing it was, which is this exact defect for the third
> time. **Count the directory — the command is in this sentence.** `docs/MACHINE.md`'s milestone
> table says *"seven gates"*, which was true when written and is the number to check first.
>
> > **`MACHINE.md`'s M5 row no longer says seven.** It now carries the command
> > (`ls -1 crates/spectrum/tests/*.rs | wc -l`) instead of an answer, and records that the integer
> > moved 13 → 14 → 16 → 19 on 2026-09-01 while the correction was being written. The sentence
> > above is left standing because it is what a reader should still do; only its example moved.

> **Correction — the M6 and M7 rows said *"a real game runs"* and *"128-only software runs"*, and
> both name tier T4.** M6 has since merged; the row is corrected rather than quietly widened.
>
> [`docs/M6.md`](docs/M6.md) Decision 8 splits the milestone's evidence into four tiers: **T1**
> proven and corpus-free (the round trips, the truncation sweep, the codec property tests, the
> hand-transcribed vectors), **T2** measured (the real ROM's `LD-BYTES` loading a synthetic tape
> through the `EAR` bit), **T3** measured (a program *we wrote*, loaded from tape by the ROM and
> executed), and **T4** *observed* — a real game, a file of ours opening in somebody else's
> emulator. **The gate is T1 + T2 + T3.** T4 cannot be automated in a repository that may not carry
> games, and a milestone gated on it would be a gate that runs nowhere — which
> [`docs/STATUS.md`](docs/STATUS.md) records this project shipping three times already.
>
> **The residue is not absorbed:** T4 is the only tier that grades a turbo loader or a program
> written by somebody who did not know how this emulator works, and it runs nowhere. That is a row
> in the register, not a footnote here.
>
> `docs/MACHINE.md` and `docs/ARCHITECTURE.md` carry the same table; both are corrected. This file
> was the copy that was missed the last time a milestone row was corrected, which is why it is
> checked first now.

---

## Layout

```
crates/z80/         Z80 CPU core. No memory, no I/O, no allocation.
crates/spectrum/    The machine: paged memory, ULA, contention, keyboard, screen, timing,
                    snapshots (`.z80`/`.sna`) and tape (`.tap`). No AY module yet — it is
                    M7 scope. This line lists the crate's remit, not its contents.
crates/frontend/    macroquad frontend — native and WebAssembly.
crates/testsupport/ The corpus-absence policy every gate shares. Test-only, never published.
docs/               Architecture, the machine design, the M6 and M7 designs, the Z80
                    reference, the status register.
testdata/           Conformance suites, fetched locally. The Sinclair 48K ROM is the one
                    exception and is committed.
```

The CPU core does not own memory. That single decision shapes everything else: the machine
supplies a `Bus`, and the CPU reports every access as it happens — which is what makes
cycle-accurate contention possible. It landed at M5.

> **Reporting every access proved necessary and *not* sufficient, and M5 measured that.** The
> reasoning on record was that contention depends only on the address and the phase within the
> frame, both of which the machine already has. It does not follow: `LD A,B` and the read-modify
> half of `INC (HL)` emit **byte-identical** call streams — `read(addr)` then four `tick(addr)` —
> while owing one contention point and two. `crates/spectrum` first reconstructed the machine-cycle
> boundaries by deferral, in a 312-line file, at a residual it had to pin rather than fix. A
> defaulted `Bus::fetch` was added so each cycle discloses itself as it opens, and that file is
> deleted. The account is in [`docs/STATUS.md`](docs/STATUS.md) and
> [`docs/MACHINE.md`](docs/MACHINE.md).

---

## Build and run

```bash
cargo build                          # workspace
cargo test                           # unit tests, FUSE vectors, the machine's integration gates
cargo test --release -p z80 --test zex_oracle -- --ignored    # zexdoc + zexall, ~90 s
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

**`cargo test` does not run the `zex` exercisers**, and that line used to say it did. At 5.8
billion instructions each they are `#[ignore]`d and release-only — ~43 s each in release, ~20
minutes each in the `dev` profile `cargo test` uses by default — which is why they have their own
line above. An ignored gate that no pipeline executes is this project's most-repeated defect; see
[`docs/STATUS.md`](docs/STATUS.md).

**A missing corpus fails its gate rather than skipping it silently**, naming the fetch
instructions. So a fresh clone fails `cargo test` until `testdata/fuse` is fetched — that is
deliberate, and it replaced an earlier arrangement in which the absent corpus left the suite green
with the same test count. `ZX_CORPUS_ALLOW_MISSING=1` is the considered opt-out, and it is
**refused** when `CI` is also set.

For a `--no-fail-fast` caveat that matters when anything is red, see `docs/STATUS.md`: `cargo test`
stops at the first failing target, so a reddened unit test prevents every integration gate from
running at all — and "the gates did not run" looks identical to "the gates passed".

The toolchain is pinned by `rust-toolchain.toml`: stable Rust with the
`wasm32-unknown-unknown` target already declared, so the browser build needs no extra setup.

Requires Rust **1.98+** (edition 2024) — `rust-version` and `edition` in the workspace `Cargo.toml`.

---

## Test data

`testdata/` is gitignored with explicit un-ignore rules, not wholesale. `git ls-files testdata/`
returns exactly `.gitkeep`, `README.md` and `roms/48.rom`; the ROM is the only *corpus* exception.
Fetch the rest locally:

| Directory | Contents | Committed? |
|---|---|---|
| `testdata/fuse/` | `tests.in`, `tests.expected` — 1335 per-instruction vectors | no — belongs to the FUSE project |
| `testdata/zex/` | `zexdoc.com`, `zexall.com` | no — third-party conformance binaries |
| `testdata/roms/48.rom` | the Sinclair 48K ROM | **yes** — under the permission quoted in `testdata/README.md`, which is also where the SHA-1, the CRC-32 and the fetch commands are. A subtly wrong ROM is the one corpus failure no harness here would explain |
| `testdata/snapshots/`, `testdata/tapes/`, `testdata/timing/` | third-party `.z80`/`.sna`, `.tap` and the 48K timing suite | no — fetched; each is the *external* check on our reading of a format, or of the machine's timing |
| `testdata/roms/` — 128 ROMs | Sinclair 128 editor + 48 BASIC | absent — needed at M7, and adding one is now a **deliberate** act |

> **The 128-ROM row said they *"will be committed by default when added: the un-ignore rule is
> `!testdata/roms/*.rom`, not one path"*, and `.gitignore` no longer works that way.** The glob was
> replaced by one explicit filename per ROM, so a new ROM is ignored until somebody edits
> `.gitignore` — which is where a reviewer sees it and where the licence question has to be answered
> rather than assumed. The reason it mattered is not that some *Sinclair* ROM has a different
> licence: it is that `*.rom` also accepts a game cartridge, a Multiface image, or an Interface 1
> ROM, and the permission this repository relies on **disclaims** the Interface 1 and 2 ROMs
> outright. The glob turned "may we redistribute this?" from a decision into an accident.

Game images are never distributed here. Supply your own.

---

## Engineering rules

These are enforced, not aspirational:

- **Our own implementation.** No code, no opcode tables and no timing tables copied or
  adapted from another emulator. Behavioural questions are resolved from hardware
  documentation, and the rule is named in a comment at the site.
- **`unsafe_code = "forbid"`** in every crate. This machine runs a 3.5 MHz CPU thousands of
  times faster than real time; `unsafe` would buy nothing.
- **`overflow-checks = true` in release too.** The Z80 is *meant* to wrap, so every wrap is
  written as an explicit `wrapping_*` call. A silent wrap is a bug, and debug must behave
  the same as release.
- **Latest stable dependencies only**, verified against crates.io. No `*` constraints.
- **No trait objects on the execute path.** `Cpu<B>` is generic and monomorphised. *(This line
  said `Cpu<B: Bus>`. The declaration is `pub struct Cpu<B>` — the `Bus` bound sits on the `impl`
  blocks, not on the struct, and its removal is recorded as closed in `docs/STATUS.md`: a
  struct-level bound forces `where B: Bus` onto every downstream type that merely **names** a
  `Cpu`. The substantive claim is unchanged and is checked in the emitted code rather than
  asserted — the commands are in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)'s *Measured*
  section.)*

---

## Documentation

| Document | Contents |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate boundaries, the five load-bearing design decisions, the 48K↔128 differences, milestones, and the *Measured* section — every performance and codegen claim with the command that produced it |
| [`docs/MACHINE.md`](docs/MACHINE.md) | The machine from M5 onward: why it owns the clock, the ULA as the second clock, the verification plan, and **the timing oracle** — 68 rows measured on real Spectrums, and the precise statement of what its green does and does not settle |
| [`docs/M6.md`](docs/M6.md) | The M6 design — `.z80`/`.sna` snapshots and `.tap` tape. Each decision carries its class of evidence, plus **ruling** for a choice no evidence forces. *This table and the `docs/` line in Layout both omitted it while the file existed and ran to hundreds of lines; nothing in the repository linked to it at all.* |
| [`docs/M7.md`](docs/M7.md) | The M7 design — the 128: paging, the second ROM, the AY-3-8912, per-bank contention. **In flight and unmerged**; its gate is T1+T2+T3 on the same four-tier scheme M6 uses |
| [`docs/Z80-REFERENCE.md`](docs/Z80-REFERENCE.md) | Register file, flag semantics, the undocumented 3/5 bits and register Q, `DAA`, instruction timing, prefix traps, interrupts |
| [`docs/STATUS.md`](docs/STATUS.md) | **What is currently true**, and the only register of what is open. Also the catalogue of this project's own failures — read it before trusting a number anywhere else |

> **Correction — the `MACHINE.md` row described it as *"the verification plan for a layer that has
> **no oracle**"*, and that stopped being true on 2026-09-01.** It was accurate as history and
> misleading as a description: `crates/spectrum/tests/timing_oracle.rs` grades the 48K contention
> model against 68 rows measured on real Spectrums, with 0 disagreements, bounded by thirteen
> mutations — `FIRST_CONTENDED_T_STATE` at 14333, 14334, 14336 and 14337 all red, only 14335 green.
>
> **State the scope, because the obvious reading is too strong.** What is established is that **the
> first contended T-state falls exactly 14335 T-states after `/INT`**, given that this machine
> asserts `/INT` at frame T-state 0. Three things stay open and each is its own row in
> `docs/STATUS.md`'s register: the frame's **origin** is a convention (moving `/INT` and the window
> together leaves the oracle green), the interrupt window's **length** is unmeasured (32 → 24 leaves
> it green), and the `64 × 224` **factorisation** is not measured — its *product* is.
>
> The floating bus, progressive drawing and keyboard ghosting remain unmodelled, so they are not
> gradeable rather than ungraded, and the *observation* half of that plan is unchanged for them.
> **`docs/M6.md`'s opening and `crates/spectrum/src/lib.rs`'s coverage table carried the same
> superseded claim**; the first is corrected, and the second is another agent's file and is named
> here so it cannot go quiet.

Claims in these documents are labelled by the kind of evidence behind them — proven, measured,
derived, or observed — and a claim whose evidence has expired is corrected in place, loudly, rather
than deleted. If you find one that is stale, that is a defect report, not a tidy-up.

---

## Licensing

The emulator is MIT OR Apache-2.0.

**Amstrad have kindly given their permission for the redistribution of their copyrighted
material but retain that copyright.**

That sentence is not a paraphrase. The permission — Cliff Lawson of Amstrad plc, posted to
`comp.sys.sinclair` on 31 August 1999 — asks in those words that *"the program/manual includes a
note to the effect that"* it, and this is that note. `testdata/roms/48.rom` is committed on its
strength; the Spectrum ROMs may be placed in `testdata/roms/` locally. Game images may not be
redistributed — they are yours to supply.

**The quotation, the author, the date, the four conditions, the hedged scope, and the one gap
still open in the sourcing are in [`testdata/README.md`](testdata/README.md) and only there.** This
section used to assert *"Amstrad has explicitly permitted redistribution of the Sinclair ROMs
alongside emulators"* and stop — one of five copies of a licensing claim carrying no quotation, no
author, no date and no URL between them. A licensing claim is the one kind where the quotation
**is** what is being relied on, so it has one home and everything else points at it. Read alongside
it: the scope is narrower and more hedged than the old sentence implied — the ZX80, the ZX81 and
the Interface 1 and 2 ROMs are disclaimed as **not Amstrad's copyright at all**, which is why
`.gitignore` names every committed ROM individually.
