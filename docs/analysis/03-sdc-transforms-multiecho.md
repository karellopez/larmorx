<!-- Generated 2026-10-08 by a read-only analysis agent against shallow clones in the parent directory.
     fmriprep @ 21a490fb (26.0.0.dev). Line numbers refer to those clones and will drift. -->

## Report: sdcflows / nitransforms / tedana / fMRIPrep resampling (read-only analysis)

All paths are relative to the workspace root (the directory that contains `larmorx/` and the upstream clones).

**Key files**
- `sdcflows/sdcflows/transform.py`
- `sdcflows/sdcflows/interfaces/bspline.py`
- `sdcflows/sdcflows/workflows/fit/{pepolar,fieldmap,syn}.py`
- `sdcflows/sdcflows/workflows/apply/{registration,correction}.py`
- `fmriprep/fmriprep/interfaces/resampling.py`
- `nitransforms/nitransforms/{resampling,base,nonlinear,linear,manip}.py`, `.../interp/bspline.py`, `.../io/*.py`
- `tedana/tedana/{decay,combine,utils,io}.py`, `.../workflows/t2smap.py`
- `fmriprep/fmriprep/interfaces/multiecho.py`, `.../workflows/bold/t2s.py`

---

## A) sdcflows: SDC strategies

**Dispatcher.** `sdcflows/fieldmaps.py:462-506` maps each estimator type to a workflow:
- MAPPED or PHASEDIFF → `init_fmap_wf`
- PEPOLAR → `init_topup_wf`, and only that. **`init_3dQwarp_wf` is never dispatched.**
- ANAT (fieldmap-less) → `init_syn_sdc_wf`

fMRIPrep wiring:
- `fmriprep/workflows/base.py:701` calls `init_fmap_preproc_wf`.
- `:767-781` limits PEPOLAR to one modality, or two images among epi/bold/sbref.
- `:783-815` adds `init_syn_preprocessing_wf` (with `debug=sloppy`, `auto_bold_nss=True`).

### A1. PEPOLAR via TOPUP (`workflows/fit/pepolar.py:37-267`)

| Step | file:line | Binary and parameters | Already pure Python? |
|---|---|---|---|
| Readout time | 140-149, `utils/epimanip.py:29-296` | none | yes (metadata only) |
| SelectPEVolumes (≤3 per PE), SortPEBlips, MergeSeries | 152-160 | none | yes |
| UniformGrid | `interfaces/utils.py:112-170` | none | yes; uses nitransforms `Affine.apply` (cubic) only when grids differ |
| Reorient to LAS | 162 | none | yes |
| **TOPUP** | 165, config chosen at 206-222 | **FSL `topup`**. Config from `_select_topup_config` (417-473): `b02b0_{4,2,1}[_quick].cnf`, picked by whether all dims divide by 4 or 2. `b02b0_2.cnf`: warpres 20,16,14,12,10,6,4,4,4; subsamp 2,2,2,2,2,1,1,1,1; fwhm 8,6,4,3,3,2,1,0,0; miter 5×5 then 10,10,20,20; lambda 5e-3…1e-11; ssqlambda 1; regmod bending_energy; estmov on for the first 5 levels; minmet LM×5 then SCG×4; splineorder 3; numprec double; interp spline; scale 1 | no |
| TOPUPCoeffReorient | `interfaces/bspline.py:494-537`, `_fix_topup_fieldcoeff` 568-635 | none | yes. Flips LR; negates if PE axis is i; affine = reference RAS affine × factors, centred; checks shape == ref//factor + 3·(factor>1) |
| Reorient back from LAS | 226-237 | none | yes |
| RobustAverage of `out_corrected` | 170 | **AFNI `3dvolreg`** (hidden: niworkflows `images.py:214-219` defaults `mc_method='AFNI'`; `:318-332` uses Fourier, `-twopass`, zpad 4), then temporal median | partly |
| `init_brainextraction_wf` | `workflows/ancillary.py:33-114` | IntensityClip (Python: `median_filter` with ball(3), percentiles 35/99.98; niworkflows `nibabel.py:667-726`) → **ANTs `N4BiasFieldCorrection`** (dim 3, 5×50 iterations, conv 1e-7, shrink 4, n_procs 8) → IntensityClip(0.01, 99.9) → BrainExtraction (Python, `utils/tools.py:153-224`, skimage `random_walker`) | mixed |
| Debug only: ApplyCoeffsField(jacobian=True) instead of TOPUP's field | 241-264 | none | yes |

