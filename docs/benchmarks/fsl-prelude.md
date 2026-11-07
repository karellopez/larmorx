# Benchmarks: prelude (`lx.fsl.prelude`, `larmorx fsl prelude`)

PRELUDE's clean-room original (`crates/larmorx-fsl`, modules `prelude` and `cli::prelude`) against
FSL 6.0.7.23's `prelude` and the bit-exact replica (`larmorx-nc fsl prelude`, FSL Licence, a
black box), on fMRIPrep 25.2.6's seven prelude calls and on large generated phase maps. Wall time
of the whole command (reading, the mask, the regions, the merging, writing).

- **Generated:** 2026-11-07 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), larmorx at commit `986bbc4` (PRELUDE's clean-room part, relay 2), inside memory boxes,
  with [`bench_calls.py`](../../validation/investigations/fsl-prelude-original/bench_calls.py)
  (`--threads 1 --threads 12 --repeat 5`: the best of five runs; FSL once; its runs:
  [fsl-prelude.json](fsl-prelude.json)) and
  [`bench_large.py`](../../validation/investigations/fsl-prelude-original/bench_large.py)
  (`--threads 1 --threads 12 --repeat 3`; FSL once, on the 128³ map only). **The machine was
  shared**: another agent (3dvolreg's replica) worked beside this one, and the parity smoke tier's
  memory tests ran during the runs (load average 6 to 7 of 32 threads), so every time may be a
  little long and the 12-thread times are noisy.
- **FSL was run again for this report**: fMRIPrep's calls through `oracle_cache.run` with a reason
  never to store them (a timed run is the one kind the cache cannot serve), the 128³ map directly.
  FSL's prelude runs on one thread.
- **The same result:** every output of every run checked bit for bit: the original's (default
  mode) and the replica's unwrapped phase against FSL's on fMRIPrep's calls (data and header but
  `descrip`); on the large maps each program's against the first's, the original at 1 and 12
  threads alike (the same bits at any thread count).

## fMRIPrep's prelude calls (wall time; in brackets the speed-up against FSL)

| Oracle run (grid, regions) | FSL | replica (1) | original (1) | original (12) |
|---|---|---|---|---|
| ds002603-sub-01 (128 × 128 × 128) | 28.44 s | 0.374 s (76×) | 0.135 s (211×) | 0.103 s (276×) |
| ds005454-sub-16 (112 × 112 × 70, 14,678 regions) | 6.68 s | 0.173 s (39×) | 0.069 s (97×) | 0.053 s (126×) |
| ds001600-sub-1, three calls (64 × 64 × 44) | 0.081 to 0.090 s | 0.016 to 0.018 s | 0.014 to 0.016 s | 0.013 to 0.015 s |
| ds002422-sub-02, two calls (64 × 64 × 24, 64 × 64 × 36) | 0.060 to 0.068 s | 0.009 to 0.014 s | 0.010 to 0.012 s | 0.011 s |

On the small calls every program's time is mostly its start and its reading and writing. On the
two large ones the original is 2.5 to 2.8 times the replica's speed on one thread.

## Large generated maps (a wrapped smooth phase with noise 0.15 rad over an ellipsoid)

| Map (regions) | FSL | replica (1) | original (1) | original (12) |
|---|---|---|---|---|
| 128 × 128 × 128 (66,020) | 183.7 s, 64 MiB | 1.36 s, 117 MiB | 0.30 s, 59 MiB | 0.24 s, 75 MiB |
| 256 × 256 × 160 (152,738) | not run | 9.41 s, 469 MiB | 1.97 s, 210 MiB | 2.56 s, 211 MiB |

(Peak resident memory by `/usr/bin/time -v`.) FSL's merging scans every constraint for each
choice (`O(L²)` per volume, `specs/prelude.md` §10): 66,020 regions take it three minutes, where
the original takes 0.3 s (610×). The original's peaks were re-measured after its memory
change of relay 3 (`a85f764`: no copy of the phase or the masks where none is needed, the
unwrapped phase allocated after the merging, the constraints' map freed before the merge's lists
are built; the same bits, the times unchanged in substance and not re-measured on the loaded
machine): on the 128³ map 59 MiB at one thread (78 before) against FSL's 64 MiB, within the memory
rule (1.1 × 64 + 8 = 78.4 MiB); at 12 threads 75 MiB (the threads' own buffers); on the 256 × 256
× 160 map 210 MiB (272 before). `tests/parity/test_memory_peak.py` measures fMRIPrep's call and
the 128³ map (as it is and with `-s`) through Python against FSL's peaks
([findings](../findings/fsl-prelude-original.md), section 7). On the 256 × 256 ×
160 map the 12-thread run was slower than the 1-thread run while the machine was loaded.

## Where the time goes, and why it is faster

The merging (`prelude::merge`): FSL looks for the largest merit by scanning every constraint,
for the members of the group that joins by scanning every region, and for its neighbours by
scanning every region number. The original keeps the constraints in slots reused by the merges,
an indexed 4-ary heap keyed by (merit descending, the pair ascending) updated in place, each
group's list of constraint slots (scanned for the constraint between two groups) and a weighted
union-find of the regions' whole turns: a choice is a heap operation, a merge costs the
neighbours of the group that joins. The order of the choices and each merge's arithmetic are
FSL's (§4.8), so the bits are. The linear parts (the mask's threshold and component, the regions,
the ramps, reading and writing) run on the call's threads; the merging is sequential (its order
is the result). The replica keeps FSL's structures with ordered sets (its own design, the same
bits).
