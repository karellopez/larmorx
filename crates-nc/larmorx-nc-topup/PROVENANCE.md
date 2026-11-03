# Provenance: larmorx-nc-topup

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.23's `topup` (the estimation of the susceptibility-induced off-resonance field from images
acquired with reversed phase-encoding directions, Jesper Andersson), written to give FSL's output
bit for bit. It is distributed under the FSL Licence's terms (`LicenseRef-FSL`: non-commercial
use only, `../LICENSE`), as part of the `larmorx-nc` package (`docs/licensing.md`). The parts of
FSL's libraries it relies on (newimage's volumes, interpolation and smoothing; NewNifti; utils'
option parser and configuration files; basisfield's spline fields; warpfns' resampling and
Jacobians; miscmaths' sparse matrices, Levenberg-Marquardt and scaled conjugate gradients;
OpenBLAS's and LAPACK's kernels as FSL's build runs them) are the shared core's,
`../larmorx-nc-core` (its `PROVENANCE.md`). The Apache-2.0 clean-room original of topup is still
to be written, from `specs/topup.md`; it will use none of this crate's code.

The FSL package is fsl-topup 2203.5 of the FSL 6.0.7.23 conda installation
(`$FSLDIR/src/fsl-topup/`; tag on `git.fmrib.ox.ac.uk`); its original source is kept,
unmodified, in `../fsl-source/fsl-topup/` (the FSL Licence requires it with any transmitted
product).

## Licences of the translated sources

- **FSL** (fsl-topup 2203.5): "FMRIB Software Library, Release 6.0 (c) 2018, The University of
  Oxford", the FSL Licence (non-commercial). topup: "Jesper Andersson, FMRIB Image Analysis
  Group; Copyright (C) 2012 University of Oxford".

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates.

## FSL files and functions translated

