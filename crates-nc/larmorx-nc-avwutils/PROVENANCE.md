# Provenance: larmorx-nc-avwutils

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.23's avwutils program `fslmaths` (the image calculator; Steve Smith, David Flitney, Mark
Jenkinson, Stuart Clare, Thomas Nichols and Matthew Webster), with the command line of `fslstats`
(image statistics; Mark Jenkinson and Matthew Webster), whose program is translated in the shared
core (`larmorx_nc_core::fslstats`, because BET's script runs it), written to give FSL's output bit
for bit. It is distributed under the FSL Licence's terms (`LicenseRef-FSL`:
non-commercial use only, `../LICENSE`), as part of the `larmorx-nc` package
(`docs/licensing.md`). The parts of FSL's libraries it relies on (newimage's volumes, arithmetic,
thresholds and robust limits, whole-volume statistics, kernels and filters; NewNifti; miscmaths;
cprob's random numbers and normal distribution functions; Armadillo's Kiss FFT, maxima, sums and
sorts; nlohmann/json's output for `fslstats -json`)
are the shared core's, `../larmorx-nc-core` (its `PROVENANCE.md`), as are the parts of these
programs FSL's scripts run (`avwutils`). The Apache-2.0 clean-room originals are still to be
written, from `specs/fslmaths.md` and `specs/fslstats.md` (written by this replica's author as the
spec writer: behaviour only); they will use none of this crate's code.

The FSL package is fsl-avwutils 2209.6 of the FSL 6.0.7.23 conda installation
(`$FSLDIR/src/fsl-avwutils/`; tag on `git.fmrib.ox.ac.uk`); its original source is kept,
unmodified, in `../fsl-source/fsl-avwutils/` (the FSL Licence requires it with any transmitted
product).

## Licences of the translated sources

- **FSL** (fsl-avwutils 2209.6): "FMRIB Software Library, Release 6.0 (c) 2018, The University of
  Oxford", the FSL Licence (non-commercial). fslmaths: "Copyright (C) 2000-2008 University of
  Oxford"; fslstats: "Copyright (C) 2003-2009 University of Oxford".

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates.

## FSL files and functions translated

| Rust module | FSL file | Functions |
|---|---|---|
| `fslmaths` | fsl-avwutils `fslmaths.cc` | `main` (the input's type, `-dt`/`-datatype` with `input`, `-odt`/`-output_datatype`, the messages and statuses), `inputParser` (the loop and its operations: `-add`, `-sub`, `-mul`, `-div`, `-rem`, `-mas`, `-max`, `-min` with a number or an image; `-thr`, `-thrp`, `-thrP`, `-uthr`, `-uthrp`, `-uthrP`; `-exp`, `-log`, `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan`, `-sqr`, `-sqrt`, `-recip`, `-abs`, `-bin`, `-binv`, `-nan`, `-nanm`, `-index`, `-grid`, `-rand`, `-randn`, `-seed`, `-range`, `-save`, `-restart`, the undocumented `-pow` and `-invbin`; the kernel state and `-kernel` (3D, 2D, box, boxv, boxv3, gauss, sphere, file), `-s`, `-fmean`, `-fmeanu`, `-dilM`, `-dilD`, `-dilF`, `-ero`, `-eroF`, `-fmedian`, `-dilall`, `-subsamp2`, `-subsamp2offc`, `-edge`, `-dog_edge`, `-fillh`, `-fillh26`, `-inm`, `-ing`, `-tensor_decomp` (on `larmorx-nc-core`'s `filters` and `kissfft`); the output's rounding and `save_volume_dtype`), `loadNewImage`, `check_for_output_name`; `-tfce_minT`, `-tfce_delta` |
| `fslmaths::stats` | fsl-avwutils `fslmaths.cc` | `inputParser`'s dimensionality reduction (`-T`, `-X`, `-Y`, `-Z` and any other capital with `max`, `min`, `mean`, `std`, `maxn`, `median`, `perc`, `ar1`; its message), `-pval`, `-pval0`, `-cpval`, `-ztop`, `-ptoz`, `-rank`, `-ranknorm`, `-roi`, `-bptf` (on `larmorx-nc-core`'s `vstats`, `cprob` and `filters::bandpass_temporal_filter`) |
| `fslmaths::tfce` | fsl-avwutils `fslmaths.cc`; fsl-newimage 2601.0 `newimagefns.h` | `-tfce` (its `catch` and message), `-tfceS` (the voxel through `niftivox2newimagevox_mat`), `-roc` (the log transforms, the true and false positive rates, the curve file, the area and its quartiles); `tfce` (the sorted interior voxels, the flood fill per threshold, `edsta`'s connectivity classes, `powf(h, H)` and `pow(size, E)`), `tfce_support` (`connected_components` per threshold, the arrays `Clusters`/`Thresholds`, the sum from the top) (on `larmorx-nc-core`'s `filters::connected_components`, `glibc_float::powf`, `miscmaths::mean_col`, `var_col`, `quantile_col`) |
| `usage` | fsl-avwutils `fslmaths.cc` | `printUsage`'s text |
| `cli` | – | the command lines as `larmorx-nc fsl fslmaths` and `larmorx-nc fsl fslstats` run them (`fslstats`: `larmorx_nc_core::fslstats::main`, translated from fsl-avwutils `fslstats.cc` in the core: its `PROVENANCE.md`) |

