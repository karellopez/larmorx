# HANDOFF: FSL FLIRT replica, part 2 (2026-11-01)

Branch: the worktree `agent-a947689786d5ba4b0` (from main 301fab7). Site clone: `<scratch>/site`,
branch `fsl-flirt-page`. Defect IDs used: FSL-75, FSL-76, FSL-77 (FSL-78 to FSL-89 and PIPE-18
to PIPE-22 unused). The text below is the exact text for each shared document; numbers marked
The record's numbers come from `docs/validation/fsl-flirt-replica.md` (re-made in this part from b92c7ed).

## 1. docs/findings/upstream-defects.md (the register)

Replace the FSL-67 row with:

| FSL-67 † | flirt (BBR, `-wmseg`) | the boundary points' normals: the gradient of the smoothed segmentation (in the segmentation's voxel units) is divided by the voxel sizes of the current reference level (FLIRT's isotropic resampling of the reference), not the segmentation's, before normalising: on an anisotropic segmentation the normals are the voxel-gradient's directions, not the mm-gradient's | c? | small: fMRIPrep's T1w masks are 0.85 × 0.83 × 0.83 mm or similar | kept in both modes (rule D then C, measured 2026-11-01: computing the normals in millimetres moves fMRIPrep's three BBR results by 0.016, 0 and 0.025 mm on average over the brain, BBR's own sensitivity; on an anisotropic 0.85 × 0.83 × 2.5 mm segmentation with a known motion the corrected and FLIRT's normals end 0.060 and 0.063 mm from the truth on average, 0.092 and 0.092 mm at the 95th percentile: no gain; `validation/investigations/fsl-flirt/fsl67_measure.py`) | – |

Add after FSL-74:

| FSL-75 † | newimage's background value (`calc_bval`), reached by FLIRT's pyramid (`subsample_by_2`), its padding (`configure_interpolation` without `-setbackground`) and the sinc fallback; mcflirt through `affine_transform` and `subsample_by_2` | the 10th percentile of the 2-voxel border is taken from a buffer sized `2(x−2ex)(y−2ey)ez + 2(x−2ex)z·ey + 2yz·ex` (`e` = min(2, n−1)), too small when the volume is 3 voxels wide along x (and 2 or more along y or z) or along y (2 or more along z, x not 2 or 4): the rest is written past the buffer (heap corruption: FSL aborts at a later `free`, segfaults, or finishes, depending on the heap's layout), and the percentile is of the values that fit; at 1 × 1 × 1 the buffer is empty and its first value is read | a | none (fMRIPrep's levels are far larger); FLIRT's pyramid meets it on images of 5 or 6 voxels along x or y, or a 3-voxel level of a small field of view (a 25 × 30 × 27 image at 0.5 × 1.2 × 1 mm reaches 3 × 3 × 3) | rule A: the percentile of the whole border by default, the values that fit FSL's buffer with `reproduce` (FSL's result where it survives), named in both modes; a single voxel refused in both modes (rule B); traced with a hardware watchpoint (2026-11-01). mcflirt's replica computes FSL's values that fit, without a notice yet (follow-up) | – |
| FSL-76 † | makerot | the axis is divided by the square root of its length (`ax / sqrt(norm2(ax))`, `norm2` being the length): for an axis that is not of unit length the rotation is by the angle times the square root of the length (30 degrees about 0,0,4 give 60; the help calls the axis "unnormalised") | a | none (fMRIPrep does not run makerot) | rule A: divided by its length by default, FSL's with `reproduce`, named where the matrix changes (2026-11-01) | – |
| FSL-77 † | makerot | `axis.value()[0..2]` and `centre.value()[0..2]` are read whatever the vectors' sizes: fewer than three values read past the vector (its buffer: the default's zeros, or the values an earlier occurrence of the option left: `-a 1,2,3 -a 7` is 7,2,3), more are ignored | a | none | rule B: refused by default (a message naming FSL-77); FSL's reading with `reproduce`, named (2026-11-01) | – |

