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

    /// The opcode byte of an M1 cycle. Defaulted, so implementing it is optional.
    fn fetch(&mut self, addr: u16) -> u8 { self.read(addr) }

    /// One T-state elapses with `addr` on the bus. Called once per T-state, never batched.
    fn tick(&mut self, addr: u16);
}

pub struct Cpu<B> { /* ... */ }
```

> **Corrected: this block carried `pub struct Cpu<B: Bus>` long after the bound moved to the
> `impl` blocks.** `STATUS.md` records the struct-level bound's removal in its *Closed* table —
> *"Removed. The declaration is `pub struct Cpu<B>`"* — and this code block kept the old
> signature anyway. That is worse than the same slip in prose, because a code block is what
> somebody copies. The substantive claim the block is here to make — six methods, one
> defaulted, generic rather than `Box<dyn Bus>` — is unaffected and is gated in
> `crates/z80/tests/codegen.rs`.

Six methods. **`fetch` is the newest and the only defaulted one**, and it exists because M1 is the
one machine cycle whose *length* the call stream does not disclose: a write is three T-states and a
port access is four, but a read is three for an operand and **four** for an opcode fetch. Routed
through one method, `LD A,B` and the read-modify half of `INC (HL)` emit byte-identical streams
while owing different amounts of contention. `MACHINE.md` argued at M4 that this did not matter and
was wrong; the full account of the argument and its falsification lives there, and only there.

**It was defaulted because that makes it non-breaking by construction, and on the day it landed
that was checked rather than asserted.** `crates/z80/src/bus.rs` gained the method while
`crates/spectrum/src/ula.rs` was left byte-identical and contained no `fn fetch` at all, so the
machine crate compiled *and* passed on the default, unmodified — `cargo test -p spectrum` reported
**98 passed, 0 failed** across the lib and the five integration binaries that existed then.

**Every clause of that is history now, and it is written in the past tense for a reason: three of
them were still in the present tense long after they stopped being true.** `Ula` implements `fetch`
(`grep -n 'fn fetch' crates/spectrum/src/ula.rs` — a command rather than a line number, because
`ula.rs` is edited by every milestone and the number this sentence carried was already stale),
`ula.rs` has been rewritten, `machine_cycle.rs` — the **312-line** module (191 of them production;
`#[cfg(test)]` begins at 192) that existed only to reconstruct M1 boundaries the old call stream had
discarded — is **deleted**, and `crates/spectrum/tests/` has grown past the five binaries of that
day. (That last
one is left as "grown past five" on purpose: a live count of files in another crate's test directory
is a claim that rots on someone else's commit, which is the defect this paragraph is an example
of. The number that matters is the one the evidence needs, and that number is historical.) What
survives is the evidence, and it
survives precisely because it is a claim about a moment: a defaulted trait method was added to a
published trait and a downstream implementor kept working untouched. That is what "non-breaking"
means, and it stays proven whether or not the implementor has since opted in. It has.

> **That sentence said "the 161-line module", and 161 is not a file size — it is a *net LOC
> delta*.** `git show 2157331:crates/spectrum/src/machine_cycle.rs | wc -l` is **312**, of
> which 191 are production. The 161 is the retirement's net change across `machine_cycle.rs`
> **and** `ula.rs` together, and [`STATUS.md`](STATUS.md) states it correctly, as
> *"Production LOC | −161"*, in the table this sentence borrowed it from.
>
> This is the class that document already has a name for — a derived figure carried away from
> its derivation, where it can no longer be checked against the thing it was derived from. The
> tell here is that the wrong reading is *plausible*: a module and a delta are both line
> counts, both about the same change, and one of them is right.

Two numbers that go with the opt-in, both **reported by the agent that did the work and not
reproduced in this pass**: `INC (HL)` in contended memory was derived independently twice as **26
T-states at contention phase 0 and 19 at phase 7**, and the implementation agrees with both. And a
warning about a phrase this repository repeats: *"one contention point, 0–6 T-states"* names the
**isolated** stall, not the error. The **net observable** residual, swept over 448 start positions,
was **0 or 1 T-state and never more**. Conflating the two once put a wrong figure (30, not 26) into
circulation, so quote whichever you mean and say which it is.

Two further properties, and the first revision of this document got both wrong. They were corrected
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

That last claim — that the internal cycles ride `IR` rather than `PC` — was an argument until M5
and is now a **measurement**: driving a real `Cpu<Ula>`, `ADD HL,BC` at `0x4000` costs **17
T-states at contention phase 0 and 11 at phase 7**, and those two numbers match a hand computation
only if all seven internal cycles are addressed by `IR`. Taken by the cold review of commit
`2157331`; **not independently reproduced in this pass**, and the person to reproduce it is whoever
next has a `Cpu<Ula>` positioned at a chosen frame phase.

