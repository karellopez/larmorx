#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build AFNI command-line programs from the pinned release tag, as parity oracles.
#
# AFNI publishes binaries only for its latest version, so the oracle for the version we port
# (UPSTREAM.md: AFNI_25.2.09) is built from source with AFNI's own standard Linux build
# (Makefile.linux_ubuntu_16_64: gcc -O2 -m64 -fPIC). Only the text-mode library is needed.
# The few objects that need Motif headers (GUI colour bars) are skipped. The library is
# archived from every compiled object that does not define main(). Each program is compiled
# by its own Makefile rule, so it gets AFNI's own flags: 3dTshift and 3dAutomask with $(CC),
# 3dUnifize with $(CCOMP) = $(CCFAST) -fopenmp -DUSE_OMP (OpenMP, as AFNI distributes it).
#
# Usage:
#   scripts/build_afni_oracle.sh [--src <workspace>/tags/AFNI-25.2.09] [--out <dir>] [program ...]
#   scripts/build_afni_oracle.sh --add [--out <dir>] program ...
# Default programs: 3dTshift 3dAutomask 3dUnifize. Output: <out>/bin/<program>,
# <out>/BUILD_INFO.txt. With --add, the programs are compiled against the library an earlier
# run left in <out>/build (same source, compiler and flags), the programs already in <out>/bin
# are left as they are, and BUILD_INFO.txt gets a section for the new ones.
#
# The source tree is copied, never modified. FSL is removed from PATH for the build:
# FSL ships its own `ld`, and FSL binaries must not be run (CLAUDE.md, D7).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
SRC="$WS/tags/AFNI-25.2.09"
OUT="$WS/oracles/afni-25.2.09"
PROGRAMS=()
ADD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --src) SRC="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --add) ADD=1; shift ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *) PROGRAMS+=("$1"); shift ;;
  esac
done
[[ ${#PROGRAMS[@]} -eq 0 ]] && PROGRAMS=(3dTshift 3dAutomask 3dUnifize)
[[ -f "$SRC/src/3dTshift.c" ]] || { echo "AFNI source not found at $SRC" >&2; exit 1; }

CLEANPATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/fsl' | paste -sd:)"
BUILD="$OUT/build"

# The flags each program's Makefile rule compiles it with (recorded in BUILD_INFO.txt).
program_flags() {
  local prog="$1" rule
  rule="$(make -s -n -B -f Makefile "$prog.o" 2>/dev/null | grep -E '(gcc|cc) ' | head -1 || true)"
  echo "${rule:-$(make -s -n -B -f Makefile "$prog" 2>/dev/null | grep -E '(gcc|cc) ' | head -1)}"
}

if [[ $ADD -eq 1 ]]; then
  [[ -f "$BUILD/src/libmri.a" ]] || { echo "--add needs an earlier build in $BUILD" >&2; exit 1; }
  cd "$BUILD/src"
  for prog in "${PROGRAMS[@]}"; do
    [[ -f "$SRC/src/$prog.c" ]] || { echo "no $prog.c in $SRC/src" >&2; exit 1; }
    # The program's own source, copied from the tag (the build tree is a copy of it).
    cmp -s "$SRC/src/$prog.c" "$prog.c" || { echo "$prog.c differs from the tag" >&2; exit 1; }
    rm -f "$prog" "$prog.o"
    env PATH="$CLEANPATH" make "$prog" > "build-$prog.log" 2>&1
    cp "$prog" "$OUT/bin/"
  done
  {
    echo
    echo "added: ${PROGRAMS[*]} ($(date -u +%Y-%m-%dT%H:%M:%SZ)), against the library above"
    echo "compiler: $(env PATH="$CLEANPATH" gcc --version | head -1)"
    for prog in "${PROGRAMS[@]}"; do
      echo "  $prog: $(PATH="$CLEANPATH" program_flags "$prog")"
      echo "  $prog sha256: $(sha256sum "$OUT/bin/$prog" | cut -d' ' -f1)"
    done
  } >> "$OUT/BUILD_INFO.txt"
  cat "$OUT/BUILD_INFO.txt"
  exit 0
fi

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
# The programs find AFNI's own shared libraries next to themselves (rpath $ORIGIN).
for lib in lib*.so; do
  [[ -f "$lib" ]] && cp "$lib" "$OUT/bin/"
done

{
  echo "AFNI version: $VERSION"
  echo "source: $SRC ($(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "makefile: Makefile.linux_ubuntu_16_64"
  echo "compiler: $(env PATH="$CLEANPATH" gcc --version | head -1)"
  echo "host: $(uname -srm), $(ldd --version | head -1)"
  echo "library objects: ${#objects[@]} (skipped: objects needing Motif headers)"
  echo "programs: ${PROGRAMS[*]}"
  for prog in "${PROGRAMS[@]}"; do
    echo "  $prog: $(PATH="$CLEANPATH" program_flags "$prog")"
    echo "  $prog sha256: $(sha256sum "$OUT/bin/$prog" | cut -d' ' -f1)"
  done
  echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$OUT/BUILD_INFO.txt"
cat "$OUT/BUILD_INFO.txt"
