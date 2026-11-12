# FreeSurfer 8.2.0: variation (first pass)

**Closed to FreeSurfer clean-room implementers.** Written from FreeSurfer's source and the official
binaries by a replica-side agent (`docs/porting-guide.md` §1).

Sub-task 5 of FreeSurfer's preparation: what can make an official program's result differ from
what its source defines, or from one run to the next: the build's floating-point settings, the
thread count, random seeds. This first pass (relay 4) holds what is measured or read so far; §4
lists what the next relay checks. Lines are FreeSurfer 8.2.0's (`<workspace>/tags/freesurfer-8.2.0`).

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
- **Open: GCC bug 110182.** GCC 11.4 and 12.3 at `-O3` drop two float roundings in ITK's B-spline fit
  (`docs/findings/ants-n4.md`); the faulty test is old (`TYPE_PRECISION` of a vector type in
  match.pd's double-conversion rule), so GCC 8.4 may have it too, and 8.2.0's default intensity
  correction is `AntsN4BiasFieldCorrectionFs` (ITK 4.13.2's N4, built by GCC 8.4). To measure (§4).

## 2. Threads

- **FreeSurfer's reproducible-OpenMP switch.** `utils/romp_support.cpp` 53-55 sets
  `romp_level = ROMP_level_assume_reproducible`, and nothing changes it (its only other writes, 140
  and 394, guard nested regions). A loop written `#pragma omp parallel for if_ROMP(<level>)` runs in
  parallel only when `<level> >= romp_level` (`ROMP_if_parallel1`, 117-120): loops marked `fast`
  ("known to get differing results parallel and serial", `include/romp_support.h` 99) or
  `experimental` run **serially** in the official programs; `assume_reproducible` and
  `shown_reproducible` loops run in parallel. Loops built with `romp_for_begin.h` (5 files) sum
  through `ROMP_Distributor`'s fixed partitions, the same for any thread count.
- **Plain OpenMP reductions** (no ROMP guard), from a scan of the 543 reached files (relay 3's
  list): 50 files hold 269 OpenMP pragmas, 68 of them with a `reduction` clause, 38 `critical`, 12
  `atomic`. Floating-point sums with `schedule(static)` depend on the thread count (each thread's
  share of the loop changes): `mri_robust_register/CostFunctions.cpp` (10 cost functions, `d`) and
  `MyMRI.cpp` 1689, 1709 (`mean`, `std`), used by fMRIPrep's `mri_robust_register` and
  `mri_robust_template`; candidates still to type-check and reach-check: `utils/surfgrad.cpp` 1362
  (`cost`), `utils/mri2.cpp` 7429, 7492, 7956 (sums), `utils/mrifilter.cpp` 6713, `utils/dmatrix.cpp`
  773, `utils/mrisurf_metricProperties_faster.cpp` 308 (`total_area`), `utils/cma.cpp` 1181 (the
  volumes). Integer counts (`nhits`, `nswaps`, ...) are exact for any thread count.
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

## 4. Next (relay 5)

1. GCC 8.4.0 and bug 110182: run `scripts/build_ants_oracle.sh`'s float-roundings probe with GCC
   8.4.0 itself (Ubuntu 20.04's `gcc-8` 8.4.0-3ubuntu2, extracted user-level; a download of about
   30 MB the orchestrator approves), or find the B-spline fit in `AntsN4BiasFieldCorrectionFs`'s
   code and look for the float round trip.
2. The reductions above: each variable's type, whether a default-path call reaches the loop, and a
   measurement at 1 and 4 threads on a recorded step (the replay tool, sub-task 7).
3. ITK's threads (`AntsN4BiasFieldCorrectionFs`, `AntsDenoiseImageFs`), TensorFlow's and PyTorch's
   (`mri_synthseg`, `mri_synthstrip`, `mri_synthmorph`, `mri_sclimbic_seg`): run twice and at two
   thread counts on one recorded step, compare bytes.
4. The random sources on default paths: which use a fixed seed, which the clock.
