# larmorx: plan

**Status:** draft v0.3.1, 2026-10-09.

**History:**
- v0.1 proposed forking fMRIPrep.
- v0.2 proposed a new Python pipeline that uses ANTsPy.
- v0.3: **port ANTs as well.** The project becomes a Rust library of the best functions from existing neuroimaging tools. Each tool has its own Python wrapper and CLI. The preprocessing pipeline is written in Python on top of that library.
- v0.3.1 (this version): the project is named **larmorx** (repo `github.com/karellopez/larmorx`). The pipeline, **larmorprepx**, lives *inside* the larmorx package.

**Background material** in [docs/analysis/](docs/analysis/) describes what fMRIPrep computes, in detail (stages, tools, parameters, file:line). It is the specification for the ports and the pipeline:

| Report | Use it for |
|---|---|
| [01-fmriprep-bold-pipeline.md](docs/analysis/01-fmriprep-bold-pipeline.md) | BOLD stages, tool options, confound definitions |
| [02-anatomical-smriprep-niworkflows.md](docs/analysis/02-anatomical-smriprep-niworkflows.md) | anatomical stages, the ANTs registration settings, the programs inside recon-all and their timings |
| [03-sdc-transforms-multiecho.md](docs/analysis/03-sdc-transforms-multiecho.md) | fieldmaps, the one-shot resampler, multi-echo, numerical hot loops |
| [04-engine-portability.md](docs/analysis/04-engine-portability.md) | Windows/macOS pitfalls to avoid |

---

## 0. Summary

**One package, `larmorx`, with two layers.** The repo is `github.com/karellopez/larmorx`.

1. **The tool library** (`pip install larmorx`, then `import larmorx as lx`).
   - Rust implementations of the most useful functions from ANTs/ITK, FreeSurfer and AFNI (ported), and from FSL and Connectome Workbench (clean-room re-implementations).
   - Every tool is available three ways:
     - a **Rust API**
     - an idiomatic **Python wrapper** (`lx.ants.registration(...)`)
     - a **CLI** that accepts the original program's arguments (`larmorx ants antsRegistration ...`, short alias `lx`)
   - Other developers can build their own pipelines on the wrappers.
2. **`larmorprepx`**, the preprocessing pipeline, shipped inside the same package.
   - Python module `larmorx.pipelines.larmorprepx`; command `larmorprepx`; installed with the `[prep]` extra (`pip install "larmorx[prep]"`).
   - Plain Python functions orchestrating `larmorx` tools.
   - BIDS in; fMRIPrep-compatible derivatives and reports out.
   - No nipype or graph engine; no external binaries.
   - `larmorx.pipelines/` leaves room for future pipelines (anatomical-only, DWI, …).

**Facts behind this plan** (all checked in this workspace, 2026-10-08/09):

| Fact | Consequence |
|---|---|
| ANTsPy (`antspyx` 0.6.3) has no wheels for Linux aarch64, Windows arm64 or Python 3.14. Its `motion_correction` is a Python loop of full registrations | Port ANTs. ANTsPy stays useful as an **in-process test oracle** and as an interim backend on x64 |
| ANTs: about 160k lines (Examples 93k, Utilities 37k, ImageRegistration 23k, ImageSegmentation 10k). The ITK modules it relies on (registration v4, metrics, optimisers, transforms, interpolators, filters, statistics, N4, denoising) are about 200k lines. ANTs 2.6 pins ITK v5.4.7 | Large, but much of ITK's size is generic N-dimensional template and pipeline machinery. A 3D/4D-only Rust port of the subset we need is far smaller. Licence: Apache-2.0, so we can port it |
| FreeSurfer's recon-all path is about 200–250k lines of C/C++ (surface library 82k, volume core 36k, GCA + GCA morph 44k, I/O 13k, programs about 60k). Latest release v8.2.0 | Separate long track, ported step by step against FreeSurfer's own intermediates. Its licence allows derivative works with conditions, so it ships as a separate wheel |
| numpy, scipy, matplotlib, scikit-learn, pandas, pillow and h5py have wheels for **Windows arm64 and Linux aarch64**; nibabel, nilearn, templateflow and jinja2 are pure Python | With ANTs ported, **all six targets are first-class**: win-x64, win-arm64, linux-x64, linux-aarch64, macOS-x64, macOS-arm64 |
| fMRIPrep's biggest compute cost (4D BOLD resampling) is Python/scipy; FSL tools have no Python equivalents and a non-commercial licence; Workbench is GPL | Rust kernels for resampling; clean-room re-implementations of FSL and Workbench functions |

---

## 1. Goals and non-goals

### Goals
- **G1 A reusable tool library.**
  - Each tool can be called from Rust, Python or the command line.
  - Each is documented with an option-by-option mapping to the original, plus its validation status.
  - Tools compose **in memory**: no temporary files between steps, unlike chaining ANTs/FSL CLIs.
- **G2 A simple pipeline.**
  - One command with good defaults.
  - The per-subject pipeline is a few hundred lines of readable Python.
  - Results can be resumed through a stage cache; no graph engine.
- **G3 Native on all six targets**, installed with `pip`/`uv`. No Docker, no external neuroimaging installs, no admin rights.
- **G4 Equivalent results.** Differences from the original tool are no larger than that tool's own variability across versions, platforms, thread counts and seeds (§11).
- **G5 Fast.** Targets, to be confirmed by benchmarks:
  - the fMRI pipeline at least 2× faster than fMRIPrep
  - larmorx ports of ANTs tools at least as fast as ANTs on equal threads
  - the recon-all port at least 4× faster than FreeSurfer with `-openmp`
  - at least 5× less scratch disk than fMRIPrep
- **G6 Reproducible.** Same input + seed gives the same output, independent of thread count. Bit-identical across OS/CPU for deterministic kernels.
- **G7 fMRIPrep-compatible outputs.** File names, spaces and confound columns match, so XCP-D, FitLins, nilearn etc. work unchanged (D8).

### Non-goals (first releases)
- Porting *everything* in ANTs, FreeSurfer, AFNI, FSL or Workbench. The catalog (§3) is tiered by need.
- Bit-identical results to the original tools, except where cheap; deterministic ANTs configurations are a stretch target.
- GPU kernels, deep-learning tools, infant/PET/DWI pipelines.

---

## 2. Architecture

