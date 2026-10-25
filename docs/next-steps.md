# Next steps

The ordered plan from here: what runs next, what is waiting for the user, and the follow-ups.
Keep it current. The present state is in CLAUDE.md ("Where things are"); the scopes of finished
parts, as they were planned, are in [status-log.md](status-log.md).

**Ground rules.** Port every tool first, then compare with fMRIPrep at the end (user,
2026-10-16): each port is validated against its own original program (parity, golden). **Full
completeness** for every tool: every option, message and failure case, with D13 switches (user,
2026-10-12); brief each agent for it from the start, so a tool takes one pass. **One agent at a
time, in bounded parts with a stop after each**, for the user's usage check (user, 2026-10-10 and
2026-10-17). Every tool's contract includes its figures (CLAUDE.md rule 7). Each merge goes
through CLAUDE.md's checklist. While an agent runs, the orchestrator prepares the next task but
starts no other agent.

## Done since 2026-10-12

| Part | Merged | What | Left over |
|---|---|---|---|
| A4.1 | `c22d336` (10-16) | ITK's HDF5 transform files in pure Rust (`larmorx-hdf5`); `-t x.h5`; h5py dropped | data stored in other files (refused); the writer's fractal-heap indirect blocks (38+ transforms written compactly) |
| A4.2 | `c6fdd61` (10-17) | `antsRegistration`'s linear stages; `antsApplyTransforms`' transform outputs; HDF5 golden fixtures | MINC transform files (the oracle cannot read them either) |
| A4.3 | `084b61d` (10-17) | `antsAI`, `antsAffineInitializer` | `antsAI -b` (blob alignment): its own task, below |
| A4.4 | `f19ffb7` (10-18) | `antsRegistration`'s point-set metrics; ANTS-54 and ITK-21 corrected by default; correctly rounded `pow` | five decisions (below) |
| A5.1 | `7f2a61e` (10-20) | `antsRegistration`'s SyN stages; displacement fields as initial transforms; parity `--jobs` | the other deformable stages (A5.2, A5.3) |
| A5.2 | `a5-part2-bspline` (10-21) | `antsRegistration`'s BSplineSyN, BSpline, GaussianDisplacementField and BSplineDisplacementField stages; the Python API's `mesh_size`, `total_mesh_size`, `spline_order` | `antsApplyTransforms` with `BSplineTransform` files; ANTS-78 and ITK-35 (below); A5.3 |
| Figures 1 | `worktree-agent-a23fe03519502c885` (10-21) | the figure framework (`larmorx_validation.figures`, `docs/figures.md`), 37 figures for every tool ported so far, a page per tool on the site, the landing page's phone layout and Greek glyphs | benchmarks for the pages that say pending; smaller figure files |
| A4.5 | `worktree-agent-aa903a90387159f4f` (10-23) | `antsAI -b` (blob alignment) to full completeness: ANTs' blob detector, patch correlations, pairing, landmark fits (LINPACK's QR, Eigen's `SelfAdjointEigenSolver`), `lx.ants.blob_alignment`; ANTS-110 to ANTS-116 and ITK-60; ANTS-106 corrected in Atropos; ANTS-72 shown to cause the ITK-35 references' run-to-run variation (the record now gives their spread); both evidence pages | ANTS-116 (decide); a benchmark of `-b` |
| AFNI masks | `worktree-agent-af5a27ad72e66b922` (10-23) | 3dAutomask and 3dUnifize: GPL replicas (both depend on MCW routines), bit-identical to AFNI in every compared case; specs for the clean-room originals; two evidence pages; AFNI-40 to AFNI-49 | the clean-room originals; AFNI-41, -42, -44, -48 (below) |
| AFNI masks original | `worktree-agent-a4282c736dbd9fe7c` (10-23) | the clean-room originals of 3dAutomask and 3dUnifize (from the specs; built for speed: a sliding window of ranked values for 3dUnifize's ball, union-find clusters, breadth-first exterior clip and depth, parallel passes), `implementation=` for both, AFNI-41 and AFNI-48 refused by default; CORE-MATH's `tanhf` and `cbrtf`; 8 golden cases; both evidence pages with the original | a benchmark report (on a quiet machine); the replica records to regenerate under `-replica`; the shared CORE-MATH preload to rebuild |
| A5.3 | `worktree-agent-aaab33aced8cdf16d` (10-23) | `antsRegistration`'s Exponential, BSplineExponential and time-varying velocity-field stages (every stage ANTs accepts); the corrections ANTS-67/68/70/78 and ITK-24/29/35 by default (and ITK-42); `antsApplyTransforms` with `BSplineTransform` files; both evidence pages | benchmark of the deformable stages; ITK-38 and ITK-40's fix pass on the full tier; MINC transform files |
| A6 | `worktree-agent-a6259dedbd25d703c` (10-22) | `Atropos` to full completeness (every initialisation, likelihood model, posterior formulation, MRF/ICM, outlier and partial-volume option), its evidence page; ITK-50, ITK-51, ANTS-90 to ANTS-107 | – |

## The order from here

1. **Visual reports, part 2: interactive evidence** (user, 2026-10-21; decision log). Part 2a, a
   pilot (merged and approved by the user, 2026-10-21): the Validation guide (method, terms,
   the verdict taxonomy, a tool-by-implementation summary), the shared interactive components
   (image viewer, time series, charts, case explorer, verdict legend) and the pages of
   N4BiasFieldCorrection and 3dTshift. Part 2b: every other tool, and the cases the summary
   counts "within thresholds, not yet classified" (antsApplyTransforms 28, ImageMath 5, ITK
   transform files 1, head-motion correction 121) given their verdicts; N4's benchmark re-run
   against the current oracle (on a quiet machine). **Part 2b merged 2026-10-21:** every tool's
   page in this form, every case classified. Each later part brings its tool's evidence in the
   same form.
2. **A5 part 3: done** (merged 2026-10-23): every `antsRegistration` stage, the corrections of
   2026-10-21, `antsApplyTransforms` with `BSplineTransform` files.
3. **A6, Atropos: done** (merged 2026-10-22, run in parallel with A5 part 3 at the user's
   request). With A4 to A6, sMRIPrep's brain extraction and MNI normalisation run in larmorx
   (A5 part 3 merged).
4. **`antsAI -b`: done** (2026-10-23): the blob alignment to full completeness; ANTS-110 to
   ANTS-116, ITK-60; ANTS-106 corrected in Atropos; the ANTs run-to-run variation explained (ANTS-72).
5. **AFNI 3dAutomask and 3dUnifize: done, replicas and clean-room originals** (2026-10-23 and 24; the replicas in parallel with `antsAI
   -b`). Both depend on MCW library routines (`docs/licensing.md`), so each has a GPL replica in
   `larmorx-gpl`, bit-identical to AFNI 25.2.09 in every compared case (fMRIPrep's calls on
   every oracle run's BOLD reference, T1w images, BOLD series, at 1, 2 and 6 threads, and
   fMRIPrep's own build of AFNI), and a spec for a clean-room original (`specs/3dAutomask.md`,
   `specs/3dUnifize.md`). **The clean-room originals: done** (2026-10-23): `larmorx afni 3dAutomask` / `3dUnifize` and
   `lx.afni.automask` / `unifize` have `implementation="auto"|"replica"|"original"` as 3dTshift
   (records `afni-automask.md`, `afni-unifize.md`; the replica's moved to `*-replica.md`). Optional later AFNI paths of
   fMRIPrep's libraries: 3dQwarp (sdcflows' alternative PEPOLAR estimator), 3dSkullStrip with
   3dcalc (niworkflows' legacy AFNI skull-stripping): **planned for later** (user,
   2026-10-19). **A later AFNI analysis track (decided by the user, 2026-10-19; not fMRIPrep):**
   3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit, 3dFWHMx/3dClustSim,
   3dAllineate, 3dcalc, 3dTstat, 3dROIstats/3dmaskave, 3dresample, 3dvolreg (low priority).
   Check each program's call graph routine by routine (2026-10-23: the programs' own files can
   be NIH while the library routines they rely on are MCW).
