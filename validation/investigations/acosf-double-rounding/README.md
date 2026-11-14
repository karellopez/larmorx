# The correctly rounded `float` arc cosine from the `double` one

Relay 19 of FreeSurfer's track D (fs.surfio). FreeSurfer's discrete curvatures and the topology
fixer's vertex pseudo-normal call `acosf`. larmorx's default takes correctly rounded maths (glibc's
`acosf` only with `reproduce`), and `larmorx_core::math` has a correctly rounded `acos` for `double`
(CORE-MATH's `cr_acos`) but no `float` one. Rounding a correctly rounded `double` result to `float`
gives the correctly rounded `float` result except where the `double` lands exactly halfway between
two floats (double rounding). This folder finds those inputs, exhaustively.

- `scan.c`: every float in [-1, 1] (2,130,706,434 of them), `cr_acos((double)x)` from the
  CORE-MATH preload (`<workspace>/oracles/core-math-preload/libcrmath.so`,
  `scripts/build_crmath_preload.sh`); prints the inputs whose result's low 29 bits are a float
  midpoint. Build and run (about 40 s, one thread):

      /usr/bin/gcc -O2 -o scan scan.c <workspace>/oracles/core-math-preload/libcrmath.so -lm \
        -Wl,-rpath,<workspace>/oracles/core-math-preload
      <workspace>/larmorx/scripts/orchestration/boxed.sh 2 ./scan > scan.out

- `decide.py`: the true value of those inputs at 200 bits (mpmath) decides the correctly rounded
  float.

Result (2026-11-13): two midpoints, `0x328885a3` (1.5893255e-08) and `0x39826222` (0.00024868647).
At both the true arc cosine lies above the midpoint (by 6.1e-17 and 1.0e-17), so the double
rounded to float (ties to even: `0x3fc90fda`, `0x3fc907b4`) is one unit too low; the correctly
rounded floats are `0x3fc90fdb` and `0x3fc907b5`. Every other float in [-1, 1] rounds correctly.
`larmorx-freesurfer-surf`'s `discrete::acosf_cr` used exactly that (the two exceptions as a table) until
`larmorx_core::math::acosf` (CORE-MATH's `cr_acosf`, `validation/investigations/coremath-acosf/`) arrived;
the two agree on every float in [-1, 1], and `discrete::acosf` now calls `Libm::acosf`.
