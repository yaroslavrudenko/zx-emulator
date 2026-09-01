//! The two things a browser does not do the way a terminal does.
//!
//! # This module is the entire `wasm32` seam
//!
//! `docs/ARCHITECTURE.md` puts M8 at *"WASM + macroquad — playable from a URL"*, and the way
//! to make that a build change rather than a rewrite is to keep the count of non-portable
//! calls small enough to list. It is two: **where the arguments come from**, and **how bytes
//! get out**. Everything else in this crate is portable already, by three deliberate choices
//! made here rather than discovered later:
//!
//! - Time comes from [`macroquad::time::get_frame_time`] and [`macroquad::time::get_time`],
//!   never from [`std::time::Instant`]. `Instant::now` is the usual thing that makes an
//!   otherwise-portable frame loop fail on `wasm32-unknown-unknown`, and [`crate::pacing`]
//!   takes a `Duration` and a `f64` precisely so it never has to ask what a clock is.
//! - Bytes come in through [`macroquad::file::load_file`], which is `async` on both targets
//!   and is a `fetch` in a browser. The `async fn main` that `#[macroquad::main]` already
//!   provides is what makes that free.
//! - Nothing in the frame loop touches the filesystem.
//!
//! # Why there is no `#[cfg]` here
//!
//! Both functions below **compile for `wasm32-unknown-unknown` as they stand** — `std::env`
//! and `std::fs` exist on that target — and degrade rather than break: [`arguments`] returns
//! empty, and [`save`] returns [`SaveError::Write`] carrying the standard library's own
//! `Unsupported`. A first WASM build therefore starts, boots a ROM fetched over HTTP, and
//! tells the user that saving is not available, which is a better first build than one that
//! does not link. Adding `#[cfg]` branches now would mean shipping a second implementation
//! that nothing compiles and nobody runs, and this project has a register full of what those
//! are worth.
//!
//! # What a real WASM build would still need
//!
//! 1. **A ROM.** `testdata/roms/48.rom` is not in the bundle; [`arguments`] returns nothing,
//!    so the default path is fetched relative to the page and the ROM has to be served beside
//!    it. Reading the query string would be the natural replacement for `argv`.
//! 2. **A download for [`save`].** The browser cannot write to a path. The replacement is a
//!    `Blob` and a synthetic `<a download>` click, which needs a few lines of JavaScript
//!    reached through `macroquad`'s `wasm-bindgen`-free FFI or a small `.js` shim beside
//!    `index.html`. [`free_path`]'s collision probing has no meaning there and would be
//!    dropped in favour of a fixed suggested filename.
//!
//!    > **Accurate about the mechanism and silent about the blocker, which is the part that
//!    > costs a day.** This crate's own `Cargo.toml` carries `unsafe_code = "forbid"`, and
//!    > every route out of `wasm` memory needs `unsafe` — including the one that looks like it
//!    > does not, because in edition 2024 an export is `#[unsafe(no_mangle)] extern "C"`, so a
//!    > "JS reaches in and takes the bytes" design moves the `unsafe` rather than removing it.
//!    > `forbid` is **not** `deny`: an `#[allow(unsafe_code)]` inside a `forbid`den crate is a
//!    > hard error, not an override. So this paragraph describes a change that does not
//!    > compile here as written. **The ruling is recorded in `docs/M8.md` Decision 4: `forbid`
//!    > stays on this crate, and the FFI moves to a small crate of its own** whose entire
//!    > contents are those few lines, each with a `// SAFETY:` comment. See that decision for
//!    > why relaxing a whole crate's posture to accommodate a download button is the worse
//!    > trade.
//! 3. **`index.html` plus `gl.js`**, which macroquad ships, and `cargo build --release
//!    --target wasm32-unknown-unknown`.
//! 4. **Keyboard capture.** A browser reserves some of the keys [`crate::keymap`] binds, and
//!    the two examples this paragraph used to give turn out to be on opposite sides of the one
//!    distinction that matters: whether the page is *offered* the event at all.
//!
//!    **`F5` was already handled, by a file that has been on this machine the whole time.**
//!    `miniquad-0.4.11`'s `js/gl.js` — which `macroquad`'s `js/mq_js_bundle.js` carries in
//!    minified form — installs `canvas.onkeydown` and calls `event.preventDefault()` on sapp
//!    keycodes **32** (`Space`), **39** (`'`), **47** (`/`), **258** (`Tab`), **259**
//!    (`Backspace`), **262–265** (the arrows) and **290–299** (`F1`–`F10`). `F5` is **294**,
//!    inside that last run. Read at `gl.js:1214-1229`, with the `"F5" => 294` mapping at
//!    `gl.js:539`, on 2026-09-01 against the version `Cargo.lock` pins. **`F5` does not reload
//!    the page and never was going to.** `F11` (300) and `F12` (301) are deliberately left
//!    out, so fullscreen and the developer tools stay with the browser — which is the right
//!    split and is evidence the list was designed rather than accumulated.
//!
//!    > **The lesson is cheaper than a wrong number and worth more.** This sentence reasoned
//!    > from *what a browser does* and was wrong, because the thing that actually decides it is
//!    > *what our vendored dependency already does about it*. A claim about a **dependency**
//!    > was made as a claim about the **platform**, when the dependency is pinned, vendored,
//!    > readable, and the first place to look. `crates/z80`'s corpus lessons say the same in
//!    > the other direction: read the artefact, not the specification of the artefact.
//!
//!    **The `Ctrl` half is real, it is worse than this paragraph suggested, and
//!    `preventDefault` cannot fix it.** Browser shortcuts fall in two classes and only one is a
//!    page's to refuse. `Ctrl+P`/`R`/`S`/`F`/`O` are dispatched to the page first and a shim
//!    can cancel them; `Ctrl+W`/`T`/`N`/`Tab` and **`Ctrl+1`…`Ctrl+9`** are consumed by the
//!    browser chrome and the page either never sees them or cannot stop them. See
//!    [`crate::keymap`] for why the second class specifically breaks a design decision rather
//!    than inconveniencing it, and `docs/M8.md` Decision 2 for the ruling that follows — `Tab`
//!    added as a third alias for `SYMBOL SHIFT`, in the one table, on both targets.

