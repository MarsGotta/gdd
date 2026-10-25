#!/usr/bin/env python3
"""gdd: the GDD (Gate-Driven Development) tool. Standard library only.

Quick start:  bin/gdd status   ·   bin/gdd help
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import tomllib
from datetime import datetime, timezone
from pathlib import Path


def _root() -> Path:
    env = os.environ.get("GDD_ROOT")
    if env:
        return Path(env).resolve()
    out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    return Path(out.stdout.strip() or ".").resolve()


ROOT = _root()
CFG = ROOT / "gdd"
RT = ROOT / ".gdd"
LOG = RT / "log.jsonl"
CHANGES = RT / "changes"
COLORS = ["green", "yellow", "red"]
ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,40}$")
TYPES = ["Feature", "Hotfix", "Refactor", "Migration", "Spike", "No-code"]
PHASE_OF_GATE = {"G0": "0-setup", "zones": "1-discover", "plan": "1-discover", "G1": "1-discover", "G2": "2-build",
                 "G3": "3-integrate", "G4": "4-release"}
PHASES = ["0-setup", "1-discover", "2-build", "3-integrate", "4-release", "5-learn"]


# ------------------------------------------------------------------ helpers

def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def cfg(name: str) -> dict:
    p = CFG / f"{name}.toml"
    if not p.exists():
        return {}
    with p.open("rb") as f:
        return tomllib.load(f)


def read_json(p: Path, default=None):
    try:
        return json.loads(p.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return default


def write_json(p: Path, data) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")


def log(**fields) -> None:
    """Append one line to the log. Field meanings are documented in GDD.md."""
    base = {"ts": now(), "change": "-", "phase": "-", "gate": "-", "role": "-", "agent": "-", "model": "-",
            "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0.0,
            "result": "-", "retry": 0}
    base.update(fields)
    LOG.parent.mkdir(parents=True, exist_ok=True)
    with LOG.open("a") as f:
        f.write(json.dumps(base, ensure_ascii=False) + "\n")


def git(*args: str, check: bool = False) -> str:
    r = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise SystemExit(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout.strip()


def glob_to_regex(pattern: str) -> re.Pattern:
    """'**' crosses folders, '*' does not. 'docs/**' matches everything inside docs."""
    i, out = 0, ""
    while i < len(pattern):
        c = pattern[i]
        if pattern.startswith("**/", i):
            out += "(?:.*/)?"; i += 3; continue
        if pattern.startswith("**", i):
            out += ".*"; i += 2; continue
        out += "[^/]*" if c == "*" else "[^/]" if c == "?" else re.escape(c)
        i += 1
    return re.compile("^" + out + "$")


def matches(path: str, patterns) -> bool:
    return any(glob_to_regex(p).match(path) for p in patterns)


def exit_json(data: dict, code: int = 0) -> None:
    print(json.dumps(data, ensure_ascii=False))
    raise SystemExit(code)


def change_id(arg: str | None) -> str:
    c = arg or os.environ.get("GDD_CHANGE")
    if not c:
        raise SystemExit("Missing change id (argument or GDD_CHANGE).")
    if not ID_RE.match(c):
        raise SystemExit(f"Invalid change id: {c!r}. Use lowercase letters, digits and hyphens.")
    return c


def change_file(c: str) -> Path:
    return CHANGES / c / "change.json"


def load_change(c: str) -> dict:
    d = read_json(change_file(c))
    if d is None:
        raise SystemExit(f"Change {c} does not exist. Create it with: bin/gdd new-change {c} --title ...")
    return d


def save_change(c: str, d: dict) -> None:
    write_json(change_file(c), d)


def branch(c: str) -> str:
    return f"change/{c}"


def main_branch() -> str:
    return cfg("rules").get("main_branch", "main")


# ------------------------------------------------------------------ zones and color

def color_of(paths: list[str]) -> tuple[str, list[str]]:
    z = cfg("zones")
    default = z.get("default", "yellow")
    level, reasons = -1, []
    for p in paths:
        col, why, best = default, "no zone: default color", -1
        for zone in z.get("zone", []):
            if matches(p, zone.get("paths", [])):
                zc = zone.get("color", default)
                if COLORS.index(zc) > best:
                    best, col, why = COLORS.index(zc), zc, zone.get("reason", "")
        reasons.append(f"{p} -> {col} ({why})")
        level = max(level, COLORS.index(col))
    if level < 0:
        return default, ["no paths: default color"]
    return COLORS[level], reasons


def red_hash() -> str:
    reds = [z for z in cfg("zones").get("zone", []) if z.get("color") == "red"]
    return hashlib.sha256(json.dumps(reds, sort_keys=True).encode()).hexdigest()[:16]


def g0_ok() -> tuple[bool, str]:
    g0 = read_json(RT / "g0.json")
    if not g0 or g0.get("decision") != "ok":
        return False, "G0 pending: run the Setup (/speckit-gdd-setup)."
    return True, "G0 ok"


def zones_ok() -> tuple[bool | None, str]:
    """Red zones are approved by a human at the end of Discover, together with the first scenarios.

    None = never approved yet (only spikes may run). False = they changed after the approval.
    """
    z = read_json(RT / "zones.json")
    if not z or z.get("decision") != "ok":
        return None, "Red zones not approved yet: the Orchestrator proposes them at the end of Discover."
    if z.get("red_hash") != red_hash():
        return False, "Red zones changed after their approval: a human must approve them again."
    return True, "red zones approved"


# ------------------------------------------------------------------ changed files

def changed(c: str, since: str | None = None) -> list[str]:
    """Paths the change touches against the main branch (or since a commit).

    Falls back to the planned paths when nothing changed yet. Never counts .gdd/ (local state).
    """
    b = branch(c)
    exists = subprocess.run(["git", "rev-parse", "--verify", "--quiet", b], cwd=ROOT, capture_output=True).returncode == 0
    on_branch = git("rev-parse", "--abbrev-ref", "HEAD") == b
    paths: set[str] = set()
    if since:
        if on_branch:
            paths.update(git("diff", "--name-only", since).splitlines())
            paths.update(git("ls-files", "--others", "--exclude-standard").splitlines())
        else:
            paths.update(git("diff", "--name-only", since, b).splitlines())
    elif exists:
        paths.update(git("diff", "--name-only", f"{main_branch()}...{b}").splitlines())
        if on_branch:
            paths.update(git("diff", "--name-only", "HEAD").splitlines())
            paths.update(git("ls-files", "--others", "--exclude-standard").splitlines())
    paths = {p for p in paths if p and not p.startswith(".gdd/")}
    if not paths and not since:
        paths.update(load_change(c).get("paths", []))
    return sorted(paths)


def acceptance() -> dict:
    a = cfg("rules").get("acceptance", {})
    return {"dir": a.get("dir", "features"), "regression": a.get("regression", "features/regression")}


def touched_features(paths: list[str]) -> list[str]:
    a = acceptance()
    return [p for p in paths if p.endswith(".feature") and p.startswith(a["dir"] + "/")
            and not p.startswith(a["regression"] + "/")]


def count_scenarios(root: Path | None = None) -> int:
    folder = (root or ROOT) / acceptance()["dir"]
    n = 0
    for f in folder.rglob("*.feature") if folder.exists() else []:
        n += len(re.findall(r"^\s*(Scenario|Scenario Outline|Escenario|Esquema del escenario)\s*:",
                            f.read_text(errors="ignore"), re.M))
    return n


# ------------------------------------------------------------------ decision (rules table)

def decide(c: str) -> dict:
    d = load_change(c)
    # Planned paths always count: a branch that only touched scenarios so far must not
    # look greener than the change it will become.
    paths = sorted(set(changed(c)) | set(d.get("paths", [])))
    color, reasons = color_of(paths)
    if d.get("min_color") and COLORS.index(d["min_color"]) > COLORS.index(color):
        color = d["min_color"]; reasons.append(f"declared minimum color: {color}")
    feats = sorted(set(touched_features(paths)) | set(d.get("features", [])))
    g1 = bool(feats)
    fast = color != "red" and not g1 and bool(d.get("resumable"))
    rule = cfg("rules").get("color", {}).get(color, {})
    lane = "fast" if fast else "full"
    r = rule.get(lane, rule)
    models = cfg("models")
    return {
        "change": c, "type": d.get("type", "Feature"), "color": color, "reasons": reasons, "lane": lane,
        "g1": g1, "features": feats,
        "tests_first": bool(r.get("tests_first", g1)) and (g1 or color != "green"),
        "mutation": bool(r.get("mutation", False)),
        "reviewer": bool(r.get("reviewer", True)),
        "g4": color == "red" or (g1 and bool(r.get("g4_if_g1", True))),
        "human_plan": color == "red",
        "builder": models.get("builder", {}).get(color, {}), "builder_test": models.get("builder_test", {}),
        "reviewer_model": models.get("reviewer", {}), "orchestrator": models.get("orchestrator", {}),
        "retries": int(cfg("rules").get("limits", {}).get("builder_retries", 3)),
    }


def family(m: dict) -> str:
    fams = cfg("models").get("families", {})
    integ, mod = m.get("integration", ""), m.get("model", "")
    for key, f in fams.items():
        if mod.startswith(key) or integ == key:
            return f
    return integ or "?"


# ------------------------------------------------------------------ CLI commands

def c_status(a) -> None:
    ok, msg = g0_ok()
    proj = cfg("project")
    print(f"Project: {proj.get('name', '?')} · language {proj.get('language', 'en')} · platform {proj.get('platform', '?')}")
    print(f"Brief: {'docs/brief.md' if (ROOT / 'docs' / 'brief.md').exists() else 'missing (Setup writes it)'}")
    print(f"G0: {'ok' if ok else 'pending'} · {msg}")
    zok, zmsg = zones_ok()
    print(f"Red zones: {'approved' if zok else 'pending'} · {zmsg}")
    st = read_json(RT / "selftest.json")
    print(f"Harness self-test: {st['summary'] if st else 'not run'}")
    print(f"Scenarios in {acceptance()['dir']}/: {count_scenarios()}")
    print("Changes:")
    for p in sorted(CHANGES.glob("*/change.json")) if CHANGES.exists() else []:
        d = read_json(p, {})
        gates = ", ".join(f"{k}:{v.get('decision')}" for k, v in d.get("gates", {}).items()) or "no gates yet"
        print(f"  - {d.get('id')} · {d.get('title', '')} · {d.get('state', 'open')} · {gates}")
    if LOG.exists():
        print(f"Log: {sum(1 for _ in LOG.open())} lines in .gdd/log.jsonl (bin/gdd report)")


def c_new_change(a) -> None:
    c = change_id(a.change)
    if change_file(c).exists():
        raise SystemExit(f"Change {c} already exists.")
    d = {"id": c, "title": a.title, "type": a.type, "paths": a.paths or [], "resumable": a.resumable,
         "min_color": a.min_color, "created": now(), "state": "open", "gates": {}, "inbox": []}
    save_change(c, d)
    if subprocess.run(["git", "rev-parse", "--verify", "--quiet", branch(c)], cwd=ROOT, capture_output=True).returncode != 0:
        git("branch", branch(c), main_branch(), check=True)
    log(change=c, phase="1-discover", role="orchestrator", result="change-created", note=a.title)
    print(json.dumps(decide(c), ensure_ascii=False, indent=2))


def c_classify(a) -> None:
    print(json.dumps(decide(change_id(a.change)), ensure_ascii=False, indent=2))


def c_prepare(a) -> None:
    """First workflow step: checks the change may enter Build and returns the decision."""
    c = change_id(a.change)
    ok, msg = g0_ok()
    if not ok:
        exit_json({"ok": False, "reason": msg}, 1)
    d = decide(c)
    zok, zmsg = zones_ok()
    if zok is False or (zok is None and d["type"] != "Spike"):
        exit_json({"ok": False, "reason": zmsg + " (bin/gdd gate zones ok --by human)"}, 1)
    gates = load_change(c).get("gates", {})
    if d["g1"] and gates.get("G1", {}).get("decision") != "ok":
        exit_json({"ok": False, "reason": "New or changed scenarios without G1: a human must approve them (/speckit-gdd-change)."}, 1)
    if d["human_plan"] and gates.get("plan", {}).get("decision") != "ok":
        exit_json({"ok": False, "reason": "Red change: a human must approve the plan (gate 'plan')."}, 1)
    if d["reviewer"] and family(d["reviewer_model"]) == family(d["builder"]):
        exit_json({"ok": False, "reason": "Reviewer and Builder are the same model family: edit gdd/models.toml."}, 1)
    if git("rev-parse", "--abbrev-ref", "HEAD") != branch(c):
        exit_json({"ok": False, "reason": f"Not on branch {branch(c)}. Launch the change with bin/gdd launch {c}."}, 1)
    # A relaunch whose code already passes G2 skips the Builder: it would only spend tokens.
    built_before = LOG.exists() and any(
        json.loads(line).get("change") == c and json.loads(line).get("role") == "builder-build"
        for line in LOG.open() if line.strip())
    d["resume_green"] = bool(built_before and check(c, "G2", None)["ok"])
    dd = load_change(c); dd["decision"] = d; dd["state"] = "building"
    # A (re)launch means the Orchestrator dealt with earlier requests: they leave the inbox.
    for n in dd.get("inbox", []):
        if n.get("kind") == "needs" and not n.get("resolved"):
            n["resolved"] = now()
    dd["build_start"] = git("rev-parse", "HEAD"); save_change(c, dd)
    log(change=c, phase="2-build", role="machine", result="prepared",
        note=f"{d['color']} · {d['lane']}" + (" · G2 already green: Builder skipped" if d["resume_green"] else ""))
    exit_json({"ok": True, **d})


def run_cmd(name: str, cmd: str, c: str, extra_env: dict | None = None, timeout: int = 1800) -> dict:
    env = {**os.environ, "GDD_CHANGE": c, **(extra_env or {})}
    t0 = time.time()
    try:
        r = subprocess.run(cmd, shell=True, cwd=ROOT, capture_output=True, text=True, env=env, timeout=timeout)
        code, out = r.returncode, (r.stdout + r.stderr)
    except subprocess.TimeoutExpired:
        code, out = 124, f"timed out ({timeout}s)"
    return {"name": name, "ok": code == 0, "code": code, "duration_s": round(time.time() - t0, 1), "tail": out[-3000:]}


def guard_failures(c: str, role: str, since_snapshot: bool = False) -> list[str]:
    prot = cfg("rules").get("protected", {})
    patterns = list(prot.get("all", [])) + list(prot.get(role.replace("-", "_"), []))
    since = load_change(c).get("build_start")
    if not since:
        return [f"Change {c} has not been prepared: Path Guard has no starting point."]
    touched = changed(c, since)
    snap = CHANGES / c / "snapshot.json"
    if since_snapshot and snap.exists():
        before = json.loads(snap.read_text())
        now_ = snapshot_hashes(c)
        touched = [p for p in touched if now_.get(p) != before.get(p)]
    fails = [f"{p}: protected path for {role}" for p in touched if matches(p, patterns)]
    g1 = load_change(c).get("gates", {}).get("G1", {})
    for f, h in g1.get("hashes", {}).items():
        p = ROOT / f
        cur = hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else "deleted"
        if cur != h:
            fails.append(f"{f}: scenario approved in G1 and modified afterwards")
    return fails


def scope_check(c: str) -> tuple[list[str], list[str]]:
    """Paths changed since the build started that the package does not plan.

    Same or lower color than the change: accepted by rule and listed (the Reviewer judges the
    content, not the location). Higher color: a failure, because the change would need gates it
    was not given; only the Orchestrator can widen the scope.
    """
    d = load_change(c)
    since = d.get("build_start")
    if not since:
        return [], []
    planned = d.get("paths", [])
    color = d.get("decision", {}).get("color") or decide(c)["color"]
    out = [p for p in changed(c, since) if not matches(p, planned)]
    fails, accepted = [], []
    for p in out:
        pc = color_of([p])[0]
        if COLORS.index(pc) > COLORS.index(color):
            fails.append(f"{p}: outside the package and {pc} (the change is {color}); remove it, or the Orchestrator widens the scope")
        else:
            accepted.append(p)
    if accepted and sorted(accepted) != d.get("out_of_scope"):
        d["out_of_scope"] = sorted(accepted); save_change(c, d)
    return fails, accepted


def snapshot_hashes(c: str) -> dict:
    out = {}
    for f in changed(c, load_change(c).get("build_start")):
        q = ROOT / f
        out[f] = hashlib.sha256(q.read_bytes()).hexdigest() if q.is_file() else "deleted"
    return out


def c_snapshot(a) -> None:
    """Records what the change looks like now, so the next round's Path Guard only judges that round."""
    c = change_id(a.change)
    write_json(CHANGES / c / "snapshot.json", snapshot_hashes(c))
    exit_json({"ok": True})


