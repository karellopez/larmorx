#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# One worked example of a documentation-site page, photographed after its viewer has drawn
# (shoot.mjs's --select alone photographs the viewers before they load their slices: blank).
#   shoot_example.sh <site checkout> <base url> <out dir> <page> <example id> [WxH] [light|dark]
# e.g. shoot_example.sh ../larmorx_documentation http://localhost:8772/larmorx_documentation/ \
#        shots tools/fsl-bet bet-S-adult 390x844 dark
set -euo pipefail
site=$1 base=${2%/} out=$3 page=$4 ex=$5 size=${6:-1440x900} theme=${7:-light}
cd "$site"
node tools/review/shoot.mjs --url "$base/$page/" --out "$out" --name "$ex" --sizes "$size" \
  --themes "$theme" --select "#example-$ex" \
  --eval "(async () => { const e = document.querySelector('#example-$ex'); e.scrollIntoView(); await new Promise(r => setTimeout(r, 5000)); return e.querySelectorAll('canvas').length; })()"
