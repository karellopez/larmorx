# The documentation revision, step 1: handoff (begun by relay 17; relay 28 completes step 1's)

The exact changes are in `HANDOFF.toml` (28 operations on 4 files, numbered here from 1 in the file's order;
`apply_handoff.py` prints them from 0), applied **after** the branch is merged:
op 26 changes `validation/programs.toml`, which only the branch has. All 28 were checked
(28 apply) on a trial merge of main `81e665e8` into the branch's head `08da1e80` (relay 28, in a scratch
worktree, nothing committed; `<scratch>/tools/r28/trial_merge.sh` makes it again, `tools/r28/mergetest`).
If main moves again before the merge, repeat it.

## How step 1 merges (checked on the trial merge)
1. **Repository:** merge the branch `worktree-agent-a82773ee467015bd4` into main. Three conflicts, each
   resolved by keeping both sides:
   - `README.md`: only the generated tools table (both sides changed `validation/tools.toml`): take either
     side, then `python -m larmorx_validation tables` writes it (the branch's ports in the new words, main's
     confounds row).
   - `docs/findings/README.md`: the branch's row of `afni-masks-clean-room.md` (renamed) and main's eleven
     confounds rows after it.
   - `validation/src/larmorx_validation/__main__.py`, `PARITY_MODULES`: the branch's
     `"tedana-t2smap-original": "tedana_t2smap_clean_room"` and main's three confounds suites.
2. `python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml` on the merged checkout (28 ops).
3. `python -m larmorx_validation tables` and `tables --site <site>`: the site's `data/status.yml` gains
   the confounds' row in the Ported table (repository links only; the planned card "Confounds, their
   pages" stays until step 2 makes them).
