//! The browser page's half of the frontend's host seam.
//!
//! # The whole browser FFI, and the only `unsafe` in this workspace
//!
//! `crates/frontend/src/host.rs` opens *"This module is the entire `wasm32` seam"* and counts
//! the non-portable calls: **where the arguments come from**, and **how bytes get out**. This
//! crate is the browser's answer to both, and it exists because those answers cannot be
//! written where the questions are.
//!
//! *This heading said **"Two functions"**, and of its two halves exactly one is still true.* The
//! count was right when the seam was [`query_string`] and [`offer_download`] alone; the crate has
//! since grown [`handoff`], the `wasm32`-only `zx_page_crate_version` export and the audio pair
//! [`audio_rate`] / [`audio_push`], so the figure is stale. It has been dropped rather than
//! re-pinned, because a number in a heading is a copy that nothing checks — which is the whole
//! reason `tests/unsafe_inventory.rs` holds this crate's other sizes in named constants.
//!
//! **The `unsafe` half was re-checked rather than assumed, and it survives.** Every other
//! member of this workspace declares `unsafe_code = "forbid"` in its own manifest, and
//! `crates/page/Cargo.toml` is the only one that deliberately does not. That is stated as a
//! complement rather than as a roster of crate names, for the reason the heading's count was
//! dropped rather than re-pinned: it would be a second copy of a fact the clause beside it
//! already carries whole, and the copy is the half that goes stale when a member is added.
//! `rg -l '^unsafe_code = "forbid"' crates/*/Cargo.toml` returns every member but this one;
//! the `^` is load-bearing, for the reason `crates/page/Cargo.toml`'s own `[lints.rust]`
//! comment records. Sweeping every `.rs` file outside this crate for
//! `unsafe` **syntax** — `unsafe {`, `unsafe extern`, `unsafe fn`, `unsafe impl`, `unsafe trait`,
//! `#[unsafe(` — returns exactly one hit, in `crates/frontend/src/host.rs`, and it is inside a
//! `//!` line quoting the `#[unsafe(no_mangle)]` argument made below: a sentence about `unsafe`,
//! not an instance of it. *The word itself appears in several files' prose, which is why the
//! sweep is over syntax and is written down here as the syntax it matched.*
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
//! | Every `unsafe` block carries a `// SAFETY:` comment discharging its invariant | `tests/unsafe_inventory.rs::every_unsafe_block_is_preceded_by_a_safety_comment`, on **every** target under an ordinary `cargo test`, **and** `clippy::undocumented_unsafe_blocks` when clippy is run for `wasm32` | **proven** |
//! | That the JavaScript does what its Rust declaration says | **nothing.** There is no compiler on the far side of an `extern "C"` block, and no test here can reach one. `web/gate.sh` asserts the *imports exist in the module*; whether the page's function honours the contract is settled by using it | **observed** |
//! | That a `Blob` reaches a user's disk | **nothing automatable.** It is a browser, a download directory and a person | **observed** |
//!
//! > **Clippy is narrower than it looks, and the narrowing is the interesting part.**
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
//! >
//! > *The `SAFETY:` row above used to credit the lint alone, and classed itself **proven, of
//! > the wasm build only** on the strength of that.* This paragraph named both mitigations and
//! > the row named one, so the table understated its own coverage — the mirror image of the
//! > failure the table exists to catch, and worth recording rather than quietly correcting:
//! > a register that undersells a gate teaches a reader to distrust a check that is in fact
//! > running, and the next person to save effort deletes it. The text sweep is the instrument
//! > that does not depend on a target, so it is now named first and the row is **proven**
//! > outright. Clippy remains the better instrument where it applies, because it understands
//! > the syntax rather than the text — which is why it is still here and still second.
//! > `clippy::multiple_unsafe_ops_per_block` no longer stands alone either: see
//! > `tests/unsafe_inventory.rs::every_unsafe_block_contains_exactly_one_ffi_call`, which
//! > counts FFI operations per block rather than lines containing `unsafe {`, and so asserts
//! > on every target the one invariant that lint was the only instrument for.
//!
//! # The far side
//!
//! `web/zx_page.js` provides every import declared below, registered through
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
/// `docs/M8.md` Decision 4 names that among the rules it holds non-negotiable for this route:
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
// `web/gate.sh` asserts every `zx_*` import this crate declares — not only this block's — is
// present in the built module *under the module name `env`*, which is what makes the page's
// `importObject.env.zx_*` assignments meet them.
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

