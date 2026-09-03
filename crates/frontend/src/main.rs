//! A window with a ZX Spectrum in it.
//!
//! ```sh
//! cargo run --release --manifest-path crates/frontend/Cargo.toml -- \
//!     testdata/roms/48.rom testdata/tapes/z80doc.tap
//! ```
//!
//! Files are told apart by extension, in any order: `.rom` builds the machine, `.tap` and `.tzx`
//! go in the drive, `.z80` and `.sna` are restored over the top. With no ROM named, the machine
//! is built from `testdata/roms/48.rom`. **Files can also be dropped on the window**, on every
//! target, and go through the same two functions.
//!
//! In a browser the same files are named by the query string —
//! `?rom=roms/48.rom&tape=games/thing.tap` — which becomes the same argument list this shell
//! already reads. See [`frontend::host::arguments_from_query`] and `web/README.md`.
//!
//! | Key | |
//! |---|---|
//! | `Shift` | `CAPS SHIFT` — either hand |
//! | `Ctrl` or `Tab` | `SYMBOL SHIFT` — either hand, and `Tab` because a browser keeps `Ctrl`+digit for itself |
//! | `Backspace`, `Escape`, `,` `.` `;` `'` `-` `=` `/` | the combination the Spectrum prints on the key |
//! | arrows | whichever scheme `F7` has selected — **`5`/`6`/`7`/`8` plus the Kempston port by default**, and *not* the cursor chord the Spectrum prints on those keys |
//! | `F1` | show or hide the pacing readout |
//! | `F2` | save a `.z80` |
//! | `F3` `F4` `F5` | tape play, stop, rewind |
//! | `F6` | reset |
//! | `F7` | what the arrow keys send — **starts on the setting for games**, and `F7` reaches the BASIC cursor keys |
//! | `F8` | how fast the machine runs — 1× → 4× → 16× → 64× → auto → 1×, shown on the readout as `speed` |
//!
//! `F8` is the answer to a tape taking three minutes. Nothing is bypassed: the machine is the same
//! machine and the tape is still the signal `docs/M6.md` Decision 4 insists on, so every loader
//! works — the wall clock is simply asked to hand over more of itself per tick. At the top fixed
//! rung a three-minute cassette is **three seconds**, which is measured rather than hoped for and
//! is derived in [`frontend::pacing::RUNGS`] from what this host actually emulates. Sound is muted
//! above 1× and the reason is at the push site below.
//!
//! **`auto` is the last rung and is the one to press if the answer wanted is *"do not make me
//! choose"*.** It runs flat out while the **machine is decoding a tape** and at real time the
//! instant it stops, so a cassette goes as fast as this host can manage and the game arrives at
//! the speed it was written for, with no second keypress and no number to get wrong. Reading the
//! machine rather than the drive is what makes the order of the two gestures stop mattering:
//! pressing PLAY *before* typing `LOAD ""` is what a person does, it is free on real hardware
//! because the leader is five seconds long, and keyed off the motor it spent those five seconds
//! in 0.055 s. The readout says
//! `speed auto (loading)` while it is working and `speed auto` when it is not, because a machine
//! running flat out and a machine with a broken clock look identical from outside.
//! [`frontend::pacing::Rung::Automatic`] carries the whole argument.
//!
//! The arrows are the one choice this emulator cannot make once and be right about, because the
//! Spectrum has no arrow keys and games disagree about what to read. See
//! [`frontend::keymap::ARROW_SCHEMES`], which records what six games were disassembled to find.
//!
//! # This file is the untestable part, and it is kept thin on purpose
//!
//! Everything with a decision in it — which key, which colour, how many frames, which file —
//! is in the library next door and is reachable from `cargo test`. What is left here needs a
//! GPU and a window and cannot run headless, so it is held to plumbing: poll, upload, draw,
//! await. `crates/frontend/src/lib.rs` carries the table of what that leaves ungraded, and
//! *"whether it looks right"* is the first row.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::time::Duration;

use macroquad::prelude::*;

use frontend::audio::{self, Resampler};
use frontend::bundle;
use frontend::drive;
use frontend::keymap::Hotkey;
use frontend::media;
use frontend::pacing::{self, EarMeter, LossMeter, Pacer, RateMeter, Rung, Speed, Tick};
use frontend::viewport::Viewport;
use frontend::{host, keymap, palette, viewport};
use spectrum::screen::{FRAME_HEIGHT, FRAME_WIDTH};
use spectrum::{Frame, Model, Spectrum};

/// What `F2` writes, before the collision counter and the extension.
const SAVE_STEM: &str = "snapshot";

/// Whole frame pixels per window pixel when the window first opens.
///
/// Three puts a 320 × 256 frame at 960 × 768, which is a comfortable window on any display
/// made this century and still leaves room to enlarge.
const INITIAL_SCALE: i32 = 3;

/// Seconds each rate window covers. See [`RateMeter::new`].
const RATE_WINDOW: f64 = 1.0;

/// Seconds of lost frames the readout's colour looks back over. See [`LossMeter`].
///
/// Held separately from [`RATE_WINDOW`] rather than sharing it, though the two agree today. They
/// are answers to different questions — how long a rate needs to read steadily, and how long a
/// stall should stay visible after it stops — and one constant serving both would make a change to
/// either silently move the other.
///
/// A second is the value both questions happen to land on. [`LossMeter`] spans the open window and
/// the one before it, so the bar reddens on the frame a stall happens and clears between one and
/// two seconds after the last lost frame: long enough that a person glancing at the bar sees it,
/// short enough that it is plainly reporting *now* rather than *earlier*. Half a second clears
/// before a glance lands; two seconds and the red has stopped meaning the present.
const LOSS_WINDOW: f64 = 1.0;

/// Height of the status bar, in window pixels.
const STATUS_HEIGHT: f32 = 22.0;

/// Left margin of every status row, in window pixels.
const STATUS_MARGIN: f32 = 6.0;

/// What the status line says before anything has happened.
///
/// # A page has no manual, so the manual is the first line of the status bar
///
/// `docs/M8.md` Decision 2 rules that `Tab` becomes a third alias for `SYMBOL SHIFT` because a
/// browser takes `Ctrl`+`8` and `Ctrl`+`9` — which are `(` and `)`, which no BASIC program
/// avoids — and then says what that ruling costs: *"The alias does not remove the sharp edge,
/// it provides a way around it, and a person has to know the way around exists. That is a
/// documentation problem with a real failure mode."*
///
/// A person who opened a URL never reads `README.md`. This line is where they find out, and it
/// is shown on both targets rather than only in a browser: the alias exists on both, and a
/// message that appears on one platform is a second thing to keep true.
///
/// # It said `F7 changes what the arrows do`, and that was not enough
///
/// It was true, it was on the screen from the first frame, and the owner still met a default
/// that made Manic Miner's Willy jump on every arrow press with no idea he needed the key. The
/// sentence told him a control **existed**; what he needed was a reason to reach for it, and
/// *"changes what the arrows do"* reads as an option rather than as a fix.
///
/// So it now names the trade instead: the arrows start on the setting for **games**, and `F7` is
/// where the BASIC cursor keys went. That is also the honest counterpart to moving the default —
/// somebody who opens this to type BASIC has had something taken away, and this is the one place
/// they are told where it is.
///
/// # It has a row to itself, because it did not fit on the shared one
///
/// Measured 2026-09-01, before this change: with the message appended to the readout the line was
/// **178 characters**, and the window [`window_conf`] opens holds **136**. The last 42 characters
/// were off the right-hand edge — the line stopped at *"F7 for the"*, cutting the sentence in half
/// and taking `drop a .tap/.z80/.sna` off the screen entirely. The instruction written because a
/// person could not discover the arrow key was itself undiscoverable, and it had been over the
/// edge since well before this message grew: the readout alone spends 65 of the 136 columns.
///
/// Nothing in the repository could see it. `tests/on_screen_strings.rs` grades these strings for
/// *drawable characters* and has no way to ask how **wide** they are, which is the same shape as
/// the em-dash defect that file was written about — an assertion comparing a string to another
/// string, when the thing that failed was the picture.
///
/// So the readout and the message are two rows now, and `mod tests` below measures both against
/// the window rather than leaving it to somebody to notice on a screenshot.
///
/// The line break is inside a phrase rather than at a separator on purpose: `\` at the end of a
/// Rust string literal eats the newline **and the next line's indentation**, so a break placed
/// just before `-   drop` silently ate two of that separator's three spaces.
///
/// # What it lists is now graded against the list itself
///
/// It said `.tap/.z80/.sna` for the fortnight after `.tzx` became loadable: **the most visible
/// string in the project**, drawn on the status bar from the first frame, describing an emulator
/// one format smaller than the one drawing it. Width was gated here and contents were not, which
/// is the narrower version of the mistake this file's own header records — measuring the string
/// against another string when the thing that failed was the picture.
///
/// So `mod tests` below reads [`media::EXTENSIONS`] and asserts this line names every extension a
/// *running* machine accepts. Not `.rom`: a ROM is what a machine is built **of**, `media::insert`
/// refuses one outright, and telling somebody to drop a file that will be turned away is worse
/// than not mentioning it.
const OPENING_MESSAGE: &str = "Tab or Ctrl = SYMBOL SHIFT   -   arrows are set for games; \
     F7 for the BASIC cursor keys   -   drop a .tap/.tzx/.z80/.sna";

