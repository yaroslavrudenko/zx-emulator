# `testdata/` — external test corpora

Nothing in this directory is committed **except this file and `testdata/roms/48.rom`**. The
corpora are third-party data with their own provenance and licensing, they are large, and they
are reproducible from an authoritative source — so they are fetched on demand instead. The ROM
is the one exception, because Amstrad has explicitly permitted redistributing the Sinclair ROMs
with emulators; it has its own section below, and `.gitignore` names it.

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

The sixteen tests around it — the CP/M shell, the report parser, and one failing case per
gate rule — are **not** ignored and run on every `cargo test -p z80` without needing the
corpus at all.

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

## `testdata/timing/` — the 48K timing test suite

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
suite's own tables. **68 hardware rows, 0 disagreements.**

**State what it grades precisely if you cite it.** Thirteen mutations bound it: 14333, 14334,
14336, 14337 and 14361 for `FIRST_CONTENDED_T_STATE` all go red and only **14335** is green, and
perturbing the delay pattern, making `Ula::fetch` a three-T-state read, or stopping internal
cycles contending all go red by 14–38 rows. But three mutations came back **green** — shortening
the interrupt window, and moving the interrupt and the window *together* — so the oracle grades
the **interval from `/INT` to the first contended T-state**, not the constant alone. The constant
is anchored; the frame's origin remains a convention and the interrupt window's length is still
ungraded.

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

## `testdata/roms/` — the Sinclair ROMs

**This is the one directory here that is committed.** The rule at the top of this file —
nothing in `testdata/` is in the repository — has one exception, and `.gitignore` names it:
Amstrad has explicitly permitted redistributing the Sinclair ROMs with emulators, so
`48.rom` may live here. Game images may not; the user supplies their own.

`48.rom` is the 48K's single 16 KB ROM: Sinclair BASIC, the editor, the character set at
`0x3D00`, and the interrupt handler the 50 Hz frame interrupt vectors into. It is the **M5**
gate — a machine that boots it to `© 1982 Sinclair Research Ltd` has a working memory map
and a working screen.

| | |
|---|---|
| Size | 16384 bytes |
| SHA-1 | `5ea7c2b824672e914525d1d5c419d71b84a426a2` |
| CRC-32 | `ddee531f` |

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
