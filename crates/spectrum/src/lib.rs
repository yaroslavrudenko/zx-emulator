//! A ZX Spectrum — 48K or 128: paged memory, the ULA, contention, the keyboard, the frame
//! loop, and the two things that make a noise.
//!
//! *(This line read "A ZX Spectrum 48K" until M7's sound half. It was true when written and
//! had been false since the 128 landed, which is the class `docs/STATUS.md` names — every one
//! of those comments was true when written, and milestone boundaries are when they stop being.)*
//!
//! ```no_run
//! use spectrum::{Key, Spectrum};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let rom = std::fs::read("testdata/roms/48.rom")?;
//! let mut machine = Spectrum::new(&rom)?;
//!
//! machine.run_frames(100);              // the ROM's start-up and copyright message
//! machine.keyboard_mut().press(Key::P);
//! machine.run_frames(2);
//! # Ok(())
//! # }
//! ```
//!
//! # The two facts this crate is built around
//!
//! **The machine owns the clock.** [`z80::Cpu::step`] returns T-states, but contention adds
//! time on this side of the bus — measured at M1, where a contended bus left `step`'s
//! return identical to a flat run while the bus's own clock diverged. So the frame is
//! driven by counting [`z80::Bus::tick`] calls, and nothing here adds up what `step`
//! returns. A machine that did would get the instruction count right and the time wrong,
//! with nothing failing.
//!
//! **One step can overrun the frame.** A run of `DD`/`FD` prefixes is *one* instruction and
//! guest memory decides how long it is, so there is no maximum to stop before.
//! [`Spectrum::run_frame`] therefore watches for the frame *counter* to change rather than
//! trying to land on 69888, and [`timing::Clock`] rolls over however far it is pushed.
//!
//! # Verification, honestly
//!
//! M1–M4 had oracles: FUSE grades an instruction, `zexdoc` and `zexall` grade the
//! processor. This section used to continue **"Nothing grades a machine."**
//!
//! **That is no longer true, and the correction is narrower than the sentence it replaces.**
//! `crates/spectrum/tests/timing_oracle.rs` runs Richard Butler's 48K timing suite — T-state
//! counts measured on real Spectrums — and grades this machine against them. What it
//! establishes, stated at exactly its own width:
//!
//! > **The first contended T-state falls exactly 14335 T-states after `/INT`**, given that this
//! > machine asserts `/INT` at frame T-state 0.
//!
//! Two neighbouring things stay open, each because a mutation of it left the oracle *green*:
//! the frame's **origin** is a convention (moving `/INT` and the window together passes), and
//! `64 x 224` is measured only as its **product** and not as its factors.
//!
//! **A third used to be on that list and has come off it, which is worth more than the row.**
//! [`timing::INTERRUPT_T_STATES`] was called unmeasured on the strength of one mutation, 32 →
//! 24, coming back green. Sweeping the whole range 1–65 pins it to **`17..=32`**, with 24 sitting
//! comfortably inside the band — so the single sample was not evidence of insensitivity, it was
//! a sample from the flat part of a curve with sharp edges either side. That is this project's
//! standing failure, *"reporting the absence of a distinguishing test as evidence of
//! correctness"*, in the form where one probe of a parameter is read as a verdict about the
//! parameter. Details, including the 14–16 band nothing explains, are in
//! [`timing::Timing::SPECTRUM_48K`].
//!
//! **The rest of the old sentence is not merely still true, it is the stronger claim.** The
//! floating bus, progressive drawing and keyboard ghosting are **not modelled**, so they are
//! not *gradeable* rather than ungraded — the suite's own groups 35–37 are excluded by name for
//! exactly that reason. And **none of it transfers to the 128**, whose contention figures are
//! transcribed from a source that cites nothing; see [`timing::Timing::SPECTRUM_128`].
//!
//! ## What the boot gate was measured to prove, which is much less than it looks
//!
//! **These verdicts were taken at commit `2157331`, against the boot *example*.** At that
//! commit there was no `crates/spectrum/tests/` directory; the gate was
//! `crates/spectrum/examples/boot.rs`, and `cargo test` builds an example without ever
//! calling its `main`. So every "green" below means green under a program nothing ran. The
//! table is left exactly as it was recorded, and what it is worth is corrected beneath it.
//!
//! This table used to end with "*All of it together* — **the boot gate**". That row was
//! doing almost none of the work it claimed. Measured by mutation, each verified present in
//! the file before its verdict was trusted:
//!
//! | Mutation | Boot gate |
//! |---|---|
//! | Screen address layout corrupted | **red** |
//! | `/INT` never asserted | green |
//! | Keyboard reports every key held | green |
//! | ROM slot made writable | green |
//! | Contention removed entirely | green — but the frame the message appears on moved 87 → 85 |
//! | Contention phase off by one (14335 → 14334) | green, and the output is byte-identical |
//!
//! Two things follow. The gate graded the memory map and the screen and nothing else. And
//! the one number that *did* discriminate — the frame the message first appeared on — was
//! printed and never asserted, so it caught nothing. `tests/boot.rs` now asserts it.
//!
//! ### Re-measured against the whole workspace, where the claim got smaller and sharper
//!
//! A coverage table is a claim about a *run*, and naming the wrong run makes every row wrong
//! at once — in both directions. Re-run under `cargo test --workspace`, **four of the five
//! green rows above were already red**, from unit tests inside `src` that an uncalled
//! example never involved. **Exactly one** of the five survived the whole workspace — the
//! contention phase off by one — together with a permutation of the keyboard matrix that
//! **is not in the table above at all**, which a cold review found.
//!
//! > That count was itself reported as *three of five* for a while, and was corrected to
//! > four when it was measured properly rather than carried forward. It is noted rather than
//! > silently bumped, because a re-measured number that changes quietly is the same defect
//! > this whole section is about.
//!
//! So the before-picture is not "five properties were ungraded". It is that the machine's
//! behaviour was tested **in isolation and never through the machine**, and that the
//! contention phase and the keyboard-matrix wiring had no gate in any form. That is the
//! smaller claim, and it is what makes those two survivors the significant ones:
//! `tests/contention_phase.rs` and `tests/keyboard_matrix.rs` exist because those were the
//! only two properties nothing anywhere could see.
//!
//! **Which four were already red is not recorded, and is not derived here.** Re-running the
//! five mutations under `cargo test -p spectrum --lib` names them, because the lib target
//! alone is what "unit tests inside `src`" means; whoever wants that list should take it
//! that way rather than infer it from the table below. Use `--no-fail-fast` for anything
//! wider: `cargo test` stops at the first failing target, and "the integration gates did not
//! run" is indistinguishable from "the integration gates passed".
//!
//! The next table therefore **disagrees with the one above on every green row** — the
//! interrupt, the keyboard, ROM writes and both halves of contention each have a gate now,
//! six in `crates/spectrum/tests/` between them. That is the correction and not a
//! contradiction: the rows above describe `2157331`, the rows below describe this commit.
//!
//! ## What is asserted, and by what
//!
//! | Property | Evidence |
//! |---|---|
//! | Memory map, slot indirection | unit tests in [`memory`] |
//! | Screen address layout | unit tests in [`screen`], including the bijection over all 6144 bytes and that no `(column, line)` pair escapes the display file |
//! | Frame length, interrupt window, contention pattern | unit tests in [`timing`] |
//! | Machine-cycle boundaries — one charge per cycle, each internal cycle charged on its own | unit tests in [`ula`] over synthesised streams, and `tests/contention_magnitude.rs` through a real `Cpu<Ula>`. The Z80 cycle lengths [`ula`] holds duplicate `crates/z80`'s private ones; nothing compares the two sets, but a wrong length here moves a hand-derived figure there |
//! | ROM write-protection | `tests/rom_write_protection.rs` — every address of the page, driven by the **slot map**, through all three write paths |
//! | Keyboard matrix | `tests/keyboard_matrix.rs` — the full 40 key × 8 half-row cross product, against a membrane table written independently of [`keyboard`]'s own map, plus the two absolute anchors |
//! | 50 Hz interrupt | `tests/frame_interrupt.rs` — the line, its window, acceptance against `IFF1` × position, `HALT` escape, and the real ROM's own `FRAMES` counter advancing once per frame |
//! | Contention *magnitude* | `tests/contention_magnitude.rs` — the same instruction one bank apart, per phase and over a run, driven through a real `Cpu<Ula>` |
//! | Read-modify-write contention, **across the family** | `tests/contention_magnitude.rs` — `INC (HL)` at 26/19 T-states, `RLC (HL)` at 34/27, `INC (IX+d)` and `RLC (IX+d)` at 58/51, and `EX (SP),HL` at 48/41, per contention phase. The family used to be gated by one member, with the rest correct *by construction* — an argument, not a verdict |
//! | Internal cycles on a **contended refresh address** | `tests/contention_magnitude.rs`. Nothing had ever graded this: only the uncontended case was covered, and it is the case M7 makes routine, since a 128 contends its banks in any slot |
//! | The whole machine | `tests/boot.rs` — the ROM reaching `© 1982 Sinclair Research Ltd`, **and the frame it does it on** |
//! | Contention *phase* on a **48K** — [`timing::FIRST_CONTENDED_T_STATE`] | `tests/timing_oracle.rs`, **against hardware**, and precisely this much: the first contended T-state falls exactly **14335 T-states after `/INT`**, given `/INT` at frame T-state 0. Of the values near it only 14335 is green — 14333, 14334, 14336, 14337 and 14361 all turn it red. `tests/contention_phase.rs` additionally pins it to the frame's structure. *(This row read **"No oracle"** for a milestone after `timing_oracle.rs` closed it, while [`timing`]'s own module documentation said the opposite three files away. A coverage table is the thing a reader trusts, so of two contradicting statements it was the worse one to leave standing.)* |
//! | The frame's **origin** and the `64 x 224` **factorisation** | **nothing**, and each is a mutation the oracle came back *green* under. They are the claims the row above must not be read as covering |
//! | The interrupt window's **length** on a 48K | `tests/timing_oracle.rs`, **against hardware**, as a band rather than a point: a sweep over 1–65 is green only on **`17..=32`**. Below 17 the contended rows disagree; at 33 the suite's *class* flips; from 44 it stops terminating. This row previously read **nothing** on the strength of a single 32 → 24 mutation — one sample from inside the band, mistaken for the band |
//! | The interrupt window's **length** on a 128 | **nothing, and it is the number the first 128 measurement will actually be sensitive to.** The 128 suite's detection row is a function of this and **not** of the contention offset: 32 predicts 1, `33..=43` predicts the 121 the file carries. 32 is shipped as a labelled hold rather than a belief, and the prediction is asserted in [`timing`] so it goes red the moment anyone moves it |
//! | Contention phase on a **128** — [`timing::Timing::SPECTRUM_128`] | **nothing that reaches *measured*.** 14361 is transcribed from the World of Spectrum 128K reference and repeated by the Sinclair Wiki **citing no source** — one ancestor seen twice, no primary posting or measurement found. A lower evidence tier than the 48K's 14335, and the two must not be quoted alike |
//! | The 128's **frame length** — 70908 | **derived, and unusually well.** Three independent lineages agree, `228 x 311` closes it, and the 128 timing suite's own detection row corroborates it arithmetically from its bytes — with a stated blind spot, since that check is periodic in 512 T-states |
//! | The 128's contention **pattern** and **contended bank set** | **derived**, from the same single ancestry as 14361. `tests/m7_contention.rs` grades the *mechanism* — that contention follows the bank into whichever slot it is paged — which is a different claim from the numbers being right |
//! | The paging port `0x7FFD` | `tests/m7_paging.rs` — all 256 values through a real `OUT`, the lock's absorbing behaviour, and the derived slot map. The **bit layout** and the **partial decode** both have a primary witness in the Sinclair *Servicing Manual* (§4.3, §4.12.11–12) and are graded here against the transcription; nothing in reach grades the decode's outer family, because no software addresses the port any other way |
//! | Reading `0x7FFD` | **not modelled, and deliberately.** On the hardware a read latches the floating bus into the paging register — the manual has the latch firing on an *"I/O read or write cycle"* and the Sinclair Wiki reports that reads crash the machine. The value latched **is** the floating bus, which this machine does not model, so implementing it would page to a byte we invented. See [`ula`] |
//! | The 128's second ROM | `tests/m7_boot_128.rs` — reached **from the keyboard**, through the menu, and confirmed by the ROM's own copyright *year* changing from 1986 to 1982. The two images are also proven distinct at construction in `tests/m7_paging.rs` |
//! | The shadow screen | `tests/m7_shadow_screen.rs` — bit 3 changing what [`screen::render`] draws **and nothing else**, and the contended set **not** following the screen select. That second property is the one a plausible wrong model passes every bank-5 test with |
//! | Paging is what makes the 128 different | `tests/m7_bank_signature.rs` — a program that passes on a 128 **and fails in an asserted way on a 48K**. Without the negative half it could be passing for a reason unrelated to paging |
//! | The interrupt **acknowledge** as one machine cycle | `tests/m7_acknowledge.rs`, through a real `Cpu<Ula>` with `IR` in a contended bank. **Pinned, not graded, and nothing can grade it** — no software on either machine reaches a contended acknowledge, which `ula`'s documentation re-derives on the 128's own geometry rather than inheriting |
//! | Floating bus | **nothing** — not modelled; see [`ula`] |
//! | Progressive drawing: multicolour, Nirvana sprites | **nothing** — not modelled; see [`screen`]. *(This row read "multicolour, **border stripes**". The border half is no longer true — see the four rows below — and the bitmap and attribute halves are unchanged)* |
//! | The **border** drawn as the beam painted it | `tests/border_stripes.rs` — a write at a known T-state landing in the row the timing model says, hand-derived per machine and asserted on both. The mapping comes out of [`timing::Timing`] and nothing else, so there is no second beam-position model beside contention's |
//! | That the border change is **visible to a frontend** | `tests/border_stripes.rs`'s `the_frontends_own_loop_shows_the_bands`. **Not a formality**: a record served only for the frame *running* shows a `run_frame(); render();` loop a uniform border every time, while passing every test that renders mid-frame. It did, and that gate is what caught it |
//! | That a frame with **no** border write is unchanged | `tests/border_stripes.rs` — byte-identical to what [`screen::render`] draws with one colour, over all 81920 pixels. That is what keeps every other screen gate meaning what it meant |
//! | The border's **resolution** | **one rendered row, derived rather than chosen.** Vertically the frame buffer maps to hardware exactly; horizontally it does not, since [`screen::BORDER`] is a uniform 32 px where the hardware's is not — so a T-state-to-column mapping would invent precision the buffer cannot carry. Measured against the effect that prompted it: the real ROM's loader changes the border every **1884–2159 T-states**, a band every **8.4 to 9.6 rows**, and the rendered result is 27 bands of 9–10 rows |
//! | A border change **within** a line | **nothing, and it cannot be shown.** Border-multicolour demos rewrite `0xFE` every 8–24 T-states; every one of those writes lands in the same row here and the last before the row begins is the one it gets. `several_writes_inside_one_row_collapse_to_the_last` gates the collapse so it is a known cost |
//! | That a loading screen **looks right** | **nothing.** `the_roms_own_loader_paints_more_than_one_band` runs the real ROM's `LD-BYTES` against a real tape and asserts the bands are present, more than one, and of the thickness the measurement predicts. That is a long way short of the appearance, which is observation |
//! | *Which* value inside the 48K's measured band — the 32 [`timing::INTERRUPT_T_STATES`] ships | **nothing, and the oracle is what says so rather than an excuse for it.** All sixteen of `17..=32` are green, so the constant sits at the top of a band nothing here separates: 32 is not distinguished from 17 by any evidence this project holds. It is the community's figure, pinned against drift. What the sweep did establish is the negative — **33 is refuted**, by the row that decides the whole suite's class — and one band, **14–16**, was not predicted and is recorded as unexplained rather than smoothed into the floor. See [`timing::Timing::SPECTRUM_48K`]. *(This row read "The 32 T-state interrupt window's **length** — **nothing**, pinned against drift, never measured", which the row naming `tests/timing_oracle.rs` and the band `17..=32` already contradicted: the length **is** hardware-graded, as a band. Two verdicts for one property is the same defect as a stale "No oracle" elsewhere in this table, and a table whose worth rests on its honest **nothing** rows cannot afford one that is false. What survives is the narrower claim that is still true — the band is measured, the point inside it is not.)* |
//! | I/O contention through a real `Cpu<Ula>` | `tests/io_contention.rs` — real `IN A,(n)`, `OUT (n),A` and `IN A,(C)` instructions, three forms × four ports × eight contention phases. It was **nothing** until that gate landed; [`ula`]'s unit tests synthesise the tick stream by hand and grade the rule, not the wiring |
//! | Keyboard ghosting / rollover | **nothing** — not modelled |
//! | `.z80` and `.sna` **header offsets** | `tests/snapshot_vectors.rs` — a file transcribed byte by byte from the format description, with its expected state written separately. **No round trip covers this**, and that is the point: a field read from the wrong offset by both the parser and the writer survives every round trip |
//! | The `.z80` run-length codec | `decompress(compress(page)) == page` as a property test in [`snapshot`], with the format description's own example (`ED 00 ED ED 05 00`) as the asymmetric anchor |
//! | The `.z80` version 3 frame-position counter | exhaustive over every position of **both** frames — 69888 and 70908 — **which cannot see a wrong formula**; plus **six** positions per machine derived by hand from the format description's sentence, `libspectrum`'s independent expressions transcribed with `quarter_states` left as the parameter of the machine it is there, and an `assert_ne!` that the two machines encode frame position **0** differently. That last one is the assertion a single-model loop structurally could not contain, and it is the one the shipped defect — a 128's frame divided into the 48K's quarters at both ends — would have failed while every round trip stayed green. **The arithmetic itself is still not covered**: every instrument here is a transcription of one paragraph or an independent implementation of that same paragraph, and nobody has loaded a file this emulator wrote into another emulator. That is observation, and it is not done. *(This row has now understated the same gate twice, both times by reading half of a sentence whose every number comes in pairs — first "three" while `the_counter_matches_the_format_descriptions_own_sentence` asserted six, then "six over 69888" after `MODELS` made it six per machine over two frames. The two copies this parenthetical went on to name are now closed, and it is corrected rather than deleted because the count of times this row has been wrong is the row's most useful number.* `encode_t_states` *heads its hand-worked list* "Six positions per machine computed by hand from the sentence above" *and scopes its sweep to* "every position of both frames — 69888 on a 48K and 70908 on a 128", *carrying its M6 verdict at the 48K-only range that verdict was taken on; and* `docs/M6.md`*'s mutation-verdict table keeps* "The three positions derived by hand" *under a framing paragraph that dates it, which is the treatment a dated verdict wants — restating one against a range it never ran on asserts a run nobody made. What found those copies is the part worth carrying forward: each round of this correction grepped for the number it was replacing, and a search for either figure returns only the copies that already agree with it — the survivors said* three *where the new figure was* six, *or* six *with no per-machine qualifier. The claim's noun —* "derived by hand", "exhaustive sweep" *— is what a renumbering leaves alone, and grepping that returned every copy in one pass.)* |
//! | The parsers on hostile input | `tests/snapshot_hostile.rs` — an exhaustive truncation sweep, a single-byte mutation sweep, two `proptest`s, and one case per row of `docs/M6.md`'s hostile-input table. The **structural** half is asserted in [`snapshot`] itself: no indexing expression anywhere in the module, and no allocation sized from the file |
//! | Our **reading of the two formats** | `tests/snapshot_corpus.rs` — third-party files from three independent emulators, and a `.z80`/`.sna` pair a third party saved from one machine, which is the only external grading `.sna` gets. Corpus-dependent: absent by default, through `crates/testsupport`'s shared policy |
//! | Whether our **arithmetic** on a format's fields is right | **not the corpus**, and that was measured: under a symmetric mutation of the T-state formula the whole corpus sweep stayed green, because everything it asserts about a foreign file is symmetric in the same way. A foreign file proves a field is *readable*. Only loading one of **our** files in another emulator would settle the arithmetic, and that is observation, not automated, and **not done** |
//! | `.sna` offsets, without a corpus | **nothing beyond the transcribed vector.** There is no `.sna` writer — [`snapshot::sna`] explains why — so the format has **no round trip at all** |
//! | Applying a `Snapshot` to a running machine | `tests/snapshot_apply.rs` — **R1** (`snapshot(restore(s)) == s`, format-free and corpus-free) and **R3** (`write(snapshot(restore(parse(f)))) == f`, over bytes). R1 is structurally immune to the permutation R2 fears, because its two halves share no field map: `restore` writes through `Cpu::set_state` and `snapshot` reads through `Cpu::state` |
//! | Which **address** a restored bank lands at | **not** R1 or R3 — measured: inverting the bank→address map in *both* halves leaves every round trip green. `a_restored_bank_lands_where_the_48k_memory_map_says` is the anchor, and its addresses are the 48K's published map |
//! | That a restore charges **no machine cycle** | `a_restore_leaves_no_machine_cycle_half_open` and `a_restore_does_not_move_the_tape`. Both were needed: routing the border through `Bus::out_port` turns exactly these two red and leaves R1, R3 and everything else green |
//! | `frames()` across a load | `a_restore_does_not_rewind_the_machines_uptime`, plus unit tests in [`timing`] and [`ula`]. A **convention**, not a measurement — no format carries a frame count |
//! | The tape's **pulse timings** | `tests/tape_rom_timings.rs` — the ROM's own `SA-BYTES` run on the machine and its `MIC` edges measured, compared to our converter's train for the same bytes, contention removed so the comparison is an equality. 3305 half-periods, two implementations, no shared code. It found two deviations the derivation had not predicted and one it had got wrong |
//! | The tape's timings against **hardware** | **nothing.** The ROM defines what a `.tap` means and every loader was written against it, but nobody here has measured a real Spectrum |
//! | The `.tap` **block framing** | `tests/tape_corpus.rs` — third-party tapes, decoded back out of the pulse train and checked against **somebody else's parity byte** over 14300-byte blocks. Corpus-dependent, through `crates/testsupport`'s shared policy |
//! | The pulse train's **shape** | `tests/tape_signal.rs` — a one-byte block written out by hand from the format's rule, a decoder this project wrote reading it back, and a `proptest` over arbitrary payloads |
//! | The `EAR` bit reaching the CPU | `tests/tape_rom_load.rs` — **the M6 gate.** The real ROM's `LD-BYTES` loads a tape through bit 6 of port `0xFE` (**T2**), and a program we wrote loads from tape and then *computes* a value that appears nowhere in its own bytes (**T3**). `no_shortcut_exists_past_the_ear_bit` is the standing assertion that nothing supplies a byte by any other route |
//! | The tape seeing **contention** | `the_tape_advances_by_contention_as_well_as_by_ticks` and a unit test in [`ula`]. Measured: a tape driven from `Bus::tick` alone leaves both red and **leaves the ROM load gates green**, because the loader's thresholds are hundreds of T-states wide |
//! | Contention *during* a load | **nothing.** It is exercised on every one of the loader's thousands of port reads and it is not graded — the loader does not depend on it |
//! | Timing **precision** on a load | **nothing.** Measured: one T-state wrong in a sync pulse, one in the pilot period, or one pulse missing from the pilot tone all leave T2 and T3 green. That is what `tests/tape_rom_timings.rs` is for, and it is why T2 grades the mechanism rather than the numbers |
//! | Issue 2 / issue 3 `EAR` readback | **nothing** — not modelled; writing bit 3 or 4 does not change what bit 6 reads. A real cause of *"loads on one emulator and not another"* |
//! | The `EAR` sampling point within an `IN` cycle | **nothing.** Approximated to the start of the cycle, ≤4 T-states early — far inside the ROM's tolerance, and a turbo loader failing is what would decide it |
//! | Turbo loaders, and a real game | **nothing.** `.tap` cannot represent a turbo loader at any speed; `.tzx` can and is deferred. A real game is T4 — observation, corpus-dependent, and **not done** |
//! | `CpuState::wz`, `CpuState::q`, `halted` across a load | **no format carries them.** Enumerated in [`snapshot::UNPRESERVED`] and each one proven to be dropped, because a field dropped in *both* directions is the one defect a round trip is green for |
//! | The **beeper** reaching the speaker — bit 4 of a `0xFE` write | `tests/m7_beeper.rs` — a guest executing real `OUT`s, and the sample stream compared **by value** against a T-state derivation written before it was run. `ula.rs:528` carried an open finding for a milestone saying this bit was dropped; this is what closed it |
//! | That a **border** write does not click | `tests/m7_beeper.rs`'s negative control: the same program with one byte changed. Without it, a model driving the speaker from any `0xFE` write, or from the wrong bit, passes the positive case exactly as well |
//! | `MIC` — bit 3 of a `0xFE` write | **nothing** — not modelled. It is tape *save*, which no milestone has claimed |
//! | The `0xFE` port's **four output levels** | **nothing** — not modelled. On real hardware `MIC` shifts the speaker's level; the shift is a magnitude with no source this project can adjudicate, so the speaker is two-level. See [`ula`] |
//! | The AY reached from a guest's `OUT`/`IN` | `tests/m7_ay_ports.rs` — the select latch, the data write, the read-back, and that a 48K answers neither port. The read arm sits *before* the floating-bus fallback, and placed after it the machine would read as "the chip is write-only" |
//! | The AY's **address decode** — which lines select `0xFFFD` and `0xBFFD` | **nothing, and no source for it was found.** Inferred from the two addresses and from the `0x7FFD` decode's style; `docs/M7.md` calls it that design's least-supported claim, and `ula::AY_PORT_MASK` repeats the label where the constant is. The family gates in `tests/m7_ay_ports.rs` grade this crate against the inference and **cannot** discover the inference is wrong |
//! | The AY's **noise period** | `ay`'s own tests — run the register and count. It needs no source at all, and its **blind spot is enumerated rather than described**: sweeping all sixteen tap positions, a period test kills ten and cannot tell the remaining six (3, 5, 6, 11, 12, 14) apart. The shipped tap is one of the six |
//! | The AY's **envelope aliasing** — sixteen shape values, eight behaviours | `ay`'s own tests, and **derived rather than tabulated**: [`ay`] implements the four `CONT`/`ATT`/`ALT`/`HOLD` bits, so the collapse into two blocks of four emerges and the test grades the decode. *Which* two upper shapes they alias onto — 9 and 15 — is a transcription, asserted in a separate test so the two claims are not read as one |
//! | The AY's **counter periodicity** | `ay`'s own tests, exhaustive over all 4096 tone values, and `tests/m7_ay_stream.rs` for the same claim surviving the trip into the sample stream |
//! | The AY's **mixer polarity** — active low | `ay`'s own tests and `tests/m7_ay_stream.rs`. A boolean rather than a magnitude, and the distinction the model must keep is that a disabled channel sits at its *level* — silence to a speaker, and not the same statement as "the amplitude is zero" |
//! | The AY's **register write masks** | **graded against the transcription**, in `ay` and again through a guest's own `OUT`/`IN` in `tests/m7_ay_ports.rs`. Worth having despite that, because software reads registers back, so a wrong mask is a wrong value returned to a guest |
//! | The AY's **magnitudes** — the volume table, the tone/noise/envelope divisors, the AY:CPU clock ratio | **nothing, and nothing here can.** Each carries its source in its own doc comment and says so. Only the table's *structure* is asserted — monotonic, silence at 0, full scale at 15 — which a reordered or truncated transcription fails and a wrong fourth digit does not |
//! | `R15`, which `.z80` v3 reserves and the `-8912` does not have | `tests/m7_ay_ports.rs`, **not** a round trip — which is blind to it by construction. [`ay::Ay::register`] returns `None` for it and a guest's read gets the floating bus. The `.z80` writer's byte 54 is ruled in [`snapshot`] and is unreachable until the 128 hardware modes land |
//! | The **sample stream**'s shape | `tests/m7_ay_stream.rs` — a tone's wave period in samples, derived from the two constants; the envelope as sixteen strictly monotonic levels; the noise varying and not repeating at any short period |
//! | That the stream is **deterministic** | `tests/m7_ay_stream.rs` and [`audio`]'s own tests. It is what every hash below rests on: a generator whose output depended on when it was asked would make each of them a gate on the consumer's call pattern instead |
//! | Whether the AY's numbers are **right** | **nothing.** `tests/m7_ay_stream.rs`'s frame hash proves **change** — `docs/MACHINE.md`'s verification item 4, *"does not prove correctness"* — and it is the only recorded-rather-than-derived number in this suite. Its positive control is what keeps it from being a hash of nothing |
//! | That the AY's hash cannot be falsified by the **beeper** | `tests/m7_ay_stream.rs` — the constraint `docs/M7.md` Decision 6 imposes, as a failing case. The two sources are separate fields of a [`Sample`] and are mixed nowhere in this crate |
//! | The AY's **generator** state across a load — tone phases, the noise register, the envelope's position | **no format carries them**, so a restore starts them from power-on. A convention chosen for determinism, documented at [`ay::Ay::restore`] and asserted in `tests/snapshot_apply.rs`. It is **not** in [`snapshot::UNPRESERVED`], which is scoped to `CpuState` fields |
//! | The chip's registers across a load | `tests/snapshot_apply.rs` — R1 over a 128, **and** a direct assertion that they travel, because a `snapshot` that never wrote them and a `restore` that never read them would leave R1 entirely green |
//! | AY write **timing within the frame** | **nothing.** Music drivers write the chip from the interrupt handler and the audible result depends on when the writes land; no gate measures it |
//! | The **sample rate** | **derived.** [`timing::Timing::cpu_hz`] divided by [`audio::SAMPLE_PERIOD_T_STATES`]. Both clocks are transcriptions; what is checked is that each implies a 50 Hz frame against its own frame length, which catches a transposed digit and establishes neither figure |
//! | What **sound costs on the hot path** | **measured, and it is nothing.** [`Ula::tick`], `Ula::contend` and `Ula::advance` are byte-identical to their pre-M7-sound selves — nothing generates audio per T-state. `benches/frame.rs` is the command that exists to keep it that way, and it is the bench `docs/M7.md` Decision 3 recorded as missing |
//! | Whether it **sounds right** | **nothing.** A human ear, `docs/M7.md`'s T4, and the honest bottom of this table |
//! | The **Kempston joystick** reached from a guest's `IN` | `tests/kempston.rs` — five switches through a real `IN A,(0x1F)`, on both machines, with the bit layout as five transcribed literals. It also gates that the joystick and the keyboard cannot disturb each other, which is the whole reason a frontend maps arrow keys to a port rather than to the membrane |
//! | The joystick's **active-high** convention | `tests/kempston.rs` — asserted against the membrane's active-low in one comparison, because a Kempston modelled the keyboard's way round reads `0x1F` idle: every direction and fire held, forever |
//! | The Kempston **decode** | **nothing, and no source for it was found.** [`joystick::KEMPSTON_PORT_MASK`] matches the canonical address's low byte and deliberately claims nothing about address lines — narrow on purpose, because a decode wider than the evidence takes ports from devices not yet written, and the Beta Disk's FDC register is already known to sit at this address |
//! | What the joystick's **unused top three bits** read as | **nothing.** Zero here; a bus buffer driving five bits would float the other three. A game misbehaving with the stick idle is the observation that would decide it |
//! | That a real **game** responds to the joystick | **nothing** — T4, a person and a look |
//! | A **128 snapshot** surviving a file | `tests/snapshot_apply.rs` — R3 over a 128, all eight banks fingerprinted, and the chip's registers at their transcribed offsets. **This was wrong from M7 until now**: the writer emitted hardware mode `0` for every model, so a 128 became a 48K file carrying three of its eight banks. The information was never missing — `Snapshot::model` existed for exactly this — and the gate's absence is why it survived |
//! | That teaching the writer about the 128 moved **no 48K byte** | `tests/snapshot_apply.rs` — and it caught a real regression: a first cut iterated banks in ascending order where the canonical form is address order, silently reordering the page blocks of every 48K file |
//! | A **128 file missing banks** | `tests/snapshot_hostile.rs` — refused. M6's guard compared the parser's bank set against the *slot map*; on a 128 that premise dissolves, so it compares against the **model**'s set instead, and it is scoped to the 128 because a 48K's three banks are always addressable and a missing one is visible without it |
//!
//! The rows reading **nothing** are the point of this table. `docs/MACHINE.md` asks for what
//! is *not* covered to be written down rather than inferred from the absence of a failing
//! test, and those are the answers.

