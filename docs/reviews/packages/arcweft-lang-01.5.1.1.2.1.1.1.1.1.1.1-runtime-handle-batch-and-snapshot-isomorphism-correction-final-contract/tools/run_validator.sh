#!/bin/sh
set -eu
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
exec python3 -S "$SCRIPT_DIR/validate_package.py" "$PACKAGE_ROOT" --self-test
