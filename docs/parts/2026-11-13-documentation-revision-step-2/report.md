# The documentation revision, step 2: the writer's final reports (relays 29 to 34, verbatim)

## Relay 29

Relay 29: step 2 has started. Q1 is done, and Q2 is done for 8 of its 14 programs (Q2a and the first part of Q2b). Both branches are committed and clean.

- **Setup:** I fast-forwarded the worktree to main (7d068bec). The site's `docs-revision` branch equals `origin/main` (714c9d2). The step-2 baseline is `<scratch>/before-step2`: the live site built, 67 pages, 0 problems. I also copied the main checkout's fresh `_core.abi3.so` into the worktree.
- **Q1 (81c737fe):** the reference exporter can now read the command lines of every family:
  - ANTs: both the `opt()` table and the help-menu form (`DenoiseImage`, `antsRegistration`).
  - AFNI's clean rooms, with AFNI's global options (`3dTshift`).
  - `mri hmc`, read for mcflirt's page.
  - tedana's parser in Rust and sdcflows' Python parser. Their synopsis is formatted by Python's own argparse at 80 columns.
  - Where a usage text is the clean room's own words (AFNI's clean rooms, `mri hmc`), the page credits it as "The clean room's usage", never to AFNI or FSL.
  - 7 new tests; BET's three pages are byte-identical.
- **Q2's readers (b312b609):** they now handle the other forms FSL's programs use, so they find every option of all 14 FLIRT programs with nothing unparsed. Probe script: `tools/r29/probe_flirt.py`.
- **Q2a (9b071efe, 42edbb3c; site fdefc3b):**
  - `docs/api/fsl-flirt.md` now opens with a Ports section, then the `flirt` and `convert_xfm` sections, and gains a History section. The options tables cover every option the code accepts. The examples are tested.
  - I dropped one stale sentence: the intro said midtrans, pointflirt, applyxfm4D and the scripts had only a replica; the page's own clean-room section and the replica registry say they have a clean room too.
  - `flirt.py`'s docstrings use the new words.
  - When a Python function lacks one of LARMORX's own arguments (`convert_xfm` has no `n_threads`), its page says the option is the command line's only.
- **Q2b part 1 (aede82eb; site bf7c061):** `avscale`, the three coordinate programs, `rmsdiff` and `makerot` now have their own sections, replacing the old part-2 table; the removed text is in `tools/r29/flirt-part2.removed.md`. `flirt_tools.py`'s docstrings are updated, and I added a coordinates input kind for the examples. FLIRT's evidence page now links all 8 reference pages.
- **Checks:**
  - The site builds with 0 problems (75 pages).
  - Reference pages: 11, with 0 problems and 0 stale.
  - The inventory against the step-2 baseline shows no regression.
  - Screenshots are in `shots-q2a/` and `shots-q2b/`.
  - Gates: 1,741 passed and 14 skipped; ruff, format, SPDX, the tables and the summary, panel-run and site-path checks are clean.
  - One test failed in each gates run, and both pass when run alone. Both were already on the known flaky list: `test_difftest`'s memory-limit test and `test_sdcflows_app`'s LAPACK test.
- **Next (in NOTES.md):** Q2b part 2 (midtrans, pointflirt, applyxfm4D, pairreg, extracttxt, aff2rigid), then Q3. "Ready for review" holds the first chunk (Q1, Q2a, Q2b part 1) for the reviewer's round 26.
- **For you to decide:**
  - Several FLIRT Python functions lack LARMORX's own arguments, mostly `n_threads`. This is a code question for the wrappers' owners, not wording.
  - On the site, inline code that starts with a hyphen can break after it at a line end. The fix is a CSS or renderer change for every page, so it should go in a later chunk.

## Relay 30

Relay 30 is done: Q2b part 2 and all of round 26 are finished, and Q3 is about half done. Both trees are clean: repository edd93b0c, site 68c45b1. NOTES.md has a "Ready for review" entry for each chunk, the Round 26 answers and the "Next" for relay 31.

