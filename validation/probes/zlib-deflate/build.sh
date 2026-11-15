#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds zlib 1.2.11 as released (the pinned <workspace>/tags/zlib-1.2.11) as libz-up.so (its own
# symbols bound inside it, so its gz functions call its own deflate) and the Rust driver. The other
# zlib compared, Ubuntu 22.04's /lib/x86_64-linux-gnu/libz.so.1 (zlib1g 1:1.2.11.dfsg-2ubuntu9.2),
# is the one FreeSurfer 8.2.0's official programs load; its checksum goes into BUILD_INFO.txt.
# Usage: build.sh <out-dir> [<zlib-1.2.11>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<zlib-1.2.11>]}
up=$(cd "${2:-$ws/tags/zlib-1.2.11}" && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
srcs=(adler32.c compress.c crc32.c deflate.c gzclose.c gzlib.c gzread.c gzwrite.c infback.c
  inffast.c inflate.c inftrees.c trees.c uncompr.c zutil.c)
( cd "$up" && gcc -O2 -fPIC -D_LARGEFILE64_SOURCE=1 -DHAVE_HIDDEN -shared -Wl,-Bsymbolic -w \
  -o "$out/libz-up.so" "${srcs[@]}" )
sys=/lib/x86_64-linux-gnu/libz.so.1
{
  gcc --version | head -1
  echo "zlib 1.2.11 as released: $(cat "$up/.upstream-sha256" 2>/dev/null || echo '(no checksum file)')"
  echo "system zlib: $(readlink -f "$sys") $(sha256sum "$(readlink -f "$sys")" | cut -d' ' -f1)"
  dpkg-query -W -f='${Package} ${Version}\n' zlib1g 2>/dev/null || true
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/zlib-deflate" "$out/"
echo "built $out/zlib-deflate and $out/libz-up.so"
