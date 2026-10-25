# BET: informal timings, and fMRIPrep's image without `dc`

`timing.sh` times FSL 6.0.7.17's `bet` (fMRIPrep's call on a BOLD reference), `bet2 -m` (a T1w)
and `betsurf --t1only -m` against the larmorx-nc replica, one run each, and checks whether
fMRIPrep 25.2.6's image can run `bet -Z`. Not a benchmark report: the machine was shared.

Result on 2026-10-24 (12 logical CPUs, load 6 to 8 from other jobs), on ds000005 sub-01's
BOLD reference (run 1, N4-corrected, from the fMRIPrep oracle run), its raw T1w, and ds000256
sub-CTS203's T1w with FSL's own bet2 mesh and FLIRT matrix:

| Call | FSL | replica |
|---|---|---|
| `bet <boldref> <out> -f 0.20 -m` (fMRIPrep's) | 1.14 s | 0.62 s |
| `bet2 <T1w> <out> -m` | 2.27 s | 1.06 s |
| `betsurf --t1only -m` | 31.4 s | 15.1 s (1 thread), 13.6 s (3 threads) |

fMRIPrep's image has no `dc` (neither on the PATH its programs get nor on its own): its `bet`
script stops at `-Z`'s padding arithmetic (`line 307: dc: command not found`) and at `-R`'s
centre test (`line 265`), so `bet -Z` and `bet -R` cannot run there; fMRIPrep itself calls
neither. The parity record's bands and the evidence's `-R` example leave that build out
accordingly.

## glibc's `pow` and `exp` are Arm's Optimized Routines (MIT)

`arm_glibc_pow_exp.py <work dir>` downloads Arm's Optimized Routines v19.11 (`math/pow.c`,
`pow_log_data.c`, `exp.c`, `exp_data.c`, `math_config.h`), preprocesses the data files with
Arm's x86-64 configuration and compares them with glibc 2.35's `e_pow.c`, `e_pow_log_data.c`,
`e_exp.c` and `e_exp_data.c` (the workspace's `tags/glibc-2.35`): `__exp_data` 270 of 270 numbers
and `__pow_log_data` 393 of 393 identical; the code differs only in macro names and in Arm's other
configurations behind `#if` (2026-10-25). So `larmorx-nc-core/src/glibc_double.rs` is translated
from Arm's MIT files and still computes glibc's `pow` and `exp` (`tools/libm-verify`: 0 of 4
million calls of each differ from the C library): the FSL-licensed package takes no LGPL code
(decision log, 2026-10-24).
