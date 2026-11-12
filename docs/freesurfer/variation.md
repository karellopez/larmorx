# FreeSurfer 8.2.0: variation

**Closed to FreeSurfer clean-room implementers.** Written from FreeSurfer's source and the official
binaries by a replica-side agent (`docs/porting-guide.md` §1).

Sub-task 5 of FreeSurfer's preparation: what can make an official program's result differ from
what its source defines, or from one run to the next: the build's floating-point settings, the
thread count, random seeds; and the post-release updates of 8.2.0 (§4). Relay 4 wrote the first
pass; relay 5 settled GCC bug 110182 for N4 (§1), classified every OpenMP accumulation on the
default paths and measured the ones fMRIPrep's calls reach (§2), and read the newest update
archive (§4). What is still open is in §5. Lines are FreeSurfer 8.2.0's
(`<workspace>/tags/freesurfer-8.2.0`).

## 1. The build

- **One compiler for everything.** Every one of the 74 official binaries' `.comment` section holds
  the single string "GCC: (Ubuntu 8.4.0-3ubuntu2) 8.4.0" (code.md §1). The section keeps one copy of
  each distinct string, so the statically linked ITK 4.13.2 and VNL objects were built by the same
  GCC 8.4.0 (checked on `AntsN4BiasFieldCorrectionFs` and `mri_ca_register`).
- **Flags** (code.md §1, per target in code.toml): C++ `-O3 -DNDEBUG -fopenmp -fno-builtin`, C++11
  without GNU extensions; C `-O3 -fopenmp`; Fortran (the 4dfp tools) `-O3 -fopenmp -fno-builtin`. No
  `-ffast-math`, no `-march` or `-mavx`.
- **Measured: no AVX, no FMA.** `objdump -d` of the 72 compiled default-path programs finds no VEX
  or FMA instruction: all floating point is SSE2 scalar or packed, `FLT_EVAL_METHOD` 0, so float
  and double operations round as their types say, and no multiply-add is fused. `-O3` turns on GCC's
  loop and SLP vectorizers.
- **The host's libm.** The binaries link `libm.so.6` dynamically (`readelf -d`: libz, libgomp,
  libcrypt, libstdc++, libm, libgcc_s, libc): `exp`, `log`, `pow`, `sin`... are those of the machine
  that runs them (glibc 2.35 on the reference machine, Ubuntu 22.04), and `-fno-builtin` keeps the
  C++ calls as calls (even `sqrt`: `call sqrt@plt` in `vnl_random`'s normal-distribution code,
  just before `drand64` in `mri_ca_register`). A replica's `reproduce`
  therefore needs glibc 2.35's functions (`larmorx_core::math`'s `*_glibc`), and the official
  results may differ on another glibc.
