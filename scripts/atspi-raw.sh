#!/usr/bin/env bash
# Asks one app's AT-SPI objects directly over D-Bus, for when a toolkit's
# tree looks empty: scripts/atspi-raw.sh PID
pid=$1
addr=$(dbus-send --session --print-reply=literal --dest=org.a11y.Bus /org/a11y/bus org.a11y.Bus.GetAddress | tr -d ' ')
name=""
for n in $(gdbus call --address "$addr" -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.ListNames | grep -oE "':[0-9.]+'" | tr -d "'"); do
  p=$(gdbus call --address "$addr" -d org.freedesktop.DBus -o /org/freedesktop/DBus -m org.freedesktop.DBus.GetConnectionUnixProcessID "$n" 2>/dev/null | grep -oE 'uint32 [0-9]+' | cut -d' ' -f2)
  [ "$p" = "$pid" ] && name=$n && break
done
echo "bus name of $pid: $name"
ask() { gdbus call --address "$addr" -d "$name" -o "$1" -m "$2" "${@:3}" 2>&1 | cut -c1-300; }
prop() { gdbus call --address "$addr" -d "$name" -o "$1" -m org.freedesktop.DBus.Properties.Get org.a11y.atspi.Accessible "$2" 2>&1 | cut -c1-200; }
path=/org/a11y/atspi/accessible/root
for depth in 0 1 2 3; do
  echo "[$depth] $path count=$(prop "$path" ChildCount) role=$(ask "$path" org.a11y.atspi.Accessible.GetRoleName)"
  echo "    children: $(ask "$path" org.a11y.atspi.Accessible.GetChildren)"
  child=$(ask "$path" org.a11y.atspi.Accessible.GetChildAtIndex 0)
  echo "    child 0: $child"
  path=$(grep -oE "objectpath '[^']+'" <<<"$child" | head -1 | cut -d"'" -f2)
  [ -z "$path" ] && break
done
