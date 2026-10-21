# Status log

The project's step-by-step status as CLAUDE.md kept it until 2026-10-20, moved here verbatim
when CLAUDE.md was reorganised (the user: keep everything well organised). The current state is
in CLAUDE.md ("Where things are"), the plan in [next-steps.md](next-steps.md), the decisions in
[decision-log.md](decision-log.md); each merge's details are in its commit message.

### Status (2026-10-12)
- **Phase L0 done:** a buildable, tested skeleton, CI green on all six targets.
- **Phase L1 in progress:**
  - `Image`/`Affine` and NIfTI-1/2 I/O: validated against nibabel (`docs/validation/nifti-io.md`) and benchmarked (`docs/benchmarks/nifti-io.md`).
  - **antsApplyTransforms (A1) done:** `lx.ants.apply_transforms`, `larmorx ants antsApplyTransforms`, `lx.transforms`, crates `larmorx-transform`, `-interp`, `-ants`. It is validated against ANTs itself: 83 cases, 63 of 79 bit-identical (`docs/validation/ants-apply-transforms.md`), and benchmarked (`docs/benchmarks/ants-apply-transforms.md`).
  - **antsApplyTransforms `--float 1` done** (2026-10-11): ANTs' single-precision pipeline, computed exactly as ITK's float instantiation defines it (`precision="float32"`, `ApplyTransformsOptions::precision`; ITK's computation type is a type parameter, `larmorx_core::ItkReal`): float points, transforms built and inverted in float (vnl's float SVD inverse, CORE-MATH `sinf`/`cosf` for Euler), float field lookups, float coordinates in every interpolator, text transforms and `-f` read as float, the output clamped to the float range (`docs/findings/ants-float.md`). Parity, standard tier: 166 cases (every case again with `--float 1`), 158 compared: 144 bit-identical to the unmodified ANTs (all 81 `--float 1` cases), 158 with CORE-MATH preloaded (the preload now also replaces `sinf`/`cosf`/`sincosf`). All 14 of fMRIPrep's `--float 1` outputs of the ds000005 run bit-identical to fMRIPrep's own (`docs/validation/fmriprep-stages-float.md`). Gaussian interpolation sped up (vnl's log Γ(1/2) once, weights on the stack): the template-prior step 10.5-11.3 s against ANTs' 12.1-13.9 s on one thread (busy machine; benchmark report pending). Register: ITK-10 (a versor that fails to load in float, reproduced).
  - **3dTshift clean-room original done** (2026-10-10): `lx.afni.tshift`, `larmorx afni 3dTshift`, crate `larmorx-afni`, written from `specs/3dTshift.md` and black-box runs only. 236 parity cases against AFNI 25.2.09 (oracle built by `scripts/build_afni_oracle.sh`): 217 agree, 158 bit-identical, 0 failures (`docs/validation/afni-tshift.md`). Since 2026-10-12 (completing 3dTshift, part 2b) it also follows spec §6.1-6.5 and §7.1: AFNI's geometry (the plumb test, the cardinal matrix, the quaternion with x87 steps emulated, `larmorx_afni::x87`), the slice fields of copies, the output-name rules, AFNI's global options and environment variables, AFNI's messages; 311 cases: 279 of 279 compared agree, 176 bit-identical, 60 of 60 global/environment cases identical in exit status and messages, 3 expected divergences (as the replica's); records from `4c6cd3a`.
  - **3dTshift replica done** (2026-10-10): `larmorx-gpl afni 3dTshift`, crates `larmorx-gpl-afni` and `larmorx-gpl-cli` in `crates-gpl/` (GPL-3.0-or-later, a separate Cargo workspace). 217 of 217 compared cases bit-identical to AFNI on 2026-10-10; on 2026-10-12, after D13 and AFNI's global options, 279 of 279 compared cases of 311 bit-identical in `reproduce` mode, headers and messages included (`docs/validation/afni-tshift-replica.md`). It carries a translation of glibc 2.35's `sinf`/`cosf` (LGPL-2.1-or-later).
  - **`implementation="auto"` done** (2026-10-11): `lx.afni.tshift` and `larmorx afni 3dTshift` run the replica when the `larmorx-gpl` program is installed, else the original; `implementation=` / `LARMORX_IMPLEMENTATION` choose; `Image.implementation` records what ran. The replica always runs as a **separate process** (files in a temporary directory), never imported, so the licences stay separate (`docs/licensing.md`, "Choosing at run time"). Registry, search (`LARMORX_GPL_BIN`, the Python scripts directory, `PATH`) and choice: `larmorx_cli::replica`, used from Python by `larmorx/_replica.py`. `larmorx-gpl` is a binary-only wheel (`crates-gpl/pyproject.toml`), installed by the extra `larmorx[exact]`; neither is on PyPI yet. How to add a replica: `docs/architecture.md`.
  - **One-shot BOLD resampler done** (2026-10-10): `lx.transforms.resample_series` and `lx.ndimage`, a replica of fMRIPrep's `ResampleSeries`, nitransforms 25.1.0 and SciPy 1.15.2's spline interpolation. All 136 compared cases bit-identical (`docs/validation/resample-series.md`). Its benchmark report is pending.
  - **Head-motion correction done, `experimental`** (2026-10-10): `lx.mri.hmc`, `larmorx mri hmc`, crate `larmorx-mri`, clean-room from `specs/mcflirt.md` (oracle runs in `oracles/fsl-6.0.7/mcflirt/`). Closer to mcflirt than mcflirt is to itself on fMRIPrep's command; one case (ds003345, default reference) misses its FD-correlation threshold by 0.0008, so it is not `validated` yet (`docs/validation/mri-hmc.md`). Not bit-exact: cost values differ by 1-2 float32 quanta after interpolation, cause unknown. The mcflirt replica (`crates-nc/`) is not started.
  - **A2 part 1 done** (2026-10-10): the ImageMath dispatcher and arithmetic/intensity ops, `ThresholdImage`, `MultiplyImages`, ITK's NIfTI writer, and the parity harness for ANTs programs run as binaries (`validation/.../parity/ants_programs.py`). Against the ANTs 2.6.5 binaries (`oracles/ants-2.6.5/bin/`), every header is byte-identical and every data output bit-identical except ImageMath `^` (glibc `powf`, ≤ 1 ulp). How to add an ImageMath group: `crates/larmorx-ants/src/cli/image_math/mod.rs`.
  - **A2 part 2 done** (2026-10-10): ITK's recursive and discrete Gaussian filters and median (`larmorx-image`, `Volume` is now D-dimensional), ImageMath `G`/`Laplacian`/`Grad`/`UnsharpMask`, `SmoothImage`, `ResampleImageBySpacing`, `ResampleImage`. Every compared output bit-identical to ANTs 2.6.5, headers included, in 2D, 3D and 4D; 1.6-7x faster on one thread.
  - **A2 parts 3-4 done** (2026-10-11): binary and grayscale morphology, FillHoles, PadImage, connected components, GetLargestComponent, Danielsson and Maurer distance maps, ExtractContours, ThresholdAtMean, ReplaceVoxelValue. ImageMath: 327 cases, 296 of 301 compared outputs bit-identical (the rest explained), all headers byte-identical. Every ImageMath op that fMRIPrep, sMRIPrep and sdcflows call is ported. Left of A2, to do with registration (A4): the shrink filter and histogram matching.
  - **Golden tests in place** (2026-10-11): 86 cases cover every CLI tool, ImageMath op and public Python function, in both packages (`tests/golden/`, `crates-gpl/larmorx-gpl-cli/tests/golden.rs`, `docs/validation/golden.md`). Record checksums with `scripts/record_golden.py` on Linux x64 after parity passes. `lx.transforms`' 4x4 products and inverses run in Rust in numpy's OpenBLAS Haswell order (`larmorx_transform::openblas`, 2026-10-11), so they no longer depend on the platform's BLAS (`docs/findings/numpy-blas.md`). **CI run 38100125479: every golden case bit-identical on all six platforms.**
