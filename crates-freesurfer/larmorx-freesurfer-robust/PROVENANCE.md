# Provenance: larmorx-freesurfer-robust

**Replica: translated from FreeSurfer's source.** This crate holds `fs.robust` (wave 3.2) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): Reuter's robust registration library (`mri_robust_register`'s shared files): the robust and Powell registrations, the cost functions, its image and matrix helpers, the multi-image registration and quaternions. It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

Queue item 3.2-02 (`fs.robust` 1 of 2; track C, relay 40, 2026-11-18): the helpers both
`mri_robust_register` and `mri_robust_template` reach (read by hand, the class templates' calls
included: `docs/findings/freesurfer-robust.md` §1). Files of `mri_robust_register/` (SHA-256 prefixes of
the v8.2.0 tag's files): `MyMRI.cpp` (`f8e956398418`), `MyMRI.h` (`f1125a5d1338`), `MyMatrix.cpp`
(`efbe9fc64c5e`), `MyMatrix.h` (`b06f981dc03d`), `CostFunctions.cpp` (`2cd72bb84df8`),
`CostFunctions.h` (`092656c7682f`); all by Martin Reuter, MGH, under FreeSurfer's licence. Audited
for third-party code: none in the functions translated (`RobustGaussian.cpp`'s quick select, after
Numerical Recipes and N. Devillard's public-domain code, is not translated here: its only callers have
no live caller).

- `src/mymri.rs`: `MyMRI::MRIvalscale`, `getPrefilter`, `getDerfilter`, `getRand` (its table),
  `subSample`, `getBlur`, `getPartials`, `findRightSize`, `getBackground`, `MRIlinearTransform`.
- `src/costs.rs`: `CostFunctions::centroid`, `orientation`.
- `src/mymatrix.rs`: `MyMatrix::convertVNL2MATRIX` (matrix and vector), `convertMATRIX2VNL`,
  `getVNLMatrix`, `isDiag`, `PolarDecomposition`, `Polar2Decomposition`, `AffineTransDistSq`,
  `RigidTransDistSq`, `getFrobeniusDiff`, `RotMatrixLogNorm` (the `vnl_matrix` overloads), and
  `vnl_matrix_inverse<double>`'s pseudo-inverse as they use it.
- VNL's arithmetic (ITK 4.13.2, BSD) is the main package's: `larmorx_core::vnl_matrix` (new in this
  item), `vnl_svd` (its double SVD gained ITK-116's stop), `vnl_eigen`, `vnl_fixed`.

Queue item 3.2-02's second part (track C, relay 41, 2026-11-20): `MyMatrix::SchurComplex` and
`MatrixSqrt` (`MyMatrix.cpp`, the same file) in `src/mymatrix.rs`: LAPACK 3.10.0's `zgees_` as Ubuntu
22.04's static library computes it (the official binary holds it: the findings' §2) is the main
package's `larmorx_core::lapack_zgees`, C++'s complex arithmetic `larmorx_core::cplx` (`__divdc3`,
`__muldc3`, `cabs` and `csqrt` on the correctly rounded `hypot`, the project's decision D4), VNL's complex
matrices and `vnl_matlab_print` `larmorx_core::vnl_matrix` (the findings' §4).

Not yet: `RegRobust.cpp`/`.h` and `Registration.cpp` 77-678 (`clear`, `computeIterativeRegistration`,
`computeMultiresRegistration`), which call item 3.2-03's templates and are ported with them; the
functions only one program reaches (its item).

## Changes from the originals

- **FS-966** (`costs::orientation`): no eigenvalue above another: the eigensystem's own column order by
  default; `reproduce` reads FreeSurfer's fourth column (two of its numbers, 0 for the one past the
  matrix).
- **FS-967** (`costs::orientation`): an image with an infinite or NaN voxel: an error and status 1 by
  default; `reproduce` FreeSurfer's result where VNL returns with eigenvectors.
- **FS-968** (`mymatrix::rigid_trans_dist_sq` through `matrix_inverse`): an SVD that did not converge:
  an error and status 1 by default; `reproduce` the distance from the decomposition reached (its NaNs'
  bits are not yet the official build's: the double SVD's operand orders are an open item).
- **FS-969** (`mymatrix::schur_complex`, `matrix_sqrt`): a matrix with an infinite or NaN element: an
  error and status 1 by default; `reproduce` computes as FreeSurfer does where LAPACK's `zlahqr`
  converges (`zlaqr0`, its retry where it does not, is not ported: an error).
- **FS-970** (the same): an empty matrix: an error and status 1 by default; `reproduce` LAPACK's
  `XERBLA` line on standard output and status 0, as FreeSurfer.
- `cabs` and `csqrt` take the correctly rounded `hypot` in both modes (the project's decision D4; glibc's
  `hypot` misses the last bit on about 0.5 % of arguments, GLIBC-07); whether glibc's would change a
  recorded output is measured at `mri_robust_register`'s and `mri_robust_template`'s replays (3.2-13 to
  3.2-16).
- **ITK-115**, **ITK-116** (VNL's eigensystem and SVD never returning, where FreeSurfer hangs): an error
  in both modes.
- `get_rand` with a negative position (FreeSurfer reads before its table; no caller passes one): a panic
  in the port.
- Speed: `getPartials`, `getBlur` are 3.9x FreeSurfer's time at 256^3 (fs.filter's `MRIconvolve1d`),
  the others 1.3-2x; memory within 3 % but `getPartials` (+15 %) and `getBlur` (+63 %), both from
  `MRIconvolve1d`'s output buffer (`docs/findings/freesurfer-robust.md` §5).
