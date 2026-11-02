# FLIRT spec answers (2026-11-02): handoff

Branch `worktree-agent-a1899e54855e47816` (worktree `<workspace>/larmorx/.claude/worktrees/agent-a1899e54855e47816`),
from main fb1de31. Relays 1 to 3. The exact changes to the shared documents are in `HANDOFF.toml`
beside this file (`python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --check`:
9 operations apply to 6 files on this branch). This file gives the reasons.

## What the part did (own files, already in the branch)

- `specs/flirt.md`: the 20 clean-room questions answered in §3, §4.1, §4.2, §6.2, §6.5, §7.2, §7.4,
  §8.1, §8.2, §8.5, §8.7, §9.2, §9.3, §12 (every new or changed line marked "(spec writer,
  2026-11-02)"); §14's table "The clean-room questions of 2026-11-02" (rows 1 to 25: the 20
  questions, the two candidate refusals, the three leads); FSL-90 to FSL-93 in §4.1, §4.2, §6.4,
  §8.1 step 9, §11 and §13. `specs/mcflirt.md` appendix A: corratio's and Woods' terms corrected and
  the sentence for `lx.mri.hmc`'s later part.
- FLIRT's replica (`crates-nc/larmorx-nc-flirt/src/run.rs`): FSL-90 (search starting points composed
  as Euler angles by default), FSL-91 and FSL-92 (`check_registrable`: refused by default, FLIRT's run
  with `reproduce`), FSL-93 (`set_basescale`: a header size of 0 counted as 1 by default, refused with
  `reproduce`; `-basescale 0` refused in both modes); `larmorx_core::defects` FSL_90 to FSL_93
  (behaviour only).
- The replica suite (`validation/src/larmorx_validation/parity/fsl_flirt_replica.py`): input prefixes
  `@zero:` and `@zerovox:`, `Run.hang_fixed`, five cases (`real/weights-empty-refweight`,
  `real/weights-empty-inweight`, `edge/one-voxel-input`, `edge/zero-voxel-size`, `edge/basescale-0`),
  FSL-90 in `_SILENT` (a notice about the starting points, whose effect may not show).
- larmorx-nc's golden cases: `flirt.quaternion-dof6` names FSL-90 (its checksum unchanged), three new
  (`flirt.fsl91.empty-refweight`, `flirt.fsl92.one-voxel` by their status 1; `flirt.fsl93.zero-voxel-size`'s
  matrix); `golden.tsv` re-recorded from 6af5ff9, every other checksum unchanged.
- `docs/findings/fsl-flirt.md` (restricted): three sections of the part (the costs and BBR; the search,
  the outputs and their precision; the refusals and leads), with FSL's routine names.
- `validation/investigations/flirt-spec-answers/`: the models, FSL's runs (`fsl_runs.py`), the replica in
  both modes (`replica_runs.py`), README with every number.

## The shared documents (HANDOFF.toml), and why

1. `docs/findings/upstream-defects.md`, three inserts: the summary rows FSL-90 to FSL-93 (behaviour
   only, as the table's rule asks), the history paragraph (FSL-94 and FSL-95 unused; no new
   **decide** entry), the detailed section `### FSL-90 to FSL-93` after FSL-78 to FSL-81's. Classes:
   FSL-90, 91, 92 (c) (deterministic, results changed), FSL-93 (a) (an undefined conversion of
   infinity; FLIRT never finishes). D13 gap "–" for each (the replica has both modes); the rows say the
   clean-room original follows in its part 2. The register's **decide** count is unchanged.
2. `docs/api/upstream-defects.md`: the users' rows FSL-90 to FSL-93 after FSL-81's.
3. `docs/api/fsl-flirt.md`: one paragraph in "Upstream defects (D13)". **This page is shared with the
   clean-room original's part 2 (running beside this part): apply the op after both merges; if its
   anchor line ("mcflirt's replica (named in both modes, the whole border by default).") moved, put
   the paragraph after the part-3 programs' paragraph.**
4. `docs/next-steps.md` item 7: the spec questions answered (the brief's text: "the spec questions
   answered; FLIRT's clean-room part 2 can close its open cases", with the sections and the models'
   numbers), and the `lx.mri.hmc` follow-up's "A spec-writer question first." replaced by the answer
   (`specs/mcflirt.md` appendix A).
5. `validation/tools.toml`: FSL-93 among the replica's expected divergences (FLIRT never finishes on
   `edge/zero-voxel-size` and `edge/basescale-0`). Then `python -m larmorx_validation tables`.
6. `validation/difftest/triage.toml`: the clean-room original's open registrations entry names the two
   candidates; its words now say they are FSL-91 and FSL-92 (the entry stays open for its other causes).

## For the orchestrator at the merge

- **The clean-room suite (`validation/src/larmorx_validation/parity/fsl_flirt.py`, the clean-room's
  file, not touched here) runs the replica suite's cases, so it gains the five new ones.** Its
  `_undefined_case` knows `hang` and `crash` but not `hang_fixed`: for `edge/zero-voxel-size` the
  original's default must finish naming FSL-93 (as the replica's `_hang_case` now checks), and only
  `reproduce` refuse. Until FLIRT's clean-room part 2 implements FSL-90 to FSL-93 (spec §11), the
  clean-room record will show these cases failing (`real/weights-empty-*` and `edge/one-voxel-input`
  compare its `reproduce` run with FSL's arbitrary matrix: the clean-room cannot be expected to give
  that matrix bit for bit on an ill-posed registration; the clean-room part should treat them as the
  replica's random-testing triage does, or as both-refuse cases by default). A brief item for FLIRT's
  clean-room part 2 (or its next relay): FSL-90 to FSL-93 from `specs/flirt.md` §11, and the
  `hang_fixed` branch in `fsl_flirt.py`.
- FSL-92 refuses one-voxel registrations by default, which the random-testing shrinker reaches
  often: `edge/background-overflow` (FSL-75's crash on an 8 × 1 × 1 reference) now has a 2 × 1 × 1
  input (FSL still crashes there; the replica finishes in both modes naming FSL-75, as before), and
  the permanent case `difftest/background-nan-order` (a 1 × 1 × 1 reference) still passes, its
  default now refused by FSL-92 before FSL-22's padding (its fix pass: 0 values changed, FSL-92
  named, where the earlier record had FSL-22's one value; `reproduce` unchanged). FSL-22's correction
  in FLIRT's padding is now covered on a real image by the new case `real/applyxfm-nan-border`
  (`-applyxfm` of the BOLD reference with NaN border voxels: 1,149,794 values changed by the default,
  FSL-22 named; `reproduce` FSL's bit for bit), beside applyxfm4D's `@nanedge` cases.
- The mcflirt and BET replicas' records are unaffected (only `larmorx-nc-flirt` and
  `larmorx_core::defects` changed); `larmorx-core` gained four constants only.
- No site change in this part (the FLIRT page's evidence is unchanged; the four entries could appear in
  the site's defect list when it is regenerated).