fMRIPrep consumes `fmap_coeff` (reoriented TOPUP coefficients), `fmap` (TOPUP field in Hz), `fmap_ref` and `fmap_mask`. `jacobians`, `xfms` and `out_warps` are exposed but not used by the BOLD path.

### A2. PEPOLAR via 3dQwarp (`pepolar.py:270-414`): legacy and unreachable

- Binaries: `mri_robust_template` (StructuralReference, 338-351), `init_enhance_and_skullstrip_bold_wf` twice, `antsRegistration` with `translation_rigid.json` (356-363: Translation→Rigid, Mattes 64 bins, smoothing 8/2 mm, Lanczos), AFNI `3dQwarp -plusminus` (365-376: blur −1/−1, minpatch 9, nopadWARP, noweight, pblur 0.05, `-noXdis -noZdis`-style flags from `_sorted_pe`, OMP threads ≤4), and `antsApplyTransforms` with LanczosWindowedSinc (382-389).
- It looks broken: it connects `inputnode.in_reference` (408-409), but that field is not declared (327).
- **Recommendation:** drop it from scope.

### A3. Phase-difference and phase1/phase2 (`workflows/fit/fieldmap.py:32-194, 264-364`)

| Step | file:line | Binary | Pure Python? |
|---|---|---|---|
| CheckRegister | `interfaces/fmap.py:245-348` | **FreeSurfer `mri_robust_register`** (auto_sens), only when the two magnitude affines differ (308-313); LTA read with nitransforms; thresholds 0.02 rad / 1 mm | otherwise yes |
| magnitude_wf: IntraModalMerge(hmc=False) → brainextraction_wf | 197-261 | ANTs N4 (in brainextraction) | merge is Python |
| PhaseMap2rads | `utils/phasemanip.py:26-46` | none | yes (min-max rescale to [0, 2π]) |
| SubtractPhases | `phasemanip.py:49-81`, `fmap.py:77-103` | none | yes (wraps to [0, 2π]; pass-through for a single phasediff) |
| **Phase unwrapping** | `fieldmap.py:345` | **FSL `prelude`** (nipype PRELUDE: `-a mag -p phase -m mask`, default options). No Python unwrapper and **no FUGUE anywhere.** | no |
| Phasediff2Fieldmap | `phasemanip.py:84-96` | none | yes (÷ 2π·ΔTE) |
| BSplineApprox | 120-127 | none | yes: spacing (16, 16, 10) mm, ridge α 1e-4, recenter=median, extrapolate, zooms_min 1.0 (4.0 when sloppy), mask = magnitude mask |

Equivalence note: `recenter='median'` (bspline.py:183-188) removes the global 2πk ambiguity left by PRELUDE. Compare unwrapped phase modulo a global offset, or compare after the B-spline fit.

### A4. Directly measured B0 map (`fieldmap.py:168-192`)

IntraModalMerge (Python mean) → CheckB0Units (`fmap.py:140-157`; divides rad/s by 2π) → BSplineApprox. The magnitude goes through magnitude_wf (ANTs N4).

### A5. Fieldmap-less SyN-SDC (`workflows/fit/syn.py`)

**Preprocessing, `init_syn_preprocessing_wf` (342-597)**
- Deoblique (Python).
- `init_epi_reference_wf` (niworkflows `workflows/epi/refmap.py`): RobustAverage → AFNI 3dvolreg; N4; `mri_robust_template`.
- BrainExtraction (Python random walker); ApplyMask; IntensityClip(0, 99.8).
- **ANTs `DenoiseImage`** (482).
- **antsRegistration** EPI→anat with `data/affine.json` (484-492): Rigid then Affine, Mattes 32 bins, centre-of-mass init, iterations [[1000,0],[250,100]], shrink [[4,1],[2,1]], smoothing [[6,0] mm, [2,1] vox], histogram matching, winsorize 0.001–0.998.
- anat→EPI and mask→EPI via `antsApplyTransforms` (Linear and MultiLabel).
- **SD prior** (519-529): `fmap_atlas.nii.gz` taken through [atlas→MNI `.mat`, std2anat, inverse of epi2anat] with antsApplyTransforms, then Binarize at `atlas_threshold=3` mm.

