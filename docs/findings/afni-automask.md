# Findings: AFNI 3dAutomask 25.2.09 (and the library routines it shares with 3dUnifize)

**Restricted** (CLAUDE.md rule 1): written from AFNI's source while translating the replica
(`crates-gpl/larmorx-gpl-afni`: `mask`, `automask`, `cli::automask`). Whoever writes the
clean-room original of 3dAutomask must not read this file; the behaviour it needs is in
`specs/3dAutomask.md`. References are `file:line` in the pinned release
(`tags/AFNI-25.2.09/src`).

## The licence of every routine that shapes the result (2026-10-23)

`3dAutomask.c`, `thd_automask.c` and `thd_cliplevel.c` carry no Medical College of Wisconsin
(MCW) header: NIH, public domain. Their results depend on these routines (dataset reading and
writing aside, which larmorx has of its own):

| Routine | File | Licence | What it decides |
|---|---|---|---|
| `main` (options, `-dilate`/`-erode`, planes, `-SI`, outputs) | `3dAutomask.c` | NIH, public domain | the program |
| `THD_automask`, `mri_automask_image`, `mri_automask_image2D`, `THD_mask_clust`, `THD_mask_clust2D`, `THD_mask_erodemany`, `THD_mask_erodemany2D`, `THD_mask_erode`, `THD_mask_dilate`, `THD_mask_fillin_once`, `THD_mask_fillin_completely`, `THD_mask_clip_neighbors`, `THD_mask_depth`, `thd_apply_mask`, `mask_count` | `thd_automask.c` | NIH, public domain | the mask |
| `THD_cliplevel`, `THD_cliplevel_partial`, `get_octant_clips`, `pointclip`, `THD_cliplevel_gradual` | `thd_cliplevel.c` | NIH, public domain | the clip levels |
| `THD_aveabs_brick` | `thd_median.c` | NIH, public domain | the averaged image |
| `mri_get_cmass_3D` | `mri_get_cmass.c` | NIH, public domain | the octants' centre |
| `THD_extract_array` | `thd_dsetto1D.c` | **MCW, GPL-2** | a voxel's series, its scaling (AFNI-01) |
| `mri_to_float`, `mri_scale_to_float` | `mri_to_float.c` | **MCW, GPL-2** | the float image of one sub-brick, its scaling and float scan |
| `mri_max` | `mri_max.c` | **MCW, GPL-2** | the histogram's top |
| `mri_cut_3D` | `mri_cut.c` | **MCW, GPL-2** | the octants' sub-volumes |
| `THD_countmask` | `thd_makemask.c` | **MCW, GPL-2** | the voxel counts printed (and the "No voxels" test) |
| `thd_floatscan` | `thd_floatscan.c` | **MCW, GPL-2** | non-finite values set to 0 |
| `THD_need_brick_factor` | `thd_initdblk.c` | **MCW, GPL-2** | whether factors are applied |
| `mri_new_vol`, `mri_new_conforming` | `mri_new.c` | **MCW, GPL-2** | zero-filled images |
| `INFO_message`, `ININFO_message`, `WARNING_message`, `ERROR_exit` | `debugtrace.c` | NIH, public domain | messages |
| `AFNI_prefilter_args`, `AFNI_yesenv`, ..., `THD_filename_ok`, `THD_write_3dim_dataset` | `afni_environ.c`, `thd_filestuff.c`, `thd_writedset.c` | **MCW, GPL-2** | global options, names, existing outputs |

So MCW code shapes the result (mostly elementary: a maximum, a copy of a box, counts, the
float conversion with its scaling), and **3dAutomask needs a GPL replica and a clean-room
original** (`docs/licensing.md`): the replica is in `crates-gpl/larmorx-gpl-afni`, and
`specs/3dAutomask.md` is written for the original.

## How AFNI computes the mask

- **The average** (`thd_median.c:273-308`): one sub-brick through `mri_scale_to_float` (any
  factor but 0 and 1, then `mri_floatscan`), several through `THD_extract_array` per voxel
  (`thd_dsetto1D.c:72-195`: float scan first, then only **positive** factors, `:187-191`);
  `fac = 1.0 / nvals` is stored in a float.
- **The clip level** (`thd_cliplevel.c:22-130`): 10001 bins, but both loops stop at bin 9999
  (`:108`, `:116`, `:118`): the maximum's bin is never counted. A two-level image (1000 voxels of
  100 and 1000 of 10) therefore gets a level of 5, below both levels (unit test
  `mask::tests::cliplevel_of_two_levels`). Real images have a spread of values and do not show
  it. The initial search can run to index -1 only with more than 10^8 positive voxels; the
  replica counts that bin as empty.
- **The gradual level** (`:195-331`): the octant boxes overlap by `rint(0.01 n)` voxels; each
  box's level comes from `THD_cliplevel` on a copy of the box (`mri_cut_3D`); levels below a
  third of the global one are raised to it.
