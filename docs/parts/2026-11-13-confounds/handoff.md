# Handoff: the confounds' part, fMRIPrep's confound estimation ported (2026-11-11 to 13)

The part's second half, ready to merge (its first half, numpy's and SciPy's linear algebra, SciPy's FFTs,
nitime's AR(1) and nipype's DVARS, is in main since the intermediate merge `89030879`, 2026-11-12). The
shared-document changes for **the whole part** (nothing of either half was in the shared documents) are in
`HANDOFF.toml` beside this file (**106 operations on 18 files**) and `HANDOFF-site.toml` (**7 operations on 2
site files**); this file gives the reasons, how to apply them, the trial merge and what to verify. Relay 5
(agent `a70923b6fb1ce4ef1`) wrote both from a trial merge with main as it is now.

## What to merge

- **larmorx:** branch `worktree-agent-abafb802f99529ccb` (worktree
  `larmorx/.claude/worktrees/agent-abafb802f99529ccb`), from main `7cd35c8c` (relay 2a), main `b13195fb` and
  `ba876f7f` merged in (relays 2e-ii and 2g); the intermediate merge took it to `e69050f6`. **Final head
  `afbf9787`** (relay 5: three of the part's own documents without relay names; relay 4's `6991718a` before it);
  94 commits not in main (`git rev-list --count main..afbf9787`), 245 since `7cd35c8c`. The tree is clean at each relay's end. No clean-room crate, replica crate
  or `crates-gpl/`, `crates-nc/`, `crates-freesurfer/` file is touched (`git diff --stat ba876f7f <head> --
  crates/larmorx-afni crates/larmorx-mri crates/larmorx-fsl crates/larmorx-tedana crates-gpl crates-nc
  crates-freesurfer crates/larmorx-afni-replica specs`: empty).
