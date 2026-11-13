#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Release build of the FreeSurfer replica workspace (the larmorx-freesurfer program). Usage: build-freesurfer.sh <checkout>
set -o pipefail
source "$(dirname "$0")/env.sh"
cd "$1/crates-freesurfer" || exit 1
cargo build --release --locked 2>&1 | tail -2
