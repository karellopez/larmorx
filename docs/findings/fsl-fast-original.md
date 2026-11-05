# FAST's clean-room original: what black-box runs of FSL showed

Observations of FSL 6.0.7.23's `fast` (`~/fsl-6.0.7.23/bin/fast`, run as a black box through the
oracle cache) made while writing the clean-room original (crate `larmorx-fsl`, module `fast`)
from [`specs/fast.md`](../../specs/fast.md): behaviour the spec does not state, or states
otherwise. Nothing here comes from FSL's source. The tools are in
[`validation/investigations/cleanroom-fast/`](../../validation/investigations/cleanroom-fast/README.md).

## The method: FSL with correctly rounded maths

FSL's program with the workspace's CORE-MATH preload (`LD_PRELOAD` of
`oracles/core-math-preload/libcrmath.so`: correctly rounded `exp`, `log`, `expf`, `logf`, the
functions FAST calls) computes FAST's arithmetic without glibc's last-bit errors; the original
uses correctly rounded functions too (`larmorx_core::math`). Where the original follows FAST's
arithmetic exactly, its verbose output equals that run's digit for digit
(`verbose_cr.py`, `files_cr.py`). This separates "a different arithmetic" from "glibc's last
bits", which the comparison with FSL as built cannot.

## The class variances (§4.4): the second moment stays in double

`specs/fast.md` §4.4 rounds both quotients, `Σ p y / Σ p` and `Σ p y y / Σ p`, to single
precision before `σ² = Σ p y y / Σ p - μ²`. With the second moment rounded to single, the
variance of log intensities (about 0.003 to 0.2 at means of 6 to 7) moves in steps of one unit
in the last place of `μ² ≈ 40` (3.8e-6): FSL's printed standard deviations (`-v`) fall between
those steps. Of the eight combinations of single and double for the second moment, the mean and
its square, only one reproduces FSL's verbose lines exactly (with correctly rounded maths, on
`t1_half`, one to three initialisation iterations, `-N`): the mean rounded to single, its square
in single, **the second moment's quotient in double**, the difference rounded to single
(`variance_model.py`; the original since 2026-11-03). Spec question
for the handoff: §4.4's "the quotients are rounded to single precision" holds for the mean only.

## The bias field's kernel (§4.5): the normalised taps stay in double

`specs/fast.md` §4.5 divides the taps (each `exp(-j² / (2σ²))` rounded to single) by their
single-precision sum "as a multiplication by the double-precision inverse of the sum". Rounding
the normalised taps to single again moves 0.05 % of one bias field's voxels (0.37 % of the mask's
on `t1_half`) by one or two units in the last place against FSL with correctly rounded maths, and
over the main loop's eight estimates that grew into segmentations outside §11's thresholds (Dice
0.995 of `_mixeltype` on the values above 2^24, `_pve` 99.9th percentiles up to 0.24 on the MNI
T1w at 4 mm). The cause was isolated with `-l 5` on `t1_half` (2 x 2.67 x 2.67 mm: a 5-tap kernel
along x, one tap along y and z): `-l 3` (one tap everywhere) is bit-identical, `-l 5` is not, so
the per-voxel `W` and `R`, the quotient, the mean and the outputs are FSL's and the convolution
is not. Against FSL's `_bias` (`conv_model.py`, numpy, from the original's `W` and `R`): the
taps rounded to single 98.12 % of the mask's voxels identical; a double accumulator, the taps in
reverse order, single-precision products, unnormalised taps with or without a division
afterwards, every tap one unit in the last place up or down: all fewer; **the taps rounded to
single, times the double inverse of their single sum, kept in double: 100 %**. With the taps in
double the original's bias field, restored image, segmentations, partial volumes and
probabilities on `-v -g -p` and `-b -B -v` are bit-identical to FSL with correctly rounded maths,
and the six cases that were outside the thresholds pass (2026-11-03). Spec question for the
handoff: §4.5's normalised taps are double-precision values.

## `MRFWeightsTotal` (§4.10)

