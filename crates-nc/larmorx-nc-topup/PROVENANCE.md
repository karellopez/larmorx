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
| `topup` | fsl-topup `topup.cpp` | `main`: the input read and scaled to a mean of 100 (`--scale`), the first level, the further levels, the outputs, every message and exit status |
| `options` | fsl-topup `topupfns.{h,cpp}` | `parse_topup_command_line` (the options in FSL's order, `-h,--help` twice; the configuration file after the command line; the usage; the log file of the parsed settings), `topup_clp` (every check, the per-level settings, `Regridding`, the output names), `existing_conf_file`, `check_exist`, `path`, `filename`, `extension` |
| `costfn` | fsl-topup `topup_costfunctions.{h,cpp}` | `TopupCF`: `cf`, `grad`, `hess` (the movement tiles of `TiledMatrix`, the interaction block), `SubSample`, `SetWarpResolution`, `SetRegridding`, `Smooth`, `FieldEnergy`, `FieldEnergyGrad`, `FieldEnergyHess`, `set_field_params`, `set_movement_params`, `sum_of_prod`, `movement_hessian`, `field_factory`, every `Write*` (coefficients, field, unwarped and mean unwarped images, Jacobians, displacement fields, movement parameters, rigid-body matrices, and `--debug`'s files); not translated: `numerical_gradient`, `numerical_hessian` (never called; their callers are commented out in FSL's source) |
| `scan` | fsl-topup `topup_costfunctions.{h,cpp}` | `TopupScan` (`update`, `GetResampled`, `GetMask`, `GetAlpha`, `GetBeta`, `GetGamma`, `GetMovementDerivative`, `GetJacobian`, `GetDisplacementField`, `ReGrid`, `SubSample`, `Smooth`, `SetMovementParameters`, `SetInterpolationModel`, `ImageSize`, `ImageVxs`, `mp_to_matrix`), `TopupScanManager` (the scans' mean, alpha, beta and gamma, the common mask, the movement parameters each scan estimates); not translated: `GetNumericalMovementDerivative` (never called) |
| `fileio` | fsl-topup `topup_file_io.{h,cpp}`, `topup_matrices.{h,cpp}`; fsl-miscmaths 2412.6 `miscmaths.cc` | `TopupDatafileReader`, `TopupFileWriter` (coefficients, field, movement parameters), `MovePar2Matrix`, `Matrix2MovePar`; `write_ascii_matrix`, `read_ascii_matrix`'s messages |
| `vol` | fsl-newimage 2601.0 `newimage.cc` | the arithmetic operators of `volume<float>` as topup uses them (in `float`; scalars converted to `float`), `calculateSums`, `mean` |
| `cli` | (larmorx) | `fsl topup` for the `larmorx-nc` program |

## Changes from the originals

- The scans' stages (original, regridded, subsampled, smoothed) are optional copies, `None`
  standing for FSL's shared pointer to the previous stage.
- The sparse Hessians are built with one thread for their products (`SPMAT_THREADS`): FSL's
  single-thread result. FSL's product of a sparse matrix and a vector sums per-thread partial
  vectors when its thread count is above one (`--nthr`, or `FSL_NUM_THREADS` for the matrices
  built at one thread), which changes its last bits; that dependence is measured and decided in
  the next part (D13: the replica gives the one-thread result).
- `TopupScanManager::_it` is never initialised in FSL; the replica starts it unset, so the first
  `SetInterpolationModel` always sets the scans' method, which every value FSL's memory could
  hold leads to as well.
- An `FSLOUTPUTTYPE` FSL cannot write ends the program at the first image written, as
  newimage's `exit()` does (unwinding to `main`).
- Not yet translated (the next sub-tasks): `--profile`'s timing file (the option is parsed),
  `applytopup`.
