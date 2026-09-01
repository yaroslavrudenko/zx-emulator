#!/bin/sh
# The `bundled` feature's gate: build a standalone with a payload this repository generates,
# and prove the embedded bytes reach the same machine a named file does.
#
#     sh crates/frontend/gate-bundled.sh
#
# ## Why this is a script and not only a `cargo test`
#
# The payload is embedded by `include_bytes!` at **compile** time, so a test cannot generate one
# and then be compiled against it — the generation has to happen first, in a separate process.
# What the script does is generate, build, and then run `cargo test --features bundled`, whose
# assertions are ordinary Rust in `crates/frontend/tests/byte_sources.rs`. The script is the
# fixture; the gate is still a test.
#
# ## No game is used, and none could be
#
# `testdata/README.md` records that this repository redistributes nothing except the Sinclair
# ROMs, on a permission quoted there with its author and date. A game is covered by no such
# permission. So the payload here is **generated**: a 16 KB ROM assembled below out of six Z80
# instructions, and a `.z80` snapshot the emulator writes of itself. The mechanism is what could
# be wrong, and the mechanism is what this grades.
#
# ## Nothing runs this automatically
#
# Stated rather than left to be found out. There is no CI in this repository that could —
# `docs/STATUS.md` still carries `.github/workflows/ci.yml` as "verified locally and enforced
# nowhere". The default `cargo test --workspace` builds without the feature and runs the
# unbundled arm of `byte_sources.rs`, which asserts that an ordinary build embeds **nothing**.

set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../.." && pwd)
cd "$root" || exit 1

work="${TMPDIR:-/tmp}/zx-bundled-gate-$$"
mkdir -p "$work" || exit 1
trap 'rm -rf "$work"' EXIT

failures=0
step() { printf '\n==> %s\n' "$*"; }
ok()   { printf '    ok   %s\n' "$*"; }
bad()  { printf '    FAIL %s\n' "$*"; failures=$((failures + 1)); }

# Every exit status is captured on the line that produced it and never after a pipe.
# `docs/STATUS.md`: "cmd | tail; echo $? reports tail's exit status, not cmd's."

# ---------------------------------------------------------------------------------------
step "generate a 16 KB ROM — six instructions, written here, redistributable by nobody but us"
# LD HL,0x4000 / LD DE,0x4001 / LD BC,0x1AFF / LD (HL),0x25 / LDIR / JR -2
#
# Fills the display file and the attributes with 0x25 and then loops forever. 0x25 as an
# attribute is flash off, bright off, paper green, ink cyan — chosen because *flash on* would
# make the screen a function of the frame number, and a gate whose subject changes with time is
# a gate that has to be told which moment it meant.
python3 - "$work/synthetic.rom" <<'PY'
import sys
program = bytes([
    0x21, 0x00, 0x40,   # LD HL,0x4000
    0x11, 0x01, 0x40,   # LD DE,0x4001
    0x01, 0xFF, 0x1A,   # LD BC,0x1AFF
    0x36, 0x25,         # LD (HL),0x25
    0xED, 0xB0,         # LDIR
    0x18, 0xFE,         # JR -2
])
rom = bytearray(16384)
rom[: len(program)] = program
sys.stdout.buffer.write(b"")
open(sys.argv[1], "wb").write(bytes(rom))
PY
status=$?
if [ "$status" -eq 0 ] && [ -s "$work/synthetic.rom" ]; then
    size=$(stat -f %z "$work/synthetic.rom" 2>/dev/null || stat -c %s "$work/synthetic.rom")
    if [ "$size" -eq 16384 ]; then ok "$size bytes"; else bad "$size bytes, wanted 16384"; fi
else
    bad "could not generate the ROM (exit $status)"
fi

# ---------------------------------------------------------------------------------------
step "generate a .z80 snapshot, written by the emulator itself"
# `zx-shot` is the headless binary; it already boots a machine and writes a file through
# `host::save`. Rather than add a snapshot mode to it, the snapshot is produced by the same
# `media::save` the F2 key uses, from a one-off test binary — which is what `cargo test` below
# does. So this step is a placeholder for the ordering: the snapshot is generated *inside*
# `tests/byte_sources.rs`, which has the ROM and the writer already.
ok "generated inside tests/byte_sources.rs, where the writer already is"

