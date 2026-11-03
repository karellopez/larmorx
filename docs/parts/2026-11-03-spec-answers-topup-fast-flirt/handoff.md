# Handoff: the spec answers for topup, FAST and FLIRT (2026-11-03, relays 1 to 5)

Branch `worktree-agent-aaf7b58fc18c485aa` (worktree `.claude/worktrees/agent-aaf7b58fc18c485aa`,
from main d0c6e83, main 1eac18c merged in at 7a3fc62). The exact changes to the shared documents
are in `HANDOFF.toml` beside this file (34 operations on 7 files, `--check` clean in the worktree
and with `--site` in the site clone; every changed TOML file still parses). This file gives the
reasons, and for each clean-room original what the answers let it close.

## What the part changed in the branch (no handoff needed)

- `specs/topup.md` (§2.6, §2.7, §4.2 to §4.7, §5, §6, §7.2 to §7.5, §9.4, §10.1, §10.2, new §13),
  `specs/fast.md` (§3.1, §4.1, §4.4, §4.5, §4.8, §4.10, §5.2, §5.3, §5.6, §5.8, §6, §7, §8.3, §9.1,
  §10, §11, new §12), `specs/flirt.md` (§2.2, §2.3, §4.1, §6.2, §6.4, §6.5, §7.3, §8.1, §8.2, §9.2 to
  §9.4, §11, §12, §14's new table, §15.1 to §15.8), `specs/mcflirt.md` §6.1 (question 25, as the
  orchestrator allowed): every new or changed line marked "(spec writer, 2026-11-03 ...)".
- The restricted findings `docs/findings/fsl-topup.md`, `fsl-fast.md`, `fsl-flirt.md`: "The spec
  answers of 2026-11-03" and "The replica's switches".
