# FreeSurfer 8.2.0's `mri_robust_template`: findings of the replica

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (source-derived: function names, lines, the code's
structure). Track C, relays 50 and 51 (2026-11-24), queue items 3.2-14 (the program but the affine mean
space) and 3.2-15 (the affine mean space, §6). The behaviour spec is `specs/mri_robust_template.md`; the probe
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
- The golden cases (6 on 16^3 since relay 51, 32^3 before: fMRIPrep's call on three inputs,
  recon-all's on two, the mean space, one global iteration with weights, NaN voxels from their bits, a
  4-D input's one `--mapmov`): official = reproduce `c22789e3...` (32^3: `5ee82ce6...`); the default
  `9b573b7c...` (32^3: `cfe17db9...`; FS-1090). On 16^3 every registration takes the paths it took on
  32^3 (one pyramid level; the same pairs converge and do not), but `--satit` also prints its
  `image might be too small` warning; the debug build's test went from 78 s to about 4 s (the cases
  side by side).
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
relay 50); FS-1099, FS-1100, FS-1311, FS-1312 (relay 51: §6).

## 6. The affine mean space (queue 3.2-15, relay 51)

`--affine` without `--fixtp` reaches `MultiRegistration::initialXforms` 1541-1545:
`meanr = MyMatrix::GeometricMean(mras3)` (the inverted RAS transforms' 3 x 3 blocks, the target's the
identity), the only caller of `MatrixLog`/`MatrixExp` in FreeSurfer 8.2.0 (`grep` of the tag).
`GeometricMean` (1905-1915) is `MatrixExp((1.0/n) * sum MatrixLog(vm[i]))`, MATLAB's `logm` and `expm`
translated (`MyMatrix.cpp` 286-1266). Ported in `larmorx-freesurfer-robust/src/matrixlog.rs` on new
larmorx-core pieces: `vnl_svd<std::complex<double>>` (LINPACK's `zsvdc` from VXL's v3p, `job = 21`;
`vnl_svd_complex.rs`), LAPACK 3.10.0's `ZTRSEN('N', 'V')` with `ZTREXC`, the 3.10 `ZLARTG` and `ZROT`
(`lapack_ztrsen.rs`), CORE-MATH's `log1p` and the decided `clog`.

