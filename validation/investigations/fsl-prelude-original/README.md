# fsl-prelude-original: the clean-room original's checks (PRELUDE's clean-room part)

The clean-room implementer's tools for `larmorx fsl prelude` (`crates/larmorx-fsl/src/prelude/`,
`cli/prelude.rs`, written from `specs/prelude.md`). They run larmorx's original and another program
(the replica, `larmorx-nc fsl prelude`, as a black box) and compare their outputs; no FSL program
runs here (FSL's runs go through the oracle cache in the parity suite `fsl-prelude`). Run them
with the workspace's `.venv` inside `scripts/orchestration/boxed.sh` (8 GiB).

| File | What it shows | How to run |
|---|---|---|
| `fmriprep_calls.py` | fMRIPrep 25.2.6's seven prelude calls (ds001600 three, ds002422 two, ds002603, ds005454) run with each program given, each output compared bit for bit with fMRIPrep's own unwrapped phase (data and header but `descrip`), with the wall time | `python -P fmriprep_calls.py <workspace>/oracles/fmriprep-25.2.6 <scratch> --program "<larmorx> fsl prelude" --program "<larmorx-nc> fsl prelude" [--threads N] [--mode reproduce] [--repeat 3]` |
| `edge_cases.py` | the validation plan's cases (§11) on generated inputs (59 cases: every option and mode, the ramps in every mode and on one slice, a neurological image, an int16 phase with a slope, series with `--start`/`--end` and a shorter mask series, complex64 in both byte orders, complex128, float32 and int16 given to `-c`, NaN magnitudes and phases, magnitudes of another size, the range error, `grot`, long names, output types), both D13 modes, two programs compared: exit status, standard output, standard error without larmorx's own lines, every file bit for bit | `python -P edge_cases.py <work dir> --a "<larmorx> fsl prelude" --b "<larmorx-nc> fsl prelude" [--only <prefix>] [--mode fix\|reproduce\|both]` |

What they gave (relay 1, 2026-11-06; the original built from this branch, the replica from main
1eb168f):

- `fmriprep_calls.py`: **7 of 7** bit-identical to fMRIPrep's own unwrapped phase (the default);
  one thread: ds005454 (112 × 112 × 70, 14,678 regions) 0.09 s (the replica 0.17 s), ds002603
  (128 × 128 × 128) 0.19 s (the replica 0.34 s), the others 0.01 to 0.06 s.
- `edge_cases.py`: 116 runs (58 cases × 2 modes), every one identical to the replica's but the
  default's three complex inputs (`complex`, `complex-be`, `complex128`), where the original's
  correctly rounded arctangent differs from glibc's `atan2f` in the last bit of some phases
  (GLIBC-07, named): the labels and the masks identical, every voxel's integer offset FSL's
  (`specs/prelude.md` §11's threshold), the unwrapped phase within 4.8e-7 rad.
- The spec writer's `blackbox.py` (run as a black box) against the original and the replica, the
  default: 58 of 59 cases' files identical (the complex case by GLIBC-07 as above), the reports
  differing only in larmorx's own lines and `descrip`; with `reproduce` (through a wrapper that
  sets `LARMORX_UPSTREAM_DEFECTS`, which the probe does not pass on): 59 of 59.
