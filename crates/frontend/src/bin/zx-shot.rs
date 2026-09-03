//! Boot a machine, optionally type at it, and write what the screen looks like to a file.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
//!     --frames 120 --keys 'P;LeftControl+P;H;E;L;L;O;LeftControl+P;Enter' \
//!     --out .agent-workspace/shots/typed.ppm
//! ```
//!
//! Then `sips -s format png typed.ppm --out typed.png` on macOS, or any image viewer that
//! reads Netpbm. See [`frontend::ppm`] for why the format is P6 and not PNG.
//!
//! # Photographing a 128
//!
//! `--rom` repeats, and the **count** is what names the machine — one is a 48K, two are a 128
//! with the first paging in at reset. There is no `--model` flag and no second machine-building
//! path; [`frontend::media::start`] carries the whole decision and `tests/media_dispatch.rs`
//! grades it.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
//!     --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom \
//!     --frames 120 --out .agent-workspace/shots/128-menu.ppm
//! ```
//!
//! **`screen::read_text` is a 48K instrument and nothing here calls it, which is why this
//! binary can photograph a 128 at all.** That function reads the character set at `0x3D00`
//! *through the machine's slot map*; the 128 editor ROM has no font there, and the menu loop
//! pages ROM 1 in and out every frame, so the same call answers correctly or answers `?`
//! depending on which frame it is asked. This binary renders pixels — `Spectrum::render` into a
//! [`spectrum::Frame`], then [`frontend::palette::write_rgba`] — and never asks the machine what
//! its screen *says*. The hazard is real and it is somebody else's; it is named here so that a
//! future `--assert-text` flag is not added without meeting it.
//!
//! # Why this is a second binary and not a `--screenshot` flag on `zx`
//!
//! `#[macroquad::main]` **opens the window before the function body runs** — it expands to a
//! `miniquad::start` that creates the context and then drives the future. A screenshot mode
//! reached from inside that body would therefore not be headless: it would open a window,
//! take a picture, and close it, which is unusable over SSH, in CI, or on any machine without
//! a display server. This binary never calls `miniquad::start`, so linking macroquad costs a
//! symbol table and nothing else.
//!
//! # It is the same pipeline the window runs, and that is the point
//!
//! `keymap::apply` -> `Spectrum::run_frame` -> `Spectrum::render` -> `palette::write_rgba`,
//! which is exactly what `zx`'s frame loop does. Only the final step differs — this writes
//! [`frontend::ppm`] bytes where the window uploads a texture — and `tests/ppm_encoding.rs`
//! asserts those bytes are the same buffer. A screenshot produced by code the window does not
//! run would prove nothing about the window, and this repository keeps catching gates that
//! graded less than they appeared to.
//!
//! > **This said *"caught five gates"* and the figure had no source.** `docs/STATUS.md` — the
//! > document that number was resting on — recorded **no such count**: verified on 2026-09-01,
//! > before that document was corrected, by
//! > `grep -n -iE 'occasion|graded less|worst form' docs/STATUS.md`, which matched nothing at
//! > all. It matches now because the correction lives there too. The one place that document
//! > does count this family is
//! > *A gate that nothing runs, for the third time — and the form got worse*, which names
//! > **three** instances and counts something narrower than this sentence claimed — see
//! > [`frontend`](crate)'s own header, where the whole correction is set out. The mechanism is
//! > what this paragraph needed and the integer was decoration, so the integer is gone.
//!
//! # `--wav`, and why it is the same argument again
//!
//! Audio was wired on both targets and there was **no way to get any of it out to a file**.
//! `tests/audio_from_the_machine.rs` measures a frequency and leaves nothing anyone can listen
//! to, which makes every claim about sound a claim about a number in a buffer.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
//!     --media testdata/games/ManicMiner.tap --play-tape \
//!     --keys 'J;LeftControl+P;LeftControl+P;Enter' --settle 9800 \
//!     --wav .agent-workspace/tune.wav --wav-from 9300 \
//!     --out .agent-workspace/tune.ppm
//! ```
//!
//! **It is the same path the device gets, and that is the whole of the design.**
//! [`Spectrum::take_samples`] -> [`frontend::audio::Resampler::feed`], which mixes and resamples
//! in one call, at the rate a device runs — the identical two lines `src/main.rs` runs every
//! frame. Only the final step differs: this writes [`frontend::wav`] bytes where the window
//! hands the buffer to a sound card. `tests/wav_encoding.rs` asserts that final step changes
//! nothing, sample for sample. A capture produced by code the speaker does not run would prove
//! nothing about the speaker — the argument this file already makes about pixels, which is why
//! it needed no new one.
//!
//! [`Spectrum::take_samples`] is called **every frame whether or not `--wav` was asked for**, so
//! there is exactly one frame loop and adding the flag cannot change the picture. The machine
//! buffers two frames and then counts what it lost; a tool that drained only when recording
//! would leave [`Spectrum::dropped_samples`] describing the flag rather than the machine.
//!
//! # Why `--wav-from` exists, and why it is a frame index
//!
//! A tape load is about **9,244 frames** — three minutes of emulated time — and it is the
//! loading tone for nearly all of it. A capture of the whole run is therefore some 36 MB of
//! screech with the tune at the end, which is not something to hand to anybody. `--wav-from`
//! opens the recording window at a frame index counted over the **whole** run, every phase
//! included: the `--frames` warm-up, both halves of every `--keys` tap, and the `--settle` tail.
//! The total is printed on success so the next index can be read off rather than guessed.
//!
//! The resampler is built when recording starts rather than at boot, so its phase and its DC
//! filter begin at the window's edge. That is not a shortcut — it is what the window itself does,
//! where the device appears after the first click and [`Resampler`] is constructed then.
//!
//! # `--keys-after`, and how it knows the game has arrived
//!
//! Every `--keys` tap happens **before** `--play-tape` presses PLAY — it has to, because the keys
//! are what type `LOAD ""` — so until now the last key of a run was released some three minutes of
//! emulated time before the game it was loading existed. `docs/images/README.md` records what that
//! cost: *Exolon* and *Cybernoid II* both load correctly, both then sit on their own menus, and
//! neither could be photographed doing anything else. It also records the sharper cost — with no
//! key to send, *"it does not advance"* and *"it is waiting for input"* produce identical evidence,
//! which is a diagnosis nobody can make rather than a gallery nobody admires.
//!
//! `--keys-after` is the second script, and the interesting part is the wait in front of it.
//!
//! **The tape says when it is finished, and it is asked rather than guessed.** `docs/M6.md`
//! Decision 5 makes the pulse train *the* representation of a tape rather than an implementation
//! detail of one, and [`spectrum::tape::Tape::pulses`] is public for exactly that reason. Its
//! half-periods are T-states; their sum is the whole cassette, end to end; dividing by the frame
//! the machine actually runs turns that into the unit this binary counts in. So the wait is
//! [`tape_frames`], a number read off the file in the drive, and it is right for a tape of any
//! length on either model without anybody sweeping for it.
//!
//! **What that does *not* settle is how long the loader then takes to jump into the game**, and
//! nothing in the machine can be asked: the last block has landed, and whether the next instant
//! is a title screen or a menu or a black frame mid-clear is a property of the game. That gap is
//! `--settle` — the flag that already means *frames run so the consequence of the last event
//! reaches the screen* — and it is applied on both sides of the after-keys, before them so the
//! game is up to receive them and after them so what they did is on the picture. It is an
//! explicit number because it is genuinely a guess, and the frame the tape ran out on is printed
//! on the way past so the guess starts from a reading.

