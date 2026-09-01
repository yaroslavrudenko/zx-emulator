//! Gate: the whole contention model against **T-state counts measured on real Spectrums**.
//!
//! This is `docs/MACHINE.md`'s verification item 2 — *"a known-timing test program … a number
//! to compare, not a picture to squint at"* — and it is the first thing in `crates/spectrum`
//! whose expectations were not written by this project.
//!
//! # What every other contention gate cannot do
//!
//! `contention_magnitude.rs`, `io_contention.rs`, `block_contention.rs` and
//! `prefix_chain_contention.rs` all assert **relative to**
//! [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE], and each of them
//! says so in its own header. They are exact, they are hand-derived, and every one of them
//! survives that constant being wrong, because they position the machine *by* it. The delay
//! pattern `[6,5,4,3,2,1,0,0]` and the four-case I/O rule are in the same position: they are
//! the emulator community's figures, transcribed here as expectations, so a gate written from
//! them cannot discover that they are wrong.
//!
//! This file positions the machine by the **interrupt** instead, and then lets a program the
//! Spectrum itself runs count how much work fits into one frame. Nothing here is expressed in
//! terms of 14335, of the pattern, or of the four cases. Every expected number is read out of
//! the corpus.
//!
//! # The corpus, and why it is an oracle rather than a second opinion
//!
//! `timing_tests_48k_v1.0.z80` is Richard Butler's 48K timing test suite (ZXSpectrum4.net,
//! 2010). It is a `.z80` snapshot carrying a BASIC front end, **37** machine-code test groups —
//! 34 covering the documented instruction set, and three more that share one program and whose
//! readings depend on the floating bus — and, the part that matters, **two tables of expected
//! results, at `0xE200` and `0xE400`**.
//!
//! Each test group is a block of instructions executed in a loop until the frame interrupt
//! fires. The interrupt handler records three numbers: the refresh register `R`, the number of
//! loop iterations completed, and the stack pointer. Every group is run twice — once from
//! uncontended memory where it sits, and once copied to `0x5B00`, in the screen bank, where
//! the ULA charges it. **The measured quantity is therefore how much a whole frame of
//! contention costs**, integrated over ~10<sup>2</sup>–10<sup>3</sup> iterations, which is a
//! quantity no assertion in this workspace has ever compared against anything external.
//!
//! The authors state the provenance in as many words: *"In order to get the correct results we
//! ran the tests on real Spectrums"*, and their results database is explicitly hardware-only —
//! *"Only submit results from genuine hardware no emulators!"*. **Twenty-eight machines have been
//! submitted by nine independent people; twenty-five of them classify, 17 as `TYPE1` and 8 as
//! `TYPE2`, spanning board issues 2, 3, 3B, 4A, 4B and 6A.** The three that do not are two Inves
//! Spanish clones — a machine with no contended memory at all — and one issue 1 board recorded as
//! returning zeros. The counts were parsed from the results table rather than read off it.
//!
//! **That is the anti-circularity argument, and it is worth stating precisely, because a
//! community test suite is usually somebody else's derivation.** The authors also say their own
//! emulator *"works perfectly and passes all the tests"* — which, read alone, is exactly the
//! shape that would make this a circular oracle. It is not, for a reason that does not depend
//! on believing them: the tables encode **two** hardware behaviours, and the machines in the
//! results database sort into those two classes rather than scattering. A table fitted to an
//! emulator has no reason to predict a second class of machine it does not implement, and no
//! reason for twenty-five real machines to sort cleanly into the two.
//!
//! # Early and late timing — what this corpus can and cannot settle
//!
//! The two tables are the suite's own `TYPE1 (Early)` and `TYPE2 (Late)`, and the strings are
//! in the snapshot's BASIC. They differ by one T-state in when the display's contention window
//! opens relative to the interrupt, and **69 of the 73 populated rows differ between them**, so
//! the difference is not subtle from the outside.
//!
//! So the corpus **cannot** tell this project whether `FIRST_CONTENDED_T_STATE` should be
//! 14335 or one more: it has an answer for both, because real Spectrums have both. What it
//! **can** do, and what nothing else here does, is:
//!
//! - **refuse a machine that is neither.** Measured, not argued: moving the window to 14336 while
//!   leaving the interrupt alone scores 7 mismatches against `TYPE1` and **64 against `TYPE2`** —
//!   it does not become the other machine, it stops being either. The mutation table is in
//!   `docs/MACHINE.md`;
//! - say **which** of the two real behaviours this emulator reproduces, as a fact rather than
//!   an intention;
//! - grade the pattern, the phase and the I/O rule **jointly**, integrated over a frame,
//!   against numbers this project did not write.
//!
//! > **Two of the three bullets above are about the graded rows, and the detection run does
//! > none of that work.** An earlier draft said the tables' 120-T-state gap in `R` on the
//! > detection run meant "a model whose pattern or four-case rule is wrong matches nothing", and
//! > that is false: the detection group is `JP (HL)` in **uncontended** memory, so contention
//! > cannot reach it. It was disproved by this file's own mutation run — at 14336 the detection
//! > still reports `TYPE1` and only the contended rows move. The sentence is recorded rather than
//! > deleted because it is the shape `docs/STATUS.md` catalogues twice already: an argument that
//! > names real quantities, predicts the right verdict, and identifies the wrong cause.
//!
//! The authors add a caveat that is itself a finding: a *cold* machine can report late and then
//! report early once warm. The one T-state is not a stable identity of a board, so "which is
//! correct" may not be a question with a single hardware answer — see `docs/MACHINE.md`.
//!
//! # What the detection group measures, and where the 120 comes from
//!
//! The paragraph above leaves the mechanism open and `docs/M7.md` said in as many words that it
//! *"is not established here and should not be guessed at"*. **It is established now**, by
//! disassembling the group and its setup out of the snapshot and then measuring the result. The
//! summary first, because it is short: **`R` counts four-T-state loop iterations, and the two
//! classes differ by exactly eight of them — 32 T-states — because on a `TYPE2` machine the
//! program takes one *extra, nested* interrupt during its own synchronisation, and on a `TYPE1`
//! machine it does not.** The gap is not 120 of anything; 120 is −8 wrapped into seven bits.
//!
//! ## The loop, and what `R` is a count of
//!
//! Group 0's code is four instructions at `0xC13C`, which the suite copies to `0xDDDD` and enters:
//! `LD BC,0` / `LD (0xEF01),BC` / **`LD HL,0xC146`** / `JP (HL)`. The address is *absolute*, so
//! both the uncontended copy at `0xDDDD` and the contended copy at `0x5B00` jump to the same
//! `0xC146`, where the byte is `E9` — `JP (HL)` with `HL` = `0xC146`, jumping to itself, four
//! T-states and one `R` increment per iteration, in uncontended RAM. That is why group 0 has an
//! uncontended row only, and it is why contention cannot reach it. The suite's BASIC confirms the
//! reading it wants: line 805 is `IF x=2 AND y=0 AND z=49478`, and `49478` is `0xC146` — **both
//! classes are interrupted inside this loop**, so the only thing that differs is how many
//! iterations fitted.
//!
//! `R` is reset by `LD R,A` at `0xC11D` with `A` = 0, and read by `LD A,R` as the first
//! instruction of the measuring handler at `0xBBBB`. Counting the M1 cycles between them gives a
//! closed form, and every term is checkable in the disassembly:
//!
//! ```text
//! recorded R = (11 + N) mod 128
//!   N  iterations of JP (HL)
//!   7  M1 cycles from LD R,A to the first JP (HL)   (LD A,n; JP (HL); then the group's four)
//!   1  the interrupt acknowledge cycle
//!   1  the JP 0xBBBB patched over the IM2 vector at 0xF4F4
//!   2  the LD A,R itself — it reads R after its own two fetches
//! ```
//!
//! `N` follows from one number: the frame T-state at which the loop is entered. The loop's period
//! divides the frame, so `N = (69888 − entry) / 4`, and:
//!
//! | entry | `N` | recorded `R` | |
//! |---|---|---|---|
//! | **292** | 17399 | **2** | `TYPE1 (Early)` — the table's row, and this machine's |
//! | **324** | 17391 | **122** | `TYPE2 (Late)` — the table's row |
//!
//! 292 is not fitted: it is 19 (the IM2 response) + 35 (the handler at `0xF5F5`) + 194 (the setup
//! block at `0xC0F3`) + 44 (group 0's own four instructions), each summed from the disassembly.
//! An instruction-level trace of this machine puts the first `JP (HL)` at frame T-state **292**
//! with `R` = 7, counts **17399** iterations, and takes the interrupt at T-state 0 of the next
//! frame — every number above, measured rather than argued.
//!
//! ## The suite synchronises itself, which is why a phase difference cannot explain 120
//!
//! Before measuring, the program locks its own phase, and the locking is what makes the 32
//! T-states the *only* thing left that can differ. `EI` / `HALT` at `0xC0AC`; a 252-iteration
//! delay loop at `0xC0B1` of 277 T-states each (69804 T, just under a frame); then a deliberate
//! race — `XOR A` / `INC A` / `DI` sit at the loop's tail, and the frame interrupt lands on one
//! of those boundaries. Landing on `XOR A` leaves `Z` set, the handler's `JP NZ` is not taken, and
//! the delay loop **restarts one T-state later relative to `/INT`**; landing on `INC A` leaves
//! `NZ` and the program proceeds. So the loop creeps by one T-state per frame until it lands, and
//! from there `HALT` puts the next acceptance exactly on the interrupt. This machine's trace shows
//! the whole sequence: interrupt 1 at T-state 0 on the `HALT`, interrupt 2 at T-state **3** on
//! `INC A`, interrupt 3 at T-state **0** on the second `HALT`. **The measurement therefore starts
//! at the same phase on any machine**, and a one-T-state shift of the whole interrupt — window and
//! all — moves nothing, which is exactly what the mutation run reported and what made the 120 look
//! unexplainable.
//!
//! ## The amplifier: a nested interrupt worth exactly 32 T-states
//!
//! The handler the program installs at `0xF5F5` is nine bytes copied from `0xC130`, and three of
//! them are dead weight until you time them:
//!
//! ```text
//! INC C   4      19 T-states of IM2 response, then
//! EI      4      4 + 4 + 5 = 13, so the first moment the CPU can see /INT again
//! RET C   5      is exactly 19 + 13 = 32 T-states after it accepted the last one
//! NOP NOP NOP
//! JP 0xC0F3
//! ```
//!
//! **`EI` does not re-enable interrupts until after the following instruction**, so the first
//! `/INT` sample after the handler re-enables them falls on the end of `RET C` — at **exactly
//! T-state 32**, exactly the far edge of the 48K's 32-T-state `/INT` window. That is not a
//! coincidence anyone should have to accept on faith: `INC C` and `RET C` are otherwise pointless
//! (the carry flag is clear, so `RET C` never returns), and they are the two instructions that
//! make 13 out of the 32.
//!
//! If `/INT` has already been released, nothing happens and the program reaches `0xC0F3` at
//! T-state 54. If it is still asserted, a **second interrupt is accepted right there**: 19
//! T-states of response, then `INC C` + `EI` + `RET C` again before the same fall-through — 19 +
//! 13 = **32 T-states**, and `0xC0F3` is reached at 86 instead of 54. The nested push is invisible
//! afterwards because `0xC0F4` is `LD SP,(0xC13A)`, which is why the tables' `sp` column reads
//! `49478` in **both** classes rather than differing.
//!
//! **32 T-states is eight iterations of a four-T-state loop, and `2 + 8 × (−1) ≡ 122 (mod 128)`.**
//!
//! ## Measured, not just derived
//!
//! Making the window one T-state longer is how this integer-T-state model expresses "the CPU still
//! sees `/INT` at exactly +32". Mutating [`INTERRUPT_T_STATES`][spectrum::timing::INTERRUPT_T_STATES]
//! in a scratch clone, with [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE]
//! left alone:
//!
//! | window | detection reading | class | rows disagreeing with `TYPE1` | with `TYPE2` |
//! |---|---|---|---|---|
//! | **32** (this machine) | `R=2 loop=0 sp=49478` | `TYPE1` | **0 of 68** | 64 of 68 |
//! | **33** | `R=122 loop=0 sp=49478` | `TYPE2` | 64 of 68 | **3 of 68** |
//! | **34** | `R=122 loop=0 sp=49478` | `TYPE2` | 64 of 68 | **3 of 68** |
//! | 33, with contention at 14336 | `R=122 loop=0 sp=49478` | `TYPE2` | 64 of 68 | 4 of 68 |
//! | 32, with contention at 14336 | `R=2 loop=0 sp=49478` | `TYPE1` | 7 of 68 | 64 of 68 |
//!
//! Three things follow, and the third is a limitation rather than a result.
//!
//! 1. **One constant carries the whole class.** The detection row goes from bit-exact `TYPE1` to
//!    bit-exact `TYPE2`, and **65 of the 68 graded rows follow it**, with the contention constant
//!    untouched. The early/late split this suite detects is a property of the **interrupt**, not
//!    of the contention window — which is why moving 14335 never moved the detection row.
//! 2. **[`INTERRUPT_T_STATES`][spectrum::timing::INTERRUPT_T_STATES] is no longer ungraded, and
//!    it is pinned from *both* sides.** Sweeping the window from 1 to 65 with the contention
//!    constant fixed:
//!
//!    | window | detection | the 68 rows |
//!    |---|---|---|
//!    | 1 | the suite never terminates | — |
//!    | 2–3 | `R` = 15 — **neither table** | the sweep hangs |
//!    | 4–13 | `TYPE1` | the sweep hangs |
//!    | 14–16 | `TYPE1` | 5, then 2, rows disagree with `TYPE1` |
//!    | **17–32** | `TYPE1` | **0 of 68 — the green band** |
//!    | **33–43** | `TYPE2` | 3 of 68 disagree with `TYPE2`, flat across the band |
//!    | 44–65 | the suite never terminates | — |
//!
//!    So the oracle pins this constant to **`17..=32`**, and the two edges are different failures
//!    with different causes. The upper edge at 33 is the amplifier above. The edge at **44** is
//!    the *same* amplifier one level out: the other handler, at `0xF4F4`, reaches its own first
//!    `/INT` sample 43 T-states after acceptance, so at 44 the program's synchronisation nests
//!    there too and its delay loop never converges — the derivation predicted 43/44 before the
//!    sweep was run, and 43 is measured to be the last value that produces any reading at all.
//!    The floor at 4 is predicted the same way: the program's second interrupt is accepted 3
//!    T-states into the frame, so a window shorter than 4 loses it. The band from 14 to 16 is
//!    **not** explained here — detection still reads `TYPE1` while contended rows disagree, so
//!    something in the contended groups is sensitive to a short window and this gate does not say
//!    what. It is recorded because a boundary nobody predicted is worth more than one that was.
//!
//!    `timing.rs` documents this constant as *"Ungraded on both machines"*. For the 48K that is
//!    now out of date — this row grades it — and correcting it is that file's owner's call, not
//!    this one's.
//! 3. **Three rows resist, and they are named rather than absorbed.** With the window at 33 the
//!    disagreements with `TYPE2` are `group 3 contended` (`R` 42 against 41), `group 7
//!    uncontended` (`R` 95 against 98) and `group 34 uncontended` (`R` 42 against 44). Group 7
//!    locates the residue exactly: its body ends `DI` / `EI` / `JR`, so its acceptance points are
//!    quantised, and the arithmetic puts an instruction boundary **exactly on** the frame's
//!    interrupt T-state — this machine takes the interrupt there and the hardware `TYPE2` machine
//!    did not. So a `TYPE2` machine is *not* exactly this machine with a 33-T-state window; some
//!    second-order difference remains at that edge. **What would settle it** is a `TYPE2`
//!    hardware submission's full 73-row result set, or a Z80 interrupt-sampling model finer than
//!    one T-state. Neither is in hand, and neither is needed for the detection row, which is
//!    bit-exact.
//!
//!    > **Both of those routes have since been taken, and both are closed. The sentence above is
//!    > left standing because it named the right two questions; it was wrong only about their
//!    > being open.** Recorded 2026-09-01, measured rather than argued — see `docs/M7.md`
//!    > Decision 11 for the full apparatus.
//!    >
//!    > - **There is no `TYPE2` row set to fetch, and there never was.** The results database
//!    >   (`spectrum48k_timing_results.htm`, HTTP 200) is a **39-row, 8-column** table whose
//!    >   columns are Model, Z80, Board Issue, ULA, Serial, **TYPE**, Notes, Submitter. It
//!    >   publishes the *verdict* and the machine's provenance — never the 73 rows. The 2010
//!    >   Internet Archive copy has the identical shape, so the rows were not published and later
//!    >   removed. The submission form carries a **600-character free-text box**, which could not
//!    >   have held them. This is a structural absence, not a fetch failure, and it means the
//!    >   suite's own embedded `TYPE2` table — which this file already reads — is the strongest
//!    >   artefact that exists.
//!    > - **A finer-than-one-T-state sampling model has no more expressive power here, and that
//!    >   is measured.** Whatever the sub-T-state rule, its only observable consequence is the set
//!    >   of instruction-boundary positions at which `/INT` is accepted, and for an interval
//!    >   `/INT` that set is an interval `[lo, hi]`. Sweeping `lo` over `0..=4` and `hi` over
//!    >   `31..=40` — the acceptance set imposed directly, contention untouched — gives: `hi`
//!    >   matters **only** through the width (`width >= 33` is `TYPE2`, and nothing above 33
//!    >   changes a single row, because the amplifier needs exactly one boundary at `+32`); `lo`
//!    >   is the only coordinate that moves the 68 rows, and **`lo = 0` is the unique minimum
//!    >   against both tables** (`lo` 0/1/2/3/4 scores 0/8/13/15/13 against `TYPE1` and
//!    >   3/6/8/12/14 against `TYPE2`). **No window of any position or width reaches zero against
//!    >   `TYPE2`. The floor is three, and it is these three.**
//!    >
//!    > **The sample point is a gauge, not a defect — and it is paired with
//!    > [`FIRST_CONTENDED_T_STATE`][spectrum::timing::FIRST_CONTENDED_T_STATE].** Sampling `/INT`
//!    > at the instruction *boundary* with contention at **14335** and sampling it at the
//!    > instruction's **last T-state** with contention at **14336** are the same machine: run
//!    > against each other they agree on all 68 rows **and** the detection row, at width 32
//!    > (`TYPE1`, 0 of 68) and at width 33 (`TYPE2`, the same 3 of 68 with the same values).
//!    > Only the *difference* between the acceptance window and the contention window is
//!    > observable, never either alone. So "this machine samples `/INT` at the wrong point" is
//!    > not a well-posed defect against this corpus; if the Z80's real rule is the last T-state,
//!    > the constant that should read 14336 is the contention one, and **nothing else moves**.
//!    >
//!    > **What the three rows actually are: our per-instruction timing is right, and the
//!    > disagreement is entirely about which boundary takes the interrupt.** Each hardware
//!    > `TYPE2` triple is an instruction boundary that **exists in this machine's own trace**,
//!    > with exactly the right `R`, calibrated on rows we match bit-exactly (`R` recorded is `R`
//!    > at acceptance `+ 4`; the third field is the interrupted `PC` wherever it varies):
//!    >
//!    > | row | hardware wants | the boundary in our trace | our acceptance |
//!    > |---|---|---|---|
//!    > | 3 contended | `R` 41, `0x5B0B` | `r` 37 at `pc 0x5B0B`, frame T-state **69887** | 69888+3 |
//!    > | 7 uncontended | `R` 98, `0xDDDD` | `r` 94 at `pc 0xDDDD`, frame T-state **+20** | +0 |
//!    > | 34 uncontended | `R` 44 | `r` 40 at `pc 0x0082`, frame T-state **+16** | +4 |
//!    >
//!    > **And their demands are mutually inconsistent.** Writing `φ` for a uniform shift of the
//!    > group's boundaries against the window, group 3 needs `φ` in `+1..=+4`, group 7 needs
//!    > `−20..=−1`, group 34 needs `−16..=−10`. Seven and 34 intersect; **3 is disjoint from
//!    > both, and wants the opposite sign** — which is the "opposite tie-breaks" an earlier pass
//!    > reported from the row values, re-derived here from the instruction stream. Meanwhile the
//!    > detection row pins the `TYPE1`→`TYPE2` entry shift to **exactly 32** (`R` = 122 requires
//!    > entry 324 and no other value), which is `φ = 0`. **Four constraints, no common solution**
//!    > — so no single change to when this machine takes an interrupt can close all three, and
//!    > moving a constant to close one would open others.
//!    >
//!    > **The remaining possibility — that these three cells are wrong — is weaker than it looks,
//!    > and it was tested rather than assumed.** The `TYPE2` column's three most suspicious cells
//!    > are reproduced by this machine *bit-exactly*: group 29 contended's `sp` = **0** (which
//!    > reads like missing data and is not), group 11 contended's `sp` = **53256** (a far outlier
//!    > against every other contended `sp` of ~23300), and group 15's loop count roughly
//!    > **doubling** between the classes (553→1100, 462→919). A table whose oddest entries an
//!    > independent implementation lands on is not a carelessly transcribed table.
//!
//! ## Why this makes the authors' strangest observation ordinary
//!
//! The class turns on a **tie** — whether the CPU's `/INT` sample and the ULA's release edge, which
//! this program has arranged to be simultaneous to the T-state, resolve one way or the other. A tie
//! decided by sub-T-state skew is exactly the kind of thing that moves with temperature and with
//! nothing else, which is why the same board reports late when cold and early when warm, and why
//! issues 3B, 4B and 6A appear in **both** classes. Read as "one T-state of contention", that
//! observation is a puzzle; read as an edge race in the interrupt, it is what you would predict.
//!
//! # What is not graded here
//!
//! - **Test groups 36 and 37.** They need a 48K floating bus, which this machine does not model:
//!   [`FLOATING_BUS_BYTE`][spectrum::FLOATING_BUS_BYTE] is a constant. They are counted and
//!   named as skipped rather than dropped. **Group 35 was in this list until 2026-09-01.** It is
//!   graded now — the section immediately below is what that buys, what it is coupled to, and
//!   the one thing about it that is not explained.
//! - **The interrupt window's *length*.** The program only needs the interrupt to be accepted
//!   at the top of the frame; 32 T-states versus 24 would not move a number here.
//! - **Anything about a 128.** The suite has a 128 edition; this is the 48K one. The 128 edition
//!   has since been fetched — read the next section but one before extending this file to it,
//!   because the obvious extension is wrong.
//!
//! # Group 35, which *is* graded — and the constant it is silently coupled to
//!
//! ## 35, 36 and 37 are one program, and the difference between them is the screen
//!
//! Read out of the file rather than inferred. The suite's dispatch table is two bytes per test
//! based at `0xE000`, and its entries for 35, 36 and 37 — `0xE046`, `0xE048`, `0xE04A` — **all
//! hold `0xC91D`**. The BASIC supplies the only difference: lines 1350, 1360 and 1370 set `l` to
//! **0**, **13** and **21** and call the subroutine at 5330, which is `CLS` followed by
//! `FOR g=0 TO l: PRINT "ZXSPECTRUMZXSPECTRUMZXSPECTRUMZX"`. So the three tests differ **only in
//! how many rows of text are on the screen** — one, fourteen, twenty-two — and the suite says so
//! in its own titles: test 36 announces itself as `… [13]`, which is `STR$ l`, not an
//! instruction list. Its front end draws the same line: `INPUT "choose test 1-35 or leave blank
//! for all"`.
//!
//! The loop at `0xC91D` is why the screen is the variable:
//!
//! ```text
//! C91D  ED 4B 01 EF                LD BC,(0xEF01)   the counter, zeroed by the suite's entry
//! C921  03                         INC BC            block at 0xC000: LD BC,0 / LD (0xEF01),BC
//! C922  ED 43 01 EF                LD (0xEF01),BC
//! C926  79 41 51 59 61 69          LD A,C / B,C / D,C / E,C / H,C / L,C
//! C92C  D3 FF                      OUT (0xFF),A      port (C<<8)|0xFF
//! C92E  ED 79  ED 41  ED 51
//!       ED 59  ED 61  ED 69        OUT (C),A/B/D/E/H/L    port BC = (C<<8)|C
//! C93A  DB FF                      IN A,(0xFF)
//! C93C  ED 78                      IN A,(C)          port (C<<8)|C
//! C93E  ED 40                      IN B,(C)          port (C<<8)|C — and B := the value read
//! C940  ED 50  ED 58  ED 60  ED 68 IN D,(C) / E,(C) / H,(C) / L,(C)   port (B<<8)|C
//! C948  18 D3                      JR 0xC91D
//! ```
//!
//! **`IN B,(C)` overwrites the port's own high byte with the value it read**, so the last four
//! `IN r,(C)` address `(bus << 8) | C`. On hardware the bus floats a display or attribute byte
//! while the ULA is fetching, so whether those four cycles are contended is a function of **what
//! is on the screen** — which is exactly the variable the BASIC changes, and exactly the thing
//! this machine does not model. That is why 36 and 37 are excluded, and it is the whole of the
//! reason. **Carried, not re-taken here**: the investigation that established this ran group 36
//! with the screen filled with `0x00`, with `0xFF` and with a text pattern and got **identical**
//! readings all three times — which is what a constant floating bus predicts and what hardware
//! cannot do. Whoever needs that number for a decision should re-take it rather than quote it.
//!
//! ## What grading group 35 buys
//!
//! Everything *before* `IN B,(C)` addresses `(C<<8)|C` with `C` the loop counter, which starts
//! at zero and advances by one per iteration through the 237 (contended) and 267 (uncontended)
//! iterations a frame holds. So it sweeps `C` over `0x01..=0xED` at least, **all** of it also on
//! the port's high half, and reaches two things nothing external reached before:
//!
//! - **The four-case I/O rule's fourth term** — `C:1, C:1, C:1, C:1`, a port that is in
//!   contended memory and is *not* the ULA's, which every odd `C` in `0x41..=0x7F` produces.
//!   `docs/STATUS.md` carried a row saying that term's only gate was `tests/io_contention.rs` —
//!   a file this project wrote. This row is its first external check, and that is **measured
//!   rather than argued**, in a scratch clone on 2026-09-01, each mutation's landing asserted
//!   before its verdict was read and each restore checked against held bytes:
//!
//!   | mutation of the `(true, false)` arm of `Ula::port_delay` | this gate at `GROUPS = 34` | at `GROUPS = 35` | `tests/io_contention.rs` |
//!   |---|---|---|---|
//!   | deleted — `(true, false) => 0` | **GREEN**, 3 passed | **RED**, 2 of 70 | **RED**, 2 of 3 |
//!   | weakened to the two-stall shape of `(true, true)` | **GREEN**, 3 passed | **RED**, 2 of 70 | **RED**, 2 of 3 |
//!   | the fourth term alone dropped — `first + second + third` | **GREEN**, 3 passed | **RED**, 1 of 70 | **RED**, 1 of 3 |
//!
//!   **Every disagreement in all three rows is group 35's own, and no other row moves** — which is
//!   simultaneously the evidence that the extension catches something and the evidence that it
//!   disturbs nothing.
//!
//!   **The third row is not a weaker version of the first two, and reading it as one is how the
//!   three came to be mistaken for two.** The first two change the arm's cost at **every** group
//!   position: deleting it charges nothing anywhere, and the two-stall shape charges the `0x4000`
//!   row `[6, 5, 4, 3, 2, 1, 0, 6]` where the rule wants `0x4001`'s `[12, 11, 10, 9, 8, 7, 6, 12]`.
//!   Dropping the fourth term alone changes the cost at **one position in eight**, and
//!   `tests/io_contention.rs` measures which: with the term gone its per-phase sweep passes phases
//!   0 through 6 of the `0x4001` case and fails at **phase +7** — 17 where the rule wants 23 — so
//!   `d = D[p + a + b + c + 3]` lands in one of the pattern's two zero slots at every position but
//!   the last, where it is 6. Group 35's **uncontended** row sees that one-position perturbation
//!   and its **contended** row does not. So the two rows do not have different *kinds* of
//!   sensitivity — both move when the arm's arithmetic moves everywhere — and no claim that the
//!   contended row grades only the case's existence survives the middle row of the table.
//! - **`OUT (C),r`**, which `tests/io_contention.rs` names in its own *what is not graded here*.
//!   Six of them are in the loop above.
//!
//! ## The coupling, which must be read before this row is believed *or* fixed
//!
//! **Group 35 passes because [`FLOATING_BUS_BYTE`][spectrum::FLOATING_BUS_BYTE] is `0xFF`, and
//! `0xFF` lies outside the 48K's contended address range `0x4000..=0x7FFF` — not because a
//! floating bus is modelled.** `0xFF` on the port's high half makes every one of the four
//! bus-dependent `IN r,(C)` cycles an *uncontended address*, costing nothing extra, so the only
//! contention this row measures is the part that does not depend on the bus.
//!
//! The row is therefore **live to that constant**, and the failure it can produce reads like a
//! regression while being nothing of the kind:
//!
//! - moving `FLOATING_BUS_BYTE` to any value whose byte lies in `0x40..=0x7F` reddens group 35
//!   for a reason that has nothing to do with the contention model being wrong;
//! - **implementing a real floating bus can redden it too, and that would not be a
//!   regression either.** A model returning the byte the ULA is fetching returns display bytes in
//!   `0x40..=0x7F` for ordinary screen content — the suite's own `"ZXSPECTRUM…"` row is built
//!   from glyphs whose ROM bitmaps include `0x7E` (`Z`, `E`), `0x7C` (`P`, `R`), `0x66` and
//!   `0x5A` (`M`) and `0x42` (most of them) — so those four `IN` cycles would begin to be
//!   charged.
//!
//! **So whoever implements a floating bus should take group 35's two rows as part of that work's
//! acceptance criterion rather than as a pre-existing gate to keep green**, and should expect 36
//! and 37 to become gradeable in the same change. If group 35 reddens on a tree that moved the
//! bus and nothing else, the first question is *what byte does the bus float during this loop*,
//! not *what moved in the contention model*.
//!
//! ## The open question — which is why this row is green rather than safe
//!
//! **Why hardware agrees with a constant `0xFF` here is not established, and the agreement is
//! stranger than "there is only one row of text" would suggest.** Both of group 35's rows are
//! reproduced bit-exactly by this machine, and both were measured on hardware holding a screen a
//! real floating bus would not have read as `0xFF`:
//!
//! - the **contended** row runs after `CLS` and one row of `"ZXSPECTRUM…"` — 32 characters whose
//!   glyph bytes are largely inside `0x40..=0x7F`;
//! - the **uncontended** row is the harder one. Line 1350 runs it *before* the `GO SUB 5330` that
//!   clears the screen, and the suite never clears between tests — `5040`–`5070` set
//!   `BORDER`/`PAPER`/`INK` and call `USR` with no `CLS` — so on a full run it is measured with
//!   the accumulated report text of the previous thirty-four tests on screen. **Being
//!   uncontended buys that row no immunity**: I/O contention is a function of the *port address*,
//!   not of where the code sits, so its four bus-dependent `IN` cycles are exposed to the screen
//!   exactly as the contended row's are.
//!
//! A fraction argument is available and is deliberately **not** offered as the answer: the ULA
//! fetches for 128 of each line's 224 T-states, so a screen whose text occupies few rows floats a
//! contended-range byte over a small part of the frame, and 36 and 37 — fourteen and twenty-two
//! rows — are where such an argument would stop working, which is where they do stop working. It
//! does not survive the uncontended row above, and no quantitative version of it has been derived
//! here. **Recorded as open rather than resolved: this is the one gap that would make grading
//! group 35 *safe* rather than merely green, and the green must not be read as standing in for
//! the explanation.**
//!
//! # The 128 edition, and the one thing that will bite whoever extends this file
//!
//! `timing_tests-128k_v1.0.z80` sits beside the 48K file under `testdata/timing/`, fetched on
//! 2026-09-01 (12960 bytes, SHA-256
//! `fedc228ddef76cefb7b81dd6e18600cca2fd826fc18b4bc3f773cfdf2e7fffc4`). **Nothing here reads it**;
//! it is documented in `docs/M7.md` Decision 8 with its provenance and its evidence tier, and it
//! becomes runnable only when a 128 exists to run it on.
//!
//! **It carries ONE expected-result table, at `0xE200`.** [`EARLY_TABLE`] and
//! [`LATE_TABLE_OFFSET`] are 48K constants and **must not be pointed at the 128 file**: the bytes
//! at `0xE400` in the 128 file are the **48K's `TYPE2 (Late)` table, identical over all 512 bytes
//! and all 73 populated rows**, left behind because the 128 edition was produced by editing the
//! 48K program in place. Reading them would compare a 128 against 48K hardware — red for a reason
//! that has nothing to do with the 128, and near enough to look like a small modelling error
//! rather than a category error.
//!
//! The evidence that it is one table and not two is in the file's own BASIC rather than in its
//! data: the 48K's classification lines 805/807/850 are **deleted** from the 128 program, so the
//! selector both files still carry — `IF (PEEK 40004)=1 THEN LET k=k+512`, where `k` starts at
//! `57856` = `0xE200` — can never fire, and the byte at 40004 is zero.
//!
//! **One prediction, so that a first run is read correctly.** The 128 file's detection row is
//! `R` = 121, which is the 48K `TYPE2 (Late)` row's 122 carried across a 70908-T-state frame
//! (`(70908 − 69888) / 4 = 255 ≡ −1 (mod 128)`, and `R` is seven bits). The `TYPE1 (Early)` row's
//! 2 gives 1, not 121. This machine reproduces `TYPE1` — asserted below — so a 128 model
//! inheriting that interrupt phase will disagree with the whole file, on the early/late axis
//! rather than on contention. **Take the detection row first and say which axis the disagreement
//! is on before touching a constant.**
//!
//! **And the prediction is now a one-constant prediction rather than an axis.** The section above
//! establishes what the detection group measures: `R` = (11 + `N`) mod 128 over `N` four-T-state
//! iterations, and the class turns on a nested interrupt worth exactly 32 T-states. Applied to the
//! 128's frame, with the loop entered at 292 or at 292 + 32:
//!
//! | frame | entry 292 (no nesting) | entry 324 (nested) |
//! |---|---|---|
//! | 69888 | `R` = **2** — the 48K `TYPE1` row | `R` = **122** — the 48K `TYPE2` row |
//! | 70908 | `R` = **1** | `R` = **121** — *the 128 file's row* |
//!
//! Both 48K rows and the 128 row fall out of the same closed form, so the 128 file's own detection
//! byte is a measurement of the **128's interrupt window**, not of its contention: it says the
//! window is long enough that the CPU still sees `/INT` 32 T-states after accepting the previous
//! one. Nothing about 14361 is involved. Two consequences for whoever runs it first:
//!
//! - **A 128 carrying this machine's 32-T-state window will read 1, and the file will be red on
//!   every row.** That is not a contention defect and no contention constant should be touched.
//! - The relevant checks are cheap and worth doing in order: the nine bytes of the handler copied
//!   from `0xC130` are **identical** in the 128 file, and so is the whole setup block at `0xC0F3`
//!   — verified by byte diff — so the amplifier is unchanged. What the 128 file *does* change is
//!   the delay loop: `0xC0E2` and `0xC0EA` ship pre-patched to `BIT 0,(HL)` and
//!   `BIT 0,(HL)` / `CPD`, which the 48K program only writes at runtime when the ROM signature at
//!   `0x004B` is not the 48K's `0x02BF`. Those patches lengthen the loop from 277 to 281 T-states
//!   an iteration — **re-calibrating the same self-synchronising race for the 70908-T-state
//!   frame**, and landing it on the identical phase. The 128 edition is not a differently-timed
//!   program; it is the same program re-tuned to the same lock.
//!
//! **So the 128's `interrupt_t_states` is the one field that decides this file's first 128 run**,
//! and `Timing::SPECTRUM_128` currently inherits the 48K's 32 with the note that it *"measures
//! nothing"*. That note is now falsifiable rather than idle: **32 predicts a detection reading of
//! 1 and a red row everywhere; any value in `33..=43` predicts 121, which is what the file
//! carries.** The band is derived, not measured — the two edges come from instruction sequences
//! byte-identical between the two files, and there is no 128 here to run — so it is a prediction
//! to test, not a constant to adopt. **Do not change it on the strength of this paragraph**: run
//! the detection row, and let the 128 file say which value it wants.
//!
//! # Absence
//!
//! Through `crates/testsupport` unchanged, exactly as every other corpus in this workspace:
//! present, it runs; absent, the gate **fails** naming the fetch; absent with
//! `ZX_CORPUS_ALLOW_MISSING`, it skips; and that opt-out is refused under `CI`.
//!
//! ```sh
//! mkdir -p testdata/timing
//! curl -fSL -o testdata/timing/timing_tests_48k_v1.0.z80 \
//!   https://raw.githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.EmulatorTestSuites.ZXSpectrum/Timing/timing_tests_48k_v1.0.z80
//! ```
//!
//! Provenance, licensing and the SHA-256 are in `docs/MACHINE.md`.

