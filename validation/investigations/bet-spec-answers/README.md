# BET's spec writer: the six questions of the clean-room original (2026-10-30)

The probes behind the answers written into [`specs/bet.md`](../../../specs/bet.md) (lines marked
"(spec writer, 2026-10-30)") for BET's clean-room original, part 2. They run FSL 6.0.7.23's
`bet2` and `bet` (`~/fsl-6.0.7.23/bin`, by full path, FSL off the `PATH`), the BET replica
(`larmorx-nc`, bit-identical to FSL) and the clean-room original (`larmorx`) as black boxes.
Run each with `python -P <script> ...` from the workspace's `.venv`.

| Script | What it shows |
|---|---|
| `quat_emul.py <input> <FSL outputs...>` | the qform quaternion by the precision path of `specs/bet.md` §8.2 (float matrices, double, x87 extended precision through numpy's `longdouble`), against what FSL wrote, NIfTI-1 floats and NIfTI-2 doubles: **18 of 18 bit-identical** (nine real T1w images of the data panel and the oracle runs, both formats) |
| `pair.py <full-precision mesh.vtk> a,b,c d,e,f` | two triangles' geometry in exact rational arithmetic (signed volumes, strict and closed edge crossings): the pair FSL's `-e` test finds on case 27 truly intersects on FSL's surface and not on the original's |
| `finding_case.py <finding.json> <dir> [--shrunk]` | a random-testing finding's recipe (or its shrunk one) as a call to run by hand, its inputs built |
| `sweep.py <out dir> <n>...` | the 2026-10-30 trial's cases run by the replica, the original and the replica with the original's arithmetic choices (`BET_PROBES`: the directory of the probe builds; `ALL`: which build), and which of their streams, images and meshes differ |
| `cmpmask.py`, `cmpmesh.py` | voxels differing and Dice of masks; per-vertex distances of meshes |

## The method (questions 1, 4 and 5)

Copies of the replica's source, each changed in one place to an arithmetic choice of the
clean-room original (read from its code; the copies are FSL-licensed and were built and run in a
scratch directory only: they are not in this repository), were run on the standard tier's three
failing cases and on the 75 random-testing cases the original still differed in:

- **One choice at a time** (`older-75y`, ds002422): the first rerun's total moves from FSL's
  19809.7 to 17522.1 (the initial placement as `c + (r/2)p`), 16004.9 (the update as
  `((p + u1) + u2) + u3`), 20357.8 (`1/R` from `s·n`), 18044.3 (the normal by products with
  `1/|v|`), 21766 (the neighbour mean likewise), 19570.9 (the triangle normal from `v1`), 20009.3
  (the centre of gravity summed per slice), 16583.2 (first-come neighbour lists); the masks to
  Dice 0.993 to 0.996. musl's `tanh` and a correctly rounded `pow` change nothing there; on the
  tumour (ds005003) musl's `tanh` alone moves the rerun's total from 737.8 to 613.5.
- **All ten together**: the original's results exactly (16099, 3203.54; the same masks on
  `older-75y`, `7t` and `tumour`).
- **Thirteen together** (also the inward depth as `2 + k·ds`, the field of view in double with
  `2r` only below `r`, the slab radius as a square root in double): 59 of the 75 random-testing
  differences reproduced exactly; of the other 16, six differ only in the skull image, two in
  zstd files (not compared by value), one is `bet -F`'s float64 brain, seven (cases 45, 121, 154,
  213, 262, 541, 677) still differ in their surface.
- **The neighbour order**: a copy of the mesh (as every rerun makes) holds the same neighbour and
  triangle lists as the first pass's mesh, for all 2562 vertices; the rule of `specs/bet.md` §4.4
  gives them.
- **The `-e` test** (case 27, shrunk): FSL's first pass finds the pair `(834, 195, 835)`,
  `(829, 53, 763)`; `pair.py` on the full-precision surfaces shows it intersecting on FSL's and not
  on the original's (which lies up to 0.14 mm away), where FSL's own test finds no pair either.

The scratch tools (the patch script that makes the copies, the build and run scripts) are kept in
`<workspace>/archive/2026-10-30-bet-spec-answers/` (FSL-licensed snippets: not for this
repository).

## Also found

- FSL reads `in.nii` when given `in.nii.gz` and only `in.nii` exists (a name with an extension
  falls back to the extension search); the original refused (`specs/bet.md` §3.1).
- `bet -F` on a float64 series writes a float64 brain with the input's double values
  (`specs/bet.md` §5.2).
- The robust range's bin width is `(hi − lo)/1000` computed first, then multiplied by the bin
  (`specs/bet.md` §4.2); the narrowed range has a formula of its own.