```
┌──────────── larmorx.pipelines.larmorprepx (pure Python; command `larmorprepx`) ─────────────┐
│ cli · settings · stages/{anat,fmap,bold,surf} · confounds · bids · report · cache           │
└─────────────────────────────────────────┬───────────────────────────────────────────────────┘
                                          │ calls only the public Python API of larmorx
┌─────────────────────────────────────────▼─────────── larmorx (Python wrappers) ─────────────┐
│ lx.Image / lx.Image4D / lx.Transform / lx.Mesh   (shared in-memory types, numpy-backed)     │
│ lx.ants   lx.afni   lx.mri (clean-room FSL/Workbench-like)   lx.freesurfer (separate wheel) │
│   each tool:  idiomatic function  +  cli.<tool>(argv)  +  .pyi stubs  +  docs page          │
│ console script:  larmorx (alias lx) <family> <tool> [original arguments]                    │
└─────────────────────────────────────────┬───────────────────────────────────────────────────┘
                                          │ PyO3 (abi3), zero-copy numpy, GIL released
┌─────────────────────────────────────────▼─────────── larmorx (Rust workspace) ──────────────┐
│ foundation:  larmorx-{core, io, interp, transform, optim, image, mesh, cli}                 │
│ ports:       larmorx-ants (ANTs/ITK, Apache-2.0)    larmorx-afni (public domain)            │
│              larmorx-freesurfer (FreeSurfer licence; own wheel)                             │
│ clean-room:  larmorx-mri (hmc, brain_mask, tissue, pepolar, unwrap, linreg/bbr, maths,      │
│              surfmap, multiecho, bspline-fieldmap)                                          │
│ bindings:    larmorx-py → "larmorx";  larmorx-freesurfer-py → "larmorx-freesurfer" (wheels) │
│ binaries:    larmorx / lx (multicall CLI, also shipped standalone without Python)           │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Repository layout

```
larmorx/                          this repo (github.com/karellopez/larmorx)
  Cargo.toml                      Rust workspace
  crates/
    larmorx-core/ larmorx-io/ larmorx-interp/ larmorx-transform/ larmorx-optim/
    larmorx-image/ larmorx-mesh/ larmorx-cli/
    larmorx-ants/ larmorx-afni/ larmorx-mri/ larmorx-freesurfer/
    larmorx-py/ larmorx-freesurfer-py/        PyO3 extension crates
  python/
    larmorx/                      wheel "larmorx"
      _core.*                     compiled extension (from crates/larmorx-py)
      ants/ afni/ mri/ cli/       tool families (Python wrappers)
      pipelines/
        larmorprepx/              the preprocessing pipeline (pure Python; needs the [prep] extra)
    larmorx_freesurfer/           wheel "larmorx-freesurfer" (exposed as lx.freesurfer)
  specs/                          clean-room specifications (FSL-like, Workbench-like tools)
  oracles/                        scripts that generate reference outputs (ANTsPy, AFNI, FreeSurfer, fMRIPrep)
  tests/  benchmarks/  docs/
  UPSTREAM.md                     exact upstream versions and SHAs ported from
