# Next steps

The ordered plan from here: what runs next, what is waiting for the user, and the follow-ups.
Keep it current. The present state is in [status.md](status.md) (CLAUDE.md's "Where things are" points to it); the scopes of finished
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
through the merge checklist (docs/orchestration.md). While an agent runs, the orchestrator prepares the next task but
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

**From 2026-11-02 (the user; decision log):** two agents at once by role, each part run by relay agents
(CLAUDE.md, "Relay agents and fewer, fuller steps"): FLIRT's clean-room part 2 beside FAST's replica, then
FAST's clean-room original beside topup's replica, topup's clean-room beside `fslmaths` and `fslstats`'s
replica, and so on; before FreeSurfer, the templates (one replay harness, one page template, generated
option tables).

**Now (2026-10-29):** the version update's AFNI and FSL parts are done (the replicas follow AFNI
26.3.05 and FSL 6.0.7.23); the clean-room originals follow them too (item 3b), and part 3 (SciPy, numpy, FSL's matrix library) is done the same day; item 3c (the AFNI replicas in the main package) is done too; next, BET's clean-room original. Before: the move
(M, below), the ANTs tools' data panel (part a) and the fourteen whole template registrations
(14 of 14 identical with correctly rounded maths). The public summary of this order is the site's
Roadmap page, updated at every completed step.
The order from here, by the user's decision on testing depth (2026-10-28, decision log), one agent
at a time:

1. **antsAI and antsAffineInitializer: memory and speed: done** (2026-10-28): the metric's images
   in float as in ANTs (peaks at or below ANTs': 95 and 91 MiB against 92 to 96 and 89 on the
   memory test's head, 676 against 688 on the 7 T; the two strict xfails removed); Mattes on the
   sampled points with each point's fixed side once and the moving side in batches; ITK-21's
   second search sharing what it can, the two at once with threads. antsAI 1.8 to 2 times as fast
   as ANTs at one thread; antsAffineInitializer at ANTs' time in `reproduce` on the 7 T, 1.2 times
   it on the head, half of it by default; 4 to 10 times as fast at eight threads; bit for bit as
   before ([benchmark](benchmarks/ants-ai.md), [findings](findings/ants-ai.md), "Memory and speed
   (2026-10-28)").
2. **The clean-room fixes: done** (2026-10-29, `docs/parts/2026-10-29-cleanroom-fixes/`): 12 of
   the 13 differences random testing and the data panel found in the clean-room originals of
   3dTshift, 3dAutomask, 3dUnifize and the head-motion correction fixed or explained by black-box
   runs (the quaternion's `1/det` in extended precision, the closest-orientation letters,
   millisecond slice times, AFNI-07's default steps and qform as the replica's, a float64 −0 kept,
   the notices of AFNI-03, -08, -43, -07 and -16 in the replica's words; the panel's obliquity
   angles explained by AFNI-10; the head-motion differences explained on all 782 findings) with 13
   permanent cases; one left open: `-cost normmi` aligns worse (below, follow-ups). The thread
   probe needed nothing in the clean-room crates (`docs/findings/ants-cli.md`: no thread outside
   the call's pool).
3. **The version update** (user 2026-10-27, decision log; the upstream watch with it):
   **part 1, AFNI: done** (2026-10-29, `docs/parts/2026-10-29-version-afni/`): AFNI 26.3.05's
   changes to the three programs are two, both fixes of defects larmorx already corrected
   (AFNI-03, AFNI-05); the GPL replicas follow 26.3.05 (records against it, AFNI's own step from
   25.2.09 in each case); the clean-room originals stay on 25.2.09 until aligned (below); the
   upstream watch runs weekly from the nightly job; the relicensing inventory done (the user
   decided the move, item 3c). **Part 2, FSL: done** (2026-10-29, `docs/parts/2026-10-29-version-fsl/`): the replicas follow FSL
   6.0.7.23 (FSL-59 to FSL-63 new, FSL-41 fixed upstream; the zstd and bzip2 formats; bet2
   `--binaryMask`); records against 6.0.7.23 with FSL's own step from 6.0.7.17 in every case;
   `specs/mcflirt.md` (with `normmi` and `mutualinfo` exactly, appendix B) and `specs/bet.md`
   updated; Armadillo 14.2.1's LAPACK inverses not followed yet (part 3). **Part 3: done** (2026-10-29, `docs/parts/2026-10-29-version-numerics/`): the FSL
   replicas follow Armadillo 14.2.1's LAPACK inverses, solves and determinants (OpenBLAS 0.3.33's
   Haswell order, bit for bit against FSL's own library on 30.2 million matrices; mcflirt's five
   expected divergences pass; BET's trial cases 80 and 141 agree; FSL-64: the harness pins FSL's
   OpenBLAS core); the resampler follows SciPy 1.18.1 (SCI-03 fixed upstream) and is checked
   against numpy 2.5.4 with OpenBLAS 0.3.34 (no change); the oracle environment
   `oracles/python-newest` and the step per case in the resample-series record.
   - **3b. The clean-room originals follow the newest versions: done** (2026-10-29,
     `docs/parts/2026-10-29-cleanroom-align/`): 3dTshift, 3dAutomask and 3dUnifize follow AFNI
     26.3.05 (header slice code 2 and `-no_detrend` as 26.3.05 in both modes, AFNI-03 and
     AFNI-05 never named; records against 26.3.05: 3dTshift 315 cases, 280 pass, 32 rejected by both, 3 expected
     divergences, none failing; 3dAutomask 130 (110, 19, 1, none failing); 3dUnifize 117 (96, 19,
     2, none failing)); the head-motion correction
     follows mcflirt 6.0.7.23 (FSL-59, FSL-60, every `FSLOUTPUTTYPE` with zstd and bzip2 in pure
     Rust, files read by content, names completed with FSL's errors; record against 6.0.7.23:
     136 cases, 121 pass, 11 rejected by both, 3 expected divergences, 1 failing: `real/ds003345/default`,
     as against 6.0.7.17; ds000005's `-cost mutualinfo` and `normmi` runs now 0.0048 and 0.011 mm
     at the 95th percentile, FD r 0.984 and 0.953, from 0.049 and 0.065 mm, 0.64 and 0.54) and computes `-cost mutualinfo` and `normmi` exactly as `specs/mcflirt.md`
     appendix B (on the 169 findings of the first long random-testing run, median ratio to
     mcflirt 6.0.7.23 1.000 for both, `normmi` 55 of 91 within 1 % (before 1.051, 13 within 1 %),
     `mutualinfo` 54 of 78 (before 0.983, 11); with `-fudge -stages 1` mcflirt's evaluated
     matrices on 22 of 30 phantoms); the in-plane mode's centre is the padded volume's (found by
     `-verbose 20` traces). Both originals' defaults moved to the newest originals at the merge.
   - **3c. The AFNI replicas move into the main package: done** (2026-10-29/30,
     `docs/parts/2026-10-29-afni-move/`, [findings](findings/afni-relicensing-26.md), \"The move\"):
     `crates/larmorx-afni-replica` (Apache-2.0 with CC BY 4.0's attribution per file, in
     `PROVENANCE.md`, `NOTICE`; the legal code and Arm's MIT licence in `LICENSES/`), run in-process
     by the `larmorx` command line and the Python wrappers; `sinf`/`cosf` translated afresh from Arm's
     Optimized Routines v19.11 and proved equal to glibc's on all 2^32 inputs of each function and
     build (FMA, SSE2); `tanhf` fdlibm's; `cbrtf` measured: its last bit changes 3dUnifize `-EPI`'s
     ball on 1,449,010 float volumes (80 in [1, 100) mm³), so the replica takes CORE-MATH's and
     warns where it could matter (the user decides, \"Waiting for the user\"); the three replica
     records re-made in-process: no status change; golden checksums unchanged; `crates-gpl/` keeps
     only the verification tools; CI's `crates-gpl` steps gone (4.0 min a run).
4. **The ports:** BET's clean-room original (item 7), the clean-room additions mcflirt FSL-20/21
   and 3dUnifize AFNI-75 (specs first), FLIRT with BBR (the replica's parts 1 to 3 done 2026-11-01/02; the clean-room original's parts 1 and 2 done 2026-11-02), FAST (the replica done 2026-11-02, the clean-room original 2026-11-03), topup (the replica and the clean-room original done 2026-11-03; item 7), fslmaths and fslstats (the replicas done 2026-11-03; item 7); field maps,
   FreeSurfer, surfaces, confounds (item 8); larmorprepx (item 9). Each with its site page from
   its smart tests.
5. **The final validation phase** (after every tool is ported): the data panel for every tool
   (part b: AFNI, FSL and I/O; the new tools), full tiers, heavy and nightly random testing
   (realistic cases; an extreme share, `--extreme-share`, only if then decided, in boxes: the user,
   2026-11-02; the extreme findings of the porting parts' trials re-checked there),
   thread-band studies, the extensive evidence on each tool's page (added to what it shows),
   speed and memory studies beyond the rule's check, independent reviews, the agents' scripts
   triage (follow-ups), then the comparisons with fMRIPrep (item 9). Machine-only runs that need
   no agent (the heavy random-testing cases) may run before, in the background. The FSL replicas' full tiers against FSL 6.0.7.23
   (`fsl-mcflirt-replica-full` and `fsl-bet-replica-full` are 6.0.7.17's: the four mcflirt cases
   only the full tier runs, among them fMRIPrep's 280-volume multiband run of ds005454, and BET's
   real T1w and betsurf cases).

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
   every measured call); FSL-22's `reproduce` made exact the same day. **BET's clean-room original: part 1 done (2026-10-30).** `crates/larmorx-fsl`: `bet2` complete
   (every option, message and failure case of specs/bet.md §4, §7, §9: FSL-40, -42, -43, -56, -60,
   -61, -62 with `reproduce`), the `bet` script's default, `-R` (FSL-63), `-F`, `-Z` (FSL-45),
   FSL-44, FSL-46, the pending `-S`, `-B`, `-A`, `-A2`; `lx.fsl.bet(implementation="original")`,
   `larmorx fsl bet2|bet` (`auto`: the replica when installed, else the original). Record
   [fsl-bet](validation/fsl-bet.md): 108 of 111 compared cases within the thresholds (fMRIPrep's
   29 calls bit-identical; 202 of 217 brains, masks and outlines bit-identical; the 3 misses: real
   T1w images whose first pass reruns); built for speed (benchmark [fsl-bet](benchmarks/fsl-bet.md):
   2.6 to 3 times FSL's speed on one thread, 6 to 11 times on 24); memory within the rule (`tests/parity/test_memory_peak.py`: bet2 and `bet -F` below FSL's, `bet -R` 117 MiB against 103 plus the interpreter's 13). **Part 2:** betsurf's original (spec §6), the site page's
   clean-room panels, and — after the spec writer answers — the reruns and the skull search.
   **Spec questions** (for the spec writer, from [findings/fsl-bet-original.md](findings/fsl-bet-original.md)):
   (1) the rerun's smoothness rule (§4.5: FSL's first rerun differs from the rule as written though
   its factor 100 and its 1000 iterations are right; the three standard-tier cases that rerun miss
   the thresholds, Dice 0.980 to 0.995); (2) the skull search's details (§4.7: Dice about 0.9);
   (3) the quaternion's precision path (§8: the last float bit of one component in some real
   headers, about 5e-9 in NIfTI-2's doubles; the record allows 2^-23); (4) `-e`'s exact test
   (§4.5: FSL finds intersections where a segment-triangle test, with folds and coplanar overlap,
   does not, on small random heads: difftest 2026-10-30 #27); (5) the order of the sums over a
   vertex's neighbours (§8 gives the triangles' only: it decides the last digits on degenerate
   surfaces); (6) move the black-box observations of the findings into the spec, marked
   (observed): the mesh numbering, the rasterisation, the robust range's rounding (the width in
   float), small images (`n/50 = 0`), the fill from any start, the option parser's values, ANALYZE
   inputs, single slices written 2D, `--ct`'s outline in Cormack units, FSL-56's layout, FSL-62's
   brain, `bet`'s variation numbers, its failure line, FSL-44's splitting, `-F`'s and `-Z`'s files.
   **The six questions answered (spec writer, 2026-10-30; `specs/bet.md`, lines marked
   "(spec writer, 2026-10-30)"): BET's clean-room part 2 can start.** (1) The rerun rule was FSL's
   all along (from the initial surface bit for bit, the same lists; only `f2·F_P` capped at 1
   where `s·n > 0`); the reruns amplify last-bit differences: one of the original's ten
   arithmetic choices alone moves `older-75y`'s first rerun from FSL's 19809.7 to 16005-21766, and
   the replica carrying all ten gives the original's results exactly (16099; the same masks on
   `older-75y`, `7t`, `tumour`), so §8.1 now gives the deformation's arithmetic operation by
   operation (the initial placement, the normal and mean by division, `1/R` from `s_n·n`,
   `p + ((u1 + u2) + u3)`, the centre's single running sum, glibc's `tanh`: fdlibm's, not musl's).
   (2) The skull search step by step (§4.7: outside voxels count as surface voxels, three grids,
   the scan's stop rule). (3) The quaternion's precision path (§8.2: float matrices, x87 extended
   steps, NIfTI-2 written unrounded; checked 18 of 18 against FSL). (4) The `-e` test as a
   predicate (§4.5); on the small head of case 27 FSL's pair truly intersects on FSL's surface and
   the original's surface differs: the test is not the cause. (5) The neighbour order (§4.4: the
   move-to-end rule; a copy holds the same lists). (6) The black-box observations are in the spec,
   marked (observed); two new: a name with an extension falls back to the search (§3.1: FSL reads
   `in.nii` given `in.nii.gz`; the original refused) and `bet -F` on a float64 series writes a
   float64 brain (§5.2). Of the 2026-10-30 trial's 75 remaining differences, the replica with
   thirteen of the original's arithmetic choices reproduces 59 exactly; seven surface cases remain
   open for part 2 (45, 121, 154, 213, 262, 541, 677; 154 and 677 may be `-c`'s solve, §8.1).
   **Part 2a done (2026-10-30):** the spec writer's answers implemented: the deformation's
   arithmetic of §8.1 operation by operation (the mesh in the file's order with §4.4's neighbour
   order, the divisions, the update's sum order, the inward depth accumulated, the float products,
   the single centre sum, the robust range's exact arithmetic), the `-e` sweep and predicate
   (§4.5), the skull search (§4.7), the quaternion's precision path (§8.2, x87 steps; `bet -Z`'s
   round trip once per FSL program of the chain), a name with an extension falling back to the
   search (§3.1), `bet -F`'s float64 brain (§5.2); glibc's `tanh` as
   `larmorx_core::math::tanh_glibc` (from netlib's fdlibm, proved equal on 12.9 billion arguments)
   and CORE-MATH's correctly rounded `larmorx_core::math::tanh` (ported, bit-identical to the C on
   its 62,880 worst cases and 805 million arguments). Record re-made: **all 142 standard-tier cases
   agree** with FSL 6.0.7.23 (111 pass, 24 rejected by both, 7 expected divergences; `real/older-75y`,
   `7t`, `tumour` bit-identical; 13 of 13 skulls bit-identical; the quaternion exact; fMRIPrep's 29
   calls bit-identical); the random-testing trial's 75 open cases: 73 agree (237 of its 239). Speed
   unchanged (0.43 and 1.20 s on one thread, FSL 1.27 and 3.21 s). **Part 2b: done (2026-10-31 and 11-01).** betsurf's clean-room original complete (spec §6:
   every option, message and failure case; FSL-43, -47, -48, -50 to -55, -58 with `reproduce`;
   `larmorx fsl betsurf`, `lx.fsl.betsurf(implementation="original")`, and `"auto"` without
   `larmorx-nc`); its profile rules settled by a black-box oracle of single profiles (330 of 330
   decided as FSL decides); GLIBC-06 as the user decided (`--tanh correctly-rounded` on request,
   the notice removed); record [fsl-bet](validation/fsl-bet.md) re-made with betsurf's real heads:
   127 of 128 compared cases within the thresholds (ds002603's scalp with its T2 misses: boundary
   p95 2 voxels), all 217 bet2 and bet brains, masks and outlines bit-identical; the site page's
   clean-room panels; benchmark [fsl-bet](benchmarks/fsl-bet.md) re-made (the original 2.5 to 3.6
   times FSL's speed on one thread, 6.5 to 13.5 on 24); random testing's trial (1,846 cases): two
   corrections of the original (`-c 0 0 0`, `bet -F`'s float64 `_tmp`), four permanent cases.
   **Open, for the spec writer (BET's questions,
   [findings/fsl-bet-original.md](findings/fsl-bet-original.md)):** (1) the centre's conversion
   on a neurologically stored image (cases 622 and 677, and now 17 of the trial's `bet`/`bet2`
   cases: `bet -R`, `-c`); (2) betsurf's deformation where few profiles vote (the inner skull up to
   11 mm on a few vertices of a skull base) and the scalp's small differences everywhere; (3) what
   betsurf's profiles read on small images (the trial's 124 differing cases); (4) when FSL-50
   stops the program and when FSL only sets the profile aside (cases 216, 635); also the observed
   T1-only rule (`OS2 >= IS`) and the two observations of 2026-11-01 to add to §4.1 and §5.2
   (`docs/parts/2026-11-01-cleanroom-bet-part2b/handoff.md` §7). **For the spec writer (the first of the four above,
   in detail):** FSL's conversion of a centre between the voxel frames of a neurologically stored
   image (`bet2 -c`, `fslstats -C`: a 4 x 4 inverse whose last bits §8.1 leaves open); random-testing
   cases 677 (`-c`: another first-pass surface) and 622 (`bet -R`: `fslstats -C` prints `-0.000000`,
   and the script's `dc` prints `dc: stack empty` twice; every file FSL's) wait for it. And `bet -S`, `-B`, `-A`, `-A2` once
   standard_space_roi, FLIRT and FAST are replicated. Then FLIRT with BBR, FAST, topup, and **`fslmaths` and `fslstats`** (found
   missing from the plan on 2026-10-30, the user's question: fMRIPrep calls `fslmaths -mul` in every
   BOLD reference's skull strip, and 14 `fslmaths` and 4 `fslstats` in the optional goodvoxels mask,
   `docs/analysis/01-fmriprep-bold-pipeline.md` rows 7 and 11; the BET replica holds only what
   `bet`'s script runs of them: `-dilF`, `-mas`, `fslstats -C -l`): a replica and a clean-room
   original each, every operation (the full-completeness rule), like ImageMath for ANTs.
   **FLIRT: the replica's part 1 done (2026-11-01).** `crates-nc/larmorx-nc-flirt`: `flirt` (every
   cost function: `corratio`, `mutualinfo`, `normmi`, `normcorr`, `leastsq`, `labeldiff`, `woods`,
   `bbr` with `-wmseg` or `-wmcoords`/`-wmnorms`, its types, slope and a field map; the search; the
   pyramid; the schedule language, every command; the default 3D and 2D schedules and FSL's shipped
   ones; `-applyxfm`, `-applyisoxfm`; weights; every output type) and `convert_xfm`; `larmorx fsl
   flirt|convert_xfm` through the replica dispatch (no original yet); `lx.fsl.flirt`,
   `lx.fsl.convert_xfm`; the core gained the weighted costs, `labeldiff`, the unsmoothed entropies,
   BBR (`larmorx-nc-core::bbr`), FLIRT's filters and Powell's optimiser (mcflirt's and BET's records
   unchanged: 236/236, 178/178). Record [fsl-flirt-replica](validation/fsl-flirt-replica.md): 107 of
   107 compared cases bit-identical (34 more rejected by both, alike), fMRIPrep's BBR calls on ds005040, ds003592, ds000228 (their
   matrices also fMRIPrep's own as text). `specs/flirt.md` for the clean-room original. FSL-67 to
   FSL-74. Memory below FSL's (fMRIPrep's BBR call 395 against 452 MiB, a 12-dof call 247 against 259).
   Speed (informal, one thread, a 3 mm BOLD reference to a 0.8 mm T1w): the default 12-dof run 3.1 s
   against FSL's 4.6 s, fMRIPrep's BBR call 57 s against 47 s. Random testing: descriptor
   `fsl-flirt`; the trial (half an hour, 8 jobs, 3945 cases on images up to 32 voxels per axis) found
   928 differences; two causes fixed (`-applyxfm`'s blur beyond the image's edges; the sign of BBR's
   NaN cost without boundary points), permanent cases `applyxfm-blur-extraslice` and
   `bbr-no-boundary-points-nan`; after the fixes 577 of the 936 findings agree, the other 359 are
   triaged for part 2 (`validation/difftest/triage.toml`, `docs/findings/fsl-flirt.md`): FSL
   corrupting its heap on small or constant images (281: to be traced, then refused in both modes),
   66 matrices (64 with voxels below 0.5 or above 5 mm), the fix pass changing results without a
   notice (8: FSL-14 on infinite voxels; `-setbackground` with spline), one message count with
   `-dof 3`, one more heap corruption, 2 runs over 3 GiB. **Part 2 done (2026-11-01).** The site's FLIRT page
   (fMRIPrep's BBR on three oracle runs with the white-matter boundary, a default 12-dof registration,
   `-applyxfm` on a raw BOLD series; the matrices as text with fMRIPrep's own, the displacement over the
   brain, the record's cases). The trial's findings settled (`recheck_findings.py`: every finding that
   fits the trial's memory agrees in its shrunk and drawn forms): FSL's heap corruption traced to its
   background value of volumes 3 voxels wide (FSL-75, rule A: the whole border by default, FSL's values
   with `reproduce`; a single voxel refused); the 66 matrices were FLIRT's base-scale reciprocal computed
   in single precision (fixed); the fix pass's unnamed changes were FSL-14 and FSL-09 (named now).
   FSL-67 measured (rule C, kept). `-anglerep quaternion`; the hangs FSL-71/72 as suite cases (FSL under a
   timeout and an address-space limit). FLIRT's other programs `avscale`, `img2imgcoord`, `std2imgcoord`,
   `img2stdcoord`, `rmsdiff`, `makerot` (FSL-76, FSL-77), each with its command line, `lx.fsl` wrapper,
   golden case and spec section (`specs/flirt.md` §15). The replica's peak memory on large references
   brought to FSL's (262 → 187 MiB against FSL's 192; with `-inweight` 503 → 310 against 374). FSL-22 in
   FLIRT (the padding's NaN order). Record [fsl-flirt-replica](validation/fsl-flirt-replica.md): 217
   cases, 152 of 152 compared cases bit-identical (61 rejected by both, alike; 4 expected divergences:
   FSL-71, FSL-72 and FSL-75's two, where FLIRT never finishes or crashes); part 1's 141 cases unchanged
   in status and details (`compare_records.py`); mcflirt's and BET's standard tiers re-run (the core
   changed): 236 of 236 and 178 of 178 cases identical to their records. Two short trials of the new
   paths (half an hour and six minutes, 12376 and 3274 cases; descriptor `fsl-flirt-programs`).
   **Part 3 done (2026-11-02).** FLIRT's replica is complete but for the coordinate programs'
   `-warp` (FNIRT's warp fields: after FSL's warp-field libraries, the follow-up below) and the
   scripts of FSL's other programs (`standard_space_roi`, `flirt_average`, `epi_reg`: with
   `fslmaths`, FAST and the field maps). Added: FSL's LAPACK beyond 4 × 4 in `larmorx-nc-core`
   (`dsyevd`, `dgesdd` with OpenBLAS's Haswell BLAS, every word identical to FSL's library on 400
   records); the schedule command `alignpaxes` and every parameter-subset mask (`specs/flirt.md`
   §8.3, §8.7); `-debug` and BBR's debug volume (§9.4); `midtrans`, `pointflirt`, `applyxfm4D`
   (every data type and interpolation, the double template included) and the scripts `pairreg`,
   `extracttxt`, `aff2rigid` (§15.5-15.8), each with its command line, `lx.fsl` wrapper and golden
   case; makerot's NaN signs; mcflirt's FSL-75 (named, the whole border by default); FSL-78 to
   FSL-81. Record [fsl-flirt-replica](validation/fsl-flirt-replica.md): 296 cases, 205 of 205
   compared cases bit-identical (87 rejected by both, alike; 4 expected divergences: FSL-71, FSL-72
   and FSL-75's two); part 2's 217 cases unchanged in status and details (`compare_records.py`);
   mcflirt's and BET's standard tiers re-run (the core changed): 237 cases, every one of the
   committed record's 236 identical, the new `edge/fsl75-narrow` rejected by both (FSL aborts with a
   double free; FSL-75 named) and 178 of 178 identical to the committed record (not re-committed).
   One trial of the new paths (half an hour, 209,657 cases, after a pre-check of 2,805; the
   realistic findings re-checked boxed with the final build, the extreme ones deferred to the final
   validation phase by the user's decision of 2026-11-02); four robustness cases with nonsense
   headers (metres, voxels of 0.01 and 50 mm) for applyxfm4D, midtrans, pointflirt and pairreg, each
   identical to FSL in a 4 GiB box. `specs/flirt.md` §14 lists only what is left by decision.
   The full tier, the data panel and long random testing wait for the final validation phase.
   **The clean-room original** (another agent, from `specs/flirt.md`, on the main package's
   clean-room engine, the cost functions, optimiser and transform algebra of `larmorx-mri`'s
   head-motion correction, never a second copy: the user, 2026-11-01, "avoid duplication"). **Part 1
   done (2026-11-02):** `flirt` and `convert_xfm` (crate `larmorx-fsl`, module `flirt`, on
   `larmorx_mri::hmc`'s shared engine; record `fsl-flirt`: 89 of 111 compared cases within
   thresholds, fMRIPrep's three BBR calls within 0.1 mm; `-anglerep quaternion` and `alignpaxes`
   included). **Part 2 done (2026-11-02, seven relay agents):** FLIRT's other programs (avscale, the
   coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D, pairreg, extracttxt,
   aff2rigid), `-debug` and every verbose level, the speed (fMRIPrep's BBR call 1.2 to 1.3 times
   FSL's speed on one thread, 12 to 14 times on 14; the benchmark report `docs/benchmarks/fsl-flirt.md`),
   the spec writer's answers (the record's open cases 30 -> 8: BBR's normals, the correlation ratio's
   and Woods' sums, §3's Euler composition, the search's grids and starts, the sinc and spline,
   `-usesqform`, `-fixscaleskew`, the inverse; FSL-90 to FSL-93 with `reproduce`), one random-testing
   trial (24 minutes, 9,854 realistic cases of every program; its fixable findings fixed), FSL-111
   (applyxfm4D reads uint8 as signed bytes: rule A), the memory and thread tests of every new
   program, the site's clean-room panels. Record `fsl-flirt`: 206 of 214 compared cases within
   thresholds (85 rejected by both, 6 expected divergences); the 8 it misses: ill-posed registrations
   (`-cost normmi`, `leastsq`, weighted `normcorr`, `normmi`, `woods`' stream) and FSL-91/FSL-92's
   reproduced runs. 24 golden cases added, 8 re-recorded on purpose. **The spec questions answered (spec writer, 2026-11-02;
   `specs/flirt.md`, lines marked "(spec writer, 2026-11-02)"; §14's table "The clean-room
   questions of 2026-11-02" names each answer's section and whether the observation was right):
   FLIRT's clean-room part 2 can close its open cases.** The 20 questions in §3, §4.1, §4.2, §6.2,
   §6.5, §7.2, §7.4, §8.1, §8.2, §8.5, §8.7, §9.2, §9.3 and §12 (the correlation ratio's and
   Woods' sparse bins and formula, the correlation ratio's two final roundings, BBR's Gaussian of
   σ = 2 mm and gradient weights 3, 1, 1/3, `local_abs`, the search's grids, minima and starting
   matrices, the convergence test, the line search's tolerance unit, the windowed sinc's and the
   spline's arithmetic, `-usesqform`'s single-precision field of view, `-fixscaleskew`'s
   composition, the inverse's zeros, the paddings); every precision answer proved by a model
   written from the spec's text alone that gives FSL's numbers
   (`validation/investigations/flirt-spec-answers/`: BBR's 16 printed costs, the search's first
   sweep of 23 evaluations on `real/dof6`, 40000 of 40000 sinc voxels, 172800 of 172800 spline
   values, five matrix outputs). The two candidate refusals and two leads decided: FSL-90 to
   FSL-93 (§11; in the replica with their `reproduce` switches; for the clean-room original in
   its part 2, its suite `fsl_flirt.py` needing `Run.hang_fixed` for `edge/zero-voxel-size`). Follow-up for `lx.mri.hmc` (its
   own part; not changed here, its record stays identical): the correlation ratio's sparse-bin rule
   found here (a reference bin with at most two points is left out of the counts, the sums and the
   total variance; `docs/findings/fsl-flirt-original.md`, "The costs") probably holds for mcflirt's
   `-cost corratio` too (`specs/mcflirt.md` appendix A leaves the sparse bins in the total); and the
   search findings (a subset's convergence test over its own tolerances) for its schedule-free
   optimiser if it uses subsets. Answered (spec writer, 2026-11-02, `specs/mcflirt.md` appendix A):
   mcflirt's `-cost corratio` and `woods` are FLIRT's two costs, the sparse bins left out of the
   counts, sums and total variance, and Woods' `n_b²σ_b/S_b` (`n_b²σ_b` when `S_b ≤ 0`). Done in FLIRT's clean-room part 2 (relay 4, 2026-11-02): `lx.mri.hmc`'s
   `corratio` and `woods` are FLIRT's (one implementation in `larmorx-mri`), its record re-made: the
   same statuses, four cases closer to mcflirt (`docs/findings/mri-hmc-original.md`). The scripts wait for other tools: `epi_reg` (FAST, `fslmaths`,
   `fugue`/`applywarp` for field maps, `convert_xfm`, `flirt`), `standard_space_roi` (`flirt`,
   `fslmaths`, `fslroi`), `aff2rigid` (`avscale`), `flirt_average` (`flirt`, `fslmaths`,
   `convert_xfm`), `pairreg` (`flirt` with `pairreg*.sch`, `bet`).
   **FAST: the replica done (2026-11-02, relays 1 to 5).** `crates-nc/larmorx-nc-fast`: `fast`
   complete (every option, message, exit status and failure case: the single-channel segmentation
   with its bias field, partial volumes and outputs, one to eight channels with Armadillo's and
   OpenBLAS's arithmetic followed to 8 × 8 covariances, FSL's tissue priors registered by FLIRT's
   replica in the same process); `lx.fsl.fast` and `larmorx fsl fast` (replica only until the
   clean-room original); `larmorx-nc-core` gained the C library's `expf`, `log` and `rand`, FSL's
   masked sums and percentiles and LAPACK up to 8 × 8 (additive: mcflirt's, BET's and FLIRT's records
   re-made from the final commit, identical case by case, `compare_records.py`). Record
   [fsl-fast-replica](validation/fsl-fast-replica.md): 86 cases, 59 of 59 compared cases bit-identical
   (every file, the messages, the exit status; fMRIPrep's call on ds000005, ds000228 and ds005003,
   fMRIPrep's own partial-volume maps the same), 26 rejected by both alike, 1 expected divergence
   (FSL-104, refused). FSL-96 to FSL-104. One random-testing trial (half an hour, 17,497 realistic
   cases: 4 differences, all FSL-101, fixed in `reproduce`, three permanent cases). Golden: 11 cases.
   Memory below FSL's on the measured calls (`tests/parity/test_memory_peak.py`); results independent
   of the thread count (`thread_pools.py`). Its site page (fMRIPrep's call on three runs, the bias
   field, the priors with FSL-09's correction, three channels). `specs/fast.md` for the clean-room
   original (its §9.2 lists the rule-D candidates, §9.3 what is not replicated). The full tier
   (fMRIPrep's call on every oracle run), the data panel and long random testing wait for the final
   validation phase.
   **FAST: the clean-room original done (2026-11-03, relays 1 to 5).** `crates/larmorx-fsl`, module
   `fast` (Apache-2.0, from `specs/fast.md`, the paper and black-box runs only): every option, message,
   exit status and failure case; one channel and any number (`-S 9` and more too: rules A-D, nothing
   to refuse; the replica refuses them), larmorx's own LU for the covariances; the priors registered
   by FLIRT's clean-room original in the same process; FSL-96 to FSL-104 in both modes; `exp` and
   `log` correctly rounded. `lx.fsl.fast(implementation="original")`, `"auto"` without `larmorx-nc`.
   Record [fsl-fast](validation/fsl-fast.md): 87 cases, 60 of 61 compared within `specs/fast.md`
   §11's thresholds (fMRIPrep's call on ds000005, ds000228 and ds005003 bit-identical to FSL run with
   correctly rounded maths; of the 60, 13 bit-identical to FSL, 36 to FSL with correctly rounded
   maths, 10 within FSL's own difference between the two, 1 a few partial volumes beyond it, each one
   fraction step away: the site's classification), 25 rejected by both, 1 expected divergence
   (FSL-104), 1 explained failure (`multi/s5`: nearly singular 5 × 5 covariances). Built for speed:
   the MRF and mixel sweeps as a pipeline of planes with the serial result, FSL's blocked sums in
   parallel in their order, a batched `exp` with the scalar bits (`larmorx_core::math::exp_in_place`,
   new, randomized check of 2^32 values: 0 differences); fMRIPrep's call 2.4 to 5.2 s on one thread
   (9 to 11 times FSL), 0.4 to 0.9 s on 14 (47 to 67 times), the same bits at any thread count
   ([benchmark](benchmarks/fsl-fast.md), informal). One random-testing trial (25 minutes, 9,005
   realistic cases, FSL through the oracle cache: 8,823 agree; 182 triaged: 153 the suite's reader,
   13 a NaN bias field outside the mask and 12 messages fixed, 3 explained, 1 open, four permanent
   cases). Golden: 7 cases. Memory: fMRIPrep's call 152 MiB against FSL's 707. Thread test: four
   programs. The shared ANALYZE reader (BET's, FLIRT's and FAST's originals: the scale field not
   applied, negative voxel-size products radiological) and FLIRT's `-applyxfm` TR (FSL's: the
   input's `|pixdim[4]|`) changed: BET's and FLIRT's records re-made, identical case by case. The
   site page with both implementations. FSL-136 to FSL-140 unused (no new defect).
   **topup: the replica done (2026-11-03, relays 1 to 11).** `crates-nc/larmorx-nc-topup`: `topup` and
   `applytopup` complete (every option, message, exit status and failure case; topup's levels,
   both minimisers in double and single precision, both regularisers, the movements, regridding,
   FSL's configurations, `--debug`, `--trace`, `--profile`; applytopup's `jac`, `lsr` and the
   undocumented `vb2D`, `vb3D`, `vb4D`); `lx.fsl.topup`, `lx.fsl.applytopup` and `larmorx fsl
   topup|applytopup` (the replica when `larmorx-nc` is installed, the clean-room original otherwise: below). `larmorx-nc-core` gained FSL's
   libraries for the FNIRT family too: the B-spline fields (fsl-basisfield) with their bending- and
   membrane-energy regularisers, the sparse matrices, conjugate gradients and the non-linear
   optimisers (fsl-miscmaths), the spline interpolator with derivatives and the warps
   (fsl-warpfns), OpenBLAS's blocked LU, LAPACK's symmetric solver and inverse, each checked against
   FSL's own libraries by probes (millions of values, 0 differ); its `dgemm` now sums as OpenBLAS's
   kernel does (four partial sums): mcflirt's, BET's, FLIRT's and FAST's replica records re-made
   from the final commit, identical case by case (`compare_records.py`). Record
   [fsl-topup-replica](validation/fsl-topup-replica.md): 173 cases, 104 of 104 compared
   bit-identical (every file, the messages, the exit status; fMRIPrep's call on ds005040 and
   ds001600, fMRIPrep's own outputs the same), 66 rejected by both alike, 3 expected divergences
   (FSL-124, FSL-134: refused in both modes). FSL-116 to FSL-135 and FSL-141 to FSL-143. One
   random-testing trial (27 minutes, 14,460 realistic cases, FSL through the oracle cache: every
   one bit-identical). Golden: 7 cases. Memory within FSL's on the measured calls (fMRIPrep's
   call 94 MiB against FSL's 88; the allowance holds); thread test: topup and applytopup. Its site
   page (fMRIPrep's two calls with FSL's own thread band beside the replica, FSL-117, applytopup's
   `lsr`, `jac` and `vb3D` with FSL-132 and FSL-143). `specs/topup.md` for the clean-room original
   (behaviour, maths, messages, the D13 table, "Built for speed", the validation plan). The full
   tier, the data panel's other PEPOLAR runs and long random testing wait for the final validation
   phase.
   **topup: the clean-room original done (2026-11-03, relays 1 to 6).** `crates/larmorx-fsl`, module
   `topup`: `topup` and `applytopup` written from `specs/topup.md`, the papers, FSL's user guides
   and black-box runs, every option, message, exit status and failure case of both programs
   (applytopup's `vb2D`, `vb3D`, `vb4D` included); FSL-116 to FSL-135 and FSL-141 to FSL-143 as
   the replica corrects them, `reproduce` FSL's where its behaviour is defined and known from
   black-box runs (FSL-133's wrap and FSL-134's field in another shape are not: restored
   circularly and refused in both modes); `sin`, `cos`, `exp` correctly rounded.
   `lx.fsl.topup|applytopup(implementation="original")`, `"auto"` without `larmorx-nc`. Record
   [fsl-topup](validation/fsl-topup.md): 180 cases (the replica suite's 173 and 7 permanent cases of
   random testing), 92 of 92 compared within `specs/topup.md` §12's thresholds (fMRIPrep's call on
   ds001600 within FSL's own thread band in every metric; on ds005040 the metrics beyond the
   thresholds, the 99th percentiles of the field, the corrected images, the Jacobians and the
   displacements, within FSL's band at `--nthr=4`, and the field's correlation 0.999958 against
   FSL's own 0.999966; of the 92, 9 bit-identical to FSL, 2 within FSL's own thread band in every
   metric, 81 within the thresholds without a band of FSL's on them or beyond the near-zero band of
   the thread settings that leave FSL unchanged: the site's classification), 66 rejected by both,
   22 expected divergences (9 estimated movements that do not converge: FSL's rotation derivatives
   are not its cost's, findings 14, a spec question; 4 FSL-133's lines; 3 FSL-143's divergence;
   vb2D's transients; FSL-126; 4 refused for FSL-124 and FSL-134). New defects: FSL-144 (`jac`
   with a z phase encoding left uncorrected: rule B), FSL-145 (two quadratic coefficients on a
   short axis: rule A), FSL-146 (mixed phase-encoding axes: FSL's gradient is not the cost's, rule
   A); FSL-147 to FSL-150 unused. Built for speed: the spline field's products along whole lines
   and planes, Levenberg-Marquardt's steps by preconditioned conjugate gradients on the Hessian
   stored by its diagonals, an array pool, deterministic parallel loops; fMRIPrep's call 2.8 and
   7.7 s on one thread (10 to 12 times FSL), 0.8 and 2.0 s on 14 (39 to 41 times), the same bits
   at any thread count; applytopup `lsr` 0.13 s against FSL's 0.42
   ([benchmark](benchmarks/fsl-topup.md), informal). One random-testing trial (11 minutes, 14,400
   realistic cases, the replica trial's draws, FSL through the oracle cache: 7,401 agree; the
   6,999 others triaged by category, findings 36: 4,462 finding 14, 895 FSL-146, 484 FSL-133, 429
   the quadratic count (fixed), 385 the suite's own mistakes (fixed), 216 the SCG weight (fixed)
   and ill-determined paths, 51 FSL-143, 22 other explained, 55 open in two categories; seven
   permanent cases). Golden: 6 cases (76 outputs). Memory: fMRIPrep's call 74 MiB against FSL's
   88. Thread test: every parallel path of topup and applytopup. The site page with both
   implementations (the original's verdict on each case from its files against FSL's).
   **fslmaths and fslstats: the replicas done (2026-11-03, relays 1 to 7 and 6b).**
   `crates-nc/larmorx-nc-avwutils` (fslmaths' program and both command lines) on `larmorx-nc-core`
   (fslstats' program, because BET's script runs it, and the shared code): every operation of
   fslmaths (87 in its usage text and the hidden `-pow`, `-invbin`; every kernel, the separable,
   direct and FFT convolutions, morphology, subsampling, edges, hole filling, the reductions along
   each dimension, p-values, ranks, `-roi`, `-bptf`, TFCE and its support, ROC, the tensor's
   decomposition; every data type in and out) and every option of fslstats (`-t`, `-K`, `-json`),
   with every message, exit status and failure case; `lx.fsl.fslmaths`, `lx.fsl.fslstats` and typed
   wrappers of every family, `larmorx fsl fslmaths|fslstats` (replica only until the clean-room
   originals). `larmorx-nc-core` gained newimage's kernels, filters, connected components and
   whole-volume statistics (masked and unmasked), fsl-cprob's probability functions (checked
   against FSL's library: 180,890 arguments, 0 differ), Armadillo 14.2.1's maximum and minimum (one
   translation for every replica) and its Kiss FFT, nlohmann/json 3.12.0's numbers (1.9 million, 0
   differ), glibc's `powf` from Arm's routines (10 million, 0 differ), CORE-MATH's double `tan` in
   `larmorx_core::math`, an unsigned byte type (FSL-154), and a NIfTI reader that streams raw and
   gzip images (memory at or below FSL's: `-Tstd` of a 63-volume series 37.5 MiB against FSL's 75).
   Records [fsl-maths-replica](validation/fsl-maths-replica.md): 421 cases, 358 bit-identical (the
   four combine_masks calls of three oracle runs and the goodvoxels mask's fourteen, fMRIPrep's
   own outputs the same), 61 rejected by both, 2 expected divergences (GLIBC-07: `-dt double -tan`,
   an FFT's twiddle factors from glibc's `sincos`); [fsl-stats-replica](validation/fsl-stats-replica.md):
   208 cases, 172 identical (goodvoxels' four calls, BET's, FSL's own tests), 36 rejected by both.
   The five FSL replica records re-made from the final commit (the shared reader, the maximum,
   FSL-187's robust range, the index error's type, calc_cog's order): identical case by case.
   FSL-151 to FSL-189 and GLIBC-07; FSL-187 reaches BET and FLIRT. One random-testing trial
   (24 minutes, 183,983 realistic cases, FSL through the oracle cache: one difference, GLIBC-07)
   and a check run; golden: 31 cases; memory cases for both; no thread test (one thread). Site
   pages: fMRIPrep's mask combination and goodvoxels mask, one call per operation family on a real
   series, GLIBC-07 on a real image, every option of fslstats on a real image.
   `specs/fslmaths.md` and `specs/fslstats.md` for the clean-room originals. The full tier, the
   data panel and long random testing wait for the final validation phase.
   **fslmaths and fslstats: the clean-room originals done (2026-11-04, relays 1 to 7 and 6b).**
   `crates/larmorx-fsl`'s `maths` (fslmaths' kernels, filters, reductions, statistics, ROC,
   TFCE, the tensor) and `cli::fslmaths`, `cli::fslstats`, from `specs/fslmaths.md`,
   `specs/fslstats.md`, FSL's usage texts and black-box runs (no FSL source): every operation,
   option, data type, message, exit status and failure case of both programs; `lx.fsl.fslmaths`,
   `lx.fsl.fslstats` and their typed wrappers with `implementation="original"` (and `auto`
   without `larmorx-nc`). Built for speed: rows of voxels in parallel, fixed-order sums (the same
   bits at any thread count), one copy of the image, the direct sum where FSL takes its FFT (its
   products in double precision there); goodvoxels' chain 4.5 times FSL's speed with half its
   memory, the heavy operations 1.8 to 28 times on one thread and up to 268 times on 14
   ([benchmark](benchmarks/fsl-maths.md)). Records
   [fsl-maths](validation/fsl-maths.md): 442 cases, 374 within §21's thresholds (358 FSL's values
   in every file, fMRIPrep's four combine_masks calls and the goodvoxels mask's fourteen
   included), 63 rejected by both, 5 expected divergences (FSL's single-precision FFT on 3
   kernels, `-subsamp2offc` on 2);
   [fsl-stats](validation/fsl-stats.md): 162 cases, 125 identical, BET's `-C` within §10's bound
   (6.1e-8), 36 rejected by both. New defects from black-box runs: FSL-191 (FSL's FFT path makes
   a volume NaN), FSL-192 (its kernel mirrored), FSL-193 (`-roc`'s unwritable curve, rule B);
   FSL-187's switch in BET's and FLIRT's originals. One trial (20 minutes, 300,000 cases, FSL
   through the cache): 5,168 of the original's mistakes fixed (21 permanent cases), 10 open;
   golden: 9 cases (224 outputs); memory: 6 calls; thread test: every parallel path. topup's
   Windows file names done (below). Site pages with both implementations; findings 1 to 31 of
   `docs/findings/fsl-maths-original.md` (31: on a large kernel a single-precision direct sum
   is less accurate than FSL's FFT, beyond §21's bound on the page's real call; the original's
   double sums there, 2026-11-04, lie closer to the exact result than FSL's FFT, 2.0e-7 of FSL's
   largest value from FSL's output).
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

- **The site: a page "Why Rust and Python"** (the user, 2026-10-31: "At some point we should clearly
  explain [on] the website why we choose rust + python and explain all these points"). Before the
  first release at the latest; written for researchers, in plain words. The points, from the
  orchestrator's answers of 2026-10-31:
  - *Why AFNI, FSL and FreeSurfer have no Windows version* (from the projects' history and
    documentation, not their developers' statements): born on Unix workstations; the main programs
    are shell scripts chaining Unix tools (`recon-all` in tcsh with Perl steps, `bet` in sh with
    `dc`, `afni_proc.py` writing tcsh); Unix system features (`fork`, pipes, symbolic links, `/`
    paths, case-sensitive names, `FSLDIR`-style variables, AFNI-SUMA shared memory and sockets); C
    that changes meaning on Windows (`long` 32 bits under LLP64, `long double` 64 bits with
    Microsoft's compiler, C99 and GCC extensions); X11/Motif and Tcl/Tk interfaces; large
    dependency stacks; small academic teams, Linux clusters and Macs as users, WSL2 removing the
    pressure. Glatard et al. 2015 (results differ across operating systems).
  - *What Rust gives:* one build tool (`cargo`) for the six targets and pure-Rust crates, wheels
    through maturin/PyO3 (abi3: one wheel per platform for every Python >= 3.12); fixed-size types;
    no silent fused multiply-adds and no x87 on x86-64; memory safety (the register's out-of-bounds
    and uninitialised-memory defects stop with an error instead of a garbage result); data races
    refused by the compiler (results independent of the thread count); C-like speed.
  - *What Rust does not give, and the project's rules do:* Rust's own `sin`, `exp`, `log` call the
    system's maths library (different digits per platform), so larmorx uses its own correctly
    rounded functions (CORE-MATH's, 17 ported: `exp`, `log`, `pow`, `sin`, `cos`, `asin`, `acos`,
    `atan2`, `cbrt`, `tanh` and `expf`, `logf`, `powf`, `sinf`, `cosf`, `tanhf`, `cbrtf`) and, for
    `reproduce`, Rust copies of the C library's own (`cbrtf_glibc`, `tanh_glibc`, Arm's `sinf`/`cosf`,
    fdlibm's `tanhf`); matrix arithmetic in one fixed operation order (OpenBLAS's Haswell kernels
    for numpy's 4 x 4 products and inverses; reference LAPACK's LU, inverse and solves for FSL's;
    only the routines the tools call, never a CPU's own kernels); ITK's vnl routines, Eigen's
    symmetric eigensolver, SciPy's spline interpolation and HDF5 in pure Rust; no C, C++ or Fortran
    dependency; explicit thread pools, seeded random numbers; golden tests on six platforms at
    every push. "Rust made the solution practical, and the rules made it work."
  - *Why Python on top:* the users' language: each tool's wrapper, typed inputs and outputs, and
    larmorprepx as plain Python functions; the Rust core does the computing and releases the GIL.
  - *Honest limits:* the six-platform proof uses small golden inputs (up to 32³); full-size runs on
    Windows and macOS come with the final validation phase and the clean-machine check; the
    originals stay Unix tools (larmorx is an alternative beside them, not a fix of them).
- **The `libm` crate's last uses** (seen 2026-10-31): `acos` in `crates/larmorx-afni/src/geometry.rs`
  and `crates/larmorx-core/src/affine.rs`, `atan2` in `crates/larmorx-mri/src/hmc/rigid.rs`, `pow`
  in `crates/larmorx-interp/src/lib.rs` (musl's, pure Rust) although `larmorx_core::math` has all
  three correctly rounded now (rule 5): check whether each is deliberate (an original's bits) and
  switch or document it, with the records the change touches; the clean-room crates' uses go to a
  clean-room agent. Also `crates/larmorx-transform/src/openblas.rs`'s header still names OpenBLAS
  0.3.30 and numpy 2.3.5 (the resampler follows numpy 2.5.4 since 2026-10-29): check and correct.
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

- **mcflirt's FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original: done** (2026-10-30, from
  `specs/mcflirt.md` §10.6, §12.1; the 33 cases of §14.1 in the mri-hmc suite, `failures/<case>`,
  mcflirt live; golden `cli.mri.hmc.unwritable` and `cli.mri.hmc.bins`; `docs/findings/mri-hmc-original.md`).
- **The mcflirt replica's FSL-65 and FSL-66: done** (2026-10-30, `specs/mcflirt.md` §10.6 and
  §12.1; the 33 cases of §14.1 in the replica's suite, mcflirt 6.0.7.23 live: 24 pass, 9 rejected
  by both, the replica's matrices mcflirt's as text wherever both run to the end; a command-line
  test each; golden `mcflirt.fsl65.*`, `mcflirt.fsl66.*`; FSL-21 named in `reproduce` too).
- **FLIRT's NaN signs** (2026-11-02): the sign of an all-NaN `-omat` matrix with an infinite input voxel under `reproduce` (FSL-14) and a
  schedule that optimises relative to a row (FSL `-nan`, the replica `nan`; part 2's path, found by
  part 3's pre-check, probe-part3b-00496; `docs/findings/fsl-flirt.md`, "Open"): trace the NaN of
  the input's centre of gravity.
- **FLIRT's clean-room original, what is left** (2026-11-02, part 2's report; for the final
  validation phase or a part of its own): the sinc and spline outputs' last float bits (1.6 % of
  `real/interp-sinc`'s voxels, applyxfm4D's spline of float images; spec question 22 of part 2's
  handoff); SIMD sampling with runtime dispatch (the same bits) and a lower-overhead parallel pass
  for the correlation ratio's small evaluations: the one-thread 12-dof registration is 0.9 times
  FSL's speed; the registrations' paths on ill-posed random pairs (the trial's 1,808 of 3,616
  registrations; `-dof 3`'s and FSL-90's message counts, the COG warnings, which follow the path);
  `-debug`'s volumes' headers on the trial's oblique and anisotropic images; avscale's residual sign
  in one of twelve sampled cases; the 8 cases its record misses.
- **FLIRT's part 2 spec questions: answered** (spec writer, 2026-11-03; `specs/flirt.md`, lines
  marked "(spec writer, 2026-11-03 ...)"; §14's table "The clean-room questions of 2026-11-03" names
  the section of each of the 38 (3 superseded by 27, 15 settled before, 29 answered by 30) and
  whether the observation was right; models: `validation/investigations/spec-answers-3/flirt_models.py`
  and `flirt_models_3b.py`, every FSL run through the oracle cache). FSL-204 found (pointflirt
  without points crashes FSL: refused in both modes, rule B; in the replica since 2026-11-03).
  **For FLIRT's clean-room original (its next part), what the answers let it close:** (1) §2.3:
  the free optimisations print at the search's lowered verbose level (not lowered again); (2) §8.1
  step 9: an invalid quaternion start keeps only its scale (no translations: FSL-90's
  `reproduce`), and FSL-90's message prints at every invalid composition, evaluations included;
  (3) §15.1: avscale's check residual signs come from §12's inverse; (4) §15.5: midtrans uses a
  transform whose determinant is not finite (NaN rows, exit status 0); (5) §15.6: FSL-204; a `nan`
  word in a coordinate file reads 0; (6) §15.2: `-warp` accepts exactly three volumes, other
  images get FSL's message and exit status 1; (7) §12: with `reproduce`, NaN and infinite matrices
  are inverted and FLIRT goes on (trial #2905, #704, #8361); (8) §15.8: pairreg's schedules carried
  by the original (no `FSLDIR`); (9) §2.3, §9.4: `-debug`'s lines and volumes (the levels compose
  only their set forms, a neurological reference's forms are swapped back with the level's grid,
  `global_refweight1`'s `pixdim[4..7]` are 1; BBR without boundary points: NaN costs that move
  nothing, the `-init` matrix written, the debug images and `-out` written; one-slice images lose
  their forms): the triage entry's 114 findings and #2424; (10) §9.2, §12: `reproduce`'s windowed
  sinc and the registrations' paths need the C library's single-precision `sinf`, `cosf` and
  `atan2f` (the default keeps the correctly rounded functions, rule 5): `real/interp-sinc`,
  applyxfm4D's float sinc findings, and the trace partings of `real/dof6`-like registrations
  (`cost-normmi`, `weights-normcorr`, `weights-normmi`, `weights-woods`) then close (how the crate
  gets those functions: the next item); (11) §15.7: applyxfm4D's spline is §9.3's (double images
  read and filtered in double, each value rounded to single): 5,177 of 13,315 voxels of five trial
  findings differ today; (12) §6.2: `leastsq`'s three-level sums (`real/cost-leastsq`); (13) §8.2:
  equal and NaN costs move nothing under §7.2's strict comparisons: FSL-91's and FSL-92's
  `reproduce` runs (`real/weights-empty-refweight`, `real/weights-empty-inweight`,
  `edge/one-voxel-input`) follow FSL's trace once `atan2f` is the C library's and the sign of a
  near-zero skew (printed `-0.000000` by FSL, `0.000000` by the original, in one trace) is found
  in the original's own composition and decomposition.
- **The C library's single-precision `sinf`, `cosf` and `atan2f` for the clean-room originals'
  `reproduce`** (2026-11-03, the spec answers; for the orchestrator and the user: moving code out
  of a replica crate needs the user's decision): FLIRT's clean-room original computes correctly
  rounded functions in both modes (rule 5), FSL's FLIRT glibc's (`sinf` and `cosf` are Arm's
  Optimized Routines v19.11, MIT; `atan2f` is fdlibm's, under Sun's permissive notice). With them
  the original's `reproduce` closes the FLIRT cases of item (10) above (`real/dof6`'s trace parts
  from FSL's at its 3,983rd evaluation by `sinf`/`cosf`, at line 4,814 by `atan2f`). The ways:
  a shared module in `larmorx-core` (the implementer may not read the AFNI replica's copy of
  Arm's `sinf`/`cosf`), or each from a behaviour spec, implemented clean-room (the `cbrtf` and
  GLIBC-07 route).
- **`lx.mri.hmc`'s centres of mass, blocked as mcflirt's** (2026-11-03, `specs/mcflirt.md` §6.1,
  spec writer): both of mcflirt's centres are FLIRT's blocked sums (`specs/flirt.md` §8.1 step 3:
  `v − v_min` in single precision, double running sums over the voxels in z, y, x order flushed into
  the totals after every max(1000, ⌊√N⌋) + 1 voxels; a denominator below 1e-5 in magnitude counts
  1, with `WARNING::in calculating COG, total = 0.0` on standard error). For the clean-room
  `lx.mri.hmc` (`crates/larmorx-mri`) in a later part: follow it and measure the centre's last bits
  and the `.par` and matrices' last digits on its record.
- **FSL-111 in the replica: done** (2026-11-03, the spec answers part): `larmorx-nc fsl applyxfm4D`
  reads a uint8 image as stored by default and names FSL-111 where the bytes written differ (its
  default resamples both readings to compare them); `reproduce` reads it as FSL, without a notice
  (`programs/applyxfm4D-uint8`, `programs/applyxfm4D-uint8-3d-spline`; golden `applyxfm4D.uint8`).
- **FSL-144, FSL-145 and FSL-146 in topup's replica: done** (2026-11-03, the spec answers part):
  its defaults refuse `applytopup --method=jac` with a z phase encoding (FSL-144), lay the general
  quadratic B-spline count on short axes (FSL-145; applytopup reads a coefficient file of either
  count) and minimise with the cost's own gradient on volumes encoded along different axes
  (FSL-146), each named, `reproduce` FSL's: FSL's count exactly (one coefficient when the axis is
  shorter than one spacing; `specs/topup.md` §4.2) and FSL's error when a level refits a single
  coefficient (`at/jac-z`, `opt/fsl145-short-axis`, `opt/fsl145-one-coefficient`,
  `opt/fsl146-four-axes`; golden `topup.fsl146`, `applytopup.jac-quadratic`).
- **topup's spec questions: answered** (spec writer, 2026-11-03; `specs/topup.md`, lines marked
  "(spec writer, 2026-11-03)"; §13's table "The clean-room questions of 2026-11-03" names each
  answer's section, whether the implementer's observation was right, and the model, written from
  the spec's text alone, that gives FSL's numbers: `validation/investigations/spec-answers-3/`).
  FSL's rotation derivatives (§4.3, §4.6, §4.7: forward differences of single-precision rotation
  matrices; **FSL-201**, rule A after measurement: the exact derivatives by default, FSL's with
  `reproduce`, in the replica since 2026-11-03), the refit between levels (§4.4: the two
  wrap-around second differences, weight 0.005, at every `--warpres` change; kept, rule C), the
  quadratic bending energy at an even knot spacing (§4.6), `lsr`'s matrices built row by row and
  its folded lines (§7.4; kept, rule C), FSL-133's read at the wrap-around (§7.4, §10.1), FSL-134's
  field in another shape (§7.3), the resampling of axes shorter than 15 voxels (§4.5, §9.4: the
  prefilter's truncated periodic start), FSL-145's single coefficient (§4.2), FSL-146's mechanism
  (§4.6); every convention the original measured confirmed (§13 row 8). **For topup's clean-room
  original (its next part), what the answers let it close:** (1) FSL-201: `reproduce` with §4.6's
  forward difference and the default's notice: the record's 9 expected divergences of finding 14
  (`opt/levels`, `opt/scg-movements`, `opt/four-volumes`, `opt/zero-readout`, `cfg/b02b0`,
  `cfg/b02b0_2`, `cfg/b02b0_4`, `cfg/b02b0_7T`, `difftest/fsl145-short-axis`) and the trial's
  4,462 cases, in `reproduce`; (2) §4.6's sampling of the bending energy: the 47 quadratic-spline
  random cases (reproducer 10753); (3) §7.4's matrices row by row: the 8 `lsr` cases on lines whose
  length divides 2^32 (reproducer 8089); (4) FSL-133 with `reproduce`: `at/lsr-long`,
  `at/vb2d-long`, `at/vb3d-long`, `at/vb4d-long` and the trial's 484 cases; (5) FSL-134's other
  shape with `reproduce`: `at/jac-other-shape` (refused today in both modes; a parity case once
  reproduced); (6) §4.5's prefilter on short axes: the short-axis random cases (case 7283) and
  the vb transients on small images; (7) FSL-145's `reproduce` exactly: one coefficient when the
  axis is shorter than one spacing, the general count at a spacing of 2 for axes of 2 and 3
  voxels, FSL's error when a level refits a single coefficient, and applytopup reading a
  coefficient file of either count by default, as the replica's; (8) to check: whether its
  FSL-146 trigger leaves out a volume encoded along an axis whose read-out time is 0 (FSL's
  gradient leaves out only the volumes whose encoding along the axis is 0).
- **topup's replica: applytopup's `lsr` is 8 times slower than FSL** (3.47 s against 0.42 s on
  ds005040's pair, its files FSL's byte for byte; `docs/benchmarks/fsl-topup.md`).
- **topup's clean-room original: FSL's own band on every case** (the final validation phase): of
  its record's 92 passing cases, 81 are open on the site because only fMRIPrep's calls and the
  thread cases have a band of FSL's; FSL's run with its read-out times changed in their last bit
  (the suite's `_random_band`, through the oracle cache: about 80 runs of seconds) would classify
  them. Its record's notes still say that options the original does not take are skipped (0 are):
  the suite's text, to correct when the record is next made.
- **topup's clean-room original: file names on Windows** (2026-11-03, seen at the merge; **done**
  2026-11-03 by fslmaths' clean-room part, relay 6: `cli::common::name_start`, `\` a separator on
  Windows only in topup's default stem, its configuration's directory test and applytopup's
  extension test, unit tests under a folder whose name holds a dot): the replica
  treats `\` as a directory separator on Windows only (ccd50d0, `docs/api/fsl-topup.md`); the original
  splits `--imain`'s default output stem and the configuration's "has a directory" test at `/` only, and
  applytopup's "has an extension" test treats `\` as a separator on Linux and macOS too, where FSL does
  not. The golden and Rust tests name files relative to the case's directory, so CI does not meet it.
  For a clean-room agent (the next clean-room part): one rule for both programs, as the replica's, with
  a test under a temporary folder whose name holds a dot.
- **The site's speed charts: FSL's dots hidden under the replica's** (FLIRT's and topup's pages, where
  the replica runs at FSL's speed): a legibility fix in the chart's drawing (offset or outline).
- **topup's replica: what is not replicated yet** (2026-11-03): applytopup's restorations of lines
  of fewer than 8 or more than 256 voxels along the phase encoding, and band systems that need
  LAPACK's blocked band factorisation, are refused in both modes with a message (FSL's arithmetic
  there follows other OpenBLAS and Armadillo paths: `docs/api/fsl-topup.md`, "Not yet"); to do when
  a real call needs them. FSL's own thread band on fMRIPrep's real call is large (ds005040 at
  `--nthr=4`: the field up to 3.2 Hz inside the brain, FSL-116; the site's page): a candidate for
  an upstream report (`docs/upstream-reports/`), the user decides.
- **fslmaths and fslstats: follow-ups** (2026-11-03, the replica part):
  - **GLIBC-07 by the `cbrtf` route** (for the orchestrator to schedule, a small part as GLIBC-05
    was): glibc 2.35's double `sin` (0.12 % of image intensities one unit in the last place), `cos`
    (0.085 %), `tan` (0.21 %), `asin` (0.16 %), `acos` (0.071 %), `atan` (0.083 %) and `sincos`
    (0.148 % of the FFT's twiddle angles; one build) are IBM's code in glibc (LGPL): a behaviour
    spec, then a clean-room implementation of glibc's bits in `larmorx_core::math`, for fslmaths'
    `reproduce` (the record's `fn/double-tan` and the permanent case `fft-twiddles-glibc-sincos`
    then pass bit for bit). "glibc's bits" means the FMA builds (decided 2026-11-03; the other
    build differs, a variant to note); `exp`, `log` and `pow` already reproduce them.
  - **The clean-room originals of fslmaths and fslstats**: done 2026-11-04 (item 7), with
    FSL-187's switch and notice in BET's and FLIRT's clean-room originals (their defaults
    unchanged; no record input reaches it).
  - **Their follow-ups** (2026-11-04; the first one found, the direct sum in double precision
    where FSL takes its FFT, findings 31, done in the part's relay 7b with the record re-made):
    (1) the original's `reproduce` gives glibc's `exp`, `log` and `pow` bits
    as §19 asks (Arm's Optimized Routines, MIT: a port into `larmorx_core::math`), today
    correctly rounded in both modes; (2) correctly rounded `erf` and `erfc` in
    `larmorx_core::math` (the original's `-ztop` uses the `libm` crate's, within rule 5);
    (3) larmorx-io's parallel gzip writer holds two 256 KiB blocks and their outputs per worker
    (about 3 MiB each): 4D outputs at 32 threads exceed FSL's memory allowance (one block per
    worker would halve it); (4) runtime SIMD dispatch in `larmorx-fsl` (an audited `unsafe`
    module: 1.26 to 1.34 times on the large convolutions, measured with an x86-64-v3 build);
    (5) the replica's `-bptf` is 2.9 times slower than FSL (5.26 s against 1.82); (6) the replica
    follow-ups FSL-191, FSL-192, FSL-193 (its default to correct, `reproduce` FSL's); (7) the
    replica suite's `fsltest/` cases (FSL's own tests, in its source tree) against the original
    at a merge (a check outside the clean room), if wanted; (8) the spec questions of the
    part's handoff (13, 14, 20, 26 to 38) for a spec-answers part.
  - **goodvoxels with a real ribbon:** the goodvoxels calls ran with a stand-in for the cortical
    ribbon (the segmentation's grey matter); re-check them once an fMRIPrep oracle run with
    FreeSurfer's reconstruction exists.
  - **zstd and bzip2 inputs are decompressed whole** before they are read by the FSL replicas'
    reader (gzip and raw images stream since 2026-11-03): memory about twice the image for those
    formats; a streaming decoder when a real call needs it.
  - **D13 notices in chains:** a correction is named per operation, so a chain whose later
    operation removes an intermediate correction's effect (a threshold, `-bin`, `-restart`, a
    reduction) names it without a visible change (the trial's 595 notices, triaged as explained);
    FSL-151's output rounding is named where both modes store the same value. Precise notices
    would need the chain's final comparison: decide whether it is worth it.
  - **Three `validation/tests` depend on the caller's environment:** inside `boxed.sh`, or with
    `LARMORX_ORACLE_CACHE_MODE` set, three tests of the validation package fail on their own
    nested box or cache mode (they pass unboxed, without the variable): they should set or clear
    what they need themselves.
  - **The probe-only feature `probe-twiddles`** of `larmorx-nc-core` (the FFT's twiddle factors
    read from a file, `validation/probes/fsl-avwutils-lib/check_fft_case.py`) is never built by
    the gates: its hand lint is in the probe's README; consider a gate line if the FFT changes
    again.
- **Random testing's permanent cases record the machine's paths** (2026-11-03, found at the merge of FAST's
  clean-room original): a case's `found.detail` keeps the harness's traceback, with the checkout's absolute path
  (`validation/difftest/cases/fsl-fast-original/zstd-outputs.json`, replaced by `<workspace>` at the merge). The writer
  should replace the workspace and the home directory by `<workspace>` and `~` itself; the existing cases checked once.
- **FAST's `-S 9` and more** (2026-11-02, the orchestrator's decision): refused in both modes with a
  message (`larmorx-nc: error: fast: -S <n>: more than 8 channels are not replicated ...`, exit 1).
  Replicating them needs OpenBLAS's aligned `iamax` path (9 to 16 channels: chosen by the column's
  16-byte alignment, which follows from Armadillo's buffers), its 8-way `ddot`/`daxpy` kernels beyond
  16 and LAPACK's blocked LU beyond 19; practically unused. The clean-room original may segment
  them (its thresholds need not OpenBLAS's bits): a decision for its part (`specs/fast.md` §9.3).
  Decided (2026-11-03, rules A-D: the input makes sense, FSL segments it): the clean-room original
  segments any number of channels with its own LU.
- **FAST's clean-room original: two speed steps left** (2026-11-03, `docs/benchmarks/fsl-fast.md`):
  a batched `expf` for the initialisation's single-precision likelihoods (19 × 3 per voxel, about
  0.2 s of 0.89 s on one thread on ds005003) and the MRF update's three `exp` per voxel batched inside
  the serial sweep (about 6 % of the sweeps); each must keep the scalar functions' bits (a randomized
  check like `exp_in_place`'s).
- **FAST's single-channel variance: decided, FSL-202** (2026-11-03, the spec answers part: rule D
  measured, then A; `specs/fast.md` §4.4, §9.1, §11): the default squares the mean of a
  double-precision quotient in double precision; on seven cases (fMRIPrep's three calls among them)
  it lies 0 or 1 label voxel from FAST computed with its class statistics in double precision
  throughout, where FSL's lies 1 to 844, and its own last-bit band is 0 voxels where FSL's is 3 to
  807 (most of FSL's band is this cancellation). In the replica since 2026-11-03 (`reproduce`
  FSL's); it changes the default outputs of fMRIPrep's FAST call (ds000005 `_seg` 54 and
  `_mixeltype` 112 voxels). The clean-room original follows in its next part (below).
- **FAST's spec questions: answered** (spec writer, 2026-11-03; `specs/fast.md`, lines marked
  "(spec writer, 2026-11-03)"; §12's table names each answer's section, whether the observation was
  right, and its model, `validation/investigations/spec-answers-3/`): the 21 questions and one
  found, FSL-203 (a NaN first voxel empties the mask: rule A, in the replica since 2026-11-03).
  **For FAST's clean-room original (its next part), what the answers let it close:** (1) FSL-202
  and FSL-203 by default, named, `reproduce` FSL's; its record then holds the default to the
  replica's default (a narrow band) and `reproduce` to FSL (FSL's band, as now: §11); (2) §4.1.1:
  `_restore` is NaN at NaN voxels unless the rule for negative values takes its second branch
  (the original writes 0 always); (3) §7: an abort loses the standard-output lines not yet flushed
  (earlier iterations' k-means lines; the partial-volume heading with `-n 0`): the `-v` cases that
  abort, and the trial's singular k-means case, print no k-means line before the abort; (4) §5.3:
  case 1840's `MEAN -nan` (a class without weight zeroes every posterior; the bias weights 0 / NaN
  make the field and then every mean NaN); (5) §10's pipeline of planes with a lag of 2 is FSL's
  in-place order exactly (the original's already; confirmed); (6) §5.8: several channels' one-step
  near ties of the partial volumes are decided by the statistics' last bits: expected, the
  thresholds unchanged (§11); the noise-free and rank-deficient random cases stay explained
  (§8.6), `multi/s5` stays (ill-conditioned).
- **FAST's degenerate multi-channel statistics** (2026-11-03, random testing 1840, open): with two
  channels and four classes whose statistics degenerate, FSL prints `MEAN -nan` from the second bias
  iteration where the clean-room original prints `MEAN 1` (a class weight of 0); the images agree.
  Answered (spec writer, 2026-11-03, `specs/fast.md` §5.3): a class without weight zeroes every
  posterior, the bias weights 0 / NaN make the field and then every mean NaN; for the clean-room
  original's next part (above).
- **FAST's multi-channel initialisation and bias field: measure, then decide (rule D)** (2026-11-02):
  the k-means starts each channel's class means from that channel's own ascending percentiles and
  assigns voxels by the square of the summed differences, so channels of opposite contrast can start
  from empty classes (FAST then ends with every voxel in class 1 and a NaN bias field, exit 0:
  `multi/s2-opposite-contrasts`); and the multi-channel bias field is estimated from the corrected
  image but replaces the previous estimate instead of refining it, with no mean removed and other
  kernels than the single channel's (`specs/fast.md` §9.2). Measure a channel-1-ordered initialisation
  and a refining field on real T1w and T2w pairs (the data panel's) before deciding A or C; both stay
  FAST's in both modes until then.
- **FSL's warp-field libraries** (warpfns, basisfield; 2026-11-01): needed by `applywarp` and `fugue`
  (and so `epi_reg`), and by FLIRT's coordinate programs' `-warp` (refused with a message until then):
  ported with the first tool that needs them, then `-warp` added to `img2imgcoord`, `std2imgcoord` and
  `img2stdcoord`.
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
    **The 17 that fit the new main machine ran on 2026-10-28 and 29** (one at a time, 60 GiB and
    48 GiB of swap; `oracles/difftest/2026-10-25/run-heavy-here.sh`, `run-heavy-here-2.sh`): 13
    agree (N4 #650 after 17.6 h), 2 notices that change no value (ResampleImageBySpacing #1278,
    ANTS-32; N4 #473, ANTS-35), and ResampleImage #556 and ResampleImageBySpacing #938 exceed
    60 GiB (their comparison holds both outputs: about 110 GiB, later). No difference. The run
    stopped once starting a case (a `PermissionError` writing the case's cgroup `memory.max`; no
    system change; the restart ran the last four without it): if it recurs, look at
    `difftest/limits.py`'s leaf creation. The 8 beyond this machine wait for the lab's server.
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
  - AFNI-75's guard: in both implementations of 3dUnifize (the clean-room original's since
    2026-10-30, `specs/3dUnifize.md` §9.1, the same words as the replica's; the §10 cases in the
    afni-unifize suite, `guard/epi-<voxels>`, the permanent case
    `difftest/epi-ball-beyond-any-machine-original`; golden `cli.afni.3dUnifize.guard`).
- **Random testing, more**: the data panel's real crops as inputs; BET in the nightly random
  testing (its descriptor `fsl-bet` is registered; its random-testing panel goes on the site after
  the first night that includes it); nights on a schedule (cron, boxed) once the machine is free
  at night.
- **Memory, the rest** (the rule as refined 2026-10-27: similar, at most 10 % above the original plus
  8 MiB): antsRegistration, antsApplyTransforms, Atropos, N4, DenoiseImage and the image programs
  measured on the panel's largest inputs and kept at or below ANTs
  (`docs/findings/ants-image-programs.md`, "Memory on the data panel's largest inputs");
  **antsAI and antsAffineInitializer: done** (2026-10-28: the images in float, 95 and 91 MiB on
  the head against ANTs' 92 to 96 and 89, 676 against 688 on the 7 T; the test's strict xfails
  removed); the AFNI programs, mcflirt and `resample_series` remain to
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
- **antsAffineInitializer's speed: done** (2026-10-28; on the new machine, where ANTs takes 33.5 s
  on the head and 252 s on the 7 T): `reproduce` at one thread 40.0 s and 254.5 s (the ITK-21
  notice's second search; the search itself takes 0.6 of ANTs' time), the default mode 20.9 s and
  124.7 s, eight threads 8.7 s and 51.6 s (`reproduce`). A further gain at one thread would need
  SIMD over a batch's points (each point's operations unchanged): not attempted.
- **The sampled Mattes path for antsRegistration** (a possible follow-up): `evaluate_sampled` is
  taken only for float-stored images (antsAI, antsAffineInitializer); antsRegistration's sampled
  Mattes stages (`Regular`/`Random`, no fixed transform, a linear stage) could take it too, with
  its own records re-made.
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
  for antsRegistration and the HDF5 files say their benchmark is pending until the
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
- **3dAutomask's clean-room original, the panel's cases: done** (2026-10-29): AFNI reports the
  obliquity of its own header extension's 7-digit matrix (AFNI-10); AFNI on the inputs without the
  extension prints the original's angle, and the parity suite now explains such messages by
  AFNI-10 (22 of 22 fMRIPrep cases pass).
- **3dUnifize's replica memory: done** (2026-10-30): the replica holds what AFNI holds (`-T2` on
  160×192×160: 115.2 MiB through `python -m larmorx`, 103.8 through the binary, against AFNI's 99.1;
  `-EPI` on 64×64×40×160: 120.8 and 110.4 against 109.2; it was 164.8 and 320.9), results unchanged
  bit for bit (the standard record re-made, compare_records: no change); the two strict expected
  failures removed (`validation/investigations/specs-memory/README.md`).
- **3dUnifize `-EPI`'s `cbrtf`: decided by the user (2026-10-30); the clean-room half and the
  replica's half done the same day:** the default keeps the correctly rounded `cbrtf` (CORE-MATH,
  the more accurate: "if Larmorx is already most accurate then the clean room library should be
  for the reproduce mode"), and `reproduce` uses `larmorx_core::math::cbrtf_glibc`, a clean-room
  cube root with glibc 2.35's bits (written from `specs/cbrtf.md` by an implementer who never read
  glibc; proved equal to the system's `cbrtf` on all 2^32 inputs, NaN bits included, and on the
  spec writer's 4096 block hashes: `validation/investigations/cleanroom-cbrtf/`). **Done in the
  clean-room original:** `-EPI`'s radius takes it with `reproduce`; the default names GLIBC-05
  where glibc's last bit changes the ball within the grid or the printed radius (both radii in the
  notice); the parity cases `glibc-05/epi-*` (AFNI 26.3.05's bytes in `reproduce`; the default AFNI's
  with correctly rounded functions preloaded). **Done in the replica (2026-10-30):** its `-EPI` radius takes `cbrtf_glibc` with `reproduce` and
the correctly rounded cube root by default, with the clean-room original's GLIBC-05 notice word
for word (its last-bit warning gone); the `glibc-05/epi-*` cases in its suite (5 of 5 pass:
`reproduce` AFNI 26.3.05's bytes, the default AFNI's with correctly rounded functions preloaded);
`specs/cbrtf.md` §7.3 (`cbrtf-measure --glibc-identity`: 0 of 2^32 differ from the glibc
translation, NaN bits included) and §7.6 (the replica's `reproduce` radius glibc's on every
float32 voxel volume, so no radius, message or ball differs from AFNI's); record re-made
(compare_records: only the 11 new cases, GLIBC-05's 5 and AFNI-75's 6 guard cases, which the
replica record had not run before); golden `3dUnifize.EPI.glibc05(.reproduce)`.
  **§7.2 corrected (spec writer, 2026-10-30):** the 2026-10-23 program's hash starts from
`1469598103934665603`, FNV's offset basis with its last digit missing; with §7.1's start the
whole sequence is `335d1fb5fb714c95`, with the program's `c679cb92d4402e33`, for glibc's
results and `cbrtf_glibc`'s alike.
- **The Python entry point's own memory: decided by the user (2026-10-30), done the same day:**
  `tests/parity/test_memory_peak.py` allows a call through `python -m larmorx` 1.10 × the
  original's peak plus the peak of `python -m larmorx --version` measured the same way, in the
  same environment (13.0 to 14.0 MiB here: the interpreter and the Rust core); the standalone
  binary's calls keep the 8 MiB (`allowance`). 3dAutomask's strict expected failure is removed:
  18.7 to 19.3 MiB in two runs against AFNI's 7.4 to 7.9 and the entry point's 13.0 to 14.0
  (allowed 21.6 to 22.7). The whole file: 30 of 30 pass (2026-10-30, larmorx `9ec1a7e` plus the
  test's change, committed in `930a42f`; log `oracles/logs/cleanroom-cbrtf-memory-2026-10-30.log`).
- **The AFNI evidence pages' entries** (2026-10-29): the figures cache's AFNI entries were computed
  with the `larmorx-gpl` program; their replica runs could be recomputed in-process at the final
  validation phase (the records show the same files; the computes now run the in-process replica).
- **CI's `gpl` job name** (2026-10-29): it runs only `crates-nc` now; renaming it (`nc`) would change
  the check names the watchers count (kept as is).
- **The resample-series step and SCI-05: done** (2026-10-30): SCI-05's cause found (a NaN
  coordinate's filter index overflows for orders 2 to 5, so SciPy reads memory before the array;
  only the NaN's bits vary, in SciPy 1.18.1 as well: register SCI-05,
  `validation/investigations/replica-side/`); the step's digests count NaN values alike and the
  shared releases' cases run twice (a comparison varying between them is reported as varying);
  the record re-made from a clean commit, two runs give the same step detail (two records from bf26b71 identical case by case, the same step line; against 2026-10-29's no status change, 9 comparisons of nearest-order2 to -order5 now identical, their only change having been SCI-05's NaN bits).
- **The parity smoke tier in parallel: done** (2026-10-30, the user: "make sure everything is
  done properly"): `scripts/orchestration/parity_smoke.py` runs `tests/parity` on 16
  pytest-xdist workers, then `test_memory_peak.py` and `test_thread_pools.py` alone
  (`tests/parity/conftest.py` `SERIAL`); the same marker for `record_golden.py`; `--serial` the
  fallback. Proved on one commit and build: the per-test outcomes of a serial and a parallel run
  identical (2,600 of 2,600 tests with the same outcome, all passed in both; 1,574 s serially, 951 s in parallel: 291 s on 16 workers, then 660 s for the 107 memory and thread tests alone, of which the memory peaks take about 626 s; commit 7582eae), `validation/investigations/replica-side/`. `verify-merge.sh`
  step 4 and `record_golden.py` use it (`--serial-parity`); verify-merge's `--suite`s run side by
  side (`run-suites.sh`; `--serial-suites` the fallback), proved on two suites
  (`afni-automask --implementation replica` and `ants-smooth-image`: the same records case by case, `compare_records.py --against`; 13 s one after the other, 8 s side by side, served from the oracle cache). pytest-xdist 3.8.0 (with execnet 2.1.2) added to the shared `.venv` and
  the lock file.
- **The shared environment's numpy and SciPy** (2026-10-29): keep the shared `.venv` at numpy
  2.3.5 / SciPy 1.15.3 until the final comparisons with fMRIPrep (`docs/parts/2026-10-29-version-numerics/handoff.md`
  §13: nothing else needs the move, it is the step's baseline, and antspyx and smriprep ask for
  numpy < 2.5); move it then, with `validation/requirements-lock.txt`.
- **FSL's OpenBLAS core on other machines:** the final validation phase runs FSL's records on the
  old laptop (AVX2: OpenBLAS picks Haswell by itself) as a check of FSL-64's pin.
- **The stage comparisons' FSL** (`stages/runners.py`, `stages/nodes.py`) do not pin the OpenBLAS
  core (they compare with fMRIPrep 25.2.6's container): pin it at the final comparisons if FSL
  6.0.7.23 is used there.
- **Affines mixing NaNs of both signs, or holding infinities, in mcflirt's decomposition into
  rotations** (85 of 200 probe affines differ from FSL 6.0.7.23 in a NaN's sign, 148 against
  6.0.7.17): the replica follows the source's order; FSL-14's matrices are all one NaN (no recorded
  run meets the mixed case). Measure with the probes (`validation/probes/fsl-mcflirt-affine-trace`)
  if a real case ever does.
- **The spec's in-plane centre and the clean-room original's remaining last bits** (found
  2026-10-29, `docs/findings/mri-hmc-original.md`): in the in-plane mode mcflirt rotates about the
  padded test volume's centre of mass (observed with `-verbose 20` traces; the original does so
  since 2026-10-29): the spec writer adds it to `specs/mcflirt.md` §12. With `-fudge -stages 1`
  8 of 30 random phantoms still part from mcflirt's evaluated matrices after 19 to 224
  evaluations, in the 5th or 6th decimal, as `normcorr` does (spec §17, open question 1): one
  cause shared by every cost, for a later clean-room part with the traces of
  `validation/investigations/cleanroom-align/mi_trace.py`.
  Also for the spec writer (observed by the clean-room implementer, 2026-10-29, `validation/investigations/cleanroom-align/fsl_io_probe.py`): §4.1 and §10.1, `-out` drops any image
  extension, the new ones included, before FSLOUTPUTTYPE's is added, and `-reffile` is completed
  like `-in`; the unknown-type message comes after the estimation and the `.mat` and `.par` files
  (exit 1); FSL-60's renaming applies to a pair's header only, never to the `.mat` directory or
  the `.par`/`.rms` files; the completion errors' words (`Multiple possible filenames detected for
  basename: <name>`, `No image files match: <name>`; signal 6). Open question 4 can record:
  settled for `mutualinfo` and `normmi` by appendix B.
- **The head-motion correction's FSL-01 notice in the in-plane mode** (found 2026-10-29 by the
  clean-room alignment's random-testing trial, `mri-hmc:20261029:45`): with `-cost corratio` and a
  separate reference on a thin slab, FSL-01 is named in `reproduce` only, because the default's
  FSL-03 correction leaves the previous volume's stage-1 matrix at the identity (so starting the
  last volume from it changes nothing). Decide by the notice rule (named where the two modes'
  results differ): name FSL-01 in both modes where mcflirt's chain would have changed the last
  volume, or declare the interplay; a clean-room agent.
- **The replicas follow Armadillo 14.2.1's LAPACK inverses: done** (2026-10-29, the version update's
  part 3: `crates-nc/larmorx-nc-core/src/lapack.rs`; mcflirt 181 of 181, BET 130 of 130).
- **The version update, what changed in the numerics: done** (2026-10-29): every file read and
  classified (`docs/findings/scipy-ndimage.md`, `numpy-blas.md`); one change of results (SciPy's
  reflect initialisation, SCI-03), followed.
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
