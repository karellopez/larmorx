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
  standard error likewise). `v2v.py` computes VNL's source arithmetic (zero-started product sums,
  `vnl_inverse`'s adjugate, `vnl_det`) on the port's three matrices and gives the port's numbers:
  either `Mfinal`'s last bits differ (its 13 printed decimals agree) or the official build computes
  the inline `vnl_inverse`/product otherwise. **Open**: a `v2v` command in the fs-robust probe
  (the matrices in, the official code's product out) settles it.
- Speed (one thread, the same machine): the port takes 41-98 s against FreeSurfer's 20-66 s
  (2.0-2.3x on 256^3 images): the registration step (fs.robust's open item, `freesurfer-robust.md`
  §6) and the B-spline resampling. Peak memory 3-11 % below FreeSurfer's on five heads, 0.4 %
  above on ds000228 (1,106 against 1,100 MiB).

## 4. Not ported yet (each ends with its message, status 1)

- `RegPowell` (`--cost` `LS`, `MI`, `NMI`, `ECC`, `NCC`, `SCR`, `TB`, `LNCC`, `SAD`, `SB`):
  `RegPowell.cpp`, `CostFunctions.cpp`'s costs, `JointHisto.cpp`; the program ends before reading
  the images (FreeSurfer reads them first).
- `MyMRI::entropyImage` (`--cost ROBENT`; `--entmov`, `--entdst`, `--entradius`, `--entball`,
  `--entcorrection` are parsed): ends after the `Converting mov to entropy image` line.
- `--test` (`RegRobust::testRobust`): ends after `--test: TEST-MODE`.
- `--debug` (the library's intermediate images, `saveGaussianPyramid`, the key presses
  `std::cin >> ch`, the `-wcheck-est.txt` file, the debug means of the half-way images): the
  library ends with `Registration: the debug output (--debug) is not ported yet`.
