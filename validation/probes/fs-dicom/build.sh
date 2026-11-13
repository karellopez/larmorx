#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the two original libraries and the Rust driver (README.md):
#   libup.so: CTN 3.0.6's object library (dcm.c, dcmdict.c, dcmcond.c, dcmsupport.c), condition
#             stack (condition.c) and lists (lst.c) from the pinned tree, with shim.c and
#             up-include/dicom_platform.h (Linux x86-64, no threads);
#   libfs.so: FreeSurfer 8.2.0's libdicom.a as the unit oracle built it, with shim.c compiled
#             against FreeSurfer's packages/dicom headers (read by the compiler only: the shim
#             calls CTN's public names and walks its private structures).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/ (-O3 -DNDEBUG
# -fopenmp -fPIC, -DLITTLE_ENDIAN_ARCHITECTURE -DLinux; its compile_commands.json).
# Usage: build.sh <out-dir> [<unit-oracle>] [<ctn-3.0.6>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<ctn-3.0.6>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
ctn=$(cd "${3:-$ws/tags/ctn-3.0.6}/facilities" && pwd)
mkdir -p "$out/obj"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
FLAGS=(-O3 -DNDEBUG -fopenmp -w -fPIC -DLITTLE_ENDIAN_ARCHITECTURE -DLinux)
SHIM=(-Werror=implicit-function-declaration)
up_inc=(-DSHORTSIZE=16 -DINTSIZE=32 -DLONGSIZE=64 -I"$here/up-include" -I"$ctn/dicom" -I"$ctn/objects"
  -I"$ctn/condition" -I"$ctn/lst" -I"$ctn/uid" -I"$ctn/thread")
objs=()
for f in objects/dcm.c objects/dcmdict.c objects/dcmcond.c objects/dcmsupport.c condition/condition.c lst/lst.c; do
  o="$out/obj/up_$(basename "$f" .c).o"
  "$CC" "${FLAGS[@]}" "${up_inc[@]}" -c "$ctn/$f" -o "$o"
  objs+=("$o")
done
"$CC" "${FLAGS[@]}" "${SHIM[@]}" "${up_inc[@]}" -c "$here/shim.c" -o "$out/obj/shim_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libup.so" \
  "$out/obj/shim_up.o" "${objs[@]}" -lm
S="$unit/src/fs-shadow/packages/dicom"
B="$unit/build/fs/packages/dicom"
"$CC" "${FLAGS[@]}" "${SHIM[@]}" -I"$S" -I"$B" -c "$here/shim.c" -o "$out/obj/shim_fs.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libfs.so" \
  "$out/obj/shim_fs.o" "$B/libdicom.a" -lm
{
  "$CC" --version | head -1
  echo "flags: ${FLAGS[*]}"
  echo "ctn: $(cat "$ctn/../.upstream-sha256")"
  ( cd "$B" && sha256sum libdicom.a )
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-dicom" "$out/"
echo "built $out/fs-dicom, $out/libup.so and $out/libfs.so"
