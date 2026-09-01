//! What the user asked for, and how bytes get out.
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
//! # Why there is still no `#[cfg]` here
//!
//! Both non-portable calls now have browser implementations, and **neither of them is in this
//! file**. They are in `crates/page`, whose two entry points compile and behave on every
//! target: [`page::query_string`] is empty off `wasm32`, and [`page::offer_download`] answers
//! [`page::Handoff::NoPage`], which is what routes [`save`] back to the filesystem.
//!
//! So the target-conditional code in this workspace is confined to one crate that exists for
//! it, and `tests/portability.rs` asserts the absence here as a property rather than leaving
//! it to habit. `docs/M8.md` Decision 7 leans on that absence hard, because it is the only
//! thing that makes a host-run test say anything about a `wasm32` build: the code
//! `cargo test -p frontend` runs and the code a browser runs are the same code.
//!
//! > **This section used to say something weaker and true, and the difference is worth
//! > keeping.** It said both functions *"compile for `wasm32-unknown-unknown` as they stand"*
//! > and degrade — `arguments` returning empty, `save` returning the standard library's
//! > `Unsupported` — which was *"a better first build than one that does not link"*. It was
//! > right, and it was a claim nobody had run. Run on 2026-09-01:
//! > `cargo build --release --manifest-path crates/frontend/Cargo.toml --target
//! > wasm32-unknown-unknown --bin zx` links, so `std::fs::write` really is `Unsupported`
//! > rather than a missing symbol. That half is now **proven of the link**. The other half —
//! > that [`Path::exists`] answers `false` rather than trapping — is a *run-time* claim and
//! > `web/README.md` records where it was observed.
//!
//! # What the browser build adds, and where each piece lives
//!
//! 1. **A ROM.** [`arguments`] appends the page's query string to `argv`, through
//!    [`arguments_from_query`], so `?rom=…&tape=…` becomes the same `Vec<String>` a command
//!    line would have carried and [`partition`] is not told which one it is holding. With no
//!    parameters the shell's default path is fetched relative to the page, which is the
//!    URL-launched case and needs nothing.
//! 2. **A download for [`save`].** A `Blob` and a synthetic `<a download>` click, reached
//!    through `crates/page`. [`free_path`]'s collision probing survives untouched — see its own
//!    documentation for why the mechanism degrading is the right outcome rather than a bug.
//!
//!    > **Accurate about the mechanism and silent about the blocker, which is the part that
//!    > costs a day.** This crate's own `Cargo.toml` carries `unsafe_code = "forbid"`, and
//!    > every route out of `wasm` memory needs `unsafe` — including the one that looks like it
//!    > does not, because in edition 2024 an export is `#[unsafe(no_mangle)] extern "C"`, so a
//!    > "JS reaches in and takes the bytes" design moves the `unsafe` rather than removing it.
//!    > `forbid` is **not** `deny`: an `#[allow(unsafe_code)]` inside a `forbid`den crate is a
//!    > hard error, not an override. `docs/M8.md` Decision 4 ruled option B — `forbid` stays
//!    > here, and the FFI is a crate of its own whose entire contents are those few lines, each
//!    > with a `// SAFETY:` comment. That crate is `crates/page` and it now exists.
//! 3. **`index.html` plus the JS bundle**, vendored from the pinned crate rather than a CDN.
//!    `web/` holds them; `web/README.md` records the provenance in the shape
//!    `testdata/README.md` uses for every corpus, because it is the same kind of artefact.
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

use crate::media::{self, Kind};

/// Command-line arguments and query-string arguments, in that order.
///
/// # Three sources, one precedence rule, and it is the one that was already there
///
/// A command line and a query string cannot both be non-empty: a browser has no `argv` past the
/// program name, and [`page::query_string`] is a compile-time empty string everywhere else. So
/// those two are **concatenated** rather than arbitrated between — not laziness about the
/// ambiguous case, but the removal of it, leaving no precedence rule to get wrong and no branch
/// that one target compiles and never executes.
///
/// A [`crate::bundle`]d build is the one source that can genuinely coexist with another, and it
/// gets the only rule here: **it supplies what nobody named.** That is not a new idea being
/// introduced; it is exactly what [`partition`]'s `default_rom` already does one level down, and
/// it means `zx some.tap` still means `some.tap` in a standalone build rather than being
/// silently overruled by the thing baked in.
///
/// `docs/M8.md` Decision 3 states the property all of this is for: routing every source into the
/// existing `Vec<String>` means they **cannot** disagree about what `--rom` means, or about
/// repeats, or about case — because after one function they are the same value, read by the same
/// [`partition`]. A fourth source, a file dropped on the window, joins further downstream still:
/// it arrives as bytes and a name and goes straight to [`crate::media::insert`], the same
/// function everything else reaches.
#[must_use]
pub fn arguments() -> Vec<String> {
    let mut asked: Vec<String> = std::env::args().skip(1).collect();
    asked.extend(arguments_from_query(&page::query_string()));
    if asked.is_empty() {
        asked.extend(crate::bundle::arguments());
    }
    asked
}