- **The mask** (`thd_automask.c:295-441`): threshold, `THD_mask_clust` (6-connected; the
  first-found largest cluster wins ties, `:1015`), `THD_mask_erodemany` with the peel settings
  (`:1323-1421`; the threshold is `MIN(18, peelthr)`, `:1335`, "until allowing for NN1,3"),
  `THD_mask_clust`, up to three `THD_mask_fillin_once(1)` (`:371-377`), the large-hole fill
  (`:388-400`), one more peel and cluster, the outside's cluster, `-eclip`'s
  `THD_mask_clip_neighbors` (in place: a voxel added in a sweep counts for later voxels, `:473`),
  and a last peel when it removed anything.
- **The 2D path** (`:2095-2132`): its own peeling (3 layers, threshold 6 of 8, `:2016`) and
  clustering (`THD_mask_clust2D`, 4-connected, keeping clusters of at least `kfrac` times the
  largest).
- **`-dilate`/`-erode`** (`3dAutomask.c:310-337`): `THD_mask_dilate(..., 3, NN)` — at least 3
  neighbours, not 1 — and `THD_mask_erode(..., 0, NN)`, which erodes every border voxel
  (`thd_automask.c:1076-1078`), each followed by `THD_mask_fillin_completely` with
  `max(1, rint(0.032 n))`.
- **`-depth`** is computed from the mask before `-dilate`, `-erode` and `-SI` (`3dAutomask.c:292-299`).
- **`-apply_prefix`** (`thd_automask.c:133-224`) copies the input's data type and brick
  factors; a factor of 1 is copied too, and 3dAutomask's dataset (not a copy) gets one from
  `THD_load_nifti` (`thd_niftiread.c:1095-1100`, "pass slope as brick_fac", also for slope 1),
  so an int16 input with `scl_slope 1` gives a masked output with `scl_slope 1` (verified).
- **CPU time**: `COX_cpu_time()` (`thd_iochan.c:1776-1787`) returns 0 when `CLK_TCK` is not
  defined, which it is not with `_GNU_SOURCE` on glibc: AFNI on Linux always prints "CPU time =
  0.000000 sec" (verified); the replica prints the same.

## Defects (register entries AFNI-40 to AFNI-44; AFNI-01 extended)

- **AFNI-40 (a)** `3dAutomask.c:139-245`: the options `-peels`, `-nbhrs`, `-clfrac`/`-mfrac`,
  `-SI`, `-dilate`, `-erode` read `argv[++iarg]` without checking it exists; as the last argument
  `strtod(NULL)` or `strtol(NULL)` crashes (signal 11, AFNI's crash banner, exit 1; verified for
  each). `-prefix`, `-apply_prefix` and `-depth` pass the NULL to `THD_filename_ok`, which
  refuses it ("-prefix (null) is illegal!", exit 1). The replica refuses the first group with
  one fatal line in both modes, after the messages AFNI prints before the crash (the author
  line); the parity's crash cases check exactly that.