```

### 2.2 Distributions

| Artifact | Contents | Licence | Install |
|---|---|---|---|
| wheel `larmorx` | the library (foundation + ants + afni + mri; one extension module `larmorx._core`) **and** the larmorprepx pipeline (`larmorx.pipelines.larmorprepx`) | Apache-2.0 (with ANTs/ITK NOTICE) | library only: `pip install larmorx`. With the pipeline: `uv tool install "larmorx[prep]"`, which provides the `larmorprepx` command |
| wheel `larmorx-freesurfer` | the FreeSurfer port | FreeSurfer Software License terms + our modifications | `pip install larmorx-freesurfer` (optional), or `larmorx[prep,surfaces]` for the pipeline with native surfaces |
| standalone `larmorx` / `lx` binary | the multicall CLI, no Python needed | as above, per family | GitHub Releases (cargo-dist), later conda-forge / Homebrew / winget |
| crates | `larmorx-*` on crates.io | as above | `cargo add larmorx-ants` |

- **Extras:** the `[prep]` extra adds the pipeline-only dependencies (templateflow, matplotlib, nilearn, pandas, jinja2). Library users who only want `lx.ants` etc. don't pay for them. Without the extra, `larmorprepx` exits with a clear message saying how to install it.
- **FreeSurfer isolation:** the FreeSurfer port is a separate wheel so its licence terms stay isolated and installing it is opt-in. Interchange between the two wheels uses numpy arrays + affines + the pure-Python `larmorx` types; no Rust objects cross extension boundaries.

---

## 3. Tool catalog

- **Tier 1** = needed by larmorprepx v1.
- **Tier 2** = widely used; added after v1.
- **Tier 3** = later or research.

"Port" = translated from source we may legally adapt. "CR" = clean-room from papers, documentation and black-box behaviour.

### 3.1 `lx.ants`: ANTs/ITK port (Apache-2.0)

| Tool | Tier | Notes |
|---|---|---|
| `registration` (antsRegistration) | 1 | Transforms: Translation, Rigid (Euler3D), Similarity, Affine, SyN, BSplineSyN. Metrics: MeanSquares, Mattes MI, MI, CC, GlobalCorrelation. Multi-metric weights, masks (per stage), sampling None/Regular/Random, histogram matching, winsorising, `restrict_deformation`, convergence windows, collapse/composite output, initial transforms |
| `apply_transforms` (antsApplyTransforms, …ToPoints) | 1 | Linear, NearestNeighbor, BSpline, Gaussian, MultiLabel (label-Gaussian), GenericLabel, Lanczos/Hamming/Cosine/Welch windowed sinc. Transform stacks with inverse flags; 3D and 4D inputs (`-e 3`) |
| `n4_bias_field_correction` | 1 | Multi-level B-spline scattered-data fit, histogram sharpening, weight mask, shrink, rescale |
| `atropos` | 1 | KMeans/Otsu/priors initialisation, GMM EM, MRF (ICM), posteriors, prior weighting |
| `denoise_image` | 1 | Adaptive non-local means, Rician/Gaussian noise models |
| `ai` / `affine_initializer` (antsAI, antsAffineInitializer) | 1 | Rotation-grid search + local optimisation |
| `image_math` (subset) | 1 | TruncateImageIntensity, Laplacian, MD/ME/MO/MC, GetLargestComponent, FillHoles, PadImage, Normalize, Grad, … |
| `threshold_image`, `resample_image`, `smooth_image`, `jacobian_determinant`, `histogram_match` | 1 | – |
| `brain_extraction` (antsBrainExtraction.sh logic) | 1 | Written in Python on top of the tools above |
| `cortical_thickness` (KellyKapowski / DiReCT), `joint_fusion`, `template_construction` | 2 | Template construction orchestrated in Python |
| `motion_correction` (antsMotionCorr) | 2 | CLI compatibility only; implemented with `lx.mri.hmc` |
| TimeVaryingVelocity / Exponential / Demons transforms, point-set metrics | 3 | – |

### 3.2 `lx.afni`: AFNI port (public domain; check each file for third-party exceptions)

| Tool | Tier |
|---|---|
| `tshift` (3dTshift: Fourier/sinc/linear, `-tzero`, `-ignore`, slice-timing files) | 1 |
| `volreg` (3dvolreg) | 1 |
| `automask` (3dAutomask) | 1 |
| `unifize` (3dUnifize) | 1 |
| `despike` (3dDespike), `tproject` (3dTproject), `blur_to_fwhm` | 2 |
| `qwarp` (3dQwarp) | 3 |

### 3.3 `lx.mri`: clean-room re-implementations (ours, Apache-2.0)

These are named by function, not by the original tool (D4). Each tool's docs state the compatibility claim (e.g. "accepts mcflirt-style options").

| Tool | Compatible with | Tier | Algorithm source |
|---|---|---|---|
| `hmc` | MCFLIRT | 1 | Jenkinson et al. 2002; parallel over volumes |
| `brain_mask` | BET | 1 | Smith 2002 |
| `tissue` | FAST | 1 (if CompCor validation needs it; otherwise 2) | Zhang et al. 2001 (HMRF-EM + partial volume) |
| `linreg` (+ `bbr`) | FLIRT, FLIRT-BBR, bbregister | 1 | Jenkinson & Smith 2001; Greve & Fischl 2009 |
| `pepolar` (+ `apply`) | TOPUP / applytopup | 1 | Andersson et al. 2003 |
| `unwrap` | PRELUDE | 1 | Jenkinson 2003 |
| `bspline_fieldmap` | sdcflows B-spline fit (Apache; port allowed) | 1 | separable exact ridge solve |
| `multiecho` | tedana t2smap (LGPL; implemented from the equations, not the code) | 1 | Posse 1999; Kundu 2012 |
| `maths`, `stats` | fslmaths, fslstats syntax | 1 (the subset used), 2 (broader) | definitions |
| `surfmap` | wb_command volume-to-surface-mapping, metric-resample (ADAP_BARY_AREA), metric-dilate, surface-resample, create-signed-distance-volume, sphere project-unproject, fill-holes, remove-islands, CIFTI helpers | 1 (for surfaces) | Workbench documentation; Glasser et al. 2013 |
| `fnirt`-like, `melodic`-like, MSM-like | – | 3 | research |

### 3.4 `lx.freesurfer`: FreeSurfer port (FreeSurfer licence, separate wheel)

| Group | Tools | Tier |
|---|---|---|
| Utilities the pipeline needs | `mri_convert`-like I/O, `lta_convert`, `mri_vol2vol`, `mri_vol2surf`, `mris_convert`, `mris_expand`, `mri_robust_register`, `mri_robust_template`, `mri_coreg` | 1 |
| recon-all "recon-lite" stream (what fMRI needs) | volume stream through aseg, wm, filled; surface stream through white, pial, sphere.reg; thickness/curv/sulc; ribbon (§7) | 1 |
| Full recon-all | parcellation (`mris_ca_label`), stats, aparc+aseg, BA labels | 2 |
| Other popular utilities | `mri_surf2surf`, `mris_preproc`, `mri_segstats`, `mri_label2vol` | 2 |

---

## 4. The per-tool contract ("each binary has its own Python wrapper")

Every catalog entry ships the same six things. CI checks that all of them exist.

1. **Rust API**: typed and builder-style; works on in-memory types.
   ```rust
   let out = larmorx_ants::Registration::new()
       .stage(Stage::rigid(0.05).metric(Metric::mattes(56).regular(0.25))
              .levels(&[100, 100], &[2, 1], Smoothing::vox(&[2.0, 1.0])))
       .stage(Stage::syn(0.1, 3.0, 0.0).metric(Metric::cc(4))
              .levels(&[100, 70, 50, 20], &[8, 4, 2, 1], Smoothing::vox(&[3.0, 2.0, 1.0, 0.0])))
       .winsorize(0.005, 0.995).histogram_matching(true).seed(42).threads(8)
       .run(&fixed, &moving)?;          // -> RegistrationResult { forward, inverse, warped, log }
   ```
2. **Python wrapper**: idiomatic and typed. Accepts `lx.Image`, nibabel images, (array, affine) pairs or paths; returns dataclasses.
   ```python
   from larmorx import ants, afni, mri
   reg = ants.registration(fixed=tpl, moving=t1,
                           stages=[ants.Rigid(0.05, metric=ants.Mattes(56, sampling="regular", rate=0.25),
                                              iterations=[100, 100], shrink=[2, 1], smoothing_vox=[2, 1]),
                                   ants.SyN(0.1, 3, 0, metric=ants.CC(4),
                                            iterations=[100, 70, 50, 20], shrink=[8, 4, 2, 1], smoothing_vox=[3, 2, 1, 0])],
                           winsorize=(0.005, 0.995), histogram_matching=True, seed=42, n_threads=8)
   warped = ants.apply_transforms(t1, reference=tpl, transforms=reg.forward, interpolation="lanczos")
   stc = afni.tshift(bold, slice_timing=times, tzero=0.5 * tr)
   hmc = mri.hmc(bold, reference=ref, n_threads=8)            # -> HmcResult(affines, params, fd)
   ```
3. **Compatible CLI**: the original argument syntax, parsed once in Rust (`larmorx-cli`). It is available as:
   - a console script: `lx ants antsRegistration --dimensionality 3 --float 1 ...`
   - a Python call: `lx.ants.cli.antsRegistration(argv)`
   - the standalone `larmorx` binary

   This lets existing scripts and settings files work unchanged, for example fMRIPrep's registration JSONs, which map 1:1 onto antsRegistration flags.
4. **Type stubs and docs**: `.pyi` stubs generated from the Rust signatures (e.g. with `pyo3-stub-gen`). A docs page per tool with the **option-mapping table** (original flag → Rust/Python parameter → status).
5. **Validation record**: the oracle used, datasets, metrics and thresholds (§11). It sets the tool's **status**:
   - `experimental`: runs, but has no oracle comparison yet
   - `validated`: within thresholds on the oracle set
   - `stable`: validated, plus API frozen under SemVer
6. **PROVENANCE.md**: for ports, the upstream files and versions; for clean-room tools, the papers and specs.

### 4.1 Shared conventions
- **Coordinates:** world space is RAS mm. Images carry a float64 affine. Converting to and from ITK's LPS conventions happens at the API boundary of `lx.ants`, exactly as ANTs does.
- **Transforms:** `lx.Transform` is a composable chain (affines, affine series, displacement fields, B-spline fields). I/O covers ITK `.txt`/`.mat`, FSL `.mat`, LTA, AFNI `.1D` and X5.
  - ITK `.h5` composites are read and written by the Python layer with h5py, which has wheels on all six targets. A minimal pure-Rust HDF5 reader/writer for the ITK layout follows, so the standalone CLI can handle `.h5` too.
- **Threads:** an explicit `n_threads` on every call (no environment-variable coupling). The GIL is released during Rust work.
- **Errors:** typed exceptions; the CLI returns the original tool's exit codes where they are documented.
- **Logs and progress:** structured logs (Python `logging`) and an optional progress callback, which is also used for cancellation (Ctrl-C works on every OS).
- **Determinism:** seeded RNG; thread-count-invariant reductions (§5).
- **Naming:** Python names are snake_case (`ants.n4_bias_field_correction`); CLI names keep the originals (`lx ants N4BiasFieldCorrection`). For clean-room tools, see D4.

---

## 5. Shared Rust foundation

| Crate | Contents |
|---|---|
| `larmorx-core` | `Image<T>` (3D/4D, f32/f64/int/label), `Affine`, physical-space helpers, error types, thread pool, deterministic parallel reductions, RNG, progress/cancel |
| `larmorx-io` | NIfTI-1/2 (`.nii`, `.nii.gz` with fast multi-threaded deflate), MGH/MGZ, GIFTI, CIFTI-2, FreeSurfer surf/curv/label/annot/LTA, ITK transform text and `.mat` |
| `larmorx-interp` | interpolators: SciPy-equivalent cubic B-spline (for the pipeline's resampler), ITK-equivalent B-spline, linear, nearest, Gaussian, label-Gaussian, windowed sincs |
| `larmorx-transform` | affine, affine series, displacement field (compose, invert, Jacobian), B-spline field; transform chains |
| `larmorx-optim` | gradient descent with learning-rate and scales estimation (ITK v4 semantics), conjugate gradient, L-BFGS, Powell/Brent, Gauss-Newton/LM; convergence monitors |
| `larmorx-image` | Gaussian/recursive smoothing, shrink/pyramids, morphology (binary and grey), connected components, distance maps, histograms and histogram matching, statistics, percentiles, gradients, Laplacian |
| `larmorx-mesh` | triangle meshes, neighbourhoods, spatial hashing/BVH, geodesics, sphere utilities |
| `larmorx-cli` | argument parsing compatible with ANTs, AFNI, FreeSurfer and FSL-style syntaxes; the multicall `larmorx` binary |

**Engineering rules:**
- **Pure-Rust dependencies only.** No C, C++, Fortran or HDF5 libraries, so wheels cross-compile for all six targets. Candidate crates: ndarray, numpy (PyO3), rayon, faer, nalgebra, rustfft, zlib-rs/libdeflater, kiddo/rstar; evaluate `nifti` 0.18.
- **3D/4D specialisation instead of ITK's N-dimensional templates.** This is the main reason the port is smaller than the C++ it replaces.
- **Determinism:**
  - Fixed chunking for reductions, independent of thread count.
  - The `libm` crate for transcendental functions, so results match across platforms.
  - f64 accumulators.
  - No fast-math.
- **Precision:** mirror the original where it matters. ANTs `--float` uses float32 images with double-precision metrics; SciPy's prefilter is float64.
- **SIMD:** auto-vectorisation plus runtime dispatch (AVX2/AVX-512; NEON baseline). Never `target-cpu=native` in release builds.
- **Safety:** `#![forbid(unsafe_code)]` except in audited hot loops; fuzz every parser; property-test every kernel against its oracle.
- **Versioning:** every tool exposes `algo_version`, which the pipeline's stage cache includes in its keys.

