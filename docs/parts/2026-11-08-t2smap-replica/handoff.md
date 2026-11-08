# Handoff: tedana's `t2smap` replica (2026-11-08)

The part's thirteen relays (1, 2, 3, 4, 5a, 5b, 6, 7a, 7b, 8a, 8b, 9, 10), ready to merge. The shared-document
changes are in `HANDOFF.toml` beside this file (104 operations, 26 files: 23 of the main repository and 3 of the
site); this file gives the reasons, how to apply them, the trial merge's conflicts and what to verify.

## What to merge

- **larmorx:** branch `worktree-agent-ac7d861cd17e74f0d` (worktree
  `larmorx/.claude/worktrees/agent-ac7d861cd17e74f0d`), head `3709e6d4`, from main `ec557ff5` with one merge of
  main `99e9024a` (`9648c29a`, relay 3); 44 commits of its own; tree clean. No clean-room crate, `crates-nc/` or
  `crates/larmorx-afni-replica` file is touched (`git diff --stat 99e9024a 3709e6d4 -- crates/larmorx-afni
  crates/larmorx-mri crates/larmorx-fsl crates-nc crates/larmorx-afni-replica`: empty); `specs/t2smap.md` is new
  (this part's spec writer).
- **Site:** the clone `<scratch>/site`, branch `t2smap-pages` at `c40ae49c` (on the site's main `01f901ed`: a
  fast-forward; `content/tools/tedana-t2smap.md`, `static/evidence/tedana-t2smap.json`,
  `static/evidence/blobs.sha256`); `evidence-blobs` at `ae1a3b62` (1,343 blobs = 1,275 + 68 added, none removed
  or replaced; the manifest's SHA-256 `f29298798b957e77`). Push `evidence-blobs` first, then `main`. The page's
  JSON was exported against the site's main `01f901ed`, which has not moved: no re-export needed.
- The trial merges are scratch worktrees, the handoff applied in them and left uncommitted:
  `<scratch>/r10/merge` (a detached worktree of the larmorx repository: main `6840f4d8` + the branch, the merge
  commits `8b080889` and `dfe1b6ab`, scratch only) and `<scratch>/r10/site-merge` (a detached worktree of the site
  clone at `c40ae49c`; `static/evidence/blobs` there is a symbolic link to the clone's blobs). Remove both with
  `git worktree remove --force` when done.

## How to apply (after the merge)

```
cd <merged larmorx checkout>
python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <merged site>
python -m larmorx_validation tables --site <merged site>
python -P scripts/orchestration/register_counts.py
python -m larmorx_validation evidence summary --site <merged site>
(site) .venv/bin/python tools/build.py --main-repo <merged larmorx checkout>
```

Checked on the trial merges: every operation applies (`--check`, then for real); `tables --site` adds README.md's
row, the validation index's `tedana-t2smap-replica` row and the site's status row, and `tables --check` then
passes; `register_counts.py`: **452 entries** ((a) 99, (b) 13, (c) 227, (d) 27, (e) 68, (f) 16; questionable 10;
no **decide**, no **yes**; one rule 8), the numbers the handoff writes into the counts table; `evidence summary`
adds the page's entry to `summary.json` (and nothing else changes there); the site with every change built with
**0 problems** (30 pages, 172 links into the main repository checked against the trial merge). Every built page
only gains words against the clone's build of `c40ae49c` (`index.html` 4,267 → 4,410, `roadmap` 4,042 → 4,204,
`tools/` 1,065 → 1,104, `validation` 7,034 → 7,039, every other page +1: the navigation's new tools entry; no
word lost on any page). The previews: `<scratch>/r10/shared-preview.diff` (the main repository's 24 files, the
generated tables included) and `<scratch>/r10/site-shared-preview.diff` (the site's 4). The generator, which
reads and checks every `old` text against the trial merge's files: `<scratch>/r10/make_handoff.py`.

The trial merge with the handoff applied, its own release wheel and the gates' Python run
(`<scratch>/r10/merge_checks.sh`, boxed 24 GiB, log `oracles/logs/t2smap-1-relay10-merge-checks.log`): see
"Results of the trial merge's checks" at the end.

## The trial merge's conflicts (larmorx; the site merges as a fast-forward)

Merging the branch into main `6840f4d8` stops at four files, all from main's cherry-picks of this part's relay-2
commits (`8e491ace`, `d0b5b5ea`, the shared numpy module, published in `f5e89ff4`):
`crates/larmorx-core/PROVENANCE.md`, `crates/larmorx-core/src/lib.rs`, `crates/larmorx-core/src/numpy/mod.rs`
(add/add) and `validation/investigations/t2smap/README.md` (add/add). Main's version of each is relay 2's
`0ed9cee2` byte for byte (`git diff main 0ed9cee2 -- <file>`: empty), and the branch's version continues it
(numpy's float printing, CPython's `python` module, the `div` of `NpFloat`, the later relays' README sections), so
the resolution is the branch's file: `git checkout --theirs -- <the four files>` (what the trial merge did). Every
other file merges cleanly; `tests/golden/golden.json` takes main's header only (the branch does not touch it).

**With the field maps' part B** (its branch `worktree-agent-a33e6cc1a7edea6ca` at `b401e22b`, merged on top of
the trial merge as a dry run, then aborted), whichever merges second meets six conflicts, each "keep both" but one:
`Cargo.toml` and `Cargo.lock` (both add a workspace crate: `larmorx-optim` here, `larmorx-ndimage` there: keep
both); `crates/larmorx-core/src/numpy/mod.rs` (`pub mod format;` here, `pub mod dot; pub mod histograms;` there;
the `NpFloat` trait's `div` here, `np_exp` and `np_dot` there: keep both); `python/larmorx/__init__.py` (the lazy
submodules: `tedana` here, `niworkflows` and `skimage` there: keep all three); `tests/golden/registry.py`
(`NAMESPACES`: `lx.tedana` here, `lx.skimage` and `lx.niworkflows` there: keep all); and
`crates/larmorx-transform/src/numpy_linalg.rs`: `dbdsqr_upper` — keep this part's version (the general
`dbdsqr_upper_ncc` with its `clear_negative_zero` parameter, which relay 3 added for the 3.12.0/3.12.1 step, and
the thin `dbdsqr_upper` wrapper) and point part B's `clear_negative_zero` argument at it (ORCHESTRATOR-STATE's
MERGE NOTE; part B's NOTES has its lines). Part B's `nanpercentile` q = −0.0 fix in the shared numpy module wins
(the orchestrator's note of 16:00).

## What to verify at the merge

Records (identical expected): **`tedana-t2smap-replica`** (standard tier: 69 cases, 44 pass, 25 rejected by both;
needs a release build of `crates-gpl`'s `larmorx-gpl`: verify-merge's `build-gpl.sh` makes
`crates-gpl/target/release/larmorx-gpl`, which the suite finds by itself (or `LARMORX_GPL_BIN`; the script's
header comment, "only its verification tools", is out of date), the launchers under
`oracles/inputs/tedana-t2smap-replica/`, both Python environments, `oracles/fmriprep-25.2.6/`'s three multi-echo
runs and fMRIPrep's image for the spread case; relay 7b's command, NOTES.md §7i: `--tier standard --jobs 8` in a
24 GiB box; 104 of its 155 original runs come from the oracle cache), **`numpy-linalg`** and **`sdcflows-fieldmap`** (relay 3 changed the shared
`numpy_linalg` routines `dnrm2` and `dbdsqr` for `lstsq`: the field maps' `svd3`/`eigh4` cases stayed identical
then: `tests/parity/test_numpy_linalg.py` with `test_nifti_io.py`, 195 passed), **`nifti-io`** (`crates/larmorx-io` gained
`nifti/nibabel_affine.rs`, `NiftiHeader::qform_nibabel` and `best_affine_nibabel`; `lx.load` is unchanged), and
**`resample-series`** if the verification runs every record that reaches `numpy_linalg`. The memory test
`tests/parity/test_memory_peak.py::T2SMAP_CASES` (fMRIPrep's ds003592 call, curve fit and log-linear: 545 / 543
MiB against tedana's 1,243 / 1,224) needs the same program and the launchers. Golden: the main package's
checksums are unchanged (no new case: `lx.tedana.t2smap` is in `REPLICA_ONLY_FUNCTIONS`, its cases are
`larmorx-gpl`'s ten in `crates-gpl/larmorx-gpl-cli/tests/golden.rs`, recorded from `e86180ff`); the CI `gpl`
jobs now also run rustfmt, clippy and `cargo test` of `larmorx-gpl-tedana` and `larmorx-gpl-cli` (about a minute
per job cold). The site's page: `python -m larmorx_validation evidence tedana-t2smap --site <site> --no-compute`
serves every run from the figures cache (`oracles/figures-cache/tedana-t2smap/`).

**NOTICE:** the handoff changes the root `NOTICE` and makes the same changes to its two copies
(`crates-nc/LICENSES/NOTICE-larmorx.txt`, `crates-gpl/LICENSES/NOTICE-larmorx.txt`: the three files stay one
text; both workspaces' `notices` tests passed on the trial merge). Any later change to `NOTICE` at the merge must
be copied to both.

**The clean-room audit:** this part wrote no clean-room original. Its agents read tedana's source (the clone,
`tags/tedana-26.0.3`, `tags/tedana-25.0.1`, the installed copies in both environments and in fMRIPrep's image),
numpy's, SciPy's, nibabel's, nilearn's, CPython's, OpenBLAS's and LAPACK's, and ran tedana as a black box. The
handoff gives the audit the tedana family's restrictions for `t2smap`'s clean-room part (D9): R07 also anchors
`tags/tedana-*` and every `site-packages/tedana` (the workspace's environments and a container's), R10 knows
`crates-gpl/larmorx-gpl-tedana` (family tedana) and counts the rest of `crates-gpl/` for both AFNI and tedana,
`docs/findings/tedana-t2smap.md` is a source finding (R11; its `named` date is the brief's, relay 1 — set it to
the merge commit's time if you prefer), the replica's difftest descriptor joins R16's files and
`validation/investigations/t2smap/` joins the briefs' forbidden investigations; five classification cases in
`validation/tests/test_cleanroom_audit.py` (they pass on the trial merge). When the clean-room part starts:
`R16_FORBIDDEN_BY` its brief's launch description, `verdicts.tool_of` the crate `crates/larmorx-tedana`, the
similarity family "tedana" (`cleanroom-audit-merge.sh --similarity`). The part's agents (for the archive step):
relay 1 `ac7d861cd17e74f0d`, relay 2 `a1e74b87182689d97`, relay 3 `af895e326a1e6d7a0`, relay 4
`a5a8b57937a8b1b27`, relay 5a `a30aa31ad03a8cbb0`, relay 5b `ae98246030672c0a4`, relay 6 `a16720ab7bb61dc5f`,
relay 7a `ad7066973cb2d2a09`, relay 7b `a255b30a7008412b5`, relay 8a `a7792b13ea745f138`, relay 8b
`a5667128987e5c88e`, relay 9 `ad96fb3c5454f2d1f`, and relay 10 (this one; its id is the orchestrator's to add).

## For the orchestrator (not in the operations)

- **Source-derived words in open files.** The replica part's own open files hold a few words that come from
  tedana's source: the record's and the permanent cases' descriptions name `fit_loglinear` (TED-17's case
  `difftest/ted17-single-voxel`), and the site page's tables quote them and the restricted findings' defects
  table (TED-17's routine; TED-04's "tedana's TODO") and the triage's entries (`decay.py:83`, `utils.py:213,
  230`, numpy's `_methods.py` lines). Relay 10 rewrote the two notice texts that named them
  (`larmorx_core::defects` TED-04, TED-17: behaviour only); the record, the cases and the page were left (a
  record is remade only from a clean commit). Recommended: the clean-room part's brief names the replica's record
  descriptions, the page's "Corrections", "Random testing" and "Every case" sections and the triage as not to
  read (as the briefs name the replica suites), or a small cleanup of those texts before that part; the spec
  (`specs/t2smap.md` §15) holds every defect in behaviour terms for the implementer.
- **The Haswell re-runs of ds000210 and ds000258** (your note of 22:16): nothing in the handoff; the record and
  the page say what they were made from.
- **Follow-ups** are in the next-steps operation (the clean-room original next; the duplicates in
  `larmorx-sdcflows` to merge onto `larmorx_core::numpy` and `larmorx_core::python`; D5's formats; the replica's
  one-thread speed; the TMPDIR `-h` tests; tedana's releases; the final phase's multi-echo data).
- `larmorx-gpl`'s wheel was built once (relay 6, `<scratch>/wheel-gpl/larmorx_gpl-0.0.1-py3-none-manylinux_2_34_x86_64.whl`,
  21 licence files); not rebuilt since (its contents' list unchanged).

## The operations and why

- **`validation/tools.toml`** (generated tables): a `[[tool]] tedana-t2smap` after sdcflows' (eco `larmorx`, page
  `/tools/tedana-t2smap/`, the replica's facts; the record's numbers, fMRIPrep's calls, memory and speed) and a
  `[[record]] tedana-t2smap-replica` after sdcflows' record. Its `{tedana-t2smap-replica.fmriprep}` metric is in
  the branch (`tables.py`'s `METRICS`, relay 10, counted as BET's).
- **`docs/findings/upstream-defects.md`:** the opening's families (TED-); the restricted note names the new
  section; summary rows NP-03 and NP-04 after NP-02, TED-01 to TED-18 (†, behaviour only) after SDC-11 (classes:
  (c) eleven, (e) six, (f) TED-14; gap "–", TED-06 "– (rule C: kept)"); GLIBC-07's row and entry extended with
  numpy's float64 `exp`/`log` and SciPy's `pow` as tedana takes them; open entries NP-03 and NP-04 before NIB-01;
  the counts (452: (c) 227, (d) 27, (e) 68, (f) 16); the D13-gaps paragraph after sdcflows'; the section
  "Restricted: tedana t2smap (source-derived)" appended at the end (TED-01 to TED-18's mechanisms in source terms,
  briefly; the findings hold the rest).
- **`docs/api/upstream-defects.md`:** TED-01 to TED-05, TED-07 to TED-18 and NP-03 in "What is corrected" (the
  API page's own texts), `larmorx tedana t2smap` in GLIBC-07's row, and a paragraph after sdcflows' (the switch;
  TED-06 kept; NP-04 a reference choice).
- **`upstream.tsv`:** tedana 26.0.3 and 25.0.1 as `tags` rows (PyPI sdists; SHA-256 checked again against
  `<workspace>/tags/tedana-*-download/` in relay 10; licence `LGPL-2.1-only`).
- **`UPSTREAM.md`:** tedana's, nilearn's and CPython's rows in "Ported sources"; the SciPy and numpy rows name
  tedana's paths; the restricted section names tedana's source.
- **`NOTICE`** (three copies): SciPy's, OpenBLAS's, numpy's, LAPACK's and nibabel's entries name the new paths
  (SciPy's `curve_fit` and `least_squares`, `scoreatpercentile`, `ndimage`'s filters, morphology and label;
  OpenBLAS's Haswell kernels on `lstsq` and the m × 2 SVDs; numpy's reductions, percentiles, float32 `log`,
  float printing, `lstsq`, `LONGDOUBLE_dot`; LAPACK's `dgelsd` path; nibabel's extended-precision `get_qform`);
  new entries for nilearn 0.14.1 (its BSD-3-Clause text from the installed wheel's `LICENSE`) and CPython 3.12.16
  (the PSF License Version 2, verbatim from the interpreter's `LICENSE.txt`, and the brief summary of changes its
  clause 3 asks for), before fMRIPrep's entry.
- **`docs/licensing.md`:** tedana's row (the replica in `larmorx-gpl`, the planned original), nilearn's and
  CPython's rows, the BSD libraries' row; `larmorx-gpl`'s packages row (published again, GPL-3.0); "A replica
  package is a program" and a new bullet for `larmorx-gpl`'s wheel (section 3's conversion, the licence files,
  the Corresponding Source, the notices test); "Publishing"'s GPL line.
- **`docs/validation/golden.md`:** a fourth column for `larmorx-gpl` in "Where they are"; its recording paragraph
  and CI estimate; the coverage table's row (10 cases); the note on `crates-gpl/larmorx-gpl-cli/tests/golden.rs`.
- **`docs/validation/difftest.md`:** the descriptor's row after sdcflows'. **`docs/validation/oracle-cache.md`:**
  the key's `python` family; the runners' row.
- **`docs/next-steps.md`:** the order's item (8): the replica done; a first block of "Follow-ups, when they fit".
  Nothing for "Waiting for the user".
- **`docs/status.md`:** a "Multi-echo (a replica)" row after "Field maps (ports)"; the "Next" paragraph.
- **`docs/decision-log.md`:** a proposed entry "2026-11-08: tedana's `t2smap` replica: what was decided in the
  part" (D2, D4 to D9 and option A from NOTES.md's "FROM THE ORCHESTRATOR" sections, the rules' outcomes, the
  notices' passes and the original's one-pass rule). Edit or drop it.
- **`README.md`:** the versions line (tedana 26.0.3), the licence paragraph, the install lines (`larmorx[all]`),
  the roadmap's item 3, the development note and the repo map (`crates-gpl/`, `crates/larmorx-optim`); the tool
  table is generated.
- **`CLAUDE.md`:** "Where things are", rule 1 (tedana's line under the replica packages; `tedana-t2smap.md` in
  the restricted list), rule 2's pins (tedana 26.0.3 and its libraries), the repo map (`crates-gpl/`, `-optim`).
  The `old` texts were checked against the main checkout's CLAUDE.md, which equals the trial merge's.
- **`docs/architecture.md`** (`larmorx-optim` in the crate list; `crates-gpl/`'s program; the golden row and step
  8; "Adding a replica" step 1), **`docs/overview.md`** (tedana's row, the install line, the GPL replicas' line),
  **`docs/porting-guide.md`** (§1's restricted list: `tedana-t2smap.md`; `crates-gpl/` holds tedana's replica).
- **The audit** (`cleanroom_audit/restrictions.py`, `verdicts.py`, `validation/tests/test_cleanroom_audit.py`):
  above.
- **Site `site.yml`** (the tools entry before sdcflows'), **`data/status.yml`** (the planned line's multi-echo;
  the generated row from `tables --site`), **`content/roadmap.md`** (a "Multi-echo" row 2 in "Under way and next",
  the rows after it renumbered; a completed row, newest first), **`static/evidence/summary.json`** (`evidence
  summary`).

## Results of the trial merge's checks

`<scratch>/r10/merge_checks.sh` (boxed 24 GiB; log `oracles/logs/t2smap-1-relay10-merge-checks.log`) on the trial
merge with the handoff applied: both workspaces' `notices` tests pass (`crates-gpl`'s and `crates-nc`'s copies of
`NOTICE` equal to the root's after the handoff); the trial merge's release wheel built (its `_core` in the trial
merge's `python/larmorx/`); `ruff check` passes; `check_spdx.py` and `tables --check` pass; `pytest tests/python
tests/golden validation/tests` (serial, TMPDIR unset) **1,129 passed, 16 skipped** (every golden checksum of both
sides reproduced by the merged build; the audit's new classification cases included). `ruff format --check` found
one inserted test line too long in that run; the operation was rewritten in ruff's form, the handoff applied again
from a clean trial merge, and then `ruff check`, `ruff format --check` (932 files), `check_spdx.py`, `tables
--check`, `pytest validation/tests/test_cleanroom_audit.py` (84 passed), `register_counts.py` (452) and the site's
build (0 problems) all pass. Not run on the trial merge (verify-merge's): the three workspaces' full Rust gates and
the parity smoke tier; the branch's own gates are in NOTES.md (relay 7b: all three workspaces, the Python gates
and the smoke tier; relays 8b to 10: the crates they changed).
