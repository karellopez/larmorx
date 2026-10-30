# HANDOFF: BET's spec writer, the six questions (2026-10-30)

Branch: `worktree-agent-ac0ade9e107f3297f` (fast-forwarded from bed7e04 to 45646bc first, then
6 commits: eb43cdc, 1f60cd7, 796482b, 34f1d9d, d14a10e, bcb1e59). Files: `specs/bet.md` (every
addition marked "(spec writer, 2026-10-30)"), `docs/findings/fsl-bet.md` (restricted: the source
references), `validation/investigations/bet-spec-answers/` (README + Apache probes). No defect of
FSL found (FSL-67, FSL-68 unused). No site work, no records.

## docs/next-steps.md (item 7, BET), after the "Spec questions" sentence of part 1

> **The six questions answered (spec writer, 2026-10-30; `specs/bet.md`, lines marked
> "(spec writer, 2026-10-30)"): BET's clean-room part 2 can start.** (1) The rerun rule was FSL's
> all along (from the initial surface bit for bit, the same lists; only `f2·F_P` capped at 1
> where `s·n > 0`); the reruns amplify last-bit differences: one of the original's ten
> arithmetic choices alone moves `older-75y`'s first rerun from FSL's 19809.7 to 16005-21766, and
> the replica carrying all ten gives the original's results exactly (16099; the same masks on
> `older-75y`, `7t`, `tumour`), so §8.1 now gives the deformation's arithmetic operation by
> operation (the initial placement, the normal and mean by division, `1/R` from `s_n·n`,
> `p + ((u1 + u2) + u3)`, the centre's single running sum, glibc's `tanh`: fdlibm's, not musl's).
> (2) The skull search step by step (§4.7: outside voxels count as surface voxels, three grids,
> the scan's stop rule). (3) The quaternion's precision path (§8.2: float matrices, x87 extended
> steps, NIfTI-2 written unrounded; checked 18 of 18 against FSL). (4) The `-e` test as a
> predicate (§4.5); on the small head of case 27 FSL's pair truly intersects on FSL's surface and
> the original's surface differs: the test is not the cause. (5) The neighbour order (§4.4: the
> move-to-end rule; a copy holds the same lists). (6) The black-box observations are in the spec,
> marked (observed); two new: a name with an extension falls back to the search (§3.1: FSL reads
> `in.nii` given `in.nii.gz`; the original refused) and `bet -F` on a float64 series writes a
> float64 brain (§5.2). Of the 2026-10-30 trial's 75 remaining differences, the replica with
> thirteen of the original's arithmetic choices reproduces 59 exactly; seven surface cases remain
> open for part 2 (45, 121, 154, 213, 262, 541, 677; 154 and 677 may be `-c`'s solve, §8.1).

## validation/difftest/triage.toml (the five open entries of part 1, for the orchestrator)

The trial's differences of the original are explained (59 of 75) by its arithmetic choices
(specs/bet.md §8.1) and the skull search (§4.7); the entries can cite
`validation/investigations/bet-spec-answers/README.md`. Still open for part 2: cases 45, 121,
154, 213, 262, 541, 677 (surfaces), 106 and 482 (zstd files, not compared by value).

## For the orchestrator

- Archived (never deleted): `<workspace>/archive/2026-10-30-bet-spec-answers/` holds the scratch
  tools, among them `patch_all.py`, which carries snippets of the replica's FSL-licensed code
  (not for the Apache repository).
- No register rows (no FSL defect found).