---

## 6. ANTs port (`larmorx-ants`)

### 6.1 Scope and source
- **Port from** ANTs 2.6.x (the line fMRIPrep uses) and ITK v5.4.7 (pinned by ANTs). Check out those tags; the local clones are `master`.
- **What comes from where:**
  - from ITK: registration framework v4, metrics v4, optimisers v4 + scales estimators, transforms, interpolators, N4, the patch-based denoising base classes, statistics/histograms, morphology, smoothing, distance maps
  - from ANTs: `itkantsRegistrationHelper` (stage logic, about 5.8k lines), the antsRegistration/antsApplyTransforms/antsAI/Atropos/DenoiseImage/ImageMath front-ends, Atropos and adaptive-NLM filters
- **Not ported:** ITK's pipeline and streaming architecture, generic N-dimensional templates, object factories, smart pointers, the IO plugin system. These are replaced by plain Rust types.

### 6.2 Parity-critical details (from the code; these become test cases)
- **Mattes MI:** B-spline Parzen windowing (moving cubic, fixed zero-order), bin count, the intensity-range handling with winsorising, and the sampling strategy. Random sampling uses a seeded RNG; we do not reproduce ITK's Mersenne-Twister stream.
- **CC metric:** neighbourhood radius, and the sliding-window computation used by ANTs.
- **Optimiser:** ITK `GradientDescentOptimizerv4` with `RegistrationParameterScalesFromPhysicalShift`, learning-rate estimation (once / each iteration), maximum step size, and the convergence monitor (window size, threshold).
- **Pyramids:** shrink factors with smoothing sigmas in voxels or mm; the exact Gaussian kernels.
- **SyN:** symmetric half-way update, Gaussian smoothing of the update field and the total field, inverse-field estimation, `restrict_deformation` weights; BSplineSyN control-point fields.
- **Transform conventions:** LPS; centre of rotation and fixed parameters; composite-transform order; how `collapse_output_transforms` merges stages.
- **Interpolation boundary handling** for each interpolator; the windowed-sinc radius and window.
- **N4:** scattered-data B-spline fitting levels and control points, the histogram-sharpening deconvolution (FWHM, Wiener noise), convergence threshold.

### 6.3 Validation
- **ANTsPy as an in-process oracle.**
  - `pip install antspyx` on the linux-x64, macOS and win-x64 CI runners gives every larmorx-ants test a live reference, without Docker.
  - The same inputs and parameters go to both, including raw antsRegistration arguments via `ants.internal.get_lib_fn`, and outputs are compared.
- **Deterministic configurations** (no random sampling, 1 thread): target transform-parameter relative difference ≤ 1e-4 and displacement ≤ 0.01 mm. Stretch goal: near bit-exact.
- **Random-sampling or multi-threaded configurations** (ANTs itself varies with thread count): within ANTs' own seed/thread variability band. Measure that band by running ANTsPy with several seeds and thread counts.
- **Pipeline-level:** every registration settings file used by fMRIPrep (`niworkflows/data/*.json`, `sdcflows/data/*.json`) becomes a regression test on real data.

### 6.4 Milestones
Sizes are person-months (PM) for one experienced engineer.

| # | Milestone | Size |
|---|---|---|
| A1 | Transforms, ITK transform I/O, interpolators, `apply_transforms` (+ToPoints); CLI-compatible `antsApplyTransforms` | 2–3 |
| A2 | Image filters used by ANTs: smoothing, shrink, histogram matching, morphology, components, distance maps; ImageMath subset, ThresholdImage, ResampleImage, SmoothImage | 2–3 |
| A3 | N4 + DenoiseImage | 2 |
| A4 | Linear registration: metrics, sampling, masks, gradient descent with scales estimation, pyramids, winsorising, histogram matching, initialisers (centre of mass/geometry, antsAI, antsAffineInitializer) | 3–4 |
| A5 | Deformable: SyN, BSplineSyN, multi-metric, `restrict_deformation`, composite/inverse outputs; `.h5` I/O | 3–4 |
| A6 | Atropos | 1–2 |
| A7 | CLI compatibility for antsRegistration, N4, Atropos, DenoiseImage, ImageMath, antsAI; brain-extraction recipe in Python | 1 |
|  | **Total, tier 1** | **14–19** |

