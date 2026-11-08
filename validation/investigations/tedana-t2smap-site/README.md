# The tedana-t2smap page's site checks (2026-11-08)

The checks of the documentation site's page `/tools/tedana-t2smap/` (the evidence of tedana's
`t2smap` replica, exported by `python -m larmorx_validation evidence tedana-t2smap --site <site>`
from `validation/src/larmorx_validation/evidence/tedana_t2smap.py`), on a site checkout served by
`tools/build.py --serve --port <port>`:

| Script | What it shows |
|---|---|
| `site_checks.sh <site> <port> <out> numbers` | the number check: every number the page shows against its JSON (`LX_EVIDENCE.check()`) |
| `site_checks.sh <site> <port> <out> viewers` | the site's viewer check: the decoded values at each comparison's largest difference |
| `site_checks.sh <site> <port> <out> drawn` | every viewer loaded and drawn (no status message left, no canvas at the browser's default 300 x 150) |
| `site_checks.sh <site> <port> <out> shots <name>=<selector>...` | element screenshots at 1440 and 390, light and dark |

A copy of `../sdcflows-fieldmap/site_checks.sh` with this page's address. The before/after
inventory of every page: `../cleanroom-bet/site_inventory.py` (`count`, `compare`). Leave TMPDIR
short (`env -u TMPDIR`): `shoot.mjs` puts Chrome's profile there, and a long path makes Chrome's
socket path too long.

Results of 2026-11-08 (the page from larmorx's evidence module at the part's relay 9): number
check 616 of 616 agree; viewers' check 66, none failing; all three viewers drawn; the 23 other
evidence pages' inventories unchanged and their HTML byte-identical.