`-v`'s `Tanaka-inner-loop-iteration=<k> MRFWeightsTotal=<T>` (k from 0 to 4) is computed after
each sweep, over the posteriors as they stand then: the first sweep's total is already within
0.3 % of the last (305,647 against 306,411 on `t1_half`), which a total accumulated during the
sweep, whose neighbours ahead still hold the random values, is not (244,454). With the total after
the sweep the original prints FSL's numbers.

## The mixel types (§4.8)

- **The sweep of iterated conditional modes keeps a voxel's type when no type's value exceeds
  the running maximum's start (-1)**: with `-R nan` every value `L_t · exp(R K_t)` is NaN, and
  FSL's `_mixeltype` is the initial one (identical in 99.96 % of the mask's voxels to the
  original's with this rule; 14.6 % with "type 0 when none wins").
- **The initial type is 0 when no likelihood exceeds -1** (all NaN): `-I 0 -O 0 -W 0 -n 2` (the
  bias field estimated with variances of 0, every value NaN) gives every voxel class 1 (`tissue 1
  1.35391e+06`, `tissue 2 0`).

## The command line and the messages

- The usage text ends with the `-p` line and two empty lines (`maps\n\n\n`); an option error adds
  an empty line and the message (`\n--bogus: Option doesn't exist!\n`).
