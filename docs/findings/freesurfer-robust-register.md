# FreeSurfer 8.2.0's `mri_robust_register`: findings of the replica (queue item 3.2-13)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/mri_robust_register.md`. Track C, relay 44 (2026-11-22). The replica:
`crates-freesurfer/larmorx-freesurfer-volumes/src/mri_robust_register/` on `fs.robust`
(`crates-freesurfer/larmorx-freesurfer-robust/`, `docs/findings/freesurfer-robust.md` §6-7); the
probe: `validation/probes/fs-robust-register/`; the record:
`docs/validation/freesurfer-mri-robust-register.md`.

## 1. What the program does and where it reaches

`main` (`mri_robust_register.cpp` 592-1360): `cout << getVersion() << endl << endl` first (`8.2.0`
and an empty line, before the version options are looked at), `handleVersionOption`,
`parseCommandLine`, then `new RegRobust` for the robust cost (`P.cost == ROB`, also `--cost ROBENT`)
or `new RegPowell` (every other cost; `P.floattype` forced). `initRegistration` (1435-1827) sets
the model and the switches, reads the source (`MRIread`; `nframes != 1` ends the program),
keeps it (`P.mri_mov = MRIcopy(...)`: every output uses the image as read), masks it (`MRImask(..., 0,
0)`) or takes its Sobel magnitude (`MRIsobel` into a float image of the source's header) or its
entropy image, the same for the target; `--ixform` (`TransformRead`, its first transform's missing
geometries from the images, `LTAchangeType` to RAS, `LTAmodifySrcDstGeom`, `LTAchangeType` to voxels,
`setMinitOrig`); `--iscalein` (`ifstream f; f >> iscale`); `--uchar` (`MyMRI::setTypeUCHAR`:
`MRISeqchangeType(mri, MRI_UCHAR, 0.0, 0.999, FALSE)`); the white backgrounds
(`MyMRI::setMaxOutsideVal`: the largest voxel of frame 0, from voxel (0, 0, 0), as `float`);
`setSourceAndTarget(mov, dst, !P.floattype)`. Then `findSaturation` + `computeMultiresRegistration`
(`--satit`), `computeIterativeRegistration` (`--nomulti`) or `computeMultiresRegistration`.

The results (`main` 698-1340): `getFinalVox2Vox`, `vnl_matlab_print` (long format) of `M`,
`vnl_determinant`, for `--affine`/`--isoscale` `MyMatrix::Polar2Decomposition` of its 3 x 3 part
printed as `Rot`, `Shear` and `Scale` (`vnl_matlab_print` of a `vnl_diag_matrix`: `Scale = diag([
<elements> ])`, ported as `larmorx_core::vnl_matrix::matlab_print_long_diag`), the intensity scale;
`LTAalloc(1, P.mri_mov)`, `MRIvoxelXformToRasXform` (or the voxel matrix for `--vox2vox`),
`getVolGeom` of both images, `strcpy(reg, P.lta.c_str())` (FS-1071) and `LTAwriteEx`; the
`--iscaleout` file (`ofstream << Md.second`: 6 significant digits); `MRIvalscale` of the kept
source by the scale (`isIscale() && Md.second > 0 && !P.entropy`); `--mapmov` (`MRIcloneDifferentType`
of the target in the source's type, `LTAtransformInterp(..., SAMPLE_CUBIC_BSPLINE)`,
`MRIcopyPulseParameters`, `MRIwrite`, its result ignored); `--mapmovhdr` (`ras2ras * vox2ras` set on
an `MRIcopy` of the source, `MRIwrite` checked: `ERROR: writing <name>` and `exit(1)`);
`--weights` (`R.getWeights()`, written as `MRIlinearScale(w, NULL, -1, 1, 0)` while
`P.oneminusweights` is true, its default: `--oneminusw` switches the `1 - w` off, whatever its help
says); the half-way outputs (both maps to `cerr`, the weights' geometry as the half-way space).
`--leastsquares` sets a field nothing reads but `parseCommandLine`'s check (`R.setRobust` is
commented out); `--satest` and `--conform` likewise.

The `cout`/`printf` lines share stdout's buffer (`sync_with_stdio`); each option's line is flushed
(`fflush(stdout)` at the end of `parseNextCommand`), so a later crash keeps them.

## 2. The defects (rules A to D; `larmorx_core::defects`)

1. **FS-1070** (class a: crash). `parseNextCommand` reads `argv[1]` without checking it exists
   (but for `--subsample`): `string(argv[1])` of `argv[argc]` (`NULL`) throws `std::logic_error`
   (GCC 8.4's inlined `basic_string`: `basic_string::_M_construct null not valid`; uncaught:
   `terminate called after throwing an instance of 'std::logic_error'` and its `what()`, `SIGABRT`,
   status 134), and `atoi(NULL)`/`atof(NULL)` crash (`SIGSEGV`) (measured: `--mov`, `--cost`,
   `--maxit`, `--epsit`, `--mapmov`, `--iscalein` last). By default `mri_robust_register: option
   <arg> needs an argument (FreeSurfer crashes here)`, status 1; `reproduce` prints libstdc++'s two
   lines and ends with 134 for the string options (the earlier options' lines are out: flushed).
   `check.py` counts these cases apart (the official status is a signal).
2. **FS-1071** (class a: crash). `char reg[STRLEN]; strcpy(reg, P.lta.c_str())` (main 803-804),
   with `_FORTIFY_SOURCE`'s `__strcpy_chk`: a name of 1,024 bytes or more aborts (`*** buffer
   overflow detected ***: terminated`, status 134) right after `writing output transformation to
   ...` (measured with a 1,211-byte name: no transform written). By default the name is used;
   `reproduce` aborts the same way (identical standard output, measured).
3. **FS-1072** (class a: memory). `LTA* lta = (LTA *) trans->xform` for any `TRANSFORM` (1790): a
   morph (`.m3z`, `MORPH_3D_TYPE`) is read as an LTA. An error in both modes. Not in the probe
   (no morph writer in the cases).
4. **FS-1073** (class a: memory; refines FS-1063). `Transformation::setParameters(const
   vnl_vector<double>&)` assigns the vector (`parameters = v`: reallocated to the vector's length)
   and then `parameters.set_size(getDOF())`, which frees and allocates again without keeping the
   contents. In double precision with an intensity scale (`--doubleprec --iscale`) the step's
   vector has `DOF + 1` entries. Where both blocks fall in glibc's same size class (`(8 n + 8 + 15)
   & ~15`, at least 32: rigid 3-D (6/7), affine 2-D (6/7) and 3-D (12/13), isotropic scale 2-D (4/5)
   and translation 2-D (2/3)), glibc 2.35's tcache hands the freed block straight back: its first 16
   bytes hold the tcache entry (`next`, mangled by safe-linking: an address shifted right by 12,
   below 2^47, read as a double: a positive subnormal number; `key`, cleared to 0 by `tcache_get`),
   the rest the vector's values. So the step's x and y translations are lost (measured: the
   official `Mupdate` prints `0.0000000000000` and `0` there, its rotation and z translation the
   solution's). `reproduce` takes a positive subnormal (`0x0000555555555000`'s bits: any such
   number gives the same results, it is absorbed in every sum) and 0; the default the solution
   (FS-1063's decision). Other models (rigid 2-D, translation 3-D, isotropic scale 3-D) get another
   block: FS-1063 as before (both modes the solution; `check.py` counts them apart). Probe: every
   seeded double-precision intensity case identical with `reproduce` (16 + 10 + 7 in seeds 1, 7, 13).
6. **FS-1075** (class a: memory, released; rule A; relay 48). `RegPowell::getHalfWayGeom()` returns
   the static `tcf`, set to the level's byte copy of the target (`mriT` after `MRISeqchangeType` or
   `MRIcopy`, `RegPowell.cpp` 447-448), which `computeIterativeRegistrationFull` frees at its end
   (`if (cleanupT) MRIfree(&mriT)`, 609-610: `cleanupT` is always true) without clearing `tcf`. `main`
   (942-1129) tests the pointer (not `NULL` once a level ran), prints `Creating half way data ...` and
   the maps, then reads the freed `MRI`'s header: `MRIvoxelXformToRasXform(P.mri_mov, mri_hwgeom,
   ...)` (RAS to RAS) and `getVolGeom(mri_hwgeom, ...)`, and `LTAtransformInterp` allocates the
   half-way images from that geometry. **Measured** (`validation/investigations/freesurfer-fs1075-
   halfway/measure.py`, 24 cases of every Powell cost with random half-way outputs, three runs each,
   never served from the cache; log `oracles/logs/fs-found-c-r48-fs1075-measure.log`, outputs
   `<workspace>/oracles/inputs/fs1075-halfway/`): of the 21 cases that reach it (63 runs), 5 gave
   the target's geometry in all three runs (the freed block not yet reused), the 16 others memory
   contents: garbage sizes, voxel sizes and directions (some read as voxel values of a later image),
   13 of them different from run to run (allocator addresses under address-space randomisation);
   28 runs ended with status 1 (`MRI::Shape::Shape(): volume size must be greater than 0 in every
   dimension`, `error: could not allocate memory for slice <k> out of <n>`), 3 with 252 (one case:
   `MatrixMultiply(): m1 is null`), 8 (four cases) were killed by their 16 GiB memory box (sizes
   of about 1.1e9 per axis: the box is the answer, not re-run), 3 cases wrote garbage geometry with
   status 0. Rule A (the input makes sense, the
   result is memory): both modes keep `tcf`'s header (`powell.rs`: the voxels released where
   FreeSurfer frees the image, `RegRobust::powell_hw`) and use it, with the notice; FreeSurfer's
   printed part (the message and the maps) as it is. `getWeights()` is `NULL` for `RegPowell`:
   `--halfweights` prints `Warning: no weights have been computed! Maybe you ran with
   --leastsquares??` (`main` 1149). With the finest level skipped (`--highit 0`) and no coarser one,
   no level runs: `tcf` stays `NULL` and the program prints `ERROR: no half way space created (not
   implemented for this cost function)!` (measured: case 10).
5. The `--iscalein` file: `ifstream f(name); if (f.good()) f >> iscale; else ErrorExit(...)`.
   libstdc++'s `>>` (`num_get`, C locale): white space only leaves 1 (the sentry fails at the end),
   no number gives 0 (C++11), one too large `+-DBL_MAX`; a folder opens (`fopen` of a folder for
   reading succeeds on Linux) and reads nothing (1). The failed open leaves `errno` (`ENOENT`), which
   `ErrorExit` prints first (`error: No such file or directory`): the port sets it from the open's
   error (`log::errno_of`). No defect.

## 3. Validation (relay 44)

- The probe (`check.py`, seeds 1, 7, 13, 300 cases each): 839 identical, 0 different, 25 undefined
  (FS-1070, FS-309's unreadable LTA, FS-1063), 36 not ported (12 per seed: the ten Powell costs,
  `ROBENT`, `--debug`). The recorded test (`tests/fs_robreg_official.rs`): seed 1's first 120.
- fMRIPrep's call on the six recorded heads (`real.py`): the transform identical byte for byte on
  all six in both modes; with every output (`--outputs`, two heads) every image and transform
  identical. Standard output identical on five; on ds004173-sub-603309 the final voxel transform
  printed by `getFinalVox2Vox` (`Rtrg * Mfinal * vnl_inverse(Rsrc)`) differs in its x and z
  translations' last two printed digits (210.6943714626283 against ...285; the half-way maps on
  standard error likewise). **Settled (relay 45, rule 8): the project's decided `hypot`, not code
  generation.** (1) The fs-robust probe's `v2v` command (`fsrobust.cpp`, built into
  `oracles/freesurfer-8.2.0-probes/fs-robust-v2v/`; `v2v_official.py`) runs the official build's
  `Registration::getFinalVox2Vox` (with VNL's `vnl_matrix` product and `vnl_inverse` as the official
  program has them) on the port's three matrices: it gives the port's numbers bit for bit, so the
  port computes the product as the official build does and the official run's `Mfinal` differed in
  its last bits. (2) The official program takes `cabs` and `csqrt` from glibc (dynamic symbols
  `cabs@GLIBC_2.2.5`, `csqrt@GLIBC_2.2.5`; the matrix square root of the symmetric half-way maps,
  `MyMatrix::MatrixSqrt` on LAPACK's complex Schur form), where the project decided (the user's D4,
  applied to them 2026-11-18) the correctly rounded `hypot` in both modes. With the decided `cabs` and
  `csqrt` preloaded into the official program (`decided_complex.sh`: `complexrt.c` with CORE-MATH's
  `hypot`; `real.py --preload`), all six heads are identical in every byte of standard output, error
  and files, ds004173-sub-603309 included (log `oracles/logs/fs-found-c-r45-robreg-preload.log`). The
  last bits of `Mfinal` on that head come from glibc's `hypot` not being correctly rounded on some
  argument of the square roots: GLIBC-07's expected divergence of the decision; the transform file
  (`float` values) is identical on every head.
- Speed (one thread, the same machine): the port takes 41-98 s against FreeSurfer's 20-66 s
  (2.0-2.3x on 256^3 images): the registration step (fs.robust's open item, `freesurfer-robust.md`
  §6) and the B-spline resampling. Peak memory 3-11 % below FreeSurfer's on five heads, 0.4 %
  above on ds000228 (1,106 against 1,100 MiB).

## 4. Not ported yet (each ends with its message, status 1)

- (Ported in relay 47, section 7: `RegPowell`'s joint-histogram costs and `SB`.)
- (Ported in relay 48, §2 item 6: the Powell registration's half-way outputs, FS-1075.)
- (Ported in relay 48, §8: the test mode, `--test`.)
- `--debug` (the library's intermediate images, `saveGaussianPyramid`, the key presses
  `std::cin >> ch`, the `-wcheck-est.txt` file, the debug means of the half-way images): the
  library ends with `Registration: the debug output (--debug) is not ported yet`.

## 5. The entropy images (`--cost ROBENT`, relay 45)

`MyMRI::entropyImage(mri, radius, ball, correction, mask)` (`MyMRI.cpp` 1177-1569) and
`get3Dcorrection` (1571-1631), ported in `larmorx-freesurfer-robust/src/entropy.rs` (fs.robust), called
from `initRegistration` (`mri_robust_register.cpp` 1588-1673) for the source (timed: ` Entropy computation
took <s> seconds.`) and the target, with the mask passed in (no `MRImask` then).
- **The arithmetic as compiled**: the weights' `sigma = ssize / (4 * sqrt(2 * log(2)))` with
  `-fno-builtin` (a runtime `log`); `exp(-xs - ys - zs)` left to right; the histogram's sum and the
  entropy's `entropy -= etmp * log(etmp)` in bin order (no reassociation without `-ffast-math`); in
  `get3Dcorrection`, `3.0f / a` is a `float` division (a `float` constant over an `unsigned`), then
  widened for the `double` products; the three pieces overwrite their shared end bins in order. The
  OpenMP loop (`schedule(static,1)` over the slices, `firstprivate` histogram) computes each voxel alone:
  the port's slices go round-robin to its threads; the result does not depend on them.
- **The mask**: `MRIlinearTransformInterp(mask, MRIcopy(mri), MRIgetVoxelToVoxelXform(mask, mri),
  SAMPLE_NEAREST)`, then `(int)` of each value (`cvttss2si`: NaN and 2^31 or more give `INT_MIN`,
  outside).
- **FS-1074** (a, crash; rule B): `int ssize = 2*radius+1; int gsize = ssize*ssize*zssize; new
  double[gsize]`: a negative radius in 3-D gives a negative count: `std::bad_array_new_length`,
  uncaught (`terminate` and SIGABRT); in 2-D `zssize = 1` and the count is positive, the weights' loops do
  nothing (their sum 0, the uninitialised weights divided by it, never read), and every voxel's box loop
  is empty: an image of zeros, then the registration of two zero images. A radius of 645 or more makes
  `gsize` overflow `int` (negative: the same abort; positive and smaller: the loops write past the
  weights). By default an error (status 1); `reproduce` the abort (status 134, compared with the official
  child's SIGABRT in the probe) or the 2-D zeros; the error in both modes where FreeSurfer writes past its
  weights.
- `entropy < 0 || isnan(entropy)` never holds (positive weights; `p <= 1` in floating point since the
  sum of positive terms is at least each term): the message is ported but unreachable.
- **Validation**: the probe's 17 entropy cases (byte, short and float images, masks on both sides, 2-D
  18 x 16 x 1 images, radius 0, 1, 2, 9 and -1 / -2, the ball, the correction with and without the
  warning, both, the written images, the Sobel image first, an affine model with the intensity scale)
  and the prelude's `--cost robent`: identical with `reproduce` in all three seeds (900 cases of seeds 1,
  7, 13: 839 identical, 0 different, 25 undefined, 36 not ported). The golden cases gain a ROBENT case
  (its default equals the official value). The recorded heads (`real.py --extra '--cost ROBENT --entmov
  em.mgz --entdst ed.mgz'`, the default radius 5): ds000228-sub-pixar008 and ds004173-sub-603309 identical
  in every byte in both modes (both entropy images, the transform, both streams with the times masked);
  the port 128 s and 179 s against FreeSurfer's 92 s and 134 s (one thread), peak memory 1,122 MiB
  against 1,087 (ds000228, +3 %) and 1,272 against 1,400 (ds004173). The log and exp: correctly rounded
  by default, glibc's with `reproduce`; on these cases both give the same images.
- The time line ` Entropy computation took <s> seconds.` is masked in the probe's and `real.py`'s
  comparisons (the port's debug build takes seconds where FreeSurfer's takes none).

## 6. The Powell registration (`--cost` `LS`, `NCC`, `TB`, `LNCC`, `SAD`; relay 46)

- **Source**: `RegPowell.cpp` (`costFunction` 77-246 to its first `return dd`, everything after it
  dead; `computeIterativeRegistrationFull` 252-601; `setTransformation` 603-646) and
  `CostFunctions.cpp`'s half-way costs (`leastSquares` with `s1`, `s2` 408-487, `absDiff` 489-559,
  `localNCC` 646-768, `mapMRI` 770-830, `NCC` 832-910, `tukeyBiweight` 1044-1137, `mean` 63-123,
  `minmax` 47-61, `rhoTukeyBiweight` in the header) and `MyMRI::MRInorm255`. `my_cost_function`
  overrides `fs_cost_function::f(const vnl_vector<double>&)`: the parameters reach the cost in
  `double` (the port's `FsCostFunction::with_double`, track A's `fs_vnl.rs`, a few lines).
- **OpenMP**: every cost's loop over slices is `#pragma omp parallel for schedule(static)
  reduction(+:d)` (the counters `omp atomic`): the threads' partial sums meet in the order they
  finish, so FreeSurfer's cost depends on the thread count and the run above one thread. The port
  computes the one-thread order (each sum from the shared 0 plus the one partial sum); the probe
  and the replays run the official program at one thread.
- **`exit` inside a cost**: a matrix square root's internal error in `costFunction`
  (`MyMatrix::MatrixSqrt Internal Error`, seen after 1,718 iterations of NaN values in an `LNCC` case
  whose blocks all held a -1: `0 / 0`) ends the program at once; `fs_powell` (the port's
  `FsPowell`) now returns when the cost reports such an end, printing nothing more (it went on for
  2,000 iterations before).
- **FS-1073 and FS-1063 reached by every Powell cost with `--iscale`**: `convertP2Md(p)` hands the
  model a `vnl_vector<double>` of the model's parameters plus the intensity's log, which
  `Transformation::setParameters` copies and resizes (the existing defects): where the two lengths
  share glibc's size class (rigid 3-D, affine 2-D and 3-D, isotropic scale 2-D, translation 2-D, as
  for `--doubleprec --iscale`) the model's first two parameters are the allocator's words (a positive
  subnormal, then 0: the first two translations never move, at every evaluation): FS-1073, reproduced
  exactly with `reproduce` (10 of the 40 probe cases noted it, all identical); otherwise (translation
  3-D, rigid 2-D, isotropic scale 3-D) memory contents: FS-1063, not compared (4 cases).
- **`setprecision(14)`** after ` final e = ` stays on `std::cout`: every later number the program
  prints with C++ has 14 significant digits (the determinant, the intensity scale, the next
  levels' lines); the port keeps the stream's precision in the registration's state.
- **The probe** (`validation/probes/fs-robust-register/check.py --family powell`, 40 cases, and the
  prelude's case of each cost in `--seed 1 --n 120`): against the official program as built, 36
  identical, 4 differ only in their last printed digits (the matrix square roots' `cabs`/`csqrt`
  through glibc's `hypot`, the user's D4 decision); with the decided `cabs`/`csqrt` preloaded
  (`--preload`, `decided_complex.sh`): 36 identical, 0 different, 4 not compared (FS-1063); seed 1's
  120 cases: 105 identical, 0 different, 8 not compared, 7 not ported (logs
  `oracles/logs/fs-found-c-r46-robreg-*.log`). The recorded tests (`fs_robreg_official.rs`,
  `fs_robreg_powell_official.rs`, the latter about 7 minutes in a debug build: run with `--release`
  or `--ignored`) hold the decided oracle's hashes.
- Not measured yet: the Powell registration on the recorded heads (neither recon-all nor fMRIPrep
  runs it), its speed (sequential sums, one thread) and memory.

## 7. The joint-histogram costs and `SB` (`--cost` `MI`, `NMI`, `ECC`, `SCR`, `SB`; relay 47)

- **`JointHisto`** (`fs.robust`'s `jointhisto.rs`): 256 x 256 bins filled from the half-way samples
  (the source first; its -1 skips the target's sample), the weights split between four bins,
  smoothed by a 19-tap Gaussian (`sigma = 7 / sqrt(8 ln 2)`: `log(2)` and the root are a constant
  GCC folds; the taps through `exp`), normalised; `computeMI`, `computeNMI`, `computeECC` take
  `log2` (65,536 calls per evaluation), `computeSCR` none. `create` sets both byte images' outside
  value to -1 and leaves it.
- **`log2`**: glibc 2.35's x86-64 `log2` (`log2@@GLIBC_2.29`, which the official program binds) has
  one build, without fused multiply-adds: Arm's Optimized Routines v19.11 `log2.c` without
  `HAVE_FAST_FMA` built with GCC 11.4 equals it on 402,653,199 arguments (NaN bits included), the
  `HAVE_FAST_FMA` form with FMA code differs on 175,963 (measured as a black box; glibc's code not
  read). `larmorx_core::math::log2_glibc` translates that form (`reproduce`), `log2` is CORE-MATH's
  correctly rounded one (the default): `validation/probes/coremath-log2/`, 2,147,552,192 arguments.
  glibc's `log2` is not correctly rounded on 0.0005 to 0.1 % of them by family (the default's
  results may then differ in their last bits: GLIBC-07, not named, the cost is iterative).
- **`SB`** (`CostFunctions::SBCost`): sorts the voxel indices by their keys with libstdc++'s
  `std::sort` (`larmorx_core::stdsort::sort_by`, moved from the FSL family's crate on the user's
  decision of 2026-11-23); ties are common (the background), so the sort's own order of equal keys
  decides the cumulative sums. A zero norm (a constant mapped image) gives NaN keys everywhere,
  which the comparison leaves in place (no step outside the vector) and the cost 0.
- **The order of the lines**: `costFunction`'s lines (the empty histogram's warning, a cost's error
  lines) go to `std::cout` among `fs_powell`'s iteration lines; relay 46 buffered the cost's apart
  and wrote them after Powell's. Now one shared stream (`powell.rs`'s `Shared`): the probe's case 19
  (`MI --satit --verbose 2`, a warning after iteration 0) found it.
- **The probe** (`check.py --family histo --preload`, 32 cases of the five costs with the Powell
  family's random options): 29 identical, 0 different, 3 not compared (FS-1063); seed 1's 120 cases
  with the preload: 110 identical, 0 different, 8 not compared, 2 not ported (`--debug`, `--test`).
  The recorded tests (`fs_robreg_histo_official.rs`, ignored in a debug build like the Powell
  family's; `fs_robreg_official.rs`) and the golden cases (`--cost MI`, `--cost SB` on 16^3 byte
  images: 7.6 s and 1.8 s in a debug build) hold the decided oracle's hashes. In a debug build (CI's)
  the seeded test hashes its five cases of these costs as `SLOW`, not run (about 270 s at one thread
  in a debug build, 620 s for the test under the NaN trap; a second recorded hash,
  `check.py`'s `official_sha256_skip_debug`); a release build compares them.


## 8. The test mode (`--test <n> <image>`; relay 48)

- `parseNextCommand`'s `TEST` (2201-2207): `RegRobust R; R.testRobust(argv[2], atoi(argv[1]));
  exit(0)`, a fresh registration (verbose 1, the options read so far unused). With nothing after
  `--test`, `atoi(argv[1])` of `NULL` dies of a segmentation fault (measured: status 139, nothing on
  standard error); with the number but no image `std::string(argv[2])` of `NULL` aborts (libstdc++'s
  `basic_string::_M_construct null not valid`, status 134): both FS-1070 (the port's `missing`).
- `Registration::testRobust` (`Registration.cpp` 1315-1700) and `buildGaussianPyramid` (3053-3141;
  the port's `fs.robust` `testmode.rs`): the pyramid is the image blurred (`MRIconvolveGaussian`,
  which keeps the type) and `MyMRI::subSample(mri_tmp, NULL, true)`, whose `randpos` defaults to 0
  (the fixed random table's offsets, not every second voxel: the first port took -1 and the probe's
  34^3 cases differed in every second level). `max == -1` becomes `w + h + d`, so the loop that
  subsamples the first level is never entered. `p.resize(i - 1)` drops the level below 16.
- **FS-1076** (class a: out of bounds; rule B). `gpS[gpS.size() - level]` with `level = gpS.size() -
  1` is always `gpS[1]`; with one level (16 to 31 voxels along the smallest axis) it is past the
  vector's end, where `resize` left the pointer of the dropped level (libstdc++ does not clear the
  storage of a trivially destructible element; the level is never freed): the test registers that
  level (below 16 voxels), whose registration ends with the minimum-size message, status 1
  (measured: `b16` cases). By default an error after the test's line; `reproduce` uses the dropped
  level: identical with the official program (the probe's three cases).
- **FS-1077** (class a: crash). `MRIread` failing returns `NULL`; testRobust prints ` Read MRI ...
  successfully!` and `buildGaussianPyramid(NULL, ...)` prints its first line and reads
  `mri_in->width`: a segmentation fault (measured). An error in both modes.
- **FS-1078** (class c; rule A). Tests 5 and 6: `MRIvox(mriTs, rr, cc, dd) = rr + cc + dd + 1` is
  `((BUFTYPE *) slices[dd][cc])[rr]`, a byte access, on an image of the source's type (the
  convolution keeps it): for a short or float image the byte `rr` of the row changes (the first
  `w/3` bytes: the first `w/6` shorts or `w/12` floats of the row get garbage), not voxel `rr`.
  By default the values are set as voxel values (`MRIsetVoxVal`); `reproduce` writes the bytes:
  identical with the official program on the four short and float cases.
- **The probe** (`check.py --family test`, 31 cases, `cases::test_cases`): 28 identical with
  `reproduce`, 0 different, 3 not compared (FS-1070's two, one of them identical, and FS-1077);
  the default differs on 7, each with its notice (FS-1076 three, FS-1078 four). The recorded test
  `fs_robreg_test_official.rs` holds the official hash (3.7 s in a debug build); the golden cases
  gained four (`--test 4`, `--test 5` on a float image, `--test 30`, `--test 0` on 16^3); the
  prelude's `--test 1 mov.mgz` (16^3: FS-1076) is compared in the seeded test now.
