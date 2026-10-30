# HANDOFF: the replica side of 2026-10-30 (GLIBC-05, FSL-65/66, SCI-05, the parallel smoke tier)

Branch `worktree-agent-a54c42b251eb08a4d` (from e1fb75a). Each block below: the file, where, and
the exact text. No new defect ID (FSL-67, FSL-68, SCI-06, SCI-07 unused).

---

## 1. `docs/findings/upstream-defects.md` (the register)

### 1a. The summary table, row SCI-05 (line 236): replace the whole row with

| SCI-05 | SciPy `map_coordinates` | a NaN coordinate with orders 2 to 5 and mode `nearest` (the suite's 3D float32 inputs, `prefilter=False` mostly): the output there is NaN in every run, but its sign and payload vary between runs on the same inputs (SciPy 1.15.3 and 1.18.1 alike) | b | none known (fMRIPrep's resampling uses other modes) | deterministic (larmorx follows SciPy 1.18.1, bit for bit; NaN values compared alike); not reproduced; the resample-series step counts NaN values alike and runs the shared releases twice (2026-10-30) | – |

### 1b. Row GLIBC-05 (line 247): in its larmorx column replace

`in the clean-room original (2026-10-30); the replica keeps its warning until a replica author wires the same function | replica |`

with

`in both implementations (2026-10-30: the clean-room original, then the replica, whose last-bit warning is gone) | – |`

### 1c. Row FSL-65 † (line 334): in its larmorx column replace

`; the replica writes everything it can and ignores the failure in both modes (a follow-up) | the replica |`

with

`; the replica the same since 2026-10-30 (by default the .mat directory, the .par and the .rms files opened before anything is written, an existing file kept until written; reproduce in mcflirt's order) | – |`

### 1d. Row FSL-66 † (line 335): in its larmorx column replace

`; the replica runs normally in both modes (a follow-up) | the replica |`

with

`; the replica the same since 2026-10-30 (its own line saying how mcflirt ends, after mcflirt's; FSL-21 named in reproduce too) | – |`

### 1e. Section `### SCI-05` (line 3284): replace the heading and the bullets **What**, **Evidence**, **larmorx**, **D13 / gap** with

### SCI-05 `map_coordinates` gives a NaN coordinate a NaN read from memory never written (SciPy 1.15.3 and 1.18.1) — (b)
- **What:** `scipy.ndimage.map_coordinates` with `mode="nearest"` and orders 2 to 5 on a NaN
  coordinate returns NaN in every run, but with another sign or payload from run to run on the
  same inputs (`7fc00000`, `ffd7ff97`, `fff7fff7`, `7ff8000000000000`, ...). **Cause** (read
  2026-10-30 from SciPy's source, `scipy/ndimage/src/ni_interpolation.c`, `NI_GeometricTransform`,
  the same code in 1.15.2 and 1.18.1): the filter's first index is `(npy_intp)floor(NaN) -
  order / 2`; x86-64 converts NaN to `INT64_MIN`, and for orders 2 to 5 subtracting 1 or 2
  overflows (undefined in C; it wraps to near `INT64_MAX`). The border test `start < 0 || start +
  order >= len` then fails (its sum overflows too), so the coefficients are read at the unchecked
  offset `stride × start`, which wraps to one or two strides before the array on each axis: memory
  the call never wrote. The NaN coordinate's spline weights are NaN, so the output is NaN whatever
  was read; when a value read is itself a NaN, its sign and payload pass into the product
  (`coeff *= weight`). Orders 0 and 1 subtract nothing and take the border path: they never vary.
- **Evidence:** measured (2026-10-29: the resample-series record's "originals' own step" detail of
  `ndimage/nearest-order5` differed between two runs); verified (2026-10-30,
  `validation/investigations/replica-side/sci05_probe.py`: the suite's `nearest-order<K>` inputs,
  each call 20 times in one process, three processes per environment of 1,296 calls each: SciPy
  1.15.3 2, 7 and 7 calls vary, SciPy 1.18.1 1, 2 and 1; always and only the output at the NaN
  coordinate, orders 2 to 5, NaN in every run, only its bits differing). The earlier "not in SciPy
  1.18.1" was two runs that happened to agree.
- **larmorx:** deterministic, bit for bit SciPy 1.18.1's elsewhere; the suites compare NaN values
  alike (`resample_series._bits`), so no comparison changes. The resample-series step (2026-10-30)
  digests the originals' outputs with NaN values counted alike and runs the shared releases' cases
  twice, reporting a comparison whose original varies between the two as varying, never as changed
  or identical (`parity.python_versions`, `STEP_PASSES`, `VARIES`).
- **D13 / gap:** class (b), never reproduced.

### 1f. Section `### FSL-65 †` (line 4363): replace the bullets **larmorx** and **D13 / gap** with

