#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the preload that gives FreeSurfer's official programs the complex absolute value and
# square root the project decided for FreeSurfer's matrix square root (the user's D4: `hypot`
# correctly rounded in both modes; track C relay 41): validation/probes/fs-robust/complexrt.c's
# `cabs` and `csqrt` (exported under their own names) with CORE-MATH's correctly rounded `hypot`
# (tags/core-math), the system's gcc as validation/probes/fs-robust/build.sh builds them.
# `real.py --preload <lib>` runs the official mri_robust_register with it (LD_PRELOAD): its
# dynamic `cabs@GLIBC_2.2.5` and `csqrt@GLIBC_2.2.5` then take these, everything else unchanged.
# Output: <workspace>/oracles/freesurfer-8.2.0-probes/fs-complex-decided/libdecidedcomplex.so.
#
#   validation/probes/fs-robust-register/decided_complex.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=${1:-$(cd "$here/../../../.." && pwd)}
case "$ws" in */.claude/worktrees/*) ws=${ws%%/larmorx/.claude/worktrees/*} ;; esac
out=$ws/oracles/freesurfer-8.2.0-probes/fs-complex-decided
mkdir -p "$out"
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -c "$ws/tags/core-math/src/binary64/hypot/hypot.c" \
  -o "$out/crhypot.o"
/usr/bin/gcc -O2 -ffp-contract=off -fPIC -D__wrap_cabs=cabs -D__wrap_csqrt=csqrt \
  -c "$here/../fs-robust/complexrt.c" -o "$out/complexrt.o"
/usr/bin/gcc -shared -o "$out/libdecidedcomplex.so" "$out/complexrt.o" "$out/crhypot.o" -lm
nm -D --defined-only "$out/libdecidedcomplex.so" | grep -E ' (cabs|csqrt|cr_hypot)$'
echo "built $out/libdecidedcomplex.so ($(sha256sum "$out/libdecidedcomplex.so" | cut -c1-16))"
