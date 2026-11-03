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

## The FFT (fslmaths' FFT convolution), 2026-11-03

What it shows: that `larmorx-nc-core`'s `KissFft` is Armadillo 14.2.1's Kiss FFT engine bit for
bit once it is handed Armadillo's own twiddle factors, and that the only difference is those
factors. FSL's library (`libfsl-newimage.so`, its machine code) computes each factor as
`cexp(0 + i (n k))`, `k = -+2 acos(-1) / N`, with glibc's `cexp`, which takes glibc's `sincos`
of the angle. glibc 2.35's `sincos` is one build on x86-64 (where `sin` and `cos` choose an FMA
build on CPUs with FMA, and then differ from it), IBM's code (LGPL) and not always correctly
rounded; the replica computes the factors with correctly rounded `cos` and `sin` in both modes
(GLIBC-07).

- `fft_probe.cc`: Armadillo's `fft` and `ifft` of seeded complex vectors (three per length: real,
  complex, complex), one line per output value, as 64-bit patterns.
- `fft_twiddles.cc`: the engine's factors (`fft_engine_kissfft<cx_double, inverse>`'s
  `coeffs_array`) beside glibc's `cexp`, `sincos`, `cos` and `sin` of the same angles (called
  through volatile pointers: never folded or merged by the compiler).
- `check_fft.py`: runs both and the replica's side (the ignored tests of
  `crates-nc/larmorx-nc-core/tests/fft_probe.rs`: `twiddles`, `kiss_fft_lines`, and
  `kiss_fft_lines_with_twiddles`, the engine given Armadillo's factors through
  `KissFft::with_twiddles`), and counts.
- `fft_reads.py`: per length, whether a butterfly reads one of the factors that are not
  correctly rounded (the engine's factor order and each butterfly's indices).
- `check_fft_case.py`: one fslmaths case end to end: FSL's output (oracle cache) against the
  replica as built and against the replica built with the probe-only feature `probe-twiddles`
  (`larmorx-nc-core`), which reads Armadillo's factors from `LARMORX_PROBE_FFT_TWIDDLES`.

```bash
PATH=$HOME/fsl-6.0.7.23/bin:/usr/bin:$PATH g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER \
    -I $HOME/fsl-6.0.7.23/include validation/probes/fsl-avwutils-lib/fft_probe.cc -o <scratch>/fft_probe
PATH=$HOME/fsl-6.0.7.23/bin:/usr/bin:$PATH g++ -O2 -std=c++17 -DARMA_DONT_USE_WRAPPER \
    -I $HOME/fsl-6.0.7.23/include validation/probes/fsl-avwutils-lib/fft_twiddles.cc -o <scratch>/fft_twiddles
(cd crates-nc && cargo test -j 14 --release --locked -p larmorx-nc-core --test fft_probe --no-run)
CARGO_BUILD_JOBS=14 scripts/orchestration/boxed.sh 8 python -P validation/probes/fsl-avwutils-lib/check_fft.py \
    --twiddles-probe <scratch>/fft_twiddles --fft-probe <scratch>/fft_probe --work <scratch>/fft --sizes 1-256
# end to end: the probe build in its own target directory (never the repository's)
(cd crates-nc && CARGO_TARGET_DIR=<scratch>/target-probe cargo build -j 14 --release --locked \
    -p larmorx-nc-cli --features larmorx-nc-core/probe-twiddles)
LARMORX_ORACLE_CACHE_MODE=use scripts/orchestration/boxed.sh 4 python -P \
    validation/probes/fsl-avwutils-lib/check_fft_case.py \
    --probe-bin <scratch>/target-probe/release/larmorx-nc --twiddles <scratch>/fft/arma_twiddles.txt
```

Results on 2026-11-03 (glibc 2.35, x86-64), lengths 1 to 256:

- Twiddle factors: 65,792; Armadillo's equal glibc's `cexp` and `sincos` on every one (0
  differ), and differ from glibc's own `cos` and `sin` on 106 (this CPU has FMA; with
  `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA` glibc's `cos` and `sin` take their other build
  and agree with `sincos` on all, while `sincos` and Armadillo's factors do not change). Against
  the replica's correctly rounded factors: **220 differ (one bit), in 80 of the 256 lengths**
  (15, 20, 30, 39, 40, 49, 51, 59, 60, 61, ...). The angles themselves are identical (0 differ:
  the same `pi`, the same product `n * k`, the same signs).
- Transforms: 197,376 output values. The replica's engine: 5,426 differ, in 60 lengths (a
  butterfly reads a differing factor in 64 of the 80 lengths; in the other 16, e.g. N = 20's
  factor 19, none is read: `fft_reads.py`; in 4 of the 64 these inputs round alike). **Given Armadillo's
  factors: 0 differ.** So the butterflies, the factor order, the complex products
  and the inverse's scaling are Armadillo's; only glibc's `sincos` differs.
- End to end (`check_fft_case.py`, the trial's case `fft-twiddles-glibc-sincos`: `-recip
  -kernel gauss 2.25 -fmeanu` of a 30 x 1 x 3 float image, transforms of 60 and 26 points):
  the replica as built and the probe build with its own factors differ from FSL's output (1 of
  90 voxels, one unit in the last place); the probe build with Armadillo's factors is
  identical.

Relay 9's first reading (`validation/investigations/fsl-maths/fft_twiddles.py`, glibc's
separate `cos` and `sin`) had compared the wrong glibc function: on this CPU those are the FMA
builds, which agree with the correctly rounded value where `sincos` does not.
