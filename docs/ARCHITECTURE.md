# ZX Spectrum emulator — architecture

Target: a correct ZX Spectrum **48K and 128** emulator, CPU-first, correctness-gated.

## Project rules

1. **Latest stable libraries only.** Every dependency is pinned to the newest stable
   release, verified against crates.io. No legacy versions, no `*`.
2. **`unsafe_code = "forbid"`** in every crate **except one**. If `unsafe` looks necessary
   for speed, it is premature — this machine runs thousands of times faster than a 3.5 MHz
   Z80. The exception is not about speed and is not an exception to that: `crates/page` holds
   the browser FFI, where `unsafe` is unavoidable in kind rather than optional for
   performance. See *Crate boundaries*.
3. **`overflow-checks = true` even in release.** The Z80 is meant to wrap; every wrap
   is written as an explicit `wrapping_*` call, so debug and release must agree.
   A silent wrap is a bug, not a feature.
4. Correctness has an external oracle up to M4. After that it is observation. Do not
   mix the two modes.

> **Rule 2 said "in every crate" for a milestone after it stopped being true, and that is the
> worst line in this document to have carried a stale absolute.** A reader acting on it believes
> there is no `unsafe` anywhere and does not go looking — which is precisely the confidence the
> rule is supposed to earn rather than assert. The rule *survives*, and it survives in a stronger
> form than it had: one crate, one file, five blocks, each with a `SAFETY:` comment, with the
> counts asserted by a test and `crates/frontend`'s own `forbid` asserted as literal text by
> another. Naming the exception is what makes the rule checkable; leaving it unnamed made it a
> slogan.
>
> **Rule 4 is also narrower than it now needs to be, and the correction already exists further
> down rather than here.** *"After M4 it is observation"* stopped being true for **contention**
> at M5: `crates/spectrum/tests/timing_oracle.rs` grades this machine against T-state counts
> measured on real Spectrums, which is a tier-1 oracle by the *Testing* section's own definition
> — a number to compare, not a picture to squint at. The rest of the rule stands, and stands
> exactly: the floating bus, progressive drawing and keyboard ghosting are not modelled, so they
> are **not gradeable** rather than ungraded. The full statement of what that oracle does and
> does not settle is in *Testing*, and only there.

## Crate boundaries

```
crates/z80/         pure CPU core. No memory, no I/O, no allocation, no std needed.
crates/spectrum/    the machine: paged memory, ULA, contention, keyboard, joystick,
                    tape, snapshots, screen, and the two things that make a noise.
crates/frontend/    macroquad: the window, the device, the mix, the keymap, pacing.
crates/page/        the browser's half of the frontend's host seam. FFI, and nothing else.
crates/testsupport/ the corpus-absence policy every gate shares. Never published.
```

The CPU does not own memory. That is the decision everything else hangs on.

> **This block listed three crates for two milestones after there were five**, and the two it
> omitted are the two whose existence is an argument rather than an arrangement. Both are small,
> and being small is the point of both.
>
> **`crates/page` exists because `unsafe_code = "forbid"` cannot be relaxed from the inside.**
> A browser build needs five imported JavaScript functions and one export, and every one of
> those declarations is `unsafe` — including the export, which in edition 2024 is
> `#[unsafe(no_mangle)] extern "C"`. `forbid` — unlike `deny` — refuses an `#[allow]` beneath
> it, so the choice was never *"an attribute on one item"*; it was **relax a whole crate's
> posture, or confine the FFI to a crate a reviewer can read in one sitting**. `docs/M8.md`
> Decision 4 took the second. So `crates/page` is *the only crate in this workspace that is not
> `unsafe_code = "forbid"`*, and the exception is confined to one file: five `unsafe` blocks,
> two `extern` blocks, one export, each with a `SAFETY:` comment.
>
> Two different tests hold the two halves of that, and it is worth saying which does which.
> `crates/page/tests/unsafe_inventory.rs` asserts the **counts**, because every block is behind
> `#[cfg(target_arch = "wasm32")]` and a lint fires only on code the current target compiles —
> so `cargo clippy` on this machine reports the crate clean whatever those blocks contain. And
> `crates/frontend/tests/portability.rs`'s `this_crate_still_forbids_unsafe` holds
> **`crates/frontend`'s** `forbid`, reading its own `Cargo.toml` through
> `include_str!("../Cargo.toml")` and asserting the literal, on the grounds that the alternative
> to `forbid` is not a compile error but a review nobody is scheduled to perform.
>
> **Its scope is one crate, and this document said four.** The test's name says *this crate*; the
> sentence above it once promoted it to *"every other crate still forbids"*, which it has never
> checked and cannot — `include_str!` resolves one path. `unsafe_code = "forbid"` is also declared
> by `crates/z80`, `crates/spectrum` and `crates/testsupport`, and `rg unsafe_code` finds **nothing
> asserting any of the three**: one manifest of four is guarded, and the gap is stated here rather
> than closed, because writing the missing assertion is not this pass's to do.
>
> **This is worth pausing on, because of when it happened.** The promotion was introduced by the
> pass that was correcting a *different* phantom-guardian citation, within the hour, in the same
> paragraph, by an author who had the family's definition in front of them. The defect is not
> carelessness about tests — it is that prose summarising a gate drifts *upward* by default. A
> sentence naming what a test covers is easier to write one notch broader than the test, nothing
> in a green run contradicts it, and the broader claim is the one a reader remembers. **The class
> reproduces during its own repair**, which is the argument for mechanical anchors over careful
> writing: careful writing is what produced this line.
>
> The seam pays a second dividend that was not the reason for it and is now the larger one.
> Because both of `page`'s entry points **compile and behave on every target** — the query
> string is empty off `wasm32`, the download answers `Handoff::NoPage` — `crates/frontend`
> contains **no `#[cfg(target…)]` at all**, and `crates/frontend/tests/portability.rs` asserts
> that absence. That absence is the whole reason a test run on this machine says anything about
> a browser: the code `cargo test -p frontend` runs and the code a browser runs are the same
> code. It is a property of every build rather than an observation about one, which is the
> stronger of the two claims this project knows how to make.
>
> **`crates/testsupport` is a crate because a policy that lives in two places is two policies.**
> Several gates read corpora — third-party snapshots, tapes, games — that this repository may
> not carry, and each needs the same answer to *"the corpus is absent"*. Sharing it through a
> dev-dependency rather than through a copied helper is what stops one gate quietly resolving
> absence as success while its neighbour resolves it as failure.

