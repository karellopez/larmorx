# Parity: One-shot BOLD resampling (fMRIPrep's ResampleSeries) and SciPy's map_coordinates

**All cases agree.** 139 cases on the `standard` tier: 136 pass, 2 rejected by both, 1 expected divergences, 0 failures.

- **Validated:** `lx.transforms.resample_series`, `lx.transforms.load_transforms`, `lx.ndimage` (crates larmorx-transform, -interp)
- **Reference:** fMRIPrep 26.0.0.dev1+g21a490fb8 `ResampleSeries` (run through nipype), nitransforms 25.1.0, SciPy 1.18.1, in-process
- **Test data:** larmorx-testdata `52124bc98f9f`, tier `standard`
- **Generated:** 2026-10-29 on Linux x86_64, with `python -m larmorx_validation parity resample-series --tier standard --python newest`
- **Original programs:** 0 runs: 0 from the oracle cache, 0 run; cache `<workspace>/oracles/cache`, mode `use`

**Bit-identical: 136 of 136 passing cases** produce exactly the values of the reference (SciPy, nitransforms or fMRIPrep's ResampleSeries), with `upstream_defects="reproduce"`.
**The originals' own step: numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 → numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1:** 136 cases compared, 122 with the same outputs under both, 14 changed: `ndimage/nearest-order2`, `ndimage/nearest-order3`, `ndimage/nearest-order4`, `ndimage/nearest-order5`, `ndimage/reflect-order2`, `ndimage/reflect-order3`, `ndimage/reflect-order4`, `ndimage/reflect-order5`, `ndimage/grid-mirror-order2`, `ndimage/grid-mirror-order3`, `ndimage/grid-mirror-order4`, `ndimage/grid-mirror-order5` …; every change has a known reason (SCI-03: SciPy 1.18 sums the reflect boundary's causal initialisation into a variable of its own (scipy/ndimage/src/ni_splines.c), so its last term reads the original first coefficient: SCI-03 fixed upstream, with the scaling's rounding of its own (modes nearest, reflect and grid-mirror, orders 2 to 5)).
- **D13 fix pass** (larmorx's default, `upstream_defects="fix"`; the comparison above runs `reproduce`): the corrections met
  - PIPE-03: 9 case(s) named it, 9 with changed values (largest difference 871): `pe/las-i`, `pe/las-i-`, `anat/las-hmc-coreg-sdc`, `std/on-grid-warp`, `std/off-grid-warp`, `threads/1`, `threads/3`, `threads/12` …
  - SCI-02: 10 case(s) named it, 10 with changed values (largest difference 2.15e+09): `ndimage/grid-constant-order1`, `ndimage/grid-constant-order2`, `ndimage/grid-constant-order3`, `ndimage/grid-constant-order4`, `ndimage/grid-constant-order5`, `ndimage/nearest-order1`, `ndimage/nearest-order2`, `ndimage/nearest-order3` …

## Thresholds

| Quantity | Requirement |
|---|---|
| Exit status | both succeed, or both reject the inputs |
| Output shape and data type | identical |
| Values | max \|diff\| ≤ 1e-06 × max \|reference\| (PLAN.md §11.3, interpolation vs SciPy); bit-identity reported for every comparison |
| Output affine | max \|diff\| ≤ 1e-6 mm |

## Results by category

| Category | Cases | Pass | Both reject | Expected divergence | Fail |
|---|---|---|---|---|---|
| 3d | 3 | 3 | 0 | 0 | 0 |
| anatomical | 3 | 3 | 0 | 0 | 0 |
| errors | 2 | 0 | 2 | 0 | 0 |
| fmriprep-real | 3 | 3 | 0 | 0 | 0 |
| interpolation | 48 | 48 | 0 | 0 | 0 |
| native | 9 | 9 | 0 | 0 | 0 |
| ndimage | 49 | 48 | 0 | 1 | 0 |
| nitransforms | 10 | 10 | 0 | 0 | 0 |
| phase-encoding | 6 | 6 | 0 | 0 | 0 |
| standard | 3 | 3 | 0 | 0 | 0 |
| threads | 3 | 3 | 0 | 0 | 0 |

## Expected divergences

| Case | Reason |
|---|---|
| `ndimage/overflowing-coordinates` | For coordinates beyond the range of int64 (or whose index arithmetic overflows), SciPy's C code casts and multiplies with undefined behaviour and then reads outside its buffer or wherever the wrapped offset points; larmorx returns NaN there instead (14 of 48 mode/order pairs differ: nearest/order2,… |

## Rejected by both

| Case | What it tests | Errors |
|---|---|---|
| `errors/hmc-not-last` | head-motion transforms that are not first in the list: ValueError | fMRIPrep: ValueError: HMC transforms must come last. Found sequence: ['LinearTransformsMapping', 'Affine']; larmorx: ValueError: HMC transforms must come last.… |
| `errors/mismatched-inverse` | two transforms, three inverse flags | fMRIPrep: ValueError: Mismatched number of transforms and inverses; larmorx: ValueError: Mismatched number of transforms and inverses |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (9b833a357520) |
| larmorx-testdata | 52124bc98f9f |
| Python | 3.12.16 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | AMD Ryzen 9 7945HX with Radeon Graphics, 32 logical CPUs |
| numpy | 2.5.4 |
| fmriprep | 26.0.0.dev1+g21a490fb8 |
| nitransforms | 25.1.0 |
| scipy | 1.18.1 |
| nibabel | 5.4.2 |
| nipype | 1.12.0 |
| h5py | 3.16.0 |

## Notes

fMRIPrep's interface runs exactly as in a workflow (nipype, files in and out); larmorx gets the same files. ndimage cases compare whole batches of outputs per configuration group.

## All cases

| Case | Status | Checks passed | What it tests |
|---|---|---|---|
| `ndimage/grid-constant-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-constant, order 0 |
| `ndimage/grid-constant-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-constant, order 1 |
| `ndimage/grid-constant-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-constant, order 2 |
| `ndimage/grid-constant-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-constant, order 3 |
| `ndimage/grid-constant-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-constant, order 4 |
| `ndimage/grid-constant-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-constant, order 5 |
| `ndimage/constant-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode constant, order 0 |
| `ndimage/constant-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode constant, order 1 |
| `ndimage/constant-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode constant, order 2 |
| `ndimage/constant-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode constant, order 3 |
| `ndimage/constant-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode constant, order 4 |
| `ndimage/constant-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode constant, order 5 |
| `ndimage/nearest-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode nearest, order 0 |
| `ndimage/nearest-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode nearest, order 1 |
| `ndimage/nearest-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode nearest, order 2 |
| `ndimage/nearest-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode nearest, order 3 |
| `ndimage/nearest-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode nearest, order 4 |
| `ndimage/nearest-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode nearest, order 5 |
| `ndimage/mirror-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode mirror, order 0 |
| `ndimage/mirror-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode mirror, order 1 |
| `ndimage/mirror-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode mirror, order 2 |
| `ndimage/mirror-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode mirror, order 3 |
| `ndimage/mirror-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode mirror, order 4 |
| `ndimage/mirror-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode mirror, order 5 |
| `ndimage/reflect-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode reflect, order 0 |
| `ndimage/reflect-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode reflect, order 1 |
| `ndimage/reflect-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode reflect, order 2 |
| `ndimage/reflect-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode reflect, order 3 |
| `ndimage/reflect-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode reflect, order 4 |
| `ndimage/reflect-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode reflect, order 5 |
| `ndimage/grid-mirror-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-mirror, order 0 |
| `ndimage/grid-mirror-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-mirror, order 1 |
| `ndimage/grid-mirror-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-mirror, order 2 |
| `ndimage/grid-mirror-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-mirror, order 3 |
| `ndimage/grid-mirror-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-mirror, order 4 |
| `ndimage/grid-mirror-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-mirror, order 5 |
| `ndimage/wrap-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode wrap, order 0 |
| `ndimage/wrap-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode wrap, order 1 |
| `ndimage/wrap-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode wrap, order 2 |
| `ndimage/wrap-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode wrap, order 3 |
| `ndimage/wrap-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode wrap, order 4 |
| `ndimage/wrap-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode wrap, order 5 |
| `ndimage/grid-wrap-order0` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-wrap, order 0 |
| `ndimage/grid-wrap-order1` | pass | 271/271 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-wrap, order 1 |
| `ndimage/grid-wrap-order2` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-wrap, order 2 |
| `ndimage/grid-wrap-order3` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-wrap, order 3 |
| `ndimage/grid-wrap-order4` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-wrap, order 4 |
| `ndimage/grid-wrap-order5` | pass | 277/277 | scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode grid-wrap, order 5 |
| `ndimage/overflowing-coordinates` | expected-divergence | 14/14 | coordinates of 1e19-1e20, where SciPy's index arithmetic overflows (undefined behaviour) |
| `nitransforms/hmc-series` | pass | 3/3 | an ITK text list of 8 MatrixOffsetTransformBase transforms (float32 parsing, LPS to RAS) |
| `nitransforms/centred-affine` | pass | 6/6 | a centred ITK affine mapping an oblique grid (float32 inputs, fused products) |
| `nitransforms/inverse-affine` | pass | 6/6 | the same affine inverted (np.linalg.inv of the RAS matrix) |
| `nitransforms/composite-on-grid` | pass | 6/6 | an ITK .h5 composite, target on the field's nodes (displacements looked up) |
| `nitransforms/composite-off-grid` | pass | 6/6 | an ITK .h5 composite, target off the nodes (cubic interpolation of the field, NaN outside) |
| `nitransforms/chain-affine-composite` | pass | 6/6 | a chain: composite then affine, as fMRIPrep chains boldref→anat and anat→std |
| `nitransforms/fmriprep-mni-warp` | pass | 6/6 | fMRIPrep 21.0.1's boldref→T1w affine and T1w→MNI .h5 warp, on the MNI res-2 grid |
| `nitransforms/ndcoords-boldref-las` | pass | 3/3 | ImageGrid.ndcoords of boldref-las.nii.gz |
| `nitransforms/ndcoords-anat-1` | pass | 3/3 | ImageGrid.ndcoords of anat-1.6mm.nii.gz |
| `nitransforms/ndcoords-std-2` | pass | 3/3 | ImageGrid.ndcoords of std-2.7mm.nii.gz |
| `native/las-hmc` | pass | 6/6 | las series onto its boldref grid: head motion only |
| `native/las-hmc-sdc-jacobian` | pass | 6/6 | las series onto its boldref: head motion + field map, PE j-, Jacobian |
| `native/las-hmc-sdc` | pass | 6/6 | las series onto its boldref: head motion + field map, PE j- |
| `native/ras-hmc` | pass | 6/6 | ras series onto its boldref grid: head motion only |
| `native/ras-hmc-sdc-jacobian` | pass | 6/6 | ras series onto its boldref: head motion + field map, PE i, Jacobian |
| `native/ras-hmc-sdc` | pass | 6/6 | ras series onto its boldref: head motion + field map, PE i |
| `native/als-hmc` | pass | 6/6 | als series onto its boldref grid: head motion only |
| `native/als-hmc-sdc-jacobian` | pass | 6/6 | als series onto its boldref: head motion + field map, PE k-, Jacobian |
| `native/als-hmc-sdc` | pass | 6/6 | als series onto its boldref: head motion + field map, PE k- |
| `pe/las-i` | pass | 6/6 | LAS series, PE i: the readout sign after reorientation to RAS |
| `pe/las-i-` | pass | 6/6 | LAS series, PE i-: the readout sign after reorientation to RAS |
| `pe/las-j` | pass | 6/6 | LAS series, PE j: the readout sign after reorientation to RAS |
| `pe/las-j-` | pass | 6/6 | LAS series, PE j-: the readout sign after reorientation to RAS |
| `pe/las-k` | pass | 6/6 | LAS series, PE k: the readout sign after reorientation to RAS |
| `pe/las-k-` | pass | 6/6 | LAS series, PE k-: the readout sign after reorientation to RAS |
| `anat/las-hmc-coreg-sdc` | pass | 6/6 | LAS series onto an oblique 1.6 mm grid: head motion + boldref→anatomical affine + field map |
| `anat/ras-hmc-coreg` | pass | 6/6 | oblique RAS series onto the anatomical grid: head motion + affine, no field map |
| `anat/inverse-affine` | pass | 6/6 | the affine inverted (inverse=[False, True]): nitransforms' inverse of a centred ITK affine |
| `std/on-grid-warp` | pass | 6/6 | LAS series to a 4 mm grid on the warp's nodes (displacements looked up): motion + affine + .h5 composite + field map |
| `std/off-grid-warp` | pass | 6/6 | LAS series to a 2.7 mm grid off the warp's nodes (displacements interpolated, NaN outside): motion + affine + .h5 + fie… |
| `std/als-off-grid` | pass | 6/6 | axis-permuted series to the 2.7 mm grid through the warp, no field map |
| `3d/sbref-anat` | pass | 6/6 | a single volume onto the anatomical grid through the affine |
| `3d/sbref-native-sdc` | pass | 6/6 | a single volume onto its own grid with a field map, PE j- |
| `3d/sbref-identity` | pass | 6/6 | a single volume onto its own grid, no transforms |
| `interpolation/order0-grid-constant` | pass | 6/6 | order 0, mode grid-constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-constant` | pass | 6/6 | order 0, mode constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-nearest` | pass | 6/6 | order 0, mode nearest: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-mirror` | pass | 6/6 | order 0, mode mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-reflect` | pass | 6/6 | order 0, mode reflect: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-grid-mirror` | pass | 6/6 | order 0, mode grid-mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-wrap` | pass | 6/6 | order 0, mode wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order0-grid-wrap` | pass | 6/6 | order 0, mode grid-wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-grid-constant` | pass | 6/6 | order 1, mode grid-constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-constant` | pass | 6/6 | order 1, mode constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-nearest` | pass | 6/6 | order 1, mode nearest: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-mirror` | pass | 6/6 | order 1, mode mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-reflect` | pass | 6/6 | order 1, mode reflect: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-grid-mirror` | pass | 6/6 | order 1, mode grid-mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-wrap` | pass | 6/6 | order 1, mode wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order1-grid-wrap` | pass | 6/6 | order 1, mode grid-wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-grid-constant` | pass | 6/6 | order 2, mode grid-constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-constant` | pass | 6/6 | order 2, mode constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-nearest` | pass | 6/6 | order 2, mode nearest: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-mirror` | pass | 6/6 | order 2, mode mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-reflect` | pass | 6/6 | order 2, mode reflect: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-grid-mirror` | pass | 6/6 | order 2, mode grid-mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-wrap` | pass | 6/6 | order 2, mode wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order2-grid-wrap` | pass | 6/6 | order 2, mode grid-wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-grid-constant` | pass | 6/6 | order 3, mode grid-constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-constant` | pass | 6/6 | order 3, mode constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-nearest` | pass | 6/6 | order 3, mode nearest: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-mirror` | pass | 6/6 | order 3, mode mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-reflect` | pass | 6/6 | order 3, mode reflect: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-grid-mirror` | pass | 6/6 | order 3, mode grid-mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-wrap` | pass | 6/6 | order 3, mode wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order3-grid-wrap` | pass | 6/6 | order 3, mode grid-wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-grid-constant` | pass | 6/6 | order 4, mode grid-constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-constant` | pass | 6/6 | order 4, mode constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-nearest` | pass | 6/6 | order 4, mode nearest: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-mirror` | pass | 6/6 | order 4, mode mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-reflect` | pass | 6/6 | order 4, mode reflect: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-grid-mirror` | pass | 6/6 | order 4, mode grid-mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-wrap` | pass | 6/6 | order 4, mode wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order4-grid-wrap` | pass | 6/6 | order 4, mode grid-wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-grid-constant` | pass | 6/6 | order 5, mode grid-constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-constant` | pass | 6/6 | order 5, mode constant: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-nearest` | pass | 6/6 | order 5, mode nearest: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-mirror` | pass | 6/6 | order 5, mode mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-reflect` | pass | 6/6 | order 5, mode reflect: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-grid-mirror` | pass | 6/6 | order 5, mode grid-mirror: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-wrap` | pass | 6/6 | order 5, mode wrap: oblique RAS series onto the anatomical grid with motion |
| `interpolation/order5-grid-wrap` | pass | 6/6 | order 5, mode grid-wrap: oblique RAS series onto the anatomical grid with motion |
| `threads/1` | pass | 6/6 | larmorx with n_threads=1 (fMRIPrep with 4): the result does not change |
| `threads/3` | pass | 6/6 | larmorx with n_threads=3 (fMRIPrep with 4): the result does not change |
| `threads/12` | pass | 6/6 | larmorx with n_threads=12 (fMRIPrep with 4): the result does not change |
| `errors/hmc-not-last` | both-error | – | head-motion transforms that are not first in the list: ValueError |
| `errors/mismatched-inverse` | both-error | – | two transforms, three inverse flags |
| `real/native-hmc-sdc` | pass | 6/6 | ds000005 run 1 (64×64×34×240, LAS) onto its boldref: seeded motion + field map, PE j, Jacobian |
| `real/T1w` | pass | 6/6 | ds000005 run 1 onto the T1w boldref grid: motion + fMRIPrep's boldref→T1w affine + field map |
| `real/MNI152NLin2009cAsym-res2` | pass | 6/6 | ds000005 run 1 onto MNI152NLin2009cAsym res-2 (97×115×97): motion + affine + fMRIPrep's T1w→MNI .h5 warp |
