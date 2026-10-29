#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Release build of the GPL workspace (since 2026-10-29 only its verification tools: the AFNI replicas are
# in the main package). Usage: build-gpl.sh <checkout>
set -o pipefail
source "$(dirname "$0")/env.sh"
cd "$1/crates-gpl" || exit 1
cargo build --release --locked 2>&1 | tail -2