## Decision 1 — the bus is a consumer-defined trait, ticked once per T-state, with the address

```rust
/// The published Z80 machine-cycle lengths. Exported, because they are the decoding key
/// for the call stream and not an implementation detail a machine happens to need.
pub const OPCODE_FETCH_T_STATES: u8 = 4;
pub const MEMORY_ACCESS_T_STATES: u8 = 3;
pub const PORT_ACCESS_T_STATES: u8 = 4;

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn in_port(&mut self, port: u16) -> u8;
    fn out_port(&mut self, port: u16, val: u8);

    /// The opcode byte of an M1 cycle. Defaulted, so implementing it is optional.
    fn fetch(&mut self, addr: u16) -> u8 { self.read(addr) }

    /// One machine cycle, `t_states` long, `IR` on the bus, no transfer — the acknowledge
    /// of an accepted interrupt. Defaulted, on the `fetch` precedent.
    fn acknowledge(&mut self, addr: u16, t_states: u8) { let _ = (addr, t_states); }

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

> **And the arity in that sentence went stale the very next milestone, which is the more
> interesting failure of the two.** The trait has **seven** methods and **two** defaulted:
> M7 added `acknowledge`, and the block above now carries it. It also now carries the three
> exported cycle lengths, which were omitted here as though they were an implementation
> detail and are not — they are the **decoding key for the call stream**, and an implementor
> cannot honour the contract without them.
>
> **The instructive part is the second half of the sentence: "is gated in
> `crates/z80/tests/codegen.rs`."** It is not, and never was. That file asserts seven things
> and the method *count* is none of them — what it gates is the other clause,
> *generic rather than `Box<dyn Bus>`*, through `the_execute_path_makes_no_indirect_call` and
> `no_bus_method_survives_as_an_out_of_line_call`. So the arity was a claim with a **named
> guardian that did not guard it**, which is strictly worse than an ungated claim: an ungated
> number invites a check, and one that cites a gate deflects it. This document has a name for
> the shape — *an unenforced instruction to re-measure is the same defect as an unrun gate* —
> and this is its sharper form, an instruction that was never issued while reading as though
> it had been.

Seven methods, **two of them defaulted**. `fetch` was the first default, and it exists because M1 is the
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

### `acknowledge`, the second default, and why it is not a `fetch`

M7 added the trait's seventh method on exactly the precedent above, and it is worth separating
the two arguments it rests on, because only one of them is about interrupts.

**The structural one.** The Z80 asserts `/M1` together with `/IORQ` in place of `/MREQ` during an
interrupt acknowledge: the interrupting device puts the byte on the data bus and **no memory is
read**. Routing that through `fetch` would report a memory cycle that never happened, at an
address the machine would be entitled to serve out of its own map. Without any callback it
reached the bus as a bare run of `tick`s with no transfer to open a cycle — indistinguishable
from that many *separate* internal cycles, each contending on its own account. The hardware
performs **one** cycle there and owes one stall; the machine charged seven. That is not a
magnitude anybody had to guess at. It is the structural question *one cycle or seven*, and
[`Z80-REFERENCE.md`](Z80-REFERENCE.md) had already answered it.

**The one about where a number lives.** The two acknowledge lengths — seven T-states after an
`INT`, five after an `NMI` — stay **private** to `crates/z80`, and the reason they were private
is preserved rather than overturned: exporting a length hands out a number a `Bus` cannot act
on. A callback falsifies that premise, because now there *is* something to act on, so the figure
arrives as the call's own argument. Which is the same rule the three exported cycle lengths obey
from the other side: a machine that had to re-transcribe either would be keeping a second copy of
this crate's knowledge, with nothing to notice when the copies disagreed.

**And the non-breaking claim is no longer a story about one afternoon — it is a standing
property that `cargo test` rebuilds.** The codegen gate's probe bus implements `read`, `write`,
`in_port`, `out_port` and `tick`, and **neither defaulted method**. It compiles, it is driven
through `step`, `interrupt` and `nmi` in all three interrupt modes, and it is rebuilt on every
run. So *"a downstream implementor kept working untouched"* stopped being a fact about M5's
`ula.rs` and became a fact about a subject that cannot quietly opt in behind the claim's back.

**What the defaults buy is compilation, not accuracy, and the difference is the whole of
`MACHINE.md`.** Both degrade to something plausible — `fetch` to a three-T-state read,
`acknowledge` to nothing at all — so a `Bus` can be silently wrong about contention while
compiling clean under `deny(warnings)`. That is the price of making the additions non-breaking,
it was paid deliberately twice, and it is why *"implementing it is optional"* in the trait's own
documentation must be read as optional for the compiler and mandatory for a machine that means
to keep time.

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
> divergence costs nothing, and not only because our contention model cannot see it: the hardware
> applies contention at **T1 of a machine cycle and nowhere else**, so M1's second half is never
> separately contended on any address. (An earlier revision of this block derived that from the
> Z80's `/WAIT` sampling rule and asserted the Spectrum contends via `/WAIT`; the sources disagree
> about which pin the ULA pulls, and the corrected sourcing — including the snow effect, which is
> the measured half — is in [`Z80-REFERENCE.md`](Z80-REFERENCE.md).) Measured as well as derived:
> driving `PC, PC, IR, IR` leaves 290/290,
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

> **Two of those clauses are wrong about the shipped crate, and this document already knew
> it — in a different section, about the same functions, without either sentence being
> reconciled to the other.** *Why M2 broke it*, below, says plainly that `dispatch` *"matches
> five values and lowers to comparisons"* and that the `CB` page *"decodes arithmetically
> through `CbOp::from_opcode` and builds no table at all"*. Both statements are correct. Both
> contradict *"sub-matches per CB/ED/DD/FD prefix"* and *"the compiler builds a jump table
> anyway"* three hundred lines above them. A document that corrects itself in the section where
> the measurement lives and leaves the design section standing has not corrected itself; it has
> acquired a second opinion, and a reader arriving at the design section — which is where
> somebody looking for the rationale arrives — gets the retired one.
>
> What is actually there, and it is a better argument than the one it replaces. **Two of the
> four prefix pages are matches and two are arithmetic.** `execute` lowers to the 119- and
> 64-entry tables M1 already had, and `execute_ed` to the 124-entry table M2 added — those are
> the three the codegen gate counts. The `CB` page instead reads its own operand fields: two
> bits select one of four groups, three bits select a shift or a bit number, and 256 encodings
> come out of four table lookups with no arm per opcode. `dispatch` — the thing the old
> sentence called *"sub-matches per prefix"* — is five values and lowers to comparisons.
>
> **The deeper correction is about which file.** There is no large match in `decode.rs` at all.
> That module is *field extraction*: an un-prefixed opcode is `xx yyy zzz`, so `LD r,r'` is
> `01 ddd sss` and `ALU A,r` is `10 ooo sss`, and those two blocks between them cover a quarter
> of the map with two range arms. The exhaustive match on the opcode byte lives in
> `instructions.rs`. The accurate form of this decision is therefore: **decoding is arithmetic
> on bit fields, dispatch is one exhaustive match, and neither is a pointer table** — which
> keeps the original's real claim (no indirect call per instruction, exhaustiveness checked at
> compile time) and stops crediting the compiler for a table it does not always build.

