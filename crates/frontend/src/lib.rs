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
//!
//! ## Not gated, and it is observation
//!
//! | Property | Why nothing here can see it |
//! |---|---|
//! | **Whether it looks right** | Nothing in this crate opens a window under `cargo test`, and a pixel comparison against a reference image would grade this crate against itself. The colours are right if [`spectrum::Colour::rgb`] is right, and that is graded next door |
//! | **Whether it is pleasant to type on** | The mapping is a *design* claim. `tests/keymap_table.rs` proves the table is a bijection and that it has not silently changed; it cannot prove the choices are good ones. A person typing `PRINT "HELLO"` is the instrument, and the run is recorded in the report rather than asserted here |
//! | **Whether the motion is smooth** | [`pacing`] measures and reports; it does not judge. A run that reports `50.0 Hz, 0 dropped` and still stutters would be a vsync interaction this crate cannot see |
//! | **That the window opens at all** | Not reachable from `cargo test` — it needs a display server. Every gate here runs headless, which is what makes them runnable and also what bounds them |
//! | **`wasm32`** | Not built, not run. [`host`] names the calls that would have to change and what they would change to; `docs/M8.md` designs the rest. `cargo check --target wasm32-unknown-unknown -p frontend` is the one-line claim nobody in that design ran |
//! | **Which membrane key a non-US keyboard produces** | **Nothing, and it is known to differ between backends.** `miniquad` derives a `KeyCode` from the physical key in a browser, on Windows and on macOS, and from the *layout's* keysym under X11 and Wayland — so one AZERTY keypress is two different membrane keys. Read out of the pinned dependency's source, observed on no machine. [`keymap`] carries the table and `docs/STATUS.md` the open row |
//! | **Whether a browser lets the page have the `Ctrl` chords at all** | Nothing here, and **nothing automatable anywhere.** A key injected through the DevTools Protocol is delivered to the page without traversing the browser's shortcut layer, so the check returns green for a question it never asked. A person at a physical keyboard is the instrument. `docs/M8.md` Decision 2 |

pub mod host;
pub mod keymap;
pub mod media;
pub mod pacing;
pub mod palette;
pub mod ppm;
pub mod viewport;

pub use keymap::{Binding, Hotkey};
pub use media::Kind;
pub use pacing::{Pacer, RateMeter};
pub use viewport::Viewport;
