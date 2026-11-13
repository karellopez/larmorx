# The documentation revision, step 1: the final report (writer relay 28, verbatim)

## Relay 28: step 1 is ready to merge

The reviewer's round 25 is fixed, and the merge material for step 1 is done. Both branches are clean:
- **Repository:** `worktree-agent-a82773ee467015bd4` at **08da1e80**, 140 commits not in main.
- **Site:** `docs-revision` at **edede81**, 65 commits on top of b768293a, so the site's main fast-forwards.

**Round 25**
- **A (important), its own commit, site bd4a694:** the roadmap and the home page's planned list now show the documentation revision and FreeSurfer as in progress. FreeSurfer's text: its preparation done on 2026-11-12, its foundations under way.
- **B (important), its own commit, site d8b706f:** the random-testing check names now show in words on the page; `triage.toml` and `difftest.json` are unchanged. 3dvolreg's own triage table got the same words (repository 77b7842d).
- **Minors C to N and the reviewer's notes:** all fixed (site 737fb9a; repository 73e32390, b8cddfd5, 08da1e80). Among them:
  - the runs table's total is 1,398,782 (BET's run of two ports counted once);
  - the replica comes before the clean room in the matrix and in both random-testing tables;
  - the resampler's added default panel has its poster (no new image);
  - the verdict "Within the thresholds", which does not exist, is gone.
- **Checks after the fixes:** build 0 problems; every changed page's numbers agree (Validation 551/551, for example); against d8c07df every page shows as much or more; the image list is unchanged. Answers are in NOTES.md, "Review answers, Round 25".

**Inventory against the live site (b768293a)**
- Every NO row is one of three things:
  - one of the 29 old addresses, now redirects;
  - Getting started losing 84 words, because the packages' full paragraph is on Concepts now;
  - numbers that only moved: 3dvolreg's rows swapped, the head-motion table reordered, BET's default view showing the same values.
- The only numbers that changed are the three records made again: 3dTshift's replica, antsRegistration and antsAI.
- All 7,325 anchors of the old pages land on the new pages.
- The 28 evidence files lose no number or field.
- 18 images were added (t2smap's clean room); none removed or changed.
- No new external links.

**Other checks**
- Screenshots: `shots-m1/` (every page's top at 1440 and 390, light and dark, 268 in all) and `shots-r25/` (round 25's changes).
- Gates: 1,670 passed and 14 skipped. One known flaky test (`test_difftest.py::test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on`) failed under load and passed alone twice. ruff, format, SPDX, tables and reference checks are clean, and the site has no machine paths.

**Trial merge (current main 81e665e8 into the branch)**
- Three conflicts, each solved by keeping both sides: README's generated table, `docs/findings/README.md`, and `__main__.py`'s `PARITY_MODULES`.
- **Without HANDOFF op 26, a test fails after the merge.** Main added `larmorx nipype run`, and `test_reference.py::test_programs_page_data_matches_the_code` wants it in `validation/programs.toml`. Op 26 adds it and names the confounds' five records.
- With the handoff applied: the merged site builds with 0 problems; the Python gates give 1,716 passed, 21 skipped and the same flaky test.

**Handoff, apply after the merge**
- `HANDOFF.toml` has 28 operations on 4 files:
  - ops 1–25: `docs/next-steps.md`;
  - op 26: `validation/programs.toml`;
  - op 27: NIB-04's wording in the register;
  - op 28: the decision log's entry for your two-step decision.
- `HANDOFF.md` gives the merge order:
  - after the merge: run `tables`, `tables --site` and `reference all --site`;
  - `tools/blobs.py commit` (1,454 images), then push `evidence-blobs` before the site's main;
  - push larmorx main before the site.

The trial merge and the site build of relay 27's state stay in scratch as git worktrees (`tools/r28/mergetest`, `before-r28`); remove them with `git worktree remove --force` when done.

## Step-1 report for docs/parts/

This tool would not let me write a report file, so the text is here.

# The documentation revision, step 1: report (relay 28)

**Status.** Step 1 (your decision of 2026-11-13 to publish in two steps) is ready to merge.
- Repository: branch `worktree-agent-a82773ee467015bd4` at 08da1e80, 140 commits not in main (six of them earlier merges of main).
- Site: `docs-revision` at edede81, 65 commits on b768293a.
- Handoff: `HANDOFF.toml` (28 operations, applied after the merge) and `HANDOFF.md`, checked on a trial merge of main 81e665e8.

**What step 1 publishes**
- **On the site:**
  - The 25 evidence pages at `/evidence/<page>/`, in the writing guide's words and BET's layout. Each worked example shows the original and its ports side by side, with a "What this shows" line and its verdict, and every figure is titled by what it compares. The old `/tools/` addresses redirect and keep every anchor.
  - t2smap's clean room added to its three worked examples. This is the only new computation: 18 images made from cached oracle outputs.
  - The home page and the header.
  - Getting started, Concepts (with Why Rust and Python under it) and About.
  - Programs, with BET's three reference pages.
  - Validation reorganised, with the data panel under it.
  - The roadmap (with the confounds' handoff applied) and the 404 page.
  - The random-testing tables brought up to date.
  - No machine paths anywhere.
- **In the repository:**
  - `docs/writing-guide.md`.
  - The shared presenter and every evidence module's `present`.
  - The reference exporter and `validation/programs.toml`.
  - 26 files renamed from `*-original*` to `*-clean-room*`, with pointer files left at the old names.
  - `figures/cache.py`'s `machine_paths`.
  - The helpers in `validation/investigations/docs-revision/`.

**Checks against the published site**
- Build: 0 problems, 67 pages (38 pages and 29 redirects).
- Inventory: every page shows as much or more. The only numbers that changed belong to the three records made again (3dTshift's replica 315 → 317 cases; antsRegistration 576 → 584 and 11 of its 36 thread-band values; antsAI 171 → 172) and to Validation's summary of them.
- All 7,325 anchors of the old pages land on the new pages.
- The 28 evidence files lose no number or field; the image list grows from 1,436 to 1,454.
- The site check finds no home path, workspace or worktree folder, account name or machine name in any file.
- Every changed page's numbers agree in the browser.
- Screenshots of every page's top at both widths and in both themes.
- Gates as above.

**Reviews and your approvals**
- The critical reviewer's rounds 1 to 25: every finding fixed or answered (NOTES.md, "Review answers").
- You approved the writing guide's five answers and the four pilots (2026-11-12) and the two steps (2026-11-13).

**Step 2 (the next day)**
- Q1–Q5: a reference page for every program.
- D1–D4: the repository's documents, API pages, the wrappers' printed lines and other code words in the new words.
- F: `programs.py`'s rule for `validated`.
- G: the list of the clean rooms' printed lines still in the old words, for a clean-room agent.
- The confounds' pages, then removing their planned card.
- M2: step 2's merge material.
- Left from the reviews:
  - the verdict hover text (`verdicts.yml`'s `means`) for "within the original's own variability";
  - the sidebar's indentation of Why Rust and Python;
  - CITATION.cff's words.
- Seen under load and passing alone, not touched here: the memory-limit test of `test_difftest`, `test_sdcflows_app`'s LAPACK test and `test_oracle_cache`'s false-alarm test.
