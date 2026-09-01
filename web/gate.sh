#!/bin/sh
# M8's gate: T1 + T2 + T3.
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
# Three of this gate's five steps are ordinary `cargo test` targets and do run under
# `cargo test --workspace`; the two that need a `wasm32` build do not, and cannot be, without a
# nested `cargo` inside a test.

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
step() { printf '\n==> %s\n' "$*"; }
ok()   { printf '    ok   %s\n' "$*"; }
bad()  { printf '    FAIL %s\n' "$*"; failures=$((failures + 1)); }

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
cargo test --workspace --no-fail-fast > /tmp/zx-m8-gate-tests.txt 2>&1
status=$?
tail -n 3 /tmp/zx-m8-gate-tests.txt
if [ "$status" -eq 0 ]; then ok "workspace suite"; else bad "workspace suite (exit $status)"; fi
grep -c '^test result: ok' /tmp/zx-m8-gate-tests.txt \
    | sed 's/^/    test binaries reporting ok: /'

step "T1     cargo fmt --check"
cargo fmt --all -- --check
status=$?
if [ "$status" -eq 0 ]; then ok "formatting"; else bad "formatting (exit $status)"; fi

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
cargo clippy --all-targets -- -D warnings > /tmp/zx-m8-gate-clippy.txt 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "clippy, host"; else bad "clippy, host (exit $status)"; cat /tmp/zx-m8-gate-clippy.txt; fi

# The host run above lints **none** of `crates/page`'s `unsafe` blocks: they are behind
# `#[cfg(target_arch = "wasm32")]`, and a lint only fires on code the current target compiles.
# So `clippy::undocumented_unsafe_blocks` and `clippy::multiple_unsafe_ops_per_block` — both
# deny in that crate's manifest — are silent on a desktop. This line is where they run. It
# caught an `unused_doc_comment` on the `extern` block on its first invocation.
step "T2     cargo clippy --target wasm32-unknown-unknown"
cargo clippy --target wasm32-unknown-unknown -p frontend -p page --lib --bins -- -D warnings \
    > /tmp/zx-m8-gate-clippy-wasm.txt 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "clippy, wasm32 — the only run that lints the unsafe blocks"
else bad "clippy, wasm32 (exit $status)"; cat /tmp/zx-m8-gate-clippy-wasm.txt; fi

# ---------------------------------------------------------------------------------------
# T2 — the build, the artefact, and what is inside it
# ---------------------------------------------------------------------------------------
step "T2     cargo build --release --target wasm32-unknown-unknown"
cargo build --release --manifest-path crates/frontend/Cargo.toml \
    --target wasm32-unknown-unknown --bin zx > /tmp/zx-m8-gate-build.txt 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "the crate links for wasm32"
else bad "wasm32 build (exit $status)"; cat /tmp/zx-m8-gate-build.txt; fi

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
# a contract with a specific file: the three `zx_*` imports with `web/zx_page.js`, and the
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
step "T2     the page's own scripts are all present"
for asset in index.html zx_page.js zx_audio_worklet.js mq_js_bundle.js; do
    if [ -f "$here/$asset" ]; then ok "$asset"; else bad "$asset is missing"; fi
done
# `zx_audio_worklet.js` is fetched by NAME at run time, by `audioWorklet.addModule`, so a build
# that forgot to copy it fails in the browser with a console error and silence — not at build
# time. That is exactly the shape this gate exists to convert into a build-time failure.
if grep -q "zx_audio_worklet.js" "$here/build.sh"; then ok "build.sh copies the worklet"
else bad "build.sh does not copy zx_audio_worklet.js; the page would be silent"; fi

step "T2     web/mq_js_bundle.js is byte-identical to the pinned crate's"
recorded=$(grep -o '[0-9a-f]\{64\}' "$here/README.md" | head -n 1)
actual=$(shasum -a 256 "$here/mq_js_bundle.js" | cut -d' ' -f1)
if [ "$actual" = "$recorded" ]; then ok "SHA-256 matches web/README.md"
else bad "SHA-256 $actual does not match the recorded $recorded"; fi

pinned=$(ls -d "$HOME"/.cargo/registry/src/*/macroquad-0.4.16/js/mq_js_bundle.js 2>/dev/null | head -n 1)
if [ -n "$pinned" ] && [ -f "$pinned" ]; then
    cmp -s "$pinned" "$here/mq_js_bundle.js"
    status=$?
    if [ "$status" -eq 0 ]; then ok "identical to $pinned"
    else bad "differs from the pinned crate's copy at $pinned"; fi
else
    # Not a pass. An unavailable instrument is an unanswered question, not a green one.
    printf '    SKIP no macroquad-0.4.16 in the registry; `cargo fetch` first\n'
fi

# ---------------------------------------------------------------------------------------
step "verdict"
if [ "$failures" -eq 0 ]; then
    cat <<'EOF'
    T1 + T2 + T3 green.

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
