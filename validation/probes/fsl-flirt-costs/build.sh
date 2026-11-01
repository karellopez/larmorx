#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Builds cost_probe against FSL 6.0.7.23's own libraries. Usage: build.sh <output path>
set -eu
FSL=${FSLDIR_623:-$HOME/fsl-6.0.7.23}
HERE=$(cd "$(dirname "$0")" && pwd)
g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER -I "$FSL/include" "$HERE/cost_probe.cc" \
  -L "$FSL/lib" -lfsl-newimage -lfsl-miscmaths -lfsl-NewNifti -lfsl-znz -lfsl-cprob \
  -lfsl-utils -llapack -lblas -lz -Wl,-rpath,"$FSL/lib" -o "$1"