use std::process::ExitCode;

use macroquad::input::KeyCode;

use frontend::audio::{self, Resampler};
use frontend::{bundle, host, keymap, media, palette, ppm, wav};
use spectrum::{Frame, Model, Spectrum};

/// What `--media` accepts, as this binary says it out loud.
///
/// # One constant because there were two copies, and both were wrong
///
/// This sentence is printed twice — in the usage block and in the refusal a bad extension earns —
/// and both said `.tap, .z80 or .sna` for a fortnight after `.tzx` became loadable. Two copies of
/// a sentence is two things to forget; `media::EXTENSIONS` had grown and neither noticed, because
/// nothing links a literal to the table it describes.
///
/// So there is one copy now and `mod tests` grades it against that table. `.rom` is not in it:
/// `--rom` builds the machine and `--media` is handed to [`media::insert`], which refuses one.
const MEDIA_FORMATS: &str = ".tap, .tzx, .z80 or .sna";

/// Frames given to the ROM before anything is typed — four seconds of emulated time, several
/// times what its start-up needs. The same figure `crates/spectrum/examples/boot.rs` uses.
const DEFAULT_FRAMES: u64 = 120;

/// Frames a key is held, and then released, unless `--hold` says otherwise.
///
/// # This was six, and six was one frame above a measured cliff
///
/// **A key held four frames or fewer is missed entirely; five registers.** Measured against the
/// 48K ROM on 2026-09-01 and confirmed independently in the window, where an 80 ms `ENTER` tap
/// did nothing and a 300 ms one started a game. The old default of six was *"well clear"* of
/// the ROM's debounce by assertion and was in fact **one frame** above the floor — so any
/// script that hit a slightly slower path, or any consumer that scans less often than the ROM,
/// would have dropped keys with nothing to explain it.
///
/// Ten is the default now: comfortably clear of the measured floor and still a fifth of a
/// second per key. The number that matters is the **floor**, and the floor is written down here
/// so the next person raising or lowering this has something to reason from.
///
/// > **It is clear of the *ROM's* scan and nothing here knows whether it is clear of a
/// > *game's*.** A game polls its own keys in a tight loop, several half-rows per frame, and
/// > decides what a hold means on its own terms. `--hold` exists because otherwise every
/// > *"the game does not respond"* is ambiguous between a defect in the emulator and a tap this
/// > tool made too short, and an ambiguous report is the expensive kind.
///
/// # The floor was one edge of two, and this comment used to name only that one
///
/// A key held past the 48K editor's repeat delay is no longer a key *held*. It is a key **typed
/// again**, and again, on a fixed period — so `--keys` stops being a script and becomes that
/// script repeated, and the line it was spelling never parses. The upper edge is
/// [`MEASURED_KEY_REPEAT_DELAY_FRAMES`] and it is as measured as the lower one:
/// `docs/images/README.md`, under *"The other end of `--hold` is a threshold, not a number"*,
/// carries the sweep and what the editor was left holding at each setting.
///
/// **It is not only the keyword that repeats**, which is the part worth reading before raising
/// this: *"Every tap in the script repeats, because every tap is held for the same `--hold`"* —
/// so the four taps of `LOAD ""` arrive at `--hold 60` as six keywords and twelve quotes.
///
/// Ten is unchanged and clears both edges by a wide margin. What changed is that the range is
/// written at both ends and gated at both ends — the `const` assertions below for this default,
/// and this binary's own `mod tests` for the threshold itself — `#[cfg(test)]`, so rustdoc cannot
/// link it from a non-test build — which is a property of the ROM rather than
/// of any one setting and so is asserted where the behaviour changes rather than at a sample.
const HOLD_FRAMES: u64 = 10;