/// The most of `window.location.search` this will copy out of the page, in bytes.
///
/// # A cap, where the previous sentence here said a cap would be wrong
///
/// *This read **"Uncapped, because the bound belongs to the browser: this length is the size of
/// `location.search`, which the browser already limits. A cap here would be a second bound with
/// no owner, and its failure mode is a silently truncated path."*** Every clause of that is
/// true of a **cooperating** shim, and that is the assumption this whole crate is built to
/// refuse. `STARTED = 1` exists because the far side may be a stub that returns `undefined`;
/// `written.min(..)` below exists because a returned count *"is not this program's to trust"*.
/// The length that **sizes a heap allocation** was the one number still taken on faith, in the
/// same function that says out loud it trusts nothing else — an inconsistent trust model on the
/// one seam this crate exists to make auditable.
///
/// It is worse than inconsistent on this target. `zx_query_length` is declared `-> usize`, the
/// wasm ABI carries it as `i32`, and a negative JavaScript return sign-extends to a
/// near-`u32::MAX` `usize`. `vec![0_u8; that]` is then an allocation failure, and under
/// `panic = "abort"` (workspace `Cargo.toml`) an allocation failure is a **process abort** —
/// the emulator vanishes, with no message anywhere. One hostile or broken number, and the page
/// is gone.
///
/// # Why this number
///
/// 64 KiB, because a URL has a real, statable bound and every browser's is far below it:
/// Chrome stops at 32,779 characters in the address bar, Internet Explorer stopped at 2,083,
/// and Firefox and Safari are in the same tens-of-thousands range. A query string is a
/// *fraction* of a URL. So the cap cannot fire on any query a browser will hand over, and the
/// only caller that can reach it is the one this crate assumes may exist.
///
/// # What happens when it is exceeded
///
/// The query is **truncated to this many bytes** and the emulator carries on. Truncation and
/// not an abort, and not a refusal: the copy below already handles a short read, the parse in
/// `frontend::host::arguments_from_query` already handles a query it cannot make sense of, and
/// a truncated argument list produces a boot that visibly does not load what was asked for —
/// which is a fault a person can see, where an abort is a blank tab.
///
/// **It cannot be reported from here, and that is a cost rather than an oversight.** This crate
/// declares its imports and a console route is not one of them; adding one would grow the FFI
/// surface `tests/unsafe_inventory.rs` pins and `docs/M8.md` Decision 4 exists to keep small,
/// to report a case only a broken shim can produce. Stated here instead, which is where a
/// person debugging a truncated query would look.
#[cfg(target_arch = "wasm32")]
const MAX_QUERY_BYTES: usize = 64 * 1024;

