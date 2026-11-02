#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Python gates of a checkout: (maturin develop --release,) pytest tests/python tests/golden
# validation/tests (the validation package's own tests: the oracle cache), the collection CI sees
# (without larmorx_validation), ruff, SPDX.
# Usage: gates-python.sh <checkout> [--no-build]
# `maturin develop` points the shared environment at <checkout>: use it in the main checkout only;
# in an agent's worktree build the wheel elsewhere, copy its _core into python/larmorx/ and pass --no-build.
set -o pipefail
source "$(dirname "$0")/env.sh"
W=$1; status=0
export VIRTUAL_ENV=$LARMORX_VENV PYTHONPATH=$W/python:$W/validation/src
cd "$W" || exit 1
if [ "$2" != "--no-build" ]; then maturin develop --release || { echo "MATURIN FAILED"; exit 1; }; fi
python -m pytest -q tests/python tests/golden validation/tests || { echo "PYTEST FAILED"; status=1; }
# CI's view (`python -m pytest`, the wheel without larmorx_validation): every test file must collect
# (a parity file imports larmorx_validation only through pytest.importorskip; 2026-11-02: a top-level
# import failed every platform's collection)
python -c "import sys; sys.modules['larmorx_validation'] = None; import pytest; sys.exit(pytest.main(['-q', '--collect-only', '-p', 'no:cacheprovider']))" >/dev/null \
  || { echo "COLLECTION WITHOUT larmorx_validation FAILED (CI's view)"; status=1; }
ruff check || { echo "RUFF CHECK FAILED"; status=1; }
ruff format --check || { echo "RUFF FORMAT FAILED"; status=1; }
python scripts/check_spdx.py || { echo "SPDX FAILED"; status=1; }
echo "GATES-PYTHON exit $status"; exit $status
