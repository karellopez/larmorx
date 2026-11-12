# Benchmarks: t2smap (`lx.tedana.t2smap`, `larmorx tedana t2smap`)

The bit-exact replica of tedana 26.0.3's `t2smap` (`larmorx-gpl tedana t2smap`, the GPL package's
program) against tedana 26.0.3 itself, on fMRIPrep 25.2.6's multi-echo `t2smap` calls (their
`t2smap_node` working directories: the resampled echoes, the dilated BOLD mask, the echo times in
ms, `--fittype curvefit`; ds003592's call again with `--fittype loglin`). Wall time of the whole
command (reading, the masks, the fit, the maps, the combination, writing).

- **Generated:** 2026-11-08 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), larmorx at commit `f3a401bc` with the benchmark's script fix (the part's relay 7b), inside a memory box, with
  [`bench_calls.py`](../../validation/investigations/t2smap/bench_calls.py) (`--threads 14 --repeat
  2`: the best of two runs for the replica; tedana once; the numbers:
  [tedana-t2smap.json](tedana-t2smap.json)). **The machine was shared**: another agent worked
  beside this one (load average 8 of 32 threads at the end), so every time may be a little long and the
  14-thread times are noisy.
- **tedana** in the newest kept environment (numpy 2.5.4, SciPy 1.18.1; OpenBLAS on its Haswell
  kernels, numpy's AVX-512 kernels off, one BLAS thread, tedana's default `--n-threads 1`), run
  through `oracle_cache.run` with a reason never to store it (a timed run is the one kind the cache
  cannot serve).
- **The same result:** every `reproduce` run's images and confounds table identical to tedana's
  (every value); the default's runs at 14 threads identical to its run at one thread.
- **The replica's two modes** compute the same way: each runs the stages from the mask to the
  combination again with the other mode's maths functions, writing nothing, to name NP-03 and
  GLIBC-07 exactly where they change a value (the other mode's functions altogether, then each
  family alone where they did: three more passes on these calls). The clean-room original is the fast
  implementation (its section below).

## fMRIPrep's calls (wall time; in brackets the speed-up against tedana)

| fMRIPrep's call | tedana 26.0.3 | replica, reproduce (1) | reproduce (14) | default (1) | default (14) |
|---|---|---|---|---|---|
| ds000210-sub-02-cuedSGT-curvefit | 29.2 s | 31.0 s (0.9x) | 8.9 s (3.3x) | 32.6 s (0.9x) | 9.5 s (3.1x) |
| ds000210-sub-02-rest-curvefit | 46.2 s | 44.7 s (1.0x) | 13.3 s (3.5x) | 44.9 s (1.0x) | 13.1 s (3.5x) |
| ds000258-sub-21262-rest-curvefit | 32.2 s | 42.2 s (0.8x) | 10.8 s (3.0x) | 41.2 s (0.8x) | 10.4 s (3.1x) |
| ds003592-sub-280-rest-curvefit | 44.3 s | 42.1 s (1.1x) | 10.6 s (4.2x) | 42.2 s (1.1x) | 10.6 s (4.2x) |
| ds003592-sub-280-rest-loglin | 6.1 s | 10.0 s (0.6x) | 7.9 s (0.8x) | 9.8 s (0.6x) | 7.9 s (0.8x) |

**What the times show.** Both of the replica's modes take the same time: each names the maths
defects by running the stages from the mask to the combination again (on these calls up to four passes
in all: the computation, the other mode's functions altogether, then each family alone, since both
families change values here: NP-03 and GLIBC-07 are named on every curvefit call, NP-03 on the
log-linear one). One pass of a curve-fit call is about a quarter of these times (relay 7a
measured one pass at 11 s on ds003592), so at one thread the replica is about as fast as tedana on
the curve-fit calls (0.8 to 1.1 times its speed) and slower on the log-linear one (tedana 6 s, the
replica's four passes 10 s); at 14 threads it is 3 to 4 times tedana's
speed on the curve-fit calls (the fits are parallel; tedana's `--n-threads 14` would add joblib
processes, not measured here). The exact notices cost the extra passes: the notices could be
named with the other mode's functions altogether only (two passes: about half the time) at the
price of naming both families together, without telling which one changes what; the decision is
the orchestrator's. Memory (one pass's arrays at a time) is in `tests/parity/test_memory_peak.py`:
545 MiB against tedana's 1,243 MiB on ds003592's curvefit call.