use spectrum::Spectrum;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// Where the snapshot lives under `testdata/`.
const CORPUS: [&str; 2] = ["timing", "timing_tests_48k_v1.0.z80"];

/// Where the committed Sinclair ROM lives under `testdata/`.
const ROM: [&str; 2] = ["roms", "48.rom"];

/// The fetch instructions, repeated in the failure message a developer will actually read.
const FETCH: &str = "curl -fSL -o testdata/timing/timing_tests_48k_v1.0.z80 https://raw.\
                     githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.\
                     EmulatorTestSuites.ZXSpectrum/Timing/timing_tests_48k_v1.0.z80";

/// Read a corpus file, or `None` having applied the shared absence policy.
///
/// `fetch` is printed **before** the policy is consulted, because on the undeclared-absence
/// path the policy panics and nothing after it runs. It is the one thing a developer meeting
/// this failure actually needs, and the shared message can only point at
/// `testdata/README.md`.
fn corpus_file(what: &str, parts: [&str; 2], fetch: Option<&str>) -> Option<Vec<u8>> {
    // Unconditionally, not only on the absent path: an obsolete spelling must be an error in
    // *every* gate, or a CI file still exporting one is silently ignored by whichever gate
    // happens to find its corpus present.
    testsupport::reject_obsolete_env();

    let path: PathBuf = testsupport::testdata_dir().join(parts[0]).join(parts[1]);
    if !path.is_file() {
        if let Some(fetch) = fetch {
            println!(
                "{what} is fetched on demand:\n  mkdir -p testdata/{}\n  {fetch}",
                parts[0]
            );
        }
        testsupport::skip_absent_corpus(what, &path);
        return None;
    }
    Some(std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display())))
}

