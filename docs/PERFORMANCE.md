# Performance

Where the time goes, what it was measured to cost before and after the 2026-10-20 work, the gate
that holds it, how to measure it again, and what was tried and refused with its numbers. Every
figure carries the command that produced it; **re-run rather than quote** — a bare number is a
measurement with its subject deleted. The decisions behind each item are the judges' common
decision of 2026-10-20 (DECISION.md, items 1–14; N1–N4 are the next batch).

Host for every figure unless stated: Apple M3 Max (Mac15,9, 16 cores), macOS 25.6, rustc 1.98.0,
the release profile as committed (`opt-level = 3`, `lto = "fat"`, `codegen-units = 1`,
`panic = "abort"`, `overflow-checks = true`). The machine was shared; the load average is given with
each wall-clock figure, and the instruction counts do not depend on it.

## The budgets

| What | Budget | Where it stands (2026-10-20) |
|---|---|---|
| One emulated frame at 1× | 20 ms (50.08 Hz on a 48K) | `quiet_48k` **74 µs** wall, 0.37 % of the frame. At 1× the CPU cost of emulation is not what a person notices; cadence and audio continuity are |
| The desktop audio callback | 10.67 ms (512 frames at 48 kHz) | p99 ≤ 6 µs, max 10 µs, 0 underruns over 180 s at 1× — and **15** (91 ms missing) in a second 185 s run under heavier host load (below) |
| The audio queue | setpoint 50 ms, backstop 100 ms, one rule on both targets: a chunk is admitted only if it fits whole | `frontend::audio::queue_target` and `frontend::audio::admits`; `web/zx_page.js`; `the_backstop_admits_a_chunk_only_whole_under_100_ms` holds the two copies to one rule |
| The browser's render quantum | 2.667 ms on the AudioWorklet thread | no per-chunk allocation (the buffer pool); not re-measured in this batch |
| Flat-out tape loading (`F8`, `auto`) and the browser build | as fast as possible — the only places core speed is visible | below: −34 % instructions on a whole game load, wasm −42 % per frame |

**One desktop behaviour changed with the shared admission rule** (DECISION.md item 7): a catch-up
push — the pacer delivers up to four frames, about 80 ms, in one tick after a stall — that would take
the queue past 100 ms is now **dropped whole**, where the desktop used to admit it and let the ring
truncate it at its capacity. With the queue at its 50 ms setpoint, 50 + 80 > 100, so a catch-up tick
now loses about 80 ms of sound instead of about 30 ms; the browser always behaved this way.

## Before and after — the batched covered ticks and the exact tape edge

**Instructions retired per frame** — `sh web/perf_gate.sh --measure`, median of three slopes of
`/usr/bin/time -l <bench> --bench --exact --sample-count {500,1500} --sample-size 1 frame::<case>`.
Before = the branch base with the owner's uncommitted work; after = the finished tree.

| Case | Before | After | Δ |
|---|---|---|---|
| `quiet_48k` | 4,198,237 | 2,659,934 | **−36.6 %** |
| `quiet_128` | 4,256,211 | 2,695,600 | −36.7 % |
| `contended_48k` | 3,980,105 | 2,712,630 | −31.8 % |
| `drained_48k` | 4,378,853 | 2,841,056 | −35.1 % |
| `beeper_48k` | 4,289,350 | 2,933,201 | −31.6 % |
| `tape_reads_48k` | 3,899,341 | 2,581,008 | −33.8 % |
| `tape_playing_48k` | 4,455,203 | 2,711,074 | **−39.1 %** |
| `border_48k` | 4,017,710 | 2,661,891 | −33.7 % |
| `ay_128` | 4,805,303 | 3,312,798 | −31.1 % |
| `halt_48k` (new case) | 3,079,010 | 1,558,881 | **−49.4 %** |
| `flat_bus` (z80 `step`, per sample) | 2,230,037 | 1,999,658 | −10.3 % |

