# An LD_PRELOAD shim: do a program's results depend on the C library's last bits?

**Open** (to every implementer): calls of the C library's functions by name (found with
`dlsym(RTLD_NEXT)`) and of CORE-MATH's correctly rounded functions (MIT, `<workspace>/tags/core-math`);
no glibc code, no FreeSurfer code.

| File | What it is |
|---|---|
| `libm_shim.c` | the shim: for `acos`, `asin`, `atan`, `atan2`, `tan`, `log10`, `log2`, `hypot`, `erf`, `erfc`, `expm1`, `log1p`, `tgamma`, `acosf`, `asinf`, `log10f`, `log2f`, `tanhf`, `hypotf` (and `csqrt`, counted only) it counts the calls and the calls whose C-library value differs from the correctly rounded one; `LARMORX_LIBM_SHIM=count` (default) returns the C library's value, `cr` the correctly rounded one (for the functions in `LARMORX_LIBM_SHIM_FUNCS`, comma-separated, or all); one line per function and process at exit in `LARMORX_LIBM_SHIM_LOG` (`program pid mode function calls differing`) |
| `build_shim.sh` | `build_shim.sh <workspace> <output folder>`: `libm_shim.so` |
| `summarize.py` | `python -P summarize.py <shim log> [--differing]`: the log summed by program and function (processes, calls, differing; `atan2`'s 100,001-call table shown apart) |
| `fmriprep_calls.sh` | fMRIPrep's and sMRIPrep's FreeSurfer calls, which recon-all's recordings do not hold (`mri_coreg` and `bbregister` with fMRIPrep's options, `mri_robust_register`, `mris_expand`, `mris_convert`, `lta_convert`, `mri_vol2surf` to fsnative and fsaverage5, `mri_robust_template` with two images), under the shim on a finished subject, one thread, one folder per mode; its header has the usage |

`LARMORX_LIBM_SHIM=ulp` (2026-11-13) returns the C library's value moved by one unit in the last
place away from zero, for the functions chosen as in `cr`: a sensitivity test where no correctly
rounded reference exists (`csqrt`, both parts moved).

**The method** (what each mode shows):

1. `count`: the program's results are the official ones (the shim only counts; a replay must
   still reproduce the recorded files, which checks the shim itself). A function with **0 differing
   calls** in a run gives the same bits correctly rounded: a correctly rounded implementation is
   exact there, for that input.
2. `cr` for one function at a time: if the outputs stay identical, the run's results do not depend
   on that function's last bits (for that input); if they change, a replica needs the C library's
   own bits for it.

With FreeSurfer's replay tool (`python -m larmorx_validation freesurfer-replay`), each recorded step
runs under the shim through `--command`:

    LARMORX_LIBM_SHIM_LOG=<log> python -m larmorx_validation freesurfer-replay run <run> <steps...> \
        --command "env LD_PRELOAD=<out>/libm_shim.so LARMORX_LIBM_SHIM=count LARMORX_LIBM_SHIM_LOG=<log> {program}" \
        --work <scratch>/replay

Results: `docs/findings/freesurfer-libm.md` (§3).