4. `python -m larmorx_validation reference all --site <site>`: BET's three reference pages change only in
   the hashes of the files they are made from (`CHANGELOG.md`, `crates/larmorx-core/src/defects.rs`,
   `validation/tools.toml`, main's), `data/programs.yml` gains `nipypecli run` and the confounds'
   records; 56 programs and modules, 0 problems. Without op 26 the merged tree fails
   `validation/tests/test_reference.py::test_programs_page_data_matches_the_code` (no entry for
   `larmorx nipype run`); with it, `test_reference.py` passes (32 tests).
5. The site's build: 0 problems, 67 pages (on the trial: the merged site copy against the merged
   checkout). `summary_check.py` (46, 0 differ), `panel_runs_check.py` (0 outside),
   `site_paths_check.py` (0), `tables --check`, `tables --site --check`, `reference all --site --check`:
   all clean on the trial. The Python gates on the trial (`validation/tests`, `tests/python`,
   `tests/golden`; main's `_core`, the same Rust; 4 workers, FSLDIR unset): 1,716 passed, 21 skipped, 1
   failed under load that passes alone (`test_difftest.py::test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on`,
   the known flaky one).
6. **The site:** commit the generated files of steps 3 and 4 on the site branch `docs-revision` (head
   `edede81`, 65 commits on b768293a, the site's main: a fast-forward). **evidence-blobs first:** `tools/blobs.py
   commit` (1,454 blobs; the published branch has 1,436; the 18 new ones are all t2smap's clean room in
   `tedana-t2smap.json`, chunk T's only new computation) and `git push --force origin evidence-blobs`
   before the site's main.
7. **Push larmorx main before the site:** the site links 20 repository paths that exist only after the
   merge (the renamed findings and modules); the old paths keep pointer files.

For the merge's clean-room audit (the orchestrator's notes already hold these): the documentation role
(`ROLE_BY_DESCRIPTION` "documentation"), the renamed `*-clean-room*` files against the restriction list's
old names, and the documentation role's edits of three clean rooms' evidence modules (provenance and
texts only: `afni_volreg_clean_room.py`, `fsl_bet_options_clean_room.py`, `fsl_prelude_clean_room.py`;
NOTES, "Shared documents changed directly", relay 20).

## docs/next-steps.md: the renamed files (HANDOFF.toml ops 1-24)
Chunk R renamed 26 files (`*-original*` that name a clean room to `*-clean-room*`, `*_clean_room*`
for Python). docs/next-steps.md is a shared document, so its 28 references to them change through
these ops, made by `validation/investigations/docs-revision/renames.py --handoff` (one op can carry
two references on one line). Two of them name a renamed test or module that no pointer file covers
(the line naming `tests/parity/test_fsl_output_types_original.py`, the line naming
`parity/afni_volreg_original.workspace()`); the rest name findings, which the pointer files at the
old names serve meanwhile. The reference to the record `docs/validation/afni-volreg-original.md`
stays (a record keeps its name until it is made again).

## docs/next-steps.md: the -dirty benchmark reports and figures-cache entries (op 25, relay 21)
Relay 20 found that seven benchmark reports the site cites, and figures-cache entries behind its
evidence pages, record a LARMORX commit ending in `-dirty` (made from a modified checkout); the
records are clean. The orchestrator decided (2026-11-13) that they are re-measured and re-drawn from
a clean commit in the final validation phase, where every benchmark is measured again on a quiet
machine; the pages keep what they show meanwhile. The op adds that follow-up after "Benchmark
reports" in "Follow-ups, when they fit", with the seven reports and the 34 cache entries by name and
commit. **Relay 28 ran `dirty_provenance.py <site>` once more on step 1's site: the same 41 rows (7
reports, 34 entries), every one named in the op**; the op stands as written.

## validation/programs.toml: `larmorx nipype run` and the confounds' records (op 26, relay 28)
`validation/programs.toml` is this part's file (the Programs page's data), on main only after the
merge. Main's confounds part added the command `larmorx nipype run` (`docs/api/nipype-run.md`) and the
confounds' five records; the Programs page's check refuses a command with no entry. The op names the
five records in the confounds' entry (its status from `confounds-fmriprep`: experimental) and adds
`nipypecli run` to the fMRIPrep group (compared with nipype 1.12.0; its record `confounds-nipype`, which
names `larmorx nipype run`; `ports = ["replica"]`, as sdcflows' BIDS app, a Python program the replica
registry does not list; from the next release). Its `does` is the API page's first sentence in short.
The site branch's Programs page makes a card's anchor from its name with spaces as hyphens
(`#fmriprep-nipypecli-run`; no name had a space before).

## docs/findings/upstream-defects.md: NIB-04's platforms (op 27; relay 27, the reviewer's round 24, minor 11)
NIB-04's text says nibabel's long double is "64-bit elsewhere". numpy's `longdouble` is 80-bit on Linux
x86-64 and on Intel macOS, IEEE quad precision (128-bit) on Linux aarch64, and 64-bit on macOS on Apple
silicon and on Windows (numpy's documentation of `numpy.longdouble`); the record measured Linux x86-64
only. NIfTI I/O's page and its 69 case reasons say "(macOS on Apple silicon, Windows)" since relay 27.

## docs/decision-log.md: the user's decision of 2026-11-13, two steps (op 28)
The entry records the user's decision ("Yes let's follow the recommendation"), what step 1 publishes
and what step 2 adds, and the rule between them (nothing the site shows gets worse), with the result of
step 1's inventory.

## Shared documents this part changed directly
The repository's (`git diff <merge base> <branch>`): `README.md` (the generated tools table, from
`validation/tools.toml` in the new words), `docs/findings/README.md` (one renamed row),
`docs/findings/upstream-defects.md` (three paths of renamed findings; words unchanged),
`docs/validation/README.md` (generated: three findings paths), `docs/porting-guide.md` (the Status
paragraph: the orchestrator's corrected definition). The site's: `site.yml`, `data/status.yml`,
`content/validation.md`, `static/evidence/summary.json` (this part rewrites them). Relay by relay: NOTES.md,
"Shared documents changed directly"; the files a clean-room implementer may read, with the sources of
their facts: NOTES.md, "Documents a clean-room implementer may read, changed by this part".
