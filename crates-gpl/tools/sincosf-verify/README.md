# sincosf-verify

Checks `larmorx_gpl_afni::glibc_sincosf` against the platform's C library on all 2^32 float
inputs (`f32::sin` and `f32::cos` call the C library's `sinf` and `cosf`).

```bash
cd crates-gpl
cargo run --release -p sincosf-verify -- --threads 4
cargo run --release -p sincosf-verify -- --vectors 40   # test vectors for the unit tests
```

It is meaningful on Linux with glibc 2.35 (the C library AFNI's oracle runs with) on an x86-64
CPU with FMA, where glibc runs its FMA build. It also reports how glibc's SSE2 build (used on
x86-64 CPUs without FMA) would differ, and how often glibc's result is not the correctly
rounded one.

Result on 2026-10-10 (Ubuntu 22.04, glibc 2.35, Intel Core i7-8750H):

| | `sinf` | `cosf` |
|---|---|---|
| port vs glibc | 0 differ | 0 differ |
| glibc's SSE2 build vs its FMA build | 12 differ | 22 differ |
| glibc not correctly rounded (all inputs) | 29,362,810 | 28,209,642 |
| glibc not correctly rounded, inputs in [0, 32] | 1,289,512 | 704,883 |

The arguments 3dTshift's weighted sinc passes lie in [0, 32].
