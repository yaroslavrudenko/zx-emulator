# Z80 behavioural reference

Working notes for **our** implementation. Everything here describes the *hardware*, so it
can be implemented from first principles. Nothing in this repository is ported, translated
or adapted from another emulator.

## Register file

| Pair | Bytes | Notes |
|---|---|---|
| `AF` | A, F | F holds the flags; bits 3 and 5 are undocumented but observable |
| `BC` `DE` `HL` | | general purpose; `HL` is the default 16-bit pointer |
| `AF'` `BC'` `DE'` `HL'` | | shadow set, swapped by `EX AF,AF'` and `EXX` |
| `IX` `IY` | IXh/IXl, IYh/IYl | index registers; their halves are individually addressable via DD/FD prefixes |
| `SP` | | stack pointer, full descending |
| `PC` | | program counter |
| `I` | | interrupt vector high byte, used by IM 2 |
| `R` | | memory refresh, **7-bit counter, bit 7 preserved** |

### Why an array and not fields

`DD` and `FD` substitute `IX` or `IY` for `HL` in the following instruction. Held as named
fields, that becomes a branch inside every `HL`-touching handler. Held as `[u8; 26]` with
index constants, it is a **constant offset**: one `PairBase` value — called `base` at every
site that takes it — selects HL, IX or IY, and the entire instruction set works unchanged.

