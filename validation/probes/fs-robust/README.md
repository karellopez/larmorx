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
