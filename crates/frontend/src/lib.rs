//! A window with a ZX Spectrum in it.
//!
//! Everything that decides *what* to draw, *which* key to press, and *how many* frames to
//! run lives in this library and is reachable from a test. [`main`](../zx/index.html) is the
//! macroquad shell around it: a window, an input poll, a texture upload, and a loop.
//!
//! ```no_run
//! use frontend::{keymap, palette, viewport};
//! use spectrum::{Frame, Spectrum};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut machine = Spectrum::new(&std::fs::read("testdata/roms/48.rom")?)?;
//! machine.run_frame();
//!
//! let mut frame = Frame::new();
//! machine.render(&mut frame);
//!
//! let mut rgba = Box::new([0_u8; palette::RGBA_BYTES]);
//! palette::write_rgba(&frame, &mut rgba);          // the only path from machine to pixels
//! # Ok(())
//! # }
//! ```
//!
//! # What grades a frontend, which is very little
//!
//! `docs/STATUS.md` records this project shipping gates that graded less than they appeared
//! to — repeatedly, and in several different shapes — and `docs/MACHINE.md` insists that what
//! is *not* covered be written down rather than inferred from the absence of a failing test. A
//! frontend is the worst case for that rule: almost everything a person means by *"is it
//! right"* — does it look right, does it feel right to type on, is the motion smooth — has no
//! oracle here and never will. So the two lists are kept apart, and the second one is not
//! softened.
//!
//! > **This paragraph said *"records five occasions"*, and there is no such figure anywhere.**
//! > Corrected loudly rather than quietly bumped, because how the number got here is worth more
//! > than the number.
//! >
//! > **What `docs/STATUS.md` actually records, established from that file and not from the
//! > brief that produced the error.** On 2026-09-01, **before that document was corrected**,
//! > `grep -n -iE 'occasion|graded less|worst form|frontend' docs/STATUS.md` matched **nothing**
//! > — it contained neither the phrase this sentence attributed to it, nor either integer, nor
//! > any mention of this crate. (It matches today, because the correction is written there too;
//! > re-checking means reading the hits rather than counting them, and every one should be a
//! > note *about* the defect.) The **one** counted enumeration of this family in it is the
//! > section
//! > *A gate that nothing runs, for the third time — and the form got worse*, which names its
//! > three instances outright: the M3 `zexdoc` job (*"The gate runs nowhere unless CI runs
//! > it"*), the CI workflow that could not be pushed (*"verified locally and enforced
//! > nowhere"*), and the M5 boot gate as an `examples/` binary. So **three** is derivable and
//! > **five** is not.
//! >
//! > **Two things make "three" the wrong repair on its own, which is why this sentence now
//! > carries no integer.** First, three counts a *narrower* family than the words around it —
//! > gates that **nothing ran**, not every gate that graded less than it appeared to. The
//! > broader family is recorded in that document over and over — a harness reporting green
//! > while verifying nothing, a codegen gate passing vacuously on an artefact that did not
//! > contain the subject, the keyboard matrix graded against itself, an interrupt window graded
//! > against its own constant — and is **counted nowhere**. Second, the phrase every consumer
//! > quotes as `STATUS.md`'s — *"the worst form so far"* — is **`docs/MACHINE.md:132`'s**.
//! >
//! > It is `docs/STATUS.md`'s own *"a derived figure repeated across documents acquires
//! > authority it never earned"*, with the sharpening `docs/M8.md` adds: **here nobody derived
//! > it even once.** The rule that document prescribes is *re-derive rather than cite*; a
//! > count that cannot be re-derived where it sits should not be written down, so this
//! > paragraph names the mechanism and points at the section that does the counting.
//!
//! ## Gated
//!
//! | Property | Evidence | Class |
//! |---|---|---|
//! | The texture's channel order — that blue stays blue | `tests/palette_texture.rs`, against **literal RGBA quadruples** written from the hardware's gun order (bit 0 blue, bit 1 red, bit 2 green), never from [`spectrum::Colour::rgb`]. Red and blue are the pair a `BGRA` mix-up swaps, so the discriminating case is a frame carrying both | **proven** |
//! | That a frame is drawn from a [`Frame`](spectrum::Frame) and nothing else | [`palette::write_rgba`]'s signature. It takes `&Frame` and a byte buffer; [`spectrum::Memory`] is not in scope and cannot be reached. This is structural, in the sense `docs/STATUS.md` means when it prefers *"allocation does not compile"* to a count of allocator call sites — there is no run in which it can be false | **proven** |
//! | The membrane map is a **bijection** — 40 host keys onto 40 membrane keys, none missed, none doubled | `tests/keymap_table.rs`. The 40 rows are literals; permuting the table under test turns it red, and that was measured rather than assumed — see [`keymap`] | **proven** |
//! | No host key is bound twice, and no hotkey shadows a membrane key | `tests/keymap_table.rs`, over the whole table | **proven** |
//! | The chords name the combinations the Spectrum's own legend names | `tests/keymap_table.rs`, against a literal table transcribed from the printed keyboard. This one has a referent outside this repository: `DELETE` really is printed above `0` | **derived** |
//! | Window geometry — integer scale, centring, and where the 256 × 192 display lands inside the border | `tests/viewport_geometry.rs`, literal window sizes to literal rectangles | **proven** |
//! | The pacing arithmetic — how many frames are owed, how many are run, how many are declared lost | `tests/pacing_accounting.rs`, literal `Duration` sequences to literal counts | **proven** |
//! | Which file a path names | `tests/media_dispatch.rs`, literal paths | **proven** |
//! | **Which machine a set of ROMs builds** — one is a 48K, two are a 128, anything else is refused with the count in the message | `tests/media_dispatch.rs`. The discriminating assertion is not that both calls succeed: the two images carry different first bytes and the 128 is asked what is at `0x0000`, so *"it built a 128"* and *"it used both ROMs, in the documented order"* are two claims. Measured rather than assumed — making the two-ROM arm build a 48K reddens two tests, exit 101, with the edit `diff`ed before the verdict was trusted | **proven** |
//! | **A URL and a command line name the same files** | `tests/argument_sources.rs`, over a table of twelve literal queries, each compared against the command line a person would have typed instead — through the **same** [`host::partition`], so the claim is *agreement* rather than *parsing*. Two rows must come out **different**, which is what stops a `partition` that ignored its argument from passing all twelve | **proven** |
//! | **Four byte sources reach the same machine** — a command line, a URL, an embedded payload, a dropped file | `tests/byte_sources.rs`, compared as `.z80` snapshots byte for byte, with a fifth machine that was never fed the payload as the assertion whose failure means *"I was not looking at the thing"*. The payload is **generated here** from the committed ROM, so a gate for a game-loading mechanism needs no game | **proven** |
//! | **This crate contains no `cfg(target`** | `tests/portability.rs`, which walks `src/` and asserts the absence — with a floor on the file count, because a walk that visited nothing reads the same as a crate that branches nowhere. It also asserts `crates/page` **does** contain it, since otherwise deleting that crate would make this one greener than ever | **proven** |
//! | **Two keys held together read low in the same scan**, a held key stays held, and a port polled a thousand times in one frame answers the same | `tests/keymap_under_a_game.rs`, against literal ports and bits taken from the hardware. This is what a *game* asks of the keymap and what forty single-key assertions cannot reach | **proven** |
//! | **Every message the status bar can draw is renderable** | `tests/on_screen_strings.rs`. It exists because three of them were not: an em dash drew as an empty box, and every test comparing those strings compared one unrenderable string to another. Carries a positive control, so the checker is known to be able to fail | **proven** |
//! | **A default build embeds nothing** | `tests/byte_sources.rs`. Not a formality: a default build that quietly carried a game would be the licensing failure the `bundled` feature is shaped to avoid | **proven** |
//! | **A written capture is the mixer's own samples** — every 16-bit word in a `.wav` body is the quantisation of the corresponding `f32` [`audio::Resampler::feed`] emitted, in order | `tests/wav_encoding.rs`. The quantisation rule is written out **again, by hand** in the test rather than imported from [`wav`], so the body is compared against a rule stated independently instead of against the function that produced it — the distinction `tests/ppm_encoding.rs` keeps `NORMAL` a literal for. Carries the same counter, because a body of zero length passes any loop by never entering it | **proven** |
//! | **A capture's header describes the body it actually contains** | `tests/wav_encoding.rs`, parsing all eleven header fields back out of the bytes and checking the two size fields against the body's real length. A player that reads the header and then runs off the end of the data is the failure this catches, and it is [`ppm`]'s `the_file_is_exactly_a_header_and_one_pixel_per_pixel` transposed | **proven** |
//! | **A loud transient clips rather than wraps** | `tests/wav_encoding.rs`, against levels past `±1.0` — which [`audio`]'s DC blocker really does produce on a step. A wrap turns the loudest instant of a tune into its opposite and is heard as a crack, not as clipping | **proven** |
//! | **Running faster than real time changes the rate and nothing else** — at the fastest multiplier and at 1×, the same forty frames leave the same [`spectrum::Spectrum::cpu_state`] and the same 327,680 bytes the window would have uploaded | `tests/speed_multiplier.rs`'s `the_fastest_multiplier_produces_the_same_machine_as_real_time`, which is what lets *nothing is bypassed* be a measurement rather than an argument — a tape still loads by its own signal because the machine cannot tell. Both runs are asserted to have **reached** frame 40 first, so a pacer that counted frames it never handed over cannot pass by leaving two identical machines at frame zero, and `the_comparison_can_fail` is the positive control: one extra frame must change both the CPU state and the picture, or every comparison above it is vacuous | **proven** |
//! | **The catch-up ceiling is a wall-clock bound and stays one at every multiplier** | `tests/speed_multiplier.rs`, literal [`Duration`](std::time::Duration) sequences in the shape `tests/pacing_accounting.rs` established. The discriminating case is `an_unscaled_bound_would_have_clipped_every_ordinary_tick`: at the top rung one ordinary 20 ms tick owes sixty-four frames and [`pacing::MAX_CATCH_UP`] is four, so a ceiling left unscaled would have run four and declared sixty lost **on every tick for ever** — and fast-forward and a machine failing to keep up are indistinguishable from outside except by that count. The bound stays 80 ms of wall clock at 1× and at 64× alike, which is why a dropped frame keeps meaning one physical thing and `LossMeter` needs no exception | **proven** |
//! | **A tape loaded fast is the same tape** — with a cassette **playing** and the guest reading the `EAR` bit every pass, the fastest multiplier and 1× leave the same [`spectrum::Spectrum::cpu_state`], the same 327,680 bytes, and the same [`spectrum::tape::Tape`]: same index, same remaining T-states, same level, same play state | `tests/speed_multiplier.rs`'s `a_tape_plays_the_same_signal_at_every_multiplier`. This is the row that makes fast **loading** safe rather than fast *running*, and it is a different claim from the one above it: that one compares two machines nothing is driving, and a pacer bug that stalled the tape would leave both screens identical and pass. Here the guest's own screen byte is an accumulation of the `EAR` bit, so a signal that drifted by one edge changes the picture. The train is sized to **run out** partway, so the end of the cassette — the transition the whole feature keys off — is inside the comparison rather than past it. `a_tape_that_never_moved_would_fail_this` is the positive control | **proven** |
//! | **The rung that decides for itself decides the same machine** — with a cassette playing, [`pacing::Rung::Automatic`] and 1× leave the same [`spectrum::Spectrum::cpu_state`], the same 327,680 bytes and the same [`spectrum::tape::Tape`], *including across the moment the drive stops itself* | `tests/speed_multiplier.rs`'s `a_tape_loaded_under_automatic_is_the_same_tape`. It is a **separate** claim from the row above and not a repetition of it: every multiplier reaches its frame count from `elapsed × factor`, and this rung reaches it by spending [`pacing::FLAT_OUT_BUDGET`] of wall clock, so nothing proven of the one carries to the other. The cassette is thirty frames of a forty-frame run, so the machine must key itself back to real time partway and finish paced — the transition `tests/speed_multiplier.rs` predicted a milestone earlier as *"the moment the emulator would key an automatic fast-load off"* | **proven** |
//! | **Automatic speeds up only while the drive is turning, and no other rung reads the drive at all** | `tests/speed_multiplier.rs`'s `automatic_runs_flat_out_only_while_the_drive_is_turning`, over the whole of [`pacing::RUNGS`] in both drive states. The second half is the one a person would feel: a machine parked at 1× to watch the loading stripes must not be overtaken when they press PLAY. The equivalence row above **cannot** catch a blinded trigger — two machines that both ran paced are still identical — so the discriminating assertion is that automatic reached the frame count in less than half the display ticks | **proven** |
//! | **A flat-out burst spends its budget and stops** | `tests/speed_multiplier.rs`'s `a_flat_out_tick_stops_when_its_budget_is_spent`, against a stepped clock so the count is a property of the budget rather than of how busy the machine is. Pinned **twice and independently**: a `const` assertion in [`pacing`] refuses at compile time any budget past the tenth of a second a person notices, and this asserts the same bound where it is spent. It is the one thing in this file whose failure mode is a **hang** rather than a red test — the burst owns its own loop — which is why the bound is asserted rather than argued | **proven** |
//! | **The tape keys report what the drive did, not what the key meant** — an empty drive, a tape wound to its end, and a tape that ran out on its own are each named, and none of the three says `tape playing` | `tests/tape_reports.rs`, driving a real [`spectrum::tape::Tape`] in a real machine to each of the three states rather than constructing the answer. The discriminating half is the **negative** controls, because the positive ones pass on a [`drive::Drive`] that simply always reported: `F4` must not produce [`drive::RAN_OUT`], a dropped cassette must not either, and the report must fire **once** rather than on every tick after. `the_reports_can_disagree` is the control proving the three cases are actually distinguishable | **proven** |
//!
//! ## Not gated, and it is observation
//!
//! | Property | Why nothing here can see it |
//! |---|---|
//! | **Whether it looks right** | Nothing in this crate opens a window under `cargo test`, and a pixel comparison against a reference image would grade this crate against itself. The colours are right if [`spectrum::Colour::rgb`] is right, and that is graded next door |
//! | **Whether it is pleasant to type on** | The mapping is a *design* claim. `tests/keymap_table.rs` proves the table is a bijection and that it has not silently changed; it cannot prove the choices are good ones. A person typing `PRINT "HELLO"` is the instrument, and the run is recorded in the report rather than asserted here |
//! | **Whether the motion is smooth** | [`pacing`] measures and reports; it does not judge. A run that reports `50.0 Hz, 0 dropped` and still stutters would be a vsync interaction this crate cannot see |
//! | **That the window opens at all** | Not reachable from `cargo test` — it needs a display server. Every gate here runs headless, which is what makes them runnable and also what bounds them |
//! | ~~**`wasm32`**~~ | ~~Not built, not run.~~ **Built, linked and run**, 2026-09-01: a served page boots the 48K, builds a 128 from the query string, saves a `.z80` through a `Blob` and restores one dropped back onto it. `web/README.md` records each observation with its provenance. What replaces this row is narrower and is below |
//! | **That the page renders, on any browser but the one that was tried** | Nothing. One browser, one operating system, one machine, one afternoon. `web/gate.sh` grades a compiler, a linker and a module's import table, and **not one of its assertions observes a pixel** |
//! | **That a `Blob` becomes a file on somebody's disk** | Nothing. The anchor's `click` was intercepted to observe it, so what was seen is the `Blob` and the `<a download>`, not a download directory |
//! | ~~**Audio**~~ | ~~Nothing, and nothing is wired.~~ **Wired, mixed, resampled and now written to a file**: [`audio`] carries the mix and the [`Resampler`](audio::Resampler), `tests/audio_from_the_machine.rs` measures `BEEP 1,0` end to end against the Sinclair manual, and `zx-shot --wav` writes what a device would have been handed. `docs/M8.md` Decision 12 still settles where resampling lives and why `macroquad`'s own audio cannot carry generated samples. What survives of this row is narrower, is the part that was always the real gap, and is below |
//! | **That anyone has heard any of it** | **Nothing, and no green anywhere in this crate bears on it.** The audio gates measure numbers in a buffer; `zx-shot --wav` writes those numbers to a file. Both stop at the same place: **this environment has no audio device and no way to capture one**, so *the tune is right* is observation by a person with speakers and is recorded as such. A capture that is well-formed, whose header describes its body, and whose body is provably the mixer's output is the strongest evidence reachable from here — and it is evidence about a **signal**, not about a sound |
//! | **Whether the pitch is right, as opposed to the mechanism** | `tests/audio_from_the_machine.rs` has the only audio oracle outside this repository — `BEEP 1,0` is middle C, 261.63 Hz, from the Sinclair BASIC manual — and the full path measures **261.71 Hz**, 0.03 % out, on a 0.2 % tolerance. **This row used to say 246.65 Hz and blame a semitone on `crates/spectrum`. There was no semitone.** The probe took its crossing *count* from the crossings and its *duration* from the amplitude envelope, and the resampler's DC blocker rings down for 34 ms after the note stops — so 1653 samples carrying no crossings were divided in as though they were period. The machine had been right all along, by its own arithmetic (`HL = 1642` ⇒ 261.69 Hz) and by three independent gates. What is genuinely uncovered is narrower: the pitch is graded, the **timbre** is not, and no assertion here has ever involved a speaker |
//! | **Whether fast-forward should be silent, and whether sound comes back when it stops** | **Nothing, and the two gates that look closest are the ones that prove it.** `tests/speed_multiplier.rs` compares two machines on [`spectrum::Spectrum::cpu_state`] and on all 327,680 bytes of the frame — and on nothing audible; the machine's own [`spectrum::Spectrum::take_samples`] is drained at every speed, so *"the same machine"* is proven of the CPU and the picture and was never asked about the device. Above real time `main.rs` feeds the resampler nothing and the readout reads `snd mute`. That is **argued** at the push site rather than gated — a card consumes one second per second however fast the pacer runs, so the alternatives are a queue clamped down to one frame in sixty-four, or a tune shifted six octaves at 64× with the resampler rebuilt on every speed change — and *silence is the right answer* is a judgement no assertion here reaches. **The tape's own screech is the case a person will ask about** and it is the same judgement: one frame in sixty-four of a loading tone is a click every twenty milliseconds rather than a fast screech, so the answer is `F8` back to `1×` — one keypress from the top rung — and not a special case. Neither *that it goes quiet* nor *that it comes back* is gated, for the same reason as the rows above: this environment has no audio device |
//! | **How fast `auto` actually goes in a window** | **Nothing here, and the number that exists is a headless one.** `tests/speed_multiplier.rs`'s `a_real_cassette_end_to_end_under_automatic` loads a real *Manic Miner* cassette through the real [`pacing::Rung::Automatic`] decision in **2.07–2.31 s, 83–93× real time** over four runs — with no window, so nothing is drawn, no texture is uploaded and no tick waits for vsync. A window pays all three out of the same tick, and [`pacing::FLAT_OUT_BUDGET`] can only measure one of them: this crate's half of a picture is 0.016 ms and the GPU's half is not reachable from a headless test at all. What a person actually gets is therefore **less than 93×, by an amount nothing in this repository can predict** — which is why the rung prints no multiplier and the status bar's `Hz` is the answer. That measurement is also corpus-backed and `#[ignore]`d, so a clean checkout never runs it. **The rung has since been run in a window and reported working** — the owner, at his own keyboard, 2026-09-02, recorded with its date and its machine in `docs/images/README.md`. That changes what has been *seen* and not one figure in this row: it reaches the half of a tick [`pacing::FLAT_OUT_BUDGET`]'s derivation marks *"not measurable from a headless test"*, which a person at the window is the only instrument for, and it says nothing about the rate, the load time, or how much of 93× survives vsync, because none of those was measured. One person, once, on one machine, grading nothing — which is why it is written into this table rather than moving the row out of it |
//! | **Whether a game runs, or responds to the keyboard** | ~~Nothing. No commercial game has ever run on this emulator.~~ **Four have**, 2026-09-01: *Cybernoid* on a 128 restored from a `.z80`, and *Manic Miner*, *Cybernoid II* and *Exolon* on a 48K **loaded from tape by the ROM itself**. `docs/images/README.md` carries nine committed photographs and the exact `zx-shot` command that re-takes each, so the claim is re-runnable rather than remembered. **Two of the four also took a keypress *after* their tape had finished** — `zx-shot --keys-after` presses once `Tape::pulses` says the cassette has run out, and *Exolon* and *Cybernoid II* both left their menus and started, which is the *responds to the keyboard* half of this row's own title reaching a tape-loaded game for the first time. Manic Miner's key reads were **measured** and not merely watched: [`keymap::ArrowTarget::Both`] records that the bare digits and the Kempston port leave byte-identical machine state after an identical hold. What is genuinely uncovered is narrower than the row it replaces and is still real — **no gate in this repository runs a game and none can**, because `testdata/games/` is gitignored and a clean checkout has no corpus, so every one of those runs is a person with their own files and `cargo test` is none the wiser. Whether a game is *playable* remains a person at a keyboard. **Ghosting and rollover are not modelled** either — `crates/spectrum/tests/keyboard_matrix.rs` says so in its own words — so a game needing a pair that shares a row and a column meets a recorded gap rather than a regression |
//! | **That the only way to load a file at runtime is a mouse gesture** | Not a coverage gap but a *design* one, recorded here because it has the same effect on a person: drag-and-drop is not keyboard-reachable and not reachable on a phone. `docs/M8.md` Decision 11 refuses a file picker and says what that costs |
//! | **Which membrane key a non-US keyboard produces** | **Nothing, and it is known to differ between backends.** `miniquad` derives a `KeyCode` from the physical key in a browser, on Windows and on macOS, and from the *layout's* keysym under X11 and Wayland — so one AZERTY keypress is two different membrane keys. Read out of the pinned dependency's source, observed on no machine. [`keymap`] carries the table and `docs/STATUS.md` the open row |
//! | **Whether a browser lets the page have the `Ctrl` chords at all** | Nothing here, and **nothing automatable anywhere.** A key injected through the DevTools Protocol is delivered to the page without traversing the browser's shortcut layer, so the check returns green for a question it never asked. A person at a physical keyboard is the instrument. `docs/M8.md` Decision 2 |

pub mod audio;
pub mod bundle;
pub mod drive;
pub mod host;
pub mod keymap;
pub mod media;
pub mod pacing;
pub mod palette;
pub mod ppm;
pub mod viewport;
pub mod wav;

pub use keymap::{Binding, Hotkey};
pub use media::Kind;
pub use pacing::{Pacer, RateMeter};
pub use viewport::Viewport;
