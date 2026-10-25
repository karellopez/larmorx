# HANDOFF: the mcflirt replica and the `larmorx-nc` package (FSL Licence)

Branch `worktree-agent-a5874ac3558d79d85` (larmorx) and `fsl-mcflirt-evidence` (site clone in
this scratch directory, `site/`). Every text below is exact; the orchestrator integrates and
harmonizes it into the shared documents. The numbers are summed up under "Numbers" at the
bottom.

## 1. `docs/findings/upstream-defects.md` (the register)

### 1a. Table rows (the restricted FSL rows; replace FSL-04, FSL-15, FSL-18; add FSL-20, FSL-21 after FSL-18)

```
| FSL-04 † | mcflirt `-gdt` | reference gradient unused; absolute `-out` aborts | c, a | none | refused by default (both implementations); the replica reproduces it with `reproduce` (2026-10-24) | – |
| FSL-15 † | mcflirt `-init` | affects the final resampling only | c? | none | reproduces | – (decided 2026-10-24, rule C: as documented, "apply to all vols"; kept and documented) |
| FSL-18 † | mcflirt Woods cost | squared bin counts | c? → none | none | reproduces | – (decided 2026-10-24, rule C: not a defect; `n_b² σ_b / Σ_b` is Woods' `n_b σ_b / μ_b`) |
| FSL-20 † | mcflirt | `.mat` directory not creatable (a missing parent): message, exit 0, nothing more written | e | none | the replica: an error by default (rule B, 2026-10-24), mcflirt's exit 0 with `reproduce`; the clean-room original: to do (the spec) | – |
| FSL-21 † | mcflirt histogram costs | `-bins` below 8: no bins in the first stage; arbitrary matrices, `corratio` writes before its arrays and aborts | a | none | the replica: refused by default (rule B, 2026-10-24); `reproduce` gives mcflirt's matrices and, for `corratio`, its abort; the clean-room original: to do (the spec) | – |
```

### 1b. Entries (section "Restricted: FSL mcflirt (source-derived)")

Replace the "larmorx" and "D13 / gap" lines of **FSL-04** with:

```
- **larmorx:** refused by default in both implementations (exit 1, the notice names FSL-04);
  the replica reproduces it with `reproduce` (the gradient images registered to the plain
  reference, `grefvol_` written), bit-identical to mcflirt on a relative `-out`
  (`edge/gdt` of the replica's record).
- **D13 / gap:** done (2026-10-24).
```

Replace the "larmorx" and "D13 / gap" lines of **FSL-15** with:

```
- **larmorx:** reproduces, in both implementations.
- **D13 / gap:** decided 2026-10-24, rule C: mcflirt's help says "initial transform matrix to
  apply to all vols", which is what it does (the final resampling); a pipeline that passes
  `-init` gets the composed resampling it asked for. Kept and documented.
```

Replace the "What", "larmorx" and "D13 / gap" lines of **FSL-18** with:

```
- **What:** the Woods cost sums `Sqr(num[b])*stdev/sum[b]` (`costfns.cc:1456-1561`). Since
  `sum[b]/num[b]` is the bin's mean, that is `n_b σ_b / μ_b`, Woods' criterion: not a defect
  (read while translating the replica, 2026-10-24).
- **larmorx:** reproduces (both implementations; the replica bit for bit).
- **D13 / gap:** decided 2026-10-24, rule C: not a defect; kept.
```

Add after FSL-18:

```
### FSL-20 † An uncreatable `.mat` directory ends the run with status 0 — (e)
- **What:** `Log::establishDir` (`Log.cc`) makes `<out>.mat` (adding `+` while the name exists);
  `decompose_mats` (`mcflirt.cc:489-494`) prints "Error! Could not create directory ... No
  write permission" for a permission error and goes on, but for any other error (a missing
  parent directory: `-out missing/out -mats`) `establishDir` prints "Error!: Unknown Log
  directory return value" and calls `exit(0)`: no matrices, `.par`, `.rms` or corrected series,
  and success reported.
- **Evidence:** read; verified (`edge/out-dir-missing` of the replica's record).
- **fMRIPrep:** none (its `-out` is in an existing work directory).
- **larmorx:** the replica (2026-10-24): an error, exit 1, naming FSL-20 (rule B: the input
  cannot be honoured, and mcflirt reports success); `reproduce` gives mcflirt's message and exit
  0. The clean-room original: to be specified (`specs/mcflirt.md`, the spec writer).
- **D13 / gap:** done in the replica; the original's is open.

### FSL-21 † Histogram costs with fewer than 8 bins have none — (a)
- **What:** the histogram costs get `no_bins / scaling` bins at each stage (`usrsetscale`,
  `mcflirt.cc`; `Costfn::set_no_bins`, `costfns.cc:3622-3661`), so with `-bins` below 8 the
  first (8 mm) stage has none, and below 4 every stage. With no bins, `mutualinfo`, `normmi`
  and `woods` compute a degenerate cost and the search ends at arbitrary matrices (every
  volume the same, or tens of millimetres away; deterministic); `corratio` folds its last bin
  into index -1 (`numy[no_bins-1] += numy[no_bins]`, `costfns.cc:1269-1274`), before its arrays,
  and glibc aborts when they are freed ("double free or corruption (out)", signal 6) at the
  first cost evaluation.
- **Evidence:** read; verified (`edge/bins4-mutualinfo`, `edge/bins1-normmi`,
  `edge/bins2-corratio` of the replica's record).
- **fMRIPrep:** none (it runs normcorr).
- **larmorx:** the replica (2026-10-24): refused by default before any work (rule B: a
  histogram needs at least one bin; mcflirt silently returns arbitrary matrices or corrupts its
  heap), the notice names FSL-21; `reproduce` gives mcflirt's matrices bit for bit and, for
  `corratio`, its abort (when the out-of-bounds write is non-zero, as at every real
  evaluation). The clean-room original: to be specified.
- **D13 / gap:** done in the replica; the original's is open.
```

### 1c. GLIBC-02 and GLIBC-03 entries: add to their "larmorx" lines

```
  The mcflirt replica (`larmorx-nc`) carries glibc 2.35's FMA build of `sinf`, `cosf`,
  `sincosf` and `logf`, translated from Arm's Optimized Routines (MIT) and verified on all 2^32
  inputs (`crates-nc/larmorx-nc-core/tools/glibc-float-verify`), as FSL's Linux binaries call
  them (2026-10-24).
```

### 1d. The register's own lists

- In "D13 gaps": FSL-04, FSL-20 and FSL-21 are done in the replica; FSL-20 and FSL-21 are open
  in the clean-room original (its spec).
- No new **decide** entry: FSL-15 and FSL-18 (both "later: when the mcflirt replica is built")
  are decided above by rule C; FSL-20 and FSL-21 by rule B.
- Counts and the history paragraph: the orchestrator's recount (`scripts/orchestration/register_counts.py`).

## 2. `docs/api/upstream-defects.md`

Add after the FSL-13 row:

```
| FSL-04 | mcflirt | `larmorx fsl mcflirt` (the replica; the original refuses `-gdt` in both modes) | `-gdt` registers each volume's gradient image to the reference's plain image | refused (exit 1); `reproduce` registers the gradient images as mcflirt does |
| FSL-20 | mcflirt | `larmorx fsl mcflirt` (the replica) | when the `.mat` directory cannot be created for another reason than its permissions, mcflirt prints "Error!: Unknown Log directory return value" and exits with status 0, writing nothing more | an error (exit 1) |
| FSL-21 | mcflirt | `larmorx fsl mcflirt` (the replica) | with a histogram cost and `-bins` below 8, the first stage's histogram has no bins: arbitrary matrices, and `corratio` aborts | an error before any work (exit 1) |
```

And in its paragraph on the "Where" column (or after the table): "Rows naming `larmorx fsl
mcflirt` are the replica's (`larmorx-nc`, `lx.mri.hmc(implementation="replica")`); it shares
the clean-room original's corrections of FSL-01, -03, -05 to -10, -13 and -14."

## 3. `README.md`

Replace the head-motion row of "Supported tools":

```
| Head-motion correction (mcflirt) | `larmorx mri hmc`, `larmorx fsl mcflirt`, `lx.mri.hmc` | clean-room original, `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial) | original: [compared with FSL's mcflirt](docs/validation/mri-hmc.md): 121 of 122 compared cases within thresholds; [5–16× faster](docs/benchmarks/mri-hmc.md) with threads; replica: [155 of 155](docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built (the 137 recorded runs, fMRIPrep's calls on the five oracle runs, 32 edge cases; 21 more rejected by both, alike), fMRIPrep's own matrices identical |
```