6. **The evidence infrastructure** (decision log 2026-10-23): in progress.
   **The oracle-output cache: done** (2026-10-25; [validation/oracle-cache.md](validation/oracle-cache.md)):
   every run of an original program computed once, keyed by the oracle build, inputs, arguments and
   settings; the runs the original varies always run; the golden recording reuses the parity smoke
   result. Measured on a busy machine (other agents ran beside it; load average 8 to 31): the smoke
   tier's records took 2021 s without the cache and 694 s warm, `pytest tests/parity` 94 minutes
   and 27, `ants-registration`'s standard record 2.0 hours and 49 minutes (509 of its 657 original
   runs from the cache); details and the proof in [oracle-cache.md](validation/oracle-cache.md). Then:
   **the data panel** (in progress: 14 OpenNeuro datasets approved by the user on 2026-10-24, one
   subject each, 2.35 GB, 3T and 7T and 1.5T, three vendors, multiband and single-band, children
   and older adults, a tumour and a stroke with its lesion mask, low quality and motion; in
   `larmorx-testdata` with its licences and hash-verified downloads; fMRIPrep 25.2.6 oracle runs
   for the nine new ones; the site's evidence made to scale without losing anything, with credits
   and a Data panel page); **differential random
   testing** (a shared harness that generates random valid command lines and inputs per tool and
   compares with the original, overnight on this machine, never in CI; then every finished
   tool); **generated tables** (the README's tool table, the validation index and the site's
   status table from one description per tool and its records). Then every finished tool is
   re-validated on the panel, with its evidence page updated.
