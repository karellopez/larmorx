#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build the ANTs command-line programs from the pinned release tags, as parity oracles.
#
# ANTsPy runs most ANTs programs in-process, but not all of them (ImageMath, for one), and
# the system's ANTs may be another version. This builds exactly what UPSTREAM.md pins:
# ITK v5.4.5 from <workspace>/tags/ITK-v5.4.5, with the modules ANTs needs, then ANTs v2.6.5
# from <workspace>/tags/ANTs-v2.6.5 against it. The ITK options mirror ANTs' own SuperBuild
# (SuperBuild/External_ITKv5.cmake): MGHIO, ITKReview, GenericLabelInterpolator,
# AdaptiveDenoising, ITK_LEGACY_REMOVE, no FFTW.
# - Release builds (-O3), ITK's and ANTs' own flags (-march=corei7: no FMA, so no
#   contraction of a*b+c), no -march=native. Source trees are used read-only.
# - The oracle must compute what the C++ source says (CLAUDE.md rule 8). GCC 11.4 and 12.3
#   at -O3 miscompile ITK's B-spline fit (GCC bug 110182: the vectorizer's float -> double
#   round trips are folded away; docs/findings/ants-n4.md), so before building, a probe
#   checks that the compiler keeps float roundings at -O3 and the build stops if it does not
#   (--allow-miscompile overrides, only to rebuild such an oracle as evidence). GCC >= 11.5,
#   12.4 or 13.2 is fixed; Ubuntu 22.04's system compilers are not. The oracle in use is built
#   with conda-forge's GCC 14.3 (the compiler of fMRIPrep 25.2.6's ANTs), in a user-level
#   environment (no system packages):
#     conda create -p <ws>/oracles/toolchains/gcc-14.3 -c conda-forge --override-channels \
#       gxx_linux-64=14.3.0 gcc_linux-64=14.3.0
#     scripts/build_ants_oracle.sh --jobs 4 --out <ws>/oracles/ants-2.6.5-src \
#       --cc <ws>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-gcc \
#       --cxx <ws>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++ \
#       --ldflags "-static-libstdc++ -static-libgcc"
#   (static libstdc++, so the programs run without the environment).
# - FSL is removed from PATH: it ships its own compilers and binutils, which must not be
#   picked up by accident.
# - The compilers, the flags given and the flags each build used are recorded in
#   BUILD_INFO.txt.
#
# Usage:
#   scripts/build_ants_oracle.sh [--jobs N] [--out <dir>] [--cc <c compiler>]
#     [--cxx <c++ compiler>] [--cflags "<flags>"] [--cxxflags "<flags>"] [--ldflags "<flags>"]
#     [--allow-miscompile] [--workspace <ws>]
# Defaults: --jobs $(nproc), --out <ws>/oracles/ants-2.6.5, --cc /usr/bin/gcc,
# --cxx /usr/bin/g++, no extra flags; <ws> is the directory above the repository (give it
# when the script runs from a worktree). Output: <out>/bin/<program>, <out>/BUILD_INFO.txt.
# Takes about an hour on 12 cores, three with 4.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
OUT=""
JOBS="$(nproc)"
CC_BIN=/usr/bin/gcc
CXX_BIN=/usr/bin/g++
EXTRA_CFLAGS=""
EXTRA_CXXFLAGS=""
EXTRA_LDFLAGS=""
ALLOW_MISCOMPILE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --jobs) JOBS="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --cc) CC_BIN="$2"; shift 2 ;;
    --cxx) CXX_BIN="$2"; shift 2 ;;
    --cflags) EXTRA_CFLAGS="$2"; shift 2 ;;
    --cxxflags) EXTRA_CXXFLAGS="$2"; shift 2 ;;
    --ldflags) EXTRA_LDFLAGS="$2"; shift 2 ;;
    --allow-miscompile) ALLOW_MISCOMPILE=1; shift ;;
    --workspace) WS="$(cd "$2" && pwd)"; shift 2 ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
