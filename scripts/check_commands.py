#!/usr/bin/env python3
"""Check that the commands the GDD skills give do what the skills say.

Usage:
  python3 scripts/check_commands.py

It reads the commands from the skills themselves, builds throwaway
repositories outside this one, plays a yellow change and a green change in
them, and runs each command with its placeholders filled in. Every case is
something that slipped through once:

  - the color on what was touched counts what the builder did, not the brief,
    the scenarios or the sealed checks;
  - the seal check fails when a sealed check is weakened, and still fails when
    the builder moves the gdd-seal ref onto its own commit;
  - in a fresh clone, the seal check isn't fooled by git replace;
  - the seal check runs on a change with no seal, where <start> is the base;
  - the seal check doesn't trip on the orchestrator's bookkeeping, and the
    reviewer sees a log that came from the builder;
  - after another change ships, the change is brought up to date through its
    seal, and the seal check and the color still see only the builder's work;
  - excluding a zone written with :(glob) needs :(exclude,glob);
  - the search for decisions sees a brief that isn't tracked yet, the search
    for the tests of a scenario doesn't take another id that begins the same,
    and the counts of the logs don't depend on spaces in the JSON;
  - the zones tried on the history look at the window they say, find the
    commits of a zone, the ones no zone covers and the ones that fixed
    something, whatever the case of the word;
  - the project's runner passes the exit code on and says what a check took;
  - the project's counter reads the caps of the change's color from the
    blueprint's template, falls back to the caps of every color, leaves out
    the waits for a person, reads JSON with and without spaces, stops a change
    whose rounds or runs before the review ran out, never stops one for a
    slow tier, and refuses a missing log or a wrong id.

It needs git and grep and nothing else, uses no network and writes only in a
temporary folder. Exit codes: 0 when every case passes, 1 when one fails, 2
when a command can't be read from the skills or git can't run. Each failure
names its case. This check is for this repository: no project installs it.
"""

import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ROOT / "skills"
ZONES = ["':(glob)src/pay/**'", "':(glob)src/ui/**'"]
SEALED = "tests/t.test"

failures = []


def command(path, pattern):
    """Return the one command in a skill file that matches the pattern."""
    text = (SKILLS / path).read_text(encoding="utf-8")
    found = set(re.findall(pattern, text))
    if len(found) != 1:
        print(f"skills/{path}: expected one command like {pattern!r}, found {len(found)}", file=sys.stderr)
        sys.exit(2)
    return found.pop().strip()


def git(repo, *args, check=True):
    result = subprocess.run(
        ["git", "-c", "user.name=check", "-c", "user.email=check@example.com", "-c", "commit.gpgsign=false", *args],
        cwd=repo, capture_output=True, text=True,
    )
    if check and result.returncode != 0:
        print(f"git {' '.join(args)}: {result.stderr.strip()}", file=sys.stderr)
        sys.exit(2)
    return result


def run(repo, template, **values):
    """Run a command from a skill, with its placeholders filled in."""
    line = template
    for name, value in values.items():
        line = line.replace(f"<{name.replace('_', ' ')}>", value)
    if re.search(r"<[a-z ]+>", line):
        print(f"a placeholder was left in: {line}", file=sys.stderr)
        sys.exit(2)
    return subprocess.run(shlex.split(line), cwd=repo, capture_output=True, text=True)


def run_in_shell(repo, line):
    """Run a command that needs a shell for its glob or its pipe.

    Only for the counts of gdd-harden, whose text comes from this
    repository's own skill file and runs in a throwaway folder.
    """
    return subprocess.run(line, cwd=repo, capture_output=True, text=True, shell=True)


def expect(case, condition, detail=""):
    if not condition:
        failures.append(f"{case}{': ' + detail if detail else ''}")


def write(repo, path, text):
    target = repo / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8")


def commit(repo, message):
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", message)
    return head(repo)


