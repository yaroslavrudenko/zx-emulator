#!/bin/sh
# The wall-clock A/B a performance-touching change pastes into its record. Evidence, never a
# verdict: it prints numbers and always exits 0 once it has measured.
#
#     git worktree add --detach ../zx-base "$(git merge-base HEAD main)"
#     sh web/perf_ab.sh ../zx-base . [case ...]      # default cases: quiet_48k tape_playing_48k
#
# Each tree's `frame` bench is built in its own target directory, then the two binaries run
# **interleaved** — A, B, A, B … — six times each, and each case reports the **minimum** of the
# six `fastest` figures with the load average before and after.
#
# Why this shape, and why it is not a gate (DECISION.md item 2): one unchanged binary's wall
# floor moved 7–12 % between back-to-back runs at a load of 7 on the M3 Max — bimodal, about
# 69 against 75 µs, P-core against E-core placement — so a single pair of runs says nothing,
# and a minimum over runs that alternate is the only estimator that lets both sides land on the
# same kind of core. Instructions retired (`web/perf_gate.sh`) are the gate; this is here because
# instructions miss layout and branch effects — the bounded prefix dispatch cost +0.4 %
# instructions and +3.3 % wall.

set -u

if [ "$#" -lt 2 ]; then
    printf 'usage: sh web/perf_ab.sh <tree A> <tree B> [case ...]\n' >&2
    exit 2
fi
a=$(CDPATH= cd -- "$1" && pwd) || exit 2
b=$(CDPATH= cd -- "$2" && pwd) || exit 2
shift 2
cases=${*:-quiet_48k tape_playing_48k}
RUNS=6

build() { # tree → bench binary, as an absolute path: cargo prints it relative to the tree
    path=$( (cd "$1" && CARGO_TARGET_DIR="$1/target/perf-ab" cargo bench --locked -p spectrum --bench frame --no-run 2>&1) |
        sed -n 's|.*Executable benches/frame.rs (\(.*\))|\1|p' | tail -n 1)
    case "$path" in /*) printf '%s\n' "$path" ;; ?*) printf '%s/%s\n' "$1" "$path" ;; esac
}

fastest() { # binary case → µs
    "$1" --bench --exact "frame::$2" 2>/dev/null | awk -v name="$2" '$2 == name {
        value = $3; unit = $4
        if (unit == "ns") value /= 1000; else if (unit == "µs") value += 0; else if (unit == "ms") value *= 1000; else if (unit == "s") value *= 1000000; else next
        printf "%.2f", value; exit
    }'
}

bin_a=$(build "$a")
bin_b=$(build "$b")
if [ ! -x "$bin_a" ] || [ ! -x "$bin_b" ]; then
    printf 'could not build both benches (A=%s B=%s)\n' "$bin_a" "$bin_b" >&2
    exit 1
fi

printf 'A %s\nB %s\nload before: %s\n' "$a" "$b" "$(uptime | sed 's/.*load/load/')"
for name in $cases; do
    min_a="" min_b="" runs_a="" runs_b=""
    run=1
    while [ "$run" -le "$RUNS" ]; do
        x=$(fastest "$bin_a" "$name"); y=$(fastest "$bin_b" "$name")
        runs_a="$runs_a $x"; runs_b="$runs_b $y"
        min_a=$(printf '%s\n' $runs_a | sort -n | head -n 1)
        min_b=$(printf '%s\n' $runs_b | sort -n | head -n 1)
        run=$((run + 1))
    done
    printf '%-17s A min %8s µs  B min %8s µs  %s\n' "$name" "$min_a" "$min_b" \
        "$(awk -v a="$min_a" -v b="$min_b" 'BEGIN { if (a > 0 && b > 0) printf "%+.1f%%", (b - a) * 100 / a; else printf "no reading" }')"
    printf '                  A runs:%s\n                  B runs:%s\n' "$runs_a" "$runs_b"
done
printf 'load after:  %s\n' "$(uptime | sed 's/.*load/load/')"
