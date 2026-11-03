# glibc-double-maths

How often glibc 2.35's double-precision `sin`, `cos`, `tan`, `atan`, `asin`, `acos`, `exp`,
`log` and `pow` differ from the correctly rounded result (`larmorx_core::math`, CORE-MATH's), on
the arguments `fslmaths` gives them (GLIBC-07, `docs/findings/fsl-maths.md`, "Known
differences"); and the check of the port of CORE-MATH's `tan` (`crates/larmorx-core/src/math/
tan.rs`) against its C. glibc's functions are called as black boxes: their code is never read.

A small Rust program of its own (its own cargo workspace, never part of larmorx's build):

- `measure [--log2n N] [--threads T]`: each function on `2^N` random arguments of each family
  (image intensities as `float` and as `double` voxels: magnitudes log-uniform from 2^-10 to
  2^17, both signs, positive for `log` and `pow`'s base; uniform on [-1, 1] for `asin` and
  `acos`; uniform on [-40, 40] for `exp`; `pow`'s exponent uniform on [-3, 3]) and on every
  integer of an int16 image's range: the double results that differ, and those whose
  difference survives the rounding to `float` (fslmaths' default `-dt float`). Counts only.
- `coremath <libcrmath.so> <tan.wc> <atan.wc> [log2n]`: `larmorx_core::math::tan` and
  `atan2(x, 1)` (the replica's `-atan`) against CORE-MATH's C `cr_tan` and `cr_atan`, on every
  worst case of `tan.wc` and `atan.wc` at ±x and `2^log2n` inputs of each of four random
  families (random bit patterns, uniform on (-10, 10) and (-2^30, 2^30), log-uniform magnitudes
  from 2^-30 to 2^1023). Prints `VERIFY-PASS` or `VERIFY-FAIL`.

```bash
bash validation/probes/glibc-double-maths/build.sh <scratch>/gdm <workspace>/tags/core-math
CM=<workspace>/tags/core-math/src/binary64
scripts/orchestration/boxed.sh 2 <scratch>/gdm/glibc-double-maths coremath <scratch>/gdm/libcrmath.so \
  $CM/tan/tan.wc $CM/atan/atan.wc 28 --threads 14
scripts/orchestration/boxed.sh 2 <scratch>/gdm/glibc-double-maths measure --log2n 26 --threads 14
```

Meaningful on x86-64 Linux with glibc 2.35 (the oracle machine, where FSL's programs call these
functions). `build.sh` compiles `tan.c` and `atan.c` with `gcc -O2 -ffp-contract=off -fPIC
-shared` (the system's gcc: `PATH=/usr/bin:$PATH` when FSL's compiler comes first).

**Results (2026-11-03, glibc 2.35, x86-64).** `coremath` with `log2n` 28: 2,154,706 worst cases
of `tan` and 111,520 of `atan` (both signs) and 1,073,741,824 random inputs of each: 0 differ
from CORE-MATH's C (`VERIFY-PASS`); glibc's `tan` is not correctly rounded on 535,826 of the tan
worst cases, its `atan` on 17,162 of the atan ones. `measure` with `log2n` 26 (67,108,864
arguments per family):

| function | float voxels | double voxels | every int16 value | float results that differ |
|---|---|---|---|---|
| `sin` | 80,197 (0.120 %) | 80,110 (0.119 %) | 88 of 65,536 | 0 |
| `cos` | 56,926 (0.085 %) | 57,295 (0.085 %) | 94 of 65,536 | 0 |
| `tan` | 138,309 (0.206 %) | 138,418 (0.206 %) | 162 of 65,536 | 0 |
| `atan` | 54,995 (0.082 %) | 55,664 (0.083 %) | 0 of 65,536 | 0 |
| `asin` (on [-1, 1]) | 105,726 (0.158 %) | 106,195 (0.158 %) | 0 of 3 | 0 |
| `acos` (on [-1, 1]) | 47,143 (0.070 %) | 47,315 (0.071 %) | 0 of 3 | 0 |
| `exp` (on [-40, 40]) | 51,224 (0.076 %) | 51,023 (0.076 %) | 1 of 1,455 | 0 |
| `log` | 13,983 (0.021 %) | 13,954 (0.021 %) | 1 of 32,767 | 0 |
| `pow` | 56,059 (0.084 %) | 55,329 (0.082 %) | 28 of 32,767 | 0 |

So a float image's result is FSL's whatever the function; a double image's differs in the last
place on that fraction of its voxels where the replica cannot have glibc's bits (`sin`, `cos`,
`tan`, `asin`, `acos`, `atan`: IBM's code in glibc, LGPL). `exp`, `log` and `pow` have glibc's
bits with `reproduce` (Arm's Optimized Routines, `larmorx_nc_core::glibc_double`).