/// The fewest frames a key must be held for the 48K ROM to see it at all.
///
/// Measured, not assumed. Kept beside [`HOLD_FRAMES`] so that a future edit lowering the
/// default has to walk past the reason it cannot go below five.
const MEASURED_KEY_FLOOR_FRAMES: u64 = 5;

/// The most frames a key can be held before the 48K editor types it a **second** time.
///
/// Measured, and the number is the ROM's own rather than one found by sweeping and left
/// unexplained: the 48K keyboard routine holds its repeat delay in `REPDEL`, which is 35 frames
/// at boot, and the first acceptance is the first scan — so the thirty-sixth frame of a hold is
/// the second acceptance. `mod tests` reads that system variable back out of a booted machine
/// and grades the two edges either side of it, so a red there says which of the two facts moved:
/// how this tool steps a key, or which ROM is in `testdata/`.
///
/// It binds the `--keys` half of a run and not the `--keys-after` half, because a game polls on
/// its own terms and has never heard of `REPDEL`. `docs/images/README.md` measured `30` as clear
/// of both edges *and* long enough for *Exolon*'s menu, which is why the two gallery commands
/// that type at a game pass it — one number serving both halves, rather than a second flag for
/// somebody to reason about.
const MEASURED_KEY_REPEAT_DELAY_FRAMES: u64 = 35;

const _: () = assert!(
    HOLD_FRAMES > MEASURED_KEY_FLOOR_FRAMES,
    "a default at or below the measured floor drops keys with nothing to explain it"
);

const _: () = assert!(
    HOLD_FRAMES <= MEASURED_KEY_REPEAT_DELAY_FRAMES,
    "a default above the measured repeat delay types every tap in the script at least twice"
);

/// Frames run after the last key, so that an `ENTER` has time to execute what it submitted.
///
/// Enough for a line of BASIC. **Not** enough for everything an `ENTER` can start: selecting
/// *48 BASIC* from the 128's menu re-pages the ROM, clears the screen and reinitialises the
/// system variables, and photographed at 25 frames it is a black rectangle — the picture is of
/// a machine that is genuinely mid-clear, not of a failure. That is what `--settle` is for, and
/// it is a flag rather than a bigger constant because the right number is a property of what was
/// typed and nothing here can know it.
const SETTLE_FRAMES: u64 = 25;

/// The device rate a capture is written at.
///
/// Fixed rather than a flag. A real device names its own rate — the window asks
/// [`page::audio_rate`] and builds the resampler for whatever comes back — but there is no
/// device here to ask, so this stands in for one, and 48,000 is what this machine's own device
/// reported and what `tests/audio_from_the_machine.rs` measures at. Every player resamples, so
/// the number a capture is written at changes the file and not the sound.
const DEVICE_HZ: u32 = 48_000;

