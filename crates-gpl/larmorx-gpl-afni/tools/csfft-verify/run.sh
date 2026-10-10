#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Karel Lopez Vilaret.
#
# Builds the two C oracle harnesses against AFNI 25.2.09's compiled library, writes their
# reference data, and compares larmorx-gpl-afni's csfft with it, bit for bit.
#
# Usage: run.sh <AFNI build src dir> <work dir>
#   e.g. run.sh <workspace>/oracles/afni-25.2.09/build/src /tmp/csfft-verify
# The AFNI build directory is the one scripts/build_afni_oracle.sh leaves (libmri.a,
# libf2c.a and csfft.c). The work directory receives the binaries and about 330 MB of data.
set -euo pipefail

B=$(cd "$1" && pwd)
W=$2
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$W/data"

# AFNI's build flags (Makefile.linux_ubuntu_16_64); FSL must be off PATH (it ships its own ld).
CLEANPATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CFLAGS=(-O2 -m64 -fPIC -DREAD_WRITE_64 -DLINUX2 -D_GNU_SOURCE -DREPLACE_XT -DHAVE_ZLIB
        -I"$B" -I"$B/nifti/nifti2" -I"$B/nifti/niftilib" -I"$B/nifti/nifticdf"
        -I"$B/nifti/znzlib" -I"$B/niml" -I"$B/rickr" -I"$B/f2c")
LIBS=("$B/libmri.a" "$B/libf2c.a" -lz -lexpat -lm -ldl)

for h in harness_cox harness_inc; do
  env PATH="$CLEANPATH" /usr/bin/gcc "${CFLAGS[@]}" -o "$W/$h" "$HERE/$h.c" "${LIBS[@]}"
done

"$W/harness_cox" "$W/data/cox.bin" "$W/data/nextup.bin"
"$W/harness_inc" "$W/data/nextup_even_internal.bin" "$W/data/generic.bin" "$W/data/sincos.bin"

python3 "$HERE/check_unrolled.py" "$B/csfft.c"

cargo run --release --manifest-path "$HERE/Cargo.toml" -p csfft-verify -- "$W/data"
