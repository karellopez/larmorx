# HANDOFF: version update, part 1 (AFNI 25.2.09 → 26.3.05), 2026-10-29

Branch: the worktree's (`worktree-agent-a08130d4a33db225a`), from main `17fbfdd`. Site branch:
`version-afni-26` in `<scratch>/site` (a clone of `larmorx_documentation`).

Each block below is the exact text for one shared document, with where it goes.

## 1. `docs/findings/upstream-defects.md`

### 1a. Table rows (replace the two rows)

```
| AFNI-03 † | 3dTshift (3dAutomask, 3dUnifize: no slice timing written) | header slice code 2 (sequential decreasing): no shift (AFNI 25.2.09; **fixed in AFNI 26.3.05**) | c | none | corrected by default in both implementations (replica 2026-10-12); fixed upstream in AFNI 26.3.05 exactly as larmorx corrected it: the GPL replicas follow 26.3.05 in both modes and name it no more (2026-10-29); the clean-room originals still reproduce 25.2.09's zeros in `reproduce` and name it (until aligned: next-steps) | – |
| AFNI-05 † | 3dTshift | `-no_detrend` de-means one voxel of each pair only (AFNI 25.2.09; **fixed in AFNI 26.3.05**) | c | none | corrected by default in both implementations (replica 2026-10-12); fixed upstream in AFNI 26.3.05 exactly as larmorx corrected it: the GPL replica follows 26.3.05 in both modes and names it no more (2026-10-29); the clean-room original still reproduces 25.2.09's pairs in `reproduce` (until aligned) | – |
```

### 1b. Section "AFNI-03", replace its "larmorx" and "D13 / gap" bullets

```
- **larmorx:** both implementations visit the slices from `slice_end` down to
  `slice_start` by default and reproduced AFNI 25.2.09's zeros with the switch, warning in both
  modes (the replica since 2026-10-12: `larmorx_gpl_afni::dataset::slice_offsets`). Only the
  command line's header-timing path meets it; the Python API requires `slice_times`.
- **AFNI 26.3.05** (2026-10-29): fixed upstream, `thd_niftiread.c:652` now loops down to
  `slice_start`, the same float sums of a double step as larmorx's correction. The GPL replicas
  follow 26.3.05 in both modes (`reproduce` gives 26.3.05's result; 25.2.09's is that of
  earlier larmorx releases) and print no notice; their records show it in "AFNI's own step"
  (`header-timing/code2-*`, the permanent cases `difftest/slice-code-2-masked-afni26` and
  `difftest/epi-slice-code-2-afni26`). The clean-room originals still reproduce 25.2.09 until a
  clean-room agent aligns them (`specs/3dTshift.md` §3.2 and §6.4, `specs/3dAutomask.md` §9,
  marked "AFNI 26.3.05").
- **D13 / gap:** done (fixed upstream).
```

### 1c. Section "AFNI-05", replace its "larmorx" and "D13 / gap" bullets