In "Roadmap", item 2: "**FSL-compatible tools:** BET, FAST, FLIRT with BBR, topup (the mcflirt
replica is done)."

## 4. `docs/licensing.md`

Replace "`larmorx-nc` will be packaged the same way." with:

```
  `larmorx-nc` is packaged the same way (`crates-nc/pyproject.toml`, licence
  `LicenseRef-FSL`): its wheel carries the FSL Licence (`crates-nc/LICENSE`), the FSL,
  Armadillo, fdlibm and Arm notices (`crates-nc/NOTICE`), the Apache-2.0 text and `NOTICE` of
  the larmorx crate it includes and Arm's MIT licence (`crates-nc/LICENSES/`), and, because the
  FSL Licence requires "all original and amended source code" in every transmitted product,
  FSL's original source of the replicated parts (`crates-nc/fsl-source/`, with `MANIFEST.tsv`)
  and the Rust translation itself, all as licence files. `crates-nc/larmorx-nc-cli/tests/notices.rs`
  checks the copies and the source manifest.
```

Under "Finding the program", item 1: "the package's environment variable, `LARMORX_GPL_BIN` or
`LARMORX_NC_BIN` (an error if it names no file);".

In the build snippet add:

```
  (cd crates-nc && maturin build --release --locked --out ../dist)    # larmorx-nc (FSL Licence)
  pip install --find-links dist larmorx-nc                            # by name, knowingly
```

## 5. `docs/validation/README.md` (the validation index)

Add after the `mri-hmc` row:

```
| [fsl-mcflirt-replica](fsl-mcflirt-replica.md) | `larmorx-nc fsl mcflirt`, `lx.mri.hmc(implementation="replica")` (the replica, FSL Licence) | mcflirt (FSL 6.0.7.17 as built: the recorded runs, fMRIPrep's calls, edge cases run here) | `validated` (155 of 155 compared cases bit-identical; 21 rejected by both, with the same messages and status) |
```

And in its commands list:

```
python -m larmorx_validation parity fsl-mcflirt-replica --tier full --out docs/validation   # needs FSL at ~/fsl and crates-nc built
```

## 6. `docs/validation/golden.md`

Add a third column to the table (`larmorx-nc` (the FSL replica)): cases
`crates-nc/larmorx-nc-cli/tests/golden.rs`: 23 cases, 179 outputs; checksums
`crates-nc/larmorx-nc-cli/tests/golden.tsv`; recorded by `LARMORX_GOLDEN_RECORD=1 cargo test
--locked -p larmorx-nc-cli --test golden` (in `crates-nc`). Add the row:

```
| `larmorx-nc fsl mcflirt` | 23 | int16 (the default outputs, int16 written), float32 with a reference file, `-stages 4` and `-sinc_final`, `-meanvol` with `-spline_final` and `-dof 12`, every cost (`mutualinfo` with `-nn_final` and `-bins 64`, `normmi`, `corratio`, `woods`, `leastsquares` with `-smooth 0`), `-init` with `-fudge` and `-rotation 2`, the in-plane mode on a 3-slice slab (and `-2d`), `-stats`, `-gdt`, uint8 and int8 input, NaN and infinite voxels, big-endian float32 in the neurological order written as NIfTI-2, a TR in milliseconds; mcflirt's defects corrected (the default) and, in 7 `*.reproduce` cases, reproduced (FSL-03, -04, -08, -09, -10, -14); every file written fingerprinted (images by header and data, `.mat`, `.par`, `.rms`); each case checks the defects named on standard error |
```