In the FSL-09 row, after "`reproduce` truncates and wraps as x86-64 does": "; FLIRT's replica names it also for values out of the output type's range (a negative `-setbackground` into uint8), 2026-11-01".
In the FSL-14 row, the "what larmorx does" cell: "corrected by default (`upstream_defects`, 2026-10-11); FLIRT's replica names it whenever the pair's volumes hold NaN or infinite values (silent where they lie outside the cost's points), 2026-11-01".
In the FSL-22 row, the where cell: "`calc_bval` (newimage): mcflirt, flirt"; append to the last cell: "FLIRT's replica uses the same switch since 2026-11-01 (its padding and pyramid levels: NaN last by default, libstdc++'s order with `reproduce`, named)".

The history paragraph: "FLIRT part 2 (2026-11-01): FSL-67 decided (rule D measured, then C), FSL-75 to FSL-77 added (FSL-75 by rule A after the trial found FLIRT surviving the overflow on an ordinary image, its single-voxel case by rule B; FSL-76 rule A; FSL-77 rule B); FSL-09, FSL-14 and FSL-22 named by FLIRT's replica too." Counts: +3 entries (FSL-75, FSL-76, FSL-77), all with switches, none "decide"; the decide count goes down by one if FSL-67's "yes (measure)" was counted among the open measurements (it is now "–").

## 2. docs/api/upstream-defects.md

Add rows (after FSL-68):

