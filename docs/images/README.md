# Images

Nine files, and which of them the repository's [`README.md`](../../README.md) shows is that file's
business rather than this one's — a tally here of how many it links would be a second copy of a
fact it owns, and the tally this page used to carry went stale the same day it was written. They
are all **produced by running the emulator**, not drawn, and this page is the commands that produce
them — so that a picture making a claim about the machine can be re-taken rather than trusted.

| File | 640 × 512 | What it is |
|---|---|---|
| `cybernoid.png` | the hero | *Cybernoid* playing, on the **128**, restored from a `.z80` snapshot |
| `central-cavern.png` | gallery | *Manic Miner* playing, on a 48K, loaded from tape by the ROM |
| `cybernoid-ii.png` | gallery | *Cybernoid II*'s screen arriving from tape, mid-load |
| `exolon.png` | gallery | *Exolon*'s screen — which was never on the tape as a screen |
| `exolon-playing.png` | gallery | *Exolon* **being played**, started from its own menu after the tape |
| `cybernoid-ii-playing.png` | gallery | *Cybernoid II* **being played**, started the same way |
| `128-menu.png` | gallery | the 128's own boot menu, unassisted |
| `tape-loading.png` | gallery | the tape three thousand frames in, mid-load |
| `title.png` | gallery | what that tape produced |

Four games: *Cybernoid*, *Manic Miner*, *Cybernoid II*, *Exolon*. Two machines. One of the nine
frames is a **128** and the rest are 48Ks, named per command below.

> **The two `-playing` files were added on 2026-09-01 and they close a gap this page named
> itself.** *What could not be photographed* said, correctly at the time, that *"a tape-loaded game
> cannot be started"* — every `--keys` tap happens before PLAY, so no key could reach a game that
> arrived three minutes of emulated time later — and it identified the fix as *"a missing flag, not
> a defect"*. `zx-shot --keys-after` is that flag. The two loading-screen frames are **kept**
> rather than replaced: `exolon.png` and `cybernoid-ii.png` are the evidence for two separate and
> still-interesting claims about how a tape arrives, and a picture of a game playing does not
> supersede a picture of a screen being decoded.

---

## Which pixels are the machine's

**All of them, in all nine files. There is no surround, no margin, no gutter and no caption
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

And the same loop over the two frames added later the same day, which is the point of a checker
that is a program rather than a paragraph — a new file is graded by running it, not by arguing
that it was produced the same way:

```
docs/images/exolon-playing.png: 13 distinct colours, 0 foreign
docs/images/cybernoid-ii-playing.png: 13 distinct colours, 0 foreign
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

Run on 2026-09-01 for **all seven**, from one binary, and every `cmp` was silent. Both checks were
run again the same day for the two `-playing` frames, against the build that first produced them:
each command re-run gave a byte-identical `.ppm`, and `pngtopam` on each published file
round-tripped to exactly `pamenlarge 2` of that `.ppm`. That is the
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

# --- the same two tapes, now started: --keys-after waits for the tape and then types
# `--settle` is the gap on *both* sides of the after-keys, and `--hold 30` is not decoration:
# see "A game's menu polls less often than the ROM does" below.
START='Space;Space;Key1'          # leave the loading screen, reach the menu, 1 START GAME
$ZX --rom testdata/roms/48.rom --media testdata/games/Exolon.tap \
    --keys "$LOAD" --play-tape --keys-after "$START" --hold 30 --settle 800 \
    --out exolon-playing.ppm
$ZX --rom testdata/roms/48.rom --media testdata/games/CybernoidII.tap \
    --keys "$LOAD" --play-tape --keys-after "$START" --hold 30 --settle 2000 \
    --out cybernoid-ii-playing.ppm

# --- the 128's own boot menu; no keys at all --------------------------------
$ZX --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom --frames 120 \
    --out 128-menu.ppm

# --- the same Manic Miner tape, earlier and later ---------------------------
$ZX --rom testdata/roms/48.rom --media testdata/games/ManicMiner.tap \
    --keys "$LOAD" --play-tape --settle 3000 --out tape-loading.ppm
$ZX --rom testdata/roms/48.rom --media testdata/games/ManicMiner.tap \
    --keys "$LOAD" --play-tape --settle 10750 --out title.ppm

# every .ppm the commands above just wrote — the stems are read off what they produced rather
# than retyped, for the reason below.
for f in *.ppm; do
    pamenlarge 2 "$f" | pamtopng > "docs/images/${f%.ppm}.png"
done
```

