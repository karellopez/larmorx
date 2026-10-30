# HANDOFF: BET's clean-room original, part 2a (2026-10-30)

Branch `worktree-agent-a7cce9b972a4c0b86` (from e90a5e0). Text for the shared documents; the
orchestrator integrates and harmonises it.

## Already in the branch (mechanical, so the gates pass)

- `validation/tools.toml`: fsl-bet's `validation` line ("brains, masks, outlines, meshes and skulls
  FSL's bit for bit, the surfaces that rerun after a self-intersecting first pass included"
  instead of "the reruns open, a question for the spec writer") and its record's `status` line
  (the clause "the surfaces whose first pass self-intersects" removed); `python -m
  larmorx_validation tables` regenerated the README's tool table and the validation index from
  them (commit 786e708). Reword freely at the merge.
- `tests/golden/golden.json`: re-recorded by `scripts/record_golden.py` from 786e708 (the parity
  smoke tier passing there: 2,607 tests, 0 failed; marker
  `oracles/cache/markers/parity-smoke-786e708229e6….json`): BET's four cases `cli.fsl.bet`,
  `cli.fsl.bet.variations`, `cli.fsl.bet2`, `py.fsl.bet` changed (21 outputs: their surfaces follow
  §8.1's arithmetic now); every other checksum unchanged; `record_golden.py --check` with the
  final build (deeed3f): 164 of 164 agree.
- `LICENSES/`: `larmorx_core::math::tanh_glibc` carries `SPDX-License-Identifier: Apache-2.0 AND
  LicenseRef-fdlibm` and Sun's notice in the file (check_spdx passes); add a
  `LICENSES/LicenseRef-fdlibm.txt` (Sun's notice: "Copyright (C) 1993 by Sun Microsystems, Inc.
  All rights reserved. Developed at SunSoft, a Sun Microsystems, Inc. business. Permission to use,
  copy, modify, and distribute this software is freely granted, provided that this notice is
  preserved.") and a NOTICE line if the package's licence files list every family.

## The defect register (`docs/findings/upstream-defects.md`): a new summary row, behaviour only

| ID | Class | Tool | What | larmorx |
|---|---|---|---|---|
| GLIBC-06 | d | glibc 2.35 (`tanh`, double) | `tanh` is not correctly rounded: on 21.3 % of a random sample of arguments (805 million, log-uniform and uniform up to 25) and 22,506 of CORE-MATH's 62,880 worst cases its result is one unit (rarely two) in the last place from the exact value. FSL's `bet2` takes its smoothness weight from it at every vertex of every iteration, and the reruns of a surface whose first pass self-intersects amplify that last bit into other surfaces and masks (specs/bet.md §4.5, §8.1) | `reproduce`: glibc's bits (`larmorx_core::math::tanh_glibc`, a port of fdlibm's `tanh`/`expm1`, proved equal to glibc 2.35's on 12.9 billion inputs). Default: **the same for now** (part 2a); part 2b makes the correctly rounded `larmorx_core::math::tanh` (CORE-MATH, ported and verified in 2a) the default with a notice where it changes the result: **D13 gap until then** |

`larmorx_core::defects::GLIBC_06` is in the branch (8abb5b4) with that behaviour text, and the
notice's machinery is wired and tested by hand: with `Deformation::tanh = CorrectlyRounded`,
`deform` checks at every step whether glibc's `tanh` would give another smoothness weight; where
it would, bet2 finds the surface again with glibc's and compares every output asked for (verbose
lines, brain/mask voxels, outline voxels, skull voxels, the mesh file's text), naming GLIBC-06
only when one changes, e.g. on the tumour: `larmorx: upstream defect GLIBC-06 (glibc 2.35)
corrected: the surface's smoothness weight uses the correctly rounded tanh; with glibc 2.35's,
which FSL calls, these outputs would differ: 24874 of 9437184 brain-mask voxels, the mesh file
(the surface: 2 pass(es) against 2, vertices up to 8.0201 mm apart) [LARMORX_UPSTREAM_DEFECTS=
reproduce gives glibc 2.35's result]`; on ds000005's T1w nothing is named (its surfaces differ
by under a micrometre, no output changes). **bet2 still uses glibc's `tanh` in both modes** (one
line, `crates/larmorx-fsl/src/cli/bet2.rs`, `tanh: bet::surface::Tanh::Glibc`): see the decision
below.

