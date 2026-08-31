# `testdata/` — external test corpora

Nothing in this directory except this file and `.gitkeep` is committed. The corpora are
third-party data with their own provenance and licensing, they are large, and they are
reproducible from an authoritative source — so they are fetched on demand instead.

`crates/z80/tests/` skips its conformance run with a printed explanation when the files
are absent, so a fresh clone still builds and tests green.

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

`zexall` is deliberately **not** wired up as a gate at M3. See `docs/STATUS.md`.

### Making absence a failure

Absence is a skip only when it has been declared, and the declaration is refused under CI —
otherwise a conformance gate becomes a green tick that proves nothing. This applies to both
corpora, through one shared decision point in `crates/z80/tests/common/vectors.rs`:

| | |
|---|---|
| corpus present | it runs |
| corpus absent | **the gate fails**, naming the fetch instructions |
| corpus absent, `Z80_FUSE_ALLOW_MISSING=1` | the gate skips, printing why |
| corpus absent, `Z80_FUSE_ALLOW_MISSING=1`, `CI` set | **refused** — the opt-out must never decide what a pipeline verifies |
| `Z80_FUSE_REQUIRED` set at all | **refused** — it is obsolete, and a variable that is set but no longer read is how a CI author believes a guard is armed when it is not |

`Z80_FUSE_ALLOW_MISSING` is accepted as any of `1`/`true`/`yes`/`on`, case-insensitively;
anything unrecognised is an error rather than a silent `false`.

### Licensing

The `zex` exercisers are Frank Cringle's `zexlax`/`zexall` work, redistributed widely under
permissive terms. As with the FUSE vectors they are fetched rather than committed, so this
repository redistributes nothing.
