# CORE-MATH's `log10f`: the port against the C, and where the others round otherwise

`larmorx_core::math::log10f` is a port of CORE-MATH's correctly rounded `cr_log10f`
(`src/binary32/log10/log10f.c` at commit 040ee482a8ca, MIT). `Libm::log10f` gives it by default and
glibc 2.35's bits (`log10f_glibc`) with `reproduce`. This folder holds the check behind its tests.

- `log10f_checksums.c`: compiles CORE-MATH's `log10f.c` from the pinned checkout and prints the
  FNV-1a checksum of its results over every 4099th float bit pattern and over all 2^32 of them
  (NaNs as `0x7fc00000`, the tests' rule); then, on every positive finite float, compares it with
  CORE-MATH's double `cr_log10` (`src/binary64/log10/log10.c`, compiled beside it) rounded to float
  and with the C library's `log10f` (glibc 2.35's on the main machine), and prints the special
  values' bits. The build and run commands are in its header comment (about 3 minutes, one thread,
  boxed at 2 GiB).

Result (2026-11-16, gcc 11.4 with `-O2 -ffp-contract=off`, glibc 2.35, log
`<workspace>/oracles/logs/fs-found-c-r33-log10f-c.log`):

    log10f 4099 f8997ea7e41e96f3
    log10f 1 1672ffa82b3c084c
    double differs 0efeee7a cr_log10f c1e99d23 double c1e99d22
    positive finite floats 2139095039, cr_log10f != (float)cr_log10 on 1, != libc log10f on 29787060
    special 0 ff800000 -0 ff800000 -1 ffc00000 -inf ffc00000 inf 7f800000 snan 7fc00001 tiny c23369f4

So glibc 2.35's `log10f` is one unit off on 29,787,060 of the 2,139,095,039 positive finite floats
(1.39 %; the first 20 are printed in the log), and the double logarithm rounded to float on one.
The Rust port gives both checksums (`math::log10f::tests`, the full one with
`cargo test --release -p larmorx-core --lib -- math::log10f --include-ignored`, about 9 s), also
with AArch64's NaN rules emulated (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`,
`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in a debug and a release build.
