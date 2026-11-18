#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds fsconvert (fsconvert.cpp: mri_convert.cpp with its main renamed and two print hooks, and
# the stats-table functions of the unit oracle's libutils) against FreeSurfer 8.2.0's unit oracle:
# its compiler (Ubuntu's gcc-8 8.4.0), its compile flags (the unit oracle harness's, without
# -Werror) and its libraries (the official build's utils and packages, ITK 4.13.2's VNL), linked as
# the official program is. The unit oracle must be built first
# (validation/investigations/freesurfer-unit-oracle, build.py all).
#
# The hooks: a copy of mri_convert.cpp (from the tag, into the output folder) gets two lines,
# `#include "hook_init.inc"` after main's last declaration (main's locals the program leaves
# unset get marked values, so the dump can tell them) and `#include "hook_dump.inc"` before its
# check of a GDF image stem (after the option loop, its checks and the parse-only exit: every
# variable the options set is printed and the process ends). Nothing else of the file changes.
# Output: <workspace>/oracles/freesurfer-8.2.0-probes/fs-convert/fsconvert.
#
#   validation/probes/fs-convert/build.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=${1:-$(cd "$here/../../../.." && pwd)}
case "$ws" in */.claude/worktrees/*) ws=${ws%%/larmorx/.claude/worktrees/*} ;; esac
unit=$ws/oracles/freesurfer-8.2.0-unit
b=$unit/build/fs
s=$unit/src/fs-shadow
tag=$ws/tags/freesurfer-8.2.0
itk=$unit/itk-4.13.2
gcc=$unit/toolchain/bin/g++
out=$ws/oracles/freesurfer-8.2.0-probes/fs-convert
[ -x "$gcc" ] || { echo "no unit oracle toolchain at $unit (build.py all first)" >&2; exit 1; }
mkdir -p "$out"
src=$tag/mri_convert/mri_convert.cpp
init='  int UpsampleFlag=0, UpsampleFactor=0;'
dump='  /* ----- check for a gdf image stem if the output type is gdf ----- */'
[ "$(grep -cxF "$init" "$src")" = 1 ] || { echo "the init hook's line is not unique" >&2; exit 1; }
[ "$(grep -cxF "$dump" "$src")" = 1 ] || { echo "the dump hook's line is not unique" >&2; exit 1; }
awk -v init="$init" -v dump="$dump" '
  $0 == dump { print "#include \"hook_dump.inc\"" }
  { print }
  $0 == init { print "#include \"hook_init.inc\"" }
' "$src" > "$out/mri_convert_hooked.cpp"
cp "$here/hook_init.inc" "$here/hook_dump.inc" "$out/"
(cd "$tag/mri_convert" && xxd -i mri_convert.help.xml) > "$out/mri_convert.help.xml.h"
flags=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DUSING_DCM2NIIXFSWRAPPER
  -DUSING_MGH_NIFTI_IO -I"$out" -I"$tag/mri_convert" -I"$s/include" -I"$b/include" -I"$s/packages/minc"
  -I"$s/packages/netcdf" -I"$s/packages/nifti" -I"$s/packages/dicom" -I"$s/packages/dcm2niix" -isystem "$itk/include/ITK-4.13" -fopenmp -fno-builtin
  -Wall -Wextra -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result
  -Wno-unused-parameter -Wno-bool-compare -Wno-unused-but-set-variable -fPIC -fdata-sections
  -ffunction-sections -O3 -DNDEBUG -Wno-self-assign -std=c++11)
"$gcc" "${flags[@]}" -c "$here/fsconvert.cpp" -o "$out/fsconvert.o"
libs=("$b/utils/libutils.a")
for p in jpeg/libjpeg tiff/libtiff xml2/libxml2 minc/libminc netcdf/libnetcdf nrrdio/libnrrdio \
  dicom/libdicom cephes/libcephes nifti/libnifti dcm2niix/libdcm2niixfs gifti/libgifti expat/libexpat; do
  libs+=("$b/packages/$p.a")
done
for l in vnl_algo vnl v3p_netlib netlib vcl; do libs+=("$itk/lib/libitk$l-4.13.a"); done
"$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections -O3 "$out/fsconvert.o" -o "$out/fsconvert" \
  "${libs[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
  "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt
sha256sum "$out/fsconvert" "$here/fsconvert.cpp" "$here/hook_init.inc" "$here/hook_dump.inc" "$src" \
  > "$out/BUILD_INFO.txt"
"$gcc" --version | head -1 >> "$out/BUILD_INFO.txt"
echo "built $out/fsconvert"
