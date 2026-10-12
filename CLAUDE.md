# larmorx: project context

Read this first. The full plan is [PLAN.md](PLAN.md) (v0.3.2). Why the plan looks the way it does is in [docs/decision-log.md](docs/decision-log.md). A plain-language summary of the concepts (replica, clean-room original, oracle), the packages, installation and what is published is in [docs/overview.md](docs/overview.md).

## What we are building
One package, **`larmorx`** (repo `github.com/karellopez/larmorx`), with two layers:

1. **The tool library**: `pip install larmorx`, then `import larmorx as lx`.
   - ANTs/ITK, AFNI and FreeSurfer functions are **ported** to Rust.
   - FSL-like and Connectome-Workbench-like functions have **both** a replica (licence-segregated package) and a clean-room original (Apache-2.0); see `docs/licensing.md`.
   - Every tool has three interfaces, plus documentation and a validation record (PLAN.md §4):
     - a Rust API (crates `larmorx-*`)
     - an idiomatic **Python wrapper** (`lx.ants.registration(...)`)
     - a CLI that accepts the original program's arguments (`larmorx ants antsRegistration ...`, short alias `lx`)
2. **`larmorprepx`**, the preprocessing pipeline, **inside the package**.
   - Module `larmorx.pipelines.larmorprepx`; command `larmorprepx`; installed via the `[prep]` extra.
   - An fMRIPrep-like pipeline in **plain Python functions** on top of the library. BIDS in, fMRIPrep-compatible derivatives and HTML reports out.
   - `larmorx.pipelines/` leaves room for future pipelines.

The FreeSurfer port is a separate wheel, `larmorx-freesurfer` (exposed as `lx.freesurfer`), so its licence terms stay isolated.

**Naming rules:**
- Always lowercase `larmorx` / `larmorprepx` in code and commands.
- Never use plain `larmor` for anything of ours: an unrelated NMR package already ships a `larmor` command.

**Targets:** native wheels for Windows x64/arm64, Linux x64/aarch64 and macOS x64/arm64. No Docker, no external neuroimaging installs, no admin rights.