/// The query-string key whose value names a ROM.
const ROM_KEY: &str = "rom";

/// What a command line spells to name a ROM whose path has no extension.
const ROM_FLAG: &str = "--rom";

/// The arguments a query string names, in the order a command line would have carried them.
///
/// # The seam exists so the two sources can be compared, and that is the only reason it is public
///
/// Pure: no browser, no `#[cfg]`, no I/O. Private, it would be ungradeable, and the gate it
/// exists for is not *"does it parse"* — it is `tests/argument_sources.rs`, which runs a query
/// and the command line a person would have typed instead through the **same** [`partition`]
/// and asserts the results are equal.
///
/// # The rules, each with the reason it is not the other choice
///
/// | Query | Becomes | Why |
/// |---|---|---|
/// | `?rom=a.rom` | `["--rom", "a.rom"]` | The flag is emitted **always**, not only for extensionless paths, so that `?rom=roms/48` — a ROM served without one — works exactly like `zx --rom roms/48`. `partition` then handles both the same way it always did |
/// | `?tape=b.tap`, `?snapshot=x.z80`, or any other key | `["b.tap"]` | The key is documentation. What a file *is* is decided by [`media::kind_of`] on its extension, in the one place that decision already lives |
/// | `?x.tap` — no `=` | `["x.tap"]` | A bare word is its own value |
/// | `?a=b=c` | `["b=c"]` | Split on the **first** `=`, because a value may contain one |
/// | `?rom=`, `?`, `?&&`, `` | `[]` | An empty value is URL punctuation rather than an argument. A URL cannot express `zx ""` and nothing should have to guess that it meant to |
///
/// **Values are not percent-decoded, and that is deliberate rather than unfinished.** A value
/// here names a file the page will fetch, and `macroquad::file::load_file` is an
/// `XMLHttpRequest` against the path as given — so the value is going straight back into a URL.
/// Decoding `roms/a%20b.rom` here would put a literal space in that request and break precisely
/// the case decoding exists to fix. The server decodes it, which is where decoding belongs.
///
/// Keys are matched case-insensitively, the way [`media::kind_of`] already matches extensions.
#[must_use]
pub fn arguments_from_query(query: &str) -> Vec<String> {
    query
        .strip_prefix('?')
        .unwrap_or(query)
        .split('&')
        .flat_map(argument_for)
        .collect()
}

/// The arguments one `key=value` parameter stands for — none, one, or two.
fn argument_for(parameter: &str) -> Vec<String> {
    let (key, value) = parameter.split_once('=').unwrap_or(("", parameter));
    if value.is_empty() {
        return Vec::new();
    }
    if key.eq_ignore_ascii_case(ROM_KEY) {
        return vec![ROM_FLAG.to_owned(), value.to_owned()];
    }
    vec![value.to_owned()]
}

/// Split what the user asked for into the ROMs to build from and the files to load afterwards.
///
/// ROM paths accumulate **in the order they were named**, because that order is what
/// [`media::start`] reads the model off: one is a 48K, two are a 128 with the first paging in at
/// reset. `--rom PATH` is accepted as well, because a path with no extension is otherwise
/// unreachable, and it accumulates like any other. `default_rom` is used only when nothing named
/// a ROM at all.
///
/// > **This said *"the last `.rom` wins, which is what every other tool does with a repeated
/// > option and needs no error of its own"*, and M7 took that option away.** The sentence was
/// > right about repeated *options* and it stopped being right the moment a second ROM meant
/// > something: on a machine that can be a 128, `a.rom b.rom` is not a person changing their
/// > mind, it is a person naming a ROM pair. **This is the quiet kind of breaking change** — the
/// > same argument, reinterpreted, with no signature to notice it by — so it is written down
/// > here rather than left in a diff. What a repeated `--rom` no longer does is override;
/// > what it does now is add, and a third one is [`media::Error::RomCount`] rather than a
/// > silently dropped file.
///
/// > **This function was private to `src/main.rs` until M8, with no test of any kind**, while
/// > that file's own header said everything with a decision in it lives in the library and is
/// > reachable from `cargo test`. The paragraph above describes a behaviour change nothing
/// > graded. It moved because [`arguments_from_query`] needs it to be gradeable — the
/// > discriminating claim is that a query and a command line *agree*, and agreement is not
/// > checkable from outside the function they have to agree in — and it should have moved
/// > before that.
///
/// `default_rom` is a parameter rather than a constant here so that the shell keeps its own
/// default and this function keeps no policy: it decides how arguments are read, not what the
/// emulator does when there are none.
#[must_use]
pub fn partition(arguments: &[String], default_rom: &str) -> (Vec<String>, Vec<String>) {
    let mut roms = Vec::new();
    let mut rest = Vec::new();
    let mut expecting_rom = false;

    for argument in arguments {
        if expecting_rom {
            roms.push(argument.clone());
            expecting_rom = false;
        } else if argument == ROM_FLAG {
            expecting_rom = true;
        } else if media::kind_of(argument) == Some(Kind::Rom) {
            roms.push(argument.clone());
        } else {
            rest.push(argument.clone());
        }
    }
    if roms.is_empty() {
        roms.push(default_rom.to_owned());
    }
    (roms, rest)
}

