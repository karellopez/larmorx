# fsl-prelude-original: the clean-room original's checks (PRELUDE's clean-room part)

The clean-room implementer's tools for `larmorx fsl prelude` (`crates/larmorx-fsl/src/prelude/`,
`cli/prelude.rs`, written from `specs/prelude.md`). They run larmorx's original and another program
(the replica, `larmorx-nc fsl prelude`, as a black box) and compare their outputs; FSL's runs go
through the oracle cache (the parity suite `fsl-prelude`, `bench_calls.py`, `recheck.py`). Run them
with the workspace's `.venv` inside `scripts/orchestration/boxed.sh` (8 GiB).

| File | What it shows | How to run |
|---|---|---|
| `fmriprep_calls.py` | fMRIPrep 25.2.6's seven prelude calls (ds001600 three, ds002422 two, ds002603, ds005454) run with each program given, each output compared bit for bit with fMRIPrep's own unwrapped phase (data and header but `descrip`), with the wall time | `python -P fmriprep_calls.py <workspace>/oracles/fmriprep-25.2.6 <scratch> --program "<larmorx> fsl prelude" --program "<larmorx-nc> fsl prelude" [--threads N] [--mode reproduce] [--repeat 3]` |
| `edge_cases.py` | the validation plan's cases (§11) on generated inputs (59 cases: every option and mode, the ramps in every mode and on one slice, a neurological image, an int16 phase with a slope, series with `--start`/`--end` and a shorter mask series, complex64 in both byte orders, complex128, float32 and int16 given to `-c`, NaN magnitudes and phases, magnitudes of another size, the range error, `grot`, long names, output types), both D13 modes, two programs compared: exit status, standard output, standard error without larmorx's own lines, every file bit for bit | `python -P edge_cases.py <work dir> --a "<larmorx> fsl prelude" --b "<larmorx-nc> fsl prelude" [--only <prefix>] [--mode fix\|reproduce\|both]` |
| `bench_large.py` | speed and peak memory (`/usr/bin/time -v`) of each program on large generated maps (a wrapped smooth phase with noise over an ellipsoid; the noise sets how many regions there are), the original at several thread counts, its output compared bit for bit with the first program's | `python -P bench_large.py <work dir> --program "original=<larmorx> fsl prelude" --program "replica=<larmorx-nc> fsl prelude" --threads 1 --threads 14 [--noise 0.15] [--sizes 128x128x128,256x256x160]` |
| `bench_calls.py` | wall time of fMRIPrep's seven calls: FSL (through `oracle_cache.run` with a reason never to store it), the replica and the original at the thread counts given, each output bit for bit against FSL's (relay 2) | `boxed.sh 8 python -P bench_calls.py <scratch> --larmorx <larmorx> --replica <larmorx-nc> --threads 1 --threads 14 [--repeat 3] [--json out.json]` |
| `recheck.py` | every finding of a random-testing trial of `fsl-prelude-original` replayed with the current build (`difftest --replay`), the verdicts by the trial's signature (relay 2) | `boxed.sh 24 python -P recheck.py <trial dir> --jobs 6 --out <dir>` |

What they gave (relay 1, 2026-11-06; the original built from this branch, the replica from main
1eb168f):

- `fmriprep_calls.py`: **7 of 7** bit-identical to fMRIPrep's own unwrapped phase, by default and
  with `reproduce`, at 1 and 14 threads; ds005454 (112 × 112 × 70, 14,678 regions) 0.070 s at one
  thread, 0.057 s at 14 (the replica 0.17 s), ds002603 (128 × 128 × 128) 0.142 s and 0.108 s (the
  replica 0.34 s), the others 0.01 to 0.02 s.
- `edge_cases.py`: 116 runs (58 cases × 2 modes), 113 identical to the replica's; the other three
  are the default's complex inputs (`complex`, `complex-be`, `complex128`), where the original's
  correctly rounded arctangent differs from glibc's `atan2f` in the last bit of some phases
  (GLIBC-07, named): the labels and the masks identical, every voxel's integer offset FSL's
  (`specs/prelude.md` §11's threshold), the unwrapped phase within 4.8e-7 rad.
- `bench_large.py`, noise 0.15 rad: 128³ (66,020 regions) 0.31 s at one thread, 0.24 s at 14, peak
  86 MiB (the replica 1.27 s, 117 MiB); 256 × 256 × 160 (152,738 regions) 2.5 s and 2.2 s, 312 MiB
  (the replica 9.5 s, 469 MiB); noise 0.6 (1.76 million regions, an extreme case) 12 s (the
  replica 33 s). Every output identical to the replica's and across thread counts.
- The spec writer's `blackbox.py` (run as a black box) against the original and the replica, the
  default: 58 of 59 cases' files identical (the complex case by GLIBC-07 as above), the reports
  differing only in larmorx's own lines and `descrip`; with `reproduce` (through a wrapper that
  sets `LARMORX_UPSTREAM_DEFECTS`, which the probe does not pass on): 59 of 59.
- Against FSL through the oracle cache: the replica's parity suite `fsl-prelude-replica` (88
  cases), run as a black box with the original standing in for the replica's program: all agree
  (68 pass, 20 refused by both); its first run found the pair header's four extension bytes the
  original then lacked (352 bytes, as FSL writes them).
