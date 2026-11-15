#!/usr/bin/env python3
"""Head-to-head: the same tasks on the same fixture window, through each
computer-use tool's own CLI, on this Mac.

Each round: read the window, click "Increment", type into "Name", read it
again. A step counts only when the fixture's own log shows it landed. The
user's front app is checked before and after every call, and the fixture
logs any real activation.

Usage: compare.py --log FIXTURE_LOG --rounds 10 --out results.json TOOL...
Tools: hush, agent-desktop, open-computer-use, cua-driver, peekaboo. Paths
to the binaries come from the environment (see TOOLS below).
"""

import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import time

APP = "HushFixture"
WINDOW = "Hush Fixture"
FRONT = os.environ.get("FRONTPID", "frontpid")


def run(argv, timeout=60):
    started = time.perf_counter()
    done = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    elapsed = (time.perf_counter() - started) * 1000
    return done.returncode, done.stdout + done.stderr, elapsed


def tokens(text):
    # The same rough rule for every tool: about 4 characters per token.
    return round(len(text) / 4)


def fixture_pid():
    found = subprocess.run(
        ["pgrep", "-f", "Contents/MacOS/HushFixture"], capture_output=True, text=True
    ).stdout.split()
    return found[0] if found else ""


def front_pid():
    return subprocess.run([FRONT], capture_output=True, text=True).stdout.strip()


class Tool:
    name = ""

    def observe(self):
        raise NotImplementedError

    def click(self, state):
        raise NotImplementedError

    def type(self, state, text):
        raise NotImplementedError


class Hush(Tool):
    name = "Hush"

    def __init__(self):
        self.bin = os.environ.get("HUSH", "hush")

    def observe(self):
        return run([self.bin, "observe", APP, "--window", WINDOW])

    def click(self, state):
        return run([self.bin, "act", APP, "--window", WINDOW, "click b1"])

    def type(self, state, text):
        return run([self.bin, "act", APP, "--window", WINDOW, f'type t1 "{text}"'])


class HushCold(Hush):
    """Every call its own process with nothing carried over, like the
    other CLIs: no daemon, so no refs or diffs from earlier calls."""

    name = "Hush (no daemon)"

    def __init__(self):
        super().__init__()
        os.environ["HUSH_NO_DAEMON"] = "1"

    def __del__(self):
        os.environ.pop("HUSH_NO_DAEMON", None)


class AgentDesktop(Tool):
    name = "agent-desktop"

    def __init__(self):
        self.bin = os.environ.get("AGENT_DESKTOP", "agent-desktop")
        _, out, _ = run([self.bin, "list-windows", "--app", APP])
        found = re.search(r'"id":"(w-\d+)"[^}]*"title":"' + WINDOW, out)
        self.window = found.group(1) if found else None

    def observe(self):
        return run([self.bin, "snapshot", "--app", APP, "--window-id", self.window, "-i"])

    def ref(self, state, name):
        found = re.search(r'"name":"' + name + r'","ref_id":"(@[^"]+)"', state)
        return found.group(1) if found else "@missing"

    def click(self, state):
        return run([self.bin, "click", self.ref(state, "Increment")])

    def type(self, state, text):
        return run([self.bin, "type", self.ref(state, "Name"), text])


class OpenComputerUse(Tool):
    """Its actions answer with the new app state, so it needs fewer reads."""

    name = "open-computer-use"

    def __init__(self):
        self.bin = os.environ.get("OCU", "open-computer-use")

    def observe(self):
        return run([self.bin, "snapshot", APP])

    def call(self, tool, args):
        return run([self.bin, "call", tool, "--args", json.dumps({"app": APP, **args})])

    def index(self, state, pattern):
        found = re.search(r"(\d+) " + pattern, state)
        return found.group(1) if found else "-1"

    def click(self, state):
        return self.call("click", {"element_index": self.index(state, "button Increment")})

    def type(self, state, text):
        # Its own advice for a text field: type_text needs a focused field,
        # set_value doesn't.
        return self.call(
            "set_value", {"element_index": self.index(state, "text field"), "value": text}
        )