7. **FSL:** the mcflirt replica, BET, FAST, FLIRT with BBR, topup: a replica in `larmorx-nc` (FSL
   licence, with FSL's source as its terms require) and a clean-room original each. In progress
   (started 2026-10-24, beside item 6 at the user's request): the mcflirt replica (with
   `crates-nc/` and FSL's shared core) and the BET replica with its spec are built, to be merged
   one after the other; then BET's clean-room original, FLIRT with BBR, FAST, topup.
8. **Field maps, FreeSurfer, surfaces, confounds:** sdcflows' field-map estimation (Apache Python:
   the B-spline fit, PEPOLAR and phase-difference preparation) and fMRIPrep's own Python steps;
   FreeSurfer (`larmorx-freesurfer`: `mri_coreg`, `lta_convert`, then recon-all; its Talairach step
   as a clean-room original with its atlas fetched on first use, its data packaged apart from the
   wheels; `docs/licensing.md`); pretrained models as Rust inference with the original weights
   (decision log 2026-10-19); Connectome Workbench- and MSM-compatible surface tools; the
   confounds (CompCor, DVARS, framewise displacement, ...).
9. **larmorprepx**, then the end-to-end comparison with fMRIPrep.
10. **Releases on PyPI** (below).

## Follow-ups, when they fit

- **The oracle cache, further:** `pipeline-stages` still keeps its own runs (`oracles/compare`)
  and could use the cache; mcflirt's variability band (`parity/mri_hmc_band.py`) and the recorded
  mcflirt runs could go through it; `oracle-cache confirm --jobs 6` overnight serves the thread
  bands of N4 and the AFNI OpenMP runs from their third agreeing run; `oracle-cache verify
  --sample 50` at each merge or weekly; an archive policy when the cache grows (it is
  regenerable; archive, do not delete).
- **Atropos' clock-seeded cases** (`ants-atropos`, `expect="random"`): the two ANTs runs that
  measure ANTs' own spread can start within the same second and then get the same seed from
  ITK's clock, so the record's "ANTs run 1 vs run 2" is sometimes "identical" (seen 2026-10-24 in
  runs with and without the cache; never cached). Wait a second between the two runs, or check
  that their outputs differ.
- **Benchmarks of the oracle cache on a quiet machine** (the measurements of 2026-10-24 ran beside
  other agents, load average 8 to 31).

- **Benchmark reports** on a quiet machine: N4 against the new oracle; `antsApplyTransforms`
  after `--float`; the BOLD resampler; SyN (one informal measurement: 96³ CC, ANTs 122 s at one
  thread, larmorx 24 to 41 s, bit-identical); the B-spline and displacement-field stages (4D
  `BSpline` on a small phantom: ANTs 461 s, larmorx 23 s at one thread, identical); HDF5 reading
  against h5py and ANTs; the velocity-field stages; informally, sMRIPrep's registration with
  `BSpline[0.1,26]` (about a million parameters at the last level): ANTs at 4 threads unfinished
  after 3 h 50 min, larmorx 38 s at one thread.
- **Figures:** every new part brings its tool's figures (`docs/figures.md`); the site's pages
  for antsRegistration, antsAI and the HDF5 files say their benchmark is pending until the
  reports above exist. The figures take about 12 MB of the site's repository (four SVGs each,
  text as outlines); embedded font subsets per SVG would make them smaller.
- **Head-motion correction's open cases** (91 of the record's 134, shown as "open" on its
  evidence page): mcflirt's own variability is measured for the real-data cases only; measuring
  its band per option and synthetic case would classify them.
- **Records made with the older CORE-MATH preload:** `ants-image-math`, `ants-n4`, `ants-denoise`,
  `ants-resample-image-by-spacing`, `ants-ai` (and its full tier) were regenerated on 2026-10-24 with the current
  preload; any other record older than 2026-10-18 still to re-run.
- **Benchmark reports of 3dAutomask and 3dUnifize** (AFNI, the replica, the original; a quiet
  machine). Informal timings of 2026-10-23 are in the originals' part report.
