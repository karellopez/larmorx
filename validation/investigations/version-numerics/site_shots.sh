#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Before/after screenshots and the number check of the pages the version update's part 3 changed
# (the resampler, BET, the head-motion correction) and the Validation page, from two built site
# checkouts (the site's no-loss rule). Each site is served from its _site/ on a local port;
# tools/review/shoot.mjs photographs each page at desktop and phone width in both themes (top,
# and the first example) and runs LX_EVIDENCE.check() and LX_EVIDENCE.checkViewers() on it.
# The model: validation/investigations/version-afni/site_shots.sh.
# Usage: site_shots.sh <site before> <site after> <out dir>
set -uo pipefail
before=${1:?usage: site_shots.sh <before> <after> <out>}
after=${2:?}
out=${3:?}
mkdir -p "$out"
python3 -m http.server 8776 -d "$before/_site" >/dev/null 2>&1 &
pb=$!
python3 -m http.server 8775 -d "$after/_site" >/dev/null 2>&1 &
pa=$!
sleep 2
check='Promise.all([LX_EVIDENCE.check(), LX_EVIDENCE.checkViewers()]).then(r => JSON.stringify({numbers: {checked: r[0].checked, agree: r[0].agree, disagree: r[0].disagree}, viewers: r[1]}))'
for page in resample-series fsl-bet mri-hmc; do
  for side in before after; do
    port=8775
    [ "$side" = before ] && port=8776
    echo "== $page $side"
    node "$after/tools/review/shoot.mjs" --url "http://localhost:$port/tools/$page/" \
      --out "$out/$side" --name "$page" --sizes 1440x900,390x844 --themes light,dark \
      --at y:0 --at y:1800 --wait 4000 --eval "$check" 2>&1 | tail -4
  done
done
for side in before after; do
  port=8775
  [ "$side" = before ] && port=8776
  echo "== validation $side"
  node "$after/tools/review/shoot.mjs" --url "http://localhost:$port/validation/" --out "$out/$side" \
    --name validation --sizes 1440x900 --themes light --at y:0 --wait 3000 --eval "$check" 2>&1 | tail -3
done
kill "$pb" "$pa"
echo "SHOTS-DONE"