`pamenlarge` and `pamtopng` are [netpbm](https://netpbm.sourceforge.net). Neither scales, filters
nor interpolates, so neither can alter a machine pixel; the `cmp` above is what shows that rather
than the sentence.

**That loop used to name seven stems, and for as long as it did, this page was a recipe that
reproduced seven of the nine files beside it.** The two `-playing` frames were photographed by the
commands above and then converted by a hand the page never described, so somebody following it end
to end finished holding two `.ppm`s with no instruction and no explanation — which is this
directory's own subject failing on itself. An image whose published path stops short of the file is
not evidence, and a recipe that silently covers most of its output is that failure in miniature. A
typed list of stems was a second copy of the `--out` names above it and it fell behind them exactly
as a second copy does; the glob is derived from what those commands wrote, so the next frame added
to this page converts itself.

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

### `exolon-playing.png` and `cybernoid-ii-playing.png` — a tape game, started

Both are the same three-tap sequence against two different games, and the sequence is uniform
because what it walks is uniform: **a Hewson loading screen and a Hewson title screen each consume
one key, and only the third is a choice.** `Space;Space;Key1` — dismiss, advance, `1 START GAME`.
Neither of the first two keys matters; `Space;Key1;Key1` and `Key1;Key1;Key1` were tried and put
both games in play as well, which is what identifies them as *advance* rather than as *select*.

`exolon-playing.png` is the first zone: the hero under the teleport arch he has just materialised
from, a gun emplacement with **a shot in the air**, three planets, and the game's own status line
— `AMMO 99  GRENADES 10  POINTS 000000  LIVES 6  ZONES 000`. The lives count is the honest detail:
nobody is at the keyboard, so the hero stands under the gun and dies about every four seconds, and
`6` is where the counter had reached 800 frames after `1` was pressed.

`cybernoid-ii-playing.png` is the first chamber, and its evidence is the **score**: `000025`, up
from zero. A frame of a static title screen and a frame of a running game are hard to tell apart
from a still, and a scoreboard that has moved is the difference. The ship is in flight, two of the
chamber's guardians are on their platform, and a pod sits below.

**Neither picture required knowing anything about either game in advance.** The wait was read off
the tape rather than swept for — see below — and the key sequence was found by pressing candidates
and looking, in eight parallel runs that took seconds.

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

**~~A tape-loaded game cannot be started.~~ Fixed the same day, by the flag this paragraph asked
for.** What it said was right: `zx-shot` presses every `--keys` tap *before* it presses PLAY, so no
key could reach a game that arrives minutes later. *Exolon* and *Cybernoid II* both loaded
correctly and then sat on their own menus waiting for a key the tool had no way to send —
*Cybernoid II* observed cycling title ↔ hall of fame out to `--settle 160000`, about 53 minutes of
emulated time, and *Exolon* holding its loading screen out to `--settle 175000`. It named the
remedy as *"a missing flag, not a defect"*, and `zx-shot --keys-after` is that flag. Both games are
now in this gallery playing, and the paragraph is corrected rather than deleted because a
prediction that came true within the day is worth more standing than removed.

**How the flag knows the game has arrived, which is the part that was not obvious.** A fixed frame
count is what every command above uses and it is fragile: it is a property of one tape on one
model, and `--settle 11750` means *Central Cavern* on this *Manic Miner* file and nothing anywhere
else. So `--keys-after` does not count — it **asks the tape**. `docs/M6.md` Decision 5 makes the
pulse train the tape's representation rather than a detail of one, and `Tape::pulses` is public
because of it; the half-periods are T-states, their sum is the cassette end to end, and dividing by
the machine's own frame length gives the wait. Read, not swept for, and right for either model:

```
--keys-after: waited 10653 frames; the tape ran out at frame 10953     # Exolon.tap
--keys-after: waited 13904 frames; the tape ran out at frame 14864     # CybernoidII.tap
```

**What the tape cannot say is how long the loader then takes to jump into the game**, and nothing
in the machine can be asked either — whether the next instant is a title screen, a menu or a black
frame mid-clear is a property of the game. That gap is `--settle`, it is an honest guess rather
than a derived number, and it is named as one. It is applied on **both** sides of the after-keys —
before them so the game is up to receive them, after them so what they did is on the picture — and
one number serving both was checked rather than assumed: `--settle` was swept over 200, 400, 600,
800, 1000, 1200, 1600 and 2000, and **every one of the sixteen runs put both games in play**. A
second flag would have bought nothing, so there is not one.

**A game's menu polls less often than the ROM does, and `--hold` is where that shows.** At the
default `--hold 10`, *Cybernoid II* starts and *Exolon* does not — it sits on its menu, because its
tap was too short for the menu's scan. `--hold 30` starts both, and both published commands use it.
This is exactly the ambiguity `zx-shot`'s own source names about `--hold` — *"every 'the game does
not respond' is ambiguous between a defect in the emulator and a tap this tool made too short"* —
and it is now a measurement rather than a worry: the tool can press a key after a load, so the two
can be told apart by pressing it for longer.

**The other end of `--hold` is a threshold, not a number.** `--hold` is shared by `--keys` and
`--keys-after`, so a hold long enough for a game's menu is also a hold the 48K editor reads as a key
being held down — and past a point it starts repeating it. It surfaced while `--hold` was being
raised for *Exolon*'s menu, and what got written down was the line a single run at `--hold 60` put
on the screen. That is a reading taken at one setting, recorded as though it were a property of the
flag: the count is a function of `--hold`, so no one rendering of the line is the true one. *(The
entry also said it was seen in "two of the sixteen sweep runs", which cannot be right — those
sixteen are the `--settle` sweep above, and every one of them put both games in play.)*

Re-measured on 2026-09-01 against `testdata/roms/48.rom`, by photographing the editor straight after
the four taps and counting the keywords on the picture rather than reasoning about repeat rates:

```sh
$ZX --rom testdata/roms/48.rom --media testdata/games/Exolon.tap \
    --keys "$LOAD" --play-tape --hold N --settle 50 --out hold-N.ppm
```

| `--hold` | `LOAD` keywords on screen | |
|---|---|---|
| 10, 20, 30, 35 | 1 | the line is `LOAD ""`, `ENTER` submits it, the tape loads |
| 36, 37, 38, 39, 40 | 2 | the rest are a syntax error and the tape never starts |
| 41, 45 | 3 | |
| 50 | 4 | |
| 55 | 5 | |
| 60 | 6 | |
| 65 | 7 | |
| 70 | 8 | |
| 75 | 9 | |
| 80 | 10 | |

So 35 is the longest hold the editor still accepts, the first duplicate arrives at 36, and every
further five frames buys one more — a 35-frame delay and a 5-frame period, read off the screen
rather than off a variable. The figure the old sentence carried was right for the run it came from
and wrong as a fact about the flag, which is the whole distinction this page exists to keep.

**It is not only the keyword that repeats, and that is the part the old line hid.** Every tap in
the script repeats, because every tap is held for the same `--hold`. At 60 the screen reads

```
LOAD ? LOAD LOAD LOAD LOAD LOAD
""""""""""""L
```

— six keywords, and **twelve** quotes rather than two, six from each `SYMBOL SHIFT`+`P`. The `?` is
the editor's flashing error marker sitting where the second `LOAD` made the line unparseable, and
the `L` is the cursor. Run the published `--keys-after` command at `--hold 60` and the `1` arrives
six times as well, into the editor, because the game it was meant for never loaded.

**Neither the tape nor the emulator is involved, and that is worth stating because it narrows what
could ever break here.** Every key is typed before PLAY, so at `--hold 60` *Exolon*, *Manic Miner*
and an empty drive produce **byte-identical** frames — the `.ppm`s have the same MD5. Nor is it
true of the machine in general: the same command with two `--rom`s never reaches an editor at all,
because the 128 boot menu takes `ENTER` as *Tape Loader* and the frame shows that loader waiting.
This is a 48K-editor property, reachable only through `--keys`.

`30` is clear of both edges — five frames under the repeat delay, and enough for *Exolon*'s menu —
and nothing was added to separate the two halves, because one number that works for both is a knob
nobody has to reason about.

**The threshold is gated now, and the gate is deliberately not this table.** `zx-shot`'s own
`mod tests` — inside the binary, because `--hold`'s default and the `press` that implements a hold
are both private to it — runs the four taps at the two settings either side of the edge and asserts
the change across them: at the delay every tap lands once and `ENTER` submits the line, one frame
above it every tap lands twice, quotes included, and the syntax error stays in the editor. Both
holds are derived from the same constant rather than typed, so a wrong constant reddens on both
sides at once. It also reads `REPDEL` — the repeat delay the 48K's keyboard routine counts down,
35 at boot — back out of the booted machine, which is where the number comes from and what makes a
red say *which* fact moved: how the tool steps a key, or which ROM is in `testdata/`.

The ramp above stays a reading and is asserted nowhere, and the reason is arithmetic rather than
taste: a delay of 30 with a period of 6 reproduces the sixty-frame row of that table exactly — six
keywords — while duplicating at 35. A gate pinned to any one row of the ramp would stay green
through that, with the edge five frames from where this page says it is.

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

**~~Whether that is a game waiting for a key or a defect cannot be decided with the tool as it
stands, and that is the point.~~ It was decided the same day, and the answer is *waiting for a
key*.** The paragraph was right that the two were indistinguishable — `zx-shot` could not press a
key after a load, so *"it does not advance"* and *"it is waiting for input"* produced identical
evidence — and it was right that this, rather than a nicer gallery, was the stronger reason for the
missing flag. With `--keys-after` the question takes one command:

```sh
$ZX --rom testdata/roms/48.rom --media testdata/games/MarioBros.tzx \
    --keys "$LOAD" --play-tape --keys-after 'Space;Space;Key1' --hold 30 --settle 600 \
    --out /tmp/mario.ppm
# --keys-after: waited 12294 frames; the tape ran out at frame 13254
```

One key past `OCEAN PRESENTS` reaches the game's own `OPTIONS` menu — `1 1 PLAYER`, `2 2 PLAYER`,
`3 REDEFINE KEYS`, `OCEAN (C) 1987`, `CODED BY CHOICE` — and `1` starts it: Mario on the girders,
the `POW` block, turtles walking, a coin, and `MARIO 000000` across the top. So the 47 minutes of
apparent inactivity were a title screen doing exactly what a title screen does, and **nothing in
the emulator was wrong**. The `.tzx` container, the four standard-speed blocks, and the game's own
code were all working the whole time.

**No Mario Bros frame is published here, and the reason is provenance rather than the picture.**
The licensing section below sets out a separate case for each rights-holder, resting on searches
that were actually performed; Ocean Software is a fourth publisher and no such search has been done
for it. A screenshot is cheap to take and a claim about permission is not, so what is recorded is
the finding and not the frame.

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

> **Three corrections to the sentence above, made 2026-09-01 by the pass that extended
> `crates/spectrum/tests/timing_oracle.rs`. The finding stands; its three specifics were each
> wrong in a way worth leaving visible.**
>
> - **It quotes one of *two* mismatching fields.** The suite highlights every field that
>   disagrees, and two did: **`R=54` against an expected `49`**, as well as `sp=23300` against
>   `23337`. Only **`loop=237`** matched. Reporting one of two mismatches understates the
>   disagreement by half.
> - **The instruction list does not identify test 36.** Tests **35, 36 and 37 are one program** —
>   the suite's dispatch table sends all three to `0xC91D` — so they share that instruction list
>   and it cannot distinguish them. The screen does: the BASIC sets `l` to **0**, **13** and
>   **21** and prints that many rows of `"ZXSPECTRUMZXSPECTRUMZXSPECTRUMZX"` before running, and
>   test 36's own title is `… [13]`, which is literally `STR$ l`. **The `[13]` is the
>   discriminating variable**; the instruction list is the part the three have in common.
> - **The third field is not a stack pointer, and 37 is not a T-state count.** The suite labels it
>   `sp`, but what its handler records is the **interrupted `PC`**: `23300` is `0x5B04` and
>   `23337` is `0x5B29`, both instruction boundaries inside the loop the suite copies to
>   `0x5B00`, and `0x5B29` is its `IN L,(C)`. The 37 between them is a *distance through the loop
>   body*, not an amount of time.
>
> **The finding has since been acted on rather than only recorded.** `timing_oracle.rs` grades
> group **35** as of the same day and names 36 and 37 as needing a floating bus — which is also
> the explanation for the numbers above: this machine floats the constant `0xFF`, so its reading
> for test 36 is its reading for test **35**, to the byte (`R=54 loop=237 sp=23300`), and 35 is
> the row hardware agrees with.

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

**`cybernoid.png` is *Cybernoid — The Fighting Machine*, `cybernoid-ii.png` and
`cybernoid-ii-playing.png` are *Cybernoid II — The Revenge*, and `exolon.png` and
`exolon-playing.png` are *Exolon*. All five are by Raffaele Cecco and published by Hewson
Consultants Ltd** — *Exolon* in 1987, both *Cybernoid*s in 1988. The two `-playing` frames are the
same three titles and the same rights-holder as the frames beside them, so they add a file to this
case and nothing to its reasoning. *Cybernoid II*'s own title screen,
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
