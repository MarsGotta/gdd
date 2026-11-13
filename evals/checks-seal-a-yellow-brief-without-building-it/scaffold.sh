#!/usr/bin/env bash
# Builds the project this case runs in. The harness runs it in the empty workspace of each run.
# A yellow change briefed, approved and sealed with its scenarios, before any test is written.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../fixtures/shop.sh" briefed .
