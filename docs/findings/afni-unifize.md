# Findings: AFNI 3dUnifize 25.2.09

**Restricted** (CLAUDE.md rule 1): written from AFNI's source while translating the replica
(`crates-gpl/larmorx-gpl-afni`: `unifize`, `cli::unifize`, `glibc_floatmath`). Whoever writes
the clean-room original of 3dUnifize must not read this file; the behaviour it needs is in
`specs/3dUnifize.md`. References are `file:line` in `tags/AFNI-25.2.09/src`. The automask and
clip level 3dUnifize uses are 3dAutomask's: `docs/findings/afni-automask.md`.

## The licence of every routine that shapes the result (2026-10-23)

`3dUnifize.c` carries no MCW header (NIH, public domain), nor do `mri_invert.c`,
`thd_dset_nbhd.c`, `thd_median.c`, `thd_dsetto3D.c`, `thd_automask.c` and `thd_cliplevel.c`.
Its results also depend on:

| Routine | File | Licence | What it decides |
|---|---|---|---|
| `main`, `mri_local_percmean`, `mri_double_down`, `median7`, `mri_double_up`, `mri_WMunifize`, `mri_GMunifize`, `mri_clipedges_inplace` | `3dUnifize.c` | NIH, public domain | the program, the local white-matter intensity, the scalings |
| `mri_invertcontrast_inplace` | `mri_invert.c` | NIH, public domain | `-T2` |
| `mri_get_nbhd_array`, `SetSearchAboutMaskedVoxel` | `thd_dset_nbhd.c` | NIH, public domain | the ball's values |
| `THD_extract_float_brick` | `thd_dsetto3D.c` | NIH, public domain | the float input |
| `THD_median_brick` | `thd_median.c` | NIH, public domain | `-EPI`'s median volume |
| the automask and clip level | `thd_automask.c`, `thd_cliplevel.c` | NIH, public domain | the masks, `-GM`'s lower bound |
| `MCW_spheremask`, `MCW_build_mask` | `edt_buildmask.c` | **MCW, GPL-2** | the ball |
| `qsort_float` | `mri_percents.c` (with `cs_qsort_small.h`) | **MCW, GPL-2** | the sorted ball values, the inversion's percentile |
| `qmed_float` and its special cases | `cs_qmed.c` | **MCW, GPL-2** | `-GM`'s medians, `-EPI`'s median |
| `THD_extract_array` | `thd_dsetto1D.c` | **MCW, GPL-2** | `-EPI`'s series (AFNI-01) |
| `mri_to_float`, `mri_max`, `mri_cut_3D`, `THD_countmask`, `thd_floatscan`, `mri_new_*` | as for 3dAutomask | **MCW, GPL-2** | conversions, the automask |
| `tanhf`, `cbrtf` | glibc 2.35 `s_tanhf.c` (fdlibm), `s_expm1f.c` (fdlibm), `s_cbrtf.c` | LGPL-2.1+ (glibc; fdlibm's notice) | the squash, `-EPI`'s radius |

MCW code shapes the result, so **3dUnifize needs a GPL replica and a clean-room original**
(`docs/licensing.md`): the replica is in `crates-gpl/larmorx-gpl-afni` (its glibc port in the
same GPL crate, as 3dTshift's `sinf`/`cosf`), and `specs/3dUnifize.md` is written for the
original.

## How AFNI computes it

- **`USE_ALL_VALS` is defined as 1** (`3dUnifize.c:4`): the "ignore tiny values" code of May
  2016 (`vbot`) is compiled out, and `-useall` only prints that it is disabled (`:1003-1006`).
- **The local percentile mean** (`:361-541`): the automask of the input (`mri_automask_image`
  with `clfrac` 0.2 by default and the exterior clip on, `:938-939`, `:1098`), the masked copy
  (`-amsave` written right there, `:393-403`), the half-size copy only above 1,000,000 voxels
  (`:557`), the ball from `MCW_spheremask(1,1,1, 0.5*vrad+0.001)` (`:425`), and
  `SetSearchAboutMaskedVoxel(1)`: every voxel gets a value, the ball's values being the masked
  (non-zero) ones (`thd_dset_nbhd.c:139-161`). The OpenMP loop (`:482-524`, compiled with
  `-fopenmp -DUSE_OMP`) computes each voxel independently: the result does not depend on the
  thread count (verified: 1, 2, 6 threads bit-identical on every real case). Its progress dots
  are printed for `vvv % 66666 == 0` among voxels with two values or more (`:517`), from any
  thread; their count is deterministic.
- **The sort**: `qsort_float` of finite values (the ball holds no non-finite value: the input is
  scanned) gives the sorted array; equal values are equal, and −0/+0 cannot change a float sum
  that starts at +0. The replica selects the values of ranks `q1..q2` and sorts only those
  (`unifize::percmean`), and sums them in ascending order as AFNI does: the same bits with less
  sorting (no benchmark was run).
- **The squash** (`:573-575`) calls the C library's `tanhf`: glibc 2.35's fdlibm `tanhf`
  (through `expm1f`), not correctly rounded on 118,674,314 of the 2^32 float inputs. The replica
  carries a port (`glibc_floatmath`), whose results hash to glibc's over all 2^32 inputs
  (`tools/floatmath-verify`). `-EPI`'s `cbrtf` (`:1064`) likewise (453,492,162 inputs not
  correctly rounded; also ported and verified). Note: in Rust, `f32::cbrt` is the standard
  library's own correctly rounded cube root, not glibc's (found while verifying).
- **`mri_invertcontrast_inplace`** (`mri_invert.c:10-100`) recomputes the percentile `ucut` from
  the image it inverts: the re-inversion of `-T2` (`3dUnifize.c:1195`) uses the WM-scaled
  image's percentile with the input's mask; `-EPI` inverts each volume twice (`:1159`, `:1163`),
  each time from that volume's values.
- **`-EPI`** (`:1060-1180`): the radius from `cbrtf(fabsf(dx*dy*dz))` (the axes' deltas),
  `18.3 / uu` in double; the median by `THD_median_brick` (`thd_median.c:9-41`), which reads
  series through `THD_extract_array` (positive factors only: AFNI-01); the per-volume float
  bricks through `THD_extract_float_brick` (any factor but 0/1). The output (`EDIT_empty_copy`
  with `ADN_datum_all` float and `ADN_brick_fac NULL`) keeps the time axis; no `-GM`, no final
  mask, no elapsed time; the dots of `iv%100 == 47` are printed whatever `verb` is (`:1151`).
- **The final mask** (08 Aug 2018, `:1200-1207`) is the automask of the unifized image.
- **The output's brick factor**: `EDIT_empty_copy` with `ADN_nvals 1` rebuilds the bricks, which
  clears the factors (`edt_dsetitems.c:762-773`): the output's `scl_slope` is 0 whatever the
  input's (verified on int16 with slopes 1 and 2).

## Defects (register entries AFNI-45 to AFNI-49; AFNI-01 and AFNI-40 extended)

- **AFNI-45 (a)** `mri_invert.c:44`: `ii = (int)rintf(nhist*uperc*0.01f); ucut = hist[ii]` reads
  `hist[nhist]` when the index rounds to `nhist`: at `-T2up 100` always, and for `u` near 100 when
  `nhist (1 - u/100) < 0.5` (e.g. `-T2up 99.99` with fewer than 5000 mask voxels). `hist` is a
  `malloc` of `nvox` floats: the value is whatever that memory holds, or past the allocation when
  the whole image is in the mask. In AFNI's runs it read 0 (a fresh allocation of a large block
  comes zeroed from `mmap`), which inverts every value to 0 and zeroes the output (verified).
  The default uses the largest sorted value (the 100th percentile); `reproduce` uses 0 (the
  deterministic fill of D13 for memory-dependent values), both named.
- **AFNI-46 (a)** `3dUnifize.c:1105-1106`, `mri_invert.c:28-34`: with `-T2`, the automask of a
  2D image smaller than 16 × 16 is NULL (`thd_automask.c:2104`), and
  `mri_invertcontrast_inplace` computes it again (NULL again) and dereferences it: a crash
  (verified: signal 11, exit 1). The replica refuses in both modes, after AFNI's messages (it
  starts a new line, as AFNI's crash banner does).
- **AFNI-47 (a)** `3dUnifize.c:166-168`, `:227-229`, `:532`: halving keeps at least 1 voxel per
  axis and doubling keeps an axis of 1 at 1, so an axis of 2 or 3 voxels comes back as 1 (in a
  volume of more than a million voxels, where the half size is used). `mri_WMunifize` then reads
  the smaller scale image `par[ii]` with the input's indices (`:570-576`), past its end, and
  `-ssave` writes it the same way. What AFNI reads there depends on the memory after the
  allocation: on the 720 × 720 × 2 phantom of the parity case `defects/slab` (the scale image
  720 × 720 × 1, the second slice read past it) AFNI crashed with signal 11 in every run of
  2026-10-23 (two threads); with other memory it would multiply the second slice by whatever it
  finds. The default expands the half grid to the input's (`unifize::double_up` with the target
  grid); `reproduce` reads AFNI's smaller image linearly with 0 past its end (D13's
  deterministic fill); both named. The parity case compares AFNI's messages before the crash
  (or, when AFNI does not crash, the header and the values that do not come from that memory)
  and is an expected divergence.