/// Both corpora this gate needs, or `None` if either is absent.
fn corpora() -> Option<(Vec<u8>, Vec<u8>)> {
    let snapshot = corpus_file(
        "Richard Butler's 48K timing test suite (timing_tests_48k_v1.0.z80)",
        CORPUS,
        Some(FETCH),
    )?;
    let rom = corpus_file("the Sinclair 48K ROM", ROM, None)?;
    Some((snapshot, rom))
}

// ---------------------------------------------------------------------------
// Reading the snapshot
//
// Deliberately not through `spectrum::snapshot`. Two reasons, and the second is the load
// bearing one. This file must be able to run against a machine whose snapshot module is
// mid-change, and — more importantly — an oracle that reaches its expectations through the
// crate under test has one more way to agree with it than it should. Sixty lines of
// run-length decoding is a cheap price for the expectations arriving from outside.
// ---------------------------------------------------------------------------

/// The whole 64 KB address space as a snapshot describes it.
type Image = [u8; 0x1_0000];

/// Where a `.z80` version 2/3 page number lands in a 48K address space.
///
/// Pages 0–3 are the 128's ROM and low banks and cannot appear in a 48K file.
const fn page_base(page: u8) -> Option<usize> {
    match page {
        4 => Some(0x8000),
        5 => Some(0xC000),
        8 => Some(0x4000),
        _ => None,
    }
}

