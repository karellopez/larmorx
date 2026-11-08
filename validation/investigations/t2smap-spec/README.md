# t2smap-spec: the black-box facts and the model behind `specs/t2smap.md`

The spec writer's tools for tedana's `t2smap` spec (2026-11-08). They are written from the spec's
text and from runs of tedana's program, never from tedana's source: a clean-room implementer may
read and run them. Run them with a Python that has numpy and nibabel (the workspace's `.venv` or
`oracles/python-newest`), with `-I`, inside `scripts/orchestration/boxed.sh` (8 GiB is ample for
the probe).

| File | What it shows | How to run |
|---|---|---|
| `blackbox.py` | 54 cases of a `t2smap` program on inputs it generates (seeded numpy; a 16 × 14 × 8 × 6 three-echo phantom whose decay is a product of per-millisecond factors, so the inputs are the same on every machine; a dropout band, rising voxels, int16 with a scale factor, a z-concatenated file, a NaN and an infinite sample, a 3D file, a mask shifted by one voxel): the help, the version, every parser error met, the echo times' units and errors, both fits and modes, both combinations, no mask, every mask method, dummy scans and `--exclude` (valid and every error), `--n-independent-echos`, `--debug`, `--quiet`, existing outputs with and without `--overwrite`, abbreviations, threads. The report: each case's exit status, standard output and error (paths written `<W>` and `<site>`, a traceback's stack left out) and every file written with its header fields, value summary, gzip header, the table's first lines and the JSON files' keys. The source of the spec's **(observed)** facts | `python -I blackbox.py <work dir> <program> [<word>...] > report.txt`, e.g. the program `oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap` (tedana 26.0.3, newest environment) or `larmorx tedana t2smap`; then `diff` two reports. The run's environment is set by the script (the spec's §17.1: `COLUMNS=80`, `TQDM_DISABLE=1`, one BLAS thread, Haswell kernels, numpy's AVX-512 kernels off) |
| `model.py` | the spec's §§3–11 in numpy and SciPy as the spec states them (nibabel's float32 reading, the mask, the dummy scans and `--exclude`, the adaptive mask with §13.2's sum orders written out, numpy's `lstsq` per echo-count group, SciPy's `curve_fit` per voxel with §7.3's options explicit, the floors, the cap, the RMSE, the table, the combination, the files); NIfTI-1 single files, tedana's behaviour (`reproduce`), files only. `--variant fast`: the changes the spec's §16 allows (a per-voxel closed-form log-linear fit, an analytic Jacobian in the curve fit, other sum orders), to measure §17.3's margins | `python -I model.py [--variant exact\|fast] <t2smap's options> --out-dir <dir>`, in the reference setup (the environment `blackbox.py` sets) |
| `check.py` | the spec's §17.3 between two output directories: per output the values, the identical ones, those passing by each rule, the failures and the largest use of its allowance; exit 0 when all pass | `python -I check.py <tedana's out dir> <other out dir> [--cwd <the run's directory>] [--details] [--json <file>]` |
| `measure.py` | the runs behind §17.3 and §18: per case tedana in the reference setup, in the shared environment, with `--n-threads 4`, with numpy's AVX-512 kernels on, the model and its fast variant, each checked against the reference; the table of the largest uses | `python -I measure.py <work dir> --tedana <workspace>/oracles/inputs/tedana-t2smap-replica [--fmriprep <workspace>/oracles/fmriprep-25.2.6] [--jobs N] [--only <case>,...] [--check-only]`, with `oracles/python-newest`'s Python, boxed (24 GiB for fMRIPrep's calls at 8 jobs) |

What `blackbox.py` gave (2026-11-08, tedana 26.0.3 in `oracles/python-newest`; the replica
`larmorx-gpl` built from this branch at 9e5e57cd's crates, a clippy change since):

- tedana: 54 cases, the report the spec quotes (its help at 80 columns, the messages with their
  prefixes, the errors' last lines, the outputs' headers: sform code 2, qform code 0, `xyzt_units`
  0, NaN scale factors, the 4D images' voxel sizes from the first echo, gzip without name and
  mtime 0, the JSON files without a final newline).
- The replica with `LARMORX_UPSTREAM_DEFECTS=reproduce`: the same report except the Python
  warnings' location lines (the replica writes `tedana/decay.py:111` where tedana writes its
  installed path: decision D6), larmorx's NP-03 notices, the progress bars (the replica prints
  them whatever `TQDM_DISABLE` says) and gzip's compression-level byte.
- The replica's default: the corrections' error texts and notices the spec's §15 quotes.

What `model.py` and `measure.py` gave (2026-11-08, relay 8b; the reports in the part's scratch
folder, the log `oracles/logs/t2smap-1-relay8b-*.log`):

- The model's files are **bit-identical** to tedana 26.0.3's on all 30 of `blackbox.py`'s cases
  that tedana completes and on fMRIPrep's four curve-fit calls, ds003592's log-linear call and
  ds003592's with `--verbose` (images' headers and data, the table's text, the registry, the call
  file, the JSON files but for the host and Python fields).
- tedana's own spread (the shared environment, `--n-threads 4`, numpy's AVX-512 kernels on) and
  the model's fast variant pass every row of the spec's §17.3; the largest uses are the numbers
  in its brackets (e.g. the curve fit's (b)/(c) share 0.28 % / 0.31 % of the fit voxels, against
  3 %; the variance maps 9.9e-5 relative, against 1e-3).
- Two statements of the spec were made precise (the exemplars' tie-break sums the thirds; the
  RMSE map's sums are `nansum`), and the replica now honours `TQDM_DISABLE` as tqdm does.

