# Handoff: the field maps' part B, sdcflows' port continued (2026-11-08 to 10)

The part's relays, ready to merge. The shared-document changes are in `HANDOFF.toml` beside this file
(89 operations, 19 files: 16 of the main repository and 3 of the site); this file gives the reasons, how
to apply them, the trial merge's conflicts and what to verify. Relay 31 wrote it; relay 32 added the seven
upstream report drafts (in the branch, below), regenerated `HANDOFF.toml` on a fresh trial merge (the
register's upstream texts, next-steps' upstream and licence-header follow-ups: the only changes) and
corrected the conflicts' list (15 files, not 14: `evidence/summary.py`).

## What to merge

- **larmorx:** branch `worktree-agent-a33e6cc1a7edea6ca` (worktree
  `larmorx/.claude/worktrees/agent-a33e6cc1a7edea6ca`), from main `f5e89ff4`; **final head `5385a9f9`
  (relay 32), 139 commits** (`git rev-list --count f5e89ff4..5385a9f9`, the same as `main..5385a9f9`; main's
  `d1491604` cherry-picked into it as `8e4ae409`, `-x`; relay 31's "139 at `47c0a5c7`" was 138). The tree is clean at each relay's end. No clean-room crate,
  replica crate of another family or `crates-gpl/` file is touched; `crates-nc/` (FSL's six `topup`
  settings files, the replica's `--config` search, `fsldata.rs`, larmorx-nc's NOTICE, MANIFEST and README)
  and `specs/topup.md` (§2.5's tables, §12) were edited by relay 17 as the orchestrator named it, spec
  writer and replica author (`git diff --stat f5e89ff4 <head> -- crates/larmorx-afni crates/larmorx-mri
  crates/larmorx-fsl crates/larmorx-tedana crates-gpl crates/larmorx-afni-replica`: empty).
- **Site:** the clone `<scratch>/site`, branch `fmap-b-pages` at `c1904f8e` (from the site's main
  `4d86f086`, which is still the site's `origin/main`: `content/tools/sdcflows-estimation.md`,
  `static/evidence/sdcflows-estimation.json`, `static/evidence/blobs.sha256`); `evidence-blobs` at
  `d680cd2a` (1,424 blobs = 1,343 + 81 added, none removed or replaced; the manifest's SHA-256
  `3883642bce0e8a05`; parent: none, as `tools/blobs.py commit` makes it). Push `evidence-blobs` first,
  then `main`.
- **The trial merges** (scratch worktrees, uncommitted, with the handoff applied): **relay 32's
  `<scratch>/r32/merge`** (a detached worktree of the larmorx repository at main `d6d3e8e7`, `git merge
  --no-commit` of the branch at `5385a9f9`; the 15 conflicts resolved by `<scratch>/r32/py/resolve.py`,
  relay 31's with the two rules below, and `python/larmorx/__init__.py` by hand as relay 31 did) and
  `<scratch>/r32/site-merge` (a detached worktree of the site clone at `c1904f8e`;
  `static/evidence/blobs` a symbolic link to the clone's blobs); relay 31's `<scratch>/r31/merge` and
  `<scratch>/r31/site-merge` (its later branch files copied in, which hid the 15th conflict). Remove all
  four with `git worktree remove --force` when done (`git -C <larmorx> worktree remove ...`, `git -C
  <scratch>/site worktree remove ...`). Previews: `<scratch>/r31/shared-preview.diff` (the main repository's 17 files, the
  generated tables included) and `<scratch>/r31/site-shared-preview.diff` (the site's 4). The generator,
  which checks every `old` text and `after` line against the trial merge: `<scratch>/r32/py/make_handoff.py`
  (relay 31's, with relay 32's texts; with `handoff_docs.py`, `handoff_docs2.py`; `counts.py` gives the
  register's counts: `python -P make_handoff.py <merge> <site> <out>`, `counts.py <merge> <out> counts.txt`
  with absolute paths, then `make_handoff.py ... counts.txt` again). Relay 31's `HANDOFF.toml` and this
  file as it left them: `<scratch>/r32/HANDOFF.relay31.toml`, `HANDOFF.relay31.md`.

## How to apply (after the merge)

```
cd <merged larmorx checkout>
python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <merged site>
python -m larmorx_validation tables --site <merged site>
python -P scripts/orchestration/register_counts.py
python -m larmorx_validation evidence summary --site <merged site>
(site) python3 tools/build.py --main-repo <merged larmorx checkout>
```

Checked again by relay 32 on its trial merges (the same results as relay 31's but the summary): every
operation applies (`--check`, then for real); `tables --site` then `tables --check` exit 0;
`register_counts.py` 541 entries, the same classes; `evidence summary` writes 25 tools (24 + the page) and
**keeps t2smap's row**, which relay 31's trial merge dropped (its copied-in `summary.py` lacked main's t2smap
entry: the 15th conflict, below); the site builds with 0 problems (185 links into `<scratch>/r32/merge`);
against relay 31's before build (`<scratch>/r31/site-check/sm-before/`): 56 files identical, 31 changed, none
removed, and the Validation page only gains the new row (against relay 31's after build only
`validation/index.html` and `summary.json` differ: t2smap's row kept). Relay 31's checks:
every operation applies (`--check`, then for real); `tables --site` writes
README.md's tool row, the validation index's seven rows (six new records; `fsl-topup-replica`'s larmorx
text) and the site's status row, and `tables --check` then passes; `register_counts.py`: **541 entries**
((a) 119, (b) 17, (c) 270, (d) 27, (e) 79, (f) 27; questionable 14; no **decide**, no **yes**; one rule 8);
`evidence summary` adds the page (port, `validated`, 57 cases bit-identical) and the pilot entry (it needs
the branch's `47c0a5c7`, the summary's page table); the site with every change builds with **0 problems**
(31 pages; 185 links into the main repository checked against the trial merge). Built before and after the
handoff (`<scratch>/r31/site-check/sm-before/` and the site-merge's `_site`): 56 files identical, 31
changed, none removed; every page only gains (the navigation's new tools entry, 7 fragments, on every page;
the tools index +14; the home page: its status table one row, "25 lines" → 26, and the planned text without
the done part B (−2 +55); the roadmap: row 1 (Field maps, now done) removed, the rows renumbered, a completed
row (−25 +26)). The three Rust workspaces of the trial merge pass `cargo check --workspace --all-targets
--locked --offline` (root, crates-gpl, crates-nc; the root's `Cargo.lock` refreshed by `cargo metadata
--offline`: +11 lines, `larmorx-ndimage`; `<scratch>/r31/check_merge.sh`, log
`oracles/logs/fmapb-r31-check-merge.log`). Not run on the trial merge: the wheel build, the Python tests,
the clippy and test gates and the parity smoke tier (verify-merge's).

## The trial merge's conflicts (larmorx; the site merges without one)

Merging the branch into main `d6d3e8e7` stops at **15 files** (relay 32; relay 31 saw 14 because its trial
merge copied the branch's later files in). All but five are both sides adding at the same place: keep
both, main's first.

- **`crates/larmorx-transform/src/numpy_linalg.rs`** (the MERGE NOTE): keep main's `dbdsqr_upper_ncc` and
  give its wrapper `dbdsqr_upper` the parameter `clear_negative_zero: bool`, passed through
  (`dbdsqr_upper_ncc(n, d, e, (vt, ncvt, n), (u, nru, n), (&mut [], 0, 1), clear_negative_zero)`); the
  branch's `dbdsdc_upper` calls `dbdsqr_upper(n, d, e, vt, u, release == LapackRelease::V3_12_1)`; main's
  `gesdd.rs` and `lstsq.rs` call `_ncc` directly (unchanged).
- **`crates/larmorx-core/src/defects.rs`**: hunk 1, the branch's SCI-09 to SCI-31 rows after SCI-03, then
  main's NP-03 (since `2a87704a` the `SDC_01` line is the same on both sides, outside the hunk); hunk 2,
  main's TED-01 to TED-18 then the branch's SDC-12 to SKI-04.
- **`validation/src/larmorx_validation/evidence/summary.py`** (the 15th, from `47c0a5c7`): `PAGES` gets two
  tuples after `sdcflows-fieldmap`'s, main's `/tools/tedana-t2smap/` first, then the branch's
  `/tools/sdcflows-estimation/` (the hunk is both tuples' bodies: close main's with `),` and open the
  branch's with `(`). Taking either side alone drops a Validation-page row.
- **`crates/larmorx-cli/src/lib.rs`**: the registry's list gets both `("tedana", TEDANA_TOOLS)` and
  `("sdcflows", PYTHON_TOOLS)` before one `];`, then both constants' blocks (`TEDANA_TOOLS`; `PYTHON_TOOLS`
  and `EXIT_NOT_AVAILABLE`).
- **`python/larmorx/__init__.py`**: the branch's multi-line lists with `"tedana"` / `tedana` added in
  alphabetical order (the import, `_SUBMODULES`, `__all__`).
- `crates/larmorx-interp/src/ndimage/mod.rs`: the module comment names both (`affine_transform`, the
  branch's; `filters` and `morphology` for nilearn, main's).
- `tests/golden/golden.json`: only the `recorded` header (main `6608931e`, the branch `a637d51f`); every case
  merges (262 + 41 = 303 cases, 2,660 outputs). Take either header, then `scripts/record_golden.py` from
  the merge commit (verify-merge does): every checksum unchanged expected.
- `Cargo.lock`: take main's, then `cargo metadata --offline` adds `larmorx-ndimage` (+11 lines; `--locked`
  then passes; crates-gpl's and crates-nc's locks need no change).
- `Cargo.toml` (`larmorx-optim` and `larmorx-ndimage`), `crates/larmorx-core/src/numpy/mod.rs` (main's
  `format` module beside the branch's `dot` and `histograms`; the branch's `np_exp`/`np_dot` trait methods),
  `docs/validation/difftest.md` (main's tedana row, the branch's three), `tests/golden/registry.py`
  (`NAMESPACES`: `lx.tedana` and the branch's five), `validation/difftest/triage.toml` (main's t2smap block,
  then the branch's), `difftest/tools/__init__.py` and `evidence/__init__.py` (both tools): keep both.

Auto-merged, to look at in review: `validation/src/larmorx_validation/stages/catalogue.py` (main's d1491604
roadmap entry is in the branch as `8e4ae409`; the branch's `adf07a34` turns it into the stages S04 to S07 and
moves `epi_merge` into the `mri_robust_template` roadmap entry: git applies it cleanly over main),
`tests/parity/test_memory_peak.py` (both appended; ruff on the result), `_core.pyi`,
`validation/src/larmorx_validation/__main__.py`, `difftest/permanent.py`, `parity/python_versions.py`.
`tests/parity/test_pipeline_stages.py::test_every_program_node_is_classified` fails on the branch alone for
the three `--use-syn-sdc` runs and passes after the merge (relay 25).

**A duplicate the merge brings together** (no conflict; a follow-up in next-steps): SciPy's
`gaussian_filter1d`, `correlate1d` (reflect), binary erosion and dilation and `label` now exist in
`larmorx_interp::ndimage::{filters, morphology}` (main, for tedana's nilearn mask) and in
`crates/larmorx-ndimage` (the branch, every mode and type).

## What to verify at the merge

Records (identical expected): the part's six, **`ndimage-skimage`** (10), **`ndimage-skimage-full`** (1, the
2^32 float32 inputs), **`niworkflows-helpers`** (5), **`sdcflows-estimation`** (19), **`sdcflows-estimation-full`**
(14) and **`sdcflows-estimation-app-shared`** (8): 57 of 57 pass (`--python newest`, the step needing the shared
environment too; the full tier's cases boxed one at a time, as their README rows say); and the suites
whose code the branch changed (relay 25): `resample-series`, `resample-series-jacobian`
(`crates/larmorx-interp/src/ndimage`), `numpy-linalg` (`crates/larmorx-transform`: the signed zero of `svd3`,
SciPy's `inv`), `nifti-io` (`python/larmorx/io/nibabel_writer.py`), `ants-apply-transforms`
(`crates/larmorx-ants/src/cli/apply_transforms.rs`), `fsl-topup-replica` (`crates-nc/larmorx-nc-topup`: the
settings copy) and `sdcflows-fieldmap` (`python/larmorx/sdcflows/phase.py` and part A's modules). The memory
test `test_fieldmap_steps_need_no_more_memory_than_the_originals` (4 calls) needs the oracle runs' working
directories. The evidence: `python -m larmorx_validation evidence sdcflows-estimation --site <site>
--no-compute` serves every run from the figures cache (`oracles/figures-cache/sdcflows-estimation/`: 7
entries, each from a clean commit of the branch); relay 31's re-export from `c7808942` changed only the
corrections table's two cells (PIPE-39's class, SCI-20's test).

**CI on the other platforms:** `py.skimage.random_walker.nan` is the first golden case whose NaN outputs are
hashed by their bits through numpy's dot emulation (`larmorx_core::numpy::dot`); every NaN of its system is
x86-64's default NaN where the dot products are taken, so it should pass everywhere; if aarch64, macOS or
Windows differ there, look at the dot's NaN propagation, not the case.

**The clean-room audit:** this part wrote no clean-room original (it is a port of permissive upstreams); its
agents read sdcflows', niworkflows', nipype's, pybids', fMRIPrep's, nitransforms', nibabel's, nilearn's,
scikit-image's, SciPy's, numpy's, OpenBLAS's and LAPACK's sources, and ran FSL's topup, ANTs, AFNI's
3dvolreg and the replicas as programs. Relay 17 (`ae878eb5ad8d31a90`) also opened FSL 6.0.7.23's six `topup`
settings files and the topup replica's `options.rs` as the named spec writer and replica author, and edited
`specs/topup.md`, larmorx-nc's files, `lx.sdcflows`' `pepolar.py` (the permissive port) and
`docs/api/fsl-topup.md`, no clean-room file (`crates/larmorx-fsl/`, `python/larmorx/fsl/` untouched). For the
later clean-room original of sdcflows (the user, 2026-11-06) the audit's restriction table will need an
sdcflows family: next-steps' part B block lists the files. The part's agents (for the archive step), as
NOTES.md's agents line holds them: relay 1 `a4cf64edc320dd324` (and its stopped first attempt
`aaea7f61392fc0d27`), 2 `a33e6cc1a7edea6ca`, 3 `ac1175bc60550e332`, 4 `a1fd06ee6416ffe30`, 5
`a58deb7daf1547dd5`, 6 `ac5079c785f056c44`, 7 `a2d2e605fadc15ca8`, 8 `a15a4cf4128e2669f`, 9
`ae78b46850d0dea1a`, 10 `a5033e389fa9c9e19`, 11 `a1a6fcf4ee03fcfb5`, 12 `ab43113819ad79ad9`, 13
`a9b207f19eb5645ce`, 14 `a7ededd34215e0b82`, 15 `a1daa4575f3643c99`, 16 `a19e3c20cbb8f7312`, 17
`ae878eb5ad8d31a90`, 18 `a7ba42bb360567da2`, 19 `a902bb371637afc9b`, 20 `aa4f59554aad847a1`, 21
`af0974b3dbdc43289`, 22 `a694a281b3b4a0f88`, 23 `a8db1ba28a0b88624`, 24 `a3f4ba1c1e9c59f40`, 25
`aabe6c62889486dea`, 26 `acc44b330546a5099`, 27 `aff910a486c582678`, 28 `aa113697e2f6b45b7`, 29
`ac9c94451e2b8426e`, 30 `a0e0bb7a68ebd3989`, 31 `a56b27a2845fe52d5`, 32 `a5c1cdfa1e9ad900e` (32 relays, 33
agents).

## The operations and why

- **`validation/tools.toml`** (generated tables): a `[[tool]] sdcflows-estimation` after part A's (page
  `/tools/sdcflows-estimation/`; `use` the BIDS app, the three estimators' workflows, `brain_masker`,
  `run_app`, `lx.niworkflows`, `lx.bids`, `lx.ndimage`, `lx.skimage`; `how` the port's releases; the
  validation text cites the six records by placeholder, the register's numbers, the benchmark and the
  memory) and six `[[record]]` entries after `sdcflows-fieldmap`'s (relay 25's texts: the `-full` records
  hold only the cases the standard tier does not run); `fsl-topup-replica`'s larmorx text names FSL's six
  settings files in larmorx-nc. The status row this generates replaces relay 30's hand draft (the same
  facts, the counts from the records). The common metrics suffice: no `tables.py` entry.
- **`docs/findings/upstream-defects.md`:** the opening's families (pybids, scikit-image's SKI-, parts A and
  B); 89 summary rows (PIPE-18 to PIPE-40 after PIPE-17, SCI-06 to SCI-32 after SCI-05, SDC-12 to SDC-46 and
  SKI-01 to SKI-04 after SDC-11, before TED-01); the counts (541) and the gap paragraph (**switch** for SDC-23,
  SDC-42, PIPE-36 beside SDC-03; **measure** for PIPE-35); part B's D13 paragraph after the t2smap
  replica's (the IDs by rule); the sections: PIPE after PIPE-17's, SCI after SCI-05's, SDC after SDC-11's,
  a new family section "scikit-image (SKI-)" before "FSL: black-box findings"; the "fMRIPrep and its
  libraries" and SDC family introductions name part B. Every row and section is generated from the
  branch's draft (`validation/investigations/sdcflows-fieldmap-b/relay17/register_part_b.py`, the same
  source as the findings' table and the site's corrections table). Relay 31 changed two entries' text there:
  PIPE-39's class **n** → **b** (nondeterminism is the register's (b); `register_counts.py` would have
  counted it under no class) and SCI-20's test written `max(abs(a)) · max(abs(1/a))` (its `|` broke the
  table's columns); and SCI-21's evidence gained relay 24's measurement. No entry has a second class.
- **`docs/api/upstream-defects.md`:** the 61 rows of `larmorx_core::defects`' part B entries after their
  families' rows (PIPE-17, SCI-03, SDC-11) and a paragraph after tedana's (what has the switch, what is
  never reproduced, what waits).
- **`upstream.tsv`:** eleven tag rows (niworkflows 1.15.4, fMRIPrep 25.2.7, sdcflows 2.15.2, scikit-image
  0.26.0 and 0.25.2, nipype 1.12.0, pybids 0.22.0 and 0.20.0, toml 0.10.2, nilearn 0.14.1 and 0.11.1: PyPI's
  source URLs, the SHA-256 checked again by relay 31 against the files in `<workspace>/tags/*-download/`).
  TemplateFlow 25.1.2 gives only a list of names (`niworkflows/bids.py`): no row (relay 12's question; the
  orchestrator's call).
- **`UPSTREAM.md`:** rows for niworkflows, nipype, pybids, scikit-image, toml and transforms3d; the sdcflows,
  nilearn, fMRIPrep and SciPy rows say what part B follows of each.
- **`NOTICE`** and its two copies (`crates-nc/LICENSES/NOTICE-larmorx.txt`,
  `crates-gpl/LICENSES/NOTICE-larmorx.txt`; identical after the operations, as ORCHESTRATOR-STATE asks):
  the SciPy, numpy, OpenBLAS, LAPACK, sdcflows, nitransforms, nilearn and fMRIPrep entries name part B's
  paths; new entries with their licence texts: niworkflows 1.15.4 and nipype 1.12.0 (Apache-2.0: their
  copyright lines; neither ships a NOTICE file), pybids 0.22.0 and toml 0.10.2 (MIT), scikit-image 0.26.0
  (BSD-3-Clause, and `filters/thresholding.py`'s own BSD-2-Clause holders), the running median of SciPy's
  `_rank_filter_1d.cpp` (MIT: ashelly, Gideon Genadi Kogan), transforms3d 0.4.2 (BSD-2-Clause).
- **In the branch, found while writing the notices (relay 31, `c7808942`):** scikit-image's
  `filters/thresholding.py` is BSD-2-Clause with its own holders (its `LICENSE.txt`), so `otsu.rs` and
  `python/larmorx/skimage.py` now say `Apache-2.0 AND BSD-2-Clause AND BSD-3-Clause` with that notice;
  scikit-image's copyright line as its `LICENSE.txt` states it; `python/larmorx/niworkflows/header.py`
  translates transforms3d's `decompose44` and said only Apache-2.0: now `Apache-2.0 AND BSD-2-Clause`
  with its notice (`check_spdx.py` checks the family, not the terms, so it did not see either).
- **`docs/licensing.md`:** relay 17's bullet on FSL's `topup` settings files (after the standard images);
  "Upstream by upstream": the sdcflows, SciPy-numpy and nilearn rows extended; rows for the MIT pieces
  (pybids, toml, the running median), scikit-image and transforms3d.
- **`docs/validation/golden.md`:** 303 cases, 2,660 outputs; the recording of 2026-11-09 (relays 26 and 28;
  41 cases, 607 outputs; the NaN case; the folder independence); the rows: command line 79 → 80,
  `lx.transforms` 8 → 10, `lx.ndimage` 3 → 13, `lx.sdcflows` 14 → 27, and new rows `lx.niworkflows` (9),
  `lx.skimage` (2), `lx.bids` (1), larmorprepx's field-map stages (3); the enumerated namespaces.
- **`docs/next-steps.md`:** the order's item (7) **done**; part A's "*Part B* (next)" bullet closed; a new
  block of follow-ups, first under "Follow-ups, when they fit" (every follow-up of relays 2 to 30's handoff
  items that is still open, grouped: waiting for other ports, PIPE-35, duplicates, speed, random testing,
  the harness, licence headers (the orchestrator's 01:26 entry: `check_spdx.py` checks the family, not a
  compound expression's terms), oracle runs, upstream reports (relay 32: which are new issues, which are
  comments for open issues, PIPE-37 fixed on pybids' master, SCI-26 on SciPy's main), the clean-room original,
  the final phase). Closed by the part
  and so not listed: SciPy's 4 × 4 `inv` (relays 18, 19), `RegridToZooms`, the Gaussian derivatives and complex
  filters, the filters' argument handling, the long-double sigma, `random_walker(copy=False)`, the OpenBLAS
  pins, SCI-21's count, the random walker's non-converging speed (relay 28), the stage catalogue's SyN
  entry, SDC-45 and `record_golden.py`'s folder. Nothing for "Waiting for the user".
- **`docs/status.md`:** the "Field maps (ports)" row (part B, its records); the "Next" line.
- **`README.md`:** the roadmap's item 3 (part B done) and the versions line of "Supported tools"; the tool
  table is generated (`tables`).
- **`CLAUDE.md`:** "Where things are" (part B complete; next `t2smap`'s clean-room original, first the
  main package's own `topup` settings) and rule 2's pins (niworkflows, nipype, pybids, scikit-image, toml,
  transforms3d, nilearn, sdcflows 2.15.2's search, fMRIPrep 25.2.7's steps). Both `old` texts were checked
  against the trial merge's CLAUDE.md, which is main's; if the orchestrator edits CLAUDE.md first, re-run
  `--check`. The agents line (one agent at a time, the user 2026-11-09) is the orchestrator's own edit.
- **`docs/decision-log.md`:** a proposed entry "2026-11-10: The field maps' part B" with the orchestrator's
  decisions of the part (D1 to D12, the Haswell re-runs, the rules' outcomes, the parity suites' design, the
  SyN viewer and ants-syn.md's scope). Edit or drop it.
- **`docs/figures.md`:** the data cases part B's examples use (relay 29's question: the page is not in
  CLAUDE.md's list but every part reads it, so it goes through the handoff).
- **Site `site.yml`** (the tools entry after part A's), **`data/status.yml`** (the planned text without part
  B; the generated status row from `tables --site`), **`content/roadmap.md`** (last updated; row 1 removed
  and the rows renumbered; a completed row, newest first), **`static/evidence/summary.json`**
  (`evidence summary`).

## For the orchestrator (not in the operations)

- `docs/findings/ants-syn.md` (the ANTs part's findings file) was edited in the branch as you asked
  (`2a87704a`): its thread paragraph is scoped to its own cases (2, 4 and 8 threads: 0 to 3.6e-14 mm) and
  names relay 30's measurement on sdcflows' fieldmap-less call (four threads up to 0.835 mm from one
  thread's warp, 0.659 mm run to run; the field 1.70 and 0.70 Hz; the Mattes histograms' sums per work
  unit; larmorx the one-thread result at any thread count).
- `crates/larmorx-core/src/defects.rs`: the branch had lost a space in `SDC_01 ="SDC-01"` (inside the macro,
  so rustfmt did not see it); restored in `2a87704a`.
- The part's documents for `docs/parts/<date>-field-maps-b/`: `BRIEF.md`, `NOTES.md`, this file,
  `HANDOFF.toml` and relay 32's report.
- The scratch folder holds 48 GB after relay 32 (the build targets: `target` 19 GB, `target-nc` 9.3 GB,
  `target-gpl` 4.4 GB, `r31/target-merge` 0.8 GB; the rest the relays' helpers and outputs, the four trial
  worktrees included): the user's disk-space discussion (ORCHESTRATOR-STATE).

## The upstream reports (relay 32, in the branch: `5385a9f9`)

Seven drafts in `docs/upstream-reports/` ("not yet filed" / "not yet posted"; the user files them), each
with its smallest reproducer, run in both kept environments and with pybids' master where pybids matters
(`validation/investigations/sdcflows-fieldmap-b/relay32/`: the scripts, `run_reproducers.py`, the three
`outcomes-*.txt`). Each project's issues were searched first (GitHub's search, 2026-11-10), which changed
three of them:

| Draft | Project | Register | What it is |
|---|---|---|---|
| `sdcflows-shared-b0fieldidentifier.md` | sdcflows (and fMRIPrep) | PIPE-32, SDC-36 | a **comment** for the open sdcflows#455 (fMRIPrep#3354, closed as not planned, is the same failure): reproducer, cause, a contained fix (clear the registry per participant) and a measured side effect (automatic names depend on the other participants) |
| `sdcflows-entities-from-parent-folders.md` | sdcflows | SDC-45 | new issue |
| `fmriprep-entities-from-parent-folders.md` | fMRIPrep | PIPE-36 | new issue |
| `pybids-entities-from-parent-folders.md` | pybids | PIPE-37 | **nothing to file**: pybids#1270, fixed on its master by #1273 (2026-08-24), not released (0.22.0 the newest); `lx.bids`' default = that fix (measured) |
| `scipy-rank-filter-1d-output-type.md` | SciPy | SCI-21 (+ SCI-22, SCI-23) | new issue (memory safety first) |
| `scipy-binary-erosion-float-output-mask.md` | SciPy | SCI-25 | new issue |
| `scipy-label-bool-output.md` | SciPy | SCI-29 | a **comment** for the open scipy#24502 (its PR #24579 closed unmerged) |

Found on the way: with pybids' fix, SDC-45 and PIPE-36 turn silent below a folder named as a datatype
(`motion`): sdcflows' fieldmap-less estimator gets an empty `IntendedFor`, fMRIPrep's `get_sbrefs` finds no
reference (both drafts say so; `docs/findings/sdcflows-estimation.md`, "The upstream reports (relay 32)");
SCI-26 is fixed on SciPy's main (#25646, milestone 2.0.0). The register draft's sections (`UPSTREAM` in
`relay17/register_part_b.py`) now carry each entry's upstream status, so `HANDOFF.toml`'s register sections
of PIPE-32, SDC-36, SDC-45, PIPE-36, PIPE-37, SCI-21, SCI-22, SCI-23, SCI-25, SCI-26 and SCI-29 changed (the
summary rows and the findings' table did not: no re-export of the site's page needed). The reports' index,
`docs/upstream-reports/README.md`, is not on CLAUDE.md's list of shared documents and was edited in the
branch (seven rows; a sentence on drafts that are comments); move it into the handoff if you count it as
shared (no conflict: main has not changed it since the branch began).
