//! The browser page's half of the frontend's host seam.
//!
//! # Two functions, and the only `unsafe` in this workspace
//!
//! `crates/frontend/src/host.rs` opens *"This module is the entire `wasm32` seam"* and counts
//! the non-portable calls: **where the arguments come from**, and **how bytes get out**. This
//! crate is the browser's answer to both, and it exists because those answers cannot be
//! written where the questions are.
//!
//! Every route for getting bytes across the JavaScript boundary needs `unsafe` — including the
//! one that looks like it does not, because in edition 2024 an export is
//! `#[unsafe(no_mangle)] extern "C"`, so a *"JS reaches in and takes the bytes"* design moves
//! the `unsafe` rather than removing it. And `crates/frontend/Cargo.toml` carries
//! `unsafe_code = "forbid"`, which is **not** `deny`: an `#[allow(unsafe_code)]` inside a
//! `forbid`den crate is a hard error, not an override.
//!
//! `docs/M8.md` Decision 4 weighed relaxing that lint against moving the FFI, and took the
//! second. The reasoning is recorded there and is not repeated here beyond the one sentence
//! that decides it: the trade is **not** *"safety versus a download button"*, it is **where the
//! exception is legible**. A crate whose entire contents are the few `unsafe` lines needed to
//! talk to a page keeps `forbid` everywhere it protects something, and confines the exception
//! to a surface a person can read in one sitting.
//!
//! # What is checkable here, and what is not
//!
//! | Property | Covered by | Class |
//! |---|---|---|
//! | A shim that was never registered reports **failure**, not success | [`handoff`], against literal codes, in `tests/handoff_codes.rs` | **proven** |
//! | The `unsafe` surface has not grown | `tests/unsafe_inventory.rs`, which reads this file as **text** and so sees every block on every target regardless of `cfg` | **proven** |
//! | Every `unsafe` block carries a `// SAFETY:` comment discharging its invariant | `clippy::undocumented_unsafe_blocks`, **only when clippy is run for `wasm32`** — see below | **proven**, of the wasm build only |
//! | That the JavaScript does what its Rust declaration says | **nothing.** There is no compiler on the far side of an `extern "C"` block, and no test here can reach one. `web/gate.sh` asserts the *imports exist in the module*; whether the page's function honours the contract is settled by using it | **observed** |
//! | That a `Blob` reaches a user's disk | **nothing automatable.** It is a browser, a download directory and a person | **observed** |
//!
//! > **The clippy row is narrower than it looks, and the narrowing is the interesting part.**
//! > Both restriction lints this crate turns on fire only on code the current target compiles,
//! > and every `unsafe` block below is behind `#[cfg(target_arch = "wasm32")]`. So
//! > `cargo clippy` on a desktop host lints **none** of them, and would report clean on a file
//! > with an undocumented `unsafe` block in it. That is a gate grading less than it appears to,
//! > which is the shape `docs/STATUS.md` keeps recording — caught here by asking what the
//! > instrument actually looks at rather than by trusting that a deny-level lint denies.
//! >
//! > Two mitigations, and they fail differently on purpose: `web/gate.sh` runs
//! > `cargo clippy --target wasm32-unknown-unknown`, and `tests/unsafe_inventory.rs` reads the
//! > source as text under `cargo test` on whatever host is running it.
//!
//! # The far side
//!
//! `web/zx_page.js` provides the three imports declared below, registered through
//! `miniquad_add_plugin` before the module is instantiated. It is vendored and versioned
//! beside `web/index.html`; see `web/README.md`.

/// What became of an offer to hand bytes to the browser.
///
/// Three states rather than a `bool` or a `Result<(), E>`, because *"there is no browser"* and
/// *"there is a browser and it said no"* call for opposite responses from the caller: the first
/// should fall back to the filesystem, and the second must not, because on that target there is
/// no filesystem to fall back to. Collapsing them would make a desktop build report a download
/// failure and a browser build silently write nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Handoff {
    /// The page took the bytes and started a download.
    ///
    /// It is an *offer*: where the file lands, and whether the user keeps it, is the browser's
    /// and the user's. Nothing here can observe either.
    Started,

    /// There is a page, and it did not take them.
    ///
    /// Reached two ways that a caller cannot tell apart and does not need to: the shim ran and
    /// threw, or the shim was never registered and miniquad substituted a stub. See [`handoff`].
    Refused,

    /// There is no page. This build is not running in a browser at all.
    NoPage,
}

