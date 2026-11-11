# t2smap-original: black-box reports for the clean-room `t2smap`

The clean-room part's own tools (2026-11-11), written from `specs/t2smap.md` and runs of the
programs only. They reuse the spec's probe (`../t2smap-spec/blackbox.py`: its inputs, cases,
environment and report form) and judge outputs with the spec's `check.py` (§17.3).

| File | What it shows | How to run |
|---|---|---|
| `compare.py` | one report per program on the probe's 54 cases and 26 added ones (a NaN observation of a one-echo voxel, an infinite first-echo sample, every voxel of one good echo, a single volume with both combinations, a voxel constant over time with PAID, a one-voxel mask, a 4D mask of one volume, a mask of another grid, a negative `--n-independent-echos`, a negative `--dummy-scans` with `--exclude`, a single infinite echo time, `--fitmode ts --debug`, the help at 40 columns; and, for the curve fit (relay 2b), the decay mask with `--debug --verbose` in both fit modes, one volume, an infinite sample, two echoes per volume, dummy scans with `--exclude`, no mask, PAID per volume, voxels with one good echo whose fits fail or converge in both fit modes (TED-08), every voxel of one good echo); with `--check <reference>` every completed case judged by `check.py` against that reference's outputs | `python -P compare.py <work> <name> <program words> [--env NAME=VALUE] [--check <reference name>]` inside `scripts/orchestration/boxed.sh` (12 GiB is ample), with `PYTHONPATH` holding `validation/src`; tedana's program (a path ending in `t2smap`) runs through the oracle-output cache. Then `diff <work>/<a>.txt <work>/<b>.txt` |
| `fmriprep_calls.py` | fMRIPrep 25.2.6's multi-echo calls (ds003592 sub-280; ds000210 sub-02 cuedSGT and rest and ds000258 sub-21262, the runs made again with OpenBLAS's Haswell kernels on 2026-11-09) with tedana, the replica and the original: (A) the original's `reproduce` against tedana, (B) its default against the replica's, judged by `check.py`, with each run's time | `python -P fmriprep_calls.py <work> --original <program> --replica "<program words>" --fittype curvefit --threads 1,8 [--runs ds003592,...] [--extra=--verbose]` inside `boxed.sh` (16 GiB) |
| `voxel.py` | one fit voxel in several runs' output folders: its count, samples, log-linear start, and per run the written T2*, S0, failure flag and §17.3's objective F | `python -I voxel.py <case folder> <index among the mask's voxels> <run name>...` |

The runs of 2026-11-11 (relay 2a): `tedana` (the launcher `oracles/inputs/tedana-t2smap-replica/
launcher-newest/t2smap`), `replica-default` and `replica-reproduce` (`larmorx-gpl` 0.1.0rc1
from PyPI, a black box), `original-reproduce` and `original-default` (the crate's example program
`t2smap_cleanroom`, `cargo build --release -p larmorx-tedana --example t2smap_cleanroom`). The
results are in the part's NOTES and, from relay 3 on, in the record.

The curve fit's runs of 2026-11-11 (relay 2b): the probe's cases and the added ones in `<scratch>/cmp2`
(and the one-echo-voxel cases alone in `cmp3`), fMRIPrep's four curve-fit calls at 1 and 8 threads;
the results in the part's NOTES.
