# FreeSurfer 8.2.0's RANLUX389, its distributions and its cubic spline, as a black box

The probes behind the **(observed)** marks of `specs/ranlux.md` (FreeSurfer's random generator and
the distributions `mri_coreg`'s dithering, `mri_volsynth`, `mri_glmfit` and `mris_expand` draw
from) and `specs/burkardt-spline.md` (the spline of `mri_normalize`). **Open to the clean-room
implementers of those specs**: the files here hold only calls of
FreeSurfer's functions by name and a comparison loop, no FreeSurfer or GSL code; the implementer may
read, build and run them (black-box runs of the original, `docs/porting-guide.md` §1).

| File | What it is |
|---|---|
| `fs_rng_probe.cpp` | calls FreeSurfer's generator (`sc_rng_alloc`, `sc_rng_set`, `sc_rng_get`), its uniform deviates, every distribution and the random-field layer (`RFspecInit`, `RFdrawVal`, as `mri_coreg` calls them), linked against the unit oracle's `libutils.a`; prints integers in decimal and doubles as the 16 hex digits of their bits. Commands: `raw <seed> <count>`, `dist <name> <seed> <count> [p1 [p2]]`, `rf <name> <seed> <count> [p1 [p2]]`, `mixed <seed> <count>` (one stream, a fixed rotation of every distribution: the rotation is in the source's header and the spec's §10.2), `crafted <name> <a> <b> [p1 [p2]]` (one draw from a state whose next two integers are `a` and `b`, the rest 0: the spec's §4.2 `R = 1` cases), `zero-seeds <first> <last>` (the 32-bit seeds whose seeded state is all zero, §2.6 of the spec) |
| `fs_spline_probe.cpp` | the same for FreeSurfer's cubic spline (`specs/burkardt-spline.md`): `SplineCubicSet`, `SplineCubicValue`, `d3_np_fs`, `OpenSpline`, `OpenSplint`; commands `cases` (cases from standard input, floats as bits; derivative outputs start as the sentinel `7fc00bad` so unwritten ones show), `d3` (systems from standard input), `set1 <n> <ibcbeg> <ibcend> t... y...` (one call, for the error messages and exit status) |
| `build.sh` | builds a probe with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output> [fs_spline_probe.cpp]` (default `fs_rng_probe.cpp`) |
| `refs.sh` | prints the reference numbers the spec quotes (§2.5's streams, §10.2's hashes and first draws): `refs.sh <fs_rng_probe>` |
| `volsynth_compare.py` | runs the official `mri_volsynth` (six random fields) and compares its float32 voxels with the probe's draws in FreeSurfer's field order (the spec's §10.4): `python -P volsynth_compare.py <FREESURFER_HOME> <FS_LICENSE> <fs_rng_probe> <scratch folder>` |
| `run_zero.sh` | the official `mri_volsynth` with the all-zero seed 2147483563 (FS-101) and a negative chi-squared degree of freedom (FS-100), each stopped after 20 s: `run_zero.sh <FREESURFER_HOME> <FS_LICENSE> <output folder>` |
| `tan_check.c` | compares the C library's `tan` (glibc 2.35 on the reference machine) with CORE-MATH's correctly rounded `cr_tan` on every argument the large-shape gamma can pass, `fl(π·k·2⁻²⁴)` for `k` in [0, 2²⁴): `gcc -O2 -fno-builtin tan_check.c <workspace>/tags/core-math/src/binary64/tan/tan.c -lm` |

Results (2026-11-12, the reference machine, glibc 2.35):

- `tan_check`: **41,708 of the 16,777,216 arguments** give a glibc result one unit in the last place
  from the correctly rounded one (20,360 of them with `k < 2²³`; every difference is ±1 bit pattern):
  the spec's §6 needs glibc 2.35's `tan` itself, not CORE-MATH's.
- the streams, distributions and seeds quoted in the spec's §2.5, §8 and §10 come from
  `fs_rng_probe` (`refs.sh`; the commands are given beside each number); `zero-seeds` over all 2³²
  32-bit seeds (8 minutes): exactly 2147483563 and −2147483563 leave the state all zero;
- `volsynth_compare.py`: the official `mri_volsynth` equals the probe on 6 × 120 voxels (Gaussian,
  chi-squared with 30 and 3 degrees of freedom, t with 9 and 2, F with 3 and 25);
- 2026-11-13 (the corrections of the spec's §2.6, §4.2, §8.2): `dist … 2147483563` (the all-zero
  stream): `gamma` of an integer shape `n ≥ 12` returns `b·(n − 1)`, `chisq` of even `ν ≥ 24` gives
  `ν − 2`, `fdist(24, 30)` 0.9821428571428571, the others stop only by the 5 s limit; `raw` on
  `m·2147483563` for `|m| ≤ 12`: all zero exactly for `m` = 1 to 8 and −1 to −4; `crafted gaussian`
  at `R = 1`: the factor is `−0` (`0 8388608` with σ = 1 gives `−0`, with σ = −1 `+0`; `8388608 0`
  the reverse);
- `run_zero.sh`: the official `mri_volsynth --pdf z` and `--pdf chi2 --dof 4` with seed 2147483563,
  and `--pdf chi2 --dof -3`, do not end within 20 s; seeds 1 and 2147483564 give identical volumes.

The unit oracle's `libutils.a` is the official build's code: track B proved instruction identity of
programs built from the same objects (`validation/investigations/freesurfer-unit-oracle/`); the
probe's numbers were also checked against the official `mri_volsynth` (the spec's §9.4).