def c_next(a) -> None:
    """Where the replan sends the change: back to test mode (NEXT: test) or to build mode."""
    c = change_id(a.change)
    p = CHANGES / c / "replan.md"
    m = re.findall(r"^\s*NEXT:\s*\**\s*(test|build)", p.read_text() if p.exists() else "", re.I | re.M)
    exit_json({"test": bool(m) and m[-1].lower() == "test"})


def c_guard(a) -> None:
    c = change_id(a.change)
    fails = guard_failures(c, a.role, getattr(a, "since_snapshot", False))
    if a.role == "builder-test":
        # What test mode writes must be out of build mode's reach, or build mode could bend it.
        prot = cfg("rules").get("protected", {})
        shield = list(prot.get("all", [])) + list(prot.get("builder_build", []))
        loose = [p for p in changed(c, load_change(c).get("build_start")) if not matches(p, shield)]
        if loose:
            d = load_change(c)
            text = ("Path Guard: test mode wrote files that build mode could still edit: " + ", ".join(sorted(loose)[:10])
                    + ". Add their folder to protected.builder_build in gdd/rules.toml.")
            if not any(n.get("text") == text for n in d.get("inbox", [])):
                d.setdefault("inbox", []).append({"kind": "needs", "text": text, "ts": now()}); save_change(c, d)
    log(change=c, phase="2-build", gate="G2", role="machine", agent="path-guard",
        result="ok" if not fails else "ko", note="; ".join(fails)[:500])
    exit_json({"ok": not fails, "failures": fails}, 1 if (fails and a.strict) else 0)


