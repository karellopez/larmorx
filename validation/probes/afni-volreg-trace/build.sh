#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds volreg_probe against the AFNI oracle's library (scripts/build_afni_oracle.sh) with
# AFNI's own flags (gcc -O2 -m64 -fPIC, Makefile.linux_ubuntu_16_64).
# Usage: build.sh [<oracle dir, default <workspace>/oracles/afni-26.3.05>] [<output binary>]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../../.." && pwd)"
if [[ -d "$WS/larmorx" ]]; then :; else WS="$(cd "$HERE/../../../../../../.." && pwd)"; fi
ORACLE="${1:-$WS/oracles/afni-26.3.05}"
OUT="${2:-$HERE/volreg_probe}"
B="$ORACLE/build/src"
CLEANPATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/fsl' | paste -sd:)"
env PATH="$CLEANPATH" /usr/bin/gcc -O2 -m64 -fPIC -DREAD_WRITE_64 -DLINUX2 -D_GNU_SOURCE \
  -DREPLACE_XT -I"$B" -I"$B/nifti/nifti2" -I"$B/nifti/niftilib" -I"$B/nifti/nifticdf" \
  -I"$B/nifti/znzlib" -I"$B/rickr" -I"$B/f2c" -o "$OUT" "$HERE/volreg_probe.c" \
  -L"$B" -Wl,-rpath,"$B" -lmri -lf2c -lz -lexpat -lm -ldl
echo "$OUT"
