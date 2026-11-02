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
