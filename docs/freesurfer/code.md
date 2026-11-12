# FreeSurfer 8.2.0: the code each program needs

**Closed to FreeSurfer clean-room implementers** (written from FreeSurfer's source; see
[README.md](README.md)).

Sub-task 2 of the preparation (relays 1 and 2, 2026-11-12): per program, its source files, the
libraries it links, the shared library functions it can reach and their size; which functions many
programs share (the foundations). The machine-readable form, one entry per program, is
[code.toml](code.toml) (generated; the porting cards are built from it). Every number here comes
from the tools in `validation/investigations/freesurfer-inventory/` (§6 says how to regenerate
them) run on `<workspace>/tags/freesurfer-8.2.0`, and from the official 8.2.0 binaries
(`<workspace>/oracles/freesurfer-8.2.0/root/`).

**Words.** *Lines* are physical lines (comments and blank lines included). A function's lines run
from the first token of its declaration to its closing brace. *Reach* is what a program's `main`
can call, directly or through other functions, as far as the source shows it (§2).

## 1. The build, as the official 8.2.0 build ran it

`cmake_targets.py` interprets FreeSurfer's CMake files from the top-level `CMakeLists.txt` down
every `add_subdirectory` (363 directories, `attic/` excluded), the way CMake does: variable scopes,
options, `if`/`elseif`/`else`, FreeSurfer's own CMake functions, `find_library`. The build machine
is replaced by what the official package shows:

- **Ubuntu 22.04** (`build-stamp.txt`: `freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45`),
  so `HOST_OS` = `Ubuntu22`;
- **GCC 8.4.0 for every compiled program, C, C++ and Fortran** (the `.comment` section of all 74
  binaries: `GCC: (Ubuntu 8.4.0-3ubuntu2) 8.4.0`), so `HOST_OS_GCC_VERSION` = 8. This settles
  relay 1's question: Washington University's 4dfp tools are built with GNU Fortran
  (`-ffixed-line-length-132 -fno-second-underscore`), not Intel's compiler and its vector-math
  library (`talairach_avi/CMakeLists.txt` 5-17); their binaries link `libquadmath` and no Intel
  runtime.

No `message(FATAL_ERROR)` is reached under these settings. With them the two gaps of relay 1's
first pass close: `dcm2niixfs` (named through `${DCM2NIIXFSLIB}`) resolves (10 files, 25,770
lines), and `mri_robust_register` / `mri_robust_template` get their targets (they return early
unless `find_library` found LAPACK, BLAS and gfortran).

**What the official binaries link** (measured, `readelf -d` on all 74): every C++ program needs
only `libz`, `libgomp` (OpenMP), `libcrypt`, `libstdc++`, `libm`, `libgcc_s` and `libc` at run
time; ITK, VXL's VNL, LAPACK, BLAS and the Fortran runtime are linked in statically. The robust
tools' BLAS is the reference `libblas.a`, not OpenBLAS (the binary holds no OpenBLAS string; the
CMake file accepts either, `CMakeLists.txt` 347-349): no CPU-dependent kernels there. The three
Fortran 4dfp tools need `libm`, `libgcc_s`, `libquadmath`, `libc`; `analyzeto4dfp` only `libc`.

**Compile flags** (per target in `code.toml`; for sub-task 5): C++ `-O3 -DNDEBUG -fopenmp
-fno-builtin -fPIC -ffunction-sections -fdata-sections` (C++11, no GNU extensions; `-fno-builtin`
is the Ubuntu branch's, `CMakeLists.txt` 795-797); C `-O3 -DNDEBUG -fopenmp` (no `-fno-builtin`);
Fortran `-O3 -fopenmp -fno-builtin` plus the 4dfp tools' line-length flags; definitions
`-DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DHAVE_OPENMP -DHAVE_OPENGL -DLinux`, and `-DNO_FIPS_SUPPORT`
for `utils`. No `-ffast-math`, `-march` or `-mavx` anywhere on these targets. The link drops every
function no program calls (`-Wl,--gc-sections`).

**One library carries almost everything.** Every compiled FreeSurfer program links `utils`, a
static library of 189 C++ files and 360,356 lines (`utils/CMakeLists.txt` 51-242) with its
headers in `include/` (269 files, 85,697 lines), and through it FreeSurfer's bundled packages: `jpeg` (46
files, 20,845 lines), `tiff` (36, 23,813), `xml2` (44, 175,766), `minc` (15, 18,727), `netcdf`
(12, 23,737), `nrrdio` (40, 18,197), `dicom` (7, 11,449), `cephes` (14, 3,819), `nifti` (2,
7,977), `gifti` (2, 7,755), `expat` (3, 9,265) and `dcm2niixfs` (10, 25,770). Most programs never
call most of them (§2, §3). Washington University's 4dfp tools link nothing of FreeSurfer's.

The programs (own files: the sources of the program's CMake target; `mri_robust_register` and
`mri_robust_template` share most of theirs):