ITK_SRC="$WS/tags/ITK-v5.4.5"
ANTS_SRC="$WS/tags/ANTs-v2.6.5"
[[ -n "$OUT" ]] || OUT="$WS/oracles/ants-2.6.5"
[[ -f "$ITK_SRC/CMakeLists.txt" && -f "$ANTS_SRC/CMakeLists.txt" ]] || {
  echo "pinned sources not found under $WS/tags" >&2; exit 1; }
[[ -x "$CC_BIN" && -x "$CXX_BIN" ]] || { echo "compiler not found: $CC_BIN / $CXX_BIN" >&2; exit 1; }

CLEANPATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/fsl' | paste -sd:)"
# The compiler's own directory first, so a toolchain's binutils (ar, ranlib) go with it.
export PATH="$(dirname "$CXX_BIN"):$CLEANPATH"
export CC="$CC_BIN" CXX="$CXX_BIN"
unset CFLAGS CXXFLAGS LDFLAGS CPPFLAGS
mkdir -p "$OUT/logs" "$OUT/probe"

# --- Does the compiler keep float roundings at -O3? -----------------------------------------
# ITK's BSplineScatteredDataPointSetToImageFilter::ThreadedGenerateDataForFitting in miniature:
# r[i] is a float array assigned from a double quotient, then used in a double expression.
cat > "$OUT/probe/float_roundings.cxx" <<'EOF'
#include <cstdio>
template <typename T, unsigned D> struct FixedArray {
  T m[D];
  T & operator[](unsigned i) { return m[i]; }
};
struct Filter { unsigned cp[3], order[3]; unsigned long size[3]; double spacing[3], origin[3]; float eps; };
__attribute__((noinline)) void fit(const Filter & f, const float * pts, int n, float * out)
{
  FixedArray<float, 3> r, epsilon, p;
  for (unsigned i = 0; i < 3; ++i)
  {
    r[i] = static_cast<float>(f.cp[i] - f.order[i]) / (static_cast<float>(f.size[i] - 1) * f.spacing[i]);
    epsilon[i] = r[i] * f.spacing[i] * f.eps;
  }
  for (int k = 0; k < n; ++k)
    for (unsigned i = 0; i < 3; ++i)
    {
      p[i] = (pts[3 * k + i] - f.origin[i]) * r[i];
      out[3 * k + i] = p[i];
    }
  out[3 * n] = epsilon[0];
}
int main()
{
  Filter f{ { 5, 5, 5 }, { 3, 3, 3 }, { 47, 59, 31 }, { 3.0, 3.0, 4.0 }, { 0.3, -0.7, 1.1 }, 1e-3f };
  const int n = 40;
  float pts[3 * n], out[3 * n + 1];
  for (int k = 0; k < 3 * n; ++k) pts[k] = 0.3f + 1.37f * k;
  fit(f, pts, n, out);
  int bad = 0;
  for (int k = 0; k < n; ++k)
    for (unsigned i = 0; i < 3; ++i)
    {
      volatile float r = static_cast<float>(f.cp[i] - f.order[i]) / (static_cast<float>(f.size[i] - 1) * f.spacing[i]);
      volatile float p = (pts[3 * k + i] - f.origin[i]) * r;
      if (p != out[3 * k + i]) ++bad;
    }
  std::printf("%d\n", bad);
  return 0;
}
EOF
PROBE_FLAGS="-std=c++17 -mtune=native -march=corei7 $EXTRA_CXXFLAGS -O3 -DNDEBUG"
# shellcheck disable=SC2086
"$CXX_BIN" $PROBE_FLAGS "$OUT/probe/float_roundings.cxx" -o "$OUT/probe/float_roundings" $EXTRA_LDFLAGS
PROBE_BAD="$("$OUT/probe/float_roundings")"
if [[ "$PROBE_BAD" == "0" ]]; then
  PROBE="keeps the source's float roundings at -O3 (0 of 120 values differ)"
else
  PROBE="MISCOMPILES float roundings at -O3 ($PROBE_BAD of 120 values differ; GCC bug 110182)"
  if [[ "$ALLOW_MISCOMPILE" != 1 ]]; then
    echo "$CXX_BIN $PROBE." >&2
    echo "Use a fixed compiler (GCC >= 11.5, 12.4 or 13.2; see the header), or --allow-miscompile." >&2
    exit 1
  fi
