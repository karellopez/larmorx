#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 3a: the release workflow's Linux steps (.github/workflows/release.yml) run here, outside
# GitHub, on a fresh clone of the checkout's commit: the sdist job (PyPI's descriptions at the
# commit, the three sdists with the workflow's maturin, twine check), the programs job (the
# companions' wheels built from their unpacked sdists inside the clone, the standalone larmorx as
# a program wheel, the companions' golden tests on the programs their wheels install, the
# archives), and the test job's steps on a fresh venv of the system's CPython 3.12 with the three
# wheels (the main one built in the clone): pytest tests/python and tests/golden with
# LARMORX_NC_BIN and LARMORX_GPL_BIN at the installed programs (as the runners: the parity suites
# would find this machine's oracles), the smoke checks, and larmorx[all] from the built files in a
# second fresh venv (PyPI for the [prep] dependencies). The workflow builds in the manylinux_2_28
# container; here the host's glibc tags the wheels and its symbol versions (2.34 or newer) make
# the smoke's glibc 2.28 check fail: that failure is expected here and listed at the end, every
# other check must pass.
#
# Usage: workflow_rehearsal.sh <checkout> <out> [<maturin>]
#   <maturin>: the maturin program (the workflow's MATURIN_VERSION; default: maturin on PATH).
# Optional: TWINE (the twine program), CLI_TARGET, NC_TARGET, GPL_TARGET, MAIN_TARGET (the four
# builds' CARGO_TARGET_DIR, to reuse a cache). Run it in a memory box:
# scripts/orchestration/boxed.sh 32 bash ... .
set -u
[ $# -ge 2 ] || { sed -n '3,24p' "$0"; exit 2; }
W=$(cd "$1" && pwd)
mkdir -p "$2" && R=$(cd "$2" && pwd)
MATURIN=${3:-maturin}
source "$W/scripts/orchestration/env.sh"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-28}
unset FSLDIR LARMORX_NC_BIN LARMORX_GPL_BIN PYTHONPATH VIRTUAL_ENV
SHA=$(git -C "$W" rev-parse HEAD)
rm -rf "$R/clone" "$R/venv" "$R/venv-all" "$R/mismatches"
git clone -q --no-hardlinks "$W" "$R/clone" && git -C "$R/clone" checkout -q "$SHA" || exit 1
C=$R/clone
cd "$C" || exit 1
VERSION=$(python -P scripts/release/set_version.py --print)
SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)
export SOURCE_DATE_EPOCH
echo "== $SHA, version $VERSION, $("$MATURIN" --version)"
fail=0
step() { # <label> <command...>
  local s=$SECONDS label=$1
  shift
  if "$@"; then echo "STEP ok ($((SECONDS - s)) s): $label"; else echo "STEP FAILED: $label"; fail=1; fi
}
sdists() {
  python -P scripts/release/pypi_readme.py --ref "$SHA" README.md crates-nc/PYPI.md crates-gpl/PYPI.md &&
    "$MATURIN" sdist --out sdist &&
    python -P scripts/release/companion_sdist.py nc --out sdist --maturin "$MATURIN" &&
    python -P scripts/release/companion_sdist.py gpl --out sdist --maturin "$MATURIN" &&
    "${TWINE:-twine}" check --strict sdist/* && ls -l sdist
}
step "sdist job: the three sdists" sdists
unpack() {
  python -c "import sys, tarfile; [tarfile.open(p).extractall('_build', filter='data') for p in sys.argv[1:]]" \
    "sdist/larmorx_nc-$VERSION.tar.gz" "sdist/larmorx_gpl-$VERSION.tar.gz" && ls _build
}
step "programs job: unpack the companions' sdists" unpack
build_in() { # <dir> <target dir> <maturin args...>
  local dir=$1 td=$2
  shift 2
  (cd "$dir" && CARGO_TARGET_DIR=${td:-} "$MATURIN" build --profile dist --locked --out "$C/programs" "$@" 2>&1 | tail -2)
}
step "programs job: larmorx-nc's wheel from its sdist" build_in "_build/larmorx_nc-$VERSION" "${NC_TARGET:-}"
step "programs job: larmorx-gpl's wheel from its sdist" build_in "_build/larmorx_gpl-$VERSION" "${GPL_TARGET:-}"
step "programs job: the standalone larmorx (program wheel)" build_in crates/larmorx-cli "${CLI_TARGET:-}" --bindings bin
ls -l programs
golden() {
  python -m venv .venv-programs &&
    .venv-programs/bin/python -m pip install -q --no-index --no-deps "programs/larmorx_nc-$VERSION"-*.whl "programs/larmorx_gpl-$VERSION"-*.whl &&
    .venv-programs/bin/larmorx-nc --version && .venv-programs/bin/larmorx-gpl --version &&
    (cd crates-nc && LARMORX_GOLDEN_PROGRAM="$C/.venv-programs/bin/larmorx-nc" cargo test -q --locked -p larmorx-nc-cli --test golden 2>&1 | tail -3) &&
    (cd crates-gpl && LARMORX_GOLDEN_PROGRAM="$C/.venv-programs/bin/larmorx-gpl" cargo test -q --locked -p larmorx-gpl-cli --test golden 2>&1 | tail -3)
}
step "programs job: the companions' golden tests on their installed programs" golden
step "programs job: the archives" python -P scripts/release/archives.py --version "$VERSION" \
  --target x86_64-unknown-linux-gnu --wheels programs --out archives
ls -l archives
step "wheel job: larmorx's wheel" build_in . "${MAIN_TARGET:-}"
P=$R/venv/bin/python
install() {
  /usr/bin/python3.12 -m venv "$R/venv" &&
    "$P" -m pip install -q --upgrade pip &&
    "$P" -m pip install -q "programs/larmorx-$VERSION"-*.whl "programs/larmorx_nc-$VERSION"-*.whl "programs/larmorx_gpl-$VERSION"-*.whl &&
    "$P" -m pip install -q --group test
}
step "test job: the three wheels in a fresh venv" install
tests() {
  (unset LARMORX_WORKSPACE; env LARMORX_GOLDEN_MISMATCHES="$R/mismatches" \
    LARMORX_NC_BIN="$R/venv/bin/larmorx-nc" LARMORX_GPL_BIN="$R/venv/bin/larmorx-gpl" \
    "$P" -m pytest -q -p no:cacheprovider tests/python tests/golden 2>&1 | tail -1 | tee "$R/pytest.txt")
  ! grep -q -E "failed|error" "$R/pytest.txt"
}
step "test job: pytest tests/python tests/golden on the installed packages" tests
"$P" -P scripts/release/smoke.py --version "$VERSION" --installed --archives archives --target x86_64-unknown-linux-gnu | grep -v "^ok "
echo "(the smoke's glibc failures are expected on this host: see the header)"
all() {
  /usr/bin/python3.12 -m venv "$R/venv-all" &&
    "$R/venv-all/bin/python" -m pip install -q --upgrade pip &&
    "$R/venv-all/bin/python" -m pip install -q --only-binary larmorx,larmorx-nc,larmorx-gpl --find-links programs "larmorx[all]==$VERSION" &&
    "$R/venv-all/bin/python" -m pip check &&
    "$R/venv-all/bin/python" -c "import importlib.util as u; m = [n for n in ('matplotlib', 'nilearn', 'pandas', 'jinja2', 'templateflow') if u.find_spec(n) is None]; assert not m, m"
}
step "test job: larmorx[all] from the built files in a fresh venv" all
git -C "$C" status --short | grep -v -E "^\?\? (_build|programs|archives|sdist|\.venv-programs)/$" | head
echo "WORKFLOW-REHEARSAL FINISHED status $fail"
exit $fail