- **Q2b part 2** (repository 6592c7f6; site 29ccc77): FLIRT's API page now has all 14 program sections. The six new ones are `midtrans`, `pointflirt`, `applyxfm4D`, `pairreg`, `extracttxt` and `aff2rigid`. Each has tested examples and counts its cases from both records. The page's later sections are reworded (the heading is now "The clean room (part 1, 2026-11-02)"), and the flirt_more.py docstrings are reworded too.
- **Round 26** (repository 873ae2f5; site 91054e6): all 4 important and 5 minor findings are fixed.
  - flirt's Ports now uses the record of 2026-11-07: 239 of 240 compared cases, 0.0113 mm.
  - Every program's Ports gives its own case counts, and the exporter now refuses "(above)" (new test case).
  - The defects heading is "Defects of the original, corrected by default", as on BET's page.
  - `FlirtResult` and `AvscaleResult` have Attributes sections.
  - The site builder no longer shows backticks in page titles.
- **Q3 so far:**
  - The command-line readers were extended (5b0698f3): usage text in a file of its own, option tables of tuples, a usage text with `${FSLDIR}` filled in, descriptions after a colon, and a new `parser:` key for `standard_space_roi`.
  - Four API pages are now in sections, with docstrings reworded and Parameters added: fsl-roi (d29f4591), fsl-prelude (9a65152c, e8d8c08b), fsl-fast (2e461799) and fsl-topup (edd93b0c).
  - The site has 6 new reference pages, and their evidence pages link to them.
  - Several numbers in these pages' old status paragraphs were out of date; they now come from the current records (listed for the reviewer in NOTES).
- **Checks:** gates in an 8 GiB box with 4 workers: 1,766 passed, 14 skipped; ruff, format, SPDX, tables, reference, summary, panel-runs and site-paths all clean.
  - Reference check: 22 pages, 0 problems, 0 stale. Site build: 87 pages, 0 problems.
  - The inventory against before-step2 shows no regression. To get there I reworded BET's Reference line, because the inventory counted its removed parentheses as lost words.
  - Screenshots are in `shots-q2b2/` and `shots-q3/`.
- **Next (relay 31):**
  - fsl-stats and fsl-maths: fslmaths' operation table needs a reader first, because 35 spellings are not found yet.
  - mri-hmc, then Q4.
  - Left for D1 and G: the wrappers' printed messages "(the clean-room original) failed" in prelude.py, fast.py and topup.py, and applytopup.rs's `NOT_YET` line in the clean room.

## Relay 31

Relay 31 is done. Q3 is complete and Q4 is under way (3dAutomask and 3dUnifize). I did not start the user's two requests (A, the FreeSurfer roadmap page; B, the confounds and the other finished tools on the site) or the merge of main c9ab2a4e: my context was nearly used up. Relay 32 starts with those, in that order.

