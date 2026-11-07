# Findings: the clean-room head-motion correction against mcflirt on random phantoms

What the clean-room implementer of `crates/larmorx-mri`'s `hmc` measured about the differences
random testing found between `lx.mri.hmc` and FSL 6.0.7.17's mcflirt (2026-10-29). Black-box
only: both programs were run and their outputs read; nothing here comes from FSL's source or
the replica's (`crates-nc/`), so this file is **unrestricted** (the source-derived findings are
[fsl-mcflirt.md](fsl-mcflirt.md), which the implementer did not open).

## The differences of the first night (2026-10-25)

- **What:** on small random phantoms (4 to 8 volumes of at most 32 × 32 × 20 voxels, a head-like
  phantom moved by a random rigid motion in each volume, rotations of 5° to 30°), 782 cases
  gave motion estimates beyond the record's thresholds against mcflirt (the matrices' RMS
  deviation, the parameters, the corrected voxels, the RMS files, FD).
- **How it was measured:** every one of the 782 findings run again (not a sample): its shrunk
  inputs rebuilt, mcflirt and `larmorx mri hmc` (in `reproduce`, as the night compared, and by
  default) run on its arguments, and the corrected series' temporal standard deviation in the
  phantom compared (voxels of mcflirt's corrected mean above 10 % of its maximum; lower is
  better aligned): [`validation/investigations/cleanroom-fixes/hmc_alignment.py`](../../validation/investigations/cleanroom-fixes/hmc_alignment.py)
  (`--summary` prints the table below from its JSON-lines output).

