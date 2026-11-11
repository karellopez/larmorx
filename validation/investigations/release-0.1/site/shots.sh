#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 5b's screenshots of the site for the first release (2026-11-11): the site before (main
# d9f67d1) and after (release-pages with HANDOFF-site's changes applied), each built by the
# site's tools/build.py and served as static files; 1440 and 390 px wide, light and dark, taken
# with the site's own tools/review/shoot.mjs (Node and Playwright, as the site's CLAUDE.md says).
# Usage: shots.sh <site checkout> <work folder>
#   <work folder>/serve-before/larmorx_documentation and serve-after/larmorx_documentation: the
#   two builds' _site folders (copies or symbolic links: the pages' links start with the site's
#   base path); the shots go to <work folder>/shots/.
set -u
SITE=$1; W=$2; SHOOT=$SITE/tools/review/shoot.mjs; OUT=$W/shots; mkdir -p "$OUT"
cd "$W" || exit 1
python3 -m http.server 8771 -d serve-before >/dev/null 2>&1 & B=$!
python3 -m http.server 8772 -d serve-after >/dev/null 2>&1 & A=$!
sleep 1
shot() { env -u TMPDIR timeout 300 node "$SHOOT" --sizes 1440x900,390x844 --themes light,dark --out "$OUT" "$@" | grep -c png; }
for page in installation why-rust-and-python; do
  echo "after $page full: $(shot --url http://localhost:8772/larmorx_documentation/$page/ --name after-$page --full 1)"
done
i=0
for sel in '.hero-meta' 'section[aria-labelledby="what-it-is"]' '.features' '.fair' '.status-note' 'header'; do
  i=$((i+1))
  echo "home $sel before: $(shot --url http://localhost:8771/larmorx_documentation/ --name before-home$i --select "$sel")"
  echo "home $sel after: $(shot --url http://localhost:8772/larmorx_documentation/ --name after-home$i --select "$sel")"
done
echo "after roadmap release: $(shot --url http://localhost:8772/larmorx_documentation/roadmap/ --name after-roadmap --select '#release')"
kill $B $A
echo SHOTS-DONE