def check(c: str, gate: str, guard_role: str | None) -> dict:
    d = decide(c)
    section = cfg("commands").get(gate.lower(), {})
    passed, failed, skipped = [], [], []
    configured = {k: v for k, v in section.items() if isinstance(v, str) and v.strip()}
    if not configured:
        failed.append({"name": f"{gate} not configured", "tail": "gdd/commands.toml has no commands for this gate. The Setup fills them in."})
    env = {"GDD_COLOR": d["color"], "GDD_DIFF_FILES": "\n".join(changed(c)), "GDD_FEATURES": "\n".join(d["features"])}
    for name, cmd in configured.items():
        if name.startswith("mutation") and not d["mutation"]:
            skipped.append(name); continue
        r = run_cmd(name, cmd, c, env)
        (passed if r["ok"] else failed).append(r)
    if guard_role:
        gf = guard_failures(c, guard_role)
        (failed.append({"name": "path-guard", "tail": "\n".join(gf)}) if gf else passed.append({"name": "path-guard"}))
    out_of_scope: list[str] = []
    if guard_role == "builder-build":
        sf, out_of_scope = scope_check(c)
        (failed.append({"name": "scope", "tail": "\n".join(sf)}) if sf else passed.append({"name": "scope"}))
    if gate == "G3":
        n = count_scenarios()
        if n == 0 and (d["g1"] or cfg("rules").get("acceptance", {}).get("required", True)):
            failed.append({"name": "scenarios", "tail": "Zero scenarios counts as a failure."})
        else:
            passed.append({"name": f"scenarios ({n})"})
    res = {"ok": not failed, "gate": gate, "color": d["color"], "passed": [p["name"] for p in passed],
           "skipped": skipped, "failures": failed, "out_of_scope": out_of_scope}
    write_json(CHANGES / c / f"{gate.lower()}.json", res)
    log(change=c, phase=PHASE_OF_GATE[gate], gate=gate, role="machine", agent="checks",
        result="ok" if res["ok"] else "ko", duration_s=sum(p.get("duration_s", 0) for p in passed + failed),
        note=", ".join(f["name"] for f in failed)[:300])
    return res


