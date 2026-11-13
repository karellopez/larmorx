#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the two original libraries and the Rust driver (README.md):
#   libup.so: gifticlib 1.12 (gifti_io.c, gifti_xml.c), expat 2.0.1 (xmlparse.c, xmlrole.c,
#             xmltok.c) and nifticlib 2.0.0 (nifti1_io.c, znzlib.c) from the pinned trees, with
#             shim.c and up-include/expat_config.h;
#   libfs.so: FreeSurfer 8.2.0's libgifti.a, libexpat.a and libnifti.a as the unit oracle built
#             them, with shim.c compiled against FreeSurfer's packages/ headers (read by the compiler
#             only: the shim calls gifticlib's and expat's public names).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/ (-O3 -DNDEBUG
# -fopenmp -fPIC; its compile_commands.json), linked against the machine's zlib.
# Usage: build.sh <out-dir> [<unit-oracle>] [<gifticlib-1.12>] [<expat-2.0.1>] [<nifticlib-2.0.0>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<gifticlib>] [<expat>] [<nifticlib>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
gifti=$(cd "${3:-$ws/tags/gifticlib-1.12}" && pwd)
expat=$(cd "${4:-$ws/tags/expat-2.0.1}/expat/lib" && pwd)
nifti=$(cd "${5:-$ws/tags/nifticlib-2.0.0}" && pwd)
mkdir -p "$out/obj"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
FLAGS=(-O3 -DNDEBUG -fopenmp -w -fPIC)
SHIM=(-Werror=implicit-function-declaration)
up_inc=(-I"$here/up-include" -I"$gifti" -I"$expat" -I"$nifti/niftilib" -I"$nifti/znzlib")
objs=()
for f in "$expat/xmlparse.c" "$expat/xmlrole.c" "$expat/xmltok.c"; do
  o="$out/obj/expat_$(basename "$f" .c).o"
  "$CC" "${FLAGS[@]}" -DHAVE_EXPAT_CONFIG_H "${up_inc[@]}" -c "$f" -o "$o"
  objs+=("$o")
done
for f in "$gifti/gifti_io.c" "$gifti/gifti_xml.c" "$nifti/niftilib/nifti1_io.c" "$nifti/znzlib/znzlib.c"; do
  o="$out/obj/up_$(basename "$f" .c).o"
  "$CC" "${FLAGS[@]}" -DHAVE_ZLIB "${up_inc[@]}" -c "$f" -o "$o"
  objs+=("$o")
done
"$CC" "${FLAGS[@]}" "${SHIM[@]}" -DHAVE_ZLIB "${up_inc[@]}" -c "$here/shim.c" -o "$out/obj/shim_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libup.so" \
  "$out/obj/shim_up.o" "${objs[@]}" -lz -lm
S="$unit/src/fs-shadow/packages"
B="$unit/build/fs/packages"
fs_inc=(-I"$S/gifti" -I"$B/gifti" -I"$S/expat" -I"$B/expat" -I"$S/nifti" -I"$B/nifti")
"$CC" "${FLAGS[@]}" "${SHIM[@]}" -DHAVE_ZLIB "${fs_inc[@]}" -c "$here/shim.c" -o "$out/obj/shim_fs.o"
libs=("$B/gifti/libgifti.a" "$B/expat/libexpat.a" "$B/nifti/libnifti.a")
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libfs.so" \
  "$out/obj/shim_fs.o" "${libs[@]}" -lz -lm
{
  "$CC" --version | head -1
  echo "flags: ${FLAGS[*]}"
  echo "gifticlib: $(cat "$gifti/.upstream-sha256")"
  echo "expat: $(cat "$expat/../../.upstream-sha256")"
  echo "nifticlib: $(cat "$nifti/.upstream-sha256")"
  ( cd "$B" && sha256sum gifti/libgifti.a expat/libexpat.a nifti/libnifti.a )
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-gifti" "$out/"
echo "built $out/fs-gifti, $out/libup.so and $out/libfs.so"
