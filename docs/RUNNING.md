# Running the emulator — the full guide

The complete manual: every command here was run before it was written down, and the dates
and caveats travel with the commands. It lived in `README.md` until 2026-09-03; the README
keeps the short form and this file keeps everything.

## Running it

The shortest thing that works, in a checkout with nothing fetched and nothing configured:

```bash
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx -- testdata/roms/48.rom
```

That is a 48K in a window, booting to `© 1982 Sinclair Research Ltd`, with a keyboard on it. The
ROM it names is committed, so there is nothing to download first. Everything below is a variation
on that one line.

### What has to be there first

**Rust 1.98 or newer**, edition 2024 — both declared in the workspace `Cargo.toml`.
`rust-toolchain.toml` pins stable and already names the `wasm32-unknown-unknown` target, so the
browser build needs no extra setup either.

**The Sinclair ROMs are committed; no game is.** `testdata/roms/48.rom`, `128-0.rom` and
`128-1.rom` are in the repository under the permission quoted in
[`testdata/README.md`](../testdata/README.md). `testdata/games/` is gitignored, and a fresh clone
finds exactly one file in it: [`PROVENANCE.md`](../testdata/games/PROVENANCE.md), the record of what
the games are and where each came from, which ships precisely because none of them may. Every
command below that names a game is naming a file of **yours**.

**The desktop sound device is a dependency, and it has been built on one platform.**
`crates/page` takes `tinyaudio` on every target except `wasm32`, and its manifest records the
measurement rather than a claim: macOS builds here, Linux would want ALSA headers, Windows is
unverified from this machine. It is the likeliest thing on this page to stop a build on a machine
unlike this one, so it is named first rather than discovered.

### A window with a Spectrum in it

```bash
# A 48K.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx -- testdata/roms/48.rom

# A 128. Two ROMs, editor first — the *count* is what names the machine, so there is no
# --model flag and no second machine-building path for it to disagree with.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx -- \
    testdata/roms/128-0.rom testdata/roms/128-1.rom

# A 48K with a tape in the drive. It arrives *stopped*; see "Loading a game from tape" below.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx -- \
    testdata/roms/48.rom testdata/games/ManicMiner.tap

# A snapshot, restored over the top of a machine that must match it.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx -- \
    testdata/roms/128-0.rom testdata/roms/128-1.rom testdata/games/Cybernoid.z80
```

**Files are told apart by extension, in any order.** `.rom` builds the machine, `.tap` and `.tzx`
go in the drive, `.z80` and `.sna` are restored over the top. Name no ROM at all and the machine is
built from `testdata/roms/48.rom`. There is no positional rule to remember and no flag to get in
the wrong place: `crates/frontend/src/host.rs` sorts the list once, and the same function sorts a
URL's query string and a payload compiled into the binary.

**Files can also be dropped on the window**, on every target, in the same five formats and through
the same two functions. The status bar then says *which verb was applied* — a tape reads
`tape in the drive, press F3 to play: <name>`, a snapshot reads `snapshot restored from <name>` —
because a tape is inserted stopped, and a drop that appears to do nothing is indistinguishable
from a broken build.

> **`.tzx` belongs in both those lists and this file left it out of both.** It landed on
> 2026-09-01 — most commercial games ship as `.tzx`, because `.tap` cannot represent a turbo
> loader at all — and the sentence here still offered `.tap`, `.z80` and `.sna`. Checked rather
> than assumed, on 2026-09-01: `zx-shot --media testdata/games/MarioBros.tzx --play-tape` loads and
> photographs the *Mario Bros* screen. The same stale trio reached three more strings — the
> window's own opening message, `zx-shot`'s usage text, and `web/index.html`'s key guide — and all
> three now name the format. **Two of them are shut by assertion rather than by hand:** the
> opening message and `zx-shot`'s list are each read back against `media::EXTENSIONS` by a test, in
> both directions, so neither can fall behind the loader a second time or advertise a format it
> would refuse. The page's key guide has no such gate — `web/gate.sh` checks that `index.html`
> exists and never what it says — so it is the one that went stale here, and the one a sweep has
> to keep honest.