- **The decisions of 2026-10-23, implemented** (small-fixes part, 2026-10-24): ANTS-17, -22,
  -23, -32, -44, -116 corrected by default (rule A), ANTS-40 measured and corrected (D, then A),
  ITK-06 measured and kept (D, then C), AFNI-41 and AFNI-48 refused by default in the GPL replicas
  (rule B). Each with its known-result parity case, its fix pass, re-recorded golden checksums
  and its evidence on the site.
- **In-place writing in 3dTshift** (decided 2026-10-21): an existing writable output in a
  directory that cannot be written is replaced in place, as AFNI does. The replica directly;
  the clean-room original from a spec amendment written from black-box runs (the implementer
  reads only the spec).
- **PIPE-16** (larmorprepx, decided 2026-10-21): the corrected field-map reference keeps N4's
  float32 result; `reproduce` gives fMRIPrep's int16 re-quantisation.
- `larmorx-interp`'s B-spline prefilter still calls the `libm` crate's `pow`: switch to
  `larmorx_core::math::pow` and re-run `antsApplyTransforms`' B-spline cases.
- A4's linear stages run in the global thread pool instead of `n_threads` (results unaffected).
- Coverage: a copy of ds005752 with `IntendedFor` added, so the direct-`fieldmap` path is covered
  (fMRIPrep ignored its field map); ds001600's four field-map N4s.
- Temporary directories `/tmp/lx-ants-inputs-*` and `/tmp/lx-reg-*` left by earlier agents
  (temporary artefacts; may be cleaned).

## Before the first release on PyPI

Checked 2026-10-13: the binaries need no other library than the system's own (on Linux:
`libc`, `libgcc_s`, and for `larmorx-gpl` glibc's `libm`, `libpthread`, `libdl`, `librt`).
Still to do:
- **Windows' C runtime:** nothing sets `+crt-static`, so the `.exe` programs (`larmorx-gpl`,
  the standalone `larmorx`) need `vcruntime140.dll`. Python's own copy serves the extension,
  but a separate program may not find one on a clean Windows. Link the C runtime statically
  for the Windows MSVC targets (`.cargo/config.toml`, both workspaces). CI cannot show the
  problem: GitHub's runners have the runtime installed.
- **Size:** `strip`, LTO and `codegen-units = 1` in the release profile (stripping alone takes
  the extension from 24 to 15 MB).
- **Python dependencies:** h5py dropped (A4 part 1, 2026-10-16: numpy is the only one); check
  that every `[prep]` dependency has wheels for all six platforms, Windows arm64 included. Linux
  wheels are manylinux_2_28 (glibc 2.28 or newer); decide whether to add musllinux wheels
  (Alpine).
- **The `[all]` extra** (decided 2026-10-16): `larmorx-gpl`, `larmorx-nc` and `larmorx-freesurfer`,
  added to `pyproject.toml` once those packages exist (an extra naming a missing package breaks
  `pip install`).
- **Publishing:** a release workflow (on a version tag: the six wheels and the sdist of each
  package, the golden tests, upload by PyPI Trusted Publishing; standalone binaries to GitHub
  Releases); reserve the names `larmorx`, `larmorx-gpl`, `larmorx-nc`, `larmorx-freesurfer`,
  `larmorprepx` (all free on 2026-10-13; needs the user's PyPI account); the licence checks for
  publishing `larmorx-nc` (FSL) and `larmorx-freesurfer` on PyPI; the first release's scope
  (D10) and version numbering.

## Deferred (decide later)

- **Byte-identical deflate in HDF5 files** (decision log, 2026-10-16): larmorx's `.h5` files hold
  the same values as ITK's, but their compressed chunks differ in bytes; porting zlib-ng's
  deflate (one to two days) would make them match the oracle's build only. Skipped for now.

## Waiting for the user

Decided 2026-10-23 (decision log): the four rules for questionable behaviours (A correct, B error, C keep and document, D measure first), applied without asking from now on; the 24 open questions by those rules.

- **Upstream reports:** PIPE-02 and PIPE-03 are drafts in `docs/upstream-reports/`, now with
  PIPE-03's real-data numbers, ready to file (from the user's GitHub account).

Decided 2026-10-22 (decision log): ANTS-106 corrected by default (follow-up below); ANTS-30's
command line approved. Decided 2026-10-21: ANTS-78, ITK-35, ANTS-67, ANTS-68, ANTS-70, ITK-24, ITK-29
corrected by default (with A5 part 3); in-place writing as AFNI does; `LARMORX_GPL_BIN` kept;
PIPE-16 keeps N4's float32 result.
