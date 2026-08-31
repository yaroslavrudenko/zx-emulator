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
| **M3** | Documented behaviour | `zexdoc` passes | in progress — the first oracle that grades the processor rather than an instruction |
| M4 | Undocumented flags | **`zexall` passes** — CPU complete | — |
| M5 | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | boots to `© 1982 Sinclair Research Ltd` | — |
| M6 | Snapshots (`.z80` / `.sna`) and tape (`.tap`) | a real game runs | — |
| M7 | 128: paging, second ROM, AY-3-8912, per-bank contention | 128-only software runs | — |
| M8 | WASM build | playable from a URL | — |

---

## Layout

```
crates/z80/         Z80 CPU core. No memory, no I/O, no allocation.
crates/spectrum/    The machine: paged memory, ULA, contention, keyboard, tape, AY.
crates/frontend/    macroquad frontend — native and WebAssembly.
docs/               Architecture and the Z80 behavioural reference.
testdata/           Conformance suites and ROMs. Not committed; fetched locally.
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

`testdata/` is intentionally empty in git. Fetch it locally:

| Directory | Contents | Why it is not committed |
|---|---|---|
| `testdata/fuse/` | `tests.in`, `tests.expected` — ~1400 per-instruction vectors | belongs to the FUSE project |
| `testdata/zex/` | `zexdoc.com`, `zexall.com` | third-party conformance binaries |
| `testdata/roms/` | Sinclair 48K and 128 ROMs | see licensing below |

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
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate boundaries, the five load-bearing design decisions, the 48K↔128 differences, milestones |
| [`docs/Z80-REFERENCE.md`](docs/Z80-REFERENCE.md) | Register file, flag semantics, the undocumented 3/5 bits and register Q, `DAA`, instruction timing, prefix traps, interrupts |

---

## Licensing

The emulator is MIT OR Apache-2.0.

Amstrad has explicitly permitted redistribution of the Sinclair ROMs alongside emulators,
so the Spectrum ROMs may be placed in `testdata/roms/` locally. Game images may not be
redistributed — they are yours to supply.