**A snapshot carries its own machine, and the command line has to agree with it.** That is why the
last line above names the 128 pair: `Cybernoid.z80` is a 128 snapshot, and handed to the default
48K it is refused outright —

```
zx-shot: testdata/games/Cybernoid.z80: a 128 snapshot cannot be restored into a 48K
```

— rather than half-restored with five banks quietly dropped. The refusal is the library being
strict on purpose; the frontend cannot fix it by guessing, because building a 128 needs two ROM
images this process may never have been given. If you meet that message, name the ROMs that
machine needs.

### The keys

| | |
|---|---|
| `Shift` | `CAPS SHIFT` — either hand |
| `Ctrl` **or** `Tab` | `SYMBOL SHIFT` — either hand |
| `Backspace`, `Escape`, `,` `.` `;` `'` `-` `=` `/` | the combination the Spectrum prints on the key |
| the arrow keys | whatever `F7` currently selects — see below |
| `F1` | show or hide the readout |
| `F2` | save a `.z80` |
| `F3` `F4` `F5` | tape play, stop, rewind |
| `F6` | reset |
| `F7` | change what the arrows send |

Everything else the Spectrum prints on a key is reached the way the machine reaches it: hold the
mapped shift and press the key the legend is printed on.

`F2` writes `snapshot-1.z80` into the directory you launched from, **numbering rather than
overwriting** — clobbering the previous save is the more expensive mistake, and there is no undo.
In a browser the same key produces a download instead.

**Use `Tab` for the brackets.** `(` and `)` are `SYMBOL SHIFT`+`8` and `SYMBOL SHIFT`+`9` on the
hardware, which is `Ctrl`+`8` and `Ctrl`+`9` — and a browser keeps those for switching tabs
*without offering them to the page at all*, so `preventDefault` has nothing to cancel and the
keystroke is simply gone. `Tab`+`8` and `Tab`+`9` work everywhere. `docs/M8.md` Decision 2 has the
reasoning and what it costs.

#### `F7`, and why the arrows are a scheme rather than a mapping

**The Spectrum has no arrow keys.** Nothing on the membrane means *move left*, so there is no
printed meaning for the keymap's founding rule to follow, and games do not agree with each other.
Six games from the owner's own collection were disassembled on 2026-09-01 to settle it — the
movement routines decoded from memory, because the printed instructions are wrong about at least
one of them — and **three of the six read no fixed keys at all**. There is no single mapping to
find, so a fixed one would silently do the wrong thing on some large fraction of any collection.
`F7` cycles, and the current choice sits on the status bar:

| Scheme | What the arrows send |
|---|---|
| `5678 + Kempston` | **the default** — the bare digits `5`/`6`/`7`/`8` *and* the joystick port |
| `cursor (BASIC)` | `CAPS SHIFT` + `5`/`6`/`7`/`8` — the editor's own cursor keys |
| `QAOP` | `Q`/`A`/`O`/`P` — the commonest hand-picked set |
| `Sinclair 1` | `6`/`7`/`8`/`9` — for a game offering *INTERFACE 2* |
| `Sinclair 2` | `1`/`2`/`3`/`4` — the second stick |
| `Kempston only` | the port and no key at all |

**The default is not the editor's, and that is deliberate.** *Manic Miner*'s jump check is
`LD BC,7EFEh : IN A,(C)`, which pulls two address lines low together and merges the row containing
`CAPS SHIFT` into the read — so holding the cursor chord to walk makes Willy jump on every press.
The bare digits are what a game means by "cursor keys". If you have come here to type BASIC, the
editor's arrows are one `F7` away, and `crates/frontend/src/keymap.rs` carries the whole table with
what each game was disassembled to find.

### Loading a game from tape

A tape in the drive does nothing on its own, and this is the step nobody guesses. It is what a
person did in 1983:

1. Type `LOAD ""` — that is the **`J`** key, which prints the whole word `LOAD`, then `Ctrl`+`P`
   **twice**, which is `SYMBOL SHIFT`+`P`, which is `"`.
2. Press `ENTER`.
3. Press **`F3`** — that is PLAY on the tape deck.