**Core, `init_syn_sdc_wf` (46-339)**
- **ANTs `ImageMath Laplacian 1.5 1`** on the anat and EPI (188-202), then `_norm_lap` (Python, 719-742).
- IntensityClip(35, 99.9) on the EPI; BinaryDilation (Python, ball radius 3); amask2epi (ANTs MultiLabel, identity); Union.
- **antsRegistration with `data/sd_syn.json`** (sloppy: `sd_syn_sloppy.json`), 234-253:
  - two SyN stages, each with two Mattes metrics (intensity and Laplacian), 48 bins;
  - metric weights [0.9, 0.1] and [0.8, 0.2]; `laplacian_weight` overrides them, clamped to ≤0.5 (242-250);
  - iterations [200, 100] then [10] (sloppy: [20, 10] then [2]); shrink [1, 1] then [1]; smoothing [2, 0] then [0] vox;
  - transform parameters [0.8, 6.0, 10.0/pe_res] and [0.8, 2.0, 0.5/pe_res] (`_mm2vox`, 626-642);
  - `restrict_deformation` 1.0 along PE and **0.1** on the other axes (`_warp_dir`, 600-623);
  - fixed masks [anat_mask, sd_prior], moving masks ['NULL', epi_union];
  - histogram matching; winsorize 0.001–0.998; convergence 1e-6/1e-8 with windows 5/2; requires ANTs ≥ 2.2 (141).
- DisplacementsField2Fieldmap (Python, `transform.py:624-674`).
- zooms_field via **antsApplyTransforms `-n BSpline -u float`**, identity, onto the EPI grid (261-268).
- BSplineApprox(recenter=False, no mask) (271-279).
- ApplyCoeffsField(jacobian=False) produces `fmap_ref` and `fmap` (258).
- Dead node: `zooms_epi` (225-226) gets inputs (304, 307) but its output goes nowhere.

### A6. Fieldmap→BOLD registration (`workflows/apply/registration.py:42-171`)

- BinaryDilation radius 5 (Python) on both masks (124-125).
- **antsRegistration with `fmap-any_registration.json`** (129-137): Rigid then Rigid, Mattes 32 bins, iterations [50] then [20], shrink 2 then 1, smoothing 8 then 2 mm, Random sampling 0.25/0.5, step 0.1, centre-of-mass init, Lanczos, histogram matching, winsorize 0.001–0.999. Sloppy uses the `_testing` variant.
- Fixed = fmap_ref, moving = BOLD reference.
- fMRIPrep converts the result to ITK text (`bold/fit.py:415-447`).

### A7. How the field is represented and applied

**Representation**
- One float32 NIfTI of cubic B-spline coefficients (Hz) per level; its affine maps knot index → RAS.
- `bspline_grid` (`bspline.py:540-565`): knot affine = control spacing × image direction cosines; shape = extent//spacing + 3; grids centred on each other.
- Spacing defaults (`bspline.py:49-51`): mid (40, 40, 20) mm, low (100, 100, 40) mm, high (16, 16, 10) mm — the workflows use high.
- Voxel shift map (VSM) = Hz × signed total readout time, in voxels along PE.

**Fitting (`BSplineApprox`, `bspline.py:128-283`)**
- Design matrix: `grid_bspline_weights` (`transform.py:677-766`) evaluates scipy `BSpline` per axis into `lil_array`, then `kron(kron(Wx, Wy), Wz)`.
- Solve: sklearn `Ridge(alpha=1e-4, fit_intercept=False)` with up to 3 attempts, failing if max|coefficient| ≥ 1e4 (203-216). For sparse input sklearn's auto solver should be `sparse_cg` with tol 1e-4 — confirm against the pinned sklearn. So the reference result is itself an approximate CG solution.
- **The design matrix is always the full grid**: the mask only zeroes the data (189), and the fit uses `colmat` over every voxel (205).
- Then field = `colmat[mask] @ coef`; extrapolation = `colmat[~mask] @ coef`; also writes an error map.
- **Rust opportunity:** with one knot level (the default), XᵀX = Ax⊗Ay⊗Az. The ridge solve can be done exactly with three small per-axis eigendecompositions and mode-n products, never forming the N×K matrix. Evaluation is also separable: three 1D passes at 4 taps each instead of 64 taps per voxel.

**sdcflows application (`ApplyCoeffsField` → `B0FieldTransform`)**
- `fit()` (`transform.py:254-367`) builds `colmat` on the target grid. When grids are not aligned (approx mode) it reconstructs on a grid 4× denser than the knots (`deoblique_and_zooms`, `utils/tools.py:28-93`), then cubic-resamples with a nitransforms Affine (365).
- `apply()` (369-549):
  - `ensure_positive_cosines`;
  - pixel coordinates from nitransforms;
  - for each volume, `_sdc_unwarp` (71-117) adds the VSM along PE, calls `ndi.map_coordinates(order=3, mode='constant', prefilter=True)`, and multiplies by `1 + np.gradient(vsm)` for the Jacobian;
  - volumes run in parallel through `asyncio` with `run_in_executor` (120-239), at most min(cpu, 12) at once;
  - **head-motion transforms are silently ignored** (509-524).
