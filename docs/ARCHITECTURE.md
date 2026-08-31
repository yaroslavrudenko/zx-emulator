# ZX Spectrum emulator — architecture

Target: a correct ZX Spectrum **48K and 128** emulator, CPU-first, correctness-gated.

## Project rules

1. **Latest stable libraries only.** Every dependency is pinned to the newest stable
   release, verified against crates.io. No legacy versions, no `*`.
2. **`unsafe_code = "forbid"`** in every crate. If `unsafe` looks necessary for speed,
   it is premature — this machine runs thousands of times faster than a 3.5 MHz Z80.
3. **`overflow-checks = true` even in release.** The Z80 is meant to wrap; every wrap
   is written as an explicit `wrapping_*` call, so debug and release must agree.
   A silent wrap is a bug, not a feature.
4. Correctness has an external oracle up to M4. After that it is observation. Do not
   mix the two modes.

## Crate boundaries

```
crates/z80/         pure CPU core. No memory, no I/O, no allocation, no std needed.
crates/spectrum/    the machine: paged memory, ULA, contention, keyboard, tape, AY.
crates/frontend/    macroquad, native + WASM.
```

The CPU does not own memory. That is the decision everything else hangs on.

## Decision 1 — the bus is a consumer-defined trait, ticked once per T-state, with the address

```rust
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn in_port(&mut self, port: u16) -> u8;
    fn out_port(&mut self, port: u16, val: u8);

    /// One T-state elapses with `addr` on the bus. Called once per T-state, never batched.
    fn tick(&mut self, addr: u16);
}

pub struct Cpu<B: Bus> { /* ... */ }
```

Two properties, and the first revision of this document got both wrong. They were corrected
after a review checked the signature against the corpus rather than against the prose.

**One call per T-state, never a batch.** Spectrum contention is a function of `t mod 8`, so
N separate one-T-state contentions starting at T do **not** sum to one N-T-state block
starting at T. The machine cannot recover the difference afterwards. Corpus vector `09`
(`ADD HL,BC`) shows seven separate `MC` events, and a batched `tick(7)` loses six of them.
Across the un-prefixed set, batching loses **88 of the corpus's 166 internal contention
points** — more than half.

**The address travels with the tick.** The machine can track the address of the last
transfer itself, since it sees every `read`/`write`. What it cannot learn is `IR` — the
refresh address the Z80 drives during the second half of M1, which is what sits on the bus
for the internal cycles of `ADD HL,ss`, `INC ss`, `JR`, `DJNZ`, `CALL`, `PUSH` and `RET cc`.
`I` and `R` are reachable only through `Cpu::state()`, which the bus cannot call because the
CPU owns it. On a 48K, program code normally lives in contended RAM `0x4000–0x7FFF`, so `IR`
points into contended memory for most of a game's runtime — this is the difference between
multicolour demos working and tearing.

Generic over `B`, never `Box<dyn Bus>` — monomorphised, no indirect call on the hot path.
`read`/`write` carry `#[inline]`; cross-crate inlining does not happen otherwise. Both
properties are verified in emitted assembly, not assumed — see *Measured*, below.

### Timestamp convention

The corpus distinguishes two event kinds, and they are asserted differently:

- **`MC` / `PC` timestamps are plain T-state indices, no offset.** `MC` at T=4 means that
  address was on the bus during T-state 4, and `tick` call #4 *is* T-state 4. One-to-one.
- **`MR` / `MW` timestamps are cycle-ends**, so they are asserted on (kind, address, byte)
  **in order, without timestamps**. Matching their stamps would assert whether the core calls
  `read()` before or after that cycle's ticks — an internal ordering choice, not a hardware
  fact. Timing is covered by the contention stream, which is strictly stronger.

## Decision 2 — registers as an array, not fields

```rust
regs: [u8; 26],   // A F B C D E H L | A' F' B' C' D' E' H' L' | IXh IXl IYh IYl | SPh SPl | I R | ...
```

This is the key to the DD/FD prefixes. They substitute IX or IY for HL in the *next*
instruction. With named fields that is a branch in every HL-touching handler; with an
array it is a **constant index offset** — one `hl_base: usize`, and the entire HL
instruction set operates on IX with no `if` at all.

