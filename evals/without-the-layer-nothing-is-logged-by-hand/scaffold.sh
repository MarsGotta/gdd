#!/usr/bin/env bash
# Builds the project this case runs in. The harness runs it in the empty workspace of each run.
# The shop as a fresh clone has it: git leaves .gdd/ out, so its layer isn't there.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../fixtures/shop.sh" base .
rm -rf .gdd