> **Reproduced, 2026-09-01 — and reproducing it falsifies the sentence three lines above it.**
> A second `Cpu<Ula>`, positioned with `advance_to(FIRST_CONTENDED_T_STATE + phase)` and stepped
> once over `ADD HL,BC` at `0x4000`, returns **17 at phase 0 and 11 at phase 7**. Twice measured,
> by two people, on two occasions. The paragraph above may drop its caveat.
>
> The reproduction was run as a **sweep over where `I` points**, which is what makes it say more
> than the original did. Same instruction, same phases, only `I` varying:
>
> | code at | `I` | refresh address | phase 0 | phase 7 |
> |---|---|---|---|---|
> | `0x4000` contended | `0x00` | `0x0001` ROM, uncontended | **17** | **11** |
> | `0x4000` contended | `0x3F` | `0x3F01` ROM, uncontended | **17** | **11** |
> | `0x4000` contended | `0x40` | `0x4001` screen, **contended** | **39** | **32** |
> | `0x8000` uncontended | `0x3F` | `0x3F01` ROM, uncontended | **11** | **11** |
> | `0x8000` uncontended | `0x40` | `0x4001` screen, **contended** | **31** | **32** |
>
> **So `17` and `11` are the figures for an *uncontended* `IR`, and the sentence that says otherwise
> is wrong.** *"On a 48K, program code normally lives in contended RAM `0x4000–0x7FFF`, so `IR`
> points into contended memory for most of a game's runtime"* conflates two different registers.
> `PC` follows the program; `IR` follows **`I`**, which the program sets and which has nothing to do
> with where the code sits. Measured against the real 48K ROM over 400,000 sampled instructions:
> **`I` = `0x3F` in 399,992 of them** (`0x00` in the first eight, before the ROM's own `LD I,A`), so
> the refresh address is `0x3Fxx` — in ROM, uncontended, throughout. An `IM 2` table is
> conventionally placed high, which is uncontended too. `IR` in contended memory is the *unusual*
> configuration, not the normal one; it is what produces "snow" on real hardware, and the row that
> costs 39 is that case, not the common one.
>
> **The paragraph's conclusion survives its premise being backwards, and is strengthened by the
> correction.** The address still has to travel with the tick — but the reason is the fourth row
> above, not the third: with `I` = `0x40` an instruction in *uncontended* memory costs **31** rather
> than 11, so a bus that assumed the internal cycles rode `PC` would get that case wrong in a bank
> where it had no other reason to expect contention at all. The property is real and large; only
> the story about which way round it fires was wrong.
>
> **What this does *not* cover: the first clause, about the second half of M1.** *"the refresh
> address the Z80 drives during the second half of M1"* is a claim about T3–T4 of the fetch itself,
> and it is a **different claim** from the one about the internal cycles that follow the fetch. The
> corpus proves the second and is silent on the first — 1335 vectors, an `MC` at T=0 of every fetch
> and none at T=1, T=2 or T=3.
>
> The first clause is nonetheless **true and now proven**, from Zilog's own *Z80 CPU User Manual*
> (UM008011-0816, Figure 5): the address bus carries `PC` for T1–T2 and the refresh address for
> T3–T4, and **this core does not do that** — `Cpu::fetch_opcode` drives `PC` for all four. The
> divergence costs nothing, and not only because our contention model cannot see it: the same manual
> says `/WAIT` is sampled *"during T2 and every subsequent automatic WAIT state"* and nowhere else,
> so a Spectrum — which charges contention by holding `/WAIT` — **cannot** stall an M1 on whatever
> its second half is driving. Measured as well as derived: driving `PC, PC, IR, IR` leaves 290/290,
> 1045/1045 and all 68 rows of the hardware timing oracle unmoved. The per-T-state table with its
> evidence classes is in [`Z80-REFERENCE.md`](Z80-REFERENCE.md), where hardware rules live; the
> disposition, the mutation table and the two files that pin the M1 interior are on
> `compare_contention` in `crates/z80/tests/common/report.rs`.

Generic over `B`, never `Box<dyn Bus>` — monomorphised, no indirect call on the hot path.
`read`/`write` carry `#[inline]`; cross-crate inlining does not happen otherwise. Both
properties are verified in emitted assembly, not assumed — see *Measured*, below.

> **Correction — the annotation is necessary and it is not sufficient, and this paragraph
> credits it for something the *link* does.** `#[inline]` makes a function's body available
> across a crate boundary; it does not make the inliner take it. Measured at M5: `Ula::tick`
> carries `#[inline]` and is nonetheless emitted out of line and **called 124 times** in
> `crates/spectrum`'s library object — `cargo rustc -p spectrum --release --lib -- --emit=asm`.
> Fat LTO at link time is what removes those calls, so the **shipped binary does get** the
> property this paragraph claims and nothing about the emulator is slower than described. What
> is wrong is the attribution: a reader who took *"cross-crate inlining does not happen
> otherwise"* to mean *"and it does happen with the annotation"* was reading a guarantee that
> is not there.
>
> The same correction applies to the sentence's other half. *"No indirect call on the hot
> path"* is true of a whole-program build and **false of the `Cpu<Ula>` object**, which keeps
> one. Both claims are properties of a **build unit**, not of the code, and neither of them
> said which. See *Codegen*, below, where the same core measures different numbers in a
> library object and in a linked binary.

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
array it is a **constant index offset** — one base index, and the entire HL instruction set
operates on IX with no `if` at all.

> **The name in that sentence used to be `hl_base`, and there has never been such an item.**
> `grep -rn 'hl_base' crates/` returns nothing at any commit. The mechanism is real and shipped;
> it is a `PairBase` threaded as a **parameter named `base`** — `pair(base)`,
> `set_pair(base, value)`, `decode.rs`'s `register_index(base)`. The note below records the
> reviewers catching the *mechanism* missing; what nobody caught until now is that the
> **name** was a phantom too, so `grep hl_base` returning zero hits stayed true after the work
> landed and meant something different. That is the `Cpu::pc()` class this document already
> records once: a plausible identifier written from the design's intent rather than from the
> crate, and nothing checking.

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
> the base will sit on top of that, not replace it. *(This sentence was future tense —
> "`hl_base` will sit on top of that" — about work that had already landed under a different
> name. A forward-looking claim is the one kind of stale prose a reader cannot detect by
> checking the crate, because the thing it names is supposed not to exist yet.)*

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

> **CORRECTION — the first sentence stopped being true for contention on 2026-09-01, and it is the
> third copy of that claim to need this note.** `crates/spectrum/tests/timing_oracle.rs` runs
> Richard Butler's 48K timing test suite — a `.z80` carrying 34 instruction groups and two tables of
> results *measured on real Spectrums* — and reports **68 hardware rows, 0 disagreements**. It is a
> tier-1 oracle by this section's own definition: a number to compare, not a picture to squint at.
> Thirteen mutations bound it, each verified to have landed before its verdict was trusted;
> `FIRST_CONTENDED_T_STATE` at 14333, 14334, 14336 and 14337 all go red and **only 14335 is green**.
> The full account is in [`MACHINE.md`](MACHINE.md)'s *The timing oracle*, and **only** there.
>
> **What that establishes is narrower than "the contention model is right", and the difference is
> the point.** Precisely: **the first contended T-state falls exactly 14335 T-states after `/INT`**,
> given that this machine asserts `/INT` at frame T-state 0. Three mutations came back *green* and
> each marks something the oracle cannot settle — the frame's **origin** is a convention (moving
> `/INT` and the window together passes), the interrupt window's **length** is unmeasured (32 → 24
> passes), and the `64 × 224` **factorisation** is not measured, only its product. All three are
> rows in [`STATUS.md`](STATUS.md)'s register.
>
> **The rest of the sentence stands unchanged.** The floating bus is not modelled, so it is not
> gradeable rather than ungraded — the suite's own groups 35–37 are excluded **by name** for exactly
> that reason — and progressive drawing and keyboard ghosting are in the same position. For those,
> *known-demanding software* is still the only instrument and it is still observation.

