# coremath-hypot

The check of the port of CORE-MATH's `hypot` (`crates/larmorx-core/src/math/hypot.rs`,
`larmorx_core::math::hypot`) against its C, and how often glibc 2.35's `hypot` is not correctly
rounded on the arguments SciPy's `Rotation.as_euler` gives it (fMRIPrep's motion parameters,
the confounds' part; decision D4: `hypot` correctly rounded in both modes, an expected
divergence under `reproduce`). glibc's `hypot` is called as a black box: its code is never read.

A small Rust program of its own (its own cargo workspace, never part of larmorx's build):
`coremath-hypot <libcrhypot.so> [log2n] [--threads T]` compares the bits of
`larmorx_core::math::hypot` (the FMA copy on this machine; the portable one is checked against
it by `larmorx-core`'s `fma_copies_are_bit_identical`), of CORE-MATH's C `cr_hypot` and of
glibc's `hypot` (NaN equals NaN) on every pair of 16 special values (zeros, subnormal and normal
extremes, ±1, ±`MAX`, infinities, quiet and signalling NaNs, 3 and 4) and on `2^log2n` pairs of
each of eight families: random bit patterns; uniform on (-1, 1) (rotation-matrix entries); the
components of random unit quaternions (as `as_euler` passes them); exponent differences 20 to 34
(around the `fma` shortcut at 27); one or both subnormal; near overflow; scaled Pythagorean
triples moved by -3 to 3 units in the last place (exact results and results near them); small
integers. Prints `VERIFY-PASS` (larmorx equals the C on every pair) or `VERIFY-FAIL`.

```bash
bash validation/probes/coremath-hypot/build.sh <scratch>/hypot <workspace>/tags/core-math
scripts/orchestration/boxed.sh 2 <scratch>/hypot/coremath-hypot <scratch>/hypot/libcrhypot.so 26 --threads 3
```

Meaningful on x86-64 Linux with glibc 2.35 (the oracle machine). `build.sh` compiles `hypot.c`
(CORE-MATH commit `040ee482a8ca`) with `gcc -O2 -ffp-contract=off -fPIC -shared` (the system's
gcc: `PATH=/usr/bin:$PATH` when another compiler comes first).

**Results**: see `crates/larmorx-core/PROVENANCE.md` (`math/hypot.rs`) and the confounds' findings.