**Facts of the official build** (objdump of the probe's `MyMatrix.o`, the official flags, and of the
official program's imports):
- The official program imports `clog@GLIBC_2.2.5` (`std::log` of a complex eigenvalue in
  `MatrixLog_isst`'s `1 x 1` case), besides `acos`, `cabs`, `csqrt`, `log`, `exp`, `pow`, `frexp`,
  `__divdc3`, `__muldc3`. glibc's `clog` matches no simple formula (black box, 2 x 10^6 arguments per
  family: `log(hypot)` differs on 3.6 to 88 %, `log1p(|z|^2 - 1) / 2` on 0.06 to 48 %, its imaginary
  part from the correctly rounded `atan2` on 0.07 to 0.5 %), so the project's decided `clog`
  (`larmorx_core::cplx::clog`, `validation/probes/fs-robust/clogrt.c`) stands in, in both modes, and
  the probes preload or wrap it (`decided.sh`, fs-robust's `build.sh`).
- `std::norm` of `std::complex<double>` (the complex Frobenius norm's squares, `vnl_c_vector`'s
  `two_norm_squared` through `vnl_math::squared_magnitude`) is libstdc++'s floating-point form
  `abs(z) * abs(z)` (`_Norm_helper<true>`), so `cabs` (the decided `hypot`) enters `MatrixLog_isst`'s
  stopping test.
- **FS-1311**: `getPadeCoefficients` (356-406) points `c` at a local array of a `case` block and
  copies `c[0..m]` into the vector after the block (and the switch) ended. GCC 8.4 at `-O3` removes the
  arrays' stores as dead: the function copies `m + 1` doubles of its own uninitialized stack frame
  (`rsp`, `rsp+0x20`, `+0x50`, `+0x90`, `+0xe0` by degree). Measured with the probe: 436 of 436 seed-1
  cases that reach `MatrixExp` give a result different from the method's (NaN, `suspicious return value
  (n) from SVDC`, or arbitrary numbers), and every geometric mean is wrong; the official program's
  affine mean space is memory contents. The probe's `fsrobust-pade` and the template probe's rebuild
  (`rebuild.sh`) compile a copy of `MyMatrix.cpp` with the five arrays `static` (the tag is never
  edited) as the reference.
- **FS-1312**: `MatrixLog`'s check (683-697) exits when the imaginary part's one-norm passes `10 n
  2^-52 ||F||_1` (MATLAB's `funm` returns the complex result there; FreeSurfer, which must return a
  real matrix, ends the program). The complex Schur arithmetic leaves imaginary parts up to about 40
  `2^-52 ||F||_1` on ordinary transforms: seed 1's 200 `logm` cases, 11 refused with imaginary parts of
  2.5e-16 to 3.4e-14 (near-identity rotations and scalings, the template's own inputs), against 25
  genuine failures at 0.0158 and above (FS-1099's nonpositive eigenvalues continued, FS-1100, a
  singular matrix). Rule A: by default refused only above `sqrt(2^-52) ||F||_1` (1.5e-8 relative; the
  gap between the two groups is five orders of magnitude on both sides).
- **FS-1100**: `sylv_tri` (1221-1266) subtracts `M.get_n_columns(0, i-1) * U.get_n_rows(0,
  i-1).get_column(i)`, `i - 1` columns, where MATLAB's `X(:,1:i-1)` (1-based) is `i` columns: the last
  solved column's term is left out for every column after the first (`get_n_columns(0, 0)` is an empty
  matrix, its product with the empty vector 0). Reached when a block of two or more eigenvalues stands
  right of a single one (a small rotation with an axis scaled by more than 10 %): the logarithm is
  wrong, usually with a large imaginary part (0.0158 to 0.0189 in seed 1), so FreeSurfer then ends
  with FS-1312's message. Rule A: the whole sum by default.
- **FS-1099**: `MatrixLog`'s eigenvalue check (539-547) prints its three lines and `assert(1==2)`, which
  `-DNDEBUG` removes: FreeSurfer goes on (a reflection's `-1` gives `log` with imaginary part pi and
  then FS-1312's exit, 3.14159 in every such case). Rule B: the message and status 1 by default.
- `OrdSchurComplexLogical` passes `long` arguments where `ZTRSEN` reads 32-bit `INTEGER`s and `int`
  `SELECT`s (little-endian: the low halves; `INFO` starts at 100 and its low half is set to 0); `S`
  and `SEP` null (`JOB = 'N'` never touches them); `LWORK = n` (at least the 1 `JOB = 'N'` needs).
- `MatrixLog_pf` sizes `nodes`/`wts` by the matrix (`n`) and `gauss_legendre(8)` reassigns them to 8.
- `sylv_tri`'s `M(m, n)` is VNL's uninitialized storage, but every column is set before it is read
  (both with FreeSurfer's `i - 1` columns and the whole sum).

**Measured** (2026-11-24, fs-robust's probe `--family logm`, the official code compiled as the
official programs are, with `fsrobust-pade` the reference of FS-1311's cases; logs
`oracles/logs/fs-found-c-r51-logm*.log`): seed 1 x 600: 598 identical with `reproduce`, 0 different,
2 not ported (`ZLAQR0`: a NaN matrix's Schur form, FS-969); seed 7 x 3,000: 2,982 identical, 10 with
different NaN bits only (expm and geomean of an infinite or NaN entry: the double SVD's NaN bits,
`SVDNAN`, an open item since relay 42), 8 not ported; seed 13 x 6,000: 5,958 identical, 19 NaN bits
only, 23 not ported. `fix` differs only with notices (FS-1099, FS-1100, FS-1311, FS-1312, FS-969).
