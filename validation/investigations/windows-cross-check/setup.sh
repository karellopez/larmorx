#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Prepares <dir> for cross_build.sh: Rust's MinGW libraries and conda-forge's llvm-dlltool (README).
set -eu
[ $# -eq 1 ] || { echo "usage: setup.sh <dir>" >&2; exit 2; }
DIR=$(mkdir -p "$1" && cd "$1" && pwd)
V=1.99.0
rustup target add x86_64-pc-windows-gnu --toolchain "$V"
T=rust-mingw-$V-x86_64-pc-windows-gnu
if [ ! -d "$DIR/$T" ]; then
  mkdir -p "$DIR/dl" && cd "$DIR/dl"
  curl -sSfO "https://static.rust-lang.org/dist/$T.tar.xz"
  curl -sSfO "https://static.rust-lang.org/dist/$T.tar.xz.sha256"
  sha256sum -c "$T.tar.xz.sha256"
  tar xf "$T.tar.xz" -C "$DIR"
fi
if [ ! -x "$DIR/llvm-tools/bin/llvm-dlltool" ]; then
  MAMBA=${MICROMAMBA:-$HOME/fsl/bin/micromamba}
  MAMBA_ROOT_PREFIX="$DIR/mamba-root" "$MAMBA" create -y -p "$DIR/llvm-tools" -c conda-forge \
    --override-channels llvm-tools
fi
echo "ready: $DIR"
