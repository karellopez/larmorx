# Handoff: topup's replica (2026-11-03, relays 1 to 11)

The exact changes are in `HANDOFF.toml` beside this file (28 operations on 13 files; `python -P
scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <site> --check` passes in the
part's worktree at 3b21e31 and the site clone; main's shared documents are the same as the
worktree's, since main's own changes after 8220300 touch none of them). Then: `python -m
larmorx_validation tables --site <site>`, `register_counts.py`, `python -m larmorx_validation
evidence summary --site <site>`.

## The register (`docs/findings/upstream-defects.md`)

- **23 new summary rows**, FSL-116 to FSL-135 and FSL-141 to FSL-143, all † (found from FSL's
  source; the rows describe behaviour only), inserted after FSL-111. Their classes, for the recount
  (an entry with two classes counts under its first):
  - (a) 7: FSL-121, FSL-124, FSL-127, FSL-128, FSL-129, FSL-131, FSL-141
  - (b) 1: FSL-116
  - (c) 9: FSL-117, FSL-118, FSL-119, FSL-130, FSL-132, FSL-133, FSL-134 (c, a), FSL-142, FSL-143
  - (e) 5: FSL-122, FSL-123, FSL-125, FSL-126, FSL-135 (e, a)
  - (f) 1: FSL-120
  - none questionable (c?); 313 + 23 = 336 entries expected. Gap column "–" for every one: each
    is corrected or refused by default with its `reproduce` switch where FSL's behaviour is
    defined, FSL-116 is class (b) (never reproduced), FSL-127 to FSL-131 are library defects no
    tool reaches (FSL-130's member subtraction is kept as FSL's in `larmorx-nc-core`, to decide by
    rules A-D if a replica ever reaches it).
- **FSL-132 and FSL-59** (the orchestrator's question, answered by relay 9 from the source): the
  same cause (newimage 2601's per-volume cache); only mcflirt (FSL-59, already corrected in both
  implementations) meets it among the other FSL replicas; FLIRT, BET and FAST do not. The FSL-59
  row gains the cross-reference, its restricted section a "See also".
- The restricted intro and section heading gain topup (`fsl-topup.md`); a new restricted section
  "FSL-116 to FSL-135, FSL-141 to FSL-143" (causes, with routine names) is appended at the end;
  a dated note "2026-11-03, topup's replica" joins the FSL notes (the rule-C list of what is kept
  as FSL's).
- **FSL-116 on fMRIPrep's real calls** (new numbers, from the site page's evidence: the figures
  cache's FSL runs at `--nthr=1` and `--nthr=4`, both through the oracle cache): on ds005040 the
  field moves by up to 26.1 Hz over the field of view and 3.19 Hz inside sdcflows' brain mask
  (99th percentile 0.49 Hz), the corrected images by up to 6,255 (of about 10^5); on ds001600 by
  up to 5.83 Hz (0.050 Hz in the brain). fMRIPrep's own outputs equal FSL's one-thread result on
  both runs, so fMRIPrep's calls do not meet it. The register row quotes these; the page's
  "thread band" tables hold them with pointers (`evidence/fsl_topup.py`, `_band_table`).

## `docs/api/upstream-defects.md`

16 rows (FSL-116 to FSL-126 with FSL-123/125/126 in one row, FSL-132 to FSL-135, FSL-141 to
FSL-143) after FSL-111, and the closing paragraph says which rows are topup's and applytopup's
(and that FSL-127 to FSL-131 are library defects no tool reaches).

## `validation/tools.toml` and the generated tables

A `[[tool]]` block `fsl-topup` after `fsl-fast`'s, and a `[[record]]` block `fsl-topup-replica`
after `fsl-fast-replica`'s (`tables.py`'s METRICS already has `fsl-topup-replica: _bet`). Checked
in a scratch copy of the worktree with the handoff applied: `tables --site` writes the README's row
("replica: 104 of 104 ... fMRIPrep's call on 2 oracle runs ... 66 more rejected by both alike; 3
refused ..."), the validation index's row and the site's status block.

## Random testing (`docs/validation/difftest.md`)

The descriptor's row after `fsl-fast-original`'s, and the trial in the oracle-cache paragraph
(FAST's clean-room trial "the first", topup's "the second": 14,460 cases, every one bit-identical,
27 minutes, 12 jobs, 1.5 GiB per case, `LARMORX_DIFFTEST_CACHE_MODE=use`). `triage.toml`: nothing
(no difference).

## Golden (`docs/validation/golden.md`)

- `larmorx-nc`'s count 79 cases, 440 outputs -> 86 cases, 494 outputs (relay 10's recording).
- The main package's count 168 cases, 1026 outputs -> 208 cases, 1283 outputs: **a pre-existing
  staleness**, not this part's (main's `tests/golden/golden.json` already holds 208 cases since
  FAST's clean-room merge; this part added no main-package case: `lx.fsl.topup` and
  `lx.fsl.applytopup` are replica-only). Drop that operation if it is not wanted here.
