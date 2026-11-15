#!/usr/bin/env python3
"""Compares note onsets in a recording with the schedule they were played from.

Onsets are found from rises in short-time energy (spectral flux on 5 ms hops), then each scheduled
note start (unique start times only: a chord is one onset) is matched to the nearest detected
onset after removing the constant offset between the two clocks. Reports the timing error.

Usage: onsets.py RECORDING SCHEDULE.txt
"""

import statistics
import subprocess
import sys
from array import array
import math

RATE = 22050
HOP = 110  # 5 ms
WINDOW = 512


def pcm(path):
    raw = subprocess.run(
        ["ffmpeg", "-loglevel", "error", "-i", path, "-ac", "1", "-ar", str(RATE), "-f", "s16le", "-"],
        capture_output=True, check=True,
    ).stdout
    samples = array("h")
    samples.frombytes(raw)
    return samples


def flux(samples):
    """Rise in band energy per hop, a cheap onset strength (low/high split, positive changes)."""
    out, prev = [], (0.0, 0.0)
    for start in range(0, len(samples) - WINDOW, HOP):
        frame = samples[start:start + WINDOW]
        diff = sum(abs(frame[i] - frame[i - 1]) for i in range(1, WINDOW, 2))
        level = sum(abs(x) for x in frame[::2])
        cur = (math.log1p(level), math.log1p(diff))
        out.append(max(0.0, cur[0] - prev[0]) + max(0.0, cur[1] - prev[1]))
        prev = cur
    return out


def peaks(strength):
    """Local maxima above an adaptive threshold, at least 40 ms apart, in ms."""
    found, last = [], -10**9
    n = len(strength)
    for i in range(1, n - 1):
        lo, hi = max(0, i - 60), min(n, i + 60)
        local = sorted(strength[lo:hi])
        threshold = local[len(local) // 2] + 0.15
        if strength[i] > threshold and strength[i] >= strength[i - 1] and strength[i] >= strength[i + 1]:
            ms = i * HOP * 1000 / RATE
            if ms - last >= 40:
                found.append(ms)
                last = ms
    return found


def main():
    recording, schedule = sys.argv[1], sys.argv[2]
    starts = sorted({float(l.split()[0]) for l in open(schedule) if " down " in l})
    detected = peaks(flux(pcm(recording)))
    # The two clocks differ by a constant: take the offset that best lines up the first notes.
    best = None
    for d in detected[:20]:
        offset = d - starts[0]
        errors = []
        j = 0
        for s in starts[:60]:
            target = s + offset
            while j + 1 < len(detected) and abs(detected[j + 1] - target) <= abs(detected[j] - target):
                j += 1
            errors.append(abs(detected[j] - target))
        score = statistics.median(errors)
        if best is None or score < best[0]:
            best = (score, offset)
    offset = best[1]
    errors, j = [], 0
    for s in starts:
        target = s + offset
        while j + 1 < len(detected) and abs(detected[j + 1] - target) <= abs(detected[j] - target):
            j += 1
        errors.append(abs(detected[j] - target))
    errors.sort()
    found = sum(e <= 50 for e in errors)
    print(f"{len(starts)} note starts in the score, {len(detected)} onsets heard")
    print(f"{found} of {len(starts)} starts heard within 50 ms")
    print(
        f"onset error: median {errors[len(errors) // 2]:.1f} ms, 90% {errors[int(len(errors) * 0.9)]:.1f} ms"
    )


if __name__ == "__main__":
    main()
