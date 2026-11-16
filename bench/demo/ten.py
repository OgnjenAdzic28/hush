#!/usr/bin/env python3
"""Ten agents at once, one Hush: ten task boards, each worked by its own session, each recorded
with its own cursor, tiled into one 4K video.

Usage: ten.py HUSH SHELL_APP_DIR OUT.mp4
  HUSH           the hush binary
  SHELL_APP_DIR  a folder for the ten demo apps (built from DemoShell)
Needs Accessibility and Screen Recording for the calling app, swiftc and ffmpeg. Everything runs in
Hush's lane; nothing shows on screen.
"""
import json, os, re, subprocess, sys, threading, time, urllib.parse

hush, apps_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
here = os.path.dirname(os.path.abspath(__file__))
work = os.path.join(apps_dir, "work")
os.makedirs(work, exist_ok=True)

BOARDS = [
    ("Roadmap", "#7c5cff", "Ship onboarding", "Write launch post|Record demo"),
    ("Design", "#ec4899", "Refresh icons", "Color tokens|Empty states"),
    ("Infra", "#06b6d4", "Rotate keys", "Upgrade runners|Cache builds"),
    ("Mobile", "#10b981", "Fix deep links", "Dark mode|Push opt-in"),
    ("Docs", "#f59e0b", "API reference", "Quickstart|Changelog"),
    ("Growth", "#3b82f6", "Pricing test", "Referral email|Landing copy"),
    ("Support", "#ef4444", "Triage inbox", "Refund macro|FAQ update"),
    ("Research", "#a855f7", "User interviews", "Survey draft|Persona map"),
    ("Billing", "#14b8a6", "Tax rules", "Invoice PDF|Dunning flow"),
    ("Hiring", "#f97316", "Screen resumes", "Job post|Panel prep"),
]
AGENTS = ["Claude", "Atlas", "Nova", "Echo", "Juno", "Orion", "Pixel", "Sage", "Vega", "Kai"]
SIZE = (720, 405)