## Decision 4 — flags as per-class helpers

`add8`, `sub8`, `adc16`, `sbc16`, `rotate`, `bit`, `daa` — one implementation each,
called from every opcode of that class. Never inline flag logic per opcode: that breeds
200 copies of the same rule, each with its own bug.

> **`rotate` is not one of them and never was.** Five of the seven names are real; `bit`,
> `adc16` and `sbc16` live in a `prefixed` submodule and the other two at the top. The rotate
> *class* is eight functions rather than one — four accumulator forms sharing one flag rule and
> four `CB` forms sharing another — over four shared bit-movement primitives, and the split is
> the design rather than an omission: `RLCA` and `RLC r` move the same bits and set **different
> flags**, so one helper covering both would have to take a discriminator and would be the
> per-opcode branch this decision exists to refuse.
>
> Three classes the list omits are the ones a reader most needs, because they are where the
> undocumented bits actually get decided: **`block_transfer`, `block_compare` and `block_io`**,
> the flag rules of `LDIR`/`CPIR`/`INIR` and their families. And the placement rule is worth
> stating, since it is what keeps the module from becoming a directory of unrelated functions:
> a helper sits beside the rules it **shares arithmetic with**, not beside its callers —
> `sbc16` and `add16` have to agree about where the 16-bit half-carry lives, so they are
> neighbours even though their opcodes are pages apart.
>
> **MEMPTR is the exception that proves the rule, and it is not in `flags.rs` at all.** The
> undocumented bits of `BIT n,(HL)` come from an internal register the Zilog documentation
> never mentions, so `flags` cannot hold its rules without becoming a second place that knows
> about CPU state. Instead `Cpu` owns `wz`, exactly **one private writer** touches it, and its
> value reaches the flag helper as an ordinary `u8` parameter. The single-writer rule is an
> auditing device rather than an encapsulation habit: the rules are undocumented, they are
> scattered across three dozen handlers, and `rg 'set_memptr'` **is** the enumeration of them.

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
pub struct Memory {
    ram: Box<[[u8; PAGE_SIZE]; BANK_COUNT]>,   // 8 banks, heap: 128 KB is not a struct field
    rom: Box<[[u8; PAGE_SIZE]; ROM_COUNT]>,    // exactly 2. A 48K fills one and ignores the other
    slots: [Slot; SLOT_COUNT],                 // Rom(i) | Bank(i) — derived, never assigned
    contended: [bool; BANK_COUNT],             // 48K: bank 5. 128: banks 1, 3, 5, 7 — derived
    model: Model,
    paging_port: u8,                           // the SSOT: the map, the screen, and the lock
}
```

> **That block is the struct M7 built. It replaces a five-field sketch of what M5 planned, and
> only one of the five survived unchanged.** The sketch read:
>
> ```rust
> struct Memory {
>     banks: [[u8; 0x4000]; 8],
>     roms: Vec<[u8; 0x4000]>,   // 48K: one. 128: two (editor + 48 BASIC).
>     slots: [Slot; 4],           // Rom(i) | Bank(i)
>     contended: [bool; 8],       // 48K: bank 5 only. 128: banks 1, 3, 5, 7.
>     paging_locked: bool,        // port 0x7FFD bit 5, until reset
> }
> ```
>
> **The deleted field is the interesting one.** There is **no `paging_locked: bool`**, and the
> crate rejects it in as many words: keeping a `paging_locked` and a `screen_bank` beside the
> port byte would be *three representations of one datum that can disagree*. The lock is bit 5
> of `paging_port`, read where it is needed. A sketch in an architecture document is exactly
> where that defect is cheapest to introduce and most expensive to notice, because nothing
> compiles it — the field was fine as a plan and would have been a bug as a struct.
>
> The rest, briefly, because each difference carries an argument. `roms` is not a `Vec`: the
> page count is **fixed at two**, sized in at M5 so that M7 added a writer and not a
> reallocation. Both arrays are **boxed**, so 160 KB sits on the heap rather than inside every
> value that holds a machine. `slots` and `contended` kept their shapes but changed status —
> they are **caches of derived values**, `slots` from `paging_port` and `contended` from
> `model`, never assigned from anywhere else, which is the same one-source discipline stated
> three different ways.
>
> `model` is the field the sketch could not have predicted, and its existence is a finding
> rather than an oversight. M7's central result is that **`Memory` needs no model check at
> all**: a 48K *is* paging-port value `0x20`, its map derives from that byte, and its inability
> to page is exactly the lock bit already being set — so `write_paging_port` returns early on
> the lock and asks nothing about the machine. That equation is a **compile-time assertion**
> against the transcribed 48K map, not a paragraph. But three things differ between the two
> machines that are **not** functions of `0x7FFD`: which banks the ULA contends, the frame's
> geometry, and which banks exist at all. The port byte is the wrong thing to ask about any of
> them, and the alternative to one discriminator is three unrelated fields that can disagree.
>
> **The hot-path consequence is the point of the whole decision and it is checkable.**
> `Memory::is_contended` is byte-for-byte what it was before the 128 existed. The 128 changes
> the *contents* of the `contended` array and not one branch of the function that reads it —
> which is what *"48K is a special case of the 128"* has to mean if it is to mean anything, and
> is why the failure it prevents is worth naming: getting this wrong does not crash, it makes a
> demo tear three years later.

### The clock is parameterised the same way, and by value

The memory map was never the only thing that differs. A 128's frame is 70908 T-states against a
48K's 69888, its first contended T-state is 14361 against 14335, and its CPU runs at 3.5469 MHz
rather than 3.5. So there is a `Timing` — seven fields, all private, **no constructor**, and
exactly two associated constants that are its whole population. A `Clock` carries one **by
value**.

Two properties follow that a `Model`-branch inside the clock would not have. The consistency
check (`lines × T-states per line == frame`, the contended span inside the frame, the interrupt
window inside it) is a `const` assertion over a **closed** set, which is only total because
nothing outside the file can build an eighth combination. And the 48K's published constants —
`T_STATES_PER_FRAME`, `FIRST_CONTENDED_T_STATE` and the rest — are now **projections** of
`Timing::SPECTRUM_48K` rather than transcriptions beside it, so the compiler holds the single
source rather than a reviewer.

The contention *pattern* is deliberately **not** a field. Both sources give both machines the
identical `[6, 5, 4, 3, 2, 1, 0, 0]`, and a shared constant states that sameness once — where a
per-model copy would state it twice and be a place for a transcription to drift. That is a
judgement about which claim is easier to get wrong, and it is written down as one.

**The evidence classes underneath those numbers are not equal, and quoting them alike is the
mistake this section exists to prevent.** 14335 is **measured**, against hardware, by the timing
oracle. 70908 is **derived**, and unusually well — three independent lineages, `228 × 311`
closing it arithmetically. 14361 is **transcribed** from one reference with one descendant
repeating it and citing nothing. And the 128's interrupt window is shipped as a **labelled
hold**: 32 is kept, it predicts that the 128 timing suite goes red, and the prediction is
asserted so that the day somebody runs that suite the disagreement is waiting for them rather
than being discovered as a surprise.

### 48K vs 128 — what differs

| | 48K | 128 |
|---|---|---|
| Memory | flat 64K | 8 banks × 16K, slot `0xC000` pageable |
| ROM | one, 16K | two: 128 editor + 48 BASIC |
| Paging port | — | `0x7FFD`: bits 0–2 bank, bit 3 screen, bit 4 ROM, **bit 5 lock until reset** |
| Screen | bank 5 | bank 5 or shadow bank 7 |
| Sound | beeper | the same beeper, **plus an AY-3-8912**: `0xFFFD` select and read, `0xBFFD` write |
| Frame | 69888 T | **70908 T** |
| Contention | address range `0x4000–0x7FFF` | **property of the bank** (1, 3, 5, 7) in *any* slot |

That last row is the real complication: on the 128 contention must be checked per access
against whichever bank is currently paged in, not by address range.

**Out of scope:** +2A/+3. They add port `0x1FFD`, all-RAM configurations and yet another
contention pattern, for a sliver of extra software. 48K + 128 covers what matters.

## Decision 6 — a port decode is a mask, and every device that matches answers

The ULA answers every port with A0 clear. The paging port answers every address with A1 and
A15 clear. The AY is selected by A15 set and A1 clear, with A14 choosing which of its two
register ports. A Kempston answers with A5, A6 and A7 clear. **Not one of those is an
equality against a canonical address**, and writing them as masks is what separates a correct
decode from a lucky one: `0x7FFD` is one member of a large family, and `IN A,(0x00)` reads a
real Kempston exactly as `IN A,(0x1F)` does, because `A0`–`A4` are not wired to anything on the
board.

The consequence is the part worth arguing, because it decides the shape of the code. **These
families overlap, and on the hardware every device that matches drives the bus.** Any address
with A0, A1 and A15 clear is claimed by the ULA *and* the paging port; any address with A0 and
A1 clear and A15, A14 set is claimed by the ULA *and* the AY; every even port up to `0x1E` is
claimed by the ULA *and* a fitted Kempston. So `out_port` is a run of **independent `if`s and
never a `match` on the port** — a `match` would silently pick one device and stop the other
answering, which is the defect that surfaces years later as *"this game works on one emulator
and not another"* rather than as a failure.

Reads cannot do that, because a read has to return one byte. There the overlap is resolved by
**fixing a priority and calling it a ruling**: the narrower decode wins, so the AY is asked
before the ULA's floating-bus fallback and the joystick before the keyboard. The rationale is
that giving priority to the device consulting three address lines rather than one changes the
fewest addresses — and the ordering is load-bearing in a way that is invisible if you get it
wrong. `0xFFFD` has A0 set, so it falls into the *"not a ULA port, therefore floating bus"* arm;
an AY arm placed after that arm never runs, and the machine reads as *"the sound chip is
write-only"*. Nothing in reach exercises any of these collisions, because every published
address has A0 the other way — which is exactly why they are written down and asserted at
compile time rather than left to be rediscovered.

**The three decodes do not have the same evidence behind them, and M7 changed two of the three
verdicts.** The paging port's is **primary**: the Sinclair *Servicing Manual* §4.12.11 states it
from the circuit — `BANK` decoded from `IORQ` and read-or-write with *"ZA1 and ZA15 low"*. The
Kempston's is **primary** too, from the Issue 4 (1989) schematic: a `74LS138` with A5, A6, A7 on
its select inputs, corroborated by an independent redraw of a compatible board. And the AY's,
which this project spent a milestone calling its own least-supported claim, now has a primary
witness as well — §5.6.3, which gives it as **two stages rather than one pattern**:

```text
  PSG  = IORQ · (RD + WR) · (ZA1 = 0) · (ZA15 = 1)     <- only A15 and A1 select the chip
  BDIR = PSG · /RD ,   BC1 = PSG · A14                 <- A14 steers; it does not gate
