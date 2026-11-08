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
| `bspline::Collocation` (`matvec_f32`, `rmatvec_f32`, `to_csr`), the dense-axis rule | SciPy `scipy/sparse/_construct.py` (`kron`: BSR with a dense right operand when `2 * nnz >= rows * cols`, else COO; `hstack`), `_coo.py` / `_compressed.py` (`tocsr`, `sum_duplicates`: sorted, zeros kept), `sparsetools/csr.h` (`csr_matvec`, `csr_matvecs`), `sparsetools/csc.h` (`csc_matvec`), `sparse/linalg/_interface.py` (`MatrixLinearOperator`, `_AdjointMatrixOperator`) | BSD-3-Clause |
| `npy::floor_divide`, `allclose_zero`, `cross`, `norm3`, `grids_aligned` | numpy `numpy/_core/src/npymath/npy_math_internal.h.src` (`npy_divmod`, `npy_floor_divide`), `numpy/_core/numeric.py` (`cross`, `isclose`, `allclose`), `numpy/linalg/_linalg.py` (`norm`) | BSD-3-Clause |

The 4×4 products and inverses and nibabel's `apply_affine` come from `larmorx-transform`
(`openblas::matmul4`, `inv4`, `resample_series::apply_affine`, `apply_affine_single`: numpy on
OpenBLAS's Haswell kernels; their provenance is in that crate's `PROVENANCE.md`).
