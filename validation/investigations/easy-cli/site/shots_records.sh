#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The easy command line's relay 3 (2026-11-11): screenshots of the two tool pages whose evidence was
# exported again from their re-made records (mri-hmc: syn/motion/small/no-fsloutputtype now rejected
# by both, as mcflirt stops; fsl-prelude: orig-types/unset's D13 check passing), before and after,
# 1440 and 390 px wide, light and dark: each page's agreement chart of the clean-room original and
# its case explorer, photographed with the site's own tools/review/shoot.mjs (Node 22, Chrome).
# Usage: shots_records.sh <site checkout> <work folder>
#   <work folder>/serve-before/larmorx_documentation and serve-after/larmorx_documentation: the
#   two builds' _site folders (symbolic links); the shots go to <work folder>/shots/.
set -u
SITE=$1; W=$2; SHOOT=$SITE/tools/review/shoot.mjs; OUT=$W/shots; mkdir -p "$OUT"
cd "$W" || exit 1
python3 -m http.server 8783 -d serve-before >/dev/null 2>&1 & B=$!
python3 -m http.server 8784 -d serve-after >/dev/null 2>&1 & A=$!
sleep 1
shot() { env -u TMPDIR timeout 300 node "$SHOOT" --sizes 1440x900,390x844 --themes light,dark --out "$OUT" "$@" | grep -c png; }
before=http://localhost:8783/larmorx_documentation
after=http://localhost:8784/larmorx_documentation
for page in mri-hmc fsl-prelude; do
  for side in before after; do
    url=$before; [ "$side" = after ] && url=$after
    echo "$page $side agreement: $(shot --url $url/tools/$page/ --name $side-$page-agree --select 'figure.ev-agree[data-impl="original"]')"
    echo "$page $side cases: $(shot --url $url/tools/$page/ --name $side-$page-cases --select '#cases')"
  done
done
kill $B $A
