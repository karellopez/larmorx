#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the probes of this directory against the headers and libraries of the FSL in FSLDIR
# (default ~/fsl, FSL 6.0.7.17; FSLDIR=~/fsl-6.0.7.23 for the newest release),
# with the GCC 14.3 toolchain of oracles/toolchains (any C++17 compiler works). The outputs
# land in the directory given as the first argument (default: this directory).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="${1:-$here}"
ws="${LARMORX_WORKSPACE:-$(cd "$here/../../../.." && pwd)}"
cxx="${CXX:-$ws/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++}"
fsl="${FSLDIR:-$HOME/fsl}"
mkdir -p "$out"
"$cxx" -std=c++17 -O2 -shared -fPIC -I"$fsl/include" -o "$out/libaff_trace.so" "$here/aff_trace.cc" -ldl
"$cxx" -std=c++17 -O2 -shared -fPIC -I"$fsl/include" -o "$out/libcost_trace.so" "$here/cost_trace.cc" -ldl
"$cxx" -std=c++17 -O2 -march=nocona -mtune=haswell -I"$fsl/include" -o "$out/cost_probe" \
  "$here/cost_probe.cc" -L"$fsl/lib" -Wl,-rpath,"$fsl/lib" -lfsl-newimage -lfsl-miscmaths \
  -lfsl-NewNifti -lfsl-znz -lfsl-utils -lfsl-cprob -llapack -lblas -lz
for p in nan_decompose nan_r2e nan_rms; do
  "$cxx" -std=c++17 -O2 -march=nocona -mtune=haswell -I"$fsl/include" -o "$out/$p" "$here/$p.cc" \
    -L"$fsl/lib" -Wl,-rpath,"$fsl/lib" -lfsl-miscmaths -lfsl-NewNifti -lfsl-znz -lfsl-utils \
    -lfsl-cprob -llapack -lblas -lz
done
# The NaN trap (plain C, no FSL).
"${CC:-gcc}" -O2 -shared -fPIC -o "$out/libnan_trap.so" "$here/nan_trap.c" -lm
echo "built in $out"
