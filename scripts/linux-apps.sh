#!/usr/bin/env bash
# Drives real Linux apps through the hush binary, the way an agent does:
# start each on Hush's display, read it, type into it, read the text back and
# take a picture. Run inside a session bus and an X server:
#
#   xvfb-run -a dbus-run-session -- scripts/linux-apps.sh target/release/hush APP...
#
# APP is one of: calculator, gtk3, gtk4, writer, qt, firefox, chrome. When an app
# leaves something out, scripts/atspi-tree.py prints what it reports.
set -euo pipefail

hush=$1
shift
out=${RUNNER_TEMP:-/tmp}/hush-apps
mkdir -p "$out"
failed=()

# Every command has a deadline. One that misses it is a bug in Hush, so the
# host's threads are dumped to show where it stuck.
h() {
  local code=0
  timeout 60 "$hush" "$@" || code=$?
  if ((code == 124)); then
    echo "--- hush $* took over 60 s; the host's threads:"
    if command -v gdb >/dev/null; then
      sudo gdb -p "$(pgrep -xo hush)" -batch -ex "thread apply all bt" 2>&1 | grep -E '^(Thread|#)' | head -150
    fi
  fi
  return $code
}

# Polls observe until TEXT shows. Apps answer in their own time (LibreOffice
# and Firefox take seconds to start), so this waits on what the app shows
# with a deadline, 0.2 s between looks.
shows() {
  local app=$1 text=$2 seconds=$3
  local end=$((SECONDS + seconds)) code
  while ((SECONDS < end)); do
    code=0
    h observe "$app" --full >"$out/last.txt" 2>&1 || code=$?
    ((code == 124)) && { cat "$out/last.txt"; return 1; }
    if ((code == 0)) && grep -q -- "$text" "$out/last.txt"; then
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
  h observe "$app" | tee "$out/$app-first.txt" || return 1
  local end=$((SECONDS + seconds)) code
  while ((SECONDS < end)); do
    code=0
    h observe "$app" --full >"$out/last.txt" 2>&1 || code=$?
    ((code == 124)) && { cat "$out/last.txt"; return 1; }
    if ((code == 0)) && grep -qv '^#' "$out/last.txt"; then
      head -60 "$out/last.txt"
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
  # Type into the first text area, by its ref, as an agent would.
  opened "$app" "$seconds" &&
    ref=$(grep -m1 -oE '\b[t][0-9]+ (textarea|textfield|document)' "$out/last.txt" | cut -d' ' -f1) &&
    h act "$app" "type $ref hello from hush" &&
    shows "$app" "hello from hush" 10
}

pictured() {
  local app=$1 name=$2
  h see "$app" --out "$out/$name.jpg" && test -s "$out/$name.jpg"
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
    h act Calculator "key 4" "key 2" &&
    shows Calculator '\b42\b' 10 &&
    pictured Calculator calculator
}
check_gtk3() { typed Mousepad 20 && pictured Mousepad gtk3; }
check_gtk4() { typed "Text Editor" 20 && pictured "Text Editor" gtk4; }
check_writer() { typed "LibreOffice Writer" 60 && pictured "LibreOffice Writer" writer; }
check_qt() { typed FeatherPad 20 && pictured FeatherPad qt; }
# Loads a page and reads its text back.
check_firefox() {
  opened Firefox 60 && pictured Firefox firefox-start || return 1
  # A new profile asks to accept the terms first; an agent clicks Continue.
  if shows Firefox 'button "Continue"' 10; then
    h act Firefox "click $(grep -m1 -oE '\bb[0-9]+ button "Continue"' "$out/last.txt" | cut -d' ' -f1)" || return 1
  fi
  # Typed as key presses; ✓ and é aren't on the keyboard, so they go on
  # spare keys, which must come back for the second page.
  h act Firefox "key ctrl+l" "type data:text/html;charset=utf-8,<h1>hello from hush ✓</h1>" "key Return" &&
    shows Firefox 'heading "hello from hush ✓"' 20 &&
    h act Firefox "key ctrl+l" "type data:text/html;charset=utf-8,<h1>café ✓</h1>" "key Return" &&
    shows Firefox 'heading "café ✓"' 20 &&
    pictured Firefox firefox
}

# Chromium's family, the same way: load a page and read its heading.
check_chrome() {
  if ! opened "Google Chrome" 60; then
    ps -eo pid,args | grep "[c]hrome" | cut -c1-160 | head -8
    return 1
  fi
  pictured "Google Chrome" chrome-start || return 1
  h act "Google Chrome" "key ctrl+l" "type data:text/html;charset=utf-8,<h1>hello from hush ✓</h1>" "key Return" &&
    shows "Google Chrome" 'heading "hello from hush ✓"' 20 &&
    pictured "Google Chrome" chrome
}

for app in "$@"; do
  run "$app"
done
# Calculator is under every app started after it now; it must still
# picture as itself.
if [[ " $* " == *" calculator "* ]]; then
  h see Calculator --out "$out/calculator-under.jpg" || failed+=(under)
fi
h apps
if h apps | grep -q "front"; then
  echo "FAIL an app on Hush's display is listed as the front app"
  failed+=(front)
fi
pkill -x hush || true

if ((${#failed[@]})); then
  echo "failed: ${failed[*]}"
  exit 1
fi