/// The value `zx_offer_download` returns when a download has started.
///
/// **Nothing else may mean success, and zero specifically must not**, which is the whole reason
/// this is a named constant with a paragraph attached rather than a `!= 0` at the call site.
///
/// `miniquad`'s `gl.js` carries `add_missing_functions_stabs`, which walks the module's imports
/// and replaces any the page did not provide with a stub that logs a warning and returns
/// `undefined`. Across the wasm ABI a JavaScript `undefined` arrives as **0**. So a page whose
/// download shim was never deployed calls a function that succeeds, returns 0, and logs into a
/// console nobody has open — and if 0 meant success, `F2` would report `saved snapshot-1.z80`
/// and nothing would be saved.
///
/// `docs/M8.md` Decision 4 names that as one of three non-negotiable rules for this route:
/// *"the Rust side must not report `saved …` on the strength of having made the call — the JS
/// must return a value the Rust side checks."* Making the success code **1** rather than 0 is
/// what turns the missing-shim case from a silent no-op into a reported failure.
const STARTED: i32 = 1;

/// Decode what the page's download shim returned.
///
/// Public because it is the discriminating assertion of this whole crate and a private function
/// would be ungradeable — the same reason `frontend::host::arguments_from_query` is public.
/// `tests/handoff_codes.rs` is the gate, and its load-bearing row is `0`.
///
/// Never [`Handoff::NoPage`]: reaching this function at all means a shim was called, so the
/// question *"is there a page"* has already been answered yes.
#[must_use]
pub const fn handoff(code: i32) -> Handoff {
    if code == STARTED {
        Handoff::Started
    } else {
        Handoff::Refused
    }
}

// ---------------------------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------------------------

// The imports `web/zx_page.js` registers into miniquad's `importObject.env`.
//
// `#[link(wasm_import_module = "env")]` is **required and is not decoration**, and that was
// established by the linker rather than reasoned about. Without it, `rust-lld` does not treat
// these as WebAssembly imports at all and the build fails:
//
//     rust-lld: error: …rcgu.o: undefined symbol: zx_query_length
//     rust-lld: error: …rcgu.o: undefined symbol: zx_query_copy
//     rust-lld: error: …rcgu.o: undefined symbol: zx_offer_download
//     error: could not compile `frontend` (bin "zx")
//
// Read on 2026-09-01, on the first `cargo build --release --target wasm32-unknown-unknown`.
// `miniquad-0.4.11` carries the same attribute over both of its own extern blocks
// (`src/native/wasm/fs.rs:1`, `src/native/wasm/webgl.rs:316`), which is where the answer was —
// in the pinned dependency, again, rather than in anything about the platform. **The failure
// is the loud kind**: a missing import module is a link error, not a module that instantiates
// against stubs, so this is one hazard on this route that cannot be silent.
//
// `web/gate.sh` asserts all three are present in the built module *under the module name
// `env`*, which is what makes the page's `importObject.env.zx_*` assignments meet them.
//
// `usize` rather than `u32` because it is what the Rust side already has; on `wasm32` a
// `usize` **is** 32 bits, so each of these lowers to the ABI's `i32` and arrives in
// JavaScript as a number.
//
// Written with `//` and not `///`: rustdoc generates nothing for an extern block, so a doc
// comment here is `unused_doc_comments` — which `cargo clippy` on a desktop host cannot see,
// because this whole block is behind the `cfg` below. It was caught by
// `cargo clippy --target wasm32-unknown-unknown`, on its first run, which is the argument for
// that line being in `web/gate.sh` rather than in somebody's memory.
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// How many bytes `window.location.search` occupies as UTF-8, leading `?` included.
    fn zx_query_length() -> usize;

    /// Copy at most `capacity` of those bytes to `destination`; returns how many were written.
    fn zx_query_copy(destination: *mut u8, capacity: usize) -> usize;

    /// Offer `bytes` to the browser as a download suggested-named `name`.
    ///
    /// Returns [`STARTED`], or anything else for a refusal.
    fn zx_offer_download(
        name: *const u8,
        name_length: usize,
        bytes: *const u8,
        bytes_length: usize,
    ) -> i32;
}

