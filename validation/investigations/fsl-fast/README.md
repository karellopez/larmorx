# FAST replica: the C library's functions FAST calls

FSL's `fast` binary binds `exp`, `log`, `expf`, `logf`, `sqrt`, `rand` and `srand` from the C
library, the `GLIBC_2.2.5` symbols (FSL's conda build links against an old sysroot: `nm -D
~/fsl-6.0.7.23/bin/fast`). On the oracle machine (glibc 2.35, a CPU with FMA and AVX2) those are
the SVID-compatible wrappers around Arm's routines in their FMA builds (`__expf_fma`,
`__log_fma`, ...). `larmorx-nc-core` carries them (`glibc_float::expf`, `logf_compat`;
`glibc_double::log`, `log_compat`; `crand::Rand`).

- `arm_glibc_expf_log.py <work dir>`: Arm's v19.11 tables of `expf` and `log`, built with Arm's
  x86-64 FMA configuration, found word for word in `libm.so.6`; the offsets of the variants
  `dlsym` resolves, for `objdump -d --start-address=<offset>` (how the fused multiply-adds GCC
  made were read: five in `__expf_fma`, every polynomial step of `__log_fma`; the near-1 path of
  `log` fuses its `r * 0x1p27` split too, with the same bits). Result on 2026-11-02: both tables
  found whole (41 and 274 words).
- The functions themselves: `crates-nc/larmorx-nc-core/tools/glibc-float-verify expf` (all 2^32
  inputs: 0 differ; glibc's `expf` is not correctly rounded on 155,482 of FAST's arguments in
  [-110, 12]), and `tools/libm-verify` with its `glibc_reference` built by FSL's own compiler so
  that it binds the same `GLIBC_2.2.5` symbols (0 of 16 million `log`, 0 of 8 million `logf` and
  0 of 8 million `expf` differ; the wrappers return C's positive `NAN` for the logarithm of a
  negative number, where the newer symbols return `-nan`).
- `rand`: `crand.rs`'s tests compare its sequences with glibc 2.35's (`srand(-1)`, FAST's seed;
  `srand(0)` and `srand(1)`; the millionth value and a checksum of the first million).

## `difftest_finding.py`: a random-testing finding's runs in full (2026-11-02)

Runs a finding's recipe (`shrunk` by default, or `recipe`) through FSL's `fast` (the oracle cache,
`use`) and the replica in `reproduce` and `fix`, printing exit statuses and both streams whole.
It showed what FAST does when FLIRT cannot register the priors (FSL-101, the trial
`oracles/difftest/2026-11-02-fsl-fast`): the verbose line before the crash, an empty mask running
on, the multi-channel exception (the permanent cases `validation/difftest/cases/fsl-fast/fsl101-*`).
Run it in a memory box:

```bash
scripts/orchestration/boxed.sh 4 python -P validation/investigations/fsl-fast/difftest_finding.py \
    <workspace>/oracles/difftest/2026-11-02-fsl-fast/findings/fsl-fast/fast-trial-04762-replica.json
```
