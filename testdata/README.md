# `testdata/` — external test corpora

Nothing in this directory is committed **except this file and the Sinclair ROMs** —
`testdata/roms/48.rom`, and since M7 `testdata/roms/128-0.rom` and `testdata/roms/128-1.rom`. The
corpora are third-party data with their own provenance and licensing, they are large, and they
are reproducible from an authoritative source — so they are fetched on demand instead. The ROMs
are the one exception, on the strength of a permission whose text, author, date, conditions and
scope are quoted in full under [*The permission this rests on*](#the-permission-this-rests-on-quoted--and-the-acknowledgement-it-asks-for);
`.gitignore` names every one of them, individually.

> **This sentence said *"except this file and `testdata/roms/48.rom`"* until M7 committed the
> 128's pair.** It is corrected rather than left to be inferred from the section below, because
> the sentence is a *licensing* claim about what this repository redistributes and it is the
> first one a reader meets. A file list that has gone stale reads exactly like a file list that
> is complete.

> **This sentence used to end *"because Amstrad has explicitly permitted redistributing the
> Sinclair ROMs with emulators"* and stop there** — one of five unsourced copies of a licensing
> claim. It now points at the one place that carries the quotation, which is the same *one register,
> one owner* rule `docs/ARCHITECTURE.md` applies to the open register. The copies are named, and
> what was wrong with them, in the section linked above.

**A fresh clone does not test green, and that is the design.** Absence of a corpus makes its
gate **fail**, naming the fetch — see *Making absence a failure*. A gate that skips silently is
a gate that verifies nothing while looking like it verifies something, which is the failure
`docs/STATUS.md` records the FUSE harness producing: with `testdata/fuse` moved aside,
`cargo test -p z80` exited 0 with the same 87 tests and zero failures.

> **This paragraph said the opposite** — *"`crates/z80/tests/` skips its conformance run with a
> printed explanation when the files are absent, so a fresh clone still builds and tests
> green"* — and it was describing the behaviour that was **deliberately removed**, in a
> document that states the replacement correctly 100 lines further down. It is corrected rather
> than deleted because `README.md` advertises bare `cargo test` as the entry point, so a reader
> who acted on it met a red suite and a paragraph that had predicted otherwise.
>
> To work without the corpora, set `ZX_CORPUS_ALLOW_MISSING=1`. That is the *declared* absence,
> and it is refused under `CI`.

---

## `testdata/fuse/` — the FUSE Z80 conformance vectors

`tests.in` and `tests.expected` come from the **FUSE** (Free Unix Spectrum Emulator)
project. They are ~1300 per-instruction vectors: an initial register set and memory image,
the expected final register set and memory, the expected bus-access trace, and — the part
that matters most here — the **expected T-state count**.

This is *test data*, not emulator source. Our CPU core is written from the Z80 hardware
specification; the vectors are the external oracle that proves it, exactly as
`docs/ARCHITECTURE.md` intends.

### Fetching

```sh
mkdir -p testdata/fuse
curl -fSL -o testdata/fuse/tests.in \
  https://raw.githubusercontent.com/floooh/chips-test/master/tests/fuse/tests.in
curl -fSL -o testdata/fuse/tests.expected \
  https://raw.githubusercontent.com/floooh/chips-test/master/tests/fuse/tests.expected
```

The URLs above are a convenient mirror of the data files. The upstream home is the FUSE
project itself (`fuse-emulator`, `z80/tests/`); any copy of the two files works, because
the harness validates their structure on load rather than trusting them.

### What the harness does with them

| | |
|---|---|
| Vectors in the corpus (as fetched 2026-08-31) | **1335** |
| Executed at M1 — un-prefixed opcodes | **290** |
| Skipped as M2 — `CB` / `ED` / `DD` / `FD` prefixed | **1045** (`DD` 343, `FD` 341, `CB` 264, `ED` 97) |

Skipped vectors are **counted and printed on every run**, never silently dropped.
Classification is by the opcode byte actually at the initial `PC`, not by the vector's
name — the name is a label, the fetched byte is what the CPU decodes.

Every executed vector asserts:

- all twelve 16-bit registers, `AF`/`AF'` decomposed into named flag bits on failure
- the undocumented `F` bits 3 and 5
- `I`, `R`, `IFF1`, `IFF2`, `IM`, and the halt flag
- every memory location the vector lists, one finding per address
- the **T-state total**, accumulated from the core's own `Bus::tick` calls

### Licensing

FUSE is distributed under the GNU GPL. The vectors are not redistributed in this
repository — they are downloaded by whoever runs the tests, which is why this file exists
instead of a checked-in copy.

---

## `testdata/zex/` — the `zex` CP/M instruction exercisers

`zexdoc.com` is the **M3** oracle and `zexall.com` is **M4**'s. They are CP/M `.COM`
programs rather than data: each runs millions of instruction sequences, folds the results
into a CRC, and prints `OK` or `ERROR` per test group against a CRC built into the binary.
The verdict is the program's, not ours.

The two are the *same program* with different flag masks — every one of `zexdoc`'s 67
descriptors masks the undocumented `F` bits off (`0xc7` / `0xd7` / `0x53`) where `zexall`
uses `0xff`, and 31 of the 67 expected CRCs differ as a result. That is why they execute an
identical instruction stream and take an identical number of T-states.

### Fetching

```sh
mkdir -p testdata/zex
base=https://raw.githubusercontent.com/anotherlin/z80emu/master/testfiles
curl -fSL -o testdata/zex/zexdoc.com "$base/zexdoc.com"
curl -fSL -o testdata/zex/zexall.com "$base/zexall.com"
```

Both files are 8704 bytes. As with the FUSE vectors, any genuine copy works: the harness
validates the image's structure on load — it must contain the four report literals the
report parser reads — rather than pinning a checksum.

### Running the gate

`zexdoc` is 5.76 billion instructions, so it is `#[ignore]`d: 43 s in release and about
20 minutes in the `dev` profile `cargo test` uses by default. It is run explicitly:

```sh
cargo test --release -p z80 --test zex_oracle -- --ignored --nocapture
```

The tests around it — the CP/M shell, the report parser, and one failing case per gate rule — are
**not** ignored and run on every `cargo test -p z80` without needing the corpus at all. **Count
them rather than quoting a number:**

```sh
grep -c '#\[test\]' crates/z80/tests/zex_oracle.rs     # 21 on 2026-09-01
grep -c '#\[ignore' crates/z80/tests/zex_oracle.rs     #  2 — the two conformance runs
```

> **This paragraph said *"the sixteen tests around it"*, and it was **nineteen***: 21 `#[test]`s of
> which `zexdoc_conformance` and `zexall_conformance` carry `#[ignore]`, counted by the two commands
> above on 2026-09-01. The argument the sentence makes is untouched — the command still exits 0
> without ever looking for the exerciser, which is the whole point. Only the integer was stale.
>
> **It was stale in two files at once, and the pass that found it fixed neither.**
> `docs/STATUS.md` records exactly that — *"the same stale sixteen sits in `testdata/README.md`,
> which is another agent's file and is routed separately"* — and it is the cleanest example in this
> repository of the propagation defect that document catalogues at length: one derived figure, two
> files, and the correction landing only in the one being read. The routing finally arrived on
> 2026-09-01. **The remedy is not a better number; it is publishing the command instead of the
> answer**, which is what the block above does, because this integer changes on somebody else's
> commit and will go stale again the moment a test is added.

~~`zexall` is deliberately **not** wired up as a gate at M3.~~ **Superseded at M4**, and struck
rather than deleted because the *"at M3"* scoping made it read as still current for two
milestones. `zexall` **is** a gate: `docs/STATUS.md`'s M4 section records both exercisers
reporting 67/67 over an identical 5,764,169,610-instruction stream, and M4 merged on it. What
that green is worth is a separate question the same section answers at length — it cannot
separate the Q rule from `A & 0x28`, having been observed passing against three different
implementations — and that is a limit of the oracle, not a reason it is not one.

### Making absence a failure

Absence is a skip only when it has been declared, and the declaration is refused under CI —
otherwise a conformance gate becomes a green tick that proves nothing. This applies to **every**
corpus in the workspace, through one shared decision point in `crates/testsupport`:

| | |
|---|---|
| corpus present | it runs |
| corpus absent | **the gate fails**, naming the fetch instructions |
| corpus absent, `ZX_CORPUS_ALLOW_MISSING=1` | the gate skips, printing why |
| corpus absent, `ZX_CORPUS_ALLOW_MISSING=1`, `CI` set | **refused** — the opt-out must never decide what a pipeline verifies |
| `Z80_FUSE_REQUIRED` set at all | **refused** — obsolete |
| `Z80_FUSE_ALLOW_MISSING` set at all | **refused** — obsolete; see below |

`ZX_CORPUS_ALLOW_MISSING` is accepted as any of `1`/`true`/`yes`/`on`, case-insensitively;
anything unrecognised is an error rather than a silent `false`.

#### Why the variable was renamed, and why the old name is an error

It was `Z80_FUSE_ALLOW_MISSING`, which named one crate and one corpus while governing three.
That understatement is not cosmetic: a developer who sets a variable named for the FUSE
corpus, in order to work without the FUSE corpus, was **also disarming the `zex` gate and
now the Spectrum ROM gate**, silently. The name had to say what it turns off.

Setting either old spelling is a hard error rather than a no-op, for the reason
`docs/STATUS.md` already records about `Z80_FUSE_REQUIRED` — *a guard that exists solely in a
file nobody runs*. A stale CI configuration exporting the old name must fail loudly on the
first run, not quietly disarm every gate while looking armed.

A per-corpus opt-out would be better still — disarming FUSE should not disarm the ROM — and
is deliberately **not** implemented here: it multiplies the number of variables a stale
configuration can get wrong, and the `CI` refusal already covers the case that matters.

### Licensing

The `zex` exercisers are Frank Cringle's `zexlax`/`zexall` work, redistributed widely under
permissive terms. As with the FUSE vectors they are fetched rather than committed, so this
repository redistributes nothing.

---

## `testdata/snapshots/` — third-party `.z80` and `.sna` snapshots

**The M6 external oracle.** Every other snapshot gate compares a value against a value that
came from our own code, so a *symmetric* misreading of the format — a field permuted, an
offset shared by the parser and the writer, a field dropped in both directions — survives all
of them. `crates/spectrum/tests/snapshot_vectors.rs` breaks that symmetry using the format
*description*; these files break it using independent *implementations* of that description.
`docs/M6.md` puts it plainly: a snapshot format is an interoperation contract with dozens of
independent implementations, and it is the one place in the machine layer where an external
check on **our reading of a specification** exists at all.

Fetched and gitignored, exactly like the FUSE vectors and the `zex` exercisers. Nothing here
is redistributed.

### Fetching

```sh
mkdir -p testdata/snapshots
cd testdata/snapshots
curl -fSL -o 3dshow_demo.z80   https://raw.githubusercontent.com/antirez/zx2040/main/games/z80/3dshow_demo.z80
curl -fSL -o spectral_test.z80 https://raw.githubusercontent.com/r-lyeh/Spectral/main/src/res/rzx/test.z80
curl -fSL -o fire.z80          https://raw.githubusercontent.com/remogatto/gospeccy/master/src/formats/testdata/fire.z80
curl -fSL -o fire.sna          https://raw.githubusercontent.com/remogatto/gospeccy/master/src/formats/testdata/fire.sna
shasum -a 256 *
```

| File | Bytes | SHA-256 | What it is | Source, and its licence |
|---|---|---|---|---|
| `3dshow_demo.z80` | 24760 | `477ec76c7556a9329408aa8976f0cfdc9516d753b7df3672fbf96076af37e57c` | **version 1**, 48K, run-length encoded (byte 12 = `0x20`) | [`antirez/zx2040`](https://github.com/antirez/zx2040) — MIT. A 1990s demoscene production the repository ships itself |
| `spectral_test.z80` | 1341 | `bc32121f62d66684e941094c355f841a757d3fdf1ca3c285583b18e3c4b7faa2` | **version 2**, 48K (additional header 23, hardware mode 0) | [`r-lyeh/Spectral`](https://github.com/r-lyeh/Spectral) — Unlicense / public domain. The emulator's own test fixture |
| `fire.z80` | 2042 | `1bdd66e2fa456a383f2d97685b5ff9ada3b027390f5d05b576ae4d6bc3c3df4a` | **version 3**, 48K (additional header 54, hardware mode 0); `PC` = `0x0038`, taken inside the interrupt handler | [`remogatto/gospeccy`](https://github.com/remogatto/gospeccy) — MIT. The *Fire104b* intro by Andrew Gerrand |
| `fire.sna` | 49179 | `54f890f3b5509f08bc98b81b2b8ef5d5591f2ff6ba102a69c262b0bab7daacb9` | **the same machine state as `fire.z80`, in the other format** | as above |

Verified 2026-09-01: all four downloaded, all four hashes reproduced, all four parsed.

**The last row is the valuable one, and it is not a convenience.** A third party saved one
machine in both formats, which is an *independent* claim that a particular `.z80` and a
particular `.sna` describe the same state. `.sna` has **no writer here** — `docs/M6.md`
Decision 3 refuses one, because a `.sna` writer must push `PC` onto the guest's stack and
destroy two bytes of the RAM it is recording — so it has no round trip of its own, and this
pair is the **only** instrument in the workspace that grades its offsets against anything but
our own expectations. It immediately earned that: it caught a wrong expectation about `SP`
(the writer pushed, so a correct pop restores the *same* `SP` the `.z80` carries, not one two
higher), which no test built from our own code could have found.

### What the gate does with them

`crates/spectrum/tests/snapshot_corpus.rs` is a **directory sweep**, not a list of named
files, so a user's own snapshots are swept too:

| | |
|---|---|
| every `.z80` and `.sna` present | must parse |
| every parsed snapshot | must survive `parse(write(parse(f))) == parse(f)` — **not** byte-identity, which is neither achievable nor desirable over a foreign file |
| every parsed snapshot | must carry banks 5, 2 and 0 |
| any `<name>.z80` + `<name>.sna` pair | must read as the same machine state through both parsers |
| a file refused as `UnsupportedHardware` | **counted and printed, not failed** — a 128 snapshot is an M7 boundary, not a defect. The sweep then asserts that *something* parsed, so a directory of only-128 files cannot read as a pass |

### Licensing

All four are demoscene, homebrew or an emulator's own test fixture, from MIT or
public-domain repositories — deliberately, since commercial game images may not be
redistributed. As with every other corpus here they are **fetched by whoever runs the tests**
rather than committed, so this repository redistributes nothing either way.

### Making absence a failure

The same rule as every other corpus, through the same environment variable and the same
`crates/testsupport` decision point:

| | |
|---|---|
| present | it runs |
| absent | **the gate fails**, naming the fetch instructions |
| absent, `ZX_CORPUS_ALLOW_MISSING=1` | the gate skips, printing why |
| absent, `ZX_CORPUS_ALLOW_MISSING=1`, `CI` set | **refused** |

All four rows were exercised on 2026-09-01 by moving the directory aside: undeclared
absence failed both tests, the declared opt-out skipped, and the opt-out under `CI` was
refused. This table is checked rather than asserted, because `testdata/README.md` has
carried a fictional one before — see the note under the ROM section.

---

## `testdata/timing/` — the timing test suites, 48K and 128

**The machine-level oracle**, and the first thing in `crates/spectrum` whose expected numbers
were not written by this project. `timing_tests_48k_v1.0.z80` is Richard Butler's 48K timing test
suite (ZXSpectrum4.net, 2010): a `.z80` snapshot carrying a BASIC front end, 34 machine-code test
groups, and — the part that matters — **two tables of expected results measured on real
Spectrums**, at `0xE200` and `0xE400`.

Its authors state the provenance in as many words — *"In order to get the correct results we ran
the tests on real Spectrums"* — and their results database is hardware-only: *"Only submit results
from genuine hardware no emulators!"*.

### Fetching

```sh
mkdir -p testdata/timing
curl -fSL -o testdata/timing/timing_tests_48k_v1.0.z80 \
  https://raw.githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.EmulatorTestSuites.ZXSpectrum/Timing/timing_tests_48k_v1.0.z80
shasum -a 256 testdata/timing/timing_tests_48k_v1.0.z80
```

| | |
|---|---|
| Size | 10883 bytes |
| SHA-256 | `1e66230a7b23737294f35d2778b8384ce3f81412b98883d35e564091377382af` |
| Verified | 2026-09-01 — downloaded, hash reproduced, gate green |

### What the gate does with it

`crates/spectrum/tests/timing_oracle.rs` runs each group twice — once where it sits in
uncontended memory and once copied into the screen bank — and compares the results against the
suite's own tables. **70 hardware rows, 0 disagreements.**

**State what it grades precisely if you cite it.** Sixteen mutations bound it: 14333, 14334,
14336, 14337 and 14361 for `FIRST_CONTENDED_T_STATE` all go red and only **14335** is green, and
perturbing the delay pattern, making `Ula::fetch` a three-T-state read, or stopping internal
cycles contending all go red by 14–38 rows. Three of the sixteen are the four-case I/O rule's
`C:1, C:1, C:1, C:1` arm, and they arrived with group 35 in 2026-09-01's extension from 34
instruction groups to 35 — the arm deleted and the arm weakened to a two-stall shape each redden
**2 of 70**, its fourth term alone dropped reddens **1 of 70**. But three mutations came back
**green** — shortening the interrupt window, and moving the interrupt and the window *together* —
so the oracle grades the **interval from `/INT` to the first contended T-state**, not the constant
alone. The constant is anchored; the frame's origin remains a convention and the interrupt
window's length is still ungraded.

**The row counts above are not interchangeable.** Everything measured before group 35 was measured
against 68 rows and is quoted at 68; only the three I/O rows ran against 70. `docs/MACHINE.md`'s
mutation table keeps both denominators visible for that reason.

> **One clause above is out of date and is left standing because a reader may have acted on it:**
> *"the interrupt window's length is still ungraded."* It is graded, on both machines, as a **band**
> — `17..=32` on a 48K and `33..=43` on a 128 — by sweeping it against each suite's own detection
> row. What remains ungraded is *which point inside the band* each machine ships, and that is a
> narrower and still-honest claim. See `crates/spectrum/src/timing.rs`.

## `timing_tests-128k_v1.0.z80` — the 128 edition, and the file that caught a wrong constant

**The same suite, the same author, a different machine — and it sat in this directory unread by any
gate from 2026-09-01 until 2026-09-02.** When something finally ran it, the 128 was red on **62 of
70** rows: `Timing::SPECTRUM_128` carried `first_contended_t_state: 14361` where the hardware wants
**14362**, and an interrupt window of 32 where the hardware wants one in **`33..=43`**. Both are
corrected, and `crates/spectrum/tests/timing_oracle.rs` now grades all 71 of the file's rows on
every `cargo test`. **A corpus no gate reads is not evidence; it is a file.**

### Fetching

Not from the MrKWatkins mirror the 48K file comes from — that mirror's `Timing/` directory holds
exactly one `.z80` and it is the 48K one — and not from the origin, whose whole
`zxspectrum4.net/downloads/timing_tests/` directory 302-redirects to the site root. The only live
copy found is SoftSpectrum 48's:

```sh
mkdir -p testdata/timing
curl -fSL -o testdata/timing/timing_tests-128k_v1.0.z80 \
  https://softspectrum48.weebly.com/uploads/6/6/7/5/66753101/timing_tests-128k_v1.0.z80
shasum -a 256 testdata/timing/timing_tests-128k_v1.0.z80
```

| | |
|---|---|
| Size | 12960 bytes |
| SHA-256 | `fedc228ddef76cefb7b81dd6e18600cca2fd826fc18b4bc3f773cfdf2e7fffc4` |
| Verified | 2026-09-02 — hash reproduced, all 71 rows green |
| Licence | No licence text in the file and none on the page serving it, exactly as for the 48K suite. Fetched, gitignored and not redistributed here; the suite is Richard Butler's |

### It is a conversion, and the run is what validates the conversion

**Butler shipped the 128 edition as `.szx`. What this URL serves is a third party's `.z80`**, so
"is this file intact?" is a real question and not a formality. The answer is in its own detection
row, which reads **121**: `R` counts four-T-state iterations of a loop that fills the frame, so
carrying the identical program from the 48K's 69888-T-state frame to the 128's 70908 must shift the
reading by `(70908 - 69888) / 4 = 255` iterations, and `255 ≡ -1 (mod 128)` against the 48K's 122.
It does, exactly — which a corrupted or mis-converted file would not, and which
`the_128_corpus_is_the_128_edition_and_its_one_table_is_not_the_48ks_leftovers` asserts rather than
assumes. (It confirms the conversion, not the frame length: the reading is periodic in 512 T-states
of frame, so it separates 70908 from 69888 and not from 70396.)

### Two traps, both of which produce a plausible wrong answer rather than an error

- **It carries ONE expectation table, at `0xE200`.** The bytes at `0xE400` are the **48K's**
  `TYPE2 (Late)` table, left in place because the 128 edition was made by editing the 48K program.
  Pointing the 48K gate's table constants at this file compares a 128 against 48K hardware — red for
  a reason that has nothing to do with a 128, and near enough to look like a small modelling error.
  The evidence it is one table is in the program rather than the data: the 48K's classification
  lines are **deleted**, so the selector both files carry can never fire and the byte at 40004 is
  zero.
- **The 48K gate's positive control passes on this file for the wrong reason.** It asserts the two
  tables differ on almost every row, and they do — 71 rows wide — because one of them is a 48K's.
  A control that passes for the wrong reason is worse than no control, which is why the 128 has its
  own.

The suite has two tables because **real Spectrums have two behaviours**, one T-state apart, and
they do not sort by board issue: the authors record a cold machine reporting late and then early
once warm, and issues 3B, 4B and 6A appear in both classes. So the corpus cannot say which of the
two this emulator *should* be — it says which one it *is*, as a fact rather than an intention.

### Licensing

Fetched, gitignored, and not redistributed here, like every corpus except the ROM. The mirror
above is `MrKWatkins/EmulatorTestSuites`, which collects freely-distributable emulator test
programs; the suite itself is Richard Butler's.

---

## `testdata/tapes/` — third-party `.tap` tapes

**The external check on our reading of the `.tap` format.** Every other tape gate builds its
`.tap` bytes in the test, so a shared misreading of the block framing — a length word taken as
big-endian, the flag byte counted outside the length, an off-by-one in where a block ends — would
be invisible: the same misreading would sit on both sides of the comparison. A file somebody
else's tool wrote breaks that symmetry, and it does it with an assertion that owes nothing to this
project at all: **the parity byte**.

Every block on a real tape ends with the XOR of the bytes before it, applied by whoever recorded
it. So if this crate splits a 14300-byte block one byte wrong, emits its bits in the wrong order,
or skips the wrong number of sync pulses, the bytes recovered from the pulse train **fail somebody
else's checksum**. Same class of instrument as `testdata/snapshots/`, and the same reason both
exist.

### Fetching

```sh
mkdir -p testdata/tapes && cd testdata/tapes
base=https://raw.githubusercontent.com/MrKWatkins/EmulatorTestSuites/main/src/MrKWatkins.EmulatorTestSuites.Z80/Program
curl -fsSL -O $base/Raxoft/V1_2A/z80doc.tap \
             -O $base/Raxoft/V1_2A/z80memptr.tap \
             -O $base/MarkWoodmass/z80tests.tap
shasum -a 256 *.tap
```

| File | Bytes | SHA-256 | What it is |
|---|---|---|---|
| `z80doc.tap` | 14390 | `4b06ce9fd517fd5f5a86d4ff2ef05a3f0ab7ed20eb075b87b0623d42fbd3e4bd` | Patrik Rak's `z80test` v1.2a, documented-behaviour build |
| `z80memptr.tap` | 14390 | `444582ddfa4d05711b6235e743ddf68295231e97ba92cc838c5d09a106c9a10f` | the same suite's MEMPTR build |
| `z80tests.tap` | 5573 | `42ce7fc8393de83876b89015be54f7d0da3c8c5b07a075981c12dd9d84c8b95d` | Mark Woodmass's Z80 test program |

Verified 2026-09-01: all three downloaded, all three hashes reproduced, all three swept green —
12 blocks, 34329 bytes recovered from pulse trains, 12 parity bytes checked.

**Any `.tap` works**, and that is the point of a sweep rather than a list: a user's own tapes are
graded too. These three are here because they are freely distributable — a real game may not be
redistributed and none is expected to be.

### One of them is also executed, and that is a different gate

`z80memptr.tap` is used **twice**, and the two uses grade unrelated things. The sweep below reads
it as a *file*; `crates/spectrum/tests/memptr_oracle.rs` **runs it as a program** — loaded by the
real ROM's `LD-BYTES` off the `EAR` bit, executed on the whole machine, and its report read back
out of the display file. It is the first use of M6's tape loader as a general capability rather
than as its own milestone gate, and it reports `Result: 045 of 160 tests failed.` on this core.

The distinction is worth keeping straight: **the sweep passing says nothing about the CPU**, and
until that oracle existed this file's presence in `testdata/` was recorded by `docs/STATUS.md` as
an instrument that was documented and never run. `z80doc.tap` and `z80tests.tap` are still swept
only — running them is a second oracle nobody has written, and `z80doc` in particular would
overlap `zex_oracle.rs` rather than add to it.

### What the gate does with them

`crates/spectrum/tests/tape_corpus.rs` is a **directory sweep**:

| | |
|---|---|
| every `.tap` present | must parse |
| every block | must come back out of the pulse train byte for byte, compared against the file read independently of `tape::tap` |
| every recovered block | must satisfy **its own parity byte** |
| a block whose flag is under 128 | must be a 19-byte header, and the corpus must contain both classes — so the flag-bit branch that chooses the pilot length is exercised by real data |

An empty directory **fails** rather than passing quietly, and each test prints what it covered.

### Licensing

Patrik Rak's `z80test` and Mark Woodmass's test program are freely distributable emulator test
software. As with every other corpus here they are fetched by whoever runs the tests, so this
repository redistributes nothing.

### Making absence a failure

The same rule, the same environment variable, the same `crates/testsupport` decision point:

| | |
|---|---|
| present | it runs |
| absent | **the gate fails**, naming the fetch instructions |
| absent, `ZX_CORPUS_ALLOW_MISSING=1` | the gate skips, printing why |
| absent, `ZX_CORPUS_ALLOW_MISSING=1`, `CI` set | **refused** |
| `Z80_FUSE_ALLOW_MISSING` set at all | **refused** — obsolete |

All five rows were exercised on 2026-09-01 by moving the directory aside: undeclared absence
exited 101, the declared opt-out exited 0, the opt-out under `CI` exited 101, and the obsolete
spelling exited 101. **Checked rather than asserted**, because this file has carried a fictional
absence table before — see the note under the ROM section.

One thing that check surfaced and is worth stating, because it is the hazard this whole policy
exists for: the skip notice is a `println!`, and **libtest captures stdout for passing tests**, so
the declared-absence run looks byte-identical to a verified one unless you pass `--nocapture`. The
notice is not the guard. The guard is that an *undeclared* absence moves the pass/fail surface.

---

## `testdata/tzx/` and `testdata/games/` — `.tzx` tapes, and the one corpus with no fetch

`.tzx` is the format that can carry a **turbo loader** — per-block pilot, sync and bit
timings — which is what `.tap` cannot express at any speed and what most commercial titles
are distributed as. `crates/spectrum/tests/tzx_corpus.rs` sweeps both directories for `*.tzx`
and grades every file it finds.

**What this corpus is *not* load-bearing for, and it is worth saying before the fetch section
below explains why there is no fetch.** A reader meeting a `.tzx` corpus that may be empty could
reasonably conclude that turbo loading is therefore graded by nothing here. That conclusion would be
wrong: `crates/spectrum/tests/tzx_turbo_load.rs` builds its own turbo tape *and its own 124-byte Z80
loader* in code and runs both on the real machine, so it needs no corpus at all and runs on every
clone. **The turbo format is graded without this directory.** What this directory could add, and
what nothing else can, is a turbo **game** — a file somebody else's tool wrote, loaded by a loader
nobody here wrote — and that is the residue `docs/STATUS.md` records at T4.

### There is no fetch command here, and that is the finding rather than an omission

Every other corpus in this file is fetched from a named URL with a recorded SHA-256. This one
is not, and the reason is worth stating plainly rather than leaving as a blank section:

- **The `.tzx` files that exist in the wild are games**, and game images may not be
  redistributed — the same rule that keeps `testdata/games/` gitignored. **Gitignored is not
  empty**, and the difference matters here: the directory holds whatever the person running the
  tests has put in it, and `.gitignore`'s blanket `testdata/**` is what keeps that out of the
  repository rather than off the machine. This file used to say *"gitignored and empty"*, which
  described a clone and was read as describing the corpus.
- **The sources this workspace already uses have none.** `MrKWatkins/EmulatorTestSuites`,
  which supplies the `.tap` corpus and the timing suite, was enumerated in full on 2026-09-01:
  2178 files, **zero** `.tzx`. So the obvious place to point a `curl` at does not have one.

### Where the local files came from, and who owns that record

**`testdata/games/PROVENANCE.md` owns it, file by file** — what each one is, its size and SHA-256
taken from the bytes on disk, where it came from, and the licensing search behind it. This file
does not restate that table, because a second copy of a per-file list is a second thing to go stale
and the sentences here have to stay true whatever the table says. What belongs here is the policy,
stated whole so it survives the table being wrong:

- **No game in `testdata/games/` is committed or may be redistributed, whatever is in it.**
  `.gitignore`'s blanket `testdata/**` covers the directory, and two negations re-admit exactly
  one path — `PROVENANCE.md`, the record itself. Every game there is ignored, and that holds for
  one file or a dozen.

  **Check that with `git ls-files -o --exclude-standard testdata/games/`, which returns exactly
  that one path — not with `git check-ignore`, which this bullet used to cite.** Two traps, both
  met rather than imagined. `check-ignore` **exits 0 for any match, including a negation**: the
  status cannot tell *ignored* from *explicitly re-included*, and only the rule it prints can.
  And the argument form changes the answer — `testdata/games` reports
  `.gitignore:69:!testdata/games/` while `testdata/games/`, with a trailing slash, reports the
  blanket `.gitignore:3:testdata/**`. The trailing-slash form is what was pasted here as proof,
  so the evidence said the opposite of what was true while looking exactly like evidence.
  (`git status --short` collapses the directory to `?? testdata/games/`; only `-uall` shows the
  one file.)
- **Availability is not permission, and the distinction is the whole point of keeping a record.** No
  sourced rights-holder permission was found for anything in that directory. What exists is
  availability, archives that disagree with each other about it, and undocumented ownership chains.
  None of it approaches what the Sinclair ROMs rest on — a permission quoted in full above with its
  author, forum, date, conditions and hedged scope — and the licensing is **weaker still** for a
  file taken from a commercial ROM-download site, which publishes no rights status to weigh at all,
  than for one from World of Spectrum or the Internet Archive, which at least state what they
  attempt and can be quoted.
- **Not every file has a recoverable origin, and that is written down as a finding rather than
  smoothed over.** Where an origin could be recovered it was **recovered rather than invented** —
  from macOS `kMDItemWhereFroms` metadata, the referring site it records, and, in one case, the
  tape's own `ID 32` archive-info block naming publisher, year and rip author, read out of the file
  rather than looked up. Where the URL attribute is simply absent, the record says **origin
  unrecorded** and gives what is known, which is the downloading application and the moment from the
  `com.apple.quarantine` record. Inventing a plausible archive for those would be exactly the tidy,
  wrong provenance the record exists to prevent, and a wrong licensing claim is worse than a wrong
  technical one because it produces a redistribution nobody was entitled to make.
- **No gate depends on any of it**, which is why the policy can be this strict without costing
  coverage. `tzx_corpus.rs` sweeps the directory and skips when there is nothing to sweep, and the
  one other reader of it is `#[ignore]`d as a measurement rather than a gate.

**`PROVENANCE.md` now ships, and the sentence that stood here rested on its not shipping.** It
read: *"`PROVENANCE.md` is itself covered by `testdata/**`, so it lives beside the corpus it
documents and a fresh clone has neither. That is the right place for it — a per-file record is
worth exactly as much as the files it describes — and it is the reason the policy above is written
out here instead of delegated by reference."* **Both halves of that premise have evaporated.**
`.gitignore` gained `!testdata/games/` and `!testdata/games/PROVENANCE.md`, so a clone gets the
record and still none of the games.

**The conclusion survives — on the reason given four sentences above, not on this one.** The policy
is written out here because this file owns the *policy* and `PROVENANCE.md` owns the *per-file
record*, and the two have different lifetimes: the sentences here have to stay true whatever the
table says. That was already the stated reason for not restating the table, and it never needed the
clone argument, which was a second and weaker justification that happened to be true and has
stopped being. Recorded rather than quietly deleted, because **a conclusion that outlives its
stated premise** is a shape this project keeps catching, and the useful part is noticing that the
premise was doing no work — the paragraph would have read the same with it removed on the day it
was written.

### So this sweep is opportunistic, and it says so

It is the **one** corpus here that does *not* use `crates/testsupport`'s absence policy. That
policy makes a missing corpus a failure naming the fetch, and it is right everywhere else —
but there is no fetch to name, so applying it would make every fresh clone fail with
instructions nobody can follow. That is a worse failure than the one the policy guards
against.

| | |
|---|---|
| files present | every one must parse, on **both** models — and the ROM's own `LD-BYTES` must load the first standard-speed block of every file that reaches one, with **at least one file reaching one** |
| no files | the sweep **prints that it verified nothing** and passes |

The second row is a deliberate departure from this file's one-convention rule, and it is
recorded in `docs/M6.md` rather than only here. **Drop any `.tzx` into either directory and it
is graded immediately** — including a game, which is the case this exists for.

### The floor in the first row, and why it is not `every file`

Added 2026-09-01, after the sweep was measured doing less than its green suggested. With the two
local games present it reported *"the ROM loaded the first block of **1 of 2** files"* and exited
**0**; with a corpus it could grade nothing in at all it reported *"0 of 1"* and **still exited
0**. A sweep that covers half of what it is handed, and says so only in a line nothing asserts on,
is the shape `docs/STATUS.md` records three times.

Two changes, and they close different halves:

- **The scan walks the metadata preamble.** `MarioBros.tzx` opens with an archive-info block and a
  text description, and the scan only ever looked at file offset 10 — so it was skipped in
  silence. The block IDs that carry **no signal** are stepped over, from the format description's
  own length column, transcribed in the test rather than read from the module under test. Blocks
  that carry signal are never stepped over: starting a load mid-tape would report a pass for a
  file the sweep never reached.
- **`loaded > 0` when the corpus is non-empty.** Deliberately not `loaded == files.len()` — a file
  whose first signal block is a *turbo* one genuinely cannot be graded by `LD-BYTES`, and
  demanding otherwise would make a legitimate corpus red. Such a file is now named **with the
  block ID that stopped the walk** rather than counted.

The floor sits after the absent-corpus return, so the departure in the second row is untouched: an
absent corpus still skips, because there is still no fetch to name. What the floor catches is the
other failure — a corpus that is **present and not actually graded**.

### What it grades that nothing else can

Every other `.tzx` gate builds its bytes in the test, so a shared misreading of the block
framing sits on both sides of the comparison and is invisible. `docs/STATUS.md` records that
measured rather than assumed: the whole third-party *snapshot* corpus stayed **green** under a
symmetric mutation.

A file somebody else's tool wrote breaks that symmetry, and this sweep uses the strongest break
available — **the real ROM loads the block, and checks its parity byte.** The file is theirs,
the loader is Sinclair's, and neither is ours.

---

## `testdata/roms/` — the Sinclair ROMs

**This is the one directory here that is committed.** The rule at the top of this file —
nothing in `testdata/` is in the repository — has one exception, and `.gitignore` names each
file by name rather than by glob: `48.rom`, `128-0.rom` and `128-1.rom` may live here on the
strength of the permission quoted immediately below. Game images may not; the user supplies
their own.

`48.rom` is the 48K's single 16 KB ROM: Sinclair BASIC, the editor, the character set at
`0x3D00`, and the interrupt handler the 50 Hz frame interrupt vectors into. It is the **M5**
gate — a machine that boots it to `© 1982 Sinclair Research Ltd` has a working memory map
and a working screen.

| | |
|---|---|
| Size | 16384 bytes |
| SHA-1 | `5ea7c2b824672e914525d1d5c419d71b84a426a2` |
| CRC-32 | `ddee531f` |

### `128-0.rom` and `128-1.rom` — the 128's ROM pair, added at M7

A 128 has **two** 16 KB ROMs and pages between them with bit 4 of `0x7FFD`. `128-0.rom` is the
128 editor — the menu, the tokeniser, `RAMTOP`-relative allocation, the AY driver — and
`128-1.rom` is 48 BASIC as the 128 ships it, which is what a machine executes after *48 BASIC*
is selected from that menu. They are the **M7** gate's corpus, and they are committed under the
same permission as `48.rom`, which names *"Spectrum 48/128"* affirmatively rather than reaching
them by inference.

| | `128-0.rom` | `128-1.rom` |
|---|---|---|
| Size | 16384 bytes | 16384 bytes |
| SHA-1 | `4f4b11ec22326280bdb96e3baf9db4b4cb1d02c5` | `80080644289ed93d71a1103992a154cc9802b2fa` |
| CRC-32 | `e76799d2` | `b96a36be` |
| SHA-256 | `3ba308f23b9471d13d9ba30c23030059a9ce5d4b317b85b86274b132651d1425` | `8d93c3342321e9d1e51d60afcd7d15f6a7afd978c231b43435a7c0757c60b9a3` |

**Every figure in that table was taken from the committed bytes on 2026-09-01, not transcribed
from the source they were fetched from**, because this file's whole argument is that the
provenance of the bytes is checked rather than asserted:

```sh
for f in testdata/roms/128-0.rom testdata/roms/128-1.rom; do
  stat -f %z "$f"; shasum -a 1 "$f"; shasum -a 256 "$f"; crc32 "$f"
done
```

#### Provenance, and two properties that make these the right ROMs rather than merely two ROMs

Fetched from the **Fuse project's `roms/` directory** on 2026-09-01, byte-identical from three
further mirrors, and independently corroborated against Debian's `spectrum-roms` — which is the
same second-party licensing review already cited below for `48.rom`, and which ships *these two
filenames* under this permission.

Two checks were then run **against the committed bytes here**, because a hash proves two files
are the same and says nothing about which machine they came off:

- **`128-1.rom` is 48 BASIC with the 128's hooks in it, and the difference is legible.** It
  differs from the trusted `48.rom` in **1177 bytes**. **1157** of those are in `0x386E–0x3CFF`,
  which is `0xFF` filler in the 48K image — verified here: `set(48.rom[0x386E..=0x3CFF]) == {0xFF}`.
  The remaining **20** are at `0x4B–0x4C`, `0xB52–0xB55`, `0x1349–0x134C`, `0x1B7D–0x1B80`,
  `0x1BF4–0x1BF6` and `0x2646–0x2648` — six `JP`/`CALL` hooks redirecting into that reclaimed
  space. **The character set at `0x3D00–0x3FFF` is byte-identical.** So the machine that boots
  this ROM is running 48 BASIC, not a lookalike.
- **It is the Sinclair toastrack, not the Amstrad +2.** `128-0.rom` carries
  `© 1986 Sinclair Research Ltd` — the string *"1986 Sinclair"* is at `0x563`, and the byte at
  `0x561` is `0x7F`, the Spectrum's own `©` glyph — and **neither ROM contains the string
  `Amstrad` anywhere.** `128-1.rom` carries *"1982 Sinclair"* at `0x153B`, which is the message
  the 48 BASIC path reaches.
  > **One relayed figure is *not* verified here and is marked rather than repeated as fact:**
  > that `128-0.rom` differs from `plus2-0.rom` in 14773 of 16384 bytes. **`plus2-0.rom` is not
  > on this machine**, so nothing here can check it. What *is* checked is the pair above — a
  > 1986 Sinclair string present and no Amstrad string at all — which settles the same question
  > without needing the +2 image. Whoever has `plus2-0.rom` can close the note; nobody here can.

**Why this distinction is worth the paragraph rather than a footnote.** The permission is
affirmative for the 48 and the 128 *by name* and reaches the `+` machines only through
*"produced the + machines ourselves"*. A `plus2-0.rom` committed under the belief that it was a
128 ROM would be a redistribution resting on the weaker half of a hedged 1999 usenet answer —
which is precisely the failure `.gitignore`'s by-name list exists to prevent, arriving through a
filename rather than through a glob.

### The permission this rests on, quoted — and the acknowledgement it asks for

**This section is the single source for the licensing claim, and everywhere else in the
repository points here rather than restating it.** `.gitignore`, `docs/ARCHITECTURE.md`,
`README.md`, `docs/M6.md` and this file's own opening paragraph each used to assert
*"Amstrad has explicitly permitted redistributing the Sinclair ROMs with emulators"* — five
copies of one claim, with **no quotation, no author, no date and no URL** between them. Compare
what this same file demands of the ROM's *bytes* — the table directly above and *Provenance* below:
size, SHA-1, CRC-32, two independent mirrors fetched and compared, and a stated reason for taking
that trouble. The provenance of the **bytes** was
documented to this project's usual standard and the provenance of the **right to ship them** was a
sentence — which is worse than the usual version of that defect, because a wrong technical claim
produces a bug and a wrong licensing claim produces a redistribution nobody was entitled to make.

**The acknowledgement, made here in the terms the permission requests:**

> **Amstrad have kindly given their permission for the redistribution of their copyrighted
> material but retain that copyright.**

That sentence is not decoration and it is not a summary of the permission — it is a thing the
permission asks for, in those words, and until now this repository made it nowhere. It is repeated
in [`../README.md`](../README.md)'s licensing section because the permission asks that *"the
program/manual"* carry it, and the manual is the front door rather than a corpus note. **A required
notice is the one kind of text that is meant to exist in more than one place**; the *sourcing*
below is not, and lives only here.

#### The statement

**Cliff Lawson of Amstrad plc**, `<clawson@amstrad.com>`, posted to **`comp.sys.sinclair`** on
**31 August 1999**, under the subject *"Amstrad ROM permissions"*, as answers to eight numbered
questions. The sentence relied on is his answer to question 1:

> *"Amstrad are happy for emulator writers to include images of our copyrighted code as long as
> the (c)opyright messages are not altered and we appreciate it if the program/manual includes a
> note to the effect that 'Amstrad have kindly given their permission for the redistribution of
> their copyrighted material but retain that copyright'."*

**The conditions that travel with it:**

| Condition | State here |
|---|---|
| The copyright messages must not be altered | `48.rom` is committed byte-identical to both mirrors; `128-0.rom` and `128-1.rom` byte-identical to four. Every SHA-1 and CRC-32 above was taken from the committed bytes, and both 128 copyright strings were located in them by offset |
| The acknowledgement note is requested (*"we appreciate it if"*) | **made above**, and in `README.md` |
| Distribution is for emulator use | this is an emulator |
| The ROM code is **not to be sold** — a shareware fee for the emulator author's own work is explicitly fine | nothing here is sold |
| Modification is permitted (*"If they choose to modify the behaviour in any way then that's entirely up to them"*), and modified ROMs may be redistributed under the same copyright proviso | not exercised |

#### Scope — stated as the source states it, not tidied

**The machine list is in the *question*, not in the answer, and the answer is hedged.** Question 5
asks about Interface 1/2, the ZX80, the ZX81 and the Spectrum 48, 128, +2, +2A and +3. What Lawson
replies is:

> *"I think Amstrad only bought the rights to Spectrum 48/128 from Sinclair and then produced the
> + machines ourselves. I do not believe the (c) for ZXs or IF1/2 has anything to do with
> Amstrad."*

On clones (question 7), in full: *"Ask Timex."*

So: **Spectrum 48 and 128 are affirmative**; the `+` machines are covered by *"produced… ourselves"*
rather than by being named as licensed; and the **ZX80, the ZX81 and the Interface 1 and Interface 2
ROMs are disclaimed as not Amstrad's copyright at all.** That last is a *stronger* reason not to
ship them than an exclusion from the permission would be — if Amstrad does not hold the rights, no
Amstrad permission can reach those ROMs **however** it is worded. It is why `.gitignore` lists every
committed ROM **by name** rather than un-ignoring `*.rom`: an `if1.rom` is exactly the sort of file
an emulator project acquires in the normal course of its work, it matches the glob, and no wording
of this permission covers it.

#### How it was read, and the gap that is left

- The wording above is transcribed from
  [`z00m128/zxs-rom/LICENSE.md`](https://github.com/z00m128/zxs-rom/blob/master/LICENSE.md),
  which reproduces the **whole thread** — all eight questions with their answers — and was read end
  to end on 2026-09-01.
- Corroborated by **Debian/Ubuntu's [`spectrum-roms` copyright file](https://launchpad.net/ubuntu/noble/+source/spectrum-roms/+copyright)**,
  which ships `48.rom`, `128-0.rom` and `128-1.rom` under this permission after a distribution's
  own licensing review. That is a second party acting on the same text, not a second fan page.
- **The gap, stated rather than left implicit: no archived copy of the original usenet article has
  been read.** Both of the above are *reproductions*. For a technical claim two agreeing
  reproductions would be plenty; for a licensing claim the quotation **is** the thing relied on, so
  the distinction survives into this file rather than being smoothed away. What would close it: the
  article itself from a usenet archive, quoted here with its Message-ID.
- **The [World of Spectrum permits page](https://worldofspectrum.net/permits/) is not a source for
  this text and must not be cited as one.** Re-fetched 2026-09-01 with a browser user-agent: **HTTP
  200**, 52454 bytes, and it does **not** carry the permission's wording. It says only that Amstrad
  *"allow free distribution of the ZX Spectrum ROMs (see the message from Amstrad as posted in
  comp.sys.sinclair for the details)"* — it points at the posting rather than reproducing it. An
  earlier reading recorded that page as returning 403; the 403 was not what hid the primary text,
  because the page never had it.

### Provenance

Fetched from two independent mirrors and compared byte for byte, because a ROM is the one
corpus here whose contents nothing else validates — the FUSE and `zex` harnesses check the
structure of what they load, and a subtly wrong ROM would simply fail to boot with no clue
why.

```sh
mkdir -p testdata/roms
curl -fSL -o testdata/roms/48.rom \
  "https://sourceforge.net/p/fuse-emulator/fuse/ci/master/tree/roms/48.rom?format=raw"

# The second mirror the committed copy was checked against:
#   https://raw.githubusercontent.com/archtaurus/RetroPieBIOS/master/BIOS/48.rom
shasum -a 1 testdata/roms/48.rom   # 5ea7c2b824672e914525d1d5c419d71b84a426a2
```

### Making absence a failure

The same rule as every other corpus, through the same environment variable — there is one
convention here, not one per corpus:

| | |
|---|---|
| present | it runs |
| absent | **the gate fails**, naming the fetch instructions |
| absent, `ZX_CORPUS_ALLOW_MISSING=1` | the gate skips, printing why |
| absent, `ZX_CORPUS_ALLOW_MISSING=1`, `CI` set | **refused** |

> **This table used to be fiction, and it is worth recording why.** It described the rule
> above while **no code implemented it for this corpus**: nothing in `crates/spectrum`
> referenced either variable, because nothing in `crates/spectrum` read the ROM. The boot gate
> was an *example*, and `cargo test` builds an example without ever calling its `main` — so
> deleting `48.rom` left the suite green at 72 passed and the test count unchanged. Prose
> asserting a protection the code does not provide is the exact defect class `docs/STATUS.md`
> catalogues, and it had been introduced in the document whose job is to describe the guard.
>
> Both halves are now real: `crates/spectrum/tests/boot.rs` is a `#[test]`, and it reaches the
> ROM through the shared policy in `crates/testsupport`.

Absence should not happen, since the file is committed — which is exactly why the guard
matters. A committed corpus that a sparse checkout or a stray `rm` removes produces the
same green-and-verifying-nothing run that `docs/STATUS.md` records the FUSE gate producing,
and "it is committed, so it is there" is the kind of reasoning that stops people looking.

### Running the gate

```sh
cargo test -p spectrum --test boot
```

That is the gate. It asserts the copyright message appears **and the frame it appears on**,
which is the number that discriminates: deleting contention entirely still reaches the
message, on frame 85 instead of 87.

The example prints the same run with the screen as text and a real-time multiple, and is a
demonstration rather than a check:

```sh
cargo run --release -p spectrum --example boot -- testdata/roms/48.rom
```

Prints the screen as text, the CPU state, the frame the copyright message first appeared
on, and the emulated-to-real-time ratio; exits non-zero if the message never appeared.
