#!/bin/sh
# M8's gate: T1 + T2 + T3, plus the PERF hot-path ceiling the 2026-09-03 audit asked for.
#
#     sh web/gate.sh
#
# ## Read this before writing "M8: green" anywhere
#
# **Not one assertion in this script observes a pixel, a keypress or a frame in a browser.**
# `docs/M8.md` Decision 7 says so in those words and explains why the distinction is sharper
# here than at M6 or M7: those milestones' top tier was unautomatable because a corpus could not
# be committed, which is a licensing accident another repository could fix. M8's is
# unautomatable because *playable* is not a property of an artefact — it is a property of a
# browser rendering a canvas, a GPU compositing it, a keyboard delivering keys, and a person
# forming an opinion.
#
# So this is a **build gate**. It grades six modules that were already graded, a compiler, a
# linker, and two seams M8 added. A green here means the browser build compiles, links, emits a
# module with the imports and exports the page expects, and that the argument path and the
# download's failure path behave. It means nothing whatever about whether the page runs.
# T4 — a person, a browser, a version, an operating system and a date — is recorded in
# `web/README.md`.
#
# ## Nothing runs this automatically
#
# Stated rather than left to be discovered. There is no CI in this repository that could:
# `docs/STATUS.md`'s open register still carries `.github/workflows/ci.yml` as *"verified
# locally and enforced nowhere"*, because the session that wrote it could not push `.github/`.
# Three of this gate's steps are ordinary `cargo test` targets and do run under
# `cargo test --workspace`; the two that need a `wasm32` build do not, and cannot be, without a
# nested `cargo` inside a test. The PERF ceiling is a third kind: a bench with a wall-clock
# threshold, which no default test suite should carry — a timing assertion inside `cargo test`
# is a flaky suite by design, so it lives here, where the pre-push instrument is.

set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
cd "$root" || exit 1

# The smallest artefact that could plausibly be this emulator.
#
# A floor and not an equality, and published with the command that produced it rather than as a
# figure to be trusted — `docs/MACHINE.md` has three stale integers on record and the fix was
# the same every time. It only has to separate *this program* from *something else*: a Z80, a
# ULA, a screen renderer and a macroquad shell are not small, and a module of a few kilobytes is
# a build of something that is not this.
#
#     ls -l target/wasm32-unknown-unknown/release/zx.wasm
#
# Measured 624544 bytes on 2026-09-01, before `crates/page` existed. The floor is set at 256 KiB
# so that ordinary growth and ordinary shrinkage — `opt-level = "z"` has never been tried, and
# `docs/STATUS.md` carries the open row saying what it would cost — do not have to move it.
FLOOR=262144

failures=0
unanswered=0
unanswered_list=""
step() { printf '\n==> %s\n' "$*"; }
ok()   { printf '    ok   %s\n' "$*"; }
bad()  { printf '    FAIL %s\n' "$*"; failures=$((failures + 1)); }

# A question this run could not put, as distinct from one it put and got a yes to.
#
# *There was no such distinction, and the `else` branch that needed it said so in its own
# comment* — *"Not a pass. An unavailable instrument is an unanswered question, not a green
# one"* — and then printed a `SKIP` line without touching any counter, so the verdict block
# below printed **green** for a check it had declined to run. A script written to hunt exactly
# that shape, embodying it. The count is not a failure and does not change the exit status: a
# machine without the crate registry has not broken anything. What it does is take the
# unqualified word *"green"* away from the verdict, which is the sentence somebody copies.
skip() {
    printf '    SKIP %s\n' "$*"
    unanswered=$((unanswered + 1))
    unanswered_list="$unanswered_list
      - $*"
}

# Where this run's logs go.
#
# *These were fixed `/tmp/zx-m8-*.txt` paths.* Two concurrent runs clobbered each other's logs,
# and on a shared host a pre-created symlink at one of those names is a write primitive pointed
# at whatever the runner can write. A per-run directory costs one line. It is deliberately **not**
# removed on exit: a failing step prints its own log, and leaving the directory named below means
# a passing-but-odd run can still be looked at.
logs=$(mktemp -d "${TMPDIR:-/tmp}/zx-m8-gate.XXXXXXXX") || exit 1

