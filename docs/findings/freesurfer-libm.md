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
| `log10f`, `acosf`, `asinf` | fdlibm (Sun's notice) | none | 0.69 % of all floats; 0.25 %, 0.22 % of the floats in [−1, 1] | **not needed** (§3): the correctly rounded `acosf` reproduces every step on two heads (up to 128 million calls, 7.3 million of them different); `log10f` and `asinf` are never called on the measured paths |
| `log10` | fdlibm (`e_log10.c`, Sun's notice) | none | 3.26 % (log-uniform 2⁻⁴⁰ to 2⁴¹) | **not needed** (§3): `mri_robust_register`'s and `mri_robust_template`'s 136 to 208 calls give the correctly rounded value |
| `log2`, `log2f` | Arm's code (glibc's `e_log2.c`, `e_log2f.c`; MIT in Arm's Optimized Routines v19.11) | none | 0.017 %; 0.0073 % of all floats | **not needed** (§3): the correctly rounded `log2` gives `mri_coreg`'s registration exactly on two heads (34 and 40 million calls, about 1,500 different); `log2f` is never called |
| `acos`, `asin`, `atan`, `tan` | IBM (LGPL; tables of searched points) | correctly rounded `acos`, `asin`, `tan`, `atan2` | 0.05 %, 0.08 %, 0.07 %, 0.19 % (`glibc-dbl.md` §10.3) | **decided: correctly rounded in `reproduce` too** (the user, 2026-11-04, `glibc-dbl.md` §9: their tables are glibc data); §3 measured no cost on the default paths (`acos`: every step on two heads; `asin`: 2 calls; `atan`, `tan`: never called) |
| `atan2` | IBM (LGPL; the same tables as `atan`) | correctly rounded | 0.087 % (uniform on [−1, 1]² and [−1000, 1000]²) | as `atan`; §3 measured no cost (its only use, `fastApproxAtan2f`'s float table, comes out exact) |
| `csqrt` | glibc's own (LGPL) | none | no correctly rounded reference | **not needed** (§3): moving both parts of each result by one unit in the last place changes nothing in `mri_robust_register` and `mri_robust_template` on two heads, so any faithful `csqrt` gives their results |
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

**Count mode.** Every step of the recording (1 to 192), **all reproduced under the shim**
(`defect2seg` only with a shorter work folder: §6; `mri_ca_register`, step 15, and the later steps 62
to 192 on 2026-11-13). The steps
that called none of the shim's functions: the conversions and headers (1-4, 9), `mri_nu_correct.mni`
(5, 8), `talairach_avi` (6) and `talairach_afd` (7), `mri_normalize` (10, 18), `mri_em_register` (11,
13), `mri_ca_normalize` (14), `mri_ca_register` (15), `mri_ca_label` (16), `mri_mask` (19), `AntsDenoiseImageFs` (20),
`mri_segment` (21), `mri_edit_wm_with_aseg` (22), `mri_pretess` (23, 25, 28), `mri_fill` (24),
`mri_tessellate` (26, 29); and of the later steps every one but those in the table, apart from
`atan2`'s table (`mris_jacobian`, `mrisp_paint`, `mris_ca_label`, `mris_place_surface`'s later calls,
`vertexvol`, `mris_volmask`, `pctsurfcon`, `mri_relabel_hypointensities`, `mri_surf2volseg`,
`mri_brainvol_stats`, `mri_segstats`, `mris_anatomical_stats`, `mri_label2label`, `mris_label2annot`).
The others:

| Step | Calls of the shim's functions (of them, differing from the correctly rounded value) |
|---|---|
| `seg2cc` (17: `mri_cc`) | `asin` 2 (0) |
| `mri_watershed` (12) | `tanhf` 450,648 (26,974) |
| `mris_fix_topology` (37, 38) | `acosf` 54,114,482 (3,335,320) and 44,208,261 (2,640,446); `tanhf` 6,241,925 (151,512) and 5,568,550 (147,571); `acos` 80,028 (242) and 80,382 (221) |
| `mris_sphere` (35, 36, 60, 61) | `acos` 3,126,439 (11,265), 2,790,472 (9,905), 279,422,769 (711,903), 290,228,661 (759,217) |
| `mris_curvature` (57, 59) | `acos` 438,535,528 (600,971), 433,304,952 (592,516) |
| `mris_remesh` (40, 41) | `acos` 2,583,042 (4,506), 2,550,114 (4,355) |
| `mris_place_surface` (46, 47) | `acos` 2,583,042 (4,133), 2,550,114 (4,102) |
| `rca-surfreg` (62: `mris_register`, both hemispheres) | `acos` 3,194,109 (12,197) and 3,374,218 (12,744) |
| `mris_place_surface` (69 to 72: white and pial, both hemispheres) | `acos` 2,583,042 (4,563), 2,550,114 (4,489), 2,583,042 (4,555), 2,550,114 (4,557) |
| `mris_curvature_stats` (85, 86) | `acosf` 738,012 (1,000), 728,604 (1,096) |
| every surface program, `mri_label2label`, `mri_label2vol`, `mris_defects_pointset` | `atan2` 100,001 (207) once per process |

The 100,001 `atan2` calls are `fastApproxAtan2f`'s table, `(float) atan2(i, 100000)` for `i` = 0 to
100,000, built once per process: 207 of its doubles differ from the correctly rounded ones, **none
of its floats** (`libm_rates.c`, every entry): the correctly rounded `atan2` gives FreeSurfer's
table exactly. `log2`, `log2f`, `log10`, `log10f`, `asinf`, `atan`, `tan`, `csqrt` and the
unreached functions: no call in these steps.

**`cr` mode** (the correctly rounded values returned):

| Step | Correctly rounded | Result |
|---|---|---|
| `mris_curvature` (56) | every function of the shim | REPRODUCED |
| `mris_sphere` (35), `mris_remesh` (40), `mris_curvature` (57) | `acos` | REPRODUCED |
| `mris_fix_topology` (37, 38), `mris_place_surface` (46, 47), `mris_sphere` (60) | every function of the shim (`acos`, `acosf`, `tanhf` included) | REPRODUCED |
| `mris_sphere` (61), `mri_watershed` (12) | every function but `tanhf` | REPRODUCED |
| `mri_ca_label` (16) | every function of the shim | REPRODUCED (none is called) |
| `mri_watershed` (12) | every function of the shim | **DIFFERS**: `brainmask.auto.mgz`, 10,878 of 16,777,216 voxels, by at most 178 |
| `mri_watershed` (12) | `tanhf` alone | **DIFFERS**: the same 10,878 voxels |
| `mri_watershed` (12) | `atan2` alone | REPRODUCED |

So on this head **only glibc's `tanhf` decides results**, in `mri_watershed`'s skull strip (through
`MRISFineSegmentation` and `MRISCorrectSurface`): `reproduce` needs a `tanhf_glibc`. In
`mris_fix_topology` the 151,512 different `tanhf` values and 3.3 million different `acosf` values
change no file; the correctly rounded `acos` (up to 438 million calls, 760,000 of them different) and
`atan2` gave FreeSurfer's files on every step.

### A second head: ds000228 sub-pixar008 (a 3.5-year-old)

The recording `ds000228-sub-pixar008-classic-1thread`, steps 1 to 61 but `mri_ca_register` (which
calls none of the shim's functions on the first head), replayed with **every function of the shim
but `tanhf` correctly rounded** (2026-11-13): **60 of 60 reproduced**, while the correctly rounded
values differed from glibc's in 1,503,766 of 578,903,915 `acos` calls (`mris_sphere`, four steps),
1,185,646 of 860,266,817 (`mris_curvature`), 8,259 and 8,544 of 5,048,820 (`mris_place_surface`,
`mris_remesh`), 166 of 66,324 and 7,300,512 of 128,506,184 `acosf` calls (`mris_fix_topology`); the
`asin` of `mri_cc` (2 calls) and `atan2`'s table as on the first head. Then `tanhf` alone:

| Step | Correctly rounded | Result |
|---|---|---|
| `mri_watershed` (12; 307,260 `tanhf` calls, 10,236 differing) | `tanhf` | **DIFFERS**: `brainmask.auto.mgz`, 21 of 16,777,216 voxels, by at most 65 |
| `mris_fix_topology` (37, 38; 8,576,175 and 9,197,675 calls, 217,529 and 214,342 differing) | `tanhf` | REPRODUCED |

So the second head confirms the first: glibc's `tanhf` decides `mri_watershed`'s mask (fewer voxels
on this smaller head), and nothing else measured needs glibc's bits.

### fMRIPrep's calls

The programs fMRIPrep and sMRIPrep run outside recon-all (`docs/freesurfer/programs.toml`, stream
`fmriprep`, run `bare`: no recording holds them) were run under the shim by
`libm-shim/fmriprep_calls.sh` on a finished subject (the recording's final subject directory, copied),
with fMRIPrep's options, one thread, 2026-11-13: `mri_coreg` (`--dof 6 --sep 4 --ftol 1e-4
--linmintol 1e-2`, the BOLD reference of fMRIPrep's oracle run) and `bbregister` from it (`--t2 --6
--init-reg`: `mri_segreg` twice, `mri_surf2surf`, `tkregister2_cmdl`, `lta_convert`), sMRIPrep's
`mri_robust_register --satit --iscale`, `mris_expand -thickness ... 0.5`, three `mris_convert`,
`lta_convert --invert`, `mri_vol2surf` to fsnative and fsaverage5 (`--projfrac-avg 0 1 0.2`, the T1w
reference standing in for the BOLD series), and `mri_robust_template` on two images of the head.
`libm-shim/compare_calls.sh` compares the outputs of two runs (provenance aside).

| Head | Calls (differing from the correctly rounded value) | Correctly rounded | Result |
|---|---|---|---|
| ds000005 sub-01 | `mri_coreg`: `log2` 34,032,856 (1,487); `mri_robust_register`, `mri_robust_template`: `acos` 67, 54 (0), `log10` 180, 142 (0), `csqrt` 1,396, 1,177 (no reference); `mris_expand`: `log2` 2 (0); `atan2`'s table in `mri_segreg`, `mris_convert`, `mris_expand`, `mri_vol2surf` | `log2` alone (`mri_coreg`, `bbregister`) | every output identical (`coreg.lta`, `bbreg.lta`, `bbreg.dat`, the costs) |
| ds000005 sub-01 | (as above) | every function of the shim, every call | every output identical (the LTAs and `.dat`, `lh.midthickness`, the GIFTI surfaces and samplings, the template) |
| ds000005 sub-01 | (as above) | `csqrt` moved by one unit in the last place (both parts; the shim's `ulp` mode: no correctly rounded `csqrt` exists to compare with) | `rr.lta`, `rt1.lta`, `rt2.lta`, `rt.mgz` identical |
| ds000228 sub-pixar008 (a 3.5-year-old) | `mri_coreg`: `log2` 39,556,088 (1,523); `acos` 51, 78 (0), `log10` 136, 208 (0), `csqrt` 1,237, 1,882; `mris_expand`: `log2` 2 (0); the `atan2` table | `log2` alone (`mri_coreg`, `bbregister`) | every output identical |
| ds000228 sub-pixar008 | (as above) | `csqrt` moved by one unit (`ulp`) | every output identical |

So fMRIPrep's FreeSurfer calls need **no glibc-exact function** on these two heads: the correctly
rounded `log2` gives `mri_coreg`'s registration exactly (its 1,487 and 1,523 different values in 34
and 40 million change no cost decision), the correctly rounded `acos`, `log10` and `atan2` everything
else, and `csqrt`'s last bits (moved by one unit) change nothing in `mri_robust_register` and
`mri_robust_template`, so any faithful `csqrt` gives their results there.

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

`mri_ca_register` (step 15) in count mode (running at the time of writing); the recording's later
steps once it is complete (`mris_register`, `mris_ca_label`, the statistics, `mri_aparc2aseg`, ...;
the 4-thread recordings have them, but their comparison needs the steps to be deterministic at 4
threads, `docs/freesurfer/variation.md`); the v8 stream; fMRIPrep's calls (`mri_coreg` through
`bbregister`: `log2` in its NMI cost); a second head for `tanhf`, `acosf` and `acos`. Each function
with differing calls is then run in `cr` mode alone.

## 6. A side finding: FS-104, long paths abort the DICOM format probe

Replaying `defect2seg` in a work folder whose paths were about 240 characters long, its
`mri_label2vol --defects` aborted with `*** buffer overflow detected ***: terminated` (signal 6);
the same step in a short folder reproduces. The backtrace (gdb on the official binary) ends in
`strcpy` into a 256-byte buffer, with the text "DCM failed to open file: <path> ...": CTN's DICOM
condition stack (`packages/dicom/condition.c` 91 and 162, `statusText[256]`, the message
`DCM_FILEOPENFAILED`, `dcmcond.c` 67, "DCM failed to open file: %s in %s"), which the volume reader's
format probe reaches for a file without a known extension (`surf/lh.defect_labels`). The official
build's `_FORTIFY_SOURCE` turns the overflow into an abort. **Measured** with the official
`mri_info` on a copy of that file: a path of 213 bytes is read, one of 215 bytes or more aborts.
recon-all's `defect2seg` (both streams) therefore fails when `<SUBJECTS_DIR>/<subject>/surf/lh.defect_labels`
reaches 215 bytes (a subjects directory of about 185 characters for a subject named `sub-01`). A crash: never
reproduced (D13); the DICOM item (3.1-16) decides the text (rule A: any path length).

## 7. The C library's sort: `qsort` (and `std::sort`, `bsearch`)

FreeSurfer's results depend on the **order** in which glibc 2.35's `qsort` leaves equal elements and
on how it treats inconsistent comparators, not only on the sort being correct; `specs/glibc-qsort.md`
describes that order (the merge order, and the quicksort fallback glibc takes only when it cannot
have a temporary array) for a clean room, checked on 16,544 cases against glibc.

**The callers** (2026-11-13; the call graph's core view scanned for `qsort`, `qsort_r`, `bsearch` and
the C++ sorts, `inv/scan.py`'s pattern; element sizes from the objects' relocations): 32 reached
functions call `qsort` (none `qsort_r`; `MRISedgeSort`'s `qsort_r`, 160-byte elements, is not
reached, even in the full view), one `bsearch`, two `std::sort`. The full view adds six, all for
input formats off the default paths: dcm2niix's `readDICOMx`, `nii_saveDTI`,
`ensureSequentialSlicePositions`, `nii_loadDirCore`, CTN's `SortDCMFileInfo` (DICOM) and libtiff's
`_TIFFMergeFieldInfo`.

| Kind of comparator | Functions (programs) | Element | Does glibc's order decide bits? |
|---|---|---|---|
| three-way on floats or doubles (`<` −1, `>` +1, else 0; NaN equal to all) | `MRIgetPercentile` (`mri_coreg`), `MRISnormalizeCurvature` (6), `MRISmedianFilterVals`, `...Val2s`, `...Val2baks` (`mris_fix_topology`), `MRIcpolvMedianAtVoxel` (`mri_segment`), `MRIsegStatsRobust` (`mri_segstats`), `MRIsort` (`mri_concat`), `MRIpercentThresh` (`mri_binarize`) | 4 or 8 bytes | only through `+0.0`/`−0.0` and NaNs (the sign of a median or percentile; NaN placement) |
| the same with `==` (NaN → +1) | `histogram_create` (`mris_curvature_stats`), `fdr2vwth` (`mri_binarize`) | 4, 8 | as above |
| three-way on integers | `short_sorted_da_list` (GIFTI writer, 67), `nunqiue_int_list` (5), `GetPrimeFactors` (`mri_surf2surf`, `mri_vol2surf`), `MRIScomputeNormals`, `MRISMP_computeNormals` (39), `mrisAsynchronousTimeStep_optionalDxDyDzUpdate` (3) | 4 | no (equal integers are identical) |
| three-way on one field, ties between different elements | `GCAapplyRenormalization`, `GCAmapRenormalize`, `GCAmapRenormalizeByClass`, `GCAcomputeTopNProbabilities`, `GCArankSamples` (label probabilities: `mri_ca_label`, `mri_ca_register`, `mri_em_register`, `mri_ca_normalize`), `MRIorderIndices` (voxels by value, `mri_ca_label`), `MRIaseg2vol` (`mri_label2vol`), `SortSurfClusterSum` (104-byte elements: the pointer path; 7), `clustSortClusterList` (`mri_binarize`, `mri_ca_label`), `VLSTsort` (32 bytes; `mri_ca_register`, `mri_vol2vol`), `matrixStatsExitHandler` (diagnostics) | 8 to 104 | **yes**: the order of tied elements (labels of equal probability, voxels of equal value) |
| never 0 (ties → −1 for both orders) | `MatrixCalculateEigenSystemHelper`, `MatrixSVDEigenValues` (eigenvalues by magnitude, 17 and 16 programs), `mrisTessellateDefect_wkr` (edges of equal length and equal first vertex, `mris_fix_topology`) | 8, 16 | **yes** (and in glibc's fallback this kind reads before the array: the spec's §5) |
| several keys, `ErrorFlag` first | `SortSDCMFileInfo` (DICOM input only) | 8 | yes, off the default paths |

**The sizes:** the largest default-path sorts hold one volume: `MRIgetPercentile` (`mri_coreg`'s
reference, 256³ floats, 64 MiB) and `MRIpercentThresh` (up to 256³ doubles, 128 MiB). glibc takes
its quicksort fallback only when the temporary array exceeds a quarter of the physical memory
(here 31.2 GiB) or cannot be allocated, so FreeSurfer's sorts take the merge order on any machine
that runs its programs: the replicas use it (spec §3).

**`std::sort`** (libstdc++'s introsort, compiled into the programs): `MyMRI::get3Dcorrection` sorts
4 unsigned integers (`mri_robust_register`), `MyMatrix::swapping` sorts `(double, int)` pairs whose
integers are distinct and whose doubles are never NaN (the block numbers of `MyMatrix::blocking` are
contiguous, so no block is empty: no `0/0`), `mri_robust_template`. Both are strict total orders on
distinguishable keys: every correct sort gives the same arrangement, and no spec of libstdc++'s sort
is needed (unlike FSL-22's). **`bsearch`** (`mrisComputeDefectMRILogUnlikelihood_wkr`) only tests
whether a face number is in a sorted list: any correct binary search.
