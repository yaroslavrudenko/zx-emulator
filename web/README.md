# `web/` — the browser build

`crates/frontend` compiled to `wasm32-unknown-unknown`, plus the four files a browser needs to
run it. This directory holds the **sources** of the page; `sh web/build.sh` assembles the
**served directory** at `target/web/`.

```sh
sh web/build.sh
cd target/web && python3 -m http.server 8000     # then http://localhost:8000/
```

`file://` will not work, and a person who tries it will conclude the build is broken.
`macroquad::file::load_file` is an `XMLHttpRequest`, and a page opened from disk cannot issue
one against a sibling file — so `48.rom` never arrives and the emulator draws its "cannot read"
screen. It has to be served over HTTP. Any static server does; `python3 -m http.server` is used
above because it is already on this machine, and `cargo install basic-http-server` is the
Rust-shaped alternative. `docs/M8.md` Decision 8 rules deployment out of scope: M8 produces a
directory, and a domain, a CI job, a CDN and a cache policy each have an owner who is not this
milestone.

The output goes to `target/web/` and nowhere else, because `target` is the only entry in
`.gitignore` — so the assembled page, which contains a copy of a Sinclair ROM, cannot be
committed by accident. `testdata/README.md` names every committed ROM individually, precisely
so a new copy has to be argued for rather than swept in by a glob.

---

## What is in the served directory

| File | Where it comes from |
|---|---|
| `index.html` | this directory. The canvas, the key guide, and the Amstrad acknowledgement as visible text |
| `mq_js_bundle.js` | **vendored** from the pinned `macroquad` crate — provenance below |
| `zx_page.js` | this directory. The only JavaScript this project wrote: the query string, the download, and focusing the canvas |
| `zx.wasm` | `cargo build --release --target wasm32-unknown-unknown --bin zx` |
| `testdata/roms/48.rom` | copied from the checkout, at the path `DEFAULT_ROM` names, so a bare URL boots |

### Naming files in the URL

```
http://localhost:8000/
http://localhost:8000/?rom=testdata/roms/48.rom&tape=games/thing.tap
http://localhost:8000/?rom=testdata/roms/128-0.rom&rom=testdata/roms/128-1.rom
http://localhost:8000/?snapshot=fire.z80
```

The query string becomes the same argument list a command line would have carried, read by the
same `frontend::host::partition` — so the two sources cannot disagree about `--rom`, about
repeats, or about case. `crates/frontend/tests/argument_sources.rs` is the gate, and it asserts
agreement rather than parsing. Paths are relative to the page, are **not** percent-decoded (the
value goes straight back into an HTTP request), and a `rom` key works for an extensionless path
because it emits `--rom`.

A file can also be **dragged onto the page** — `.tap`, `.z80` or `.sna`. Dropping a `.rom` is
refused with `a ROM cannot be loaded into a machine that is already running`, which is the
right answer: a ROM is what the machine is made of, and swapping it means starting the emulator
again, which in a browser is reloading the page.

---

## Provenance of the vendored bundle

macroquad's own `README.md` gives an `index.html` whose script tag points at
`https://not-fl3.github.io/miniquad-samples/mq_js_bundle.js`. **That is refused**, for three
reasons in descending order: it is a third-party CDN in the trust path of a page that runs a
ROM, it cannot be pinned to the `miniquad` version in `Cargo.lock`, and it makes the page depend
on a host being up — a strange property for an emulator whose entire dependency set is otherwise
vendored and hashed. `docs/M8.md` Decision 6.

So the bundle is copied out of the pinned crate's own source and recorded here the way
`testdata/README.md` records a corpus, because it is the same kind of artefact: bytes from
outside this project whose provenance is the whole point.

### Fetching

```sh
src=$(ls -d ~/.cargo/registry/src/*/macroquad-0.4.16)
cp "$src/js/mq_js_bundle.js" web/mq_js_bundle.js
stat -f %z web/mq_js_bundle.js; shasum -a 256 web/mq_js_bundle.js; shasum -a 1 web/mq_js_bundle.js
cmp "$src/js/mq_js_bundle.js" web/mq_js_bundle.js && echo identical
```

