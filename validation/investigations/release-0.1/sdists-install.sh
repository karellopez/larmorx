#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 2a: the three sdists (main by maturin sdist, the companions by companion_sdist.py), each
# installed from source (pip, build isolation: maturin from PyPI, the machine's Rust) in a fresh
# venv; then the smoke checks and the Python and golden tests on the installed packages.
set -u
W=${1:?usage: $0 <checkout> <output dir>}
S=${2:?usage: $0 <checkout> <output dir>}
source "$W/scripts/orchestration/env.sh"
R=$S
export CARGO_BUILD_JOBS=28 OPENBLAS_CORETYPE=Haswell
unset FSLDIR LARMORX_NC_BIN LARMORX_GPL_BIN PYTHONPATH VIRTUAL_ENV
rm -rf "$R/sdist" "$R/venv-src" "$R/wheels-src"; mkdir -p "$R/sdist" "$R/wheels-src"
cd "$W" || exit 1
echo "== sdists"
maturin sdist -o "$R/sdist" 2>&1 | tail -2
python -P scripts/release/companion_sdist.py nc --out "$R/sdist" 2>&1 | tail -3
python -P scripts/release/companion_sdist.py gpl --out "$R/sdist" 2>&1 | tail -3
ls -la "$R/sdist"
echo "== twine check"
twine check --strict "$R"/sdist/*.tar.gz
echo "== wheels from the sdists (pip wheel, build isolation)"
/usr/bin/python3.12 -m venv "$R/venv-src"
V=$R/venv-src/bin
"$V/pip" install -q --upgrade pip
start=$(date +%s)
for f in "$R"/sdist/*.tar.gz; do
  case $(basename "$f") in larmorx-0*) td=target-plain;; larmorx_nc*) td=target-nc-plain;; larmorx_gpl*) td=target-gpl-plain;; esac
  "$V/pip" wheel -q --no-deps -w "$R/wheels-src" "$f" 2>&1 | tail -3
done
echo "seconds $(( $(date +%s) - start ))"
ls -la "$R/wheels-src"
twine check --strict "$R"/wheels-src/*.whl
echo "== install"
"$V/pip" install -q --no-index --find-links "$R/wheels-src" larmorx larmorx-nc larmorx-gpl || "$V/pip" install -q --find-links "$R/wheels-src" larmorx larmorx-nc larmorx-gpl
"$V/pip" install -q pytest packaging nibabel
"$V/pip" list 2>/dev/null | grep -i -E "larmorx|numpy"
echo "== versions"
for c in larmorx lx larmorprepx larmorx-nc larmorx-gpl; do "$V/$c" --version; done
"$V/python" -c "import larmorx, larmorx._core as c, importlib.metadata as m; print(larmorx.__version__, c.__version__, m.version('larmorx'), m.version('larmorx-nc'), m.version('larmorx-gpl'))"
ls "$R/venv-src/share/larmorx-nc/fsl-data" | head
echo "== tests on the installed packages (no FSL, CI's view)"
cd "$W" || exit 1
LARMORX_GOLDEN_MISMATCHES=$R/mism-src "$V/python" -m pytest -q -p no:cacheprovider tests/python tests/golden 2>&1 | tail -8
echo "SDISTS DONE"
