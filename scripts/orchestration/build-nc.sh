#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Release build of the FSL replica workspace (the larmorx-nc program). Usage: build-nc.sh <checkout>
set -o pipefail
source "$(dirname "$0")/env.sh"
cd "$1/crates-nc" || exit 1
cargo build --release --locked 2>&1 | tail -2