| | |
|---|---|
| Size | 37407 bytes |
| SHA-256 | `4bf663a44a06c113bed92e0596c58139cd9db7f0d67ef2cc49ca06016e0b5ea0` |
| SHA-1 | `382096af45a592986f6ddaa054f0578729a0bf46` |
| From | `macroquad` **0.4.16**, `js/mq_js_bundle.js` — the version `Cargo.lock` pins |
| Contains | `miniquad` **0.4.11**'s `js/gl.js` verbatim, then `quad-snd`, `sapp-jsutils` and `quad-net` each wrapped in an IIFE, minified. That recipe is in the crate's own `js/README.md` |
| Verified | 2026-09-01 — copied from the pinned crate source in this checkout, `cmp` reports identical, every figure above taken from the committed bytes rather than transcribed |

`web/gate.sh` re-checks both halves: the SHA-256 against the figure in this table, and the file
against the registry copy with `cmp` when the registry is present. **The hash is the gate and
miniquad's own version check is not.** `gl.js` carries `const version = 2`, compares it against
the module's `crate_version()`, and `console.error`s on a mismatch — which does not stop the
page. A bundle from a different miniquad therefore produces an emulator that starts, draws, and
misbehaves somewhere, with the explanation in a console nobody has open. A recorded hash fails
loudly at build time; the runtime check fails quietly at the only moment it could not help.

### `zx_page.js` and `crates/page` are one contract in two files

The three import names, their argument order, and the rule that **1 means started and everything
else means refused** are fixed by `crates/page/src/lib.rs`. `PLUGIN_VERSION` appears in both
files and must match; `web/gate.sh` additionally asserts that all three imports exist in the
built module under the `env` module name, so a rename on one side that the other does not follow
is caught at build time.

That matters more than it sounds, because of one line in `gl.js`:

```js
function add_missing_functions_stabs(obj) { ... console.warn("No " + imports[i].name + " function in gl.js"); ... }
```

An import the page does not provide is replaced by a stub that logs a warning and returns
`undefined`, and `undefined` crosses the wasm ABI as **0**. So a page served without
`zx_page.js` still starts, still boots, still takes the keyboard — and `F2` calls a function
that appears to succeed. Making the success code `1` is what turns that from a silent no-op into
`the page did not take snapshot-1.z80: nothing was saved`.

---

## Licensing on a served page

`testdata/README.md` is the single source for the licensing claim and quotes Cliff Lawson of
Amstrad plc in full. The condition that reaches this directory is his:

> *"Amstrad are happy for emulator writers to include images of our copyrighted code as long as
> the (c)opyright messages are not altered and we appreciate it if **the program/manual**
> includes a note to the effect that 'Amstrad have kindly given their permission for the
> redistribution of their copyrighted material but retain that copyright'."*

**For a URL-launched emulator the page is both the program and the manual.** A person who opens
the link never sees `README.md`, never sees `testdata/README.md`, and never obtains a checkout —
so every copy of the acknowledgement this repository currently makes is invisible to exactly the
person the redistribution is being made to. The note is therefore in `index.html`, as visible
text under the canvas, in the permission's own words. Not in a comment, not behind a link, not
in the console.

`testdata/README.md`'s conditions table marks the acknowledgement row *"made above, and in
`README.md`"*. That is accurate for a checkout and became incomplete the moment this directory
existed. That file is not this pass's to edit; the row is reported in the M8 hand-off.

---

## What the gate covers

```sh
sh web/gate.sh
```

