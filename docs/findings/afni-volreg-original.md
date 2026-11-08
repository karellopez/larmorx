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
and AFNI's evaluation is not specified. §14.3's thresholds allow for AFNI's own spread under the
ties, measured by the spec writer (step 3 of 2026-11-08: AFNI's arithmetic with other choices at
every resampling, on 21 cases and 16 real runs, and on random testing's findings).

To measure what the ties do, the original takes a given side on request:
`LARMORX_VOLREG_TIES` (a validation switch, not a user setting:
`crates/larmorx-afni/src/volreg/rotate.rs`) is `first`, `last` or the index (0 to 5) of the order
taken among those whose norms are within a relative 1e-12 of the smallest, the same side at every
resampling, or `random:<seed>` (the spec's §14.3 "The ties' spread", step 3 of 2026-11-08): a
uniform draw among those orders at every resampling that meets a tie (every derivative image,
every iteration, the output, every coarse rotation of every volume), from a generator seeded with
`<seed>` and started afresh by every run; the same bits at any thread count (the resamplings are
planned one after another; measured: `-twopass -Fourier` on `s1` with `random:3` identical at 1
and 14 threads); unset, the spec's rule (the smallest computed norm); any other value an error.

**What the record of relay 3 found, and how the spec answered** (the spec writer's step 3: AFNI's
own spread under the ties, measured with AFNI's arithmetic and other choices at every
resampling). The first record re-ran a failing case with the original's four global sides and
called a failure the ties' when one side (or, for the registered volumes and fMRIPrep's median,
each volume from the side closest to AFNI's) met every threshold; three cases passed only so and
one failed even so:

- **The registered volumes of a real high-motion run.** fMRIPrep's call on ds000256's child run
  with the most motion (`restfixationnofeedback`, rotations to 4.7°, 20 volumes): every parameter
  within 0.004 of AFNI's, but six volumes beyond the fixed 2e-3 of their largest value and
  fMRIPrep's median by 2 % at an edge voxel. AFNI decides each volume's resampling its own way
  (190 of the run's 334 ties to the first of the tied orders, 144 to another: the spec's §5.3), so
  no global side reproduces it. The spec now holds each volume to a limit that grows with its own
  rotation, `L_v = max(2e-3, 1e-3·θ_v)` with `θ_v = |roll| + |pitch| + |yaw|` (mean |diff| within
  `L_v` of its largest value, NCC at least `1 − 50·L_v²`); AFNI's own tie variants reach 0.68 of
  it on this run, the original's default 0.681.
- **fMRIPrep's medians beyond 1e-4** (ds005454 1.07e-4, ds004958 1.37e-4, ds000228 2.4e-4 of the
  largest value, ds000256's runs more): the median moves at a voxel by at most the largest of its
  volumes' differences there, so the spec holds it to the limit its volumes imply (mean |diff|
  within `L_med·max + q`, NCC at least `1 − 50·L_med²`, `L_med` the run's largest `L_v`, q the
  stored median's step) and reports its largest voxel.
- **`-final NN`**: a tie decided the other way moves whole rows by a voxel at each of the four
  shears (two along the same axis by up to two), so `-twopass -Fourier -final NN` on `s1` gives
  every `-1Dfile` line of AFNI's and a volume differing at 34,036 of 42,240 voxels (NCC 0.9917).
  The spec's nearest-neighbour rule: wherever the two outputs differ, the original's value is the
  input's at a voxel within two voxels (along each axis) of the input voxel whose value AFNI's
  output holds there, the input taken as 0 outside its grid; checked where the input's values are
  unique.
- **No single rule does better than the spec's.** Identical `-1Dfile` lines with AFNI's for each
  side (smallest, 0, 1, 2, 3, last): ds000005 with fMRIPrep's call 17, 14, 13, 13, 12, 12 of 20;
  `s1` (the suite's moving series) heptic 2, 2, 2, 1, 1, 1 of 6, Fourier with `-twopass` 6, 4, 6,
  4, 6, 6 of 6; linear 1 of 6 for every side. The spec writer measured the same (AFNI's choice
  follows its own rounding; agreeing in 75 % of the derivative images), so the original keeps the
  spec's rule.
- **The re-examination with global sides is gone**: the spec (§14.3 "The ties' spread", point 6)
  does not read "either side of every tie" as a global side (one point of the spread, not its
  extent); on the suite's cases and fMRIPrep's calls the revised requirements hold for every tie
  choice the spec writer measured, and the record of relay 4 needs no re-examination
  (its record from 88ecca2: 251 cases, 185 pass, 63 refused alike, 3 expected divergences (larmorx refuses AFNI's .HEAD/.BRIK format, twice, and AFNI-96's short attribute, rule B), none failing; ds000256's high-motion run's volumes at 0.681 of their limits, its median at 0.115 of its own, ds005454's and ds004958's medians at 0.0005 and 0.0016, `-final NN` without a voxel against the rule).

## Random testing: the ties on small, sharp phantoms; runaways

The descriptor (`validation/src/larmorx_validation/difftest/tools/afni_volreg_original.py`) draws
moving head phantoms (nested ellipsoids with noise, sharp edges) and blobs on grids of 12 to 32
voxels in-plane and 3 to 24 slices, every option. A check night (1,813 cases) and the trial
(`oracles/difftest/2026-11-08-volreg-original-trial`: 0.5 h, 4 jobs, 21,710 cases) agree on 11,385
and differ on 10,325 (48 %; 72 % of the head phantoms, 19 % of the blobs in the check night).

**The mechanism** (case 977 of the check night: blobs on 12 × 20 × 13 voxels of 3 × 3 × 3.3 mm,
the default heptic, converged in 2 to 3 iterations, the roll 0.085° from AFNI's): the first fit's
roll is 0.164 in AFNI and 0.102 to 0.106 in the original with every tie side, while the derivative
images' ranges agree within 0.4 %. The roll is weakly determined by such a phantom (nearly
collinear with the shifts on a small grid), so the derivative images' tie-dependent last
percent moves it by more than the fixed thresholds; AFNI's own result moves by only 0.002° when
`-delta` changes by 3 %, a smooth change of the same images. The spec writer measured AFNI's own
spread under the ties on such findings (AFNI's arithmetic with other tie choices at every
resampling): its parameters move by a median of 25 thresholds and up to 256 (spec §5.3).

**How the descriptor judges a case now** (the spec's §14.3 "The ties' spread", step 3 of
2026-11-08; `parity/afni_volreg_original.ties_judgement`): first the fixed requirements (the coarse
search's results reported, not required, in random testing); where they fail,
- **a runaway** (a volume beyond 15° or 20 mm in AFNI's run or in any of the original's runs, its
  default and its 12 seeded ones, or AFNI-97: AFNI's crash, the original's stop with `reproduce`)
  is held to the output's structure: when both end with exit status 0, the files written, their
  headers, types and dimensions and the messages with their numbers masked; an exit status that
  differs only by AFNI-97 is accepted; the numbers are reported;
- otherwise **the original's own spread**: 12 runs with `LARMORX_VOLREG_TIES=random:1` to
  `random:12` (`reproduce`; for the default against the replica's default, 12 by default), the
  largest difference between the original's default run and a seeded run in each requirement's
  units, and both comparisons made again with every requirement that follows the ties (the
  parameters and their median, `-rotcom`'s and the messages' parameters, the registered volumes'
  mean difference and NCC, `-verbose -verbose`'s derivative ranges) at the larger of its fixed
  value and 3 times the spread (9 times for 1 − NCC); the derived quantities (matrices, maximum
  displacements, `-savedisp`, rmsnew) keep their bounds.

**The trial's seeded sample of 1,200 differences, judged again with the final build**
(`triage.py`, relay 4):

| Verdict | Cases | What it is |
|---|---|---|
| agrees (fixed requirements) | 67 | fixed after the night (the notices, `-savedisp`'s slice fields, AFNI-97, AFNI-99's precision, `-SAVEDISP`'s line, AFNI-92's naming), or within the revised fixed requirements |
| within the ties' spread | 860 | beyond a fixed requirement, within 3 times the original's own spread |
| runaway | 268 | rotations beyond 15° or shifts beyond 20 mm, or AFNI-97; the structure as required |
| finding | 5 | below |

The five: three where the original names AFNI-99 and the replica does not (or the reverse): the
maximum displacement lies on a rounding boundary of its two-decimal message (case 5378: the
original 2.02 mm by default and 2.01 with `reproduce`, the replica 2.02 in both), so whether the
printed value changes with AFNI-99's correction follows the parameters' last digits, which the
ties decide (explained); two at the edge of 3 times the spread estimated from 12 draws (case
13852: the parameters 27.0 thresholds from AFNI's against a limit of 22.3; case 18762: the median
0.0516 against 0.0504), both within it when the spread is measured from 36 draws
(`triage.py --seeds 36`; explained: the spread is a sampled estimate, as the spec writer found
cases at its edge too, 6.90 against 6.89 thresholds).

**AFNI-97 in the original's trial**: the trial met the condition AFNI-97 describes (a Fourier rotation
shifting the last of an odd count of rows beyond the row with the previous pair's second shift):
AFNI's runaway first passes on small uint8 or int16 series with `-Fourier -twopass` (rotations of
hundreds of degrees), where AFNI crashes and the replica reproduces the crash as an error. The
original names AFNI-97 by default and stops with AFNI's error with `reproduce` wherever it meets
the condition itself; whether a run meets it is decided by the ties (the spec writer: in 2 to 12
of AFNI's own 16 tie variants), so random testing accepts an exit status that differs only by
AFNI-97 (a runaway, above).

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
- **The coarse search's weight percentage** (`-verbose`'s `w[...%]`) counts the automask's voxels,
  also those whose weight fell below 0.05 inside it (found by random testing: 96.3 % against 91.7 %
  on a head phantom; the weight itself was right).
- **`-savedisp`'s datasets** carry the input's slice fields as the registered output does.
- **AFNI-97** named by default and AFNI's stop with `reproduce` (above).
- **Defects named as the replica names them**: AFNI-98 (both messages; in both modes), AFNI-12
  when a `-prefix` AFNI refuses is written (3dvolreg checks the name only when writing, §10.1),
  AFNI-26 and AFNI-90 for a kept name of 256 bytes the system cannot hold, the option scan's
  notices (AFNI-86, -90, -95) before any later error, every notice in the order of its ID, AFNI-99
  only where a value the run prints or writes differs (`-maxdisp1D`'s three decimals only when the
  file is written, `-verbose`'s two only with `-verbose`).

- **AFNI-92 and the order of the notices** (relay 4, found by the trial's re-triage): AFNI-92 was
  named for every run with `-nomaxdisp`, `-twopass` and the coarse rotation search, also one that
  stopped before its first pass (a base of fewer than 100 non-zero voxels); it is now named when
  the first pass with the coarse rotation search starts, as the replica names it. The notices are
  printed in the order the run meets them, phase by phase: the option scan's and the setup's
  (AFNI-86, -90, -95, -98, ...), then AFNI-92, then the run's and the output's (AFNI-87, -89, -93,
  -94, -97, -99, ...), by ID within each phase (measured against the replica: with `-base 0 -base
  2` on a uint8 series AFNI-95 comes before AFNI-87; AFNI-92 after AFNI-86 and -95 and before
  AFNI-87, -89, -93 and -94).

## Other observations

- **A scale mismatch between the series and an external base** (a uint16 series four times the
  base's values): AFNI and the original both end with rotations of about 280° (no defect: 3dvolreg
  fits no intensity scale without `-twopass`'s initialisation, and is meant for small motions).
- **`-SAVEDISP`'s products**: §10.3's arithmetic as it first stood (the line of 3dTshift's fit)
  gave AFNI's three normalised datasets bit for bit on AFNI's own raw `-savedisp` values but its
  sixteen products differed at a few voxels (1 to 26 of 253,440 values on `s1`, 2 to 20 of 20,000
  in random testing); the spec writer found the cause in the line's coefficients (their double
  order of operations, now §10.3 step 1; the textbook forms change 3,983 of ds000005's
  normalised values) and the original follows it since relay 4: AFNI's nineteen datasets bit for
  bit on `s1` and ds000005 (`validation/investigations/3dvolreg-spec/savedisp.py --original`: 0
  values differ), required by the suite's step 1.
- **Inputs re-made for fMRIPrep's calls**: niworkflows' own lines in fMRIPrep 25.2.6's image give
  `_sliced` files byte-identical with the replica part's, and the median of AFNI's registered
  volumes, written as niworkflows writes it (the sliced input's header: int16 with nibabel's scale
  factors for an int16 run), is fMRIPrep's own `gen_avg` output byte for byte on all eight calls.
