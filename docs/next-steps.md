# Next steps

The ordered work after 2026-10-12, when 3dTshift was completed and the stage comparisons on
the field-map runs were merged. Keep it current: CLAUDE.md's queue points here.

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
the n-bit and scale-offset filters, extensible-array chunk indices and virtual datasets
(ITK's HDF5 reads them; no transform file uses them); a fractal heap's indirect blocks in the
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

## 2. A4, part 2: `antsRegistration`, the linear stages

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

## 3. A4, part 3: `antsAI` and `antsAffineInitializer`

Every option of both programs (`antsAffineInitializer` was added to the oracle on
2026-10-12), with sMRIPrep's and niworkflows' calls as regression cases. Then re-run the
stage comparisons: the brain-extraction initialisation and the registration nodes become
comparable.

## 4. A5: `antsRegistration`'s deformable stages; A6: Atropos

SyN, BSplineSyN, the exponential, Gaussian and B-spline displacement-field and time-varying
transforms, their composite outputs (part 1's HDF5); then Atropos (k-means, EM, MRF; ANTs'
random seeding). With A4-A6, sMRIPrep's brain extraction and MNI normalisation run in larmorx.

## 5. The rest of the `--fs-no-reconall` path

- AFNI: 3dUnifize, 3dAutomask (fMRIPrep's BOLD reference enhancement), 3dvolreg. Check each
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
- **Publishing:** a release workflow (on a version tag: the six wheels and the sdist of each
  package, the golden tests, upload by PyPI Trusted Publishing; standalone binaries to GitHub
  Releases); reserve the names `larmorx`, `larmorx-gpl`, `larmorx-nc`, `larmorx-freesurfer`,
  `larmorprepx` (all free on 2026-10-13; needs the user's PyPI account); the licence checks for
  publishing `larmorx-nc` (FSL) and `larmorx-freesurfer` on PyPI; the first release's scope
  (D10) and version numbering.

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
