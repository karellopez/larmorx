#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# cross_build.sh <dir> <workspace folder> <cargo arguments...>: cargo for x86_64-pc-windows-gnu
# with rust-lld, Rust's MinGW libraries and llvm-dlltool from <dir> (setup.sh); the target folder
# is <dir>/target. Run from the repository's root.
set -eu
[ $# -ge 3 ] || { echo "usage: cross_build.sh <dir> <workspace folder> <cargo args...>" >&2; exit 2; }
DIR=$(cd "$1" && pwd); WS=$2; shift 2
V=1.99.0
LIBS="$DIR/rust-mingw-$V-x86_64-pc-windows-gnu/rust-mingw/lib/rustlib/x86_64-pc-windows-gnu/lib/self-contained"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$HOME/.rustup/toolchains/$V-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C linker-flavor=ld.lld -C link-self-contained=yes -L native=$LIBS -C dlltool=$DIR/llvm-tools/bin/llvm-dlltool"
export CARGO_TARGET_DIR="$DIR/target"
unset FSLDIR || true
cd "$WS"
cargo "$@" --target x86_64-pc-windows-gnu
