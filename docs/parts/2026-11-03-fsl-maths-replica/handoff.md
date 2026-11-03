# Handoff: the fslmaths and fslstats replica part (2026-11-03, relays 1 to 7 and 6b)

The exact changes are in `HANDOFF.toml` beside this file (32 operations on 14 files). They are
written against **main 05454bc** (topup's clean-room original merged) and the **site's main
0d1dbdc**, and checked there with `apply_handoff.py --check` (every operation applies, nothing
written). Against the worktree's own older copies (main ccd50d0 + 3dd05c7) six anchors differ
(the topup clean-room merge changed them): apply after merging the branch into main.

Branch `worktree-agent-a0c92dea6a2eec1a2`, last commit fb25f03 (tree clean). Site branch
`fsl-maths-page` in `<scratch>/site` (8074099, from 171a8ae) with `evidence-blobs` 3e03069 (897
blobs = 851 + 46 new, the manifest +46 −0); the site's main has moved to 0d1dbdc since: join the
blobs (union) as at the earlier merges.

## Why each change

- **The register (`docs/findings/upstream-defects.md`)**: summary rows FSL-151 to FSL-189 (behaviour
  only, †: their source-derived causes are restricted), after FSL-146; GLIBC-07's row after GLIBC-06
  and its own section after GLIBC-04's (it carries the orchestrator's decision: "glibc's bits" means
  the FMA build); FSL-75's row extended to fslmaths' `-subsamp2` (random testing's permanent case
  `subsamp2-three-wide`; `larmorx_core::defects`'s FSL-75 text already says so, tool still "flirt");
  the restricted header and section title gain fslmaths and fslstats (`fsl-maths.md` covers both);
  a dated paragraph (the rules each entry was decided by, the rule-C list); the restricted section's
  causes appended at the end. FSL-190 unused. **The counts paragraph is yours to redo**
  (`register_counts.py`): on the worktree plus this handoff it gives 376 entries (classes a 90,
  b 13, c 176, d 24, e 58, f 13; c? 10; no decide, no yes); on main add topup's clean-room FSL-144
  to FSL-146.
- **`docs/api/upstream-defects.md`**: rows FSL-151 to FSL-189 and GLIBC-07, the sentence on which
  rows are whose, FSL-75's row.
- **`validation/tools.toml`**: `[[tool]]` fsl-maths and fsl-stats (after fsl-topup), `[[record]]`
  fsl-maths-replica and fsl-stats-replica (after fsl-topup-replica); then `python -m
  larmorx_validation tables --site <site>` (the branch's `tables.py` has their METRICS: `_bet`).
  Rendered on a scratch copy: "replica: 358 of 358 compared cases bit-identical ... fMRIPrep's 18
  calls ...; 61 more rejected by both alike; 2 one unit in the last place ... (GLIBC-07)" and "172 of
  172 ... fMRIPrep's 4 goodvoxels calls ...; 36 more rejected by both alike".
- **`docs/validation/difftest.md`**: the two descriptors' rows; the trial in the cache paragraph.
- **`validation/difftest/triage.toml`**: the part's seven entries; the FFT entry (`register`,
  GLIBC-07) first of fslmaths' "every file bit-identical" ones, as relay 6b ordered them. **The
  file's header lists the statuses fixed, register, open, harness, unclear but not `explained`**,
  which `difftest.md` allows and eight earlier entries use (the 595-notices entry uses it): add
  `explained` to the header's list.
- **`docs/validation/golden.md`**: the `larmorx-nc` row (117 cases, 661 outputs: golden.tsv) and a
  paragraph on the 31 new cases. The main package's registry count is unchanged (208 cases, 1,283
  outputs; record_golden from f56c282 changed only the header).
