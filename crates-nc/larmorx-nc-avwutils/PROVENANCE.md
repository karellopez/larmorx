# Provenance: larmorx-nc-avwutils

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.23's avwutils programs `fslmaths` (the image calculator; Steve Smith, David Flitney, Mark
Jenkinson, Stuart Clare, Thomas Nichols and Matthew Webster) and, to come, `fslstats`, written to
give FSL's output bit for bit. It is distributed under the FSL Licence's terms (`LicenseRef-FSL`:
non-commercial use only, `../LICENSE`), as part of the `larmorx-nc` package
(`docs/licensing.md`). The parts of FSL's libraries it relies on (newimage's volumes, arithmetic,
thresholds and robust limits; NewNifti; miscmaths; cprob's random numbers and normal quantiles)
are the shared core's, `../larmorx-nc-core` (its `PROVENANCE.md`), as are the parts of these
programs FSL's scripts run (`avwutils`). The Apache-2.0 clean-room originals are still to be
written, from `specs/fslmaths.md` and `specs/fslstats.md`; they will use none of this crate's
code.

The FSL package is fsl-avwutils 2209.6 of the FSL 6.0.7.23 conda installation
(`$FSLDIR/src/fsl-avwutils/`; tag on `git.fmrib.ox.ac.uk`); its original source is kept,
unmodified, in `../fsl-source/fsl-avwutils/` (the FSL Licence requires it with any transmitted
product).

## Licences of the translated sources

- **FSL** (fsl-avwutils 2209.6): "FMRIB Software Library, Release 6.0 (c) 2018, The University of
  Oxford", the FSL Licence (non-commercial). fslmaths: "Copyright (C) 2000-2008 University of
  Oxford".

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates.

## FSL files and functions translated

| Rust module | FSL file | Functions |
|---|---|---|
| `fslmaths` | fsl-avwutils `fslmaths.cc` | `main` (the input's type, `-dt`/`-datatype` with `input`, `-odt`/`-output_datatype`, the messages and statuses), `inputParser` (the loop and its operations: `-add`, `-sub`, `-mul`, `-div`, `-rem`, `-mas`, `-max`, `-min` with a number or an image; `-thr`, `-thrp`, `-thrP`, `-uthr`, `-uthrp`, `-uthrP`; `-exp`, `-log`, `-sin`, `-cos`, `-tan`, `-asin`, `-acos`, `-atan`, `-sqr`, `-sqrt`, `-recip`, `-abs`, `-bin`, `-binv`, `-nan`, `-nanm`, `-index`, `-grid`, `-rand`, `-randn`, `-seed`, `-range`, `-save`, `-restart`, the undocumented `-pow` and `-invbin`; the kernel state and `-kernel` (3D, 2D, box, boxv, boxv3, gauss, sphere, file), `-s`, `-fmean`, `-fmeanu`, `-dilM`, `-dilD`, `-dilF`, `-ero`, `-eroF`, `-fmedian`, `-dilall`, `-subsamp2`, `-subsamp2offc`, `-edge`, `-dog_edge`, `-fillh`, `-fillh26`, `-inm`, `-ing`, `-tensor_decomp` (on `larmorx-nc-core`'s `filters` and `kissfft`); the output's rounding and `save_volume_dtype`), `loadNewImage`, `check_for_output_name`; not translated yet (refused): the dimension reductions, `-roi`, `-bptf`, the statistics, `-tfce`, `-roc` |
| `usage` | fsl-avwutils `fslmaths.cc` | `printUsage`'s text |
| `cli` | – | the command line as `larmorx-nc fsl fslmaths` runs it |

## Changes from the originals

- **D13** (`docs/api/upstream-defects.md`): the defects FSL-151, FSL-152, FSL-153, FSL-155,
  FSL-156, FSL-158 and FSL-159 (`docs/findings/fsl-maths.md`) are corrected by default and
  reproduced with `LARMORX_UPSTREAM_DEFECTS=reproduce`; FSL's crashes (FSL-156, FSL-158, and
  FSL-159's read past a smaller image) are refused in both modes with a message, exit status 1.
- **Maths:** `exp`, `log` and `pow` are glibc 2.35's as the core translates them; `sin`, `cos`,
  `asin`, `acos` and `atan` are larmorx-core's correctly rounded ones (glibc's results on every
  case compared); `tan` is the `libm` crate's (FreeBSD's msun) until glibc's is ported: in
  `double` it differs from glibc's in the last bit of some values (`docs/findings/fsl-maths.md`).
- **Uncaught exceptions** (a missing image, images of other sizes): FSL aborts (SIGABRT) after
  the C++ runtime's text; the replica prints the same text and exits with status 1.
