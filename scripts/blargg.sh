#!/usr/bin/env bash
# Run every Blargg test ROM in a directory and summarize the serial output.
#
# Usage: scripts/blargg.sh <rom-dir> [timeout-seconds]

set -u

rom_dir=${1:?usage: $0 <rom-dir> [timeout-seconds]}
timeout_secs=${2:-10}

cd "$(dirname "$0")/.." || exit 1
# only show compiler output if the build fails
cargo build --release -q 2>/dev/null || { cargo build --release; exit 1; }
emu=./target/release/juegoboy

out=$(mktemp)
err=$(mktemp)
trap 'rm -f "$out" "$err"' EXIT

passed=0
total=0

shopt -s nullglob
for rom in "$rom_dir"/*.gb; do
    total=$((total + 1))
    name=$(basename "$rom" .gb)

    RUST_LOG=off "$emu" "$rom" >"$out" 2>"$err" &
    pid=$!

    # Blargg ROMs loop forever after reporting, so stop as soon as a result shows up
    deadline=$((SECONDS + timeout_secs))
    while kill -0 "$pid" 2>/dev/null && ((SECONDS < deadline)); do
        grep -qE 'Passed|Failed' "$out" && break
        sleep 0.1
    done
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    status=$?

    if grep -q 'Passed' "$out"; then
        passed=$((passed + 1))
        printf '\e[32mPASS\e[0m  %s\n' "$name"
    elif grep -q 'Failed' "$out"; then
        printf '\e[31mFAIL\e[0m  %s\n' "$name"
        # everything after the test name line: failing opcodes and "Failed #n"
        tail -n +2 "$out" | grep -v '^$' | sed 's/^/      /'
    elif [ "$status" -eq 101 ]; then
        printf '\e[31mCRASH\e[0m %s\n' "$name"
        grep -m1 -A1 'panicked' "$err" | sed 's/^/      /'
    else
        printf '\e[33mTIME\e[0m  %s (no result after %ss)\n' "$name" "$timeout_secs"
    fi
done

if [ "$total" -eq 0 ]; then
    echo "No .gb files found in $rom_dir"
    exit 1
fi

echo
echo "$passed/$total passed"
[ "$passed" -eq "$total" ]
