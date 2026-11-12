#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: mirrors FreeSurfer 8.2.0's build.
#
# Does GCC 8.4.0 keep the float roundings of ITK 4.13.2's B-spline fit (GCC bug 110182)?
# 1. Configures (does not build) ITK 4.13.2 with GCC 8.4.0 and FreeSurfer 8.2.0's own ITK options
#    (packages/source/build_itk.sh: Release, -msse2 -mfpmath=sse, C++11, the six module groups),
#    which writes ITK's generated headers (itkConfigure.h, vnl_config.h, ...).
# 2. Compiles n4_tu.cxx (FreeSurfer's N4 pipeline on ITK 4.13.2) with FreeSurfer's C++ flags
#    (code.md section 1) and GCC's GIMPLE dumps after the loop vectorizer, the SLP vectorizer and
#    at the end; the object is kept (its symbols let fit_in_binary.py find the same code in the
#    official program).
# Usage: gcc84_n4_dumps.sh <itk-4.13.2-source> <gcc-8.4-prefix> <out-dir>
set -euo pipefail
SRC="$(cd "$1" && pwd)"
GCC="$(cd "$2" && pwd)"
OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
CXX="$GCC/bin/x86_64-conda-linux-gnu-g++"
CC="$GCC/bin/x86_64-conda-linux-gnu-gcc"
mkdir -p "$OUT/build" "$OUT/dumps"
OUT="$(cd "$OUT" && pwd)"
export PATH="$GCC/bin:$PATH"
if [[ ! -f "$OUT/build/CMakeCache.txt" ]]; then
  (cd "$OUT/build" && cmake "$SRC" \
    -DCMAKE_C_COMPILER="$CC" -DCMAKE_CXX_COMPILER="$CXX" \
    -DCMAKE_BUILD_TYPE:STRING=Release \
    -DCMAKE_CXX_FLAGS:STRING="-msse2 -mfpmath=sse" -DCMAKE_C_FLAGS:STRING="-msse2 -mfpmath=sse" \
    -DCMAKE_CXX_STANDARD:STRING=11 -DCMAKE_C_STANDARD:STRING=11 \
    -DITK_BUILD_DEFAULT_MODULES:BOOL=OFF -DITKGroup_Core:BOOL=ON -DITKGroup_Filtering:BOOL=ON \
    -DITKGroup_Numerics:BOOL=ON -DITKGroup_Registration:BOOL=ON -DITKGroup_Segmentation:BOOL=ON \
    -DITKGroup_IO:BOOL=ON -DModule_AnisotropicDiffusionLBR:BOOL=ON -DBUILD_TESTING:BOOL=OFF \
    > "$OUT/configure.log" 2>&1) || { echo "configure failed: $OUT/configure.log"; exit 1; }
fi
INCS=()
while IFS= read -r d; do INCS+=("-I$d"); done < <(
  find "$OUT/build" -name '*.h' -printf '%h\n' | sort -u
  find "$SRC/Modules" -type d -name include -not -path '*/test/*' | sort
  echo "$SRC/Modules/ThirdParty/VNL/src/vxl/core"
  echo "$SRC/Modules/ThirdParty/VNL/src/vxl/vcl"
  echo "$SRC/Modules/ThirdParty/VNL/src/vxl/v3p/netlib"
)
FLAGS=(-std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin -fPIC -ffunction-sections -fdata-sections)
# Ubuntu 20.04's gcc-8 (8.4.0-3ubuntu2, the official build's) turns these on by default;
# conda-forge's GCC 8.4.0 does not (the official binaries hold endbr64 and stack canaries).
UBUNTU=(-fcf-protection=full -fstack-protector-strong -fstack-clash-protection -D_FORTIFY_SOURCE=2)
[[ "${NO_UBUNTU_DEFAULTS:-0}" == 1 ]] && UBUNTU=()
"$CXX" "${FLAGS[@]}" "${UBUNTU[@]}" "${INCS[@]}" -c "$HERE/n4_tu.cxx" -o "$OUT/n4_tu.o" \
  -fdump-tree-vect -fdump-tree-slp1 -fdump-tree-optimized 2> "$OUT/compile.log" \
  || { echo "compile failed: $OUT/compile.log"; exit 1; }
mv "$OUT"/n4_tu.cxx.*t.* "$OUT/dumps/"
{ "$CXX" --version | head -1; echo "flags: ${FLAGS[*]} ${UBUNTU[*]}"; } > "$OUT/compiler.txt"
echo "dumps in $OUT/dumps ($(ls "$OUT/dumps" | wc -l) files), object $OUT/n4_tu.o"