```

Two things fall out of those equations that appear in no table in the manual: at A15 = 1 and
A1 = 0 the chip is **always** engaged, so there is no no-match state, and **`A0` is not in the
AY's decode at all**. The implemented masks were already right; what changed is that they can
now say why.

> **Two of those verdicts are stale in the source, in the direction that matters.**
> `crates/spectrum/src/ula.rs`'s `AY_PORT_MASK` still calls itself *"the least-supported claim
> in M7"* and quotes *"no source was found stating which lines decode `0xFFFD` and `0xBFFD`"*;
> `crates/spectrum/src/lib.rs`'s coverage table still says the AY decode has **nothing** behind
> it and that the Kempston mask *"matches the canonical address's low byte and deliberately
> claims nothing about address lines"* — which is a description of a mask the crate no longer
> carries. Both were true when written and stopped being true in the commit that found the
> sources. They are recorded here rather than corrected there because this document does not
> own those files; the corrections belong with whoever does.
>
> Note which way the error runs. Neither is a machine that behaves wrongly — the masks are
> right. Both are claims that **understate their own evidence**, which is the rarer direction
> and the harder one to catch: nothing goes red, and a sentence disclaiming a source it now has
> reads as modesty rather than as an error.

## Decision 7 — sound is generated late, and on an edge rather than on the clock

`Ula::tick` is the hottest function in the emulator and **sound costs it one `bool` test**. That
is possible because the chip's output at any instant is a pure function of its registers and the
time since they were last written, so the generator can be run at only three kinds of moment:
when the guest does something that would otherwise be lost — the speaker bit **changing**, or a
write to an AY register — when the **tape's own signal flips**, and when a consumer asks for the
samples. Between those it does nothing at all.

> **This heading said *"never on the hot path"*, and the section said *"not a branch, not a load,
> not a field"* over *"two kinds of moment"*, both guest-initiated. M8 added the tape's `EAR`
> signal to the mix — a real Spectrum's `EAR` socket feeds the amplifier as well as the ULA's
> input, which is why a loading tape is audible — and the tape's signal is driven by the
> **machine's own passage of time**, not by anything the guest does. That is a third kind of
> moment and it is unavoidable; what was avoidable was *where the test lived*.
>
> **The first wiring polled it, and it cost +23%.** `Ula::advance` read `Tape::level()` on every
> elapsed T-state and let the audio module compare — the shape `set_beeper` uses, and the wrong
> one here, because the *timestamp* argument is evaluated at the call site before any guard can
> discard it, and `Clock::t_states` is a 64-bit multiply-add that under this workspace's
> `overflow-checks = true` compiles to two multiplies and two branch-to-panic edges. 69,888 calls
> a frame, on a machine with no tape in the drive. Measured on `benches/frame.rs`: **+19% to +30%
> across all eleven cases, mean +23%.**
>
> `Tape::advance` already knew when the level moved — `finish_pulse` is the only thing that
> writes it, and it runs right there — so it says so, and the timestamp is computed only when
> there is something to timestamp. The recovery is complete and the invariant is stronger than
> before, because the level test now lives with the level. **The lesson is not "keep sound off
> the hot path" — it is that an eagerly-evaluated argument defeats a guard inside the callee**,
> which is a Rust evaluation-order fact and not an audio one.
>
> No benchmark in this workspace played a tape, which is why a regression on the tape path
> shipped green; `benches/frame.rs` now has a `tape_playing_48k` case.

The trade is smaller than it looks in both directions, and that is why it is defensible rather
than merely cheap. Total work is proportional to emulated time either way, so generating eagerly
saves nothing and spends a per-T-state branch. And a border write — which is what nearly every
write to `0xFE` is — costs **one comparison**, because the speaker setter takes the level and
compares rather than trusting the caller to know whether anything moved. The comparison belongs
where the state is.

**The property that makes late generation legitimate rather than a shortcut is that when the
generator runs cannot change what it produces.** Rendering a span in one call and rendering it
in two hundred arbitrary pieces yield identical samples, and that is asserted rather than
argued. Without it, every frame hash over the audio would silently be grading the *consumer's
call pattern*.

Two shape decisions in the sample carry real weight:

**The sources stay apart.** A `Sample` is `{ channels: [u16; 3], beeper: u16, tape: u16 }` — five
sources in three fields, and *this line listed two of the three, from before M8 added the tape* —
and nothing in `crates/spectrum` ever adds them together. `Sample` is `#[non_exhaustive]`, which
is what made the third field additive rather than breaking, and the structural guarantee that no
mix can happen here is stronger than a promise: **`crates/spectrum/src` contains zero `f32` and
zero `f64`**, so a weighted sum does not have a type to be written in. Two independent rulings
converge on it. The AY's own
gate must not be falsifiable by the beeper landing, or it goes red for a reason unrelated to
what it grades and gets muted. And the mix belongs downstream, in `crates/frontend`, because the
sum is **irreversible** — stereo panning, which is how a 128 is conventionally presented, needs
the three channels a mixdown would have destroyed. Summing the AY's own channels would be a mix
too, so this crate does not do that either.