fi
echo "compiler probe: $PROBE"

echo "configuring ITK v5.4.5 ..."
cmake -S "$ITK_SRC" -B "$OUT/itk-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER="$CC_BIN" -DCMAKE_CXX_COMPILER="$CXX_BIN" \
  -DCMAKE_C_FLAGS="$EXTRA_CFLAGS" -DCMAKE_CXX_FLAGS="$EXTRA_CXXFLAGS" \
  -DCMAKE_EXE_LINKER_FLAGS="$EXTRA_LDFLAGS" \
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

PROGRAMS=(ImageMath ThresholdImage SmoothImage ResampleImage ResampleImageBySpacing
  MultiplyImages antsApplyTransforms antsRegistration N4BiasFieldCorrection DenoiseImage
  Atropos antsAI antsAffineInitializer LabelGeometryMeasures PrintHeader
  CopyImageHeaderInformation)
echo "configuring ANTs v2.6.5 ..."
cmake -S "$ANTS_SRC" -B "$OUT/ants-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER="$CC_BIN" -DCMAKE_CXX_COMPILER="$CXX_BIN" \
  -DCMAKE_C_FLAGS="$EXTRA_CFLAGS" -DCMAKE_CXX_FLAGS="$EXTRA_CXXFLAGS" \
  -DCMAKE_EXE_LINKER_FLAGS="$EXTRA_LDFLAGS" \
  -DANTS_SUPERBUILD=OFF \
  -DUSE_SYSTEM_ITK=ON -DITK_DIR="$OUT/itk-build" \
  -DBUILD_TESTING=OFF -DBUILD_SHARED_LIBS=OFF \
  -DRUN_LONG_TESTS=OFF -DRUN_SHORT_TESTS=OFF > "$OUT/logs/ants-configure.log" 2>&1
echo "building ANTs (${PROGRAMS[*]}) ..."
cmake --build "$OUT/ants-build" -j "$JOBS" --target "${PROGRAMS[@]}" > "$OUT/logs/ants-build.log" 2>&1

mkdir -p "$OUT/bin"
for p in "${PROGRAMS[@]}"; do
  f="$(find "$OUT/ants-build" -maxdepth 4 -type f -executable -name "$p" | head -1)"
  [[ -n "$f" ]] || { echo "program $p was not built" >&2; exit 1; }
  cp "$f" "$OUT/bin/"
done

# The flags each build really used: one ITK and one ANTs compile command, without includes.
flags_of() {
  ninja -C "$1" -t commands "$2" 2>/dev/null | grep -E -- " -c [^ ]*/$3\$" | head -1 \
    | sed -E 's/ -(I|isystem ?)[^ ]+//g; s/ -D[^ ]+//g; s/ -MD//; s/ -M[TF] [^ ]+//g; s/ -o [^ ]+//; s/ -c [^ ]+$//'
}
{
  echo "ANTs: $ANTS_SRC ($(git -C "$ANTS_SRC" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "ITK:  $ITK_SRC ($(git -C "$ITK_SRC" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "compiler: $("$CXX_BIN" --version | head -1)"
  echo "CC: $CC_BIN"
  echo "CXX: $CXX_BIN"
  echo "extra flags: CFLAGS='$EXTRA_CFLAGS' CXXFLAGS='$EXTRA_CXXFLAGS' LDFLAGS='$EXTRA_LDFLAGS'"
  echo "compiler probe (float roundings, $PROBE_FLAGS): $PROBE"
  echo "ITK compile: $(flags_of "$OUT/itk-build" lib/libITKCommon-5.4.a itkObject.cxx)"
  echo "ANTs compile: $(flags_of "$OUT/ants-build" Examples/N4BiasFieldCorrection N4BiasFieldCorrection.cxx)"
  echo "cmake: $(cmake --version | head -1)"
  echo "host: $(uname -srm), $(ldd --version | head -1)"
  echo "programs: $(ls "$OUT/bin" | tr '\n' ' ')"
  echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$OUT/BUILD_INFO.txt"
cat "$OUT/BUILD_INFO.txt"