| Step | Class | Runs under `cargo test --workspace`? |
|---|---|---|
| Every existing frontend gate, plus the query/`argv` agreement table and the download's refusal codes | **proven** | yes |
| `crates/frontend` contains no `cfg(target`, and `crates/page` does | **proven** | yes |
| The `unsafe` surface is three blocks, one extern block and one attribute, each with a `SAFETY:` comment | **proven** | yes |
| `cargo clippy --target wasm32-unknown-unknown` — the only run that lints the `unsafe` blocks at all | **proven**, of the wasm build | **no** |
| The crate builds and **links** for `wasm32-unknown-unknown` | **proven**, of the link | **no** |
| The artefact exists, clears a size floor, and carries the imports `zx_page.js` provides and the exports `gl.js` calls | **proven**, of the artefact | **no** |
| `mq_js_bundle.js` is the pinned crate's, by SHA-256 and by `cmp` | **proven** | **no** |
| The page renders, keys arrive, a download lands, it is playable | **observed** | **no, and not by anything** |

**Not one assertion in that gate observes a pixel, a keypress or a frame in a browser**, and
that sentence belongs beside any future *"M8: green"*. `docs/M8.md` Decision 7 is explicit about
why this differs in kind from M6's and M7's top tiers: theirs were unautomatable because a
corpus could not be committed, which is a licensing accident another repository could fix. M8's
is unautomatable because *playable* is not a property of an artefact.

**Nothing runs `gate.sh` automatically**, and there is no CI here that could —
`docs/STATUS.md`'s open register still carries `.github/workflows/ci.yml` as *"verified locally
and enforced nowhere"*. Three of its steps are ordinary `cargo test` targets and do run with the
workspace suite; the ones needing a `wasm32` build cannot, short of a nested `cargo` inside a
test.

---

## T4 — the runs

An unrecorded observation is indistinguishable from one nobody made. Each run is recorded with
browser, version, operating system, what was done, what happened, and the date, exactly as M6's
T4 requires.

### 2026-09-01 — Chrome (stable), macOS 15.6 / Darwin 25.6.0, aarch64

Served from `target/web/` by `python3 -m http.server 8000 --bind 127.0.0.1`. Driven through the
Chrome DevTools Protocol, in a **browser instance launched for the purpose**, and each
observation below was gathered by a probe that returned `location.href` **in the same call as
the evidence**.

> **That last sentence is a correction, not a flourish.** The first pass drove the developer's
> own Chrome, whose selected tab moves while a person uses it — and a screenshot taken that way
> came back showing a 48K boot screen for a run that had asked for the 128. The picture was real
> and it was of **the wrong tab**. Nothing in the tool said so; the frame was a Spectrum, so it
> looked exactly like the answer. It was caught only because the answer was one the design said
> should not happen, which is a bad reason to catch something. This project's standing rule is
> that tooling can manufacture false evidence and that evidence which looks right is more
> dangerous than none; **a screenshot with no provenance in it is that hazard in a picture**, so
> every probe now carries its own URL and every claim below is one of those.