**Every sample is a mean, not a reading.** A sample is the T-state-weighted average of its
source over the window it covers. That costs one multiply-add per state change and buys the one
property a beeper needs: **a pulse shorter than a sample period arrives attenuated rather than
missing**. Point-sampling drops it entirely and silently, and 48K beeper music is written as
exactly those loops.

The grid is the chip's own rather than an invented one: one sample every 32 T-states, which is
exactly two of the AY's internal steps, so no generator ever needs a fractional accumulator. The
rate is deliberately **not exposed as a number**, because on a 128 it is not an integer —
110840.625 Hz — and a consumer resampling to a host rate wants the ratio, not a rounded
frequency. The same arithmetic is why a 128's frame is not a whole number of sample periods and
its per-frame count alternates, so a consumer must read the length it is handed. `take_samples`
renders up to now, hands over a borrowed slice of a buffer allocated once at construction, and
resets — so a frontend draining once a frame allocates nothing, and one that drains less often
is **told how many samples it lost** rather than hearing an unexplained gap.

## Decision 8 — the joystick is a port, and that is the whole reason it exists

A Spectrum has no arrow keys. Nothing on the membrane means *move left*, so a game reaches a
control one of three ways: the cursor keys, which are `CAPS SHIFT` chorded with `5`–`8`;
arbitrary letters the author picked; or a Kempston, which is an interface **on the bus** rather
than anything on the membrane. Only the third can be driven without colliding with something the
game also reads on the keyboard — and a mapping cannot know which keys that is in general.

The sharpest demonstration is not hypothetical. **Manic Miner reads `LD BC,0x7EFE` for its jump
key**, and `B = 0x7E` holds A8 and A15 low *together*, merging two half-rows into one scan. So
holding `CAPS SHIFT` to walk left makes Willy jump continuously — the machine behaving exactly
correctly while a keyboard mapping is wrong. A port has no such failure mode available to it,
because no keyboard scan can reach it.

Two model decisions follow from the hardware and are worth stating because both are easy to get
backwards. The five switches are **active high**, the exact inverse of the membrane's active-low
— a model that used one convention for both reads `0x1F` idle, which is every direction and fire
held down forever. And there is **no interlock**: left and right at once is a state a real
switch box can be forced into and some games test for, so refusing it here would model a machine
nobody built.

