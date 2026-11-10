#!/usr/bin/env python3
"""Check that the GDD skills keep to their contract and to their design.

Usage:
  python3 scripts/check_skills.py           check, and exit 1 if anything is off
  python3 scripts/check_skills.py --write   first copy every template of
                                            skills/CONTRACT.md into the assets
                                            of the skills that use it

What it checks:
  - every asset is identical to its template in skills/CONTRACT.md, and every
    skill ends with the report block of the contract;
  - each SKILL.md has a name equal to its folder, a description in the third
    person, a version and the contract gdd/1;
  - the sizes of decision 19 of skills/DESIGN.md: gdd-build about 200 words,
    the GDD block of AGENTS.md under 100, every other SKILL.md under 500 lines;
  - no skill ships scripts, and only gdd can be invoked implicitly in Codex;
  - every file a SKILL.md points to exists, shared references are identical,
    and no file has a long or medium dash.

Exit codes: 0 when everything passes, 1 when a check fails, 2 when the
repository can't be read. Each failure names the file and, when it can, the
line. It reads only skills/ and writes only with --write, into skills/*/assets.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ROOT / "skills"
CONTRACT = SKILLS / "CONTRACT.md"
ROLES = ["gdd-prepare", "gdd-brief", "gdd-checks", "gdd-build", "gdd-review", "gdd-harden"]
ALL = ["gdd"] + ROLES
SHARED = [("gdd-checks/references/scripts.md", "gdd-review/references/scripts.md")]
BUILD_WORDS = 230
BLOCK_WORDS = 100
MAX_LINES = 500

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
        import yaml  # optional: when it's installed, the front matter must parse as YAML
        yaml.safe_load(match.group(1))
    except ImportError:
        pass
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
        if re.search(r"\b(you|your)\b", description, re.I) or re.match(r"(Use|Load|Run)\b", description):
            problem(skill, "the description isn't in the third person", 3)
        version = fields.get("version")
        if not version:
            problem(skill, "metadata has no version")
        if fields.get("contract") != "gdd/1":
            problem(skill, "metadata doesn't say contract: gdd/1")

        block = report_block(body)
        expected = found.get("report", "").replace("<skill> <version>", f"{name} {version}")
        if name != "gdd":
            if block is None:
                problem(skill, "doesn't end with the gdd-report block")
            elif block != expected:
                problem(skill, "its gdd-report block differs from the one in skills/CONTRACT.md")

        lines = skill.read_text(encoding="utf-8").count("\n")
        if lines >= MAX_LINES:
            problem(skill, f"{lines} lines, the limit is {MAX_LINES}")
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

    for first, second in SHARED:
        a, b = SKILLS / first, SKILLS / second
        if a.is_file() and b.is_file() and a.read_bytes() != b.read_bytes():
            problem(b, f"differs from skills/{first}")

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
