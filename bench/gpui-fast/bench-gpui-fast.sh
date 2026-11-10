#!/usr/bin/env bash
# GPUI Fast spike (karyalabs/matty#828): bench master against GPUI Fast on this Mac.
# Builds two release binaries with GPUI's own frame/latency histograms
# (--features gpui-profiler), then runs bench/run.py three times, Matty only:
#   master, fast, fast with GPUI_SCROLL_LAYERS=0.
# Usage: bash bench-gpui-fast.sh [path-to-matty-checkout]   (default ~/projects/ui)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="${1:-$HOME/projects/ui}"
cd "$repo"
git remote get-url origin | grep -q 'matty' || { echo "error: $repo is not a matty checkout" >&2; exit 1; }
git fetch -q origin master claude/project-thread-ngfu9u
git merge-base --is-ancestor b921e43 HEAD || { echo "error: $repo's bench/ predates #821; update it to master first" >&2; exit 1; }

work="$repo/target/gpui-fast-bench"
out="$repo/output/matty-bench/gpui-fast-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$work" "$out"
export CARGO_TARGET_DIR="$work/target"

checkout() { # name ref
    local dir="$work/$1"
    if [[ -d "$dir" ]]; then git -C "$dir" checkout -q -f --detach "$2"; git -C "$dir" clean -qfd
    else git worktree add -q -f --detach "$dir" "$2"; fi
}

checkout master origin/master
git -C "$work/master" apply "$here/gpui-profiler.patch"
checkout fast origin/claude/project-thread-ngfu9u

for v in master fast; do
    echo "== building $v ($(git -C "$work/$v" rev-parse --short HEAD))"
    (cd "$work/$v" && cargo build --release -p matty --features gpui-profiler)
    cp "$CARGO_TARGET_DIR/release/matty" "$work/matty-$v"
done

# The bench server (Rosetta Docker) must be up with the corpus loaded.
colima status >/dev/null 2>&1 || colima start --vz-rosetta --cpu 10 --memory 16
bench/server up
bench/server status

run() { # label binary [env...]
    local label="$1" bin="$2"; shift 2
    echo "== bench $label"
    env "$@" bench/.venv/bin/python bench/run.py --apps matty --app "matty=$bin" \
        --fps 120 --no-video --out "$out/$label"
}
run master "$work/matty-master"
run fast "$work/matty-fast"
run fast-nolayers "$work/matty-fast" GPUI_SCROLL_LAYERS=0

# One summary: each run's report, plus GPUI's histograms (last line of each launch).
{
    for label in master fast fast-nolayers; do
        echo "######## $label"
        cat "$out/$label/report.md" 2>/dev/null || echo "(no report.md)"
        for f in "$out/$label"/matty/perf-*.log.gpui.jsonl; do
            [[ -f "$f" ]] && echo "GPUI $(basename "$f"): $(tail -1 "$f")"
        done
    done
} > "$out/summary.txt"
echo "== done: $out/summary.txt"