- **Commits, repository** (branch worktree-agent-a82773ee467015bd4, head 8735289f; tree clean):
  - 2295e734: the reader changes. FSL's usage lines that start at the beginning of the line or after `preoption`, with their values before a colon. fslmaths' `-kernel` lines become one option. fslmaths' operations are read from its tables of operations (the `-Tmean`/`-Xmean`… forms count as one option per statistic). Python's `*options` and `*operations` appear in signatures.
  - 6d7d3f5d fsl-stats, 68989737 fsl-maths, fc0e0af9 mri-hmc (mcflirt's page).
  - 370906a2 afni-automask, 8735289f afni-unifize. This commit also reads a usage line's "(also -oink)" as another spelling.
- **Commits, site** (branch docs-revision, head 49e4f9a): one commit per program, 4f18ebe, 1db3db0, d51e09c, bf57356 and 49e4f9a. Each holds the new reference page, `data/programs.yml` and the evidence page's Reference line naming the reference page.
- **Each API page now:** a Ports section, then the program's section with its reference block, Python call, options table (every option the clean room accepts), tested examples and ports. The wrappers' docstrings use the new words and have Parameters; programs.toml's `does` now comes from the API page.
- **Numbers that changed:** the mri-hmc and fsl-maths API pages were stale and now carry the current records' figures.
  - **mri-hmc (record of 2026-11-11):** 170 cases, 0 failures.
    - The old "one case misses its threshold" paragraph moved to History.
    - On fMRIPrep's command the matrices are 0.0151–0.0274 mm from mcflirt's, against mcflirt's own spread of 0.0164–0.1014 mm.
    - 230 of 11,475 matrix files are identical to mcflirt's.
    - The stale "72 mm woods run" sentence is replaced by the record's figures: 0.0204 mm median and at most 15.9 mm on real runs.
  - **mcflirt's replica (record of 2026-11-02):** 205 of 205 cases bit-identical.
  - **fslmaths' clean room (record of 2026-11-07):** 454 cases, 386 within its thresholds, 370 identical to FSL.
- **Facts added to readable pages** come only from the pages themselves, the current records, spec section numbers (hidden options: fslstats' `-D`, fslmaths' `-pow`, `-invbin`, `-tfce_minT`, `-tfce_delta`) and the clean rooms' own code (`-datatype`/`-output_datatype` as spellings of `-dt`/`-odt`, `-oink`).
- **Checks (self-checked, no reviewer):**
  - The site builds with 0 problems (92 pages).
  - `reference all --site --check` reports 0 problems and 0 stale pages (28 pages).
  - The inventory against before-step2 shows no page with less after every chunk.
  - summary_check, panel_runs_check and site_paths_check all report 0.
  - Final gates in an 8 GiB box: 1,785 passed, 14 skipped, 0 failed, and ruff, format, SPDX and tables pass. The earlier run after Q3 had one failure, the known flaky memory test in test_difftest, which passes when run alone.
  - No screenshots: no layout changed.
- **Next for relay 32** (written into NOTES.md, "State (relay 31 done)"):
  1. Merge main c9ab2a4e into the worktree.
  2. (A) The FreeSurfer roadmap page with `data/freesurfer-roadmap.yml`.
  3. (B) The confounds' evidence page and every finished tool on the Programs and Validation pages.
  4. The rest of Q4: 3dTshift, 3dvolreg, then ANTs' six (the pattern is in 370906a2 and 8735289f; `tools/r31/refline.py` updates the Reference lines).
  5. Q5, the reduced D1, D2's one paragraph through the handoff, and a lean M2.

The wrappers' printed "(the clean-room original) failed" messages are left for the reduced D1, as you decided.

## Relay 32

Relay 32 is done. Both of the user's requests are built and committed; nothing is pushed or merged yet.

1. **Main merged in.** Main c9ab2a4e is merged into the worktree as 64943297. Main's changes to `defects.rs` made the 28 reference pages out of date, so I exported them again; only their stamps changed (site 8f0c893).

2. **(A) FreeSurfer's roadmap page, `/roadmap/freesurfer/`** (site 0874c10). It covers:
   - what the step delivers and the two streams (classic and FreeSurfer 8's networks);
   - a "where it stands" table of the waves;
   - every step of `recon-all` in order (before the first step, autorecon1, 2 and 3): each step's programs in plain words, what it does, its stream and its wave;
   - fMRIPrep's programs outside `recon-all`;
   - the waves 3.0 to 3.7 and N, each with what it delivers, its size, its programs and its status for the replica and the clean room;
   - how each program is validated, and the reference runs.

   All statuses live in `data/freesurfer-roadmap.yml`. Its header tells you how to update it at each merge without touching the page. Wave 3.1 has 13 parts, with today's statuses from your ~17:30 entry. The page names programs and stages only; its one repository link is `docs/freesurfer-plan.md`. It is linked from the roadmap's FreeSurfer row, the home page's planned card, the Programs page's FreeSurfer group and the sidebar. On a phone its tables stack. Screenshots are in `shots-r32/fsroadmap-*`.

3. **(B) The confounds' evidence page, `/evidence/confounds/`** (repository 89cd51d0 to 75722946; site 6ca0943, 0889887, 3c9ffd2).
   - I added `present()` to `confounds.py` with a test, and exported the page from the clean commit e52defd5.
   - The page has the 88 cases (4,181 checks), fMRIPrep's 36 confounds files column by column, four runs, the masks' Gaussian resampling with its chain of causes, the corrections, speed, memory and random testing (514,694 calls).
   - In `tools.toml` the confounds row now has its page, and its software is fMRIPrep (it was marked as LARMORX's own). `programs.toml` links the page for `lx.confounds` and `nipypecli run`.
   - The Validation summary has its row (placed last, so existing rows keep their places), and random testing's runs table lists the confounds' run and its two checks. It is in the sidebar under fMRIPrep.
   - The "Confounds, their pages" planned card is removed, as you decided.
   - Every other finished program and module already had a place on Programs and Validation; nothing else was missing.
   - Screenshots are in `shots-r32/confounds-*`.

4. **Checks.**
   - The site builds with 94 pages and 0 problems.
   - The inventory against before-step2 flags only two things, both expected: the home page lost one heading (the removed card), and three evidence files changed by additions only.
   - Gates: 1,788 passed and 14 skipped. The two known flaky tests (test_difftest's memory test, test_sdcflows_app's LAPACK test) failed under load; both pass when run alone. ruff, format, SPDX, tables, tables --site, reference, summary, panel-runs and site-paths all report 0 problems.
   - I edited these shared files directly; they are listed in NOTES. Site: `data/status.yml`, `site.yml`, `content/roadmap.md`, `content/programs/index.md`, `site.css`. Repository: `tools.toml`, `programs.toml`.

5. **For the merge:** the evidence-blobs branch was re-made locally (fbd1c674, 1,501 blobs). Push it before the site's main.

6. **Next, relay 33:** 3dTshift's API page is 494 lines to rework, which is why I stopped here. The order is 3dTshift, then 3dvolreg, then ANTs' six, then the confounds' reference pages (at least `nipypecli run`), then Q5, D1 reduced, D2's paragraph and M2 lean.

Heads: repository 75722946 in `<workspace>/larmorx/.claude/worktrees/agent-a82773ee467015bd4`, site 3c9ffd2 in `<scratchpad>/docs-revision/site`. Both trees are clean and no server is running. NOTES, under "State (relay 32 done)", is at `<scratchpad>/docs-revision/NOTES.md`.

## Relay 33

Relay 33 is done. Heads: repository 4cc88549 (worktree-agent-a82773ee467015bd4), site faea557 (docs-revision). Both trees are clean. NOTES.md has "State (relay 33 done)" and a "Next" for relay 34.

- **Q4 is complete, each program in its own commits.** 3dTshift f937acf0 and 3dvolreg c3d0f9f0 follow the pattern of 370906a2. The clean rooms' old words are replaced in both pages and wrappers. The six ANTs programs are done: DenoiseImage, N4BiasFieldCorrection, Atropos, antsAI, antsAffineInitializer, antsApplyTransforms and antsRegistration (d6b12dba to 9faeeb38). Every page now has a reference block, a tested example and its numbers from the current records. Several pages were out of date and are corrected: N4 now 46 compared cases, Atropos 97 cases / 58 compared, antsAI 132 / 144 cases, antsRegistration 513 / 355, 3dTshift 317 / 282 / 180. The site has all their reference pages and the Reference lines on their evidence pages.
- **The confounds:** `nipypecli run` has its reference page (2a6157aa; site /programs/fmriprep/nipypecli-run/). To make it, the exporter gained:
  - a reader for the click command that `nipype run` emulates;
  - page addresses with a hyphen for a blank in the program's name;
  - an optional `software` key in the reference block.

  `docs/api/confounds.md` has no reference block: `lx.confounds` is a module, not a program.
- **Exporter changes for 3dvolreg and the other pages:**
  - 3dvolreg's usage layouts can now be read.
  - Options compared one by one with `arg == "-x"` are found.
  - The options table accepts `-rlt+`.
  - Repeated help lines on a reference page now show once, so the 3dTshift and flirt pages lost their duplicates.
  - The examples have three new input kinds.
  - Two new tests cover the new readers.
- **Q5 has begun:** MultiplyImages is done. Its evidence page (ants-programs) now has a Reference line, and `refline.py` refuses to insert into it a second time, so relay 34 must add the next ANTs programs to that line by hand.
- **D1 reduced, part 1:** the wrappers' printed messages now say "(the clean room) failed" and "no clean room of ...", in 8b51b5ac with a fix in 4cc88549. While doing this I renamed volreg's private `_run_original`, which broke 5 golden cases because niworkflows imports it. The name is restored and the tests are updated. D1's `n_threads` part is not done: the list of wrappers that lack it (FLIRT's programs, fslroi, standard_space_roi and two ANTs functions) is in "Next".
- **D2:** a new `<scratch>/HANDOFF-step2.toml` with two edits (one paragraph each in `docs/porting-guide.md` §11 and `docs/figures.md` "Add a tool"). `apply_handoff.py --check` says both apply. The reasons are in `HANDOFF-step2.md`.
- **Checks at the final heads:** 1,824 passed and 14 skipped. One failed under load: the known flaky difftest memory test, which passes alone. ruff, format, SPDX, tables, tables --site, the reference check (0 problems, 0 stale), summary, panel runs and site paths all give 0. The site builds 105 pages with 0 problems. The inventory against before-step2 shows no regression (`tools/r33/inventory-final.md`).

Shared documents changed directly:
- site: `data/programs.yml` (generated) and the Reference lines on ten evidence pages;
- repository: `validation/programs.toml` (`does` removed for the 11 programs whose API page now says it).

Relay 34's next steps: D1 `n_threads` (wrappers, `.pyi`, tests, API signature lines), then the rest of Q5, then M2 lean; push evidence-blobs fbd1c674 or newer before the site's main.

## Relay 34

Relay 34 is done. Step 2 is ready to merge: the repository is at `518913e7` (branch worktree-agent-a82773ee467015bd4) and the site at `95b117b` (docs-revision). Both trees are clean. evidence-blobs `fbd1c67` is unchanged and already on origin.

- **D1, `n_threads` (1152e822; site 27d4970):** every wrapper in your list now takes `n_threads=1` (0 = all). That is FLIRT's 13 other programs and scripts, `fslroi`, `standard_space_roi`, `distance_map` and `apply_transforms_to_points`.
  - The FSL wrappers pass it to the program as `OMP_NUM_THREADS` and use it for reading and writing images. `distance_map` uses it only to read a path, because the map runs on one thread. `apply_transforms_to_points` accepts it, but maps the points on one thread.
  - A negative count raises `ValueError`. No Rust code changed and `_core.pyi` is unchanged, since only the Python wrappers' signatures changed.
  - New test `tests/python/test_n_threads.py`: every public function of `lx.ants`, `lx.afni`, `lx.fsl`, `lx.mri` and `lx.tedana` takes `n_threads` except six helpers that run no program, and results are equal at 0, 1 and 3 threads.
  - The API pages' signature lines are updated. I added one CHANGELOG Unreleased bullet directly in the branch, because `CHANGELOG.md` is part of every reference page's source stamp.
- **Q5 is complete: all 46 programs with a command now have a reference page.** New this relay: ImageMath, ThresholdImage, SmoothImage, ResampleImage, ResampleImageBySpacing, t2smap and sdcflows' BIDS app. Each has examples the tests run, and every number comes from the current records. ResampleImageBySpacing's and ImageMath's pages still had older records' numbers, now corrected.
  - The exporter now names a restricted record (t2smap's replica record) instead of linking it, with a test.
  - I added new example inputs: three echoes and a small PEPOLAR BIDS dataset.
  - The old words are fixed in the tedana and field-map pages and in `t2smap.py`'s docstrings.
  - The modules (transforms-hdf5, ndimage-filters, io and the others) have no command line, so they get no reference page. They keep their API pages.
- **M2, lean:** I compared the site against the live site `3c9ffd2`. 110 of 111 pages are at least as complete as before, 0 shown numbers changed, nothing removed, and the 1,531 evidence files are byte-identical. The site builds 112 pages with 0 problems. No layout changed, so I took no screenshots.
  - The one page marked as a loss is `programs/fsl/flirt` (2831 → 2808 words). This comes from relay 33's change that prints a help text once per row. With the help text taken out the page is identical line for line, and no help text is gone (`tools/r34/flirt_words.py`).
- **Handoff:** `HANDOFF-step2.md` now has relay 34's notes, with no new ops. The two existing ops still apply after a trial merge with main `5efa89e5`, which merged cleanly.
- **After you merge, re-run `python -m larmorx_validation reference all --site <site>` and commit the site.** Main's `crates/larmorx-core/src/defects.rs` changed since the branch, and that file stamps every reference page. On the trial merge the re-export changed exactly one stamp line in each of the 46 pages and no content.
- **Gates** (8 GiB box, 4 workers): 1,879 passed and 2 failed under load. The two are the known flaky tests (oracle cache `test_verify_command` and the sdcflows LAPACK test); both pass when run alone. ruff, format, SPDX, tables, reference, summary, panel-runs and site-paths checks all report 0 problems.
- **Follow-up for the final phase:** the function libraries (`lx.sdcflows` helpers, `lx.niworkflows`, `lx.confounds` and others) still have functions without `n_threads`. They were not in my list.

Everything is in <scratchpad>/docs-revision: `NOTES.md` ("State (relay 34 done)"), `HANDOFF-step2.md`, `tools/r34/inventory-2a.md` and `tools/r34/gates.log`.