A tape started before `ENTER` loads nothing: the loader would meet the middle of a block, which is
why a tape goes in stopped and why `F3` is a separate key rather than something a drop does for
you.

Then wait, and the wait is the point rather than a fault. *Manic Miner* is about **9,244 frames**
of tape — a bit over three minutes at 50 Hz, which is what the window runs at, and what the real
machine took. The border stripes are the ROM's `LD-BYTES` toggling the border as it decodes each
bit; a flat rectangle would be the broken outcome, not that. There is no fast-forward.

**On a 128 it is shorter, and you do not type `LOAD ""` at all.** The boot menu comes up with
*Tape Loader* already highlighted, so it is `ENTER`, then `F3`, then the same wait. Checked rather
than assumed on 2026-09-01, headlessly, with one `Enter` and nothing else: *Manic Miner* loaded
through the 128's own loader and was photographed running one of its caverns. Pick *48 BASIC* if
you want the three steps above instead — **`128 BASIC` enters words a letter at a time**, so `J`
there prints `j` rather than `LOAD`, which was photographed beside it.

### One binary that boots straight into a game

```bash
ZX_BUNDLE_ROM=testdata/roms/48.rom \
ZX_BUNDLE_MEDIA=testdata/games/your.tap \
cargo build --release --manifest-path crates/frontend/Cargo.toml --features bundled --bin zx
```

The result takes **no arguments and looks for no files** — the ROM and the game are compiled in,
and the same loader that would have read a filesystem consults the payload first. It is off by
default, and the build **fails loudly** if the feature is on and the payload is missing, empty, or
a format this emulator cannot load: a standalone that starts and shows nothing looks like a broken
emulator rather than a build that was never given anything.

**The game is not in this repository and this builds a private artefact.** `testdata/games/` is
gitignored, no game in it is committed, and no permission here covers a game — so this is something
you build on your own machine, from a file you already have, for yourself. The binary carries
Amstrad's acknowledgement under the picture whenever the embedded ROM really is a Sinclair one,
because for a double-clicked artefact the window is both the program and the manual.

Measured 2026-09-01 with `48.rom` and a 33,168-byte `.tap`: the binary grows by **49,552 bytes**
against the same build without the feature, which is the two payloads and nothing else.

### In a browser

```bash
sh web/build.sh
cd target/web && python3 -m http.server 8000     # or: cargo install basic-http-server
```

Then `http://localhost:8000/`. The query string names files the way `argv` does, and is read by the
same function:

```
http://localhost:8000/?rom=testdata/roms/128-0.rom&rom=testdata/roms/128-1.rom
http://localhost:8000/?snapshot=fire.z80
```

**`file://` will not work, and it is not a bug to chase.** `macroquad::file::load_file` is an
`XMLHttpRequest` — the string is in the vendored bundle, one occurrence — and a page opened from
disk cannot issue one against a sibling file, so the ROM never arrives and the emulator draws its
*cannot read* screen. It has to be served over HTTP; any static server will do.

**Sound starts on your first click or keystroke.** A browser suspends an `AudioContext` until the
user has interacted with the page and no desktop browser can be argued out of it, so the readout
shows `snd --` until then and the emulator is silent while looking otherwise healthy. One click on
the canvas is the whole fix. On a desktop the device opens on the first frame and no click is
needed.

`web/README.md` records what has been *observed* in a browser and what has not — the page renders,
the keys arrive, a `.z80` round-trips out through `F2` and back in through a drop — and is careful
that none of it is a claim about being playable.

### Photographing a machine without opening a window

`zx-shot` is the second binary in the same crate. It boots a machine, optionally types at it
through the same keymap, and writes the screen to a file — no window, no GPU, usable over SSH, and
it is what produced every image in `README.md` and `docs/JOURNAL.md`.

```bash
# The 48K boot screen.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
    --out boot.ppm

# The 128's own menu. Two --rom, exactly as the window takes them.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
    --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom --out menu.ppm

# A game, loaded from tape by the real ROM: LOAD "" typed through the keymap, then PLAY.
cargo run --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot -- \
    --media testdata/games/ManicMiner.tap --play-tape \
    --keys 'J;LeftControl+P;LeftControl+P;Enter' --settle 9800 --out title.ppm
```