**Wall time, minimum of six interleaved runs** — `sh web/perf_ab.sh <base tree> . <cases>`, load
average 19–41:

| Case | Before | After | Δ |
|---|---|---|---|
| `quiet_48k` | 138.70 µs | 74.12 µs | −46.6 % |
| `contended_48k` | 143.90 µs | 86.58 µs | −39.8 % |
| `tape_playing_48k` | 147.20 µs | 87.37 µs | −40.6 % |
| `halt_48k` | 110.90 µs | 42.41 µs | −61.8 % |

**A whole tape load** — `/usr/bin/time -l zx-shot --media testdata/games/<game> --play-tape --keys
'J;LeftControl+P;LeftControl+P;Enter' --keys-after Enter --hold 30 --settle 600 --out x.ppm`:

| Game | Before | After |
|---|---|---|
| Exolon `.tap` | 53.81 G instructions, 2.19 s | **35.11 G (−34.7 %)**, 1.89 s |
| Manic Miner `.tap` | 48.48 G instructions, 1.96 s | **32.26 G (−33.5 %)**, 1.45 s |

**The browser build** — the machine compiled to `wasm32-unknown-unknown` with the release profile
and run under `node` (2,000 frames, min of six): **282.0 → 164.5 µs per frame (−42 %)**. The
machine boots a **16 KiB ROM of zeros**, so the CPU runs a stream of `NOP`s — the `quiet_48k`
workload, not a game frame; compare it with `quiet_48k`, never with a real game. The harness is not
in the repository (its one exported function needs `#[unsafe(no_mangle)]`, and `crates/page` is the
only crate this workspace lets carry `unsafe`); *How to measure* below gives it whole.

### What made the difference

- **Batched covered ticks** (`Bus::ticks`, DECISION.md item 3). Once a machine cycle has opened and
  its contention is charged, the T-states that follow are already paid for; the core used to report
  them one call at a time, each paying a `Tape::advance` and a clock step. They now arrive as one
  call where that is indistinguishable from the per-tick loop — a covered run with no tape edge
  inside — and per tick everywhere else. `docs/ARCHITECTURE.md` Decision 1's amendment carries the
  rule and the proof: a `proptest` that `ticks(n)` ≡ n × `tick` on tapes of 0–7 T-state pulses, a
  `Cpu<Ula>` against a per-tick `Cpu` over committed short- and zero-pulse fixtures, and the ten-
  workload `zx-shot` corpus byte-identical. **+11 code lines and +2 branches**, a recorded RULE-4
  exception taken under performance-first for this change alone.
- **The exact tape-edge stamp** (item 5, a separate change). Not a speed change (±0.03 %
  instructions): an edge inside a contention stall or a port delay is stamped at the T-state it
  happened rather than up to 12 T-states late. It changes the WAV on every tape load (at most 722 of
  32767, 46.6–58.7 dB below the signal) and nothing else: screen and machine state are
  byte-identical on all ten workloads, and the batched ticks plus the stamp equal the stamp alone.

## The gate

`web/gate.sh`'s PERF step runs `sh web/perf_gate.sh`: **instructions retired per frame, eleven
cases, each within 1 % of `web/perf-baseline.txt`**. Instructions, not microseconds, because the
same binary's wall floor moved 7–12 % between back-to-back runs at a load of 7 here (P-core against
E-core placement), while its instruction count moved at most 0.38 % under a load of 186. 1 % is
more than 2.5× the worst spread recorded and catches the 1–2 % class a 2 % threshold would pass.

It was proven able to go red before it was trusted — each mutation applied, its diff checked, the
gate run, the file restored and diffed back:

| Mutation | Effect | Gate |
|---|---|---|
| A P1-like read: `black_box(self.clock.t_states())` hoisted above the edge guard | +7.4 to +13.5 % on ten cases | **red** |
| **The literal P1 code** (DECISION.md item 2): `Ula::advance` calling `set_tape(level, t_states())` on every advance, the edge guard gone | +6.8 to +12.4 % on ten cases (2026-10-20, fix round 1) — less than P1's +19.6 % because the batched ticks call `advance` once per covered run | **red** |
| The batched ticks removed (`Ula`'s `ticks` override deleted) | +37 to +82 % | **red** |
| A synthetic +1.5 % (10,000 `black_box` iterations per `run_frame`) | +1.2 to +2.6 % | **red** — the size a 2 % threshold would let through |
| A bench that aborts after 600 of its 1,500 frames (fix round 1) | read as −90 % and **ok** before the gate kept the bench's exit status; now `FAIL … produced no count` | **red** |
| A baseline row of `0` or `abc` (fix round 1) | read as **ok** before (an awk division error printed nothing, and nothing was a pass); now `FAIL … could not compare` | **red** |

A count belongs to a binary: the gate skips (counted) on another architecture, and fails on another
compiler of the same architecture (below).
**So it does not run in CI, by design:** the baseline is `arm64 rustc 1.98.0`, and `ci/ci.yml`'s gate
job runs on `ubuntu-latest` (x86_64), where the step exits 3 and `web/gate.sh` counts a SKIP. It is a
pre-push gate on the machine the baseline was recorded on. The toolchain is the other half, and
the gate now treats it as a failure rather than a question: **same architecture with a different
rustc, or a baseline with no `# toolchain` header, is FAIL (exit 1)**; another architecture is SKIP
(exit 3); a match runs. A compiler change is therefore never silent — it is a commit that bumps
`rust-toolchain.toml` and re-records from the merge base together. The judges decided (Q4,
2026-10-20) to pin `rust-toolchain.toml` to `channel = "1.98.0"` — the MSRV, the same build
(`88d9e12ae`) as today's stable, so the recorded baseline stays valid — installed once with a bare
`rustup toolchain install` in the repository. **Pending:** that install is about 1.5 GB and waits for
the owner's approval, so the file still says `stable` today; until it is pinned, a `rustup update`
past 1.98.0 turns this gate red here, which is the intended prompt.

**Re-recording, the rule.** When the toolchain or the architecture changes, or a change makes any
case faster by more than 1 % (the gate prints `gain > 1 %: re-record the baseline` beside the row,
exit 0), record from the **merge base**, not from the head: `git worktree add --detach ../zx-base
"$(git merge-base HEAD main)"`, `sh web/perf_gate.sh --record` there, copy the baseline back, then
run the check on the head. Re-recording on a head that already regressed bakes the regression in;
leaving a gain unrecorded lets a later regression of that size pass green. Put the date in the
commit that carries the new baseline. **Wall time is still required evidence** for any change that
touches the hot path — `sh web/perf_ab.sh <merge-base tree> .`, interleaved, pasted beside the
change — and never a verdict: instructions miss layout and branch effects (the bounded prefix
dispatch cost +0.4 % instructions and +3.3 % wall).

## How to measure

| Question | Command |
|---|---|
| Instructions per frame, every case | `sh web/perf_gate.sh --measure` |
| Did this change regress? | `sh web/perf_gate.sh` (or `sh web/gate.sh`) |
| Wall time against the merge base | `git worktree add --detach ../zx-base "$(git merge-base HEAD main)"` then `sh web/perf_ab.sh ../zx-base .` |
| One frame in a browser engine | the harness below, built for `wasm32-unknown-unknown`, then `node wasmbench.mjs <a.wasm> <b.wasm>` |
| Real-time cadence and audio at 1× | `ZX_TELEMETRY=1 cargo run --release -p frontend --bin zx -- <rom> <game>`; read the `zx telemetry` and `zx audio telemetry` lines on stderr |
| Did the output change? | the `zx-shot` corpus: `--out x.ppm --snapshot x.z80 --wav x.wav` on the same arguments for both trees, then `cmp` each file |

**The browser-engine harness**, outside the workspace (a scratch directory; `crates/page` is the only
crate allowed `unsafe`). `Cargo.toml` — `[package] name = "wasmbench"`, `edition = "2024"`,
`[lib] crate-type = ["cdylib"]`, `spectrum = { path = "<repo>/crates/spectrum" }`, an empty
`[workspace]`, and the repository's `[profile.release]` copied (`opt-level = 3`, `lto = "fat"`,
`codegen-units = 1`, `panic = "abort"`, `overflow-checks = true`). `src/lib.rs`:

```rust
use spectrum::Spectrum;
static mut M: Option<Spectrum> = None;
#[unsafe(no_mangle)]
pub extern "C" fn run(frames: u32) -> u32 {
    #[allow(static_mut_refs)] // single-threaded wasm, one machine
    let m = unsafe { M.get_or_insert_with(|| Spectrum::new(&[0u8; 16384]).expect("rom")) };
    m.run_frames(u64::from(frames));
    m.frame_t_state()
}
```

Build with `cargo build --release --target wasm32-unknown-unknown`; `wasmbench.mjs` instantiates each
`.wasm`, warms each with `run(300)`, then six interleaved rounds of `run(2000)` timed with
`process.hrtime.bigint()`, and prints the minimum µs per frame of each.

## At 1× in a window (DECISION.md item 8)

`ZX_TELEMETRY=1 zx testdata/roms/48.rom testdata/games/ManicMiner_BugByte.z80`, 180 s on
2026-10-20, the built-in ProMotion display (measured interval p50 8.312 ms — 120 Hz), load average
57–113, with the merged instrument (`crates/page/src/telemetry.rs`):

| Figure | Value |
|---|---|
| Frames run | 8,864 in 180 s — 50.08 Hz |
| Dropped | 3, all in the first window (a 140.7 ms start-up tick); 0 after |
| Frame interval | p50 8.312 ms; p99 9.6 ms in the median window, 18.3 ms worst |
| Tick (work per display tick) | p99 0.27 ms |
| Audio callback | 16,414 callbacks; p99 ≤ 6 µs, max 10 µs |
| Underruns / missing samples / `audio_lost` | **0 / 0 / 0** |

**Not measured, and said plainly:** 60 Hz — the window opens on the main display and the 60 Hz
monitor was not used; the browser — the page has no `ZX_TELEMETRY` reader. Whether it *sounds*
continuous is a person's observation, not this table's.

> **Corrected and extended 2026-10-20 (fix round 1).** *"The built-in ProMotion display"* above was
> an inference from the 8.3 ms interval, not an observation: on this host the window opens at
> (452, −1257), which is the **60 Hz DELL P2723QE** above the built-in panel (`System Events`
> reports the position; `NSScreen` gives that panel `maximumFramesPerSecond = 60` and the built-in
> 120). The loop ticks every 8.2–8.3 ms on the 60 Hz panel all the same — miniquad's swap does not
> follow the panel this window sits on — so **a 60 Hz cadence (16.7 ms ticks, one tick in six with
> no frame) does not occur on this machine** without changing a display's refresh rate, which was
> not done. Where the first run's window sat was not recorded. A second run of the same command,
> 185 s on the finished tree, window confirmed on the 60 Hz DELL, load average 18 rising to 68
> (other work on the host):
>
> | Figure | Value |
> |---|---|
> | Frames run | 9,415; dropped 3, all in the first window (a 135 ms start-up tick) |
> | Frame interval | p50 8.2 ms; p99 15.9 ms in the median window, 19.2 ms worst; max 57.6 ms after start-up |
> | Tick | p99 0.28 ms |
> | Audio callback | 17,344 callbacks; p99 ≤ 3 µs, max 8 µs |
> | Underruns / missing samples / `audio_lost` | **15 / 4,361 / 0** — in 8 of 37 report windows, each beside an interval p99 of 11–18 ms; 4,361 samples is 91 ms of silence in 189 s |
>
> The underruns are new against the first run's 0 and are recorded, not explained: the frame path
> dropped nothing and lost no audio at the queue, so the shortfall is at the device ring while the
> loop's own ticks stretched. Next batch, measure-first.
>
> **The browser was not measured, and cannot be the same way.** `crates/page/src/telemetry.rs`'s
> one reader is `std::env::var_os("ZX_TELEMETRY")`, which is always `None` on
> `wasm32-unknown-unknown`, so the page prints no figures at all; and a headless Chrome launched for
> the purpose has no audio device, so an underrun count from it would describe nobody's browser.
> Measuring it needs a page-side switch (a `?telemetry` query flag) and a worklet that counts its
> own starved quanta — new code, not a measurement; next batch.