## Milestones

| | Goal | Gate |
|---|---|---|
| M1 | Registers, flags, un-prefixed opcodes | fuse green for un-prefixed |
| M2 | CB/ED/DD/FD prefixes | fuse green in full |
| M3 | Documented behaviour | **zexdoc passes** |
| M4 | Undocumented flags | **zexall passes** — CPU is done |
| M5 | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | **the gates in `crates/spectrum/tests/`** — boot **and the frame it lands on**, the 50 Hz line, the keyboard matrix, ROM write protection, contention magnitude and phase, the four-case I/O rule, and the hardware timing oracle. **Count them, do not quote a number:** `ls -1 crates/spectrum/tests/*.rs \| wc -l` |
| M6 | Snapshots (Z80/SNA) **and** TAP tape | **T1 + T2 + T3** — see below |
| M7 | 128: paging, second ROM, AY, contention per bank | **T1 + T2 + T3** ([`M7.md`](M7.md) Decision 5). *"128-only software runs"* is **T4** |
| M8 | WASM + macroquad | playable from a URL |

> **The M6 row said *"Snapshots (Z80/SNA) **or** TAP tape"* and *"a real game runs"*, and both
> halves are corrected here rather than quietly widened.** The design is
> [`M6.md`](M6.md), and its Decision 8 splits the milestone's evidence into four tiers:
>
> | Tier | What it is | Evidence class | Corpus |
> |---|---|---|---|
> | **T1** | the round trips, the truncation sweep, the codec property tests, the transcribed vectors | **proven** | none — built in code |
> | **T2** | the real ROM's `LD-BYTES` loads a synthetic tape through the `EAR` bit | **measured** | the committed 48K ROM |
> | **T3** | a program **we wrote** is loaded from tape by the ROM and then executes | **measured** | the same ROM |
> | T4 | a real game reaches its title screen; one of our `.z80` files opens elsewhere | **observed** | user-supplied, absent by default |
>
> **The gate is T1 + T2 + T3, and T3 is what replaces "a real game runs".** T4 is *observation*
> in this project's vocabulary and cannot be automated in a repository that may not carry games
> — `docs/ARCHITECTURE.md`'s own licensing note says why — so a milestone gated on it would be
> a gate that runs nowhere, which `STATUS.md` records three times already. It is **not done**,
> and that residue is stated rather than absorbed: T4 is the only tier that grades a turbo
> loader, and no `.tap` can carry one at any speed.
>
> The **or** was the smaller error and the more consequential one: snapshots and the tape are
> both built, and a row offering a choice between them would have let either alone count.
> `MACHINE.md` carries the same row and is not this document's to edit; it is flagged for
> whoever owns it.
>
> > **Discharged, and the flag is now stale in the other direction.** `MACHINE.md`'s M6 **and** M7
> > rows were corrected in the M6 merge (`0d3e7ef`), and its own correction block there says
> > *"`ARCHITECTURE.md`'s copy of the table is owned elsewhere and still carries the old wording for
> > **all three rows**"* — which was true of M5 and M7 and had already stopped being true of M6.
> > **Both files were partly corrected in the same session and each recorded the other as
> > untouched.** That is the propagation defect in its symmetrical form: not one document lagging,
> > but two documents each describing the other's *previous* state. The remaining two rows are
> > corrected above; `MACHINE.md`'s sentence about this file is corrected there.
>
> **M6 has since merged.** T3 shipped as a program written here, stored as a `.tap`, loaded by the
> real ROM's own `LD-BYTES` through the `EAR` bit, and executed — computing a value asserted to
> appear **nowhere in its own bytes**, so *"the data arrived"* and *"it ran"* are graded as separate
> claims. What that gate leaves ungraded is a list rather than a caveat, and it lives in
> [`STATUS.md`](STATUS.md)'s M6 section.
>
> **The M7 row is corrected at the same time so the next milestone does not inherit the mismatch**,
> which is the mistake this block records M6 making. [`M7.md`](M7.md) Decision 5 puts the 128 on the
> same four-tier scheme; *"128-only software runs"* is T4 there for the same reason it is here.

Performance is a non-goal. 3.5 MHz × 50 Hz ≈ 70,000 T-states per frame; a modern machine
does that thousands of times faster than real time. Optimise nothing until measured.

## Measured

Everything in this section is a measurement or an assembly inspection, not an estimate. **Every row
carries the command that produced it, the date it was last run, and what enforces it** — because the
one row that carried none of those was wrong for three milestones and nobody could tell.

