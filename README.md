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
| 3dUnifize (intensity uniformization) | `larmorx afni 3dUnifize`, `lx.afni.unifize` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [117 cases](docs/validation/afni-unifize.md), none failing: 37 of 92 compared cases that write outputs bit-identical to AFNI, the other 55 within 1e-5 (at most 8.7e-8 of the largest value) and bit-identical to AFNI run with correctly rounded maths (glibc's `tanhf` in AFNI's squash), fMRIPrep's call on every oracle run included; 4 to 25 times faster than AFNI (informal timings); replica: [90 of 90](docs/validation/afni-unifize-replica.md) compared cases that write outputs bit-identical at 1, 2 and 6 threads |
| Head-motion correction (mcflirt) | `larmorx mri hmc`, `larmorx fsl mcflirt`, `lx.mri.hmc` | clean-room original, `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial) | original: [compared with FSL's mcflirt](docs/validation/mri-hmc.md): 121 of 122 compared cases within thresholds; [5–16× faster](docs/benchmarks/mri-hmc.md) with threads; replica: [181 of 181](docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built, FSL 6.0.7.23 (the recorded runs, fMRIPrep's calls on every oracle run with BOLD runs, the data panel's included, and the edge cases; 22 more rejected by both, alike; [173 of 173](docs/validation/fsl-mcflirt-replica-full.md) in the full tier against FSL 6.0.7.17), fMRIPrep's own matrices identical |
| BET (brain extraction; the skull and scalp surfaces) | `larmorx fsl bet`, `larmorx fsl bet2`, `larmorx fsl betsurf`, `lx.fsl.bet`, `lx.fsl.betsurf` | exact replica in `larmorx-nc` (FSL Licence, non-commercial); clean-room original planned (`specs/bet.md`) | [130 of 130](docs/validation/fsl-bet-replica.md) compared cases bit-identical to FSL 6.0.7.23, every file, exit status and message ([129 of 129](docs/validation/fsl-bet-replica-full.md) in the full tier against FSL 6.0.7.17, the data panel's T1w images included), fMRIPrep's 29 BET calls on every oracle run with BOLD runs included; 31 more rejected by both, alike; 12 where FSL crashes and `reproduce` fills what it leaves undefined; 1 where FSL's fill reads memory beyond the volume (FSL-43, undefined); `bet -S`, `-B`, `-A`, `-A2` pending (FLIRT, FAST) |
<!-- END GENERATED: tools table -->

## Roadmap

The ordered plan is in [docs/next-steps.md](docs/next-steps.md). In outline:

1. **AFNI:** 3dQwarp, 3dSkullStrip,
   3dcalc and an analysis track (3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit,
   3dFWHMx/3dClustSim, ...).
2. **FSL-compatible tools:** FAST, FLIRT with BBR, topup, BET's clean-room original (the mcflirt
   and BET replicas are done).
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
