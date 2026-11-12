#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds fs_rng_probe against the unit oracle's FreeSurfer 8.2.0 libraries (queue item 3.1-02:
# <workspace>/oracles/freesurfer-8.2.0-unit), with its compiler (Ubuntu's GCC 8.4.0) and
# FreeSurfer's flags. Usage: build.sh <workspace> <output-binary>
set -euo pipefail
W=${1:?workspace}
OUT=${2:?output binary}
U="$W/oracles/freesurfer-8.2.0-unit"
B="$U/build/fs"
S="$U/src/fs-shadow"
I="$U/itk-4.13.2"
HERE=$(cd "$(dirname "$0")" && pwd)
"$U/toolchain/bin/g++" -fopenmp -fno-builtin -O3 -DNDEBUG -std=c++11 -fPIC \
  -DHAVE_OPENMP -DHAVE_ZLIB -DLinux \
  -I"$S/include" -I"$B/include" -I"$S/packages/minc" -I"$S/packages/netcdf" -I"$S/packages/nifti" \
  -isystem "$I/include/ITK-4.13" \
  -c "$HERE/fs_rng_probe.cpp" -o "$OUT.o"
"$U/toolchain/bin/g++" -fopenmp -O3 -Wl,--gc-sections "$OUT.o" -o "$OUT" \
  "$B/utils/libutils.a" "$B/packages/jpeg/libjpeg.a" "$B/packages/tiff/libtiff.a" \
  "$B/packages/xml2/libxml2.a" "$B/packages/minc/libminc.a" "$B/packages/netcdf/libnetcdf.a" \
  "$B/packages/nrrdio/libnrrdio.a" "$B/packages/dicom/libdicom.a" "$B/packages/cephes/libcephes.a" \
  "$B/packages/nifti/libnifti.a" "$B/packages/dcm2niix/libdcm2niixfs.a" "$B/packages/gifti/libgifti.a" \
  "$B/packages/expat/libexpat.a" /usr/lib/x86_64-linux-gnu/libz.so \
  "$I/lib/libitkvnl_algo-4.13.a" "$I/lib/libitkvnl-4.13.a" "$I/lib/libitkv3p_netlib-4.13.a" \
  "$I/lib/libitknetlib-4.13.a" "$I/lib/libitkvcl-4.13.a" -lm \
  "$W/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt
rm -f "$OUT.o"
echo "built $OUT"
