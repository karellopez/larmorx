# HANDOFF: PRELUDE's clean-room original part (2026-11-07)

Branch `worktree-agent-af4a71de4e2ee037f` (worktree `larmorx/.claude/worktrees/agent-af4a71de4e2ee037f`,
from main 1eb168f, 43 commits, last `245282f`); site branch `prelude-original-pages` in
`<scratch>/site` (845fae4 on site main 72ca95e; evidence-blobs `16ca775`, 1209 blobs, the manifest's
SHA-256 e712f344b84113ec). `<scratch>` = `<scratch>`.
The exact changes are in `<scratch>/HANDOFF.toml` (63 operations); this file gives their reasons and
what the merge needs besides them.

## 1. Merge order and the conflicts (measured with a merge trial in `<scratch>/mergetest` and `<scratch>/site-mergetest`)

Main moved to `d25c34f` (3dvolreg's replica merged) while this part ran. `git merge` of the branch
into main conflicts in four files, the site branch into the site's main (f16f910) in one:

| File | Resolution |
|---|---|
| `README.md` | either side, then `python -m larmorx_validation tables` (generated) |
| `tests/golden/golden.json` | both sides recorded: re-record from the merge (`scripts/record_golden.py`, or `merge_golden.py` then the re-record); 240 cases, 1,777 outputs; no value may change |
| `tests/golden/registry.py` | `REPLICA_ONLY_FUNCTIONS = {"lx.afni.volreg": "afni 3dvolreg"}` (lx.fsl.prelude has its original now); comment: "``lx.fsl.prelude`` ran a tool with only its replica (FSL's prelude) from 2026-11-06 until its clean-room original came the same day (its cases below). Since 2026-11-07 ``lx.afni.volreg`` runs a tool with only its replica (AFNI's 3dvolreg)." |
| `validation/difftest/triage.toml` | both sides appended at the end: keep both (this part's 4 `fsl-prelude-original` entries and 3dvolreg's); it parses with `tomllib` after removing the three marker lines |
| site `static/evidence/blobs.sha256` | the union: site main's 1223 lines without the 8 blobs this branch's export replaced, with its 8 new ones: 1223 lines, exactly the blobs the merged pages reference (`<scratch>/handoff/blobs.sha256.merged`); evidence-blobs = site main's `b8cffa4` without those 8, with the 8 of `16ca775` |

Auto-merged without conflict: `validation/tools.toml`, `docs/validation/README.md`, `larmorx-cli`'s
`lib.rs` and `replica.rs`, `test_memory_peak.py`, `__main__.py`, `difftest/permanent.py`,
`difftest/tools/__init__.py`, `evidence/__init__.py`, `evidence/summary.py`, `tables.py`.

The trial (`<scratch>/handoff/trial.sh`: both merges, the conflicts resolved as above, the handoff
applied, `tables --site`, `evidence summary`, the site built with `--main-repo` the merged tree):
63 of 63 operations apply; `tables --check` passes; `register_counts.py` 421 entries, no
**decide**, no **yes** (unchanged); the site builds with 0 problems (28 pages, 161 links into the
repository); `blobs.py verify` 1223 blobs, 0 problems.

## 2. Which operations apply where

- **After both merges** (the order the orchestrator uses): all 63.
- **On main's checkout as it is (d25c34f) with the site's main (f16f910):** 62; op [6], the
  `[[record]] fsl-prelude` block, applies only after the merge (the record block exists only in the
  branch).
- **On the branch alone with the site branch:** 55; [15] `docs/status.md`, [17] golden.md's count
  and [56]-[61] the roadmap's rows cite text that came with 3dvolreg's merge (3dvolreg in status.md,
  the main package's 237 cases, the roadmap's 3dvolreg row), so they apply on main and after the
  merge, not on the branch.
- The seven `validation/tools.toml` block operations [0]-[6] carry the branch's final blocks, which
  the branch already holds: after the merge they change nothing (they reproduce the branch's text on
  main's file).

## 3. The operations and their reasons

- **`validation/tools.toml` [0]-[6]:** PRELUDE's `[[tool]]` (the clean-room original first, its
  speed, its validation sentence) and its new `[[record]] fsl-prelude`; FLIRT's tool and record
  (`real/weights-normcorr` FSL's since the count per point: 239 of 240, `real/weights-woods` left);
  topup's tool (FSL-273 with a coefficient field, 2026-11-07); the head-motion correction's tool (the
  count per point and `leastsquares` as FLIRT's) and record (5 expected divergences, two of them the
  synthetic path divergences within mcflirt's measured spread). The records `fsl-maths` and
  `fsl-topup` keep their texts (placeholders: 386 of 386, 106 of 106).
- **`docs/next-steps.md` [7]-[14]:** "The order from here" (3) done; PRELUDE's replica part's
  follow-ups for the original and from the spec answers done (the FFT item becomes a spec
  question); its site item done at 72ca95e; FLIRT's record failures at three places (one left); a
  new follow-up section "From PRELUDE's clean-room original part" at the top: what the part did
  besides the original (the spec answers, the pair header and the time step in every FSL original,
  the records), what is left (the two hmc path divergences, the hmc's 5 % at one thread, the trial's
  7 open findings), and **the spec questions in one list** (fslmaths §19/§10.2's FFT; prelude §3.3's
  pair header, §3.1/§3.2's time step, §4.11, §4.6/§9.3, the 1e-15 offsets, FSL-271 with `-s`;
  mcflirt/flirt §6.4's parting after ~40 evaluations).
- **`docs/status.md` [15], `CLAUDE.md` [16]:** PRELUDE's clean-room original complete; next
  3dvolreg's clean-room original. (CLAUDE.md's paragraph also gains 3dvolreg's replica, merged.)
- **`docs/validation/golden.md` [17]-[24]:** the main package's count (240 cases, 1,777 outputs
  after the merge: 239 + 3dvolreg's `py.afni.fmriprep_volreg_options`, 1,775 + 2); the part's
  recordings (from 986bbc4 and 1df84f3: 3 cases added, 12 re-recorded on purpose: 6 for the pair
  header and mcflirt's `extents`/`regular`, the rest for the count per point and `leastsquares`);
  the command-line row 72 → 74 and `lx.fsl` 27 → 28 with PRELUDE's cases; the replica's bullet in
  the past tense; a bullet for the original's three cases.
- **The register [25]-[40]** (summary table rows only, and the gap sentence): FSL-261 to FSL-266
  and FSL-268 to FSL-272 gain "the clean-room original the same in both modes since 2026-11-06
  (record `fsl-prelude`)" and their gap **original** → –; FSL-267 "by both implementations"; FSL-273
  "a coefficient field since 2026-11-07 ... | –"; GLIBC-07 names prelude's `-c` arctangent
  (`specs/prelude.md` §6.4: `atan2f`, GLIBC-07 named in both modes where the two differ); the gap
  sentence: **original** only for 3dvolreg's thirteen, **replica** for FSL-191 to -193, -241, -242.
  No new defect: FSL-274 to FSL-280 unused. The counts do not change (421).
- **`docs/api/upstream-defects.md` [41]-[52]:** FSL-261 to FSL-273's rows name both implementations.
- **The site [53]-[62]:** `site.yml`'s prelude (the clean-room original first), FLIRT (answers of
  2026-11-05 and 2026-11-07) and head-motion correction entries; the roadmap: step 1 ("FSL: the
  clean-room catch-up, BET's options, PRELUDE") complete, so it leaves "Under way" and the rows are
  renumbered (3dvolreg first); a "Completed" row for PRELUDE's clean-room original (2026-11-07), in
  plain words. "Last updated" is already 2026-11-07 on the site's main.

## 4. Generated files, after applying

- `python -m larmorx_validation tables --site <site>`: `data/status.yml` (PRELUDE: the original's
  how and validation, 138 of 138; FLIRT 239 of 240; topup 106 of 106; fslmaths 386 of 386; mri-hmc
  145 of 145) and the README's and validation index's tables. The trial's result:
  `<scratch>/handoff/status.yml.after-merge`.
- `python -m larmorx_validation evidence summary --site <site>`: `static/evidence/summary.json`.
  **`evidence/summary.py` changed in the branch (c6e6b63):** its `PAGES` listed only the replicas of
  PRELUDE, topup and BET, though their pages carry the clean-room originals; now each original is
  counted beside its replica (PRELUDE 166 cases, topup 187, BET 252). Topup's and BET's were missing
  since their originals' parts (not this part's; added because the summary is generated and the
  evidence only grows). Also moved: FLIRT's original 209 bit-identical (207), mri-hmc 170 cases,
  fslmaths 454. The trial's result: `<scratch>/handoff/summary.json.after-merge`.
- `content/validation.md` needs no change (it lists no implementation).

## 5. Records of this part (each from a clean commit)

| Record | Commit | Cases | Pass | Both | Expected | Fail | Before (main) |
|---|---|---|---|---|---|---|---|
| `fsl-prelude` (new) | 1ca117e (relay 5; relay 2's from 6321137 identical but stamps) | 166 | 138 | 28 | 0 | 0 | – |
| `fsl-flirt` | f005a08 | 339 | 239 | 89 | 10 | 1 | 339 / 238 / 89 / 10 / 2 |
| `fsl-topup` | fa35411 | 187 | 106 | 67 | 14 | 0 | 185 / 104 / 67 / 14 / 0 |
| `fsl-maths` | f005a08 | 454 | 386 | 63 | 5 | 0 | 452 / 384 / 63 / 5 / 0 |
| `mri-hmc` (`--jobs 1`) | a3f1d34 (relay 4; 1df84f3's statuses) | 170 | 145 | 20 | 5 | 0 | 169 / 145 / 20 / 3 / 1 |

`fsl-bet` (252) and `fsl-fast` (102) were re-run at f005a08 with the time step's change: unchanged,
not committed. The merge's step 5 re-runs: `fsl-prelude` (about two minutes: 152 of 166 FSL runs
from the cache), and the four others if wanted.

## 6. For the orchestrator

- **The clean-room audit** (step 5a): the part's five agents are listed in NOTES.md, with every
  slip. At 3dvolreg's merge the audit already judged relay 1's stand-in script (named `larmorx-nc`,
  a false match of R10 on its name: no slip).
- **mri-hmc's two synthetic path divergences** (your note of 2026-11-07): relay 4 measured mcflirt's
  own spread (`hmc_spread.py`): both within it, kept as expected divergences with the numbers in
  their reasons; no status moved, but relay 4 re-made the record anyway (a3f1d34) so that the record
  and the site's case explorer carry the measured reasons instead of the rejected "open" words.
- **Machine:** relay 3 saw 28 idle orphan Python spawn workers, 1 to 4 days old (parent `systemd
  --user`, about 3 GB resident together), not this part's.
- **Scratch trial trees:** `<scratch>/mergetest` (a registered worktree of the larmorx repository,
  detached, with an uncommitted trial merge) and `<scratch>/site-mergetest`, `<scratch>/site-gen`
  (worktrees of the site clone): temporary; `git worktree remove --force <path>` when done.
- The benchmark `docs/benchmarks/mri-hmc.md` was not re-made after the count per point (about 5 %
  slower at one thread on a loaded machine): a follow-up for the quiet machine.