- Conventions every tool follows are in `docs/architecture.md`.
- **Findings about the upstream tools** go in `docs/findings/`. Keep adding to it while porting and validating: the behaviour, its upstream file and line, how it is known, and what larmorx does. Every defect found also gets an entry in `docs/findings/upstream-defects.md` (its class, evidence, effect on fMRIPrep users, what larmorx does, and the D13 gap); its top lists the defects larmorx still reproduces by default.
- **Decided 2026-10-09:** transcendental functions are correctly rounded (CORE-MATH ports in `larmorx_core::math`; rule 5). See `docs/findings/platform-math.md`.
- Development moved from the user's Mac to this **Linux machine**. It is always on, so it is suited to long oracle runs and benchmarks.
- **Decided:** names (D1), licence Apache-2.0 (D2) and distributions (D3), see above.
- **Pending decisions** are in PLAN.md §16. Until the user decides, use these defaults:
  - neutral names for re-implemented (FSL-/Workbench-like) tools
  - Python ≥ 3.12
  - FreeSurfer v8.2.0 as the port target
  - the M1 "preview" milestone first
- **D7 decided (2026-10-09): larmorx is not commercial.** FSL and MSM binaries may be run as validation oracles under their non-commercial licences.

- **A3 part 1 done** (2026-10-11): `N4BiasFieldCorrection` (`lx.ants.n4_bias_field_correction`), with ITK's shrink filter, B-spline fitting and vnl's FFT. N4 follows the C++ source (rule 8). Against the unmodified ANTs program at one thread: 29 of 45 cases (35 of 54 outputs) bit-identical; the rest differ only through glibc's not-correctly-rounded `expf`/`logf`/`exp` (45 of 45 bit-identical when those are replaced by correctly rounded ones, and 45 of 45 against fMRIPrep's own ANTs 2.6.2 the same way), NCC ≥ 0.9999999. 6.4-7.4x faster on one thread.
- **A3 part 2 done** (2026-10-11): `DenoiseImage` (`lx.ants.denoise_image`), the adaptive non-local means filter of ITK's remote module AdaptiveDenoising (commit `24825c8d`, `tags/ITKAdaptiveDenoising`), box mean and variance, resampling with float coordinates. Against the unmodified ANTs program at one thread: 7 of 40 cases bit-identical; the rest differ only through glibc's `expf`/`exp` in a few voxels (40 of 40 bit-identical with CORE-MATH preloaded), at most 2.2e-4 of ANTs' own thread band. **ANTs' DenoiseImage changes a lot with its thread count** (up to 271 on a T1w whose values reach 1,850; from 8 threads also from run to run), so fMRIPrep's denoised T1w is not reproducible; larmorx gives the one-thread result on any thread count. 10.7-17x faster on one thread (sMRIPrep's T1w: 13 s instead of 217 s; 2.9 s on 12 threads).
- **Stage-by-stage comparison with fMRIPrep 25.2.6 done** (2026-10-11): `python -m larmorx_validation pipeline-stages --oracle <run dir> --out docs/validation` (`validation/.../stages/`) reads nipype's `command.txt` and `_report/report.rst`, runs larmorx on fMRIPrep's own inputs and parameters for every step it has, compares with fMRIPrep's output as it ran, and takes differences apart by re-running the step in fMRIPrep's image (one thread, CORE-MATH preloaded) and with the ANTs 2.6.5 oracle. Record `docs/validation/fmriprep-stages.md`: 61 of 92 nodes bit-identical, the rest explained by threads, glibc's maths or the ANTs build, except 14 `antsApplyTransforms --float 1` calls (larmorx computed them in double; since 2026-10-11 all 14 are bit-identical, `docs/validation/fmriprep-stages-float.md`); HMC within thresholds. Findings `docs/findings/fmriprep-oracle.md`: fMRIPrep's image ships **ANTs 2.6.2 on ITK 5.4.4**; nipype passes threads as `NSLOTS` (now read by `larmorx ants`); **fMRIPrep's N4 binary follows the C++ source, where the GCC 11 oracle and larmorx's N4 do not** (open decision: which to follow); fMRIPrep's `ResampleSeries` races with multi-threaded OpenBLAS (only the image's `OMP_NUM_THREADS=1` prevents it).
- **D13 switch and first corrections done** (2026-10-11): `upstream_defects="fix"|"reproduce"` (Python), `LARMORX_UPSTREAM_DEFECTS` (command line), `larmorx_core::defects` (Rust), `lx.UpstreamDefectWarning` ([docs/api/upstream-defects.md](api/upstream-defects.md)). Corrected by default: PIPE-03, PIPE-04, SCI-03, AFNI-01/02/03/05/07 (clean-room original), FSL-01/03/05–10/13/14; FSL-02 opt-in and measured (`docs/validation/mri-hmc-normcorr-count.md`: a change about the size of mcflirt's band, no gain against the ground truth), pending the user's decision. PIPE-03 measured on real geometry (`docs/validation/resample-series-jacobian.md`: fMRIPrep's factor off by more than 1 % in 40 % of T1w-space brain voxels). Parity compares in `reproduce` mode (bit-identical as before) and reports a fix pass; golden `*.reproduce` cases equal the pre-D13 checksums. Still to do: the 3dTshift replica's switch, and the replica's sform for metre/micrometre inputs (an expected divergence in its record).
- **Next (user decisions, 2026-10-11):** (1) a stage-by-stage comparison with the fMRIPrep 25.2.6 oracle run, feeding fMRIPrep's own intermediates to larmorx's tools (done, above); (2) then the missing tools, natively: A4 (antsRegistration rigid/affine, antsAI, antsAffineInitializer, histogram matching), A5 (SyN), A6 (Atropos), AFNI's 3dvolreg/3dUnifize/3dAutomask, FSL's BET/FAST/FLIRT-BBR, FreeSurfer's mri_coreg/lta_convert, and the confounds; then larmorprepx. **fMRIPrep oracle:** fMRIPrep 25.2.6 runs through Apptainer as the user, no Docker and no sudo (`scripts/run_fmriprep_oracle.sh`, image `oracles/images/fmriprep-25.2.6.sif`, digest and settings in each run's `RUN_INFO.txt`). First run done: ds000005 sub-01, `--fs-no-reconall`, outputs and working directory in `oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall/`. Never ask the user for their sudo password.

- **D13 part 1 done** (2026-10-12): every ANTs and ITK gap of the register corrected by default (ANTS-03 to -05, -08 to -16, -19, -20, -24, -25, -27, -30, -31, -33, -35, -36, -38, -39, -43; ITK-01, -02, -03, -10), undefined conversions clamped with a warning (ANTS-06, SCI-02), a new entry ANTS-47; failure and garbage cases with a switch (ANTS-29 and ITK-08 byte for byte; ANTS-07, -19, -28 with the undefined memory as zeros). ResampleImage's command line redesigned (ANTS-30: parameters after the interpolator, then the pixel type; `docs/api/ants-resample-image.md`, "The seventh argument"; awaits review). ITK-01 and ANTS-39 warn only where a stored value changes (`larmorx_core::IndexFloor::checked`). Parity in `reproduce` mode unchanged against `oracles/ants-2.6.5` (records from `50509e30b3e8`), plus a fix pass per suite; two former expected divergences now pass (ResampleImage size 1 byte for byte; ResampleImageBySpacing's uninitialised buffer, everything but the values). Notable fix-pass change: ANTS-43 alters 7-8 % of the voxels of sMRIPrep's denoised T1w (median 0.6 to 2.9, within ANTs' thread band). Found: ITK-11 (ITK's Gaussian kernel inaccurate above a variance of about 50 voxels², not D13). Golden: 7 `*.reproduce` cases; the changed defaults re-recorded.
- **D13 part 2a done** (2026-10-11): the shared compatibility switch (`larmorx_core::defects`, Python `upstream_defects="fix"|"reproduce"` and `lx.UpstreamDefectWarning`, CLI `LARMORX_UPSTREAM_DEFECTS`; `docs/api/upstream-defects.md`) and the fixes for PIPE-03/-04, SCI-03, the head-motion original's FSL-* and the 3dTshift original's AFNI-* defects. FSL-02 stays opt-in (`normcorr_count="weights"`). Reports for fMRIPrep (PIPE-02, PIPE-03) are drafts in `docs/upstream-reports/`, not yet filed by the user.

#### Work in progress and queue (2026-10-11, evening)
One agent at a time. **The plan, in order, with scopes and pending decisions: [docs/next-steps.md](next-steps.md)** (keep it current). The queue so far:
1. **Done: `antsApplyTransforms --float 1`** (see the status above; worktree branch `worktree-agent-a689dd2fa349688de`). D13 defaults unchanged (ANTS-33, ITK-01, ITK-10 reproduced).
2. **Done: D13 part 1** (agent branch `worktree-agent-a448db6a134de1e65`, see the status above). For the user: review ResampleImage's corrected command line (ANTS-30).
3. **Done: D13 part 2b** (merged 2026-10-12, `8ab6cc5`; verified by re-running both 3dTshift suites). **Completing 3dTshift** (decision log 2026-10-12): part 1 done (the replica's AFNI-16 default, spec §6.1-6.5; merged `3d2ae09`, verified). Part 2a done (AFNI's global options and environment variables in the replica, spec §7.1; merged `ebc3122`, verified). AFNI-23 fixed by default in the replica, and the Python API taking AFNI settings explicitly (`afni_settings=`, never the environment; user decisions 2026-10-12; merged `d0388dd`, verified). **Part 2b done** (merged 2026-10-12, `cbf5081`; verified: checklist steps 1-4 on the merge, both 3dTshift suites re-run from it, every case's status and checks as in the agent's records): the clean-room original implements spec §6-7 (the geometry, copies, output names, the global options and variables, AFNI-23, `afni_settings`); its 71 expected divergences are down to the replica's 3. Part 2b found two AFNI behaviours, now AFNI-25 (`-ignore` as the last argument crashes; no gap) and AFNI-26 (a failed write exits 0). **AFNI-26's switch done in both** (2026-10-12; the original's by a clean-room agent, branch `worktree-agent-a0313b1bd242cb7eb`, merged `c299afa`; the replica's default then aligned with the spec and the original: an output AFNI's writer cannot open, a read-only file included, is an error naming AFNI-26). **3dTshift is complete:** both implementations 314 cases, 0 failures, the same 3 expected divergences. Known difference left in both (not an AFNI defect): an existing writable output in a directory that cannot be written, with `-overwrite`, which AFNI rewrites in place and larmorx cannot (its temporary file); proposed to the user: fall back to writing in place there, in both implementations (not decided). What 2b was: the same fixes in the GPL 3dTshift replica (replica author, may read AFNI source), plus its bug: it writes a metre or micrometre input's sform unconverted (the two expected divergences in `REPLICA_DIVERGENCES`).
4. **Done: stage comparisons on the field-map runs** (merged 2026-10-12, `c0543d0`, agent branch `worktree-agent-acd8a87c00dd1de09`; verified: checklist steps 1-4 on the merge, ds005040's record re-run from it, identical apart from timings). Nodes bit-identical to fMRIPrep 25.2.6 as it ran: ds000005 75 of 92 (regenerated; all 14 `--float 1`), ds005040 56 of 65, ds005454 55 of 68, ds005752 65 of 78; every other difference explained (fMRIPrep's thread counts, glibc's maths). Slice timing compared at last (the F02 pattern fixed): the replica bit-identical in 4 of 4, the original within thresholds. PIPE-03 confirmed on real field maps (ds005040, MNI: 59.5 % of brain voxels off by more than 1 %). New: PIPE-16 (nipype re-quantizes the field-map N4 to int16; decide for larmorprepx). ds005752's field map is never used by fMRIPrep (no `IntendedFor`), so the direct-`fieldmap` path is still uncovered. Records `docs/validation/fmriprep-stages*.md`.
5. **Pending measurements on a quiet machine:** `docs/benchmarks/ants-n4.md` against the new oracle; antsApplyTransforms after `--float`.
6. **Done: A4 part 1, ITK's HDF5 transform files in Rust** (merged 2026-10-16, `c22d336`; verified: checklist steps 1-4 on the merge, `itk-hdf5` and `ants-apply-transforms` re-run from it, every case as in the agent's records). Crate `larmorx-hdf5` (reader of everything HDF5 1.12 stores in one file; writer replaying HDF5 1.12.1's layout for ITK's calls, object times fixed at 0); `-t x.h5` on the command line; `lx.transforms` read/write without h5py (dropped). `itk-hdf5`: 144 cases, 0 failures, 5 expected divergences; `ants-apply-transforms`: 182 cases, 160 of 174 bit-identical to the unmodified ANTs (174 with the preload). Register: ITK-12 to ITK-17, HDF-01, PIPE-17. **Done: A4 part 2** (merged 2026-10-17, `c6fdd61`; verified: checklist steps 1-4 on the merge, `ants-registration` and `ants-apply-transforms` standard tiers re-run from it, every case as in the agent's records): `antsRegistration`'s linear stages to full completeness (crate `larmorx-registration`, ITK's v4 framework; every option, message and exit status; 2D/3D/4D, double and float); `ants-registration` 166 standard / 175 full cases, 0 failures, 102 bit-identical to the unmodified ANTs, every other difference glibc's maths; larmorx gives ANTs' one-thread result at any thread count; `antsApplyTransforms`' transform outputs (`-o Linear/CompositeTransform/DisplacementField`, 220 cases); HDF5 golden fixtures (14 KB). Register: ANTS-48 to ANTS-51, ITK-18; ANTS-46 explained. **Done: A4 part 3** (merged 2026-10-17, `084b61d`; verified: checklist steps 1-4, `ants-ai`, `ants-affine-initializer`, `ants-registration`, `ants-apply-transforms` standard tiers re-run from the merge, every case as in the agent's records; the agent's two `-dirty` records replaced by these clean ones): `antsAI` and `antsAffineInitializer` to full completeness except `antsAI -b` (blob alignment, refused; requirements in `docs/findings/ants-ai.md`, "Blobs"); 0 failures in every tier; fMRIPrep's ten `init_aff` calls within 1.5e-9 of its own transforms; correctly rounded `asin`/`acos`/`atan2`/`cbrt` (the preload replaces them too; the old preload archived in `archive/2026-10-17-crmath-preload/`); `antsRegistration`'s gradient filter now rounds to float as ITK does (an A4 part 2 bug, found on oblique grids). Register: ANTS-52 to ANTS-60, ITK-19 to ITK-21. **Decided 2026-10-17:** ANTS-54 and ITK-21 corrected by default (to implement first in the next run, then re-record `ants-affine-initializer`); `antsAI -b` later, its own task after A5/A6. **Done: A4 part 4 and the two corrections** (merged 2026-10-18, `f19ffb7`; verified: checklist steps 1-4, `ants-ai`, `ants-affine-initializer`, `ants-registration`, `ants-apply-transforms` standard tiers re-run from the merge, every case as in the records). The agent's run was stopped once by the weekly limit and resumed; it stalled after its last test run, and the orchestrator finished its last steps (records into the docs, golden checksums from `e53126c`). ANTS-54 and ITK-21 corrected by default in `antsAffineInitializer` (switches reproduce ANTs; golden `.reproduce` cases equal the old checksums); `antsRegistration`'s point-set metrics ICP, PSE, JHCT, IGDM to full completeness (ITK's k-d tree, label-image/text/VTK readers, ANTs' intensity point-set filter; `lx.ants.PointSetMetric`, `lx.ants.IntensityPointSetMetric`); correctly rounded `pow` (port of CORE-MATH's `cr_pow`; the preload replaces `pow`/`powf` too); `ants-registration` 224 standard / 235 full cases, 0 failures, every compared point-set case identical to the unmodified ANTs. Register: ANTS-61 to ANTS-70, ITK-22 to ITK-29. **To decide (user):** ANTS-67, ANTS-68, ANTS-70, ITK-24, ITK-29 (all in the point-set metrics, unused by fMRIPrep). **Follow-ups:** `larmorx-interp`'s B-spline prefilter still uses the `libm` crate's `pow` (switch to `larmorx_core::math::pow` with `antsApplyTransforms`' B-spline cases re-run); the records made with the older CORE-MATH preload (`ants-image-math`, `ants-n4`, `ants-denoise`, ...) predate `pow`/`powf`/`asin`/`acos`/`atan2`/`cbrt` in it: re-run when convenient. **Done: A5 part 1, SyN** (merged 2026-10-20, `7f2a61e`; verified: checklist steps 1-4, `ants-registration`, `ants-apply-transforms`, `itk-hdf5` standard tiers re-run from the merge, every case as in the records; `itk-hdf5` re-recorded from it with A4 part 2's 5 fixture cases): `antsRegistration`'s SyN stages to full completeness (`larmorx_registration::syn`, `::field`: displacement fields, composition, inversion, SyN smoothing; every image metric with fields, Demons; point-set metrics in SyN; fields as initial transforms; Warp/InverseWarp, composites with fields, saved states); `ants-registration` 298 standard / 327 full cases, 0 failures; sMRIPrep's whole brain extraction and template registration on the five oracle runs identical to ANTs with the CORE-MATH preload (glibc's `log` otherwise, up to about 1.6 mm); about 5x faster than ANTs at one thread on a 96^3 CC case, bit-identical (informal; benchmark report pending); parity harness `--jobs N`. Register: ANTS-71 to ANTS-74, ITK-30 to ITK-34. **Decided 2026-10-20:** ITK-31 (stale masks) corrected by default (fMRIPrep's brain extraction moves at most 0.18 mm, inside ANTs' own thread band). Follow-ups: the benchmark report; A4's linear stages still use the global thread pool instead of `n_threads` (results unaffected); old `/tmp/lx-ants-inputs-*`, `/tmp/lx-reg-*` temp dirs of earlier agents. **Next: A5 part 2** (BSplineSyN, BSpline, Gaussian and B-spline displacement fields), only when the user says go. **A4** (linear registration, antsAI, antsAffineInitializer, histogram matching), validated against the GCC 14.3 oracle only, to full completeness (decision log 2026-10-12). **Its first part: ITK's HDF5 transform files in Rust.** Found 2026-10-12: `larmorx ants antsApplyTransforms` refuses `.h5` transforms ("HDF5 transforms are read by the Python layer", `larmorx_io::itk_transform`), so the command line cannot take fMRIPrep's composites (only `lx.transforms` reads them, through h5py); `antsRegistration` must also write them (`--write-composite-transform`). Rule 5 rules out the HDF5 C library: a pure-Rust reader and writer of the HDF5 subset ITK writes, checked against ITK/ANTs, h5py and nitransforms. `antsAffineInitializer` added to the oracle (`scripts/build_ants_oracle.sh`, built 2026-10-12 into `oracles/ants-2.6.5-src`). Then A5, A6, AFNI's 3dvolreg/3dUnifize/3dAutomask, FSL's BET/FAST/FLIRT-BBR, FreeSurfer's mri_coreg/lta_convert, the confounds, larmorprepx.

**fMRIPrep oracle runs** (`oracles/fmriprep-25.2.6/<run>/`; done when its `RUN_INFO.txt` has `finished:`):
- `ds000005-sub-01-noreconall`: done; no field maps, no slice timing.
- `ds001600-sub-1-noreconall`: done, anatomy only; its BOLD runs have 5 volumes, which fMRIPrep skips.
- `ds005040-sub-002-noreconall` (PEPOLAR plus slice timing): done, 58 min; its BOLD was processed (10 functional outputs, a PEPOLAR field-map report).
- `ds005454-sub-16-noreconall` (`phasediff`, multiband 4, 280 volumes): done, 1 h 25 min.
- `ds005752-sub-ON01016-noreconall` (direct `fieldmap`): done, 55 min.
All three finished on 2026-10-11 with exit status 0 (`oracles/logs/fmriprep-25.2.6-sdc-chain.log`); their stage comparisons are queue item 3.

# Plan sections of finished parts (moved from next-steps.md, 2026-10-20)

### Where things stand (2026-10-12)

Of the ~25 programs on fMRIPrep's `--fs-no-reconall` path, done and validated:
- ANTs: `antsApplyTransforms` (and `--float 1`), N4, DenoiseImage, every ImageMath operation
  fMRIPrep uses, ThresholdImage, MultiplyImages, SmoothImage, ResampleImage(BySpacing);
- AFNI: 3dTshift, replica and clean-room original, complete;
- FSL: head-motion correction (`lx.mri.hmc`, clean-room, `experimental`);
- fMRIPrep: the one-shot BOLD resampler (`ResampleSeries`).

Stage by stage against fMRIPrep 25.2.6 (`docs/validation/fmriprep-stages*.md`): ds000005 75 of
92 nodes bit-identical, ds005040 56 of 65, ds005454 55 of 68, ds005752 65 of 78; every other
difference explained (fMRIPrep's thread counts, glibc's maths). Not started: `antsRegistration`,
`antsAI`, `antsAffineInitializer`, Atropos, FAST, FLIRT-BBR, BET, 3dUnifize, 3dAutomask, topup,
sdcflows' field-map estimation, the confounds, larmorprepx; FreeSurfer is a separate track.

### 1. A4, part 1: ITK's HDF5 transform files in Rust

**Done 2026-10-16** (agent branch `worktree-agent-a5b5185d32a7fb70f`, to merge): crate
`larmorx-hdf5` (a reader of HDF5's format, and a writer replaying HDF5 1.12.1's layout),
`larmorx_io::itk_hdf5` (ITK's reading and writing, ANTs' messages, ITK-12 to ITK-17, HDF-01),
`-t x.h5` on the command line, `lx.transforms.read`/`write`/`load_itk_composite` in Rust (PIPE-17),
h5py dropped. Validation [itk-hdf5](validation/itk-hdf5.md); API
[transforms-hdf5](api/transforms-hdf5.md); findings [itk-hdf5](findings/itk-hdf5.md).
**Left for later:** antsApplyTransforms' transform outputs (`-o Linear[...]`,
`-o CompositeTransform[...]`, `-o DisplacementField[...]`, refused today: they need ANTs'
linear collapse and `TransformToDisplacementFieldFilter`; the HDF5 writer is ready); reading
data stored elsewhere (external raw files, external links, virtual datasets: ITK's HDF5 follows
them, larmorx refuses them; no transform writer produces them); a fractal heap's indirect blocks in the
writer (a composite of 38 or more transforms is written compactly instead); the read benchmark
against h5py and ANTs (pending a quiet machine).

**Why first:** `larmorx ants antsApplyTransforms` refuses `.h5` transforms (only
`lx.transforms` reads them, through h5py), so the command line cannot take fMRIPrep's
composites; `antsRegistration` writes them (`--write-composite-transform`) and reads them as
initial transforms. Rule 5 rules out the HDF5 C library.

**Scope:** a pure-Rust reader and writer of the HDF5 subset that ITK's `HDF5TransformIO`
writes and reads (the superblock versions it produces, groups, the `TransformType` strings,
`TransformParameters` and `TransformFixedParameters` in float and double, contiguous and
chunked, compressed if ITK compresses), for single and composite transforms, displacement
fields included. Used by `larmorx_io::itk_transform`, the command line and `lx.transforms`
(which can then drop h5py). Spec of the format: The HDF Group's published file-format
specification; the HDF5 library is BSD-style, so reading its source is allowed (record it in
`PROVENANCE.md`).

**Validation:** every `.h5` in the oracle runs read exactly as h5py and nitransforms read it;
files written by larmorx read back by ANTs 2.6.5 (`antsApplyTransforms` gives the same output
as with ANTs' own file), h5py and nitransforms; the bytes compared with ITK's own output where
HDF5 allows (object times may differ; say what differs). Golden cases on all six platforms.

### 2. A4, part 2: `antsRegistration`, the linear stages (done 2026-10-17, merged `c6fdd61`)

Done, with `antsApplyTransforms`' transform outputs and the HDF5 golden fixtures: crate
`larmorx-registration` (ITK's v4 framework), `lx.ants.registration`, `larmorx ants antsRegistration`;
`docs/validation/ants-registration.md` (166 standard / 175 full cases, 0 failures, 102 bit-identical to
the unmodified ANTs, the rest only through glibc's maths; 168 standard, 104 bit-identical, since A4
part 3's gradient-filter correction). Refused, with messages: the point-set
metrics (part 4 below), displacement fields among the initial transforms (A5), MINC transform
files (the oracle cannot read them either). The original scope, kept for reference:

**Scope (full completeness):** the whole command line parsed and checked as ANTs does (every
option; the deformable transforms accepted by the parser and implemented in A5); the
transforms Translation, Rigid, Similarity, Affine, CompositeAffine; the metrics usable with
them (MeanSquares, CC, Mattes, MI, GC, and the point-set metrics if ANTs allows them in linear
stages); sampling None/Regular/Random with ANTs' seeding; fixed and moving masks per stage;
shrink factors and smoothing sigmas (voxels and mm); convergence (iterations, threshold,
window); winsorizing; histogram matching; initial transforms (files, `[file,useInverse]`,
the image-centre and centre-of-mass initialisations); outputs (`0GenericAffine.mat`, warped and
inverse-warped images, composite `.h5`, collapsed outputs, `--float`, verbose output,
`--write-interval-volumes`, save/restore state); messages and failure cases with D13
switches. ITK's shrink filter and histogram matching (A2's leftovers) come with it.

**Validation:** against the GCC 14.3 oracle (`oracles/ants-2.6.5`) only, bit-identical at one
thread as the target; the registration settings fMRIPrep, sMRIPrep and sdcflows use
(`niworkflows/niworkflows/data/*.json`, `sdcflows/sdcflows/data/*.json`) as regression cases,
on real data from the oracle runs; ANTs' own variability with threads measured (whether its
metrics' threaded sums make results depend on the thread count) before any threshold is set.

### 3. A4, part 3: `antsAI` and `antsAffineInitializer` (done 2026-10-17, branch `worktree-agent-a00b4c84605ebf2d7`, not merged yet)

Both programs ported (`larmorx_ants::ai`, `larmorx_ants::affine_initializer`, `lx.ants.ai`,
`lx.ants.affine_initializer`), with every option but `antsAI -b` (blob alignment, refused with a
message; what its port needs is in `docs/findings/ants-ai.md`, "Blobs": ANTs' blob detector,
patch correlations in float, LINPACK's QR and Eigen's vectorised `SelfAdjointEigenSolver`).
Parity records `docs/validation/ants-ai.md` and `ants-affine-initializer.md` (fMRIPrep's own
`init_aff` command lines with fMRIPrep's seed; niworkflows' `antsAffineInitializer` call on
stand-ins). Register: ANTS-52 to ANTS-60, ITK-19 to ITK-21. To decide: ANTS-54, ITK-21.

### 3a. A4, part 3 (done 2026-10-17, merged `084b61d`) and its two corrections

`antsAI` and `antsAffineInitializer` are ported (all but `antsAI -b`). Decided the same day and
done the same day (A4 part 4's first step): ANTS-54 (honour `antsAffineInitializer`'s local-search
argument) and ITK-21 (each start point at the configured learning rate), both corrected by default
with switches; `ants-affine-initializer` re-recorded. `antsAI -b`: its own task after A5/A6.

### 3b. A4, part 4: `antsRegistration`'s point-set metrics (done 2026-10-18, branch `worktree-agent-a1b1eb9dc5f346cdd`, not merged yet)

ICP, PSE, JHCT and IGDM, to full completeness: ITK's exact k-d tree, the readers for label
images (boundary points too), text and VTK (ASCII and binary) point sets, ANTs' intensity
point-set filter, the labelled wrapper, the manifold Parzen density, every combination with
each other and with image metrics, 2D/3D/4D, double and float (`larmorx_registration::pointset`,
`larmorx_ants::registration::pointsets`; `lx.ants.PointSetMetric`,
`lx.ants.IntensityPointSetMetric`). `ants-registration` re-recorded from `8bac0bf`: standard
224 cases (145 identical to the unmodified ANTs, 24 with CORE-MATH, 0 failures), full
235 (146 and 33, 0 failures); every compared point-set case identical to the
unmodified ANTs; expected divergences only where ANTs is not repeatable (a point set's random
fraction, ANTS-64), reads undefined memory (ANTS-66) or never ends (ANTS-69). Correctly rounded
`pow` (a port of CORE-MATH's `cr_pow`, for JHCT) and `pow`/`powf` in the CORE-MATH preload.
Register: ANTS-61 to ANTS-70, ITK-22 to ITK-29; **to decide:** ANTS-67, ANTS-68, ANTS-70, ITK-24,
ITK-29 (all in the point-set metrics, which fMRIPrep does not use). Left: `larmorx-interp`'s
B-spline prefilter still calls the `libm` crate's `pow` (switching it needs `antsApplyTransforms`'
B-spline cases re-run).

### 4. A5: `antsRegistration`'s deformable stages; A6: Atropos

**Part 1, SyN (done 2026-10-19, branch `worktree-agent-aca867b0b4aec5f3f`, not merged yet).**
`SyN[step,update,total]` stages to full completeness: ITK's `SyNImageRegistrationMethod`
(`larmorx_registration::syn`, `::field`), every image metric with local support (`Demons`
included), several metrics, masks, sampling, the point-set metrics in tangent space, 2D/3D/4D,
double and float; displacement fields among the initial transforms (images and composites),
`Warp`/`InverseWarp` outputs, the collapse with fields, composites, `--save-state` and
`--restore-state` of SyN states, `--initialize-transforms-per-stage`, the SyN observer, every
message and failure. `ants-registration` re-recorded from `a3c6cd6`: standard 298 cases
(193 identical to the unmodified ANTs, 32 with CORE-MATH, 57 rejected by both,
16 expected divergences, 0 failures), full 327 (194, 59, 57, 17, 0), with
sMRIPrep's brain extraction and template registration whole on the five oracle runs (on every
third voxel: ds000005 in the standard tier, all five in the full tier; full size in the full
tier; all 20 identical to ANTs with CORE-MATH) and sdcflows' fieldmap-less SyN settings. Register:
ANTS-71 to ANTS-74, ITK-30 to ITK-34. Benchmark report pending (busy machine; one measurement:
96³ CC 10x5, ANTs 122 s at one thread; larmorx 24 to 41 s at one thread and 5 to 9 s at 12,
bit-identical, in two runs on a loaded machine).

**Part 2:** BSplineSyN, `BSpline`, `GaussianDisplacementField`, `BSplineDisplacementField`
(refused today, naming part 2). **Part 3:** the time-varying velocity fields and the
exponential transforms. Then A6, Atropos (k-means, EM, MRF; ANTs' random seeding). With A4-A6,
sMRIPrep's brain extraction and MNI normalisation run in larmorx.

### A5 part 2, as the agent summarised it (from its next-steps text, branch `a5-part2-bspline`, 2026-10-21)

**Part 2, the B-spline and displacement-field stages (done 2026-10-20, branch
`a5-part2-bspline`, not merged yet).** `BSplineSyN`, `GaussianDisplacementField`,
`BSplineDisplacementField` and `BSpline` to full completeness: ITK's B-spline approximation of
fields and scattered points (`larmorx_registration::bspline_field`, ITK's one-thread result on any
thread count), B-spline SyN with weighted masks and point-set fits, displacement fields with
Gaussian and B-spline smoothing on update in `ImageRegistrationMethodv4` (the field scales, the
in-place smoothing), the cubic `BSplineTransform` (domain, level adaptor, scales, a sparse
Jacobian with ITK's compensated sums bit for bit), knot spacings, `BSplineTransform` files read
and written, the collapse with B-spline transforms; the Python API (`mesh_size`,
`total_mesh_size`, `spline_order`). `ants-registration` standard re-recorded from `9a4d65a`:
403 cases (273 identical to the unmodified ANTs, 36 with CORE-MATH, 77 rejected by both, 17
expected divergences, 0 failures; the 106 new cases: 80, 4, 20, 2), with sMRIPrep's template
registration with antsRegistrationSyN.sh's `BSplineSyN[0.1,26,0,3]` on every third voxel; the
full tier adds 4D, the other runs and full size. Register: ANTS-75 (extended), ANTS-76, ANTS-77;
**to decide:** ANTS-78 (BSplineDisplacementField doubles its base mesh) and ITK-35 (no Jacobian
with respect to position for B-spline transforms: later stages fail). 4D `BSpline` on a small
phantom: ANTs 461 s, larmorx 23 s at one thread, identical (informal; benchmark report pending).
Left: `antsApplyTransforms` does not read `BSplineTransform` files yet.

The full tier, recorded later from `fc87e0e`: 441 cases (278 identical to the unmodified ANTs, 68 with CORE-MATH, 77 rejected by both, 18 expected divergences, 0 failures; `docs/validation/ants-registration-full.md`).