/// Decode the version 2/3 `.z80` in `file` into a 64 KB image, RAM only.
///
/// Structure is validated rather than a checksum pinned — the same choice `testdata/README.md`
/// records for the FUSE vectors and the `zex` exercisers, and for the same reason: any genuine
/// copy of the file must work, and what the gate actually depends on is the content, which
/// `the_corpus_is_the_timing_suite_and_carries_two_distinct_hardware_tables` — this file's own
/// positive control, which asserts *"the snapshot must contain the suite's own … banner"* for
/// both hardware tables — checks directly.
fn load_z80(file: &[u8]) -> Image {
    assert!(file.len() > 34, "a .z80 header is at least 30 bytes");
    assert_eq!(
        u16::from_le_bytes([file[6], file[7]]),
        0,
        "expected a version 2/3 .z80, whose version-1 PC field is zero"
    );
    let extra = usize::from(u16::from_le_bytes([file[30], file[31]]));
    assert!(
        matches!(extra, 23 | 54 | 55),
        "unexpected additional header length {extra}"
    );
    assert_eq!(
        u16::from_le_bytes([file[32], file[33]]),
        0xC000,
        "the suite's entry point is 0xC000"
    );
    assert_eq!(file[34], 0, "expected hardware mode 0 — a 48K");

    let mut image = [0_u8; 0x1_0000];
    let mut seen = [false; 3];
    let mut at = 32 + extra;
    while at + 3 <= file.len() {
        let length = usize::from(u16::from_le_bytes([file[at], file[at + 1]]));
        let page = file[at + 2];
        at += 3;

        let base = page_base(page).unwrap_or_else(|| panic!("page {page} is not a 48K page"));
        let index = match page {
            4 => 0,
            5 => 1,
            _ => 2,
        };
        assert!(!seen[index], "page {page} appears twice");
        seen[index] = true;

        if length == 0xFFFF {
            let end = at + 0x4000;
            assert!(end <= file.len(), "page {page} is truncated");
            image[base..base + 0x4000].copy_from_slice(&file[at..end]);
            at = end;
        } else {
            let end = at + length;
            assert!(end <= file.len(), "page {page} is truncated");
            decompress_page(&file[at..end], &mut image[base..base + 0x4000], page);
            at = end;
        }
    }
    assert_eq!(at, file.len(), "trailing bytes after the last page");
    assert_eq!(seen, [true; 3], "a 48K snapshot carries pages 4, 5 and 8");
    image
}