fn main() -> ExitCode {
    match run(&host::arguments()) {
        Ok(path) => {
            println!("wrote {path}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("zx-shot: {message}");
            eprintln!(
                "usage: zx-shot --out PATH [--rom PATH]... [--media PATH]... [--frames N]\n\
                 \x20                     [--keys SCRIPT] [--keys-after SCRIPT] [--hold N]\n\
                 \x20                     [--settle N] [--wav PATH] [--wav-from FRAME]\n\
                 one --rom is a 48K; two are a 128, editor ROM first\n\
                 --media takes a {MEDIA_FORMATS}\n\
                 --hold is frames per key, and has an edge at each end: under \
                 {MEASURED_KEY_FLOOR_FRAMES} the ROM misses\n\
                 \x20  the tap, over {MEASURED_KEY_REPEAT_DELAY_FRAMES} the 48K editor repeats \
                 every one of them. A game's menu may\n\
                 \x20  want the top of that range; the gallery's game shots use 30\n\
                 --play-tape presses PLAY after the keys: a .tap loads nothing without it\n\
                 --keys-after types once the tape has played out, which is how a loaded game is\n\
                 \x20  started; --settle is then the gap either side of it\n\
                 --wav writes what a device would have been handed, mono 16-bit at 48 kHz\n\
                 --wav-from skips to a frame index over the whole run: a tape load is ~9244"
            );
            ExitCode::FAILURE
        }
    }
}

/// Everything the command line can say.
struct Options {
    /// In the order named. One is a 48K, two are a 128 — see [`media::start`].
    roms: Vec<String>,
    /// Tapes and snapshots, in the order named, loaded after the machine is built.
    media: Vec<String>,
    out: String,
    frames: u64,
    /// Frames run after the last key. Defaults to [`SETTLE_FRAMES`].
    settle: u64,
    /// Frames each key is held, and then released. Defaults to [`HOLD_FRAMES`].
    hold: u64,
    /// Press PLAY on the tape after the keys have been typed.
    ///
    /// # Without this, `--media` on a `.tap` reads as though it works and does not
    ///
    /// [`media::insert`] puts a tape in **stopped**, deliberately — the loader would otherwise
    /// meet the middle of a block — and the window's answer is `F3`. But `F3` is a
    /// [`keymap::HOTKEYS`] entry, and `--keys` resolves names through
    /// [`keymap::code_named`], which searches `BINDINGS` **only** and is documented as a
    /// deliberate restriction: *"a caller driving the machine by name cannot press a key the
    /// window could not press, and cannot reach a hotkey, which is an emulator control and not
    /// a key on the machine at all."*
    ///
    /// That restriction is right and stays. The consequence was not: it left `--media` with a
    /// `.tap` accepting the file, loading nothing, and reporting success — **the same class as
    /// a drop that does nothing**, which this crate spent Decision 11 fixing in the window and
    /// then shipped in the headless tool. A flag is the fix, because pressing PLAY is an
    /// emulator control and belongs beside `--frames` rather than inside a key script.
    play_tape: bool,
    /// Where to write what a device would have been handed, if anywhere.
    wav: Option<String>,
    /// The frame index, over the whole run, at which recording starts.
    wav_from: u64,
    /// Each entry is one tap: the host keys held together for [`Options::hold`] frames.
    taps: Vec<Vec<KeyCode>>,
    /// The same again, typed once the tape has played out. See this file's header.
    ///
    /// A second list rather than a marker inside [`Options::taps`], because the two are separated
    /// by an event and not by an index: everything before PLAY is typed at the ROM, everything
    /// after it is typed at whatever the tape delivered. A script that had to encode *"and now
    /// wait for the tape"* inside its own `;`-separated syntax would be spelling an emulator
    /// control as a keystroke, which is the mistake `--play-tape` exists to avoid.
    after: Vec<Vec<KeyCode>>,
}

/// Runs frames, and keeps the ones a device would have heard.
///
/// # Why this owns the frame loop rather than sitting beside it
///
/// [`Spectrum::run_frames`] cannot be used once anything is listening, because the machine
/// buffers two frames of samples and then starts counting what it lost — so a run of hundreds of
/// frames without a drain discards nearly all of its own audio and reports the loss as
/// [`Spectrum::dropped_samples`]. Every frame this binary runs therefore goes through
/// [`Recorder::advance`], including the ones nobody is recording, which is what keeps a `--wav`
/// run and a plain one the same run.
struct Recorder {
    /// Built at the recording window's edge, not at boot. See this file's header.
    resampler: Option<Resampler>,
    /// The machine's clock, kept because the resampler is built late.
    cpu_hz: u32,
    /// What a device would have been handed, at [`DEVICE_HZ`].
    samples: Vec<f32>,
    /// Frames run so far, across every phase.
    elapsed: u64,
    /// The index at which to start keeping samples, or `None` to keep none.
    from: Option<u64>,
}

impl Recorder {
    /// A recorder for `machine`, keeping samples from frame `from` if `wanted`.
    fn new(machine: &Spectrum, wanted: bool, from: u64) -> Self {
        Self {
            resampler: None,
            // A 48K runs at 3,500,000 T-states a second and a 128 at 3,546,900, so the two
            // resample differently.
            //
            // **This asked `ay().is_some()`, and cited `src/main.rs` as asking the same question
            // for the same reason.** `main.rs` documents *retiring* exactly that: what sets the
            // clock rate is the **model**, and a machine is not fast because it has a sound chip
            // — the two agree today and agree by coincidence. So the comment made a checkable
            // claim about a sibling file and the sibling file disproved it, while the defect it
            // described stayed live here: a +2A is a 128-clocked machine, and `--wav` from one
            // with no AY wired resampled at 3,500,000 instead of 3,546,900 — **1.34% out, a
            // quarter-tone of pitch error**, in the crate's only audio evidence artefact. The
            // fix was half-applied because the derivation exists twice; that is the duplication,
            // and this is what it cost.
            cpu_hz: audio::cpu_hz(machine.model() == Model::Spectrum128),
            samples: Vec::new(),
            elapsed: 0,
            from: wanted.then_some(from),
        }
    }

    /// Run one frame, draining the machine's audio and keeping it if the window is open.
    ///
    /// The unit is a single frame rather than a count because [`press`] re-applies the keyboard
    /// **between** frames, exactly as the window re-polls the host between them. Collapsing that
    /// into one `apply` and a run of `hold` frames would be a change to how a key is held, made
    /// silently, in a tool whose whole job is to reproduce what the window does.
    fn frame(&mut self, machine: &mut Spectrum) {
        machine.run_frame();
        // Taken every frame whether or not anything is listening, exactly as the window does
        // it: a consumer that stops draining makes `dropped_samples` climb for a reason that
        // has nothing to do with audio.
        let produced = machine.take_samples();
        if self.from.is_some_and(|from| self.elapsed >= from) {
            self.resampler
                .get_or_insert_with(|| Resampler::new(self.cpu_hz, DEVICE_HZ))
                .feed(produced, &mut self.samples);
        }
        self.elapsed += 1;
    }

    /// Run `frames` with the keyboard left as it is.
    fn advance(&mut self, machine: &mut Spectrum, frames: u64) {
        for _ in 0..frames {
            self.frame(machine);
        }
    }
}

/// Read a file, consulting the embedded payload first.
///
/// The same two-step the window's own loader performs, and it has to be: a bundled build's
/// whole claim is that it needs no files, and a headless binary that ignored the bundle would
/// be photographing a different machine from the one the window runs. `crates/frontend`'s
/// [`bundle`] is the single lookup both share; only the fallback differs, because this binary
/// reads a filesystem and the window may be reading a network.
fn read(path: &str) -> Result<Vec<u8>, String> {
    if let Some(embedded) = bundle::bytes(path) {
        return Ok(embedded.to_vec());
    }
    std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))
}

