# Images

Seven files, referenced from the repository's [`README.md`](../../README.md). They are **produced
by running the emulator**, not drawn, and this page is the commands that produce them — so that a
picture making a claim about the machine can be re-taken rather than trusted.

| File | 640 × 512 | What it is |
|---|---|---|
| `cybernoid.png` | the hero | *Cybernoid* playing, on the **128**, restored from a `.z80` snapshot |
| `central-cavern.png` | gallery | *Manic Miner* playing, on a 48K, loaded from tape by the ROM |
| `cybernoid-ii.png` | gallery | *Cybernoid II*'s screen arriving from tape, mid-load |
| `exolon.png` | gallery | *Exolon*'s screen — which was never on the tape as a screen |
| `128-menu.png` | gallery | the 128's own boot menu, unassisted |
| `tape-loading.png` | gallery | the tape three thousand frames in, mid-load |
| `title.png` | gallery | what that tape produced |

Four games: *Cybernoid*, *Manic Miner*, *Cybernoid II*, *Exolon*. Two machines. One of the seven
frames is a **48K**, one is a **128**, and the rest are named per command below.

---

## Which pixels are the machine's

**All of them, in all seven files. There is no surround, no margin, no gutter and no caption
baked into any image**, so the question the first version of this page had to argue — *where
does the emulator's output stop?* — does not arise. Each file is one 320 × 256 frame and nothing
else.

Every pixel reached the file through the path the window runs: `Spectrum::render` into a
`spectrum::Frame`, then `frontend::palette::write_rgba`. The binary that writes it is `zx-shot`,
which differs from `zx` only in the last step — it emits
[`frontend::ppm`](../../crates/frontend/src/ppm.rs) bytes where the window uploads a texture, and
`crates/frontend/tests/ppm_encoding.rs` asserts those are the same buffer. A screenshot produced
by code the window does not run would prove nothing about the window.

### That claim is decidable, and here is the decision procedure

[`crates/spectrum/src/screen.rs`](../../crates/spectrum/src/screen.rs)'s `Colour::rgb` emits
exactly three channel levels — `0x00`, `0xD7` and `0xFF` — and one further constraint that is
easy to miss: a colour's non-zero channels are *all at the same level*, because `gun` is handed a
single `level` for all three. So the ULA can emit **fifteen** RGB values and no others, and
`[0xD7, 0xFF, 0x00]` is as impossible as `[0x3C, 0x3C, 0x3C]`.

That makes *"every pixel is the machine's"* a property a program can check rather than a promise
a reader has to accept:

```sh
for f in docs/images/*.png; do
  pngtopam "$f" | ppmhist -noheader | awk -v f="$f" '
    { bad = 0; lvl = 0
      for (i = 1; i <= 3; i++) {
        v = $i
        if (v == 0) continue
        if (v != 215 && v != 255) { bad = 1; break }
        if (lvl == 0) lvl = v; else if (v != lvl) { bad = 1; break }
      }
      if (bad) { printf "  FOREIGN rgb(%d,%d,%d) x%d\n", $1, $2, $3, $5; n++ }
      total++ }
    END { printf "%s: %d distinct colours, %d foreign\n", f, total, n; exit (n > 0) }'
done
```

Run 2026-09-01, over all seven:

```
docs/images/128-menu.png: 7 distinct colours, 0 foreign
docs/images/central-cavern.png: 12 distinct colours, 0 foreign
docs/images/cybernoid-ii.png: 15 distinct colours, 0 foreign
docs/images/cybernoid.png: 12 distinct colours, 0 foreign
docs/images/exolon.png: 12 distinct colours, 0 foreign
docs/images/tape-loading.png: 9 distinct colours, 0 foreign
docs/images/title.png: 11 distinct colours, 0 foreign
```

`cybernoid-ii.png` uses **fifteen**, which is every value the hardware has.

### The checker was broken on purpose, three times, because a green that cannot go red proves nothing

A checker is worth exactly what its failure mode is worth, and this repository keeps finding
gates that graded less than they appeared to. Each mutation below **prints the pixel before and
after**, and *asserts that the value actually changed*, because a substitution that silently
matches nothing and a guard that cannot fail produce the identical output: exit 0. The assertion
is in the mutating program, not in the reader's head.

