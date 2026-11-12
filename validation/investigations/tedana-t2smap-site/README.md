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
| `site_checks.sh <site> <port> <out> views` | every view of every viewer (each output in each plane, selected in turn) drawn: no error thrown, no status message left, no canvas at the browser's default size, every window a number (2026-11-12) |
| `site_checks.sh <site> <port> <out> shots <name>=<selector>...` | element screenshots at 1440 and 390, light and dark |
| `rerun_original_example.py "<why>"` | the clean-room original's worked example (its cache entry `fmriprep-ds003592-curvefit-original`): the original's and the replica's runs again from the current commit, tedana's kept, with the reason given recorded in its `RUN_INFO.json` (relay RS4, 2026-11-12; the evidence module, the clean-room original's, called as a library) |

A copy of `../sdcflows-fieldmap/site_checks.sh` with this page's address. The before/after
inventory of every page: `../cleanroom-bet/site_inventory.py` (`count`, `compare`). Leave TMPDIR
short (`env -u TMPDIR`): `shoot.mjs` puts Chrome's profile there, and a long path makes Chrome's
socket path too long.

Results of 2026-11-08 (the page from larmorx's evidence module at the part's relay 9): number
check 616 of 616 agree; viewers' check 66, none failing; all three viewers drawn; the 23 other
evidence pages' inventories unchanged and their HTML byte-identical.

Results of 2026-11-12 (the clean-room part's site relay RS3: the page with the clean-room
original's part, `tedana_t2smap_original.extend`, exported from larmorx 4e5c3ccd with
`--no-compute`, the figures cache's entries brought to the branch by
`python -m larmorx_validation.evidence.tedana_t2smap --refresh`): build 0 problems; number check
854 of 854 agree; viewers' check 86, none failing; all four viewers drawn, and every view (51:
each output in each plane) drawn without an error. The first export failed 16 numbers and 5
viewer checks, and selecting the original example's RMSE map threw: the map holds NaN where fewer
than two echoes are good, and the evidence's comparisons and windows were not NaN-aware (fixed in
`evidence/core.py` and `evidence/example.py`, the viewer's `evidence.js` leaves a NaN voxel
undrawn). The inventory (`../afni-volreg-original/site_inventory.py`, per page): the t2smap page
numbers 617 -> 856, examples 3 -> 4, panels 12 -> 16, differences 9 -> 11, charts 9 -> 15, tables
11 -> 18, table rows 245 -> 463, case rows 69 -> 227, images 18 -> 24, words 14,473 -> 26,876; the
31 other pages' HTML byte-identical; `../cleanroom-bet/site_inventory.py compare`: after >= before
(every panel, comparison, case and speed row kept). Of the JSON's 5,408 values before, 344 are
not found after, every one re-measured, not lost: 32 provenance; 223 of ds000258's example
(computed again: its oracle run was made again with OpenBLAS's Haswell kernels, its mask and
fMRIPrep's own files changed); the calls table's 12 and the files table's 24 (the same run;
the replica's record re-made from it); the memory table's 6 (measured again at one thread with
the branch's replica); the corrections table's 25 (its links follow the findings' heading that
now names TED-31 and TED-46, TED-01's text "not NaN", the cases naming each defect); 13 of 69
cases (the record re-made: the traceback chain's check added); the case counts (69 -> 102 and
227); 8 of ds003592's examples (the replica's commit; the comparison of tedana 25.0.1 with
fMRIPrep's own file now made with numpy's AVX-512 kernels on, as fMRIPrep's run had them, its
earlier form kept as `t25off-fmriprep` with a label that names the kernels); the speed note's
last sentence ("the clean-room original, still to be written").