- Used for the SyN `fmap_ref`, TOPUP debug mode, and `init_unwarp_wf` (`apply/correction.py:146-150`), which fMRIPrep does not use.

**fMRIPrep application (where SDC is really applied)**
- `ReconstructFieldmap` (`resampling.py:158-188, 637-725`) plus `ResampleSeries` (`77-128, 235-611`).
- No `applytopup` and no `antsApplyTransforms` for BOLD SDC.

---

## B) nitransforms and fMRIPrep resampling

### Formats (nitransforms never runs a binary in its source code; only tests do)

| Format | Read | Write | Code |
|---|---|---|---|
| ITK/ANTs affine `.tfm`/`.txt` (single and list) | yes | yes | `io/itk.py:20-329` |
| ITK `.mat` (MATLAB binary via `scipy.io.loadmat`/`savemat`) | yes | single transform only (arrays must use `.h5`) | `itk.py:5, 64-66, 106-109, 233-236` |
| ITK composite `.h5` (AffineTransform and DisplacementFieldTransform; handles the "Tranform" typo) | yes | no | `itk.py:368-434` (h5py) |
| ITK displacement NIfTI (5D, LPS) | yes | yes | `itk.py:331-365` |
| FSL `.mat` (needs reference and moving images: `_fsl_aff_adapt`) | yes | yes | `io/fsl.py:20-177, 211-224` |
| FSL displacement field (x negated) | yes | yes | `fsl.py:179-208` (no fnirt coefficient support) |
| AFNI `.aff12.1D` (single and array; obliquity and WARPDRIVE emulation) | yes | yes | `io/afni.py:22-194, 228-393` |
| AFNI displacement field | yes | yes | `afni.py:196-225` |
| FreeSurfer LTA (VolumeGeometry, ras2ras and vox2vox) | yes | yes | `io/lta.py:27-413` |
| X5 (BIDS-transforms HDF5; affine, dense field, B-spline) | yes | yes | `io/x5.py`, `linear.py:457`, `nonlinear.py:497`, `manip.py:206-288` |

Dependencies: numpy ≥2, scipy ≥1.10, nibabel ≥5.1.1, h5py ≥3.11 (`pyproject.toml:22-27`).

### Resampling implementation
- `resampling.apply` (`resampling.py:135-346`):
  - coordinates: reference RAS grid (`ImageGrid.ndcoords`, `base.py:202-216`, built with `np.mgrid` as int64 then a float32 homogeneous product in `_as_homogeneous`/`_apply_affine`, 351-384) → `transform.map` → `ImageGrid.index`;
  - **3D+t with ≥8 resamplings (`SERIALIZE_VOLUME_WINDOW_WIDTH`, line 35)**: one `ndi.map_coordinates` per volume, through asyncio and `run_in_executor` (45-132), max min(cpu, 12);
  - fewer volumes: one 4D `map_coordinates` call that includes time as a coordinate (304-319). At order 3 that is 256 taps per voxel instead of 64.
  - Defaults: order 3, mode 'constant', cval 0, prefilter True; input dtype capped by `dtype_width=8`.
- `Affine.map` / `LinearTransformsMapping.map`: a 4×4 matrix product (`linear.py:252-282, 378-429`).
- `TransformChain.map` applies transforms in sequence (`manip.py:129-157`).
- `DenseFieldTransform.map` (`nonlinear.py:124-209`): if the points are on the grid, a lookup; otherwise **three cubic `map_coordinates` calls (one per field component, prefilter recomputed each time, cval NaN)**. This is how fMRIPrep pushes target coordinates through the anat→MNI `.h5` warp (typically a 193×229×193×3 field), once per run.
- `BSplineFieldTransform.map` (`nonlinear.py:457-494, 522-553`): a **pure-Python loop over points**, extremely slow, not used by fMRIPrep. `to_field` (410-429) uses `interp/bspline.py:34-97`: dense distance matrix per axis, float16 positions, CSR, `kron`.