def make_app(name):
    app = os.path.join(apps_dir, f"{name}.app")
    os.makedirs(os.path.join(app, "Contents/MacOS"), exist_ok=True)
    with open(os.path.join(app, "Contents/Info.plist"), "w") as f:
        f.write(f"""<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.hush.demo.{name.lower()}</string>
<key>CFBundleName</key><string>{name}</string>
<key>CFBundleExecutable</key><string>DemoShell</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>""")
    binary = os.path.join(app, "Contents/MacOS/DemoShell")
    if not os.path.exists(binary):
        subprocess.run(["cp", os.path.join(apps_dir, "DemoShell"), binary], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", app], capture_output=True)
    return app


class Session:
    def __init__(self, name):
        self.p = subprocess.Popen([hush, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.i = 0
        self.send({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": name, "version": "1"}}})

    def send(self, message):
        self.p.stdin.write(json.dumps(message) + "\n")
        self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())

    def call(self, tool, **args):
        self.i += 1
        reply = self.send({"jsonrpc": "2.0", "id": self.i, "method": "tools/call",
                           "params": {"name": tool, "arguments": args}})
        return reply["result"]["content"][0].get("text", "")

    def close(self):
        self.p.stdin.close()
        self.p.wait()


def ref(outline, pattern):
    """The ref on the outline line matching `pattern`."""
    m = re.search(r"^\s*([a-z]+\d+) " + pattern, outline, re.M)
    if not m:
        raise RuntimeError(f"nothing matches {pattern!r}")
    return m.group(1)


def agent(app, label, new_tasks, results, index):
    s = Session(label)
    try:
        s.call("act", app=app, steps=["lane"])
        # The page may still be loading.
        for _ in range(50):
            if re.search(r"^\s*t\d+ field", s.call("observe", app=app, full=True), re.M):
                break
            time.sleep(0.1)
        s.call("record", app=app)
        time.sleep(0.9)
        look = lambda: s.call("observe", app=app, full=True)
        pause = "wait 350ms"
        o = look()
        s.call("act", app=app, steps=[f"type {ref(o, '(?:field|search)')} {new_tasks[0]}", f"click {ref(o, 'button \"Add\"')}", pause])
        o = look()
        first = ref(o, r'group "(?!New task|To do|Doing|Done)[^"]+"')
        s.call("act", app=app, steps=[f"drag {first} to {ref(o, 'group \"Doing\"')}", pause])
        o = look()
        doing = o.split('group "Doing"')[1].split('group "Done"')[0]
        card = ref(doing, r'group "[^"]+"')
        check = ref(doing, r'button "Complete')
        s.call("act", app=app, steps=[f"click {check}", pause, f"drag {card} to {ref(o, 'group \"Done\"')}", pause])
        o = look()
        s.call("act", app=app, steps=[f"type {ref(o, '(?:field|search)')} {new_tasks[1]}", f"click {ref(o, 'button \"Add\"')}", "wait 900ms"])
        results[index] = s.call("record", app=app, stop=True, label=label,
                                out=os.path.join(work, f"{index:02d}-{app}.mp4"))
    except Exception as e:
        results[index] = f"error: {e}"
    finally:
        s.close()


NEW = [("Fix login bug", "Write tests"), ("Logo pass", "Motion spec"), ("Alert routing", "Backups"),
       ("Crash report", "Release 2.1"), ("Code samples", "SDK guide"), ("A/B results", "Onboarding"),
       ("Bulk replies", "SLA report"), ("Diary study", "Synthesis"), ("Usage meter", "Receipts"),
       ("Offer letter", "Onsite plan")]

# Launch the ten boards in the background and give a lane tool time to hide them.
for name, accent, doing, todo in BOARDS:
    query = urllib.parse.urlencode({"title": name, "accent": accent, "todo": todo, "doing": doing},
                                   quote_via=urllib.parse.quote_plus)
    url = f"file://{os.path.join(here, 'board.html')}?{query}"
    subprocess.run(["/usr/bin/open", "-g", "-n", make_app(name), "--args", url, name, str(SIZE[0]), str(SIZE[1])], check=True)
time.sleep(4)

results = [None] * len(BOARDS)
threads = [threading.Thread(target=agent, args=(b[0], AGENTS[i], NEW[i], results, i)) for i, b in enumerate(BOARDS)]
started = time.time()
for t in threads: t.start()
for t in threads: t.join()
print(f"10 agents finished in {time.time() - started:.1f} s")
for name, r in zip([b[0] for b in BOARDS], results):
    print(f"  {name}: {r}")

videos = sorted(os.path.join(work, f) for f in os.listdir(work) if f.endswith(".mp4"))


def compose(videos, out):
    """The ten recordings in a 5 by 2 grid on a titled 4K backdrop."""
    backdrop = os.path.join(work, "backdrop.png")
    tool = os.path.join(work, "backdrop")
    subprocess.run(["swiftc", "-O", "-o", tool, os.path.join(here, "Backdrop.swift")], check=True)
    subprocess.run([tool, backdrop, "3840", "2160", "Ten agents. One Hush.",
                    "Each in its own window, all in the lane. Nothing on your screen. Real time.", "420"], check=True)
    inputs = sum((["-i", v] for v in videos), []) + ["-loop", "1", "-i", backdrop]
    tiles = "".join(f"[{i}:v]scale=720:-2,setpts=PTS-STARTPTS[v{i}];" for i in range(10))
    layout = "|".join(f"{(i % 5) * 744}_{(i // 5) * 460}" for i in range(10))
    graph = (tiles + "".join(f"[v{i}]" for i in range(10)) +
             f"xstack=inputs=10:layout={layout}:fill=0x0b0b0f[grid];"
             "[10:v][grid]overlay=(W-w)/2:(H-h)/2+120:shortest=1[out]")
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *inputs, "-filter_complex", graph,
                    "-map", "[out]", "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p",
                    "-r", "30", out], check=True)
    print("wrote", out)


if len(videos) == 10:
    compose(videos, out)
for name, *_ in BOARDS:
    subprocess.run(["/usr/bin/pkill", "-f", f"{name}.app/Contents/MacOS/DemoShell"])
