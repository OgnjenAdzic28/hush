#!/usr/bin/env python3
"""Compiles a MIDI file into a Hush timeline for virtualpiano.net's keyboard.

virtualpiano.net: white keys C2..C7 are "1234567890qwertyuiopasdfghjklzxcvbnm"; a black key is
Shift plus the white key just below it. The sustain pedal (CC64) is honored by holding notes until
it lifts, as a pianist's pedal would. Notes outside C2..C7 move by octaves into range.
The page shows each note's name as it plays: `t` is middle C (C4), so `1` is C2.

Usage: compile.py IN.mid OUT.txt [--from-ms N] [--to-ms N] [--track NAME] [--list]
--track keeps only tracks whose name contains NAME (several, comma-separated) (tempo still applies); drums (channel 10) are
always left out. --list prints each track's name and note count.
Writes one event per line ("MS down KEY" / "MS up KEY") and prints a summary.
"""

import struct
import sys

WHITE = "1234567890qwertyuiopasdfghjklzxcvbnm"
LOW, HIGH = 36, 96  # C2, C7
WHITE_STEPS = {0: 0, 2: 1, 4: 2, 5: 3, 7: 4, 9: 5, 11: 6}


def key_for(note: int) -> str:
    while note < LOW:
        note += 12
    while note > HIGH:
        note -= 12
    octave, step = divmod(note - LOW, 12)
    if step in WHITE_STEPS:
        return WHITE[octave * 7 + WHITE_STEPS[step]]
    return "shift+" + WHITE[octave * 7 + WHITE_STEPS[step - 1]]


def read_midi(path):
    data = open(path, "rb").read()
    assert data[:4] == b"MThd"
    _, _, ntracks, division = struct.unpack(">IHHH", data[4:14])
    pos = 14
    events = []  # (tick, kind, a, b, track)
    names = []
    for track_index in range(ntracks):
        names.append("")
        assert data[pos:pos + 4] == b"MTrk"
        (length,) = struct.unpack(">I", data[pos + 4:pos + 8])
        track = data[pos + 8:pos + 8 + length]
        pos += 8 + length
        i, tick, status = 0, 0, 0
        while i < len(track):
            delta = 0
            while True:
                byte = track[i]
                i += 1
                delta = (delta << 7) | (byte & 0x7F)
                if byte < 0x80:
                    break
            tick += delta
            if track[i] >= 0x80:
                status = track[i]
                i += 1
            if status == 0xFF:
                kind, length = track[i], track[i + 1]
                # Lengths here are short; a full varlen reader isn't needed for this file.
                payload = track[i + 2:i + 2 + length]
                i += 2 + length
                if kind == 0x51:
                    events.append((tick, "tempo", int.from_bytes(payload, "big"), 0, track_index))
                elif kind == 0x03:
                    names[track_index] = payload.decode("latin-1").strip()
            elif status in (0xF0, 0xF7):
                length = track[i]
                i += 1 + length
            else:
                high = status & 0xF0
                a = track[i]
                b = track[i + 1] if high not in (0xC0, 0xD0) else 0
                i += 1 if high in (0xC0, 0xD0) else 2
                if (status & 0x0F) == 9:
                    continue  # drums
                if high == 0x90 and b > 0:
                    events.append((tick, "on", a, b, track_index))
                elif high == 0x80 or (high == 0x90 and b == 0):
                    events.append((tick, "off", a, 0, track_index))
                elif high == 0xB0 and a == 64:
                    events.append((tick, "pedal", b, 0, track_index))
    order = {"tempo": 0, "off": 1, "pedal": 2, "on": 3}
    events.sort(key=lambda e: (e[0], order[e[1]]))
    return division, events, names


def to_ms(division, events):
    tempo, last_tick, ms = 500000, 0, 0.0
    out = []
    for tick, kind, a, b, _ in events:
        ms += (tick - last_tick) * tempo / division / 1000
        last_tick = tick
        if kind == "tempo":
            tempo = a
        else:
            out.append((ms, kind, a))
    return out


def compile_timeline(timed, start=0.0, end=float("inf")):
    lines, held, sustained, pedal = [], {}, set(), False

    def release(key, at):
        lines.append((at, f"{at - start:.1f} up {key}"))

    for ms, kind, a in timed:
        if ms > end:
            break
        if kind == "pedal":
            pedal = a >= 64
            if not pedal:
                for key in sorted(sustained):
                    if key not in held:
                        release(key, ms)
                sustained.clear()
            continue
        key = key_for(a)
        if kind == "on":
            if ms < start:
                continue
            if key in held or key in sustained:
                # Strike again: lift first, then press, at the same instant.
                release(key, ms)
                sustained.discard(key)
            held[key] = held.get(key, 0) + 1
            lines.append((ms, f"{ms - start:.1f} down {key}"))
        elif kind == "off" and key in held:
            held[key] -= 1
            if held[key] == 0:
                del held[key]
                if pedal:
                    sustained.add(key)
                else:
                    release(key, ms)
    stop = max((t for t, _ in lines), default=start) + 600
    for key in sorted(set(held) | sustained):
        release(key, stop)
    lines.sort(key=lambda line: line[0])  # stable: same-instant up stays before down
    return [text for _, text in lines]


if __name__ == "__main__":
    args = sys.argv[1:]
    start = float(args[args.index("--from-ms") + 1]) if "--from-ms" in args else 0.0
    end = float(args[args.index("--to-ms") + 1]) if "--to-ms" in args else float("inf")
    division, events, names = read_midi(args[0])
    if "--list" in args:
        for i, name in enumerate(names):
            notes = sum(1 for e in events if e[4] == i and e[1] == "on")
            print(f"track {i}: {name!r}, {notes} notes")
        sys.exit(0)
    if "--track" in args:
        wanted = [w.strip() for w in args[args.index("--track") + 1].lower().split(",")]
        keep = {i for i, name in enumerate(names) if any(w in name.lower() for w in wanted)}
        events = [e for e in events if e[1] == "tempo" or e[4] in keep]
    timed = to_ms(division, events)
    lines = compile_timeline(timed, start, end)
    open(args[1], "w").write("\n".join(lines) + "\n")
    notes = sum(" down " in line for line in lines)
    length = float(lines[-1].split()[0]) / 1000 if lines else 0
    print(f"{notes} notes, {len(lines)} events, {length:.1f} s")