# Every exit status is captured on the line that produced it, never after a pipe.
# `docs/STATUS.md`: "cmd | tail; echo $? reports tail's exit status, not cmd's, so a gate
# written that way reads green whatever cmd did."

# ---------------------------------------------------------------------------------------
# T1 + T3 — the headless gates, including the two seams M8 adds
# ---------------------------------------------------------------------------------------
# `--no-fail-fast` is not optional. Without it `cargo test` stops after the first failing
# target, so a mutation that reddens one unit test prevents every integration gate from running
# — and "the gates did not run" is indistinguishable from "the gates passed".
step "T1+T3  cargo test --workspace --no-fail-fast"
cargo test --workspace --no-fail-fast > "$logs/tests.txt" 2>&1
status=$?
tail -n 3 "$logs/tests.txt"
if [ "$status" -eq 0 ]; then ok "workspace suite"; else bad "workspace suite (exit $status); log: $logs/tests.txt"; fi

# *This printed the count of binaries reporting ok and compared it to nothing*, which by this
# repository's own standard is decoration rather than an assertion. It is compared to the number
# of binaries `cargo` said it was **running**, which needs no magic number and no floor to go
# stale: every target that starts must produce a result line, so a mismatch is a binary that
# died without reporting — a crash, a `SIGKILL`, a harness that never reached its summary.
# `--no-fail-fast` is what makes the two numbers comparable at all.
ran=$(grep -cE '^ +(Running|Doc-tests) ' "$logs/tests.txt")
reported=$(grep -c '^test result: ok' "$logs/tests.txt")
if [ "$ran" -eq 0 ]; then
    bad "no test binaries ran at all — this is not a pass, it is an empty run"
elif [ "$reported" -eq "$ran" ]; then
    ok "$reported of $ran test binaries reported ok"
else
    bad "$ran test binaries ran and $reported reported ok"
fi

step "T1     cargo fmt --check"
cargo fmt --all -- --check
status=$?
if [ "$status" -eq 0 ]; then ok "formatting"; else bad "formatting (exit $status)"; fi

# **Nothing in this project ran rustdoc, so a broken intra-doc link was invisible while clippy
# was green.** They are different instruments: clippy grades code, and an `[`Item`]` in a doc
# comment that names nothing reachable is not code. The failure mode is a published page whose
# cross-references are dead, in a repository whose documents are half its argument.
step "T1     cargo doc — intra-doc links resolve"
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --no-deps \
    > "$logs/doc.txt" 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "every intra-doc link resolves"
else bad "broken intra-doc links (exit $status)"; cat "$logs/doc.txt"; fi

# **Not `--all-features`, and this is a real interaction rather than an oversight.**
# `--all-features` turns on `bundled`, and `crates/frontend/build.rs` then *correctly* fails the
# build because no payload was configured — which is the loud failure that feature exists to
# have. A feature whose payload comes from the environment is fundamentally incompatible with
# `--all-features`, because Cargo cannot distinguish "the user asked for this feature" from "the
# user asked for every feature". The trade is deliberate: a standalone that starts and shows
# nothing is worse than a flag combination that does not apply here. Named in
# `crates/frontend/Cargo.toml` too, so somebody who types it gets an explanation rather than a
# mystery.
step "T1     cargo clippy (host)"
cargo clippy --all-targets -- -D warnings > "$logs/clippy.txt" 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "clippy, host"; else bad "clippy, host (exit $status)"; cat "$logs/clippy.txt"; fi

# The host run above lints **none** of `crates/page`'s `unsafe` blocks: they are behind
# `#[cfg(target_arch = "wasm32")]`, and a lint only fires on code the current target compiles.
# So `clippy::undocumented_unsafe_blocks` and `clippy::multiple_unsafe_ops_per_block` — both
# deny in that crate's manifest — are silent on a desktop. This line is where they run. It
# caught an `unused_doc_comment` on the `extern` block on its first invocation.
step "T2     cargo clippy --target wasm32-unknown-unknown"
cargo clippy --target wasm32-unknown-unknown -p frontend -p page --lib --bins -- -D warnings \
    > "$logs/clippy-wasm.txt" 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "clippy, wasm32 — the only run that lints the unsafe blocks"
