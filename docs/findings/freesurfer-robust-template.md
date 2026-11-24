# FreeSurfer 8.2.0's `mri_robust_template`: findings of the replica

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (source-derived: function names, lines, the code's
structure). Track C, relay 50 (2026-11-24), queue item 3.2-14 (part 1 of the program; 3.2-15 and
3.2-16 follow). The behaviour spec is `specs/mri_robust_template.md`; the probe
`validation/probes/fs-robust-template/`.

## 1. What the program is

`mri_robust_register/mri_robust_template.cpp` (954 lines) parses the command line into `Parameters`,
fills a `MultiRegistration` (`MultiRegistration.cpp`, 2,269 lines) and calls, by path:
`initialXforms(inittp, fixtp, 0, 5, 0.01)` then `mapAndAverageMov(0)` (`--noit` or two inputs), or
`initialXforms` then `computeTemplate(iterate, epsit, pairiterate, pairepsit)` (more inputs); then the
writers. Each pairwise registration is a `RegRobust` (fs.robust's `registration.rs`, ported for
`mri_robust_register`, queue 3.2-13): the program adds only the multi-image logic, `MyMatrix::
RotationMean`, the LTA conversions (`VOXmatrix2LTA`, `RASmatrix2LTA`, `LTA2VOXmatrix`,
`LTA2RASmatrix`), `Quaternion::importMatrix`/`getRotVec`, the `MATRIX` overload of
`AffineTransDistSq` and `MyMRI::MRIvoxelXformToRasXform`. `halfWayTemplate` has no caller (its call in
`main` is commented out): not translated. The program's paths:

- fMRIPrep (sMRIPrep's `StructuralReference`): `--satit --fixtp --mov ... --inittp 1 --iscale --noit
  --template <t>.nii.gz --subsample 200 --lta ...`: the inputs registered to the first and averaged
  there (median); recorded in two fMRIPrep oracle runs (§4).
- recon-all's motion correction: the same with `--average 1 --iscaleout` (`scripts/recon-all` 1474-1486).
- sMRIPrep `--longitudinal`: no `--fixtp`, no `--noit`: the rigid mean space (`initialXforms`' second
  half, `RotationMean`, `createTemplateGeo`), and with three inputs or more the global iterations
  (`computeTemplate`).
- The affine mean space (`--affine` without `--fixtp`): `MyMatrix::GeometricMean`, i.e. `MatrixLog`
  (`MyMatrix.cpp` 509-1268: the Schur-Parlett logarithm with `OrdSchurComplex`, `MatrixLog_isst`,
  `MatrixLog_pf`, `gauss_legendre`, `blocking`, `swapping`, `cumsum0`) and `MatrixExp` (286-508, the
  Padé approximants): queue 3.2-15. The port stops there with `MyMatrix::GeometricMean (...) is not
  ported yet`, status 1 (one prelude case of the probe).

## 2. Facts of the official build

- **Assertions compiled out** (`-DNDEBUG`, as the rest of fs.robust: `docs/findings/freesurfer-robust.md`):
  the binary holds `__assert_fail` (from other code) but none of this program's assertion texts.
  So `--mov` followed by another option leaves the list empty (`    Total: 0 input volumes`), `assert(n
  == m || m == 0)` on the masks' count does not stop fewer masks (FS-1084), `assert(tpi >= 0 && tpi <
  nin)` does not stop `--inittp` past the inputs (FS-1092), and `averageConformSet`'s `assert(1==2)`
  after its "not implemented" message does not stop `--conform` (FS-1091): it writes a trilinear
  template.
- **OpenMP**: `initialXforms`' and `computeTemplate`'s loops over the inputs are `#pragma omp parallel
  for schedule(static,1)` with `critical` sections around some prints and the centroid's sum; `averageSet`'s
  median loop over slices `schedule(guided)` (each voxel alone: independent of threads). With more than
  one thread and more than one registration the lines interleave and the centroid's sum takes the order
  the threads finish (FS-1081); `maxchange`'s update is outside any `critical` (a race, but a maximum:
  the same value). The port runs the registrations one after the other (one thread's order). `--threads
  <n>` calls `omp_set_num_threads` (no line).
- **The command line's crashes** (FS-1080): a list option given last reads `argv[argc]` (NULL) in its
  `option[0] != '-'` test: SIGSEGV; `--template` last: `std::string(NULL)` (libstdc++'s `logic_error`,
  abort); `--conform` last: `P.conform = argv[1]` (the assignment's `strlen(NULL)`: SIGSEGV, measured);
  the numbers: `atoi(NULL)`/`atof(NULL)`: SIGSEGV; `--test` given last: `atoi(argv[1])` of NULL
  (SIGSEGV, evaluated first), with one argument `std::string(argv[2])` of NULL (abort), as
  mri_robust_register's (FS-1070).
- **The debug folder** (FS-1098): `MR.setOutdir(P.mean.substr(0, pos - 1))` for the last `/` at `pos`:
  `out/t.mgz` gives `ou`, so `--debug`'s files land in the working folder named `outp1_to_...`; `pos` 0
  wraps to `npos` (the whole name). Only `--debug` and the registrations' names (unused without the
  registration's own debug, which the program never sets: `//R.setDebug(debug)`) see it.
- **The random target** (FS-1082, FS-1083, FS-1097): `getRandomNumber(1, P.mov.size(), seed)`, `P.mov`
  being the command line's list: one 4-D input gives `rand() % 1 + 1 = 1` whatever the seed. `getSeed`
  sums the axes through each input's centre (`MRIgetVoxVal`, `fabs`, `double`), returns `(unsigned
  int)(dseed)` (x86-64's `cvttsd2si` into a 64-bit register: values from 2^32 wrap, from 2^63 the
  indefinite integer's low bits 0); a sum of 0 makes `getRandomNumber` take `time(NULL)`.
- **The 4-D input's mask**: `if (!mri4d)` after reading the mask (instead of `!mask4d`): a mask that
  does not read is then dereferenced (FS-1085); more than one mask leaves `mask4d` NULL and the first
  frame's `MRIcopyFrame(mask4d, ...)` crashes (FS-1086); with no mask `MRIfree(&mask4d)` of NULL prints
  `MRIfree: null pointer` (an error message, kept in both modes, no ID).
- **`--ixforms`**: `TransformRead` returning NULL is dereferenced (`trans->xform`; FS-1087); a morph
  is cast to an LTA (FS-1088); for one 4-D input only frame 0 gets a transform and `mapAndAverageMov`
  dereferences the others' NULL (FS-1089).
- **`--inittp 0`** without transforms: with `--noit` or two inputs `mapAndAverageMov` applies NULL
  transforms (`LTAtransformBSpline(..., NULL)`: FS-1093); with more inputs `computeTemplate` starts from
  `initialAverageSet` (defined).
- **`--weights` with `--leastsquares`** and the global iterations: `mri_weights` is never sized
  (`if (robust)` in `loadMovables`), `computeTemplate` stores `mri_weights[i]` past its end (FS-1094);
  `writeWeights` is skipped with `--leastsquares` anyway.
- **`--mapmov` with one name** (only for one 4-D input: the parser requires as many names as `--mov`'s):
  `MRIallocSequence` without `MRIcopyHeader`: the 4-D output keeps the default header (FS-1090).
- **`--average 2`**: `Regression<float>(b).getRobustEst(sat)` per voxel crashes in the official build
  unless the values are almost equal (FS-1065, relay 42); `--average 3`: `vnl_matrix<float>(n v, v)`
  (FS-1095: 2 x 4,096 x 4,096 floats for two 16^3 images, impossible for heads).
- **Failed writes** (FS-1096): `writeMean`, `writeLTAs`, `writeWarps`, `writeIntensities`,
  `writeWeights` return `false` on a failed write; `main` ignores the values and exits 0.

## 3. The decided C library functions the comparison needs

The official program takes `acos`, `cabs` and `csqrt` from glibc (dynamic `acos@GLIBC_2.2.5`,
`cabs@GLIBC_2.2.5`, `csqrt@GLIBC_2.2.5`). The project decided `acos` correctly rounded in both modes
(2026-11-04, applied to FreeSurfer in relay 38) and `cabs`/`csqrt` on the correctly rounded `hypot`
(the user's D4, relay 41); the port follows. The probe and `real.py` preload `decided.sh`'s
`libdecidedtemplate.so` (fs-robust's `complexrt.c` with CORE-MATH's `hypot`, and `acosrt.c` with
CORE-MATH's `acos`, glibc's for a NaN argument) into the official program. Measured 2026-11-24:
- without any preload, seed 1 x 150: 13 cases differ, all in the mean space's printed tiny norms
  (` Input rotation's max deviation from rotation is: 1.95098e-15` against the port's 6.71647e-16, `
  mapping back to rot, err = ...`): the registrations' matrix square roots (glibc's `hypot` inside
  `cabs`/`csqrt`: the D4 divergence, as mri_robust_register's probe found); the templates and LTAs
  identical;