/// Point size of the status text.
const STATUS_TEXT: f32 = 16.0;

/// What the `snd` field says when no audio device has appeared. See [`Status::queue`].
const NO_DEVICE: &str = "--";

/// What the `snd` field says when a device is being sent nothing. See [`Status::queue`].
const MUTED: &str = "mute";

/// The second line [`complain`] draws, under whatever went wrong.
///
/// A constant rather than an inline literal so that `mod tests` can measure it. It is the same
/// class of string as [`OPENING_MESSAGE`] — a fixed instruction that has to fit the window it is
/// drawn in — and the same gate should cover both.
///
/// Neither "on the command line" nor "in the query string" alone: this window is the same binary
/// in both places, it cannot tell which it is in without a `#[cfg]` this crate does not have, and
/// a person reading it can. Naming both is the honest form and it is also the useful one —
/// somebody who reached a broken page needs the URL shape, and `docs/M8.md` Decision 3 flagged
/// the old line for saying only the other.
const COMPLAIN_ADVICE: &str =
    "name one 16 KB .rom (48K) or two (128): on the command line, or as ?rom=path in the URL";

fn window_conf() -> Conf {
    Conf {
        window_title: "ZX Spectrum 48K".to_owned(),
        window_width: FRAME_WIDTH as i32 * INITIAL_SCALE,
        window_height: FRAME_HEIGHT as i32 * INITIAL_SCALE,
        window_resizable: true,
        // The frame is 320 × 256 of hard-edged pixels, so a retina display is worth having:
        // it lets the integer scale in `viewport::fit` land on a bigger whole number.
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let arguments = host::arguments();
    let (rom_paths, media_paths) = host::partition(&arguments, media::DEFAULT_ROM);

    let mut roms = Vec::with_capacity(rom_paths.len());
    for path in &rom_paths {
        match load(path).await {
            Ok(bytes) => roms.push(bytes),
            Err(message) => return complain(&message).await,
        }
    }
    let borrowed: Vec<&[u8]> = roms.iter().map(Vec::as_slice).collect();
    let mut machine = match media::start(&borrowed) {
        Ok(machine) => machine,
        Err(error) => return complain(&format!("{}: {error}", rom_paths.join(", "))).await,
    };

    let mut video = Video::new();
    let mut status = Status::new();

    for path in media_paths {
        match load(&path).await {
            Ok(bytes) => status.report(media::accept(&mut machine, &path, &bytes)),
            Err(message) => status.report(message),
        }
    }

    let mut pacer = Pacer::new();
    let mut meter = RateMeter::new(RATE_WINDOW, get_time());
    let mut loss = LossMeter::new(LOSS_WINDOW, get_time());
    // The frame length comes off the machine rather than from a literal, for the reason
    // `EarMeter::new` gives: a 128's frame is 1020 T-states longer and its threshold differs.
    // Built once, here, because the model cannot change under a running window — `Spectrum::restore`
    // refuses a snapshot of the other machine.
    let mut ear = EarMeter::new(machine.ula().clock().timing().frame_t_states());

    // The machine's own clock decides its sample rate, and the two machines differ: 3,500,000
    // against 3,546,900 T-states a second.
    //
    // **This asked `ay().is_some()` and excused it in a comment that had stopped being true.**
    // The comment said `Spectrum::model()` was "still absent" and that "the AY's presence *is*
    // the 128-ness, so this is the honest question rather than a workaround for a missing
    // accessor". The accessor landed with M7 — `crates/spectrum/src/lib.rs` records that it was
    // added because `media::insert`'s `.tzx` arm needed the model to count a turbo loader's
    // T-states — so the second half of that argument is what is left, and it does not stand on
    // its own: what sets the clock rate is the *model*, and a machine is not fast because it has
    // a sound chip. The two agree today and agree by coincidence. Asking the question this line
    // actually means costs one comparison and stops a +2A — a 128-clocked machine — from turning
    // on whether its AY was wired.
    let machine_hz = audio::cpu_hz(machine.model() == Model::Spectrum128);
    // `None` until a device exists. In a browser that is after the first click or keystroke,
    // because an `AudioContext` is suspended until the user has interacted with the page and no
    // desktop browser can be argued out of it; on a desktop it is after the device opens.
    let mut resampler: Option<Resampler> = None;
    // Allocated once and refilled in place, like `Video`'s buffers and for the same reason:
    // "a leak-shaped mistake rather than a slow one", fifty times a second.
    //
    // Empty rather than sized, because the size is a device's and no device exists yet — the
    // reserve happens where the rate arrives, below. What that costs is the doubling chain of
    // the first few frames, about four reallocations once, which is named here rather than left
    // to look like an oversight.
    let mut mixed: Vec<f32> = Vec::new();

    loop {
        for &(code, action) in keymap::HOTKEYS {
            if is_key_pressed(code) {
                act(action, &mut machine, &mut status);
            }
        }
        for file in get_dropped_files() {
            let message = accept_drop(&mut machine, &file);
            // A dropped cassette replaces what is in the drive, so the tape that was turning a
            // moment ago is simply gone. That is not a tape running out, and without this the
            // next tick would say so over the top of the message naming the file. See
            // [`drive::Drive::follow`].
            status.drive.follow(machine.tape());
            status.report(message);
        }
        // Both devices are rebuilt from the host's state every frame. The joystick needs it
        // more than the membrane does: it is active high and has no interlock, so a direction
        // the host stops reporting — a lost window focus, a backgrounded browser tab — would
        // otherwise stay pressed for the rest of the session.
        let scheme = &keymap::ARROW_SCHEMES[status.arrows];
        let joystick = keymap::apply_with(is_key_down, machine.keyboard_mut(), scheme);
        *machine.joystick_mut() = joystick;

        // Pushed into the pacer every frame from the one place that owns the choice, exactly as
        // the keyboard and the joystick above are rebuilt from the host every frame. It is a push
        // rather than a second copy: `Status` decides, `Pacer` obeys, and there is no third place
        // the two could drift apart in. `keymap::Hotkey` records what shadowing costs.
        //
        // **The machine is asked here rather than remembered**, for the same reason. A guest
        // stops reading the `EAR` line the moment its loader finishes, so a cached answer would
        // keep `Rung::Automatic` running flat out into the game — which is the shape of the bug
        // `spectrum::tape::Tape::is_playing` was added to remove, one signal along.
        //
        // **This asked the drive, and the drive is not the machine.** `pacing::Rung::Automatic`
        // carries what that cost: a turning motor with nobody listening is a cassette being spent
        // at 90×, so pressing PLAY before typing `LOAD ""` — free on every real Spectrum, because
        // the leader is five seconds long — burned the tape in 0.055 s. `EarMeter` reads how hard
        // the machine is sampling the line instead, which covers the load *and* the wait before
        // it, and is sampled before the decision rather than after so the rate describes frames
        // that have actually run.
        ear.sample(machine.ear_reads(), machine.frames());
        let tick = pacing::RUNGS[status.speed].this_tick(ear.decoding());
        run_tick(tick, &mut machine, &mut pacer);
        // **Asked here, immediately after the frames that could have ended the tape.** Under
        // `Rung::Automatic` a three-minute cassette reaches its end in about two seconds, so a
        // report arriving a tick late is a report nobody can connect to anything.
        report_a_finished_tape(&machine, &mut status);
        // One clock reading for both, so the figure and the colour describe the same second
        // rather than two instants a few microseconds apart.
        let now = get_time();
        meter.sample(now, pacer.ran());
        loss.sample(now, pacer.dropped());

        // Polled every frame until a device appears, then built once for that exact rate.
        //
        // **`audio_rate` is never asked again after that, and the consequence is worth stating.**
        // A device that changes its rate mid-session — an output switched from speakers to a
        // 44.1 kHz interface — keeps being resampled to the old one, which is a pitch error and
        // not a crash. Rebuilding the resampler would discard its phase and its DC-blocker
        // history, so it is a click at the moment of the switch against a wrong pitch until the
        // next launch; neither is obviously right and nothing here has measured it.
        //
        // What is *not* left to this poll is the device going **away**. A suspended browser
        // `AudioContext` stops draining, and a push that kept succeeding into it would grow the
        // backlog and freeze the depth this loop steers on — the loop would then be running
        // open-loop on a constant while memory and latency climbed for as long as the tab lived.
        // `zx_audio_push` answers `-1` while the context is not `running`, the same value and the
        // same reason as `zx_audio_rate`, so `track` sees `None` and leaves the rate alone.
        if resampler.is_none() {
            let device_hz = page::audio_rate();
            if device_hz != 0 {
                // One frame of output plus slack, now that the rate is known: 50 is the frame
                // rate to within a fifth of a percent on both machines, and the correction
                // above can only move the count by 0.5%.
                mixed.reserve(device_hz as usize / 50 + 64);
                resampler = Some(Resampler::new(machine_hz, device_hz));
            }
        }
        // Drained **every** frame whether or not anything is listening: the machine buffers two
        // frames and then starts counting what it lost, and a consumer that stops taking would
        // make `Spectrum::dropped_samples` climb for a reason that has nothing to do with audio.
        let produced = machine.take_samples();
        // **Above real time the device is fed nothing, and that is a decision rather than an
        // omission.** A sound card consumes exactly one second per second whatever the pacer
        // does, so at 64× the resampler offers it sixty-four seconds of samples per wall second:
        // the queue grows at sixty-three times real time until the ceiling below clamps it, and
        // what comes out after that is one frame in sixty-four, chosen by an accounting estimate
        // rather than by anything musical — twenty-millisecond fragments of unrelated moments,
        // which is a fault noise and not a fast tune.
        //
        // **The tape's own screech is the case worth naming, because somebody will want it.** It
        // is the sound a Spectrum is remembered for and it is the one thing playing while `F8` is
        // most useful. It still goes: a loading tone is a square wave a couple of kilohertz wide,
        // and one frame in sixty-four of it is not a fast screech but a click every twenty
        // milliseconds — the sampling artefact, not the signal. Somebody who wants to hear a tape
        // load has the whole of real time to do it in, and that is `F8` back to `1×`, which is one
        // keypress from the top rung by construction.
        //
        // The two alternatives were weighed and are worse, and the wider range makes both worse
        // rather than better. *Decimating* the mixed buffer aliases a 48 kHz signal down by the
        // multiplier and puts tones in it the machine never made — the same argument
        // `audio::Resampler::feed` already makes against nearest-neighbour picking, six octaves up
        // instead of one. *Resampling at the multiplied ratio* — playing the fast-forward
        // time-compressed — shifts a beeper tune **six** octaves at 64×, which is not "mostly" out
        // of the audible band but entirely out of it, and needs the resampler rebuilt on every
        // speed change, discarding the DC filter history that
        // `feeding_in_pieces_matches_feeding_in_one_go` exists to preserve.
        //
        // So: silence, and the readout says `snd mute` rather than leaving a frozen number that
        // reads as a device still holding samples. `take_samples` above is called either way —
        // draining is not optional, because the machine buffers two frames and then counts what
        // it lost, and `Spectrum::dropped_samples` must keep describing the machine rather than
        // the speed key.
        //
        // **The condition is the tick and not the rung, which is what makes `auto` sound right
        // for free.** Automatic resolves to `Tick::Paced(REAL_TIME)` the moment the drive stops,
        // so the sound comes back on the first tick after a cassette ends without a second rule
        // anywhere — and goes away again the moment somebody presses PLAY. Writing this against
        // the *rung* would have muted an automatic machine that was sitting at real time doing
        // nothing, which is the state a person spends nearly all their time in.
        if let Some(resampler) = resampler.as_mut()
            && tick == Tick::Paced(Speed::REAL_TIME)
        {
            mixed.clear();
            resampler.feed(produced, &mut mixed);
            // **The backlog is closed as a loop, on the returned depth, for both targets.**
            // Observed in a browser on 2026-09-01: `snd 10080` after four minutes — 210 ms of
            // backlog and still climbing, because the emulator was running at 50.2 Hz against a
            // device consuming exactly one second per second. A fifth of a percent is nothing
            // per frame and is an unbounded latency over a session, which is the same
            // self-amplifying shape `crate::pacing` refuses for frames.
            //
            // This used to be a ceiling: past it the frame was not pushed at all. That bounded
            // the latency and **it is what a person heard as a tick every few seconds** — a
            // discarded frame is 20 ms missing from the middle of a waveform, which is a
            // discontinuity, which is a click, recurring on a period set by the drift. The
            // ceiling was doing its job and the job was the wrong shape.
            //
            // `Resampler::track` closes the same loop by moving the output rate a fraction of a
            // percent instead, which is inaudible as pitch and continuous by construction.
            //
            // **The setpoint moved out of this line and into `audio::queue_target`.** It was
            // `device_hz() * page::BUFFER_MILLISECONDS / 2000` written here, and `2000` fused a
            // unit conversion to a ruling — half the buffer — inside a function this file's own
            // header holds *"to plumbing: poll, upload, draw, await"* precisely because nothing
            // in it can be graded. It is a named function with a test now, for the same reason
            // `ink`, `speed_message` and `report_a_finished_tape` are.
            let target = audio::queue_target(resampler.device_hz());
            resampler.track(u32::try_from(status.audio_queued).ok(), target);
            // **The bound survives, as a backstop rather than as the mechanism.** The loop above
            // holds the queue near `target` — half the buffer — and while it is working this
            // branch never fires. What it covers is the case the loop *cannot*: a machine so far
            // behind that 0.5% of rate correction never catches up, where without a ceiling the
            // latency grows for as long as the session lasts. Twice the target is a full buffer,
            // so a frame is only ever discarded after the correction has had the whole of its
            // range and lost.
            //
            // **This is the only bound the two targets share, and it is the one that must not be
            // removed.** Each device has a last-resort bound of its own below it — `desktop::push`
            // drops at the ring's capacity, `zx_audio_worklet.js` drops its oldest samples when
            // its ring fills — but those are the device's own self-defence and they are reached
            // only after this branch has already declined to feed it. Deleting this line moved
            // the browser from *bounded* to *unbounded* once before.
            let ceiling = target * 2;
            if status.audio_queued < 0 || status.audio_queued < ceiling as i32 {
                status.audio_queued = page::audio_push(&mixed);
            } else {
                status.audio_queued -= mixed.len() as i32;
            }
        }

        video.draw(&machine);
        status.draw(&machine, pacer, meter, loss, ear);
        next_frame().await;
    }
}

/// Read a file, as a message rather than as an error type.
///
/// `load_file` is macroquad's, not the standard library's, and that is the whole reason this
/// returns a `String`: on `wasm32` it is an HTTP fetch whose failures do not map onto
/// [`std::io::Error`], and the shell's only response to any of them is to put the text on the
/// screen. See [`frontend::host`].
///
/// [`bundle::bytes`] is consulted first, which is what lets a standalone build need no files at
/// all. It is a lookup in a slice of at most two entries and it is the only thing that
/// distinguishes an embedded payload from a fetched one — everything downstream, from
/// `media::kind_of` to `media::insert`, is handed the same name and the same bytes and is never
/// told which it is holding.
async fn load(path: &str) -> Result<Vec<u8>, String> {
    if let Some(embedded) = bundle::bytes(path) {
        return Ok(embedded.to_vec());
    }
    macroquad::file::load_file(path)
        .await
        .map_err(|error| format!("cannot read {path}: {error}"))
}

/// Take a file the user dropped on the window and say what happened.
///
/// # The whole feature is two existing functions and a name
///
/// [`media::insert`] takes bytes somebody else fetched — its module says so in as many words,
/// *"Nothing in this module performs I/O; every entry point takes bytes that somebody else
/// fetched"* — and [`media::kind_of`] takes a name. A drop supplies both, so it needed neither
/// a new path through the machine nor a new error. `docs/M8.md` Decision 5 makes the point that
/// this was not arranged for: the design anticipated a byte source that is neither a command
/// line nor a fetch, without being told one was coming.
///
/// **The name is the file's name and not a path, and in a browser it cannot be anything else.**
/// `miniquad`'s drop handler reads `file.name` from the `DataTransfer` entry; there is no path,
/// because a page is never told where a dropped file lives. That is all [`media::kind_of`]
/// wants.
///
/// **This is [`media::Error::RomAfterStart`]'s first likely route.** It was reachable before
/// only from `zx --rom a.rom b.rom`; dropping a `.rom` on a running page is a thing people will
/// actually do, and the existing error is already the right answer — starting the emulator
/// again is what swaps a ROM, and in a browser that is reloading the page.
///
/// # A hostile file holds the frame loop, and the question is how long
///
/// This runs on the frame thread. `#[macroquad::main]` has one, nothing here spawns another, and
/// the whole chain — `get_dropped_files`, this function, [`media::insert`], the format's parser —
/// is synchronous between two `next_frame().await`s. So a file that takes a long time to refuse
/// *is* a frozen window for exactly that long, and `.tzx` is the first format where a small file
/// can ask for a lot of work: a three-byte jump-to-itself spins until `crates/spectrum`'s block
/// ceiling stops it at 16,777,216 executions.
///
/// **Measured 2026-09-01 rather than reasoned about, through this function, on the three
/// pathological files `crates/spectrum/tests/tzx_hostile.rs` already builds:**
///
/// | File | `--release` | debug |
/// |---|---|---|
/// | a jump to itself (13 bytes) | **47.6 ms** — 2.4 frames | 716 ms — 36 frames |
/// | a loop whose body jumps into itself (17 bytes) | 26.1 ms — 1.3 frames | 678 ms |
/// | 65535 passes over a 65535-pulse tone (19 bytes) | 6.9 ms — 0.3 frames | 90 ms |
///
/// So it is bounded, and in the build a person actually runs the worst case is under a twentieth
/// of a second — one dropped display frame, which is a hitch and not a hang. **Nothing is added
/// here for it.** A worker thread, a progress indicator or a size limit would each be machinery
/// bought against a 48 ms event, and `macroquad`'s drop handler hands over the bytes on the frame
/// thread anyway. What makes this safe is the two ceilings next door; without them the same three
/// bytes would be an unbounded hang and no amount of frontend care would help.
///
/// The readout tells the truth about it either way, which is worth stating because it did not
/// used to. A stall this long is exactly what [`frontend::pacing::Pacer`] counts: in release it
/// owes two frames, stays inside `MAX_CATCH_UP` and loses nothing, so the bar does not colour; in
/// debug it owes 36, loses 32, and the bar goes red — and then, since 2026-09-01, **goes back to
/// grey a second or two later**. Before that fix a single hostile drop would have left the status
/// bar red for the rest of the session.
fn accept_drop(machine: &mut Spectrum, file: &DroppedFile) -> String {
    let name = file
        .path
        .as_deref()
        .and_then(std::path::Path::to_str)
        .unwrap_or("dropped file");
    // `bytes` is an `Option` on both targets and the `None` is reported rather than skipped: a
    // drop that produced a name and no content is the browser or the window system failing at
    // something the user watched themselves do, and silence would read as the emulator
    // ignoring the gesture.
    let Some(bytes) = file.bytes.as_deref() else {
        return format!("{name}: dropped, but no bytes arrived");
    };
    media::accept(machine, name, bytes)
}

/// Run the frames this tick calls for.
///
/// # Why the two arms are down here and not in the frame loop
///
/// They were up there, and the loop's nesting went from three levels to five: a `loop`, a `match`,
/// an arm, and then the `for` that actually runs the frames. This file's header says it is *"held
/// to plumbing"*, and five levels of it is not plumbing — it is the shape a reader has to unpick
/// before finding the one line that runs a frame. Every other decision in this file already sits
/// in a named function for the same reason, [`ink`] most explicitly.
///
/// The two arms are genuinely two loops rather than one with a variable bound, and that is the
/// whole of what [`Tick`] distinguishes. [`Pacer::advance`] computes a count from time that has
/// **already** elapsed and this file runs it; [`Pacer::run_flat_out`] owns its loop because the
/// count can only be discovered by spending time that has not elapsed yet.
///
/// Nothing is returned. The frames reach [`Pacer::ran`] either way, which is what the readout's
/// `Hz` is drawn from, so a second count here would be a number with no reader and somewhere for
/// the two to disagree.
fn run_tick(tick: Tick, machine: &mut Spectrum, pacer: &mut Pacer) {
    match tick {
        Tick::Paced(speed) => {
            pacer.set_speed(speed);
            // `try_from_secs_f32` rather than `from_secs_f32`: the latter panics on a negative or
            // non-finite argument, and `get_frame_time` is a number from a windowing system rather
            // than one this program computed.
            let elapsed = Duration::try_from_secs_f32(get_frame_time()).unwrap_or(Duration::ZERO);
            for _ in 0..pacer.advance(elapsed) {
                machine.run_frame();
            }
        }
        // `get_time` and not `get_frame_time`: the budget is spent *during* this call, so the clock
        // has to move while it is being read. macroquad documents that one of the two does and the
        // other is a per-tick delta, and `Pacer::run_flat_out` says what handing over the wrong one
        // would cost.
        Tick::FlatOut => {
            pacer.run_flat_out(get_time, || machine.run_frame());
        }
    }
}

/// Say so when the tape has run out, which is the one drive change no key made.
///
/// # A named function rather than three lines in the loop, for [`ink`]'s reason
///
/// [`ink`] records what happens to a decision left inside a function no test can reach: putting
/// the old defect back left **every** test green, because `Status::draw` needs a GPU. The frame
/// loop is worse — it needs a window *and* never returns — so a call site in it is not merely
/// hard to grade, it is unreachable. Out here `mod tests` drives it directly, and the assertion
/// that the wiring exists is a machine's rather than a reader's.
///
/// The drive is asked **after** the frames have run, because those frames are what ends a
/// cassette; asking first would report every tape one tick late.
fn report_a_finished_tape(machine: &Spectrum, status: &mut Status) {
    if let Some(message) = status.drive.ran_out(machine.tape()) {
        status.report(message);
    }
}

/// Carry out a hotkey.
fn act(action: Hotkey, machine: &mut Spectrum, status: &mut Status) {
    match action {
        Hotkey::ToggleStatus => status.visible = !status.visible,
        Hotkey::SaveSnapshot => status.report(write_snapshot(machine)),
        // **Each of the three asks the drive afterwards rather than announcing beforehand.**
        // `spectrum::tape::Tape::play` starts nothing on an empty drive and nothing on a tape
        // wound to its end, and this arm used to report `tape playing` in both — a message that
        // reports the keystroke cannot be wrong about the keystroke and cannot be right about
        // anything else. `frontend::drive` carries the whole argument and owns the strings, which
        // is also what makes them reachable from a test; `tests/on_screen_strings.rs` records that
        // literals living here are not.
        Hotkey::PlayTape => {
            machine.tape_mut().play();
            let message = status.drive.played(machine.tape());
            status.report(message);
        }
        Hotkey::StopTape => {
            machine.tape_mut().stop();
            let message = status.drive.stopped(machine.tape());
            status.report(message);
        }
        Hotkey::RewindTape => {
            machine.tape_mut().rewind();
            let message = status.drive.rewound(machine.tape());
            status.report(message);
        }
        Hotkey::Reset => {
            machine.reset();
            status.report("reset");
        }
        // The arrows are a choice, so the choice needs a key and the current one has to be on
        // the screen. A person who presses an arrow and sees nothing move concludes the
        // emulator is broken — the same class as a drop that does nothing — and the only cure
        // is that the mapping is visible and one keystroke from the one they want.
        //
        // **The hint, not just the name.** The status bar carries the name every frame and has
        // room for nothing longer; this fires once, at the moment somebody asked the question,
        // and is where a name like `5678` gets to say what it sends. A name nobody can decode
        // is a readout that reports without informing, which is how the previous default stayed
        // invisible while being wrong.
        Hotkey::CycleArrows => {
            status.arrows = (status.arrows + 1) % keymap::ARROW_SCHEMES.len();
            let scheme = &keymap::ARROW_SCHEMES[status.arrows];
            status.report(format!("arrows: {} - {}", scheme.name, scheme.hint));
        }
        // The same shape as the arrows above, for the same reason: an index into a table, moved
        // by one key, named on the bar every frame. The message says what the rung *costs* as well
        // as what it is — somebody who pressed this and lost the sound needs to be told why by the
        // thing that took it, not by a README they are not reading. `speed_message` below carries
        // the three sentences and the argument for which of them each rung earns.
        Hotkey::CycleSpeed => {
            status.speed = (status.speed + 1) % pacing::RUNGS.len();
            status.report(speed_message(pacing::RUNGS[status.speed]));
        }
    }
}

/// What the bar says at the moment the speed key moves.
///
/// # A function, so that `mod tests` can measure the strings
///
/// This was a `match` inside [`act`]'s own arm, and it took the nesting there to five levels for
/// three one-line answers. Out here it is three, and — the reason that actually matters — the
/// three sentences become **values a test can reach**. `tests/on_screen_strings.rs` records that
/// it *"cannot cover `main.rs`'s own literals … because they are private to a binary that needs a
/// window"*, and every literal that stayed inside `act` is still in that gap. These three are not.
///
/// The message is written from the **rung** rather than from the tick, and that is the one place
/// in this file where the distinction matters to a person. A press of `F8` is a statement about
/// what the machine will do from now on, so `auto` has to announce itself as *the rung that
/// decides* even when the drive happens to be stopped and it is, at this instant,
/// indistinguishable from 1×. The status bar carries the other half — whether it is doing
/// anything right now, via [`Rung::note`] — and the two together are what stop `auto` reading as
/// a key that did nothing.
fn speed_message(rung: Rung) -> String {
    // A guard rather than a `Rung::Fixed(Speed::REAL_TIME)` pattern: `Speed` wraps a `NonZeroU32`,
    // which is not a structural-match type, so the constant cannot appear in a pattern at all.
    // Comparing in a guard is the same question the compiler will actually answer.
    match rung {
        Rung::Automatic => {
            "speed: automatic - flat out while a tape plays, real time otherwise".to_owned()
        }
        Rung::Fixed(speed) if speed == Speed::REAL_TIME => {
            "speed: 1x real time - sound on".to_owned()
        }
        Rung::Fixed(speed) => format!("speed: {}x real time - sound muted", speed.factor()),
    }
}

/// Write the machine out as a numbered `.z80`, and say where it went.
fn write_snapshot(machine: &Spectrum) -> String {
    let path = match host::free_path(SAVE_STEM, "z80") {
        Ok(path) => path,
        Err(error) => return error.to_string(),
    };
    match host::save(&path, &media::save(machine)) {
        Ok(()) => format!("saved {path}"),
        Err(error) => error.to_string(),
    }
}

/// Draw `message` until the window is closed.
///
/// A frontend that cannot start still has a window open — `#[macroquad::main]` opened it
/// before this function ran — so exiting silently leaves a blank rectangle and no
/// explanation. Somebody who double-clicked the binary never sees stderr.
async fn complain(message: &str) {
    loop {
        clear_background(BLACK);
        draw_text(message, 20.0, 40.0, STATUS_TEXT * 1.5, RED);
        draw_text(
            COMPLAIN_ADVICE,
            20.0,
            40.0 + STATUS_TEXT * 2.0,
            STATUS_TEXT,
            GRAY,
        );
        next_frame().await;
    }
}

/// The machine's screen, as a texture on the GPU.
///
/// Three fields, all of them buffers, all built once. The per-frame path allocates nothing:
/// [`Frame`] and the `RGBA` buffer are reused in place and the texture is uploaded over
/// rather than recreated. `docs/ARCHITECTURE.md` makes performance a non-goal, so this is
/// not tuning — a texture rebuilt 50 times a second is a GPU allocation 50 times a second,
/// and that is a leak-shaped mistake rather than a slow one.
struct Video {
    // Not `Box<Frame>`: `Frame` already boxes its own 80 KB of pixels, so it is a pointer
    // wide, and wrapping it again would be a second allocation and a second indirection
    // bought for nothing.
    frame: Frame,
    rgba: Box<[u8; palette::RGBA_BYTES]>,
    texture: Texture2D,
}

impl Video {
    /// Allocate the buffers and the texture.
    fn new() -> Self {
        let rgba = palette::buffer();
        let texture = Texture2D::from_rgba8(FRAME_WIDTH as u16, FRAME_HEIGHT as u16, &rgba[..]);
        // Nearest, always. A Spectrum pixel is a square of flat colour and every filter mode
        // that is not nearest turns the whole screen into a smear at these scales.
        texture.set_filter(FilterMode::Nearest);
        Self {
            frame: Frame::new(),
            rgba,
            texture,
        }
    }

    /// Render the machine and put it on the screen.
    fn draw(&mut self, machine: &Spectrum) {
        machine.render(&mut self.frame);
        palette::write_rgba(&self.frame, &mut self.rgba);
        self.texture
            .update_from_bytes(FRAME_WIDTH as u32, FRAME_HEIGHT as u32, &self.rgba[..]);

        // The letterbox is cleared to the machine's own border, so the margin integer scaling
        // leaves over reads as a wider border rather than as black bars — and changes colour
        // with it when a program writes port 0xFE.
        clear_background(to_color(machine.border()));

        let Viewport {
            x,
            y,
            width,
            height,
            ..
        } = viewport::fit(screen_width(), screen_height());
        draw_texture_ex(
            &self.texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }
}

/// The pacing readout, and whatever happened last.
struct Status {
    visible: bool,
    /// Which of [`keymap::ARROW_SCHEMES`] the arrow keys currently press.
    ///
    /// INVARIANT: in range for that slice. Starts at `0` and is only ever written as
    /// `(x + 1) % ARROW_SCHEMES.len()`, in [`act`] and nowhere else. Six sites index these two
    /// fields raw — three of them in [`Status`]'s own methods, which is a *different type* from
    /// the one enforcing the bound — so the enforcement and the consumption are apart and the
    /// invariant is written down where the reader is, per this crate's own rule. A newtype would
    /// make it structural for about ten lines against a bound that is already true; the comment
    /// is the smaller answer and the finding stays open behind it.
    arrows: usize,
    /// Which of [`pacing::RUNGS`] the machine is being run at.
    ///
    /// The **only** copy of that choice. [`Pacer`] is handed it every frame rather than keeping
    /// its own, so there is nowhere for the two to disagree — the frame loop says why.
    ///
    /// INVARIANT: in range for [`pacing::RUNGS`], on the same terms as [`Status::arrows`] above
    /// and enforced at the same single site.
    speed: usize,
    /// Samples the device still has to play, or a negative number when there is none.
    ///
    /// On the readout because a silent emulator otherwise gives a person nothing to reason
    /// from: a browser tab before its first click, a machine with no sound card and a bug in
    /// the mixer all look identical from the outside. A number that climbs and falls says the
    /// device is alive; `--` says there is not one.
    audio_queued: i32,
    /// The tape drive as this bar last described it, so that a tape ending is news.
    ///
    /// It lives here rather than beside [`Pacer`] in the frame loop because it is a property of
    /// the *readout* and not of the machine: it records what was last said out loud, which is a
    /// question only the thing doing the saying can answer. [`drive::Drive`] carries the argument
    /// for why that is not the shadow copy `keymap` warns about.
    drive: drive::Drive,
    message: Cow<'static, str>,
    /// Reused so the per-frame path formats into one buffer rather than building a new one.
    ///
    /// *This said "without allocating", and one argument in that `write!` still does.*
    /// [`Status::queue`] returns a `String` — 2 to 11 bytes, one malloc and one free per frame —
    /// so the invariant the buffer was engineered for is true of the buffer and false of the
    /// line. It is recorded rather than fixed because every repair measured is worse than the
    /// defect: a `Cow` still allocates in the live case (a number); a `Display` newtype is the
    /// sibling pattern [`pacing::Rung`] uses and costs +11 lines and +3 branches for 11 bytes;
    /// and a `write!`-into-`&mut String` signature would split the single `write!` into three,
    /// destroying the exact property the buffer exists to give. **The finding stays open** —
    /// what is not acceptable is the sentence that was here, because it is the one a future
    /// reader consults when deciding whether a per-frame `format!` is allowed on this path.
    line: String,
}

impl Status {
    /// Visible, showing [`OPENING_MESSAGE`] until something else happens.
    ///
    /// Visible by default because the brief for this frontend is that a machine failing to
    /// keep 50 Hz should be *visible rather than silently drifting*, and a readout somebody
    /// has to know to switch on is the silent case with extra steps. The same argument is why
    /// it opens with the keyboard hint rather than blank: a person who launched from a URL has
    /// no other place to be told.
    fn new() -> Self {
        Self {
            visible: true,
            arrows: 0,
            speed: 0,
            audio_queued: -1,
            drive: drive::Drive::new(),
            message: Cow::Borrowed(OPENING_MESSAGE),
            line: String::with_capacity(128),
        }
    }

    /// Replace the message shown alongside the readout.
    /// Takes `impl Into<Cow<'static, str>>` rather than `String`, so the call sites whose message
    /// is a `&'static str` stop allocating a copy of it to hand over. Each was 12 to 48 bytes on a
    /// keypress path, which is not the point — the point is that `.to_owned()` on a literal reads
    /// as required by the signature when it is required by nothing. A `String` from
    /// `write_snapshot` or `speed_message` still converts, as `Cow::Owned`, at no cost.
    fn report(&mut self, message: impl Into<Cow<'static, str>>) {
        self.message = message.into();
    }

    /// What the `snd` field says.
    ///
    /// Three states, and the two that are not a number mean different things. [`NO_DEVICE`] is
    /// *nothing is listening* — a browser tab before its first click, a machine with no sound
    /// card — and [`MUTED`] is *something is listening and is deliberately being sent nothing*,
    /// which is what a multiplier above real time does. Collapsing them would put a silenced
    /// emulator and a broken one behind the same two characters, which is the confusion
    /// [`Status::audio_queued`] exists to prevent, one case wider.
    ///
    /// The frozen number is the case worth naming: while fast-forwarding nothing is pushed, so
    /// the last depth would sit on the bar reading as a device still holding samples it has long
    /// since played.
    ///
    /// It takes the **tick** and not the rung, which is the same choice the push site makes and
    /// for the same reason: [`Rung::Automatic`] is muted while a cassette is moving and audible
    /// the instant it stops, so a field written from the rung would say `mute` at a machine that
    /// was playing perfectly well.
    fn queue(&self, tick: Tick) -> String {
        if tick != Tick::Paced(Speed::REAL_TIME) {
            MUTED.to_owned()
        } else if self.audio_queued < 0 {
            NO_DEVICE.to_owned()
        } else {
            self.audio_queued.to_string()
        }
    }

    /// Draw the readout along the bottom of the window.
    ///
    /// # Two rows, because a dial and a sentence are different kinds of thing
    ///
    /// Five figures and a message did not fit across the window — [`OPENING_MESSAGE`] carries the
    /// measurement, and the overflow was 42 characters — and they are read differently anyway: the
    /// figures are glanced at, the sentence is read once. Separating them is also what lets the
    /// colour mean something exact, because only the row carrying the pacing figures changes with
    /// it.
    fn draw(
        &mut self,
        machine: &Spectrum,
        pacer: Pacer,
        meter: RateMeter,
        loss: LossMeter,
        ear: EarMeter,
    ) {
        // Drawn **before** the visibility check, and `F1` does not hide it. A build that
        // embeds a Sinclair ROM is a redistribution to whoever runs the binary, and Amstrad's
        // permission asks that "the program/manual" carry the acknowledgement — for a
        // double-clicked artefact the window is both, because there is no README on the path
        // between the file and its user. `web/index.html` answers the same question the same
        // way for the same reason: a permanent line of small text under the picture.
        //
        // A notice somebody has to switch on, or has to know a `--about` flag exists to find,
        // is the silent case with extra steps — which is the argument `Status::new` already
        // makes about the readout itself.
        //
        // It sits directly on top of whatever else is showing, so `F1` slides it down to the
        // bottom rather than leaving it floating above an empty strip.
        if let Some(notice) = bundle::acknowledgement() {
            draw_row(if self.visible { 2.0 } else { 0.0 }, notice, GRAY);
        }

        if !self.visible {
            return;
        }

        draw_row(1.0, &self.message, LIGHTGRAY);

        self.line.clear();
        // The multiplier is on the bar unconditionally, at `1x` as much as at `64x`. A machine
        // running sixty-four times too fast and a machine with a broken clock look identical from
        // the outside, and a field that only appears when something is unusual is a field nobody
        // learns to look at — the argument `Status::new` already makes about the readout itself.
        //
        // `Hz` beside it is the honest corroboration rather than a duplicate: it reports emulated
        // frames per **wall** second, so `speed 64x` next to `3200.0 Hz` is the machine confirming
        // it, and `speed 64x` next to `2000.0 Hz` says this host cannot sustain what it was asked
        // for. The colour, from `LossMeter`, is the same question a third way. That pair is the
        // whole of how a fast machine stays legible, and it is why [`pacing::RUNGS`]'s fixed
        // entries top out one rung below what this host saturates at rather than at the
        // saturation point: a bar that is red for the whole of every load is an alarm nobody
        // would act on.
        //
        // **`auto` is the rung that needs that pair most**, because it names no multiplier at
        // all. `speed auto (loading)` beside `4800.0 Hz` is the only way a person can see that
        // it is working; `speed auto` beside `50.0 Hz` is it correctly doing nothing. Without
        // the suffix the two would be one string, and a rung that looks identical whether or not
        // it is doing its job is a rung nobody can trust.
        //
        // Infallible: writing to a `String` cannot fail. The `Result` is there for writers
        // that can, and is discarded here rather than handled.
        //
        // **`(loading)` is drawn from the same meter the frame loop paced by**, which is what
        // makes the word true. It used to come from the drive, so it appeared over a turning
        // cassette nobody was reading — the same lie `drive::Drive` was written to take out of
        // the row above, one field along. The meter is passed in rather than re-read from the
        // machine so that the label describes the tick that just ran rather than a fresh sample
        // taken after it.
        let rung = pacing::RUNGS[self.speed];
        let decoding = ear.decoding();
        let _ = write!(
            self.line,
            "{:.1} Hz   speed {rung}{}   dropped {}   frame {}   snd {}   arrows {}",
            meter.hz(),
            rung.note(decoding),
            pacer.dropped(),
            machine.frames(),
            self.queue(rung.this_tick(decoding)),
            keymap::ARROW_SCHEMES[self.arrows].name,
        );

        // Red while frames are being lost **now**: the count alone is easy to read past, and the
        // difference between "keeping up" and "not" is the one thing this bar is for.
        //
        // The count stays lifetime and the colour does not, which is the whole of the fix. This
        // read `pacer.dropped() == 0`, and that total never falls: one lost frame — and start-up
        // very nearly guarantees one — held the bar red for the rest of a session that then ran
        // perfectly. A running total answers *"has anything ever gone wrong"*; a colour is read as
        // *"is something wrong now"*; [`LossMeter`] is the second question, and both numbers stay
        // true. See [`frontend::pacing::LossMeter`].
        draw_row(0.0, &self.line, ink(loss));
    }
}

/// The colour the pacing row is drawn in.
///
/// # A function rather than an expression, because a mutation survived
///
/// This was `let ink = if ... { LIGHTGRAY } else { RED };` inside [`Status::draw`], and putting
/// the old cumulative test back — `pacer.dropped() == 0` — left **every** test in
/// `tests/pacing_accounting.rs` green. Measured 2026-09-01, in an isolated clone: 123 passed, 0
/// failed, with the defect fully restored.
///
/// Those tests grade [`LossMeter::keeping_up`] and they cannot grade the *wiring*, because
/// `draw` needs a GPU and never runs under `cargo test`. So the one line that chooses between the
/// two sources moved out of the function no test can reach and into one that `mod tests` calls
/// directly — the same trade `pacing` makes by putting `keeping_up` in the library, applied one
/// layer up. [`Color`] is a plain struct of four floats and needs no context to compare.
fn ink(loss: LossMeter) -> Color {
    if loss.keeping_up() { LIGHTGRAY } else { RED }
}

/// Draw one line of status text, `row` rows up from the bottom of the window.
///
/// The dark rectangle is what keeps the text legible over a bright border, and it is drawn per row
/// rather than once behind the stack so that a row nobody asked for leaves no strip behind.
fn draw_row(row: f32, text: &str, ink: Color) {
    let top = screen_height() - STATUS_HEIGHT * (row + 1.0);
    draw_rectangle(
        0.0,
        top,
        screen_width(),
        STATUS_HEIGHT,
        Color::new(0.0, 0.0, 0.0, 0.65),
    );
    draw_text(text, STATUS_MARGIN, top + STATUS_TEXT, STATUS_TEXT, ink);
}

/// A Spectrum colour as a macroquad one.
///
/// Goes through [`spectrum::Colour::rgb`] like everything else, so the gun order is applied
/// in exactly one place in this crate. See [`frontend::palette`].
fn to_color(colour: spectrum::Colour) -> Color {
    let [red, green, blue] = colour.rgb();
    Color::from_rgba(red, green, blue, palette::OPAQUE)
}

/// Does the writing on the screen fit on the screen?
///
/// # The gap this closes, and the one it does not
///
/// `tests/on_screen_strings.rs` grades every status string this crate can hand a test for
/// *drawable characters*, and its own header records what it cannot reach: *"It cannot cover
/// `main.rs`'s own literals — `OPENING_MESSAGE` and the two lines `complain` draws — because they
/// are private to a binary that needs a window ... it is a person's to run."* Nobody ran it, and
/// [`OPENING_MESSAGE`] was 42 characters over the edge.
///
/// A binary target's unit tests are compiled and run by `cargo test` like any other, and a
/// `#[cfg(test)] mod tests` inside the binary can see its private constants. So the check is a
/// machine's now, and it needed no window, no new public item, and no library module.
///
/// It measures **width**, which is the property that failed, and it cannot see **height**,
/// overlap, or whether the result looks right — the same line `crate::pacing` draws between the
/// arithmetic and the observation, and it is not softened here either.
#[cfg(test)]
mod tests {
    use super::*;

    /// Logical pixels one character of the status font occupies.
    ///
    /// macroquad's default font is `ProggyClean.ttf`, and every glyph in it advances 896/2048 em —
    /// it is monospaced, which is the only reason a character count is a width at all.
    /// `draw_text` rasterises at `ceil(font_size * dpi_scale)` and divides the advance back down
    /// by that same scale, and `screen_width` is likewise logical, so the figure is identical on
    /// a retina display and a plain one: 896/2048 × 16 = **7.0** logical pixels. The font ships
    /// no `kern` table and no GPOS, so there is no pair adjustment to add.
    ///
    /// Derived from the font's metrics rather than read from `measure_text`, which needs a GPU
    /// context these tests do not have. That is this gate's honest limit and it is stated rather
    /// than assumed away: it grades the arithmetic of the layout, not the picture.
    const CHARACTER_WIDTH: f32 = 896.0 / 2048.0 * STATUS_TEXT;

    /// The longest thing [`Status::draw`] can put in the `snd` field.
    const WIDEST_QUEUE: i32 = i32::MAX;

    /// A rate wider than the meter can ever report, so the bound below holds for any of them.
    const WIDEST_RATE: f64 = 99_999.9;

    /// Characters that fit across the window [`window_conf`] opens.
    ///
    /// The narrowest case and the only one worth gating: the window is resizable, so every size a
    /// person can drag it to is one this already fits.
    fn columns() -> usize {
        let window = (FRAME_WIDTH as i32 * INITIAL_SCALE) as f32;
        ((window - STATUS_MARGIN) / CHARACTER_WIDTH) as usize
    }

    fn assert_fits(what: &str, text: &str) {
        let width = text.chars().count();
        let room = columns();
        assert!(
            width <= room,
            "{what} is {width} characters against a window holding {room}, so the last {} would \
             be drawn off the right-hand edge where nobody can read them:\n{text}",
            width.saturating_sub(room),
        );
    }

    #[test]
    fn the_opening_message_fits_the_row_it_is_drawn_on() {
        // The defect this file was sent to find. Before the split it shared a row with the
        // readout, and the two together came to 178 characters against a window holding 136.
        assert_fits("OPENING_MESSAGE", OPENING_MESSAGE);
    }

    #[test]
    fn the_opening_message_names_every_format_a_drop_can_load() {
        // The other half of the same string, and the half nothing was watching. Width has been
        // gated here since the message got its own row; **contents were not**, and the line spent
        // a fortnight offering three formats to a machine that accepts four — on the status bar,
        // from the first frame, where it is the most-read sentence this project has.
        //
        // Read out of `media::EXTENSIONS` rather than listed again, because a second list is
        // precisely what went wrong. `.rom` is skipped on purpose: `media::insert` turns one away,
        // so naming it here would advertise a drop that cannot work.
        let mut named = 0;
        for &(extension, kind) in media::EXTENSIONS {
            if kind == media::Kind::Rom {
                continue;
            }
            assert!(
                OPENING_MESSAGE.contains(&format!(".{extension}")),
                "OPENING_MESSAGE offers no .{extension}, so the first thing anybody reads \
                 describes a smaller emulator than the one they are running:\n{OPENING_MESSAGE}",
            );
            named += 1;
        }
        // Vacuously true is this file's recurring failure mode — an `EXTENSIONS` that yielded
        // nothing would satisfy the loop by never entering it. Four is what the table holds
        // today; the floor only has to be enough to prove the loop ran.
        assert!(
            named >= 4,
            "only {named} droppable formats were checked — the table has shrunk",
        );
    }

    /// A ROM of nothing but `NOP`, so the clock advances and the cassette with it.
    const NOTHING: [u8; 16 * 1024] = [0x00; 16 * 1024];

    /// A machine whose 60-T-state cassette has already been played off the end.
    fn a_machine_with_a_spent_cassette() -> Spectrum {
        let mut machine = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
        machine.insert_tape(spectrum::Tape::new(vec![10, 20, 30]));
        machine.tape_mut().play();
        machine.run_frame();
        assert!(
            !machine.tape().is_playing(),
            "the fixture is wrong: 60 T-states must not survive a 69,888 T-state frame",
        );
        machine
    }

    #[test]
    fn pressing_play_on_a_spent_cassette_does_not_claim_to_be_playing() {
        // **The wiring, not the decision.** `tests/tape_reports.rs` grades `frontend::drive` and
        // would stay perfectly green with this file's `Hotkey::PlayTape` arm reverted to the
        // `status.report("tape playing".to_owned())` it used to be — which is the whole of the
        // defect, and it lived here rather than in the library. `ink` records the same lesson
        // from the same file: a mutation that survives is a gate looking at the wrong function.
        let mut machine = a_machine_with_a_spent_cassette();
        let mut status = Status::new();

        act(Hotkey::PlayTape, &mut machine, &mut status);
        assert_eq!(status.message, drive::AT_THE_END);

        // And the empty drive, which is the other press `tape playing` used to answer.
        let mut empty = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
        act(Hotkey::PlayTape, &mut empty, &mut status);
        assert_eq!(status.message, drive::NO_TAPE);
    }

    #[test]
    fn a_tape_running_out_is_reported_by_the_loop() {
        // The other half of the wiring, and the half the frame loop cannot be asked about at all:
        // it needs a window and never returns. So the call it makes is a named function, and this
        // drives that function exactly as the loop does — press PLAY, run the frames, ask.
        let mut machine = Spectrum::new(&NOTHING).expect("16 KB is a 48K ROM");
        machine.insert_tape(spectrum::Tape::new(vec![10, 20, 30]));
        let mut status = Status::new();

        act(Hotkey::PlayTape, &mut machine, &mut status);
        assert_eq!(status.message, drive::PLAYING);
        report_a_finished_tape(&machine, &mut status);
        assert_eq!(status.message, drive::PLAYING, "nothing has ended yet");

        machine.run_frame();
        report_a_finished_tape(&machine, &mut status);
        assert_eq!(status.message, drive::RAN_OUT);
    }

    #[test]
    fn every_thing_the_drive_can_say_fits_the_row_it_is_drawn_on() {
        // These share row 1 with `OPENING_MESSAGE`, and they are the messages a person reads at
        // the moment they are most confused — a press that did nothing, or a cassette that has
        // just ended. A sentence explaining the recovery is worth nothing with the recovery off
        // the right-hand edge, which is exactly what happened to `OPENING_MESSAGE` for a
        // fortnight while width was gated in one place and not the other.
        //
        // Listed rather than looped over a table, because there is no table: each is a `pub const`
        // in `frontend::drive` and a new one that nobody added here would be a new way to overrun
        // the row. That is the same trade `spectrum::tape`'s own `SOURCES` makes, and for the same
        // reason — a walk that silently stopped visiting a file reads as a file with nothing to
        // find.
        for (name, text) in [
            ("drive::PLAYING", drive::PLAYING),
            ("drive::NO_TAPE", drive::NO_TAPE),
            ("drive::AT_THE_END", drive::AT_THE_END),
            ("drive::RAN_OUT", drive::RAN_OUT),
            ("drive::STOPPED", drive::STOPPED),
            ("drive::REWOUND", drive::REWOUND),
        ] {
            assert_fits(name, text);
        }
    }

    #[test]
    fn the_readout_fits_at_every_value_it_can_hold() {
        // A bound rather than a guess: `u64::MAX` frames and losses, `i32::MAX` queued samples,
        // a rate no meter can reach, and whichever arrow scheme has the longest name. Nothing a
        // running emulator can produce is wider than this, so a pass here is a pass for good
        // rather than a pass for a plausible afternoon.
        //
        // The speed field is bounded by `pacing::RUNGS` rather than by `u32::MAX`, and that is
        // the honest bound rather than a convenient one: `Status::speed` is an index into that
        // table and `Hotkey::CycleSpeed` is modulo its length, so no other rung can reach this
        // line. A table that grew a five-digit entry would redden this test, which is the point.
        //
        // **Both states of every rung**, because the widest thing the field can hold is no longer
        // a number: `auto (loading)` is eleven characters wider than `64x`, and it is drawn on the
        // same row as everything else here. Taken through `Rung::note` rather than by pasting the
        // suffix in, so a rung that grew a longer one is measured rather than assumed away.
        let widest = keymap::ARROW_SCHEMES
            .iter()
            .map(|scheme| scheme.name)
            .max_by_key(|name| name.chars().count())
            .expect("ARROW_SCHEMES is never empty");
        let speed = pacing::RUNGS
            .iter()
            .flat_map(|rung| [false, true].map(|playing| format!("{rung}{}", rung.note(playing))))
            .max_by_key(|field| field.chars().count())
            .expect("RUNGS is never empty");
        let line = format!(
            "{WIDEST_RATE:.1} Hz   speed {speed}   dropped {}   frame {}   \
             snd {WIDEST_QUEUE}   arrows {widest}",
            u64::MAX,
            u64::MAX,
        );
        assert_fits("the readout at its widest", &line);
    }

    #[test]
    fn the_sound_field_tells_a_muted_machine_from_one_with_no_device() {
        // Both are silence and they are different silences, so the two strings must differ and
        // both must be drawable — this file's whole `snd` field exists because a person cannot
        // otherwise tell a browser tab before its first click from a broken mixer, and
        // fast-forward adds a third case to the same confusion.
        let real_time = Tick::Paced(Speed::REAL_TIME);
        let mut status = Status::new();
        assert_eq!(status.queue(real_time), NO_DEVICE, "no device yet");

        status.audio_queued = 1024;
        assert_eq!(status.queue(real_time), "1024", "a live device");

        assert_eq!(
            status.queue(Tick::FlatOut),
            MUTED,
            "fast-forward left the last queue depth on the bar, which reads as a live device",
        );
        assert_ne!(MUTED, NO_DEVICE);

        // And the case `auto` adds, which is the one a rung-shaped condition would have got
        // wrong: an automatic machine with nothing in the drive is a machine at real time, and
        // it must sound like one. This is the assertion that reddens if `queue` is ever rewritten
        // to ask which rung is selected instead of what the tick resolved to.
        assert_eq!(
            status.queue(Rung::Automatic.this_tick(false)),
            "1024",
            "an automatic machine with the drive stopped was silenced for no reason",
        );
        assert_eq!(
            status.queue(Rung::Automatic.this_tick(true)),
            MUTED,
            "an automatic machine running flat out was reported as feeding a device",
        );
    }

    #[test]
    fn every_message_the_speed_key_can_report_fits_and_is_drawable() {
        // The payoff of `speed_message` being a function rather than a `match` inside `act`.
        // `tests/on_screen_strings.rs` grades every status string this crate can hand a test, and
        // records in its own header that it *"cannot cover `main.rs`'s own literals … because they
        // are private to a binary that needs a window"*. These three sentences were inside that
        // gap; now they are values, and they are graded on both axes the bar cares about — how
        // wide they are, and whether the font has a glyph for every character in them.
        let mut checked = 0;
        for &rung in pacing::RUNGS {
            let message = speed_message(rung);
            assert_fits("a speed message", &message);
            for character in message.chars() {
                assert!(
                    character.is_ascii() && !character.is_ascii_control(),
                    "{message:?} holds {character:?}, which the status bar draws as an empty box",
                );
            }
            checked += 1;
        }
        // Vacuously true is this file's recurring failure mode: an empty `RUNGS` would satisfy the
        // loop by never entering it.
        assert!(
            checked >= 2,
            "only {checked} rungs were checked — the table has shrunk"
        );

        // And that the rung which decides says something different from the rung it is currently
        // indistinguishable from. Somebody pressing F8 from 64x to auto has to be able to tell
        // that anything happened, and at that instant the drive is very likely stopped.
        assert_ne!(
            speed_message(Rung::Automatic),
            speed_message(Rung::Fixed(Speed::REAL_TIME)),
            "auto and 1x report the same sentence, so the press that reached auto reads as a no-op",
        );
    }

    #[test]
    fn the_failure_screens_advice_fits() {
        // `complain`'s other line is a path and an error and has no bound, so it is not gated
        // here — that is reported rather than papered over. This one is fixed text and can be.
        assert_fits("COMPLAIN_ADVICE", COMPLAIN_ADVICE);
    }

    #[test]
    fn the_check_is_capable_of_failing() {
        // A positive control, because `assert_fits` is otherwise a function that has only ever
        // been shown to say yes — and it passes vacuously on an empty string. `on_screen_strings`
        // carries the same control for the same reason.
        let result = std::panic::catch_unwind(|| {
            assert_fits("a deliberate overflow", &"x".repeat(columns() + 1));
        });
        assert!(
            result.is_err(),
            "assert_fits accepted a line one character wider than the window",
        );
    }

    #[test]
    fn the_readout_takes_its_colour_from_recent_losses_not_the_lifetime_total() {
        // The wiring, which `tests/pacing_accounting.rs` cannot see. It grades `keeping_up`
        // thoroughly and in both directions; what it cannot grade is that `draw` asks *that*
        // rather than asking `Pacer::dropped` the way it used to. Restoring the old line left all
        // 123 of its tests green, so this is the assertion that was missing rather than a second
        // copy of one that was already there.
        let mut loss = LossMeter::new(LOSS_WINDOW, 0.0);

        loss.sample(0.1, 21);
        assert_eq!(ink(loss), RED, "twenty-one frames lost and the bar is grey");

        // The lifetime total is still 21 here and always will be. Only the colour lets go.
        loss.sample(1.0, 21);
        loss.sample(2.0, 21);
        assert_eq!(
            ink(loss),
            LIGHTGRAY,
            "two clean seconds after the last loss and the bar is still red — this is the latch",
        );

        // And the threshold's own boundary, because without it this test cannot tell
        // `keeping_up` from a plain `recent() == 0` — the two agree on 0 and on 21 and differ
        // only at 1, which is exactly where the decision about what deserves an alarm was made.
        let mut single = LossMeter::new(LOSS_WINDOW, 0.0);
        single.sample(0.1, 1);
        assert_eq!(
            ink(single),
            LIGHTGRAY,
            "one lost frame is a hiccup and must not colour the bar",
        );
    }

    #[test]
    fn a_character_is_seven_logical_pixels() {
        // The number the whole gate rests on, pinned so that a change to `STATUS_TEXT` fails
        // here — visibly, with the arithmetic in view — rather than quietly moving every budget
        // above it.
        assert!((CHARACTER_WIDTH - 7.0).abs() < f32::EPSILON);
        assert_eq!(columns(), 136);
    }
}