### fMRIPrep one-shot resampler (`fmriprep/interfaces/resampling.py`)
- `ResampleSeries` (77-128):
  - load the transform chain (`utils/transforms.py:8-34`: `.h5` via `nt.manip.load`, otherwise `nt.linear.load`);
  - the HMC `LinearTransformsMapping` must come last; each motion matrix is converted to voxel-to-voxel (578);
  - all other transforms are chained with `Affine(ras2vox)` and mapped once (583-584);
  - the **whole 4D source is read as float32** (593);
  - `resample_series_async` (321-427): one `resample_vol` task per volume through `asyncio` with a semaphore and the default ThreadPoolExecor (`utils/asynctools.py:5-8`); output is preallocated in Fortran order.
- `resample_vol` (235-318), the **main hot loop**, per volume:
  - `nb.affines.apply_affine(hmc, coords)` over the full target grid (295);
  - add `fmap_hz * ro` along the PE axis (302-303);
  - `ndi.map_coordinates(order=3, prefilter=True)`, which builds a float64 `spline_filter` of the volume every time (305);
  - Jacobian `1 + np.gradient(vsm, axis=pe)` (316). This is constant across volumes but recomputed each time, and it is taken along the *target* array axis.
- **Default mode is `'grid-constant'`** (first value of the Enum, 56-68). sdcflows and nitransforms default to `'constant'`. A Rust port must reproduce SciPy's boundary rules exactly: the prefilter mirrors at the edges; `grid-constant` pre-pads 12 voxels.
- Threads: `n_procs=omp_nthreads` on the nodes (`bold/fit.py:510, 774`; `bold/apply.py:131`) is passed to `num_threads` by nipype (`nipype/pipeline/engine/nodes.py:202-203, 299-300`). Any thread speedup depends on SciPy's ndimage C code releasing the GIL, which I did not check against the installed SciPy.
- `reconstruct_fieldmap` (637-725): if the chain collapses to an aligned affine, it builds `colmat` directly on the target grid. For ~0.7–1M target voxels that is roughly 45–70M non-zeros, several hundred MB. Otherwise it reconstructs on the fieldmap grid and cubic-resamples with `nt.resampling.apply` (718).

---

## C) tedana / multi-echo

**How fMRIPrep calls it**
- `T2SMap` is a nipype `CommandLine` that runs the **`t2smap` console script in a subprocess** (`fmriprep/interfaces/multiecho.py:45-119`, `_cmd='t2smap'` at 104).
- Arguments: `-d <echo files> -e <TE in ms>` (×1000 at 108-111) `--mask` (BOLD mask dilated by 2, `bold/t2s.py:119`) `--fittype <method>` `--exclude 0:<dummy scans>`.
- **Defaults differ:** fMRIPrep uses **`curvefit`** (`config.py:645`, CLI flag at `cli/parser.py:493`); tedana itself defaults to `loglin`.
- `--n-threads` is not passed, so tedana uses 1 thread.
- fMRIPrep reads `T2starmap.nii.gz` and `desc-optcom_bold.nii.gz` (`S0map` is also declared, 113-119).
- Wiring: each echo goes through `ResampleSeries` separately (`bold/fit.py:773-790`), then a JoinNode feeds `init_bold_t2s_wf` (804-836).
- The T2* report (`t2s.py:140-229`) uses ANTs MultiLabel and nireports, not tedana.
- Version pin: `tedana >= 25.1.0` (`fmriprep/pyproject.toml:42`).

**tedana's t2smap code path**
- CLI defaults (`workflows/t2smap.py`): fittype loglin (163), masktype `['dropout']` (125), fitmode `all` (174), combmode `t2s` (181), n-threads 1 (220); `threadpool_limits` set at 565-567.
- Steps: load and mask (`io.load_data_nilearn`, `io.py:1084-1170`; direct nibabel with a nilearn fallback) → `make_adaptive_mask` with threshold 1 (`utils.py:33-241`) → `t2smap_subworkflow` (`decay.py:818-1046`) → `make_optcom` (`combine.py:105-236`).
- Data arrays are voxels × echoes × time.

| Kernel | file:line | Algorithm | Size and cost |
|---|---|---|---|
| Log-linear fit | `decay.py:318-416` | per adaptive-mask echo count: `np.linalg.lstsq` with a (E·T × 2) design matrix and all voxels as right-hand sides on log(\|x\|+1) | vectorised; cheap |
| **Curve fit (fMRIPrep default)** | `decay.py:86-120, 123-315` | `scipy.optimize.curve_fit` **per voxel** (bounded, so TRF with a 2-point numerical Jacobian), started from the log-linear estimate; bounds s0 ∈ [min(data), ∞), T2* ∈ [0, ∞); `joblib.Parallel(n_jobs)` with tqdm; runs once per echo count | ~10⁵–3·10⁵ in-mask voxels, each with E·T points (e.g. 3×300 = 900). **This is the biggest tedana hot loop.** |
| Adaptive mask | `utils.py:129-192` | NaN/≤0 check; 33rd percentile (method "higher") of mean echo 1; threshold = exemplar/3; optional "decay" rule | cheap |
| T2* floor and cap | `decay.py:20-64, 640-701` | exp underflow floor; 99.5th percentile cap (`interpolation_method='lower'`) | cheap |
| RMSE maps | `decay.py:704-817` | loops over echo counts | moderate |
| Optimal combination | `combine.py:11-60` | α = TE·exp(−TE/T2*), `np.average` over echoes | vectorised |