---

## 7. FreeSurfer port (`larmorx-freesurfer`)

### 7.1 Where the time goes and why a port can be faster
FreeSurfer 7, single thread, from [02-anatomical…](docs/analysis/02-anatomical-smriprep-niworkflows.md):
- `mri_em_register` + `mri_ca_register`: 1–1.5 h
- `mri_ca_label`: 15–30 min
- `mris_fix_topology`: 10–60 min per hemisphere
- `mris_place_surface`: 30–60 min per hemisphere
- `mris_sphere` + `mris_register`: 40–80 min per hemisphere
- `mris_volmask`: 10–20 min
- total: 5–8 h, or 2.5–4 h with `-openmp`

How a port gets faster:

| Lever | Changes results? |
|---|---|
| One process, data kept in memory (no ~100 program invocations exchanging `.mgz` files; no tcsh) | no |
| Parallel per voxel, vertex, defect and hemisphere, with deterministic reductions | no |
| Better data structures (BVH/spatial hash for intersection tests, cache-friendly meshes) | no |
| SIMD | no (with `libm` and a fixed operation order) |
| Better optimisers for the *same* objective (L-BFGS, multigrid, proper convergence tests) | slightly; validate per step |
| Algorithm substitutions (learned segmentation, other spherical registration) | yes; opt-in "fast" profiles only |

**"Same results"** means differences no larger than FreeSurfer's own cross-version and cross-platform variability (Gronenschild et al. 2012). FreeSurfer is not bit-reproducible across versions or operating systems.

**Speed target, to confirm by profiling:** 30–60 min on 8 cores in faithful mode (about 4–8× faster than FreeSurfer with `-openmp`); under 20 min with validated optimiser upgrades.

### 7.2 Strategy: step by step against FreeSurfer's own intermediates
recon-all writes every step's output to the subject directory, which gives a natural step-level oracle.

| # | Milestone | Size |
|---|---|---|
| R0 | Oracle: the pinned FreeSurfer release (D5) on 20–50 diverse T1w scans (OpenNeuro, CC0), keeping all intermediates and timings; two versions/OSes to measure the variability band | 1 |
| R1 | Foundations: FreeSurfer volume conventions (conformed LIA, `vox2ras-tkr`), MRIS mesh; mgz/surf/curv/label/annot/ctab/LTA/GCA/.gcs/.tif readers and writers | 2–3 |
| R2 | Volume stream: talairach, intensity normalisation, `mri_em_register` → `mri_ca_normalize` → `mri_ca_register` → `mri_ca_label`, `mri_normalize -aseg`, `mri_mask`, `mri_segment`, `mri_edit_wm_with_aseg`, `mri_pretess`, `mri_fill` | 6–10 |
| R3 | Surface stream: tessellate, smooth, inflate, qsphere, `mris_fix_topology`, `mris_remove_intersection`, `mris_autodet_gwstats`, `mris_place_surface` (white, pial, T2/FLAIR), curvature, sphere, `mris_register`, jacobian, `mris_volmask` (= **recon-lite** when complete) | 8–12 |
| R4 | Parcellation and stats: `mris_ca_label`, `mri_surf2volseg`, `mris_anatomical_stats`, `mri_segstats` | 2–3 |
| R5 | Optimisation pass (profiling-driven; optional GPU) | ongoing |
| U | Tier 1 utilities (`mri_vol2surf`, `mri_robust_register`/`mri_robust_template`, `mri_coreg`, `mris_expand`, `lta_convert`, `mri_vol2vol`, `mris_convert`) | 2–3 |
|  | **Total:** recon-lite + utilities ≈ **17–25**; full stream ≈ **22–33** |  |

**Mixed mode:**
- With FreeSurfer installed (Linux/macOS), `lx freesurfer recon --until <step>` hands over to FreeSurfer, and the reverse.
- This keeps the stream usable during the port and lets every step be validated in isolation.

**Third-party code inside FreeSurfer is not covered by MGH's licence:** `talairach_avi` (4dfp tools) and `mri_nu_correct.mni` (MNI N3, Perl/MINC). Plan:
- Replace them with larmorx-ants affine registration to MNI305 and with N4.
- Validate that downstream differences stay within the variability band.

### 7.3 Licence obligations
- **Code:** the `larmorx-freesurfer` crate and wheel carry the FreeSurfer Software License with its required preface, keep attributions, and mark modifications.
- **Audit:** check each ported file for third-party code first.
- **Atlases** (GCA, `.gcs`, folding `.tif`, LUTs, fsaverage) are FreeSurfer data. The default is to download them at first use from the official distribution, after the user accepts the licence (D6). Ask MGH about bundling and about the registration-key requirement.

---

## 8. AFNI ports and clean-room tools (summary)

| Tool | Approach | Key parity details (see analysis docs) | Size |
|---|---|---|---|
| `afni.tshift` | port | Fourier default, `-tzero`, `-ignore N`, `@file` timing, `k-` reversal | 0.5 |
| `afni.volreg` | port | `-Fourier -twopass -zpad 4` | 1 |
| `afni.automask`, `afni.unifize` | port | `-dilate 1`; `-T2 -clfrac 0.2 -rbt 18.3 65 90` | 1 |
| `mri.hmc` | CR | normcorr cost, coarse-to-fine schedule; matrices + params + FD; parallel over volumes | 1–2 |
| `mri.brain_mask` | CR | BET `-f 0.2 -m` behaviour on EPI | 1–2 |
| `mri.tissue` | CR | HMRF-EM + partial-volume estimation; `-N` (no bias) mode | 2–3 |
| `mri.linreg` / `mri.bbr` | CR | FLIRT-style cost functions and search; BBR cost (T2 contrast sign, projection distances, slope); 6/9/12 DOF; surface or WM-boundary input | 2–3 |
| `mri.pepolar` | CR | b02b0-style multi-resolution schedule, bending-energy regularisation, movement estimation, LM → SCG | 3–5 |
| `mri.unwrap` | CR | region-merging/quality-guided 3D unwrapping; compared after the B-spline fit | 1 |
| `mri.bspline_fieldmap` | port (sdcflows, Apache) | knot spacing (16, 16, 10) mm, ridge α = 1e-4, median re-centring; exact separable solve | 0.5–1 |
| `mri.multiecho` | CR | adaptive mask, log-linear + bounded LM fit, T2\* floor/cap, optimal combination | 0.5–1 |
| `mri.maths`, `mri.stats` | CR | fslmaths/fslstats semantics for the operators used | 0.5–1 |
| `mri.surfmap` | CR | ribbon-constrained mapping (voxel subdivision), ADAP_BARY_AREA, geodesic dilation, signed distance, barycentric resampling, CIFTI assembly | 3–5 |
|  | **Total** |  | **≈ 18–27** |

---

## 9. larmorprepx: the pipeline

