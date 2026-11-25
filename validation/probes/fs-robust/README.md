# fs-robust: FreeSurfer 8.2.0's robust-registration helpers against the port

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running `fsrobust`
is a black-box run of FreeSurfer's code).

- `fsrobust.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-robust/fsrobust` (`build.sh
  <workspace>`): `mri_robust_register/`'s `MyMRI.cpp`, `MyMatrix.cpp`, `CostFunctions.cpp`,
  `Quaternion.cpp` and `JointHisto.cpp` compiled from the tag with the official programs' flags (the
  sources mapped to the official build's path, so `__FILE__` prints as there), linked with the unit
  oracle's libraries, ITK 4.13.2's VNL and Ubuntu 22.04's static LAPACK and BLAS (the code the official
  binary holds: `docs/findings/freesurfer-robust.md` §2), `acos` wrapped to CORE-MATH's (the project's
  choice for glibc's `acos`). One line per case, as fs-volume's probe: `<result> | <text>`, `EXIT
  <status> | <text>`, `CRASH <signal> | <text>` (a 5 s alarm: `CRASH 14` is a hang).
- The port's side: `crates-freesurfer/larmorx-freesurfer-robust/examples/fs_robust_probe.rs` (`cases
  <seed> <n>`, `run <cases> fix|reproduce`; the generator and the answers in `tests/fs_robust/cases.rs`,
  shared with the hash test `tests/fs_robust_official.rs`).
- `check.py --seed <s> --n <n> [--json out] [--dump lines.jsonl]`: the cases through FreeSurfer by the
  oracle-output cache at one thread, the port in both modes, the comparison (`reproduce` identical but
  for `SKIP`: fs-volume's and fs-filter's lists of memory FreeSurfer does not own, crashes and hangs, and
  FS-966, FS-967, ITK-115; FS-968's lines compared with every NaN read as one NaN and hashed as
  `NANBITS`; `fix` different only with a notice), and the hash the test checks. Run it inside
  `scripts/orchestration/boxed.sh`.
- `bench.cpp` → `.../fs-robust/bench` (built by `build.sh`): `bench <op> <n>` times one operation on an
  `n^3` float volume and prints the FNV-1a of its outputs; the port's
  `examples/fs_robust_bench.rs` prints the same (run both under `/usr/bin/time -f %M` in a box).

Commands (queue 3.2-02, `fs.robust` 1 of 2; volumes are fs-volume's `get_vol` with painted voxels):
`vscale` (`MRIvalscale`, new or in place), `prefilter`, `derfilter`, `rand` (`getRand`), `subsample`,
`blur` (new or in place), `partials`, `rightsize` (`findRightSize`), `bg` (`getBackground`), `ltx`
(`MyMRI::MRIlinearTransform`, new or given destination), `centroid` (with an outside value),
`orient` (`CostFunctions::orientation`), `v2m`, `vv2m`, `m2v` (the conversions), `getvnl`, `isdiag`,
`polar`, `polar2`, `adist`, `rdist` (the `vnl_matrix` distances), `fdiff`, `rlog`.

Commands of 3.2-02's second part (relay 41): `schur` (`MyMatrix::SchurComplex`: `U` and `T`, each
`rows cols` and the elements' real and imaginary parts) and `sqrtm` (`MyMatrix::MatrixSqrt`), in a seeded
stream of their own (`check.py --family sqrtm`; the example's `cases-sqrtm <seed> <n>`). `cabs` and
`csqrt` are linked in the official code with `--wrap` to `complexrt.c`'s, the project's decision
(CORE-MATH's correctly rounded `hypot` inside both, compiled from `tags/core-math` beside it; the same
functions as `larmorx_core::cplx`), so the comparison checks LAPACK's chain, C++'s complex arithmetic
(the system's `__divdc3` and `__muldc3`, unwrapped) and VNL's products bit for bit. The port's lines that
stop at a path not ported (LAPACK's `ZLAQR0`, where `ZLAHQR` does not converge on a matrix with an
infinite or NaN element: FS-969) are counted and hashed as `NOTPORTED`.

```bash
bash validation/probes/fs-robust/build.sh <workspace>
scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-robust/check.py --seed 1 --n 2300 --jobs 12
```

Measured 2026-11-18 (track C, relay 40): seed 1, 2,300 cases: 2,248 identical with `reproduce`, 52 not
comparable, 0 different, hash `afdca92c06265190e0ff1f7a897995758181c306a471e510b0e91271ca1c934a` (2
`NANBITS`); seed 7, 4,600: 4,500 identical, 100 not comparable, 0 different; seed 13, 9,200: 8,989
identical, 211 not comparable, 0 different. `fix` differs only with notices. The bench at 256^3 (one
thread, identical hashes): `docs/findings/freesurfer-robust.md` §5.

Measured 2026-11-20 (track C, relay 41), `--family sqrtm`: seed 1, 1,200 cases: 1,187 identical, 13
`NOTPORTED`, 0 different, hash `42dd17ef0dda71dd610e75d35615262066e7e2c719fe6e124ca19ad4c12e1d42` (the
test's `reproduce_gives_the_official_square_roots`); seed 7, 3,000: 2,961 identical, 39 `NOTPORTED`, 0
different; seed 13, 6,000: 5,913 identical, 87 `NOTPORTED`, 0 different. `fix` differs only with
notices (FS-969, FS-970).

Commands of 3.2-03's first part (relay 42), in a seeded stream of their own (`check.py --family
regstep`; the example's `cases-regstep <seed> <n>`): `rgauss` (`RobustGaussian<T>`'s `median`,
`medianI`, `mad`, `kth_smallest(I)`, `quick_select(I)`; the result, then the list as the call left
it), `regress` (`Regression<T>`: `rob` the robust solution with or without `A`, `ls` the least
squares, `wls` and `wlsf` the weighted least squares, `sigma` `getSigmaMAD`, `tukey` the weights; then
the last error, weight and zero percentages and `b` as the call left it), `trans` (the fifteen
transformation models: DOF, parameters from each kind of vector, steps, matrix, gradient), `quat`
(`importRotVec`, both forms, `importZYXAngles`; the quaternion and its rotation matrix) and `rstep`
(`RegistrationStep<T>(R).computeRegistrationStep(S, T)`, one or two calls, with a `RegRobust` whose
fields the step reads set from the case: the matrix, the intensity parameter, the weight checks, the
parameters, the weights image and the index image). `fsrobust.cpp` opens these classes' members with
`#define private public` and `protected public` around their headers (included after every header
they include, so only they are affected); `build.sh` compiles `Registration.cpp`, `RegRobust.cpp` and
`RegPowell.cpp` with the rest. `check.py` hashes as `NANBITS` the port's `SVDNAN` lines (a `double`
least squares of a matrix with an infinite or NaN number: the double SVD's NaN bits, an open item),
and as `SKIP` FS-1063..FS-1067 and ITK-113 (memory contents or crashes in FreeSurfer).

