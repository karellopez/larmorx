# FAST replica (relays 1 to 5, 2026-11-02): the handoff

Branch `worktree-agent-ac6eb2fb8c3621cf1` (from main 0bed438), worktree
`<workspace>/larmorx/.claude/worktrees/agent-ac6eb2fb8c3621cf1`. Site clone `<scratch>/site`,
branch `fsl-fast-page` (from 94729d5). The exact changes to the shared documents are in
`HANDOFF.toml` beside this file (24 operations; `apply_handoff.py --check` passes on the branch's
HEAD with the site clone); this file gives the reasons and what the orchestrator does by hand.

## 1. The register (docs/findings/upstream-defects.md)

Nine rows, FSL-96 to FSL-104 (FSL-105 to FSL-110 unused), every one †: written from FAST's source
through `docs/findings/fsl-fast.md` (restricted). The rows describe behaviour only (no routine,
variable or file names of FSL's), so the clean-room implementer may read the table; the source
lines stay in `docs/findings/fsl-fast.md` and the appended restricted section only points there.
Classes: FSL-96, -97 (memory never written), -100 (abort), -101 (crash), -104 (out of bounds): a;
FSL-98, -103: c (rule A); FSL-99 (meaningless numbers, as BET's FSL-42) and FSL-102 (the shell, as
BET's FSL-44): e. The counts edited in the handoff (312 entries; a 69, c 141, e 53) were checked
with `register_counts.py` on a copy of the branch with the handoff applied: 312 entries, a 69, b 12,
c 141, d 23, e 53, f 12, two explained; no **decide**, no **yes**. The history paragraph goes after
FLIRT's part 3. FSL-96's row says what the record measured: zeros in fMRIPrep's calls, other values
in two multi-channel cases (`multi/s2-outputs`, `multi/s4-n2`: one file each). FSL-98's row adds a
finding of this part: an ordinary two-channel call meets it (MNI152NLin2009cAsym's T1w and T2w named
in that order, so the T2w is channel 1: 2,345,041 values change by default, `multi/s2-outputs`).

`docs/api/upstream-defects.md`: the nine users' rows and one sentence under the table (FAST's rows are
the replica's; FSL-09 and FSL-68 met inside `-a`).

## 2. Random testing (triage.toml, difftest.md)

The trial (`oracles/difftest/2026-11-02-fsl-fast`, half an hour, seed `fast-trial`, 8 jobs at one
thread, 2 GiB per case in a 20 GiB box, the oracle cache in `use` mode): 17,497 realistic cases,
16,768 agree, 725 rejected by both alike, 4 differ, 0 memory, 0 timeout. The four findings (#3230,
#3415, #4273, #4762) are three kinds of one defect, FSL-101 (FLIRT cannot write the registered
priors in `FSLOUTPUTTYPE=ANALYZE`, which FSL 6.0.7.23 refuses; FAST then uses priors it never
allocated). The replica's `reproduce` now does what FAST defines (prints its verbose line first; runs
on with an empty mask; throws `Out of Bounds (time index)` with several channels whose first voxel is
in the mask) and refuses only where FAST writes through a null pointer; three permanent cases
(`validation/difftest/cases/fsl-fast/fsl101-*`, in the record's standard tier). The triage entries
are in the order verbose, multi-channel, empty mask: the empty-mask finding fails both stream checks
and so takes the first entry that shares one (the verbose one); both explanations are FSL-101's.

## 3. Golden (docs/validation/golden.md)

The crates-nc row said 65 cases, 313 outputs (FLIRT part 3's count; main's `golden.tsv` at 0bed438
already held 68 cases, 316 outputs, with the spec answers' three); `golden.tsv` now holds 79 cases,
440 outputs (FAST's 11 cases, 124 outputs, recorded from b42aa20,
committed as bb068f1; every other checksum unchanged). The main package's golden needed no new
checksum (`lx.fsl.fast` is replica-only: `REPLICA_ONLY_FUNCTIONS`), so `record_golden.py` was not
run (relay 4).

## 4. larmorx-nc-core's additive change: mcflirt, BET, FLIRT

FAST added to `larmorx-nc-core` the C library's `expf` and `log` (glibc's FMA paths, every float
input of `expf` verified), `rand`/`srand`, FSL's masked sums and percentiles (`stats.rs`), `FSLDIR`
in the settings table, and LAPACK and BLAS paths for 5 × 5 to 8 × 8 matrices beside the untouched
4 × 4 ones. Nothing existing changed its code path. Relay 4 re-made mcflirt's, BET's and FLIRT's
replica records from the clean commit bb068f1 into `<scratch>/records` (not committed, as FLIRT part
3 did for BET): mcflirt 237 of 237, BET 178 of 178, FLIRT 302 of 302 cases, each identical to its
committed record in status and details (`compare_records.py`); every existing larmorx-nc golden
checksum unchanged. So the merge changes none of their records; the orchestrator's own re-run at the
merge should find the same.

## 5. FSL run outside the oracle cache (relay 3, said plainly)

Relay 3's command-line probe `validation/probes/fsl-fast-cli/probe.sh` runs FSL 6.0.7.23's `fast`
directly, not through the oracle-output cache: about 40 calls of a few seconds each on small inputs
(the usage and option errors, the checks before an image is read, missing and degenerate images,
FSL-99's values, class counts 0 and below, a singular covariance, one class, verbose texts, FSL-100),
run once inside a memory box. Its README says so and points to the suite's `err/` cases, which run
the same paths through the cache and are the checked record. Every other FSL run of the part (the
suite, the record, the trial, the replays, the memory measurements, this page's evidence) went
through the cache in `use` mode; the evidence's six FSL runs were all served from it.

## 6. The site (larmorx_documentation)

The branch `fsl-fast-page` of `<scratch>/site`: `content/tools/fsl-fast.md` (new), the export
`static/evidence/fsl-fast.json` and the manifest `static/evidence/blobs.sha256`, and the local
branch `evidence-blobs` re-made with `tools/blobs.py commit` (the earlier blobs fetched first from
the local repository's `refs/remotes/origin/evidence-blobs`, 2a38d50, 592 blobs: the local
`evidence-blobs` branch is still at 79a1750). Site commit 97101ab; `evidence-blobs` 6977211 (689
blobs: the 592 before, none removed, and FAST's 97). Push `evidence-blobs` (force) before `main`.

The page: six worked examples (fMRIPrep's call on ds000005 as a volume; the bias field `opt/bias-outputs`,
the priors `priors/a-verbose` with FSL-09's correction as the one non-zero difference, three channels
`multi/s3`; the data panel's ds000228 and ds005003 as three full-resolution slices, ds005003's through
the tumour), every image FAST writes as a table, the tissue volumes with fMRIPrep's own maps, and the
86 cases with verdicts (59 bit-identical, 26 rejected by both, 1 expected divergence) and reasons
(the replica's default mode run on every case into the figures cache entry `cases-default-mode`: the
entries it names and its exit status). Exported from larmorx da6ce74 (clean; exporter tree hash
be810c3e); the page is 23.5 MB (budget 40 MB); the site 277.9 MB.

Checks (with the shared changes of the handoff applied in the clone's working tree for the preview,
then restored): build 0 problems with the 127 links into this worktree checked (and 0 for the branch
alone); number check 136 of 136; viewer check 195 of 195 (6 examples); inventory per page
(`<scratch>/py/inventory.py`: every evidence JSON's leaf paths and every built page's words, tables,
rows, numbers, examples, images and links): `<scratch>/inventory-before.json`,
`inventory-after.json`, 0 problems, every page after >= before (each page gained the navigation
entry; summary.json grew; fsl-fast.json and its page new; no other evidence JSON changed, no blob of
another page removed); sizes `<scratch>/sizes-before.txt`, `sizes-after.txt`. Screenshots
`<scratch>/shots/final/`: the FAST page's top, first example and cases at 1440 and 390, light and dark;
the tools index and the roadmap before (main 94729d5) and after (with the handoff), the same four
each; `compare-tools.png` side by side. The preview's shared-file diff: `<scratch>/site-shared-preview.diff`.

For the orchestrator (shared site files):
- `site.yml`: FAST's navigation entry after FLIRT's (in the handoff).
- `data/status.yml`: regenerate with `python -m larmorx_validation tables` (tools.toml's `fsl-fast`
  entry has `page = "/tools/fsl-fast/"` since relay 4).
- `static/evidence/summary.json`: re-export (`python -m larmorx_validation evidence summary --site
  <site>`); `evidence/summary.py`'s PAGES gained `/tools/fsl-fast/` in this branch (584017e).
- `content/validation.md`: nothing to change (it lists no tools by name).
- `content/roadmap.md`: row 2's status cell and a Completed row (in the handoff; "Last updated" stays
  2026-11-02). The site's roadmap may also mention FLIRT's spec answers at the clean-room part 2
  merge (the state file's note).

## 7. Other shared documents

- `docs/next-steps.md`: item 4 (FAST's replica done), item 7 (FAST's paragraph after FLIRT's), two
  follow-ups (`-S 9` and more: the orchestrator's decision, refused in both modes; the rule-D
  measurement of the multi-channel initialisation and bias field).
- `README.md`: the planned FSL list was stale (it still named FLIRT and BET's clean-room original as
  planned): rewritten. The README's tool table is generated (relay 4 ran `tables`).
- `docs/status.md`: the FSL row (FAST's replica, its record) and the FSL bullet; its "Next" sentence
  ("Next: FLIRT's clean-room original's part 2 beside FAST's replica ..., then FAST, topup, ...") is
  left to the orchestrator (the FLIRT clean-room merge rewrites it too): FAST's replica done, FAST's
  clean-room original next beside topup's replica.
- **Main moved while I worked** (CLAUDE.md on disk says FLIRT's clean-room original is complete): the
  handoff's `--check` passes on this branch (da6ce74 + main 0bed438). After merging FLIRT's clean-room
  part 2, re-run `--check`: the operations' anchors were kept short for that reason (item 4 of
  next-steps anchors on "), FAST, topup (item 7); field maps,", the status row on its record list),
  but README's planned list and the register's counts paragraph may need the new wording (the counts:
  recount with `register_counts.py`; my numbers are FSL-96..104's nine added to 303).
- `docs/porting-guide.md` §1: `findings/fsl-fast.md` added to the findings the clean-room implementer
  must not open. **CLAUDE.md** (the orchestrator's): rule 1's list of source-derived findings needs
  `fsl-fast.md` too, beside `fsl-mcflirt.md`, `fsl-bet.md`, `fsl-flirt.md`; and "Next" (FAST's
  replica done).
- `docs/licensing.md`, `docs/overview.md`: no change needed (FAST was already planned in
  `larmorx-nc`; the overview's table of fMRIPrep's stages names FAST as FSL's).
- The decision log: the orchestrator's `-S 9` decision (2026-11-02) and the user's decision of
  2026-11-02 ~15:10 (FLIRT's spec answers before FAST's replica), as the state file says.

## 8. For FAST's clean-room implementer (spec text, behaviour only; not yet in specs/fast.md)

The spec writer may add these to `specs/fast.md` (§5.7, §6 and §11); they are observations of FSL's
and of the replica's results, not of FSL's code:

- **FSL-98 on an ordinary call (observed).** `fast -S 2 -B -b -g -p -o seg <T1w> <T2w>` on
  MNI152NLin2009cAsym's 2 mm T1w and T2w inside its brain mask (the T2w, named last, is channel 1;
  the default image type `-t 1`) sorts the classes by a cycle: the default (each class keeps its own
  statistics) and FSL's result differ in 2,345,041 values of the outputs (the record's
  `multi/s2-outputs`). A two-channel call with the T1w named first meets FSL-98, so the original's
  validation of such calls compares its `reproduce` mode with FSL, and its default with the
  replica's default.
- **FSL-09 inside `-a` (observed).** With FSL's standard priors (`-a` and a matrix, no `-A`) on
  MNI152NLin2009cAsym's 2 mm T1w, the default's rounded priors change 496,961 values of the outputs
  against FSL's truncated ones (`priors/a-verbose`; with `-P` 1,189,247, `priors/a-P-probs`; two
  classes 480,789; two channels 496,228). The same holds for the original: its default against FSL
  differs on every `-a` case; its `reproduce` against FSL is the comparison.
- **FSL-96 is named on every fMRIPrep call** (`-p`): the replica names it in both modes on each of
  the three oracle runs (the maps outside the mask are 0 in FSL's files there, so nothing changes).
- **Times, for the speed target (informal, not a benchmark report).** The replica (one thread for
  the MRF, four for nothing on `-N`) took 15.6 s (ds000005, 1 × 1.33 × 1.33 mm, 0.76 M brain voxels),
  23.2 s (ds000228, 1.23 M) and 31.5 s (ds005003, 1.64 M) for fMRIPrep's call on the main machine;
  the record's case times (FSL's run served from the cache) are in its JSON.

## 9. Notes for the orchestrator

- The evidence module ran FSL only through the oracle cache (`fsl_fast_replica.oracle_run`, `use`
  mode): six runs, all served (the record's own); its seventh entry (`cases-default-mode`) runs only
  the replica (its default mode on every case, for the case explorer's reasons). Figures cache:
  `<workspace>/oracles/figures-cache/fsl-fast/` (7 entries).
- My memory boxes: one at a time, 8 GiB (the evidence) and 8 GiB (a check of the default mode's
  notices, replica only); the other agent's boxes were not counted against mine beyond 32 GiB at any
  moment.
- No process of mine is left running.
