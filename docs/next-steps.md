# Next steps

The ordered work after 2026-10-12, when 3dTshift was completed and the stage comparisons on
the field-map runs were merged. Keep it current: CLAUDE.md's queue points here.

**Order (user, 2026-10-16): port every tool first, then compare with fMRIPrep at the end.** Each
port is still validated against its original program (parity, golden); the stage-by-stage and
end-to-end comparisons with fMRIPrep wait until the tools are all ported.

Ground rules (decision log, 2026-10-12): **full completeness** for every tool (every option,
message and failure case, D13 switches included); **one agent at a time**; each agent's work
merged through CLAUDE.md's checklist. Brief each porting agent for full completeness from the
start, so a tool takes one pass, not several "completing" parts. While an agent runs, the
orchestrator prepares the next task (oracle programs, case lists, the brief), but starts no
other agent.

## Where things stand (2026-10-12)

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

## 1. A4, part 1: ITK's HDF5 transform files in Rust

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

## 2. A4, part 2: `antsRegistration`, the linear stages (done 2026-10-17, merged `c6fdd61`)

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

## 3. A4, part 3: `antsAI` and `antsAffineInitializer` (done 2026-10-17, branch `worktree-agent-a00b4c84605ebf2d7`, not merged yet)

Both programs ported (`larmorx_ants::ai`, `larmorx_ants::affine_initializer`, `lx.ants.ai`,
`lx.ants.affine_initializer`), with every option but `antsAI -b` (blob alignment, refused with a
message; what its port needs is in `docs/findings/ants-ai.md`, "Blobs": ANTs' blob detector,
patch correlations in float, LINPACK's QR and Eigen's vectorised `SelfAdjointEigenSolver`).
Parity records `docs/validation/ants-ai.md` and `ants-affine-initializer.md` (fMRIPrep's own
`init_aff` command lines with fMRIPrep's seed; niworkflows' `antsAffineInitializer` call on
stand-ins). Register: ANTS-52 to ANTS-60, ITK-19 to ITK-21. To decide: ANTS-54, ITK-21.

## 3a. A4, part 3 (done 2026-10-17, merged `084b61d`) and its two corrections

`antsAI` and `antsAffineInitializer` are ported (all but `antsAI -b`). Decided the same day and
done the same day (A4 part 4's first step): ANTS-54 (honour `antsAffineInitializer`'s local-search
argument) and ITK-21 (each start point at the configured learning rate), both corrected by default
with switches; `ants-affine-initializer` re-recorded. `antsAI -b`: its own task after A5/A6.

## 3b. A4, part 4: `antsRegistration`'s point-set metrics (done 2026-10-18, branch `worktree-agent-a1b1eb9dc5f346cdd`, not merged yet)

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

## 4. A5: `antsRegistration`'s deformable stages; A6: Atropos

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

## 5. The rest of the `--fs-no-reconall` path

- AFNI: 3dUnifize, 3dAutomask (fMRIPrep's BOLD reference enhancement). Checked 2026-10-19:
  current fMRIPrep, niworkflows and sdcflows do not call 3dvolreg (to drop unless wanted);
  optional paths of its libraries use 3dQwarp (sdcflows' alternative PEPOLAR estimator) and
  3dSkullStrip with 3dcalc (niworkflows' legacy AFNI skull-stripping): **planned for later** (user, 2026-10-19). Licence per
  file (`docs/licensing.md`, "Notes checked 2026-10-19"): 3dUnifize and 3dAutomask may need
  only one Apache replica. **A later AFNI analysis track (decided by the user, 2026-10-19; not fMRIPrep):** 3dTproject,
  3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit, 3dFWHMx/3dClustSim, 3dAllineate, 3dcalc,
  3dTstat, 3dROIstats/3dmaskave, 3dresample, 3dvolreg (low priority). Check each
  source file's licence: NIH public-domain files get one Apache replica; MCW (GPL) files a
  replica in `larmorx-gpl` and a clean-room original from a spec.
- FSL: BET, FAST, FLIRT with BBR, topup: a replica in `larmorx-nc` (FSL licence, with FSL's
  source as its terms require) and a clean-room original each.
- sdcflows' field-map estimation (Apache Python: the B-spline fit, PEPOLAR and phase-difference
  preparation) and fMRIPrep's own Python steps; FreeSurfer's `mri_coreg` and `lta_convert`.
- The confounds (CompCor, DVARS, framewise displacement, ...), then larmorprepx itself.

## Smaller items, when they fit

- Benchmarks on a quiet machine: N4 against the new oracle; `antsApplyTransforms` after
  `--float`; the BOLD resampler.
- Coverage: a copy of ds005752 with `IntendedFor` added, so the direct-`fieldmap` path is
  covered (fMRIPrep ignored its field map); ds001600's four field-map N4s.

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
- **Python dependencies:** h5py dropped (A4 part 1, 2026-10-16: numpy is the only one); check that every `[prep]` dependency has wheels for all six platforms, Windows arm64
  included. Linux wheels are manylinux_2_28 (glibc 2.28 or newer); decide whether to add
  musllinux wheels (Alpine).
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
