#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Fetch Arm's Optimized Routines v19.11 (MIT) sinf/cosf sources into $1.
set -euo pipefail
dir=$1
mkdir -p "$dir"
base=https://raw.githubusercontent.com/ARM-software/optimized-routines/v19.11
for f in math/sincosf.h math/sincosf.c math/sinf.c math/cosf.c math/sincosf_data.c \
         math/math_config.h math/math_errf.c LICENSE; do
  curl -sfL -o "$dir/$(basename "$f")" "$base/$f" || echo "fail $f"
done
sha256sum "$dir"/*