/// Why bytes could not be written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SaveError {
    /// The write itself failed.
    ///
    /// Unreachable on `wasm32`, where [`save`] never reaches the filesystem at all — a browser
    /// answers [`page::Handoff::Started`] or [`page::Handoff::Refused`] and neither routes
    /// here. It is the only variant on every other target.
    #[error("could not write {path}: {source}")]
    Write {
        /// Where it was going.
        path: String,
        /// What the operating system said.
        source: io::Error,
    },

    /// Every candidate filename was taken.
    ///
    /// Unreachable on `wasm32` — see [`free_path`] — and kept rather than `#[cfg]`ed away,
    /// because it is reachable natively and because [`SaveError`] is `#[non_exhaustive]`, so
    /// nothing about the type has to move for a variant to be target-specific in practice.
    #[error("{stem}-1.{extension} through {stem}-{limit}.{extension} all exist already")]
    NoFreeName {
        /// The stem that was probed.
        stem: String,
        /// The extension that was probed.
        extension: String,
        /// How many were tried.
        limit: u32,
    },

    /// The page would not take the bytes.
    ///
    /// # The variant that exists because success is the dangerous answer
    ///
    /// Reached two ways a caller cannot distinguish and does not need to: the page's download
    /// shim ran and threw, or **it was never registered at all**. `miniquad`'s
    /// `add_missing_functions_stabs` replaces an absent import with a stub that returns
    /// `undefined`, which crosses the ABI as `0` — so a page deployed without `zx_page.js`
    /// calls a function that appears to work. `page::handoff` maps everything that is not the
    /// success code to a refusal, and this variant is where that lands.
    ///
    /// Without it, [`save`] would report `saved snapshot-1.z80` over a page that saved nothing,
    /// which `docs/M8.md` Decision 4 names as the one failure this route must not have.
    #[error("the page did not take {name}: nothing was saved - is web/zx_page.js served?")]
    Download {
        /// The filename that was suggested to the browser.
        name: String,
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
/// # On `wasm32` the mechanism stops working and the guarantee survives, which is why it stays
///
/// A browser has no filesystem for [`Path::exists`] to consult, so it answers `false`, the
/// first candidate always wins, and every save is offered as `snapshot-1.z80`. That reads like
/// a bug and is the correct outcome: **nothing is clobbered**, because the name is a
/// *suggestion* and the browser resolves the collision in the download directory by appending
/// its own counter. The stated reason survives; only the mechanism changes hands.
///
/// So the loop is left alone rather than switched off behind a `#[cfg]`. A branch would buy
/// 999 immediate `Err(Unsupported)` returns on one keypress — not syscalls, and not on any
/// per-frame path — and cost this crate the target-conditional-code-free property that
/// `tests/portability.rs` asserts and `docs/M8.md` Decision 7 rests its T1 tier on. That is a
/// bad trade in both directions.
///
/// [`SaveError::NoFreeName`] is therefore unreachable in a browser. Unreachable, not
/// impossible: it stays for the target where a thousand files really can exist.
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

/// Put `bytes` where the host puts bytes: a file on a desktop, a download in a browser.
///
/// `path` is a path on a desktop and a **suggested filename** in a browser, because a browser
/// has no addressable download directory. `docs/M8.md` lists that reinterpretation among the
/// risky changes rather than the safe ones — *"a reinterpreted parameter is the quiet kind of
/// breaking change"* — so it is stated here rather than left in a signature that did not move.
///
/// The three-way match is what keeps the fallback honest. [`page::Handoff::NoPage`] means
/// *there is no browser*, and only that answer reaches [`std::fs::write`]; a browser that
/// refused is **not** retried against a filesystem that does not exist.
///
/// # Errors
///
/// [`SaveError::Write`] if the path cannot be written, and [`SaveError::Download`] if a page
/// declined the bytes or never had the shim to take them.
pub fn save(path: &str, bytes: &[u8]) -> Result<(), SaveError> {
    match page::offer_download(path, bytes) {
        page::Handoff::Started => Ok(()),
        page::Handoff::Refused => Err(SaveError::Download {
            name: path.to_owned(),
        }),
        page::Handoff::NoPage => std::fs::write(path, bytes).map_err(|source| SaveError::Write {
            path: path.to_owned(),
            source,
        }),
        // `Handoff` is `#[non_exhaustive]`: a variant added in `crates/page` must be decided
        // here rather than swallowed, and the compiler cannot force that on a foreign enum.
        // Refusing is the safe default — it reports failure for an outcome nobody has
        // classified, where the alternative would report a save that may not have happened.
        _ => Err(SaveError::Download {
            name: path.to_owned(),
        }),
    }
}
