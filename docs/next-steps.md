# Next steps

The ordered plan from here: what runs next, what is waiting for the user, and the follow-ups.
Keep it current. The present state is in CLAUDE.md ("Where things are"); the scopes of finished
parts, as they were planned, are in [status-log.md](status-log.md).

**Ground rules.** Port every tool first, then compare with fMRIPrep at the end (user,
2026-10-16): each port is validated against its own original program (parity, golden). **Full
completeness** for every tool: every option, message and failure case, with D13 switches (user,
2026-10-12); brief each agent for it from the start, so a tool takes one pass. **One agent at a
time, in bounded parts with a stop after each**, for the user's usage check (user, 2026-10-10 and
2026-10-17). Every tool's contract includes its figures (CLAUDE.md rule 7). **Testing depth**
(user, 2026-10-28; decision log): each part tests smartly (every code path once, fMRIPrep's real
calls on about three oracle runs, the defects' edge cases, the standard and smoke tiers) and
publishes its tool's page with figures and metrics from those tests; the very extensive testing
waits for **the final validation phase** (below), once every tool is ported. Each merge goes
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
| Random testing, tables | `worktree-agent-ab7de196daece3c4e` (10-25) | `difftest` (21 tool descriptors, shrinking, permanent cases, triage, nightly script, first night 34 880 cases); fixes it found in the ports (the sform/qform choice in vnl's float SVD, allocation refusals, singular directions, unregistered composite names, a float transform's point indexed in float, vnl's SVD warning in Mattes, ImageFileWriter's origin and empty images, Atropos on an empty mask, CC's Jacobian and ITK-35; and from the first night: ResampleImage's B-spline on a 2-voxel axis, DenoiseImage's Rician smoothing kernel and dimension without -d, ITK's shortest numbers at a tie, four transform classes in antsApplyTransforms, a float versor's cosine, the empty sample set's exception, antsAI's warning order, the median's zero, the aliased derivatives of sampled metrics (and in SyN), a later level's zero Mattes bin size, a field's class under a .mat name, SyN's metric sum in float, Atropos' model warnings, the NIfTI writer's non-orthogonal warning, the 2D B-spline fit's memory and time, memory guards (N4, the resamplers, 3dUnifize -EPI)); ANTS-160 to ANTS-163, ITK-95 to ITK-100, AFNI-75; the memory box and the heavy queue; memory no more than ANTs' for the image programs (`tests/parity/test_memory_peak.py`); `tables` (README, validation index, site status); the site's "Random testing" panels | "Random testing's open items" and "Memory against the originals" (follow-ups) |
| BET | `worktree-agent-a38ebb82788afa285` (10-26) | the BET replica (`larmorx-nc fsl bet`, `bet2`, `betsurf`; `lx.fsl.bet`, `lx.fsl.betsurf`), bit-identical to FSL 6.0.7.17 in every compared case (129 of 129, 364 files); its FSL libraries joined to mcflirt's in `larmorx-nc-core`; FSL-40 to FSL-58; `specs/bet.md`; its random-testing descriptor, memory cases and evidence page; FSL-22's `reproduce` exact | BET's clean-room original; `bet -S`, `-B`, `-A`, `-A2` (FLIRT, FAST); "BET, further" (follow-ups) |

## The order from here

**Now (2026-10-28):** the move is done (M, below); the ANTs tools' data panel (part a) and the
fourteen whole template registrations are done (14 of 14 identical with correctly rounded maths).
The order from here, by the user's decision on testing depth (2026-10-28, decision log), one agent
at a time:

1. **antsAI and antsAffineInitializer: memory and speed** (a small part: their metric's images in
   float, the initializer's speed; decision log 2026-10-27).
2. **The clean-room fixes** found behaviourally (the thread probe, the D13 corrections, 3dAutomask's
   obliquity-message number), by an agent that has never read the source.
3. **The version update** (user 2026-10-27, decision log; the upstream watch with it): AFNI to its
   newest release (26.3.05 today), FSL to its newest 6.0.7.x (6.0.7.23 today), SciPy, NumPy and
   OpenBLAS checked; each tool aligned with its newest original, with the per-part testing.
4. **The ports:** BET's clean-room original (item 7), the clean-room additions mcflirt FSL-20/21
   and 3dUnifize AFNI-75 (specs first), FLIRT with BBR, FAST, topup (item 7); field maps,
   FreeSurfer, surfaces, confounds (item 8); larmorprepx (item 9). Each with its site page from
   its smart tests.
5. **The final validation phase** (after every tool is ported): the data panel for every tool
   (part b: AFNI, FSL and I/O; the new tools), full tiers, heavy and nightly random testing,
   thread-band studies, the extensive evidence on each tool's page (added to what it shows),
   speed and memory studies beyond the rule's check, independent reviews, the agents' scripts
   triage (follow-ups), then the comparisons with fMRIPrep (item 9). Machine-only runs that need
   no agent (the heavy random-testing cases) may run before, in the background.

The items below keep their history.

- **M. The move to the new main machine** (user, 2026-10-25; CLAUDE.md "Machines"; **the copy and
  the proof done, 2026-10-28**: neither larmorx nor any original computes differently there, numpy's
  OpenBLAS kernel aside, now pinned for the validation: [machine-move-proof.md](findings/machine-move-proof.md);
  the old laptop cleaned to a helper the same day (phase 4, 204 GB freed); the steps, the user's and the orchestrator's:
  [machine-move.md](machine-move.md), scripts in `scripts/orchestration/move/`), one bounded
  part: the whole workspace copied over SSH to the new laptop (Ubuntu 22.04, 32 threads, 124 GiB)
  with its repositories as they are (history, worktrees, unpushed commits), the oracle builds and
  their outputs (the same bits the records were made with, checked by SHA-256 against
  `MOVE-MANIFEST.json`), `~/fsl`, Rust and the test data; only the build caches rebuilt; glibc the
  same point release (2.35); then the proof: every record's standard tier,
  `pytest tests/parity` and the golden suite re-run there and compared case by case with the
  committed records (`compare_records.py`), and a sample of the oracle cache verified. Only then
  does new work run there. Then the helpers: key-based SSH to this laptop (benchmarks, a second
  Linux reference), the Windows laptop and, for the 25 heavy cases and later FreeSurfer batches,
  the lab's server (originals in an Ubuntu 22.04 container: its glibc is 2.39).
- **Meanwhile, here, and then** (until 2026-10-28; the order above replaces it): every finished
  tool re-validated on the data panel (item 6; **the ANTs tools: done**, part a, 2026-10-27; the
  fourteen whole template registrations: done, 2026-10-28, 14 of 14 identical with correctly
  rounded maths; part b, AFNI, FSL and I/O, moved to the final validation phase).

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
   **the data panel: done** (2026-10-25; larmorx-testdata `docs/panel.md`: fourteen subjects
   approved by the user on 2026-10-24, 2.35 GB, every criterion covered, defacing checked; the
   site's evidence scales to it without losing anything: slices views for new panel cases, the
   blob branch, the size guard, credits; the site's Data panel page; 3dAutomask's first panel
   case); its nine new fMRIPrep oracle runs done (all exit 0); **differential random testing:
   done** (2026-10-25; [validation/difftest.md](validation/difftest.md)): one descriptor per
   finished tool (21), seeded plain-arithmetic inputs, the parity runners as judges, shrinking,
   permanent cases, `triage.toml`, the nightly script, every night in a memory box (never in CI);
   its first night (6.2 h): 34 880 cases, 31790 agree, 3 explained, 1350 notice, 394 undefined,
   1227 differ, 76 memory, 38 timeout; 58 groups of differences: 34 fixed in larmorx, 6 defects
   of the originals in the register, 17 open for the clean-room originals' agents, 1 the
   harness's own; 76 heavy cases queued (40 run here: 26 agree, 14 notices; 11 beyond any
   machine; 25 for a larger machine); the mcflirt replica's 107 differences of that night are all
   bit-identical with its follow-up's fixes (the `atanf` constant, e687cd1; FSL-08's notice of the
   `-meanvol` name, d196995; recounted with `validation/investigations/difftest/mcflirt_pair.py`);
   **generated tables: done** (2026-10-25): `validation/tools.toml` +
   the records → the README's tool table, the validation index and the site's status table
   (`python -m larmorx_validation tables --check`); and, at the user's request the same day,
   **memory no more than the original's** for the ANTs image programs (13 calls in
   `tests/parity/test_memory_peak.py`). Then every finished tool is
   re-validated on the panel, with its evidence page updated. Each tool: its existing examples
   unchanged; the panel's cases added beside them, two or three key datasets as whole volumes,
   the rest as slices (`docs/figures.md`, Views); a page over its budget gets a raised one with
   its reason, never less content (the user, 2026-10-24). **The ANTs tools: done** (part a, 2026-10-26 and 27):
   panel records for every ANTs suite (`docs/validation/ants-*-panel.md`: ImageMath 374/374,
   ThresholdImage 18/18, MultiplyImages 44/44, SmoothImage 4/4, ResampleImage 4/4,
   ResampleImageBySpacing 3/3 bit-identical; N4 69/69, DenoiseImage 21/21, Atropos 17/17,
   antsApplyTransforms 425/425 bit-identical with CORE-MATH; antsAI 40/40, antsAffineInitializer
   14/14, antsRegistration 71/71 identical to ANTs at one thread), each ANTs page with its panel
   section; n_threads made exact; memory fixes; Atropos' input check. Part b (AFNI, FSL and I/O) and the
   new tools: in the final validation phase (user, 2026-10-28).
