# Findings: FreeSurfer 8.2.0's `mri_coreg` (replica, queue item 3.2-17)

Closed to FreeSurfer clean-room implementers (written from FreeSurfer's source,
`mri_coreg/mri_coreg.cpp`, `utils/randomfields.cpp`, `utils/transform.cpp`). Track C, relay 52.

## 1. What the program does, as the source has it

- `COREGhist` samples the reference every `sep` voxels in 32 chunks of columns (OpenMP over the
  chunks, each its own histogram, the chunks summed in chunk order: the source's comment, "to get
  deterministic results independent of the number of threads"). The replica runs the chunks on the
  run's threads and sums them the same way; each chunk keeps only the bins it touched (a bin no
  sample reached adds `+0`, which changes no sum: the golden digest is the same as with dense
  histograms).
- The coordinate dither is `sep * dither` computed in `float` (`int * float`), then added to the
  `double` coordinate. The coordinate dither and the reference's intensity dither come from two
  generators seeded alike (`RFspecInit(coreg->seed)` each), so they draw the same stream; the
  moving volume's field (`movirfs`) is named `uniform` but its parameters stay `calloc`'s 0: every
  draw is `flat(0, 0) = 0`, so the moving volume is never intensity-dithered (FreeSurfer's
  behaviour, kept: rule C, a deliberate default the official results rely on; the draws are made).
  Only `sc_ran_flat` is reached: larmorx-core's `ranlux` clean room (`Ranlux::flat`) gives
  FreeSurfer's draws on every recorded head and probe case (no gap for the spec writer).
- The masks: `int m = MRIgetVoxVal(...)`; `m < 0.5` masks the voxel: any value below 1 (0.5 in a
  float mask) masks it out, as the integer truncation gives (rule C: kept).
- `COREGoptBruteForce` keeps one minimum (`mincost = 10e10`) over all six parameters, not per
  parameter; the samples `p` from `pmin` while `p <= pmax`, `p += pdelta` accumulated (30 or 31
  steps by rounding).
- `OpenPowell2(..., coreg->nitersmax, ...)` passes the iteration limit as fs_powell's `maxfev`
  (`numerics.cpp`), so `--nitersmax 4` is four of fs_powell's iterations (its own loop count).
- "Final matrix parameters" prints `COREGoptSchema2MatrixPar(coreg, NULL)`: the schema's parameters
  placed into a `calloc`'d array, so the scales not optimised print `0.0000` (not 1).
- `COREGcostPowell`'s statics (`initcost`, `mincost`, `ppmin`) persist across separations; each
  Powell call resets them at its first evaluation (`startmin`).
- `--diag-show` and `--diag-verbose` assign `Gdiag & flag` (they can only clear bits).

## 2. Defects (rules A to D; register rows in the relay's HANDOFF)

- **FS-1313** (class a, memory): `seplist[10]` in `CMDARGS` and `COREG`. The 11th `--sep` lands in
  `CMDARGS`'s padding before `DoInitCostOnly`; copying it into `COREG` writes `coreg->seplist[10]`,
  which is `coreg->nsep` (`int seplist[10], nsep`): measured, 11 separations run as 4 when the
  11th is 4 (official exit 0). Layout-dependent: an error in both modes.
- **FS-1314** (class a): `--threads abc` passes an uninitialized `int` to `omp_set_num_threads`.
- **FS-1315** (class a): `fsenv->SUBJECTS_DIR` with `fsenv` `NULL` (`FSENVgetenv` fails without
  `FREESURFER_HOME` or its colour table) crashes (measured: `CRASH 11`, its buffered standard output
  lost); `sprintf(tmpstr[3000], ...)` overflows.
- **FS-1316** (class a): `double par[12]` (`--par2mat`) and `RMSDiffRad` (`--rms`) are not
  initialized; a value `sscanf` does not read prints stack contents (measured 0 here).
- **FS-1317** (class a): `--rms` checks `nargc < 2` and reads `pargv[2]`, `pargv[3]` (the `NULL`
  terminator for 2 or 3 arguments: `LTAread(NULL)`); `fopen` of its file unchecked.
- **FS-1318** (class a): `--lrrev` with no `L`/`R` letter leaves `k = -1`: `K2->rptr[-1][1]`
  (the row-pointer array's malloc header read as a pointer): an error in both modes (not measured).
- **FS-1319** (class a, hang): `--bf-lim 0` (step 0), a negative `--bf-nsamp` (negative step), or a
  step below the parameter's precision: `p <= pmax` forever (measured: the official program did
  not end in 600 s).