```bash
scripts/orchestration/boxed.sh 16 python -P validation/probes/fs-robust/check.py --family regstep --seed 1 --n 1000 --jobs 10
```

Measured 2026-11-22 (track C, relay 42), `--family regstep`: seed 1, 1,000 cases: 914 identical, 80
`SKIP`, 6 `NOTPORTED` (FS-1067's least squares under `reproduce`), 0 different, hash
`401667eb618dc40ebb57e6110aa4f7b25c2a1af0d4d679e840562e11b37f6bb2` (the test's
`reproduce_gives_the_official_registration_steps`); seed 7, 1,000: 922 identical, 71 `SKIP`, 7
`NOTPORTED`, 0 different; seed 13, 2,000: 1,840 identical, 154 `SKIP`, 6 `NOTPORTED`, 0 different. `fix`
differs only with notices (GLIBC-07 for the correctly rounded `sin`, `cos` and `exp`). `bench rstep|rstepa
<n>` (one robust rigid step; affine with the intensity scale) at 128^3: FreeSurfer 0.45 s / 101 MB and
0.76 s / 145 MB, the port 0.66 s / 98 MB and 1.06 s / 140 MB, identical hashes
(`docs/findings/freesurfer-robust.md` §6).

Measured 2026-11-22 (track C, relay 43), `--family reg` (3.2-03 part 2: `RegRobust` with `Registration`'s
code): one command `rreg` (the set-up of `mri_robust_register`'s `initRegistration`, `setSourceAndTarget`,
then `findSaturation` and `computeMultiresRegistration`, `computeIterativeRegistration` or
`computeMultiresRegistration`; the images as set, `Rsrc`, `Rtrg`, the final and half-way maps, the scale,
convergence, saturation, weight check, both weights and the centroids; 60 s per case) on analytic volume
pairs (`get_blob`). Seed 1, 60 cases: 55 identical, 5 not comparable; seed 7, 300: 288 identical, 12 not
comparable; seed 13, 1,000: 931 identical, 69 not comparable (FS-1063, FS-1069, FS-163); 0 different; `fix`
differs only with notices. Seed 1's first 40: hash `5485548405ae9d5c608171b3bf6567e796903b3bf43f6614f114d2f8d9e5ecf3`
(4 `SKIP`), the test's `reproduce_gives_the_official_registrations`. The probe built before relay 43 is in
`<workspace>/archive/2026-11-22-fsrobust-probe-before-r43/`.

Commands of 3.2-15 (relay 51, `mri_robust_template`'s affine mean space), in a seeded stream of their
own (`check.py --family logm`; the example's `cases-logm <seed> <n>`): `logm`
(`MyMatrix::MatrixLog`: the complex Schur form, its blocking and reordering by LAPACK's `ZTRSEN`, the
inverse scaling and squaring, the block Parlett recurrence and `sylv_tri`), `expm`
(`MyMatrix::MatrixExp`) and `geomean` (`MyMatrix::GeometricMean` of 3 x 3 matrices), on transforms'
linear parts (small, large and half-turn rotations, isotropic and anisotropic scalings, shears),
diagonal and triangular matrices, Jordan blocks, nonpositive and zero eigenvalues, other sizes, tiny
and huge scales and special entries. `clog` (`std::log` of `std::complex<double>`) is linked with
`--wrap` to `clogrt.c`'s, the decided complex logarithm (CORE-MATH's correctly rounded `atan2`,
`log1p`, `log` and `hypot`; the same function as `larmorx_core::cplx::clog`). FreeSurfer's
`getPadeCoefficients` returns its numbers from arrays whose lifetime has ended, and the official build
reads uninitialized stack memory there (FS-1311): `build.sh` also builds `fsrobust-pade`, the same
probe with a copy of `MyMatrix.cpp` whose five arrays are `static` (the tag is never edited), and
`check.py` compares the cases where the port notes FS-1311 with it (`pade_reference_cases`; the
unpatched official line counted apart, `pade_unpatched_identical`). The port's `SVDNAN` lines (the
exponential's double SVD of a matrix with an infinite or NaN number) are hashed as `NANBITS`.

```bash
bash validation/probes/fs-robust/build.sh <workspace>
scripts/orchestration/boxed.sh 20 python -P validation/probes/fs-robust/check.py --family logm --seed 1 --n 600 --jobs 10
```

Measured 2026-11-24 (track C, relay 51), `--family logm`: see `docs/findings/freesurfer-robust-template.md`
§6 (seed 1 x 600: 598 identical, 0 different, 2 `NOTPORTED`: `ZLAQR0`, FS-969; the hash of
`tests/fs_robust_official.rs`'s `reproduce_gives_the_official_logarithms`).