else bad "clippy, wasm32 (exit $status)"; cat "$logs/clippy-wasm.txt"; fi

# ---------------------------------------------------------------------------------------
# PERF — the hot-path ceiling. Prose does not go red; this number can.
# ---------------------------------------------------------------------------------------
# `benches/frame.rs` and the coverage tables defend the hot path with recorded medians, and
# the 2026-09-03 audit named the gap in that: every one of those figures is prose, so the
# next regression lands on whatever baseline the last one left, exactly as the +23 % one did.
# This step is the number with a gate on it: `quiet_48k` — the defended invariant, one frame
# of `NOP`s with nobody touching sound — must stay under a recorded ceiling.
#
# The derivation, in full, so that a red is diagnosable and a re-baseline is honest:
#
#   BASELINE_US    138.7 µs — the per-run `fastest` floor recorded 2026-09-03 on this machine
#                  (hw.model Mac15,9, rustc 1.98.0), bit-stable across seven undisturbed runs
#                  in two independent sessions, under a one-minute load of 8.6–11.4 on 16
#                  cores. The lowest *median* the same sessions recorded is 138.8 µs, which is
#                  the figure `benches/frame.rs`'s doc header carries.
#   MARGIN         ×1.15 over the floor. The audit's variance report: a load spike displaces
#                  a whole run by up to ~10 %, and *"do not set the gate tighter than 8 % on
#                  this hardware"*. The obvious estimator — lowest median of three, ×1.10 —
#                  was measured here and disqualified: under a sustained load of 11, three
#                  consecutive runs' medians all sat at 152.6–152.7 µs, exactly ×1.10 of
#                  baseline, while the same runs' `fastest` floors stayed at 131.9–142.2. The
#                  floor is the estimator that survives load, and the ceiling it is held to
#                  still catches a P1-class regression — +19.6 % on this case, reproduced on
#                  the floor — with margin to spare.
#   THE ESTIMATE   the lowest `fastest` across three runs. To breach the ceiling, the best of
#                  100 samples must exceed it three runs in a row: noise cannot arrange that,
#                  a regression cannot avoid it.
#
# A red here means the hot path is slower than every recorded state of this tree, or the
# baseline is stale. Re-baseline only from a measurement — re-run, record the new floor with
# its date and conditions here — never by widening MARGIN until a finding goes away. What
# this step cannot see: a regression under ~15 % (an M4-sized change is ±3 %; that resolution
# needs a quiet machine), and anything on hardware that is not the baseline's — there it
# skips, counted, because this baseline compared against another machine's clock would be a
# gate grading geography.
BASELINE_US=138.7
MARGIN=1.15
BASELINE_MODEL="Mac15,9"
step "PERF   quiet_48k stays under its recorded ceiling"
model=$(sysctl -n hw.model 2>/dev/null || true)
if [ "$model" != "$BASELINE_MODEL" ]; then
    skip "hot-path ceiling: this is '${model:-an unknown machine}' and the ${BASELINE_US} µs baseline was recorded on ${BASELINE_MODEL}; re-baseline before gating here"