## Status (2026-10-12)
- **Phase L0 done:** a buildable, tested skeleton, CI green on all six targets.
- **Phase L1 in progress:**
  - `Image`/`Affine` and NIfTI-1/2 I/O: validated against nibabel (`docs/validation/nifti-io.md`) and benchmarked (`docs/benchmarks/nifti-io.md`).
  - **antsApplyTransforms (A1) done:** `lx.ants.apply_transforms`, `larmorx ants antsApplyTransforms`, `lx.transforms`, crates `larmorx-transform`, `-interp`, `-ants`. It is validated against ANTs itself: 83 cases, 63 of 79 bit-identical (`docs/validation/ants-apply-transforms.md`), and benchmarked (`docs/benchmarks/ants-apply-transforms.md`).
  - **antsApplyTransforms `--float 1` done** (2026-10-11): ANTs' single-precision pipeline, computed exactly as ITK's float instantiation defines it (`precision="float32"`, `ApplyTransformsOptions::precision`; ITK's computation type is a type parameter, `larmorx_core::ItkReal`): float points, transforms built and inverted in float (vnl's float SVD inverse, CORE-MATH `sinf`/`cosf` for Euler), float field lookups, float coordinates in every interpolator, text transforms and `-f` read as float, the output clamped to the float range (`docs/findings/ants-float.md`). Parity, standard tier: 166 cases (every case again with `--float 1`), 158 compared: 144 bit-identical to the unmodified ANTs (all 81 `--float 1` cases), 158 with CORE-MATH preloaded (the preload now also replaces `sinf`/`cosf`/`sincosf`). All 14 of fMRIPrep's `--float 1` outputs of the ds000005 run bit-identical to fMRIPrep's own (`docs/validation/fmriprep-stages-float.md`). Gaussian interpolation sped up (vnl's log Γ(1/2) once, weights on the stack): the template-prior step 10.5-11.3 s against ANTs' 12.1-13.9 s on one thread (busy machine; benchmark report pending). Register: ITK-10 (a versor that fails to load in float, reproduced).
  - **3dTshift clean-room original done** (2026-10-10): `lx.afni.tshift`, `larmorx afni 3dTshift`, crate `larmorx-afni`, written from `specs/3dTshift.md` and black-box runs only. 236 parity cases against AFNI 25.2.09 (oracle built by `scripts/build_afni_oracle.sh`): 217 agree, 158 bit-identical, 0 failures (`docs/validation/afni-tshift.md`).
  - **3dTshift replica done** (2026-10-10): `larmorx-gpl afni 3dTshift`, crates `larmorx-gpl-afni` and `larmorx-gpl-cli` in `crates-gpl/` (GPL-3.0-or-later, a separate Cargo workspace). 217 of 217 compared cases bit-identical to AFNI, Fourier included (`docs/validation/afni-tshift-replica.md`). It carries a translation of glibc 2.35's `sinf`/`cosf` (LGPL-2.1-or-later).
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
- **D13 switch and first corrections done** (2026-10-11): `upstream_defects="fix"|"reproduce"` (Python), `LARMORX_UPSTREAM_DEFECTS` (command line), `larmorx_core::defects` (Rust), `lx.UpstreamDefectWarning` ([docs/api/upstream-defects.md](docs/api/upstream-defects.md)). Corrected by default: PIPE-03, PIPE-04, SCI-03, AFNI-01/02/03/05/07 (clean-room original), FSL-01/03/05–10/13/14; FSL-02 opt-in and measured (`docs/validation/mri-hmc-normcorr-count.md`: a change about the size of mcflirt's band, no gain against the ground truth), pending the user's decision. PIPE-03 measured on real geometry (`docs/validation/resample-series-jacobian.md`: fMRIPrep's factor off by more than 1 % in 40 % of T1w-space brain voxels). Parity compares in `reproduce` mode (bit-identical as before) and reports a fix pass; golden `*.reproduce` cases equal the pre-D13 checksums. Still to do: the 3dTshift replica's switch, and the replica's sform for metre/micrometre inputs (an expected divergence in its record).
- **Next (user decisions, 2026-10-11):** (1) a stage-by-stage comparison with the fMRIPrep 25.2.6 oracle run, feeding fMRIPrep's own intermediates to larmorx's tools (done, above); (2) then the missing tools, natively: A4 (antsRegistration rigid/affine, antsAI, antsAffineInitializer, histogram matching), A5 (SyN), A6 (Atropos), AFNI's 3dvolreg/3dUnifize/3dAutomask, FSL's BET/FAST/FLIRT-BBR, FreeSurfer's mri_coreg/lta_convert, and the confounds; then larmorprepx. **fMRIPrep oracle:** fMRIPrep 25.2.6 runs through Apptainer as the user, no Docker and no sudo (`scripts/run_fmriprep_oracle.sh`, image `oracles/images/fmriprep-25.2.6.sif`, digest and settings in each run's `RUN_INFO.txt`). First run done: ds000005 sub-01, `--fs-no-reconall`, outputs and working directory in `oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall/`. Never ask the user for their sudo password.

- **D13 part 1 done** (2026-10-12): every ANTs and ITK gap of the register corrected by default (ANTS-03 to -05, -08 to -16, -19, -20, -24, -25, -27, -30, -31, -33, -35, -36, -38, -39, -43; ITK-01, -02, -03, -10), undefined conversions clamped with a warning (ANTS-06, SCI-02), a new entry ANTS-47; failure and garbage cases with a switch (ANTS-29 and ITK-08 byte for byte; ANTS-07, -19, -28 with the undefined memory as zeros). ResampleImage's command line redesigned (ANTS-30: parameters after the interpolator, then the pixel type; `docs/api/ants-resample-image.md`, "The seventh argument"; awaits review). ITK-01 and ANTS-39 warn only where a stored value changes (`larmorx_core::IndexFloor::checked`). Parity in `reproduce` mode unchanged against `oracles/ants-2.6.5` (records from `50509e30b3e8`), plus a fix pass per suite; two former expected divergences now pass (ResampleImage size 1 byte for byte; ResampleImageBySpacing's uninitialised buffer, everything but the values). Notable fix-pass change: ANTS-43 alters 7-8 % of the voxels of sMRIPrep's denoised T1w (median 0.6 to 2.9, within ANTs' thread band). Found: ITK-11 (ITK's Gaussian kernel inaccurate above a variance of about 50 voxels², not D13). Golden: 7 `*.reproduce` cases; the changed defaults re-recorded.
- **D13 part 2a done** (2026-10-11): the shared compatibility switch (`larmorx_core::defects`, Python `upstream_defects="fix"|"reproduce"` and `lx.UpstreamDefectWarning`, CLI `LARMORX_UPSTREAM_DEFECTS`; `docs/api/upstream-defects.md`) and the fixes for PIPE-03/-04, SCI-03, the head-motion original's FSL-* and the 3dTshift original's AFNI-* defects. FSL-02 stays opt-in (`normcorr_count="weights"`). Reports for fMRIPrep (PIPE-02, PIPE-03) are drafts in `docs/upstream-reports/`, not yet filed by the user.

### Work in progress and queue (2026-10-11, evening)
One agent at a time. The queue, in order:
1. **Done: `antsApplyTransforms --float 1`** (see the status above; worktree branch `worktree-agent-a689dd2fa349688de`). D13 defaults unchanged (ANTS-33, ITK-01, ITK-10 reproduced).
2. **Done: D13 part 1** (agent branch `worktree-agent-a448db6a134de1e65`, see the status above). For the user: review ResampleImage's corrected command line (ANTS-30).
3. **Done: D13 part 2b** (merged 2026-10-12, `8ab6cc5`; verified by re-running both 3dTshift suites). **Completing 3dTshift** (decision log 2026-10-12): part 1 done (the replica's AFNI-16 default, spec §6.1-6.5; merged `3d2ae09`, verified). **Running: part 2a**, AFNI's global options `-overwrite` and `-Dname=value` in the replica plus their spec (replica author; agent branch `worktree-agent-a5cefece59c950414`). **Then part 2b**: the clean-room original implements spec §6.1-6.5 and the global options. What 2b was: the same fixes in the GPL 3dTshift replica (replica author, may read AFNI source), plus its bug: it writes a metre or micrometre input's sform unconverted (the two expected divergences in `REPLICA_DIVERGENCES`).
4. **Stage comparisons on the new oracle runs:** `python -m larmorx_validation pipeline-stages --oracle <run>` for each run below, once finished. Confirm PIPE-03 on real PEPOLAR data (ds005040). Regenerate `docs/validation/fmriprep-stages.md` for ds000005 too: its N4 rows predate the source-faithful N4, and D13 changed defaults.
5. **Pending measurements on a quiet machine:** `docs/benchmarks/ants-n4.md` against the new oracle; antsApplyTransforms after `--float`.
6. **A4** (linear registration, antsAI, antsAffineInitializer, histogram matching), validated against the GCC 14.3 oracle only. Then A5, A6, AFNI's 3dvolreg/3dUnifize/3dAutomask, FSL's BET/FAST/FLIRT-BBR, FreeSurfer's mri_coreg/lta_convert, the confounds, larmorprepx.

**fMRIPrep oracle runs** (`oracles/fmriprep-25.2.6/<run>/`; done when its `RUN_INFO.txt` has `finished:`):
- `ds000005-sub-01-noreconall`: done; no field maps, no slice timing.
- `ds001600-sub-1-noreconall`: done, anatomy only; its BOLD runs have 5 volumes, which fMRIPrep skips.
- `ds005040-sub-002-noreconall` (PEPOLAR plus slice timing): done, 58 min; its BOLD was processed (10 functional outputs, a PEPOLAR field-map report).
- `ds005454-sub-16-noreconall` (`phasediff`, multiband 4, 280 volumes): done, 1 h 25 min.
- `ds005752-sub-ON01016-noreconall` (direct `fieldmap`): done, 55 min.
All three finished on 2026-10-11 with exit status 0 (`oracles/logs/fmriprep-25.2.6-sdc-chain.log`); their stage comparisons are queue item 3.

## The user's direction (settled; do not re-propose the alternatives)
- **Not a fork of fMRIPrep.** No nipype, traits, niworkflows/smriprep/sdcflows runtime dependencies, or graph engines. Ops are plain typed functions; per-subject orchestration is readable Python.
- **Port to Rust** where existing Python packages are limited, **including ANTs**: ANTsPy has no Linux-aarch64, Windows-arm64 or Python-3.14 wheels. Also **FreeSurfer recon-all**: the goal is the same results, several times faster.
- Rust implementations may **reimagine** algorithms for speed, provided validation shows equivalent results.
- **Better algorithms by default (D11, user decision 2026-10-10).** larmorx should be an evolution of the upstreams: faster and better, not only replicas. An improved algorithm becomes the default once its results stay within the upstream tool's own variability (PLAN.md G4, §11); larger changes stay opt-in. Replicas always stay exact.
- **Correct by default, faithful on request (D13, user decision 2026-10-11).** "What I like at the end is to have the best and methodically correct implementation." When an original has a *deterministic* bug that changes results (e.g. AFNI's negative-slope scaling, mcflirt's last-volume initialisation), larmorx fixes it by default and warns when the bug would have changed the result; a documented compatibility switch reproduces the original bit for bit, and parity tests use it. Crashes, out-of-bounds reads, races and nondeterminism are never reproduced by default. Details decided 2026-10-11: undefined float-to-integer conversions clamp and warn; replicas share the corrected default, and their switch gives exactly the original's result; even failure and garbage cases get a switch (exact where the original is deterministic; where it depends on memory contents, the defined parts are reproduced and the rest is filled deterministically with a warning); mcflirt's weight-count bug is measured before deciding. Every defect found goes into `docs/findings/upstream-defects.md`, and the method into `docs/porting-guide.md`.
- **Every platform from the start** (user, 2026-10-10): Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. Design, test and benchmark with all six in mind; CI must stay green on all of them.
- **How to port (user, 2026-10-09):**
  - Make a faithful **replica** when the licence allows it: ANTs/ITK (Apache-2.0), AFNI's NIH public-domain code, CORE-MATH (MIT), nibabel (MIT). Bit-identical results are the target.
  - When the licence restricts it (AFNI's MCW code, Workbench, FSL), make **both**: a bit-exact replica in that licence family's package, and a clean-room original in the main package (rule 1, `docs/licensing.md`).
  - MSM: a clean-room original only, in the surfaces track, after a patent check.
- **Each tool gets its own Python wrapper**, so other developers can build on the library.

## Workspace layout
```
<workspace>/                     e.g. ~/.../super_fmriprep
  CLAUDE.md                      imports this file (written by the bootstrap script)
  .venv/                         Python venv (evaluation: fmriprep, antspyx, SimpleITK)
  larmorx/                       THIS REPO (github.com/karellopez/larmorx)
  fmriprep/ smriprep/ niworkflows/ sdcflows/ nireports/ nitransforms/ nipype/ nibabel/
  nitime/ pybids/ python-client/ tedana/ acres/ migas-py/
  ANTs/ ITK/ afni/ freesurfer/   read-only upstream references, pinned in upstream.tsv
  reference_src/workbench/  reference_src/MSM_HOCR/     licence-restricted sources (docs/licensing.md)
```

Recreate it with:
```
larmorx/scripts/bootstrap_workspace.sh --venv .venv --install-rust
```

## Hard rules
1. **Licences: two tracks, kept apart** (user decision, 2026-10-09). See `docs/licensing.md`.
   - **Replica:** a bit-for-bit port of the upstream source, for the most accurate output. It carries the upstream's licence and lives in a package per licence family:
     - permissive upstreams (ANTs/ITK, AFNI's NIH files, CORE-MATH, nibabel): the main `larmorx` package (Apache-2.0);
     - AFNI's MCW files and Workbench (GPL-2.0-or-later): `larmorx-gpl` (GPL-3.0-or-later, `crates-gpl/`);
     - FSL: `larmorx-nc` (the FSL Licence's non-commercial terms, `crates-nc/`);
     - FreeSurfer: `larmorx-freesurfer`.
   - **Original:** a strict clean-room implementation with the same behaviour, Apache-2.0, in the main package. It is needed wherever the replica is not Apache-compatible. The implementer works only from `specs/<tool>.md`, documentation, papers and black-box runs of the upstream binary; never the upstream source, the replica crates or source-derived findings (`docs/findings/afni-tshift.md`, `fsl-mcflirt.md` and the sections of `upstream-defects.md` marked restricted). Who may read what: `docs/porting-guide.md` §1.
   - **Never** move replica code into the main package, even renamed. Never link code from different licence families into one binary: `larmorx-gpl` and `larmorx-nc` are incompatible with each other.
   - **MSM_HOCR:** no replica can be published (its ELC library forbids public distribution of derivatives, and its FastPD optimiser is patented). Its original must avoid those optimisation methods.
   - **Every file records its origin** with an SPDX header and in its crate's `PROVENANCE.md`.
2. **Ported code records its provenance.** Each crate's `PROVENANCE.md` lists the upstream files and versions it was ported from. Port from the pinned **release tags**, not the local master/dev clones:
   - ANTs v2.6.5
   - ITK v5.4.5 (the version ANTs 2.6.5 pins; decided 2026-10-09)
   - AFNI 25.2.09
   - FreeSurfer v8.2.0 (pending D5)
3. **FreeSurfer-derived code lives only in `crates/larmorx-freesurfer`** (separate wheel, carries the FreeSurfer licence text and preface). Audit each ported file for third-party code first.
4. **Upstream clones are read-only.** Never modify them, and never vendor code from them without attribution.
5. **Rust rules:**
   - Pure-Rust dependencies only: no C, C++, Fortran or HDF5 libraries.
   - Deterministic reductions, independent of thread count.
   - Correctly rounded transcendental functions from `larmorx_core::math` (pure-Rust ports of CORE-MATH, decided 2026-10-09). They give the same bits on every platform and match glibc, and so ANTs/AFNI on Linux, in more than 99.8 % of calls. Use the `libm` crate only for functions not yet ported, and say so in the code.
   - Seeded RNG; no fast-math; no `target-cpu=native` in release builds.
   - Explicit `n_threads` on every call; release the GIL.
   - `#![forbid(unsafe_code)]` outside audited hot loops.
6. **Python rules:**
   - ≥ 3.12.
   - Typed dataclass inputs and outputs; no global config; no pickles in caches.
   - UTF-8 I/O; no `shell=True`; `spawn` only.
7. **The per-tool contract** (PLAN.md §4) is mandatory for every tool. Start every port from `docs/porting-guide.md`: the method, and the contract as a checklist with the models to copy (§11).
   - Rust API, Python wrapper, compatible CLI
   - `.pyi` stubs + a docs page with the option-mapping table
   - a validation record and status (`experimental` / `validated` / `stable`)
   - `PROVENANCE.md`
   - at least one **golden case** (rule 10), so every platform is checked against Linux
8. **The source is the specification; an oracle binary is only a check** (user, 2026-10-11). A replica reproduces what the upstream source defines, not what one compiler made of it. When larmorx and an oracle binary disagree and the cause is the binary's code generation (e.g. GCC 11 at `-O3` dropping two float roundings in N4's B-spline fit), follow the source, record the binary's behaviour in `docs/findings/`, and prefer an oracle build that honours the source.
   **No equivalence claims without oracle numbers.** Thresholds are in PLAN.md §11.3; variability bands come from repeated oracle runs.
9. **Git: no AI attribution.** This rule is the user's instruction and overrides any tool default.
   - Never add `Co-Authored-By: Claude …` (or any AI co-author) trailers to commits.
   - Never add "Generated with Claude Code" or similar lines to commit messages, PR descriptions, issues or release notes.
   - Commits are authored by the repo's configured git identity only. This repo uses the user's GitHub no-reply address (`git config user.email`), so their personal email is not exposed in this public repo.
10. **Cross-platform golden tests, and a small CI budget** (user decision, 2026-10-10).
   - **Two kinds of checks.** *Parity* compares larmorx with the original tools; it runs only on the Linux x64 machine, where the originals exist. *Golden tests* check that the other five platforms (Linux aarch64, macOS Intel and Apple silicon, Windows x64 and arm64) reproduce larmorx's own Linux x64 output. Most originals do not exist on Windows or macOS, so comparing with them there is not the goal. Golden tests cover replicas and clean-room originals alike.
   - **Every function has at least one golden case:** every CLI tool, ImageMath op and public Python function. A case is an input recipe generated by the test itself, the arguments, and the SHA-256 of the output data.
     - Inputs use seeded random numbers and plain arithmetic only: no `sin`, `exp` or other transcendental functions, which differ between platforms. So inputs are identical everywhere, and no image data is committed.
     - The checksums are recorded on Linux x64, by a script, from a commit whose parity passed.
     - A test fails when a registered tool has no golden case.
   - **Bit-identity is the rule.** A tolerance needs a documented reason in `docs/findings/`, and preferably a fix instead.
   - **Keep CI small; GitHub Actions time is limited.** Be careful and precise:
     - golden inputs at most 32³ voxels (4 volumes for 4D); one case per distinct code path, not one per parameter value;
     - the whole golden suite runs in under a minute per platform, on the release wheel the test job already builds; once per platform (one Python version), inside the existing jobs; no new jobs or runners;
     - parity, benchmarks and real data never run in CI;
     - docs-only changes should not trigger CI (`paths-ignore`);
     - before adding any CI work, estimate its minutes times the number of jobs (13 today).

## Facts worth knowing (from docs/analysis/)
- fMRIPrep's heaviest compute, 4D BOLD resampling, is Python: `fmriprep/fmriprep/interfaces/resampling.py`.
- fMRIPrep/ANTs registration settings are in `niworkflows/niworkflows/data/*.json` and `sdcflows/sdcflows/data/*.json`. They are regression tests for `lx.ants.registration`.
- **ANTsPy as an in-process oracle:**
  - `ants.internal.get_lib_fn('antsRegistration')(args)` runs raw antsRegistration arguments.
  - Wheels exist for linux-x64, macOS and win-x64, Python 3.8–3.13.
  - `ants.motion_correction` is a slow per-volume Python loop (one reason `lx.mri.hmc` exists).
- fMRIPrep Docker images are linux/amd64. Generate fMRIPrep and FreeSurfer oracles **on this machine**.
- Sizes:
  - ANTs ≈ 160k lines; the ITK modules it relies on ≈ 200k
  - FreeSurfer's recon-all path ≈ 200–250k lines (surface library 82k, GCA + GCA morph 44k)
- Upstream fMRIPrep CI only compares output file *lists*. Our numeric oracle harness is new work.

## Working on this machine
- Run long jobs (fMRIPrep and FreeSurfer oracles, benchmarks) in the background with `tmux` or `nohup`.
  - Log to `oracles/logs/`.
  - Record the exact command, image tag/version, seeds and thread counts next to the outputs.
- Never commit image data or oracle outputs to git. Use content-addressed storage (PLAN.md §11.1).
- **Run one agent at a time** (user, 2026-10-10; a two-agent trial on 2026-10-11 was stopped the same evening to stay within usage limits). Four parallel agents once exhausted the session limit and were killed mid-task. Brief every agent to commit as soon as its work passes the gates; when pausing one, ask for a WIP checkpoint commit and a NOTES file first.
- **The ANTs oracle is built with conda-forge GCC 14.3** (`oracles/toolchains/gcc-14.3`, user-level). Ubuntu 22.04's GCC 11.4 and 12.3 miscompile float roundings at `-O3` (GCC bug 110182); `scripts/build_ants_oracle.sh` probes for it and refuses such a compiler. The old GCC 11 build is kept as evidence in `oracles/ants-2.6.5-gcc11-O3`; validate registration (A4/A5) against the new build only.
- **Wait loops must not match themselves.** `until ! pgrep -f "larmorx_validation bench"` also matches its own command line, so it never ends (four such loops ran for a day, 2026-10-11). Use a bracket: `pgrep -f "[l]armorx_validation bench"`.
- **Never delete work products** (code, drafts, ports, harnesses, reports). If something must leave the repo or the build, archive it in `<workspace>/archive/<date>-<topic>/` (not under `/tmp`) and tell the user. Ask before deleting anything that is not a temporary build artefact.
- Verify the environment at session start: `uname -m`, `nproc`, free RAM, `apptainer --version`, `rustc --version`. Docker is not installed; Apptainer replaces it.
- **Merging an agent's work: the checklist** (the user: "Make sure everything works before merging"). Nothing reaches `main` or GitHub before steps 1-5 pass on the merged tree in the main checkout:
  0. Start from a clean tree; chain the merge with `&&` and check `.git/MERGE_HEAD`. On 2026-10-11 an uncommitted file made `git merge` abort, and the checks then ran on the old tree.
  1. Both Rust workspaces: `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked`, in the root and in `crates-gpl`.
  2. `maturin develop --release`, then `pytest tests/python tests/golden`.
  3. `ruff check`, `ruff format --check`, `scripts/check_spdx.py`.
  4. `pytest tests/parity` (the smoke tier against the real oracles).
  5. Re-run the new or changed tool's standard-tier parity yourself, from the committed merge (a clean tree, so the record's commit is clean), and compare with the agent's record.
  6. Commit, check for AI attribution, push, then watch CI on all six platforms.
- **Reading CI results without a login:** GitHub serves job logs only to logged-in users. Golden mismatches are printed as workflow annotations, which `https://api.github.com/repos/karellopez/larmorx/check-runs/<job id>/annotations` serves without a login.
- **Detached jobs (`nohup ... &`) send no notice when they end.** Watch their log for a final line in a tracked background command.

## Repo map
| Path | Contents |
|---|---|
| `PLAN.md` | the plan |
| `CLAUDE.md` | this file |
| `docs/decision-log.md` | direction changes and open decisions |
| `docs/analysis/01–04` | what fMRIPrep computes (stages, tools, parameters, file:line) |
| `upstream.tsv` | pinned upstream commits |
| `scripts/bootstrap_workspace.sh` | recreates the workspace |
| `docs/overview.md`, `docs/licensing.md` | the concepts in plain terms; the licence tracks and what is published |
| `docs/porting-guide.md` | how to port a tool: tracks and who may read what, the source as the specification, the original's own variability, oracle coverage, the pipeline's real parameters, separating causes, validation records, D11/D13, determinism, working safely, the per-tool checklist |
| `Cargo.toml`, `crates/` | Rust workspace: `larmorx-core`, `-io`, `-transform`, `-interp`, `-image`, `-ants`, `-afni`, `-cli` (standalone `larmorx`/`lx`), `-py` (`larmorx._core`) |
| `crates-gpl/` | the GPL-3.0-or-later replica workspace (`docs/licensing.md`); it builds the Apache crates by path, so refresh `crates-gpl/Cargo.lock` whenever their dependencies change (CI checks it with `--locked`) |
| `crates-nc/` (planned) | the FSL replica workspace |
| `specs/` | behaviour specs, the input of clean-room originals |
| `docs/upstream-reports/` | drafts of bug reports for the upstream projects (marked "draft, not yet filed" until the user files them) |
| `pyproject.toml`, `python/larmorx/` | maturin package (abi3, CPython >= 3.12), including `pipelines/larmorprepx/` |
| `tests/python/` | Python tests of the installed package |
| `.github/workflows/ci.yml` | lint + wheel build and tests on the six targets |
| `validation/` | parity suites and benchmarks (dev-only package `larmorx-validation`) |
| `docs/architecture.md`, `docs/api/`, `docs/validation/`, `docs/benchmarks/`, `docs/findings/` | conventions, user docs per tool, validation records, benchmark reports, findings about the upstream tools |
| `docs/findings/upstream-defects.md` | the register of every defect found in the originals (class, evidence, fMRIPrep impact, what larmorx does, D13 gap), with the list of defects larmorx still reproduces by default |
| `../larmorx-testdata/` (separate repo) | test-data catalog, generated edge cases, hash-verified downloader (`pip install -e`) |