- **GCC bug 110182 does not touch the official N4: GCC 8.4.0 keeps the source's float roundings.**
  GCC 11.4 and 12.3 at `-O3` drop two float roundings in ITK's B-spline fit
  (`docs/findings/ants-n4.md`): the SLP vectorizer builds `vector(2) double` → `vector(2) float` →
  `vector(2) double` round trips and match.pd's `TYPE_PRECISION` test folds them away. 8.2.0's
  default intensity correction is `AntsN4BiasFieldCorrectionFs` (ITK 4.13.2's N4, built by GCC
  8.4.0). *Measured* (relay 5, `validation/investigations/freesurfer-inventory/gcc84/`, with
  conda-forge's GCC 8.4.0 under `<workspace>/oracles/toolchains/gcc-8.4`):
  - the probe of `scripts/build_ants_oracle.sh` (ITK's `r` computation in miniature): 0 of 120
    values differ at `-O3`, with FreeSurfer's flags, with `-msse2 -mfpmath=sse` and with
    `-march=corei7`; Ubuntu 22.04's `g++-11` at plain `-O3`: 31 of 120;
  - FreeSurfer's N4 pipeline (`n4_tu.cxx`: its ITK calls on ITK 4.13.2, configured with
    FreeSurfer's `packages/source/build_itk.sh` options) compiled by GCC 8.4.0 with FreeSurfer's
    flags and Ubuntu's defaults (`-fcf-protection -fstack-protector-strong`, the official
    binaries carry `endbr64` and stack canaries): its GIMPLE after the vectorizers holds **no
    lossy vector conversion chain** for the bug to fold (`scan_folds.py`, the GCC 11
    investigation's scanner: 0 chains; GCC 8.4 does vectorize other double pairs, 423
    `vector(2) double` mentions in the SLP dump);
  - the official binary: `fit_in_binary.py` locates the object's functions in the stripped
    `AntsN4BiasFieldCorrectionFs` by runs of identical instructions (the B-spline fit at
    `0x69660`, the control-point evaluation at `0x6d180`, `UpdateBiasFieldEstimate` at `0x6fa30`;
    runs of 13 to 53 instructions, the rest differing in inlining and the system headers); in
    those three windows of the official program there is **no packed floating-point
    instruction at all**, and the fit's three divisions for `r` are scalar `divsd` each rounded
    to float by `cvtsd2ss`, as the source says (GCC 11's oracle: `divpd`, kept in double).
  So a replica that follows the source reproduces the official N4's fit; no `gcc_double_axes`
  mode is needed for FreeSurfer.
- **`long double` is the x87's 80-bit format, in 12 reached functions.** `FLT_EVAL_METHOD` 0
  holds for `float` and `double` (SSE2), but `long double` on x86-64 Linux is the x87 extended
  format (64-bit significand), computed with x87 instructions (the official `mri_coreg` holds 76
  of them, `mris_place_surface` 202, `objdump`). Reached by default-path programs
  (call graph, full view): `MRIgaussianSmoothNI` (the kernel sums; `mri_coreg`, `mri_convert`,
  `mri_normalize`, `mri_segreg`, `mri_vol2surf`), `DMatrixMultiply`, `DVectorDot` and the
  surface-gradient code of `utils/surfgrad.cpp` (`MRISfaceNormalFace`, `MRISfaceNormalGradFace`,
  `SurfGradUnitVector`, `MRISedgeLengthCost`, `MRISedgeAngleCost`; `mris_place_surface`,
  `mris_remesh`, `mris_remove_intersection`, `mris_convert`), `DListStats`, and two I/O helpers
  (`gifti_xml.c`'s `ewrite_data_line`, nlohmann's `strtof`). A bit-exact replica needs
  80-bit extended arithmetic (Rust has no such type: a software implementation of the x87's
  add, multiply, divide, square root and conversions with round-to-nearest-even, the precision
  control at its Linux default of 64 bits).

## 2. Threads

- **FreeSurfer's reproducible-OpenMP switch.** `utils/romp_support.cpp` 53-55 sets
  `romp_level = ROMP_level_assume_reproducible`, and nothing changes it (its only other writes, 140
  and 394, guard nested regions). A loop written `#pragma omp parallel for if_ROMP(<level>)` runs in
  parallel only when `<level> >= romp_level` (`ROMP_if_parallel1`, 117-120): loops marked `fast`
  ("known to get differing results parallel and serial", `include/romp_support.h` 99) or
  `experimental` run **serially** in the official programs; `assume_reproducible` and
  `shown_reproducible` loops run in parallel. Loops built with `romp_for_begin.h` (5 files) sum
  through `ROMP_Distributor`'s fixed partitions, the same for any thread count.
- **Every OpenMP accumulation, classified** (relay 5, `omp_reductions.py`, over the 543 reached
  files: 50 files hold 269 OpenMP pragmas; 129 accumulated variables in `reduction` clauses and in
  `critical` or `atomic` blocks). For each: the declared type (integer counts are exact in any
  order), whether the loop runs in parallel in the official binaries (the ROMP switch above), and
  whether a default-path program reaches the function (call graph, full view). A floating-point
  sum in a parallel `reduction` depends on how the iterations are shared among the threads; in a
  `critical` or `atomic` block also on the order in which the threads arrive, so it can change
  from run to run. **24 floating-point accumulations run in parallel in reached functions**; read
  one by one:

  | Where | Programs | What it sums | Effect |
  |---|---|---|---|
  | `utils/mrifilter.cpp` 6713, `MRIgaussianSmoothNI` | `mri_coreg` (by default: its images are smoothed at each scale), `mri_convert`, `mri_normalize`, `mri_segreg`, `mri_vol2surf` (with smoothing options) | the kernel's sum and sum of squares, `long double`, partitioned by thread | **thread-count dependent**, at the `long double` level: FreeSurfer's comment (6738-6741) says `double` gave "small differences in voxel values ... across different number of threads" and `long double` "resolves the differences" (it makes them rarer; the order still changes) |
  | `mri_robust_register/MultiRegistration.cpp` 1419-1423, `initialXforms` | `mri_robust_template` (several inputs: recon-all's MotionCor, sMRIPrep's template) | the centroid, a `vnl_vector_fixed<double,4>`, added in a `critical` block of a `parallel for schedule(static,1)` over the registrations | **run-to-run nondeterministic** with four or more inputs and more than one thread (three or more terms added in the order the registrations finish; two terms commute exactly), **but only for an unbiased template**: with `--fixtp` (recon-all's MotionCor, sMRIPrep's default) `initialXforms` returns before the centroid is used (1478); sMRIPrep's `--longitudinal` template drops `--fixtp --noit` |
  | `utils/cma.cpp` 1181, `ComputeBrainVolumeStats2` | `mri_brainvol_stats` | ten volumes as sums of `VoxelVol` | exact for 1 mm voxels (recon-all's conformed space: integers); thread-count dependent for non-integer voxel volumes (`-cm`, high-resolution conforming) |
  | `utils/mri2.cpp` 7492, `MRIcropToFoV` | `mri_mask` | voxel coordinates (integers) as `double` | exact |
  | `utils/mrisurf_mri.cpp` 1784, `MRIScomputeBorderValues_new` | `mris_place_surface` | four means, `double` | printed only (2556), no output depends on them |
  | `utils/mrisurf_metricProperties_faster.cpp` 308 | 39 surface programs (static reach) | the total surface area, `double` | only with `FS_FASTER_MP` set (`MRIScomputeMetricProperties`, `mrisurf_metricProperties.cpp` 1846); no FreeSurfer script sets it |
  | `utils/mrisurf_sseTerms.cpp` 2865 | 5 surface programs | three areas, `double` | inside `#ifdef BEVIN_MRISCOMPUTESSE_CHECK`, not compiled; the compiled pragma is `if_ROMP(fast)`: serial |

  **Not on fMRIPrep's paths, against relay 4's first reading:** the ten `reduction(+:d)` loops of
  `mri_robust_register/CostFunctions.cpp` (`mean`, `norm`, `leastSquares`, `absDiff`, `localNCC`,
  `NCC`, `tukeyBiweight`) and `MyMRI::getNormalizedImage` (1689, 1709) serve only `RegPowell`'s
  cost functions (`--cost` other than the robust default) and the normalised images; sMRIPrep's
  `mri_robust_register --satit --iscale` and recon-all's `mri_robust_template --satit` run
  `RegRobust`, which reaches none of them (call graph; `RegPowell.cpp` 151-166 are the only
  callers). The other parallel floating-point reductions (`utils/dmatrix.cpp` 773,
  `utils/mri2.cpp` 7429 and 7956, `utils/surfgrad.cpp` 1362, ...) are in functions no default-path
  program reaches. The full table: `<scratch>/r5/omp-acc.tsv` (regenerate with the tool).
- **Measured at 1, 2 and 4 threads** (relay 5, `thread_check.py` with `thread_jobs.toml`, on the
  reference runs' subjects, in a 12 GiB box; every run's output hashed, LTA files also without
  their comment lines, which hold the account name and the time of the run):
  - sMRIPrep's `mri_robust_register --satit --iscale --mov T1.mgz --dst <T1w> --lta` (ds000005's
    T1.mgz onto its rawavg.mgz) at 1, 4, 4 and 2 threads: **the same transform in all four runs**
    (only the `# created by` line differs), and the same time (66-68 s) at every count: nothing
    parallel runs on this path, as the call graph says.
  - `mri_coreg` with its defaults (dithering with seed 53, smoothing on: "DoSmoothing 1", both
    images smoothed at each scale) at 1, 4, 4 and 2 threads (`--threads`): **the same transform in
    all four runs**, 132 s at one thread and 52-60 s at four. The `long double` kernel sums of
    `MRIgaussianSmoothNI` did not change this result; FreeSurfer's own case for a difference is a
    large image with a kernel of 6 mm FWHM or more (§5).
  - recon-all's MotionCor (`mri_robust_template --satit --inittp 1 --fixtp --noit --iscale
    --subsample 200`) on four inputs (the conformed `orig.mgz` of four reference runs; the
    raw images were refused: "images have different voxel sizes") at 1, 4, 4 and 4 threads:
    **the same template (identical bytes) and the same four transforms in all four runs**, 95 s
    at one thread, 43 s at four: the three registrations do run side by side, and with `--fixtp`
    the centroid sum above is never used.
- **ITK's threads.** Both ITK programs run ITK at one thread whatever recon-all's `-itkthreads`
  says: `AntsN4BiasFieldCorrectionFs` takes its count from `--threads-nondetermistic` (default 1,
  lines 81-86 and 158-163, which also print that more threads "may be non-deterministic") and recon-all passes
  none; `AntsDenoiseImageFs` sets 1 unconditionally (lines 42-47). So N4's B-spline fit uses one
  work unit (the per-unit sums of `docs/findings/ants-n4.md` do not vary), and the denoiser's
  filters run single-threaded.
- **Thread counts in the reference runs:** recon-all's `-openmp 4 -itkthreads 4` (OMP_NUM_THREADS,
  ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS, and `--threads 4` to the network programs and wrappers;
  `mris_fix_topology` always `-threads 1`; programs.md §1).

## 3. Random numbers

- **`vnl_random::drand64`'s call order, measured.** ITK 4.13.2's `vnl_random.cxx` 220 adds two
  `lrand32()` calls (`double(lrand32())/0xffffffff + double(lrand32())/(0xffffffff²)`), whose order
  C++ leaves unspecified. In every official binary that has the function, the first call's result
  (kept in `%rbx` across the second) is divided by 4294967295.0 and the second's by the square
  (folded to `0x43efffffffc00000`): **left to right**, then `(upper - lower) * sum + lower`. 65 of
  the 72 compiled default-path programs contain it, all compiled the same way; the 7 without are
  `mri_matrix_multiply`, `fs_temp_file`, `fs_temp_dir` and four 4dfp tools
  (`validation/investigations/freesurfer-inventory/drand64_order.sh`). FreeSurfer's `OpenRan1`
  (`utils/numerics.cpp` 1277) casts `drand64(MIN, MAX)` to float.
- **Seeds.** `setRandomSeed` (`utils/utils.cpp`) reseeds through `OpenRan1` (with the 2014 detour
  through `seed - 1`, 85-96) and seeds `srand`/`srandom` too; with no seed set, `randomNumber` falls
  back to `-abs(time(NULL))` (148) after a warning, unless `FREESURFER_SEED` is set (119). recon-all
  passes `-seed 1234` to most programs (programs.md §3). Relay 3's list of reached functions calling
  a random source has 97 rows (`drand48`, `srand48`, `rand`, `setRandomSeed`, `randomNumber`, ...;
  third-party.md, TP-15 for glibc's generators); which of them run with a clock seed on a default
  path is the next relay's check.
- **Time-of-day seeds in fMRIPrep's programs.** `mri_segreg` (387-388, bbregister's optimiser),
  `mri_vol2vol` (663-664) and `mri_surf2surf` (844-848, only when synthesizing data) seed
  `drand48` with `PDFtodSeed()` unless a seed is given: `tv_sec + tv_usec` of the time of day
  (`utils/pdf.cpp` 32-38); `mri_vol2surf` seeds it only when synthesizing its source (357-360;
  its `seed`, from the time of day when not given, 1873). Whether a default
  call draws from it (for `mri_segreg`: the random rotations at 1314-1316) is the next relay's
  check.

## 4. The post-release updates of 8.2.0 (D3)

FreeSurfer updates an installed 8.2.0 with `fs820_updates.sh`, which fetches the archive named in
`pub/dist/freesurfer/.fs820_updates/default` (data-files.md §4). Relay 5 downloaded the current one
(the orchestrator's D3): `patch_data_20260625.tgz`, 23,515,931 bytes, last modified 2026-06-25,
SHA-256 `99361361e40fbb4044cbede4202e3ef2ddeacbae145637761db669f52cca3e70`, one folder per
platform (CentOS 7, Rocky 8 and 9, Ubuntu 22 and 24, macOS Intel and Apple silicon). For the
reference runs' platform (`ubuntu22_x86_64`, the package `freesurfer-linux-ubuntu22_x86_64-8.2.0-
20260314-d932c45`) it replaces ten files, each with a one-line note (its `.msg`), compared with the
installed package byte by byte:

| File | Note in the archive | On a default path? |
|---|---|---|
| `bin/recon-all` | "recon-all rca-surfreg fixed bugs in expert options" | **yes**: one line differs from the release, 4220 `if($XOptsFile)` becomes `if($#XOptsFile)` (the call of `rca-surfreg`) |
| `bin/rca-surfreg` | the same note | **yes** (v8's surface registration): accepts `--expert` (a `case "--expert":` added) |
| `bin/mri_glmfit` | "Re-added Logan KM" | no |
| `bin/mri_dualperm` | "Added MaxDV to mri_dualperm command." | no |
| `bin/mri_tumorsynth`, `bin/create_nnUNet_v1.7_env.sh` | exit-status fix; Python 3.9 for nnU-Net | no |
| `bin/SegmentAAN.sh`, `bin/segmentHA_T2.sh`, `bin/mri_histo_atlas_segment_fireants` | output path, typos, citations | no |
| `python/packages/SuperSynth/scripts/inference.py` | "Reinstating logic that was unintentionally dropped during refactor" | no |

**Neither default-path change alters a default run.** recon-all's `XOptsFile` is an empty list
unless `-expert` is given (line 85), and tcsh evaluates `if()` on an empty list as false and
`if(<file name>)` as a syntax error (*measured* with `/usr/bin/tcsh`: "if: Expression Syntax."):
the release breaks `rca-surfreg`'s call only with an expert-options file, which the update fixes.
The June archive's `recon-all` also carries the configured version line the March archive left as
`'@FS_VERSION@ (@BUILD_STAMP@)'` (data-files.md §4), so against the release it differs in line 4220
only. *For the replicas:* follow the updated `recon-all` and `rca-surfreg` (newest upstream; the
change is a bug fix by rule A anyway); the unpatched reference runs stay valid for every default
path. No data file and no compiled default-path program is patched.

## 5. Next

1. TensorFlow's and PyTorch's threads (`mri_synthseg`, `mri_synthstrip`, `mri_synthmorph`,
   `mri_sclimbic_seg`): run twice and at two thread counts on one recorded step, compare bytes
   (with the replay tool, sub-task 7).
2. `MRIgaussianSmoothNI` at 1 and 4 threads on a large image with a wide kernel (FreeSurfer's own
   case), and `mri_robust_template` as an unbiased template (without `--fixtp --noit`, sMRIPrep's
   `--longitudinal`) on four or more inputs at several thread counts, to size the two effects of §2.
3. The random sources on default paths (§3): which use a fixed seed, which the clock; whether a
   default call of `mri_segreg` draws from its time-of-day seed.
