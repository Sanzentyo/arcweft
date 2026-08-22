#!/usr/bin/env sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
python3 "$ROOT/tools/validate_contract.py" "$ROOT"
python3 "$ROOT/tools/negative_self_tests.py" "$ROOT"
