#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the preload that gives FreeSurfer's official mri_robust_template the functions the project
# decided where FreeSurfer's results reach the C library's last bit: the complex absolute value and
# square root of the matrix square roots (fs-robust-register's decided_complex.sh: validation/probes/
# fs-robust/complexrt.c's cabs and csqrt on CORE-MATH's correctly rounded hypot, the user's D4) and
# acos (acosrt.c: CORE-MATH's correctly rounded acos, glibc's for a NaN argument: the decision of
# 2026-11-04 for glibc's acos, applied to FreeSurfer; the quaternions' angles of the mean rotation
# and the registrations' rotation distances) and clog (fs-robust's clogrt.c, relay 51: the affine
# mean space's matrix logarithm), the system's gcc as fs-robust's build.sh builds them.
# check.py --preload <lib> and real.py --preload <lib> run the official program with it
# (LD_PRELOAD): its dynamic acos@GLIBC_2.2.5, cabs@GLIBC_2.2.5 and csqrt@GLIBC_2.2.5 take these.
# Output: <workspace>/oracles/freesurfer-8.2.0-probes/fs-template-decided/libdecidedtemplate.so.
#
#   validation/probes/fs-robust-template/decided.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
# the workspace: the argument, else the checkout's parent (a worktree's: the main checkout's)
case "$here" in
  */larmorx/.claude/worktrees/*) guess=${here%%/larmorx/.claude/worktrees/*} ;;
  *) guess=$(cd "$here/../../../.." && pwd) ;;
esac
ws=${1:-$guess}
[ -d "$ws/oracles" ] || { echo "no oracles/ in $ws" >&2; exit 1; }
out=$ws/oracles/freesurfer-8.2.0-probes/fs-template-decided
mkdir -p "$out"
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -c "$ws/tags/core-math/src/binary64/hypot/hypot.c" \
  -o "$out/crhypot.o"
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -c "$ws/tags/core-math/src/binary64/acos/acos.c" \
  -o "$out/cracos.o"
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -D__wrap_cabs=cabs -D__wrap_csqrt=csqrt \
  -c "$here/../fs-robust/complexrt.c" -o "$out/complexrt.o"
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -c "$here/acosrt.c" -o "$out/acosrt.o"
# the decided clog (relay 51: fs-robust's clogrt.c on CORE-MATH's atan2, log1p and log; the
# affine mean space's matrix logarithm)
for f in atan2 log1p log; do
  /usr/bin/gcc -O2 -ffp-contract=off -fPIC -c "$ws/tags/core-math/src/binary64/$f/$f.c" -o "$out/cr$f.o"
done
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -D__wrap_clog=clog -c "$here/../fs-robust/clogrt.c" \
  -o "$out/clogrt.o"
/usr/bin/gcc -shared -o "$out/libdecidedtemplate.so" "$out/complexrt.o" "$out/crhypot.o" \
  "$out/acosrt.o" "$out/cracos.o" "$out/clogrt.o" "$out/cratan2.o" "$out/crlog1p.o" \
  "$out/crlog.o" -lm -ldl
nm -D --defined-only "$out/libdecidedtemplate.so" | grep -E ' (acos|cabs|csqrt|clog|cr_hypot|cr_acos)$'
sha256sum "$out/libdecidedtemplate.so" "$here/acosrt.c" "$here/../fs-robust/complexrt.c" \
  "$here/../fs-robust/clogrt.c" "$ws/tags/core-math/src/binary64/hypot/hypot.c" \
  "$ws/tags/core-math/src/binary64/acos/acos.c" "$ws/tags/core-math/src/binary64/atan2/atan2.c" \
  "$ws/tags/core-math/src/binary64/log1p/log1p.c" "$ws/tags/core-math/src/binary64/log/log.c" \
  > "$out/BUILD_INFO.txt"
/usr/bin/gcc --version | head -1 >> "$out/BUILD_INFO.txt"
echo "built $out/libdecidedtemplate.so ($(sha256sum "$out/libdecidedtemplate.so" | cut -c1-16))"
