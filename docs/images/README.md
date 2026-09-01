# Images

One file, referenced from the repository's [`README.md`](../../README.md). It is **produced by
running the emulator**, not drawn, and this page is the command that produces it — so that a
picture making a claim about the machine can be re-taken rather than trusted.

---

## `48k-and-128.png` — 684 × 280

Two frames side by side: a 48K on the left, a 128 on the right.

**Every pixel inside the two 320 × 256 rectangles is the emulator's own output**, and it reached
this file through the path the window runs — `Spectrum::render` into a `spectrum::Frame`, then
`frontend::palette::write_rgba`. The binary that writes it is `zx-shot`, which differs from `zx`
only in the last step: it emits [`frontend::ppm`](../../crates/frontend/src/ppm.rs) bytes where
the window uploads a texture, and `crates/frontend/tests/ppm_encoding.rs` asserts those are the
same buffer. A screenshot produced by code the window does not run would prove nothing about
the window.

**Nothing was retouched and nothing was resampled.** The frames are placed at scale 1, so the
pixels in the PNG are byte-identical to the bytes `zx-shot` wrote — not merely equivalent to
them. `pamtopng` is lossless and round-trips back to the same P6.

### Which pixels are not the machine

The 12-pixel margin and the 20-pixel gutter, and nothing else — 14.5% of the image. They are
`#3C3C3C`, and that value is chosen so the question cannot be argued rather than so it looks
right. [`crates/spectrum/src/screen.rs`](../../crates/spectrum/src/screen.rs)'s `Colour::rgb`
emits exactly three channel levels — `0x00`, `0xD7` and `0xFF` — so `0x3C` is a value the ULA
**cannot** produce, in any colour, on any pixel. The frame rectangles and the surround are
therefore separable by inspection, not by taking anyone's word for where the boundary is.

### What the left-hand frame is doing

A 48K, booted from the ROM, then typed at through
[`frontend::keymap`](../../crates/frontend/src/keymap.rs) — the same table the window presses, so
`zx-shot` cannot press a key the window could not. Two direct commands:

```basic
BORDER 2
PRINT AT 10,10;"zx-emulator"
```

`0 OK, 0:1` at the bottom is the ROM's own report, not a caption. Three things in that script are
the keymap's documented limits rather than conveniences added for a picture, and **no binding was
added to make this easier**:

| On screen | Typed as | Why |
|---|---|---|
| `"` | `LeftControl+P` | `SYMBOL SHIFT`+`P`. `"` is a *shifted* key on a PC, so the keymap deliberately leaves it to the hardware route |
| `AT` | `LeftControl+I` | `SYMBOL SHIFT`+`I` — a token, not three letters |
| `-` | `Minus` | Bound, because a PC `-` and `SYMBOL SHIFT`+`J` mean the same thing |
| lowercase | — | A 48K boots in `L` mode, so unshifted letters arrive lowercase. That is the machine's doing, and it happens to be how the project spells its own name |

`(` and `)` are deliberately unbound and were not needed.

### The commands

Taken 2026-09-01. `zx-shot` is deterministic — both frames reproduce byte-for-byte on a re-run,
checked with `cmp`.

```sh
cargo build --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot

# Left: the 48K. One --rom is a 48K; the key script is one tap per `;`, `+` holds keys together.
./target/release/zx-shot --rom testdata/roms/48.rom --frames 120 --settle 40 \
    --keys 'B;Key2;Enter;P;LeftControl+I;Key1;Key0;Comma;Key1;Key0;Semicolon;LeftControl+P;Z;X;Minus;E;M;U;L;A;T;O;R;LeftControl+P;Enter' \
    --out left-48k.ppm

# Right: the 128. Two --rom make a 128, editor ROM first; its menu needs no keys.
./target/release/zx-shot --rom testdata/roms/128-0.rom --rom testdata/roms/128-1.rom \
    --frames 120 --out right-128.ppm
```

Composition is [netpbm](https://netpbm.sourceforge.net) — `pamcat` concatenates and `pamtopng`
encodes; neither scales, filters or interpolates, so neither can alter a machine pixel:

```sh
ppmmake rgb:3c/3c/3c 12 256 > margin.ppm
ppmmake rgb:3c/3c/3c 20 256 > gutter.ppm
ppmmake rgb:3c/3c/3c 684 12 > band.ppm
pamcat -leftright margin.ppm left-48k.ppm gutter.ppm right-128.ppm margin.ppm > row.ppm
pamcat -topbottom band.ppm row.ppm band.ppm > final.ppm
pamtopng final.ppm > docs/images/48k-and-128.png
```

### Scale

**Scale 1, and the frames are not enlarged.** A Spectrum pixel is a square of solid colour, so a
non-integer resample is the one thing that would visibly damage this subject —
[`crates/frontend/src/viewport.rs`](../../crates/frontend/src/viewport.rs) refuses fractional
scaling in the window for the same reason. At 684 px the image is inside the width GitHub gives a
README, so **GitHub does not resize it at all**, which is the only resampling step this repository
controls.

Scale 2 was measured and rejected: two frames at ×2 is 1324 px, which GitHub then scales by
0.64 — a non-integer reduction, and strictly worse than not enlarging. One frame at ×2 fits, but
a single frame cannot show both machines, and *48K and 128* is what the milestone table claims.
The cost is stated rather than hidden: at scale 1 a browser on a high-density display enlarges
the file by 2, so the pixel edges are softer there than on a 1× display, where the image is
exact.
