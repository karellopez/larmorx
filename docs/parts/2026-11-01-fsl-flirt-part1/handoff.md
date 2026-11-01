# HANDOFF: FLIRT replica, part 1 (2026-11-01)

Branch `worktree-agent-a49385b701ec80acb` (from main 7f6c43b). The exact texts for the shared
documents; integrate and harmonize at the merge. The counts come from the record
`docs/validation/fsl-flirt-replica.md` (standard tier, made from the branch's clean commit
6913b97: 141 cases, 107 bit-identical, 34 rejected by both alike, 0 failures; the first record, from
435d03f, had 137 cases, 103 and 34).

## 1. The defect register (`docs/findings/upstream-defects.md`)

### 1a. Summary table: new rows (after FSL-66). All † (restricted: written from FSL's source).

| FSL-67 † | flirt (BBR, `-wmseg`) | the boundary points' normals: the gradient of the smoothed segmentation (in the segmentation's voxel units) is divided by the voxel sizes of the current reference level (FLIRT's isotropic resampling of the reference), not the segmentation's, before normalising: on an anisotropic segmentation the normals are the voxel-gradient's directions, not the mm-gradient's | c? | small: fMRIPrep's T1w masks are 0.85 × 0.83 × 0.83 mm or similar | kept in both modes; rule D (measure first: part 2) | yes (measure) |
| FSL-68 † | flirt | every error after the command line (an image or matrix file that cannot be read, a matrix of the wrong size, an unknown `-bbrtype`) ends with exit status 0: `main` catches every exception, prints its message and returns 0, nothing written | e | nipype sees the missing outputs only | an error by default (exit 1, FSL's message); `reproduce` exits 0 (rule B); named (2026-11-01) | – |
| FSL-69 † | flirt | a `setscale` the data are too coarse for (`min_sampling > 1.25·scale`, e.g. the default schedule's `setscale 2` on 3 mm data) only resets the input: the previous scale's image pair stays, now comparing its reference level with the clamped, unblurred input (the pair holds references to `main`'s volumes) | c? | every 3 mm BOLD reference registered with FLIRT's default schedule | kept (rule C: FLIRT's results, and the pipelines', depend on it), documented (2026-11-01) | – |
| FSL-70 † | flirt (schedule) | a matrix name FLIRT cannot interpret (`Uq`, an empty word from the trailing-blank quirk) leaves the list pointer uninitialised; `U[` names a 28th list (out of bounds) | a | none | refused in both modes after FSL's message (2026-11-01) | – |
| FSL-71 † | flirt (BBR) | `setoption bbrstep` below 1: the loop over the boundary points never ends (0) or reads before its arrays (negative) | a | none | refused in both modes when BBR's cost uses it (2026-11-01) | – |
| FSL-72 † | flirt (schedule) | `read` or `readparams` of a file with a word that is not a number: the stream fails without reaching its end and the loop pushes rows for ever | a | none | refused in both modes (2026-11-01) | – |
| FSL-73 † | flirt | `-pedir` without a field map read (no `-fieldmap`; or `-applyxfm`, which never reads it): the empty field map is interpolated | a | none | refused in both modes (2026-11-01) | – |
| FSL-74 † | flirt | the usage text lists `-displayinit` (the option is `-printinit`) and leaves `woods` (accepted) out of `-cost` | f | none | the same text (FSL's messages kept) | – |

### 1b. Existing rows: add FLIRT to the tool column

- FSL-09: "mcflirt" → "mcflirt, flirt (`save_volume_dtype`)".
- FSL-11: "mcflirt" → "mcflirt, flirt".
- FSL-14: "mcflirt" → "mcflirt, flirt".
- FSL-21: "mcflirt" → "mcflirt, flirt (`-bins` below 8: 0 bins at the 8 mm scale)"; larmorx's column: "refused in both modes (FLIRT: the heap corruption, then glibc's abort)".
- FSL-66: "mcflirt" → "mcflirt, flirt (a negative number of bins at a scale: FLIRT catches `std::bad_array_new_length` and exits 0, FSL-68)".

### 1c. A restricted section (with the other FSL sections)

```
### FSL-67 to FSL-74: FLIRT (FSL 6.0.7.23, fsl-flirt 2111.4) †

Found while translating FLIRT for the replica (2026-11-01; `docs/findings/fsl-flirt.md`, restricted,
has the source lines). FSL-68 has a switch: by default an error after the command line exits with
status 1 (FLIRT's message unchanged); `reproduce` exits 0 as FLIRT does; the notice names it. FSL-70
to FSL-73 are undefined behaviour and refused in both modes (`larmorx-nc: error: ... (FSL-7n)`).
FSL-67 (the BBR normals' voxel sizes) is to be measured (rule D) before it is corrected or kept;
FSL-69 (the image pair's references) is kept (rule C). FSL-74 is a message only.
```

### 1d. History paragraph / counts
Add: "2026-11-01: FLIRT's replica (part 1): FSL-67 to FSL-74 (one switch, FSL-68; four refusals;
one to measure, FSL-67; one kept, FSL-69; one message)." No new **decide** entry (FSL-67 is rule D:
measure, not the user's).

## 2. `docs/api/upstream-defects.md`: a row (with FSL-65, FSL-66)

| FSL-68 | flirt | the replica `larmorx-nc fsl flirt` (`larmorx fsl flirt`, `lx.fsl.flirt`) | an error after the command line (an unreadable image or matrix, an unknown `-bbrtype`) ends with exit status 0 | exit status 1 with FLIRT's message; `reproduce` exits 0 |

## 3. `validation/tools.toml` (then `python -m larmorx_validation tables`)

```toml
[[tool]]
id = "fsl-flirt"
title = "FLIRT (affine registration, boundary-based registration; convert_xfm)"
eco = "fsl"
page = "/tools/fsl-flirt/"
use = ["larmorx fsl flirt", "larmorx fsl convert_xfm", "lx.fsl.flirt", "lx.fsl.convert_xfm"]
how = "exact replica in `larmorx-nc` (FSL Licence, non-commercial), `experimental` (part 1); the clean-room original to be written from specs/flirt.md"
validation = "replica: [{fsl-flirt-replica.pass} of {fsl-flirt-replica.compared}](record:fsl-flirt-replica) compared cases bit-identical to FSL 6.0.7.23 (matrices, images, exit status and messages): fMRIPrep's boundary-based calls on three oracle runs (their matrices also identical as text to fMRIPrep's own), every cost function, the search options, every shipped schedule, `-applyxfm` 3D and 4D with every interpolation and data type, BBR with its types, coordinates and a field map; {fsl-flirt-replica.both} more rejected by both, alike"

[[record]]
id = "fsl-flirt-replica"
larmorx = "`larmorx-nc fsl flirt`, `convert_xfm`; `lx.fsl.flirt`, `lx.fsl.convert_xfm` (the replica, FSL Licence)"
reference = "flirt and convert_xfm (FSL 6.0.7.23); fMRIPrep's BBR calls also against fMRIPrep's own matrices"
status = "`experimental` (part 1: {fsl-flirt-replica.pass} of {fsl-flirt-replica.compared} compared cases bit-identical, fMRIPrep's calls included; {fsl-flirt-replica.both} rejected by both, with the same messages and status; `-anglerep quaternion`, `alignpaxes` and general parameter subsets wait for part 2)"
```
(The tool entry has no site page yet: the page is part 2. If `tables` needs `page` to exist,
leave the `page` key out until then.)

## 4. README
Generated from tools.toml (above).

## 5. `docs/validation/golden.md`
Under the FSL replicas' golden cases (`crates-nc/larmorx-nc-cli/tests/golden.rs`), add:
"FLIRT and convert_xfm (2026-11-01, ten cases in `BET_CASES`, on BET's generated head: the T2-like
head registered to the T1-like one): `flirt.default` (the default schedule and its search, `-out`),
`flirt.normmi-dof6`, `flirt.weights-leastsq` (`-refweight`, `-inweight`, `-nosearch`), `flirt.bbr`
(FSL's `bbr.sch` as text, `-wmseg`), `flirt.applyxfm-sinc`, `flirt.applyisoxfm-4d-spline`,
`flirt.int16-out` and its `.reproduce` (FSL-09), `convert_xfm.inverse-concat`,
`convert_xfm.fixscaleskew`. The whole crates-nc golden test runs in about 31 s (debug, FLIRT at
opt-level 1 like the core and BET). Recorded from 320e4ec; `flirt.applyisoxfm-4d-spline`
re-recorded from 76520c3 after the `-applyxfm` blur fix (the others unchanged). `lx.fsl.flirt`
and `lx.fsl.convert_xfm` are replica-only functions (`tests/golden/registry.py`,
`REPLICA_ONLY_FUNCTIONS`): their golden cases are these."

## 6. `docs/next-steps.md`, item 7 (after BET's paragraphs)

"**FLIRT: the replica's part 1 done (2026-11-01).** `crates-nc/larmorx-nc-flirt`: `flirt` (every
cost function: `corratio`, `mutualinfo`, `normmi`, `normcorr`, `leastsq`, `labeldiff`, `woods`,
`bbr` with `-wmseg` or `-wmcoords`/`-wmnorms`, its types, slope and a field map; the search; the
pyramid; the schedule language, every command; the default 3D and 2D schedules and FSL's shipped
ones; `-applyxfm`, `-applyisoxfm`; weights; every output type) and `convert_xfm`; `larmorx fsl
flirt|convert_xfm` through the replica dispatch (no original yet); `lx.fsl.flirt`,
`lx.fsl.convert_xfm`; the core gained the weighted costs, `labeldiff`, the unsmoothed entropies,
BBR (`larmorx-nc-core::bbr`), FLIRT's filters and Powell's optimiser (mcflirt's and BET's records
unchanged: 236/236, 178/178). Record [fsl-flirt-replica](validation/fsl-flirt-replica.md): 107 of
107 compared cases bit-identical (34 more rejected by both, alike), fMRIPrep's BBR calls on ds005040, ds003592, ds000228 (their
matrices also fMRIPrep's own as text). `specs/flirt.md` for the clean-room original. FSL-67 to
FSL-74. Memory below FSL's (fMRIPrep's BBR call 395 against 452 MiB, a 12-dof call 247 against 259).
Speed (informal, one thread, a 3 mm BOLD reference to a 0.8 mm T1w): the default 12-dof run 3.1 s
against FSL's 4.6 s, fMRIPrep's BBR call 57 s against 47 s. Random testing: descriptor
`fsl-flirt`; the trial (half an hour, 8 jobs, 3945 cases on images up to 32 voxels per axis) found
928 differences; two causes fixed (`-applyxfm`'s blur beyond the image's edges; the sign of BBR's
NaN cost without boundary points), permanent cases `applyxfm-blur-extraslice` and
`bbr-no-boundary-points-nan`; after the fixes 577 of the 936 findings agree, the other 359 are
triaged for part 2 (`validation/difftest/triage.toml`, `docs/findings/fsl-flirt.md`): FSL
corrupting its heap on small or constant images (281: to be traced, then refused in both modes),
66 matrices (64 with voxels below 0.5 or above 5 mm), the fix pass changing results without a
notice (8: FSL-14 on infinite voxels; `-setbackground` with spline), one message count with
`-dof 3`, one more heap corruption, 2 runs over 3 GiB. **Part 2:**
those, `-anglerep quaternion`, `alignpaxes`, the SVD pseudo-inverse of general parameter subsets,
`-debug`'s BBR volume, FSL-67's measurement, a second random-testing run, the site page, the full
tier (fMRIPrep's 22 calls), the other programs (`avscale`, `makerot`, `midtrans`, `rmsdiff`, `img2imgcoord`,
`img2stdcoord`, `std2imgcoord`, `applyxfm4D`, `pointflirt`). Then the clean-room original (another
agent, from `specs/flirt.md`). The scripts wait for other tools: `epi_reg` (FAST, `fslmaths`,
`fugue`/`applywarp` for field maps, `convert_xfm`, `flirt`), `standard_space_roi` (`flirt`,
`fslmaths`, `fslroi`), `aff2rigid` (`avscale`), `flirt_average` (`flirt`, `fslmaths`,
`convert_xfm`), `pairreg` (`flirt` with `pairreg*.sch`, `bet`)."

And in "4. The ports" of "The order from here": "FLIRT with BBR (part 1 done 2026-11-01; part 2
next)".

