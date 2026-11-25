#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds fsrobust (fsrobust.cpp) with FreeSurfer 8.2.0's robust-registration files
# (mri_robust_register/ MyMRI.cpp, MyMatrix.cpp, CostFunctions.cpp, Quaternion.cpp, JointHisto.cpp,
# Registration.cpp, RegRobust.cpp, RegPowell.cpp, compiled from the tag) against the unit oracle: its compiler (Ubuntu's gcc-8 8.4.0), the official
# programs' compile flags (the unit oracle's compile_commands.json for mri_normalize, without
# -Werror; the source paths mapped to the official build's, so __FILE__ prints as there), its
# libraries (the official build's utils and packages, ITK 4.13.2's VNL), and Ubuntu 22.04's static
# LAPACK and BLAS (liblapack-dev and libblas-dev 3.10.0-2ubuntu1, the code the official
# mri_robust_register holds byte for byte: validation/investigations/complex-runtime/README.md)
# with gfortran 11's runtime. Output:
# <workspace>/oracles/freesurfer-8.2.0-probes/fs-robust/fsrobust.
#
#   validation/probes/fs-robust/build.sh [<workspace>] [<output folder>]
# (an output folder of its own keeps the recorded build, whose outputs the oracle-output cache holds)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=${1:-$(cd "$here/../../../.." && pwd)}
case "$ws" in */.claude/worktrees/*) ws=${ws%%/larmorx/.claude/worktrees/*} ;; esac
unit=$ws/oracles/freesurfer-8.2.0-unit
b=$unit/build/fs
s=$unit/src/fs-shadow
tag=$ws/tags/freesurfer-8.2.0
rr=$tag/mri_robust_register
itk=$unit/itk-4.13.2
gcc=$unit/toolchain/bin/g++
out=${2:-$ws/oracles/freesurfer-8.2.0-probes/fs-robust}
[ -x "$gcc" ] || { echo "no unit oracle toolchain at $unit (build.py all first)" >&2; exit 1; }
mkdir -p "$out"
flags=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DUSING_DCM2NIIXFSWRAPPER
  -DUSING_MGH_NIFTI_IO -I"$s/include" -I"$b/include" -I"$s/packages/minc" -I"$s/packages/netcdf"
  -I"$s/packages/nifti" -I"$rr" -I"$itk/include/ITK-4.13" -fopenmp -fno-builtin -Wall -Wextra
  -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result
  -Wno-unused-parameter -Wno-bool-compare -Wno-unused-but-set-variable -fPIC -fdata-sections
  -ffunction-sections -O3 -DNDEBUG
  -fmacro-prefix-map="$tag"=/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0
  -fopenmp -std=c++11)
objs=()
for f in MyMRI MyMatrix CostFunctions Quaternion JointHisto Registration RegRobust RegPowell; do
  "$gcc" "${flags[@]}" -c "$rr/$f.cpp" -o "$out/$f.o"
  objs+=("$out/$f.o")
done
"$gcc" "${flags[@]}" -c "$here/fsrobust.cpp" -o "$out/fsrobust.o"
# the decided cabs and csqrt (complexrt.c, CORE-MATH's correctly rounded hypot at its commit in
# tags/core-math), with the system's gcc as validation/probes/coremath-hypot builds hypot.c
/usr/bin/gcc -O2 -ffp-contract=off -c "$ws/tags/core-math/src/binary64/hypot/hypot.c" -o "$out/crhypot.o"
/usr/bin/gcc -O2 -ffp-contract=off -c "$here/complexrt.c" -o "$out/complexrt.o"
# the decided clog (clogrt.c, relay 51) on CORE-MATH's correctly rounded atan2, log1p and log
for f in atan2 log1p log; do
  /usr/bin/gcc -O2 -ffp-contract=off -c "$ws/tags/core-math/src/binary64/$f/$f.c" -o "$out/cr$f.o"
done
/usr/bin/gcc -O2 -ffp-contract=off -c "$here/clogrt.c" -o "$out/clogrt.o"
libs=("$b/utils/libutils.a")
for p in jpeg/libjpeg tiff/libtiff xml2/libxml2 minc/libminc netcdf/libnetcdf nrrdio/libnrrdio \
  dicom/libdicom cephes/libcephes nifti/libnifti dcm2niix/libdcm2niixfs gifti/libgifti expat/libexpat; do
  libs+=("$b/packages/$p.a")
done
for l in vnl_algo vnl v3p_netlib netlib vcl; do libs+=("$itk/lib/libitk$l-4.13.a"); done
lapack=(/usr/lib/x86_64-linux-gnu/lapack/liblapack.a /usr/lib/x86_64-linux-gnu/blas/libblas.a
  /usr/lib/gcc/x86_64-linux-gnu/11/libgfortran.a /usr/lib/gcc/x86_64-linux-gnu/11/libquadmath.a)
"$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections -Wl,-z,muldefs \
  -Wl,--wrap=acos,--wrap=cabs,--wrap=csqrt,--wrap=clog -O3 \
  "$out/fsrobust.o" "${objs[@]}" "$out/complexrt.o" "$out/crhypot.o" "$out/clogrt.o" \
  "$out/cratan2.o" "$out/crlog1p.o" "$out/crlog.o" -o "$out/fsrobust" \
  "${libs[@]}" "${lapack[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
  "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt -ldl
# fsrobust-pade (relay 51): the same probe with MyMatrix.cpp's getPadeCoefficients reading its
# coefficients from static arrays (FS-1311: the official build reads its five local arrays after
# their lifetime ends, uninitialized stack memory), the reference for the matrix exponential's and
# the geometric mean's cases (check.py --family logm); the tag's file is copied, never edited
sed '356,406s/^    double d\[\] =$/    static double d[] =/' "$rr/MyMatrix.cpp" > "$out/MyMatrix_pade.cpp"
[ "$(grep -c '^    static double d\[\] =$' "$out/MyMatrix_pade.cpp")" = 5 ] || { echo "pade patch" >&2; exit 1; }
"$gcc" "${flags[@]}" \
  -fmacro-prefix-map="$out/MyMatrix_pade.cpp"=/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0/mri_robust_register/MyMatrix.cpp \
  -c "$out/MyMatrix_pade.cpp" -o "$out/MyMatrix_pade.o"
pobjs=()
for o in "${objs[@]}"; do
  case "$o" in */MyMatrix.o) pobjs+=("$out/MyMatrix_pade.o") ;; *) pobjs+=("$o") ;; esac