- **larmorx:** the clean-room original (2026-10-30) and the replica (2026-10-30, the same day):
  an error by default (exit 1, the file and FSL-65 named, nothing of the step written; every text
  output opened, the `.mat` directory made, before any is written, what the step made removed; the
  replica keeps a file that existed until it writes it); mcflirt's behaviour with `reproduce` (the
  replica writes in mcflirt's order: the `.par` file opened first, the `.mat` directory, the `.rms`
  files line by line, then each volume's `MAT_` file and `.par` line; an unopenable `.par` ends at
  volume 0's line with mcflirt's message and status 0, after `MAT_0000` and the `.rms` files; an
  unopenable `.rms` not written) (`specs/mcflirt.md` §10.6; the 33 cases of §14.1 in both suites).
  The replica's earlier code wrote the `.par` file whole at the end, so an unopenable one went
  unnoticed and the run wrote everything else with status 0 in both modes.
- **D13 / gap:** none.

### 1g. Section `### FSL-66 †` (line 4385): replace the bullets **larmorx** and **D13 / gap** with

- **larmorx:** both implementations since 2026-10-30: refused by default before any work (rule B:
  a negative number of bins makes no sense, and mcflirt crashes on most of them), exit 1, every
  cost, `-stages` 1 or more; with `reproduce`, where mcflirt crashes (a stage with −1 bins: SIGFPE;
  −2 or fewer: `std::bad_array_new_length`, its two `terminate` lines printed) an error saying how
  mcflirt ends, exit 1, nothing written; mcflirt's run where it does not (−1 to −3 with `normcorr`
  and `leastsquares`, FSL-66 named) (`specs/mcflirt.md` §12.1). The replica checks the stage's bins
  where mcflirt prepares them (`Costfn::set_no_bins`, for every cost), so a one-volume series
  without `-reffile` (nothing registered) runs as mcflirt.
- **D13 / gap:** none.

### 1h. The history paragraphs: after the paragraph beginning `**2026-10-30, the clean-room \`cbrtf\``, add

**2026-10-30, the replica side:** GLIBC-05 done in the AFNI replica (`cbrtf_glibc` with
`reproduce`, the clean-room original's notice by default; `specs/cbrtf.md` §7.3 and §7.6 run:
`cbrtf_glibc` equal to the glibc translation on all 2^32 inputs, and the replica's `reproduce`
radius glibc's on every float32 voxel volume); FSL-65 and FSL-66 done in the mcflirt replica (the
D13 gaps of both closed); SCI-05's cause found (a NaN coordinate's index overflow, in SciPy 1.18.1
as well) and the resample-series step made robust to it. No new entry, no **decide** entry.

### 1i. The paragraph `**The mcflirt replica**` (line ~538): replace its last sentence

`**Open:** FSL-65
and FSL-66 in the replica (the clean-room original has FSL-20, FSL-21, FSL-65 and FSL-66 since
2026-10-30).`

with

`FSL-65 and FSL-66 refused by default with
the switch since 2026-10-30, as in the clean-room original (FSL-21 now named in `reproduce` too).`

---

## 2. `docs/api/upstream-defects.md`

Row GLIBC-05 (line 106), column "where": replace `(the clean-room original, \`implementation="original"\`)` with `(both implementations since 2026-10-30)`.

Row FSL-65 (line 120): replace `` `larmorx mri hmc` (the replica: to do) `` with `` `larmorx mri hmc` and the replica `larmorx-nc fsl mcflirt` ``.

Row FSL-66 (line 121): replace `` `larmorx mri hmc`, `lx.mri.hmc` (the replica: to do) `` with `` `larmorx mri hmc`, `lx.mri.hmc` and the replica `larmorx-nc fsl mcflirt` ``.

---

## 3. `docs/next-steps.md`

### 3a. Replace the follow-up `- **The mcflirt replica's FSL-65 and FSL-66** (found 2026-10-30): ...` (lines 273-277) with

- **The mcflirt replica's FSL-65 and FSL-66: done** (2026-10-30, `specs/mcflirt.md` §10.6 and
  §12.1; the 33 cases of §14.1 in the replica's suite, mcflirt 6.0.7.23 live: 24 pass, 9 rejected
  by both, the replica's matrices mcflirt's as text wherever both run to the end; a command-line
  test each; golden `mcflirt.fsl65.*`, `mcflirt.fsl66.*`; FSL-21 named in `reproduce` too).

### 3b. In the follow-up `- **3dUnifize \`-EPI\`'s \`cbrtf\`: decided by the user (2026-10-30); the clean-room half done the same day, the replica's half to do:**` (lines 476-499): change its title to `... the clean-room half and the replica's half done the same day:**`, replace the sentences from `**To do (a replica author):**` to `... as its correctly rounded cube root implies.` with