else
    lowest=""
    for run in 1 2 3; do
        cargo bench -p spectrum --bench frame quiet_48k > "$logs/bench$run.txt" 2>&1
        status=$?
        if [ "$status" -ne 0 ]; then
            bad "bench run $run failed (exit $status); log: $logs/bench$run.txt"
            continue
        fi
        # The row reads "╰─ quiet_48k  138.7 µs │ <slowest> │ <median> │ <mean> │ …", so the
        # second whitespace field is the name and the next two are the `fastest` value and
        # its unit. divan scales units per value, so a regressed run printing "1.02 ms" must
        # not be read as 1.02: the unit is normalised, and a row this cannot read is a FAIL
        # rather than a silent green — this script exists to hunt vacuous gates, not to
        # ship one.
        micros=$(awk '$2 == "quiet_48k" {
            value = $3; unit = $4
            if      (unit == "ns") value /= 1000
            else if (unit == "µs") value += 0
            else if (unit == "ms") value *= 1000
            else if (unit == "s")  value *= 1000000
            else next
            printf "%.1f", value
            exit
        }' "$logs/bench$run.txt")
        if [ -z "$micros" ]; then
            bad "bench run $run has no readable quiet_48k row; log: $logs/bench$run.txt"
            continue
        fi
        ok "run $run: fastest $micros µs"
        if [ -z "$lowest" ] || awk "BEGIN { exit !($micros < $lowest) }"; then lowest=$micros; fi
    done
    if [ -n "$lowest" ]; then
        ceiling=$(awk "BEGIN { printf \"%.1f\", $BASELINE_US * $MARGIN }")
        if awk "BEGIN { exit !($lowest <= $ceiling) }"; then
            ok "lowest of three: $lowest µs, under the $ceiling µs ceiling ($BASELINE_US µs × $MARGIN)"
        else
            bad "quiet_48k floor $lowest µs breached the $ceiling µs ceiling ($BASELINE_US µs × $MARGIN, recorded 2026-09-03) — a hot-path regression, or a stale baseline; the derivation above says how to tell them apart"
        fi
    fi
fi

# ---------------------------------------------------------------------------------------
# T2 — the build, the artefact, and what is inside it
# ---------------------------------------------------------------------------------------
step "T2     cargo build --release --target wasm32-unknown-unknown"
cargo build --release --manifest-path crates/frontend/Cargo.toml \
    --target wasm32-unknown-unknown --bin zx > "$logs/build.txt" 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "the crate links for wasm32"
else bad "wasm32 build (exit $status)"; cat "$logs/build.txt"; fi

# Assertion 2 of Decision 7's three: a build that produced nothing must not pass. `--emit=asm`
# without `link` is on this project's record as passing a gate vacuously in another language.
wasm=target/wasm32-unknown-unknown/release/zx.wasm
step "T2     the artefact exists and is not a stub"
if [ -f "$wasm" ]; then
    ok "$wasm exists"
    bytes=$(stat -f %z "$wasm" 2>/dev/null || stat -c %s "$wasm")
    if [ "$bytes" -ge "$FLOOR" ]; then ok "$bytes bytes (floor $FLOOR)"
    else bad "$bytes bytes is under the $FLOOR floor — this is a build of something else"; fi
else
    bad "$wasm does not exist"
fi

# Assertion 3, and it is much stronger than a size. A size floor cannot tell this emulator from
# any other half-megabyte of WebAssembly; the import and export names can. Every one of these is
# a contract with a specific file: the `zx_*` imports listed below with `web/zx_page.js`, and the
# miniquad exports with `web/mq_js_bundle.js`.
step "T2     the module's imports and exports are the ones the page provides and calls"
if [ -f "$wasm" ]; then
    node -e '
const fs = require("fs");
const module_ = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const imports = WebAssembly.Module.imports(module_);
const exports_ = new Set(WebAssembly.Module.exports(module_).map((e) => e.name));

let bad = 0;
const fail = (message) => { console.log("    FAIL " + message); bad += 1; };
const pass = (message) => console.log("    ok   " + message);

// `web/zx_page.js` registers these into `importObject.env`. A rename on either side that the
// other does not follow is the failure `add_missing_functions_stabs` hides: it substitutes a
// stub, the page runs, and F2 reports success over a save that did not happen.
for (const name of ["zx_query_length", "zx_query_copy", "zx_offer_download",
                    "zx_audio_rate", "zx_audio_push"]) {
    const found = imports.find((i) => i.name === name);
    if (!found) fail(`import ${name} is missing — crates/page is not linked in`);
    else if (found.module !== "env") fail(`import ${name} is in module ${found.module}, not env`);
    else pass(`env.${name}`);
}