> **How this landed, and why the note stays.** The *layout* arrived first and the *indirection*
> did not: two independent reviewers found `grep hl_base` returning zero hits while this
> document described it as the decision "everything hangs on". For a while the crate therefore
> paid the layout's cost — pairs are stored high-byte-first, so `pair()` compiles to `ldrh` +
> `rev` + `lsr` where a native `u16` field would be one `ldrh` — and collected none of the
> benefit. `base` is now threaded through the decoder, and `add_pair(base, base)` makes
> `DD 29` into `ADD IX,IX` with no new code.
>
> Two things to know when extending it. The substitution is **asymmetric**: `H` and `L`
> shift, but `B`/`C`/`D`/`E`/`A` must not — `DD 44` is `LD B,IXh`, not `LD IXh,IXh`. And
> `0x29` (`ADD IX,IX`) needs the base substituted in **two** positions. Separately, the
> operand-field→array-index mapping is already an 11-instruction branch cascade before any
> offset is added, because the two orderings differ by a permutation that LLVM cannot fold;
> `hl_base` will sit on top of that, not replace it.

## Decision 3 — decode with `match`, not a function-pointer table

A large `match` on the opcode byte, with sub-matches per CB/ED/DD/FD prefix. The compiler
builds a jump table anyway, and exhaustiveness is checked at compile time. A pointer table
costs an indirect call per instruction and defeats inlining.

## Decision 4 — flags as per-class helpers

`add8`, `sub8`, `adc16`, `sbc16`, `rotate`, `bit`, `daa` — one implementation each,
called from every opcode of that class. Never inline flag logic per opcode: that breeds
200 copies of the same rule, each with its own bug.

## Known Z80 traps — these are the ones that actually break emulators

| Trap | Detail |
|---|---|
| Flags 3 and 5 | Undocumented F bits copy from the result — but from the address calculation in `BIT n,(HL)`, and from the previous instruction (register Q) in `SCF`/`CCF`. This is exactly what zexall tests. |
| EI delay | An interrupt is not accepted until after the instruction *following* `EI`. Many games depend on it. |
| Register R | Increments on every M1 (opcode fetch), 7-bit, bit 7 preserved. Used by games for randomness and copy protection. |
| DDCB / FDCB | The displacement byte comes **before** the opcode byte, not after. A naive decoder breaks here. |
| DD/FD chains | `DD DD DD 21` — each prefix is a separate instruction with its own R increment. |
| `RETN` / NMI | IFF1 is restored from IFF2. |
| HALT | Executes NOPs until an interrupt; PC must not advance past it. |

## Decision 5 — paged memory from day one, 48K as a configuration

Writing `mem: [u8; 65536]` at M5 means rewriting it at M7 along with everything attached.
Model slots → banks from the start; then **48K is a special case of the 128**: paging
locked, banks fixed. Cost: about thirty lines.

```rust
struct Memory {
    banks: [[u8; 0x4000]; 8],
    roms: Vec<[u8; 0x4000]>,   // 48K: one. 128: two (editor + 48 BASIC).
    slots: [Slot; 4],           // Rom(i) | Bank(i)
    contended: [bool; 8],       // 48K: bank 5 only. 128: banks 1, 3, 5, 7.
    paging_locked: bool,        // port 0x7FFD bit 5, until reset
}
```

### 48K vs 128 — what differs

| | 48K | 128 |
|---|---|---|
| Memory | flat 64K | 8 banks × 16K, slot `0xC000` pageable |
| ROM | one, 16K | two: 128 editor + 48 BASIC |
| Paging port | — | `0x7FFD`: bits 0–2 bank, bit 3 screen, bit 4 ROM, **bit 5 lock until reset** |
| Screen | bank 5 | bank 5 or shadow bank 7 |
| Sound | beeper | **AY-3-8912**, ports `0xFFFD` select / `0xBFFD` data |
| Frame | 69888 T | **70908 T** |
| Contention | address range `0x4000–0x7FFF` | **property of the bank** (1, 3, 5, 7) in *any* slot |

That last row is the real complication: on the 128 contention must be checked per access
against whichever bank is currently paged in, not by address range.

**Out of scope:** +2A/+3. They add port `0x1FFD`, all-RAM configurations and yet another
contention pattern, for a sliver of extra software. 48K + 128 covers what matters.

## Testing — three tiers, strictly in this order

