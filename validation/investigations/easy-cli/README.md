# The easy command line: when a tool's messages reach the user

`stream_timing.py` runs one long, talkative command (`N4BiasFieldCorrection -v` on a generated
192³ image, 4 threads) through one or more `larmorx` command lines, reads its standard output and
error as pipes, and records when each line arrives: the run's duration, the first and last line's
time, the median line's time and the share of the lines that arrived in the first half of the
run.

```
python -P validation/investigations/easy-cli/stream_timing.py --work <scratch folder> \
    --larmorx "python -c 'import sys; from larmorx.cli import main; sys.exit(main())'" \
    --larmorx target/release/larmorx
```

(with `PYTHONPATH=<checkout>/python`, inside `scripts/orchestration/boxed.sh`; a `larmorx`
without the options, 0.1.0rc1, is run with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` instead).

## What it showed (2026-11-11, main machine)

| command line | run (s) | first line (s) | median line (s) | lines in the first half |
|---|---|---|---|---|
| console script, main `7cd35c8c` (0.1.0rc1's code) | 12.55 | 12.54 | 12.54 | 0 % |
| console script, this branch | 12.59 | 0.01 | 12.44 | 0.4 % |
| standalone program, this branch | 12.61 | 0.00 | 12.46 | 0.4 % |

0.1.0rc1's console script (`pip install larmorx`'s `larmorx`) collected the tool's output and
wrote it when the tool ended: nothing appeared for the whole run. The console script now passes
each piece to `sys.stdout` and `sys.stderr` as the tool writes it (`crates/larmorx-py/src/stream.rs`;
standard output line-buffered and standard error unbuffered, as the standalone program's
streams; a replica program's output read from its pipes as it comes,
`larmorx_cli::replica::Output::Stream`), and its lines arrive when the standalone program's do,
with the same bytes (`tests/python/test_cli_options.py`). (N4 itself writes most of its lines
during its last, full-resolution level, so most arrive near the end in both.)
