# `testdata/games/` — commercial game images. **Fetched, never committed.**

Same rule as every other corpus in `testdata/`: fetched by whoever runs the observation, and
covered by `.gitignore`'s blanket `testdata/**`. Checked rather than reasoned about:

```
$ git check-ignore -v testdata/games/ManicMiner.tap
.gitignore:3:testdata/**	testdata/games/ManicMiner.tap

$ git ls-files -o --exclude-standard testdata/games/
testdata/games/PROVENANCE.md
```

**This file is the one thing in this directory that ships, and that is deliberate.** The second
command is the load-bearing one: it lists what git would actually offer to add, and it returns
exactly one path — this one. The games are other people's copyright and the blanket rule keeps
them out. The record *about* them is this project's own writing, and a provenance record that
exists only on the machine that wrote it documents nothing, so `.gitignore` re-includes it by
name, beneath the ROM block and on the reasoning argued there.

> **Read `check-ignore`'s pattern, never its exit status.** It exits `0` when a pattern matched,
> and a **negation** is a pattern — so `git check-ignore -v testdata/games/PROVENANCE.md` exits `0`
> while naming `!testdata/games/PROVENANCE.md`, which is an acquittal and not a conviction. The
> argument form matters too: `testdata/games` reports the negation that un-ignores the directory,
> and `testdata/games/`, with the trailing slash, reports the blanket rule instead. Two spellings
> of one question, two different answers, one of them misleading. `git ls-files -o` has no such
> edge and is why it is the command pasted above.

**So read this as a stranger would, because a stranger is who it is for.** Every claim below is
one of two kinds: something you can re-run for yourself — the fetch commands, the hashes, the
block and byte dumps — or something observed once on the machine these files live on, which you
can only take on testimony. The sections resting on the second kind say so in their own words
rather than leaving you to notice.

**Nothing here may be committed except this file, and the licensing below is why — it is
materially weaker than the ROM's.** `testdata/README.md` rests the ROMs on a permission it quotes
in full, with author, forum, date, conditions and scope. **There is no equivalent for Manic Miner.
None was found.**

---

## Licensing status of Manic Miner — stated precisely

**No sourced rights-holder permission for redistribution was found anywhere.** What exists is
availability, two archives that contradict each other about it, and an undocumented ownership
chain. Each of the following was fetched and read directly on 2026-09-01, not relayed:

