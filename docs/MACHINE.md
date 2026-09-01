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

The change is a correctness fix and is justified on that ground alone; `ARCHITECTURE.md`'s position
that performance is a non-goal is not in tension with it. Measured against the real ROM, the boot
run reaches the copyright message on **frame 87 either way** — but on 658,144 instructions rather
than 658,277, so the changed path is genuinely exercised during boot and the effect is 0.02 %, far
below what moves a frame boundary. (Removing contention *entirely* changes instructions-per-frame by
roughly 20 % and does move it, 87 → 85.)
