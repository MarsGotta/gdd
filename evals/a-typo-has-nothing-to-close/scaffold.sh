#!/usr/bin/env bash
# Builds the project this case runs in. The harness runs it in the empty workspace of each run.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../fixtures/shop.sh" base .
# A blueprint that says when what promised nothing closes by itself, approved with the rest.
python3 - <<'PY'
import pathlib
blueprint = pathlib.Path("gdd/blueprint.md")
text = blueprint.read_text()
assert text.count("\ncontext: []\n") == 1
blueprint.write_text(text.replace("\ncontext: []\n", "\nclosing:\n  days: 7\ncontext: []\n"))
PY
git -c user.name=Eval -c user.email=eval@example.com -c commit.gpgsign=false commit -q -am "Say when what promised nothing closes by itself"