/// The page's query string, `?` included, or empty if there is none.
///
/// The value is handed to [`frontend::host::arguments_from_query`](../frontend/host/fn.arguments_from_query.html)
/// **undecoded**, and that is deliberate: a query value here names a file the page will fetch
/// over HTTP, so it is going straight back into a URL. Percent-decoding it would turn
/// `roms/a%20b.rom` into a literal space in an `XMLHttpRequest` path and break exactly the case
/// decoding exists to fix.
///
/// Longer than [`MAX_QUERY_BYTES`] is truncated; see that constant for why there is a bound at
/// all and what a caller sees when it fires.
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
    // `min` is the whole of the cap, and it is one operation because it has to cover two cases
    // that look nothing alike and are the same arithmetic: a query genuinely longer than any
    // browser produces, and a negative `i32` that arrived as a near-`u32::MAX` `usize`.
    let capacity = length.min(MAX_QUERY_BYTES);
    let mut buffer = vec![0_u8; capacity];

    // SAFETY: `buffer` is a live, uniquely-owned allocation of exactly `capacity` initialised
    // bytes, so the pointer is valid for writes of `capacity` bytes; nothing in this function
    // drops, moves or reallocates it before the call returns. The callee is
    // `web/zx_page.js`'s `zx_query_copy`, which constructs one `Uint8Array` view starting at
    // `destination` and no longer than the `capacity` passed beside it — it clamps to
    // `Math.min(bytes.length, capacity)` for its own reasons, so the view can be *shorter*
    // than the buffer and can never be longer — and writes only inside it. That view is
    // created and dropped inside the call, and the shim never calls back into Rust, so there
    // is no point at which wasm memory could grow and leave the view — or this pointer —
    // aliasing a stale buffer.
    let written = unsafe { zx_query_copy(buffer.as_mut_ptr(), capacity) };

    // `min` and not a bare `truncate`: the returned count comes from the far side of an FFI
    // boundary and is not this program's to trust, and `truncate` past the length would be a
    // no-op that silently kept bytes the shim did not write.
    buffer.truncate(written.min(capacity));

    // `zx_page.js` encodes with `TextEncoder`, whose output is valid UTF-8 by construction, so
    // every byte the shim wrote is part of a well-formed sequence. **Two truncations can still
    // cut one of those sequences in half**, and neither is exotic: `Math.min(bytes.length,
    // capacity)` on the far side, which `zx_page.js:268-271` documents as its defence against
    // the URL changing between the two calls, and `MAX_QUERY_BYTES` on this side. So the lossy
    // path is *reachable* — it was described here as unreachable, which was true only of the
    // case where nothing truncates — and what it produces is `U+FFFD` at the join. That is the
    // right outcome and is why this is `lossy` rather than a `Result`: a caller has nothing to
    // do about a query string it cannot decode that it would not also do about one that is
    // empty, and a failure path with no handler is not error handling.
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
    // `Uint8Array` view per pointer and retains neither view nor pointer past the call. Each
    // view's contents leave wasm memory before anything else happens to them: `name` through
    // `DECODER.decode` (`zx_page.js:282`), which returns a fresh JavaScript string, and
    // `bytes` through `.slice()` (`:287`), which returns a fresh `ArrayBuffer` — so the
    // `Blob`, whose lifetime outlives this call, holds a copy rather than a window onto a
    // buffer that may later grow and detach.
    //
    // **What discharges the reallocation half is the ordering inside the shim, and it used to
    // be a claim about the whole page.** This comment said *"The shim never calls back into
    // Rust, so no reallocation can occur between the views' construction and their last
    // read."* That is not a property this side can hold: `anchor.click()` at `zx_page.js:304`
    // is `HTMLElement.click()`, which **synchronously** dispatches a bubbling `click` through
    // `document`, and any listener on that path may call `wasm_exports.*` and grow wasm
    // memory. No listener does today — `rg 'addEventListener\("click"|onclick' web/` returns
    // nothing, and the roster is deliberately not copied here, for the reason the module
    // header gives about counts: the sweep is the fact, a transcription of what it matched is
    // a second copy that goes stale on the next vendored bundle. But a
    // memory-safety argument that rests on a page-wide absence is falsified by adding one
    // listener anywhere, in a file that has nothing to do with this one, and a reviewer who
    // checks the stated reason would be checking the wrong thing.
    //
    // The ordering is local and is checkable in one file: both last reads of wasm memory
    // (`:282` and `:287`) complete before `:304` runs. Everything after that point touches
    // only the copies. So re-entrancy through the click is irrelevant to this block, whether
    // or not it ever happens.
    //
    // A zero length yields a dangling-but-aligned pointer from the empty slice, which
    // JavaScript uses only to construct a zero-length view and never dereferences.
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