#![deny(missing_docs)]

pub mod audio;
pub mod ay;
pub mod joystick;
pub mod keyboard;
pub mod memory;
pub mod model;
pub mod screen;
pub mod snapshot;
pub mod tape;
pub mod timing;
pub mod ula;

pub use audio::Sample;
pub use ay::Ay;
pub use joystick::Joystick;
pub use keyboard::{Key, Keyboard};
pub use memory::{Memory, RomSizeError};
pub use model::Model;
pub use screen::{Colour, Frame};
pub use snapshot::{ModelMismatch, Snapshot};
pub use tape::Tape;
pub use ula::{FLOATING_BUS_BYTE, Ula};

use memory::BankIndex;
use z80::{Cpu, CpuState, StepError};

/// A ZX Spectrum — a 48K or a 128.
///
/// Owns the CPU, which owns the [`Ula`], which owns the [`Memory`]. That chain is the
/// `crates/z80` ownership model rather than a choice made here: the core takes its bus by
/// value so the calls monomorphise, and reaches it back out through
/// [`z80::Cpu::bus_mut`].
///
/// # One type, because a 48K *is* a 128 that powered on with the lock set
///
/// There is no `Spectrum128`, no model generic and no trait. The two machines differ in the
/// contents of fields that already existed — the slot map, the contended-bank array, the frame
/// geometry — and in one new byte, the paging port. `M7.md` Decision 1 makes that an equation
/// rather than a slogan: port value `0x20` derives a 48K's map exactly, and its inability to
/// page is exactly the lock bit already being set, which [`memory`] asserts at compile time.
///
/// The alternative costs more than it looks. A sibling type duplicates **this file's frame
/// loop** — the part of the crate with the subtlest rules in the project, `MACHINE.md`
/// Decisions 1 and 2 — and a second copy is a second place to get them wrong, with the copy's
/// failure invisible until a 128 game runs 20 % long and nothing fails. A `Ula<M: Model>`
/// generic is worse still: it buys runtime polymorphism nothing needs, since a machine does not
/// change model mid-run, and pays by monomorphising the whole `Cpu<Ula<M>>` instantiation
/// twice.
#[derive(Debug)]
pub struct Spectrum {
    cpu: Cpu<Ula>,
}