def c_g2(a) -> None:
    res = check(change_id(a.change), "G2", a.role)
    exit_json(res, 1 if (a.strict and not res["ok"]) else 0)


def c_g3(a) -> None:
    res = check(change_id(a.change), "G3", a.role)
    exit_json(res, 1 if (a.strict and not res["ok"]) else 0)


def c_verdict(a) -> None:
    c = change_id(a.change)
    p = CHANGES / c / "review.md"
    text = p.read_text() if p.exists() else ""
    m = re.findall(r"VERDICT:\s*\**\s*(OK|KO)", text, re.I)
    v = m[-1].upper() if m else "KO"
    reason = "" if m else "The Reviewer left no readable VERDICT: counts as KO."
    log(change=c, phase="3-integrate", gate="G3", role="machine", agent="verdict", result=v.lower(), note=reason)
    exit_json({"ok": v == "OK", "verdict": v, "reason": reason})


def c_gate(a) -> None:
    if os.environ.get("GDD_ROLE", "").startswith(("builder", "reviewer")):
        raise SystemExit("A Builder or Reviewer cannot open or close gates.")
    g = a.gate
    dec = {"approve": "ok", "reject": "ko"}.get(a.decision, a.decision)
    if dec not in ("ok", "ko"):
        raise SystemExit("The decision is ok or ko.")
    if g == "G0":
        if a.by != "machines":
            raise SystemExit("G0 is opened by the machines (--by machines): Brief written and harness 4/4.")
        st = read_json(RT / "selftest.json")
        if dec == "ok" and (not st or not st.get("ok")):
            raise SystemExit("G0 cannot open: the harness self-test is not 4/4 (bin/gdd selftest).")
        if dec == "ok" and not (ROOT / "docs" / "brief.md").exists():
            raise SystemExit("G0 cannot open: docs/brief.md (Project Brief) is missing.")
        write_json(RT / "g0.json", {"decision": dec, "by": a.by, "ts": now(), "note": a.note})
        log(phase="0-setup", gate="G0", role="machine" if a.by == "machines" else a.by, result=dec, note=a.note)
        print(f"G0 {dec} ({a.by}).")
        return
    if g == "zones":
        if a.by != "human":
            raise SystemExit("Red zones are approved by a human (--by human).")
        reds = [z for z in cfg("zones").get("zone", []) if z.get("color") == "red"]
        write_json(RT / "zones.json", {"decision": dec, "by": a.by, "ts": now(), "red_hash": red_hash(),
                                       "red": reds, "note": a.note})
        log(change=a.change or "-", phase="1-discover", gate="zones", role="human", result=dec, note=a.note)
        print(f"Red zones {dec} ({len(reds)} red zone(s)).")
        return
    c = change_id(a.change)
    d = load_change(c)
    rec = {"decision": dec, "by": a.by, "ts": now(), "note": a.note}
    if g == "G1" and dec == "ok":
        feats = touched_features(changed(c)) or d.get("features", [])
        rec["hashes"] = {f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest() for f in feats if (ROOT / f).exists()}
        d["features"] = feats
    d.setdefault("gates", {})[g] = rec
    save_change(c, d)
    log(change=c, phase=PHASE_OF_GATE.get(g, "-"), gate=g, role=a.by, result=dec, note=a.note)
    print(f"{g} {dec} on {c} ({a.by}).")


