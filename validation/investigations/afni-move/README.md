# The AFNI replicas move into the main package (2026-10-29)

Next-steps item 3c: AFNI 26.3.05 releases the Medical College of Wisconsin's code under CC BY
4.0, so the bit-exact replicas of 3dTshift, 3dAutomask and 3dUnifize moved from the GPL package
`larmorx-gpl` (`crates-gpl/larmorx-gpl-afni`) into the main package
(`crates/larmorx-afni-replica`, Apache-2.0 with CC BY 4.0's attribution), and run inside larmorx.
What was found and done: [docs/findings/afni-relicensing-26.md](../../../docs/findings/afni-relicensing-26.md),
"The move (2026-10-29)"; the crate's provenance:
[PROVENANCE.md](../../../crates/larmorx-afni-replica/PROVENANCE.md).

## The C library's maths (task 1)

The tools are in `crates-gpl/tools/` (GPL-3.0-or-later: they hold the earlier glibc
translations as their reference); `crates-gpl/README.md` says how to run them. Their logs from
2026-10-29 are in `<workspace>/oracles/logs/afni-move-*.log`.

| Tool | What it shows | Result |
|---|---|---|
| `sincosf-verify` | the replica's `sinf`/`cosf`, translated from Arm's Optimized Routines v19.11 (MIT), on all 2^32 inputs, both x86-64 builds | 0 differ from the glibc translation and from glibc 2.35 (FMA build, and SSE2 build under `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`); the builds differ from each other on 12 and 22 inputs |
| `floatmath-verify` | the replica's `tanhf` (fdlibm) on all 2^32 inputs; the reference `cbrtf`'s hash | 0 differ; `c679cb92d4402e33`, glibc's |
| `cbrtf-measure` | what glibc's last bit of `cbrtf` changes in 3dUnifize `-EPI` (radius, message, ball) over every float32 voxel volume, and the replica's warning | the ball differs on 1,449,010 volumes, the message on 65,145,681 (from 0.001 mm³: 8,794 and 552); the replica warns on all of them (917,292,437 warned, 0 missed) |

`fetch_aor.sh` fetches Arm's sources the translation was made from (`<scratch>`; the files'
SHA-256 equal the version-afni part's copy), `fetch_ccby.sh` CC BY 4.0's legal code
(`LICENSES/CC-BY-4.0.txt`, from creativecommons.org).

## The move (task 2)

- `relicense_headers.py`: the one-off rewrite of the moved crate's licence headers (SPDX from
  what each file translates; CC BY 4.0's attribution; the request to cite; the modification).
- `ci_minutes.py`: the CI minutes of the `gpl` jobs' `crates-gpl` steps on the three newest
  runs of main (4.0 min per run of 14.7 to 16.1 for the six jobs), which the move removes; the
  replicas' tests now run in the `test` jobs' `cargo test` (their debug build and tests, a part of
  that time back).

## Validation (task 3)

- **Records:** the three replica suites' standard tiers re-made in-process from a clean commit
  (`python -m larmorx_validation parity afni-tshift|afni-automask|afni-unifize --implementation
  replica --tier standard`), compared case by case with the records made through `larmorx-gpl`
  (`scripts/orchestration/compare_records.py`): see the table below.
- **Golden:** the 40 cases and 90 outputs moved to `crates/larmorx-cli/tests/afni_replica_golden.rs`
  with their checksums unchanged (they pass on Linux x86-64 through the in-process replica).
- **Memory** (`memory.py`, the inputs of `tests/parity/test_memory_peak.py`'s `AFNI_CASES`, one
  thread, peak resident memory):

  | Program | AFNI 26.3.05 | `python -m larmorx` | `larmorx` binary | `larmorx-gpl` (before) |
  |---|---|---|---|---|
  | 3dTshift (BOLD 64×64×40×160) | 205.2 MiB | 121.0 | 107.4 | 106.6 |
  | 3dAutomask (its mean) | 7.5 | 21.2 | 8.2 | 7.0 |
  | 3dUnifize `-T2` (160×192×160) | 98.0 | 164.2 | 151.5 | 150.2 |
  | 3dUnifize `-EPI` (the BOLD) | 109.2 | 321.1 | 308.1 | 307.3 |

  The move adds nothing (the in-process replica is within 1.2 MiB of the separate program); the
  Python entry point adds its interpreter and numpy (about 13 MiB). Two calls exceed the memory
  rule, as before the move: 3dAutomask's small image through Python (its own footprint is
  AFNI's), and the replica's 3dUnifize (1.5 and 2.8 times AFNI's peak); strict expected failures
  in the test until a part fixes them (follow-ups).
- **Speed** (`speed.py`, the standard tiers' real cases): see the table below.

## Results

### Records (re-made in-process from b6c7066, compared with main's)

| Suite | Cases | Status differs | Details differ |
|---|---|---|---|
| `afni-tshift-replica` | 315 (280 pass, 32 rejected by both, 3 expected divergences) | 0 | 0 |
| `afni-automask-replica` | 125 (105, 19, 1) | 0 | 1: `divergences/default-prefix`'s reason names `larmorx` for `larmorx-gpl` |
| `afni-unifize-replica` | 116 (95, 19, 2) | 0 | 1: the same reworded reason |

The AFNI runs came from the oracle-output cache (624, 318 and 294 runs; 6, 4 and 5 ran again); only
larmorx ran. The parity smoke tier of the three suites: 374 passed. The thread probe
(`tests/parity/test_thread_pools.py`): 77 passed, the in-process replicas' three paths included
(no thread outside a call's own pool).

### Speed (`speed.py`, median of 3 after one untimed run; larmorx's default mode)

| Program | Threads | Cases | Median original / replica |
|---|---|---|---|
| 3dTshift | 1 | 20 | 0.95 |
| 3dTshift | 8 | 20 | 0.72 |
| 3dAutomask | 1 | 3 | 0.54 |
| 3dAutomask | 8 | 3 | 0.44 |
| 3dUnifize | 1 | 2 | 0.47 |
| 3dUnifize | 8 | 2 | 0.27 |

The ratio is the clean-room original's time over the replica's: the originals, built for speed,
are faster everywhere (3dTshift by 5 % at one thread and 28 % at eight; 3dAutomask about two times;
3dUnifize two to five times). `implementation="auto"` runs the replica (its documented rule,
unchanged); the per-case table is in `<workspace>/oracles/logs/afni-move-speed.log`.

### Random testing (a 30-minute trial, 2026-10-30)

`python -m larmorx_validation difftest --tools afni-tshift,afni-automask,afni-unifize --budget 0.5
--jobs 8` from the clean commit 6ee7765, in a 20 GiB box, into
`<workspace>/oracles/difftest/2026-10-30-afni-move/`: 13,971 cases (3dTshift 6,348, 3dAutomask
5,289, 3dUnifize 2,334), both implementations against AFNI 26.3.05: **no difference**. Besides the
agreeing cases: D13 notices that change nothing the input shows (AFNI-01, -16, -43, -45: the known
classes), AFNI-20's random identifier (44 cases, never comparable), and 14 3dUnifize `-EPI` cases
whose ball of neighbours needs more than the case's 3 GiB (AFNI-75's class, queued in `heavy.jsonl`;
the predictions there show the replica's -EPI ball at about 2.8 times AFNI's memory, the follow-up
above). Nothing new for `validation/difftest/triage.toml`.

Two earlier attempts the same night are set aside, never used: the first
(`2026-10-30-afni-move-invalid-extension-replaced/`) had larmorx's extension replaced under it
while it ran (every later case an exception in both implementations); the second
(`2026-10-30-afni-move-contaminated-overlap/`) overlapped the first one's end, whose report and
settings landed in its directory.
