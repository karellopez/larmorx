# ANTS-72: antsRegistration varies from run to run (the clock's seed)

**Finding** (2026-10-23; register ANTS-72, `docs/findings/upstream-defects.md`): with a displacement
field as initial transform (`-r <field>`) before a linear stage, antsRegistration gives a different
result on every run, even at one thread with `ANTS_RANDOM_SEED` set: the scales of the linear
stage are estimated from 1000 voxels drawn by ITK's Mersenne twister, which is seeded from a hash of
`time()` and `clock()` in that path. Found when the orchestrator's re-run of the
`ants-registration-corrections` record gave 4.70 mm where the agent's run gave 3.75 mm for the
reference "ANTs with the B-spline transform as a displacement field".

**How it was shown:**
1. `repro.py <out dir> [repetitions]` runs that reference's construction (the field written by
   antsApplyTransforms from the B-spline transform, then antsRegistration with `-r <field>` and a
   linear stage, one thread, `ANTS_RANDOM_SEED=42`) several times in identical conditions and prints
   the SHA-256 of each run's output: they differ.
2. Built from `fixclock.c` (`cc -shared -fPIC -o libfixclock.so fixclock.c`), a preload makes
   `time()` and `clock()` return constants; with `FIXCLOCK=<path to libfixclock.so>`, `repro.py`'s
   runs give identical bytes. So the variation is the clock's seed, nothing else.

Run from the larmorx checkout with `PYTHONPATH=python:validation/src` (it uses the parity suite's
inputs), the ANTs oracle in `<workspace>/oracles/ants-2.6.5`. The parity record now runs that
reference five times and reports its median, range and ANTs' own spread (up to 8.5 mm).