**And the joystick's most interesting property is one it gets from Decision 1 rather than from
its own decode.** A cheap Kempston clone that decodes A5 alone and ignores the read strobe drives
the data bus during the Z80's interrupt-acknowledge cycle — `/IORQ` is asserted there with
neither `/RD` nor `/WR` — and the CPU takes the joystick's byte as its IM 2 vector. That defect
**cannot happen on this machine, and not because of anything the joystick does**: an acknowledge
reaches the bus through `Bus::acknowledge` and never through `in_port`. The `/RD` and `/M1` terms
of the schematic are satisfied by the *shape of the trait*. It is written down here because the
implication runs the other way too: a future bus that routed an acknowledge through `in_port`
would reintroduce a hardware defect this machine currently cannot have.

## Decision 9 — a tape is a pulse train, and that was a bet on the next format

The internal form of a tape is `Vec<u32>`: **half-period lengths in T-states**, in playback
order. Not a block list, not a header, not a flag byte, not a timing table. Playing it is three
pieces of state and one transition — during `pulses[i]` the signal holds a level, and at the end
of it the level flips.

The reason is a claim about the *next* format rather than about this one. **A `.tap` cannot
represent a custom loader's tape at all**: it is block data with the ROM's timings implied, and
nothing in it can say *"this loader uses 700-T-state bits"*. `.tzx` exists for exactly that and
is what most commercial games ship as. A block-list internal form would have made `.tzx` a
rewrite of the tape subsystem; a pulse train makes it a second converter with the machine side
untouched.

**That prediction was cashed in, and it held.** `tzx::parse` exports one function, takes bytes
and a model, and returns a `Tape`. Loops are unrolled, calls are inlined and jumps are followed
**at parse time**, so the `Tape` that reaches the ULA does not know `.tzx` exists and there is no
runtime interpretation to get wrong. `Tape` gained no field, no method and no variant. The
ULA's five tape-touching items — the field, `insert_tape`, `tape_mut`, `advance`, `ear_bit` —
are unchanged.

> **Two qualifications, because the claim as usually stated is slightly wider than the
> evidence.** The module's *error* enum went from 2 variants to 12: `Tape` gained nothing, and
> `tape::Error` gained ten. It is `#[non_exhaustive]`, so this was additive rather than
> breaking, but *"no variant"* reads as covering the module's error type and does not.
>
> And *"`ula.rs` and `lib.rs` changed by zero lines"* cannot be checked at the granularity it is
> usually quoted at: `.tzx` landed inside a commit that also carried sound, the AY, the joystick
> and the browser build, and those two files show +288 and +135 in it. Filtering both diffs for
> tape-related lines leaves only **doc comments**. So the claim is true of the code and
> unverifiable from the commit — which is worth saying plainly, because a structural prediction
> is exactly the kind of claim that deserves better evidence than a stat line that happens to
> agree with it.

The seam between the tape and the machine is **two calls** — the tape is told that time passed,
and the tape is asked what level it is driving — and where the first one comes from is the
decision that shapes everything. Contention means the clock does **not** advance one T-state at
a time, so a tape driven from `Bus::tick` alone would run slow by exactly the contention a
loader suffers, silently, because nothing else would move. The ULA therefore has one private
`advance` that moves the clock **and** the tape together, and every call site in the file routes
through it, so the two cannot drift. The other call is the read: bit 6 of a `0xFE` read is the
tape's level, and **nothing supplies a byte to the CPU by any other route.**

That last clause is the milestone's real content. The cheap alternative — watch for `PC`
reaching the ROM's `LD-BYTES`, write the block straight into the buffer, set the flags and
return — is fifty lines and works today. It is refused because **it would make the milestone's
gate grade the trap**: a trap bypasses the ULA, the contention model, the frame clock, the
interrupt window and the port decode, so *"a real game loads"* would mean *"the injection
works"*. A debugging trap is not forbidden forever, and the two rules if one ever lands are
written down: off by default, and the tape gates assert that it is off.

## Decision 10 — a file becomes a value; the value meets the machine

Every format this emulator reads — `.tap`, `.tzx`, `.z80`, `.sna` — is parsed by a function of
its **bytes** and nothing else. No filesystem, no clock, no machine. A snapshot parser produces a
`Snapshot`, which is a neutral description of a machine's state; applying that description is the
*other* half and lives with the machine. Two things follow, and both are why the split is there.

**The parsers are exhaustively testable, fuzzable, and unchanged under wasm**, because there is
nothing to stand up in order to run one. And the canonical type is the **machine's state, not
the richest file format**. `.z80` version 3 is the richest, and adopting it as the canonical form would leak
every one of its quirks inward until the machine was storing a file — its page numbering, its
hardware-mode byte, its sixteenth AY register that the chip does not have. The memory image is
therefore keyed by **bank**, which is also the only key that survives paging: a 48K's page
numbers are neither contiguous nor derivable from the 128's rule, and on a 128 five of eight
banks have no address at any given moment while remaining part of the machine.

**Applying a snapshot charges no machine cycle**, and that is an invariant rather than a
nicety. A restore goes through the ULA's own setters and never through `Bus::out_port`, because
a port cycle advances the clock by its contention stall — and the tape advances with the clock,
so a restore performed through the bus would **move the tape head**. Restoring is not elapsed
time. Three things deliberately survive a load for the same reason and each is labelled a
convention rather than a measurement, since no format carries the field: the machine's uptime in
frames, the ROM, and the tape, because loading a snapshot does not eject a cassette.

### Guest bytes are the one hostile surface, and the gates are unusual

The release profile is `panic = "abort"`, so a panic on a malformed file is not an exception a
caller can catch — it is the process dying, and `catch_unwind` is not a backstop that exists.
With `unsafe_code = "forbid"` the routes to one are few enough to enumerate: a slice index,
arithmetic overflow, and an explicit `unwrap`/`expect`/`panic!`. Which makes the property look
*checkable by reading*, and so the tape and snapshot modules **scan their own source text** and
assert it — no indexing expression anywhere, none of the panicking calls, in either module's
production half.

Three details make that a gate rather than a gesture. The files are **listed, not globbed**,
because a file that quietly stopped being scanned would be indistinguishable from a file with
nothing to find. Each scanner has its own positive **and** negative cases, so it cannot be a
scanner that finds nothing while asserting nothing is there. And there are deliberately **two
scanners rather than one shared one**, because a single bug in a shared helper would turn both
gates green at once.

**And the enumeration was incomplete, which is the more useful thing to know about it than that
it exists.** `split_at` panics, and it is neither an index expression nor an `unwrap` — so it is
a *fourth* route, and neither scanner could see it. Three sites in the tape path went through it
before anybody noticed. The lesson is not that the gate is bad; it is that a gate built by
enumerating its targets is exactly as complete as the enumeration, and the enumeration is the
part nothing checks.

