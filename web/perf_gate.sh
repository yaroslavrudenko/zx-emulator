#!/bin/sh
# The performance gate: instructions retired per emulated frame, held to 1 % of a recorded
# baseline.
#
#     sh web/perf_gate.sh            # check against web/perf-baseline.txt (exit 1 on a regression)
#     sh web/perf_gate.sh --measure  # print this tree's figures, compare nothing
#     sh web/perf_gate.sh --record   # rewrite web/perf-baseline.txt from this tree
#
# `web/gate.sh` calls the check. `docs/PERFORMANCE.md` carries the derivation, the mutation
# proofs that turned it red, and the history of the numbers; this header carries only what a
# person reading a red needs.
#
# ## Why instructions and not wall time
#
# The judges' decision of 2026-10-20 (DECISION.md item 2). One binary's wall-clock `fastest`
# moved 7–12 % between back-to-back runs at a load of 7 on this machine (P-core against E-core
# placement), so a wall threshold tight enough to catch a 2 % regression goes red on unchanged
# code. The instruction count of the same binary moved at most 0.38 % under a load of 186, and
# 0.02–0.24 % under ordinary load. 1 % is more than 2.5× the worst spread anyone recorded and
# catches the 1–2 % class (the contended-slot cache was −3.2 % wall, about −1 % instructions)
# that 2 % would let through.
#
# Wall time is still required, as evidence and never as a verdict: `web/perf_ab.sh` runs the
# interleaved merge-base A/B every performance-touching change pastes into its record.
#
# ## How a figure is taken
#
# One process runs N frames of a case. Its instruction total is setup plus N frames, so the
# slope between N=1500 and N=500, divided by 1000, is one frame with the setup cancelled. Each
# case takes three slopes and keeps the median.
#
# ## What makes it skip or fail rather than pass
#
# An instruction count belongs to a binary, so it belongs to an architecture and a compiler.
# The baseline records both. On another architecture the run is a counted skip: this host cannot
# answer the question. On another compiler of the same architecture, or with no `# toolchain`
# header, it fails: a compiler bump is a commit that bumps `rust-toolchain.toml` and re-records
# from the merge base together, with the date in the commit that carries it.

set -u

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/.." && pwd)
cd "$root" || exit 1

THRESHOLD_PERCENT=1
BASELINE="$here/perf-baseline.txt"
FRAME_CASES="quiet_48k quiet_128 contended_48k drained_48k beeper_48k tape_reads_48k tape_playing_48k border_48k ay_128 halt_48k"
STEP_CASES="flat_bus"

mode=${1:-check}
case "$mode" in
    check|--check) mode=check ;;
    --measure) mode=measure ;;
    --record) mode=record ;;
    *) printf 'usage: sh web/perf_gate.sh [--measure|--record]\n' >&2; exit 2 ;;
esac

# `instructions retired` from the kernel's accounting of one process. macOS prints it under
# `/usr/bin/time -l`; Linux needs `perf`. Anything else has no counter, and says so.
counter=""
if /usr/bin/time -l true 2>&1 | grep -q 'instructions retired'; then
    counter=macos
elif command -v perf > /dev/null 2>&1 && perf stat -x, -e instructions:u true > /dev/null 2>&1; then
    counter=linux
fi

# The counter's report goes to a file, never down a pipe, so the bench's own exit status is the
# one this function returns: a bench that crashed part-way still prints counters, and a crash
# after 600 of 1500 frames would read as a 60 % improvement.
report=$(mktemp "${TMPDIR:-/tmp}/zx-perf-gate.XXXXXXXX") || exit 1
trap 'rm -f "$report"' EXIT
instructions() { # binary sample-count case
    if [ "$counter" = macos ]; then
        /usr/bin/time -l "$1" --bench --exact --sample-count "$2" --sample-size 1 "$3" 2> "$report" > /dev/null || return 1
        awk '/instructions retired/ { print $1 }' "$report"
    else
        perf stat -x, -e instructions:u "$1" --bench --exact --sample-count "$2" --sample-size 1 "$3" 2> "$report" > /dev/null || return 1
        awk -F, '/instructions/ { print $1 }' "$report"
    fi
}

# One frame: the median of three (N=1500 − N=500) / 1000 slopes. divan's `--exact` wants the
# full path, `frame::quiet_48k`; a bare `quiet_48k` matches nothing and measures an empty run,
# so a slope near zero is refused below rather than recorded.
per_frame() { # binary path::case
    slopes=""
    for repeat in 1 2 3; do
        low=$(instructions "$1" 500 "$2") || return 1
        high=$(instructions "$1" 1500 "$2") || return 1
        if [ -z "$low" ] || [ -z "$high" ]; then return 1; fi
        slope=$(( (high - low) / 1000 ))
        if [ "$slope" -lt 10000 ]; then return 1; fi
        slopes="$slopes $slope"
    done
    printf '%s\n' $slopes | sort -n | sed -n 2p
}

bench_binary() { # package bench — empty, so the build check below fails, when cargo fails
    cargo bench --locked -p "$1" --bench "$2" --no-run > "$report" 2>&1 || return 1
    sed -n "s|.*Executable benches/$2.rs (\(.*\))|\1|p" "$report" | tail -n 1
}