**Done in the replica (2026-10-30):** its `-EPI` radius takes `cbrtf_glibc` with `reproduce` and
the correctly rounded cube root by default, with the clean-room original's GLIBC-05 notice word
for word (its last-bit warning gone); the `glibc-05/epi-*` cases in its suite (5 of 5 pass:
`reproduce` AFNI 26.3.05's bytes, the default AFNI's with correctly rounded functions preloaded);
`specs/cbrtf.md` §7.3 (`cbrtf-measure --glibc-identity`: 0 of 2^32 differ from the glibc
translation, NaN bits included) and §7.6 (the replica's `reproduce` radius glibc's on every
float32 voxel volume, so no radius, message or ball differs from AFNI's); record re-made
(compare_records: only the 11 new cases, GLIBC-05's 5 and AFNI-75's 6 guard cases, which the
replica record had not run before); golden `3dUnifize.EPI.glibc05(.reproduce)`.

and replace the sentences from `**For the spec writer:**` to `... or the number \`335d1fb5fb714c95\`.` with

**§7.2 corrected (spec writer, 2026-10-30):** the 2026-10-23 program's hash starts from
`1469598103934665603`, FNV's offset basis with its last digit missing; with §7.1's start the
whole sequence is `335d1fb5fb714c95`, with the program's `c679cb92d4402e33`, for glibc's
results and `cbrtf_glibc`'s alike.

### 3c. Replace the follow-up `- **The resample-series step and SciPy 1.15.3's varying outputs (SCI-05)** ...` (lines 513-518) with

- **The resample-series step and SCI-05: done** (2026-10-30): SCI-05's cause found (a NaN
  coordinate's filter index overflows for orders 2 to 5, so SciPy reads memory before the array;
  only the NaN's bits vary, in SciPy 1.18.1 as well: register SCI-05,
  `validation/investigations/replica-side/`); the step's digests count NaN values alike and the
  shared releases' cases run twice (a comparison varying between them is reported as varying);
  the record re-made from a clean commit, two runs give the same step detail (two records from bf26b71 identical case by case, the same step line; against 2026-10-29's no status change, 9 comparisons of nearest-order2 to -order5 now identical, their only change having been SCI-05's NaN bits).

### 3d. Add under the follow-ups (near the orchestration items)