done
"$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections -Wl,-z,muldefs \
  -Wl,--wrap=acos,--wrap=cabs,--wrap=csqrt,--wrap=clog -O3 \
  "$out/fsrobust.o" "${pobjs[@]}" "$out/complexrt.o" "$out/crhypot.o" "$out/clogrt.o" \
  "$out/cratan2.o" "$out/crlog1p.o" "$out/crlog.o" -o "$out/fsrobust-pade" \
  "${libs[@]}" "${lapack[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
  "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt -ldl
# the speed and memory bench (bench.cpp), with the same objects and libraries
"$gcc" "${flags[@]}" -c "$here/bench.cpp" -o "$out/bench.o"
"$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections -Wl,-z,muldefs \
  -Wl,--wrap=cabs,--wrap=csqrt -O3 \
  "$out/bench.o" "${objs[@]}" "$out/complexrt.o" "$out/crhypot.o" -o "$out/bench" \
  "${libs[@]}" "${lapack[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
  "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt -ldl
sha256sum "$out/fsrobust" "$out/fsrobust-pade" "$out/bench" "$here/fsrobust.cpp" "$here/bench.cpp" "$here/complexrt.c" \
  "$here/clogrt.c" "$ws/tags/core-math/src/binary64/atan2/atan2.c" \
  "$ws/tags/core-math/src/binary64/log1p/log1p.c" "$ws/tags/core-math/src/binary64/log/log.c" \
  "$ws/tags/core-math/src/binary64/hypot/hypot.c" "${lapack[@]}" "$ws/oracles/core-math-preload/libcrmath.so" > "$out/BUILD_INFO.txt"
"$gcc" --version | head -1 >> "$out/BUILD_INFO.txt"
echo "built $out/fsrobust, $out/fsrobust-pade and $out/bench"
