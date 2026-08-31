# Changelog

Notable changes to the published surface of `crates/z80`. Milestones are recorded even before
the first release, because the crate's API is being frozen decision by decision and the moment
each one stopped being free is worth knowing.

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
