# 3dvolreg's clean-room original: findings

Written by the clean-room implementer of `specs/3dvolreg.md` (the original in
`crates/larmorx-afni/src/volreg/`, `crates/larmorx-afni/src/cli/volreg.rs`). Every statement here
comes from the spec, AFNI's documentation and black-box runs of AFNI 26.3.05's `3dvolreg`
(through the oracle-output cache) and of the replica (`LARMORX_IMPLEMENTATION=replica`, a black
box): no AFNI source and no replica code were read. Unrestricted. The tools behind them are in
`validation/investigations/afni-volreg-original/` (its README) and the parity suite
`afni-volreg-original` (`validation/src/larmorx_validation/parity/afni_volreg_original.py`; the
record `docs/validation/afni-volreg-original.md`).

## The shear decomposition's exact ties decide the remaining differences

The spec (§5.3 "Numerics", §16) explains why the original cannot be bit-identical: where two
axes have the same voxel size, pairs of axis orders of the four-shear decomposition have equal
norms in exact arithmetic, so the evaluation's last bits choose the order at every resampling,
and AFNI's evaluation is not specified. §14.3's thresholds allow for either side of every tie on
the spec writer's 15 cases.

To measure what the ties do on the suite's and random testing's inputs, the original takes a
given side on request: `LARMORX_VOLREG_TIES` (a validation switch, not a user setting:
`crates/larmorx-afni/src/volreg/rotate.rs`) is `first`, `last` or the index (0 to 5) of the order
taken among those whose norms are within a relative 1e-12 of the smallest; unset, the spec's rule
(the smallest computed norm). The suite re-runs a case with the four sides whenever a comparison
fails, and reports a failure as the ties' when one side (or, for the registered volumes and
fMRIPrep's median, each volume from the side closest to AFNI's) meets every threshold.

- **The registered volumes of a real high-motion run.** fMRIPrep's call on ds000256's child run
  with the most motion (`restfixationnofeedback`, rotations to 4.7°, 20 volumes): every parameter
  within 0.004 of AFNI's, but six volumes differ from AFNI's by a mean of 1.5e-3 to 2.3e-3 of their
  largest value (§14.3: 2e-3) and up to 3.5 % at single edge voxels, and fMRIPrep's median by 2 %
  at an edge voxel. With tie side 1 (or 3) the original gives AFNI's volumes 10 to 17 to a mean of
  1e-5 or better, but volume 8 then differs (3.4e-3), which the default side gives to 1e-5: AFNI
  decides each volume's resampling its own way. Per volume, one of the original's sides gives each
  of AFNI's volumes. The difference is the circular Fourier shift's wrap-around along the axes the
  two orders shift (x-y-x against y-x-y shears for a rotation about z).
- **fMRIPrep's medians beyond 1e-4.** ds005454 1.07e-4, ds004958 1.37e-4, ds000228 2.4e-4 of the
  largest value (§14.3: 1e-4, measured by the spec writer on ds000005 and ds003592 only, where the
  original gives 1.5e-5: one stored int16 step).
- **No single rule does better than the spec's.** Identical `-1Dfile` lines with AFNI's for each
  side (smallest, 0, 1, 2, 3, last): ds000005 with fMRIPrep's call 17, 14, 13, 13, 12, 12 of 20;
  `s1` (the suite's moving series) heptic 2, 2, 2, 1, 1, 1 of 6, Fourier with `-twopass` 6, 4, 6,
  4, 6, 6 of 6; linear 1 of 6 for every side. The spec writer measured the same (AFNI's choice
  follows its own rounding; agreeing in 75 % of the derivative images), so the original keeps the
  spec's rule.
- **`-final NN`**: nearest neighbour rounds at each of the four shears, so a tie decided the other
  way moves whole rows by a voxel: `-twopass -Fourier -final NN` on `s1` gives every `-1Dfile` line
  of AFNI's and one volume differing at 34,036 of 42,240 voxels (NCC 0.9917); with tie side 1 the
  original gives AFNI's volumes.

**Open question for the spec writer:** §14.3's registered-volume (2e-3) and median (1e-4)
thresholds are exceeded by the ties alone on real data with more motion than the 15 cases had
(ds000256, ds000228, ds005454, ds004958); `-final NN` has no threshold that the ties can meet.

## Random testing: fits that do not converge, ill-conditioned fits

<!-- filled from the trial -->

## Behaviours found and corrected in the original (black-box, against AFNI and the replica)

The suite's first runs found these; each is fixed, and the case that found it is in the suite.

- **A volume that is not 3D, with `-twopass`** (§7.3): each of the coarse search's 44 rotations
  prints `*** THD_rota3D_matvec: non-3D image input!` (a blank line before; with `-verbose`
  interleaved with the search's marks, the identity's mark alone), and the first pass's failure is
  preceded by AFNI-93's notice (the search's region was empty).
