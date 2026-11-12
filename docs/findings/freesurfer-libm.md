# FreeSurfer 8.2.0's C-library maths: what it calls, and where glibc's last bits matter

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's functions).
Written by track D's spec writer (FreeSurfer's foundations, queue item 3.1-08, 2026-11-12) for the
replicas (`larmorx-freesurfer`, `reproduce`) and for whoever writes the missing glibc-exact
functions. larmorx's **default** stays correctly rounded everywhere (CLAUDE.md rule 5); the question
here is only what `reproduce` (the bit-for-bit mode the parity checks use) needs: where FreeSurfer's
results depend on glibc 2.35's own last bits, a replica needs glibc's function, and where they do
not, the correctly rounded one gives FreeSurfer's results too. Every claim below is measured (the
tools: `validation/investigations/freesurfer-libm-inventory/`, closed, and the open shim
`validation/investigations/libm-shim/`).

## 1. What the programs call (static)

- **The imports.** The 70 compiled programs on FreeSurfer's default paths (recon-all's two streams
  and fMRIPrep's calls, `docs/freesurfer/programs.toml`) import 45 C-library maths symbols (`nm -D`
  on the official binaries). Every transcendental one is a double or float function; the only
  `long double` one is `sqrtl` (4 programs: `mris_remesh`, `mris_remove_intersection`,
  `mris_place_surface`, `mris_convert`), an exact operation. No program loads `libmvec` (no vector
  maths).
