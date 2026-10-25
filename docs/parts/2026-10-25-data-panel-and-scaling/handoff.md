# HANDOFF: the data panel, and evidence that scales to it (2026-10-24)

Branches and commits (nothing pushed):
- larmorx `worktree-agent-a7c5a95288e70994d`: `bc7918d` (the panel's oracle-run views and runner),
  `569ea25` (the exporter: slices views, credits, the size guard, the blob manifest), `fda552e`
  (the user's rule: N4's ANTS-40 a whole volume again, crops of constant margins only, budgets).
- larmorx-testdata clone `<scratch>/panel/testdata`, branch `panel`: `6f6429e`, `18c28f1`,
  `7c477fc`, `d08c544`.
- site clone `<scratch>/panel/site`: branch `panel-site` `90ea9e2` (on `fb65cac`), and the orphan
  branch `evidence-blobs` `6587824` (294 blobs, 154.0 MB; the same commit every time it is made
  from this manifest).

**Proof of the user's rule** (in `<scratch>/panel/`): `inventory.md` (per page: examples, whole
volumes, slices, panels, comparisons, metric values, cases, tables, charts, images, shown numbers;
after ≥ before on all 16 pages; every shared JSON value equal except the exporter's commit hash and
three licence corrections; every existing image byte-identical; the browser checks: 3,368 → 3,405
numbers, all agreeing, 379 → 385 viewer checks, none bad; sizes per page) and
`shots/before-after/` (36 side-by-side images: every existing worked example at 1440 px light,
N4's ANTS-40 also dark and at 390 px, two more at 390 px, the Validation page's top). A second
export from `fda552e` was byte-identical (311 files).

## 0. Decisions for the orchestrator

1. **ds004884's lesion mask on the T1w's grid: accepted by the orchestrator (2026-10-24).**
   Every ARC lesion mask is drawn on a T2w and stored on its grid; no session has a mask on its
   T1w's grid, and fMRIPrep 25.2.6 needs the `_roi` in the T1w's space (sMRIPrep subtracts it
   voxel by voxel from a mask on the T1w's grid: niworkflows `create_cfm`). The oracle run reads a
   **derived input**: the mask resampled onto the T1w's grid by nearest neighbour through both
   files' scanner coordinates, no registration, by the committed script (larmorx
   `validation/investigations/panel-oracle-runs/bids_view.py`, `mask_on_grid`; 176,205 voxels,
   the original's 176.2 mL; on the infarct; SHA-256 `02440da7…cf962d0`). The session was chosen
   for it: its T2w and T1w align by their headers to 0.38° and at most 1.1 mm over the brain. The
   catalog keeps the upstream mask unchanged; the run's `RUN_INFO.txt`, the sources page and
   `panel.toml` say so.
2. **ds004958's subject changed** (no decision needed, reported): sub-fsm02cd's BOLD is truncated
   as published (OpenNeuro stores it so: 101.7 of 563 volumes, exactly 109 MiB, matching its annex
   key); the panel takes sub-fsm64ps (68, F; 600 volumes, the smallest complete run, every other
   file the proposal wanted). This also adds a fourth older adult.
3. **The budgets (proposed): 500 MB for the site, 40 MB per page by default** (the guard in
   every export), **raised per page with its reason, never by removing content** (the user's
   rule of 2026-10-24): N4's page is 50.7 MB (its ANTS-40 example a 40 MB whole volume, kept), so
   its budget is 60 MB (`sizes.PAGE_BUDGETS`, the reason printed in every size report). The site
   after this part: 158.2 MB (before: 157.8 MB).
4. **The user's rule of 2026-10-24 applied** (the coordinator's message): an interim version had
   turned N4's ANTS-40 example into three slices; it is a whole volume again (larmorx `fda552e`),
   identical to before. The slices form is used only for the new data-panel case (3dAutomask,
   below); crops leave out only margins where every array is exactly constant; the threshold head
   mask is gone. Proof: `<scratch>/panel/inventory.md` (per page, after ≥ before, every shared
   value equal except the exporter's hash and three licence corrections) and the side-by-side
   screenshots in `<scratch>/panel/shots/before-after/`.

## 1. Push steps (the site)

After merging `panel-site` into the site's `main` (it git-ignores `static/evidence/blobs/` and
removes the blob files from the index: their history stays), in the site's main checkout:

```bash
git fetch <scratch>/panel/site evidence-blobs:evidence-blobs     # the agent's blob branch (one commit)
python3 tools/blobs.py extract                                     # into static/evidence/blobs/, verified
# re-export the summary if the merge needs it, then, whenever the manifest changed:
python3 tools/blobs.py commit                                      # the branch from the manifest (deterministic)
python3 tools/build.py                                             # 0 problems
git push --force origin evidence-blobs                             # the images first
git push origin main                                               # then the JSON and the manifest
```