class Mcp:
    """A long-running stdio MCP server, for tools whose CLI can't run
    without a separately granted app (cua-driver's daemon)."""

    def __init__(self, argv, env):
        self.process = subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, env={**os.environ, **env},
        )
        self.next_id = 0
        self.request("initialize", {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "hush-bench", "version": "1"},
        })
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def request(self, method, params):
        self.next_id += 1
        self.send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        while True:
            reply = json.loads(self.process.stdout.readline())
            if reply.get("id") == self.next_id:
                return reply

    def call(self, tool, args):
        """(exit code, text, ms, image tokens) like `run`."""
        started = time.perf_counter()
        reply = self.request("tools/call", {"name": tool, "arguments": args})
        elapsed = (time.perf_counter() - started) * 1000
        result = reply.get("result", {})
        text = "\n".join(c.get("text", "") for c in result.get("content", []) if c["type"] == "text")
        images = [c for c in result.get("content", []) if c["type"] == "image"]
        code = 1 if result.get("isError") or "error" in reply else 0
        return code, text or json.dumps(reply.get("error")), elapsed, len(images)


class CuaDriver(Tool):
    """Driven over MCP in `--direct` mode, so it uses this terminal's
    permissions; each read also returns a screenshot of the window."""

    name = "cua-driver"

    def __init__(self):
        binary = os.environ.get("CUA", "cua-driver")
        self.mcp = Mcp([binary, "mcp", "--direct"], {"CUA_DRIVER_RS_PERMISSIONS_GATE": "0"})
        self.pid = int(subprocess.run(
            ["pgrep", "-f", "Contents/MacOS/HushFixture"], capture_output=True, text=True
        ).stdout.split()[0])
        _, out, _, _ = self.mcp.call("list_windows", {"pid": self.pid})
        self.window = int(re.search(r'"' + WINDOW + r'" \[window_id: (\d+)\]', out).group(1))
        self.image_tokens = 0

    def observe(self):
        code, out, ms, images = self.mcp.call(
            "get_window_state", {"pid": self.pid, "window_id": self.window}
        )
        # Anthropic's image rule: width x height / 750 for the 480x452 shot.
        return code, out + " " * (4 * round(480 * 452 / 750)) * images, ms

    def token(self, state, pattern):
        snapshot = re.search(r"snapshot_id=(s[0-9a-f]+)", state)
        row = re.search(r"\[(\d+)\] " + pattern, state)
        return f"{snapshot.group(1)}:{row.group(1)}" if snapshot and row else "s00000000:0"

    def click(self, state):
        code, out, ms, _ = self.mcp.call("click", {"element_token": self.token(state, 'AXButton "Increment"')})
        return code, out, ms

    def type(self, state, text):
        code, out, ms, _ = self.mcp.call(
            "type_text", {"element_token": self.token(state, "AXTextField"), "text": text}
        )
        return code, out, ms


class Peekaboo(Tool):
    name = "Peekaboo"

    def __init__(self):
        self.bin = os.environ.get("PEEKABOO", "peekaboo")

    def observe(self):
        code, out, ms = run([self.bin, "see", "--app", APP, "--window-title", WINDOW, "--json-output"])
        try:
            data = json.loads(out)["data"]
            # What an agent is handed: the element map, without debug logs.
            out = json.dumps({k: data[k] for k in ("snapshot_id", "ui_elements") if k in data})
        except (ValueError, KeyError, TypeError):
            code = code or 1
        return code, out, ms

    def snapshot(self, state):
        found = re.search(r'"snapshot_id": "([^"]+)"', state)
        return found.group(1) if found else "latest"

    def element(self, state, title):
        for element in json.loads(state).get("ui_elements", []):
            if element.get("label") == title or element.get("title") == title or element.get("ax_role") == title:
                return element["id"]
        return "missing"

    def click(self, state):
        return run([self.bin, "click", "--on", self.element(state, "Increment"),
                    "--snapshot", self.snapshot(state), "--app", APP])

    def type(self, state, text):
        # It refuses a second action on a snapshot that already drove a
        # change, so it needs a fresh read first; that read is counted here.
        _, state, read = self.observe()
        snapshot = self.snapshot(state)
        code, out, first = run([self.bin, "click", "--on", self.element(state, "AXTextField"),
                                "--snapshot", snapshot, "--app", APP])
        if code != 0:
            return code, out, read + first
        code, out, second = run([self.bin, "type", text, "--snapshot", snapshot, "--app", APP])
        return code, out, read + first + second


