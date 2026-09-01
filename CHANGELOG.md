# Changelog

Notable changes to the published surface of `crates/z80`. Milestones are recorded even before
the first release, because the crate's API is being frozen decision by decision and the moment
each one stopped being free is worth knowing.

## Unreleased — `Bus::fetch`, the M1 opcode fetch

### Added

- **`Bus::fetch(&mut self, addr: u16) -> u8`**, defaulted to `Bus::read`. Non-breaking: every
  existing implementation compiles and behaves identically without touching it, and `spectrum`
  was left unmodified as the proof.

  It exists because M1 is the one machine cycle whose **length** a machine cannot infer from
  the call stream. A write is three T-states and a port access is four, but a read is three for
  an operand and four for an opcode fetch — so `LD A,B` (one M1 cycle) and the read-modify half
  of `INC (HL)` (a three-T-state read, then an internal cycle) emitted **byte-identical**
  streams: one transfer callback followed by four ticks at the same address. A contention model
  owes one stall for the first and two for the second, and nothing in the stream said which.

  This is not a speculative addition. `crates/spectrum/src/machine_cycle.rs` reconstructs cycle
  boundaries by deferring the fourth tick until a fifth discloses the shape, and its residual
  error is exactly one contention point — 0 to 6 T-states — on the read-modify-write family
  (`INC`/`DEC (HL)` and `(IX+d)`, the `CB` operations on memory, `EX (SP),HL`). That residual is
  pinned by a test of its own. `fetch` removes the ambiguity at the source rather than
  reconstructing it downstream.

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

### Note for machine authors

The correspondence above is also the gate. `crates/z80/tests/bus_timing.rs` asserts
**one `Bus::fetch` call per `R` increment** across an un-prefixed instruction, a 300-byte prefix
run, a `DDCB` form, an `ED` block instruction mid-repeat and `HALT`, with the interrupt exception
tested rather than left as a footnote. `R` is independently graded by 290/290 and 1045/1045 FUSE
vectors and by `zexall`, so the new method is anchored to something already proven rather than to
a hand-count of call sites — and the check bites in both directions, since a fetch left on `read`
drops one side of the equation while an operand read promoted to `fetch` inflates the other.

## Unreleased — M3, `zexdoc`

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
no-op `tick`. That is within 7 % of `benches/step.rs`'s 329x, so the benchmark's figure holds
over a real instruction mix and not just its own sample.

The number that matters for a frame loop is the other one in that pair: a `dev`-profile build
runs the same work at ~4.9 M instructions/s, **27x slower**. Anything scheduling `Cpu::step`
against a wall clock must be built in release.

## Unreleased — M2, the four prefixes

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

## Unreleased — M1, the un-prefixed opcodes

### Added

- `Bus`, `Cpu<B>`, `CpuState`, `InterruptMode`, `StepError` and the un-prefixed instruction set.
- `Bus::tick(&mut self, addr: u16)` — one call per T-state, never batched, with the address the Z80
  actually drives. Contention is a function of `t mod 8`, so N separate cycles are not one N-cycle
  block; and a machine can track its own transfers but can never learn `IR`, which is what sits on
  the bus during the internal cycles of `ADD HL,ss`, `JR`, `DJNZ`, `CALL` and `PUSH`.
- `Cpu::interrupt` and `Cpu::nmi`. Without them there was no route out of `HALT` at all.