fn run(arguments: &[String]) -> Result<String, String> {
    let options = parse(arguments)?;

    // Printed before anything else and to stdout, not stderr, so it survives a redirect of the
    // interesting output. A build that embeds a Sinclair ROM redistributes it to whoever runs
    // the binary, and Amstrad's permission asks the program to carry the note. The window
    // draws it under the picture; a headless tool has no picture, so it says it.
    if let Some(notice) = bundle::acknowledgement() {
        println!("{notice}");
    }

    let mut roms = Vec::with_capacity(options.roms.len());
    for path in &options.roms {
        roms.push(read(path)?);
    }
    let borrowed: Vec<&[u8]> = roms.iter().map(Vec::as_slice).collect();
    let mut machine =
        media::start(&borrowed).map_err(|error| format!("{}: {error}", options.roms.join(", ")))?;

    for path in &options.media {
        let bytes = read(path)?;
        // **One decision, and this used to be a second copy of it.** These three steps —
        // `unsupported`, `kind_of`, `insert`, each with its own message — were written out here
        // and again in `media::accept`, and `media.rs`'s own header predicts what happens next:
        // *"a second way to decide 'what is this file and what do we do with it' is a second
        // thing that can disagree."* They had already disagreed. The copy here had no arm for
        // `Error::Model`, so a 128 snapshot on a 48K stopped this binary with *"needs a 128"*
        // while the window said the same thing **and** named the ROMs that would fix it.
        media::load_named(&mut machine, path, &bytes)?;
    }

    let mut recorder = Recorder::new(&machine, options.wav.is_some(), options.wav_from);
    recorder.advance(&mut machine, options.frames);
    for tap in &options.taps {
        press(&mut machine, &mut recorder, tap, options.hold);
    }
    // After the keys, not before: the real sequence is `LOAD ""`, ENTER, then PLAY, and a tape
    // running while the ROM is still booting would have the loader meet the middle of a block —
    // which is the reason `media::insert` inserts it stopped in the first place.
    //
    // The frame the pulse train runs out on is fixed *here*, because here is where it starts and
    // `media::insert` leaves a tape wound to its beginning — so from this instant the whole
    // cassette is still to come and its length is the wait. `None` when nothing was started, and
    // a run with no tape then falls straight through to the keys, which is right for a `.z80`.
    let tape_ends_at = options
        .play_tape
        .then(|| recorder.elapsed + tape_frames(&machine));
    if options.play_tape {
        machine.tape_mut().play();
    }
    if !options.taps.is_empty() {
        recorder.advance(&mut machine, options.settle);
    }

    // The second half of the run, and the only one a tape-loaded game can be started from. The
    // shape deliberately mirrors the block above — wait for the thing, type at it, let the screen
    // catch up — because it is the same three steps against a different event.
    if !options.after.is_empty() {
        wait_for_the_tape(&mut machine, &mut recorder, tape_ends_at);
        recorder.advance(&mut machine, options.settle);
        for tap in &options.after {
            press(&mut machine, &mut recorder, tap, options.hold);
        }
        recorder.advance(&mut machine, options.settle);
    }

    // The pipeline, identical to the window's up to the last line.
    let mut frame = Frame::new();
    machine.render(&mut frame);
    let mut rgba = palette::buffer();
    palette::write_rgba(&frame, &mut rgba);

    host::save(&options.out, &ppm::encode(&rgba)).map_err(|error| error.to_string())?;

    let mut wrote = options.out;
    if let Some(path) = options.wav {
        let bytes = wav::encode(&recorder.samples, DEVICE_HZ).map_err(|error| error.to_string())?;
        host::save(&path, &bytes).map_err(|error| error.to_string())?;
        // The seconds and the frame total are what make the next `--wav-from` a reading rather
        // than a guess, and a capture of zero samples says so here instead of arriving as a
        // silent file somebody plays twice before checking.
        wrote = format!(
            "{wrote} and {path}: {} samples, {:.2}s at {DEVICE_HZ} Hz, from frame {} of {}",
            recorder.samples.len(),
            recorder.samples.len() as f64 / f64::from(DEVICE_HZ),
            options.wav_from,
            recorder.elapsed,
        );
    }
    Ok(wrote)
}

/// Frames the tape in the drive needs to play from end to end.
///
/// # The tape is asked, not guessed at
///
/// `docs/M6.md` Decision 5 makes the pulse train the tape's representation rather than a detail
/// of one, and [`spectrum::tape::Tape::pulses`] is public because of it — its own documentation
/// says the train *"**is** the tape in this design"*. Each entry is one half-period in T-states,
/// so their sum is the cassette end to end, and the frame length comes from the machine's own
/// [`spectrum::timing::Timing`] rather than a literal: a 128 runs 70,908 T-states to a 48K's
/// 69,888, and a tape wait that assumed one of them would be a minute out on an hour's tape.
///
/// Rounded **up**, so the answer is never short by the remainder. `Spectrum::run_frame` hands the
/// clock at least a frame's worth of T-states — it runs until the frame counter moves and carries
/// any overshoot into the next — so this many frames is enough for the drive to have reached the
/// end of the train and stopped itself.
///
/// With nothing in the drive this is zero, and zero is the right answer rather than a degenerate
/// one: `Tape`'s `Default` is *"a tape drive with nothing in it"*, a `.z80` is already running,
/// and there is nothing to wait for.
/// # It used to take `&mut Spectrum`, and it never wrote anything
///
/// [`spectrum::Spectrum::tape_mut`] was the only way to reach the drive, so a function whose
/// whole job is to **read** a pulse train had to announce that it intended to change the machine
/// — and then arrange its two lines around the borrow that announcement cost, with a comment
/// explaining the order. [`spectrum::Spectrum::tape`] is what removes both. The signature is now
/// the truth about what this does, which is also what lets the caller keep holding the machine.
fn tape_frames(machine: &Spectrum) -> u64 {
    let frame = u64::from(machine.ula().clock().timing().frame_t_states());
    let total: u64 = machine.tape().pulses().iter().copied().map(u64::from).sum();
    total.div_ceil(frame)
}

