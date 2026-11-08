# t2smap part (tedana's `t2smap`: replica + spec): NOTES

Worktree: <workspace>/larmorx/.claude/worktrees/agent-ac7d861cd17e74f0d
(branch `worktree-agent-ac7d861cd17e74f0d`, from main ec557ff5). Relay 1 committed nothing, so the
harness removed its worktree; relay 2 re-created it at the same path and branch name
(`git worktree add -b worktree-agent-ac7d861cd17e74f0d <path> ec557ff5`). Commits so far:
464b9528, 0ed9cee2 (relay 2); 9648c29a (merge of main 99e9024a), 7c3fa5f0, a1a6a0ce (relay 3);
7bb40be4, e0212a6b (relay 4); 213d9484, 96016899 (relay 5a); d6d91faf, abd204e6 (relay 5b);
68f1126d, ffd33999, c04c344e (relay 6: see "7g").
e86180ff, 288802de, 474ff5c8, 8cc50b49, 77e8596c (relay 7a: see "7h").
d03c83e8, a519ad06, 291a5751, 9e5e57cd, f3a401bc, fd84ba6d, 7b9cb9c3, ddc275d2, 1b3e87c4 (relay 7b: "7i").
762cd475, 570e1218, 731da621 (relay 8a: "7j").
1048a174, faf9ddda, cb4b8874 (relay 8b: "7k").
9fb37590, 320fd1ac, 6e6b6bc5, 99b58349, a41384b8, 1b85f0f1, 51a749b0, 32d2f91d, 89e77474, 78f8432b (relay 9: "7l";
the site clone `<scratch>/site`, branch `t2smap-pages`: c40ae49c; evidence-blobs ae1a3b62).
3709e6d4 (relay 10: the tables' metric, TED-04's and TED-17's notice texts; then the handoff, scratch only).
Build outputs: `CARGO_TARGET_DIR=<scratch>/target` (this folder's
`target/`, not the worktree's). Scratch: this folder.
Agents of this part (for the audit's transcript archive): relay 1 = ac7d861cd17e74f0d; relay 2 =
a1e74b87182689d97; relay 3 = af895e326a1e6d7a0; relay 4 = a5a8b57937a8b1b27; relay 5a = a30aa31ad03a8cbb0; relay 5b = ae98246030672c0a4; relay 6 = a16720ab7bb61dc5f; relay 7a = ad7066973cb2d2a09;
relay 7b = a255b30a7008412b5; relay 8a = a7792b13ea745f138; relay 8b = a5667128987e5c88e; relay 9 =
ad96fb3c5454f2d1f; relay 10 = the agent the orchestrator launched after 22:16 on 2026-11-08 (its id in the
orchestrator's launch record).
GPL build outputs: `CARGO_TARGET_DIR=<scratch>/target-gpl` (crates-gpl; the root's stay in
`<scratch>/target`).

## State
- **Relay 1 (sub-task 1: inventory, licence, design): DONE.** Tags in `<workspace>/tags/` (below).
- **Relay 2 (2a, permissive numerics I, without lstsq): DONE** (464b9528, 0ed9cee2; "Relay 2
  results" below). Gates run: `cargo fmt --all --check`, `cargo clippy -p larmorx-core
  --all-targets --locked -D warnings`, `cargo test -p larmorx-core --locked` (145 pass), ruff check
  and format, `check_spdx.py`; the full three-workspace and Python gates not run (no Python or
  extension change; they run before the part's final commits).
- **Relay 3 (2a', numpy's `lstsq`): DONE** (9648c29a merge of main, clean; 7c3fa5f0, a1a6a0ce;
  "7c. Relay 3 results" below). Gates run: `cargo fmt --all --check`, `cargo clippy -p
  larmorx-transform --all-targets --locked -D warnings`, `cargo test --workspace --locked` (root:
  38 test binaries, all pass), parity `tests/parity/test_numpy_linalg.py` + `test_nifti_io.py`
  (195 passed, with the relay's wheel: the shared `dnrm2`/`dbdsqr` changes leave svd3/eigh4 as
  they were); `crates-gpl`/`crates-nc` workspaces, ruff and the Python gates not run (no Python
  package change; they run before the part's final commits).
- **Relay 4 (2b, SciPy's `curve_fit`): DONE** (7bb40be4, e0212a6b; "7d. Relay 4 results" below).
  Gates run: `cargo fmt --all --check`, `cargo clippy -p larmorx-optim -p larmorx-transform
  --all-targets --locked -D warnings`, `cargo test -p larmorx-optim -p larmorx-transform --locked`
  (3 + 43 pass), `ruff check .` and `ruff format --check .` (whole repository: clean; relay 3's
  `check_lstsq.py` was not formatted and had one B007, both fixed here), `check_spdx.py`; the full
  three-workspace and Python gates not run (no Python package change; `crates-gpl`/`crates-nc` do
  not use the new crate yet, and no dependency changed).
- **Relay 5a (2c, the replica program's first half): DONE** (213d9484, 96016899; "7e. Relay 5a
  results" below). Gates run: `cargo fmt --all --check` (root and crates-gpl), `cargo clippy -p
  larmorx-core --all-targets --locked -D warnings` (root), `cargo clippy -p larmorx-gpl-tedana -p
  larmorx-gpl-cli --all-targets --locked -D warnings` (crates-gpl), `cargo test -p larmorx-core`
  (152 pass, 7 ignored), `cargo test -p larmorx-gpl-tedana -p larmorx-gpl-cli` (4 pass), `ruff
  check .`, `ruff format --check .`, `check_spdx.py` (all clean). Not run: the full three-workspace
  gates and the Python gates (no Python package change; the root's other crates do not use the new
  modules).
- **Relay 5b (2c, the replica program's second half): DONE** (d6d91faf; "7f. Relay 5b results"
  below). Gates run: `cargo fmt --all --check` (root and crates-gpl), `cargo clippy --workspace
  --all-targets --locked -D warnings` (root: whole workspace; crates-gpl: whole workspace), `cargo
  test -p larmorx-core -p larmorx-interp -p larmorx-io -p larmorx-optim -p larmorx-transform`
  (root) and `-p larmorx-gpl-tedana -p larmorx-gpl-cli` (crates-gpl), all pass; `ruff check`,
  `ruff format --check` on `validation/investigations/t2smap`, `check_spdx.py`: clean. Not run:
  the root's other crates' tests, crates-nc, the Python gates (no Python package change yet).
- **Relay 6 (2d, the interfaces and the package): DONE** (68f1126d, ffd33999, c04c344e; "7g.
  Relay 6 results" below). Gates: `gates-rust.sh <worktree>` root, gpl and nc (each with its scratch target
  directory; all three OK), `gates-python.sh <worktree> --no-build` (with the relay's wheel's
  `_core`: exit 0; tests/python + tests/golden + validation/tests passed, the CI-environment run
  and the collection without larmorx_validation passed; ruff, SPDX), `tables --check` (exit 0).
  Logs `oracles/logs/t2smap-1-relay6-gates-{rust,python}.log`. Not run: the parity smoke tier
  (nothing of the parity suites changed; relay 7 runs it with the record).
- **Relay 7a (sub-task 3, first half): DONE** (e86180ff, 288802de, 474ff5c8, 8cc50b49, and the
  77e8596c record; "7h. Relay 7 results" below). Gates run: `cargo fmt --check`, clippy and tests
  of the two GPL crates (golden included), `cargo test -p larmorx-core --lib defects`, ruff on
  every changed Python file; the record (standard tier) and the memory test. Not run: the full
  three-workspace and Python gates, the parity smoke tier (relay 7b, with its own changes).
- **Relay 7b (sub-task 3, second half): DONE** (d03c83e8, a519ad06, 291a5751, 9e5e57cd,
  f3a401bc, fd84ba6d, 7b9cb9c3, ddc275d2, 1b3e87c4; "7i. Relay 7b results" below). Gates: see 7i (all
  pass; 3 environmental failures of the smoke tier's pipeline-stages test, part B's SyN runs).
- **Relay 8a (sub-task 4, first half: the spec's text and its black-box probe): DONE** (762cd475,
  570e1218, 731da621; "7j. Relay 8a results" below). Gates: ruff check and format on
  `validation/investigations/t2smap-spec/`, `check_spdx.py` (clean); no code of the packages
  changed, so no Rust or Python gates (they run before the part's final commits).
- **Relay 8b (sub-task 4, second half: the model, the thresholds, the spec's revision; the
  TQDM_DISABLE fix): DONE** (1048a174, faf9ddda, cb4b8874; "7k. Relay 8b results" below). Gates: `cargo fmt
  --all --check`, clippy (`-D warnings`) and tests of `larmorx-gpl-tedana` and `larmorx-gpl-cli`
  (golden included: unchanged), `ruff check`/`ruff format --check` on `t2smap-spec/`,
  `check_spdx.py` (all clean). No Rust or Python package of the main workspace changed, so its
  gates were not run.
- **Relay 9 (sub-task 5: the site page): DONE** in one relay (no split; "7l. Relay 9 results"
  below). Gates: ruff check/format on the evidence package, `check_spdx.py`, `tables --check`,
  `pytest validation/tests -k "evidence or summary or tables or difftest"` (124 passed); the
  site: build 0 problems (30 pages, 172 links into the main repository checked against this
  worktree), number check 616/616, viewers 66 none failing, all 3 viewers drawn, no table
  overflowing at 1440, inventory 0 losses on the 23 other evidence pages, their HTML
  byte-identical. No Rust or package code changed (no Rust/package gates needed).
- **Relay 10 (sub-task 6: the trial merge, the handoff, the final report): DONE** (3709e6d4 in the branch;
  the rest in scratch). The trial merge `<scratch>/r10/merge` (main 6840f4d8 + the branch: four conflicts,
  all main's cherry-picks of relay 2 = 0ed9cee2 byte for byte, resolved with the branch's files; merge commits
  8b080889, dfe1b6ab, scratch only) and `<scratch>/r10/site-merge` (the site clone at c40ae49c, a fast-forward
  of 01f901ed), the handoff applied in both, uncommitted. `<scratch>/HANDOFF.toml` (104 operations, 26 files;
  generator `<scratch>/r10/make_handoff.py`, which checks every `old` text against the trial merge) and
  `<scratch>/HANDOFF.md` (reasons, conflicts incl. the dry run with part B's b401e22b, what to verify, the
  agents' ids, the source-derived words in open files); the part's final report is relay 10's final message
  to the orchestrator (the harness refused a report file in scratch).
  Checked: `apply_handoff.py --check` then for real; tables (`--check` passes), register 452 entries, evidence
  summary, site build 0 problems, every built page only gains words; `<scratch>/r10/merge_checks.sh` (log
  `oracles/logs/t2smap-1-relay10-merge-checks.log`): notices tests of crates-gpl and crates-nc, the trial merge's
  wheel, ruff, SPDX, `pytest tests/python tests/golden validation/tests` 1,129 passed / 16 skipped; after the
  one format fix: ruff format 932 files, the audit test 84 passed. Previews `<scratch>/r10/shared-preview.diff`,
  `site-shared-preview.diff`.
- **The part is complete.** Next: the orchestrator's verification and merge (HANDOFF.md).

## 1. Pins (tags made 2026-11-08; upstream.tsv through the handoff)
- **Newest tedana: 26.0.3** (released 2026-04-03; git tag `26.0.3` = commit
  `e0a18e331abd6323c926522999a1c4fd3e695d1c`, `git ls-remote`; no newer tag; PyPI sdist
  `tedana-26.0.3.tar.gz` SHA-256 `3d767a09c2ae3a1e8fc0c62e8c86655de677ebd6424af1f7818af0e3c3aa5b2f`,
  equal to PyPI's digest). `tags/tedana-26.0.3/` (extracted sdist) and `tags/tedana-26.0.3-download/`.
  It is what both Python environments have installed (`.venv` and `oracles/python-newest`).
- **fMRIPrep 25.2.6's image: tedana 25.0.1** (git tag commit `9ecc94f3a7d54394ee54b52bc27198ed35b8c413`;
  sdist SHA-256 `b85742f83f7c853dc709cae8c9aa37f34fd964971fe986296861ad5286c4ccfa`), with numpy 2.2.6,
  SciPy 1.15.2, nibabel 5.3.2, scikit-learn 1.6.1, pandas 2.2.3, conda-forge OpenBLAS 0.3.30
  (pthreads; on this AVX-512 machine it picks Cooperlake under `--cleanenv`), Python 3.12.11, glibc 2.35
  (Ubuntu 22.04.5). `tags/tedana-25.0.1/`, `-download/`.
- upstream.tsv's row today: `tedana ... 45dd96d6 LGPL-2.1 ws` (the workspace clone, a dev commit of
  2026-10-08). Handoff: add `tedana-26.0.3` (tag commit e0a18e33, sdist digest) and `tedana-25.0.1`
  rows as the sdcflows part did, licence `LGPL-2.1-only`.
- The two environments (`OPENBLAS_CORETYPE=Haswell`, both libraries' OpenBLAS on Haswell kernels):
  - shared `.venv`: Python 3.12.16, numpy 2.3.5 (scipy-openblas64 0.3.30), SciPy 1.15.3
    (scipy-openblas32 0.3.28), nibabel 5.4.2, nilearn 0.14.1, pandas 3.0.6, joblib 1.6.0, tqdm 4.70.1,
    threadpoolctl 3.7.0, tedana 26.0.3, mapca 0.0.8, robustica 0.1.4, bokeh 3.10.0, matplotlib 3.11.2,
    scikit-learn 1.9.1;
  - `oracles/python-newest`: the same but numpy 2.5.4 (scipy-openblas64 0.3.34.106.0) and SciPy 1.18.1
    (scipy-openblas32 0.3.31.dev).

## 2. Licence facts (quoted; tedana 26.0.3, identical in 25.0.1)
- `LICENSE` = the verbatim text of the GNU LESSER GENERAL PUBLIC LICENSE, Version 2.1, February 1999
  (502 lines, SHA-256 d716613abed076e2db04e79e7d402d69a10bd57c0d990321a568db528b5795ad), including the
  FSF's "How to Apply" template (its "either version 2.1 of the License, or (at your option) any later
  version" is the template's text, not a notice applied to tedana).
- **No source file has a header**: no copyright line, no licence notice, no SPDX (checked every file of
  the path: `workflows/t2smap.py`, `decay.py`, `combine.py`, `utils.py`, `io.py`,
  `workflows/parser_utils.py`, `config.py`, `__init__.py`, `__about__.py`,
  `resources/config/outputs.json`). `__about__.py`: `__copyright__ = "Copyright 2023, The ME-ICA
  Developers"`; credits: the `.zenodo.json` contributors.
- `docs/index.rst`: "tedana is licensed under GNU Lesser General Public License version 2.1."
  `docs/contributing.rst`: "shipped under an LGPL2 license". README badge: "License-LGPL 2.0" linking to
  opensource.org/licenses/LGPL-2.1. `pyproject.toml`: `license = {file = "LICENSE"}`, classifier "GNU
  Library or Lesser General Public License (LGPL)".
- Reading: a version (2.1) is specified and "any later version" is not, so **LGPL-2.1-only** (LGPL-2.1
  §13). Not legal advice.
- **What LGPL-2.1 allows for our use: §3** "You may opt to apply the terms of the ordinary GNU General
  Public License instead of this License to a given copy of the Library. To do this, you must alter all
  the notices that refer to this License, so that they refer to the ordinary GNU General Public
  License, version 2, instead of to this License. (If a newer version than version 2 of the ordinary
  GNU General Public License has appeared, then you can specify that version instead if you wish.)
  ... Once this change is made in a given copy, it is irreversible for that copy". So a Rust
  translation can be **GPL-3.0** (one version named: "that version"); GPL-3.0 accepts the Apache-2.0
  crates it links (GPL-2.0 would not). Whether "or later" may be added is doubtful for a 2.1-only
  library, so the safe form is **GPL-3.0-only** for the tedana-derived files.
- Staying LGPL-2.1 instead is worse: LGPL-2.1 §2(a) ("The modified work must itself be a software
  library") does not fit a program, and Apache-2.0 is not compatible with (L)GPL-2.x.
- The other code the path reaches is permissive: numpy, SciPy (and the LAPACK, MINPACK, OpenBLAS
  they ship), nilearn, pandas, joblib, threadpoolctl (BSD-3-Clause); nibabel (MIT); tqdm (MPL-2.0 and
  MIT); CPython's argparse, logging, json formatting (PSF-2.0). Their translations may live in the main
  package (Apache-2.0 with their notices), as numpy's and SciPy's already do.

## 3. Proposed package and names (for the orchestrator to confirm, with the user if needed)
- **The replica: `larmorx-gpl`** (`crates-gpl/`), a program as `larmorx-nc` is (`docs/licensing.md`, "A
  replica package is a program"): a new crate `crates-gpl/larmorx-gpl-tedana` (the tedana-derived
  logic only: workflow, adaptive mask, fits' driver, limited/full maps, RMSE, combination, the output
  conventions and messages; SPDX `GPL-3.0-only`, "converted from tedana's LGPL-2.1 under its section 3",
  modification notices, PROVENANCE.md) and `crates-gpl/larmorx-gpl-cli` (the `larmorx-gpl` program:
  `larmorx-gpl tedana t2smap <t2smap's arguments>`, GPL-3.0-or-later; the binary as a whole is
  GPL-3.0); a binary-only wheel `larmorx-gpl` (maturin `bin`, licence files: GPL-3.0 text, tedana's
  LGPL-2.1 text, NOTICE with "Copyright 2023, The ME-ICA Developers" and the permissive notices of
  the Apache crates it includes, the Rust source, as `larmorx-nc`'s wheel carries its source);
  `Package` entry `LARMORX_GPL` in `larmorx_cli::replica` with `LARMORX_GPL_BIN`; `[all]` installs it.
  The workspace's existing tools (glibc verification, LGPL-2.1-or-later) stay out of the wheel.
- **The permissive numerics in the main package (Apache-2.0)**, so the GPL crate holds only tedana's
  logic and nothing BSD/MIT is relicensed (and the clean-room original could later reuse it if the
  audit allows: it is not tedana-derived): numpy's and SciPy's routines as the newest environment
  computes them (below, "Libraries"). Proposed homes: `larmorx-optim` (the crate `docs/architecture.md`
  already plans: SciPy's `least_squares`/TRF, `approx_derivative`, `curve_fit`'s covariance),
  `larmorx_transform::numpy_linalg` extended to tall matrices (the field maps' module: dgesdd's QR
  path, dgelsd, the Haswell kernels' main loops), numpy's reductions/percentiles/formatting in a
  `larmorx_core::numpy` module (or `larmorx-io` for the text formats), SciPy's binary morphology,
  label and gaussian_filter1d plus nilearn's `compute_epi_mask` in `larmorx-image` (nilearn) or
  `larmorx-interp` (ndimage). Final placement agreed in relay 2 with the orchestrator.
- **Python and command line:** family `tedana`: `lx.tedana.t2smap(echoes, echo_times, *, mask=None,
  ..., implementation="auto", upstream_defects="correct", n_threads=1)` returning a typed result
  (dataclass with the maps, the series, the adaptive mask, failures, RMSE table, `implementation`);
  `larmorx tedana t2smap <t2smap's own arguments>` (registry line `Replica { family: "tedana", tool:
  "t2smap", location: Location::Program(&LARMORX_GPL), original: false }` until the original exists).
- **The clean-room original later:** `crates/larmorx-tedana` (Apache-2.0, main package; the AFNI/FSL
  pattern `larmorx-afni`/`larmorx-fsl`), from `specs/t2smap.md`, built for speed, same Python and
  command-line entry (`implementation="original"`).

## 4. Inventory of `t2smap` (tedana 26.0.3; source-derived: goes into `docs/findings/tedana-t2smap.md`)
### 4.1 Command line (`tedana.workflows.t2smap:_main`, argparse, `ArgumentDefaultsHelpFormatter`)
| Option | Values / default | Effect |
|---|---|---|
| `-d FILE [FILE ...]` (required) | existing files (`is_valid_file`: "The file X does not exist!", exit 2) | echo files ascending; ONE file = z-concatenated (deprecated: warning naming issue 1313) |
| `-e TE [TE ...]` (required) | floats | seconds if all in (0, 1) (converted ×1000 to ms, INFO line), ms if all ≥ 1 (deprecation WARNING), else ValueError "TE values must be positive and either all in seconds ..." |
| `--out-dir PATH` | `.` | `os.mkdir` (one level) if missing |
| `--prefix` | `""` | `_` appended unless it ends with `_` |
| `--convention {orig,bids}` | `bids` (= `bidsv1.5.0`) | file names (outputs.json) |
| `--verbose` | off | curvefit only: also variance/covariance maps |
| `--overwrite`, `-f` | off | removes `confounds tsv` first; without it an existing output → RuntimeError "File X already exists. ..." |
| `--mask FILE` | none → nilearn `compute_epi_mask(first echo)` with a WARNING | mask = data > 0 of the (squeezed to 3D) image |
| `--masktype {dropout,decay,none} ...` | `dropout` | adaptive-mask methods (min of base, dropout (appended twice), decay) |
| `--dummy-scans INT` | 0 | removes the first N volumes (also from the combined series); WARNING |
| `--exclude STR` | none | `0,5:10,15` (0-based, end-exclusive; no negatives, steps, open ranges: ValueErrors); excluded from mask and fit, kept in combination and RMSE; not with `--fitmode ts` (ValueError) |
| `--fittype {loglin,curvefit}` | `loglin` | the fit (fMRIPrep passes `curvefit` by default) |
| `--fitmode {all,ts}` | `all` | one fit per voxel, or per voxel and volume (4D maps) |
| `--combmode {t2s,paid}` | `t2s` | Posse 1999 weights `TE·exp(-TE/T2*)`, or PAID `TE·mean/std` |
| `--n-independent-echos INT` | none | only two warnings in the adaptive mask (EPTI) |
| `--n-threads INT` | 1 | `threadpool_limits(limits=n)` (-1 → no limit) and joblib processes for curvefit (-1/≤0 → cpu_count) |
| `--debug` | off | DEBUG log level (help claims a TSV log file: none is written) |
| `--quiet` (hidden) | off | WARNING log level |
| `-v`, `--version` | | prints `t2smap v26.0.3`, exit 0 |
Exit statuses: 0; argparse errors 2 (usage + `t2smap: error: ...`); any exception 1 (Python traceback).
### 4.2 Workflow (`t2smap_workflow`), in order
out-dir; parse exclude; loggers (stream handler on stderr, format `%(levelname)-8s
%(module)s:%(funcName)s:%(lineno)d %(message)s`, INFO); `t2smap_call.sh` (= `"t2smap " + " ".join(argv)`,
no newline); system info; ts+exclude check; TE check; reference image (first echo, or z-cat's first
slab); OutputGenerator (creates an empty `figures/` directory: 26.0.3 no longer passes
`make_figures=False`); mask (user / compute_epi_mask); data loading (float32, (Mb × E × T) for mask
voxels; fast path by boolean indexing, any exception → nilearn `apply_mask` fallback; 3D files →
"Expected 4D image"); dummy scans; exclude (offset by dummies; `> n_vols` check); adaptive mask
(threshold 1; base: first echoes with no NaN and no value ≤ 0 over time, counted consecutively;
dropout: 33rd percentile ("higher") of the first echo's non-zero temporal means, the exemplar voxels'
means / 3 per echo (several exemplars: the one with the largest sum), last echo above its threshold;
decay: first echo whose mean does not decrease; minimum of all; voxels < 1 removed with WARNING
"N voxels in user-defined mask do not have good signal. Removing voxels from mask.") → saved as
`adaptive mask img` (int32); fit (`fit_decay` or `fit_decay_ts`); curvefit: failures saved (uint8),
verbose: variances/covariance; `modify_t2s_s0_maps` (T2* inf → 500, ≤ 0 → 1, a floor where
`exp(-TE/T2*)` underflows: `max(TE)/(-log(eps))`... exactly `min(-TE)/log(eps)`; S0 NaN → 0; limited
copies with adaptive-mask-1 voxels set to 0; **cap only on the limited T2***: values > 10 × the 99.5th
percentile ("lower", `scipy.stats.scoreatpercentile` over the whole limited map incl. zeros) set to
that percentile); unmask; save S0 full; RMSE (`rmse_of_fit_decay_ts` with the LIMITED maps, per voxel
mean over volumes of sqrt(mean over the good echoes of squared residuals), float32; the TSV columns
rmse_mean, rmse_std, rmse_min, percentiles 2/25/50/75/98, rmse_max per volume, `nanpercentile` linear);
save RMSE map, confounds TSV, limited S0, limited T2* (ms/1000 → s); combination with the FULL T2*
(ms) over all volumes incl. excluded (`make_optcom`: echoes 1..n per adaptive-mask value, value 1 uses
2 echoes); save full T2* (s) and the combined series; dataset_description.json; registry json; two
newsletter INFO lines; loggers torn down.
### 4.3 The fits
- **loglin** (`fit_loglinear`): per adaptive-mask value n (1 → 2): `log(|data|+1)` (float32 numpy log),
  design `[1, -TE]` repeated per volume, `np.linalg.lstsq(X, Y, rcond=None)` over all voxels of that
  value at once (one LAPACK `dgelsd` call, NRHS = voxels); T2* = 1/β1, S0 = exp(β0) (float64 exp).
- **curvefit** (`fit_monoexponential`): loglin first as start; per voxel `scipy.optimize.curve_fit(
  s0·exp(-TE/t2s), TE repeated, data, p0=(S0, T2*), bounds=((min(data), 0), (inf, inf)))` → bounds →
  **method 'trf'** (`least_squares`, jac '2-point', x_scale 1 for trf, tr_solver 'exact' (dense SVD),
  ftol=xtol=gtol=1e-8, max_nfev 100·n = 200); NOT MINPACK (only an unbounded `curve_fit` reaches
  `leastsq`). `asarray_chkfinite` (NaN/inf → ValueError), infeasible p0 (p0 outside the bounds, e.g.
  loglin S0 below min(data) or T2* ≤ 0) → ValueError, no convergence → RuntimeError: all caught →
  "failure", the loglin estimate kept, failures map. Covariance: `svd(res.jac)`, singular values above
  `eps·max(shape)·s0`, `(VT.T/s²)·VT · 2·cost/(m-2)`; OptimizeWarning when indeterminate.
  joblib `Parallel(n_jobs=n_threads)` over voxels, tqdm bars "{n}-echo monoexponential" on stderr.
- **fitmode ts**: `fit_decay` per volume (each a 1-volume fit), 4D maps.
### 4.4 Outputs (bids names; orig in brackets) and formats
`desc-adaptiveGoodSignal_mask` [adaptive_mask] int32; `desc-decayFitFailures_mask` [fit_failures]
uint8 (curvefit); `stat-variance_desc-t2star_statmap`, `stat-variance_desc-s0_statmap`,
`stat-covariance_desc-t2star+s0_statmap` [t2s_var, s0_var, t2s_s0_covar] (curvefit + verbose);
`S0map` [s0vG]; `desc-rmse_statmap` [rmse]; `desc-confounds_timeseries.tsv` [confounds];
`desc-limited_S0map` [s0v]; `desc-limited_T2starmap` [t2sv] (s); `T2starmap` [t2svG] (s);
`desc-optcom_bold` [ts_OC]; `dataset_description.json`; `desc-tedana_registry.json` [registry];
`t2smap_call.sh`; empty `figures/`. Images: `.nii.gz`, float64 → float32, int64 → int32, through
nilearn `masking.unmask` (`new_img_like(..., copy_header=False)`): a FRESH nibabel header from the
mask's affine (observed: sform_code 2, qform_code 0 with the quaternion filled, xyzt_units 0, descrip
empty, pixdim[0] = qfac), 4D: zooms of the first echo (TR in pixdim[4]); gzip header mtime 0, no name,
level 1. TSV: pandas `to_csv(sep="\t", lineterminator="\n", na_rep="n/a", index=False)`, float64 repr
(shortest round trip). JSON: `json.dump(indent=4, sort_keys=True)`; dataset_description carries the
host's `platform.uname()`, `sys.version` and 12 library versions.
### 4.5 Messages (observed, fMRIPrep's call, stderr)
`INFO     t2smap:t2smap_workflow:365 Using output directory: <abs>`; `WARNING  utils:check_te_values:796
TE values appear to be in milliseconds. ...`; `INFO     io:__init__:164 Generating figures directory:
...`; `INFO     utils:load_mask:916 Using user-defined mask`; `INFO     t2smap:t2smap_workflow:414
Loading input data: [...]`; `INFO     utils:make_adaptive_mask:167 Echo-wise intensity thresholds for
adaptive mask: [180.95544  65.59678  18.5366 ]` (numpy float32 array print); `WARNING
utils:make_adaptive_mask:195 1424 voxels in user-defined mask ...`; `INFO ... Computing T2* map`, tqdm
bars, `Calculating model fit quality metrics`, `Computing optimal combination`, `INFO
combine:make_optcom:202 Optimally combining data with voxel-wise T2* estimates`, `Workflow completed`,
the newsletter lines. Debug adds shapes, counts, floors, cap, curvefit failure counts. Python
`warnings` (RuntimeWarning, OptimizeWarning) print the installed file path; tqdm prints timings.
### 4.6 Library routines each path reaches (must match exactly)
- numpy: float32 `log` (numpy's own: identical on AVX-512, AVX2 and baseline for every non-negative
  float32, probe `log32_all.py`); float64 `exp`/`log` (**differ by path**: numpy's AVX512F kernels on
  AVX-512 CPUs, the C library (glibc 2.35) otherwise; probe `simd_probe.py`); `linalg.lstsq` →
  LAPACK `dgelsd` (rcond = eps·max(m, n)) with OpenBLAS kernels (dgemv_t/dger/ddot/dnrm2 main loops,
  not only tails: m = E·T rows, NRHS = voxels; per-column results may depend on the column's place in
  the kernel's groups of 4 and on BLAS threads: to measure); `mean` (float32 pairwise), `std`, `average`,
  `percentile` ('higher'), `nanpercentile`/`nanstd`, `unique`, `minimum.reduce`, `diff`, `argmax`;
  float formatting (array print, repr).
- SciPy: `optimize.curve_fit` → `least_squares` TRF (`_lsq/trf.py`, `common.py`, `least_squares.py`),
  `_numdiff.approx_derivative` ('2-point', bounds-adjusted steps), `scipy.linalg.svd` (`dgesdd`, tall
  m × 2: the QR path) under SciPy's own OpenBLAS 0.3.31.dev, `numpy.linalg.norm` (ddot); covariance as
  above; `stats.scoreatpercentile(..., "lower")`. SciPy 1.15.3 → 1.18.1 changed `least_squares`
  (`jac(x)` without `f`, callbacks, `x_scale` handling) and `_numdiff` (491 diff lines).
- nibabel: load (NIfTI-1/2, ANALYZE, and anything `nb.load` reads: MGH, AFNI BRIK/HEAD, MINC, PAR/REC,
  ...: `_convert_to_nifti1`), `dataobj` float32 scaling, `squeeze_image`, `slicer` (z-cat), the fresh
  header (`set_qform` via `mat2quat` → `eigh`), `to_filename` (gzip).
- nilearn 0.14.1: `compute_epi_mask` (mean with smooth=1 → `gaussian_filter1d`, sorted-value gap
  threshold 0.2-0.85, opening 2: `binary_erosion`/`largest_connected_component`/`binary_dilation`),
  `masking.unmask`, `new_img_like`, `apply_mask` (fallback), `check_niimg`.
- pandas 3.0.6 `to_csv`; joblib; tqdm; Python logging/argparse/json/`str.format`.
### 4.7 fMRIPrep's real call and the oracle runs
- fMRIPrep 25.2.6 `init_bold_t2s_wf`: `T2SMap(echo_times, fittype=config.workflow.me_t2s_fit_method)`
  (`--me-t2s-fit-method`, default **curvefit**, or loglin), `mask_file` = the BOLD mask dilated by
  niworkflows `BinaryDilation(radius=2)`; the interface multiplies the TEs by 1000 (ms: tedana 26 warns),
  `minlen=3` echoes, no other option (out-dir = node directory, n_threads 1); fMRIPrep uses
  `T2starmap.nii.gz` (the FULL map) and `desc-optcom_bold.nii.gz`.
- Oracle runs reaching it: **only ds003592-sub-280-noreconall** (3 echoes, TE 14/29.96/45.92 ms, 64×64×43
  × 200, curvefit; node dir `work/.../bold_native_wf/bold_t2smap_wf/t2smap_node`, inputs kept:
  `_echoidx_{0,1,2}/boldref_bold/*_xformresampled.nii.gz`, `dilate_mask/dilated_mask.nii.gz`; fMRIPrep's
  own T2starmap and optcom kept; tedana 25.0.1 took 103 s on the old laptop). Other multi-echo data in
  the catalog: ds000210 (3 echoes, fMRIPrep CI's subject), ds000258 (4 echoes; echo 4 big-endian int16,
  LPS, oblique). tedana's own tests use three- and five-echo OSF data.

## 5. Measured (probes in `probes/`, logs `oracles/logs/t2smap-1-probe-*.log`)
- fMRIPrep's call with tedana 26.0.3 on this machine: loglin 17.7 s (newest) / 9.6 s (shared), curvefit
  55.2 s / 44.1 s, peak RSS 1.25-1.27 GB, 1 thread; 52,826 voxels in the adaptive mask (595 with 2
  echoes fitted, 52,231 with 3), 1,424 removed; curvefit failures 1,549.
- **Library step** (tedana 26.0.3, shared → newest): loglin bit-identical on every output (TSV too);
  curvefit differs (SciPy's TRF): T2* 2,175 voxels differ (the full map's huge values up to 46,929 s
  apart; limited ≤ 0.0022 s), optcom 22,744 of 35.2 M values (≤ 0.00125), TSV differs, failures and
  adaptive mask identical.
- **The step to fMRIPrep's** (fMRIPrep's own outputs, tedana 25.0.1 on the old laptop, vs 26.0.3 newest
  here): T2starmap identical in 173,576 / 176,128 voxels, 76 differ by > 1e-3 relative (max 239,794 s:
  26.0.x no longer caps the FULL map, 25.0.1 did; full-map max now 1,083 s); optcom identical in
  35,186,265 / 35,225,600 values, 3,043 differ by > 1e-3 relative (max 1.56).
- 25.0.1 → 26.0.3 on fMRIPrep's path also changed: data loaded as float32 (25.0.1 float64: the threshold
  line printed 8 digits), the cap moved to the limited map, `nan_to_num`/negative clamps removed, the
  floors applied to the full map, TE units check, figures directory, RMSE from limited maps.

## 6. Defect candidates (TED-01..30 to assign in relay 5, the program; rules A-D)
1. A NaN anywhere in the first echo inside the mask → `ValueError: attempt to get argmax of an empty
   sequence` (the 33rd percentile is NaN, no exemplar voxel): crash on sensible input → **A** (probe
   `edge_probe.py`).
2. `--exclude` index equal to the number of volumes passes the check (`>` instead of `>=`) →
   `IndexError` → **B** (the clear ValueError the check gives for larger indices).
3. No mask and `compute_epi_mask` finds none (tiny/homogeneous data) → `IndexError: index -1 ...` →
   **B** (clear error).
4. The user mask's affine is never compared with the data's ("TODO: add affine check") → **D/B**.
5. 26.0.x caps only the limited T2*; the full map (fMRIPrep's output, the combination's weights) is
   uncapped → **D**, measured in relay 5b (`defects/ted05_full_map_cap.py`): 2-34 voxels per call
   above 10 x the cap (full-map maxima up to 239,794,244 ms), 25.0.1's t2smap capped the full map
   → **TED-05, rule A** (default: the full map gets the same cap; `reproduce`: tedana's).
6. curvefit's lower S0 bound = the voxel's minimum signal; a loglin start outside the bounds (S0 below
   it, T2* ≤ 0) is a silent "failure" → **D** (count which failures are infeasible starts).
   **Measured (relay 4):** of the 1,930 curvefit failures on fMRIPrep's four multi-echo calls, 1,919
   are starts outside the bounds (ds003592 1,563 of 1,563; ds000210 cuedSGT 31 of 41, rest 16 of
   16; ds000258 309 of 310), 11 the evaluation budget (cuedSGT 10, ds000258 1); none other. The
   voxels keep the log-linear estimate. **Measured (relay 5b, `defects/ted06_feasible_start.py`):**
   every infeasible start has T2* <= 0 (signal rising with TE; 5 also S0 below min); refits from
   the clipped start converge but beat the log-linear residuals in only 15-29 % → **TED-06, rule
   C** (keep; register row and findings only, no switch).
7. `--debug`'s help promises a TSV log file that t2smap never writes; docs say limited maps hold NaN
   where the code writes 0 → **C** (document).
8. `make_adaptive_mask` appends the dropout mask twice; RepLGR lines go nowhere → harmless, no ID.
9. Python warnings and tqdm text carry installation paths and timings → not defects; message design.
10. (relay 3, measured) **One infinite sample in one voxel makes every voxel of its adaptive-mask
    group NaN** in the log-linear fit (so in loglin's maps and in curvefit's starting values):
    tedana solves the whole group in one `lstsq`; `log(|inf| + 1) = inf`, so DGELSD's `BNRM` is
    infinite, B is scaled by `BIGNUM/inf = 0` (inf·0 = NaN, the rest 0) and unscaled by `inf`.
    Reproduced bit for bit (`check_lstsq.py`'s edge case; 9 voxels: all T2* and S0 NaN). Does
    tedana's adaptive mask let an infinite voxel through? (base mask: "no NaN, no value ≤ 0":
    inf passes; to check in relay 5 with a real file holding inf). Likely **A** (default: such
    voxels fitted apart or excluded with a warning; `reproduce`: the NaN group).
11. (relay 3) A non-finite TE (`-e inf`, if `check_te_values` accepts it: to check) gives a
    non-finite design: numpy 2.5.4 raises `LinAlgError("SVD did not converge in Linear Least
    Squares")` before LAPACK; numpy 2.3.5 too, through `DLASCL`'s argument check (a NaN norm in
    `DLALSD`, INFO = -4), after printing OpenBLAS's ` ** On entry to DLASCL parameter number  4
    had an illegal value` (and `5`) on stdout. `lstsq` returns `Err(DidNotConverge)` in both;
    the xerbla lines are not printed (the replica may, in its 2.3.5 step only). Rule **B** (a clear
    error for a TE that is not finite) when relay 5 confirms tedana accepts it.

## 7. Threads and memory
- tedana: 1 thread by default; `--n-threads` limits BLAS and makes joblib process pools for curvefit
  (each voxel's fit independent; to measure: does `dgelsd`'s per-column result depend on BLAS
  threads?). The replica: per-voxel (and per-column-group) parallel work in fixed chunks, the same bits
  at any thread count, threads from `--n-threads` (and `OMP_NUM_THREADS` from the wrapper).
- Peak memory to stay under (rule: ≤ 110 % + 8 MiB): tedana 1.25 GB (loglin) / 1.27 GB (curvefit) on
  ds003592; the replica needs the float32 data once (≈ 420 MB) plus the per-chunk work.

## 7b. Relay 2 results (the permissive numerics, main package, Apache-2.0 + BSD-3-Clause)
- **Placement (decided in relay 2, nothing GPL):** `larmorx_core::numpy` (new module:
  `reduce` = `pairwise_sum`, `add_reduce`, `add_reduce_axis`, `mean_axis`, `var_axis`, `std_axis`,
  `nansum_axis`, `nanvar_axis`/`nanstd_axis` (-> `NanVar { values, bad_dof }`), `average_axis`
  (-> `Err(ZeroWeights)`); `quantile` = `percentile_higher`, `nanpercentile_linear_axis0` (->
  `Percentiles { values, float32, all_nan_column }`), `scoreatpercentile_lower`; `umath` =
  `log_f32`, `exp_f64`, `log_f64`), with the trait `NpFloat` (x86-64 SSE arithmetic and NaNs on
  every platform; `math::x86_nan` made `pub(crate)` for it); `larmorx_core::math::logf_numpy`
  (numpy's own float32 log kernel, FMA copy dispatched like the glibc functions). Arrays are
  C-ordered `(outer, n, inner)` with the reduced axis in the middle. PROVENANCE.md updated.
- **numpy's sum order (measured and from the source):** every reduction starts at +0.0 (add's
  identity); along the innermost remaining axis (also an outer axis whose later axes all have
  length 1: a single volume) it is `0 + pairwise` (blocks of 8 accumulators up to 128, halves
  rounded down to multiples of 8; short sums start at -0.0); along an outer axis, in index order.
  No chunking up to 100,000 elements in 1-D. mean/var divide in float64 (NEP 50: f32 / intp).
- **Verified** (`validation/investigations/t2smap/numerics/`, `results.txt`, both environments):
  every reduction/percentile case identical (109 x 9 reduction groups, 7 average, 51 percentile,
  6 nanpercentile, 26 scoreatpercentile); `logf_numpy` = numpy's float32 log on all 2^32 inputs,
  AVX-512 and AVX2/FMA3 paths, numpy 2.3.5 and 2.5.4; float64 exp/log with AVX-512 off =
  `exp_glibc` and `log_glibc` except numpy's negative log = C's +NaN (numpy's wheels bind the old
  symbols `exp@GLIBC_2.2.5`, `log@GLIBC_2.2.5`; `logf@GLIBC_2.27`): `numpy::umath::log_f64`.
- **D2's variable, corrected:** D2's list ("AVX512F AVX512CD AVX512_SKX ...") does NOT turn off
  AVX-512 in numpy 2.5.4 (its dispatch targets are `X86_V3`, `X86_V4`, `AVX512_ICL`,
  `AVX512_SPR`). Use, for both versions (unknown names are ignored):
  `NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"`
  (checked: 2.3.5 keeps AVX2/FMA3; 2.5.4 keeps `X86_V3`). The suite (relay 7) and the record's
  header use this string. Turning off `X86_V3` (2.5.4) or `FMA3 AVX2` (2.3.5) too makes numpy's
  float32 log glibc's `logf` (the baseline path): never do that.
- **Measured for the findings (proposed IDs; relay 5 assigns, after checking the register's next
  NP- numbers, which the field maps' part B may also take):**
  - NP-03 (proposed) numpy's float32 `log` is numpy's own kernel on x86-64 CPUs with AVX2/FMA3 or
    AVX-512 (not correctly rounded: 10,947,841 of the 201,326,593 floats in [1, 2^24] differ from
    the correctly rounded, at most 3 ulp; 78.4 M of all positive floats, at most 4 ulp), the C
    library's `logf` on older x86-64 CPUs and other platforms: tedana's log-linear fit depends on
    the CPU. Convention as D2: default correctly rounded `math::logf`, `reproduce` `logf_numpy`.
  - NP-04 (proposed) numpy's float64 `exp`/`log` on AVX-512 (AVX512_SKX) CPUs are Intel's SVML
    kernels: `exp` differs from glibc's on 4.1 % of arguments in [-800, 0] (tedana's
    `exp(-TE/T2*)`), 4.6 % in [-20, 20]; `log` 0.10 % in [0, 10]; on other CPUs glibc's (and the
    platform's libm elsewhere). The replica follows the non-AVX-512 path in `reproduce` (D2).
  - tedana finding (black-box, open section of `docs/findings/tedana-t2smap.md`): `lstsq`'s
    solution of a column depends on the batch (about a fifth of columns solved alone differ in
    their last bits; a 1-3 column shift changes 2 of 5,003) and on BLAS threads (`--n-threads` 2,
    4, 14 change 4 to 14 of 5,003 voxels' fits): tedana's log-linear maps depend on
    `--n-threads`. The replica reproduces one thread (fMRIPrep's) whatever its own thread count.
- **For the replica's messages and TSV (relays 5-6):** numpy's warnings tedana can print:
  `RuntimeWarning: Degrees of freedom <= 0 for slice.` (nanstd of a volume without RMSE values),
  `RuntimeWarning: All-NaN slice encountered` (nanpercentile), `RuntimeWarning: Mean of empty
  slice` (no voxels), `invalid value encountered in divide`; `nanpercentile` with tedana's list of
  q returns float64 (the lerp in float64; `b - a` in float32), so the TSV's percentile columns are
  float64 while `rmse_mean`, `rmse_std` are float32 values cast to float64 by `column_stack`;
  but when volume 0's column has no number the whole percentile block is float32
  (`apply_along_axis` takes the first column's type) -- `Percentiles::float32`.
- **Handoff text to collect (relay 10):** NOTICE: under numpy 2.5.4 add "larmorx-core's
  `numpy` module and `math::logf_numpy` (numpy's sums, means, variances, percentiles, weighted
  average and float32 log, for tedana's t2smap)"; under SciPy 1.18.1 "scipy/stats:
  scoreatpercentile (larmorx-core's `numpy::quantile`)". Follow-up idea (not this part):
  `larmorx-sdcflows` has its own numpy mean/pairwise sum (its `npy.rs`/`approx.rs`); it could use
  `larmorx_core::numpy` later.

## 7c. Relay 3 results (numpy's `lstsq`, main package, Apache-2.0 + BSD-3-Clause)
- **API:** `larmorx_transform::numpy_linalg::lstsq(a, m, n, b, nrhs, rcond, release, n_threads)
  -> Result<Lstsq { x, residuals, rank, s }, LstsqError>` (also re-exported as
  `numpy_linalg::{lstsq, Lstsq, LstsqError}`): `a` row-major `m × 2` (numpy's layout), `b`
  column-major `m × nrhs` **overwritten in place** (LAPACK's B: the replica builds it directly as
  float64 from the masked data, voxel by voxel, rows echo-major `e·T + t` as tedana's
  `data_cat[vox, :n, :].reshape(-1)`, so no second copy: ds003592's 600 × 52,231 is 251 MB),
  `rcond = None` numpy's `eps·max(m, n)`; `x` row-major `2 × nrhs` (betas[0] intercept,
  betas[1] slope), residuals empty unless rank 2 and `m > 2`; `release` V3_12_1 = numpy 2.5.4
  (also its finite check of `a`), V3_12_0 = numpy 2.3.5; errors: `DidNotConverge` (numpy's
  LinAlgError), `Unsupported` (only `n = 2`, `m >= 2` are ported), `IncompatibleDimensions`.
  Any `n_threads` gives numpy's one-thread result (columns in chunks of 256, each column's
  kernel class from its global index). File: `crates/larmorx-transform/src/numpy_linalg/lstsq.rs`
  (a child module of `numpy_linalg.rs`, using its private LAPACK pieces).
- **Shared code changed in `numpy_linalg.rs`** (field maps' module; behaviour for their calls
  unchanged by construction and by their parity tests): `dnrm2` now follows `nrm2.S`'s
  four-accumulator main loop for `n >= 8` (it asserted `n < 8`) and the full double range (squares
  of subnormals, a subnormal or overflowing root: `round_to_f64`; the 2^±400 fallback is gone);
  `dbdsqr_upper` is a wrapper of the new `dbdsqr_upper_ncc` (VT, U and C with their leading
  dimensions; `clear_negative_zero` = LAPACK 3.12.1's "avoid -ZERO"; the wrapper keeps `true`
  for svd3/eigh's callers, as before). **Open (not this part):** svd3's 3.12.0 path also clears
  -0 singular values (3.12.0 does not): differs from numpy 2.3.5 only for a singular 3 × 3
  matrix with an exactly zero singular value; mention to the orchestrator.
- **What the kernels do (from the source and both libraries' disassembly, GCC 14.2 for 0.3.34,
  GCC 10.2 for 0.3.30):** `dgemv_t_4.c` rows in blocks of 2048 then `m mod 4` tail rows
  (contracted C: `fma(a0,x0,a1·x1)` [+ `fma(a2,x2,·)`] then `+ y`; one row `fma(a,x,y)`);
  columns: groups of 4 → Haswell microkernel (4 FMA lanes by row mod 4, `(l0+l2)+(l1+l3)`), then
  a pair → SSE2 4x2 (2 lanes, rounded products, hadd), then a single → SSE2 4x1 (4 lanes in 2
  registers, rounded products, `(s0+s2)+(s1+s3)`); `dger` = generic ger.c → `daxpy.c`: first
  `16·⌊m/16⌋` rows FMA (microkernel), rest `y + round(a·x)` (daxpy.c, ddot, dscal etc. are
  compiled without -mfma; only rot and gemv_t get `FMAFLAG`); strided `daxpy` not fused;
  strided `drot` `fma(c,x,s·y)`, `fma(c,y,-(s·x))`; `dgemm` small-matrix permit returns 0 (both
  builds), no GEMM→GEMV forwarding on x86-64 (arm64/power/riscv only), Haswell 2×N kernel
  `fma(a2,b2,fma(a1,b1,+0))·alpha + C(=+0)`; `nrm2.S` 4 x87 accumulators (element k of the first
  8⌊n/8⌋ → k mod 4, the rest → 0), total `acc3 + ((acc2 + acc0) + acc1)`.
  LAPACK: `MNTHR = 3`; NB = 32 ≥ K = 2 so every routine is unblocked; DLASDQ sorts the singular
  values *up* after DBDSQR sorted them down (rows of VT and B swapped twice); `DLASCL`'s argument
  check (cfrom 0/NaN, cto NaN) returns INFO < 0 without scaling.
- **Verified** (`validation/investigations/t2smap/numerics/`, `check_lstsq.py`,
  `dump_lstsq_calls.py`, `run_lstsq_check.sh`, README "numpy's lstsq"; log
  `oracles/logs/t2smap-1-relay3-lstsq-real.log`): 550 cases per environment, **all identical** at
  1 and 4 threads in both (numpy 2.3.5/LAPACK 3.12.0 and 2.5.4/3.12.1): tedana 274, random 217,
  edge 53, and **real 6/6**: tedana 26.0.3's own lstsq calls (recorded in-process, BLAS threads
  [1, 1]) on fMRIPrep's three multi-echo t2smap calls: ds003592 sub-280 (400 × 595 two-echo
  voxels, 600 × 52,231 three-echo), ds000210 sub-02 cuedSGT (520 × 3,169, 780 × 28,356) and rest
  (408 × 9,718, 612 × 47,486). The ds000210 calls come from the new oracle run's working directory
  (`oracles/fmriprep-25.2.6/ds000210-sub-02-noreconall/work/...`, both t2smap nodes finished;
  the run itself was still in anatomical registration at 13:51; ds000258's run is not there yet).
  Recorded dumps and cases: `<scratch>/r3/real/` (dump-*/, cases-*/; 3 GB, scratch only).
- **Speed** (ds003592's 600 × 52,231): numpy 0.35 s; larmorx ≈ 0.4 s at 1 thread, 0.2 s at 14
  (`mul_add` per fused operation; `larmorx-transform` forbids unsafe, so no runtime-dispatched
  FMA/SIMD; good enough for the replica; the clean-room original is the fast one).
- **For relay 10's handoff (NOTICE):** under numpy 2.5.4 / OpenBLAS 0.3.34 add "larmorx-transform's
  `numpy_linalg::lstsq` (numpy's lstsq: LAPACK dgelsd and OpenBLAS's Haswell dgemv_t, daxpy,
  drot, dgemm and nrm2 kernels, for tedana's t2smap)"; check that NOTICE already lists
  OpenBLAS 0.3.30's LAPACK (the field maps' part added OpenBLAS notices).

## 7d. Relay 4 results (SciPy's `curve_fit`, main package, Apache-2.0 + BSD-3-Clause)
- **API (new crate `crates/larmorx-optim`, "SciPy's optimizers for larmorx"; workspace dependency
  `larmorx-optim` added to the root `Cargo.toml`):**
  `larmorx_optim::curve_fit2(model: &mut Model, xdata, ydata, p0: [f64; 2], lb, ub, scipy: Scipy,
  libm: &Libm) -> Result<CurveFit { popt, pcov (row-major), warn_cov, nfev, status },
  CurveFitError>` with `Model = dyn FnMut(&[f64] xdata, &[f64; 2] p, &mut [f64] out)` (the model's
  values; curve_fit subtracts `ydata`); `CurveFitError::{NotFinite, EmptyData, LeastSquares(..),
  NotFound (status 0: RuntimeError), CovarianceSvd(..)}` and `.python()` → (class, message) as SciPy
  raises it (`ValueError`, `RuntimeError`, `LinAlgError`: every one caught by tedana's `except
  (RuntimeError, ValueError)`, LinAlgError being a ValueError subclass). Under it
  `least_squares::least_squares_trf2(fun, m, x0, lb, ub, scipy, libm)` (`trf_bounds`, exact solver,
  2-point Jacobian; errors `BoundsOrder`, `OutsideBounds`, `ResidualsNotFinite`, `SZero`,
  `NotWithinTrustRegion`, `Svd(..)`). `Scipy::{V1_18_1 (default), V1_15_3}`: the only difference on
  this path is the memory order of SciPy's SVD factors (1.18.1's batched `_svd` returns C-ordered `U`,
  `VT`; 1.15.3 Fortran-ordered), which sends numpy's `U.T.dot(f)` and `V.dot(x)` to OpenBLAS's
  `dgemv_n` or `dgemv_t`. `libm`: C's `pow` of the method (`(a·b) ** 0.5` and `Delta ** 2` on numpy
  scalars, `denom ** 3` through the power ufunc); arrays' `** 0.5`/`** 2` are sqrt/square
  (`fast_scalar_power`). The model's `exp` is the caller's: for tedana `s0 * np.exp(-tes / t2star)`
  = `p0 · exp((-te)/t2)`, numpy's float64 exp = `larmorx_core::numpy::umath::exp_f64` (glibc) with
  AVX-512 off.
- **In `larmorx_transform::numpy_linalg` (field maps' module, additions only):** `gesdd::gesdd_s2(a
  col-major m×2, m, release) -> Result<Svd2 { u (col-major), s, vt (col-major) }, GesddError
  {NotFinite, DidNotConverge, Unsupported}>` = `scipy.linalg.svd(a, full_matrices=False)` (and
  numpy's): `dgesdd` path 3 (m ≥ 3: dgeqr2, dorg2r, dgebd2 of R, dbdsdc → dlasdq → dbdsqr, the two
  dormbr, `U = Q·U_R` by the Haswell dgemm chain) and path 5s (m = 2); SciPy's two wheels bundle
  LAPACK 3.12.0's routines (no `dlarf1f` symbol in either `libscipy_openblas`; `ilaver` 3.12.0) and
  GCC 10.2 kernels; `blas::{ddot, dot, gemv_t, gemv_n (two rows), matvec(a, rows, cols, Layout::{C,
  F}, x)}` = numpy's `dot` through `cblas_matrixproduct` (row-major dgemv for a C-contiguous matrix,
  column-major otherwise; Haswell `ddot` microkernel: 16 FMA lanes). lstsq.rs's kernel helpers made
  `pub(super)`. Nothing of the existing code changed.
- **Verified** (`validation/investigations/t2smap/numerics/check_curve_fit.py`,
  `run_curve_fit_check.sh`, `dump_curve_fit_calls.py`; logs `oracles/logs/t2smap-1-relay4-*.log`;
  README section "SciPy's `curve_fit` (relay 4)"): **every record identical in both environments**
  (SciPy 1.18.1/numpy 2.5.4 and 1.15.3/2.3.5, AVX-512 off, Haswell, one BLAS thread): dot 13, matvec
  41, SciPy svd 69, numpy svd 67 (3.12.0 and 3.12.1), fit 144, edge 98 and **tedana's own curve_fit
  calls on fMRIPrep's four multi-echo t2smap calls: 169,344/169,344** (ds003592 52,826; ds000210
  cuedSGT 31,525, rest 57,204; ds000258 sub-21262 rest 27,789: four echoes, its oracle run's t2smap
  node had finished). Exits reached: status 1-4, the covariance warning (two residuals), the budget
  (status 0 → RuntimeError), a start outside the bounds, data/Jacobian not finite, residuals not
  finite at the start. `intersect_trust_region`'s two ValueErrors are ported but unreached (none in
  30,000 random fits, `search_rare_exits.py`).
- **Library step** (`compare_curve_fit_envs.py`): different last bits in most fits between the two
  environments (ds003592: parameters differ in 39,211 of 52,826, median relative 9e-10, largest
  0.70; ds000258 largest 3.46), evaluation counts/status in 186-2,168 per call, never the outcome.
- **No Python warnings** came from tedana's curve_fit calls on the four real calls (`warnings.txt`
  empty): relay 5 need not reproduce numpy RuntimeWarnings from inside the fit for real data; edge
  inputs (e.g. a start of T2* = inf) can raise numpy warnings (`invalid value`, `divide by zero`) in
  `_numdiff`/`trf`; their replication (once per place, D6) is relay 5's decision.
- **Speed:** 169,785 records in 33.8 s at one thread (0.2 ms a fit, about 5x SciPy's ~1 ms), 4.6 s at
  12 threads, same bits. The checker's model computes `exp` per element; the replica can compute it
  once per distinct echo time (tedana's `xdata` is each TE repeated per volume: same arguments, same
  bits) and fit voxels in parallel (each fit independent).
- **For relay 10's handoff (NOTICE):** under SciPy 1.18.1 add "larmorx-optim (scipy/optimize:
  curve_fit, least_squares' trust-region reflective method, approx_derivative)" and under the
  OpenBLAS/LAPACK entries "larmorx-transform's numpy_linalg::gesdd and ::blas (dgesdd's paths 3 and
  5s, numpy's dot)"; `docs/architecture.md`'s crate list: `larmorx-optim` moves from "(next)" to
  the list ("SciPy's optimizers: curve_fit with bounds, least_squares' trust-region reflective
  method"); `validation/requirements-lock.txt` unchanged.

## 7e. Relay 5a results (the replica program's first half; GPL from here)
- **Crates (crates-gpl workspace, members and path dependencies added; `crates-gpl/Cargo.lock`
  refreshed):** `crates-gpl/larmorx-gpl-tedana` (lib, `license = "GPL-3.0-only"`, every file's
  header: SPDX `GPL-3.0-only` + "Translated from tedana 26.0.3 (Copyright 2023, The ME-ICA
  Developers; GNU LGPL 2.1), under the GNU GPL version 3 as section 3 of the LGPL 2.1 allows;
  modified 2026 by Karel Lopez Vilaret"; `lib.rs` holds the licence and modification notices;
  `PROVENANCE.md`): `parser` (t2smap's `_get_parser`), `log` (tedana's GENERAL logger
  `%(levelname)-8s %(module)s:%(funcName)s:%(lineno)d`, Python warnings once per place,
  `PyError` + traceback (first and last lines; the stack not reproduced), tqdm-like bars),
  `data` (nibabel class by extension and magic: NIfTI-1/2 single and pair; ANALYZE, MGH, AFNI,
  MINC, PAR/REC, .bz2/.zst refused clearly (D5); `load_ref` incl. z-cat; `load_user_mask`
  (`get_fdata() > 0`, squeeze only for converted images); `load_data`: fast path (boolean index,
  C order: x slowest), nilearn fallback (4D one-volume mask squeezed, empty-mask error, affine
  allclose and shape messages with numpy's 2-D printing, non-finite -> 0, DimensionError text),
  z-cat), `adaptive` (`make_adaptive_mask`, all methods, messages, the n_independent_echos
  warnings with numpy's round and Python repr), `decay` (`fit_loglinear` on
  `numpy_linalg::lstsq`, `fit_monoexponential` on `larmorx_optim::curve_fit2` with the model's
  exp once per TE, voxels in parallel (rayon, fixed results), `fit_decay`, `fit_decay_ts`;
  `Libraries::{Newest, Shared}`), `workflow` (`prepare_and_fit`: out-dir (Python abspath, OSError
  texts `[Errno N] ...: '<path>'`), `parse_volume_indices`, setup_loggers, "Using output
  directory", `t2smap_call.sh` (written: "t2smap " + argv, no newline), ts+exclude error,
  `check_te_values`, ref image, figures directory, overwrite's removal of the confounds TSV,
  user mask (no mask: the warning, then a NotImplementedError: compute_epi_mask is 5b's),
  "Loading input data: [...]" (Python list repr), z-cat deprecation warning, dummy scans,
  exclude (setdiff, offset, checks), adaptive mask, "Computing T2* map", the fit; `t2smap()`:
  the program, which after the fit prints a WIP line and exits 3 (5b removes it)).
  Examples: `t2smap_stages` (stages as .npy), `t2smap_parse` (the parse outcome).
  `crates-gpl/larmorx-gpl-cli`: the `larmorx-gpl` program (`larmorx-gpl tedana t2smap ...`,
  `-V`, `-h`; `LARMORX_UPSTREAM_DEFECTS`; `LARMORX_TEDANA_LIBRARIES=newest|shared`; notices as
  `larmorx-gpl: upstream defect ...` on stderr). Threads: `--n-threads` (tedana's default 1;
  <= 0: all CPUs); results never depend on it (lstsq gives tedana's one-BLAS-thread result).
- **Main package (Apache) additions:** `larmorx_core::python` (`argparse`: options-only parser,
  usage/help with ArgumentDefaultsHelpFormatter, every error and exit; `textwrap`;
  `number`: Python's float()/int()/repr/'%.Nf'; SPDX `Apache-2.0 AND PSF-2.0`, CPython 3.12.16)
  and `larmorx_core::numpy::format` (`array_str`, `array_str_2d`, `scalar_str`: numpy's float
  printing; `Apache-2.0 AND BSD-3-Clause`); PROVENANCE entries in `crates/larmorx-core`.
  `larmorx_core::defects`: TED_01, 02, 04, 07, 08, 09, 10, 11, 12, 13 (texts there; the register
  rows through the handoff). `scripts/check_spdx.py`: files under `crates-gpl/larmorx-gpl-tedana/`
  must be `GPL-3.0-only` (in the branch: a script, not a shared document).
- **Defects (rules A-D; default fixes, `reproduce` = tedana; all verified black-box,
  `check_failures.py`):** TED-01 NaN first-echo mean -> argmax crash (A: percentile/exemplars over
  finite means; no finite mean at all -> clear error); TED-02 `--exclude` index == n_vols ->
  IndexError (B); TED-04 mask affine never compared (B: measured, the 4 real calls' masks have
  affines identical to the data's; default error with nilearn's message, mask vs first echo);
  TED-07 verbose variance maps swapped (A; `Fit` stores true names; the swap on output and the
  notice when `--verbose` are 5b's); TED-08 failures/variances of count-1 voxels not reported (A;
  `Fit.count1_failed/fitted`; the notice is 5b's: when count1_failed > 0, or verbose and
  count1_fitted > 0); TED-09 an infinite sample -> the whole count group NaN (A: that voxel NaN
  alone, the rest solved without it); TED-10 infinite TE -> LinAlgError (B); TED-11
  `--n-independent-echos 0` -> ZeroDivisionError (B); TED-12 one echo -> unrelated numpy errors
  (B; z-cat with one TE); TED-13 `--dummy-scans` >= n_vols or < 0 (B; tedana: argmax crash after
  the "Mean of empty slice" warnings, which the replica prints; a negative count shifts the
  exclude indices up). Reserved for 5b: TED-03 (compute_epi_mask finds nothing -> IndexError, B),
  TED-05 (cap on the limited T2* only: D, likely C), TED-06 (curvefit's infeasible starts: D),
  TED-14 (the floor's debug message counts E x V "voxels": A, message only).
- **New facts (source-derived, for `docs/findings/tedana-t2smap.md`):** numpy puts the indexed
  axis first in memory for `data_cat[:, :, use_volumes]` (strides (T-outer)), so with `--exclude`
  the adaptive mask's temporal mean is summed in order, not pairwise (last-bit differences in the
  thresholds otherwise; `adaptive::temporal_means`); `_convert_to_nifti1` returns NIfTI-1 single
  files as they are (no squeeze of a 4D mask: the fast path then fails and nilearn's path runs,
  which zeroes non-finite data); two TE counts errors (files vs TEs) are tedana's own clear
  `fit_decay` ValueError, after the adaptive mask; one echo's numpy scalar threshold prints with
  numpy's scalar str (`1e-04` style).
- **Verified** (`validation/investigations/t2smap/replica/`, README section "replica/"; logs
  `oracles/logs/t2smap-1-relay5a-stages-{newest,shared}.log`): `check_stages.py` 18/18 identical
  in both environments (adaptive mask, T2*, S0, failures, variances, covariance bit for bit;
  messages up to "Computing T2* map" line by line): fMRIPrep's four multi-echo calls (curvefit;
  ds003592 also loglin) and 13 synthetic cases; `check_parser.py` 109/109 (help at 19 widths,
  errors, abbreviations, clusters, `--`); `check_failures.py` 11/11.
- **For the handoff (relay 10):** NOTICE: add CPython 3.12.16 (PSF License Version 2; its text)
  for `larmorx_core::python`, and numpy's printing under numpy 2.5.4 (`larmorx_core::numpy::format`);
  the root NOTICE and `crates-nc/LICENSES/NOTICE-larmorx.txt` must stay the same file;
  `crates-gpl/README.md` and `crates-gpl/NOTICE` (tedana's notice, the GPL-3.0-only conversion)
  are relay 6's (the wheel); follow-up idea: `larmorx_sdcflows::npy::py_repr` duplicates
  `larmorx_core::python::number::float_repr`.

## 7f. Relay 5b results (the replica program's second half; d6d91faf, abd204e6: the probes)
- **The program is complete:** `larmorx-gpl tedana t2smap` runs tedana 26.0.3's whole workflow and
  exits 0 (the WIP exit 3 is gone). New in `crates-gpl/larmorx-gpl-tedana`: `maps.rs`
  (`_apply_t2s_floor`, `modify_t2s_s0_maps`, `unmask`, `rmse_of_fit_decay_ts`), `combine.rs`
  (`make_optcom`: t2s and PAID; numpy's `average` orders: in index order along the echoes, the
  weights' sum pairwise for `fitmode ts` (echo-innermost in memory, probe `r5b/probe1.py`), both
  pairwise for one volume; PAID's `divide(f32, f32, out=f64)` is a float32 division), `outputs.rs`
  (`OutputGenerator`: names from outputs.json, the existence `RuntimeError`, images through
  nilearn's unmask + a fresh nibabel header (sform 2/qform 0 via `header_for_image_nibabel`'s
  `set_affine_nibabel`; nibabel's "affine close to the fresh header's" quirk keeps codes 0;
  `set_zooms` of the reference's zooms for 4D, its two HeaderDataErrors; `load_mask_img`'s empty
  mask ValueError), pandas' TSV (`scalar_str` of float64, or float32 when `Percentiles::float32`),
  `json.dump(indent=4, sort_keys=True)`, the registry, `uname` for D6), `data::compute_mask`
  (no `--mask`: the reference as nibabel holds it, Fortran, or the z-cat slab's `get_fdata()`, C:
  probed for every file kind), `workflow::finish` (everything after the fit, in tedana's order;
  the adaptive mask is now saved in `prepare_and_fit` before "Computing T2* map", as tedana does).
  `Stages` gained `command` and `outputs`; `Libraries::lapack` is public.
- **Main package (Apache) additions:** `larmorx_interp::ndimage::{filters, morphology}` (SciPy
  1.18.1 `gaussian_filter1d`/`correlate1d` reflect, `binary_erosion`/`binary_dilation` with the
  cross, `label`; no FMA in SciPy's `_nd_image`, objdump), `larmorx_interp::nilearn`
  (`compute_epi_mask` with nilearn 0.14.1's defaults; `Layout` Fortran/C; `Maths` for numpy's
  exp/log); `larmorx_core::python::json` (CPython's `json.dump` with indent); `NpFloat::div`;
  **a fix in shared code:** `larmorx_core::numpy::format::shortest_ties_even`, used by
  `PrintFloat::sci` (f32, f64) and `python::number::float_repr`: between two shortest digit
  strings equally close to the value numpy's Dragon4 and Python's dtoa pick the even last digit,
  Rust's `{:e}` may pick the other (920.45343017578125: `...812`, found in tedana's TSV).
  PROVENANCE entries in larmorx-core, larmorx-interp, larmorx-gpl-tedana.
- **Defects (in `larmorx_core::defects`):** TED-03 (B: no `--mask` and nilearn finds an empty mask
  (MaskWarning, then IndexError in the adaptive mask) or too few voxels (argmax ValueError):
  default a clear error), TED-05 (A, measured), TED-14 (A, message: the floor's debug count),
  TED-15 (PAID with zero temporal std in all good echoes: ZeroDivisionError for the whole run;
  default: those voxels get equal weights (A), a one-volume series a clear error (B)); TED-07's
  swap on output in `reproduce` and its notice with `--verbose`; TED-08's notices (failures map
  when count1_failed > 0; variance maps with `--verbose` when count1_fitted > 0). TED-06: rule C
  (no switch). IDs used: TED-01..15.
- **Verified** (`replica/check_outputs.py`, README section; logs
  `oracles/logs/t2smap-1-relay5b-outputs-{real-newest,all-shared,synthetic-newest}.log`): every
  output file, the logger's lines, exit status and last error line identical to tedana 26.0.3
  (`reproduce`) on 33 cases in both environments (4 fMRIPrep calls + ds003592 loglin, 13
  synthetic of check_stages, 15 new: no-mask f32/scaled-i16/z-cat, PAID all/ts, verbose orig +
  prefix, verbose ts, nine echoes ts/all, one volume all/ts, PAID constant voxels, no-mask
  failure, existing output without/with --overwrite); `check_failures.py` 11/11 still as
  expected. Defect measurements: `defects/ted05_full_map_cap.py`, `defects/ted06_feasible_start.py`
  (logs `t2smap-1-relay5b-ted0{5,6}.log`).
- **Speed and memory (one run, default mode, fMRIPrep's ds003592 curvefit call, 1 thread):**
  11.1 s, peak RSS 494 MiB (tedana 26.0.3: ~55 s, 1.27 GB; relay 1's numbers). Default-mode
  notices there: TED-08 (14 count-1 failures marked), TED-05 (34 full-map values capped).
- **Not done / for relay 6 or later:** (a) the default mode's correctly rounded `exp`/`log`/`logf`
  emit no GLIBC-07/NP notice where a value differs from numpy's (the FSL replicas name GLIBC-07;
  decide whether t2smap computes both and names the difference, relay 6 or 7); (b) Python
  `warnings` from numpy inside the curve fit on edge inputs are not reproduced (none on real data,
  relay 4); (c) the `dataset_description.json` "Python" text is the D6 wording in
  `workflow::finish` (review with the API page).
- **For the handoff (relay 10):** NOTICE: nilearn 0.14.1 (BSD-3-Clause) for
  `larmorx_interp::nilearn` (add under nilearn if a block exists, else a new block) and SciPy
  1.18.1's ndimage filters/morphology under the existing SciPy ndimage block; CPython's
  `Lib/json/encoder.py` under the CPython 3.12.16 block (relay 5a's); register rows TED-03, 05,
  06 (C, no switch), 14, 15 with the numbers above; `docs/findings/tedana-t2smap.md` facts: the
  adaptive mask is written before the fit (an existing file stops the run there), nibabel's
  close-affine quirk, the TSV's float32 case, PAID's float32 division, `fitmode ts`'s pairwise
  weights' sum, the z-cat reference's C layout, tedana 25.0.1 vs 26.0.3's full-map cap.

## 7g. Relay 6 results (the interfaces and the package; 68f1126d, ffd33999, c04c344e)
- **The larmorx command line** (`crates/larmorx-cli`): `replica::LARMORX_GPL` (`larmorx-gpl`,
  program `larmorx-gpl`, `LARMORX_GPL_BIN`, licence `GPL-3.0-only`, install `pip install larmorx-gpl
  (GNU GPL version 3)`); the registry line `Replica { family: "tedana", tool: "t2smap", location:
  Location::Program(&LARMORX_GPL), original: false }`; a new family `("tedana", TEDANA_TOOLS =
  ["t2smap"])` in `FAMILIES` (run_tool's `match family` falls through to `None`: the replica
  lookup always answers first). `larmorx tedana t2smap ...` runs `larmorx-gpl tedana t2smap ...`
  when found, else exit 127 with the install command; `original`: exit 127, "no clean-room
  original of tedana t2smap yet". Tests: `tedana_t2smap_has_only_its_replica`,
  `tedana_t2smap_runs_the_larmorx_gpl_program` (a stand-in shell script, unix), the registry's
  `tools_without_an_original`; `help_uses_the_invoked_program_name` now expects
  `LARMORX_GPL_BIN` in the help (it asserted its absence since the AFNI move).
- **`lx.tedana.t2smap`** (`python/larmorx/tedana/__init__.py`, `t2smap.py`; `tedana` added to
  `larmorx/__init__.py`'s lazy submodules): every t2smap option as a keyword (`mask`, `masktype`
  str or list, `fittype`, `fitmode`, `combmode`, `dummy_scans`, `exclude` str or ints,
  `n_independent_echos`, `convention`, `prefix`, `verbose`, `debug`, `quiet`, `out_dir` (None: a
  temporary directory, read back and removed), `overwrite`, `libraries` (`LARMORX_TEDANA_LIBRARIES`),
  `implementation`, `upstream_defects` ("fix"/"reproduce": the codebase's names, not "correct"),
  `n_threads` (0 -> `--n-threads -1`)); echo times passed as `repr(float)`; in-memory images
  written with `lx.io.save` in their own type; `T2smapResult` (the 11 images by tedana's registry
  keys, `confounds` dict of float64 arrays with NaN for `n/a`, `dataset_description`, `files`,
  `out_dir`, `log` without notice lines, `implementation`); notices -> `UpstreamDefectWarning`;
  choices checked before anything runs. No `.pyi` needed (pure Python, typed; `_core` unchanged).
  `tests/python/test_tedana.py` (18 tests: registry, no program, 10 ValueErrors, a stand-in program
  (arguments, environment, files read back, warnings, log), defaults with a kept out_dir and a
  z-cat file, in-memory echoes and all threads, a failure, the command line, and the real program
  when `LARMORX_GPL_BIN` is set or `crates-gpl/target/release/larmorx-gpl` exists: T2* = 31.3 ms
  on the plain-ratio echoes, wrapper = program bit for bit): 18 pass with the program.
- **Golden registry** (`tests/golden/registry.py`, shared minimal change): `lx.tedana` in
  `NAMESPACES`; `REPLICA_ONLY_FUNCTIONS = {"lx.tedana.t2smap": "tedana t2smap"}`; `GPL_GOLDEN`
  (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`) in `REPLICA_GOLDEN`.
- **larmorx-gpl's golden tests** (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`, `golden.tsv`, 210
  lines, recorded from 68f1126d, clean): 10 cases on a generated 16 x 14 x 8 x 4 three-echo series
  (integer ratios; every 17th head voxel rising, every 23rd losing its last echo), relative paths
  (so `t2smap_call.sh` and `dataset_description.json`'s Command are platform-free),
  `dataset_description.json` hashed without its `Node` block; fingerprints of every file (images
  header/data decompressed), the exit status and the defects named. Cases: loglin (fix,
  reproduce), curvefit + verbose + orig + prefix + 2 threads (fix, reproduce), curvefit shared
  libraries (reproduce), curvefit ts + PAID (reproduce), no mask + float32 + ms + dropout decay +
  dummy 1 + exclude 0 + debug (fix, reproduce), z-cat + masktype none, TED-02 failure (exit 1).
  0.3 s in debug, 0.1 s in release; debug = release bits. **Seen there:** loglin's default and
  reproduce differ in 7 data outputs and the default names nothing: the maths notices (relay 7).
  Dev-dependencies flate2, sha2 (crates-gpl `Cargo.toml` workspace deps; `Cargo.lock` +66 lines,
  offline, sha2 0.11.1 / digest 0.11.3 as the root's).
- **The wheel** (`crates-gpl/pyproject.toml`: maturin `bin`, manifest `larmorx-gpl-cli`, licence
  `GPL-3.0-only`; licence files `LICENSE` (GPL-3.0 text), `NOTICE`, `LICENSES/LGPL-2.1-tedana.txt`
  (tedana 26.0.3's LICENSE byte for byte, sha d716613a..., also 25.0.1's), `LICENSES/Apache-2.0.txt`
  and `LICENSES/NOTICE-larmorx.txt` (copies of the root's LICENSE and NOTICE), the workspace's
  Cargo files and the two crates' sources and PROVENANCE; tools/ stay out). `crates-gpl/NOTICE`
  rewritten (the program; source-code directions: the wheel carries this workspace's source, the
  complete Corresponding Source is the repository at the release's commit; tedana's notice
  "Copyright 2023, The ME-ICA Developers", what was translated and modified, the section 3
  conversion; the included Apache crates; the tools' glibc notice kept). `crates-gpl/README.md`
  rewritten (the package, contents, building, the wheel, the tools). Test
  `larmorx-gpl-cli/tests/notices.rs` (the copies identical to the root's files; tedana's text's
  hash; NOTICE names section 3 and the copyright). Root `pyproject.toml`: `all` installs
  `larmorx-gpl` (comment updated). **Built once** (`maturin build --release --locked` in crates-gpl,
  `<scratch>/wheel-gpl/larmorx_gpl-0.0.1-py3-none-manylinux_2_34_x86_64.whl`): 26 files, the
  program at `larmorx_gpl-0.0.1.data/scripts/larmorx-gpl`, 21 licence files (every glob matched:
  LICENSE, NOTICE, the three LICENSES/ texts, Cargo.toml/.lock, both crates' Cargo.toml, main.rs,
  PROVENANCE.md and the ten tedana sources).
- **CI (D7):** `.github/workflows/ci.yml`'s `gpl` job (comment rewritten): rust-cache also for
  `crates-gpl`; steps `rustfmt (crates-gpl)`, `clippy (crates-gpl)` (`-p larmorx-gpl-tedana -p
  larmorx-gpl-cli --all-targets --locked -D warnings`: the tools are not built), `cargo test
  (crates-gpl)` (the same two packages, `LARMORX_GOLDEN_MISMATCHES`, the failures' annotations).
  **Cost, measured** (`<scratch>/ci_estimate.sh`, log `oracles/logs/t2smap-1-relay6-ci-estimate.log`:
  an empty target directory, 4 jobs, this machine): fmt 2.4 s, clippy 4.5 s, test build + run
  8.3 s (golden 0.3 s). GitHub's runners are about 3-5 times slower: about 1 minute per job cold,
  well under with the cache; x 6 gpl jobs = about 6 job-minutes per CI run cold (rule 10's
  estimate; no new job or runner).
- **Windows:** `workflow::py_abspath` now follows `ntpath` on Windows (drive or share prefix and
  backslashes; it dropped the drive before), unit test per platform; Linux unchanged.
- **API page** `docs/api/tedana-t2smap.md` (implementations, usage, the option-mapping table with
  every option and both environment variables, the outputs table (tedana's descriptions, BIDS and
  orig names, result fields), D6's difference, TED-01..15 with tedana vs default, TED-06 (C), the
  maths (NP-03, NP-04, GLIBC-07; notices: relay 7), validation so far, speed).
- **TED-05's history** (the orchestrator's item): `docs/findings/tedana-t2smap.md` created
  (restricted header; the TED-05 section only, the rest of the findings is relay 7's): the full
  map's cap left t2smap.py in ME-ICA/tedana#1290 ("Write out T2* & S0 variance + T2*-S0
  covariance", tsalo, merged 2026-01-08, released in 26.0.0 on 2026-02-25; closes #1268); its
  description: "Create new function to create the limited maps from the full maps and apply the
  ceilings and floors"; no comment, review or release note mentions the full map's cap; main (checked
  2026-11-08) caps only the limited map; tedana.py capped its full map too before. **Decision: rule
  A stands** (not a design pipelines rely on; fMRIPrep's tedana capped it). Note: the replica's
  default caps the full map with the LIMITED map's percentile (26.0.3's cap value), not the full
  map's own percentile as 25.0.1 did (equal unless the one-echo voxels shift the 99.5th
  percentile): written so in the findings; TED-05's register text says "where tedana 25.0.1's
  t2smap capped the full map too" (true).
- **The maths notices (the orchestrator's second item): NOT done here, planned for relay 7 first**
  (allowed: "Relay 6 or 7, with a case"); the design is in §8, relay 7, step 1.
- **For relay 10's handoff (collect):** `docs/licensing.md` (larmorx-gpl published again: tedana's
  t2smap; its packages table row; "A replica package is a program" names larmorx-gpl; the
  Corresponding Source directions: publish the sdist or point to the tagged commit);
  CLAUDE.md (rule 1's Workbench line: `larmorx-gpl` now also holds tedana's replica, GPL-3.0-only
  for the tedana-derived crate; the repo map's `crates-gpl/` row: "CI does not build it" -> CI's
  gpl job builds and tests the program's two crates; rule 1's restricted list:
  `docs/findings/tedana-t2smap.md`); `docs/validation/golden.md` (larmorx-gpl's 10 cases, how to
  record: `LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-gpl-cli --test golden` in
  crates-gpl, from a clean commit; the orchestrator's golden merge/record scripts may need to know
  the third TSV); `docs/architecture.md` "Adding a replica" step 1 (a new licence family's
  program: larmorx-gpl is the second; it takes its threads from the tool's own `--n-threads`, not
  `OMP_NUM_THREADS`); whenever root `NOTICE` changes, BOTH copies (`crates-nc/LICENSES/
  NOTICE-larmorx.txt`, `crates-gpl/LICENSES/NOTICE-larmorx.txt`) must be the same file (both
  workspaces test it); the API index/README tool table through `validation/tools.toml` (relay 7's
  record entry).

## 7h. Relay 7 results (split: 7a here, 7b next)
- **Relay 7a's agent:** the agent the orchestrator launched at about 17:03 on 2026-11-08 (its id
  in the orchestrator's launch record). Commits: e86180ff (maths notices), 288802de (golden
  re-recorded), 474ff5c8 (parity suite, fixes), 8cc50b49 (two more replica fixes, cases with
  masks), 77e8596c (the record).
- **The default mode's maths notices (DONE, e86180ff):** new `crates-gpl/larmorx-gpl-tedana/src/
  maths.rs`: `Maths` (two families: `Family::Logf` = numpy's float32 log kernel vs correctly
  rounded `logf`, NP-03; `Family::ExpLog` = numpy's float64 exp/log (glibc 2.35) and C's pow in
  the curve fit vs correctly rounded, GLIBC-07), a witness per family (AtomicBool, the OR over
  every argument: deterministic at any thread count; `libm()`/`note_libm` for the curve fit's
  pow; `data::compute_mask` computes nilearn's mask with both choices while the witness looks).
  Every maths switch in decay/maps/combine/data now takes `&Maths` (D13's `reproduce` stays in
  `Notices`). The workflow is split: `prepare` (to the overwrite removal; `Setup`), `fit_stage`
  (mask .. fit; `Sink`), `save_stage` (maps .. combined image), `end` (JSON, last lines);
  `prepare_and_fit` kept for the stages example (returns `(Setup, Stages)`). The program
  (`workflow::run`): first pass with `Sink::Keep` (each image's stored bits over its voxels and
  the TSV's text kept: ds003592's combined series 43 MB) and the log captured from the mask on
  (`Log::start_capture`/`fork`: level and warning registry as at the capture; bars not
  captured); then for each family whose witness saw a difference, `fit_stage`+`save_stage` again
  with `Maths::with_other(family)`, `Sink::Compare` (writes nothing, counts per file the values
  whose bits differ, and the TSV's cells), a buffer log; NP-03/GLIBC-07 named with "would give N
  other written value(s) (file n, ...) [and L other printed line(s)]" or "would make the run fail
  with ...", nothing when none differs; BOTH modes (in `reproduce` the other pass is the
  correctly rounded one). Checked: golden loglin's NP-03 count (244) equals the fix-vs-reproduce
  difference exactly; check_outputs.py 33/33 still identical (log
  `oracles/logs/t2smap-1-relay7-outputs-newest.log`). ds003592 curvefit (fMRIPrep's call, 1
  thread): 31.9 s (3 passes; one pass 11 s; tedana 26.0.3 ~55 s), peak 544 MiB (tedana 1.27
  GB); notices there: TED-08, TED-05, NP-03 (32,950 values), GLIBC-07 (810). NP_03 added to
  `larmorx_core::defects` (after SCI_03; tool "numpy"); GLIBC_07's text now ends naming
  numpy's float64 exp/log and tedana's uses (register row through the handoff). The golden
  test's mismatch description no longer panics on text outputs.
- **Golden (288802de):** `crates-gpl/larmorx-gpl-cli/tests/golden.tsv` re-recorded from clean
  e86180f: only the named-defects lines of 9 cases changed (NP-03 and/or GLIBC-07); every data
  checksum unchanged.
- **The parity suite `tedana-t2smap-replica` (474ff5c8):**
  `validation/src/larmorx_validation/parity/tedana_t2smap_replica.py`, registered in
  `__main__.PARITY_MODULES`; `tests/parity/test_tedana_t2smap_replica.py`. tedana runs as a
  program through the oracle cache: launcher `oracles/inputs/tedana-t2smap-replica/
  launcher-<env>/t2smap` (shebang the env's python -P, recursion guard) + `ENVIRONMENT.txt`
  (python version, every distribution's RECORD sha256: the cache key's "build directory");
  env: Haswell, `NPY_DISABLE_CPU_FEATURES` (D2), `OPENBLAS_NUM_THREADS=1`, `COLUMNS=80`,
  `TQDM_DISABLE=1` (tqdm's frames follow the clock: a dispute on 2026-11-08, resolved with that
  reason, entry 845e66d7); new cache family `python` (`oracle_cache/key.py`: ENV_FAMILIES
  "python": (), FAMILY_ONLY_ENV "python": NPY_, OPENBLAS_, OMP_, MKL_, PYTHON*, COLUMNS, TQDM_;
  no other family's key changes); inputs absolute, `--out-dir out` in a fresh run directory
  (TMPDIR in scratch). Cases (47 standard, 21 smoke): `fmriprep/` (5: ds003592 curvefit +
  loglin, ds000210 cuedSGT + rest, ds000258 rest; standard), `syn/` (28 from check_stages +
  check_outputs; 10 smoke), `fail/` (11 from check_failures; smoke), `spread/` (3: ds003592 +
  two synthetic in fMRIPrep's form; standard). Per case, in BOTH environments: tedana vs replica
  reproduce (exit, last error, log lines + Python warnings normalised to the package path,
  files via check_outputs.compare_dirs; an empty dir only the replica has is ignored: the cache
  does not replay tedana's empty `figures/`); tedana's step shared -> newest (reported; must be
  0 for loglin); the D13 fix pass (newest; a twice-run case names what either run named); a
  named TED for fail cases; fMRIPrep's own T2starmap/optcom vs tedana 26.0.3 and vs larmorx's
  default (reported). Spread: AVX-512 on, `--n-threads 4`, shared releases, tedana 25.0.1 in
  fMRIPrep's image (`run_fmriprep_tedana`, apptainer through the cache), and the replica at 4
  threads = 1 thread. **The trial run (dirty, before the last fixes) gave**: ds003592 spread:
  AVX-512 on 22,899 values differ, 4 threads 1 value, shared 28,808, tedana 25.0.1 176,938;
  syn-curvefit 8 / 0 / 5 / 447; syn-loglin 0 / 0 / 0 / 1,646; fMRIPrep's files vs tedana
  26.0.3: 12,295 (cuedSGT), 12,565 (rest), 35,226 (ds000258), 41,897 (ds003592); vs larmorx's
  default 12,117 / 12,602 / 34,918 / 42,180.
- **Replica fixes the suite found (474ff5c8):** TED-02 in `reproduce`: tedana logs "Excluding N
  volumes from adaptive mask and T2*/S0 estimation" before its IndexError (the replica returned
  first); numpy's warnings differ per release: "Mean of empty slice" (2.5.4) / "Mean of empty
  slice." (2.3.5), `_methods.py` line 134/136, nanstd's 1997/2015, `_nanquantile_unchecked`'s
  1593/1617 (`decay::Libraries::{mean_of_empty_slice, methods_divide_line, nanstd_line,
  nanquantile_line}`; `make_adaptive_mask` and `rmse_of_fit_decay_ts` take the libraries).
- **Memory test (474ff5c8):** `tests/parity/test_memory_peak.py::T2SMAP_CASES` (fMRIPrep's
  ds003592 call, curvefit and loglin; `python -m larmorx tedana t2smap` vs tedana through the
  launcher, both modes, files identical in reproduce); `_original_peak`/`_original_run` take
  `volatile` (shared test file, minimal). Result: see "Record and memory" below.
- **Two more replica fixes (8cc50b49), found by the record's first run (moved aside:
  `<scratch>/r7/record-failed-1/`):** SciPy's `OptimizeWarning: Covariance of the parameters
  could not be estimated` attributed to `tedana/decay.py:111` (source line `popt, cov =
  scipy.optimize.curve_fit(`), once per place, after the group's fits (`VoxelFit` carries
  `warn_cov`; per-volume fits of three echoes reach it; hidden before behind tqdm's frames);
  make_optcom's message "voxel- and volume-wise" whenever `--fitmode ts` (tedana: `t2s.ndim`),
  even for one volume (new parameter `volume_wise`). The suite: bar frames (`\r`) split into
  their own lines before the messages are compared; `_with_masks`: x-nine-ts, x-nine-all,
  x-one-volume, x-one-volume-ts again with a mask of ones (`<inputs>/extra/ones-*.nii.gz`):
  without `--mask` nilearn finds no mask in their 6 x 5 x 4 images (TED-03; relay 5b's
  "verified" nine-echo and one-volume cases had stopped there in both programs).
- **Record and memory:** `docs/validation/tedana-t2smap-replica.{md,json}` from clean 8cc50b49
  (77e8596c): **51 cases, 34 pass, 17 rejected by both, 0 expected, 0 fail**, standard tier
  (5 fmriprep, 32 syn, 11 fail, 3 spread); 104 of 115 original runs served from the cache. Fix
  pass: GLIBC-07 named in 7 cases, NP-03 in 33, TED-01..05, 07..13, 15 where met; every case's
  D13 check passes (named iff changed). Reported: tedana's step shared -> newest 12,285 / 13,130 /
  29,028 / 28,808 values on the four curvefit calls, 0 on ds003592 loglin; fMRIPrep's files
  (25.0.1) vs tedana 26.0.3: 12,295 / 12,565 / 35,226 / 41,897, vs larmorx's default 12,117 /
  12,602 / 34,918 / 42,180; spread (ds003592 / syn-curvefit / syn-loglin): AVX-512 on 22,899 /
  8 / 0, `--n-threads 4` 1 / 0 / 0, shared releases 28,808 / 5 / 0, tedana 25.0.1 176,938 / 447 /
  1,646. Logs `oracles/logs/t2smap-1-relay7-record{,2}.log`. **Memory**
  (`oracles/logs/t2smap-1-relay7-memory.log`): fMRIPrep's ds003592 call, curvefit 545.4 /
  546.1 MiB (reproduce / fix) vs tedana 1,242.8 MiB; loglin 542.9 / 544.6 vs 1,223.8 (2 passed).
- **For relay 10's handoff (collect):** `validation/tools.toml` (tool + record entries, draft
  below) and `tables.py` METRICS `"tedana-t2smap-replica": _bet` (counts the `fmriprep`
  category); `docs/validation/oracle-cache.md`: the runners table (`tedana-t2smap-replica`:
  `tedana_t2smap_replica.run_tedana`, a launcher per environment; threads 1 (4 for the spread,
  served after 3 runs); never: the twice-run cases) and the key's environment table (the
  `python` family); register: NP-03 row (numpy; rule: numpy's own kernel, default correctly
  rounded, `reproduce` numpy's; named exactly by the second pass), NP-04 (AVX-512 SVML exp/log:
  register row only; measured in the record's spread: 22,899 values on ds003592), GLIBC-07's
  text extension; `docs/api/tedana-t2smap.md` (relay 7b: the notices are now implemented: say so
  in its maths section, and the default's time: up to three passes).
  Draft tools.toml:
  `[[tool]] id = "tedana-t2smap"`, title "tedana's t2smap (T2* and S0 maps of multi-echo fMRI and
  their optimal combination)", eco "larmorx" (or a new "tedana" eco if the site wants it),
  page "/tools/tedana-t2smap/", use ["larmorx tedana t2smap", "larmorx-gpl tedana t2smap",
  "lx.tedana.t2smap"], how "exact replica in `larmorx-gpl` (GNU GPL version 3, converted from
  tedana's LGPL-2.1 under its section 3), `experimental` (2026-11-08: every option, message,
  output and failure case of tedana 26.0.3; TED-01 to TED-15 corrected by default or refused,
  each with `reproduce`); the clean-room original later, in the main package", validation
  "replica: [{tedana-t2smap-replica.pass} of {tedana-t2smap-replica.compared}](record:tedana-
  t2smap-replica) compared cases bit-identical to tedana 26.0.3 in both kept environments (every
  file, message and exit status): {tedana-t2smap-replica.fmriprep} of fMRIPrep's multi-echo
  calls on three oracle runs (also against fMRIPrep's own files), every option, data type and
  failure; {tedana-t2smap-replica.both} more rejected by both alike";
  `[[record]] id = "tedana-t2smap-replica"`, larmorx "`larmorx-gpl tedana t2smap`;
  `lx.tedana.t2smap` and `larmorx tedana t2smap` (the replica, GNU GPL version 3)", reference
  "tedana 26.0.3's t2smap (numpy 2.5.4 / SciPy 1.18.1 and numpy 2.3.5 / SciPy 1.15.3); fMRIPrep's
  calls also against fMRIPrep's own files (tedana 25.0.1)", status "`experimental`
  ({tedana-t2smap-replica.pass} of {tedana-t2smap-replica.compared} compared cases bit-identical
  in both environments, fMRIPrep's {tedana-t2smap-replica.fmriprep} calls included;
  {tedana-t2smap-replica.both} rejected by both, with the same messages and status)".

## 7i. Relay 7b results (difftest, trial, fixes, findings, speed, API page, gates)
- **Relay 7b's agent:** the agent the orchestrator launched at about 17:50 on 2026-11-08 (its id
  in the orchestrator's launch record). Commits: d03c83e8 (descriptor + nibabel's qform),
  a519ad06 (descriptor tuning), 291a5751 (the trial's fixes, TED-16..18), 9e5e57cd (more fixes,
  17 permanent cases), f3a401bc (docs, triage, harness), fd84ba6d (speed), 7b9cb9c3 (the suite's collection
  race), ddc275d2 (the record).
- **The difftest descriptor** `validation/src/larmorx_validation/difftest/tools/tedana_t2smap.py`
  (registered in `tools/__init__.py`; `permanent.SUITES` gained `tedana-t2smap-replica`): the
  recipe's `extra` holds the series (shape, geometry, timing, echo times in ms (integers), units,
  TE oddities, z-cat, S0 head phantom + per-voxel T2* drawn, rising/dropout shares, temporal noise,
  background, storage: dtype/scaling/suffix incl. `.hdr` and NIfTI-2/big-endian, specials, mask
  kind), and `materialize(recipe, dir)` writes the files with `gen.header_bytes` (the decay as a
  product of per-ms factors: no transcendental); groups hold `@data`, `@tes`, `@mask`
  placeholders (inputs = {}), the shrinker's moves are the tool's own (`_shrink`: volumes, grid,
  echoes, specials, storage, plumb geometry) plus dropping option groups. The case payload is
  `{"recipe", "tier"}` and `run_random` calls the suite's `_main_case("random", ...)` (both envs,
  the step, the fix pass). Inputs live in TMPDIR, so the cache never stores these runs (their
  outputs name the temporary paths): every random tedana run is fresh. ~4 s a case.
- **The trial:** `oracles/difftest/2026-11-08-t2smap-replica` (seed `t2smap-trial`, 0.5 h, 12
  jobs, 1.5 GiB a case, box 24 GiB, a519ad06): 3,886 cases, 3,816 agree, 11 notices, 59
  differences; all 70 re-run with the final build agree (`recheck_trial.py`, log
  `oracles/logs/t2smap-1-relay7b-recheck2.log`). Check run on the final build:
  `oracles/difftest/2026-11-08-t2smap-replica-check` (seed `t2smap-check`, 10 minutes, 12 jobs, 9e5e57cd): 1,268 cases, 1,267 agree, 1 notice (TED-15 in a run twice: the fix pass now compares the first of two runs, permanent case `ted15-first-of-two-runs`), no difference; its harness process segfaulted at exit after the report (the `_core` replaced in place during the run by the wheel script, now an atomic rename). Probes: `r7b/dt-probe1`, `-probe2` (seed `t2smap-probe`).
- **Replica fixes (all in crates-gpl unless said):** nibabel's `get_qform` in x87 extended
  precision (NEW in larmorx-io, Apache + MIT: `crates/larmorx-io/src/nifti/nibabel_affine.rs`,
  `NiftiHeader::{qform_nibabel, best_affine_nibabel}`, used by `data::load_header` and the
  outputs' close check; `check_affine.py` 4,000 headers identical; example
  `larmorx-gpl-tedana/examples/nibabel_affine.rs`); new images always get sform/qform (the old
  "close to the fresh header" quirk was wrong: nibabel's `Nifti1Pair.__init__` calls
  `_affine2header` unconditionally); NIfTI-2 outputs when no mask and a single-file NIfTI-2 first
  echo (`Outputs::version`; `isinstance(Nifti2Image, Nifti1Image)`: no conversion, and
  `data.rs` squeezes only pairs); `numpy_linalg::lstsq` accepts m = 0 (NEW in larmorx-transform:
  x of zeros, rank 0; shared code, additive); zero volumes: pandas' shape error before the RMSE
  files, the curve fit's empty-minimum ValueError; numpy warnings: empty mean whatever the
  voxels, scalar 0/0 of n-independent-echos (utils 213/230), PAID infinite samples (`_methods.py`
  132/168 reduce, 188 subtract; 134/170/190 in 2.3.5; `combine::paid_warnings`), RMSE T2* = 0
  divide (decay.py:83) and infinite S0 multiply, nanstd subtract (1845/1863) and `_lerp`'s
  (4609-4611 / 4671-4674) on infinite RMSE (`maps::lerp_warnings`); numpy 2.3.5's float32 q in the
  dropout percentile (`adaptive::percentile_higher_shared`); maths notices: the joint pass first
  (both families flipped = the other mode), then single families, else both named ("together");
  TED-07 named only where the swap changes a written value outside count-1 voxels; TED-04 only for
  a 3D mask of the reference's shape. `Libraries` gained the numpy line helpers;
  `make_optcom` takes `libraries`.
- **New defects (in `larmorx_core::defects`; register rows through the handoff):** TED-16 (B:
  `--exclude` removing every volume; default a clear ValueError after the exclusion line), TED-17
  (A: one voxel to fit, IndexError; default fits), TED-18 (A: cap 0 zeroes the limited map;
  default applies no cap <= 0 to either map, so TED-05 is not named then). IDs used: TED-01..18.
- **The suite (`parity/tedana_t2smap_replica.py`):** tedana's step shared -> newest is now
  reported for log-linear fits too (`LOGLIN_STEP_REASON`: LAPACK 3.12.0 -> 3.12.1's dlarf1f), the
  threshold text changed. `check_outputs.describe_image` compares text header fields without
  `equal_nan`.
- **Permanent cases** `validation/difftest/cases/tedana-t2smap/` (17, tier smoke; three
  `register`: ted16/17/18), triage entries appended to `validation/difftest/triage.toml` (12).
- **Findings** `docs/findings/tedana-t2smap.md` (436 lines, restricted header; the last section
  "Black-box facts" is open): versions, licence, the command line, workflow, adaptive mask, fits,
  combination, outputs, messages, the defects table TED-01..18 (+NP-03/04, GLIBC-07), TED-05's
  history, the libraries, random testing's findings, black-box facts.
- **Speed** (`docs/benchmarks/tedana-t2smap.md` and `.json`; `validation/investigations/t2smap/
  bench_calls.py`, best of 2, a shared machine, load 8 at the end): tedana 26.0.3 at one thread
  29.2 / 46.2 / 32.2 / 44.3 s on fMRIPrep's curvefit calls (cuedSGT, rest, ds000258, ds003592),
  6.1 s on ds003592 loglin; the replica, both modes alike (the notices' passes run in both: up to
  four passes, i.e. one more than relay 7a's three), 31.0 / 44.7 / 42.2 / 42.1 s at one thread
  (0.8-1.1x tedana) and 8.9 / 13.3 / 10.8 / 10.6 s at 14 (3.0-4.2x); loglin 10.0 s / 7.9 s
  (0.6x / 0.8x). **For the orchestrator (a decision):** naming the maths defects from the joint
  pass alone (both families named together whenever the other mode changes something: two passes,
  about half the time) instead of attributing per family (four passes); recommended: keep the
  exact per-family attribution (the replica's role is exactness; the clean-room original is the
  fast one), or choose the two-pass form if the replica's speed at one thread matters.
- **API page** `docs/api/tedana-t2smap.md`: TED-16..18 rows, the maths notices as implemented
  (passes), validation (record, random testing, golden, memory 545/543 vs 1,243/1,224 MiB, speed).
- **Gates:** `gates-rust.sh` root OK, gpl OK (after a clippy fix: `contains`), nc OK (logs
  `oracles/logs/t2smap-1-relay7b-gates-{rust,gpl}.log`); `gates-python.sh <worktree> --no-build`
  OK (main wheel's `_core` rebuilt with `r7b/../build_wheel.sh`, which now replaces it by an
  atomic rename) **with TMPDIR unset**: the scratch path's `-home-...` holds `-h`, which the BET
  temporary-folder tests refuse (`test_bet_temporary_folders_hold_no_option_text` and the cache's
  key test); run the Python gates and the smoke tier with TMPDIR unset (/tmp); `tables --check`
  and `check_spdx.py` OK; the parity smoke tier: the serial part (memory and thread tests, t2smap's
  memory cases included) 210 passed, 23 skipped (`t2smap-1-relay7b-smoke.log`); its parallel part
  had failed at collection (12 workers wrote the suite's synthetic inputs at once: fixed in
  7b9cb9c3, a lock and a marker) and was run again alone: 2,511 passed, 8 skipped, 3 failed, the 3 environmental:
  `test_pipeline_stages.py::test_every_program_node_is_classified[<run>-syn]` for the SyN oracle
  runs `ds000228-sub-pixar008-noreconall-syn`, `ds004884-sub-M2079-noreconall-syn`,
  `ds005003-sub-RS001-noreconall-syn` that the field maps' part B made today in the shared
  `oracles/fmriprep-25.2.6/` (their SyN-SDC nodes `fmap_preproc_wf/...`, `syn_preprocessing_...`
  are classified neither in main nor in this branch: part B's to classify; tell the orchestrator)
  (`t2smap-1-relay7b-smoke-parallel.log`).
- **For relay 8 (FIRST, before the spec):** re-make the record from a clean commit (the replica
  changed and the suite's step check's threshold text changed; the 17 permanent cases now join the
  standard tier): `scripts/orchestration/boxed.sh 24 scripts/orchestration/lxv.sh <worktree>
  parity tedana-t2smap-replica --tier standard --out <worktree>/docs/validation --jobs 8` (with
  `LARMORX_GPL_BIN`, the env of `r7b/env.sh`); expect 51 + 17 = 68 cases, 0 failures; commit it.
  **DONE by relay 7b:** ddc275d2 (from clean 7b9cb9c3): 69 cases, 44 pass, 25 both-error, 0
  expected, 0 fail (log `oracles/logs/t2smap-1-relay7b-record.log`); relay 8 needs no record.
- **For relay 10's handoff (collect):** register rows TED-16, TED-17, TED-18 (texts in
  `larmorx_core::defects`; rules B, A, A; the permanent cases as evidence); `docs/validation/
  difftest.md`'s tools table row `tedana-t2smap` (draft: "| `tedana-t2smap` |
  `parity.tedana_t2smap_replica` (`_main_case` on files the descriptor writes from the recipe:
  multi-echo series that share one S0 and T2* map; both environments, tedana's step, the D13 fix
  pass; inputs under TMPDIR, so no random run is cached; the permanent cases in
  `validation/difftest/cases/tedana-t2smap/` (17)) | replica vs tedana 26.0.3's t2smap | two to
  six echoes (s or ms; unsorted, mixed, one too few now and then), one file per echo or
  z-concatenated, 1 to 12 volumes on 6 to 24 voxels across and 1 to 10 slices, drawn geometries,
  float32/float64/int16 (scaled)/uint16/uint8, little- or big-endian, .nii.gz/.nii/pairs/NIfTI-2,
  a head phantom's S0 and per-voxel T2*, rising and dropout voxels, temporal noise (none: PAID),
  NaN/inf/0/negative/-0 samples; masks (ones, thresholded head of four types and values, box, 4D
  of one volume, empty, another grid, another shape) or none; every option, abbreviations and
  `=` forms, runs twice with and without `--overwrite`; its trial `oracles/difftest/2026-11-08-
  t2smap-replica` (seed `t2smap-trial`, 30 minutes, 12 jobs at one thread, 1.5 GiB a case:
  3,886 cases, 59 differences and 11 notices, all fixed or registered, TED-16 to TED-18; every one
  agrees with the final build) and a 10-minute check of the final build (`oracles/difftest/2026-11-08-t2smap-replica-check` (seed `t2smap-check`, 10 minutes, 12 jobs, 9e5e57cd): 1,268 cases, 1,267 agree, 1 notice (TED-15 in a run twice: the fix pass now compares the first of two runs, permanent case `ted15-first-of-two-runs`), no difference; its harness process segfaulted at exit after the report (the `_core` replaced in place during the run by the wheel script, now an atomic rename)) |");
  NOTICE (nibabel's MIT notice: larmorx-io's `nibabel_affine.rs` (get_qform in extended
  precision) under the nibabel block, and numpy's `LONGDOUBLE_dot` under numpy's).

## 7j. Relay 8a results (the spec's text and its black-box probe; 762cd475, 570e1218, 731da621)
- **Relay 8 split** (this relay's decision: the spec and its model are too large for one relay):
  8a wrote the whole text of `specs/t2smap.md` (1,227 lines, 19 sections) and the black-box probe;
  8b writes the model and the thresholds' checker, measures, and revises the spec.
- **`specs/t2smap.md`** (spec writer's role; behaviour only: no code, no tedana routine names
  except inside the message prefixes tedana prints, no line references except those prefixes):
  header (the two implementations; who must not open what: tedana's clone, tags and every
  installed copy, `crates-gpl/`, `docs/findings/tedana-t2smap.md`, `validation/investigations/
  t2smap/`, the replica's parity suite module, its difftest descriptor and permanent cases,
  `docs/parts/`; what may be used: `t2smap-spec/`, tedana's launchers, the main package's permissive
  ports), the sources (Posse 1999, Poser 2006, Kundu 2012/2017, DuPre 2021, tedana's docs), the
  exactness strategy (exact where cheap: masks, files, messages, elementwise maths; thresholds where
  an order decides: numpy's sums, the batched lstsq, SciPy's TRF), versions; §1 purpose and
  fMRIPrep's call; §2 command line (options table, the help at 80 columns verbatim, parser errors,
  exit statuses); §3 the run's 21 steps in tedana's order; §4 inputs (formats and D5, z-cat, the
  reference, user and computed masks with nilearn's algorithm, nibabel's float32 scaling, the
  extended-precision qform); §5 dummy scans and `--exclude`; §6 the adaptive mask (base, dropout,
  decay, minimum, n-independent-echos warnings); §7 the fits (groups, log-linear, the bounded curve
  fit with SciPy's TRF options, failures, covariance, ts mode); §8 floors, limited maps, cap;
  §9 RMSE and the table; §10 the combination (t2s, PAID); §11 outputs (names, order and existence
  checks, image headers, TSV, JSON with D6's wording for the original, the call file); §12
  messages (the log format, 31 messages with their prefixes, Python warnings, errors); §13 numeric
  conventions (precisions, numpy's sum orders, elementwise functions per mode, lstsq and TRF facts,
  percentiles, printing, NaN, the main package's permissive ports by name); §14 larmorx's
  interfaces; §15 the defects TED-01..18 (the default's error texts and notices as observed from the
  replica), NP-03/NP-04/GLIBC-07, rule C items; §16 what must stay / may change, memory and speed
  numbers; §17 the validation plan (reference setup, cases, **provisional** thresholds: identical
  where exact; the log-linear maps in the rate domain; the curve fit by parameters or by the
  objective (F' <= F (1 + 1e-6)) or a start's feasibility; the RMSE and the combination by bounds
  derived from the maps' differences; tedana's own spread quoted); §18 the model (8b); §19 open
  points.
- **`validation/investigations/t2smap-spec/blackbox.py`** (+ README): 54 cases on a generated
  16 x 14 x 8 x 6 three-echo phantom; run (boxed, 8 GiB) against tedana 26.0.3 (newest launcher)
  and against the replica in both modes: reports and every case's output directory in
  `<scratch>/r8/bb-ted.txt` + `bb-ted/`, `bb-rep.txt` + `bb-rep/` (reproduce), `bb-fix.txt` +
  `bb-fix/` (default); the runner `<scratch>/r8/run_bb_replica.sh`. The replica's `reproduce`
  report = tedana's except the warnings' location lines (D6), larmorx's NP-03 notices, the progress
  bars and gzip's xfl byte (the check that the probe sees what matters).
- **Observation for the orchestrator (not fixed here):** the replica prints tqdm's progress bars
  in both modes although `TQDM_DISABLE=1` is set (tedana prints none then); the parity suite
  normalises bars away, so the record does not see it; harmless (bars are not part of the contract,
  D6), a small follow-up if wanted (the replica's `log.rs` `progress` could honour TQDM_DISABLE).
- **Open point for the orchestrator (spec §19, point 1):** the clean-room original's NP-03 and
  GLIBC-07 notices: the replica's exact rule (up to four passes) or a one-pass rule (named when the
  other function's value differs on an argument that reaches an output). Recommended: the one-pass
  rule for the original (it is the fast one; the replica keeps the exact attribution), to be
  decided before the clean-room part starts.
- **For relay 10's handoff (collect):** the audit's restriction table for the tedana family (D9)
  should also list `validation/investigations/t2smap/`, `validation/src/larmorx_validation/parity/
  tedana_t2smap_replica.py`, `validation/src/larmorx_validation/difftest/tools/tedana_t2smap.py`
  and `validation/difftest/cases/tedana-t2smap/` (the spec's header names them as not to be opened);
  `validation/investigations/t2smap-spec/` is open (written from the spec and black-box runs only).

## 7k. Relay 8b results (the model, the thresholds, the spec's revision; 1048a174, faf9ddda, cb4b8874)
- **`validation/investigations/t2smap-spec/model.py`** (written from the spec's §§3-11 only,
  never from tedana's source: numpy, SciPy, nibabel, nilearn's `compute_epi_mask`; §13.2's sum
  orders and §13.6's percentile rules written out; SciPy's `curve_fit` with §7.3's options passed
  explicitly; the TSV with Python's `repr`; NIfTI-1 single files, `reproduce` only, files only;
  `--variant fast`: per-voxel closed-form log-linear fit, analytic Jacobian, other sum orders).
  **Bit-identical to tedana 26.0.3** in the reference setup (`oracles/python-newest`, the env of
  `blackbox.py`) on all 30 of blackbox's completed cases and on fMRIPrep's four curvefit calls,
  ds003592's loglin and ds003592's with `--verbose` (52,826 curve fits and their variances).
- **`check.py`** = §17.3 between two output dirs (the command from `t2smap_call.sh`, inputs read
  again with the model's readers; per output: values, identical, by rule, failed, the largest
  "use" = |diff|/allowance; `--details`). **`measure.py`** = the runs: per case tedana `ref`,
  `shared` (launcher-shared), `threads4` (`--n-threads 4`, OPENBLAS_NUM_THREADS unset),
  `avx512` (NPY_DISABLE_CPU_FEATURES unset), `model`, `model-fast`; checks; the table of largest
  uses. Reports: `<scratch>/r8b/{bb,fp,fpv}/report.json` (the first runs, during the work); **the
  confirmation from cb4b8874 (the final code): `<scratch>/r8b/final2-bb.txt`, `final2-fp.txt`
  and their work dirs `final2-bb/`, `final2-fp/` (each case: `ref/`, `shared/`, `threads4/`,
  `avx512/`, `model/`, `model-fast/` with `out/` and `stderr.txt`, `check-<run>.json`); log
  `oracles/logs/t2smap-1-relay8b-final2.log`, last line `R8B-FINAL2-END`**. (`final-*` are an
  earlier confirmation whose report.json a partial re-check shrank: superseded.) Relay 9 may use
  `final2-fp/fmriprep/ds003592-curvefit/` for the site's "tedana's own variability" panel.
- **The checker's own bug found and fixed on the way:** the written T2* maps are in seconds; the
  first version used them with ms echo times in the objective (b) and the combination's weights,
  which made (b) pass trivially. Fixed before any number below.
- **Measured (largest uses; tedana's spread / the fast variant):** loglin T2*/S0 identical on every
  voxel (both); curvefit (a) 0.997/0.995 (the rest by (b)); (b)/(c) share 0.28 %/0.31 % of the fit
  voxels, all by (b), F'/F-1 in [-1.9e-8, 3.2e-8]; the cap 0.005 (as an order statistic: tedana's
  cap moved by 2.6e-6 relative in the shared env, which the provisional "c within the T2*
  tolerance" failed); variances at most 8.1e-6 relative where (a) holds on ds003592 --verbose (the
  provisional 1e-4 kept: 12x; a first reading of 9.9e-5 had mixed in voxels outside (a), fixed in
  the checker: the use is taken over (a)'s voxels only); RMSE 0.43/0.14 (after adding the float32 summation bound
  2^-24 (T + E + 64) RMSE: the fast variant's plain sum over 200 volumes differed by 7 ulps where
  T2* was floored and the predictions ~0); table 0.07/0.02; combined 0.85/0.44 (after adding the
  T2* float32-rounding term 2^-20 (|dc/dlnT2*| + |c|): the one-ulp form failed tedana's own
  spread in 6-242 values per call).
- **Spec revised:** §17.3 rewritten (measured numbers in brackets, "provisional" gone; (b)/(c)
  share 3 %; variances 1e-4 kept; the cap as an order statistic; the RMSE's and the table's summation
  bounds; the combination's rounding term); §15.2 the decided one-pass rule for the original
  (NP-03 never named by the original; GLIBC-07 where a written log-linear S0 = exp(b0) would
  differ, with the notice texts) and §16's passes bullet; §6.2: several exemplars are ranked by the
  sum of their **thirds** (tedana's `lthrs.sum(axis=0).argmax()` on the means / 3: checked in the
  source as the spec writer), not of the means; §9: the RMSE map's and the table's sums are numpy's
  `nansum` (NaN as +0 in place); §11.4: the table's last line ends with `\n`; §12.3: tqdm's
  `TQDM_DISABLE` rule (any non-empty value, observed on tedana); §13.4: the closed form measured;
  §18 (model, check, measure, results), §19 (how checked; open points 1 decided, 2 done, 4 what the
  model does not cover). The header names the model's confirmation.
- **The TQDM_DISABLE fix in the replica** (1048a174): `log::tqdm_disabled` (tqdm 4.70.1's rule:
  names `TQDM_` + `disable` in any case, the last winning, any non-empty value; on Windows the
  names upper-cased as Python's os.environ holds them), `Log::with_bars`; unit tests; observed:
  no bars with `1` and `0`, bars with an empty value, as tedana. The record's comparison masks the
  bars, so `tedana-t2smap-replica` (from 7b9cb9c) is unaffected; golden unchanged (tested).
- **For relay 10's handoff (collect):** the findings' tqdm line changed (docs/findings/
  tedana-t2smap.md, source-derived, restricted); `t2smap-spec/` stays open (model.py, check.py,
  measure.py written from the spec and black-box runs; measure.py imports blackbox.py only).

## 7l. Relay 9 results (the site page; larmorx 9fb37590..78f8432b, site c40ae49c, blobs ae1a3b62)
- **The evidence module** `validation/src/larmorx_validation/evidence/tedana_t2smap.py` (in `TOOLS`
  as `tedana-t2smap`; `summary.PAGES` gained the page: "larmorx-gpl replica",
  `tedana-t2smap-replica`, experimental, eco `larmorx`). Figures-cache entries (computed boxed,
  16 GiB, log `oracles/logs/t2smap-1-relay9-entries.log`; tedana runs through the oracle cache,
  most served): `oracles/figures-cache/tedana-t2smap/call-ds003592-sub-280-rest-curvefit` and
  `call-ds000258-sub-21262-rest-curvefit` (runs: tedana 26.0.3 reference setup, the replica
  reproduce 1 and 14 threads and default 14, tedana AVX-512 on, shared releases, `--n-threads 4`,
  tedana 25.0.1 in fMRIPrep's image), `memory` (ds003592 curvefit and loglin under `/usr/bin/time
  -v`: tedana 1,243 / 1,223 MiB, replica 547/545 and 544/544 MiB: 44 %). Export:
  `<scratch>/r9/export.sh [--no-compute]` (boxed 16; logs `oracles/logs/t2smap-1-relay9-export{1..8}.log`;
  the JSON's exporter fingerprint = HEAD 78f8432b's evidence tree, cd89a3ab).
- **The page** `content/tools/tedana-t2smap.md` (title t2smap, kicker "Tools · tedana", status
  experimental): facts, what it is (decay model, adaptive mask, fits, combination, fMRIPrep's
  call, CLI + Python examples, the licence/package, the permissive numerics in larmorx itself,
  threads, the notices' passes), the agreement chart (69: 44 bit-identical, 25 both-reject), three
  examples: `maps-ds003592` (inputs echo volumes 1 + fMRIPrep's mask; outputs T2*, S0, limited
  T2*/S0, adaptive mask, fit failures; first window of the T2* maps 0 to their 95th percentile;
  the default view on TED-05's largest capped voxel), `optcom-ds003592` (3 of 200 volumes,
  fMRIPrep's own combined series a panel), `maps-ds000258` (4 echoes, big-endian float32, LPS
  oblique); tables `calls` (5 calls from the record + the benchmark's named defects), `files`
  (every image and the confounds TSV of the shown calls x 7 comparisons, NaN alike: the RMSE map
  holds NaN, kept out of the viewer), `spread` (the record's spread cases), `corrections`
  (TED-01..18 from the findings' table without its relay notes, NP-03, GLIBC-07, NP-04; the
  record's D13 list for the cases), `memory`, `trial` (trial + check), `triage`; speed chart
  (benchmark JSON); every case; known limits. Measured on the page: ds003592: replica = tedana in
  every map value; default vs reproduce 7,561 of 1,056,768 map values (TED-05: 34 voxels capped);
  fMRIPrep's own T2* = tedana 25.0.1 run here (0 differ; its run was on the 12-CPU machine), vs
  tedana 26.0.3 2,531 values; ds000258: 14 capped; fMRIPrep's own run (this machine, AVX-512 on)
  vs tedana 25.0.1 here (AVX-512 off) 581 values. Verdicts: replica-tedana bit-identical;
  replica-fMRIPrep within-variability (tedana's own version step); default-fMRIPrep d13.
- **Checks** (`validation/investigations/tedana-t2smap-site/site_checks.sh` + README; outputs in
  `<scratch>/r9/`: `site-before/` (the `_site` of 01f901ed), `inv-before/`, `inv-after/`,
  `checks/`, `shots/` (56: the top and 13 components at 1440 and 390, light and dark),
  `shots-old/` superseded): see the State entry. Looked at: the three examples (both themes),
  the calls table at 390, the corrections table, the top at 390. Fixed on the way: the T2*
  window (tissue near black in 0.5-99.5 %), the maps' metrics table and the corrections table
  overflowing at 1440 (shorter output labels; the shortest case name as wrapping text), internal
  names ("relay 4") in the findings' TED-06 row and in the suite's STEP_REASON (the page uses its
  own texts: VERSION_STEP, RELEASE_STEP), `>f4` written as "float32, big-endian".
- Note: `pkill -f` with a bracketed pattern still kills its own shell when the same command line
  contains the unbracketed text later (exit 144 once; nothing lost): run pkill alone.
- **For relay 10's handoff (collect; the site's shared files):**
  - `static/evidence/summary.json`: `python -m larmorx_validation evidence summary --site <site>`
    after the merge (the page's entry is in `summary.PAGES`, this branch).
  - `site.yml` (the tools list, after sdcflows' entry): `- { title: t2smap, url:
    "/tools/tedana-t2smap/", eco: larmorx, summary: "tedana's T2* and S0 maps of multi-echo fMRI
    and the echoes' optimal combination: an exact replica in larmorx-gpl (experimental), every file
    bit for bit in both library environments, fMRIPrep's calls included, with 44 % of tedana's
    memory; the clean-room original later." }`
  - `data/status.yml`: generated from `validation/tools.toml` (the draft in 7h).
  - `content/roadmap.md`: the multi-echo step's row (replica done, its page; the clean-room
    original next in the order) and a completed row of 2026-11-08 "**Multi-echo: tedana's
    t2smap replica.**" (the T2*/S0 fits and the optimal combination fMRIPrep runs for every
    multi-echo run, translated from tedana 26.0.3 with numpy's, SciPy's, nibabel's and nilearn's
    arithmetic, in the larmorx-gpl package (GPL-3.0 under LGPL-2.1 section 3); every file,
    message and exit status bit-identical in both library environments in every compared case
    of its record (69: 44 + 25 rejected by both), fMRIPrep's five calls on three oracle runs
    included; TED-01 to TED-18 (and NP-03, NP-04, GLIBC-07); 44 % of tedana's memory, a quarter
    to a third of its time at 14 threads; the specification for the clean-room original.
    [The page](/tools/tedana-t2smap/)).
  - The orchestrator pushes `evidence-blobs` (ae1a3b62: 1343 = 1275 + 68 new, none replaced or
    removed; manifest SHA-256 f29298798b957e77) before the site's `main`; the page's JSON was
    exported against site main 01f901ed: if main moved, re-export with `--no-compute` after the
    merge (the blobs are content-addressed: unchanged).

## 8. Relay plan (sub-tasks 2-6 split; each relay: commit, NOTES, stop)
- **Relay 2 (2a) permissive numerics I: DONE** (above), except `lstsq` (moved to relay 3).
- **Relay 3 (2a') numpy's `lstsq`: DONE** (7c. above). The plan it followed: `git merge main` in the worktree (main >= 4ead4671; expect no conflict:
  relay 2 touched only `crates/larmorx-core` (lib.rs one line, math/mod.rs, math/x86_nan.rs
  visibility, new files) and `validation/investigations/t2smap/`; main changed
  `crates/larmorx-core/src/defects.rs`). Then `lstsq(X, B, rcond=None)` as numpy 2.5.4 (and 2.3.5)
  runs it: `_umath_linalg.lstsq` (B copied to Fortran order, rcond = eps * max(m, n), work
  queries), LAPACK `dgelsd` for X = m x 2 (m = echoes x volumes) and NRHS = voxels: both paths
  (m >= mnthr = int(1.6 * 2) = 3: `dgeqrf`(`dgeqr2`), `dormqr`('L','T') on B (k = 2 < nb: the
  unblocked `dorm2r` -> `dlarf`/`dlarf1f`: `dgemv_t` m x NRHS and `dger`), `dgebrd`(`dgebd2`) of
  R, `dormbr`; m < 3 (two echoes, one volume): `dgebrd` of X directly), `dlalsd` with n <=
  SMLSIZ (`dlasdq` -> `dlasr` on B's rows, the threshold `rcond * max(d)`, `dlascl` row scaling,
  `dgemm`('T','N', 2, NRHS, 2) back, `dlacpy`), `dormbr`('P'), `dlascl` of A and B when their
  norms are outside [smlnum, bignum]; with `numpy_linalg::LapackRelease` (3.12.1's `dlarf1f`
  vs 3.12.0's `dlarf`). The OpenBLAS 0.3.34/0.3.30 Haswell kernels on these shapes, main loops
  included: `dgemv_t` (`dgemv_t_4.c` + `dgemv_t_microk_haswell-4.c`: 4 columns at a time, the
  row loop's accumulator order), `dger` (`daxpy` microkernel per column), `dgemm`
  (`dgemm_kernel_4x8_haswell.S`, k = 2, beta = 0, the N blocking: the 1-3 column shift effect),
  `dscal`/`dnrm2` as `numpy_linalg` has them. Target: the whole batch at one BLAS thread
  bit-identical to numpy in both environments (`lstsq_columns.py`'s shapes; random tedana-like
  batches of 1..60,000 columns, m from 2 to 5 x 600; fMRIPrep's ds003592 log data, from the
  replica's own masked arrays or a probe's dump), per-column independence of the Rust code's
  threads (fixed column chunks aligned to the kernel's blocks so every column sees numpy's
  one-thread context). Home: `larmorx_transform::numpy_linalg` (extended; it is main's after the
  merge) or a sibling module `numpy_lstsq.rs` there.
- **Relay 4 (2b) permissive numerics II: DONE** (7d. above). The plan it followed: SciPy 1.18.1 `curve_fit`/`least_squares` TRF
  with `approx_derivative` ('2-point', bounds-adjusted steps), `scipy.linalg.svd` (gesdd tall
  `m × 2` path under SciPy's own `libscipy_openblas` 0.3.31.dev: check its kernels' build (GCC,
  FMA flags) in its disassembly as relay 3 did for numpy's; `numpy_linalg`'s pieces and
  relay 3's long kernels (`lstsq.rs`: `gemv_t_column`, `axpy_column`, the classes) are there to
  reuse: move shared ones into `numpy_linalg.rs` if both need them), `numpy.linalg.norm`
  (ddot), the covariance (`svd(res.jac)`, threshold `eps·max(shape)·s0`), every exit status
  (`status` 0..4, `max_nfev = 200`, ValueErrors of `asarray_chkfinite` and infeasible `p0`,
  OptimizeWarning), tedana's bounds `((min(data), 0), (inf, inf))` and `p0` from loglin; voxel
  by voxel against SciPy in both environments on random voxels and on the real voxels (the dumps
  of relay 3 hold ds003592's and ds000210's log-linear inputs; a curvefit dump can record each
  voxel's `curve_fit` call the same way, `dump_lstsq_calls.py` as the model); the step 1.15.3 →
  1.18.1 measured (relay 1: curvefit's T2* differs in 2,175 voxels on ds003592). Smart testing:
  every code path once, not sweeps.
- **Relay 5a (2c, first half): DONE** (7e. above).
- **Relay 5b (2c, second half): DONE** (7f. above). The plan it followed: in `crates-gpl/larmorx-gpl-tedana` (and permissive pieces
  in the main package), following tedana 26.0.3's `t2smap_workflow` after the fit, then remove
  the WIP exit 3 of `workflow::t2smap`:
  1. nilearn 0.14.1's `compute_epi_mask` (no `--mask`): `compute_mean` (mean over time of the
     reference image's float data, `smooth=1` -> `_smooth_array` = SciPy `gaussian_filter1d`
     per axis, fwhm 1 with an identity affine), `ensure_finite`, sorted values, the 0.2-0.85 gap
     threshold, `_post_process_mask` (opening 2: `binary_erosion` iterations 2, the largest
     connected component (`ndimage.label` + bincount argmax), `binary_dilation` iterations 4,
     and its `mask & original` step if any; read nilearn's source), the int8 mask image; SciPy
     ndimage's `correlate1d` (mode reflect, truncate 4.0), binary morphology and `label` in an
     Apache crate (`larmorx-interp::ndimage` or `larmorx-image`); TED-03 (nothing found ->
     IndexError: B, clear error). Check on the synthetic cases without `--mask` and on the 4 real
     calls' first echoes (tedana's mask vs ours, bit for bit).
  2. `modify_t2s_s0_maps`: inf -> 500, <= 0 -> 1, `_apply_t2s_floor` (exp(-TE/T2*) == 0 test with
     numpy's exp = `exp(reproduce)`; floor `min(-TE)/log(eps)`; 2-D per volume for ts), S0 NaN ->
     0, limited copies with count-1 voxels 0, the cap: `scoreatpercentile(limited.flatten(), 99.5,
     "lower")`, values > 10 x cap set to cap; debug lines (decay:58 floor (TED-14), decay:624 cap
     `{cap*10:.5f}`). TED-05: measure (D) on the real calls (full map's uncapped values, their
     effect on the combination), then C (document) or A.
  3. `utils.unmask` to the mask's voxels; `rmse_of_fit_decay_ts` (float32 predicted values from
     float64 `s0 * exp(-te / t2s)`, squares, mean over echoes (outer axis, in order), sqrt;
     `nansum`/count maps and series with the float64 divide into float32; `nanstd`;
     `nanpercentile(rmse, [0, 2, 25, 50, 75, 98, 100], axis=0)` (`Percentiles::float32`); numpy's
     warnings: "Degrees of freedom <= 0 for slice.", "All-NaN slice encountered", divide).
  4. `combine.make_optcom` (read combine.py: t2s weights `TE * exp(-TE/T2*)`, paid
     `TE * mean / std`, the adaptive mask's echo counts, `np.average`'s order and its
     ZeroDivisionError, messages combine:181/187/202).
  5. The writers through `OutputGenerator.save_file`: the existence check (RuntimeError "File X
     already exists. In order to allow overwrite please use the --overwrite option in the command
     line or the overwrite parameter in the Python API."), names from outputs.json (both
     conventions, prefix, `.nii.gz`/`.tsv`/`.json`), images through nilearn's `masking.unmask` +
     `new_img_like(copy_header=False)`: a fresh nibabel header from the MASK's affine
     (`larmorx_io::nifti::header_for_image_nibabel`: sform 2, qform 0 with the quaternion),
     float64 -> float32, int64 -> int32, uint8 failures, 4D: `set_zooms(reference zooms)` (TR;
     converted references have float32 zooms), gzip as nibabel writes (compare data and headers,
     not compressed bytes, as other records do); the TSV (pandas `to_csv(sep="\t",
     lineterminator="\n", na_rep="n/a", index=False)`, float64 repr, the float32 case); JSON
     (`json.dump(indent=4, sort_keys=True)`; `dataset_description.json` per D6: tedana's keys,
     the host's uname, larmorx named where tedana names Python and its libraries; the registry
     with relative names); the order of files as tedana writes them (adaptive mask first, failures
     and verbose maps, S0, RMSE, TSV, limited S0, limited T2* (ms -> s), combined, T2* (s), JSON).
  6. TED-07's swap on output in `reproduce` and its notice (verbose); TED-08's notice; TED-06:
     measure (D) what a feasible start gives on the real calls' 1,919 infeasible-start voxels
     (e.g. the start clipped into the bounds: converged fits, RMSE against the log-linear
     estimate), then A or C.
  7. The end: "Calculating model fit quality metrics", "Computing optimal combination",
     "Workflow completed", the newsletter lines (utils 812, 816), teardown; exit 0.
  8. Extend `check_stages.py` (or a sibling) to compare every output file of tedana and the
     replica (data bit for bit, headers field by field, TSV and JSON text) on its 18 cases and
     both environments; `check_failures.py`'s TED-04 row then compares full runs.
- **Relay 6 (2d): DONE** (7g. above). The plan it followed: the program's interfaces and package, in the same worktree:
  1. The `larmorx` registry line (`larmorx_cli::replica`: a `Package` entry `LARMORX_GPL` with
     `LARMORX_GPL_BIN`, like `larmorx-nc`'s; `Replica { family: "tedana", tool: "t2smap", location:
     Location::Program(&LARMORX_GPL), original: false }`), so `larmorx tedana t2smap <args>` (and
     `lx`) runs `larmorx-gpl tedana t2smap <args>` when installed and says how to install it
     otherwise; check the CLI program registry's tests.
  2. `lx.tedana.t2smap(echoes, echo_times, *, mask=None, ..., implementation="auto",
     upstream_defects="correct", n_threads=1)` in a new family `python/larmorx/tedana/`
     (`__init__.py`, `.pyi`; a typed result dataclass: the files' paths and the maps loaded on
     request, `implementation`, the notices), calling the `larmorx-gpl` program as the FSL
     wrappers call `larmorx-nc` (read how `lx.fsl.*` run the replica program: environment,
     `LARMORX_UPSTREAM_DEFECTS`, `LARMORX_TEDANA_LIBRARIES`, errors); `implementation="original"`
     raises until the clean-room original exists; tests in `tests/python/`.
  3. The `larmorx-gpl` wheel (binary-only maturin `bin` project in `crates-gpl/`, as
     `crates-nc`'s wheel: read its `pyproject.toml` and CI steps): licence files (GPL-3.0 text,
     tedana's LGPL-2.1 text, NOTICE with "Copyright 2023, The ME-ICA Developers" and the
     permissive notices of the Apache crates it includes: numpy, SciPy, OpenBLAS/LAPACK, CPython,
     nibabel, nilearn), the Rust source as `larmorx-nc`'s wheel carries it; `[all]` extra in the
     root `pyproject.toml` (shared: through the handoff if the orchestrator prefers);
     `crates-gpl/README.md` and `crates-gpl/NOTICE` (the GPL-3.0-only conversion); the glibc
     tools stay out of the wheel. D7: the golden cases run in CI's existing gpl job: estimate
     minutes x 6 jobs (rule 10) and write the step (handoff for `ci.yml` if shared).
  4. `docs/api/tedana-t2smap.md` (the option-mapping table: every t2smap option, the files, the
     D6 difference, the defects TED-01..15 with default/reproduce, `LARMORX_TEDANA_LIBRARIES`,
     threads; the default-mode notices), `check_spdx.py` (already allows GPL-3.0-only there).
  5. Decide 7f's (a) (GLIBC-07/NP notice in the default mode) with the orchestrator if it is
     more than a small addition.
  Then the gates for what it touched (the Python gates now apply: `gates-python.sh <worktree>
  --no-build` after a `maturin build` of the main wheel, `tables --check`).
- **Relay 7 (3), split in two (relay 7a's decision: too large for one relay):**
  - **7a: DONE** (7h. above): the maths notices, golden, the parity suite and its record, the
    memory test, the replica fixes the suite found.
  - **7b: DONE** (7i. above): the difftest descriptor, the trial and its triage (every
    difference fixed or registered: TED-16..18), the check of the final build, the findings doc,
    speed, the API page, the gates.
- **Relay 8 (4), split in two (relay 8a's decision):** 8a DONE (7j. above: the whole text and the
  probe); **8b: DONE** (7k. above; its plan, followed, below):
  - **8b's plan** (same worktree, the spec writer's role; environment `r7b/env.sh` with TMPDIR
    unset for Python gates; smart testing): (1) `validation/investigations/t2smap-spec/model.py`
    written from the spec's §§4-11 only (numpy, SciPy, nibabel, nilearn's compute_epi_mask is
    permissive and may be called; `numpy.linalg.lstsq` per echo-count group, `scipy.optimize.
    curve_fit` per voxel with §7.3's bounds and start, the floors, the cap, the RMSE, the table,
    the combination, the files with nibabel's new image from the mask's affine, the TSV with
    Python's repr, `json.dump(indent=4, sort_keys=True)`), run in `oracles/python-newest` with
    §17.1's environment, compared bit for bit with tedana's files of the probe's cases
    (`<scratch>/r8/bb-ted/<case>/out`; re-run blackbox.py if the scratch is gone) and on
    fMRIPrep's four curvefit calls and ds003592's loglin (tedana's outputs through the oracle
    cache, or the replica's `reproduce` outputs, which the record shows identical): every
    difference is a spec error, fixed in the spec; (2) a `--fast` variant (a per-voxel 2 x 2
    least squares, other sum orders in the RMSE, the table and the combination) to measure
    §17.3's margins; (3) `check.py` = §17.3 between two output directories (with the inputs, for
    F, the predictions and the weights), run on tedana's own spread (shared vs newest
    environment, `--n-threads 4`, AVX-512 on: the launchers with other env) on ds003592's call and
    a few probe cases, and on the model's `--fast` variant; (4) revise §17.3's numbers (never
    stricter than tedana's own spread; a margin of about 10x over the measured largest), drop
    "provisional", fill §18 (results) and §19 (how checked); README updated; ruff, SPDX; commit.
    Boxed (16 GiB for fMRIPrep's calls, 14 threads at most).
- **Relay 8 (4), the original plan** (same worktree; the spec writer's role):
  0. (Done by relay 7b: the record ddc275d2.) Environment as `r7b/env.sh` (and TMPDIR unset for
     the Python gates and the smoke tier).
  1. `specs/t2smap.md` (porting guide §1: behaviour, maths, conventions, command line, outputs,
     messages, failure cases, the D13 defects TED-01..18 with what the default and `reproduce` do,
     the validation plan with thresholds where exact results depend on an order (lstsq's batch,
     curve_fit's TRF path, numpy's sums), never code, routine names or line references; the
     published equations: Posse et al. 1999, Kundu et al. 2012/2017, DuPre et al. 2021, tedana's
     documentation; the black-box facts of `docs/findings/tedana-t2smap.md`'s last section; what
     nibabel, nilearn and pandas do with the files (as behaviour)). Large: split into 8a (command
     line, workflow, masks, fits, maps) and 8b (outputs, messages, defects, validation plan) if
     needed.
  2. A model under `validation/investigations/t2smap-spec/` written from the spec and black-box
     runs (Python, clean of tedana's code: numpy/scipy/nibabel calls as the spec states them),
     with a comparison against tedana on the record's synthetic cases and fMRIPrep's calls
     (thresholds of the spec), so the clean-room implementer may run it.
- **Relay 9 (5): DONE** (7l. above; its plan, followed:) the site page (clone in `<scratch>/site` from the site's `main` and
  `evidence-blobs` as they are when it starts; branch `t2smap-pages`; the no-reply identity first;
  `docs/figures.md` and the site's CLAUDE.md say how; smart testing). Plan:
  1. Read `docs/figures.md`, the site's CLAUDE.md and two recent tool pages of the same kind (a
     replica with a record and real-data calls: e.g. 3dvolreg's or PRELUDE's pages and their
     evidence modules under `validation/src/larmorx_validation/evidence/`) and copy their form.
  2. An evidence module `validation/src/larmorx_validation/evidence/tedana_t2smap.py` (from the
     record `docs/validation/tedana-t2smap-replica.json` and real-data runs: fMRIPrep's ds003592
     call (the echoes, tedana 26.0.3's T2* and S0 maps and combined series, the replica's in both
     modes: `reproduce` identical, the default's differences TED-05 (the full map capped) and the
     maths (NP-03/GLIBC-07) with their notices), fMRIPrep's own files (tedana 25.0.1) against
     both (the version step), and tedana's own spread (relay 8b's `measure.py` reports in
     `<scratch>/r8b/final2-fp/`: the shared environment, 4 threads, AVX-512, as a "the original's
     own variability" panel). Inputs come from the oracle runs' working directories
     (`oracles/fmriprep-25.2.6/<run>/work/.../bold_t2smap_wf/`); outputs to be made by the
     relay (boxed) into the evidence cache.
  3. The page in the site clone (the tool's page, its evidence data in `evidence-blobs`), the
     site's checks (build 0 problems, number and viewer checks, every view draws, inventory after
     >= before on every page, screenshots 1440 and 390, light and dark), commit in the clone.
  Split it (9a evidence module and data, 9b the page and checks) if it is large.
- **Relay 10 (6): DONE** (State above), the handoff and the part's final report (the brief's sub-task 6):
  (1) a trial merge of this branch with main as it is then, in a scratch worktree only (main has
  moved: f5e89ff4 golden + the shared numpy module, ce57fb5a field-map oracle runs, later merges;
  expected conflicts: `crates/larmorx-core`'s numpy module (main's version where the orchestrator's
  notes say so: part B's nanpercentile q = -0.0), `numpy_linalg.rs` (ORCHESTRATOR-STATE's MERGE NOTE:
  dbdsqr_upper), `evidence/__init__.py` and `summary.py` (other parts' pages), `triage.toml`
  (appends), the golden registry, NOTICE and its copies in crates-nc and crates-gpl), named in
  HANDOFF.md; (2) HANDOFF.toml (`apply_handoff.py --check`, with `--site <scratch>/site`, against
  that trial merge) and HANDOFF.md; (3) the part's agents' ids here, and the part's final report.
  HANDOFF items: upstream.tsv, licensing.md, the register's TED family and the
  NP- entries, tools.toml, golden.md, next-steps, NOTICE (7b), CLAUDE.md rule 1's list + audit
  `SOURCE_FINDINGS` + R07 paths (D9)), final report.

## 9. Decisions for the orchestrator (D1-D9; recommendation first)
- **D1 package**: the replica in `larmorx-gpl` (GPL-3.0; tedana-derived files GPL-3.0-only under
  LGPL-2.1 §3), names as in §3; the clean-room original later in `crates/larmorx-tedana`. The user to
  confirm: `larmorx-gpl` becomes a published wheel again (today "reserved for Workbench").
- **D2 numpy's float64 exp/log**: the reference is numpy's non-AVX-512 path = glibc 2.35's `exp`/`log`
  (`larmorx_core::math::{exp,log}_glibc`, permissive), with the oracles run with
  `NPY_DISABLE_CPU_FEATURES="AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"`
  on this machine (the analogue of `OPENBLAS_CORETYPE=Haswell`; fMRIPrep's run was on an AVX2 CPU);
  the AVX-512 kernels documented as a finding (NP-) with their measured effect.
- **D3 the field maps' `numpy_linalg`** (their branch): relay 2 needs it; cherry-pick its commits into
  this branch now (merge conflicts then trivial) or wait for that part's merge.
- **D4 versions**: the replica follows tedana 26.0.3 with numpy 2.5.4 / SciPy 1.18.1 (python-newest);
  the record shows per case the step to the shared environment (numpy 2.3.5 / SciPy 1.15.3) and to
  fMRIPrep's image (tedana 25.0.1); no "tedana 25" mode.
- **D5 input formats**: NIfTI-1/2 and ANALYZE (larmorx-io) plus MGH if available; other formats nibabel
  reads (AFNI BRIK, MINC, PAR/REC) refused with a clear message, a documented gap.
- **D6 messages**: tedana's log lines exact (with its module:function:line); Python warnings and tqdm
  bars in their form, paths and timings the replica's own (the suite masks them);
  dataset_description.json with tedana's keys, the host's node data, `Version` 26.0.3, and the Python
  fields describing the replica and the library versions it reproduces (a documented difference).
- **D7 CI**: the replica's golden cases need `crates-gpl` back in CI (a `cargo test -p larmorx-gpl-cli`
  step per job; estimate in relay 6, about 2-3 min × 13 jobs) or golden cases only through the Python
  wheel test; the orchestrator to choose.
- **D8 data**: only one fMRIPrep oracle run is multi-echo; fMRIPrep's call emulated on ds000210 and
  ds000258 (catalog) suffices for "about three runs", or new fMRIPrep oracle runs (about 1.5 h each).
- **D9 audit**: R07 today restricts only the clone `tedana/`; add `tags/tedana-*` and every installed
  `site-packages/tedana` (both environments, the image) at this part's merge; the similarity family
  "tedana".

## FROM THE ORCHESTRATOR (2026-11-08 12:12, after relay 1)
- Relay 1's agent id: ac7d861cd17e74f0d. Its relay plan (relays 2 to 9) is agreed.
- **D1 (the package): asked of the user.** Relays 2 and 3 (the permissive numerics, in the main package's Apache
  crates) do not depend on it; nothing GPL is written before the user's answer, which the orchestrator adds here.
- **D2: decided** (as `OPENBLAS_CORETYPE=Haswell` and GLIBC-07): the replica's `reproduce` follows numpy's non-AVX-512
  path, glibc 2.35's float64 `exp`/`log` (`larmorx_core::math::{exp,log}_glibc`), the oracles run with AVX-512 off
  (`NPY_DISABLE_CPU_FEATURES` as you propose, recorded in the record's header); the default follows the GLIBC-07
  convention of every replica (correctly rounded, GLIBC-07 named where a value differs). numpy's AVX-512 kernels: a
  finding (NP-, next ID) with their measured effect.
- **D3: decided:** wait for the field maps' part A, now in verification and to be published within about an hour; the
  orchestrator then tells you (a message, and here) and you merge main into this branch (`git merge main`, in your
  worktree only) before the `lstsq` work. Do the other numerics first.
- **D4: decided** as you propose (tedana 26.0.3 with numpy 2.5.4 / SciPy 1.18.1; the step to the shared environment
  and to fMRIPrep's tedana 25.0.1 per case; no "25" mode).
- **D5: decided** as you propose (NIfTI-1/2, ANALYZE, MGH where larmorx reads it; other formats a clear refusal and a
  documented gap, among the completeness parts' items).
- **D6: decided** as you propose; `dataset_description.json` names larmorx honestly where tedana names itself, a
  documented difference, the rest of its keys exact.
- **D7: decided:** the replica's golden cases run in CI's existing `gpl` job (steps beside crates-nc's, no new job or
  runner), its cost measured in relay 6 against rule 10's budget (estimate minutes x 6 jobs before adding).
- **D8: decided:** real fMRIPrep oracle runs (quality first; machine time only): the orchestrator runs fMRIPrep 25.2.6
  on ds000210 (sub-02) and ds000258 (one subject) with the existing runs' settings, boxed, after the current merge's
  verification; relay 6 uses their `t2smap` nodes (and the emulation only where a run cannot be made).
- **D9: decided:** the orchestrator adds `tags/tedana-*` and every installed `site-packages/tedana` to R07 at this part's
  merge (through your handoff's text), the similarity family "tedana".

## FROM THE ORCHESTRATOR (2026-11-08 12:41, after relay 2)
- Relay 2's agent id: a1e74b87182689d97. Its plan for relay 3 (`git merge main`, then numpy's `lstsq`) is agreed.
- D2's variable corrected as relay 2 measured: `NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX
  AVX512_CNL AVX512_ICL AVX512_SPR"` (both numpy releases).
- The field maps' part A (with `numpy_linalg`) was pushed to main at about 12:38 (5a50dfb6). Relay 3 starts only when its CI and
  Pages are green (the user's rule, 2026-11-08: never start what builds on a part before that part is verified and
  published); the orchestrator launches it then.

## FROM THE ORCHESTRATOR (2026-11-08 13:09, D1 answered)
- **D1 decided by the user:** "Follow your recommendation. We will have a clean room as well, that can go in the main
  package." The replica in `larmorx-gpl` (`crates-gpl/`) as §3 proposes: `crates-gpl/larmorx-gpl-tedana` (tedana-derived,
  GPL-3.0-only, converted from LGPL-2.1 under its section 3, with the modification notices and PROVENANCE.md),
  `crates-gpl/larmorx-gpl-cli` (the `larmorx-gpl` program), a binary-only `larmorx-gpl` wheel with its licence files and
  source, `[all]` installing it; the permissive numerics in the main package's Apache crates; the clean-room original
  later in `crates/larmorx-tedana` (main package, Apache-2.0). `docs/licensing.md` (tedana's row, the packages table,
  "A replica package is a program") through the handoff.

## FROM THE ORCHESTRATOR (2026-11-08 13:36, the user chose option A)
- **The user, after the orchestrator set out two options (A: this part's plan, the bit-exact replica, then the clean-room
  original; B: a spec writer and the clean-room original only): "Do option A then, remember that for now we are doing
  smart testing and not extensive one."** So this part continues as planned (§8, relays 3 to 10).
- **Smart testing, not extensive** (CLAUDE.md, "Every part, in full", testing depth; the user, 2026-10-28 and again
  today): every code path once (each option, data type, odd header and size), fMRIPrep's real calls on about three
  runs (ds003592 and the two new oracle runs, D8), the defects' edge cases, the standard and smoke tiers, one short
  random-testing trial (0.5 h). No full tier, no heavy or repeated random testing, no exhaustive sweeps beyond what an
  exactness proof needs (as relay 2's all-2^32 `logf` check, which is cheap): those wait for the final validation phase.
- Add to the record what tedana itself varies (a measurement, not a sweep): fMRIPrep's call and two synthetic cases
  under AVX-512 on and off, 1 and several threads, the shared and newest libraries, tedana 25.0.1 against 26.0.3; the
  page shows both the exact agreement on the reference setup and that spread.
- **Main now holds the field maps' part A** (verified, published, CI green: 5a50dfb6, cd9f2f99; main 99e9024a). Relay 3:
  `git merge main` in this worktree first, then numpy's `lstsq` on `larmorx_transform::numpy_linalg`.

## FROM THE ORCHESTRATOR (2026-11-08 13:38, D8's oracle runs)
- larmorx-testdata 52e371a (pushed) adds ds000258 sub-21262's echoes 1-3 and sidecars (big-endian int16, LPS,
  oblique; TE 12/28/44/60 ms with echo 4). The fMRIPrep 25.2.6 runs (orchestrator, boxed, 4 threads):
  `oracles/fmriprep-25.2.6/ds000210-sub-02-noreconall` (running since ~13:30: two 3-echo runs, rest and cuedSGT) and
  `ds000258-sub-21262-noreconall` (after it). Their `t2smap` nodes are relay 6's real calls with ds003592's.

## FROM THE ORCHESTRATOR (2026-11-08 14:16, after relay 3)
- Relay 3's agent id: af895e326a1e6d7a0. Its plan for relay 4 (SciPy's `curve_fit`: the trust-region fit with bounds,
  `approx_derivative`, SciPy's own SVD, the covariance, every exit status) is agreed; smart testing as before.
- Your relay 2 commits (`larmorx_core::numpy`, `logf_numpy`) are being brought into main as a shared module (cherry-picks
  8e491ace, d0b5b5ea; the field maps' part B builds on it). When you next `git merge main` (not needed now), the same
  changes merge cleanly; if git reports a conflict there, keep your branch's version and say so.
- The field maps' `svd3` signed-zero difference you found (3.12.0 path) goes to the field maps' part B (its relay 2
  touches `numpy_linalg`); do not change it here unless your own cases need it.
- (orchestrator, 14:51) Both new fMRIPrep runs are done (exit 0): `oracles/fmriprep-25.2.6/ds000210-sub-02-noreconall`
  (two 3-echo runs) and `ds000258-sub-21262-noreconall` (one 4-echo run, big-endian, LPS, oblique); their t2smap nodes are
  under `work/fmriprep_25_2_wf/sub_*/bold_*_wf/bold_native_wf/bold_t2smap_wf/t2smap_node`.

## FROM THE ORCHESTRATOR (2026-11-08 14:59, after relay 4)
- Relay 4's agent id: a5a8b57937a8b1b27. Its plan for relay 5 (5a, then 5b) is agreed: the replica program, now GPL
  (`crates-gpl/larmorx-gpl-tedana`, `larmorx-gpl-cli`; D1, the user's decision); the main package's Apache crates hold the
  numerics only.
- **NOTICE:** whenever the handoff changes the root `NOTICE`, `crates-nc/LICENSES/NOTICE-larmorx.txt` must be the same file
  (crates-nc's notices test; CI's gpl jobs failed on it on 2026-11-08): say so in HANDOFF.md, and `crates-gpl`'s wheel will
  need its own copy of the notices it carries (relay 6 of the plan).

## FROM THE ORCHESTRATOR (2026-11-08 15:48, after relay 5a)
- Relay 5a's agent id: a30aa31ad03a8cbb0. Its plan for relay 5b is agreed. CPython's (PSF-2.0) and numpy's printing in the
  main package: their notices in NOTICE through the handoff (and the NOTICE copy rule above).
- (orchestrator, 16:00) Library defect IDs: this part NP-03 to NP-04 and SCI-13 onwards (the field maps' part B has
  SCI-06 to SCI-12, NP-05 to NP-08). The field maps' part B fixes `nanpercentile`'s q = -0.0 in main's module; at this
  part's merge the shared module takes main's version.

## FROM THE ORCHESTRATOR (2026-11-08 16:34, after relay 5b)
- Relay 5b's agent id: ae98246030672c0a4. Its plan for relay 6 is agreed.
- **TED-05 (the full T2* map uncapped in 26.x):** rule A by default, as you chose (values to 239,794,244 ms are not a
  T2*; tedana 25.0.1, fMRIPrep's own, capped the full map, so the default also keeps fMRIPrep's output), **unless**
  tedana's history shows the change was a deliberate design pipelines rely on (rule C): relay 6 reads the changelog and
  the pull request that moved the cap to the limited map and records the finding with the decision in the findings doc.
- **The default mode's maths notices (§7f (a)):** as every replica: name GLIBC-07 where the correctly rounded
  `exp`/`log` change an output value, and the NP- defect of numpy's own kernel (`logf`) the same way; nothing where no
  written or printed value changes. Relay 6 or 7, with a case.
- (orchestrator, 16:57) A finding of the field maps' part B (relay 3): numpy 2.2.6 (fMRIPrep's image) sums a whole
  array in chunks of 8,192 elements, numpy 2.3 and later in one pass; any sum, mean or standard deviation over more than
  8,192 elements can differ. For this part's step to fMRIPrep's tedana 25.0.1 (numpy 2.2.6), name it as a known reason
  where it applies (`larmorx_core::numpy` has the old order since that relay, in the field maps' branch).

## FROM THE ORCHESTRATOR (2026-11-08 17:01, after relay 6)
- Relay 6's agent id: a16720ab7bb61dc5f. Its plan for relay 7 is agreed (the maths notices first, then the suite, the
  record, the memory call, the difftest trial, speed; smart testing). TED-05 rule A stands (tedana #1290 gives no reason).
- The CI addition (the gpl job's steps for the two GPL crates, about 1 min x 6 jobs per run, no new job): accepted under
  rule 10's budget; relay 7 re-measures it if the record changes the crates.
- NOTICE: at the merge the orchestrator keeps both copies (crates-nc's and crates-gpl's) equal to the root's; your test
  and crates-nc's catch a miss.

## FROM THE ORCHESTRATOR (2026-11-08 17:48, after relay 7a)
- Relay 7a's agent id: ad7066973cb2d2a09. Its plan for relay 7b is agreed.
- The default mode's extra passes for the notices (31.9 s against 11 s on ds003592): accepted for the replica (exact
  notices; still faster than tedana); say it on the API page and in the benchmark, and keep the passes to the families
  whose function results actually differ, as you designed. The clean-room original will be the fast one.

## FROM THE ORCHESTRATOR (2026-11-08 19:59, after relay 7b)
- Relay 7b's agent id: a255b30a7008412b5. Its plan for relay 8 (the spec and its model, 8a/8b if needed) is agreed.
- **The notices' passes: keep the exact per-family attribution (up to four passes)**, as recommended: the replica's role is
  exactness; the clean-room original is the fast one. The API page and the benchmark already say it.
- The three `test_every_program_node_is_classified[*-syn]` failures: the orchestrator's (its new oracle runs); it classifies
  the new runs' nodes in main once the PEPOLAR and direct runs have finished, verified and published before this part's
  merge. Do not touch the stage catalogue here.
- TMPDIR holding `-h` (the BET temporary-folder tests): noted as a follow-up for next-steps (portable tests); keep running
  the Python gates and the smoke tier with TMPDIR unset.

## FROM THE ORCHESTRATOR (2026-11-08 20:28, after relay 8a)
- Relay 8a's agent id: a7792b13ea745f138. The split (8b: the model, the thresholds' checker and measurements, the spec's
  revisions) is agreed.
- **Spec §19 point 1 (the fast original's maths notices): decided by the precedent of the user's decision of 2026-11-04**
  (GLIBC-07 in the FSL clean-room originals; and GLIBC-06, where the user declined an exact notice that made BET 1.9 times
  slower): no extra whole-run passes in the original. NP-03 and GLIBC-07 are named, in one pass, only where the function's
  own value is written as an output value (exact there: the value written differs); where it only feeds later arithmetic
  (the combination's weights, sums) or the iterative curve fit, the original names nothing, as FSL's iterative programs and
  filters. Write this rule into the spec (replacing the open point) and into the validation plan; the replica keeps its
  exact four-pass rule.
- **TQDM_DISABLE:** fix it in the replica (tedana prints no bars with `TQDM_DISABLE=1`; the replica must not either), with
  a case; small, in this relay or the next.
- The four restricted paths for the audit's tedana restrictions: relay 10's handoff, as written.

## FROM THE ORCHESTRATOR (2026-11-08 21:12, after relay 8b)
- Relay 8b's agent id: a5667128987e5c88e. Its plan for relay 9 (the site page) is agreed.

## FROM THE ORCHESTRATOR (2026-11-08 22:16, after relay 9)
- Relay 9's agent id: ad96fb3c5454f2d1f. Its plan for relay 10 (a trial merge with main in a scratch worktree, HANDOFF.toml
  and HANDOFF.md, the final report) is agreed. Main is now d34848c6 (CI green at b4ae4202): the stage catalogue's SyN entry,
  the crates-nc profiler test, the fMRIPrep oracle script (OpenBLAS's Haswell kernels in the container from now on, not yet
  pushed), the oracle-run docs. The site's main is still 01f901ed.
- **OpenBLAS's kernels in today's fMRIPrep runs (measured by the orchestrator):** on this machine fMRIPrep's image chooses
  OpenBLAS's AVX-512 kernels unless told otherwise; the earlier runs (an AVX2 laptop) had Haswell's. ds000210 and ds000258
  were made with AVX-512 kernels (as your page says for ds000258's 581 values). They STAY as they are for this part (its
  record and page are made from them and are correct as stated); they will be made again with Haswell's kernels before
  `t2smap`'s clean-room part, which then refreshes the replica record's real calls and the page's fMRIPrep panels. Nothing
  for relay 10 to change; the handoff need not mention it (the orchestrator writes it into next-steps).