Host for every figure below: Apple M3 Max (16 cores), macOS 25.6.0, rustc 1.98.0 (88d9e12ae
2026-08-18), `[profile.release]` as shipped. **Last re-run in full: 2026-09-01**, against
`crates/z80/src` at the checksums the codegen gate rebuilds from.

> **The instruction that used to sit here said "Re-run after M2 — it quadruples the opcode count on
> exactly the paths measured here."** It named the right milestone. Nothing enforced it, and **two**
> separate rows have now been dated to exactly M2 — the bounds-check count and the jump-table count —
> each surviving to M5 unchallenged. Three further rows turn out not to reproduce at all. That is the lesson this
> project already had a name for — *an unenforced instruction to re-measure is the same defect as an
> unrun gate* — recurring inside the section that gave the warning.
>
> **So the instruction has been replaced by a test.** `crates/z80/tests/codegen.rs` builds a fixed
> probe in release and asserts the deterministic rows below on every `cargo test`. The rows it does
> **not** assert are marked *ungated* and say what would settle them. Nothing here is left to a
> reader's diligence that a machine could hold instead.

### The verdicts, in one table

| Claim as written | Verdict | Enforced by |
|---|---|---|
| Batched tick: 507× flat / 294× paged | **cannot be reproduced** — batched `tick` exists in no tree; M1's hardening round removed it | nothing, and nothing can |
| Per-T-state tick, flat 64K bus: **329×** | **unresolved** — 296–308× measured under load; load can only depress a throughput figure | ungated (noisy) |
| Per-T-state tick, paged + contended: **145×** | **falsified** — 159–161× on the same bench and the same tree, *while loaded* | ungated (noisy) |
| Cost of `overflow-checks = true`: **5 %** | **cannot be reproduced** — 36–42 % on this benchmark on every tree including M1, once two load-contaminated runs are set aside | ungated (noisy) |
| Unproven bank index: **6.6 %** | **falsified** — the checks do vanish, the time does not move | ungated (noisy) |
| Contention arithmetic is **−21.5 points of the M7 decomposition** | **cannot be reproduced** — no "M7 decomposition" exists anywhere in this repository | nothing, and nothing can until one does |
| No `dyn` on the execute path | **holds** — 0 hits for `dyn`/`Box`/`Rc`/`Arc` in `crates/z80/src` | `the_execute_path_makes_no_indirect_call` |
| Indirect-branch (`blr`) count **0** | **subject-dependent** — 0 in a whole-program build, **1** in the `Cpu<Ula>` library object | gated for the probe only |
| `Bus::read` compiles to **one instruction** | **cannot be reproduced** — the original never recorded which bus | refused; see below |
| …and no call to any bus method | **subject-dependent** — true in the probe, **false** for `Cpu<Ula>` | `no_bus_method_survives_as_an_out_of_line_call` |
| Decode lowers to **two** tables (119 + 64) | **falsified at M2** — three now: 124 + 119 + 64. Attributed: 119 + 64 are `execute`'s and are M1's two; **124 is `execute_ed`'s**, the `ED` page M2 added | `decode_still_lowers_to_jump_tables` |
| The execute path allocates nothing | **holds**, and structurally: the crate is `no_std` with no `alloc` | `the_execute_path_allocates_nothing` |
| Register indexing: `panic_bounds_check` **15** in `Cpu<Ula>` | **reproduced exactly** — 15 = 7 + 6 + 2 | `bounds_checks_in_the_execute_path_have_not_moved` (at 7, its own probe) |
| Decision 5's bank masking: **0** in `crates/spectrum/src` | **reproduced exactly** — 0 | ungated; `crates/spectrum` owns its own gates |

### Throughput

```
$ cargo bench -p z80 --bench step          # divan; the counter column is T-states/s
```

Divide the counter by the Z80's 3.5 MHz for the realtime multiple. Measured 2026-09-01 with the
one-minute load average between **5.1 and 11.3 on 16 cores** — another agent was compiling in the
same tree throughout, and this could not be arranged otherwise. **Timing figures taken under load
are reported with the load, not laundered.**

| | claimed | fastest sample | median | on which tree |
|---|---|---|---|---|
| Flat 64K bus | 329× | 308.3× | 306.6× | `2157331` (M5) |
| Flat 64K bus | — | 302.0× | 300.9× | working tree |
| Paged + contended | 145× | **161.4×** | **160.6×** | `2157331` (M5) |
| Paged + contended | — | 159.4× | 157.8× | working tree |

**The flat row is unresolved and the paged row is falsified, and the asymmetry is the whole point.**
A busy machine can only make a throughput number *smaller*, so 306× against a claimed 329× is
exactly what a contended measurement of a true 329× looks like — it is not evidence of a
regression, and it is not confirmation either. But 160× against a claimed **145×** cannot be
explained that way: the measurement came out **11 % faster than the claim while the machine was
loaded**, and load does not run in that direction.

There is a second reading that needs no quiet machine at all. Both benches run in the same process
seconds apart, so **their ratio is load-cancelling**:

| | claimed | M1 | M5 | working tree |
|---|---|---|---|---|
| paged ÷ flat | **2.27** | 2.03 | 1.91 | 1.90 |

The shipped table says the paged bus costs 2.27× the flat one. It measures 1.90×, and it did not
measure 2.27 even at M1. Whatever produced 329/145 was not this benchmark in this shape.

**One orphan, carried forward rather than deleted.** This section used to end: *"The `spectrum`
crate's contention arithmetic is the one cost worth watching — it is the largest term (−21.5 points
of the M7 decomposition) and it is irreducible."* The engineering judgement is sound and is kept.
The number is **not reproducible**: no "M7 decomposition" exists in any file here, so there is
nothing for the −21.5 to be a share of. It is recorded as an orphan instead of
being quietly dropped, because a row that simply disappears is indistinguishable from a row nobody
re-read — which is the failure this whole section is a response to.

