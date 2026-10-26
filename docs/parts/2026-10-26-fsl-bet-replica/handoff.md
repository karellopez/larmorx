# HANDOFF: BET's merge (the BET replica joined to `crates-nc` and `larmorx-nc-core`)

Branch `worktree-agent-a38ebb82788afa285` (larmorx); the site clone `<scratch>/site`, branch
`fsl-bet-evidence` (on the site's main `ebf30bd`), its local `evidence-blobs` holding BET's
blobs (push the blob branch before `main`). `<scratch>` =
`/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/fsl-bet`.
Part 1's handoff (the replica itself) is kept as `<scratch>/HANDOFF-part1.md`; this one replaces
it: every text below is the final one. Numbers are those of the committed records
(`docs/validation/fsl-bet-replica.md`, standard tier; `fsl-bet-replica-full.md`, full tier).

**Final state.** The larmorx branch ends at `82e4f9e` (clean; FSL-22 reproduced exactly, after `04dd94d`); main merged into it up to `f86ce4f`. Records from `8e0059a`, crates-nc golden from `bb72fb6`, main golden from `d54d773` (parity smoke tier passing). The site branch `fsl-bet-evidence` = the site's main `ebf30bd` + `825dc38`; its local `evidence-blobs` = `11febbc` (push it first). New defect ID used: FSL-58 (FSL-59 unused; PIPE-35..39 unused).

## 0. What the merge changed (for the orchestrator's review)

- **One `crates-nc` workspace** (C's), with `larmorx-nc-bet` a member; the stand-in program
  `larmorx-nc-bet` is retired: `larmorx-nc fsl bet|bet2|betsurf` (`larmorx-nc-cli`'s
  `FSL_TOOLS` = mcflirt, bet, bet2, betsurf). BET's golden cases are in
  `crates-nc/larmorx-nc-cli/tests/golden.rs` (`BET_CASES`), its checksums in the one
  `golden.tsv`. Main merged twice (60b8405, then 33256cd and f86ce4f).
- **One translation of each FSL library, in `larmorx-nc-core`.** Where both parts had translated
  the same library, one version remains, and both replicas' golden files, records and NaN
  matrices give the same bytes with it:
  - `volume`: newimage's `volume<T>` generic over its element types (`Elem`: `i8` (`char`),
    `i16`, `i32`, `f32`, `f64`; BET reads and writes every stored type, mcflirt `float`);
    `read_volume::<T>`, `read_volume4d::<T>`, `save_volume`, `save`/`save_typed` with
    `SaveOptions`; `robust_limits` (`find_thresholds`), `calc_cog`, `background_value` (FSL-22's
    `NanOrder`); file-name rules.
  - `nifti`: NewNifti with FSL-56's offset as a switch (`save_image(..., reproduce_offset)`), gzip
    files with znzlib's settings (zlib's gzip header of a Unix zlib, level 6): their decompressed
    bytes are FSL's; the compressed stream is zlib-rs's (compared decompressed, as every
    compressed output: decision log 2026-10-16; part 1's note that `.nii.gz` files were FSL's
    byte for byte compressed was wrong and is corrected in the core's PROVENANCE).
  - `interp`, `newimagefns`: interpolation generic over the element types; `smooth`,
    `gaussian_kernel_1d` (BET's) beside mcflirt's `affine_transform` and `gradient`.
  - `arma` (both parts' small-matrix paths), `fdlibm` (mcflirt's `atan2f`, `atanf` and BET's
    `tanh`, `expm1`), `cnum` (`strtod`, `strtof`, `atof`, `atoi`, `strtol`, `printf`), `x87`.
  - moved from BET: `mesh` (meshclass: `Mesh`, `Profile`, `Mesh::load`), `options`
    (fsl-utils' parser), `avwutils` (the parts of `fslstats`, `fslroi`, `fslmerge`, `fslmaths`,
    `fslval` and fslpy's `imtest`/`remove_ext`/`imglob` that `bet` runs), `istream`, `stdsort`,
    `glibc_double` (`pow`, `exp`) and `tools/libm-verify`.
  - Documented: `crates-nc/larmorx-nc-core/PROVENANCE.md` ("The API, for the next replicas", one
    row per module).
- **NaN rules (C's `sse`) in BET's own arithmetic**, wherever NaN or infinite voxels reach it:
  newimage's `find_thresholds` (the histogram's range and the 2 and 98 % thresholds), meshclass's
  points and vectors (`V3` operations, centroids, normals, neighbour means and distances,
  translations, retessellation), bet2's initial mesh (the threshold, the centre of gravity's
  sums and division) and deformation step, betsurf's deformation step. Found by BET's NaN matrix
  (`validation/probes/fsl-bet-nan`): an infinite voxel made `0 * inf` in the histogram's range,
  and the mesh then printed `-nan` on x86 and `nan` with AArch64's default NaN emulated. The
  final matrix: 60 cases, 0 differ (release, unoptimised debug, release with ARM's NaN; FSL's
  bits wherever FSL finishes; FSL crashed in 18).
- **D13 failure cases** (defined parts reproduced, the rest filled deterministically and named;
  the default refuses):
  - FSL-41 (no voxel strictly between the 2nd and 98th percentiles inside the initial sphere: FSL
    reads the median from an empty list and crashes): `reproduce` takes the median as 0, names
    FSL-41 and writes bet2's outputs.
  - FSL-52 (mesh files read without checks): `reproduce` takes 0 for every value FSL never sets
    and leaves out the faces that name missing vertices, names FSL-52 and writes betsurf's
    outputs; only an empty mesh is refused in both modes (FSL divides by its size: nothing to
    fill).
  - FSL-43's crash class (a fill starting far outside the image or at a NaN centre, where FSL
    writes far outside the volume and crashes, as after FSL-42's NaN `-f` or `-w 0`):
    `reproduce` skips that fill (the mask holds the drawn surface only) and names FSL-43.
  - FSL-58 (new, random testing): betsurf with no voxel of the T1 inside the brain surface reads
    its 98th percentile from an empty list: `reproduce` takes it as 0.
  - Inside `bet`, where FSL's bet2 crashes, `reproduce` gives FSL's defined outcome instead of a
    fill: the script's `bet failed during command:...` and status 1 (bet2 alone still fills).
  - Parity: the record's six crash-filled cases (`bet2/constant`, `defects/nan-f`,
    `defects/zero-w`, `defects/mesh-unread`, `-missing-vertex`, `-truncated`) are expected
    divergences of the kind "FSL crashes; `reproduce` fills": larmorx exits 0, names the defect,
    writes deterministic outputs (run twice, byte-identical), and what FSL printed before its
    crash begins the replica's output (FSL writes no output to compare; its buffered standard
    output may be lost at the crash). In the default mode those inputs are refused.
- **FSL-22** (C's, `calc_bval` now in the core): reproduced exactly (2026-10-26, the
  orchestrator's go-ahead). By default NaN border values are sorted last (D13: undefined
  behaviour is never reproduced by default); `reproduce` sorts them as libstdc++'s `std::sort`
  does, move for move (`stdsort::sort_f32_checked`: the SGI STL's introsort with libstdc++'s
  pivot step, every index checked against the vector's ends; where its loops would leave the
  vector, NaNs last and named). Both name FSL-22 where NaN border voxels make the order
  undefined (mcflirt: the final resampling, `-meanvol`'s second pass); the crates-nc golden case
  `mcflirt.nonfinite` names FSL-14 and FSL-22 (its checksums unchanged).
  - Evidence: the sort equals libstdc++'s on 768 seeded vectors, 495 of them with NaNs (GCC 11.4
    and 14.3 give the same hash; FSL 6.0.7.17 was built with GCC 13.2;
    `validation/probes/libstdcxx-sort/`, the unit test
    `checked_sort_gives_libstdcxx_order_with_nan`); C's NaN matrix: 160 of 160 cases give
    mcflirt's bytes in `reproduce` (32 differed before) and agree among the replica's four runs.
  - The matrix's `bgnan-stats` then showed a second cause, fixed: mcflirt's `-stats` sums pass
    NaNs on in the operand order of FSL's binary (`run_and_save_stats`: `addss` with the voxel's
    value as the destination for the mean, the product for the variance), not the source's; 6
    voxels with NaNs of both signs had the other sign in the mean, variance and sigma.
  - The HP/SGI copyright and permission notice is in `crates-nc/NOTICE` and the core's
    `PROVENANCE.md` (and named in `stdsort.rs`).
  - Not re-run: mcflirt's parity tiers (no series of theirs holds a NaN, so neither change
    reaches them; the golden suite passes unchanged).
