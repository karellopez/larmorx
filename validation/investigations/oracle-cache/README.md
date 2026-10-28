# The oracle-output cache: its measurements and its proof

The tools behind "The proof, and the gain" in
[docs/validation/oracle-cache.md](../../../docs/validation/oracle-cache.md): every parity suite
run with the cache off, cold and warm, the records compared, the times and the original runs
counted. Run from the repository's checkout, with the environment of
`scripts/orchestration/env.sh` (the workspace's venv, the oracles, the test data), from a clean
commit (the records name it).

| Tool | What it does | How to run it |
|---|---|---|
| `run_suites.sh <checkout> <out> <tier> <use\|refresh\|off> [--jobs N] [suite ...]` | every parity suite (or those named; `name:replica` for the GPL replica) at one tier and in one mode: the records in `<out>/records[-replica]/`, a log per suite, `<out>/timings.tsv` (wall time, runs from the cache, runs that ran) | `LARMORX_ORACLE_CACHE=<cache> bash run_suites.sh . /tmp/m smoke off` |
| `measure.sh <checkout> <out> [--jobs N] [--skip-smoke] [--skip-standard]` | the whole measurement: the smoke tier off, cold, `oracle-cache confirm`, warm; `pytest tests/parity` off and warm; `ants-registration`, `ants-image-math`, `afni-tshift` (both implementations) and `mri-hmc` at the standard tier off, cold, confirm, warm; the comparisons (`same-*.txt`); one line per step in `<out>/steps.tsv`; caches of its own in `<out>/cache-smoke`, `<out>/cache-standard` | about 10 hours on a busy machine (2026-10-24: 15:42 to 02:24) |
| `remeasure.sh <checkout> <out>` | after the fixes of 2026-10-25: the corrections suite's smoke tier cold and twice warm, `pytest tests/parity` warm with `--durations=40` | about 35 minutes |
| `same_records.py <A> <B> [suite ...] [--volatile]` | whether two directories of records are the same apart from the cache's line, the JSON's `oracle_cache` and the timings; `--volatile` also masks memory addresses, temporary paths and the date of "Generated:", which change between any two runs | `python -P same_records.py /tmp/m/smoke-off/records /tmp/m/smoke-warm/records --volatile` |
| `merge_caches.py <into> <from>...` | copies caches' entries and their blobs into another (the workspace's), leaving out disputed entries and those written before the fixes of 2026-10-25 | `python -P merge_caches.py <workspace>/oracles/cache /tmp/m/cache-standard /tmp/m/cache-smoke` |
| `copy_entries.py <into> KEY... [--from <cache>]` | copies chosen entries (key prefixes, or `@file`: a verify log works) with their blobs (hard links) into a cache of their own, to run `oracle-cache verify` on them without writing the workspace's cache (2026-10-28: the 89 entries of the move's verify) | `python -P copy_entries.py /tmp/c89 @keys.txt`, then `python -m larmorx_validation oracle-cache --cache /tmp/c89 verify --keys @keys.txt` |
| `rekey_count.py [--cache <cache>] [--list]` | how many entries the corrections of 2026-10-28 re-key (a path with a blank, files named by a pattern, mcflirt's working directory, a path's text) or retire (ANTs' rules: ANTS-82, ANTS-07, ANTS-28), without running anything | `python -P rekey_count.py` |

The machine's load during a measurement is worth recording next to its times (the measurements
of 2026-10-24 sampled `/proc/loadavg` every minute; the table in the document gives the means).
