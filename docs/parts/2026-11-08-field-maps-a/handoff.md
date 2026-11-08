# Handoff: the field maps' part A, sdcflows' port (2026-11-08)

The part's eleven relays, ready to merge. The shared-document changes are in `HANDOFF.toml` beside this
file (48 operations, 17 files: 14 of the main repository and 3 of the site); this file gives the reasons,
how to apply them, the trial merge's conflicts and what to verify.

## What to merge

- **larmorx:** branch `worktree-agent-ad5d6c85d74dc98ba` (worktree
  `larmorx/.claude/worktrees/agent-ad5d6c85d74dc98ba`), head `eca11c3a`, from main `2b453c1`; 52 commits;
  tree clean. No clean-room crate, spec, replica crate, `crates-gpl/` or `crates-nc/` file is touched
  (`git diff --stat 2b453c1 eca11c3a -- crates/larmorx-afni crates/larmorx-mri crates/larmorx-fsl specs
  crates-nc crates-gpl crates/larmorx-afni-replica`: empty).
- **Site:** the clone `<scratch>/site`, branch `fmap-a-pages` at `7130949f` (from the site's main
  `b9d118cc`: `content/tools/sdcflows-fieldmap.md`, `static/evidence/sdcflows-fieldmap.json`,
  `static/evidence/blobs.sha256`); `evidence-blobs` at `96bca689` (1,275 blobs = 1,236 + 39 added, none
  removed or replaced; the manifest's SHA-256 `8565bf68d65fe9ca`; parent: none, as `tools/blobs.py commit`
  makes it). Push `evidence-blobs` first, then `main`.