/// The page's query string, `?` included, or empty if there is none.
///
/// The value is handed to [`frontend::host::arguments_from_query`](../frontend/host/fn.arguments_from_query.html)
/// **undecoded**, and that is deliberate: a query value here names a file the page will fetch
/// over HTTP, so it is going straight back into a URL. Percent-decoding it would turn
/// `roms/a%20b.rom` into a literal space in an `XMLHttpRequest` path and break exactly the case
/// decoding exists to fix.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn query_string() -> String {
    // SAFETY: takes no arguments and touches no memory this program owns; the only thing
    // crossing the boundary is a return value. If the page never registered the shim,
    // miniquad's `add_missing_functions_stabs` has replaced it with a stub returning
    // `undefined`, which arrives as 0 — an empty query, which is what a URL with no parameters
    // produces and is handled identically one line below.
    let length = unsafe { zx_query_length() };
    if length == 0 {
        return String::new();
    }

    // `vec![0; n]` and not `Vec::with_capacity` + `set_len`: every byte is initialised before
    // the pointer leaves Rust, so there is no uninitialised memory for the callee to be
    // trusted about and no second `unsafe` to write the length back.
    //
    // Uncapped, because the bound belongs to the browser: this length is the size of
    // `location.search`, which the browser already limits. A cap here would be a second bound
    // with no owner, and its failure mode is a silently truncated path.
    let mut buffer = vec![0_u8; length];

    // SAFETY: `buffer` is a live, uniquely-owned allocation of exactly `length` initialised
    // bytes, so the pointer is valid for writes of `length` bytes; nothing in this function
    // drops, moves or reallocates it before the call returns. The callee is
    // `web/zx_page.js`'s `zx_query_copy`, which constructs one `Uint8Array` view over exactly
    // the range `[destination, destination + capacity)` and writes only inside it. That view
    // is created and dropped inside the call, and the shim never calls back into Rust, so
    // there is no point at which wasm memory could grow and leave the view — or this
    // pointer — aliasing a stale buffer.
    let written = unsafe { zx_query_copy(buffer.as_mut_ptr(), length) };

    // `min` and not a bare `truncate`: the returned count comes from the far side of an FFI
    // boundary and is not this program's to trust, and `truncate` past the length would be a
    // no-op that silently kept bytes the shim did not write.
    buffer.truncate(written.min(length));

    // `zx_page.js` encodes with `TextEncoder`, whose output is valid UTF-8 by construction, so
    // the lossy path is unreachable in practice. It is `lossy` rather than a `Result` because
    // a caller has nothing to do about a query string it cannot decode that it would not also
    // do about one that is empty, and a failure path with no handler is not error handling.
    String::from_utf8_lossy(&buffer).into_owned()
}

/// Offer `bytes` to the browser as a download, suggesting the filename `name`.
///
/// `name` is a **suggestion**, not a path. A browser has no addressable download directory, so
/// a collision is resolved by the browser appending its own counter — which is why
/// `frontend::host::free_path`'s probing loop degrades harmlessly on this target rather than
/// having to be switched off.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn offer_download(name: &str, bytes: &[u8]) -> Handoff {
    // SAFETY: both pointers come from borrows this function holds for the whole call, so each
    // is valid for reads of exactly the length passed beside it and neither can be freed or
    // moved while the callee runs. `web/zx_page.js`'s `zx_offer_download` builds one
    // `Uint8Array` view per pointer, **copies** the bytes out with `.slice()` before handing
    // them to `Blob`, and retains neither view nor pointer past the call — the copy is what
    // makes the `Blob`'s lifetime, which outlives this call, independent of wasm memory that
    // may later grow and detach. The shim never calls back into Rust, so no reallocation can
    // occur between the views' construction and their last read. A zero length yields a
    // dangling-but-aligned pointer from the empty slice, which JavaScript uses only to
    // construct a zero-length view and never dereferences.
    let code = unsafe { zx_offer_download(name.as_ptr(), name.len(), bytes.as_ptr(), bytes.len()) };
    handoff(code)
}

/// The version `web/zx_page.js` must declare for its plugin.
///
/// Bumped whenever the contract between this file and that one changes — a renamed import, a
/// changed signature, a changed return convention.
#[cfg(target_arch = "wasm32")]
const PLUGIN_VERSION: u32 = 1;

/// The plugin version check miniquad performs on `zx_page.js`.
///
/// `gl.js`'s `init_plugins` calls `wasm_exports[name + "_crate_version"]()` for every
/// registered plugin and `console.error`s on a mismatch. That is a weak check — it is a console
/// message and it does not stop the page — but it is strictly better than the alternative,
/// which is a page and a module that disagree about an ABI with **nothing** noticing.
/// `docs/M8.md` Decision 6 is explicit that the runtime check is not the gate; the gate is the
/// hash in `web/README.md` and the import assertion in `web/gate.sh`.
///
/// # Safety
///
/// `#[unsafe(no_mangle)]` is an assertion that this symbol name is unique in the linked
/// artefact. It is discharged by the name: `zx_page_crate_version` is the crate's own name
/// followed by miniquad's required suffix, no other crate in this workspace defines a
/// `no_mangle` symbol at all, and `web/gate.sh` asserts the built module exports it — so a
/// collision that silently shadowed it would turn that assertion red rather than pass quietly.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn zx_page_crate_version() -> u32 {
    PLUGIN_VERSION
}