## Changes from the originals

- **D13** (`docs/api/upstream-defects.md`): fslmaths' defects FSL-151 to FSL-180 and FSL-187
  (the robust range of `-thrp`, `-thrP`, `-uthrp`, `-uthrP`), and fslstats' FSL-181 to FSL-189
  (`docs/findings/fsl-maths.md`), are corrected by default and reproduced with
  `LARMORX_UPSTREAM_DEFECTS=reproduce`; FSL's crashes and hangs (FSL-156, FSL-158, FSL-159's read
  past a smaller image, FSL-165 with `reproduce`, FSL-174's endless `-tfce`, FSL-177's division by
  zero, FSL-178's and FSL-179's memory outside the images, fslstats' FSL-184, and FSL-185 with
  `reproduce`) are refused in both modes with a message, exit status 1.
- **Maths (GLIBC-07):** fslmaths' `-exp`, `-log`, `-sin`, `-cos`, `-tan`, `-asin`, `-acos`,
  `-atan` and the hidden `-pow` are larmorx-core's correctly rounded functions (CORE-MATH's; `tan`
  ported for fslmaths, 2026-11-03) by default; with `reproduce`, `exp`, `log` and `pow` are glibc
  2.35's (Arm's Optimized Routines, as the core translates them), since 2026-11-04 `sin` and `cos`
  too (`larmorx_core::math::sin_glibc`, `cos_glibc`: LARMORX's clean-room implementation of
  `specs/glibc-dbl.md`, Apache-2.0, no glibc code), and `tan`, `asin`, `acos` and `atan` stay
  correctly rounded (glibc's are IBM's LGPL code with searched and fitted tables, not translated:
  the user's decision of 2026-11-04): a double image then differs from FSL's in the last place on
  the measured fraction of its voxels there (0.07 to 0.21 %), a float image never
  (`docs/findings/fsl-maths.md`, "Known differences"). The FFT convolution's twiddle factors are
  glibc's `sincos` with `reproduce` (`larmorx_core::math::sincos_glibc`). The kernels', filters' and TFCE's own
  `exp`, `expf`, `logf`, `pow` and `powf` are glibc's in both modes (`powf` from Arm's Optimized
  Routines: 0 of 10 million differ).
- **Uncaught exceptions** (a missing image, images of other sizes): FSL aborts (SIGABRT) after
  the C++ runtime's text; the replica prints the same text and exits with status 1.
