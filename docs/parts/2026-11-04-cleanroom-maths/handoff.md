# HANDOFF: fslmaths' and fslstats' clean-room originals (relays 1 to 7, 6b and 7b)

Branch `worktree-agent-a1677db426608d052` (worktree `<workspace>/larmorx/.claude/worktrees/agent-a1677db426608d052`),
from main d0c6e83; head da76711, clean. Site clone `<scratch>/site`, branch
`fsl-maths-cleanroom-page` at 52966cc (from the site's local main 779779e: the two pages, their
JSON and the blobs' manifest), blobs on its branch `fsl-maths-cleanroom-blobs` at 206fa20 (995
blobs: the 994 of `spec-answers-3-blobs` and one new, `619de7ce962508b9c1ff3012.bin`, the
original's `-kernel sphere 21 -fmean` output with relay 7b's double sums; relay 7's
`dae0c2dc45a1df53bec737c8.bin` no longer named). The fslstats page exported from larmorx afb60da
(clean; relay 7b changes nothing it shows), the fslmaths page re-exported by relay 7b from
da76711 (clean), its two recomputed figures-cache entries of the original (`original-families`,
`original-cases-default-mode`) from da76711, the others from f092d74 (relay 7's, untouched by
the change: no FFT path). The site's checks: build 0 problems (149 links into the worktree
checked); numbers 463 of 463 (fslmaths) and 88 of 88 (fslstats) agree with the JSON (before
relay 7: 146 and 51); viewers 152 checks, 0 failures (before relay 7: 61); the inventory
(`validation/investigations/cleanroom-maths/site_inventory.py`) after >= before on every count of
both pages (relay 7: e.g. fslmaths comparisons 7 -> 17, tables 9 -> 14, cases 421 -> 446, case
results 421 -> 863, numbers with a JSON pointer 147 -> 464; fslstats tables 4 -> 7, cases 208 ->
221; relay 7b against relay 7: every count equal, the fslmaths page's words 39,518 -> 39,725);
screenshots before and after in `<scratch>/site-check/` (`shots-elements/`: relay 7's, every
component at 1440 and 390, light and dark; `shots-7b/`: relay 7b's, the components its change
reaches; the full-page shots repeat the page's top beyond Chrome's 16,384 px, so the element shots
are the review set).

The exact changes to the shared documents are `<scratch>/HANDOFF.toml` (25 operations;
`python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <site> --check`
passes in the worktree and with the site clone; applied to copies, `validation/tools.toml` and
`validation/difftest/triage.toml` parse (21 tools, 45 records; 370 differences), and the
generated tables render from the new `tools.toml`, the site's status table parsing as YAML).
This file gives the reasons, what the orchestrator does at the merge, the follow-ups, every
relay's clean-room statement and the spec questions.

## At the merge (the orchestrator)

1. Apply `HANDOFF.toml` (with `--site`), then `python -m larmorx_validation tables` (the README's
   table, the validation index, the site's `data/status.yml`): the two tools' texts use
   `{fsl-maths.identical}` and `{fsl-stats.identical}`, metrics added in this branch
   (`tables.py`'s `_maths_original`: the passing cases whose every measured value shows no
   difference; 358 and 125). Relay 7b changed the original's Rust code (`maths::filters`, the
   sums where FSL takes its FFT) after the smoke tier of 479e9b3: the merge verification runs
   the smoke tier and `record_golden.py` again. No golden case moves: the main package's
   golden cases never reach FSL's FFT path (`tests/golden` 225 of 225 pass with relay 7b's
   build, without FSLDIR and `LARMORX_NC_BIN`).
2. The site: merge `fsl-maths-cleanroom-page`; its blobs are `fsl-maths-cleanroom-blobs` (one
   parentless commit, `tools/blobs.py commit`); re-export the summary (`evidence summary`):
   `evidence/summary.py` lists both implementations of `/tools/fsl-maths/` and `/tools/fsl-stats/`
   in this branch (a conflict with main's topup line is possible: keep both). The roadmap's three
   edits are in `HANDOFF.toml` (`site:content/roadmap.md`: row 1's status, the completed row, the
   date).
3. golden.md's table of counts (cases and outputs of `tests/golden/registry.py`) after the merge:
   this branch adds 9 cases and 224 outputs (223 cases, 1,583 outputs here).
4. `register_counts.py`: three new rows (FSL-191 to FSL-193, class c, c, b; the Gap column
   "replica": their replica follow-ups).
5. The re-runs the earlier relays asked for: `fsl-bet` and `fsl-flirt` (relay 5 changed BET's and
   FLIRT's originals' FSL-187 path, defaults unchanged; relay 5's slip opened `parity/fsl_bet.py`'s
   docstring); the replica suite's `fsltest/` cases against the original if wanted (outside the
   clean room).
6. CLAUDE.md "Where things are", `docs/status.md`, the decision log: the part done (2026-11-04).

## The handoff's operations, and why

- **`validation/tools.toml`** (blocks `fsl-maths`, `fsl-stats`; records inserted before each
  replica's): the originals' rows. Their numbers from the records `fsl-maths` (relay 7b's, from
  4182e89: 442 cases: 374 pass, 63 both-error, 5 expected, 0 fail; 358 identical) and
  `fsl-stats` (162: 126 pass, 36 both-error; 125 identical), the speed from the benchmark report
  (relay 7b re-measured its two FFT-path rows: 1.8 to 28 times FSL's speed on one thread, up to
  268 times on 14).
- **`docs/validation/golden.md`**: the nine cases of relay 6 (recorded by 6b from 479e9b3 with
  the smoke marker: 9 cases, 224 outputs; every family with a path that makes NaN from numbers,
  as the orchestrator asked after the replica's ARM NaN), and the replica paragraph's tense.
- **`docs/validation/difftest.md`**: the descriptors `fsl-maths-original` and `fsl-stats-original`
  (relay 6) and the trial's sentence in the oracle-cache list.
- **`validation/difftest/triage.toml`** (append, never opened by any relay): 192 `[[difference]]`
  entries, one per distinct signature of the trial (190 of fslmaths, 2 of fslstats), each with
  the categories of its findings and their counts (`validation/investigations/cleanroom-maths/
  triage_entries.py`, from `trial_triage.json`'s rules on relay 6's re-check with the final
  build). Status: the first of open, register, fixed, harness, explained among its findings'
  (5 open: the signatures holding the 10 open findings; 1 register: FSL-75; 47 fixed; 7 harness;
  132 explained). The trial's totals: 6,784 differing findings = 5,168 the original's mistakes
  fixed (21 permanent cases) + 58 chains those fixes reach + 170 the suite's mistakes + 1,338
  explained (fslmaths 1,178, fslstats 160) + 33 expected + 7 FSL-75 + 10 open.
- **The register** (`docs/findings/upstream-defects.md`, summary rows): FSL-191, FSL-192,
  FSL-193 (behaviour only, from black-box runs: findings 9, 14, 20 of
  `docs/findings/fsl-maths-original.md`); FSL-75's row (fslmaths' original names it in both
  modes, the record compares outputs where FSL ends by a signal); FSL-154, FSL-157, FSL-165 (the
  original's naming since random testing, findings 27 and 29); FSL-187 (BET's and FLIRT's
  originals' switch, relay 5, findings 22); GLIBC-07 (the original: correctly rounded in both
  modes; its `reproduce` lacks glibc's `exp`, `log`, `pow`: noticed in this relay, see below).
  The rows FSL-151 to FSL-189 otherwise hold for the original as they are (the same switches,
  checked by its record's d13 cases); the orchestrator may add "fslmaths' and fslstats'
  clean-room originals 2026-11-04" to their larmorx column as for topup's.
- **`docs/api/upstream-defects.md`**: FSL-191 to FSL-193's lines; FSL-187's line names the
  originals.
- **`docs/next-steps.md`**: item 7's paragraph for the originals; topup's Windows follow-up done
  (relay 6); the originals' item done and their follow-ups (below).
- **`docs/benchmarks/README.md`**: the row for `fsl-maths.md`.
- **The site's roadmap**: row 1's status, the completed row of 2026-11-04, "Last updated".

## Found in relay 7 (the site part)

- **Findings 31** (`docs/findings/fsl-maths-original.md`, `fft_reference.py`): on the fslmaths
  page's real call `-kernel sphere 21 -fmean` of ds005040's BOLD mean (a 15 × 15 × 13 kernel of
  1,313 voxels, FSL's FFT path), the original lies 1.39e-5 of FSL's largest value from FSL,
  beyond §21's bound of 1e-5 (the record's synthetic FFT cases stay within 2.6e-6). Against the
  exact result (float64, from §10.2's definition), FSL's FFT is within 1.5e-7, the original's
  single-precision direct sum (§10.2's arithmetic of FSL's direct path, FSL's bits there) within
  1.4e-5: the original is the less accurate. The exact result rounded once is 2.0e-7 from FSL's
  output (75.8 % within 4 ulp): a double accumulation where FSL takes its FFT would put the call
  inside the bound. Not changed in this relay (no Rust change: the records, smoke tier and golden
  would have to be re-made): a follow-up for the orchestrator to schedule. The page shows the
  call as an open difference with these numbers. The record's reason for its 5 FFT expected
  divergences ("the FFT's rounding is relative to the image's largest values") is half the story
  and should be revisited with the follow-up; the page's case explorer adds that note to them.
  **Done in relay 7b** (the orchestrator's item: a fix is not postponed): see "Relay 7b" below.
- **The original's `reproduce` lacks GLIBC-07's `exp`, `log`, `pow` bits** (specs/fslmaths.md
  §19 asks them): it computes CORE-MATH's correctly rounded values in both modes (the page's
  table: 110 of 136,400 `exp` and 15 `log` double values one unit from FSL's, within its record's
  one unit per function). glibc 2.35's `exp`, `log`, `pow` are Arm's Optimized Routines (MIT;
  the replica reproduces them): a follow-up (a port into `larmorx_core::math`). Written in the
  API page's "Not yet" and the register's GLIBC-07 row.
- **Findings 23's second item is gone**: fslstats' unmasked `-S` on the 63-volume series, which
  differed in its last digits at relay 5, is FSL's text in all 252 numbers with the final build
  (at 1 and 14 threads; compared with FSL's output of relay 5's benchmark, no new FSL run); the
  benchmark report's row and findings 23 corrected; spec question 27 needs no answer for the
  original. Which change of relay 6 brought it was not isolated.

## Relay 7b: the double sums where FSL takes its FFT (findings 31, done)

- **The change** (`crates/larmorx-fsl/src/maths/filters.rs`, `direct_volume`'s `wide` sum, used
  by `convolve_in_place` where §10.2's criterion sends FSL to its FFT, in both modes): each
  product in double precision (the value as the computing type gives it, a double image's as it
  is, times the kernel's value: exact for single-precision values), added to a double-precision
  sum in the kernel's order, rounded once to single precision. FSL's direct path keeps §10.2's
  single-precision sum, bit for bit. The same NaN rules (a NaN result recomputed by x86-64's),
  the same bits at any thread count (unit tests at 1 and 3 threads, a new test of the sum
  against the exact one), no full-size copy (one row of double sums per worker): the memory is
  unchanged (the benchmark: 42 and 188 MiB on one thread, as before).
- **The page's call** (`-kernel sphere 21 -fmean` of ds005040's BOLD mean): the original 2.0e-7
  of FSL's largest value from FSL (was 1.39e-5, beyond §21's 1e-5), 75.7 % of the voxels within
  4 units in the last place: inside the bound, below the share, as the record's FFT cases (an
  *expected divergence* on the page now, was *open*). Against the exact result: the original
  7.9e-8 (median 3.4e-9), FSL's FFT 1.5e-7 (median 1.6e-8): the original is the closer.
- **The record** `fsl-maths` (from 4182e89, clean, once; `compare_records.py` against relay
  6b's): 442 cases, 374 pass, 63 both-error, 5 expected, 0 fail; 358 identical (unchanged). Only
  the seven FFT-path cases moved: `conv/fft-sphere` (1.2e-6 -> 9.6e-8 of FSL's largest value,
  share 30.0 % -> 100 %) and `conv/fft-odd` (2.6e-6 -> 2.0e-7, 18.1 % -> 100 %) pass;
  `conv/fft-double` (1.4e-6 -> 1.1e-7, 0.5 % -> 30.0 %), `conv/fft-4d` (3.0e-7 -> 8.6e-8, 68.9 %
  -> 95.2 %) and `orig/fft-asym` (1.0e-6 -> 1.0e-7, 70.5 % -> 98.8 %) stay expected divergences
  of the share only (their reason rewritten: FSL's single-precision FFT; against the exact result
  FSL 6.9e-8 to 1.0e-7, the original 4.1e-8 to 6.3e-8, `fft_cases_reference.py`);
  `conv/fft-box` (9.7e-8 -> 4.9e-8) and `conv/fft-even` (2.4e-7 -> 8.1e-8) closer; `conv/fft-int`,
  `conv/fft-nan`, `orig/fft-nan-local` bit for bit before and after. No threshold changed (the two
  passing cases left the suite's list of expected divergences).
- **Speed** (`bench.py`, the original only, FSL's and the replica's runs of relay 5): `-kernel
  boxv 13 -fmeanu` on the series 2.98 -> 3.86 s on one thread, 0.43 -> 0.51 s on 14 (FSL 28.73
  s); `-kernel sphere 6 -fmean` on the T1w 5.41 -> 8.14 s, 0.64 -> 0.85 s (FSL 227.71 s): 1.3 and
  1.5 times slower, still 7.4 and 28 times faster than FSL on one thread; against FSL 2.8e-6 ->
  1.0e-7 and 8.4e-6 -> 1.9e-7 of its largest value. A variant converting each source row to
  double once per kernel row measured slower (4.43 and 8.62 s) and was reverted.
- **Golden**: no case moves (above). **Gates** at 4182e89: `gates-rust.sh all` OK (1,004 tests
  over the three workspaces), `gates-python.sh --no-build` OK (781 passed; 619 without FSLDIR),
  `tables --check` 0.
- **The site**: the fslmaths page re-exported from da76711: the families example's FFT call (its
  new output blob, verdict *expected divergence* with the exact result's numbers), the original's
  families table, the record's cases (two FFT cases now *open* within the thresholds, three
  reasons rewritten with their exact-result numbers), the speed rows; the page's prose (the lead,
  the record's counts, the FFT call, the speed, "Not yet").
- **Shared documents** (`HANDOFF.toml`): tools.toml's speed; next-steps' item 7 (374 within, 5
  expected, the double sums, the speed, findings 31's line) and its follow-ups (the first one
  done, the others renumbered); the benchmarks README row; the roadmap's two rows.

## Follow-ups (in next-steps through the handoff)

1. The original's `reproduce`: glibc's `exp`, `log`, `pow` (Arm's Optimized Routines, MIT).
2. Correctly rounded `erf` and `erfc` in `larmorx_core::math` (`-ztop` uses the `libm` crate's,
   within rule 5).
3. larmorx-io's parallel gzip writer: about 3 MiB per worker (two 256 KiB blocks and their
   outputs); 4D outputs at 32 threads exceed FSL's memory allowance (the memory test measures at
   one thread).
4. Runtime SIMD dispatch in `larmorx-fsl` (an audited `unsafe` module; 1.26 to 1.34 times on
   the large convolutions with an x86-64-v3 build).
5. The replica's `-bptf` is 2.9 times slower than FSL (5.26 s against 1.82).
6. The replica follow-ups FSL-191, FSL-192, FSL-193 (its default to correct, `reproduce` FSL's).
7. The replica suite's `fsltest/` cases against the original (outside the clean room), if wanted.
8. A spec-answers part for the questions below.

## Not done (what this kind of part owes)

- GLIBC-07's `exp`, `log`, `pow` in the original's `reproduce` (found in relay 7), and correctly
  rounded `erf`/`erfc` (findings 31's double sum, the other follow-up of relay 7, done in relay
  7b).
- 10 open trial differences (8 display ranges after `-range` in their last bits, e.g.
  `fsl-maths-trial-41014-original`, `-100080-original` (FSL -2147483648 against NaN); 2 `-roc`
  curve numbers 2.4e-7 and 5e-7 from FSL's, `fsl-maths-trial-54988-original`, `-76339-original`).
- The spec questions (below) and the unverified choices of relays 3 and 4 (NOTES).
- The goodvoxels calls with a real cortical ribbon; the full tier, the data panel, long random
  testing: the final validation phase.

## The clean-room statement of every relay (from NOTES.md, with relays 7's and 7b's)

### Relay 1
Opened (read): CLAUDE.md files (loaded), the brief; `specs/fslmaths.md` and `specs/fslstats.md`
whole; `specs/bet.md` §3 (lines 74-176) and §4.2 (lines 252-291), the sections fslmaths' spec
refers to; larmorx's own Apache code: `crates/larmorx-fsl` (lib.rs, cli/mod.rs, cli/common.rs,
cli/bet2.rs parts, cli/bet.rs lines 300-370, cli/options.rs strtod/atof, cli/topup.rs parts
(usage/refuse/main start, write_outputs), bet/image.rs, bet/params.rs (robust range), bet/mod.rs
(Vol, pooled), fast/rng.rs, flirt/mat.rs atoi), `crates/larmorx-mri/src/hmc/image.rs` and
`files.rs` (function lists), `crates/larmorx-core` (defects.rs API and FSL-151..159 rows,
math/mod.rs, element.rs, array.rs, affine.rs signatures, parallel.rs), `crates/larmorx-io`
nifti header/read signatures, `crates/larmorx-cli/src/replica.rs` and `lib.rs` (test),
`crates/larmorx-py/src/fsl.rs`; Python: `python/larmorx/fsl/maths.py`, `topup.py` (run path),
`_replica.py` (function list), `bet.py` (_threads), `image.py` (as_image); tests:
`tests/python/test_fsl_maths.py`, `tests/golden/registry.py` (replica-only part, topup's CLI
case), `tests/golden/test_golden.py` (two tests); validation: `parity/fsl_maths_replica.py`
(lines 1-900 and 2126-2206: inputs, Run, _prepare, _run_case, fmriprep and goodvoxels cases, the
case list, suite), `parity/fsl_topup.py` (header, run_original, fix_pass, cases, suite),
`parity/harness.py` (lines 40-140), `parity/defects.py`, `difftest/permanent.py` (attach,
SUITES), `__main__.py` (registry, parity arguments), `docs/validation/oracle-cache.md` (lines
1-30), `docs/validation/fsl-maths-replica.md` (lines 1-130: the summary and the both-error
table), `docs/api/fsl-maths.md` (lines 1-43, 120-230), `scripts/orchestration/env.sh`, `lxv.sh`,
`boxed.sh` (heads).
Ran: FSL 6.0.7.23's fslmaths only through the oracle cache in `use` mode (the suite: 145 served,
14 ran, all crashes; the probes `probe_thresholds.py` 23 runs and `show_fsl.py` 6 cases), all in
`boxed.sh` boxes (16 GiB suite, 4 GiB probes; never more than one box at a time); the
`larmorx-nc` replica was not run.
Used without opening: `fsl_maths_replica`'s imports from `fsl_flirt_replica`/
`fsl_mcflirt_replica` (`_environment`, `Completed`; printed `_environment({})`'s keys once:
FSLDIR, FSLOUTPUTTYPE=NIFTI_GZ, HOME, OPENBLAS_CORETYPE, PATH).
Not opened: FSL's source, crates-nc/, the replica crates, Armadillo/nlohmann/glibc sources, the
restricted findings, docs/parts/, other investigations folders, validation/probes/, triage.toml,
the replica authors' permanent cases, the oracle cache's files, replica/spec-writer git history.
Slips: none of restricted content. Listings to report: one `ls` of the main checkout's `target/`
(the build-cache seed the brief names) and a `cp -a` of it; `ls` of
`validation/src/larmorx_validation/parity/` (module names only, e.g. `fsl_flirt_replica.py`, not
opened) and of `tests/parity/` filtered on fsl/topup; one grep for "fsl-topup|fsl_topup" over
`validation/src/larmorx_validation/*.py` (the package's top-level modules: `__main__.py`,
`tables.py`) and `tests/parity/*.py` (it printed lines of `__main__.py`, `tables.py`,
`test_memory_peak.py`, `test_fsl_topup_replica.py`: suite names only); `ls` of `<workspace>/tags/`
(release folder names); `git log --oneline -1` at the start (the head commit's title only).
### Relay 2
Opened (read): CLAUDE.md (loaded), NOTES.md, BRIEF.md; `specs/fslmaths.md` §2.1-§2.4 (lines
67-166), §3-§5 (306-408), §9-§10 (515-661), §17-§21 (834-998); larmorx's own Apache code:
`crates/larmorx-fsl/src/maths/{mod,num,ops,image}.rs`, `cli/fslmaths.rs` (whole),
`cli/fslmaths_usage.txt` (grep of the filter lines), `cli/common.rs` (abort, TRUNCATED: grep),
`bet/image.rs` lines 124-239 (Output, output_header, write), `crates/larmorx-io/src/nifti/header.rs`
(field and fn lines by grep; `new` lines 205-240), `crates/larmorx-core/src/math/mod.rs` (the
list of functions), `crates/larmorx-core/src/defects.rs` (FSL-160..172 and 180..189 rows, the
Notices API by grep), `crates/larmorx-fsl/Cargo.toml`, the root `Cargo.toml` (grep "fft");
Python: `python/larmorx/fsl/maths.py` (the function list; lines 517-706); tests:
`tests/python/test_fsl_maths.py` (helpers, test_without_the_replica); validation:
`parity/fsl_maths_replica.py` lines 62-125, 128-299 (inputs), 436-481 (Run, _prepare), 850-865,
1225-1622 (the case lists of sub-task 2), 2126-2175 (cases, run_case), plus greps inside it for
case names, input names, `def `/class lines and imports; `parity/fsl_maths.py` (whole, ours);
`parity/defects.py` lines 30-56, 104-140; `parity/harness.py` (grep EXPECTED/statuses);
`docs/findings/fsl-maths-original.md`, `docs/api/fsl-maths.md` (lines 14-30, 186-237),
`crates/larmorx-fsl/PROVENANCE.md` (lines 266-292), `validation/investigations/cleanroom-maths/`
(README, show_fsl.py).
Ran: FSL 6.0.7.23's fslmaths only through the oracle cache in `use` mode, in `boxed.sh` boxes
(16 GiB suites, 4 GiB probes, one box at a time): `show_fsl.py` on 26 replica cases (served);
`fsl_outputs.py` on 6 replica cases (served) and 13 ad-hoc command lines (new runs: the impulse,
ramp and asymmetric-kernel probes); the suite fsl-maths 7 times (r2a-r2f: sub-task 2's regex
four times, relay 1's regex once, orig/ once, the whole standard tier twice, the first of
which crashed on a recursion in my `_prepare` before any FSL run). The `larmorx-nc` replica was
not run. Numpy analyses of FSL's outputs in `<scratch>/py/` (gauss_fit, fit2-fit9).
Used without opening: `fsl_maths_replica`'s `_save`, `_affine`, `workspace`, `_r`, `oracle_run`
(called, read above).
Not opened: FSL's source, crates-nc/, the replica crates, Armadillo/nlohmann/glibc/Kiss FFT
sources, the restricted findings, docs/parts/, other investigations folders (spec-answers-3
included), validation/probes/, triage.toml, the replica authors' permanent cases, the oracle
cache's files, replica/spec-writer git history (only `git log --oneline` of this branch's own
commits and `git status`).
Slips: one. To learn whether an FFT crate is a dependency, one `grep -n fft Cargo.toml
crates/*/Cargo.toml` also read `crates/larmorx-afni-replica/Cargo.toml` (a replica crate's
manifest); it printed one comment line of it ("Exposes private routines to the bit-exactness
harnesses (tools/csfft-verify and"), nothing else; no replica code. Otherwise no listing or
search over the worktree, docs/ or validation/ as a whole: greps were inside named files of
our own crates (`crates/larmorx-fsl/src`, `crates/larmorx-mri/src` for one message text) and of
the reading list; `ls` of `crates/larmorx-fsl/src/`, its `maths/`, `cli/`, `topup/` (our own
crate), of `validation/investigations/cleanroom-maths/` (ours) and of my scratch.
### Relay 3
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md`
§2.1-§2.3 (67-145), §8 (483-514), §10.2 (580-614), §11-§21 (662-998); the register's summary rows
FSL-162, FSL-167..179 (a `sed` of lines 52-432 filtered on those IDs); larmorx's own Apache code:
`crates/larmorx-fsl/src/maths/{mod,num,ops,image,kernel,filters,spatial}.rs` (function lists, num
1-270, ops 20-165 and 765-930, image 30-68 and 240-414, filters 1-56, 362-375, 540-1015, spatial
480-560), `cli/fslmaths.rs` (whole), `flirt/schedule.rs` 270-330 (`std_sort_checked`), the
`Cargo.toml` of larmorx-fsl, the root `Cargo.toml` (grep libm), `crates/larmorx-core/src/math/mod.rs`
(34-165), `defects.rs` (FSL-191's rows, `macro_rules` and `contains` lines by grep),
`crates/larmorx-fsl/PROVENANCE.md` (280-300); Python: `python/larmorx/fsl/maths.py` (the function
list, 707-800); tests: `tests/python/test_fsl_maths.py` (imports, 132-185); validation:
`parity/fsl_maths.py` (whole, ours), `parity/fsl_topup.py` 90-175 and 920-945 (the
expected-divergence model), `parity/harness.py` (EXPECTED lines by grep, 160-215),
`parity/defects.py` 104-150, `parity/fsl_maths_replica.py` (Run and `_r`: 439-475, 852-861; input
lines 278-348 by grep; the case lists 1510-1540 and 1625-2125), `docs/findings/fsl-maths-original.md`
(whole, ours), `docs/api/fsl-maths.md` (14-32, 215-250), `validation/investigations/cleanroom-maths/`
(ours).
Greps: only inside files and folders of the reading list: `crates/larmorx-fsl/src`,
`crates/larmorx-mri/src`, `crates/larmorx-core/src`, `crates/larmorx-afni/src` (clean-room, for an
existing sort and `erf`), `python/larmorx`, `tests/python`, `tests/golden`, `crates/larmorx-cli`,
`crates/larmorx-py` (for FSL-191), and the named files above.
Ran: FSL 6.0.7.23's fslmaths only through the oracle cache in `use` mode, inside `boxed.sh`
(16 GiB suites, 4 GiB probes, one box at a time): the suite fsl-maths 9 times (r3a, r3f, r3i the
whole standard tier; r3b-r3e, r3g, r3h subsets); `fsl_outputs.py` on 26 replica cases (served or
crashes) and 12 ad-hoc command lines (new: the orientation impulses, the double `-bptf` series);
`show_fsl.py` on 18 replica cases. The `larmorx-nc` replica was not run. Numpy fits in
`<scratch>/py/` (orient, fit_bptf, fit_lp, fit_both, fit_both2, fit_tfce, cmp_case, cmp_args) and
the committed `fit_orientation.py`, `fit_bptf.py`, `fit_tfce.py`.
Not opened: FSL's source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica (no manifest
either), Armadillo/nlohmann/glibc/LAPACK/Cephes sources, the restricted findings, docs/parts/,
other investigations folders (spec-answers-3 included), validation/probes/, triage.toml, the
replica authors' permanent cases, the oracle cache's files, replica/spec-writer git history (only
this branch's own `git log --oneline`, `git status`, `git diff` of my own changes).
Slips: none. Listings: `ls` of my scratch, of `validation/investigations/cleanroom-maths/` (ours)
and of FSL probe folders in my scratch.
### Relay 4
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md` §15,
§18, §19.2-§19.3, §21 (785-998 in parts), `specs/fslstats.md` whole; the register's summary rows
FSL-179..189 (a `sed` of lines 52-432 filtered on those IDs); larmorx's own Apache code:
`crates/larmorx-fsl/src/cli/fslmaths.rs` (whole), `cli/fmt.rs`, `cli/common.rs` (50-107, function
list), `cli/mod.rs` (registry), `cli/options.rs` (atof, strtod lines), `cli/bet.rs` (300-470),
`maths/num.rs` (1-100), `ops.rs` (20-66, 130-165, 470-726), `tfce.rs` (1-80), `image.rs` (63-100,
179-240), `mod.rs`, `reduce.rs` (60-100), `filters.rs` (abort_lines, refuse_empty), the signature
of `flirt::schedule::std_sort_checked` (grep), `crates/larmorx-core/src/defects.rs` (the macro,
FSL-179..192 rows, Notices), `math/mod.rs` (function list, 56-125), `crates/larmorx-cli/src/
replica.rs` (255-290, 715-735), `lib.rs` (35-70, 405-470), `crates/larmorx-py/src/fsl.rs`
(fsl_run_original), the nifti header's pixdim/sform/qform lines (grep); Python:
`python/larmorx/fsl/maths.py` (roc, _run), `stats.py` (1-160), `_replica.py` (select); tests:
`tests/python/test_fsl_maths.py` (parts); validation: `parity/fsl_maths.py` (ours),
`parity/fsl_maths_replica.py` (355-405 inputs, 2047-2125 roc cases, `_r`, `_save`, `_affine`),
`parity/fsl_stats_replica.py` (1-282, 345-420, 628-640 cut at 60 columns, 640-783; never the body
of `_fsl_test_cases`: only the line numbers of its reads, printed by awk), `parity/defects.py`
(50-56, 104-150), `__main__.py` (registry by grep); `docs/api/fsl-maths.md` (14-40, 215-260),
`docs/api/fsl-stats.md` (10-37, 120-155), `docs/findings/fsl-maths-original.md` (ours),
`crates/larmorx-fsl/PROVENANCE.md` (266-310), `validation/investigations/cleanroom-maths/` (ours).
Ran: FSL 6.0.7.23's fslmaths, fslstats, bet2 and flirt only through the oracle cache in `use`
mode, inside `boxed.sh` (16 GiB suites, 4 GiB probes, one box at a time): `fsl_outputs.py` on the
13 replica roc cases (served) and 8 ad-hoc probes (rocA-rocH, new); the suite fsl-maths twice
(r4a roc subset, r4b whole tier); `show_fslstats.py` on about 75 replica synthetic cases (served)
and about 14 ad-hoc probes (new); the suite fsl-stats 4 times (s4a-s4c all, s4d orig/);
`probe_f187.py` twice (FSL's bet2 once, flirt 6 times: new). BET's and FLIRT's clean-room
originals ran as black boxes (larmorx's own binary). The `larmorx-nc` replica was not run.
Numpy fits in `<scratch>/py/` (fit_roc, exp_check, sd_check, tp_check, cog_x) and the committed
`fit_roc.py`.
Used without opening: `fsl_stats_replica`'s imports from `fsl_flirt_replica`/`fsl_mcflirt_replica`
(`_environment`, `fsldir`, `Completed`, `workspace`) through its namespace; FSL's
`etc/flirtsch/measurecost1.sch`, passed to flirt by path.
Not opened: FSL's source (its fslstats tests included: the original's suite lists the replica's
cases with `_fsl_test_cases` replaced by an empty dict, so FSL's test script is never read),
crates-nc/, crates-gpl/, crates/larmorx-afni-replica, Armadillo/nlohmann/glibc/Cephes/Kiss FFT
sources, the restricted findings, docs/parts/, other investigations folders (spec-answers-3
included), validation/probes/, triage.toml, the replica authors' permanent cases, the oracle
cache's files, replica/spec-writer git history (only this branch's own `git log -1`, `git status`).
Slips: none that showed restricted content. To report: one `ls ~/fsl-6.0.7.23/etc/flirtsch/ |
grep -i measure` (a file name of FSL's installed schedules, not its source: `measurecost1.sch`);
my script printed the replica suite's synthetic case list (names and arguments, from a file on the
reading list); `awk` printed the line numbers (no content) of `_fsl_test_cases`'s file reads.
### Relay 5
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; the register's summary row
FSL-187 (a `sed` of lines 52-432 filtered on the ID); `specs/fslmaths.md` §20-§21 (921-998),
`specs/fslstats.md` §5.2 (211-238), §9 and §10's speed lines (352-392 by grep); `docs/porting-guide.md`
404-460 (the clean-room speed section, §9 determinism); `docs/validation/oracle-cache.md` 140-150;
larmorx's own Apache code: `crates/larmorx-fsl/src/bet/{params.rs (whole), mod.rs (80-130,
200-225, tests), surface.rs (18-75)}`, `cli/bet2.rs` (395-440, 495-660, 766-808, tests),
`flirt/engine.rs` (imports, 125-175, 690-790), `flirt/cost.rs` (grep for bins), `maths/{filters,
regions, reduce, image, num}.rs` (the parts I changed), `maths/kernel.rs` (86-120),
`cli/fslmaths.rs` (200-520), `cli/fslstats.rs` (145-215, 525-550, 1155-1200), `examples/tanh_effect.rs`,
`crates/larmorx-mri/src/hmc/cost.rs` (860-940: the MI bins, clean-room), `crates/larmorx-core/src/
{defects.rs (FSL-183..189 rows, the Notice API), math/mod.rs (60-200: the FMA dispatch), lib.rs
(module list), array.rs (1-30), element.rs (DataType)}`, `crates/larmorx-io/src/{nifti/mod.rs
(503-509, 563-620), nifti/header.rs (205-240), gzip.rs (1-200)}`, `crates/larmorx-cli/src/replica.rs`
(12-30, grep), `python/larmorx/_replica.py` (10-30, grep); validation: `parity/fsl_maths.py`
(740-830, ours), `parity/fsl_maths_replica.py` (588-820: fMRIPrep's calls and the goodvoxels chain;
the function list), `parity/fsl_stats_replica.py` (684-691: BET's image), `__main__.py` (the suite
registry, 40-95), `tests/parity/test_memory_peak.py` (grep for threads); ndarray 0.17.2's
`into_raw_vec_and_offset` signature (a grep in the cargo registry: a third-party MIT/Apache crate).
Ran: FSL 6.0.7.23's fslmaths, fslstats, bet2 and flirt through the oracle cache in `use` mode, in
`boxed.sh` boxes (never more than 24 GiB at once): probe_f187.py again (served), probe_gauss.py
(3 new fslmaths impulse runs, then served), the suites fsl-maths three times (r5a whole, r5b subset,
r5c whole) and fsl-stats twice (s5a, s5b), every FSL run served but crashes (never stored).
FSL timed directly for the benchmarks (the cache's documented exception): bench0 (14 cases once:
the stats-series case with a wrong argument order, so its two fslstats cases ran once more in
bench2). The `larmorx-nc` replica as a black box through larmorx's dispatch (bench2, 14 cases once),
built in my scratch (`cargo build --release -q --locked --manifest-path crates-nc/Cargo.toml
--target-dir <scratch>/nc-target`: its sources compiled, never opened; its build log held only the
exit line). An x86-64-v3 build of larmorx-cli in my scratch for one timing comparison.
Not opened: FSL's source, crates-nc/ (no file, no manifest), crates-gpl/, crates/larmorx-afni-replica,
Armadillo/nlohmann/glibc/Kiss FFT sources (glibc's `expf`/`exp` only called through ctypes, as a
black box), the restricted findings, docs/parts/, other investigations folders (spec-answers-3
included), validation/probes/, triage.toml, the replica authors' permanent cases, the oracle
cache's files, replica/spec-writer git history (only this branch's own `git log -1`, `git status`,
`git diff --stat` of my file).
Slips: one. For the orchestrator's "one suite case each in fsl-bet and fsl-flirt", I opened the
first 40 lines (the module docstring) of `validation/src/larmorx_validation/parity/fsl_bet.py`
and stopped reading when it said it is the BET replicas' parity suite (it showed only mechanics:
how FSL and the replica run, what is compared, the case groups and "the defects (FSL-40 to -56)");
nothing else of it, and not `fsl_flirt.py`. Greps only inside named files and our own crates
(`crates/larmorx-{fsl,core,mri,io,image,interp,afni,cli,py}/src` for call sites and the dispatch
pattern). Listings: `ls` of my scratch and of our own crate's `src/{bet,flirt,maths,cli}` and
`examples/`; one `test -x` of `crates-nc/target/release/larmorx-nc`
(a single path, absent).
### Relay 6
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md` §10.4
to §10.7, §15, §21 (lines by grep and sed); the register's summary rows FSL-75 and FSL-151 to
FSL-189 (a `sed` of lines 52-432 filtered on those IDs); larmorx's own Apache code:
`crates/larmorx-fsl/src/maths/{kernel, filters, spatial, image, reduce, tfce, stats, roc, num}.rs`
(the parts changed), `cli/{fslmaths, fslstats, topup, applytopup, common}.rs` (parts),
`bet/quatern.rs` (whole), `flirt/pyramid.rs` (300-460: the FSL-75 shell count),
`crates/larmorx-io/src/nifti/header.rs` (`qform`), `crates/larmorx-mri/src/hmc/image.rs`
(`is_neurological`), `crates/larmorx-cli/src/replica.rs` (grep); Python: `python/larmorx/fsl/
{maths, stats}.py` (signatures, constants); tests: `tests/golden/registry.py` (inputs, writers,
helpers, topup's cases, the replica-only list), `test_golden.py`, `tests/parity/test_memory_peak.py`
(1-80, 526-720), `test_thread_pools.py`; validation: `parity/fsl_maths.py`, `fsl_stats.py` (ours),
`parity/fsl_maths_replica.py` (20-58: docstring, imports; greps for `_SILENT`, zst), `parity/
thread_pools.py` (100-175, 300-460), `parity/defects.py` (grep), `difftest/core.py` (greps,
185-240), `difftest/permanent.py`, `difftest/tools/{__init__, fsl_avwutils (1-80, 600-725),
fsl_topup_original}.py`, the evidence modules' triage lines (grep), `validation/investigations/
cleanroom-topup/` (its listing, `trial_triage.json`, `triage_counts.py` 1-40),
`docs/validation/difftest.md`, `docs/validation/fsl-topup.md` (head), `docs/api/fsl-topup.md`
(136-146), our findings, benchmark report, PROVENANCE and README; the replica trial's `run.json`
(`oracles/difftest/2026-11-03-fsl-maths-replica/run.json`, its head: meta only).
Ran: FSL 6.0.7.23's fslmaths and fslstats through the oracle cache in `use` mode, in `boxed.sh`
boxes (never more than 24 GiB of mine at once): probe_gauss.py (one new 3D run), probe_geometry.py
(about 165 new runs), probe_kernel_file.py (84 new), compare_call.py (about 15 new),
show_finding.py (about 60, served or new), fsl_outputs.py (2 served), the trial (300,000 cases;
the fslmaths stream's runs past the replica trial's cases new, stored), four re-checks (findings'
shrunk recipes, served or stored), the suites (served). The larmorx-nc replica as a black box on
one case (`fslmaths <3x4x3 image> -subsamp2`: its FSL-75 notice text seen, which named the
register's FSL-75). FSL's own `zstd` program (`~/fsl-6.0.7.23/bin/zstd -dc`, a black box) to read
zstd outputs in the suite and by hand.
Not opened: FSL's source, crates-nc/ (no file, no manifest; only the built binary run),
crates-gpl/, crates/larmorx-afni-replica, Armadillo/nlohmann/glibc/Cephes/NIfTI library sources,
the restricted findings, docs/parts/, other investigations folders (spec-answers-3 included;
cleanroom-topup is on the list), validation/probes/, triage.toml, the replica authors' permanent
cases, the oracle cache's files, `parity/fsl_bet.py` (the memory test calls its `_head` and
`_save` through `_avw_inputs`, unread), replica/spec-writer git history.
Slips: three **direct FSL runs outside the oracle cache** (`~/fsl-6.0.7.23/bin/fslmaths` on a
finding's char image while bisecting the subsampling: `-Zmean -odt float` failed on its
arguments, `-dt input ... -Zmean -subsamp2`, `-dt input ... -Zmean`; FSLOUTPUTTYPE set; outputs
in my scratch); every other FSL run through the cache. One listing `ls validation/difftest/cases/
| grep original` (the permanent cases' folder names, filtered to the originals'); single-path
tests of `~/fsl-6.0.7.23/bin/zstd`.
### Relay 6b
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md (its headings and the
lines of sub-tasks 6 and 7, the reading list, the rules); `specs/fslmaths.md` §21 (947-1000) and a
grep of it for TFCE and FSL-75; the register's summary row FSL-75 (a `sed` of lines 52-432
filtered on the ID); our own validation code: `parity/fsl_maths.py` (1-130, 258-300, 437-670,
876-948), `parity/fsl_stats.py` (130-135, 540-579), `parity/fsl_maths_replica.py` (`_affine`
122-135; the `odd3` and `-s` case lines by grep), `parity/harness.py` (grep for `notes`),
`parity/render.py` (lines 129-130, printed by the grep below); `docs/validation/fsl-topup.md`
(1-40), `docs/validation/oracle-cache.md` (8-26), `docs/api/fsl-maths.md`, `fsl-stats.md`,
`fsl-topup.md` (its status paragraphs), `docs/findings/fsl-maths-original.md` (13, 24-30),
`docs/benchmarks/fsl-maths.md` (grep); `scripts/orchestration/{gates-rust.sh, gates-python.sh,
boxed.sh, parity_smoke.py (docstring)}`, `scripts/record_golden.py` (docstring),
`tests/golden/test_golden.py` (30-97); `validation/investigations/cleanroom-maths/` (ours: its
listing, `trial_triage.json`, `triage_counts.py`, README by grep); my scratch's `recheck4.jsonl`;
the records and golden.json I made.
Ran: FSL 6.0.7.23's fslmaths and fslstats only through the oracle cache in `use` mode, in
`boxed.sh` boxes (never more than 28 GiB of mine at once, one box at a time): the records
fsl-maths (396 served, 41 ran: FSL's crashes, never stored) and fsl-stats (157 served, 5
crashes), the smoke tier once (every suite's FSL runs through the cache), record_golden once
(the marker reused). The wheel (maturin build into my scratch, `_core.abi3.so` copied); the
gates, whose cargo builds and tests compiled crates-nc and crates-gpl (their sources and
manifests never opened; of the gates' log only the summary lines and `test result` counts read).
The larmorx-nc replica was not run by me (the smoke tier ran its suites).
Not opened: FSL's source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica (no file, no
manifest), Armadillo/nlohmann/glibc sources, the restricted findings, docs/parts/, other
investigations folders (spec-answers-3 included), validation/probes/, triage.toml, the replica
authors' permanent cases, the oracle cache's entries, replica/spec-writer git history (only this
branch's own `git log --oneline`, `git status`, `git diff --stat`, and `git show HEAD:` of
golden.json to compare my recording).
Slips: one search wider than the reading list: `grep -n '\.notes' validation/src/
larmorx_validation/parity/*.py validation/src/larmorx_validation/*.py | grep -v 'fsl_\|afni\|ants'`
(to find how a suite's notes reach the record) read every parity module, the replica suites
among them; the lines of `fsl_*`, `afni*` and `ants*` modules were filtered out before printing,
and it printed only render.py's two lines. Listings: `ls validation/difftest/cases/ | grep -c
original` (printed a count, 4), `ls` of our own `cases/fsl-maths-original/` and
`fsl-stats-original/`, and `ls` of the oracle cache's `markers/` folder filtered on my commit (it
printed my smoke marker's file name, which the smoke log had printed).


### Relay 7
Opened (read): CLAUDE.md (the worktree's rules; the harness also showed the main checkout's
CLAUDE.md as loaded context, which I did not open), NOTES.md, BRIEF.md; `docs/figures.md`; the
evidence modules `evidence/fsl_maths.py`, `fsl_stats.py` (whole) and `fsl_topup.py` (1-120,
366-515, 545-705, 954-1263, 1700-2127: the clean-room panels' model), `evidence/common.py`,
`core.py`, `example.py`, `summary.py`, `__init__.py` (the parts used), `figures/cache.py` and
`figures/core.py` (the entry, run, provenance and commit functions); our own suites
`parity/fsl_maths.py` and `parity/fsl_stats.py` (parts), `parity/harness.py` (the `Check`
fields), `parity/defects.py` (its function list), `difftest/permanent.py` (25-155);
`validation/src/larmorx_validation/tables.py` (parts); `scripts/orchestration/README.md`,
`apply_handoff.py`, `git-hooks/install.sh`; our records `docs/validation/fsl-maths.md` (1-240) and
both records' JSON (their measured values, by script), `docs/benchmarks/fsl-maths.md`,
`docs/findings/fsl-maths-original.md` (head, 8, 9, 23, 30), `docs/api/fsl-maths.md`,
`fsl-stats.md` (parts), `docs/api/upstream-defects.md` (its FSL-75, FSL-144 to 146 and FSL-187
to 189 lines by grep); the register's summary rows (a `sed` of lines 52-432 filtered on FSL-75,
FSL-143 to FSL-146, FSL-154, FSL-157, FSL-165, FSL-187, FSL-189, GLIBC-07, the table's header);
the shared documents the handoff edits, by their parts (`validation/tools.toml`,
`docs/validation/golden.md`, `docs/validation/difftest.md`, `docs/next-steps.md`,
`docs/benchmarks/README.md`'s topup row, by script); `tests/golden/registry.py` (4940-5200: this
part's golden cases); `validation/investigations/cleanroom-maths/` (ours); the trial's own
`summary.json` and findings (by script) and my scratch's `recheck4.jsonl` and relay 5's benchmark
outputs; in the site clone: its `CLAUDE.md`, `README.md`, `tools/blobs.py`, `build.py` (greps),
`evidence.py` (the metrics table), `tools/review/shoot.mjs` (parts), `data/verdicts.yml`, the
pages `content/tools/fsl-maths.md`, `fsl-stats.md`, `fsl-topup.md` and `content/roadmap.md`
(1-51).
Ran: **no FSL run** (FSL's outputs are the replicas' figures-cache entries, made by the replica
side through the oracle cache, and relay 5's benchmark output of `fslstats -t <BOLD> -M -S -r`);
the original (the release binary at 13a1356's Rust code) for its evidence entries, first in a
test cache in my scratch, then in the figures cache from the clean commit f092d74 (each: 213 runs
of the real calls, the default mode on the 442 and 162 cases of the records at one thread), and
on the series' statistics at 1 and 14 threads; the exports of `fsl-maths` and `fsl-stats` (a
test export from a dirty tree, the final ones from the clean commit afb60da); the site's build,
number and viewer checks and screenshots (headless Chrome, 1440 and 390, light and dark, before
and after, served from copies of the two builds); every job in `boxed.sh` boxes (at most 24 GiB of
mine at once, one at a time). The larmorx-nc replica was not run.
Not opened: FSL's source, crates-nc/, crates-gpl/, crates/larmorx-afni-replica (no file, no
manifest), Armadillo/nlohmann/glibc sources, the restricted findings, docs/parts/, other
investigations folders (spec-answers-3 included), validation/probes/,
`validation/difftest/triage.toml` (the handoff appends to it; applied to a copy whose parse was
checked by count only), the replica authors' permanent cases, the oracle cache's files.
Slips and what they showed:
- One `git log --oneline -1 -- docs/validation/fsl-maths-replica.json`, to check that the site's
  replica evidence came from the same record: it printed the title of the replica side's commit
  that re-made that record (a validation record's history, no code or source).
- To learn `triage.toml`'s field names, one grep for "triage" over the difftest harness's
  top-level modules (`validation/src/larmorx_validation/difftest/*.py`, larmorx's own Apache
  tooling, not on the reading list; it printed nothing) and the reading of
  `evidence/difftest.py` 1-20 and 86-126 (the module that reads the triage file: its fields
  `tool`, `implementation`, `signature`, `status`, `explanation`, `link`, `defect`); no restricted
  content.
- While checking the register's dagger convention, the first 120 characters of the rows FSL-100
  to FSL-104 (FAST's, within lines 52-432: the reading list's range, rows I did not need).
- The fsl-stats export runs the replica's evidence code, whose case list (the replica suite's
  full tier) reads FSL's own fslstats test list inside the export's process, as it always did
  for the replica's case explorer; nothing of it was printed or viewed (the JSON checked by
  counts and by the original's rows only; the fsl-stats screenshots viewed outside the case
  explorer).

### Relay 7b
Opened (read): CLAUDE.md (the worktree's; the harness also showed the main checkout's CLAUDE.md
as loaded context, which I did not open), NOTES.md, BRIEF.md; `specs/fslmaths.md` §10.2, §18,
§20, §21 (by line ranges); our crate's `maths/filters.rs` (whole) and `maths/num.rs` (1-180,
270-300), `crates/larmorx-fsl/PROVENANCE.md` (its end); our suite `parity/fsl_maths.py` (parts:
the thresholds, the expected divergences, `_prepare`, `_own_cases`, `_run_case`, the notes),
`parity/fsl_maths_replica.py` (the `conv/fft-*` cases by grep, `oracle_run`, `run_oracle`);
`evidence/fsl_maths_original.py` (1-640), `evidence/fsl_maths.py` (800-935 and greps),
`figures/cache.py` (1-220), `tables.py` (`_maths_original`); `scripts/orchestration/README.md`
(grep), `compare_records.py` (head), `gates-rust.sh` and `gates-python.sh` (greps); our docs:
the findings (headings, 7-9, 31), `docs/api/fsl-maths.md` (parts), `docs/benchmarks/fsl-maths.md`,
the records `docs/validation/fsl-maths.md` and `.json` (by script); `tests/golden/registry.py`
(4940-5260: this part's cases); `validation/investigations/cleanroom-maths/` (ours); in the site
clone its `README.md` (the checks' commands), `data/verdicts.yml`, `content/tools/fsl-maths.md`,
`content/roadmap.md` (grep).
Ran: FSL only through the oracle cache in `use` mode, never directly: `fsl_outputs.py` for
`conv/fft-sphere`, `-odd`, `-double`, `-4d` and `orig/fft-asym` (served), the suite on 130
convolution, `orig/fft`, `d13/` and permanent cases (served but FSL's crashes, never stored) and
the record (437 FSL runs: 396 served, 41 ran = FSL's crashes, never stored); FSL's outputs of
the page's call from the replica's figures-cache entry `fsl-maths/families` and of the
benchmark's FFT-path calls from relay 5's benchmark directory (no new FSL run). The original:
unit tests, the suite, the record, `fft_cases_reference.py`, `bench.py` (the original at 1 and
14 threads on the two FFT-path calls, twice: the kept design and a reverted trial; the replica
not run), the recomputed figures-cache entries and the export; the gates (the Rust gates build
and test `crates-nc` and `crates-gpl` as always: only their `OK` lines and test counts read);
the site's build, number and viewer checks and screenshots. Every job in a `boxed.sh` box (at
most 24 GiB at once, one at a time).
Not opened: FSL's source, `crates-nc/`, `crates-gpl/`, `crates/larmorx-afni-replica`,
Armadillo's, nlohmann's and glibc's sources, the restricted findings, `docs/parts/`, other
investigations folders (`spec-answers-3` included), `validation/probes/`, `triage.toml`, the
replica authors' permanent cases, the oracle cache's files, the main checkout.
Slips: none known. The fslmaths export runs the replica's evidence code in its process (its
case list reads the replica suite's cases), as relay 7's exports did.

## Spec questions (for a spec-answers part; the evidence in docs/findings/fsl-maths-original.md)

1. FSL-152: the far side is inclusive for -thr/-uthr (so float >= 2^24 maxima and +inf are kept)
   and exclusive for -bin/-binv/-mas; `short` wraps as `char` and `int` do (spec and register
   name char/int only).
2. -save writes the input's display range (not (0,0) after an operation, not -range's).
3. FSL-159 -max/-min with a larger image: the same voxel indices (the operand's own strides).
4. The input's missing-image message is `#63` only (no `#22`, no `Error :`); an operand's has
   all three.

Relay 2 (findings 5-13 hold the evidence):
5. The morphological filters' count: NaN kernel values are counted (gauss 0 -dilF: no "Empty mask
   image"); the spec says values in [0.5, FLT_MAX].
6. A kernel without voxels in -fmean/-fmeanu: "ERROR:: Empty mask image" once, then the off-centre
   warning per convolution (size 0 even); the separable path with negative sizes prints nothing
   and gives zeros; an unknown -kernel type consumes its numeric argument; 3D/2D take none.
7. The non-separable kernel's normalising sum is a double sum rounded to single (spec: single).
8. FSL's FFT path is a true convolution (kernel flipped), the direct sum a correlation; for even
   sizes their windows are one voxel apart (the spec does not say).
9. FSL-191 (new defect, rule A): the FFT path's NaN volume.
10. -edge with ≤ 2 slices: the denominator over all three axes.
11. -dog_edge under integer types: filtered values truncated to the type before the test.
12. -inm/-ing empty mask: the mean is 0 (m/0: ±inf, NaN only for 0 or NaN voxels); the spec says
    "the image becomes NaN".
13. -subsamp2/-subsamp2offc under floating types: FSL's last bits match no simple order (a spec
    answer from a source reader would let the original match bit for bit).
14. §21's share criterion (99 % within 4 ulp) cannot hold for the FFT path (its rounding is
    relative to the image's largest values) nor where a sum cancels to near 0; the suite gates on
    the max-relative bound and reports the share (expected divergences): propose that as the spec.
    Since relay 7b (the original's double sums where FSL takes its FFT, findings 31) 2 of the 5
    FFT cases meet the share; the 3 that do not lie closer to the exact result than FSL's FFT
    (the original 4.1e-8 to 6.3e-8 of its largest value, FSL 6.9e-8 to 1.0e-7), so the share
    measures FSL's rounding there. A spec answer might also say whether §10.2's "FFT, the
    product in single precision" makes FSL's FFT path an exact-result reference (the exact
    convolution, rounded once) rather than FSL's bits.
Relay 3 (findings 14-18 hold the evidence):
15. FSL-192 (new defect, rule A): FSL's FFT path mirrors the kernel (finding 14).
16. `-bptf`'s high-pass products are taken left to right, `C = sum (w D) D`, `D = sum (w D) x`
    (the spec's `w D^2` differs in double's last bits; finding 15).
17. `-ztop` of +-inf gives the positive NaN (0x7fc00000); `-ptoz` of NaN gives the default NaN
    (finding 16).
18. TFCE's `E` (and `H`) are single precision: `pow(s, (double) E)` (finding 17).
19. `-tfceS` without the sum reaching T: the voxel's cluster at h0; without any threshold: the
    voxels outside every cluster at h0; FSL-178's in-volume offset: the value at the offset, the
    voxel in no cluster (finding 17).
20. `-tensor_decomp`: the nine names are the output's name as given plus `_L1`... (so
    `out.nii.gz_L1.nii.gz`); the L and MD display ranges are 0 (the zeros included) to the max;
    the eigenvectors' signs are LAPACK's, with no simple rule (finding 18).
Relay 4 (findings 19-22 hold the evidence):
21. `-roc` (findings 19): the noise counts take every voxel of a noise volume (the truth's mask
    not applied), `sd(FP)` from the noise volumes' rates over their voxels, 0 for one volume; the
    candidates are the mask's voxels not above 0; the rates' means are double sums rounded to
    single; a double image is compared in single; each volume's area is on the common rate; a
    rate that never reaches |L| adds the last rectangle at the last TP; the quartiles come
    before the division by |L|; the curve file is created before the noise and truth are read.
22. FSL-193 (new defect, rule B): `-roc`'s unwritable curve file (findings 20).
23. fslstats (findings 21): the preoptions are taken while at least two arguments remain (the
    spec says "more than two"); a crash loses buffered standard output (each standard-error line
    flushes it); `-P`, `-e`, `-E`, `-w` make the masks as `-l` does, multiplying the image; `-M`,
    `-S`, `-s`, `-m` of zeros print 0 without a message; `-json -K` without labels writes
    `"indices": []`; the histogram's empty-mask message comes before its limits' check; `-l 100
    -e` prints it, then FSL-185's crash.
24. fslstats `-C` of a neurological image: FSL's x is 6e-8 relative from the spec's double sums
    (BET's call), within §10's bound; FSL's arithmetic for the mapping back is unknown.
25. FSL-187 in BET's and FLIRT's originals: corrected by default, no `reproduce` (findings 22).
Relay 5 (findings 23 holds the evidence):
26. `-s`'s Gaussian at voxel sizes not exact in binary (0.8333333, 0.85 mm): FSL's impulse
    response has the spec's extent but differs from the spec's form in 20 and 24 of 49 taps
    (1 mm: identical); on the T1w -s 5: 2.8e-7 of the largest value, 98.6 % within 4 ulp.
27. fslstats' unmasked `-S` with many nonzero voxels (`-t <BOLD> -M -S -r`): the last printed
    digits differ from §5.2's form (54 of 63 volumes, 3e-10 relative).
Unverified choices, relay 4: `-roc`'s true voxels limited to the mask (matters only for a truth
whose maximum is at most 0); FSL-179 with a noise image of other x, y, z sizes (offsets with the
input's sizes: verified for other volume counts only); `FP < |L|` with L in double; fslstats
`-H`'s order of checks (nbins < 1 against a missing min or max); the masked variance of values
holding NaN kept NaN; `-h`/`-H` without a histogram write no JSON entry; `-k`/`-d` images read
once per run.
Unverified choices, relay 3: FSL-168's refusal (libstdc++'s scans leaving the values) never
reached by a case; `-rank`/`-ranknorm` NaN in the default (NaN entries stay NaN, the rest ranked
among themselves, T their count); `-pval`/`-pval0` NaN permutations left out of the count and the
denominator; `-cpval`'s maxima without NaN from the image's non-NaN minimum; `-tfceS`'s FSL-177
refusal for arrays of no element and its NaN-step refusal; FSL-178 offsets other than x = 10;
`-tfce`'s order of the size and step errors when both apply; `-roi` with negative sizes on t.

Unverified choices (no case reaches them; black-box later if needed): -kernel boxv 0 with the
separable -fmean warns per axis (size 0 even); an unknown type keeps the separable flag; -fmean's
convolution of ones warns per volume of a 4D image; the even-extent warning from the offsets of
values > 0.5; the window overflow counted over the entering values (the dilations: non-zero
ones); FSL-162's "nothing left" result is the default NaN; -inm's block counting over masked
values; -subsamp2's geometry for a flipped (neurological) input.

Relay 6 (findings 23 to 30):
26. (answered) §9's Gaussian: each term `i² (d d)`, added x, y, z (findings 23).
31. §3/§10.3: a new grid's geometry: unset matrices through the mirroring only; the arithmetic
    (single-precision matrices, double products; 6 of 165 probe calls one ulp off) (findings 24).
32. §15: -roc's candidates are the voxels below the true voxels' bound (findings 29).
33. §21's tensor metric for degenerate tensors (eigenvalues at the rounding level; "distinct"
    relative to what).
34. fslstats -c/-C of images holding NaN and infinities: FSL's NaN signs.
35. -ztop's and -ptoz's last bits (Φ, Φ⁻¹: the exact Cephes forms would let the original match).
36. -randn in double: Φ⁻¹'s exact form.
37. §10.1: the morphology's warning by the kernel's sizes; no empty-mask message below 0.5.
38. §10.3 under the floating types: FSL's subsampling arithmetic (still unknown; findings 29).

Relay 7:
39. §21 and §20 on FSL's FFT path: §10.2's direct sum in single precision (FSL's direct
    path's arithmetic) is less accurate than FSL's FFT on large kernels (findings 31: 1.4e-5 of
    the largest value from the exact result on a 1,313-voxel sphere, FSL's FFT 1.5e-7), so the
    sums' bound of 1e-5 can fail on real data; may the original accumulate in double where
    FSL would take its FFT (the specification's measure then holds: 2.0e-7)?
27 (relay 5) needs no answer for the original's sake: with the final build fslstats' unmasked
`-S` on the series is FSL's text (findings 23).
