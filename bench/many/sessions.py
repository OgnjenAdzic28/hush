#!/usr/bin/env python3
"""N agents' sessions at once through `hush mcp` pipes, each repeating observe, see, the menu
catalog and an AppleScript; times every call and samples the host's CPU, memory and threads.

Needs FilmCraft running (any app that draws its own interface works; change APP) and Finder.
Usage: sessions.py HUSH N ROUNDS
"""
import json, statistics, subprocess, sys, threading, time

hush, n, rounds = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])

def call(proc, i, name, args):
    msg = {"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": name, "arguments": args}}
    t = time.perf_counter()
    proc.stdin.write(json.dumps(msg) + "\n"); proc.stdin.flush()
    reply = json.loads(proc.stdout.readline())
    ms = (time.perf_counter() - t) * 1000
    c = reply["result"]["content"][0]
    ok = not reply["result"].get("isError") and (c["type"] == "image" or "permission" not in c.get("text", ""))
    return ms, ok, (c.get("text") or "<image>")[:90]

results, errors = [], []
def session(k):
    p = subprocess.Popen([hush, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": f"load{k}", "version": "1"}}}) + "\n")
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"); p.stdin.flush()
    p.stdout.readline()
    i = 1
    plan = [("observe", {"app": "FilmCraft", "full": True}), ("see", {"app": "FilmCraft", "width": 800}),
            ("observe", {"app": "FilmCraft", "menus": True}),
            ("script", {"app": "Finder", "source": 'tell application "Finder" to get name of startup disk'}),
            ("observe", {"app": "FilmCraft"})]
    for _ in range(rounds):
        for name, args in plan:
            ms, ok, text = call(p, i, name, args); i += 1
            results.append((name, ms))
            if not ok: errors.append((k, name, text))
    p.stdin.close(); p.wait()

samples = []
stop = False
def sample():
    while not stop:
        out = subprocess.run(["ps", "-axo", "pid,%cpu,rss,command"], capture_output=True, text=True).stdout
        for line in out.splitlines():
            if line.strip().endswith(f"{hush} daemon"):
                pid, cpu, rss = line.split()[:3]
                th = subprocess.run(["ps", "-M", "-p", pid], capture_output=True, text=True).stdout.count("\n") - 1
                samples.append((float(cpu), int(rss) / 1024, th))
        time.sleep(0.25)

# Lane FilmCraft once in the host (it stays there for every session).
setup = subprocess.Popen([hush, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
setup.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "setup", "version": "1"}}}) + "\n"); setup.stdin.flush(); setup.stdout.readline()
print("lane:", call(setup, 1, "act", {"app": "FilmCraft", "steps": ["lane"]})[2].replace("\n", " / "))

threading.Thread(target=sample, daemon=True).start()
t0 = time.perf_counter()
threads = [threading.Thread(target=session, args=(k,)) for k in range(n)]
for t in threads: t.start()
for t in threads: t.join()
wall = time.perf_counter() - t0
stop = True
setup.stdin.close(); setup.wait()

print(f"{n} sessions x {rounds} rounds: {len(results)} calls in {wall:.1f} s, {len(errors)} errors")
for name in dict.fromkeys(r[0] for r in results):
    ms = sorted(m for r, m in results if r == name)
    print(f"  {name:8} median {statistics.median(ms):6.0f} ms   p95 {ms[int(len(ms)*.95)-1]:6.0f} ms   max {ms[-1]:6.0f} ms")
if samples:
    print(f"host: cpu peak {max(s[0] for s in samples):.0f}% · rss peak {max(s[1] for s in samples):.0f} MB · threads peak {max(s[2] for s in samples)}")
for e in errors[:5]: print("  error:", e)
