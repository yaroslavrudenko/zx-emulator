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
| M6 | Snapshots (`.z80` / `.sna`) and tape (`.tap`) | a real game runs | design landed in [`docs/M6.md`](docs/M6.md); implementation under way, unmerged |
| M7 | 128: paging, second ROM, AY-3-8912, per-bank contention | 128-only software runs | — |
| M8 | WASM build | playable from a URL | — |

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

---

## Layout

```
crates/z80/         Z80 CPU core. No memory, no I/O, no allocation.
crates/spectrum/    The machine: paged memory, ULA, contention, keyboard, screen, timing.
                    Snapshots and tape are landing at M6; there is no AY module yet — it
                    is M7 scope. This line lists the crate's remit, not its contents.
crates/frontend/    macroquad frontend — native and WebAssembly.
crates/testsupport/ The corpus-absence policy every gate shares. Test-only, never published.
docs/               Architecture, the machine design, the M6 design, the Z80 reference,
                    the status register.
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
| `testdata/roms/48.rom` | the Sinclair 48K ROM | **yes** — Amstrad permits redistribution, and a subtly wrong ROM is the one corpus failure no harness here would explain. SHA-1 and fetch commands in `testdata/README.md` |
| `testdata/roms/` — 128 ROMs | Sinclair 128 editor + 48 BASIC | absent — needed at M7. Note they will be committed *by default* when added: the un-ignore rule is `!testdata/roms/*.rom`, not one path |

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
| [`docs/MACHINE.md`](docs/MACHINE.md) | The machine from M5 onward: why it owns the clock, the ULA as the second clock, and the verification plan for a layer that has **no oracle** |
| [`docs/M6.md`](docs/M6.md) | The M6 design — `.z80`/`.sna` snapshots and `.tap` tape. Each decision carries its class of evidence, plus **ruling** for a choice no evidence forces. *This table and the `docs/` line in Layout both omitted it while the file existed and ran to hundreds of lines; nothing in the repository linked to it at all.* |
| [`docs/Z80-REFERENCE.md`](docs/Z80-REFERENCE.md) | Register file, flag semantics, the undocumented 3/5 bits and register Q, `DAA`, instruction timing, prefix traps, interrupts |
| [`docs/STATUS.md`](docs/STATUS.md) | **What is currently true**, and the only register of what is open. Also the catalogue of this project's own failures — read it before trusting a number anywhere else |

Claims in these documents are labelled by the kind of evidence behind them — proven, measured,
derived, or observed — and a claim whose evidence has expired is corrected in place, loudly, rather
than deleted. If you find one that is stale, that is a defect report, not a tidy-up.

---

## Licensing

The emulator is MIT OR Apache-2.0.

Amstrad has explicitly permitted redistribution of the Sinclair ROMs alongside emulators,
so the Spectrum ROMs may be placed in `testdata/roms/` locally. Game images may not be
redistributed — they are yours to supply.
