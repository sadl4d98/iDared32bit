#!/bin/bash
# Measures the emulator's main-thread CPU time per presented frame while
# playing the start of Rayman Fiesta Run's first level, the same way each
# run. Lower is better. Needs perf (linux-tools-generic) besides
# headless-play.sh's tools.
# usage: dev-scripts/bench-rayman.sh GAME.ipa OUT_DIR
set -u
ipa=$1; out=$2
perf=$(ls /usr/lib/linux-tools/*/perf | head -1)
cd "$(dirname "$0")/.."
taps=$(for i in $(seq 1 16); do printf 'tap:300,200 wait:3 '; done)
dev-scripts/headless-play.sh "$ipa" "$out" wait:70 tap:250,170 wait:45 tap:241,144 wait:25 tap:60,250 wait:2 $taps &
sleep 150
pid=$(pgrep -f "target/release/touchHLE" | head -1)
before=$(grep -c "EAGLContext.*FPS" "$out/log.txt")
"$perf" stat -e task-clock -t "$pid" -x, -o "$out/stat.txt" -- sleep 25
after_lines=$(grep "EAGLContext.*FPS" "$out/log.txt" | tail -n +$((before + 1)))
wait
frames=$(echo "$after_lines" | head -25 | awk '{s += $NF} END {print s}')
cpu_ms=$(grep task-clock "$out/stat.txt" | cut -d, -f1)
echo "frames=$frames cpu_ms=$cpu_ms ms_per_frame=$(echo "$cpu_ms $frames" | awk '{printf "%.2f", $1 / $2}')"
