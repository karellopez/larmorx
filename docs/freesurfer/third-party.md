# FreeSurfer 8.2.0: the third-party audit

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; see
[README.md](README.md)).

Sub-task 3 of the preparation (CLAUDE.md rule 3: "Audit each ported file for third-party code
first"). **State: first pass by relay 2 (2026-11-12)**; relay 3 completes it (each package's
terms read in full, the files without a notice, the code inside FreeSurfer's files that names
another origin, and what each finding means for porting). This page lists; it decides nothing.

## The files

`validation/investigations/freesurfer-inventory/licences.py` reads every file the programs on a
default path are built from: the files holding a function the call graph's full view reaches
(what the official binaries contain; [code.md](code.md) §2), the sources of their CMake targets
(the 4dfp tools' Fortran included), and the scripts and Python programs of
[programs.toml](programs.toml) with their package files. 543 files, 736,506 lines. For each it
records the notice in its first 80 lines and every line elsewhere that names another origin (a
copyright other than MGH's, "Numerical Recipes", "adapted from", "copied from", a licence's name,
netlib, ...).

| Area | Notice in the header | Files | Lines |
|---|---|---:|---:|
| `utils/` | FreeSurfer (MGH) | 112 | 307,215 |
| `utils/` | none | 19 | 9,431 |
| `utils/` | another copyright (the MARS project, MIT CSAIL) | 2 | 489 |
| `include/` | FreeSurfer (MGH) | 17 | 9,260 |
| `include/` | none | 52 | 30,645 |
| `include/` | another copyright (nlohmann/json, Geometric Tools, ...) | 3 | 19,682 |
| program directories | FreeSurfer (MGH) | 98 | 138,097 |
| program directories | none | 16 | 9,325 |
| program directories | Apache-2.0 (ITK's adaptive non-local means filter) | 1 | 516 |
| `scripts/` | FreeSurfer (MGH) | 9 | 12,884 |
| `scripts/` | none | 18 | 5,343 |
| `talairach_avi/` | Washington University | 16 | 2,959 |
| `talairach_avi/` | none | 10 | 2,676 |
| `talairach_avi/` | FreeSurfer (MGH) | 1 | 253 |
| `packages/minc` | FreeSurfer (MGH) header on MNI's code | 12 | 15,251 |
| `packages/minc` | MNI (McConnell Brain Imaging Centre) | 3 | 3,668 |
| `packages/netcdf` | UCAR/Unidata | 11 | 25,323 |
| `packages/dicom` | Washington University (Mallinckrodt Institute: the CTN library) | 5 | 11,013 |
| `packages/xml2` | its copyright notices; permissive wording | 11 | 27,215 |
| `packages/expat` | its copyright notices (MIT terms in its `LICENSE`) | 3 | 9,265 |
| `packages/jpeg` | the Independent JPEG Group | 45 | 20,702 |
| `packages/tiff` | Sam Leffler / Silicon Graphics (permissive wording); one file none | 27 | 14,236 |
| `packages/nrrdio` | Gordon Kindlmann (`copyright.NrrdIO`) | 37 | 16,727 |
| `packages/nifti` | public domain | 2 | 7,977 |
| `packages/gifti` | none in the files (no `LICENSE` in the directory) | 2 | 7,755 |
| `packages/dcm2niix` | BSD (1), MIT (1), public domain (2), none (6) | 10 | 28,347 |
| `packages/cephes` | Stephen L. Moshier's | 1 | 252 |

## What the first pass found inside the files (relay 3 checks each)

- **Numerical Recipes.** FreeSurfer replaced its Numerical Recipes routines by open ones
  (`utils/numerics.cpp`, "Wrappers for core math routines from open-sources: VNL and CEPHES";
  `OpenSvdcmp`, `OpenRan1`), keeping their calling conventions ("silly numerical recipes in C
  requires 1-based stuff", `utils/matrix.cpp` 369, 494, 559; `utils/dmatrix.cpp` 403).
  `OpenRan1` (`numerics.cpp` 1248-1256) is "meant to mimic ... the ran1 algorithm in numerical
  recipes": whether its code is NR's or written from the description decides whether it can be
  ported. The quickselect in `utils/utils.cpp` 1382 and `mri_robust_register/RobustGaussian.cpp`
  34-38 is "based on the algorithm described in Numerical recipes in C ... based on code by
  Nicolas Devillard - 1998. Public domain."
- **VXL/VNL**: `utils/fs_powell.cpp` and `include/fs_vnl/fs_powell.h` are "taken from
  vnl_powell"; `utils/fs_lbfgs.cpp` calls VNL's netlib L-BFGS (`v3p_netlib_lbfgs_`); VNL is linked
  statically from ITK (code.md §1). VXL's terms apply to that code.
- **Copyrights of others inside FreeSurfer's files**: `utils/DICOMRead.cpp` 5590 ("these funcs
  Copyright (c) Joerg-R. Hill, December 2000"; the file is in every volume reader's reach);
  `tkregister2/tkregister2.cpp` 9 (CorTechs Labs, with MGH); `utils/MARS_DT_Boundary.cpp` and
  `utils/min_heap.cpp` (the MARS project, Thomas Yeo and Mert Sabuncu, MIT CSAIL, 2006-2008);
  `include/utilsmath.h` (Geometric Tools, LLC, Boost Software License 1.0);
  `include/json.h` (Niels Lohmann's JSON for Modern C++, MIT; parts "taken from ranges-v3");
  `utils/mriio.cpp` 9696 ("borrowed from nifti1_io.c", public domain).
- **ITK**: `AntsDenoiseImageFs/itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx` (Insight Software
  Consortium, Apache-2.0); the two `Ants*Fs` programs otherwise use ITK's headers (N4, denoising),
  which larmorx's ITK ports partly cover.
- **The network programs**: `mri_synthseg` and `mri_sclimbic_seg` carry neurite's U-Net layers,
  "Copied with permission from github.com/adalca/neurite" (lines 752-948 and 1078-1271); their
  files have no notice of their own.
- **Washington University**: the 4dfp tools (16 files with its notice, 10 without) and the CTN
  DICOM library (`packages/dicom`). CLAUDE.md's settled choice already gives the Talairach step a
  clean room with its atlas fetched from FreeSurfer.

## Questions for the orchestrator (relay 3 adds to them)

1. Files with no notice (`include/` 52, `utils/` 19, program directories 16, `scripts/` 18,
   `talairach_avi/` 10): does FreeSurfer's `LICENSE.txt` (Part B) cover them by default? Relay 3
   reads `LICENSE.txt`'s scope and the files' history notes.
2. `packages/gifti` has no licence file and no notices (GIFTI's reference library is public
   domain upstream); relay 3 checks.
3. `OpenRan1` and `OpenSvdcmp`: written from NR's description, or NR's code? The seeds of
   `mris_fix_topology`, `mris_sphere`, `mri_normalize` (`-seed 1234`, programs.md) go through
   it (`setRandomSeed` and `randomNumber`, `utils/utils.cpp` 92-95 and 153, call `OpenRan1`), so
   this matters for exact replicas.

The raw table (per file: area, lines, notice, views, programs, marker lines) is regenerated with

```
python -P validation/investigations/freesurfer-inventory/licences.py \
    <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml <scratch>/cmake.json \
    <scratch>/full.json <scratch>/core.json --json <scratch>/licences.json
```

(the inputs as in code.md §6).