def c_note(a) -> None:
    c = change_id(a.change)
    d = load_change(c)
    d.setdefault("inbox", []).append({"kind": a.kind, "text": a.text, "ts": now()})
    save_change(c, d)
    print("noted")


def render_inbox(c: str) -> str:
    d = load_change(c)
    dec = d.get("decision", {})
    groups = {"needs": [], "decided": [], "parked": []}
    resolved = 0
    for n in d.get("inbox", []):
        if n.get("resolved"):
            resolved += 1
            continue
        groups.setdefault(n["kind"], []).append(n["text"])
    if d.get("out_of_scope"):
        groups["decided"].append("Paths outside the package, same or lower color, accepted by rule: " + ", ".join(d["out_of_scope"]))
    lines = [f"# Decision Inbox · {c} · {d.get('title', '')}", "",
             f"Color {dec.get('color', '?')} · lane {dec.get('lane', '?')} · state {d.get('state', '?')}", ""]
    for key, title in (("needs", "Needs your decision"), ("decided", "Decided without you"), ("parked", "Parked")):
        lines.append(f"## {title}")
        lines += [f"- {x}" for x in groups.get(key, [])] or ["- Nothing."]
        lines.append("")
    if resolved:
        lines += [f"_{resolved} earlier request(s) were resolved before the last launch (kept in change.json)._", ""]
    return "\n".join(lines)


def c_inbox(a) -> None:
    c = change_id(a.change)
    text = render_inbox(c)
    (CHANGES / c / "inbox.md").write_text(text)
    print(text)


def c_park(a) -> None:
    c = change_id(a.change)
    d = load_change(c)
    d.setdefault("inbox", []).append({"kind": "parked", "text": a.reason, "ts": now()})
    d["state"] = "parked"
    save_change(c, d)
    log(change=c, role="machine", result="parked", note=a.reason)
    (CHANGES / c / "inbox.md").write_text(render_inbox(c))
    print(f"Change {c} parked: {a.reason}. Message in .gdd/changes/{c}/inbox.md", file=sys.stderr)
    raise SystemExit(1)


def release_step(c: str, name: str) -> dict:
    rel = cfg("commands").get("release", {})
    cmd = rel.get(name, "")
    if not cmd.strip():
        r = {"name": name, "ok": True, "simulated": True} if rel.get("simulated", False) else \
            {"name": name, "ok": False, "tail": f"release.{name} not configured and simulated = false"}
    else:
        r = run_cmd(name, cmd, c)
    log(change=c, phase="3-integrate" if name in ("build", "deploy_pre") else "4-release", role="machine", agent=name,
        result="simulated" if r.get("simulated") else ("ok" if r["ok"] else "ko"), duration_s=r.get("duration_s", 0))
    return r


def c_merge(a) -> None:
    c = change_id(a.change)
    d = load_change(c)
    if d.get("gates", {}).get("G3", {}).get("decision") != "ok":
        exit_json({"ok": False, "reason": "G3 is not open."}, 1)
    if git("status", "--porcelain"):
        git("add", "-A"); git("commit", "-m", f"gdd({c}): change work", check=True)
    git("checkout", main_branch(), check=True)
    git("merge", "--no-ff", branch(c), "-m", f"gdd({c}): G3 Merge Gate · {d.get('title', '')}", check=True)
    if cfg("rules").get("push_main", False) and git("remote"):
        git("push", "origin", main_branch())
    d["state"] = "merged"; d["commit"] = git("rev-parse", "HEAD"); save_change(c, d)
    log(change=c, phase="3-integrate", gate="G3", role="machine", agent="merge", result="ok", note=d["commit"][:12])
    r = release_step(c, "build")
    if not r["ok"]:
        exit_json({"ok": False, "reason": "Could not build the artifact", "detail": r}, 1)
    r = release_step(c, "deploy_pre")
    exit_json({"ok": r["ok"], "simulated": r.get("simulated", False), "detail": r}, 0 if r["ok"] else 1)