use std::io;
use std::path::Path;

/// Command-line arguments, without the program name.
///
/// Empty on `wasm32`, where a page has no `argv`. That is not an error condition: the shell
/// falls back to its default ROM path, which is what a URL-launched emulator wants anyway.
#[must_use]
pub fn arguments() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// Why bytes could not be written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SaveError {
    /// The write itself failed. On `wasm32` this is the standard library's `Unsupported`.
    #[error("could not write {path}: {source}")]
    Write {
        /// Where it was going.
        path: String,
        /// What the operating system said.
        source: io::Error,
    },

    /// Every candidate filename was taken.
    #[error("{stem}-1.{extension} through {stem}-{limit}.{extension} all exist already")]
    NoFreeName {
        /// The stem that was probed.
        stem: String,
        /// The extension that was probed.
        extension: String,
        /// How many were tried.
        limit: u32,
    },
}

/// Candidate filenames tried before giving up.
const NAME_LIMIT: u32 = 1000;

/// The first `stem-N.extension` that does not exist yet.
///
/// Snapshots are numbered rather than overwritten. Clobbering the previous save is the more
/// expensive mistake by a wide margin — a snapshot is the only record of a machine state that
/// took someone twenty minutes to reach, and there is no undo.
///
/// # Errors
///
/// [`SaveError::NoFreeName`] once [`NAME_LIMIT`] names are taken.
pub fn free_path(stem: &str, extension: &str) -> Result<String, SaveError> {
    (1..=NAME_LIMIT)
        .map(|index| format!("{stem}-{index}.{extension}"))
        .find(|candidate| !Path::new(candidate).exists())
        .ok_or_else(|| SaveError::NoFreeName {
            stem: stem.to_owned(),
            extension: extension.to_owned(),
            limit: NAME_LIMIT,
        })
}

/// Write `bytes` to `path`.
///
/// # Errors
///
/// [`SaveError::Write`] if the path cannot be written — including on `wasm32`, where it
/// always cannot.
pub fn save(path: &str, bytes: &[u8]) -> Result<(), SaveError> {
    std::fs::write(path, bytes).map_err(|source| SaveError::Write {
        path: path.to_owned(),
        source,
    })
}
