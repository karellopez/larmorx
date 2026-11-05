# The `libm` crate's last uses (2026-11-05)

A small program of its own (its own cargo workspace, never part of larmorx's build) that measured
the four last calls of the `libm` crate (musl's algorithms, pure Rust) against
`larmorx_core::math` (correctly rounded, CORE-MATH's) and the C library (glibc 2.35 on the main
machine, called as a black box through Rust's `std`), on the inputs each call meets, before they
were switched to `larmorx_core::math` (CLAUDE.md rule 5; `docs/findings/platform-math.md`, "The
`libm` crate's last uses"):

- AFNI's oblique angle (`crates/larmorx-afni/src/geometry.rs`): every float32 in [0, 1];
- the obliquity of `larmorx_core::affine` (nibabel's): 2·10⁷ random doubles in [0, 1);
- the head-motion correction's decomposition (`crates/larmorx-mri/src/hmc/rigid.rs`): `atan2` of
  2·10⁷ random float pairs and 2·10⁷ near-identity ones;
- the B-spline prefilter (`crates/larmorx-interp/src/lib.rs`): `z^(n−1)` for ITK's poles of orders
  2 to 5 and every line length that reaches that branch.

```bash
cd validation/investigations/libm-last-uses
CARGO_TARGET_DIR=<scratch>/libm-target cargo build --release --offline
RAYON_NUM_THREADS=12 <scratch>/libm-target/release/libmcheck
```

The output of 2026-11-05 (about two minutes on 12 threads) is in the findings.
