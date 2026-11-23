#!/bin/bash
# Runs a game in the Linux build on a virtual screen, taps and takes
# screenshots, for testing without a phone. Needs Xvfb, xdotool, ImageMagick
# and Mesa's EGL (the iDared32bit cloud environment's setup script installs them).
#
# usage: dev-scripts/headless-play.sh GAME.ipa OUT_DIR STEP...
#   steps: wait:SECONDS  tap:X,Y (window pixels; 480x320 landscape unscaled)  shot:LABEL
# Emulator options can be passed in IDARED_OPTS, e.g. IDARED_OPTS=--scale-hack=2.
# The log goes to OUT_DIR/log.txt and screenshots to OUT_DIR/LABEL.png.
set -u
ipa=$1; out=$2; shift 2
mkdir -p "$out"
pgrep -x Xvfb >/dev/null || { Xvfb :99 -screen 0 2400x1600x24 >/dev/null 2>&1 & sleep 2; }
export DISPLAY=:99 SDL_AUDIODRIVER=dummy LIBGL_ALWAYS_SOFTWARE=1
cd "$(dirname "$0")/.."
target/release/touchHLE "$ipa" --no-error-popup --print-fps ${IDARED_OPTS:-} > "$out/log.txt" 2>&1 &
pid=$!
locate_window() { eval "$(xdotool getwindowgeometry --shell "$(xdotool search --pid $pid | tail -1)")"; }
for step in "$@"; do
  kill -0 $pid 2>/dev/null || { echo "Game exited before $step; see $out/log.txt"; break; }
  case $step in
    wait:*) sleep "${step#wait:}" ;;
    tap:*) locate_window; xy=${step#tap:}
      xdotool mousemove $((X + ${xy%,*})) $((Y + ${xy#*,})) mousedown 1 sleep 0.15 mouseup 1 ;;
    shot:*) locate_window
      import -window root -crop "${WIDTH}x${HEIGHT}+$X+$Y" +repage "$out/${step#shot:}.png" ;;
  esac
done
kill $pid 2>/dev/null; wait $pid 2>/dev/null
