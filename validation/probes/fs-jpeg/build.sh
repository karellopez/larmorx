#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the originals and the Rust driver (README.md):
#   libjp_up.so: IJG 6b as released (<workspace>/tags/jpeg-6b, jpegsrc.v6b.tar.gz) with
#                shim_jpeg.c, the source files FreeSurfer 8.2.0's packages/jpeg/CMakeLists.txt
#                lists, and a jconfig.h with the settings IJG's configure writes on Linux;
#   libjp_fs.so: FreeSurfer 8.2.0's libjpeg.a as the unit oracle built it, with shim_jpeg.c
#                compiled against FreeSurfer's packages/jpeg headers (read by the compiler only).
# Both with the unit oracle's GCC 8.4.0 and FreeSurfer's C flags for packages/jpeg (its
# compile_commands.json: -DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux
# -fopenmp -O3 -DNDEBUG -w -fPIC).
# Usage: build.sh <out-dir> [<unit-oracle>] [<jpeg-6b>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<jpeg-6b>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
jpeg=$(cd "${3:-$ws/tags/jpeg-6b}" && pwd)
mkdir -p "$out/obj" "$out/up-include"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CC="$unit/toolchain/bin/gcc"
FLAGS=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -fopenmp -O3 -DNDEBUG -w -fPIC)
# IJG's configure on Linux (jconfig.cfg's choices: prototypes, unsigned char and short, the BSD
# string functions, __inline__)
cat > "$out/up-include/jconfig.h" <<'EOF'
#define HAVE_PROTOTYPES
#define HAVE_UNSIGNED_CHAR
#define HAVE_UNSIGNED_SHORT
#undef void
#undef const
#undef CHAR_IS_UNSIGNED
#undef HAVE_STDDEF_H
#undef HAVE_STDLIB_H
#define NEED_BSD_STRINGS
#undef NEED_SYS_TYPES_H
#undef NEED_FAR_POINTERS
#undef NEED_SHORT_EXTERNAL_NAMES
#undef INCOMPLETE_TYPES_BROKEN
#ifdef JPEG_INTERNALS
#undef RIGHT_SHIFT_IS_UNSIGNED
#define INLINE __inline__
#undef DEFAULT_MAX_MEM
#undef NO_MKTEMP
#endif
EOF
srcs=(jcapimin jcapistd jctrans jcparam jdatadst jcinit jcmaster jcmarker jcmainct jcprepct jccoefct
  jccolor jcsample jchuff jcphuff jcdctmgr jfdctfst jfdctflt jfdctint jdapimin jdapistd jdtrans
  jdatasrc jdmaster jdinput jdmarker jdhuff jdphuff jdmainct jdcoefct jdpostct jddctmgr jidctfst
  jidctflt jidctint jidctred jdsample jdcolor jquant1 jquant2 jdmerge jcomapi jutils jerror jmemmgr
  jmemnobs)
objs=()
for s in "${srcs[@]}"; do
  o="$out/obj/up_$s.o"
  "$CC" "${FLAGS[@]}" -I"$out/up-include" -I"$jpeg" -c "$jpeg/$s.c" -o "$o"
  objs+=("$o")
done
"$CC" "${FLAGS[@]}" -Werror=implicit-function-declaration -I"$out/up-include" -I"$jpeg" \
  -c "$here/shim_jpeg.c" -o "$out/obj/shim_up.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libjp_up.so" \
  "$out/obj/shim_up.o" "${objs[@]}"
S="$unit/src/fs-shadow/packages/jpeg"
B="$unit/build/fs/packages/jpeg"
"$CC" "${FLAGS[@]}" -Werror=implicit-function-declaration -I"$B" -I"$S" -c "$here/shim_jpeg.c" \
  -o "$out/obj/shim_fs.o"
"$CC" -shared -fopenmp -Wl,-Bsymbolic -Wl,--no-undefined -o "$out/libjp_fs.so" \
  "$out/obj/shim_fs.o" "$B/libjpeg.a"
{
  "$CC" --version | head -1
  echo "flags: ${FLAGS[*]}"
  echo "IJG 6b: $(cat "$jpeg/.upstream-sha256" 2>/dev/null || echo '(no checksum file)')"
  ( cd "$B" && sha256sum libjpeg.a )
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-jpeg" "$out/"
echo "built $out/fs-jpeg"
