#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Fetch Arm's Optimized Routines v19.11 (MIT) single-precision sine and cosine into <dir>, the
# permissive source of glibc 2.35's sinf and cosf (README.md). Usage: fetch_aor.sh <dir>
set -euo pipefail
dir=${1:?usage: fetch_aor.sh <dir>}
mkdir -p "$dir"
base=https://raw.githubusercontent.com/ARM-software/optimized-routines/v19.11
for f in math/sincosf.h math/sincosf.c math/sinf.c math/cosf.c math/sincosf_data.c \
         math/math_config.h LICENSE; do
  curl -sfL -o "$dir/$(basename "$f")" "$base/$f"
done
ls -l "$dir"
