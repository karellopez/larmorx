# The documentation revision, step 2: handoff (relay 33 onward)

The exact changes are in `HANDOFF-step2.toml` (apply with
`python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF-step2.toml`, from the merged checkout).

## D2, reduced (the orchestrator's 2026-11-13 ~17:15 entry; promised to the user)

- `docs/porting-guide.md` §11, after the per-tool contract's table: one paragraph naming the writing guide, the API
  page's reference block and `python -m larmorx_validation reference` (the reference page, the options table checked
  against the code; the model `docs/api/afni-automask.md`) and the evidence module's `present()` as the way a new
  tool's API page and site pages are made, so the FreeSurfer agents' and later briefs follow the new layout.
- `docs/figures.md`, "Add a tool", after step 7: one paragraph: the evidence page's new address
  (`content/evidence/<tool>.md`, the old one a redirect), `bet`'s layout, `present()` and the guide's words, the
  figure-title pattern, the three implementations side by side with "What this shows" and the verdict, the colours;
  the reference page from the API page's reference block.

Both files are readable by clean-room implementers: the paragraphs name only readable files (the writing guide,
`apipage.py`, `present.py`, `ants_n4.py`, `afni-automask.md`) and no fact of a replica.

## Relay 34: no new ops; what the merge needs

- **CHANGELOG.md** was edited in the branch (1152e822: one bullet under Unreleased, Changed: `n_threads` on every
  program's Python call), not through an op: every reference page's source stamp includes `CHANGELOG.md`, so an op
  applied after the merge would make all 46 reference pages stale. Harmonize its wording if needed, then re-export
  (below).
- **After merging into main: re-export the reference pages** (`python -m larmorx_validation reference all --site
  <site>`, then commit in the site). main changed `crates/larmorx-core/src/defects.rs` since the branch (the FreeSurfer
  tracks), and that file stamps every reference page. A trial merge of the branch head with main 5efa89e5 (scratch
  `tools/r34/mergetest`, clean, no conflict) re-exported gives 46 pages that differ by exactly that one stamp line
  each, no content (tools/r34/site-mergetest). The two ops above still apply to that merge (`--check`: 2 apply).
- **Shared documents changed directly in relay 34:** CHANGELOG.md (above); validation/programs.toml (`does`
  removed for ImageMath, ThresholdImage, SmoothImage, ResampleImage, ResampleImageBySpacing, t2smap and sdcflows:
  their API pages' reference blocks say it); the site's data/programs.yml (generated) and the Reference lines of
  content/evidence/ants-programs.md, tedana-t2smap.md and sdcflows-estimation.md.
- **The inventory against the live site 3c9ffd2** (tools/r34/inventory-2a.md): 110 of 111 pages after >= before,
  0 shared shown numbers changed, 0 removed, the 1,531 evidence files byte-identical. The one NO,
  programs/fsl/flirt (words 2831 -> 2808), is relay 33's exporter fix that prints a help text once per row instead of
  once per option the row names: with the help spans taken out the page's 314 lines are identical, and no help text
  is gone (tools/r34/flirt_words.py).
- **Q5's modules without a reference page** (transforms-hdf5, sdcflows' fieldmap module, niworkflows-helpers, bids,
  resample-series, fieldmap-steps, ndimage-filters, skimage, io, confounds): no command line, so the generator has
  nothing to check a table against; they keep their API pages and the Programs page's `does`. Every program with a
  command (46) has its reference page.
- **D1 is complete for the program wrappers:** every public function of `lx.ants`, `lx.afni`, `lx.fsl`, `lx.mri`
  and `lx.tedana` takes `n_threads` but six helpers that run no program (tests/python/test_n_threads.py keeps it so).
  The libraries of functions (`lx.sdcflows`' helpers, `lx.niworkflows`, `lx.bids`, `lx.confounds`, `lx.transforms`,
  `lx.io`, `lx.skimage`, `lx.ndimage`) were not in the list and still have functions without it (43, 29, 4, 10,
  10, 3, 3, 7): a follow-up for the final phase if rule 5 is meant for them too.
