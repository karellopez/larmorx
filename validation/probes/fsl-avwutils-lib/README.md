# fslmaths' library functions, bit for bit

What it shows: that `larmorx-nc-core`'s translations of the library functions fslmaths and
fslstats call outside newimage give FSL 6.0.7.23's own bits:

- Armadillo 14.2.1's `max()` and `min()` of a dense vector (`op_max::direct_max`,
  `op_min::direct_min`: two partial extrema over the even and the odd elements), as
  `arma::direct_max` and `arma::direct_min` (the morphology filters' maximum and minimum, and
  `Mat::max_abs`, which FLIRT, mcflirt and warpfns call);
- fsl-cprob 2111.0's `ndtr`, `ndtri`, `erf` and `erfc` (Cephes, `UNK` constants, with
  `expx2`), with the error reports `mtherr` prints on standard output (`cprob`): `-ztop`,
  `-ptoz`, `-ranknorm` and `-randn`.

`probe.cc` runs FSL's own code (Armadillo's headers, `libfsl-cprob.so`) on one input file and
prints every result as its 64-bit pattern; the replica's side is the ignored test
`avwutils_probe` of `crates-nc/larmorx-nc-core/tests/avwutils_probe.rs`. `check.py` makes the
inputs (seeded), runs both and counts the differing lines:

- 534 vectors of 1 to 101 elements: signed zeros in both orders and positions, NaN (first,
  alone, among numbers), infinities, subnormals, ties, integers with signed zeros, random floats
  with NaN;
- 180,890 arguments: z statistics stored as floats (normal, sd 3, and uniform over ±45, beyond
  `erfc`'s underflow), p-values stored as floats, tails from 1e-320 to 0.1 and 1 - 1e-16 to 0.9,
  `-ranknorm`'s `(k - 0.5) / n` for n = 1 to 601, `drand() - 1`, each branch limit of `ndtr`,
  `erf`, `erfc` and `ndtri` with its neighbours, and the special values (signed zeros, 1, NaN,
  infinities, subnormals).

```bash
bash validation/probes/fsl-avwutils-lib/build.sh <scratch>/avw_probe
scripts/orchestration/boxed.sh 6 python -P validation/probes/fsl-avwutils-lib/check.py \
    --probe <scratch>/avw_probe --work <scratch>/avwprobe
```

Result on 2026-11-03: `arma: 534 inputs, 2136 lines, 0 differ`; `cprob: 180890 inputs, 936094
lines, 0 differ`. The first run found two mistakes of the replica, both fixed: a digit of
`ndtri`'s `Q2[1]` (`...608594...` written `...604594...`: 16 of the 30,000 tail arguments, where
`sqrt(-2 log y) >= 8`, one bit off), and the NaN `ndtr(±inf)` gives (FSL's build multiplies
`y * sqrt(z)` into the square root's register, so of two NaNs the square root's, positive, is
passed on: the replica now takes the order explicitly).
