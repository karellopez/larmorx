# FreeSurfer 8.2.0: the code each program needs

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; see
[README.md](README.md)).

Sub-task 2 of the preparation. **State: started by relay 1** (the build graph's first pass, §1);
relay 2 completes §1's table, then the call graph (§2) and the foundations (§3).

## 1. The build graph (first pass)

From every `CMakeLists.txt` of `<workspace>/tags/freesurfer-8.2.0` (`attic/` excluded), by
`validation/investigations/freesurfer-inventory/cmake_targets.py` (it reads `set`,
`add_executable`, `add_library`, `target_link_libraries`; it does **not** evaluate `if()` blocks
or names held in variables yet, so its link lists include conditional libraries and miss
`dcm2niixfs`, which is named through `${DCM2NIIXFSLIB}`). All 74 compiled programs of
`programs.toml` have a target.

- **One shared library carries almost everything.** Every compiled FreeSurfer program links
  `utils`, a static library of 189 C++ files and 360,356 lines (`utils/CMakeLists.txt` 51-242),
  plus its headers in `include/`. A program's own file is small beside it: from 29 lines
  (`fs_temp_dir`) and 42 (`mri_brainvol_stats`) to 11,400 (`mri_watershed`); the multi-file
  programs are `mri_robust_register` (9 files, 15,787 lines), `mri_robust_template` (8, 15,016),
  `mris_topo_fixer` (10, 5,206), `mri_em_register` (3, 2,968), `mris_remesh` (2, 2,358) and
  `mris_volmask` (2, 1,317). What each program reaches inside `utils` is §2's question.
- **`utils` links FreeSurfer's bundled third-party packages** (`packages/`): `jpeg` (46 files,
  20,845 lines), `tiff` (36, 23,813), `xml2` (44, 175,766), `minc` (15, 18,727), `netcdf` (12,
  23,737), `nrrdio` (40, 18,197), `dicom` (7, 11,449), `cephes` (14, 3,819), `nifti` (2, 7,977),
  `gifti` (2, 7,755), `expat` (3, 9,265) and `dcm2niixfs`; and outside the tree zlib, ITK's VNL
  (`${ITKVNL_LIBRARIES}`), OpenMP, `crypt`, `rt` and, with FIPS support, OpenSSL's `crypto`
  (243-304). These are sub-task 3's first list: most programs never call most of them (image
  formats they do not read), which the call graph will show.
- **Programs with more**: `mri_robust_register` and `mri_robust_template` link LAPACK, BLAS,
  gfortran and quadmath; `AntsN4BiasFieldCorrectionFs` and `AntsDenoiseImageFs` link ITK
  (`ITKCommon` and the filters they include: ITK's N4 and denoising, which larmorx's ITK ports
  already cover in part).
- **Washington University's 4dfp tools** (`talairach_avi/`) link no FreeSurfer library: C files
  with Fortran routines (`analyzeto4dfp` 5 files, 1,213 lines; `gauss_4dfp` 13, 3,091;
  `imgreg_4dfp` 18, 4,277; `compute_vox2vox` 20, 4,279; shared files among them). With Intel's
  Fortran compiler they are built with `-ax=CORE-AVX512 -qopenmp` and linked with Intel's runtime
  and vector-math libraries (`irc`, `svml`, `imf`; `talairach_avi/CMakeLists.txt` 5-7); with
  GNU Fortran with `-ffixed-line-length-132 -fno-second-underscore` (8-17). Which compiler built
  the official 8.2.0 package decides the classic stream's Talairach numbers (sub-task 5: read
  the binaries' `.comment` sections once the package is unpacked).

The raw output (per program: directory, sources, own files and lines, libraries) is regenerated
by the tool; relay 2 fixes the two gaps above and puts the table here.