- **AFNI-41 (questionable; decided: rule B)** `3dAutomask.c:203-206, 276-283`: `-nbhrs` sets `nbhrs` directly;
  the range check reads `new_nbhrs`, which nothing sets, so it never runs, and
  `THD_automask_set_peelcounts` (`thd_automask.c:42-46`) silently replaces values outside 6..26
  by 17; values 19..26 act as 18 (`MIN(18, peelthr)`). The help says "should be between 6 and
  26"; the dead check would have refused values outside 1..max-1. **Decided 2026-10-23, rule B**
  (the input makes no sense and AFNI silently does something arbitrary): the replica's default
  refuses the last `-nbhrs` value given when it is outside 6..26, the help's range, whether or
  not `-NN1/2/3` override it (AFNI's dead check sits after the `-NNx` override but reads the
  `-nbhrs` value itself), right after the option scan: the notice line `*+ WARNING: upstream
  defect AFNI-41 (3dAutomask) corrected: ...` and the fatal error `-nbhrs 40 is illegal: choose
  a value between 6 and 26 (AFNI-41; LARMORX_UPSTREAM_DEFECTS=reproduce gives AFNI's result)`,
  exit 1. `reproduce` runs as AFNI does and names AFNI-41 (the peel count AFNI uses: 17, or
  `min(18, n)` for `-NNx`'s). The range is the help's, not the dead check's 1..max-1: the
  documented contract, and the one the spec gives the clean-room original (`specs/3dAutomask.md`
  §5.3). *Validated* (parity cases `options/nbhrs-40`, `-5`, `-27`, `-0`, `nbhrs-40-then-NN1`:
  `reproduce` bit-identical to AFNI, the default refuses naming AFNI-41; unit tests).
- **AFNI-42 (questionable)** `3dAutomask.c:262-273`: `-NN1/2/3` also set `nbhrs` to 6, 18, 26
  ("keep historic default of 17 for NN2 unless changed via NNx or nbhrs options"), overriding a
  `-nbhrs` given before or after. The help says "the default peeling processes still use NN2"
  (true of the neighbourhood, not of the threshold): an explicit `-NN2` peels with 18 of 18
  neighbours where the default peels with 17, and `-NN1` with 6 of 18. Kept as AFNI's until
  decided.
- **AFNI-43 (c)** `thd_automask.c:44`: `peelcount = (p > 0) ? p : 1`, so `-peels 0` peels once;
  the help: "Turn off by setting to 0". Corrected by default (no peel), named when the mask
  differs; `reproduce` peels once.
- **AFNI-44 (questionable)** `thd_automask.c:388-393`: the large-hole step runs when `nmm > 1 ||
  jj > 0`, `jj` being the last assignment, `rint(0.016 nz)`: with no axis of 94 voxels or more,
  a volume with 32 to 93 slices runs `THD_mask_fillin_completely(1)` (the small holes, until
  none is left) and one with fewer than 32 slices does not. Whether `jj > 0` was meant is not
  clear; kept as AFNI's until decided.
- **AFNI-01 (c), extended**: `THD_extract_array`'s positive-factor test (`thd_dsetto1D.c:187-191`)
  drops a negative factor and its magnitude when several sub-bricks are averaged; the mask
  scales with the data and rarely changes, but the printed clip levels do. Corrected by default
  (the factor applied), named whenever it applies, with the number of mask voxels it changes.
- AFNI-02, -07, -11, -12, -16, -18, -20, -22, -23, -26: the dataset reading and writing and the
  output names 3dAutomask shares with 3dTshift (`docs/findings/afni-tshift.md`); handled by the
  same replica code. AFNI-23 (`AFNI_NIFTI_SCALE=NO`): the default reads the data as without the
  variable (float32, `slope * x + inter`), so the mask and the masked copy see the right values.

## Environment variables

Traced with an `LD_PRELOAD` library logging `getenv` (scratch, not kept) over 12 runs (options,
types, an existing output, `-overwrite`, `-D`, a missing input, refused names): 55 names, all
among 3dTshift's list (`environ::CONSULTED`) except none new; `AFNI_1D_ZERO_TEXT` is not read
(no slice-time file). `AFNI_QUIET_STARTUP` (read by `machdep`, `machdep.c:46`) hides the
author line and every `WROTE_DSET` line (`mrilib.h:1891-1904`). The replica's table:
`environ::AUTOMASK`.

## AFNI 26.3.05 (2026-10-29)

The replicas follow AFNI 26.3.05 since 2026-10-29 (decision log 2026-10-27, "Newest upstream
versions, updated in steps"). What changed between 25.2.09 and 26.3.05 in the code the three
programs run was found two ways (`validation/investigations/version-afni/`): the source of every
file `PROVENANCE.md` names diffed between the release trees, and every function of the three
binaries (both builds from GCC 11.4 at `-O2`) compared by machine code, which also catches a
changed macro or inlined routine in a file not listed. *Read* and *measured*.

| Change | Where | What it does to 3dAutomask |
|---|---|---|
| AFNI-03 fixed: header slice code 2 (sequential decreasing) visits `slice_end` down to `slice_start` (25.2.09's loop stopped at `slice_end`, so every slice time stayed 0) | `THD_open_nifti` (`thd_niftiread.c:652`) | the slice timing an output copies from a code-2 series (found by random testing 2026-10-25 for 3dUnifize): 26.3.05 keeps the header's times, as larmorx's correction did; the replica does so in both modes and names no defect |
| The MCW licence headers (GPL-2 → CC BY 4.0) | 27 of the named files (licence text only) | nothing ([afni-relicensing-26.md](afni-relicensing-26.md)) |
| `THD_mask_erode2D`, new | `thd_automask.c`, `3ddata.h` | nothing: called only by `mri_warp3D_align.c`, which none of the three programs links (it is in the 3dAutomask and 3dUnifize binaries because `thd_automask.o` is) |
| `isMacTahoe` in `machdep` | `machdep.c` | nothing on Linux (returns 0); a macOS version check |
| Five help-search routines (`approx_str_sort_all_popts`, `form_C_progopt_string_from_struct`, `get_updated_help_file`, `progopt_C_array`, `unescape_unix_str`) | `thd_getpathprogs.c`, `thd_ttatlas_query.c`, `thd_niml.c` | reached only by AFNI's `-h_*` and `-all_opts` options, which the replicas treat as unknown options (as with 25.2.09); `-help` itself is unchanged but for the line naming the build |
| New atlases in `AFNI_atlas_spaces.niml` (JulichBrain v3.1) | data | nothing: no template space and no generic space changed |

Every other function of the three binaries compiles to the same instructions in both builds
(3dTshift 3,280, 3dAutomask 3,382, 3dUnifize 3,412). The replicas' records show it case by case:
each case also runs AFNI 25.2.09 and names the cases whose AFNI output changed, with the reason
(`docs/validation/afni-automask-replica.md`, "AFNI's own step").

## Verification

- `docs/validation/afni-automask.md`: the parity record against the AFNI 25.2.09 binary.
- The golden cases of `crates-gpl/larmorx-gpl-cli/tests/golden.rs` (`3dAutomask.*`).
