#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Resolve larmorx's runtime and [prep] dependencies for each platform, wheels only
# (pip dry run: metadata only, nothing installed). pip does not expand a custom --platform to the
# older tags a real interpreter accepts, so each platform lists them.
cd "$(dirname "$0")" || exit 1
PY=${LARMORX_VENV:?set LARMORX_VENV (scripts/orchestration/env.sh)}/bin/python
ml() { local a=$1 s=""; for m in 28 27 26 25 24 23 22 21 20 19 18 17; do s+=" --platform manylinux_2_${m}_$a"; done; echo "$s --platform manylinux2014_$a"; }
mac() { local a=$1 top=$2 s=""; for v in $(seq 10 "$top"); do if [ "$v" = 10 ]; then for m in 9 10 11 12 13 14 15 16; do s+=" --platform macosx_10_${m}_$a"; done; else s+=" --platform macosx_${v}_0_$a"; fi; done; [ "$a" != x86_64 ] || s+=" --platform macosx_10_9_universal2"; [ "$a" != arm64 ] || s+=" --platform macosx_11_0_universal2 --platform macosx_10_9_universal2"; echo "$s"; }
declare -A P
P[linux-x64]=$(ml x86_64)
P[linux-aarch64]=$(ml aarch64)
P[macos-x64-12]=$(mac x86_64 12)
P[macos-x64-13]=$(mac x86_64 13)
P[macos-arm64-11]=$(mac arm64 11)
P[macos-arm64-12]=$(mac arm64 12)
P[macos-arm64-14]=$(mac arm64 14)
P[windows-x64]="--platform win_amd64"
P[windows-arm64]="--platform win_arm64"
for pyv in 3.12 3.14; do
  abi=cp${pyv/./}
  for name in linux-x64 linux-aarch64 macos-x64-12 macos-x64-13 macos-arm64-11 macos-arm64-12 macos-arm64-14 windows-x64 windows-arm64; do
    out=r-$pyv-$name
    # shellcheck disable=SC2086
    $PY -m pip install --dry-run --quiet --ignore-installed --only-binary=:all: ${P[$name]} \
      --python-version $pyv --implementation cp --abi $abi --abi abi3 --abi none --target "$PWD/tgt-$out" \
      --report $out.json "numpy>=1.26" templateflow matplotlib nilearn pandas jinja2 > $out.log 2>&1
    echo "$pyv $name exit=$?"
  done
done
