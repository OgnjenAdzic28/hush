#!/usr/bin/env bash
# Drives real Linux apps through the hush binary, the way an agent does:
# start each on Hush's display, read it, type into it, read the text back and
# take a picture. Run inside a session bus and an X server:
#
#   xvfb-run -a dbus-run-session -- scripts/linux-apps.sh target/release/hush APP...
#
# APP is one of: calculator, gtk3, gtk4, writer, qt, firefox.
set -euo pipefail

hush=$1
shift
out=${RUNNER_TEMP:-/tmp}/hush-apps
mkdir -p "$out"
failed=()

# Polls observe until TEXT shows. Apps answer in their own time (LibreOffice
# and Firefox take seconds to start), so this waits on what the app shows
# with a deadline, 0.2 s between looks.
shows() {
  local app=$1 text=$2 seconds=$3
  for _ in $(seq $((seconds * 5))); do
    if "$hush" observe "$app" --full >"$out/last.txt" 2>&1 && grep -q -- "$text" "$out/last.txt"; then
      return 0
    fi
    sleep 0.2
  done
  echo "--- $app never showed \"$text\"; last look:"
  cat "$out/last.txt"
  return 1
}

# Waits for the app to have a window with something in it: an outline
# line, not only a note ("# no window yet").
opened() {
  local app=$1 seconds=$2
  "$hush" observe "$app" | tee "$out/$app-first.txt" || return 1
  for _ in $(seq $((seconds * 5))); do
    if "$hush" observe "$app" --full >"$out/last.txt" 2>&1 && grep -qv '^#' "$out/last.txt"; then
      cat "$out/last.txt" | head -40
      return 0
    fi
    sleep 0.2
  done
  echo "--- $app has no window; last look:"
  cat "$out/last.txt"
  return 1
}

typed() {
  local app=$1 seconds=$2
  opened "$app" "$seconds" &&
    "$hush" act "$app" "type hello from hush" &&
    shows "$app" "hello from hush" 10
}

pictured() {
  local app=$1 name=$2
  "$hush" see "$app" --out "$out/$name.jpg" && test -s "$out/$name.jpg"
}

run() {
  local name=$1
  local started=$SECONDS
  echo "::group::$name"
  # errexit is off inside an if, so each check chains its steps with &&.
  if "check_$name"; then
    echo "::endgroup::"
    echo "ok   $name ($((SECONDS - started)) s)"
  else
    echo "::endgroup::"
    echo "FAIL $name"
    failed+=("$name")
  fi
}

check_calculator() {
  opened Calculator 20 &&
    "$hush" act Calculator "key 4" "key 2" &&
    shows Calculator '\b42\b' 10 &&
    pictured Calculator calculator
}
check_gtk3() { typed Mousepad 20 && pictured Mousepad gtk3; }
check_gtk4() { typed "Text Editor" 20 && pictured "Text Editor" gtk4; }
check_writer() { typed "LibreOffice Writer" 60 && pictured "LibreOffice Writer" writer; }
check_qt() { typed FeatherPad 20 && pictured FeatherPad qt; }
check_firefox() {
  opened Firefox 60 && pictured Firefox firefox
}

for app in "$@"; do
  run "$app"
done
"$hush" apps
if "$hush" apps | grep -q "front"; then
  echo "FAIL an app on Hush's display is listed as the front app"
  failed+=(front)
fi
pkill -x hush || true

if ((${#failed[@]})); then
  echo "failed: ${failed[*]}"
  exit 1
fi
