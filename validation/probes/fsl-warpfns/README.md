# fsl-warpfns' resampling and miscmaths' spline interpolator, bit for bit

What it shows: that `larmorx-nc-core`'s `splinterpolator` (fsl-miscmaths 2412.6's
`Splinterpolator<T>`), `interp_partial` (fsl-newimage 2601.0's `interpolate`, `interp1partial`,
`interp3partial`, `valid` with extrapolation parameters) and `warpfns` (fsl-warpfns 2501.0's
`raw_general_transform` and its wrappers, `deffield2jacobian`) give FSL's own bits, at one and at
three threads. Made for the topup replica (2026-11-03, sub-task 1c): topup resamples its images
with `general_transform_3partial` (spline or trilinear interpolation, periodic extrapolation
valid along the phase-encoding direction) and modulates them with `deffield2jacobian`;
applytopup uses `general_transform`, `affine_transform` and `interp3partial`.

`warpfns_probe.cc` links FSL 6.0.7.23's own `libfsl-warpfns`, `libfsl-newimage`,
`libfsl-miscmaths` and `libfsl-basisfield`, reads one case directory (`params.txt` and raw
little-endian arrays) and prints every result as the hexadecimal bits of its value, one value per
line, or `<tag> error <what()>` for an exception. Four kinds of case:

- `spl`: a `Splinterpolator<float>` and a `Splinterpolator<double>` of the case's data: the
  coefficients; at 40 points (inside, outside, on and next to grid points; NaN and infinite
  coordinates in `spl-nan`) the value, the value with each derivative, `ValAndDerivs`, the
  `operator()(x, y, z, t)` and `(x, y, z, dd)` forms; at 10 grid points `Grad`, `Deriv`, `Grad3D`.
  Orders 0 to 7, one to five dimensions, a dimension of one voxel, each extrapolation (`Zeros`,
  `Constant`, `Mirror`, `Periodic`, `SoftZeros`, `CorrectConstant`).
- `vol`: a 9 × 8 × 7 `volume<float>` with an interpolation (spline of order 2, 3, 5; trilinear;
  nearest neighbour), an extrapolation (each of newimage's five), a pad value and extrapolation
  validity: at 83 points `interpolate`, `interp1partial` along x, y, z, `interp3partial` and
  `valid`, with the volume's own parameters and with other ones (`extrapparams(method, validity)`).
- `warp`: `general_transform_3partial` with a mask (topup's call), `general_transform` with and
  without a mask, `affine_transform` with a mask, `affine_transform_3partial`,
  `displacement_transform_1D_3partial`, `raw_general_transform` with one derivative,
  `apply_warp` with `TT` and `M`, `affine_transform` of two slices: the outputs, derivatives,
  masks, sforms and qforms. Seven cases: topup-like (y phase encoding, a small rigid movement,
  cubic splines, periodic, valid along y), x phase encoding with trilinear interpolation, the
  image on another grid than the output (topup's regridding), displacement fields on their own
  grid (`displacements_with_iT`), a 12-parameter affine with quadratic splines and zero padding,
  mirror extrapolation, nearest neighbour (derivatives refused).
- `jac`: `deffield2jacobian` of one field along x and y (and z, refused), of three fields
  (topup's: the same field scaled along x and y, zero along z) with the identity, of three fields
  with an affine, `deffield2jacobian_matrix`; `jac` the field's size, smaller (with periodic,
  mirror and extraslice extrapolation: FSL writes the voxels outside `jac` through its
  extrapolation) and larger.

The replica's side is the ignored test `warpfns_probe` of
`crates-nc/larmorx-nc-core/tests/warpfns_probe.rs`, which writes the same lines. `check.py` makes
the cases (seeded), runs the probe on each (the `warp` cases at one and at three threads), runs
the replica's side at one and at three threads, and counts the differing lines per quantity.

```bash
bash validation/probes/fsl-warpfns/build.sh <scratch>/warpfns_probe
scripts/orchestration/boxed.sh 8 python3 -P validation/probes/fsl-warpfns/check.py \
    --probe <scratch>/warpfns_probe --work <scratch>/wfprobe
```

Result on 2026-11-03: 43 cases, 327,745 lines, 0 differ (the replica at one and at three threads);
FSL's three-thread results identical to its one-thread ones, except `warp-nearest`, where FSL
aborts (below).

Facts it found (for the topup replica's findings, restricted):

- **A worker thread's exception ends FSL's program.** `raw_general_transform` runs its rows on
  `std::thread`s; an exception thrown in one of them (`interp3partial` with nearest-neighbour or
  sinc interpolation) is not caught there, so the program ends with `terminate called without an
  active exception` (SIGABRT) instead of the exception the one-thread run throws. The replica
  returns that exception at every thread count. topup and applytopup interpolate by splines or
  trilinearly, so they never meet it.
- **newimage's periodic and mirror extrapolation truncate the index to `int`.** `extrapolate`
  calls miscmaths' `periodicclamp(int, int, int)` with `int64_t` indices: a position whose floor
  is beyond 2^31 (an infinite or NaN coordinate: x86-64 converts it to `INT64_MIN`) wraps to
  voxel 0. Found by the case `vol-tri-zeropad` (periodic parameters, an infinite coordinate);
  `interp_partial::periodicclamp_int` does the same (`volume::periodicclamp`, which the older
  paths use, takes the 64-bit index).
- **`validate_input` never checks the slices.** Its test of the slice list runs only when the
  list is empty (`if (slices.size()==0)`), so a slice beyond the output is written through the
  output's extrapolation (lost for zero padding, wrapped into the volume for periodic). The
  replica refuses it. Not reachable from topup or applytopup (they pass no slices).
- **`raw_apply_warp` drops the image's extrapolation parameters.** `calculate_extrapolation`
  takes only the method and validity from `infext` (for the fields') and returns default
  parameters for the image, so `apply_warp(invol, outvol, warpvol, premat, postmat)`'s
  `{extraslice, backgroundval()}` has no effect on the image's padding (applywarp's path; not
  topup's).
- **`Splinterpolator::Set(data, dim, et, ...)` (one extrapolation) ignores `et`**: it builds a
  vector of `Zeros`. Not called by newimage, topup or applytopup.
- Several argument checks build an exception and do not throw it (`ValAndDerivs`' `rderiv`
  length, `Deriv`'s and `Grad`'s index length, `Grad3D`'s dimensionality): FSL then reads or
  writes past the arrays; the replica refuses the first three and, for `Grad3D`, returns what
  FSL computes (zeros beyond the dimensions).
- With NaN coordinates, the spline interpolator's start index is `INT_MIN`; its `Periodic` and
  `Mirror` index loops then take about 2^31 / size steps per coefficient (seconds per value).
  The replica's index mapping is the closed form of those loops (the same results).
- New volumes are not initialised by newimage (`new T[n]`): the probe sets its outputs to 0
  first; topup does the same before resampling (`_resampled = 0.0; _derivs = 0.0; _mask = 0`).