**Purity and dependencies**
- Pure Python: no compiled extensions and no subprocess calls in the library (`subprocess` appears only in `tests/test_integration.py`).
- Declared dependencies (`tedana/pyproject.toml`): bokeh, mapca, matplotlib, nibabel, nilearn, numpy, pandas, pybtex, robustica, scikit-learn, scipy, seaborn, threadpoolctl, tqdm, plus requests used in `io.py`.
- The t2smap path really needs only numpy, scipy, nibabel, pandas, joblib, tqdm and threadpoolctl (nilearn only as a fallback). But **`tedana/utils.py:11-27` imports bokeh, mapca, matplotlib, nilearn, robustica and sklearn at module level** just to report versions, and `workflows/__init__.py:5-7` imports the full tedana and ica_reclassify workflows. So the `t2smap` entry point loads every heavy dependency.

---

## D) Numerical kernels worth porting to Rust

Typical sizes:
- BOLD: 64×64×36 (~150k voxels) up to 104×104×72 (~780k), with T = 200–1200 volumes and E = 3–5 echoes.
- MNI152NLin2009cAsym: res-2 is 97×115×97 (1.08M); res-1 is 193×229×193 (8.5M).
- GRE fieldmap: ~64×64×40.
- Knot grid at (16, 16, 10) mm: ~16×19×20, so K ≈ 4–8k coefficients.

| # | Kernel | Location | Input size and frequency | Notes for Rust |
|---|---|---|---|---|
| 1 | Cubic spline prefilter + `map_coordinates` (order 3; modes constant / grid-constant) | `fmriprep/interfaces/resampling.py:305`; `sdcflows/transform.py:99`; `nitransforms/resampling.py:118, 312` | **every BOLD volume** × every output space (native, T1w, MNI) × every echo | Top priority. Fuse affine + VSM + interpolation + Jacobian; prefilter once per volume; must match SciPy boundary semantics |
| 2 | Per-volume affine coordinate map (voxel-to-voxel HMC) | `resampling.py:295`; `nitransforms/base.py:378-384`, `linear.py:282` | 3×N_target per volume | trivial to fuse into #1 |
| 3 | Displacement-field interpolation (3 cubic interpolations per point) | `nitransforms/nonlinear.py:193-205` | N_target points (~1M) on a ~8.5M×3 field; once per run and space | fuse; prefilter each component once |
| 4 | Tensor B-spline collocation matrix (scipy `BSpline` + `lil` + `kron`) | `sdcflows/transform.py:677-766`; `nitransforms/interp/bspline.py:34-97`; `fmriprep/resampling.py:704` | N × K sparse, 64 non-zeros per row (10–70M) | replace with separable 1D weights (4 taps per axis) |
| 5 | B-spline field evaluation (`colmat @ c`) | `transform.py:343`; `bspline.py:252, 278`; `resampling.py:713` | N voxels; once per run and space | separable mode-products, O(12·N) |
| 6 | Sparse ridge least squares | `interfaces/bspline.py:203-216` | N ~0.15–0.7M, K ~4–8k; once per fieldmap | exact Kronecker eigen-solve (single level); CG as fallback |
| 7 | Jacobian `np.gradient` | `resampling.py:316`; `transform.py:115` | N per volume | compute once per run |
| 8 | Fieldmap↔displacement conversion | `transform.py:575-674` | N | trivial |
| 9 | Morphology with ball structuring elements: dilation r = 2/3/5, grey closing r = 1, median filter r = 3, binary opening/closing/erosion r = 3/5, `label` | `utils/tools.py:178-199`; `interfaces/brainmask.py:125`; niworkflows `nibabel.py:706` | 3D reference (~0.2–1M voxels); several per run | `median_filter` with ball(3) (123-voxel footprint) is the slowest |
| 10 | Random-walker segmentation (sparse Laplacian solve) | `utils/tools.py:204` (skimage) | padded 3D reference; per fieldmap or reference | heavy; could keep in Python first |
| 11 | Percentiles, medians, temporal median | `syn.py:729-733`; IntensityClip; RobustAverage (niworkflows `images.py:354`); bspline recenter | 3D or 4D | selection algorithms |
| 12 | Per-voxel monoexponential fit | `tedana/decay.py:111-117, 255-265` | ~10⁵ voxels × E·T samples | Levenberg-Marquardt/TRF with analytic Jacobian, parallel over voxels |
| 13 | Log-linear multi-RHS least squares | `decay.py:396-405` | E·T × 2 by V | closed-form 2×2 |
| 14 | Optimal combination | `combine.py:45-59` | V × E × T | trivial |
| 15 | Surface resampling (KDTree + Python loops) | `nitransforms/surface.py:236-267, 294-320` | ~32k–160k vertices | not on fMRIPrep's path (Workbench is used instead) |

