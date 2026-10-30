## Report: BET's spec writer, the six questions (2026-10-30)

All six questions of the clean-room original's part 1 are answered in `specs/bet.md` (every new
line marked "(spec writer, 2026-10-30)", behaviour and mathematics only): BET's clean-room part 2
can start. Branch `worktree-agent-ac0ade9e107f3297f` (fast-forwarded to 45646bc first), 6 commits
(eb43cdc, 1f60cd7, 796482b, 34f1d9d, d14a10e, bcb1e59). No FSL defect found (FSL-67, FSL-68
unused); no site or record work. The handoff is `handoff.md` here.

1. **Reruns (§4.5, §8.1), high confidence:** the rule as written was FSL's (from the initial
   surface bit for bit, `l` reset, only `f2·F_P` capped at 1 where `s·n > 0`; a copy holds the same
   neighbour and triangle lists). The misses come from the reruns amplifying last-bit differences:
   on older-75y any one of the original's arithmetic choices moves the first rerun from FSL's
   19809.7 to 16005-21766 (Dice 0.993-0.996); the replica carrying all ten gives the original's
   results exactly; on the tumour `tanh` matters too (musl's 613.5 against glibc/fdlibm's 737.8).
   §8.1 now lists every operation of the deformation step.
2. **Skull search (§4.7), high:** outside voxels count as surface voxels, candidate ranges not
   clipped, three separately accumulated grids, darkness and rise searches on in-image samples
   only, the rise scan stops at the first rise smaller than the largest so far.
3. **Quaternion precision path (§8.2), high:** float matrices, a float polar decomposition, x87
   extended steps, NIfTI-2 writing its doubles unrounded; a numpy emulation matched FSL on 18 of 18
   headers.
4. **`-e` test (§4.5), high:** a sweep plus a strict sign-based segment-triangle predicate; on case
   27 FSL's pair really intersects on FSL's own surface, the original's surface differs: the test
   is not the cause.
5. **Neighbour order (§4.4), high:** triangles in order, each vertex's other two moved to the end of
   its list.
6. **Observations moved in (observed), medium-high:** mesh numbering, rasterisation and the fill's
   start, the robust range's exact arithmetic (corrected), small images, the option parser, ANALYZE
   inputs, single slices written 2D, `--ct`'s outline, FSL-56's layout, FSL-62's brain, the `bet`
   script's messages, FSL-44's splitting, `-F`'s and `-Z`'s files. Two new differences for part 2:
   FSL reads `in.nii` given `in.nii.gz` when only `in.nii` exists (§3.1); `bet -F` on a float64
   series writes a float64 brain (§5.2).

Random testing: of the trial's 75 remaining differences, the replica carrying thirteen of the
original's arithmetic choices reproduces 59 exactly; 6 differ only in the skull image (answer 2), 2
in zstd files (not compared by value), 1 is the `-F` float64 brain, 7 surface cases stay open (45,
121, 154, 213, 262, 541, 677; 154 and 677 use `-c`, a matrix solve on neurologically stored images).

### At the merge (orchestrator)
next-steps item 7 updated; the five open triage entries of part 1 cite the explanation (59 of 75)
and the cases still open. The agent's scratch tools are archived outside the repository
(`<workspace>/archive/2026-10-30-bet-spec-answers/`: one carries replica code under the FSL
Licence).