| Mutation | One pixel set to | In | Result |
|---|---|---|---|
| A foreign surround | `rgb(60,60,60)` — `#3C3C3C`, the grey the first version of this page used for its margin | `exolon.png` | `FOREIGN rgb(60,60,60) x1`, **exit 1** |
| A resampling artefact | `rgb(108,108,0)` — the midpoint of black and yellow, which is what a non-integer rescale puts on the edge of a letter | `cybernoid.png` | `FOREIGN rgb(108,108,0) x1`, **exit 1** |
| **An impossible ULA mix** | `rgb(215,255,0)` — **every channel is a legal level**, and the colour is still unreachable, because `gun` hands one `level` to all three | `cybernoid-ii.png` | `FOREIGN rgb(215,255,0) x1`, **exit 1** |

The unmutated files, through the same checker, still report `0 foreign` and exit 0.

**The second mutation is the one that matters and the third is the one that was missing.** The
hazard this page exists to exclude is an image being **resampled** somewhere between `zx-shot` and
a browser, which does not announce itself and is only visible as softness; a blend value is what
that leaves behind, and mutation two is that. But mutations one and two only ever tested the
*set* of channel levels — a checker that validated `{0, 215, 255}` and stopped would have passed
both and still been wrong, because it would accept `rgb(215,255,0)`. Mutation three is the first
one that tests the **same-level** half of the constraint, which is the half the prose above calls
easy to miss. It was easy to miss here too: two mutations had been standing as proof of a
two-part rule.

### Nothing was retouched and nothing was resampled

The two claims are checked separately rather than asserted together:

```sh
# `zx-shot` is deterministic: every frame reproduces byte-for-byte on a re-run
cmp first-run/cybernoid.ppm second-run/cybernoid.ppm

# and the PNG is exactly the frame enlarged, with nothing else done to it
pamenlarge 2 cybernoid.ppm > enlarged.ppm
pngtopam docs/images/cybernoid.png | pamtopnm > back.ppm
cmp enlarged.ppm back.ppm
```

Run on 2026-09-01 for **all seven**, from one binary, and every `cmp` was silent. That is the
stronger of the two available checks and it is worth naming why: the four older files were taken
by an earlier build, so re-running their commands now and getting the published bytes back shows
that **the commands on this page still produce the files in this directory** — not merely that
each file was honest when it was made. `pamenlarge` replicates each pixel into an N × N block: it
does not filter, interpolate or average, so every pixel of the PNG is a bit-for-bit copy of a
pixel `palette::write_rgba` wrote. `pamtopng` is lossless and round-trips back to the same P6,
which is what the second `cmp` shows.

---

## Scale

**Scale 2, by `pamenlarge`, and 2 is the largest integer that GitHub will not then undo.**

A Spectrum pixel is a square of solid colour, so a non-integer resample is the one thing that
would visibly damage this subject — [`crates/frontend/src/viewport.rs`](../../crates/frontend/src/viewport.rs)
refuses fractional scaling in the window for the same reason. GitHub gives a README roughly 850 px
of width and scales anything wider down to fit:

| Scale | Width | What GitHub does |
|---|---|---|
| 1 | 320 | nothing — but a 320 px frame is small, and a pale one is unreadable |
| **2** | **640** | **nothing. Chosen** |
| 3 | 960 | scales by ~0.885 — a non-integer reduction, strictly worse than not enlarging |

The cost is stated rather than hidden: at scale 2 a browser on a high-density display enlarges the
file again by 2, and that step is outside this repository's control. On a 1× display the image is
exact.

**Scale 1 was measured and rejected, and the measurement is the reason the composition changed.**
An earlier version of this page put two frames side by side at scale 1. Both were ROM screens — a
BASIC report and the 128 menu — and the ROM's own screens are a white field with a line of small
black text. At 320 px they are legible only in principle. Photographed at scale 2 and looked at,
the 48K BASIC frame is a grey rectangle with eleven characters in the middle of it; it was dropped
rather than enlarged, because enlarging a dull picture makes a larger dull picture.