External algorithms that would need re-implementing in Rust: TOPUP, PRELUDE, antsRegistration (SyN with restricted deformation; Rigid/Affine with Mattes), N4, DenoiseImage, ImageMath Laplacian, antsApplyTransforms BSpline and MultiLabel, 3dvolreg, mri_robust_template, mri_robust_register.

---

## E) Test suites and data (inputs to an equivalence harness)

**nitransforms**
- Heavy data: GIN `https://gin.g-node.org/oesteban/nitransforms-tests`, fetched with datalad (+datalad-osf/next) into `TEST_DATA_HOME` (default `~/.nitransforms/testdata`, `conftest.py:10`).
  - Files used: someones_anatomy(+brainmask), someones_displacement_field, someones_bspline_coefficients, func/sbref/bold/fmap, `ds-005_*` affines/warps and `from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5`, tpl-OASIS30ANTs, fsLR/fsaverage surfaces.
- In-repo data (18 MB, `tests/data`): affines in LAS/LPS/RAS/oblique in every format; `regressions/` with fsnative/bold/scanner LTA, FSL and TFM; an FSL warp; `affine-antsComposite.h5`.
- CI: CircleCI Docker image with ANTs 2.6 and fsl-flirt 2111.4 (`env.yml`), FreeSurfer 7.4.1 `mri_vol2vol`, AFNI 25.2.09 (`3dAllineate`, `3dNwarpApply`, `3dWarp`, `3drefit`, `3dvolreg`). GitHub Actions runs on ubuntu only.
- Comparisons against reference tools, all `shell=True` and **skipped when `shutil.which` fails**:

| Test | Reference tool | Criterion |
|---|---|---|
| `test_resampling.py:78-163` | flirt / antsApplyTransforms / 3dAllineate / mri_vol2vol | **nearest-neighbour** resampling; RMSE < 0.09 |
| `test_resampling.py:169-247` | antsApplyTransforms, 3dNwarpApply | mask mismatch < 1e-8; RMSE < 0.09 |
| `test_resampling.py:250-286` | same | RMSE < 0.09 |
| `test_resampling.py:289-330` | antsApplyTransforms on a chain | fraction of \|diff\| > 1e-3 is < 0.09 |
| `test_io_itk.py:395-460, 500-530` | `antsApplyTransformsToPoints` | on-grid: atol 0, rtol 1e-4; off-grid: atol 0.1, rtol 1e-3 |
| `test_io.py:340-401` | `3drefit -deoblique`, `3dWarp` | RMSE < 0.1; affine `allclose` |
| `test_linear.py:171-204` | FSL conventions | rtol 1e-2 |
| `test_nonlinear.py:117-260` | internal B-spline vs dense field | atol 1e-7 to 1e-1 |

- Bottom line: **these tests validate geometry and conventions, not the interpolation kernels.** Pytest config: `--doctest-modules --strict-markers` (`pyproject.toml:66-79`); tox passes `PYTHON_GIL` through.

