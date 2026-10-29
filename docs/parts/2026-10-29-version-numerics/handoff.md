# HANDOFF: the version update's part 3 (version-numerics, 2026-10-29)

Branch `worktree-agent-a8576b00480da36c8` from 3a3a500. Site branch `version-numerics` in
`<scratch>/site`. Defect ID used: **FSL-64** (none of NP-03..07, SCI-05..09 needed: SciPy's
change is SCI-03 fixed upstream; numpy's SVD change reaches no ported path).

Below, the exact texts for the shared documents. Rows of the register are behaviour-only; the
section marked † carries the source detail (restricted).

## 1. `docs/findings/upstream-defects.md`

### 1a. Table row, FSL (after FSL-63)

```
| FSL-64 † | FSL 6.0.7.23 (Armadillo 14.2.1, OpenBLAS 0.3.33) | the last bits of its 4 × 4 inverses, solves and determinants, and so of mcflirt's and BET's results computed from them, depend on the CPU: OpenBLAS chooses its kernels at start-up, and those of AVX-512 CPUs fuse multiplications and additions that the others round separately (on this AMD Zen 4, 1 of mcflirt's 137 recorded runs changes: a relative rms of identical volumes, 8.07e-15 against 7.98e-15 mm) | d | the last bits of mcflirt's and BET's outputs depend on the machine (an AVX-512 server against an AVX2 laptop) | one order everywhere: the Haswell kernels' (as numpy's, NP-01); the harness runs FSL on that core (`OPENBLAS_CORETYPE=Haswell`, 2026-10-29) | – |
```

### 1b. SCI-03 row (replace)

```
| SCI-03 | SciPy splines | `reflect`, `grid-mirror` do not interpolate on short axes (SciPy up to 1.17) | c | none | fixed in SciPy 1.18, whose initialisation larmorx follows in both modes since 2026-10-29 (corrected by default from 2026-10-11 until then) | – |
```

### 1c. Section FSL-64 (after FSL-63's section)

```
### FSL-64 † FSL 6.0.7.23's matrix arithmetic depends on the CPU — (d)
- **What:** FSL 6.0.7.23's Armadillo 14.2.1 inverts, solves with and takes determinants of
  4 × 4 matrices through LAPACK (`dgetrf` + `dgetri`, `dtrtri`, `dgesv`), which is OpenBLAS
  0.3.33 (`liblapack.so.3` → `libopenblasp-r0.3.33.so`, conda-forge, `DYNAMIC_ARCH`). OpenBLAS
  picks its kernels by CPU at start-up: its Haswell and Zen cores compile the C kernels with
  `-mavx2` only, its SkylakeX, Cooperlake and Sapphire Rapids cores (AVX-512 CPUs) with
  `-march=skylake-avx512`, where GCC fuses `dgemv_n`'s tails, `daxpy` and the triangular solves
  into multiply-adds. On 100,000 seeded matrices per family, Cooperlake's `inv()` equals
  Haswell's for 6 % of random matrices, 19 % of affines, 18 % of rigid ones, every diagonal
  sampling, swap and translation matrix; its `inv(A)·B` for 0 to 2 %.
- **Evidence:** measured (`validation/investigations/version-numerics/lapack_probe.py --cores`,
  `lapack_probe_summary.json`, `openblas_kernels.py`); mcflirt's 137 recorded runs made again on
  the Haswell core (`oracles/fsl-6.0.7.23/mcflirt-haswell`) against the Cooperlake ones this CPU
  chose: 136 identical, 1 changed (`identical-copies/reffile-fudge`, its relative rms:
  `compare_cores.py`).
- **fMRIPrep:** its mcflirt and BET (FSL 6.0.7.23 and later, when it moves to them) give other
  last bits on an AVX-512 machine than on an AVX2 one.
- **larmorx:** the replicas follow the Haswell order on every platform (`larmorx-nc-core`,
  `lapack.rs`), as `larmorx_transform::openblas` does for numpy (NP-01); the harness runs FSL
  with `OPENBLAS_CORETYPE=Haswell` (`fsl_versions.OPENBLAS_CORE`, part of the oracle cache's key
  for FSL's programs).
- **D13 / gap:** not D13 (no defect of FSL's code: a property of its build).
```

### 1d. SCI-03 section (replace the larmorx paragraph)

```
- **larmorx:** corrected by default (2026-10-11 to 2026-10-28), `reproduce` giving SciPy's.
  **SciPy 1.18 fixed it** (`ni_splines.c`: the sum in a variable of its own, then
  `c[0] += sum·z/(1 − z^2n)`; the rounding of its scaling differs from larmorx's correction);
  larmorx follows SciPy 1.18.1 in both modes since 2026-10-29 and names nothing
  (`docs/findings/scipy-ndimage.md`, "SciPy 1.18.1"). Under SciPy 1.15 the resample-series
  cases the change reaches are expected divergences (`python_versions.CHANGES`).
- **D13 / gap:** done (fixed upstream).
```

### 1e. The "done" table (main package): SCI-03's row: "fixed upstream in SciPy 1.18 (2026-10-29)". History paragraph: "2026-10-29 (the version update's part 3): FSL-64 new (d); SCI-03 fixed in SciPy 1.18.1, followed." Counts: one new entry (FSL-64, class d); no **decide** entry added.

## 2. `docs/api/upstream-defects.md`

SCI-03 row (replace):
```
| SCI-03 | SciPy `ndimage` | `lx.ndimage.*`, `resample_series(mode=...)` | SciPy up to 1.17: the spline prefilter's reflect boundary (modes `reflect`, `grid-mirror`, `nearest`) read a coefficient it had already updated, so its splines missed the samples on short axes | SciPy 1.18's exact half-sample-symmetric initialisation, in both modes (no notice since 2026-10-29) |
```
FSL-64 row (FSL section):
```
| FSL-64 | mcflirt, bet (FSL 6.0.7.23) | `larmorx-nc fsl mcflirt`, `larmorx-nc fsl bet`, `lx.mri.hmc(implementation="replica")`, `lx.fsl.bet` | the last bits of results computed from 4 × 4 inverses and solves depend on the CPU FSL runs on | the order of FSL's library on CPUs without AVX-512, on every platform |
```

## 3. `docs/licensing.md` (the FSL track, after "FSL 6.0.7.23")

```
- **Armadillo 14.2.1, OpenBLAS 0.3.33 and reference LAPACK 3.12.0** (2026-10-29): the FSL
  replicas follow FSL 6.0.7.23's matrix layer, whose 4 × 4 inverses, solves and determinants go
  through LAPACK; `larmorx-nc-core/src/lapack.rs` translates the routines from OpenBLAS 0.3.33
  (BSD-3-Clause) and the reference LAPACK 3.12.0 it bundles (modified BSD), Armadillo 14.2.1's
  rules are in `arma.rs` (Apache-2.0); their notices are in `crates-nc/NOTICE`. Permissive
  sources, included in the FSL-licensed package as the other permissive ones are.
```

## 4. `UPSTREAM.md`

- SciPy row: version `v1.15.2` → `0f1fd4a7268b`, **and 1.18.1** (sdist SHA-256
  `52c4b7422442…`, `<workspace>/tags/scipy-1.18.1`; followed since 2026-10-29: the reflect
  initialisation, `docs/findings/scipy-ndimage.md`).
- OpenBLAS row: add "0.3.34 (numpy 2.5.4's scipy-openblas64 0.3.34.106.0; tarball SHA-256
  `cd7e12986832…`): the same kernels, checked 2026-10-29" and a new row:
  `| OpenBLAS (FSL's) | BSD-3-Clause | 0.3.33 release tarball, SHA-256 6761af1d9f5d… (tags/OpenBLAS-0.3.33) | FSL 6.0.7.23's liblapack.so.3 / libblas.so.3 (libopenblas 0.3.33, conda-forge) | LAPACK's 4 × 4 getrf, getri, trtri, gesv and the Haswell kernels they reach, with the reference LAPACK 3.12.0 (dgetri) it bundles: larmorx-nc-core's lapack.rs |`