- The trial merges of this relay are scratch worktrees (`<scratch>/r11/merge`, a detached worktree of the
  larmorx repository at main `ec557ff5` + this branch, and `<scratch>/r11/site-merge`, of the site clone at
  `69662a86` + `fmap-a-pages`), with the handoff applied in them (uncommitted): remove them with
  `git worktree remove` when done (`<scratch>/r11/site-merge/static/evidence/blobs` is a symbolic link to
  the clone's blobs, for the evidence summary and the build).

## How to apply (after the merge)

```
cd <merged larmorx checkout>
python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <merged site>
python -m larmorx_validation tables --site <merged site>
python -P scripts/orchestration/register_counts.py
python -m larmorx_validation evidence summary --site <merged site>
(site) tools/build.py --main-repo <merged larmorx checkout>
```

Checked on the trial merges (main `ec557ff5` + the branch; the site's `69662a86` + `fmap-a-pages`): every
operation applies (`--check`, then for real); `tables --site` adds README.md's row, the validation index's
`sdcflows-fieldmap` row and the site's status row, and `tables --check` then passes; `register_counts.py`:
**432 entries** ((a) 99, (b) 13, (c) 216, (d) 25, (e) 62, (f) 15; questionable 10; no **decide**, no
**yes**; one rule 8); `evidence summary` adds the page's entry (37 cases: 33 bit-identical, 4 d13) and the
tool to `summary.json`; the site with every change built with **0 problems** (29 pages, 167 links into the
main repository checked against the trial merge). Built before and after the handoff (`<scratch>/r11/
site-before/`, `site-after/`): every page only gains (the navigation's new tools entry on every page; the
home page's status table one row and its count 23 → 24 lines; the planned field-map text rewritten with
more; the roadmap's row 1 rewritten and a completed row; the validation matrix one row, the NIfTI I/O row's
position indices only). The previews: `<scratch>/r11/shared-preview.diff` (the main repository's 15 files,
with the generated tables) and `<scratch>/r11/site-shared-preview.diff` (the site's 4). The generator,
which reads every `old` text from the trial merge's files: `<scratch>/r11/make_handoff.py`.

The trial merge was also built (its release wheel, `<scratch>/r11/build_and_test_merge.sh`) and the gates'
Python run made on it with the handoff applied (`<scratch>/r11/serial_tests.sh`, log
`oracles/logs/fmap-a-11-merge-serial.log`): `pytest tests/python tests/golden validation/tests` **1,106
passed, 15 skipped** (every golden checksum of both sides reproduced by the merged build), `ruff check`,
`ruff format --check` (891 files) and `check_spdx.py` pass. Under pytest-xdist (`-n 12`, log
`oracles/logs/fmap-a-11-merge-test.log`) one more test failed,
`validation/tests/test_difftest.py::test_a_case_over_the_memory_limit_is_recorded_and_the_night_goes_on`
(a whole night in a box of its own beside eleven other workers); it passes alone and in the serial run, as
the two oracle-cache tests that fail only under `-n 8` (next-steps' harness item). The Rust gates and the
parity smoke tier were not run on the trial merge (verify-merge's).

## The trial merge's conflicts (larmorx; the site merges without one)

Merging the branch into main `ec557ff5` stops at three files, each appended to at the same place by both
sides; keep both sides, main's first:

- **`tests/golden/golden.json`**: only the `recorded` header (main `5e66d1e2`, the branch `14ff7709`);
  every case merges (246 + 16 = 262 cases, 2,053 outputs). Take either header, then
  `scripts/record_golden.py` from the merge commit (verify-merge does): every checksum unchanged expected.
- **`tests/parity/test_memory_peak.py`**: both appended after `test_larmorx_needs_no_more_memory_than_afni`
  (main: `ORIGINAL_VOLREG_CASES` and `test_volreg_original_needs_no_more_memory_than_afni`; the branch:
  `SDCFLOWS_RUNS`, `SDCFLOWS_CASES`, `_sdcflows_inputs` and
  `test_sdcflows_port_needs_no_more_memory_than_sdcflows`). git interleaves the two blocks in two hunks:
  resolve by taking main's file and appending the branch's added lines (what the trial merge did:
  `git show ec557ff5:<f>` + the branch's lines after the merge base's last); `ruff check` and `ruff format
  --check` pass on the result.
- **`validation/difftest/triage.toml`**: both appended at the end (main: 3dvolreg's clean-room original's
  block; the branch: "sdcflows-fieldmap (2026-11-08)", 4 entries); main's then the branch's, as above;
  `tomllib` reads it.

Auto-merged, to look at in review: `python/larmorx/_core.pyi`, `tests/golden/registry.py` (the branch adds
`lx.sdcflows` to `NAMESPACES` and 16 cases at the end), `validation/src/larmorx_validation/__main__.py`
(`PARITY_MODULES`), `difftest/permanent.py`, `difftest/tools/__init__.py`, `evidence/__init__.py`,
`evidence/summary.py`, `crates/larmorx-core/src/defects.rs` (SDC-01, -02, -04 to -11 after SCI-03).

## What to verify at the merge

Records (identical expected): **`sdcflows-fieldmap`** (`--python newest`, standard tier: 37 cases, 37 pass;
the step needs the shared environment too; about 14 minutes at 6 jobs in 24 GiB), **`resample-series`** and
**`numpy-linalg`** (`crates/larmorx-transform/src/openblas.rs`: `getf2` is now generic over 3×3 and 4×4 and
`lu_sign_and_diagonal3` was added for numpy's `det`; the smoke tier and the golden checksums were unchanged
at relay 8), **`nifti-io`** (`crates/larmorx-io/src/nifti/header.rs` and `mod.rs` gained nibabel's qform
functions beside the existing ones; `lx.load` and `lx.save` are unchanged). The memory test
`test_sdcflows_port_needs_no_more_memory_than_sdcflows` (3 calls) needs the oracle runs'
working directories (`oracles/fmriprep-25.2.6/`). The evidence: `python -m larmorx_validation evidence
sdcflows-fieldmap --site <site> --no-compute` serves every run from the figures cache
(`oracles/figures-cache/sdcflows-fieldmap/`, 11 entries computed at `61185255`).

**The clean-room audit:** this part wrote no clean-room original (it is a port of a permissive upstream);
its agents read sdcflows', niworkflows', nitransforms', nibabel's, scikit-learn's, SciPy's, numpy's and
OpenBLAS's source, and ran FSL's topup and `lx.fsl.prelude` as black boxes (SDC-07's measurement, the
phase-difference workflow). For the later clean-room original of sdcflows (the user, 2026-11-06), the
audit's restriction table will need an sdcflows family then: `docs/findings/sdcflows-fieldmap.md`,
`crates/larmorx-sdcflows/`, `python/larmorx/sdcflows/`, `validation/investigations/sdcflows-fieldmap/`,
`parity/sdcflows_fieldmap.py`, `difftest/tools/sdcflows_fieldmap.py`, `evidence/sdcflows_fieldmap*.py`
(nothing to add now). The part's agents (for the archive step): relays 1 to 10 `ad5d6c85d74dc98ba`,
`a52fb97af12fd7a98`, `ab2c4c87b023ea94e`, `aed354163dc359170`, `add71b54eddc33c55`, `a12f04869a7af7743`,
`abf1e3da6428896bb`, `a3f6fcb919202dd47`, `a7b32ff0ee5bd279b`, `a1349b2efb577f2fe`, and relay 11 (this one;
its id is the orchestrator's to add).

## The operations and why

- **`validation/tools.toml`** (generated tables): a `[[tool]] sdcflows-fieldmap` after the resampler (eco
  `larmorx`, page `/tools/sdcflows-fieldmap/`, how "port (sdcflows 2.17.1, ...)"; the record's numbers, the
  defects, the benchmark's speed at 14 threads and the memory) and a `[[record]] sdcflows-fieldmap` after
  the resampler's (reference: sdcflows 2.17.1 and its libraries in the newest environment, the step from
  the shared one, fMRIPrep 25.2.6's own files). The common metrics suffice: no `tables.py` entry.
- **`docs/findings/upstream-defects.md`:** the opening's families (SDC-); eleven summary rows SDC-01 to
  SDC-11 after ANTSPY-03 (classes all (c), SDC-03 "c, f"; gap "–" but SDC-03's **switch**, with the
  FreeSurfer part); the new section "## sdcflows: field-map estimation (SDC-)" before "FSL: black-box
  findings" (open to everyone: sdcflows is Apache-2.0; the entries as relay 5 drafted them, SDC-11 added by
  relay 7, SDC-02 with the real `ApplyCoeffsField` call's 3.65 Hz RMS of relay 9, SDC-11's "10 of its 14
  differences" corrected); the counts (432, (c) 216, sixteen with a second class: SDC-03's (f)) and the gap
  paragraph (SDC-03's **switch**); the D13 gaps' paragraph for the port after 3dvolreg's. Three existing
  entries updated by what the port does: **PIPE-14** (`apply` ignores head-motion transforms: sdcflows
  warns that the feature is not enabled; the port keeps it with the warning, rule C; "silently" corrected),
  **NP-02** (`lx.sdcflows` decides orientations with numpy's own SVD replicated; the resampler keeps vnl's)
  and **NIB-04** (`lx.io.load_nibabel` computes the qform in x86-64 Linux's extended precision on every
  platform).
- **`docs/api/upstream-defects.md`:** ten rows SDC-01, -02, -04 to -11 after SCI-03 ("What is corrected")
  and a paragraph after 3dvolreg's (the switch; SDC-03 without one until FreeSurfer).
- **`upstream.tsv`:** five tag rows (sdcflows 2.17.1 and 2.15.1, scikit-learn 1.9.1 and 1.6.1, SciPy 1.15.3:
  PyPI sdists, SHA-256 checked against PyPI's JSON in relay 1 and again here against the files in
  `<workspace>/tags/*-download/`).
- **`UPSTREAM.md`:** sdcflows' and scikit-learn's rows in "Ported sources"; the SciPy, OpenBLAS, numpy and
  nibabel rows say what the port follows of each since 2026-11-08.
- **`NOTICE`:** sdcflows 2.17.1 (Apache-2.0, the NiPreps developers' notice and their licensing note),
  scikit-learn 1.9.1, numpy 2.5.4 (their BSD-3-Clause texts in full, from the tags) and LAPACK 3.12.1/3.12.0
  as OpenBLAS bundles it (its modified-BSD text in full, from `lapack-netlib/LICENSE`); the SciPy, OpenBLAS
  and nibabel entries name the new paths. The crates' module headers say "notices below and in NOTICE".
- **`docs/licensing.md`:** two rows in "Upstream by upstream": sdcflows (a port in the main package; its
  clean-room original later, in another crate) and the BSD libraries whose arithmetic the ports follow.
- **`docs/validation/golden.md`:** 262 cases and 2,053 outputs; the recording of 2026-11-08 from 14ff7709
  (16 cases, 190 outputs, about 1.9 s together; 3c9c10af changes no checksum; no NaN output, paths named);
  the I/O row 4 → 6 (`save_nibabel`, `load_nibabel`); a new `lx.sdcflows` row (14 cases); `lx.sdcflows` in
  the enumerated namespaces.
- **`docs/validation/difftest.md`:** the descriptor's row after the resampler's.
- **`docs/next-steps.md`:** the order's item (6) done and (7) part B named (with sdcflows' command line
  last); item 8 (part A done; `check_register`'s FreeSurfer branch and SDC-03 with FreeSurfer); a first
  block of "Follow-ups, when they fit" (part B; FreeSurfer; the resampler's orientation; the one-thread
  speed in a long-lived process and the unused stored transpose; the clean-room original; the final phase;
  the harness's two xdist-only oracle-cache test failures and the two cache disputes; the site's viewer
  check). Nothing for "Waiting for the user".
- **`docs/status.md`:** a "Field maps (ports)" row after "I/O and transforms"; the "Next" paragraph.
- **`README.md`:** the roadmap's item 3 (part B next, part A done) and the versions line of "Supported
  tools" (AFNI 26.3.05 and FSL 6.0.7.23 as the records are, with the resampler and sdcflows 2.17.1); the
  tool table is generated (`tables`).
- **`CLAUDE.md`:** "Where things are" (3dvolreg's clean-room original and the field maps' part A complete;
  next the field maps' part B) and rule 2's pins (sdcflows 2.17.1 with scikit-learn 1.9.1; their fMRIPrep
  versions kept; SciPy 1.15.3's sdist). Both `old` texts were checked against the main checkout's current
  CLAUDE.md, whose wait-loop line was being edited: they occur once there too.
- **`docs/decision-log.md`:** a proposed entry "2026-11-08: The field maps' part A, sdcflows' port" with the
  orchestrator's decisions of the part (from NOTES.md's "FROM THE ORCHESTRATOR" sections: the command line
  in part B, nibabel's writer and header arithmetic in `larmorx.io`, the two environments' record, the
  stored matrix at one thread, `check_register`'s FreeSurfer branch) and the rules' outcomes. Edit or drop it.
- **Site `site.yml`** (the tools entry after the resampler's), **`data/status.yml`** (the planned field-map
  line; the generated row from `tables --site`), **`content/roadmap.md`** (row 1: part A done, part B next;
  a completed row, newest first), **`static/evidence/summary.json`** (`evidence summary`).

## For the orchestrator (not in the operations)

- Commit `1c48bc82` is labelled "check_nibabel_write.py: lint" but also holds the unwarp binding
  (`_core.sdcflows_unwarp_volume`), the crate's `larmorx-interp` dependency and `pub mod unwarp` (relay 4;
  no history rewrite).
- The first memory run's two oracle-cache disputes (keys `fac9bd9a…`, `ba61bf75…`: nipype's time-stamped
  INFO lines; the script's key changed since, so they are never served again) stay in the cache's
  `findings.jsonl`, as you decided.
- PRELUDE's page's ds005454 label (3 T → 7 T): fixed by you (larmorx `ec557ff5`, site `69662a86`); this
  page says 7 T.