// miniquad calls these by name from `gl.js`. `on_file_dropped` is drag-and-drop; its absence
// would mean Decision 5 shipped a feature the bundle cannot reach.
for (const name of ["main", "crate_version", "allocate_vec_u8",
                    "on_files_dropped_start", "on_file_dropped", "on_files_dropped_finish",
                    "zx_page_crate_version"]) {
    if (exports_.has(name)) pass(`export ${name}`);
    else fail(`export ${name} is missing`);
}

console.log(`    ${imports.length} imports, ${exports_.size} exports`);
process.exit(bad === 0 ? 0 : 1);
' "$wasm"
    status=$?
    if [ "$status" -eq 0 ]; then ok "module surface"; else bad "module surface (exit $status)"; fi
else
    bad "module surface — no artefact to read"
fi

# ---------------------------------------------------------------------------------------
# T2 — the vendored bundle is the pinned crate's
# ---------------------------------------------------------------------------------------
# `docs/M8.md` Decision 6: the hash is the gate and miniquad's own runtime version check is not,
# because that check is a `console.error` that does not stop the page. A bundle from a different
# miniquad produces an emulator that starts, draws, and misbehaves somewhere, with the
# explanation in a console nobody has open.
# Two questions per asset, and *only one of them was being asked of three of the four*: does the
# source exist in `web/`, and does `build.sh` copy it into the served directory.
#
# **The unprotected omission was the strictly worse one.** Only `zx_audio_worklet.js` was checked
# against `build.sh`, and `web/build.sh` copies all four in a single `cp` — so deleting
# `zx_page.js` from that line left this gate green, and the page it produces is precisely the
# scenario `crates/page`, this script and `docs/M8.md` Decision 4 all exist for:
# `add_missing_functions_stabs` substitutes stubs, the page boots and takes the keyboard, and
# `F2` reports `saved snapshot-1.z80` over a save that did not happen. The protected omission was
# *silence*; the unprotected one was **silent data loss**.
#
# Both are the same shape — an asset fetched or linked by NAME at run time, so an omission fails
# in a browser rather than at build time — which is why the check is now one loop over all four
# rather than one loop plus a special case.
step "T2     the page's own scripts are all present, and build.sh copies every one"
for asset in index.html zx_page.js zx_audio_worklet.js mq_js_bundle.js; do
    if [ -f "$here/$asset" ]; then ok "$asset"; else bad "$asset is missing"; fi
    if grep -qF "$asset" "$here/build.sh"; then ok "build.sh copies $asset"
    else bad "build.sh does not copy $asset; the served page would be missing it"; fi
done

# **`PLUGIN_VERSION` and `STARTED` are one fact each, written in two files, and nothing compared
# them.** `web/README.md` says they *"must match"*; the import assertion above checks that the
# names exist and the export assertion checks that the export exists, and neither reads a value.
# The only value check anywhere was miniquad's, which is a `console.error` that does not stop the
# page — and `STARTED` has no runtime check at all. It is the single number the whole
# download-failure design rests on: if the two files disagree, `handoff` reads a successful
# download as a refusal, or worse, a stub's `0` as a success.
step "T2     PLUGIN_VERSION and STARTED are the same number on both sides of the seam"
node -e '
const fs = require("fs");
const rust = fs.readFileSync(process.argv[1], "utf8");
const js = fs.readFileSync(process.argv[2], "utf8");

let bad = 0;
const fail = (message) => { console.log("    FAIL " + message); bad += 1; };
const pass = (message) => console.log("    ok   " + message);

// Exactly one match, and the count is asserted rather than assumed. A pattern that found
// nothing would otherwise compare undefined to undefined and pass — a gate reporting agreement
// between two numbers it never read, which is the vacuous shape this whole script exists to
// hunt.
const only = (text, pattern, where, what) => {
    const found = [...text.matchAll(pattern)];
    if (found.length !== 1) {
        fail(`${where} declares ${what} ${found.length} times, expected exactly 1`);
        return null;
    }
    return Number(found[0][1]);
};