Defects the default names on these calls: GLIBC-07 and NP-03 (the maths), TED-05 (the full T2*
map's cap) and, on three of the four curvefit calls, TED-08 (failed fits of voxels with one good
echo).

## The clean-room original (2026-11-11)

The clean-room original (`crates/larmorx-tedana`, the main package; through its own binding,
`larmorx._core.tedana_t2smap_cleanroom`, run as a program with `python -c`, as before its wiring
into `larmorx tedana t2smap` of 2026-11-12: the same code) beside tedana and the replica, all three measured again together on the
same calls and inputs.

- **Generated:** 2026-11-11 on the main machine (Linux x86_64, AMD Ryzen 9 7945HX, 16 cores / 32
  threads), larmorx at commit `3d934964`, inside a 16 GiB memory box, with
  [`bench.py`](../../validation/investigations/t2smap-original/bench.py) (`--threads 1,8 --repeat
  2`: the best of two runs for the replica and the original, tedana once; the numbers:
  [tedana-t2smap-original.json](tedana-t2smap-original.json)). **The machine was shared**: two
  other agents worked beside this one (load average 14.9 of 32 threads at the start, 3.8 at the
  end), and this part's budget was 9 threads, so the many-thread columns are at 8 threads, not 14.
- **tedana** as above (one thread), run fresh through `oracle_cache.run` with a reason never to
  store it; **the replica** `larmorx-gpl` 0.1.0rc1 from PyPI, by default (a black box).
- **The same result:** the original's files at 8 threads identical to its files at one thread, in
  both modes, on every call (every image's values and the table, decompressed); its `reproduce`
  against tedana and its default against the replica's are judged on these calls by the record
  [`tedana-t2smap-original`](../validation/tedana-t2smap-original.md) (§17.3's thresholds).

| fMRIPrep's call | tedana 26.0.3 | replica default (1) | replica default (8) | original, reproduce (1) | reproduce (8) | default (1) | default (8) |
|---|---|---|---|---|---|---|---|
| ds000210-sub-02-cuedSGT-curvefit | 27.5 s | 30.2 s (0.9x) | 10.5 s (2.6x) | 2.73 s (10.1x) | 1.44 s (19.1x) | 2.72 s (10.1x) | 1.43 s (19.3x) |
| ds000210-sub-02-rest-curvefit | 49.0 s | 44.7 s (1.1x) | 14.8 s (3.3x) | 3.66 s (13.4x) | 1.98 s (24.8x) | 3.67 s (13.4x) | 1.95 s (25.1x) |
| ds000258-sub-21262-rest-curvefit | 34.0 s | 44.3 s (0.8x) | 13.0 s (2.6x) | 3.01 s (11.3x) | 1.35 s (25.2x) | 2.92 s (11.7x) | 1.33 s (25.6x) |
| ds003592-sub-280-rest-curvefit | 47.1 s | 43.4 s (1.1x) | 12.6 s (3.7x) | 3.16 s (14.9x) | 1.56 s (30.2x) | 3.05 s (15.4x) | 1.56 s (30.2x) |
| ds003592-sub-280-rest-loglin | 6.2 s | 10.1 s (0.6x) | 8.40 s (0.7x) | 3.05 s (2.0x) | 1.53 s (4.1x) | 3.04 s (2.1x) | 1.52 s (4.1x) |

**What the times show.** The original takes 2.7 to 3.7 s at one thread on the curve-fit calls,
10 to 15 times tedana's speed (and the replica's), and 1.3 to 2.0 s at eight threads, 19 to 30
times tedana's; on the log-linear call 3.0 s and 1.5 s (2 and 4 times tedana's 6.2 s). Both modes
take the same time: the original makes one pass (§15.2's rule; the replica's extra passes name
the maths notices exactly). It is bound by the files: on ds003592's curve-fit call at one / eight
threads, reading and inflating the three echoes takes 1.62 / 0.92 s, writing the combined series
0.69 / 0.32 s, the RMSE, its map and the table 0.44 / 0.32 s, the log-linear fit 0.10 / 0.02 s
and the curve fit itself 0.09 / 0.014 s (a temporary phase timer of the part's relay 2b, not
committed): the fit costs O(echoes) per evaluation on the per-echo means, not O(echoes × volumes).
The curve-fit and log-linear calls of ds003592 take the same time for that reason. Memory on that
call: 307 MiB at one thread and 546 MiB at eight (three echoes decoded at once), against tedana's
1,244 MiB and the replica's 545 MiB (`tests/parity/test_memory_peak.py`).
