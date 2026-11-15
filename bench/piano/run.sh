#!/usr/bin/env bash
# Plays a MIDI file on virtualpiano.net through `hush timeline`, quietly, records the window and the
# page's audio, and writes OUT.mp4 (synced) and OUT.m4a. See README.md.
#
# Usage: run.sh [MIDI] [OUT] [--track NAME[,NAME]]
#   With no MIDI, downloads Moonlight Sonata mvt 1 from the Mutopia Project.
# Needs Accessibility and Screen Recording for the calling process, and ffmpeg.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$here/../.."
midi="${1:-}"; out="${2:-$PWD/hush-piano}"; shift $(( $# > 2 ? 2 : $# )) || true
work="$(mktemp -d -t hush-piano)"
echo "working in $work"

if [ -z "$midi" ]; then
  curl -sSL -o "$work/mids.zip" https://www.mutopiaproject.org/ftp/BeethovenLv/O27/moonlight/moonlight-mids.zip
  (cd "$work" && unzip -o -q mids.zip moonlight1.mid)
  midi="$work/moonlight1.mid"
fi

mkdir -p "$work/HushPiano.app/Contents/MacOS"
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
swiftc -O -o "$work/rec" "$here/Rec.swift"
(cd "$repo" && cargo build --release -q)
hush="$repo/target/release/hush"

python3 -I "$here/compile.py" "$midi" "$work/score.txt" "$@"
# A click to give the page focus, one warm-up key, then the piece 1.5 s in.
{ echo "0 tap 700,120"; echo "300 tap x"; awk '{ $1 = $1 + 1500; print }' "$work/score.txt"; } > "$work/play.txt"

pkill -x HushPiano || true
"$work/front" "$work/front.log" & watcher=$!
open -g -n -a "$work/HushPiano.app" --args https://virtualpiano.net/ "$work/page.log" "$work/audio.m4a"
for _ in $(seq 1 60); do grep -q loaded "$work/page.log" 2>/dev/null && break; sleep 0.5; done
sleep 1
"$hush" act HushPiano lane
"$work/rec" HushPiano "Hush Piano" "$work/video.mp4" 30 > "$work/rec.log" 2>&1 & rec=$!
sleep 1.5
"$hush" timeline HushPiano - < "$work/play.txt"
sleep 2.5
kill -INT "$rec"; wait "$rec" || true
pkill -USR1 -x HushPiano
for _ in $(seq 1 40); do [ "$(grep -c audio_saved "$work/page.log")" -ge 2 ] && break; sleep 0.5; done
kill "$watcher"; pkill -x HushPiano || true

python3 -I - "$work" "$out" <<'PY'
import glob, json, re, statistics, subprocess, sys
work, out = sys.argv[1], sys.argv[2]
log = [json.loads(l) for l in open(f"{work}/page.log")]

def peak(path):
    r = subprocess.run(["ffmpeg", "-hide_banner", "-i", path, "-af", "volumedetect", "-f", "null", "-"],
                       capture_output=True, text=True).stderr
    m = re.search(r"max_volume: (-?[\d.]+)", r)
    return float(m.group(1)) if m else -999

# The page records one file per audio context; the piano is the loud one.
files = {0: f"{work}/audio.m4a"}
for f in glob.glob(f"{work}/audio-*.m4a"):
    files[int(f.rsplit("-", 1)[1].split(".")[0])] = f
ctx = max(files, key=lambda c: peak(files[c]))
audio_start = next(e["t"] for e in log if e.get("kind") == "recording" and e.get("context") == ctx) / 1000
video_start = float(open(f"{work}/rec.log").read().split("first frame at ")[1].split()[0])
skip = max(video_start - audio_start, 0)  # audio recorded before the first video frame
subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", f"{work}/video.mp4",
                "-ss", f"{skip:.3f}", "-i", files[ctx], "-map", "0:v", "-map", "1:a",
                "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p",
                "-af", "loudnorm=I=-16:TP=-1.5:LRA=11", "-c:a", "aac", "-b:a", "192k", "-shortest",
                f"{out}.mp4"], check=True)
subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", f"{out}.mp4", "-vn",
                "-c:a", "copy", f"{out}.m4a"], check=True)

scheduled = [float(l.split()[0]) for l in open(f"{work}/play.txt") if " down " in l and "tap" not in l]
arrived = [e["t"] for e in log if e.get("kind") == "keydown"]
if len(arrived) == len(scheduled) + 1:
    arrived = arrived[1:]  # the warm-up key
delays = [a - s for a, s in zip(arrived, scheduled)]
typical = statistics.median(delays)
err = sorted(abs(d - typical) for d in delays)
print(f"{len(arrived)} of {len(scheduled)} notes reached the page; vs the score: median "
      f"{err[len(err)//2]:.0f} ms, 95% {err[int(len(err)*.95)]:.0f} ms, worst {err[-1]:.0f} ms")
print("piano window activated:", sum(1 for e in log if e.get("kind") == "ACTIVATED"), "times")
print(f"wrote {out}.mp4 and {out}.m4a")
PY
echo "front-app changes:"; cat "$work/front.log"