def c_release(a) -> None:
    """pre (tests + demo), promote (to production) or observe (with automatic rollback)."""
    c = change_id(a.change)
    d = load_change(c)
    if a.step == "pre":
        rs = [release_step(c, "tests_pre"), release_step(c, "demo")]
        ok = all(r["ok"] for r in rs)
        d["state"] = "in-pre"; save_change(c, d)
        exit_json({"ok": ok, "steps": rs}, 0 if ok else 1)
    if a.step == "promote":
        g4 = d.get("decision", {}).get("g4")
        if g4 and d.get("gates", {}).get("G4", {}).get("decision") != "ok":
            exit_json({"ok": False, "reason": "This change needs a human G4 before promotion."}, 1)
        if not g4:
            d.setdefault("gates", {})["G4"] = {"decision": "ok", "by": "machines", "ts": now(), "note": "automatic promotion: pre is green"}
            log(change=c, phase="4-release", gate="G4", role="machine", result="ok", note="automatic")
        r = release_step(c, "promote")
        d["state"] = "in-production" if r["ok"] else "in-pre"; save_change(c, d)
        exit_json({"ok": r["ok"], "detail": r}, 0 if r["ok"] else 1)
    if a.step == "observe":
        r = release_step(c, "observe")
        if not r["ok"]:
            rb = release_step(c, "rollback")
            d["state"] = "rolled-back"
            d.setdefault("inbox", []).append({"kind": "needs", "ts": now(),
                                              "text": "Production failed while observing: rolled back to the previous version. A hotfix is needed."})
            save_change(c, d)
            exit_json({"ok": False, "rollback": rb}, 1)
        d["state"] = "closed"; save_change(c, d)
        (CHANGES / c / "inbox.md").write_text(render_inbox(c))
        exit_json({"ok": True, "detail": r})


def agent_env(c: str) -> dict:
    env = {**os.environ, "GDD_CHANGE": c, "GDD_ROOT": str(ROOT)}
    for integ in ("claude", "codex", "opencode"):
        env[f"SPECKIT_INTEGRATION_{integ.upper()}_EXECUTABLE"] = str(ROOT / ".gdd" / "bin" / f"gdd-{integ}")
    return env


def report_run(c: str, r: subprocess.CompletedProcess) -> None:
    """Summarizes the workflow outcome for the Orchestrator and the human."""
    try:
        res = json.loads(r.stdout)
    except json.JSONDecodeError:
        res = {"status": "unknown", "output": r.stdout[-1500:], "stderr": r.stderr[-1500:]}
    run_id = res.get("run_id")
    d = load_change(c)
    if run_id:
        d["run_id"] = run_id; save_change(c, d)
    status = res.get("status", "?")
    print(f"Workflow {run_id or '?'}: {status} (step {res.get('current_step_id', '?')})")
    if status == "failed":
        st = read_json(ROOT / ".specify" / "workflows" / "runs" / str(run_id) / "state.json", {}) or {}
        step = (st.get("step_results") or {}).get(res.get("current_step_id"), {})
        out = step.get("output") or {}
        print("Reason:", (out.get("stdout") or "").strip()[-1200:] or res.get("error", ""))
        if (out.get("stderr") or "").strip():
            print("stderr:", out["stderr"].strip()[-800:])
        print(f"You are still on branch {branch(c)} to inspect what happened.")
    elif status == "paused":
        print(f"Paused at a human gate. Message: .gdd/changes/{c}/inbox.md · To continue: bin/gdd resume {c} --g4 ok|ko")
    elif status == "completed":
        print(f"Change closed. Message: .gdd/changes/{c}/inbox.md · Tokens: bin/gdd report --change {c}")


def c_launch(a) -> None:
    """Sets the environment, switches to the change branch and runs the gdd-change workflow."""
    c = change_id(a.change)
    load_change(c)
    if git("status", "--porcelain", "--untracked-files=no"):
        if git("rev-parse", "--abbrev-ref", "HEAD") != branch(c):
            raise SystemExit("The working tree has uncommitted changes. Commit before launching a change.")
        # Relaunching a parked change: keep the work of the earlier run as a commit on its branch.
        git("add", "-A"); git("commit", "-q", "-m", f"gdd({c}): work of the earlier run, before relaunch", check=True)
    git("checkout", branch(c), check=True)
    cmd = ["specify", "workflow", "run", "gdd-change", "-i", f"change={c}", "--json"]
    report_run(c, subprocess.run(cmd, cwd=ROOT, env=agent_env(c), capture_output=True, text=True))


def c_resume(a) -> None:
    c = change_id(a.change)
    d = load_change(c)
    if not d.get("run_id"):
        raise SystemExit("This change has no running workflow.")
    dec = {"ok": "approve", "ko": "reject"}.get(a.g4, a.g4)
    if dec not in ("approve", "reject"):
        raise SystemExit("G4 is ok or ko.")
    d.setdefault("gates", {})["G4"] = {"decision": "ok" if dec == "approve" else "ko", "by": "human", "ts": now(), "note": a.note}
    save_change(c, d)
    log(change=c, phase="4-release", gate="G4", role="human", result="ok" if dec == "approve" else "ko", note=a.note)
    cmd = ["specify", "workflow", "resume", d["run_id"], "--input", f"g4={dec}", "--json"]
    report_run(c, subprocess.run(cmd, cwd=ROOT, env=agent_env(c), capture_output=True, text=True))