1. **fuse suite** (`tests.in` / `tests.expected`) — ~1400 micro-tests: initial registers
   and memory, expected final state, **and expected T-states**. Start here, not with
   zexall. Fast feedback per opcode, and cycle accuracy is tested from day one rather
   than bolted on later.
2. **zexdoc, then zexall** — real CP/M binaries that self-check via CRC. zexdoc covers
   documented behaviour, zexall adds the undocumented flags. A hard pass/fail oracle.
3. **proptest** — ALU flag invariants against an independent reference implementation.

Machine-level timing has no such oracle. Contention and floating bus are verified against
known-demanding software (Nirvana-engine demos, Aquaplane for floating bus,
multicolour effects). That is observation, not a green check.

## Milestones

| | Goal | Gate |
|---|---|---|
| M1 | Registers, flags, un-prefixed opcodes | fuse green for un-prefixed |
| M2 | CB/ED/DD/FD prefixes | fuse green in full |
| M3 | Documented behaviour | **zexdoc passes** |
| M4 | Undocumented flags | **zexall passes** — CPU is done |
| M5 | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | boots to `© 1982 Sinclair Research Ltd` |
| M6 | Snapshots (Z80/SNA) or TAP tape | a real game runs |
| M7 | 128: paging, second ROM, AY, contention per bank | 128-only software runs |
| M8 | WASM + macroquad | playable from a URL |

Performance is a non-goal. 3.5 MHz × 50 Hz ≈ 70,000 T-states per frame; a modern machine
does that thousands of times faster than real time. Optimise nothing until measured.

## Measured

Everything in this section is a measurement or an assembly inspection, not an estimate.
Host: Apple M3 Max, rustc 1.98.0, `[profile.release]` as shipped. Re-run after M2 — it
quadruples the opcode count on exactly the paths measured here.

| | Batched tick (before C1) | Per-T-state tick (shipped) |
|---|---|---|
| Throughput, flat 64K bus | 507× real-time | **329× real-time** |
| Throughput, M7-shaped paged + contended bus | 294× real-time | **145× real-time** — 138 µs of a 20,000 µs frame, **0.7 % of the budget** |

**The drop is C1's price, and it is the right trade.** Ticking once per T-state instead of once
per batch is roughly 3× more calls, each doing contention arithmetic. It buys the 88 contention
points that batching discarded — without which M5 and M7 cannot be correct at all. 145× still
leaves two orders of magnitude of headroom over the 1× requirement, which is precisely why the
benchmark is gated: so the next person can see what a change costs instead of guessing.

| Other measurements | |
|---|---|
| Cost of `overflow-checks = true` | **5 %** on the core. One `cmp`/`b.hi` guarding the T-state accumulator — the only panic path in the whole `step` function |
| Unproven bank index at M7 | **6.6 %**, avoidable for free by masking or a newtype |

Design claims, each verified in the emitted assembly rather than assumed:

| Claim | Evidence |
|---|---|
| Monomorphised, no `dyn` on the execute path | 0 hits for `dyn`/`Box`/`Rc`/`Arc`; indirect-branch (`blr`) count **0** |
| `#[inline]` makes cross-crate inlining happen | `Bus::read` compiles to **one instruction** inside `step`; no call to any bus method |
| Decode lowers to a jump table, not a compare chain | Two real tables (119 + 64 entries); the `LD r,r'` and `ALU A,r` blocks need none — `ubfx` extracts the field directly |
| The execute path allocates nothing | 0 allocation sites outside `#[cfg(test)]`; Rust has no escape analysis, so this is certain, not probabilistic |
| Register indexing is in-range | `panic_bounds_check` count in `step`: **0**. The `// INVARIANT:` comments are facts the compiler proves |

The `spectrum` crate's contention arithmetic is the one cost worth watching — it is the
largest term (−21.5 points of the M7 decomposition) and it is irreducible, because
per-access timing is the property M7 exists to deliver.

## Open items

**The register lives in [`STATUS.md`](STATUS.md), and only there.** This document describes the
design; `STATUS.md` records what is currently true. They were briefly duplicated, and within one
session they disagreed about four facts — the exact defect class that let the `tick` contract
survive unchallenged. One register, one owner.

## Licensing note

Amstrad has explicitly permitted redistribution of the Sinclair ROMs with emulators, so
the Spectrum ROMs may live in this repository. Game images may not — the user supplies
their own.