/// Expand one run-length encoded page. `ED ED count value` is a run; everything else is literal.
fn decompress_page(block: &[u8], page: &mut [u8], number: u8) {
    let mut out = 0_usize;
    let mut at = 0_usize;
    while at < block.len() {
        if at + 3 < block.len() && block[at] == 0xED && block[at + 1] == 0xED {
            let count = usize::from(block[at + 2]);
            assert!(
                out + count <= page.len(),
                "a run overflows page {number} at offset {out}"
            );
            page[out..out + count].fill(block[at + 3]);
            out += count;
            at += 4;
        } else {
            assert!(out < page.len(), "page {number} overflows at offset {out}");
            page[out] = block[at];
            out += 1;
            at += 1;
        }
    }
    assert_eq!(out, page.len(), "page {number} decoded to {out} bytes");
}

// ---------------------------------------------------------------------------
// The suite's own layout, all of it read out of the image
// ---------------------------------------------------------------------------

/// The three numbers the suite's interrupt handler records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reading {
    /// The Z80 refresh register at the moment the interrupt was taken.
    refresh: u8,
    /// Loop iterations the test group completed before the interrupt.
    iterations: u16,
    /// The stack pointer at the moment the interrupt was taken.
    stack_pointer: u16,
}

impl std::fmt::Display for Reading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "R={:3} loop={:5} sp={:5}",
            self.refresh, self.iterations, self.stack_pointer
        )
    }
}

