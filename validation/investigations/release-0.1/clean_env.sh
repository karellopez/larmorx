#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 4: the release's Linux x86_64 files (clean_build.sh's <out>/files) installed and tested in
# one clean environment, a container image run by Apptainer as this unprivileged user with
# nothing of this machine's on the paths (--containall --cleanenv, an empty home, every venv,
# cache and toolchain inside <out>; the checkout's clone read-only at /src for its tests and
# release scripts, the files read-only at /files):
#   1. the built wheels, each form in a fresh venv with the venv's own pip (as a user has it):
#      `larmorx` (the versions of larmorx, lx, larmorprepx and `python -m larmorx`; no companion),
#      `larmorx[prep]` (pip check, the [prep] modules, larmorprepx's "not ready yet"),
#      `larmorx[all]` (pip check, scripts/release/smoke.py --installed --archives on the
#      published-form files: versions, FSL data, the archives' programs, the glibc 2.28 floor
#      by objdump or, without binutils, by the files' own version tables), then the test group and `pytest tests/python tests/golden`
#      with LARMORX_NC_BIN and LARMORX_GPL_BIN at the installed programs (the golden suite and
#      tests/python/test_smoke.py included);
#   2. where the image has a C compiler (SDIST_RUST names a Rust toolchain, installed by
#      rustup into <out>): the three sdists built from source by pip (build isolation, maturin
#      from PyPI, the default release profile, no MATURIN_PEP517_ARGS: a user's
#      `pip install --no-binary`), `larmorx[all]`, then `larmorx` and `larmorx[prep]` (each
#      built again by a pip older than 26, taken from pip's wheel cache by pip 26), smoke.py
#      --installed --source-build, the same pytest run, and the companions' golden tests (cargo, LARMORX_GOLDEN_PROGRAM) on the
#      programs of both installs (the wheels' and the sdists');
#   3. SDIST_RUST=none (no compiler in the image): one `pip install` of larmorx's sdist, which must
#      fail (maturin's build backend fetches Rust itself when none is installed, then the link
#      needs a C compiler); what it said is kept as the message a user would see.
#
# Usage: clean_env.sh <name> <image.sif> <python> <build out> <out>
#   <python>: the interpreter in the image (python3.12, /usr/bin/python3.12, python3.14);
#   <build out>: clean_build.sh's <out> (its files/ and clone/).
# Optional: SDIST_RUST (1.99.0, stable or none; default none), JOBS (cargo's, default 28), SRC
# (another checkout for /src: its tests and release scripts; default <build out>/clone), RERUN=1
# (after a run in <out>: the three wheel installs, the smoke checks and pytest again, and the
# smoke checks and pytest on that run's sdist-built venv, whose builds are not repeated).
# Run it in a memory box: scripts/orchestration/boxed.sh 32 bash ... .
set -u
if [ "${1:-}" = inside ]; then
  # --- inside the image: /work is <out>, /files the files, /src the clone ---
  set -o pipefail
  NAME=$2 PY=$3 SDIST_RUST=$4 RERUN=$5
  export PIP_CACHE_DIR=/work/pip-cache PIP_DISABLE_PIP_VERSION_CHECK=1 PYTHONDONTWRITEBYTECODE=1
  T=x86_64-unknown-linux-gnu
  V=$($PY -P /src/scripts/release/set_version.py --print)
  fail=0
  step() { # <label> <command...>
    local s=$SECONDS label=$1
    shift
    if "$@"; then echo "STEP ok ($((SECONDS - s)) s): $NAME: $label"; else echo "STEP FAILED: $NAME: $label"; fail=1; fi
  }
  expect() { # <text> <command...>: the command's standard output is exactly <text>
    local want=$1 got
    shift
    got=$("$@") || { echo "exit status $? from $*"; return 1; }
    [ "$got" = "$want" ] || { echo "printed '$got', expected '$want'"; return 1; }
    echo "  $got"
  }
  have() { type -P "$1" >/dev/null; }
  echo "== $NAME: $(. /etc/os-release && echo "$PRETTY_NAME"), $(uname -m), $(ldd --version 2>&1 | head -1)," \
    "$($PY --version), compiler: $(have cc && cc --version | head -1 || echo none)," \
    "objdump: $(have objdump && echo yes || echo no), user $(id -u):$(id -g), version $V"
  venv() { # <dir>: a fresh venv, its pip as it comes
    rm -rf "$1" && $PY -m venv "$1" && echo "  $("$1/bin/python" -m pip --version | cut -d' ' -f1-2)"
  }
  versions() { # <venv> [all]
    local b=$1/bin
    expect "larmorx $V" "$b/larmorx" --version && expect "larmorx $V" "$b/lx" --version &&
      expect "larmorprepx $V" "$b/larmorprepx" --version &&
      expect "larmorx $V" "$b/python" -m larmorx --version &&
      expect "$V $V" "$b/python" -c "import larmorx, larmorx._core as c; print(larmorx.__version__, c.__version__)" &&
      if [ "${2:-}" = all ]; then
        expect "larmorx-nc $V" "$b/larmorx-nc" --version && expect "larmorx-gpl $V" "$b/larmorx-gpl" --version
      else
        expect "absent absent" "$b/python" -c "import importlib.metadata as m, shutil
def f(d):
    try: m.version(d); return 'installed'
    except m.PackageNotFoundError: return 'absent' if shutil.which(d, path='$b') is None else 'on path'
print(f('larmorx-nc'), f('larmorx-gpl'))"
      fi
  }
  prep_modules() { # <venv>
    "$1/bin/python" -c "import importlib.util as u; m = [n for n in ('matplotlib', 'nilearn', 'pandas', 'jinja2', 'templateflow', 'bids') if u.find_spec(n) is None]; assert not m, m" &&
      { "$1/bin/larmorprepx" >/work/tmp/prepx.txt 2>&1; [ $? = 1 ] && grep -q "not ready yet" /work/tmp/prepx.txt; } &&
      echo "  larmorprepx: $(head -c 120 /work/tmp/prepx.txt)..."
  }
  pytests() { # <venv> <label>: the test job's pytest run on the installed packages
    local b=$1/bin
    "$b/python" -m pip install -q --upgrade pip && (cd /src && "$b/python" -m pip install -q --group test) &&
      (cd /src && LARMORX_GOLDEN_MISMATCHES=/work/mismatches-$2 LARMORX_NC_BIN=$b/larmorx-nc \
        LARMORX_GPL_BIN=$b/larmorx-gpl "$b/python" -m pytest -q -p no:cacheprovider tests/python tests/golden 2>&1 |
        tail -1 | tee /work/pytest-$2.txt) && ! grep -q -E "failed|error" "/work/pytest-$2.txt"
  }
  cargo_golden() { # <venv> <label>: the companions' golden tests on that venv's programs
    local b=$1/bin
    for c in nc gpl; do
      (cd "/src/crates-$c" && RUSTUP_TOOLCHAIN=$SDIST_RUST CARGO_TARGET_DIR=/work/target-golden \
        LARMORX_GOLDEN_MISMATCHES=/work/mismatches-$2-$c LARMORX_GOLDEN_PROGRAM=$b/larmorx-$c \
        cargo test -q --locked -p "larmorx-$c-cli" --test golden 2>&1 | grep -E "^test result|FAILED|panicked" | tail -3) || return 1
    done
  }
  W=/work/v
  mkdir -p $W
  # 1. the built wheels
  wheel_larmorx() { venv $W/wheel-larmorx && $W/wheel-larmorx/bin/python -m pip install -q --find-links /files/wheels "larmorx==$V" &&
    $W/wheel-larmorx/bin/python -m pip check && versions $W/wheel-larmorx; }
  step "wheels: pip install larmorx" wheel_larmorx
  wheel_prep() { venv $W/wheel-prep && $W/wheel-prep/bin/python -m pip install -q --find-links /files/wheels "larmorx[prep]==$V" &&
    $W/wheel-prep/bin/python -m pip check && versions $W/wheel-prep && prep_modules $W/wheel-prep; }
  step "wheels: pip install larmorx[prep]" wheel_prep
  wheel_all() { venv $W/wheel-all && $W/wheel-all/bin/python -m pip install -q --only-binary larmorx,larmorx-nc,larmorx-gpl \
    --find-links /files/wheels "larmorx[all]==$V" && $W/wheel-all/bin/python -m pip check && versions $W/wheel-all all &&
    prep_modules $W/wheel-all && $W/wheel-all/bin/python -m pip list --format=freeze >/work/freeze-wheel-all.txt; }
  step "wheels: pip install larmorx[all]" wheel_all
  smoke_wheels() {
    $W/wheel-all/bin/python -P /src/scripts/release/smoke.py --version "$V" --installed --archives /files/archives \
      --target $T >/work/smoke-wheels.txt 2>&1
    local s=$?
    grep -v "^ok " /work/smoke-wheels.txt
    return $s
  }
  smoke_src() {
    $W/src-all/bin/python -P /src/scripts/release/smoke.py --version "$V" --installed --source-build \
      >/work/smoke-src.txt 2>&1
    local s=$?
    grep -v "^ok " /work/smoke-src.txt
    return $s
  }
  if [ "$RERUN" = 1 ]; then # the smoke checks and pytest again; the sdists' venv of the run before
    echo "== RERUN=1: the smoke checks and pytest again (the sdist-built venv of the run before)"
    step "wheels: smoke.py --installed --archives (the published-form files)" smoke_wheels
    step "wheels: pytest tests/python tests/golden" pytests $W/wheel-all wheels
    if [ -d "$W/src-all" ]; then
      step "sdists: smoke.py --installed --source-build" smoke_src
      step "sdists: pytest tests/python tests/golden" pytests $W/src-all src
    fi
    echo "CLEAN-ENV $NAME RERUN FINISHED status $fail"
    exit $fail
  fi
  step "wheels: smoke.py --installed --archives (the published-form files)" smoke_wheels
  step "wheels: pytest tests/python tests/golden" pytests $W/wheel-all wheels
  # 2. or 3. the sdists
  if [ "$SDIST_RUST" = none ]; then
    attempt() {
      venv $W/src-attempt || return 1
      if timeout 1800 $W/src-attempt/bin/python -m pip install --no-binary larmorx --find-links /files/sdist "larmorx==$V" \
        >/work/sdist-attempt.txt 2>&1; then
        echo "  the sdist built without a compiler (unexpected)"
        return 1
      fi
      echo "  pip failed, as it must; what it said about Rust and the linker, and its last lines:"
      grep -E "puccinialin|Rust not found|cargo [0-9]|linker" /work/sdist-attempt.txt | sed 's/^ */    /'
      grep -v "^\s*$" /work/sdist-attempt.txt | tail -4 | sed 's/^ */    /'
    }
    step "sdist without a C compiler: pip install fails" attempt
  else
    rust() {
      export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup PATH=/work/cargo/bin:$PATH
      curl -sSf --proto '=https' --tlsv1.2 https://sh.rustup.rs -o /work/tmp/rustup-init.sh &&
        sh /work/tmp/rustup-init.sh -y -q --no-modify-path --profile minimal --default-toolchain "$SDIST_RUST" &&
        echo "  $(rustc --version)"
    }
    step "Rust $SDIST_RUST (rustup)" rust
    export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup PATH=/work/cargo/bin:$PATH
    src_all() { venv $W/src-all && $W/src-all/bin/python -m pip install -q --no-binary larmorx,larmorx-nc,larmorx-gpl \
      --find-links /files/sdist "larmorx[all]==$V" && $W/src-all/bin/python -m pip check && versions $W/src-all all &&
      prep_modules $W/src-all; }
    step "sdists: pip install --no-binary ... larmorx[all] (built from source)" src_all
    src_larmorx() { venv $W/src-larmorx && $W/src-larmorx/bin/python -m pip install -q --no-binary larmorx \
      --find-links /files/sdist "larmorx==$V" && $W/src-larmorx/bin/python -m pip check && versions $W/src-larmorx; }
    step "sdists: pip install --no-binary larmorx larmorx" src_larmorx
    src_prep() { venv $W/src-prep && $W/src-prep/bin/python -m pip install -q --no-binary larmorx \
      --find-links /files/sdist "larmorx[prep]==$V" && $W/src-prep/bin/python -m pip check && versions $W/src-prep &&
      prep_modules $W/src-prep; }
    step "sdists: pip install --no-binary larmorx larmorx[prep]" src_prep
    step "sdists: smoke.py --installed --source-build" smoke_src
    step "sdists: pytest tests/python tests/golden" pytests $W/src-all src
    step "wheels: the companions' golden tests (cargo) on the installed programs" cargo_golden $W/wheel-all wheels
    step "sdists: the companions' golden tests (cargo) on the installed programs" cargo_golden $W/src-all src
    find "$PIP_CACHE_DIR/wheels" -name "larmorx*.whl" -printf "  built and cached: %s bytes %f\n" 2>/dev/null
  fi
  echo "CLEAN-ENV $NAME FINISHED status $fail"
  exit $fail