7. **FSL:** the mcflirt replica, BET, FAST, FLIRT with BBR, topup: a replica in `larmorx-nc` (FSL
   licence, with FSL's source as its terms require) and a clean-room original each. In progress
   (started 2026-10-24, beside item 6 at the user's request). **The mcflirt replica: done**
   (2026-10-25): `crates-nc/` (`larmorx-nc`, FSL Licence, FSL's source included), the core crate
   `larmorx-nc-core` (NIfTI as FSL does it, `volume<float>`, interpolation, FLIRT's cost
   functions, the optimiser, the transform algebra, Armadillo's rules, glibc's float maths) for
   the next FSL replicas; record [fsl-mcflirt-replica](validation/fsl-mcflirt-replica.md): 173 of
   173 compared cases bit-identical (the data panel's fMRIPrep calls and 61 random-testing findings
   included, after one `atanf` constant was corrected). **BET: the replica done** (built
   2026-10-24 and 25, merged 2026-10-26; `larmorx-nc fsl bet`, `bet2`, `betsurf`; `lx.fsl.bet`,
   `lx.fsl.betsurf`; records [fsl-bet-replica](validation/fsl-bet-replica.md): 115 of 115
   compared cases bit-identical, fMRIPrep's 29 calls included, and
   [fsl-bet-replica-full](validation/fsl-bet-replica-full.md): 129 of 129, 364 of 364 files), its
   FSL libraries joined to mcflirt's in `larmorx-nc-core` (one translation of each); under random
   testing (`fsl-bet`, four trial runs, every difference fixed) and the memory rule (below FSL's in
   every measured call); FSL-22's `reproduce` made exact the same day. Next for BET: the
   clean-room original from `specs/bet.md` (an agent that has never read FSL's source; built for
   speed: the spec's §10), and `bet -S`, `-B`, `-A`, `-A2` once standard_space_roi, FLIRT and FAST
   are replicated. Then FLIRT with BBR, FAST, topup.
8. **Field maps, FreeSurfer, surfaces, confounds:** sdcflows' field-map estimation (Apache Python:
   the B-spline fit, PEPOLAR and phase-difference preparation) and fMRIPrep's own Python steps;
   FreeSurfer 8.2.0, every program complete, a replica (`larmorx-freesurfer`) and a clean-room
   original (main package, built for speed) each (decision log 2026-10-26): the oracle runs first
   (8.2.0 on the panel with every intermediate and command kept; fMRIPrep with recon-all, 7.3.2, as
   the variability band) and one generic step-replay harness; FreeSurfer's shared library once;
   the programs fMRIPrep and sMRIPrep run, in groups that share code; the slow programs' originals
   first; then recon-all's other streams; its Talairach step as a clean-room original with its
   atlas fetched on first use, atlases downloaded on first use (`docs/licensing.md`); pretrained models as Rust inference with the original weights
   (decision log 2026-10-19); Connectome Workbench- and MSM-compatible surface tools; the
   confounds (CompCor, DVARS, framewise displacement, ...).
9. **larmorprepx**, then the end-to-end comparison with fMRIPrep.
10. **Releases on PyPI** (below).

## Follow-ups, when they fit

- **larmorx-testdata:** `ruff format --check` fails on main for three generators
  (`generators/ants_filters_synthetic.py`, `resampling_synthetic.py`, `tshift_synthetic.py`;
  untouched by the panel part); its CI's lint step needs them formatted.
- **Oracle runs in progress, in the parity suites:** `test_every_program_node_is_classified` skips
  a run that has not finished (`larmorx_validation.stages.oracle.run_finished`). Two suites still
  enumerate every run without that check: `parity/afni_phantoms.fmriprep_runs` and
  `parity/itk_hdf5._found_files`; one line each (`if run_finished(...)`).
- **The site: a JavaScript syntax check in `tools/build.py`** (`node --check` on `static/js/*.js`
  when node is present): an apostrophe in a single-quoted string once broke `evidence.js` on every
  page while the build reported 0 problems; only the browser number check caught it.

- **mcflirt's FSL-20 and FSL-21 in the clean-room original:** a spec writer adds them to
  `specs/mcflirt.md` (behaviour only; the replica's record shows mcflirt's black-box outputs),
  then a clean-room implementer adds them to `lx.mri.hmc`'s original.
- **A benchmark report of the mcflirt replica** (informal: fMRIPrep's call on ds000005, the
  replica 20 s at 3 threads and 36 s at one, mcflirt 70 s, on a busy machine).
- **BET, further:** a benchmark report (FSL against the replica; bet2 and betsurf on the data
  panel); `robustfov` (in fsl-bet2, not translated); a row in `upstream.tsv` for Arm's Optimized
  Routines v19.11 (the source of the FSL replicas' float functions and of `pow` and `exp`; not
  cloned).
- **mcflirt's full tier** in an 8 GiB box: run it at `--jobs 1` (or in a larger box); at two jobs
  the panel's ds004958 call (600 volumes) is killed by the box's limit, and alone it passes (601
  of 601 files; BET's handoff, 2026-10-26).

- **Random testing's open items** (2026-10-25, `validation/difftest/triage.toml`):
  - the heavy queue of the first night: batch 1 ran here (2026-10-25, 40 cases: 26 agree, 14
    notices that changed no value, no difference); 11 are beyond any machine (guard cases); 25
    wait for a larger machine (the lab's server): 13 N4 cases too long for this machine (51.5 h
    predicted in all, one at a time; 4 to 58 GiB), 8 predicted beyond its heavy box of 24 GiB and
    48 GiB of swap (7 N4, one ResampleImageBySpacing; 119 to 1,140 GiB), and 4 of the first batch
    to run again with the harness's fixes (ResampleImage #556 and ResampleImageBySpacing #938,
    whose comparison held both outputs of billions of voxels; #1278, whose comparison started
    before a fix; N4 #777, which passed the suite's 30-minute run limit while swapping); each with
    its prediction and command line in the night's `heavy-results.jsonl` (verdict "pending:
    larger machine"; `validation/investigations/difftest/heavy_pending.py` prints them).
  - ITK-01's notice is missing where the exact floor reaches antsRegistration's
    displacement-field lookups (`FieldXfm`): a value changes in its last bit by default with no
    notice (`reproduce` is exact; `ants-registration:20261025:428`).
  - the `reproduce` switches of ANTS-161 (ANTs' NaN image), ITK-95 (ITK's moving-histogram values
    at radius 0, and its notice) and ITK-96 (the part-written file); the notice of ITK-98 in
    antsRegistration's outputs and the N-D resampler.
  - `lx.transforms.read` does not read 4D transforms (`DisplacementFieldTransform_double_4_4`):
    the registration runner reads such files by their HDF5 parameters.
  - `lx.transforms.resample_series` gives one voxel of a near-zero value (−7.8·10⁻¹⁷, a
    cancellation) one float ulp from fMRIPrep's (`resample-series:20261025:462`;
    `validation/investigations/difftest/resample_pair.py`): the order of one sum to find.
  - the timeouts of the first night (38: afni-unifize 1, ants-ai 1, ants-n4 14, ants-registration
    10, ants-resample-image 8, ants-resample-image-by-spacing 3, ants-smooth-image 1): cases over
    their tool's time limit, not yet run to their end.
  - the clean-room originals' differences (3dTshift, 3dAutomask, 3dUnifize, head-motion
    correction), each with a black-box reproducer (`python -m larmorx_validation difftest --replay
    oracles/difftest/2026-10-25/findings/<tool>/<finding>.json`), passed to their agents through
    the orchestrator's brief (clean-room implementers do not read `docs/parts/`).
  - D11: N4's lattice as a sparse lattice with virtual padding (ANTS-162; the same result in
    memory proportional to the data).
  - AFNI-75's guard is in the GPL replica of 3dUnifize only: the clean-room original needs it
    through `specs/3dUnifize.md` (a spec writer), then a clean-room agent.
- **Random testing, more**: the data panel's real crops as inputs; BET in the nightly random
  testing (its descriptor `fsl-bet` is registered; its random-testing panel goes on the site after
  the first night that includes it); nights on a schedule (cron, boxed) once the machine is free
  at night.
- **Memory, the rest** (the rule as refined 2026-10-27: similar, at most 10 % above the original plus
  8 MiB): antsRegistration, antsApplyTransforms, Atropos, N4, DenoiseImage and the image programs
  measured on the panel's largest inputs and kept at or below ANTs
  (`docs/findings/ants-image-programs.md`, "Memory on the data panel's largest inputs");
  **antsAI and antsAffineInitializer still above** (127 and 110 MiB against 92 and 91 on a
  128 × 128 × 96 head; +40 and +156 MiB on the 7 T): their metric holds the images in double, ANTs
  in float; store them in float (the next small part; strict-xfail cases in
  `tests/parity/test_memory_peak.py`); the AFNI programs, mcflirt and `resample_series` remain to
  measure (part b: `larmorx_interp::ndimage`'s spline filter copies the volume on its last axis).
- **The fourteen whole template registrations** of the panel: **done** (2026-10-28, on the new
  machine at `c223e64`, 14 at once in 2.9 hours, about 40 hours of CPU in all): every one
  identical to ANTs with CORE-MATH (exit status, every DIAGNOSTIC line, every file), none to ANTs
  as built (glibc's `exp` and `log`, as in the small versions); record
  [ants-registration-panel-template](validation/ants-registration-panel-template.md). Every
  antsRegistration node of the fourteen runs is now compared at full size. Their evidence on the
  site (the whole registrations beside the small ones) waits for the next site part.
  **antsRegistration's standard record re-made** the same day (at `f15b00a`, 35 minutes): 513 cases,
  399 pass (355 identical as built, 44 with CORE-MATH), 93 rejected by both, 21 expected
  divergences; against the record of 2026-10-23 no status or detail changed, and the 8 cases added
  since (permanent cases from random testing) pass.
- **antsAffineInitializer's speed**: larmorx 277 s against ANTs' 73 s at one thread on a
  128 × 128 × 96 head, 1212 s against 332 s on the 7 T (the D13 notice of ITK-21 runs every local
  search twice; the rest is the metric's speed): the next small part.
- **ImageMath's panel run in a box**: its 600-volume `m` case needs about 8 GiB in the harness
  (both outputs and larmorx in-process): two jobs in a 20 GiB box.
- **ResampleImage's speed** with nearest-neighbour resampling to `unsigned char` (3D, most voxels
  beyond the input): 2.4 times ANTs' time (15.2 s against 6.4 s on 756 million output voxels,
  `ants-resample-image:20261025:556` at half its spacing; the same memory).

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

- **The agents' scripts into the repository** (found 2026-10-27, the move; the rule of
  2026-10-23: every tool that produced a finding or a record is committed under
  `validation/investigations/<topic>/` with a README, or `validation/probes/`): the archive holds
  2,332 distinct scripts the agents wrote (in their scratch directories, archived) that are in no
  repository, against 12 investigation topics and 4 probes committed. Triage, on the new laptop
  (the archive's documents are copied there, `machine-move.md`): for each finding
  (`docs/findings/`) and record (`docs/validation/`), find the scripts that produced it; commit
  those not yet in the repository, each topic with a README (what it shows, how to run it); the
  rest (one-off edits, superseded drafts) stays in the archive. One bounded part, in the final
  validation phase (user, 2026-10-28: testing depth); the briefs' rule keeps new scripts in the
  repository meanwhile.
- **The oracle cache across CPUs** (found 2026-10-27, the move): the key includes the CPU's model
  and the hash of all its flags, so the old laptop's 40 GB of entries are never hits on the new
  AMD machine. After the move's `verify` sample: if the originals give identical outputs on both
  CPUs, narrow the machine part to what glibc's kernel selection reads (the ISA features its
  `ifunc` resolvers test: FMA, AVX2, AVX-512 ...), measured first (rule D), so entries are shared
  between machines whose originals compute alike. **Measured 2026-10-28** (the move's proof,
  [machine-move-proof.md](findings/machine-move-proof.md)): of the old laptop's 4,953 short
  entries, 4,541 identical on the AMD machine, the other 89 replay differently on their own
  machine too, 17 long ones identical; so the originals compute alike on these two CPUs. The
  narrowing must follow what each program's dispatchers read: numpy's OpenBLAS picks other
  kernels on AVX-512 (inverses differ), glibc's `libm` does not.
- **Tests that depend on the machine, from the move's proof**: **done 2026-10-28**
  ([machine-move-proof.md](findings/machine-move-proof.md), "Fixed"; part
  `docs/parts/2026-10-28-machine-tests/`). The originals run under a
  fixed address-space limit (64 GiB, `LARMORX_ADDRESS_SPACE_LIMIT`; the largest real call 9.4 GiB,
  the smallest refusal the records expect 96 GiB; random testing derives it from each case's
  memory budget, so `--heavy` runs under twice its box), so `allocation-refused` and `MD -1` fail
  alike on every machine; BET's temporary folders hold no option text and the cache key keeps
  the text a program reads in a path (`bet2/one-argument`); the replay gaps are reproduced or
  never cached (`oracle-cache verify` on the 89: 45 identical, 44 skipped with their reason). The
  smoke tier passes here in full, twice; the 45 entries that verify identically resolved at the
  merge.
- **Random testing: a run refused by the address-space limit is not an agreement** (found at the
  merge of 2026-10-28): since the limit, a night's case whose original asks for more than 64 GiB
  at once is refused by both programs and counted *agree*, where before it was killed by its
  memory limit and queued as a heavy case. The user's rule (2026-10-25: heavy cases are run, not
  avoided) wants it queued: a refusal under the harness's own limit becomes a *memory* verdict
  (queued, with its prediction), except in the permanent cases that expect the refusal
  (`allocation-refused`, `MD -1`). No night runs before this fix (the nights are not scheduled
  yet). Small; with the next random-testing or harness part.