> **The recipe this paragraph published for checking itself did not work.** It read: *"The number
> is **not reproducible**: `grep -rn 'decomposition\|21\.5' docs/ crates/` returns nothing"*. It
> returns **six** hits — five inside this file, guaranteed the moment the orphan figure was
> quoted here, and one in `crates/spectrum/tests/contention_magnitude.rs`. The conclusion
> survives untouched: no "M7 decomposition" exists anywhere, so the −21.5 is a share of nothing.
> What failed is the *command*, and it failed in the section whose stated remedy is that
> re-running must be cheaper than re-deriving. **A published command that cannot return what it
> claims is worse than no command**, because it will be run once, contradicted, and then
> distrusted along with the finding it was supporting. The prose keeps the finding; the
> self-invalidating recipe is gone.

**A second orphan, and this one was carried forward by halves.** The `overflow-checks` block that
used to sit below recorded *"the release build of `Cpu<Ula>` carries **46** `panic_const_add_overflow`
sites and **15** `panic_bounds_check` sites."* The 15 survives, in Subject B of the codegen table.
The 46 does not: `grep -rn 'panic_const_add_overflow' docs/ crates/` now returns nothing. Every
other deletion in that pass is preserved with a correction attached — the old throughput table, the
5 % row, the 6.6 % row, the *"re-run after M2"* instruction, the retired Open rows — and this is the
one exception, so it is recorded here rather than left as a number that vanished. **It is not
re-measured**, because nothing in this pass needed it and re-taking a figure to fill a gap in a
table is how the unreproducible ones got here. Whoever wants it back:
`cargo rustc -p spectrum --release --lib -- --emit=asm -C debuginfo=2` and count the symbol.

**C1's price is still the right trade, and that argument does not depend on the disputed digits.**
Ticking once per T-state instead of once per batch is roughly 3× the calls, each doing contention
arithmetic; it buys the 88 contention points batching discarded, without which M5 and M7 cannot be
correct at all. At 160× real time the paged bus spends ~125 µs of a 20,000 µs frame — **0.6 % of the
budget** — which is why the benchmark is kept and the number is not chased.

### `overflow-checks`, and a row that was never reproducible

```
$ # flip `overflow-checks` in the workspace [profile.release], then:
$ cargo bench -p z80 --bench step
```

The claim was **5 % on the core, measured at M1**, and its method was never recorded. Re-run on the
fixed benchmark — which is **byte-identical in all five milestone trees and the working tree**, so
the workload is genuinely held constant — the flat bus costs this much more with checks on than off,
taking divan's fastest sample as the load-robust estimator:

| tree | flat bus | paged bus | load during the pair |
|---|---|---|---|
| `be865f6` M1 | **+40.6 %** | +8.4 % | 10.7–12.3 |
| `c7d63cb` M2 | +90.5 % | +10.9 % | 9.3–9.6 |
| `e3b4c58` M3 | +64.3 % | +4.2 % | 8.5–8.9 |
| `0ecf59c` M4 | +41.7 % | +6.7 % | 11.5–12.0 |
| `2157331` M5 | +36.3 % | +3.4 % | 11.1–11.3 |
| working tree | **+37.0 %** | +6.3 % | 9.5–10.0 |

**This is not a decay — it is a row that does not reproduce on the tree it was measured on.** M2 and
M3 ran at the highest load and are the two outliers; ignore them entirely and the remaining four
trees still cluster at **36–42 %**, seven to eight times the claimed 5 %, with M1 among them. The
honest verdict is therefore *cannot be reproduced* rather than *falsified at milestone N*: either
the 5 % measured something other than this benchmark, or it measured it differently, and the row
does not say which. **A measurement without its method cannot be defended, only re-taken.**

Note the second column, because it is the more useful one for M7: on the paged bus — the shape the
real machine has — the checks cost **3–8 %**. The core's arithmetic is a much smaller share of a
run that also does contention work on every T-state. Whatever the flat-bus figure is, the machine
pays the smaller one.

The rule itself (`overflow-checks = true` in release, so debug and release agree and every wrap is
an explicit `wrapping_*`) is a correctness decision and is **not** revisited by any of this. The
number attached to it was simply wrong.

### The unproven bank index: the checks are real, the cost is not

`benches/step.rs` now measures this instead of quoting it. `PagedRam<MASKED>` is one bus with one
line of difference — `banks[self.slots[slot]]` versus `banks[self.slots[slot] & 3]` — so
`paged_contended_bus` and `paged_contended_bus_masked` differ in nothing else.

**The masking does what it claims**, verified in the emitted assembly rather than assumed:

```
$ cargo rustc -p z80 --bench step --release -- --emit=asm -C debuginfo=2
```

All four `panic_bounds_check` sites attributable to `benches/step.rs` (`:228` in `read_byte` and
`fetch_opcode`, `:234` in `write_byte`) sit in symbols mangled `PagedRam` **`Kb0_`** — the
`MASKED = false` instantiation. The `Kb1_` instantiation has **none**.

And it buys nothing measurable. Fastest sample, four runs, load 6.1–8.2:

| | run 1 | run 2 | run 3 | run 4 |
|---|---|---|---|---|
| unproven | 147.5 µs | 150.7 µs | 146.6 µs | 149.4 µs |
| masked | 149.7 µs | 149.4 µs | 150.2 µs | 148.2 µs |

The masked variant is *slower* in three runs of four. The spread within one variant (±1.4 %) is
larger than the difference between them, and the sign is not stable. **6.6 % is falsified**: an
effect that size would be ~10 µs and it is not there. The four bounds-check sites on the access
paths are, on this core, free — which is worth knowing before anyone spends a newtype on them at
M7.

### Codegen — what the compiler actually emitted