**sdcflows**
- `TEST_DATA_HOME` defaults to `~/sdcflows-tests` (`conftest.py:39`), plus `TEST_OUTPUT_DIR`, `TEST_WORK_DIR`, and `TEST_PRODUCTION` (sloppy mode unless set).
- Datasets fetched with datalad from `github.com/nipreps-data/*` (`.github/workflows/build-test-publish.yml:81-116`): ds001600 (sub-1), HCP101006, ds001771 (sub-36 + openneuro derivatives), ds000054 (sub-100185 + smriprep-0.6), ds000206 (sub-05), brain-extraction-tests, hcph-pilot_fieldmaps.
- TemplateFlow assets (`tools/cache_templateflow.py`): MNI152NLin2009cAsym res-2 brain mask, res-1 brain probseg, res-2 fMRIPrep boldref.
- `mri_robust_template` is downloaded from OSF (`sx2n7/providers/osfstorage/5e825301d0e35400ebb481f2`). CI also installs AFNI (linux_ubuntu_16_64) and conda `fsl-fugue fsl-topup ants`.
- In-repo data (4.2 MB, `sdcflows/tests/data`): `epi.nii.gz`, `topup-coeff.nii.gz`, `topup-coeff-fixed.nii.gz`, `topup-field.nii.gz`, `field-coeff-tests.nii.gz`, dsA/dsB/dsC BIDS skeletons. `epi2fmap_xfm.txt` and `fmap2epi_xfm.txt` are empty files.
- Markers: `slow` and `veryslow` (`pyproject.toml:141-144`); tox envs fast / slow / veryslow (`tox.ini:79-83`).
- Numeric checks:

| Test | What is compared | Criterion |
|---|---|---|
| `tests/test_transform.py:88-123` | sdcflows vs `antsApplyTransforms -n BSpline` on an ITK displacement field | √Σdiff² / N < 0.1 (loose) |
| `interfaces/tests/test_bspline.py:141-152` | sdcflows evaluation of TOPUP coefficients vs TOPUP's own `out_field` | **RMSE < 3 Hz** — so sdcflows ≠ TOPUP; port sdcflows' semantics, not TOPUP's |
| `test_bspline.py:43-109` | 100 random synthetic fit/interpolate cases | 95% of errors < 25 Hz |
| `test_transform.py:326-343` | `grid_bspline_weights` | **hard-coded golden values** (e.g. w[0,0] = 0.00089725334); good exact regression target |
| `test_transform.py:304-325` | fmap↔displacement round trip | `allclose` |
| `test_transform.py:151-265` | hcph-pilot data | 95th-percentile error < 0.5 (HMC only) or < 200 (fieldmap; the code admits the oracle is bad) |

- The TOPUP, phasediff, SyN, ancillary and registration workflow tests are **smoke tests with no numeric oracle.**

**tedana**
- Integration data is downloaded from OSF (`download_test_data(osf_id)`).
- The t2smap integration test (`tests/test_integration.py:815-845`) only checks which output files exist.

**Harness implication:** none of the suites gives a numeric oracle for interpolation, the B-spline fit or the end-to-end SDC result. Plan to pre-generate golden outputs on Linux (with SciPy, sklearn and the binaries pinned) and ship them to the Windows/macOS runners; otherwise the binary-dependent tests will just be skipped there.

---

## F) POSIX-specific code

| Where | Issue |
|---|---|
| `sdcflows/cli/main.py:100` | `mp.set_start_method('fork')` wrapped in `suppress(RuntimeError)`. On Windows this raises **ValueError**, so the CLI crashes; on macOS fork is unsafe once threads exist. |
| `sdcflows/cli/main.py:135, 143, 162`; `fmriprep/workflows/base.py:718` | `os.EX_SOFTWARE`, `os.EX_USAGE`, `os.EX_DATAERR` **do not exist on Windows** (AttributeError). |
| `fmriprep/config.py:92-104` | `set_start_method('forkserver')` is not available on Windows. |
| `sdcflows/config.py:135, 178`; `utils/misc.py:67` | `OMP_NUM_THREADS` and `os.cpu_count`, psutil (portable); MultiProc plugin via ProcessPoolExecutor (`cli/main.py:92-107`). |
| niworkflows `interfaces/freesurfer.py:80-98` (used by `init_epi_reference_wf`) | `StructuralReference` uses `'echo Only one time point!'` as its command line and still runs FreeSurfer `lta_convert` for a single volume. |
| nitransforms tests (`test_resampling.py:126, 145, 214, 235, 275, 322`; `test_io_itk.py:418, 524`; `test_io.py:357, 395`) and sdcflows `tests/test_transform.py:112-118` | `check_call(..., shell=True)` with POSIX-style command strings; `os.chmod` in `test_io_itk.py:194, 231, 253`; `tempfile.mktemp` in `test_surface.py`. |
| tedana `rica.py:758` | chmod of executable bits (not on the t2smap path). tedana's t2smap path otherwise uses joblib/loky and `platform`, both portable. |
| nitransforms and sdcflows libraries | no `symlink`, `/tmp`, `signal` or `resource` use. Default `asyncio.run` with the thread executor is portable. |
