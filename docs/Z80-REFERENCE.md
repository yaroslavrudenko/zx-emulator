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
index constants, it is a **constant offset**: one `hl_base: usize` selects HL, IX or IY and
the entire instruction set works unchanged.

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

`zexall` tests them. Three cases where they are *not* simply the result:

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
| M1 opcode fetch | 4 | every instruction; also increments `R` |
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

Two traps:

- `DD DD DD 21` is three separate one-byte instructions followed by an `LD IX,nn`. Each
  prefix is its own M1 cycle and increments `R` on its own.
- A prefix followed by an opcode that does not involve `HL` behaves as if the prefix were a
  `NOP` — but it still cost its 4 T-states and its `R` increment.

## Interrupts

| | Behaviour |
|---|---|
| `IFF1` | master enable; cleared on interrupt acceptance |
| `IFF2` | shadow copy; `RETN` restores `IFF1` from it |
| `EI` | **the interrupt is not accepted until after the instruction following `EI`** |
| `DI` | clears both immediately |
| `HALT` | executes internal NOPs; `PC` stays on the `HALT` until an interrupt arrives |
| `IM 0` | executes an instruction placed on the bus (on the Spectrum, effectively `RST 38h`) |
| `IM 1` | `RST 38h` |
| `IM 2` | vector at `(I << 8) \| bus_value`; on the Spectrum the bus floats to `0xFF` |
| `NMI` | jumps to `0x0066`, copies `IFF1` into `IFF2` and clears `IFF1` |

The `EI` delay is not a detail — games rely on `EI` immediately followed by `RET` or `HALT`
without taking an interrupt in between.

## `R` register

Incremented on **every M1 opcode fetch**, including each prefix byte. Only the low 7 bits
count; bit 7 keeps whatever was last written by `LD R,A`. Games use it as a cheap random
source and some protection schemes check it, so an emulator that ignores it will run most
software and fail a few titles strangely.

## Verification strategy

| Tier | Source | What it proves |
|---|---|---|
| 1 | FUSE vectors (`tests.in` / `tests.expected`) | per-opcode registers, flags, memory **and T-states** |
| 2 | `zexdoc` | documented behaviour, self-checked by CRC |
| 3 | `zexall` | undocumented flags included — the real gate |
| 4 | `proptest` | ALU flag invariants against an independent derivation |

These are conformance suites, not code. They are fetched locally into `testdata/` (which is
gitignored) and never vendored into this repository.
