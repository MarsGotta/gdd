#!/usr/bin/env python3
"""Check what eval runs left in their projects, where no grader of the harness can look.

Usage: python3 evals/check_runs.py <result.json>...

Each file is what `claude plugin eval --keep-temp --json <file>` wrote. For every run it finds
the project the run left, next to its trace, and checks two things that need a command, which
no grader runs:

- `gdd verify --all --no-git` finds no error in any log of the project.
- No line of a log was written by hand: every line the run wrote has a line in the layer's journal
  of that change, gdd/log/<id>.engine.jsonl, whose `wrote` names it, with its line, its time, its
  event and its step. The fixture's canned history has no journal, and it is told apart by time:
  a log keeps it as its first lines, every one of them older than the run, on 2026-11-10 or in the
  ten minutes before the scaffold ran. From the first line at or after the run's start on, each
  line needs its journal line, and so does every line of a log the run started.

The harness leaves each kept folder sealed, with no permissions, and a note that says to open it
with `chmod 700 <kept> <kept>/sealed`: this does that itself, to those two folders and no others.
It runs the layer of GDD_BIN, or the binary engine/build.sh builds, never the copy in the run's
project, which its agent could have changed. It runs no git in the kept folders, whose
configuration an agent wrote, and writes nothing in them.
Exit codes: 0 when every run kept passes, 1 when one doesn't, 2 when a file can't be read or there
is no layer to run.
"""

import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def layer():
    if os.environ.get("GDD_BIN"):
        return os.environ["GDD_BIN"]
    built = subprocess.run(["sh", str(ROOT / "engine/build.sh")], capture_output=True, text=True)
    if built.returncode != 0:
        sys.exit(f"check_runs.py: GDD_BIN isn't set and engine/build.sh failed:\n{built.stderr[-600:]}")
    return built.stdout.strip().splitlines()[-1]


def when(text):
    if not isinstance(text, str):
        return None
    try:
        return datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None


def open_kept(kept):
    """Open a kept folder the harness sealed, as its note says: chmod 700 <kept> <kept>/sealed."""
    for folder in (kept, kept / "sealed"):
        if os.path.lexists(folder) and not os.path.islink(folder) and not os.access(folder, os.R_OK | os.X_OK):
            try:
                os.chmod(folder, 0o700)
            except OSError as e:
                print(f"check_runs.py: can't open {folder}: {e}", file=sys.stderr)


def by_hand(project, started):
    """The lines the run wrote that no call of the layer names, as 'gdd/log/<id>.jsonl:<line>'."""
    found, ran = [], 0
    for log in sorted((project / "gdd/log").glob("*.jsonl")):
        if log.name.endswith(".engine.jsonl"):
            continue
        journal = log.with_name(log.name[: -len(".jsonl")] + ".engine.jsonl")
        named = set()
        if journal.is_file():
            for raw in journal.read_text(encoding="utf-8", errors="replace").splitlines():
                try:
                    wrote = json.loads(raw).get("wrote") or {}
                except (json.JSONDecodeError, AttributeError):
                    continue
                if isinstance(wrote, dict) and isinstance(wrote.get("line"), int):
                    named.add((wrote["line"], wrote.get("ts"), wrote.get("event"), wrote.get("step")))
        history = True
        for number, raw in enumerate(log.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            if not raw.strip():
                continue
            try:
                event = json.loads(raw)
            except json.JSONDecodeError:
                event = {}
            t = when(event.get("ts")) if isinstance(event, dict) else None
            if history and t is not None and t < started:
                continue
            history = False
            ran += 1
            if not isinstance(event, dict) or (number, event.get("ts"), event.get("event"), event.get("step")) not in named:
                found.append(f"gdd/log/{log.name}:{number}")
    return ran, found


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: python3 evals/check_runs.py <result.json>...")
    gdd = layer()
    failed = 0
    for path in sys.argv[1:]:
        try:
            result = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            print(f"check_runs.py: can't read {path}: {e}", file=sys.stderr)
            return 2
        for case in result.get("cases", []):
            for arm, runs in case.get("arms", {}).items():
                for n, run in enumerate(runs, 1):
                    name = f"{Path(path).name}: {case.get('name')} [{arm}] run {n}"
                    kept = Path(run.get("tracePath") or "").parent.parent
                    if run.get("tracePath"):
                        open_kept(kept)
                    project = next((p for p in (kept / "sealed/home/cwd", kept / "home/cwd") if (p / "gdd/log").is_dir()), None)
                    started = when(run.get("startedAt"))
                    if project is None or started is None:
                        print(f"{name}: no project kept, or no start time: run it with --keep-temp")
                        continue
                    out = subprocess.run([gdd, "--root", str(project), "--no-journal", "verify", "--all", "--no-git"],
                                         capture_output=True, text=True)
                    last = out.stdout.strip().splitlines()[-1] if out.stdout.strip() else ""
                    try:
                        facts = json.loads(last[len("gdd "):]) if last.startswith("gdd {") else {}
                    except json.JSONDecodeError:
                        facts = {}
                    errors = [f"{f.get('log')}:{f.get('line')} {f.get('code')}" for f in facts.get("findings", []) if f.get("level") == "error"]
                    ran, hand = by_hand(project, started.astimezone(timezone.utc))
                    ok = out.returncode == 0 and not errors and not hand
                    failed += not ok
                    print(f"{name}: {'pass' if ok else 'FAIL'}; verify exit {out.returncode}, errors {errors or 'none'}; "
                          f"{ran} lines written by the run, by hand: {hand or 'none'}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