- **Random testing's heavy comparison holds both outputs** (found 2026-10-28): the heavy case
  `ants-resample-image:20261025:556` (6·10⁹ output voxels) was stopped at the 60 GiB box here
  because the process that runs larmorx and compares also holds ANTs' output (about 41 GiB each);
  compare in blocks (memory-mapped) so the comparison adds little to larmorx's own peak. Until
  then that case runs again in a box of about 110 GiB when the machine is otherwise free.
- **3dAutomask's clean-room original, the panel's cases** (found 2026-10-28): five fMRIPrep calls
  of the panel runs (added after its record) fail on the obliquity message's number (AFNI
  `20.646870`, the original `20.646889`; the masks agree). To the clean-room fixes part, as a
  behaviour-only description.
- **The version update, what changed in the numerics** (the newest releases fetched 2026-10-28
  beside the old ones in the workspace's `tags/`; FSL 6.0.7.23 installed in `~/fsl-6.0.7.23`,
  `~/fsl` still 6.0.7.17; AFNI 26.3.05 built in `oracles/afni-26.3.05`): SciPy 1.15.2 → 1.18.1
  changes `_interpolation.py` (66 lines), `ni_interpolation.c` (56), `ni_splines.c` (9),
  `ni_support.c` (2); NumPy 2.3.5 → 2.5.4 changes `cblasfuncs.c` (5), `matmul.c.src` (23),
  `umath_linalg.cpp` (239); OpenBLAS 0.3.30 → 0.3.34 leaves every kernel larmorx reproduces
  unchanged (the Haswell `dgemm`/`dgemv` kernels, `idamax`, `getf2_k`, `getrs`, `laswp`, the
  `trsm` kernels, `KERNEL.HASWELL`, `KERNEL.ZEN`) and changes `interface/gemm.c` (86),
  `level3.c` (25), `param.h` (322), `KERNEL.SKYLAKEX` (15), `cpuid_x86.c` (3,457),
  `dynamic.c` (18). The part reads each change and re-runs the suites on the new versions.

