You are the **spec writer for BET** (github.com/karellopez/larmorx; your worktree is a checkout of main at the local merge commit 45646bc, which the orchestrator is verifying beside you). BET's clean-room original, part 1, was merged today; its implementer (who never read FSL's source) left **six questions** for the spec writer. Answer them in `specs/bet.md`, so BET's clean-room part 2 can start next week. Aim for 2 to 3 hours; stop at the end.

Read first: the repo's `CLAUDE.md` (rule 1, "Agent working rules"); `docs/porting-guide.md` §1 (what a spec may and may not contain: behaviour, maths, conventions, command line and validation plan, **never code, routine names or line references**); `specs/bet.md`; `docs/findings/fsl-bet-original.md` (the clean-room implementer's black-box observations); `docs/parts/2026-10-30-cleanroom-bet-part1/report.md` and `handoff.md` ("Spec questions"); `docs/validation/fsl-bet.md` (the original's record: its 3 failing cases).

## Your role: spec writer (you may read FSL's source)
You may read FSL 6.0.7.23's source (`~/fsl-6.0.7.23/src`, `crates-nc/fsl-source/`), the BET replica (`crates-nc/larmorx-nc-bet`, `larmorx-nc-core`) and the restricted `docs/findings/fsl-bet.md`. Because you read them, you **never edit the clean-room crate** (`crates/larmorx-fsl`) nor its findings file; you edit `specs/bet.md` only as spec writer, every new or changed line marked "(spec writer, 2026-10-30)", in behaviour and mathematics only.

## The six questions (`docs/findings/fsl-bet-original.md` has the evidence and the probes in `validation/investigations/cleanroom-bet/`)
1. **The rerun after a self-intersecting first pass (§4.5):** FSL's rerun differs from the rule as written, though its factor 100 and its 1000 iterations are right; the three standard-tier cases that rerun (older-75y, 7t, tumour) miss the thresholds (Dice 0.980 to 0.995). State exactly what the rerun starts from and changes, and in what order, so an implementer can reproduce FSL on those cases.
2. **The skull search (§4.7):** the details the original lacks (Dice about 0.9 against FSL).
3. **The quaternion's precision path (§8):** the last float bit of one component in some real headers (about 5e-9 in NIfTI-2's doubles): the order and precision of each operation that writes it.
4. **`-e`'s exact test (§4.5):** FSL finds intersections where a segment-triangle test (with folds and coplanar overlap) does not, on small random heads (difftest 2026-10-30 #27): the exact test.
5. **The order of the sums over a vertex's neighbours (§8):** the spec gives the triangles' order only; the neighbours' order decides the last digits on degenerate surfaces.
6. **Move the black-box observations into the spec**, marked "(observed)": the mesh numbering, the rasterisation, the robust range's rounding (the width in float), small images (`n/50 = 0`), the fill from any start, the option parser's values, ANALYZE inputs, single slices written 2D, `--ct`'s outline in Cormack units, FSL-56's layout, FSL-62's brain, `bet`'s variation numbers, its failure line, FSL-44's splitting, `-F`'s and `-Z`'s files.

For each answer, check it against FSL 6.0.7.23 as a black box where a check is cheap (the replica, already bit-identical, may serve as a reference for what FSL computes); say in your report how confident each answer is.

## Documents
- **Your own files:** `specs/bet.md` (as spec writer), the restricted `docs/findings/fsl-bet.md` if you add source-level notes, `validation/investigations/bet-spec-answers/` (README, probes).
- **Shared, through `<scratch>/HANDOFF.md`:** next-steps (item 7: the six questions answered; BET part 2 ready), the register's rows only if a defect is found (FSL-67, FSL-68 reserved).
- No site work, no records.

## Machine (shared)
- **Up to 16 threads** while the orchestrator's verification runs beside you; memory boxes together at most **30 GiB**.
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`. FSL stays off PATH.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge.
- Scratch: `<scratch>/bet-spec-answers/` (this folder): NOTES.md, HANDOFF.md; never write under a `.claude/` folder.
- `source scripts/orchestration/env.sh`. Gates for a spec-only part: `python scripts/check_spdx.py` and `ruff` on any probe you commit.
- Never: delete work products; modify the FSL installations or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>`, `<scratch>`, `~/fsl-<version>`.
- If the orchestrator asks for a checkpoint (the weekly usage limit), commit a WIP, update NOTES.md and stop.

## Stop after this part; report (concise)
- Each question: the answer in one or two lines, where it is in the spec, how it was checked, confidence.
- Commits, anything for the user.