- **The coarse search's line prints the internal angles**: the candidate's angle times the axis's
  sign, so a zero about an axis of negative sign is `-0` (`0 -0 -0 : 0 0 0` for data stored RAS),
  and a candidate of 9° is printed `-9` there.
- **An empty trim's bounds**: ` + skipping empty trim, {xyz}{a,b} = N,-1, ...` (the search's start,
  N the axis's size) when some weight is not 0 but no voxel survives the trimming's cluster and
  peel; `0,0, ...` only for an all-zero weight (as §7.3 states).
- **The trimming's lines when the box saves too little**: ` + wtrim: ...` (the box found, with
  `-verbose -verbose`) comes before ` + skipping use of trim - too little savings`; with `-wtinp`
  the `:` after the weight's blur, as for the default weight.
- **`-rotcom`** ends with a blank line (§10.2).
- **`-rotparent` and `-gridparent` that cannot be opened**: AFNI's line and a blank line
  (`reproduce`, and for a missing value, AFNI-88); by default the replica's line with the name and
  the reason (`** Can't open -rotparent dataset <name>: <reason>`).
- **The NIfTI type conversion notice** is printed for the run's first converted dataset only
  (the `-base` dataset when it is read first, during the option scan).
- **`** FATAL ERROR: perhaps you could make your datasets match?`** is printed with an uncoloured
  label also with `reproduce`; **a base of fewer than 100 non-zero voxels** is a labelled fatal
  error (`** FATAL ERROR: 3dvolreg fails: ...`); **an existing output with `AFNI_DECONFLICT=NO`**
  names it as AFNI does (`./out.nii`) under a fatal label.
- **Defects named as the replica names them**: AFNI-98 (both messages; in both modes), AFNI-12
  when a `-prefix` AFNI refuses is written (3dvolreg checks the name only when writing, §10.1),
  AFNI-26 and AFNI-90 for a kept name of 256 bytes the system cannot hold, the option scan's
  notices (AFNI-86, -90, -95) before any later error, every notice in the order of its ID.

## Other observations

- **A scale mismatch between the series and an external base** (a uint16 series four times the
  base's values): AFNI and the original both end with rotations of about 280° (no defect: 3dvolreg
  fits no intensity scale without `-twopass`'s initialisation, and is meant for small motions).
- **`-SAVEDISP`'s third-order products**: §10.3's arithmetic (the three normalised displacements,
  `P_a(X)·P_b(Y)·P_c(Z)` in float32 left to right, normalised the same way), applied to AFNI's own
  raw `-savedisp` values (`_core.afni_volreg_savedisp`), gives AFNI's 3 normalised datasets and the
  6 products of one or two factors bit for bit, but 7 of the 10 products of three factors differ at
  1 to 26 of 253,440 values (by up to 140 units in the last place after the normalisation) on the
  suite's `s1`. **Open question for the spec writer:** the order of those products' arithmetic.
- **Inputs re-made for fMRIPrep's calls**: niworkflows' own lines in fMRIPrep 25.2.6's image give
  `_sliced` files byte-identical with the replica part's, and the median of AFNI's registered
  volumes, written as niworkflows writes it (the sliced input's header: int16 with nibabel's scale
  factors for an int16 run), is fMRIPrep's own `gen_avg` output byte for byte on all eight calls.
