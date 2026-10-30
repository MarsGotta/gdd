"""Check Markdown docs: relative links and GitHub anchors resolve; no em dashes in docs/.

    python3 check_md_links.py [REPO_ROOT]

External links are not fetched. Code blocks and inline code are ignored. Exit 1 on any
problem, so CI can run it.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()
SKIP = {".git", ".gdd", "target", "build", ".venv", "node_modules", "dist", ".claude", ".agents", ".opencode"}
LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)|<img[^>]+src=\"([^\"]+)\"")
IMAGE = re.compile(r"!\[[^\]]*\]\(([^)\s]+)\)")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
FENCE = re.compile(r"^\s*(```|~~~)")


def files() -> list[Path]:
    return sorted(
        p for p in ROOT.rglob("*.md") if not any(x in SKIP for x in p.relative_to(ROOT).parts)
    )


def strip_code(text: str) -> str:
    out, fenced = [], False
    for line in text.splitlines():
        if FENCE.match(line):
            fenced = not fenced
            out.append("")
            continue
        out.append("" if fenced else re.sub(r"`[^`]*`", "", line))
    return "\n".join(out)


def slug(heading: str) -> str:
    text = re.sub(r"<[^>]+>", "", heading)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = text.replace("`", "").replace("*", "").lower().strip()
    return "".join(c for c in text if c.isalnum() or c in " -_").replace(" ", "-")


def anchors(path: Path) -> set[str]:
    found, counts, fenced = set(), {}, False
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        if FENCE.match(line):
            fenced = not fenced
            continue
        m = None if fenced else HEADING.match(line)
        if m:
            s = slug(m.group(2))
            n = counts.get(s, 0)
            counts[s] = n + 1
            found.add(s if n == 0 else f"{s}-{n}")
    found.update(re.findall(r'<a (?:name|id)="([^"]+)"', text))
    return found


def main() -> int:
    problems, cache = [], {}
    for md in files():
        rel = md.relative_to(ROOT)
        raw = md.read_text(encoding="utf-8")
        text = strip_code(raw)
        if rel.parts[0] == "docs" and "\u2014" in text:
            problems.append(f"{rel}: em dash")
        for target in [a or b for a, b in LINK.findall(text)] + IMAGE.findall(text):
            if re.match(r"^[a-z][a-z0-9+.-]*:", target):
                continue
            path_part, _, frag = target.partition("#")
            dest = md if not path_part else (md.parent / path_part).resolve()
            if not dest.exists():
                problems.append(f"{rel}: broken link {target}")
            elif frag and dest.suffix == ".md":
                cache.setdefault(dest, anchors(dest))
                if frag not in cache[dest]:
                    problems.append(f"{rel}: no heading for #{frag} in {dest.relative_to(ROOT)}")
    for p in problems:
        print(p)
    print(f"{len(files())} files, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
