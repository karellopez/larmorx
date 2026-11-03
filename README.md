# LARMORX

**Library of Algorithmic Reimplementations for MRI through Open Rust eXecution**

> **LARMORX — Bringing Neuroimaging Tools into Resonance.**
> Reimplemented in Rust. Accessible through Python. Built for FAIR research.

LARMORX is an open-source, cross-platform neuroimaging toolbox that reimplements established
neuroimaging algorithms and command-line tools in Rust. It aims to reproduce the functionality
and behaviour of tools from established neuroimaging ecosystems, including ANTs, AFNI,
FreeSurfer and others, without requiring users to install the original binaries or manage
complex system dependencies. Where a licence requires it, algorithms are reimplemented under
clean-room development practices, so that restrictively licensed source code is not
incorporated.

**Status: early development.** The tools listed under [Supported tools](#supported-tools) are
implemented and validated against the original programs; everything else is planned. LARMORX is
not yet published on PyPI: build it from source ([Development](#development)). The package,
module and commands are spelled `larmorx` (and `larmorprepx` for the planned pipeline).

## Core features

- **Rust implementations.** Pure Rust, with no C, C++ or Fortran library dependencies.
  By design, results do not depend on the thread count, and transcendental functions come from
  correctly rounded [CORE-MATH](https://core-math.gitlabpages.inria.fr/) ports (or the
  deterministic pure-Rust `libm` where not yet ported), so a tool gives the same bits on every
  platform.
- **Command-line compatibility.** `larmorx <family> <program> <the original's arguments>`
  (short alias `lx`), e.g. `larmorx ants antsApplyTransforms ...` or `larmorx afni 3dTshift ...`,
  accepting the original's options; how far messages and exit statuses match is stated in each
  tool's validation record.
- **Python bindings.** `import larmorx as lx`: the same tools with typed options, on in-memory
  images.
- **Cross-platform.** Continuous integration builds the wheels and runs the test suites on
  Linux (x86-64, aarch64), macOS (Intel, Apple silicon) and Windows (x64, arm64); golden tests
  check that every tool reproduces its Linux x86-64 output bit for bit on each platform
  ([golden tests](docs/validation/golden.md)).
- **Two ways of reimplementing, kept apart by licence** ([licensing](docs/licensing.md)):
  - tools with permissive licences (ANTs and ITK, Apache-2.0) are ported from their source,
    with each Rust function's origin recorded in the crate's `PROVENANCE.md`;
  - where the licence is restrictive (FSL, Workbench), a **clean-room original** is
    written from a behavioural specification, documentation and runs of the original program,
    never from its source; an exact replica, where one exists, is a separate package under the
    upstream's licence (e.g. `larmorx-nc`) that runs as a separate program. AFNI's tools have
    both inside larmorx since AFNI 26.3.05 released its MCW code under CC BY 4.0. Tools with both
    take `implementation="auto" | "replica" | "original"`.
- **Validated, with documented differences.** Every tool has a validation record comparing it
  case by case with the original program ([docs/validation](docs/validation/)). Defects found
  in the originals are recorded ([register](docs/findings/upstream-defects.md)) and corrected by
  default with a warning; `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`
  on the command line) gives the original's behaviour, and the parity tests run in that mode
  ([upstream defects](docs/api/upstream-defects.md)).

## Philosophy

The name is inspired by the **Larmor frequency**, a fundamental concept in magnetic resonance
imaging: in a uniform magnetic field B₀, nuclear spins of the same species precess at the same
frequency, ω = γB₀. In the same way, LARMORX aims to bring neuroimaging tools developed in
different software ecosystems into a common computational framework, aligned toward a shared
objective: **FAIR** scientific research, research that is Findable, Accessible, Interoperable
and Reusable.

The project follows the FAIR Principles for Research Software
([FAIR4RS](https://doi.org/10.15497/RDA00068)):

- **Findable:** a public repository with versioned history; PyPI releases with citation
  metadata are planned.
- **Accessible:** open source under Apache-2.0; native builds with no external neuroimaging
  installations, containers or administrator rights.
- **Interoperable:** standard formats (NIfTI-1/2, ITK transform files including HDF5), the
  original tools' command-line arguments, and a Python API for pipelines.
- **Reusable:** a permissive licence, per-tool documentation, provenance records, case-by-case
  validation records and a register of the differences from the originals.

## Installation and usage

LARMORX is not on PyPI yet. To build it, see [Development](#development). The planned
distribution ([overview](docs/overview.md)):

```bash
pip install larmorx                  # the library and the larmorx / lx commands   (planned)
pip install larmorx                  # the exact AFNI replicas included (CC BY 4.0's attribution)
pip install "larmorx[prep]"          # + the larmorprepx pipeline                   (planned)
```

Command line, with the original programs' arguments:

```bash
larmorx ants N4BiasFieldCorrection -d 3 -i t1w.nii.gz -o t1w_n4.nii.gz
larmorx ants antsApplyTransforms -d 3 -i boldref.nii.gz -r t1w.nii.gz -t xfm.h5 -o boldref_t1w.nii.gz
larmorx afni 3dTshift -tpattern alt+z -TR 2 -prefix bold_stc.nii.gz bold.nii.gz
```

Python:

```python
import larmorx as lx

t1w = lx.load("t1w.nii.gz")
t1w_n4 = lx.ants.n4_bias_field_correction(t1w, n_threads=8)
boldref_t1w = lx.ants.apply_transforms(lx.load("boldref.nii.gz"), t1w, ["xfm.h5"])
bold_stc = lx.afni.tshift(lx.load("bold.nii.gz"), slice_times="alt+z", tr=2.0)
lx.save(t1w_n4, "t1w_n4.nii.gz")
```

## Supported tools

Implemented and validated against the original programs (ANTs 2.6.5, AFNI 25.2.09, FSL 6.0.7,
nibabel). Each line links to its validation record; API pages are in [docs/api](docs/api/),
benchmark reports in [docs/benchmarks](docs/benchmarks/).
The table is generated from [validation/tools.toml](validation/tools.toml) and the records
(`python -m larmorx_validation tables`). Beside the records, every tool is tested against its
original with random valid inputs and options, a night at a time
([random testing](docs/validation/difftest.md)).

<!-- BEGIN GENERATED: tools table (python -m larmorx_validation tables; edit validation/tools.toml) -->
| Tool | Command / Python | How | Validation (standard tier) |
|---|---|---|---|
| NIfTI-1/2 I/O | `lx.load`, `lx.save`, `lx.Image` | own implementation | [254 files](docs/validation/nifti-io.md), data bit-identical to nibabel; [4–8× faster](docs/benchmarks/nifti-io.md) on compressed writes with threads |
| antsApplyTransforms | `larmorx ants antsApplyTransforms`, `lx.ants.apply_transforms` | port (ANTs/ITK) | [258 cases](docs/validation/ants-apply-transforms.md): 185 of 200 compared bit-identical to ANTs, the rest differ only through glibc's maths; B-spline transforms included; data panel: [329 of 425](docs/validation/ants-apply-transforms-panel.md) as built, all with correctly rounded maths (fMRIPrep's 421 calls on the fourteen oracle runs) |
| ITK transform files (`.mat`, `.txt`, `.h5`) | `lx.transforms` | port (ITK, HDF5) | [149 cases](docs/validation/itk-hdf5.md), 0 failures; HDF5 read and written in pure Rust |
| N4BiasFieldCorrection | `larmorx ants N4BiasFieldCorrection`, `lx.ants.n4_bias_field_correction` | port | [29 of 46](docs/validation/ants-n4.md) bit-identical to ANTs, 46 of 46 with correctly rounded maths; ANTS-40 corrected by default; data panel: [20 of 69](docs/validation/ants-n4-panel.md) as built, 69 of 69 with correctly rounded maths |
| DenoiseImage | `larmorx ants DenoiseImage`, `lx.ants.denoise_image` | port | [7 of 40](docs/validation/ants-denoise.md) bit-identical to ANTs, 40 of 40 with correctly rounded maths; ANTS-43 and ANTS-44 corrected by default; data panel: [0 of 21](docs/validation/ants-denoise-panel.md) as built, 21 of 21 with correctly rounded maths |
| ImageMath (every operation fMRIPrep, sMRIPrep and sdcflows call) | `larmorx ants ImageMath`, `lx.ants.image_math` | port | [334 cases](docs/validation/ants-image-math.md): values bit-identical in 303 of 308, headers in 308 of 308; ANTS-17, -22 and -23 corrected, checked against known results; data panel: [374 of 374](docs/validation/ants-image-math-panel.md) bit-identical |
| ThresholdImage, MultiplyImages, SmoothImage, ResampleImage, ResampleImageBySpacing | `larmorx ants ...` | port | bit-identical in every compared case ([threshold](docs/validation/ants-threshold-image.md), [multiply](docs/validation/ants-multiply-images.md), [smooth](docs/validation/ants-smooth-image.md), [resample](docs/validation/ants-resample-image.md), [by spacing](docs/validation/ants-resample-image-by-spacing.md)); data panel: [threshold](docs/validation/ants-threshold-image-panel.md), [multiply](docs/validation/ants-multiply-images-panel.md), [smooth](docs/validation/ants-smooth-image-panel.md), [resample](docs/validation/ants-resample-image-panel.md), [by spacing](docs/validation/ants-resample-image-by-spacing-panel.md), every compared case bit-identical |
| antsRegistration (every stage: linear, SyN, B-spline SyN, B-spline, displacement-field, exponential and time-varying velocity-field; image and point-set metrics) | `larmorx ants antsRegistration`, `lx.ants.registration` | port | [513 cases](docs/validation/ants-registration.md): 355 of 399 compared identical to ANTs (outputs and messages), 44 more with correctly rounded maths; [the corrections' effects](docs/validation/ants-registration-corrections.md); data panel: [71 compared](docs/validation/ants-registration-panel.md), 16 identical as built, 55 more with correctly rounded maths, and [the 14 whole template registrations](docs/validation/ants-registration-panel-template.md) at full size, 14 of 14 identical with correctly rounded maths |
| antsAI, antsAffineInitializer | `larmorx ants antsAI`, `larmorx ants antsAffineInitializer` | port | [antsAI](docs/validation/ants-ai.md): 76 of 76 compared identical (75 as built, 1 with correctly rounded maths), the blob alignment (`-b`) included (its corrections, ANTS-116 among them, by default); [antsAffineInitializer](docs/validation/ants-affine-initializer.md): 26 of 26; data panel: [antsAI](docs/validation/ants-ai-panel.md) 40 of 40, [antsAffineInitializer](docs/validation/ants-affine-initializer-panel.md) 14 of 14 |
| Atropos (every initialization, likelihood model, MRF, outlier and partial-volume option) | `larmorx ants Atropos`, `lx.ants.atropos` | port | [97 cases](docs/validation/ants-atropos.md): 27 of 58 compared bit-identical to ANTs, 58 of 58 with correctly rounded maths, sMRIPrep's call on the five oracle runs included; partial-volume classes with the MRF corrected (ANTS-106); data panel: [3 of 17](docs/validation/ants-atropos-panel.md) as built, 17 of 17 with correctly rounded maths, the classes identical in all |
| fMRIPrep's one-shot BOLD resampler | `lx.transforms.resample_series` | port (fMRIPrep, nitransforms, SciPy) | [139 cases](docs/validation/resample-series.md): 136 of 136 compared bit-identical |
| 3dTshift (slice timing) | `larmorx afni 3dTshift`, `lx.afni.tshift` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [280 of 280](docs/validation/afni-tshift.md) compared agree, 178 bit-identical, [2–6× faster](docs/benchmarks/afni-tshift.md); replica: [280 of 280](docs/validation/afni-tshift-replica.md) bit-identical |
| 3dAutomask (brain mask) | `larmorx afni 3dAutomask`, `lx.afni.automask` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [130 cases](docs/validation/afni-automask.md), 92 of 106 compared cases that write outputs bit-identical to AFNI (the other 14: inputs carrying AFNI's own header extension, AFNI-10: the data bit-identical, the header and messages AFNI's for the input without it), fMRIPrep's call on every oracle run's BOLD reference included, the data panel's too; replica: [101 of 101](docs/validation/afni-automask-replica.md) compared cases that write outputs bit-identical |
| 3dUnifize (intensity uniformization) | `larmorx afni 3dUnifize`, `lx.afni.unifize` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [129 cases](docs/validation/afni-unifize.md), none failing: 44 of 99 compared cases that write outputs bit-identical to AFNI, the other 55 within 1e-5 (at most 8.7e-8 of the largest value) and bit-identical to AFNI run with correctly rounded maths (glibc's `tanhf` in AFNI's squash), fMRIPrep's call on every oracle run included; 4 to 25 times faster than AFNI (informal timings); replica: [97 of 97](docs/validation/afni-unifize-replica.md) compared cases that write outputs bit-identical at 1, 2 and 6 threads |
| Head-motion correction (mcflirt) | `larmorx mri hmc`, `larmorx fsl mcflirt`, `lx.mri.hmc` | clean-room original, `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial) | original: [compared with FSL's mcflirt](docs/validation/mri-hmc.md): 145 of 146 compared cases within thresholds; [5–16× faster](docs/benchmarks/mri-hmc.md) with threads; replica: [205 of 205](docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built, FSL 6.0.7.23 (the recorded runs, fMRIPrep's calls on every oracle run with BOLD runs, the data panel's included, and the edge cases; 32 more rejected by both, alike; [173 of 173](docs/validation/fsl-mcflirt-replica-full.md) in the full tier against FSL 6.0.7.17), fMRIPrep's own matrices identical |
| BET (brain extraction; the skull and scalp surfaces) | `larmorx fsl bet`, `larmorx fsl bet2`, `larmorx fsl betsurf`, `lx.fsl.bet`, `lx.fsl.betsurf` | clean-room original (`bet2`, the `bet` script and `betsurf`), `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial) | original: [127 of 128](docs/validation/fsl-bet.md) compared cases within thresholds against FSL 6.0.7.23, fMRIPrep's 29 BET calls on every oracle run with BOLD runs bit-identical; bet2's and bet's brains, masks, outlines, meshes and skulls FSL's bit for bit, the surfaces that rerun after a self-intersecting first pass included; betsurf's profiles decided as FSL's, its masks within its thresholds on 4 of 5 real heads (Dice 0.9907 to 0.9997); [2.5 to 3.6 times faster than FSL on one thread, 6.5 to 13.5 times on 24, betsurf included](docs/benchmarks/fsl-bet.md); replica: [130 of 130](docs/validation/fsl-bet-replica.md) compared cases bit-identical to FSL 6.0.7.23, every file, exit status and message ([129 of 129](docs/validation/fsl-bet-replica-full.md) in the full tier against FSL 6.0.7.17, the data panel's T1w images included), fMRIPrep's 29 BET calls on every oracle run with BOLD runs included; 31 more rejected by both, alike; 12 where FSL crashes and `reproduce` fills what it leaves undefined; 1 where FSL's fill reads memory beyond the volume (FSL-43, undefined); `bet -S`, `-B`, `-A`, `-A2` pending (FLIRT, FAST) |
| FLIRT (affine registration, boundary-based registration; convert_xfm and FLIRT's other programs) | `larmorx fsl flirt`, `larmorx fsl convert_xfm`, `larmorx fsl avscale`, `larmorx fsl img2imgcoord`, `larmorx fsl std2imgcoord`, `larmorx fsl img2stdcoord`, `larmorx fsl rmsdiff`, `larmorx fsl makerot`, `larmorx fsl midtrans`, `larmorx fsl pointflirt`, `larmorx fsl applyxfm4D`, `larmorx fsl pairreg`, `larmorx fsl extracttxt`, `larmorx fsl aff2rigid`, `lx.fsl.flirt`, `lx.fsl.convert_xfm`, `lx.fsl.avscale`, `lx.fsl.img2imgcoord`, `lx.fsl.std2imgcoord`, `lx.fsl.img2stdcoord`, `lx.fsl.rmsdiff`, `lx.fsl.makerot`, `lx.fsl.midtrans`, `lx.fsl.pointflirt`, `lx.fsl.applyxfm4d`, `lx.fsl.pairreg`, `lx.fsl.extracttxt`, `lx.fsl.aff2rigid` | clean-room original of every program, `experimental` (parts 1 and 2: `flirt` with `-debug` and every verbose level, `convert_xfm`, avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D and the scripts pairreg, extracttxt, aff2rigid; fMRIPrep's BBR call 12 to 14 times FSL's speed on 14 threads); exact replica of every program in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (parts 1 to 3); both: every program and script of fsl-flirt but `-warp`, `standard_space_roi`, `flirt_average`, `epi_reg` | original: [206 of 214](docs/validation/fsl-flirt.md) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's three boundary-based calls within 0.02 mm at the 95th percentile of the brain, `-applyxfm` FSL's images bit for bit with trilinear and nearest neighbour, the other programs' printed results and files FSL's but for the sinc and spline kernels' last bits, every verbose level and `-debug`, the schedules' and the command line's messages FSL's), 85 more rejected by both alike, 6 where FLIRT crashes or never finishes; the 8 it misses are in its record (ill-posed registrations whose paths part in the costs' last bits, FSL-91 and FSL-92 reproduced); replica: [209 of 209](docs/validation/fsl-flirt-replica.md) compared cases bit-identical to FSL 6.0.7.23 (matrices, images, printed results, exit status and messages): fMRIPrep's boundary-based calls on three oracle runs (their matrices also identical as text to fMRIPrep's own), every cost function, the search options, every shipped schedule and every schedule command (`alignpaxes`, any parameter-subset mask), `-anglerep quaternion`, `-debug`, `-applyxfm` 3D and 4D with every interpolation and data type, BBR with its types, coordinates and a field map, avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D in every data type and interpolation, pairreg, extracttxt and aff2rigid; 87 more rejected by both, alike; 6 where FLIRT crashes or never finishes (FSL-71, FSL-72, FSL-75, FSL-93) |
| FAST (tissue segmentation: classes, partial volumes, bias field) | `larmorx fsl fast`, `lx.fsl.fast` | clean-room original, `experimental` (2026-11-03: every option, message and failure case, one channel or any number of them, FSL's tissue priors registered by FLIRT's clean-room original in the same process; fMRIPrep's call 9 to 11 times FSL's speed on one thread, 47 to 67 times on 14, the same bits at any thread count); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` | original: [60 of 61](docs/validation/fsl-fast.md) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's call on 3 oracle runs bit for bit what FSL computes with correctly rounded maths, every single-channel case FSL's values or FSL's with correctly rounded maths, several channels within FSL's own difference between the two but for a few partial volumes one fraction step away), 25 more rejected by both alike, 1 where FSL reads past the smaller of channels of different sizes (FSL-104, refused); the 1 it misses is in its record (five channels with nearly singular covariances, decided by the matrix arithmetic's last bits); replica: [59 of 59](docs/validation/fsl-fast-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every image written, the printed messages, the exit status): fMRIPrep's call on 3 oracle runs (an adult, a 3.5-year-old, a glioma), every option, one to eight channels, FSL's tissue priors registered by FLIRT's replica, the image types, 2D images; 26 more rejected by both alike; 1 where FSL reads and writes past the smaller of channels of different sizes (FSL-104, refused) |
| topup and applytopup (the susceptibility field from reversed phase encodings, and its correction) | `larmorx fsl topup`, `larmorx fsl applytopup`, `lx.fsl.topup`, `lx.fsl.applytopup` | clean-room original, `experimental` (2026-11-03: every option, message and failure case of both programs, applytopup's undocumented vb2D, vb3D and vb4D included; fMRIPrep's call 10 to 12 times FSL's speed on one thread, 39 to 41 times on 14, the same bits at any thread count); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03: the same programs, options and methods) | original: [92 of 92](docs/validation/fsl-topup.md) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's call on 2 oracle runs: ds001600 within FSL's own thread band in every metric, ds005040 in every one but the field's correlation, 0.999958 against FSL's own 0.999966), 66 more rejected by both alike, 22 expected divergences, each with its reason (estimated movements that do not converge, where FSL's rotation derivatives are not its cost's; FSL-133's lines; FSL-143's diverging iterations, reproduced; inputs refused for FSL-124 and FSL-134); replica: [104 of 104](docs/validation/fsl-topup-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every file written, the printed messages, the exit status): fMRIPrep's call on 2 oracle runs, every option, FSL's configurations, applytopup's five methods; 66 more rejected by both alike; 3 refused where FSL needs more memory than any machine holds (a negative --warpres, FSL-124) or reads a field on another grid past its lines (FSL-134) |
| fslmaths (image arithmetic, thresholds, filters, reductions, statistics, TFCE) | `larmorx fsl fslmaths`, `lx.fsl.fslmaths`, `lx.fsl.smooth`, `lx.fsl.threshold` | exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03: every operation, option, data type, message and failure case); the clean-room original is to be written from `specs/fslmaths.md` | replica: [358 of 358](docs/validation/fsl-maths-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every file written, the printed messages, the exit status): fMRIPrep's 18 calls (combine_masks on three oracle runs, the goodvoxels mask's fourteen), every operation in every data type; 61 more rejected by both alike; 2 one unit in the last place where FSL's glibc 2.35 is not correctly rounded and the replica is (GLIBC-07: `-dt double -tan`, an FFT's twiddle factors) |
| fslstats (image statistics: means, extremes, percentiles, histograms, per volume and label, JSON) | `larmorx fsl fslstats`, `lx.fsl.fslstats`, `lx.fsl.image_mean`, `lx.fsl.image_std` | exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03: every option, `-t`, `-K` and `-json`, every message and failure case; BET's replica runs it); the clean-room original is to be written from `specs/fslstats.md` | replica: [172 of 172](docs/validation/fsl-stats-replica.md) compared cases printing FSL 6.0.7.23's text byte for byte (with its messages and exit status): fMRIPrep's 4 goodvoxels calls, BET's call, FSL's own tests, every option; 36 more rejected by both alike |
<!-- END GENERATED: tools table -->

## Roadmap

The ordered plan is in [docs/next-steps.md](docs/next-steps.md). In outline:

1. **AFNI:** 3dQwarp, 3dSkullStrip,
   3dcalc and an analysis track (3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit,
   3dFWHMx/3dClustSim, ...).
2. **FSL-compatible tools:** `fslmaths`' and `fslstats`' clean-room originals (the mcflirt, BET,
   FLIRT, FAST, topup, `fslmaths` and `fslstats` replicas and the clean-room originals of BET, FLIRT,
   FAST and topup are done).
3. **sdcflows' field-map estimation, FreeSurfer** (`larmorx-freesurfer`), Connectome
   Workbench- and MSM-compatible surface tools, the confounds.
4. **larmorprepx:** an independent, fMRIPrep-compatible preprocessing pipeline (BIDS in,
   fMRIPrep-compatible derivatives and reports out), written in plain Python on the library,
   then compared with fMRIPrep end to end.
5. **Releases on PyPI** for all six platforms.

| Document | Contents |
|---|---|
| [docs/overview.md](docs/overview.md) | the project in plain terms: replicas and clean-room originals, packages, installation, what is published |
| [docs/licensing.md](docs/licensing.md) | how each upstream's licence is handled |
| [docs/next-steps.md](docs/next-steps.md) | the plan of the next tasks |
| [PLAN.md](PLAN.md) | the overall plan |
| [docs/decision-log.md](docs/decision-log.md) | decisions so far |
| [docs/analysis/](docs/analysis/) | what fMRIPrep computes, in detail |
| [CLAUDE.md](CLAUDE.md) | project context and rules for contributors |

## Development

### Setting up a development workspace
The repo expects to live inside a workspace directory, next to read-only clones of the upstream projects it is based on:

```bash
mkdir -p ~/work/super_fmriprep && cd ~/work/super_fmriprep
git clone https://github.com/karellopez/larmorx.git
python3.12 -m venv .venv           # Python >= 3.12
larmorx/scripts/bootstrap_workspace.sh --venv .venv --install-rust
```

The script:
- shallow-clones every upstream listed in [upstream.tsv](upstream.tsv) at its pinned commit (about 2 GB; `--skip freesurfer,afni` saves about 1.1 GB)
- installs Rust with rustup (user-level)
- installs the evaluation packages into the venv

### Building and testing
You need Rust (stable, via rustup) and Python ≥ 3.12. From the repo, with the workspace venv active:

```bash
source ../.venv/bin/activate
pip install --group dev          # maturin, pytest, packaging, nibabel, ruff (pip >= 25.1)
maturin develop --release        # build the larmorx._core extension into the venv
cargo test --workspace           # Rust tests
pytest tests/python tests/golden # Python tests and the cross-platform golden tests
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
ruff check && ruff format --check
```

The FSL replicas are a separate Cargo workspace: `cd crates-nc && cargo test --workspace`; `crates-gpl` holds only verification tools (`crates-gpl/README.md`)
(build its wheel with `maturin build --release`).

Parity checks and benchmarks need the test data (`larmorx-testdata`, github.com/karellopez/larmorx-testdata, kept next to this one; its data panel, `docs/panel.md` there, is the fixed set every tool is validated on), the validation package and the original programs as oracles (`scripts/build_ants_oracle.sh`, `scripts/build_afni_oracle.sh`):

```bash
pip install -e ../larmorx-testdata -e "validation[oracles]"
pytest tests/parity                                             # parity with the reference tools (smoke tier)
python -m larmorx_validation parity nifti-io --tier standard --out docs/validation
python -m larmorx_validation bench nifti-io --out docs/benchmarks
python -m larmorx_validation difftest --tools all --budget 6 --jobs 4   # a night of random testing
python -m larmorx_validation tables --check                     # the generated tables are current
```

| Path | Contents |
|---|---|
| `crates/larmorx-core`, `crates/larmorx-io`, `crates/larmorx-hdf5` | shared foundation: images, grids, linear algebra, correctly rounded maths, NIfTI and transform files, HDF5 in pure Rust |
| `crates/larmorx-transform`, `crates/larmorx-interp`, `crates/larmorx-image` | ITK transforms, interpolators and image filters |
| `crates/larmorx-registration`, `crates/larmorx-ants` | ITK's v4 registration framework; the ANTs programs |
| `crates/larmorx-afni`, `crates/larmorx-mri` | clean-room originals: 3dTshift, 3dAutomask, 3dUnifize; head-motion correction |
| `crates-gpl/` | the GPL-3.0-or-later workspace, reserved for Workbench's replicas; today the AFNI replicas' verification tools (`crates-gpl/README.md`) |
| `crates/larmorx-afni-replica` | the AFNI replicas (3dTshift, 3dAutomask, 3dUnifize), Apache-2.0 with CC BY 4.0's attribution |
| `crates-nc/` | the FSL replicas (`larmorx-nc`, FSL Licence, non-commercial: mcflirt, BET; the shared core `larmorx-nc-core`), a separate Cargo workspace with FSL's original source in `fsl-source/` |
| `crates/larmorx-cli` | the multicall CLI; standalone `larmorx` and `lx` binaries |
| `crates/larmorx-py` | the `larmorx._core` extension module (PyO3, abi3 for CPython ≥ 3.12) |
| `python/larmorx` | the Python package, including `pipelines/larmorprepx` |
| `tests/python`, `tests/golden`, `tests/parity` | Python tests; cross-platform golden tests; parity suites under pytest |
| `validation/` | parity suites and benchmarks against the reference tools |
| `specs/` | behavioural specifications, the input of clean-room originals |
| `docs/` | [architecture](docs/architecture.md), [API pages](docs/api/), validation records, benchmark reports, [findings](docs/findings/) about the tools reimplemented |
| `.github/workflows/ci.yml` | lint, plus wheel build and tests on the six target platforms |

## Licence

Apache-2.0 for LARMORX itself. Replicas of restrictively licensed tools are separate packages
under their upstream's licence (`larmorx-nc`: the FSL Licence). The AFNI replicas carry CC BY
4.0's attribution for the Medical College of Wisconsin's code ([NOTICE](NOTICE), `LICENSES/`); see
[docs/licensing.md](docs/licensing.md). LARMORX is an independent project, not affiliated with
or endorsed by the developers of ANTs, AFNI, FSL, FreeSurfer or fMRIPrep.

## Contributing
- Commit messages, PR descriptions and release notes must not contain AI co-author trailers (`Co-Authored-By: Claude …`) or "Generated with …" lines.
- Follow the rules in [CLAUDE.md](CLAUDE.md): clean-room policy, provenance records, and the per-tool contract.
