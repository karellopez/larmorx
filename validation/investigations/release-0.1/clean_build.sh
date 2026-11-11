#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 4: the release's Linux x86_64 files built as .github/workflows/release.yml builds them,
# inside the manylinux_2_28 image the workflow pins (by digest), through Apptainer, with no
# administrator rights and nothing of this machine's on the paths (--containall --cleanenv; a
# fresh clone of the checkout's commit; Rust 1.99.0 installed by rustup and maturin 1.15.0 and
# twine 7.0.0 by pip, all into <out>; the image's own CPython 3.12):
#   the sdist job:    PyPI's descriptions at the commit, `maturin sdist`, companion_sdist.py nc
#                     and gpl, `twine check --strict`;
#   the wheel job:    larmorx's wheel, `maturin build --profile dist --locked --manylinux 2_28`;
#   the programs job: the companions' sdists unpacked into _build/ inside the clone and their
#                     wheels built there (dist profile, manylinux 2_28), the standalone larmorx
#                     as a program wheel of crates/larmorx-cli (--bindings bin), the companions'
#                     golden tests on the programs their wheels install (LARMORX_GOLDEN_PROGRAM,
#                     RUSTUP_TOOLCHAIN as in the workflow), the archives (archives.py);
#   and `twine check --strict` on the wheels, the SHA-256 of every file (<out>/SHA256SUMS).
# The files: <out>/files/{sdist,dist,programs,archives}/ and <out>/files/wheels/ (the three
# published wheels only). clean_env.sh installs and tests them in clean environments.
#
# Usage: clean_build.sh <checkout> <out> <manylinux_2_28.sif>
#   <manylinux_2_28.sif>: apptainer pull ... docker://quay.io/pypa/manylinux_2_28_x86_64@sha256:
#   e0adf18521ad35192df733a6c62d35df7801957d24e0470238aacecfab2188a3 (release.yml's pin).
# Optional: JOBS (cargo's, default 28). Run it in a memory box:
# scripts/orchestration/boxed.sh 48 bash ... .
set -u
if [ "${1:-}" = inside ]; then
  # --- inside the image: /work is <out>, /work/clone the clone ---
  set -o pipefail
  export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup PATH=/work/cargo/bin:$PATH
  PY=/opt/python/cp312-cp312/bin/python
  T=x86_64-unknown-linux-gnu
  fail=0
  step() { # <label> <command...>
    local s=$SECONDS label=$1
    shift
    if "$@"; then echo "STEP ok ($((SECONDS - s)) s): $label"; else echo "STEP FAILED: $label"; fail=1; fi
  }
  echo "== the image: $(cat /etc/redhat-release), $(ldd --version | head -1), $($PY --version), $(gcc --version | head -1)"
  rustup_install() {
    curl -sSf --proto '=https' --tlsv1.2 https://sh.rustup.rs -o /work/tmp/rustup-init.sh &&
      sh /work/tmp/rustup-init.sh -y -q --no-modify-path --profile minimal --default-toolchain 1.99.0 &&
      rustc --version && cargo --version
  }
  step "Rust 1.99.0 (rustup)" rustup_install
  tools() { $PY -m venv /work/tools && /work/tools/bin/pip install -q maturin==1.15.0 twine==7.0.0 && /work/tools/bin/maturin --version; }
  step "maturin 1.15.0, twine 7.0.0" tools
  M=/work/tools/bin/maturin
  cd /work/clone || exit 1
  SHA=$(git rev-parse HEAD)
  VERSION=$($PY -P scripts/release/set_version.py --print)
  SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)
  export SOURCE_DATE_EPOCH
  echo "== $SHA, version $VERSION, SOURCE_DATE_EPOCH $SOURCE_DATE_EPOCH"
  F=/work/files
  sdists() {
    $PY -P scripts/release/pypi_readme.py --ref "$SHA" README.md crates-nc/PYPI.md crates-gpl/PYPI.md &&
      $M sdist --out $F/sdist &&
      $PY -P scripts/release/companion_sdist.py nc --out $F/sdist --maturin $M &&
      $PY -P scripts/release/companion_sdist.py gpl --out $F/sdist --maturin $M &&
      /work/tools/bin/twine check --strict $F/sdist/*
  }
  step "sdist job: the three sdists" sdists
  build_in() { # <dir> <out> <maturin args...>
    local dir=$1 out=$2
    shift 2
    (cd "$dir" && $M build --profile dist --locked --manylinux 2_28 --target $T --out "$out" "$@" 2>&1 | tail -3)
  }
  step "wheel job: larmorx's wheel" build_in . $F/dist
  unpack() {
    $PY -c "import sys, tarfile; [tarfile.open(p).extractall('_build', filter='data') for p in sys.argv[1:]]" \
      "$F/sdist/larmorx_nc-$VERSION.tar.gz" "$F/sdist/larmorx_gpl-$VERSION.tar.gz" && ls _build
  }
  step "programs job: unpack the companions' sdists" unpack
  step "programs job: larmorx-nc's wheel from its sdist" build_in "_build/larmorx_nc-$VERSION" $F/programs
  step "programs job: larmorx-gpl's wheel from its sdist" build_in "_build/larmorx_gpl-$VERSION" $F/programs
  step "programs job: the standalone larmorx (program wheel)" build_in crates/larmorx-cli $F/programs --bindings bin
  golden() {
    $PY -m venv /work/venv-programs &&
      /work/venv-programs/bin/pip install -q --no-index --no-deps "$F/programs/larmorx_nc-$VERSION"-*.whl "$F/programs/larmorx_gpl-$VERSION"-*.whl &&
      /work/venv-programs/bin/larmorx-nc --version && /work/venv-programs/bin/larmorx-gpl --version &&
      (cd crates-nc && RUSTUP_TOOLCHAIN=1.99.0 LARMORX_GOLDEN_MISMATCHES=/work/mismatches LARMORX_GOLDEN_PROGRAM=/work/venv-programs/bin/larmorx-nc \
        cargo test -q --locked -p larmorx-nc-cli --test golden 2>&1 | tail -4) &&
      (cd crates-gpl && RUSTUP_TOOLCHAIN=1.99.0 LARMORX_GOLDEN_MISMATCHES=/work/mismatches LARMORX_GOLDEN_PROGRAM=/work/venv-programs/bin/larmorx-gpl \
        cargo test -q --locked -p larmorx-gpl-cli --test golden 2>&1 | tail -4)
  }
  step "programs job: the companions' golden tests on their installed programs" golden
  step "programs job: the archives" $PY -P scripts/release/archives.py --version "$VERSION" \
    --target $T --wheels $F/programs --out $F/archives
  wheels() {
    mkdir -p $F/wheels && cp $F/dist/larmorx-"$VERSION"-*.whl $F/programs/larmorx_nc-"$VERSION"-*.whl \
      $F/programs/larmorx_gpl-"$VERSION"-*.whl $F/wheels/ && /work/tools/bin/twine check --strict $F/wheels/*
  }
  step "the three published wheels, twine check" wheels
  (cd $F && find . -type f ! -name SHA256SUMS | sort | xargs sha256sum >SHA256SUMS && ls -l sdist dist programs archives wheels)
  git status --short | grep -v -E "^\?\? (_build|target)/$" | head
  echo "CLEAN-BUILD FINISHED status $fail"
  exit $fail
fi
[ $# -ge 3 ] || { sed -n '3,25p' "$0"; exit 2; }
W=$(cd "$1" && pwd)
mkdir -p "$2" && R=$(cd "$2" && pwd)
SIF=$(cd "$(dirname "$3")" && pwd)/$(basename "$3")
SELF=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")
SHA=$(git -C "$W" rev-parse HEAD)
rm -rf "$R/clone" "$R/files" "$R/venv-programs" "$R/mismatches" "$R/tools"
mkdir -p "$R/home" "$R/tmp" "$R/files"
git clone -q --no-hardlinks "$W" "$R/clone" && git -C "$R/clone" checkout -q "$SHA" || exit 1
cp "$SELF" "$R/clean_build.sh"
echo "clean build of $SHA in $(basename "$SIF") ($(sha256sum "$SIF" | cut -c1-16)...)"
exec apptainer exec --containall --cleanenv --home "$R/home:/home/builder" --bind "$R:/work" \
  --pwd /work/clone --env "TMPDIR=/work/tmp,CARGO_BUILD_JOBS=${JOBS:-28},CARGO_TERM_COLOR=never" \
  "$SIF" bash /work/clean_build.sh inside