## Before the first release on PyPI

Checked 2026-10-13: the binaries need no other library than the system's own (on Linux:
`libc`, `libgcc_s`, and for `larmorx-gpl` glibc's `libm`, `libpthread`, `libdl`, `librt`).
Still to do:
- **A clean-machine check** (user, 2026-10-27): CI's runners have compilers and runtimes
  installed, which can hide a missing piece. Install and test on truly clean machines: Windows
  Sandbox on the Windows laptop (a throwaway clean Windows), a minimal Linux container here
  (`apptainer` with a slim Python image); and test `larmorx-gpl` and `larmorx-nc` as installed
  Python packages on the six platforms (today CI runs only their `cargo test`). Requirements
  stated for users: Python 3.12 or newer; Linux with glibc 2.28 or newer; templates (and later
  FreeSurfer's atlases) downloaded once on first use.
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
- **For the user's information, FSL-22** (2026-10-26; decision log): `reproduce` now sorts NaN
  border values in libstdc++'s order, from an emulation written from the SGI STL (HP/SGI
  permissive notice) with no libstdc++ text; the orchestrator accepted that licensing reading
  (algorithms, not code, are reused). The user may prefer the earlier behaviour instead (NaNs last
  in both modes, with a warning). No **decide** entry in the register.

Decided 2026-10-22 (decision log): ANTS-106 corrected by default (follow-up below); ANTS-30's
command line approved. Decided 2026-10-21: ANTS-78, ITK-35, ANTS-67, ANTS-68, ANTS-70, ITK-24, ITK-29
corrected by default (with A5 part 3); in-place writing as AFNI does; `LARMORX_GPL_BIN` kept;
PIPE-16 keeps N4's float32 result.
