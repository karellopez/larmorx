#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# A checkout's Windows x86_64 files for the Windows helper's checks: the standalone larmorx.exe (a
# program wheel) and larmorx's abi3 wheel, cross-built with Rust's x86_64-pc-windows-gnu target
# and zig's MinGW-w64 linker, as validation/investigations/release-0.1/win_gnu_build.sh does but
# without its container (no manylinux image needed): a Rust 1.99.0 of its own (RUSTUP_HOME and
# CARGO_HOME in <work>, the crates' registry shared with ~/.cargo by a link), ziglang 0.14.1 from
# PyPI in a venv in <work>, the given maturin, the release profile. The files link the Universal C
# Runtime (part of Windows 10 and 11). Run it in a memory box (scripts/orchestration/boxed.sh 20).
# Usage: win_build.sh <checkout> <work> [<maturin>]   (JOBS: cargo's jobs, default 9)
# The files go to <work>/out/; the last line: WIN-BUILD exit <status>.
set -u -o pipefail
[ $# -ge 2 ] || { sed -n '3,11p' "$0"; exit 2; }
C=$(cd "$1" && pwd)
mkdir -p "$2"
S=$(cd "$2" && pwd)
M=${3:-maturin}
T=x86_64-pc-windows-gnu
export RUSTUP_HOME=$S/rustup CARGO_HOME=$S/cargo XDG_CACHE_HOME=$S/cache CARGO_TARGET_DIR=$S/target
export CARGO_BUILD_JOBS=${JOBS:-9} CARGO_TERM_COLOR=never
export PATH=$CARGO_HOME/bin:$(echo "$PATH" | tr ':' '\n' | grep -v -e /fsl -e '\.cargo/bin' | paste -sd:)
if ! [ -x "$CARGO_HOME/bin/cargo" ]; then
  curl -sSf --proto '=https' --tlsv1.2 https://sh.rustup.rs -o "$S/rustup-init.sh" &&
    sh "$S/rustup-init.sh" -y -q --no-modify-path --profile minimal --default-toolchain 1.99.0 &&
    ln -s "$HOME/.cargo/registry" "$CARGO_HOME/registry" || { echo "WIN-BUILD exit 1 (rustup)"; exit 1; }
fi
rustup target add --toolchain 1.99.0 $T 2>&1 | tail -1
[ -x "$S/zig/bin/python" ] || { python3.12 -m venv "$S/zig" && "$S/zig/bin/pip" install -q "ziglang==0.14.1"; } ||
  { echo "WIN-BUILD exit 1 (zig)"; exit 1; }
export PATH=$S/zig/bin:$PATH
echo "zig $("$S/zig/bin/python" -m ziglang version), $("$M" --version), $(rustc --version)"
mkdir -p "$S/out"
status=0
# The program wheel from its crate's folder (the root's pyproject.toml defines scripts, which a
# binary wheel refuses); its build also makes cargo-zigbuild's linker wrapper.
(cd "$C/crates/larmorx-cli" && "$M" build --release --locked --zig --target $T --bindings bin --out "$S/out" 2>&1 | tail -3) || status=1
# The extension without --zig (maturin's .def names PyInit_larmorx._core, which the link cannot
# find), with that wrapper as its linker and Python's import library made by pyo3.
L=$(find "$S/cache/cargo-zigbuild" -name "zigcc-$T-*.sh" | head -1)
if [ -n "$L" ]; then
  Z=$(dirname "$L")
  (cd "$C" && export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$L CC_x86_64_pc_windows_gnu=$L AR_x86_64_pc_windows_gnu=$Z/ar \
    ZIG_COMMAND="$S/zig/bin/python3 -m ziglang" PATH=$Z:$PATH
    "$M" build --release --locked --target $T --features pyo3/generate-import-lib --out "$S/out" 2>&1 | tail -3) || status=1
else
  echo "no cargo-zigbuild wrapper"; status=1
fi
ls -l "$S/out"
echo "WIN-BUILD exit $status"
