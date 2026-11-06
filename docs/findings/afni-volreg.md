# Findings: AFNI 3dvolreg 26.3.05 (the replica)

**Restricted** (CLAUDE.md rule 1): written from AFNI's source while translating the replica
(`crates/larmorx-afni-replica`: `vecmat`, `shear`, `eispack`, `rot3d`, `blur`, `lsqfit`,
`align3d`, `volreg`, `cli::volreg`). Whoever writes the clean-room original of 3dvolreg must not
read this file; the behaviour it needs is in `specs/3dvolreg.md`. References are to the pinned
release (`tags/AFNI-26.3.05/src`; the same text in 25.2.09 for every routine below).

## What decides the result, bit for bit

- **`rot_to_matrix` calls `sincos` for its third axis and `sin`, `cos` for the first two**
  (`thd_shear3d.c`): GCC turns the pair for one angle into one `sincos` call only where both
  are computed from the same argument in one expression. The replica calls glibc's routines as
  AFNI's build does (`vecmat::rot_to_matrix`, `Libc`), so the rotation matrices of the
  registration, of `-1Dmatrix_save` and of the coarse search are AFNI's to the last bit.
- **`fir_gaussian_load`'s `ii*dx` is a float product** (an `int` times a `float`), the weights
  then summed in double: the FIR blur matches AFNI only with that rounding.
- **The blur's row-end mirrors differ by axis** (`edt_blur.c`): `BMIRROR` (x and y) mirrors the
  first point about itself but repeats the last point (`r[M+N-1+m] = r[M+N-m]`), `RMIRROR` (z)
  mirrors about both end points, and `fir_blury`'s path for `nx >= 512` mirrors one point at
  each end and pads the rest with zeros. The comments call both "fully mirror". **Rule D,
  measured (2026-11-06, `examples/volreg_mirrors.rs`):** mirroring every row about its end
  points on every axis and size moves the motion parameters by at most 1.5e-5 degrees and
  1.1e-5 mm on synthetic series, 5e-7 degrees and 2.4e-7 mm on fMRIPrep's inputs re-made from
  ds000005, ds003592 and ds005454 (`-Fourier -twopass -zpad 4` and the default heptic run), the
  maximum displacements by 7e-6 mm, and the registered volumes by at most 1.4e-6 of their
  largest value; on a 512-wide series (the one-point path) by 2e-7 degrees and 7e-7 mm. Far
  below the 1e-4 AFNI prints and any variability of the method: **rule C**, kept in both modes
  and documented, no defect ID (AFNI-91, planned for it, stays free).
- **`mri_3dalign_wtrimming(1)` is always on** (since 2004): `-wtrim` only prints its notice.
- **3dvolreg's automask clip fraction leaks** (AFNI-92): `THD_automask_set_clipfrac(0.333f)`
  for `-maxdisp` stays set for the rest of the run, so `-twopass`'s coarse rotation search
  (`new_get_best_shiftrot`, whose automask would use 0.5) depends on `-nomaxdisp`.
