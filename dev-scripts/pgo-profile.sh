#!/bin/bash
# Records which paths of the CPU interpreter are hot while playing the start
# of Rayman Fiesta Run's first level, for profile-guided optimisation, and
# merges the recording into src/cpu/mgba_wrapper/pgo/interpreter.profdata,
# which builds with clang (Xcode's included) then use.
#
# Needs clang, llvm-profdata and clang's profiling runtime (on Ubuntu,
# libclang-rt-18-dev), besides headless-play.sh's tools.
# usage: dev-scripts/pgo-profile.sh GAME.ipa
set -eu
ipa=$1
cd "$(dirname "$0")/.."
out=$(mktemp -d)
CC=clang CXX=clang++ TOUCHHLE_PGO_GENERATE=1 CARGO_TARGET_DIR=target-pgogen \
    cargo build --release
taps=$(for i in $(seq 1 16); do printf 'tap:300,200 wait:3 '; done)
# The emulator writes the profile as it exits, which it does on the SIGTERM
# headless-play.sh ends it with.
LLVM_PROFILE_FILE="$out/rayman-%p.profraw" IDARED_BIN=target-pgogen/release/touchHLE \
    dev-scripts/headless-play.sh "$ipa" "$out/play" \
    wait:70 tap:250,170 wait:45 tap:241,144 wait:25 tap:60,250 wait:2 $taps
mkdir -p src/cpu/mgba_wrapper/pgo
llvm-profdata merge -o src/cpu/mgba_wrapper/pgo/interpreter.profdata "$out"/*.profraw
echo "Wrote src/cpu/mgba_wrapper/pgo/interpreter.profdata"
