# Brief: Source real test data catalog

As given to agent `a1c6d77a0ffc5fb58` on 2026-10-09, verbatim (paths are this machine's).

You are building the real-data part of a test-data repository for **larmorx**, a project that re-implements neuroimaging tools (NIfTI I/O first; later ANTs registration, AFNI tools, an fMRIPrep-like pipeline) in Rust with Python wrappers. The data will be used for unit tests, parity checks against nibabel/ITK/ANTs/fMRIPrep, and performance benchmarks. Quality bar: comprehensive, representative of real-world neuroimaging data, very well organised, every file with verified provenance, licence, SHA-256 and size.

## Repository and tooling (already exists)
- Repo: `/home/karelo/VScode_projects/super_fmriprep/larmorx-testdata` (a git repo). READ `README.md` first: it defines the catalog format (TOML per collection), paths/prefixes, tiers and tag vocabulary. Follow it exactly.
- Package `larmorx_testdata` is installed (editable) in the venv `/home/karelo/VScode_projects/super_fmriprep/.venv` (Python 3.12, has nibabel, numpy). Use `/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python -m larmorx_testdata {list,fetch,verify,index,path}` to validate your catalog files: `fetch --tier full` downloads into the cache and verifies hashes; `index --tier full` writes `catalog/index.json`.
- Do NOT create or modify anything under `data/synthetic/` or `generators/` (someone else is writing the synthetic generator), and do not modify `src/`. If you find a bug in `src/`, describe it in your report instead of fixing it.
- Do NOT commit. Leave your changes as uncommitted files; they will be reviewed and committed afterwards.
- Never read anything under `/home/karelo/VScode_projects/super_fmriprep/reference_src/` (clean-room restriction). Do not download FSL software or FSL-licensed data.

## What to produce
One `catalog/<collection>.toml` per collection and one `docs/sources/<collection>.md` per collection (origin, exact version/snapshot/commit, licence with evidence — e.g. the dataset_description.json `License` field or the LICENSE file —, citation, and for each file *why it is in the set*: what property or real-world situation it represents). Also write `docs/sources/README.md`: a table of all collections (id, licence, tiers, number of files, total size) and the selection rationale.

Collections to build (adjust if something is unavailable, but explain substitutions):
1. `nibabel-testdata` (MIT): nibabel's own test files for NIfTI-1, NIfTI-2, Analyze (.hdr/.img pairs, including gz), and MGH/MGZ, from `nibabel/tests/data/` at the **5.3.2 release tag** commit (the nibabel version fMRIPrep uses). Resolve the tag's commit with `git ls-remote --tags https://github.com/nipy/nibabel` and use `https://raw.githubusercontent.com/nipy/nibabel/<commit>/nibabel/tests/data/<file>` URLs. Paths: `nibabel/tests/data/<file>`. Include all NIfTI/Analyze/MGH files (they are small; tier smoke unless large). Tag mgh files `mgh`, Analyze pairs `analyze`.
2. OpenNeuro raw BIDS data (CC0 — verify each dataset's licence). Use **snapshot-pinned** URLs (OpenNeuro's GraphQL API at https://openneuro.org/crn/graphql gives per-snapshot file URLs; or `https://s3.amazonaws.com/openneuro.org/<ds>/<path>?versionId=<id>`), record the snapshot tag in `version`. Keep BIDS paths: `openneuro/<ds>/sub-XX/...`, and include `dataset_description.json`, top-level task JSONs and the sidecar JSONs of every image you include. Collections (one per dataset, id `openneuro-<ds>`):
   - ds000005 (fMRIPrep's main test dataset): sub-01 T1w + both/all BOLD runs of sub-01.
   - a multi-echo dataset (fMRIPrep CI uses ds000210): one subject's multi-echo BOLD run(s) + anat.
   - fieldmaps: ds001600 (has phasediff, two-phase, direct fieldmap and PEPOLAR for one subject) — one subject, all fmap types, the BOLD they apply to, and anat. Also one PEPOLAR-only dataset if ds001600 lacks a clean PEPOLAR case.
   - multiband BOLD with an `sbref` image.
   - a 7T acquisition (anat and BOLD).
   - an oblique acquisition (check obliquity with nibabel: `nib.affines.obliquity(img.affine)` > ~0.05 rad) and, if possible, a dataset whose images have qform/sform codes or orientations that differ from the usual (e.g. LAS/RAS mix, sform_code 0, scaled int16 data with scl_slope != 1).
   - one pediatric T1w and one older-adult T1w (different head size/contrast).
3. `openneuro-derivatives-*`: published fMRIPrep outputs for ds000005 (the OpenNeuroDerivatives project hosts them, e.g. on GitHub `OpenNeuroDerivatives/ds000005-fmriprep` / S3 `openneuro-derivatives`). Include for sub-01: preprocessed T1w, brain mask, dseg, probseg, boldref, a BOLD in native/T1w space, confounds TSV, and the transform files (`*_xfm.h5`, `*_xfm.txt`, tag `transform`). Note the fMRIPrep version that produced them.
4. TemplateFlow (`templateflow-*`, licence per template from its `template_description.json`): `tpl-MNI152NLin2009cAsym` res-01 and res-02 T1w, res-01/02 `desc-brain_mask`, `label-brain_probseg`, res-02 `desc-fMRIPrep_boldref`; `tpl-OASIS30ANTs` res-01 T1w, `desc-BrainCerebellumExtraction_mask`, `label-brain_probseg`, `desc-brain_mask`; `tpl-MNI152NLin6Asym` res-02 T1w. Use `https://templateflow.s3.amazonaws.com/tpl-<name>/<file>` URLs and record the template repo commit (github.com/templateflow/tpl-<name>) as version if you can determine it. Paths: `templateflow/tpl-<name>/<file>`. Include `template_description.json` too.
5. Benchmark files (tier `full`, tag `benchmark`): at least (a) a long multiband BOLD run (several hundred MB compressed, ≥ 500 volumes), (b) a high-resolution anatomical (≤ 0.8 mm isotropic), (c) a typical single-band BOLD (~64×64×36×200+). They can come from the datasets above if suitable.

## Tiers and budgets (strict)
- `smoke` total ≤ 50 MB: nibabel test files, one real T1w, one small real BOLD (pick a short/small run), the small JSON sidecars, one template at res-02, a few masks.
- `standard` total ≤ 2 GB (including smoke): the rest of the representative set.
- `full`: benchmark files and anything large.
Report the per-tier totals.

## Validation before you finish
- `python -m larmorx_testdata fetch --tier full --jobs 6` must succeed (it verifies every hash).
- `python -m larmorx_testdata index --tier full` must succeed; check `catalog/index.json` looks sensible.
- `python -m larmorx_testdata list --tier smoke` and `--tier standard`: confirm budgets.
- Every collection has a licence that permits redistribution and use in tests; if a licence is unclear, exclude the data and say so.

## Report back (concise)
A table of collections (files, size per tier, licence), notable properties covered (NIfTI versions, byte orders, data types, scaling, qform/sform code combinations, obliquity, orientations, 4D/5D, compressed/uncompressed, pairs), anything you could not find, and any problems with the tooling.