## `docs/api/upstream-defects.md`

- **GLIBC-06** (glibc 2.35's `tanh`, class d): the C library's double-precision hyperbolic tangent
  is not correctly rounded (about one argument in five gets the last bit wrong). FSL's BET uses it
  for the surface's smoothness weight, and where a surface self-intersects and is rerun, those
  last bits grow into a different surface. larmorx reproduces glibc's bits with `upstream_defects=
  "reproduce"`; the default uses them too until BET's clean-room part 2b switches it to the
  correctly rounded `tanh` with a warning where that changes the result.

## `docs/next-steps.md`, item 7 (BET's clean-room original)

- **Part 2a done (2026-10-30):** the spec writer's answers implemented: the deformation's arithmetic
  of §8.1 operation by operation (mesh in the file's order, §4.4's neighbour order, divisions,
  the update's sum order, the inward depth accumulated, the float products, the single centre
  sum, the robust range's exact arithmetic), the `-e` sweep and predicate (§4.5), the skull search
  (§4.7), the quaternion's precision path (§8.2, x87 steps), a name with an extension falling back
  to the search (§3.1), `bet -F`'s float64 brain (§5.2); glibc's `tanh` as
  `larmorx_core::math::tanh_glibc` (fdlibm from netlib, proved on 12.9 billion inputs) and
  CORE-MATH's correctly rounded `larmorx_core::math::tanh` (ported, bit-identical to the C on its
  worst cases and 805 million inputs). Record re-made: **all 142 standard-tier cases agree** (111
  of 111 compared within thresholds; the three rerun cases, `real/older-75y`, `7t`, `tumour`,
  bit-identical; 13 of 13 skulls bit-identical; the quaternion exact). The trial's 75 open cases:
  **73 agree**; 237 of its 239 differing cases.
- **Part 2b (next):** betsurf's original (spec §6) and the site page's clean-room panels (as
  planned); **the decision on BET's default `tanh` (GLIBC-06), for the user** (the rules do not
  settle it: the user's principle of 2026-10-30, the most accurate maths by default, against
  rule C, a correction measured to bring no gain, and the speed goal). Measured in 2a
  (`crates/larmorx-fsl/examples/tanh_effect.rs`, bet2's defaults, the standard tier's 13 real
  images): on 9 the two surfaces agree within a micrometre and every output is the same; on 4 the
  masks differ: the three rerun cases (Dice 0.9959, 0.9848, 0.9920; vertices 5.7 to 8.0 mm apart)
  and ds000005's BOLD first volume (0.9966). Where they differ the correctly rounded result is not
  more accurate (the reruns amplify any last bit): it is another, equally valid surface, away from
  FSL's. The exact notice needs the second search in practically every run (some weight's last
  bit differs almost always): **the default 1.9 times slower** (ds000005's T1w 0.43 → 0.83 s,
  ds002422's 1.20 → 2.33 s on one thread; 0.17 → 0.26 s and 0.34 → 0.63 s on 24; still faster
  than FSL's 1.3 and 3.2 s). Options: (a) the correctly rounded default with the exact notice (the
  machinery is ready: one line); (b) the same with a cheaper, inexact notice (e.g. whenever a
  weight differs: in practically every run); (c) glibc's bits by default (rule C), the correctly
  rounded `tanh` on request. **Recommendation: (c)**, as the branch has it now: FSL's surface by
  default where the choice is arbitrary, no slowdown, and nothing of the user's principle lost
  where it matters (the exact maths is one switch away and documented). Also
  remove `libm` from `crates/larmorx-fsl/Cargo.toml` (unused since 2a; the three lockfiles change)
  and re-run the benchmark (`docs/benchmarks/fsl-bet.md` is part 1's; a quick check in 2a: FSL
  1.27 / 3.21 s, the original 0.43 / 1.20 s on one thread and 0.17 / 0.34 s on 24, ds000005's and
  ds002422's T1w: as in part 1).
- **For the spec writer (one question):** FSL's conversion of a centre between the voxel frames of
  a neurologically stored image (`bet2 -c` and `fslstats -C`: a 4 x 4 inverse, §8.1 leaves its
  last bits open). Random-testing cases 677 (`-c`: another first-pass surface) and 622 (`bet -R`:
  FSL's `fslstats -C` prints `-0.000000` for x, `dc` then prints `dc: stack empty` twice; every
  file FSL's) wait for it.

## Triage notes (`validation/difftest/triage.toml`), by case (the 2026-10-30 trial's original)

- The five open entries of part 1 (messages 29, meshes 15, brains and masks 25, outlines 5, a
  header 1): resolved by part 2a except two cases:
  - **677** (`bet2 -f 0.3 -c 10 6 15 -s -w 5 -v`, NIfTI2_GZ): open; the first pass's
    self-intersection total 258194 (FSL) against 259595: `-c` converted on a neurologically stored
    image through FSL's 4 x 4 inverse (spec §8.1, open). Not an FSL defect.
  - **622** (`bet -R -f 0.2 -m -t`, NIFTI_GZ): open; files and stdout FSL's, stderr lacks FSL's two
    `dc: stack empty` lines: FSL's `fslstats -C` prints `-0.000000` for the centre's x (the same
    conversion), and the script's `dc` takes the leading `-` as its subtraction. Not an FSL defect
    of consequence here (the loop ends alike); rule C if the spec writer confirms.
  - Re-run with part 2a's build: `oracles/logs/retrial-cleanroom-bet-2a-2026-10-30-b.log` (237 of
    239 agree; the suite's skull and quaternion checks exact since 2a).

## `docs/validation/golden.md`

- BET's clean-room original (2026-10-30, part 2a): the checksums of `cli.fsl.bet`,
  `cli.fsl.bet.variations`, `cli.fsl.bet2` and `py.fsl.bet` re-recorded from 786e708 (the parity
  smoke tier passing): their outputs follow `specs/bet.md` §8.1's arithmetic (the deformation
  operation by operation, glibc's `tanh` as `larmorx_core::math::tanh_glibc`, which uses no fused
  multiply-add and gives the same bits on every platform); `cli.fsl.bet2.options` and
  `cli.fsl.bet2.reproduce` unchanged. No new case (no new public function: `larmorx_core::math::
  tanh` and `tanh_glibc` are Rust-only, with their own unit tests: recorded values, a sequence
  hash, the FMA copy against the portable one).

## Gates (for the merge)

- Record: `docs/validation/fsl-bet.md` made from 8abb5b4, the final code commit (all 142 cases
  agree; it was first made from 5bb2c5a with the same results; compare_records against main's:
  3 statuses fail → pass, `real/older-75y`, `real/7t`, `real/tumour`; 20 cases' details
  improved, e.g. `--binaryMask`'s files and every skull now bit-identical; nothing worse).
- Rust: `gates-rust.sh <worktree> all` OK at 8abb5b4 (the root, crates-gpl, crates-nc: fmt,
  clippy -D warnings, tests).
- `python -m larmorx_validation tables --check`: in sync.
- Python: ruff, ruff format, check_spdx OK; `pytest tests/python tests/golden validation/tests`
  643 passed, 13 skipped (serially, at 8abb5b4); under 16 xdist workers two oracle-cache tests
  each failed once (`test_verify_command`, `test_above_one_thread_a_run_is_served_after_three_
  identical_runs`) and passed alone and in the next run: timing flakes to note, like part 1's
  `test_an_unknown_thread_count…`. `record_golden.py --check` at 8abb5b4: 164 of 164 agree.
- Parity smoke tier: passed at 786e708 (2,607 tests; record_golden's run).

## The site's Roadmap row (`content/roadmap.md`)

- BET's clean-room original, part 2a: **completed 2026-10-30** (the reruns, the skull and the
  quaternion exact: every standard-tier case agrees with FSL 6.0.7.23). Next: part 2b (betsurf's
  original, the correctly rounded `tanh` as the default, the site page).
