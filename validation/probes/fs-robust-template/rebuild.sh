#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Rebuilds FreeSurfer 8.2.0's mri_robust_template from the tag (track C, relay 51, queue 3.2-15)
# with the unit oracle's compiler, the official programs' flags and libraries (as fs-robust's
# build.sh builds its probe; CMake's list of files for the program, mri_robust_register/
# CMakeLists.txt, with `-z muldefs` and Ubuntu 22.04's static LAPACK and BLAS), in two builds:
#
# - mri_robust_template: the tag's files as they are. check.py --exe compares it with the official
#   binary (it must give the official outputs, so the second build can stand in for it);
# - pade/mri_robust_template: the same with MyMatrix.cpp's getPadeCoefficients reading its
#   coefficients from static arrays (FS-1311: the official build reads its five local arrays
#   after their lifetime ends, uninitialized stack memory, so its affine mean space, --affine
#   without --fixtp, is NaN or arbitrary): the reference for those cases (check.py --pade; a
#   folder of its own, so the program's name is the same).
#
# The tag's files are copied, never edited. Output:
# <workspace>/oracles/freesurfer-8.2.0-probes/fs-template-rebuild/.
#
#   validation/probes/fs-robust-template/rebuild.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
case "$here" in
  */larmorx/.claude/worktrees/*) guess=${here%%/larmorx/.claude/worktrees/*} ;;
  *) guess=$(cd "$here/../../../.." && pwd) ;;
esac
ws=${1:-$guess}
unit=$ws/oracles/freesurfer-8.2.0-unit
b=$unit/build/fs
s=$unit/src/fs-shadow
tag=$ws/tags/freesurfer-8.2.0
rr=$tag/mri_robust_register
itk=$unit/itk-4.13.2
gcc=$unit/toolchain/bin/g++
out=$ws/oracles/freesurfer-8.2.0-probes/fs-template-rebuild
official_root=/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0
[ -x "$gcc" ] || { echo "no unit oracle toolchain at $unit" >&2; exit 1; }
mkdir -p "$out"
# the help text's header (add_help: xxd -i from the source folder)
(cd "$rr" && xxd -i mri_robust_template.help.xml "$out/mri_robust_template.help.xml.h")
flags=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DUSING_DCM2NIIXFSWRAPPER
  -DUSING_MGH_NIFTI_IO -I"$s/include" -I"$b/include" -I"$s/packages/minc" -I"$s/packages/netcdf"
  -I"$s/packages/nifti" -I"$out" -I"$rr" -I"$itk/include/ITK-4.13" -fopenmp -fno-builtin -Wall -Wextra
  -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result
  -Wno-unused-parameter -Wno-bool-compare -Wno-unused-but-set-variable -fPIC -fdata-sections
  -ffunction-sections -Wl,--gc-sections -O3 -DNDEBUG -fmacro-prefix-map="$tag"="$official_root"
  -fopenmp -std=c++11)
objs=()
for f in mri_robust_template Registration RegRobust CostFunctions MyMatrix MyMRI Quaternion \
  MultiRegistration; do
  "$gcc" "${flags[@]}" -c "$rr/$f.cpp" -o "$out/$f.o"
  objs+=("$out/$f.o")
done
sed '356,406s/^    double d\[\] =$/    static double d[] =/' "$rr/MyMatrix.cpp" > "$out/MyMatrix_pade.cpp"
[ "$(grep -c '^    static double d\[\] =$' "$out/MyMatrix_pade.cpp")" = 5 ] || { echo "pade patch" >&2; exit 1; }
"$gcc" "${flags[@]}" -fmacro-prefix-map="$out/MyMatrix_pade.cpp"="$official_root/mri_robust_register/MyMatrix.cpp" \
  -c "$out/MyMatrix_pade.cpp" -o "$out/MyMatrix_pade.o"
libs=("$b/utils/libutils.a")
for p in jpeg/libjpeg tiff/libtiff xml2/libxml2 minc/libminc netcdf/libnetcdf nrrdio/libnrrdio \
  dicom/libdicom cephes/libcephes nifti/libnifti dcm2niix/libdcm2niixfs gifti/libgifti expat/libexpat; do
  libs+=("$b/packages/$p.a")
done
lapack=(/usr/lib/x86_64-linux-gnu/lapack/liblapack.a /usr/lib/x86_64-linux-gnu/blas/libblas.a
  /usr/lib/gcc/x86_64-linux-gnu/11/libgfortran.a /usr/lib/gcc/x86_64-linux-gnu/11/libquadmath.a)
link() {
  local exe=$1
  shift
  "$gcc" -fdata-sections -ffunction-sections -Wl,--gc-sections -Wl,-z,muldefs -fopenmp -O3 "$@" \
    -o "$out/$exe" "${libs[@]}" /usr/lib/x86_64-linux-gnu/libz.so \
    "$itk/lib/libitkvnl_algo-4.13.a" "$itk/lib/libitkvnl-4.13.a" "$itk/lib/libitkv3p_netlib-4.13.a" \
    "$itk/lib/libitknetlib-4.13.a" "$itk/lib/libitkvcl-4.13.a" "${lapack[@]}" -lm \
    "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
    /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt -ldl
}
link mri_robust_template "${objs[@]}"
pobjs=()
for o in "${objs[@]}"; do
  case "$o" in */MyMatrix.o) pobjs+=("$out/MyMatrix_pade.o") ;; *) pobjs+=("$o") ;; esac
done
mkdir -p "$out/pade"
link pade/mri_robust_template "${pobjs[@]}"
sha256sum "$out/mri_robust_template" "$out/pade/mri_robust_template" "$here/rebuild.sh" \
  "${lapack[@]}" > "$out/BUILD_INFO.txt"
"$gcc" --version | head -1 >> "$out/BUILD_INFO.txt"
nm -D "$out/mri_robust_template" | grep -E ' U (acos|cabs|clog|csqrt)@' >> "$out/BUILD_INFO.txt"
echo "built $out/mri_robust_template and $out/pade/mri_robust_template"