And: the main package's golden registry has a new case `cli.fsl.mcflirt` (`larmorx fsl
mcflirt`, the clean-room original of `mri hmc`), and the `py.mri.hmc*` cases now pass
`implementation=ORIGINAL` (their checksums do not change).

## 7. `docs/next-steps.md`

In the FSL item: "the mcflirt replica: done (2026-10-24): `crates-nc/` (`larmorx-nc`, FSL
Licence, FSL's source included), the core crate `larmorx-nc-core` (NIfTI as FSL does it,
`volume<float>`, interpolation, FLIRT's cost functions, the optimiser, the transform algebra,
Armadillo's rules, glibc's float maths) for the next FSL replicas; record
[fsl-mcflirt-replica](validation/fsl-mcflirt-replica.md)". Next: BET (in progress), FAST,
FLIRT-BBR, topup. Follow-ups:
- the clean-room original's FSL-20 and FSL-21 (a spec writer adds them to `specs/mcflirt.md`:
  behaviour only; the replica's record shows mcflirt's black-box outputs);
- a benchmark report of the replica (informal timings in its record);
- the oracle-output cache at `run_oracle` in `parity/fsl_mcflirt_replica.py`.

## 8. `docs/decision-log.md` (2026-10-24)

```
- **The FSL replica carries glibc's float functions** (as the 3dTshift replica carries glibc's
  `sinf`/`cosf`). mcflirt calls the C library's `sincosf` at every cost evaluation, `logf` in
  the mutual-information costs and `sinf` for the sinc kernels; glibc 2.35's are not correctly
  rounded, and with correctly rounded functions 12 of 79 synthetic recorded runs differed from
  mcflirt as built (each identical to mcflirt with the CORE-MATH preload). `larmorx-nc-core`
  translates glibc's FMA build from Arm's Optimized Routines (MIT; the same code glibc ships,
  so no LGPL code enters the FSL-licensed package) and is verified on all 2^32 inputs; with it
  the replica is bit-identical to mcflirt as users run it on Linux x86-64 in every case. The
  double `cos` and `log` stay correctly rounded: glibc agrees with them at every argument
  mcflirt uses.
