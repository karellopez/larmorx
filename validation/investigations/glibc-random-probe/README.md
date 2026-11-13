# glibc 2.35's `rand`, `srand`, `random` and `srandom`, as a black box

The probes behind the **(observed)** marks of `specs/glibc-random.md` (the C library's default
generator, which FreeSurfer's `mri_robust_template` and `setRandomSeed` call). **Open to the clean-room
implementer of that spec**: the files here hold only calls of the C library's functions by name, a
loop and a hash, no glibc code; the implementer may read, build and run them (black-box runs of the
original, `docs/porting-guide.md` §1).

| File | What it is |
|---|---|
| `glibc_random_probe.c` | calls the system's `srand`, `rand`, `srandom`, `random`; commands `rand`, `random`, `default`, `shared`, `hash`, `long`, `raw` (its header says what each prints). Build: `cc -O2 -o glibc_random_probe glibc_random_probe.c` on a machine with glibc 2.35 (the reference machine: Ubuntu 22.04, x86-64) |
| `robust_template_tp.sh` | the official FreeSurfer 8.2.0 `mri_robust_template`'s random first time point against the probe: 4 input counts x 7 seeds, inputs made by the official `mri_volsynth` (8 x 8 x 8), each run stopped after 30 s. `robust_template_tp.sh <FREESURFER_HOME> <FS_LICENSE> <glibc_random_probe> <output folder>` |

Results (2026-11-12, the reference machine, glibc 2.35-0ubuntu3.15):

- the reference numbers and hashes of the spec's §4 and §6 (the commands are given beside each);
- `robust_template_tp.sh`: the official program's "Will use TP k" equals `(o₀ mod N) + 1` from the
  probe in **28 of 28** cases (`N` = 2, 3, 5, 7; seeds 1, 2, 53, 1234, 99991, 2147483647, −1).
