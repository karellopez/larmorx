# fs.bspline's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's B-splines (`crates/larmorx-fs-core/src/bspline/`, queue item
C3.2-01) was written from [`specs/freesurfer-bspline.md`](../../specs/freesurfer-bspline.md) and the
papers it names, and checked only against black boxes: the prebuilt probe `fs_volume_probe`
(FreeSurfer's compiled library and the C library, glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-volume-probe/`](../../validation/investigations/freesurfer-volume-probe/README.md).
Nothing here comes from FreeSurfer's source or from EPFL's B-spline code; this file is open to everyone.
The comparisons and the tools are in
[`validation/investigations/freesurfer-cleanroom-volume/`](../../validation/investigations/freesurfer-cleanroom-volume/README.md).

## The comparison

- **What:** the spec's §9 groups `vbspline` (410 calls: the coefficients of volumes of the six numeric
  types for degrees 2 to 9 and 0, 1, 10, -1 with no, a fitting, another or a coefficient-less
  B-spline; sampling inside, at and past the edges, NaN and infinite points, frames -1 to one past;
  the frame-range sampler; coefficients sampled as given with degrees -2 to 12; the voxel, RAS and
  transform-array transforms; both pyramids) and `vbsrules` (71 fixed calls), and fs.volume's cubic
  B-spline paths (`specs/freesurfer-volume.md` §29 step 5, §31 step 4: group `vbsvol`, 70 calls), on
  the probe (plainly, again, and in two guard runs; every reference hash the spec's, the probe of
  commit `5d35ae1f`) and on the clean room's candidate, in both modes.
  - `reproduce`: `vbspline` 375 identical, 24 where FreeSurfer did not finish (sources without their
    cached matrices, whose header copy crashes, FS-252; a source without voxels; transforms into a
    destination with fewer frames than the source), 9 memory lines (FS-962's reads past the source, a
    frame-range sample one frame past the volume's, degrees above 9 on one voxel), and 2 lines not yet
    implemented (below); `vbsrules` 68 identical and 3 memory lines; `vbsvol` 66 identical and 4 where
    FreeSurfer did not finish.
  - `fix`: none unexplained (`vbspline` 318 identical, 58 corrected by a noted defect; `vbsrules` 64 and
    4; `vbsvol` 29 and 36: FS-962, FS-964 and the volume access's FS-52 to FS-58, FS-252).
- **How we know:** *validated* (`compare.py compare --groups vbspline,vbsrules,vbsvol`, both modes,
  2026-11-19, relay 12; with every other group of the volume probe in the same runs, log
  `oracles/logs/fs-cleanroom-2-relay12-full.log`).

## FS-964 above degree 9: FreeSurfer's end is deterministic

- **What:** sampling coefficients of degree 10 or 11 whose volume has an axis longer than one voxel
  (§4.1: FreeSurfer writes the interpolation indices past their arrays) ends the program in FreeSurfer
  with `Invalid spline degree` and status 1, the same in all four of the probe's runs (plain, again in
  another process, both guard allocators): its weights refuse the degree right after the indices, so
  the past-array writes never show. The spec states FS-964 as an error in both modes. The clean room
  follows D13 (`reproduce` gives the original's result wherever it is deterministic): `reproduce` ends
  with FreeSurfer's message and status, noting FS-964; the default refuses the degree with FS-964's
  error (`ERROR_BADPARM`, no sample). On a volume of one voxel (degrees above 9, or below 0) the probe's
  runs differ (memory lines): both modes refuse it. Lines: `vbsrules`' `bsraw` of degrees 10 and 11 on
  3 x 3 x 3 coefficients and one `vbspline` `bsraw` of degree 10 on 3 x 4 x 3.
- **How we know:** *measured* (`vbsrules`, `vbspline`, 2026-11-19); reported to the spec writer.

## Not yet implemented: physical-voxel transform arrays

- **What:** `LTAtransformBSpline` (§5.3) with an array of type `LINEAR_PHYSVOX_TO_PHYSVOX` (2) changes it
  to RAS with `LTAchangeType` (`specs/freesurfer-volume.md` §54), whose text is not yet read through for
  the clean room (it also holds FS-506). The clean room implements its first step, every source and
  destination geometry valid (`LTAchangeType: src geometry must be valid` / `dst`, status 252; observed
  in `vbspline`), and ends the program there for valid geometries (code -3, `not yet ported`). Two
  `vbspline` lines reach it; FreeSurfer ends both later in that chain (`TransformInvert: xform
  noninvertible`, and the matrix product's `m1 is null`, status 252).
- **How we know:** *measured* (`vbspline`, 2026-11-19).

## Arithmetic: plain operations first

- **What:** the line filter, the pyramids' reduce and expand filters and the sampler's sums are written
  with x86-64's SSE rules for NaNs made explicit (`crate::sse`). Without a NaN anywhere the plain IEEE
  operations give the same bits on every platform, and in these functions a NaN always reaches the
  output (no comparison or selection between the operations): the clean room computes with plain
  operations and computes again with the explicit rules only when a NaN reaches the output (the line
  filter tests its line for non-finite samples first). Every line of the three groups is the same either
  way; the goldens agree in debug and release builds and under the AArch64 NaN emulation.
- **How we know:** *validated* (the comparisons above, before and after the change; the golden runs).

## Speed

- **What:** the spec's §9 gives one reference number (a 256^3 `float` volume's coefficients in about half
  a second at one thread); measured on byte volumes (finite values), one batch line each end to end
  (process start, parsing and printing included, about 0.004 s), best of 3, the candidate at one
  thread, every result line identical; the probe's coefficients run slices in parallel: the first line
  with `OMP_NUM_THREADS=1` (without it 0.062 s), the others as `run.sh` runs it, without a thread limit
  (so the ratios are lower bounds)
  (`validation/investigations/freesurfer-cleanroom-volume/speed/`, `mk.py bspline`):

  | Line | FreeSurfer | Clean room | Ratio |
  |---|---|---|---|
  | coefficients of a 128^3 volume, degree 3, one sample | 0.063 to 0.067 s | 0.052 s | 1.2x |
  | coefficients of a 64^3 volume, degree 5, one sample | 0.013 s | 0.010 s | 1.4x |
  | coefficients of a 64^3 volume and 20,000 samples, degree 3 | 0.036 s | 0.025 s | 1.4x |
  | `MRIlinearTransformBSpline` of a 32^3 volume by a rotation | 0.012 s | 0.009 s | 1.3x |
  | `MRIdownsample2BSpline` of a 64^3 volume | 0.015 s | 0.015 s | 1.0x |
  | `MRIupsample2BSpline` of a 32^3 volume | 0.022 s | 0.020 s | 1.1x |
  | `MRIlinearTransformInterp` with method 5, 32^3 | 0.012 s | 0.010 s | 1.2x |
  | `MRIresampleFill` with type 5 onto a 40^3 template | 0.020 s | 0.017 s | 1.2x |

  The changes that brought these (results unchanged): the in-range voxel reads without the per-call
  rules (`Mri::inside_vox_val`), the plain arithmetic above, the coefficients' passes on the `float`
  buffer.
- **How we know:** *measured* (2026-11-19, relay 12; the reference machine).

## Golden cases and platforms

- **What:** `tests/golden_volume.rs`'s groups `bspline` (every fs.bspline command of the probe's grammar
  on seeded arguments, the transform arrays on clean geometries too) and `bsvol` (the cubic B-spline
  paths), in both modes; `tests/defects_volume.rs`'s cases of FS-962, FS-963 and FS-964. Identical in
  debug and release builds on x86-64 and under the AArch64 NaN emulation of both.
- **How we know:** *measured* (2026-11-19, relay 12).
