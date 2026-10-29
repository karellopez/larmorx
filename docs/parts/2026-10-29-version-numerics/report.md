## Version update, part 3 (version-numerics, 2026-10-29): report

Branch `worktree-agent-a8576b00480da36c8`, 26 commits from 3a3a500 to f5408ff; site branch
`version-numerics` (561337d, 419c303); the handoff is `handoff.md` here.

### A. The FSL replicas follow Armadillo 14.2.1 through LAPACK
- **Ported** into `crates-nc/larmorx-nc-core/src/lapack.rs`, from OpenBLAS 0.3.33 (BSD-3-Clause)
  and the reference LAPACK 3.12.0 it bundles: `getrf` and its unblocked version; `getrs` through
  the triangular solves for one right-hand side and the triangular-matrix kernels for several, with
  the Haswell 3 × 3 tiles; `gesv`; `trtri` (with OpenBLAS's zero check); the reference unblocked
  `getri` with the Haswell matrix-vector kernel's fused multiply-add and tail orders. Which
  operand's NaN each operation passes on was measured on FSL's own library.
- **Armadillo dispatch** (`arma.rs`): `inv`, `solve` and `det` follow 14.2.1; the 10.5.1 rules
  are kept for FSL 6.0.7.17. Downstream: an all-NaN affine gives every rotation the affine's own
  NaN, as 6.0.7.23 does; the rms deviation's operation order taken from the disassembly.
- **Probe:** 30,200,000 records (18 routines and Armadillo calls × 22 families × 100,000
  matrices), every output word identical to FSL 6.0.7.23's `liblapack.so.3` on the Haswell core,
  NaN signs and payloads included (determinants by value).
- **FSL-64 (new, class d):** FSL 6.0.7.23's last bits depend on the CPU (OpenBLAS picks its
  kernels at start-up; on AVX-512 CPUs, Cooperlake here, they fuse multiply-adds); 1 of mcflirt's
  137 recorded runs changes between the cores. The harness runs every FSL program with
  `OPENBLAS_CORETYPE=Haswell`, part of the oracle cache key for FSL; mcflirt's 6.0.7.23
  recordings re-made in `oracles/fsl-6.0.7.23/mcflirt-haswell` (the Cooperlake set kept).
- **BET:** the replica's `dc` reads 6.0.7.23's `-0.000000` as GNU dc does; the version trial's
  cases 80 (as drawn) and 141 (shrunk) are permanent cases, both pass.
- **Records** (clean commits; compare_records against 3a3a500): mcflirt replica 203 cases, 181
  pass, 22 rejected by both, 0 expected (was 176 / 22 / 5: exactly the five Armadillo cases
  changed, expected → pass); BET replica 178 cases, 130 pass, 31 rejected by both, 17 expected,
  318 of 318 files byte for byte (no status change; 2 new cases, both pass). FSL's own step from
  6.0.7.17 explained in every case of both.

### B. SciPy 1.18.1, numpy 2.5.4, OpenBLAS 0.3.34
- **SciPy 1.15.3 → 1.18.1:** one change of results, the reflect boundary's spline initialisation
  (SCI-03), fixed upstream; `larmorx-interp` follows 1.18.1 in both modes, no notice.
- **numpy 2.5.4 / OpenBLAS 0.3.34:** the same Haswell kernels. numpy's SVD now raises on ±∞
  where 2.3 returned NaN; no ported path reaches it. No NP or SCI ID needed.
- **Harness:** the Python environment of the newest releases `<workspace>/oracles/python-newest`;
  `--python newest|shared` (`LARMORX_PYTHON`) for `parity`, `evidence` and `difftest`; the suites
  of the Python originals use `newest` by default.
- **Records:** resample-series 139 cases, 136 pass, 2 rejected by both, 1 expected (as before);
  from SciPy 1.15.3 / numpy 2.3.5, 122 of 136 cases unchanged, 14 changed, all SCI-03.
  resample-series-jacobian: every number unchanged. numpy-linalg: 18,143,058 of 18,143,058
  results bit-identical.
- **Recommendation:** keep the shared `.venv` and `requirements-lock.txt` until the final
  fMRIPrep comparisons (antspyx and smriprep require numpy < 2.5; handoff §13).

### Golden, memory, random testing
- Golden: `larmorx-nc` (from f587288) 1 output changed (the NaN sign in
  `mcflirt.nonfinite.reproduce`'s `.par`); main package (from 4811172, smoke 2,583 passed) 7 of 895
  outputs changed, all `lx.ndimage` reflect-family.
- Memory: BET's calls pass; mcflirt on fMRIPrep's ds000005 call: FSL 336.8 MiB in 15.6 s, the
  replica 352.9 MiB in 3.2 s (+4.8 %, within the rule).
- Trial (30 min, 5,269 cases): fsl-bet 1,118 (1,112 agree, 2 FSL-45 notices, 4 timeouts),
  fsl-mcflirt 1,767 (all agree), resample-series 2,384 (2,250 agree, 130 PIPE-03 notices, 4 differ:
  the existing open near-zero one-ulp class, grid-constant, at most 5.7e-14; case 369 differs the
  same with SciPy 1.15.3). The BET timeouts run to their end: 70, 351, 446 agree; 843 (voxels of
  2.5 m × 50 m × 1 cm) runs past 20 minutes in both, identical files until then. Harness fixes: a
  timed-out case's leftover processes stopped (`Leaf.stop_others`); `difftest --case` honours
  `--case-timeout`.

### Site
resample-series (panels for both environments, the step), mri-hmc (181 of 181, FSL-64), fsl-bet
(178 cases, 318 files); summary and status regenerated. Inventory: no loss; number check: no
disagreement (mri-hmc checks 247 numbers instead of 251 and Validation 361 instead of 362: the
expected-divergence segment now has a zero count). Build 0 problems.

### Gates (at f5408ff)
Rust (three workspaces), Python (618 passed; ruff, format, SPDX), `pytest tests/parity` 2,583
passed, `tables --check`.

### At the merge (orchestrator)
- The orphaned `bet2` of the FSL version trial (running since 10:06, one CPU) stopped.
- The handoff integrated: the register (FSL-64 row and † section, SCI-03 row, section, done
  table, history), the public defect page, licensing, `UPSTREAM.md`, `upstream.tsv`, CLAUDE.md
  (rule 2, the resampler and FSL rows, the Python environments, Next), next-steps (part 3 done,
  two follow-ups done, four added), the decision log, triage (the near-zero entry extended, the
  BET timeouts explained), golden.md, `validation/tools.toml` (generated tables re-made).
