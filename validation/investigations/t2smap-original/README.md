# t2smap-original: black-box reports for the clean-room `t2smap`

The clean-room part's own tools (2026-11-11), written from `specs/t2smap.md` and runs of the
programs only. They reuse the spec's probe (`../t2smap-spec/blackbox.py`: its inputs, cases,
environment and report form) and judge outputs with the spec's `check.py` (§17.3).

| File | What it shows | How to run |
|---|---|---|
| `compare.py` | one report per program on the probe's 54 cases and 14 added ones (a NaN observation of a one-echo voxel, an infinite first-echo sample, every voxel of one good echo, a single volume with both combinations, a voxel constant over time with PAID, a one-voxel mask, a 4D mask of one volume, a mask of another grid, a negative `--n-independent-echos`, a negative `--dummy-scans` with `--exclude`, a single infinite echo time, `--fitmode ts --debug`, the help at 40 columns); with `--check <reference>` every completed case judged by `check.py` against that reference's outputs | `python -P compare.py <work> <name> <program words> [--env NAME=VALUE] [--check <reference name>]` inside `scripts/orchestration/boxed.sh` (12 GiB is ample), with `PYTHONPATH` holding `validation/src`; tedana's program (a path ending in `t2smap`) runs through the oracle-output cache. Then `diff <work>/<a>.txt <work>/<b>.txt` |

The runs of 2026-11-11 (relay 2a): `tedana` (the launcher `oracles/inputs/tedana-t2smap-replica/
launcher-newest/t2smap`), `replica-default` and `replica-reproduce` (`larmorx-gpl` 0.1.0rc1
from PyPI, a black box), `original-reproduce` and `original-default` (the crate's example program
`t2smap_cleanroom`, `cargo build --release -p larmorx-tedana --example t2smap_cleanroom`). The
results are in the part's NOTES and, from relay 3 on, in the record.
