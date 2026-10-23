# floatmath-verify

Checks `larmorx_gpl_afni::glibc_floatmath` (glibc 2.35's `tanhf` and `cbrtf`, which 3dUnifize
calls) against the C library on all 2^32 float inputs.

```bash
cd crates-gpl
cargo run --release -p floatmath-verify -- --threads 4     # tanhf, input by input
cargo run --release -p floatmath-verify -- --checksums     # both, as one hash per function
gcc -O2 -o glibc_reference larmorx-gpl-afni/tools/floatmath-verify/glibc_reference.c -lm
./glibc_reference checksums                                # the same hashes, from glibc
./glibc_reference vectors 24                               # test vectors for the unit tests
```

In a Rust program the symbol `cbrtf` resolves to the Rust standard library's own cube root,
which is correctly rounded (so `f32::cbrt` is not glibc's), hence the C program: the port
matches glibc when the two hashes are equal. `tanhf` resolves to glibc's and is also compared
input by input. It is meaningful on Linux with glibc 2.35 (the C library AFNI's oracle runs
with) on x86-64, where both functions have a single build (no FMA variant).

Result on 2026-10-23 (Ubuntu 22.04, glibc 2.35):

| | `tanhf` | `cbrtf` |
|---|---|---|
| hash of the port's results | `3c6bad00ea34c387` | `c679cb92d4402e33` |
| hash of glibc's results | `3c6bad00ea34c387` | `c679cb92d4402e33` |
| port vs glibc, input by input | 0 differ | (hash only) |
| glibc not correctly rounded (all inputs) | 118,674,314 | 453,492,162 |
