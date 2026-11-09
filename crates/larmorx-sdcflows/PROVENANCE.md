# Provenance: larmorx-sdcflows

**Ported** (an exact port of a permissive upstream, in the main package: `docs/licensing.md`,
`docs/porting-guide.md` §1) from sdcflows **2.17.1** (PyPI sdist, SHA-256
`0365de256edd71481ab49497ee6c8c6688f4a99f4eb11819d4770e95630e5614`; Apache-2.0, Copyright The
NiPreps Developers), the newest release. fMRIPrep 25.2.6 runs sdcflows 2.15.1, whose files on
these paths are byte-identical (`interfaces/bspline.py`, `interfaces/fmap.py`, `transform.py`,
`utils/phasemanip.py`, `utils/tools.py`, `workflows/fit/fieldmap.py`).

The arithmetic of the libraries sdcflows calls is followed operation by operation, because it
decides bit-identity: SciPy 1.18.1 (BSD-3-Clause; identical on these paths in 1.15.3 and
1.15.2), scikit-learn 1.9.1 (BSD-3-Clause; identical on these paths in 1.6.1), numpy 2.5.4
(BSD-3-Clause), OpenBLAS 0.3.34 (BSD-3-Clause; identical on these paths in 0.3.30). The logic is
re-expressed in Rust; no code was copied. The notices are in the modules that follow each
upstream.