def head(repo, ref="HEAD"):
    return git(repo, "rev-parse", ref).stdout.strip()


def new_repository(folder):
    repo = Path(folder)
    git(repo, "init", "-q", "-b", "main", ".")
    write(repo, "src/pay/a.js", "pay\n")
    write(repo, "src/ui/b.js", "ui\n")
    write(repo, "src/ui/c.js", "more ui\n")
    write(repo, "gdd/blueprint.md", "blueprint\n")
    write(repo, "gdd/backlog.md", "backlog\n")
    write(repo, "AGENTS.md", "agents\n")
    return repo, commit(repo, "base")


def sealed_change(repo, name, source):
    """Seal a brief, a scenario and a check, and build on a branch of its own."""
    git(repo, "checkout", "-q", "-b", f"gdd-seal/{name}", "main")
    write(repo, f"gdd/briefs/{name}.md", "brief\n")
    write(repo, f"features/{name}.feature", "scenario\n")
    write(repo, f"tests/{name}.test" if name != "x1" else SEALED, "assert strong\n")
    start = commit(repo, f"seal {name}")
    git(repo, "checkout", "-q", "-b", f"work/{name}")
    write(repo, source, f"{name}, changed\n")
    commit(repo, f"build {name}")
    return start


def counter_cases(shop):
    """The project's counter, on the blueprint's own template and sample logs."""
    counter = SKILLS / "gdd-prepare/assets/limits.sh"
    template = (SKILLS / "gdd-prepare/assets/blueprint.md").read_text(encoding="utf-8")
    late = "  - id: e2e\n    type: machine\n    command: sh e2e.sh\n    runs: before review\n  - id: review\n"
    if template.count("  - id: review\n") != 1:
        failures.append("the blueprint's template no longer has one review check to put a slow check before")
        return
    blueprint = template.replace("  - id: review\n", late)
    old = ("---\ncontract: gdd/1\nlimits:\n  fix_rounds_per_gate: 2\n  rounds_per_change: 6\n"
           "  minutes_per_change: 120  # without the time it waits for a person\n---\n")

    def ev(minute, rest, spaced=False):
        line = '{"contract":"gdd/1","ts":"2026-11-10T%02d:%02d:00Z","brief":"x",%s}' % (10 + minute // 60, minute % 60, rest)
        return line.replace('":"', '": "').replace('","', '", "').replace(',"', ', "') if spaced else line

    brief = '"event":"step","step":"brief","report":{"skill":"gdd-brief","result":"yellow","tokens":10}'
    ask = '"event":"ask","of":"decision","what":"a rounding"'
    approval = '"event":"approval","who":"Mar","of":"brief"'
    checks = '"event":"step","step":"checks","report":{"result":"sealed"}'
    build = '"event":"step","step":"build","report":{"result":"done"}'
    gates_ok = '"event":"step","step":"gates","gates":[{"check":"seal","seconds":1,"result":"pass"},{"check":"tests","seconds":20,"result":"pass"}]'
    gates_slow = '"event":"step","step":"gates","gates":[{"check":"tests","seconds":130,"tail":"the \\"check\\":\\"x\\" text","result":"pass"},{"check":"e2e","seconds":30,"result":"pass"}]'
    gates_late = '"event":"step","step":"gates","gates":[{"check":"tests","seconds":5,"result":"pass"},{"check":"e2e","seconds":40,"result":"pass"}]'

    def count(plan, lines, *args):
        with tempfile.TemporaryDirectory() as folder:
            write(Path(folder), "gdd/blueprint.md", plan)
            if lines is not None:
                write(Path(folder), "gdd/log/x.jsonl", "\n".join(lines) + "\n")
            result = subprocess.run(["sh", str(counter), *(args or ("x",))], cwd=folder, capture_output=True, text=True)
            last = result.stdout.strip().splitlines()[-1] if result.stdout.strip() else ""
            return result, last

    # Waiting for a person doesn't count: 80 minutes from the first event to the open ask, 60 of them waiting.
    waited = [ev(0, brief), ev(5, ask), ev(65, approval), ev(70, checks), ev(75, build), ev(80, ask)]
    result, last = count(blueprint, waited)
    expect("the counter reads the caps of the change's color from the blueprint's template",
           "color yellow" in result.stdout and "rounds 0 of 2" in result.stdout and "minutes 20 of 30" in result.stdout, result.stdout)
    expect("the counter lets a change with room go on", result.returncode == 0 and last.startswith('gdd-limits {"color":"yellow","stop":[]'), last)
    result, _ = count(blueprint, [ev(0, brief, True), ev(5, ask, True), ev(65, approval, True), ev(70, checks, True), ev(75, build, True), ev(80, ask, True)])
    expect("the counter reads JSON with spaces the same", "minutes 20 of 30" in result.stdout and "rounds 0 of 2" in result.stdout, result.stdout)

    # Two fix rounds of a yellow change use up its rounds, and the checks sent it back past its cap.
    rounds = [ev(0, brief), ev(1, checks), ev(2, build), ev(3, gates_ok.replace('"pass"}]', '"fail"}]')),
              ev(4, build), ev(5, gates_ok.replace('"pass"}]', '"fail"}]')), ev(6, build), ev(7, ask)]
    result, last = count(blueprint, rounds)
    expect("the counter stops a change whose rounds ran out", result.returncode == 1 and '"rounds"' in last and '"sent back by the checks"' in last, result.stdout)

    # A tier past its seconds stops nothing; the runs before the review do, once used up.
    result, last = count(blueprint, [ev(0, brief), ev(1, checks), ev(2, build), ev(3, gates_slow), ev(4, ask)])
    expect("a slow tier goes to the backlog and stops nothing",
           result.returncode == 0 and '"slow":["seconds of the checks of a round"]' in last and "130 of 120" in result.stdout, result.stdout)
    result, last = count(blueprint, [ev(0, brief), ev(1, checks), ev(2, build), ev(3, gates_late), ev(4, gates_late), ev(5, ask)])
    expect("the counter stops a change whose runs before the review ran out",
           result.returncode == 1 and '"runs before the review"' in last and "runs before the review 2 of 2" in result.stdout, result.stdout)

    # A blueprint from before 0.10: the caps right under limits hold for every color.
    result, last = count(old, [ev(0, brief), ev(1, checks), ev(2, build), ev(3, build), ev(4, ask)])
    expect("an old blueprint's caps hold for every color", result.returncode == 0 and "rounds 1 of 6" in result.stdout and "minutes 4 of 120" in result.stdout, result.stdout)

    result, _ = count(blueprint, None)
    expect("the counter refuses a missing log", result.returncode == 2 and "missing" in result.stderr, result.stderr)
    result, _ = count(blueprint, waited, "X y")
    expect("the counter refuses a wrong id", result.returncode == 2, result.stderr)
    result, _ = count(blueprint, waited, "x", "tests")
    expect("the counter says how it is called when it is called wrong", result.returncode == 2 and "usage" in result.stderr, result.stderr)

    carried = re.search(r"cat > gdd/limits\.sh <<'EOF'\n(.*?)EOF\n", shop, re.S)
    expect("the shop of the eval cases carries the counter as the template gives it",
           carried is not None and carried.group(1) == counter.read_text(encoding="utf-8"))


def main():
    if "--help" in sys.argv or "-h" in sys.argv:
        print(__doc__.strip())
        return 0

    color = command("gdd-review/SKILL.md", r"git diff --name-only <start> HEAD -- <pathspec>")
    sealed_only = command("gdd-review/SKILL.md", r"git diff --name-only <base> <start>")
    bookkeeping = command("gdd-review/SKILL.md", r"git diff --name-only <start> HEAD -- gdd/log gdd/backlog\.md")
    exclude = command("gdd-review/SKILL.md", r"':\(exclude,glob\)<pattern>'")
    seal = command("gdd-prepare/assets/blueprint.md", r"git diff --exit-code <start> HEAD -- [^\"\n]+")
    nothing = command("gdd/SKILL.md", r"git diff --stat <start>")
    decisions = command("gdd-brief/SKILL.md", r"git grep --untracked -n \"\\\[DECISION:\" -- gdd/briefs features")
    cited = command("CONTRACT.md", r"git grep -nE \"gdd:<id>\(\[\^a-z0-9-\]\|\$\)\"")
    counts = re.findall(r"^- `(grep [^`]+)`", (SKILLS / "gdd-harden/SKILL.md").read_text(encoding="utf-8"), re.M)
    if len(counts) != 4:
        print(f"skills/gdd-harden/SKILL.md: expected four grep commands, found {len(counts)}", file=sys.stderr)
        return 2
    excluded = " ".join(exclude.replace("<pattern>", zone.strip("'").replace(":(glob)", "")) for zone in ZONES)
    uncovered = f". {excluded}"

    with tempfile.TemporaryDirectory() as folder:
        repo, base = new_repository(folder)
        start = sealed_change(repo, "x1", "src/ui/b.js")

        listed = run(repo, color, start=start, pathspec=ZONES[1]).stdout.split()
        expect("the color sees what the builder touched", listed == ["src/ui/b.js"], f"got {listed}")
        left = run(repo, color, start=start, pathspec=uncovered).stdout.split()
        expect("the brief, the scenarios and the sealed checks don't count for the color", left == [], f"got {left}")
        control = run(repo, color.replace("<start>", "<base>"), base=base, pathspec=uncovered).stdout.split()
        expect("control: the same count from the base does see them", len(control) == 3, f"got {control}")
        wrong = excluded.replace("(exclude,glob)", "(exclude):(glob)")
        leaked = run(repo, color, start=start, pathspec=f". {wrong}").stdout.split()
        expect("control: :(exclude) in front of :(glob) excludes nothing", "src/ui/b.js" in leaked, f"got {leaked}")

        only = run(repo, sealed_only, base=base, start=start).stdout.split()
        expect("the seal holds only the brief, the scenarios and the checks",
               sorted(only) == ["features/x1.feature", "gdd/briefs/x1.md", SEALED], f"got {only}")

        result = run(repo, seal, start=start, sealed_paths=SEALED)
        expect("the seal check passes when nothing that judges was touched", result.returncode == 0, result.stdout[:200])
        result = run(repo, bookkeeping, start=start)
        expect("before shipping, the change carries no log and no backlog", result.stdout == "", result.stdout[:200])
        built = head(repo)

        # Another change ships to the main branch in the meantime.
        sealed_change(repo, "y2", "src/pay/a.js")
        git(repo, "checkout", "-q", "main")
        git(repo, "merge", "-q", "--no-ff", "-m", "ship y2", "work/y2")
        git(repo, "checkout", "-q", "-b", "control/x1", built)
        git(repo, "merge", "-q", "-m", "main, merged straight into the change", "main")
        result = run(repo, seal, start=start, sealed_paths=SEALED)
        expect("control: with the main branch merged straight into the change, the seal check trips", result.returncode == 1)
        git(repo, "checkout", "-q", "gdd-seal/x1")
        git(repo, "merge", "-q", "-m", "the main branch, into the seal", "main")
        moved = head(repo)
        git(repo, "checkout", "-q", "work/x1")
        git(repo, "merge", "-q", "-m", "the seal's new tip, into the change", "gdd-seal/x1")
        result = run(repo, seal, start=moved, sealed_paths=SEALED)
        expect("brought up to date through its seal, the seal check passes", result.returncode == 0, result.stdout[:200])
        listed = run(repo, color, start=moved, pathspec=".").stdout.split()
        expect("brought up to date through its seal, the color sees only the builder's work", listed == ["src/ui/b.js"], f"got {listed}")

        # The orchestrator's bookkeeping goes on top when the change is about to ship.
        write(repo, "gdd/log/x1.jsonl", "{}\n")
        write(repo, "gdd/backlog.md", "backlog, x1 briefed\n")
        shipped = commit(repo, "bookkeeping")
        result = run(repo, seal, start=moved, sealed_paths=SEALED)
        expect("the seal check ignores the orchestrator's bookkeeping", result.returncode == 0, result.stdout[:200])
        result = run(repo, bookkeeping, start=moved)
        expect("the reviewer sees a log or a backlog that is on the change's branch", "gdd/log/x1.jsonl" in result.stdout)

        # The builder weakens a sealed check, then tries to hide it.
        write(repo, SEALED, "assert weak\n")
        weak = commit(repo, "a weakened check")
        result = run(repo, seal, start=moved, sealed_paths=SEALED)
        expect("the seal check fails on a weakened check", result.returncode == 1, f"exit {result.returncode}")
        git(repo, "branch", "-f", "gdd-seal/x1", "HEAD")
        result = run(repo, seal, start=moved, sealed_paths=SEALED)
        expect("the seal check still fails when the ref was moved", result.returncode == 1, f"exit {result.returncode}")
        by_name = run(repo, seal.replace("<start>", "gdd-seal/x1"), sealed_paths=SEALED)
        expect("control: compared with the ref's name, the moved seal passes", by_name.returncode == 0, f"exit {by_name.returncode}")
        git(repo, "replace", moved, weak)
        result = run(repo, seal, start=moved, sealed_paths=SEALED)
        expect("control: in the builder's repository, git replace fools the seal check", result.returncode == 0, f"exit {result.returncode}")
        with tempfile.TemporaryDirectory() as second:
            clone = Path(second) / "clone"
            git(repo, "clone", "-q", "--branch", "work/x1", str(repo), str(clone))
            result = run(clone, seal, start=moved, sealed_paths=SEALED)
            expect("in a fresh clone, git replace doesn't fool the seal check", result.returncode == 1, f"exit {result.returncode}")
            sealed_hash = git(repo, "--no-replace-objects", "rev-parse", f"{moved}:{SEALED}").stdout.strip()
            found_hash = git(clone, "hash-object", SEALED).stdout.strip()
            expect("the blob hash of a weakened check differs from the sealed one", sealed_hash != found_hash)
        git(repo, "replace", "-d", moved)

        git(repo, "reset", "-q", "--hard", shipped)
        write(repo, "AGENTS.md", "agents, edited\n")
        commit(repo, "the agents' own file, edited")
        result = run(repo, seal, start=moved, sealed_paths=SEALED)
        expect("the seal check fails when the agents' own files change", result.returncode == 1, f"exit {result.returncode}")

    with tempfile.TemporaryDirectory() as folder:
        # A green change: nothing is sealed, so <start> is the base.
        repo, base = new_repository(folder)
        git(repo, "checkout", "-q", "-b", "work/g1")
        result = run(repo, nothing, start=base)
        status = git(repo, "status", "--porcelain").stdout
        expect("a build that changed nothing shows nothing", result.stdout == "" and status == "", result.stdout[:200])
        write(repo, "src/ui/b.js", "ui, typo fixed\n")
        built = commit(repo, "a typo")
        result = run(repo, seal, start=base, sealed_paths="")
        expect("the seal check runs on a change with no seal", result.returncode == 0, f"exit {result.returncode}: {result.stderr[:200]}")

        sealed_change(repo, "y2", "src/pay/a.js")
        git(repo, "checkout", "-q", "main")
        git(repo, "merge", "-q", "--no-ff", "-m", "ship y2", "work/y2")
        tip = head(repo)
        git(repo, "checkout", "-q", "work/g1")
        git(repo, "merge", "-q", "-m", "the main branch, into the change", "main")
        result = run(repo, seal, start=tip, sealed_paths="")
        expect("with nothing sealed, the tip of the main branch is the new start", result.returncode == 0, result.stdout[:200])
        control = run(repo, seal, start=base, sealed_paths="")
        expect("control: against the old base, the seal check trips on the other change", control.returncode == 1)

        write(repo, "gdd/blueprint.md", "blueprint, loosened\n")
        commit(repo, "the blueprint, edited")
        result = run(repo, seal, start=tip, sealed_paths="")
        expect("the seal check fails when a green change edits the blueprint", result.returncode == 1, f"exit {result.returncode}")

        # The search for decisions, on a brief that isn't tracked yet.
        write(repo, "gdd/briefs/new.md", "Rounding: [DECISION: per line or on the total?]\n")
        result = run(repo, decisions)
        expect("the search for decisions sees an untracked brief", "gdd/briefs/new.md" in result.stdout, result.stdout[:200])

        # The tests of a scenario, when another id begins the same.
        write(repo, "tests/ids.test", "gdd:pay-1 one\ngdd:pay-1-click-3 another\ngdd:pay-10 a third\n")
        git(repo, "add", "tests/ids.test")
        found = run(repo, cited, id="pay-1").stdout.strip().splitlines()
        expect("the search for a scenario's tests finds only its own", len(found) == 1 and "one" in found[0], f"got {found}")

        # The counts of the logs, with and without spaces in the JSON.
        write(repo, "gdd/log/a.jsonl", '{"event":"ask"}\n{"verdict":"fix","closed":[{"outcome":"false"}]}\n')
        write(repo, "gdd/log/b.jsonl", '{"event": "ask"}\n{"verdict": "fix", "closed": [{"outcome": "minor"}]}\n')
        write(repo, "gdd/log/c.jsonl", '{"step":"gates","gates":[{"check":"tests","seconds":4,"command":"x","exit":0,"tail":"a } b","result":"pass"},{"check": "lint", "seconds": 12, "command": "y"},{"check":"fast","seconds":0.4},{"check":"old","command":"z"},{"check":"ci","seconds":null}]}\n')
        asks, fixes, outcomes, took = (run_in_shell(repo, line).stdout for line in counts)
        expect("what each check took is read with and without spaces, and only where it was measured",
               sorted(re.findall(r"[0-9.]+", took)) == ["0.4", "12", "4"] and "old" not in took and "ci" not in took, took)
        expect("the count of asks doesn't depend on spaces", asks.count(":1") == 2, asks)
        expect("the list of changes sent back doesn't depend on spaces", len(fixes.split()) == 2, fixes)
        expect("the count of outcomes doesn't depend on spaces", len(outcomes.strip().splitlines()) == 2, outcomes)

    # The zones, tried on the history: gdd-prepare runs these before the person approves.
    history = "gdd-prepare/references/existing-project.md"
    since_cmd = command(history, r"git rev-list --no-merges --skip=200 -n 1 <main branch>")
    count_cmd = command(history, r"git rev-list --no-merges --count <since>\.\.<main branch>")
    zone_cmd = command(history, r"git rev-list --no-merges <since>\.\.<main branch> -- <pathspec>")
    uncovered_cmd = command(history, r"git rev-list --no-merges <since>\.\.<main branch> -- \. ':\(exclude,glob\)<pattern>'")
    fixes_cmd = command(history, r"git rev-list --no-merges -i --grep=fix --grep=revert <since>\.\.<main branch>")
    with tempfile.TemporaryDirectory() as folder:
        repo, base = new_repository(folder)
        write(repo, "src/pay/a.js", "pay, fixed\n")
        c1 = commit(repo, "Fix the total")
        write(repo, "src/ui/b.js", "ui, changed\n")
        c2 = commit(repo, "Change the button")
        write(repo, "tools/x.py", "a tool\n")
        c3 = commit(repo, "Add a tool")
        write(repo, "src/ui/c.js", "more ui, back\n")
        c4 = commit(repo, "revert the banner")
        outside = uncovered_cmd.replace("':(exclude,glob)<pattern>'", excluded)

        short = run(repo, since_cmd, main_branch="main").stdout.strip()
        expect("a history shorter than the window gives no <since>", short == "", f"got {short}")
        whole = lambda line: line.replace("<since>..", "")
        total = run(repo, whole(count_cmd), main_branch="main").stdout.strip()
        expect("without <since>, the count is the whole history", total == "5", f"got {total}")
        fixed = run(repo, whole(fixes_cmd), main_branch="main").stdout.split()
        expect("the fixes are found whatever the case of the word", sorted(fixed) == sorted([c1, c4]), f"got {fixed}")

        since = run(repo, since_cmd.replace("--skip=200", "--skip=3"), main_branch="main").stdout.strip()
        expect("<since> is the commit just before the window", since == c1, f"got {since}")
        total = run(repo, count_cmd, since=since, main_branch="main").stdout.strip()
        expect("the count is the window's", total == "3", f"got {total}")
        ui = run(repo, zone_cmd, since=since, main_branch="main", pathspec=ZONES[1]).stdout.split()
        expect("a zone finds its commits in the window", sorted(ui) == sorted([c2, c4]), f"got {ui}")
        pay = run(repo, zone_cmd, since=since, main_branch="main", pathspec=ZONES[0]).stdout.split()
        expect("a zone nobody touched in the window finds none", pay == [], f"got {pay}")
        nozone = run(repo, outside, since=since, main_branch="main").stdout.split()
        expect("what no zone covers is found, and only that", nozone == [c3], f"got {nozone}")
        fixed = run(repo, fixes_cmd, since=since, main_branch="main").stdout.split()
        expect("the fixes are the window's", fixed == [c4], f"got {fixed}")

    # The project's runner: it passes the exit code on and ends with what the check took.
    runner = SKILLS / "gdd-prepare/assets/check.sh"
    with tempfile.TemporaryDirectory() as folder:
        def check(*args):
            return subprocess.run(["sh", str(runner), *args], cwd=folder, capture_output=True, text=True)

        passed = check("tests", "--", "sh", "-c", "echo one two; exit 0")
        last = passed.stdout.strip().splitlines()[-1] if passed.stdout.strip() else ""
        expect("the runner ends with what a passing check took",
               passed.returncode == 0 and re.fullmatch(r'gdd-check \{"check":"tests","seconds":\d+,"exit":0\}', last) is not None
               and "one two" in passed.stdout, passed.stdout)
        failed = check("lint", "--", "sh", "-c", "exit 3")
        expect("the runner passes a failing exit code on, and says it",
               failed.returncode == 3 and failed.stdout.strip().endswith('"exit":3}'), failed.stdout)
        slow = check("slow", "--", "sleep", "1")
        took = re.search(r'"seconds":(\d+)', slow.stdout)
        expect("the runner counts the seconds", took is not None and int(took.group(1)) >= 1, slow.stdout)
        wrong = check("tests")
        expect("the runner says how it is called when it is called wrong", wrong.returncode == 2 and "usage" in wrong.stderr, wrong.stderr)
    shop = (ROOT / "evals/fixtures/shop.sh").read_text(encoding="utf-8")
    carried = re.search(r"cat > gdd/check\.sh <<'EOF'\n(.*?)EOF\n", shop, re.S)
    expect("the shop of the eval cases carries the runner as the template gives it",
           carried is not None and carried.group(1) == runner.read_text(encoding="utf-8"))

    counter_cases(shop)

    for failure in failures:
        print(f"scripts/check_commands.py: {failure}")
    if failures:
        print(f"{len(failures)} case(s) failed", file=sys.stderr)
        return 1
    print("commands: every command the skills give does what they say")
    return 0


if __name__ == "__main__":
    sys.exit(main())
