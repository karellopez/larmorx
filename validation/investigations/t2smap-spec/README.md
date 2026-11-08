# t2smap-spec: the black-box facts and the model behind `specs/t2smap.md`

The spec writer's tools for tedana's `t2smap` spec (2026-11-08). They are written from the spec's
text and from runs of tedana's program, never from tedana's source: a clean-room implementer may
read and run them. Run them with a Python that has numpy and nibabel (the workspace's `.venv` or
`oracles/python-newest`), with `-I`, inside `scripts/orchestration/boxed.sh` (8 GiB is ample for
the probe).

| File | What it shows | How to run |
|---|---|---|
| `blackbox.py` | 54 cases of a `t2smap` program on inputs it generates (seeded numpy; a 16 × 14 × 8 × 6 three-echo phantom whose decay is a product of per-millisecond factors, so the inputs are the same on every machine; a dropout band, rising voxels, int16 with a scale factor, a z-concatenated file, a NaN and an infinite sample, a 3D file, a mask shifted by one voxel): the help, the version, every parser error met, the echo times' units and errors, both fits and modes, both combinations, no mask, every mask method, dummy scans and `--exclude` (valid and every error), `--n-independent-echos`, `--debug`, `--quiet`, existing outputs with and without `--overwrite`, abbreviations, threads. The report: each case's exit status, standard output and error (paths written `<W>` and `<site>`, a traceback's stack left out) and every file written with its header fields, value summary, gzip header, the table's first lines and the JSON files' keys. The source of the spec's **(observed)** facts | `python -I blackbox.py <work dir> <program> [<word>...] > report.txt`, e.g. the program `oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap` (tedana 26.0.3, newest environment) or `larmorx tedana t2smap`; then `diff` two reports. The run's environment is set by the script (the spec's §17.1: `COLUMNS=80`, `TQDM_DISABLE=1`, one BLAS thread, Haswell kernels, numpy's AVX-512 kernels off) |
| `model.py` (relay 8b) | the spec's §§4–11 in numpy and SciPy, compared with tedana's files | to come |
| `check.py` (relay 8b) | the spec's §17.3 thresholds between two output directories | to come |

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
