#!/usr/bin/env bash
# Plays Moonlight Sonata mvt 1 on virtualpiano.net through `hush timeline`, quietly, and reports
# the timing. See README.md. Needs Accessibility for the calling process and ffmpeg.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$here/../.."
work="$(mktemp -d -t hush-piano)"
echo "working in $work"

mkdir -p "$work/midi" "$work/HushPiano.app/Contents/MacOS"
curl -sSL -o "$work/midi/mids.zip" https://www.mutopiaproject.org/ftp/BeethovenLv/O27/moonlight/moonlight-mids.zip
(cd "$work/midi" && unzip -o -q mids.zip moonlight1.mid)

cat > "$work/HushPiano.app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.hush.piano</string>
<key>CFBundleName</key><string>HushPiano</string>
<key>CFBundleExecutable</key><string>HushPiano</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
swiftc -O -o "$work/HushPiano.app/Contents/MacOS/HushPiano" "$here/HushPiano.swift"
codesign --force --sign - "$work/HushPiano.app" 2>/dev/null
swiftc -O -o "$work/front" "$here/Front.swift"
(cd "$repo" && cargo build --release -q)
hush="$repo/target/release/hush"

python3 -I "$here/compile.py" "$work/midi/moonlight1.mid" "$work/score.txt"
# A click to give the page focus, one warm-up key, then the piece 1.5 s in.
{ echo "0 tap 700,120"; echo "300 tap x"; awk '{ $1 = $1 + 1500; print }' "$work/score.txt"; } > "$work/play.txt"

"$work/front" "$work/front.log" & watcher=$!
open -g -n -a "$work/HushPiano.app" --args https://virtualpiano.net/ "$work/page.log" "$work/recording.m4a"
for _ in $(seq 1 60); do grep -q loaded "$work/page.log" 2>/dev/null && break; sleep 0.5; done
"$hush" act HushPiano lane
"$hush" timeline HushPiano - < "$work/play.txt"
sleep 2
pkill -USR1 -x HushPiano
for _ in $(seq 1 40); do [ "$(grep -c audio_saved "$work/page.log")" -ge 2 ] && break; sleep 0.5; done
kill "$watcher"
pkill -x HushPiano || true

# The piano plays through one of the page's audio contexts; keep the loudest recording.
loudest=$(for f in "$work"/recording*.m4a; do
  echo "$(ffmpeg -hide_banner -i "$f" -af volumedetect -f null - 2>&1 | sed -n 's/.*max_volume: \(.*\) dB/\1/p') $f"
done | sort -gr | head -1 | cut -d' ' -f2-)
grep " down " "$work/play.txt" | grep -v tap > "$work/schedule.txt"
python3 -I "$here/onsets.py" "$loudest" "$work/schedule.txt"
python3 -I - "$work" <<'EOF'
import json, statistics, sys
work = sys.argv[1]
scheduled = [float(l.split()[0]) for l in open(f"{work}/play.txt") if " down " in l]
arrived = [json.loads(l)["t"] for l in open(f"{work}/page.log") if '"keydown"' in l]
if len(arrived) == len(scheduled) + 1:
    arrived = arrived[1:]  # the warm-up key
delays = [a - s for a, s in zip(arrived, scheduled)]
typical = statistics.median(delays)
errors = sorted(abs(d - typical) for d in delays)
print(f"{len(arrived)} of {len(scheduled)} notes reached the page")
print(f"page arrival vs score: median {errors[len(errors)//2]:.0f} ms, 95% {errors[int(len(errors)*.95)]:.0f} ms, worst {errors[-1]:.0f} ms")
EOF
echo "front-app changes:"; cat "$work/front.log"
echo "recording: $loudest"
