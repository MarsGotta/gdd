#!/usr/bin/env python3
"""Check the Spanish docs against the English ones.

    python3 scripts/check_translation.py

docs/es/ mirrors docs/: every English page a user reads has a Spanish twin at the same path, and
the twin keeps the same code. For each pair it checks that the Spanish page exists, has as many
headings and code blocks, and that every code block but the Mermaid diagrams (whose labels are
translated) is identical: commands, configuration and real output stay in English. A Spanish
page whose English page is newer in git is listed as possibly out of date (a warning, not an
error). The project's records (Brief, standards, ADRs, Discover notes) and STYLE.md are not
translated. Exit 1 on any problem, so CI can run it.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
ES = DOCS / "es"
NOT_TRANSLATED = ("STYLE.md", "brief.md", "standards.md", "adr/", "discover/", "es/")
FENCE = re.compile(r"^\s*(```|~~~)(.*)$")
HEADING = re.compile(r"^#{1,6} ")


def translated(rel: str) -> bool:
    return not any(rel == s or (s.endswith("/") and rel.startswith(s)) for s in NOT_TRANSLATED)


def parts(path: Path) -> tuple[int, list[tuple[str, str]]]:
    """The number of headings outside code, and the code blocks as (info string, body)."""
    headings, blocks, fence, info, body = 0, [], None, "", []
    for line in path.read_text(encoding="utf-8").splitlines():
        m = FENCE.match(line)
        if fence is None and m:
            fence, info, body = m.group(1), m.group(2).strip(), []
        elif fence is not None and line.strip().startswith(fence) and not line.strip()[len(fence):].strip():
            blocks.append((info, "\n".join(body)))
            fence = None
        elif fence is not None:
            body.append(line)
        elif HEADING.match(line):
            headings += 1
    return headings, blocks


def last_change(path: Path) -> int:
    out = subprocess.run(["git", "log", "-1", "--format=%ct", "--", str(path)], cwd=ROOT,
                         capture_output=True, text=True).stdout.strip()
    return int(out) if out else 0


def main() -> int:
    problems, stale, pages = [], [], 0
    for en in sorted(DOCS.rglob("*.md")):
        rel = en.relative_to(DOCS).as_posix()
        if not translated(rel):
            continue
        pages += 1
        es = ES / rel
        if not es.is_file():
            problems.append(f"docs/es/{rel}: missing (translation of docs/{rel})")
            continue
        (h_en, b_en), (h_es, b_es) = parts(en), parts(es)
        if h_en != h_es:
            problems.append(f"docs/es/{rel}: {h_es} headings, docs/{rel} has {h_en}")
        if len(b_en) != len(b_es):
            problems.append(f"docs/es/{rel}: {len(b_es)} code blocks, docs/{rel} has {len(b_en)}")
        else:
            for n, ((info_en, body_en), (info_es, body_es)) in enumerate(zip(b_en, b_es), 1):
                if info_en != info_es:
                    problems.append(f"docs/es/{rel}: code block {n} is `{info_es}`, not `{info_en}`")
                elif info_en != "mermaid" and body_en != body_es:
                    problems.append(f"docs/es/{rel}: code block {n} differs from docs/{rel}")
        if last_change(en) > last_change(es):
            stale.append(f"docs/es/{rel}: docs/{rel} changed after it")
    for extra in sorted(ES.rglob("*.md")):
        rel = extra.relative_to(ES).as_posix()
        if not (DOCS / rel).is_file():
            problems.append(f"docs/es/{rel}: no English page docs/{rel}")
    for line in stale:
        print(f"warning: {line}")
    for line in problems:
        print(line)
    print(f"{pages} pages, {len(problems)} problems, {len(stale)} possibly out of date")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
