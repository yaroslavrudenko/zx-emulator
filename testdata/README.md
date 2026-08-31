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

### Making absence a failure in CI

Absence is a skip locally and must be a failure in CI, or the conformance gate becomes a
green tick that proves nothing:

```sh
Z80_FUSE_REQUIRED=1 cargo test -p z80
```

### Licensing

FUSE is distributed under the GNU GPL. The vectors are not redistributed in this
repository — they are downloaded by whoever runs the tests, which is why this file exists
instead of a checked-in copy.