| Program | Streams | Directory | Own files | Own lines | Links besides `utils` and its packages |
|---|---|---|---:|---:|---|
| `analyzeto4dfp` | classic | `talairach_avi` | 5 | 1,213 | nothing of FreeSurfer's |
| `AntsDenoiseImageFs` | v8, classic | `AntsDenoiseImageFs` | 1 | 98 | `ITKCommon` |
| `AntsN4BiasFieldCorrectionFs` | v8, classic | `AntsN4BiasFieldCorrectionFs` | 1 | 302 | `ITKCommon` |
| `compute_vox2vox` | classic | `talairach_avi` | 20 | 4,279 | nothing of FreeSurfer's |
| `fs_temp_dir` | classic | `io` | 1 | 29 | - |
| `fs_temp_file` | v8, classic | `io` | 1 | 31 | - |
| `gauss_4dfp` | classic | `talairach_avi` | 13 | 3,091 | nothing of FreeSurfer's |
| `imgreg_4dfp` | classic | `talairach_avi` | 18 | 4,277 | nothing of FreeSurfer's |
| `lta_convert` | v8, classic, fmriprep | `lta_convert` | 1 | 1,441 | - |
| `mri_add_xform_to_header` | v8, classic | `mri_add_xform_to_header` | 1 | 225 | - |
| `mri_binarize` | v8, classic | `mri_binarize` | 1 | 1,586 | - |
| `mri_brainvol_stats` | v8, classic | `mri_brainvol_stats` | 1 | 42 | - |
| `mri_ca_label` | classic | `mri_ca_label` | 1 | 5,529 | - |
| `mri_ca_normalize` | v8, classic | `mri_ca_normalize` | 1 | 2,308 | - |
| `mri_ca_register` | v8, classic | `mri_ca_register` | 1 | 2,563 | - |
| `mri_cc` | v8, classic | `mri_cc` | 1 | 2,662 | - |
| `mri_compute_seg_overlap` | (none) | `mri_compute_seg_overlap` | 1 | 596 | - |
| `mri_concat` | v8, classic | `mri_concat` | 1 | 1,498 | - |
| `mri_concatenate_lta` | v8 | `mri_concatenate_lta` | 1 | 747 | - |
| `mri_convert` | v8, classic, fmriprep | `mri_convert` | 1 | 4,010 | - |
| `mri_cor2label` | v8 | `mri_cor2label` | 1 | 673 | - |
| `mri_coreg` | fmriprep | `mri_coreg` | 1 | 2,149 | - |
| `mri_diff` | v8 | `mri_diff` | 1 | 1,325 | - |
| `mri_edit_wm_with_aseg` | v8, classic | `mri_edit_wm_with_aseg` | 1 | 3,259 | - |
| `mri_em_register` | v8, classic | `mri_em_register` | 3 | 2,968 | - |
| `mri_fill` | v8, classic | `mri_fill` | 1 | 6,729 | - |
| `mri_info` | v8, classic | `mri_info` | 1 | 1,469 | - |
| `mri_label2label` | v8, classic | `mri_label2label` | 1 | 1,601 | - |
| `mri_label2vol` | v8, classic | `mri_label2vol` | 1 | 1,391 | - |
| `mri_make_uchar` | v8, classic | `mri_convert` | 1 | 289 | - |
| `mri_mask` | v8, classic | `mri_mask` | 1 | 655 | - |
| `mri_matrix_multiply` | classic | `mri_matrix_multiply` | 1 | 366 | - |
| `mri_normalize` | v8, classic | `mri_normalize` | 1 | 2,168 | - |
| `mri_pretess` | v8, classic | `mri_mc` | 1 | 1,413 | - |
| `mri_relabel_hypointensities` | v8, classic | `mri_relabel_hypointensities` | 1 | 293 | - |
| `mri_robust_register` | fmriprep | `mri_robust_register` | 9 | 15,787 | `liblapack.a`, `libblas.a`, `libgfortran.a`, `libquadmath.a` |
| `mri_robust_template` | v8, classic, fmriprep | `mri_robust_register` | 8 | 15,016 | `liblapack.a`, `libblas.a`, `libgfortran.a`, `libquadmath.a` |
| `mri_seg_diff` | (none) | `mri_seg_diff` | 1 | 359 | - |
| `mri_segment` | v8, classic | `mri_segment` | 1 | 1,906 | - |
| `mri_segreg` | fmriprep | `mri_segreg` | 1 | 2,399 | - |
| `mri_segstats` | v8, classic | `mri_segstats` | 1 | 2,716 | - |
| `mri_surf2surf` | fmriprep | `mri_surf2surf` | 1 | 3,035 | - |
| `mri_surf2volseg` | v8, classic | `mri_aparc2aseg` | 1 | 1,029 | - |
| `mri_tessellate` | v8, classic | `mri_tessellate` | 1 | 720 | - |
| `mri_vol2surf` | v8, classic, fmriprep | `mri_vol2surf` | 1 | 2,473 | - |
| `mri_vol2vol` | v8, fmriprep | `mri_vol2vol` | 1 | 2,862 | - |
| `mri_warp_convert` | v8 | `mri_warp_convert` | 1 | 968 | - |
| `mri_watershed` | classic | `mri_watershed` | 1 | 11,400 | - |
| `mris_anatomical_stats` | v8, classic | `mris_anatomical_stats` | 1 | 1,742 | - |
| `mris_autodet_gwstats` | v8, classic | `mris_make_surfaces` | 1 | 391 | - |
| `mris_ca_label` | v8, classic | `mris_ca_label` | 1 | 615 | - |
| `mris_calc` | v8, classic | `mris_calc` | 1 | 2,628 | - |
| `mris_convert` | v8, classic, fmriprep | `mris_convert` | 1 | 1,411 | - |
| `mris_curvature` | v8, classic | `mris_curvature` | 1 | 726 | - |
| `mris_curvature_stats` | v8, classic | `mris_curvature_stats` | 1 | 3,386 | - |
| `mris_defects_pointset` | v8, classic | `mris_defects_pointset` | 1 | 166 | - |
| `mris_euler_number` | v8, classic | `mris_euler_number` | 1 | 222 | - |
| `mris_expand` | fmriprep | `mris_expand` | 1 | 486 | - |
| `mris_extract_main_component` | v8, classic | `mri_mc` | 1 | 69 | - |
| `mris_fix_topology` | v8, classic | `mris_fix_topology` | 1 | 891 | - |
| `mris_inflate` | v8, classic | `mris_inflate` | 1 | 716 | - |
| `mris_jacobian` | v8, classic | `mris_jacobian` | 1 | 306 | - |
| `mris_label2annot` | v8, classic | `mris_label2annot` | 1 | 981 | - |
| `mris_place_surface` | v8, classic | `mris_make_surfaces` | 1 | 2,174 | - |
| `mris_register` | v8, classic | `mris_register` | 1 | 1,416 | - |
| `mris_remesh` | v8, classic | `mris_remesh` | 2 | 2,358 | - |
| `mris_remove_intersection` | v8, classic | `mris_fix_topology` | 1 | 485 | - |
| `mris_smooth` | v8, classic | `mris_smooth` | 1 | 912 | - |
| `mris_sphere` | v8, classic | `mris_sphere` | 1 | 879 | - |
| `mris_topo_fixer` | v8, classic | `mris_topo_fixer` | 10 | 5,206 | - |
| `mris_volmask` | v8, classic | `mris_volmask` | 2 | 1,317 | - |
| `mrisp_paint` | v8, classic | `mrisp_paint` | 1 | 387 | - |
| `talairach_afd` | classic | `talairach_afd` | 1 | 677 | - |
| `tkregister2_cmdl` | fmriprep | `tkregister2` | 1 | 5,392 | - |

