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

use std::process::ExitCode;

use macroquad::input::KeyCode;

use frontend::audio::{self, Resampler};
use frontend::{bundle, host, keymap, media, palette, ppm, wav};
use spectrum::{Frame, Spectrum};

/// The ROM used when the command line names none.
const DEFAULT_ROM: &str = "testdata/roms/48.rom";

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
const HOLD_FRAMES: u64 = 10;

/// The fewest frames a key must be held for the 48K ROM to see it at all.
///
/// Measured, not assumed. Kept beside [`HOLD_FRAMES`] so that a future edit lowering the
/// default has to walk past the reason it cannot go below five.
const MEASURED_KEY_FLOOR_FRAMES: u64 = 5;

const _: () = assert!(
    HOLD_FRAMES > MEASURED_KEY_FLOOR_FRAMES,
    "a default at or below the measured floor drops keys with nothing to explain it"
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
                 \x20                     [--keys SCRIPT] [--hold N] [--settle N]\n\
                 \x20                     [--wav PATH] [--wav-from FRAME]\n\
                 one --rom is a 48K; two are a 128, editor ROM first\n\
                 --media takes a .tap, .z80 or .sna; --hold is frames per key, raise it for a game\n\
                 --play-tape presses PLAY after the keys: a .tap loads nothing without it\n\
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
            // `ay()` is how a frontend asks which machine it is holding — the AY's presence *is*
            // the 128-ness — and it is the same question `src/main.rs` asks for the same reason:
            // a 48K runs at 3,500,000 T-states a second and a 128 at 3,546,900, so the two
            // resample differently.
            cpu_hz: audio::cpu_hz(machine.ay().is_some()),
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
        if let Some(reason) = media::unsupported(path) {
            return Err(format!("{path}: {reason}"));
        }
        let kind =
            media::kind_of(path).ok_or_else(|| format!("{path}: not a .tap, .z80 or .sna"))?;
        media::insert(&mut machine, kind, &bytes).map_err(|error| format!("{path}: {error}"))?;
    }

    let mut recorder = Recorder::new(&machine, options.wav.is_some(), options.wav_from);
    recorder.advance(&mut machine, options.frames);
    for tap in &options.taps {
        press(&mut machine, &mut recorder, tap, options.hold);
    }
    // After the keys, not before: the real sequence is `LOAD ""`, ENTER, then PLAY, and a tape
    // running while the ROM is still booting would have the loader meet the middle of a block —
    // which is the reason `media::insert` inserts it stopped in the first place.
    if options.play_tape {
        machine.tape_mut().play();
    }
    if !options.taps.is_empty() {
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
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    if roms.is_empty() && media.is_empty() {
        // A bundled build supplies what nobody named, exactly as `host::arguments` does for the
        // window — and through the same `host::partition`, so a `--rom`-shaped embedded ROM is
        // sorted from an embedded tape by the one function that makes that decision anywhere.
        let (bundled_roms, bundled_media) = host::partition(&bundle::arguments(), DEFAULT_ROM);
        roms = bundled_roms;
        media = bundled_media;
    } else if roms.is_empty() {
        roms.push(DEFAULT_ROM.to_owned());
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
