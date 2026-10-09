#!/usr/bin/env bash
# Build AFNI command-line programs from the pinned release tag, as parity oracles.
#
# AFNI publishes binaries only for its latest version, so the oracle for the version we port
# (UPSTREAM.md: AFNI_25.2.09) is built from source with AFNI's own standard Linux build
# (Makefile.linux_ubuntu_16_64: gcc -O2 -m64 -fPIC). Only the text-mode library is needed.
# The few objects that need Motif headers (GUI colour bars) are skipped. The library is
# archived from every compiled object that does not define main().
#
# Usage:
#   scripts/build_afni_oracle.sh [--src <workspace>/tags/AFNI-25.2.09] [--out <dir>] [program ...]
# Default programs: 3dTshift. Output: <out>/bin/<program>, <out>/BUILD_INFO.txt.
#
# The source tree is copied, never modified. FSL is removed from PATH for the build:
# FSL ships its own `ld`, and FSL binaries must not be run (CLAUDE.md, D7).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
SRC="$WS/tags/AFNI-25.2.09"
OUT="$WS/oracles/afni-25.2.09"
PROGRAMS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --src) SRC="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) PROGRAMS+=("$1"); shift ;;
  esac
done
[[ ${#PROGRAMS[@]} -eq 0 ]] && PROGRAMS=(3dTshift)
[[ -f "$SRC/src/3dTshift.c" ]] || { echo "AFNI source not found at $SRC" >&2; exit 1; }

CLEANPATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/fsl' | paste -sd:)"
BUILD="$OUT/build"
rm -rf "$BUILD" && mkdir -p "$BUILD" "$OUT/bin"
cp -r "$SRC/src" "$BUILD/"
cd "$BUILD/src"
cp Makefile.linux_ubuntu_16_64 Makefile
# The Makefile derives AFNI_version.h from git metadata, which the copy does not have.
VERSION="$(cat AFNI_version_base.txt)"
cat > AFNI_version.h <<EOF
#undef  AFNI_VERSION_LABEL
#define AFNI_VERSION_LABEL    "$VERSION"
#undef  AFNI_VERSION_PLATFORM
#define AFNI_VERSION_PLATFORM "linux_ubuntu_16_64"
EOF
echo "$VERSION" > AFNI_version.txt

echo "compiling AFNI's library objects (serially: the niml sub-make races under -j) ..."
env PATH="$CLEANPATH" make -k libmri.a > build-libmri.log 2>&1 || true
env PATH="$CLEANPATH" make libf2c.a > build-f2c.log 2>&1

objects=()
for o in *.o niml/*.o; do
  [[ -f "$o" ]] || continue
  nm "$o" 2>/dev/null | grep -qE ' T main$' && continue
  objects+=("$o")
done
rm -f libmri.a
ar q libmri.a "${objects[@]}" 2>/dev/null
ranlib libmri.a

for prog in "${PROGRAMS[@]}"; do
  env PATH="$CLEANPATH" make "$prog" > "build-$prog.log" 2>&1
  cp "$prog" "$OUT/bin/"
done

{
  echo "AFNI version: $VERSION"
  echo "source: $SRC ($(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "makefile: Makefile.linux_ubuntu_16_64"
  echo "compiler: $(env PATH="$CLEANPATH" gcc --version | head -1)"
  echo "host: $(uname -srm), $(ldd --version | head -1)"
  echo "library objects: ${#objects[@]} (skipped: objects needing Motif headers)"
  echo "programs: ${PROGRAMS[*]}"
  echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$OUT/BUILD_INFO.txt"
cat "$OUT/BUILD_INFO.txt"