| Source | What it says, verbatim | What that is |
|---|---|---|
| **Spectrum Computing**, [entry 3012](https://spectrumcomputing.co.uk/entry/3012/ZX-Spectrum/Manic_Miner) — the ZXDB front end, and it names **this exact file**, `ManicMiner.tap.zip`, under *Bug-Byte Software Ltd (UK) 1983* | *"Unfortunately these files are distribution denied and you can't download them."* | An explicit **refusal** to distribute |
| **World of Spectrum** `infoseek` API, record id 2058, publishers Bug-Byte / Software Projects / Ventamatic / Mastertronic / EDOS | `"availability_text": "Available"` (`availability_id` 1) | An archive's **classification**, not a grant |
| **World of Spectrum** `infoseek` API, publisher records | `Bug-Byte Software Ltd` → `distribution_status: "Unknown"`, last owner *Grandslam Interactive Ltd*; `Software Projects Ltd` → `distribution_status: "Unknown"` | **No named permission** from either publisher |
| **World of Spectrum**, [/archive](https://worldofspectrum.org/archive) | *"We also try our best to get permission from the copyright holders to distribute their software freely from this site."* | A stated **effort**, not a permission |
| **Internet Archive**, `zx_Manic_Miner_1983_Bug_Byte_Software` and `..._Software_Projects` | `licenseurl` and `rights` are both **absent** from the item metadata | **No stated licence** |

**The two archives that share the ZXDB dataset disagree about the same filename.** WoS's mirror
served `ManicMiner.tap.zip` with HTTP 200 while Spectrum Computing's page for the same file says
distribution is denied. That conflict is reported, not resolved — resolving it is not something a
fetch can do.

**No statement by Matthew Smith releasing the game, disclaiming rights, or permitting free
distribution was found.** The words were looked for and not found; that is different from their
not existing. The post-1988 ownership chain after Software Projects Ltd is undocumented in every
source reached, and one archival disassembly attributes the copyright to Bug-Byte Ltd rather than
to Smith — a conflict with the usual account, also reported rather than resolved.

**So: widely available, and no permission.** Availability is not permission and must not be
written up as though it were. Treat the game as fully copyrighted; fetch it to run it, and do not
redistribute it.

---

## What is here, and where each file came from

Sizes and SHA-256 taken from the bytes on disk on 2026-09-01, not transcribed from the source.

```sh
mkdir -p testdata/games && cd testdata/games
curl -fsSL -o ManicMiner.tap.zip https://worldofspectrum.net/pub/sinclair/games/m/ManicMiner.tap.zip
curl -fsSL -o ManicMiner.tzx.zip https://worldofspectrum.net/pub/sinclair/games/m/ManicMiner.tzx.zip
unzip -o ManicMiner.tap.zip && mv MANIC.TAP ManicMiner.tap
unzip -o ManicMiner.tzx.zip && mv "Manic Miner.tzx" ManicMiner.tzx
curl -fsSL -o ManicMiner_BugByte.z80 \
  https://archive.org/download/zx_Manic_Miner_1983_Bug_Byte_Software/Manic_Miner_1983_Bug_Byte_Software.z80
curl -fsSL -o ManicMiner_SoftwareProjects.z80 \
  https://archive.org/download/zx_Manic_Miner_1983_Software_Projects/Manic_Miner_1983_Software_Projects.z80
shasum -a 256 *
```

| File | Bytes | SHA-256 | Source | What it is |
|---|---|---|---|---|
| `ManicMiner.tap` | 33168 | `0229ca09bb87c58024d68b05a6a5ecd8ac93fb4932d937883722467f86277f4e` | World of Spectrum `pub/` mirror, inside `ManicMiner.tap.zip` (14047 bytes, `1fcb7eed…`) | **The one that matters.** Six standard blocks, no turbo loader — see below |
| `ManicMiner.tzx` | 33205 | `3c9bbf17c2a9f4c0aa10e2799d127c05d1b24de639a14adedc935c5667dfa724` | as above, from `ManicMiner.tzx.zip` (14220 bytes, `86fecf76…`) | **Loads.** A revision 1.10 file whose first block is an `ID 10` standard-speed block at offset 10, and which carries no archive-info at all. Also corroboration that the two WoS encodings agree |
| `ManicMiner_BugByte.z80` | 26676 | `4231bd7632def2953b5050cd9169ef08c567ee28c34d839bcbca70c795c980a2` | Internet Archive, `zx_Manic_Miner_1983_Bug_Byte_Software` | v1 snapshot, `PC=9303`. **Carries `KEMP=1` — see the warning below** |
| `ManicMiner_SoftwareProjects.z80` | 24560 | `edc383904c4087bb1185a03a4b8ebb1a21517c8ae16e3a14eb80133f3b19ec80` | Internet Archive, `zx_Manic_Miner_1983_Software_Projects` | Restores to a state that **does not run** — byte-identical output at frame 0 and frame 200 |

> **Correction, 2026-09-02.** The `ManicMiner.tzx` row previously read *"**Not loadable here** —
> this emulator reads `.tap` only."* It was true when it was written and stopped being true when
> `crates/spectrum/src/tape/tzx.rs` landed. It is corrected rather than quietly rewritten because
> the failure mode is the one this repository keeps cataloguing: **a document that states a
> capability as absent goes on being believed after the capability arrives**, and nobody re-reads
> a line that only says "no". Checked rather than assumed —
> `cargo test -p spectrum --test tzx_corpus` reports
> *"the ROM loaded the first block of 2 of 2 files"*, and those two files are `ManicMiner.tzx` and
> `MarioBros.tzx`.
>
> **That last sentence is a local result and will not reproduce on a fresh clone.** `tzx_corpus.rs`
> sweeps this directory and skips when there is nothing to sweep, so on a checkout with no corpus
> it prints that it verified nothing and passes. *Two of two* counts the files that happened to be
> here; it is not a property of the emulator anybody can re-observe without supplying their own
> `.tzx`. What a reader **can** re-run is the fetch recipe above, which produces `ManicMiner.tzx`
> from a named URL — and that alone makes the sweep report one of one.

---

## The other five games, and where they actually came from

The table above covers Manic Miner and nothing else, while five more games sit in this directory.
That is a worse gap than a wrong row: **a file whose entire subject is provenance, silently
omitting most of what it is supposed to account for.** They are added here, and where the origin
cannot be established it says so rather than guessing.

All five arrived by a **different route** from the Manic Miner files above — not by the documented
`curl` recipe but through a browser, which is why each carries a macOS quarantine record and the
`curl`-fetched ones do not. Sizes and SHA-256 are from the bytes on disk on 2026-09-02; the dates
are decoded from `com.apple.quarantine`, not from the filesystem's mtime, which has been rewritten
since.

**This whole section is testimony, and the hashes are the only part of it a reader can check.**
The evidence it rests on is macOS extended attributes — `com.apple.quarantine` and
`com.apple.metadata:kMDItemWhereFroms` — written by the browser that did the downloading, on the
one machine that did it. They are not in any file's bytes: copy a file over the network, unzip it,
or fetch it yourself and you get *your* quarantine record or none at all, never this one. So there
is no command in this document that reproduces the two columns on the right, and there was no way
to write one. What survives the journey is the SHA-256, and that is the column to use: if a copy
you hold matches a row, you hold the same bytes this document is describing, whatever either of us
knows about where they came from.

| File | Bytes | SHA-256 | Downloaded | Origin |
|---|---|---|---|---|
| `MarioBros.tzx` | 42566 | `fa6752652e7b38c8b1cd44fd747b6239bdf527683438e909d19536fbfa0b22a7` | Safari, 2022-07-06 07:32 UTC | **Recorded.** `com.apple.metadata:kMDItemWhereFroms` holds `https://static.downloadroms.io/a202207065D1qvYjT2J1/output.bin?attach=Mario Bros (1987)(Erbe Software)[a][re-release].tzx`, referred from `https://www.romsgames.net/` |
| `Exolon.tap` | 34140 | `944494e554b14ba5dfb308639b249c3df49f93ab2a453cdfe8687ee3acd9a4c1` | Safari, 2022-07-04 14:37 UTC | **Unrecorded** — see below |
| `Batty.tap` | 26112 | `4352f2eb942d3c0e644f75bdce123a9f40c02ff5d2a33940601f0086bce368c0` | Safari, 2022-07-04 14:38 UTC | **Unrecorded** |
| `Cybernoid.z80` | 88523 | `402d706f255a2bb700214b1333068342f551c86f0c51ac865a3a31dbaaf45ab1` | Safari, 2022-07-04 14:42 UTC | **Unrecorded** |
| `CybernoidII.tap` | 48033 | `bf416648617f2ec3fbc9de99db38c679d9777a9eb993864d53f2b1bc92a2de92` | Safari, 2022-07-04 15:04 UTC | **Unrecorded** |

**"Unrecorded" is the finding and not a placeholder.** Those four carry a `com.apple.quarantine`
attribute, which is why the downloading application and the moment are known to the second, but
their `kMDItemWhereFroms` attribute — the one that holds the URL — is **absent**. It was looked for
on each and is not there. So the source URL of four of this directory's six games cannot be
established from anything on the machine that holds them, and inventing a plausible archive for
them would be exactly the kind of tidy, wrong provenance this document exists to prevent. The
twenty-seven minutes that separate the first from the last say they came from one sitting on one
afternoon, and that is as far as the evidence goes.

**Nor can it be established later, which is why it is written down as closed rather than pending.**
The attribute is absent, not unread; there is no second place to look, no re-run that recovers it,
and no reader anywhere else can go and check — the thing that would have carried the answer is the
thing that is missing. A row saying *origin unrecorded* is the finished state of this enquiry.

**A quarantine timestamp says *when*, never *where from*.** The two attributes answer different
questions and only one of them is present on these four. Reading the moment of arrival as evidence
of a source is the specific mistake this table is arranged to prevent, and it is the mistake to
watch for in anything that cites this file.

### What `MarioBros.tzx` says about itself

Its own `ID 32` archive-info block, read out of the file rather than looked up:

| Field | Value |
|---|---|
| Title | `Mario Bros` |
| Publisher | `Ocean/ERBE The Hit Squad` |
| Year | `1987` |
| Language | `English` |
| Comment | `TZX by Miguel A. Garcia Prada` / `D.L.: M-15210-1987` |
| Description (`ID 30`) | `Created with Ramsoft MakeTZX` |

It is a revision 1.10 file. The metadata occupies its first 147 bytes, and `docs/M6.md` records
that preamble as the reason `tzx_corpus.rs` had to learn to walk past non-signal blocks before it
could grade this file at all.

### The licensing statement above applies to all six, and is weaker for these five

The Manic Miner section is written about Manic Miner, and its conclusion — **widely available, and
no permission** — carries over unchanged. For these five it is if anything **weaker**, and the
distinction should not be lost in a shared paragraph:

- The Manic Miner files came from World of Spectrum and the Internet Archive: archives that
  *attempt* to obtain permission, publish their status, and were quoted verbatim above. That is
  still not a grant, and it was not written up as one.
- `MarioBros.tzx` came from a commercial ROM-download site. Such a site makes **no claim of
  permission at all**, publishes no rights status to quote, and the file is a re-release image of
  a game published by Ocean. There is nothing here to weigh, which is a different situation from a
  conflict between two archives.
- The four whose URL is unrecorded cannot be assessed at all, because assessing a source requires
  knowing which one it was.

**Treat all six as fully copyrighted. Fetch them to run them; do not redistribute them.** All six
are covered by `.gitignore`'s blanket `testdata/**` and none is committed — checked rather than
reasoned about:

```
$ git check-ignore -v testdata/games/MarioBros.tzx
.gitignore:3:testdata/**	testdata/games/MarioBros.tzx
```

**No gate in this repository depends on any of the six, and that was checked rather than assumed.**
Only two places in the workspace read this directory at all:

- `crates/spectrum/tests/tzx_corpus.rs` sweeps it, and is explicit that it *"skips when there is
  nothing to sweep"* — a deliberate departure from the shared corpus policy, recorded in
  `docs/M6.md`, because there is no fetch instruction it could name for files that may not be
  redistributed.
- `crates/frontend/tests/speed_multiplier.rs` opens `ManicMiner.tap`, and that one is **not a
  gate**: it carries `#[ignore = "a measurement rather than a gate: it needs testdata/games and
  reports a wall clock"]`, so it does not run under `cargo test` unless it is asked for by name.

Everything else that could have wanted a commercial tape builds its own instead.
`crates/spectrum/tests/tzx_turbo_load.rs` is the clearest case: it is the turbo gate, a commercial
turbo loader is exactly what it is about, and it assembles both its tape and its loader in code —
because **a gate resting on a gitignored file is a gate that runs nowhere.**

---

## Manic Miner's tape is a plain ROM-loader tape, and that is the interesting part

`docs/MACHINE.md` says of item 5 of the verification plan that *"`.tap` cannot represent a turbo
loader at any speed, and most commercial titles are turbo-loaded"*, and concludes that a real game
stays at tier T4. **The general claim is right and Manic Miner is an exception to it.** Every block
is standard, every parity byte checks, and the ROM's own `LD-BYTES` loads all three:

| # | Flag | Bytes | Header |
|---|---|---|---|
| 0 | `0x00` | 19 | Program `ManicMiner`, length 69, **autostart line 10** |
| 1 | `0xFF` | 71 | the BASIC loader |
| 2 | `0x00` | 19 | Code `mmm`, length 256, start **22784** = `0x5900` — the attribute file, rows 8–15 |
| 3 | `0xFF` | 258 | the "MANIC" attribute art shown while loading |
| 4 | `0x00` | 19 | Code `mm1`, length 32768, start **32768** = `0x8000` |
| 5 | `0xFF` | 32770 | the game |

The BASIC loader, decoded from block 1:

```basic
 10 CLEAR 30000
 20 PAPER 0: INK 0: CLS: LOAD ""CODE: LOAD ""CODE
 30 RANDOMIZE USR 33792
```

and `33792` = `0x8400`, where the game begins `DI / LD SP,$9CFE / JP $85CC`.

### The game's controls, read out of the tape rather than recalled

Plain ASCII at `0x9D31` inside the `mm1` block — the scrolling instruction text:

> `MANIC MINER . . BUG-BYTE ltd. 1983 . . By Matthew Smith . . . Q to P = Left & Right . .`
> `Bottom row = Jump . . A to G = Pause . . H to L = Tune On/Off . . . Guide Miner Willy`
> `through 20 lethal caverns`

| Control | Keys | Half-row read |
|---|---|---|
| Left | `Q W E R T` | `0xFBFE` |
| Right | `Y U I O P` | `0xDFFE` |
| Jump | the whole bottom row | `0xFEFE` and `0x7FFE` |
| Pause | `A`–`G` | `0xFDFE` |
| Tune on/off | `H`–`L` | `0xBFFE` |

**A direction and jump are always in different half-rows**, so the unmodelled membrane ghosting
cannot reach this game's control scheme.

### Warning about the two `.z80` snapshots

**`ManicMiner_BugByte.z80` is not a fair test of this emulator and should not be used as one.**
It differs from the pristine tape image in seven bytes, and one of them is `KEMP` at `$8459`,
the game's *"a Kempston joystick is fitted"* flag, set to `01`. The game's own detection routine
at `$861C` — 256 reads of port `0x1F` OR-ed together, then `AND $20` — never ran on the emulated
Spectrum this snapshot restores; the flag arrived in the file. With it set, the game reads port `0x1F`, this ULA answers
`FLOATING_BUS_BYTE` = `0xFF`, and the game sees **left, right, up, down and fire all held at
once**: it auto-starts from the title screen and Willy cannot be moved.

Loaded from the tape instead, the detection runs, `0xFF` has bit 5 set, the game concludes there
is no joystick, and it plays normally. **The defect is in the snapshot, not in the emulator** —
but it looks exactly like a dead keyboard, so it is written down here.