- **Site:** no site branch (the orchestrator's decision of 2026-11-13: the documentation revision rebuilds
  every page in a new layout; its writer adds the confounds' page after this merge). The export the page is
  made from: `<scratch>/r4/site/static/evidence/confounds.json` and its 47 blobs (3.9 MB, from the clean
  `6991718a`), made again with `python -m larmorx_validation evidence confounds --site <site> --no-compute`
  from the figures cache (`<workspace>/oracles/figures-cache/confounds/`: `workflows`, `gaussian`, `memory`, each
  from a clean commit). `HANDOFF-site.toml` changes only the roadmap and the status page's planned list.
- **The trial merge** (uncommitted, with both handoffs applied, as a preview): `<scratch>/r5/trial`, a detached
  worktree of the larmorx repository at main `bd71286b`, `git merge --no-commit` of the branch at `6991718a`
  (`afbf9787` after it changes `docs/api/confounds.md`, `docs/findings/confounds-blas.md` and
  `confounds-workflow.md` only, files no operation touches);
  the site's `<scratch>/r5/site`, a clone of the site at `b768293a` (its `origin/main`). Remove the worktree
  with `git -C <larmorx> worktree remove --force <scratch>/r5/trial` when done. The generator, which checks
  every `old` text and `after` line in memory before writing, is `<scratch>/r5/make_handoff.py` (with
  `hgen.py`, `register.py`, `notice.py`, `others.py`, `entries_a.py`, `entries_b.py`: the 49 register entries
  as data): `python -P make_handoff.py <trial> <site> <scratch>`; a preview of every changed file in
  `<scratch>/r5/preview/`.

## How to apply (after the merge)

```
cd <merged larmorx checkout>
python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml
python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF-site.toml --site <merged site>
python -m larmorx_validation tables --site <merged site>
python -P scripts/orchestration/register_counts.py
(site) python3 tools/build.py --main-repo <merged larmorx checkout>
```

Checked on the trial (details under "Checked" below): every operation applies (`--check`, then for real);
`tables --site` then `tables --check` exit 0 (the README's tool row, the validation index's five rows, the
site's status row); `register_counts.py` **625 entries** ((a) 147, (b) 18, (c) 308 of them 17 questionable,
(d) 29, (e) 92, (f) 29; no **decide**, no **yes**; one rule 8), the counts paragraph already written with these
numbers; the four NOTICE files equal (`cmp`).

## The trial merge's conflicts

Merging the branch (`6991718a`) into main `bd71286b` stops at **one file**: `tests/golden/golden.json`, the
`recorded` header only; `scripts/orchestration/merge_golden.py` resolves it case by case (334 cases: main's 318,
every one unchanged, and the part's 16); then `scripts/record_golden.py` from the merge commit (every checksum
unchanged expected). Merged by git, to look at in review: `crates/larmorx-core/src/defects.rs` (main's FS
entries, the part's PIPE/PD/SCI entries), `crates/larmorx-io/PROVENANCE.md` and `src/nifti/mod.rs` (the part's
`read_first_volumes` beside main's FreeSurfer readers), `docs/findings/README.md` (both index rows),
`validation/src/larmorx_validation/__main__.py` (`PARITY_MODULES`). `Cargo.lock` and the three other workspaces'
locks: unchanged by the branch since `ba876f7f`; `cargo metadata --offline --locked` passes in the four
workspaces of the trial.

**Minimal shared changes in the branch** (code that had to change, CLAUDE.md "Shared documents through a
handoff"): `crates/larmorx-py/src/lib.rs` (the `confounds` module), `python/larmorx/__init__.py` (`confounds`,
`nipype`), `_core.pyi` (the new bindings at its end), `larmorx_core::defects` (PIPE-41 to PIPE-82, PD-01 to
PD-03, SCI-41), the golden registry (16 cases; `NAMESPACES` gains `lx.confounds`, `lx.nipype`,
`lx.pipelines.larmorprepx.confounds`) and `golden.json`, `crates/larmorx-cli/src/lib.rs` (the family `nipype` in
`PYTHON_PROGRAMS`), `python/larmorx/cli/__init__.py`, `tests/parity/test_memory_peak.py` (its last section),
`validation/difftest/triage.toml` (the confounds' block), `difftest/tools/__init__.py` and `permanent.py`,
`evidence/__init__.py`, `__main__.PARITY_MODULES`; in other parts' crates: `larmorx_io::nifti::read_first_volumes`
(nipype's detector reads only its 50 volumes) with the binding's `volumes=`, `crates/larmorx-ndimage`'s `ball`
(scikit-image's negative radii: a larmorx fix), `larmorx_transform::openblas::lu_sign_and_diagonal4`, and
**`numpy_linalg::DBDSQR_TOLMUL`** (relay 3a-iv: the constant corrected to 2^6.625 correctly rounded; it serves the
field maps' and tedana's code too: below); `python/larmorx/_nipype.py` (PIPE-80, shared with the field maps:
their command-line records unchanged, below); `python/larmorx/sdcflows/pepolar.py` (the quoted words of
`topup_node`).

## The operations and why

- **`validation/tools.toml`** (2; the generated tables): a `[[tool]] confounds` after `tedana-t2smap` (no `page`
  yet: the documentation revision's writer adds `/evidence/confounds/` or the new layout's address with the page;
  `tables.py` lists a tool without a page without a link; `use` names `larmorx nipype run`, so no separate CLI
  row) and five `[[record]]`s after `tedana-t2smap-replica`: `confounds-linalg` (32 cases, 1,673 checks),
  `confounds-linalg-full` (12, 840), `confounds-nipype` (25, 588: relay 2i's re-made record with the command
  line's cases), `confounds-nipype-full` (3, 90), `confounds-fmriprep` (16, 990); every case passes, none
  rejected by both, no expected divergence; all `experimental` (the orchestrator's rule of 2026-11-12). The
  common metrics suffice: no `tables.py` entry. The tool's validation text cites the records by placeholder,
  the register's outcome (29 corrected, 12 refused, 41 with `reproduce`), the benchmark (the 26 Python nodes 1.1
  to 1.4 times fMRIPrep's at one thread, 1.5 to 1.6 at eight; the ported SVD and least squares slower) and the
  memory (at most 0.96 of the originals' peak on the six calls).
- **`docs/findings/upstream-defects.md`** (27): the opening's families (pandas, PD-); "fMRIPrep and its
  libraries"'s introduction names the part; 49 summary rows (PIPE-41 to PIPE-83 after PIPE-40, SCI-41 after
  SCI-32, NP-09 and NP-10 after NP-04, PD-01 to PD-03 after SKI-04: pandas is a family of its own, D2) and 49
  sections in the part B form (What, Evidence, fMRIPrep, larmorx, D13 / gap), PIPE after PIPE-40's, SCI-41 after
  SCI-32's, NP-09 and NP-10 after NP-04's, and a new family section "pandas (PD-)" before "FSL: black-box
  findings"; the rows and sections of PIPE-08 (the motion parameters now ported, fMRIPrep's centre kept in both
  modes; still decided with larmorprepx), PIPE-23 (nipype's own detector the same), NP-04 (numpy's AVX-512
  `power` too; its title) and GLIBC-07 (the cosine basis' sign flips, nipy's functions, the powers; `atan2` and
  `hypot` in SciPy's `as_euler` correctly rounded in both modes, `atan2`'s **reproduce** gap until a clean-room
  `atan2_glibc`, the micro-part next-steps already lists); the gap paragraph (2026-11-13: **measure** for PIPE-79,
  **reproduce** for PIPE-57's float32 series and GLIBC-07's `atan2`, PIPE-42 later); the part's D13 paragraph
  before "Random testing's gaps" (every ID by rule); the counts (625; recounted on the trial). The register's
  classes are the register's, not the rules (a relay's notes wrote "class a/b/c" for rules A/B/C in places):
  (a) the undefined casts PIPE-74 and PIPE-76 and NP-09, NP-10; (b) PIPE-43; (c?) PIPE-48 (a deliberate MKL
  workaround, rule C), PIPE-49 (fMRIPrep's documented `dropped_` entries, rule C) and PIPE-79 (rule D open); (d)
  PD-01, PD-02 (imprecision, with switches, as GLIBC-07); (e) the option, formula and command-line quirks
  (PIPE-54, -57, -58, -62, -64, -65, -66, -69, -77, -80, -81, -82, -83); (c) the rest. The texts come from
  `larmorx_core::defects`' descriptions (with code in backticks), the findings files
  (`docs/findings/confounds-*.md`) and the relays' handoff items; every entry links its findings file.
- **`docs/api/upstream-defects.md`** (5): the 42 rows of the part's `larmorx_core::defects` entries (PIPE-41 to
  PIPE-82 but PIPE-42, -48, -49 and -79, which have no switch; PD-01 to PD-03; SCI-41) after their families'
  rows (PIPE-40, SCI-31, SKI-04), with the functions that apply each; a paragraph after the field maps' part B's;
  GLIBC-07's row names the confounds' functions.
- **`NOTICE`** and its three copies (11 each, kept equal: `crates-nc/LICENSES/`,
  `crates-gpl/LICENSES/NOTICE-larmorx.txt`, `crates-freesurfer/LICENSES/NOTICE-larmorx.txt`): CORE-MATH's
  `hypot`; SciPy's FFTs, 1 × 1 solve, `gesvd` driver, `Rotation` and `center_of_mass`; OpenBLAS 0.3.34-106 and
  0.3.30's level 1 to 3 routines and SciPy's own 0.3.28 and 0.3.31-22; LAPACK's routines of the part (listed);
  numpy's products, SVD, least squares, reductions and casts; nitransforms' ITK reading and `from_ras`; nibabel's
  `mat2euler`; niworkflows', nipype's (`algorithms/confounds.py`, `nipypecli run`) and fMRIPrep's (the confound
  interfaces in both releases' forms, `init_bold_confs_wf`, `ds_confounds`) lines extended; new entries (before
  CPython's) for nitime and nipy (one BSD-3 text, the NIPY Developers), pocketfft and ducc0 (Max-Planck-Society,
  Peter Bell), and pandas, traits and click (BSD-3, each with its own holders). No new licence identifier:
  `scripts/release/licences.py --check` is unaffected (the phrases `bsd-3-clause`, `mit license`, `apache license,
  version 2.0`, `modified bsd licence` are already there).
- **`upstream.tsv`** (1, ten rows after `freesurfer-8.2.0`) and **`UPSTREAM.md`** (5): the tags the part ported
  from, each tarball's SHA-256 checked again on 2026-11-13 against `<workspace>/tags/*-download/`: pandas 3.0.6 and
  2.2.3, nitime 0.12.1 and 0.11, nipy 0.6.1 (the GitHub tag's tarball: PyPI has wheels only), numpy 2.2.6 (fMRIPrep
  25.2.6's), fMRIPrep 25.2.6 (`FSLMotionParams`' step), OpenBLAS v0.3.34-106-g446c436e (numpy 2.5.4's
  scipy-openblas64 0.3.34.106.0: not the release, the release and 106 commits; a git commit row, the folder from
  the codeload tarball, SHA-256 `ddecf3872f66…`), v0.3.31-22-g5ffbf38b (SciPy 1.18.1's scipy-openblas32, a git
  commit row, tarball SHA-256 `51257fb2f0aa…`) and 0.3.28 (SciPy 1.15.3's: the release tarball); `UPSTREAM.md`'s
  rows for pandas, nitime, nipy, traits and click (the installed wheels: no tag, as nilearn and transforms3d) and
  the OpenBLAS, fMRIPrep, niworkflows and nipype rows extended.
- **`docs/validation/golden.md`** (9): the main package's counts (318 → 334 cases, 3,053 → 3,641 outputs); the
  recording paragraph (relays 3b and 2i, each from a clean commit with the smoke tier passing, 3,059 tests; every
  earlier checksum unchanged, the `DBDSQR` correction's reach included); the command line's row (82, `cli.nipype.run`),
  `lx.niworkflows` (12) and `lx.skimage` (3) extended, new rows `lx.confounds` (9) and larmorprepx's confounds
  workflow (2); the coverage test's namespaces.
- **`docs/next-steps.md`** (4): item 4 (9) **done** with its numbers; the order table's row 1 done; the
  `OPENBLAS_L2_SIZE` follow-up (already there from 2026-11-11) gets the exact proposal (512, the cache key, which
  records to re-check: NP-09's class); a block "From the confounds' part" before FreeSurfer's preparation's: the
  site page (with the documentation revision, from the export), PIPE-79's measurement, PIPE-42 and the reportlets,
  speed, the refused paths, `larmorx nipype run`'s remaining interfaces and commands, the notices (PD-01 per step,
  fMRIPrep's `warnings.warn`), upstream report drafts, the other parts' re-made records (optional), and the
  restriction table's confounds family for the later clean room (the files listed). The `atan2_glibc` micro-part
  is already in the 2026-11-11 block (391ff53f).
- **`docs/status.md`** (1): a row "Confounds (ports)" after "Multi-echo". Its long "Next" paragraph (it still lists
  "the confounds, the templates, FreeSurfer, ...") is the orchestrator's to harmonize.
- **`docs/decision-log.md`** (1, appended): "2026-11-11 to 13: the confounds' part" (D1 to D10, OpenBLAS on Zen
  4/5, the rules' outcomes, the correctly rounded `cos` kept by default, the `DBDSQR` correction, the records
  `experimental`, the site page from the export only).
- **`CHANGELOG.md`** (3, "Unreleased"): Added: the confound estimation (`bold_confounds`, `lx.confounds`), `larmorx
  nipype run`, the linear algebra and FFTs; Changed: PIPE-80 (file names with a blank, the field maps' steps too);
  Fixed: `lx.skimage.ball`'s negative radii, the `DBDSQR` tolerance, `savetxt`'s line ends on Windows.
- **`README.md`** (1): the plan's third line (the confounds done). The tool table is generated (above).
- **`CLAUDE.md`** (1): "The plan to the end": the confounds complete 2026-11-13.
- **`docs/licensing.md`** (1, two rows after sdcflows'): nipype, niworkflows and fMRIPrep's confound code
  (Apache-2.0); pandas, nitime, nipy, pocketfft and ducc0, traits and click, the OpenBLAS builds and LAPACK
  (BSD-3-Clause), all in the main package.
- **`docs/findings/numpy-blas.md`** (1, another part's findings file): two corrections from the confounds' part:
  numpy 2.5.4's OpenBLAS is v0.3.34 and 106 commits (the same arithmetic), and OpenBLAS 0.3.31's block on AMD
  Zen 4/5 (the file's 4 × 4 products are far below it: unchanged).
- **The site (`HANDOFF-site.toml`):** the roadmap (last updated 2026-11-13; the confounds' row out of "Under way
  and next", the rows renumbered 1 to 4; a completed row on top, without a link: the page comes with the new
  layout); `data/status.yml`'s planned list (the confounds done; written without ": ", which YAML refuses). The
  status table's generated row comes from `tables --site`. No `summary.json` change: `evidence summary` reads
  `summary.py`'s `PAGES`, whose confounds row comes with the page.

## What the page must show (for the documentation revision's writer; relay 4's list)

1. The agreement overview of the port (88 cases, 18 categories; `experimental`) and the case explorer.
2. The five examples (`confounds-ds000005` single-band, `confounds-gaussian-ds000005` the chain of causes,
   `confounds-ds000256` a child with strong motion, `confounds-ds004958` multiband 8 with 600 volumes,
   `confounds-ds003592` an older adult's multi-echo run): each run example's mask viewer (aCompCor's CSF, WM and
   CSF + WM masks, tCompCor's and the crown's over the BOLD reference; panels fMRIPrep's files, larmorx
   `reproduce`, larmorx default; comparisons with fMRIPrep's file, default − reproduce, 1 − 8 threads), its time
   series (`lines.confounds`: 14 columns, the D4 Δ panel where cells differ, the GLIBC-07 negated column on
   ds003592) and its column table (`lines.confounds.stats`, a verdict per family); the Gaussian example with its
   chain (yardstick rows without a verdict: ANTs as built = fMRIPrep's file, fMRIPrep's ANTs again = its file;
   larmorx = ANTs with CORE-MATH).
3. The tables: the 36 files column by column (the headline), every column by family, the node files, the Gaussian
   chain on 9 items, the defects (50 rows, each linked to its findings section), memory, random testing (the trial
   and the checks) and its triage; the speed chart (nodes, workflow, linear algebra: the port's own `svd`, `lstsq`
   and `gesvd` slower, a stated speed item).
4. The explained differences, never hidden: D4 (`divergence`: 11 files' motion cells, 0 with fMRIPrep's own
   motion table), GLIBC-07's negated columns in the default (`divergence`, "expected": 60 columns in 5 of the 36
   derivatives with the newest releases, 88 in 20 of the 108 components files with fMRIPrep 25.2.6's), the
   Gaussian masks' last bits (`identical-cr`), PIPE-43's column order (columns matched by name), PIPE-41's five JSON
   entries (a correction), PIPE-79 open (fMRIPrep runs with FreeSurfer needed).
5. `summary.py`'s `PAGES` row (the page's address, "fMRIPrep's confounds", port, record `confounds-fmriprep`,
   `experimental`) and `validation/tools.toml`'s `page`; the credits come with the export (15 datasets).

Found on the way (the documentation revision's step 1 fixes it for every page): other pages' exports leak this
machine's folders in their provenance (`figures/cache.py`'s `_short`); the confounds' own `_provenance` names
`<larmorx>` and `<workspace>` only.

## Checked (relay 5, on the trial)

- Both handoffs apply (`--check`, then for real); `tables --site`, `tables --check`; `register_counts.py`; the
  NOTICE copies equal; `cargo metadata --offline --locked` in the four workspaces.
- The trial's wheel built (`maturin build --release`, boxed 8 GiB, 4 jobs, 3 minutes; log
  `oracles/logs/confounds-r5-trial-wheel.log`) and, on it with FSLDIR unset, the tests that read the shared
  documents and the golden suite (`tests/python/test_defects.py`, `test_defects_ants.py`, `validation/tests/`'s
  `test_tables.py`, `test_release.py`, `test_docs_shell_blocks.py`, `test_cleanroom_audit.py`,
  `test_cleanroom_verdicts.py`, `test_cleanroom_summary.py`, `test_evidence_confounds.py`,
  `test_confounds_scripts.py`, and `tests/golden`: every one of the 334 checksums): **736 passed, 1 skipped**
  (log `oracles/logs/confounds-r5-doc-tests.log`; `<scratch>/r5/doc_tests.sh`).
- The site trial (`<scratch>/r5/site`, `static/evidence/blobs` a link to the site checkout's blobs) built with
  `tools/build.py --main-repo <trial>`: **0 problems, 33 pages** (199 links into the trial checked). Built
  before and after the site handoff and `tables --site`: only `index.html` (the status table +1 row, words 5,246
  → 5,510) and `roadmap/index.html` (words 5,192 → 5,339) differ; the inventory (`site_inventory.py`): every
  count kept or up, 30 pages unchanged. Screenshots before and after of the home page's status section and the
  whole roadmap at 1440 and 390, light and dark: `<scratch>/r5/shots/before/`, `<scratch>/r5/shots/after/` (8
  each; `<scratch>/r5/shots.sh`).
- Not run on the trial: the Rust gates, the parity smoke tier, the records (verify-merge's).

## What to verify at the merge

- **Records** (identical expected): the part's five (`confounds-linalg`, `-linalg-full`, `-nipype`,
  `-nipype-full`, `-fmriprep`; relay 3a-i's standard tier took about 2,000 s of case time, `--jobs 2` in
  a 24 GiB box; the full tiers are longer; their environments `e1` (`oracles/python-sdcflows-2.17` with `OPENBLAS_L2_SIZE=512`), `e1-zen`
  (the full tiers only), `e2` (the shared `.venv`) and `e3` (fMRIPrep 25.2.6's image through Apptainer)).
- **The `DBDSQR` correction's reach** (relay 3b re-made them from `9a106cf1` and compared: every result
  unchanged; main's records kept, not swapped): `sdcflows-fieldmap` 37/37, `sdcflows-estimation` 19/19,
  `sdcflows-estimation-full` 14/14, `sdcflows-estimation-app-shared` 8/8, `niworkflows-helpers` 5/5,
  `tedana-t2smap-replica` 102/102 (`LARMORX_GPL_BIN` at the merged tree's `crates-gpl` build),
  `tedana-t2smap-original` 125/125.
- **PIPE-80's `_nipype.py`** (shared with the field maps): the field maps' checks re-ran on the branch (relay 3a-i:
  the golden case `py.larmorprepx.fieldmaps`, `tests/python/test_sdcflows_syn.py`, the smoke tiers of
  `sdcflows-estimation` and `niworkflows-helpers`, whose `nipype-command-lines` case compares every program's line
  with every oracle run's `command.txt`): all pass.
- **The parity smoke tier with FSLDIR set** (it compares with FSL); `cargo test` and the Python unit gates CI-like
  (FSLDIR unset, `LARMORX_GPL_BIN` at the merged build). The confounds' smoke cases pin numpy's and SciPy's
  OpenBLAS to one thread with `threadpoolctl` (relay 3a-iv: the permanent case
  `cosine-filter-wide-basis-dbdsqr-tolerance` is above OpenBLAS's threading thresholds).
- **Golden:** `record_golden.py` from the merge commit, every checksum unchanged expected (334 cases).
- **The memory test** `test_confounds_need_no_more_memory_than_the_originals` (6 calls) needs the oracle runs'
  working directories and `oracles/python-newest`.

## The clean-room audit

This part wrote no clean room (an exact port of permissive upstreams: nipype, niworkflows, fMRIPrep, nitime,
nipy, pandas, numpy, SciPy, OpenBLAS, LAPACK, nitransforms, nibabel, traits, click); its agents read those sources
and ran FSL, ANTs and AFNI only as programs through the oracle cache. Declared slips (NOTES.md §13, §42):
relay 1's `grep -rn "pandas" docs/findings/*.md ...` printed three lines of `docs/findings/tedana-t2smap.md`
(restricted for `t2smap`'s clean room: its environments' versions, a `to_csv` call, a pandas error text; relay 1 is
not that clean room's implementer and edited no clean-room crate; later relays searched `docs/findings/` file by
file); relay 2g-i's merge listed file names under `crates-gpl/` and `docs/parts/2026-11-12-t2smap-clean-room/` and
showed the public TED-31 and TED-46 texts of `larmorx_core::defects`; relay 2h-i's whole-tree grep printed two
lines of the release part's `docs/parts/` notes, and a grep over every `PROVENANCE.md` printed two file names;
relay 2i's two greps over `crates/` scanned `crates/larmorx-afni-replica` without printing anything of it; relay
3b's grep for `numpy_linalg`'s callers printed two `use` lines of the tedana replica in `crates-gpl/` and its file
names. Nothing else of `crates-gpl/`, `crates-nc/` or the restricted findings was opened (relays 2a-ii and 2b-iii
declared two other slips of their own logs: an overwritten log kept under another name, an aborted record kept;
and one refused `sudo -n true` probe). No agent of the part edited a clean-room crate or `specs/`. For the later
clean room of the confounds, the restriction table needs a confounds family (the next-steps block lists the files).

The part's agents (31; their ids for the archive step; NOTES.md §45 has the same list): relay 1
`ad699d63b701b94b4`; 2a `abafb802f99529ccb`; 2a-ii `a2ff97d994bc5f188`; 2b-i `a86d27bfb4a061f25`; 2b-ii and 2b-iii
`ace99d9d00fafb1ec`; 2c-i `ae8ccd6a7548d06ac`; 2c-ii `a8890e0c8f8e860c3`; 2d-i and 2d-ii-a `a72a3d63ba2e3a393`;
2d-ii-b `abad8927d5a0117d9`; 2d-iii-a and 2d-iii-b's first piece `aae2de6b3116b066c`; 2d-iii-c `aa16a289f0b4eaa1e`;
2e-i `ac58ec7830589adb3`; 2e-ii `afdfa05789a8abec2`; 2f-i `a75d98df8ef10307b`; 2f-ii-a `a100b3c4e47716a12`;
2f-ii-b `a796c7bc7d2342d73`; 2g (the merge of `ba876f7f`) and 2g-i `a62cbcc7f4ba1b74c`; 2g-ii `a4815719b5eb82e2d`;
2g-iii `ae3190cfcd4eaddcc`; 2g-iv `a93c2097abb3e1089`; 2h-i `a0aa1a21dc0e1237c`; 2h-ii `a970ac0f56d488d53`; 3a-i
`aa8ae05f623d3b7df`; 3a-ii's first attempt (stopped by an interruption, nothing committed) `a350e93d6105a3196`;
3a-ii `a09a1b62d0b6ecad5`; 3a-iii `a13af479ca56a9f84`; 3a-iv `a571a657997d1944d`; 3b `aecda30c518995cc5`; 2i
`a680ea5c107a23251`; 4 `a6e4be2263820f992`; 5 `a70923b6fb1ce4ef1`. All in the orchestrator's session
`b322ad6c-679d-4079-be53-839676da5898`.
