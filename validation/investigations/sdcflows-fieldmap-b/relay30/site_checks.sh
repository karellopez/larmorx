#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The site checks of the sdcflows-estimation page (field maps part B, relay 30, 2026-11-10; part A's
# ../../sdcflows-fieldmap/site_checks.sh with this page's address), on a site checkout
# served by `tools/build.py --serve --port <port>`:
#   site_checks.sh <site> <port> <out> numbers    the number check: every number the page shows against its JSON
#   site_checks.sh <site> <port> <out> viewers    the site's viewer check (decoded values at each largest difference)
#   site_checks.sh <site> <port> <out> drawn      every viewer loaded and drawn: no status message left and no canvas
#                                           at the browser's default 300 x 150 (a viewer whose drawing throws,
#                                           e.g. a default view naming a comparison without images, passes the
#                                           viewer check but draws nothing)
#   site_checks.sh <site> <port> <out> shots <name>=<css selector>...
#                                           element screenshots at 1440 and 390, light and dark, each element
#                                           scrolled into view and given 5 s to draw first
# <out> receives the photographs. shoot.mjs makes Chrome's profile in the system's temporary folder:
# leave TMPDIR short (a long one makes Chrome's socket path too long, and Chrome does not start).
# The before/after inventory of every page: ../../cleanroom-bet/site_inventory.py (count, compare).
set -u
SITE=$1; PORT=$2; OUT=$3; WHAT=$4; shift 4
URL=http://localhost:$PORT/larmorx_documentation/tools/sdcflows-estimation/
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
cd "$SITE" || exit 1
shoot() { timeout 300 node tools/review/shoot.mjs --url "$URL" --sizes 1440x900 --themes light "$@"; }
case $WHAT in
  numbers)
    shoot --out "$OUT" --name numbers \
      --eval "LX_EVIDENCE.check().then(r => [r.checked, r.agree, r.disagree])" ;;
  viewers)
    shoot --out "$OUT" --name viewers \
      --eval "(async () => { for (const e of document.querySelectorAll('.ev-example')) { e.scrollIntoView(); await new Promise(r => setTimeout(r, 2500)); } const r = await LX_EVIDENCE.checkViewers(); return [r.length, r.filter(x => !x.ok)]; })()" ;;
  drawn)
    shoot --out "$OUT" --name drawn \
      --eval "(async () => { const out = []; for (const e of document.querySelectorAll('.ev-example')) { e.scrollIntoView(); await new Promise(r => setTimeout(r, 8000)); const st = e.querySelector('.ev-status'); const cs = [...e.querySelectorAll('.ev-live canvas')]; out.push([e.id, st && !st.hidden ? st.textContent.slice(0, 120) : 'drawn', cs.length, cs.filter(c => c.width === 300 && c.height === 150).length]); } return out; })()" ;;
  shots)
    for spec in "$@"; do
      name=${spec%%=*}; sel=${spec#*=}
      timeout 300 node tools/review/shoot.mjs --url "$URL" --out "$OUT" --name "$name" \
        --sizes 1440x900,390x844 --themes light,dark --select "$sel" \
        --eval "(async () => { const e = document.querySelector('$sel'); e.scrollIntoView(); await new Promise(r => setTimeout(r, 5000)); return e.getBoundingClientRect().height; })()"
    done ;;
  *) echo "unknown check: $WHAT" >&2; exit 2 ;;
esac
