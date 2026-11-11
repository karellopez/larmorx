#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The easy command line's screenshots of the documentation site (2026-11-11): the site before
# (main) and after (branch easy-cli-pages), each built by the site's tools/build.py, served as
# static files and photographed with the site's own tools/review/shoot.mjs (Node 22, Chrome),
# 1440 and 390 px wide, light and dark: the installation page whole, the home page's command-line
# block with its new sentence, and the four pages whose shell blocks lost their comments (the
# block and, after, the sentence that now says what the comments said).
# Usage: shots.sh <site checkout> <work folder>
#   <work folder>/serve-before/larmorx_documentation and serve-after/larmorx_documentation: the
#   two builds' _site folders (symbolic links); the shots go to <work folder>/shots/.
set -u
SITE=$1; W=$2; SHOOT=$SITE/tools/review/shoot.mjs; OUT=$W/shots; mkdir -p "$OUT"
cd "$W" || exit 1
python3 -m http.server 8781 -d serve-before >/dev/null 2>&1 & B=$!
python3 -m http.server 8782 -d serve-after >/dev/null 2>&1 & A=$!
sleep 1
shot() { env -u TMPDIR timeout 300 node "$SHOOT" --sizes 1440x900,390x844 --themes light,dark --out "$OUT" "$@" | grep -c png; }
before=http://localhost:8781/larmorx_documentation
after=http://localhost:8782/larmorx_documentation
echo "installation before: $(shot --url $before/installation/ --name before-installation --full 1)"
echo "installation after: $(shot --url $after/installation/ --name after-installation --full 1)"
echo "home before: $(shot --url $before/ --name before-home-try --select 'section:has(.try)')"
echo "home after: $(shot --url $after/ --name after-home-try --select 'section:has(.try)')"
for page in tools/fsl-fast tools/fsl-flirt tools/mri-hmc validation; do
  n=$(echo "$page" | tr / -)
  echo "$page before: $(shot --url $before/$page/ --name before-$n --select 'div.code[data-lang="bash"]')"
  echo "$page after: $(shot --url $after/$page/ --name after-$n --select 'div.code[data-lang="bash"]')"
  echo "$page after, the sentence: $(shot --url $after/$page/ --name after-$n-text --select 'div.code[data-lang="bash"] + p')"
done
kill $B $A
echo SHOTS-DONE
