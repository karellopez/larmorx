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

**Status: early development; the first release is 0.1.0rc1, a pre-release** (a release
candidate, [CHANGELOG](CHANGELOG.md)). The tools listed under [Supported tools](#supported-tools)
are implemented and compared case by case with the original programs; everything else is
planned. The preprocessing pipeline `larmorprepx` is not ready yet: only its field-map stages
exist. The package, module and commands are spelled `larmorx` (and `larmorprepx` for the
pipeline).

## Core features

- **Rust implementations.** Pure Rust, with no C, C++ or Fortran library dependencies.
  By design, results do not depend on the thread count, and transcendental functions come from
  correctly rounded [CORE-MATH](https://core-math.gitlabpages.inria.fr/) ports (or the
  deterministic pure-Rust `libm` where not yet ported), so a tool gives the same bits on every
  platform. Why Rust, with Python on top, and why the original tools have no Windows version:
  [Why Rust and Python](https://karellopez.github.io/larmorx_documentation/why-rust-and-python/)
  on the documentation site.
- **Command-line compatibility.** `larmorx <family> <program> <the original's arguments>`
  (short alias `lx`), e.g. `larmorx ants antsApplyTransforms ...` or `larmorx afni 3dTshift ...`,
  accepting the original's options; how far messages and exit statuses match is stated in each
  tool's validation record. larmorx's own three options (threads, implementation, defect mode)
  go before the family ([First steps](#first-steps)).
- **Python bindings.** `import larmorx as lx`: the same tools with typed options, on in-memory
  images.
- **Cross-platform.** Continuous integration builds the wheels and runs the test suites on
  Linux (x86-64, aarch64), macOS (Intel, Apple silicon) and Windows (x64, arm64); golden tests
  check that every tool reproduces its Linux x86-64 output bit for bit on each platform
  ([golden tests](docs/validation/golden.md)).
- **Two ways of reimplementing, kept apart by licence** ([licensing](docs/licensing.md)):
  - tools with permissive licences (ANTs and ITK, Apache-2.0) are ported from their source,
    with each Rust function's origin recorded in the crate's `PROVENANCE.md`;
  - where the licence is restrictive (FSL, tedana, Workbench), a **clean-room original** is
    written from a behavioural specification, documentation and runs of the original program,
    never from its source; an exact replica, where one exists, is a separate package under the
    upstream's licence (`larmorx-nc` for FSL, `larmorx-gpl` for tedana's `t2smap`) that runs as a separate program. AFNI's tools have
    both inside larmorx since AFNI 26.3.05 released its MCW code under CC BY 4.0. Tools with both
    take `implementation="auto" | "replica" | "clean-room"`.
- **Validated, with documented differences.** Every tool has a validation record comparing it
  case by case with the original program ([docs/validation](docs/validation/)). Defects found
  in the originals are recorded ([register](docs/findings/upstream-defects.md)) and corrected by
  default with a warning; `upstream_defects="reproduce"` (`--upstream-defects reproduce` on the
  command line) gives the original's behaviour, and the parity tests run in that mode
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

- **Findable:** a public repository with versioned history, releases on PyPI and GitHub, and
  citation metadata ([CITATION.cff](CITATION.cff)).
- **Accessible:** open source under Apache-2.0; native wheels for six platforms that need no
  external neuroimaging installations, containers or administrator rights (container images
  are offered for those who prefer them).
- **Interoperable:** standard formats (NIfTI-1/2, ITK transform files including HDF5), the
  original tools' command-line arguments, and a Python API for pipelines.
- **Reusable:** a permissive licence, per-tool documentation, provenance records, case-by-case
  validation records and a register of the differences from the originals.

## Installation

```bash
pip install larmorx
```

installs the library and the commands `larmorx` and `lx` (Apache-2.0). Two extras add more:

- `pip install "larmorx[prep]"` adds what the `larmorprepx` pipeline will need (not ready yet);
- `pip install "larmorx[all]"` adds `[prep]` and the replica packages `larmorx-nc` and
  `larmorx-gpl`.

The quotation marks keep the square brackets from being read by the shell.

The first release, 0.1.0rc1, is a pre-release: pip installs a pre-release by itself only while
no final release exists; `pip install "larmorx==0.1.0rc1"` (or `pip install --pre larmorx`) asks
for it. `[all]` installs the three packages at the same version. Step by step, with a virtual
environment, the standalone programs and how to check a download: the documentation site's
[installation page](https://karellopez.github.io/larmorx_documentation/installation/).

| Package | What it holds | Its licence, in plain words |
|---|---|---|
| `larmorx` | every tool below: the ports, the clean-room originals and the AFNI replicas | [Apache-2.0](LICENSE): any use, commercial use included; keep the licence and the notices. The works it includes and their licences, permissive ones and the Mozilla Public License 2.0 for routines translated from Eigen (whose source stays public), are listed in [NOTICE](NOTICE). |
| `larmorx-nc` | the exact replicas of FSL's tools | the [FSL Licence](crates-nc/LICENSE): **non-commercial use only**. Once it is installed, the FSL tools' default (`implementation="auto"`) runs these replicas, so FSL's terms then cover your default results; `implementation="clean-room"` (`--implementation clean-room` on the command line) runs larmorx's own clean-room originals instead. |
| `larmorx-gpl` | the exact replica of tedana's `t2smap` | the [GNU GPL version 3](crates-gpl/LICENSE): any use; whoever passes it on, changed or not, must pass on its source under the same licence. |

The two replica packages are separate programs (`larmorx-nc`, `larmorx-gpl`) that larmorx runs as
separate processes, so their licences stay with them; each is installed only when asked for by
name or through `[all]`.

**Requirements.** Nothing else is needed: no ANTs, AFNI, FSL or tedana installation, no
compiler, no administrator rights. The wheels:

- **Python** 3.12 or newer (CPython; one wheel serves every version from 3.12 on; tested on 3.12
  on every platform, and on 3.13 and 3.14 on Linux); the free-threaded builds of Python are not
  supported.
- **Linux**, x86-64 and aarch64, with glibc 2.28 or newer (`manylinux_2_28`: for example RHEL 8,
  Debian 10, Ubuntu 20.04 and later); no wheels for musl-based systems such as Alpine Linux (use
  a container image).
- **macOS 11** or newer, Intel and Apple silicon; on Apple silicon `[prep]` and `[all]` need
  macOS 12, as SciPy's and scikit-learn's wheels do.
- **Windows** 10 and 11, x64 and arm64; Visual C++'s runtime is built in, nothing to install.

A build from the source distribution (where no wheel fits, or with `pip install --no-binary
larmorx larmorx`) needs a C compiler (gcc or clang; Microsoft's C++ build tools on Windows) and
Rust 1.99 or newer, which maturin downloads by itself when none is installed.

**Standalone programs.** Each release on [GitHub](https://github.com/karellopez/larmorx/releases)
also has `larmorx` (with `lx`), `larmorx-nc` and `larmorx-gpl` for each platform as archives
that need no Python (the tools written in Python, `larmorx sdcflows` and `larmorprepx`, need the
Python package), with every file's SHA-256 in `SHA256SUMS`.

**Container images** (linux/amd64 and linux/arm64, for Apptainer, Docker and Podman):
`ghcr.io/karellopez/larmorx:<version>` holds larmorx with `[prep]`, without the replica packages
(commercial use allowed); `ghcr.io/karellopez/larmorx:<version>-all` adds larmorx-gpl and
larmorx-nc (so non-commercial use only). `latest` and `latest-all` name the newest final release.

```bash
apptainer pull docker://ghcr.io/karellopez/larmorx:0.1.0rc1
apptainer exec larmorx_0.1.0rc1.sif larmorx --version
docker run --rm -v "$PWD:/data" -w /data ghcr.io/karellopez/larmorx:0.1.0rc1 \
    larmorx ants N4BiasFieldCorrection -d 3 -i t1w.nii.gz -o t1w_n4.nii.gz
```

Under Apptainer the image runs as you, with your home directory, where TemplateFlow keeps the
templates it downloads on first use (`$HOME/.cache/templateflow`); Docker runs it as root, so
mount a volume at `/root/.cache/templateflow` (or set `TEMPLATEFLOW_HOME` to a mounted folder) to
keep them. `/opt/larmorx/README.txt` in the image says what it holds and where its licences and
sources are.

## First steps

**Where pip puts the commands.** pip installs `larmorx`, `lx` and `larmorprepx` into the
Python environment's folder of programs: `bin` on Linux and macOS, `Scripts` on Windows. For a
virtual environment named `larmorx-env` that is `larmorx-env/bin` or `larmorx-env\Scripts`, and
activating the environment puts it on the `PATH`, so the commands work by their names. Without a
virtual environment, pip names the folder in a warning when it is not on the `PATH`; this prints
it (in PowerShell, with `python` for `python3`):

```bash
python3 -c "import sysconfig; print(sysconfig.get_path('scripts'))"
```

After `pip install --user`, the folder is `sysconfig.get_path('scripts',
sysconfig.get_preferred_scheme('user'))` instead. `python3 -m larmorx` (`python -m larmorx` on
Windows), followed by the same arguments, runs the same command without the folder on the `PATH`.

**larmorx's own options** (new since 0.1.0rc1, so in the next release; in 0.1.0rc1 the
environment variables of the table do the same, as the documentation site's
[installation page](https://karellopez.github.io/larmorx_documentation/installation/) shows):

```text
larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects fix|reproduce] <family> <tool> <the original's arguments>
```

They stand between `larmorx` (or `lx`) and the family, so they never clash with the original
programs' options: everything after the family belongs to the tool. Each one takes the values of
the Python argument of the same name and of the environment variable it replaces, and wins over
that variable when both are set; `--n-threads 4` and `--n-threads=4` are the same.

| Command line | Python argument | Environment variable it replaces | Values |
|---|---|---|---|
| `--n-threads N` | `n_threads` | `OMP_NUM_THREADS`; for ANTs' tools `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` and `NSLOTS` | the number of threads; `0`, the command line's default, is every logical CPU (Python's default is 1) |
| `--implementation NAME` | `implementation` | `LARMORX_IMPLEMENTATION` | `auto` (the default): a tool's exact replica when it is built in or installed, otherwise its clean-room original; `replica`; `clean-room` |
| `--upstream-defects MODE` | `upstream_defects` | `LARMORX_UPSTREAM_DEFECTS` | `fix` (the default): the original program's known defects corrected, each with a notice (below); `reproduce`: the original program's result |

`clean-room` was spelled `original` in 0.1.0rc1: that spelling still works (Python adds a
`DeprecationWarning`). A tool whose original program has its own thread option uses it when it
is given, otherwise `--n-threads`: tedana's `t2smap --n-threads`, sdcflows' `--nprocs` and
`--omp-nthreads`, FSL's `topup --nthr`. The replica programs `larmorx-nc` and `larmorx-gpl` take
`--n-threads` and `--upstream-defects` in the same place; their `--implementation` takes only
`replica`, the one implementation they hold. `larmorx --help` lists the options.

**An example, start to end**, on one participant of the public OpenNeuro dataset
[ds000005](https://openneuro.org/datasets/ds000005) (public domain, PDDL; 21 MB): a new virtual
environment, the two images downloaded, then ANTs' bias-field correction on four threads, FSL's
brain extraction, head-motion correction and registration, and AFNI's brain mask computed by the
replica (the default) and by the clean-room original. FSL's tools write `.nii.gz` files and say so
in one line each (`upstream defect FSL-11 ... corrected`, below): FSL's own programs need
`FSLOUTPUTTYPE` and stop without it, larmorx's do not (new since 0.1.0rc1, whose FSL tools other
than `flirt` and `mcflirt` need it as FSL's do). Set it to choose another type, as with FSL
(`export FSLOUTPUTTYPE=NIFTI` writes `.nii` files; `$env:FSLOUTPUTTYPE = "NIFTI"` in PowerShell).
The last command gives the original N4's result instead of the corrected one. On Linux and macOS
(bash or zsh), with Python 3.12 or newer as `python3` (macOS's own `python3` is older: install one
from [python.org](https://www.python.org) or Homebrew):

```bash
python3 -m venv larmorx-env
source larmorx-env/bin/activate
pip install larmorx
mkdir larmorx-demo
cd larmorx-demo
curl -L -o t1w.nii.gz https://s3.amazonaws.com/openneuro.org/ds000005/sub-01/anat/sub-01_T1w.nii.gz
curl -L -o bold.nii.gz https://s3.amazonaws.com/openneuro.org/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz
larmorx --n-threads 4 ants N4BiasFieldCorrection -d 3 -i t1w.nii.gz -o t1w_n4.nii.gz
larmorx fsl bet t1w_n4.nii.gz t1w_brain -R -m
larmorx fsl mcflirt -in bold.nii.gz -out bold_mcf -plots
larmorx fsl fslmaths bold_mcf -Tmean bold_mean
larmorx afni 3dAutomask -prefix bold_mask.nii.gz bold_mean.nii.gz
larmorx --implementation clean-room afni 3dAutomask -prefix bold_mask_clean_room.nii.gz bold_mean.nii.gz
larmorx fsl fslmaths bold_mean -mas bold_mask bold_brain
larmorx fsl flirt -in bold_brain -ref t1w_brain -dof 6 -omat bold2t1w.mat -out bold2t1w
larmorx --upstream-defects reproduce ants N4BiasFieldCorrection -d 3 -i t1w.nii.gz -o t1w_n4_ants.nii.gz
```

On Windows, in PowerShell, with Python 3.12 or newer as `python` (python.org's installer puts it
on the `PATH` when "Add python.exe to PATH" is ticked; otherwise write `py` for `python` in the
first line):

```powershell
python -m venv larmorx-env
larmorx-env\Scripts\Activate.ps1
pip install larmorx
mkdir larmorx-demo
cd larmorx-demo
curl.exe -L -o t1w.nii.gz https://s3.amazonaws.com/openneuro.org/ds000005/sub-01/anat/sub-01_T1w.nii.gz
curl.exe -L -o bold.nii.gz https://s3.amazonaws.com/openneuro.org/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz
larmorx --n-threads 4 ants N4BiasFieldCorrection -d 3 -i t1w.nii.gz -o t1w_n4.nii.gz
larmorx fsl bet t1w_n4.nii.gz t1w_brain -R -m
larmorx fsl mcflirt -in bold.nii.gz -out bold_mcf -plots
larmorx fsl fslmaths bold_mcf -Tmean bold_mean
larmorx afni 3dAutomask -prefix bold_mask.nii.gz bold_mean.nii.gz
larmorx --implementation clean-room afni 3dAutomask -prefix bold_mask_clean_room.nii.gz bold_mean.nii.gz
larmorx fsl fslmaths bold_mean -mas bold_mask bold_brain
larmorx fsl flirt -in bold_brain -ref t1w_brain -dof 6 -omat bold2t1w.mat -out bold2t1w
larmorx --upstream-defects reproduce ants N4BiasFieldCorrection -d 3 -i t1w.nii.gz -o t1w_n4_ants.nii.gz
```

If PowerShell refuses `Activate.ps1` because running scripts is disabled on the computer,
`Set-ExecutionPolicy -Scope Process RemoteSigned` allows it in that window only. Each command
writes its results into `larmorx-demo`; `deactivate` leaves the virtual environment.

**What the `upstream defect ... corrected` lines mean.** Some commands print a notice such as

```text
larmorx: upstream defect ANTS-40 (N4BiasFieldCorrection) corrected: 6208 of the 92160 voxels N4 uses have no logarithm (6208 are 0, 0 negative or NaN): they are left out of the histogram and the fit; ITK keeps them in its log image as they are (0 is the log of 1) [LARMORX_UPSTREAM_DEFECTS=reproduce gives N4BiasFieldCorrection's result]
```

It is not an error (the command still ends with exit status 0). It says that a known defect of
the original program (here ANTs' N4) would have changed this result, and that larmorx computed
the corrected result, as it does by default. The code (`ANTS-40`) is the defect's entry in the
[register](docs/findings/upstream-defects.md), and the [upstream defects](docs/api/upstream-defects.md)
page explains the rules. To get the original program's result, bit for bit, give
`--upstream-defects reproduce` before the family (the notice's `LARMORX_UPSTREAM_DEFECTS=reproduce`
is the same setting as an environment variable; `upstream_defects="reproduce"` in Python); the
notice then says `reproduced`. FSL-11's notice (`FSLOUTPUTTYPE` not set, the images written as
`.nii.gz`) is printed once per command that writes an image; with `--upstream-defects reproduce`
an FSL tool stops at its first image with FSL's message, as FSL's does (`standard_space_roi`'s
script goes on and exits 0 without an image, as FSL's: its notice says so). The replica programs' notices start with `larmorx-nc:` or
`larmorx-gpl:`, and AFNI's tools print theirs as AFNI's warnings (`*+ WARNING: upstream defect
AFNI-99 (3dvolreg) corrected: ...`).

## Usage

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

Implemented and validated against the original programs (ANTs 2.6.5, AFNI 26.3.05, FSL 6.0.7.23,
nibabel, fMRIPrep's resampler and field-map steps, sdcflows 2.17.1, niworkflows 1.15.4, SciPy 1.18.1,
scikit-image 0.26.0, tedana 26.0.3). Each line links to its validation record; API pages are in [docs/api](docs/api/),
benchmark reports in [docs/benchmarks](docs/benchmarks/).
The table is generated from [validation/tools.toml](validation/tools.toml) and the records
(`python -m larmorx_validation tables`). Beside the records, every tool is tested against its
original with random valid inputs and options, a night at a time
([random testing](docs/validation/difftest.md)).

<!-- BEGIN GENERATED: tools table (python -m larmorx_validation tables; edit validation/tools.toml) -->
| Tool | Command / Python | How | Validation (standard tier) |
|---|---|---|---|
| NIfTI-1/2 I/O | `lx.load`, `lx.save`, `lx.Image` | own implementation | [257 files](docs/validation/nifti-io.md), data bit-identical to nibabel; [4–8× faster](docs/benchmarks/nifti-io.md) on compressed writes with threads |
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
| sdcflows' field-map estimation (the B-spline field fit, the field evaluated and applied, the phase-difference preparation) | `lx.sdcflows.bspline_approx`, `lx.sdcflows.B0FieldTransform`, `lx.sdcflows.apply_coeffs_field`, `lx.sdcflows.phdiff_workflow`, `lx.io.save_nibabel` | port (sdcflows 2.17.1, with scikit-learn's, SciPy's, numpy's and nibabel's arithmetic) | [37 cases](docs/validation/sdcflows-fieldmap.md): 37 of 37 bit-identical to sdcflows 2.17.1 (every array, file, header, warning and message), fMRIPrep's seven phase-difference estimators and their B-spline fits included (also fMRIPrep's own files); SDC-01, -02 and -04 to -11 corrected by default or refused, each with `reproduce`; the fit [6.5 to 7.8 times faster](docs/benchmarks/sdcflows-fieldmap.md) than scikit-learn's at 14 threads, with 22 to 32 % of sdcflows' peak memory at one |
| sdcflows' PEPOLAR, direct and fieldmap-less estimation (the masks, the estimators of a BIDS dataset, the derivatives, the BIDS app; niworkflows' helpers; fMRIPrep's own field-map steps) | `larmorx sdcflows sdcflows`, `lx.sdcflows.topup_workflow`, `lx.sdcflows.fmap_workflow`, `lx.sdcflows.syn_sdc_workflow`, `lx.sdcflows.brain_masker`, `lx.sdcflows.run_app`, `lx.niworkflows`, `lx.bids`, `lx.ndimage`, `lx.skimage` | port (sdcflows 2.17.1 with 2.15.2's estimator search, niworkflows 1.15.4, nipype 1.12.0's command lines, pybids 0.22.0's index, fMRIPrep 25.2.7's field-map steps; SciPy's, scikit-image's and numpy's arithmetic in Rust) | [19](docs/validation/sdcflows-estimation.md), [14](docs/validation/sdcflows-estimation-full.md) and [8](docs/validation/sdcflows-estimation-app-shared.md) cases of the estimation, [5](docs/validation/niworkflows-helpers.md) of niworkflows' helpers and [10](docs/validation/ndimage-skimage.md) and [1](docs/validation/ndimage-skimage-full.md) of SciPy's, scikit-image's and numpy's arithmetic, every one bit-identical to the originals (every array, file, header, warning, message and command line): sdcflows' BIDS app whole on its eight oracle datasets in both library environments, fMRIPrep's own field-map derivatives from LARMORX's stage run as fMRIPrep ran it, the fieldmap-less (SyN) registration ANTs' one-thread warp byte for byte; SDC-12 to SDC-46, PIPE-18 to PIPE-40, SCI-06 to SCI-32 and SKI-01 to SKI-04: 42 corrected by default and 15 refused, 61 with `reproduce`; the BIDS app [1.2 to 5.3 times faster](docs/benchmarks/sdcflows-estimation.md) than sdcflows at one thread, with 16 to 72 % of the originals' peak memory |
| tedana's t2smap (the T2* and S0 maps of multi-echo fMRI and the echoes' optimal combination) | `larmorx tedana t2smap`, `larmorx-gpl tedana t2smap`, `lx.tedana.t2smap` | exact replica in `larmorx-gpl` (GNU GPL version 3, converted from tedana's LGPL-2.1 under its section 3), `experimental` (2026-11-08: every option, message, output and failure case of tedana 26.0.3, with numpy's, SciPy's, nibabel's and nilearn's arithmetic; TED-01 to TED-18 corrected by default or refused, each with `reproduce`, but TED-06, kept); the clean-room original later, in the main package | replica: [62 of 62](docs/validation/tedana-t2smap-replica.md) compared cases bit-identical to tedana 26.0.3 in both kept library environments (every file, message and exit status): fMRIPrep's 5 multi-echo calls on three oracle runs (also against fMRIPrep's own files), every option, data type and failure; 40 more rejected by both alike; 44 % of tedana's peak memory on fMRIPrep's calls, [3.0 to 4.2 times its speed](docs/benchmarks/tedana-t2smap.md) at 14 threads on fMRIPrep's curve-fit calls |
| 3dTshift (slice timing) | `larmorx afni 3dTshift`, `lx.afni.tshift` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [282 of 282](docs/validation/afni-tshift.md) compared agree, 180 bit-identical, [2–6× faster](docs/benchmarks/afni-tshift.md); replica: [282 of 282](docs/validation/afni-tshift-replica.md) bit-identical |
| 3dAutomask (brain mask) | `larmorx afni 3dAutomask`, `lx.afni.automask` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [130 cases](docs/validation/afni-automask.md), 92 of 106 compared cases that write outputs bit-identical to AFNI (the other 14: inputs carrying AFNI's own header extension, AFNI-10: the data bit-identical, the header and messages AFNI's for the input without it), fMRIPrep's call on every oracle run's BOLD reference included, the data panel's too; replica: [101 of 101](docs/validation/afni-automask-replica.md) compared cases that write outputs bit-identical |
| 3dUnifize (intensity uniformization) | `larmorx afni 3dUnifize`, `lx.afni.unifize` | clean-room original and exact replica, both in larmorx (the replica with CC BY 4.0's attribution) | original: [129 cases](docs/validation/afni-unifize.md), none failing: 44 of 99 compared cases that write outputs bit-identical to AFNI, the other 55 within 1e-5 (at most 8.7e-8 of the largest value) and bit-identical to AFNI run with correctly rounded maths (glibc's `tanhf` in AFNI's squash), fMRIPrep's call on every oracle run included; 4 to 25 times faster than AFNI (informal timings); replica: [97 of 97](docs/validation/afni-unifize-replica.md) compared cases that write outputs bit-identical at 1, 2 and 6 threads |
| 3dvolreg (rigid-body registration of a series) | `larmorx afni 3dvolreg`, `lx.afni.volreg` | clean-room original, `experimental` (2026-11-08: every option, message and failure case of `specs/3dvolreg.md`, AFNI-86 to AFNI-99 handled as the replica handles them, each with `reproduce`; built for speed: fMRIPrep's calls 2.8 to 5.2 times AFNI's speed and 1.5 to 1.9 times the replica's on one thread, the same bits at any thread count); exact replica in larmorx (with CC BY 4.0's attribution; 2026-11-07: every option, message and failure case; AFNI-86 to AFNI-99 corrected or refused by default, AFNI's behaviour with `reproduce` where it is defined) | original: [185 of 185](docs/validation/afni-volreg-original.md) compared cases within `specs/3dvolreg.md` §14.3's limits against AFNI 26.3.05, its default within them of the replica's default (exit statuses, messages, files written and headers exact; the numbers that the shear decomposition's ties decide within limits measured from AFNI's own spread under those ties): fMRIPrep's 10 RobustAverage and PEPOLAR calls (their medians at most 0.115 of their limits from fMRIPrep's own outputs), every option, data type, message and failure case; 63 more rejected by both alike; [2.8 to 5.2 times faster than AFNI](docs/benchmarks/afni-volreg.md) on one thread, 9 to 15 times on 14; replica: [137 of 137](docs/validation/afni-volreg-replica.md) compared cases that write outputs bit-identical to AFNI 26.3.05 (every file written, the messages, the exit status), fMRIPrep's RobustAverage calls on six oracle runs' BOLD references and sdcflows' PEPOLAR reference averages on two included (their medians fMRIPrep's own outputs byte for byte); 30 more rejected by both alike; 2.2 to 3.1 times faster than AFNI (informal timings) |
| Head-motion correction (mcflirt) | `larmorx mri hmc`, `larmorx fsl mcflirt`, `lx.mri.hmc` | clean-room original, `experimental` (2026-11-07: the `normcorr` count's running total taking each point's weight as the point is visited, and `leastsquares` as FLIRT's `leastsq`, the spec answers of 2026-11-06); exact replica in `larmorx-nc` (FSL Licence, non-commercial) | original: [compared with FSL's mcflirt](docs/validation/mri-hmc.md): 145 of 145 compared cases within thresholds; [5–16× faster](docs/benchmarks/mri-hmc.md) with threads; replica: [205 of 205](docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built, FSL 6.0.7.23 (the recorded runs, fMRIPrep's calls on every oracle run with BOLD runs, the data panel's included, and the edge cases; 32 more rejected by both, alike; [173 of 173](docs/validation/fsl-mcflirt-replica-full.md) in the full tier against FSL 6.0.7.17), fMRIPrep's own matrices identical |
| BET (brain extraction; the skull and scalp surfaces) | `larmorx fsl bet`, `larmorx fsl bet2`, `larmorx fsl betsurf`, `larmorx fsl standard_space_roi`, `larmorx fsl fslroi`, `lx.fsl.bet`, `lx.fsl.betsurf`, `lx.fsl.standard_space_roi`, `lx.fsl.fslroi` | clean-room original (`bet2`, the `bet` script with every variation (`-S`, `-B`, `-A`, `-A2` since 2026-11-05) and `betsurf`; `standard_space_roi` and `fslroi` since 2026-11-05, FSL's standard images found where the user has them or fetched once), `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial), with `standard_space_roi` and `fslroi` (since 2026-11-04) | original: [198 of 200](docs/validation/fsl-bet.md) compared cases within thresholds against FSL 6.0.7.23, fMRIPrep's 29 BET calls on every oracle run with BOLD runs bit-identical; bet2's and bet's brains, masks, outlines, meshes and skulls FSL's bit for bit, the surfaces that rerun after a self-intersecting first pass included; betsurf's profiles decided as FSL's, its masks within its thresholds on 4 of 5 real heads (Dice 0.9907 to 0.9997); [2.5 to 3.6 times faster than FSL on one thread, 6.5 to 13.5 times on 24, betsurf included](docs/benchmarks/fsl-bet.md); replica: [201 of 201](docs/validation/fsl-bet-replica.md) compared cases bit-identical to FSL 6.0.7.23, every file, exit status and message ([129 of 129](docs/validation/fsl-bet-replica-full.md) in the full tier against FSL 6.0.7.17, the data panel's T1w images included), fMRIPrep's 29 BET calls on every oracle run with BOLD runs included; 47 more rejected by both, alike; 13 where FSL crashes and `reproduce` fills what it leaves undefined; 1 where FSL's fill reads memory beyond the volume (FSL-43, undefined); `bet -S`, `-B`, `-A`, `-A2`, `standard_space_roi` and `fslroi` in the replica since 2026-11-04 (FSL's standard images shipped in `larmorx-nc`); the original's `standard_space_roi` and `fslroi` since 2026-11-05 (every case of the two FSL's bit for bit, the six real heads included), and its `-S`, `-B`, `-A`, `-A2` (`-S` and `-B` FSL's bit for bit on every case but a 3.5-year-old's `-B`, bet2's `-c` on a neurologically stored image; `-A` and `-A2` with betsurf's masks within its thresholds) |
| FLIRT (affine registration, boundary-based registration; convert_xfm and FLIRT's other programs) | `larmorx fsl flirt`, `larmorx fsl convert_xfm`, `larmorx fsl avscale`, `larmorx fsl img2imgcoord`, `larmorx fsl std2imgcoord`, `larmorx fsl img2stdcoord`, `larmorx fsl rmsdiff`, `larmorx fsl makerot`, `larmorx fsl midtrans`, `larmorx fsl pointflirt`, `larmorx fsl applyxfm4D`, `larmorx fsl pairreg`, `larmorx fsl extracttxt`, `larmorx fsl aff2rigid`, `lx.fsl.flirt`, `lx.fsl.convert_xfm`, `lx.fsl.avscale`, `lx.fsl.img2imgcoord`, `lx.fsl.std2imgcoord`, `lx.fsl.img2stdcoord`, `lx.fsl.rmsdiff`, `lx.fsl.makerot`, `lx.fsl.midtrans`, `lx.fsl.pointflirt`, `lx.fsl.applyxfm4d`, `lx.fsl.pairreg`, `lx.fsl.extracttxt`, `lx.fsl.aff2rigid` | clean-room original of every program, `experimental` (parts 1 and 2: `flirt` with `-debug` and every verbose level, `convert_xfm`, avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D and the scripts pairreg, extracttxt, aff2rigid; 2026-11-05: the spec answers of 2026-11-03, FSL-204 and FSL-241 refused in both modes, NaN centres run as FSL runs them with `reproduce`; 2026-11-07: the weighted `normcorr`'s count per point, the spec answer of 2026-11-06; fMRIPrep's BBR call 12 to 14 times FSL's speed on 14 threads); exact replica of every program in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (parts 1 to 3); both: every program and script of fsl-flirt but `-warp`, `flirt_average` and `epi_reg` (`standard_space_roi`: the replica's since 2026-11-04, the original's since 2026-11-05, its cases in BET's records) | original: [239 of 240](docs/validation/fsl-flirt.md) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's three boundary-based calls within 0.012 mm at the 95th percentile of the brain with `reproduce` (glibc's maths functions, as FSL is built), `-applyxfm` FSL's images bit for bit with trilinear and nearest neighbour, applyxfm4D's spline FSL's values, the other programs' printed results and files FSL's, every verbose level and `-debug`, the schedules' and the command line's messages FSL's), 89 more rejected by both alike, 10 where FLIRT crashes or never finishes, or writes results the spec cannot define (FSL-241, refused); the 1 it misses is in its record (the weighted `woods` registration, whose path parts at its cost's last bits; since 2026-11-05 the search orders its starting points of equal cost as FSL does, with libstdc++'s `std::sort`; since 2026-11-07 the weighted `normcorr`'s count takes each point's weight as the point is visited, its matrix FSL's as text); replica: [211 of 211](docs/validation/fsl-flirt-replica.md) compared cases bit-identical to FSL 6.0.7.23 (matrices, images, printed results, exit status and messages): fMRIPrep's boundary-based calls on three oracle runs (their matrices also identical as text to fMRIPrep's own), every cost function, the search options, every shipped schedule and every schedule command (`alignpaxes`, any parameter-subset mask), `-anglerep quaternion`, `-debug`, `-applyxfm` 3D and 4D with every interpolation and data type, BBR with its types, coordinates and a field map, avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D in every data type and interpolation, pairreg, extracttxt and aff2rigid; 87 more rejected by both, alike; 7 where FLIRT crashes or never finishes (FSL-71, FSL-72, FSL-75, FSL-93) |
| FAST (tissue segmentation: classes, partial volumes, bias field) | `larmorx fsl fast`, `lx.fsl.fast` | clean-room original, `experimental` (2026-11-03: every option, message and failure case, one channel or any number of them, FSL's tissue priors registered by FLIRT's clean-room original in the same process; 2026-11-05: the spec answers of 2026-11-03, FSL-202 and FSL-203 in both modes, standard output as FAST buffers it at an abort; fMRIPrep's call 9 to 11 times FSL's speed on one thread, 47 to 67 times on 14, the same bits at any thread count); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` | original: [64 of 65](docs/validation/fsl-fast.md) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's call on 3 oracle runs and every single-channel case FSL's values bit for bit with `reproduce` (glibc's maths functions, as FSL is built), two to eight channels FSL's segmentations with their partial volumes FSL's or within 1.3e-7 on average; its default held to the replica's default, bit for bit with glibc's functions on every single-channel case), 35 more rejected by both alike, 2 expected divergences (FSL-104, refused; two channels' defaults that both correct FSL-98's cycle, the correctly rounded functions' band); the 1 it misses is in its record (five channels with nearly singular covariances, decided by the matrix arithmetic's last bits); replica: [59 of 59](docs/validation/fsl-fast-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every image written, the printed messages, the exit status): fMRIPrep's call on 3 oracle runs (an adult, a 3.5-year-old, a glioma), every option, one to eight channels, FSL's tissue priors registered by FLIRT's replica, the image types, 2D images; 27 more rejected by both alike; 1 where FSL reads and writes past the smaller of channels of different sizes (FSL-104, refused) |
| topup and applytopup (the susceptibility field from reversed phase encodings, and its correction) | `larmorx fsl topup`, `larmorx fsl applytopup`, `lx.fsl.topup`, `lx.fsl.applytopup` | clean-room original, `experimental` (2026-11-03: every option, message and failure case of both programs, applytopup's undocumented vb2D, vb3D and vb4D included; 2026-11-04: the spec answers of 2026-11-03, FSL-133, FSL-134, FSL-145 and FSL-201 reproduced with `reproduce`; 2026-11-05: the scaled conjugate gradients' `--miter` count, FSL-242 corrected by default; 2026-11-07: FSL-273 with a coefficient field corrected by default (the Jacobian of the displacement applied), FSL's scales with `reproduce`; fMRIPrep's call 10 to 12 times FSL's speed on one thread, 39 to 41 times on 14, the same bits at any thread count); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03: the same programs, options and methods; 2026-11-06: FSL-273, applytopup's Jacobian with a field of other voxel sizes, corrected by default) | original: [106 of 106](docs/validation/fsl-topup.md) compared cases within thresholds against FSL 6.0.7.23 (fMRIPrep's call on 2 oracle runs: ds001600 within FSL's own thread band in every metric, ds005040 in every one but the field's correlation, 0.999958 against FSL's own 0.999966), 67 more rejected by both alike, 14 expected divergences, each with its reason (estimated movements that do not converge, where FSL-201's forward differences make FSL's own path depend on its inputs' last bits, FSL's measured spread beside the original's; FSL-143's diverging iterations, reproduced; vb2D's transients; FSL-126; inputs refused for FSL-124 and FSL-134); replica: [107 of 107](docs/validation/fsl-topup-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every file written, the printed messages, the exit status): fMRIPrep's call on 2 oracle runs, every option, FSL's configurations, applytopup's five methods; 67 more rejected by both alike; 3 refused where FSL needs more memory than any machine holds (a negative --warpres, FSL-124) or reads a field on another grid past its lines (FSL-134) |
| fslmaths (image arithmetic, thresholds, filters, reductions, statistics, TFCE) | `larmorx fsl fslmaths`, `lx.fsl.fslmaths`, `lx.fsl.smooth`, `lx.fsl.threshold` | clean-room original, `experimental` (2026-11-04: every operation, option, data type, message and failure case, FSL-191 to FSL-193 found by its black-box runs; 1.8 to 28 times FSL's speed on one thread, up to 268 times on 14, the same bits at any thread count); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03: every operation, option, data type, message and failure case) | original: [386 of 386](docs/validation/fsl-maths.md) compared cases within thresholds against FSL 6.0.7.23, 370 of them FSL's values in every file (fMRIPrep's 18 calls included; the others: FSL's FFT path, `-subsamp2` under the floating types, the last bits of Φ, the eigenvectors' signs, GLIBC-07), 63 more rejected by both alike, 5 expected divergences (FSL's FFT convolution against the original's direct sum, `-subsamp2offc`'s means); replica: [360 of 360](docs/validation/fsl-maths-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every file written, the printed messages, the exit status): fMRIPrep's 18 calls (combine_masks on three oracle runs, the goodvoxels mask's fourteen), every operation in every data type; 61 more rejected by both alike; 1 one unit in the last place where FSL's glibc 2.35 is not correctly rounded and the replica is (GLIBC-07: `-dt double -tan`; glibc's `sin`, `cos` and the FFT's `sincos` are the replica's `reproduce` bits since 2026-11-04) |
| fslstats (image statistics: means, extremes, percentiles, histograms, per volume and label, JSON) | `larmorx fsl fslstats`, `lx.fsl.fslstats`, `lx.fsl.image_mean`, `lx.fsl.image_std` | clean-room original, `experimental` (2026-11-04: every option, `-t`, `-K` and `-json`, every message and failure case); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-03: every option, `-t`, `-K` and `-json`, every message and failure case; BET's replica runs it) | original: [126 of 126](docs/validation/fsl-stats.md) compared cases within thresholds against FSL 6.0.7.23, 125 of them FSL's numbers (fMRIPrep's 4 goodvoxels calls included; BET's call within 6e-8 relative), 36 more rejected by both alike; replica: [172 of 172](docs/validation/fsl-stats-replica.md) compared cases printing FSL 6.0.7.23's text byte for byte (with its messages and exit status): fMRIPrep's 4 goodvoxels calls, BET's call, FSL's own tests, every option; 36 more rejected by both alike |
| prelude (phase unwrapping of field maps' phase differences and complex images) | `larmorx fsl prelude`, `larmorx-nc fsl prelude`, `lx.fsl.prelude` | clean-room original, `experimental` (2026-11-06: every option, message and failure case of `specs/prelude.md`, FSL-261 to FSL-272 corrected by default or refused, each with `reproduce`; the regions merged through an indexed heap and a weighted union-find: fMRIPrep's two large calls 97 and 211 times FSL's speed and 2.5 to 2.8 times the replica's on one thread, the same bits at any thread count); exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (2026-11-06: every option, message and failure case, FSL-261 to FSL-272 corrected by default or refused, each with `reproduce`) | original: [138 of 138](docs/validation/fsl-prelude.md) compared cases within thresholds against FSL 6.0.7.23 (with `reproduce` every file bit-identical to FSL's; by default bit-identical to the replica's but the phases of complex images, GLIBC-07, which keep FSL's whole turns): the replica suite's cases with the original standing in, fMRIPrep's 7 calls (also against fMRIPrep's own outputs), every option, image type, NaN input, failure and output type; 28 more rejected by both alike; replica: [68 of 68](docs/validation/fsl-prelude-replica.md) compared cases bit-identical to FSL 6.0.7.23 (every file written, the printed messages, the exit status): fMRIPrep's 7 calls on four oracle runs, the data panel's two other phase-difference maps, every option, complex and phase-and-magnitude inputs, series, NaN values (in a complex128 image's case, FSL-265, the values FSL takes from memory it never wrote are reported, not compared); 20 more rejected by both alike |
<!-- END GENERATED: tools table -->

## Roadmap

The ordered plan is in [docs/next-steps.md](docs/next-steps.md). In outline:

1. **AFNI:** 3dQwarp, 3dSkullStrip,
   3dcalc and an analysis track (3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit,
   3dFWHMx/3dClustSim, ...).
2. **FSL-compatible tools:** the clean-room originals' catch-up with their specifications'
   answers (topup, FAST, FLIRT, BET, the head-motion correction), with the clean-room originals of
   BET's `-S`, `-B`, `-A`, `-A2`, `standard_space_roi` and `fslroi`; then PRELUDE (FSL's phase
   unwrapping), a replica and a clean-room original (the mcflirt, BET with every option, FLIRT,
   FAST, topup, `fslmaths`, `fslstats`, `standard_space_roi` and `fslroi` replicas and the
   clean-room originals of BET, FLIRT, FAST, topup, `fslmaths` and `fslstats` are done).
3. **multi-echo**: tedana's `t2smap`, its clean-room original (the exact replica, in `larmorx-gpl`, is
   done; sdcflows' field-map estimation, parts A and B, with niworkflows' helpers and fMRIPrep's own
   field-map steps, is done), **FreeSurfer** (`larmorx-freesurfer`), Connectome
   Workbench- and MSM-compatible surface tools, the confounds.
4. **larmorprepx:** an independent, fMRIPrep-compatible preprocessing pipeline (BIDS in,
   fMRIPrep-compatible derivatives and reports out), written in plain Python on the library,
   then compared with fMRIPrep end to end.
5. **Releases:** the first, 0.1.0rc1, for all six platforms (PyPI, the GitHub Release, the
   container images); 0.1.0 once it installs cleanly everywhere ([CHANGELOG](CHANGELOG.md),
   [RELEASING.md](RELEASING.md)).

| Document | Contents |
|---|---|
| [docs/overview.md](docs/overview.md) | the project in plain terms: replicas and clean-room originals, packages, installation, what is published |
| [CHANGELOG.md](CHANGELOG.md) | what each release holds, each tool's status, known limitations |
| [RELEASING.md](RELEASING.md) | how a release is made and checked |
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
python3.12 -m venv .venv
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
pip install --group dev
maturin develop --release
cargo test --workspace
pytest tests/python tests/golden
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
ruff check && ruff format --check
```

In order: the development tools (maturin, pytest, packaging, nibabel, ruff; pip 25.1 or newer
reads `--group`), the `larmorx._core` extension built into the virtual environment, the Rust
tests, the Python tests with the cross-platform golden tests, and the linters.

The FSL replicas are a separate Cargo workspace: `cd crates-nc && cargo test --workspace`; `crates-gpl` holds tedana's `t2smap` replica (`cd crates-gpl && cargo test -p larmorx-gpl-tedana -p larmorx-gpl-cli`) and verification tools (`crates-gpl/README.md`)
(build its wheel with `maturin build --release`).

Parity checks and benchmarks need the test data (`larmorx-testdata`, github.com/karellopez/larmorx-testdata, kept next to this one; its data panel, `docs/panel.md` there, is the fixed set every tool is validated on), the validation package and the original programs as oracles (`scripts/build_ants_oracle.sh`, `scripts/build_afni_oracle.sh`):

```bash
pip install -e ../larmorx-testdata -e "validation[oracles]"
pytest tests/parity
python -m larmorx_validation parity nifti-io --tier standard --out docs/validation
python -m larmorx_validation bench nifti-io --out docs/benchmarks
python -m larmorx_validation difftest --tools all --budget 6 --jobs 4
python -m larmorx_validation tables --check
```

In order: the test data and the validation package; the parity suites' smoke tier, which
compares larmorx with the reference programs; one tool's parity suite (its standard tier) and
benchmark, written into the repository's records; six hours of differential random testing on
four processes; and the check that the generated tables are current.

| Path | Contents |
|---|---|
| `crates/larmorx-core`, `crates/larmorx-io`, `crates/larmorx-hdf5` | shared foundation: images, grids, linear algebra, correctly rounded maths, NIfTI and transform files, HDF5 in pure Rust |
| `crates/larmorx-transform`, `crates/larmorx-interp`, `crates/larmorx-image` | ITK transforms, interpolators and image filters |
| `crates/larmorx-optim` | SciPy's optimizers: `curve_fit` with bounds (`least_squares`' trust-region reflective method) |
| `crates/larmorx-registration`, `crates/larmorx-ants` | ITK's v4 registration framework; the ANTs programs |
| `crates/larmorx-afni`, `crates/larmorx-mri` | clean-room originals: 3dTshift, 3dAutomask, 3dUnifize; head-motion correction |
| `crates-gpl/` | the GPL workspace (`larmorx-gpl`): tedana's `t2smap` replica (`larmorx-gpl-tedana`, GPL-3.0-only, and the `larmorx-gpl` program), later Workbench's replicas; the AFNI replicas' verification tools (`crates-gpl/README.md`) |
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

Apache-2.0 for LARMORX itself ([LICENSE](LICENSE)). The licence expression PyPI shows for each
package lists, beside its own licence, the licences of the works its files include (the ported
code's and the Rust crates' it is built with; their texts and notices in [NOTICE](NOTICE) and
`LICENSES/`). Replicas of restrictively licensed tools are separate packages under their
upstream's licence (`larmorx-nc`: the FSL Licence, non-commercial use only; `larmorx-gpl`: the GNU
GPL version 3). The AFNI replicas carry CC BY 4.0's attribution for the Medical College of
Wisconsin's code; see [docs/licensing.md](docs/licensing.md). How to cite LARMORX:
[CITATION.cff](CITATION.cff). LARMORX is an independent project, not affiliated with or endorsed
by the developers of ANTs, AFNI, FSL, FreeSurfer, tedana or fMRIPrep.

## Contributing
- Commit messages, PR descriptions and release notes must not contain AI co-author trailers (`Co-Authored-By: Claude …`) or "Generated with …" lines.
- Follow the rules in [CLAUDE.md](CLAUDE.md): clean-room policy, provenance records, and the per-tool contract.
