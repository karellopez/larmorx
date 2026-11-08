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
