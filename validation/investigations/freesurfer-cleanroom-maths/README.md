# fs.maths' clean room against FreeSurfer 8.2.0, as a black box

larmorx's clean room of FreeSurfer's mathematics (`crates/larmorx-fs-core/src/maths/`, written from
[`specs/freesurfer-maths.md`](../../../specs/freesurfer-maths.md) alone) compared with the prebuilt
probe of the open folder [`../freesurfer-maths-probe/`](../freesurfer-maths-probe/README.md)
(FreeSurfer 8.2.0's library as the official build compiles it). The probe is only run, never built.

| Path | What it is |
|---|---|
| `candidate/` | `fs-maths-candidate batch <file>`: the probe's commands and print forms over the clean room (its own Cargo workspace, a validation tool outside larmorx's build); each line on a process state of its own, `LARMORX_UPSTREAM_DEFECTS` the mode, the defect notices of a line on its standard error |
| `../../../crates/larmorx-fs-core/tests/maths_cmd/` | the commands themselves (the probe's grammar and print forms, the minimizers' four test costs and their FNV-1a hash), shared with the crate's golden cases (`tests/golden_maths.rs`); the candidate includes the folder by path |
| `speed.py` | the products, the inverse, the SVD and the histograms against the probe, black box (indicative) |

## Running it

```sh
(cd candidate && cargo build --release)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-maths-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe \
  --candidate <target>/release/fs-maths-candidate --scratch <scratch>/maths --mode reproduce   # then fix
```

## Results (2026-11-13, seed 7, every group again after relay 6: the spec as amended at `3a1ef09e`; the probe's reference hashes are the spec's §23.2)

`compare.py` per group: calls, identical, FreeSurfer did not finish, NaN bits only (§17.4), memory,
defect corrected, unexplained.

| Group | Calls | reproduce | fix |
|---|---|---|---|
| hmake | 632 | 628 / 4 / 0 / 0 / 0 / **0** | 482 / 4 / 0 / 0 / 146 / **0** |
| harith | 4,364 | 4,339 / 25 / 0 / 0 / 0 / **0** | 4,152 / 25 / 0 / 0 / 187 / **0** |
| hsmooth | 450 | 450 / 0 / 0 / 0 / 0 / **0** | 450 / 0 / 0 / 0 / 0 / **0** |
| hsearch | 3,360 | 3,333 / 0 / 0 / 27 / 0 / **0** | 3,328 / 0 / 0 / 27 / 5 / **0** |
| hstats | 1,651 | 1,651 / 0 / 0 / 0 / 0 / **0** | 1,539 / 0 / 0 / 0 / 112 / **0** |
| hrobust | 74 | 70 / 2 / 0 / 2 / 0 / **0** | 68 / 2 / 0 / 3 / 1 / **0** |
| hfiles | 62 | 62 / 0 / 0 / 0 / 0 / **0** | 62 / 0 / 0 / 0 / 0 / **0** |
| mmake | 1,100 | 702 / 324 / 0 / 74 / 0 / **0** | 374 / 324 / 0 / 74 / 328 / **0** |
| mproduct | 1,202 | 1,200 / 2 / 0 / 0 / 0 / **0** | 900 / 2 / 0 / 18 / 282 / **0** |
| affine | 276 | 274 / 2 / 0 / 0 / 0 / **0** | 172 / 2 / 0 / 0 / 102 / **0** |
| dmatrix | 2,904 | 2,895 / 9 / 0 / 0 / 0 / **0** | 2,878 / 9 / 0 / 0 / 17 / **0** |
| melements | 3,500 | 3,469 / 31 / 0 / 0 / 0 / **0** | 2,796 / 31 / 0 / 0 / 673 / **0** |
| mregions | 880 | 871 / 3 / 0 / 6 / 0 / **0** | 69 / 3 / 0 / 80 / 728 / **0** |
| mvectors | 1,602 | 1,430 / 172 / 0 / 0 / 0 / **0** | 1,382 / 172 / 0 / 0 / 48 / **0** |
| mprint | 363 | 363 / 0 / 0 / 0 / 0 / **0** | 360 / 0 / 0 / 0 / 3 / **0** |
| minverse | 862 | 852 / 10 / 0 / 0 / 0 / **0** | 801 / 10 / 0 / 0 / 51 / **0** |
| primes | 215 | 215 / 0 / 0 / 0 / 0 / **0** | 213 / 0 / 0 / 2 / 0 / **0** |
| draws | 6 | 6 / 0 / 0 / 0 / 0 / **0** | 6 / 0 / 0 / 0 / 0 / **0** |
| minimizers | 181 | 179 / 2 / 0 / 0 / 0 / **0** | 160 / 2 / 0 / 9 / 10 / **0** |
| nr | 5 | 5 / 0 / 0 / 0 / 0 / **0** | 5 / 0 / 0 / 0 / 0 / **0** |
| eigen | 1,460 | 1,151 / 161 / 0 / 148 / 0 / **0** | 1,088 / 161 / 0 / 148 / 63 / **0** |
| svd | 2,280 | 1,820 / 373 / 10 / 77 / 0 / **0** | 1,219 / 373 / 0 / 114 / 574 / **0** |
| mfiles | 723 | 695 / 1 / 0 / 27 / 0 / **0** | 378 / 1 / 0 / 27 / 317 / **0** |
| rfields | 16 | 16 / 0 / 0 / 0 / 0 / **0** | 16 / 0 / 0 / 0 / 0 / **0** |
| all | 28,168 | 26,676 / 1,121 / 10 / 361 / 0 / **0** | 22,898 / 1,121 / 0 / 502 / 3,647 / **0** |