- **AFNI-48 (questionable)** `3dUnifize.c:1193-1198`: `do_T2 == 1` re-inverts, `do_T2 == 2`
  clips edges, and a third `-T2` does neither, so the output stays inverted (verified). The help
  documents once and twice only. Kept as AFNI's until decided.
- **AFNI-49 (f)** `3dUnifize.c:818-822`: the help gives `-clfrac`'s default as 0.1 and then
  says it was raised to 0.2 in 2016; the code uses 0.2 (`:659`). Text only.
- **AFNI-40, extended** `3dUnifize.c:1036`: `-T2up` reads `argv[++iarg]` without the check every
  other option makes; as the last argument it crashes (verified). Refused in both modes.
- **AFNI-01, extended** (`thd_median.c:35-38` through `thd_dsetto1D.c:187-191`): `-EPI`'s median
  drops a negative factor, so a series stored with a negative `scl_slope` gets a median of the
  wrong sign, while each volume is scaled with the factor: the output is all zeros (verified).
  Corrected by default, named whenever a negative factor is met.
- AFNI-02 (float32 with a slope and an intercept, scaled twice) changes the unifized values only
  by rounding: the scaling divides out, exactly for a slope that is a power of two (verified:
  slope 2 gives the same bits). The default scales once and names it.

## Environment variables

The `getenv` trace (9 runs) found 73 names: 3dAutomask's, and the OpenMP runtime's (`OMP_*`,
`GOMP_*`, `ACC_*`, read by libgomp at start-up; `OMP_NUM_THREADS` also by `AFNI_SETUP_OMP`,
`Aomp.h:47-54`, before the global options: a `-D` cannot set it). None changes a result. The
replica's table: `environ::UNIFIZE`.

## Verification

- `docs/validation/afni-unifize.md`: the parity record against the AFNI 25.2.09 binary.
- The golden cases of `crates-gpl/larmorx-gpl-cli/tests/golden.rs` (`3dUnifize.*`).