TOOLS = {
    "hush": Hush,
    "hush-cold": HushCold,
    "agent-desktop": AgentDesktop,
    "open-computer-use": OpenComputerUse,
    "cua-driver": CuaDriver,
    "peekaboo": Peekaboo,
}


def log_lines(path):
    with open(path, errors="replace") as f:
        return f.read().splitlines()


def bench(tool, rounds, log_path):
    times = {"observe": [], "click": [], "type": [], "observe again": []}
    sizes = {"observe": [], "observe again": []}
    first_read = None
    failures, disturbed = [], 0
    for i in range(rounds):
        before = len(log_lines(log_path))
        text = f"r{i}"
        code, state, ms = tool.observe()
        times["observe"].append(ms)
        sizes["observe"].append(tokens(state))
        if first_read is None:
            first_read = {"ms": round(ms, 1), "tokens": tokens(state)}
        if code != 0:
            failures.append(f"observe: {state[:200]}")
            failures.append(f"round {i}: couldn't read the window")
            continue
        for step, action in (("click", lambda: tool.click(state)), ("type", lambda: tool.type(state, text))):
            code, out, ms = action()
            times[step].append(ms)
            if code != 0:
                failures.append(f"{step}: {out[:200]}")
            # The user may switch apps during a long run; what counts is the
            # target taking the front.
            if front_pid() == fixture_pid():
                disturbed += 1
        code, state, ms = tool.observe()
        times["observe again"].append(ms)
        sizes["observe again"].append(tokens(state))
        time.sleep(0.2)
        new = log_lines(log_path)[before:]
        if not any(line.startswith("increment") for line in new):
            failures.append(f"round {i}: the click didn't land")
        if not any(line.startswith("field") and text in line for line in new):
            failures.append(f"round {i}: the typing didn't land")
        if any("ACTIVATED" in line for line in new):
            disturbed += 1
    landed = rounds - len({f.split(":")[0] for f in failures if f.startswith("round")})
    median = lambda v: round(statistics.median(v), 1) if v else None
    return {
        "tool": tool.name,
        "rounds": rounds,
        "rounds fully landed": sum(
            1
            for i in range(rounds)
            if not any(f.startswith(f"round {i}:") for f in failures)
        ),
        "median ms": {k: median(v) for k, v in times.items()},
        "median tokens": {k: median(v) for k, v in sizes.items()},
        "first read": first_read,
        "round trip ms (median sum)": round(sum(median(v) or 0 for v in times.values()), 1),
        "disturbances": disturbed,
        "failures": failures[:6],
        "_landed_hint": landed,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", required=True)
    parser.add_argument("--rounds", type=int, default=10)
    parser.add_argument("--out")
    parser.add_argument("tools", nargs="+", choices=sorted(TOOLS))
    args = parser.parse_args()
    results = []
    for name in args.tools:
        try:
            tool = TOOLS[name]()
            result = bench(tool, args.rounds, args.log)
        except Exception as error:  # a tool that can't start is a result too
            result = {"tool": name, "error": str(error)}
        result.pop("_landed_hint", None)
        print(json.dumps(result, indent=1), flush=True)
        results.append(result)
    if args.out:
        with open(args.out, "w") as f:
            json.dump(results, f, indent=1)


if __name__ == "__main__":
    sys.exit(main())