- **FS-1320** (class a, memory): `MRIgetPercentile`'s `n = round(nvox * Pct / 100)` reaches `nvox`
  (`--sat 100`; the default 99.99 for `nvox < 5000`), and `n >= nvox - 1` reads `vect[n]`, one past
  the `calloc`'d array: for `nvox * 4 % 16 == 0` that is the chunk's zeroed slack (0.0; every
  measured case gave 0, the port's `reproduce`), otherwise the next chunk's size field. Rule A: the
  last value by default.
- **FS-1321** (class a): NaN coordinates (NaN parameters through `--rot nan ...`) make `floor`/`ceil`
  `INT_MIN` indices: a read outside the byte vectors; an error in both modes (not measured).
- **FS-1322** (class a, memory): `--dof` above 12 with schema 1 reads `cmdargs->params[12..]` (the
  bytes of `nsep` and `seplist`) and writes `coreg->params[12..]` (`nparams` and `mparams`): the
  official run ended with status 0 on our case, its result memory-dependent. An error in both modes.
- **FS-1323** (class a): `--sep 0`: `(width + sep - 1) / sep` divides by zero at the separation's
  first histogram (measured: `CRASH 8`, SIGFPE, after the earlier separations' output; reproduced
  with `reproduce`). A negative separation: the chunk sizes are negative or 0; where every chunk is
  empty (a reference of width 32: `chunkSize = 0`) no sample is taken and the program runs
  (reproduced); where a chunk is not empty its loop steps down forever.
- **FS-1324** (class a): `fopen` unchecked for `--init-cost-only`, `--params`, `--final-cost`
  (measured: `CRASH 11`; standard output after the last `MatrixPrint` lost).
- **FS-1325** (class a): `MatrixInverse(V2V)` returns `NULL` for a singular matrix and `LTAcreate`
  copies it (`MatrixCopy(NULL)`); `MRIconformNoScale` uses `MRIresample`'s result unchecked: an error
  in both modes (not measured).

- **FS-1326** (class a): `ReadSTVPairFile`'s `int sno, tno; double sx, ...` are declared once
  before the loop: a number `sscanf` does not read keeps the previous line's value, and on the
  first line memory never written (the official output of the probe's `part.txt` case changed from
  run to run: once VNL's `suspicious return value (2) from SVDC` with a NaN matrix, once not).
- **FS-1327** (class a): `ComputeReg` returns `NULL` for a singular `XtX`; `ComputeLTA` goes on:
  `TKR` `MatrixInverse(NULL, NULL)` (`ErrorExit`, status 252), `VOX` `LTAchangeType`'s
  `MatrixMultiply(NULL, ...)` (`ErrorExit`, status 252), `RAS` `LTAwrite` opens the file and crashes
  on the matrix (measured: an empty `lm.lta`, `CRASH 11`).
  Reproduced so with `reproduce`; a clear error by default.
- `ReadCoords`: `sxyzfile.find("json") > 0 && txyzfile.find("json") > 0` is true unless a name
  *starts* with `json` (`find` returns `npos` for a name without it): any name not starting with
  `json` is read as a JSON point set (rule C: kept; the probe's `json1` case).

## 3. Standard output at a crash

FreeSurfer's `stdout` is fully buffered (4,096 bytes) when redirected; `MatrixPrint`, the dither
lines' `fflush(stdout)`, `COREGpreproc`'s two, `COREGcostPowell`'s, `COREGMinPowell`'s after
`nEvals`, the brute force's and the C++ error stream's messages flush it. A crash loses the rest
(probe case 96: `--params nodir/par.txt` lost "Final matrix parameters" and "Final opt parameters").
The replica keeps standard output in `mri_mask`'s `Cout` and, at a crash's status with `reproduce`,
writes only what FreeSurfer's buffer had written out.

## 4. Speed and memory (relay 52, one thread)

On the recorded heads (fMRIPrep's call): the replica 50-59 s against FreeSurfer's 31-39 s (about
1.5 times), peaks 344-365 MiB against 413-433 MiB. Where the time goes has not been measured yet
(no profiler here): candidates are the smoothing of the 256^3 reference (`MRIgaussianSmoothNI`,
fs.filter), the sampling (`COREGsamp`'s eight corners with checked indices) and the percentiles'
sort of 16.7 million values; a speed relay, bit for bit, later.