- numpy (behaviour references): "2.3.5, and 2.5.4 (sdist SHA-256 `9a94cf751c9a…`; checked
  2026-10-29: no change on the paths larmorx follows)".
- FSL table: `fsl-armawrap` row's last cell: "4 × 4 inverses, solves and determinants through
  LAPACK (followed since 2026-10-29)"; `libopenblas` row: "Armadillo 14.2.1's LAPACK (followed,
  Haswell kernels)".

## 5. `upstream.tsv` (append)

```
scipy-1.18.1	https://github.com/scipy/scipy/releases/download/v1.18.1/scipy-1.18.1.tar.gz	sha256:52c4b7422442aba924d03ad4019852b08a92e64ea187b933135687bfe2747307	BSD-3-Clause	tags
numpy-2.5.4	https://files.pythonhosted.org/packages/source/n/numpy/numpy-2.5.4.tar.gz	sha256:9a94cf751c9ad8ebaa835bcd3d40dacf8534ad086b88c38029b65123c7999d2a	BSD-3-Clause	tags
OpenBLAS-0.3.33	https://github.com/OpenMathLib/OpenBLAS/releases/download/v0.3.33/OpenBLAS-0.3.33.tar.gz	sha256:6761af1d9f5d353ab4f0b7497be2643313b36c8f31caec0144bfef198e71e6ab	BSD-3-Clause	tags
OpenBLAS-0.3.34	https://github.com/OpenMathLib/OpenBLAS/releases/download/v0.3.34/OpenBLAS-0.3.34.tar.gz	sha256:cd7e129868320cc2d033afa920e31202dfe0b8066a5b66661900ccc0f197dfed	BSD-3-Clause	tags
```
(The URLs of the two upstream-watch downloads, scipy and numpy, as the watch fetched them: check
`scripts/orchestration/upstream_watch.py`'s sources; the SHA-256 are of the files in `tags/`.)

## 6. `CLAUDE.md`

- Rule 2: add "SciPy 1.18.1, numpy 2.5.4 with OpenBLAS 0.3.34 (since 2026-10-29: the
  resampler and numpy's 4 × 4 arithmetic; SciPy 1.15.3 / numpy 2.3.5 stay in the shared
  environment for the step); FSL's own OpenBLAS 0.3.33 with its reference LAPACK 3.12.0 and
  Armadillo 14.2.1 (the FSL replicas' 4 × 4 arithmetic)".
- "Where things are", I/O row: "fMRIPrep's one-shot BOLD resampler (`lx.transforms.resample_series`,
  following SciPy 1.18.1 and numpy 2.5.4 since 2026-10-29: records in `oracles/python-newest`,
  `--python newest`)".