fi
[ $# -ge 5 ] || { sed -n '3,33p' "$0"; exit 2; }
NAME=$1
SIF=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
PY=$3
B=$(cd "$4" && pwd)
mkdir -p "$5" && R=$(cd "$5" && pwd)
SELF=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")
[ -d "$B/files/wheels" ] && [ -d "$B/clone" ] || { echo "no files or clone in $B (clean_build.sh first)"; exit 2; }
SRC=$(cd "${SRC:-$B/clone}" && pwd)
if [ "${RERUN:-0}" = 1 ]; then
  [ -d "$R/v/wheel-all" ] || { echo "RERUN=1: no earlier run in $R"; exit 2; }
  rm -rf "${R:?}"/mismatches-* "${R:?}/tmp" && mkdir -p "$R/tmp"
else
  rm -rf "${R:?}/home" "${R:?}/tmp" "${R:?}/v" "${R:?}"/mismatches-* "${R:?}/target-golden"
  mkdir -p "$R/home" "$R/tmp"
fi
cp "$SELF" "$R/clean_env.sh"
echo "clean environment $NAME: $(basename "$SIF") ($(sha256sum "$SIF" | cut -c1-16)...), files of" \
  "$(git -C "$B/clone" rev-parse HEAD), tests and scripts of $(git -C "$SRC" rev-parse HEAD)"
exec apptainer exec --containall --cleanenv --home "$R/home:/home/user" --bind "$R:/work" \
  --bind "$B/files:/files:ro" --bind "$SRC:/src:ro" --pwd /work \
  --env "TMPDIR=/work/tmp,CARGO_BUILD_JOBS=${JOBS:-28},CARGO_TERM_COLOR=never" \
  "$SIF" bash /work/clean_env.sh inside "$NAME" "$PY" "${SDIST_RUST:-none}" "${RERUN:-0}"