- The replicas (`crates-nc/`): FSL-111 (FLIRT's applyxfm4D), FSL-144, FSL-145, FSL-146, FSL-201
  (topup), FSL-202, FSL-203 (FAST), FSL-204 (pointflirt); `larmorx_core::defects` (behaviour
  only: FSL-125, FSL-134 and FSL-145 corrected, FSL-201 to FSL-204 new); the parity cases; the
  API pages `docs/api/fsl-topup.md`, `fsl-fast.md`, `fsl-flirt.md`; the measurement hooks kept as
  patches in `crates-nc/*/dev-patches/` (restricted location).
- The records `docs/validation/fsl-topup-replica.*` (176 cases), `fsl-fast-replica.*` (87),
  `fsl-flirt-replica.*` (305) from clean 53796ce; **c7ac893 regenerated the generated blocks of
  README.md and docs/validation/README.md** in the branch (`python -m larmorx_validation tables`,
  from the three records' counts; nothing else edited), so `tables --check` passes; re-run it at
  the merge (and the site's `data/status.yml` from it).
- `crates-nc/larmorx-nc-cli/tests/golden.tsv` re-recorded from c7ac893 (fabf9c2).
- `validation/investigations/spec-answers-3/` (every model and measurement, README with the
  numbers); the evidence modules `fsl_fast.py`, `fsl_topup.py` (and the figures cache's
  `merge_rerun`, `recompute`) for the site (below).

## The reasons for each operation

- **The gap legend's "original"** (op 0): no code existed for "corrected in the replica, the
  clean-room original still to do"; the old meaning (3dTshift's original `reproduce`) has had no
  entry since 2026-10-12, so the word takes the new meaning with its history kept in the text.
- **FSL-111, FSL-144, FSL-145, FSL-146 rows** (ops 1, 4 to 6): the replicas now correct them by
  default (sub-task 4), so the gap "replica" becomes "–". FSL-145 gets gap **reproduce**: the
  clean-room original's `reproduce` lays two coefficients where FSL lays one (an axis shorter than
  one spacing) and where FSL lays the general count (a spacing of 2), and it runs where FSL stops
  (a level refitting one coefficient). FSL-111's replica names it in its default mode only (it
  resamples both readings there to compare); `reproduce` reads as FSL without a notice (the
  orchestrator's item).
- **FSL-125 and FSL-134 rows** (ops 2, 3): relay 1b found the behaviour text wrong or incomplete:
  a negative `--fwhm` acts as no smoothing (not "a negative-width kernel"); a field of the images'
  voxel count in another shape is laid as stored with its own Jacobian at the same indices, 0
  beyond the field's grid.
- **FSL-201 to FSL-204 rows** (op 7), **the history paragraph** (op 8), **the restricted section**
  (op 10): the four new entries, behaviour-only in the summary, the source lines only in the †
  section (appended at the file's end, which is the restricted FSL section). FSL-201 gap
  **reproduce** (the original's default is already the exact derivative; its `reproduce` must
  follow §4.6's forward difference); FSL-202, FSL-203 and FSL-204 gap **original** (the clean-room
  originals follow in their next parts). **FSL-202 affects fMRIPrep users**: fMRIPrep's FAST
  call's default outputs move, toward the exactly computed statistics (the row's numbers);
  `reproduce` is unchanged. The register's count paragraph is yours to redo
  (`register_counts.py`: four entries added, three (c), one (a)).
- **FSL-111's black-box section** (op 9): its D13 gap is closed.
- **docs/api/upstream-defects.md** (ops 11 to 15): "both" for FSL-111, FSL-144 to FSL-146; rows for
  FSL-201 to FSL-204.
- **golden.md** (ops 16, 17): larmorx-nc's 121 cases and 679 outputs, the four new cases and the
  checksums that moved on purpose (FSL-201, FSL-202, FSL-145). The main package's golden file is
  unchanged.
- **next-steps** (ops 18 to 24): the replica follow-ups done; topup's, FAST's and FLIRT's spec
  questions answered, with each clean-room original's follow-ups (below); the glibc
  single-precision functions as a question for the orchestrator and the user only (moving Arm's
  `sinf`/`cosf` out of the AFNI replica crate needs the user); `lx.mri.hmc`'s blocked centres.
- **specs/bet.md §3.1** (op 25): FAST's question 10 showed the image layer mirrors an ANALYZE
  image only when the product of its stored voxel sizes is positive, and applies no scale field;
  BET's spec said "x reversed" without the condition. Not this part's spec: apply it, or ask
  BET's spec writer.
- **triage.toml** (ops 26 to 32): seven entries of the clean-room originals now explained by the
  specs (statuses unchanged: the originals still differ until their next parts); the topup
  movement entry gains `defect = "FSL-201"`.
- **The site's Roadmap** (op 33, `site:`): the completed row of this part.

## What the answers let each clean-room original close (its next part)

- **topup** (`specs/topup.md` §13): (1) FSL-201's `reproduce` (§4.6's forward difference) and the
  default's notice: the record's 9 expected divergences of finding 14 (`opt/levels`,
  `opt/scg-movements`, `opt/four-volumes`, `opt/zero-readout`, `cfg/b02b0`, `cfg/b02b0_2`,
  `cfg/b02b0_4`, `cfg/b02b0_7T`, `difftest/fsl145-short-axis`) and the trial's 4,462 cases (in
  `reproduce`); (2) §4.6's bending-energy sampling: the 47 quadratic random cases (10753); (3) §7.4's
  row-by-row matrices: the 8 `lsr` cases on lines dividing 2^32 (8089); (4) FSL-133's read with
  `reproduce`: `at/lsr-long`, `at/vb2d-long`, `at/vb3d-long`, `at/vb4d-long`, the trial's 484; (5)
  FSL-134's other shape with `reproduce`: `at/jac-other-shape` (refused today; a parity case once
  reproduced); (6) §4.5's truncated prefilter: the short-axis random cases (7283) and vb transients
  on small images; (7) FSL-145's `reproduce` exactly (one coefficient when n < k, the general count
  at k = 2 for n = 2, 3, FSL's refit error) and applytopup reading either count by default; (8) to
  check: its FSL-146 trigger with a volume of read-out time 0. Stays open: fits the data hardly
  determine (another minimum as low as FSL's), FSL's own band on the 78 cases without one (the final
  validation phase).
- **FAST** (`specs/fast.md` §12): (1) FSL-202 and FSL-203 by default and `reproduce` (its record
  then holds the default to the replica's default, `reproduce` to FSL with FSL's band); (2) §4.1.1's
  `_restore` NaN at NaN voxels; (3) §7's lost unflushed lines at an abort (the `-v` abort cases,
  the trial's singular k-means case); (4) §5.3: case 1840 (`MEAN -nan`); (5) §10's lag-2 pipeline
  confirmed; (6) §5.8's one-step near ties expected (thresholds unchanged). Stays: `multi/s5`, the
  noise-free and rank-deficient random cases (the matrix arithmetic's last bits, §8.6).
- **FLIRT** (`specs/flirt.md` §14, the 2026-11-03 table): the 13 numbered follow-ups of the
  next-steps item (op 24): the verbose levels, FSL-90's start and messages, avscale's signs,
  midtrans' non-finite determinant, FSL-204 and `nan` read as 0, `-warp`'s three volumes, NaN
  inverses with `reproduce` (#2905, #704, #8361), pairreg's schedules, `-debug`'s forms and BBR
  without boundary points (the triage's 114 findings, #2424), the C library's `sinf`, `cosf`,
  `atan2f` for `reproduce` (`real/interp-sinc`, applyxfm4D's float sinc, the `real/dof6`-like
  trace partings: depends on the orchestrator's question), applyxfm4D's spline (5,177 voxels of
  five findings), `leastsq`'s sums (`real/cost-leastsq`), the equal and NaN costs of FSL-91/92's
  `reproduce` runs (with `atan2f` and the near-zero skew's sign, which this part did not find:
  the original's own arithmetic).
- **`lx.mri.hmc`** (`specs/mcflirt.md` §6.1): the blocked centres of mass.

## The site (branch `spec-answers-3-pages` in `<scratch>/site`, blobs branch `spec-answers-3-blobs`)

`spec-answers-3-pages` = 4a70f2e (c030e59, 4a70f2e on site main a50a8ce); `spec-answers-3-blobs` = d3ee6ba
(994 blobs, the manifest's SHA-256 de0abdf20c1a6da0; push it as `evidence-blobs` before `main`). Exports from clean
commits of this branch: FAST 3b65d4e, FLIRT 4c047cf, topup e3ad16b. What changed: FAST's replica default panels
(FSL-202: its six examples' default-mode runs made again; fMRIPrep's three calls now a *deliberate correction* row,
ds000005 54 `_seg` and 112 `_mixeltype` voxels, ds000228 66 and 89, ds005003 392 and 453) and its case explorer (87
cases); topup's membrane example (FSL-117 and FSL-201 named), its case explorer (176 cases), a new example
`#movements` (`opt/levels`, FSL-201, with a movement table: the replica's default and the clean-room original within
3.7e-6 mm and 1.9e-7 rad, FSL 3.1e-4 mm and 8.4e-5 rad away), its triage texts; FLIRT's replica record (305 cases,
FSL-111 named in its uint8 cases); the three pages' texts. Checks: build 0 problems (links into the worktree
checked), blobs verify 0 problems, number check FAST 1641/1641, topup 943/943, FLIRT 319/319, viewer check 520, 182,
38 with 0 failing, inventory after >= before on all 21 tool pages (`site_inventory.py compare`), screenshots before
(site main) and after at 1440 and 390, light and dark (`<scratch>/shots/sel-before`, `sel-after`). The evidence reads
no `triage.toml` text for these pages, so applying ops 26 to 32 needs no re-export; `summary.json` and
`data/status.yml` are yours to regenerate at the merge.