- FSL row, the mcflirt replica: replace "(since 2026-10-29; standard tier: 176 of 176 compared
  cases, 5 expected divergences in the last bits from Armadillo 14.2.1's LAPACK inverses, FSL's
  own step from 6.0.7.17 explained in every case; ..." by "(since 2026-10-29; standard tier: 181
  of 181 compared cases, Armadillo 14.2.1's LAPACK inverses, solves and determinants followed bit
  for bit, FSL on OpenBLAS's Haswell kernels (FSL-64); FSL's own step from 6.0.7.17 explained in
  every case; ...". BET: "every file FSL 6.0.7.23 writes byte for byte in every compared case of
  its standard tier (130 of 130, 318 files; `--binaryMask`, the zstd and bzip2 types and
  6.0.7.23's matrix arithmetic included)".
- "In place": "**Python environments:** `oracles/python-newest` (numpy 2.5.4, SciPy 1.18.1)
  beside the shared `.venv`; `--python newest|shared` (`LARMORX_PYTHON`) for `parity`,
  `evidence`, `difftest`, the step from the shared releases in the resample-series record."
- "Next": the version update done (parts 1-3); next 3c (AFNI replicas into the main package).

## 7. `docs/next-steps.md`

- Item 3: "**Part 3: done** (2026-10-29, `docs/parts/2026-10-29-version-numerics/`): the FSL
  replicas follow Armadillo 14.2.1's LAPACK inverses, solves and determinants (OpenBLAS 0.3.33's
  Haswell order, bit for bit against FSL's own library on 30.2 million matrices; mcflirt's five
  expected divergences pass; BET's trial cases 80 and 141 agree; FSL-64: the harness pins FSL's
  OpenBLAS core); the resampler follows SciPy 1.18.1 (SCI-03 fixed upstream) and is checked
  against numpy 2.5.4 with OpenBLAS 0.3.34 (no change); the oracle environment
  `oracles/python-newest` and the step per case in the resample-series record."
- The follow-up "The replicas follow Armadillo 14.2.1's LAPACK inverses": **done** (2026-10-29).
- "The version update, what changed in the numerics": "**done**: every file read and classified
  (`docs/findings/scipy-ndimage.md`, `numpy-blas.md`); one change of results (SciPy's reflect
  initialisation), followed."
- New follow-ups:
  - **The shared environment's numpy and SciPy** (recommendation in section 13).
  - **FSL's core on other machines:** the final validation phase runs FSL's records on the old
    laptop (AVX2: OpenBLAS picks Haswell by itself) as a check of FSL-64's pin.
  - **The stage comparisons' FSL** (`stages/runners.py`, `stages/nodes.py`) do not pin the
    OpenBLAS core (they compare with fMRIPrep 25.2.6's container): pin it at the final
    comparisons if FSL 6.0.7.23 is used there.
  - **Affines mixing NaNs of both signs, or holding infinities, in mcflirt's decomposition into
    rotations** (85 of 200 probe affines differ from FSL 6.0.7.23 in a NaN's sign, 148 against
    6.0.7.17): the replica follows the source's order; FSL-14's matrices are all one NaN (no
    recorded run meets the mixed case). Measure with the probes
    (`validation/probes/fsl-mcflirt-affine-trace`) if a real case ever does.
  - **Random testing's harness** (done in this part): a run stopped for time leaves no process
    behind (`Leaf.stop_others`; the version trial's BET timeouts had left FSL's `bet2` running
    for hours), and `difftest --case` honours `--case-timeout`. **For the orchestrator:** a
    `bet2` of the version-fsl agent's trial (`2026-10-29-version-fsl-2`) still runs since 10:06
    on 2026-10-29, orphaned in that trial's memory box (scope of `agent-afdb6eadb1ef500f6`'s
    `lxv.sh ... difftest`), one CPU; it can be stopped (`pkill -f "lx-difftest-inputs_n0y528hb"`
    or stopping that scope); this part did not touch it.

## 8. `docs/decision-log.md` (2026-10-29)

```
- **FSL's OpenBLAS core pinned to Haswell** (the version update's part 3; rules settle it): FSL
  6.0.7.23's 4 × 4 matrix arithmetic depends on the core OpenBLAS picks for the CPU (FSL-64,
  class d). The replicas follow the Haswell order, as numpy's port does (NP-01); the harness
  runs every FSL program with `OPENBLAS_CORETYPE=Haswell`, the variable is part of the oracle
  cache's key for FSL, and mcflirt's 6.0.7.23 recordings were made again on that core
  (`oracles/fsl-6.0.7.23/mcflirt-haswell`; the Cooperlake recording kept).
- **The newest Python releases beside the shared environment** (the user's "newest versions,
  updated in steps"): `oracles/python-newest` (numpy 2.5.4, SciPy 1.18.1, the shared
  environment's other packages) is the reference of the Python originals' suites
  (`--python newest` by default for resample-series, resample-series-jacobian, numpy-linalg);
  the shared `.venv` is unchanged (section 13 of the handoff: the recommendation).
```

## 9. `validation/difftest/triage.toml`

The trial (`difftest --tools fsl-mcflirt,fsl-bet,resample-series --python newest --budget 0.5
--jobs 6 --seed 20261029 --out <workspace>/oracles/difftest/2026-10-29-version-numerics`; FSL
6.0.7.23 on the Haswell core, numpy 2.5.4 and SciPy 1.18.1): 5,269 cases in 30 min; numbers in
section 12. Its larmorx is stamped `1281f57a806f-dirty`: the uncommitted files were the Jacobian
suite's header line and an investigation script (committed a minute later as 06c5013); the code
the trial's three tools run (the descriptors, the parity suites, the crates) is the same from
1281f57 to 9b833a3's parent (`git diff --stat 1281f57 9b833a3^ -- validation/src python crates
crates-nc`: only the resample-series evidence and the Jacobian suite).

(a) Append to the existing open entry of `resample-series` ("one voxel of a near-zero value
(-7.8e-17, a cancellation) one float ulp from fMRIPrep's ...") the sentence:

```
; the version trial of 2026-10-29 (numpy 2.5.4, SciPy 1.18.1) found four more of the class (`resample-series:20261029:369`, 1243, 1700, 2144: mode grid-constant, orders 1, 3, 4, 3; 1 to 3 values of 13,104 to 24,000 differ, at most 5.7e-14 in absolute value), and 369 replays alike against numpy 2.3.5 and SciPy 1.15.3 (`--python shared`): not the update's
```

(b) New entry (the BET timeouts):

```
[[difference]]
tool = "fsl-bet"
implementation = "replica"
signature = ["timeout"]
status = "explained"
explanation = "slow, not different: the version trial's BET timeouts (300 s, 6 jobs at 1 thread) run to their end one by one, FSL 6.0.7.23 and the replica timed apart (validation/investigations/version-numerics/bet_timeouts.py, 20 min each): fsl-bet:20261029:70 (bet -Z -f 0.5 -r 60 on voxels of 0.01 x 2.5 x 1.2 micrometres), 351 (bet -f 0.3 -r 60 -m -s) and 446 (bet2 -f 0.1 -r 30 -m -s -n) agree, every file byte for byte and both streams as the suite compares them (FSL 97 s, 366 s, 108 s; the replica 99 s, 250 s, 74 s); 843 (bet -F -m on voxels of 2.5 m x 50 m x 1 cm, an extreme geometry) runs past 20 min in bet2 on its first volume in both, the files written until then identical. 70, 351 and 843 also timed out in the version-fsl trial (same seed). Their bet2 was left running, orphaned, after the timeout: fixed in the harness (Leaf.stop_others)"
link = "validation/investigations/version-numerics/README.md"
```

(c) The 130 PIPE-03 notices of resample-series (the notice named where no value changed: a
field map with the Jacobian on another target grid, every case of the class) and the 2 FSL-45
notices of BET are the classes the long run of 2026-10-25 and the version-fsl trial already
counted (87 and 2 there); the update does not touch either notice. No entry (notices are counted
per defect, not triaged: `docs/validation/difftest.md`).

## 10. `docs/validation/golden.md`

- `larmorx-nc`'s checksums re-recorded on 2026-10-29 from f587288 (Armadillo 14.2.1's LAPACK
  path): 1 output changed, `mcflirt.nonfinite.reproduce.par` (the all-NaN volume's y rotation
  keeps the affine's NaN, as FSL 6.0.7.23 gives it; 6.0.7.17 gave it the other sign); the rest
  (261 outputs) unchanged.
- The main package's checksums re-recorded on 2026-10-29 from 4811172 (the parity smoke tier
  passed there: 2,583 tests): 7 of 895 outputs changed, every one `lx.ndimage`'s reflect-family
  boundary (`py.ndimage.map_coordinates` modes `reflect` and `grid-mirror`;
  `py.ndimage.spline_filter`; `py.ndimage.reproduce`'s `reflect`, `grid-mirror`,
  `spline_filter` and `spline_filter1d`): SciPy 1.18.1's reflect initialisation, in both modes
  (SCI-03 fixed upstream); the rest (888) unchanged.

## 11. The site's Roadmap (`content/roadmap.md`) and `validation/tools.toml`

Roadmap, "Under way and next": remove row 1 ("SciPy, NumPy and OpenBLAS at their newest
releases"), renumber, and set "The AFNI replicas in the main package" to **Next**. "Completed"
(newest first), a new first row:

```
| 2026-10-29 | **SciPy, NumPy and OpenBLAS at their newest releases.** The one-shot BOLD resampler follows SciPy 1.18.1 (its fix of the `reflect` boundary, SCI-03) and is checked against numpy 2.5.4 with OpenBLAS 0.3.34 (nothing LARMORX follows changed); the FSL replicas follow FSL 6.0.7.23's matrix library (Armadillo 14.2.1 on OpenBLAS 0.3.33) bit for bit, so mcflirt's 5 expected divergences pass; FSL's own last digits depend on the computer's processor (FSL-64), and every record runs FSL on one set of kernels |
```

`validation/tools.toml` (then `python -m larmorx_validation tables`; the branch's generated tables
were made from the current text):
- `resample-series`, `reference`: "fMRIPrep's `ResampleSeries`, nitransforms 25.1.0, SciPy
  1.18.1 and numpy 2.5.4 (in-process, `oracles/python-newest`; the step from SciPy 1.15.3 and
  numpy 2.3.5 in every case)".
- `numpy-linalg`, `reference`: "numpy 2.5.4 on OpenBLAS 0.3.34's Haswell kernels
  (`oracles/python-newest`; numpy 2.3.5 on OpenBLAS 0.3.30 gave the same results), nitransforms,
  sdcflows, fMRIPrep (in-process); other kernels in
  [numpy-linalg-kernels.json](repo:docs/validation/numpy-linalg-kernels.json)" (the kernels file
  was made with numpy 2.3.5 and is not re-made).
- `fsl-mcflirt-replica`, `status`: "`validated` ({fsl-mcflirt-replica.pass} of
  {fsl-mcflirt-replica.compared} compared cases bit-identical, fMRIPrep's calls included;
  {fsl-mcflirt-replica.both} rejected by both, with the same messages and status; FSL 6.0.7.23's
  LAPACK 4 x 4 inverses followed since 2026-10-29, FSL on OpenBLAS's Haswell kernels (FSL-64))"
  (the current text ends "{...expected} expected divergences: the last bits of FSL 6.0.7.23's
  LAPACK 4 x 4 inverses, not followed yet", which now reads "0 expected divergences: ... not
  followed yet").

## 12. Numbers (records, golden, trial)

Records (each from a clean commit of the branch; `compare_records.py` against main's 3a3a500):
- `fsl-mcflirt-replica` (standard, FSL 6.0.7.23 on the Haswell core, from 5924325): 203 cases,
  **181 pass**, 22 rejected by both, **0 expected** (was 176 / 22 / 5): exactly the five cases of
  Armadillo 14.2.1's LAPACK path changed status, expected divergence → pass (`opt-stages1`,
  `nan/default`, `nan/reffile`, `identical-copies/default`, `identical-copies/reffile-fudge`);
  nothing else differs. FSL's own step: 179 of 197 identical, 18 changed, every one explained.
- `fsl-bet-replica` (standard, from 9b833a3): 178 cases, **130 pass**, 31 rejected by both, 17
  expected (12 filled crashes, 4 pending, 1 FSL-43 memory); 318 of 318 files byte for byte;
  against main's record (176 cases) no case changed status; the 2 new cases are the version
  trial's 80 and 141 (both pass). FSL's own step: 27 of 174 identical, 147 changed, every one
  explained (the two new cases by `arma-lapack-centre`).
- `resample-series` (standard, numpy 2.5.4 / SciPy 1.18.1, from 9b833a3): 139 cases, **136
  pass**, 2 rejected by both, 1 expected (as before); the originals' own step from numpy 2.3.5 /
  SciPy 1.15.3: 122 of 136 the same, 14 changed, every one SCI-03 (modes `reflect`,
  `grid-mirror`, `nearest`, orders 2 to 5); in the shared environment the same suite gives 124
  pass and 13 expected divergences (the SCI-03 cases, named).
- `resample-series-jacobian` (numpy 2.5.4 / SciPy 1.18.1, from 3674d8b): every number as before.
- `numpy-linalg` (standard, numpy 2.5.4 with OpenBLAS 0.3.34, from d7bd05d): 28 cases, 27 pass, 1
  expected; 18,143,058 of 18,143,058 results bit-identical on the Haswell kernels, as with numpy
  2.3.5.

The LAPACK probe: 30,200,000 records (18 routines and Armadillo calls × 22 families, 302 pairs,
100,000 matrices each), every output word identical to FSL 6.0.7.23's `liblapack.so.3` on the
Haswell core, NaN signs and payloads included; the determinant compared by value, its NaN as
NaN (Armadillo's `det()` is emulated in Python, and FSL only compares a determinant: 11,814 of
the non-finite family's 100,000 differ in NaN bits only).

Golden: section 10. The parity smoke tier: 2,583 passed (at 4811172).

Memory: the BET calls of `tests/parity/test_memory_peak.py` pass (smoke tier); mcflirt (not in
the test): fMRIPrep's call on ds000005 run 1, FSL 6.0.7.23 336.8 MiB in 15.6 s, the replica
352.9 MiB in 3.2 s (+4.8 %, +16.1 MiB, within the rule's 10 % + 8 MiB), every output FSL's
(image header's description aside, which the replica writes as its own name outside the suite).

The trial (section 9): 5,269 cases in 30 min (6 jobs): fsl-bet 1,118 (1,112 agree, 2 notices,
4 timeouts), fsl-mcflirt 1,767 (all agree), resample-series 2,384 (2,250 agree, 130 notices, 4
differ: the open near-zero class). The four BET timeouts run to their end
(`bet_timeouts.py`, 20 min each): 70, 351 and 446 agree (FSL 97, 366 and 108 s; the replica 99,
250 and 74 s); 843 (voxels of 2.5 m × 50 m × 1 cm) runs past 20 min in both, the files written
until then identical (section 9b).

Gates (at f5408ff): Rust, the three workspaces (fmt, clippy -D warnings, tests): OK; Python
(`gates-python.sh --no-build`): 618 passed, 12 skipped, ruff, format, SPDX OK; `pytest
tests/parity`: 2,583 passed; `tables --check`: OK.

## 13. Recommendation: the shared `.venv` and `validation/requirements-lock.txt`

Recommendation: **keep the shared `.venv` at numpy 2.3.5 / SciPy 1.15.3 for now**, and move it
with the final comparisons with fMRIPrep (when fMRIPrep's own lock moves, or when the newest
fMRIPrep is run at the end). Reasons:
- Nothing else needs the move: the suites of the Python originals run in `oracles/python-newest`
  by default (`--python newest`), and the other suites use numpy and SciPy only as tools
  (reading images, comparing arrays), whose results these releases do not change.
- The shared environment is the step's baseline: the resample-series record reports, per case,
  what SciPy 1.15.3 → 1.18.1 changed; it needs the old releases somewhere.
- What a move would change: the `pytest tests/parity` resample-series smoke cases of the reflect
  modes would pass instead of being expected divergences; `numpy-linalg` and `nifti-io` would
  run on numpy 2.5.4 (the same results, measured for numpy-linalg); `antspyx 0.6.3` and
  smriprep's pins ask for numpy < 2.5 (pip refuses the combination; `make_python_newest.sh`
  installs with `--no-deps`), so the lock would need those two checked or bumped; the
  evidence's cached plots would not change (they read stored arrays).
```