for (const [name, inRust, inJs] of [
    ["PLUGIN_VERSION", /const PLUGIN_VERSION: u32 = (\d+);/g, /const PLUGIN_VERSION = (\d+);/g],
    ["STARTED",        /const STARTED: i32 = (\d+);/g,        /const STARTED = (\d+);/g],
]) {
    const a = only(rust, inRust, "crates/page/src/lib.rs", name);
    const b = only(js, inJs, "web/zx_page.js", name);
    if (a === null || b === null) continue;
    if (a === b) pass(`${name} = ${a} in both files`);
    else fail(`${name} is ${a} in crates/page/src/lib.rs and ${b} in web/zx_page.js`);
}
process.exit(bad === 0 ? 0 : 1);
' "$root/crates/page/src/lib.rs" "$here/zx_page.js"
status=$?
if [ "$status" -eq 0 ]; then ok "the two duplicated numbers agree"
else bad "the Rust and JavaScript sides disagree (exit $status)"; fi

step "T2     web/mq_js_bundle.js is byte-identical to the pinned crate's"
# Anchored on the table row rather than taking the *first* 64-hex run in the file, which is what
# this did: any earlier 64-hex string added to `README.md` — another artefact's digest, a commit
# hash, a quoted example — would silently become the figure this gate checks against, and the
# check would keep printing green while grading the wrong number.
recorded=$(grep 'SHA-256' "$here/README.md" | grep -o '[0-9a-f]\{64\}')
actual=$(shasum -a 256 "$here/mq_js_bundle.js" | cut -d' ' -f1)
if [ "$(printf '%s\n' "$recorded" | grep -c .)" -ne 1 ]; then
    bad "web/README.md does not carry exactly one SHA-256 digest to check against"
elif [ "$actual" = "$recorded" ]; then ok "SHA-256 matches web/README.md"
else bad "SHA-256 $actual does not match the recorded $recorded"; fi

pinned=$(ls -d "$HOME"/.cargo/registry/src/*/macroquad-0.4.16/js/mq_js_bundle.js 2>/dev/null | head -n 1)
if [ -n "$pinned" ] && [ -f "$pinned" ]; then
    cmp -s "$pinned" "$here/mq_js_bundle.js"
    status=$?
    if [ "$status" -eq 0 ]; then ok "identical to $pinned"
    else bad "differs from the pinned crate's copy at $pinned"; fi
else
    # Not a pass. An unavailable instrument is an unanswered question, not a green one — and
    # `skip` is what now makes the verdict say so, rather than this comment saying it while the
    # code below printed green anyway.
    skip "no macroquad-0.4.16 in the registry, so mq_js_bundle.js was not compared to it; \`cargo fetch\` first"
fi

# ---------------------------------------------------------------------------------------
step "verdict"
# The mktemp comment above says a passing-but-odd run can still be looked at; this line is
# what makes that true on a green run, where no failing step has printed a path.
printf '    logs %s\n' "$logs"
if [ "$failures" -eq 0 ] && [ "$unanswered" -ne 0 ]; then
    printf '    T1 + T2 + T3 + PERF pass, with %s question(s) this run could not put:%s\n\n' \
        "$unanswered" "$unanswered_list"
    printf '    That is not the same as green, and the difference is the point: an instrument\n'
    printf '    that was unavailable graded nothing. Re-run where it is available before\n'
    printf '    writing "M8: green" anywhere.\n\n'
fi
if [ "$failures" -eq 0 ]; then
    if [ "$unanswered" -eq 0 ]; then printf '    T1 + T2 + T3 + PERF green.\n\n'; fi
    cat <<'EOF'
    What that does NOT say, and it belongs beside any "M8: green":
      - that the page renders. The first WebGL context this code meets is in a browser.
      - that a keypress arrives, or which membrane key a non-US keyboard produces.
      - that a Ctrl chord is cancellable. No automation here can ask that question: a key
        injected through the DevTools Protocol never traverses the browser's shortcut layer,
        so the check returns green for a question it did not ask.
      - that a download reaches anybody's disk.
      - that it is playable, which is the milestone's stated goal.
    Those are T4. `web/README.md` records the runs.
EOF
    exit 0
else
    printf '    %s step(s) failed\n' "$failures"
    exit 1
fi