/// The audio latency this project budgets for, in milliseconds.
///
/// # The number is a latency, and it is a trade with a visible other side
///
/// Sound arrives at the device ahead of when it is heard, so this buffer **is** the delay
/// between a key press and its click. Too small and every hiccup is a gap; too large and the
/// machine feels detached from the keyboard.
///
/// 100 ms is five frames of emulated time, chosen against the pacer's catch-up window: it
/// absorbs a stall longer than the pacer will ever try to make up before declaring the rest
/// lost, so a single dropped frame is inaudible, and a run of them is both audible *and*
/// already counted and shown in red on the status bar.
///
/// *That paragraph used to name the window as `MAX_CATCH_UP` **four** frames, and called
/// `Pacer` a sibling of [`Handoff`].* Both were wrong in the same way. `Pacer` lives in
/// `crates/frontend/src/pacing.rs`, this crate does not depend on `crates/frontend` and cannot
/// see it, and `Handoff` has no sibling of that name — so the figure was an unverified copy of
/// a number across a boundary with no gate on it, which is the defect this repository
/// catalogues most often. The value was correct when written and is not the point: `pacing.rs`
/// scales the window by the speed multiplier, so *"four frames"* was only ever true at 1×.
/// The relationship is what belongs here; the number belongs where it is defined.
///
/// # Who enforces it, and it is not this constant
///
/// This is a budget, not a mechanism — the first line of this document used to say *"before
/// this stops handing it more"*, which described `desktop::push` and nothing on `wasm32`.
/// Three things act, in order, and only the first is meant to:
///
/// 1. `frontend::audio::Resampler::track` holds the queue near **half** this figure by moving
///    the output rate a fraction of a percent. On both targets. This is the mechanism.
/// 2. `crates/frontend`'s frame loop refuses to push past **twice** that setpoint — a full
///    buffer — for the case the loop cannot cover. On both targets.
/// 3. The device's own ring drops rather than grows: `desktop::push` below, and
///    `web/zx_audio_worklet.js`'s preallocated ring in the browser. Last resort, and reached
///    only if something above has already failed.
///
/// # Stretching, and what this ruling does and does not refuse
///
/// *This section read: **"Audio is what makes a dropped frame audible, and that is not papered
/// over.** A resampler that stretched to hide an underrun would turn a counted, visible failure
/// into an inaudible-but-wrong one, which is the trade this project refuses everywhere else."*
/// `frontend::audio::Resampler::track` is now exactly a resampler that stretches, and
/// `crates/frontend/src/main.rs` reads this constant to do it. **The ruling was not amended
/// when that landed, so it is amended here rather than deleted** — a ruling that is quietly
/// dropped the first time it is inconvenient was never a ruling.
///
/// The distinction that reconciles them is real and is about *whose failure it is*:
///
/// - **A dropped frame is a failure, it is counted, and hiding it is still refused.** The
///   pacer knows it dropped one; the status bar shows the count in red. A resampler that
///   stretched the surrounding audio to cover the 20 ms gap would convert that into an
///   inaudible-but-wrong output with a visible counter nobody would trust. Still no.
/// - **Crystal drift between two free-running clocks is nobody's failure.** The emulated
///   machine runs at 50.08 Hz and the sound card consumes exactly one second per second, and
///   the two were never going to agree. There is no event to count and no fault to display,
///   only a queue that grows or shrinks forever. Correcting it continuously is the only
///   treatment that is not itself a defect — the ceiling that used to bound it discarded a
///   whole frame at a time, which is a discontinuity, which is the tick a person heard.
///
/// **The clamp is what keeps the second from becoming the first.** `MAX_CORRECTION` in
/// `crates/frontend/src/audio.rs` is ±0.5 %, which is 8.6 cents of pitch — below what anyone
/// hears — and at that rate absorbing one 20 ms dropped frame would need **four full seconds**
/// of the correction pinned at its rail. A stretch that small cannot conceal a dropped frame;
/// it can only track a drift. Widening the clamp would move this back into the refused case,
/// and that is the tripwire to remember if anyone is ever tempted.
pub const BUFFER_MILLISECONDS: u32 = 100;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// The sample rate of the page's `AudioContext`, or 0 unless it is `running`.
    fn zx_audio_rate() -> u32;

    /// Hand `count` mono `f32` samples at `samples` to the page.
    ///
    /// # The contract, which is one contract
    ///
    /// **Returns the number of samples queued ahead of the caller, counting the ones just
    /// handed over.** `-1` means there is no device, and nothing was taken.
    ///
    /// *There were three of these and the sentence here described none of them.* It said
    /// *"Returns samples still queued"*, `desktop::push` measured the depth **after** the
    /// push, and `web/zx_page.js` returned the worklet's last periodic report — which
    /// **excludes** the chunk just posted and lags by up to `REPORT_EVERY` render quanta. That
    /// stopped being a documentation defect the moment `frontend::audio::Resampler::track`
    /// started consuming this number every 20 ms: a control loop whose sensor systematically
    /// under-reads by one frame is a loop with a bias built into it, and the bias was invisible
    /// because each of the three answers was locally reasonable. The desktop's is the contract;
    /// the browser was made to honour it.
    ///
    /// # How the browser honours it, given that the worklet is on another thread
    ///
    /// `postMessage` is **ordered**, so the page can account for what is in flight rather than
    /// guess. The worklet tracks `received` — the running total of samples it has ever
    /// accepted — and reports it beside `queued`; the page tracks `pushed`, the running total
    /// it has ever handed over, and answers
    ///
    /// ```text
    /// reportQueued + (pushed - reportReceived)
    /// ```
    ///
    /// `pushed - reportReceived` is *exactly* the samples posted but not yet counted by a
    /// report, so the systematic omission disappears rather than being estimated away.
    ///
    /// # The residual, and which way it errs
    ///
    /// What is left is the worklet's **drain** since its last report, which the sum above
    /// cannot see. It is bounded by `REPORT_EVERY` render quanta — 8 × 128 = 1024 samples,
    /// ≈21.3 ms at 48 kHz, just inside one emulated frame — and it can only make the answer
    /// **too large**. So the loop errs toward believing the queue is shorter than it is, which
    /// pushes the correction toward producing *less* audio, which is the safe direction: the
    /// failure it protects against is a queue that climbs for the life of the tab.
    fn zx_audio_push(samples: *const f32, count: usize) -> i32;
}

