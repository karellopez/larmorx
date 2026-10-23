#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Release build of the GPL workspace (the larmorx-gpl replica program). Usage: build-gpl.sh <checkout>
set -o pipefail
source "$(dirname "$0")/env.sh"
cd "$1/crates-gpl" || exit 1
cargo build --release --locked 2>&1 | tail -2
