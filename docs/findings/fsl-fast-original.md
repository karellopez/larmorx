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