impl Spectrum {
    /// Build a 48K holding `rom`, at the start of frame zero.
    ///
    /// Unchanged by M7 in signature and in meaning: this already meant a 48K, because a 48K
    /// was the only machine there was.
    ///
    /// # Errors
    ///
    /// [`RomSizeError`] if `rom` is not exactly one 16 KB page.
    pub fn new(rom: &[u8]) -> Result<Self, RomSizeError> {
        Ok(Self {
            cpu: Cpu::new(Ula::new(Memory::spectrum_48k(rom)?)),
        })
    }

    /// Build a 128 holding both its ROMs, at the start of frame zero.
    ///
    /// `editor` is the 128's own ROM — page 0, the one it resets into and the one that prints
    /// `© 1986 Sinclair Research Ltd` — and `basic` is the 48 BASIC ROM its menu's *48 BASIC*
    /// entry selects with bit 4 of `0x7FFD`.
    ///
    /// The machine starts with paging **live**: `0x7FFD` reads as `0x00`, so bank 0 is at
    /// `0xC000`, the screen is bank 5 and the editor ROM is selected. Everything that follows
    /// is the guest's business.
    ///
    /// # Errors
    ///
    /// [`RomSizeError`] if either image is not exactly one 16 KB page.
    pub fn spectrum_128(editor: &[u8], basic: &[u8]) -> Result<Self, RomSizeError> {
        Ok(Self {
            cpu: Cpu::new(Ula::new(Memory::spectrum_128(editor, basic)?)),
        })
    }