One more asymmetry worth carrying. `.tzx` termination is **not** structural the way every other
loop here is: its jump block revisits a block without consuming input, so there is no
decreasing quantity to argue from. The ceiling on it is therefore called a **budget** and not
dressed up as a proof — and it is a second ceiling, separate from the one bounding memory,
because a block can execute without emitting a pulse and the pulse ceiling would never fire.

## Decision 11 — the border is drawn as the beam painted it; the bitmap is not

A frame is rendered from the screen **as it stands when `render` is called**, so software that
rewrites attributes or the bitmap partway down a frame — multicolour, Nirvana sprites — is drawn
as though the last value had applied all frame. That boundary is unchanged and deliberate:
progressive drawing needs the frame's write history keyed by T-state over 6912 bytes, which is a
different data structure and a different verification story with no oracle behind it.

**The border is now the exception, and it moved for one reason.** A tape load is the one place a
mid-frame write is what a person is actually looking at, and a loading screen drawn in a single
colour is *visibly* wrong rather than subtly so. So the ULA keeps a `BorderTrace`: the colour in
effect at the moment each rendered row began.

The asymmetry is the argument, and it is about cost rather than taste. The border's history is
**one slot per rendered row**, and a guest cannot create rows — so there is no event list, no
allocation sized by guest behaviour, no drop policy, and therefore no failing case for a policy
to have. It is bounded by construction, which is stronger than a bound that is enforced. The
bitmap's history has none of those properties. **The cheap half being done is not the expensive
half being started.**

Three further things a reader should not have to rediscover.

**The record and the current colour are one field, not two.** The colour showing now and the log
of where it changed are one datum at two resolutions, so `BorderTrace` owns both and there is no
pair to disagree — the same rule the paging port obeys one module away.

**The row mapping is derived from `Timing` and from nothing else**, because a second
T-state-to-beam-position model would be a second thing that has to agree with contention's, and
two mappings that must agree is the defect class this project keeps catching. Vertically the
frame buffer maps to the hardware exactly; horizontally it does not, since the rendered border
is a uniform 32 pixels a side where the real one is wider than it is tall. So a
T-state-to-*column* mapping would be inventing precision the buffer cannot carry, and a
T-state-to-*row* mapping is not. What that cannot show is stated rather than rounded away: a
border change *within* a line — the eight-to-twenty-four-T-state rewrites of a
border-multicolour demo — all land in the same row, and the last before the row begins is the
one it gets.

**And the record serves the frame just finished as well as the one running**, which is the
difference between the feature working and not. A frontend's loop is `run_frame(); render();`,
and `run_frame` returns the instant the frame *counter* advances — so at the moment it renders,
the machine stands a few T-states into the next frame and the record describes the previous one.
A rule of *"this frame only"* shows a frontend a uniform border **every time** while passing any
test that renders mid-frame. That was not reasoned out in advance. It was a gate going red.

## Decision 12 — the frontend keeps the window, and a lint level created a crate

`crates/frontend` holds everything with a decision in it — which key, which colour, how many
frames, which file — and the binary's own `main` is held to plumbing: poll, upload, draw, await.
The split is drawn along *testability*: what is left in `main` is what needs a GPU and a window
and cannot run headless. That is also why `zx-shot` exists as a **second binary** rather than a
flag: `#[macroquad::main]` opens the window before the function body runs, so a screenshot mode
reached from inside it would not be headless at all. `zx-shot` never calls `miniquad::start`, and
it drives the *same* pipeline the window drives — keymap, `run_frame`, `render`, palette — so
that a picture taken in CI is evidence about the window. A screenshot produced by code the window
does not run would prove nothing about it.

**Three things the frontend owns because `crates/spectrum` must not.** The *mix* of the AY
against the beeper — the beeper weighted **2.65×**, which is not taste but the ratio of the two
resistors the 128's own board sums them through, asserted against the resistances rather than
against the implementation. The *resampling* to a host rate, because if the machine crate
resampled, its output would become a function of the machine it ran on — 44,100 here, 48,000
there — and a frame hash that moves with the hardware is not a gate at all. And the *pacing*:
elapsed time converted into whole frames owed, at most four of them run per tick, and **the rest
counted as lost rather than carried**. Both alternatives are wrong in
ways worth naming — one emulated frame per displayed frame runs 20 % fast on a 60 Hz monitor and
reports nothing; unbounded catch-up tries to run the whole backlog in one tick, which takes
longer than a tick, which grows the backlog. The second is self-amplifying: one slow frame
becomes a freeze. The count of dropped frames is the point, not a diagnostic.

The keyboard is a flat table of bindings rather than a `match`, for a reason that is about the
host rather than about style: the frame loop must **enumerate** the host keys it should ask
about, because `is_key_down` answers one key at a time and there is no *"what is held"* query.
It is rebuilt from scratch every frame — never edge-tracked — because a key left held when the
host stops reporting it is indistinguishable from a broken emulator.

**And the arrows are a choice rather than a mapping, which is a fact about the games.** Six
titles were disassembled to settle it and **three of them read no fixed keys at all**; they
redefine. So there is no single mapping to find, and the emulator's job is to be able to deliver
any key so a game's own redefine menu works. `ARROW_SCHEMES` is that choice, one keystroke away,
with the current one on screen. The default sends the bare digits **and** the Kempston port,
because the port cannot collide with the membrane, so a scheme sending both reaches strictly more
titles at no cost to either. What it does *not* send is the `CAPS SHIFT` chord — that is the
editor's cursor keys, it is what the legend prints, it stays one keypress away, and it is the
mapping that makes Willy jump.

> **That default is where this decision was learned rather than designed.** The chord was
> shipped as the default with the merged-half-row hazard already documented in the same file and
> a test already asserting that the cursor scheme trips Manic Miner's jump read. The gate graded
> the *scheme* and not the *choice of default*, and the reason recorded for the choice was
> continuity with a previous build — which is not something anybody had asked for. A predicted
> hazard placed behind a key the user has to know to press is an unshipped fix.

**The browser half of all of this is `crates/page`**, and why that is a crate rather than an
attribute is argued once under *Crate boundaries* and not repeated here. What belongs in this
decision is the consequence for the frontend, which is the larger half and was not the reason.

