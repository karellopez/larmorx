#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the two original libraries and the Rust driver (README.md):
#   libup.so: MINC 1.5.1's volume_io (the .xfm code) and nifticlib 2.0.0 (nifti1_io.c, znzlib.c), from
#             the pinned release trees, with shim_up.c, up_stubs.c and up-include/config.h;
#   libfs.so: FreeSurfer 8.2.0's libminc.a, libnetcdf.a and libnifti.a as the unit oracle built them,
#             with shim_fs.c compiled against FreeSurfer's packages/ headers (read by the compiler only).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/ (-O3 -DNDEBUG -fopenmp
# -fPIC; its compile_commands.json), linked against the machine's zlib.
# Usage: build.sh <out-dir> [<unit-oracle>] [<minc-1.5.1>] [<nifticlib-2.0.0>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<minc-1.5.1>] [<nifticlib-2.0.0>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
minc=$(cd "${3:-$ws/tags/minc-1.5.1}" && pwd)
nifti=$(cd "${4:-$ws/tags/nifticlib-2.0.0}" && pwd)
mkdir -p "$out/obj"
out=$(cd "$out" && pwd)
# FSL's binutils must not be the ones used (scripts/orchestration/env.sh takes FSL off the path).
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
FLAGS=(-O3 -DNDEBUG -fopenmp -w -fPIC)
# The shims themselves must declare everything they call (a pointer through an implicit int is cut).
SHIM=(-Werror=implicit-function-declaration)
V="$minc/volume_io"
up_inc=(-I"$here/up-include" -I"$V/Include" -I"$minc/libsrc" -I"$nifti/niftilib" -I"$nifti/znzlib")
up_src=(MNI_formats/gen_xf_io.c MNI_formats/gen_xfs.c MNI_formats/mni_io.c MNI_formats/thin_plate_spline.c
        Geometry/transforms.c Geometry/inverse.c Geometry/gaussian.c Geometry/newton.c
        Prog_utils/files.c Prog_utils/string.c Prog_utils/print.c Prog_utils/alloc.c
        Prog_utils/alloc_check.c Prog_utils/arrays.c)
objs=()
for f in "${up_src[@]}"; do
  o="$out/obj/minc_$(basename "$f" .c).o"
  "$CC" "${FLAGS[@]}" "${up_inc[@]}" -c "$V/$f" -o "$o"
  objs+=("$o")
done
for f in niftilib/nifti1_io.c znzlib/znzlib.c; do
  o="$out/obj/nifti_$(basename "$f" .c).o"
  "$CC" "${FLAGS[@]}" -DHAVE_ZLIB "${up_inc[@]}" -c "$nifti/$f" -o "$o"
  objs+=("$o")
done
"$CC" "${FLAGS[@]}" "${SHIM[@]}" "${up_inc[@]}" -c "$here/shim_up.c" -o "$out/obj/shim_up.o"
"$CC" "${FLAGS[@]}" "${up_inc[@]}" -c "$here/up_stubs.c" -o "$out/obj/stubs_up.o"
"$CC" "${FLAGS[@]}" "${SHIM[@]}" -DHAVE_ZLIB "${up_inc[@]}" -c "$here/shim_nifti.c" -o "$out/obj/shim_nifti_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libup.so" \
  "$out/obj/shim_up.o" "$out/obj/stubs_up.o" "$out/obj/shim_nifti_up.o" "${objs[@]}" -lz -lm
S="$unit/src/fs-shadow/packages"
B="$unit/build/fs/packages"
fs_inc=(-I"$S/minc" -I"$B/minc" -I"$S/netcdf" -I"$S/nifti" -I"$B/nifti")
"$CC" "${FLAGS[@]}" "${SHIM[@]}" "${fs_inc[@]}" -c "$here/shim_fs.c" -o "$out/obj/shim_fs.o"
"$CC" "${FLAGS[@]}" "${SHIM[@]}" -DHAVE_ZLIB "${fs_inc[@]}" -c "$here/shim_nifti.c" -o "$out/obj/shim_nifti_fs.o"
libs=("$B/minc/libminc.a" "$B/netcdf/libnetcdf.a" "$B/nifti/libnifti.a")
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libfs.so" \
  "$out/obj/shim_fs.o" "$out/obj/shim_nifti_fs.o" "${libs[@]}" -lz -lm
{
  "$CC" --version | head -1
  echo "flags: ${FLAGS[*]}"
  echo "minc: $(cat "$minc/.upstream-sha256")"
  echo "nifticlib: $(cat "$nifti/.upstream-sha256")"
  ( cd "$B" && sha256sum minc/libminc.a netcdf/libnetcdf.a nifti/libnifti.a )
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-minc-nifti" "$out/"
echo "built $out/fs-minc-nifti, $out/libup.so and $out/libfs.so"