    /// Which machine this is.
    ///
    /// # Public because the alternative copies 48 KB to read an enum
    ///
    /// `docs/M7.md` listed `Spectrum::model()` under *deliberately absent* — *"nothing asks"* —
    /// and something now does. The only route that compiled without it was
    /// `machine.snapshot().model()`, which clones **every RAM bank** to read one discriminant:
    /// 48 KB on a 48K and 128 KB on a 128, per call.
    ///
    /// Additive, and `Model` is already `#[non_exhaustive]`, so the semver cost is the
    /// addition and nothing more.
    #[must_use]
    pub fn model(&self) -> Model {
        self.cpu.bus().memory().model()
    }

    /// Press the reset button: CPU and ULA back to their power-on state, RAM untouched.
    pub fn reset(&mut self) {
        self.cpu.set_state(CpuState::default());
        self.cpu.bus_mut().reset();
    }

    /// Offer the interrupt if the ULA is raising one, then run one instruction.
    ///
    /// Returns the T-states the CPU charged. **That number is not the clock** — contention
    /// is added on the bus's side and is not included. It is returned because the CPU
    /// returns it, and it is useful for asserting an instruction's nominal length; use
    /// [`Spectrum::frame_t_state`] for time.
    ///
    /// The interrupt is *offered*, not forced: [`z80::Cpu::interrupt`] declines while
    /// `iff1` is clear or the `EI` window is open, and returns zero having changed nothing.
    /// The acceptance rule lives there and is deliberately not repeated here — including
    /// as a "once per frame" guard, which would be exactly that duplication.
    pub fn step(&mut self) -> u32 {
        if self.cpu.bus().interrupt_asserted() {
            let accepted = self.cpu.interrupt(FLOATING_BUS_BYTE);
            if accepted != 0 {
                return accepted;
            }
        }
        self.cpu.step()
    }

