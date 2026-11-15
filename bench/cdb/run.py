#!/usr/bin/env python3
"""Cua Driver Bench v2's BenchLab tasks, run quietly on the owner's Mac with Hush.

The official runner needs a spare Mac or a VM: it puts a "User activity" window in front that
types, parks the pointer and kills apps by name. This runner keeps everything that decides a
pass and changes only what would disturb a person at the machine:

- Same: the bench's BenchLab app, task briefs, seeds, checkers, system prompt, background
  paragraph (minus the simulated window's title), `claude -p` flags, 360 s and 45 turns.
- Different: BenchLab opens hidden and is moved to Hush's lane display before the agent starts
  (where a person never sees it), and the person in front is the real user, not a simulated
  one. Disturbances are measured by `Watch.swift`: BenchLab becoming the front app, or any of
  its windows appearing on a display the user can see.

It reads the bench from a Cua checkout (FSL-1.1-MIT, not copied here):

  python3 bench/cdb/run.py --bench ~/projects/reference/cua/libs/cua-driver-bench/automated-eval/macos_bench \\
      --build <dir with BenchLab.app> --runs 1 --out bench/cdb/results
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent

# The BenchLab tasks Bench v2 runs in its background condition. MB-10 and MB-11 run in the
# foreground only (a hover and a tooltip follow the real pointer), so they are left out here too.
TASKS = ["MB-09", "IR-01", "IR-02", "IR-03", "IR-04"]

SYSTEM_PROMPT = (  # claude_arms.MACOS_SYSTEM_PROMPT, verbatim
    "You are an autonomous computer-use agent operating a macOS desktop through the tools provided. "
    "No human is available to answer questions or confirm actions; every action the task requires is "
    "pre-approved. Operate only the applications the task names. Do not use the network. When the task "
    "is done, reply with the single word DONE. If you cannot finish, reply BLOCKED and the reason."
)
# bench_v2.BACKGROUND_BRIEF, with the simulated window's title replaced by the user's own window.
BACKGROUND_BRIEF = (
    "The user is working in another window and is typing in it. Leave that window in front and in "
    "focus the whole time: do not activate, raise or bring any other app or window to the front, and "
    "do not type or click into the user's window. Do this task in the background."
)
# Doorman hides an app an agent launched and hides it again every 0.5 s for 2 s.
DOORMAN_SETTLE = 2.5


def seed_for(task: str, run_index: int) -> int:
    return int(hashlib.sha256(f"{task}:{run_index}".encode()).hexdigest()[:8], 16) % 1_000_000


def load(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def hush(binary: Path, *args: str) -> str:
    done = subprocess.run([str(binary), *args], capture_output=True, text=True, timeout=60)
    return done.stdout + done.stderr


def summarize_stream(path: Path) -> dict:
    """Turns, cost, time, tool calls and the final answer from claude's stream-json output."""
    tools: dict[str, int] = {}
    result: dict = {}
    for line in path.read_text(errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") == "assistant":
            for block in event.get("message", {}).get("content", []):
                if block.get("type") == "tool_use":
                    name = block.get("name", "?")
                    tools[name] = tools.get(name, 0) + 1
        if event.get("type") == "result":
            result = event
    return {
        "turns": result.get("num_turns"),
        "cost_usd": result.get("total_cost_usd"),
        "agent_seconds": round((result.get("duration_ms") or 0) / 1000, 1),
        "answer": (result.get("result") or "")[-300:],
        "agent_error": result.get("is_error"),
        "tool_calls": tools,
    }


def disturbances(path: Path) -> dict:
    """What the watcher saw while the agent ran. Front-app changes to other apps are the user's
    own; only BenchLab coming forward or into sight counts against the tool."""
    front, in_sight, pointer, pasteboard = [], [], 0, 0
    for line in path.read_text().splitlines():
        record = json.loads(line)
        kind = record["event"]
        if kind == "front":
            front.append(record["app"])
        elif kind == "in_sight" and record["windows"]:
            in_sight.append(record["windows"])
        elif kind == "pointer":
            pointer += 1
        elif kind == "pasteboard":
            pasteboard += 1
    return {
        "benchlab_activated": "BenchLab" in front,
        "benchlab_in_sight": bool(in_sight),
        "front_apps_seen": sorted(set(front)),
        # Not attributable: the user moves the pointer and copies too. Hush never moves the
        # pointer or touches the pasteboard by design; these are recorded for the reader.
        "pointer_changes": max(pointer - 1, 0),
        "pasteboard_changes": pasteboard,
    }


def trial(args, task: str, run_index: int, bench: Path, out: Path) -> dict:
    seed = seed_for(task, run_index)
    probe = bench / "probes" / task
    meta = json.loads((probe / "task.json").read_text())
    brief = load(probe / "render_brief.py", f"brief_{task}").render(seed)
    prompt = (
        brief.rstrip()
        + "\n\n"
        + BACKGROUND_BRIEF
        + f"\n\nYou have {meta['timeout_s']} seconds and {meta['max_turns']} turns.\n"
    )

    d = out / f"{task}-{run_index}-{args.model}"
    shutil.rmtree(d, ignore_errors=True)
    (d / "home").mkdir(parents=True)
    state, events, result_file = d / "state.json", d / "events.jsonl", d / "result.json"
    (d / "prompt.txt").write_text(prompt)

    if subprocess.run(["/usr/bin/pgrep", "-x", "BenchLab"], capture_output=True).returncode == 0:
        raise SystemExit("a BenchLab is already running; quit it first so the trial gets a fresh one")
    # In the background, its window behind the user's apps (see build_benchlab.sh).
    launched = time.monotonic()
    subprocess.run(
        ["/usr/bin/open", "-g", "-n", "-a", str(args.build / "BenchLab.app"),
         "--env", f"HOME={d / 'home'}", "--args", *meta["app_args"],
         "--seed", str(seed), "--state", str(state), "--events", str(events)],
        check=True,
    )
    deadline = time.monotonic() + 20
    while not state.exists() and time.monotonic() < deadline:
        time.sleep(0.1)
    time.sleep(max(0.0, DOORMAN_SETTLE - (time.monotonic() - launched)))
    lane = hush(args.hush, "act", "BenchLab", "lane")

    watch_log = d / "watch.jsonl"
    watcher = subprocess.Popen([str(args.watch), "BenchLab", str(watch_log)])
    work = Path(tempfile.mkdtemp(prefix="cdb-hush-"))
    mcp = work / "mcp.json"
    mcp.write_text(json.dumps({"mcpServers": {"hush": {"command": str(args.hush), "args": ["mcp"]}}}))
    env = {k: os.environ[k] for k in ("HOME", "PATH", "USER", "LANG", "TMPDIR", "TERM") if k in os.environ}
    env.update(CLAUDE_CODE_DISABLE_AUTO_MEMORY="1", CLAUDE_CODE_DISABLE_CLAUDE_MDS="1", DISABLE_AUTOUPDATER="1")
    command = [
        args.claude, "-p", "--model", args.model, "--system-prompt", SYSTEM_PROMPT,
        "--strict-mcp-config", "--mcp-config", str(mcp),
        "--output-format", "stream-json", "--verbose", "--no-session-persistence",
        "--tools", "ToolSearch", "--setting-sources", "project", "--permission-mode", "dontAsk",
        "--max-turns", str(meta["max_turns"]), "--max-budget-usd", "6",
        "--allowedTools", "mcp__hush",
    ]
    started = time.monotonic()
    with open(d / "stream.jsonl", "w") as stream:
        try:
            # On stdin: `--allowedTools` takes a list and would swallow a prompt argument.
            subprocess.run(command, input=prompt, text=True, stdout=stream, stderr=subprocess.STDOUT,
                           env=env, cwd=work, timeout=meta["timeout_s"] + 30)
            timed_out = False
        except subprocess.TimeoutExpired:
            timed_out = True
    wall = round(time.monotonic() - started, 1)
    watcher.terminate()
    watcher.wait()

    subprocess.run([sys.executable, str(probe / "evaluate.py"), "--seed", str(seed), "--state", str(state),
                    "--events", str(events), "--result", str(result_file)], capture_output=True)
    # `--` so pkill doesn't read the pattern as an option.
    subprocess.run(["/usr/bin/pkill", "-f", "--", f"--state {state}"], capture_output=True)
    shutil.rmtree(work, ignore_errors=True)
    shutil.rmtree(d / "home", ignore_errors=True)

    checked = json.loads(result_file.read_text()) if result_file.exists() else {}
    seen = disturbances(watch_log)
    row = {
        "task": task,
        "run": run_index,
        "seed": seed,
        "model": args.model,
        "passed": bool(checked.get("passed")),
        "score": checked.get("score"),
        "failed_checks": [name for name, c in checked.get("checks", {}).items() if not c.get("passed")],
        "undisturbed": not (seen["benchlab_activated"] or seen["benchlab_in_sight"]),
        "wall_seconds": wall,
        "timed_out": timed_out,
        "lane": lane.strip().splitlines()[0] if lane.strip() else "",
        **summarize_stream(d / "stream.jsonl"),
        **seen,
    }
    (d / "row.json").write_text(json.dumps(row, indent=2))
    return row


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--bench", type=Path, required=True, help="cua-driver-bench/automated-eval/macos_bench")
    parser.add_argument("--build", type=Path, required=True, help="directory with BenchLab.app (swift/build.sh)")
    parser.add_argument("--hush", type=Path, default=REPO / "target/release/hush")
    parser.add_argument("--watch", type=Path, required=True, help="the built Watch.swift")
    parser.add_argument("--claude", default=shutil.which("claude") or "claude")
    parser.add_argument("--model", default="claude-sonnet-5-5")
    parser.add_argument("--tasks", nargs="+", default=TASKS)
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--out", type=Path, default=HERE / "results")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    rows_file = args.out / f"rows-{args.model}.jsonl"
    for run_index in range(args.runs):
        for task in args.tasks:
            row = trial(args, task, run_index, args.bench, args.out)
            with rows_file.open("a") as rows:
                rows.write(json.dumps(row) + "\n")
            print(
                f"{task} run {run_index}: {'PASS' if row['passed'] else 'fail'}"
                f"{'' if row['undisturbed'] else ' (DISTURBED)'} · {row['agent_seconds']} s · "
                f"{row['turns']} turns · ${row['cost_usd'] or 0:.2f} · failed {row['failed_checks']}",
                flush=True,
            )


if __name__ == "__main__":
    main()