/// Where the running program leaves its measurement.
const RESULT: u16 = 0xEF00;

/// Base of the `TYPE1 (Early)` expectation table.
const EARLY_TABLE: u16 = 0xE200;

/// The `TYPE2 (Late)` table sits this far above it.
const LATE_TABLE_OFFSET: u16 = 512;

/// Bytes per test group in a table: uncontended at +0, contended at +5.
const TABLE_STRIDE: u16 = 10;

/// The suite's own name for a machine's one-T-state timing class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimingType {
    /// `TYPE1 (Early)` — the table at [`EARLY_TABLE`].
    Early,
    /// `TYPE2 (Late)` — the table 512 bytes above it.
    Late,
}

impl TimingType {
    /// The BASIC line the suite prints when it detects this class, verbatim from the snapshot.
    const fn banner(self) -> &'static [u8] {
        match self {
            Self::Early => b"TYPE1 (Early) timings detected.",
            Self::Late => b"TYPE2 (Late) timings detected.",
        }
    }

    const fn table(self) -> u16 {
        match self {
            Self::Early => EARLY_TABLE,
            Self::Late => EARLY_TABLE + LATE_TABLE_OFFSET,
        }
    }
}

/// One row of an expectation table.
fn expected(image: &Image, timing: TimingType, group: u8, contended: bool) -> Reading {
    let at = timing.table() + u16::from(group) * TABLE_STRIDE + if contended { 5 } else { 0 };
    read_reading(image, at)
}