### 9.1 Orchestration
- Plain Python functions in `pipeline/`. No DAG framework.
- Parallelism lives inside `larmorx` calls (Rust threads) and across BOLD runs and subjects, through a small executor with a CPU/RAM budget. Threads are preferred, since `larmorx` releases the GIL.
- Resume uses a `@stage` cache:
  - The key is the op name + `algo_version` + input content hashes + parameters.
  - Results are stored as NIfTI/NumPy + JSON. No pickles.
  - Directories are short hashed names, which avoids Windows path-length limits.

```python
def bold_pipeline(run, anat, fmap, s):
    bold = io.load_bold(run)                                  # header checks, dummy-scan detection
    ref = mri.hmc_reference(bold, n=20)                       # robust average
    hmc = mri.hmc(bold, reference=ref, n_threads=s.threads)
    stc = afni.tshift(bold, run.slice_timing, tzero=s.slice_time_ref) if run.slice_timing else bold
    sdc = fmaps.align(fmap, ref) if fmap else None            # ants.registration rigid
    coreg = mri.bbr(ref, anat, init=ants.registration(..., stages=[ants.Rigid(...)]))
    outs = {sp: xfm.resample_series(stc, hmc >> sdc >> coreg >> anat.to(sp), sp) for sp in s.spaces}
    return BoldResult(outs, confounds.compute(outs["boldref"], hmc, anat, s), hmc, coreg)
```

### 9.2 Stage → tool mapping

