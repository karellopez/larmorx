# Handoff: AFNI 3dAutomask and 3dUnifize

The agent's text for the shared documents, verbatim; integrated and harmonized by the orchestrator at the merge.

# HANDOFF: 3dAutomask and 3dUnifize → the shared documents

Exact text for the orchestrator to put into the shared documents (CLAUDE.md, "Shared documents
through a handoff"). The branch is `worktree-agent-af5a27ad72e66b922` (larmorx) and
`afni-mask-unifize-evidence` (the site clone in `<scratch>/site`). The numbers come from
the records in the branch (last section, "Numbers").

Defect IDs used: AFNI-40 to AFNI-49 (all ten). `larmorx_core::defects` changed in the branch:
AFNI-43, AFNI-45, AFNI-47 added; the `tool` of the shared AFNI entries (AFNI-01, -02, -07, -08,
-11, -12, -16, -18, -20, -22, -23, -26) changed from "3dTshift" to "AFNI", since 3dAutomask and
3dUnifize name them too (notices now read `upstream defect AFNI-01 (AFNI) corrected: ...`;
the tests that matched "(3dTshift)" were updated).

---

## 1. `docs/licensing.md`

Replace the bullet "**AFNI: one implementation or two is decided per file.**" with:

```markdown
- **AFNI: one implementation or two is decided per routine.** AFNI's `LICENSE.txt`: code
  written at NIH is public domain; the "major portions" written at the Medical College of
  Wisconsin are GPL-2, marked at the top of each file. A program needs a GPL replica and a
  clean-room original when the code its results depend on comes from MCW files (dataset I/O
  aside: larmorx has its own); the first step of each AFNI port traces the call graph routine
  by routine. **Checked 2026-10-23 for 3dAutomask and 3dUnifize: both need the two
  implementations.** Their own files carry no MCW header (`3dAutomask.c`, `3dUnifize.c`,
  `thd_automask.c`, `thd_cliplevel.c`, `mri_invert.c`, `thd_dset_nbhd.c`, `thd_median.c`,
  `thd_dsetto3D.c`, `mri_get_cmass.c`: NIH, public domain), but their results depend on MCW
  routines:

  | MCW routine (GPL-2) | File | What it decides | Program |
  |---|---|---|---|
  | `THD_extract_array` | `thd_dsetto1D.c` | a voxel's series and its scaling (positive factors only: AFNI-01) | both (the average of a series; `-EPI`'s median) |
  | `mri_to_float`, `mri_scale_to_float` | `mri_to_float.c` | one sub-brick as float, its scaling | both |
  | `mri_max` | `mri_max.c` | the clip level histogram's top | both |
  | `mri_cut_3D` | `mri_cut.c` | the octants' boxes of the gradual clip level | both |
  | `THD_countmask` | `thd_makemask.c` | the voxel counts printed, "No voxels?" | 3dAutomask |
  | `thd_floatscan` | `thd_floatscan.c` | non-finite values set to 0 | both |
  | `THD_need_brick_factor` | `thd_initdblk.c` | whether factors are applied | both |
  | `mri_new_vol`, `mri_new_conforming` | `mri_new.c` | zero-filled images | both |
  | `MCW_spheremask`, `MCW_build_mask` | `edt_buildmask.c` | the ball of the local white-matter level | 3dUnifize |
  | `qsort_float` (with `cs_qsort_small.h`) | `mri_percents.c` | the sorted ball values; `-T2`'s percentile | 3dUnifize |
  | `qmed_float` | `cs_qmed.c` | `-GM`'s medians, `-EPI`'s median | 3dUnifize |
  | `AFNI_prefilter_args`, `THD_filename_ok`, `THD_write_3dim_dataset`, ... | `afni_environ.c`, `thd_filestuff.c`, `thd_writedset.c` | global options, output names, existing outputs | both |

  3dUnifize's squash also calls the C library's `tanhf` (and `-EPI` `cbrtf`): the replica
  carries a port of glibc 2.35's (LGPL-2.1-or-later, fdlibm's notice; `crates-gpl/NOTICE`),
  checked against glibc on all 2^32 float inputs. So the replicas are in `larmorx-gpl`
  (`crates-gpl/larmorx-gpl-afni`), and `specs/3dAutomask.md` and `specs/3dUnifize.md` are
  written for the clean-room originals (still to do). The routine-by-routine tables, with
  file:line, are in the restricted findings `docs/findings/afni-automask.md` and
  `afni-unifize.md`.
```

## 2. `docs/findings/upstream-defects.md`

### 2a. The restricted-sections note at the top

In the "Restricted entries (†)" box, after "...through [afni-tshift.md](afni-tshift.md) and
[fsl-mcflirt.md](fsl-mcflirt.md)", add: "; the section "Restricted: AFNI 3dAutomask and
3dUnifize" through [afni-automask.md](afni-automask.md) and [afni-unifize.md](afni-unifize.md)",
and "those two sections" → "those sections".

### 2b. Summary table rows (after AFNI-26 †)

The shared entries' Tool column: AFNI-01, -02, -07, -11, -12, -16, -18, -23 and -26 now
apply to 3dAutomask and 3dUnifize too. Change their Tool cell from `3dTshift` to
`3dTshift, 3dAutomask, 3dUnifize` (AFNI-08, -20 and -22 to `3dTshift, 3dAutomask, 3dUnifize`
as well: the same dataset writer and option scan). Then add:

```markdown
| AFNI-40 † | 3dAutomask, 3dUnifize | an option's value missing as the last argument (`-peels`, `-nbhrs`, `-clfrac`, `-SI`, `-dilate`, `-erode`; `-T2up`): a crash | a | none | an error (exit 1) in both modes, after AFNI's messages | – |
| AFNI-41 † | 3dAutomask | `-nbhrs` outside 6..26 silently becomes 17 (the range check never runs); 19..26 act as 18 | c? | none | AFNI's behaviour in both modes | decide |
| AFNI-42 † | 3dAutomask | `-NN1`, `-NN2`, `-NN3` also set the peeling threshold to 6, 18, 26, overriding `-nbhrs` (an explicit `-NN2` peels with 18, the default with 17) | c? | none | AFNI's behaviour in both modes | decide |
| AFNI-43 † | 3dAutomask | `-peels 0` peels once; the help says 0 turns the peeling off | c | none | corrected by default (no peel), named when the mask changes; `reproduce` peels once | – |
| AFNI-44 † | 3dAutomask | the large-hole filling runs or not depending on the number of slices only (32 to 93 slices: yes; fewer: no) | c? | every BOLD reference (fMRIPrep's) | AFNI's behaviour in both modes | decide |
| AFNI-45 † | 3dUnifize | `-T2up` at or near 100: the percentile read one past the sorted values (memory never written; in AFNI's runs 0, which zeroes the output) | a | none | inverts about the largest value by default, named; `reproduce` fills the read with 0 | – |
| AFNI-46 † | 3dUnifize | `-T2` on a 2D image smaller than 16 × 16: an empty automask dereferenced, a crash | a | none | an error (exit 1) in both modes, after AFNI's messages | – |
| AFNI-47 † | 3dUnifize | more than a million voxels with an axis of 2 or 3: the half-size scale image comes back smaller than the input and is read past its end (a crash in AFNI's runs, or values from that memory) | a | none | the scale brought back to the input's grid by default, named; `reproduce` reads AFNI's smaller image, 0 past its end | – |
| AFNI-48 † | 3dUnifize | `-T2` three times: neither inverted back nor edge-clipped (the output stays inverted); the help documents once and twice | c? | none | AFNI's behaviour in both modes | decide |
| AFNI-49 † | 3dUnifize | the help gives `-clfrac`'s default as 0.1, then says it was raised to 0.2 in 2016; the code uses 0.2 | f | none | 0.2; larmorx's docs say so | – |
```

### 2c. Counts

The counts table (178 entries on 2026-10-18; later parts added more — recount from the table
at the merge): +10 entries: (a) +4 (AFNI-40, -45, -46, -47), (c) +5 of which questionable +4
(AFNI-41, -42, -43, -44, -48), (f) +1 (AFNI-49).

### 2d. The history paragraph (append)

```markdown
The AFNI part of 2026-10-23 (3dAutomask and 3dUnifize, a GPL replica each; in parallel with
`antsAI -b`) added AFNI-40 to AFNI-49 and extended AFNI-01 (3dAutomask's average of a series
and 3dUnifize's `-EPI` median drop a negative scale factor, as 3dTshift's extraction does):
AFNI-43 (class (c)) corrected by default with a switch; AFNI-45 and AFNI-47 (class (a), reads
of memory AFNI never wrote) defined by default, filled deterministically with 0 on request;
AFNI-40 and AFNI-46 (crashes) refused in both modes; AFNI-49 a help text; AFNI-41, -42, -44 and
-48 to decide. The dataset defects 3dTshift found (AFNI-01, -02, -07, -08, -11, -12, -16, -18,
-20, -22, -23, -26) apply to both programs and are handled by the same replica code; their
tool in `larmorx_core::defects` is now "AFNI".
```

and in the gap count sentence: 14 **decide** → 18 **decide** (+ AFNI-41, -42, -44, -48; recount
at the merge).

### 2e. A new restricted section (before "## Restricted: FSL mcflirt (source-derived)")

```markdown
## Restricted: AFNI 3dAutomask and 3dUnifize (source-derived)

> **Clean-room implementers: do not read this section.** Written from AFNI 25.2.09's source
> (NIH and MCW files), through [afni-automask.md](afni-automask.md) and
> [afni-unifize.md](afni-unifize.md). The clean-room originals' implementers work from
> `specs/3dAutomask.md` and `specs/3dUnifize.md`.

### AFNI-40 † An option's missing value crashes — (a)
- **What:** 3dAutomask's `-peels`, `-nbhrs`, `-clfrac`/`-mfrac`, `-SI`, `-dilate` and `-erode`
  read `argv[++iarg]` without checking it exists (`3dAutomask.c:139-245`), and so does
  3dUnifize's `-T2up` (`3dUnifize.c:1036`; its other options check). As the last argument,
  `strtod(NULL)`/`strtol(NULL)` crash: signal 11, AFNI's crash banner, exit 1. 3dAutomask's
  `-prefix`, `-apply_prefix` and `-depth` pass the NULL to `THD_filename_ok` instead: "-prefix
  (null) is illegal!", exit 1 (reproduced as is).
- **Evidence:** read; verified with the oracle for each option (2026-10-23); parity cases
  `errors/missing-value-*` (3dAutomask) and `errors/T2up-missing` (3dUnifize): AFNI crashes,
  the replica refuses, the messages before the crash compared.
- **fMRIPrep:** none.
- **larmorx:** the replica stops with one fatal line naming the option and AFNI-40, exit 1, in
  both modes, after the messages AFNI prints before the crash. Not reproduced: the crash banner.
- **D13 / gap:** never reproduced. No gap.

### AFNI-41 † `-nbhrs` outside 6..26 is silently 17 — (c?)
- **What:** `-nbhrs` sets `nbhrs` directly; the range check reads `new_nbhrs`, which nothing
  sets, so it never runs (`3dAutomask.c:203-206`, `:276-283`); `THD_automask_set_peelcounts`
  replaces values outside 6..26 by 17 (`thd_automask.c:42-46`), and the peeling uses
  `MIN(18, peelthr)` (`:1335`), so 19..26 act as 18. The help: "should be between 6 and 26".
- **Evidence:** read; verified (2026-10-23; parity cases `options/nbhrs-*`).
- **fMRIPrep:** none (it never passes `-nbhrs`).
- **larmorx:** AFNI's behaviour in both modes.
- **D13 / gap:** decide: refuse values outside 6..26 (what the dead check meant), or keep.

### AFNI-42 † `-NN1/2/3` change the peeling threshold — (c?)
- **What:** `-NN1`, `-NN2`, `-NN3` also set `nbhrs` to 6, 18 and 26 (`3dAutomask.c:262-273`,
  "keep historic default of 17 for NN2 unless changed via NNx or nbhrs options"), overriding
  any `-nbhrs`. The help says "the default peeling processes still use NN2", true of the
  neighbourhood only: an explicit `-NN2` peels with 18 of 18 neighbours where the default peels
  with 17, and `-NN1` with 6.
- **Evidence:** read; verified (parity cases `options/NN*-dilate-erode-depth`,
  `options/nbhrs-then-NN2`).
- **fMRIPrep:** none.
- **larmorx:** AFNI's behaviour in both modes.
- **D13 / gap:** decide.

### AFNI-43 † `-peels 0` peels once — (c)
- **What:** `THD_automask_set_peelcounts`: `peelcount = (p > 0) ? p : 1`
  (`thd_automask.c:44`), so `-peels 0` peels once; the help: "Turn off by setting to 0".
- **Evidence:** read; verified (AFNI's `-peels 0` mask equals its `-peels 1` mask on fMRIPrep's
  BOLD reference of ds005040; parity case `defects/peels-0`; the site's worked example).
- **fMRIPrep:** none (it never passes `-peels`).
- **larmorx:** corrected by default: no peeling, named when the mask changes; `reproduce`
  peels once (`larmorx_gpl_afni::mask::MaskSettings::set_peelcounts`).
- **D13 / gap:** done.

### AFNI-44 † The large-hole filling depends on the number of slices — (c?)
- **What:** the large-hole step runs when `nmm > 1 || jj > 0` (`thd_automask.c:388-393`), `jj`
  being the last value assigned to it, `rint(0.016 nz)`. With no axis of 94 voxels or more, a
  volume of 32 to 93 slices runs `THD_mask_fillin_completely(1)` (the small holes, until none is
  left) and one with fewer than 32 does not. Whether `jj > 0` was meant is not clear.
- **Evidence:** read. The parity cases meet both sides (the 40-slice phantoms and fMRIPrep's
  BOLD references, which run the step; the 2-slice and 2 × 2 × 2 inputs, which do not), all
  bit-identical to AFNI; no case isolates the step's effect.
- **fMRIPrep:** its BOLD references (most have 32 to 93 slices): AFNI's behaviour is what
  fMRIPrep gets.
- **larmorx:** AFNI's behaviour in both modes.
- **D13 / gap:** decide (probably keep: changing it changes fMRIPrep's masks).

### AFNI-45 † `-T2up` near 100 reads past the sorted values — (a)
- **What:** `mri_invertcontrast_inplace` (`mri_invert.c:44`): `ii = (int)rintf(nhist * uperc *
  0.01f); ucut = hist[ii]` reads `hist[nhist]` when the index rounds to `nhist`: at `-T2up 100`
  always, and for `u` near 100 when `nhist (1 − u/100) < 0.5`. `hist` is a `malloc` of `nvox`
  floats: what it holds there is memory never written (past the allocation when the whole image
  is in the mask). In AFNI's runs it read 0, which inverts every value to 0: the output is all
  zeros.
- **Evidence:** read; verified (2026-10-23; parity cases `defects/T2up-100`,
  `defects/T2up-99.99`; the site's worked example on fMRIPrep's BOLD reference of ds005040).
- **fMRIPrep:** none (it uses the default 98.5).
- **larmorx:** by default the 100th percentile (the largest sorted value), named;
  `reproduce` fills the read with 0 (D13's deterministic fill), named
  (`larmorx_gpl_afni::unifize::invert_contrast`).
- **D13 / gap:** done.

### AFNI-46 † `-T2` on a small 2D image crashes — (a)
- **What:** with `-T2`, the automask of a 2D image smaller than 16 × 16 is NULL
  (`thd_automask.c:2104`); `mri_invertcontrast_inplace` computes it again and dereferences it
  (`3dUnifize.c:1105-1106`, `mri_invert.c:28-34`): signal 11, exit 1.
- **Evidence:** read; verified (parity case `errors/slice-small-T2`).
- **fMRIPrep:** none.
- **larmorx:** an error (exit 1) in both modes, after AFNI's messages.
- **D13 / gap:** never reproduced. No gap.

### AFNI-47 † A thin volume of more than a million voxels reads past its scale image — (a)
- **What:** above 1,000,000 voxels the local white-matter level is computed on a half-size copy
  (`3dUnifize.c:557`). Halving keeps at least one voxel per axis and doubling keeps an axis of 1
  at 1 (`:166-168`, `:227-229`), so an axis of 2 or 3 voxels comes back as 1: the scale image is
  smaller than the input, and `mri_WMunifize` reads it with the input's indices, past its end
  (`:570-576`); `-ssave` writes it the same way.
- **Evidence:** read; verified on a 720 × 720 × 2 phantom (parity case `defects/slab`): AFNI
  crashed (signal 11) reading past the scale image in every run of 2026-10-23; the case
  compares its messages before the crash and is an expected divergence (with other memory
  AFNI would instead multiply the second slice by whatever it reads).
- **fMRIPrep:** none.
- **larmorx:** by default the half-size image is brought back to the input's grid, named;
  `reproduce` reads AFNI's smaller image linearly with 0 past its end, named
  (`larmorx_gpl_afni::unifize::double_up`).
- **D13 / gap:** done.

### AFNI-48 † `-T2` three times leaves the output inverted — (c?)
- **What:** `do_T2 == 1` inverts back, `do_T2 == 2` clips the bright edges instead
  (`3dUnifize.c:1193-1198`); a third `-T2` does neither. The help documents once and twice.
- **Evidence:** read; verified (parity case `options/T2-T2-T2`).
- **fMRIPrep:** none.
- **larmorx:** AFNI's behaviour in both modes.
- **D13 / gap:** decide (refuse a third `-T2`, or treat it as two).

### AFNI-49 † `-clfrac`'s default in 3dUnifize's help — (f)
- **What:** the help gives the default as 0.1 and then says it was raised to 0.2 in 2016
  (`3dUnifize.c:818-822`); the code uses 0.2 (`:659`).
- **Evidence:** read.
- **fMRIPrep:** none (it passes 0.2).
- **larmorx:** 0.2; `docs/api/afni-unifize.md` says so.
- **D13 / gap:** not D13.

### AFNI-01 †, extended to 3dAutomask and 3dUnifize
- **What:** 3dAutomask averages a series' sub-bricks through `THD_extract_array`
  (`thd_median.c:273-308`), whose positive-factor test (`thd_dsetto1D.c:187-191`) drops a
  negative factor and its magnitude; 3dUnifize's `-EPI` median goes through the same routine
  (`thd_median.c:35-38`) while each volume is scaled with its factor, so a series with a
  negative `scl_slope` unifizes to all zeros.
- **Evidence:** read; verified (parity cases `defects/negative-slope-4d` of 3dAutomask and
  `defects/EPI-negative-slope` of 3dUnifize).
- **larmorx:** both replicas apply the factor by default, named whenever a negative factor is
  met (with the number of mask voxels it changes, in 3dAutomask); `reproduce` drops it.
- **D13 / gap:** done.
```

## 3. `docs/api/upstream-defects.md`

In "What is corrected", after the AFNI-26 row, add (the Original column as for 3dTshift):

```markdown
| AFNI-43 | 3dAutomask | `lx.afni.automask(peels=0)`, `larmorx afni 3dAutomask -peels 0` | peels once | no peeling, as the help says |
| AFNI-45 | 3dUnifize | `lx.afni.unifize(t2=True, t2_percentile=100)`, `-T2up` near 100 | the percentile is read one past the sorted values (memory never written; AFNI read 0 and wrote zeros) | the largest value (the 100th percentile); `reproduce` uses 0 |
| AFNI-47 | 3dUnifize | inputs of more than a million voxels with an axis of 2 or 3 | the half-size scale image is read past its end | brought back to the input's grid; `reproduce` reads AFNI's image, 0 past its end |
| AFNI-01 | 3dAutomask, 3dUnifize `-EPI` | a series with a negative `scl_slope` | the factor dropped when the series is averaged (3dAutomask) or its median taken (3dUnifize `-EPI`: all zeros) | the factor applied |
```

and change the AFNI-01, -02, -07, -11, -12, -16, -18, -23, -26 rows' "Original" column from
`3dTshift` to `3dTshift, 3dAutomask, 3dUnifize` where they say only 3dTshift (the larmorx column
gains `lx.afni.automask`, `lx.afni.unifize`). After the paragraph "Both 3dTshift
implementations have the switch ...", add:

```markdown
The 3dAutomask and 3dUnifize replicas (`larmorx-gpl`, 2026-10-23) have the switch for every
AFNI entry that applies to them; they have no clean-room original yet. AFNI-40 and AFNI-46
(crashes) are refused in both modes.
```

and in "How the switch is validated", add the records `afni-automask.md` and
`afni-unifize.md` to the list.

## 4. `README.md`

Tools table, after the 3dTshift row:

```markdown
| 3dAutomask (brain mask) | `larmorx afni 3dAutomask`, `lx.afni.automask` | exact replica in `larmorx-gpl`; clean-room original planned ([spec](specs/3dAutomask.md)) | [105 cases](docs/validation/afni-automask.md): 81 of 81 compared cases that write outputs bit-identical to AFNI, fMRIPrep's call on every oracle run's BOLD reference included |
| 3dUnifize (intensity uniformization) | `larmorx afni 3dUnifize`, `lx.afni.unifize` | exact replica in `larmorx-gpl`; clean-room original planned ([spec](specs/3dUnifize.md)) | [97 cases](docs/validation/afni-unifize.md): 72 of 72 compared cases that write outputs bit-identical to AFNI (at 1, 2 and 6 threads), fMRIPrep's call on every oracle run's BOLD reference included |
```

Repo map row `crates-gpl/`: "GPL-3.0-or-later replicas: 3dTshift, 3dAutomask, 3dUnifize".

## 5. `docs/validation/README.md`

After the `afni-tshift-replica` row:

```markdown
| [afni-automask](afni-automask.md) | `larmorx-gpl afni 3dAutomask` (GPL replica, `crates-gpl/`), run by `larmorx afni 3dAutomask` and `lx.afni.automask` | 3dAutomask (AFNI 25.2.09 binary; fMRIPrep 25.2.6's AFNI as a second build) | `validated` |
| [afni-unifize](afni-unifize.md) | `larmorx-gpl afni 3dUnifize` (GPL replica), run by `larmorx afni 3dUnifize` and `lx.afni.unifize` | 3dUnifize (AFNI 25.2.09 binary, 1, 2 and 6 OpenMP threads; fMRIPrep 25.2.6's AFNI) | `validated` |
```

and in the commands block:

```bash
python -m larmorx_validation parity afni-automask --tier standard --out docs/validation   # the AFNI oracle and the larmorx-gpl build
python -m larmorx_validation parity afni-unifize --tier standard --out docs/validation
```

No full-tier record is in the branch (the machine was shared; a full run is about two hours):
the full tier's own cases were run once with the harness instead ("Numbers").

## 6. `docs/validation/golden.md`

Table row after `larmorx-gpl afni 3dTshift`:

```markdown
| `larmorx-gpl afni 3dAutomask`, `3dUnifize` | 16 | 3dAutomask: fMRIPrep's call on an int16 head phantom with `-apply_prefix` and `-depth`; `-eclip -nograd -NN1 -erode`; a 4D series with `-SI`; a 2D slice; `-peels 0` (AFNI-43) in both modes. 3dUnifize: the default on int16 with `-ssave` and `-amsave`; fMRIPrep's `-T2` call; `-GM -nosquash`; `-T2 -T2`; `-EPI` on a series, and on one with a negative scale factor in both modes (AFNI-01); `-T2up 100` in both modes (AFNI-45); a 2D slice |
```

and a note in the coverage section:

```markdown
- **Tools with only a replica** (3dAutomask and 3dUnifize, until their clean-room originals
  exist): the main package's command line has no implementation of its own to check, so the
  coverage test (`test_tools_with_only_a_replica_have_golden_cases_in_crates_gpl`) requires
  their golden cases in `crates-gpl/larmorx-gpl-cli/tests/golden.tsv` instead, and the Python
  registry lists `lx.afni.automask` and `lx.afni.unifize` as replica-only
  (`tests/golden/registry.py`, `REPLICA_ONLY_FUNCTIONS`); `lx.afni.fmriprep_unifize_options`
  has its own case. 3dUnifize's half-size path (more than a million voxels) and AFNI-47 are
  beyond the 32³ budget: the parity records cover them (real T1w images; a 720 × 720 × 2 slab).
```

## 7. `docs/next-steps.md`

Replace item 5 ("**AFNI:** 3dUnifize, 3dAutomask ...") with:

```markdown
5. **AFNI 3dAutomask and 3dUnifize: the replicas done** (2026-10-23, in parallel with `antsAI
   -b`). Both depend on MCW library routines (`docs/licensing.md`), so each has a GPL replica in
   `larmorx-gpl`, bit-identical to AFNI 25.2.09 in every compared case (fMRIPrep's calls on
   every oracle run's BOLD reference, T1w images, BOLD series, at 1, 2 and 6 threads, and
   fMRIPrep's own build of AFNI), and a spec for a clean-room original (`specs/3dAutomask.md`,
   `specs/3dUnifize.md`). **Next for them: the clean-room originals**, by an agent that has
   never read AFNI's source, from the specs only (the porting guide's speed rules: the ball's
   percentile mean is the hot loop); `larmorx afni 3dAutomask` / `3dUnifize` then get
   `implementation="auto"|"replica"|"original"` as 3dTshift. Optional later AFNI paths of
   fMRIPrep's libraries: 3dQwarp (sdcflows' alternative PEPOLAR estimator), 3dSkullStrip with
   3dcalc (niworkflows' legacy AFNI skull-stripping): **planned for later** (user,
   2026-10-19). **A later AFNI analysis track (decided by the user, 2026-10-19; not fMRIPrep):**
   3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit, 3dFWHMx/3dClustSim,
   3dAllineate, 3dcalc, 3dTstat, 3dROIstats/3dmaskave, 3dresample, 3dvolreg (low priority).
   Check each program's call graph routine by routine (2026-10-23: the programs' own files can
   be NIH while the library routines they rely on are MCW).
```

Under "Waiting for the user", add:

```markdown
- **Four new questionable behaviours of AFNI** (2026-10-23; register entries marked
  **decide**), each kept as AFNI's until decided: AFNI-41 (3dAutomask's `-nbhrs` outside 6..26
  silently 17: proposed, refuse it as the dead check meant), AFNI-42 (`-NN1/2/3` change the
  peeling threshold: proposed, keep AFNI's, document), AFNI-44 (the large-hole filling depends
  on the number of slices: proposed, keep, since it shapes fMRIPrep's BOLD masks), AFNI-48
  (3dUnifize's third `-T2`: proposed, refuse).
```

Follow-ups (when they fit):

```markdown
- **The clean-room originals of 3dAutomask and 3dUnifize** (from `specs/3dAutomask.md`,
  `specs/3dUnifize.md`); a benchmark report for each once the original exists.
```

## 8. `CLAUDE.md`, "Where things are"

In the table's AFNI row:

```markdown
| AFNI | 3dTshift, complete: clean-room original (`crates/larmorx-afni`) and GPL replica (`crates-gpl/`); 3dAutomask and 3dUnifize: GPL replicas (`crates-gpl/`), clean-room originals still to write from their specs | `afni-tshift`, `afni-tshift-replica`, `afni-automask`, `afni-unifize` |
```

and a status line: "**AFNI 3dAutomask and 3dUnifize merged** (the merge's date): GPL replicas, bit-identical
to AFNI 25.2.09 in every compared case; specs for the clean-room originals; evidence pages."

## 9. The site (`larmorx_documentation`)

### `data/status.yml` (`ported:`, after 3dTshift)

```yaml
  - eco: afni
    tool: 3dAutomask (brain mask)
    page: /tools/afni-automask/
    use: ["larmorx afni 3dAutomask", "lx.afni.automask"]
    how: exact replica in `larmorx-gpl`; clean-room original planned
    validation: "[105 cases](repo:docs/validation/afni-automask.md): 81 of 81 compared cases that write outputs bit-identical to AFNI, fMRIPrep's call on every oracle run included"
  - eco: afni
    tool: 3dUnifize (intensity uniformization)
    page: /tools/afni-unifize/
    use: ["larmorx afni 3dUnifize", "lx.afni.unifize"]
    how: exact replica in `larmorx-gpl`; clean-room original planned
    validation: "[97 cases](repo:docs/validation/afni-unifize.md): 72 of 72 compared cases that write outputs bit-identical to AFNI at 1, 2 and 6 threads, fMRIPrep's call on every oracle run included"
```

and in `planned:`, the AFNI entry's text:

```yaml
    text: the clean-room originals of 3dAutomask and 3dUnifize; then 3dQwarp, 3dSkullStrip, 3dcalc and an analysis track (3dTproject, 3dDespike, 3dReHo, 3dRSFC, 3dDeconvolve/3dREMLfit, 3dFWHMx/3dClustSim, ...).
```

### `site.yml` (the tools navigation, after 3dTshift)

```yaml
  - { title: 3dAutomask, url: "/tools/afni-automask/", eco: afni,
      summary: "A brain mask from intensities: an exact replica in larmorx-gpl." }
  - { title: 3dUnifize, url: "/tools/afni-unifize/", eco: afni,
      summary: "White-matter intensity uniformization: an exact replica in larmorx-gpl." }
```

### `content/validation.md`

No change needed beyond the summary matrix (from `summary.json`). Optionally, in the
paragraph "Three ways of reimplementing", after "(`larmorx-gpl` for AFNI)": nothing; the text
already covers tools with only a replica so far? It does not: add after "...and at speed
([licensing](repo:docs/licensing.md))." the sentence: "A tool may have its replica before its
clean-room original (3dAutomask and 3dUnifize, for now)."

### `static/evidence/summary.json`

Re-export after merging: `python -m larmorx_validation evidence summary --site <site>`
(`evidence/summary.py` lists the two pages; each with one implementation, "larmorx-gpl
replica").

---

## Numbers (the records in the branch)

- **`docs/validation/afni-automask.md`** (standard tier, from `0cd5ea2`): 105 cases: 85 pass,
  19 rejected by both (6 of them AFNI's crashes, AFNI-40), 1 expected divergence (AFNI's own
  format as the default prefix), 0 failures. Every output of the 81 compared cases that write
  outputs bit-identical to AFNI (data and header); messages 98 of 98; fMRIPrep's own masks 7 of
  7 (every oracle run's BOLD reference); AFNI's own variability: at 1 thread 10 of 10, at 6
  threads 7 of 7, fMRIPrep's build 10 of 10 give the oracle's bytes; the Python API 20 of 20.
  The fix pass meets AFNI-01, -02, -07, -11, -12, -16, -18, -23, -26 and -43.
- **`docs/validation/afni-unifize.md`** (standard tier, from `ee419c4`): 97 cases: 76 pass,
  19 rejected by both (3 of them AFNI's crashes, AFNI-40 and AFNI-46), 2 expected divergences
  (the default prefix; AFNI-47, where AFNI crashed reading past its scale image), 0 failures.
  Every output of the 72 compared cases that write outputs bit-identical to AFNI; messages 93
  of 93; fMRIPrep's own outputs 7 of 7; AFNI at 1 thread 7 of 7, at 6 threads 9 of 9,
  fMRIPrep's build 9 of 9; the Python API 20 of 20. The fix pass meets AFNI-01, -02, -07, -11,
  -12, -16, -18, -23, -26, -45 and -47.
- The parity runs use `OMP_NUM_THREADS=2` for the oracle and the replica (two parallel
  agents shared the machine); the bands add 1 and 6.

## Other notes for the merge

- **The site's viewer** (`static/js/evidence.js`, in the site branch) gained mask overlays: a
  panel's `overlay: {<output>: {on: <panel>, window: [lo, hi]}}` draws that output over the
  other panel's image (tinted with AFNI's pastel, its edge in the slice drawn in full), in the
  side-by-side, swipe, blink and checkerboard modes alike; the still posters do the same
  (`larmorx_validation.evidence.example.poster_overlay`, used by `common.finish`). The site's
  README documents the field. No other page uses it, so no other page changes.
- **A bug in Atropos' evidence, found here, not fixed** (another tool's module):
  `evidence/ants_atropos.py` passes the module-level `OUTPUTS` and `OUTPUTS_PRIORS =
  [OUTPUTS[0], OUTPUTS[2]]` to `common.finish`, which writes each output's `windows` into those
  dicts; the two examples share them, so the first example's intensity windows for the labels
  and the grey matter's posterior are those of the priors example (computed last). Only the
  default display windows are affected, no number or verdict. The fix is
  `copy.deepcopy(OUTPUTS)` per example (as `afni_automask.py` and `afni_unifize.py` do), then a
  re-export of `ants-atropos`.
- **AFNI-18 in `reproduce` mode is not named** (code shared with 3dTshift's replica,
  `cli::common::open_dataset`): the default names the correction, `reproduce` prints AFNI's
  messages without a "reproduced" notice, where AFNI-12 is named in both modes. Harmless (the
  parity compares AFNI's messages), but inconsistent with the rule that both modes name a
  defect; left as 3dTshift has it.
- **Evidence size:** 3dAutomask's page adds 0.4 MB of blobs, 3dUnifize's 6.4 MB (the T1w).

### The full tier's own cases (run once, from `300fcd8`; not a record)

With the harness (`run_one`), log `oracles/logs/afni-mask-unifize-full-extras.log`: 3dAutomask's
default on the BOLD series of ds005040, ds005454 and ds005752 (3 of 3 pass); 3dUnifize's
`-nodouble`, the default on the T1w of ds001600, ds005040, ds005454 (MP2RAGE) and ds005752,
`-GM` on ds000005's T1w and `-EPI` on ds005454's multiband series (7 of 7 pass): every output
bit-identical to AFNI, AFNI's other thread counts and fMRIPrep's build giving the same bytes.
With the standard records: every oracle run's BOLD reference through both programs as fMRIPrep
calls them, every oracle run's T1w through 3dUnifize, a BOLD series of every oracle run through
3dAutomask, and sMRIPrep's skull-stripped T1w (ds000005) through 3dAutomask. A full-tier record can be made with `--tier full` (about two hours on a quiet
machine).