    /// Run until the frame counter advances.
    ///
    /// The loop watches the counter rather than a T-state budget, which is what makes an
    /// instruction that overruns the frame a non-event: it lands in the next frame and the
    /// overshoot is carried, exactly as it is on the hardware — including the case where
    /// the overshoot is long enough to miss the following interrupt.
    pub fn run_frame(&mut self) {
        let target = self.frames() + 1;
        while self.frames() < target {
            // Deliberately discarded: the frame is driven by the bus's tick count, never
            // by summing what `step` returns. See the crate documentation.
            let _ = self.step();
        }
    }

    /// Run `count` frames.
    pub fn run_frames(&mut self, count: u64) {
        for _ in 0..count {
            self.run_frame();
        }
    }

    /// Frames completed since power-on.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.cpu.bus().clock().frames()
    }

    /// T-states elapsed since the start of the current frame, contention included.
    #[must_use]
    pub fn frame_t_state(&self) -> u32 {
        self.cpu.bus().clock().frame_t_state()
    }

    /// Draw the current screen into `frame`.
    ///
    /// A snapshot of the screen as it stands now, not a record of what the ULA drew during
    /// the frame — see [`screen`] for what that costs.
    pub fn render(&self, frame: &mut Frame) {
        let ula = self.cpu.bus();
        screen::render_border_trace(ula.memory(), ula.border_trace(), self.frames(), frame);
    }

    /// The complete CPU state — what a `.z80` or `.sna` snapshot carries.
    #[must_use]
    pub fn cpu_state(&self) -> CpuState {
        self.cpu.state()
    }

    /// Replace the complete CPU state.
    pub fn set_cpu_state(&mut self, state: CpuState) {
        self.cpu.set_state(state);
    }

    /// What stopped the most recent operation short, if anything.
    ///
    /// Should always be `None` on a Spectrum: the only fault the core can raise is a mode 0
    /// interrupt supplying a byte that is not an `RST`, and this machine's bus floats to
    /// `0xFF`, which is `RST 38h`. A fault here is a finding, not a condition to handle.
    #[must_use]
    pub fn fault(&self) -> Option<StepError> {
        self.cpu.fault()
    }

    /// The ULA.
    #[must_use]
    pub fn ula(&self) -> &Ula {
        self.cpu.bus()
    }

    /// The ULA, mutably.
    pub fn ula_mut(&mut self) -> &mut Ula {
        self.cpu.bus_mut()
    }

    /// The address space.
    #[must_use]
    pub fn memory(&self) -> &Memory {
        self.cpu.bus().memory()
    }

    /// The address space, mutably.
    pub fn memory_mut(&mut self) -> &mut Memory {
        self.cpu.bus_mut().memory_mut()
    }

    /// The keyboard.
    #[must_use]
    pub fn keyboard(&self) -> &Keyboard {
        self.cpu.bus().keyboard()
    }

    /// The keyboard, mutably — how a frontend or a test presses a key.
    pub fn keyboard_mut(&mut self) -> &mut Keyboard {
        self.cpu.bus_mut().keyboard_mut()
    }

    /// The Kempston joystick.
    #[must_use]
    pub fn joystick(&self) -> Joystick {
        self.cpu.bus().joystick()
    }

    /// The Kempston joystick, mutably — how a frontend pushes it.
    ///
    /// The cleanest of the three ways a Spectrum game can be steered, and the only one that
    /// cannot collide with the keyboard: it is a port rather than part of the membrane, so a
    /// frontend mapping arrow keys to it does not have to know which letters the game itself
    /// reads. See [`joystick`].
    pub fn joystick_mut(&mut self) -> &mut Joystick {
        self.cpu.bus_mut().joystick_mut()
    }

    /// The colour last written to the border.
    #[must_use]
    pub fn border(&self) -> Colour {
        self.cpu.bus().border()
    }

    /// The sound chip, on a machine that has one.
    ///
    /// `None` on a 48K, which does not contain it. The `Option` is the machine's shape and
    /// not a caller's convenience: a 48K's `OUT (0xBFFD)` reaches nothing and its
    /// `IN (0xFFFD)` floats, and an accessor that returned a chip anyway would be asserting
    /// hardware the machine does not have.
    #[must_use]
    pub fn ay(&self) -> Option<&Ay> {
        self.cpu.bus().ay()
    }

    /// Generate the sound up to now and hand over everything since the last call.
    ///
    /// The consumer's whole job: call it once a frame and copy what it returns. It borrows a
    /// buffer allocated once at construction and refilled in place, so a frontend draining
    /// every frame allocates nothing at all — which is what `docs/M8.md` asks for.
    ///
    /// **The AY's three channels and the beeper arrive separate.** Mixing them is the
    /// frontend's job, by `docs/M8.md` Decision 9, and keeping them apart is also what makes
    /// the AY's own gate immune to the beeper — `docs/M7.md` Decision 6.
    ///
    /// See [`Spectrum::dropped_samples`] for what happens to a consumer that calls it less
    /// often than that.
    pub fn take_samples(&mut self) -> &[Sample] {
        self.cpu.bus_mut().take_samples()
    }

    /// Samples lost because [`Spectrum::take_samples`] was not called often enough.
    ///
    /// Zero for a consumer draining once a frame; the buffer holds two of the longest frame
    /// this crate models. It is counted rather than swallowed because an unexplained gap in
    /// audio gets blamed on everything except the buffer that caused it.
    #[must_use]
    pub fn dropped_samples(&self) -> u64 {
        self.cpu.bus().dropped_samples()
    }

    /// This machine's whole state, as a value a snapshot format can encode.
    ///
    /// The CPU, the border, the frame position, the model and its paging port, and **every RAM
    /// bank the machine has**. ROM is not carried: no format carries it, and a machine that
    /// loaded one would be loading somebody else's ROM.
    ///
    /// The frame *counter* is not carried either — see [`Spectrum::restore`].
    ///
    /// # Why this reads banks and not addresses
    ///
    /// It used to walk the slot map and read through the addresses each bank appeared at. That
    /// is correct on a 48K, where all three banks always have an address, and it captures
    /// **three banks of eight** on a 128, where five of them typically have no address at all.
    /// [`Memory::bank`] is what makes the difference, and the model is what says how many there
    /// are — not the slot map, whose premise dissolves the moment paging exists.
    ///
    /// The paging port goes with them, because eight banks without the map that arranges them
    /// do not describe a machine that can be restored.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let ula = self.cpu.bus();
        let memory = ula.memory();
        let mut snapshot =
            Snapshot::new(self.cpu.state(), ula.border(), ula.clock().frame_t_state());
        snapshot.set_model(memory.model(), memory.paging_port());
        for &bank in memory.model().banks() {
            let bank = BankIndex::new(bank);
            snapshot.set_bank(bank, Box::new(*memory.bank(bank)));
        }
        if let Some(ay) = ula.ay() {
            snapshot.set_ay(ay);
        }
        snapshot
    }

    /// Put `snapshot`'s state into this machine.
    ///
    /// **Nothing is stepped, nothing is ticked, and no contention is charged.** A restore is
    /// not a machine cycle, so it goes through [`Ula`]'s own setters rather than through the
    /// bus — `docs/M6.md` Decision 2. Two things go wrong if it does not, and both were
    /// measured rather than reasoned about: a port cycle **advances the clock by its
    /// contention stall, and the tape advances with the clock**, so a restore would move the
    /// head; and it leaves the ULA's cycle bookkeeping armed for four T-states, which the next
    /// **bare tick** — an interrupt acknowledge is seven of them, with no transfer in between —
    /// would spend on contention it owes.
    ///
    /// Three things deliberately survive the load, and each is a convention rather than a
    /// measurement because no format carries the field:
    ///
    /// - **[`Spectrum::frames`]** — the machine's uptime. The boot gate asserts on it and the
    ///   FLASH phase derives from it, so rewinding it would make one number mean two things.
    ///   The visible cost is a snapshot taken mid-flash rendering inverted for up to
    ///   [`screen::FLASH_FRAMES`] frames after loading.
    /// - **The ROM**, which no format carries.
    /// - **The tape**, because loading a snapshot does not eject a cassette.
    ///
    /// # Why it is fallible, and why the rule is symmetric
    ///
    /// **This is M7's one breaking change**, and both directions of the refusal earn it. A 128
    /// snapshot restored into a 48K has five banks with nowhere to go, and dropping them
    /// silently is the *"silent last-write-wins"* `docs/M6.md` refused for duplicate pages. The
    /// other direction is broken too and is the quieter half: a 48K image carries no paging
    /// byte, so restoring one into a 128 leaves 48K code running against the **128 editor
    /// ROM** — a machine that looks loaded and executes the wrong ROM. See [`ModelMismatch`].
    ///
    /// # Why banks are written through [`Memory::bank_mut`] and the map through
    /// `Memory::set_paging_port`
    ///
    /// It used to write each bank through the addresses the slot map showed it at, and skip any
    /// bank the map did not show. On a 128 that would silently drop five of eight — the exact
    /// defect the refusal above exists to prevent, reappearing one layer down. And the paging
    /// port cannot go through `Ula`'s `out_port` either: the machine being restored **into** may
    /// already be locked from whatever it was running before, so a guest-facing write would be
    /// discarded and every field a round trip compares would still match.
    ///
    /// # Errors
    ///
    /// [`ModelMismatch`] if `snapshot` describes a different machine. Nothing is changed when
    /// it does: the check runs before anything is written, so a refused restore leaves the
    /// machine exactly as it was rather than half-loaded.
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<(), ModelMismatch> {
        let machine = self.model();
        if snapshot.model() != machine {
            return Err(ModelMismatch {
                snapshot: snapshot.model(),
                machine,
            });
        }

        self.cpu.set_state(snapshot.cpu);
        let ula = self.cpu.bus_mut();
        ula.set_border(snapshot.border);
        ula.set_frame_t_state(snapshot.frame_t_state);
        ula.memory_mut().set_paging_port(snapshot.paging_port());

        for (bank, page) in snapshot.banks() {
            *ula.memory_mut().bank_mut(bank) = *page;
        }
        // The model check above already guarantees these agree: a snapshot with a chip is a
        // 128's and so is a machine with one. Written as a pair anyway, because a `let else`
        // that silently skipped would be indistinguishable from a restore that dropped the
        // sound chip — which is the same shape as the five banks the refusal exists to catch.
        if let (Some(state), Some(ay)) = (snapshot.ay(), ula.ay_mut()) {
            ay.restore(state.selected, &state.registers);
        }
        Ok(())
    }

    /// Put a tape in the drive, stopped. [`Spectrum::tape_mut`] is how it is started.
    pub fn insert_tape(&mut self, tape: Tape) {
        self.cpu.bus_mut().insert_tape(tape);
    }

    /// The tape in the drive — how a frontend or a test plays, stops or rewinds it.
    ///
    /// With nothing inserted this is a blank tape rather than a `None`: a drive with no
    /// cassette and a cassette with nothing on it drive the `EAR` line identically, so there
    /// is no state for a caller to distinguish and no option for it to unwrap.
    pub fn tape_mut(&mut self) -> &mut Tape {
        self.cpu.bus_mut().tape_mut()
    }
}