- **The first pass's weight**: `if( VL_imwt != NULL || !VL_twosum || VL_imbase ==
  DSET_BRICK(VL_dset,0) )` never takes its last branch (`VL_imbase` is a scaled copy, never
  the sub-brick itself), so with `-twopass` the first-pass weight is always the blurred sum of
  the blurred base and the shifted first sub-brick, clipped at 2.5 % of its maximum, also when
  the base is sub-brick 0 (fMRIPrep's call). **Rule C**: AFNI's behaviour since 2000, fMRIPrep's
  default path relies on it, and the first pass only seeds the final one: kept in both modes.
- **`-nocoarserot` is `-noclip`** (AFNI-86): `strncmp(Argv[Iarg],"-noclip",4)` is tested before
  `strcmp(...,"-nocoarserot")`, so the four letters `-noc` win.

## The option scan

- **Options given last** (AFNI-88): `Argv[++Iarg]` is read without a check by `-params`,
  `-maxite`, `-x_thresh`, `-rot_thresh`, `-delta`, `-final`, `-twoblur`, `-zpad`, `-prefix`,
  `-dfile`, `-1Dfile`, `-base`, `-coarse`, `-edging`, `-weight`, `-tshift`, `-maxdisp1D`: a NULL
  pointer, and AFNI dies with "Fatal Signal 11 (SIGSEGV)" (exit status 1, its crash banner).
  `-savedisp` checks `++Iarg > Argc`, one too late (`strdup(NULL)`: the same crash).
  `-1Dmatrix_save` checks properly ("no argument after '-1Dmatrix_save'!"). `-input`/`-source`
  pass the NULL to `THD_open_dataset`, which fails: "Can't open input dataset '(null)'";
  `-rotparent`, `-gridparent` to `THD_open_one_dataset`: "** Can't open -rotparent dataset!"
  (with `ERREX`'s extra newline). The replica gives AFNI's message in the last cases and an
  error naming AFNI-88 in the others, in both modes.
- **Fixed buffers** (AFNI-90): `VL_prefix`, `VL_dfile`, `VL_1Dfile` are `char[256]` filled by
  `strcpy`; `-weight`'s dataset name goes to `char dname[256]` by `memcpy`. AFNI's build
  (`_FORTIFY_SOURCE`) aborts on 256 bytes or more: "*** buffer overflow detected ***:
  terminated", then its handler's "Fatal Signal 6 (SIGABRT)" banner, exit status 1 (checked:
  251 + `.nii` runs, 252 + `.nii` aborts). `-maxdisp1D`'s `MCW_strncpy(...,998)` cuts at 997
  bytes. By default the replica keeps every name whole (rule A); `reproduce` refuses the long
  ones at the same point of the scan and cuts `-maxdisp1D`'s.
- **`-base 0 -base 3`** (AFNI-95): the "two -base" test is `VL_imbase != NULL || VL_nbase > 0`.
- **The datasets of the scan**: `-input`, `-base dset`, `-weight`, `-rotparent`, `-gridparent`
  are opened (and `-base`'s, `-weight`'s loaded) where the scan meets them, so their messages
  (the obliquity report, "Reading in base dataset" only when `-verbose` came first, "-base
  dataset ... has more than 1 sub-brick") come in the scan's order. `DSET_BRIKNAME` of a NIfTI
  dataset is its name as given; `DSET_HEADNAME` (the mismatch messages) adds `./` to a name
  without a directory; `ORIENT_shortstr` is `R-L`, not `R2L`.
- **`THD_report_obliquity`'s long warning comes once per run** (`first_oblique`); the
  "Oblique dataset: ... degrees from plumb" line for each of the first 20 oblique datasets.
- **`-@`** (`addto_args`): only as the last argument, after AFNI's global options were taken
  out (`mainENTRY`), so global options read from standard input are unknown options; nothing
  read (or only blanks) leaves the `-@` in place, which the scan then reports.
- **`-maxdisp1D -`** writes the maximum displacements to standard output and their changes to
  a file named `-_delt` (`modify_afni_prefix`), as the help's "mm_delt" says literally. **Rule
  C**: kept, documented.
- **The help is wrong twice** (rule C, kept): "-twopass resets the -maxite parameter to 66" (only
  a `-maxite` below 10 is raised; the default 23 stays), and "the environment variable
  AFNI_ROTA_ZPAD can be used to set a nonzero default" (3dvolreg sets the padding itself with
  `THD_rota_setpad`; the variable is never read).

## -tshift, -savedisp, -rotparent, -gridparent, -twodup (relay 2)

- **`-tshift ii`** calls `THD_dataset_tshift` (`thd_tshift.c`): 3dTshift's default (Fourier,
  `-rlt` 0: the trend removed and restored, both clips) with the mean slice offset as the time
  origin, on the input in place before anything reads it (the base, the `-maxdisp` automask,
  `-wtinp`'s weight and both passes use the shifted data). 3dvolreg's own checks come first
  (one sub-brick; fewer slice offsets than slices; `ii > nvals-5`; an internal base before
  `ii` only warns); `THD_dataset_tshift`'s failures (offsets outside 0..TR or all equal, TR 0)
  print "some error during -tshift operation!" and the run goes on unshifted. On success the
  output has no slice offsets and time origin 0. AFNI-01 (a negative brick factor) applies on
  this path as in 3dTshift.
- **`-savedisp sss`**: three float datasets (`sss_DX`, `_DY`, `_DZ`, the suffix before
  `.nii`/`.nii.gz`) on the output's grid (`EDIT_empty_copy(new_dset)` before `-twodup` moves its
  origin), each voxel's displacement by the volume's `VOLREG_MATVEC` (in the dataset's axis
  order, about `DSET_XCEN`) at the input's voxel coordinates; written after the registered
  dataset. `-SAVEDISP` normalises each (`THD_linear_detrend`, `THD_normmax`) and adds 16
  products of Legendre polynomials `P1..P3` of them (float: `1.5f*x*x-0.5f`,
  `(2.5f*x*x-1.5f)*x`), each normalised again, written in `VL_create_disprod`'s order.
- **`-rotparent rset`** reads `VOLREG_MATVEC_000000` (12 floats, at least), `VOLREG_CENTER_OLD`
  and `VOLREG_CENTER_BASE` from the dataset's attributes (a NIfTI file's AFNI extension: only
  AFNI's own 3dvolreg writes them; larmorx writes no extension). A dataset without the last two
  is read through a NULL pointer (`atr->fl[0]`), and one with fewer than 3 numbers past them
  (**AFNI-96**, rule B: an error in both modes). The matrix must be orthogonal (the sum of
  `|RᵀR - I|`, stored as float, at most 0.01). The shift is moved by `R(E2 - S2) - (E1 - S1)`
  (the centres of the output grid, of the gridparent, and the rotparent's two), then both are
  converted to the dataset's axis order. Each volume is registered without output
  (`mri_3dalign_method(..., noreg=1, ...)`), and its parameters, printed with `%.4f` and read
  back by `THD_rotcom_to_matvec` (with `-ashift`), are composed after the rotparent's
  (`MUL_DVECMAT(vm2, vm1)`) and applied to the (zero-padded) input with `-final`'s method;
  the clip uses the padded input's range (its zeros included). `rmsold`, `rmsnew` are 0.
  Slice offsets move by `rint(tvec.z / |dz|)` slices (zeros shifted in).
- **`-gridparent gset`** (only with `-rotparent`, else a warning and ignored): the same x, y
  size, spacing and orientation (`THD_dataset_mismatch`'s two tests), at least as many slices;
  more are zero-padded (`THD_zeropad`, `ZPAD_EMPTY`: half before, the rest after, by the sign
  of the slice axis), which replaces the output's matrix by the cardinal one (an oblique input
  becomes plumb); the output then takes the gridparent's origin (directly, so an oblique
  matrix without padding stays and only `qoffset` moves).
- **`-twodup`** needs `-twopass` and an external base, and no `-rotparent` (and `-gridparent`
  overrides it): the written output gets the base's origin, after the matrices, the
  displacements and the `-savedisp` datasets were computed.
- **A volume of one slice** is not `MRI_IS_3D`: `mri_3dalign_setup` prints "cannot use nD
  images!" (with a bell) and 3dvolreg "Can't initialize base image for alignment" (or "first
  pass alignment algorithm"); `-twopass`'s coarse rotation search first prints
  `THD_rota3D_matvec`'s "non-3D image input!" for each of its 44 rotations (its sums over the
  empty region read no voxel).
- **AFNI-93's region**: `voldif`'s region (the voxels at least `(num+1)·del` from every
  face) is the same for every shift, so on a volume too thin for it every shift of a rotation
  scores 0 (and every rotation of the coarse rotation search scores the full `Σ b²`): AFNI keeps
  the first shift (-2 steps along z) and the first rotation (-9, -18, -9 degrees). Corrected,
  neither is a candidate: no shift and no rotation.
- **A constant base** (AFNI-89): `lsqfit`'s "vectors are all 0" errors, then the crash at the
  first registered volume, in both passes.

## The parity suite and random testing (relay 3)

- **The subsampled coarse search needs 120 voxels along every axis** (`new_get_best_shiftrot`:
  `nx >= 120 && ny >= 120 && nz >= 120`), not 120³ voxels in all: none of the oracle runs'
  series takes it (ds005454 is 160 × 160 × 96). The suite's `e_big120` series (120³, 3 volumes)
  does, and AFNI-94's case (`-coarse 1`, the subsampled step 0) is checked on it.
- **AFNI-97** (random testing's trial of 2026-11-06, rule A): `fft_shift2` (`thd_shift2.c`)
  returns zeros when both rows' shifts are beyond the row length, `for(ii...) f[ii] = g[ii] =
  0.0`, without the `g != NULL` test every other `*_shift2` has. The shears (`apply_xshear`,
  `apply_yshear`, `apply_zshear`, `thd_rot3d.c`) shift the last row of an odd count alone
  (`g = NULL`), passing the previous pair's `a1` as the second shift; when that row's shift and
  the stale `a1` are both beyond the row length (a rotation far beyond a head's motion: the
  trial's runaway fits, `-twopass` first passes ending at 160 degrees), AFNI writes through
  the NULL pointer (`memset`, SIGSEGV in `fft_shift2` under `apply_3shear`). Only the Fourier
  method (`-Fourier`, `-final Fourier`) and an odd row count along the sheared axis. The
  replica clears the existing row alone by default (as AFNI clears a pair) and names AFNI-97;
  with `reproduce` it stops where AFNI crashes, after AFNI's messages (the fit's rotations and
  the `-rotparent` final rotation).
- **`THD_open_nifti`'s float-conversion notice is printed once per run** (its static
  `n_xform_warn`, unless `AFNI_NIFTI_TYPE_WARN` is YES): 3dvolreg opens several datasets (a
  uint16 base and input: the trial's 348 message differences); the replica mutes the later ones
  as it mutes `THD_report_obliquity`'s long warning.
- **AFNI-12 with 3dvolreg**: 3dvolreg's `-prefix` is not checked during the option scan; the
  writer refuses a name `THD_filename_ok` rejects ("Illegal filename for NIfTI output", nothing
  written, exit status 0). By default the replica writes it and names AFNI-12 (it named
  nothing before); `-savedisp`'s names alike.
- **The replica's temporary output name** (`cli/output.rs`, `.<name>.larmorx-<pid>-<n>.tmp`)
  exceeded 255 bytes for an output name of 228 bytes or more, which AFNI writes: the stem is now
  cut so that the temporary name stays within 255 bytes. The clean-room AFNI crate names its
  temporary files the same way (a follow-up for its agent, `docs/next-steps.md`).
- **A run that stops names its defects**: the default's refusals (AFNI-94, AFNI-95) printed no
  notice, because the notices were printed only at the end of a run that ends; now a run that
  stops prints the notices met first.
- **fMRIPrep's input to 3dvolreg depends on numpy's version**: RobustAverage divides the sliced
  volumes by their drift, `np.mean(data, axis=(0, 1, 2))` of float32 data, which numpy 2.3
  sums in another order than fMRIPrep 25.2.6's numpy 2.2.6 (2,002 of 2,785,280 stored values
  one unit apart on ds000005's run 1); the suite re-makes the input with fMRIPrep's own Python
  in its image, and the median of the replica's registered volumes is then fMRIPrep's
  `gen_avg` output byte for byte on every call compared.
- **A constant series** registers without `-twopass` (every parameter 0, AFNI's exit status
  0); with `-twopass` it is AFNI-89 (the crash).
