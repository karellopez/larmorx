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

## The order from here

1. **The visual reports** (user, 2026-10-20), before the next program: the shared figure code in
   `larmorx_validation` (one style, every figure drawn from the validation records, reproducible
   from a commit); figures for every tool already ported (agreement overviews, difference
   distributions, image comparisons such as N4 bias fields, registration overlays, warp-field
   differences and 3dTshift time series, the originals' own variability, speed); per-tool pages on
   the documentation site (`larmorx_documentation`), with the landing page's two fixes (the hero
   on phones mid-scroll, a font with Greek letters).
2. **A5 part 3:** the time-varying velocity fields and the exponential transforms.
3. **A6, Atropos** (k-means, EM, MRF; ANTs' random seeding). With A4 to A6, sMRIPrep's brain
   extraction and MNI normalisation run in larmorx.
4. **`antsAI -b`** (blob alignment), its own task (user, 2026-10-17): ANTs' blob detector, patch
   correlations in float, LINPACK's QR and Eigen's vectorised `SelfAdjointEigenSolver`
   (`docs/findings/ants-ai.md`, "Blobs").
5. **AFNI:** 3dUnifize, 3dAutomask (fMRIPrep's BOLD reference enhancement). Checked 2026-10-19:
   current fMRIPrep, niworkflows and sdcflows do not call 3dvolreg (to drop unless wanted);
   optional paths of its libraries use 3dQwarp (sdcflows' alternative PEPOLAR estimator) and
   3dSkullStrip with 3dcalc (niworkflows' legacy AFNI skull-stripping): **planned for later**
   (user, 2026-10-19). Licence per file (`docs/licensing.md`, "Notes checked 2026-10-19"):
   3dUnifize and 3dAutomask may need only one Apache replica. **A later AFNI analysis track
   (decided by the user, 2026-10-19; not fMRIPrep):** 3dTproject, 3dDespike, 3dReHo, 3dRSFC,
   3dDeconvolve/3dREMLfit, 3dFWHMx/3dClustSim, 3dAllineate, 3dcalc, 3dTstat,
   3dROIstats/3dmaskave, 3dresample, 3dvolreg (low priority). Check each source file's licence:
   NIH public-domain files get one Apache replica; MCW (GPL) files a replica in `larmorx-gpl`
   and a clean-room original from a spec.
6. **FSL:** the mcflirt replica, BET, FAST, FLIRT with BBR, topup: a replica in `larmorx-nc` (FSL
   licence, with FSL's source as its terms require) and a clean-room original each.
7. **Field maps, FreeSurfer, surfaces, confounds:** sdcflows' field-map estimation (Apache Python:
   the B-spline fit, PEPOLAR and phase-difference preparation) and fMRIPrep's own Python steps;
   FreeSurfer (`larmorx-freesurfer`: `mri_coreg`, `lta_convert`, then recon-all; its Talairach step
   as a clean-room original with its atlas fetched on first use, its data packaged apart from the
   wheels; `docs/licensing.md`); pretrained models as Rust inference with the original weights
   (decision log 2026-10-19); Connectome Workbench- and MSM-compatible surface tools; the
   confounds (CompCor, DVARS, framewise displacement, ...).
8. **larmorprepx**, then the end-to-end comparison with fMRIPrep.
9. **Releases on PyPI** (below).

## Follow-ups, when they fit

- **Benchmark reports** on a quiet machine: N4 against the new oracle; `antsApplyTransforms`
  after `--float`; the BOLD resampler; SyN (one informal measurement: 96³ CC, ANTs 122 s at one
  thread, larmorx 24 to 41 s, bit-identical); the B-spline and displacement-field stages (4D
  `BSpline` on a small phantom: ANTs 461 s, larmorx 23 s at one thread, identical); HDF5 reading
  against h5py and ANTs.
- **`antsApplyTransforms` with `BSplineTransform` files** (the `BSpline` stage's output; refused
  with a message today, `warped=True` gives the warped image), with parity cases.
- **Records made with the older CORE-MATH preload** (`ants-image-math`, `ants-n4`, `ants-denoise`,
  ...): re-run; the preload now also replaces `pow`, `powf`, `asin`, `acos`, `atan2` and `cbrt`.
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

- **PIPE-16** (for larmorprepx): nipype's N4 interface writes the corrected field-map
  reference in the input's int16 type (re-quantized, up to 0.3 % at the dimmest voxels). Keep
  fMRIPrep's behaviour, or keep N4's float32 result.
- **In-place writing:** an existing writable output in a directory that cannot be written is
  replaced in place by AFNI but is an error in larmorx (it writes a temporary file and renames
  it). Proposed: fall back to writing in place there, in both 3dTshift implementations.
- **ANTS-30:** review ResampleImage's corrected command line (`docs/api/ants-resample-image.md`).
- **Upstream reports:** PIPE-02 and PIPE-03 are drafts in `docs/upstream-reports/`, now with
  PIPE-03's real-data numbers, ready to file.
- **`LARMORX_GPL_BIN`:** Python still reads it, only to find the replica program.
- **The point-set metrics' defects** (A4.4; fMRIPrep does not use these metrics): ANTS-67,
  ANTS-68, ANTS-70, ITK-24, ITK-29, each kept faithful until decided (the register says what each
  is).
- **Two questionable behaviours of the B-spline stages** (A5.2), reproduced until decided:
  **ANTS-78**, BSplineDisplacementField's base-level mesh is already doubled at the base level;
  **ITK-35**, B-spline transforms have no Jacobian with respect to position in ITK, so a later
  linear, field or BSpline stage fails (implementing it would let those stages run).
