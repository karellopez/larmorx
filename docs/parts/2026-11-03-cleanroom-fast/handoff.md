# HANDOFF: FAST's clean-room original (relays 1 to 5, 2026-11-02/03)

Worktree `<workspace>/larmorx/.claude/worktrees/agent-aa48faf7b7fe7f452`, branch
`worktree-agent-aa48faf7b7fe7f452` (from main a173af5, FAST's replica merged). The site: the clone
`<scratch>/site`, branch `fsl-fast-cleanroom-page` (from the site's main 9940a19; commit 886cfc6,
exported from larmorx 0706968, a clean commit), and its `evidence-blobs` branch (`tools/blobs.py
commit`: 034431b, 780 blobs, push it before the site's main). Larmorx commits of relay 5: c5f3080,
5e9e4c7, df6b055, 0706968 (the evidence code, the API page, `tables.py`); no code of the tool
changed after f0dd88b, so no record, smoke tier or golden checksum was re-run. The exact changes to the shared documents are in
`<scratch>/HANDOFF.toml` (24 operations; `apply_handoff.py --check` with `--site <scratch>/site`:
all apply). Afterwards: `python -m larmorx_validation tables` (README, the validation index, the
site's `data/status.yml`), `register_counts.py`, `lxv.sh . evidence summary --site <site>` (the
summary's FAST row now lists the clean-room original: `evidence/summary.py`, changed in the branch).

## Why each shared change

- **`validation/tools.toml`**: FAST's tool row names both implementations (the original first, as
  BET's and FLIRT's rows do) and the speed; a new `[[record]]` `fsl-fast` (the original's record,
  `docs/validation/fsl-fast.md`) placed before `fsl-fast-replica`. Its `{fsl-fast.fmriprep}` needs
  `tables.py`'s METRICS entry `"fsl-fast": _bet` (changed in the branch, one line, as the replica's).
  The rendered rows were checked in memory (`<scratch>/py/tables_check.py`): every placeholder
  resolves (60 of 61, 3 calls, 25, 1, 1).
- **The register's summary rows FSL-96 to FSL-104**: each "what larmorx does" cell adds "its
  clean-room original 2026-11-03", with the original's own numbers where the replica's are given
  (FSL-98: 2,345,055 and 1,127,078 values change in the original's fix pass; FSL-103: 10,439) and
  its own behaviour where it differs: FSL-99's refusal prefix (`larmorx: error: fast:`) and, with
  several channels and a NaN `-H`, `reproduce`'s β = 0 where FAST reads memory it never wrote;
  FSL-101's trial case (`-v`'s line before the crash); FSL-102's FLIRT clean-room original in the
  same process. Behaviour only; only the summary rows (lines 40 to 400) were read.
- **`docs/api/upstream-defects.md`**: FSL-96's row names both implementations (rows FSL-97 to
  FSL-104 say "the same"); FSL-102's row names the original's FLIRT.
- **`docs/validation/golden.md`**: the original's seven cases (six of FAST, one of the ANALYZE
  reader BET's, FLIRT's and FAST's originals share), recorded from f0dd88b with the smoke tier's
  marker; `cli.fsl.fast` goes through the batched `exp` (the orchestrator's item: a golden case whose
  output depends on `exp_in_place`'s AVX2 path on Linux x64 and its portable path on aarch64).
- **`docs/next-steps.md`**: item 4's FAST status; item 7's paragraph for the original (numbers from
  the record, the benchmark, the trial, the memory and thread tests); the `-S 9` follow-up decided;
  four new follow-ups: two speed steps left (a batched `expf` for the initialisation, the MRF's own
  three `exp` per voxel), the single-channel variance (rule D candidate from relay 1), FAST's spec
  questions for a spec writer (as FLIRT's spec answers were), the open message of random testing's
  case 1840.
- **`validation/difftest/triage.toml`**: five `[[difference]]` entries of `fsl-fast-original` (an
  `append` op, from `<scratch>/triage-fsl-fast-original.toml`; the file itself was never opened).
- **`docs/validation/difftest.md`**: the descriptor's row in the tools table; "The oracle cache"
  gains `LARMORX_DIFFTEST_CACHE_MODE=use` (a part's short trial runs its originals through the
  cache, so its replays and permanent cases are served; nights keep `off`).
- **The site** (`site:` ops): `site.yml`'s FAST summary ("the clean-room original planned" was
  stale); the roadmap's row 1 (FAST's clean-room original complete; "topup's replica beside it":
  adjust to topup's state at the merge), a Completed row of 2026-11-03, "Last updated".

## The shared code changed in the branch, and why

- **`bet::image::read_analyze`** (`crates/larmorx-fsl/src/bet/image.rs`, relay 2; BET's, FLIRT's and
  FAST's originals share it): ANALYZE's scale field (`funused1`) is not applied, and stored voxel
  sizes whose product is negative are read radiologically (no x reversal), as FSL reads its own
  standard priors (black box: `validation/investigations/cleanroom-fast/analyze_scale.py`). BET's
  and FLIRT's records were re-made from f0dd88b and compared with main's (`compare_records.py`:
  168/168 and 305/305 cases, 0 status and 0 detail differences); `cli.fsl.flirt.analyze` pins it.
  BET's and FLIRT's originals change on such inputs (none in their records).
- **FLIRT's `-applyxfm` TR** (`crates/larmorx-fsl/src/flirt/engine.rs`, relay 3, the orchestrator's
  item): the output's `pixdim[4]` is the input's as FSL reads it (`|pixdim[4]|`, 1 for a time axis
  with TR 0); `docs/findings/fsl-flirt-original.md`, "`-applyxfm`'s TR". No FLIRT record moved.
- **`larmorx_core::math::exp_in_place`** (`crates/larmorx-core/src/math/exp.rs`, `mod.rs`'s audited
  dispatch; relay 3, additive): `exp`'s fast path four values at a time (AVX2 + FMA through the
  runtime dispatch, the portable build elsewhere), `exp` itself outside it: the scalar function's
  bits. Relay 4's randomized check (the orchestrator's item: an ignored test, 2^32 values of nine
  kinds, both builds against `exp` bit for bit, NaN payloads too): 0 differences. In larmorx-core's
  PROVENANCE.