# ---------------------------------------------------------------------------------------
step "build.rs refuses the feature with no payload"
# The loud-failure requirement, asserted rather than trusted: a standalone that starts and shows
# nothing is the outcome this check exists to prevent, so the check itself needs a failing case.
env -u ZX_BUNDLE_ROM -u ZX_BUNDLE_MEDIA \
    cargo build --manifest-path crates/frontend/Cargo.toml --features bundled --lib \
    > "$work/no-payload.txt" 2>&1
status=$?
if [ "$status" -ne 0 ] && grep -q "nothing to embed" "$work/no-payload.txt"; then
    ok "refused, and the message says what to set"
else
    bad "the feature built with no payload (exit $status) — a silent standalone is possible"
    tail -n 20 "$work/no-payload.txt"
fi

step "build.rs refuses a format the emulator cannot load"
# **This step named `.tzx`, and it is why this gate exited 1.** It embedded a `.tzx` and required
# the build to fail with `cannot load a .tzx`. `.tzx` became loadable on 2026-09-01, so the build
# it expected to refuse succeeded — and the step then reported "a .tzx was embedded — it would
# fail at run time on somebody's machine", which was the gate announcing a defect on the day the
# emulator gained a feature.
#
# The invariant was never about `.tzx` and is worth keeping: a format outside `media::EXTENSIONS`
# must be refused **here**, where a person can act on it, rather than at a stranger's
# double-click. So it is repointed at `.dsk` — a +3 disk image, a real format this project knows
# the name of and has written no parser for — and the next format to land moves it again.
printf 'MV - CPCEMU Disk-File\r\n' > "$work/game.dsk"
ZX_BUNDLE_MEDIA="$work/game.dsk" \
    cargo build --manifest-path crates/frontend/Cargo.toml --features bundled --lib \
    > "$work/dsk.txt" 2>&1
status=$?
if [ "$status" -ne 0 ] && grep -q "cannot load a .dsk" "$work/dsk.txt"; then
    ok "a .dsk is refused at build time, by name"
else
    bad "a .dsk was embedded (exit $status) — it would fail at run time on somebody's machine"
    tail -n 20 "$work/dsk.txt"
fi

step "build.rs refuses a payload that is not there"
ZX_BUNDLE_ROM="$work/absent.rom" \
    cargo build --manifest-path crates/frontend/Cargo.toml --features bundled --lib \
    > "$work/absent.txt" 2>&1
status=$?
if [ "$status" -ne 0 ] && grep -q "cannot be read" "$work/absent.txt"; then
    ok "a missing payload is a build failure, naming the path it looked at"
else
    bad "a missing payload built (exit $status)"
    tail -n 20 "$work/absent.txt"
fi

# ---------------------------------------------------------------------------------------
step "the bundled arm of tests/byte_sources.rs"
ZX_BUNDLE_ROM="$work/synthetic.rom" \
    cargo test --manifest-path crates/frontend/Cargo.toml --features bundled --no-fail-fast \
    > "$work/tests.txt" 2>&1
status=$?
grep -E '^test result' "$work/tests.txt" | sed 's/^/    /'
if [ "$status" -eq 0 ]; then ok "suite green with the feature on"
else bad "suite failed with the feature on (exit $status)"; tail -n 40 "$work/tests.txt"; fi

# ---------------------------------------------------------------------------------------
step "a standalone binary links, and photographs the machine it embedded"
ZX_BUNDLE_ROM="$work/synthetic.rom" \
    cargo build --release --manifest-path crates/frontend/Cargo.toml \
    --features bundled --bin zx --bin zx-shot > "$work/build.txt" 2>&1
status=$?
if [ "$status" -eq 0 ]; then ok "zx and zx-shot link with a payload compiled in"
else bad "the standalone did not build (exit $status)"; tail -n 30 "$work/build.txt"; fi

