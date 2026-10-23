# ANTS-116: antsAI compares blob patches possibly reflected (decided 2026-10-23: rule A)

**Finding** (register ANTS-116, `docs/findings/upstream-defects.md`; `docs/findings/ants-ai.md`):
`antsAI -b` compares a fixed and a moving blob by the correlation of their spherical patches
after turning the moving patch onto the fixed one with Wahba's `A = V·Uᵀ` of
`B = Σ outer(moving axis, fixed axis)`, the patches' principal axes (`antsAI.cxx:228-262`). The
axes come from `vnl_symmetric_eigensystem`, whose eigenvectors' signs are arbitrary, and `A` is
never checked for a determinant of −1 (antsAI's own principal-axes alignment checks it,
`:1001-1023`). With the two largest axes in 3D (the largest in 2D), `B` has rank 2 (rank 1) and
the singular vector of its zero singular value has an arbitrary sign too: identical patches can be
compared reflected, or turned by 180° about an axis.

**The correction** (larmorx's default, `larmorx_ants::ai_blobs`): every sign combination of the
moving axes (4 in 3D, 2 in 2D: flipping a fixed and a moving axis together changes nothing), each
with Kabsch's proper rotation (the right singular vector of the smallest singular value negated
where `det A < 0`), and the largest correlation kept. ANTs' own rotation is one of the candidates
when it is proper (its correlation then reused, bit for bit). A reflection is not among them: a
reflected match is no match for a rigid or affine transform. `reproduce` keeps ANTs' method.

**What `measure.py` measured** (2026-10-23, `results.json`; `lx.ants.blob_alignment` at 30 blobs,
both modes; "fix" is larmorx's default, which also corrects ANTS-110, -111, -112, -114 and ITK-60;
"reproduce" is ANTs' result bit for bit):

| Case | Mode | identical blob pairs at 1 | true / matched pairs | true landmarks / landmarks | distance from the true transform (mean, largest) |
|---|---|---|---|---|---|
| fMRIPrep's T1w of ds000005 vs its copy with the origin moved | ANTs | 4 of 13 | 10 / 11 | 10 / 10 | 0, 0 mm |
| | larmorx | **13 of 13** | 13 / 13 | 13 / 13 | 0, 0 mm |
| the T1w vs its copy rotated by 8° and shifted | ANTs | – | 5 / 7 | 0 / 0 | no landmark (NaN) |
| | larmorx | – | 7 / 8 | 7 / 8 | 1.08, 1.95 mm |
| 3D blob phantom, a translation | ANTs | 5 of 19 | 24 / 26 | 24 / 24 | 0, 0 mm |
| | larmorx | **19 of 19** | 19 / 19 | 19 / 19 | 0, 0 mm |
| 3D blob phantom, rotated by 7° and 4° and shifted | ANTs | – | 15 / 16 | 13 / 13 | 34.0, 66.8 mm |
| | larmorx | – | 9 / 11 | 9 / 9 | **0.50, 0.65 mm** |
| 2D blob phantom, a translation | ANTs | 14 of 28 | 19 / 20 | 19 / 19 | 0, 0 mm |
| | larmorx | **28 of 28** | 28 / 28 | 28 / 28 | 0, 0 mm |
| 2D blob phantom, rotated by 9° and shifted | ANTs | – | 9 / 17 | 6 / 7 | 6.2, 12.3 mm |
| | larmorx | – | 10 / 16 | 10 / 11 | **0.85, 1.24 mm** |

"Identical blob pairs" are the blobs found at the same voxel in both images (the moved copies keep
their voxels), counted where both patches fit in their images (the other pairs have no patch and
correlation 0 in both modes). ANTs correlates the others at 0.04 to 0.75, or negatively (0);
corrected, every one correlates at 1. A pair is true when the moving blob lies within two voxels of
where the known transform sends the fixed one.

Against larmorx's default before this correction (the `ants-ai` record of the antsAI merge,
`444420d`): the translations were found 0.33 to 1.19 mm off on average (a pair matched to a
neighbouring blob), now exactly; the 3D rotated phantom kept no landmark (an error), now 0.50 mm
off; the 2D rotated phantom 1.62 mm, now 0.85 mm; the rotated T1w 1.03 mm, now 1.08 mm.

**Decision: rule A, corrected by default.** The patch comparison exists to find the same anatomy in
both images; a rotation that reflects a patch, or one that depends on the arbitrary sign an
eigensolver gives an axis, compares a patch with something it can never be under the transform
being fitted. Trying the orientations that differ only by those signs, and only proper rotations,
compares the patches as the method means; identical patches then correlate at 1, and the blob
alignment recovers rotations ANTs misses. The cost: up to four patch comparisons per blob pair
instead of one (three more interpolations of a 33,401-point patch in 3D).

**How to run** (from the larmorx checkout, larmorx built into `python/`; the oracle run
`ds000005-sub-01-noreconall` in `<workspace>/oracles/fmriprep-25.2.6` for the T1w):
`PYTHONPATH=python:validation/src python -P validation/investigations/ants-116-blob-orientation/measure.py --out <dir> [--threads 6]`
(about 10 minutes).
