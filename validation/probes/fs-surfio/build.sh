#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds fssurfio (fssurfio.cpp) against FreeSurfer 8.2.0's unit oracle: its compiler (Ubuntu's
# gcc-8 8.4.0), its compile flags (those of the unit oracle's harness, fsunit, without -Werror)
# and its libraries (the official build's utils and packages, ITK 4.13.2's VNL), linked as fsunit
# is. The unit oracle must be built first (validation/investigations/freesurfer-unit-oracle,
# build.py all). The headers' __FILE__ (cheapAssert's messages in mrisurf.h) is mapped to the
# official build's source folder, read from the unit oracle's library, as the unit oracle maps its
# own. Output: <workspace>/oracles/freesurfer-8.2.0-probes/fs-surfio/fssurfio and fssurfio-mpstub
# (MRIScomputeMetricProperties replaced by the probe's stub, for MRISsmoothSurface).
#
#   validation/probes/fs.surfio/build.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=${1:-$(cd "$here/../../../.." && pwd)}
case "$ws/" in */.claude/worktrees/*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
unit=$ws/oracles/freesurfer-8.2.0-unit
b=$unit/build/fs
s=$unit/src/fs-shadow
itk=$unit/itk-4.13.2
gcc=$unit/toolchain/bin/g++
out=$ws/oracles/freesurfer-8.2.0-probes/fs-surfio
[ -x "$gcc" ] || { echo "no unit oracle toolchain at $unit (build.py all first)" >&2; exit 1; }
mkdir -p "$out"
official=$(grep -a -o -m1 '/[[:graph:]]*/utils/mri\.cpp' "$b/utils/libutils.a" | head -1)
official=${official%/utils/mri.cpp}
[ -n "$official" ] || { echo "no official source folder in $b/utils/libutils.a" >&2; exit 1; }
flags=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DUSING_DCM2NIIXFSWRAPPER
  -DUSING_MGH_NIFTI_IO -I"$s/include" -I"$b/include" -I"$s/packages/minc" -I"$s/packages/netcdf"
  -I"$s/packages/nifti" -isystem "$itk/include/ITK-4.13" -fopenmp -fno-builtin -Wall -Wextra
  -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result
  -Wno-unused-parameter -Wno-bool-compare -Wno-unused-but-set-variable -fPIC -fdata-sections
  -ffunction-sections -O3 -DNDEBUG -std=c++11 "-fmacro-prefix-map=$s=$official")
libs=("$b/utils/libutils.a")
for p in jpeg/libjpeg tiff/libtiff xml2/libxml2 minc/libminc netcdf/libnetcdf nrrdio/libnrrdio \
  dicom/libdicom cephes/libcephes nifti/libnifti dcm2niix/libdcm2niixfs gifti/libgifti expat/libexpat; do
  libs+=("$b/packages/$p.a")
done
for l in vnl_algo vnl v3p_netlib netlib vcl; do libs+=("$itk/lib/libitk$l-4.13.a"); done
# fssurfio, and fssurfio-mpstub: the same probe with a stub MRIScomputeMetricProperties that the
# library's own calls reach (its definition first, --allow-multiple-definition)
for variant in fssurfio fssurfio-mpstub; do
  extra=()
  link=()
  if [ "$variant" = fssurfio-mpstub ]; then extra=(-DPROBE_MP_STUB); link=(-Wl,--allow-multiple-definition); fi
  "$gcc" "${flags[@]}" "${extra[@]}" -c "$here/fssurfio.cpp" -o "$out/$variant.o"
  "$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections "${link[@]}" -O3 "$out/$variant.o" \
    -o "$out/$variant" "${libs[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
    "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
    /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt
done
sha256sum "$out/fssurfio" "$out/fssurfio-mpstub" "$here/fssurfio.cpp" > "$out/BUILD_INFO.txt"
"$gcc" --version | head -1 >> "$out/BUILD_INFO.txt"
echo "built $out/fssurfio and $out/fssurfio-mpstub"
