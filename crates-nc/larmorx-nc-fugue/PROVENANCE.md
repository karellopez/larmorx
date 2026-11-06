# Provenance: larmorx-nc-fugue

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.23's phase-unwrapping program `prelude` (PRELUDE, Phase Region Expanding Labeller for
Unwrapping Discrete Estimates; Jenkinson 2003, "Fast, automated, N-dimensional phase-unwrapping
algorithm", MRM 49:193-197) and the routines of `unwarpfns` it calls, written to give FSL's
output bit for bit. It is distributed under the FSL Licence's terms (`LicenseRef-FSL`:
non-commercial use only, `../LICENSE`), as part of the `larmorx-nc` package
(`docs/licensing.md`). The parts of FSL's libraries and of the C and C++ libraries it relies on
(newimage's volumes, their extrema, percentiles, binarisation and connected components, the file
reading and writing, the complex reader `read_complexvolume` with `phase` and `abs`; NewNifti;
utils' option parser; glibc's `atan2f`; libstdc++'s `std::sort`; armawrap's printing of a column
vector) are the shared core's, `../larmorx-nc-core` (its `PROVENANCE.md`; the complex reader is
its module `complex`). The Apache-2.0 clean-room original of PRELUDE is still to be written, from
`specs/prelude.md`; it will use none of this crate's code.

The FSL package is fsl-fugue 2603.0 of the FSL 6.0.7.23 conda installation
(`$FSLDIR/src/fsl-fugue/`; tag on `git.fmrib.ox.ac.uk`); its original source is kept,
unmodified, in `../fsl-source/fsl-fugue/` (`prelude.cc`, `unwarpfns.{h,cc}`, the `Makefile` that
links `prelude` from `prelude.o` and `unwarpfns.o`; the FSL Licence requires it with any
transmitted product).

## Licences of the translated sources

- **FSL** (fsl-fugue 2603.0): "FMRIB Software Library, Release 6.0 (c) 2018, The University of
  Oxford", the FSL Licence (non-commercial). PRELUDE: "Mark Jenkinson, FMRIB Image Analysis
  Group; Copyright (C) 2000/2001 University of Oxford".

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates.

## FSL files and functions translated

| Rust module | FSL file | Functions |
|---|---|---|
| `prelude` | fsl-fugue `prelude.cc` | the options in their order, `main` (the usage, the option errors, the checks of `--complex`, `--phase`, `--abs`, `--numphasesplit`), `wrap(volume<float>&)`, `do_unwrapping` (the inputs, the phase range check and rewrapping, the automatic 2D labels, the per-volume masks, ramps, labels and unwrapping, the files written) |
| `unwarpfns` | fsl-fugue `unwarpfns.{h,cc}` | `maximum_position`, `basic_mask_threshold`, `make_basic_head_mask`, `largest_connected_component`, `make_head_mask`, `make_head_mask2D`, `wrap(float)`, `estimate_linear_ramps`, `remove_linear_ramps`, `restore_linear_ramps`, `sconstraint`, `getP`, `find`, `find_maxC`, `calc_constraint`, `make_constraints`, `apply_unwrapping`, `find_phase_labels`, `find_phase_labels2D`, `unwrap`, `unwrap2D`; not translated (prelude does not call them): `pava`, the despike and median filters, `fill_head_mask`, `fill_holes`, the extrapolations, `connection_matrices`, `calc_cost`, `calc_delta_cost`, `calc_fmap`, `fmap2pixshift_factor`, `yderiv`, `limit_pixshift`, `apply_pixshift`, `simple_erode`, `simple_dilate` |

## Changes from the originals

- **The merging of `unwrap`** keeps, beside FSL's constraint map (a `BTreeMap` with `std::map`'s
  key order), each label's neighbours and members and the constraints ordered by `C` (largest
  first, then the smallest pair: `find_maxC`'s choice, a NaN first `C` included), so each merge
  touches the merged labels' constraints only; FSL scans every label for every merge (O(L²) for
  L labels: 6.5 s against 0.15 s on fMRIPrep's ds005454 call, 14,678 labels). The constraints
  visited, their arithmetic and its order are FSL's; the test `unwrap_matches_fsl_loops` compares
  the result with `unwrap_reference`, a statement-by-statement translation.
- One volume's work after its mask (`per_volume`) is a function, so that a correction whose
  notice needs FSL's result computes both (FSL-261, FSL-268, FSL-269: the files, `-v`'s lines
  and, in `reproduce`, grot compared); the messages of `-v` keep FSL's order.
- The thresholds' two percentiles sort the values once (FSL sorts the same values twice: the
  same order).
- Slices (`ROI`, `copyROI`) are copied plainly; a slice beyond a smaller mask holds the mask's
  extrapolation, as FSL's `ROI` reads it (FSL-271's reproduction).

## Upstream defects (D13)

`LARMORX_UPSTREAM_DEFECTS=reproduce` gives FSL's result bit for bit; the default `fix` corrects
FSL-261 to FSL-266 and FSL-268 to FSL-272 (FSL-267, the range message, is kept as FSL prints it;
FSL-272, the extrema of the values that are not NaN: `prelude::extrema_without_nan`, 2026-11-06):
`docs/findings/fsl-prelude.md` and the register.