/// The highest rate [`audio_rate`] will accept from `zx_audio_rate`, in Hz.
///
/// # The count in `MAX_QUERY_BYTES`'s argument was wrong by one, and this is the one
///
/// That constant's doc calls the query length *"the one number still taken on faith"*. It was
/// not alone: this rate crossed the same seam unchecked, and it does both of the things that
/// argument turns on. It feeds checked `u32` arithmetic — `frontend::audio::queue_target`
/// computes `device_hz * BUFFER_MILLISECONDS`, which under this workspace's
/// `overflow-checks = true` and `panic = "abort"` is a **process abort** for any rate above
/// 42,949,672 Hz — and it sizes a heap allocation, `mixed.reserve(device_hz / 50 + 64)` in
/// `crates/frontend`'s frame loop, which a hostile `u32::MAX` turns into a ~344 MB request: a
/// second abort route on this target. Both are the blank tab that doc refuses. The ABI hole is
/// also the same one: the wasm ABI carries this `u32` as `i32`, so a shim returning a negative
/// number arrives as a value near `u32::MAX` rather than as anything an honest device reports.
///
/// # Why this number
///
/// 768,000 Hz is the ceiling browsers put on `AudioContext.sampleRate` — Chromium refuses to
/// construct a context outside 8,000..=768,000 Hz, and Firefox stops at the same bound — so no
/// rate a real page can hand over loses to this cap, and the only caller that can reach it is
/// the stub or hostile shim this crate assumes may exist.
///
/// # What happens above it
///
/// The rate is **refused**: [`audio_rate`] answers 0, which the caller already reads as
/// *"there is no device"* — the frame loop keeps polling and the emulator carries on silent,
/// a fault a person can notice, where the alternative is an abort with no message anywhere.
/// Refusal rather than clamping, because a shim claiming five gigahertz has not mis-measured
/// a device; resampling for a 768 kHz fiction would invent one. Like `MAX_QUERY_BYTES`, the
/// firing cannot be reported from here — the reason is at that constant and is not restated.
///
/// Not `#[cfg(target_arch = "wasm32")]` like its sibling, deliberately: the assertion below is
/// the compile-time proof that every accepted rate keeps `queue_target`'s product inside
/// `u32`, and gating the constant to `wasm32` would take that proof out of every native build,
/// `cargo test` included.
const MAX_DEVICE_HZ: u32 = 768_000;

// The checkable half of the refusal above: no accepted rate can overflow
// `frontend::audio::queue_target`'s `device_hz * BUFFER_MILLISECONDS`. Graded at compile time
// on every target, which is what leaving `MAX_DEVICE_HZ` un-`cfg`-gated buys.
const _: () = assert!(MAX_DEVICE_HZ as u64 * BUFFER_MILLISECONDS as u64 <= u32::MAX as u64);

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
///
/// **A caller that stops polling once it has a rate is making a bet, and this is where the bet
/// is recorded.** `crates/frontend`'s frame loop polls only until a resampler exists, so it
/// never learns that a context has *left* `running` — an iOS interruption, an output-device
/// change, any `suspend`. That case is handled at the other end instead: [`audio_push`]
/// answers `-1` and takes nothing whenever the context is not `running`, which is the same
/// *"there is no device"* the caller already understands, so a suspended tab cannot accumulate
/// a queue nothing is draining.
///
/// **A fourth zero exists on this target only: a refused rate.** Anything above
/// [`MAX_DEVICE_HZ`] — a bound no real browser's `AudioContext` can reach — is reported as
/// this same 0 rather than believed; that constant carries the arithmetic it protects.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn audio_rate() -> u32 {
    // SAFETY: takes no arguments and touches no memory this program owns; only a return value
    // crosses. If the page never registered the shim, miniquad's
    // `add_missing_functions_stabs` substituted one returning `undefined`, which arrives as 0 —
    // "there is no device", which is exactly the right answer for a page without the shim.
    let rate = unsafe { zx_audio_rate() };
    // Refused rather than believed above the bound; the argument lives on `MAX_DEVICE_HZ`.
    if rate > MAX_DEVICE_HZ { 0 } else { rate }
}