Because `page`'s entry points compile and behave on **every** target, `crates/frontend` contains
no `#[cfg(target…)]` at all — and the absence is asserted, together with its counterweight: a
second assertion that the target-conditional code **is** in `crates/page`, without which
deleting that crate entirely would leave the first test greener than ever. The one thing the
pair cannot see is stated in the test itself: a `#[cfg]`-free crate can still behave differently
on two targets, through a dependency, through pointer width, through whatever a browser's WebGL
does with a texture upload. It grades that this crate does not **branch** on the target, which
is a real and narrow property, and not that the two targets agree.

One convention runs through the whole seam and is worth stating once, because it is a property
of the boundary rather than of any function. **Zero must never mean success.** miniquad replaces
an import the page did not register with a stub that returns `undefined`, which crosses the wasm
ABI as `0` — so a page served without its JavaScript half would call the download function, get
success, and save nothing. Started is therefore `1`, every other code is a refusal, and the
frontend's save path routes to the filesystem **only** on the answer that means *there is no
browser* — a browser that refused is not retried against a filesystem that does not exist.

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
> Richard Butler's 48K timing test suite — a `.z80` carrying 37 machine-code groups, 34 over the
> documented instruction set and three sharing one program, and two tables of results *measured on
> real Spectrums* — and reports **70 hardware rows, 0 disagreements**. It is a tier-1 oracle by this
> section's own definition: a number to compare, not a picture to squint at. Sixteen mutations
> bound it, each verified to have landed before its verdict was trusted;
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
> gradeable rather than ungraded: the suite's own groups 36 and 37 are excluded **by name** for
> exactly that reason, and group 35 — the third of that block, and the one the oracle does grade —
> reaches a verdict only because `FLOATING_BUS_BYTE` is `0xFF` and so never charges the
> bus-dependent cycles. Progressive drawing and keyboard ghosting are in the same position. For
> those, *known-demanding software* is still the only instrument and it is still observation.

## Milestones

| | Goal | Gate |
|---|---|---|
| M1 | Registers, flags, un-prefixed opcodes | fuse green for un-prefixed |
| M2 | CB/ED/DD/FD prefixes | fuse green in full |
| M3 | Documented behaviour | **zexdoc passes** |
| M4 | Undocumented flags | **zexall passes** — CPU is done |
| M5 | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | **the gates in `crates/spectrum/tests/`** — boot **and the frame it lands on**, the 50 Hz line, the keyboard matrix, ROM write protection, contention magnitude and phase, the four-case I/O rule, and the hardware timing oracle. **Count them, do not quote a number:** `ls -1 crates/spectrum/tests/*.rs \| wc -l` |
| M6 | Snapshots (Z80/SNA) **and** TAP tape | **T1 + T2 + T3** — see below |
| M7 | 128: paging, second ROM, AY, contention per bank — **and the beeper both machines already had**, and the Kempston port | **T1 + T2 + T3** ([`M7.md`](M7.md) Decision 5). *"128-only software runs"* is **T4** |
| M8 | WASM + macroquad | playable from a URL |

> **Two things landed that this table has no row for, and in one case that is the finding rather
> than the omission.** `.tzx` has no milestone of its own because it did not need one: Decision 9
> chose a representation three milestones earlier precisely so that the next tape format would be
> a converter, and a converter is not a milestone. A row for it would misrepresent the work as
> larger than it was — which is the opposite of the usual failure and worth as much.
>
> The **beeper** is the other, and it has no row because it was deferred rather than scheduled.
> It is bit 4 of a `0xFE` write — a ULA output on **both** machines, with nothing to do with the
> 128 — and M5 dropped it on the stated grounds that a later milestone would want it. The
> comment saying so then sat in `ula.rs` for a milestone describing itself as *an open finding,
> not a deferral*. What settled it was a ruling that being **audible** does not make a ULA output
> the frontend's business, which is the same boundary Decision 7 draws for the AY and draws in
> the same direction. M7's goal cell is widened above rather than a row being invented, because
> that is where the work landed whatever its natural home was.

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
> **game**. It read *"a turbo loader"* until `crates/spectrum/tests/tzx_turbo_load.rs` landed,
> which grades the loader on every clone because this repository wrote both the tape and the
> loader that reads it; what is left at T4 is a commercial title, and one may not be committed.
> The premise beneath is unchanged — no `.tap` can carry a turbo loader at any speed, and most
> commercial titles are turbo-loaded.
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
$ cargo bench -p spectrum                  # the real machine: a real Cpu<Ula>, a real frame
```

> **The second command did not exist when this section was written, and its absence was
> published as a correction elsewhere before it was fixed.** `docs/M7.md` named
> `cargo bench -p z80 --bench step` as the way to measure a change to the *contention
> constants*, then withdrew it in the open: that bench measures a CPU against a bus M7 does not
> touch, and *"there is no benchmark in this workspace that exercises `spectrum::Ula`"* — so
> naming a command that exists and measures something else is worse than naming none, because
> it runs, it produces numbers, and every one of them is about a different bus. `benches/frame.rs`
> is the bench that was named as the thing that would settle it. Every case runs **one real
> frame** of a real `Spectrum` and the cases differ only in what the guest does, so a gap between
> two rows is attributable to that and to nothing else.
>
> It exists because M7's sound half makes a claim about the hot path, and a performance claim
> with no command behind it is the same defect one milestone later. **What it measured, on
> 2026-09-01: `quiet_48k` is 148.0 µs before and after sound and the border record landed** —
> 0.74 % of a 20,000 µs frame — which is the measured form of *"neither is on the hot path"*.
> Drawing bands rather than one colour costs **3.5 µs**, taken as a difference of differences so
> that the cost of rendering at all cancels. The three heaviest cases are upper bounds rather
> than workloads and are labelled so: a music driver writes the AY about fourteen times a
> *frame*, not two thousand.
>
> **Re-measured 2026-09-03**, after M8's tape half landed — with the same-day +23 % regression
> and its reversal that Decision 7's correction carries — and after `Memory::is_contended` was
> folded into a per-slot cache rebuilt on the paging write: `quiet_48k` is **138.8 µs — 0.69 %
> of a frame** — and a cassette actually turning costs **+0.8 µs** over the same frame drained
> silent (`tape_playing_48k` 147.1 µs against `drained_48k` 146.3 µs; lowest median of three
> runs, one-minute load average 12–17 on 16 cores). The 2026-09-01 figures stand as that day's
> record.

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
| Indirect calls (`blr`) | **0** | **1**, at `instructions.rs:475` — `shuffle(self.regs.a(), memory)` |
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

**The indirect call is real.** `instructions.rs:475` is `shuffle(self.regs.a(), memory)` inside
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
