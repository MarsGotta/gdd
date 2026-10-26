#!/usr/bin/env python3
"""GDD agent wrapper.

Spec Kit launches every agent (claude, codex, opencode) through this script, thanks to
SPECKIT_INTEGRATION_<AGENT>_EXECUTABLE. The script:
  1. adds GDD options (no MCP servers, per-role permissions, read-only Reviewer);
  2. always asks the agent for JSON output (needed to count tokens) and hands Spec Kit the
     format it asked for;
  3. appends one line per call to .gdd/log.jsonl (change, phase, role, agent, model, tokens, cost);
  4. stores the agent's final text in .gdd/changes/<change>/outputs/ (and review.md for the Reviewer).
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(os.environ.get("GDD_ROOT") or Path(__file__).resolve().parents[2])
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "gdd-kit" / "scripts"))
import gdd  # noqa: E402

# Commands each role may run without asking (headless agents cannot ask for permission).
GIT_READ = ["Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)", "Bash(git show:*)", "Bash(ls:*)"]
ALLOWED = {
    "builder-build": ["Read", "Edit", "Write", "Glob", "Grep", "Bash(bin/gdd g2:*)", "Bash(bin/gdd guard:*)", "Bash(bin/gdd note:*)", *GIT_READ],
    "builder-test": ["Read", "Edit", "Write", "Glob", "Grep", "Bash(bin/gdd g2:*)", "Bash(bin/gdd guard:*)", "Bash(bin/gdd note:*)", *GIT_READ],
    "orchestrator": ["Read", "Edit", "Write", "Glob", "Grep", "Bash(bin/gdd status:*)", "Bash(bin/gdd classify:*)",
                     "Bash(bin/gdd note:*)", "Bash(bin/gdd report:*)", *GIT_READ],
    "reviewer": ["Read", "Glob", "Grep", *GIT_READ],
}
FORBIDDEN = ["Bash(bin/gdd gate:*)", "Bash(bin/gdd launch:*)", "Bash(bin/gdd resume:*)", "Bash(bin/gdd merge:*)",
             "Bash(bin/gdd release:*)", "Bash(git commit:*)", "Bash(git switch:*)", "Bash(git checkout:*)",
             "Bash(git push:*)", "WebFetch", "WebSearch"]

ROLES = {"build-test": ("builder-test", "2-build", "G2"), "build": ("builder-build", "2-build", "G2"),
         "review": ("reviewer", "3-integrate", "G3"), "replan": ("orchestrator", "2-build", "G2")}


def real_binary(name: str) -> str:
    own = (ROOT / ".gdd" / "bin").resolve()
    path = os.pathsep.join(p for p in os.environ.get("PATH", "").split(os.pathsep) if Path(p).resolve() != own)
    b = shutil.which(name, path=path)
    if not b:
        sys.exit(f"gdd: cannot find the real {name} binary")
    return b


def gdd_command(args: list[str]) -> str:
    m = re.search(r"speckit[-.]gdd[-.]([a-z0-9-]+)", " ".join(args))
    return m.group(1) if m else "?"


def value(args: list[str], *flags: str) -> str:
    for i, a in enumerate(args):
        if a in flags and i + 1 < len(args):
            return args[i + 1]
    return "-"


def usage_claude(out: str) -> tuple[dict, str, str]:
    try:
        d = json.loads(out.strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        return {}, out, "-"
    u = d.get("usage", {}) or {}
    models = ",".join((d.get("modelUsage") or {}).keys()) or "-"
    return ({"tokens_in": u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0),
             "tokens_cache": u.get("cache_read_input_tokens", 0), "tokens_out": u.get("output_tokens", 0),
             "cost_usd": d.get("total_cost_usd", 0.0) or 0.0}, d.get("result", "") or "", models)


def usage_codex(out: str) -> tuple[dict, str, str]:
    t = {"tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0}
    text = ""
    for line in out.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        if e.get("type") == "turn.completed":
            u = e.get("usage", {})
            t["tokens_in"] += u.get("input_tokens", 0) - u.get("cached_input_tokens", 0)
            t["tokens_cache"] += u.get("cached_input_tokens", 0)
            t["tokens_out"] += u.get("output_tokens", 0) + u.get("reasoning_output_tokens", 0)
        if e.get("type") == "item.completed" and (e.get("item") or {}).get("type") == "agent_message":
            text = e["item"].get("text", "")
    return t, text, "-"


def usage_opencode(out: str) -> tuple[dict, str, str]:
    t = {"tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0}
    parts = []
    for line in out.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        p = e.get("part") or {}
        if e.get("type") == "step_finish":
            tk = p.get("tokens", {})
            t["tokens_in"] += tk.get("input", 0)
            t["tokens_cache"] += (tk.get("cache") or {}).get("read", 0)
            t["tokens_out"] += tk.get("output", 0) + tk.get("reasoning", 0)
            t["cost_usd"] += p.get("cost", 0) or 0
        if e.get("type") == "text":
            parts.append(p.get("text", ""))
    return t, "\n".join(parts), "-"


def price(model: str, t: dict) -> float:
    pr = gdd.cfg("models").get("prices", {}).get(model, {})
    if not pr:
        return 0.0
    return (t["tokens_in"] * pr.get("input", 0) + t["tokens_cache"] * pr.get("cache", 0) + t["tokens_out"] * pr.get("output", 0)) / 1_000_000


def main() -> None:
    agent, args = sys.argv[1], sys.argv[2:]
    cmd = gdd_command(args)
    role, phase, gate = ROLES.get(cmd, ("orchestrator", "-", "-"))
    change = os.environ.get("GDD_CHANGE", "-")
    json_flags = {"claude": ["--output-format", "json"], "codex": ["--json"], "opencode": ["--format", "json"]}[agent]
    asked_json = json_flags[0] in args
    extra: list[str] = [] if asked_json else list(json_flags)
    if agent == "claude":
        extra += ["--strict-mcp-config", "--permission-mode", "plan" if role == "reviewer" else "acceptEdits"]
        extra += ["--allowedTools", *ALLOWED.get(role, ALLOWED["builder-build"])]
        # GDD state (approvals, G1 hashes, log) is written only through bin/gdd, never by hand.
        state = ["Edit(.gdd/**)", "Write(.gdd/**)"] if role != "orchestrator" else \
            ["Edit(.gdd/log.jsonl)", "Write(.gdd/log.jsonl)", "Edit(.gdd/changes/*/change.json)", "Write(.gdd/changes/*/change.json)"]
        extra += ["--disallowedTools", *(FORBIDDEN if role != "orchestrator" else []), *state]
    elif agent == "codex":
        extra += ["--sandbox", "read-only" if role == "reviewer" else "workspace-write", "-c", "mcp_servers={}"]
        effort = (gdd.cfg("models").get(role.split("-")[0], {}) or {}).get("effort")
        if effort:
            extra += ["-c", f"model_reasoning_effort={effort}"]
    real = [real_binary(agent)]
    if agent == "codex" or (agent == "opencode" and args[:1] == ["run"]):
        argv = real + args[:1] + extra + args[1:]
    else:
        argv = real + args + extra
    t0 = time.time()
    r = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True, env={**os.environ, "GDD_ROLE": role})
    dur = round(time.time() - t0, 1)
    reader = {"claude": usage_claude, "codex": usage_codex, "opencode": usage_opencode}[agent]
    use, text, models = reader(r.stdout)
    sys.stdout.write(r.stdout if asked_json else (text or r.stdout))
    sys.stderr.write(r.stderr)
    model = value(args, "--model", "-m")
    if model == "-" and models != "-":
        model = models
    if use and not use.get("cost_usd"):
        use["cost_usd"] = price(model, use)
    previous = 0
    if gdd.LOG.exists():
        for line in gdd.LOG.open():
            try:
                f = json.loads(line)
            except json.JSONDecodeError:
                continue
            previous += f.get("change") == change and f.get("role") == role
    out_dir = gdd.CHANGES / change / "outputs"
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / f"{role}-{previous + 1}.md").write_text(text or "")
    (out_dir / f"{role}-{previous + 1}.raw").write_text(r.stdout)
    if role == "reviewer":
        (gdd.CHANGES / change / "review.md").write_text(text or "")
    if agent == "claude":
        try:
            sid = json.loads(r.stdout.strip().splitlines()[-1]).get("session_id")
        except (json.JSONDecodeError, IndexError):
            sid = None
        if sid:
            with (gdd.RT / "headless_sessions.txt").open("a") as fh:
                fh.write(sid + "\n")
    gdd.log(change=change, phase=phase, gate=gate, role=role, agent=agent, model=model, duration_s=dur,
            result="ok" if r.returncode == 0 else f"error-{r.returncode}", retry=previous, note=f"/{cmd}", **use)
    sys.exit(r.returncode)


if __name__ == "__main__":
    main()