- **`difftest/runner.py`**: `LARMORX_DIFFTEST_CACHE_MODE` (`off` by default, `use` for a part's
  trial), so the trial ran FSL through the cache (the brief's rule) and its replays were served.
- **`difftest/permanent.py`**: `SUITES` maps the suite `fsl-fast` to the original's permanent cases
  (`validation/difftest/cases/fsl-fast-original/`); `difftest/tools/__init__.py` registers the
  descriptor `fsl-fast-original`.
- **`parity/thread_pools.py`** and `tests/parity/test_thread_pools.py`: four FAST programs (fMRIPrep's
  call, the default with `-b -B -v`, two channels, priors). The test found a bug: the outputs'
  `Grid::expand` and the negative-value rule ran on rayon's global pool (32 threads at `n_threads`
  1); fixed with `Grid::expand_on` (FAST's own `fast/grid.rs`) and `with_threads`.
- **`tests/parity/test_memory_peak.py`**: the original's three calls against FSL's peaks (from the
  cache); **`tests/golden/registry.py`**: the seven cases, new generated inputs, `lx.fsl.fast` out
  of `REPLICA_ONLY_FUNCTIONS`; **`tests/python/test_fsl_fast.py`**: 2 and 9 channels, the singular
  abort. Relay 1's wiring: `crates/larmorx-cli/src/replica.rs` (`original: true`),
  `crates/larmorx-py/src/fsl.rs`, `python/larmorx/fsl/fast.py`, `_core.pyi`.
- Relay 5: `evidence/fsl_fast.py` (the page), `evidence/summary.py` (FAST's row), `tables.py` (the
  METRICS entry).

**The trial ran from a dirty tree** (e60046f-dirty: the descriptor and the cache mode not yet
committed); every finding was replayed afterwards from committed code (relay 4: four replays, at
most 8 at a time, box 24 GiB), and the fixes are permanent cases of the record made from f0dd88b.

## Spec questions (`specs/fast.md`, for a spec writer; black-box evidence in `docs/findings/fsl-fast-original.md`)

Relay 1: §4.4, the class variance's second-moment quotient stays in double (only the mean is
rounded to single); §4.10, `MRFWeightsTotal` is computed after each sweep, over the posteriors as
they stand; §4.8, the ICM keeps a voxel's type when no type lowers its energy, the initial type 0;
FSL-100 with `-A`; `_restore` holds 0 at the voxels FSL reads as NaN. Register candidate (rule D):
the single-precision variance in the log domain.
Relay 2: §4.5, the bias kernel's normalised taps are double values (single taps times the double
inverse of their single sum, kept in double); §5.2, the initial assignment's products in double,
summed in single; §5.6, the bias weights `q / C(i,i)` rounded to single, and several channels print
no tissue volumes with `--nopve`; §3.1 (and BET's §3.1), ANALYZE: no scale field, negative voxel
sizes radiological; §6.4, FLIRT `-applyxfm`'s `pixdim[4]` (FLIRT's spec); §8.3, how often `ERROR::
Empty mask image` is printed (4n per class statistics, 1 per bias estimate, n per volumes); §9.1
FSL-99, `-H nan`'s ` TOTAL ... RHS 0 ...` lines and stderr pairs, and β with several channels.
Relay 3: §10, an exact parallel order of the Gauss-Seidel sweeps exists (a pipeline of planes: row
`(y, z)` after plane `z-1`'s row `y+1`), which the spec's "wavefront" can name; FLIRT §9.3, the TR
as FSL reads it (`|pixdim[4]|`, 1 for a time axis with TR 0), and `-verbose 2`'s "Resampled volume"
TR for a 3D input (not observed).
Relay 4: §4.5, the field outside the mask is `(0 - mean) · 0` (NaN over the whole image when the
mask's mean is NaN); §5.2, the k-means line is printed after the iteration's inverses (absent at a
singular abort); §9.1 FSL-101, `-v`'s line before the crash (already in the spec); §5.5/§5.3, how
FSL's statistics become NaN after a NaN covariance (random testing's case 1840: FSL `MEAN -nan`,
the original 1).
Relay 5 (from the page's classification): §8.6/§5.8, with several channels the partial volumes'
fraction search has near ties that the matrix arithmetic's last bits decide (the original equals FSL
with correctly rounded maths but for a few partial volumes one step of 0.01 away: 10 cases within
FSL's own band, `multi/s2-options` 18 values where the band has 15): the spec could say that one
step is expected; §11, the thresholds are narrower than FSL's own band on small classes and small
images (ds005003's `_mixeltype` Dice 0.998 between FSL and FSL with correctly rounded maths; the
suite falls back on the band there and says so).
No new defect: FSL-136 to FSL-140 unused.

## The site page (relay 5)

FAST's page now shows the clean-room original beside FSL and the replica in all six examples
(fMRIPrep's call on ds000005, ds000228, ds005003; the bias field; the priors; three channels): four
new panels each (FSL with CORE-MATH's correctly rounded `exp`, `log`, `expf`, `logf` preloaded, the
original in reproduce mode at 14 and 1 threads and in its default mode at 14), six new comparisons
(original − FSL, original − FSL with correctly rounded maths, 1 − 14 threads, default − reproduce,
original − fMRIPrep's own, and the yardstick FSL with correctly rounded maths − FSL), each with a
verdict decided by code from the metrics; the files table gains three columns (the values that
differ), the volumes table three rows; the original's agreement chart; the case explorer with both
implementations (90 cases: 83 shared, 3 the replica's permanent ones, 4 the original's); the speed
chart from the benchmark. Verdicts on fMRIPrep's three calls: the original is bit for bit what FSL
computes with correctly rounded maths (*identical with correctly rounded maths*), as on the bias
field and the priors; the three channels within FSL's own band. The original's 87 cases (each
passing case run again into the figures cache entry `fsl-fast/cases-original`: FSL as built and
with the preload through the oracle cache, the original in reproduce mode, every image compared):
13 bit-identical, 36 identical with correctly rounded maths, 10 within FSL's own band, 2 open
(`multi/s2-options`, and `multi/s5` outside the thresholds, explained), 25 rejected by both, 1
expected divergence. Text corrected: the lede, the facts (implementations, status, records), "what
it is" (the dispatch: no error without `larmorx-nc` any more), the verdicts, each example's text,
the cases, a Speed section, the corrections (the original's), the limits ("no benchmark report yet",
`-S 9` and more). Checks: build 0 problems; number check 1,316 of 1,316; viewer check 520 of 520;
no example reports images that could not be loaded; the inventory of all 20 pages after >= before
(FAST's: 6 examples, panels 5 -> 9 each, comparisons 3-4 -> 8-10, cases 86 -> 90, implementations
1 -> 2, speed rows 0 -> 3; the others unchanged); the page 29.6 MB (budget 40 MB); screenshots
before and after at 1440 and 390, light and dark, in `<scratch>/shots/before` and
`<scratch>/shots/after`. The data-panel examples keep their three slices where they were (the
voxel chosen as before), the glioma's through the tumour.

## Tell the user (through the orchestrator)

- FAST's clean-room original is done: on fMRIPrep's call it gives exactly what FSL computes when
  FSL's maths library rounds `exp` and `log` correctly, and it is 9 to 11 times faster than FSL on
  one thread, 47 to 67 times on 14.
- More than eight channels are supported by the clean-room original (the replica refuses them).
- No new upstream defect was found.
