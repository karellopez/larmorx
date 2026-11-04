# The oracle cache's tests under pytest-xdist (2026-11-04)

Run serially (as `gates-python.sh` runs them), every test of `validation/tests` passes. Run on 16
pytest-xdist workers, one or two tests of `validation/tests/test_oracle_cache.py` fail, different
ones in each run: in each failure a run of the fake original program that the test expects the
cache to store is not stored (`(e,) = entries(cache)` finds none, or `.cached` is false on the
second call). The same happens on main `0f10327`, so it does not come from BET's
`-S`/`-B`/`-A`/`-A2` part:

| Checkout | Run | Failed |
|---|---|---|
| main `0f10327` | `-n 16` | `test_a_rewritten_input_is_replayed_onto_this_runs_input`, `test_a_command_not_found_is_never_stored_nor_served` (159 passed, 1 skipped) |
| main `0f10327` | serial | none (161 passed, 1 skipped: a scratch checkout has no workspace with `oracles/` above it) |
| the part's branch at `d7d2c448` | `-n 16` | `test_stdout_bytes_are_what_subprocess_text_mode_gives` (161 passed) |
| the part's branch, relay 6's run | `-n 16` | `test_a_peak_is_measured_kept_and_served`, `test_a_command_not_found_is_never_stored_nor_served` |
| the part's branch, relay 6's run | serial | none (162 passed) |

Not investigated further: the cache decides whether to store a run from the files it wrote and
their sizes, times and inodes (`oracle_cache.core._snapshot`, `_changes`) and from how the run
ended; under the load of 16 workers one of those conditions seems to come out otherwise. Until
it is understood, run `validation/tests` serially.

    run_n16.sh <checkout> <log>     # one run on 16 workers; prints the failures and the counts
