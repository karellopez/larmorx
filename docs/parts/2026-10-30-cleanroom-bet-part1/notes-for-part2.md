# Clean-room BET, part 1: notes (for part 2)

Part 1 ran 2026-10-30 17:18-20:30 CET in the worktree
`<workspace>/larmorx/.claude/worktrees/agent-a8aa142043f423b7f` (branch from bed7e04). Helpers in
this scratch folder: `run.sh` (the environment, then exec), `fsl.sh`, `build.sh` (the wheel, its
`_core` copied into `python/larmorx/`), `py/` (scratch probes), `ph/` (copies of the suite's
phantoms), `nc/` (the random-testing night's inputs), `cmp/` (comparison runs).

## What part 1 built (all committed)
- `crates/larmorx-fsl` (clean-room, Apache-2.0, PROVENANCE): `bet::{mesh, params, surface, raster,
  skull, image, quatern}`, `cli::{options, bet2, bet, fmt}`. bet2 complete; bet's default, -R, -F,
  -Z, the pending -S/-B/-A/-A2; FSL-40/42/43/44/45/46/56/60/61/62/63 with `reproduce`.
- Wiring: `larmorx-cli` (registry `original: true` for bet and bet2; run_tool), `larmorx-py`
  (`_core.fsl_run_original`), `python/larmorx/fsl/bet.py` (`implementation="original"`; auto =
  replica when installed, else original), `_core.pyi`.
- Validation: `validation/.../parity/fsl_bet.py` `suite("original")` (specs/bet.md §11 thresholds;
  the quaternion within 2^-23; betsurf's cases left out), record `docs/validation/fsl-bet.md`;
  `tables.py` maps `fsl-bet` to `_bet`; difftest descriptor runs both implementations (betsurf
  skipped for the original); `tests/parity/test_memory_peak.py` (bet_original, 4 calls),
  `thread_pools.py` (3 programs); golden cases `cli.fsl.bet2`, `cli.fsl.bet2.options`,
  `cli.fsl.bet2.reproduce`, `cli.fsl.bet`, `cli.fsl.bet.variations`, `py.fsl.bet`.
- Docs: `docs/api/fsl-bet.md` (the original's section), `docs/findings/fsl-bet-original.md` (every
  black-box observation, for the spec writer), `docs/benchmarks/fsl-bet.md`,
  `validation/investigations/cleanroom-bet/` (README: every probe).

## Open (part 2 or the spec writer)
1. **Reruns** (§4.5): the first rerun differs from the spec's rule (real T1w: older-75y, 7t,
   tumour miss the thresholds). The factor 100 and the 1000 iterations are right (scans); what
   else differs is unknown. Probes needed a temporary build (findings, "Reruns").
2. **Skull** (§4.7): Dice ~0.9 (profile from the surface voxel, steepest rise's end sample).
3. **-e's exact test**: FSL flags small random heads that a segment-triangle test (with folds and
   coplanar overlap) does not (difftest #27).
4. Small/degenerate random heads: last digits of the self-intersection total (sum orders); one
   `thresh` digit (difftest #74).
5. `bet -F` on a float64 series: the values are read as float32 (FSL keeps double): last bits.
6. **betsurf's original** (spec §6) not started: the CLI registry keeps `original: false` for
   betsurf; `lx.fsl.betsurf(implementation="original")` raises NotImplementedError; the suite's
   original mode skips betsurf's cases (`_original_cases`); the difftest descriptor skips them.
   To add: `larmorx_fsl::cli::TOOLS` + "betsurf", `run_with`, registry `original: true`,
   `python/larmorx/fsl/bet.py` betsurf's original path (as bet's `_run_original`), the golden
   case (REPLICA_ONLY_FUNCTIONS then empty), `_original_cases` without the filter.
7. The site page's clean-room panels (part 2).

## Random testing
- Trial: `<workspace>/oracles/difftest/2026-10-30-cleanroom-bet` (log
  `oracles/logs/difftest-cleanroom-bet-2026-10-30.log`), 718 cases, seed 20261030; the replica
  agreed in every finished case; the original 334 of 573 as run. `retrial.py` re-runs a night's
  differing cases with the current build: 164 of 239 agree after the fixes (scratch
  `retrial2.txt`).
