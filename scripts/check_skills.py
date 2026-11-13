#!/usr/bin/env python3
"""Check that the GDD skills keep to their contract and to their design.

Usage:
  python3 scripts/check_skills.py           check, and exit 1 if anything is off
  python3 scripts/check_skills.py --write   first copy every template of
                                            skills/CONTRACT.md into the assets
                                            of the skills that use it
  uv run --with pyyaml python3 scripts/check_skills.py
                                            the same check, where PyYAML isn't
                                            installed

What it checks:
  - every asset is identical to its template in skills/CONTRACT.md, the three
    roles that run as their own agent end with the report block of the
    contract, and the skills the orchestrator runs itself carry none;
  - the command of the seal check is the same in the contract, in the
    blueprint's template and in gdd-review;
  - each SKILL.md has front matter that parses as YAML (this needs PyYAML, and
    without it the check fails instead of passing unchecked), a name equal to
    its folder, a description in the third person, the MIT license, a version
    and the contract gdd/1;
  - the sizes of decision 19 of skills/DESIGN.md: gdd-build about 200 words,
    the GDD block of AGENTS.md under 100, every SKILL.md under 500 lines and
    14,500 bytes, and under 400 characters of description for every skill
    but gdd, the only one that is found by its description;
  - no skill ships scripts, but the templates of the project's runner and
    counter in the assets of gdd-prepare, and only gdd can be invoked
    implicitly in Codex;
  - the GDD block the eval cases give their agents is the one of the contract;
  - what the contract names reaches the skills, and the other way round: every
    file of the contract, every check of the blueprint and every event and
    step of the log is named in some SKILL.md, the orchestrator knows the
    markers the roles answer with, and no skill names a gdd/ path the
    contract doesn't have;
  - every file a SKILL.md points to exists, shared references are identical,
    the reviewer asks a brief the same seven questions gdd-brief answers, and
    no file has a long or medium dash.

Exit codes: 0 when everything passes, 1 when a check fails, 2 when the
repository can't be read. Each failure names the file and, when it can, the
line. It reads only skills/ and writes only with --write, into skills/*/assets.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ROOT / "skills"
EVALS = ROOT / "evals"
CONTRACT = SKILLS / "CONTRACT.md"
ROLES = ["gdd-prepare", "gdd-brief", "gdd-checks", "gdd-build", "gdd-review", "gdd-harden"]
ALL = ["gdd"] + ROLES
LAUNCHED = ["gdd-checks", "gdd-build", "gdd-review"]
SHARED = [
    ("gdd-checks/references/scripts.md", "gdd-review/references/scripts.md"),
    ("gdd-prepare/references/web.md", "gdd-checks/references/web.md"),
]
PROJECT_SCRIPTS = {"gdd-prepare/assets/check.sh", "gdd-prepare/assets/limits.sh"}  # templates the project copies and owns: the skills never run them
SCRIPTS = {".sh", ".bash", ".zsh", ".py", ".js", ".mjs", ".cjs", ".ts", ".rb", ".pl", ".ps1"}
BUILD_WORDS = 230
BLOCK_WORDS = 100
MAX_LINES = 500
MAX_BYTES = 14_500  # an agent keeps the first 20,000 characters of a skill after a compaction: this leaves room
DESCRIPTION_CHARS = 400
SEAL = re.compile(r"git diff --exit-code <start> HEAD -- [^`\n\"]+")
NO_YAML = (
    "scripts/check_skills.py: PyYAML is missing, so no front matter was checked as YAML. "
    "Run it as: uv run --with pyyaml python3 scripts/check_skills.py"
)

problems = []


def problem(path, message, line=None):
    where = str(path.relative_to(ROOT)) if isinstance(path, Path) else path
    if line:
        where += f":{line}"
    problems.append(f"{where}: {message}")


def templates():
    """Return {destination: text} from the template markers in CONTRACT.md."""
    text = CONTRACT.read_text(encoding="utf-8")
    found = {}
    pattern = re.compile(r"<!-- template: ([^>]+?) -->\n(`{3,})[^\n]*\n(.*?)\n\2\n", re.S)
    for match in pattern.finditer(text):
        for destination in match.group(1).split():
            found[destination] = match.group(3) + "\n"
    if not found:
        problem(CONTRACT, "no templates found")
    return found


def front_matter(path):
    text = path.read_text(encoding="utf-8")
    match = re.match(r"---\n(.*?)\n---\n(.*)", text, re.S)
    if not match:
        problem(path, "no YAML front matter", 1)
        return {}, text
    try:
        import yaml
    except ImportError:  # a check that can't run must fail, so this one isn't skipped
        if NO_YAML not in problems:
            problems.append(NO_YAML)
    else:
        try:
            yaml.safe_load(match.group(1))
        except Exception as error:  # a YAML error, which agents would hit too
            problem(path, f"the front matter isn't valid YAML: {str(error).splitlines()[0]}", 1)
    fields = {}
    for line in match.group(1).splitlines():
        item = re.match(r"\s*([a-z_-]+):\s*(.*)", line)
        if item:
            fields[item.group(1)] = item.group(2).strip().strip('"')
    return fields, match.group(2)


def report_block(body):
    match = re.search(r"```gdd-report\n(.*?)\n```", body, re.S)
    return match.group(1) + "\n" if match else None


def words(text):
    return len(re.findall(r"[\w'`./<>@-]+", text))


def main():
    if "--help" in sys.argv or "-h" in sys.argv:
        print(__doc__.strip())
        return 0
    if not CONTRACT.is_file():
        print(f"cannot read {CONTRACT}", file=sys.stderr)
        return 2

    found = templates()
    if "--write" in sys.argv:
        for destination, text in found.items():
            if destination == "report":
                continue
            target = SKILLS / destination
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8")

    for destination, text in found.items():
        if destination == "report":
            continue
        target = SKILLS / destination
        if not target.is_file():
            problem(target, "missing: its template is in skills/CONTRACT.md")
        elif target.read_text(encoding="utf-8") != text:
            problem(target, "differs from its template in skills/CONTRACT.md")

    for name in ALL:
        folder = SKILLS / name
        skill = folder / "SKILL.md"
        if not skill.is_file():
            problem(skill, "missing")
            continue
        fields, body = front_matter(skill)
        if fields.get("name") != name:
            problem(skill, f"name is {fields.get('name')!r}, not {name!r}", 2)
        description = fields.get("description", "")
        if not description or len(description) > 1024:
            problem(skill, "the description is missing or longer than 1024 characters", 3)
        if name != "gdd" and len(description) > DESCRIPTION_CHARS:
            problem(skill, f"a description of {len(description)} characters, the limit is {DESCRIPTION_CHARS}", 3)
        if re.search(r"\b(you|your)\b", description, re.I) or re.match(r"(Use|Load|Run)\b", description):
            problem(skill, "the description isn't in the third person", 3)
        if fields.get("license") != "MIT":
            problem(skill, "the license is missing or isn't MIT", 1)
        version = fields.get("version")
        if not version:
            problem(skill, "metadata has no version")
        if fields.get("contract") != "gdd/1":
            problem(skill, "metadata doesn't say contract: gdd/1")

        block = report_block(body)
        expected = found.get("report", "").replace("<skill> <version>", f"{name} {version}")
        if name in LAUNCHED:
            if block is None:
                problem(skill, "doesn't end with the gdd-report block")
            elif block != expected:
                problem(skill, "its gdd-report block differs from the one in skills/CONTRACT.md")
        elif block is not None:
            problem(skill, "has a gdd-report block, and the orchestrator runs this skill itself")

        lines = skill.read_text(encoding="utf-8").count("\n")
        if lines >= MAX_LINES:
            problem(skill, f"{lines} lines, the limit is {MAX_LINES}")
        size = skill.stat().st_size
        if size > MAX_BYTES:
            problem(skill, f"{size} bytes, the limit is {MAX_BYTES}")
        if name == "gdd-build":
            prose = re.sub(r"```gdd-report\n.*?\n```", "", body, flags=re.S)
            count = words(prose)
            if count > BUILD_WORDS:
                problem(skill, f"{count} words, about 200 is the target and {BUILD_WORDS} the limit")

        for referenced in re.findall(r"`((?:references|assets)/[^`\s]+)`", body):
            if not (folder / referenced).exists():
                problem(skill, f"points to {referenced}, which doesn't exist")

        if (folder / "scripts").exists():
            problem(folder / "scripts", "skills ship no scripts")
        for script in sorted(folder.rglob("*")):
            if script.is_file() and script.suffix in SCRIPTS and script.relative_to(SKILLS).as_posix() not in PROJECT_SCRIPTS:
                problem(script, "skills ship no scripts: the only ones are the templates of the project's runner and counter")
        policy = folder / "agents" / "openai.yaml"
        wanted = "true" if name == "gdd" else "false"
        if not policy.is_file():
            problem(policy, "missing")
        elif not re.search(rf"allow_implicit_invocation:\s*{wanted}\b", policy.read_text(encoding="utf-8")):
            problem(policy, f"allow_implicit_invocation must be {wanted}")

    block = found.get("gdd-prepare/assets/agents-block.md", "")
    count = words(re.sub(r"<!--.*?-->", "", block))
    if count >= BLOCK_WORDS:
        problem("skills/gdd-prepare/assets/agents-block.md", f"{count} words, the limit is {BLOCK_WORDS}")

    commands = {}
    for path in (CONTRACT, SKILLS / "gdd-prepare/assets/blueprint.md", SKILLS / "gdd-review/SKILL.md"):
        if path.is_file():
            commands[path] = {match.group(0).strip() for match in SEAL.finditer(path.read_text(encoding="utf-8"))}
    wanted = commands.get(CONTRACT, set())
    if len(wanted) != 1:
        problem(CONTRACT, f"gives {len(wanted)} commands for the seal check, and there must be one")
    for path, got in commands.items():
        if got != wanted:
            problem(path, "its command for the seal check differs from the one in skills/CONTRACT.md")

    block_lines = [line.strip() for line in re.sub(r"<!--.*?-->", "", block).splitlines() if line.strip()]
    carriers = sorted(EVALS.rglob("case.yaml")) + [EVALS / "fixtures" / "shop.sh"] if EVALS.is_dir() else []
    for path in carriers:
        if not path.is_file() or "## GDD" not in path.read_text(encoding="utf-8"):
            continue  # a case about a project without GDD carries no block
        text = [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]
        missing = [line for line in block_lines if line not in text]
        if missing:
            problem(path, f"its GDD block differs from the one in skills/CONTRACT.md: {missing[0][:60]!r} is missing")

    contract = CONTRACT.read_text(encoding="utf-8")
    installed = {}
    for name in ALL:
        folder = SKILLS / name
        installed[name] = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted(folder.rglob("*"))
            if path.is_file() and path.suffix in {".md", ".feature", ".jsonl"}
        )
    everything = "\n".join(installed.values())
    files = sorted(set(re.findall(r"^\| `((?:gdd|features)/[^`<]*)", contract, re.M)))
    blueprint = found.get("gdd-prepare/assets/blueprint.md", "")
    checks = sorted(set(re.findall(r"^  - id: ([a-z-]+)$", blueprint, re.M)))
    log = found.get("gdd/assets/log-events.jsonl", "")
    steps = sorted(set(re.findall(r'"step":"([a-z]+)"', log)))
    events = sorted(set(re.findall(r'"event":"([a-z]+)"', log)) - {"step"})
    prose = "\n".join((SKILLS / name / "SKILL.md").read_text(encoding="utf-8") for name in ALL)
    examples = {"tests", "lint"}  # checks of the template that stand for a project's own, not the method's
    for kind, names in (("file", files), ("check", [c for c in checks if c not in examples])):
        for item in names:
            if f"`{item}" not in prose:
                problem(CONTRACT, f"the {kind} `{item}` of the contract is named in no SKILL.md")
    for kind, names in (("step", steps), ("event", events)):
        for item in names:
            if not re.search(rf"\b{item}\b", prose, re.I):
                problem(CONTRACT, f"the {kind} `{item}` of the log is named in no SKILL.md")
    orchestrator = installed.get("gdd", "")
    for marker in ("CANNOT:", "COLOR:", "gdd-report", "`pass`", "`fix`", "`stop`"):
        if marker not in orchestrator:
            problem(SKILLS / "gdd" / "SKILL.md", f"doesn't know the marker {marker}, which the roles answer with")
    for name in ALL:
        for path in sorted(set(re.findall(r"`(gdd/[a-z]+(?:\.md)?)[/`]", installed[name]))):
            if path not in contract:
                problem(SKILLS / name / "SKILL.md", f"names `{path}`, which the contract doesn't have")

    for first, second in SHARED:
        a, b = SKILLS / first, SKILLS / second
        if a.is_file() and b.is_file() and a.read_bytes() != b.read_bytes():
            problem(b, f"differs from skills/{first}")

    questions = re.compile(r"^1\. \*\*The product's rules\.\*\*.*?^7\. [^\n]*$", re.S | re.M)
    asked, reading = SKILLS / "gdd-brief/SKILL.md", SKILLS / "gdd-review/references/brief.md"
    if asked.is_file() and reading.is_file():
        mine = questions.search(asked.read_text(encoding="utf-8"))
        theirs = questions.search(reading.read_text(encoding="utf-8"))
        if not mine or not theirs:
            problem(reading if mine else asked, "has no list of seven questions")
        elif mine.group(0) != theirs.group(0):
            problem(reading, "its seven questions differ from the ones of skills/gdd-brief/SKILL.md")

    for path in sorted(SKILLS.rglob("*")):
        if path.is_file() and path.suffix in {".md", ".feature", ".yaml", ".jsonl", ".json"}:
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                if "—" in line or "–" in line:
                    problem(path, "has a long or medium dash", number)

    for line in problems:
        print(line)
    if problems:
        print(f"{len(problems)} problem(s)", file=sys.stderr)
        return 1
    print("skills: everything in line with skills/CONTRACT.md and skills/DESIGN.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