# The end-to-end claim: with **no arguments naming a ROM**, a bundled `zx-shot` photographs the
# machine its embedded ROM builds. The picture is compared against the same ROM passed by name
# to an ordinary build, which is the four-way equivalence's outermost ring — two separate
# processes, two separate binaries, one expected image.
shot="target/release/zx-shot"
if [ -x "$shot" ]; then
    "$shot" --out "$work/embedded.ppm" --frames 40 > "$work/embedded.txt" 2>&1
    status=$?
    if [ "$status" -eq 0 ]; then ok "the standalone ran with no arguments at all"
    else bad "the standalone would not run (exit $status)"; cat "$work/embedded.txt"; fi
else
    bad "no zx-shot to run"
fi

cargo build --release --manifest-path crates/frontend/Cargo.toml --bin zx-shot \
    > "$work/plain.txt" 2>&1
status=$?
if [ "$status" -ne 0 ]; then bad "the ordinary build failed (exit $status)"; fi
if [ -x "$shot" ]; then
    "$shot" --rom "$work/synthetic.rom" --out "$work/named.ppm" --frames 40 \
        > "$work/named.txt" 2>&1
    status=$?
    if [ "$status" -ne 0 ]; then bad "the ordinary build would not run (exit $status)"; cat "$work/named.txt"; fi
fi

step "an embedded ROM and a named ROM produce the same picture"
if [ -f "$work/embedded.ppm" ] && [ -f "$work/named.ppm" ]; then
    cmp -s "$work/embedded.ppm" "$work/named.ppm"
    status=$?
    if [ "$status" -eq 0 ]; then ok "byte-identical"
    else bad "the two pictures differ — embedding changes what the machine does"; fi

    # The assertion whose failure means "I was not looking at the thing". Both pictures being
    # equal is worth nothing if every picture is equal: a `zx-shot` that wrote a blank frame
    # regardless would pass the line above. So a *different* ROM must produce a *different*
    # picture.
    python3 - "$work/other.rom" <<'PY'
import sys
program = bytes([0x21, 0x00, 0x40, 0x11, 0x01, 0x40, 0x01, 0xFF, 0x1A,
                 0x36, 0x0A,          # LD (HL),0x0A — a different fill
                 0xED, 0xB0, 0x18, 0xFE])
rom = bytearray(16384)
rom[: len(program)] = program
open(sys.argv[1], "wb").write(bytes(rom))
PY
    "$shot" --rom "$work/other.rom" --out "$work/other.ppm" --frames 40 >/dev/null 2>&1
    cmp -s "$work/embedded.ppm" "$work/other.ppm"
    status=$?
    if [ "$status" -ne 0 ]; then ok "a different ROM gives a different picture, so the comparison bites"
    else bad "two different ROMs photograph identically — this gate cannot fail"; fi
else
    bad "one of the two pictures is missing"
fi

# ---------------------------------------------------------------------------------------
step "verdict"
if [ "$failures" -eq 0 ]; then
    cat <<'EOF'
    The mechanism is green.

    What that does NOT say:
      - that any particular game runs. Four have — Cybernoid, Manic Miner, Cybernoid II and
        Exolon, photographed in docs/images/ with the command that re-takes each — and not one
        of them ran from here. This gate deliberately embeds a ROM this repository wrote so
        that it needs no corpus, and testdata/games/ is gitignored, so a clean checkout has
        nothing else it could embed.
      - that a game is playable, or that its keys respond. Manic Miner's key reads were
        measured (keymap::ArrowTarget::Both, byte-identical state after an identical hold);
        whether it is playable is a person at a keyboard. `tests/keymap_under_a_game.rs` grades
        what a game would read, and nothing this script builds has ever been read by one.
      - that the in-game music plays. The beeper landed at M7 — bit 4 of a 0xFE write drives
        it in crates/spectrum/src/ula.rs, and crates/spectrum/tests/m7_beeper.rs grades it —
        but nothing here mixes a sample or opens a device, and this environment has no audio
        hardware to open. The tune remains observation by somebody with speakers.
EOF
    exit 0
else
    printf '    %s step(s) failed\n' "$failures"
    exit 1
fi