## 2. The call graph

`callgraph.py` indexes the source without a compiler (FreeSurfer's dependencies need not be
installed): each file's comments and literals removed, its preprocessor evaluated as the official
build saw it (GCC 8.4.0, Linux x86-64, C++11, the definitions above; `#if 0` and the other
platforms' branches dropped), every definition indexed (functions, methods, constructors, K&R C,
macros, file-level tables of handlers), and each body's names resolved approximately as the
linker and the compiler would (the file's `static` function first, then the program's own files,
then the libraries; member calls only to classes the file's `#include` closure declares, and a
name the C++ standard library's containers also use (`size`, `push_back`, ...) only when the
calling function names that class). From each program's `main` it walks every reachable function.

**Checks.** The definitions it finds match a plain count of the functions in the large files
(`mri.cpp` 351 found, 348 lines starting with `{`; `mriio.cpp` 93 and 93; `gca.cpp` 293 and
295; `gcamorph.cpp` 209 and 209). After preprocessing, the braces of 947 of the 949 files read
balance; the two that do not, `include/romp_for_begin.h` and `romp_for_end.h`, are fragments
included in the middle of a loop by design. Paths it reports were read in the source (`--why`):
`fs_temp_dir` reaches `libxml2` through `--help`'s XML rendering (`ArgumentParser::parse` →
`outputHelpXml`); every volume reader reaches the DICOM library through `mri_identify` →
`IsDICOM` → `DCM_OpenFile`; `mghRead` reaches MINC's `input_transform_file` through the
`talairach.xfm` tag an MGZ file can carry (`MRITAGread`), which is how FreeSurfer reads MNI's
`.xfm` transforms. Three earlier versions of the resolver over-reached (a local variable named
`hash` pulled in the JSON library's `std::hash`, `vector::push_back` its `basic_json::push_back`,
a template's `::value` a fake table); each was fixed and is described in the tool.

**Three views.** It over-approximates where only types decide (virtual calls, overloads), and
`mri_read` / `MRIwriteType` dispatch on the file's type, so statically **every program that reads
a volume reaches every format reader**: 130,000 to 174,000 lines each (*static reach, every
format* below; the official binaries do contain all of that code). The **core** view stops at
the readers and writers of the formats neither recon-all 8.2.0 nor fMRIPrep's FreeSurfer calls
read or write (COR, Siemens, GE Genesis/Signa/LX, AFNI, MINC volumes, SDT, GDF, DICOM, Bruker,
XIMG, NRRD, the image formats; `callgraph.py`'s `UNUSED_FORMATS`) and at `--help`'s XML rendering;
it keeps MGH/MGZ, NIfTI, ANALYZE (`talairach_avi`'s `.img`), GIFTI (`mris_convert` for fMRIPrep),
`.m3z`, LTA and MNI `.xfm`. The **calls-only** core view also drops names in a value position
(function pointers, and false hits on variables with a function's name): 10 to 18 % fewer lines,
so most of the reach is direct calls. The 4dfp tools' Fortran routines are not indexed (their
C `main` is; their size is their own lines in §1).

Programs in order of core reach (lines of functions; *in the packages*: FreeSurfer's bundled
third-party libraries):

| Program | Core reach: functions | lines in its own files | in `utils` and `include/` | in the packages | all lines (core) | calls only (core) | static reach, every format |
|---|---:|---:|---:|---:|---:|---:|---:|
| `mris_place_surface` | 3,460 | 1,791 | 71,493 | 15,931 | 89,215 | 80,139 | 174,070 |
| `mris_register` | 3,331 | 1,267 | 66,843 | 15,931 | 84,041 | 75,016 | 168,878 |
| `mris_sphere` | 3,343 | 761 | 66,901 | 15,931 | 83,593 | 74,674 | 168,429 |
| `mri_ca_label` | 2,597 | 4,745 | 61,942 | 15,931 | 82,618 | 74,195 | 167,427 |
| `mris_fix_topology` | 2,764 | 787 | 65,007 | 15,931 | 81,725 | 71,986 | 166,670 |
| `mris_expand` | 3,281 | 411 | 64,646 | 15,931 | 80,988 | 72,069 | 163,624 |
| `mri_ca_register` | 1,633 | 2,376 | 58,688 | 15,956 | 77,020 | 68,561 | 161,897 |
| `mris_inflate` | 3,233 | 597 | 59,069 | 15,931 | 75,597 | 66,676 | 160,434 |
| `mris_topo_fixer` | 3,176 | 4,451 | 54,822 | 15,931 | 75,204 | 65,988 | 157,949 |
| `mri_watershed` | 2,636 | 10,291 | 48,687 | 15,931 | 74,909 | 65,993 | 159,855 |
| `mri_normalize` | 2,393 | 1,820 | 52,046 | 15,956 | 69,822 | 61,317 | 154,742 |
| `mri_surf2surf` | 2,474 | 2,104 | 50,956 | 15,931 | 68,991 | 60,647 | 151,737 |
| `mris_smooth` | 2,675 | 771 | 52,180 | 15,931 | 68,882 | 59,827 | 153,738 |
| `mri_label2label` | 2,539 | 1,406 | 50,474 | 15,931 | 67,811 | 59,335 | 150,557 |
| `mris_convert` | 2,445 | 1,173 | 50,608 | 15,931 | 67,712 | 59,340 | 152,658 |
| `mris_remesh` | 2,534 | 2,105 | 49,225 | 15,931 | 67,261 | 58,787 | 152,202 |
| `mris_curvature` | 2,548 | 639 | 49,783 | 15,931 | 66,353 | 57,887 | 151,298 |
| `mris_curvature_stats` | 2,517 | 2,920 | 47,384 | 15,931 | 66,235 | 57,716 | 148,980 |
| `mri_vol2surf` | 2,369 | 1,906 | 47,992 | 15,931 | 65,829 | 57,494 | 148,575 |
| `mri_binarize` | 2,343 | 1,292 | 48,435 | 15,931 | 65,658 | 57,294 | 148,404 |
| `mris_ca_label` | 2,434 | 506 | 48,404 | 15,931 | 64,841 | 56,509 | 149,787 |
| `mris_remove_intersection` | 2,525 | 390 | 48,347 | 15,931 | 64,668 | 56,199 | 149,613 |
| `mri_segstats` | 2,320 | 2,476 | 46,088 | 15,956 | 64,520 | 56,112 | 149,441 |
| `mri_segreg` | 2,326 | 2,005 | 46,137 | 15,931 | 64,073 | 55,658 | 146,818 |
| `mris_anatomical_stats` | 2,382 | 1,282 | 46,685 | 15,931 | 63,898 | 55,525 | 148,844 |
| `mrisp_paint` | 2,341 | 309 | 47,644 | 15,931 | 63,884 | 55,419 | 148,830 |
| `mri_label2vol` | 2,346 | 1,223 | 46,665 | 15,931 | 63,819 | 55,425 | 146,565 |
| `mri_em_register` | 1,452 | 2,620 | 45,121 | 15,956 | 63,697 | 54,708 | 148,575 |
| `tkregister2_cmdl` | 2,268 | 3,834 | 43,904 | 15,931 | 63,669 | 55,316 | 146,279 |
| `mri_vol2vol` | 1,404 | 2,165 | 45,201 | 15,931 | 63,297 | 54,885 | 146,089 |
| `mris_volmask` | 2,285 | 1,117 | 44,981 | 15,931 | 62,029 | 53,663 | 146,975 |
| `mri_brainvol_stats` | 2,296 | 30 | 45,580 | 15,931 | 61,541 | 53,176 | 146,485 |
| `mris_defects_pointset` | 2,339 | 142 | 45,428 | 15,931 | 61,501 | 53,136 | 146,445 |
| `mris_label2annot` | 2,277 | 685 | 44,808 | 15,931 | 61,424 | 53,101 | 144,170 |
| `mri_robust_template` | 1,419 | 9,182 | 35,789 | 15,931 | 60,902 | 52,410 | 145,894 |
| `mris_extract_main_component` | 2,227 | 25 | 44,799 | 15,931 | 60,755 | 52,437 | 145,701 |
| `mris_autodet_gwstats` | 2,248 | 290 | 44,507 | 15,931 | 60,728 | 52,247 | 143,473 |
| `mris_jacobian` | 2,226 | 238 | 44,417 | 15,931 | 60,586 | 52,263 | 145,532 |
| `mri_surf2volseg` | 2,273 | 859 | 43,636 | 15,931 | 60,426 | 52,060 | 145,372 |
| `mri_cor2label` | 2,274 | 493 | 43,995 | 15,931 | 60,419 | 51,910 | 143,165 |
| `mri_relabel_hypointensities` | 2,226 | 227 | 43,970 | 15,931 | 60,128 | 51,761 | 142,874 |
| `mri_convert` | 1,270 | 3,912 | 39,876 | 15,931 | 59,719 | 51,339 | 144,539 |
| `mris_euler_number` | 2,211 | 160 | 43,243 | 15,931 | 59,334 | 50,969 | 144,280 |
| `mri_robust_register` | 1,367 | 6,729 | 36,129 | 15,931 | 58,789 | 50,418 | 143,806 |
| `mri_ca_normalize` | 1,419 | 1,927 | 40,738 | 15,956 | 58,621 | 50,209 | 143,589 |
| `mri_fill` | 1,296 | 6,228 | 36,082 | 15,956 | 58,266 | 49,870 | 143,235 |
| `mri_tessellate` | 2,120 | 568 | 40,918 | 15,931 | 57,417 | 49,102 | 142,390 |
| `mri_segment` | 1,245 | 1,128 | 37,036 | 15,982 | 54,146 | 45,760 | 139,088 |
| `mri_cc` | 1,190 | 2,500 | 34,960 | 15,956 | 53,416 | 45,026 | 138,385 |
| `mri_coreg` | 1,251 | 1,799 | 34,954 | 15,931 | 52,684 | 44,281 | 135,478 |
| `mri_edit_wm_with_aseg` | 1,157 | 3,085 | 32,269 | 15,931 | 51,285 | 42,914 | 136,279 |
| `mri_mask` | 1,198 | 563 | 34,494 | 15,931 | 50,988 | 42,617 | 135,982 |
| `mri_concat` | 1,158 | 1,269 | 33,452 | 15,931 | 50,652 | 42,270 | 133,446 |
| `AntsN4BiasFieldCorrectionFs` | 1,202 | 271 | 34,190 | 15,931 | 50,392 | 42,021 | 135,384 |
| `lta_convert` | 1,143 | 1,286 | 33,051 | 15,931 | 50,268 | 41,897 | 135,262 |
| `mri_warp_convert` | 1,157 | 868 | 33,392 | 15,931 | 50,191 | 41,820 | 135,185 |
| `mris_calc` | 1,201 | 2,093 | 32,022 | 15,931 | 50,046 | 41,356 | 135,002 |
| `mri_concatenate_lta` | 1,124 | 624 | 32,829 | 15,931 | 49,384 | 41,013 | 132,178 |
| `mri_info` | 1,119 | 1,333 | 31,985 | 15,931 | 49,249 | 40,878 | 132,016 |
| `mri_make_uchar` | 1,137 | 219 | 32,980 | 15,982 | 49,181 | 40,810 | 131,924 |
| `talairach_afd` | 1,126 | 554 | 32,672 | 15,931 | 49,157 | 40,786 | 134,151 |
| `mri_pretess` | 1,125 | 1,342 | 31,695 | 15,931 | 48,968 | 40,597 | 133,962 |
| `mri_diff` | 1,108 | 1,095 | 31,711 | 15,931 | 48,737 | 40,366 | 131,531 |
| `mri_compute_seg_overlap` | 1,107 | 483 | 31,955 | 15,931 | 48,369 | 39,998 | 133,363 |
| `AntsDenoiseImageFs` | 1,116 | 99 | 31,963 | 15,931 | 47,993 | 39,622 | 132,985 |
| `mri_seg_diff` | 1,105 | 235 | 31,700 | 15,931 | 47,866 | 39,495 | 130,660 |
| `mri_add_xform_to_header` | 1,099 | 169 | 31,703 | 15,931 | 47,803 | 39,432 | 132,761 |
| `mri_matrix_multiply` | 54 | 316 | 1,889 | 436 | 2,641 | 2,139 | 2,641 |
| `gauss_4dfp` | 31 | 883 | 0 | 0 | 883 | 883 | 883 |
| `analyzeto4dfp` | 22 | 551 | 0 | 0 | 551 | 551 | 551 |
| `fs_temp_file` | 34 | 19 | 523 | 0 | 542 | 537 | 2,742 |
| `fs_temp_dir` | 34 | 17 | 522 | 0 | 539 | 534 | 2,739 |
| `imgreg_4dfp` | 14 | 387 | 0 | 0 | 387 | 387 | 387 |
| `compute_vox2vox` | 7 | 226 | 0 | 0 | 226 | 226 | 226 |

## 3. The foundations: what the programs share

The 72 compiled programs that run on a default path (streams `v8`, `classic` or `fmriprep` in
[programs.toml](programs.toml)), core view. A function counts in a column when at least that many
programs reach it.

| Library | Lines of functions indexed | reached by 1+ of the 72 programs | 2+ | 18+ (25 %) | 36+ (50 %) | 65+ (90 %) |
|---|---:|---:|---:|---:|---:|---:|
| `utils` | 321,264 | 165,282 | 104,392 | 41,794 | 40,164 | 30,363 |
| `include` | 31,124 | 11,501 | 9,471 | 2,639 | 2,635 | 1,185 |
| `expat` | 7,595 | 5,809 | 5,809 | 5,809 | 5,809 | 5,809 |
| `gifti` | 6,622 | 4,556 | 4,556 | 4,556 | 4,556 | 4,556 |
| `dicom` | 8,429 | 4,371 | 4,371 | 4,371 | 4,371 | 4,371 |
| `minc` | 12,642 | 1,676 | 1,676 | 1,625 | 1,625 | 1,625 |
| `dcm2niix` | 25,946 | 1,395 | 1,395 | 1,395 | 1,395 | 1,395 |
| `nifti` | 6,279 | 335 | 335 | 335 | 335 | 335 |
| `tiff` | 14,305 | 26 | 26 | 0 | 0 | 0 |
| `nrrdio` | 13,336 | 4 | 4 | 4 | 4 | 4 |
| `cephes` | 345 | 2 | 2 | 2 | 2 | 2 |
| `jpeg` | 14,424 | 0 | 0 | 0 | 0 | 0 |
| `netcdf` | 21,625 | 0 | 0 | 0 | 0 | 0 |
| `xml2` | 30,371 | 0 | 0 | 0 | 0 | 0 |
| **all** | 514,307 | 194,957 | 132,037 | 62,530 | 60,896 | 49,645 |

So the libraries hold about 195,000 lines of functions that some program on a default path can
reach, 132,000 of them shared by two programs or more, and 50,000 reached by nearly every program
(the base layer below). The programs' own files add 158,929 lines (the sum of §1's own lines over
the 72; less in reach, as some of their functions are never called).

Per file of `utils/` and `include/` (files with at least 400 lines reached; *programs reaching
it*: at least one function):

| File | Lines (file) | Lines of functions indexed | reached (union) | 2+ programs | 50 % | 90 % | Programs reaching it |
|---|---:|---:|---:|---:|---:|---:|---:|
| `utils/gca.cpp` | 23,592 | 22,831 | 15,226 | 9,683 | 2,791 | 2,791 | 65 |
| `utils/mrisurf_defect.cpp` | 15,533 | 14,674 | 14,374 | 1,758 | 0 | 0 | 11 |
| `utils/gcamorph.cpp` | 17,465 | 16,670 | 12,787 | 9,285 | 1,927 | 1,927 | 65 |
| `utils/mri.cpp` | 18,000 | 16,125 | 10,054 | 7,697 | 4,014 | 4,014 | 65 |
| `utils/mrisurf_metricProperties.cpp` | 13,772 | 12,037 | 8,521 | 5,646 | 563 | 0 | 39 |
| `utils/mriio.cpp` | 12,900 | 12,360 | 6,796 | 5,997 | 5,596 | 5,596 | 65 |
| `utils/mrisurf_io.cpp` | 6,990 | 6,231 | 4,731 | 4,657 | 1,742 | 0 | 39 |
| `utils/mrisurf_mri.cpp` | 10,516 | 9,869 | 4,676 | 1,428 | 173 | 0 | 38 |
| `utils/mrisurf_compute_dxyz.cpp` | 5,088 | 4,768 | 4,284 | 3,123 | 0 | 0 | 5 |
| `utils/transform.cpp` | 5,795 | 5,171 | 3,854 | 2,985 | 1,735 | 936 | 65 |
| `utils/mri2.cpp` | 8,126 | 7,050 | 3,589 | 1,231 | 120 | 120 | 65 |
| `include/json.h` | 18,879 | 7,215 | 3,508 | 3,504 | 0 | 0 | 13 |
| `utils/DICOMRead.cpp` | 7,718 | 6,914 | 3,484 | 3,484 | 3,475 | 3,475 | 65 |
| `utils/mrisurf_deform.cpp` | 5,700 | 4,932 | 3,450 | 1,770 | 0 | 0 | 15 |
| `utils/mrisurf_integrate.cpp` | 3,108 | 2,979 | 2,979 | 1,425 | 0 | 0 | 5 |
| `utils/mrinorm.cpp` | 3,663 | 3,335 | 2,944 | 1,195 | 831 | 831 | 65 |
| `utils/mrifilter.cpp` | 7,305 | 6,180 | 2,874 | 1,864 | 0 | 0 | 16 |
| `utils/gifti.cpp` | 3,007 | 2,818 | 2,709 | 2,645 | 1,574 | 720 | 65 |
| `utils/mrisurf_topology.cpp` | 3,417 | 3,141 | 2,686 | 1,937 | 869 | 0 | 39 |
| `utils/mrisurf_timeStep.cpp` | 2,720 | 2,556 | 2,207 | 2,101 | 0 | 0 | 8 |
| `utils/matrix.cpp` | 4,916 | 4,249 | 2,173 | 2,005 | 1,410 | 1,273 | 66 |
| `utils/mriBSpline.cpp` | 2,977 | 2,320 | 2,057 | 1,768 | 785 | 785 | 65 |
| `include/mrisurf_SurfaceFromMRISPV_generated_suffix.h` | 8,909 | 8,694 | 2,019 | 1,725 | 498 | 21 | 65 |
| `utils/mriset.cpp` | 4,022 | 3,490 | 1,930 | 1,063 | 95 | 95 | 65 |
| `utils/mrisurf_vals.cpp` | 4,218 | 3,697 | 1,826 | 1,241 | 0 | 0 | 27 |
| `include/mrisurf_SurfaceFromMRIS_generated_suffix.h` | 8,792 | 8,577 | 1,776 | 1,482 | 498 | 21 | 65 |
| `utils/mrisp.cpp` | 3,044 | 2,775 | 1,751 | 1,302 | 0 | 0 | 9 |
| `utils/colortab.cpp` | 2,751 | 2,423 | 1,744 | 1,354 | 923 | 901 | 65 |
| `utils/resample.cpp` | 2,984 | 2,498 | 1,697 | 204 | 10 | 0 | 55 |
| `utils/cma.cpp` | 2,392 | 2,147 | 1,647 | 415 | 0 | 0 | 7 |
| `utils/mrimorph.cpp` | 7,541 | 6,575 | 1,607 | 886 | 0 | 0 | 5 |
| `utils/realm.cpp` | 2,594 | 1,786 | 1,600 | 560 | 560 | 0 | 39 |
| `utils/label.cpp` | 4,288 | 3,679 | 1,438 | 809 | 0 | 0 | 26 |
| `include/mrisurf_SurfaceFromMRIS_MP_generated_suffix.h` | 3,245 | 2,993 | 1,376 | 1,274 | 441 | 21 | 65 |
| `utils/histo.cpp` | 3,038 | 2,541 | 1,253 | 1,029 | 0 | 0 | 26 |
| `utils/mrisurf_base.cpp` | 1,843 | 1,449 | 1,119 | 933 | 433 | 0 | 40 |
| `utils/mripolv.cpp` | 4,450 | 3,890 | 1,093 | 0 | 0 | 0 | 1 |
| `utils/gcsa.cpp` | 2,610 | 2,178 | 1,079 | 337 | 0 | 0 | 2 |
| `utils/mrisurf_metricProperties_fast.h` | 1,175 | 1,083 | 1,066 | 1,066 | 962 | 0 | 39 |
| `utils/fstagsio.cpp` | 1,196 | 1,018 | 1,018 | 1,018 | 1,018 | 1,018 | 65 |
| `utils/mri_identify.cpp` | 1,209 | 1,048 | 983 | 983 | 852 | 808 | 65 |
| `utils/mrisurf_metricProperties_slow.h` | 1,009 | 957 | 957 | 957 | 689 | 0 | 39 |
| `utils/numerics.cpp` | 2,353 | 1,718 | 954 | 664 | 156 | 156 | 66 |
| `utils/utils.cpp` | 1,999 | 1,335 | 941 | 852 | 497 | 332 | 67 |
| `utils/mrisegment.cpp` | 1,112 | 957 | 904 | 511 | 0 | 0 | 7 |
| `utils/matfile.cpp` | 1,170 | 1,065 | 861 | 861 | 861 | 861 | 66 |
| `utils/mrisutils.cpp` | 4,205 | 3,579 | 858 | 321 | 18 | 0 | 38 |
| `utils/fmriutils.cpp` | 3,308 | 2,918 | 814 | 65 | 0 | 0 | 6 |
| `utils/volcluster.cpp` | 2,589 | 2,300 | 792 | 682 | 0 | 0 | 9 |
| `utils/mrisurf.cpp` | 1,008 | 850 | 751 | 198 | 198 | 0 | 39 |
| `utils/mrihisto.cpp` | 1,361 | 1,169 | 747 | 311 | 0 | 0 | 9 |
| `utils/surfgrad.cpp` | 2,716 | 2,320 | 738 | 473 | 0 | 0 | 4 |
| `utils/warpfield.cpp` | 969 | 857 | 694 | 694 | 694 | 694 | 65 |
| `utils/mrisurf_metricProperties_faster.cpp` | 868 | 692 | 692 | 692 | 692 | 0 | 39 |
| `utils/image.cpp` | 3,823 | 3,252 | 671 | 671 | 114 | 114 | 65 |
| `include/MRISOBBTree.h` | 877 | 661 | 650 | 0 | 0 | 0 | 1 |
| `utils/mriflood.cpp` | 1,823 | 1,695 | 638 | 406 | 0 | 0 | 15 |
| `utils/fio.cpp` | 1,142 | 964 | 564 | 546 | 496 | 427 | 65 |
| `utils/MRISrigidBodyAlignGlobal.cpp` | 543 | 517 | 517 | 0 | 0 | 0 | 1 |
| `utils/connectcomp.cpp` | 1,338 | 1,149 | 499 | 307 | 0 | 0 | 4 |
| `utils/stats.cpp` | 1,809 | 1,686 | 465 | 0 | 0 | 0 | 2 |
| `utils/mrisurf_sseTerms.cpp` | 3,152 | 2,766 | 444 | 289 | 0 | 0 | 5 |
| `utils/autoencoder.cpp` | 2,250 | 2,134 | 425 | 425 | 425 | 425 | 65 |
| `include/utilsmath.h` | 730 | 575 | 424 | 0 | 0 | 0 | 1 |
| `utils/path.cpp` | 451 | 419 | 419 | 0 | 0 | 0 | 1 |
| `utils/voxlist.cpp` | 1,222 | 1,166 | 405 | 282 | 0 | 0 | 5 |
| `utils/fscnpy.cpp` | 630 | 431 | 402 | 402 | 402 | 402 | 65 |
| (134 more files, under 400 lines reached each) | | | 11,572 | | | | |

Files of `utils/` none of the 72 programs reaches (core view): 62, 44,120 lines of functions: `GradUnwarp.cpp`, `MRIScomputeTriangleProperties_extracted.h`, `afni.cpp`, `box.cpp`, `class_array.cpp`, `cluster.cpp`, `cmat.cpp`, `corio.cpp`, `cvector.cpp`, `dct.cpp`, `density.cpp`, `evschutils.cpp`, `fcd.cpp`, `fftutils.cpp`, `fsPrintHelp.cpp`, `fsglm.cpp`, `gcaboundary.cpp`, `gcamorphtestutils.cpp`, `gcautils.cpp`, `gclass.cpp`, `geos.cpp`, `gw_ic2562.cpp`, `gw_utils.cpp`, `hippo.cpp`, `ic12.cpp`, `ic163842.cpp`, `ic2562.cpp`, `ic42.cpp`, `ic642.cpp`, `imageio.cpp`, `imautils.cpp`, `intensity_eig.cpp`, `ipmpar.cpp`, `joint_histo.cpp`, `kernel.cpp`, `legendre.cpp`, `lut.cpp`, `mgh_filter.cpp`, `mideface.cpp`, `morph.cpp`, `mri_level_set.cpp`, `mri_tess.cpp`, `mri_topology.cpp`, `mriclass.cpp`, `mricurv.cpp`, `mrisComputeVertexDistancesWkr_extracted.h`, `mris_compVolFrac.cpp`, `mris_fastmarching.cpp`, `mris_sphshapepvf.cpp`, `mrisurf_sphere_interp.cpp`, `offset.cpp`, `pgmstubs.cpp`, `rbf.cpp`, `rbm.cpp`, `retinotopy.cpp`, `rforest.cpp`, `svm.cpp`, `tfce.cpp`, `tukey.cpp`, `vlabels.cpp`, `xTypes.cpp`, `xVoxel.cpp`.

**The base layer**: the functions 65 or more of the 72 programs reach, by file. They are the
volume I/O of `MRIread` / `MRIwrite` (MGH/MGZ, NIfTI, GIFTI volumes, `.m3z` through `GCAMread`,
type identification with its DICOM probe, MINC's `.xfm` reader), the `MRI` structure
(`mri.cpp`), matrices, transforms, colour tables, tags, error handling and the licence check
(`chklc.cpp`: a run-time check of the key; CLAUDE.md, "Settled technical choices"):

| File | Functions reached by 65+ programs | their lines |
|---|---:|---:|
| `utils/mriio.cpp` | 44 | 5,596 |
| `packages/expat/xmlparse.c` | 80 | 4,350 |
| `utils/mri.cpp` | 71 | 4,014 |
| `utils/DICOMRead.cpp` | 50 | 3,475 |
| `utils/gca.cpp` | 41 | 2,791 |
| `packages/gifti/gifti_xml.c` | 81 | 2,780 |
| `packages/dicom/dcm.c` | 38 | 2,228 |
| `utils/gcamorph.cpp` | 30 | 1,927 |
| `packages/dicom/dcmdict.c` | 38 | 1,822 |
| `packages/gifti/gifti_io.c` | 73 | 1,776 |
| `packages/dcm2niix/miniz.c` | 42 | 1,395 |
| `utils/matrix.cpp` | 35 | 1,273 |
| `packages/expat/xmlrole.c` | 75 | 1,175 |
| `packages/minc/minc_volume_io.c` | 32 | 1,020 |
| `utils/fstagsio.cpp` | 43 | 1,018 |
| `utils/transform.cpp` | 19 | 936 |
| `utils/colortab.cpp` | 17 | 901 |
| `utils/matfile.cpp` | 13 | 861 |
| `utils/mrinorm.cpp` | 8 | 831 |
| `utils/mri_identify.cpp` | 24 | 808 |
| `utils/mriBSpline.cpp` | 15 | 785 |
| `utils/gifti.cpp` | 7 | 720 |
| `utils/warpfield.cpp` | 13 | 694 |
| `packages/minc/minc_files.c` | 21 | 464 |
| `utils/fio.cpp` | 25 | 427 |
| `utils/autoencoder.cpp` | 11 | 425 |
| `utils/fscnpy.cpp` | 7 | 402 |
| `include/gcalinearnode.hpp` | 30 | 395 |
| `include/mri.h` | 20 | 363 |
| `utils/chklc.cpp` | 7 | 354 |
| (43 more files) | 205 | 3,639 |

## 4. The Python programs

The four network programs and three helper scripts on a default path (programs.toml), with the
imports their main file makes (`<workspace>/tags/freesurfer-8.2.0`):

| Program | Lines | Imports |
|---|---:|---|
| `mri_synthstrip` | 326 | `torch`, `torch.nn`, `numpy`, `surfa` |
| `mri_synthseg` | 2,715 | `tensorflow`, `tensorflow.keras`, `numpy`, `scipy.ndimage`, `scipy.interpolate`, `nibabel`, `surfa`; neurite's layers copied in the file ("Neurite Utilities", line 1053) |
| `mri_synthmorph` | 419 | `surfa` and the package `mri_synthmorph/synthmorph/` (`registration.py`, `utils.py`: `tensorflow`, `voxelmorph`, `h5py`, `numpy`, `surfa`) |
| `mri_sclimbic_seg` | 1,077 | `tensorflow`, `numpy`, `scipy.ndimage`, `surfa`; neurite's encoder and decoder "Copied with permission from github.com/adalca/neurite" (lines 728-948) |
| `rca-config`, `rca-config2csh` | 143, 48 | `yaml` |
| `csvprint` | 223 | standard library only |

The installed package's Python (`python/lib/python3.8/site-packages` of the official 8.2.0):
CPython 3.8, numpy 1.24.3, SciPy 1.10.1, TensorFlow 2.13.1 with Keras 2.13.1, PyTorch 2.1.2
(CPU build), surfa 0.6.3, nibabel 5.2.1, h5py 3.11.0, PyYAML 6.0.2, OpenCV 4.11.0.86, neurite and
voxelmorph 0.2. The networks' weights sit beside the programs in the tree (`synthstrip.1.pt`,
`synthstrip.nocsf.1.pt`; `synthseg_*.h5` and the label tables `synthseg_*.npy`;
`synthmorph.{rigid.1,affine.2,deform.3}.h5`; `sclimbic*.h5`, `mca-dura.both-lh.nstd21.fhs.h5`,
`vsinus.no-sp.m.all.nstd10-070.h5`, `entowm.fsm31.t1.nstd00-30.nstd21-108.h5`): sub-task 4 lists
them with sizes and sources.

## 5. What this shows for the plan (observations for sub-task 6)

- **A shared base of about 50,000 lines** (§3's last table) comes before every program: the volume
  formats recon-all uses, the `MRI` structure, matrices, LTA and `.xfm` transforms, colour tables.
  Most of it is I/O that larmorx partly has (`larmorx-io`: NIfTI, MGH); the formats' own parsing
  libraries (GIFTI over `expat`, the DICOM probe, MINC's `.xfm` reader) are third-party code for
  sub-task 3.
- **Clusters by file** (§3's per-file table, *programs reaching it*): the surface library
  (`mrisurf_*.cpp`, reached by 38 to 40 programs, the largest shared block after the base:
  `mrisurf_metricProperties.cpp` 8,521 lines, `mrisurf_io.cpp` 4,731, `mrisurf_mri.cpp` 4,676,
  `mrisurf_topology.cpp` 2,686, `mrisurf_base.cpp` 1,119), the topology fixer's own block
  (`mrisurf_defect.cpp`, 14,374 lines, 11 programs), the deformation and integration code
  (`mrisurf_deform.cpp`, `mrisurf_integrate.cpp`, `mrisurf_compute_dxyz.cpp`,
  `mrisurf_timeStep.cpp`: 5 to 15 programs), GCA and GCA morphs (`gca.cpp`, `gcamorph.cpp`:
  15,226 and 12,787 lines in reach; 2,000 to 3,000 lines of each in the base through `.m3z`
  reading, the rest for the `mri_ca_*` programs and `mri_em_register`), filters and morphology
  (`mrifilter.cpp`, 16 programs).
- **The largest programs by core reach** are `mris_place_surface`, `mris_register`,
  `mris_sphere`, `mri_ca_label`, `mris_fix_topology`, `mris_expand` and `mri_ca_register` (77,000
  to 89,000 lines each, about 48,000 of them the base); the converters (`mri_add_xform_to_header`,
  `mri_diff`, `mri_info`, `mri_concatenate_lta`, `lta_convert`: 47,800 to 50,300 lines) are little
  more than the base.
- **62 files of `utils/` (44,120 lines) no program on a default path reaches**: they stay out of
  `crates-freesurfer/` until a program needs them.

## 6. Regenerating

```
python -P validation/investigations/freesurfer-inventory/cmake_targets.py \
    <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml --json <scratch>/cmake.json
python -P validation/investigations/freesurfer-inventory/callgraph.py \
    <workspace>/tags/freesurfer-8.2.0 <scratch>/cmake.json --toml docs/freesurfer/programs.toml \
    --json <scratch>/full.json
```

and the same `callgraph.py` call with `--core` (into `core.json`) and with `--core --calls-only`
(into `core-calls.json`), about 90 seconds each; then

```
python -P validation/investigations/freesurfer-inventory/report_code.py \
    docs/freesurfer/programs.toml <scratch>/cmake.json <scratch>/full.json <scratch>/core.json \
    <scratch>/core-calls.json --toml-out docs/freesurfer/code.toml > <scratch>/tables.md
```

prints the tables of §1 to §3 and writes `code.toml`. `callgraph.py --programs <name> --core
--why <file or function>` prints the path from `main` to what it names.