| larmorx | Upstream | Licence |
|---|---|---|
| `bspline::bspline_grid`, `DEFAULT_*_ZOOMS_MM` | sdcflows `sdcflows/interfaces/bspline.py` (`bspline_grid`, the constants); nibabel 5.4.2 `nibabel/affines.py` (`voxel_sizes`, `apply_affine`) | Apache-2.0, MIT |
| `bspline::grid_bspline_weights`, `AxisWeights`, `LevelWeights`, `NOT_ALIGNED` | sdcflows `sdcflows/transform.py` (`grid_bspline_weights`); SciPy `scipy/interpolate/_bsplines.py` (`BSpline.__call__`), `scipy/interpolate/src/__fitpack.cc` (`_find_interval`, `_deBoor_D`, `_evaluate_spline`), `scipy/sparse/_lil.py` and `_csparsetools.pyx` (`lil_insert`: exact zeros not stored) | Apache-2.0, BSD-3-Clause |
| `bspline::SampleWeights`, `ExactLevel`, `extended_interval` (the corrections of SDC-01 and SDC-02) | larmorx's own: the same recursion (`_deBoor_D`) on sdcflows' uniform knots continued beyond the grid; every sample's weights at its own coordinates (nibabel's `apply_affine`, sdcflows' rounding and product order); written from the register's description, checked against sdcflows' construction with the knots continued and an independent numpy construction (`validation/investigations/sdcflows-fieldmap/measure_sdc01_02.py`) | Apache-2.0 |
| `bspline::Collocation` (`matvec_f32`, `rmatvec_f32`, `to_csr`), the dense-axis rule | SciPy `scipy/sparse/_construct.py` (`kron`: BSR with a dense right operand when `2 * nnz >= rows * cols`, else COO; `hstack`), `_coo.py` / `_compressed.py` (`tocsr`, `sum_duplicates`: sorted, zeros kept), `sparsetools/csr.h` (`csr_matvec`, `csr_matvecs`), `sparsetools/csc.h` (`csc_matvec`), `sparse/linalg/_interface.py` (`MatrixLinearOperator`, `_AdjointMatrixOperator`) | BSD-3-Clause |
| `bspline::StoredCollocation`, `Collocation::store`, `scipy_upcasts`, `matvec_f64_upcast`, `LevelWeights::is_empty`, `sdc02_changes` | SciPy `scipy/sparse/_construct.py` (`kron`'s early return: an empty float64 `coo` when an operand stores nothing; `hstack`'s upcast), `sparsetools/csr.h` (`csr_matvec` in the upcast type), `sparsetools/csc.h` (`csc_matvec` on `X.T`: the stored transpose's order); the stored matrix's layout and the SDC-02 test are larmorx's own | BSD-3-Clause |
| `blas::sdot`, `dot_f32`, `norm_f32` | OpenBLAS `interface/dot.c`, `kernel/x86_64/sdot.c`, `kernel/x86_64/sdot_microk_haswell-2.c` (`KERNEL.HASWELL`, `KERNEL.ZEN`); numpy `numpy/_core/src/multiarray/arraytypes.c.src` (`FLOAT_dot`), `numpy/_core/src/common/cblasfuncs.c` (`cblas_matrixproduct`, `_select_matrix_shape`: a one-element vector is a scalar), `numpy/linalg/_linalg.py` (`norm`) | BSD-3-Clause |
| `ridge::cg_f32`, `ridge::Operator`, `CsrOperator` | SciPy `scipy/sparse/linalg/_isolve/iterative.py` (`cg`, `_get_atol_rtol`), `_isolve/utils.py` (`make_system`, `IdentityOperator`), `sparse/linalg/_interface.py` (`aslinearoperator`, `MatrixLinearOperator`, `_AdjointMatrixOperator`), `sparsetools/csr.h` (`csr_matvec`), `sparsetools/csc.h` (`csc_matvec`) | BSD-3-Clause |
| `ridge::ridge_sparse_cg`, `RidgeFit`, `RidgeSystem` | scikit-learn `sklearn/linear_model/_ridge.py` (`Ridge.fit`, `_BaseRidge.fit`, `_ridge_regression`, `resolve_solver_for_numpy`, `_solve_sparse_cg`), `sklearn/linear_model/_base.py` (`_preprocess_data`), `sklearn/utils/fixes.py` (`_sparse_linalg_cg`), `sklearn/utils/_param_validation.py` and `validation.py` (the messages) | BSD-3-Clause |
| `approx::bspline_approx`, `Recenter`, `median_f32`, `mean_f32`, `mode_f32` | sdcflows `sdcflows/interfaces/bspline.py` (`BSplineApprox._run_interface`); numpy `numpy/lib/_function_base_impl.py` (`median`, `_median`), `numpy/_core/_methods.py` (`_mean`, `_count_reduce_items`), `numpy/_core/src/umath/loops_utils.h.src` (`FLOAT_pairwise_sum`), `loops_arithm_fp.dispatch.c.src` (the reduction); SciPy `scipy/stats/_stats_py.py` (`mode`) | Apache-2.0, BSD-3-Clause |
| `approx::bspline_approx_logged` | the same; the records sdcflows writes before it raises | Apache-2.0 |
| `tools::rescale_affine`, `voxel_sizes`, `resample_to_zooms_grid`, `apply_affine` | sdcflows `sdcflows/utils/tools.py` (`resample_to_zooms`); nibabel 5.4.2 `nibabel/affines.py` (`rescale_affine`, `voxel_sizes`, `apply_affine`) | Apache-2.0, MIT |
| `tools::affine_coordinates`, `identity_coordinates` | nitransforms 25.1.0 `nitransforms/resampling.py` (`apply`), `base.py` (`ImageGrid.ndcoords`, `index`, `_as_homogeneous`, `_apply_affine`), `linear.py` (`Affine.map`) | MIT |
| `unwarp::unwarp_volume`, `ReadoutTime`, `UnwarpOptions` | sdcflows `sdcflows/transform.py` (`_sdc_unwarp`, `unwarp_parallel`, `B0FieldTransform.apply`'s per-volume work); numpy `numpy/lib/_function_base_impl.py` (`gradient`: unit spacing, `edge_order=1`), NEP 50's scalar rules (a Python float readout time keeps float32, a numpy float64 one gives float64); SciPy's `map_coordinates` through `larmorx-interp` (its provenance there) | Apache-2.0, BSD-3-Clause |
| `npy::py_repr`, `py_format_2e` | CPython's float `repr` ("short" style) and `format(x, ".2e")`, as the original messages print numbers (behaviour only) | (no code) |
| `npy::floor_divide`, `allclose_zero`, `cross`, `norm3`, `grids_aligned` | numpy `numpy/_core/src/npymath/npy_math_internal.h.src` (`npy_divmod`, `npy_floor_divide`), `numpy/_core/numeric.py` (`cross`, `isclose`, `allclose`), `numpy/linalg/_linalg.py` (`norm`) | BSD-3-Clause |

The 4×4 products and inverses and nibabel's `apply_affine` come from `larmorx-transform`
(`openblas::matmul4`, `inv4`, `resample_series::apply_affine`, `apply_affine_single`: numpy on
OpenBLAS's Haswell kernels; their provenance is in that crate's `PROVENANCE.md`).

The Python layer (`python/larmorx/sdcflows/`) ports the rest of each interface with numpy's own
elementwise arithmetic and the bindings above: `approx.py` (`BSplineApprox`'s loading, masks,
resize, files, headers and log records; nipype's `split_filename` and `fname_presuffix`),
`tools.py` (`resample_to_zooms`), `coeffs.py` (`TOPUPCoeffReorient` / `_fix_topup_fieldcoeff`,
`TransformCoefficients` / `_move_coeff`, `reorient_pedir`; nibabel's `as_reoriented`,
`set_intent`), `transform.py` (`B0FieldTransform.fit`, `deoblique_and_zooms`,
`ensure_positive_cosines`; nibabel's `strided_scalar`; `B0FieldTransform.apply` and
`to_displacements`, `ApplyCoeffsField`; niworkflows 1.15.3's `reorient_image` (Apache-2.0),
nibabel's `axcodes2ornt`; nitransforms' `Affine.from_filename(fmt='itk')` and `~Affine`),
`fmap.py` (`fmap_to_disp`, `disp_to_fmap`, `DisplacementsField2Fieldmap`; nibabel's
`apply_affine`, `set_intent`), `phase.py` (`utils/phasemanip.py`: `au2rads`,
`subtract_phases`, `phdiff2fmap`, `delta_te`; `interfaces/fmap.py`: `PhaseMap2rads`,
`SubtractPhases`, `Phasediff2Fieldmap`, `CheckB0Units`, `CheckRegister` with
`_check_gross_geometry` and `_conform_img`; `workflows/fit/fieldmap.py`: `init_phdiff_wf`, with
nipype's `PRELUDE` call through `lx.fsl.prelude`) (its files written through `lx.io.save_nibabel`, nibabel's scaled writer: its provenance in
`crates/larmorx-io/PROVENANCE.md`), from sdcflows 2.17.1 (Apache-2.0), nibabel 5.4.2 and nitransforms 25.1.0
(MIT) and nipype 1.12.0's `utils/filemanip.py` and `interfaces/fsl/epi.py` (Apache-2.0).

The D13 switches of this port (`upstream_defects`; `docs/findings/sdcflows-fieldmap.md`):
SDC-01 (weights beyond the control grid: the cubic B-spline's; measured,
`measure_sdc01_02.py`), SDC-02 (a rotated sampling grid: the weights at every voxel's position;
measured), SDC-04 (`au2rads` of a constant phase map), SDC-05 (an integer or float32 readout
time in `apply`), SDC-06 (a 4D field map of one volume: its first three zooms), SDC-07
(`TOPUPCoeffReorient`'s negation for a BIDS `i` direction, measured: `measure_sdc_g.py`),
SDC-08 (`apply` on a positive-axes series read from a file), SDC-09 (`SubtractPhases` without
`EchoTime`), SDC-10 (`B0FieldTransform.fit`'s approximation built and sampled where the
transform puts the target; measured), SDC-11 (`resample_to_zooms` onto a grid of one voxel:
nitransforms' bare assertion; found by relay 7's random-testing trial). SDC-03 (`CheckRegister`'s signed translation test) waits
for its FreeSurfer branch.

Not ported (private, used by no workflow): `_split_itk_file`, `_b0_resampler`; `init_fmap_wf`
needs part B's magnitude preparation and masks.

## Part B: `brain_masker` (`src/masker.rs`)

**Ported** from sdcflows 2.17.1 `sdcflows/utils/tools.py` (`brain_masker`; identical in 2.15.1),
on `larmorx-ndimage` (SciPy's `grey_closing`, `median_filter`, `binary_*`, `label`, `sum`) and
its `skimage` module (`ball`, `threshold_otsu`, `random_walker`), and numpy's `percentile` and
`std` (`larmorx_core::numpy`). The Python wrapper `python/larmorx/sdcflows/masks.py` ports
`brain_masker`'s file names and writes, `interfaces/brainmask.py`'s `BrainExtraction`,
`BinaryDilation` (`_dilate`) and `Union` (`_union`). Bit-identical to sdcflows' own
`brain_masker` in both kept environments and to fMRIPrep 25.2.6's own `masker` files (numpy
2.2's sums, SciPy 1.15's order) on the nine `brainextraction_wf` nodes of the oracle runs
(`validation/investigations/sdcflows-fieldmap-b/check_skimage.py`). Its switches: the rough mask
without a component (SDC-h) and no brain seed after the erosion: errors by default,
`upstream_defects='reproduce'` gives sdcflows' results (IDs in the part's defects relay).


## Part B: the PEPOLAR estimation (`python/larmorx/sdcflows/pepolar.py`, relay 5)

**Ported** in Python (no new Rust; numpy's add-reduce order is `larmorx_core::numpy::reduce`,
bound as `_core.numpy_add_reduce`) from sdcflows 2.17.1 (Apache-2.0): `utils/epimanip.py`
(`get_trt`), `interfaces/epi.py` (`GetReadoutTime`, `SelectPEVolumes`, `SortPEBlips`),
`interfaces/utils.py` (`UniformGrid`, `ReorientImageAndMetadata`, `PadSlices` /
`_pad_num_slices`), `workflows/fit/pepolar.py` (`init_topup_wf`, `_select_topup_config`,
`_topup_mats_to_world`), and, for `wiring="2.15"`, sdcflows 2.15.1's `workflows/fit/pepolar.py`
(identical in 2.15.2); nipype 1.12.0's `interfaces/fsl/epi.py` (`TOPUP`: the command line,
`_generate_encfile`, `_list_outputs`; Apache-2.0); nitransforms 25.1.0's
`resampling.apply` (one affine, a 3D image: `tools::affine_coordinates` above) and `io/fsl.py`
(`FSLLinearTransform.to_ras`, `_fsl_aff_adapt`; MIT). sdcflows' own topup configurations
(`python/larmorx/sdcflows/data/flirtsch/b02b0.cnf`, `b02b0_quick.cnf`, `b02b0_{1,2,4}_quick.cnf`)
are copied unchanged from sdcflows 2.17.1's `sdcflows/data/flirtsch/` (Apache-2.0); FSL's
`b02b0_{1,2,4}.cnf` (FSL Licence) are not shipped (D9). niworkflows' `RobustAverage`,
`MergeSeries` and `ReorientImage` (with nibabel 5.4.2's `axcodes2ornt`, `aff2axcodes`,
`ornt_transform`, `as_reoriented`, `squeeze_image`, `four_to_three`, `concat_images`; MIT) are in
`python/larmorx/niworkflows/` (niworkflows 1.15.4, Apache-2.0; the same in 1.15.2). Switch:
SDC-12 (`ReorientImageAndMetadata` reorients to the upper-cased code it checked).

## Part B: nipype's command lines, the field map's registration and the BOLD skull-stripping (relay 6)

**Ported** in Python (no new Rust): nipype 1.12.0 (Apache-2.0) `interfaces/base/core.py`
(`CommandLine._parse_inputs`, `_format_arg`, `_filename_from_source`, `_list_outputs`),
`utils/filemanip.py` (`split_filename`, `fname_presuffix`, `ensure_list`),
`interfaces/ants/registration.py` (`Registration`), `interfaces/ants/resampling.py`
(`ApplyTransforms`), `interfaces/ants/segmentation.py` (`N4BiasFieldCorrection`, `DenoiseImage`),
`interfaces/ants/utils.py` (`ImageMath`), `interfaces/ants/base.py` (`ANTSCommand`'s `NSLOTS`,
`_format_xarray`), `interfaces/fsl/preprocess.py` (`BET`), `interfaces/fsl/maths.py`
(`MultiImageMaths`, `BinaryMaths`), `interfaces/fsl/epi.py` (`TOPUP`), `interfaces/fsl/base.py`
(`_gen_fname`), `interfaces/afni/preprocess.py` (`Automask`, `Volreg`), `interfaces/afni/base.py`
(`_overload_extension`, `_list_outputs`), and niworkflows 1.15.4's `interfaces/fixes.py`
(`FixHeaderRegistration`, `FixHeaderApplyTransforms`), into `python/larmorx/_nipype.py`;
niworkflows 1.15.4 (Apache-2.0; the same in 1.15.2) `utils/images.py` (`_copyxform`),
`interfaces/header.py` (`CopyHeader`), `interfaces/nibabel.py` (`ApplyMask`),
`interfaces/utility.py` (`KeySelect`) and `func/util.py` (`init_skullstrip_bold_wf`) into
`python/larmorx/niworkflows/header.py`; sdcflows 2.17.1 (Apache-2.0; the same in 2.15.1)
`workflows/apply/registration.py` (`init_coeff2epi_wf`) into
`python/larmorx/sdcflows/registration.py`. sdcflows' ANTs settings files
(`python/larmorx/sdcflows/data/fmap-any_registration.json`, `fmap-any_registration_testing.json`,
`affine.json`, `sd_syn.json`, `sd_syn_sloppy.json`, `translation_rigid.json`) are copied
unchanged from sdcflows 2.17.1's `sdcflows/data/` (Apache-2.0; identical in 2.15.1).

## Part B: the fieldmap-less (SyN) preprocessing (`python/larmorx/sdcflows/syn.py`, relay 9)

sdcflows 2.17.1 (Apache-2.0; the same in 2.15.1) `workflows/fit/syn.py` (`init_syn_preprocessing_wf`,
`_set_dtype`, `_merge_meta`) and `interfaces/utils.py` (`Deoblique`, `Reoblique`: `_deoblique`,
`_reoblique`, `OBLIQUE_THRESHOLD_DEG`); sdcflows' `data/fmap_atlas_2_MNI152NLin2009cAsym_affine.mat`
(Apache-2.0, shipped as `python/larmorx/sdcflows/data/`; SHA-256
`588820bdaefab4f98c42b3a394c7c975ad9764bc5d8b095718f47419a9ea6ffc`); `data/fmap_atlas.nii.gz` is
fetched, not shipped (SHA-256 `72b8d2cc7ff3324f3bfaf52712bc191256e795bcff5ee168563b89e584aa40bb`, from
`https://raw.githubusercontent.com/nipreps/sdcflows/2.17.1/sdcflows/data/fmap_atlas.nii.gz`); sdcflows'
`DenoiseImage` (`interfaces/utils.py`) with nipype 1.12.0's `CopyHeaderInterface` and
`utils/imagemanip.copy_header` (Apache-2.0); nibabel 5.4.2's `affines.obliquity` (MIT). Into
`python/larmorx/niworkflows/`: niworkflows 1.15.4 (Apache-2.0; the same in 1.15.2)
`interfaces/header.py` (`ValidateImage`), `interfaces/bold.py` (`NonsteadyStatesDetector`),
`interfaces/nibabel.py` (`Binarize`, `GenerateSamplingReference`, `_gen_reference`),
`interfaces/freesurfer.py` (`StructuralReference`'s one-file shortcut), `workflows/epi/refmap.py`
(`init_epi_reference_wf`, `_post_merge`); nipype 1.12.0 `algorithms/confounds.py` (`is_outlier`);
nilearn 0.14.1 and 0.11.1 (BSD-3-Clause; tags `nilearn-0.14.1`, `nilearn-0.11.1`) `image/resampling.py`
(`resample_img`, `get_bounds`, `to_matrix_vector`, `_resampling_not_needed`,
`_check_resample_img_inputs`, `_resample_one_img`) and `image/image.py` (`new_img_like`, `crop_img`,
`_downcast_from_int64_if_possible`); transforms3d 0.4.2 (BSD-2-Clause) `affines.decompose44` and
`axangles.mat2axangle` (`ValidateImage`'s report). nibabel 5.4.2's `ArrayWriter(check_scaling=False)`
path into `python/larmorx/io/nibabel_writer.py` (`save_nibabel(scale=False)`).

## Part B: the fieldmap-less (SyN) estimation (`python/larmorx/sdcflows/syn.py`, relay 10)

sdcflows 2.17.1 (Apache-2.0; the same in 2.15.1) `workflows/fit/syn.py` (`init_syn_sdc_wf`,
`INPUT_FIELDS`, `MAX_LAPLACIAN_WEIGHT`, `_warp_dir`, `_mm2vox`, `_adjust_zooms`, `_norm_lap`) and
`utils/misc.py` (`front`, `last`); nipype 1.12.0's `interfaces/ants/utils.py` (`ImageMath`, its
`copy_header` through `interfaces/mixins/fixheader.py`'s `CopyHeaderInterface`; Apache-2.0);
nibabel 5.4.2's `orientations.aff2axcodes` with sdcflows' unsigned labels and `affines.voxel_sizes`
(MIT). The nodes run through larmorx's earlier ports (`lx.niworkflows.intensity_clip`, sdcflows'
`BinaryDilation`, `Union`, `GetReadoutTime`, `DisplacementsField2Fieldmap`, `BSplineApprox`,
`ApplyCoeffsField`; ANTs' `ImageMath`, `antsRegistration`, `antsApplyTransforms`).

## Part B: the estimators of a BIDS dataset (relay 11)

Into `python/larmorx/bids/` (`layout.py`, Apache-2.0 AND MIT): pybids 0.22.0 (MIT, Copyright (c)
2016-2026 The PyBIDS Developers; tag `pybids-0.22.0`, sources under `src/bids`)
`layout/layout.py` (`BIDSLayout`: `get`, `get_file`, `get_metadata`, `get_entities`,
`parse_file_entities`, `add_derivatives`, `__getattr__`, `_in_scope`, `_get_layouts_in_scope`,
`_sanitize_query_dtypes`, `_build_file_query`; `Query`), `layout/index.py` (`BIDSLayoutIndexer`,
`_regexfy`, `_check_path_matches_patterns`, `_validate_path`, `_index_dir`, `_index_file`,
`_index_metadata`, `_resolve_intent`), `layout/models.py` (`Config`, `Entity`, `Tag`,
`_create_tag_dict`, `BIDSFile`, `BIDSDataFile`, `BIDSImageFile`, `BIDSJSONFile`),
`layout/validation.py` (`validate_root`, `validate_derivative_path`, `validate_indexing_args`,
`_sort_patterns`, `DEFAULT_LOCATIONS_TO_IGNORE`, `ALWAYS_IGNORE`), `layout/utils.py`
(`BIDSMetadata`, `PaddedInt`, `parse_file_entities`), `layout/db.py` (the `regexp` function),
`utils.py` (`listify`, `hashablefy`, `natural_sort`, `make_bidsfile`, `bids_sort` with
bidsschematools 2.0.0's `rules.entities` order), `exceptions.py`; pybids 0.20.0 (tag `pybids-0.20.0`, sdist SHA-256
`a68b577426942e50e6ac37e011463361d00a378d19b75df817619450357ebbb6`, the same files as fMRIPrep 25.2.6's image) for the step (`get_entities` unsorted, `index.py`'s
`IntendedFor` join). Shipped unchanged:
`python/larmorx/bids/config/bids.json` and `derivatives.json` (pybids 0.22.0, MIT; SHA-256
`bc8d7ef6b9b5f922a4b435c66598b068d03ce2afdd1a826d2adca4dca4542ad0`,
`9b05f472ba5ca236496abe1cb46b204f94cc54bed572bbd5fcc338d97b77314d`) and `nipreps.json` (niworkflows
1.15.4, Apache-2.0, the same in 1.15.2; `40935adf9e3af887150874ab7481a3ed639946ae695f3b632592a630c788fc4d`).

Into `python/larmorx/sdcflows/estimators.py` and `wrangler.py`: sdcflows 2.17.1 (Apache-2.0)
`fieldmaps.py` (`MetadataError`, `EstimatorType`, `MODALITIES`, `FieldmapFile`,
`FieldmapEstimation`, `get_identifier`, `clear_registry`; 2.15.1/2.15.2's path check as
`sdcflows_version="2.15"`), `utils/bimap.py` (`bidict`, `EstimatorRegistry`; the same in 2.15.x),
`utils/misc.py` (`create_logger`), `utils/wrangler.py` (`find_estimators`,
`find_anatomical_estimators`, `_normalize_intent`, `_resolve_intent`, `_filter_metadata`,
`_log_debug_estimation`, `_log_debug_estimator_fail`; = 2.15.1's) and sdcflows 2.15.2's
`utils/wrangler.py` (tag `sdcflows-2.15.2`: the default search, SDC-17); niworkflows 1.15.4
`utils/bids.py` (`relative_to_root`).

Into `python/larmorx/pipelines/larmorprepx/estimators.py`: fMRIPrep 25.2.7 (Apache-2.0; the same
on these paths in 25.2.6) `config.py` (`execution.init`'s ignore patterns), `workflows/base.py`
(`map_fieldmap_estimation`, `get_estimator`, `init_single_subject_wf`'s precomputed field maps,
"Setting up fieldmap" and PEPOLAR checks), `utils/bids.py` (`collect_fieldmaps`, `_get_layout`)
and `data/fmap_spec.json` (its queries, as `FMAP_SPEC`).