// ---------------------------------------------------------------------------------------
// Everywhere else
// ---------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------
// The audio device
// ---------------------------------------------------------------------------------------

/// How much audio the device is allowed to hold before this stops handing it more.
///
/// # The number is a latency, and it is a trade with a visible other side
///
/// Sound arrives at the device ahead of when it is heard, so this buffer **is** the delay
/// between a key press and its click. Too small and every hiccup is a gap; too large and the
/// machine feels detached from the keyboard.
///
/// 100 ms is five frames of emulated time, which is chosen against a number this project
/// already knows: [`crate::Handoff`]'s sibling `Pacer` runs at most `MAX_CATCH_UP` **four**
/// frames before declaring the rest lost. So the buffer absorbs a stall one frame longer than
/// the pacer will ever try to catch up on — a single dropped frame is inaudible, and a run of
/// them is both audible *and* already counted and shown in red on the status bar.
///
/// **Audio is what makes a dropped frame audible, and that is not papered over.** A resampler
/// that stretched to hide an underrun would turn a counted, visible failure into an
/// inaudible-but-wrong one, which is the trade this project refuses everywhere else.
pub const BUFFER_MILLISECONDS: u32 = 100;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// The sample rate of the page's `AudioContext`, or 0 if there is none yet.
    fn zx_audio_rate() -> u32;

    /// Hand `count` mono `f32` samples at `samples` to the page. Returns samples still queued,
    /// or `-1` if there is no device.
    fn zx_audio_push(samples: *const f32, count: usize) -> i32;
}

/// The sample rate the host's audio device runs at, or **0** if there is no device.
///
/// Zero is a real answer and not an error: a browser tab that has not been clicked yet has a
/// suspended `AudioContext`, a machine may have no sound card, and a build may have had its
/// audio dependency turned off. The caller's response to all three is the same — do not
/// generate audio — so they are one value rather than an error type nobody would match on.
///
/// **Poll it.** In a browser the rate is available immediately but the context is suspended
/// until a user gesture, and on a desktop the device is opened on the first call. A frame loop
/// asking every frame until it gets a non-zero answer costs one function call.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn audio_rate() -> u32 {
    // SAFETY: takes no arguments and touches no memory this program owns; only a return value
    // crosses. If the page never registered the shim, miniquad's
    // `add_missing_functions_stabs` substituted one returning `undefined`, which arrives as 0 —
    // "there is no device", which is exactly the right answer for a page without the shim.
    unsafe { zx_audio_rate() }
}

/// Hand mixed mono samples to the device; returns how many are queued ahead of them.
///
/// A negative return means there is no device. A caller that keeps generating audio into one
/// is not doing anything harmful — the samples are dropped — but it is doing work for nothing,
/// which is what [`audio_rate`] is for.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn audio_push(samples: &[f32]) -> i32 {
    // SAFETY: `samples` is borrowed for the whole call, so the pointer is valid for reads of
    // exactly `samples.len()` `f32`s and cannot be freed or moved while the callee runs.
    // `web/zx_page.js` builds one `Float32Array` view over that range, **copies** it with
    // `.slice()` before handing it to `postMessage`, and retains neither the view nor the
    // pointer — the copy is what makes the transferred buffer independent of wasm memory, which
    // may grow and detach afterwards. The shim never calls back into Rust, so no reallocation
    // can happen between the view's construction and its last read.
    unsafe { zx_audio_push(samples.as_ptr(), samples.len()) }
}