/// Three numbers in the suite's own layout: a byte, then two little-endian words.
fn read_reading(image: &Image, at: u16) -> Reading {
    let byte = |offset: u16| image[usize::from(at.wrapping_add(offset))];
    Reading {
        refresh: byte(0),
        iterations: u16::from_le_bytes([byte(1), byte(2)]),
        stack_pointer: u16::from_le_bytes([byte(3), byte(4)]),
    }
}

// ---------------------------------------------------------------------------
// Running one test group on the machine
// ---------------------------------------------------------------------------

/// `RANDOMIZE USR 49152`, entered after the ROM has already put the address in `BC`.
///
/// The three instructions at `0x34B6` are `LD HL,0x2D2B` / `PUSH HL` / `PUSH BC` / `RET`, so
/// entering here jumps to `BC` with the ROM's own continuation on the stack — which is how the
/// suite is started from BASIC, and therefore the state it was measured in.
const USR_ENTRY: u16 = 0x34B6;

/// Where the machine code the suite runs begins.
const PROGRAM: u16 = 0xC000;

/// Where it stops. The bytes there are `DI / LD A,0x3F / LD I,A / IM 1 / EI / RET` — the
/// suite handing the machine back to BASIC, and the last moment its measurement is intact.
const STOP: u16 = 0xBC28;

/// The suite reads its test number from here, exactly as the BASIC front end pokes it.
const TEST_NUMBER: u16 = 40000;

/// Non-zero here runs the group from `0x5B00`, in the contended bank, instead of where it sits.
const CONTENDED_FLAG: u16 = 40002;

/// Stack pointer the BASIC front end leaves before `USR`.
const ENTRY_SP: u16 = 0xFFFE;

/// A ceiling that is far more than the two frames a group needs, and far less than forever.
const T_STATE_CEILING: u64 = 10_000_000;

/// Run one test group and return what its interrupt handler recorded.
///
/// The two-machine dance is the faithful part rather than a convenience. The suite's numbers are
/// a count of work completed before an interrupt, so they are only meaningful if the program
/// starts where the hardware starts it: at the **top of a frame**. Running the ROM's four-
/// instruction `USR` prologue moves the clock off zero, so the prologue runs on one machine and
/// its result — RAM and registers — is installed into a second machine that is still at frame
/// zero, T-state zero. A fresh [`Spectrum`] is the only thing in this crate's public surface that
/// is definitionally there, which is why the second machine is built rather than rewound.
fn run_group(rom: &[u8], image: &Image, group: u8, contended: bool) -> Reading {
    let mut prologue = machine_holding(rom, image);
    prologue.memory_mut().write(TEST_NUMBER, group);
    prologue
        .memory_mut()
        .write(CONTENDED_FLAG, u8::from(contended));

    let mut state = prologue.cpu_state();
    state.pc = USR_ENTRY;
    state.bc = PROGRAM;
    state.sp = ENTRY_SP;
    prologue.set_cpu_state(state);

    // `LD HL,nn` / `PUSH HL` / `PUSH BC` / `RET`. Bounded well above four so a ROM that does
    // something else fails with a position rather than hanging.
    for _ in 0..16 {
        if prologue.cpu_state().pc == PROGRAM {
            break;
        }
        prologue.step();
    }
    assert_eq!(
        prologue.cpu_state().pc,
        PROGRAM,
        "the ROM's USR prologue did not reach {PROGRAM:#06X}"
    );

    let mut ram = [0_u8; 0xC000];
    for (offset, byte) in ram.iter_mut().enumerate() {
        *byte = prologue
            .memory()
            .read(0x4000 + u16::try_from(offset).expect("48K of RAM"));
    }

    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");
    for (offset, byte) in ram.iter().enumerate() {
        machine
            .memory_mut()
            .write(0x4000 + u16::try_from(offset).expect("48K of RAM"), *byte);
    }
    machine.set_cpu_state(prologue.cpu_state());
    assert_eq!(
        (machine.frames(), machine.frame_t_state()),
        (0, 0),
        "the measurement must start at the top of a frame"
    );

    while machine.cpu_state().pc != STOP {
        machine.step();
        let elapsed = machine.frames() * u64::from(spectrum::timing::T_STATES_PER_FRAME)
            + u64::from(machine.frame_t_state());
        assert!(
            elapsed <= T_STATE_CEILING,
            "test group {group} ({}) ran {elapsed} T-states without reaching {STOP:#06X}; \
             PC={:#06X}",
            label(contended),
            machine.cpu_state().pc
        );
    }
    assert_eq!(
        machine.fault(),
        None,
        "a Spectrum cannot fault: its bus floats to 0xFF, which is RST 38h"
    );

    read_reading_from(&machine, RESULT)
}

/// The same three numbers, read out of a running machine rather than out of the image.
fn read_reading_from(machine: &Spectrum, at: u16) -> Reading {
    let byte = |offset: u16| machine.memory().read(at.wrapping_add(offset));
    Reading {
        refresh: byte(0),
        iterations: u16::from_le_bytes([byte(1), byte(2)]),
        stack_pointer: u16::from_le_bytes([byte(3), byte(4)]),
    }
}

/// A machine holding the ROM and the snapshot's RAM, at the top of frame zero.
fn machine_holding(rom: &[u8], image: &Image) -> Spectrum {
    let mut machine = Spectrum::new(rom).expect("the 48K ROM is one page");
    for address in 0x4000..=0xFFFF_u32 {
        let address = u16::try_from(address).expect("inside the address space");
        machine.memory_mut().write(address, image[address as usize]);
    }
    machine
}

const fn label(contended: bool) -> &'static str {
    if contended {
        "contended"
    } else {
        "uncontended"
    }
}

// ---------------------------------------------------------------------------
// What is graded
// ---------------------------------------------------------------------------

/// The group the suite uses to decide which of the two machines it is running on.
///
/// It is not one of the suite's 37 numbered test groups: it is `JP (HL)` jumping to itself out of
/// uncontended memory, so it measures **only** where the interrupt falls, at four T-states and
/// one `R` increment per iteration.
///
/// The loop is at the absolute address `0xC146` — group 0's own code ends `LD HL,0xC146` /
/// `JP (HL)`, so both the uncontended and the contended copy converge on it. The recorded byte is
/// `(11 + N) mod 128` over `N` iterations, and the two classes differ by eight iterations for the
/// reason the module doc derives.
const DETECTION_GROUP: u8 = 0;

/// The instruction groups, `1..=GROUPS`.
///
/// **35 rather than 34 since 2026-09-01**, which is 70 hardware rows rather than 68. Group 35 is
/// the suite's first floating-bus group and it grades here anyway, because the part of it that
/// this machine can run — a port of `(C<<8)|C` sweeping every high byte and both parities of A0 —
/// reaches the four-case I/O rule's fourth term and `OUT (C),r`, neither of which any other
/// external check in this workspace touches. **It passes for a reason that is not "the floating
/// bus is right", and the module doc's *Group 35* section is that reason**; read it before
/// treating a red here as a contention defect.
const GROUPS: u8 = 35;

