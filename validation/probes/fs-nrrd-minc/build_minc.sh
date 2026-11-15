#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the MINC originals of the probe (README.md), called by build.sh:
#   libminc_up.so: MINC 1.5.1's libsrc and volume_io (the release tarball, <workspace>/tags/minc-1.5.1)
#                  over netCDF 3.6.0-p1's libsrc (the release tarball, <workspace>/tags/netcdf-3.6.0-p1),
#                  with shim_minc.c, up-include-minc/config.h and up-include-netcdf/ncconfig.h (the two
#                  configure scripts' answers for Linux x86-64);
#   libminc_fs.so: FreeSurfer 8.2.0's libminc.a and libnetcdf.a as the unit oracle built them, with
#                  shim_minc.c compiled against FreeSurfer's packages/minc header (read by the compiler only).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/ (-O3 -DNDEBUG -fopenmp
# -w -fPIC).
# Usage: build_minc.sh <out-dir> [<unit-oracle>] [<minc-1.5.1>] [<netcdf-3.6.0-p1>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build_minc.sh <out-dir> [<unit-oracle>] [<minc-1.5.1>] [<netcdf-3.6.0-p1>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
minc=$(cd "${3:-$ws/tags/minc-1.5.1}" && pwd)
netcdf=$(cd "${4:-$ws/tags/netcdf-3.6.0-p1}" && pwd)
mkdir -p "$out/obj/minc_up"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
AR="$(dirname "$CC")/gcc-ar"
[ -x "$AR" ] || AR=ar
FLAGS=(-O3 -DNDEBUG -fopenmp -w -fPIC)
SHIM=(-Werror=implicit-function-declaration)
nc="$netcdf/src/libsrc"
V="$minc/volume_io"
up_inc=(-I"$here/up-include-minc" -I"$here/up-include-netcdf" -I"$nc" -I"$V/Include" -I"$minc/libsrc")
objs=()
for f in attr dim error libvers nc ncio ncx putget string v1hpg v2i var; do
  o="$out/obj/minc_up/nc_$f.o"
  "$CC" "${FLAGS[@]}" -I"$here/up-include-netcdf" -I"$nc" -c "$nc/$f.c" -o "$o"
  objs+=("$o")
done
for f in netcdf_convenience minc_convenience image_conversion dim_conversion value_conversion \
         minc_error minc_globdef nd_loop time_stamp; do
  o="$out/obj/minc_up/mi_$f.o"
  "$CC" "${FLAGS[@]}" "${up_inc[@]}" -c "$minc/libsrc/$f.c" -o "$o"
  objs+=("$o")
done
for d in Volumes Prog_utils Geometry MNI_formats; do
  for f in "$V/$d"/*.c; do
    o="$out/obj/minc_up/vio_${d}_$(basename "$f" .c).o"
    "$CC" "${FLAGS[@]}" "${up_inc[@]}" -c "$f" -o "$o"
    objs+=("$o")
  done
done
rm -f "$out/obj/minc_up/libminc_up.a"
"$AR" rcs "$out/obj/minc_up/libminc_up.a" "${objs[@]}"
"$CC" "${FLAGS[@]}" "${SHIM[@]}" -DUPSTREAM "${up_inc[@]}" -c "$here/shim_minc.c" -o "$out/obj/shim_minc_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libminc_up.so" \
  "$out/obj/shim_minc_up.o" "$out/obj/minc_up/libminc_up.a" -lm
S="$unit/src/fs-shadow/packages"
B="$unit/build/fs/packages"
fs_inc=(-I"$S/minc" -I"$B/minc" -I"$S/netcdf")
"$CC" "${FLAGS[@]}" "${SHIM[@]}" "${fs_inc[@]}" -c "$here/shim_minc.c" -o "$out/obj/shim_minc_fs.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libminc_fs.so" \
  "$out/obj/shim_minc_fs.o" "$B/minc/libminc.a" "$B/netcdf/libnetcdf.a" -lm
{
  echo "minc: $(cat "$minc/.upstream-sha256")"
  echo "netcdf: $(cat "$netcdf/.upstream-sha256")"
  ( cd "$B" && sha256sum minc/libminc.a netcdf/libnetcdf.a )
} >> "$out/BUILD_INFO.txt"
echo "built $out/libminc_up.so and $out/libminc_fs.so"
