#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the originals and the Rust driver (README.md):
#   libtf_up.so: libtiff 3.6.1 as released (<workspace>/tags/libtiff-3.6.1, tiff-v3.6.1.tar.gz)
#                with shim_tiff.c: the source files FreeSurfer 8.2.0's packages/tiff/CMakeLists.txt
#                lists, tif_fax3sm.c made by the release's mkg3states, and a port.h with the
#                settings libtiff's configure writes on x86-64 Linux as FreeSurfer's copy holds
#                them (no HAVE_MMAP: files are never mapped);
#   libtf_fs.so: FreeSurfer 8.2.0's libtiff.a as the unit oracle built it, with shim_tiff.c
#                compiled against FreeSurfer's packages/tiff headers (read by the compiler only).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/tiff (its
# compile_commands.json: -DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux
# -DPIXARLOG_SUPPORT -DZIP_SUPPORT -fopenmp -O3 -DNDEBUG -w -fPIC), linked with the system's
# zlib (Ubuntu 22.04's libz.so.1, the one FreeSurfer's official programs load).
# Usage: build.sh <out-dir> [<unit-oracle>] [<libtiff-3.6.1>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<libtiff-3.6.1>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
tiff=$(cd "${3:-$ws/tags/libtiff-3.6.1}/libtiff" && pwd)
mkdir -p "$out/obj" "$out/up-include"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
FLAGS=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DPIXARLOG_SUPPORT
  -DZIP_SUPPORT -fopenmp -O3 -DNDEBUG -w -fPIC)
cat > "$out/up-include/port.h" <<'PORT'
#ifndef _PORT_
#define _PORT_ 1
#ifdef __cplusplus
extern "C" {
#endif
#include <sys/types.h>
#define HOST_FILLORDER FILLORDER_MSB2LSB
#define HOST_BIGENDIAN	0
#include <stdio.h>
#include <unistd.h>
#include <string.h>
#include <stdlib.h>
#include <fcntl.h>
typedef double dblparam_t;
#ifdef __STRICT_ANSI__
#define	INLINE	__inline__
#else
#define	INLINE	inline
#endif
#define GLOBALDATA(TYPE,NAME)	extern TYPE NAME
#ifdef __cplusplus
}
#endif
#endif
PORT
"$CC" -O2 -w -I"$out/up-include" -I"$tiff" -o "$out/obj/mkg3states" "$tiff/mkg3states.c"
( cd "$out/obj" && rm -f tif_fax3sm.c && ./mkg3states -c const tif_fax3sm.c )
srcs=(tif_aux tif_close tif_codec tif_compress tif_color tif_dir tif_dirinfo tif_dirread
  tif_dirwrite tif_dumpmode tif_error tif_extension tif_fax3 tif_getimage tif_jpeg tif_flush
  tif_luv tif_lzw tif_next tif_ojpeg tif_open tif_packbits tif_pixarlog tif_predict tif_print
  tif_read tif_swab tif_strip tif_thunder tif_tile tif_unix tif_version tif_warning tif_write
  tif_zip)
objs=()
for s in "${srcs[@]}"; do
  o="$out/obj/up_$s.o"
  "$CC" "${FLAGS[@]}" -I"$out/up-include" -I"$tiff" -c "$tiff/$s.c" -o "$o"
  objs+=("$o")
done
"$CC" "${FLAGS[@]}" -I"$out/up-include" -I"$tiff" -c "$out/obj/tif_fax3sm.c" -o "$out/obj/up_tif_fax3sm.o"
objs+=("$out/obj/up_tif_fax3sm.o")
"$CC" "${FLAGS[@]}" -Werror=implicit-function-declaration -I"$out/up-include" -I"$tiff" \
  -c "$here/shim_tiff.c" -o "$out/obj/shim_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libtf_up.so" \
  "$out/obj/shim_up.o" "${objs[@]}" -lz -lm
S="$unit/src/fs-shadow/packages/tiff"
B="$unit/build/fs/packages/tiff"
"$CC" "${FLAGS[@]}" -Werror=implicit-function-declaration -I"$B" -I"$S" -c "$here/shim_tiff.c" \
  -o "$out/obj/shim_fs.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libtf_fs.so" \
  "$out/obj/shim_fs.o" "$B/libtiff.a" -lz -lm
{
  "$CC" --version | head -1
  echo "flags: ${FLAGS[*]}"
  echo "libtiff 3.6.1: $(cat "$tiff/../.upstream-sha256" 2>/dev/null || echo '(no checksum file)')"
  ( cd "$B" && sha256sum libtiff.a )
  echo "zlib: $(readlink -f /lib/x86_64-linux-gnu/libz.so.1)"
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-tiff" "$out/"
echo "built $out/fs-tiff"