| Rust module | FSL file | Functions, classes |
|---|---|---|
| `topup` | fsl-topup `topup.cpp` | `main`: the input read and scaled to a mean of 100 (`--scale`), the first level, the further levels, the outputs, every message and exit status; `--profile`'s entries (`FSLProfiler`, the core's `profiler`: `<out>.topup_profile_topup.cpp_main.txt`) |
| `options` | fsl-topup `topupfns.{h,cpp}` | `parse_topup_command_line` (the options in FSL's order, `-h,--help` twice; the configuration file after the command line; the usage; the log file of the parsed settings), `topup_clp` (every check, the per-level settings, `Regridding`, the output names), `existing_conf_file`, `check_exist`, `path`, `filename`, `extension` |
| `costfn` | fsl-topup `topup_costfunctions.{h,cpp}` | `TopupCF`: `cf`, `grad`, `hess` (the movement tiles of `TiledMatrix`, the interaction block), `SubSample`, `SetWarpResolution`, `SetRegridding`, `Smooth`, `FieldEnergy`, `FieldEnergyGrad`, `FieldEnergyHess`, `set_field_params`, `set_movement_params`, `sum_of_prod`, `movement_hessian`, `field_factory`, every `Write*` (coefficients, field, unwarped and mean unwarped images, Jacobians, displacement fields, movement parameters, rigid-body matrices, and `--debug`'s files); not translated: `numerical_gradient`, `numerical_hessian` (never called; their callers are commented out in FSL's source) |
| `scan` | fsl-topup `topup_costfunctions.{h,cpp}` | `TopupScan` (`update`, `GetResampled`, `GetMask`, `GetAlpha`, `GetBeta`, `GetGamma`, `GetMovementDerivative`, `GetJacobian`, `GetDisplacementField`, `ReGrid`, `SubSample`, `Smooth`, `SetMovementParameters`, `SetInterpolationModel`, `ImageSize`, `ImageVxs`, `mp_to_matrix`), `TopupScanManager` (the scans' mean, alpha, beta and gamma, the common mask, the movement parameters each scan estimates); not translated: `GetNumericalMovementDerivative` (never called) |
| `fileio` | fsl-topup `topup_file_io.{h,cpp}`, `topup_matrices.{h,cpp}`; fsl-miscmaths 2412.6 `miscmaths.cc` | `TopupDatafileReader`, `TopupFileWriter` (coefficients, field, movement parameters), `MovePar2Matrix`, `Matrix2MovePar`; `write_ascii_matrix`, `read_ascii_matrix`'s messages |
| `vol` | fsl-newimage 2601.0 `newimage.cc` | the arithmetic operators of `volume<float>` as topup uses them (in `float`; scalars converted to `float`), `calculateSums`, `mean` |
| `applytopup` | fsl-topup `applytopup.{h,cpp}` | `main` (the options in FSL's order, the usage, the compulsory options), `applytopup` (the lists, `--datatype`, the reads, the mask, the methods, the output), `jac_resample`, `volume_deffield_2_jacobian`, `resample_using_movement_parameters`, `lsr_resample` (armawrap's `CroutMatrix` inverse: Armadillo's `lu` and two triangular `solve()` calls, OpenBLAS's `dgetrf` and triangular kernels; Armadillo's band detection), `TopupCollections`, `parse_commaseparated_list`, `parse_commaseparated_numbers`, `row_col_is_alright`, `extract_row_col`, `add_to_rows_cols`, `zero_out_rows_cols`; not translated yet: `vb_resample_2D`, `vb_resample_3D`, `vb_resample_4D` and their helpers (`--method=vb2D`, `vb3D`, `vb4D`: undocumented; refused), Armadillo's band solver (`dgbtrf`, `dgbtrs`) for a line whose field is zero at both ends (refused) |
| `dispvec` | fsl-topup `displacement_vector.{h,cpp}` | `CircularArray` (`operator[]`, `IndexInRange`, `Find`, `set_from_row_or_col`), `DispVec` (`SetFromRow`, `SetFromColumn`, `GetK_Matrix`, `GetS_Matrix`, `get_non_zero_entries_of_row`); not translated: `Inv`, `GetInverseDisplacements`, `GetDisplacements`, `GetSparseK_Matrix`, `RowIsAlright`, `ColumnIsAlright`, `Print` (unused by jac and lsr) |
| `fileio` (applytopup) | fsl-topup `topup_file_io.{h,cpp}` | `TopupFileReader` (`common_read`, `read_coef_file`, `ReadMovements`, `read_movement`, `FieldAsVolume`, `Field`, `MoveMatrix`, `MoveParValid`) |
| `cli` | (larmorx) | `fsl topup` and `fsl applytopup` for the `larmorx-nc` program |

## Changes from the originals

- The scans' stages (original, regridded, subsampled, smoothed) are optional copies, `None`
  standing for FSL's shared pointer to the previous stage.
- The sparse Hessians are built with one thread for their products (`SPMAT_THREADS`): FSL's
  single-thread result at any `--nthr` and `FSL_NUM_THREADS` (D13 class (b), FSL-116). FSL's
  product of a sparse matrix and a vector sums per-thread partial vectors when its thread count
  is above one (`--nthr`, or `FSL_NUM_THREADS` for the matrices built at one thread): measured on
  the half-resolution pair, `--nthr=4` moves FSL's field by up to 0.032 Hz and its corrected
  images by up to 88 (of about 20,000), `FSL_NUM_THREADS=4` at `--nthr=1` the field by up to
  3.8e-6 Hz; OpenBLAS's own threads (`FSL_SKIP_GLOBAL=1`) changed nothing on two cases.
- D13 (`LARMORX_UPSTREAM_DEFECTS`): FSL-117 (the membrane energy's Hessian in the cost's units:
  `costfn::field_energy_hess`), FSL-118 (the grid grown by the subsamplings' least common
  multiple: `topup::regridding`), FSL-119 (the field's intent parameters: `fileio::field_volume`),
  FSL-120 (the `--acqp` file named: `options`), FSL-121 (FSLDIR unset: `options::parse`), FSL-122
  (configuration lines of any length: `options::parse`), FSL-123, FSL-125, FSL-126 (negative
  `--miter`, `--fwhm`, `--lambda` refused: `topup::meaningless_settings`), FSL-124 (a negative
  `--warpres`, refused in both modes where FSL's knot spacing wraps: `costfn::knot_spacing`);
  `reproduce` gives FSL's behaviour for each (FSL-124: a refusal there too).
- With `--trace`, `TopupScanManager::update` is traced once per getter FSL calls (`GetMean`,
  `GetMask`, ...), as FSL's are.
- `TopupScanManager::_it` is never initialised in FSL; the replica starts it unset, so the first
  `SetInterpolationModel` always sets the scans' method, which every value FSL's memory could
  hold leads to as well.
- An `FSLOUTPUTTYPE` FSL cannot write ends the program at the first image written, as
  newimage's `exit()` does (unwinding to `main`).
- applytopup: D13 (`LARMORX_UPSTREAM_DEFECTS`): FSL-132 (the first image's first volume
  resampled with the interpolation asked for: `applytopup::Ctx::first_volume`; reproduced, with
  the interpolation it was read with, as newimage's cache of a series' volumes gives FSL),
  FSL-133 (displacement lines wrapped circularly: `dispvec::CircularArray`), FSL-134 (a field
  image's Jacobian in the y voxel size; a field on another grid refused: `applytopup`), FSL-135
  (an unknown `--method` and `--method=lsr` without movements refused); `reproduce` gives FSL's
  behaviour for each (FSL-134 with `--method=lsr`: a refusal there too, FSL reads past the
  field). An uncaught exception ends FSL's applytopup with `std::terminate` (SIGABRT): the
  replica prints the same text and exits with status 1.
- applytopup: lines of fewer than 8 or more than 256 voxels along the phase encoding, a line whose
  restoration system Armadillo would solve as a band matrix, or whose condition estimate could
  fall below machine precision (Armadillo then switches to an approximate solution) are refused
  with `--method=lsr` (not replicated yet).
