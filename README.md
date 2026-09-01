# zx-emulator

A **ZX Spectrum 48K and 128** emulator written from scratch in Rust.

Not a port. Not a translation of an existing emulator. The CPU is implemented from the Z80
hardware specification and from our own architecture; correctness is then proven against
public conformance suites rather than against somebody else's source.

**The bar:** the emulator is not "done" until `zexall` passes — including the undocumented
flag behaviour that most emulators quietly skip.

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
| **M5** | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | boots to `© 1982 Sinclair Research Ltd` | the machine boots, on frame 87. **The gate work is unfinished** — five mutations leave it green and nothing yet runs it; see [`docs/STATUS.md`](docs/STATUS.md) before treating M5 as done |
| M6 | Snapshots (`.z80` / `.sna`) and tape (`.tap`) | a real game runs | — |
| M7 | 128: paging, second ROM, AY-3-8912, per-bank contention | 128-only software runs | — |
| M8 | WASM build | playable from a URL | — |

---

## Layout

```
crates/z80/         Z80 CPU core. No memory, no I/O, no allocation.
crates/spectrum/    The machine: paged memory, ULA, contention, keyboard, tape, AY.
crates/frontend/    macroquad frontend — native and WebAssembly.
crates/testsupport/ The corpus-absence policy every gate shares. Test-only, never published.
docs/               Architecture, the machine design, the Z80 reference, the status register.
testdata/           Conformance suites, fetched locally. The Sinclair 48K ROM is the one
                    exception and is committed.
```

The CPU core does not own memory. That single decision shapes everything else: the machine
supplies a `Bus`, and the CPU reports every access as it happens, which is what makes
cycle-accurate contention possible later.

---

## Build and run

```bash
cargo build                          # workspace
cargo test                           # unit tests + conformance harness
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The toolchain is pinned by `rust-toolchain.toml`: stable Rust with the
`wasm32-unknown-unknown` target already declared, so the browser build needs no extra setup.

Requires Rust **1.98+** (edition 2024).

---

## Test data

`testdata/` is gitignored apart from one exception. Fetch the rest locally:

| Directory | Contents | Committed? |
|---|---|---|
| `testdata/fuse/` | `tests.in`, `tests.expected` — 1335 per-instruction vectors | no — belongs to the FUSE project |
| `testdata/zex/` | `zexdoc.com`, `zexall.com` | no — third-party conformance binaries |
| `testdata/roms/48.rom` | the Sinclair 48K ROM | **yes** — Amstrad permits redistribution, and a subtly wrong ROM is the one corpus failure no harness here would explain. SHA-1 and fetch commands in `testdata/README.md` |
| `testdata/roms/` — 128 ROMs | Sinclair 128 editor + 48 BASIC | not yet; needed at M7 |

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
- **No trait objects on the execute path.** `Cpu<B: Bus>` is generic and monomorphised.

---

## Documentation

| Document | Contents |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate boundaries, the five load-bearing design decisions, the 48K↔128 differences, milestones, and the *Measured* section — every performance and codegen claim with the command that produced it |
| [`docs/MACHINE.md`](docs/MACHINE.md) | The machine from M5 onward: why it owns the clock, the ULA as the second clock, and the verification plan for a layer that has **no oracle** |
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
