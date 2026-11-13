#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the originals and the Rust driver (README.md): libup.so (shim.c with miniz 1.15 r4 as
# released, C) and libfs.so (shim.c with FreeSurfer 8.2.0's packages/dcm2niix/miniz.c, compiled
# as C++ as nii_dicom_batch.cpp includes it, with its flags), and `facts` (facts.cpp with
# FreeSurfer's include/ and the unit oracle's libnrrdio.a, libcephes.a, libtiff.a), all with
# the unit oracle's GCC 8.4.0.
# Usage: build.sh <out-dir> [<unit-oracle>] [<miniz-1.15r4>] [<freesurfer-8.2.0>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<miniz-1.15r4>] [<freesurfer-8.2.0>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
up=$(cd "${3:-$ws/tags/miniz-1.15r4}" && pwd)
fs=$(cd "${4:-$ws/tags/freesurfer-8.2.0}" && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
CXX="$unit/toolchain/bin/g++"
CFLAGS=(-fopenmp -O3 -DNDEBUG -fPIC -w)
CXXFLAGS=(-DUSING_DCM2NIIXFSWRAPPER -DUSING_MGH_NIFTI_IO -DmyDisableOpenJPEG -DmyEnableJNIFTI
  -fopenmp -msse2 -mfpmath=sse -O3 -DNDEBUG -fPIC -w)
"$CC" "${CFLAGS[@]}" -DMINIZ_FILE="\"$up/miniz.c\"" -shared -Wl,-Bsymbolic \
  -o "$out/libup.so" "$here/shim.c"
"$CXX" "${CXXFLAGS[@]}" -x c++ -DMINIZ_FILE="\"$fs/packages/dcm2niix/miniz.c\"" -shared \
  -Wl,-Bsymbolic -o "$out/libfs.so" "$here/shim.c"
lib="$unit/build/fs/packages"
"$CXX" -O3 -DNDEBUG -w -I"$fs/include" -o "$out/facts" "$here/facts.cpp" \
  "$lib/nrrdio/libnrrdio.a" "$lib/cephes/libcephes.a" "$lib/tiff/libtiff.a" -lz -lm
{
  "$CXX" --version | head -1
  echo "C flags: ${CFLAGS[*]}"
  echo "C++ flags (FreeSurfer's copy): ${CXXFLAGS[*]}"
  echo "miniz 1.15 r4: $(cat "$up/.upstream-sha256")"
  ( cd "$fs" && sha256sum packages/dcm2niix/miniz.c include/fnvhash.h include/fnv_hash.h )
  ( cd "$lib" && sha256sum nrrdio/libnrrdio.a cephes/libcephes.a tiff/libtiff.a )
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-misc" "$out/"
echo "built $out/fs-misc, $out/libup.so, $out/libfs.so and $out/facts"
