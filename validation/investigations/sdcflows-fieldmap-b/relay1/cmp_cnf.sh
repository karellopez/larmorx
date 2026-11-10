#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Are sdcflows' topup configurations (data/flirtsch/*.cnf) FSL's own files?
F=${FSLDIR:?}/etc/flirtsch
D=${LARMORX_WORKSPACE:?}/tags/sdcflows-2.17.1/sdcflows/data/flirtsch
for f in "$D"/*.cnf; do
  b=$(basename "$f")
  p=$(find "$F" -name "$b" 2>/dev/null | head -1)
  if [ -z "$p" ]; then echo "$b: not in FSL 6.0.7.23"; continue; fi
  if cmp -s "$p" "$f"; then echo "$b: identical to FSL's $p"; else echo "$b: differs from FSL's $p"; diff "$p" "$f" | head -6; fi
done
head -3 "$D/b02b0_2.cnf"