/// The desktop device, and the ring the audio callback drains.
#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    use super::BUFFER_MILLISECONDS;

    /// Channels the device is opened with.
    ///
    /// Two, and the mono mix is written to both. A Spectrum is mono and asking for one channel
    /// is the honest description — but a mono device is the configuration least likely to exist
    /// on an arbitrary machine, and a silent emulator because the device refused a channel
    /// count is a worse outcome than one duplicated `f32`.
    ///
    /// Lives in this module rather than beside `BUFFER_MILLISECONDS` because a browser's
    /// `AudioContext` is configured in JavaScript and never sees it: a constant compiled for a
    /// target that cannot use it is dead code, and `wasm32` said so.
    const CHANNELS: usize = 2;

    /// The rate the device is asked for.
    ///
    /// A request, not a measurement: `tinyaudio` takes a rate and configures the device, and
    /// what a given sound card actually does with 48,000 is between it and the operating
    /// system. 48 kHz is what macOS, Windows and modern Linux desktops run natively, so asking
    /// for it usually means no resampling happens below this layer either.
    const REQUESTED_HZ: u32 = 48_000;

    /// Samples per channel the device asks for at a time.
    ///
    /// 512 at 48 kHz is about 10 ms — small enough that the callback is responsive, large
    /// enough that it is not called absurdly often.
    const BLOCK: usize = 512;

    /// Everything generated and not yet played.
    static RING: OnceLock<Mutex<VecDeque<f32>>> = OnceLock::new();

    /// Whether a device has been opened, and at what rate. Zero means "not yet, or refused".
    static RATE: OnceLock<u32> = OnceLock::new();

    fn ring() -> &'static Mutex<VecDeque<f32>> {
        RING.get_or_init(|| {
            Mutex::new(VecDeque::with_capacity(
                (REQUESTED_HZ * BUFFER_MILLISECONDS / 1000) as usize,
            ))
        })
    }

    /// Open the device once, and report its rate.
    pub fn rate() -> u32 {
        *RATE.get_or_init(|| {
            let parameters = tinyaudio::OutputDeviceParameters {
                channels_count: CHANNELS,
                sample_rate: REQUESTED_HZ as usize,
                channel_sample_count: BLOCK,
            };
            let result = tinyaudio::run_output_device(parameters, move |output| {
                fill(output);
            });
            match result {
                Ok(device) => {
                    // **Deliberately leaked.** `OutputDevice` stops the stream when it drops,
                    // and this function has nowhere to keep it: it is reached from a free
                    // function with no owner, called once per process, for a device that
                    // should live exactly as long as the process. A `static` holding it would
                    // need it to be `Sync`, which it is not. Leaking one device once is the
                    // smallest honest answer; it is named here rather than hidden behind a
                    // handle nobody keeps.
                    std::mem::forget(device);
                    REQUESTED_HZ
                }
                Err(error) => {
                    // Reported, not swallowed. A machine with no sound card, or a container
                    // with no audio server, is an ordinary situation — but *"the emulator is
                    // silent"* with no explanation anywhere is the failure this project keeps
                    // recording, and the caller only receives a 0.
                    eprintln!("zx: no audio device ({error}); the emulator will be silent");
                    0
                }
            }
        })
    }

    /// The audio callback. Runs on the device's own thread.
    fn fill(output: &mut [f32]) {
        let Ok(mut queued) = ring().lock() else {
            // A poisoned lock means a panic happened while holding it. Silence is the only
            // thing this callback can do about that, and it must not panic in turn: unwinding
            // out of an audio callback crosses an FFI boundary.
            output.fill(0.0);
            return;
        };
        for frame in output.chunks_mut(CHANNELS) {
            // Underrun is silence, not the previous sample repeated. A repeat is a buzz that
            // sounds like a defect in the emulator; silence sounds like a gap, which is what it
            // is, and `Pacer::dropped` is already counting the cause on screen.
            let sample = queued.pop_front().unwrap_or(0.0);
            frame.fill(sample);
        }
    }

    /// Queue `samples`, and report how many are waiting.
    pub fn push(samples: &[f32]) -> i32 {
        if rate() == 0 {
            return -1;
        }
        let Ok(mut queued) = ring().lock() else {
            return -1;
        };
        let ceiling = (REQUESTED_HZ * BUFFER_MILLISECONDS / 1000) as usize;
        for &sample in samples {
            if queued.len() >= ceiling {
                // Dropped rather than grown. An unbounded queue does not fix an emulator
                // running fast; it converts a small permanent error into a latency that climbs
                // for as long as the session lasts, which is the self-amplifying shape
                // `crate::pacing`'s own documentation refuses for frames.
                break;
            }
            queued.push_back(sample);
        }
        i32::try_from(queued.len()).unwrap_or(i32::MAX)
    }
}

/// The sample rate the host's audio device runs at, or **0** if there is no device.
///
/// See the `wasm32` twin for what a zero means and why it is not an error.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn audio_rate() -> u32 {
    desktop::rate()
}

/// Hand mixed mono samples to the device; returns how many are queued ahead of them.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn audio_push(samples: &[f32]) -> i32 {
    desktop::push(samples)
}

/// Empty: a command line has no query string.
///
/// This is what keeps `#[cfg]` out of `crates/frontend` entirely. `host::arguments` appends
/// this to `std::env::args()` unconditionally, and on each target exactly one of the two is
/// ever non-empty — so there is no precedence rule between them to get wrong, and no branch
/// that one target compiles and never runs.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn query_string() -> String {
    String::new()
}

/// [`Handoff::NoPage`]: there is no browser here, so the caller should use the filesystem.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn offer_download(_name: &str, _bytes: &[u8]) -> Handoff {
    Handoff::NoPage
}
