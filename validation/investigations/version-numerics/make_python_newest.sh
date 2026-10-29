#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Makes <workspace>/oracles/python-newest, the oracle environment of the newest numerical releases
# (numpy 2.5.4, SciPy 1.18.1; every other package as in validation/requirements-lock.txt), beside the
# shared .venv, which it never touches. Records what it holds in <venv>/ENVIRONMENT.txt: pip freeze,
# numpy's and SciPy's build configuration (the OpenBLAS their wheels bundle) and the OpenBLAS cores.
# Usage: make_python_newest.sh <workspace>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
ws="${1:?usage: make_python_newest.sh <workspace>}"
venv="$ws/oracles/python-newest"
py="${PYTHON:-/usr/bin/python3.12}"
if [ ! -x "$venv/bin/python" ]; then
  "$py" -m venv "$venv"
fi
"$venv/bin/python" -m pip install --upgrade --quiet pip
# --no-deps: exactly the listed versions (antspyx 0.6.3 and smriprep's pins ask for numpy < 2.5;
# no suite of this environment imports them for numpy's sake, and nothing else is pulled in).
"$venv/bin/python" -m pip install --quiet --no-deps -r "$here/requirements-newest.txt"
"$venv/bin/python" -m pip install --quiet --no-deps "$ws/fmriprep"
site="$("$venv/bin/python" -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
echo "$ws/larmorx-testdata/src" > "$site/larmorx-testdata.pth"
{
  echo "# $venv, made $(date -Iseconds) by larmorx validation/investigations/version-numerics/make_python_newest.sh"
  "$venv/bin/python" -V
  echo "## pip freeze"
  "$venv/bin/python" -m pip freeze
  echo "## numpy.show_config()"
  OPENBLAS_CORETYPE=Haswell "$venv/bin/python" -c 'import numpy; numpy.show_config()'
  echo "## scipy.show_config()"
  OPENBLAS_CORETYPE=Haswell "$venv/bin/python" -c 'import scipy; scipy.show_config()'
  echo "## threadpoolctl (OPENBLAS_CORETYPE=Haswell)"
  OPENBLAS_CORETYPE=Haswell "$venv/bin/python" -c 'import numpy, scipy.linalg, threadpoolctl, json; print(json.dumps(threadpoolctl.threadpool_info(), indent=1))'
  echo "## OpenBLAS libraries"
  find "$site" -name 'libscipy_openblas*' -exec sha256sum {} \;
} > "$venv/ENVIRONMENT.txt" 2>&1
echo "made $venv"