| # | Done | Observed |
|---|---|---|
| 1 | Opened `http://127.0.0.1:8000/` | Loaded. `document.title` = `ZX Spectrum 48K`; canvas 2580 × 2266 (high-dpi ×2) |
| 2 | Asked the module about itself | `wasm_exports.crate_version()` = **2**, which is `gl.js`'s own `const version = 2`, so the vendored bundle and the module agree. `wasm_exports.zx_page_crate_version()` = **1**, so `zx_page.js` registered and its imports are real rather than `add_missing_functions_stabs` stubs |
| 3 | Asked which element had focus | `document.activeElement.id` = **`glcanvas`**. The plugin's `on_init` focus worked, so keys reach the emulator without a click first |
| 4 | Read the framebuffer at the bare URL, twice, 1.3 s apart | 79,992 of 81,920 pixels `(215,215,215)` — non-bright white paper — with black ink and 40 blend colours. **The two samples differed**, so the machine is being *stepped*, not drawn once |
| 5 | Photographed it | The 48K boot screen: `© 1982 Sinclair Research Ltd`. Probe reported `search: ""` and the only ROM fetched was `/testdata/roms/48.rom` |
| 6 | Cropped the status bar at 1:1 | `50.3 Hz   dropped 0   frame 156   Tab or Ctrl = SYMBOL SHIFT - drop a .tap/.z80/.sna to load it - F1 hides this` |
| 7 | Opened `?rom=testdata/roms/128-0.rom&rom=testdata/roms/128-1.rom` | The probe reported the page fetched `/testdata/roms/128-0.rom` **and** `/testdata/roms/128-1.rom` **and not `48.rom`**, and photographed the **128 menu** — `© 1986`, the rainbow stripe, all five entries. A different machine, from the same page, by the URL alone |
| 8 | Dispatched `F2` at the canvas, with `URL.createObjectURL` and the anchor's `click` hooked | A `Blob` of `application/octet-stream`, **1380 bytes**, and an `<a download="snapshot-1.z80" href="blob:http://…">`. So `Path::exists()` answers **`false`** on `wasm32` rather than trapping, `free_path` yielded the first candidate, and `zx_offer_download` returned the success code |
| 9 | Took that same `Blob`, wrapped it in a `File`, and dispatched a `drop` at the window | Status bar: **`snapshot restored from snapshot-1.z80`**. A full round trip — saved by the browser, dropped back into it, restored by the machine |
| 10 | Dropped a `manic.tzx` | Status bar: `manic.tzx: a .tzx tape is not supported yet - the pulse-level pa…`, in red. Refused by name rather than as *"not a .rom, .tap, .z80 or .sna"* |

**Step 10 found a defect that only a screenshot could find, and it is worth the paragraph.** The
message was first written with an em dash, and the status bar drew it as an **empty box** —
`not supported yet ▯ the pulse-level`. It read perfectly in every test, because every assertion
about it compared one string to another string and both were equally unrenderable.
`crates/frontend/tests/on_screen_strings.rs` now asserts that everything `media::accept` and
`host::SaveError` can produce is drawable ASCII, and carries a positive control so the checker
itself is known to be able to fail. Three strings were fixed; the run above is the re-check.

### What these runs did not, and could not, settle

- **Whether a real `Ctrl+8` reaches the page.** A key injected through the DevTools Protocol is
  delivered *to the page* and does not traverse the browser's own shortcut layer, so an injected
  `Ctrl+8` arrives as an ordinary cancellable `keydown` **whether or not** a real one would
  switch tabs. The check would return a confident green for a question it never asked, so it was
  **refused rather than run**, and refusing it is the result. A person at a physical keyboard, in
  Chrome and in Firefox, on three operating systems, is the instrument. The same caveat applies
  in a weaker form to steps 8–10: those keys and drops are synthetic events aimed at the page's
  own handlers, which is exactly what they claim to exercise — the Rust path from `gl.js` inward
   — and they say nothing about the browser's shortcut layer.
- **That a file reached a download directory.** The anchor's `click` was intercepted, so what is
  proven is the `Blob` and the anchor, not a file on disk.
- **The layout divergence.** `miniquad`'s browser, Windows and macOS backends key off the
  **physical** position of a key and its X11 and Wayland backends off the **layout's keysym**, so
  one AZERTY keypress is two different membrane keys. Read out of five mapping tables in the
  pinned source and **observed on no machine**: every tester here is on a US layout, where all
  five backends agree and nothing is visible.
- **Whether it is playable**, which is the milestone's stated goal and is not reducible to
  anything above it. No game has been run.

### One question these runs did answer that was open

`docs/M8.md` listed *"whether `requestAnimationFrame` and `Pacer` interact well"* among the
things it could not settle, noting that `Pacer` *"has never been seen"* in a browser. Step 6
reads **50.3 Hz, dropped 0** over a one-second window. That is one machine, one browser, one
minute — **observed**, not proven — but it is no longer a hypothesis. And step 9's `dropped 28`,
in red, is the other half of the same behaviour working: the emulator was stalled by the probe,
counted what it lost, and said so.
