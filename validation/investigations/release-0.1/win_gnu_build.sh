#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 4: Windows x86_64 files for a trial of scripts/release/check_install_windows.ps1 before
# the release (the release's own Windows files are built by GitHub's Windows runners with MSVC;
# a dry run's artifacts cannot be fetched without a login). Cross-built here with no Microsoft
# SDK or MSVC (nothing whose terms someone would have to accept): Rust's x86_64-pc-windows-gnu
# target and zig's MinGW-w64 linker (ziglang from PyPI, through the cargo-zigbuild wrappers
# maturin makes), in the manylinux_2_28 image, in clean_build.sh's <out> (its clone, Rust and
# maturin): larmorx's abi3 wheel (python3.dll's import library made by pyo3's
# generate-import-lib, no Windows Python files needed), larmorx-nc's and larmorx-gpl's wheels from
# their unpacked sdists, and the standalone larmorx (a program wheel). The release profile.
# These files link the Universal C Runtime (its api-ms-win-crt-* DLLs, part of Windows 10 and
# 11), which the release's PE check refuses, as it must: the trial's smoke check fails exactly
# there and nowhere else.
#   `maturin build --zig` builds the companions and the program; for the extension its .def file
#   names `PyInit_larmorx._core` (the dotted module name), which the link cannot find, so the
#   extension is built without --zig, with the zig wrapper of the companions' build as linker.
#
# Usage: win_gnu_build.sh <clean_build out> <manylinux_2_28.sif>
#   (after clean_build.sh; the files go to <out>/wgnu/). Optional: JOBS (default 8), ZIG
#   (ziglang's version, default 0.14.1). Run it in a memory box: scripts/orchestration/boxed.sh 24.
set -u
if [ "${1:-}" = inside ]; then
  set -o pipefail
  export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup PATH=/work/cargo/bin:$PATH
  PY=/opt/python/cp312-cp312/bin/python
  T=x86_64-pc-windows-gnu
  O=/work/wgnu
  M=/work/tools/bin/maturin
  fail=0
  rustup target add --toolchain 1.99.0 $T 2>&1 | tail -1
  [ -x /work/zig/bin/python ] || { $PY -m venv /work/zig && /work/zig/bin/pip install -q "ziglang==${ZIG:-0.14.1}"; }
  export PATH=/work/zig/bin:$PATH
  echo "zig $(/work/zig/bin/python -m ziglang version), $($M --version), $(rustc --version)"
  cd /work/clone || exit 1
  V=$($PY -P scripts/release/set_version.py --print)
  rm -rf $O && mkdir -p $O
  zigbuild() { # <dir> <maturin args...>
    local dir=$1
    shift
    (cd "$dir" && $M build --release --locked --zig --target $T --out $O "$@" 2>&1 | tail -2) || fail=1
  }
  zigbuild "_build/larmorx_nc-$V"
  zigbuild "_build/larmorx_gpl-$V"
  zigbuild crates/larmorx-cli --bindings bin
  # the zig wrapper the companions' builds made, as the extension's linker
  L=$(find /home/builder/.cache/cargo-zigbuild -name "zigcc-$T-*.sh" | head -1)
  [ -n "$L" ] || { echo "no cargo-zigbuild wrapper"; exit 1; }
  Z=$(dirname "$L")
  (export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$L CC_x86_64_pc_windows_gnu=$L AR_x86_64_pc_windows_gnu=$Z/ar \
    ZIG_COMMAND="/work/zig/bin/python3 -m ziglang" PATH=$Z:$PATH
    $M build --release --locked --target $T --features pyo3/generate-import-lib --out $O 2>&1 | tail -2) || fail=1
  ls -l $O
  echo "WIN-GNU-BUILD FINISHED status $fail"
  exit $fail
fi
[ $# -ge 2 ] || { sed -n '3,22p' "$0"; exit 2; }
R=$(cd "$1" && pwd)
SIF=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
[ -d "$R/clone/_build" ] && [ -x "$R/tools/bin/maturin" ] || { echo "run clean_build.sh into $R first"; exit 2; }
cp "$0" "$R/win_gnu_build.sh"
exec apptainer exec --containall --cleanenv --home "$R/home:/home/builder" --bind "$R:/work" \
  --pwd /work/clone --env "TMPDIR=/work/tmp,CARGO_BUILD_JOBS=${JOBS:-8},CARGO_TERM_COLOR=never,ZIG=${ZIG:-0.14.1}" \
  "$SIF" bash /work/win_gnu_build.sh inside