| Stage | fMRIPrep uses | larmorprepx uses |
|---|---|---|
| Conform, average T1w | DenoiseImage, N4, mri_robust_template | `ants.denoise_image`, `ants.n4…`, `freesurfer.robust_template` (interim: `ants` rigid + median) |
| Brain extraction | antsBrainExtraction recipe | `ants.brain_extraction` (same recipe and settings) |
| Tissue segmentation | FAST | `ants.atropos` posteriors (v1); `mri.tissue` if validation requires it |
| Normalisation | antsRegistration SyN (settings JSON, with fallbacks) | `ants.registration` with the same settings |
| Fieldmaps | TOPUP / PRELUDE / B-spline fit / SyN-SDC | `mri.pepolar` / `mri.unwrap` + `mri.bspline_fieldmap` / `ants.registration` SyN restricted to the PE axis |
| BOLD reference, HMC | 3dvolreg, mcflirt | `afni.volreg` or `mri.hmc` |
| STC | 3dTshift | `afni.tshift` |
| Coregistration reference + mask | ANTs premask, N4, BET, 3dUnifize, 3dAutomask | `ants.*`, `mri.brain_mask`, `afni.unifize`, `afni.automask`, or a simplified reimagined mask (validated by Dice) |
| BOLD → T1w | mri_coreg + bbregister / FLIRT-BBR | `ants.registration` (MI) init + `mri.bbr` |
| Multi-echo | t2smap | `mri.multiecho` |
| One-shot resampling | Python (nitransforms + scipy) | Rust `xfm.resample_series` (fused, streamed) |
| Confounds | Python | numpy (fMRIPrep's formulas) |
| Surfaces / CIFTI | recon-all, mri_vol2surf, wb_command | `freesurfer.recon_lite` (or an imported FreeSurfer/FastSurfer subject dir), `freesurfer.vol2surf`, `mri.surfmap` |
| Reports | nireports | matplotlib/nilearn, Jinja2; self-contained HTML |

**Improvements over fMRIPrep, by design:**
- Multi-echo interpolated once.
- No unused 4D resamples.
- Prefilter and Jacobian computed once.
- Surfaces resampled once per subject.
- 4D data streamed.
- Small scratch footprint.

**Interim backend.** Until each `lx.ants` tool is `validated`, the pipeline may call ANTsPy for that step on x64 platforms (`--backend ants=antspy`). This gives an early usable pipeline and a side-by-side comparison. ANTsPy is removed as a dependency at v1.0.

---

## 10. Packaging and deployment
- **Wheels:** maturin + PyO3 abi3 (CPython ≥ 3.12). Built in CI for:
  - linux-x64 and linux-aarch64 (manylinux_2_28)
  - macOS arm64 and x64
  - Windows x64 and arm64
- **Standalone `larmorx` binaries:** cargo-dist for the same targets.
- **Install the pipeline:** `uv tool install "larmorx[prep]"`, which provides the `larmorprepx` command.
  - `larmorprepx templates fetch` and `--offline-bundle` for air-gapped HPC.
  - `larmorprepx doctor` checks the environment: disk, RAM, cgroups, Windows long paths, synced folders.
- **Docker/Apptainer** images built from the same wheels (optional, for HPC).
- **Supply chain:** PyPI trusted publishing, SBOM, `cargo deny`. The licence policy is enforced: no GPL or non-commercial crates; FreeSurfer-licensed code only in `larmorx-freesurfer`.
- **Cross-platform checklist** (from [04-engine-portability.md](docs/analysis/04-engine-portability.md)):
  - `spawn` only, never `fork`
  - ≤ 61 worker processes on Windows
  - no `shell=True`; paths with spaces handled
  - UTF-8 I/O
  - retries on antivirus/memmap file locks
  - no symlinks
  - case-insensitive filesystems
  - cgroup memory limits
  - no thread oversubscription (Rust × BLAS × parallel runs ≤ cores)

---

## 11. Validation

### 11.1 Oracles

| Oracle | Provides | Where it runs |
|---|---|---|
| **ANTsPy** (pip) | live in-process reference for every `lx.ants` tool | CI: linux-x64, macOS, win-x64 |
| SciPy / scikit-image / nitransforms / sdcflows code | references for interpolation, morphology, transforms, B-spline fit | CI: all targets |
| AFNI binaries | `afni.*` references (fixtures; the binaries themselves run only on Linux/macOS) | fixture generation on Linux |
| FreeSurfer (pinned, Docker) | step-level intermediates for the port; variability band | fixture generation on Linux |
| FSL binaries | black-box behaviour for clean-room specs and comparisons (only under licence terms that allow it, D7) | fixture generation on Linux |
| fMRIPrep (pinned, Docker) | end-to-end derivatives and intermediates | fixture generation on Linux x64 |

- Fixtures are content-addressed in object storage or GitHub Releases, never in git.
- Every oracle is run several times with different seeds and thread counts, and on neighbouring releases. The observed spread **defines the acceptance band**.

### 11.2 Levels
1. **Unit/property tests:** every kernel against its oracle; thread-count invariance; parser fuzzing; all targets, on every PR.
2. **Tool level:** every catalog tool against its oracle on real data; on every PR (subset) and nightly (full).
3. **CLI compatibility:** the same argument lists to larmorx and to the original tool, compare outputs. This includes all fMRIPrep settings files.
4. **Pipeline level:** larmorprepx vs fMRIPrep on ds000005, ds000054, ds000210, ds001600/HCP (PEPOLAR), ds001771/ds000206 (phasediff), plus an OpenNeuro diversity set (oblique, anisotropic, 7T, multiband + sbref, long/short runs, lesions, pediatric/elderly); weekly and at release.
5. **FreeSurfer step level:** each ported step against FreeSurfer intermediates.
6. **Scientific equivalence:** GLM z-maps, connectivity, thickness statistics vs the reference tools' cross-version band; each minor release.
7. **Cross-platform reproducibility:** identical output hashes for deterministic tools across the six targets; nightly.

### 11.3 Acceptance metrics (initial; calibrated by the variability bands)

| Output | Metric | Threshold |
|---|---|---|
| Deterministic arithmetic, STC, confounds | max relative difference | ≤ 1e-6 (f64) / 1e-5 (f32) |
| Interpolation | max relative difference vs reference | ≤ 1e-6 (SciPy), ≤ 1e-4 (ITK) |
| ANTs linear registration, deterministic configuration | parameter relative difference; displacement | ≤ 1e-4; ≤ 0.01 mm |
| Registration, general | displacement in mask, mean / 95th percentile | ≤ 0.2 / 0.5 mm, or within band |
| SyN | warped-label Dice; displacement RMSE | within the ANTs band; Dice ≥ 0.97 |
| N4 / denoise | NCC | > 0.995 |
| Masks / segmentation | Dice | ≥ 0.98 / ≥ 0.95 (GM/WM), ≥ 0.90 (CSF) |
| Motion parameters | median difference; FD correlation | ≤ 0.05 mm / 0.05°; r > 0.99 |
| Fieldmaps | RMSE in mask | ≤ 2 Hz |
| Preprocessed BOLD | temporal-mean NCC; tSNR r; per-voxel time-series r (median) | > 0.99; > 0.98; > 0.98 |
| FreeSurfer surfaces | vertex distance, median / 95th percentile | ≤ 0.1 / 0.5 mm, or within band |
| FreeSurfer thickness / aseg | ICC, mean abs difference / Dice | > 0.95, ≤ 0.05 mm / ≥ 0.95 |

---

## 12. Licensing

| Source | Licence | Treatment |
|---|---|---|
| ANTs, ITK | Apache-2.0 | **port**; keep LICENSE and NOTICE, mark modifications, record files in PROVENANCE |
| AFNI | public domain (+ per-file third-party exceptions) | **port** after a per-file check |
| FreeSurfer | FreeSurfer Software License v1.0 | **port** into `larmorx-freesurfer` only; carry the licence and preface; audit third-party files; atlases per D6 |
| sdcflows / fMRIPrep / nitransforms code | Apache-2.0 / MIT | may port with attribution |
| FSL | non-commercial | **clean-room only**: never read the source (not cloned) |
| Connectome Workbench | GPL-2.0-or-later | **clean-room only**: implement from the documentation; never read the source |
| MSM_HOCR | non-commercial; patent-encumbered optimiser | avoid; fsaverage-based fsLR registration for now |
| tedana | LGPL-2.1 | implement from the published equations; tedana is a test oracle only |
| Our own code | **Apache-2.0** (D2) | – |

**Clean-room practice:**
- Move the `workbench/` and `MSM_HOCR/` clones to `reference_src/`, outside the working tree.
- Implementers of clean-room tools, including AI assistants, must not consult them.
- `specs/<tool>.md` is the only input besides published papers and documentation.

**Naming:**
- larmorprepx and larmorx must not present themselves as fMRIPrep, ANTs, FreeSurfer, AFNI or FSL.
- Ported families keep upstream tool names for compatibility (with attribution).
- Clean-room tools use neutral names (D4).

---

## 13. Roadmap

Five tracks; they can run in parallel once L0 exists. Sizes are person-months for one experienced engineer, for prioritisation.

| Track | Content | Size |
|---|---|---|
| **L Foundation** | larmorx-core/io/interp/transform/optim/image/mesh/cli; Python type layer; wrapper and stub conventions; docs generator; CI on the six targets; oracle harnesses | 3–5 |
| **A ANTs port** | A1–A7 (§6.4) | 14–19 |
| **C AFNI + clean-room** | §8 | 18–27 |
| **F FreeSurfer port** | R0–R4 + utilities (§7.2); recon-lite first | 17–25 (recon-lite) / 22–33 (full) |
| **P Pipeline** | orchestration, BIDS, cache, confounds, reports, derivatives, CLI, validation study | 6–9 |
|  | **Total: about 58–93 PM** (about 1.5–2 years with a team of 4) |  |

### Release milestones

| Milestone | What users get | Depends on |
|---|---|---|
| **M1 Preview** | larmorprepx on x64 platforms; larmorx resampling, `mri.hmc`, `afni.tshift`, `image_math`; ANTsPy interim backend for ANTs steps | L, P (part), A1, some of C |
| **M2 Native linear** | `lx.ants` linear registration, N4, denoise, apply_transforms validated; BOLD pipeline native on **all six targets** (SyN still interim) | A1–A4, C (hmc, masks, bbr) |
| **M3 v1.0** | SyN + Atropos + brain extraction native; ANTsPy dropped; all SDC estimators; volumetric pipeline fully native everywhere; validation report | A5–A7, C (pepolar, unwrap, multiecho), P |
| **M4 Surfaces from an existing FreeSurfer subject dir** | `mri.surfmap`, `freesurfer.vol2surf`, CIFTI outputs | C (surfmap), F (utilities) |
| **M5 Native surfaces** | recon-lite in Rust; larmorprepx fully native including surfaces | F (R0–R3) |
| **M6 Full library** | full recon-all stream; tier 2 tools across families | F (R4), tier 2 |

---

## 14. Risks

| Risk | Mitigation |
|---|---|
| Scope (about 60–90 PM) | Tiered catalog; release milestones that are useful on their own; interim backends; recon-lite before the full stream |
| ANTs parity (MI sampling, optimiser scales, SyN regularisation) | ANTsPy in-process oracle on every PR; deterministic configurations first; variability bands for stochastic ones |
| FreeSurfer port subtleties | Step-level oracle; mixed mode; profiling before optimising |
| "Same results" disputes | Published variability-band methodology and metrics per tool; status labels (`experimental` / `validated` / `stable`) |
| Licence contamination | Clean-room practice; PROVENANCE; `cargo deny`; separate FreeSurfer wheel |
| API sprawl (three interfaces per tool) | CLI parsing written once in Rust; Python wrappers and stubs partly generated; CI enforces the per-tool contract |
| Windows/ARM-specific bugs | All six targets in CI from day one; deep-path and spaces-in-path tests |

---

## 15. Next steps (first 2–4 weeks)

1. ~~Create the repo~~ (done 2026-10-09: `github.com/karellopez/larmorx`, public). Optionally reserve a `larmorx` GitHub org later and transfer the repo there; GitHub redirects old URLs. Settle the remaining decisions in §16.
2. Install Rust (`rustup`, stable). It is not installed on this machine yet.
3. `git init`; scaffold the Cargo workspace, `larmorx-core`, `larmorx-io`, `larmorx-py`, `larmorx-cli`, `python/larmorx` (with `pipelines/larmorprepx/`); set up a CI matrix that builds, imports and runs `larmorx --version` on all six targets. Publish a real minimal 0.0.1 to PyPI and crates.io soon after, to claim the names.
4. Pin the upstream versions to port (ANTs 2.6.x and ITK v5.4.7 tags, AFNI 25.2.09, FreeSurfer per D5) and record them in `UPSTREAM.md`. Move `workbench/` and `MSM_HOCR/` to `reference_src/`.
5. Implement `Image`/`Affine`/`Transform` (Rust + Python) and NIfTI I/O, with round-trip tests against nibabel.
6. First tools, with the full per-tool contract:
   - `lx.ants.apply_transforms` (A1), validated against ANTsPy in CI
   - `lx.afni.tshift`
7. Pipeline skeleton: BIDS indexer, `@stage` cache, executor, derivatives writer. Then a first end-to-end path on ds000005 using the interim backends.
8. Oracles: on a Linux x64 machine, run fMRIPrep (ds000005, `--fs-no-reconall`, fixed seeds) and FreeSurfer (D5 version) on 3–5 T1w scans, keeping intermediates and timings.

---

## 16. Decisions needed from you

| # | Decision | Recommendation |
|---|---|---|
| D1 | Names | **Decided 2026-10-09:** project/package `larmorx` (repo `github.com/karellopez/larmorx`, `import larmorx as lx`, CLI `larmorx`/`lx`, crates `larmorx-*`); pipeline `larmorprepx`, inside the package. All were free on PyPI, crates.io and GitHub that day |
| D2 | Licence of our code | **Decided 2026-10-09:** Apache-2.0 |
| D3 | Distributions | **Decided with D1:** wheel `larmorx` (library + larmorprepx pipeline via the `[prep]` extra) and wheel `larmorx-freesurfer` (licence isolation); plus standalone CLI binaries |
| D4 | Names for clean-room tools | **Neutral names** (`mri.hmc`, `mri.brain_mask`, `mri.pepolar`, …) with documented compatibility ("accepts mcflirt-style options"), not the FSL/Workbench program names |
| D5 | FreeSurfer version to port | **v8.2.0**, also validated against 7.4.x |
| D6 | FreeSurfer atlas data | **Download at first use after licence acceptance**, until MGH confirms whether bundling is allowed |
| D7 | Is the project (or its main users) commercial? | Determines whether FSL and MSM binaries may be run, even for validation, and frames the FreeSurfer licence questions |
| D8 | Output compatibility | **fMRIPrep-compatible derivative names and confound columns** |
| D9 | Minimum Python | **3.12** |
| D10 | First milestone | **M1 Preview** (pipeline usable early on x64 with interim ANTsPy), while the ANTs port proceeds |

---

## Appendix A: upstream snapshot (shallow clones next to this repo)

| Repo | SHA | Date | Licence |
|---|---|---|---|
| nipreps/fmriprep | 21a490f | 2026-10-07 | Apache-2.0 |
| nipreps/smriprep | 10d70ee | 2026-10-05 | Apache-2.0 |
| nipreps/niworkflows | dacf491 | 2026-10-07 | Apache-2.0 |
| nipreps/sdcflows | d0dd398 | 2026-10-02 | Apache-2.0 |
| nipreps/nireports | e4ef786 | 2026-10-07 | Apache-2.0 |
| nipy/nitransforms | 06537ca | 2026-09-15 | MIT |
| nipy/nipype | 599a762 | 2026-10-07 | Apache-2.0 |
| nipy/nibabel | 4be8b5c | 2026-10-07 | MIT |
| nipy/nitime | a014ce0 | 2026-08-14 | BSD |
| bids-standard/pybids | 47eb9fb | 2026-08-24 | MIT |
| templateflow/python-client | 06ccc7f | 2026-10-07 | Apache-2.0 |
| ME-ICA/tedana | 45dd96d | 2026-10-07 | LGPL-2.1 |
| nipreps/acres, migas-py | 46fa586, 4c721fe | – | Apache-2.0 |
| ANTsX/ANTs | 0f65b0e (master; port from the 2.6.x tag) | 2026-09-22 | Apache-2.0 |
| InsightSoftwareConsortium/ITK | dfef0816 (master; port from v5.4.7) | 2026-10-08 | Apache-2.0 |
| afni/afni | 0eb4d34 | 2026-10-07 | public domain (+ exceptions) |
| freesurfer/freesurfer | 766ac05 (dev after v8.2.0) | 2026-10-07 | FreeSurfer Software License v1.0 |
| Washington-University/workbench | 9906328 | 2026-10-06 | GPL-2.0-or-later (clean-room: do not read) |
| ecr05/MSM_HOCR | c9d8996 | 2022-07-21 | non-commercial (do not read) |

FSL is not cloned. Installed in `.venv` for evaluation: fmriprep 26.0.0.dev with its dependencies, antspyx 0.6.3, SimpleITK 2.5.6.

## Appendix B: method references
- **fMRIPrep:** Esteban et al. 2019, *Nat Methods*.
- **ANTs:**
  - SyN: Avants et al. 2008, *Med Image Anal*
  - ANTs reproducibility/evaluation: Avants et al. 2011, *NeuroImage*
  - N4: Tustison et al. 2010, *IEEE TMI*
  - Atropos: Avants et al. 2011, *Neuroinformatics*
  - DiReCT: Das et al. 2009, *NeuroImage*
- **ITK v4 registration:** Avants et al. 2014, *Front Neuroinform*.
- **Mattes MI:** Mattes et al. 2003, *IEEE TMI*.
- **NLM denoising:** Manjón et al. 2010, *JMRI*.
- **FreeSurfer:**
  - Dale et al. 1999 and Fischl et al. 1999, *NeuroImage*
  - Fischl et al. 2002, *Neuron*
  - Fischl et al. 2004, *Cereb Cortex*
  - Ségonne et al. 2007, *IEEE TMI*
  - Reuter et al. 2010, *NeuroImage*
  - Greve & Fischl 2009, *NeuroImage*
  - Gronenschild et al. 2012, *PLoS ONE* (reproducibility)
- **FSL methods:**
  - Jenkinson & Smith 2001, *Med Image Anal*
  - Jenkinson et al. 2002, *NeuroImage*
  - Smith 2002, *Hum Brain Mapp*
  - Zhang et al. 2001, *IEEE TMI*
  - Andersson et al. 2003, *NeuroImage*
  - Jenkinson 2003, *MRM*
- **AFNI:** Cox 1996, *Comput Biomed Res*.
- **Multi-echo:** Posse et al. 1999, *MRM*; Kundu et al. 2012, *NeuroImage*.
- **HCP / CIFTI:** Glasser et al. 2013, *NeuroImage*.
- **Confounds:** Behzadi et al. 2007 (CompCor); Power et al. 2012 (FD/DVARS), *NeuroImage*.
