#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the shim (shim.cxx) against ITK 4.13.2's VNL as the unit oracle built it (GCC 8.4.0, FreeSurfer
# 8.2.0's ITK flags), as a shared library, and the Rust driver.
# Usage: build.sh <out-dir> [<unit-oracle>] [<itk-4.13.2-tag>]
#   <unit-oracle>: <workspace>/oracles/freesurfer-8.2.0-unit (validation/investigations/freesurfer-unit-oracle/
#   build.py: its toolchain/, itk-4.13.2/ install and src/InsightToolkit-4.13.2/); <itk-4.13.2-tag>: the pinned
#   <workspace>/tags/ITK-v4.13.2, whose VNL tree must equal the one the oracle was built from (checked).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<itk-4.13.2-tag>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
tag=$(cd "${3:-$ws/tags/ITK-v4.13.2}" && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
vnl_rel=Modules/ThirdParty/VNL
# The oracle's ITK was built from the release tarball; its VNL must be the pinned tag's, file for file.
diff -rq "$tag/$vnl_rel" "$unit/src/InsightToolkit-4.13.2/$vnl_rel" > "$out/vnl-tree.diff" || {
  echo "the unit oracle's VNL differs from $tag (see $out/vnl-tree.diff)"; exit 1; }
CXX="$unit/toolchain/bin/g++"
inc="$unit/itk-4.13.2/include/ITK-4.13"
lib="$unit/itk-4.13.2/lib"
libs=(libitkvnl_algo-4.13.a libitkvnl-4.13.a libitkv3p_netlib-4.13.a libitkvcl-4.13.a)
( cd "$lib" && sha256sum "${libs[@]}" ) > "$out/libraries.sha256"
# FreeSurfer 8.2.0's C++ flags for utils/numerics.cpp (the unit oracle's compile_commands.json).
FLAGS=(-std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin -fPIC -fdata-sections -ffunction-sections -w -isystem "$inc")
"$CXX" "${FLAGS[@]}" -c "$here/shim.cxx" -o "$out/shim.o"
"$CXX" -shared -fopenmp -o "$out/libfsvnl.so" "$out/shim.o" "${libs[@]/#/$lib/}" -lm
{ "$CXX" --version | head -1; echo "flags: ${FLAGS[*]}"; } > "$out/compiler.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-vnl" "$out/"
echo "built $out/fs-vnl and $out/libfsvnl.so"
