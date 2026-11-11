#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 5b's screenshots, second round (2026-11-11): the installation page after its fixes (the
# whole page) and the roadmap's release paragraph, 1440 and 390 px wide, light and dark.
# Usage: shots2.sh <site checkout> <work folder>   (the folders as for shots.sh; only serve-after)
set -u
SITE=$1; W=$2; SHOOT=$SITE/tools/review/shoot.mjs; OUT=$W/shots; mkdir -p "$OUT"
cd "$W" || exit 1
python3 -m http.server 8772 -d serve-after >/dev/null 2>&1 & A=$!
sleep 1
env -u TMPDIR timeout 300 node "$SHOOT" --sizes 1440x900,390x844 --themes light,dark --out "$OUT" \
  --url http://localhost:8772/larmorx_documentation/installation/ --name after2-installation --full 1 | grep -c png
env -u TMPDIR timeout 300 node "$SHOOT" --sizes 1440x900,390x844 --themes light,dark --out "$OUT" \
  --url http://localhost:8772/larmorx_documentation/roadmap/ --name after-roadmap-release --select 'h2#release + p' | grep -c png
kill $A
echo SHOTS2-DONE
