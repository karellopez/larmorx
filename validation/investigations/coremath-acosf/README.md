# CORE-MATH's `acosf`: the port against the C, and the double-rounding cases

`larmorx_core::math::acosf` is a port of CORE-MATH's correctly rounded `cr_acosf`
(`src/binary32/acos/acosf.c` at commit 040ee482a8ca, MIT). `Libm::acosf` gives it by default and
glibc 2.35's bits (`acosf_glibc`) with `reproduce`. This folder holds the check behind its tests.

- `acosf_checksums.c`: compiles CORE-MATH's `acosf.c` from the pinned checkout and prints the
  FNV-1a checksum of its results over every 4099th float bit pattern and over all 2^32 of them
  (NaNs as `0x7fc00000`, the tests' rule), then compares it with CORE-MATH's double `cr_acos`
  (the preload `<workspace>/oracles/core-math-preload/libcrmath.so`) rounded to float on every
  float in [-1, 1]. The build and run commands are in its header comment (about 40 s, one thread,
  boxed at 2 GiB).

Result (2026-11-13, gcc 11.4, log `<workspace>/oracles/logs/fs-found-c-r18-acosf-c.log`):

    acosf 4099 58752c4715886fcd
    acosf 1 2513576ba72710ad
    differs 328885a3 cr_acosf 3fc90fdb double 3fc90fda
    differs 39826222 cr_acosf 3fc907b5 double 3fc907b4
    floats in [-1, 1] 2130706434, cr_acosf != (float)cr_acos on 2

The Rust port gives both checksums (`math::acosf::tests`, the full one with
`cargo test --release -p larmorx-core --lib -- math::acosf --include-ignored`, about 10 s), also
with AArch64's NaN rules emulated (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`,
`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in a debug and a release build. The two floats where the
rounded double is one unit too low are the ones CORE-MATH's own code names; the same two were found
independently by an exhaustive scan with a 200-bit decision (2026-11-13).
