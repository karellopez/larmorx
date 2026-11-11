# t2smap-spec: the black-box facts and the model behind `specs/t2smap.md`

The spec writer's tools for tedana's `t2smap` spec (2026-11-08). They are written from the spec's
text and from runs of tedana's program, never from tedana's source: a clean-room implementer may
read and run them. Run them with a Python that has numpy and nibabel (the workspace's `.venv` or
`oracles/python-newest`), with `-I`, inside `scripts/orchestration/boxed.sh` (8 GiB is ample for
the probe).

| File | What it shows | How to run |
|---|---|---|
| `blackbox.py` | 54 cases of a `t2smap` program on inputs it generates (seeded numpy; a 16 × 14 × 8 × 6 three-echo phantom whose decay is a product of per-millisecond factors, so the inputs are the same on every machine; a dropout band, rising voxels, int16 with a scale factor, a z-concatenated file, a NaN and an infinite sample, a 3D file, a mask shifted by one voxel): the help, the version, every parser error met, the echo times' units and errors, both fits and modes, both combinations, no mask, every mask method, dummy scans and `--exclude` (valid and every error), `--n-independent-echos`, `--debug`, `--quiet`, existing outputs with and without `--overwrite`, abbreviations, threads. The report: each case's exit status, standard output and error (paths written `<W>` and `<site>`, a traceback's stack left out) and every file written with its header fields, value summary, gzip header, the table's first lines and the JSON files' keys. The source of the spec's **(observed)** facts | `python -I blackbox.py <work dir> <program> [<word>...] > report.txt`, e.g. the program `oracles/inputs/tedana-t2smap-replica/launcher-newest/t2smap` (tedana 26.0.3, newest environment) or `larmorx tedana t2smap`; then `diff` two reports. The run's environment is set by the script (the spec's §17.1: `COLUMNS=80`, `TQDM_DISABLE=1`, one BLAS thread, Haswell kernels, numpy's AVX-512 kernels off) |
| `model.py` | the spec's §§3–11 in numpy and SciPy as the spec states them (nibabel's float32 reading, the mask, the dummy scans and `--exclude`, the adaptive mask with §13.2's sum orders written out, numpy's `lstsq` per echo-count group, SciPy's `curve_fit` per voxel with §7.3's options explicit, the floors, the cap, the RMSE, the table, the combination, the files); every image form of the spec's §4.1 (NIfTI-1 and NIfTI-2 single files and pairs, ANALYZE 7.5, MGH/MGZ, converted as tedana converts them) and nilearn's masking of §4.4 (a 4D mask of one volume), tedana's behaviour (`reproduce`), files only. `--variant fast`: the changes the spec's §16 allows (a per-voxel closed-form log-linear fit, an analytic Jacobian in the curve fit, other sum orders), to measure §17.3's margins | `python -I model.py [--variant exact\|fast] <t2smap's options> --out-dir <dir>`, in the reference setup (the environment `blackbox.py` sets) |
| `check.py` | the spec's §17.3 between two output directories: per output the values, the identical ones, those passing by each rule, the failures and the largest use of its allowance; exit 0 when all pass. Each run's cap is read from its files as §17.3's cap row says (tedana's files cap only the limited map; a run with no full value above ten caps is read as larmorx's default, whose full map TED-05 caps too); two caps further apart than their allowance fail the cap's rows; the fit values whose log-linear observations are all equal (the spec's §7.2, open point 5) are counted in the T2* rows' notes, not judged apart | `python -I check.py <tedana's out dir> <other out dir> [--cwd <the run's directory>] [--details] [--json <file>]` |
| `recheck.py` | `check.py` again on output folders that already exist (those `../t2smap-original/compare.py` and `fmriprep_calls.py` leave), no program run, optionally beside another `check.py` (`--baseline`, e.g. a copy of an earlier version): per case the two verdicts and the failing rows, and the count of verdicts that changed; exit 1 when a case the baseline passes fails | `python -I recheck.py [--probe <work> <reference run> <other run>]... [--fmriprep <work> <reference run> <other run>]... [--baseline <old check.py>] [--jobs N] [--json <file>]`, boxed (28 GiB at 6 jobs for fMRIPrep's calls) |
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
  3 %; the variance maps 8.1e-6 relative where the parameters agree, against 1e-4).
- Two statements of the spec were made precise (the exemplars' tie-break sums the thirds; the
  RMSE map's sums are `nansum`), and the replica now honours `TQDM_DISABLE` as tqdm does.

What `recheck.py` gave (2026-11-11, the spec writer; log `oracles/logs/t2smap-cr-sw-recheck-*.log`):
the clean-room part's folders (the probe's cases and its added ones; fMRIPrep's five calls: the
original's `reproduce` against tedana at 1 and 8 threads, its default against the replica's
default) judged by `check.py` before and after TED-05's default cap was read from the files:
113 verdicts that passed still pass, every count and use the same but two variance uses, which
fell (the cap's voxels of a `--fitmode ts` case no longer judged there); the three default
comparisons that failed only at TED-05's capped voxels pass (ds003592 with and without
`--verbose`, 11 T2* values each, the caps 1413.787 / 1413.784 ms at use 0.0027 of their
allowance; ds000258, one S0 value, passing by (b) with F at the cap); three cases stay outside
the model (a mask of another grid, TED-01's NaN sample).

What spec writer 2 gave (2026-11-11; the random-testing trial of the clean-room original, its
findings replayed by `../t2smap-original/replay.py`; logs `oracles/logs/t2smap-cr-sw2-*.log`):

- `model.py` reads every image form of §4.1 and takes nilearn's masking path of §4.4, and two
  statements it did not follow are now followed (the RMSE map's voxels without a value are
  numpy's `nan`, the positive quiet NaN; the table's per-volume mean with one volume is a pairwise
  sum over the voxels): on the 202 replayed trial cases tedana completed (NIfTI-1 78 incl. a
  z-concatenated file and 4D masks of one volume, NIfTI-2 78, NIfTI-1 pairs 22, SPM2 ANALYZE 18,
  MGH/MGZ 34; computed masks among them) the model's files equal tedana 26.0.3's **bit for bit**
  (every image's header and data, NaN bits included, the table's text, the registry).
- `check.py` with that model, by `recheck.py` beside the earlier `check.py`: the clean-room part's
  probe folders (`cmp2` to `cmp5`): 116 verdicts that passed still pass, 17 cases it could not
  read now pass (ANALYZE, MGH, pairs, 4D masks), 1 FAIL and 1 ERROR unchanged; fMRIPrep's calls:
  unchanged (22 pass; 8 fail only on the empty `figures/` directory that runs replayed from the
  oracle-output cache lack; the variances' floor is never used there); the trial's 328 replayed
  cases ((A) and (B), 636 verdicts): 264 that it could not read pass, 24 passed and pass, 19 that failed pass
  (the variances' rounding floor), and **every failing verdict (79) is a case with equal
  observations** (the spec's open point 5), 14 of them newly failing because their caps differ
  beyond the cap row's allowance; 250 are runs that fail in tedana or in the model (nothing to
  judge). Log `oracles/logs/t2smap-cr-sw2-recheck-final.log`.

The measurements behind the spec's §7.2 and §13.4 on observations without a slope and the
variance row's rounding floor (spec writer 2, 2026-11-11). Each takes replayed trial cases
(`../t2smap-original/replay.py <finding> --keep <work>/f<n>`, so `<work>/f<n>/{tedana,reproduce,
default,replica}/out`) and runs with `oracles/python-newest`'s Python, `-I`, in the reference setup
(`OPENBLAS_NUM_THREADS=1 OPENBLAS_CORETYPE=Haswell`, numpy's AVX-512 kernels off), boxed:

| File | What it shows | How to run |
|---|---|---|
| `no_slope_cases.py` | per case, the fit voxels whose exact log-linear slope is 0 (rational arithmetic), how many have equal observations, the sign of numpy's batched slope there (tedana's call), the sign solved alone, and that tedana's written T2* follows from it | `python -I no_slope_cases.py <this folder> <work>` |
| `no_slope_signs.py` | numpy's batched slopes at the equal-observation voxels as JSON, to compare library setups (numpy 2.3.5's LAPACK, `OPENBLAS_CORETYPE=SkylakeX`, `Prescott`, `Zen`, more BLAS threads) | `python -I no_slope_signs.py <this folder> <work> <out.json>`, once per setup |
| `no_slope_places.py` | a column of equal observations placed at each place of batches of 1 to 8 columns, for 210 of tedana's designs: how often the place changes the slope and its sign | `python -I no_slope_places.py` |
| `fmriprep_no_slope.py` | fMRIPrep's calls: their equal-observation voxels (none) and their smallest slope against the rounding noise's scale | `python -I fmriprep_no_slope.py <this folder> <tedana's out dir>...` |
| `lstsq_groups.py` | a call's log-linear least-squares problems (the model's inputs, numpy's results) in the format of `../t2smap/numerics/dump_lstsq_calls.py`, to time `larmorx_transform::numpy_linalg::lstsq` with its `lstsq_check` (`LSTSQ_CHECK_TIMES=1`) | `python -I lstsq_groups.py <this folder> <tedana's out dir> <dump dir>` |
| `model_vs_tedana.py` | `model.py` on every replayed case tedana completed, its files against tedana's (headers and data bits, NaN bits apart, the table's text, the registry), summed by input form | `python -I model_vs_tedana.py model.py <work> <run name>` |
| `variance_floor.py` | the variance maps' failing values against the variance's rounding floor of §17.3 | `python -I variance_floor.py <this folder> <cases.txt: case ref other> <work>` |

