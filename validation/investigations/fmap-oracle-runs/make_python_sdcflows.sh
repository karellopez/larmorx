#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Makes <workspace>/oracles/python-sdcflows-2.17, the environment of sdcflows 2.17.1's own oracle runs
# (the field maps' part B, decision D8 (d)): every package as oracles/python-newest
# (validation/investigations/version-numerics/requirements-newest.txt) but niworkflows 1.15.4, the newest
# release, and without fMRIPrep. oracles/python-newest is never touched (its records were made with it).
# Records what it holds in <venv>/ENVIRONMENT.txt.
# Usage: make_python_sdcflows.sh <workspace>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
ws="${1:?usage: make_python_sdcflows.sh <workspace>}"
venv="$ws/oracles/python-sdcflows-2.17"
py="${PYTHON:-/usr/bin/python3.12}"
if [ -e "$venv" ]; then
  echo "$venv exists: never overwritten" >&2
  exit 1
fi
req="$(mktemp)"
trap 'rm -f "$req"' EXIT
sed 's/^niworkflows==1\.15\.3$/niworkflows==1.15.4/' \
  "$here/../version-numerics/requirements-newest.txt" > "$req"
grep -qx 'niworkflows==1.15.4' "$req"
"$py" -m venv "$venv"
"$venv/bin/python" -m pip install --upgrade --quiet pip
# --no-deps: exactly the listed versions, as oracles/python-newest
"$venv/bin/python" -m pip install --quiet --no-deps -r "$req"
{
  echo "# $venv, made $(date -Iseconds) by larmorx validation/investigations/fmap-oracle-runs/make_python_sdcflows.sh"
  "$venv/bin/python" -V
  echo "## pip freeze"
  "$venv/bin/python" -m pip freeze
} > "$venv/ENVIRONMENT.txt" 2>&1
echo "made $venv"
