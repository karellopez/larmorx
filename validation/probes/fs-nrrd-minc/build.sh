#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the originals and the Rust driver (README.md):
#   libnrrd_up.so: NrrdIO r3991 (Teem's SVN, <workspace>/tags/NrrdIO-r3991) with shim_nrrd.c, built
#                  as FreeSurfer 8.2.0 builds its copy (TEEM_DIO=0 TEEM_QNANHIBIT=1 TEEM_32BIT=0
#                  TEEM_ENDIAN=1234, no TEEM_ZLIB, no TEEM_BZIP2);
#   libnrrd_fs.so: FreeSurfer 8.2.0's libnrrdio.a as the unit oracle built it, with shim_nrrd.c
#                  compiled against FreeSurfer's packages/nrrdio header (read by the compiler only);
#   libminc_up.so / libminc_fs.so: see build_minc.sh (MINC 1.5.1's volume input, FreeSurfer's).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/ (-O3 -DNDEBUG
# -fopenmp -w -fPIC; its compile_commands.json).
# Usage: build.sh <out-dir> [<unit-oracle>] [<NrrdIO-r3991>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<NrrdIO-r3991>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
nrrdio=$(cd "${3:-$ws/tags/NrrdIO-r3991}" && pwd)
mkdir -p "$out/obj"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
FLAGS=(-O3 -DNDEBUG -fopenmp -w -fPIC)
TEEM=(-DTEEM_DIO=0 -DTEEM_QNANHIBIT=1 -DTEEM_ENDIAN=1234 -DTEEM_32BIT=0)
SHIM=(-Werror=implicit-function-declaration)
objs=()
for f in "$nrrdio"/*.c; do
  case "$(basename "$f")" in sampleIO.c|qnanhibit.c|preamble.c) continue ;; esac
  o="$out/obj/nrrdio_$(basename "$f" .c).o"
  "$CC" "${FLAGS[@]}" "${TEEM[@]}" -I"$here/up-include" -I"$nrrdio" -c "$f" -o "$o"
  objs+=("$o")
done
"$CC" "${FLAGS[@]}" "${TEEM[@]}" "${SHIM[@]}" -I"$here/up-include" -I"$nrrdio" -c "$here/shim_nrrd.c" -o "$out/obj/shim_nrrd_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libnrrd_up.so" \
  "$out/obj/shim_nrrd_up.o" "${objs[@]}" -lm
S="$unit/src/fs-shadow/packages"
B="$unit/build/fs/packages"
"$CC" "${FLAGS[@]}" "${TEEM[@]}" "${SHIM[@]}" -I"$S/nrrdio" -I"$B/nrrdio" -c "$here/shim_nrrd.c" \
  -o "$out/obj/shim_nrrd_fs.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libnrrd_fs.so" \
  "$out/obj/shim_nrrd_fs.o" "$B/nrrdio/libnrrdio.a" -lm
{
  "$CC" --version | head -1
  echo "flags: ${FLAGS[*]} ${TEEM[*]}"
  echo "NrrdIO: $(cat "$nrrdio/.upstream-source")"
  echo "NrrdIO manifest: $(sha256sum "$nrrdio/MANIFEST.sha256" | cut -d' ' -f1)"
  ( cd "$B" && sha256sum nrrdio/libnrrdio.a )
} > "$out/BUILD_INFO.txt"
if [ -x "$here/build_minc.sh" ]; then "$here/build_minc.sh" "$out" "$unit"; fi
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-nrrd-minc" "$out/"
echo "built $out/fs-nrrd-minc"
