Worktree (repository): <workspace>/larmorx/.claude/worktrees/agent-a82773ee467015bd4
Branch: worktree-agent-a82773ee467015bd4 (from main ba876f7f)
Site clone: <scratchpad>/docs-revision/site
  (branch docs-revision, from main b768293a; identity = the no-reply address; commit-msg guard
  installed in its .git/hooks; evidence blobs fetched: 1436 blobs, 350.8 MB, 0 problems; branch
  head 4f3093d (relay 15). Build with the workspace's venv, no .venv of its own:
  <workspace>/.venv/bin/python tools/build.py --main-repo <worktree>; serve with --serve --port
  8791 (add python -u, else its log stays empty); screenshots: node 22 at
  ~/.nvm/versions/node/v22.23.2/bin, tools/review/shoot.mjs --review 0 [--select css] [--nojs 1])
The worktree's python/larmorx/_core.abi3.so (git-ignored) is a copy of the main checkout's build
of 6e18b57d (no Rust changed since in this branch): run Python with
PYTHONPATH=<worktree>/python:<worktree>/validation/src after `source scripts/orchestration/env.sh`.

# The documentation revision: notes

## Ready for review
(One line per finished chunk of the rollout: its commits and pages. The reviewer checks it while
the writer continues; findings in reviews/round-<n>-<chunk>.md.)
- **S1 (relay 11): N4BiasFieldCorrection's evidence page and the shared presenter.** Repository
  829963e2, 5cf0ae67, d030a098, a89dd7af, 76222ccd (evidence/present.py new with its test
  validation/tests/test_evidence_present.py; evidence/ants_n4.py's `present`; words.py one rule;
  validation/tools.toml and programs.toml: N4's address; the helpers in
  validation/investigations/docs-revision/). Site fafe803, 9b2fc9b:
  content/evidence/ants-n4.md (new: the page in the guide's words and bet's layout),
  content/tools/ants-n4.md (a redirect now), static/evidence/ants-n4.json (exported again from
  the cache: texts and the port/order/shows fields only), site.yml (N4's entry), data/status.yml
  and data/programs.yml (generated), tools/evidence.py and static/css/evidence.css (the band, the
  chain, the random-testing panel and the status matrix on presented pages). Check against the
  old page (/tools/ants-n4/ at site 6574906, its facts all kept): scratch
  tools/r11/ants-n4.tools-before.md. Screenshots: scratch shots-s1/.
- **S2 (relay 12): DenoiseImage's and antsApplyTransforms' evidence pages, with the reviewer's
  round 9 on S1 applied.** Repository 31f42a05, 85f50e2d, 6cee019b, 4d2e9ec9, 16a21833,
  c2062050, 8a80d301, 90ee800b (evidence/ants_denoise.py's and ants_apply_transforms.py's
  `present`; present.py: the labels "larmorx --float 1, ..." and "ANTs 2.6.5 double, ...",
  WARNS and `warnings` for a correction that only warns (ANTS-36), panel_shows with it;
  ants_panel._cmd names each file ("openneurosub-16_..." was one word); ants_n4.py: the tumour
  example's What this shows in the review's words, the band's `software`; tests in
  test_evidence_present.py (8 now); validation/tools.toml and programs.toml: the two
  addresses; validation/investigations/docs-revision/page_old_words.py). Site 778f41b, 2b2306e:
  content/evidence/ants-denoise.md and ants-apply-transforms.md (new, the guide's words and
  bet's layout), content/tools/ants-denoise.md and ants-apply-transforms.md (redirects),
  content/evidence/ants-n4.md (the review's lede and ANTS-36 text), static/evidence/
  ants-denoise.json, ants-apply-transforms.json and ants-n4.json (exported again from the
  cache: texts and the port/order/shows/presentation/software fields only), site.yml (the two
  entries), data/status.yml and data/programs.yml (generated), content/programs/fsl/bet*.md
  (tools.toml's new hash only), tools/evidence.py (the band's key names the page's software).
  Check against the old pages (site 9b2fc9b, every fact kept): scratch
  tools/r12/ants-denoise.tools-before.md and ants-apply-transforms.tools-before.md.
  Screenshots: scratch shots-s2/.

- **S3 (relay 13): antsRegistration's and Atropos' evidence pages, with the reviewer's round 10
  on S2 applied (minors 1-8).** Repository f3bbcb26, 75455b5f, f91e0bf1, 4284819d, aa30e403,
  411d48b3, 3719c31b (evidence/ants_registration.py's and ants_atropos.py's `present`;
  present.py: `run_rows`/`table(runs=True)` (a table of an example's runs named as its panels,
  no bracket inside another), `example_table` (an example's own tables), `dots` (the thread
  band's dots chart), a port's run without a panel named by its id, "a fixed seed and the
  clock's" (Atropos' `lxseed-seed1`), the notice "larmorx: upstream defect ..." kept as printed,
  a port named without its program when it is the page's own, the original against a known
  answer in its own group; tests in test_evidence_present.py (13 now); ants_denoise.py: round
  10's minors 4, 5, 6, 7b; validation/tools.toml and programs.toml: the two addresses;
  docs/writing-guide.md rule 7: the lede's two counts in two sentences). Site 1bc3691:
  content/evidence/ants-registration.md and ants-atropos.md (new, the guide's words and bet's
  layout), content/tools/ants-registration.md and ants-atropos.md (redirects), the three S2
  pages (minors 1-3: the ledes, antsApplyTransforms' 4 threads, glibc's functions),
  static/evidence/ants-registration.json, ants-atropos.json and ants-denoise.json (exported
  again from the cache), site.yml (the two entries), data/status.yml and data/programs.yml
  (generated), content/programs/fsl/bet*.md (tools.toml's new hash only), tools/evidence.py
  (the dots chart on a presented page: its title with code spans, its series in their ports'
  marks; the agreement chart over two records links both by name), static/css/site.css (a
  lede's code on one line). Check against the old pages (site 2b2306e, every fact kept):
  scratch tools/r13/ants-registration.tools-before.md and ants-atropos.tools-before.md.
  **For the reviewer: antsRegistration's JSON now reads its standard record of 2026-10-28**
  (the page showed the record of 2026-10-23; the record was made again on the main machine with
  the 8 permanent random-testing cases added since, commit 3899deb4, and never exported): 584
  cases (was 576), the agreement's new category `difftest` (8 bit-identical) moves the panel's
  two rows down by one, and ANTs' own thread band, measured again, moves 11 of its 36 values
  (tools/r13/shown-check-reg.txt lists every shown number that moved; no case removed, no
  verdict changed). Screenshots: scratch shots-s3/ (reg-*, reg2-* the final lede; atropos-*;
  s2fix-* the S2 pages' changed parts).

- **S4a (relay 14): ImageMath and the filters' and antsAI's evidence pages, with the reviewer's
  round 11 on S3 applied.** Repository 87d94d96, 41b25405 (address), be72cb2f, 496c6844,
  f5a921c5 (the priors' pattern, page_old_words.py), 16351618 (address), 6dac5a72
  (summary_check.py), cda2af69
  (ants_programs.py's and ants_ai.py's `present`; present.py: a page of several programs
  (`Page.programs`; an example of several programs' calls names the software only), a label
  that begins with its program, the original's or the port's own result ("ANTs 2.6.5's blobs",
  "larmorx's blobs": "its blobs"), a modeless port panel named by the page, "default mode
  (ANTS-112 corrected)" in one bracket, `finish(pages=...)`, fMRIPrep's panel without its
  program's prefix, the guard of round 11's X1 note, `<fMRIPrep run>` in facts too;
  common.difftest_descriptions; tests (17 in test_evidence_present.py); tools.toml and
  programs.toml: the two addresses). Site 5f8bd6c, 5f47891, 6660758: content/evidence/
  ants-programs.md and ants-ai.md (new), content/tools/ants-programs.md and ants-ai.md
  (redirects), static/evidence/ants-programs.json and ants-ai.json (exported again), site.yml,
  data/status.yml, data/programs.yml, content/programs/fsl/bet*.md (hash only); round 11:
  static/evidence/summary.json (made again), ants-registration.json and ants-atropos.json
  (exported again), content/evidence/ants-atropos.md (lede, corrections). Check against the old
  pages (site 1bc3691): scratch tools/r14/ants-programs.tools-before.md and
  ants-ai.tools-before.md. Screenshots: scratch shots-s4/.
  **For the reviewer:** (1) antsAI's JSON now reads its standard record of 2026-10-28 (the page
  showed 2026-10-24's; commit ee7a191a): one case more, `difftest/scales-warnings-before-
  multistart` (bit-identical, the agreement's new category *random testing*); no case removed,
  no verdict changed (compared by id and by category). (2) **The exports ran with
  LARMORX_FMRIPREP_RUNS = the data panel's fourteen runs** (scratch tools/r14/panel-runs,
  links): the panel cases' descriptions ("the same call as 13 other node(s): ...") are made at
  export from the runs on disk, and the nine fMRIPrep runs of 2026-11-08 would have added nodes
  the records never compared (22 instead of 13, three datasets more in the credits). With the
  fourteen, every description is the old one but for the two words. **Every later export of a
  page with data-panel cases needs this** (AFNI, FSL, sdcflows pages too, where they use
  fmriprep_nodes). (3) A fact the old page had wrong: ImageMath's "ANTS-30 ... awaits the
  user's review" is "approved by the user (2026-10-22)" in the register's row: History says so,
  the corrections paragraph links the API page's design. (4) The lede counts: ImageMath 303 and
  306 of 308 (the record's "Bit-identical: 303 of 308 passing" and the five classified on the
  old page: three `^` with correctly rounded maths, two signed zeros), 374 of 374 on the panel;
  antsAI 75 and 76 of 76 (the record's summary line), 38 and 40 of 40, antsAffineInitializer 26
  of 26 and 14 of 14 as built. (5) antsAI's blobs-rotated example has no verdict chip: its only
  comparison is the replica against the known answer (no verdict in the JSON).

- **S4b (relay 15): ITK transform files' evidence page, with the reviewer's round 12 on S4a
  applied (both important findings and the minors 3-4; 5 answered).** Repository f31277fd,
  65f88519 (address), 96c50708, 995188ca (itk_hdf5.py's `present`: the tables' columns
  ("h5py ↔ replica", "Values (the replica's reading)"), titles, notes, rows and cells, the
  cases, the agreement chart (titled "Agreement, replica against the original: ITK 5.4.5's
  transform files, and HDF5 as h5py reads it": the record reads every file with h5py too) and
  random testing named by the replica; its categories named (files refused, HDF5's storage
  features, reading: the test data's / fMRIPrep 25.2.6's / files h5py wrote / files ITK wrote,
  writing); "in `fix`, its default, it stores the empty parameters" (ITK-15's case); present.py:
  `Page.part`, a page whose original is a part of its software, not a program ("ITK 5.4.5's
  transform files" in the titles); a test (17 in test_evidence_present.py); tools.toml and
  programs.toml: the address. Round 12: parity/fmriprep_nodes.py reads only the data panel's
  fourteen runs (`PANEL_RUNS`, committed; LARMORX_FMRIPREP_RUNS now only names another folder
  of them), validation/investigations/docs-revision/panel_runs_check.py (new: every run named in
  a page's data panel is one of the fourteen; in every chunk's checks from now on),
  page_old_words.py's `option` lines (an option of LARMORX's own command line without "next
  release"), ants_atropos_pv.py (the mask's voxels in the fix-ants reason)). Site 4be03f1,
  4f3093d: content/evidence/itk-hdf5.md (new: the guide's words and bet's layout),
  content/tools/itk-hdf5.md (a redirect), static/evidence/itk-hdf5.json (exported again from the
  cache: texts and the port, title, comparison, presentation, category and random-testing
  fields only), site.yml (the entry), data/status.yml and data/programs.yml (generated),
  content/programs/fsl/bet*.md (tools.toml's new hash only); round 12: static/evidence/
  ants-denoise.json and ants-atropos.json (exported again, WITHOUT the variable: DenoiseImage's
  three panel descriptions lose the 2026-11-08 runs' nodes and now differ from b768293's only by
  the two wordings, checked case by case; Atropos' one reason), content/evidence/ants-programs.md
  (the option sentence the other pages use; the lede's five cases). Check against the old page
  (site 6660758): scratch tools/r15/itk-hdf5.tools-before.md. Screenshots: scratch shots-s4b/
  (itk-*, the agreement, tables and random testing by selector; r12-programs-*).

- **R (relay 16): the renames, with the reviewer's round 13 on S4b applied (its 3 minor).**
  Repository 4d203efd (merge of main), 9e1d1ff7, 36cbb619, bc80cccf, d38c0f74: 26 files renamed
  (`git mv`: the eight clean rooms' findings `docs/findings/*-original.md` -> `-clean-room.md`,
  four tests, eleven validation modules, one test of the evidence), every reference that may
  change following them (both repositories: API pages, findings, the register's three paths,
  docs/findings/README.md, docs/validation/difftest.md, tools.toml, triage.toml's links, the
  suites', evidence's and random testing's registries, tests; validation/investigations/
  docs-revision/renames.py), each record's `.json` kept with its `.md` (7 later), the suites'
  names kept with their records', pointer files at the findings' old names, the modules'
  old import names (tested), the guide's §1 and §7, the inventory made again at its heads
  (only the pair rule's lines changed). Site 3096db2 (round 13: content/evidence/itk-hdf5.md:
  the port's bytes with their condition; "Corrections by default" from docs/api/
  transforms-hdf5.md's D13 table; the random-testing paragraph from the descriptor
  difftest/tools/itk_transforms.py), 2365d5e (content/tools/afni-automask, afni-unifize,
  fsl-flirt, fsl-topup, mri-hmc, tedana-t2smap and content/evidence/fsl-bet: links only;
  content/programs/fsl/bet, bet2, betsurf exported again; static/evidence/difftest, fsl-bet,
  fsl-flirt, fsl-topup, afni-volreg exported again: paths only, and 7 clipped reasons 10
  characters longer). **For the reviewer:** the pointer files and the old import names are this
  relay's decisions (State, relay 16); fsl-fast's JSON was not exported (the box's memory) and
  keeps the old path; the before/after inventory: scratch tools/r16/inventory-r.md.

- **X and S5 (relay 17): the presenter for every software, and AFNI's three pages.**
  Repository `git log d38c0f74..` (present.py: each panel's port from its label, the clean room
  against the replica, the original's other shapes, the version step, the defect, two encodings,
  the guide's mapped role labels, hidden runs by `runs` or the run compared with, panel_shows by
  role for two ports, texts in the port each is about, the charts and cases by port; tests: 31
  in test_evidence_present.py; round 14: renamed.py; evidence/afni_pages.py and the three AFNI
  modules' `present`; tools.toml and programs.toml: the three addresses; the gate). Site
  e71cc6c, ca6c840, 6859f6f: content/evidence/afni-tshift.md, afni-automask.md,
  afni-unifize.md (new), content/tools/afni-*.md (redirects), static/evidence/afni-*.json
  (exported again), summary.json, site.yml, data/status.yml, data/programs.yml,
  content/programs/fsl/bet*.md (hash only). Old pages: scratch tools/r17/old-pages/.
  Screenshots: shots-s5/. **For the reviewer:** (1) 3dTshift's JSON reads its replica's record
  of 2026-11-04 (315 -> 317 cases: State, relay 17); (2) 3dUnifize's page said 117 and 116
  cases and "37 of the 92" since its records were made again on 2026-10-30 (129 and 127; 44 of
  99): the page now says the records' numbers; (3) two sentences of the exports said the clean
  room is compared with AFNI 25.2.09 "until it follows 26.3.05": both clean rooms' records name
  AFNI_26.3.05 as their reference (docs/validation/afni-*.md, "Reference"), so the pages say so
  (afni_pages.TEXTS); (4) on these pages "the original" in the old texts is the clean room
  (TEXTS), and each case result's reason now names its own port (a clean room's message had
  been named the replica's before the fix: 3dTshift's case 230); (5) the "What this shows"
  lines are hand-written from each example's verdicts and reasons (afni_*.py SHOWS), the data
  panel's from present.panel_shows.

- **S6 and round 15's fixes (relay 18): 3dvolreg's evidence page.** Round 15's six findings:
  repository 5c63398f, site 5f00f86 (Review answers, Round 15). S6: repository ec3ccae3 (present in
  afni_volreg.py, the replica's module, restricted for AFNI clean-room implementers, called last by
  afni_volreg_clean_room.export as a black box; present.lines; larmorx `<commit>` kept),
  8a89249f (the address in tools.toml and programs.toml; the cache's words), 9ffe62c1 (test, 32
  in test_evidence_present.py), f124b158 (the replica's rows first). Site 7488ffd: content/evidence/afni-volreg.md (new, the guide's words and bet's layout: lede, facts
  with both ports, What is compared, Its two ports, every section kept with its anchor, the
  corrections as a list, History), content/tools/afni-volreg.md (a redirect), the JSON exported
  again from the cache (texts, ports, order, marks and presentation fields; the high-motion
  example's two text-output rows swapped: replica first), site.yml, data/status.yml and
  data/programs.yml (generated), content/programs/fsl/bet*.md (hash only), content/roadmap.md (two
  links), tools/evidence.py (a line chart on a presented page: its title with code spans, class
  ev-ported), static/css/evidence.css and static/js/evidence.js (the line charts' marks by port:
  the original solid, the replica long dashes, the clean room short dashes, `reproduce` thinner, a
  limit's band a wash of the original's ink). Old page: scratch tools/r18/old-pages/.
  Screenshots: shots-s6/. **For the reviewer:** (1) the line charts (5) and their tables are new
  presented forms (guide §5: 3dvolreg's 13 replica series were marked as the apricot); (2) the
  replica's informal-timing chart keeps its open triangle for `python -m larmorx` (same thread
  count; the key names it), as the old chart had its ring; (3) the AFNI-99 example's metrics note
  said its rows of AFNI against itself were "its previous release and fMRIPrep's build" (the first
  example's note, shared): now "the same volumes stored another way", its one such row; (4)
  afni_pages.TEXTS turned "the cache of the original's runs" (the guide's own words, after a
  second rewording) into the clean room's: fixed (only 3dvolreg's high-motion example met it);
  (5) the lede's counts are the records' (137 of 190; 185 of 251), the speed sentence the
  benchmark table's (1-thread 2.8-5.2, 14 threads 9-15, the replica 1.6-3); (6) History's dates
  are the records' Generated lines, the informal timings' and the benchmark's.

- **S7 (relay 19): FLIRT's evidence page.** Repository e877e0d2 (fsl_flirt.py's `present`; the new
  evidence/fsl_pages.py, the FSL pages' shared presentation for S8-S11: the replica's package
  "larmorx-nc, FSL Licence" as on bet's page, the old texts' "original" the clean room, the modes,
  a clean room's panel naming its licence once, "A clean-room implementation, so not bit-identical
  by design" said as it is, the catch-up's relays as its steps; a test, 33 in
  test_evidence_present.py), d45696f6 (the address in tools.toml and programs.toml), 8cbaf253
  (the speed note's shared machine); round 16's hardening ad37ad48. Site 55e2abe, 8a28495: content/evidence/fsl-flirt.md (new:
  the guide's words and bet's layout: lede, facts with both ports, What is compared, Its two
  ports, the replica's verdicts before the clean room's, the cases covered and the corrections as
  lists, every section kept with its anchor, History), content/tools/fsl-flirt.md (a redirect),
  the JSON exported again from the cache (texts, ports, order, marks and presentation fields
  only), site.yml (the entry), content/roadmap.md (FLIRT's five links), data/status.yml and
  data/programs.yml (generated), content/programs/fsl/bet*.md (hash only). Old page: scratch
  tools/r19/before/ (the JSON) and the site at 7488ffd (content/tools/fsl-flirt.md). Screenshots:
  shots-s7/. **For the reviewer:** (1) **numbers the old text had from before the catch-up of
  2026-11-05, now the JSON's and the records'**: the first example's paragraph said 0.0065 mm and
  "42 % (1 − NCC 2 × 10⁻⁷)", the JSON (its clean room's runs made again on 2026-11-05) says
  0.0113 mm, 42.2 % and 2.6 × 10⁻⁶, as the old verdicts paragraph did; Known limits said the
  clean room "misses its record's thresholds in 5 cases", its record (2026-11-07) says 1; Known
  limits listed `standard_space_roi` among the scripts not ported, validation/tools.toml says the
  replica's since 2026-11-04 and the clean room's since 2026-11-05, its cases in BET's records;
  the C library paragraph's "0.0065, 0.0002 and under 0.00001 mm now" is "then" (ds005040's
  0.0113 mm since the catch-up). (2) The agreement charts and the random-testing title name
  "FSL 6.0.7.23's FLIRT programs" (Page.part): the records cover all 14 programs; the examples,
  tables and speed chart name FSL `flirt` 6.0.7.23. (3) The dates moved to History (the replica's
  cases of 2026-11-02 and 2026-11-03, the clean room's parts, 2026-11-04's moves, the catch-up,
  2026-11-06 and 07). (4) "trial" said "random-testing run" everywhere on the page, its
  duration given (31 minutes, from the table's row); "relay 10/4/5" as the catch-up's steps;
  "another agent's runs" as "other runs". (5) The lede counts: the replica 211 of 211 compared
  (305 cases), the clean room 239 of 240 (339), 209 bit-identical, 0.0113 mm, 12 to 14 times.

- **S8 and round 17's fixes (relay 20): FAST's evidence page; 3dTshift's clean record.** First: main
  (3bc71341) merged (2e1723f4) and 3dTshift's JSON exported again from the replica's clean record (site
  5661100: only the two record files' stamp 629e357ab4aa, date 2026-11-13 and SHA-256 changed; the page says
  the replica's record has had the glibc cases since 2026-11-04, History adds 2026-11-13). S8: repository
  428d66cc and 6bbbd86e (fsl_fast.py's `present`: SHOWS from each example's verdicts, reasons and metrics,
  raising where a verdict is not the sentence's; the files' and volumes' tables by port and titled; the clean
  room's random-testing run of 30 minutes; speed labels and caption; categories; FAST's TEXTS), 8f955872
  (the address). Site 67e196d: content/evidence/fsl-fast.md (new: lede, facts with both ports, What is
  compared, Its two ports, the C library and the catch-up, the replica's verdicts first, the cases covered,
  the clean room's verdicts and random testing as lists, every section with its anchor, Corrections as a
  list, History), content/tools/fsl-fast.md (a redirect), the JSON exported again (16,730 numbers
  unchanged; texts, ports, order, marks, presentation fields, the renamed findings' path), site.yml, the
  roadmap's four links, data/status.yml and data/programs.yml. Round 17 (all nine): Review answers, Round 17.
  Old page: tools/r20/fsl-fast.tools-before.md and tools/r20/before/fsl-fast.json. Screenshots: shots-s8/.
  **For the reviewer:** (1) **sentences the old page had from before the clean-room catch-up of
  2026-11-05, now the JSON's**: the first example's "Its default mode gives what FSL computes with
  correctly rounded maths, bit for bit ..., so its two modes differ by exactly the yardstick row" and "The
  clean-room original's default still computes FAST's variance; it follows in its next part" (the JSON:
  `ofix-cr` is d13, FSL-202, 52 segmentation values from FSL with correctly rounded maths; `ofix-rfix`
  within FSL's band, 27, 37 and 10 partial-volume values): now "Until the catch-up ... Since 2026-11-05 it
  makes the replica's correction too"; the bias field's "and FSL's with correctly rounded maths bit for bit
  in its default" and the data panel's "its default what FSL computes with correctly rounded maths, bit for
  bit" alike (their `ofix-cr` d13). (2) "the four cases random testing gave each implementation are its own"
  was three of the replica's and five of the clean room's (the JSON's `difftest` cases): the page says so.
  (3) The catch-up's six "several channels" cases: the triage table's 4 open and 2 explained before the
  catch-up (the old text called all six open). (4) New facts, each from the page's JSON or readable files:
  the priors' two defaults equal (`ofix-rfix` bit-identical), the replica's date 2026-11-02 and the clean
  room's first random-testing run's 2026-11-03 (the site's roadmap; docs/findings/fsl-fast-clean-room.md's
  heading). (5) The lede counts: 59 of 59 compared, out of 87; 64 of 65, out of 102, 54 bit-identical; 9 to
  11 and 47 to 67 times (the benchmark rows: 8.99, 9.05, 11.4; 56, 46.6, 67).

- **S9 (relay 21): topup's and applytopup's evidence page.** Repository feeabaaf (fsl_topup.py's
  `present`: SHOWS from each example's verdicts, reasons, metrics and movement table, raising where a
  verdict is not the sentence's; applytopup's three examples on their own `Page("applytopup")`, the
  agreement, speed and random-testing charts naming both programs; the files', thread band's,
  metrics' and movements' tables by port and titled; the summaries' clean-room sentence said from its
  own `fix − reproduce`; the three runs with lengths and dates; categories; topup's TEXTS;
  fsl_pages.present's `examples=` and `speed_page=`, both optional: the regression gate on every
  presented page 0 differences), 78a20a42 (the address), 5c59a968 (the suite's module as code),
  633582b2 (dirty_provenance.py), 37f93ded (a test); the reviewer's round 18 on S8: 1178e9f1, site
  2fa5b60 and a1c1d73 (topup's notes too). Site f99fcbe, 9da64d6: content/evidence/fsl-topup.md (new: lede,
  facts with both ports, What is compared, Its two ports, the C library and the catch-up as lists,
  the replica's verdicts first, the cases covered, the clean room's verdicts as a list, every
  section with its anchor, Random testing's three runs as lists, Corrections as lists, History),
  content/tools/fsl-topup.md (a redirect), the JSON exported again (every number unchanged; texts,
  ports, order, marks, presentation fields; the provenance lines of entries whose runs were made
  again), site.yml, the roadmap's five links, data/status.yml and data/programs.yml, bet's
  reference pages (hash only). Old page: tools/r21/fsl-topup.tools-before.md and
  tools/r21/before/fsl-topup.json. Screenshots: shots-s9/. **For the reviewer:** (1) **a sentence of
  the JSON that was wrong, now said from its own data**: each summary of the record's cases ended
  "LARMORX's clean-room original runs the same call ... and its default mode, which makes the same
  correction" whatever its default did; on `lsr` the clean room's `fix − reproduce` is
  bit-identical (no correction applies there: its reason says so), and on `vb3d` its default names
  FSL-143 only where the replica's names FSL-132 and FSL-143. Now: lsr "on this call the two give
  the same values", jac and membrane "which corrects ... as the replica's does", vb3d "which corrects
  FSL-143"; the page's lsr paragraph adds the same fact. (2) Numbers the old page had from before
  the records of 2026-11-07: "the nine cases random testing gave the clean-room original" is ten
  (the JSON's `difftest` category, 10 cases; `difftest/time-step-zero-series` came on 2026-11-07);
  Known limits listed the field image's Jacobian scales (§7.3) as a question left by the catch-up's
  run, which the page's own verdicts paragraph says was answered on 2026-11-07 (FSL-273): the item
  now says so. (3) "half an hour" of the replica's random-testing run is its table's 27 minutes.
  (4) New facts, each from the page's JSON: the clean room's default makes FSL-117's and FSL-201's
  corrections on the membrane case, FSL-132's on jac, FSL-143's on vb3D (its ofix reasons); the
  categories' names. (5) The lede counts: the replica 107 of 107 compared, out of 177; the clean
  room 106 of 106 compared, out of 187, 14 expected divergences; 10 to 12 and about 40 times (the
  benchmark rows: 11.5, 10.0; 40.8, 39.1). (6) The three random-testing tables' titles name their
  runs' dates and counts, the two triage tables their runs' dates (they had one title).

- **S10 (relay 22): fslmaths' and fslstats' evidence pages, with the reviewer's round 19 on S9 applied
  (its 3 minors).** Repository a47898af (fsl_maths.py's `present`: SHOWS from each example's verdicts,
  reasons and tables, raising where a verdict or a table's cell is not the sentence's; the FFT call's
  numbers from its reason; the tables' titles by the guide's pattern and their columns by port; the two
  random-testing runs (both programs at once) with their lengths and dates; the benchmark report's steps
  by their dates (its relay 5: 2026-11-03; relays 7 and 7b: 2026-11-04; relay 6's kernel correction by
  findings 23); the case categories; fslmaths' TEXTS before the FSL pages', shared with fsl_stats.py's
  `present`; fsl_pages.replica_threshold by a regex: "every file fslmaths writes"), 23912d04 (the two
  addresses), 8dffe711 (a test, 39 in test_evidence_present.py), b986d806 (the cases' commands: "a
  synthetic input of the clean room's suite"; the glibc cells: "glibc's bits in `reproduce`"), 629ebc2e
  (the clean room's glibc note: "Since 2026-11-04 its `reproduce` gives"). Site dd06805, b5b5bba, 9a98cec: content/evidence/fsl-maths.md and fsl-stats.md (new: lede, facts with both ports,
  What is compared, Its two ports, the C library as a list, the replica's verdicts first, the cases
  covered, the clean room's verdicts as a list, every section with its anchor, Corrections as lists,
  History), content/tools/fsl-maths.md and fsl-stats.md (redirects), the JSONs exported again (every
  number unchanged; texts, ports, order, shows and presentation fields), site.yml, the roadmap's four
  links, data/status.yml and data/programs.yml, bet's reference pages (hash only); round 19: site
  a6ca1f7 (topup's page). Old pages: tools/r22/fsl-maths.tools-before.md, fsl-stats.tools-before.md and
  tools/r22/before/*.json. Screenshots: shots-s10/. **For the reviewer:** (1) **sentences the old
  fslmaths page had from before 2026-11-04, now its own other sections' and the JSON's**: Known limits
  said the clean room's "reproduce mode does not give glibc's exp, log and pow bits yet" and that
  glibc's double `sin`, `cos` and the FFT's `sincos` "are correctly rounded in both modes: their glibc
  bits wait for a clean-room implementation" (the same page's C library paragraph and GLIBC-07 section,
  and both glibc tables: `reproduce` 0 values differing for sin, cos, exp and log in both ports, say
  they are glibc's bits since 2026-11-04): the item now says so, `tan`, `asin`, `acos` and `atan`
  correctly rounded in both ports; Corrections' "Its double-precision functions are correctly rounded
  in both modes" is "by default". (2) The lede's counts are the records' (fsl-maths-replica: 422 cases,
  360 pass, 61 both, 1 expected; fsl-maths: 454, 386 pass, 63, 5; fsl-stats-replica 208, 172, 36;
  fsl-stats 162, 126, 36), "370 of them FSL's values in every file" the JSON's verdicts; the clean
  room's verdict list counts the JSON's 6 expected divergences as the record's 5 plus `fn/double-tan`
  (the record holds it within one unit per function; the JSON's verdict is GLIBC-07's divergence).
  (3) The clean room's random-testing tables' notes say "fslmaths and fslstats at once" (the run's
  "both programs at once"), its length from its row (19.8 → 20 minutes, the old page's 20); the
  replica's run 24 minutes (its row). (4) The benchmark rows' "relay 7b's double sum", "relay 6's
  kernel correction", "relay 7's check ... at relay 5" by their dates and finding, from the benchmark
  report (docs/benchmarks/fsl-maths.md: "2026-11-03 ... relay 5", "relay 7, 2026-11-04", "Since relay 7b
  (2026-11-04)", "(findings 23). Corrected in relay 6"). (5) fslstats' clean room's record "the
  replica suite's cases but FSL's own tests, BET's call among them" (BET's call is in the replica's
  suite: 208 − 56 FSL tests − 3 random-testing cases + 9 + 4 = 162). (6) The triage's FSL-75 row's
  "(rule A)" is "correcting it by default"; "the user's decision of" is "as decided on".

- **S11 (relay 23): prelude's and the head-motion correction's evidence pages, with the reviewer's round 20
  on S10 applied (its 3 minors).** Repository 3874a564 (main 5c5383ca merged), 8e47a17e (fsl_prelude.py's
  `present`: SHOWS of its ten examples from their verdicts, reasons and facts, raising where one is not the
  sentence's; FSL-272's two corrections against FSL's run without the NaN by their role, "the replica (the
  clean room) against the original: fix, and FSL without the NaN"; every metrics note gives the replica's
  record's threshold, then the clean room's record's from its Thresholds table (both_thresholds); the
  tables' titles by the guide's pattern and their columns by port; the two random-testing runs with their
  lengths and dates; the case categories; fsl_prelude_clean_room.export ends with `present`, its failure
  reported without the replica module's text, as 3dvolreg's), 852d2bda (mri_hmc.py's `present`: What this
  shows from the example's verdicts and reasons; the labels that named mcflirt without its software and
  version; the motion chart's keys, series and panels by port; **the comparisons' table under it renamed
  and its 17 rows moved into the guide's order** (the original against itself, fMRIPrep's output, the
  replica, the clean room; each group's first row separated), its note naming the replica's records'
  threshold beside the clean room's; three agreement charts by their records' versions; the speed chart
  against the benchmark report's "FSL 6.0.7"; the long random-testing run of 2026-10-25 against FSL
  6.0.7.17; the catch-up's run with its length and date; the categories), 8b5b7cdb (the addresses),
  380fcfb5 (three tests, 42 in test_evidence_present.py; round 20's minor 2 in fsl_maths.py), 4815a494
  (prelude's synthetic example "generated by the comparison suite"), 2b914894 (the motion chart's legend
  in the guide's order). Site 460def5 (the two pages,
  redirects, site.yml, the roadmap's four links and FLIRT's one to /evidence/mri-hmc/, generated data,
  the JSONs exported), ef89f1f (round 20 on fslmaths' and fslstats' pages; fslmaths' and prelude's JSONs
  exported again), 8b97b25 (mri-hmc's JSON again: the legend). Old pages: tools/r23/fsl-prelude.tools-before.md, mri-hmc.tools-before.md and
  tools/r23/before/*.json. Screenshots: shots-s11/. **For the reviewer:** (1) **facts the old pages' prose
  had stale, now the records' the JSONs cite** (the charts already showed them): prelude's clean room in
  `fix`, "248 of 248" files the replica's -> 252 of 252 (record of 2026-11-11; 248 kept in History); the
  mcflirt replica's standard comparison "181 of 181 ... 22 where both fail" -> 205 of 205, 32 (record of
  2026-11-02: the 33 failure cases of specs/mcflirt.md §14.1 since 2026-10-30, edge/fsl75-narrow); the
  clean room's "One case misses its threshold: real/ds003345/default ... r = 0.9885" -> none since the
  record of 2026-11-07 (FD r 0.9907, the case's reason; the old fact kept under What is open); "for it,
  FSL-20 and FSL-21 are still to be specified and corrected" -> handled since 2026-10-30 (the roadmap's
  row; the record's FSL-20, -21, -65, -66 entries); the clean room's corrections list from its record's
  fix pass (FSL-07, -20, -21, -65, -66 added to the old list); syn/motion/small/no-fsloutputtype rejected
  by both since 2026-11-11 (it was an expected divergence: the site's JSON before cffa3c3). (2) mri-hmc's
  lede counts: compared = cases neither refuses (the guide): the replica 205 of 237 and 173 of 194, the
  clean room 145 of 149 (170 - 21), "the other 4 expected divergences" (the record's 4: two synthetic runs,
  two random-testing phantoms); the speed factors from the chart's rows (1.15 to 2.31 on one thread,
  4.94 to 9.42 on 12). (3) **The reorder of mri-hmc's comparisons' table** moves its numbers' pointers:
  the inventory lists 67 shared shown numbers "changed"; tools/r23/shown_rows.py proves every one moved
  with its row (102 numbers of the rows moved and equal, 140 other shared equal, 0 differ), and
  tools/r23/stats_check.py the JSON's cells (17 rows, 0 differ). (4) The random-testing panel of mri-hmc
  still shows difftest.json's label "head-motion correction, clean-room original" and an explanation's
  "larmorx over mcflirt": chunk E's (difftest.py's PAGES labels and the table made again), not touched
  here. (5) prelude's lede says 67 of 68 and 134 of 138 compared cases with the FSL-263/265 cases as
  expected divergences (the JSON's verdicts), where the generated index counts them as passing (138 of
  138, 68 of 68): the page says what the comparison of those cases leaves out.

- **S12 and chunk T (relay 24): t2smap's evidence page, and the clean room in its three worked examples
  that showed tedana and the replica only (the revision's one computation), with the reviewer's round 21 on
  S11 applied (its 2 minors).** Repository 0446079a (main e36d1707 merged), 17c4d489 (chunk T in
  tedana_t2smap.py: the clean room's panels `clean-reproduce` (reproduce, 1 thread) and `clean` (fix, 8
  threads), its run `clean-1` (fix, 1 thread), five comparisons per example: `cr-ted` and `crfix-rfix` by
  specs/t2smap.md §17.3's check.py (the clean room's suite, as its own example judges them), `cr1-cr` threads,
  `crfix-cr` fix against reproduce with the notices it prints, `crfix-fmriprep`; ds003592's runs are the clean
  room's own example's entry (the same call: its echoes' and mask's SHA-256 checked, tedana's files byte for
  byte the same), ds000258's a new cache entry `tedana-t2smap/clean-room-ds000258-sub-21262-rest-curvefit`,
  **computed at the clean commit 17c4d489** with the wheel built from 0446079a (scratch build24/; 3 runs,
  9.5 s); tedana's stored files checked with the empty `figures` folder tedana writes, which the call's entry
  leaves out (_check_tedana); the default view adds the clean room's panel, its diffs unchanged so no poster
  moves), 921d47bb, 00eae8c8, 06fb0cab (t2smap's `present`: What this shows of the four examples from their
  verdicts and reasons, raising where one is not what it says; fMRIPrep's output a port of its own where it
  is no panel; tedana's version step named as one; the replica's record's threshold beside the clean room's
  in every metrics note; the charts and the twelve tables titled by the guide's pattern, columns by port; the
  clean room's texts that call it "the original"; categories), 90b45755 (two tests, 44 in
  test_evidence_present.py). Site f284bf8 (round 21), 2b2f380, 0f321cf: content/evidence/tedana-t2smap.md
  (new, the guide's words and bet's layout, every section and anchor kept, History), content/tools/
  tedana-t2smap.md (redirect), site.yml, the roadmap's two links, data/status.yml and data/programs.yml
  (generated; bet's reference pages: tools.toml's hash only), static/evidence/tedana-t2smap.json (exported
  from the cache at 06fb0cab) and blobs.sha256 (1,436 -> 1,451). Old page: tools/r24/
  tedana-t2smap.tools-before.md; old JSON tools/r24/before/. Screenshots: shots-s12/.
  **For the reviewer:** (1) **the new numbers** (chunk T): each maps example 522 numbers more, the series
  example 108; the clean room's check.py counts equal its record's fMRIPrep cases (ds003592-curvefit (A)
  11,381,033/11,406,278 0.305 %, (B) 11,380,746/11,406,278 0.259 %; ds000258-curvefit (A)
  7,311,090/7,342,822 0.124 %, (B) 7,312,250/7,342,822 0.076 %); 0 old numbers changed or lost
  (tools/r24/num_check.py, matched by ids; 15 re-keyed by their reworded row heads only). (2) **15 new
  blobs** (0.03 MB: the clean room's panels stored as xor against tedana's, and the five new posters),
  every old blob kept byte for byte (tools/r24/blobs_check.py). (3) The lede's counts: the replica 62 of 62
  compared, out of 102; the clean room 88 of 89 (65 + 23; 125 - 36 refused by both), the one divergence.
  (4) The old page's "--implementation" sentences now say "from the next release" (t2smap's clean room comes
  with it; 0.1.0rc1 has the replica only). (5) The provenance keeps the cache entries' version strings as
  quoted ("larmorx's reference", "the clean-room part's branch"), as on every page.

- **S13 (relay 25): sdcflows' two evidence pages, with the reviewer's round 22 on S12 and chunk T applied (its
  important finding and 2 minors; Review answers, Round 22).** Repository fc29ff70, 2116d866 (round 22), 2ca3c0b3,
  6bb47904, 10a2c416, f9c39cc3 (and the S13 commits named in State, relay 25): sdcflows_fieldmap.py's and
  sdcflows_estimation.py's `present` (What this shows of the 11 examples from their verdicts and reasons, raising
  where one is not what it says; the record's threshold in each metrics note; the tables, columns, speed charts,
  agreement charts and the records' categories by the guide's pattern; LARMORX named as the replica where the old
  texts used it for the port, kept for the project's replicas underneath and its reference setup; the rows a reason
  points at named by what they compare); present.py: `known` (a known answer's panel, sdcflows' exact field: the
  input's grey, compared as "the known answer"), `split` (each port's comparisons in one group per target, so the
  metrics tables keep their old group rows: the inventory's rows after >= before), a comparison the page names
  itself is not read by its shape, a page's two ports counted from LARMORX's panels only; tests (48 in
  test_evidence_present.py); tools.toml and programs.toml: the two addresses. Site 1711d35, b46854b: content/evidence/sdcflows-fieldmap.md and sdcflows-estimation.md (new, the guide's words and bet's layout:
  Ports, Status (validated before the definition), Records, Reference; What is compared, Its port, the verdicts in
  lists, every section and anchor kept, History), content/tools/sdcflows-*.md (redirects), site.yml's two entries,
  the roadmap's two links, data/status.yml and data/programs.yml (generated; bet's reference pages: tools.toml's
  hash only), the two JSON files exported from the cache (texts, ports, order and shows only; every blob kept).
  Old pages: tools/r25/sdcflows-*.tools-before.md; old JSON tools/r25/before/. Screenshots: shots-s13/.
  **For the reviewer:** (1) the SyN example's original is ANTs' registration (`antsRegistration` 2.6.5, sdcflows'
  steps after it), named so in its panels and comparisons; its four comparisons of ANTs with itself and fMRIPrep's
  build are named by the page. (2) The exact field (SDC-10) is the known answer, the input's grey, as the guide's
  section 5 says. (3) The estimation page's "19 reach fMRIPrep's own calls (the table's fourth column)" says
  "fifth": the table's columns are Entry, Original, Class, What the original does, fMRIPrep's calls. (4) Both pages
  keep their records' status, validated before the definition (VALIDATED_BEFORE lists all seven records). (5) The
  case category `relay22/audit_contract` (a script's folder) is shown as "the per-program contract"; its case's
  description says "the contract of every program" (was "the part's contract").

- **S14 (relay 26): fMRIPrep's one-shot resampler's and NIfTI I/O's evidence pages; site.yml's evidence entries
  and summary.json in the new words.** Repository a01a4523, 76a7f303, f1d22bff: resample_series.py's `present`
  (What this shows of its two examples from their verdicts and reasons, raising where one is not what it says;
  fMRIPrep's resampler run again here, role `fmriprep`, is the original "fMRIPrep ResampleSeries 26.0.0.dev1" as
  the record's Reference line says; the replica's reproduce run of PIPE-03, role `fmriprep`, is the replica's,
  named "fMRIPrep's factor"; PIPE-03's factor on the BOLD's own grid is "a reference computed another way", the
  input's grey; the record's threshold in the native example's note; the categories named; random testing
  named), nifti_io.py's `present` (LARMORX's own reader and writer against nibabel 5.4.2: the agreement, the two
  tables' titles and columns, the cases' texts with a bare "larmorx" as LARMORX, the speed chart's marks `own`
  in the clean room's colour, random testing; "parity" in a case's text reworded), present.example's
  `panel_ports` (a panel's port where its role says another) and the group "against a reference computed another
  way", summary.py (every page at /evidence/, the ports named replica, clean room, "replica (larmorx-nc)",
  LARMORX's own; the software of the four pages that had LARMORX's group; the note), tools.toml and programs.toml
  (the two addresses), 2 tests. Site 45ca4a6, fda17d6, 19fffb6 (the JSON at f1d22bff): content/evidence/
  resample-series.md and nifti-io.md (new, the guide's words and bet's layout, every section and anchor kept,
  History), content/tools/{resample-series,nifti-io}.md (redirects), the JSON exported again (texts, ports,
  order, shows, presentation only; every blob kept, blobs.sha256 unchanged), summary.json made again (texts only),
  site.yml's `tools:` all in the new words, tools/evidence.py (the matrix's `moved` mapping gone; PORT_LABELS
  `own`), evidence.css and evidence.js (`own` = the clean room's colour and circle), the sdcflows pages' links,
  data/status.yml and data/programs.yml (generated). Old pages: tools/r26/*.tools-before.md; old JSON
  tools/r26/before/. Screenshots: shots-s14/ (taken before the last export: PIPE-03's reproduce panel was then
  named "replica", now "fMRIPrep's factor"). **For the reviewer:** (1) **a fact the old page had loose**: it said
  "validated against fMRIPrep 25.2.6"; the record's original is fMRIPrep 26.0.0.dev1 at commit 21a490fb (its
  Reference line, the benchmark's packages), the worked example's own output fMRIPrep 25.2.6's run: the titles name
  26.0.0.dev1. (2) The old Known limits "the field-map estimation itself (sdcflows) is planned" was stale: sdcflows
  is ported (its two pages linked). (3) SCI-02 added to Corrections from the register's row and the record's fix
  pass (10 cases). (4) NIfTI I/O's lede counts: 166 + 69 + 5 = 240 compared of 257 (17 refused by both); the 69
  differ only in the matrices, at most 4.2 × 10⁻¹³ mm (the largest of their reasons). (5) SimpleITK keeps its open
  square in NIfTI I/O's speed chart (the caption says what it is). (6) Validation's matrix header still says
  "Tool"/"Implementation" and its rows keep their dots and order: P2's.
- **P1 (relay 26): the header, Getting started, Concepts, Why Rust and Python under Concepts, About, the home page's
  links.** Site 19fffb6, 107114e: site.yml (`nav:` Getting started, Concepts, Programs, Validation, Roadmap, About;
  `docs:` the sidebar's and footer's list), content/getting-started.md (git mv of installation.md; the new words:
  programs, "Port" for the setting with `clean-room` from the next release, the clean room; its packages a short
  list with its links kept, in full on Concepts; every anchor kept), content/installation.md (a redirect),
  content/concepts/index.md (new: Four words with LARMORX's own and where each port lives; The three packages and
  their licences in full, moved from installation; Three settings: `auto`, `fix`/`reproduce` with the ANTS-40
  notice, threads; Why clean rooms exist and the clean-room audit; Why Rust and Python), content/concepts/
  why-rust-and-python.md (git mv, the new words: programs, projects, the cross-platform checks),
  content/why-rust-and-python.md (a redirect), content/about.md (new: the licence, citing from CITATION.cff, the
  two repositories, credits and notices from the footer, a link to the name and FAIR on Home), content/index.md (its
  links to Getting started, Concepts and Why Rust and Python; the closing buttons), content/roadmap.md and
  evidence/mri-hmc.md (links). Inventory (tools/r26/inventory-p1.md, against b46854b built, the moved pages
  paired): every content page after >= before but words; the header's 8 links -> 6 (shown twice, desktop and menu)
  and the footer's list +2 leave redirect pages 2 links fewer (41 -> 39), content pages as many or more; new
  pages Concepts (1,586 words) and About (646). Screenshots: shots-p1/. **For the reviewer:** Concepts' audit
  paragraph and About's citation are new facts from readable files (docs/validation/clean-room-audit.md,
  CITATION.cff); About names the author as CITATION.cff does (no e-mail).

- **Round 23's fixes (relay 26):** repository 8be96831 (sdcflows_estimation.py: `ORIGINALS` with niworkflows 1.15.3
  (1.15.4's code on these paths); the contract case's description and category), site 50a13a0 (both sdcflows pages'
  Corrections sentence; the estimation JSON exported again: 8 texts, nothing else). Screenshot:
  shots-s14/r23-est-agreement-1440-light-y0.png; the resampler's PIPE-03 example after its last export:
  shots-s14/rs-pipe03-final-*.

- **P2 (relay 27): Validation reorganised, the data panel under it, the roadmap's words with the confounds' site
  handoff, 404, the site's README.md and CLAUDE.md.** Repository d7579286 (validation/programs.toml: the confounds'
  group summary and description in their new state). Site ce1205f, 9700380 (and f9de904's one word): content/
  validation.md (How LARMORX tests: Every port runs beside its original, Two kinds of port (example 11, the AFNI move
  and mcflirt's `mri hmc` kept), `fix` and `reproduce`, Corrections checked, Three sizes of comparison (example 12),
  the data panel, the commands with their sentence, the cross-platform checks; Levels of testing and Now/before as
  h3 under it in the new words; How to read the evidence with three new bullets (What this shows, Titles, Colours:
  from the guide's sections 4 and 5); The verdicts; Where every program stands with the matrix grouped by software
  (pastel chips, Programs page's order, "Program"/"Port", no dots, links in the accent); #statuses and
  #validated-before as before; Random testing; The records; The data panel (new short section); every old anchor
  kept), content/validation/data-panel.md (git mv from content/data-panel.md, words: program, port, evidence page;
  its #panel link to /evidence/afni-automask/), content/data-panel.md (a redirect), the eight evidence pages' links
  to the data panel, site.yml (`docs:`), content/roadmap.md (every "clean-room original", "tool", "oracle runs",
  "reproduce mode", "the main package", "LARMORX itself", "exact replica", "golden tests", "parity" in the new
  words; the confounds' HANDOFF-site.toml: Last updated 2026-11-13, Confounds out of Under way and the rows
  renumbered, the Completed row of 2026-11-13 as the handoff wrote it; example 17's row; /tools/fsl-bet/ links ->
  /evidence/; "all four AFNI originals" -> "clean rooms" (docs/next-steps.md: the four AFNI clean rooms' long
  names)), data/status.yml (the planned confounds entry: "Confounds, their pages", ported, its pages and its line
  next: the handoff's status op in the new structure; surfaces "programs"), content/programs/index.md (the confounds
  sentence), data/programs.yml (generated), content/404.md (the Programs link, the letter-case sentence), README.md
  and CLAUDE.md (the new words, the new folders and helpers, eight pastels + LARMORX's, copyable blocks without
  comments), tools/evidence.py `matrix`, static/css/evidence.css. Old pages: tools/r27/*.before.md. **For the
  reviewer:** (1) the planned list keeps a "Confounds, their pages" card until step 2 (the confounds' tools.toml row
  is on main, not in this branch, so the Ported table does not show them yet); (2) roadmap row 1 FreeSurfer stays
  "Planned" as the handoff left it; (3) Validation's new Colours bullet states the guide's section 5 token set.
- **E (relay 27): random testing brought up to date.** Repository 5d784ef9 (evidence/difftest.py: PAGES at /evidence/,
  labels "mcflirt" and "head-motion correction (larmorx mri hmc)", `ports` per tool (original -> clean room, port ->
  replica, nifti-io's LARMORX's own), the triage's explanations through words.reword and LARMORX in prose outside
  code spans (`_prose`), `runs`: the 25 runs since the long run as the evidence modules name them (TRIAL, TRIALS,
  TRIAL_CATCHUP), one row per program, port and run, from oracles/difftest/<run>/summary.json; RUN_NOTES; a test in
  test_difftest.py). Site 806fce6: difftest.json exported again (every number of the long run unchanged: 279 of 279;
  25 runs added), the 14 evidence pages' random_testing calls at /evidence/, Validation's long-run table headed
  Program with each implementation by its port, and `random_testing_runs()` (Program, Port, Run with its length and
  note, Random cases, Agree, Notices, Undefined, Differ, Skipped, Unclear; each program linked to its page's random-
  testing section; a note on what Differ counts). **For the reviewer:** the runs' numbers equal the pages' own
  tables (e.g. topup's clean room 14,400: 7,401 / 6,999; BET 909; mcflirt's clean room 83,244: 31,552 / 51,681 / 11);
  runs the pages do not show (3dvolreg's replica 2026-11-06, FAST's replica 2026-11-02, FLIRT's replica 2026-11-01,
  BET's 2026-10-30/31 and 11-04) are not listed: the sentence says "the runs the evidence pages show".
- **Round 25's fixes and M1's rest (relay 28): step 1's merge material.** Repository 8af53a29 (merge_inventory.py),
  73e32390, 77b7842d, b8cddfd5, 08da1e80 (round 25 in the evidence modules: the resampler's poster, the triage's words,
  NIfTI's quick check, the matrix's note, 3dvolreg's triage table); site bd4a694 (A: the roadmap and the home page's
  planned list), d8b706f (B: the check names in words), 6dbb6e8 (the Programs page's anchors with spaces as hyphens),
  737fb9a (the minors C-N and the exports), edede81. Round 25's answers: Review answers, Round 25.
- **M1's machine paths (relay 27).** Repository c184170b (figures/cache.py machine_paths, _short through it; evidence/
  core.py `_clean` applies it to every exported string; validation/tests/test_evidence_machine_paths.py), 2a6b5670
  (validation/investigations/docs-revision/site_paths_check.py, README). Site f9de904: ants-programs, afni-volreg,
  sdcflows-estimation, sdcflows-fieldmap, mri-hmc, tedana-t2smap, fsl-bet, fsl-flirt exported again (tools/r27/
  pathdiff.py: every changed string is machine_paths(old) but ants-programs' 420 commands, whose path becomes
  `<fMRIPrep run>/` (the live site has `<fMRIPrep oracle run>/`), and the exporter's hash; numbers, blobs and
  blobs.sha256 unchanged). site_paths_check.py: 0 on the site (42 before); the 1,454 blobs scanned: none.

## The rollout plan (relay 11, 2026-11-12; the user approved the four pilots)
The order follows the orchestrator's list (FROM THE ORCHESTRATOR, last entry): (2) the site's
pages, software by software; (3) the reference pages; (4) the repository's documents, the
renames, the wrappers' messages; (5) t2smap's three examples and the random-testing table; (6)
programs.py's rule; (7) the clean rooms' printed lines. One chunk = one relay. A relay that
finishes a chunk early starts the next one; a chunk too large is split (say so here).

**Every chunk's checks** (as for the pilots; from relay 14 also: `summary_check.py <site>` 0
differ after every export, making summary.json again with `evidence summary --site <site>
--no-compute` when it differs; every export with `LARMORX_FMRIPREP_RUNS=<scratch>/tools/r14/
panel-runs`): the site's build 0 problems (`--main-repo
<worktree>`); `reference all --site <site> --check` 0 problems, 0 stale; the inventory against
the site before the chunk (a git worktree of the site clone at the chunk's start, built:
everything but words, after >= before; tools/r8/shown_check.py-style comparison for shown
numbers that moved); on an evidence page the number check (`LX_EVIDENCE.check()`) and the
viewers' check (`LX_EVIDENCE.checkViewers()` with every viewer loaded) all agree,
`static/evidence/blobs.sha256` unchanged and every blob byte for byte; screenshots 1440 and 390,
light and dark, of each changed page's main parts (scratch shots-<chunk>/); the repository's
gates for what changed (ruff, format, SPDX; validation/tests in an 8 GiB box, 4 workers; the
evidence words tests). Old words left in a page's visible text: none but quoted record or
version strings (as bet's). Every readable file changed is listed under "Documents a clean-room
implementer may read" with the sources of its facts; every shared document under "Shared
documents changed directly".

**How an evidence page moves** (bet's pattern, relays 4a-10): the evidence module gets a
`present(doc)` at the end of `export` (words.example per worked example with its "What this
shows" line from the example's own reasons and metrics; agreement and speed charts titled by
the guide's pattern; tables and cases through words.table/words.cases; `presentation`); the
JSON is exported again from the cache (tools/r5/reexport.sh in `boxed.sh 8`, into a copy of the
site; blobs.sha256 compared; then the JSON copied); the page's Markdown moves from
content/tools/<page>.md to content/evidence/<page>.md (kicker "Evidence · <software>"), in the
new words and bet's layout (lede; facts line Ports/Status/Records/Reference; "What is compared"
with the old anchor #what-it-is or the page's own; the ports' paragraph; every section kept with
its anchor; dates into a History section); content/tools/<page>.md becomes a redirect page;
site.yml's `tools:` entry gets the new address and its summary in the new words. Reference
pages that do not exist yet are linked as their API page until chunk Q makes them.

### S: the evidence pages, software by software
- **S1 (relay 11, DONE)** the shared presenter for pages whose ports are named in the old forms
  (evidence/present.py: each panel, comparison and chain node named from its shape, the guide's
  role labels, the metrics table's groups, comparisons of runs not shown as panels, the oracle's
  words, the categories' names, the random-testing panel's title; tests in
  validation/tests/test_evidence_present.py); ANTs `N4BiasFieldCorrection` (ants_n4.py,
  content/evidence/ants-n4.md, 8 examples). How a page uses it: "How an ANTs-like page moves"
  under State (relay 11 done).
- **S2** ANTs `DenoiseImage` (ants_denoise.py, content/tools/ants-denoise.md, 6 examples, a band
  and a chain like N4's), `antsApplyTransforms` (ants_apply_transforms.py,
  ants_apply_transforms_bspline.py, 5 examples, two chains).
- **S2 (relay 12, DONE)** `DenoiseImage`, `antsApplyTransforms` (and the reviewer's round 9 on
  S1: ANTS-36 said as it is, N4's lede, the band's software).
- **S3 (relay 13, DONE)** `antsRegistration` (ants_registration.py, ants_panel.py, 5), `Atropos` (ants_atropos.py,
  ants_atropos_pv.py, 5). The reviewer's probe (round 9, finding 2): antsRegistration's 5
  examples pass today's presenter; Atropos refuses `fmriprep-atropos` (its comparison
  "larmorx, seed 121212 − ANTs, the clock's seed (run 1)": present._what needs the shape "a
  fixed seed against the clock's seed", a port against the original, so `_role` must name it
  "the replica against the original: a fixed seed and the clock's" or similar; add it with a
  test).
- **S4** ImageMath and the filters (ants_programs.py, 8), `antsAI` (ants_ai.py,
  ants_ai_blobs.py, 6), ITK transform files (itk_hdf5.py, no example: prose, tables, cases).
  **Split (relay 14):** S4a ImageMath and antsAI DONE; **S4b ITK transform files next**
  (itk_hdf5.py: Page("ITK", "transform files", "5.4.5") or LARMORX's own reader against h5py
  and ITK, as tools.toml says "replica (ITK, HDF5)"; its cases' categories read/catalog,
  read/fmriprep-25.2.6, read/h5py, read/itk, write, hdf5, errors; its three tables; check its
  `read/fmriprep-25.2.6` cases for the same repeated-call descriptions: export with the
  fourteen runs).
  The probe: antsAI refuses `init-aff` (a panel "antsAffineInitializer: larmorx" without its
  mode) and `blobs-found` ("ANTs 2.6.5's blobs", a known answer): add those shapes in S4.
- **X (relay 16 began it: X4's probe and regression gate, validation/investigations/docs-revision/probe_present.py, and example(errors=...); X1-X3 next) (the presenter for every software, before S5; the reviewer's round 9, finding 2)**: one
  extension of present.py, not page by page: a port per panel (the label decides replica or
  clean room: "larmorx replica", "larmorx-nc replica", "larmorx (replica), 1 thread",
  "larmorx replica, -peels 0", "larmorx replica, stored LPS", "LARMORX (port), 1 thread";
  `Page.port` only the default), so two-port pages work; the originals "in fMRIPrep's image"
  ("AFNI 25.2.09 in fMRIPrep's image", "AFNI in fMRIPrep's image") and of another version or
  program ("mcflirt, FSL 6.0.7.17", "AFNI 26.3.05's series, median") as the original with that
  qualifier; explicit kinds for reference, known-answer and input-derived panels ("Initial
  alignment (-init)", "The factor on the BOLD's own grid, carried here", "ANTs 2.6.5's
  blobs"); sdcflows' run workflow with no port; `panel_shows` beyond the ANTs ids (its id
  pattern `\b[A-Z]+-\d+\b`, restricted to the register's ids) and WARNS for other software
  where a correction only warns. Then the reviewer's probe (reviewer-work/probe_present.py)
  into the repository as a gate (a test or validation/investigations/docs-revision/
  probe_present.py) that dry-runs `present` over every evidence JSON of the site and lists the
  refusals; it must skip pages already presented (`"presentation": "writing-guide"`: today
  fsl-bet, ants-n4, ants-denoise, ants-apply-transforms; the reviewer's probe refuses them
  because their labels are already new), or check them through their own module. Each later
  chunk's gate: 0 refused for the page about to be rewritten. Tests for every new shape.
  **Added from the reviewer's round 10 (reviews/round-10-s2.md, "Chunk X's plan"; the
  orchestrator's message):**
  - **X1** each run's port from its panel (or from the `runs` map for a run shown without a
    panel), never from the id or the label's first word: the two-port pages' old ids call the
    clean room "original" (`original-afni`, `original-fsl`, `original1-original`, `ofix-cr`,
    `ofix-original`). `_role` gets the guide's role for comparisons between the two ports
    (afni `original-replica`; fsl `original-replica`, `ofix-rfix`, `orig-replica`): "the clean
    room against the replica", and its `fix` form ("the clean room against the replica: both
    `fix`"). Note: relay 13's rule in `example()` (a run without a panel whose id begins with
    "larmorx" is the page's port) is an id rule: X1 replaces it by the run's port from the
    `runs` map or the panel it is compared with, Atropos' `larmorx-seed` named in ants_atropos'
    `runs`.
  - **X2** version steps beyond ANTs 2.6.2/2.6.5: `_what` reads the two versions from the two
    panels (or labels) and says "the version step, A and B" (AFNI 25.2.09 and 26.3.05's
    `afni26-afni`; tedana 25.0.1 and 26.0.3's `t25-ted`; FSL 6.0.7.17 and 6.0.7.23); kinds for
    "the machine" and a numerical library ("the machine and numpy's kernels": tedana's
    `avx-ted`, `shared-ted`); each with a test.
  - **X3** `panel_shows` by role, not by id (the port against the original, the port against
    itself by threads, `fix` against `reproduce`; the FSL and AFNI ids `replica-fsl`,
    `original-fsl`, `fix-replica`, `ofix-original`, `replica1-replica`, `original1-original`),
    looping over the page's ports: on a two-port page one "What this shows" sentence and one
    verdict chip per port, as on bet's evidence page.
  - **X4** a regression gate beside the probe: for every page already presented, its module's
    `present` run on its JSON before the revision (`git show b768293:static/evidence/<page>.json`,
    present_dry_run.py), compared with the dry run of the published presenter (export_diff.py),
    0 differences or only those listed as intended (relay 13 ran this by hand for N4, DenoiseImage
    and antsApplyTransforms with present.py at 90ee800b and now: tools/r13/gate/); the probe builds
    each `Page` from validation/tools.toml and programs.toml (software, version, ports), not a hand
    map, and collects every refusal of an example, not only the first.
  - **Two smaller points:** `WARNS` keyed by id alone holds only while a warning-only correction
    never changes values on the calls it is used for (ANTS-36's other half, an unreadable mask,
    is an error); today `panel_shows` serves only fMRIPrep's own calls: say so in its docstring
    or key it by id and call shape. Comparison kinds next to the panel kinds: "the defect, in
    AFNI" (afni-automask), "what the registration changed" (FLIRT's `fsl-init`), "the original
    against the truth" (prelude).
- **R (relay 16, DONE: State, relay 16; fsl-fast's JSON export left to S8)** (the renames, before AFNI's pages): the 28 files of the inventory renamed now (git mv;
  every link in both repositories; record ids in validation/tools.toml; `tables`; the comparison
  suites' old names kept as aliases; descriptor NAMEs kept), the 5 "renamed later" left; every
  evidence page whose provenance or text names a renamed file exported again (paths only where
  the page has no presentation yet). Tests renamed with them. Files: inventory.md's "Files named
  with original" table.
- **S5 (relay 17, DONE; round 15 fixed by relay 18)** AFNI `3dTshift` (afni_tshift.py, afni_step.py, 2), `3dAutomask` (afni_automask.py, 4),
  `3dUnifize` (afni_unifize.py, 3).
- **S6 (relay 18, DONE)** AFNI `3dvolreg` (afni_volreg_clean_room.py after R; afni_volreg.py is restricted for
  clean-room implementers: wording only, as in any readable file; 4 examples).
- **S7 (relay 19, DONE)** FSL FLIRT (fsl_flirt.py, 5; the longest prose, 4,975 words; fsl_pages.py for the FSL pages).
- **S8 (relay 20, DONE)** FSL FAST (fsl_fast.py, 6).
- **S9 (relay 21, DONE)** FSL topup and applytopup (fsl_topup.py, 7).
- **S10 (relay 22, DONE)** FSL `fslmaths` (fsl_maths.py, fsl_maths_clean_room.py, 3) and `fslstats` (fsl_stats.py,
  no example).
- **S11 (relay 23, DONE)** FSL prelude (fsl_prelude.py, fsl_prelude_original.py, 10) and head-motion correction
  (mri_hmc.py, content/tools/mri-hmc.md: mcflirt's replica and the clean room `mri hmc`, 1).
- **S12 (relay 24, DONE, with T)** tedana `t2smap` (tedana_t2smap.py restricted: wording only; tedana_t2smap_clean_room.py
  after R; 4 examples), with chunk T if it fits, else T next.
- **S13 (relay 25, DONE)** sdcflows' field-map estimation (sdcflows_fieldmap.py, 6) and PEPOLAR, direct and
  fieldmap-less estimation (sdcflows_estimation.py, 5).
- **S14 (relay 26, DONE; then P1 (relay 26, DONE), P2, E, M1 = step 1 of the publication; Q, D, F, G, confounds, M2 = step 2)** fMRIPrep's one-shot resampler (resample_series.py, 2) and NIfTI I/O (nifti_io.py,
  LARMORX's own against nibabel, no example); the sidebar and site.yml's `tools:` all in the new
  words; summary.json (summary.py) in the new words and with the /evidence/ addresses (then
  tools/evidence.py's `moved` mapping in `matrix`, added in S1, can go).
- **T (relay 24, DONE)** t2smap's clean room on its three worked examples that lack it: the only computation of
  the revision, from the cached oracle outputs through the evidence module (an 8 GiB box, 4
  threads); new blobs added (blobs.sha256 grows, no existing blob changes).

### P: the site's other pages
- **P1 (relay 26, DONE)** Getting started (content/installation.md -> content/getting-started.md, redirect with
  its anchors), Concepts (new: the four words in depth, the packages and licences in full, auto,
  fix and reproduce, threads, why clean rooms exist and the clean-room audit; facts from the home
  page, Validation, installation and docs/licensing.md only), Why Rust and Python under Concepts
  (moved, redirect), About (new: the footer's licence, citing, repositories, credits, notices),
  the header's links (Getting started, Concepts, Programs, Validation, Roadmap, About), site.yml's
  `docs:`, the home page's links to them.
- **P2 (relay 27, DONE)** Validation reorganised (the guide's section 6: how LARMORX tests, how to read the
  evidence, the verdicts, where every program stands: the status matrix without the deep dots
  (tools/evidence.py `matrix`), the records), the rest of round 8's minor 4; the data panel under
  Validation (content/data-panel.md -> content/validation/data-panel.md, redirect); the roadmap's
  words; 404; the site's README.md and CLAUDE.md in the new words (CLAUDE.md: the pastels are
  eight + LARMORX).
- **E (relay 27, DONE)** (difftest.json's page keys become the /evidence/ addresses; each moved page's
  `ev.random_testing('/tools/<page>/', ...)` call with them) the random-testing table brought up to date (difftest.json, difftest.py): from the
  recorded runs' summaries since the last long run (t2smap's and the FSL programs' runs), no new
  run; Validation's random-testing section with it.

### Q: a reference page for every program (rule 48: no restricted link)
- **Q1** the exporter's command-line readers for the other families (ANTs, AFNI, tedana,
  sdcflows, `mri hmc`; reference/code.py), proved on one program each, with tests.
- **Q2** FSL FLIRT: docs/api/fsl-flirt.md in sections (14 programs), python/larmorx/fsl/flirt.py's
  docstrings, the 14 reference pages.
- **Q3** FSL's other API pages: fsl-roi (2), fsl-fast, fsl-topup (2), fsl-maths, fsl-stats,
  fsl-prelude, mri-hmc (mcflirt's page with `mri hmc`); their wrappers' docstrings.
- **Q4** AFNI's four API pages and ANTs' first six (`antsRegistration`, `antsApplyTransforms`,
  `N4BiasFieldCorrection`, `DenoiseImage`, `Atropos`, `antsAI`/`antsAffineInitializer`).
- **Q5** ANTs' other seven and transforms-hdf5; tedana-t2smap; sdcflows-fieldmap (the command and
  the module), niworkflows-helpers, bids; resample-series, fmriprep-fieldmap-steps,
  ndimage-filters, skimage, io. `confounds` only after the confounds part has merged (its API
  page is that part's file).
- Each: the Programs data regenerated (`reference all --site`), every evidence page's
  "Reference" line pointing at the new pages, the examples' test passing.

### D: the repository's documents and code words
- **D1** the Python wrappers' printed messages in the 9 modules ("(the clean-room original)
  failed" and the rest; bet.py's tanh refusal) and the docstrings Q did not reach
  (python/larmorx/, 42 files); tests that match them.
- **D2** README.md (beyond the generated table), docs/overview.md, docs/licensing.md,
  docs/architecture.md, docs/porting-guide.md, docs/figures.md, PLAN.md (§4's statuses: through
  the handoff, the orchestrator's), CHANGELOG.md's Unreleased.
- **D3** docs/validation's documents (README's prose, golden.md, difftest.md, oracle-cache.md,
  oracle-runs.md, oracles.md), docs/findings (27 files, words only), the register and
  docs/api/upstream-defects.md (words only; ids and entries unchanged).
- **D4** code words that are not clean-room code: larmorx-cli's and the replica crates' own
  lines (crates/, crates-nc/, crates-gpl/), scripts/, tests/ (names, messages), the validation
  package's report writers (parity/render.py, stages/record.py, tables.py, the audit's summary,
  the bench renderer: the new words for the next records); golden cases recorded again where a
  printed line changed (record_golden.py from a clean commit, after the quick check).
- **F** programs.py refuses a record marked validated outside VALIDATED_BEFORE unless it names
  its extensive evidence (a field pointing at its final-phase or panel record); tested (with D4
  or Q1, whichever has room).
- **G** the list of lines the clean rooms print in the old words (a grep over printed strings of
  crates/larmorx-afni, -mri, -fsl, -tedana and larmorx-core's clean-room files), for a clean-room
  agent: into HANDOFF.md (with D4).
- **M** the merge's material (the last relay): every page's inventory after >= before against
  the site's main (b768293a), screenshots, the links, HANDOFF.toml and HANDOFF.md, the final
  report.

Parallel parts to keep clear of: the confounds part (validation/tools.toml rows at its handoff,
docs/api/confounds.md, the confounds evidence if any) and FreeSurfer's preparation
(docs/freesurfer/, validation/investigations/freesurfer-oracle/).

## State (relay 28 done, 2026-11-13): STEP 1 READY TO MERGE
- Done: **the reviewer's round 25** (2 important, A and B, each its own commit; the minors C-N and the notes; Review
  answers, Round 25) and **M1's rest**, step 1's merge material.
- **Heads:** repository `08da1e80` (140 commits not in main; this relay 8af53a29, 73e32390, 77b7842d, b8cddfd5,
  08da1e80); site `edede81` (65 commits on b768293a; this relay bd4a694 (A), d8b706f (B), 6dbb6e8 (programs' anchor),
  737fb9a (minors and the exports), edede81).
- **The inventory against the published site b768293a, judged** (tools/r27/inventory-vs-main.md; tools/r28:
  shown-multiset.txt, redirect-anchors.txt, json-keyed-all.txt; repository merge_inventory.py): every NO row is a
  redirect (29 old addresses), Getting started's 84 words (the packages' paragraph in full on Concepts), or numbers that
  moved (3dvolreg's rows swapped, mri-hmc's table reordered with its 17 rows' numbers and notes equal by head, BET's
  default view on FSL 6.0.7.23 with the same values) but the three records made again since (3dTshift's replica 315 ->
  317, antsRegistration 576 -> 584 and its thread band, 11 of 36 values; antsAI 171 -> 172) and Validation's summary of
  them. As multisets no page shows any other number less. 7,325 anchors of the 29 old pages all land. The 28 JSON lose
  no number or field but those (the old `groups` are `order.groups`: 55 examples, every comparison kept). blobs.sha256
  1,436 -> 1,454 (18 added, all tedana-t2smap.json's clean room; 0 removed or changed). Links: no new external link
  but the site's own addresses; the 36 no longer linked still served (redirects, pointer files).
- **Round 25's fixes, checked:** build 0 problems; numbers Validation 551/551, resample-series 83/83 and 77/77 with 8
  viewers, nifti-io 82/82, afni-volreg 683/683, afni-tshift 174/174, afni-automask 156/156, Atropos 459/459, the data panel
  30/30, mri-hmc 261/261 (tools/r28/checks-r25.txt); inventory against d8c07df (built in <scratch>/before-r28, a site
  worktree): every page after >= before, no number less (tools/r28/inventory-r25.md); exports (tools/r28/reexport.sh,
  boxes of 8 GiB): resample-series (the poster, no new blob), nifti-io (1 text), difftest (5 texts), afni-volreg (1
  text), summary (the note); blobs.sha256 unchanged. Screenshots: shots-r25/ (the roadmap's table, the home page's
  planned list, Validation's reading list, matrix and runs table, 3dTshift's random-testing panel, 3dvolreg's triage
  table, the resampler's example without scripts, the Programs page; 1440 and 390, light and dark).
- **M1's screenshots:** shots-m1/, every page's top (67 pages, 268 shots; the redirects without scripts), taken before
  round 25 (its changed parts in shots-r25/).
- **The trial merge** (tools/r28/trial_merge.sh, worktree tools/r28/mergetest, a git worktree of the repository in
  scratch: removable with `git worktree remove --force`): main 81e665e8 into 08da1e80: 3 conflicts (README's
  generated table, docs/findings/README.md, __main__.py's PARITY_MODULES), kept from both sides; HANDOFF.toml's 28 ops
  apply; tables, `reference all --site` (56, 0 problems: needs op 26, `nipypecli run`), the merged site's build 0
  problems, summary/panel/paths checks clean; Python gates 1,716 passed, 21 skipped, 1 flaky (passes alone).
- **Gates (branch, tools/r28/gates.log; 8 GiB box, 4 workers, FSLDIR unset):** 1,670 passed, 14 skipped, 1 failed
  under load that passes alone twice (test_difftest's memory-limit test); ruff, format, SPDX, tables, tables --site,
  reference (55, 0 problems), summary_check (46, 0 differ), panel_runs_check (0), site_paths_check (0).
- **HANDOFF.toml** (28 ops, 4 files, after the merge: next-steps 25, programs.toml 1, NIB-04 1, the decision log's
  two-step entry 1) and **HANDOFF.md** (how step 1 merges: the conflicts, the commands, evidence-blobs before the
  site's main, larmorx main before the site). dirty_provenance.py: the same 41 rows the op names.
- **The step-1 report** is in the relay's final message (a subagent may not write report files): the orchestrator
  puts it in docs/parts/.
- **NEXT: step 2** (after the orchestrator merges step 1): Q1-Q5, D1-D4, F, G, the confounds' pages (and the planned
  card's removal), M2; left from reviews: verdicts.yml's `means` for within-variability (round 25's note), the
  sidebar's indentation of Why Rust and Python, CITATION.cff's words.

## State (relay 27 done, 2026-11-13)
- Done: **P2** (Validation reorganised, the data panel under it, the roadmap's words with the confounds'
  HANDOFF-site.toml, 404, the site's README.md and CLAUDE.md), **E** (random testing up to date: the long run in the
  new words and the 25 runs since, port by port), **the reviewer's round 24** (4 important, 15 minor: all fixed; Review
  answers, Round 24) and **M1's machine paths** (blocking for step 1: fixed). "Ready for review": P2, E, M1's
  machine paths; round 24's fixes are in Review answers.
- **Repository commits** (`git log 8be96831..`): d7579286 (P2), 5d784ef9 (E), c184170b, 2a6b5670 (M1 paths),
  c6f11991 (P2/E words: panel.py, difftest), 1728dd54, 88db9f47 (round 24). **Site commits** (`git log 50a13a0..`):
  ce1205f, 9700380 (P2), 806fce6 (E), f9de904 (M1 paths), d8c07df (round 24 and the last exports).
- **Exports** (tools/r27/reexport.sh, boxes of 8 and 12 GiB, no LARMORX_FMRIPREP_RUNS; the JSON before in
  tools/r27/before/): difftest (E; again at c6f11991), ants-programs, afni-volreg, sdcflows-estimation,
  sdcflows-fieldmap, mri-hmc, tedana-t2smap, fsl-bet, fsl-flirt (M1 paths, at c184170b: tools/r27/pathdiff.py, every
  changed string machine_paths(old) but ants-programs' 420 commands -> `<fMRIPrep run>/`), panel (c6f11991: 3 texts;
  the catalog's criterion now says "all fourteen done, exit 0", a newer catalog fact), nifti-io, resample-series,
  summary (round 24, at 1728dd54/88db9f47: nifti-io 74 texts + verdict_labels; resample-series 4 texts, the default's
  fifth panel, provenance +1 run line and +1 cache entry appended last; summary 1 text). blobs.sha256 unchanged
  (1,454 blobs) every time; blobs scanned for paths: none.
- **Checks:** build 0 problems, 67 pages; numbers (port 8797; tools/r27/checks-m1.txt, checks-r24.txt): Validation
  551/551 (443 + the runs' 108), ants-programs 448/448 and 408/408 with 74 viewers, afni-volreg 683/683,
  sdcflows-estimation 527/527, sdcflows-fieldmap 158/158, mri-hmc 261/261 and 259/259 with 12 viewers, tedana-t2smap
  1,141/1,141, fsl-bet 1,051/1,051, fsl-flirt 349/349, ants-n4 748/748, nifti-io 82/82, afni-automask 156/156, the
  data panel 30/30, the resampler 83/83 and 77/77 with 8 viewers; 0 bad. site_paths_check.py: 0 (42 before).
  Inventory against 50a13a0 (tools/r27/inventory-p2.md, the data panel paired in tools/r27/before-paired): every page
  after >= before but words (mri-hmc 29,378 -> 29,377: the random-testing label); the old /data-panel/ a redirect.
  Gates (tools/r27/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset): 1,671 passed, 14 skipped (gates.log); after round
  24 (gates2.log) 1,670 passed and 1 failed under xdist that passes alone twice
  (tests/python/test_sdcflows_app.py::test_library_sets_choose_the_writers_lapack: not touched here); ruff, format,
  SPDX, tables, tables --site, reference (55, 0 problems), summary_check (46, 0 differ), panel_runs_check (0
  outside), site_paths_check (0). page_old_words: Validation's and the roadmap's FSL where FSL is meant, the long
  run's labels with "(larmorx mri hmc)" and "replica (larmorx-nc)" (package names), the data panel's catalog texts
  fixed. Screenshots: shots-p2/ (Validation's parts, the matrix, the two random-testing tables, the data panel and its
  redirect without scripts, the roadmap, 404, the home page's planned list, round 24's: NIfTI I/O's top and agreement,
  the resampler's top and native example, Concepts' settings and clean rooms; 1440 and 390, light and dark).
- **NEXT (step 1): M1's rest** (the merge material for step 1): (a) the inventory against the live site b768293a is
  made: tools/r27/inventory-vs-main.md (b768293a built in before-main, the moved pages paired in
  tools/r27/main-paired: /tools/* -> /evidence/*, /tools/ -> /programs/, installation -> getting-started,
  why-rust-and-python -> concepts/, data-panel -> validation/): judge every "NO" row: the old addresses are redirects
  (expected); the evidence pages' "shared shown numbers changed" and "removed" (afni-tshift 3: its record re-made,
  315->317; afni-volreg 6: example 3's motion stats; ants-ai 4 + 1 removed identical-cr; ants-registration 16 + 1
  removed identical-cr, cases_total and the band; fsl-bet 6 + 1; mri-hmc 67: moved rows, relay 23's shown_rows.py;
  validation 6: summary.json's counts) against each chunk's own inventory in the notes (relays 12-26) and the
  records re-made on main since b768293a; words that fell (getting-started 2,629 -> 2,545: the packages on Concepts);
  (b) the links (the build checks them; also every redirect's anchors); (c) screenshots of every changed page's top
  at 1440 and 390, light and dark; (d) HANDOFF.toml/HANDOFF.md for the documents step 1 touches (the existing 25 ops on
  docs/next-steps.md; dirty_provenance.py once more; the register's NIB-04 wording, HANDOFF.md; the shared documents
  this part changed directly, listed under "Shared documents changed directly"); (e) a short report. Then step 2: Q,
  D, F, G, the confounds' page (remove status.yml's "Confounds, their pages" planned card when the confounds' row is
  in the Ported table), M2.
- **For step 2 / the orchestrator:** the confounds' tools.toml row is on main (636831fa), not in this branch: after
  the merge, `tables --site` adds it to the Ported table, and the planned card "Confounds, their pages" goes;
  programs.toml's confounds row (`records`) then names its five records. The sidebar's indentation of Why Rust and
  Python under Concepts (round 24's note) is a layout change for M2.

## State (relay 26 done, 2026-11-13)
- Done: **S14** (the resampler's and NIfTI I/O's evidence pages, site.yml's evidence entries and summary.json in the
  new words: "Ready for review", S14), **P1** (the header, Getting started, Concepts, Why Rust and Python under
  Concepts, About, the home page's links: "Ready for review", P1) and **the reviewer's round 23** (S13 PASS, 3
  minor, all fixed: Review answers, Round 23).
- **NEXT (step 1 continues): P2** (Validation reorganised: the guide's section 6 (how LARMORX tests, how to read the
  evidence, the verdicts, where every program stands: the matrix without the deep dots, its header "Program" /
  "Port" and the Programs page's order of the software, tools/evidence.py `matrix`; the records), the rest of round
  8's minor 4; the data panel under Validation (content/data-panel.md -> content/validation/data-panel.md, a
  redirect; its link to /tools/afni-automask/#panel -> /evidence/afni-automask/#panel); the roadmap's words (its
  /tools/fsl-bet/ links -> /evidence/fsl-bet/) **and the confounds' site handoff** (FROM THE ORCHESTRATOR, 13:00:
  `<scratch-root>/confounds/HANDOFF-site.toml`, 7 ops on the roadmap and the status page's planned list, applied in
  the new words); 404; the site's README.md and CLAUDE.md in the new words), then **E**, then **M1** (with the
  machine-path fix: `_short` in figures/cache.py, then ants-programs and the other pages holding a home path
  exported again; see the finding below).
- **Repository commits** (`git log f9c39cc3..`): a01a4523, 76a7f303, f1d22bff (S14), 8be96831 (round 23). **Site
  commits:** 45ca4a6, fda17d6 (S14), 19fffb6 (S14's last export and P1), 107114e (P1), 50a13a0 (round 23).
- **Exports** (tools/r26/reexport.sh, an 8 GiB box, no variable; the JSON before in tools/r26/before/):
  resample-series last at f1d22bff, nifti-io at 76a7f303 (computed in-process from the test data, no cache: the
  same numbers), sdcflows-estimation at 8be96831 (round 23: 8 texts, nothing else); summary.json made again
  (`evidence summary --site <copy> --no-compute`: texts only). export_diff: texts, ports, order, shows,
  presentation and the categories' names only; blobs.sha256 unchanged (1,454 blobs) every time.
- **Checks:** build 0 problems, 66 pages (62 before P1). Numbers (port 8796, tools/r26/checks-s14.txt,
  checks-s14b.txt): resample-series 83/83 on load and 77/77 with 8 viewers loaded, nifti-io 82/82, the redirect
  83/83, validation 443/443, sdcflows-fieldmap 158/158; 0 bad. Inventory (tools/r26/inventory-s14.md,
  inventory-p1.md, against b46854b built in before-r24, the moved pages paired in tools/r26/before-paired): every
  content page after >= before but words (resample-series rows 157 -> 158, headings 11 -> 13, links 288 -> 301;
  nifti-io headings 9 -> 10, links 502 -> 509; Getting started links 102 -> 103), 0 shared numbers changed; the
  header's 8 -> 6 links (shown twice) leave each redirect page 2 links fewer. Round 23's pages after their
  export: sdcflows-estimation 527/527, sdcflows-fieldmap 158/158 (tools/r26/checks-r23.txt). Regression gate
  (`--ref f9c39cc3`, tools/r26/gate1.txt): 0 differences on the 22 pages presented before. Old words (page_old_words.py): package names,
  the provenance's quoted strings, FSL where FSL is meant, random testing's "Tool" (chunk E's). Gates
  (tools/r26/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r26/gates.log): 1,668 passed, 14 skipped, 0
  failed; again after round 23 (gates2.log): 1,667 passed, 1 failed under load that passes alone (test_difftest's
  memory-limit test, the one noted by relay 25); ruff, format, SPDX, tables, tables --site, reference (55, 0 problems), summary_check (46, 0 differ),
  panel_runs_check (0 outside).
- **Lesson:** the serving build (`--serve`) rebuilds on every change into the same `_site`: a manual build while it
  runs races with it (181 "problems" of missing files once). Stop the server (by the PID that holds the port: `ss
  -ltnp`) before building.
- **Found (for M1, important): ants-programs.json holds 490 case commands with the full home path (account name
  included: "/home/<user>/<projects folder>/...")**, since relay 14's export with LARMORX_FMRIPREP_RUNS=panel-runs
  (symlinks): the live site (b768293a) has the placeholder `<fMRIPrep oracle run>`. Re-exporting ants-programs
  (PANEL_RUNS committed, no variable) should restore `<fMRIPrep run>`; with the orchestrator's `_short` fix
  (machine paths) in M1. Other machine paths: afni-volreg (6 reasons, ~/<projects folder>), mri-hmc (1),
  tedana-t2smap (1), fsl-bet (22 PYTHONPATH), fsl-flirt (15), sdcflows-fieldmap (79), sdcflows-estimation (39 +
  31 LARMORX_WORKSPACE); "/home/fmriprep" in 4 (the container's home: fine).

## State (relay 25 done, 2026-11-13)
- Done: **the reviewer's round 22** (finding 1 and minors 2, 3: Review answers, Round 22) and **S13** (sdcflows'
  two pages: "Ready for review", S13).
- **NEXT, STEP 1 of the publication (the user's decision, FROM THE ORCHESTRATOR, 2026-11-13 ~09:50; target this
  evening), in this order:** **S14** (fMRIPrep's resample-series, resample_series.py, 2 examples; NIfTI I/O,
  nifti_io.py; the sidebar and site.yml's `tools:` all in the new words; summary.json in the new words with the
  /evidence/ addresses, then tools/evidence.py's `moved` mapping can go; summary.py still lists /tools/ addresses
  for fsl-prelude, sdcflows-fieldmap, tedana-t2smap and sdcflows-estimation, lines 224-254), then **P1**, **P2**,
  **E** (the rollout plan's sections P and E), then **M1** (the merge material for step 1: every page's inventory
  after >= before against the site's main b768293a, screenshots, the links, the build, HANDOFF.toml/HANDOFF.md for
  what step 1 touches, a short report). Pages not yet converted keep their form and stay reachable; nothing the
  site shows may get worse between the steps. **Step 2** (a day later): Q1-Q5, D1-D4, F, G, the confounds' page
  (after the confounds part merges), M2. Fix the reviewer's findings first, as before (round 23 will cover S13).
- **Repository commits** (`git log 06fb0cab..`): fc29ff70, 2116d866 (round 22), 2ca3c0b3, 6bb47904, 10a2c416,
  f9c39cc3. **Site commits:** f3c5ac7 (round 22), 1711d35 (S13), b46854b (S13: the JSON exported again at f9c39cc3).
- **Exports** (tools/r25/reexport.sh, a 12 GiB box, about 30 s a page; the JSON before in tools/r25/before/):
  t2smap at 2116d866 (round 22: 79 blobs named, 0 lost; blobs.sha256 +3, the three `cr-ted` posters); both
  sdcflows pages last at f9c39cc3 (blobs: 38 and 61 named, all kept, 0 new; blobs.sha256 unchanged; export_diff:
  texts, ports, order, shows, the categories' names and the presentation mark only; the old `groups` replaced by
  `order.groups` as on every presented page).
- **Checks** (site built, 60 pages, 0 problems): number checks (port 8795) sdcflows-fieldmap 158/158 on load and
  132/132 with every viewer loaded (62 viewer checks), sdcflows-estimation 527/527 and 509/509 (76 viewer checks),
  the two redirects the same, validation 443/443, t2smap 1,141/1,141 and 1,135/1,135 (126), 0 bad
  (tools/r25/checks-s13.txt, checks-s13b.txt and checks-s13c.txt after the last export, checks-t2s.txt). Inventory against f3c5ac7 (before-r24 checked out there and built; the
  moved pages paired in tools/r25/before-paired; tools/r25/inventory-s13.md): every page after >= before
  (sdcflows-fieldmap rows 153 -> 156, headings 20 -> 22, links 245 -> 260, words 11,854 -> 12,982;
  sdcflows-estimation rows 295, headings 20 -> 22, links 366 -> 384, words 19,506 -> 20,637; 0 shared numbers
  changed); the /tools/ addresses redirects (the accepted move). The regression gate (`--ref 06fb0cab`,
  tools/r25/gate1.txt): 0 differences on the 20 pages presented before. page_old_words.py (tools/r25/old-words*.txt):
  package names, paths, the provenance's quoted strings, FSL where FSL is meant.
- **Gates** (tools/r25/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r25/gates.log): 1,664 passed, 14
  skipped, 2 failed under load that pass alone (test_sdcflows_app's LAPACK test, the order-dependent one already
  noted; test_difftest's memory-limit test, with the screenshots running beside it); ruff, format, SPDX, tables,
  tables --site, reference (55, 0 problems), summary_check (46, 0 differ), panel_runs_check (0 outside).
- The local server (port 8795): stopped by its PID (tools/r25/serve.pid). Screenshots: shots-s13/ (both pages: top,
  ports, agreement, speed, History, the redirect without scripts; fieldmap's ds005454, apply and SDC-10 examples and
  corrections; estimation's PEPOLAR, SyN and epi_mask examples and random testing; 1440 and 390, light and dark;
  every example scrolled into view first so its tiles load; the agreement charts and estimation's SyN and masker
  examples again after the last export, tools/r25/shots-s13b.sh), shots-r22fix/ (t2smap's three examples).

## State (relay 24 done, 2026-11-13)
- Done: main (e36d1707) merged (0446079a); **chunk T** and **S12** (t2smap): "Ready for review", S12 and T;
  **the reviewer's round 21** (S11 PASS, 2 minor): both fixed (Review answers, Round 21). NEXT: **S13**
  (sdcflows' field-map estimation, sdcflows_fieldmap.py, 6 examples, and PEPOLAR, direct and fieldmap-less
  estimation, sdcflows_estimation.py, 5): start with the probe (`probe_present.py <site> --page
  sdcflows-fieldmap --page sdcflows-estimation`), the regression gate (`--ref 06fb0cab`; no intended
  differences), then the dry runs (tools/r24/dry1.py and show.py show a module's `present` on a JSON);
  every export of these pages needs the fourteen runs (they use fmriprep_nodes: panel_runs_check). Models:
  tedana_t2smap.present (a page whose original is not FSL or AFNI: its own Page, TEXTS, SHOWS, table titles
  and columns), fsl_prelude.present.
- **The wheel**: the worktree's python/larmorx/_core.abi3.so is now the build of 0446079a (scratch build24/,
  CARGO_TARGET_DIR there, 3 min with a copy of the main checkout's target/release as seed); no Rust changed in
  this branch since.
- **Repository commits** (`git log 2b914894..`): 0446079a (merge), 17c4d489, 921d47bb, 00eae8c8, 90b45755,
  06fb0cab. **Site commits:** f284bf8 (round 21), 2b2f380 (S12, T), 0f321cf (the option sentence, the JSON
  again).
- **Exports** (tools/r24/reexport.sh, t2smap alone in a 12 GiB box, about 3 minutes each; the JSON before
  in tools/r24/before/): the last at 06fb0cab; tools/r24/blobs_check.py: 64 old blobs kept, 15 new, 0 lost.
- **Checks** (site 0f321cf, built: 58 pages, 0 problems): number checks on load 1,141/1,141 (the redirect the
  same), with every viewer loaded 1,135/1,135 (126 viewer checks), 0 bad; the old page (site 8b97b25, served
  from before-r24) 854/854 and 848/848 (86 viewer checks); validation 443/443, prelude 291/291, mri-hmc
  261/261 (tools/r24/checks-new.txt, checks-new2.txt, checks-before.txt). Inventory against 8b97b25
  (before-r24 built; the moved page paired in tools/r24/before-paired; tools/r24/inventory-s12.md): every
  page after >= before (t2smap: numbers 856 -> 1,143, headings 18 -> 20, tables 18, rows 463 -> 512, figures
  42 -> 45, viewers 4, images 35 -> 38, links 465 -> 528, words 26,893 -> 30,643; 0 shared numbers changed);
  /tools/tedana-t2smap/ a redirect (the accepted move). tools/r24/num_check.py: 0 numbers changed or lost.
  The probe: tedana-t2smap presented; the regression gate (`--ref 2b914894`, tools/r24/gate1.txt): 0
  differences on every presented page. page_old_words.py (tools/r24/old-words.txt): package names, paths,
  the provenance's quoted strings, tedana's own `--n-threads`, the footer's FSL. Phrases: tools/r23/scan.py
  and tools/r21/phrases.py on the JSON (every "original" left is tedana or an id; every one turned into "the
  clean room" was the clean room).
- **Gates** (tools/r24/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r24/gates.log): 1,664 passed, 14
  skipped, 0 failed; ruff, format, SPDX, tables, tables --site, reference (55, 0 problems), summary_check
  (46, 0 differ), panel_runs_check (0 outside).
- The local servers: stopped by their PIDs (tools/r24/serve.pid, serve-before.pid). Screenshots shots-s12/
  (top, ports, agreement, verdicts, the three maps and series examples with the clean room's panel, the
  clean room's own example, calls, the clean room's calls, corrections, speed, speed and memory of the clean
  room, random testing of both, History, the redirect without scripts; 1440 and 390, light and dark; the
  examples' image panels show their placeholders in element shots, lazy loading, as before).
- **For the merge: the 15 new blobs** are in the site clone's static/evidence/blobs/ (git-ignored): the branch
  evidence-blobs must be made again (`tools/blobs.py commit`, 1,451 blobs) and pushed with the site's main.
  **The new cache entry** oracles/figures-cache/tedana-t2smap/clean-room-ds000258-sub-21262-rest-curvefit
  (computed at 17c4d489, clean).

## State (relay 23 done, 2026-11-13)
- Done: main (5c5383ca) merged (3874a564); **chunk S11** (prelude and the head-motion correction): "Ready
  for review", S11; **the reviewer's round 20** (S10 PASS, 3 minor): all three fixed (Review answers,
  Round 20). NEXT: **S12** (tedana `t2smap`: tedana_t2smap.py is BRIEF_RESTRICTED, wording only;
  tedana_t2smap_clean_room.py a clean room's module, the hardening rule; 4 examples; then chunk T if it
  fits). Start with the probe (`probe_present.py <site> --page tedana-t2smap`) and the regression gate
  (`--ref 4815a494 --intended <scratch>/tools/r23/intended.txt`: r21's list plus fslmaths' four round-20
  texts), then a dry run (tools/r23/dry.py `<site> <out> <page> <module> <program>`; it uses the module's
  `present` if it has one). Models: fsl_prelude.present (a clean room's exporter calling the replica
  module's present with the hardening, as afni_volreg_clean_room), mri_hmc.present (a page whose
  records are against two versions: a Page per version). t2smap's provenance lines gain the reruns clause
  at its export (round 19's note). Check phrases with tools/r21/phrases.py and tools/r23/scan.py.
- **Repository commits** (`git log 5c5383ca..`): 3874a564 (merge), 8e47a17e, 852d2bda, 8b5b7cdb, 380fcfb5,
  4815a494, 2b914894 (the motion chart's legend in the guide's order). **Site commits:** 460def5 (S11),
  ef89f1f (round 20; the JSONs of fslmaths and prelude again), 8b97b25 (mri-hmc's JSON again: the legend).
- **Exports** (tools/r23/reexport.sh; prelude and mri-hmc in 8 GiB boxes, fslmaths alone in 14 GiB; the
  JSONs before in tools/r23/before/): prelude "added port 192, text changed 568, ...", the old `groups`
  fields moved into `order` (as every presented page); mri-hmc "text changed 549, added port 30, ...",
  its table's rows reordered (checked: stats_check.py, shown_rows.py); the later exports: fslmaths 4 texts,
  prelude 1, mri-hmc the legend's order (2 keys swapped); no number changed; blobs.sha256 unchanged,
  every blob byte for byte.
- **Checks** (site ef89f1f, built: 57 pages, 0 problems): number checks on load prelude 291/291, mri-hmc
  261/261, the two redirects the same, FLIRT 349/349, Validation 443/443; with every viewer loaded prelude
  239/239 (88 viewer checks, 10 viewers), mri-hmc 259/259 (12 viewer checks), 0 bad (tools/r23/checks.txt;
  after round 20: tools/r23/checks2.txt, fslmaths 462/462 and 450/450 with 152 viewer checks, fslstats
  88/88; after the legend: tools/r23/checks3.txt, mri-hmc 261/261 and 259/259); inventory against site 9a98cec (before-r21 checked out there and
  built; the moved pages paired in tools/r23/before-paired; tools/r23/inventory-s11.md): every page after
  >= before (prelude: numbers 292 = 292, headings 17 -> 19, tables 17, rows 355 -> 368, figures 89, viewers
  10, images 65, links 553 -> 585, words 26,442 -> 29,425; mri-hmc: numbers 262 = 262, headings 10 -> 12,
  tables 7, rows 303 -> 305, figures 18, viewers 1, images 28, links 847 -> 861, words 27,883 -> 29,372;
  its 67 "changed" shared numbers are the table's reorder, item (3) above; 0 elsewhere). The probe:
  fsl-prelude 10 of 10, mri-hmc 1 of 1 pass; the regression gate (`--ref 629ebc2e`, tools/r23/gate1.txt):
  0 differences on every presented page but fslmaths' four intended texts (round 20), now in
  tools/r23/intended.txt. page_old_words.py: FSL's name, package names, "with larmorx <commit>", the
  quoted `LARMORX_IMPLEMENTATION=original` of the clean room's runs, a path, and on mri-hmc the
  random-testing panel's label and explanation (chunk E). The "original" phrases: tools/r23/phrases1.txt,
  phrases-hmc.txt (every one turned into "the clean room" was the clean room; those kept are FSL's, ids,
  the cache's words).
- **Gates** (tools/r23/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r23/gates.log): 1,660 passed,
  14 skipped, 0 failed; ruff, format, SPDX, tables, tables --site, reference (55, 0 problems), summary_check
  (46, 0 differ), panel_runs_check (0 outside); after 2b914894: test_evidence_present.py 42 passed, ruff,
  format and SPDX clean.
- The local server: stopped by its PID (tools/r23/serve.pid). Screenshots shots-s11/ (prelude: top, ports,
  the two fMRIPrep tables, ds005454, FSL-272, ramps, truth, the three random-testing tables, speed,
  benchmark, History, the redirect without scripts; mri-hmc: top, the example with its motion chart, the
  comparisons' table, speed, random testing, the catch-up's two tables, open, History, the redirect; the
  agreement charts and round 20's parts: shots-s11/extra-*; 1440 and 390, light and dark). The examples'
  image panels show their placeholders in element shots (lazy loading, as before).
- **For chunk E:** difftest.py's PAGES labels ("head-motion correction, clean-room original", "mcflirt
  replica") and the explanation "larmorx over mcflirt" in difftest.json show on mri-hmc's random-testing
  panel and Validation's table.

## State (relay 22 done, 2026-11-13)
- Done: **chunk S10** (fslmaths and fslstats): "Ready for review", S10; **the reviewer's round 19** (S9 PASS, 3
  minor): all three fixed on topup's page (Review answers, Round 19). NEXT: **S11** (FSL prelude,
  fsl_prelude.py and fsl_prelude_clean_room.py, 10 examples; head-motion correction, mri_hmc.py,
  content/tools/mri-hmc.md, mcflirt's replica and the clean room `mri hmc`, 1 example). Start with the probe
  (`probe_present.py <site> --page fsl-prelude --page mri-hmc`) and the regression gate (`--ref 629ebc2e
  --intended <scratch>/tools/r21/intended.txt`; it lists round 17's notes and FAST's six, still valid), then a
  dry run (tools/r22/dry.py takes `<site> <out> <page> <module> <program>`), the models fsl_maths.present /
  fsl_topup.present with fsl_pages. **fsl_prelude_clean_room.py is a clean room's evidence module (wording
  only; the hardening rule if it calls a replica presenter: a neutral message).** Check which module calls
  which (fsl_maths.py imports its clean room's module; afni_volreg_clean_room.py calls the replica's). The
  metrics notes: fsl_pages.replica_threshold takes "every file <program> writes" now. mri-hmc and t2smap's
  provenance lines gain the reruns clause at their exports (round 19's note). Check fsl_pages.TEXTS phrase by
  phrase (tools/r21/phrases.py <old json> <dry json>) and scan the presented JSON for ",.", ",;", "with
  reproduce", "the original's suite" and the like (tools/r22/scan.py <json>).
- **Repository commits** (`git log 1178e9f1..`): a47898af (fslmaths' and fslstats' present; fsl_pages'
  replica_threshold by regex), 23912d04 (the addresses), 8dffe711 (the test, 39 in test_evidence_present.py),
  b986d806 (the synthetic inputs' placeholder and glibc's cells), 629ebc2e (the glibc note's subject).
  **Site commits:** dd06805 (the two pages, redirects, site.yml, roadmap, generated data, the JSONs), a6ca1f7
  (round 19 on topup's page), b5b5bba and 9a98cec (the JSONs again: texts only).
- **Exports** (tools/r22/reexport.sh; fslmaths alone in a 14 GiB box, fslstats in 8 GiB; the JSONs before in
  tools/r22/before/): export_diff against the old JSONs: fslmaths "text changed 426, added port 51, order 3,
  shows 3, ...", groups moved into order; fslstats "text changed 131, added port 6, ..."; the later exports
  25, 4 and 2 texts; no number changed; blobs.sha256 unchanged.
- **Checks** (site 9a98cec): build 0 problems, 55 pages; number checks on load fslmaths 462/462, fslstats
  88/88, topup 1,028/1,028, the two redirects the same, Validation 443/443; with every viewer loaded
  fslmaths 450/450 (152 viewer checks, 3 viewers), fslstats 88/88 (no viewer), topup 1,014/1,014 (182), 0 bad;
  inventory against site a1c1d73 (worktree before-r21 checked out there and built; the moved pages paired in
  tools/r22/before-paired; tools/r22/inventory-s10-final.md): every page after >= before (fslmaths numbers 463
  = 463, headings 13 -> 15, tables 14, rows 624 -> 629, figures 30, viewers 3, images 26, links 1,145 ->
  1,161, words 41,901 -> 44,077; fslstats numbers 89 = 89, headings 11 -> 13, tables 7, rows 281, figures 9,
  links 505 -> 511, words 15,578 -> 16,439; topup words 46,812 -> 46,844; 0 shared shown numbers changed on
  any page). The probe: fsl-maths 3 of 3 pass, fsl-stats 0 examples; the regression gate (`--ref 1178e9f1`):
  0 differences on every presented page (tools/r22/gate2.txt). page_old_words.py: only FSL's name, package
  names, "with larmorx <commit>", "larmorx `<commit>`" in a run's note, quoted cache-entry strings,
  "LARMORX's implementation of glibc's ..." and "a LARMORX mistake" (as topup's triage). The "original"
  phrases: tools/r22/phrases-maths.txt and phrases-stats.txt (every one turned into "the clean room" was the
  clean room; those kept are FSL's, the cache's words, ids and quoted names).
- **Gates** (tools/r22/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r22/gates.log): 1,645 passed, 14
  skipped, 1 failed under load and passed alone (test_difftest.py's memory-limit test, the known flaky one);
  after the last two commits validation/tests again: 690 passed; ruff, format, SPDX, tables, tables --site,
  reference (55, 0 problems), summary_check (46, 0 differ), panel_runs_check (0 outside).
- The local server: stopped by its PID. Screenshots shots-s10/ (fslmaths: top, ports, agreement, the three
  examples, the combine calls, the fslstats numbers, the families' clean-room table, both glibc tables, the
  three random-testing tables, speed, benchmark, History, the redirect without scripts; fslstats: top,
  agreement, goodvoxels, options, the clean room's run, speed, History; round 19: topup's verdicts and
  Corrections; 1440 and 390, light and dark). The examples' image panels show their placeholders in element
  shots (lazy loading, as before; the viewers' check loads them all).

## State (relay 21 done, 2026-11-13)
- Done: **chunk S9** (topup and applytopup): "Ready for review", S9; **the follow-up for the -dirty benchmark
  reports and cache entries**: HANDOFF.toml op 25 (docs/next-steps.md, "Follow-ups, when they fit", after
  "Benchmark reports": 7 reports and 34 cache entries by name and commit) and HANDOFF.md's section, made from
  `validation/investigations/docs-revision/dirty_provenance.py <site>` (41 rows on 2026-11-13; relay M runs it
  again and updates the op if later exports name more); `apply_handoff.py --check`: 25 ops apply. **The
  reviewer's round 18** (S8 PASS, 4 minor; round 17's minor 5): all five fixed (Review answers, Round 18).
  NEXT: **S10** (FSL `fslmaths`, fsl_maths.py and fsl_maths_clean_room.py, 3 examples; `fslstats`,
  fsl_stats.py, no example). Start with the probe (`probe_present.py <site> --page fsl-maths --page fsl-stats`)
  and the regression gate (`--ref 1178e9f1 --intended <scratch>/tools/r21/intended.txt`; against an older ref
  the same file still serves: it lists round 17's notes and FAST's six), then a dry run (tools/r21/dry.py is
  topup's: copy it), the model fsl_topup.present / fsl_fast.present with fsl_pages. **fslmaths' clean room has
  its own module (fsl_maths_clean_room.py, a clean room's evidence module: wording only, and the hardening
  rule: a replica presenter's failure caught with a neutral message).** If its metrics notes read "The
  record's threshold: ... The clean room's record (...)", call `fsl_pages.replica_threshold(doc)` (round 18,
  minor 2). Check fsl_pages.TEXTS phrase by phrase (tools/r21/phrases.py <old json> <dry json>). Its cache
  entries carry -dirty commits (in the follow-up's list) and reruns (the provenance line will say so).
- **Repository commits** (`git log 2e1723f4..` after relay 20's): feeabaaf (topup's present; fsl_pages'
  examples= and speed_page=), 78a20a42 (the address), 5c59a968 (the suite's module as code), 633582b2
  (dirty_provenance.py), 37f93ded (topup's test), 1178e9f1 (round 18's minor 2: fsl_pages.replica_threshold,
  FAST and topup, a test; 38 in test_evidence_present.py). **Site commits:** f99fcbe (topup's page, redirect,
  site.yml, roadmap, generated data), 9da64d6 (topup's JSON again), 2fa5b60 (round 18 on FAST's and FLIRT's
  pages), a1c1d73 (FAST's and topup's JSONs again).
- **Exports** (tools/r21/reexport.sh; FAST alone in a 16 GiB box, topup in 8 GiB; the JSONs before in
  tools/r21/before/): every change a text, port, mark, order or presentation field (export_diff: topup
  "added port 112, text changed 631, ...", no number changed; FAST 7 texts, topup 8 at the last export);
  blobs.sha256 unchanged.
- **Checks** (site a1c1d73): build 0 problems, 53 pages; number checks on load topup 1,028/1,028, FAST
  2,826/2,826, FLIRT 349/349, Validation 443/443; with every viewer loaded topup 1,014/1,014 (182 viewer checks,
  7 viewers), FAST 2,804/2,804 (650), 0 bad; inventory against site b899cc2 (worktree before-r21, built; the
  moved page paired in tools/r21/before-paired; tools/r21/inventory-s9-final.md): every page after >= before
  (topup's /evidence/ page against the old /tools/ page: numbers 1,029 = 1,029, headings 16 -> 18, tables 25,
  rows 513 -> 525, figures 61, viewers 7, images 57, links 627 -> 660, words 44,709 -> 46,812; 0 shared shown
  numbers changed on any page). The probe: fsl-topup 7 of 7 pass; the regression gate (`--ref 1dcacdeb`):
  0 differences not intended (FAST's six notes intended, tools/r21/gate2.txt). page_old_words.py: only FSL's
  name, package names, "with larmorx <commit>", "larmorx `<commit>`" in a run's note, quoted cache-entry strings
  and a path (`oracles/inputs/fsl-topup-replica/`). The "original" phrases: tools/r21/original-phrases.txt
  (every one turned into "the clean room" was the clean room; those kept are FSL, ids and quoted names).
- **Gates** (tools/r21/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r21/gates2.log): 1,645 passed,
  14 skipped, 0 failed (the first run: 1 failed under load and passed alone, test_difftest.py's memory-limit
  test, the known flaky one in next-steps' follow-ups); ruff, format, SPDX, tables, tables --site, reference
  (55, 0 problems), summary_check (46, 0 differ), panel_runs_check (0 outside).
- The local server: stopped by its PID. Screenshots shots-s9/ (topup: top, ports, agreement, the first example,
  its files, thread band and metrics tables, ds001600, membrane, movements and its table, lsr, jac, vb3d, the five
  random-testing tables, speed, History, the redirect without scripts; round 18: FAST's ports and limits, FAST's
  and topup's first metrics notes, FLIRT's ports; 1440 and 390, light and dark). The examples' image panels show
  their placeholders in element shots (lazy loading, as on relay 20's; the viewers' check loads them all).

## State (relay 20 done, 2026-11-13)
- Done: main (3bc71341) merged (2e1723f4); **3dTshift re-exported from the clean replica record** (only the
  two record files' stamp 3a795ff019fb-dirty -> 629e357ab4aa, date 2026-11-04 -> 2026-11-13 and SHA-256
  changed, as expected; the page's two sentences on that record follow it); **chunk S8** (FSL FAST) and the
  reviewer's **round 17** (all nine, each its own commit): "Ready for review" and "Review answers", Round 17.
  NEXT: **S9** (FSL topup and applytopup, fsl_topup.py, 7 examples): start with the probe (`probe_present.py
  <site> --page fsl-topup`) and the regression gate (`--ref 1dcacdeb --intended <scratch>/tools/r18/intended.txt`:
  1dcacdeb has round 17's metrics notes; against an older ref use tools/r20/intended.txt, which adds them), then a dry run (tools/r20/dry.py is
  FAST's: copy it), the model fsl_fast.present / fsl_flirt.present; topup's cache entries have re-runs (the
  provenance line now says so). Check fsl_pages.TEXTS phrase by phrase on topup (role labels "the original
  against itself" on 2 examples: the round-17 note), as tools/r20/original-phrases.txt does for FAST.
- **Repository commits** (`git log 2e1723f4..`): 2e1723f4 (merge of main), S8's 428d66cc (present),
  6bbbd86e (its helper's name), 8f955872 (the address); round 17's 922b7dce (9), 0c6de004 (1), 10b6fbab (2),
  d9ff811c (7), 475aa848 (the API note), 1dcacdeb (7 in two more modules). **Site commits:** 5661100 (3dTshift),
  67e196d (FAST's page), e80fc16 (round 17's page texts), b899cc2 (the five JSONs exported again).
- **Exports** (tools/r20/reexport.sh; FAST alone in a 16 GiB box, the others in 8 GiB; the JSONs before in
  tools/r20/before/ and before2/): every change a text, port, mark or presentation field; numbers unchanged by
  pointer (FAST 16,730, FLIRT 1,583, 3dvolreg 3,630, antsAI 1,551, N4 3,286); blobs.sha256 unchanged.
  LESSON: a helper named like one of the module's own (`_differ`) shadows it: FAST's first export failed.
- **Checks** (site b899cc2): build 0 problems, 52 pages; number checks on load FAST 2,826/2,826, FLIRT
  349/349, 3dvolreg 683/683, antsAI 559/559, N4 748/748, BET 1,051/1,051, 3dTshift 174/174, Validation
  443/443; with every viewer loaded FAST 2,804/2,804 (650 viewer checks), 3dTshift 166/166 (23), FLIRT
  331/331 (38), 3dvolreg 669/669 (32), antsAI 533/533 (20), N4 724/724 (76), 0 bad; inventory against site
  8a28495 (before-r18 now checked out there and built; the moved page paired in tools/r20/before-paired;
  tools/r20/inventory-s8-paired2.md): every page after >= before (FAST's /evidence/ page against the old
  /tools/ page: numbers 2,827 = 2,827, headings 14 -> 16, tables 18, rows 662 -> 671, figures 50, viewers 6,
  images 45, links 476 -> 503, words 34,276 -> 36,285; 0 shared shown numbers changed on any page). The
  probe: fsl-fast 6 of 6 pass; the regression gate (`--ref 8cbaf253`, relay 19's code; tools/r20/gate1.txt,
  intended list tools/r20/intended.txt): 0 differences not intended; the 8 intended are round 17's metrics
  notes (FLIRT 5, 3dvolreg 3); FAST "presented after". (Round 17's provenance lines and FLIRT's fact are
  the exporter's, so the gate, which runs `present` on the old JSONs, does not see them.) page_old_words.py: only FSL's name, package names, quoted cache-entry strings, a path
  (`oracles/inputs/fsl-fast-replica/`) and "with larmorx <commit>".
- **Gates** (tools/r20/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r20/gates.log): 1,642 passed,
  14 skipped, 1 failed under load and passed alone (tests/python/test_sdcflows_app.py::
  test_library_sets_choose_the_writers_lapack, the known order-dependent one); ruff, format, SPDX, tables,
  tables --site, reference (55, 0 problems), summary_check (46, 0 differ), panel_runs_check (0 outside).
- **For the orchestrator:** seven benchmark reports the site cites are stamped `-dirty` (their
  `larmorx_commit`): docs/benchmarks/afni-tshift.json (3b3f69caabcb-dirty), ants-denoise (77bb3c657378-dirty),
  ants-n4 (1f345c61c340-dirty), ants-programs (2960b32a61f4-dirty), ants-gaussian (30959407505d-dirty),
  ants-morphology (57245bee1dee-dirty), mri-hmc (fc392f579ce7-dirty); and figures cache entries record a
  `-dirty` commit too (e.g. fsl-flirt's `real-applyxfm-4d-original`: 80cc208f...-dirty; the pages show its
  first 12 characters, without the mark). Records are clean; whether the rule covers benchmarks and cache
  entries is the orchestrator's to decide.
- The local server: stopped by its PID at the end (tools/r20/serve.pid). Screenshots shots-s8/ (FAST: top,
  ports, agreement, the first example, its files and volumes tables, priors, three channels, the glioma
  patient, the two random-testing tables, speed; 3dTshift's History; the redirect without scripts; round 17:
  FLIRT's top, its first example's metrics note, its History, 3dvolreg's metrics note; 1440 and 390, light and
  dark). The 390-wide shot of FAST's first example is 18,892 px tall, beyond Chrome's 16,384 px capture,
  so its tail repeats (a capture artefact; the 1440 shot and the checks show the element whole).

## State (relay 19 done, 2026-11-13)
- Done: **chunk S7** (FSL FLIRT): "Ready for review" above; the reviewer's **round 16** (S6 PASS;
  its optional hardening done, below). NEXT: **S8** (FSL FAST, fsl_fast.py, 6
  examples): its export was killed by the OOM killer in an 8 GiB box in relay 16 (7.2 GB at 4
  threads): **one export alone in a box of up to 16 GiB** (the orchestrator's decision), threads
  4. Start with the probe (`probe_present.py <site> --page fsl-fast`) and the regression gate
  (`--ref 8cbaf253 --intended <scratch>/tools/r18/intended.txt`), then a dry run with
  fsl_pages (tools/r19/dry.py is FLIRT's: copy it); fsl-fast's JSON still names
  docs/findings/fsl-fast-original.md (the pointer file serves it) and its export changes that
  path too. Fix round 16's findings first if they have arrived.
- **How FLIRT's page is presented** (the model for FSL's pages of two ports whose modules are
  readable: FAST, topup, fslmaths, fslstats): the module ends `export` with `present(doc)`,
  which calls `fsl_pages.present(doc, PAGE, SHOWS, speed caption, speed labels, categories=,
  charts=, diffs=, tables=)` and then renames what is the page's own (the examples' tables'
  columns and rows, the tables' titles by the guide's pattern, the speed note with
  `port="both"`). PAGE = `fsl_pages.page("<program>", texts=<the page's own TEXTS>)`; CHARTS a
  page with `part=` where the records cover several programs; TRIAL a page with
  `port="clean-room"` for a clean room's random-testing tables (a bare "larmorx" there is the
  clean room). SHOWS: functions reading each example's own reasons, facts and verdicts, raising
  where a verdict is not the one the sentence says.
- **Checks** (site 8a28495): build 0 problems, 51 pages; number checks on load 349/349 (FLIRT),
  1,051/1,051 (bet), 443/443 (Validation); with every viewer loaded 331/331, 38 viewer checks, 0 bad;
  inventory against 7488ffd (scratch before-r18/, now checked out at 7488ffd and built; the moved
  page paired in tools/r19/before-paired; reports tools/r19/inventory-s7.md and
  inventory-s7-paired.md): every page after >= before but /tools/fsl-flirt/ (the redirect), whose
  /evidence/ page holds as much or more (numbers 350 = 350, headings 14 -> 16, tables 16, rows
  485 -> 495, figures 52, viewers 5, images 49, links 903 -> 921, words 41,101 -> 43,053; 0
  shared shown numbers changed). blobs.sha256 unchanged, every blob byte for byte (1436), no
  number of the JSON changed (1,583 numbers compared by pointer). The regression gate
  (tools/r19/gate1.txt, `--ref f124b158`): 0 differences on the 13 presented pages. The probe:
  fsl-flirt 5 of 5 pass. page_old_words.py: only package names with versions, the provenance's
  quoted record strings and "with larmorx <commit>".
- **Exports** (tools/r19/reexport.sh, an 8 GiB box, into tools/r11/site-copy; the JSON before in
  tools/r19/before/; tools/r18/textonly.py: every change a text, port, mark or presentation field
  (the 38 "not text" are table rows' `head` and the provenance's quoted strings, texts too)).
- **Gates** (tools/r19/gates.sh, an 8 GiB box, 4 workers, FSLDIR unset; tools/r19/gates.log): 1,609
  passed, 14 skipped, 0 failed; ruff, format, SPDX, tables, tables --site, reference (55, 0
  problems), summary_check (46, 0 differ), panel_runs_check (0 outside) pass. After it, round 16's
  hardening (ad37ad48): test_evidence_present.py 34 passed, ruff clean.
- **The reviewer's round 16** (reviews/round-16-s6.md) arrived during this relay: round 15
  RESOLVED, **S6 PASS** (no blocking or important finding), the clean room's exporter calling the
  replica's `present()` SOUND. Answered: its optional hardening done (ad37ad48: a refusal of the
  replica's presenter is reported without its message, with a test); its wording note taken (the
  "viewers" count of the checks is the number of viewer checks, not of viewers: this relay's
  report says "38 viewer checks"). Its two notes for the merge stay for the orchestrator (the
  documentation role's edit of afni_volreg_clean_room.py, now twice: ec3ccae3 and ad37ad48; the
  validation tests outside a workspace).
- The local server is stopped (by its PID). Screenshots shots-s7/ (top, ports, agreement, the
  first example, its matrices, dof12, the three-year-old, the two random-testing tables, speed;
  1440 and 390, light and dark; the redirect without scripts): the viewers' images are not yet
  drawn in the selector shots (lazy loading), the viewer checks load them.

## State (relay 18 done, 2026-11-13)
- Done: the reviewer's **round 15** on S5 (1 important, 5 minor: all fixed, "Review answers",
  Round 15) and **chunk S6** (AFNI 3dvolreg): "Ready for review" above. NEXT: **S7** (FSL FLIRT,
  fsl_flirt.py, 5 examples, the longest prose: 4,975 words). FLIRT has two ports (the replica in
  larmorx-nc, the clean room in larmorx); its module is readable for FSL clean-room implementers
  unless BRIEF_RESTRICTED lists it (check `cleanroom_audit/verdicts.py` line ~1009): then wording
  only. Start with the probe's dry run of fsl-flirt (probe_present.py <site> --page fsl-flirt) and
  the regression gate with `--ref f124b158 --intended <scratch>/tools/r18/intended.txt` (the 12
  intended differences on the three S5 pages: their facts and 3dUnifize's metrics note, already
  published at 5f00f86). present.lines exists for mri-hmc's line chart (S11).
- **How 3dvolreg's page is presented** (a model for FSL's pages of two ports whose replica module
  is restricted: prelude, t2smap): the clean room's module exports the page and ends with
  `present(doc)`, a delegate to the replica module's `present`; the replica module holds the
  page's own TEXTS (prepended to afni_pages'), CATEGORIES, SHOWS, the line charts (LINES:
  title, marks, labels per chart), the tables' TITLES and the rows' run names (`run_name`,
  `_rows`: the example's new comparison labels where the head is an old one, the replica's rows
  first).
- **Checks** (site at S6): build 0 problems, 50 pages; number checks on load 683/683, with every
  viewer 669/669 (32 viewers); the three S5 pages 174/174, 156/156, 203/203 and 166/166, 134/134,
  193/193; inventory against 6859f6f (scratch before-r18/, a git worktree, blobs linked; the moved
  page paired in tools/r18/before-paired; reports tools/r18/inventory-s6.md and
  inventory-s6-paired.md): every page after >= before but /tools/afni-volreg/ (the redirect),
  whose /evidence/ page holds as much or more (numbers 685 = 685, headings 14 -> 16, rows 625 ->
  630, figures 44, viewers 4, images 66, links 688 -> 704, words 34,409 -> 36,524; shared shown
  numbers changed: the 6 of the swapped rows, the same values at each other's pointers;
  0 before the swap). blobs.sha256 unchanged, every blob byte for byte. The
  regression gate (tools/r18/gate3.txt): 0 differences not intended. page_old_words.py: only
  quoted record/version strings in the provenance, the commands `larmorx afni 3dvolreg` and
  `python -m larmorx` in the timing chart's key, the memory note's quoted title.
- **Gates** (tools/r18/gates.sh, an 8 GiB box, 4 workers; tools/r18/gates.log): 1,607 passed, 14
  skipped, 1 failed under xdist and passed alone and with its file (tests/python/
  test_sdcflows_app.py::test_library_sets_choose_the_writers_lapack: it reads
  nibabel_header._LAPACK after the run, which another test on the same worker may have set;
  untouched here: for the orchestrator, a possibly order-dependent test); ruff, format, SPDX,
  tables, tables --site, reference (55, 0 problems), summary_check (0 differ), panel_runs_check
  (0 outside) all pass. Number checks after the last export: 683/683 on load, 669/669 with all 32
  viewers.
- **Exports** (tools/r18/reexport.sh, an 8 GiB box, into tools/r11/site-copy; the JSONs before in
  tools/r18/before/; tools/r18/textonly.py: every change a text, port, mark or presentation field,
  and the swapped rows).

## State (relay 17 done, 2026-11-13)
- Done: **chunk X1-X3** (present.py), the reviewer's **round 14** (3 minor, fixed: "Review
  answers", Round 14), and **chunk S5** (AFNI 3dTshift, 3dAutomask, 3dUnifize): "Ready for
  review" above. NEXT: **S6** (AFNI 3dvolreg: afni_volreg_clean_room.py exports the page;
  afni_volreg.py is restricted for clean-room implementers, wording only; its dry run already
  passes the probe; reuse afni_pages.py's TEXTS/PORTS where they fit, its replica follows
  26.3.05 ("the replica's reference"), the clean room's panels "LARMORX clean-room original").
- **Repository commits:** c1209efc, 9d43127b (X1-X3), ba86c095 (round 14), c240ea54 (S5
  groundwork: `text(port=)`, implementation(port=), speed(marks=, labels=)), c7f881eb (S5:
  evidence/afni_pages.py and the three modules' present), 30f2610c (cases' ports;
  /evidence/afni-tshift/ in tools.toml and programs.toml), f77300d4 (the gate's "presented
  after <ref>"),
  17bd7daf (the two other addresses; two more old words). `git log d38c0f74..` lists them.
- **Site commits:** e71cc6c (the three JSONs exported, summary.json), ca6c840 (3dTshift's page,
  redirect, site.yml, generated data; the JSONs again), 6859f6f (3dAutomask's and 3dUnifize's
  pages, redirects, site.yml, generated data; the JSONs again).
- **Exports** (tools/r17/reexport.sh, an 8 GiB box, into tools/r11/site-copy; the old JSONs in
  tools/r17/before/): every change a text or a presentation field (tools/r17/textonly.py);
  blobs.sha256 unchanged, every blob byte for byte (1436). 3dTshift's JSON reads its replica's
  record of 2026-11-04 (stamped `3a795ff019fb-dirty`, on main since 87e1bb8a: for the
  orchestrator): 315 -> 317 cases, 280 -> 282 bit-identical (the two glibc cases now with the
  replica's result), same 318 case ids, no verdict changed; summary.json those two counts only.
- **Checks** (site 6859f6f): build 0 problems, 49 pages; number checks on load 174/174,
  156/156, 203/203, Validation 443/443; with every viewer 166/166 (23 viewers), 134/134 (54),
  193/193 (41), 0 bad; inventory against 2365d5e (tools/r17/inventory-s5.md, before-r16 now
  checked out at 2365d5e and built): every page after >= before but the three moved
  /tools/afni-*/ (redirects), whose /evidence/ pages hold as much or more: numbers 175, 157,
  204 each; headings 11->13, 13->15, 12->14; rows 352->357, 190->198, 182->187; figures,
  viewers, images equal; links 840->852, 491->506, 490->503; words 15,183->16,451,
  12,987->14,194, 14,096->15,514. Shown numbers by pointer (tools/r17/shown_moved.py): only
  3dTshift's three record counts moved (315->317, 280->282, 84->86). page_old_words.py: left
  only quoted record/version strings and difftest.json's triage texts (chunk E).
  Screenshots: shots-s5/ (each page's top and first example, 1440/390, light/dark). Gates
  (tools/r17/gates.sh): 1,606 passed, 14 skipped, 1 failed under load and passed alone twice
  (validation/tests/test_oracle_cache.py::test_resolve_serves_a_false_alarm_again_and_logs_it,
  untouched here); ruff, format, SPDX, tables, tables --site, reference (55, 0 problems),
  summary_check (0 differ), panel_runs_check (0 outside). Regression gate `--ref d38c0f74`:
  0 differences on the nine pages presented before; the AFNI pages "presented after".
- **The probe** (tools/r17/probe-after-x.txt): 61 refused at the start -> 6 (resample-series 1,
  sdcflows-estimation 1, sdcflows-fieldmap 4: S13/S14 add those shapes).
- **Handoff begun:** <scratch>/HANDOFF.toml (24 ops for docs/next-steps.md) and HANDOFF.md.
- LESSON: `pkill -f "[b]uild.py ... --serve"` in a command whose own line also starts the
  server kills that shell (the bracket only hides pkill's own argv): start the server from a
  script (tools/r17/serve.sh), stop it by PID.

## State (relay 16 done, 2026-11-13)
- Done: **chunk R** (the renames) and the reviewer's **round 13** on S4b (3 minor, all fixed:
  "Review answers", Round 13); **chunk X begun**: its gate (X4's probe and regression gate) and
  example()'s listing dry run. X1-X3 not started.
- **Repository commits:** 4d203efd (merge of local main 8b68a74a: the audit accepts both names),
  9e1d1ff7 (R: the 26 `git mv`s; every reference that may change; the record-pair rule in
  old_words.py and the inventory made again at its own heads 6eb5f150/d9cf5fa9: only the pair
  rule's lines changed, 26 now / 7 later; the guide's §1 "Renamed files"/"Renamed identifiers"
  and §7's count; the three packages' `_RENAMED` aliases, validation/tests/
  test_renamed_modules.py; tables made again), 36cbb619 (the eight pointer files at the
  findings' old names), bc80cccf (renames.py's own exceptions), d38c0f74 (probe_present.py;
  example(errors=...) with its test; two clip limits: fsl_bet_options_clean_room 900 -> 910,
  fsl_flirt 600 -> 610, so the reasons cut short keep every character they showed with the
  two-characters-longer names). **Site commits:** 3096db2 (round 13: content/evidence/
  itk-hdf5.md), 2365d5e (R: content pages' links, bet's three reference pages exported again,
  five JSONs exported again).
- **R's tool:** validation/investigations/docs-revision/renames.py (dry run now: 0 files to
  change but docs/next-steps.md's handoff ops). **Decisions taken (for the orchestrator):**
  (1) the suites keep their names (`afni-volreg-original`, `tedana-t2smap-original`): a suite's
  name is its record's (its SUITE names the files it writes and its oracle inputs' folder), so it
  changes with its record, later, the old name an alias then (the guide's §1 says so now);
  (2) each renamed finding keeps a pointer file at its old name (docs/findings/*-original.md, 8):
  the clean rooms' code comments and PROVENANCE (17 files, 42 references), specs/flirt.md and
  topup.md (3), a record (docs/validation/mri-hmc.json), two random-testing cases and the parts'
  history (108 files) name them, and this part may edit none of these; (3) the renamed modules
  import under their old names (PEP 562 `__getattr__` in parity, evidence and difftest.tools),
  so the investigations' 20 `from ... import <old>` lines still run. **docs/next-steps.md**
  (shared): 24 `[[op]]` edits in scratch tools/r16/next-steps.toml (apply_handoff --check: ok),
  for relay M's HANDOFF.toml. **For chunk G** (a clean-room agent and a spec writer): the
  references left as they are, scratch tools/r16/kept-after.md (sections "a clean-room agent"
  and "a spec writer").
- **Exports** (tools/r16/reexport.sh: tools/r5/reexport.sh without LARMORX_FMRIPREP_RUNS, an
  8 GiB box, into tools/r11/site-copy, the old JSONs in tools/r16/before/): difftest, fsl-topup,
  afni-volreg at bc80cccf; fsl-bet and fsl-flirt again at d38c0f74 (the clip limits). Every
  change is a findings path (-original.md -> -clean-room.md), the exporter's tree hash
  (afni-volreg also its `original_exporter`), or a clipped reason that now shows 10 more
  characters (bet's examples 6, 8, 9; FLIRT's cases 2, 63, 67, 70): tools/r16/jsondiff.py.
  blobs.sha256 unchanged, every blob byte for byte (diff -rq, 1436 each). **fsl-fast's export
  was killed by the OOM killer in the 8 GiB box** (7.2 GB at 4 threads before it died): its
  JSON is unchanged and names docs/findings/fsl-fast-original.md, which the pointer file serves;
  export it in S8 (with the orchestrator's larger box if 8 GiB stays too small). summary.json
  and panel.json unchanged by the exports.
- **Checks** (site at 2365d5e): build 0 problems, 46 pages (the before build at 4f3093d, scratch
  before-r16/, reports 9 stale reference pages against today's worktree, as expected: its _site
  was still built); `reference all --site --check` 0 problems, 0 stale; tables and tables --site
  ok; summary_check 46, 0 differ; panel_runs_check 28 files, 0 runs outside the fourteen;
  inventory against 4f3093d (tools/r16/inventory-r.md): every page after >= before (fsl-bet
  words 51,016 -> 51,019, fsl-flirt 41,096 -> 41,101, itk-hdf5 headings 11 -> 12, links 292 ->
  295, words 5,956 -> 6,219), 0 shared shown numbers changed; the five JSONs changed as above.
  Numbers on load (tools/r16/checks.txt): itk-hdf5 33/33, fsl-bet 1,051/1,051 (with every viewer
  loaded 1,009/1,009, 272 viewers, 0 bad), fsl-flirt 349/349, fsl-topup 1,028/1,028, afni-volreg
  683/683, Validation 443/443. Screenshots of the ITK page's changed paragraphs (ports,
  corrections, random testing), 1440 and 390, light and dark: scratch shots-r16/. Gates (an
  8 GiB box, 4 workers, tools/r16/gates.sh with tests/python and tests/golden added, FSLDIR
  unset): 1,593 passed, 14 skipped; once 1-2 failed, then passed in two more runs:
  validation/tests/test_difftest.py::test_a_case_over_the_memory_limit_is_recorded_and_the_
  night_goes_on is load-dependent (the machine was busy with other agents' boxes); ruff, format,
  SPDX, tables, reference, summary, panel runs clean. The regression gate
  (`probe_present.py <site> --ref bc80cccf`): 0 differences on the nine presented pages.
- **The probe today** (tools/r16/probe-before-x.txt; every refusal of every example): AFNI 4
  pages (13 examples), FSL fast 6, flirt 5, maths 3, prelude 10, topup 7, mri-hmc 1, tedana,
  sdcflows, resample-series: all refused. The shapes it lists, for X1-X3: the ports' panels
  "larmorx replica", "larmorx-nc replica", "LARMORX clean-room original", "larmorx clean-room
  original", with a qualifier (", -peels 0", ", -T2up 100", ", stored LPS", "'s series,
  median"); originals "AFNI 25.2.09 in fMRIPrep's image", "AFNI in fMRIPrep's image", "AFNI
  25.2.09, 3dAutomask" (the program after the version), "AFNI 25.2.09, fMRIPrep's call",
  "mcflirt, FSL 6.0.7.17", "Initial alignment (-init)"; comparisons "original − replica (both
  default mode)", "clean-room original − replica, reproduce mode", "clean-room original, default
  − replica, default", "AFNI 26.3.05 − AFNI 25.2.09" and "AFNI 25.2.09 − 26.3.05" (the version
  step), "fMRIPrep's AFNI − the repository's AFNI build", "AFNI: -peels 0 − -peels 1" (the
  defect, in AFNI), "FSL's result − its initial alignment", "FSL with the NaN − FSL without it",
  "replica, --nthr=4 − --nthr=1" and "FSL, --nthr=4 − --nthr=1", "clean-room original, 1 thread
  − all CPUs", "mcflirt: second run − first run", "AFNI 26.3.05: stored LPS − LAS",
  "clean-room original (default) − itself on the usual encoding", "AFNI on the negative scale
  factor − AFNI on the usual encoding", an input against an input (afni-01's `inputs`), and
  "replica − fMRIPrep's own output" refused only because the probe's Page names one port (X1:
  the port from the panel).
- NEXT: **X1-X3** (present.py: the port per panel from its label, `Page.ports`, the roles
  between the two ports, the version step, the other originals' and comparisons' shapes above,
  panel_shows by role for two ports; a test per shape), each step checked with the probe (the
  refusals fall) and the regression gate (`--ref d38c0f74`: 0 differences on the nine presented
  pages, or only those listed as intended). Then S5 (AFNI) with 0 refused on its four pages.

## State (relay 15 done, 2026-11-12)
- Done: chunk S4b (ITK transform files) and the reviewer's round 12 on S4a (findings 1-4 fixed,
  5 answered: "Review answers", Round 12). **R not started** (this relay's context; the
  orchestrator's message put round 12 before it): the next relay starts with R, then X.
- **Repository commits** f31277fd, 65f88519, 96c50708, 995188ca. **Site commits** 4be03f1, 4f3093d.
- **Exports** (tools/r5/reexport.sh in an 8 GiB box into tools/r11/site-copy, the old JSON put
  back first): ITK's at f31277fd (with LARMORX_FMRIPREP_RUNS, as relay 14 said; its cases read
  only the record), DenoiseImage's and Atropos' at 96c50708 **without the variable**
  (tools/r15/reexport2.sh unsets it: fmriprep_nodes reads the fourteen runs itself). blobs.sha256
  unchanged, every blob byte for byte (diff -rq) each time; export_diff: texts and fields only.
- **Checks** (site at 4f3093d): build 0 problems, 46 pages; reference 0 problems; tables and
  tables --site ok; summary_check 46, 0 differ; panel_runs_check 28 files, 0 runs outside the
  fourteen; inventory against site 6660758 built (scratch before-r15/, a git worktree, blobs
  linked; the moved page paired in tools/r15/before-paired; reports tools/r15/inventory-s4b.md
  and inventory-r12fix.md): every page after >= before but /tools/itk-hdf5/ (the redirect,
  paired), the three JSONs exported again and DenoiseImage's words 14,178 -> 14,154 (the three
  wrong "the same call as 1 other node(s): ...-syn/..." phrases removed, finding 1);
  evidence/itk-hdf5 shown numbers 34, headings 9 -> 11, links 287 -> 292, words 5,322 -> 5,956, 0
  shown numbers changed. Numbers on load: ITK 33 of 33 (also with every viewer loaded; it has
  none), Validation 443 of 443, ImageMath 448 of 448, DenoiseImage 450 of 450, Atropos 459 of
  459 (tools/r15/checks.txt). Gates (an 8 GiB box, 4 workers, tools/r15/gates.sh, now with
  panel_runs_check): validation/tests 602 passed; ruff, format, SPDX, tables, reference, summary,
  panel runs clean. page_old_words on the ITK page: the sidebar's FSL, the footer and the
  random-testing panel's texts (chunk E: "larmorx read them ..."); its `option` check finds none
  on the nine presented pages (it finds the old ImageMath sentence on the build before).
  Screenshots 1440 and 390, light and dark: scratch shots-s4b/.
- **What changed on ITK's page** (for the reviewer): the lede in the guide's form (what the files
  are; the replica of ITK 5.4.5's reading and writing and HDF5's layout; the record's 128 of 149,
  16 refused alike, 5 expected divergences, from its summary line; the old lede's last sentence
  kept); the facts line (Ports: the replica's sources from crates/larmorx-hdf5/PROVENANCE.md:
  HDF5 1.12.1, the version ITK bundles, The HDF Group's BSD-style licence; Status with the link
  to #validated-before (transforms-hdf5 is on the frozen list); Records with the record's
  reference line; Reference: the API page); "What is compared" (#what-it-is, the old auto
  anchor) with "Its port" (the clean room of ANTs' and ITK's code planned after FreeSurfer, the
  guide's §1 table); the Python example's description on its own line (rule 11); the verdicts'
  paragraph in two sentences more; "(decided 2026-10-16)" moved from Known limits to History
  (with the record's 2026-10-21); the random-testing call names the page's JSON
  (`ev.random_testing('/tools/itk-hdf5/', 'itk-hdf5')`: the title by the guide's pattern).
- **R's plan, checked by this relay (for the next):** the inventory's table (validation/
  investigations/docs-revision/inventory.md, "Files named with original") renames
  `docs/validation/afni-volreg-original.json` and `tedana-t2smap-original.json` now and their
  `.md` later; but a record is one pair loaded by one name (`figures/records.load(repo, name)`
  reads `<name>.json` and `<name>.md`), so splitting a pair breaks every reader of the record.
  **Keep each record's pair together: both renamed later** (26 now, 7 later), and say so in the
  guide's §1 ("Renamed files") and the inventory (a regenerated table). The findings renamed now
  (for the orchestrator's audit rule, restrictions.py line 541, which exempts `-original.md`
  from the "findings-unlisted" class: add `-clean-room.md`, as the FreeSurfer rule above it
  already does): docs/findings/afni-masks, afni-volreg, fsl-bet, fsl-fast, fsl-flirt, fsl-topup,
  mri-hmc and tedana-t2smap `-original.md` -> `-clean-room.md`; later fsl-maths and fsl-prelude.
- The local server is stopped.
- NEXT: **R** (the renames; the pair rule above; the suites' old names as aliases; descriptor
  NAMEs kept; links in both repositories; tools.toml ids and the tables; the pages whose JSON or
  text names a renamed file exported again, paths only where the page has no presentation yet;
  panel_runs_check and summary_check after each export), then **X** with round 10's X1-X4
  before S5. The reviewer's round 13 covers S4b with round 12's fixes.

## State (relay 14 done, 2026-11-12)
- Done: chunk S4a (ImageMath and the filters, antsAI with its two shapes) and the reviewer's
  round 11 on S3 (finding 1, minors 2-8, the X1 guard: "Review answers", Round 11). **S4b (ITK
  transform files) not started** (this relay's context): the next relay starts there, then R,
  then X before S5.
- **Repository commits** 87d94d96, 41b25405, be72cb2f, 496c6844, f5a921c5, 16351618, 6dac5a72,
  cda2af69. **Site commits** 5f8bd6c, 5f47891, 6660758.
- **Exports** (tools/r5/reexport.sh in an 8 GiB box into scratch tools/r11/site-copy, the old
  JSON put back first, `LARMORX_FMRIPREP_RUNS=<scratch>/tools/r14/panel-runs`): ImageMath's at
  87d94d96 (export_diff: texts and the presentation's fields only; a first export without the
  fourteen runs added nine runs' nodes to 421 case descriptions and three credits: redone),
  antsAI's at cda2af69 (texts, presentation fields, the newer record's one case), antsRegistration
  at 496c6844 and Atropos at f5a921c5 ("no other change"). blobs.sha256 unchanged and every blob
  byte for byte each time. summary.json made again (`evidence summary --site --no-compute`):
  antsRegistration 576 -> 584 cases, 363 -> 371 bit-identical; antsAI 171 -> 172, 112 -> 113;
  nothing else; summary_check 46 implementations, 0 differ (at site 1bc3691: 1 differs).
- **Regression gate by hand (X4's):** N4's, DenoiseImage's, antsApplyTransforms', Atropos',
  antsRegistration's, ImageMath's and antsAI's `present` on their JSON before the revision
  (site b768293), present.py at each commit's parent and now (scratch tools/r14/gate/gate.sh):
  the same but the intended texts (round 11: antsRegistration's three template lines; Atropos'
  phantom line and priors command).
- **Checks** (site at 6660758): build 0 problems, 45 pages; `reference all --site --check` 0
  problems; `tables --check`, `tables --site --check` ok; summary_check 0 differ; inventory
  against site 1bc3691 built (scratch before-r14/, a git worktree, blobs linked; the two moved
  pages paired in tools/r14/before-paired; report tools/r14/inventory-s4.md): every page after
  >= before but the two redirects (paired), antsAI's agreement (its new category *random
  testing* inserted: the positional pointers moved, no category's counts changed, compared by
  category) and Validation's four summary numbers (finding 1: antsRegistration's and antsAI's
  cases and bit-identical counts, now as their evidence pages); evidence/ants-programs
  headings 18 -> 20, rows 1080 -> 1081, links 1290 -> 1317, words 41,473 -> 43,956;
  evidence/ants-ai shown numbers 560 -> 561, headings 13 -> 15, rows 403 -> 404, links 448 ->
  468, words 15,000 -> 16,523; Atropos and antsRegistration words only. Numbers on load:
  ImageMath 448 of 448, antsAI 559 of 559, Atropos 459 of 459, antsRegistration 1,191 of 1,191,
  N4 748 of 748, Validation 443 of 443; with every viewer loaded: ImageMath 408 of 408 (viewers
  74 of 74), antsAI 533 of 533 (20 of 20), Atropos 451 of 451 (86 of 86); 0 failed
  (tools/r14/checks.txt). Gates (an 8 GiB box, 4 workers, tools/r14/gates.sh): validation/tests
  601 passed; ruff, format, SPDX, tables, reference, summary clean. Old words in the two pages'
  visible text (page_old_words.py, now with code placeholders): none but the package in panel
  subtitles, the provenance's commits, messages the replica prints, the sidebar's FSL, the
  footer and the random-testing panel's texts (chunk E). Screenshots 1440 and 390, light and
  dark: scratch shots-s4/ (both pages' top and sections, antsAI's first example, Atropos' top
  and corrections, Validation's matrix).
- **What changed on the pages** (for the reviewer):
  - ImageMath and the filters: the lede in the guide's form (ImageMath's two counts in two
    sentences, the cause last; the other five "every compared case of their records"; sMRIPrep's
    calls; 22 corrections, from the corrections paragraph's list); facts line (Ports, Status,
    Records standard and data panel, Benchmarks, Reference: six API pages and the findings);
    "What is compared" (#what-they-are, the old auto anchor) with "Their port" (#ports); the
    example section "In sMRIPrep: the brain extraction, step by step" (#example); "Deliberate
    corrections" became "Corrections checked against known results" (#deliberate kept), its
    date to History; the data panel, cases, speed (a sentence added), corrections (ANTS-30's
    status corrected, above), random testing (`ev.random_testing(..., 'ants-programs')`: its
    title names the six programs), limits, History (2026-10-12, 10-22, 10-23, 10-26/27).
  - antsAI: the lede (the four counts as above); facts line; "What is compared"
    (#what-they-are) with "Their port"; the shell example on one line with its brackets quoted
    (zsh); "In sMRIPrep: the initial transform of the brain extraction" (#example); the blob
    alignment (#blobs) and ANTS-116's paragraph ("corrected by default"; its date to History);
    data panel, cases, speed, random testing, limits; History (2026-10-23, 10-17/24/28, 10-26).
  - The JSON: ImageMath's examples of several programs name ANTs without a program on their
    panels and comparisons, the others by the call's program; the panel examples' lines by
    `panel_shows`; ImageMath's categories by its operations; antsAI's initial transform names
    each comparison by its program's original (DIFFS) and its tables' rows (ROWS), the
    antsAffineInitializer run named by the page (its mode from the module's run: reproduce, 8
    threads); the blobs' panels "its blobs"; the titles without "larmorx".
- The local server (port 8791) is stopped at the end of this relay.
- NEXT: S4b (ITK transform files), then R, then X with round 10's X1-X4 (X1 as round 11 says:
  a hidden run's port from the `runs` map or from its own side of the label, refused otherwise;
  the guard added in 496c6844 until then) before S5.

## State (relay 13 done, 2026-11-12)
- Done: chunk S3 (antsRegistration, Atropos) and the reviewer's round 10 on S2 (all eight
  minors, "Review answers", Round 10); round 10's plan for chunk X written into the rollout plan
  (X1-X4 and the two smaller points). S4 not started (this relay's context).
- **Repository commits** f3bbcb26, 75455b5f, f91e0bf1, 4284819d, aa30e403, 411d48b3, 3719c31b.
  **Site commit** 1bc3691.
- **Exports** (tools/r5/reexport.sh in an 8 GiB box into scratch tools/r11/site-copy, the old
  JSON put back first): antsRegistration at 75455b5f, Atropos at aa30e403, DenoiseImage at
  f91e0bf1. blobs.sha256 unchanged and every blob byte for byte (diff -rq) each time.
  export_diff: Atropos only texts and the fields port, order, shows, presentation,
  category_labels, random_testing, comparison, title (the pv-phantom example's old `groups`
  replaced by `order.groups`, as words.example does); DenoiseImage exactly 4 texts (the band's
  title, the fMRIPrep example's summary, both What this shows) and the exporter's hash;
  antsRegistration texts and those fields, and the newer record's data (see Ready for review).
- **A regression gate by hand (X4's, for present.py):** N4's, DenoiseImage's and
  antsApplyTransforms' `present` run on their JSON before the revision (site b768293) with
  present.py at 90ee800b and at aa30e403: no difference (scratch tools/r13/gate/).
- **Checks** (site at 1bc3691): build 0 problems, 43 pages; `reference all --site --check` 0
  problems; `tables --check` and `tables --site --check` ok; inventory against site 2b2306e
  built (scratch before-r13/, a git worktree; the moved pages paired in tools/r13/before-paired):
  every page after >= before but antsRegistration's 16 shown numbers from the newer record
  (explained above) and the two redirects; evidence/ants-registration headings 13 -> 15, rows
  783 -> 791, links 851 -> 874, words 38,979 -> 41,433; evidence/ants-atropos headings 12 -> 14,
  links 347 -> 364, words 14,092 -> 15,216, 0 shown numbers changed; the S2 pages links +1 each
  (the agreement chart links both records); report tools/r13/inventory-s3.md. Numbers on load:
  antsRegistration 1,191 of 1,191, Atropos 459 of 459, N4 748 of 748, DenoiseImage 450 of 450,
  antsApplyTransforms 258 of 258, bet 1,051 of 1,051, Validation 443 of 443; with every viewer
  loaded: antsRegistration 1,175 of 1,175 (viewers 92 of 92), Atropos 451 of 451 (86 of 86),
  DenoiseImage 440 of 440 (49 of 49); 0 failed (tools/r13/checks.txt). Gates (an 8 GiB box, 4
  workers, tools/r13/gates.sh): validation/tests 598 passed; ruff, format, SPDX, tables,
  reference clean. Old words in the two pages' visible text and tooltips (page_old_words.py):
  none but the package in panel subtitles ("larmorx, Apache-2.0"), the provenance's "with
  larmorx `<commit>`", messages the replica prints ("larmorx: antsRegistration: ...", "larmorx:
  upstream defect ... reproduced: ...", one quoting "the oracle's build"), the sidebar's FSL and
  the random-testing panel's texts (chunk E).
- **What changed on the pages** (for the reviewer):
  - antsRegistration: the lede in the guide's form with round 10's pattern (399 of 399 and 71
    of 71 with correctly rounded maths, 355 and 16 as built: both records' summaries); facts
    line (Records: standard, full with its velocity-field cases, the corrections' effects, data
    panel with its template registrations at full size, a record the old page did not link);
    "What is compared" (#what-it-is) with "Its port" (#ports); the shell example on one line
    with its brackets quoted (zsh); the verdicts paragraph names the 93 cases rejected by both
    and the 21 expected divergences (the record); the example's heading "In sMRIPrep: a real
    T1w registered to MNI space" (#example kept); "Against the original's own variability"
    (#variability) with the dots chart titled by the guide; Corrections with the reproduce
    sentence (the 2026-10-21 decision moved to History); the data panel paragraph: the
    categories as this page has them (*data panel: fMRIPrep's calls, every third voxel*, not
    `panel-images`, which this record lacks), the full-size template registrations now from
    their own record (14 of 14 with correctly rounded maths, 2026-10-28; the old page said they
    were left for a larger machine), ITK-31 in `fix` on sMRIPrep's brain extractions (the
    panel record's fix pass: 28 cases, values changed; the register's row); History (2026-10-21,
    2026-10-27, 2026-10-28: git log 3899deb4 and the records' Generated lines).
  - Atropos: the lede (58 of 58 and 17 of 17, 27 and 3 as built, the classified image in all:
    both records; sMRIPrep's call in five runs; ITK-50 and nineteen others = ITK-51 and
    ANTS-90 to ANTS-107 in the register); "What is compared" with "Its port"; the shell example
    on one line, brackets quoted; the partial-volume section says the replica corrects it by
    default (the 2026-10-22 decision to History); Corrections with the reproduce sentence; the
    ANTS-106 table's rows "MRF 0.1: the replica, `fix`"; History (2026-10-22, 2026-10-23 the
    records' Generated date, 2026-10-26 the panel record).
  - The JSON (both pages): panels, comparisons and chain nodes named by port; What this shows
    lines (antsRegistration from each example's metrics, its field table (a voxel's
    displacement over the template's brain: 1.08, 1.49; 0.51, 1.05; 0.27, 0.92 mm) and its
    Output fact (865, 857, 988 lines); Atropos from the examples' metrics and reasons: 1.2e-07,
    307, 304, 1 of 5,898,240, 0.038; the phantom's 97.7 %, 94.0 %, 1,418, 454, 1,368, 408, 237,
    106); the run tables "every run's transform" with each run named as the panels (no bracket
    inside another); the examples' own tables name the replica; the categories named.
- The local server (port 8791) is stopped at the end of this relay.
- NEXT: S4 (ImageMath and the filters, antsAI with its two shapes, ITK transform files), then R,
  then X with round 10's X1-X4 before S5. The reviewer's round on S3 (and round 10's fixes on
  the S2 pages) beside it.

## State (relay 12 done, 2026-11-12)
- Done: main merged into the branch first (7af05bf4: main's aaaba30a, 39bc9a35, 9aecb344 and the
  FreeSurfer preparation's first half; no conflict); chunk S2 (DenoiseImage, antsApplyTransforms)
  and the reviewer's round 9 on S1 (answers: "Review answers", Round 9). S3 not started: this
  relay stopped after S2 (its context), with S3's and S4's shapes and the new chunk X planned
  above.
- **Repository commits** 7af05bf4 (merge), 31f42a05, 85f50e2d, 6cee019b, 4d2e9ec9, 16a21833,
  c2062050, 8a80d301, 90ee800b. **Site commits** 778f41b, 2b2306e.
- **Exports** (tools/r5/reexport.sh in an 8 GiB box, into scratch tools/r11/site-copy with the
  old JSON put back first; each about 2 minutes, antsApplyTransforms 25 s): N4 at 6cee019b,
  DenoiseImage at c2062050, antsApplyTransforms at 16a21833. blobs.sha256 unchanged and every
  blob byte for byte (diff -rq); export_diff.py against the JSON before the revision: only texts
  and the fields port, order, shows, presentation, category_labels, random_testing, comparison,
  title, caption, software added; "no other change" for all three. N4 against S1's JSON: 7 texts
  (the exporter, four panel Command facts without "openneuro", the tumour's reason and What this
  shows) and `software`.
- **Checks** (site at 2b2306e): build 0 problems, 41 pages; `reference all --site --check` 0
  problems, 0 stale (bet's three reference pages carry tools.toml's new hash); `tables --check`
  and `tables --site --check` ok; inventory against site 9b2fc9b built (scratch before-r12/; the
  two moved pages paired in tools/r12/before-paired): every page after >= before;
  evidence/ants-denoise headings 13 -> 15, rows 158 -> 159, links 291 -> 312, words 12,859 ->
  14,133; evidence/ants-apply-transforms headings 12 -> 14, links 911 -> 922, words 31,802 ->
  34,560; evidence/ants-n4 words 19,646 -> 19,696; 0 shown numbers changed or removed; the
  only "regressions" are /tools/ants-denoise/ and /tools/ants-apply-transforms/ (redirects now,
  paired) and the three JSON files (exported again); report tools/r12/inventory-s2.md. Number
  checks on load: DenoiseImage 450 of 450, antsApplyTransforms 258 of 258, N4 748 of 748, bet
  1,051 of 1,051, ants-programs 448 of 448, Validation 443 of 443; with every viewer loaded:
  440 of 440, 240 of 240, 724 of 724, viewers 49 of 49, 36 of 36, 76 of 76, 0 failed. Gates (an
  8 GiB box, 4 workers, tools/r12/gates.sh): ruff, format, SPDX, tables, reference clean;
  validation/tests 592 passed and 1 failed:
  test_difftest.py::test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on fails
  only in the whole suite at -n 4 inside the 8 GiB box (twice); it passes alone, with its own
  file at -n 4 (69 passed), and the suite passes without it (592). It runs a nested 2 GiB
  difftest box; it is not touched by this part (relay 11's 568 passed before main's merge
  brought new tests): **for the orchestrator** (likely the box's memory under the merged
  suite). Old words in the two pages' visible text and tooltips (page_old_words.py): none but
  the panel's provenance "with larmorx `<commit>`" (kept, as on N4), messages' "larmorx:
  DenoiseImage:" prefixes in cases, and the random-testing panel's texts from difftest.json
  ("larmorx smoothed it ...", "larmorx failed on the file's dimension", "fixed in LARMORX",
  "Tool"): chunk E. Screenshots 1440 and 390, light and dark: scratch shots-s2/ (DenoiseImage:
  top, ports, verdicts, the first example, ANTS-44, the band, the data panel, cases, speed,
  corrections, random testing, History; antsApplyTransforms: the same with the B-spline
  example; the two chains, the band and the changed What this shows lines alone; N4's top, its
  corrections paragraph and the tumour's line).
- **What changed on the pages** (for the reviewer):
  - DenoiseImage: the lede in the guide's form (the replica's numbers from both records: 40 of
    40 and 21 of 21 with correctly rounded maths, 7 and 0 as built; 85 % of a T1w's voxels at
    12 threads, from the example's JSON and the old lede); facts line (Ports: with ITK's
    AdaptiveDenoising module, as the old facts said; Status; Records; Reference = the API page);
    "What is compared" (#what-it-is) with "Its port" (#ports); the example "In fMRIPrep:
    denoising a real T1w" (#example kept), the viewer's controls sentence as on N4; ANTS-44's
    section without its date (the date to History: 2026-10-23); "Against the original's own
    variability" (#variability); Speed got a sentence (the benchmark's six inputs; the chart's
    1 and 12 threads); Corrections with the reproduce sentence as on N4; History (2026-10-12
    ANTS-43, from the JSON's ANTS-44 reason; 2026-10-21 the repeats, from the example's facts;
    2026-10-23 ANTS-44; 2026-10-26 the panel record's date). The Python example's comment on a
    line of its own (rule 11).
  - antsApplyTransforms: the lede (200 of 200 and 425 of 425 with correctly rounded maths, 185
    and 329 as built, `--float 1` 102 of 102 and 167 of 167 as built, transform outputs 34 of
    34: both records' summaries); "What is compared" (#what-it-is), "Its port" (the maths that
    differ: the old verdicts paragraph's words); "the four expected divergences" (the record's
    4); the example (#example); the B-spline section (#bspline): "LARMORX's replica of
    `antsRegistration`" where the old page said LARMORX; Speed's sentence (the benchmark's six
    jobs, ANTsPy, double precision: the JSON's note); Corrections; History (2026-10-23: B-spline
    transform files, from the example's old summary "Before A5 part 3 ...", dated by
    docs/status-log.md's A5 part 3 entry; 2026-10-26 the panel record). The shell example on one
    line (no continuation).
  - The JSON (both pages): each panel, comparison and chain node named by its port; What this
    shows lines (DenoiseImage's first from the example's metrics: 5,522 of 5,898,240 voxels by
    at most 0.0002, 271, 73, 437,915 and 2.81; ANTS-44's from its effect fact: 611,567 and
    177; antsApplyTransforms': 2,871 and 2,715 of 1,082,035 voxels by at most 6.8e-13, 0.013);
    the record's categories named (antsApplyTransforms: interpolators, transforms and their
    files, B-spline transforms, geometries, output types, time series, single precision
    (--float 1), transform outputs).
- The local server is stopped.
- NEXT: S3 (antsRegistration, Atropos: the seed-comparison shape first), then S4, R, X before S5.
  The reviewer's round on S2 beside it.

## State (relay 11 done, 2026-11-12)
- Done: the interrupted attempt's files reviewed and kept, with fixes (below); the rollout plan
  above (chunks S, R, P, E, Q, D, F, G, M; one relay each); chunk S1 finished ("Ready for
  review"). DenoiseImage not started (S2).
- **Review of the interrupted attempt** (present.py, the edits to ants_n4.py and words.py): the
  design kept (a shared presenter that reads today's label shapes; a page names only its
  program, port and What this shows lines); fixed: "the replica (the replica) at commit" (a rule
  matched its own output), a bare "larmorx" at a sentence's end left as it was, bet's "(the row
  above)" rule naming FSL on N4's page (a page's own rewordings now run first, before
  words.example), "fMRIPrep 25.2.6 fMRIPrep runs", the band's title in code spans that the
  renderer showed as plain text (it shows titles with Markdown on presented pages now), the
  random-testing field the site never read (now `random_testing.title` and `.names`, read by
  `ev.random_testing(page, tool)`), the chain (not presented before), the categories (named now,
  as on bet's page), a message's "larmorx: N4BiasFieldCorrection:" prefix (kept), the stills'
  captions; tests added (test_evidence_present.py, 6).
- **Repository commits** 829963e2, 5cf0ae67, d030a098, a89dd7af, 76222ccd. **Site commits** fafe803,
  9b2fc9b (branch docs-revision).
- **N4's JSON exported again** at a89dd7af (tools/r5/reexport.sh in an 8 GiB box, into scratch
  tools/r11/site-copy, the old JSON put back first): blobs.sha256 unchanged, every blob byte for
  byte (diff -rq), tools/r11/jsondiff.py: only texts changed and the fields port, order, shows,
  presentation, category_labels, random_testing, comparison, title, caption added; no number,
  list length or order changed.
- **Checks** (site at 9b2fc9b): build 0 problems, 39 pages (against the worktree);
  `reference all --site --check` 0 problems, 0 stale (bet's three reference pages carry
  tools.toml's new hash); `tables --check` and `tables --site --check` ok; inventory against
  site 6574906 built (scratch before-r11/; the moved page paired in tools/r11/before-paired, its
  old HTML at the new address): every page after >= before, evidence/ants-n4 rows 273 -> 274,
  links 370 -> 391, words 18,324 -> 19,646, 0 shown numbers changed or removed; Validation's links
  216 -> 217 (the status matrix links bet's moved page again); the only "regressions" are
  /tools/ants-n4/ (now a redirect, paired) and ants-n4.json (exported again, as the guide says);
  report tools/r11/inventory-s1.md. Number checks: N4 748 of 748 on load, 724 of 724 with every
  viewer loaded, viewers 76 of 76, 8 examples, 0 failed; bet 1,051 of 1,051; Validation 443 of
  443. Gates (an 8 GiB box, 4 workers, tools/r11/gates.sh): validation/tests 568 passed; ruff,
  format, SPDX clean. Old words in N4's visible text (tooltips included): none but a path
  (`<workspace>/oracles/bids-views/...` in one case's command). Screenshots 1440 and 390, light
  and dark: scratch shots-s1/ (top, ports, verdicts, the first example and its chain, ANTS-40,
  the band, the data panel, cases, speed, corrections, random testing, History; the band, chain
  and cases alone; a still without scripts).
- **What changed on N4's page** (for the reviewer): the lede in the guide's form (what it does;
  the replica's numbers from both records: 46 of 46 and 69 of 69 with correctly rounded maths, 29
  and 20 as built); the facts line (Ports, Status with the link to #validated-before, Records,
  Reference = the API page until chunk Q); "What is compared" (the old "What it is", anchor kept)
  with "Its port" (the clean room planned after FreeSurfer: the roadmap's row 5); the example's
  heading "In fMRIPrep: the final N4 of a real T1w" (#example kept); "Against the original's own
  variability" (#variability kept); Speed got a sentence (its rows and series from the JSON); the
  corrections paragraph names ANTS-36 on the tumour's panel example (the old page said only
  ANTS-40 applies to fMRIPrep's calls; the panel example's own reason says ANTS-36 and ANTS-40);
  History new (2026-10-12: the register's rows; 2026-10-23: ANTS-40's row; 2026-10-26: the panel
  record's date); the shell example's brackets quoted for zsh (`-b '[200]'`); "every tool of
  LARMORX is validated on" became "on which LARMORX's programs are validated" (not every program
  has a panel record yet).
- **Site code** (shared, all gated by the JSON's presentation): tools/evidence.py: `band` on a
  presented page (the band a wash of the original's ink, the port's two ratios in its mark,
  filled for |Δ|, open for 1 − NCC, the key in the guide's words, the title with Markdown);
  `chain` (a node's `port`: its colour and mark); `random_testing(page, tool)` (the title
  "Random testing, the replica against the original: ANTs `N4BiasFieldCorrection` 2.6.5, the
  long run of ...", the rows' and differences' implementations by their ports' names); the
  random-testing columns' explanations in the new words on every page (wording only); the status
  matrix links a page moved to /evidence/ (summary.json keeps /tools/ addresses until S14).
  static/css/evidence.css: the band, swatch and chain styles.
- **How an ANTs-like page moves** (S2-S4, and the AFNI/FSL pages with present.py's shapes):
  1. In the module: `PAGE = present.Page(software, program, version, texts=[...])` (texts: the
     page's own phrases, e.g. "(the row above)" named by what that row compares); a `SHOWS` dict
     (example id -> (What this shows, [verdict diff ids])) written from each example's own
     metrics and reasons (numbers only from the JSON); `present(doc)` calling
     `present_.example` per example (or `present_.panel_shows` for ants_panel's data-panel
     examples), `present_.implementation`, `present_.speed`, the band's title, 
     `present_.random_testing(PAGE, doc, {"port": "replica"})` and `present_.finish(PAGE, doc,
     labels)`; `export` returns `present(doc)`.
  2. Dry run first, on the published JSON: `python -P validation/investigations/docs-revision/
     present_dry_run.py <module> <site>/static/evidence/<page>.json <out>` (prints every panel,
     group, comparison, reason and fact), then `evidence_texts.py <out>` and a grep for larmorx,
     oracle, parity, (port),
     default mode, reproduce mode, FSL on a non-FSL page, "the replica (the".
  3. Commit, then export from the cache (`scripts/orchestration/boxed.sh 8 bash
     tools/r5/reexport.sh tools/r11/site-copy <page>` after putting the old JSON back into the
     copy); `cmp` blobs.sha256, `diff -rq` the blobs, `export_diff.py old new`; copy the
     JSON into the site.
  4. The page: content/evidence/<page>.md as N4's; content/tools/<page>.md a redirect;
     site.yml's entry; validation/tools.toml `page` and validation/programs.toml `evidence`;
     `python -m larmorx_validation tables --site <site>` and `reference all --site <site>`.
  5. Checks: **stop the --serve process before building** (it rebuilds on every change and a
     build beside it fails with 1,339 problems); build; inventory with the moved page paired
     (copy before-r11/_site, put the old /tools/<page>/index.html at /evidence/<page>/,
     tools/r11/before-paired is N4's); serve; the number and viewer checks (the EVAL of
     validation/investigations/difftest/site_checks.py); screenshots; the old-word scan of the
     built page (tooltips included).
  6. `ev.random_testing('/tools/<page>/', '<page>')`: the first argument is difftest.json's key,
     which stays the old address until chunk E makes the table again.
- The first set of S1's screenshots (taken while the server rebuilt beside a build) is in
  <workspace>/archive/2026-11-12-docs-revision-shots-s1-first/; shots-s1/ holds the final ones.
  The local server is stopped.
- NEXT: S2 (DenoiseImage, antsApplyTransforms); the reviewer's round on S1 beside it.

## State (relay 10 done, 2026-11-12)
- Done: the reviewer's round 8 (round-8-pilots.md: all four pilots PASS) minors 1-3; minor 4
  (the rest of Validation) left to the rollout, as briefed. Answers: "Review answers", Round 8.
- Commits. Repository: a230b2b5 (words.provenance, its test). Site (branch docs-revision):
  6574906 (bet's evidence status tag, data/verdicts.yml, fsl-bet.json exported again).
- **Evidence module** (validation/src/larmorx_validation/evidence/words.py, presentation only):
  `words.provenance(items)`, applied by `words.example` to an example's `provenance`: the items
  reworded (`reword`, Markdown kept), and a version string a figures cache entry recorded in the
  old words (`larmorx 0.0.1 (crate larmorx-fsl: the clean-room original)`, regex
  `_RECORDED_VERSION`) quoted unchanged, followed by ", as the cache entry records it"; the
  rule "the parity suite's" -> "the comparison suite's" (was "the comparison's"; the guide's
  word), which also changes example 2's summary and its first fact on bet's page (both passed
  in round 5 as "the comparison's synthetic head": now the same words as its provenance). Test:
  `test_provenance_reworded_and_recorded_versions_quoted`.
- **bet's evidence** exported again from the cache at a230b2b5 (exporter tree 67ed21b0, no
  "-dirty"; tools/r5/reexport.sh in an 8 GiB box, into scratch tools/r10/site-copy): blobs.sha256
  unchanged; fsl-bet.json differs in exactly 15 strings: 12 provenance items (7 "Runs of the
  clean room:", 11 recorded versions quoted, 1 "the comparison suite's synthetic head"), example
  2's summary and fact, the exporter's tree hash.
- **Site:** content/evidence/fsl-bet.md's facts line: the replica's *validated* in
  `status-tag status-tag--plain`; data/verdicts.yml's d13 `means`: "... recorded in the register
  of the originals' defects. LARMORX's default, fix, corrects it and warns; reproduce
  (upstream_defects="reproduce" in Python; on the command line --upstream-defects reproduce from
  the next release, LARMORX_UPSTREAM_DEFECTS=reproduce in 0.1.0rc1) gives the original's result,
  and the comparisons with the original run with it." (the reviewer's text with the environment
  variable kept and the option marked "from the next release", as the evidence page's own
  sentence does; plain text: the site shows `means` escaped, in a tooltip and on Validation).
- Checks (site at 6574906): build 0 problems, 38 pages (against the worktree); `reference all
  --site --check` 0 problems, 0 stale, programs 0 problems; bet's number check 1,051 of 1,051,
  viewers 272 of 272 (11 examples, 0 failed); words.py tests 37 passed with test_reference.py;
  ruff, format, SPDX clean. Old words in bet's evidence visible text: "clean-room original" only
  inside the 11 quoted versions, "parity" only in the module name larmorx_validation.parity.fsl_bet;
  "reproduce mode" none (tooltips included). Validation keeps one title with "parity runner" and
  "clean-room original" (its matrix: the rollout's, minor 4).
- Screenshots of the four pilots, 1440 and 390, light and dark: scratch shots-pilots/ (home,
  programs, bet-reference, bet-evidence, each at its sections' offsets, 126 files; log
  tools/r10/shots.log) and shots-pilots/changed/ (the three fixes: bet's facts line, two
  provenance blocks opened, Validation's Deliberate correction; 16 files).
- NEXT: the user's approval of the four pilots (the orchestrator brings them; tell the user that
  Validation outside #statuses and #validated-before is revised in the rollout, round 8's minor
  4); then the rollout (sub-task 4).

## State (relay 9 done, 2026-11-12)
- Done: the reviewer's round 6 on bet's reference pages (finding 11 and the minors 12-15) and,
  at the orchestrator's message, round 7's programs finding 1 with its minors 2-3 (4 needs no
  change); answers below, "Review answers", Round 6 and Round 7. The home page and bet's evidence
  page were NOT edited.
- Commits. Repository: 558e161c (round 6), 8bd11fa0 (round 7's list). Site (branch
  docs-revision): f622e8d (bet's reference pages exported again), 18afbb5 (Programs, Validation,
  data/programs.yml).
- **docs/api/fsl-bet.md** (wording, every number from fsl-bet.json / fsl-bet-replica.json, both
  linked): bet's Ports states bet -B's miss in full (largest boundary distance 2.24 voxels for the
  brain and the mask against 2, the mask's 95th percentile 1.0 against 0.5, the Dice coefficients
  0.9989 and 0.9983 within 0.995); the clean-room section's child-3y-B sentence the same; The
  records' "the other two miss one" -> "miss thresholds of the boundary distance"; the three
  replica sentences name all five programs the records hold (standard_space_roi and fslroi too);
  bet2's Ports: 378 of 380 as the record counts them (betsurf's counted apart), the two misses
  bet -B's; "the user's decision" -> "decision GLIBC-06" (History), "decided on 2026-10-31 /
  2026-11-04" (the two in the clean-room section); the example's comment on two lines.
- **python/larmorx/fsl/bet.py**: "(the clean room) failed", "is the clean room's option", "an
  UpstreamDefectWarning". The same message in the other 8 modules stays for the rollout (other
  parts' files; one change for all, as decided).
- **The exporter** (reference/page.py `_lede`, apipage.py's docstring): the lede is the first
  sentence and "What it does" leaves it out (its lines and paragraphs kept); one sentence: no
  lede, the whole text in the section. Tests: `test_lede_is_not_repeated` (3 cases),
  `test_bet_pages_say_their_first_sentence_once`.
- **The statuses' list** (reference/programs.py): `DEFINED = "2026-11-12"`, `VALIDATED_BEFORE`
  (the 37 records whose status was validated that day, frozen); each port's `earlier`;
  data/programs.yml's `validated_before` (by software, the ports named by program); a frozen
  record gone from tools.toml is a problem. Test: `test_validated_before_lists_the_earlier_ports`.
  The guide's §1 and the porting guide's Status point at the list.
- **The site**: content/programs/index.md and content/validation.md #statuses in the reviewer's
  round-7 words ("most of AFNI's" for "AFNI's": 3dvolreg's clean room is experimental), the
  Programs page links `/validation/#validated-before`, where a list by software (chips) follows,
  generated from data/programs.yml.
- Checks: build 0 problems, 38 pages (against the worktree); `reference all --site --check` 0
  problems, 0 stale, programs 0 problems; gates in an 8 GiB box, 4 workers: tests/python,
  tests/golden, validation/tests 1,517 passed, 14 skipped; without FSLDIR 956 passed; ruff,
  format, SPDX clean; `tables --check` ok. Screenshots (scratch shots-r9/): bet's header and What
  it does, Ports, Examples (1440 and 390, light and dark), bet2's header; the Programs page's
  statuses and Validation's #statuses with the list (1440 and 390, light and dark).
- NEXT: the reviewer's last round on the reference pages (round 6's answers) and on the Programs
  and Validation statuses (round 7's answers); then the user's approval of the four pilots; then
  the rollout (sub-task 4).
- For the orchestrator (found in relay 9):
  - A status set after 2026-11-12 is not checked against the definition: a part that marks a
    port `validated` without the final phase's evidence would show it validated, and the list
    would (rightly) leave it out. programs.py could refuse a validated record outside
    VALIDATED_BEFORE until the final validation phase; not done (it could stop the confounds
    part's export at the merge). The orchestrator's call.
  - The records' summary lines and bet's API page say "brains, masks and outlines" for 380 files
    that include standard_space_roi's and fslroi's outputs (about 61 of them, by the record's
    case categories): the suite's word (parity/fsl_bet.py `highlights_original`), for the
    rollout or the next record.
  - The code blocks draw `->` as an arrow (the mono font's ligature) in `# -> 'clean-room'`, as
    `.opt-help` did before relay 7: if the reviewer asks, `font-variant-ligatures: none` on
    `pre code`.

## State (relay 8 done, 2026-11-12)
- Done: the user's decisions written into the guide's §8 (five approvals, the hero line); the
  reviewer's round 5 answered, all 7 important and every minor (below, "Review answers", Round 5),
  with the orchestrator's decision on the statuses. bet's reference pages and docs/api/fsl-bet.md
  were NOT edited (round 6 runs on them); `reference all --site --check`: 0 stale, 0 problems.
- Commits. Repository: bbd8aa6e, 5492d37e, 6a880b13, 19e05fad (tree clean). Site (branch
  docs-revision): 7d86cfc, 48f5673.
- **Guide** (docs/writing-guide.md): §8 "The user's decisions" (the five, as decided; decision 1
  carries the user's line and says §2's word list does not apply to the user's own sentence);
  "question N" -> "decision N"; the header no longer a draft for approval; example 1 and the home
  outline's hero with the user's line; §1 new "The three statuses" (the orchestrator's
  definition, the pages that state it, the ports marked validated before it); §5: the pastels'
  check for any pair (>= 7 normal, >= 4 protanopia and deuteranopia), fMRIPrep's pastel never
  beside a replica's mark without its chip's text, a status neutral, the sidebar by software.
- **docs/porting-guide.md**, Status paragraph: the corrected definition (a shared document this
  part may rewrite: listed below).
- **pastels.py** (and its README): every pair printed under the four visions; a new pastel's
  pairs need >= 4 under protanopia and deuteranopia (closest: tedana/LARMORX 4.3); today's three
  pairs under 4 (ANTs/FreeSurfer, AFNI/LARMORX, FSL/Workbench) printed as information. PASS.
- **Programs data** (reference/programs.py, validation/programs.toml): `does` comes from the API
  page's reference block where one exists (its "what it does" first sentence; an entry that also
  gives `does` is a problem), so bet, bet2 and betsurf lost their `does` lines; every
  docs/api/*.md but upstream-defects.md must be some entry's `api`; a group's port carries
  `next_release` when every program's port of that kind has it (tedana's clean room). Tests: 5 new
  in validation/tests/test_reference.py.
- **Evidence module** (words.py, fsl_bet.py, fsl_bet_options_original.py; presentation only): the
  record's "D13 section" -> "(the record's list of what `fix` changed)", linked per case to its
  port's record and in the examples to the replica's standard record; "the user's decision of
  <date>" -> "as decided on <date>" (three forms); `words.link_ids` links register ids to the
  register's summary table (the What this shows lines); the random-testing tables' titles ("Random
  testing, both ports against the original: FSL `bet` 6.0.7.23 (909 random calls, 2026-11-05)",
  "The clean room's differences from the original, by category"), their note and rows without
  trial, catch-up, relay 7 or relay 10 (the run's length, 31 minutes, from the same summary); the
  case table's port headings "replica 6.0.7.17", "replica 6.0.7.23", "clean room";
  `category_labels` (fMRIPrep's calls, defects of FSL, file formats, real heads, random testing,
  standard_space_roi; the ids stay the filters); `_default_newest`: the first four examples show
  FSL 6.0.7.23 and the replica's 6.0.7.23 run by default (6.0.7.17's stills stand for them, which
  it checks: every output identical between the two versions and the replica's two runs).
- **bet's evidence** exported again from the cache at 19e05fad (exporter tree 5df9b57d; no
  computation; blobs.sha256 unchanged; only fsl-bet.json changed). Page prose: "117 MiB, against
  FSL's 103; 13 of the 117 are Python's own"; GLIBC-06 and GLIBC-07 as decisions of the project,
  linked; the two links to findings/fsl-bet.md marked "written from FSL's source: for the
  replica's authors"; "Random testing of both ports" (#trial kept); History without catch-up.
- **Site code**: tools/evidence.py (category labels in the agreement charts, the case table and
  its filter, with break opportunities after `_` on such pages only; the random-testing tables'
  titles in their letter case on a ported page), static/js/evidence.js (the filter's option text),
  static/css/evidence.css (the table title), templates/doc.html + site.css (the sidebar's evidence
  pages grouped by software under a pastel chip, no dots), site.yml (t2smap -> tedana, the two
  sdcflows pages -> sdcflows, the resampler -> fmriprep, in the Programs page's order),
  data/verdicts.yml ("Deliberate correction", without D13), content/programs/index.md,
  content/index.md, content/validation.md (#statuses new; #reading's images and variability),
  content/roadmap.md (row 6 the completeness parts; "Later": AFNI), data/status.yml (planned).
- Checks (site at 48f5673): build 0 problems, 38 pages; inventory against the site before this
  relay (scratch before-r8/, a git worktree at 7521371, built): every page after >= before but
  five, each explained (tools/r8/inventory-r8.md): four evidence pages lose only the word "(D13)"
  of the verdict's label (words only, decision 3); bet's evidence page's six "changed" and one
  "removed" shown numbers are the default stills' "Bit-identical N of N voxels", now shown under
  the 6.0.7.23 comparisons' pointers with the same values (tools/r8/shown_check.py: no number text
  lost); every other page differs from before only in the sidebar and that label (checked).
  Number checks: bet 1,051 of 1,051 (on load; 1,009 of 1,009 with every viewer loaded, as before),
  viewers 272 of 272, 11 examples, 0 failed; Validation 443 of 443. Case table 1,058 px in its
  1,058 px frame at 1440 (was 1,091). Gates in an 8 GiB box, 4 workers: tests/python, tests/golden,
  validation/tests 1,512 passed, 14 skipped; without FSLDIR 956 passed; ruff, format, SPDX clean;
  `tables --check` ok; pastels.py PASS. Old words in the three pilots' visible text: none of
  clean-room original, D13, catch-up, the user's decision, the trial, relay, exact replica,
  bit-exact, tier, oracle ("main branch" only as "the code in development (the repository's main
  branch)"). Screenshots 1440 and 390, light and dark: scratch shots-r8/after/ (home hero end,
  intro, cards, software, features, planned; Programs top, statuses, tedana, FSL head, bet's row;
  Validation #statuses, #reading; sidebar; bet's first example without scripts, agreement chart,
  random-testing tables, ports, case table) and shots-r8/before/ (the same main parts).
- NEXT: the reviewer's round 6 (bet's reference pages, running) and a round on this relay's
  answers (round 5's pages); then the user's approval of the four pilots; then the rollout.
- For the orchestrator (found in relay 8):
  - **The statuses' definition does not match every status shown**, so the pages say so: since
    2026-10-28 several parts still marked ports `validated` without the final phase's evidence
    (afni-volreg-replica 2026-11-07, niworkflows-helpers, numpy-linalg, itk-hdf5, nifti-io,
    resample-series and AFNI's three of before; with a full record: the bet and mcflirt replicas,
    sdcflows, ndimage-skimage). The pages and the porting guide state the definition, then "the
    ports marked validated before this definition keep their status: ANTs' programs with their
    data panel (2026-10-27), and the others because every case of their record passed; the final
    validation phase gives them the rest". If the orchestrator prefers the statuses to follow the
    definition, those record rows in tools.toml change (not the writer's call); PLAN.md §4's three
    lines say the old definition too (a handoff item).
  - The Validation page's status matrix still draws each tool's deep software colour as a dot
    (tools/evidence.py `matrix`, `.ev-matrix .dot`): the rollout's, with the Validation page.
  - The home page's status table links tedana-t2smap-replica.md, which tedana's clean-room
    briefs forbid (round 5, home 6): tables.py writes that link (README's table too); under the
    rule-48 decision for readable pages, the rollout could drop the link for BRIEF_RESTRICTED
    records and keep the text. The orchestrator's call.
  - bet's t1w-robust example still shows the replica's `reproduce` run against 6.0.7.17 by
    default beside the 6.0.7.23 panels (no 6.0.7.23 reproduce run exists; its still is that run's).
  - The roadmap page (maintained at each merge) gained row 6 and a "Later" paragraph: merge it
    with the orchestrator's own roadmap updates.

## State (relay 7 done, 2026-11-12)
- Done: round 4's answers (below, "Review answers", Round 4): findings 1 to 10 of
  round-4-bet-reference.md (1 blocking, 4 important, the rest minor) and the guide's minor 48 and 49, plus relay 6's §5 pastel line. bet's
  evidence page, the home page and the Programs page were NOT edited (round 5 runs on them);
  `reference all --site --check` says data/programs.yml is unchanged.
- Commits. Repository: 0a61185e (WIP: API page, bet.py, exporter), 93a0903b (tests, guide,
  verify-merge.sh, digests quoted). Site (branch docs-revision): 7521371.
- **docs/api/fsl-bet.md** in the new words: the shared Ports section (example 26's facts: replica
  translated from FSL's source in `larmorx-nc`, run as a separate program; clean room from
  specs/bet.md, the paper and runs, in `larmorx`, Apache-2.0; auto, the values, the command line
  from the next release with 0.1.0rc1's `LARMORX_IMPLEMENTATION=original`), no link into
  crates-nc (the licensing page instead); each program a "What it does" that says what it does
  (the reviewer's texts; bet2's "deforms a tessellated sphere" from bet.py's docstring) and a
  `### Ports of <program>` (one or two sentences per port, from the records and the page's own
  clean-room section); the two Status paragraphs moved, every fact kept, into `## The records`
  (shared, linked from the reference pages' "More in the repository"), part 2a/2b's records named
  "the first record" and "the second record (2026-10-31)"; every date of the exported parts and of
  the Status paragraphs into a new `## History`; "Upstream defects (D13)" -> "Defects of the
  original, corrected by default"; "The clean-room original" -> "The clean room"; every
  "clean-room original" and "the original" meaning the clean room reworded in the whole page;
  "standard tier" -> standard comparison, "trial" -> run, "(observed)" -> "as runs of FSL show" or
  dropped (`-c` row), "(CLAUDE.md rule 6)" dropped; the `--tanh` rows reworded (GLIBC-06 named,
  its date to History, the findings linked: fsl-bet-original.md, readable). Old text:
  scratch tools/fsl-bet-api.before-r7.md.
- **python/larmorx/fsl/bet.py** (docstrings and comments only; old copy tools/bet.py.before-r7):
  ports, clean room, no D13 or CLAUDE.md; BetResult.mesh holds the dropped "Returns a BetResult"
  paragraph's mesh facts (vertices (n, 3) in FSL's millimetre coordinates, triangles (m, 3), from
  the Surface docstring); BetsurfResult's fields say "on the T1's grid" and their options. The
  printed messages "(the clean-room original) failed" (9 modules) and bet's tanh refusal are
  LEFT for the rollout (one change for all modules; no test matches them).
- **The exporter** (validation/src/larmorx_validation/reference/): code.py records the spellings
  the parser accepts (`CommandLine.parsed`: specs and match arms); page.py: a usage-text spelling
  the parser does not accept is a problem unless the reference block lists it `usage-only`
  (bet's `-h`, which bet.rs's fallback prints the usage for), a stale usage-only too; a link to
  restricted material on a reference page is a problem (`restricted()`: the audit's ANCHORS and
  SOURCE_FINDINGS, verdicts.BRIEF_RESTRICTED/BRIEF_FILES, difftest cases); `next_release()` marks
  LARMORX's own options "From the next release" while CHANGELOG.md's Unreleased holds the synopsis
  `larmorx [--n-threads` and no released section does (under the Command line's synopsis and in
  the own options' section; each row "The variable X in 0.1.0rc1"; after the release: "Replaces X,
  which keeps working"); the n_threads row gives the command line's default (every logical CPU,
  `0`) beside Python's; a program's `### Ports ...` section goes under the page's Ports heading
  before the shared text; synopses are ```` ```text synopsis ````; the front matter names
  `software` and quotes the digests; sources add defects.rs and CHANGELOG.md; the Evidence line
  says "measured against FSL". apipage.py's docstring documents usage-only, the Ports section and
  `# ->`.
- **Tests** (validation/tests/test_reference.py, 22 pass): a Python example runs statement by
  statement, `# -> <repr>` after an expression is checked, any other comment after code fails;
  bet's example: a comment line says which port ran (the clean room of larmorx, or the replica
  once larmorx-nc is installed), then the same call with implementation="clean-room" and
  `c.mask.implementation.kind  # -> 'clean-room'`; DRIFTS (7) mutate a copy of every file the
  exporter reads (`_mirror`): the old four, the parser dropping `-t`, a usage-only `-m`, a link
  into crates-nc; the mirror unmutated passes; 5 cases of the output check; next_release before
  and after a release.
- **verify-merge.sh** (the orchestrator's script, changed here: list below): with `--site`, STEP5
  reference runs `python -m larmorx_validation reference all --site <site> --check` (in the
  VERDICT).
- **The site**: tools/build.py: `check_references` (with the main repository at hand, every
  reference page's `sources` hashed again; a changed or missing file is a problem: shown to fire
  9 times against the main checkout, 0 against the worktree); a ```` ```text synopsis ```` block
  has no Copy button; templates/reference.html: the original with its square (`.port-mark`), the
  status neutral (`.status-tag--plain`, the dashed experimental kept), "every comparison with
  FSL" (`software`), "made from docs/api/fsl-bet.md and the code"; site.css: those two classes
  and `.opt-help` without Inter's contextual alternates (`->` stayed `->` in the HTML; the font drew
  the arrow).
- Checks: build 0 problems, 38 pages (against the worktree); `reference all --site --check` 0
  problems, 0 stale, programs 0 problems; gates in an 8 GiB box, 4 workers: tests/python,
  tests/golden, validation/tests 1,507 passed, 14 skipped; without FSLDIR 956 passed; ruff,
  format, SPDX clean. Old words on the three pages: none of "clean-room original", bit-exact,
  D13, tier, oracle, (observed), CLAUDE.md, part 2a/2b; every "original" left names FSL's
  program or the value. Screenshots (1440 and 390, light and dark): scratch shots-r7/after/ (the
  header of bet, bet2, betsurf; bet's command line and note, options, own options, ports).
- NEXT: the reviewer's round 6 on bet's reference pages (after round 5); then the user's approval
  of the pilots; then the rollout (sub-task 4).
- For the rollout and the orchestrator (found in relay 7):
  - docs/orchestration.md (through the handoff, the merge relay): the merge's site build runs
    with `--main-repo <merged checkout>` and now fails on a reference page whose sources changed
    in the merge (tools.toml is also the confounds part's: re-export with `reference all --site`
    after merging both); `verify-merge.sh --site` runs STEP5 reference, which fails for a site
    branch whose reference pages are missing or stale (any part merged after this one: merge the
    site's main first).
  - API pages (readable) still link restricted files (fsl-bet.md: findings/fsl-bet.md in its
    History, crates-nc/larmorx-nc-core/PROVENANCE.md in its Rust section); the rule of finding
    48 is written for reference pages, whose exporter refuses such links; whether API pages follow
    it is a rollout question for the orchestrator.
  - The Python wrappers' printed messages: "(the clean-room original) failed" in bet.py, fast.py,
    maths.py, stats.py, prelude.py, topup.py, _nipype.py, tedana/t2smap.py, and afni/volreg.py's
    "(clean-room original)"; bet.py's "tanh='correctly-rounded' is the clean-room original's
    option": writer-editable (not clean-room code), one change for all in the rollout.

## State (relay 6 done, 2026-11-12)
- Done: sub-task 3's three pilots beside bet's reference pages: bet's evidence prose, the home
  page, the Programs page. The guide was NOT edited (round 4 ran beside); one change it may need
  is proposed below ("For the guide"). Next, as the orchestrator wrote after round 4: **relay 7**
  answers round 4 (the guide's minor findings 48 and 49; bet's reference pages: the blocking
  old words in docs/api/fsl-bet.md and bet.py's docstring, the four important, the six minor;
  re-export the three reference pages after); the reviewer's round 5 covers relay 6's three
  pages (bet's evidence prose, the home page, the Programs page); then the user's approval of
  the pilots, then the rollout (sub-task 4). Relay 7 may also add the pastel line below to the
  guide's §5 if the orchestrator agrees.
- Commits. Repository: a75f11dc, 27c64564, 627c83a6. Site (branch docs-revision): f942f66,
  a1a7f47.
- **bet's evidence page** (content/evidence/fsl-bet.md, prose only; the JSON is relay 5's): the
  lede = guide example 13 (one change: "the last needs fslpy's conversion of a file to another
  type, which the replica does not replicate yet", where the example said "an FSL program": the
  page's own fact is fslpy's imcp conversion); facts line: Ports (the two, with marks, packages,
  licences, how each was written), Status (example 14), Records (the replica's standard and full
  comparisons, the clean room's record, benchmark), Reference (bet, bet2, betsurf, the API pages);
  "What is compared" with the old anchor #what-it-is, then "Its two ports" (#ports); the
  replica's verdicts before the clean room's (one order); fix/reproduce, the next release's
  --implementation and --upstream-defects beside 0.1.0rc1's variables; "the cross-platform
  checks"; long paragraphs split, lists for the -S/-B/-A items, the corrections and the limits;
  every date moved to a new History section (#history): 2026-10-29, -30, -31, 2026-11-03, -04,
  -05. Old text kept in scratch tools/fsl-bet.before-r6.md for the reviewer's fact check.
- **Programs page** (/programs/, content/programs/index.md, wide): intro (guide example 9 plus
  the facts of /tools/'s intro), the two words and the statuses in one paragraph (PLAN.md §4's
  definitions), open/close-all buttons (shown with scripts only), one toggle per software
  (<details>, its name an h2 chip on the pastel, the count, "compared with <original>", its ports
  with packages and licences), inside: each program with what it does, its commands and Python
  name, its ports with status ("from the next release" where 0.1.0rc1 lacks it), Reference (or
  the API page until the reference exists) and Evidence; FreeSurfer and Workbench as planned;
  a closing paragraph (mcflirt's two commands, t2smap's clean room, the confounds). A link to
  #<group> or #<group>-<program> opens its toggle (site.js). Its data: data/programs.yml,
  GENERATED by `python -m larmorx_validation reference programs --site S` (also part of
  `reference all`) from validation/programs.toml (55 entries: 46 commands, mri hmc as mcflirt's
  `also`, and 10 modules; groups with summaries) + the replica registry's ports + tools.toml's
  record statuses (a module without a record: its API page's "**Status:**"). Checks: every
  command of FAMILIES once, ports = the registry's, records/API pages/Python names exist, the
  evidence page exists in the site. Tests: validation/tests/test_reference.py (2 new).
- /tools/ is now a redirect to /programs/ (anchor reading -> /validation/#reading); its "How to
  read the figures" bullets moved into Validation's "How to read the evidence" (Agreement, the
  original's own variability, Images, Speed) and Validation's first line links Programs. Stale
  facts corrected there, not only reworded (for the reviewer): "Brain slices only" (whole volumes
  and full-resolution slices since part 2b), "OpenNeuro datasets in the public domain (PDDL-1.0,
  CC0-1.0)" (now "each credited", site CLAUDE.md: CC-BY datasets too), "N4 and 3dTshift ... the
  other pages follow; until then they show the figures below" (every page is evidence since
  2026-10-21; dropped as history).
- **Home page** (content/index.md, guide §6's outline): 1 hero as it was (tagline kept; closing
  text = example 1, its kicker "The animation"; the description's "ecosystem" -> software);
  2 the lead (example 2) and a note (port, six platforms, early development, rc1 a pre-release,
  "from the next release", the old Status margin note); 3 #what-it-is: Original, Replica (ex. 4
  plus the old card's facts: separate programs, AFNI's CC BY 4.0), Clean room (ex. 5), bet
  through the three cards and a margin note (fMRIPrep's call, both ports every voxel, link to
  #example), ex. 6 below with Programs and licensing links (Concepts does not exist yet); 4
  #software: one card per group from data/programs.yml (pastel band, summary, ports, count link
  to /programs/#id; FreeSurfer, Workbench planned); 5 #features: Command line, Python,
  LARMORX's options (next release; rc1's OMP_NUM_THREADS, LARMORX_IMPLEMENTATION,
  LARMORX_UPSTREAM_DEFECTS), Rust, Platforms, Validation; the examples and the note as they were;
  6 #status: lede, the In the repository note, the status table grouped by software (pastel chip
  rows; "How" -> "Ports"), Planned (status.yml's hand-written list rewritten after the roadmap,
  chips for the software), the install note; 7 #philosophy "The name": example 7, equation, text
  (ex. 8), FAIR list, FAIR4RS; 8 the closing with Validation and Installation buttons. Nav: What
  it is, How to use it, Status, The name, Install, Validation, Programs, Roadmap.
- **tools.toml** (tool rows only, never the record rows): "port" -> replica, "own
  implementation" -> LARMORX's own, "clean-room original" -> clean room, "exact replica" ->
  replica, "original:" -> "clean room:", 5 "the original's" meaning the clean room, "oracle
  runs" -> fMRIPrep's runs, "full tier" -> full comparison; the replica's part first in every
  how and validation text (script: split at "; replica in"/"; replica: "; FLIRT's "both:" clause
  kept last); eco of tedana-t2smap, resample-series, sdcflows-* to their software; BET's page to
  /evidence/fsl-bet/. `tables` regenerated README.md's tool table and the site's status.yml
  (docs/validation/README.md unchanged). bet/bet2/betsurf reference pages re-exported (only
  tools.toml's hash in their front matter).
- **Pastels** (data/ecosystems.yml, site.css tokens and .eco-* classes): tedana #bab967 (deep
  #868437), sdcflows #93e3b1 (#3c9061), fMRIPrep #85dcfc (#268bab), appended after LARMORX (the
  hero keeps its five). Chosen by search within the existing pastels' lightness and chroma
  (L 0.77-0.85, C 0.065-0.105) and checked by validation/investigations/docs-revision/pastels.py:
  neighbours fsl/tedana 12.0 normal, 12.3/11.5 protan/deutan; tedana/sdcflows 11.9, 12.5/9.8;
  sdcflows/fmriprep 11.2, 11.0/10.9; fmriprep/larmorx 19.0, 15.9/15.2; any two >= 7.3; ink on
  each >= 9:1; hues >= 39 deg from the ports'. Swatches: scratch tools/r6/swatch.png.
- Site-wide: the sidebar's group label "Tools" -> "Evidence" (templates/doc.html), the header's
  nav (above), site.yml's description without "established"; .port--own (the clean room's colour,
  a circle) for LARMORX's own.
- Checks (site at a1a7f47): build 0 problems, 38 pages; inventory against the site before this
  relay (scratch before-r6/, a git worktree at fe33cb4, built, blobs linked): every page after >=
  before but /tools/, now a redirect, paired with /programs/ (headings 2 -> 10, links 103 -> 190,
  words 1,258 -> 2,257) and /validation/#reading (words +209); evidence files 29 byte-identical
  (blobs.sha256 untouched); bet's number check 1,051 of 1,051, viewers 272 of 272 (11 examples,
  0 failed), Validation's numbers 443 of 443; `reference all --site --check` 0 stale, 0 problems;
  `tables --check` ok; validation/tests 540 passed + the known memory-limit flake (passes alone);
  ruff, format, SPDX clean. Inventory: scratch tools/r6/inventory-r6.md. Screenshots, 1440 and 390,
  light and dark, before and after: scratch shots-r6/final/ (home hero, its end, its body;
  Programs open at #fsl and closed, /tools/ before; bet's head, facts, What is compared, History;
  Validation's #reading).
- For the guide (proposed, not edited): §5 says the new pastels are "checked as the verdicts'
  colours were"; pastels cannot meet the verdicts' thresholds (>= 18 and >= 11): today's AFNI/FSL
  neighbours are 13.6/10.3, FreeSurfer/Workbench 9.0/5.9. Proposed line: "checked with the
  verdicts' measures, to the thresholds pastels allow: neighbours >= 10 normal and >= 9 under
  protanopia and deuteranopia, any two >= 7 (pastels.py)".
- Facts found (for the rollout): API pages' status lines disagree with their records for
  niworkflows-helpers, ndimage-filters, skimage (API "experimental", tools.toml "validated");
  confounds and fmriprep-fieldmap-steps have no record in tools.toml (status from the API page);
  CHANGELOG's Unreleased does not list lx.confounds (the confounds part's handoff); the home
  page's examples are not run by a test (the reference pages' are); the site's CLAUDE.md names
  five software pastels (now eight + LARMORX); tools.toml is also edited by the confounds part
  (rows added at its handoff): merge both and run `tables` again.
- Shared documents changed directly (for the orchestrator): README.md (generated tool table
  only), site.yml (nav, docs list, description), content/validation.md (#reading's first line
  and four bullets), data/status.yml (generated block, the hand-written planned list and its
  header comment), data/ecosystems.yml (three entries), templates/doc.html and home.html (one
  label each), static/css/site.css, static/js/site.js (the Programs page's toggles).
- Readable files changed (the clean-room rule): all wording or facts from readable files:
  bet's evidence page (its own facts; the next release's options from bet's reference page and
  CHANGELOG), the home page (the old page, the roadmap, CHANGELOG, bet's evidence JSON's "What
  this shows", bet's reference page's options table), the Programs page and
  validation/programs.toml (each sentence from its program's API page; statuses PLAN.md §4),
  content/validation.md (/tools/'s text and the site's CLAUDE.md), data/status.yml's planned
  list (the old list, the roadmap, CLAUDE.md's "0.1.0"), validation/tools.toml and README.md
  (wording, order, software).

## State (relay 5 done, 2026-11-12)
- Round 3 answered (below, "Review answers", Round 3): guide 6eb5f150, ea1265b6 (inventory made
  again at 6eb5f150), 53699e09.
- 4b done: bet's evidence page in the guide's layout, through the evidence module and a re-export
  from the cache (no computation; every blob byte for byte, blobs.sha256 unchanged). Repository:
  504d47f6, 3c2e3a8d (+ the site commit, below). What it does:
  - validation/src/larmorx_validation/evidence/words.py: `words.example(ex, ports=, panels=,
    diffs=, groups=, shows=, verdicts=, rank=)` gives each panel and comparison its `port`
    (input, original, fmriprep, replica, clean-room), new labels and role labels (guide §4),
    `ex.order` (panels, diffs, groups: the order shown; the JSON lists keep their order, because
    the site's number check and the merge's inventory address numbers by position), the default
    view's lists reordered (ids only: the posters stay), `ex.shows` (the line and one verdict per
    port: each port's main comparison against the newest original); `words.reword` (the vetted
    old-to-new phrases, with and without code spans); `words.cases`, `words.table`; a page that
    went through it carries `"presentation": "writing-guide"`. Test:
    validation/tests/test_evidence_words.py.
  - fsl_bet.py: `present(doc)` at the end of `export`: the rules that name bet's panels
    ("FSL bet 6.0.7.17", "FSL bet, fMRIPrep's build", "fMRIPrep's output", "replica"/"clean room"
    with "larmorx-nc, FSL Licence · fix (the default) · against FSL bet 6.0.7.23, which it
    follows"), the comparisons (A − B, the port first; role labels by the ports of A and B), the
    groups (the original against itself; the replica against ...; the clean room against ...),
    `SHOWS` (11 lines, every number from the example's own reasons and metrics), the agreement
    charts' `title`/`comparison`/`port`, the speed chart's `title`/`caption`/marks
    (clean-room, clean-room-open), the triage table's "the original" = the clean room.
  - Site (tools/evidence.py, static/js/evidence.js, static/css/evidence.css, site.css's
    --port-input): all gated by the JSON's `presentation`, so no other page changes (checked:
    every built index.html but bet's evidence and reference pages byte for byte). Panels and
    comparisons get `port-*` classes, the port's caption ("Replica", "Clean room", "Original",
    "Input", "fMRIPrep's output") with its mark (.pmark: square, triangle, circle) and colour
    (--port-* tokens); the "What this shows" box with each port's verdict chip; the metrics
    table in the guide's groups with a mark per row; the agreement title "Agreement, clean room
    against the original: FSL `bet` 6.0.7.23 (standard comparison, 252 cases of the record), by
    category and verdict", not in capitals (program names keep their case); the speed chart's
    title, caption, squares/triangles/circles, open at 24 threads; the explorer's columns with
    marks; the viewer's chooser and A/B selects in the guide's order; five panels in one row at
    1440 (two columns on phones).
- Commits: repository 6eb5f150, ea1265b6, 53699e09 (round 3), 504d47f6, 3c2e3a8d (4b; tree
  clean); site fe33cb4 (branch docs-revision; the JSON exported at 3c2e3a8d, exporter tree
  3e54708a, no "-dirty").
- Checks (site at fe33cb4): build 0 problems, 37 pages; the inventory against the site's head
  before this relay (scratch before-r5/, a git worktree of the site clone at d9cf5fa, built):
  every page after >= before (evidence/fsl-bet: links 1100 -> 1122, words 48,465 -> 50,215, all
  else equal; programs/fsl/bet: one link more), 0 shared numbers changed, 1 evidence file changed
  (fsl-bet.json, the re-export), blobs.sha256 unchanged (1,436 blobs); the number check 1,051 of
  1,051 agree, the viewers' check 272 of 272, no script error, no failed viewer;
  `reference docs/api/fsl-bet.md --check`: 0 stale. Tests: validation/tests 538 passed in an
  8 GiB box with -n 4, the one failure test_difftest's memory-limit case, which passes alone (the
  flake relay 4a saw). Screenshots, 1440 and 390, light and dark, after and before:
  scratchpad docs-revision/shots-r5/final/ (worked example head and viewer, B-child's head and
  metrics table, the clean room's agreement chart, speed); inventory: tools/r5/inventory-r5.md.
- Tools: tools/r5/reexport.sh <site copy> <tool> (the export from the cache, 4 threads; about
  4 minutes for bet; run it in boxed.sh 8, into a copy of the site, compare blobs.sha256, then copy
  the JSON); tools/r5/try_present.py (the presentation applied to a published JSON, a dry run);
  tools/r5/changed_texts.py (every reworded text, before and after).
- Reference pages: a reference block may name `evidence-sections` (anchor: title); bet's names
  `data`, so bet's reference page links /evidence/fsl-bet/#data (the guide's §1 rule).
- NEXT (the evidence pilot's prose, sub-task 3, then round 4): the page's markdown in the new
  words (content/evidence/fsl-bet.md: the lede per guide example 13, the facts line (example 14),
  "What it is" -> "What is compared" with the old anchor, the prose of every section, a History
  section with the dates; every fact kept, rule 13); the verdict name "Deliberate correction
  (D13)" is data/verdicts.yml's (shared by every page: the rollout, guide example 18); "D13" is
  left only inside the quoted name of the record's list, “D13 fix pass”. Then the reviewer's
  round 4 (the guide and the bet pilots). The guide changed twice in relay 5: round 3's answers
  (6eb5f150, ea1265b6) and, with 4b, section 4's worked example (one verdict per port beside the
  line) and section 1's re-export bullet (the added fields; the JSON lists keep their order),
  504d47f6: round 4 should see both.
- Found in relay 5: the re-export from the cache reproduces the published fsl-bet.json byte for
  byte but the exporter's tree hash (so a re-export is a safe path for the rollout's texts); two
  of bet's reasons pointed at "the row above", which was not above even in the old order: now
  named ("the row of `reproduce` against FSL in this table"); program names in the old chart
  titles' capitals would read BET for `bet`, so the new titles are not set in capitals.

## State (relay 4a done, 2026-11-12)
- Relay 4 split sub-task 2 in two. **4a (done)**: the reference-page exporter, its template, the
  examples' test, the builder's redirect page type and the 404 page's address list, proved on
  bet. **4b (next, unless round 3 comes first: see below)**: the evidence page layout on bet's
  evidence page (now content/evidence/fsl-bet.md): every figure titled by what it compares
  (guide §4's pattern, its table of today's 854 role_label values), the ports side by side in
  each worked example in the guide's panel order, a "What this shows" line with its verdict,
  the port colours from the new tokens (--port-original/-replica/-clean-room, in site.css since
  4a) instead of PAL and --deep-larmorx, the variability band as the original's ink wash.
- The order of the next relays: the orchestrator's note of round 3 says "the relay after relay
  4" answers round 3 (findings 44-47, the guide only). So: relay 5 = round 3's answers (guide),
  then 4b, unless the orchestrator says otherwise.
- The guide was not edited in relay 4 (round 3 ran). Facts for round 3's relay, from 4a:
  (1) the guide §6 row "A tool's page: 'The standard images the variations read' (`#data`) ->
  the reference page of bet" conflicts with the clean-room readers' rule: the reference page is
  generated from docs/api/fsl-bet.md, a readable file, and the #data paragraph's facts (FSL's
  fsl-data_standard 2208.0, 1,420,897 bytes) come from the site page, which cites
  findings/fsl-bet.md (restricted); moving it into the API page would add facts to a readable
  document. Proposal: #data stays on the evidence page (the redirect maps it there by default);
  the reference page links to it. (2) The exporter reads only readable sources (below), so the
  "one source" bullet of §6 can say so.

## Done in relay 4a (commits)
Repository (worktree branch):
- fd06e351: docs/api/fsl-bet.md in one section per program (`## \`bet\``, `## \`bet2\``,
  `## \`betsurf\``), each with a reference block (an HTML comment: program, group, original,
  python, command-line, records, evidence), `### Python (...)`, `### Options of ...`,
  `### Examples of ...` (code blocks `python example` / `bash example`, after
  `<!-- inputs: name=kind -->`); a `## Ports` section with the two ports' paragraphs (the clean
  room's Status paragraph moved up from "The clean-room original"); every line of the old page
  kept (checked: only the removed headings "Quick start", "Option mapping ...", "`bet`'s
  variations" and the two signature headings, now code lines, are gone). Per-program tables:
  bet's from the code (bet takes `-r`, `-m`, ... without their long forms; `-h`; rows for
  `smoothness`, `head_ct` (bet2's) and `fsl_data`); bet2's adds `--binaryMask <image>` and
  `-h`, `--help` (both in bet2's code, missing from the old table).
- 6185098f, c739c2d9, 98aa5258: validation/src/larmorx_validation/reference/ (apipage.py: the API page's
  sections, tables, examples; code.py: the options a Rust command line accepts (its USAGE
  constant, its SPECS table, its parser's match arms), the Python function from its source
  (ast: signature, numpydoc, the result dataclass, Literal aliases), LARMORX's three options
  (larmorx_core::cli::OPTIONS, the variables they replace), the ports from the replica
  registry (crates/larmorx-cli/src/replica.rs); page.py: the checks and the site page;
  inputs.py: the synthetic inputs `head`, `head4d`, `matrix`); `python -m larmorx_validation
  reference <api-page>|all [--site S] [--check]`; validation/tests/test_reference.py (the
  checks for every API page with reference blocks, a determinism check, every example run in
  a new folder: Python blocks exec'd, shell lines through `python -m larmorx`; skipped without
  _core). The checks were shown to fail on six deliberate drifts (a missing row, an option the
  program does not accept, a Python name the function lacks, a missing argument, a stale
  signature; four of them kept as tests, 98aa5258). 10 tests pass in 5 s; validation/tests 531 passed (one run with -x in an 8 GiB
  box failed test_difftest's memory-limit case once; it passes alone and in the next full run:
  a flake under load, not this part's).
Site (branch docs-revision):
- d9cf5fa: templates/reference.html (the header's facts: original, command line, Python,
  ports with colour + mark + name + status, evidence, source), doc.html's header in a block;
  tools/build.py: the redirect page type (front matter `redirect:` and `anchors:`;
  templates/redirect.html: its script keeps the anchor or follows the anchors' table, a link
  for readers without scripts, noindex, canonical to the new address, left out of the
  sitemap; a link to an anchor of a redirect page is checked on the page it lands on; a
  redirect to a missing page is a problem), the 404 page's address list and its script (an
  address in another letter case goes to the one that matches in lower case, anchor and query
  kept); content/programs/fsl/{bet,bet2,betsurf}.md (exported); bet's evidence moved to
  content/evidence/fsl-bet.md (byte-identical body; only its kicker "Evidence · FSL"; built
  HTML differs from the old page only in canonical, og:url, the sidebar's current mark and the
  kicker), content/tools/fsl-bet.md a redirect to it; site.yml's BET entry to the new address;
  site.css: the port tokens (guide §5 values, light and dark), the reference page's styles
  (code in tables never breaks inside a word; on phones each table row stacks).
- Checked: build 0 problems, 37 pages (33 + 3 reference + the moved evidence page); in Chrome:
  /tools/fsl-bet/#speed -> /evidence/fsl-bet/#speed, /tools/fsl-bet/#options -> the same
  anchor, /programs/FSL/BET/#options -> /programs/fsl/bet/#options, /Programs/fsl/betsurf/
  -> the right page; the builder's problems on a probe with a missing anchor through a
  redirect, a moved anchor to a missing id and a redirect to a missing page (probe removed).
  Screenshots (1440 and 390, light and dark): scratchpad docs-revision/shots-r4b..r4f.

## Facts found in relay 4a (for 4b and the rollout)
- The exporter reads only files a clean-room implementer may read: docs/api/<page>.md, the
  main package's Python (python/larmorx), the clean rooms' and larmorx-cli's command-line code,
  larmorx-core's cli.rs and defects.rs, validation/tools.toml. Never crates-nc, crates-gpl or
  the replica crates. A reference page is therefore readable material made from readable
  sources.
- The API page format the exporter reads is in apipage.py's docstring; the rollout restructures
  every API page that way (one `##` section per program with a reference block; GitHub shows
  none of the comments). `{#id}` is not used in API pages (GitHub would show it).
- Command-line readers exist for the FSL clean rooms' forms (USAGE, SPECS, match arms). ANTs,
  AFNI, tedana, sdcflows and `mri hmc` need their own readers in the rollout (look at each
  family's CLI code first); a program the replica registry does not list (ANTs) takes its
  ports from its reference block (`ports: [replica]`).
- Python docstrings use the old words ("clean-room original", e.g. bet's `implementation`) and
  flow into the reference pages: the rollout rewords the main package's Python docstrings
  (python/larmorx/..., not clean-room crates) or the pages keep the old words.
- The generated pages carry each source's SHA-256 prefix (deterministic; no commit). The
  merge's checks should run `python -m larmorx_validation reference all --site <site> --check`
  (fails on a stale page) beside the site's build: for docs/orchestration.md through the
  handoff.
- The examples test is in validation/tests (the gates run it; CI does not: no CI minutes).
  Moving it to tests/python would run it on six platforms: about 4 s per API page like bet's.
- The merge's site inventory compares pages by address: bet's evidence page moved from
  /tools/fsl-bet/ to /evidence/fsl-bet/ (the merge relay pairs them).
- Harness quirk (again): `rm -f $VAR/*.png` is refused; use a literal path or a new folder.

## State (relay 3 done, 2026-11-12)
- Sub-task 1, the writing guide: revised after the reviewer's round 2 (every finding fixed or
  answered below, "Review answers", Round 2); committed cb45ecb4 and 47d7cf02 (the inventory made
  again at cb45ecb4). It waits for the reviewer's round 3, then the user's approval (with the four
  pilots).
- Next relay: if <scratch>/reviews/round-3-*.md has arrived, answer it first (fix the guide, or
  explain under "Review answers"); otherwise sub-task 2, the page templates and the generated
  option tables (BRIEF.md), with the guide's section 6 decisions (one source = docs/api/ + an
  exporter in this repository beside the evidence module; one reference page per program
  section of an API page; a redirect page type in the site's builder that maps each old anchor;
  the 404 page's list of addresses for another letter case; the addresses and groups in
  section 6).

## Done (with commits)
- 47d7cf02: the inventory made again at cb45ecb4 (writer 361 files to change, 1,998 "clean-room
  original", 193 "original" for a clean room, 228 "exact replica", 4,733 "original" to read;
  generated 64 files: the exported evidence 28, records' and reports' data 4, records' pages 21,
  benchmark reports 10, the clean-room audit's page 1); the guide's section 7 with it; the README
  of the investigation says how MADE_PAGES tells the pages made by a measurement apart.
- cb45ecb4: docs/writing-guide.md after round 2 (findings 32-43, below) and old_words.py's
  MADE_PAGES: a page of docs/validation/ whose "Generated" line names a `python -m
  larmorx_validation` command is a record (generated), clean-room-audit.md its own generated area,
  every docs/benchmarks/*.md but README a report (generated); the other docs/validation pages
  (README, golden, difftest, oracle-runs, oracles, oracle-cache) stay the writer's.
- 72f8dab3: merged main (eb15fccc: documentation-plan.md's corrected port definition).
- 49a88f63: validation/investigations/docs-revision/evidence_roles.py (+ README): the count of
  role_labels, panel roles and marks behind the guide's §4 and §5.
- cbc38220: docs/writing-guide.md after round 1 (645 lines: header with the sources of its facts;
  §1 the four words as the plan defines them, where each port lives on main, LARMORX's own and
  nibabel, Other words with compared cases, byte/bit, the original's own variability, project,
  companion packages; Who changes what with the clean-room readers' rule; Mechanics of the
  rollout; §2 rules 3, 5, 7, 11 revised, 13 "shorten words, never facts", 14 "pages describe
  main"; §3 28 examples (new 8: the FAIR text; 12, 13, 4, 5, 6, 10/11 rewritten); §4 A/B values,
  a table mapping all 854 of today's role_labels (22 kinds), panel order; §5 the other roles'
  colours, one token set, verdict/software/accent collisions, the band's reason; §6 pages,
  addresses, groups, 55 reference pages, one source, redirects and anchors, where every part of
  today's site goes, the home outline; §7 all ten columns; §8 questions 1-5);
  inventory.md made again (repo 72f8dab3; the only change: "original, to read" 19,783 -> 19,785,
  documentation-plan.md's corrected lines).
- 2827ad12: validation/investigations/docs-revision/old_words.py (+ README.md): the old-words
  inventory; inventory.md generated.
- 2f42aa94: docs/writing-guide.md (sections: 1 four words + other words + where old words stay +
  renames; 2 tone; 3 27 before/after examples, 7 from the home page; 4 figure titles; 5 colours;
  6 site structure + home outline; 7 inventory totals; 8 three questions for the user);
  old_words.py skips docs/writing-guide.md; inventory.md regenerated (repo 2827ad12, site b768293a).

Run the inventory again (15 s):
  python3 -P validation/investigations/docs-revision/old_words.py --repo . --site <site clone> --out validation/investigations/docs-revision/inventory.md
Totals now (repo cb45ecb4): 844 files to change in all; the writer's own 361 (1,998 "clean-room
original", 193 "original" for a clean room, 228 "exact replica", 1,526 jargon on user pages);
generated 64 (incl. the pages made by a measurement); others (clean-room agent, spec writer,
orchestrator) 42; kept 377. 33 files renamed, 14 kept.

## Facts found (for sub-tasks 2 to 4)
- Colours today: static/js/evidence.js PAL: original #4a453e/#b6b0a3, larmorx #c76717/#d97530,
  replica #467dea/#5890fa. CSS static/css/evidence.css: .ln-replica uses var(--v-within-variability)
  (the verdict's colour variable); .ln-band too (the original's variability band in the blue: the
  guide moves it to an ink wash); .role-larmorx .ev-role is the accent; markers today mk-dot,
  mk-ring, mk-x (squares and triangles are new).
- Evidence JSON: panels' role is 'original' | 'larmorx' | 'input' | 'fmriprep' | 'reference' (the
  replica and the clean room are both 'larmorx'); chart series' "mark": ANTs pages mark their ports
  'larmorx' (they are replicas); afni-volreg has 13 replica series marked 'larmorx'. The evidence
  module (validation/src/larmorx_validation/evidence/) must name the port; a re-export changes the
  JSON files, which the merge's site inventory (validation/investigations/difftest/site_inventory.py)
  flags byte for byte: the merge judges them.
- 81 of 116 worked examples' posters hold difference images (baked webp, the diverging blue/apricot
  scale): the guide keeps that scale (no redraw).
- Diff labels say "original − FSL 6.0.7.23" for the clean room (static/evidence/*.json, 143 such
  forms); panel labels "larmorx's clean-room original".
- The site's page inventory counts words per page; the guide's question 3 asks the user to compare
  everything but words.
- The installation page documents 0.1.0rc1 (`implementation="original"`, LARMORX_IMPLEMENTATION),
  with "coming with the next release" notes: keep that until the next release (guide §1).
- data/ecosystems.yml has no pastel for tedana, sdcflows, fMRIPrep (the guide since relay 2: a
  pastel each, chosen and checked in the Programs pilot); validation/tools.toml marks them eco
  "larmorx"; its `how` column says "port" for the ANTs ports (the rollout: "replica").
- (relay 2) The evidence's role_label values: 22 kinds, 854 in all (the guide's §4 table maps each;
  the script: validation/investigations/docs-revision/evidence_roles.py <site>, commit 49a88f63). Panels with role 'reference' (54) are the original
  with correctly rounded maths ("ANTs 2.6.5, correctly rounded maths"); role 'fmriprep' (36) is
  fMRIPrep's run ("fMRIPrep 25.2.6, as it ran", "fMRIPrep 25.2.6's own output"); "against the
  reference" (resample-series, sdcflows-fieldmap) is a computed exact answer.
- (relay 2) .dot takes the deep colour (--kd): ANTs #467dea = the replica's blue, LARMORX #c76717 =
  the clean room's apricot; --accent is --c-larmorx; focus rings and sliders --deep-larmorx.
  Dark theme: .role-larmorx .ev-role and .ln-larmorx use --deep-larmorx = #f2b28a, PAL #d97530.
- (relay 2) The builder (site tools/build.py) checks internal links and anchors (problems), has no
  redirect page type (step 2 adds one).
- (relay 2) 46 commands: ANTs 13, AFNI 4, FSL 26 (mcflirt's clean room is `mri hmc`), mri 1,
  tedana 1, sdcflows 1 (crates/larmorx-cli/src/lib.rs FAMILIES); 38 API pages = 37 programs and
  modules + upstream-defects.md.
- Printed lines of clean rooms in the old words (for the clean-room agent's list, sub-task 4):
  crates/larmorx-afni/src/cli/volreg.rs:48 (3dvolreg's usage line), crates/larmorx-fsl/src/cli/
  applytopup.rs:77 (NOT_YET message), crates/larmorx-fsl/src/topup/settings/*.cnf line 2 (6 files,
  shipped settings' headers); the rest of the 30 hits in clean-room code are comments and
  PROVENANCE. A full list from a grep over printed strings is still to make (sub-task 4).
- Harness quirk: Bash commands that cd outside the worktree with heredocs, `git -C`, `source`, or
  python fed text containing the word "git" are refused; write small scripts to <scratch>/tools/
  and run them with literal paths.

- (relay 3) Pages made by a measurement: 77 docs/validation/*.md carry the renderer's "- **Generated:**"
  line (parity/render.py, stages/record.py), 2 more "Generated with `python -m larmorx_validation
  .parity...`" (mri-hmc-normcorr-count, resample-series-jacobian); clean-room-audit.md is written
  by `cleanroom-audit summary` ("do not edit by hand"). 11 of the 21 benchmark reports are written
  by `python -m larmorx_validation bench`, 10 by hand from an investigation script's JSON. The
  evidence cites by SHA-256 68 record .md and three hand-written benchmark .md (afni-volreg,
  fsl-maths, fsl-prelude), plus every benchmark .json (provenance.records, .benchmarks).
- (relay 3) The clean-room audit's restriction table (cleanroom_audit/restrictions.py) has rules
  R01-R07 (sources: FSL, AFNI, FreeSurfer, Workbench, MSM_HOCR, glibc/libstdc++, tedana), R10
  replica crates, R11 the 13 source-derived findings (afni-tshift.md and fsl-mcflirt.md carry no
  "Restricted" word: "Two implementations, two rules ... must not open this file" and "Forbidden
  to the clean-room implementer"), R12 the register's "## Restricted:" sections, R13 docs/parts,
  R14 validation/probes, R15 the replica authors' random-testing cases, R16 replicas' suite and
  evidence modules a brief names, R17 transcripts, R18 other agents' scratch.
- (relay 3) Role 'reference' (54 panels): 51 ANTs runs with correctly rounded maths, 1 FSL prelude
  "the same map without the NaN" (runs of the original), resample-series' computed reference
  ("The factor on the BOLD's own grid, carried here"), sdcflows-fieldmap's exact field (a known
  answer). resample-series' two "against the reference" differences: the replica in fix
  ("larmorx's factor") and in reproduce ("fMRIPrep's factor", bit-identical to fMRIPrep's).
- (relay 3) `larmorx fsl mcflirt` runs `larmorx_mri::cli::run_given("hmc", ...)` when the
  clean room is chosen (crates/larmorx-cli/src/lib.rs); in 0.1.0rc1 LARMORX_IMPLEMENTATION=original
  chose it (replica.rs IMPLEMENTATION_ENV).
- (relay 3) The site's 404.md is built to 404.html (tools/build.py); GitHub Pages serves it for any
  missing address. Built output (_site/) is not committed.

## Questions for the user (in the guide, §8): DECIDED 2026-11-12, all five as recommended (the guide's §8, relay 8)
1. Keep the tagline in the hero (recommended: yes); "Many toolkits, one framework" goes.
2. Keep the verdicts' and difference maps' colours that share a hue with a port (recommended:
   yes, with the channel rule: ports = lines and labels; verdicts = patterned fills).
3. The merge's inventory compares everything but word counts, and the reviewer checks that every
   fact stays (recommended).
4. (relay 2) The pages describe `main`, with what 0.1.0rc1 lacks marked "from the next release"
   (recommended).
5. (relay 2) Developer documents keep oracle, upstream, parity, the main package (recommended).
Question 2 now also covers the software colours (ANTs' deep = the replica's blue, LARMORX's = the
clean room's apricot): only pastels with names; a pastel each for tedana, sdcflows, fMRIPrep.

## Shared documents changed directly (for the orchestrator)
- (relay 28) the site's content/roadmap.md (the documentation revision's row and FreeSurfer's state), data/status.yml
  (planned: the documentation revision, FreeSurfer's state, the confounds' card), content/validation.md (round 25's C,
  D, E, F, H), content/validation/data-panel.md (J), README.md (K), content/programs/index.md (the cards' anchors),
  tools/evidence.py (DIFFTEST_CHECKS, the runs' total, one order of the ports, Unclear's hover, "21 programs"),
  static/evidence/summary.json (made again: the note); repository: evidence/summary.py (the note), difftest.py
  (`_TRIAL`, CHECK_WORDS), afni_volreg.py (the replica's module: its triage table's check names), resample_series.py
  (the poster), nifti_io.py (one rule). docs/next-steps.md, the register and the decision log: NOT edited (HANDOFF.toml
  ops 1-25, 27, 28; op 27's NIB-04 platforms from numpy's documentation of `numpy.longdouble`, a public source the entry
  names).
- (relay 27) the site's content/validation.md (P2: reorganised in the guide's words, every anchor kept; the matrix's
  call with the Programs page's groups; E: the runs since the long run), site.yml (`docs:` the data panel's address),
  data/status.yml (planned: the confounds' entry in its new state, from the confounds' HANDOFF-site.toml in the new
  words), content/roadmap.md (the new words and the confounds' HANDOFF-site.toml: Last updated, the Confounds row
  out of "Under way" with the rows renumbered, the Completed row of 2026-11-13; example 17's row), data/programs.yml
  (generated), the site's README.md and CLAUDE.md (the new words), tools/evidence.py (the matrix grouped by software,
  no dots; random_testing_runs; port names in random testing's tables), static/css/evidence.css (the matrix's group
  rows and links; the runs' note); repository: validation/programs.toml (the confounds' summary and description),
  validation/src/larmorx_validation/evidence/difftest.py (E), figures/cache.py and evidence/core.py (M1's machine
  paths: every exported string).
- (relay 26) the site's site.yml (`nav:` the header's six links of the guide's section 6; `docs:` the sidebar's
  and footer's list with Getting started, Concepts, Why Rust and Python, About; `tools:` the two pages' addresses and
  summaries, BET's summary in the new words, the comment), content/roadmap.md (three links to /getting-started/),
  content/index.md (the home page's links: Getting started, Concepts, Why Rust and Python), data/status.yml and
  data/programs.yml (generated: the addresses); validation/tools.toml and programs.toml (the two addresses),
  validation/src/larmorx_validation/evidence/summary.py (the summary's addresses and words, its generator).
- (relay 25) the site's site.yml (sdcflows' two entries: addresses and summaries in the new words),
  content/roadmap.md (two links), data/status.yml and data/programs.yml (generated: the addresses),
  content/evidence/tedana-t2smap.md (round 22's sentence); validation/tools.toml and programs.toml (the two
  addresses). Shared code: evidence/present.py (`known`, `split`, page-named comparisons not read by their shape,
  two ports counted from LARMORX's panels; the regression gate --ref 06fb0cab: 0 differences on the 20 pages
  presented before).
- (relay 24) the site's site.yml (t2smap's entry: address and summary in the new words), content/roadmap.md
  (two links), data/status.yml and data/programs.yml (generated: the address), content/evidence/
  fsl-prelude.md and mri-hmc.md (round 21's History lines); validation/tools.toml and programs.toml (the
  address). Shared code: none (present.py, words.py and the FSL and AFNI helpers untouched). The figures
  cache: one new entry (above). docs/next-steps.md: NOT edited.
- (relay 23) the site's site.yml (prelude's and the head-motion correction's entries: addresses and summaries
  in the new words), content/roadmap.md (four links), content/evidence/fsl-flirt.md (one link to
  /evidence/mri-hmc/), data/status.yml and data/programs.yml (generated: the addresses);
  validation/tools.toml and programs.toml (the addresses). Shared code: none (fsl_pages.py and present.py
  untouched). docs/next-steps.md: NOT edited.
- (relay 22) the site's site.yml (fslmaths' and fslstats' entries: addresses and summaries in the new
  words), content/roadmap.md (four links), data/status.yml and data/programs.yml (generated: the
  addresses); validation/tools.toml and programs.toml (the addresses). Shared code:
  evidence/fsl_pages.py (`replica_threshold` takes "every file <program> writes"; the regression gate
  against 1178e9f1: 0 differences on every presented page). docs/next-steps.md: NOT edited.
- (relay 21) the site's site.yml (topup's entry: address and summary in the new words), content/roadmap.md
  (topup's five links), data/status.yml and data/programs.yml (generated: the address); validation/tools.toml
  and programs.toml (the address). Shared code: evidence/fsl_pages.py (`present`'s optional `examples=` and
  `speed_page=`; the regression gate: 0 differences on every presented page). docs/next-steps.md: NOT
  edited; its follow-up for the -dirty benchmark reports and cache entries is HANDOFF.toml's op 25.
- (relay 20) the site's site.yml (FAST's entry: address and summary in the new words), content/roadmap.md
  (FAST's four links), data/status.yml and data/programs.yml (generated: the address); validation/tools.toml
  and programs.toml (the address). Shared code: evidence/common.py (`computed`, used by `provenance` and
  six modules). docs/api/fsl-flirt.md (Not yet: standard_space_roi). docs/next-steps.md: NOT edited.
  **For the merge's audit:** the documentation role's edits of clean rooms' evidence modules again
  (afni_volreg_clean_room.py: 922b7dce, d9ff811c; fsl_bet_options_clean_room.py and
  fsl_prelude_clean_room.py: d9ff811c, 1dcacdeb: the provenance line only).
- (relay 19) the site's site.yml (FLIRT's entry: address and summary in the new words),
  content/roadmap.md (FLIRT's five links), data/status.yml and data/programs.yml (generated: the
  address); validation/tools.toml and programs.toml (the address). Shared code: the new
  evidence/fsl_pages.py. docs/next-steps.md: NOT edited.
- (relay 18) the site's site.yml (3dvolreg's entry: address and summary), data/status.yml and
  data/programs.yml (generated: the address); validation/tools.toml and programs.toml (the
  address). Shared code: evidence/present.py (`lines`, `LINE_MARKS`, larmorx `<commit>` kept),
  evidence/afni_pages.py (the rules for "the original", `categories=`). docs/next-steps.md: NOT
  edited.
- (relay 17) the site's site.yml (the three AFNI entries: addresses and summaries in the new
  words), data/status.yml and data/programs.yml (generated: the three addresses),
  static/evidence/summary.json (made again: 3dTshift's replica's two counts); shared code:
  validation/src/larmorx_validation/renamed.py (new) and the three packages' `__init__.py`
  (`renamed.alias`), evidence/ants_atropos.py (`RUNS`); validation/tools.toml and
  programs.toml (the three addresses). docs/next-steps.md: NOT edited, <scratch>/HANDOFF.toml.
- (relay 16) docs/findings/upstream-defects.md (the register: three paths, the renamed findings
  and test, words unchanged), docs/validation/README.md (generated from tools.toml: three
  findings paths), validation/tools.toml (three findings paths), docs/findings/README.md (the
  index's link); the site's content pages (links), data unchanged. docs/next-steps.md: NOT
  edited, 24 ops in scratch tools/r16/next-steps.toml for the handoff. Shared code: the three
  packages' `__init__.py` (`_RENAMED` and `__getattr__`), validation/src/larmorx_validation/
  __main__.py (PARITY_MODULES' two module names), crates/larmorx-cli/tests/fsl_output_types.rs
  (two comments naming the renamed test). **For the orchestrator's audit:** the eight findings
  renamed (-clean-room.md, accepted since 8b68a74a) and eight new pointer files at the old names.
- (relay 15) the site's site.yml (ITK transform files' entry: the new address, its summary in the
  new words), data/status.yml and data/programs.yml (generated); validation/tools.toml (one
  `page`) and validation/programs.toml (one `evidence`). Not a shared document but shared code
  for the parity suites: validation/src/larmorx_validation/parity/fmriprep_nodes.py now reads
  the data panel's fourteen runs only (`PANEL_RUNS`), so a panel record made again compares the
  same runs as today's records (the 2026-11-08 runs in the folder were otherwise read too).
- (relay 14) the site's site.yml (ImageMath's and antsAI's entries: the new addresses, summaries
  in the new words), data/status.yml and data/programs.yml (generated), static/evidence/
  summary.json (made again from the evidence JSONs: the reviewer's round 11, finding 1;
  antsRegistration's and antsAI's counts only); validation/tools.toml (two `page`s) and
  validation/programs.toml (eight `evidence`s).
- (relay 13) the site's site.yml (antsRegistration's and Atropos' entries: the new addresses,
  summaries in the new words), data/status.yml and data/programs.yml (generated),
  tools/evidence.py (the dots chart on a presented page; the agreement chart's title over two
  records links both), static/css/site.css (`.lede > code` on one line); validation/tools.toml
  (two `page`s) and validation/programs.toml (two `evidence`s); docs/writing-guide.md rule 7
  (the lede's two counts, round 10's minor 2).
- (relay 12) the site's site.yml (DenoiseImage's and antsApplyTransforms' entries: the new
  addresses and their summaries in the new words), data/status.yml and data/programs.yml
  (generated), tools/evidence.py (the band's key and tooltips name the page's software, from
  the band's `software`; ANTs before it); validation/tools.toml (the two `page`s) and
  validation/programs.toml (the two `evidence`s).
- (relay 11) the site's site.yml (N4's entry: the new address and its summary in the new words),
  data/status.yml and data/programs.yml (generated from tools.toml and programs.toml),
  tools/evidence.py and static/css/evidence.css (presented pages' band, chain, random-testing
  panel; the random-testing columns' explanations on every page, wording only; the status
  matrix's link to a moved page); validation/tools.toml (N4's `page`).
- (relay 10) the site's data/verdicts.yml (the d13 definition, wording only: every fact in it is
  on the installation page and bet's evidence page).
- (relay 9) docs/porting-guide.md (Status: a pointer to VALIDATED_BEFORE and the site's list);
  the site's content/validation.md (#statuses), content/programs/index.md, data/programs.yml
  (generated).
- (relay 8) docs/porting-guide.md (the Status paragraph: the orchestrator's corrected definition);
  the site's site.yml (tools' `eco` and order), templates/doc.html, static/css/site.css and
  evidence.css, static/js/evidence.js, tools/evidence.py (category labels, table titles; all but
  the sidebar gated by the page's JSON), data/verdicts.yml (one label), content/roadmap.md,
  content/validation.md, data/status.yml (planned).
- (relay 7) scripts/orchestration/verify-merge.sh (the orchestrator's script: STEP5 reference
  with --site); the site's tools/build.py (check_references, synopsis blocks),
  templates/reference.html, static/css/site.css (.port-mark, .status-tag--plain, .opt-help).
- (relay 6) README.md (generated tool table), site.yml, content/validation.md, data/status.yml,
  data/ecosystems.yml, templates/doc.html and home.html, site.css, site.js: the list and why in
  "State (relay 6 done)".
- (relay 5) none of CLAUDE.md's list. The site's shared code changed for bet's pilot:
  tools/evidence.py, static/js/evidence.js, static/css/evidence.css, static/css/site.css (the
  --port-input token); every change is gated by the JSON's `"presentation": "writing-guide"`.
- (relay 4a) site.yml: the BET entry of `tools:` points at /evidence/fsl-bet/ (the old address is
  a redirect page). (docs/writing-guide.md and validation/investigations/docs-revision/ are new
  files of this part.)
- (relay 4a) validation/src/larmorx_validation/__main__.py: the `reference` subcommand (code that
  must change for the tool to work, minimal).

## Documents a clean-room implementer may read, changed by this part (the orchestrator's rule)
- (relay 27) content/validation.md (P2, words only and moved sentences, every fact kept from the old page
  (tools/r27/validation.before.md); new sentences: the AFNI move kept from the old "Three ways" paragraph; the
  colours, titles and What this shows from docs/writing-guide.md sections 4 and 5 and docs/documentation-plan.md;
  the runs' paragraph from difftest.json's runs, i.e. the recorded summaries of oracles/difftest/<run>/), the data
  panel (content/validation/data-panel.md, words only), content/roadmap.md (words only; the confounds' row from the
  confounds part's HANDOFF-site.toml, its facts in its CHANGELOG/API page on main), content/404.md,
  content/programs/index.md (the confounds sentence: the confounds part on main), data/status.yml's confounds entry
  (the same source), difftest.json (E: labels, port names, the triage's texts reworded by words.reword and LARMORX
  in prose; the runs' numbers from the runs' summaries, the same numbers the FSL, AFNI, tedana and sdcflows pages
  show), the site's README.md and CLAUDE.md (words), the eight JSON exported again for M1 (paths only).
- (relay 26) content/evidence/resample-series.md and nifti-io.md with their JSON (no clean room of either; the
  resampler's is planned): every fact from the old pages (tools/r26/*.tools-before.md), their JSON, the records'
  headers (docs/validation/resample-series.md: the reference "fMRIPrep 26.0.0.dev1+g21a490fb8 ResampleSeries",
  nitransforms 25.1.0, SciPy 1.18.1, 139 cases, the fix pass's PIPE-03 and SCI-02; resample-series-jacobian.md;
  nifti-io.md), the benchmark reports' environments (fMRIPrep 26.0.0.dev1+g21a490fb8; SimpleITK 2.5.6), the
  register's rows (PIPE-03, SCI-02, SCI-03, NIB-04 with their dates), crates/larmorx-transform/PROVENANCE.md and
  larmorx-interp/PROVENANCE.md (fMRIPrep at 21a490fb Apache-2.0, nitransforms 25.1.0 MIT, SciPy BSD-3-Clause),
  docs/status.md, the sdcflows pages (larmorprepx); content/getting-started.md (the installation page, words only;
  its packages' full text moved to Concepts), content/concepts/index.md (new: from docs/writing-guide.md section 1,
  the home page's cards and ways-note, the installation page's packages, settings and notice, Why Rust and Python's
  "every sum over parallel work is added in a fixed order", docs/validation/clean-room-audit.md's "What this page
  proves" and "The records"), content/concepts/why-rust-and-python.md (words only), content/about.md (new: the
  footer's notes and credits, CITATION.cff's message and its authors, title, version and repository; the evidence
  pages' credits), the evidence modules resample_series.py, nifti_io.py and present.py (presentation texts from the
  JSON's own reasons).
- (relay 25) content/evidence/sdcflows-fieldmap.md and sdcflows-estimation.md with their JSON (sdcflows has no
  clean room yet; one is planned): wording only, every fact from the old pages (tools/r25/sdcflows-*.tools-before.md),
  their JSON (the examples' verdicts, reasons, facts and notes), the records' headers (docs/validation/sdcflows-*.md,
  ndimage-skimage*.md, niworkflows-helpers.md: their dates for History), the roadmap's rows and
  reference/programs.py's VALIDATED_BEFORE (the status). The SyN example names ANTs' registration as its
  original, as its panels did. tedana_t2smap.py (restricted): the default view and two texts (round 22).
- (relay 24) evidence/tedana_t2smap.py is restricted (BRIEF_RESTRICTED) and the page it exports with
  content/evidence/tedana-t2smap.md and static/evidence/tedana-t2smap.json is restricted for tedana's
  clean-room implementers (the guide, section 1); readable once exported, so wording only: every fact from
  the old page (site 8b97b25, content/tools/tedana-t2smap.md), the JSON (its examples' verdicts, reasons
  and facts, the tables, the cases), the records it cites (docs/validation/tedana-t2smap-original.md and
  .json: the fMRIPrep cases' (A) and (B) counts, the verdict counts; tedana-t2smap-replica.md: its counts),
  the benchmark rows, the site's roadmap (2026-11-08 and 2026-11-12 rows: TED-01 to TED-18, NP-03, NP-04,
  GLIBC-07, ANALYZE and MGH) and the guide's table (the clean room from the next release); the new numbers
  of the three examples come from the clean room's runs (black-box runs of LARMORX's own program) and the
  clean room's suite's check.py. validation/tests/test_evidence_present.py (two tests): the same facts.
  No clean-room file changed (tedana_t2smap_clean_room.py untouched: the replica's module calls its RUNNER,
  _settings, _check, _share, BC, WHAT, SPEC, TRIAL and entry as a black box).
- (relay 23) evidence/fsl_prelude.py (readable: not in BRIEF_RESTRICTED) `present` and both_thresholds,
  evidence/fsl_prelude_clean_room.py (a clean room's module: its export ends with `present`, the hardened
  call of the replica module's, no text), evidence/mri_hmc.py `present`, and the site's
  content/evidence/fsl-prelude.md, mri-hmc.md and static/evidence/fsl-prelude.json, mri-hmc.json (exported
  by them): wording only, every fact from the old pages (site 9a98cec, content/tools/fsl-prelude.md and
  mri-hmc.md), their JSONs (the examples' verdicts, reasons, facts and tables; the cases' verdicts and
  categories; the random-testing and triage tables; the benchmark rows and the speed chart's rows), the
  records they cite (docs/validation/fsl-prelude.md and .json, fsl-prelude-replica.md, mri-hmc.md and
  .json, fsl-mcflirt-replica.md, fsl-mcflirt-replica-full.md: counts, Thresholds tables, the fix pass's
  list, Generated dates, the measured offsets of the GLIBC-07 cases), docs/benchmarks/mri-hmc.md and .json
  (the report's date, machine, runs), docs/api/mri-hmc.md (both commands run lx.mri.hmc), the register's
  rows (FSL-11), the site's roadmap (2026-10-10, 10-25, 10-29, 10-30, 11-04, 11-05, 11-06, 11-07) and
  the site's JSON before cffa3c3 (the no-fsloutputtype case's earlier verdict). Round 20: fsl_maths.py's
  TEXTS (wording), content/evidence/fsl-maths.md and fsl-stats.md (the review's sentences; the roadmap's
  2026-11-04 row). validation/tests/test_evidence_present.py (three tests).
- (relay 22) evidence/fsl_maths.py and fsl_stats.py (readable: not in BRIEF_RESTRICTED) `present` and
  the site's content/evidence/fsl-maths.md, fsl-stats.md and static/evidence/fsl-maths.json,
  fsl-stats.json (exported by them): wording only, every fact from the old pages (site a1c1d73,
  content/tools/fsl-maths.md and fsl-stats.md), their JSONs (the examples' verdicts, reasons and tables;
  the cases' ports and categories; the random-testing and triage tables; the glibc tables), the four
  records (docs/validation/fsl-maths.md, fsl-maths-replica.md, fsl-stats.md, fsl-stats-replica.md:
  counts, Generated dates), the benchmark report docs/benchmarks/fsl-maths.md (the steps' dates:
  2026-11-03 for its relay 5, 2026-11-04 for relays 7 and 7b; findings 23 for relay 6's kernel
  correction) and the site's roadmap (the replicas 2026-11-03, the clean rooms complete 2026-11-04).
  fsl_maths_clean_room.py (a clean room's evidence module): NOT edited (its synthetic inputs' placeholder
  is reworded by fsl_maths.present_cases). evidence/fsl_pages.py: `replica_threshold`'s pattern, no
  text. The site's content/evidence/fsl-topup.md (round 19: wording, the review's sentences; the facts
  from the same page and its JSON). validation/tests/test_evidence_present.py (one test).
- (relay 21) evidence/fsl_topup.py (readable: not in BRIEF_RESTRICTED) `present` and the site's
  content/evidence/fsl-topup.md and static/evidence/fsl-topup.json (exported by it): wording only, every
  fact from the old page (site b899cc2, content/tools/fsl-topup.md), its JSON (the examples' verdicts,
  reasons, metrics and movement table; the difftest cases; the three random-testing tables), the two
  records (docs/validation/fsl-topup.md, fsl-topup-replica.md: counts, Generated dates), the benchmark
  rows of the JSON and the site's roadmap (2026-11-03's two parts). evidence/fsl_pages.py (readable):
  two optional parameters, no text. validation/investigations/docs-revision/dirty_provenance.py and its
  README line (new, no fact of a port: provenance commits only).
- (relay 20) evidence/fsl_fast.py (readable) `present` and the site's content/evidence/fsl-fast.md and
  static/evidence/fsl-fast.json (exported by it): wording only, every fact from the old page (site
  8a28495), its JSON (the examples' verdicts, reasons and metrics; the difftest cases' ports; the triage
  table), the two records (docs/validation/fsl-fast.md, fsl-fast-replica.md: counts, Generated dates), the
  benchmark rows of the JSON, the site's roadmap (the replica 2026-11-02, the clean room 2026-11-03) and
  docs/findings/fsl-fast-clean-room.md (a clean room's own findings: the first random-testing run's date).
  Round 17: fsl_flirt.py (the metrics notes' thresholds from docs/validation/fsl-flirt.md's Thresholds table;
  the "Original" fact's runs from the example's own comparisons), content/evidence/fsl-flirt.md (the lede and
  Known limits from the page and validation/tools.toml; History from the site's roadmap), fsl-bet.md and
  fsl-fast.md (the decision's citation, wording), docs/api/fsl-flirt.md (standard_space_roi ported: from
  validation/tools.toml and docs/api/fsl-bet.md), common.py, afni_tshift.py, afni_automask.py,
  afni_volreg_clean_room.py, fsl_bet_options_clean_room.py, ants_n4.py, fsl_prelude_clean_room.py (the
  provenance line's re-run dates, from each entry's RUN_INFO.json; no other fact); afni_volreg.py (restricted
  for AFNI clean-room implementers: its metrics note's clean-room thresholds come from
  docs/validation/afni-volreg-original.md's Thresholds table, a readable record);
  validation/tests/test_evidence_present.py (two tests).
- (relay 19) evidence/fsl_flirt.py (readable: not in BRIEF_RESTRICTED) and the new
  evidence/fsl_pages.py: presentation only (rewordings, titles, the "What this shows" lines built
  from each example's own reasons and facts in the JSON), no fact; the site's
  content/evidence/fsl-flirt.md and static/evidence/fsl-flirt.json (readable, exported by
  fsl_flirt.py): wording only, every fact from the old page (site 7488ffd), its JSON, the two
  records (docs/validation/fsl-flirt.md and fsl-flirt-replica.md: their counts, 1 failure,
  Generated dates), validation/tools.toml (`standard_space_roi`: the replica's since 2026-11-04,
  the clean room's since 2026-11-05, its cases in BET's records) and the benchmark rows of the
  JSON (the speed ratios); validation/tests/test_evidence_present.py (a test of the presenter).
- (relay 18) evidence/afni_volreg_clean_room.py (readable): its export ends with `present(doc)`, a
  delegate to the replica module's (a call, no fact); evidence/afni_pages.py and present.py
  (presentation rules, no fact); validation/tests/test_evidence_present.py (a test of the
  presenter); the three S5 pages (round 15: every fact from their records and JSONs, the
  review's sentences). 3dvolreg's page and JSON are restricted for AFNI clean-room implementers
  (exported by afni_volreg.py, BRIEF_RESTRICTED); its new texts live in afni_volreg.py.
- (relay 17) the AFNI pages 3dTshift, 3dAutomask, 3dUnifize (site content/evidence/afni-*.md
  and their JSONs; readable: their modules are not in BRIEF_RESTRICTED): wording only, every
  fact from the old page, its JSON or the records the page cites (docs/validation/afni-*.md:
  their counts and Reference line, the facts the reviewer's items (1)-(3) name); the "What this
  shows" lines from each example's own verdicts and reasons.
- (relay 16) paths only (a renamed file's new name, nothing else): docs/api/afni-automask,
  afni-unifize, afni-volreg, fsl-bet, fsl-fast, fsl-flirt, fsl-roi, fsl-topup, mri-hmc,
  tedana-t2smap; docs/findings/README.md, platform-math.md, afni-volreg-clean-room.md,
  fsl-bet-clean-room.md, fsl-fast-clean-room.md, tedana-t2smap-clean-room.md (renamed, paths
  inside); the eight pointer files (new: the new name, the date, the guide's section, which
  kinds of files link them); the site's content/tools pages (links). The ITK page's round-13
  text (ITK has no clean room yet; the rule holds): docs/api/transforms-hdf5.md's D13 table,
  the register's rows ITK-12 to ITK-17, HDF-01, PIPE-04, PIPE-17 (their fMRIPrep column),
  difftest/tools/itk_transforms.py's docstring, the record's Notes.
- (relay 15) the site's content/evidence/itk-hdf5.md (new; ITK has no clean room yet, the rule
  still holds): its facts from the old page (content/tools/itk-hdf5.md at 6660758), the record
  (docs/validation/itk-hdf5.md: 149 cases, 128 pass, 16 rejected by both, 5 expected
  divergences; its reference line; Generated 2026-10-21), crates/larmorx-hdf5/PROVENANCE.md (HDF5
  1.12.1, the version ITK 5.4.5 bundles, a BSD-style licence; the writer replays HDF5's layout)
  and the writing guide's §1 table (ANTs and ITK: clean room planned after FreeSurfer); its JSON
  exported by itk_hdf5.py (wording only). content/evidence/ants-programs.md's two sentences
  (round 12): from the old page (b768293) and the other presented pages' sentence.
- (relay 14) the site's content/evidence/ants-programs.md and ants-ai.md (new; ANTs has no clean
  room yet, the rule still holds) and ants-atropos.md's lede and corrections paragraph: their
  facts from the old pages (content/tools/*.md at 1bc3691), the records (docs/validation/
  ants-image-math.md: 303 of 308, the five within a tolerance; the data panel records' summary
  lines and Generated dates; ants-ai.md: 76 compared, 75 as built; ants-affine-initializer.md:
  26 of 26; ants-ai-panel.md 40, 38; ants-affine-initializer-panel.md 14), the register's rows
  (ANTS-30 approved 2026-10-22; ANTS-90 to ANTS-106, ITK-50, ITK-51), the JSON's examples and
  ants_ai.py's run (antsAffineInitializer in reproduce at 8 threads).
- (relay 13) the site's content/evidence/ants-registration.md and ants-atropos.md (new; ANTs
  has no clean room yet, the rule still holds), the S2 pages' ledes and antsApplyTransforms'
  two sentences on threads: their facts from the old pages (content/tools/*.md at 2b2306e), the
  records docs/validation/ants-registration.md (513 cases, 399 compared, 355 as built, 93, 21;
  thread band 2, 4, 8; 2026-10-28), -panel.md (71, 16, 55; ITK-31's 28 cases; thread band 2, 3;
  2026-10-27), -panel-template.md (14 of 14 with CORE-MATH; 2026-10-28), -full.md and
  -velocity-full.md (names only), -corrections.md; ants-atropos.md (97 cases, 27 of 58, 58 of
  58; 2026-10-23), -panel.md (17 of 17, 3; 2026-10-26), -full.md; ants-apply-transforms.md and
  -panel.md ("run with 4 threads"); the register's rows ITK-31, ITK-50, ITK-51, ANTS-90 to
  ANTS-107 (their count); git log of docs/validation/ants-registration.md (3899deb4: the 8
  permanent random-testing cases); the evidence JSON (the What this shows numbers above);
  docs/writing-guide.md rule 7 (wording, an example from DenoiseImage's lede). No fact from
  restricted material.
- (relay 12) the site's content/evidence/ants-denoise.md and ants-apply-transforms.md (new; ANTs
  has no clean room yet, the rule still holds) and content/evidence/ants-n4.md (the reviewer's
  texts): their facts from the old pages (content/tools/ants-denoise.md and
  ants-apply-transforms.md at 9b2fc9b), the records docs/validation/ants-denoise.md (51 cases,
  7 of 40, 40 of 40; the fix pass's ids), ants-denoise-panel.md (21 of 21, 0; 2026-10-26),
  ants-apply-transforms.md (200 of 200, 185; 102 of 102; 34 of 34; 4 expected divergences;
  ANTS-06) and ants-apply-transforms-panel.md (425 of 425, 329; 167 of 167; 2026-10-26), the
  benchmark reports docs/benchmarks/ants-denoise.md and ants-apply-transforms.md (their six
  inputs, ANTsPy, double precision), the evidence JSON (the examples' metrics and facts: 85 %,
  5,522, 271, 73, 437,915, 2.81, 611,567, 177, 2,871, 2,715, 6.8e-13, 0.013; ANTS-43
  "corrected since 2026-10-12"; the repeats of 2026-10-21), docs/status-log.md (A5 part 3 merged
  2026-10-23, B-spline transform files), the register's ANTS-36 row and section (`-r` without a
  mask does nothing in both modes, and a warning says so), the roadmap's row 5.
- (relay 11) the site's content/evidence/ants-n4.md (new; ANTs has no clean room yet, the rule
  still holds): its facts from the old page (content/tools/ants-n4.md at 6574906), the records
  docs/validation/ants-n4.md and ants-n4-panel.md (46 of 46, 29; 69 of 69, 20; 2026-10-26),
  docs/api/ants-n4.md (ANTs 2.6.2 in fMRIPrep's image, the GCC 11 build), the register's
  summary rows ANTS-35, -36, -38, -40 and ITK-02 (their dates; fMRIPrep passes `-r` without a
  mask), the site's roadmap row 5 (the permissive ports' clean rooms after FreeSurfer), Validation's
  #validated-before and the page's JSON (the speed rows, the panel examples' reasons naming
  ANTS-36); static/evidence/ants-n4.json (wording and fields; the What this shows lines' numbers
  from the same examples' metrics and facts: 0.031, 1.0000 to 1.0002, 43, 0.485 to 1.157, the
  panel's largest differences); evidence/present.py, ants_n4.py, words.py (texts only).
- (relay 9) docs/api/fsl-bet.md (wording; its numbers from docs/validation/fsl-bet.json and
  fsl-bet-replica.json, both linked from the page: child-3y-B's 0.998917 / 0.998305, 1.00 / 2.24,
  the thresholds; the categories ssroi and fslroi of both records); python/larmorx/fsl/bet.py
  (two messages, one article); reference/page.py, apipage.py, programs.py (the lede rule; the
  frozen list of records from validation/tools.toml); docs/writing-guide.md §1 and
  docs/porting-guide.md (pointers to that list); the site's three reference pages (exported),
  Programs and Validation (#statuses: the record ids and statuses of tools.toml, the records'
  dates). No fact from restricted material.
- (relay 8) docs/porting-guide.md (the statuses' definition, the orchestrator's decision; its
  facts: the testing-depth decision of 2026-10-28 and the data panel of 2026-10-27, both in
  CLAUDE.md and the site's Validation page); docs/writing-guide.md (§8 the user's decisions, §1
  the statuses, §5 from pastels.py, site.yml and the Programs page); validation/programs.toml and
  reference/programs.py (bet's `does` now from its API page); the site's home, Programs,
  Validation and roadmap pages (facts from docs/next-steps.md "The order from here" and its AFNI
  item 5, docs/benchmarks/fsl-bet.md's memory line, the register's summary table and its
  2026-10-31 and 2026-11-04 entries for GLIBC-06 and GLIBC-07, docs/figures.md's data rule);
  bet's evidence (fsl_bet.py, words.py, fsl_bet_options_original.py: wording, links and the
  default view; the 31 minutes from the run's own summary.json). No fact from restricted
  material; the two links into findings/fsl-bet.md were already on the page and are now marked.
- (relay 7) docs/api/fsl-bet.md (reworded, no new fact: the "What it does" texts from the page and
  bet.py's docstring; the per-program Ports sentences from the records fsl-bet.md and
  fsl-bet-replica.md and the page's own clean-room section; the link to fsl-roi.md's "imcp's
  conversion" and to findings/fsl-bet-original.md, both readable); python/larmorx/fsl/bet.py
  (docstrings: the mesh's vertices and triangles from the Surface docstring, "on the T1's grid"
  and the options from the same file); docs/writing-guide.md (48, 49, rule 11, §5: facts from the
  exporter's code, verify-merge.sh, data/ecosystems.yml and pastels.py); the three reference
  pages (exported).
- (relay 6) bet's evidence page, the home page, the Programs page and its data
  (validation/programs.toml), content/validation.md, data/status.yml's planned list,
  validation/tools.toml and README.md: each with its sources in "State (relay 6 done)".
- (relay 5) docs/writing-guide.md: round 3's text; its new facts come from
  cleanroom_audit/verdicts.py (BRIEF_RESTRICTED, BRIEF_FILES: names of files, no content), git
  ls-files (the 14 modules, the 22 folders), the benchmark reports' links (the five files), the
  exporter's code (what it reads), the site's afni-volreg.json and tedana-t2smap.json (which
  labels come from them).
- (relay 5) docs/api/fsl-bet.md: one key in bet's reference block (`evidence-sections`: a link
  to the evidence page's `#data`, no fact); validation/src/larmorx_validation/reference/
  (apipage.py docstring, page.py: the link).
- (relay 5) the evidence module (words.py new, fsl_bet.py's `present`) and the site's
  fsl-bet.json: wording, ports and order; the SHOWS lines' numbers all come from the same
  example's metrics and reasons in that JSON (1,277; 470; 2,439; Dice 0.9955, 0.9862; 2 to 14
  voxels; Dice 0.9922 to 0.9995, 0.9925 to 0.9994; 2,802). The page is bet's, not exported by a
  restricted module.
- (relay 4a) docs/api/fsl-bet.md: restructured (text moved, no sentence lost). Its new facts,
  each from the main package's code a clean-room implementer may read, none from the replica:
  bet's options without long forms and its `-h` (crates/larmorx-fsl/src/cli/bet.rs: USAGE and
  the parser's arms), bet2's `--binaryMask <image>` and `-h`, `--help` (bet2.rs: USAGE, SPECS),
  the rows naming `smoothness`, `head_ct` and `fsl_data` (python/larmorx/fsl/bet.py's signature
  and docstring); one new sentence on the page's structure (no fact about a program).
- (relay 4a) the site's content/programs/fsl/*.md: generated from the readable sources above
  (the usage text quoted is the clean room's USAGE constant).
- docs/writing-guide.md (new in this part; not marked restricted, so a clean-room implementer may
  read it). Every fact in it comes from documents they may read: the site's pages and evidence
  JSON/CSS/JS, the validation records (docs/validation/fsl-bet.md, fsl-bet-replica.md), the API
  pages, docs/benchmarks/tedana-t2smap.md, CHANGELOG.md, data/verdicts.yml and ecosystems.yml, and
  the main package's Python and command-line code (python/larmorx/_replica.py, bids/, sdcflows/,
  niworkflows/; crates/larmorx-cli/src/lib.rs and replica.rs; crates/larmorx-ants' TOOLS list;
  tests/golden/registry.py). Nothing from the restricted findings (docs/findings/*.md of the
  porting guide's list, the register's restricted sections) or the replica crates. Example 26
  (docs/api/fsl-bet.md) adds no fact: "non-commercial use only" and "translated from FSL's source"
  are in that file (its lines 8 and 11).
- validation/investigations/docs-revision/inventory.md (generated counts of words; no facts).
- (relay 3) The guide's new facts of round 2 come from readable files, each named in the guide or
  here: docs/validation/fsl-bet-replica.md and .json (the 15 expected divergences), fsl-bet.md
  (198 of 200, 378 of 380, 114 of 116), the site's static/evidence/*.json (role 'reference',
  tedana's speed series, provenance.records/.benchmarks), content/tools/*.md (the section
  headings, `#data`), tools/build.py and content/404.md, docs/porting-guide.md §1 and
  validation/src/larmorx_validation/cleanroom_audit/restrictions.py (the restricted material:
  the table's names of things, no content of them), the heads of docs/findings/afni-tshift.md
  and fsl-mcflirt.md (their banners only, read to check the mark; the guide names the two files,
  which §1 already names), the register's summary table and headings (ANTS-40, ITK-50, no †),
  docs/api/*.md (the programs per page), crates/larmorx-cli/src/lib.rs and the v0.1.0rc1 tag's
  replica.rs (mcflirt's dispatch, LARMORX_IMPLEMENTATION), the parity, stages and bench renderers
  in validation/src/larmorx_validation/ (the Generated lines).

## Review answers
Round 1 (reviews/round-1-writing-guide.md), answered in cbc38220. Every finding is fixed; where the
fix differs from the reviewer's suggested wording, why:
- 1 (blocking, port): fixed as the plan now defines it (eb15fccc): one reimplementation, a replica
  or a clean room, in Rust or kept in Python (pybids' index, sdcflows' workflows: checked, no
  _core call); bullet "port is the word for both kinds. Never make it a third kind". (c): the plan
  was corrected on main, so the guide departs from nothing and says nothing about it.
- 2 (replica's licence): fixed (licence terms; tedana's under GPL-3.0); example 4 names every
  replica's package and keeps the PROVENANCE.md fact (rule 13).
- 3 (bet's lede): fixed with the record's numbers: replica 201 of 263 and "47 more are rejected by
  both" (the record's own summary, line 3; "both complete" would be wrong: 15 more are expected
  divergences, 13 of them FSL crashes); clean room 378 of 380, betsurf 114 of 116 within the
  thresholds, 34 bit for bit. Rule 3 now allows one sentence per port.
- 4 (figure titles): fixed: A/B values with fMRIPrep's output and the known answer; self
  comparisons after a colon; a table for all 22 role_label kinds (854; the reviewer's 870 counted
  something else: my count is over role_label fields of static/evidence/*.json). One correction to
  the suggested table: "larmorx's correction" is mostly a replica's (ants-atropos, ITK-50), so it
  maps to "the replica (or the clean room) against itself: fix and reproduce", not "the clean
  room's correction". Panel order as suggested, with the known answer beside the input.
- 5 (colours): (a) fixed (three verdicts; rejected-by-both shares the inputs' grey). (b) fixed:
  resample-series, sdcflows-fieldmap, sdcflows-estimation added; LARMORX's own settled: the clean
  room's apricot with the label "LARMORX's own" (apricot = code LARMORX writes itself; it never
  shares a chart with a clean room: only nifti-io's page). (c) fixed (one token set; dark #d97530
  everywhere; today's #f2b28a named). (d) settled differently from the suggestion: runs of the
  original in any build (role 'reference' = correctly rounded maths, role 'fmriprep' = fMRIPrep's
  run) take the original's colour with a second line naming the run, because they are the
  original; inputs and known answers keep PAL.input's grey (#8a847a), dotted, as today, rather than
  --ink-3, which is almost the original's ink (#5b554c against #4a453e) in the light theme.
- 6 (software colours): fixed: software colours only as pastels (a band, a chip with the name),
  never the deep colour as a dot; tedana, sdcflows, fMRIPrep a pastel each, chosen and checked in
  the Programs pilot (values not picked in the guide: they need the colour-vision check the
  verdicts had); the site's accent stays apricot for controls only; question 2 names the
  collision.
- 7 (version): fixed: rule 14 "pages describe main", lines 40-41 match example 6, examples 2, 5,
  11 (Validation), 25 and the port table mark tedana's clean room "from the next release"; home
  section 5 marks the three options; question 4.
- 8 (facts): fixed: rule 13; question 3; a table "Where every part of today's site goes" (every
  section of every page, incl. Corrections by default, Known limits, the standard images, How to
  read the evidence/figures, The records, the Rust card, the margin note, the FAIR paragraph, now
  example 8); anchors and redirects carry old anchors. One choice differs from the suggestion
  (finding 25 too): the name and FAIR stay in full on Home, where they are today and where the
  user's tagline points; About links to them. Moving them was not asked for, and keeping them
  moves nothing.
- 9 (clean-room readers): fixed with the orchestrator's rule, in its words, in §1 "Who changes
  what"; the guide's header says where its own facts come from.
- 10 (programs and functions): fixed: groups, the API pages by group (all 38), module addresses,
  the count (46 commands, 45 + 10 = 55 reference pages, 25 evidence pages), mcflirt's page, one
  source (docs/api/ + an exporter), a redirect page type, the addresses table. Letter case: the
  command's own spelling (the address shows what the reader types; the builder's link check catches
  a wrong case).
- 11 (mechanics): fixed: shared documents per the brief (those this part rewrites edited directly
  and listed for the orchestrator; next-steps, the decision log and CLAUDE.md by handoff; generated
  files through their source); golden re-recording; suite aliases and NAMEs kept; the re-export
  keeps blobs.sha256.
- 12 (example 1, "software"): fixed as suggested (the animation's five colours, which is what it
  shows); "project" for the countable form; home section 4.
- 13-24, 26-31: fixed as suggested (13 "sentences they replace"; 14 example 11; 15 example 12 with
  every fact of the old sentence and "each time a change is added to the code" for "at every
  merge", the same page's table; 16 auto's both halves, AFNI's built in; 17; 18; 19 examples 19
  and 20, the image panel's second line in §4; 20 307 MiB; 21 "the original's own variability"
  everywhere and the byte/bit row; 22 the compared-cases row; 23 one evidence page per page of
  today's content/tools/, bet's three records; 24 all ten columns, from the re-run inventory
  (5,378: documentation-plan.md's corrected lines); 26 the three long passages split; 27 question
  5 and "the companion packages"; 28 rule 11; 29 the band's reason; 30 "program or function",
  nibabel named; 31 the family row).
- 25 (two pages): fixed: packages and licences in full on Concepts, a short list on Getting
  started; the name and FAIR in full on Home, a link on About (the reason under 8).

Round 2 (reviews/round-2-writing-guide.md: 3 important, 9 minor), answered in cb45ecb4 and
47d7cf02. Every finding is fixed; where the fix differs from the reviewer's wording, why:
- 32 (the clean-room rule): the reviewer's replacement text, as the orchestrator decided, with
  one correction: its list of restricted material is completed from the audit's restriction
  table (restrictions.py, R01-R18), which the text now names beside porting-guide §1. The
  reviewer's list left out the probes (R14), the replica authors' random-testing cases (R15),
  the replicas' suite and evidence modules a brief names (R16), the transcripts and other
  agents' scratch (R17, R18), so "treat every file as readable except ..." would have declared
  them readable, the contradiction (c) points at. "The findings marked **Restricted**" became
  "the thirteen files §1 lists, by name": afni-tshift.md and fsl-mcflirt.md say it in other
  words, so a mark-based test would miss them. The sources are "the originals' source that the
  table restricts" (today FSL, AFNI, tedana, FreeSurfer, Workbench, MSM_HOCR, glibc, libstdc++:
  ANTs' and ITK's are not restricted today). "The register's sections marked †": the † marks the
  entries in the summary table, the sections are headed "## Restricted:", so the text says
  both. Point (d): a sentence says the rule widens what the writer guards, never what an
  implementer may read. The header (lines 8-11) points at the rule and says "Nothing comes from
  the restricted material".
- 33 (mcflirt): fixed as suggested; section 1 also says where LARMORX's options stand, with an
  example, so the rollout does not repeat the mistake.
- 34 (records): fixed as the orchestrator decided, and extended to the benchmark reports and the
  clean-room audit's page: section 1's new bullet "Pages made by a measurement" (never changed
  by hand, words kept until made again in the normal course, none re-made for its words, the
  new words in what writes them, the evidence module may reword shown text, a renamed record or
  report byte for byte); the re-export bullet names the provenance's paths and the exporter's
  commit, and says every cited SHA-256 stays. Why the benchmark reports too: the evidence cites
  them by SHA-256 as it cites the records (every benchmark .json, and three hand-written .md:
  afni-volreg, fsl-maths, fsl-prelude), and 11 of the 21 are written by `bench`; the other 10
  were written by hand from a script's measurements, so the bullet says so. The inventory:
  old_words.py's MADE_PAGES moves the records' pages (21 with words to change), the audit's page
  and the 10 reports out of the writer's area; the other docs/validation pages (README, golden,
  difftest, oracle-runs, oracles, oracle-cache) stay the writer's. Section 7 re-made: writer 361
  files, generated 64.
- 35 (several programs per API page): fixed as suggested (numbers checked: flirt 14, bet 3,
  roi 2, topup 2, the other four FSL pages 1 each, 26 with mcflirt's).
- 36 (labels): fixed: "a reference computed another way", "the input" and "the original's
  starting point" in the A/B list; "against the reference" mapped per page (resample-series:
  the replica in fix and in reproduce, which is fMRIPrep's factor, against a reference computed
  another way; sdcflows-fieldmap: the original or the replica against the known answer); section
  5 says what role 'reference' holds (51 ANTs runs with correctly rounded maths and prelude's
  run without the NaN take the original's colour; the computed reference and the exact field the
  grey) and that the evidence module names which.
- 37 (two modes in one chart): fixed as suggested, split into two sentences.
- 38 (gaps, redirects): fixed: rows for "What they are", "FLIRT's other programs" and every other
  section; the redirect page holds a table of anchors (`#data` to bet's reference page), a line
  of script, a link for readers without scripts. One part differs: no lower-case alias pages.
  On the usual file systems of macOS and Windows `antsRegistration/` and `antsregistration/` are
  one folder, so a site built there would let one page overwrite the other. Instead the 404 page
  (which GitHub Pages serves for every missing address) holds the site's addresses, written by
  the builder, and sends an address that matches one in lower case there, anchor kept; the
  table's 404 row says so.
- 39 (bet's lede): fixed: the replica's other 15 (13 crashes, 1 read beyond the image, 1 needing
  an FSL program not replicated yet, from the record's expected divergences) and the clean room
  "198 of its 200 compared cases" before the images; rule 3 now allows "a sentence or two per
  port", since the replica's state needs two; the example links its two records.
- 40 (three wordings): fixed as suggested.
- 41 (ids): fixed as suggested (ANTS-40 and ITK-50 have no † and are in the summary table).
- 42 (shared documents): the orchestrator confirmed the exception; the bullet names it ("the
  documentation revision's exception to CLAUDE.md's handoff rule (the orchestrator,
  2026-11-12)"), says each is listed in the part's notes and checked again at the merge, and is
  split into short sentences.
- 43 (long sentences): fixed as suggested (the writer's bullet, the shared-documents bullet,
  row 19's "lives ... is"); three new long sentences of this round split too.

Round 3 (reviews/round-3-writing-guide.md: 2 important, 2 minor), answered by relay 5 in 6eb5f150
and ea1265b6 (the inventory made again there). Every finding is fixed; where the fix differs from
the reviewer's wording, why:
- 44: the reviewer's replacement text, as the orchestrator decided (the brief-forbidden files of
  `BRIEF_RESTRICTED` and `BRIEF_FILES` in the list; the "readable once exported, wording only"
  clause; the header's "all but `tedana-t2smap-replica`"). Checked against git ls-files: 14
  modules, 22 investigation folders, as the reviewer counted. With the orchestrator's answer to the
  reviewer's question, one more item: the site's pages or sections those modules export (today
  3dvolreg's and t2smap's) are restricted for that software's clean-room implementers, through
  their briefs; implementers work from the specs and runs, never from their software's site pages.
  The header also names "the clean-room audit's restriction table and lists" (verdicts.py is a
  source now) and says the four labels of section 4 come from 3dvolreg's and t2smap's evidence
  (checked: the two "machine" labels from tedana-t2smap.json, the other two from afni-volreg.json)
  and name runs, no source fact.
- The orchestrator's decision on `#data`: a new bullet in section 1, "A program's reference page is
  readable material": the exporter writes it only from readable files; a section whose facts come
  from restricted material stays on the evidence page and the reference page links to it.
  Checked every page of content/: of the sections the guide moved to reference pages ("What it
  is", "What they are", "FLIRT's other programs", `#data`), only `#data` cites a restricted finding.
  Section 6: "One source" names what the exporter reads (relay 4a's fact); the redirect's example
  of a moved anchor is now `/tools/#reading` -> `/validation/#reading` (no tool-page anchor moves
  to a reference page any more); "What it is" stays on the evidence page as its first section,
  "What is compared", the reference page's "what it does" being the API page's own text (moving
  the site's text into an API page would put a page's facts into a readable source for no gain);
  `#data` to the evidence page, linked from `bet`'s reference page.
- 45: fixed as suggested, with full paths (two files are named tedana-t2smap-original.json; only
  docs/benchmarks/'s is linked by the frozen report). old_words.py computes "renamed later" from
  the links of the pages made by a measurement (not a fixed list) and names the linking pages;
  the inventory made again at 6eb5f150 (site d9cf5fa9): 28 renamed now, 5 later, 14 kept. Section
  7 says so, and the investigation's README.
- 46: fixed as suggested.
- 47: wrapped. Section 7's numbers come from the inventory made again: relay 4a's new site pages
  (bet's reference pages, the moved evidence page) raise the writer's row to 365 files.

Round 4 (reviews/round-4-bet-reference.md: 1 blocking, 4 important, 6 minor; round-4-writing-guide.md:
PASS with 2 minor, 48 and 49), answered by relay 7 in 0a61185e and 93a0903b (site 7521371). Every
finding is fixed; where the fix differs from the reviewer's wording, why:
- 1 (blocking, words and tone): fixed in docs/api/fsl-bet.md and bet.py (State, relay 7). The
  reviewer's "What it does" texts taken, bet2's with "deforms a tessellated sphere onto the
  brain's surface" (bet.py's docstring) for "fits a surface to the brain". Ports: the reviewer's
  example-26 text, as a list (each port one item) with the facts of the old paragraph (separate
  program, `pip install larmorx-nc`, the values, the command line); per program one or two
  sentences per port (bet: the clean room's miss on a 3.5-year-old's `bet -B`; bet2: 378 of 380,
  11 of 11 meshes; betsurf: 114 of 116, 34 bit for bit, ds002603's scalp). The replica's numbers
  are the record's for the three programs together (the record does not split them), so each
  sentence says so. The two Status paragraphs did NOT move to the evidence page: this relay may
  not edit it (round 5 runs on it), and the evidence page already holds the records and a History
  (relay 6). They moved into the API page's new `## The records` and `## History`, which every
  reference page links under "More in the repository"; every fact kept (rule 13). The docstring
  fixes as listed; the "Returns a BetResult" paragraph dropped from the API page, its one fact
  the generated table lacked (the mesh's vertices and triangles) moved into BetResult's `mesh`
  description. The `--tanh` rows: the reviewer's text, with "glibc 2.35's, which FSL calls and
  which is the default" for "the C library's (glibc 2.35, as FSL does), which is the default"
  (clearer which one is the default), CORE-MATH kept, "No notice: it is asked for" kept, the
  findings linked (fsl-bet-original.md, readable), the user's decision date moved to History.
  "oracle runs" -> "recorded runs" (the old "twelve oracle runs" are fMRIPrep's runs).
- 2 (next release): fixed from one string (`_not_yet`), under the synopsis and at the top of the
  own options' section, the rows "The variable X in 0.1.0rc1". Not a constant: the exporter reads
  CHANGELOG.md (the options' synopsis in Unreleased and in no released section), so the mark goes
  away by itself at the release; CHANGELOG.md is among the pages' sources.
- 3 (example's claim): fixed as suggested in substance: the claim became a comment line of its own
  ("the clean room of larmorx, or the replica once larmorx-nc is installed"), because the new
  rule allows a comment after an expression only as a checked output; a second call chooses the
  clean room and shows `# -> 'clean-room'`, which the test checks. The test and rule 11 say:
  "An example shows an output only after an expression, as `# -> <repr>`, and the test checks
  that the run gives it; a description goes on a line of its own."
- 4 (drift gaps): (a) fixed as suggested (usage-only, the `-t` drift on a mutated copy of bet.rs,
  plus a stale usage-only and the mirror's own clean pass). (b) both alternatives: the site's
  build checks the hashes when the main repository is at hand, and verify-merge.sh --site runs
  `reference all --site --check`; defects.rs (and CHANGELOG.md) added to `sources`.
- 5 (replica crate link): fixed (the package named, docs/licensing.md linked), and the exporter
  refuses any link to restricted material on a reference page (a drift test proves it).
- 6 (help verbatim): the HTML already held `0-&gt;1` exactly; Inter's contextual alternates
  (`calt`, on for the body) drew the arrow. Fixed in CSS: `.opt-help` turns `calt` off (checked
  in the screenshots), so the help stays text in the sans font rather than code.
- 7 (header): fixed as suggested (square, neutral status with the dashed experimental kept,
  "made from ... and the code", "every comparison with FSL"; the Evidence section's line too:
  "measured against FSL").
- 8 (threads default): fixed as suggested ("Default: every logical CPU on the command line (`0`),
  `1` in Python": "logical CPU" is cli.rs's and CHANGELOG's word).
- 9 (Copy on synopses): fixed: no Copy button, but only on blocks marked `text synopsis`, not on
  every `text` block: the installation page's `text` block (a notice to look up) keeps its button,
  and its page inventory stays as it is. Rule 11 says so.
- 10 (same Ports text): fixed through 1 (per-program sentences; the shared text is now three short
  items).
- 48 (guide): fixed as suggested, plus "The exporter's checks refuse such a link".
- 49 (guide): fixed as suggested, in "One source", with CHANGELOG.md among what the exporter reads.
- Relay 6's pastel line (§5): taken, with the three values and the chip's ink, from pastels.py's
  criteria (neighbours >= 10 normal, >= 9 protanopia and deuteranopia, any two >= 7, ink 4.5:1),
  and why pastels cannot reach the verdicts' 18 and 11.

### Round 5 (relay 8): round-5-bet-evidence.md, round-5-home.md, round-5-programs.md
Every finding is fixed; where the fix differs from the reviewer's text, the reason is given.

**bet-evidence**
- 1 (old words visible, important): fixed. data/verdicts.yml: "Deliberate correction" (every
  page's legend). "(the record's list of corrections, “D13 fix pass”)" -> "([the record's](...)
  list of what `fix` changed)", linked: each case's reason to its port's record (replica 6.0.7.17
  -> fsl-bet-replica-full, 6.0.7.23 -> fsl-bet-replica, clean room -> fsl-bet), the examples'
  notes and facts to the replica's standard record they name (the presentation refuses a note it
  cannot place). "catch-up": heading "Random testing of both ports" (anchor #trial kept), first
  sentence as suggested, History "The random-testing run of 909 calls of both ports, after it."
  "the user's decision" (4 on the page, 14 in the JSON): "a decision of the project, recorded
  with GLIBC-06 / GLIBC-07" in the prose (ids linked to the register's summary table), "decided
  that day" in History, "as decided on 2026-11-04" in the JSON's reasons. Also gone from the
  JSON: "relay 7's open class" (-> "a class already open before this run"), "relay 7's rules"
  (dropped: the findings file is named in the same sentence), "Fixed after the trial (relay 10)"
  (-> "Fixed after this run"), "the trial's cases 622 and 677" (-> random testing's).
- 2 (random-testing table, important): fixed as suggested: "Random testing, both ports against
  the original: FSL `bet` 6.0.7.23 (909 random calls, 2026-11-05)" (count and date from the run's
  summary) and "The clean room's differences from the original, by category"; the note says
  "A random-testing run of both ports, 31 minutes long" instead of "Differential random testing's
  trial". The table titles keep their letter case on the ported page (`bet`, not BET), as the
  charts do.
- 3 (default panels mix versions, minor): fixed, the reviewer's first option: the first four
  examples show FSL 6.0.7.23 and the replica's 6.0.7.23 run by default, the comparison "replica −
  FSL bet 6.0.7.23" among the default differences and in the viewer's A/B; 6.0.7.17 stays in the
  chooser. No still was drawn: 6.0.7.17's stills stand for 6.0.7.23's, which the presentation
  allows only where both versions and the replica's two runs are identical in every output (it
  checks the three comparisons' metrics). Left: t1w-robust's default keeps the replica's
  `reproduce` run (against 6.0.7.17; no 6.0.7.23 `reproduce` run exists), and the yardstick
  "fMRIPrep's build − FSL bet 6.0.7.17" (no 6.0.7.23 counterpart).
- 4 (case table too wide, minor): fixed: port headings "replica 6.0.7.17", "replica 6.0.7.23",
  "clean room"; measured 1,058 px in the 1,058 px frame at 1440 (was 1,091), case column 94 px.
- 5 (links into findings/fsl-bet.md, minor): marked, the reviewer's second option ("written from
  FSL's source: for the replica's authors"): the orchestrator's decisions keep restricted-derived
  facts on the evidence page (#data) and move restricted links from readable pages "to the
  evidence page or a restricted findings index", so the links stay here, labelled.
- 6 (raw category ids, minor): fixed: `category_labels` in the JSON (fMRIPrep's calls, bet2, bet,
  betsurf, defects of FSL, file formats, real heads, random testing, fslroi, standard_space_roi),
  shown in the agreement charts, the case table and the explorer's category filter (the ids stay
  the filter's values, so the charts' links into the explorer still work).
- 7 (small wordings, minor): fixed: "117 MiB, against FSL's 103; 13 of the 117 are Python's own"
  (docs/benchmarks/fsl-bet.md: "117 against 103 plus the interpreter's 13"); every register id in
  the What this shows lines linked to the register's summary table (`words.link_ids`, outside code
  spans and links).

**home**
- 1 ("Rust, and nothing else", important): fixed: heading "No C, C++ or Fortran", the body's
  first sentence unchanged.
- 2 (planned list, important): fixed: the list follows the roadmap and the plan: 1-5 as before,
  6 "The completeness parts" (next-steps item 7, with the FSL item's "done" sentence kept), 7 "The
  final validation phase", 8 "Release 0.1.0", 9 "Later, AFNI" (3dQwarp, 3dSkullStrip with 3dcalc,
  the analysis track: next-steps' AFNI item 5, "planned for later", decided 2026-10-19). The
  roadmap page gained row 6 (the completeness parts) and a "Later" paragraph (AFNI), so both lists
  agree and the published home page's AFNI text stays (rule 13).
- 3 (two wordings, minor): fixed as suggested ("Each project has a colour, always shown with its
  name."; "Most programs are ports of an original program, ...").
- 4 (tedana's card, minor): fixed: "from the next release" beside the clean room's chip (the
  group's port carries `next_release` when every program's port of that kind has it); the
  Programs page's group header too (programs 7).
- 5 (Replica card long, minor): fixed: the card keeps the definition, the packages and "installed
  apart and run as separate programs"; AFNI's CC BY 4.0 sentence is on AFNI's card (section 02,
  the same fact) and the PROVENANCE.md sentence moved to the Rust card (section 03), until
  Concepts exists. 101 -> 71 words.
- 6 (small points, minor): "main branch" -> "the code in development (the repository's main
  branch)" (intro note and the release item). The header's labels: for the rollout, as the
  reviewer says. The status table's link to tedana-t2smap-replica.md: the orchestrator's call
  (tables.py writes it; listed under "For the orchestrator" in relay 8's state).
- 7: no change needed.
- The user's line is the hero's end title: "Many neuroimaging toolboxes working synergistically
  in *one* package." (the italic on "one" as the slogan had it), the text below unchanged.

**programs**
- 1 (statuses, important): the orchestrator's decision: every status kept, the definition
  corrected (validated: every case passes and the final validation phase's evidence exists;
  experimental: the part's tests pass, that evidence still to come, or a case fails; stable: an
  interface that no longer changes), on the Programs page (a list and a link), Validation (new
  "What each status means", #statuses), the guide (§1) and docs/porting-guide.md (Status). Since
  some ports marked validated have no final-phase evidence yet, each page adds, truthfully, that
  the ports marked validated before this definition keep their status (ANTs' programs with their
  data panel since 2026-10-27, the others because every case passed) and get the rest in the
  final validation phase. The list of those ports is in relay 8's state for the orchestrator.
- 2 (sidebar dots, important): fixed: the sidebar's evidence pages grouped by software, each
  group under a chip with its Programs-page name and pastel, no dot per page; site.yml gives
  t2smap tedana, the two sdcflows pages sdcflows and the resampler fMRIPrep (in the Programs
  page's order); NIfTI I/O stays LARMORX's own. The Validation matrix's dots remain (rollout).
- 3 (Validation #reading, important): fixed with the reviewer's two texts (the licences: CC0-1.0,
  PDDL-1.0, CC-BY, CC-BY-SA with the share-alike notice; the variability: thread counts, builds,
  versions, and an imperceptible change of input, linked to the verdicts).
- 4 (pastels, minor): fixed: pastels.py prints every pair under the four visions and requires
  >= 4 under protanopia and deuteranopia for every pair with a new pastel (all pass; closest
  tedana/LARMORX 4.3); three pairs of today's pastels are under 4 and are printed as information
  (they are not this part's); the guide's §5 keeps the name-always rule and adds that fMRIPrep's
  pastel never stands beside a replica's mark without its chip's text.
- 5 (status badges in pastel, minor): fixed: validated is `status-tag--plain` (ink, solid
  border), experimental dashed, as on the reference pages; the guide's §5 says so.
- 6 (two sources for "does", minor): fixed both ways: `does` comes from the API page's reference
  block (its first sentence) where one exists, and an entry that also gives `does` is a problem
  (bet, bet2, betsurf lost theirs); every docs/api/*.md but upstream-defects.md must be some
  entry's `api`. Tests for both.
- 7 (tedana's group header, minor): fixed (with home 4).

### Round 6 (relay 9): round-6-bet-reference.md
Every finding is fixed; where the fix differs from the reviewer's text, the reason is given.
- 11 (bet -B, important): fixed in both places. bet's Ports: "One of the two others is `bet`'s:
  a 3.5-year-old's `bet -B`. The boundaries of its brain and mask lie up to 2.24 voxels from
  FSL's, where the threshold is 2, and the mask's boundary distance at the 95th percentile is 1.0
  voxel, where the threshold is 0.5. Their Dice coefficients, 0.9989 and 0.9983, are within the
  threshold of 0.995." Differs from the reviewer's text in two points: "the boundaries ... lie up
  to 2.24 voxels from FSL's" for "reach 2.24 voxels from FSL's at most" (says what is measured),
  and both Dice coefficients (the brain's 0.998917 too, from the same record), since the sentence
  names the brain. The clean-room section (old line 510): "The mask's Dice coefficient, 0.9983,
  is within its threshold (at least 0.995), but its boundary distance misses both of its
  thresholds: 1.0 voxel at the 95th percentile, where the threshold is 0.5, and 2.24 voxels at its
  largest, where the threshold is 2; the brain's largest is 2.24 voxels too." Also fixed, the same
  understatement: The records' "the other two miss one" -> "the other two miss thresholds of the
  boundary distance". And the clean room's "198 of its 200 compared cases" now says "of its
  record, which holds the same programs" (below).
- 12 (bet2's numbers, minor): fixed, with a correction to the reviewer's text: the 380 are not
  only `bet2`'s and `bet`'s. The suite counts every image compared by value but the skulls and
  betsurf's (parity/fsl_bet.py `highlights_original`), and in the record's cases about 61 of the
  380 are standard_space_roi's and fslroi's (categories ssroi and fslroi). So bet2's Ports reads:
  "The clean room's record holds the same programs. Of its 380 brains, masks and outlines
  (`betsurf`'s counted apart), 378 are FSL's bit for bit; the two others are `bet -B`'s. 11 of 11
  meshes ...". And every replica sentence (three pages) now names all five programs its record
  holds: "`bet`, `bet2`, `betsurf`, `standard_space_roi` and `fslroi`" (the record's categories:
  51 of the replica's 263 cases are fslroi's and standard_space_roi's); the old "which holds
  `bet`, `bet2` and `betsurf`" undercounted them.
- 13 (old words, minor): fixed: "(the clean room) failed", "is the clean room's option" in
  bet.py; History "decision GLIBC-06"; also the clean-room section's two "the user's decision of
  <date>" -> "decided on <date>". The same message in the 8 other modules: the rollout (other
  parts' files, as decided after relay 7).
- 14 (lede repeated, minor): the reviewer's second option. The exporter keeps the lede (rule 3:
  a lede says what the program does) and "What it does" leaves out its first sentence; a
  one-sentence text still has no lede (betsurf). Tested.
- 15 (renderings, minor): the comment on two lines (seen at 1440: it ends well inside the block);
  "an `UpstreamDefectWarning`" (bet.py's docstring, which the options table quotes).
- 16 (for the orchestrator): the API page's two restricted links stay for the rollout (decided
  after relay 7: API pages follow rule 48 then). The replica's status: `fsl-bet-replica` is
  among the records frozen as validated before the definition (VALIDATED_BEFORE), so it keeps
  its status by the orchestrator's decision; its full comparison (`fsl-bet-replica-full`, FSL
  6.0.7.17, 173 cases) is part of the evidence it has, and the data panel and the long
  random-testing runs on 6.0.7.23 come in the final validation phase, as for every port of the
  list. The rollout's reference pages for 3dvolreg and t2smap: the orchestrator decided after
  round 6 that a reference page may link its own evidence page with no extra mark.

### Round 7 (relay 9): round-7-pilots.md, the programs finding (the orchestrator's message)
- 1 (statuses explanation, important): fixed with the reviewer's replacement text on the
  Programs page and Validation's #statuses, one word added: "most of AFNI's" for "AFNI's"
  (3dvolreg's clean room is experimental; rule 7). The Programs page adds "([the full
  list](/validation/#validated-before))"; Validation keeps its first sentence's facts ("every
  code path once and the original's real use"; the extensive tests wait for the final phase),
  then the reviewer's text with "(all of them below)". The guide's §1 sentence stays (the rule
  going forward), with where the list comes from.
- 2 ("tested where it matters", minor): gone from the Programs page with finding 1's text; on
  Validation "tested as it is written: every code path once and the original's real use".
- 3 (the full list, minor): on Validation (#validated-before), generated: programs.py's frozen
  `VALIDATED_BEFORE` (the 37 records validated on 2026-11-12, tools.toml's statuses that day) ->
  data/programs.yml's `validated_before` -> a list by software under the pastel chips: ANTs and
  ITK (14 replicas), AFNI (3dTshift, 3dAutomask and 3dUnifize both ports; 3dvolreg's replica),
  FSL (the replicas of mcflirt, bet, bet2, betsurf, fslroi, standard_space_roi), sdcflows (4),
  fMRIPrep (resample-series, ndimage-filters, skimage), LARMORX's own (`io`). Frozen, so a port
  validated later is not listed; a frozen record gone from tools.toml stops the export.
  (numpy-linalg is validated but is no entry of the Programs page, so it has no line.)
- 4 (t1w-robust default): no change, as the reviewer says.

### Round 8 (relay 10): round-8-pilots.md (all four pilots PASS; 4 minor)
- 1 (bet's evidence status in FSL's teal): fixed, `status-tag status-tag--plain`, as the
  reference header and Programs show it.
- 2 ("Deliberate correction" in the old words): fixed with the reviewer's text, in plain text
  (the site escapes `means`), keeping LARMORX_UPSTREAM_DEFECTS=reproduce (0.1.0rc1's way) and
  marking --upstream-defects "from the next release" (decision 4, as bet's evidence page says
  it); "the register of upstream defects" -> "the register of the originals' defects".
- 3 (collapsed provenance): fixed through the presentation (words.provenance), the evidence
  exported again from the cache, the same blobs: "Runs of the clean room:", "the comparison
  suite's synthetic head"; the recorded version string quoted, "as the cache entry records it".
  The general rule changed to "the comparison suite's", so example 2's summary and fact (passed
  as "the comparison's synthetic head") now say the same.
- 4 (the rest of Validation): left to the rollout, as briefed; for the orchestrator to tell
  the user with the pilots.

### Round 9 (relay 12): round-9-s1-n4.md (S1: 2 important, 3 minor)
- 1 (ANTS-36 overstated): fixed with the review's texts. N4's corrections paragraph ends "Many
  of the data panel's calls pass `-r` without a mask (ANTS-36): the replica, like ANTs, does
  nothing with it in either mode, and warns. The panel's record names it in 36 cases. In the
  tumour example the values change through ANTS-40 alone."; the tumour example has a SHOWS entry
  in ants_n4.py ("... leaves the zero and negative voxels out of the fit (ANTS-40) and warns
  that `-r` does nothing without a mask (ANTS-36)"); the generator: present.WARNS (ANTS-36,
  with the register's words) makes panel_shows say "corrects" only for the other ids and
  "warns that ..." for these (a d13 line naming only warning ids is refused), and
  present.warnings rewords an example's reason the same way ("corrects ANTS-40 ([register])
  and warns that `-r` does nothing without a mask (ANTS-36)"), so the table's reason and the
  line agree; tested. N4's JSON exported again (6cee019b); no other page names ANTS-36.
- 2 (the presenter carries only ANTs' pages): planned as its own chunk X before S5 (the
  rollout plan), with the probe as a gate; S3 and S4 add their two ANTs shapes (Atropos' seed
  comparison, antsAI's panel without its mode and the known-answer blobs). Note for the gate:
  the probe refuses pages already presented (their labels are new), so it must skip them.
- 3 (N4's lede): fixed with the review's sentence.
- 4 (the band's "ANTs"): the band carries `software` (the page's software, from present's
  `Page`), and the site's band() names it in the key and the tooltips (ANTs' possessive
  "ANTs'" kept for a name ending in s); N4's and DenoiseImage's JSON carry it.
- 5 (random testing's key): left to S14 and chunk E, as the review says.

### Round 10 (relay 13): round-10-s2.md (S2 PASS, 8 minor; round 9 RESOLVED; chunk X's plan)
- 1 (antsApplyTransforms' records at 4 threads): fixed with the review's texts ("What is
  compared" and the data-panel paragraph); both records' Reference lines say "run with 4
  threads".
- 2 (the ledes' "where glibc's ..." clause): the three S2-era ledes (N4, DenoiseImage,
  antsApplyTransforms) and S3's two give the counts in two sentences, the cause last; the
  pattern is in docs/writing-guide.md rule 7.
- 3 (glibc's functions named three ways): "`exp`, `log`, `sin` or `cos`" in the lede and the
  verdicts paragraph.
- 4 (ANTS-44's What this shows): the review's text (ants_denoise.SHOWS).
- 5 (the replica against fMRIPrep's output on DenoiseImage): the review's clause added; no
  second chip (the guide: one chip per port).
- 6 ("twice at 8"): the example's summary reworded on the page (Page.texts) to "(six times at 8
  and three at 12; two of the runs at 8 shown)", as its Repeats fact and chain say.
- 7 (titles): the agreement chart over two records ("standard comparison and data panel")
  links both by name and says "cases of the two records" (tools/evidence.py: the panel record
  `<record>-panel.md` from the provenance; the inventory's links +1 on each such page instead of
  −1 had it linked neither); DenoiseImage's band "32 of the 33 cases that differ from it as
  built" (the drawn cases and those its note names as not drawn).
- 8 (`--float 1` broken in the lede): `.lede > code { white-space: nowrap }` (site.css).
- Chunk X's plan: X1-X4 and the two smaller points added to the rollout plan (chunk X).

### Round 25 (relay 28): round-25-p2-e.md (privacy PASS; round 24 RESOLVED; P2 and E CHANGES REQUIRED: 2 important, 10 minor)
All fixed in step 1 (repository 73e32390, 77b7842d, b8cddfd5; site bd4a694, d8b706f, 737fb9a):
- A (important): the roadmap's "Under way and next": FreeSurfer 8.2.0 "In progress: its preparation done on 2026-11-12
  (the inventory of its code, the queue of its ports, the replay tool); its foundations, the shared library, under way"
  (the reviewer's text) and an unnumbered row for the documentation revision, in progress; the home page's planned list
  (data/status.yml's hand-written `planned`, not generated): the documentation revision first, in progress, and
  FreeSurfer's entry with the same state. The confounds' card no longer promises "its line in the table above" (the
  merge's `tables --site` adds that line). Own commit bd4a694.
- B (important): display-only word map: the site's renderer `DIFFTEST_CHECKS` (tools/evidence.py, every difference's
  failed checks: the four shorthand names in words, the others in code; own commit d8b706f) and, for 3dvolreg's own
  triage table (a table of afni-volreg.json, not difftest.json), `difftest.CHECK_WORDS` applied by afni_volreg.present
  (77b7842d, exported again). triage.toml and difftest.json's check names unchanged. 0 raw names left on any page.
- C: Validation's Agreement bullet: the reviewer's text (within the original's own variability, deliberate correction,
  open difference). D: the Python originals' sentence. E: "about a quarter of an hour". F: LARMORX's own colour.
  J: the data panel's sentence. K: README's `ev.random_testing_runs()`.
- H: the runs table's total counts a run of two ports once (1,398,782; the title says so); "runs of 10 to 48
  minutes, most of them half an hour"; Unclear's hover "timed out or crashed: not compared" (both tables).
- I: the triage's words through difftest._prose (`_TRIAL`: a trial as a random-testing run, "the clean room's own,
  from its specification", the run's folder as "(its run of <date>)"; difftest.json exported again: 5 texts); the
  long-run caption "21 programs". Seeds and run ids that hold "trial" stay (identifiers).
- L: resample_series._poster_of_identical: the added default panel fmriprep-newest takes the poster of a panel it is
  compared with and identical to in every shown value (lx-newest: differ 0): e9fd1adae7ca12fb63c8d09e.webp, no new
  blob; images on the page 21 -> 22.
- M: nifti_io TEXTS "for smoke tests" -> "for the quick check". N: one order, the replica before the clean room, in the
  matrix (rows sorted, pointers kept), the long-run table's port names and the runs table (rows by program, date,
  replica first; pointers kept).
- Notes: the matrix's note (summary.py) names the deliberate correction and nibabel's variability for LARMORX's own
  NIfTI reader (summary.json made again: 1 text). The verdicts' hover text (verdicts.yml's `means`) left for step 2.
- Checks: build 0 problems; numbers on load Validation 551/551, resample-series 83/83 (77/77 with 8 viewers), nifti-io
  82/82, afni-volreg 683/683, afni-tshift 174/174, afni-automask 156/156, Atropos 459/459, the data panel 30/30, mri-hmc
  261/261; summary_check 0 differ, panel_runs_check 0, site_paths_check 0, tables and reference checks clean;
  inventory against d8c07df (tools/r28/inventory-r25.md): every page after >= before, no shown number less;
  blobs.sha256 unchanged; screenshots shots-r25/.

### Round 24 (relay 27): round-24-s14-p1.md (round 23 RESOLVED; S14 CHANGES REQUIRED, 3 important, 9 minor; P1 CHANGES REQUIRED, 1 important, 6 minor)
All 19 fixed (repository 1728dd54, 88db9f47 and c6f11991's difftest rule; site d8c07df; the privacy list by M1's
c184170b / f9de904):
1. NIfTI I/O's lede: the reviewer's text (235 compared, the 5 one-library refusals named, 1.0 to 6.0 times with the
   smallest file named); site.yml's summary then holds.
2. The resampler: 136 of its 137 compared cases in the lede, the verdicts paragraph and site.yml, the other named.
3. NIfTI I/O's verdicts in its own words: nifti_io.present sets `verdict_labels` ({"within-variability": "Within
   nibabel's own variability"}); tools/evidence.py `vl(tool, vid)` names a verdict wherever a page shows it (worked
   examples' chips, metrics, agreement rows and legend, tables, the case table, whose filter takes its labels from
   the rendered chips): 70 on the page, 0 "Within the original"; line 30 says so too.
4. Concepts' #settings and lede, Getting started's sentence: `implementation` only where a program has two ports
   (the reviewer's texts).
5. The resampler's description, Records fact and What is compared name fMRIPrep's development version 26.0.0.dev1.
6. Its native example shows `fmriprep-newest` by default (present: inserted before `fmriprep`); no new blob
   (blobs.sha256 unchanged).
7. Its provenance names `resample-series/fmriprep-ds005040-native-numerics` (provenance.cache, appended last so the
   list keeps its order; the example's Runs line, by the exporter).
8. Random testing's open status: "open: not yet fixed or explained (a clean room's difference is reported for its
   agent)" (tools/evidence.py DIFFTEST_STATUS).
9. In finding 1's lede.
10. "The quick check's" for smoke-tier (nifti_io TEXTS, 2 descriptions).
11. "(macOS on Apple silicon, Windows)" in the bullet, the 69 reasons and the table's note (nifti_io TEXTS, from
    numpy's documentation of `numpy.longdouble`); the register's NIB-04 ("80-bit on Linux x86-64, 128-bit on Linux
    aarch64, 64-bit on macOS arm64 and Windows") is the orchestrator's: HANDOFF.md.
12. larmorprepx said what it is, with the roadmap's link. 13. The PIPE-03 sentence: the reviewer's text.
14. Concepts' Why clean rooms exist: the reviewer's paragraph (tedana, glibc's functions as clean rooms only, the
    authors, the one-port programs later); the lede's end. 15. The four rules in #defects. 16. About's licences.
17. Getting started: `<family> <program>`, the lede. 18. Why Rust and Python: "For `reproduce`, the setting ...",
    the closing slogan gone (its first sentence says it). 19. The roadmap's link "Getting started".
For P2 (its notes): Validation and the roadmap are in the new words now; summary.json's note reworded (summary.py,
exported again: 1 text); the sidebar's indentation of Why Rust and Python not done (a layout change, M2); CITATION.cff
waits for D. Checks: build 0 problems; numbers 551/551 Validation, 82/82 NIfTI I/O, 83/83 and 77/77 with 8 viewers
the resampler, 30/30 the data panel, 1,051/1,051 bet; summary_check 0 differ; site_paths_check 0.

### Round 23 (relay 26): round-23-s13.md (round 22 RESOLVED; S13 PASS, 3 minor)
- **Minor 1 (the estimation page's originals name niworkflows 1.15.4):** fixed in sdcflows_estimation.py's
  `ORIGINALS` (repository 8be96831): "sdcflows 2.17.1, niworkflows 1.15.3 (1.15.4's code on these paths), SciPy,
  scikit-image and numpy", the reviewer's second form, in the five titles (agreement chart and label, speed, the
  records table, the random-testing table); the JSON exported again.
- **Minor 2 (the Corrections sentence):** both pages now give "(on the command line `--upstream-defects reproduce`
  from the next release, `LARMORX_UPSTREAM_DEFECTS=reproduce` in 0.1.0rc1)" as the other pages do.
- **Minor 3 (`contract-audit`):** the description "the per-program contract for these families: every registered
  _core function has its stub, every public function its API row, every source file its PROVENANCE line" and the
  category "the per-program contract (these families)", the reviewer's words (8be96831; the JSON exported again).

### Round 22 (relay 25): round-22-s12.md (chunk T CHANGES REQUIRED, 1 important; S12 PASS, 2 minor; round 21 RESOLVED)
- **1 (important), fixed** (repository 2116d866, site f3c5ac7): the three worked examples chunk T extended show the
  clean room by default: `clean` (its `fix` at 8 threads) among the panels and `cr-ted` (clean room, reproduce − tedana
  26.0.3) among the comparisons, 10 tiles as on FLIRT's first example (the optcom example keeps `fmriprep`). The JSON
  exported again from the cache (nothing computed): export_diff shows only the defaults (5 -> 6 panels, 3 -> 4
  comparisons), the posters' new `clean` panel and `cr-ted` difference and their ranges (unchanged: the fix-rep
  poster's blob is the same), the spread table's two texts and the exporter's hash; blobs: 79 named, 79 kept, 0 lost;
  blobs.sha256 +3 lines (the three `cr-ted` posters; the `clean` panels' posters already existed as blobs), 0 removed.
  Screenshots after scrolling every example into view (the viewer draws near the viewport): shots-r22fix/ (the three
  examples, 1440 and 390, light and dark; the six panels and four comparisons load). Number checks (port 8795):
  1,141/1,141 on load; with every viewer loaded 1,135/1,135, 126 viewer checks, 0 bad.
- **2 (minor), fixed** (site f3c5ac7): after "Rejected by both, 36.": "The record counts these 36 among its 124 passes,
  since both programs' refusals are compared."
- **3 (minor), fixed** (repository 2116d866): the row "The replica at `--n-threads 4`" (a page rewording of the
  export's "larmorx (replica) at ..."), the title "The original against itself, and the replica at four threads:
  tedana `t2smap` 26.0.3 (values that differ from its reference setup)".

### Round 21 (relay 24): round-21-s11.md (S11 PASS, 2 minor; round 20 RESOLVED)
- Minor 1 (prelude's clean room's run dated twice): History's entry is now "2026-11-06 and 07 ... its first
  record and its random-testing run (2026-11-06), 23,780 calls in 30 minutes", the review's text (site
  f284bf8).
- Minor 2 (mri-hmc's History counted "compared" otherwise): "145 of its 149 compared cases within the
  thresholds then, 3 expected divergences and one case, `real/ds003345/default`, just beyond its band", the
  review's text (site f284bf8).
- The note on the audit's list (fsl_prelude.py not in BRIEF_RESTRICTED): for the orchestrator; nothing
  changed here. Chunk E's random-testing strings and the clean room's evidence module edit (8e47a17e): as
  the review says, for chunk E and the merge.

### Round 20 (relay 23): round-20-s10.md (S10 PASS, 3 minor; round 19 RESOLVED)
- **Minor 1** (the clean rooms' records leave out the replica's random-testing cases): fixed with the
  review's words on both pages (site ef89f1f): fslmaths "the replica suite's cases but its five
  random-testing cases, 18 of its own ...", fslstats "... but FSL's own tests (...) and the replica's three
  random-testing cases, BET's call among them, ...".
- **Minor 2** ("LARMORX's implementation of glibc's ..."): "LARMORX's own version of glibc's ..." on the page
  (three sentences) and in the presenter's table texts (fsl_maths.py TEXTS, 380fcfb5: the glibc table's two
  cells and its note, the replica's random-testing note), fslmaths' JSON exported again (4 texts).
- **Minor 3** (the clean rooms' date): the facts "(2026-11-03 and 04)" on both pages; History's 2026-11-04
  entries begin "the clean room complete: every operation and option ([the roadmap](/roadmap/))"
  (fslstats: "every option").

### Round 19 (relay 22): round-19-s9.md (S9 PASS, 3 minor; round 18 RESOLVED)
- 1 (the two cases of 2026-11-07): the review's words in the verdicts list and History, site a6ca1f7.
- 2 ("PRELUDE's specification writer"): "an answer written for PRELUDE's specification", both places,
  site a6ca1f7 (the JSON never had it).
- 3 (the clean room's notices on lsr and vb3D): the review's sentence in Corrections, site a6ca1f7.
- Notes: none for this relay (mri-hmc and t2smap gain the reruns clause at their exports, S11 and S12).
### Round 18 (relay 21): round-18-s8.md (S8 PASS, 4 minor; round 17 RESOLVED, 1 minor; 3dTshift PASS)
- 1 (FAST's Known limits on `multi/s5`): the review's words, site 2fa5b60.
- 2 (the metrics notes' "the record"): `fsl_pages.replica_threshold` turns "The record's threshold:" into
  "The replica's record's threshold:" where the note then gives the clean room's record's, and refuses a
  note without the two sentences; called by FAST's and topup's `present` (topup had the same note);
  repository 1178e9f1 with a test (38 in test_evidence_present.py); the regression gate against 1dcacdeb:
  FAST's six notes and nothing else (tools/r21/gate2.txt, intended tools/r21/intended.txt); FAST and topup
  exported again (FAST alone in a 16 GiB box).
- 3 (the first random-testing run's "realistic draws") and 4 (the C library's knotted sentence): the
  review's words, site 2fa5b60.
- 5 (FLIRT's nested parenthesis): the review's words, site 2fa5b60.
- Notes: topup's re-export (S9) added the reruns to its provenance lines as the review expected (its
  original-* entries: "its runs made again since, last on 2026-11-07"; its replica's: "1 of its N runs");
  the audit's judgement of the clean rooms' evidence modules stays the orchestrator's at the merge.

### Round 17 (relay 20): round-17-s7.md (S7 CHANGES REQUIRED: 3 important, 6 minor; the hardening PASS)
All nine fixed, each as its own commit (repository; site e80fc16 for the page texts, the JSONs exported again):
1. The metrics notes: fsl_flirt.THRESHOLDS (FLIRT's five examples) and afni_volreg.THRESHOLD(S) (3dvolreg's three
   replica examples; the clean room's own example keeps its note) give both records' thresholds, from the clean
   rooms' records' Thresholds tables (docs/validation/fsl-flirt.md; afni-volreg-original.md), the reviewer's
   wording with "the numbers in them read as numbers" (the table's) and "the image from its own matrix" (the
   table's "from the original's own matrix"); each refuses when the old sentence is gone (0c6de004). 3dAutomask's,
   3dTshift's, 3dUnifize's, BET's and FAST's notes already give both.
2. The clean room's fact: fixed at its root, the exporter (fsl_flirt.py's "Original" fact): each run said as its
   comparison finds it, "at 1 thread the same values as at 4; in its default mode another matrix (the comparisons
   below)", which the presenter words "in `fix`, its default, another matrix" (10b6fbab). The export said "differ"
   when either differed; a presenter's guard on a wrong export text would have kept the wrong text in the export
   itself, so the exporter is corrected instead (the reviewer proposed the presenter; the result is its wording).
3. FLIRT's lede: the reviewer's sentence ("Both ports also run the ten other programs FSL builds with it, from
   `convert_xfm` to `applyxfm4D`, and its scripts `pairreg`, `extracttxt` and `aff2rigid`."); Known limits: "every
   command-line program and script", "`flirt_average` and `epi_reg` are not ported yet: they need ..." (e80fc16).
4. History: 2026-11-01 (the replica's parts 1 and 2, with FLIRT's other programs of part 1/2 as the roadmap says)
   and the replica's part 3 in 2026-11-02's entry, from the site's roadmap (rows 70, 71) (e80fc16).
5. "the clean-room catch-up of 2026-11-05, when the clean room took in the specification's answers of 2026-11-03
   (History)" at its first use (e80fc16).
6. The replica's libraries: the reviewer's sentence on FLIRT's page, and the same knot on FAST's page (S8) fixed alike
   (e80fc16).
7. The provenance lines: common.computed(info) says "computed <time>; <k> of its <n> runs made again since, last on
   <time>," before "with larmorx <commit>" (the last run's commit) wherever RUN_INFO.json has a `rerun` list; in
   common.provenance and the six modules that write their own line (afni_tshift, afni_automask,
   afni_volreg_clean_room, fsl_bet_options_clean_room, ants_n4, fsl_prelude_clean_room); a test (d9ff811c,
   1dcacdeb). Of the site's pages, entries with re-runs are on ants-ai, ants-n4, fsl-flirt and fsl-fast (exported again
   now) and fsl-topup, mri-hmc, tedana-t2smap (their chunks export them).
8. The decision of 2026-11-04: "(a decision of the project of 2026-11-04, recorded with GLIBC-07)" on FLIRT's, BET's
   and FAST's pages; the case reasons keep "as decided on 2026-11-04" (e80fc16).
9. afni_volreg_clean_room.present catches every exception (a neutral RuntimeError, from None); a second test with a
   KeyError (922b7dce).
Notes: docs/api/fsl-flirt.md's Not yet no longer lists standard_space_roi (475aa848). For S8: every "original" the
shared rule replaced on FAST's page read phrase by phrase (tools/r20/original-phrases.txt, 129 distinct): each is the
clean room; the role label and group title "the original against itself" (FSL against FSL with correctly rounded
maths) stay, and the case reason "the original against itself with one rounding changed" (multi/s5, the clean room)
became "the clean room against itself", rightly.

### Round 16 (relay 19): round-16-s6.md (round 15 RESOLVED; S6 PASS; the exporter's call SOUND)
- Optional hardening: done (ad37ad48, afni_volreg_clean_room.present raises RuntimeError("3dvolreg's
  page: the presenter refused a shape; a documentation agent must look") from None on the
  presenter's ValueError; test_the_clean_rooms_exporter_hides_a_refusal_of_the_replicas_presenter).
- "32 viewers" in relay 18's report: the number of viewer checks; relay 19 says "viewer checks".
- Notes for the merge (1, 2): the orchestrator's.

### Round 15 (relay 18): round-15-s5.md (S5 CHANGES REQUIRED: 1 important, 5 minor; round 14 RESOLVED; X1-X3 PASS)
All six fixed (repository 5c63398f; site 5f00f86, the three JSONs exported again from the cache, texts only):
1. 3dAutomask's lede and verdicts paragraph: the review's sentences (14 cases, fMRIPrep's calls on images AFNI
   wrote, AFNI-10; the record's lines 11-13 and the agreement chart).
2. The AFNI versions fact: afni_pages.TEXTS turns "the GPL replica's reference since 2026-10-29" into "both
   ports' reference since 2026-10-29" (9 facts on the three pages).
3. 3dTshift's History: "until 2026-10-29: the replica followed AFNI 25.2.09, bit-identical to it in every
   compared case of its record" (the old lede's "(and, before it, 25.2.09)").
4. 3dUnifize's tanhf sentence: the review's wording.
5. "golden tests": the other pages' "cross-platform checks ([how they work](...))" on both pages.
6. The auto parenthesis on all three pages ("before 2026-10-29 only where `larmorx-gpl` was installed, the clean
   room otherwise"); 3dAutomask's History: "2026-10-30: both records made again against AFNI 26.3.05: 130 and 125
   cases" (both records' Generated lines).
Found in passing (relay 18): afni_pages.TEXTS turned 3dUnifize's metrics note "The rows *the original against
itself* are AFNI's own variability" into "*the clean room against itself*" (3 examples): the rules no longer touch
"the original" after an asterisk or in "'... the original's'"; the regression gate --ref 17bd7daf lists exactly
those 3 differences (tools/r18/gate1.txt). Checks after: build 0 problems, 49 pages; numbers 174/174, 156/156,
203/203 on load, with every viewer 166/166, 134/134, 193/193; summary_check 0 differ; blobs.sha256 unchanged.

### Round 14 (relay 17): round-14-r.md (chunk R PASS, 3 minor; round 13 RESOLVED)
- 1 (an old module name only in the from-import form): fixed as the reviewer preferred, (a):
  larmorx_validation/renamed.py, a finder for the old dotted names that the three packages give
  their `_RENAMED`; `import a.b.old`, `importlib.import_module`, `from a.b.old import x` and
  `python -m a.b.old` all reach the renamed module (one module object, its own `__spec__`);
  validation/tests/test_renamed_modules.py tests each form for each old name and `python -m`
  (ba86c095). The README's `python -m larmorx_validation.evidence.fsl_prelude_original` runs.
- 2 (the test fails outside a workspace): the parity suites' entries are skipped when no
  `oracles/` folder is above the checkout (the suites themselves unchanged).
- 3 (the next-steps handoff): written now, <scratch>/HANDOFF.toml (24 `[[op]]` edits for 28
  references; apply_handoff --check ok) with HANDOFF.md; relay M adds the rest.
- Notes for the merge (the audit's judgement of the documentation role's path edits in the
  clean rooms' findings; 9e1d1ff7's "eleven" for thirteen modules): for the orchestrator.

### Round 13 (relay 16): round-13-s4b.md (S4b PASS, 3 minor; round 12 RESOLVED)
- **1 (the port's bytes without their condition): fixed** (site 3096db2): "...writes ITK's file
  byte for byte given ITK's compressed chunks and object times (its own compressed chunks are
  compared by content)", the lede's words and the record's Notes.
- **2 (no Corrections by default): fixed**: a section before Random testing, the other pages'
  switch sentence, and every entry of the API page's D13 table (docs/api/transforms-hdf5.md:
  ITK-12, ITK-13, ITK-14, ITK-15, HDF-01, PIPE-04, PIPE-17, each as that table says what the
  default does), "ITK-16 ... and ITK-17 are refused either way" (the table's last line), the
  register's fMRIPrep column (none, PIPE-04 latent from 11 transforms). The reviewer's "check
  first": the record holds no per-case result of its fix pass (its JSON keeps only failed
  checks; its Notes say "a fix pass reports what the default changes"), so the section says
  what each correction does, from the API page, and not which cases it changes. ITK-17 stays in
  Known limits too (an expected divergence).
- **3 (random testing's paragraph for image programs): fixed**: the paragraph says what the
  descriptor (difftest/tools/itk_transforms.py) draws: text and MATLAB files of one to four
  transforms of every linear type ITK reads, double and float, 2-D and 3-D, composites, numbers
  spelt in many ways, HDF5 files from the replica's writer with and without compression holding
  displacement fields too; read by ITK (an ANTs program writes back what it read, in double and
  single precision), the replica, h5py and nitransforms. Not the reviewer's "every HDF5 storage
  feature the record covers": the descriptor writes its HDF5 files with the replica's writer only.

### Round 12 (relay 15): round-12-s4a.md (S4a CHANGES REQUIRED: 2 important, 3 minor; round 11 RESOLVED)
- 1 (DenoiseImage's panel cases name nodes of the 2026-11-08 runs): DenoiseImage exported again
  (site 4f3093d); its 72 case descriptions now differ from b768293's only by "oracle run" ->
  "fMRIPrep run" and "larmorx" -> "the replica" (0 others, checked by id). The run list no longer
  depends on the variable: `fmriprep_nodes.PANEL_RUNS` (the fourteen, committed, with
  docs/validation/oracle-runs.md as their source) limits `finished_runs()`, so a record made
  again and every export read the same fourteen runs on any machine; the export ran with the
  variable unset. panel_runs_check.py (every run named in a page's data-panel cases and examples
  is one of the fourteen) is in every chunk's checks and in tools/r15/gates.sh: 28 files, 0
  outside. (A figures-cache entry named after a run, "fmriprep-ds000228-...-original" on FLIRT's
  page, is not a run and is not counted.)
- 2 (ImageMath's option sentence): the review's text: "...keeps ANTs' result one switch away:
  `upstream_defects="reproduce"` (on the command line `--upstream-defects reproduce` from the
  next release, `LARMORX_UPSTREAM_DEFECTS=reproduce` in 0.1.0rc1)." page_old_words.py now lists
  any paragraph, list item or cell that offers `--upstream-defects`, `--n-threads` or
  `--implementation` without "next release" (`option` lines): none on the nine presented pages;
  it finds the old sentence on the build before.
- 3 (ImageMath's lede, "the other three"): "With ANTs as built, 303 of the 308 match; three more
  differ only where glibc's `powf` is not correctly rounded, and the two signed zeros as above."
- 4 (Atropos' fix-ants reason): "...97.7% of the mask's 23,984 voxels" (ants_atropos_pv.py
  counts them as the other two reasons do; exported again).
- 5 (optional, antsAI's rotated copy without a chip): kept as it is, the review's alternative: its
  What this shows line already says "the replica's `reproduce` writes the same file". A chip
  needs a comparison with its own arrays and metrics, which the export would have to compute.

### Round 11 (relay 14): round-11-s3.md (S3 CHANGES REQUIRED: 1 important, 7 minor; round 10 RESOLVED)
- 1 (summary.json stale): made again from the evidence JSONs (antsRegistration 584 / 371,
  antsAI 172 / 113); `validation/investigations/docs-revision/summary_check.py <site>` compares
  every implementation of summary.json with its evidence JSON (by record) and is in every
  chunk's checks and in tools/r14/gates.sh. summary.json stays a shared document of the site:
  listed under "Shared documents changed directly".
- 2 (Atropos' defect counts): the review's texts, in the lede and the corrections paragraph
  (checked: 19 register rows ANTS-90 to ANTS-106, ITK-50, ITK-51; the fifteen of the record).
- 3 (the ambiguous lede sentence): the review's text (checked: the category's 1 bit-identical
  and 4 with correctly rounded maths).
- 4 (the phantom's denominator): "of the voxels inside the phantom's mask" in the What this
  shows line; the two reasons give the mask's 23,984 voxels (the exporter counts them from the
  cached truth, no computation beyond a count); the ANTS-106 table's column "voxels of the mask
  as the truth" and its title.
- 5 (the placeholder and the cache path): `<fMRIPrep run>` in facts too (present._ORACLE_TEXTS);
  the priors' folder `<the OASIS30ANTs priors in the T1w's space>/prior%02d.nii.gz`
  (ants_atropos._priors_path); page_old_words.py lists placeholders and paths in code that name
  the oracle, the workspace or the figures cache. N4's and DenoiseImage's `<workspace>/oracles/
  bids-views/...` case commands left to the commands' chunk, as the review says.
- 6 (the permanent random-testing cases): `common.difftest_descriptions` fills each difftest
  case's description from its case file ("random testing's permanent case: ..."):
  antsRegistration's eight and antsAI's one. The commands are not filled: a case file holds a
  recipe (options and generated inputs), not a command line; for each later page where the
  exporter has a command the chunk adds it. The AFNI and FSL pages get it in their chunks.
- 7 ("all 865 lines"): "(addresses, times and paths masked, as the record compares them)" on the
  three template lines.
- 8 (Grubbs-Rosner): the case's reason says the record passes it, the heap's values not
  required, and that the page counts it with the expected divergences.
- The X1 note: the plan's X1 now says so (rollout plan, NEXT); until X1 the presenter raises
  when a hidden run whose id begins with "larmorx" is in a variability comparison (tested).

## FROM THE ORCHESTRATOR
- 2026-11-12, after relay 1 and the reviewer's round 1 (`reviews/round-1-writing-guide.md`: CHANGES REQUIRED,
  1 blocking, 11 important, 19 minor): **relay 2 addresses every finding first** (fix it, or answer under "Review
  answers" why not), then the reviewer runs round 2 on the revised guide; only a PASS goes to the user.
  - Finding 1 (port): the orchestrator corrected `docs/documentation-plan.md` on main (eb15fccc): a port
    is one reimplementation, a replica or a clean room; a program can have both (`bet` has two ports); in Rust, or
    in Python where the original is Python kept in Python. Align the guide with it.
  - **A new rule** (from finding 7, the leak risk): you have read replica material, so in any document a
    clean-room implementer may read (the API pages of programs with a clean room, `docs/porting-guide.md`,
    `docs/architecture.md`, the clean rooms' records and findings, anything not marked restricted) your changes are
    **wording only**: no fact, number, name or behaviour that is not already in that same file or in a public source
    the file cites; new facts only in documents marked restricted or in replica-side material. List those files in
    NOTES.md; the reviewer checks their diffs for added facts.
  - The writer's three questions (guide §8) go to the user with the guide after the reviewer's pass.
- 2026-11-12, after the reviewer's round 2 (`reviews/round-2-writing-guide.md`: CHANGES REQUIRED, 3 important, 9
  minor; all 31 of round 1 resolved, the six alternatives accepted): **relay 3 addresses every round-2 finding**,
  then round 3. The orchestrator's decisions:
  - **Finding 32 (the clean-room rule's wording):** take the reviewer's replacement text (wording only, no new facts;
    facts may come from any file a clean-room implementer may read, named or linked, never from restricted material;
    who may read what: the porting guide's §1). The orchestrator's earlier wording was unclear.
  - **Finding 34 (record pages):** the validation records (`docs/validation/*.md` and `.json`) are generated by the
    comparison suites and cited by SHA-256: **never edit them by hand**. Change the words in the generators (suites,
    evidence modules, `tables.py`) so new records use them; existing records keep their words until each is re-made
    in its normal course (no record is re-made only for wording). Where the site shows a record's text, the evidence
    module's presentation may reword it. Move the 26 record pages out of the writer's area in the inventory.
  - **Finding 42 (shared documents edited directly):** allowed by the brief for the documents this part exists to
    rewrite, each listed in NOTES.md; the orchestrator re-checks them at the merge. Keep the list current.
- 2026-11-12, after the reviewer's round 3 (`reviews/round-3-writing-guide.md`: CHANGES REQUIRED, 2 important, 2
  minor; 33-43 resolved, 38's alternative accepted): **the relay after relay 4** (the guide is not edited while relay 4
  builds the templates) addresses 44-47, then round 4. The orchestrator's decisions:
  - **44:** take the reviewer's replacement text: the files the audit forbids through `BRIEF_FILES` and
    `BRIEF_RESTRICTED` (`cleanroom_audit/verdicts.py`) named in the list, the "wording only for exported text" clause
    and the header's exception.
  - **The reviewer's question (site pages exported by replica-side modules):** a site page, or a section of one,
    exported by a module the audit lists in `BRIEF_RESTRICTED` (today the 3dvolreg and t2smap replicas' evidence) is
    restricted for the clean-room implementers of that tool's family, through their briefs, as t2smap's clean-room
    brief already did for the replica's page. The guide says so in its list; clean-room implementers work from the
    specs and black-box runs, never from a family's site page. Text exported by such a module changes in wording only.
  - **45:** take the reviewer's fix: the five files that frozen benchmark reports link to keep their names until those
    reports are made again (28 renamed now, 5 later); the guide and the inventory say which.
  - 46, 47: as the reviewer proposes.
- 2026-11-12, after relay 4 (4a; the orchestrator): accepted (fd06e351..98aa5258; site d9cf5fa). **bet's `#data`
  stays on the evidence page** (its facts come from restricted findings; a reference page is readable by clean-room
  implementers): the guide says so for every such section. The docstrings' "clean-room original" go into the rollout
  (wording only; the Python wrappers are not clean-room code). Next: **relay 5** answers round 3 (44-47, the decisions
  above), then 4b (bet's evidence page layout); then the reviewer's round 4 covers the guide and the bet pilots so far.
- 2026-11-12, after relay 5 (the orchestrator): accepted (guide 6eb5f150..53699e09; 4b site fe33cb4). The reviewer's
  round 4 now covers the guide (all changes since round 3, from 504d47f6) and bet's reference pages (4a). **Relay 6, in
  parallel:** bet's evidence page prose in the new words, then sub-task 3's other two pilots, the home page (the three
  words early, plain human words, every part it has today kept) and the programs page (grouped by software, toggles,
  ports per program); then the reviewer's round 5 covers the evidence page and these two. Do not edit the guide in relay
  6 unless a pilot shows it must change (say so in NOTES.md).
- 2026-11-12, after the reviewer's round 4 (the orchestrator): **the guide PASSES** (round-4-writing-guide.md: 2 minor,
  48 and 49: fix them with the next relay). **bet's reference pages: CHANGES REQUIRED** (round-4-bet-reference.md: 1
  blocking (the old words in `docs/api/fsl-bet.md` and `bet.py`'s docstring, wording only; the Status paragraphs'
  history), 4 important (mark the new options "from the next release"; an example that claims the replica ran; the
  drift check that misses an option removed from the parser; nothing checks the site's copy against the code: run
  `reference --check` in the merge gates and the site build, read the source hashes; no link to a replica crate's file),
  6 minor). **Relay 7** (after relay 6) addresses them; the guide goes to the user now.
- 2026-11-12, after relay 6 (the orchestrator): accepted (a75f11dc..627c83a6; site f942f66, a1a7f47). /tools/ becoming a
  redirect is accepted as a move, not a loss (its content is on /programs/ and Validation's #reading; the merge's
  inventory compares /tools/ old against those two). **Relay 7** answers round 4's bet-reference findings (1 blocking,
  4 important, 6 minor) and the guide's minor 48-49 (and adds relay 6's proposed §5 line on the software pastels); the
  reviewer's round 5 runs beside it on bet's evidence page (prose), the home page, the programs page and the three
  facts relay 6 corrected in Validation's #reading; then round 6 on the reference pages after relay 7.
- 2026-11-12, **the user approved the guide's §8 recommendations** ("Regarding the other decision and recommendation I
  agree"): (1) the tagline stays in the hero; "Many toolkits, one framework" goes, its replacement being chosen by the
  user from the orchestrator's options (proposed first: "FSL, AFNI, ANTs and more, in one package."; second: "Many
  neuroimaging toolboxes, one package."); (2) ports as lines and labels, verdicts as patterned fills, software colours as
  pastels with their names; (3) the inventory compares everything except words, the reviewer checks page by page that
  every fact stays; (4) the pages describe main, marking what 0.1.0rc1 lacks "from the next release"; (5) developer
  documents keep oracle, upstream, parity and "the main package". Write these into the guide's §8 as decided (the next
  relay). The four pilots still go to the user after the reviewer passes them.
- 2026-11-12, **the user chose the line replacing "Many toolkits, one framework"**: "Many neuroimaging toolboxes working
  sinergically in one package" (written by the user; spelled **"Many neuroimaging toolboxes working synergistically in
  one package."**). The orchestrator offered "working together" as a plainer variant; the user's line stands unless they
  say otherwise. Put it on the home page (the hero's end title) in the next relay, and into the guide's §8 as decided.
- 2026-11-12, after the reviewer's round 5 (bet-evidence, home, programs: CHANGES REQUIRED each; 0 blocking, 7 important,
  16 minor): the relay after relay 7 addresses all of them, with the user's chosen hero line. **The orchestrator's
  decision on the statuses (programs finding):** keep every port's status as it is and correct the definition: since the
  user's testing-depth decision (2026-10-28: each part tests "decent but not heavily extensive", the extensive evidence in
  the final validation phase), `validated` means every case of the record passes **and** the final validation phase's
  evidence exists for that port (the data panel, the long random-testing runs, the full tier); `experimental` means its
  part's tests pass but that evidence is still to come (or a case still fails); `stable` adds a frozen API. Say so on the
  programs page, the validation page and the guide; `docs/porting-guide.md`'s status paragraph (§7, "Status") gets the same
  correction (a shared document this part may rewrite: list it). The sidebar's deep-colour dots and the apricot on
  tedana/sdcflows/fMRIPrep pages follow the guide; the image licences return to Validation's #reading.
- 2026-11-12, after relay 7 (the orchestrator): accepted (0a61185e, 93a0903b; site 7521371; the change to verify-merge.sh
  is reviewed at the merge). **API pages follow rule 48 too** in the rollout: a page readable by clean-room implementers
  (every `docs/api/*.md` of a program with a clean room) links no restricted file; such links move to the evidence page or
  a restricted findings index. The wrappers' "(the clean-room original) failed" messages: in the rollout, all 9 modules at
  once. **Relay 8** answers round 5 (bet-evidence, home, programs; the decisions above: statuses, the hero line); the
  reviewer's round 6 on bet's reference pages runs beside it (do not edit the reference pages or fsl-bet.md in relay 8).
- 2026-11-12, after the reviewer's round 6 (bet-reference: CHANGES REQUIRED, finding 11 important, 6 minor; round 4's ten
  resolved, three alternatives accepted): **the relay after relay 8** fixes 11 (bet -B's failing case: both its 95th
  percentile 1.0 against 0.5 and its maximum 2.24 against 2 fail; the reviewer's wording) and the minors. Decisions:
  - **The status definition, refined:** `validated` = every case of the record passes **and the extensive evidence
    exists**: for ports validated before the testing-depth decision (2026-10-28) their own parts made it (their full
    tiers, panels, random testing); for later ports the final validation phase makes it. Check every port shown as
    `validated` against this; report any that lacks that evidence in NOTES.md for the orchestrator (never change a
    status silently).
  - A reference page may link its own evidence page even where that page is restricted for the program's clean-room
    implementers (3dvolreg, t2smap): a link shows only a name, and their briefs forbid the page; the exporter needs no rule.
- 2026-11-12, after relay 8 (the orchestrator): accepted (bbd8aa6e..19e05fad; site 7d86cfc, 48f5673). The statuses: as
  relay 8 wrote it (ports validated before the definition keep their status; the final validation phase adds the rest);
  the orchestrator updates PLAN.md §4 at the merge. A readable page may link tedana's replica record (a link shows only a
  name). **Relay 9** fixes round 6's finding 11 and its minors on bet's reference pages (not the three pilots relay 8
  changed: the reviewer's round 7 checks those beside you); then the reviewer's last round on the reference pages.
- 2026-11-12, after relay 9 (the orchestrator): accepted (558e161c, 8bd11fa0; site f622e8d, 18afbb5). **Decision:** in the
  rollout, `programs.py` refuses a record marked `validated` that is not on the frozen list of 2026-11-12 unless it names
  its extensive evidence (a field pointing at its final-phase or panel record): new ports are `experimental` until then
  (the confounds part's records included; the orchestrator tells that part). The reviewer's last round on relay 9's fixes
  runs now; on a pass the four pilots go to the user.
- 2026-11-12, **THE USER APPROVED THE FOUR PILOTS** ("Yes, I like it", after 13 phone-width screenshots: home, programs,
  bet's reference, bet's evidence). **Start the rollout** (BRIEF sub-task 4), in relay-sized chunks, the reviewer checking
  each finished chunk while you do the next: (1) a rollout plan in NOTES.md (chunks, order, files per chunk, from the
  inventory's 361 hand-written files, 64 generated, the renames); (2) the site's pages, software by software (every
  evidence page in the new layout and words, from the cache, no recomputation; the Validation page; installation,
  roadmap, data panel and the rest; the sidebar); (3) the reference pages for every program (from every API page, rule 48:
  no restricted link); (4) the repository's documents and API pages in the new words; the 28 renames now (5 later); the
  Python wrappers' messages in the 9 modules; (5) t2smap's three worked examples' clean room (the only computation, from
  the cached oracle outputs); the random-testing table brought up to date; (6) `programs.py` refusing a new `validated`
  without its extensive evidence; (7) the list of clean-room programs' printed lines in old words, for a clean-room agent.
  Every chunk: build 0 problems, the inventory (everything but words, the reviewer checks facts page by page), number and
  viewer checks, screenshots. Keep NOTES.md current so any relay can continue.
- 2026-11-12 ~19:10 (the orchestrator): **relay 11 was stopped by an interruption before committing** (the user's message
  interrupted the turn that launched it). Its unfinished, uncommitted work is in the worktree: a new
  `validation/src/larmorx_validation/evidence/present.py` and edits to `evidence/ants_n4.py` and `evidence/words.py` (the
  N4 page's presentation, started). The next relay (11 again) reviews them, keeps what is right, commits, and writes the
  rollout plan into NOTES.md first, as the entry above asks.
- 2026-11-12, the coherence review (`<scratchpad>/coherence-review-2026-11-12.md`; the orchestrator): in the rollout, also
  findings 8 (the restricted lists in the guide, porting guide §1 and CLAUDE.md-facing texts gain the audit's FreeSurfer rules
  on main b73ac7ab: crates-freesurfer/, docs/freesurfer/, the inventory tools, the borrowed code's sources R19; no FreeSurfer
  inventory fact on a readable page), 9 (the porting guide's track table and checklist: FreeSurfer's clean room planned,
  the permissive ports' clean rooms after FreeSurfer, up to ~6 agents in a burst), 11 (renaming `*-original.md` findings
  trips `restrictions.py` line 541's `-original.md` exception: list the renames for the orchestrator, who updates the audit
  in the same merge), 12 (the README's and the site's roadmaps match next-steps, the release decision and the stream order).
  Main now has the orchestrator's coherence commit aaaba30a (merge main before touching licensing.md, PLAN.md, next-steps).
- 2026-11-12 ~23:10, reviewer round 12 (reviews/round-12-s4a.md; the orchestrator): round 11 RESOLVED; S4a CHANGES
  REQUIRED, 2 important + 3 minor. Fix all five before S4b goes to the reviewer: (1) DenoiseImage's page names nodes of
  the 2026-11-08 runs ("the same call as 1 other node(s): ...-noreconall-syn/..."): re-export DenoiseImage with the
  fourteen runs, and make the run list not depend on an environment variable (take it from the panel record or a
  committed file, or refuse runs outside it); add "every run named is one of the fourteen" to every chunk's checks;
  (2) ImageMath's page dropped LARMORX_UPSTREAM_DEFECTS=reproduce and offers --upstream-defects reproduce without "from
  the next release": the sentence the other five pages use; (3)-(5) the minors as the review says (ImageMath's lede
  "the other three" -> five cases; Atropos' "97.7% of the voxels" "of the mask's 23,984"; antsAI's rotated-copy verdict chip).
  Then the reviewer's round 13: S4b + these fixes.
- 2026-11-12 ~23:40, after relay 15 (the orchestrator): accepted (S4b f31277fd/65f88519, site 4be03f1; round 12's five
  fixes 96c50708/995188ca, site 4f3093d; PANEL_RUNS noted as a shared-code change for the merge). **Chunk R: CONFIRMED,
  each record's .json and .md renamed together** (26 files now, 7 later), so the record loader keeps one name per pair.
  The clean-room audit now accepts both names (`-original.md` and `-clean-room.md` findings, `-original`/`-clean-room`
  permanent cases): local main 8b68a74a (not pushed yet; merge main into your branch to have it). The reviewer's round 13
  (S4b + round 12's fixes) runs beside you. **Relay 16 (next):** chunk R, then chunk X (the presenter for the non-ANTs
  pages, X1-X4); fix round 13's findings first if they arrive while you work.
- 2026-11-12 ~23:30, reviewer round 13 (reviews/round-13-s4b.md; the orchestrator): round 12 RESOLVED; S4b PASS with 3
  minors, to fix in this or the next relay: (1) "Its port": the replica writes HDF5's bytes "given ITK's compressed
  chunks and object times" (16 of 17 files byte-identical), the record's condition; (2) add the "Corrections by default"
  section (ITK-15, HDF-01), the review gives the paragraph; (3) the random-testing paragraph for transform files (the
  review's version). The orchestrator looked at the reviewer's r13 screenshots (1440 light, 390 dark): fine. Keep
  panel_runs_check in every chunk (the FSL and AFNI evidence modules still find the runs folder themselves).
- 2026-11-13 ~00:30, after relay 16 (the orchestrator): accepted (round 13's fixes site 3096db2; chunk R 9e1d1ff7,
  36cbb619, bc80cccf, site 2365d5e; X groundwork d38c0f74). Decisions 1-3 CONFIRMED (suite names follow their records;
  pointer files at the old paths until chunk G's clean-room agent changes the 42 references; old import names kept,
  tested). tools/r16/next-steps.toml goes into the part's handoff at the merge. fsl-fast's export: in S8 with a larger
  box (up to 16 GiB, one export alone). The reviewer's round 14 (chunk R + round 13's fixes) runs beside you.
  **Relay 17 (next):** X1-X3 in present.py (the probe's to-do list, --ref d38c0f74 as the regression gate), then S5
  (AFNI); fix round 14's findings first if they arrive.
- 2026-11-13 ~01:00, reviewer round 14 (reviews/round-14-r.md; the orchestrator): round 13 RESOLVED; chunk R PASS with 3
  minors, to fix in this relay or the next: (1) the reviewer's preference, decided: a small name-mapping finder in each
  package so every form of an old module name works (`import pkg.old`, importlib, `python -m`), with tests; fsl-prelude-
  original/README.md line 47's command then runs as written; (2) the old-names test outside a workspace: make the parity
  module's oracles/ lookup lazy (not a skip); (3) next-steps' 28 references: write them into the part's HANDOFF.toml
  (from tools/r16/next-steps.toml), lines 893 and 923 included. For the merge (the orchestrator): the audit's judgement of
  the documentation role's path-only edits in the eight clean rooms' findings and two doc comments in crates/larmorx-cli
  tests; 9e1d1ff7's message says "eleven" for 13 modules (history stays; the merge message says 13).
- 2026-11-13 ~01:40, after relay 17 (the orchestrator): accepted (X1-X3, round 14's fixes, S5: repository
  c1209efc..17bd7daf, site e71cc6c, ca6c840, 6859f6f). **afni-tshift-replica's record is stamped 3a795ff019fb-dirty on
  main** (a rule breach since 2026-11-04): the orchestrator re-makes it from a clean commit after the current FreeSurfer
  merge is pushed and tells you here; then re-export 3dTshift's page from the clean record (texts only; its blobs stay).
  test_oracle_cache's false-alarm test failing once under load: noted as a flaky test (follow-up), not yours. The
  reviewer's round 15 (X1-X3 + round 14's fixes + S5) runs beside you. **Relay 18 (next):** S6 (3dvolreg); fix round
  15's findings first if they arrive.
- 2026-11-13 ~02:20, reviewer round 15 (reviews/round-15-s5.md; the orchestrator): round 14 RESOLVED; X1-X3 PASS; S5
  CHANGES REQUIRED: 1 important (3dAutomask's lede and verdicts paragraph say the clean room's header differs in "one"
  case where its record lists 14 (AFNI-10): the review's sentences) + 5 minors (the "AFNI versions" fact in 9 places:
  26.3.05 is both ports' reference; 3dTshift's History line for the 25.2.09 bit-identical fact; 3dUnifize's "as every
  LARMORX function does" contradiction (the replica's tanhf is fdlibm's); "golden tests" -> "the cross-platform checks";
  the optional auto parenthesis and 3dAutomask's 2026-10-30 History entry). Fix all before S6 goes to the reviewer;
  the reviewer then takes S6 with these fixes.
- 2026-11-13 ~02:40, after relay 18 (the orchestrator): accepted (round 15's fixes 5c63398f / site 5f00f86; S6
  ec3ccae3, 8a89249f, 9ffe62c1, f124b158 / site 7488ffd). test_sdcflows_app's LAPACK test failing under 4 workers: noted
  as order-dependent (follow-up), not yours. The 3dTshift record is being re-made by the orchestrator (you will be told
  here). The reviewer's round 16 (round 15's fixes + S6) runs beside you. **Relay 19 (next):** S7 (FSL FLIRT) as your
  plan says; fix round 16's findings first if they arrive.
- 2026-11-13 ~02:50 (the orchestrator): **afni-tshift-replica's record re-made from a clean commit**, local main 3bc71341
  (pushed with the next merge): identical case for case to the dirty one (317: 282 pass, 32 both reject, 3 expected
  divergences; no status, reason or measurement changed); only the stamp (629e357ab4aa), the test-data commit, run times
  and two temporary names. Merge main into your branch and re-export 3dTshift's page from it (texts only).
- 2026-11-13 ~03:00, reviewer round 16 (reviews/round-16-s6.md; the orchestrator): round 15 RESOLVED; S6 PASS (no
  blocking or important). Take the optional hardening (decided): in every clean room's evidence module that calls a
  replica module's present()/export (afni_volreg_clean_room.py and any later ones), catch the presenter's ValueError and
  raise a neutral error that quotes no replica text, with a test; say "32 viewer checks across 4 viewers" where a report
  says "32 viewers". For the merge (the orchestrator): the audit's judgement of the documentation role's edits to clean
  rooms' evidence modules.
- 2026-11-13 ~03:20, after relay 19 (the orchestrator): accepted (S7 e877e0d2, d45696f6, 8cbaf253, ad37ad48; site
  55e2abe, 8a28495). **Relay 20 (next): FIRST** merge main (3bc71341, pushed) into your branch and re-export 3dTshift's
  page from the clean afni-tshift-replica record (texts only; say in NOTES what changed: the stamp and nothing else
  expected); THEN S8 (FSL FAST; its export alone in a box of up to 16 GiB). The reviewer's round 17 (S7 + the hardening
  ad37ad48) runs beside you; fix its findings first if they arrive.
- 2026-11-13 ~03:45, reviewer round 17 (reviews/round-17-s7.md; the orchestrator): the hardening PASS; S7 CHANGES
  REQUIRED, 3 important + 6 minor, fix all before S8 goes to the reviewer: (1) the metrics notes' threshold is only the
  replica's ("every file bit-identical"): give both ports' thresholds (the clean room's 0.1 / 0.5 mm and correlation
  0.999), on FLIRT's five examples AND 3dvolreg's three (the same gap, missed in round 16); (2) the "Clean room" fact's
  false thread claim (the 1-thread runs are bit-identical; only fix differs); (3) the lede's script count (six scripts,
  as Known limits and tools.toml say) and Known limits naming flirt_average and epi_reg as not ported; minors 4-8 as the
  review says (7: the exporter's "computed" date from the latest run, every page); 9 decided: catch every exception in
  the clean rooms' calls of a replica presenter, a neutral message. Also: docs/api/fsl-flirt.md still lists
  standard_space_roi as not ported; for S8-S11, check the shared "original" -> "clean room" rule phrase by phrase (role
  labels "the original against itself" must stay where they mean the original).
- 2026-11-13 ~04:20, after relay 20 (the orchestrator): accepted (3dTshift's re-export, S8, round 17's nine fixes:
  repository 2e1723f4..1dcacdeb, site 5661100, 67e196d, e80fc16, b899cc2). **Decided, the -dirty benchmark reports and
  figures-cache entries:** re-measured / re-drawn from a clean commit in the final validation phase, where every
  benchmark is measured again on a quiet machine (benchmarks need one; this machine is busy now). Put that as a
  follow-up into the part's handoff for docs/next-steps.md (the seven reports and the cache entries by name, from your
  list); the pages keep what they show meanwhile. The reviewer's round 18 (S8 + round 17's fixes + the 3dTshift
  re-export) runs beside you. **Relay 21 (next):** S9 (topup and applytopup); fix round 18's findings first if they arrive.
- 2026-11-13 ~04:40, reviewer round 18 (reviews/round-18-s8.md; the orchestrator): round 17 RESOLVED; the 3dTshift
  re-export PASS; S8 PASS with 5 minors (FAST's Known limits: the first channel is a combination of the last two; FAST's
  metrics notes name each record as FLIRT's and 3dvolreg's do; "the replica's realistic draws" in plain words; the
  knotted sentence in "The C library's last digits"; FLIRT's nested parenthesis): fix in this relay or the next. The
  topup, mri-hmc and t2smap pages still print the old provenance line: their re-exports in S9 and later fix it.
- 2026-11-13 ~05:10, after relay 21 (the orchestrator): accepted (S9 feeabaaf, 78a20a42, 5c59a968, 37f93ded, site f99fcbe,
  9da64d6; round 18's fixes 1178e9f1, site a1c1d73, 2fa5b60; dirty_provenance.py 633582b2, handoff op 25). The orchestrator
  looked at the r18 screenshots (topup's note 1440 light, FAST's note 390 dark): fine. The reviewer's round 19 (S9 + round
  18's fixes) runs beside you. **Relay 22 (next):** S10 (fslmaths and fslstats); fix round 19's findings first if they arrive.
- 2026-11-13 ~05:45, reviewer round 19 (reviews/round-19-s9.md; the orchestrator): round 18 RESOLVED; S9 PASS with 3
  minors, fix in this relay or the next: the 2026-11-07 sentence and History entry (difftest/time-step-zero-series did
  not come with the Jacobian answer); "PRELUDE's specification writer" (an internal role on a public page: plain words);
  the Corrections/lsr contradiction (the review's wording: the replica's change is at most one unit in the last place and
  the clean room's default changes nothing there).
- 2026-11-13 ~07:10, after relay 22 (the orchestrator): accepted (S10 a47898af, 23912d04, 8dffe711, b986d806, 629ebc2e,
  site dd06805, b5b5bba, 9a98cec; round 19's fixes site a6ca1f7). The reviewer's round 20 (S10 + round 19's fixes) runs
  beside you. **Relay 23 (next):** S11 (prelude and mri-hmc); fix round 20's findings first if they arrive. Main moved
  (FreeSurfer merges, the audit's rules): merge main into your branch before the S11 exports.
- 2026-11-13 ~07:30, reviewer round 20 (reviews/round-20-s10.md; the orchestrator): round 19 RESOLVED; S10 PASS with 3
  minors, fix in this relay or the next: (1) say that each clean room's record drops the replica's random-testing cases
  (5 for fslmaths, 3 for fslstats) so 37 and 13 add up; (2) "LARMORX's versions of glibc's sin, cos and sincos" (not
  "implementation"), as the other FSL pages; (3) the clean room's date: complete 2026-11-04 (its work and random-testing
  run on 2026-11-03), as status.md and the roadmap say.
- 2026-11-13 ~08:45, after relay 23 (the orchestrator): accepted (S11 + round 20's fixes: repository 3874a564..2b914894,
  site 460def5, ef89f1f, 8b97b25). The reviewer's round 21 (S11 + round 20's fixes) runs beside you. **Relay 24 (next):**
  S12 (tedana t2smap), including the plan's one allowed computation: the clean room on the three worked examples that
  show only the replica (documentation-plan.md, "No recomputation": only what is missing), from a clean commit, boxed;
  fix round 21's findings first if they arrive.
- 2026-11-13 ~09:40, reviewer round 21 (reviews/round-21-s11.md; the orchestrator): round 20 RESOLVED; S11 PASS with 2
  minors (prelude's History: the clean room's random-testing run is 2026-11-06, as its section and table say;
  mri-hmc's History: 145 of 149 for 2026-11-05, by the lede's counting). The orchestrator looked at the r23 screenshots
  (mri-hmc's motion lines 1440 light, its clean-room agreement 390 dark): fine. For the merge (the orchestrator): whether
  every replica evidence module (fsl_*.py, afni_*.py, ...) joins the audit's BRIEF_RESTRICTED as afni_volreg.py did; and
  the documentation role's edits to fsl_prelude_clean_room.py judged.
- 2026-11-13 ~11:00, after relay 24 (the orchestrator): accepted (round 21's fixes site f284bf8; chunk T 0446079a,
  17c4d489, the cache entry clean-room-ds000258-sub-21262-rest-curvefit; S12 921d47bb, 00eae8c8, 06fb0cab, 90b45755, site
  2b2f380, 0f321cf). evidence-blobs is re-made at the merge (tools/blobs.py commit, 1,451 blobs). The orchestrator looked
  at shots-s12/t2s-ds000258-1440-light: the clean room's panel and its five comparison rows are there; the tiles show blank
  in the full-page capture (lazy loading): the reviewer confirms they load. The reviewer's round 22 (S12 + T + round 21's
  fixes) runs beside you. **Relay 25 (next):** S13 (sdcflows' two pages); fix round 22's findings first if they arrive.
- 2026-11-13 ~11:15 (the orchestrator), for later: the confounds part (fMRIPrep's confound estimation, `lx.confounds`,
  `larmorx nipype run`) writes its evidence EXPORT only (relay 4); after it merges to main, a chunk of yours adds its
  present() and its page in the new layout (/evidence/confounds/ or as the export names it). Add it to your chunk list
  after S14.
- 2026-11-13 ~11:45, reviewer round 22 (reviews/round-22-s12.md; the orchestrator): round 21 RESOLVED; chunk T's DATA right
  (only the missing runs, counts equal the record, tiles load); **chunk T CHANGES REQUIRED, important: the clean room is
  hidden by default** (its fix panel and all five comparisons, clean room - tedana included, unticked under "Panels
  shown"): the user's own complaint. FIX FIRST: add the clean room's panels (`clean`) and its comparison with the original
  (`cr-ted`) to the default view in all three examples, as every other two-port page shows them; re-render the three
  posters; presentation only. S12 PASS with 2 minors (say that the record counts the 36 both-refuse cases among its 124
  passes; the spread-table row "The replica (replica) at --n-threads 4" and its table title).
- 2026-11-13 ~09:50, **THE USER'S DECISION: the revision is published in TWO STEPS** (the user: "Yes let's follow the
  recommendation"). **Step 1 (target: this evening, Friday 2026-11-13):** S13 (sdcflows, in progress), S14 (resample-series),
  P1 (home's links, Concepts, About, the header), P2 (Validation reorganised, the data panel under it, roadmap's words,
  404, the site's README/CLAUDE.md), E (the random-testing table from the recorded runs), then **M1**: the merge material
  for step 1 (every page's inventory after >= before against the site's main b768293a, screenshots, the links, the
  build, HANDOFF.toml/HANDOFF.md for the documents step 1 touches, a short report). Pages not yet converted keep their
  current form and stay reachable; nothing the site shows may get worse between the steps. **Step 2 (a day later):** Q1-Q5
  (the reference pages), D1-D4, F, G, the confounds' page (after the confounds part merges), M2. Keep the order; fix the
  reviewer's findings first as before.
- 2026-11-13 ~10:30 (the orchestrator), **for step 1 (by M1 at the latest): machine paths in published exports.** The
  live site's JSONs carry local folder layout in their provenance lines: "~/<workspace folder>"
  (sdcflows-estimation.json) and worktree folder names "<workspace>/larmorx/.claude/worktrees/agent-.../python" in
  PYTHONPATH (ten files: ants-denoise, ants-atropos, afni-volreg, ants-n4, sdcflows-estimation, sdcflows-fieldmap,
  fsl-bet, fsl-flirt, mri-hmc, tedana-t2smap). No account name, but it does not belong on a public page. Fix it once in
  validation/src/larmorx_validation/figures/cache.py's `_short` (every form of the workspace path, absolute and
  ~-relative, -> <workspace>; a worktree folder -> <worktree>; a scratch folder -> <scratch>), with a test that no
  exported string holds a home path, "<projects folder>", ".claude/worktrees" or the user's name; then re-export (from the
  cache, nothing recomputed) every page still holding such a string, and add that check to M1's checks.
- 2026-11-13 ~11:40, after relay 25 (the orchestrator): accepted (round 22's fixes 2116d866 / site f3c5ac7: t2smap's clean
  room in the default view; S13 fc29ff70, 2ca3c0b3, 6bb47904, 10a2c416, f9c39cc3 / site 1711d35, b46854b). The reviewer's
  round 23 (round 22's fixes + S13) runs beside you. **Relay 26 (next): step 1 continues: S14 (resample-series), then P1**
  (home's links, Concepts, About, the header), as far as the relay goes; fix round 23's findings first if they arrive.
  Remember step 1's machine-path fix (`_short`, the entry above) by M1 at the latest.
- 2026-11-13 ~12:20, reviewer round 23 (reviews/round-23-s13.md; the orchestrator): round 22 RESOLVED (t2smap's clean
  room shown by default, checked in a browser at both widths and themes); S13 PASS with 3 minors, fix in this relay or
  the next: (1) the five estimation titles name niworkflows 1.15.3 as the original compared (the records' version; the
  replica is translated from 1.15.4, as the lede says); (2) both pages' upstream_defects sentence in the usual "0.1.0rc1 /
  from the next release" form; (3) the contract-audit case: "the per-program contract of these families".
- 2026-11-13 ~13:00 (the orchestrator), **for step 1 (P2):** the confounds part is finished; its site handoff
  `<scratch-root>/confounds/HANDOFF-site.toml` (7 ops on the roadmap and the status page's planned list; reasons in
  confounds/HANDOFF.md) goes into YOUR site branch with P2's roadmap work (apply them, or their content in the new
  words, so step 1 publishes them), not as a separate site push. The confounds' page itself stays in step 2 (after the
  confounds merge to main, which comes after the FreeSurfer batch now in verification).
- 2026-11-13 ~12:00, after relay 26 (the orchestrator): accepted (S14 a01a4523, 76a7f303, f1d22bff / site 45ca4a6, fda17d6,
  19fffb6; P1 site 19fffb6, 107114e; round 23's fixes 8be96831 / site 50a13a0). **BLOCKING FOR M1: ants-programs.json in
  your site branch carries the full home path WITH THE ACCOUNT NAME in 490 commands** (the live site has the placeholder):
  it must not be published. Fix `_short` (the entry of 10:30) and re-export ants-programs (and every page your relay-26
  list names) BEFORE M1; M1's check fails on any home path, "<projects folder>", ".claude/worktrees", the account name or
  the host name in any file of the site. A FRESH reviewer takes round 24 (S14 + P1 + round 23's fixes). **Relay 27
  (next):** P2 (with the confounds' HANDOFF-site.toml), then E; then M1 with the path fix if the relay has room.
- 2026-11-13 ~12:40, the fresh reviewer's round 24 (reviews/round-24-s14-p1.md; the orchestrator): round 23 RESOLVED; S14
  and P1 CHANGES REQUIRED: 4 important (NIfTI I/O's lede: 235 of 257 compared, the 5 expected divergences in their own
  sentence; the resampler: "136 of its 137 compared cases" in the lede, the verdicts paragraph and site.yml; NIfTI I/O's
  "Within the original's own variability" on a page whose nibabel is never an original: a per-page verdict label or
  the reworded sentence; Concepts and Getting started: not every function takes `implementation`) + 15 minors (the
  review). Fix the four important before M1, the minors in step 1 if they fit, else step 2. **The privacy list for M1**
  (the review's last section): shown on pages: ants-programs.json (420 commands with the full home path), afni-volreg.json
  (3), mri-hmc.json (1), tedana-t2smap.json (1); in the JSON only: fsl-bet, fsl-flirt, sdcflows-estimation (also
  LARMORX_WORKSPACE), sdcflows-fieldmap; not private: the container's /home/fmriprep in three ANTs files.
- 2026-11-13 ~13:40, after relay 27 (the orchestrator): accepted (P2, E, round 24's 19 fixes, the machine-path fix with
  site_paths_check.py: repository d7579286..88db9f47, site ce1205f, 9700380, 806fce6, f9de904, d8c07df). NIB-04's
  register wording: the orchestrator applies it from your HANDOFF at the merge. The reviewer's round 25 (P2 + E + round
  24's fixes + the privacy check) runs beside you. **Relay 28 (next): M1's rest**, step 1's merge material: judge every
  "NO" row of tools/r27/inventory-vs-main.md (a record re-made since b768293a is fine when the page shows the newer
  record's numbers; anything shown less is a finding to fix), screenshots of every changed page at 1440 and 390, light
  and dark, the links, the build, HANDOFF.toml + HANDOFF.md for the repository's documents step 1 touched (the decision
  log's entry of the user's two-step decision of 2026-11-13 included), and a short step-1 report for docs/parts/. Fix
  round 25's findings first if they arrive. Stop when step 1 is ready to merge.
- 2026-11-13 ~14:50, reviewer round 25 (reviews/round-25-p2-e.md; the orchestrator): **PRIVACY PASSES** (0 machine paths or
  names anywhere: tracked files, _site, the 1,454 blobs); round 24 RESOLVED (13 of 15 minors fully, L and M partly);
  P2 and E CHANGES REQUIRED, 2 important + 10 minor. FIX BEFORE STEP 1 MERGES: (1) the roadmap shows FreeSurfer as
  "Planned" with nothing under way, and the home page's status gives it no state: FreeSurfer's preparation was done on
  2026-11-12 and its foundations are in progress (wave 3.1: much on main: the core, maths, MRI structure, the permissive
  pieces, the surface library under way, two clean rooms), and the documentation revision itself is under way: say so in
  the roadmap and the home page's status; (2) the random-testing triage's raw check names ("DN defaults: replica =
  original" where original means the clean room, "DN" unexplained, "larmorx panicked"): a display-only word map,
  triage.toml unchanged. Minors in step 1 if they fit (the non-existent verdict "Within the thresholds"; the runs table's
  total 1,398,782 (BET's 909 counted twice); the replica before the clean room in the matrix and both random-testing
  tables; the resampler's default panel's missing poster), else step 2. NIB-04's register wording: the orchestrator.