- **Restricted lists**: `docs/porting-guide.md` §1 and CLAUDE.md rule 1 gain `fsl-maths.md`.
- **README, `docs/status.md`, `docs/next-steps.md`**: the replicas done; item 4's list; item 7's
  done paragraph (after topup's clean-room paragraph); the follow-ups (below).
- **The site**: `site.yml`'s two tool entries (after topup), the roadmap's row 1 (fslmaths and
  fslstats: the replicas complete, the clean-room originals next) and a Completed row (top), the
  planned FSL text in `data/status.yml` (its generated block comes from `tables`). "Last updated"
  stays 2026-11-03. `summary.json`: re-export `summary` (the branch's `summary.py` lists both pages).

## Follow-ups (in the next-steps op)

GLIBC-07 by the `cbrtf` route (the six IBM functions and `sincos`, with their frequencies; the FMA
builds); the clean-room originals from the specs, and BET's and FLIRT's originals checked for
FSL-187; goodvoxels with a real ribbon (once a recon-all oracle run exists); zstd and bzip2 read
whole; D13 notices in chains (the 595 explained notices); the three `validation/tests` that
depend on the caller's environment (inside `boxed.sh` or with `LARMORX_ORACLE_CACHE_MODE`); the
probe-only feature `probe-twiddles` linted by hand only (the command is now in the probe's README).

## For the decision log (at the merge)

- GLIBC-07 (the orchestrator's decisions of 2026-11-03): correctly rounded by default; `reproduce`
  gives glibc's bits for `exp`, `log`, `pow` (permissive code); the six IBM functions and `sincos`
  correctly rounded in both modes until the `cbrtf` route; "glibc's bits" means the FMA build.
- FSL-187 measured first (rule D: no fMRIPrep input reaches it), then rule A, in the robust range
  shared with BET and FLIRT (their records identical).

## Evidence of this relay (sub-task 7)

- Evidence modules `validation/src/larmorx_validation/evidence/fsl_maths.py` and `fsl_stats.py`
  (TOOLS, `summary.py`'s PAGES). Figures-cache entries in `<workspace>/oracles/figures-cache/
  fsl-maths/` and `fsl-stats/` from the clean commit b4b23e2 (FSL served from the oracle cache for
  fMRIPrep's and goodvoxels' calls: 1 + 14 + 4 runs; run through the cache for the new real-data
  calls: 15 families, 8 GLIBC-07 functions, 28 fslstats options); exported from 70d05f3.
- fslmaths' page: fMRIPrep's combine_masks on ds005040 (masks over the reference; the record's four
  calls on three runs in a table, fMRIPrep's own outputs the same), goodvoxels' fourteen calls (the
  ribbon stand-in said in the text, the facts and the panel), one call per operation family on the
  real series (15 outputs, the FFT's sphere-21 mean filter among them: every one FSL's bit for bit,
  in both modes), GLIBC-07 on a real image (`-dt double`: sin 211, cos 84, tan 313, asin 209, acos
  20, atan 200 of 136,400 values one unit from FSL's; exp and log reproduce glibc exactly, their
  default corrects 110 and 15), the record's two expected divergences in plain words (the FFT case's
  verdict *expected divergence*), the trial, 421 cases. fslstats' page: goodvoxels' four calls,
  every option on a real image (29 calls incl. `-t`, `-K`, `-json` and BET's; all FSL's text; the
  default's FSL-182 correction visible on `-S`), the trial, 208 cases.
- Checks: build 0 problems (142 links into the worktree); number check 146/146 (fsl-maths) and
  51/51 (fsl-stats); viewer check 61/61; no "could not be loaded"; `blobs.py verify` 897, 0
  problems; inventory (`validation/investigations/difftest/site_inventory.py`,
  `<scratch>/inventory.md`): every page after ≥ before, both pages new, every other page and every
  earlier evidence file identical (the manifest +46 −0); pages 5.59 MB and 0.26 MB, the site 294 MB.
  Screenshots `<scratch>/shots/after/` (both pages at 1440 and 390, light and dark, several
  positions); no "before" shots: the pages are new and nothing else changed.
- Also in this relay: GLIBC-07's defect text corrected (tan 0.21 %, not 0.25 %: the measurement's
  0.206 %), the findings' sentence on the FMA build, the glibc-double-maths probe's clippy warning
  (a type alias), the probe README's hand lint of `probe-twiddles`, `fft_real_kernels.py`.