if [ -z "$counter" ]; then
    printf '    SKIP no instruction counter on this host (macOS /usr/bin/time -l, or Linux perf)\n'
    exit 3
fi

arch=$(uname -m)
rustc=$(rustc --version | cut -d' ' -f2)
if [ "$mode" = check ]; then
    recorded=$(sed -n 's/^# toolchain //p' "$BASELINE" 2>/dev/null)
    # Same architecture, another compiler (or no header at all) is a baseline nobody re-recorded:
    # a failure, not a question. Another architecture is a question this host cannot answer.
    case "$recorded" in
        "$arch rustc $rustc") ;;
        "$arch "* | "") printf '    FAIL baseline is for "%s"; this host is "%s rustc %s"; bump rust-toolchain.toml and re-record from the merge base in one commit (docs/PERFORMANCE.md)\n' \
            "$recorded" "$arch" "$rustc"; exit 1 ;;
        *) printf '    SKIP baseline is for "%s"; this host is "%s rustc %s"; another architecture\n' \
            "$recorded" "$arch" "$rustc"; exit 3 ;;
    esac
fi

frame=$(bench_binary spectrum frame)
step=$(bench_binary z80 step)
if [ ! -x "$frame" ] || [ ! -x "$step" ]; then
    printf '    FAIL could not build the bench binaries (frame=%s step=%s)\n' "$frame" "$step"
    exit 1
fi

measured=""
for name in $FRAME_CASES; do
    value=$(per_frame "$frame" "frame::$name") || { printf '    FAIL frame %s produced no count\n' "$name"; exit 1; }
    measured="$measured
frame $name $value"
done
for name in $STEP_CASES; do
    value=$(per_frame "$step" "step::$name") || { printf '    FAIL step %s produced no count\n' "$name"; exit 1; }
    measured="$measured
step $name $value"
done
measured=$(printf '%s\n' "$measured" | sed '/^$/d')

if [ "$mode" = measure ]; then
    printf '%s\n' "$measured"
    exit 0
fi

if [ "$mode" = record ]; then
    {
        printf '# Instructions retired per frame (per sample for step), median of three slopes.\n'
        printf '# Written by `sh web/perf_gate.sh --record`; checked by `sh web/perf_gate.sh`.\n'
        printf '# recorded %s on %s\n' "$(date +%Y-%m-%d)" "$(sysctl -n hw.model 2>/dev/null || uname -n)"
        printf '# toolchain %s rustc %s\n' "$arch" "$rustc"
        printf '%s\n' "$measured"
    } > "$BASELINE"
    printf '    recorded %s\n' "$BASELINE"
    exit 0
fi

# Every baseline row must be measured, and every measured row must have a baseline: a case
# that silently fell out of either list is a gate that stopped asking.
failures=0
rows=0
while read -r bench name base; do
    case "$bench" in ''|'#'*) continue ;; esac
    rows=$((rows + 1))
    now=$(printf '%s\n' "$measured" | awk -v b="$bench" -v n="$name" '$1 == b && $2 == n { print $3 }')
    if [ -z "$now" ]; then
        printf '    FAIL %s %s is in the baseline and was not measured\n' "$bench" "$name"
        failures=$((failures + 1))
        continue
    fi
    # A baseline row that is not a positive count is a gate that cannot compare: it fails, and so
    # does any verdict this cannot read — an empty one is never a pass.
    case "$base" in ''|*[!0-9]*|0) base="not-a-count:$base" ;; esac
    verdict=$(awk -v now="$now" -v base="$base" -v t="$THRESHOLD_PERCENT" 'BEGIN {
        delta = (now - base) * 100 / base
        printf "%+.2f%% %s", delta, (delta > t ? "FAIL" : delta < -t ? "gain" : "ok")
    }')
    case "$verdict" in
        *[0-9]%\ FAIL) printf '    FAIL %-5s %-17s %9s → %9s  %s (limit +%s%%)\n' "$bench" "$name" "$base" "$now" "${verdict% FAIL}" "$THRESHOLD_PERCENT"
               failures=$((failures + 1)) ;;
        *[0-9]%\ ok) printf '    ok   %-5s %-17s %9s → %9s  %s\n' "$bench" "$name" "$base" "$now" "${verdict% ok}" ;;
        *[0-9]%\ gain) printf '    ok   %-5s %-17s %9s → %9s  %s  gain > %s%%: re-record the baseline (docs/PERFORMANCE.md)\n' "$bench" "$name" "$base" "$now" "${verdict% gain}" "$THRESHOLD_PERCENT" ;;
        *) printf '    FAIL %-5s %-17s could not compare %s with the baseline %s\n' "$bench" "$name" "$now" "$base"
           failures=$((failures + 1)) ;;
    esac
done < "$BASELINE"
missing=$(printf '%s\n' "$measured" | while read -r bench name value; do
    grep -q "^$bench $name " "$BASELINE" || printf '%s %s ' "$bench" "$name"
done)
if [ -n "$missing" ]; then
    printf '    FAIL measured but absent from the baseline: %s\n' "$missing"
    failures=$((failures + 1))
fi
if [ "$rows" -eq 0 ]; then
    printf '    FAIL %s holds no rows — an empty baseline grades nothing\n' "$BASELINE"
    exit 1
fi
[ "$failures" -eq 0 ]