## 7. `CLAUDE.md`

- "Ported and validated", the FSL row: append "; the **FLIRT** replica (part 1, 2026-11-01:
  `larmorx-nc fsl flirt`, `convert_xfm`; `lx.fsl.flirt`, `lx.fsl.convert_xfm`; `experimental`):
  107 of 107 compared cases bit-identical to FSL 6.0.7.23 (34 more rejected by both), fMRIPrep's BBR calls on three oracle runs
  included; `specs/flirt.md` for the clean-room original" and add `fsl-flirt-replica` to the records
  column.
- "In place", FSL line: "the mcflirt, BET and FLIRT replicas".
- Hard rule 1, the clean-room implementer's forbidden list: add `fsl-flirt.md` after `fsl-bet.md`
  (also in `docs/porting-guide.md` §1's list: "[findings/fsl-flirt.md](findings/fsl-flirt.md)").

## 8. `docs/licensing.md`
After BET's paragraph in the `larmorx-nc` section: "FLIRT (2026-11-01, part 1) is the third FSL
replica: `larmorx-nc fsl flirt` and `convert_xfm`, translated from FSL 6.0.7.23's fsl-flirt 2111.4
(`flirt.cc`, `globaloptions.{h,cc}`, `defaultschedule.h`, `convert_xfm.cc`; FSL's `bbr.sch` copied
with them), whose files are in `crates-nc/fsl-source/fsl-flirt/` (`scripts/fsl_source.py`; the
wheel's licence files now include `fsl-source/*/*/*` for the schedule)."

## 9. The site (larmorx_documentation)
No FLIRT page yet (part 2). For the Roadmap page (`content/roadmap.md`): FLIRT's row: "FLIRT with
BBR: the replica, part 1 done (2026-11-01: every cost function, BBR, the schedules, convert_xfm;
fMRIPrep's calls bit-identical; a first random-testing trial); part 2 (the trial's open findings,
quaternions, alignpaxes, the page) next; then the clean-room original". "Last updated" 2026-11-01.
The status table: from tools.toml (the `fsl-flirt` tool, without a page link until the page
exists).

## 10. Changed in the branch (not shared documents; for the merge's harmonizing)
- `docs/validation/difftest.md`: the descriptor table gains `fsl-flirt`.
- `validation/difftest/triage.toml`: five FLIRT entries (2 fixed, 3 `unclear` for part 2).
- `validation/src/larmorx_validation/difftest/permanent.py`: `SUITES` gains
  `"fsl-flirt-replica": ("fsl-flirt", "replica")`.
- `tests/golden/registry.py`: `REPLICA_ONLY_FUNCTIONS` gains `lx.fsl.flirt`, `lx.fsl.convert_xfm`.
- `tests/parity/test_fsl_flirt_replica.py` (new: the suite's smoke cases, 7, in the smoke tier).
- Core (`crates-nc/larmorx-nc-core`): the separable convolution reads beyond the edges by the
  volume's extrapolation (zero for the default zeropad, so mcflirt and BET unchanged:
  compare_records 236/236 and 178/178 from 9272446); BBR's normalisation with SSE's NaN.

## 11. For the orchestrator
- The parity smoke tier's marker takes `HEAD` when the run ends: a commit made while it runs gets
  the marker (here 320e4ec, though the run started at 435d03f; only Python files and an unbuilt
  Rust change differed). `parity_smoke.py` could take the commit at its start and refuse to write
  a marker if `HEAD` moved.
- The trial's directory is `<workspace>/oracles/difftest/2026-11-01-fsl-flirt` (a part's trial:
  never the site's night).