/// Groups the suite carries that this machine cannot run.
///
/// 36 and 37 differ from 35 *only* in how many rows of text the BASIC draws first, and their
/// readings therefore turn on what the floating bus returns — which here is the constant
/// [`FLOATING_BUS_BYTE`][spectrum::FLOATING_BUS_BYTE]. Named rather than silently skipped,
/// because `docs/STATUS.md` records what a silently narrowed gate costs.
const NEEDS_FLOATING_BUS: [u8; 2] = [36, 37];

#[test]
fn the_corpus_is_the_timing_suite_and_carries_two_distinct_hardware_tables() {
    // The positive control. Every number this file asserts is read out of the loaded image, so
    // a wrong, renamed or truncated file would supply its own expectations and the gate would
    // agree with itself — this project's recorded "a count of zero and an absence of the
    // subject are the same observation", in the one shape that fits a data-driven oracle.
    let Some((snapshot, _)) = corpora() else {
        return;
    };
    let image = load_z80(&snapshot);

    for timing in [TimingType::Early, TimingType::Late] {
        assert!(
            image
                .windows(timing.banner().len())
                .any(|window| window == timing.banner()),
            "the snapshot must contain the suite's own {:?} banner",
            std::str::from_utf8(timing.banner()).expect("ASCII"),
        );
    }

    let mut differing = 0_usize;
    let mut populated = 0_usize;
    for group in 0..=GROUPS {
        for contended in [false, true] {
            let early = expected(&image, TimingType::Early, group, contended);
            let late = expected(&image, TimingType::Late, group, contended);
            let empty = Reading {
                refresh: 0,
                iterations: 0,
                stack_pointer: 0,
            };
            if early == empty && late == empty {
                continue;
            }
            populated += 1;
            if early != late {
                differing += 1;
            }
        }
    }
    assert!(
        populated >= 2 * usize::from(GROUPS),
        "only {populated} table rows are populated; the tables are not the suite's"
    );
    assert!(
        differing * 10 >= populated * 9,
        "the two tables should differ on almost every row ({differing} of {populated}); if \
         they agree, this gate cannot see the one T-state that separates the two machines"
    );
}

#[test]
fn the_machine_reproduces_one_of_the_two_measured_hardware_timings() {
    // The detection group is `JP (HL)` jumping to itself in **uncontended** memory, so what this
    // grades is where the interrupt falls relative to an instruction boundary — and nothing about
    // contention, which cannot reach it. The two hardware answers are `R` = 2 and `R` = 122
    // against an identical loop count and stack pointer, so a machine whose interrupt lands
    // anywhere else matches neither. What it does *not* do is grade the contention phase; that is
    // the seventy rows in the test below.
    //
    // What it *does* grade, and nothing else in this workspace does, is
    // `timing::INTERRUPT_T_STATES` — the module doc derives why, and measures it: at 32 this row
    // reads 2, at 33 it reads 122. `timing.rs` still calls that constant's length ungraded; this
    // assertion is what makes that sentence out of date.
    let Some((snapshot, rom)) = corpora() else {
        return;
    };
    let image = load_z80(&snapshot);

    let actual = run_group(&rom, &image, DETECTION_GROUP, false);
    let early = expected(&image, TimingType::Early, DETECTION_GROUP, false);
    let late = expected(&image, TimingType::Late, DETECTION_GROUP, false);

    let detected = if actual == early {
        TimingType::Early
    } else if actual == late {
        TimingType::Late
    } else {
        panic!(
            "this machine is neither of the two real behaviours the suite has ever seen.\n  \
             measured here : {actual}\n  \
             TYPE1 (Early) : {early}\n  \
             TYPE2 (Late)  : {late}"
        )
    };

    // Pinned, so that a change of class is loud rather than absorbed. Which class this machine
    // lands in is a *measurement of the emulator*, not a target: it follows from
    // `timing::FIRST_CONTENDED_T_STATE` and from where the interrupt is asserted.
    assert_eq!(
        detected,
        TimingType::Early,
        "the machine changed timing class; see docs/MACHINE.md before updating this"
    );
}

#[test]
fn every_instruction_group_matches_the_hardware_table_contended_and_not() {
    // Seventy numbers, each of them a whole frame's worth of contention integrated over
    // hundreds of iterations, and not one of them derived from `FIRST_CONTENDED_T_STATE`, from
    // the delay pattern, or from the four-case I/O rule.
    //
    // The last two are group 35's, and they are the only ones here that grade the I/O rule's
    // **fourth** term — see the module doc, which also says what they are coupled to and why
    // their green is narrower than it looks.
    let Some((snapshot, rom)) = corpora() else {
        return;
    };
    let image = load_z80(&snapshot);

    let detection = run_group(&rom, &image, DETECTION_GROUP, false);
    let timing = if detection == expected(&image, TimingType::Early, DETECTION_GROUP, false) {
        TimingType::Early
    } else {
        TimingType::Late
    };

    let mut failures: Vec<String> = Vec::new();
    let mut against_early = 0_usize;
    let mut against_late = 0_usize;
    let mut compared = 0_usize;
    let mut readings: Vec<Reading> = Vec::new();
    for group in 1..=GROUPS {
        assert!(
            !NEEDS_FLOATING_BUS.contains(&group),
            "group {group} needs a floating bus and must not be in the graded range"
        );
        for contended in [false, true] {
            let want = expected(&image, timing, group, contended);
            let got = run_group(&rom, &image, group, contended);
            readings.push(got);
            compared += 1;
            against_early +=
                usize::from(got != expected(&image, TimingType::Early, group, contended));
            against_late +=
                usize::from(got != expected(&image, TimingType::Late, group, contended));
            if got != want {
                failures.push(format!(
                    "  group {group:2} {:<11}  want {want}   got {got}",
                    label(contended)
                ));
            }
        }
    }

    assert_eq!(
        compared,
        2 * usize::from(GROUPS),
        "every group must be run in both forms"
    );

    // The assertion whose failure means "I was not looking at the thing". Every number below
    // is read out of the guest's own RAM at `RESULT`, so a run in which the program never
    // executed — a prologue that landed somewhere else, a stop address reached immediately —
    // would report whatever was already there, identically, seventy times, and the
    // comparison above would be a comparison of one stale word against a table. The floor is
    // deliberately loose: this only has to separate *ran* from *did not run*, and the observed
    // spread is 60 distinct iteration counts across the 70 rows.
    let mut distinct: Vec<u16> = readings.iter().map(|r| r.iterations).collect();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(
        distinct.len() >= 20,
        "only {} distinct loop counts across {compared} groups — the test program is not \
         running",
        distinct.len()
    );

    // Both counts, always, because the interesting failure is not "we drifted off our table"
    // but "we are between the two real machines" — a shape that is invisible if only the
    // detected table is reported, and one a wrong contention phase produces exactly.
    assert!(
        failures.is_empty(),
        "{} of {compared} groups disagree with the {timing:?} hardware table.\n{}\n\
         Mismatches against TYPE1 (Early): {against_early} of {compared}; against TYPE2 \
         (Late): {against_late} of {compared}. A machine that is genuinely one of the two \
         scores zero against that one.\n\
         Groups {NEEDS_FLOATING_BUS:?} are excluded: they read the floating bus, which this \
         machine does not model.",
        failures.len(),
        failures.join("\n")
    );
}
