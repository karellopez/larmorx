#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds fsfilter (fsfilter.cpp) against FreeSurfer 8.2.0's unit oracle: its compiler (Ubuntu's
# gcc-8 8.4.0), its compile flags (those of the unit oracle's harness, fsunit, without -Werror)
# and its libraries (the official build's utils and packages, ITK 4.13.2's VNL), linked as fsunit
# is. The unit oracle must be built first (validation/investigations/freesurfer-unit-oracle,
# build.py all). Output: <workspace>/oracles/freesurfer-8.2.0-probes/fs-filter/fsfilter.
#
#   validation/probes/fs-filter/build.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=${1:-$(cd "$here/../../../.." && pwd)}
case "$ws" in */.claude/worktrees/*) ws=${ws%%/larmorx/.claude/worktrees/*} ;; esac
unit=$ws/oracles/freesurfer-8.2.0-unit
b=$unit/build/fs
s=$unit/src/fs-shadow
itk=$unit/itk-4.13.2
gcc=$unit/toolchain/bin/g++
out=$ws/oracles/freesurfer-8.2.0-probes/fs-filter
[ -x "$gcc" ] || { echo "no unit oracle toolchain at $unit (build.py all first)" >&2; exit 1; }
mkdir -p "$out"
flags=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DUSING_DCM2NIIXFSWRAPPER
  -DUSING_MGH_NIFTI_IO -I"$s/include" -I"$b/include" -I"$s/packages/minc" -I"$s/packages/netcdf"
  -I"$s/packages/nifti" -isystem "$itk/include/ITK-4.13" -fopenmp -fno-builtin -Wall -Wextra
  -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result
  -Wno-unused-parameter -Wno-bool-compare -Wno-unused-but-set-variable -fPIC -fdata-sections
  -ffunction-sections -O3 -DNDEBUG -std=c++11)
"$gcc" "${flags[@]}" -c "$here/fsfilter.cpp" -o "$out/fsfilter.o"
libs=("$b/utils/libutils.a")
for p in jpeg/libjpeg tiff/libtiff xml2/libxml2 minc/libminc netcdf/libnetcdf nrrdio/libnrrdio \
  dicom/libdicom cephes/libcephes nifti/libnifti dcm2niix/libdcm2niixfs gifti/libgifti expat/libexpat; do
  libs+=("$b/packages/$p.a")
done
for l in vnl_algo vnl v3p_netlib netlib vcl; do libs+=("$itk/lib/libitk$l-4.13.a"); done
"$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections -O3 "$out/fsfilter.o" -o "$out/fsfilter" \
  "${libs[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
  "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt
sha256sum "$out/fsfilter" "$here/fsfilter.cpp" > "$out/BUILD_INFO.txt"
"$gcc" --version | head -1 >> "$out/BUILD_INFO.txt"
echo "built $out/fsfilter"
