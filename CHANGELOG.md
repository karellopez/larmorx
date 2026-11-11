# Changelog

The changes of each release of larmorx, larmorx-nc and larmorx-gpl (one version for the three
packages). Versions are written as PyPI writes them ([PEP 440](https://peps.python.org/pep-0440/):
`0.1.0rc1`, `0.1.0`); each release's tag adds a `v` (`v0.1.0rc1`). The form follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). How a release is made:
[RELEASING.md](RELEASING.md).

## [Unreleased]

The changes on `main` since 0.1.0rc1, for the next release.

### Added

- **larmorx's own command-line options**, between `larmorx` (or `lx`) and the family, so that
  they never clash with the original programs' options:
  `larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects fix|reproduce] <family> <tool> ...`
  (`--opt value` or `--opt=value`). Each has the name and the values of the Python argument
  (`n_threads`, `implementation`, `upstream_defects`) and replaces an environment variable
  (`OMP_NUM_THREADS`, for ANTs' tools `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` and `NSLOTS`;
  `LARMORX_IMPLEMENTATION`; `LARMORX_UPSTREAM_DEFECTS`), which keeps working: an option given
  wins over its variable, with the same results. `--n-threads 0`, the command line's default, is
  every logical CPU. A tool whose original has its own thread option (tedana's
  `t2smap --n-threads`, sdcflows' `--nprocs` and `--omp-nthreads`, FSL's `topup --nthr`) uses it
  when it is given, else `--n-threads`. A wrong or missing value, or an option given twice, is a
  usage error (exit status 2); `larmorx --help` lists the options.
- **The companion programs** `larmorx-nc` and `larmorx-gpl` take the same options before the
  family: `--n-threads`, `--upstream-defects`, and `--implementation replica` (their one
  implementation; `auto`, `clean-room` and `original` are refused with exit status 2).
- **The tools' messages while they run**, in the console script pip installs (`larmorx`, `lx`) as
  in the standalone program: until now they appeared when the tool ended, so a long
  `antsRegistration -v` looked frozen.
- **First steps** in the README: where pip puts the commands (`<venv>/bin`, `<venv>\Scripts`,
  the `sysconfig` line, `python -m larmorx`), the three options with the variables they replace,
  an example from start to end for Linux and macOS and one for Windows PowerShell, and what the
  `upstream defect ... corrected` notices mean.

### Changed

- **The clean-room originals are called `clean-room`**, as "the original" means FSL's or AFNI's
  own program: `implementation="clean-room"` in Python, `--implementation clean-room` and
  `LARMORX_IMPLEMENTATION=clean-room` on the command line, `Implementation(kind='clean-room', ...)`
  in a result, and `++ implementation: clean-room (larmorx ...)` in AFNI's `-verbose` line.
- **topup's replica without `--nthr`, and `t2smap` without its own `--n-threads`**, run on the
  thread count given (`--n-threads`, else `OMP_NUM_THREADS`, else every logical CPU) instead of
  one thread; their results are the same bytes at every thread count.
- **PyPI's author** of the three packages: Karel Mauricio López Vilaret, with the e-mail address
  `CITATION.cff` gives.
- **FSL's tools without `FSLOUTPUTTYPE`** (FSL-11): every FSL tool's clean-room original writes
  its images as `.nii.gz` (`NIFTI_GZ`) when the variable is unset, as `flirt` and `mcflirt`
  already did, and names FSL-11 on one line per command; FSL's own programs stop at their first
  image (exit 1), which `--upstream-defects reproduce` gives. Setting the variable is optional.

### Deprecated

- `original` as an implementation's name, the spelling of 0.1.0rc1: it still works, with a
  `DeprecationWarning` in Python and silently on the command line and in
  `LARMORX_IMPLEMENTATION`.

### Fixed

- The copyable shell blocks of the README, the PyPI descriptions and the API pages hold no
  comments: zsh, macOS's default shell, does not take a `#` typed at its prompt as a comment, so
  `pip install larmorx  # the library ...` also asked pip for packages named `#`, `the`, ... A
  test keeps comments out of them.
- The FSL-09 notice (an integer output rounded instead of truncated) of `flirt`, `applyxfm4D`
  and `applytopup` names the program that wrote it; it named `mcflirt`.
- `scripts/release/check_install.py` keeps pip's temporary files and cache in its work folder:
  inside Apptainer's `--containall`, `/tmp` and the home folder are a 64 MiB space that pip
  filled ("No space left on device").

## [0.1.0rc1] - 2026-11-11

The first release, as a **release candidate**: a pre-release, published to check that the
packages install and work on every platform before 0.1.0. LARMORX is in early development: the
tools below are complete and compared case by case with the original programs, the pipeline
`larmorprepx` is not ready yet, and FreeSurfer's tools are still to come.

### What is in it

- **Three packages on PyPI, at one version:**
  - `larmorx` (Apache-2.0): the Python library (`import larmorx as lx`), the commands `larmorx`
    and `lx`, which take the original programs' arguments (`larmorx ants antsRegistration ...`),
    and `larmorprepx` (a preview, below). It holds the ports of ANTs 2.6.5 and ITK 5.4.5, of
    sdcflows 2.17.1 with niworkflows' helpers and fMRIPrep's resampler and field-map steps; its own
    NIfTI-1/2 reading and writing (compared with nibabel's); the clean-room originals of AFNI's
    and FSL's tools; and the exact replicas of AFNI 26.3.05's tools (with CC BY 4.0's
    attribution).
  - `larmorx-nc` (the FSL Licence: **non-commercial use only**): exact replicas of FSL
    6.0.7.23's `mcflirt`, BET (`bet`, `bet2`, `betsurf`, `standard_space_roi`, `fslroi`), FLIRT
    and its programs, FAST, `topup` and `applytopup`, `fslmaths`, `fslstats` and `prelude`, with
    the FSL files they read (three standard images, topup's settings files).
  - `larmorx-gpl` (the GNU GPL version 3): the exact replica of tedana 26.0.3's `t2smap`.
  - The extras: `larmorx[prep]` adds what `larmorprepx` will need (templateflow, matplotlib,
    nilearn, pandas, jinja2); `larmorx[all]` adds `[prep]` and the two companions at the same
    version.
- **Wheels for six platforms:** Linux x86-64 and aarch64 (glibc 2.28 or newer), macOS 11 or
  newer (Intel and Apple silicon) and Windows 10 and 11 (x64 and arm64; the C runtime is built
  in, no Visual C++ installation needed). One wheel per platform serves Python 3.12 and every
  newer version (CPython's stable ABI); tested on 3.12 on every platform, and on 3.13 and 3.14 on
  Linux. The source distributions of the three packages build on other systems with Rust and a C
  compiler.
- **Standalone programs** on the GitHub Release, for each platform: `larmorx` (with `lx`),
  `larmorx-nc` and `larmorx-gpl` as archives that need no Python, each with its licences
  (`larmorx-nc`'s with FSL's original and amended source, as its licence requires).
- **Two container images** for linux/amd64 and linux/arm64, usable by Apptainer, Docker and
  Podman: `ghcr.io/karellopez/larmorx:0.1.0rc1` (larmorx with `[prep]`, without the replica
  packages: commercial use allowed) and `ghcr.io/karellopez/larmorx:0.1.0rc1-all` (also
  larmorx-gpl and larmorx-nc: non-commercial use only).
- **Checks for every file:** its SHA-256 in the release's `SHA256SUMS`, and its build
  provenance (GitHub's attestations, PyPI's for its uploads).
- `larmorx --version`, `larmorx-nc --version` and `larmorx-gpl --version` print the version as
  PyPI writes it (`0.1.0rc1`).

### The tools and their status

Every tool has a validation record that compares it with the original program, case by case
(the [validation index](docs/validation/README.md); the numbers are in the README's
[tool table](README.md#supported-tools)). A record's status means:

- `validated`: every case of its comparison is within its thresholds (bit-identical, for an
  exact replica);
- `experimental`: the tool runs and is compared with the original, but not every case is within
  its thresholds yet, or its comparison is not complete. A tool can be `experimental` although
  every compared case agrees: its record says what it has been compared on. The final validation
  phase (the fixed data panel, the full test tiers, long random-testing runs), still to come,
  settles the rest.

A record's name ending in `-replica` is the exact replica's (in larmorx itself for AFNI's tools,
in `larmorx-nc` for FSL's, in `larmorx-gpl` for tedana's); `-full` is a record's full tier; the
other records are the port's or the clean-room original's.

<!-- BEGIN statuses: validation/tools.toml (scripts/release/changelog.py VERSION --write-statuses) -->
| Tool | Validation records and their status |
|---|---|
| NIfTI-1/2 I/O | [nifti-io](docs/validation/nifti-io.md): `validated` |
| antsApplyTransforms | [ants-apply-transforms](docs/validation/ants-apply-transforms.md): `validated` |
| ITK transform files (`.mat`, `.txt`, `.h5`) | [itk-hdf5](docs/validation/itk-hdf5.md): `validated` |
| N4BiasFieldCorrection | [ants-n4](docs/validation/ants-n4.md): `validated` |
| DenoiseImage | [ants-denoise](docs/validation/ants-denoise.md): `validated` |
| ImageMath (every operation fMRIPrep, sMRIPrep and sdcflows call) | [ants-image-math](docs/validation/ants-image-math.md): `validated` |
| ThresholdImage, MultiplyImages, SmoothImage, ResampleImage, ResampleImageBySpacing | [ants-threshold-image](docs/validation/ants-threshold-image.md): `validated`; [ants-multiply-images](docs/validation/ants-multiply-images.md): `validated`; [ants-smooth-image](docs/validation/ants-smooth-image.md): `validated`; [ants-resample-image](docs/validation/ants-resample-image.md): `validated`; [ants-resample-image-by-spacing](docs/validation/ants-resample-image-by-spacing.md): `validated` |
| antsRegistration (every stage: linear, SyN, B-spline SyN, B-spline, displacement-field, exponential and time-varying velocity-field; image and point-set metrics) | [ants-registration](docs/validation/ants-registration.md): `validated`; [ants-registration-corrections](docs/validation/ants-registration-corrections.md): `validated` |
| antsAI, antsAffineInitializer | [ants-ai](docs/validation/ants-ai.md): `validated`; [ants-affine-initializer](docs/validation/ants-affine-initializer.md): `validated` |
| Atropos (every initialization, likelihood model, MRF, outlier and partial-volume option) | [ants-atropos](docs/validation/ants-atropos.md): `validated` |
| fMRIPrep's one-shot BOLD resampler | [resample-series](docs/validation/resample-series.md): `validated` |
| sdcflows' field-map estimation (the B-spline field fit, the field evaluated and applied, the phase-difference preparation) | [sdcflows-fieldmap](docs/validation/sdcflows-fieldmap.md): `validated` |
| sdcflows' PEPOLAR, direct and fieldmap-less estimation (the masks, the estimators of a BIDS dataset, the derivatives, the BIDS app; niworkflows' helpers; fMRIPrep's own field-map steps) | [sdcflows-estimation](docs/validation/sdcflows-estimation.md): `validated`; [sdcflows-estimation-full](docs/validation/sdcflows-estimation-full.md): `validated`; [sdcflows-estimation-app-shared](docs/validation/sdcflows-estimation-app-shared.md): `validated`; [niworkflows-helpers](docs/validation/niworkflows-helpers.md): `validated`; [ndimage-skimage](docs/validation/ndimage-skimage.md): `validated`; [ndimage-skimage-full](docs/validation/ndimage-skimage-full.md): `validated` |
| tedana's t2smap (the T2* and S0 maps of multi-echo fMRI and the echoes' optimal combination) | [tedana-t2smap-replica](docs/validation/tedana-t2smap-replica.md): `experimental` |
| 3dTshift (slice timing) | [afni-tshift](docs/validation/afni-tshift.md): `validated`; [afni-tshift-replica](docs/validation/afni-tshift-replica.md): `validated` |
| 3dAutomask (brain mask) | [afni-automask](docs/validation/afni-automask.md): `validated`; [afni-automask-replica](docs/validation/afni-automask-replica.md): `validated` |
| 3dUnifize (intensity uniformization) | [afni-unifize](docs/validation/afni-unifize.md): `validated`; [afni-unifize-replica](docs/validation/afni-unifize-replica.md): `validated` |
| 3dvolreg (rigid-body registration of a series) | [afni-volreg-original](docs/validation/afni-volreg-original.md): `experimental`; [afni-volreg-replica](docs/validation/afni-volreg-replica.md): `validated` |
| Head-motion correction (mcflirt) | [mri-hmc](docs/validation/mri-hmc.md): `experimental`; [fsl-mcflirt-replica](docs/validation/fsl-mcflirt-replica.md): `validated`; [fsl-mcflirt-replica-full](docs/validation/fsl-mcflirt-replica-full.md): `validated` |
| BET (brain extraction; the skull and scalp surfaces) | [fsl-bet](docs/validation/fsl-bet.md): `experimental`; [fsl-bet-replica](docs/validation/fsl-bet-replica.md): `validated`; [fsl-bet-replica-full](docs/validation/fsl-bet-replica-full.md): `validated` |
| FLIRT (affine registration, boundary-based registration; convert_xfm and FLIRT's other programs) | [fsl-flirt](docs/validation/fsl-flirt.md): `experimental`; [fsl-flirt-replica](docs/validation/fsl-flirt-replica.md): `experimental` |
| FAST (tissue segmentation: classes, partial volumes, bias field) | [fsl-fast](docs/validation/fsl-fast.md): `experimental`; [fsl-fast-replica](docs/validation/fsl-fast-replica.md): `experimental` |
| topup and applytopup (the susceptibility field from reversed phase encodings, and its correction) | [fsl-topup](docs/validation/fsl-topup.md): `experimental`; [fsl-topup-replica](docs/validation/fsl-topup-replica.md): `experimental` |
| fslmaths (image arithmetic, thresholds, filters, reductions, statistics, TFCE) | [fsl-maths](docs/validation/fsl-maths.md): `experimental`; [fsl-maths-replica](docs/validation/fsl-maths-replica.md): `experimental` |
| fslstats (image statistics: means, extremes, percentiles, histograms, per volume and label, JSON) | [fsl-stats](docs/validation/fsl-stats.md): `experimental`; [fsl-stats-replica](docs/validation/fsl-stats-replica.md): `experimental` |
| prelude (phase unwrapping of field maps' phase differences and complex images) | [fsl-prelude](docs/validation/fsl-prelude.md): `experimental`; [fsl-prelude-replica](docs/validation/fsl-prelude-replica.md): `experimental` |
<!-- END statuses -->

### Known limitations

- **`larmorprepx` is not ready yet.** The command exists and says so: its field-map stages exist
  as Python functions, but the anatomical (FreeSurfer) and BOLD stages, the confounds and the
  final validation come in later releases ([the plan](docs/next-steps.md)).
- **Not yet ported:** FreeSurfer (the planned `larmorx-freesurfer`), Connectome Workbench- and
  MSM-like surface tools, AFNI's other tools (3dQwarp, 3dSkullStrip, ...), the confounds.
- **tedana's `t2smap` needs `larmorx-gpl`:** its clean-room original is not written yet, so
  without `larmorx-gpl` `lx.tedana.t2smap` says how to install it.
- **Installing `larmorx-nc` changes what the FSL tools run by default.** With it installed,
  `implementation="auto"` (the default) runs the FSL replicas, so FSL's non-commercial terms then
  cover the default results; `implementation="original"` (`LARMORX_IMPLEMENTATION=original` on the
  command line) runs the clean-room originals instead.
- **Python:** CPython 3.12 or newer; the free-threaded builds of Python are not supported.
- **Linux:** no wheels for musl-based systems (such as Alpine Linux) or for glibc older than 2.28;
  the container images run anywhere Apptainer, Docker or Podman does.
- **macOS:** larmorx and its companions run on macOS 11 or newer; on Apple silicon `[prep]` and
  `[all]` need macOS 12 or newer, because SciPy's and scikit-learn's wheels do.
- **Python 3.13 and newer with `[prep]`:** pip installs a pre-release of wrapt (2.6.0.dev1 at the
  time of writing), because formulaic, which pybids needs, asks for `wrapt>=1.17.0rc1` there;
  every test passes with it.
- **Builds from source** (the sdists) need Rust 1.99 or newer, which maturin downloads by itself
  when none is installed, and a C compiler (gcc or clang; Microsoft's C++ build tools on
  Windows). On Windows such a build links Visual C++'s runtime dynamically, where the published
  wheels and programs have it built in.
- **Two messages still show the version in Cargo's form**, `0.1.0-rc.1`: the `++ implementation:`
  line of the AFNI programs, and the `"Python"` field of the `dataset_description.json` that
  `t2smap` writes. From 0.1.0 on both forms are the same.

[Unreleased]: https://github.com/karellopez/larmorx/compare/v0.1.0rc1...HEAD
[0.1.0rc1]: https://github.com/karellopez/larmorx/releases/tag/v0.1.0rc1
