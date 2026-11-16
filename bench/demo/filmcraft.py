#!/usr/bin/env python3
"""The FilmCraft edit at Hush's own speed: cut at 10 s, a "Made with Hush" title, a 1080p export,
as fixed steps with no model in between, recorded with the agent cursor.

Usage: filmcraft.py HUSH OUT_DIR [EXPORT_DIR]
EXPORT_DIR (default /tmp/hush-demo) is where FilmCraft writes its export; it shows on screen, so it
should hold nothing personal (a home folder shows the user name).
FilmCraft must be running (muted: its demo project has a soundtrack). Coordinates are for its
default layout in a 1640x990 window. Menu commands go through Accessibility, so the cursor moves
only for clicks and typing.
"""
import json, os, re, subprocess, sys, time

hush, out_dir = sys.argv[1], sys.argv[2]
export_dir = sys.argv[3] if len(sys.argv) > 3 else "/tmp/hush-demo"
os.makedirs(out_dir, exist_ok=True)
os.makedirs(export_dir, exist_ok=True)
export = os.path.join(export_dir, "hush-demo.mp4")
if os.path.exists(export):
    os.remove(export)

p = subprocess.Popen([hush, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
n = [0]


def call(tool, **args):
    n[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n[0], "method": "tools/call",
                              "params": {"name": tool, "arguments": args}}) + "\n")
    p.stdin.flush()
    return json.loads(p.stdout.readline())["result"]["content"][0].get("text", "")


p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
    "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "demo", "version": "1"}}}) + "\n")
p.stdin.flush()
p.stdout.readline()

app = "FilmCraft"
call("act", app=app, steps=["lane", "menu File > New > Demo Project", "wait 1500ms", "click 216,18", "wait 600ms"])
call("observe", app=app)
print(call("record", app=app))
time.sleep(1.0)
started = time.time()
call("act", app=app, steps=["key home", "wait 150ms"])
# One frame per key: FilmCraft takes one key per screen refresh, so 48 steps of 5 frames, 20 ms apart.
call("timeline", app=app, events=[f"{i * 20} tap shift+right" for i in range(48)])
call("act", app=app, steps=["wait 250ms", "menu Sequence > Add Edit", "wait 300ms",
                            "menu Graphics and Titles > New Layer > Text", "wait 500ms",
                            "click 1300,541", "key end", "key shift+home", "type Made with Hush", "key tab",
                            "wait 600ms", "menu File > Export > Export", "wait 1200ms"])
# FilmCraft draws its own text fields: setting them through Accessibility doesn't reach its model,
# so they are clicked by ref and typed into (select the line, then type over it). The export page's
# first two fields are the file name and the folder.
page = call("observe", app=app, full=True)
fields = re.findall(r"^\s*(t\d+) field", page, re.M)
# FilmCraft grows its window a little each time the export page opens, so the Export button (drawn,
# not in the outline) is placed from the window's current bottom-right corner.
call("act", app=app, steps=[f"click {fields[0]}", "key end", "key shift+home", "type hush-demo", "key tab",
                            f"click {fields[1]}", "key end", "key shift+home", f"type {export_dir}", "key tab",
                            # A click on the page's header ends the field edit; a first click on Export
                            # would only do that.
                            "click 340,56", "wait 400ms"])
w, h = map(int, re.search(r'window "FilmCraft" (\d+)x(\d+)', call("observe", app=app, full=True)).groups())
call("act", app=app, steps=[f"click {w - 68},{h - 64}"])
edited = time.time() - started
# Done when the movie is complete (FilmCraft writes it in bursts, so a quiet file isn't enough).
def complete():
    probe = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", export],
                           capture_output=True, text=True)
    return os.path.exists(export) and probe.returncode == 0 and probe.stdout.strip() != ""
while not complete() and time.time() - started < 120:
    time.sleep(0.25)
call("act", app=app, steps=["wait 1200ms"])
print(call("record", app=app, stop=True, label="Claude", out=os.path.join(out_dir, "filmcraft-demo.mp4")))
print(f"edit steps took {edited:.1f} s; export finished {time.time() - started:.1f} s after the first step")
p.stdin.close()
p.wait()