- **The oracle-output cache**: BET's suite runs FSL only through `oracle_cache.run`
  (`fsl_bet.oracle_run`, `Policy(threads=1, family="fsl")`, `case_never` for FSL-43, -51 and -53's
  memory and FSL-46's removals); FSL names images without their extension, so the cache gained
  `inputs=` (files a program reads that its arguments do not name as files, keyed by content:
  `validation/src/larmorx_validation/oracle_cache/{core,key}.py`, a test in
  `validation/tests/test_oracle_cache.py`). Derived inputs (bet2 meshes and FLIRT matrices of the
  real images, made by FSL) moved from `oracles/cache/fsl-bet-inputs/` to
  `oracles/inputs/fsl-bet/<sha256 prefix>/`.
- **Memory never above FSL's** (the rule of 2026-10-25): `tests/parity/test_memory_peak.py` gained
  `BET_CASES` (bet2 with every output, `bet -R`, `bet -F` on 30 volumes, betsurf on bet2's
  surface; outputs FSL's byte for byte in `reproduce`). To pass it: saving converts and writes a
  volume's values chunk by chunk (no copy of the image, of header and data together, or of the
  volume unless its left-right order changes back: the core's `save_image`/`write`, mcflirt's
  saves too), bet2 keeps one copy of its input unless `--ct` and moves its masks into the outputs,
  betsurf replaces each count image by its smoothed one. Measured peaks (larmorx's command against
  FSL's program, MiB): bet2 on a real T1w 84 (FSL 118), bet `-R` 106 (121), betsurf `--t1only`
  190 (240), bet `-F` on a 240-volume BOLD 226 (264), the synthetic head below FSL's in every call.
- **Random testing** (`validation/src/larmorx_validation/difftest/tools/fsl_bet.py`, registered;
  `docs/validation/difftest.md` has its row): bet2's options, bet's variations (`-R`, `-F` on a
  series, `-Z` on a slab) with them, betsurf with and without a T2 on a closed surface the
  descriptor makes (a subdivided octahedron on a sphere, square roots only), every stored type and
  FSL's output types; BET's suite judges such a case with `expect="random"` (a case where FSL
  crashes is judged by the failure case larmorx names); FSL-51, -53, -55 join the runner's
  memory-dependent defects (verdict *undefined*). Four trial runs (seeds `bettrial1` to
  `bettrial4`, 34, 324, 30 (stopped: the extension was rebuilt under it) and 201 cases; the last run's one difference, a filled run that then meets a later failure, is judged by the suite now) found 7 differences, every one fixed: fslstats `-C` had dropped
  newimage's COG warning of an empty brain; bet's `reproduce` had filled where FSL's bet2
  crashes (now the script's failure); a filled run's messages compared from FSL's (the suite);
  betsurf panicked on an empty brain (FSL-58, new); fslmaths `-mas` wrote float for a double
  series (it computes and writes double); FSL-43's correction was named where it changed nothing
  (now compared with FSL's fill, and named only where an output shows the mask); betsurf
  `--t1only` with an unknown `FSLOUTPUTTYPE` exited 0 in `reproduce` (FSL exits 1 from its
  library: FSL-54 does not apply). Six permanent cases (`validation/difftest/cases/fsl-bet/`),
  `triage.toml` entries. BET's site page gets its random-testing panel once a night includes
  `fsl-bet` (section 11).
- **`pow` and `exp` translated from Arm's Optimized Routines v19.11 (MIT)**, not glibc's LGPL
  files: the decision log of 2026-10-24 says no LGPL code enters the FSL-licensed package, and
  part 1 had translated glibc's `e_pow.c`/`e_exp.c`. glibc 2.35's are Arm's: every number of
  `__exp_data` (270) and `__pow_log_data` (393) identical with Arm's x86-64 configuration, the
  code differing only in macro names
  (`validation/investigations/fsl-bet/arm_glibc_pow_exp.py`); `tools/libm-verify` still finds 0
  of 4 million calls of each different from the C library. The LGPL-2.1 text that part 1 had
  added to `crates-nc/LICENSES/` is gone again; NOTICE names Arm for `pow` and `exp`.
- **CI time**: BET's golden cases took 130 s of the debug `cargo test` (CI's) unoptimised;
  `crates-nc/Cargo.toml` now builds `larmorx-nc-core` and `larmorx-nc-bet` at `opt-level = 1` in
  the dev profile (debug assertions and overflow checks stay on): about 30 s on this machine
  (shared). Estimate on the runners: about +1 min per platform for the golden test (~6 min per CI
  run), +20 to 40 s of compilation cold. No new job or step.
- **Provenance paths**: `figures/cache.py`'s `checkout_paths` now also maps `crates-nc/...`
  builds to `<larmorx>/crates-nc/...` (the BET page had named the worktree); a test
  (`validation/tests/test_figures_cache_paths.py`). No other page's export changes (no other page
  names a crates-nc build).
- **The evidence cache entries** (`oracles/figures-cache/fsl-bet/`) were computed again with
  `larmorx-nc` (the stand-in's entries archived in
  `<workspace>/archive/2026-10-25-fsl-bet-evidence-entries/`, with a README): FSL's and the
  replica's outputs 54 of 54 files identical to the archived ones; the data-panel entry
  (`panel-ds000228-pixar008`) is new.

## 1. `docs/findings/upstream-defects.md`

### 1a. The note on restricted entries (top of the file): add BET

Replace "the section "Restricted: AFNI 3dAutomask and 3dUnifize" through
[afni-automask.md](afni-automask.md) and [afni-unifize.md](afni-unifize.md)." by:

> the section "Restricted: AFNI 3dAutomask and 3dUnifize" through
> [afni-automask.md](afni-automask.md) and [afni-unifize.md](afni-unifize.md); the section
> "Restricted: FSL BET" through [fsl-bet.md](fsl-bet.md).

(If the note lists `fsl-mcflirt.md` for the mcflirt section, add `fsl-bet.md` beside it in the
same form.)

### 1b. Summary table rows (after FSL-22)

```
| FSL-40 † | bet2 | names lose their first `.hdr` wherever it stands; a name with `.img` and no `.hdr` aborts | e | none | corrected by default (`upstream_defects`, 2026-10-24): a trailing extension removed | – |
| FSL-41 † | bet2 | no voxel strictly between the 2nd and 98th percentiles inside the initial sphere (a constant or binary image): the median read from an empty list, crash | a | none | refused by default; `reproduce` takes the median as 0, names FSL-41 and writes bet2's outputs (D13's failure case, 2026-10-25) | – |
| FSL-42 † | bet2 | NaN `-f`/`-g`, a negative, NaN or infinite `-r`, `-w` ≤ 0, a non-finite `-c` or search depth used as given | e | none | refused by default, naming the value (rule B, 2026-10-24); `reproduce` computes with it | – |
| FSL-43 † | bet2, betsurf | a mask's fill starts at the surface's centroid without a bounds check: outside the image (a single slice) FSL writes and reads outside the volume; far outside or at a NaN centre it crashes | a | none | the fill starts at the nearest voxel inside (rule A, 2026-10-24); `reproduce` does FSL's defined part one voxel outside, and where FSL crashes skips the fill (the mask holds the drawn surface) and names FSL-43 (2026-10-25) | – |
| FSL-44 † | bet | arguments expanded unquoted: names with blanks split into words, an empty argument ends the options | e | none | corrected by default (`upstream_defects`, 2026-10-24) | – |
| FSL-45 † | bet `-Z` | the crop back runs `imglob` without `${FSLDIR}/bin`: off the PATH the outputs stay padded, on it other images are cropped too | c | none (fMRIPrep's image has no `dc`: its `bet -Z` stops earlier) | corrected by default (`upstream_defects`, 2026-10-24) | – |
| FSL-46 † | bet | the cleanup removes every `<output>_tmp*` file, also files the run did not create | c | none | corrected by default (`upstream_defects`, 2026-10-24) | – |
| FSL-47 † | betsurf `--t1only` | the scalp mask is filled from the outer skull surface's centre, in the padded grid | c | none | corrected by default (`upstream_defects`, 2026-10-24), named when the mask changes | – |
| FSL-48 † | betsurf | a T2 on another grid than the T1 is sampled at the T1's voxel coordinates | e | none | refused by default (rule B, 2026-10-24) | – |
| FSL-49 † | betsurf | when the T2's scalp lies beyond the T1's, the T1's estimate is validated and kept (`outskin = outskin`, flagged "very suspicious" in FSL's source) | c (c? until 2026-10-24) | none | kept as FSL's and documented (rule C, 2026-10-24) | – |
| FSL-50 † | betsurf | one profile with an empty search range ends the program, status 255, no message, no output | c | none | the profile dropped (rule A, 2026-10-24) | – |
| FSL-51 † | betsurf | profile searches can read past the profile's samples (memory never set) and use it | a | none | the profile dropped; `reproduce` reads "nothing there" and warns | – |
| FSL-52 † | betsurf | mesh files read without checks: values never set, faces naming missing vertices, an empty mesh | a | none | refused by default, naming the problem; `reproduce` takes 0 for the values FSL never sets and leaves out the faces naming missing vertices, names FSL-52 and writes the outputs (D13's failure case, 2026-10-25); an empty mesh refused in both modes | – |
| FSL-53 † | betsurf | a matrix with fewer than the 12 numbers used: entries never set | e | none | refused by default (rule B, 2026-10-24); `reproduce` uses 0 and warns | – |
| FSL-54 † | betsurf `--t1only` | exits 0 when it fails (too few arguments, matrix not found) | e | none | corrected by default (`upstream_defects`, 2026-10-24) | – |
| FSL-55 † | betsurf | a NaN in the T1 inside the brain surface: the 98th percentile's selection is undefined | a | none | refused by default (rule B, 2026-10-24); `reproduce` runs libstdc++'s selection, stopped at the vector's ends | – |
| FSL-56 † | FSL's NIfTI writer (NewNifti) | two or more extensions of different sizes: `vox_offset` counts the first one's size once per extension | c | none (extensions are dropped unless `FSL_LOAD_NIFTI_EXTENSIONS`) | corrected by default (`upstream_defects`, 2026-10-24), in every FSL replica (the switch is the core's) | – |
| FSL-57 † | betsurf | the smoothness weight is set to 0 where a mask applies after the term was computed (no effect) | c (c? until 2026-10-24) | every betsurf run | kept as FSL's and documented (rule C, 2026-10-24) | – |
| FSL-58 † | betsurf | no voxel of the T1 inside the brain surface (a surface outside the image, a tiny image): the 98th percentile is read from an empty list, before its start; FSL crashes or uses memory it never wrote | a | none | refused by default, naming the entry; `reproduce` takes the percentile as 0 and goes on (D13's failure case; found by random testing, 2026-10-26) | – |
```

And the FSL-22 row's larmorx cell, now done: "NaNs sorted last by default (defined; D13,
2026-10-11); `reproduce` exact (SGI STL's sort), 2026-10-26: libstdc++'s order of the NaN values,
move for move, from the SGI STL's introsort (HP/SGI permission notice) with libstdc++'s pivot
step, not from libstdc++'s code; both name FSL-22 when NaN border voxels make the original's
order undefined".

And in the FSL-22 row's evidence wording, if the row says the order "is not emulated" or
"differs in 32 of 160 cases" anywhere else, replace it by: "the NaN matrix: 160 of 160 cases
mcflirt's bytes in `reproduce` (2026-10-26)".

Counts: +19 entries (a 6: FSL-41, -43, -51, -52, -55, -58; c 7: FSL-45, -46, -47, -49, -50,
-56, -57, of them FSL-49 and FSL-57 questionable, decided; e 6: FSL-40, -42, -44, -48, -53, -54):
243 → 262 (recount against the register as it stands at the merge). No **decide** and no **yes**:
every entry was settled by rules A to D or is a D13 failure case.

### 1c. The restricted section (after "Restricted: FSL mcflirt")

```
## Restricted: FSL BET

**Restricted (†):** written from FSL's source through [fsl-bet.md](fsl-bet.md), which has the
`file:line` references; clean-room implementers read `specs/bet.md` instead.

### FSL-40 † Names lose their first `.hdr` — (e)
- **What:** bet2 erases `.hdr` wherever it appears in the input and output names (a directory
  `x.hdr/` is read as `x/`), and for a name containing `.img` erases at `.hdr` again: with `.img`
  and no `.hdr` that throws `std::out_of_range`, uncaught.
- **Evidence:** read; verified (`defects/hdr-in-name`, `bet2/img-output`).
- **fMRIPrep:** none.
- **larmorx:** removes a trailing image extension only; `reproduce` erases as FSL does and,
  where FSL aborts, refuses with FSL's message.
- **D13 / gap:** rule A. No gap.

### FSL-41 † No voxel for the median intensity — (a)
- **What:** the median within-brain intensity is the largest of the lower half of the voxels
  strictly between `t2` and `t98` inside the initial sphere; with none (a constant or binary
  image) FSL dereferences an empty vector and crashes.
- **Evidence:** read; verified (`bet2/constant`: FSL ends by SIGSEGV).
- **fMRIPrep:** none.
- **larmorx:** refused by default, after FSL's messages; `reproduce` takes the median as 0
  (FSL's defined part: everything before and after the read), names FSL-41 and writes bet2's
  outputs (D13's rule for failure cases, 2026-10-11; done 2026-10-25).
- **D13 / gap:** No gap.

### FSL-42 † Meaningless numeric options are used — (e)
- **What:** bet2 checks `-f` and `-g` against their ranges only (NaN passes); `-r`, `-w`, `-c`
  and `--parameter_d1/d2` not at all: NaN or infinite values, a negative radius or a smoothness
  of 0 or less are computed with (the surface diverges or FSL crashes).
- **Evidence:** read; verified (`defects/nan-f`, `defects/negative-r`, `defects/zero-w`).
- **fMRIPrep:** none.
- **larmorx:** refused by default, naming the value; `reproduce` computes as FSL does (where FSL
  then crashes in its fill: FSL-43's crash class).
- **D13 / gap:** rule B (decided 2026-10-24). No gap.

### FSL-43 † A mask's fill starts outside the image — (a)
- **What:** the fill of the brain mask (bet2) and of every surface's mask (betsurf) starts at the
  truncated centroid of the vertices, written without a bounds check; on a single slice the
  centroid lies one voxel below it and FSL writes and reads beside the volume's buffer (and goes
  on); far outside, or at a NaN centroid (a diverged surface), it writes far from the buffer and
  crashes.
- **Evidence:** read; verified (`defects/2d`: voxel `47 55 -1`, FSL's outputs written;
  `defects/nan-f`, `defects/zero-w`: FSL crashes).
- **fMRIPrep:** none.
- **larmorx:** the fill starts at the nearest voxel inside the image (on `defects/2d`, FSL's
  outputs bit for bit); `reproduce` does FSL's defined part one voxel outside (no write outside,
  the start's neighbours inside the image examined) and warns; where FSL crashes, `reproduce`
  skips that fill (the mask holds the drawn surface only), names FSL-43 and goes on; the default
  refuses there.
- **D13 / gap:** rule A (decided 2026-10-24); the failure case's fill 2026-10-25. No gap.

### FSL-44 † bet expands its arguments unquoted — (e)
- **What:** the `bet` script uses `$1`, `$2`, `$IN`, `$OUT` unquoted: a name with a blank becomes
  several words (an input is "not valid", an output name writes other files), and its option
  loop ends at an empty argument.
- **Evidence:** read; verified (`defects/blank-in-output`, `defects/empty-argument`).
- **fMRIPrep:** none (nipype's names have no blanks).
- **larmorx:** each argument is one word; `reproduce` splits as bash does.
- **D13 / gap:** rule A. No gap.

### FSL-45 † `bet -Z` crops with `imglob` from the PATH — (c)
- **What:** after padding, the script crops `for f in \`imglob ${OUT}*\``, calling `imglob`
  without `${FSLDIR}/bin`: with FSL's programs off the PATH, "imglob: command not found" and the
  outputs stay padded (26 slices for 18); with them on it, every image whose name starts with
  the output's is cropped, other images included.
- **Evidence:** read; verified (`bet/small-fov`, `bet/small-fov-verbose`, `bet/debug`).
- **fMRIPrep:** none (fMRIPrep's image lacks `dc`, so its `bet -Z` and `-R` stop before).
- **larmorx:** crops exactly its own outputs; `reproduce` looks for `imglob` on the PATH and
  does what FSL's script would (the Python API never puts FSL on the PATH).
- **D13 / gap:** rule A. No gap.

### FSL-46 † bet's cleanup removes other files — (c)
- **What:** `rm -f ${OUT}_tmp*` removes files the run did not create.
- **Evidence:** read; verified (`defects/cleanup`).
- **fMRIPrep:** none.
- **larmorx:** removes only the files the run created; `reproduce` the glob.
- **D13 / gap:** rule A. No gap.

### FSL-47 † `--t1only` fills the scalp mask from the wrong surface — (c)
- **What:** `t1only_main` fills the scalp mask from the outer skull mesh's centroid, still in the
  padded working grid, instead of the scalp mesh's (the T1+T2 path uses the scalp's).
- **Evidence:** read; the masks rarely differ (the centroid falls inside the scalp).
- **fMRIPrep:** none (`bet -A` is not used).
- **larmorx:** fills from the scalp surface's centre, named when the mask changes.
- **D13 / gap:** rule A. No gap.

### FSL-48 † A T2 on another grid — (e)
- **What:** betsurf samples the T2 at the T1's voxel coordinates without checking its grid.
- **Evidence:** read; verified (`defects/t2-other-grid`).
- **fMRIPrep:** none.
- **larmorx:** refused by default (other dimensions or voxel sizes); `reproduce` samples as FSL.
- **D13 / gap:** rule B (decided 2026-10-24). No gap.

### FSL-49 † The T1's scalp kept when the T2's lies further out — (c, decided: kept)
- **What:** when the T1's and T2's scalp estimates differ by more than 5 mm and the T2's lies
  further out, betsurf validates the T1's against a T2 search and keeps it; FSL's own source
  calls that line "very suspicious". The other branch replaces the T1's estimate.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** kept (rule C: keeping the nearer, validated estimate is sensible, possibly
  deliberate; nothing to measure a change against).
- **D13 / gap:** – (decided 2026-10-24).

### FSL-50 † One bad profile ends betsurf — (c)
- **What:** in the T1+T2 profile search, when the outer skull search range is empty FSL sets the
  profile aside and then calls `exit(-1)`: status 255, no message, no output, the run lost.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** the profile dropped (what the line before the exit intends), named.
- **D13 / gap:** rule A. No gap.

### FSL-51 † Profile searches read past their samples — (a)
- **What:** the profile class's forward searches dereference the end of its samples, and an empty
  region of interest at the profile's end reads one sample past it, from the vector's unused
  capacity (memory never set).
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** such profiles dropped by default; `reproduce` reads "nothing there" (+∞, NaN) and
  warns that FSL's result depends on memory contents.
- **D13 / gap:** the memory never set is not reproduced; every defined part is. No gap.

### FSL-52 † Mesh files read without checks — (a)
- **What:** betsurf reads OFF, VTK and FreeSurfer meshes with unchecked `>>`: a number that does
  not read leaves later values never set; a face naming a missing vertex reads out of range; an
  empty mesh divides by 0.
- **Evidence:** read; verified (`defects/mesh-unread`, `defects/mesh-missing-vertex`,
  `defects/mesh-truncated`: FSL ends by SIGSEGV).
- **fMRIPrep:** none.
- **larmorx:** refused by default, naming the problem; `reproduce` takes 0 for the values FSL
  never sets, leaves out the faces that name missing vertices, names FSL-52 and writes betsurf's
  outputs (D13's failure case, 2026-10-25); an empty mesh is refused in both modes (nothing to
  fill: the mesh's size is FSL's divisor).
- **D13 / gap:** No gap.

### FSL-53 † A short matrix file — (e)
- **What:** betsurf reads 16 numbers unchecked into an uninitialised matrix; it uses 12.
- **Evidence:** read; verified (`defects/short-matrix`).
- **fMRIPrep:** none.
- **larmorx:** refused by default with fewer than 12 readable numbers; `reproduce` uses 0 for the
  missing ones and warns.
- **D13 / gap:** rule B (decided 2026-10-24). No gap.

### FSL-54 † `--t1only` exits 0 on failure — (e)
- **What:** `main` drops `t1only_main`'s status and exits 0 (a failure FSL's libraries end with
  `exit` themselves, such as an unknown `FSLOUTPUTTYPE`, still exits 1).
- **Evidence:** read; verified (`defects/too-few-t1only`, `defects/no-matrix-t1only`; random
  testing's `difftest/betsurf-t1only-output-type`).
- **fMRIPrep:** none.
- **larmorx:** exits with the failure's status (255); `reproduce` exits 0 where FSL does.
- **D13 / gap:** rule A. No gap.

### FSL-55 † NaN in the 98th percentile — (a)
- **What:** `nth_element` over values that include NaN has no strict weak order; libstdc++'s
  unguarded loops can run outside the vector.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** refused by default; `reproduce` runs libstdc++'s algorithm (the core's `stdsort`,
  written from the SGI STL's published introselect, not from libstdc++) with its loops stopped
  at the vector's ends, and warns.
- **D13 / gap:** rule B (decided 2026-10-24). No gap.

### FSL-56 † NewNifti's `vox_offset` with several extensions — (c)
- **What:** the data offset adds the first extension's size once per extension: with two or more
  of different sizes the data start inside the extensions or after a gap.
- **Evidence:** read; verified (`defects/extensions`: FSL's file cannot be read by nibabel).
- **fMRIPrep:** none (FSL drops extensions unless `FSL_LOAD_NIFTI_EXTENSIONS` is set).
- **larmorx:** the correct offset; `reproduce` FSL's. The switch is `larmorx-nc-core`'s
  (`SaveOptions::reproduce_offset`), so every FSL replica has it (mcflirt's outputs included).
- **D13 / gap:** rule A. No gap.

### FSL-58 † No voxel inside the brain surface — (a)
- **What:** betsurf's robust maximum is the 98th percentile of the T1's values inside the brain
  surface (`nth_element`, then `brain_hist[e98 - 1]`); with no voxel inside (a surface outside the
  image, a tiny image) the list is empty, `e98` is 0, and FSL reads the element before the list's
  start: it crashes (SIGSEGV on a one-voxel constant image) or uses memory it never wrote.
- **Evidence:** read; found by random testing (2026-10-26, `difftest/betsurf-empty-brain`: FSL
  ends by SIGSEGV; the replica had panicked).
- **fMRIPrep:** none.
- **larmorx:** refused by default, naming the entry; `reproduce` takes the percentile as 0 and goes
  on, naming it (D13's failure case).
- **D13 / gap:** No gap.

### FSL-57 † betsurf's smoothness weight set to 0 after its use — (c, decided: kept)
- **What:** where a mask applies, betsurf sets the smoothness weight to 0 after the smoothness term
  was computed: a dead store; the intent was probably to switch the term off there.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** kept (rule C: changing it would change every skull and scalp surface, with nothing
  to measure the change against).
- **D13 / gap:** – (decided 2026-10-24).
```

And the FSL-22 detailed entry, three lines:

- **Evidence:** "the NaN matrix (`validation/probes/fsl-mcflirt-affine-trace/nan_matrix.py`):
  inputs with NaN background voxels in `reproduce`, 32 of 160 cases, the output images of
  volumes whose matrix is NaN padded with another value (2026-10-25); with libstdc++'s order
  reproduced, 160 of 160 give mcflirt's bytes (2026-10-26). The sort equals libstdc++'s
  `std::sort` on 768 seeded vectors with and without NaNs (GCC 11.4 and 14.3;
  `validation/probes/libstdcxx-sort/`)."
- **larmorx:** "NaNs sorted last by default (defined, the same on every platform; D13: undefined
  behaviour is never reproduced by default). `reproduce` exact (SGI STL's sort), 2026-10-26:
  `calc_bval` sorts as libstdc++'s `std::sort` does, move for move
  (`larmorx_nc_core::stdsort::sort_f32_checked`, written from the SGI STL's introsort, under its
  HP/SGI permission notice, with libstdc++'s pivot step and every index checked; where
  libstdc++'s loops would leave the vector, NaNs last, named). The decision of 2026-10-25 ("not
  emulated": libstdc++'s code is GPL-3.0) rested on copying libstdc++; the emulation copies none
  of it (orchestrator, 2026-10-26)."
- **D13 / gap:** "done (2026-10-26): both modes name FSL-22 where NaN border voxels make the
  order undefined (mcflirt's final resampling and `-meanvol`)."

### 1d. The history paragraph (beside "The mcflirt replica")

> **The BET replica** (`larmorx-nc`, built 2026-10-24, merged 2026-10-25 with its FSL libraries
> joined to `larmorx-nc-core`): FSL-40 to FSL-57 from fsl-bet2 2111.8, fsl-meshclass 2111.0 and
> FSL's image libraries ([fsl-bet.md](fsl-bet.md)), each settled by rules A to D: corrected by
> default FSL-40, -43, -44, -45, -46, -47, -50, -51, -54, -56; refused by default (rule B)
> FSL-42, -48, -53, -55; kept (rule C) FSL-49, -57; FSL-41 and FSL-52, where FSL crashes on what
> it leaves undefined, refused by default with `reproduce` filling the undefined part
> deterministically (D13's failure cases), as for FSL-43's crash class and FSL-58 (betsurf with no
> voxel inside the brain surface, found by BET's first random tests, 2026-10-26). FSL-22 reproduced
> exactly in `reproduce` (the core's `calc_bval` with libstdc++'s order, from the SGI STL's
> introsort; 2026-10-26). **Open:** BET's clean-room original (from
> `specs/bet.md`).

## 2. `docs/api/upstream-defects.md`, "What is corrected": rows (after FSL-21)

```
| FSL-40 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | the first `.hdr` of a name is removed wherever it stands; a name with `.img` and no `.hdr` aborts | a trailing image extension removed |
| FSL-41 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | a constant or binary image: FSL crashes reading the median of an empty list | an error naming FSL-41; `reproduce` takes the median as 0 and writes the outputs |
| FSL-42 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | NaN `-f`/`-g`, a negative, NaN or infinite `-r`, `-w` ≤ 0, a non-finite `-c` or search depth used as given | an error naming the value (exit 1; decided 2026-10-24, rule B) |
| FSL-43 | bet2, betsurf | both programs | a mask's fill starts at the surface's centroid unchecked: one voxel outside the image (a single slice) FSL writes and reads outside the volume; further out it crashes | the fill starts at the nearest voxel inside; `reproduce` does FSL's defined part and warns, and skips a fill FSL crashes on |
| FSL-44 | bet | `larmorx fsl bet` | names with blanks split into words; an empty argument ends the options | each argument one word |
| FSL-45 | bet `-Z` | `larmorx fsl bet -Z`, `lx.fsl.bet(variation="small_fov")` | the outputs cropped back by `imglob` from the PATH: left padded when FSL is not on it | exactly its outputs cropped |
| FSL-46 | bet | `larmorx fsl bet` | every `<output>_tmp*` file removed | only the run's own |
| FSL-47 | betsurf `--t1only` | `larmorx fsl betsurf -1 -m`, `lx.fsl.betsurf(masks=True)` | the scalp mask filled from the outer skull surface's centre in the padded grid | from the scalp surface's |
| FSL-48 | betsurf | `larmorx fsl betsurf`, `lx.fsl.betsurf(t2=...)` | a T2 on another grid sampled at the T1's voxel coordinates | an error (decided 2026-10-24, rule B) |
| FSL-50 | betsurf | both | one profile with an empty search range ends the program (255, no message, no output) | the profile dropped |
| FSL-51 | betsurf | both | profile searches read past the samples, into memory never set | the profile dropped; `reproduce` reads "nothing there" and warns |
| FSL-52 | betsurf | both | a mesh file read without checks: FSL crashes on values it never set or faces naming missing vertices | an error naming the problem; `reproduce` takes 0 for the values never set, leaves out those faces and writes the outputs |
| FSL-53 | betsurf | both | a matrix with fewer than 12 readable numbers: entries never set | an error (decided 2026-10-24, rule B); `reproduce` uses 0 |
| FSL-54 | betsurf `--t1only` | `larmorx fsl betsurf -1` | exit status 0 after a failure | the failure's status (255) |
| FSL-55 | betsurf | both | NaN inside the brain surface: the 98th percentile undefined | an error (decided 2026-10-24, rule B) |
| FSL-58 | betsurf | both | no voxel of the T1 inside the brain surface: FSL reads the 98th percentile of an empty list and crashes | an error naming FSL-58; `reproduce` takes it as 0 |
| FSL-56 | FSL's NIfTI writer | every FSL replica (`larmorx fsl bet2`, `bet`, `betsurf`, `mcflirt`) with `FSL_LOAD_NIFTI_EXTENSIONS` | several extensions of different sizes: a wrong `vox_offset` | the correct offset |
```

## 3. `README.md`, the validation index and the site's status table (generated)

These tables are generated since the random-testing part (`python -m larmorx_validation tables`,
`validation/tools.toml`). BET's rows are in the branch: `validation/tools.toml` gained the tool
`fsl-bet` (after `mri-hmc`) and the records `fsl-bet-replica` and `fsl-bet-replica-full` (after
`fsl-mcflirt-replica`); `larmorx_validation.tables` gained BET's metrics (`fmriprep`: fMRIPrep's
calls; `filled` and `pending`: the two kinds of expected divergence); README's table and the
validation index were regenerated from the committed records (`tables --check` passes). At the
merge: `python -m larmorx_validation tables --site <site>` for the site's `data/status.yml`
(BET's row comes from the same description).

Elsewhere in `README.md` (not generated):
- The roadmap, item 2: "**FSL-compatible tools:** FAST, FLIRT with BBR, topup, BET's clean-room
  original (the mcflirt and BET replicas are done)."
- The repository map, `crates-nc/`: "the FSL replicas (`larmorx-nc`, FSL Licence,
  non-commercial: mcflirt, BET; the shared core `larmorx-nc-core`), a separate Cargo workspace
  with FSL's original source in `fsl-source/`".
- Where `larmorx-nc` is introduced (installation), if anywhere: "`larmorx-nc` holds the FSL
  replicas under the FSL Licence's terms (non-commercial use only): `pip install larmorx-nc`.
  Until BET's clean-room original exists, `lx.fsl.bet` and `lx.fsl.betsurf` need it."

## 4. `docs/licensing.md`

In the `larmorx-nc` packaging paragraph (l. 54 ff.), replace "the FSL, Armadillo, fdlibm and Arm
notices (`crates-nc/NOTICE`)" by "the FSL, Armadillo, fslpy, fdlibm and Arm notices
(`crates-nc/NOTICE`)", and add after the sentence on FSL's original source:

> BET (2026-10-25) is the second FSL replica: `larmorx-nc fsl bet`, `bet2` and `betsurf`,
> translated from FSL 6.0.7.17's fsl-bet2, fsl-meshclass, fsl-newimage, fsl-newnifti,
> fsl-miscmaths, fsl-utils, fsl-znzlib and fsl-avwutils, whose original source is in
> `crates-nc/fsl-source/` beside mcflirt's (one `MANIFEST.tsv`). Also translated, each with its
> notice in `crates-nc/NOTICE`: fslpy 3.21.1's `imtest`, `remove_ext` and `imglob` (Apache-2.0),
> fdlibm's `tanh` and `expm1` (Sun's notice), glibc 2.35's `pow` and `exp` from Arm's Optimized
> Routines v19.11 (MIT: the code glibc ships, as for the float functions; no LGPL code), and
> bet2's `--ct` conversion, which FSL adapted from Chris Rorden's BSD-2-clause `clinical_h2c.m`.

## 5. `docs/validation/README.md` (the index)

Generated (section 3). Outside the generated table, the commands section (after the
`fsl-mcflirt-replica` line):

```
python -m larmorx_validation parity fsl-bet --implementation replica --tier standard --out docs/validation   # needs FSL at ~/fsl and fMRIPrep's image
python -m larmorx_validation parity fsl-bet --implementation replica --tier full --out <dir>   # then <dir>/fsl-bet-replica.* -> docs/validation/fsl-bet-replica-full.*
```

## 6. `docs/validation/golden.md`

- The table's crates-nc column, cases: "`crates-nc/larmorx-nc-cli/tests/golden.rs`: 33 cases, 242
  outputs" (was 23, 179); the main column: `tests/golden/registry.py`: 153 cases, 892 outputs (was 152, 890: `py.fsl.read_vtk`).
- The coverage table, a row after `larmorx-nc fsl mcflirt`:
  ```
  | `larmorx-nc fsl bet`, `bet2`, `betsurf` | 10 | bet2's outputs (brain, mask, outline, skull, VTK mesh; `-f 0.4`), its threshold, gradient, centre and smoothness options (NIFTI_GZ), `--ct` (NIFTI_PAIR; the self-intersection passes), FSL-40 reproduced; `bet -R`, `-F` (4D), `-Z` corrected and reproduced (FSL-45); `betsurf --t1only -o -m -s` and with the T2 and `-p 1`. The inputs: a head of 32 x 32 x 24 voxels of 2 mm (brain, CSF, skull, scalp) from SplitMix64 and integer arithmetic, its T2 contrast, 4 volumes of it, a slab of 10 slices, and bet2's own mesh of the head; every file written fingerprinted (images by header and data, meshes as text); each case checks the defects named on standard error. `PATH` is emptied for the runs (FSL-45's branch looks for `imglob` on it) |
  ```
- `lx.fsl` (the main package's registry): a row
  ```
  | `lx.fsl` | 1 | `read_vtk` (a VTK mesh as FSL writes it, read back); `lx.fsl.bet` and `lx.fsl.betsurf` run only the replica: their cases are `larmorx-nc`'s |
  ```
- The note under the table: "`larmorx-nc`'s checksums: BET's 10 cases recorded at BET's merge
  (from ee72d36, then again from bb72fb6 after the memory and random-testing fixes, every checksum
  unchanged); mcflirt's 179 unchanged. The FSL core and BET are built at `opt-level = 1` in the
  dev profile so that the debug `cargo test` of CI runs BET's cases in about 30 s (130 s
  unoptimised)."
- The main package: `py.fsl.read_vtk` (part 1, recorded without parity) re-recorded with the parity
  smoke tier: recorded from `d54d773` (commit `04dd94d`), the parity smoke tier passing (2461 passed), every checksum unchanged.

## 7. `docs/validation/oracle-cache.md`

- "The runners" table, a row after `mri-hmc`:
  ```
  | `fsl-bet` | `fsl_bet.oracle_run` (FSL in `~/fsl`; fMRIPrep's FSL through its image) | 1 (FSL's BET is single-threaded) | FSL-43 and FSL-53 (memory FSL never wrote), FSL-46 (the cache replays no removal), crashes; images named without their extension through `inputs=` |
  ```
- "The key", the inputs row: append "; files a program reads that its arguments do not name as
  files, when the suite names them (`inputs=`: FSL's images named without their extension,
  `bet in out` reading `in.nii.gz`), count by their content alike".
- "Limits", the first bullet: "An input a program reads without its path in the arguments (a
  file named inside another file) counts by the path written there, not its content, unless the
  suite names it (`inputs=`); `verify` catches a change."
- "Limits", replace the bullet "Another agent's FSL suite keeps its own inputs in
  `<workspace>/oracles/cache/fsl-bet-inputs` ..." by: "BET's suite keeps the inputs FSL derives
  (bet2's meshes and FLIRT's matrices of the real images) in `<workspace>/oracles/inputs/fsl-bet/`
  (since 2026-10-25), outside the cache's directory, as every suite keeps its inputs."

## 8. `docs/next-steps.md`, item 7 (FSL)

Replace "**BET:** the replica and its spec are built, to be merged after its FSL library code
moves into `larmorx-nc-core`; then BET's clean-room original, FLIRT with BBR, FAST, topup." by:

> **BET: the replica done** (2026-10-25; `larmorx-nc fsl bet`, `bet2`, `betsurf`;
> `lx.fsl.bet`, `lx.fsl.betsurf`; records [fsl-bet-replica](validation/fsl-bet-replica.md):
> 115 of 115 compared cases bit-identical, fMRIPrep's 29 calls included, and
> [fsl-bet-replica-full](validation/fsl-bet-replica-full.md): 129 of 129, 364 of 364 files), its
> FSL libraries joined to mcflirt's in `larmorx-nc-core` (one translation of each); under random
> testing (`fsl-bet`, four trial runs, every difference fixed) and the memory rule (below FSL's in
> every measured call). Next for BET: the clean-room original from `specs/bet.md` (an agent that
> has never read FSL's source; built for speed: the spec's §10), and `bet -S`, `-B`, `-A`, `-A2`
> once standard_space_roi, FLIRT and FAST are replicated. Then FLIRT with BBR, FAST, topup.
> Follow-ups: BET in the nightly random testing (its random-testing panel on the site after the
> first night that includes it); a benchmark report (FSL, the replica; bet2 and betsurf on the
> data panel); `robustfov` (in fsl-bet2, not translated).

## 9. `docs/decision-log.md` (2026-10-25)

> ## 2026-10-25: BET's merge
>
> - **One translation of each FSL library** (`larmorx-nc-core`): BET's and mcflirt's translations
>   of newimage, NewNifti, Armadillo, fdlibm and the C library were joined into one (newimage's
>   `volume<T>` generic over its element types); every case of both replicas' records, golden
>   tests and NaN matrices gives the same bytes with it. The next FSL replicas build on the core's
>   API (its `PROVENANCE.md`).
> - **BET's questionable behaviours, settled by rules A to D** (no decision needed from the user):
>   A (corrected) FSL-40, -43, -44, -45, -46, -47, -50, -51, -54, -56; B (refused by default)
>   FSL-42, -48, -53, -55; C (kept and documented) FSL-49, -57. Where FSL crashes on what it leaves
>   undefined (FSL-41, FSL-52, FSL-43's crash class) the default refuses and `reproduce` keeps
>   every defined part and fills the rest deterministically, naming the defect (D13's rule of
>   2026-10-11 for failure cases).
> - **The Python API never puts FSL on the PATH** and passes no `FSL*` variable of the process:
>   `lx.fsl.bet(..., upstream_defects="reproduce")` gives the result of FSL's `bet` run with FSL
>   off the PATH (FSL-45: `-Z`'s outputs left padded), which is deterministic.
> - **`pow` and `exp` come from Arm's Optimized Routines** (MIT), as the float functions do: no
>   LGPL code in the FSL-licensed package (the rule of 2026-10-24 applied to BET's maths; glibc's
>   are the same code, `validation/investigations/fsl-bet/arm_glibc_pow_exp.py`).
> - **The FSL core and BET at `opt-level = 1` in debug builds**, so the golden tests stay small in
>   CI (BET's 10 cases: about 30 s instead of 130 s); the NaN matrices compare release builds with
>   unoptimised debug builds (`--config` overrides).
> - **Inside `bet`, `reproduce` gives the script's failure where FSL's bet2 crashes** (FSL-41 on a
>   constant image: bash reports the signal, the ERR trap prints `bet failed during command:...`
>   and exits 1): that is FSL's defined outcome; bet2 run alone still fills (D13's failure case).
> - **FSL-58** (betsurf with no voxel of the T1 inside the brain surface reads its 98th percentile
>   from an empty list; found by random testing): a D13 failure case like FSL-41: refused by
>   default, `reproduce` takes it as 0.
> - **BET needs no more memory than FSL** in any measured call (`tests/parity/test_memory_peak.py`):
>   saving streams a volume's values instead of copying them, which every FSL replica shares.

## 10. `CLAUDE.md`

- Hard rule 1, the list of source-derived findings a clean-room implementer must not read
  (`docs/findings/afni-tshift.md`, ..., `fsl-mcflirt.md`): add `fsl-bet.md`.
- "Where things are", the table, FSL row, after the mcflirt replica's text: "; the **BET** replica
  (`larmorx-nc fsl bet`, `bet2`, `betsurf`; `lx.fsl.bet`, `lx.fsl.betsurf`; `bet -S`, `-B`, `-A`,
  `-A2` pending FLIRT and FAST): every file FSL 6.0.7.17 writes byte for byte in every compared
  case (129 of 129, 364 files, fMRIPrep's 29 BET calls on the twelve oracle runs with BOLD runs
  included); its FSL libraries are the core's, shared with mcflirt" | records add
  `fsl-bet-replica`, `fsl-bet-replica-full`.
- After the "mcflirt replica merged" paragraph: "**The BET replica merged** (2026-10-25; FSL, item
  7): `larmorx-nc fsl bet|bet2|betsurf`, bit-identical to FSL 6.0.7.17 (115 of 115 compared cases
  in the standard tier, 129 of 129 and 364 of 364 files in the full tier); BET's and mcflirt's FSL
  libraries joined in `larmorx-nc-core` (one translation of each, its API in the core's
  `PROVENANCE.md`); FSL-40 to FSL-58 by rules A to D, FSL-41, -52, -58 and FSL-43's crash class as
  D13 failure cases; NaN and infinite voxels with SSE's bits on every platform (BET's NaN matrix);
  under random testing (`fsl-bet`; four trial runs, every difference fixed, seven permanent cases)
  and below FSL's peak memory in every measured call; `pow` and `exp` from Arm's MIT routines."
- "Next": "BET's clean-room original (from `specs/bet.md`), then FLIRT with BBR, FAST, topup."
- "Build-cache seed": `.claude/worktrees/agent-a38ebb82788afa285/` (`target/`, `crates-nc/target/`;
  no `crates-gpl/target/` beyond the gates' build), if it is the newest merged.

## 11. The site (`<scratch>/site`, branch `fsl-bet-evidence`)

The branch `fsl-bet-evidence` is the site's main `ebf30bd` (random testing's evidence) plus one
commit, `825dc38`: `content/tools/fsl-bet.md` (new), `static/evidence/fsl-bet.json` (new),
`static/evidence/blobs.sha256` (+27 lines: BET's images). The local `evidence-blobs` is `11febbc`
(322 blobs: the 295 of `ebf30bd`'s manifest and BET's 27; made by `tools/blobs.py commit`): push it
(`--force`) before `main`. The page was exported from larmorx `d54d773`'s tree (exporter: the
current one, with the data credits) from the final records and the evidence cache computed with
`larmorx-nc` (`oracles/figures-cache/fsl-bet/`: ds000005's BOLD reference and T1w, the synthetic
head for betsurf, the data panel's ds000228 three-year-old as three full-resolution slices).

Checked:
- the build: 20 pages, 0 problems, 86 links into the main repository checked against the branch;
- the number check (`tools/review/shoot.mjs --eval`, `LX_EVIDENCE.check`): 44 of 44 numbers agree;
  50 viewers, none failing; no failed panel;
- the inventory (`<scratch>/inventory.md`, `inventory.json`; before: `ebf30bd` as built, after: the
  branch as built): every other page unchanged (0 shared JSON values changed, 0 removed, 0 shared
  shown numbers changed, 0 images lost); BET's page new: 4 examples (3 whole volumes, 1 panel case
  as slices), 22 panels, 20 comparisons, 550 metric values, 173 cases, 27 images, 65 shown numbers;
- screenshots (`<scratch>/shots-final/`): the whole page and each worked example at 1440 and 390,
  light and dark; no face (the BOLD references and the T1w inside their brain masks dilated by 2
  voxels; betsurf's scalp only on the synthetic head);
- size: BET's page 5.3 MB (27 blobs, 5.1 MB); the site 168.6 MB.

Not on the page yet: **its Random testing panel** (`{{ ev.random_testing('/tools/fsl-bet/') }}`),
which needs a night that includes `fsl-bet` in `static/evidence/difftest.json` (the build refuses
the macro for a page the night does not list). After the next night with `--tools all`: re-export
the random-testing evidence, then add before "## Every case" on BET's page:

```
## Random testing {#random-testing}

Random testing runs random valid inputs and option combinations through the original program and LARMORX, a night at a time: inputs from seeded plain arithmetic (up to 32³ voxels; orientations, data types, scale factors and value ranges of every kind), with options drawn in random combinations from those the program accepts ([how random testing works](repo:docs/validation/difftest.md)). A difference is shrunk to the smallest input and fewest options that still show it, then fixed and kept as a parity case, registered as a defect of the original, or reported. The latest night:

{{ ev.random_testing('/tools/fsl-bet/') }}
```

### 11a. `site.yml`, the tools list (after 3dUnifize, before Head-motion correction)

```yaml
  - { title: BET, url: "/tools/fsl-bet/", eco: fsl,
      summary: "Brain extraction (bet, bet2) and the skull and scalp (betsurf): an exact replica in larmorx-nc; the clean-room original planned." }
```

### 11b. `data/status.yml`

Generated: `python -m larmorx_validation tables --site <site>` from the merged larmorx (BET's
`[[tool]]` in `validation/tools.toml`). In the `planned` list, the FSL entry's text: "BET's
clean-room original, FAST, FLIRT with BBR, topup."

### 11c. `content/validation.md` and `static/evidence/summary.json`

`validation/src/larmorx_validation/evidence/summary.py` has BET's row (the replica, record
`fsl-bet-replica`, the standard tier); re-export the summary from the merge (`python -m larmorx_validation
evidence summary --site <site>`).

## 12. Code outside BET's crates changed in the branch (minimal, for the tool to work)

- `crates/larmorx-core/src/defects.rs`: `FSL_22` (C's entry, now named by mcflirt), `FSL_40` to
  `FSL_48`, `FSL_50` to `FSL_56`, `FSL_58` (FSL-41, -52 and -58 carry switches: `reproduce`
  fills; FSL-43's tool "bet2, betsurf").
- `crates/larmorx-cli/src/{lib.rs,replica.rs}`: the `fsl` family lists `mcflirt`, `bet`, `bet2`,
  `betsurf`; `LARMORX_NC` runs them (`original: false` for BET's three).
- `crates-nc/Cargo.toml` (members, `larmorx-nc-bet`, the dev profile's `opt-level`),
  `crates-nc/Cargo.lock`; `crates-nc/larmorx-nc-cli` (`FSL_TOOLS`, its help test, `golden.rs`
  with `BET_CASES`, `golden.tsv`); `crates-nc/larmorx-nc-mcflirt/src/run.rs` (the core's
  `SaveOptions`, `read_volume::<f32>`, FSL-22's `NanOrder` and notice, the `-stats` sums' operand order); `crates-nc/{NOTICE,README.md}`,
  `crates-nc/fsl-source/` (BET's FSL files added: one `MANIFEST.tsv`, its README),
  `scripts/fsl_source.py` (BET's packages).
- `python/larmorx/__init__.py` (`lx.fsl`, lazily loaded like the other subpackages since main's
  PEP 562 change), `python/larmorx/fsl/`, `python/larmorx/_replica.py`,
  `tests/python/conftest.py`, `tests/python/test_fsl_bet.py`.
- `tests/golden/registry.py` (`NC_GOLDEN` → `crates-nc/larmorx-nc-cli/tests/golden.rs`;
  `py.fsl.read_vtk`), `tests/golden/test_golden.py`, `tests/golden/golden.json`.
- `validation/src/larmorx_validation/oracle_cache/{core,key}.py` (`inputs=`),
  `validation/tests/test_oracle_cache.py`; `figures/cache.py` (`crates-nc` paths),
  `validation/tests/test_figures_cache_paths.py`; `__main__.py` (`fsl-bet`), `parity/fsl_bet.py`, `tests/parity/test_fsl_bet_replica.py`,
  `evidence/__init__.py`, `evidence/fsl_bet.py`, `evidence/summary.py` (BET's row).
- Random testing: `validation/src/larmorx_validation/difftest/tools/fsl_bet.py` (new) and its
  registration in `difftest/tools/__init__.py`; `difftest/permanent.py` (`SUITES`:
  `fsl-bet-replica`); `difftest/runner.py` (`UNDEFINED_DEFECTS`: FSL-51, -53, -55);
  `validation/difftest/cases/fsl-bet/` (seven permanent cases), `validation/difftest/triage.toml`
  (BET's entries), `docs/validation/difftest.md` (BET's row in the tools table; "FSL's BET is
  one").
- `tests/parity/test_memory_peak.py` (`BET_CASES`; `_peak` takes a working directory).
- `validation/tools.toml` (BET's tool and its two records), `validation/src/larmorx_validation/
  tables.py` (BET's metrics); README's tool table and the validation index regenerated by
  `python -m larmorx_validation tables` (nothing else of either file edited).
- `validation/probes/fsl-bet-nan/` (the NaN matrix), `validation/investigations/fsl-bet/`
  (timings, `arm_glibc_pow_exp.py`).
- `upstream.tsv`: fsl-bet2 2111.8, fsl-meshclass 2111.0, fsl-avwutils 2209.3, fslpy 3.21.1
  (Arm's Optimized Routines v19.11 is not pinned there, though C's float functions and now
  `pow`/`exp` come from it: a row may be worth adding, with a location code meaning "not
  cloned").

## 13. Facts for the orchestrator's own checks

- **Records** from the clean commit `8e0059a` (main `f86ce4f` merged), `--jobs 2` in an 8 GiB
  box, on a shared machine (timings not a benchmark): standard 159 cases in about 4 minutes, full
  173 in about 6; FSL's runs about 220 from the oracle cache and 18 run each time: FSL's 8 crashes
  (never stored), the 8 of FSL-43's cases and one each of FSL-46 and FSL-53 that the suite never
  caches (random testing's permanent cases name FSL-43 as silent, which `case_never` reads as
  memory FSL never wrote: they always run, tiny). Your standard-tier re-run serves the rest from
  the cache.
- **fMRIPrep's BET calls**: 29 on the twelve oracle runs with BOLD runs (`enhance_and_skullstrip_bold`
  per BOLD run, `skullstrip_bold` where field maps correct the distortion); each also against
  fMRIPrep's own outputs of the node (58 of 58 files) and fMRIPrep's image's FSL (38 runs).
- **mcflirt's suite with the joined core**: the standard tier with the final `larmorx-nc` (`--jobs 1`, an 8 GiB box): 196 cases, 175 pass, 21 rejected by both, 0 failures; `compare_records` against `fsl-mcflirt-replica`: 0 statuses and 0 details differ; new: its 6 permanent random-testing cases (the record predates them); gone: 4 cases of the full tier only. Its full tier with the joined core (before the memory changes, `--jobs 2`): 172 pass, 21 both, and ds004958's 600-volume call killed by the 8 GiB box at two jobs, which alone (`--jobs 1`) passes (601 of 601 files): run that tier at one job or in a larger box
- **Golden**: crates-nc re-recorded from `bb72fb6` (BET's 10 cases, 63 outputs, unchanged since
  `ee72d36`; mcflirt's 179 unchanged); the main package's `golden.json` from `d54d773` (commit `04dd94d`) with the parity smoke
  tier passing (`pytest tests/parity`: 2461 passed in 30 minutes, BET's memory cases included; 153 cases, 892 outputs, every checksum unchanged).
- **Gates** (`gates-rust.sh all`, the Python gates): `gates-rust.sh all` at `c89b16e` (root, crates-gpl, crates-nc: fmt, clippy -D warnings, tests) all OK; after it only Python, documents, records and checksums changed (crates-nc's golden test passed again when re-recording at `bb72fb6`); the Python gates at `04dd94d`: pytest tests/python tests/golden validation/tests 599 passed, ruff check, ruff format --check, check_spdx OK; `pytest tests/parity` (smoke) 2461 passed at `d54d773`. FSL-22's commit `82e4f9e`: `gates-rust.sh nc` (fmt, clippy -D warnings, tests, the crates-nc golden suite with every checksum unchanged) OK on its tree; ruff and check_spdx OK; C's NaN matrix 160 of 160.
- **The NaN matrix** (`validation/probes/fsl-bet-nan`, release and unoptimised debug builds of
  `2e0f5ad`, whose code the final commit keeps): 60 cases, 0 differ, FSL crashing in 18, no
  trapped run over its time limit; about 1 h 20 min (the trapped runs single-step).
- **mcflirt's full tier in an 8 GiB box at `--jobs 2`**: the panel's ds004958 call (600 volumes)
  was killed by the box (SIGKILL, a memory limit, not the replica): alone (`--jobs 1`) it passes,
  601 of 601 files. Run that tier at `--jobs 1` or in a larger box.
- **The evidence cache** (`oracles/figures-cache/fsl-bet/`): three entries, computed at `ccbec99`
  with `larmorx-nc` (the stand-in's archived in
  `<workspace>/archive/2026-10-25-fsl-bet-evidence-entries/`, README there).
- **Derived inputs**: `<workspace>/oracles/inputs/fsl-bet/` (bet2 meshes and FLIRT matrices of the
  real images, made by FSL; the suite makes them if missing). The old
  `oracles/cache/fsl-bet-inputs/` was moved there.
- **fMRIPrep's image has no `dc`**: its `bet -R` and `-Z` stop (`validation/investigations/fsl-bet`).
- **Build-cache seed**: this worktree's `target/` and `crates-nc/target/` (built with the joined
  core; release and debug, the dev profile at opt-level 1 for the FSL crates).

## 14. A question the rules leave to the orchestrator

- **FSL-22: settled** (the orchestrator, 2026-10-26: "yes, if it's exact"). It is exact: 160 of
  160 NaN-matrix cases (§1b, §1c). For the decision log: "FSL-22's `reproduce` sorts NaN border
  values in libstdc++'s order (2026-10-26). The decision of 2026-10-25 ('not emulated', as
  libstdc++'s code is GPL-3.0) assumed copying libstdc++; `stdsort.rs` is written from the SGI
  STL's introsort (HP/SGI permission notice, in `crates-nc/NOTICE`) and copies no libstdc++
  code. NaNs stay last by default (D13)."
  One nuance to weigh: the sequence that gives FSL's bytes is libstdc++'s, which refines the SGI
  STL's pivot step (the SGI STL partitions around a copy of the median of the first, middle and
  last elements; libstdc++ moves the median of the second, middle and last to the first place
  and partitions the rest). `stdsort.rs` expresses that step from its description (the same
  basis as meshclass's sweep, which has used it since BET's part 1); it holds no libstdc++
  text. If that step should count as libstdc++'s, it concerns the mesh sweep as much as FSL-22.
