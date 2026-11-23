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
for third-party code: none in the functions translated (`RobustGaussian.cpp` came with 3.2-03, below).

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

Queue item 3.2-03's first part (track C, relay 42, 2026-11-22): the registration step's classes, files
of `mri_robust_register/` (SHA-256 prefixes of the v8.2.0 tag's files) `RobustGaussian.cpp` (`40b70bef94f1`), `RobustGaussian.h` (`09e453f76464`), `Regression.cpp` (`cb9a4c74f372`), `Regression.h` (`e137b64606f6`), `Quaternion.h` (`4183a4ddc13e`), `Quaternion.cpp` (`4c1672e48c0b`), `Transformation.h` (`ae6b10ab59df`), `RegistrationStep.h` (`a8efdc4f97d6`); all by Martin Reuter, MGH, under FreeSurfer's
licence. Audited for third-party code: `RobustGaussian.cpp`'s selections follow Numerical Recipes in C
(2nd ed., §8.5) and Wirth (1976) after N. Devillard's public-domain code (credited in its comments);
the translation is of FreeSurfer's modified versions (the positions kept alongside), no other
third-party code.

- `src/robust_gauss.rs`: `RobustGaussian<T>::quick_select`, `quick_selectI`, `kth_smallest`,
  `kth_smallestI`, `median`, `medianI`, `mad`.
- `src/regression.rs`: `Regression<T>` (its constructors, `setVerbose`, `setFloatSvd`, `getRobustEst`,
  `getRobustEstW`, `getRobustEstWAB`, `getRobustEstWB`, `getLSEst`, `getWeightedLSEst`,
  `getWeightedLSEstFloat`, `getSigmaMAD`, `getSqrtTukeyDiaWeights`, `getLast*`); `vnl_svd<T>`'s
  pseudo-inverse as `getLSEst` uses it.
- `src/quaternion.rs`: `Quaternion`'s constructors, `importRotVec` (both), `importZYXAngles`,
  `operator*`, `getNormalized`, `getRotMatrix3d`.
- `src/transformation.rs`: `Transformation` and its fifteen models (`getDOF`, `setParameters` (the three
  kinds), `setIdentity`, `getSteps`, `getMatrix`, `getGradient`). Relay 44: FS-1073, the `double`
  `setParameters` of a longer vector whose block glibc 2.35's allocator hands straight back
  (`set_size` reallocates without keeping the contents): its first two words, the allocator's,
  modelled with `reproduce` (`docs/findings/freesurfer-robust-register.md` §2).
- `src/regstep.rs`: `RegistrationStep<T>` (its constructor from the registration's settings,
  `setFloatSVD`, `computeRegistrationStep`, `constructAb`, the getters); `Registration::Cost`.
- `src/real.rs`: larmorx's own (the two element types, `vnl_matrix<T>`'s product and
  `matrix_x_vector` as ITK 4.13.2's VNL computes them, BSD).

Queue item 3.2-03, part 2 (track C, relay 43, 2026-11-22), of `mri_robust_register/` `Registration.cpp`
(`67d96ed05d0c`), `Registration.h` (`d81a648e1cf7`), `RegRobust.cpp` (`bed4f484dea5`), `RegRobust.h`
(`7fa07ed9fc61`); by Martin Reuter, MGH, under FreeSurfer's licence; no third-party code.

- `src/registration.rs`: `RegRobust` with `Registration`'s fields: the constructors' defaults, `clear`
  (both), the setters of `Registration.h`, `setSampleType`, `setName`, `setTransformation`,
  `setSourceAndTarget`, `reorientSource`, `needReslice`, `makeIsotropic`, `getMinitResampled`,
  `getFinalVox2Vox`, `getHalfWayMaps`, `getCentroidS`, `getCentroidT`, `getCentroidSinT`, `convertP2Md`,
  `initializeTransform`, `getGPLimits`, `buildGPLimits`, `computeIterativeRegistration`,
  `computeIterativeRegistrationFull`, `iterativeRegistrationHelper<T>`, `mapToNewSpace`, `checkSqrtM`,
  `computeMultiresRegistration`, `findSatMultiRes`, `findSaturation`; VNL's fixed 4 x 4 product
  (`vnl_matrix_fixed_mat_mat_mult`, BSD, ITK 4.13.2).

Relay 46 of track C (queue 3.2-13): `src/powell.rs` from `mri_robust_register/RegPowell.cpp`
(`costFunction` 77-246, `computeIterativeRegistrationFull` 252-601, `setTransformation` 603-646, as
`RegRobust` with `powell` set: `new_powell`, `class_name`, `set_transformation`'s second-type models, the
stream's precision `cout_precision`) and `MyMRI::MRInorm255` (`MyMRI.cpp` 42-47); `src/costs.rs` gains
`CostFunctions.cpp`'s `minmax` (47-61), `mean(mri, frame)` (63-123), `leastSquares` with `s1`, `s2`
(408-487), `absDiff` (489-559), `localNCC` (646-768), `mapMRI` (770-830), `NCC` (832-910),
`tukeyBiweight` with the half-way maps (1044-1137) and `rhoTukeyBiweight` (`CostFunctions.h` 180-189);
the one-thread order of their OpenMP sums. `registration.rs`: the level images passed mutably (the
change of type), the multiresolution lines at the stream's precision.

Not yet: `testRobust` and `constructAb2` (`--test`) and the `debug > 0` paths (with `mri_robust_register`,
3.2-13); `JointHisto.cpp` (`MI`, `NMI`, `ECC`, `SCR`), `CostFunctions::SBCost` (`SB`, libstdc++'s
`std::sort`); not reached by either program: `flipInputs`, `init_scaling`, `buildGaussianPyramid`, `setSource`,
`setTarget`, `RegRobust::estimateIScale`; the functions only one program reaches (their items).

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
- **FS-1061** (`regstep`): more than 2,147,483,647 voxels and frames: computed as FreeSurfer (its
  32-bit row numbers), noted. **FS-1062** (`regstep`, rule D): the subsampled step's gradient positions
  kept, noted. **FS-1063** (`transformation::set_parameters`): a `double` vector of another length: its
  first `DOF` numbers in both modes (FreeSurfer's are memory contents). **FS-1064** (`robust_gauss`): the
  median of an empty list NaN in both modes. **FS-1065** (`regression`, the location): the weights sized
  to `b` in both modes (FreeSurfer crashes). **FS-1066** (`regression::get_ls_est` without `A`): the mean
  in both modes. **FS-1067** (`regression`): fewer rows than columns: an error by default; `reproduce`
  ends too (the robust and weighted solves read memory; the least squares is not ported yet).
  **ITK-113** (VNL's QR of one zero row): 0, noted.
- `sin`, `cos` (the quaternions) and `exp` (the weights' Gaussian): correctly rounded by default,
  glibc's with `reproduce`, GLIBC-07 noted where they differ.
- `get_rand` with a negative position (FreeSurfer reads before its table; no caller passes one): a panic
  in the port.
- Speed: `getPartials`, `getBlur` are 3.9x FreeSurfer's time at 256^3 (fs.filter's `MRIconvolve1d`),
  the others 1.3-2x; memory within 3 % but `getPartials` (+15 %) and `getBlur` (+63 %), both from
  `MRIconvolve1d`'s output buffer (`docs/findings/freesurfer-robust.md` §5).
- **FS-1068** (`registration::get_half_way_maps`): the half-way maps before any iteration (FreeSurfer reads
  empty matrices): an error in both modes, noted. **FS-1069** (`regstep::construct_ab`): a step of two
  images of different extents, frames or types (FreeSurfer reads past one): an error in both modes, noted.
- A debug level above 0 (`--debug`): `Registration: the debug output (--debug) is not ported yet` and
  status 1 (its writes and key waits come with 3.2-13).
- `src/entropy.rs` (queue 3.2-13, relay 45 of track C): `mri_robust_register/MyMRI.cpp`
  `MyMRI::entropyImage(MRI*, int, bool, bool, MRI*)` (1177-1569) and `MyMRI::get3Dcorrection`
  (1571-1631), FreeSurfer 8.2.0. Changes: FS-1074 (rule B: a negative or overflowing radius an error by
  default; `reproduce` the abort or the 2-D zeros); the OpenMP slices on the port's threads (each voxel
  computed alone); `log` and `exp` through `larmorx_core::math::Libm` (correctly rounded by default,
  glibc 2.35's with `reproduce`).
