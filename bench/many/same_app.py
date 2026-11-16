#!/usr/bin/env python3
"""K sessions type into the same field of the fixture app at once. The field must end up as K
whole blocks: two agents acting on one app take turns, so their keys never interleave.

Needs bench/fixture running (open -g -n HushFixture.app). Usage: same_app.py HUSH K
"""
import json, re, subprocess, sys, threading, time

hush, k = sys.argv[1], int(sys.argv[2])

def open_session(name):
    p = subprocess.Popen([hush, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": name, "version": "1"}}}) + "\n"); p.stdin.flush(); p.stdout.readline()
    return p

def call(p, name, args, i=[1]):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": "tools/call", "params": {"name": name, "arguments": args}}) + "\n"); p.stdin.flush()
    r = json.loads(p.stdout.readline())["result"]
    return r["content"][0].get("text", "")

setup = open_session("setup")
print(call(setup, "act", {"app": "HushFixture", "steps": ["lane"]}).splitlines()[0])
outline = call(setup, "observe", {"app": "HushFixture", "full": True})
field = re.search(r"^\s*(t\d+) field", outline, re.M).group(1)
call(setup, "act", {"app": "HushFixture", "steps": [f"set {field} \"\""]})
print("field", field)

letters = "abcdefghij"[:k]
sessions = [open_session(f"typist{j}") for j in range(k)]
went = threading.Barrier(k)
def typist(j):
    went.wait()
    call(sessions[j], "act", {"app": "HushFixture", "steps": [f"type {field} {letters[j] * 12}"]})
threads = [threading.Thread(target=typist, args=(j,)) for j in range(k)]
t0 = time.perf_counter()
for t in threads: t.start()
for t in threads: t.join()
print(f"{k} sessions typed in {time.perf_counter() - t0:.2f} s")
value = re.search(rf"{field} field[^=]*= \"([^\"]*)\"", call(setup, "observe", {"app": "HushFixture", "full": True})).group(1)
blocks = re.findall(r"(.)\1*", value)
runs = [m.group(0) for m in re.finditer(r"(.)\1*", value)]
print("field now:", value)
print("whole blocks:", all(len(r) == 12 for r in runs) and len(runs) == k)
for p in sessions + [setup]:
    p.stdin.close(); p.wait()
