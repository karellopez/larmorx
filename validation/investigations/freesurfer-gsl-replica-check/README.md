# FreeSurfer's GSL generators: the replica against FreeSurfer and against the clean room

The validation of `crates-gpl/larmorx-gpl-gsl` (FreeSurfer's foundations, queue item 3.1-53): GSL
1.8's RANLUX389 and its distributions as FreeSurfer 8.2.0's `utils/numerics.cpp` transcribes them
(`sc_rng_*`, `sc_uni*`, `sc_ran_*`, `sc_gamma_*`), translated statement by statement, the bit-exact
reference of larmorx's clean room `larmorx_core::ranlux` (queue item 3.1-05, from `specs/ranlux.md`
alone). Written by the replica's author (who read GSL's and FreeSurfer's source): **closed to
FreeSurfer's clean-room implementers**, as the crate itself is.

| File | What it is |
|---|---|
| `compare_replica.py` | `specs/ranlux.md` §10's plan with the replica: the sections of the clean room's `compare_random.py` (imported from `../freesurfer-cleanroom-random-spline/`) run on the replica's probe (`freesurfer`: streams, distributions, `mixed`, FS-100 and FS-101 in both modes, the official `mri_volsynth`, the tangent list), the `crafted` draws (§4.2's `R = 1` and 300 seeded pairs through 13 draws, against `fs_rng_probe` built from its current source), the replica against the clean room's `cleanroom-probe` on every command in both modes and with both tangents, byte for byte (`cleanroom`), and the generators' speed |
| `result-*.json` | the results, from a clean commit (their `larmorx_commit`) |

The replica's probe is the crate's example `gsl_probe` (the commands of `fs_rng_probe` and
`cleanroom-probe`: `raw`, `dist`, `mixed`, `crafted`, `tan-list`, `bench`; `--reproduce`,
`--system-tan`, `--tan-hits FILE`).

## How to run

```sh
# the replica's probe (crates-gpl's workspace) and the clean room's (its own workspace)
(cd crates-gpl && CARGO_TARGET_DIR=<scratch>/target-gpl cargo build --release --locked -p larmorx-gpl-gsl --examples)
(cd validation/investigations/freesurfer-cleanroom-random-spline/cleanroom-probe && \
  CARGO_TARGET_DIR=<scratch>/target-cleanroom cargo build --release --locked)
# fs_rng_probe from its current source (the crafted command), against the unit oracle
bash validation/investigations/freesurfer-ranlux-probe/build.sh <workspace> <scratch>/fs_rng_probe
# the comparisons (in a memory box)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-gsl-replica-check/compare_replica.py \
  --replica <scratch>/target-gpl/release/examples/gsl_probe \
  --cleanroom <scratch>/target-cleanroom/release/cleanroom-probe \
  --crafted-probe <scratch>/fs_rng_probe --scratch <scratch> --out <scratch>/gsl.json
```

`--quick` takes 20 random seeds and 2,000 draws per case; `--sections` chooses among
`freesurfer,crafted,cleanroom,speed`. The prebuilt `fs_rng_probe` of
`<workspace>/oracles/freesurfer-8.2.0-unit/probes/` serves the `freesurfer` section (its runs are in
the oracle-output cache from the clean room's validation); `mri_volsynth` reads the FreeSurfer
licence in place (`--fs-license`). Exits 1 when anything unexpected differs.

## Results (2026-11-13, `result-2026-11-13-gsl.json`, from 879c3370)

- **Against FreeSurfer** (`fs_rng_probe`, the official `mri_volsynth`): 2,022 seeds' first 100,000 values
  identical; 421 distribution cases, 8,382,003 draws: 8,381,462 identical and the 541 others exactly the
  large-shape gamma draws that met one of the 41,708 arguments where glibc's `tan` is not correctly rounded
  (10,613 draws met one; with the C library's `tan` every draw is identical); FS-100 and FS-101 in both modes
  as `specs/ranlux.md` §8 says; `mri_volsynth`'s 10 fields identical; 3,908 `crafted` draws identical.
- **Against the clean room** (`cleanroom-probe`): 10,084 runs, every command in both modes and with both
  tangents, byte-identical (standard output and standard error; 80 of them the same error in both).
- **Speed**: 20 million integers printed, `fs_rng_probe` 2.26 s, the replica 0.74 s, the clean room 0.69 s.