> **Correction — this sentence named `hl_base`, a symbol that has never existed.**
> `grep -rn 'hl_base' crates/` returns zero hits, at every commit. The mechanism is real and the
> argument above is right; the *name* was the wrong part. The value is a `PairBase` newtype over
> `usize` — `crates/z80/src/registers.rs` — and it is **threaded as a parameter rather than stored
> as a field**: `pair(base)`, `set_pair(base, value)`, `register_index(base)`,
> `resolve(operand, index, register_base)`. There is no `base` field on `Cpu`. That is why
> `DD 29` becomes `ADD IX,IX` with no new code — `add_pair(base, base)` already says it.
>
> **It is recorded rather than quietly renamed, because it had already been caught and the fix did
> not reach this file.** [`ARCHITECTURE.md:150`](ARCHITECTURE.md) carries the note that *"two
> independent reviewers found `grep hl_base` returning zero hits while this document described it
> as the decision 'everything hangs on'"*, and [`STATUS.md:540`](STATUS.md)'s M1 hardening-round
> table records the resolution — *"fixed; `base` threaded, so `DD 29` becomes `ADD IX,IX` with no
> new code"*. Two documents found the phantom in a third. **Neither of them carried the fix all the
> way**, and the sweep run for this correction says exactly where it stopped: `ARCHITECTURE.md`
> attached its correction *beneath* the prose but left the phantom name standing in the sentence
> above it (`:146`) and in a forward-looking one below (`:162` — *"`hl_base` will sit on top of
> that"*, future tense about work that has landed under another name); this file received nothing
> at all. So a reader who grepped today reproduced, in full, the failure a sibling document
> describes as solved.
>
> **That is this project's propagation lesson running backwards.** `STATUS.md` writes it up as *a
> derived figure repeated across documents acquires authority it never earned* — a wrong claim
> spreading by being copied. This is the mirror case and it is the quieter one: a **correction**
> failing to spread, which leaves no trace at all, because the uncorrected copy still reads exactly
> as it always did. The rule covers both directions: **a correction is not landed until you have
> grepped for every other copy of the thing you corrected.**
>
> Same class as `Cpu::pc()`, which `MACHINE.md` documented as an available method for two
> milestones and which never existed — `MACHINE.md:383` records that correction. A name is a claim
> about the code, and it is the cheapest claim in any of these documents to check.

## Flag register

```
bit  7   6   5   4   3   2   1   0
     S   Z   5   H   3  P/V  N   C
```

| Flag | Meaning |
|---|---|
| `S` | copy of bit 7 of the result |
| `Z` | result is zero |
| `5` | **undocumented** — copy of bit 5 of the result |
| `H` | half carry: carry out of bit 3 (8-bit) or bit 11 (16-bit) |
| `3` | **undocumented** — copy of bit 3 of the result |
| `P/V` | parity for logical ops and rotates, signed overflow for arithmetic |
| `N` | set by subtraction, cleared by addition — `DAA` reads it |
| `C` | carry |

### The undocumented bits are not optional

`zexall` tests them. Five cases where they are *not* simply the result of the operation:

0. **`CP r`** — bits 3 and 5 come from the **operand**, not the result. This is what separates
   `CP` from `SUB`, which are otherwise identical; the accumulator is unchanged and only these two
   bits differ.
0. **`ADD HL,ss` and the 16-bit arithmetic** — they come from the **high byte of the result**.

…and three that depend on state the instruction does not otherwise touch:

1. **`BIT n,(HL)`** — bits 3 and 5 come from the internal address latch (the high byte of
   the address formed during the operation), not from the tested value.
2. **`BIT n,(IX+d)` / `(IY+d)`** — same, from the high byte of `IX+d`.
3. **`SCF` / `CCF`** — the value depends on whether the *previous* instruction modified the
   flags. This is the "register Q" behaviour: if the last instruction wrote F, bits 3/5
   come from A; otherwise they are OR-ed with the existing F bits.

Implement `Q` as a small piece of CPU state: set it when an instruction writes F, clear it
otherwise, and read it in `SCF`/`CCF`.

## Overflow (`P/V`) for arithmetic

Signed overflow, not parity. For `a + b = r` on 8 bits:

```
overflow = ((a ^ r) & (b ^ r) & 0x80) != 0
```

For subtraction `a - b = r`:

```
overflow = ((a ^ b) & (a ^ r) & 0x80) != 0
```

Deriving both from one shared helper is how the flag classes stay honest.

## `DAA`

The one instruction that cannot be implemented by pattern-matching a table without
understanding it. It adjusts A after BCD arithmetic, and its behaviour depends on `N`,
`H` and `C` together:

- if `H` is set, or the low nibble of A is above 9 → add or subtract `0x06`
- if `C` is set, or A is above `0x99` → add or subtract `0x60`, and set `C`
- the direction (add or subtract) is chosen by `N`

`H` after `DAA` is set from the carry out of the low-nibble adjustment.

## Instruction timing

Every instruction is a sequence of machine cycles; the fuse vectors check the total. The
shapes worth internalising:

| Cycle | T-states | When |
|---|---|---|
| M1 opcode fetch | 4 | every instruction; also increments `R`. **Not every M1 cycle reads memory** — see *Which M1 cycles read memory*, under the `R` register |
| Memory read | 3 | operand or data fetch |
| Memory write | 3 | data store |
| IO read/write | 4 | `IN` / `OUT` |
| Internal operation | 1–5 | index displacement add, 16-bit arithmetic, etc. |

`tick()` must be called **as each of these happens**, not summed at the end — Spectrum
contention depends on *when* within the frame the bus is touched.

Conditional instructions cost differently by outcome:

- `JR cc,e` — 12 T if taken, 7 if not
- `RET cc` — 11 if taken, 5 if not
- `CALL cc,nn` — 17 if taken, 10 if not
- `DJNZ` — 13 if the branch is taken, 8 if not
- Block instructions (`LDIR`, `CPIR`, `INIR`, `OTIR`) — 21 T while repeating, 16 on the
  final iteration

## Prefixes (milestone M2, documented now so M1 leaves room)

| Prefix | Effect |
|---|---|
| `CB` | bit, rotate and shift group |
| `ED` | extended: block ops, 16-bit `ADC`/`SBC`, `IN`/`OUT` variants, `IM`, `RETN`/`RETI` |
| `DD` | the next instruction uses `IX` instead of `HL` |
| `FD` | the next instruction uses `IY` instead of `HL` |
| `DDCB` / `FDCB` | **the displacement byte comes before the opcode byte** |

Traps, all of them found by reading the corpus rather than by reading a specification:

- `DD DD DD 21` is three separate one-byte instructions followed by an `LD IX,nn`. Each
  prefix is its own M1 cycle and increments `R` on its own.
- A prefix followed by an opcode that does not involve `HL` behaves as if the prefix were a
  `NOP` — but it still cost its 4 T-states and its `R` increment.
- **`ED` ignores an index prefix entirely.** `DD ED 44` is `NEG` at three M1 cycles.
- **Unassigned `ED` encodings are two-byte `NOP`s.** That is hardware behaviour, not a fallback —
  an emulator that faults on them is wrong, and one that treats the arm as a catch-all has hidden
  a real rule inside an accident.
- **The `HL`→`IX`/`IY` substitution is wider than "`H` and `L` shift".** *If either operand is
  memory, neither register half is substituted* — the prefix has already been spent on the
  address. `DD 66` (`LD H,(IX+d)`) writes the **real** `H`; `DD 74` (`LD (IX+d),H`) stores the
  **real** `H`, not `IXh`.
- **`DDCB`'s two operand bytes are memory reads, not M1 fetches.** `R` advances twice for a
  four-byte instruction.
- **The `DDCB` set has undocumented forms that write twice** — to `(IX+d)` *and* to a register.
  The low three bits select `B C D E H L — A`; only the `110` encoding is the documented
  memory-only form. Seven of eight encodings copy.
- **The index-computation cost is not a constant.** Five T-states are owed between the
  displacement byte and the memory access, and **any 3-T fetch in between spends three of them** —
  which is why `LD (IX+d),n` and every `DDCB` form charge two rather than five. Compute the
  address and charge the time in separate places; only the caller knows what intervened.

### The block instructions repeat on four different addresses

`LDIR`/`CPIR`/`INIR`/`OTIR` and their decrementing twins spend their five repeat T-states on
**the last address that was on the bus** — which is a different register in each family:

| | repeats on | because |
|---|---|---|
| `LDIR` / `LDDR` | `DE` | the write |
| `CPIR` / `CPDR` | `HL` | the read |
| `INIR` / `INDR` | `HL` | the write |
| `OTIR` / `OTDR` | the port (`BC` after `B`'s decrement) | the output |

One rule, four addresses. Nothing in a specification says this; the trace does.

Two more of their rules are not the general ones: **P/V comes from `BC ≠ 0`**, not parity, and
**bit 5 of `F` comes from bit 1 of `A + transferred byte`** while bit 3 comes from bit 3 of the
same sum. A test whose sum makes those two readings agree proves nothing.

Their repeat is `PC -= 2` with **one `step()` per iteration** — the instruction re-fetches its own
opcode each pass, so `R` advances by two per iteration. That is not an implementation choice: it
is what keeps a 64 KB `LDIR` interruptible, and therefore what lets it coexist with a 50 Hz frame
interrupt.

## Interrupts

| | Behaviour |
|---|---|
| `IFF1` | master enable; cleared on interrupt acceptance |
| `IFF2` | shadow copy; `RETN` restores `IFF1` from it |
| `EI` | **the interrupt is not accepted until after the instruction following `EI`** |
| `DI` | clears both immediately |
| `HALT` | executes internal NOPs; `PC` stays on the `HALT` until an interrupt arrives. Each of those cycles is a **full M1 cycle** — it refreshes, so it fetches; the byte is discarded, not un-read |
| `IM 0` | executes an instruction placed on the bus (on the Spectrum, effectively `RST 38h`) |
| `IM 1` | `RST 38h` |
| `IM 2` | vector at `(I << 8) \| bus_value`; on the Spectrum the bus floats to `0xFF` |
| `NMI` | jumps to `0x0066`, copies `IFF1` into `IFF2` and clears `IFF1` |

The `EI` delay is not a detail — games rely on `EI` immediately followed by `RET` or `HALT`
without taking an interrupt in between.

## `R` register

Incremented on **every M1 cycle**, including each prefix byte. Only the low 7 bits
count; bit 7 keeps whatever was last written by `LD R,A`. Games use it as a cheap random
source and some protection schemes check it, so an emulator that ignores it will run most
software and fail a few titles strangely.

### What the address bus carries during each T-state of M1

`R` exists to drive a refresh address at DRAM, and *where in M1* that address appears is a separate
question from what `R` counts. This project asserted an answer to it in three places without ever
citing one; it is settled here, once, with the evidence class attached.

**Source, and it is primary.** *Z80 CPU User Manual*, Zilog, **UM008011-0816**, ©2016 —
<https://www.zilog.com/docs/z80/um0080.pdf>, fetched 2026-09-01, SHA-256
`e3c83da5a5d8e372364c20fa53665e6fbb165ec6ac38c8c1eebc359603447b5e`. Section *Instruction Fetch* and
Figure 5, *Instruction Op Code Fetch*.

| T-state | `A15–A8` | `A7` | `A6–A0` | Class |
|---|---|---|---|---|
| T1, T2 | `PC` | `PC` | `PC` | **proven** — *"The Program Counter is placed on the address bus at the beginning of the M1 cycle"*, and Figure 5 labels the `A15–A0` row `PC` across T1–T2 |
| T3, T4 | `I` | `R` bit 7 | `R` bits 6–0 | `A15–A8` and `A6–A0` **proven**; `A7` **derived** — see below |

Figure 5 draws the address-bus row as exactly two fields, `PC` then `Refresh Address`, **with the
boundary between T2 and T3**. The address bus changes mid-cycle, and M1 is the only cycle where it
does — a memory read or write holds one address for all three of its T-states.

The two halves of the refresh address come from two different sentences, which is why the bits are
classed separately:

> During T3 and T4, **the lower seven bits of the address bus contain a memory refresh address** and
> the RFSH signal becomes active, indicating that a refresh read of all dynamic memories must be
> performed. To prevent data from different memory segments from being gated onto the data bus, an
> RD signal is not generated during this refresh period. The MREQ signal during this refresh period
> should be used to perform a refresh read of all memory elements. The refresh signal cannot be used
> by itself, because **the refresh address is only guaranteed to be stable during the MREQ period**.

> *(Memory Refresh (R) Register)* … The data in the refresh counter is sent out on the lower portion
> of the address bus along with a refresh control signal while the CPU is decoding and executing the
> fetched instruction. … **During refresh, the contents of the I Register are placed on the upper
> eight bits of the address bus.**

So `A8–A15` = `I` is stated outright, and `A0–A6` = the seven counting bits is stated outright.
**`A7` is the one bit the manual never names**: it is `R`'s eighth bit, the latch `LD R,A`
writes and the counter does not touch, and it reaches the bus only via *"sent out on the
lower portion of the address bus"*. `Registers::refresh_address` composes all sixteen as
`{I, R}`, which is the universal emulator convention and is **derived, not proven, in exactly
one bit**. Nothing here depends on that bit, but a claim that `IR` is proven wholesale would
be one bit too strong.

### `R` on the bus: **pre**-increment during M1, **post**-increment after it

A different question from *which* register is on the bus, the one most likely to produce a wrong
"fix", and one this repository currently states only half of.

- **During T3–T4 of M1** the bus carries `{I, R}` with **`R` as it was before this fetch's
  increment**. **Measured**, at die level: floooh's run of the visual6502 Z80 netlist shows `AB` =
  `2203` held across `3/0 … 4/0` while the `R` register itself steps `03 → 04` at `3/1`
  ([the netlist write-up][floooh-m1]). Independently **observed** in `redcode/Z80`, whose
  `z80_refresh_address()` returns `(r - 1) & 127 | (r7 & 128)` — it must subtract one to recover
  the address of the M1 in progress, which is correct only if the hardware drove the pre-increment
  value.
- **During the internal cycles *after* M1** it is the **post**-increment value. **Proven by the
  corpus**: `c7` starts at `R`=`0x00`, runs one M1, and records `4 MC 0001`; `ed57` starts at
  `R`=`0x17`, runs two M1 cycles (`ED` then `57`), and records `8 MC 1e19` = `0x17 + 2`.

[floooh-m1]: https://floooh.github.io/2021/12/06/z80-instruction-timing.html

**This core implements the second and never faces the first**, because it drives `PC` through M1.
`Cpu::fetch_opcode` calls `increment_r()` before its ticks precisely so the internal cycles that
follow get the post-increment value, and `bus_timing.rs` gates that with `REFRESH_ADDRESS` =
`0x4006` after an initial `R` of `0x05`.

> **`crates/z80/src/lib.rs`'s `fetch_opcode` doc-comment states this as one rule and it is two.** It
> reads *"the refresh address the Z80 drives during **and after** M1 carries the post-increment
> value"* — right for *after*, wrong for *during*. That is the same conflation as the
> `ARCHITECTURE.md` sentence corrected in this pass, in a third place: a claim about the fetch's
> own T3–T4 fused with a claim about the cycles that follow it. **Anyone making M1
> hardware-accurate must read `refresh_address()` before `increment_r()` for the fetch's own two
> T-states and after it for everything downstream** — one function needing both values. That file
> is not this one's to change;
> the defect is recorded here because this is where the hardware rule lives.
>
> > **Corrected on 2026-09-01: the comment now states the two rules separately**, and points back
> > here rather than restating the evidence. Only the *comment* — the model is unchanged and
> > `fetch_opcode` still drives `PC` for all four T-states, deliberately; see *What this core does,
> > and why the difference is inert* below, and the Open row in [`STATUS.md`](STATUS.md) that still
> > carries it. **This note is written from the changed file rather than from memory of it**, which
> > is the discipline [`MACHINE.md`](MACHINE.md) arrived at after two documents each recorded the
> > other as untouched and a reader would have read that as corroboration.

Two further things the manual settles, both of which matter more than they look:

- **`RD` is not asserted during T3–T4.** The refresh is an `MREQ`-only cycle. A machine that decides
  "is this a memory access" from `MREQ` alone sees the refresh address; one that requires `RD` or
  `WR` does not.
- **`/WAIT` is sampled at T2 and at each `TW`, and nowhere else**: *"During T2 and every subsequent
  automatic WAIT state (TW), the CPU samples the WAIT line with the falling edge of the clock. If
  the WAIT line is active at this time, another WAIT state is entered during the following
  cycle."* Wait states are inserted **between T2 and T3 — before the refresh address exists**. So on
  any machine that contends by asserting `/WAIT`, the M1 refresh half cannot lengthen its own cycle.

> **Do not extend that last point to the Spectrum without naming the mechanism — the sources
> disagree, and an earlier revision of this section asserted the wrong one.** It said *"a Spectrum
> charges contention by holding `/WAIT`"*. The Sinclair Wiki's *ZX Spectrum 16K/48K* page says the
> opposite: *"the ZX Spectrum uses a memory contention scheme based on stopping the Z80's clock,
> rather than using the Z80's `WAIT` signal"* — and a stopped clock freezes the CPU wherever it
> stands, T3 and T4 included, which is the case the `/WAIT` rule would have excluded. Chris Smith's
> gate-level ULA reverse engineering is the primary source that settles it and is not in hand;
> `zxdesign.info` refused connection again on 2026-09-01, as it did throughout M7, and
> `web.archive.org` is unreachable from this environment.
>
> **The conclusion is unaffected and is better sourced than the mechanism was.** Contention is
> applied at **T1 of a machine cycle and nowhere else** — the Sinclair Wiki's contention page
> (*"this happens on the first tstate (T1) of any instruction fetch, memory read or memory write
> operation"*, **observed**), the community opcode-timing table, which writes every fetch as `pc:4`
> with no `ir` term (**observed**), and the snow effect, which is the **measured** half: `I` in
> `0x40..=0x7F` corrupts the display while, per World of Spectrum's 48K reference, *"the Spectrum
> won't crash, and program will continue to run normally"*. The refresh address demonstrably
> reaches the memory system and demonstrably does not change the instruction's timing.
>
> **Where the refresh address *does* change timing on a 48K is the internal cycles after the
> fetch**, not the fetch — the `ir:1` terms in the community table. Those are MREQ-inactive
> T-states that the Ferranti ULA contends and the Amstrad gate array does not, which is why
> `INC dd` is `pc:4,ir:1 ×2` on a 48K and a flat `pc:6` on a +2A/+3. This core models the 48K
> behaviour, and `contention_magnitude.rs` gates it.

**What this core does, and why the difference is inert.** `Cpu::fetch_opcode` drives `PC` for all
four T-states — it diverges from the hardware on T3–T4 and matches it on T1–T2. Nothing can see it:
the FUSE corpus names T=0 of every fetch and no interior T-state in any of its 1335 vectors; `Ula`
discards `Bus::tick`'s address inside an open machine cycle; and the hardware, per the block above,
applies contention only at T1 of a cycle. Verified by mutation rather than argued — driving
`PC, PC, IR, IR` leaves 290/290, 1045/1045 and all 68 rows of the hardware timing oracle unmoved.
The full account, the mutation table and the disposition are on `compare_contention` in
`crates/z80/tests/common/report.rs`.

> The `HALT` paragraph below says a halted M1 cycle has *"the address driven from `PC`"*. Read as a
> statement about the machine **cycle** — which bus transfer it performs, and therefore whether it
> is a `fetch` or a `read` — that is right and is the point being made there. Read per T-state it is
> the same approximation as everywhere else in this repository, and the table above is what it
> approximates.

### Which M1 cycles read memory, and why `R` and the opcode fetch are not the same count

`R` counts M1 cycles. It does **not** count opcode fetches, and the two part company because two M1
cycles are not ordinary reads of the program stream. Both cases are decided by asking what the
hardware puts on which bus, not by what is convenient to implement.

**A halted CPU's discarded byte *is* a fetch.** A halted Z80 has not stopped: it keeps issuing M1
cycles and refreshing memory, executing an internal `NOP` each time, with `PC` parked on the `HALT`
opcode. **`R` settles it — the Z80 has no way to refresh without an M1 cycle, and this cycle
refreshes.** So it is an M1 cycle in full: four T-states, `/M1` asserted, the address driven from
`PC`, differing from any other opcode fetch only in that the core throws the byte away. Calling it
an ordinary read would tell a contention model to charge a three-T-state read plus an internal cycle
for what the hardware spends as one four-T-state fetch.

**An interrupt acknowledge is *not* a fetch.** It reads no memory. The Z80 asserts `/M1` together
with **`/IORQ` in place of `/MREQ`**, and the interrupting device answers on the data bus; in mode 0
that byte *is* the instruction, and it arrives from the device rather than through the address
space at all. What the address bus carries is `IR` — the refresh address, not an address to fetch
from — so reporting a memory cycle here would invite the machine to contend it and to serve it from
its own memory map, both of which would be fiction. Modes 1 and 2 read no instruction byte at all,
and mode 2's two vector-table lookups are ordinary memory reads.

**So the tempting invariant — one opcode fetch per `R` increment — is true only with its scope
stated.** Exactly:

> `R` increments **once per M1 cycle**. An opcode fetch happens once per M1 cycle **that reads
> memory**. The interrupt acknowledge is the only M1 cycle that is neither.

The correspondence is therefore **exact across `step()`** — where a frame loop spends effectively
all of its time — and **off by one per accepted interrupt or NMI**. Nothing about the hardware
changed between those two sentences; only the universal phrasing was wrong.

**How that was found matters as much as the rule: by trying to test the invariant rather than by
asserting it.** `crates/z80/tests/bus_timing.rs` carries
`an_interrupt_acknowledge_refreshes_without_fetching` for exactly this reason. An acknowledge that
started routing through `Bus::fetch` would read as a tidy-up — one more M1 cycle brought into line
with the rest — and would silently charge a memory cycle the hardware never performs. The exception
now has a failing case of its own, so it cannot quietly become a bug.

## Check the trace before writing the handler

Five times on this project, reading the corpus beat reasoning from the specification:

| | What only the trace showed |
|---|---|
| `MC` is one event per **machine cycle**, not per T-state | a uniform per-T-state tick would over-contend every fetch |
| `RST` spends its internal cycle on `IR`, `CALL` on the last operand address | they share one handler; `RST` has no operand fetch to overwrite the bus |
| `DJNZ` uses **two** addresses in one instruction | the extra M1 T-state on `IR`, then five on the displacement's own address |
| The four block families repeat on four different registers | see above |
| The `LD r,r'` substitution asymmetry | `DD 66` writes real `H` |

The pattern in every case is the same, and it is what makes the rule worth stating: **the
specification was silent, not wrong.** Reading it more carefully would not have helped. When a
behaviour depends on what was last on the bus, on the order of fetches within an instruction, or
on state the instruction does not name, only a trace can answer — so look before writing the
handler, not after the vector fails.

## Verification strategy

| Tier | Source | What it proves |
|---|---|---|
| 1 | FUSE vectors (`tests.in` / `tests.expected`) | per-opcode registers, flags, memory **and T-states** |
| 2 | `zexdoc` | documented behaviour, self-checked by CRC |
| 3 | `zexall` | undocumented flags included — the real gate |
| 4 | `proptest` | ALU flag invariants against an independent derivation |

These are conformance suites, not code. They are fetched locally into `testdata/` and never
vendored into this repository.

> **Correction — this said `testdata/` is *"gitignored"*, full stop, and that is incomplete in the
> one place it matters.** `.gitignore` carries `testdata/**` and then **un-ignores by exception**:
> `!testdata/.gitkeep`, `!testdata/README.md`, `!testdata/roms/`, `!testdata/roms/48.rom`. So the
> Sinclair 48K ROM **is committed** — `git ls-files testdata/` returns `.gitkeep`, `README.md` and
> `roms/48.rom` — under the permission quoted in [`../testdata/README.md`](../testdata/README.md),
> and because a subtly wrong ROM is the one corpus failure no harness here would explain.
> `README.md`'s *Test data* table states this correctly and `MACHINE.md` calls it *"the committed
> ROM"*; this line did not, and is corrected to match rather than left as the odd one out.
>
> > **The last exception was `!testdata/roms/*.rom` when this block was written, and it is now one
> > explicit filename per ROM.** The quotation above is corrected in place rather than annotated,
> > because it is a transcription of another file and a transcription that has drifted is simply
> > wrong. Why the glob went: it accepts *any* file ending in `.rom` — a game cartridge, a Multiface
> > image, an Interface 1 ROM — and the permission this repository relies on **disclaims** the
> > Interface 1 and 2 ROMs as not Amstrad's copyright at all. A glob turns *"may we redistribute
> > this?"* from a decision into an accident.
>
> Absence is not silent. A missing corpus makes its gate **fail**, naming the fetch instructions;
> `ZX_CORPUS_ALLOW_MISSING=1` is the deliberate opt-out and is **refused** when `CI` is also set.
> The rule and its failing cases live in `crates/testsupport`.