```
- **larmorx:** both implementations de-mean each voxel with its own mean by default and
  reproduced AFNI 25.2.09's pairs with the switch, warning in both modes (the replica since
  2026-10-12: `larmorx_gpl_afni::tshift`, `shift_pair`); the original's last bits are not exact
  in either mode.
- **AFNI 26.3.05** (2026-10-29): fixed upstream, `3dTshift.c:633` now passes `gar+ignore`, the
  computation larmorx's correction made. The GPL replica follows 26.3.05 in both modes (no
  notice); its record shows the step on `options/*-no-detrend`. The clean-room original still
  reproduces 25.2.09's pairs until aligned (`specs/3dTshift.md` §4, marked "AFNI 26.3.05").
- **D13 / gap:** done (fixed upstream).
```

### 1d. History paragraph (append)

```
**2026-10-29, the version update (AFNI 25.2.09 → 26.3.05):** of the AFNI entries, AFNI-03 and
AFNI-05 are fixed upstream in 26.3.05, the same way larmorx corrected them; the GPL replicas
follow 26.3.05 (both modes, no notice), the clean-room originals reproduce 25.2.09 until aligned.
No other AFNI entry changed (the source of every translated file and the machine code of every
function of the three programs compared: `validation/investigations/version-afni/`). No new
defect; no ID of AFNI-76 to AFNI-85 used.
```

## 2. `docs/api/upstream-defects.md` (replace the two rows)

```
| AFNI-03 | 3dTshift | `larmorx afni 3dTshift` (header slice timing); 3dAutomask's `-apply_prefix` and 3dUnifize `-EPI` keep the corrected slice fields (both implementations) | AFNI 25.2.09: header slice code 2 (sequential decreasing) gives every slice time 0: nothing is shifted. **Fixed in AFNI 26.3.05** (the replicas follow it in both modes) | slices `slice_end` down to `slice_start`, as NIfTI defines the code |
| AFNI-05 | 3dTshift | `lx.afni.tshift(detrend=False)`, `-no_detrend` | AFNI 25.2.09: only the first voxel of each pair has its mean removed and restored; the second is shifted raw (its edges fall toward 0). **Fixed in AFNI 26.3.05** (the replica follows it in both modes) | every voxel has its mean removed and restored |
```

## 3. `docs/licensing.md` (append to "Notes checked 2026-10-19", after the AFNI bullet)

```
- **AFNI 26.3.05 relicenses MCW's code** (checked 2026-10-29,
  [findings/afni-relicensing-26.md](findings/afni-relicensing-26.md)): "As of May 12, 2026, MCW
  has granted NIH a Creative Commons Attribution License (CC BY 4.0) for all of the code under
  its copyright" (`doc/README/README.copyright`); every MCW file's header now says CC BY 4.0.
  None of the 55 AFNI files the replicas derive from is GPL in 26.3.05. The AFNI replicas could
  therefore move into the main Apache package (attribution, CC BY 4.0's text, `sinf`/`cosf`
  re-sourced from Arm's MIT routines; glibc's LGPL `cbrtf` measured or kept apart): **the user's
  decision**, nothing moved. Until then they stay in `larmorx-gpl` (CC BY 4.0 allows the GPL-3
  package), and `crates-gpl/NOTICE` carries the CC BY 4.0 attribution.
```

## 4. `UPSTREAM.md`

Replace the AFNI row of "Ported sources":

```
| AFNI | public domain (NIH) + CC BY 4.0 (code copyrighted by the Medical College of Wisconsin; GPL-2 up to 25.2.09) | `AFNI_26.3.05` → `07c1dcfb5fdc` (since 2026-10-29; before: `AFNI_25.2.09` → `b1e12b26dae2`, kept in `tags/AFNI-25.2.09` and `oracles/afni-25.2.09`) | AFNI_25.2.09 | the GPL replicas (`crates-gpl/`, GPL-3.0-or-later) follow 26.3.05; the clean-room originals were validated against 25.2.09 and follow 26.3.05 once a clean-room agent aligns them (AFNI-03, AFNI-05). The MCW files' relicensing (CC BY 4.0) is inventoried in `docs/findings/afni-relicensing-26.md` (no move without the user's decision). The tags are also the sources of the AFNI oracles (`scripts/build_afni_oracle.sh`) |
```

and in the policy paragraph replace "AFNI (newest 26.3.05) and FSL (newest 6.0.7.23) wait for the
update part after the move" with "AFNI moved to 26.3.05 on 2026-10-29 (part 1 of the version
update); FSL (newest 6.0.7.23) waits for part 2".

## 5. `upstream.tsv` (add after the AFNI-25.2.09 row)

```
AFNI-26.3.05	https://github.com/afni/afni.git	07c1dcfb5fdc933d2429669826bcbddbc4047461	public-domain+exceptions	tags
```

## 6. `CLAUDE.md`, rule 2

Replace "   - AFNI 25.2.09" with "   - AFNI 26.3.05 (since 2026-10-29; 25.2.09 kept for the clean-room
originals until they are aligned, and for the step between the two)". In "Where things are",
the AFNI row's tools: "3dTshift, 3dAutomask, 3dUnifize, complete: clean-room originals
(`crates/larmorx-afni`, validated against AFNI 25.2.09) and GPL replicas (`crates-gpl/`,
validated against AFNI 26.3.05 since 2026-10-29)". In "Oracle builds":
"`scripts/build_afni_oracle.sh` (`oracles/afni-25.2.09`, `oracles/afni-26.3.05`)".

## 7. `docs/next-steps.md`

Item 3 of "The order from here", replace:

```
3. **The version update** (user 2026-10-27, decision log; the upstream watch with it):
   **part 1, AFNI: done** (2026-10-29, `docs/parts/2026-10-29-version-afni/`): AFNI 26.3.05's
   changes to the three programs are two, both fixes of defects larmorx already corrected
   (AFNI-03, AFNI-05); the GPL replicas follow 26.3.05 (records against it, AFNI's own step from
   25.2.09 in each case); the clean-room originals stay on 25.2.09 until aligned (below); the
   upstream watch runs weekly from the nightly job; the relicensing inventory waits for the user.
   Part 2: FSL to its newest 6.0.7.x (6.0.7.23 today; the watch lists 15 of the replicas' 57 FSL
   files changed and one removed), SciPy, NumPy and OpenBLAS checked; each tool aligned with its
   newest original, with the per-part testing.
```

New follow-up (with the clean-room items):

```
- **The clean-room originals follow AFNI 26.3.05** (a clean-room agent; from the specs only,
  lines marked "AFNI 26.3.05"): (1) 3dTshift `-no_detrend`: `reproduce` de-means every voxel
  with its own mean, as `fix` does, and AFNI-05 is never named (`specs/3dTshift.md` §4); (2)
  header slice code 2 in 3dTshift, 3dAutomask (`-apply_prefix`) and 3dUnifize (`-EPI`):
  `reproduce` reads the times `end, end−1, …, start` as `fix` does, a copy where they exceed the
  TR, and AFNI-03 is never named (`specs/3dTshift.md` §3.2, §6.4, §6.5; `specs/3dAutomask.md`
  §9; `specs/3dUnifize.md`); (3) the permanent cases `difftest/slice-code-2-masked-notices`,
  `epi-slice-code-2-fix-notice`, `epi-slice-code-2-reproduce-notices` re-described for 26.3.05;
  (4) their records re-made against 26.3.05 and `DEFAULT["original"]` in
  `validation/src/larmorx_validation/parity/afni_versions.py` set to 26.3.05; (5) version strings
  (`PROVENANCE.md`: black-box runs of `oracles/afni-26.3.05`). Against 26.3.05 today
  (`--afni-version 26.3.05`, standard tier): 3dTshift 6 of 315 cases fail (`options/*-no-detrend`
  ×4, `header-timing/code2-seq-dec`, `header-timing/code2-beyond-tr`), 3dAutomask 1 of 130
  (`difftest/slice-code-2-masked-notices`), 3dUnifize 2 of 117 (`difftest/epi-slice-code-2-*`);
  every other case agrees as against 25.2.09; random testing against 26.3.05: 190 findings and 62
  notices, every one AFNI-03 or AFNI-05.
```

"Waiting for the user" (add):

```
- **AFNI's relicensing (CC BY 4.0):** move the AFNI replicas into the main Apache package?
  [findings/afni-relicensing-26.md](findings/afni-relicensing-26.md) lists what it needs.
  Recommendation: yes, as its own part, once `cbrtf` (3dUnifize `-EPI`'s radius) is measured
  (rule D) and `sinf`/`cosf` are re-sourced from Arm's MIT routines; `pip install larmorx` would
  then give AFNI's exact bits without the GPL package.
```

## 8. `validation/difftest/triage.toml` (append)

The trial (`difftest --tools afni-tshift,afni-automask,afni-unifize --budget 0.5 --afni-version
26.3.05 --jobs 8 --out <workspace>/oracles/difftest/2026-10-29-version-afni`, larmorx `bd39a5d`):
13,063 cases in 30.6 min; the replicas: no difference (notices without changed values as in every
night: AFNI-01, -07, -16, -45; AFNI-75's guard on 11 heavy cases); the clean-room originals: 190
findings and 62 notices (AFNI-05 59, AFNI-03 3), every one AFNI-03 or AFNI-05 by AFNI's own step
(`validation/investigations/version-afni/triage_trial.py`). The nightly runs keep each
implementation's default build (the originals on 25.2.09), so these entries document the trial:

```toml
# ---------------------------------------------------------------- the version update (2026-10-29)

[[difference]]
tool = "afni-tshift"
implementation = "original"
signature = ["values", "values (integer output)", "notice: AFNI-05"]
status = "open"
explanation = "AFNI 26.3.05 fixed AFNI-05 (-no_detrend de-means every voxel); the clean-room original's `reproduce` still gives 25.2.09's pairs and names AFNI-05 (the trial against 26.3.05: 75 findings, 59 notices); its alignment is the clean-room agent's (specs/3dTshift.md §4, marked AFNI 26.3.05)"
link = "<workspace>/oracles/difftest/2026-10-29-version-afni/findings/afni-tshift/20261029-00092-original.json"

[[difference]]
tool = "afni-tshift"
implementation = "original"
signature = ["header fields (scaling, timing, slice fields, units, xform codes)", "DN fix pass", "TR (pixdim[N])", "notice: AFNI-03"]
status = "open"
explanation = "AFNI 26.3.05 fixed AFNI-03 (header slice code 2 read end..start); the clean-room original's `reproduce` still reads 25.2.09's zeros and names AFNI-03 (the trial against 26.3.05: 73 findings, 4 with AFNI-05 too); its alignment is the clean-room agent's (specs/3dTshift.md §3.2, §6.4)"
link = "<workspace>/oracles/difftest/2026-10-29-version-afni/findings/afni-tshift/20261029-00014-original.json"

[[difference]]
tool = "afni-automask"
implementation = "original"
signature = ["masked.nii: header bytes"]
status = "open"
explanation = "AFNI 26.3.05 fixed AFNI-03: -apply_prefix's output keeps a code-2 series' slice timing; the clean-room original's `reproduce` writes 25.2.09's (25 findings against 26.3.05); its alignment is the clean-room agent's (specs/3dAutomask.md §9)"
link = "<workspace>/oracles/difftest/2026-10-29-version-afni/findings/afni-automask/20261029-00046-original.json"

[[difference]]
tool = "afni-unifize"
implementation = "original"
signature = ["uni.nii: header bytes", "uni.nii.gz: header bytes"]
status = "open"
explanation = "AFNI 26.3.05 fixed AFNI-03: -EPI's output keeps a code-2 series' slice timing; the clean-room original's `reproduce` writes 25.2.09's (21 findings against 26.3.05); its alignment is the clean-room agent's (specs/3dUnifize.md)"
link = "<workspace>/oracles/difftest/2026-10-29-version-afni/findings/afni-unifize/20261029-00219-original.json"
```

(The links name findings outside git, as the trial's are; promote one of each with `difftest
--promote` if the orchestrator prefers a committed reproducer. The clean-room originals' own
permanent cases `slice-code-2-masked-notices`, `epi-slice-code-2-fix-notice` and
`epi-slice-code-2-reproduce-notices` already reproduce AFNI-03 against 26.3.05.)

## 9. `validation/tools.toml` and the generated tables

Done in the branch (`validation/tools.toml`: the replicas' references name AFNI 26.3.05 and the
step; `python -m larmorx_validation tables` rewrote the generated blocks of `README.md` and
`docs/validation/README.md`; the site's `data/status.yml` by `tables --site` on the site branch).

## 10. The site (branch `version-afni-26` of `<scratch>/site`, commits `4df1bc5`, `abc7dbe`)

- The three AFNI pages: every worked example gains AFNI 26.3.05's panel (the replica's
  reference), "replica − AFNI 26.3.05" and "AFNI 26.3.05 − AFNI 25.2.09" (a reference row: the
  version step, the same bytes on every call shown, 10 examples) and an "AFNI versions" fact; the
  page text says which version each record ran and what changed; the replicas' cases from their
  new records (the AFNI step reported per case). `summary.json` re-exported; `data/status.yml`
  generated. No new image blob (the new panels are AFNI 25.2.09's bytes; the manifest is
  unchanged), so `evidence-blobs` needs no new commit.
- Before/after inventory (`validation/investigations/panel-ants/site_inventory.py`, the JSON per
  page: NO LOSS on all three; `validation/investigations/difftest/site_inventory.py`, the built
  site): shown numbers 155 → 159 (3dTshift), 145 → 154 (3dAutomask), 174 → 180 (3dUnifize);
  tables' rows, links and words grew; no shown number removed; the values the tool lists as
  changed are all increases (cases 124 → 125 and 115 → 116, bit-identical 97 → 98 and 90 → 91: the
  new permanent cases), on the two pages and the Validation page.
- Number check (`LX_EVIDENCE.check()` via `tools/review/shoot.mjs --eval`): 158/158, 153/153,
  179/179 numbers agree, Validation 350/350; build 0 problems (links into the repository checked).
  Screenshots before and after, desktop and phone, light and dark: `<scratch>/shots/` (made by
  `validation/investigations/version-afni/site_shots.sh`).

## 11. The upstream watch's first report

### Upstream watch, 2026-10-29 (as it ran; `UPSTREAM.md` still named AFNI 25.2.09)

Made by `scripts/orchestration/upstream_watch.py`: the newest release of every upstream of `UPSTREAM.md`, and for a newer one the files the crates' `PROVENANCE.md` name that it changed (the release trees in `<workspace>/tags/`, FSL's in `~/fsl-<version>/src`).

| Upstream | larmorx | Newest | Status | Named files changed | Found by |
|---|---|---|---|---|---|
| ANTs | v2.6.5 | 2.6.5 | newest | – | git ls-remote --tags |
| ITK | v5.4.5 | 5.4.8 | **newer release** | not compared (below) | git ls-remote --tags |
| ITK remote module AdaptiveDenoising | 24825c8d2469 | 0ba75b2c9f4f | **newer release** | not compared (below) | git ls-remote (HEAD) |
| AFNI | AFNI_25.2.09 | 26.3.05 | **newer release** | 37 of 60 (27 licence text only) | git ls-remote --tags; AFNI.version says AFNI_26.3.05 |
| FreeSurfer | v8.2.0 | 8.2.0 | newest | – | git ls-remote --tags |
| CORE-MATH | 040ee482a8ca | c7100a074285 | **newer release** | not compared (below) | git ls-remote (HEAD) |
| GNU C Library | glibc-2.35 | 2.44 | **newer release** | not compared (below) | git ls-remote --tags |
| OpenBLAS | v0.3.30 | 0.3.34 | **newer release** | 4 of 26 | git ls-remote --tags |
| SciPy | v1.15.2 | 1.18.1 | **newer release** | 5 of 5 | PyPI |
| numpy | 2.3.5 | 2.5.4 | **newer release** | 3 of 3 | PyPI |
| nibabel | 5.4.2 | 5.4.2 | newest | – | PyPI |
| nitransforms | 25.1.0 | 25.1.0 | newest | – | PyPI |
| fMRIPrep | 21a490fb89ac | 25.2.7 | **newer release** | not compared (below) | PyPI |
| FSL | 6.0.7.17 | 6.0.7.23 | **newer release** | 15 of 57 (1 removed) | FSL's release manifest |

### ITK: 5.4.5 → 5.4.8

- follows ANTs: larmorx ports the ITK that ANTs' newest release builds with.
- 3 newer release(s): 5.4.8, 5.4.7, 5.4.6; compared: 5.4.5 against 5.4.8 (cumulative).
- not compared: fetch <workspace>/tags/ITK-v5.4.8 (tag v5.4.8).

### ITK remote module AdaptiveDenoising: 24825c8d2469 → 0ba75b2c9f4f

- follows ITK: the commit ITK's release fetches.
- not compared: the branch moved from 24825c8d2469 to 0ba75b2c9f4f: fetch it to compare.

### AFNI: 25.2.09 → 26.3.05

- AFNI releases several times a month: updated in batches unless a release changes results.
- 50 newer release(s): 26.3.05, 26.3.04, 26.3.03, 26.3.02, 26.3.01, 26.3.00, 26.2.09, 26.2.08, 26.2.07, 26.2.06, 26.2.05, 26.2.04 ...; compared: 25.2.09 against 26.3.05 (cumulative).
- 60 named files: 23 unchanged, 37 changed, 0 removed.
  - changed: `src/3dTshift.c`, 9 lines (larmorx-gpl-afni)
  - changed: `src/3ddata.h`, 13 lines (larmorx-gpl-afni)
  - changed: `src/AFNI_atlas_spaces.niml`, 79 lines (larmorx-gpl-afni)
  - changed: `src/machdep.c`, 68 lines (larmorx-gpl-afni)
  - changed: `src/mrilib.h`, 7 lines (larmorx-gpl-afni)
  - changed: `src/thd_automask.c`, 196 lines (larmorx-gpl-afni)
  - changed: `src/thd_getpathprogs.c`, 18 lines (larmorx-gpl-afni)
  - changed: `src/thd_niftiread.c`, 2 lines (larmorx-gpl-afni)
  - changed: `src/thd_niml.c`, 3 lines (larmorx-gpl-afni)
  - changed: `src/thd_ttatlas_query.c`, 19 lines (larmorx-gpl-afni)
  - changed: `src/afni_environ.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/afni_environ.h`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/cs_qmed.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/csfft.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/edt_buildmask.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/edt_fullcopy.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/mri_cut.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/mri_max.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/mri_percents.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/mri_read.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/mri_to_float.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_1Dtodset.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_coords.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_detrend.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_dsetto1D.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_filestuff.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_floatscan.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_initdblk.c`, 6 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_makemask.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_mastery.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_notes.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_opendset.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_shift2.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_timeof.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_writedset.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/thd_zblock.c`, 4 lines, licence text only (larmorx-gpl-afni)
  - changed: `src/vecmat.h`, 4 lines, licence text only (larmorx-gpl-afni)

### CORE-MATH: 040ee482a8ca → c7100a074285

- no releases: its master branch.
- not compared: the branch moved from 040ee482a8ca to c7100a074285: fetch it to compare.

### GNU C Library: 2.35 → 2.44

- larmorx follows the glibc the oracles run with (Ubuntu 22.04's 2.35), not the newest.
- 9 newer release(s): 2.44, 2.43, 2.42, 2.41, 2.40, 2.39, 2.38, 2.37, 2.36; compared: 2.35 against 2.44 (cumulative).
- not compared: fetch <workspace>/tags/glibc-2.44 (tag glibc-2.44).

### OpenBLAS: 0.3.30 → 0.3.34

- 4 newer release(s): 0.3.34, 0.3.33, 0.3.32, 0.3.31; compared: 0.3.30 against 0.3.34 (cumulative).
- 26 named files: 22 unchanged, 4 changed, 0 removed.
  - changed: `driver/level3/level3.c`, 25 lines (larmorx-transform)
  - changed: `interface/gemm.c`, 86 lines (larmorx-transform)
  - changed: `interface/lapack/gesv.c`, 16 lines (larmorx-transform)
  - changed: `param.h`, 586 lines (larmorx-transform)

### SciPy: 1.15.2 → 1.18.1

- 9 newer release(s): 1.18.1, 1.18.0, 1.17.1, 1.17.0, 1.16.3, 1.16.2, 1.16.1, 1.16.0, 1.15.3; compared: 1.15.2 against 1.18.1 (cumulative).
- 5 named files: 0 unchanged, 5 changed, 0 removed.
  - changed: `scipy/ndimage/_interpolation.py`, 66 lines (larmorx-interp)
  - changed: `scipy/ndimage/_ni_support.py`, 6 lines (larmorx-interp)
  - changed: `scipy/ndimage/src/ni_interpolation.c`, 58 lines (larmorx-interp)
  - changed: `scipy/ndimage/src/ni_splines.c`, 11 lines (larmorx-interp)
  - changed: `scipy/ndimage/src/ni_support.c`, 2 lines (larmorx-interp)

### numpy: 2.3.5 → 2.5.4

- 12 newer release(s): 2.5.4, 2.5.3, 2.5.2, 2.5.1, 2.5.0, 2.4.6, 2.4.5, 2.4.4, 2.4.3, 2.4.2, 2.4.1, 2.4.0; compared: 2.3.5 against 2.5.4 (cumulative).
- 3 named files: 0 unchanged, 3 changed, 0 removed.
  - changed: `numpy/_core/src/common/cblasfuncs.c`, 5 lines (larmorx-transform)
  - changed: `numpy/_core/src/umath/matmul.c.src`, 23 lines (larmorx-transform)
  - changed: `numpy/linalg/umath_linalg.cpp`, 251 lines (larmorx-transform)

### fMRIPrep: 21a490fb89ac → 25.2.7

- a reference for the final comparisons with fMRIPrep, not a version larmorx follows.
- not compared: 21a490fb89ac is not a release; the newest release is 25.2.7.

### FSL: 6.0.7.17 → 6.0.7.23

- FSL's official build, installed beside the old one (~/fsl-<version>).
- 6 newer release(s): 6.0.7.23, 6.0.7.22, 6.0.7.21, 6.0.7.20, 6.0.7.19, 6.0.7.18; compared: 6.0.7.17 against 6.0.7.23 (cumulative).
- packages of the replicated code that changed version: fsl-avwutils 2209.3 → 2209.6, fsl-bet2 2111.8 → 2111.9, fsl-miscmaths 2203.2 → 2412.6, fsl-newimage 2203.12 → 2601.0, fsl-newnifti 4.1.0 → 5.0.0, fsl-utils 2203.5 → 2412.6, fsl-znzlib 2111.0 → 2511.3, fslpy 3.21.1 → 3.27.0.
- 57 named files: 41 unchanged, 15 changed, 1 removed.
  - changed: `fsl-bet2/bet2.cpp` (larmorx-nc)
  - changed: `fsl-miscmaths/kernel.h` (larmorx-nc)
  - changed: `fsl-miscmaths/miscmaths.cc` (larmorx-nc)
  - changed: `fsl-miscmaths/miscmaths.h` (larmorx-nc)
  - changed: `fsl-miscmaths/splinterpolator.h` (larmorx-nc)
  - changed: `fsl-newimage/generalio.cc` (larmorx-nc)
  - changed: `fsl-newimage/newimage.cc` (larmorx-nc)
  - changed: `fsl-newimage/newimage.h` (larmorx-nc)
  - changed: `fsl-newimage/newimagefns.cc` (larmorx-nc)
  - changed: `fsl-newimage/newimagefns.h` (larmorx-nc)
  - changed: `fsl-newimage/newimageio.h` (larmorx-nc)
  - changed: `fsl-newnifti/NewNifti.cc` (larmorx-nc)
  - changed: `fsl-newnifti/NewNifti.h` (larmorx-nc)
  - changed: `fsl-newnifti/legacyFunctions.h` (larmorx-nc)
  - changed: `fsl-znzlib/znzlib.h` (larmorx-nc)
  - removed: `fsl-znzlib/znzlib.c` (larmorx-nc)
## 12. `docs/validation/golden.md`

The GPL replica's golden checksums were re-recorded from `8218894` (commit `dc84558`): two
3dTshift cases changed, every other checksum is the same. Line 18 (the `larmorx-gpl` column):
"40 cases, 90 outputs" (it said 25 and 50, the 3dTshift count of 2026-10-12; 41 and 92 before
this part). Line 56 (`larmorx-gpl afni 3dTshift`), replace "| 25 |" by "| 24 |" and the sentence
"AFNI's defects corrected (the default) and, in 7 `*.reproduce` cases, reproduced: a negative
scale factor (AFNI-01), float32 with a slope and an intercept in an oblique sform in metres with
header slice code 2 (AFNI-02, -03, -07), a copy keeping slice times within 3 ms of 0 (AFNI-08),
`-no_detrend` (AFNI-05), an sform ..." by "AFNI's defects corrected (the default) and, in 6
`*.reproduce` cases, reproduced: a negative scale factor (AFNI-01), float32 with a slope and an
intercept in an oblique sform in metres with header slice code 2 (AFNI-02, -07; the code-2
timing as AFNI 26.3.05 reads it, AFNI-03 being fixed upstream), a copy keeping slice times within
3 ms of 0 (AFNI-08), an sform ..." (the `-no_detrend` reproduce case was removed on 2026-10-29:
AFNI 26.3.05 fixed AFNI-05, so both modes take one path, which `3dTshift.heptic.uint8.no_detrend`
covers).

## 13. `docs/decision-log.md` (append)

```
## 2026-10-29: AFNI 26.3.05, the version update's first part (orchestrator, by the rules)
- **The replicas follow AFNI 26.3.05.** Between 25.2.09 and 26.3.05 the code 3dTshift, 3dAutomask
  and 3dUnifize run changed in two places, both fixes of defects larmorx already corrected the
  same way (AFNI-03, header slice code 2; AFNI-05, `-no_detrend`): `reproduce` now gives
  26.3.05's result, which is the default's; no notice in either mode (the brief's rule for a
  registered defect the new release fixes). 25.2.09's behaviour stays available in earlier
  larmorx releases (FAIR4RS, decision of 2026-10-27).
- **Two oracle builds, one choice:** the harness runs AFNI 26.3.05 for the replicas, 25.2.09 for
  the clean-room originals until a clean-room agent aligns them (`--afni-version`,
  `LARMORX_AFNI_VERSION`); every replica case also runs the other build and its record names
  AFNI's own step, with the reason.
- **Waiting for the user:** AFNI 26.3.05 relicenses MCW's code under CC BY 4.0, so the AFNI
  replicas could move into the Apache package (`docs/findings/afni-relicensing-26.md`); nothing
  moved.
```

## 14. Gates (from `a562e78`, the branch's head)

`gates-rust.sh all`: root, `crates-gpl`, `crates-nc` OK (fmt, clippy `-D warnings`, tests);
`gates-python.sh --no-build`: 628 passed, ruff, SPDX OK; `pytest tests/parity`: 2564 passed;
`tables --check`: exit 0. Logs: `<workspace>/oracles/logs/version-afni-gates.log`,
`version-afni-gates2.log`. (A note for the orchestration scripts: in a worktree, `env.sh`'s default
`LARMORX_WORKSPACE`, the repository's parent, is `.claude/worktrees`; the first run of the root
and Python gates failed on it, a missing venv, and passed with `LARMORX_WORKSPACE` set.)