- A paragraph "topup and applytopup, the replica" after FAST's clean-room paragraph: the 7 cases
  and the refusal, the relative input paths (the topup logs), the recordings (79dbdf0 as a0e6ccf,
  then 5c26745 as d9fed9c), every other checksum unchanged (the core's `dgemm` reordering included).

## Restricted lists, README, status, next steps

- `docs/porting-guide.md` §1 and CLAUDE.md rule 1 gain `fsl-topup.md`.
- README's outline item 2 (it was stale: FLIRT's and FAST's clean-room originals are done).
- `docs/status.md`: the FSL line, and topup's replica complete with "Next: topup's clean-room
  original beside ...".
- `docs/next-steps.md`: item 4's topup, item 7's "topup: the replica done" paragraph after FAST's
  clean-room one, and one follow-up (applytopup's lines < 8 or > 256 voxels and blocked band
  systems not replicated; FSL-116's size on fMRIPrep's real call as a candidate upstream report,
  the user decides).
- Not in the operations (the orchestrator's own text): CLAUDE.md's "Where things are" ("now
  topup's replica ...") and `ORCHESTRATOR-STATE.md`.

## The site (branch `fsl-topup-page` of the clone `<scratch>/site`)

- Commit `8d9e280` on `fsl-topup-page` (from main 4223a61): `content/tools/fsl-topup.md`,
  `static/evidence/fsl-topup.json`, `static/evidence/blobs.sha256`. Branch `evidence-blobs`:
  `72d1f2d` (851 blobs: the 780 before, none removed, and topup's 71). Push `evidence-blobs`
  (force) before `main`.
- Exported from larmorx 3b21e31 (clean; exporter tree hash 7ba5d847); every figures-cache entry
  computed from 3b21e31 (the first entries, computed while the module was uncommitted and stamped
  `-dirty`, are archived in `<workspace>/archive/2026-11-03-topup-figures-cache-dirty/`).
- The page: six worked examples. fMRIPrep's call on ds005040 (opens on the first corrected volume)
  and on ds001600 (opens on the field): the pair before the correction as two input panels, every
  output on the input's grid (each volume corrected, their mean, the field, the Jacobians, the
  displacements' y components; the x and z components are 0, stated), FSL at one thread (served
  from the oracle cache: the record's runs) and at four (run through the cache), the replica in
  reproduce mode at one and four threads and in its default mode; comparisons replica − FSL,
  replica − fMRIPrep's own, replica 4 − 1 threads, default − reproduce (all bit-identical) and the
  yardstick FSL 4 − 1 threads (no verdict); tables: every file against FSL's (with fMRIPrep's own)
  and FSL's thread band per output inside and outside the brain. Then `opt/membrane-lm` (FSL-117,
  d13), `at/lsr-y` and `at/jac-y` (FSL-132, d13) and `at/vb3d-y` (FSL-132 and FSL-143, d13: FSL's
  divergent output beside the corrected one), each with the field applytopup applies as an input.
  The trial's table (from the trial's `summary.json`, its SHA-256 in the provenance). The case
  explorer: 173 cases (104 bit-identical, 66 rejected by both, 3 expected divergences), each with
  the replica's default mode run on it (figures-cache entry `cases-default-mode`: its exit status
  and the entries it names).
- Shown inside sdcflows' brain mask of the corrected reference (dilated by 2 voxels; the half
  images by 1 at half resolution): no face.
- Shared site files (in the operations; verified in the clone's working tree for the preview, then
  restored; the preview's diff: `<scratch>/site-shared-preview.diff`): `site.yml` (topup's entry
  after FAST's), `content/roadmap.md` (step 1's status, a Completed row), `data/status.yml` (the
  planned FSL line, which was stale; the generated block from `tables --site`),
  `static/evidence/summary.json` (re-export; `evidence/summary.py`'s PAGES gained
  `/tools/fsl-topup/` in the branch).
- Checks: build 0 problems (135 links into the worktree checked); number check 270 of 270; viewer
  check 72 of 72; no example says its images could not be loaded; size guard: the page 6.80 MB
  (budget 40 MB), the site 298.0 MB. Inventory (`validation/investigations/difftest/site_inventory.py`,
  `<scratch>/inventory.md`): every page after >= before; topup's page new; the tools index, the
  home page and the roadmap gained a row. The tool marks the Validation page "NO" only because the
  summary's tools are addressed by index and topup is inserted after FAST (index 15), shifting
  the later tools' pointers: compared by page, every earlier tool's summary entry is the same,
  the manifest gained 71 lines and lost none, and the page's shown numbers grew 382 -> 386.
  Screenshots: `<scratch>/shots/after/` (topup's page at 1440 and 390, light and dark: the top,
  the verdicts, the first viewer, vb3D, the trial, the cases) and the tools index, roadmap and
  Validation page before (`<scratch>/shots/before/`, main 4223a61) and after (with the handoff),
  each at both widths and themes.

## Tell the user (through the orchestrator)

- topup's and applytopup's replicas are done: every file both programs write is FSL's, bit for
  bit, on fMRIPrep's two real calls and every compared case; 14,460 random cases agreed too.
- FSL's own topup gives different answers at different thread counts: on fMRIPrep's ds005040 call
  at four threads the field differs by up to 3.2 Hz inside the brain. fMRIPrep runs it at one
  thread, so its results are unaffected; LARMORX always gives the one-thread answer.
- The corrections by default that matter in practice: the membrane energy's units (FSL-117),
  applytopup's first volume interpolated trilinearly (FSL-132), and the `vb3D`/`vb4D` restorations
  that diverge in FSL (FSL-143), besides refusals of meaningless options.