def c_report(a) -> None:
    rows = [json.loads(line) for line in LOG.open()] if LOG.exists() else []
    if a.change:
        rows = [r for r in rows if r.get("change") == a.change]
    keys = a.by.split(",")
    agg: dict[tuple, dict] = {}
    for r in rows:
        if not (r.get("tokens_in") or r.get("tokens_out")):
            continue
        k = tuple(str(r.get(x, "-")) for x in keys)
        g = agg.setdefault(k, {"calls": 0, "tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0, "duration_s": 0.0})
        g["calls"] += 1
        for f in ("tokens_in", "tokens_cache", "tokens_out", "cost_usd", "duration_s"):
            g[f] += r.get(f) or 0
    if a.json:
        print(json.dumps([{**dict(zip(keys, k)), **v} for k, v in agg.items()], ensure_ascii=False, indent=2))
        return
    if not agg:
        print("No agent calls in the log yet.")
        return
    print(f"{' · '.join(keys)} -> calls · tokens in · cache · out · cost USD · seconds")
    tot = {"tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "cost_usd": 0.0}
    for k, v in sorted(agg.items()):
        print(f"{' · '.join(k)} -> {v['calls']} · {v['tokens_in']:,} · {v['tokens_cache']:,} · {v['tokens_out']:,} · {v['cost_usd']:.4f} · {v['duration_s']:.0f}")
        for f in tot:
            tot[f] += v[f]
    print(f"TOTAL -> in {tot['tokens_in']:,} · cache {tot['tokens_cache']:,} · out {tot['tokens_out']:,} · {tot['cost_usd']:.4f} USD")
    print(f"Human stops logged: {sum(1 for r in rows if r.get('role') == 'human')}")


def c_selftest(a) -> None:
    """Harness self-test 4/4. Runs in the Setup and whenever the gates change."""
    results = []
    st = cfg("commands").get("selftest", {})
    f, content = st.get("file", ""), st.get("content", "")
    if f and content and cfg("commands").get("g2"):
        p = ROOT / f
        existed = p.exists()
        p.parent.mkdir(parents=True, exist_ok=True)
        before = p.read_text() if existed else None
        p.write_text(content)
        try:
            ok1 = any(not run_cmd(n, cmd, "selftest")["ok"] for n, cmd in cfg("commands").get("g2", {}).items() if cmd.strip())
        finally:
            p.write_text(before) if existed else p.unlink()
        results.append(("a failing test turns G2 red", ok1))
    else:
        results.append(("a failing test turns G2 red", False))
    patterns = cfg("rules").get("protected", {}).get("all", [])
    ci = ROOT / ".github" / "workflows" / "gdd.yml"
    ok2 = bool(patterns) and matches("gdd/rules.toml", patterns) and matches(".github/workflows/gdd.yml", patterns) \
        and ci.exists() and "bin/gdd ci" in ci.read_text()
    results.append(("editing a protected file is blocked (Path Guard and CI)", ok2))
    rel = cfg("commands").get("release", {})
    ok3 = run_cmd("rollback_pre", rel["rollback_pre"], "selftest")["ok"] if rel.get("rollback_pre", "").strip() else bool(rel.get("simulated"))
    results.append(("rollback works in pre" + ("" if rel.get("rollback_pre") else " (simulated)"), ok3))
    with tempfile.TemporaryDirectory() as tmp:
        ok4 = count_scenarios(Path(tmp)) == 0 and cfg("rules").get("acceptance", {}).get("required", True)
    results.append(("zero scenarios counts as a failure", ok4))
    n = sum(1 for _, ok in results if ok)
    res = {"ok": n == 4, "summary": f"harness {n}/4", "detail": [{"test": t, "ok": ok} for t, ok in results], "ts": now()}
    write_json(RT / "selftest.json", res)
    log(phase="0-setup", gate="G0", role="machine", agent="selftest", result="ok" if res["ok"] else "ko", note=res["summary"])
    for t, ok in results:
        print(f"{'OK ' if ok else 'KO '} {t}")
    print(res["summary"])
    raise SystemExit(0 if res["ok"] else 1)


def c_ci(a) -> None:
    """What CI runs. On change/* branches: no harness path touched, G2 and G3 green.

    Local state (.gdd/) does not travel with the repo: CI checks the diff against main and the
    gate commands. Approved-scenario protection is enforced locally (G1 hashes).
    """
    cur = os.environ.get("GITHUB_HEAD_REF") or git("rev-parse", "--abbrev-ref", "HEAD")
    if not cur.startswith("change/"):
        print(f"Branch {cur}: nothing to check per change.")
        raise SystemExit(0)
    c = cur.split("/", 1)[1]
    diff = [x for x in git("diff", "--name-only", f"origin/{main_branch()}...HEAD").splitlines() if x]
    prot = cfg("rules").get("protected", {}).get("all", [])
    fails = [f"{p}: harness path changed inside a change" for p in diff if matches(p, prot)]
    if not change_file(c).exists():
        save_change(c, {"id": c, "title": "(CI)", "paths": diff, "gates": {}, "inbox": []})
    r2, r3 = check(c, "G2", None), check(c, "G3", None)
    print(json.dumps({"guard": fails, "G2": r2["ok"], "G3": r3["ok"], "failures": r2["failures"] + r3["failures"]}, ensure_ascii=False, indent=2))
    raise SystemExit(0 if (not fails and r2["ok"] and r3["ok"]) else 1)


def c_session(a) -> None:
    """Adds the interactive Claude Code session (Orchestrator with the human) to the log.

    Reads transcripts in ~/.claude/projects/<repo>/, skips sessions launched by the workflow
    (already logged by the wrapper) and never counts the same request twice.
    """
    folder = Path.home() / ".claude" / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(ROOT))
    headless_f = RT / "headless_sessions.txt"
    headless = set(headless_f.read_text().split()) if headless_f.exists() else set()
    seen_p = RT / "imported_requests.json"
    seen = set(read_json(seen_p, []))
    new: dict[str, dict] = {}
    for f in sorted(folder.glob("*.jsonl")) if folder.exists() else []:
        if f.stem in headless:
            continue
        for line in f.open():
            try:
                d = json.loads(line)
            except json.JSONDecodeError:
                continue
            m = d.get("message") or {}
            if not isinstance(m, dict) or not m.get("usage"):
                continue
            rid = d.get("requestId") or d.get("uuid")
            if rid and rid not in seen:
                new[rid] = {"model": m.get("model", "-"), "u": m["usage"]}
    agg: dict[str, dict] = {}
    for x in new.values():
        g = agg.setdefault(x["model"], {"tokens_in": 0, "tokens_cache": 0, "tokens_out": 0, "calls": 0})
        u = x["u"]
        g["tokens_in"] += (u.get("input_tokens") or 0) + (u.get("cache_creation_input_tokens") or 0)
        g["tokens_cache"] += u.get("cache_read_input_tokens") or 0
        g["tokens_out"] += u.get("output_tokens") or 0
        g["calls"] += 1
    for model, g in agg.items():
        log(change=a.change or "-", phase=a.phase, role="orchestrator", agent="claude-interactive", model=model,
            tokens_in=g["tokens_in"], tokens_cache=g["tokens_cache"], tokens_out=g["tokens_out"],
            result="ok", note=f"session with the human · {g['calls']} requests")
    write_json(seen_p, sorted(seen | set(new)))
    print(f"Session imported: {len(new)} new requests across {len(agg)} model(s), phase {a.phase}.")


HELP = """gdd commands (run from the repository root):
  status                           Where we are: project, G0, changes, log.
  selftest                         Harness self-test 4/4 (Setup, G0).
  gate <G> <ok|ko> [change]        Record a gate: G0 (machines), zones, plan, G1, G3, G4. --by human|machines
  new-change <id> --title ...      Create a change and its branch change/<id>.
  classify <id>                    Color, lane and gates for a change.
  launch <id>                      Run the gdd-change workflow (Build -> Release).
  resume <id> --g4 ok|ko           Human G4: resume the paused workflow.
  snapshot [<id>]                 Records the change as it is now (the next round's Path Guard judges only that round).
  next [<id>]                      Where the replan sends the change: {"test": true} = back to test mode.
  note <id> <kind> <text>          Note for the Decision Inbox (needs|decided|parked).
  inbox <id>                       The change message for the human.
  report [--by change,model,role] [--change id]   Tokens by change, phase, model, role, agent.
  session --phase <phase> [--change id]          Log the tokens of the interactive session.
Internal (used by the workflow): prepare, g2, g3, guard, verdict, merge, release, park, ci."""


def main(argv=None) -> None:
    p = argparse.ArgumentParser(prog="gdd", description="GDD · Gate-Driven Development")
    s = p.add_subparsers(dest="cmd")
    s.add_parser("help").set_defaults(f=lambda a: print(HELP))
    s.add_parser("status").set_defaults(f=c_status)
    s.add_parser("selftest").set_defaults(f=c_selftest)
    x = s.add_parser("new-change"); x.add_argument("change"); x.add_argument("--title", required=True)
    x.add_argument("--type", default="Feature", choices=TYPES)
    x.add_argument("--paths", nargs="*"); x.add_argument("--resumable", action="store_true")
    x.add_argument("--min-color", choices=COLORS); x.set_defaults(f=c_new_change)
    for name, fn in (("classify", c_classify), ("prepare", c_prepare), ("verdict", c_verdict), ("inbox", c_inbox)):
        x = s.add_parser(name); x.add_argument("change", nargs="?"); x.set_defaults(f=fn)
    for name, fn in (("g2", c_g2), ("g3", c_g3), ("guard", c_guard)):
        x = s.add_parser(name); x.add_argument("change", nargs="?"); x.add_argument("--role", default="builder")
        x.add_argument("--strict", action="store_true"); x.add_argument("--since-snapshot", action="store_true")
        x.set_defaults(f=fn)
    x = s.add_parser("gate"); x.add_argument("gate", choices=["G0", "zones", "plan", "G1", "G2", "G3", "G4"])
    x.add_argument("decision"); x.add_argument("change", nargs="?"); x.add_argument("--by", default="human")
    x.add_argument("--note", default=""); x.set_defaults(f=c_gate)
    x = s.add_parser("snapshot"); x.add_argument("change", nargs="?"); x.set_defaults(f=c_snapshot)
    x = s.add_parser("next"); x.add_argument("change", nargs="?"); x.set_defaults(f=c_next)
    x = s.add_parser("note"); x.add_argument("change"); x.add_argument("kind", choices=["needs", "decided", "parked"])
    x.add_argument("text"); x.set_defaults(f=c_note)
    x = s.add_parser("park"); x.add_argument("reason"); x.add_argument("--change"); x.set_defaults(f=c_park)
    x = s.add_parser("merge"); x.add_argument("change", nargs="?"); x.set_defaults(f=c_merge)
    x = s.add_parser("release"); x.add_argument("step", choices=["pre", "promote", "observe"])
    x.add_argument("change", nargs="?"); x.set_defaults(f=c_release)
    x = s.add_parser("launch"); x.add_argument("change"); x.set_defaults(f=c_launch)
    x = s.add_parser("resume"); x.add_argument("change"); x.add_argument("--g4", required=True)
    x.add_argument("--note", default=""); x.set_defaults(f=c_resume)
    x = s.add_parser("report"); x.add_argument("--by", default="change,model,role"); x.add_argument("--change")
    x.add_argument("--json", action="store_true"); x.set_defaults(f=c_report)
    s.add_parser("ci").set_defaults(f=c_ci)
    x = s.add_parser("session"); x.add_argument("--phase", required=True, choices=PHASES)
    x.add_argument("--change"); x.set_defaults(f=c_session)
    a = p.parse_args(argv)
    if not getattr(a, "f", None):
        print(HELP); return
    a.f(a)


if __name__ == "__main__":
    main()