---

## The commands

Taken 2026-09-01, from `crates/frontend`'s `zx-shot`, built at commit `1945c2e` plus the
working tree of that day. Every image below is one command; none was composed, cropped or
retouched, and the `--settle` numbers are the whole of what distinguishes several of them.

> **A `--settle` number is a reading, not a constant, and the tape model was rewritten underneath
> these within the hour.** The timeline is recorded rather than summarised, because *"taken on
> 2026-09-01"* would hide the part that matters:
>
> | Time (2026-09-01) | What happened |
> |---|---|
> | 18:58 | `zx-shot` built — **the binary that took every picture in this directory** |
> | 19:14 | the three new frames written |
> | 19:17 | `cargo test --workspace --no-fail-fast` green, and **this is the tree the pictures came from** |
> | 19:20 | `crates/spectrum/src/tape/` rewritten by another change — `signal.rs`, `tap.rs`, `reader.rs`, and a new `tzx.rs` |
> | 19:26 | all seven commands re-run against the 18:58 binary; every `cmp` silent |
> | 19:28 | `zx-shot` **rebuilt over the rewritten tape** |
> | 20:38 | all seven re-run against *that* binary — **every `cmp` still silent** |
>
> **The last row is the one that was in doubt, and it came back clean.** A tape frame index is a
> function of how the tape is replayed, the replay was rewritten at 19:20, and the honest
> expectation was that some of these numbers would move. None did: the same seven commands produce
> the same seven files across both builds. That is a fact about *these* two builds and it does not
> generalise — if a command on this page ever lands somewhere else, **the number has moved, not the
> picture**. Nothing about any frame was hand-chosen, so the fix is to re-read the number rather
> than to argue with it.
>
> The way to re-read one is a sweep, which is how every number here was found in the first place:
> run the same command over a spread of `--settle` values, tile the frames, and look. The
> composition step is one line of netpbm —
>
> ```sh
> for s in 11000 11250 11500 11750 12000 12250; do
>     $ZX --rom testdata/roms/48.rom --media testdata/games/ManicMiner.tap \
>         --keys "$LOAD" --play-tape --settle $s --out /tmp/f$s.ppm
> done
> pamcat -lr -jtop /tmp/f*.ppm | pamtopng > /tmp/sheet.png
> ```
>
> — and it is cheap: the twenty-four-value *Exolon* sweep that found `2500` took **10.8 s of wall
> clock** and 68 s of CPU across eight parallel runs on 2026-09-01, so a tape load is a second or
> two. A contact sheet is a working artefact and is never published: every file in this directory
> is a single frame from a single command.

```sh
cargo build --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot

ZX=./target/release/zx-shot
LOAD='J;LeftControl+P;LeftControl+P;Enter'          # LOAD "" — see the table below

# --- the hero: Cybernoid, playing, on the 128 -------------------------------
# Two --rom make a 128, editor ROM first. The snapshot lands before any frame runs,
# so `1` — START GAME on the game's own menu — is pressed into a machine that is
# already holding the game.
$ZX --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom \
    --media testdata/games/Cybernoid.z80 --frames 120 --keys 'Key1' --settle 600 \
    --out cybernoid.ppm

# --- Manic Miner, playing, loaded from tape by the ROM ----------------------
$ZX --rom testdata/roms/48.rom --media testdata/games/ManicMiner.tap \
    --keys "$LOAD" --play-tape --settle 11750 --out central-cavern.ppm

# --- Cybernoid II: the screen has landed, the game is still coming ----------
$ZX --rom testdata/roms/48.rom --media testdata/games/CybernoidII.tap \
    --keys "$LOAD" --play-tape --settle 5000 --out cybernoid-ii.ppm

# --- Exolon: the picture is finished and the ROM is listening for the next block
$ZX --rom testdata/roms/48.rom --media testdata/games/Exolon.tap \
    --keys "$LOAD" --play-tape --settle 2500 --out exolon.ppm

# --- the 128's own boot menu; no keys at all --------------------------------
$ZX --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom --frames 120 \
    --out 128-menu.ppm

# --- the same Manic Miner tape, earlier and later ---------------------------
$ZX --rom testdata/roms/48.rom --media testdata/games/ManicMiner.tap \
    --keys "$LOAD" --play-tape --settle 3000 --out tape-loading.ppm
$ZX --rom testdata/roms/48.rom --media testdata/games/ManicMiner.tap \
    --keys "$LOAD" --play-tape --settle 10750 --out title.ppm

for f in cybernoid central-cavern cybernoid-ii exolon 128-menu tape-loading title; do
    pamenlarge 2 $f.ppm | pamtopng > docs/images/$f.png
done
```