The Pages workflow (`.github/workflows/pages.yml`) checks `evidence-blobs` out into `_blobs/`,
runs `tools/blobs.py publish-dir --dir _blobs` (every blob of `static/evidence/blobs.sha256`
present with its SHA-256, no extra file, every blob a JSON names in the manifest) and builds.
If `main` is pushed before the branch, the deployment stops at that step (nothing broken is
published); push the branch and re-run the workflow. `verify-merge.sh --site` and the merge
checklist's site checkout need the blobs too: `python3 tools/blobs.py fetch` (or `extract` from a
fetched ref) before building.

## 2. larmorx `CLAUDE.md`, "Where things are": add after the "Part 2b merged" paragraph

> **The data panel and evidence that scales merged** (2026-10-24): fourteen OpenNeuro subjects in
> larmorx-testdata (`panel.toml`, `docs/panel.md`; `python -m larmorx_testdata fetch --tier full
> --tag panel`, 219 files, 2.35 GB; every file hash-verified and checked against its snapshot's
> content key, every anatomical image checked for defacing); the evidence exporter ships a
> per-example view (every existing example a whole volume, as before; new panel cases may be three
> full-resolution slices through the largest difference, cropped only where every array is
> constant), keeps integer types, credits every dataset from the catalog, guards the site's size
> (500 MB; 40 MB per page, raised with a reason, never by removing content: N4 60 MB;
> `evidence sizes`), and the site's images moved to the orphan branch `evidence-blobs`
> (`tools/blobs.py`; `static/evidence/blobs.sha256` on `main`); the site has a Data panel page,
> and 3dAutomask's page the first panel case (ds000256's extreme-motion child run). The panel's nine new fMRIPrep oracle runs all finished (exit 0; `docs/validation/oracle-runs.md`), and the pipeline-stage catalogue classifies every program node of the fourteen runs (multi-echo included).

And in "fMRIPrep oracle runs", after the list of five: "and the data panel's nine (2026-10-24
and 25, `validation/investigations/panel-oracle-runs/`, every one finished with exit status 0):
`ds000228-sub-pixar008-noreconall` (a 3.5-year-old), `ds005003-sub-RS001-noreconall` (a
glioma), `ds004173-sub-603309-noreconall` (anatomy only, strong motion), `ds002422-sub-02-noreconall`
(1.5 T, two field-map estimators), `ds000256-sub-CTS203-noreconall` (a child's three motion
levels), `ds004884-sub-M2079-noreconall` (a stroke, with its lesion mask resampled onto the T1w's
grid), `ds003592-sub-280-noreconall` (multi-echo), `ds002603-sub-01-noreconall` (Philips
multiband, a field map), `ds004958-sub-fsm64ps-noreconall` (multiband-8, 600 volumes; field maps
not linked)."

## 3. `docs/next-steps.md`, item 6: replace "**the data panel** (one agreed set … downloads);" by

> **the data panel: done** (2026-10-24; larmorx-testdata `docs/panel.md`: fourteen subjects, every
> criterion covered, defacing checked; the site's evidence scales to it: slices views for new
> panel cases, the blob branch, the size guard, credits; the site's Data panel page; 3dAutomask's
> first panel case); its nine new fMRIPrep oracle runs done (2026-10-25, all exit 0);

and after "Then every finished tool is re-validated on the panel, with its evidence page updated."
add: "Each tool: its existing examples unchanged; the panel's cases added beside them, two or
three key datasets as whole volumes, the rest as slices (`docs/figures.md`, Views); a page over
its budget gets a raised one with its reason, never less content (the user, 2026-10-24)."

Under "Follow-ups, when they fit", add:

> - **larmorx-testdata:** `ruff format --check` fails on main for three generators
>   (`generators/ants_filters_synthetic.py`, `resampling_synthetic.py`, `tshift_synthetic.py`;
>   untouched by the panel part); its CI's lint step needs them formatted.
> - **README and the validation index** still say the test data are "not published yet" (published
>   2026-10-23).
> - **Oracle runs in progress, in the parity suites:** `test_every_program_node_is_classified` now
>   skips a run that has not finished (`larmorx_validation.stages.oracle.run_finished`: RUN_INFO's
>   `finished:` line or fMRIPrep's end message). Two suites still enumerate every run without that
>   check: `parity/afni_phantoms.fmriprep_runs` (3dUnifize's and 3dAutomask's fMRIPrep cases, standard
>   tier) and `parity/itk_hdf5._found_files` (the runs' `.h5` files, standard and full tiers); a run
>   in progress can add half-written cases to them. One line each (`if run_finished(...)`), left to
>   their owners (the panel part's brief kept it off the parity runners).
> - **The site: a JavaScript syntax check in `tools/build.py`** (`node --check` on `static/js/*.js`
>   when node is present). During the panel part an apostrophe in a single-quoted string broke
>   `evidence.js` for every page while the build still reported 0 problems; only the browser
>   number check caught it (fixed before the commit).

## 4. `docs/decision-log.md`: a new entry

> ## 2026-10-24: The data panel; evidence at full quality that scales
>
> **Approved by the user**: the research agent's proposal of a data panel, its twelve datasets plus
> ds002603 (Philips multiband) and one stroke subject with a manual lesion mask from ds004884 (the
> Aphasia Recovery Cohort): fourteen datasets, in larmorx-testdata (`panel.toml`, `docs/panel.md`).
> **Agreed with the user the same day**, the site's evidence at full quality that scales to it:
> voxel values stay lossless; integer data ship in their own type; a view is cropped only where
> every array it shows (input, outputs, differences, masks) is exactly constant, its offset
> recorded; every existing example stays as it is, a whole volume; new panel cases are added
> beside them, a tool's two or three key datasets as whole volumes, the others as three
> full-resolution slices through the voxel of largest difference, every metric still over every
> voxel; the images leave the site's `main` history for an orphan branch `evidence-blobs`
> replaced at every export (verified against a committed manifest by the Pages workflow); a size
> guard (500 MB for the site; 40 MB per page, raised for a page with its reason, never by removing
> content: N4 60 MB); every example credited from the catalog (CC-BY attribution, CC-BY-SA
> share-alike notice); a Data panel page.
> **The user's rule (2026-10-24):** the website never shows less, or lower quality, than before;
> every change adds quality and information (nothing existing removed or reduced; no existing
> 3D view turned into slices; crops only of exactly constant margins, never a threshold box; the
> size guard never removes content). Each change is proven by a before/after inventory per page
> and side-by-side screenshots.
> **Found while building it:** ds004958 sub-fsm02cd's BOLD is truncated as published (the panel
> takes sub-fsm64ps); every ds004884 lesion mask is drawn on a T2w (the oracle run reads the mask
> resampled onto the T1w's grid by the scanner coordinates, a derived input accepted the same day); three examples stated a
> wrong licence (ds005454 is CC-BY-4.0, ds000005 PDDL-1.0), corrected and now checked by the export.

## 5. `docs/validation/oracle-runs.md`

After the five runs' sections, add:

> ## The data panel's nine runs (2026-10-24 and 25)
>
> The data panel (larmorx-testdata `docs/panel.md`) added nine datasets. Their runs read a **BIDS
> view** each (`<workspace>/oracles/bids-views/<dataset>`): the panel's files hard-linked from the
> test-data cache, so fMRIPrep saw exactly the files the catalog lists and verified
> (`validation/investigations/panel-oracle-runs/`: `bids_view.py`, `run_panel_oracles.sh`,
> `verify_runs.py`). They ran one after the other at `--nprocs 6 --omp-nthreads 6` (the five
> earlier runs used 8) on a machine shared with three development agents, so their times are not
> benchmarks; outputs are unaffected. Every run ended with exit status 0, fMRIPrep's end message,
> no crash file and every expected output (`verify_runs.py`); every program node of every run is
> classified by the pipeline-stage catalogue (`unclassified.py`: 0 left over the fourteen runs).
> **ds004884's lesion mask is a derived input** (accepted by the orchestrator, 2026-10-24): the
> dataset's manual mask, drawn on the T2w and stored on its grid, resampled onto the T1w's grid
> by nearest neighbour through both files' scanner coordinates, no registration, by
> `bids_view.py` (`mask_on_grid`), because fMRIPrep reads a lesion mask only in the T1w's space;
> the test-data catalog keeps the original. The template registration read it (its node's
> `lesion_mask` input). After each run the runner appended the BIDS view and its own commit to
> `RUN_INFO.txt` (the lines from `BIDS view:` on), and for ds004884 the derived input.
>
> | Run | What it adds | Minutes | BOLD series | Field maps applied |
> |---|---|---:|---:|---|
> | `ds000228-sub-pixar008-noreconall` | a 3.5-year-old: small head, oblique field of view | 66 | 1 | none (none acquired) |
> | `ds005003-sub-RS001-noreconall` | a 70 mL frontal oligodendroglioma | 78 | 2 | none (none acquired) |
> | `ds004173-sub-603309-noreconall` | one MPRAGE, strong instructed head motion (`--anat-only`) | 69 | 0 | (anatomical only) |
> | `ds002422-sub-02-noreconall` | 1.5 T, an older adult, float32 T1w | 95 | 3 | two estimators (`wf_auto_00000`, `wf_auto_00001`), 3 runs unwarped |
> | `ds000256-sub-CTS203-noreconall` | a 7.5-year-old's three rest runs of graded head motion | 70 | 3 | none (none acquired) |
> | `ds004884-sub-M2079-noreconall` | a chronic stroke with its lesion mask (the derived input below) | 73 | 2 | none (none acquired) |
> | `ds003592-sub-280-noreconall` | an 82-year-old, three-echo BOLD (the first multi-echo run: `t2smap`, the T2* maps) | 84 | 1 (3 echoes) | none (none acquired) |
> | `ds002603-sub-01-noreconall` | Philips, single-band and multiband-4 of the same task | 90 | 2 | one estimator, 2 runs unwarped |
> | `ds004958-sub-fsm64ps-noreconall` | multiband-8, 600 volumes, an older adult | 100 | 1 | none: its spin-echo pair and GRE phase difference are not linked (no `IntendedFor`, no `B0FieldIdentifier`), as the panel intends |

then one section per run, in the order run (verbatim from each `RUN_INFO.txt`, machine paths as
`<workspace>` and `~`; printed by `verify_runs.py`):

### ds000228-sub-pixar008-noreconall

```text
fMRIPrep oracle run: ds000228-sub-pixar008-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000228
participant: pixar008
arguments: /data /out participant --participant-label pixar008 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T19:21:12Z
finished: 2026-10-24T20:27:14Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000228, the panel's files of ds000228 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds005003-sub-RS001-noreconall

```text
fMRIPrep oracle run: ds005003-sub-RS001-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds005003
participant: RS001
arguments: /data /out participant --participant-label RS001 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T20:28:02Z
finished: 2026-10-24T21:46:08Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds005003, the panel's files of ds005003 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds004173-sub-603309-noreconall

```text
fMRIPrep oracle run: ds004173-sub-603309-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004173
participant: 603309
arguments: /data /out participant --participant-label 603309 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall --anat-only
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T21:46:35Z
finished: 2026-10-24T22:55:19Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004173, the panel's files of ds004173 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds002422-sub-02-noreconall

```text
fMRIPrep oracle run: ds002422-sub-02-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds002422
participant: 02
arguments: /data /out participant --participant-label 02 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-24T22:56:07Z
finished: 2026-10-25T00:31:06Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds002422, the panel's files of ds002422 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds000256-sub-CTS203-noreconall

```text
fMRIPrep oracle run: ds000256-sub-CTS203-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds000256
participant: CTS203
arguments: /data /out participant --participant-label CTS203 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T00:31:33Z
finished: 2026-10-25T01:41:19Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds000256, the panel's files of ds000256 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds004884-sub-M2079-noreconall

```text
fMRIPrep oracle run: ds004884-sub-M2079-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004884
participant: M2079
arguments: /data /out participant --participant-label M2079 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T01:41:52Z
finished: 2026-10-25T02:55:12Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004884, the panel's files of ds004884 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
derived input: sub-M2079/ses-217/anat/sub-M2079_ses-217_label-lesion_roi.nii.gz, sha256
  02440da7ebdb5625ce344e4997bd52545a656dc81b544c78c14514574cf962d0,
  the dataset's manual lesion mask (derivatives/lesion_masks/sub-M2079/ses-217/anat/
  sub-M2079_ses-217_acq-spc3_run-4_T2w_desc-lesion_mask.nii.gz, drawn on the T2w's grid)
  resampled onto the T1w's grid (sub-M2079_ses-217_acq-tfl3p2_run-3_T1w) by nearest
  neighbour through both files' scanner coordinates, no registration, by bids_view.py's
  mask_on_grid; the test-data catalog keeps the original mask unchanged
```

### ds003592-sub-280-noreconall

```text
fMRIPrep oracle run: ds003592-sub-280-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds003592
participant: 280
arguments: /data /out participant --participant-label 280 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T02:55:51Z
finished: 2026-10-25T04:19:24Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds003592, the panel's files of ds003592 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds002603-sub-01-noreconall

```text
fMRIPrep oracle run: ds002603-sub-01-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds002603
participant: 01
arguments: /data /out participant --participant-label 01 -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T04:20:01Z
finished: 2026-10-25T05:49:29Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds002603, the panel's files of ds002603 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

### ds004958-sub-fsm64ps-noreconall

```text
fMRIPrep oracle run: ds004958-sub-fsm64ps-noreconall
image: docker://nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e (tag 25.2.6)
image file: <workspace>/oracles/images/fmriprep-25.2.6.sif, sha256 7f84379b40f399876909ecf7c236105d6b7c43fdec5111fb7b03f8dee204ac10
fmriprep: fMRIPrep v25.2.6
apptainer: apptainer version 1.4.5
BIDS input: <workspace>/oracles/bids-views/ds004958
participant: fsm64ps
arguments: /data /out participant --participant-label fsm64ps -w /work --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash --nprocs 6 --omp-nthreads 6 --random-seed 0 --skull-strip-fixed-seed --output-spaces MNI152NLin2009cAsym:res-2 T1w func --fs-no-reconall
seeds: --random-seed 0 --skull-strip-fixed-seed; threads: --nprocs 6 --omp-nthreads 6
host: Linux 6.8.0-124-generic x86_64, 12 CPUs, 31 GB RAM
started: 2026-10-25T05:50:14Z
finished: 2026-10-25T07:30:11Z
exit status: 0
BIDS view: <workspace>/oracles/bids-views/ds004958, the panel's files of ds004958 hard-linked from the test-data cache
  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit 3e8abda8b0d22df0bdd2e617712f007d16b22737)
```

And in the file's introduction, after "...the arguments, seeds and threads.": "The data panel's
nine runs read BIDS views of hard links (below)."

## 6. README (larmorx), line 187, and `docs/validation/README.md`, line 3

"(`larmorx-testdata`, a separate repository kept next to this one; not published yet)" →
"(`larmorx-testdata`, github.com/karellopez/larmorx-testdata, kept next to this one; its data
panel, `docs/panel.md` there, is the fixed set every tool is validated on)".

## 7. The site

**`site.yml`**, `docs:` — add after Validation:

```yaml
  - { title: Data panel, url: "/data-panel/" }
```

**`content/validation.md`**, "The method", after the *Parity tiers* paragraph (before the code
block), add:

> **One data panel for every tool.** Since 2026-10-24 every tool is to be validated, and shown, on
> the same fourteen shareable datasets: 1.5, 3 and 7 T; Siemens, GE and Philips; single-band,
> multiband, multi-echo and sparse BOLD; every kind of field map; children, older adults, a
> tumour and a stroke; head motion and low quality ([Data panel](/data-panel/)). The finished
> tools are re-validated on it one by one.

and in "How to read the evidence", after the *What was run, on what* bullet:

> - **Whole volumes, and three slices for panel cases.** Every example is a whole volume, except
>   cases from the data panel beyond a tool's key datasets: those show three full-resolution
>   slices, one per plane, through the voxel where the comparison differs most (the example says
>   so under its panels). Every metric is over every voxel of the files either way. Every example
>   carries the credit line of the data it shows, and every page ends with its data credits.

**The site's `CLAUDE.md`**, "Validation evidence", replace the third bullet ("The data under
`static/evidence/` is content-addressed …") by:

> - The data under `static/evidence/` is content-addressed and byte-deterministic; its images
>   (`blobs/`) are not on `main` but on the orphan branch `evidence-blobs`, one commit replaced at
>   every export and verified by the Pages workflow against `static/evidence/blobs.sha256`
>   (`tools/blobs.py`: `fetch` before a local build, `commit` after an export; push the branch
>   before `main`). Brain-masked, or the head of an image whose defacing was checked
>   (larmorx-testdata `panel.toml`, `show = "head"`), never a recognisable face; only data whose
>   licence allows it, each credited from the test-data catalog under every example and at the
>   page's end (CC-BY attribution, CC-BY-SA share-alike notice); templates with their notices.
> - The size guard of every export: the site at most 500 MB, a page at most 40 MB unless its
>   budget is raised with a reason (`sizes.PAGE_BUDGETS`; N4 60 MB); `python -m
>   larmorx_validation evidence sizes`. The guard never removes content.
> - The site never shows less, or lower quality, than before (the user, 2026-10-24): nothing
>   existing is removed or reduced, no existing whole volume becomes slices, crops leave out only
>   exactly constant margins; every change is proven with a before/after inventory per page and
>   side-by-side screenshots.
> - The Data panel page (`content/data-panel.md`) describes the fixed data every tool is shown on.

and under "Build, preview, deploy", before the first command: `python3 tools/blobs.py fetch   # the evidence's images (branch evidence-blobs)`.
