#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Fetch CC BY 4.0's legal code (LICENSES/CC-BY-4.0.txt) into $1.
set -euo pipefail
dir=$1
mkdir -p "$dir"
curl -sfL -o "$dir/legalcode.txt" https://creativecommons.org/licenses/by/4.0/legalcode.txt
curl -sfL -o "$dir/spdx-CC-BY-4.0.txt" https://raw.githubusercontent.com/spdx/license-list-data/main/text/CC-BY-4.0.txt || echo "spdx fetch failed"
sha256sum "$dir"/*
wc -l "$dir"/*
