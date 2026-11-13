# x87-extended

The check of `larmorx_core::x87::F80` (the x87's 80-bit extended arithmetic in software,
`crates/larmorx-core/src/x87/`) against the machine's x87, for FreeSurfer 8.2.0's replicas: the
functions that compute in `long double` (`docs/freesurfer/variation.md` §1) are compiled by GCC
8.4.0 into x87 instructions, and a replica needs their bits.

Two parts:

- **The hardware side**, `libx87probe.so`, built by `build.sh` with GCC 8.4.0 (conda-forge's, in
  `<workspace>/oracles/toolchains/gcc-8.4`, the official binaries' compiler) and FreeSurfer's flags
  (C: `-O3 -fopenmp`, with `-fno-builtin` so that `sqrtl` and `llrintl` stay calls into glibc; C++:
  `-std=c++11 -O3 -DNDEBUG -fopenmp -fno-builtin`; both with Ubuntu's default hardening flags, as the
  official build):
  - `x87_ops.c`: the four operations, the square root three ways (the `fsqrt` instruction, glibc's
    `sqrtl`, GCC's `__builtin_sqrtl`, which C++'s `std::sqrt(long double)` compiles to), the six
    comparisons, the conversions to `double`, `float`, `long`, `int`, `short`, `unsigned long`,
    `unsigned int` (C's casts) and from `double`, `float`, `long`, `int`, `unsigned long`,
    `unsigned int`, glibc's `llrintl`, and the control word;
  - `patterns.cpp`: the arithmetic patterns of the FreeSurfer functions that compute in
    `long double`, written for this probe as plain loops (no FreeSurfer code): sums of `double`
    products (`DVectorDot`, `DMatrixMultiply`), a mean, variance and standard deviation from `long
    double` sums (`DListStats`), edge vectors and a cross product from `float` coordinates
    (`MRISfaceNormalFace`), a unit vector and its gradient (`SurfGradUnitVector`), `float` kernel
    weights normalised by their `long double` sum (`MRIgaussianSmoothNI`), squared length
    differences (`MRISedgeLengthCost`).
- **The driver**, `x87-probe` (its own cargo workspace, never part of larmorx's build): the cases
  of `larmorx_core::x87::cases` (seeded operand families: random bit patterns, normals of every
  exponent, nearby exponents, sparse significands with exact results and ties, cancellations,
  products and quotients in the denormal range and near overflow, denormals and pseudo-denormals,
  exact `double`s, `float`s and integers, special values, the boundaries of `double`, `float` and
  the integer types; every pair of 42 special values, unsupported encodings included), computed by
  `F80` and by the hardware, compared bit for bit (NaN payloads included). It prints the counts,
  the kinds of the hardware's results (normal, denormal, zero, infinite, indefinite, other NaNs,
  exact ties of products), a control per pattern (how many of its outputs differ when computed in
  `double`: the patterns depend on the extended precision), and `VERIFY-PASS` or `VERIFY-FAIL`
  (exit status 1). `--record <file>` writes the hashes of the hardware's results on the cases the
  unit tests of `larmorx_core::x87` run (the first 1,024 of each operation and family, every pair
  of special values, the first 1,024 of each conversion, the first 256 of each pattern): the unit
  tests (`crates/larmorx-core/src/x87/tests.rs`) check them on every platform.

```bash
bash validation/probes/x87-extended/build.sh <scratch>/x87
scripts/orchestration/boxed.sh 6 <scratch>/x87/x87-probe <scratch>/x87/libx87probe.so 24 \
    --record <scratch>/x87/hashes.rs
```

Meaningful on x86-64 Linux only (`long double` is the x87's format there); the probe refuses to
run unless the x87 control word is Linux's default `0x037f` (64-bit precision, round to nearest,
every exception masked).

**Results**: see `crates/larmorx-core/PROVENANCE.md` (`x87/`).