- `-gp` is `-g -p`; `--class 2` is an error (the value only after `=`); `-n 0x2 -I 010` reads 2
  and 8 (C's base 0).
- An unknown `FSLOUTPUTTYPE` is reported after the whole computation, after `-v`'s tissue volumes
  (nothing is written), exit 1.
- `-v`: the initial means (`<c> <μ_c>`) are printed before the k-means exception (a constant
  image: `1 4.61512`, `2 4.61512`, `3 4.61512`, then `Exception: Not enough classes detected to
  init KMeans` on standard output, exit 255). With `--nopve` there is no ` Starting Partial Volume
  Estimation ` line; the tissue volumes follow with their empty line.
- **FSL-100 and `-A`**: FSLDIR unset with `-A` (without `-a`) segments normally (exit 0): only
  without `-A` does FAST abort. The original follows that.
- `-N`'s main loop prints its `Tanaka Iteration <i> bias field <I>` lines all the same.

## The restored image (`-B`)

At the voxels FSL reads as NaN, `_restore` holds 0; a voxel outside the mask that is not NaN
keeps its value (the maximum 2^25 of `t1_big`, outside FSL-103's mask, is written as it is). The
bias field there is 1.

## FSL's own last-bit band (§11)

FSL against FSL with correctly rounded maths (`band.py`): on fMRIPrep's call of ds000005 every
output within §11's thresholds (`_mixeltype` Dice 0.99998, `_pve` mean differences 2e-6 to 5e-6);
on ds005003 (a glioma) `_mixeltype`'s rarest type (the CSF and white-matter mixture, 8,700
voxels) has a Dice of 0.99800, below §11's 0.999; on the derived 2D slice `_seg` 0.99914 and
`_mixeltype` 0.99761; on the values above 2^24, `_mixeltype` 0.99886. So §11's thresholds are
narrower than FSL's own band on small classes and small images: the original's allowance there is
the band (its suite checks the original against that run where a check fails against FSL, and
says so). On these cases the original lands on the band's value itself: on ds005003 its
`_mixeltype` Dice against FSL is 0.99800 too, on the 2D slice its `_mixeltype` and `_pveseg` are
identical to FSL with correctly rounded maths.

The single-channel variance is FSL's numerical weak point: `σ² = Σ p y y / Σ p - μ²` in the log
domain cancels most digits (μ² about 40 against σ² about 0.003, and with values above 2^24, μ²
about 265 against σ² about 0.002), and a last-bit change of a posterior or a log moves the
variance noticeably; that is why the band is wide where classes are small. A candidate for the
register (rule D: measure whether a two-pass or double-precision variance changes the
segmentation beyond the band before deciding), raised in the handoff.

## Several channels (§5): what the arithmetic is (relay 2)

With the default `-S 2 -B -b` on the MNI T1w and T2w at 4 mm and `-S 2 -n 4` at 2 mm the
original is bit-identical to FSL with correctly rounded maths inside the mask (`files_cr.py`,
`verbose_cr.py --full`, each step isolated with `-N`, `-I 0 -O 0`, `-W 0/1/15`, `-l 1` for a
one-tap kernel). Three conventions decide it where `specs/fast.md` reads otherwise:
- **The bias field's weights** (§5.6): `t = q_c / C_c(i,i)` is the double quotient rounded to
  single, `W` sums the `t` in single, `R` adds each `t (y_bi - μ_ci)` in double to its single
  sum. With `t` kept in double (the spec's reading) 97 % of a one-tap field's voxels are
  identical, 99.2 % with FSL's kernel; with the convention above 100 %. Of seven variants
  (double sums, single residuals, a quotient `q (y - μ) / C`) only this one reproduces FSL.
- **The initial hard assignment** (§5.2): the products `(y_i - μ_ci)(y_j - μ_cj)` in double,
  summed into a single-precision value (the spec: single precision). Single-precision products
  move a few voxels' class (8 of 200,000 at 2 mm), and every later statistic with them.
- **`-v` without partial volumes**: several channels print no tissue volumes with `--nopve`
  (one channel prints the posteriors' volumes).
Spec questions for the handoff: §5.6's weights in single precision, §5.2's products in double,
§5.6's `-v` without the volumes under `--nopve`.

The messages of several channels, observed: the means `<i> <c> <μ>` are printed channel by
channel as they are computed, so with FSL-104 (channel 1 smaller than the others, no `-s`) FSL
prints channel 1's lines and then aborts at channel 2's percentile (`Image Exception : #3 :: mask
and vol have different sizes in calc_percentiles`); `-S 4` with two images reads channel 1, 2,
then 3 (the word `4`: `No image files match: 4`); a singular covariance (the same image twice)
aborts with `std::runtime_error`, `inv(): matrix is singular`, without any other line; `-S 2 -n 0`
writes `_seg`, `_pveseg` and `_mixeltype`, all 0.

## Five channels: an ill-conditioned covariance (`multi/s5`)

`-S 5` with the T1w + T2w sum, the T1w/T2w ratio, the BOLD reference, the T2w and the T1w: the
first channel is nearly a combination of the last two, so the class covariances are nearly
singular and their inverses amplify the last bits of the matrix arithmetic. FSL against FSL with
correctly rounded maths agrees there (Dice 0.99993: the functions it changes do not touch the
matrices), but the original against itself with one rounding changed in the covariance's
products (`q (d_i d_j)` instead of `(q d_i) d_j`: `self_band.py` with `self_band.patch`) moves
further (`_mixeltype` Dice 0.865, `_seg` 0.975) than it lands from FSL (0.907, 0.982). The
members after the initialisation's first statistics (`-W 0`) differ from FSL's by 5e-5 in 30 % of
the voxels already: the 5 x 5 inverse (FSL: LAPACK's LU and inverse, OpenBLAS's kernels for the
quadratic form above four channels) against larmorx's LU. The result of such an input is
decided by rounding, not by the data: the record reports the case outside §11's thresholds with
this reason (two, three, four and eight channels on the same images pass). Matching it would need
LAPACK's and OpenBLAS's exact operation order, which `specs/fast.md` §8.6 leaves to the replica.

## FSL's standard priors: how FSL reads an ANALYZE image

`-a` without `-A` registers `$FSLDIR/data/standard/tissuepriors/avg152T1_{csf,gray,white}`:
big-endian ANALYZE 7.5 pairs, uint8, voxel sizes `-2 2 2`, a scale field (`funused1`, where NIfTI
keeps `scl_slope`) of 1/255. FSL's `flirt -applyxfm` of the CSF prior with the identity
(`analyze_scale.py`, through the oracle cache) writes uint8 0 to 217, and with `-datatype float`
float 0.0 to 217.0: **FSL does not apply the scale field**. And it samples the prior **as
stored, without the x reversal** it applies to other ANALYZE images (`specs/bet.md`'s
observation): a numpy model of `-applyxfm` (`std2mni.mat`, trilinear, truncated) reproduces
FSL's registered priors exactly with the reference reversed and the prior not (100 % of the
voxels), and 75 % with both reversed (what larmorx's reader did). The negative product of the
stored voxel sizes is the difference. larmorx's ANALYZE reader (`bet::image::read_analyze`,
shared by BET's and FLIRT's clean-room originals) now does both; FAST's registered priors, and
with them `priors/a-verbose`'s whole segmentation, are bit-identical to FSL's. For the
orchestrator: this changes BET's and FLIRT's originals on ANALYZE inputs with a scale field or
negative voxel sizes (neither suite holds one).

## FLIRT's `-applyxfm` output: `pixdim[4]`

With `-A` (the template's float priors, `pixdim[4..7]` 0) and the identity, FSL's registered
priors carry `pixdim[4] = 0` (the input's) and `pixdim[5..7] = 1` (the reference's); larmorx's
FLIRT wrote the reference's `pixdim[4] = 1` for a 3D input (`priors/A-a`: every image of the
segmentation bit-identical, the header check of the three `_stdspace` images failed). Fixed in
FLIRT's clean-room original in relay 3 (the apply path writes the input's TR, a 3D input's too:
black-box evidence in [`fsl-flirt-original.md`](fsl-flirt-original.md), "`-applyxfm`'s TR").

## `-H nan` (FSL-99 with `reproduce`)

One channel (`-H nan -v` on the MNI T1w at 4 mm): in each `-O` iteration, after the sweeps and
the class line, FAST prints ` TOTAL <T> RHS 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ` on standard output
(also without `-v`; `T` the last sweep's `MRFWeightsTotal`, fifteen zeros each followed by a
blank) and fourteen lines `0.05 0.05` to `0.7 0.7` on standard error, then ` BETA 0`: the
estimate of β against a table of zeros is 0, and the next sweeps use `beta=0`. The original
does the same. Several channels print none of these lines and take a β from memory FAST never
writes (0.2 in one run, `never`-cached); the original takes the table as zeros (β = 0) and names
it (FSL-99).

## `ERROR:: Empty mask image` (§8.3)

An image without a voxel above 0 and `-s` means (`-s t1_means.txt` on the zero image): FSL prints
the message once per masked sum over the empty mask, on standard error: 4 per class for each
class statistics (the initialisation's `-O + -W` and each main-loop iteration's), 1 per bias
field estimate, and per class for the tissue volumes (`n` with `--nopve`, `5 n` with the partial
volumes' statistics and volumes). Nine runs (`-I`, `-O`, `-W` 0 or 1, `-N`, `--nopve`) fit this
count exactly (3, 4, 16, 17, 28, 65 and 344 messages); the original prints the same number. With
`-a` and a matrix FLIRT cannot read on the zero image FSL runs on (FSL-101 with `reproduce`):
`Beginning prior-based initialisation`, the class lines all `-nan`, the messages after FLIRT's
and the prior message (their count there follows the same rule, from the priors' one class
statistics; not counted from that run).

## Random testing: what the short trial showed (relay 4, 2026-11-03)

One short trial of the descriptor `fsl-fast-original` (seed `cleanroom-fast`, 25 minutes, 3 jobs,
9,005 realistic cases of the replica's draw, FSL's runs through the oracle cache in `use` mode;
`oracles/difftest/2026-11-03-cleanroom-fast/`): 8,823 agreed, 182 differed. Every difference was
replayed (`validation/investigations/cleanroom-fast/replay_findings.py`, the cache serving FSL's
runs) and looked at with `probe.py --finding` (both runs kept, streams diffed):

- **The bias field outside the mask** (13 cases, 14 on replay; each with a class of NaN variance,
  FSL's `MeaNsK variance nan` or NaN fields). §4.5's last step `b ← b · M` multiplies the whole
  image: when the mask's mean of the field is NaN, `b - mean` is NaN everywhere and `NaN · 0` is
  NaN, so FSL's `_restore` and `_bias` are NaN outside the mask as well (observed in all 14). The
  original now keeps `(0 - mean) · 0` as the field outside the mask (±0, or NaN): the same
  outputs. Permanent case `bias-nan-outside-mask`.
- **A singular covariance during the k-means initialisation with `-v`** (9 cases, several
  channels): FSL's standard output ends with the class means' lines; the `KMeans Iteration 0` line
  of the iteration whose covariance is singular is not printed before `inv(): matrix is singular`
  aborts it. The original prints each iteration's line once its covariances are inverted (the same
  output where nothing aborts). Whether FSL computes the statistics before printing or loses an
  unflushed line at the abort cannot be told apart from these runs. Permanent case
  `singular-kmeans-line`.
- **FSL-101 with `-v`** (3 cases): FSL prints `Beginning prior-based initialisation` before it
  crashes (signal 11) or aborts; the original's refusal (and its abort for several channels) now
  follows that line, as `specs/fast.md` §9.1 says. Permanent case `fsl101-verbose-line`.
- **A rank-deficient covariance** (1 case: two channels, `-n 4`, one slice): the original's LU
  meets an exactly zero pivot and aborts as FAST does on a singular matrix; FSL's arithmetic
  leaves a pivot of rounding size and finishes. Which one a covariance that is singular in exact
  arithmetic gives is decided by the matrix arithmetic's last bits (§8.6: not followed by the
  original).
- **Two noise-free channels** (2 cases): ill-conditioned covariances; the partial volumes (and
  one class's `_pveseg` Dice, 0.99891) move with the arithmetic's last bits, as `multi/s5` does
  (above).
- **Degenerate statistics with several channels** (1 case, open: `cleanroom-fast` 1840, `-n 4`
  with two channels, `-v`): from the second bias iteration on FSL prints every class's mean as
  `-nan` where the original prints `1` (a class weight sum of 0, the mean kept at 0 as §5.3 says);
  their images agree within the thresholds. Treating a non-positive sum as "keep the value"
  instead of "0" in the members or the likelihoods did not reproduce FSL's lines (tried and
  reverted). A question for the spec: how FSL's posteriors become NaN after a NaN covariance.
- **The suite itself** (153 cases): nibabel needs `backports.zstd` for FSL 6.0.7.23's `NIFTI_ZST`
  images and refuses a NIfTI-2 header whose quaternion's squares sum to a little more than 1
  (FSL's own output); the suite now reads both. Permanent case `zstd-outputs`.

The thread test found, before the trial, that the outputs' expansion and the negative-value rule
ran on rayon's global pool (one thread per CPU) after the segmentation's pool had returned; they
now run on the call's own pool (`fast::grid::Grid::expand_on`).

## glibc's functions in `reproduce` (the glibc maths part, 2026-11-04)

With `reproduce` the original now calls glibc 2.35's `exp`, `log`, `expf` and `logf` where
`specs/fast.md` §8 names them (`larmorx_core::math::Libm`; the default keeps the correctly
rounded functions, unchanged). The standard record, made again from `be3db91`: fMRIPrep's three
calls and every single-channel case are FSL's values bit for bit (Dice and identical voxels
1.00000, partial volumes, probabilities and bias fields 0; before: Dice 0.998 to 0.99998, partial
volumes up to 0.00022 on average), so their checks no longer need the comparison with FSL under
the CORE-MATH preload. Two to eight channels: the segmentations FSL's, the partial volumes FSL's
or within 1.3e-7 on average (nine cases; the covariances' arithmetic). The fix passes now change
values (the default's functions against glibc's), reported as "GLIBC-07 (not named)" (FAST names
nothing, the user's decision of 2026-11-04). `multi/s5` still fails (identical voxels 0.95852 to
0.95850 in `_mixeltype`): its five channels' nearly singular covariance, above, not the C
library. New in the record (main's suite gained it after main's record; the same with main's
code): `opt/nan-first`, where the default should name FSL-203 and names FSL-103: FSL-203 is not
yet in the original (a follow-up; done 2026-11-05, below).

## The catch-up of 2026-11-05 (the spec answers of 2026-11-03)

The clean-room catch-up's relay 2 brought the original up to `specs/fast.md` §12's answers
(black-box runs through the oracle cache, `fsl_streams.py`, `probe.py`; the replica's program run
as a black box where the spec's validation plan compares the default with it):

- **FSL-202** (§4.4): the default subtracts the square of the double-precision mean (the mean
  itself still rounded to single for every other use), in the class statistics and the partial
  volumes' statistics; `reproduce` FAST's single-precision square; named in both modes when a
  class's variance differs between the two (nearly every run). **The default is the replica's
  default exactly where the functions are the same:** the original built with
  `glibc_default.patch` (`LXM_GLIBC=1`: its default calls glibc's `exp`, `log`, `expf`, `logf`,
  as `reproduce` does) gives the replica's default bit for bit, standard output and every image,
  on every single-channel case of the suite (`glibc_default.py`: fMRIPrep's three calls,
  `real/fmriprep-call-half`, 34 `opt/` and `priors/` cases, the two NaN cases below), so the two
  corrections are the same arithmetic. With its correctly rounded functions the default stays
  within the narrow band of the corrected variance (§11): fMRIPrep's call on ds000005 `_mixeltype`
  identical to the replica's default, partial volumes 3.5e-7 apart on average.
- **Several channels have no FSL-202**, so their default's band is FSL's own: with glibc's
  functions their labels are the replica's default's exactly and only the partial volumes move
  (larmorx's own LU and quadratic forms, §8.6); with the correctly rounded functions `multi/s3`
  lands at `_mixeltype` Dice 0.99898 (identical 0.99964) and `multi/s2-n4` at 0.99851 (0.99920)
  from the replica's default: **the same numbers, to every digit, as FSL against FSL with
  correctly rounded maths** (`band.py`). The suite allows that band where several channels
  correct nothing compared (FSL with correctly rounded maths is then the default's arithmetic
  with its functions); `multi/s2-manualseg-cycle` (both defaults correct FSL-98's cycle, a path
  FSL never runs) is an expected divergence with this reason (`_mixeltype` 0.99880; FSL's own
  band on its uncorrected order 0.99941). For a spec writer: §11's "the default compared with the
  replica's default, a narrow band" holds for one channel.
- **FSL-203** (§4.1.2): FAST's extrema from the first voxel (a NaN never replaces them) with
  `reproduce`, the values that are not NaN by default; named in both modes when an input's first
  voxel is NaN (`opt/nan-first`'s default now names FSL-203, and FSL-103's count uses the
  corrected maximum, so it no longer names FSL-103 there).
- **`_restore` at NaN voxels** (§4.1.1): the rule for negative values sets to 0 every value not
  within `[0, max]` by both comparisons (NaN voxels too) in its second branch only; `_restore` is
  the image after the rule times the correction, so a NaN voxel stays NaN unless that branch ran.
  Two cases of the original's own (`original/nan-no-negative`: no negative value, the rule
  skipped; `original/nan-offset`: the background at -40, the minimum subtracted) agree with FSL;
  `opt/nan` (one tiny negative value: the second branch) keeps FSL's 0.
- **Standard output at an abort** (§7): the original writes standard output as FAST's C library
  buffers it (`fast::Buffered`): the lines FAST flushes survive an abort, the others written since
  the last flush (the k-means lines, the partial-volume heading) are lost. Observed besides the
  spec, each now the original's: (1) `Starting Single Image Segmentation` / `Starting Multi Image
  Segmentation` is printed **before the images are read** (with an image that cannot be read,
  standard output holds that line alone; with `-S 0` it precedes the channels' error);
  (2) FSL-100's abort (FSLDIR unset) comes **after the image lines** (`T1-weighted image`,
  `Imagesize`, `Pixelsize`, the empty line; several channels: after the heading); (3) fewer than
  0 classes abort after the image is read and those lines are printed, and after FSL-100's abort
  (FSLDIR unset with `-n -1` gives FSL-100's message); (4) **`-n 0` runs FAST's loops without a
  class**: one channel prints its `-O + -W` k-means lines, each main-loop heading, five
  `MRFWeightsTotal=0` lines and an empty class line per iteration, ` BETA` after each `-O`
  iteration (with `-H nan` FSL-99's ` TOTAL 0 RHS ...` and its pairs first), then aborts in the
  partial volumes, ` Starting Partial Volume Estimation ` lost (standard output ends at ` BETA
  0.1`), or with `--nopve` ends with `\n total tissue 0`; several channels print `-W` k-means
  lines, the headings, an empty ` CHANNEL i` line per channel, ` BETA`, and with partial volumes
  `Partial Volume Estimation ` and `\n total tissue 0` (exit 0). Eleven cases of the original's
  own (`original/`) hold these, every one identical to FSL (standard output and status; the
  aborts exit 1 where FSL exits 134).
- **Several channels' class without weight** (§5.3, random testing 1840): its means are the plain
  sums `Σ q y` (0, or NaN once the corrected channels are NaN), its covariance `0 / 0` as
  x86-64's NaN in every entry; FSL's `MEAN 1 STDDEV -nan`, then `MEAN -nan` from the next
  iteration, are now the original's (permanent case `multi-class-without-weight`).
- **Confirmed**: §10's pipeline of planes with a lag of 2 is the original's sweep order (its
  test `pipelined_sweeps_are_serial_sweeps`); §5.8's one-step near ties of several channels'
  partial volumes stay expected (§11's thresholds unchanged); the noise-free and rank-deficient
  random cases stay explained (§8.6) and `multi/s5` stays (ill-conditioned).

The relay-4 trial's 182 findings replayed with this build (`replay_findings.py`, FSL's runs served
by the cache): 178 agree. The four left: 2215 and 2311 (two noise-free channels), 5605 (the
rank-deficient covariance: the original's LU meets a zero pivot), all three explained before
(above), and 3544, the new comparison with the replica's default on a 32 x 20 x 1 slice with
four classes (`-t 2 -H 0 -N --nopve -v`): a `-v` standard deviation 1.72904 against 1.72799
(6e-4 relative); with glibc's functions the original's default is the replica's default bit for
bit there too: the correctly rounded functions' last bits, amplified by four classes on 256
brain voxels.

## The catch-up's random-testing trial (relay 8, 2026-11-05)

`oracles/difftest/2026-11-05-catchup-fast` (seed `cleanroom-fast`, the 2026-11-03 trial's, drawn
again and further; 30 minutes, 14 jobs at one thread, 3 GiB a case, the oracle cache in `use`
mode): 19,695 cases, 19,645 agree, 50 differ. 40 differ only where the record holds the default to
the replica's default (Dice 0.988 to 0.999 of a label image, or a `-v` line in its 4th digit):
with glibc's functions in the default (`glibc_default.patch`, `LXM_GLIBC=1`, replayed with
`replay_findings.py`) 39 of them are the replica's bit for bit, so the two defaults differ only by
the correctly rounded functions' last bits; the 40th (`cleanroom-fast-02117`, two channels with
`-a` priors) was not measured (the patched build predates FLIRT's changes). 4 are a covariance
singular in exact arithmetic, where the matrix arithmetic's last bits decide which program aborts
(`12578`: FSL; `13533`, `05605`: the original). 6 of several channels part from FSL in
`reproduce` (`02215` and `02311` explained before; open: `15057`, `16101`, `18394`, `19057`), and one
two-channel run with `FSLOUTPUTTYPE=ANALYZE` prints the image layer's message where FSL's
uncaught `std::runtime_error` prints `terminate called after throwing ...` (`14174`, open).

