#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# every finding of a trial replayed through the (fixed) descriptor: its verdict line and reasons
T=$1
for f in "$T"/findings/tedana-t2smap-original/*.json; do
  n=$(basename "$f" | sed 's/.*trial-0*\([0-9]*\)-.*/\1/')
  echo "########## $n"
  timeout 600 python -m larmorx_validation difftest --replay "$f" 2>&1 | tail -8
done
echo "RETRIAGE-END"