- **Who calls them.** The 526 objects of the programs (the unit oracle's `utils` and packages, built
  from the tag with the official compiler and flags, and the 103 of the 104 default-path program
  sources compiled the same way; `tkregister2.cpp` needs Tcl's headers) hold 1,331 functions that
  call the C library's maths; joined with the call graph's core view, the table below counts the
  reached ones (some callers are inlined into functions of other names, so the source's view, last
  column, is the other half of the evidence).
- **`-fno-builtin`.** FreeSurfer compiles its C++ with `-fno-builtin`, so GCC neither merges a
  `sin` and a `cos` of one argument into `sincos` nor inlines `sqrt`, `floor` or `fabs`: the calls in
  the source are the calls made. Two exceptions: the C++ overloads for `float` (`std::sin(float)`,
  `std::exp(float)`, ...) are builtins whatever the flag, so a float `sin` and `cos` of one argument
  become one `sincosf` (`MatrixReallocRotation`, `MRISrotate`, `MRISgradientToParameterization`;
  glibc's `sincosf` gives `sinf`'s and `cosf`'s bits, `larmorx_core::math::sincosf_glibc`); and the
  code compiled without the flag (ITK's, in `AntsN4BiasFieldCorrectionFs`; the Talairach C tools,
  `gauss_4dfp`) has a double `sincos`, whose sine differs from `sin`'s on some arguments
  (`specs/glibc-dbl.md` §7.4: `larmorx_core::math::sincos_glibc`).

| Symbol | Programs importing it | Reached functions calling it (objects) | Programs reaching them | Reached functions naming it (source) |
|---|---:|---:|---:|---:|
| `acos@GLIBC_2.2.5` | 38 | 11 | 40 | 14 |
| `acosf@GLIBC_2.2.5` | 63 | 4 | 6 | 1 |
| `asin@GLIBC_2.2.5` | 1 | 1 | 1 | 1 |
| `atan2@GLIBC_2.2.5` | 63 | 3 | 40 | 12 |
| `atan2f@GLIBC_2.2.5` | 41 | 12 | 41 | 3 |
| `atanf@GLIBC_2.2.5` | 1 | 1 | 1 | 0 |
| `cabs@GLIBC_2.2.5` | 1 | 0 | 0 | 0 |
| `ceil@GLIBC_2.2.5` | 63 | 26 | 67 | 0 |
| `ceilf@GLIBC_2.2.5` | 1 | 1 | 1 | 0 |
| `copysign@GLIBC_2.2.5` | 1 | 0 | 0 | 0 |
| `cos@GLIBC_2.2.5` | 63 | 12 | 67 | 20 |
| `cosf@GLIBC_2.2.5` | 66 | 1 | 1 | 0 |
| `csqrt@GLIBC_2.2.5` | 1 | 1 | 1 | 0 |
| `exp@GLIBC_2.29` | 67 | 36 | 68 | 44 |
| `expf@GLIBC_2.27` | 17 | 6 | 13 | 0 |
| `fabs@GLIBC_2.2.5` | 63 | 251 | 67 | 0 |
| `fabsf@GLIBC_2.2.5` | 37 | 5 | 39 | 0 |
| `fegetround@GLIBC_2.2.5` | 1 | 0 | 0 | 0 |
| `floor@GLIBC_2.2.5` | 63 | 21 | 67 | 0 |
| `fmax@GLIBC_2.2.5` | 63 | 0 | 0 | 0 |
| `fminf@GLIBC_2.2.5` | 63 | 0 | 0 | 0 |
| `fmodf@GLIBC_2.2.5` | 63 | 1 | 1 | 0 |
| `log@GLIBC_2.29` | 66 | 36 | 67 | 49 |
| `log10@GLIBC_2.2.5` | 63 | 4 | 4 | 7 |
| `log10f@GLIBC_2.2.5` | 64 | 3 | 67 | 0 |
| `log2@GLIBC_2.29` | 6 | 3 | 5 | 3 |
| `log2f@GLIBC_2.27` | 2 | 1 | 2 | 0 |
| `logf@GLIBC_2.27` | 63 | 18 | 67 | 1 |
| `lroundf@GLIBC_2.2.5` | 4 | 0 | 0 | 0 |
| `pow@GLIBC_2.29` | 66 | 34 | 67 | 42 |
| `powf@GLIBC_2.27` | 9 | 4 | 5 | 1 |
| `remainder@GLIBC_2.2.5` | 1 | 1 | 1 | 0 |
| `rint@GLIBC_2.2.5` | 63 | 1 | 67 | 0 |
| `round@GLIBC_2.2.5` | 63 | 14 | 11 | 0 |
| `roundf@GLIBC_2.2.5` | 63 | 1 | 1 | 0 |
| `sin@GLIBC_2.2.5` | 67 | 10 | 67 | 18 |
| `sincos@GLIBC_2.2.5` | 2 | 0 | 0 | 0 |
| `sincosf@GLIBC_2.2.5` | 14 | 7 | 13 | 0 |
| `sinf@GLIBC_2.2.5` | 63 | 1 | 1 | 0 |
| `sqrt@GLIBC_2.2.5` | 67 | 228 | 68 | 0 |
| `sqrtf@GLIBC_2.2.5` | 66 | 168 | 67 | 0 |
| `sqrtl@GLIBC_2.2.5` | 4 | 3 | 4 | 0 |
| `tan@GLIBC_2.2.5` | 63 | 1 | 2 | 1 |
| `tanh@GLIBC_2.2.5` | 7 | 3 | 7 | 6 |
| `tanhf@GLIBC_2.2.5` | 3 | 4 | 3 | 0 |

(The source scan looks for the transcendental families only, so the exact operations have 0 there.
`sincos` (double) is imported by `AntsN4BiasFieldCorrectionFs` and `gauss_4dfp`, from code outside
FreeSurfer's own objects; `fmax`, `fminf`, `copysign`, `fegetround`, `lroundf`, `cabs` are called
by unreached functions or by code outside the indexed objects.)

## 2. Each function's route for `reproduce`

glibc 2.35's code for each function, where `larmorx_core::math` stands, and how often glibc's result
differs from the correctly rounded one (`libm_rates.c`, glibc 2.35 against CORE-MATH, 2026-11-12;
`specs/glibc-dbl.md` §10.3 for the double `tan`, `asin`, `acos`, `atan`):

| Function | glibc 2.35's code | `larmorx_core::math` | glibc ≠ correctly rounded | Route for glibc's bits |
|---|---|---|---|---|
| `exp`, `log`, `pow` (the 2.29 symbols), `expf`, `logf`, `powf`, `sinf`, `cosf`, `sincosf` | Arm's Optimized Routines (MIT) | `*_glibc` exist | | done |
| `sin`, `cos`, `sincos` | IBM's Accurate Mathematical Library (LGPL) | `sin_glibc`, `cos_glibc`, `sincos_glibc` (clean room of `specs/glibc-dbl.md`) | | done |
| `tanh`, `atanf`, `atan2f` | fdlibm (Sun's notice) | `tanh_glibc`, `atanf_glibc`, `atan2f_glibc` | | done |
| `tanhf` | fdlibm (`s_tanhf.c`, Sun's notice; it calls `expm1f`, fdlibm's `s_expm1f.c`; one build on x86-64, no FMA) | `tanhf` (correctly rounded) only | 2.76 % of all floats | **needed** (§3): a translation of both, the `tanh_glibc` route (no spec): netlib's fdlibm 5.3 has no float versions; the float ones with Sun's notice are NetBSD's 1995 files (`$NetBSD: s_tanhf.c,v 1.4`), which glibc carries; checked on all 2³² floats against glibc 2.35 |
| `log10f`, `acosf`, `asinf` | fdlibm (Sun's notice) | none | 0.69 % of all floats; 0.25 %, 0.22 % of the floats in [−1, 1] | the same route if §3 finds them needed |
| `log10` | fdlibm (`e_log10.c`, Sun's notice) | none | 3.26 % (log-uniform 2⁻⁴⁰ to 2⁴¹) | the same route if needed |
| `log2`, `log2f` | Arm's code (glibc's `e_log2.c`, `e_log2f.c`; MIT in Arm's Optimized Routines v19.11) | none | 0.017 %; 0.0073 % of all floats | a translation from Arm's MIT code, the `log_glibc` route (no spec), if §3 finds them needed |
| `acos`, `asin`, `atan`, `tan` | IBM (LGPL; tables of searched points) | correctly rounded `acos`, `asin`, `tan`, `atan2` | 0.05 %, 0.08 %, 0.07 %, 0.19 % (`glibc-dbl.md` §10.3) | **decided: correctly rounded in `reproduce` too** (the user, 2026-11-04, `glibc-dbl.md` §9: their tables are glibc data); §3 measures the cost |
| `atan2` | IBM (LGPL; the same tables as `atan`) | correctly rounded | 0.087 % (uniform on [−1, 1]² and [−1000, 1000]²) | as `atan`: a spec would need glibc's tables; §3 |
| `csqrt` | glibc's own (LGPL) | none | not measured (no correctly rounded reference) | a behaviour spec if §3 finds it needed |
| `sqrt`, `sqrtf`, `sqrtl`, `floor`, `ceil`, `ceilf`, `fabs`, `fabsf`, `rint`, `round`, `roundf`, `lroundf`, `fmodf`, `remainder`, `fmax`, `fminf`, `copysign`, `fegetround` | | exact operations (`sqrtl`: x87 `fsqrt` at 64-bit precision, `larmorx_core::x87`) | 0 by definition | any correct implementation |
| `hypot`, `hypotf`, `erf`, `erfc`, `expm1`, `log1p`, `tgamma` | | | | not reached in the core view |

## 3. Where the last bits matter (run time)

**The method** (`libm-shim/README.md`): the recorded steps of the reference runs are replayed
(`larmorx_validation`'s FreeSurfer replay) with an `LD_PRELOAD` shim. In `count` mode it returns
glibc's values and counts, per function, the calls and the calls whose value differs from the
correctly rounded one (a replay must still reproduce the recorded files: the shim's own check); in
`cr` mode it returns the correctly rounded values for the functions named, and the replay shows
whether FreeSurfer's files change. All on the main machine, 2026-11-12, the recording
`ds000005-sub-01-classic-1thread` (the classic stream at one thread).

**Count mode** (21 steps so far, all REPRODUCED under the shim):

| Step | Calls of the shim's functions (differing from correctly rounded) |
|---|---|
| `mri_normalize` (10), `mri_ca_normalize` (14), `AntsDenoiseImageFs` (20), `mri_segment` (21), `mri_edit_wm_with_aseg` (22), `mri_fill` (24), `mri_tessellate` (26) | none |
| `seg2cc` (17: `mri_cc`) | `asin` 2 (0) |
| `mri_watershed` (12) | `tanhf` 450,648 (26,974); `atan2` 100,001 (207) |
| `mris_curvature` (56), `mris_extract_main_component` (27), `mris_smooth` (31), `mris_inflate` (33, 54), `mris_remove_intersection` (42), `mris_autodet_gwstats` (44), `mri_label2label` (48 via `label-cortex`, 49) | `atan2` 100,001 (207) each |
| `mris_sphere` (35) | `acos` 3,126,439 (11,265); `atan2` 100,001 (207) |
| `mris_remesh` (40) | `acos` 2,583,042 (4,506); `atan2` 100,001 (207) |
| `mris_curvature` (57) | `acos` 438,535,528 (600,971); `atan2` 100,001 (207) |

The 100,001 `atan2` calls are `fastApproxAtan2f`'s table, `(float) atan2(i, 100000)` for `i` = 0 to
100,000, built once per process: 207 of its doubles differ from the correctly rounded ones, **none
of its floats** (`libm_rates.c`, every entry): the correctly rounded `atan2` gives FreeSurfer's
table exactly.

**`cr` mode:**

| Step | Correctly rounded | Result |
|---|---|---|
| `mris_curvature` (56) | every function of the shim | REPRODUCED (identical curvatures) |
| `mris_sphere` (35), `mris_remesh` (40), `mris_curvature` (57) | `acos` | REPRODUCED (11,265, 4,506 and 600,971 different `acos` values change no file) |
| `mri_watershed` (12) | every function of the shim | **DIFFERS**: `brainmask.auto.mgz`, 10,878 of 16,777,216 voxels, by at most 178 |
| `mri_watershed` (12) | `tanhf` alone | **DIFFERS**: the same 10,878 voxels |
| `mri_watershed` (12) | `atan2` alone | REPRODUCED |

So **glibc's `tanhf` decides results** (`mri_watershed`'s skull strip, through
`MRISFineSegmentation` and `MRISCorrectSurface`; also called by `mri_ca_label` and
`mris_fix_topology`): `reproduce` needs a `tanhf_glibc`. The correctly rounded `acos` and `atan2`
gave FreeSurfer's files on every step measured.

## 4. `tan`

The double `tan` is reached at run time only by `sc_gamma_large`, the gamma draw for shapes of 12
or more (`specs/ranlux.md` §4.5), which no default-path program runs: `mri_coreg` dithers with flat
draws, `mris_expand`'s random target map is off in fMRIPrep's call, and the distributions that use
it (chi-squared from 24 degrees of freedom, t, F) belong to `mri_volsynth` and `mri_glmfit`'s
simulations. dcm2niix's gantry-tilt functions call it too (DICOM input, not reached in the core
view). glibc's `tan` differs from the correctly rounded one on 41,708 of the 2²⁴ arguments
`sc_gamma_large` can pass (`tan_check.c`). A glibc-exact `tan` needs glibc's table of 186 searched
points (`glibc-dbl.md` §9: no definition reproduces them), so the user's decision of 2026-11-04
holds: `tan` stays correctly rounded in `reproduce` too, and `specs/ranlux.md` §6 says what that
costs (large-shape gamma draws whose uniform falls on one of those 41,708 values, off the default
paths). Not a defect of FreeSurfer's: the register's GLIBC-07 covers glibc's `tan`.

## 5. Still to measure

The count pass over the recording's other steps (the conversions, `mri_nu_correct.mni`,
`talairach_avi`, `mri_em_register`, `mri_ca_register`, `mri_ca_label`, `mris_fix_topology`,
`mris_place_surface`, the second hemisphere's steps, `mris_sphere`'s last two) is running at the time
of writing; then the v8 stream and fMRIPrep's calls (`mri_coreg` through `bbregister`: `log2` in its
NMI cost). Each function with differing calls is then run in `cr` mode alone.
