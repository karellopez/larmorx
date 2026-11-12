# t2smap-original: black-box reports for the clean-room `t2smap`

The clean-room part's own tools (2026-11-11), written from `specs/t2smap.md` and runs of the
programs only. They reuse the spec's probe (`../t2smap-spec/blackbox.py`: its inputs, cases,
environment and report form) and judge outputs with the spec's `check.py` (§17.3).

| File | What it shows | How to run |
|---|---|---|
| `compare.py` | one report per program on the probe's 54 cases and 26 added ones (a NaN observation of a one-echo voxel, an infinite first-echo sample, every voxel of one good echo, a single volume with both combinations, a voxel constant over time with PAID, a one-voxel mask, a 4D mask of one volume, a mask of another grid, a negative `--n-independent-echos`, a negative `--dummy-scans` with `--exclude`, a single infinite echo time, `--fitmode ts --debug`, the help at 40 columns; and, for the curve fit (relay 2b), the decay mask with `--debug --verbose` in both fit modes, one volume, an infinite sample, two echoes per volume, dummy scans with `--exclude`, no mask, PAID per volume, voxels with one good echo whose fits fail or converge in both fit modes (TED-08), every voxel of one good echo); with `--check <reference>` every completed case judged by `check.py` against that reference's outputs | `python -P compare.py <work> <name> <program words> [--env NAME=VALUE] [--check <reference name>]` inside `scripts/orchestration/boxed.sh` (12 GiB is ample), with `PYTHONPATH` holding `validation/src`; tedana's program (a path ending in `t2smap`) runs through the oracle-output cache. Then `diff <work>/<a>.txt <work>/<b>.txt` |
| `fmriprep_calls.py` | fMRIPrep 25.2.6's multi-echo calls (ds003592 sub-280; ds000210 sub-02 cuedSGT and rest and ds000258 sub-21262, the runs made again with OpenBLAS's Haswell kernels on 2026-11-09) with tedana, the replica and the original: (A) the original's `reproduce` against tedana, (B) its default against the replica's, judged by `check.py`, with each run's time | `python -P fmriprep_calls.py <work> --original <program> --replica "<program words>" --fittype curvefit --threads 1,8 [--runs ds003592,...] [--extra=--verbose]` inside `boxed.sh` (16 GiB) |
| `voxel.py` | one fit voxel in several runs' output folders: its count, samples, log-linear start, and per run the written T2*, S0, failure flag and §17.3's objective F | `python -I voxel.py <case folder> <index among the mask's voxels> <run name>...` |
| `casediff.py` | per case, the lines of one report that differ from another's (the check sections left out): how (A) and (B) were read line by line | `python -I casediff.py <a.txt> <b.txt> [case,...]` |
| `scipy_edges.py` | SciPy's `curve_fit` itself (a permissive library, called directly) on the start edge cases of §7.3: a T2* start of +∞ fails (forward differences NaN, refused by its SVD), −∞ and NaN starts are outside the bounds, a start of 0 or −0 converges at once at 1e-10 ms, S0 = +∞ gives residuals that are not finite | `python -I scipy_edges.py` (any Python with SciPy) |

The runs of 2026-11-11 (relay 2a): `tedana` (the launcher `oracles/inputs/tedana-t2smap-replica/
launcher-newest/t2smap`), `replica-default` and `replica-reproduce` (`larmorx-gpl` 0.1.0rc1
from PyPI, a black box), `original-reproduce` and `original-default` (the crate's example program
`t2smap_cleanroom`, `cargo build --release -p larmorx-tedana --example t2smap_cleanroom`). The
results are in the part's NOTES and, from relay 3 on, in the record.

The curve fit's runs of 2026-11-11 (relay 2b): the probe's cases and the added ones in `<scratch>/cmp2`
(and the one-echo-voxel cases alone in `cmp3`), fMRIPrep's four curve-fit calls at 1 and 8 threads;
the results in the part's NOTES.

## Relay 3a-ii (2026-11-11): benchmarks and random testing

| Script | What it shows | How to run |
|---|---|---|
| `bench.py` | wall times of fMRIPrep's five multi-echo calls: tedana once (fresh, through the oracle cache with a reason never to store it), the replica's default and the original in both modes at each thread count (best of `--repeat`), the original's files at every thread count identical to one thread's (`docs/benchmarks/tedana-t2smap.md`, "The clean-room original"; `tedana-t2smap-original.json`) | `python -P bench.py <work> --replica <larmorx-gpl> --threads 1,8 --repeat 2 --json <file>` (boxed) |
| `replay.py` | a random-testing finding of `tedana-t2smap-original` by hand: its inputs rebuilt from the recipe, tedana, the original in both modes and the replica, each one's exit status and standard error (tracebacks kept to their first and last lines, warnings without their location and source lines) | `python -P replay.py <finding.json> [--drawn] [--keep DIR] [--only tedana,reproduce,default,replica]` (boxed) |
| `mask_probe.py` | relay 3c's black-box probe (2026-11-11): the order of tedana's first reading's two errors (an echo both 3D and of another grid), nilearn's second reading across echoes (a 3D first echo before a second of another grid), 4D masks of one volume with other values than 0 and 1 (no nilearn refusal: tedana's `> 0`), masks of another grid with such values or empty (the empty mask's text first), an empty mask beside a 3D echo, and every voxel removed by the adaptive mask with each fit type and mode (TED-31): tedana 26.0.3 (oracle-output cache), the original in both modes and the replica, standard error as the contract keeps it | `python -P mask_probe.py <work dir> [--only <case>,...] [--runs tedana,reproduce,default,replica]` (boxed) |
| `classify.py` | a trial's findings by cause (the first rule matching a failed check's detail), with their signatures and examples: the categories of `validation/difftest/triage.toml`'s entries for the tool | `python3 classify.py <oracles/difftest/<trial>>` |
| `retriage.sh` | every finding of a trial replayed through the current descriptor (`difftest --replay`), its verdict line and reason: which findings the fixes and the harness's changes settle (relay 3c: all 50 of trial 2) | `bash retriage.sh <oracles/difftest/<trial>> > log` (boxed, `LARMORX_GPL_BIN` set) |
| `classify_replayed.py` | a trial's findings by cause after `retriage.sh`: the replay's reason (TED-46's expected divergence, the replica's gaps) or the failed checks' kind at the trial (a stopped run's partial outputs, TED-15's count, TED-01's infinite mean, the model's stop) | `python3 classify_replayed.py <oracles/difftest/<trial>> <retriage log>` |

## Relay C5 (2026-11-12): the site's typographer

| Script | What it shows | How to run |
|---|---|---|
| `typographer_hazards.py` | every text of an evidence part's JSON that the site renders as Markdown (the facts, summary, reasons, notes, titles, row heads and subs, cell texts, provenance), rendered as the site does (markdown-it with its typographer, the replacements and smart quotes), and each one whose rendering gains a sign (`(c)` ©, `(r)` ®, `(tm)` ™, `--` –, `+-` ±, `..` …) or holds a section the random-testing harness wrote as `§N.N`: in the part exported by relay C4, 29 (`(b)/(c)` in the reasons and the fMRIPrep table's note; `§N.N` in the trial table's row heads), none after relay C5 | `python3 typographer_hazards.py <part.json>` (markdown-it-py from the workspace's environment) |