Totals: `reproduce` 26,676 of 28,168 identical (1,121 not finished, 10 NaN bits only, 361 memory);
`fix` 22,898 identical (1,121 not finished, 502 memory, 3,647 corrected by a noted defect); none
unexplained in either mode.

The differences left, all understood:

- **svd, 10 (reproduce only): NaN bits only.** Decompositions of matrices with infinite or NaN entries
  that FreeSurfer finishes: U's NaNs keep other signs or payloads, because the main package's float
  SVD does not yet follow the official build's LINPACK kernels (the spec's §17.4 allows it).
- **Where FreeSurfer did not finish** (crashes, and in `eigen` and `svd` the matrices VNL never
  finishes): the candidate finishes, and in `eigen` and `svd` its standard error holds the spec's
  refusal on every such line (FS-47, FS-48: refused in both modes).
- Before relay 6: `eigen` 5 and `svd` 8 lines in `reproduce` (FS-47 and FS-48 refused every matrix
  with an infinite or NaN entry, the amended §17 gives VNL's result where it returns); before main
  `c9ab2a4e`: `minverse` 34 lines in `reproduce` and 27 in `fix`, NaN bits only (VNL's float routines,
  now in the official build's operand order).

What the amended spec changed and the runs confirmed (the findings): FS-13's sizes (`mmake-sizes`,
`mmake-sizes-old`), `AffineMV`'s operand order (`affine-nan`: the first fit differed on 6 lines), the
`errno` at the end of the input (`mfiles-end`: the first rule differed on 7 lines), FS-49's stack slot
(`mfiles-cut`), FS-47 and FS-48 (`eigen-nonfinite`, `svd-nonfinite`).

Operand orders measured on the probe where the spec gives none (which NaN a result keeps; the
fits are scripts of the comparison's scratch folder, reproduced in the findings):
`MatrixMultiplyD`'s complex sums (the term first), `VectorTripleProduct`; the peak searches take a
bin when `!(count <= pct × largest)` (a NaN count is a candidate), `HISTOinit`'s bin size `<= 0`
becomes 1 (a NaN stays), `HISTOfindLinearFit` starts at (a, b) = (1, 0), `HISTOfindCurrentPeak` clips
a start beyond the last bin, `hp.plt` is Z's bins with the fitted curve.

## Golden cases, defect tests, speed

- `cargo test -p larmorx-fs-core --test golden_maths`: every command of the probe's grammar on seeded
  arguments, both modes, 16 SHA-256 (record: `FS_CORE_GOLDEN_RECORD=1 ... -- --nocapture`;
  `FS_CORE_GOLDEN_DUMP=<folder>` writes each case's bytes). Identical in debug and release builds
  and under the AArch64 NaN emulation (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, built
  as `libnan_trap.so`; run the test binary with `LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1
  NAN_TRAP_DIR=<scratch>/nt`). `OpenEigenSystem` and `OpenNonSymmetricEigenSystem` get finite
  arguments: VNL's eigensystem port does not yet give the official build's NaN bits (the findings).
- `cargo test -p larmorx-fs-core --test defects_maths`: each defect of the spec's §24 noted in both
  modes on a line of the comparison's own batches (the shortest), the two modes' results different
  where the correction changes them (FS-13 also with FreeSurfer's matrices of a negative count, FS-47
  and FS-48 both where VNL never returns and where it does); FS-29, FS-35 and FS-36 (kept, not
  noted), FS-39 at the driver.
  Not covered: FS-30 (unreachable through `drand48`, the findings), FS-41, FS-42, ITK-111.
- `speed.py` (2026-11-13, one thread, indicative): products of 256 x 256 about 1.7 to 1.9 times the
  probe's speed, the inverse of 128 x 128 1.8 times, the SVD 1.5 times, `HISTOcount` 2.8 times;
  `HISTOsmooth` and `HISTOrobustGaussianFit` slower by under 1.5 ms a call. The table:
  [`docs/findings/fs-maths-clean-room.md`](../../../docs/findings/fs-maths-clean-room.md).

```sh
scripts/orchestration/boxed.sh 4 python -P validation/investigations/freesurfer-cleanroom-maths/speed.py \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe \
  --candidate <target>/release/fs-maths-candidate --scratch <scratch>/speed
```
