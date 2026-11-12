#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds ITK 4.13.2's own vnl_random.cxx with the shim (GCC 8.4.0, FreeSurfer 8.2.0's C++ flags) as
# a shared library, and the Rust driver.
# Usage: build.sh <out-dir> <itk-4.13.2-source> [<gcc-8.4-prefix>]
#   <itk-4.13.2-source>: the unpacked InsightToolkit-4.13.2.tar.gz (SHA-256 d8760b27...,
#   docs/freesurfer/third-party.md section 10); default compiler <workspace>/oracles/toolchains/gcc-8.4
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:?usage: build.sh <out-dir> <itk-4.13.2-source> [<gcc-8.4-prefix>]}
itk=$(cd "${2:?usage: build.sh <out-dir> <itk-4.13.2-source> [<gcc-8.4-prefix>]}" && pwd)
gcc84=${3:-$here/../../../../oracles/toolchains/gcc-8.4}
mkdir -p "$out"
out=$(cd "$out" && pwd)
CXX="$gcc84/bin/x86_64-conda-linux-gnu-g++"
vnl="$itk/Modules/ThirdParty/VNL/src/vxl/core"
sha256sum "$vnl/vnl/vnl_random.cxx" "$vnl/vnl/vnl_random.h" > "$out/sources.sha256"
UBUNTU=(-fcf-protection=full -fstack-protector-strong -fstack-clash-protection -D_FORTIFY_SOURCE=2)
FLAGS=(-std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin -fPIC "${UBUNTU[@]}" -I"$here/stub" -I"$vnl")
"$CXX" "${FLAGS[@]}" -c "$vnl/vnl/vnl_random.cxx" -o "$out/vnl_random.o"
"$CXX" "${FLAGS[@]}" -c "$here/shim.cxx" -o "$out/shim.o"
"$CXX" -shared -o "$out/libvnlrandom.so" "$out/vnl_random.o" "$out/shim.o" -lm
"$gcc84/bin/x86_64-conda-linux-gnu-objdump" -d --no-show-raw-insn "$out/vnl_random.o" > "$out/vnl_random.dis"
{ "$CXX" --version | head -1; echo "flags: ${FLAGS[*]}"; } > "$out/compiler.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-random" "$out/"
echo "built $out/fs-random and $out/libvnlrandom.so"
