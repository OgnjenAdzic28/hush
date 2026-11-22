#!/usr/bin/env bash
# Runs Hush against a real desktop: GNOME Shell on Wayland, headless, with
# Calculator open on it the way a person would have it. Checks that Hush
# reads and clicks the person's app through accessibility, refuses raw keys
# and pictures of it, and starts its own apps on its hidden display, where
# they never reach the desktop. Run inside a session bus:
#
#   dbus-run-session -- scripts/linux-desktop.sh target/release/hush
set -euo pipefail

hush=$1
out=${RUNNER_TEMP:-/tmp}/hush-desktop
mkdir -p "$out"
fail() {
  echo "FAIL $*"
  exit 1
}

export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=GNOME
gnome-shell --headless --wayland --virtual-monitor 1280x800 >"$out/shell.log" 2>&1 &
# GNOME Shell takes 1 to 2 s to come up; wait for its Wayland socket.
for _ in $(seq 100); do
  [ -S "$XDG_RUNTIME_DIR/wayland-0" ] && break
  sleep 0.1
done
[ -S "$XDG_RUNTIME_DIR/wayland-0" ] || fail "GNOME Shell never came up"
export WAYLAND_DISPLAY=wayland-0
unset DISPLAY

# The person's own Calculator, on the desktop.
gnome-calculator &
user_calc=$!

# Polls observe until TEXT shows, 0.1 s between looks.
shows() {
  local app=$1 text=$2
  for _ in $(seq 100); do
    "$hush" observe "$app" --full >"$out/last.txt" 2>&1 && grep -q -- "$text" "$out/last.txt" && return 0
    sleep 0.1
  done
  cat "$out/last.txt"
  fail "$app never showed $text"
}

echo "--- Hush reads the person's Calculator, without starting another"
shows Calculator 'button "7"'
if grep -q "started" "$out/last.txt"; then fail "Hush started a second Calculator"; fi
"$hush" apps | tee "$out/apps.txt"

echo "--- and clicks it through accessibility"
seven=$(grep -m1 -oE '\bb[0-9]+ button "7"' "$out/last.txt" | cut -d' ' -f1)
"$hush" act Calculator "click $seven"
shows Calculator 'textarea = "7"'

echo "--- but sends it no raw keys and takes no pictures of it"
"$hush" act Calculator "key 4" | tee "$out/keys.txt"
grep -q "stopped at step 1" "$out/keys.txt" || fail "a raw key went to the person's app"
if "$hush" see Calculator --out "$out/user.jpg"; then fail "Hush pictured the person's app"; fi

echo "--- Text Editor starts on Hush's display, and takes typing there"
"$hush" observe "Text Editor" | tee "$out/editor.txt"
grep -q "started Text Editor" "$out/editor.txt" || fail "Text Editor didn't start"
shows "Text Editor" 'textarea'
"$hush" act "Text Editor" "type hello from hush"
shows "Text Editor" 'textarea = "hello from hush"'
"$hush" see "Text Editor" --out "$out/editor.jpg"

echo "--- and never reaches the desktop"
editor=$(pgrep -n gnome-text-edit)
env=$(tr '\0' '\n' <"/proc/$editor/environ")
if grep -q '^WAYLAND_DISPLAY=' <<<"$env"; then fail "Text Editor can reach the Wayland desktop"; fi
grep '^DISPLAY=' <<<"$env"
if "$hush" apps | grep "Text Editor.*front"; then fail "Hush's app is listed as the front app"; fi

echo "--- the person's Calculator is as the click left it"
shows Calculator 'textarea = "7"'
kill "$user_calc"
pkill -x hush || true
echo "ok   desktop"