- with fs-robust-register's `libdecidedcomplex.so`: seed 1 x 300: 1 case differs (286, four inputs:
  two of the `mapping back to rot` norms): glibc's `acos` in `Quaternion::getRotAngle` (the mean
  rotation's rotation vectors) one ulp off CORE-MATH's; with `libdecidedtemplate.so` identical.

## 4. Measurements (2026-11-24, the official program at one thread with the decided preload)

- The probe (`check.py`): seeds 1, 7, 13 x 300: 293, 291, 292 identical, 0 different, 6, 8, 7 not
  compared (`SKIP`: FS-1065, FS-1083, FS-1084, FS-1092, FS-1094, the readers' FS-309), 1 not ported each
  (the affine mean space); the default differs only with notices. The recorded test
  `tests/fs_robtmpl_official.rs` holds seed 1 x 150 (debug builds: the first 45 cases).
- The golden cases (6 on 32^3: fMRIPrep's call on three inputs, recon-all's on two, the mean space,
  one global iteration with weights, NaN voxels from their bits, a 4-D input's one `--mapmov`):
  official = reproduce `5ee82ce6...`; the default `cfe17db9...` (FS-1090).
- fMRIPrep's recorded calls (`real.py`; first with fs-robust-register's complex pair only, then for the
  record, from the clean commit e9b605fd, with `libdecidedtemplate.so`: the same verdicts and times within a
  few seconds): ds004173's three N4-corrected T1w and ds005454's two, the
  fMRIPrep, recon-all and longitudinal recipes: 6/6 identical in every byte (standard output and error,
  the template, LTAs, intensity files, the mapped inputs) with `reproduce` and the default. One thread:
  FreeSurfer 76 s, 76 s, 271 s (ds004173's fMRIPrep, recon-all, longitudinal), 57 s, 56 s, 59 s
  (ds005454's); the port 127 s, 126 s, 468 s, 100 s, 100 s, 104 s (1.65 to 1.8 times: the registration
  of 3.2-13, its speed items). Peak memory: FreeSurfer 1,880, 1,880, 1,996, 1,397, 1,397, 1,397 MiB;
  the port 1,658, 1,661, 1,774, 1,377, 1,378, 1,377 MiB (below FreeSurfer's;
  the memory test `tests/parity/test_memory_peak.py` holds ds005454's fMRIPrep call).

## 5. Defects

FS-1080 to FS-1098 (`larmorx_core::defects`; the spec's §8; register rows in track C's HANDOFF.md,
relay 50). FS-1099, FS-1100 unused.