/// Run frames until the tape has played itself out, and say what that cost.
///
/// The count is printed rather than only used, in the shape `--wav`'s success line already
/// established: *"the total is printed on success so the next index can be read off rather than
/// guessed"*. `--settle` for a game with an animated title still has to be found by sweeping, and
/// a sweep that starts from the frame the tape actually ended on is a much shorter sweep than one
/// that starts from zero.
fn wait_for_the_tape(machine: &mut Spectrum, recorder: &mut Recorder, ends_at: Option<u64>) {
    let Some(end) = ends_at else {
        // Not silence: `--keys-after` without `--play-tape` is a legitimate run — it is how a
        // `.z80` gets a second keypress — but it is also what a forgotten flag looks like, and
        // the two are worth telling apart before somebody sweeps `--settle` for an hour.
        println!("--keys-after: no tape was started, so nothing was waited for");
        return;
    };
    let waited = end.saturating_sub(recorder.elapsed);
    recorder.advance(machine, waited);
    println!("--keys-after: waited {waited} frames; the tape ran out at frame {end}");
}

/// Hold `tap` for `hold` frames, then release for the same, through the real keymap.
fn press(machine: &mut Spectrum, recorder: &mut Recorder, tap: &[KeyCode], hold: u64) {
    for _ in 0..hold {
        keymap::apply(|code| tap.contains(&code), machine.keyboard_mut());
        recorder.frame(machine);
    }
    for _ in 0..hold {
        keymap::apply(|_| false, machine.keyboard_mut());
        recorder.frame(machine);
    }
}

fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut roms = Vec::new();
    let mut media = Vec::new();
    let mut out = None;
    let mut frames = DEFAULT_FRAMES;
    let mut settle = SETTLE_FRAMES;
    let mut hold = HOLD_FRAMES;
    let mut play_tape = false;
    let mut wav = None;
    let mut wav_from = 0;
    let mut taps = Vec::new();
    let mut after = Vec::new();

    let mut rest = arguments.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            // Repeated rather than overriding, and it is the same rule `zx`'s `partition`
            // follows: the count is what names the machine. One `--rom` is a 48K, two are a
            // 128 in the order given. This is why the shot of a 128 needs no `--model` flag
            // and no second construction path — the ROMs the user named already say it.
            "--rom" => roms.push(next(&mut rest, flag)?),
            // A tape or a snapshot, dispatched by extension through `media::kind_of` — the same
            // decision the window makes, in the same place. This binary photographed only what
            // a bare ROM does until M8; a game is media, so a headless picture of a game needs
            // this. Explicit rather than a bare positional path, so an unknown argument stays
            // an error rather than being silently taken for a filename.
            "--media" => media.push(next(&mut rest, flag)?),
            "--out" => out = Some(next(&mut rest, flag)?),
            "--frames" => {
                let raw = next(&mut rest, flag)?;
                frames = raw
                    .parse()
                    .map_err(|_| format!("--frames wants a whole number, not {raw:?}"))?;
            }
            "--settle" => {
                let raw = next(&mut rest, flag)?;
                settle = raw
                    .parse()
                    .map_err(|_| format!("--settle wants a whole number, not {raw:?}"))?;
            }
            "--hold" => {
                let raw = next(&mut rest, flag)?;
                hold = raw
                    .parse()
                    .map_err(|_| format!("--hold wants a whole number, not {raw:?}"))?;
            }
            "--play-tape" => play_tape = true,
            "--wav" => wav = Some(next(&mut rest, flag)?),
            "--wav-from" => {
                let raw = next(&mut rest, flag)?;
                wav_from = raw
                    .parse()
                    .map_err(|_| format!("--wav-from wants a whole number, not {raw:?}"))?;
            }
            "--keys" => taps = script(&next(&mut rest, flag)?)?,
            // The same parser, so a name that works before the tape works after it and a typo is
            // an error on both sides. There is no second vocabulary to learn and none to drift.
            "--keys-after" => after = script(&next(&mut rest, flag)?)?,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    if roms.is_empty() && media.is_empty() {
        // A bundled build supplies what nobody named, exactly as `host::arguments` does for the
        // window — and through the same `host::partition`, so a `--rom`-shaped embedded ROM is
        // sorted from an embedded tape by the one function that makes that decision anywhere.
        let (bundled_roms, bundled_media) =
            host::partition(&bundle::arguments(), media::DEFAULT_ROM);
        roms = bundled_roms;
        media = bundled_media;
    } else if roms.is_empty() {
        roms.push(media::DEFAULT_ROM.to_owned());
    }
    Ok(Options {
        roms,
        media,
        out: out.ok_or("--out is required")?,
        frames,
        settle,
        hold,
        play_tape,
        wav,
        wav_from,
        taps,
        after,
    })
}