/// Hand mixed mono samples to the device; returns how many are queued **including** them.
///
/// One contract on both targets, stated once on the `zx_audio_push` declaration above and not
/// restated here: the count is the samples ahead of the caller *counting the ones just handed
/// over*, and `-1` means there is no device and nothing was taken.
///
/// `-1` covers a suspended `AudioContext` as well as an absent one, and covers it *late* — a
/// context can leave `running` long after the caller stopped polling [`audio_rate`]. A caller
/// that keeps generating audio into a `-1` is not doing anything harmful — nothing is queued,
/// so nothing grows — but it is doing work for nothing.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn audio_push(samples: &[f32]) -> i32 {
    // SAFETY: `samples` is borrowed for the whole call, so the pointer is valid for reads of
    // exactly `samples.len()` `f32`s and cannot be freed or moved while the callee runs.
    // `web/zx_page.js` builds one `Float32Array` view over that range and retains neither the
    // view nor the pointer. The view's contents leave wasm memory before anything else happens
    // to them, through `.slice()`, which returns a fresh `ArrayBuffer` — the copy is what makes
    // the transferred buffer independent of wasm memory, which may grow and detach afterwards.
    //
    // The reallocation half is discharged by the ordering inside the shim, for the reason
    // `offer_download`'s block sets out at length: `.slice()` is this view's last read, and
    // everything the shim does afterwards touches only the copy. That is a local fact about one
    // function, where *"the shim never calls back into Rust"* — which this comment used to say,
    // and which is true of this shim today — is a claim about the whole page that no reviewer
    // on this side can check.
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

    /// The most the ring may hold: [`BUFFER_MILLISECONDS`] of audio at [`REQUESTED_HZ`].
    ///
    /// *This expression was written twice — once to size the allocation, once per `push` to
    /// test against it.* One derived quantity with two spellings is the shape this workspace
    /// refuses everywhere else, and the second copy was recomputed fifty times a second for a
    /// figure that cannot change while the process runs.
    ///
    /// **Making it a `const` also moves an overflow from run time to compile time**, which is
    /// the half worth stating. `REQUESTED_HZ * BUFFER_MILLISECONDS` is `u32 × u32` and the
    /// workspace sets `overflow-checks = true` in *both* profiles, so any `BUFFER_MILLISECONDS`
    /// above 89,478 was a panic on the audio path — in a `pub` constant this crate documents as
    /// a latency knob somebody is invited to change. In a `const` the same arithmetic is a
    /// build failure with the constant named in it.
    const CEILING: usize = (REQUESTED_HZ * BUFFER_MILLISECONDS / 1000) as usize;

    /// Everything generated and not yet played.
    static RING: OnceLock<Mutex<VecDeque<f32>>> = OnceLock::new();

    /// Whether a device has been opened, and at what rate. Zero means "not yet, or refused".
    static RATE: OnceLock<u32> = OnceLock::new();

    fn ring() -> &'static Mutex<VecDeque<f32>> {
        RING.get_or_init(|| Mutex::new(VecDeque::with_capacity(CEILING)))
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

    /// Take as much of `samples` as fits under [`CEILING`], and report the new depth.
    ///
    /// Pure, and separated from [`push`] for two reasons that turned out to be the same one.
    /// `push` opens a real audio device on its first call, so nothing about it can be tested
    /// on a machine in CI; this half is the whole of the policy and needs no device at all.
    /// And the policy is worth testing: *dropping rather than growing* is argued for in four
    /// sentences below and was, until this was extracted, asserted by nothing.
    fn enqueue(queued: &mut VecDeque<f32>, samples: &[f32]) -> usize {
        // **Dropped rather than grown.** An unbounded queue does not fix an emulator running
        // fast; it converts a small permanent error into a latency that climbs for as long as
        // the session lasts, which is the self-amplifying shape `crate::pacing`'s own
        // documentation refuses for frames. `web/zx_audio_worklet.js`'s ring is the same rule
        // on the other target, arrived at by the same argument.
        //
        // **In bulk, and the bulk is not a micro-optimisation — it is how long this holds the
        // lock.** This ran as a per-sample `push_back` behind a per-sample `len() >= ceiling`
        // test: ~958 iterations per frame, fifty times a second, ~48,000 iterations a second,
        // *all inside the mutex the 93.75 Hz real-time callback below needs*. `extend` over a
        // `TrustedLen` iterator specialises to a bulk copy, so the critical section becomes one
        // `memcpy` and the break-on-full semantics are unchanged: `room` is exactly the number
        // the loop would have pushed before its first `break`.
        let room = CEILING.saturating_sub(queued.len()).min(samples.len());
        queued.extend(samples[..room].iter().copied());
        queued.len()
    }

    /// Write one queued sample to every channel of each frame, and silence when it runs dry.
    ///
    /// The other half of [`fill`] that needs no device — see [`enqueue`].
    fn drain(queued: &mut VecDeque<f32>, output: &mut [f32]) {
        for frame in output.chunks_mut(CHANNELS) {
            // Underrun is silence, not the previous sample repeated. A repeat is a buzz that
            // sounds like a defect in the emulator; silence sounds like a gap, which is what it
            // is, and `Pacer::dropped` is already counting the cause on screen.
            let sample = queued.pop_front().unwrap_or(0.0);
            frame.fill(sample);
        }
    }

    /// The audio callback. Runs on the device's own thread.
    fn fill(output: &mut [f32]) {
        // **This is a real-time thread and this is a blocking lock**, which the comment here
        // did not say — it addressed poisoning only, and poisoning is the rarer hazard by a
        // wide margin. `tinyaudio` calls this every `BLOCK` frames, so the deadline is
        // 512/48000 = 10.67 ms and a miss is an audible hole. The producer holds the same
        // mutex.
        //
        // What bounds the wait is [`enqueue`]'s critical section, which is one bulk copy of at
        // most `CEILING` `f32`s — microseconds, and no allocation, because the `VecDeque` was
        // built at `CEILING` capacity and never grows past it. That is a mitigation and not a
        // proof: a blocking lock on a real-time thread is unbounded priority inversion in
        // principle, and the fix that removes the possibility is a single-producer
        // single-consumer lock-free ring. That was weighed and dropped at ~+30 lines for a
        // window now measured in microseconds. **`try_lock` is not the answer** and is written
        // down here because it looks like one: it would convert a rare few-microsecond wait
        // into a rare 10.67 ms hole, which is audibly worse than the thing it avoids.
        let Ok(mut queued) = ring().lock() else {
            // A poisoned lock means a panic happened while holding it. Silence is the only
            // thing this callback can do about that, and it must not panic in turn: unwinding
            // out of an audio callback crosses an FFI boundary.
            output.fill(0.0);
            return;
        };
        drain(&mut queued, output);
    }

    /// Queue `samples`, and report how many are waiting including them.
    pub fn push(samples: &[f32]) -> i32 {
        if rate() == 0 {
            return -1;
        }
        let Ok(mut queued) = ring().lock() else {
            return -1;
        };
        let depth = enqueue(&mut queued, samples);
        i32::try_from(depth).unwrap_or(i32::MAX)
    }

    /// The desktop device's policy, tested without a desktop device.
    ///
    /// **What is covered, and what cannot be.** This module had no tests at all: nothing
    /// exercised the ring, the ceiling whose drop-rather-than-grow rule is argued for in four
    /// sentences, or `fill`'s underrun-is-silence. The obstacle was structural rather than
    /// anybody's oversight — [`push`] calls [`rate`] first, `rate` opens a real device through
    /// `tinyaudio`, and a machine in CI has none — so the two pure operations were lifted out
    /// from behind that gate as [`enqueue`] and [`drain`], and they are the whole of the policy.
    ///
    /// Still uncovered, and named rather than left to be discovered: `push`'s `-1` on a refused
    /// device, `push`'s `-1` on a poisoned lock, and `fill`'s silence on a poisoned lock. The
    /// first needs a machine with no sound card; the other two need a panic deliberately
    /// induced while holding a `static` shared with every other test in this binary, which
    /// would poison it for all of them. `web/gate.sh`'s T4 register is where a device is
    /// observed at all.
    #[cfg(test)]
    mod tests {
        use super::{BLOCK, CEILING, CHANNELS, VecDeque, drain, enqueue};

        /// A ring at the size `ring()` builds, holding `samples`.
        fn ring_of(samples: &[f32]) -> VecDeque<f32> {
            let mut queued = VecDeque::with_capacity(CEILING);
            queued.extend(samples.iter().copied());
            queued
        }

        #[test]
        fn the_ceiling_is_the_documented_buffer_at_the_requested_rate() {
            // 100 ms of audio at 48 kHz is 4,800 samples. A literal and not the expression the
            // constant is defined by, which would assert nothing: this is the arithmetic
            // `BUFFER_MILLISECONDS` promises a reader, checked independently of how `CEILING`
            // computes it. It goes red if the latency budget moves, which is the point — that
            // is a decision, not a refactor.
            assert_eq!(CEILING, 4_800);
        }

        #[test]
        fn a_push_that_fits_is_taken_whole_and_reports_the_new_depth() {
            let mut queued = ring_of(&[]);
            assert_eq!(enqueue(&mut queued, &[0.25, 0.5, 0.75]), 3);
            assert_eq!(enqueue(&mut queued, &[1.0]), 4);
            assert_eq!(
                queued.iter().copied().collect::<Vec<f32>>(),
                vec![0.25, 0.5, 0.75, 1.0],
            );
        }

        #[test]
        fn an_empty_push_changes_nothing() {
            let mut queued = ring_of(&[0.5]);
            assert_eq!(enqueue(&mut queued, &[]), 1);
            assert_eq!(queued.len(), 1);
        }

        #[test]
        fn a_push_that_overflows_the_ceiling_is_truncated_rather_than_grown() {
            // The load-bearing assertion of the whole module: the *tail* is discarded and the
            // audio already queued is untouched, so latency stops climbing instead of the ring
            // absorbing the error. A queue that grew here would pass every other test in this
            // file.
            let mut queued = ring_of(&[]);
            let more_than_fits: Vec<f32> = (0..CEILING + 100).map(|_| 0.5).collect();
            assert_eq!(enqueue(&mut queued, &more_than_fits), CEILING);
            assert_eq!(queued.len(), CEILING);
        }

        #[test]
        fn the_samples_kept_are_the_oldest_and_the_ones_dropped_are_the_newest() {
            // Which end is discarded is not arbitrary. The device is about to play what is at
            // the front, so dropping from the front would put a hole in audio the ring already
            // holds; dropping from the back loses samples nothing has committed to yet.
            let mut queued = ring_of(&[]);
            let head: Vec<f32> = (0..CEILING - 2).map(|_| 1.0).collect();
            assert_eq!(enqueue(&mut queued, &head), CEILING - 2);
            assert_eq!(enqueue(&mut queued, &[2.0, 3.0, 4.0, 5.0]), CEILING);
            assert_eq!(queued.front().copied(), Some(1.0));
            assert_eq!(queued.back().copied(), Some(3.0));
        }

        #[test]
        fn a_push_into_a_full_ring_takes_nothing_and_still_reports_the_depth() {
            let mut queued = ring_of(&[]);
            let fills: Vec<f32> = (0..CEILING).map(|_| 0.5).collect();
            assert_eq!(enqueue(&mut queued, &fills), CEILING);
            assert_eq!(enqueue(&mut queued, &[9.0]), CEILING);
            assert_eq!(queued.back().copied(), Some(0.5));
        }

        #[test]
        fn the_ring_never_reallocates_under_the_ceiling() {
            // `CEILING` is what `ring()` sizes the `VecDeque` with, and `enqueue` is the only
            // thing that puts samples in it. If the two ever disagree the allocation happens on
            // the frame thread while the real-time callback waits for the lock.
            let mut queued = VecDeque::with_capacity(CEILING);
            let capacity = queued.capacity();
            let fills: Vec<f32> = (0..CEILING).map(|_| 0.5).collect();
            enqueue(&mut queued, &fills);
            enqueue(&mut queued, &fills);
            assert_eq!(queued.capacity(), capacity);
        }

        #[test]
        fn every_channel_of_a_frame_gets_the_same_mono_sample() {
            let mut queued = ring_of(&[0.25, 0.75]);
            let mut output = [-1.0_f32; 2 * CHANNELS];
            drain(&mut queued, &mut output);
            assert_eq!(output, [0.25, 0.25, 0.75, 0.75]);
            assert!(queued.is_empty());
        }

        #[test]
        fn an_underrun_is_silence_and_not_the_previous_sample_repeated() {
            // A repeat is a buzz that sounds like a defect in the emulator; silence sounds like
            // a gap, which is what it is. The distinction is argued for in `drain` and this is
            // the assertion behind it.
            let mut queued = ring_of(&[0.5]);
            let mut output = [-1.0_f32; 3 * CHANNELS];
            drain(&mut queued, &mut output);
            assert_eq!(output, [0.5, 0.5, 0.0, 0.0, 0.0, 0.0]);
        }

        #[test]
        fn a_drain_takes_only_what_the_device_asked_for() {
            let mut queued = ring_of(&[1.0, 2.0, 3.0]);
            let mut output = [0.0_f32; CHANNELS];
            drain(&mut queued, &mut output);
            assert_eq!(output, [1.0, 1.0]);
            assert_eq!(queued.iter().copied().collect::<Vec<f32>>(), vec![2.0, 3.0]);
        }

        #[test]
        fn a_full_ring_survives_a_whole_device_block_without_underrunning() {
            // The two halves against each other at the sizes the device actually uses, which is
            // the only place a `CHANNELS`/`BLOCK` mismatch would show: `output` is `BLOCK`
            // frames of `CHANNELS` samples, so a `drain` consumes `BLOCK` queued samples, not
            // `BLOCK * CHANNELS`.
            let mut queued = ring_of(&[]);
            let frame: Vec<f32> = (0..BLOCK).map(|_| 0.5).collect();
            enqueue(&mut queued, &frame);
            let mut output = vec![0.0_f32; BLOCK * CHANNELS];
            drain(&mut queued, &mut output);
            assert!(queued.is_empty());
            assert!(output.iter().all(|&sample| sample == 0.5));
        }
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