- **mcflirt's questionable behaviours, by the rules of 2026-10-23:** FSL-15 (`-init` in the
  final resampling only) rule C, kept; FSL-18 (Woods' squared counts) not a defect (it is
  Woods' criterion), rule C; new FSL-20 (an uncreatable `.mat` directory: exit 0) rule B, an
  error; new FSL-21 (histogram costs with `-bins` below 8: no bins) rule B, an error. Each
  keeps the `reproduce` switch in the replica.
- **`implementation="auto"` runs the FSL replica when `larmorx-nc` is installed**, as for the
  GPL replicas; `larmorx-nc` is installed by name only (non-commercial licence).
```

## 9. `CLAUDE.md` ("Where things are")

Replace the FSL row of the table:

```
| FSL | head-motion correction with mcflirt-style options, clean-room original, `experimental` (`lx.mri.hmc`); the mcflirt replica (`crates-nc/`, package `larmorx-nc`, FSL Licence: non-commercial, FSL's source in `crates-nc/fsl-source/`), bit-identical to mcflirt as built (155 of 155 compared cases of its record, fMRIPrep's calls on the five oracle runs included, and fMRIPrep's own matrices identical); `larmorx-nc-core` is the shared core of the next FSL replicas | `mri-hmc`, `fsl-mcflirt-replica` |
```

Repo map: add `| `crates-nc/` | the FSL replica workspace (`larmorx-nc`, FSL Licence; `fsl-source/`, FSL's original source; `scripts/fsl_source.py` copies and checks it) |` (replacing "`crates-nc/` (planned)"). Gates: "both Rust workspaces" becomes "the three Rust workspaces (root, `crates-gpl`, `crates-nc`)"; `scripts/orchestration/gates-rust.sh <checkout> all` and `verify-merge.sh` now run `crates-nc` too (in the branch).

Build-cache seed: this worktree's `target/` and `crates-nc/target/`.

## 10. The site (`larmorx_documentation`, branch `fsl-mcflirt-evidence`)

- `content/tools/mri-hmc.md` is in the branch (my tool's page). The evidence
  `static/evidence/mri-hmc.json` and its blobs: exported in the branch from this branch's
  evidence module; re-export after the merge (`python -m larmorx_validation evidence mri-hmc
  --site <site>`), then the summary.
- `data/status.yml`: replace the head-motion entry with

```yaml
  - eco: fsl
    tool: Head-motion correction (mcflirt)
    page: /tools/mri-hmc/
    use: ["larmorx mri hmc", "larmorx fsl mcflirt", "lx.mri.hmc"]
    how: clean-room original, `experimental`; exact replica in `larmorx-nc` (FSL Licence, non-commercial)
    validation: "original: [compared with FSL's mcflirt](repo:docs/validation/mri-hmc.md): 121 of 122 compared cases within thresholds; [5–16× faster](repo:docs/benchmarks/mri-hmc.md) with threads; replica: [155 of 155](repo:docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built, fMRIPrep's calls on the five oracle runs included"
```

and in `planned`, the FSL entry's text: `BET, FAST, FLIRT with BBR, topup (the mcflirt replica is done).`
- `content/validation.md`: in "Three ways of reimplementing", replace "(`larmorx-gpl` for AFNI)" with "(`larmorx-gpl` for AFNI, `larmorx-nc` for FSL)" and "Every AFNI tool so far has both." with "Every AFNI tool so far has both, and so has head-motion correction (FSL's mcflirt)."
- `static/evidence/summary.json`: re-export after the merge.

## 11. Code that changed outside my own files (minimal, needed for the tool)

- `Cargo.toml` (root): `exclude = ["crates-gpl", "crates-nc"]`.
- `crates/larmorx-core/src/defects.rs`: FSL-04, FSL-20, FSL-21 registry entries.
- `crates/larmorx-cli`: the `larmorx-nc` package and the `fsl mcflirt` replica entry
  (`replica.rs`), the `fsl` family (`lib.rs`, `fsl mcflirt` = the original `mri hmc`); a test
  that used `lx fsl bet` as an unknown command now uses `lx spm realign`.
- `crates/larmorx-py/src/mri.rs`: `mri_hmc_world` (the replica's matrices in RAS/ITK and FD).
- `python/larmorx/_replica.py` (docs), `python/larmorx/_core.pyi`, `python/larmorx/mri/hmc.py`
  (`implementation=`), `tests/python/conftest.py` (`no_replica` unsets `LARMORX_NC_BIN`),
  `tests/python/test_mri.py` (runs the original).
- `tests/golden/registry.py`: `cli.fsl.mcflirt`; `implementation=ORIGINAL` in `py.mri.hmc*`.
- `.github/workflows/ci.yml`: crates-nc's fmt, clippy and tests inside the `gpl` job.
- `scripts/orchestration/{gates-rust.sh,verify-merge.sh,README.md,build-nc.sh}`,
  `scripts/fsl_source.py`.
- `validation/src/larmorx_validation/__main__.py`: the suite `fsl-mcflirt-replica`.
- Merging with the BET branch: `crates-nc/Cargo.toml` members, `crates-nc/Cargo.lock`,
  `larmorx-nc-cli/src/main.rs` `FSL_TOOLS`, `scripts/fsl_source.py` `FILES`/`VERSIONS`,
  `crates-nc/NOTICE`, `crates-nc/README.md`, golden cases; BET's library parts can move into
  `larmorx-nc-core` (its API: `volume`, `nifti`, `interp`, `costfns`, `miscmaths`, `arma`).

## Numbers

- Record `docs/validation/fsl-mcflirt-replica.{md,json}`, full tier, from clean commit
  920ee8e: 176 cases, 155 pass (bit-identical: every file, the exit status, both streams), 21
  rejected by both alike, 0 failures. By tier: smoke 110 (91 + 19), standard 172 (151 + 21),
  full 176 (155 + 21). Recorded runs 137 (125 + 12), edge cases 32 (23 + 9), fMRIPrep's calls
  7 (7; fMRIPrep's own `.mat` files identical: 1,273 of 1,273 matrices).
- D13 fix pass (the replica's default against its `reproduce` run): every correction named
  exactly when it changes something (FSL-01, -03, -04, -05, -06, -08, -09, -10, -13, -14, -20,
  -21).
- Before the switch to glibc's float functions: 67 of 79 synthetic recorded runs bit-identical
  with correctly rounded maths, the 12 others bit-identical to mcflirt with the CORE-MATH
  preload; with glibc's: 137 of 137 recorded runs (`validation/investigations/fsl-mcflirt-replica/replay.py`).
- glibc float functions: 0 differences on 2^32 inputs each (`sinf`, `cosf`, `sincosf` both
  outputs, `logf`).
- Golden: crates-nc 23 cases, 179 outputs, recorded from 53481ee; the main package's new case
  `cli.fsl.mcflirt` recorded from d628990 with `--skip-parity` (record_golden's own parity
  smoke tier was stopped at 30 % after 1.5 h on the shared machine; the AFNI and ANTs suites it
  was running are untouched by this branch; the head-motion parity tests ran separately).
- Site: number check 182 of 182 agree (before: 116 of 116); re-export byte-identical.
- CI: crates-nc's fmt, clippy and tests inside the `gpl` job; cold locally 12 s (clippy) + 20 s
  (tests) with 3 jobs; estimate on the runners +1 to 2 min per platform cold, about 0.5 min
  warm: +6 to 12 min per CI run cold (×6 platforms), about 3 min warm.


## The site page: before/after inventory (the user's rule of 2026-10-24: only add)

| | before | after |
|---|---|---|
| sections | 7 | 7 (the same) |
| evidence components | 5 | 6 (+ the replica's verdict bars) |
| implementations | original | original, replica |
| agreement bars | 8 (original) | 8 (original, unchanged) + 10 (replica) |
| cases in the explorer | 134 (original verdicts) | 176 (134 with the original's verdicts, unchanged; the replica's verdict on 134 of them and 42 cases of its own) |
| worked example | 1 (fmriprep-hmc), grid 53 × 55 × 34, 6 volumes | the same example, grid and volumes |
| viewer panels | 7 | 10 (+ replica reproduce 3 threads, 1 thread, default) |
| difference panels (metrics, verdict, reason) | 5 | 9 (+ replica − mcflirt: bit-identical; replica default − reproduce; replica 1 − 3 threads; original − replica, both default) |
| default view | 4 panels + 3 differences | 5 panels + 4 differences (+ the replica, + replica − mcflirt) |
| line chart panels | 9 | 9 (the deviation and the corrections panels gain the replica's series) |
| metric table rows | 9 | 14 |
| tables | open (4 rows) | the same |
| speed chart | 3 inputs × 3 series | the same |
| number check | 116 of 116 | 182 of 182 |

Screenshots (`shots/`): `before/mri-hmc-{1440,390}-{light,dark}-y0.png` and
`after/mri-hmc-{1440,390}-{light,dark}-y0.png` (full pages); `before/viewer-*-y3300.png` and
`after/viewer-*-y3950.png` (the viewer, live); `before/example-1440-light-y0.png`,
`after/example-*.png` (the example alone: its canvases render only in the live viewport shots).

## Follow-up of 2026-10-25 (supersedes the numbers above)

Branch `worktree-agent-a5874ac3558d79d85` now = local main `c3e5976` (merged in twice: d0b4036's cache,
then the data panel) + the follow-up commits e687cd1 (atanf aT[0]), ceaaca5, 37d6505 (streamed compare),
d196995 (FSL-08 -meanvol notice), 205bee4 (cases), cd80351 (record), ebcf570 (findings/API),
d6ba67e (merge of main c3e5976), aa2c28f (atanf_with_at0 in the verify tool; investigation tools; atanf
re-verified on all 2^32 inputs after this refactor, so the record from 205bee4 stands).
Site branch `fsl-mcflirt-evidence` = `panel-site` (0ec6198, merged in) + f0eba84; its local
`evidence-blobs` = 5b89607 (295 blobs; push it before main, as the panel part says).

### Numbers (record `docs/validation/fsl-mcflirt-replica.{md,json}`, full tier, from clean 205bee4)
194 cases: 173 pass (bit-identical: every file, exit status, both streams), 21 rejected by both
alike, 0 failures. Recorded runs 137 (125 + 12), edge cases 35 (26 + 9; new: atanf-phantom-2, -11,
-27), fMRIPrep's calls 22 on 12 oracle runs (22 pass; fMRIPrep's own `.mat` files identical:
4,517 of 4,517). Original programs: 57 runs, 50 from the oracle cache. Smoke tier: 113 cases.

### README.md (Tools table, head-motion row: replace the replica part)
replica: [173 of 173](docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built (the 137 recorded runs, fMRIPrep's 22 calls on the 12 oracle runs with BOLD runs, the data panel's included, 35 edge cases; 21 more rejected by both, alike), fMRIPrep's own matrices identical (4,517)

### docs/validation/README.md (fsl-mcflirt-replica row, status cell)
`validated` (173 of 173 compared cases bit-identical, fMRIPrep's 22 calls on the 12 oracle runs included; 21 rejected by both, with the same messages and status)

### CLAUDE.md
- FSL row: "... bit-identical to mcflirt as built (173 of 173 compared cases of its record, fMRIPrep's 22 calls on the twelve oracle runs with BOLD runs included, and fMRIPrep's own matrices identical) ..."
- "The mcflirt replica merged" paragraph: "(173 of 173 compared cases; 21 rejected by both alike)"; add: "Its follow-up (2026-10-25): one constant of the C library's `atanf` (fdlibm's `aT[0]`, taken from a source comment) explained the two panel calls and 58 of the first 59 random-testing findings; FSL-08's notice now also covers the `-meanvol` image's name."

### site data/status.yml (Head-motion correction row)
    how: clean-room original, `experimental`; exact replica in `larmorx-nc`
    validation: "original: [compared with FSL's mcflirt](repo:docs/validation/mri-hmc.md): 121 of 122 compared cases within thresholds; [5–16× faster](repo:docs/benchmarks/mri-hmc.md) with threads; replica: [173 of 173](repo:docs/validation/fsl-mcflirt-replica.md) compared cases bit-identical to mcflirt as built, fMRIPrep's 22 calls on the twelve oracle runs included"
And re-export `static/evidence/summary.json` at the merge (my export wrote only mri-hmc.json and the manifest).

### docs/findings/upstream-defects.md
- FSL-08 table row (l. 279): "mcflirt `-stats`, `-meanvol`" | "end slices left 0; names with the extension inside (the `-stats` images and `-meanvol`'s `_mean_reg`)" | ... | "corrected by default (`upstream_defects`, 2026-10-11); the replica names it for `-meanvol` alone too (2026-10-25, random testing `fsl-mcflirt:20261025:616`)".
- FSL-08 detailed entry (l. 3814 ff., stale: it still says "reproduces both" and "Gap" while the table says corrected): **larmorx:** "corrected by default in both implementations: every slice, and the `-stats` and `-meanvol` images named after the stem of `-out` (`x_meanvol.nii.gz`, `x_mean_reg.nii.gz`); `reproduce` gives mcflirt's slices and names." **D13 / gap:** "none."
- GLIBC-03 (or a sentence in the glibc section): "glibc 2.35's `atanf` (fdlibm, no FMA build) is not correctly rounded on 21,089,462 of the 2^32 inputs (0.49 %), 7,060 with |x| ≤ 0.05; the replica carries it bit for bit (`atanf` on all 2^32 inputs, `atan2f` on 3 × 2^30 seeded pairs: 0 differences)."
- History paragraph: "2026-10-25: the mcflirt replica's follow-up found no new defect of mcflirt: the panel's two differing calls and the random tests' 61 findings were the replica's own (`atanf`'s `aT[0]` from fdlibm's comment, and a missing FSL-08 notice for `-meanvol`), both fixed."

### docs/api/upstream-defects.md (FSL-08 row, "larmorx call" cell)
`larmorx mri hmc -stats`, `-meanvol`

### docs/validation/golden.md
No change: no checksum changed (crates-nc golden and `cli.fsl.mcflirt` pass unchanged).

### Random testing (for the random-testing part's merge)
Six permanent cases in its format: `validation/difftest/cases/fsl-mcflirt/{atanf-default-call,
atanf-matrices-from-0, atanf-matrices-from-4, atanf-printed-params, atanf-par-only,
meanvol-name-notice}.json` (written with its `permanent.promote`, tier smoke, status fixed); they
attach to `fsl-mcflirt-replica` once its harness hook is on main. All 61 findings (shrunk and
original recipes, 122 runs) are bit-identical to mcflirt with this branch, the fix pass included.
