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

1. **Boots to `© 1982 Sinclair Research Ltd`.** Binary, cheap, and it exercises the memory map, the
   interrupt, the keyboard scan and the screen in one go. If this fails, nothing else matters.
2. **A known-timing test program** — the Spectrum community's contention test suites report
   measured T-state counts for the machine to print. That is the closest thing to an oracle
   available, and it is a real one: a number to compare, not a picture to squint at.
3. **A snapshot round trip.** Load a `.z80`, run zero frames, save, compare. Byte-identical or the
   state model is wrong. Cheap, deterministic, and independent of timing.
4. **Frame-hash regression.** Render frame N of a fixed program and hash it. Does not prove
   correctness — proves *change*, which is what catches a regression once something works.
5. **Known-demanding software.** Multicolour demos, Nirvana-engine programs, a tape loader. This is
   observation and must be labelled as such.

**Write down which of these covers what, and — as with the CPU — which properties nothing covers.**
The M3 lesson stands: reporting the absence of a distinguishing test as evidence of correctness is
the failure this project keeps catching.

## Milestones

| | Goal | Gate |
|---|---|---|
| **M5** | 48K: paged memory, ULA screen, keyboard, 50 Hz interrupt | boots to the copyright message |
| M6 | `.z80`/`.sna` snapshots, `.tap` tape | a real game runs |
| M7 | 128: paging, second ROM, AY-3-8912, per-bank contention | 128-only software runs |
| M8 | WASM + macroquad | playable from a URL |

Contention lands at M5 for the 48K and is extended at M7. It is not deferred: the entire `tick`
contract exists for it, and a machine built without it would have to be rebuilt rather than
extended.

## What the CPU already gives the machine

- `Cpu::interrupt(data) -> u32` and `nmi()`. `interrupt` **declines** — returns 0, changes
  nothing — while `iff1` is clear or the `EI` window is open, so the acceptance rule lives in one
  place and the machine does not reimplement it.
- `Cpu::state()` / `set_state()` over a `CpuState` carrying everything a `.z80` snapshot needs,
  including `wz` and `q`.
- `Cpu::pc()` for cheap loop checks.
- `Cpu::fault()` — a mode-0 device byte with no defined meaning is the one genuine runtime fault.

## What it does not give, and what to do about it

The machine **cannot distinguish an M1 opcode fetch from an operand read** — both arrive as
`Bus::read`. Contention is unaffected, since the delay depends on address and `t mod 8` and the
machine has both. It matters only for a debugger or a precise floating-bus model, and the fix is
non-breaking whenever it is wanted: a defaulted `fn fetch(&mut self, addr) -> u8 { self.read(addr) }`.

Cycle boundaries **are** recoverable without it: a transfer callback opens a machine cycle, and any
`tick` outside such a window is a standalone internal cycle.
