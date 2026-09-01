# zx-emulator

A **ZX Spectrum 48K and 128** emulator written from scratch in Rust — the Z80 core, the ULA,
contended memory, the tape, and the screen you are looking at.

![Cybernoid running in the emulator: a cavern walled in magenta and red alien masonry with yellow girders across it, a cyan status panel along the top carrying lives, a shield bar, a bomb count and a timer, and a scatter of small red and magenta enemies swarming down toward the player's ship at the lower left.](docs/images/cybernoid.png)

**_Cybernoid_, playing, on the 128.** The game arrives as a `.z80` snapshot, which lands *before*
the first frame runs — and that is what makes this a picture of a game being played rather than of
a menu: `1`, *START GAME* on the game's own screen, could be pressed through the same keymap table
the window uses. The 600 frames after it are the game running with nobody at the keyboard, so
every enemy on screen is where the game put it.

> **Every pixel of every image here is the emulator's output, enlarged by 2 and otherwise
> untouched** — `Spectrum::render` into a `Frame`, then `palette::write_rgba`, which is the path
> the window runs. There is no surround, no caption and no margin in any file, so *"which pixels
> are the machine's"* is not an argument but a program: `Colour::rgb` can emit exactly **fifteen**
> RGB values, so anything else in a file is foreign and a checker decides it. The checker was
> broken on purpose **three** times to show it can say no — most recently with a colour whose every
> channel is legal and whose *combination* is not, which the two earlier breaks would have missed.
> [`docs/images/README.md`](docs/images/README.md) has the commands, the checker and the breaks.

---

## Three more games

![Manic Miner running in the emulator: the Central Cavern, a black cave framed by yellow brick walls and red platforms, with Miner Willy in yellow standing on a green conveyor at the centre. The cavern's name is on a yellow strip below it, then the AIR meter and the score, all inside the machine's own red border.](docs/images/central-cavern.png)

**_Manic Miner_, playing, on a 48K, and it got there the long way.** `LOAD ""` typed through the
same keymap table the window presses, then the tape played, then the ROM's own `LD-BYTES` read all
six blocks off it — about three minutes of emulated time. **Nothing was pressed afterwards, and
nothing could have been:** `zx-shot` presses every key *before* it starts the tape. This is one
stop on the game's own attract loop, which tours the caverns while the title screen waits for an
`ENTER` that never came.

![The Cybernoid II loading screen: a green cavern floor under a black sky, crowded on both sides by magenta and red alien creatures, a blue-white orb burning at the top, and a large ship in the centre firing white beams past a small wreck in flames — framed by a border of tightly alternating blue and yellow horizontal stripes.](docs/images/cybernoid-ii.png)

**_Cybernoid II_, half-arrived.** This screen was on the tape *as a screen*: one block carrying no
header and exactly **6,912** bytes — 6,144 of pixels and 768 of colour, which is what a Spectrum
has. The border is still striped because the other **40,191** bytes have not landed yet. Those
stripes are the ROM's loader toggling the border as it decodes each bit, so they come out like
this only if the border is modelled *below* frame resolution; written once per frame it would be a
flat rectangle.

![The Exolon loading screen: a close-up of an armoured spaceman against a flat yellow sky, in white and grey armour with red and green panels, a red sun flaring at the left and the word ROCK scrawled across the helmet — framed by a border of tightly alternating red and cyan horizontal stripes.](docs/images/exolon.png)

**_Exolon_, whose picture was never on the tape.** Its three files are 125 bytes of BASIC and two
blocks of code, for addresses 27000 and 28000 — nothing 6,912 bytes long, nothing aimed at the
display file. **The picture is painted by code**, and 100 frames earlier it can be caught
half-drawn with a *black* border, which is the tell: the ROM's loader never leaves the border
alone. Here it is finished and the border is red and cyan — the ROM listening for the next block's
pilot tone, which is a different part of `LD-BYTES` from the blue and yellow above.

![Exolon being played: a black starfield carrying a green planet, a large red one and a small magenta one, a white and cyan lander hanging above a green pillared arch at the left, an armoured figure in white standing beneath the arch on a yellow tiled ledge, a green gun emplacement to the right of centre with a small shot in the air beside it and a yellow rock formation behind it, a band of red and magenta rubble below the ledge, and the game's own status line across the bottom reading AMMO 99, GRENADES 10, POINTS 000000, LIVES 6, ZONES 000.](docs/images/exolon-playing.png)

**_Exolon_, started — by a key pressed the better part of four minutes after the tape began.** Every
`--keys` tap happens *before* PLAY, so nothing here could reach a game that arrives that much later;
`--keys-after` presses once the tape has run out instead. It does not count frames to know when —
**it asks the tape.** `Tape::pulses` half-periods are T-states, their sum is the cassette end to end, and dividing
by the machine's own frame length gives the wait: 10,653 frames here, against a tape that ran out at
10,953. Then `Space;Space;Key1` — dismiss, advance, `1 START GAME`. **`LIVES 6` is the honest
detail:** nobody is at the keyboard, so the hero stands under that gun and dies about every four
seconds, and `6` is where the counter had reached 800 frames after `1`.

![Cybernoid II being played: a chamber walled in red and yellow alien masonry with cyan and green blocky platforms inside it, a white ship in flight at the upper left trailing yellow, two white round-headed guardians standing on a yellow platform below it, a large magenta pod at the lower right, and a cyan status panel across the top carrying a lives count, the scores 000025 and 000000, BOMBS 20, an hourglass and a bar meter.](docs/images/cybernoid-ii-playing.png)

**_Cybernoid II_, the same three taps, and the evidence is the scoreboard.** `000025`, up from
zero — a still of a title screen and a still of a running game are hard to tell apart, and a score
that has moved is the difference. The sequence is uniform because what it walks is uniform: a Hewson
loading screen and a Hewson title screen each consume one key, and only the third is a choice.
`Key1;Key1;Key1` starts both games too, which is what identifies the first two as *advance* rather
than as *select* — so neither picture required knowing anything about either game in advance.

> **No game is distributed by this repository** — `testdata/**` is `.gitignore`d and nothing in
> `testdata/games/` is committed. *Manic Miner* is Matthew Smith's, published by Bug-Byte Software
> Ltd in 1983; *Cybernoid*, *Cybernoid II* and *Exolon* are Raffaele Cecco's, published by Hewson
> Consultants Ltd in 1987 and 1988. **No permission covering these screenshots was found for any of
> them.** For *Manic Miner*, two archives sharing one dataset contradict each other about this exact
> file. For the three Hewson titles something better exists and is still not that: an archive
> relays the publisher's *"no objection"* to **its own** downloads, which is not a permission for
> screenshots taken by somebody else. [`docs/images/README.md`](docs/images/README.md) states all of
> it in full, with the quotations, and separates it from the 128 menu below — which *is* covered, by
> Amstrad's quoted ROM permission.

---

## The other machine, and the three minutes before a game appears

The 128 first, then the tape a game arrives on, then what arrived — which is the order they have
to happen in. All three are `zx-shot` output at scale 2, whole frames, nothing added.

![The 128's boot menu: a white panel on a grey field, the word 128 above a rainbow stripe of red, yellow, green and cyan, the five entries Tape Loader, 128 BASIC, Calculator, 48 BASIC and Tape Tester with the first highlighted, and the 1986 Sinclair Research copyright below.](docs/images/128-menu.png)

**The 128's own boot menu, unassisted.** The number of `--rom` arguments is what names the
machine — two make a 128, editor ROM first — so this needed no `--model` flag and no key at all.
The rainbow is the machine's signature and it is drawn by the ROM, not by anything here.

![Manic Miner mid-load: a black screen carrying the word MANIC in chunky letters of red, yellow, green, cyan and magenta, framed by a border of tightly alternating blue and yellow horizontal stripes.](docs/images/tape-loading.png)

**Three thousand frames into the tape.** Two things are worth naming. The stripes are the ROM's
`LD-BYTES` toggling the border as it decodes each bit, so they come out right only if the border
is modelled *below* frame resolution — written once per frame it would be a flat rectangle. And
`MANIC` is not a bitmap: it is 256 bytes loaded straight into the attribute file at `0x5900`,
which is why the letters are 8 × 8 blocks of flat colour.

![The Manic Miner title screen: a cyan sky with a yellow sunrise, a tree, a green hillside, a house and a red car, above a red banner reading MANIC MINER starring Miner Willy and PRESS ENTER TO START, above a drawn piano keyboard, with the Bug-Byte 1983 copyright scrolling below it.](docs/images/title.png)

**What the tape produced.** The same four keys, 7,750 frames later. The cavern above is one stop
on the attract loop this screen starts when the `ENTER` it is asking for does not arrive — and it
cannot arrive, because `zx-shot` presses every `--keys` tap *before* it starts the tape. **That
ordering was also the limitation, and it named its own remedy.** *Exolon* and *Cybernoid II* have
no attract loop to photograph, so for as long as every key landed before PLAY they could be shown
arriving and not playing; [`docs/images/README.md`](docs/images/README.md) recorded that as a
missing flag rather than a defect, and `--keys-after` is that flag. The two frames above are what
it bought, taken the same day. That page keeps the prediction standing rather than deleting it —
together with the two games that were tried and left out, and why a picture nobody can regenerate
would not have been worth having.

---

Not a port. Not a translation of an existing emulator. The CPU is implemented from the Z80
hardware specification and from our own architecture; correctness is then proven against
public conformance suites rather than against somebody else's source.

**The bar:** the emulator is not "done" until `zexall` passes — including the undocumented
flag behaviour that most emulators quietly skip.

> **What that green proves is narrower than the sentence reads, and this repository measured the
> gap rather than assuming it.** `zexall` reports 67/67 (M4), and it genuinely **is** sensitive to
> the undocumented `F3`/`F5` bits — forcing them to a constant `0` or `0x28` fails it, while a
> control mutation of a *documented* bit fails both exercisers, which is what proves the group is
> executed rather than skipped. But `zexall` **cannot decide the rule behind those bits**: the same
> 67/67 has been observed under three different implementations, including one whose flag latch was
> stuck at zero. Two FUSE vectors are the only gate in this project that can see that latch at all.
>
> So the bar is necessary and not sufficient, and the tempting one-line summary — *"`zexall`
> passes, so the undocumented flags are right"* — is true of the first claim and false of the
> second. [`docs/STATUS.md`](docs/STATUS.md) holds the coverage table that keeps them apart.

---

## Why this project

Emulating a Z80 is a rare kind of engineering problem: the correct answer is not a matter
of opinion. `zexall` either prints `OK` for all 67 tests or it does not. That makes it an
honest way to exercise the parts of Rust that matter — exhaustive `match` over an
instruction set, types that make an invalid state unrepresentable, and property tests over
arithmetic — with an external oracle instead of self-assessment.

---

## Status

| Milestone | Goal | Gate | State |
|---|---|---|---|
| M1 | Registers, flags, un-prefixed opcodes | FUSE vectors green for un-prefixed | **290/290** — merged |
| M2 | `CB` / `ED` / `DD` / `FD` prefixes | FUSE vectors green in full | **1045/1045** — merged |
| M3 | Documented behaviour | `zexdoc` passes | **67/67 first run** — merged |
| M4 | Undocumented flags | **`zexall` passes** — CPU complete | **67/67**, made a gate with its limits stated — merged |
| **M5** | Spectrum 48K: memory map, ULA, keyboard, 50 Hz interrupt | boots to `© 1982 Sinclair Research Ltd` | the machine boots, **on frame 87**, and **the gate work landed**: `crates/spectrum/tests/boot.rs` is a real `#[test]` that runs the ROM under `cargo test` and asserts the frame, alongside the other integration gates in that directory. Of the five mutations, **four were already red and one survived**. **Contention is no longer ungraded either**: `crates/spectrum/tests/timing_oracle.rs` grades the model against **70 rows measured on real Spectrums, 0 disagreeing**. See [`docs/STATUS.md`](docs/STATUS.md) for what M5's green does and does not mean, and [`docs/MACHINE.md`](docs/MACHINE.md) for the oracle's scope, which is narrower than the sentence sounds |
| **M6** | Snapshots (`.z80` / `.sna`) and tape (`.tap`) | **T1 + T2 + T3** — *not* "a real game runs" | **merged.** A program written here, stored as a `.tap`, loaded by the real ROM's own `LD-BYTES` through the `EAR` bit, and executed — computing a value asserted to appear **nowhere in its own bytes**, so that *"the data arrived"* and *"it ran"* are separate claims. Design in [`docs/M6.md`](docs/M6.md); what it opened and closed is in [`docs/STATUS.md`](docs/STATUS.md) |
| M7 | 128: paging, second ROM, AY-3-8912, **the beeper**, per-bank contention | **T1 + T2 + T3** — *not* "128-only software runs" | design in [`docs/M7.md`](docs/M7.md). **All five parts have landed**, each with its own gate among `crates/spectrum/tests/m7_*.rs`. **The memory half boots:** the 128 reaches its own `© 1986` copyright, draws all five menu entries, the highlight moves under `CAPS SHIFT`+`6`, and selecting *48 BASIC* reaches the `© 1982` message through ROM page 1 — the year changing is what makes that a claim about which ROM is executing. **The sound half is a device rather than a plan:** `crates/spectrum/src/ay.rs` is the AY-3-8912, reached from a guest's own `OUT`/`IN` in `m7_ay_ports.rs` and followed into the sample stream by `m7_ay_stream.rs`, and the beeper is bit 4 of a `0xFE` write, graded by value against a T-state derivation in `m7_beeper.rs`. What none of that settles is whether it **sounds** right — nobody has listened, and *What this is like at the moment* below says so first |
| M8 | WASM build | **T1 + T2 + T3** — a *build* gate, and ~~"playable from a URL"~~ cannot be one | **the browser build runs.** `wasm32-unknown-unknown` links, and on 2026-09-01 a served page booted the 48K to `© 1982 Sinclair Research Ltd` at **50.3 Hz, 0 dropped**, built a **128** from the query string alone, saved a `.z80` through a `Blob`, and restored it from a file dropped back onto the page. Each observation is recorded with its provenance in [`web/README.md`](web/README.md). Also landed: `crates/page` (the whole `unsafe` surface of this workspace, five blocks — `EXPECTED_BLOCKS` in `crates/page/tests/unsafe_inventory.rs`), a `bundled` feature that compiles a ROM and a game into one artefact, and drag-and-drop on both targets. Design in [`docs/M8.md`](docs/M8.md) |

> **Two corrections to the rows above, both made 2026-09-01 rather than left to be inferred from
> the design documents.**
>
> **The beeper joined M7 and left M8.** `docs/M6.md` had assigned *Sound — the speaker bit of a
> `0xFE` write* to M8, and `docs/M7.md` said in three places that M8 owned the audio device and
> the beeper with it. **The AY is a 128 device and the beeper is a ULA feature, so both are the
> machine's** — decoded by the same `Ula::out_port` that already takes the border out of the same
> byte — and `crates/spectrum` is where the machine is modelled. **What M8 owns is routing audio
> the machine already produces to a browser's audio device**: the mix, the resampling, the device.
> Four rows were corrected with the originals struck; `docs/M8.md` Decision 9 carries the ruling.
>
> **M8's gate is a build gate and the row's *"playable from a URL"* cannot be one.** *Playable* is
> not a property of an artefact — it is a property of a browser rendering a canvas, a GPU
> compositing it, a keyboard delivering keys and a person forming an opinion — so no corpus and no
> licence makes it automatable. This is the **third** milestone row corrected this way, and the
> reason differs from M6's and M7's: theirs were unautomatable because a corpus could not be
> committed, which another repository could fix, and M8's is unautomatable structurally.

> **Correction — the M5 row said *"the gate work is unfinished: five mutations leave it green and
> nothing yet runs it"*, and the repository falsifies both halves.** It is corrected loudly rather
> than quietly bumped, because how it survived is worth more than the row.
>
> *"Nothing yet runs it"* was already false when it was written. `crates/spectrum/tests/boot.rs` is
> a committed `#[test]`, landed with the M5 gates; `docs/STATUS.md` closes the item in as many
> words — *"nothing runs the boot gate — `crates/spectrum/tests/boot.rs` runs it."*
>
> *"Five mutations leave it green"* was never a verdict about a run that happened. Those mutations
> were graded against `crates/spectrum/examples/boot.rs`, which `cargo test` **builds and never
> calls**. Re-measured against the pre-gate lib target, **four of the five were already red** — 5,
> 7, 1 and 13 failing unit tests inside `src` — and **one survived**, the contention-phase
> off-by-one.
>
> **The propagation is the finding.** The pass that fixed the figure gives a whole section to *"a
> derived figure repeated across documents acquires authority it never earned"*, and states that
> the wrong number had spread into three documents. `docs/STATUS.md` and `docs/MACHINE.md` each
> carry a correction of it. This file did not — the repository's front door, and the only document
> a newcomer reads first. **The section diagnosing the propagation missed a copy of the thing
> propagating, and the copy it missed was the most-read one.**
>
> The rule that follows is cheap and absolute: *a correction is not landed until you have grepped
> for every other copy of what you corrected.* Every fact in these documents costs seconds to
> sweep across `docs/`, `README.md`, `CHANGELOG.md` and `testdata/`.
>
> > **Correction — this paragraph said the sweep *"did not come back clean"* and named
> > `docs/MACHINE.md:358` as still asserting *"five mutations left it green"*. Both halves are
> > false, and the second was pointed at the wrong line.** Re-swept 2026-09-01:
> > `grep -rn "five mutations" docs/ README.md` puts `MACHINE.md`'s only two hits at **`:616`**,
> > which states *"four of five mutations were already red"*, and **`:628`**, which is the block
> > retracting the old wording. `docs/MACHINE.md:358` is prose introducing the timing oracle and
> > has nothing to do with mutations. **The file this note was holding open had already closed
> > it** — so the note outlived the defect it named. That is the same failure one turn later: a
> > correction can go stale exactly as a claim can, and it is the harder one to catch, because it
> > wears the costume of a fix.
>
> **The row above deliberately carries no gate count**, and that is not vagueness — it is the same
> lesson applied one step earlier. That integer moved repeatedly on 2026-09-01 while this was
> being written — `docs/MACHINE.md` records the trail — so any number here would have been stale
> before the sentence containing it was, which is this exact defect for the third time.
> **Count the directory: the command below is the answer, and no integer written beside it can be.**
>
> ```sh
> ls -1 crates/spectrum/tests/*.rs | wc -l
> ```
>
> > **This is where that rule was broken by the paragraph teaching it.** The text above used to
> > carry the integers the command had returned that morning, plus a count of how many were
> > committed, and then asserted that *"`docs/MACHINE.md`'s milestone table says seven gates …
> > and is the number to check first."* Every one of those was stale within the day, and the
> > `seven gates` claim was already false when written — a nested note three lines below it said
> > so, so this file contradicted itself on screen and sent the reader to the wrong document.
> > `MACHINE.md`'s M5 row carries the command, not an answer. `grep -rn "seven gates" docs/`
> > now finds the live copies in **`docs/STATUS.md`**, in its M5 headings; those are that file's
> > to correct and are named here so they cannot go quiet.

> **Correction — the M6 and M7 rows said *"a real game runs"* and *"128-only software runs"*, and
> both name tier T4.** M6 has since merged; the row is corrected rather than quietly widened.
>
> [`docs/M6.md`](docs/M6.md) Decision 8 splits the milestone's evidence into four tiers: **T1**
> proven and corpus-free (the round trips, the truncation sweep, the codec property tests, the
> hand-transcribed vectors), **T2** measured (the real ROM's `LD-BYTES` loading a synthetic tape
> through the `EAR` bit), **T3** measured (a program *we wrote*, loaded from tape by the ROM and
> executed), and **T4** *observed* — a real game, a file of ours opening in somebody else's
> emulator. **The gate is T1 + T2 + T3.** T4 cannot be automated in a repository that may not carry
> games, and a milestone gated on it would be a gate that runs nowhere — which
> [`docs/STATUS.md`](docs/STATUS.md) records this project shipping three times already.
>
> **The residue is not absorbed:** T4 is the only tier that grades a turbo loader or a program
> written by somebody who did not know how this emulator works, and it runs nowhere. That is a row
> in the register, not a footnote here.
>
> `docs/MACHINE.md` and `docs/ARCHITECTURE.md` carry the same table; both are corrected. This file
> was the copy that was missed the last time a milestone row was corrected, which is why it is
> checked first now.

---

## Layout

```
crates/z80/         Z80 CPU core. No memory, no I/O, no allocation.
crates/spectrum/    The machine: paged memory, ULA, contention, keyboard, screen, timing,
                    sound (the beeper and the AY-3-8912), snapshots (`.z80`/`.sna`) and tape
                    (`.tap`/`.tzx`). This line lists the crate's remit, not its contents.
crates/frontend/    macroquad frontend: the `zx` window, and `zx-shot`, which photographs a
                    machine headlessly. Portable to `wasm32` — no `cfg(target)` anywhere, and
                    `tests/portability.rs` asserts that rather than leaving it to habit. ~~"but
                    that build has not been run"~~ — it has, and the page boots; see web/ below.
crates/page/        The browser page's half of the frontend's host seam: the query string and
                    the download. The **only** crate in this workspace that is not
                    `unsafe_code = "forbid"`, and its entire unsafe surface is five blocks, two
                    extern blocks and one attribute — a count `tests/unsafe_inventory.rs` asserts
                    as EXPECTED_BLOCKS, EXPECTED_EXTERN_BLOCKS and EXPECTED_UNSAFE_ATTRIBUTES.
web/                index.html, the vendored macroquad JS bundle, and the two scripts that
                    build and gate the browser build. `sh web/build.sh` assembles target/web/.
crates/testsupport/ The corpus-absence policy every gate shares. Test-only, never published.
docs/               Architecture, the machine design, the M6, M7 and M8 designs, the Z80
                    reference, the status register.
testdata/           Conformance suites, fetched locally. The Sinclair 48K ROM is the one
                    exception and is committed.
```

The CPU core does not own memory. That single decision shapes everything else: the machine
supplies a `Bus`, and the CPU reports every access as it happens — which is what makes
cycle-accurate contention possible. It landed at M5.

> **Reporting every access proved necessary and *not* sufficient, and M5 measured that.** The
> reasoning on record was that contention depends only on the address and the phase within the
> frame, both of which the machine already has. It does not follow: `LD A,B` and the read-modify
> half of `INC (HL)` emit **byte-identical** call streams — `read(addr)` then four `tick(addr)` —
> while owing one contention point and two. `crates/spectrum` first reconstructed the machine-cycle
> boundaries by deferral, in a 312-line file, at a residual it had to pin rather than fix. A
> defaulted `Bus::fetch` was added so each cycle discloses itself as it opens, and that file is
> deleted. The account is in [`docs/STATUS.md`](docs/STATUS.md) and
> [`docs/MACHINE.md`](docs/MACHINE.md).

---

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
[`testdata/README.md`](testdata/README.md). `testdata/games/` is gitignored and empty in a fresh
clone, and every command below that names a game is naming a file of **yours**.

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
gitignored, nothing in it is committed, and no permission here covers a game — so this is something
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
it is what produced every image on this page.

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
macOS, or anything that reads Netpbm. [`docs/images/README.md`](docs/images/README.md) has the full
recipes, including how each image on this page was taken and re-checked.

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
  [`docs/images/README.md`](docs/images/README.md), *What could not be photographed, and why*,
  under *"The other end of `--hold` is a threshold, not a number"*. `30` clears both edges, which
  is why every published command uses it. *(This entry read "`zx-shot` cannot press a key after the
  tape starts" and called the remedy a missing flag rather than a defect. The flag landed the same
  day; the two games at the top of this page that are shown playing rather than arriving are what
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
[`docs/STATUS.md`](docs/STATUS.md).

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

---

## Test data

`testdata/` is gitignored with explicit un-ignore rules, not wholesale. `git ls-files testdata/`
returns `.gitkeep`, `README.md` and the three Sinclair ROMs — `roms/48.rom`, `roms/128-0.rom` and
`roms/128-1.rom`. Those are the only *corpus* exceptions. Fetch the rest locally:

> **This said the command returned *"exactly `.gitkeep`, `README.md` and `roms/48.rom`"*, and M7
> committed the 128's pair.** `testdata/README.md` was corrected when they landed and this file was
> not — the propagation defect described above, happening this time to a **licensing** claim rather
> than a technical one, which is the class where the sentence *is* the thing being relied on. A file
> list that has gone stale reads exactly like a file list that is complete, so it fails silently in
> the direction of claiming less than the repository actually redistributes. Run the command; do not
> trust this paragraph either.

| Directory | Contents | Committed? |
|---|---|---|
| `testdata/fuse/` | `tests.in`, `tests.expected` — 1335 per-instruction vectors | no — belongs to the FUSE project |
| `testdata/zex/` | `zexdoc.com`, `zexall.com` | no — third-party conformance binaries |
| `testdata/roms/48.rom` | the Sinclair 48K ROM | **yes** — under the permission quoted in `testdata/README.md`, which is also where the SHA-1, the CRC-32 and the fetch commands are. A subtly wrong ROM is the one corpus failure no harness here would explain |
| `testdata/snapshots/`, `testdata/tapes/`, `testdata/timing/` | third-party `.z80`/`.sna`, `.tap` and the 48K timing suite | no — fetched; each is the *external* check on our reading of a format, or of the machine's timing |
| `testdata/roms/128-0.rom`, `128-1.rom` | Sinclair 128 editor + 48 BASIC | **yes** — committed at M7, on the same permission, which names *"Spectrum 48/128"* outright. Sizes, SHA-1s and CRC-32s are in `testdata/README.md`, taken from the committed bytes. Adding a *further* ROM stays a **deliberate** act: `.gitignore` un-ignores each one by name |

> **The 128-ROM row said they *"will be committed by default when added: the un-ignore rule is
> `!testdata/roms/*.rom`, not one path"*, and `.gitignore` no longer works that way.** The glob was
> replaced by one explicit filename per ROM, so a new ROM is ignored until somebody edits
> `.gitignore` — which is where a reviewer sees it and where the licence question has to be answered
> rather than assumed. The reason it mattered is not that some *Sinclair* ROM has a different
> licence: it is that `*.rom` also accepts a game cartridge, a Multiface image, or an Interface 1
> ROM, and the permission this repository relies on **disclaims** the Interface 1 and 2 ROMs
> outright. The glob turned "may we redistribute this?" from a decision into an accident.

Game images are never distributed here. Supply your own.

---

## Engineering rules

These are enforced, not aspirational:

- **Our own implementation.** No code, no opcode tables and no timing tables copied or
  adapted from another emulator. Behavioural questions are resolved from hardware
  documentation, and the rule is named in a comment at the site.
- **`unsafe_code = "forbid"`** in every crate **except one**. This machine runs a 3.5 MHz CPU
  thousands of times faster than real time, so `unsafe` would buy nothing on speed — and the
  exception is not about speed. `crates/page` holds the browser FFI, where `unsafe` is
  unavoidable in kind rather than optional for performance, and it is confined to a crate instead
  of an `#[allow]` because `forbid` cannot be overridden from inside. *(This line read "in every
  crate", full stop, for several milestones after that stopped being true.
  [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) corrected its own copy of the same absolute —
  "the worst line in this document to have carried a stale absolute" — and this one, the front
  door and the only document a newcomer reads first, was missed: the *grep for every other copy*
  rule failing on its own most-read instance. The exception is checkable rather than asserted, which is what
  keeps the rule a rule: `crates/page/tests/unsafe_inventory.rs` pins the surface in
  `EXPECTED_BLOCKS`, `EXPECTED_EXTERN_BLOCKS` and `EXPECTED_UNSAFE_ATTRIBUTES`.)*
- **`overflow-checks = true` in release too.** The Z80 is *meant* to wrap, so every wrap is
  written as an explicit `wrapping_*` call. A silent wrap is a bug, and debug must behave
  the same as release.
- **Latest stable dependencies only**, verified against crates.io. No `*` constraints.
- **No trait objects on the execute path.** `Cpu<B>` is generic and monomorphised. *(This line
  said `Cpu<B: Bus>`. The declaration is `pub struct Cpu<B>` — the `Bus` bound sits on the `impl`
  blocks, not on the struct, and its removal is recorded as closed in `docs/STATUS.md`: a
  struct-level bound forces `where B: Bus` onto every downstream type that merely **names** a
  `Cpu`. The substantive claim is unchanged and is checked in the emitted code rather than
  asserted — the commands are in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)'s *Measured*
  section.)*

Three rules govern the documents rather than the code, and they are siblings rather than one rule
stated three times. The first is written down where it was learned — *a correction is not landed
until you have grepped for every other copy of what you corrected* — and it covers the half a
machine can settle. The second covers the half it cannot: **a citation is not written until its
target has been read.** The third decides what counts as a *copy*, which is what the first one needs
before anybody can act on it: **does the surrounding sentence stay true when the list or the figure
goes wrong?** If it does, the list is a second copy of something the sentence already carries whole,
and it should become a complement or a reference. If the sentence becomes unverifiable without it,
the list **is** the content and it stays. That test deletes and protects about equally, which is
what makes it a rule rather than a tidying habit: out go a roster of crate names standing beside
*"the only crate that is not"* and two ordinals sitting a line above the constants they restated,
while the enumeration of `unsafe` syntax forms in `crates/page` stays — it is the operative
definition of what the sweep matched — and so does *"two mitigations, and they fail differently on
purpose"*, because the claim that they differ cannot be checked without naming them and no third can
be added without touching the numeral.

Twelve false citations were audited here and each was classified at the commit that introduced it,
by `git log -S` rather than by inference: five were wrong when written, seven were broken by
drift. The two classes need different remedies. Every drift case had a checkable subject — a
function that still exists, a constant that was deleted — so carrying an anchor instead of a bare
line number turns it into something a `grep` settles later, and a gate can be built for that. Not
one of the five would have been caught by a better citation format, because each named something
that had never existed: an invented test function, an aspirational CI job, a test's scope nobody
had read, a line number for a sentence quoted out of a third file. Anchors do not rescue a
citation whose referent is imaginary. Only opening the target does, once, at the moment of
writing.

The second half is the one this repository keeps failing, and the evidence is not a guess.
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) said `crates/frontend/tests/portability.rs`'s
`this_crate_still_forbids_unsafe` guarded four crates, when `include_str!` resolves one path and
the test's own name says *this crate* — and that sentence was introduced **during the pass that
was fixing a different false citation**, by careful writing. Prose summarising a gate drifts
upward by default: the honest description of what a test checks is always narrower and duller than
the reason it was written, and nothing in a green run ever contradicts the flattering version. So
open the file, read the assertion, then write the sentence — never the other order.

---

## Documentation

| Document | Contents |
|---|---|
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate boundaries, the five load-bearing design decisions, the 48K↔128 differences, milestones, and the *Measured* section — every performance and codegen claim with the command that produced it |
| [`docs/MACHINE.md`](docs/MACHINE.md) | The machine from M5 onward: why it owns the clock, the ULA as the second clock, the verification plan, and **the timing oracle** — 70 rows measured on real Spectrums, and the precise statement of what its green does and does not settle |
| [`docs/M6.md`](docs/M6.md) | The M6 design — `.z80`/`.sna` snapshots and `.tap` tape. Each decision carries its class of evidence, plus **ruling** for a choice no evidence forces. *This table and the `docs/` line in Layout both omitted it while the file existed and ran to hundreds of lines; nothing in the repository linked to it at all.* |
| [`docs/M7.md`](docs/M7.md) | The M7 design — the 128: paging, the second ROM, the AY-3-8912, the beeper, per-bank contention. **All of it has landed and is on `main`** — the 128 boots, the AY answers a guest's own `OUT`/`IN` and carries into the sample stream, the beeper is graded by value against a T-state derivation, and `crates/spectrum/tests/m7_*.rs` are the gates. Its gate is T1+T2+T3 on the same four-tier scheme M6 uses. *(This row said **"in flight and unmerged"**, which stopped being true when the M7 memory commit merged — `git diff main..m5-followup` is empty.)* |
| [`docs/M8.md`](docs/M8.md) | The M8 design — the browser: the build target, the query string as `argv`, the `Blob` download, drag-and-drop, the vendored bundle, and the arrow schemes. Thirteen decisions in the same evidence vocabulary, and **Decision 7 is the one to read first** — it is where *"playable from a URL"* is shown not to be a property an artefact can have, and so not a thing any gate here can assert |
| [`docs/Z80-REFERENCE.md`](docs/Z80-REFERENCE.md) | Register file, flag semantics, the undocumented 3/5 bits and register Q, `DAA`, instruction timing, prefix traps, interrupts |
| [`docs/STATUS.md`](docs/STATUS.md) | **What is currently true**, and the only register of what is open. Also the catalogue of this project's own failures — read it before trusting a number anywhere else |

> **Correction — the `MACHINE.md` row described it as *"the verification plan for a layer that has
> **no oracle**"*, and that stopped being true on 2026-09-01.** It was accurate as history and
> misleading as a description: `crates/spectrum/tests/timing_oracle.rs` grades the 48K contention
> model against 70 rows measured on real Spectrums, with 0 disagreements, bounded by sixteen
> mutations — `FIRST_CONTENDED_T_STATE` at 14333, 14334, 14336 and 14337 all red, only 14335 green,
> and the last three arriving with group 35, the first row here to reach the I/O rule's **fourth**
> term at all. Those three are three separate edits to one `match` arm, not one edit read three
> ways: deleting the arm and weakening it to a two-stall shape each redden 2 of the 70 rows, while
> dropping its fourth term alone reddens 1. **This figure read *fourteen* until 2026-09-01**, when
> the three were re-measured together in one scratch clone; the count is settled in
> [`docs/MACHINE.md`](docs/MACHINE.md)'s mutation table, which is where it should be cited from.
>
> **State the scope, because the obvious reading is too strong.** What is established is that **the
> first contended T-state falls exactly 14335 T-states after `/INT`**, given that this machine
> asserts `/INT` at frame T-state 0. Three things stay open and each is its own row in
> `docs/STATUS.md`'s register: the frame's **origin** is a convention (moving `/INT` and the window
> together leaves the oracle green), the interrupt window's **length** is unmeasured (32 → 24 leaves
> it green), and the `64 × 224` **factorisation** is not measured — its *product* is.
>
> The floating bus, progressive drawing and keyboard ghosting remain unmodelled, so they are not
> gradeable rather than ungraded, and the *observation* half of that plan is unchanged for them.
> **`docs/M6.md`'s opening and `crates/spectrum/src/lib.rs`'s coverage table carried the same
> superseded claim**; the first is corrected, and the second is another agent's file and is named
> here so it cannot go quiet.

Claims in these documents are labelled by the kind of evidence behind them — proven, measured,
derived, or observed — and a claim whose evidence has expired is corrected in place, loudly, rather
than deleted. If you find one that is stale, that is a defect report, not a tidy-up.

---

## Licensing

The emulator is MIT OR Apache-2.0.

**Amstrad have kindly given their permission for the redistribution of their copyrighted
material but retain that copyright.**

> **That note now has two more homes, and both were added because the permission asks for
> *"the program/manual"* and the reader of a shipped artefact sees neither this file nor
> `testdata/README.md`.** For a page served from a URL, **the page is both** — so it is visible
> text under the canvas in `web/index.html`. For a `--features bundled` binary, **the window is
> both** — so it is a permanent line under the picture that `F1` does not hide, and `zx-shot`
> prints it. In the binary it appears only when the embedded ROM actually contains the bytes
> `Sinclair`, because printing Amstrad's notice over a ROM they did not write would be a false
> statement in the one place this repository is most careful not to make one.
>
> **A game is covered by none of this.** `--features bundled` embeds whatever it is pointed at,
> and what it is pointed at is never committed: games live in gitignored `testdata/games/`, the
> feature is off by default, and a default build neither needs a game nor looks for one.

That sentence is not a paraphrase. The permission — Cliff Lawson of Amstrad plc, posted to
`comp.sys.sinclair` on 31 August 1999 — asks in those words that *"the program/manual includes a
note to the effect that"* it, and this is that note. `testdata/roms/48.rom` is committed on its
strength; the Spectrum ROMs may be placed in `testdata/roms/` locally. Game images may not be
redistributed — they are yours to supply.

**The quotation, the author, the date, the four conditions, the hedged scope, and the one gap
still open in the sourcing are in [`testdata/README.md`](testdata/README.md) and only there.** This
section used to assert *"Amstrad has explicitly permitted redistribution of the Sinclair ROMs
alongside emulators"* and stop — one of five copies of a licensing claim carrying no quotation, no
author, no date and no URL between them. A licensing claim is the one kind where the quotation
**is** what is being relied on, so it has one home and everything else points at it. Read alongside
it: the scope is narrower and more hedged than the old sentence implied — the ZX80, the ZX81 and
the Interface 1 and 2 ROMs are disclaimed as **not Amstrad's copyright at all**, which is why
`.gitignore` names every committed ROM individually.
