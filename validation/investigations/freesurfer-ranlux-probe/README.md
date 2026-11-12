# FreeSurfer 8.2.0's RANLUX389 and its distributions, as a black box

The probes behind the **(observed)** marks of `specs/ranlux.md` (FreeSurfer's random generator and
the distributions `mri_coreg`'s dithering, `mri_volsynth`, `mri_glmfit` and `mris_expand` draw
from). **Open to the clean-room implementer of that spec**: the files here hold only calls of
FreeSurfer's functions by name and a comparison loop, no FreeSurfer or GSL code; the implementer may
read, build and run them (black-box runs of the original, `docs/porting-guide.md` §1).

| File | What it is |
|---|---|
| `fs_rng_probe.cpp` | calls FreeSurfer's generator (`sc_rng_alloc`, `sc_rng_set`, `sc_rng_get`), its uniform deviates, every distribution and the random-field layer (`RFspecInit`, `RFdrawVal`, as `mri_coreg` calls them), linked against the unit oracle's `libutils.a`; prints integers in decimal and doubles as the 16 hex digits of their bits. Commands: `raw <seed> <count>`, `dist <name> <seed> <count> [p1 [p2]]`, `rf <name> <seed> <count> [p1 [p2]]`, `mixed <seed> <count>` (one stream, a fixed rotation of every distribution), `zero-seeds <first> <last>` (the 32-bit seeds whose seeded state is all zero, §2.4 of the spec) |
| `build.sh` | builds the probe with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output>` |
| `tan_check.c` | compares the C library's `tan` (glibc 2.35 on the reference machine) with CORE-MATH's correctly rounded `cr_tan` on every argument the large-shape gamma can pass, `fl(π·k·2⁻²⁴)` for `k` in [0, 2²⁴): `gcc -O2 -fno-builtin tan_check.c <workspace>/tags/core-math/src/binary64/tan/tan.c -lm` |

Results (2026-11-12, the reference machine, glibc 2.35):

- `tan_check`: **41,708 of the 16,777,216 arguments** give a glibc result one unit in the last place
  from the correctly rounded one (20,360 of them with `k < 2²³`; every difference is ±1 bit pattern):
  the spec's §6 needs glibc 2.35's `tan` itself, not CORE-MATH's.
- the streams, distributions and seeds quoted in the spec's §2.5, §4 and §9 come from
  `fs_rng_probe` (the commands are given beside each number).

The unit oracle's `libutils.a` is the official build's code: track B proved instruction identity of
programs built from the same objects (`validation/investigations/freesurfer-unit-oracle/`); the
probe's numbers were also checked against the official `mri_volsynth` (the spec's §9.4).