| Subset | Findings | larmorx lower | within 1 % | higher | ratio larmorx / mcflirt, median (5 % – 95 %) | above 1.10 | below 0.90 |
|---|---|---|---|---|---|---|---|
| all | 782 | 162 | 458 | 162 | 1.000 (0.911 – 1.090) | 37 | 33 |
| `normcorr` (mcflirt's default, and given) | 524 | 77 | 384 | 63 | 1.000 (0.951 – 1.035) | 5 | 9 |
| all but `normmi` | 691 | 150 | 441 | 100 | 1.000 (0.909 – 1.048) | 11 | 30 |
| `corratio` | 52 | 20 | 21 | 11 | 0.998 (0.905 – 1.062) | 2 | 3 |
| `leastsquares` | 37 | 8 | 23 | 6 | 1.000 (0.945 – 1.022) | 0 | 0 |
| `mutualinfo` | 78 | 45 | 13 | 20 | 0.981 (0.809 – 1.089) | 4 | 18 |
| **`normmi`** | **91** | 12 | 17 | **62** | **1.033 (0.924 – 1.203)** | **26** | 3 |

(By default, the corrections of FSL-01 and others widen the spread, all findings: median 1.000,
5 % – 95 % 0.893 – 1.140. mcflirt's own corrected series has a temporal standard deviation 1.30
times smaller than the uncorrected input's, median.)

- **Conclusion, all costs but `normmi`:** the two programs align these phantoms equally well
  (691 findings: median ratio 1.000, larmorx the better aligned in 150 and the less in 100). With motions this
  large and phantoms this small the cost has several minima, and the two optimisers end in
  different ones, which is what the record's thresholds (made for real BOLD runs, where both
  agree within mcflirt's own band) count as a difference. The spec allows this
  ([specs/mcflirt.md](../../specs/mcflirt.md), "Where exact agreement will be hard"). Explained:
  the permanent case `difftest/phantom-several-minima` keeps one such finding as an expected
  divergence with this reason.
- **`-cost normmi`: aligns worse, cause open.** On the 91 findings with the normalised mutual
  information, larmorx's corrected series is the less aligned one in 62 (median ratio 1.033,
  26 above 1.10); with `mutualinfo`, the same histogram and entropies without the
  normalisation, it is the better aligned one (median 0.981). The cause was not found by
  black-box runs in this part: mcflirt's `-verbose 20` trace prints the matrices it evaluates
  but no cost values, and the spec describes the histogram costs only in outline (its open
  question 4: appendix A). Left open: the permanent case `difftest/normmi-aligns-worse`
  (status open) keeps the worst finding (ratio 1.38). Next step: the spec writer describes
  `normmi`'s histogram, its fuzzy binning and the entropy correction exactly (and whether
  mcflirt's normalised cost is `(H1 + H2)/H12` or another form), then a clean-room fix
  measured with the same tool.

## FSL 6.0.7.23 and the exact histogram costs (2026-10-29)

What the clean-room implementer who aligned `hmc` with FSL 6.0.7.23's mcflirt measured, by
black-box runs of mcflirt (`~/fsl-6.0.7.23/bin/mcflirt`), the replica program and larmorx
([`validation/investigations/cleanroom-align/`](../../validation/investigations/cleanroom-align/)).

- **`-cost normmi` and `mutualinfo` exactly (appendix B of the spec).** The costs now follow
  the spec's appendix B step by step: float32 histograms filled in the sampling order, the test
  bins over the whole test volume, the fuzzy weights, x86-64's conversions to integers, the
  entropies over every grid point corrected to the overlap. **Evaluation by evaluation**, with
  `-fudge -stages 1` (`mi_trace.py`: mcflirt's `-verbose 20` trace against a traced build of
  larmorx): every evaluated matrix is mcflirt's, to its printed 6 decimals, on the synthetic
  series of the recorded runs (`normmi` 429 of 429, `mutualinfo` 422 of 422) and on 22 of 30
  random phantoms (11 of 15 per cost); the other 8 agree for 19 to 224 evaluations, then differ
  in the 5th or 6th decimal of a translation, as `normcorr`'s path does (spec §17, open question
  1: the search's float32 last bits, shared by every cost). **On the 169 findings of the first
  long random-testing run** (2026-10-25; larmorx in `reproduce` against mcflirt 6.0.7.23, the
  corrected series' temporal standard deviation, larmorx / mcflirt; `hmc_alignment_bin.py`):

  | Cost | Build | larmorx lower | within 1 % | higher | median (5 % – 95 %) |
  |---|---|---|---|---|---|
  | `normmi` (91) | before | 10 | 13 | 68 | 1.051 (0.929 – 1.330) |
  | `normmi` (91) | after | 17 | 55 | 19 | 1.000 (0.957 – 1.057) |
  | `mutualinfo` (78) | before | 43 | 11 | 24 | 0.983 (0.809 – 1.208) |
  | `mutualinfo` (78) | after | 10 | 54 | 14 | 1.000 (0.919 – 1.046) |

  So `normmi` no longer aligns worse: the open item of the earlier part is closed (the
  permanent case `difftest/normmi-aligns-worse` is re-described from this measurement).
- **The in-plane mode's centre of rotation is the padded volume's** (spec §12 does not say).
  On a thin slab (finding 00298, 26 × 15 × 6 voxels of 3 mm) mcflirt's second evaluated matrix
  (the first rotation step) had another translation than larmorx's: the rotation was about
  another centre. With the centre of mass of the test volume as the in-plane mode pads it (one
  copy of the first and last slices, §12), 154 of 154 `normmi` evaluations agree (`normcorr`
  on the same series: 39 identical instead of 1). Both modes use it; the spec writer may add it
  to §12 as observed.
- **FSL 6.0.7.23's I/O** (`fsl_io_probe.py`): every `FSLOUTPUTTYPE` writes the files mcflirt
  writes (zstd and bzip2 single files and pairs included), whose decompressed headers and values
  read back alike; files are read by their content (a bzip2 or zstd file named `.nii` or
  `.nii.gz`, a gzip file named `.nii.zst`); a name without an extension is completed in FSL's
  order, with FSL's errors for two matches and for none; an unknown `FSLOUTPUTTYPE` prints FSL's
  message (larmorx before any work; mcflirt after the estimation and the `.mat` and `.par`
  files); FSL-60 (an image name holding `.img`): with `reproduce`, larmorx reads and writes the
  files mcflirt 6.0.7.23 does in the 11 probed cases (a pair's header under the renamed name,
  its data file under the name given; the `.mat` directory and `.par` never renamed). FSL-59:
  `-sinc_final` and `-nn_final` give the default run's series with `reproduce`, as mcflirt
  6.0.7.23's do, and are named where the two series differ.
- **Random testing against FSL 6.0.7.23** (a trial of 0.5 h, 2,826 cases, 2026-10-29): 1,732
  phantoms beyond the record's thresholds, as well aligned as mcflirt's for every cost (median
  ratio of the temporal standard deviations 0.999 to 1.001 per cost; `normmi` 1.085 before this
  part): the several-minima class of the first long run. One case names FSL-01 in `reproduce`
  only (`mri-hmc:20261029:45`: `-cost corratio` on a thin slab, where the default's FSL-03
  correction leaves the previous volume's matrix at the identity, so FSL-01 changes nothing
  there): reported, not changed in this part
  ([`validation/investigations/cleanroom-align/README.md`](../../validation/investigations/cleanroom-align/README.md)).

## FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original (2026-10-30)

Written from `specs/mcflirt.md` §10.6, §12.1 and §14.1 (the spec writer's lines of 2026-10-30)
and black-box runs of FSL 6.0.7.23's mcflirt, by a clean-room implementer who opened none of
FSL's source, the replica crates or the restricted findings. The cases of §14.1 are in the
`mri-hmc` suite (`failures/<case>`, `validation/src/larmorx_validation/parity/mri_hmc_failures.py`:
mcflirt live, larmorx in both modes); the probes are in
[`validation/investigations/cleanroom-additions/`](../../validation/investigations/cleanroom-additions/README.md).

- **Every case of §14.1 as the spec says** (33 cases on a 24 × 24 × 16 × 5 phantom, mcflirt
  6.0.7.23): mcflirt's exit status (0, signal 6, signal 8) as observed by the spec writer; with
  `reproduce`, larmorx's exit status (mcflirt's, 1 where mcflirt ends by a signal), mcflirt's
  first line of standard error (`Error!: Unknown Log directory return value`, `Error! Could not
  create directory: <-in's directory><out>.mat. No write permission`, `error: unable to open
  output file!`, the `terminate` lines of an image that cannot be written, `double free or
  corruption (out)`, the `std::bad_array_new_length` lines) and the files written, name for name
  (`MAT_0000` and the `.rms` files where the `.par` file cannot be opened; the two `.rms` files
  that can be where two cannot); by default, exit 1 and nothing written, the defect named.
- **What the spec left to the implementer, settled black-box:** mcflirt prints the permission
  message and the `terminate` lines with the names as given (`-out ro/o`: `ro/o.mat`,
  `ro/o.nii.gz`); with `-report` the FSL-20 message follows `Registering volumes ...` and the
  volumes' indices on the same line. In the default mode the `.mat` directory mcflirt would make
  for `-rmsrel` or `-rmsabs` without `-mats` (FSL-13 corrected: none is made) is checked, so a
  missing parent names FSL-20 there too (with FSL-13), as §14.1's `fsl20-rmsrel` asks.
- **Few bins, measured:** with no bins in a stage (`-bins` below 8) `mutualinfo`, `normmi` and
  `woods` run in `reproduce` as appendix B's rules give with `B = 0`; their cost is rounding noise,
  and so are their matrices against mcflirt's (reported, not required: 95th-percentile RMS
  deviation 3.1 mm for `normmi -bins 0`, 4.9 to 7.4 mm for `mutualinfo`, 74 mm for `woods -bins
  1` on the phantom; mcflirt's own are nothing like its default's, §12.1). With one bin in stage 1 (`-bins 8` to 15) the mutual
  information is 0 but for rounding as well: on the phantom 3 of the 4 registered volumes are
  identical to mcflirt's as text after stage 1, the others 0.4 and 2.1 mm away, and after three
  stages up to 1.4 mm; from 16 bins on the two agree within 0.07 mm (`-bins 16`, 32, 64; probe
  `hmc_bins_matrices.py`). So `failures/bins8-mutualinfo-s3` reports its matrices; `corratio`
  with 8 bins agrees within the record's thresholds (0.07 mm).
- **Random testing after the additions** (a trial of 0.5 h, 2026-10-30, larmorx `6f64291`, FSL
  6.0.7.23, `oracles/difftest/2026-10-30-cleanroom-additions`): 2,411 cases, 882 agree, 1,528
  beyond the record's thresholds, the several-minima class of the earlier nights (failed checks:
  the matrices' RMS deviation in 1,105, the `.rms` values, the corrected voxels, FD), as well
  aligned as mcflirt's: the corrected series' temporal standard deviation, larmorx's over
  mcflirt's, median 1.000 (5 % – 95 %: 0.929 – 1.047), larmorx lower in 312, within 1 % in 973,
  higher in 243 (`../../validation/investigations/cleanroom-fixes/hmc_alignment.py`; the per-finding
  results in [`validation/investigations/cleanroom-additions/results/hmc-alignment-trial-2026-10-30.jsonl`](../../validation/investigations/cleanroom-additions/results/hmc-alignment-trial-2026-10-30.jsonl)).
  One case names FSL-01, FSL-09 and FSL-59 by default only (`mri-hmc:20261030:1778`: `-cost
  mutualinfo -bins 128 -nn_final -stages 1` on a thin slab, where FSL-03's correction changes the
  first stage's path), the class of `mri-hmc:20261029:45`: reported, not changed in this part.
  The random draws use 64 to 256 bins and write into existing directories, so they meet none of
  this part's four defects; those are the suite's `failures/` cases.

## `corratio` and `woods` exactly (2026-11-02)

`specs/mcflirt.md` appendix A now says (the spec writer, 2026-11-02, from FLIRT's questions) that
mcflirt's `-cost corratio` and `-cost woods` are FLIRT's two costs unchanged (`specs/flirt.md`
§6.2): the points of a bin with at most two (weighted) points are left out of the count and the
total variance too, not only of the within-bin sum; the cost is 1 (not 0) when no bin has more
than two points or the total variance is not positive; Woods' term is `n_b·σ_b/μ_b` (the
appendix's earlier `n_b²·σ_b/μ_b` was `n_b` times too large), `n_b²·σ_b` for a bin whose sum is
not positive; and the sums are single-precision running sums per bin in the sampling order with
two final double-to-single roundings. `lx.mri.hmc` uses FLIRT's implementation of them
(`hmc::cost::bin_moments`, `corr_ratio_of`, `woods_of`). The standard tier re-made from 04b58ca
(`docs/validation/mri-hmc.md`): the same statuses (145 pass, 20 rejected by both, 3 expected, 1
failure, `real/ds003345/default`, as before), and only the four cases of these costs changed, each
closer to mcflirt:

| Case | Before | After |
|---|---|---|
| `real/ds000005/opt-cost-corratio` | RMS deviation p95 0.0662 mm, FD r 0.578, 0/240 matrices identical | 0.0284 mm, FD r 0.864, 1/240 identical |
| `real/ds000005/opt-cost-woods` | "no parameter is identifiable" 48.21 / 50.64 | 13.42 / 2.65 |
| `failures/bins1-woods` (`reproduce`) | RMS deviation p95 73.82 mm | 3.64 mm |
| `failures/bins8-corratio` (`reproduce`) | RMS deviation p95 0.0717 mm | 0.0416 mm |

## glibc's `logf` in `reproduce` (the glibc maths part, 2026-11-04)

With `reproduce` the mutual-information costs now take glibc 2.35's `logf` (and the no-overlap
`log`) where `specs/mcflirt.md` names them (`larmorx_core::math::Libm`; the default keeps the
correctly rounded functions; the rotations' sines and cosines are not wired: the spec does not
name glibc's there). The standard record, made again from `be3db91` (serially: the report's
tables come from per-case metrics that `--jobs` workers do not return): every status as before;
`real/ds000005/opt-cost-mutualinfo` 0.0048 to 0.0047 mm (the 95th percentile of the RMS
deviation), `opt-cost-normmi` 0.0108 to 0.0126 mm (threshold 0.1), `difftest/normmi-aligns-worse`
(an expected divergence) a median 0.0214 to 0.0200 mm, and the `failures/` cases' reported
`reproduce` deviations 7.35 to 7.34, 1.15 to 0.97 and 4.88 to 3.18 mm. `normmi`'s move away from
FSL is the size of the fit's own sensitivity to the C library's last bits: the default against
`reproduce` is 0.0109 mm (`validation/investigations/cleanroom-glibc-dbl/record_moves.py`); the
default against FSL stays 0.0108 mm, main's number. The interpolation and the sums are not FSL's
bit for bit, so the paths still part.

## The centres of mass as mcflirt's blocked sums, its COG warning and `atan2` (the clean-room catch-up, relay 7, 2026-11-05)

Both of mcflirt's centres of mass, each test volume's (the rotations' centre) and the reference's
(the `.par` file's), are now FLIRT's blocked sums (`specs/mcflirt.md` §6.1, the spec writer's
answer of 2026-11-03; `hmc::volume::Volume::centre_counted`): `v − v_min` in single precision, double
running sums over the voxels in z, y, x order flushed into the totals every `max(1000, trunc(√N)) +
1` voxels, a total below 1e−5 in magnitude counted as 1. Before, one running sum.

- **What it changes: nothing on the record's inputs.** `centre_bits.py` computed every volume's
  centre both ways: the nine real runs are integer images (int16, uint16, int32, no scaling), whose
  weighted sums are exact in double either way (ds000005's 240 centres identical), and the record's
  synthetic series give 0 differing centres too (their values sum exactly as well). The blocks matter
  only where the sums round, as on the golden cases' random float series; even there no golden
  checksum of the head-motion correction moves with the blocks alone (the main build, the blocks
  alone and the final build compared on the nine golden cases: the two that move, `py.mri.hmc` and
  `py.mri.hmc.options`, move with `atan2`, below).
- **mcflirt's COG warning** (`WARNING::in calculating COG, total = 0.0` on standard error, §6.1):
  `cog_warning_probe.py` ran mcflirt 6.0.7.23 (through the oracle cache) on small series holding
  constant volumes: it prints the line once per registration of a constant test volume (three with
  the default three stages, four with `-stages 4`, six with `-meanvol`'s two passes, three with
  `-fudge`), once per `.par` line when the reference is constant (`-plots`: one per volume), and
  never for a constant reference without `-plots` (its centre is measured only for the `.par`
  file). The original now prints it as often (`Estimate::cog_warnings`, `HmcOutput::par_cog_zero`):
  8 of 8 calls alike. The suite does not compare messages; the unit test
  `cog_warnings_per_registration` holds the count.
- **`atan2`** in the decomposition is `larmorx_core::math::atan2` (correctly rounded, double),
  the `libm` crate's before (the libm crate's last uses, `docs/findings/platform-math.md`): it
  differs from the old one in the last bit of 25 % of near-identity rotations; the start parameters
  of each registration carry it. mcflirt's own is glibc's single-precision `atan2f` (`specs/flirt.md`
  §3's decomposition, which §4.4 shares), for a later part's `reproduce`.
- **The record** (`mri-hmc`, standard tier, `--jobs 1`, from 0842fbd): 169 cases, 145 pass, 20 rejected by both, 3 expected divergences, 1 failure (`real/ds003345/default`, as main's); against main's record (`compare_records.py`) no status and no detail differs. Below the record's printed precision, `centre_moves.py` ran fMRIPrep's call on four real runs (ds000005, ds000117, ds005040, ds006010) and `-cost normmi`, `-meanvol` and `-dof 12` on ds000005 with main's build, the blocks alone and the final build (`reproduce`, the `.par` file added): the 1,344 matrices and `.par` lines are the same text in all three.

## The catch-up's random-testing trial (relay 8, 2026-11-05)

`oracles/difftest/2026-11-05-catchup-hmc` (seed `20261030`, the 2026-10-30 trial's drawn again and
further; 30 minutes, 12 jobs at one thread, 3 GiB a case, the oracle cache in `use` mode): 83,244
cases, 31,552 agree, 51,681 differ, 11 notices (FSL-01, -09, -59). A first run of the same trial
overlapped two rebuilds of the extension (its numbers, kept in `...-overlapped-rebuild`, are not
used). The registrations of small phantoms with large motions end elsewhere than mcflirt's, as in
the earlier trials (37 % agreeing on 2026-10-30, 38 % now), and are as well aligned:
`validation/investigations/cleanroom-fixes/hmc_alignment.py` on 400 findings (the corrected series'
temporal standard deviation in the phantom, larmorx over mcflirt) gives in `reproduce` mode, as the
trial compares, a median of 1.000 (5 and 95 % quantiles 0.933 and 1.041; 279 within 1 %, 67 lower,
54 higher; mcflirt's own residual against the input's 1.41). The default (FSL-01, -03 and the other
corrections on) has a longer tail: median 1.002, above 1.05 in 102 of the 400 (`reproduce`: 18), a
question for the head-motion correction's next part.

6,363 findings were the harness's: a run with `-rmsrel` or `-rmsabs` without `-mats` makes
mcflirt create an empty `.mat` directory (FSL-13, which `reproduce` follows), and the oracle-output
cache keeps a run's files, not its empty directories, so the trial's replays from the cache lacked
it (the first run, mcflirt live, had no such difference). The descriptor runs those mcflirt calls
live now (`difftest/tools/mri_hmc.py`'s `makes_empty_directory`;
`validation/tests/test_mri_hmc_descriptor.py`).

## The count per point and `leastsquares` as FLIRT's (PRELUDE's clean-room part, relay 3, 2026-11-07)

**The count.** The spec writer's answer of 2026-11-06 (`specs/mcflirt.md` §6.4, from FLIRT's record:
`specs/flirt.md` §6.4): the count `N`'s first running total `C` takes each contributing point's
weight as the point is visited, over the whole grid; at every row's end `C` is added to `A`, at
every slice's end `A` to `N`. The original added each row's total at the row's end, which rounds
otherwise where the weights are fractional (the edge factors within the smoothing distance of the
test volume's bounds). Now each slice keeps its points' weights in visiting order (runs of unit
weights as their lengths, the other weights as they are), the slices are sampled in parallel in
groups of four per thread and replayed in slice order (`hmc::cost::sample_fold`,
`CorrTotal::add`, `add_ones`): the same bits at any thread count. **Black box**
(`validation/investigations/cleanroom-align/count_trace.py`: mcflirt 6.0.7.23's `-verbose 20`
through the oracle cache against two traced builds, `reproduce`): the per-point count follows
mcflirt's evaluated matrices longer on every recorded run traced: the first matrix that differs
from mcflirt's comes at evaluation 39 instead of 3 (`syn/geometry/single-volume/reffile`), 41
instead of 9 (`ras-unflipped/reffile`), 33 instead of 8 (`ras-unflipped/default`), 31 instead of
23 (`motion/small/default` and `meanvol`), 35 instead of 7 (`motion/small/reffile`), 39 instead
of 7 (`motion/small/dof12`); `syn/content/ramp/reffile` parts at its second evaluation in both
(48 matrices identical instead of 3). Where the paths still part after some 30 to 40 evaluations
is open (the line search's last bits, as before). Speed: on ds000005's fMRIPrep call 16.6 s
against 15.7 s at one thread (a loaded machine), 2.05 against 2.13 s at 14.

**`leastsquares`** (`specs/mcflirt.md` appendix A, the spec writer's answer of 2026-11-06): FLIRT's
`leastsq` unchanged: `(w·δ)·δ` and `w` (with `-smooth 0` `δ·δ` and the count) in three
single-precision levels (row, slice, total), the quotient in single precision, `(max − min)²` of
the two volumes' finite values when the weight (count) is not above 1; the original summed in
double precision. Unit test `hmc::cost::tests::least_squares_sums_three_levels`.

**The record** `mri-hmc` re-made (`--jobs 1`; first from `f005a08`: 170 cases, 145 pass, 20
rejected by both, 3 expected divergences, 2 failures; before: 169, 145, 20, 3, 1). The matrices identical
to mcflirt's as text: 231 of 11,476 (54 before); the 95th percentile of the matrix RMS deviation
lower in 65 cases, higher in 42, the same in 10 of 117 (median ratio 0.98); ds000005's default
run 35 of 240 matrices identical (1 before). `real/ds003345/default` passes (it failed: p95
0.0279 mm, now 0.0235), and the new permanent case (`time-step-negative-series`, the time step as
read) passes; two synthetic cases near their thresholds now miss them: `syn/content/ramp/reffile`
(a ramp, which only an x translation can align: that translation 0.342 mm from mcflirt's, 0.181
before, threshold 0.3; the other parameters are not constrained) and
`syn/orientation/ras-unflipped/reffile` (p95 0.110 mm, 0.066 before, threshold 0.1). Both
follow mcflirt's path longer than before (above) and part later: open, with the parting point
(at the 41st evaluation of `ras-unflipped/reffile` the printed matrices differ in the sixth decimal
of one translation: the parameters had already parted below the printed digits, in the line
search's arithmetic). The suite counts these two as expected divergences with their reasons
(`parity/mri_hmc.py`'s `PATH_DIVERGENCES`: only when every failed check is one of the estimates'
metrics; every check still runs and is reported), as it counts `difftest/phantom-several-minima`;
the record is re-made with it (`docs/validation/mri-hmc.md`: 145 pass, 20 rejected by both, 5
expected divergences, 0 failures expected).
`real/ds000005/opt-cost-leastsquares` passes (p95 0.0227 mm, 0.0220 before).