| Flag | |
|---|---|
| `--out PATH` | **required** — a P6 Netpbm frame, 320 × 256 |
| `--rom PATH` | repeats; one is a 48K, two are a 128, editor first |
| `--media PATH` | repeats; a `.tap`, `.tzx`, `.z80` or `.sna` |
| `--frames N` | frames before anything is typed. Default 120 |
| `--keys SCRIPT` | `;` separates taps, `+` joins keys held together — `J;LeftControl+P;Enter` |
| `--hold N` | frames each key is held. Default 10; **4 or fewer is missed by the ROM entirely, 5 registers, and 36 starts the 48K editor's auto-repeat** — both edges measured. Raise it for a game, which polls its own keys on its own terms and which nothing here has measured |
| `--settle N` | frames after the last key |
| `--play-tape` | press PLAY *after* the keys. A `.tap` loads nothing without it |
| `--wav PATH` | what a device would have been handed, mono 16-bit at 48 kHz |
| `--wav-from FRAME` | start recording at a frame index over the whole run |

There is no `--help`; `--help` is an unknown argument, and running with no `--out` prints the usage
to stderr and exits 1. Convert the output with `sips -s format png title.ppm --out title.png` on
macOS, or anything that reads Netpbm. [`docs/images/README.md`](images/README.md) has the full
recipes, including how each image was taken and re-checked.

**It is the same pipeline the window runs** — `keymap::apply` → `run_frame` → `render` →
`write_rgba` — differing only in the last step, where it writes bytes instead of uploading a
texture, and a test asserts those are the same buffer. Headless it is not paced to 50 Hz, so the
tape load above finishes in **about two seconds of wall clock** for a 10,000-frame run rather than
three minutes.

### What this is like at the moment

A development snapshot, and these are the things a person meets rather than reads about.

- **`zx-shot` can press a key after the tape starts, and the number it still cannot derive is the
  one after that.** `--keys-after` waits for the tape rather than counting to a constant: the
  half-periods `Tape::pulses` yields are T-states, their sum is the cassette end to end, and
  dividing by the machine's own frame length gives the wait. What the tape cannot say is how long
  the *game's* loader then takes to reach something that will accept a key — whether the next
  instant is a title screen, a menu or a black frame mid-clear is a property of the game, and
  nothing in the machine can be asked. That gap is `--settle`, it is an honest guess rather than a
  derived number, and `--hold` has an edge at each end: too short and *Exolon*'s menu never sees
  the tap, too long and the 48K editor's auto-repeat duplicates *every* tap in the script — the
  keyword, both quotes, and whatever `--keys-after` was going to send — so the line never parses
  and the tape never loads at all. How many copies of each you get is a function of `--hold` rather
  than a fact about it, so the count is measured in one place and not restated here:
  [`docs/images/README.md`](images/README.md), *What could not be photographed, and why*,
  under *"The other end of `--hold` is a threshold, not a number"*. `30` clears both edges, which
  is why every published command uses it. *(This entry read "`zx-shot` cannot press a key after the
  tape starts" and called the remedy a missing flag rather than a defect. The flag landed the same
  day; the two games shown playing rather than arriving, in `docs/JOURNAL.md`'s gallery, are what
  it bought.)*
- **The AY has never been heard — no listening to it is recorded anywhere here.** Its *structure*
  is gated: counter periodicity over all 4096 tone values, the envelope's sixteen shapes decoding to
  eight behaviours, mixer polarity, register write masks. Its *magnitudes* — the volume table and
  the tone, noise and envelope divisors — are **transcriptions from a datasheet**, and
  `crates/spectrum/src/ay.rs` says of each one, in its own doc comment, that nothing in this
  repository can adjudicate it. `docs/M7.md` puts *"that it sounds correct"* in T4, against a human
  ear. **So if it sounds wrong, this is the first thing to suspect**, and saying so is worth more
  than a green suite of sound tests, which would read as *"the sound chip works"* whatever it
  actually asserted.