// `exposed_banks`, `slot_address`, `slot_base` and `PAGE_SIZE_U16` used to live here: the
// snapshot writer and the applier both matched banks to addresses through the slot map, and
// walked pages with a `u16` offset. `Memory::bank`/`bank_mut` replace all four — the map is the
// wrong question on a 128, where a bank the map shows nowhere is still part of the machine —
// and they are deleted rather than left, because a private helper with no caller is
// indistinguishable from one whose caller was lost.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PAGE_SIZE;
    use crate::timing::{INTERRUPT_T_STATES, T_STATES_PER_FRAME};

    /// A ROM of `NOP`s: the CPU runs off the end of it into RAM and wraps, forever.
    fn nop_machine() -> Spectrum {
        Spectrum::new(&[0x00; PAGE_SIZE]).expect("a page-sized ROM")
    }

    #[test]
    fn a_rom_of_the_wrong_size_is_refused() {
        assert!(Spectrum::new(&[0x00; 100]).is_err());
    }

    #[test]
    fn a_fresh_machine_starts_at_the_top_of_frame_zero() {
        let machine = nop_machine();
        assert_eq!((machine.frames(), machine.frame_t_state()), (0, 0));
        assert_eq!(machine.cpu_state().pc, 0);
    }

    #[test]
    fn the_frame_loop_is_driven_by_the_bus_and_not_by_step_returns() {
        // MACHINE.md Decision 1, as a failing case rather than as prose. Running from the
        // ROM the two accounts agree, because nothing there is contended...
        let mut machine = nop_machine();
        let mut charged = 0_u32;
        while machine.frames() == 0 {
            charged += machine.step();
        }
        assert_eq!(machine.frames(), 1);
        assert_eq!(
            charged,
            T_STATES_PER_FRAME + machine.frame_t_state(),
            "uncontended, `step` returns and the bus clock must agree exactly"
        );

        // ...and running the same instructions from the contended bank they do not. A
        // machine that summed `step` returns would run this frame roughly 20% long, and
        // no test anywhere would fail.
        let mut machine = nop_machine();
        let mut state = machine.cpu_state();
        state.pc = 0x4000;
        machine.set_cpu_state(state);
        let mut charged = 0_u32;
        while machine.frames() == 0 {
            charged += machine.step();
        }
        assert!(
            charged < T_STATES_PER_FRAME,
            "contention adds time on the bus's side, so the CPU must charge less than a \
             frame ({charged} of {T_STATES_PER_FRAME})"
        );
    }

    #[test]
    fn one_frame_of_nops_takes_the_whole_frame_budget() {
        let mut machine = nop_machine();
        machine.run_frame();
        assert_eq!(machine.frames(), 1);
        assert!(
            machine.frame_t_state() < 4,
            "the overshoot should be less than one NOP, not {}",
            machine.frame_t_state()
        );
    }

    /// Enable interrupts in `mode`, leaving everything else as it stands.
    fn enable_interrupts(machine: &mut Spectrum, mode: z80::InterruptMode) {
        let mut state = machine.cpu_state();
        state.iff1 = true;
        state.iff2 = true;
        state.im = mode;
        state.sp = 0xFF00;
        machine.set_cpu_state(state);
    }

    #[test]
    fn an_interrupt_is_declined_while_the_cpu_has_them_disabled() {
        // Power-on state is `iff1` clear, so every offer this frame is refused: the
        // machine keeps executing, and nothing is pushed.
        let mut machine = nop_machine();
        machine.run_frame();
        assert!(!machine.cpu_state().iff1);
        assert_eq!(machine.cpu_state().sp, u16::MAX, "nothing was pushed");
        assert!(
            machine.cpu_state().pc > 0x1000,
            "the CPU should have run a frame of NOPs, not vectored"
        );
        assert_eq!(machine.fault(), None);
    }

    #[test]
    fn an_enabled_interrupt_is_accepted_at_the_top_of_the_frame() {
        // Run a frame with interrupts off, so the acceptance under test is the one at the
        // start of frame 1 rather than the one waiting at power-on.
        let mut machine = nop_machine();
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);
        assert_eq!(machine.frames(), 1);

        machine.step(); // the first step of the new frame sees `/INT` low

        assert_eq!(
            machine.cpu_state().pc,
            0x0038,
            "mode 1 vectors to 0x0038 and the machine must not reimplement the rule"
        );
        assert!(
            !machine.cpu_state().iff1,
            "acceptance clears both flip-flops"
        );
        assert!(!machine.cpu_state().iff2);
        assert_eq!(
            machine.cpu_state().sp,
            0xFEFE,
            "the return address was pushed"
        );
    }

    #[test]
    fn the_interrupt_is_a_window_and_an_offer_past_it_is_not_made() {
        let mut machine = nop_machine();
        while machine.frame_t_state() < INTERRUPT_T_STATES {
            machine.step();
        }
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);

        let before = machine.cpu_state().pc;
        machine.step();
        assert_ne!(machine.cpu_state().pc, 0x0038, "the line is no longer low");
        assert!(machine.cpu_state().pc > before);
        assert_eq!(machine.cpu_state().sp, 0xFF00, "nothing was pushed");
    }

    #[test]
    fn an_interrupt_is_offered_again_on_the_next_frame() {
        let mut machine = nop_machine();
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);
        machine.step();
        assert_eq!(machine.cpu_state().pc, 0x0038);

        // Acceptance cleared `iff1`; the handler's `EI` is what lets the next one in.
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode1);
        machine.step();
        assert_eq!(machine.cpu_state().pc, 0x0038);
        assert_eq!(machine.frames(), 2);
    }

    #[test]
    fn reset_returns_the_cpu_and_the_clock_but_keeps_ram() {
        let mut machine = nop_machine();
        machine.memory_mut().write(0x8000, 0xA5);
        machine.run_frames(2);
        machine.reset();
        assert_eq!((machine.frames(), machine.frame_t_state()), (0, 0));
        assert_eq!(machine.cpu_state().pc, 0);
        assert_eq!(machine.memory().read(0x8000), 0xA5);
    }

    #[test]
    fn a_spectrum_never_faults_because_its_bus_floats_to_rst_38h() {
        // Mode 0 is the only mode `Cpu::interrupt` can decline with a fault, and it does
        // so for any byte that is not an `RST`. A 48K's bus floats to 0xFF, which is
        // `RST 38h` — so the two modes land in the same place and the fault cannot happen.
        let mut machine = nop_machine();
        machine.run_frame();
        enable_interrupts(&mut machine, z80::InterruptMode::Mode0);
        machine.step();
        assert_eq!(machine.fault(), None);
        assert_eq!(machine.cpu_state().pc, 0x0038, "0xFF decodes as RST 38h");
    }

    #[test]
    fn rendering_takes_the_border_and_the_flash_phase_from_the_machine() {
        // The wiring, which nothing in `screen`'s own tests can see: the border comes from
        // the ULA's latch and the FLASH phase from this machine's frame count. The border
        // is set the only way anything ever sets it — an `OUT` the guest performs.
        use z80::Bus;

        let mut machine = nop_machine();
        machine.ula_mut().out_port(0x00FE, 2);
        for offset in 0..screen::ATTRIBUTE_FILE_LEN {
            // FLASH, INK 0, PAPER 7 — every cell swaps on the second half of the cycle.
            machine
                .memory_mut()
                .write(screen::ATTRIBUTE_FILE + offset as u16, 0x80 | 0x38);
        }

        let mut frame = Frame::new();
        machine.render(&mut frame);
        assert_eq!(
            frame.pixel(0, 0),
            Some(Colour::new(2)),
            "border came through"
        );
        assert_eq!(
            frame.pixel(screen::BORDER, screen::BORDER),
            Some(Colour::new(7)),
            "frame 0 is the first half of the FLASH cycle: paper is paper"
        );

        machine.run_frames(screen::FLASH_FRAMES);
        assert_eq!(machine.frames(), screen::FLASH_FRAMES);
        machine.render(&mut frame);
        assert_eq!(
            frame.pixel(screen::BORDER, screen::BORDER),
            Some(Colour::new(0)),
            "frame 16 is the second half: ink and paper have swapped"
        );
    }
}