The counts below name their subject, and this is not pedantry: **the same core has now measured 7,
10, 11 and 15 bounds checks** under four different probes. A bare integer is not a measurement.

**Two further things about that integer, both measured while settling the M2 question above, and
both of which make the subject line load-bearing rather than decorative.**

*It is a count of instructions, not of checks.* What the gate counts is `bl …panic_bounds_check`
call sites. LLVM tail-merges cold blocks, so several bounds checks share one call: in the
`PairBase`-proof variant, **five `b.hi` branches reach three call sites**. The number is exact,
reproducible and useful for pinning drift — and it is not the number of bounds checks, and was
never described as one.

*It is a property of the inliner, not only of the probe.* Holding the probe, the toolchain and
the source semantics fixed and changing only inlining decisions, the same M2 core measures
**0, 3, 5, 7 and 10** — the table in *When each falsified row broke* is that experiment. So the
gate pins a build, and a build is the probe **plus** the inliner's budget on the day. That is
worth pinning and it is not worth reading as a property of the code.

**Subject A — the gate's probe.** A `Cpu<NullBus>` where `NullBus` owns no array, driven through
`step`, `interrupt` and `nmi` in all three interrupt modes, built in release with fat LTO. It is
defined in `crates/z80/tests/codegen.rs` and rebuilt by it, so it cannot drift from the number.

```
$ cargo test -p z80 --test codegen
```

**Subject B — `Cpu<Ula>`, the real machine.** The library object, not a linked binary.

```
$ cargo rustc -p spectrum --release --lib -- --emit=asm -C debuginfo=2
$ grep -c panic_bounds_check target/release/deps/spectrum-*.s
15
```

| | Subject A (probe) | Subject B (`Cpu<Ula>`) |
|---|---|---|
| `panic_bounds_check` in `crates/z80/src` | **7** — 3 at `registers.rs:143`, 3 at `:148`, 1 in `instructions.rs` | **15** — 6 at `registers.rs:143`, 7 at `:148`, 2 in `instructions.rs` |
| `panic_bounds_check` in `crates/spectrum/src` | — | **0** |
| Allocator call sites | *not counted — see below* | *not counted — see below* |
| Indirect calls (`blr`) | **0** | **1**, at `instructions.rs:380` |
| Out-of-line `Bus` methods | **0** | **1** — `Ula::tick`, called 124 times |
| Decode jump tables | 124 + 119 + 64 | 124 + 119 + 64, all in `Cpu::<Ula>::dispatch` |

Two of those rows are new findings and both are the *same* defect as the bounds-check row — a count
published without its subject.

**Allocator call sites are deliberately not counted, and the reason is worth recording**, because
counting them is the obvious thing to do and it does not work. A probe that made `Cpu::step` build a
`Vec` on every call still measured **zero** allocator sites attributable to `crates/z80/src`: the
`bl` to the allocator lives inside `alloc`'s own code and carries `alloc`'s source location, not the
caller's. The count could not have failed, which makes it worse than no count. What replaces it is
strictly stronger — `crates/z80` is `#![cfg_attr(not(test), no_std)]` with no `extern crate alloc`,
so in every non-test build **allocation does not compile**. That is a property of all builds rather
than an observation about one, and it is what the gate asserts.

**The indirect call is real.** `instructions.rs:380` is `shuffle(self.regs.a(), memory)` inside
`rotate_digit`, which takes `shuffle: fn(u8, u8) -> (u8, u8)`. There are exactly two such parameters
in the crate — `rotate_digit` (`RRD`/`RLD`, so `ED`-prefixed, M2) and `rotate_a` (`RLCA`/`RRCA`/
`RLA`/`RRA`, **present since M1**). Whether LLVM specialises them away is a property of the build
unit: whole-program LTO devirtualises both, a single library object keeps one. So the row was never
a property of the code — it was a property of a build nobody wrote down. It is gated for Subject A,
where it is 0 and staying 0 is meaningful.

**No change is proposed, and saying so is part of the finding.** The two declarations are
`instructions.rs:376` and `instructions.rs:975`; the second predates the row itself. The shipped
emulator is linked with fat LTO, where both devirtualise and the cost is **zero**, and this
document's standing position is that performance is a non-goal and nothing is optimised until
measured. So this is a correction to what the row *claimed* — "0 indirect calls" as a property of
the core — and not a defect to fix. It is written down because the next person to read `1` in a
library object should find out that it is expected, is old, and costs nothing where it ships,
rather than opening an investigation.

**`Ula::tick` is not inlined**, despite carrying `#[inline]`, and is called 124 times in the library
object. The claim *"`#[inline]` makes cross-crate inlining happen"* holds for a linked binary and
does not hold for an intermediate object; the shipped emulator is a binary, so the machine gets the
inlining — but the row as written overstates what the annotation guarantees. **The design section
that made the claim now carries the correction too** — see the note under *Decision 1*, which is
where a reader looking for the rationale will arrive, and where the uncorrected sentence had
survived this row being written.

**`Bus::read` compiles to one instruction** is **refused rather than gated**. The instruction count
of a bus method is a property of *that bus*, and the original recorded neither which bus it used nor
the command. `NullBus::read` is a store and two ALU operations before inlining; `Ula::read` is a
page of paging logic. Neither is wrong, and once inlined and scheduled into `step` neither is
cleanly countable — which is the second reason this row cannot be restored, independent of the
first. The half of that row that *is* a property of the core —
no surviving call to a bus method — is gated.

#### When each falsified row broke, measured with one probe held fixed

The `Bus` trait and the `Cpu` API are **byte-identical across all five milestone merges**, so a
single unmodified probe compiles against every one of them and the comparison is like-for-like. Each
tree exported with `git archive`, built with the same pinned profile:

| Tree | bounds checks | jump tables | indirect calls | out-of-line bus methods |
|---|---|---|---|---|
| `be865f6` — M1 merge | **0** | **119 + 64** | 0 | 0 |
| `c7d63cb` — M2 merge | **7** | **124 + 119 + 64** | 0 | 0 |
| `e3b4c58` — M3 merge | 7 | 124 + 119 + 64 | 0 | 0 |
| `0ecf59c` — M4 merge | 7 | 124 + 119 + 64 | 0 | 0 |
| `2157331` — M5 | 7 | 124 + 119 + 64 | 0 | 0 |
| working tree | 7 | 124 + 119 + 64 | 0 | 0 |

**Both rows were true when written and both broke at M2**, the milestone the deleted instruction
named. The bisect agrees with the earlier one that used a different array-free probe and got 10
rather than 7 — **the date is reproducible, the integer is not**, which is exactly the distinction
the subject line exists to preserve.

#### Why M2 broke it — measured, and the inference on record was half wrong

This section used to end here, with an inference. It is quoted rather than deleted, because it
is the thing being corrected:

> *Why* M2 broke the bounds-check row remains **inferred, not measured**: `registers.rs` is
> essentially unchanged since M1, so the change must be upstream of it, and the prefix decoder now
> produces a runtime operand field plus a `DD`/`FD` base where M1 had constants. Reading the two
> decoders side by side is what would confirm it, and nobody has. Why M2 broke the jump-table row
> needs no inference: the third table is the prefix dispatch that M2 introduced.

The decoders have now been read side by side and, more usefully, **nine trees were built and
counted with one probe held fixed** — M1, M2 and the working tree, plus six trees differing from
one of those by a single deliberate edit. `git diff be865f6 c7d63cb -- crates/z80/src/registers.rs`
is **empty**: the file really is byte-identical, which is what made the question worth asking. The
M1 and M2 rows below reproduce the bisect above exactly (0 and 7), which is what makes the six
edited rows comparable to it.

```
$ git archive <rev> | tar -x -C <tree>            # per tree
$ cargo rustc --release --offline -- --emit=asm,link   # probe crate, PROBE_PROFILE, depends on <tree>/crates/z80
$ # count `bl …panic_bounds_check` whose current .loc names crates/z80/src
```

| Tree, and the one thing changed in it | `bl panic_bounds_check` | `.loc`s in `crates/z80/src` | jump tables | `crates/z80` functions left out of line |
|---|---|---|---|---|
| `be865f6` M1 | **0** | 618 | 119 + 64 | `apply_alu` |
| M1 **+ M2's whole `dispatch` loop** — a loop-carried `PairBase` ∈ {`HL`,`IX`,`IY`} | **0** | 767 | 119 + 64 | `apply_alu` |
| M1 + that loop **+ a second caller** for the two functions below | **0** | 790 | 119 + 64 | `apply_alu` |
| `c7d63cb` M2 | **7** | 1418 | 124 + 119 + 64 | `apply_alu`, `load_pair_absolute`, `store_pair_absolute` |
| working tree | **7** | 1427 | 124 + 119 + 64 | the same three |
| M2 + range proof at `PairBase::high`/`low` | **3** | 1410 | 124 + 119 + 64 | `apply_alu` |
| M2 + range proof at `self.regs[…]` | **0** | 1443 | 124 + 119 + 64 | — |
| M2 + `#[inline(always)]` on the two functions | **5** | 1408 | 124 + 119 + 64 | `dispatch` |
| M2 + `#[inline(never)]` on `execute`, `execute_cb`, `execute_ed` | **10** | 1435 | 124 + 119 + 64 | + those three |

**Four things follow, and the first two settle the question.**

**1. Every one of the seven is `Registers::get`/`set`'s `self.regs[index.0]`.** Proving the
index at that single expression takes the count to **0**; nothing else in the crate indexes that
array. The site attributed to `instructions.rs` is the same access — its `.loc` carries line 0
because it landed in a tail-merged cold block.

**2. Four of the seven are in `load_pair_absolute` and `store_pair_absolute`, and those two
functions are byte-identical between M1 and M2.** What changed is the *call graph* around them.
At M1 each had **one** call site, reached with `base = pair::HL` — a compile-time constant
propagated in from `step`. M2 gave each a **second** call site on the new `ED` page
(`instructions.rs:227`/`228`, `LD (nn),dd` and `LD dd,(nn)`) and made both arguments runtime
values — `index.base()` ∈ {`HL`,`IX`,`IY`} and `ed_pair(opcode)` ∈ {`BC`,`DE`,`HL`,`SP`}. LLVM
then declines to inline them, **in its own words**:

```
$ RUSTFLAGS="-Cllvm-args=-pass-remarks-missed=inline" cargo rustc --release --offline
remark: src/instructions.rs:120:26: '…load_pair_absolute…' not inlined into '…main'
        because too costly to inline (cost=615, threshold=525)
remark: src/instructions.rs:119:26: '…store_pair_absolute…' not inlined into '…main'
        because too costly to inline (cost=585, threshold=525)
```

One shared out-of-line copy of each survives, taking `PairBase` as a parameter. **In that copy
the parameter carries no provable range**, so `self.regs[index.0]` needs a check: two per
function, four in total. Proving the range at `PairBase::high`/`low` removes **exactly those
four** and leaves the other three, which is the same fact measured from the other end.

**3. So the recorded inference named the right operand and the wrong cause, and one of its two
clauses describes a change that never happened.** The `DD`/`FD` base *is* what the surviving
checks index with. But it is **not sufficient**: grafting M2's `dispatch` verbatim onto the M1
core — the same loop, the same three-valued runtime `PairBase` — leaves the count at **0**,
because on a core that size LLVM still inlines both functions and knows the base at every site.
Adding a second caller on top of that also leaves it at 0. And *"a runtime operand field"*
is simply false as a description of a change: `Operand::source`/`destination` index `OPERANDS`
with `opcode & 0x07` **at M1 as well**. The operand field was never a constant.

