# HANDOFF: FSL FLIRT replica, part 3 (2026-11-01/02)

Branch: the worktree `agent-a0ef3a192c69be27a` (from main 04662aa). Site clone: `<scratch>/site`,
branch `fsl-flirt-part3`. Defect IDs used: FSL-78, FSL-79, FSL-80, FSL-81 (FSL-82 to FSL-89 and
PIPE-18 to PIPE-22 unused). The text below is the exact text for each shared document. The
record's numbers come from `docs/validation/fsl-flirt-replica.md` (re-made in this part from
bc119ff, committed as 8826d3b; mcflirt's from 68ad3fd, committed as bc119ff).

## 1. docs/findings/upstream-defects.md (the register)

Add after FSL-77:

| FSL-78 † | midtrans | `main` catches every `std::exception` of `do_work` (a singular `AAᵀ` in the Procrustes average, a singular input matrix in `M44.i()·v`, `det()` of a non-square matrix, an image that cannot be read), prints `what()` and returns 0: exit status 0 with nothing written | e | none (fMRIPrep does not run midtrans) | rule B: exit status 1 by default with FSL's message, 0 with `reproduce`, named in both (2026-11-01) | – |
| FSL-79 † | applyxfm4D (`Splinterpolator<T>` for integer `T`) | spline interpolation of a char, short or int image: `SplineColumn::Set` stores each deconvolution pass's results as `static_cast<T>(c + 0.5)` (rounded half up, toward zero below −0.5) and the value is returned as `static_cast<T>(value_at(...))` (truncated) before newimage's float: the coefficients of a coarser, shifted spline and integer values before the output's own conversion | c | none (fMRIPrep resamples with its own resampler) | rule A: the float image's coefficients and values by default, FSL's with `reproduce`; named where the written output changes (2026-11-01) | – |
| FSL-80 † | applyxfm4D `-3D` (newimage's kernel cache, fsl-miscmaths `kernel`) | `setinterpolationmethod(sinc)` defines the default kernel (blackman, full width 7) first, and the cache `std::set<kernelstorage*, kernelstorage::comparer>` is ordered by an equality test (true for equal kernels, false otherwise): two different kernels are "not less" both ways, equivalent, so `definesincinterpolation("r", 9)` gets the cached blackman 7 back | c | none | rule A: the rectangular width-9 kernel the program asks for by default, blackman 7 with `reproduce`; named where the written output changes (2026-11-01) | – |
| FSL-81 † | aff2rigid (FSL 6.0.7.23's Python script) | `from numpy import *` then `mat(zeros((4,1)))`: NumPy 2 (FSL's 2.4.6) removed `mat`, so the script stops with `NameError` before writing its matrix | b | none | rule A: the matrix computed by default (identical to FSL's script with `mat` restored on 22 matrices, `validation/probes/fsl-flirt-programs/aff2rigid_mat.py`), FSL's traceback and exit status 1 with `reproduce`, named in both (2026-11-01) | – |

In the FSL-75 row, the "what larmorx does" cell: replace "mcflirt's replica computes FSL's values that fit, without a notice yet (follow-up)" with "applyxfm4D's (a volume 3 voxels wide) and mcflirt's replicas the same since 2026-11-01 (part 3: mcflirt's pads with the whole border by default, FSL's values with `reproduce`, named; FSL aborts with a double free on such a series)".
In the FSL-09 row, append to the last cell: "; applyxfm4D (FLIRT part 3, 2026-11-01): the same correction, int8 kept".
In the FSL-68 row, append: "; midtrans's equivalent is FSL-78; since FLIRT part 3 also the exceptions thrown inside a cost evaluation (a singular matrix: `inv(): matrix is singular`) and newimage's `Out of memory`".
In the FSL-22 row, the where cell: add "applyxfm4D"; append to the last cell: "applyxfm4D's replica the same since 2026-11-02 (its padding value, in the image's type)".

The history paragraph: "FLIRT part 3 (2026-11-01/02): FSL-78 (rule B), FSL-79, FSL-80, FSL-81 (rule A) added; FSL-75 named in applyxfm4D and mcflirt's replica." Counts: +4 entries (FSL-78 class e, FSL-79 and FSL-80 class c, FSL-81 class b), all with switches, none "decide".

## 2. docs/api/upstream-defects.md

Add rows (after FSL-77):

| FSL-78 | midtrans | the replica `larmorx-nc fsl midtrans` (`larmorx fsl midtrans`, `lx.fsl.midtrans`) | an error of the computation (a singular matrix, a matrix that is not 4 × 4) prints its message and exits with status 0, nothing written | exit status 1 with the message, named; `reproduce` exits 0 |
| FSL-79 | applyxfm4D | the replica `larmorx-nc fsl applyxfm4D` (`larmorx fsl applyxfm4D`, `lx.fsl.applyxfm4d`) | spline interpolation of an integer image keeps the spline coefficients and each value in the image's integer type (a coarser, shifted spline) | computed as for a float image, named where the output changes; `reproduce` FSL's values |
| FSL-80 | applyxfm4D | the same | `-3D` with sinc uses the default blackman kernel of width 7 instead of the rectangular one of width 9 it asks for | the rectangular width-9 kernel, named where the output changes; `reproduce` FSL's kernel |
| FSL-81 | aff2rigid | the replica `larmorx-nc fsl aff2rigid` (`larmorx fsl aff2rigid`, `lx.fsl.aff2rigid`) | FSL 6.0.7.23's script stops with a Python error under NumPy 2 before writing its matrix | the matrix computed and written, named; `reproduce` FSL's error and exit status 1 |

FSL-75's row: add "and `larmorx-nc fsl applyxfm4D`, mcflirt's replica (`larmorx-nc fsl mcflirt`) too".

## 3. validation/tools.toml

The `fsl-flirt` tool:

```toml
[[tool]]
id = "fsl-flirt"
title = "FLIRT (affine registration, boundary-based registration; convert_xfm and FLIRT's other programs)"
eco = "fsl"
page = "/tools/fsl-flirt/"
use = ["larmorx fsl flirt", "larmorx fsl convert_xfm", "larmorx fsl avscale", "larmorx fsl img2imgcoord", "larmorx fsl std2imgcoord", "larmorx fsl img2stdcoord", "larmorx fsl rmsdiff", "larmorx fsl makerot", "larmorx fsl midtrans", "larmorx fsl pointflirt", "larmorx fsl applyxfm4D", "larmorx fsl pairreg", "larmorx fsl extracttxt", "larmorx fsl aff2rigid", "lx.fsl.flirt", "lx.fsl.convert_xfm", "lx.fsl.avscale", "lx.fsl.img2imgcoord", "lx.fsl.std2imgcoord", "lx.fsl.img2stdcoord", "lx.fsl.rmsdiff", "lx.fsl.makerot", "lx.fsl.midtrans", "lx.fsl.pointflirt", "lx.fsl.applyxfm4d", "lx.fsl.pairreg", "lx.fsl.extracttxt", "lx.fsl.aff2rigid"]
how = "exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (parts 1 to 3: every program and script of fsl-flirt but `-warp`, `standard_space_roi`, `flirt_average`, `epi_reg`); the clean-room original to be written from specs/flirt.md"
validation = "replica: [{fsl-flirt-replica.pass} of {fsl-flirt-replica.compared}](record:fsl-flirt-replica) compared cases bit-identical to FSL 6.0.7.23 (matrices, images, printed results, exit status and messages): fMRIPrep's boundary-based calls on three oracle runs (their matrices also identical as text to fMRIPrep's own), every cost function, the search options, every shipped schedule and every schedule command (`alignpaxes`, any parameter-subset mask), `-anglerep quaternion`, `-debug`, `-applyxfm` 3D and 4D with every interpolation and data type, BBR with its types, coordinates and a field map, avscale, the coordinate programs, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D in every data type and interpolation, pairreg, extracttxt and aff2rigid; {fsl-flirt-replica.both} more rejected by both, alike; {fsl-flirt-replica.expected} where FLIRT crashes or never finishes (FSL-71, FSL-72, FSL-75)"
```

The `fsl-flirt-replica` record:

```toml
[[record]]
id = "fsl-flirt-replica"
larmorx = "`larmorx-nc fsl flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D`, `pairreg`, `extracttxt`, `aff2rigid`; `lx.fsl.flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4d`, `pairreg`, `extracttxt`, `aff2rigid` (the replica, FSL Licence)"
reference = "flirt, convert_xfm, avscale, img2imgcoord, std2imgcoord, img2stdcoord, rmsdiff, makerot, midtrans, pointflirt, applyxfm4D, pairreg, extracttxt and aff2rigid (FSL 6.0.7.23); fMRIPrep's BBR calls also against fMRIPrep's own matrices"
status = "`experimental` (part 3: {fsl-flirt-replica.pass} of {fsl-flirt-replica.compared} compared cases bit-identical, fMRIPrep's calls included; {fsl-flirt-replica.both} rejected by both, with the same messages and status; {fsl-flirt-replica.expected} expected divergences where FLIRT crashes or never finishes; every program and script of fsl-flirt but the coordinate programs' `-warp` and the scripts of FSL's other programs)"
```

## 4. docs/validation/golden.md

In the table, `larmorx-nc`'s cell: "59 cases, 305 outputs" becomes "65 cases, 313 outputs"; the Python registry gains six replica-only functions (`lx.fsl.midtrans`, `pointflirt`, `applyxfm4d`, `pairreg`, `extracttxt`, `aff2rigid`, mapped to their `larmorx-nc` cases). Paragraph:

"`larmorx-nc`'s checksums recorded on 2026-11-02 from 222e226 (committed as d2300bb) (FLIRT part 3): 6 cases added, `midtrans.two`, `pointflirt.points`, `applyxfm4D.series` (a matrix per volume, sinc, FSL-09's rounding of the int16 output), `pairreg.heads`, `extracttxt.lines` (a negative relative start) and `aff2rigid.default` (FSL-81 corrected), the printing programs' standard output kept as `stdout.txt`; every other checksum unchanged (the core's new LAPACK, its generic background value and the interpolation of double images changed no golden output)."

## 5. docs/next-steps.md, item 7 (the FLIRT paragraph)

Replace the sentence that begins "**Part 3:** the schedule command `alignpaxes` ..." up to "...then the clean-room original from `specs/flirt.md`." with:

"**Part 3 done (2026-11-02).** FLIRT's replica is complete but for the coordinate programs' `-warp` (FNIRT's warp fields: after FSL's warp-field libraries, the follow-up below) and the scripts of FSL's other programs (`standard_space_roi`, `flirt_average`, `epi_reg`: with `fslmaths`, FAST and the field maps). Added: FSL's LAPACK beyond 4 × 4 in `larmorx-nc-core` (`dsyevd`, `dgesdd` with OpenBLAS's Haswell BLAS, every word identical to FSL's library on 400 records); the schedule command `alignpaxes` and every parameter-subset mask (`specs/flirt.md` §8.3, §8.7); `-debug` and BBR's debug volume (§9.4); `midtrans`, `pointflirt`, `applyxfm4D` (every data type and interpolation, the double template included) and the scripts `pairreg`, `extracttxt`, `aff2rigid` (§15.5-15.8), each with its command line, `lx.fsl` wrapper and golden case; makerot's NaN signs; mcflirt's FSL-75 (named, the whole border by default); FSL-78 to FSL-81. Record [fsl-flirt-replica](validation/fsl-flirt-replica.md): 296 cases, 205 of 205 compared cases bit-identical (87 rejected by both, alike; 4 expected divergences: FSL-71, FSL-72 and FSL-75's two); part 2's 217 cases unchanged in status and details (`compare_records.py`); mcflirt's and BET's standard tiers re-run (the core changed): 237 cases, every one of the committed record's 236 identical, the new `edge/fsl75-narrow` rejected by both (FSL aborts with a double free; FSL-75 named) and 178 of 178 identical to the committed record (not re-committed). One trial of the new paths (half an hour, 209,657 cases, after a pre-check of 2,805; the realistic findings re-checked boxed with the final build, the extreme ones deferred to the final validation phase by the user's decision of 2026-11-02); four robustness cases with nonsense headers (metres, voxels of 0.01 and 50 mm) for applyxfm4D, midtrans, pointflirt and pairreg, each identical to FSL in a 4 GiB box. `specs/flirt.md` §14 lists only what is left by decision. **Next:** the clean-room original from `specs/flirt.md` (reusing the main package's clean-room engine, `crates/larmorx-mri`); the full tier, the data panel and long random testing in the final validation phase."

In the order list: "FLIRT with BBR (parts 1 to 3 done 2026-11-01/02; the clean-room original next)".

Follow-ups (next-steps' follow-up list), add:
- "FLIRT: the sign of an all-NaN `-omat` matrix with an infinite input voxel under `reproduce` (FSL-14) and a schedule that optimises relative to a row (FSL `-nan`, the replica `nan`; part 2's path, found by part 3's pre-check, probe-part3b-00496; `docs/findings/fsl-flirt.md`, "Open"): trace the NaN of the input's centre of gravity."

## 6. CLAUDE.md, "Where things are", the FSL row

Replace "the **FLIRT** replica (parts 1 and 2, 2026-11-01: `larmorx-nc fsl flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`; `lx.fsl.*` the same; `experimental`): 152 of 152 compared cases bit-identical to FSL 6.0.7.23 (61 more rejected by both, 4 where FLIRT crashes or never finishes), fMRIPrep's BBR calls on three oracle runs included (their matrices also fMRIPrep's own), `-anglerep quaternion`; FSL-75 (the background value's heap overflow) corrected by default; its site page; `specs/flirt.md` for the clean-room original; part 3: `alignpaxes`, the SVD programs, applyxfm4D" with:

"the **FLIRT** replica (parts 1 to 3, 2026-11-01/02: `larmorx-nc fsl flirt`, `convert_xfm`, `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D`, `pairreg`, `extracttxt`, `aff2rigid`; `lx.fsl.*` the same; `experimental`): 205 of 205 compared cases bit-identical to FSL 6.0.7.23 (87 more rejected by both, 4 where FLIRT crashes or never finishes), fMRIPrep's BBR calls on three oracle runs included (their matrices also fMRIPrep's own), every schedule command, `-anglerep quaternion`, `-debug`; FSL's LAPACK (`dsyevd`, `dgesdd`) in `larmorx-nc-core`; FSL-75 corrected by default (flirt, applyxfm4D, mcflirt); its site page; complete but `-warp` and the scripts of FSL's other programs; `specs/flirt.md` for the clean-room original (§14: only what is left by decision)"

and in "Next:": "FLIRT's replica, parts 1 to 3, done (2026-11-01/02). Next: FLIRT's clean-room original, then FAST, ...".

## 7. validation/difftest/triage.toml

Append to `validation/difftest/triage.toml` (statuses used: `fixed`, `register`, `explained`,
`open`; the deferred extreme findings use `open`, which the evidence export and the tests accept,
with the user's decision of 2026-11-02 in the explanation):

```toml
# ---------------------------------------------------------------- FLIRT's replica, part 3: the trial of the new paths (2026-11-01/02)
# A pre-check (`difftest --tools fsl-flirt-part3`, seed probe-part3b, 2805 cases) and one trial
# (`difftest --tools fsl-flirt-part3 --budget 0.5 --jobs 20 --seed part3-trial`, <workspace>/oracles/difftest/
# 2026-11-01-fsl-flirt-part3: 209657 cases, 209455 agree, 136 notices, 19 differ, 47 over the memory limit).
# Every realistic finding re-checked with the final build in a memory box (recheck_findings.py, memory findings
# skipped); the extreme ones (odd units, voxels of 0.01 to 50 mm) deferred to the final validation phase (the
# user, 2026-11-02: random testing in the porting parts uses realistic cases only).

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["exit status", "files written"]
status = "fixed"
explanation = "FLIRT's part 3: applyxfm4D on a double-precision input was refused; its double template is translated (the interpolated voxels through single precision, the voxels the padding mask sets holding the background value in double precision, the sinc sums adding double products, double spline coefficients). About 100 pre-check cases; every one agrees since"
link = "validation/difftest/cases/fsl-flirt/applyxfm4d-double-padding.json"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["every file bit-identical"]
status = "fixed"
explanation = "FLIRT's part 3: applyxfm4D's spline interpolation of an integer image with reproduce (FSL-79): the coefficients are stored as the type's value of c + 0.5 and each interpolated value is converted to the type before the output's conversion (the replica had truncated the coefficients only); every one agrees since"
link = "validation/difftest/cases/fsl-flirt/applyxfm4d-int-spline-small.json"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["every file bit-identical"]
status = "register"
defect = "FSL-22"
explanation = "FLIRT's part 3: applyxfm4D's background value of a volume with a NaN voxel in its 2-voxel border (trial 39242, 152078): the order of NaN values in the sort is undefined in C++; libstdc++'s order (FSL's build's) with reproduce, NaN sorted last by default, named (as flirt and mcflirt)"
link = "validation/difftest/cases/fsl-flirt/applyxfm4d-nan-background.json"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["files written", "stderr identical"]
status = "fixed"
explanation = "FLIRT's part 3 (trial 57755): a parameter subset whose line search reaches a scale of 0 on one-voxel images: the cost function's inverse of the singular matrix throws inv(): matrix is singular, which FLIRT's main catches (FSL-68), nothing written; the replica had gone on with a NaN matrix. The exception now unwinds to FLIRT's main (larmorx-nc-core exception)"
link = "validation/difftest/cases/fsl-flirt/paramsubset-singular-cost.json"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["stderr identical"]
status = "fixed"
explanation = "FLIRT's part 3: extracttxt with a start before the first line: dc reads its own negative result back as its subtraction command (dc: stack empty) and sed reads a range beginning with - as options (invalid option, GNU sed's usage, exit 1); the replica had printed another sed message. 390 of 390 combinations identical since (validation/probes/fsl-flirt-programs/extracttxt_ranges.py)"
link = "validation/difftest/cases/fsl-flirt/extracttxt-dc-negative.json"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["every file bit-identical"]
status = "open"
explanation = "FLIRT's part 3: the sign of NaN results from inputs with NaN or infinite voxels under reproduce (FSL-14 lets them in): an all-NaN -omat matrix when a schedule optimises relative to a row (trial 65393, 83738, 97568, pre-check probe-part3b-00496; also without a parameter subset, so a path of part 2), and NaN voxels of applyxfm4D's spline interpolation (trial 102752, 122789, 191754): FSL writes nan where the replica writes -nan, or the reverse; every value is NaN in both. Left open (specs/flirt.md §14.2): the operand order FSL's build gives two NaN operands in these sums"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["exit status", "stderr identical"]
status = "fixed"
explanation = "FLIRT's part 3 (extreme cases: voxels of 0.01 mm make the isotropic reference 417 or 585 GB; trial 70320, 116905, 165530): newimage's Out of memory (#99), caught by FLIRT's main (FSL-68); the replica's allocator had aborted. The volume is allocated fallibly and the exception unwinds to FLIRT's main. Re-checked once before the user's decision of 2026-11-02 (agree); not run again"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["larmorx exceeds the memory limit"]
status = "open"
explanation = "FLIRT's part 3: the trial's 47 runs over its 3 GiB limit, all extreme cases (flirt schedules with alignpaxes or a parameter subset on voxels of 0.01 to 50 mm): deferred to the final validation phase (the user's decision of 2026-11-02: random testing in the porting parts uses realistic cases only); queued in the trial's heavy.jsonl. One measured in the pre-check (probe-part3b-01970): FSL 8,966 MiB, the replica 8,342, the matrices identical (memory_findings.py)"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["original exceeds the memory limit"]
status = "open"
explanation = "FLIRT's part 3: the trial's runs where FSL's flirt itself exceeds 3 GiB (extreme cases, as above): deferred to the final validation phase (the user's decision of 2026-11-02)"
link = "docs/findings/fsl-flirt.md"

[[difference]]
tool = "fsl-flirt-part3"
implementation = "replica"
signature = ["notice: FSL-79", "notice: FSL-80", "notice: FSL-09"]
status = "explained"
explanation = "FLIRT's part 3: the trial's 136 notices without a visible change: FSL-79 and FSL-80 were named where the coefficients or the kernel changed but not the written image, FSL-09 for voxels the padding mask overwrites. Now named where the written output changes (both conversions compared, masked voxels left out): 131 of the 136 agree on a re-run (difftest --case, boxed); in 5, FSL-79 or FSL-80 and FSL-09 meet in one voxel and cancel (a float spline's 2.6 rounded to 3, FSL's integer 3 truncated to 3): notices by design"
link = "docs/findings/fsl-flirt.md"
```

(If `signature` must name one check per entry, split the last entry into three with the same text.)


## 8. docs/validation/difftest.md

In the tools table, after the `fsl-flirt-programs` row:

| `fsl-flirt-part3` | `parity.fsl_flirt_replica` (the same payload; registered for part 3's trial of the new paths) | replica vs flirt, midtrans, pointflirt, applyxfm4D, extracttxt | midtrans of 2 to 4 drawn transforms (`-o`, `-v`, `--debug`, `--separate`, `--template`); pointflirt of 1 to 20 points and their drawn affine images (`-o`, `-v`, `--vox`); applyxfm4D on 3D and 4D images of every stored type with every interpolation (`-3D`, `-singlematrix`); flirt schedules with `alignpaxes` or a drawn parameter-subset mask; extracttxt with drawn words, counts and relative starts, negative ones included (since 2026-11-01) |

## 9. The site (larmorx_documentation)

The branch `fsl-flirt-part3` of the site clone `<scratch>/site` (from 92fa368): the FLIRT page's
part 3 text (`content/tools/fsl-flirt.md`: the lede and description, the verdicts paragraph with
the record's new counts and the trial, the programs section with midtrans, pointflirt,
applyxfm4D and the scripts, the corrections FSL-78 to FSL-81, the known limits) and the evidence
re-exported from the record (`static/evidence/fsl-flirt.json`, the manifest). Its images are
unchanged (39 blobs, 29.97 MB; the page's JSON 0.23 to 0.28 MB, its cases 217 to 296; every other page unchanged). The clone fetched `evidence-blobs` from GitHub (2a38d50, 592 blobs:
the local repository's branch is still at 79a1750); `tools/blobs.py commit` was not needed (the manifest is unchanged); site commit 1f5773b. Checks: build 0 problems (links into this worktree checked), number check 125 of 125, viewer check 18 of 18 (5 examples), inventory per page after >= before (`<scratch>/inventory-before.txt`, `inventory-after.txt`), screenshots before and after at 1440 and 390, light and dark (`<scratch>/shots/`).

For the orchestrator (shared site files):
- `data/status.yml`: regenerated by `python -m larmorx_validation tables` from §3's tools.toml.
- `content/roadmap.md`, row 1 ("FLIRT with BBR"), the status cell: "The replica complete
  (2026-11-01/02, parts 1 to 3): `flirt`, `convert_xfm`, `avscale`, the coordinate programs,
  `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D`, the scripts `pairreg`,
  `extracttxt`, `aff2rigid`, every cost function, BBR, every schedule command, 205 of 205
  compared cases bit-identical to FSL 6.0.7.23, fMRIPrep's BBR calls included; [its page](/tools/fsl-flirt/);
  the clean-room original next". A "Completed" row: "| 2026-11-02 | **FLIRT's replica, part 3.**
  The rest of FSL's FLIRT package: `midtrans`, `pointflirt`, `applyxfm4D` in every data type (double
  precision included), the scripts `pairreg`, `extracttxt`, `aff2rigid`, the schedule's
  principal-axes alignment and any parameter subset (FSL's own LAPACK translated word for word),
  `-debug`; FSL-78 to FSL-81 corrected by default; [the page](/tools/fsl-flirt/) |"; "Last updated".
- Navigation: no change (the page exists).


## 10. Generated tables

After 3: `python -m larmorx_validation tables` (README's tool table, the validation index, the site's status).

## 11. Notes for the orchestrator

- `validation/investigations/fsl-flirt/recheck_findings.py` (26ccc60) refuses to run outside a memory box,
  holds each worker in a leaf with the trial's per-case limit and its address-space limit, skips memory
  findings unless `--memory` (then one at a time), at most 8 spawn workers (after the 00:08 crash).
- New core module `larmorx-nc-core::exception` (resume_unwind payloads for FSL's exceptions thrown inside
  value-returning computations): the FLIRT run and the larmorx-nc entry point catch them; mcflirt's and BET's
  records re-run after it (mcflirt 236/236 + 1 new, BET 178/178 identical).
- The local `larmorx_documentation` repository's `evidence-blobs` branch is at 79a1750 while GitHub's is
  2a38d50 (592 blobs); the site clone fetched GitHub's. The page's blobs are unchanged: no blobs push needed.
- The difftest descriptor's midtrans `--template`/`--separate` now pass FSL's long-option form (=value);
  before, every drawn case with them was rejected by both programs alike.
