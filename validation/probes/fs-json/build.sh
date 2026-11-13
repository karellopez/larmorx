#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the two originals and the Rust driver (README.md): libup.so (shim.cpp against
# nlohmann/json 3.2.0's single_include/nlohmann/json.hpp from the pinned tree) and libfs.so
# (shim.cpp against FreeSurfer 8.2.0's include/json.h), both with the unit oracle's GCC 8.4.0
# (g++) and FreeSurfer's C++ flags (-O3 -DNDEBUG -fopenmp -fno-builtin -fPIC, its
# compile_commands.json).
# Usage: build.sh <out-dir> [<unit-oracle>] [<json-3.2.0>] [<freesurfer-8.2.0>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=$(cd "$here/../../../.." && pwd)
case "$ws" in */.claude/worktrees*) ws=${ws%%/larmorx/.claude/worktrees*} ;; esac
out=${1:?usage: build.sh <out-dir> [<unit-oracle>] [<json-3.2.0>] [<freesurfer-8.2.0>]}
unit=$(cd "${2:-$ws/oracles/freesurfer-8.2.0-unit}" && pwd)
up=$(cd "${3:-$ws/tags/nlohmann-json-3.2.0}" && pwd)
fs=$(cd "${4:-$ws/tags/freesurfer-8.2.0}" && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)
CXX="$unit/toolchain/bin/g++"
FLAGS=(-O3 -DNDEBUG -fopenmp -fno-builtin -fPIC -std=c++11 -w)
"$CXX" "${FLAGS[@]}" -DJSON_HEADER="\"$up/single_include/nlohmann/json.hpp\"" -shared \
  -Wl,-Bsymbolic -o "$out/libup.so" "$here/shim.cpp"
"$CXX" "${FLAGS[@]}" -DJSON_HEADER="\"$fs/include/json.h\"" -shared -Wl,-Bsymbolic \
  -o "$out/libfs.so" "$here/shim.cpp"
{
  "$CXX" --version | head -1
  echo "flags: ${FLAGS[*]}"
  echo "nlohmann/json: $(cat "$up/.upstream-sha256")"
  ( cd "$fs" && sha256sum include/json.h )
} > "$out/BUILD_INFO.txt"
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/fs-json" "$out/"
echo "built $out/fs-json, $out/libup.so and $out/libfs.so"