/// The value following `flag`.
fn next<'a>(rest: &mut impl Iterator<Item = &'a String>, flag: &str) -> Result<String, String> {
    rest.next()
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

/// Parse `P;LeftControl+P;Enter` into taps of host keys.
///
/// `;` separates taps and `+` joins the keys held together for one. Names are resolved by
/// [`keymap::code_named`], so only keys the window can press are reachable and a typo is an
/// error rather than a silently ignored tap.
fn script(source: &str) -> Result<Vec<Vec<KeyCode>>, String> {
    source
        .split(';')
        .map(str::trim)
        .filter(|tap| !tap.is_empty())
        .map(|tap| {
            tap.split('+')
                .map(str::trim)
                .map(|name| {
                    keymap::code_named(name)
                        .ok_or_else(|| format!("{name:?} is not a key this emulator binds"))
                })
                .collect()
        })
        .collect()
}

/// Does what this binary tells a person it accepts match what it actually accepts, and does
/// `--hold` still mean what its comment says?
///
/// # Why the checks are here and not in `tests/`
///
/// [`MEDIA_FORMATS`] is private to a binary, and an integration test links the *library* — it
/// cannot see it. `main.rs` met the same wall over `OPENING_MESSAGE` and settled it the same
/// way, in as many words: *"a `#[cfg(test)] mod tests` inside the binary can see its private
/// constants."* A binary target's unit tests are compiled and run by `cargo test` like any
/// other, so this costs no new public item and no library module.
///
/// The alternative was reading this file as **text** from `tests/bundled_extensions.rs`, which is
/// what that file does to `build.rs`. It is the weaker instrument and is used there only because
/// a build script genuinely cannot be linked: it grades the source, where this grades the value
/// the program prints.
///
/// [`MEASURED_KEY_REPEAT_DELAY_FRAMES`] arrives at the same wall from the other side and settles
/// it the same way. It is private, and so are [`press`] and [`script`] — and those two are the
/// point rather than an obstacle: an integration test could only re-implement *hold a tap, then
/// release it, re-applying the keyboard between frames*, and would then be grading its own copy
/// of the thing whose threshold is in question. Here the run goes through the binary's own two
/// functions, so what is measured is what `--keys --hold N` does.
#[cfg(test)]
mod tests {
    use spectrum::screen;

    use super::*;

    /// The four taps of `LOAD ""`, exactly as `docs/images/README.md` publishes them.
    ///
    /// Not a script written for this test. Every tape picture in that gallery was taken with
    /// this string, so the threshold graded below is the threshold of the command a person
    /// following that page actually runs.
    const LOAD_SCRIPT: &str = "J;LeftControl+P;LeftControl+P;Enter";

    /// `LOAD` keywords [`LOAD_SCRIPT`] types, at one acceptance per tap. The `J` is the whole
    /// token, because a 48K starts a line in `K` mode.
    const KEYWORDS: usize = 1;

    /// Quotes [`LOAD_SCRIPT`] types, at one acceptance per tap — one from each `SYMBOL SHIFT`+`P`.
    const QUOTES: usize = 2;

    /// Where the 48K ROM keeps `REPDEL`, the repeat delay its keyboard routine counts down in
    /// frames while a key is held. The address is from the 48K system-variable table.
    const REPDEL: u16 = 0x5C09;

    /// What one run of [`LOAD_SCRIPT`] left the 48K editor holding.
    struct Typed {
        /// `REPDEL` as the booted machine holds it, before a key is touched.
        repeat_delay: u8,
        /// The screen after the three taps that spell `LOAD ""`, before `ENTER`.
        ///
        /// Every row joined rather than row 23 picked out: which row the line lands on is a
        /// function of how far it has wrapped, and at `--hold 60` it takes two of them.
        typed: String,
        /// The screen once `ENTER` has been held, released, and settled.
        settled: String,
    }

    /// Type `LOAD ""` at a fresh 48K, holding every key for `hold` frames.
    fn type_load_at(rom: &[u8], hold: u64) -> Typed {
        let mut machine = media::start(&[rom]).expect("one ROM is a 48K");
        let mut recorder = Recorder::new(&machine, false, 0);
        recorder.advance(&mut machine, DEFAULT_FRAMES);
        let repeat_delay = machine.memory().read(REPDEL);

        // Split at the last tap, because that is where the editor's answer changes: the first
        // three put a line together and `ENTER` decides whether it parses.
        let taps = script(LOAD_SCRIPT).expect("the gallery's own script");
        // The script and the two counts are one statement of the same fact, so they are tied
        // together rather than left to drift: a keyword tap, a tap per quote, and `ENTER`. A tap
        // added to `LOAD_SCRIPT` without moving the counts would otherwise be graded against the
        // old expectation and read as a repeat.
        assert_eq!(
            taps.len(),
            KEYWORDS + QUOTES + 1,
            "{LOAD_SCRIPT:?} is no longer a keyword, {QUOTES} quotes and an ENTER",
        );
        let (typing, submitting) = taps.split_at(taps.len() - 1);
        for tap in typing {
            press(&mut machine, &mut recorder, tap, hold);
        }
        let typed = screen::read_text(machine.memory()).concat();
        for tap in submitting {
            press(&mut machine, &mut recorder, tap, hold);
        }
        recorder.advance(&mut machine, SETTLE_FRAMES);

        // A blank screen is one of the two readings below, so a machine that had stopped would
        // supply half the expected answer. It has not stopped: a `Spectrum` fault is documented
        // next door as "a finding, not a condition to handle".
        assert!(
            machine.fault().is_none(),
            "the machine faulted while typing at --hold {hold}, so neither reading means anything",
        );
        Typed {
            repeat_delay,
            typed,
            settled: screen::read_text(machine.memory()).concat(),
        }
    }

    /// The upper edge of `--hold`, asserted where the behaviour changes.
    ///
    /// # Two holds and not a sweep, because a sweep pins samples
    ///
    /// `docs/images/README.md` measured eighteen settings and published the ramp: *"35 is the
    /// longest hold the editor still accepts, the first duplicate arrives at 36, and every
    /// further five frames buys one more"*. Re-recording a row of that table here would gate a
    /// **reading** rather than the edge, and the two come apart: a delay of 30 with a period of
    /// 6 puts six keywords on the screen at `--hold 60` exactly as 35-and-5 does, and duplicates
    /// at 35. A test pinned to the sixty-frame row stays green while the number a person setting
    /// `--hold` actually needs has moved five frames.
    ///
    /// So this runs the two settings either side of [`MEASURED_KEY_REPEAT_DELAY_FRAMES`] and
    /// asserts the change of behaviour across them. Both holds are derived from that constant
    /// rather than written out, so a wrong constant cannot be satisfied by moving a literal: it
    /// reddens on both sides at once, and the ROM's own `REPDEL` below says whether the tool or
    /// the ROM image is what moved.
    ///
    /// The gallery's page owns the measurement and this owns the property. Neither restates the
    /// other, which is what keeps the number in one place.
    #[test]
    fn one_frame_past_the_repeat_delay_the_editor_types_every_tap_twice() {
        let path = testsupport::testdata_dir().join("roms").join("48.rom");
        let Ok(rom) = std::fs::read(&path) else {
            testsupport::skip_absent_corpus("the Sinclair 48K ROM", &path);
            return;
        };

        let accepted = type_load_at(&rom, MEASURED_KEY_REPEAT_DELAY_FRAMES);
        let repeated = type_load_at(&rom, MEASURED_KEY_REPEAT_DELAY_FRAMES + 1);

        // Where the number comes from. Without this the constant is a figure somebody swept for;
        // with it, the sweep and the ROM's own parameter are two independent derivations of one
        // fact, and a disagreement names which of them changed.
        assert_eq!(
            u64::from(accepted.repeat_delay),
            MEASURED_KEY_REPEAT_DELAY_FRAMES,
            "the ROM in {} boots with a repeat delay of {} frames, so the threshold this file \
             pins is not that ROM's",
            path.display(),
            accepted.repeat_delay,
        );

        // At the delay, every tap lands once and the line is the one that was typed. This is
        // also the positive control for everything below: a run that typed nothing would fail
        // here rather than pass the "it was submitted" assertion vacuously.
        assert_eq!(
            (
                accepted.typed.matches("LOAD").count(),
                accepted.typed.matches('"').count()
            ),
            (KEYWORDS, QUOTES),
            "at --hold {MEASURED_KEY_REPEAT_DELAY_FRAMES} the editor should hold exactly what \
             was typed: {:?}",
            accepted.typed.trim(),
        );

        // One frame more and **every** tap doubles — not only the keyword. That is the half the
        // gallery's earlier wording hid, and it is why the quotes are counted too: a repeat
        // confined to `J` would leave the quote count alone and still read as a duplicate.
        assert_eq!(
            (
                repeated.typed.matches("LOAD").count(),
                repeated.typed.matches('"').count()
            ),
            (2 * KEYWORDS, 2 * QUOTES),
            "at --hold {} every tap should have arrived twice: {:?}",
            MEASURED_KEY_REPEAT_DELAY_FRAMES + 1,
            repeated.typed.trim(),
        );

        // And the consequence, which is the part a person meets: at the delay `ENTER` submits
        // the line and it leaves the editor. One frame more and it is a syntax error that stays
        // on screen, so the tape is never asked for.
        assert_eq!(
            accepted.settled.matches("LOAD").count(),
            0,
            "at --hold {MEASURED_KEY_REPEAT_DELAY_FRAMES} ENTER should have submitted the line, \
             and it is still on the screen: {:?}",
            accepted.settled.trim(),
        );
        assert_eq!(
            repeated.settled.matches("LOAD").count(),
            2 * KEYWORDS,
            "at --hold {} the doubled line cannot parse, so it should still be in the editor \
             after ENTER: {:?}",
            MEASURED_KEY_REPEAT_DELAY_FRAMES + 1,
            repeated.settled.trim(),
        );
    }

    #[test]
    fn the_media_formats_line_names_every_format_that_can_be_inserted() {
        // Read out of the table rather than written again, because writing it again is the
        // defect: this sentence and its twin in the usage block both described a four-format
        // emulator for a fortnight after `.tzx` made it five.
        let mut named = 0;
        for &(extension, kind) in media::EXTENSIONS {
            // `--rom` builds the machine; `--media` goes to `media::insert`, which turns a ROM
            // away with `RomAfterStart`. Offering it here would document a failure.
            if kind == media::Kind::Rom {
                continue;
            }
            assert!(
                MEDIA_FORMATS.contains(&format!(".{extension}")),
                "--media accepts .{extension} and says it does not, so a person with a valid \
                 file is told to go away: {MEDIA_FORMATS}",
            );
            named += 1;
        }
        // An `EXTENSIONS` that yielded nothing would satisfy the loop by never entering it —
        // the vacuous pass every gate in this crate carries a counter against.
        assert!(
            named >= 4,
            "only {named} insertable formats were checked — the table has shrunk",
        );
    }

    #[test]
    fn nothing_it_offers_is_something_it_would_refuse() {
        // The other direction, and not symmetry for its own sake: a typo here — `.z8`, `.tzk` —
        // costs somebody a file they were entitled to load and reads as a broken emulator. The
        // loop above cannot see one, because it only asks whether each real extension appears.
        for offered in MEDIA_FORMATS.split(|c: char| !c.is_ascii_alphanumeric() && c != '.') {
            let Some(extension) = offered.strip_prefix('.') else {
                continue;
            };
            let kind = media::kind_of(&format!("a.{extension}"));
            assert!(
                kind.is_some_and(|kind| kind != media::Kind::Rom),
                "--media offers .{extension} and `media::kind_of` does not know it, or knows it \
                 as a ROM: {MEDIA_FORMATS}",
            );
        }
    }
}
