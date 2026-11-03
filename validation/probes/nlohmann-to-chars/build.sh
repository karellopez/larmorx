#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Builds the probe against FSL 6.0.7.23's nlohmann/json headers (no FSL library is linked).
# Usage: build.sh <output path>
set -eu
FSL=${FSLDIR_623:-$HOME/fsl-6.0.7.23}
HERE=$(cd "$(dirname "$0")" && pwd)
g++ -O2 -std=c++17 -I "$FSL/include" "$HERE/probe.cc" -o "$1"