| FSL-75 | flirt | the replica `larmorx-nc fsl flirt` (`larmorx fsl flirt`, `lx.fsl.flirt`) | the background value of a volume 3 voxels wide (a pyramid level, the input's padding) comes from a buffer too small for its border: FLIRT corrupts its heap and takes the percentile of the values that fit; a single voxel reads an empty buffer | the percentile of the whole border, named; `reproduce` FLIRT's values (FLIRT itself may abort there); a single voxel refused in both modes |
| FSL-76 | makerot | the replica `larmorx-nc fsl makerot` (`larmorx fsl makerot`, `lx.fsl.makerot`) | the axis is divided by the square root of its length: a rotation by the angle times √length | the axis divided by its length, named; `reproduce` FSL's matrix |
| FSL-77 | makerot | the same | an axis or centre of other than three values: past the values, what the vector's buffer holds | an error naming FSL-77; `reproduce` FSL's reading, named |

FSL-22's row (if listed): add "and the FLIRT replica (its padding and pyramid levels)".

## 3. validation/tools.toml

The `fsl-flirt` tool:

```toml
[[tool]]
id = "fsl-flirt"
title = "FLIRT (affine registration, boundary-based registration; convert_xfm and FLIRT's other programs)"
eco = "fsl"
page = "/tools/fsl-flirt/"
use = ["larmorx fsl flirt", "larmorx fsl convert_xfm", "larmorx fsl avscale", "larmorx fsl img2imgcoord", "larmorx fsl std2imgcoord", "larmorx fsl img2stdcoord", "larmorx fsl rmsdiff", "larmorx fsl makerot", "lx.fsl.flirt", "lx.fsl.convert_xfm", "lx.fsl.avscale", "lx.fsl.img2imgcoord", "lx.fsl.std2imgcoord", "lx.fsl.img2stdcoord", "lx.fsl.rmsdiff", "lx.fsl.makerot"]
how = "exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (parts 1 and 2); the clean-room original to be written from specs/flirt.md"
validation = "replica: [{fsl-flirt-replica.pass} of {fsl-flirt-replica.compared}](record:fsl-flirt-replica) compared cases bit-identical to FSL 6.0.7.23 (matrices, images, printed results, exit status and messages): fMRIPrep's boundary-based calls on three oracle runs (their matrices also identical as text to fMRIPrep's own), every cost function, the search options, every shipped schedule, `-anglerep quaternion`, `-applyxfm` 3D and 4D with every interpolation and data type, BBR with its types, coordinates and a field map, avscale, the coordinate programs, rmsdiff and makerot; {fsl-flirt-replica.both} more rejected by both, alike; {fsl-flirt-replica.expected} where FLIRT crashes or never finishes (FSL-71, FSL-72, FSL-75)"
```

The `fsl-flirt-replica` record:

```toml
[[record]]
id = "fsl-flirt-replica"
larmorx = "`larmorx-nc fsl flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`; `lx.fsl.flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot` (the replica, FSL Licence)"
reference = "flirt, convert_xfm, avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff and makerot (FSL 6.0.7.23); fMRIPrep's BBR calls also against fMRIPrep's own matrices"
status = "`experimental` (part 2: {fsl-flirt-replica.pass} of {fsl-flirt-replica.compared} compared cases bit-identical, fMRIPrep's calls included; {fsl-flirt-replica.both} rejected by both, with the same messages and status; {fsl-flirt-replica.expected} expected divergences where FLIRT crashes or never finishes; `alignpaxes`, general parameter subsets, `-debug`'s BBR volume, applyxfm4D, midtrans and pointflirt wait for part 3)"
```

(`{fsl-flirt-replica.expected}` is `tables.py`'s count of expected divergences; checked: both blocks render with the record of 4436ee9.)

## 4. docs/validation/golden.md

In the table, `larmorx-nc`'s cell: "52 cases, 298 outputs" becomes "59 cases, 305 outputs"; the Python registry gains six replica-only functions (`lx.fsl.avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`, mapped to their `larmorx-nc` cases). Paragraph:

"`larmorx-nc`'s checksums recorded on 2026-11-01 from 2979df4 (FLIRT part 2): 7 cases added, `flirt.quaternion-dof6` (`-anglerep quaternion`), `avscale.allparams`, `img2imgcoord.vox`, `std2imgcoord.vox`, `img2stdcoord.mm`, `rmsdiff.mask` and `makerot.axis-centre` (FSL-76 corrected), the printing programs' standard output kept as `stdout.txt`; every other checksum unchanged (the core's memory and background-value changes changed no golden output)."

## 5. docs/next-steps.md, item 7 (after "Part 2:" of the FLIRT paragraph)

Replace the sentence that begins "**Part 2:** those, `-anglerep quaternion`, ..." up to "...the heavy tests stay at the end)." with:

"**Part 2 done (2026-11-01).** The site's FLIRT page (fMRIPrep's BBR on three oracle runs with the white-matter boundary, a default 12-dof registration, `-applyxfm` on a raw BOLD series; the matrices as text with fMRIPrep's own, the displacement over the brain, the record's cases). The trial's findings settled (`recheck_findings.py`, every finding that fits the trial's memory agrees in its shrunk and drawn forms): FSL's heap corruption traced to newimage's background value (FSL-75, rule A: the whole border by default, FSL's values with `reproduce`; a single voxel refused); the 66 matrices were `scalemat`'s single-precision quotient `1/basescale` (fixed); the fix pass's unnamed changes were FSL-14 and FSL-09 (named now). FSL-67 measured (rule C, kept). `-anglerep quaternion`; the hangs FSL-71/72 as suite cases (FSL under a timeout and an address-space limit). FLIRT's other programs `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot` (FSL-76, FSL-77), each with its command line, `lx.fsl` wrapper, golden case and spec section (`specs/flirt.md` §15). The replica's peak memory on large references brought to FSL's (the blur's passes, two input copies, the weighted filter's temporaries: 262 → 187 MiB against FSL's 192). FSL-22 in FLIRT (the padding's NaN order). Record [fsl-flirt-replica](validation/fsl-flirt-replica.md): 217 cases, 152 of 152 compared cases bit-identical (61 rejected by both, alike; 4 expected divergences: FSL-71, FSL-72 and FSL-75's two, where FLIRT never finishes or crashes); part 1's 141 cases unchanged in status and details (`compare_records.py`); mcflirt's and BET's standard tiers re-run (the core changed): 236 of 236 and 178 of 178 cases identical to their records. Two short trials of the new paths (half an hour and six minutes, 12376 and 3274 cases). **Part 3:** the schedule command `alignpaxes` (LAPACK `dsyevd`), general parameter subsets' pseudo-inverse, `midtrans` and `pointflirt` (LAPACK `dgesdd`), `applyxfm4D` (templated over the data type), `-debug`'s BBR volume, the coordinate programs' `-warp` (after FNIRT), makerot's NaN signs (three calls with a NaN or infinite angle or axis), mcflirt's FSL-75 notice and correction; then the clean-room original from `specs/flirt.md`. The full tier (fMRIPrep's 22 BBR calls on every oracle run), the data panel and long random-testing runs wait for the final validation phase."

In the order list (line 113): "FLIRT with BBR (parts 1 and 2 done 2026-11-01; part 3 next: `alignpaxes`, the SVD programs, applyxfm4D)".

## 6. CLAUDE.md, "Where things are", the FSL row

Replace "the **FLIRT** replica (part 1, 2026-11-01: `larmorx-nc fsl flirt`, `convert_xfm`; `lx.fsl.flirt`, `lx.fsl.convert_xfm`; `experimental`): 107 of 107 compared cases bit-identical to FSL 6.0.7.23 (34 more rejected by both), fMRIPrep's BBR calls on three oracle runs included (their matrices also fMRIPrep's own); `specs/flirt.md` for the clean-room original" with:

"the **FLIRT** replica (parts 1 and 2, 2026-11-01: `larmorx-nc fsl flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`; `lx.fsl.*` the same; `experimental`): 152 of 152 compared cases bit-identical to FSL 6.0.7.23 (61 more rejected by both, 4 where FLIRT crashes or never finishes), fMRIPrep's BBR calls on three oracle runs included (their matrices also fMRIPrep's own), `-anglerep quaternion`; FSL-75 (the background value's heap overflow) corrected by default; its site page; `specs/flirt.md` for the clean-room original; part 3: `alignpaxes`, the SVD programs, applyxfm4D"

and in "Next:": "FLIRT's replica, parts 1 and 2, done (2026-11-01). Next: FLIRT's part 3, its clean-room original, then FAST, ...".

## 7. validation/difftest/triage.toml

Replace the four `status = "unclear"` FLIRT entries of the 2026-11-01 trial with:

```toml
[[difference]]
tool = "fsl-flirt"
implementation = "replica"
signature = ["every file bit-identical"]
status = "fixed"
explanation = "FLIRT's part 2: the 66 registrations whose matrix differed: scalemat's 1/basescale is a single-precision quotient in FLIRT (an int divided by a float: exactly 100 for 0.01 mm voxels), the replica divided in double; visible with -init or -usesqform and a base scale other than 1 (voxels below 0.75 mm or above 12 mm). Fixed (larmorx-nc-flirt run.rs scalemat); every one agrees since (recheck_findings.py, shrunk and drawn); suite case real/basescale-small-init"
link = "validation/difftest/cases/fsl-flirt/basescale-init-float.json"

[[difference]]
tool = "fsl-flirt"
implementation = "replica"
signature = ["exit status (flirt is killed; the replica exits N)", "files written", "stderr identical"]
status = "register"
defect = "FSL-75"
explanation = "FLIRT's part 2: the 281 cases where FSL aborts or segfaults: newimage's background value (calc_bval) writes past a buffer too small for the border of a volume 3 voxels wide (FLIRT's pyramid levels, its padding), or reads an empty one at 1 x 1 x 1; traced with a hardware watchpoint (validation/probes/fsl-flirt-bval). FSL-75, rule A: the whole border by default, FSL's values that fit with reproduce, named; a single voxel refused in both modes (rule B). The suite takes FSL killed with the replica naming FSL-75 as agreement"
link = "validation/difftest/cases/fsl-flirt/background-level-overflow-survives.json"

[[difference]]
tool = "fsl-flirt"
implementation = "replica"
signature = ["DN fix pass"]
status = "fixed"
explanation = "FLIRT's part 2: the 8 fix-pass changes without a notice: FSL-14 named now whenever the pair's volumes hold NaN or infinite values (silent where they lie outside the cost's points), FSL-09 also for values out of the output type's range (a negative -setbackground into uint8); every one agrees since"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt"
implementation = "replica"
signature = ["larmorx exceeds the memory limit"]
status = "explained"
explanation = "FLIRT's part 2: the trial's runs over 3 GiB (trial-flirt-01374, -01768, -02317, -03183, -03192, -03718: voxels of 0.01 mm or 25-50 mm make FLIRT's isotropic levels huge): FSL's own peak is 8.0 GiB to above 39 GiB on them, or it runs over 30 minutes (validation/investigations/fsl-flirt/memory_findings.py); the replica's peak was brought to FSL's (12.1 GiB against 8.0 on -01374 before the fix). Heavy cases of the final validation phase"
link = "docs/findings/fsl-flirt.md"
```

(The single "one message count with -dof 3" finding was the same scalemat cause: covered by the first entry.)

Add, for part 2's trials:

```toml
# ---------------------------------------------------------------- FLIRT's replica, part 2: the trials of the new paths (2026-11-01)
# `difftest --tools fsl-flirt-programs --budget 0.5 --jobs 8 --seed trial-flirt-programs` (<workspace>/oracles/difftest/
# 2026-11-01-fsl-flirt-programs): 12376 cases, 12309 agree, 58 differ, 5 over the memory limit, 4 timeouts; then
# `--budget 0.1 --seed trial-flirt-programs-2` (2026-11-01-fsl-flirt-programs-2, makerot added): 3274 cases, 3222 agree,
# 47 notices, 2 differ, 3 over the memory limit. Every finding re-checked with the final build (recheck_findings.py).

[[difference]]
tool = "fsl-flirt-programs"
implementation = "replica"
signature = ["stderr identical"]
status = "fixed"
explanation = "-anglerep quaternion: FLIRT's optimise builds the starting matrix (vector2affine) at every verbosity and prints it only from -verbose 5; the replica built it only then, so an invalid starting axis printed one 'Parameters do not form a valid axis' message less (50 findings). Fixed (larmorx-nc-flirt run.rs optimise)"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-programs"
implementation = "replica"
signature = ["every file bit-identical"]
status = "fixed"
explanation = "FSL-22 in FLIRT: a NaN voxel in the border of the input whose background value pads the output (a NaN matrix makes every voxel the padding): libstdc++'s order put the NaN first in FLIRT, the replica's volumes sorted it last in both modes; FLIRT's background values now take FSL-22's switch (trial-flirt-programs-00931, -00902)"
link = "validation/difftest/cases/fsl-flirt/background-nan-order.json"

[[difference]]
tool = "fsl-flirt-programs"
implementation = "replica"
signature = ["exit status", "files written"]
status = "register"
defect = "FSL-75"
explanation = "trial-flirt-programs-11900 (and -03179, -03583, -05316, -05574, -05793): FLIRT survives FSL-75's overflow (a 3 x 3 x 3 or 3 x 2 x 2 level) and finishes, where the replica refused: FSL-75 decided by rule A instead (the values that fit with reproduce: FSL's matrix)"
link = "validation/difftest/cases/fsl-flirt/background-level-overflow-survives.json"

[[difference]]
tool = "fsl-flirt-programs"
implementation = "replica"
signature = ["notice: FSL-76"]
status = "fixed"
explanation = "makerot named FSL-76 where the angle gives the identity either way (0 degrees, or below FLIRT's 1e-8 radians): named now only where the matrix changes (47 notices of the second trial)"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-programs"
implementation = "replica"
signature = ["flirt is killed after FSL-N's heap corruption; the replica finishes and names FSL-N (exit N)"]
status = "fixed"
explanation = "the suite required exit 0 of the replica where FLIRT is killed after FSL-75; going on, the replica ends as FLIRT would without the corruption (here exit 1: FSLOUTPUTTYPE=ANALYZE cannot be written): the check now requires FSL-75 named and no signal (trial-flirt-programs-2-02673, -02874)"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-programs"
implementation = "replica"
signature = ["larmorx exceeds the memory limit"]
status = "explained"
explanation = "quaternion registrations over 3 GiB (trial-flirt-programs-00026, -00667, -02324, -06285, -09485; -2-01831, -01944, -02240): extreme voxel sizes. Re-checked after the memory fixes in a 16 GiB box: -00667 and -09485 agree now (the replica fits), on -06285 FSL is killed and the replica finishes, on -00026 and -02324 both are killed; the second trial's three measured alone in a 20 GiB box (memory_findings.py): FSL is killed at 20 GiB on all three, the replica on two and finishes the third at 19.0 GiB (-2-01944). Heavy cases of the final validation phase (heavy.jsonl)"
link = "docs/findings/fsl-flirt.md"
```

## 8. The site (larmorx_documentation)

- `content/tools/fsl-flirt.md` and `static/evidence/fsl-flirt.json` (with its blobs): in the branch `fsl-flirt-page` (my own files).
- `site.yml`, the tools navigation, after BET:
  `  - { title: FLIRT, url: "/tools/fsl-flirt/", eco: fsl,
      summary: "Affine and boundary-based registration (flirt) with convert_xfm, avscale, the coordinate programs, rmsdiff and makerot: an exact replica in larmorx-nc; the clean-room original planned." }`
- `data/status.yml`: regenerate with `python -m larmorx_validation tables` after tools.toml (the page link appears).
- `content/validation.md`: in the "Parity, full" row nothing changes; if the page lists the tools with a random-testing descriptor, add FLIRT (`fsl-flirt`, `fsl-flirt-programs`).
- `static/evidence/summary.json`: re-export (`python -m larmorx_validation evidence summary --site <site>`); the summary's PAGES gained `/tools/fsl-flirt/` in this branch (`evidence/summary.py`).
- `content/roadmap.md`, row 1 (FLIRT with BBR), its status cell: "The replica's parts 1 and 2 done (2026-11-01): `flirt`, `convert_xfm`, `avscale`, the coordinate programs, `rmsdiff` and `makerot`, every cost function, BBR and the schedules, `-anglerep quaternion`, 152 of 152 compared cases bit-identical to FSL 6.0.7.23, fMRIPrep's BBR calls included; [its page](/tools/fsl-flirt/); part 3 next: `alignpaxes`, the SVD programs (midtrans, pointflirt), applyxfm4D; then the clean-room original". Under "Completed", a row: "| 2026-11-01 | **FLIRT's replica, part 2.** FLIRT's other programs (avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, makerot), quaternion parameters, FSL's heap overflow in the background value traced and corrected (FSL-75), the page with fMRIPrep's boundary-based registrations |". "Last updated" stays 2026-11-01.

## 9. Follow-ups (next-steps' follow-up list)

- mcflirt's replica: FSL-75's notice and correction (it computes FSL's values that fit since this part, silently).
- docs/validation/difftest.md: the descriptor list gains `fsl-flirt-programs` (FLIRT's other programs and quaternions; its short trials).
- makerot's NaN signs (part 3).

## 10. Generated tables

The branch regenerated README's tool table and the validation index for the FLIRT record's new
counts only (ed7128c; `validation/tests/test_tables.py` requires them in sync); run `python -m
larmorx_validation tables` again after integrating §3 (the new text), and in the site regenerate
`data/status.yml` the same way. The Python golden checksums were re-recorded from 3f6fc31 (every
value unchanged: `tests/golden/golden.json`'s header only).