- **`sh crates/frontend/gate-bundled.sh` exits 1 today, and the stale half is the gate.** Run
  2026-09-01: nine of its checks pass and one fails. The nine include the one that matters — an
  embedded ROM and a named ROM photograph byte-identically, and a *different* ROM does not, so the
  comparison bites. The failure is a step asserting that a `.tzx` payload is **refused** at build
  time, which stopped being true when `.tzx` support landed the same day and `build.rs`'s loadable
  list gained it. The feature is fine; the assertion is out of date. `crates/frontend` belongs to
  other changes in flight, so this is reported rather than edited.
- **`--all-features` fails, on purpose.** It turns on `bundled` without setting either payload
  variable, and `build.rs` then does exactly what it exists for. Cargo cannot tell *"the user asked
  for `bundled`"* from *"the user asked for everything"*. Use `--features bundled` with the
  variables set, or neither.
- **A non-US keyboard has not been tried by anybody here.** `miniquad` derives a key code from the
  *physical* key in a browser, on Windows and on macOS, and from the *layout's* keysym under X11 and
  Wayland — so one AZERTY keypress is two different membrane keys depending on the backend. Read out
  of five mapping tables in the pinned source and observed on no machine.

### Checking it

```bash
cargo test --workspace --no-fail-fast   # the unit tests, the FUSE vectors, the machine's gates
sh web/gate.sh                          # the browser build: T1 + T2 + T3, and what it cannot see
sh crates/frontend/gate-bundled.sh      # the bundled feature — see the note above; exits 1 today
cargo test --release -p z80 --test zex_oracle -- --ignored    # zexdoc + zexall
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

**Count the gates rather than trusting a number here.** That integer has moved repeatedly inside a
single day, and a stale count reads exactly like a current one:

```sh
ls -1 crates/spectrum/tests/*.rs | wc -l
```

**`cargo test` does not run the `zex` exercisers.** At 5.8 billion instructions each they are
`#[ignore]`d and release-only, which is why they have their own line — the pair took **43.6 s**
here on 2026-09-01 in release, against tens of minutes in the `dev` profile `cargo test` uses by
default. An ignored gate that no pipeline executes is this project's most-repeated defect; see
[`docs/STATUS.md`](STATUS.md).

**`--no-fail-fast` is not decoration.** Without it `cargo test` stops at the first failing target,
so one reddened unit test prevents every integration gate from running at all — and *"the gates did
not run"* looks identical to *"the gates passed"*.

**A missing corpus fails its gate rather than skipping it silently**, naming the fetch
instructions. So a fresh clone fails `cargo test` until `testdata/fuse` is fetched — that is
deliberate, and it replaced an earlier arrangement in which the absent corpus left the suite green
with the same test count. `ZX_CORPUS_ALLOW_MISSING=1` is the considered opt-out, and it is
**refused** when `CI` is also set.

> **This section was called *Build and run* and had no `run` line in it at all** until 2026-09-01 —
> the one thing a stranger wants from a README was the one thing the file did not say. It was then
> given one, and still assumed the reader knew that a tape needs `LOAD ""` and a keypress, that a
> `.z80` names its own machine, and that `F7` exists at all.
>
> **Every command in this section was run, on macOS, on 2026-09-01, before it was written down** —
> a command in a README that nobody has executed is the same class of defect as a gate that nothing
> runs, which this repository has now caught three times. Two of them cannot be run to the end and
> are not pretended otherwise:
>
> - **The `zx` lines open a window.** `#[macroquad::main]` creates the window *before* the function
>   body runs, so there is no headless way to exercise them and nothing was launched here. What was
>   checked is that the binary links, and that the same files sorted by the same `host::partition`
>   build the machines described — through `zx-shot`, which shares the loader, the keymap and the
>   render path and differs only in writing bytes instead of uploading a texture.
> - **The browser lines were built and served, not viewed.** `sh web/build.sh` assembled
>   `target/web/`, `python3 -m http.server 8000` served it, and every asset a bare URL needs came
>   back `200` over HTTP — the page, the module, both 128 ROMs, the page script and the audio
>   worklet. Nothing here rendered it; `web/README.md` records the runs that did, and is equally
>   careful that none of them establishes *playable*.

