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

## Results (2026-11-13, seed 7, every group again after relay 4; the probe's reference hashes are the spec's §23.2)

`compare.py` per group: calls, identical, FreeSurfer did not finish, memory, defect corrected,
unexplained.

| Group | Calls | reproduce | fix |
|---|---|---|---|
| hmake | 632 | 628 / 4 / 0 / 0 / **0** | 482 / 4 / 0 / 146 / **0** |
| harith | 4,364 | 4,339 / 25 / 0 / 0 / **0** | 4,152 / 25 / 0 / 187 / **0** |
| hsmooth | 450 | 450 / 0 / 0 / 0 / **0** | 450 / 0 / 0 / 0 / **0** |
| hsearch | 3,360 | 3,333 / 0 / 27 / 0 / **0** | 3,328 / 0 / 27 / 5 / **0** |
| hstats | 1,651 | 1,651 / 0 / 0 / 0 / **0** | 1,539 / 0 / 0 / 112 / **0** |
| hrobust | 74 | 70 / 2 / 2 / 0 / **0** | 68 / 2 / 3 / 1 / **0** |
| hfiles | 62 | 62 / 0 / 0 / 0 / **0** | 62 / 0 / 0 / 0 / **0** |
| mmake | 356 | 354 / 2 / 0 / 0 / **0** | 278 / 2 / 0 / 76 / **0** |
| mproduct | 1,202 | 1,200 / 2 / 0 / 0 / **0** | 900 / 2 / 18 / 282 / **0** |
| affine | 243 | 241 / 2 / 0 / 0 / **0** | 172 / 2 / 0 / 69 / **0** |
| dmatrix | 2,904 | 2,895 / 9 / 0 / 0 / **0** | 2,878 / 9 / 0 / 17 / **0** |
| melements | 3,500 | 3,469 / 31 / 0 / 0 / **0** | 2,796 / 31 / 0 / 673 / **0** |
| mregions | 880 | 871 / 3 / 6 / 0 / **0** | 69 / 3 / 80 / 728 / **0** |
| mvectors | 1,602 | 1,430 / 172 / 0 / 0 / **0** | 1,382 / 172 / 0 / 48 / **0** |
| mprint | 363 | 363 / 0 / 0 / 0 / **0** | 360 / 0 / 0 / 3 / **0** |
| minverse | 862 | 818 / 10 / 0 / 0 / **34** | 774 / 10 / 0 / 51 / **27** |
| primes | 215 | 215 / 0 / 0 / 0 / **0** | 213 / 0 / 2 / 0 / **0** |
| draws | 6 | 6 / 0 / 0 / 0 / **0** | 6 / 0 / 0 / 0 / **0** |
| nr | 5 | 5 / 0 / 0 / 0 / **0** | 5 / 0 / 0 / 0 / **0** |
| eigen | 1,224 | 996 / 75 / 148 / 0 / **5** | 996 / 75 / 148 / 5 / **0** |
| svd | 1,920 | 1,514 / 333 / 65 / 0 / **8** | 1,088 / 333 / 95 / 404 / **0** |
| mfiles | 635 | 634 / 0 / 1 / 0 / **0** | 359 / 0 / 1 / 275 / **0** |
| rfields | 16 | 16 / 0 / 0 / 0 / **0** | 16 / 0 / 0 / 0 / **0** |
| minimizers | 181 | 179 / 2 / 0 / 0 / **0** | 160 / 2 / 9 / 10 / **0** |

Totals: `reproduce` 25,739 of 26,707 identical (672 not finished, 249 memory, 47 below); `fix`
22,533 identical (672 not finished, 383 memory, 3,092 corrected by a noted defect, 27 below).

The differences left, all understood:

- **minverse, 34 (reproduce) and 27 (fix):** every value identical; the lines differ only in the
  bits of NaNs (the sign of a default NaN, which input NaN survives) where NaN inputs go through
  VNL's routines in float (`vnl_det`/`vnl_inverse` of 2 to 4 rows, `vnl_qr<float>`): larmorx-core's
  ports evaluate the C++ source's operand order, the official build (GCC 8.4.0) another for some
  commutative operations. The spec (§8.8) names the main package's ports, without an operand
  order of its own.
- **eigen, 5, and svd, 8 (reproduce only):** FS-47 and FS-48 refuse, in both modes as the spec says,
  matrices with infinite or NaN entries that VNL usually never finishes, "the rare such matrices
  FreeSurfer does finish refused too" (a 2 x 2 with an infinity, the 1 x 1 infinity of
  `MatrixNSConditionNumber`'s product, two pseudo-inverses of NaN matrices): the candidate notes the
  defect, which `compare.py` excuses in `fix` only.

Operand orders measured on the probe where the spec gives none (which NaN a result keeps; the
fits are scripts of the comparison's scratch folder, reproduced in the findings):
`AffineMV` (`A[i] x0`, `x1 A[4+i]`, `x2 A[8+i]`, `A[12+i] x3`), `AffineMM` (`B[k+4j] A[i+4k]`, summed
left to right), `MatrixMultiplyD`'s complex sums (the term first), `VectorTripleProduct`; the peak
searches take a bin when `!(count <= pct × largest)` (a NaN count is a candidate), `HISTOinit`'s bin
size `<= 0` becomes 1 (a NaN stays), `HISTOfindLinearFit` starts at (a, b) = (1, 0),
`HISTOfindCurrentPeak` clips a start beyond the last bin, `hp.plt` is Z's bins with the fitted curve;
and FS-49's `reproduce`: FreeSurfer reads each number of a binary matrix file into one stack slot it
byte-swaps in place, so a short read keeps the previous number's other bytes, swapped (not zeros).

## Golden cases, defect tests, speed

- `cargo test -p larmorx-fs-core --test golden_maths`: every command of the probe's grammar on seeded
  arguments, both modes, 16 SHA-256 (record: `FS_CORE_GOLDEN_RECORD=1 ... -- --nocapture`;
  `FS_CORE_GOLDEN_DUMP=<folder>` writes each case's bytes). Identical in debug and release builds
  and under the AArch64 NaN emulation (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, built
  as `libnan_trap.so`; run the test binary with `LD_PRELOAD=<scratch>/libnan_trap.so NAN_TRAP_ARM=1
  NAN_TRAP_DIR=<scratch>/nt`). The inverse, determinant, QR factors and `OpenEigenSystem` /
  `OpenNonSymmetricEigenSystem` get finite arguments: VNL's ports in the main package do not yet
  give the official build's NaN bits there (the findings).
- `cargo test -p larmorx-fs-core --test defects_maths`: each defect of the spec's §24 noted in both
  modes on a line of the comparison's own batches (the shortest), the two modes' results different
  where the correction changes them; FS-29, FS-35 and FS-36 (kept, not noted), FS-39 at the driver.
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
