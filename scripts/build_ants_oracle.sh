#!/usr/bin/env bash
# Build the ANTs command-line programs from the pinned release tags, as parity oracles.
#
# ANTsPy runs most ANTs programs in-process, but not all of them (ImageMath, for one), and
# the system's ANTs may be another version. This builds exactly what UPSTREAM.md pins:
# ITK v5.4.5 from <workspace>/tags/ITK-v5.4.5, with the modules ANTs needs, then ANTs v2.6.5
# from <workspace>/tags/ANTs-v2.6.5 against it. The ITK options mirror ANTs' own SuperBuild
# (SuperBuild/External_ITKv5.cmake): MGHIO, ITKReview, GenericLabelInterpolator,
# AdaptiveDenoising, ITK_LEGACY_REMOVE, no FFTW.
# - Release builds, no -march=native.
# - Source trees are used read-only (out-of-source builds).
# - FSL is removed from PATH: it ships its own compilers and binutils, and FSL binaries must
#   not be run (CLAUDE.md, D7). The system gcc is used.
#
# Usage:
#   scripts/build_ants_oracle.sh [--jobs N] [--out <dir>]
# Output: <out>/bin/<program>, <out>/BUILD_INFO.txt. Takes about an hour on 12 cores.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
ITK_SRC="$WS/tags/ITK-v5.4.5"
ANTS_SRC="$WS/tags/ANTs-v2.6.5"
OUT="$WS/oracles/ants-2.6.5"
JOBS="$(nproc)"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --jobs) JOBS="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[[ -f "$ITK_SRC/CMakeLists.txt" && -f "$ANTS_SRC/CMakeLists.txt" ]] || {
  echo "pinned sources not found under $WS/tags" >&2; exit 1; }

CLEANPATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/fsl' | paste -sd:)"
export PATH="$CLEANPATH" CC=/usr/bin/gcc CXX=/usr/bin/g++
mkdir -p "$OUT/logs"

echo "configuring ITK v5.4.5 ..."
cmake -S "$ITK_SRC" -B "$OUT/itk-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DBUILD_EXAMPLES=OFF \
  -DITK_BUILD_DEFAULT_MODULES=ON \
  -DITK_LEGACY_REMOVE=ON -DITK_FUTURE_LEGACY_REMOVE=OFF -DITKV3_COMPATIBILITY=OFF \
  -DKWSYS_USE_MD5=ON \
  -DModule_MGHIO=ON \
  -DModule_ITKReview=ON \
  -DModule_GenericLabelInterpolator=ON \
  -DModule_AdaptiveDenoising=ON \
  -DITK_WRAPPING=OFF > "$OUT/logs/itk-configure.log" 2>&1
echo "building ITK ..."
cmake --build "$OUT/itk-build" -j "$JOBS" > "$OUT/logs/itk-build.log" 2>&1

echo "configuring ANTs v2.6.5 ..."
cmake -S "$ANTS_SRC" -B "$OUT/ants-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DSUPERBUILD_ANTS=OFF \
  -DUSE_SYSTEM_ITK=ON -DITK_DIR="$OUT/itk-build" \
  -DBUILD_TESTING=OFF -DBUILD_SHARED_LIBS=OFF \
  -DRUN_LONG_TESTS=OFF -DRUN_SHORT_TESTS=OFF > "$OUT/logs/ants-configure.log" 2>&1
echo "building ANTs ..."
cmake --build "$OUT/ants-build" -j "$JOBS" > "$OUT/logs/ants-build.log" 2>&1

mkdir -p "$OUT/bin"
find "$OUT/ants-build" -maxdepth 2 -type f -executable \
  \( -name 'ImageMath' -o -name 'ThresholdImage' -o -name 'SmoothImage' -o -name 'ResampleImage*' \
     -o -name 'MultiplyImages' -o -name 'antsApplyTransforms' -o -name 'antsRegistration' \
     -o -name 'N4BiasFieldCorrection' -o -name 'DenoiseImage' -o -name 'Atropos' -o -name 'antsAI' \
     -o -name 'LabelGeometryMeasures' -o -name 'PrintHeader' -o -name 'CopyImageHeaderInformation' \) \
  -exec cp {} "$OUT/bin/" \;

{
  echo "ANTs: $ANTS_SRC ($(git -C "$ANTS_SRC" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "ITK:  $ITK_SRC ($(git -C "$ITK_SRC" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "compiler: $(/usr/bin/g++ --version | head -1)"
  echo "cmake: $(cmake --version | head -1)"
  echo "host: $(uname -srm), $(ldd --version | head -1)"
  echo "programs: $(ls "$OUT/bin" | tr '\n' ' ')"
  echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$OUT/BUILD_INFO.txt"
cat "$OUT/BUILD_INFO.txt"