`pamenlarge` and `pamtopng` are [netpbm](https://netpbm.sourceforge.net). Neither scales, filters
nor interpolates, so neither can alter a machine pixel; the `cmp` above is what shows that rather
than the sentence.

**The game files are yours to supply and are named here as they were on the machine that took
these.** Nothing in `testdata/games/` is committed — `git check-ignore -v testdata/games/` is the
proof, and [`testdata/games/PROVENANCE.md`](../../testdata/games/PROVENANCE.md) carries it. SHA-1
of the four used above, taken from the bytes on disk on 2026-09-01, so that somebody with the same
files can tell whether they have the same files:

```
1c7df3e9be2d57ae0f094d9d426898475bf82ca3  Batty.tap            (rejected — see below)
e4cca809aed052fbc04ac52222e593a41190f9cc  Cybernoid.z80
aed4265e8253fa01fab9f151637528084a596162  CybernoidII.tap
2678271cccc4b2485a8fe5ea05c3ac781e1f421b  Exolon.tap
84808c20566aa65e9308c3f8910a16bacfa1b982  ManicMiner.tap
```

### The four taps of `LOAD ""`, and the keymap's documented limits

**No binding was added to make this easier**:

| On screen | Typed as | Why |
|---|---|---|
| `LOAD` | `J` | A 48K starts a line in `K` mode, where `J` *is* the `LOAD` token — one keypress, not four letters |
| `"` | `LeftControl+P` | `SYMBOL SHIFT`+`P`. `"` is a *shifted* key on a PC, so the keymap deliberately leaves it to the hardware route |
| `"` | `LeftControl+P` | again — `LOAD ""` is the empty filename, so the loader accepts whatever block arrives |
| — | `Enter` | submits the line, and is the last key any of the tape pictures involves |

### `--play-tape` is the one thing `--keys` cannot express, and it is not mine

`media::insert` puts a tape in the drive **stopped**; the window starts it with `F3`. `F3` is a
`keymap::Hotkey`, not a membrane key, and `keymap::apply` only ever closes switches on the
Spectrum's own keyboard matrix — so no `--keys` script can spell *"press play"* however it is
written, and without it the ROM's `LOAD ""` sits polling an `EAR` bit that never moves. The flag
was added to `zx-shot` by another change on this branch, not by this one.

---

## What each frame is showing, which is more specific than "a game"

### `cybernoid.png` — a 128 game, and the only route to a played frame this tool has

The `.z80` is restored by `media::insert` **before the first frame runs**, so the machine is
already holding *Cybernoid* when `--keys` starts pressing. That is the whole reason this is a
frame of a game being *played*: `Key1` selects `1 START GAME` from the game's own menu, and the
600 frames of `--settle` after it are the game running with nobody at the keyboard. The ship sits
where it was left; everything else on screen is the game moving.

**A tape cannot be photographed this way, and the ordering is why.** `zx-shot` runs `--frames`,
then every `--keys` tap, then presses PLAY, then `--settle`. Every key therefore happens *before*
the tape starts, and a tape that takes ~9,244 frames to load finishes long after the last key has
been released. See *What could not be photographed* below.

### `cybernoid-ii.png` — the screen was on the tape, as a screen

`CybernoidII.tap` is a BASIC loader (672 bytes), a 200-byte `Code` block headed for 25000, and
then **two blocks with no header at all**: one of **6,912** bytes and one of **40,191**. 6,912 is
exactly a Spectrum screen — 6,144 bytes of pixels and 768 of colour — and it is why the picture
can be watched filling in. The border is still striped blue and yellow in this frame because the
other 40,191 bytes are still arriving.

The stripes are the ROM's, not the game's: `LD-BYTES` toggles the border as it decodes, which
means they only come out like this if the border is modelled **below** frame resolution. Written
once per frame it would be a flat rectangle.

The screen is signed `HUGH` in its lower right corner — which the game's own title screen, further
along the same tape, credits as *"GRAPHICS BY HUGH BINNS"*. Both are legible in this repository's
own output, which is a small piece of evidence that the 6,912 bytes arrived intact rather than
merely plausibly.

### `exolon.png` — the picture was *not* on the tape

`Exolon.tap` carries three files and **not one of them is a screen**: `EXOLON`, 125 bytes of
BASIC; `exolon$`, 5,339 bytes of `Code` to address **27000**; and `exolon`, 28,601 bytes of `Code`
to **28000**. Nothing is 6,912 bytes and nothing is aimed at 16384, where the display file lives.
The picture is therefore painted by code that arrived in the second block — and that is
observable, not just inferred:

| `--settle` | Border | Screen |
|---|---|---|
| 2 320 – 2 380 | blue / yellow | black — the ROM is reading `exolon$` |
| **2 400** | **black** | **half-drawn** — the loader has stopped and code is painting |
| 2 420 | plain white | finished |
| **2 500** | **red / cyan** | finished — **the published frame** |
| 2 540 onward | red / cyan, then blue / yellow again | finished, while `exolon` streams in |

**The black border at 2 400 is the tell.** The ROM's loader never leaves the border alone; a black
border means no load is in progress, so whatever drew those rows was the machine executing, not
bytes landing in the display file.

**And the two stripe colours are two different parts of `LD-BYTES`.** Red and cyan is the ROM
listening for a block's pilot tone; blue and yellow is the ROM reading data. Both are visible in
this set — `exolon.png` is red and cyan, `tape-loading.png` and `cybernoid-ii.png` are blue and
yellow — which is the evidence for the sentence rather than a claim about a disassembly.

### `tape-loading.png` — the letters are the attribute file

The Manic Miner tape's third block carries 256 bytes to `22784` = `0x5900` — attribute rows 8–15,
not display memory. That is why `MANIC` is built from 8 × 8 blocks of flat colour: they are
character-cell attributes with nothing but solid ink underneath. The block table is in
[`testdata/games/PROVENANCE.md`](../../testdata/games/PROVENANCE.md).

### `central-cavern.png` and `title.png` — nothing was pressed after the load

The only keys in either run are the four of `LOAD ""`, all of them **before** the tape starts.
After the load completes the title screen appears, and *Manic Miner*'s own attract sequence tours
the caverns while it waits for an `ENTER` that never came. Sampled across that tour on 2026-09-01,
one frame per `--settle` value:

| `--settle` | On screen |
|---|---|
| 3 000 | mid-load: the border stripes, the `MANIC` art |
| 10 500 – 11 500 | the title screen, its instruction line scrolling |
| **11 750** | **Central Cavern** |
| 12 000 | The Cold Room |
| 12 250 – 12 500 | The Menagerie |
| 12 750 | Abandoned Uranium Workings |
| 13 000 | Eugene's Lair |
| 13 250 | Processing Plant |
| 13 500 – 13 750 | The Vat |
| 14 000 | Miner Willy meets the Kong Beast |
| 14 250 | Wacky Amoebatrons |
| 14 500 | The Endorian Forest |
| 15 000 | Attack of the Mutant Telephones |
| 15 500 | Ore Refinery |
| 16 000 | The Bank |
| 16 500 | The Sixteenth Cavern |
| 17 000 | Amoebatrons' Revenge |
| 17 500 | Solar Power Generator |

The cavern changing roughly every 250 frames with no key pressed is what identifies this as the
attract loop rather than a game somebody started.

---

## What could not be photographed, and why

Recorded because a gallery that shows only what worked is a gallery with an unstated denominator.

**A tape-loaded game cannot be started.** `zx-shot` presses every `--keys` tap *before* it presses
PLAY, so no key can reach a game that arrives minutes later. *Exolon* and *Cybernoid II* both load
correctly and then sit on their own menus waiting for a key that this tool has no way to send:
*Cybernoid II* was observed cycling title ↔ hall of fame out to `--settle 160000` — about 53
minutes of emulated time — and *Exolon* held its loading screen out to `--settle 175000`. Neither
has an attract mode, which is why *Manic Miner*, which does, is the only tape game here shown
playing. **This is a missing flag, not a defect**: something that lets keys be pressed after
`--play-tape` (a `--keys-after`, or interleaving `--keys` with `--settle`) would put both games in
this gallery. It has been reported rather than added — `crates/frontend` belongs to other changes
in flight.

**The ordering also costs the *snapshot* games nothing, which is the shape of the fix.** A `.z80`
lands before frame zero, so `cybernoid.png` gets its keypress and is the one game here shown
playing that did not need an attract loop. Everything a tape game is missing is the same
capability, half a run later.

**`Mario Bros` is a `.tzx`, which stopped being a reason at 19:41.** This section said, correctly
at 19:14, that nothing here loads a `.tzx` and quoted `media::unsupported` saying so. **`.tzx`
support merged at 19:41 the same evening** — `NOT_YET` is now an empty table — and the game loads
on the first try. The paragraph is corrected rather than quietly deleted, because a claim that
expired inside ninety minutes is the clearest possible argument for dating them.

What it loads into is worth recording, and it is *not* a picture:

- The tape is **four files, every one a standard-speed `0x10` block**: `MARIO`, 86 bytes of BASIC;
  `scr`, 6,912 bytes of `Code` to **40000**; `cm`, 35,328 bytes of `Code` to **25088**. So this
  particular `.tzx` exercises the container and not turbo loading — a `.tap` could have carried the
  same bytes.
- All four arrive, the loading screen draws, and the game then paints `OCEAN PRESENTS` — which
  means **the game's own code is executing**, not merely resident.
- And there it stays, out to `--settle 140000`: roughly **47 minutes** of emulated time.

**Whether that is a game waiting for a key or a defect cannot be decided with the tool as it
stands, and that is the point.** `zx-shot` cannot press a key after a load, so *"it does not
advance"* and *"it is waiting for input"* produce identical evidence. `zx-shot`'s own source
already names this failure mode, about `--hold`: *"every 'the game does not respond' is ambiguous
between a defect in the emulator and a tap this tool made too short, and an ambiguous report is the
expensive kind."* The same sentence applies one level up, to the key that cannot be sent at all.
This is the second and stronger reason for the missing flag above: the first was a nicer gallery,
this one is a diagnosis nobody can currently make.

**`Batty` was rejected on provenance, not on looks.** The copy to hand
(`1c7df3e9be2d57ae0f094d9d426898475bf82ca3`) is a cracked release: it loads to a full-screen
advertisement for a warez site and stops there. That is not a picture of the machine running a
game, and publishing it would attribute somebody's crack screen to this emulator.

**The 48K timing suite was rejected on honesty.** `testdata/timing/timing_tests_48k_v1.0.z80`
restores, accepts `Enter`, and runs — printing `Pass` for test after test, which would have made a
striking and very on-brand picture. It then reaches **test 36** (`IN A,(n)` / `OUT (n),A` /
`IN r,(C)` / `OUT (C),r`, contended) and prints **`Fail`**, `sp=23300` against an expected
`sp=23337`. Photographing an early page of that run would have been selecting a green frame from a
suite that goes red later, which is the exact failure this repository documents itself committing.
The observation is left here as a finding for whoever owns the timing model; it is not a picture.

---

## Licensing — three different cases, and they are not the same

### 1. ROM output, and it is covered

**`128-menu.png` is ROM output.** It shows the 128 editor ROM's boot menu, including its
`© 1986 Sinclair Research Ltd` line, unaltered. That falls under the permission
[`testdata/README.md`](../../testdata/README.md) quotes in full — Cliff Lawson, for Amstrad,
31 August 1999: *"Amstrad are happy for emulator writers to include images of our copyrighted code
as long as the (c)opyright messages are not altered…"* — and this repository carries the
acknowledgement that permission asks for.

### 2. *Manic Miner*, and there is no permission

**`central-cavern.png`, `tape-loading.png` and `title.png` are screenshots of *Manic Miner*,
written by Matthew Smith and published by Bug-Byte Software Ltd in 1983. There is no permission
for them.** [`testdata/games/PROVENANCE.md`](../../testdata/games/PROVENANCE.md) records the
search: Spectrum Computing says of this exact file that *"these files are distribution denied"*,
World of Spectrum classifies the same title *"Available"* and serves it, both publishers'
distribution status is *"Unknown"*, and no statement by Matthew Smith releasing the game was found
— looked for, not found, which is not the same as absent. That conflict is reported here, not
resolved.

### 3. The three Hewson games, where something was found — and it is not a permission for this

**`cybernoid.png` is *Cybernoid — The Fighting Machine*, `cybernoid-ii.png` is *Cybernoid II — The
Revenge*, and `exolon.png` is *Exolon*. All three are by Raffaele Cecco and published by Hewson
Consultants Ltd** — *Exolon* in 1987, both *Cybernoid*s in 1988. *Cybernoid II*'s own title screen,
visible in `cybernoid-ii.png`, credits *"BY RAFFAELE CECCO / GRAPHICS BY HUGH BINNS / MUSIC BY DAVE
ROGERS"* and carries `CYBERNOID II (C) 1988 HEWSON` unaltered.

Fetched and read directly on 2026-09-01, not relayed:

| Source | What it says, verbatim | What that is |
|---|---|---|
| **Spectrum Computing**, entries [1686](https://spectrumcomputing.co.uk/entry/1686/ZX-Spectrum/Exolon), [1196](https://spectrumcomputing.co.uk/entry/1196/ZX-Spectrum/Cybernoid) and [1199](https://spectrumcomputing.co.uk/entry/1199/ZX-Spectrum/Cybernoid_II_The_Revenge) — the same sentence on all three | *"Hewson Consultants have told Spectrum Computing that they have no objection to us making this title available for download."* | A publisher's **stated non-objection, relayed by an archive**, scoped to *that archive's* downloads |
| **World of Spectrum / ZXDB** API, records `0001686`, `0001196`, `0001199` | `"availability": "Available"` for all three | An archive's **classification**, not a grant |
| **World of Spectrum**, [/archive](https://worldofspectrum.org/archive) | *"We also try our best to get permission from the copyright holders to distribute their software freely from this site."* | A stated **effort**, not a permission |

**That non-objection is real, it is better than what exists for *Manic Miner*, and it is still not
a permission covering these three files.** It was given to a named third party, about *making the
game available for download*, and these are screenshots taken by somebody else. Reading it as
cover for this repository would be stretching a sentence past what it says — which is the one move
`testdata/README.md` exists to prevent.

### What is true of every game frame here, and the whole of it

- **No game is distributed by this repository.** `testdata/**` is covered by `.gitignore` and
  nothing in `testdata/games/` is committed; `PROVENANCE.md` carries the `git check-ignore` output
  that shows it. Whoever runs these commands supplies the tapes themselves.
- **The provenance of the local copies is the machine's owner, not a documented fetch.**
  `PROVENANCE.md` records exactly where each *Manic Miner* file came from, with URLs. The four
  Hewson-game files came from a personal collection; their SHA-1s are listed above so a reader can
  tell whether they hold the same bytes, and that is all that can honestly be said about them.
- **These are screenshots, and screenshots are the ordinary practice of emulator projects.** They
  are here because a picture of a real game is the only honest way to show that one runs.
- **`title.png` carries the game's own `© BUG-BYTE ltd. 1983` line, and `cybernoid-ii.png` carries
  `CYBERNOID II (C) 1988 HEWSON`, both unaltered** — the condition the ROM permission attaches to
  ROM images. No comparable permission has been granted for either game, so that is a courtesy
  rather than a compliance.
- **No permission is claimed for any of them.** If a rights-holder objects, the right response is
  to remove the files, not to argue about them.
