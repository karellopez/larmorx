#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the hardware side of the x87 probe (x87_ops.c and patterns.cpp, GCC 8.4.0 with FreeSurfer
# 8.2.0's flags, as one shared library) and the Rust driver.
# Usage: build.sh <out-dir> [<gcc-8.4-prefix>]   (default: <workspace>/oracles/toolchains/gcc-8.4)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:?usage: build.sh <out-dir> [<gcc-8.4-prefix>]}
gcc84=${2:-$here/../../../../oracles/toolchains/gcc-8.4}
mkdir -p "$out"
out=$(cd "$out" && pwd)
CC="$gcc84/bin/x86_64-conda-linux-gnu-gcc"
CXX="$gcc84/bin/x86_64-conda-linux-gnu-g++"
# Ubuntu 20.04's gcc-8 (the official build's) turns these on by default; conda-forge's does not.
UBUNTU=(-fcf-protection=full -fstack-protector-strong -fstack-clash-protection -D_FORTIFY_SOURCE=2)
# FreeSurfer's C flags are -O3 -fopenmp; -fno-builtin added so that sqrtl and llrintl stay calls
# into the C library (the probe checks glibc's functions as well as the instructions).
"$CC" -O3 -fopenmp -fno-builtin -fPIC "${UBUNTU[@]}" -c "$here/x87_ops.c" -o "$out/x87_ops.o"
# FreeSurfer's C++ flags (docs/freesurfer/code.md section 1).
"$CXX" -std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin -fPIC "${UBUNTU[@]}" \
  -c "$here/patterns.cpp" -o "$out/patterns.o"
# Linked without libgomp and libstdc++ (neither is called).
"$CC" -shared -o "$out/libx87probe.so" "$out/x87_ops.o" "$out/patterns.o" -lm
"$gcc84/bin/x86_64-conda-linux-gnu-objdump" -d "$out/libx87probe.so" > "$out/libx87probe.dis"
{ "$CC" --version | head -1; echo "C: -O3 -fopenmp -fno-builtin -fPIC ${UBUNTU[*]}";
  echo "C++: -std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin -fPIC ${UBUNTU[*]}"; } > "$out/compiler.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/x87-probe" "$out/"
echo "built $out/x87-probe and $out/libx87probe.so"
