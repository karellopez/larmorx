#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Builds the probe against FSL 6.0.7.23's own headers (Armadillo 14.2.1) and libfsl-cprob.
# Usage: build.sh <output path>
set -eu
FSL=${FSLDIR_623:-$HOME/fsl-6.0.7.23}
HERE=$(cd "$(dirname "$0")" && pwd)
g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER -I "$FSL/include" "$HERE/probe.cc" \
  -L "$FSL/lib" -lfsl-cprob -Wl,-rpath,"$FSL/lib" -o "$1"