- **The parity smoke tier in parallel: done** (2026-10-30, the user: "make sure everything is
  done properly"): `scripts/orchestration/parity_smoke.py` runs `tests/parity` on 16
  pytest-xdist workers, then `test_memory_peak.py` and `test_thread_pools.py` alone
  (`tests/parity/conftest.py` `SERIAL`); the same marker for `record_golden.py`; `--serial` the
  fallback. Proved on one commit and build: the per-test outcomes of a serial and a parallel run
  identical (2,600 of 2,600 tests with the same outcome, all passed in both; 1,574 s serially, 951 s in parallel: 291 s on 16 workers, then 660 s for the 107 memory and thread tests alone, of which the memory peaks take about 626 s; commit 7582eae), `validation/investigations/replica-side/`. `verify-merge.sh`
  step 4 and `record_golden.py` use it (`--serial-parity`); verify-merge's `--suite`s run side by
  side (`run-suites.sh`; `--serial-suites` the fallback), proved on two suites
  (`afni-automask --implementation replica` and `ants-smooth-image`: the same records case by case, `compare_records.py --against`; 13 s one after the other, 8 s side by side, served from the oracle cache). pytest-xdist 3.8.0 (with execnet 2.1.2) added to the shared `.venv` and
  the lock file.

---

## 4. `docs/validation/golden.md`

### 4a. After the sentence ending `... every other checksum unchanged; \`larmorx_core::math::cbrtf_glibc\` ...` paragraph (line ~34), add

The AFNI replicas' checksums (`crates/larmorx-cli/tests/afni_replica_golden.tsv`) were re-recorded
on 2026-10-30 from 0072334 (the replica's parity passing): 2 cases added,
`3dUnifize.EPI.glibc05` and `.reproduce` (GLIBC-05: 2 × 2 × 2.9924254 mm voxels, both modes, the
outputs differing), every other checksum unchanged. `larmorx-nc`'s were re-recorded on 2026-10-30
from f822f64: 6 cases added, `mcflirt.fsl65.par` (refused: recorded by its exit status, nothing
written), `mcflirt.fsl65.par.reproduce`, `mcflirt.fsl65.rms.reproduce`, `mcflirt.fsl66.bins`
(refused), `mcflirt.fsl66.bins.reproduce` (the `bad_array_new_length` crash as an error: status 1)
and `mcflirt.fsl66.bins-1.reproduce` (20 rows), every other checksum unchanged.

### 4b. The recording command (line ~241): replace

`python scripts/record_golden.py           # runs pytest tests/parity (smoke tier) first; refuses if it fails`

with

`python scripts/record_golden.py           # runs the parity smoke tier first (in parallel: scripts/orchestration/parity_smoke.py; --serial-parity serially); refuses if it fails`

---

## 5. `scripts/orchestration/README.md` (the table)

Replace the `verify-merge.sh` row with

| `verify-merge.sh <checkout> <out> [--suite "..."]... [--site <dir> --evidence <tool>...] [--golden] [--threads N] [--oracle-cache use\|refresh\|off] [--serial-parity] [--serial-suites]` | the checklist's steps 1-5 on a committed merge, one line per step in `<out>/main.log`; step 4 the smoke tier in parallel (`parity_smoke.py`, `--threads` its workers, 16 without), the `--suite`s side by side (`run-suites.sh`: their `--jobs` add up), STEP5 lines in the order given; the parity runs use the oracle-output cache (below) |

and add after it

| `parity_smoke.py [--workers N] [--serial] [--junit-dir DIR] [-- <pytest args>]` | the parity smoke tier (`pytest tests/parity`) on N pytest-xdist workers (16), then the memory-peak and thread-pool files alone; the marker `record_golden.py` reuses; `--serial` one pytest run (run with `python -P`, `PYTHONPATH` set) |
| `run-suites.sh <checkout> <out> [--serial] [--main-log FILE] "<parity args>"...` | standard-tier records of several suites side by side (verify-merge's step 5), each in its own log |
| `compare_records.py <out>/records <suite>... [--against DIR]` | the re-run records against the committed ones, case by case (or against another re-run's) |

(the last replaces the existing `compare_records.py` row).

---

## 6. `CLAUDE.md`

- "Gates before the final commits": replace `` `pytest tests/parity` `` (the last item) with `` the parity smoke tier (`python -P scripts/orchestration/parity_smoke.py`: `pytest tests/parity` on 16 pytest-xdist workers, then the memory and thread tests alone, about 16 minutes, 26 serially; `--serial` for one pytest run) ``.
- "Merging an agent's work", step 4: replace `` 4. `pytest tests/parity` (the smoke tier against the real oracles). `` with `` 4. The parity smoke tier against the real oracles (`verify-merge.sh` runs it in parallel, `scripts/orchestration/parity_smoke.py`; `--serial-parity` serially). ``
- "Records only from a clean commit": `(it runs the parity smoke tier first, about 10 minutes, ...` → `(it runs the parity smoke tier first, in parallel, about 16 minutes, 26 serially, ...`.

---

## 7. The site (`larmorx_documentation`), `content/roadmap.md`

Row 1 ("A cube root with the C library's digits, for `reproduce`"): status `Clean-room half done (2026-10-30); the replica's half under way` → move the step to "Completed" with the date 2026-10-30 and the text: **A cube root with the C library's digits, for `reproduce`.** 3dUnifize `-EPI`, in the clean-room original and the replica, keeps LARMORX's correctly rounded cube root by default (naming GLIBC-05 where the C library's last bit would change the result) and gives AFNI's exact result in `reproduce` mode, through a clean-room cube root with the C library's bits, proved on all 2^32 inputs. (No site files of mine were changed: the AFNI and mcflirt pages are regenerated from the records at the final validation phase or by the orchestrator's re-export.)

---

## 8. `specs/mcflirt.md` (not mine to edit: for the orchestrator or the spec writer; behaviour only)

Two spec-writer notes are now outdated (the replica does both since 2026-10-30):

- §10.6, line ~766: replace `FSL-65 is new: the replica writes everything it can and ignores an
  unopenable \`.par\` or \`.rms\` file in both modes, a follow-up of the replica (the table is its
  target).` with `FSL-65 too since 2026-10-30 (the table's rows, observed with §14.1's cases).`
- §12.1, line ~898: replace `FSL-66 is new: the replica and the clean-room original both run
  normally on negative \`-bins\` in both modes (they take the stage's negative bins as none, or the
  original as one), a follow-up of the replica (the table is its target).` with `FSL-66 too since
  2026-10-30, in the replica and the clean-room original (the table's rows, observed with §14.1's
  cases).`

## 9. `CLAUDE.md`, "Where things are" (the FSL row)

Replace `standard tier: 181 of 181 compared cases,` with `standard tier: 205 of 205 compared cases (31 rejected by both; FSL-65 and FSL-66 since 2026-10-30, the 33 failure cases of specs/mcflirt.md §14.1),`. In the AFNI row the replicas' `cbrtf` sentence ("`cbrtf`'s last bit measured, a decision for the user") can read: `cbrtf` glibc 2.35's bits with `reproduce` in both implementations since 2026-10-30 (GLIBC-05).
