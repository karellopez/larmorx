# FreeSurfer's random numbers and cubic spline: the clean rooms against FreeSurfer (black box)

The validation of two clean rooms in `larmorx-core` (FreeSurfer's foundations, queue items 3.1-05
and 3.1-06, 2026-11-13):

- `larmorx_core::ranlux` (`crates/larmorx-core/src/ranlux/`): RANLUX389 and the eight
  distributions FreeSurfer 8.2.0 draws from, from [`specs/ranlux.md`](../../../specs/ranlux.md);
- `larmorx_core::cubic_spline` (`crates/larmorx-core/src/cubic_spline/`): the cubic spline of
  `mri_normalize`, from [`specs/burkardt-spline.md`](../../../specs/burkardt-spline.md).

Written by their clean-room implementer, who never opened FreeSurfer's, GSL's or Burkardt's code:
everything here runs FreeSurfer as a black box (the probes `fs_rng_probe` and `fs_spline_probe`,
prebuilt by the orchestrator in `<workspace>/oracles/freesurfer-8.2.0-unit/probes/`, and the
official `mri_volsynth`). What the runs showed: [`docs/findings/ranlux-clean-room.md`](../../../docs/findings/ranlux-clean-room.md)
and [`docs/findings/cubic-spline-clean-room.md`](../../../docs/findings/cubic-spline-clean-room.md).

| File | What it is |
|---|---|
| `cleanroom-probe/` | the clean rooms behind the probes' commands (its own Cargo workspace): `raw`, `dist`, `mixed` as `fs_rng_probe` takes them, `cases`, `d3`, `set1` as `fs_spline_probe` takes them; `--reproduce` before the command for FS-100, FS-101 and FS-103; `--system-tan` gives the large-shape gamma draw the C library's `tan` (a black box, through Rust's `f64::tan`) instead of larmorx's correctly rounded one, and `--tan-hits FILE` writes the draws that met an argument where the two tangents differ; `infer-mixed` names the draw behind each line of the probe's `mixed` (how its rotation was found), `tan-list` the arguments where the two tangents differ, `bench` the generator's speed |
| `compare_random.py` | the random numbers' validation plan (`specs/ranlux.md` §10.1 to §10.5, §9's speed): 2,022 seeds' first 100,000 values, 421 distribution cases (20,000 draws each, three seeds), the `mixed` stream, FS-100 and FS-101 under both modes, the official `mri_volsynth` (10 fields), the tangent list; the probe and `mri_volsynth` through the oracle-output cache (one thread, the family `freesurfer`) |
| `compare_spline.py` | the spline's validation plan (`specs/burkardt-spline.md` §9.1 to §9.3): 20,000 seeded cases, 1,000 tridiagonal systems, the five messages, FS-103's NaN knots, and the speed of both programs on the same cases |
| `stdin-runner/run_stdin.sh` | runs a program with a file as its standard input: the oracle-output cache passes none, so `compare_spline.py` runs `fs_spline_probe` through it (the probe and the input file are arguments, keyed by their content) |
| `observations.sh` | the one-off observations behind the findings: the seeds whose state is all zero (multiples of 2147483563 from −12 to 12), the draws that return from the all-zero stream, the input forms of `fs_spline_probe`'s `cases` and `d3` (read from its outputs) |
| `result-2026-11-13-random.json`, `result-2026-11-13-spline.json` | the results, from a clean commit (their `larmorx_commit`) |

## How to run

```sh
# the clean room's probe (its own workspace; any target directory)
cd validation/investigations/freesurfer-cleanroom-random-spline/cleanroom-probe
CARGO_TARGET_DIR=<scratch>/target cargo build --release --locked
cd ../../../..
# the comparisons (in a memory box; the workspace's validation package on the path)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-cleanroom-random-spline/compare_random.py \
  --cleanroom <scratch>/target/release/cleanroom-probe --scratch <scratch> --out <scratch>/random.json
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-cleanroom-random-spline/compare_spline.py \
  --cleanroom <scratch>/target/release/cleanroom-probe --scratch <scratch> --out <scratch>/spline.json
bash validation/investigations/freesurfer-cleanroom-random-spline/observations.sh <workspace> <scratch>/target/release/cleanroom-probe
```

`compare_random.py` takes about 3 minutes with a warm cache (its 24 runs of the probe that never
return are stopped after 5 s each, and never cached), `--quick` for 20 random seeds and 2,000 draws
per case; `--sections raw,dist,defects,volsynth,tan,speed` chooses. `mri_volsynth` reads the
FreeSurfer licence in place (`--fs-license`, default `/usr/local/freesurfer/license.txt`); it is
never copied or printed. Both scripts exit 1 when anything unexpected differs.
