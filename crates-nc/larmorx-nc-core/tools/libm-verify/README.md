# libm-verify

Checks the C library functions FSL's programs call, as the shared core of the FSL replicas carries
them (`crates-nc/larmorx-nc-core/src/`: `glibc_double`, `fdlibm`, `x87`), against this machine's glibc
and x87 unit: `pow` and `exp`
(glibc 2.35's `__pow_fma` and `__exp_fma`), `tanh` (fdlibm), and the `long double` operations of
FSL's NIfTI layer (an x87 result rounded to 64 bits, then stored as `double` or `float`). It is
what supports the replica's claim that its results are FSL's on every input, not only on the
parity cases (`docs/findings/fsl-bet.md`, "The oracle").

```bash
gcc -O2 -o glibc_reference glibc_reference.c -lm
cargo run --release -p libm-verify -- ./glibc_reference 1000000   # per family; 2 million by default
```

The arguments: BET's own calls (the radius `x^(1/3)` and `x^(1/2)`, `f^0.275`, the Gaussian
kernels' `-j²/(2σ²)`, the smoothness term's `tanh` arguments) and random operands over the whole
range. Meaningful only on x86-64 Linux with glibc 2.35 and a CPU with FMA and AVX2 (the oracle
machine). Result on 2026-10-24: 0 of 4 million `pow`, 0 of 4 million `exp`, 0 of 3 million
`tanh` and 0 of 5 million x87 operations differ.