**4. Which leaves a residual that is honestly not settled.** The other three sites sit in the
fully-inlined body in `main`, are reachable (five `b.hi` branches target them), and are *not*
removed by proving the base's range — only by proving the index at the array. Every `RegIndex`
in the crate comes from an `index::*` constant or from `PairBase::high`/`low`, so on the source
alone they should already be provable, and they are not. **What would settle it:** LLVM's
value-range reasoning through the specific phis in the inlined `execute`, read with
`-Cllvm-args=-pass-remarks-analysis` or from the pre-codegen IR. Nobody has. **An explanation for
four of the seven is written here as an explanation for four of the seven** — the temptation to
let a good account of the majority stand in for the whole is exactly the move that produced the
inference this section is correcting.

**And the jump table needs no inference either, but the old sentence named the wrong construct.**
Outlining the three dispatch functions attributes the tables directly: **124 is
`Cpu::<B>::execute_ed`'s** — the `ED` page M2 added — while **119 + 64 are `Cpu::<B>::execute`'s**
and are the two M1 already had. It is not "the prefix dispatch": `dispatch` itself matches five
values and lowers to comparisons, and the `CB` page decodes arithmetically through
`CbOp::from_opcode` and builds no table at all.

### What the gate asserts, and what it deliberately does not

`crates/z80/tests/codegen.rs` runs on every `cargo test` — not `#[ignore]`d, not `--release`-only
(it builds its probe in release regardless of how the suite was invoked, because a gate that only
fires under a flag is a gate that nothing runs). Seven assertions, **every one of which has been
watched to fail** on a mutation proven to have landed:

| Assertion | Proven to bite by |
|---|---|
| `the_probe_actually_exercises_the_core` | the probe stops calling `step`; caught a real error first time out |
| `bounds_checks_in_the_execute_path_have_not_moved` | `self.regs[index.0 % REGISTER_COUNT]`, and independently by running against the M1 tree |
| `the_execute_path_allocates_nothing` | dropping `no_std` and allocating in `step` |
| `no_bus_method_survives_as_an_out_of_line_call` | `#[inline(never)]` on `NullBus::read` |
| `the_execute_path_makes_no_indirect_call` | `#[inline(never)]` on `rotate_a` |
| `decode_still_lowers_to_jump_tables` | running the unmodified gate against the M1 tree: `[119, 64]` |
| `the_probe_profile_still_matches_the_shipped_release_profile` | `opt-level = 3` → `2` in the workspace manifest |

**Two rows are refused rather than asserted, and refusing is the honest answer.**

*Every timing row.* Wall-clock figures move by more than the effects they measure on a shared
machine. Within a **single** 100-sample run this pass saw the flat bus range from 79.9 µs to
143.6 µs — a 1.8× spread, same binary, same second, load alone — and the same tree reported 295.7×
and 300.9× an hour apart.
A perf assertion that flaky gets muted within a month and then asserts nothing while looking like it
asserts something, which is the failure mode this whole section exists to document. They stay here,
labelled ungated, with the command that re-runs them.

*"`Bus::read` is one instruction."* Not stable and not meaningful: it is a property of the bus, and
no bus is named.

**Stability against the toolchain is handled by pinning, not by loosening.** The counts are asserted
as exact equalities, and `rust-toolchain.toml` says `channel = "stable"`, which floats — so a future
rustc will eventually turn one of them red. That is deliberate. The failure message reads the
running `rustc --version`, compares it to the pinned `1.98.0`, and says in the failure text whether
this is a re-pin or a real codegen change, with the command to reproduce. A bound (`<= 7`) would
have been quieter and would have let the number drift downwards unnoticed — and "silently drifted
while looking green" is the defect being fixed, not a cost worth paying to avoid a re-pin.

**What still enforces nothing:** the four timing rows, the `Cpu<Ula>` counts in Subject B (the gate
owns a probe, not the `spectrum` crate — `crates/spectrum` needs its own if it wants one), and the
`crates/spectrum/src` zero. Those are re-run by the commands printed above and by nothing else.

## Open items

**The register lives in [`STATUS.md`](STATUS.md), and only there.** This document describes the
design; `STATUS.md` records what is currently true. They were briefly duplicated, and within one
session they disagreed about four facts — the exact defect class that let the `tick` contract
survive unchallenged. One register, one owner.

## Licensing note

The Spectrum ROMs may live in this repository; game images may not, and the user supplies their
own. **The permission that makes the first half true is quoted in full — text, author, forum, date,
the four conditions it carries, its hedged scope, and the one gap still open in its sourcing — in
[`../testdata/README.md`](../testdata/README.md), and only there.**

> **This note used to be one of five unsourced copies of a licensing claim.** It read *"Amstrad has
> explicitly permitted redistribution of the Sinclair ROMs with emulators"*, and so did
> `.gitignore`, `README.md`, `M6.md` and `testdata/README.md` — five assertions, no quotation, no
> author, no date, no URL between them. That is the same shape as the defects
> [`STATUS.md`](STATUS.md) catalogues — a claimed protection with nothing behind it a reader can
> check — and worse in one specific way: **a wrong technical claim produces a bug; a wrong
> licensing claim produces a redistribution nobody was entitled to make.**
>
> It is not a summary problem, so it is not fixed by summarising better. For a licensing claim the
> **quotation is the thing relied on**, which makes it exactly the kind of fact that gets one home
> and links from everywhere else — the rule this document already applies to the open register.
> Read the scope there before adding a ROM: it is a hedged 1999 usenet answer, not a licence grant
> with a schedule, and the ZX80, ZX81 and Interface 1/2 ROMs are **disclaimed** rather than merely
> omitted.