## Refused, with the numbers — do not retry without new evidence

| Idea | Measured | Why refused |
|---|---|---|
| `overflow-checks = false` | `flat_bus` +31 % faster on the bare core, but `quiet_48k` 146.1 vs 148.1 µs floors — nothing on the real machine | the checks cost the shipped machine nothing measurable |
| `-C target-cpu=native` | cycles within run-to-run noise on quiet, contended and AY | no gain, and a binary that faults on another CPU |
| Replace the runtime `%` in `Timing::delay` with a constant | `contended_48k` cycles 520–543 k vs 521–534 k | null |
| Force-inline `Ula::advance` together with batched ticks | worse than batching alone (3.13 M instructions on `quiet_48k`) | measured worse in combination |
| Fuse `render` and the RGBA conversion | 12 µs of CPU per frame in total | would change `Frame`'s contract for nothing measurable |
| Avoid repainting the border | ≤ 1–2 µs per frame | below relevance |
| A frame-equality guard before the texture upload | comparing two 80 KB frames costs 27.6 µs, more than the ~10.5 µs it would save | the window now uploads only when a frame ran (`Pacer::ran`), which needs no comparison (item 6) |
| A `SharedArrayBuffer` audio ring in the browser | the copy it removes is ≤ 3.8 KiB per frame | needs COOP/COEP headers on every host serving the page |
| Reshape the `DD`/`FD` prefix dispatch | on the batched tree: null (±0.07 %), +8 lines, +1 nesting | RULE 4; required only if the batched ticks are ever removed (it recovered the prefix dispatch's +3.3 % wall before them) |
| HALT fast-forward | the halted frame already fell 3.08 M → 1.56 M instructions with the batched ticks | skipping to the interrupt would change `R`, contention and tape timing |
| `opt-level = "s"` / `"z"` for the browser | 179.8 / 472.6 µs per frame against 159.2 for `3`, all three in one session (item 11, the branch base read 274.5 there) — the 164.5 above is the final session's figure for the same `3` build; compare within a session only | slower |
| `strip = true` for the browser | −13,756 B gzipped (−6.1 %) of 225,502, measured on the tree before the telemetry merge | loses the `name` section and with it readable stack traces |
| Pre-size `zx-shot --wav`'s buffer | max RSS ≈ 185.5 MB and peak footprint ≈ 178.6 MB either way over the whole R-Type load | measured null; `docs/JOURNAL.md`, *Growth amortisation* |
| Native PGO (deferred to the next batch as N4, not refused) | trained on real loads: **−21.9 % instructions** on games it never saw, outputs identical. Trained on the benches: **+5.5 to +6.5 % worse** on real games. On wasm: +11 % slower | adoption needs a training set, pinned tools, a refresh rule and its own gate — `docs/STATUS.md`, N4 |

The browser build's size, as shipped by the finished tree: **706,448 B, 226,095 B gzipped**
(`cargo build --release -p frontend --target wasm32-unknown-unknown --bin zx`, `gzip -9`), up from
686,003 B at the branch base (+3.0 %): 705,339 B (gz 225,502) after the batched ticks, and
+1,109 B from the merged telemetry module.
