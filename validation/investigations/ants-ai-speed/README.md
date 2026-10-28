# antsAI and antsAffineInitializer: memory and speed (2026-10-28)

The tools behind `docs/findings/ants-ai.md`, "Memory and speed (2026-10-28)", and the benchmark
`docs/benchmarks/ants-ai.md` (made by `python -m larmorx_validation bench ants-ai`, the module
`larmorx_validation.bench.ants_ai`). Run them with the repository's `python/` (with a built
`_core`) and `validation/src` on `PYTHONPATH`, with `python -P`, heavy runs in a memory box
(`scripts/orchestration/boxed.sh`).

| Tool | What it shows | How to run it |
|---|---|---|
| `measure.py` | wall, user and system time, CPU %, peak memory and voluntary and involuntary context switches of ANTs (CORE-MATH preloaded, one thread or `--threads`) and of larmorx's command in either mode, on the memory test's head or the 7 T MP2RAGE; larmorx's outputs compared with ANTs' byte for byte. It showed that ANTs stays on one thread (99 % of a CPU, 3 voluntary switches in 35 s) | `python -P measure.py <dir> ai-head\|init-head\|ai-7t\|init-7t ants reproduce fix [--threads N] [--repeats R] [--json out]` |
| `gdb_profile.py` | where larmorx's time goes without `perf` (refused to users here: `perf_event_paranoid` 4) and beyond py-spy (which sees only Python threads): gdb attaches to the command every `--interval` seconds and every thread's stack is counted by function | `python -P gdb_profile.py <out.json> [--interval 0.3] -- antsAffineInitializer 3 f.nii.gz m.nii.gz o.mat 15 0.1 0 10` (an extension built with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` shows inlined functions) |
| `bench_sampled.rs` | the sampled Mattes path's cost a point, value-only and with the derivative, and a checksum of its results (a change that alters a bit shows) | copy into `crates/larmorx-registration/tests/`, then `BENCH_THREADS=1 [BENCH_ROTATE=1] cargo test --release -p larmorx-registration --test bench_sampled -- --ignored --nocapture` |
| `compare_builds.py` | two builds of larmorx on every case of the antsAI and antsAffineInitializer parity tiers, in both D13 modes, the new one at several thread counts: exit status, both streams (every notice) and every file identical; a difference is tried again once (a run the machine stopped is printed as such) | `python -P compare_builds.py <dir> <old python/ tree> <new python/ tree> [--tier standard] [--threads 1,4] [--jobs N]` |
| `record_fields.py` | every field that differs between re-made parity records and the committed ones (compare_records prints the first five cases) | `python -P record_fields.py <dir of re-made records> ants-ai ants-affine-initializer` (from the checkout) |

Results (2026-10-28, the main machine): `compare_builds.py` with the build before the part
(`e50fcf9`) and the part's last code: 172 cases, 0 differences, in both modes, the new build at
1 and 4 threads; the micro-benchmark at one thread went from 57.6 to 33 ns a point (value) and
from 106 to 79 ns (derivative), turned 36 and 108 ns.
