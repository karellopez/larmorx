# Licensing: replicas and originals

larmorx replicates tools whose licences differ. This page records how each upstream is
handled. Decided by the user on 2026-10-09: accuracy first, and **both** implementations
where the licence restricts a replica. Not legal advice; when in doubt, ask a lawyer.

## The two tracks

| Track | What it is | How it is written | Licence | Where it lives |
|---|---|---|---|---|
| **Replica** | Bit-for-bit port of the upstream code, for the most accurate output possible | From the upstream source; operation order, constants and edge cases kept | The upstream's (or a compatible one) | A package per licence family (below) |
| **Original** | Independent implementation with the same behaviour | Strict clean-room: a spec (`specs/<tool>.md`) written by someone who may read the source, and an implementer who never sees the source **or the replica** | Apache-2.0 | The main `larmorx` package |

For permissively licensed upstreams one track is enough: the replica is Apache-compatible
and lives in the main package.

## Upstream by upstream

| Upstream | Licence (as stated by the upstream) | Replica | Original (clean-room) |
|---|---|---|---|
| ANTs, ITK | Apache-2.0 | ✓ main package | not needed |
| AFNI, NIH-written files | public domain ("United States Government Work") | ✓ main package, after a per-routine check | not needed |
| AFNI, files copyrighted by the Medical College of Wisconsin | CC BY 4.0 from AFNI 26.3.05 ("As of May 12, 2026, MCW has granted NIH a Creative Commons Attribution License (CC BY 4.0) for all of the code under its copyright", `doc/README/README.copyright`); GPL "Version 2 (or any later edition)" until 25.2.09 | ✓ main package since 2026-10-29 (`crates/larmorx-afni-replica`, with CC BY 4.0's attribution; `larmorx-gpl` until then) | ✓ main package (built for speed) |
| Connectome Workbench | GPL-2.0-or-later (file headers: "version 2 … or (at your option) any later version") | ✓ `larmorx-gpl`, as GPL-3.0-or-later | ✓ main package |
| FSL | FSL Licence: non-commercial. Modification and transmission allowed "without financial return", if the licence conditions are imposed on the receiver and all original and amended source is included (`~/fsl/LICENCE.FSL`) | ✓ `larmorx-nc`, under the FSL Licence's terms | ✓ main package |
| FreeSurfer | FreeSurfer Software License v1.0: derived works and redistribution allowed, also commercially, if the licence text with its required preface ships with them, notices are kept and modifications marked. Every user inherits "research use only" and an indemnity in MGH's favour | ✓ `larmorx-freesurfer` (planned, D3) | not planned (not required; revisit for single parts if needed) |
| MSM_HOCR | non-commercial. Its ELC library: "derivatives must not be publicly distributed without a prior consent". Its FastPD optimiser: "protected by … patent applications" | ✗ cannot be published | ✓ main package, in the surfaces track, **without** the FastPD/ELC optimisation methods (patents cover methods, however they are coded). First step: a patent check of FastPD and HOCR |
| tedana | LGPL-2.1 (only: its `LICENSE` is the LGPL-2.1 text, its documentation names "version 2.1", no "any later version" and no file headers) | ✓ `larmorx-gpl` since 2026-11-08 (`crates-gpl/larmorx-gpl-tedana`: an exact replica of tedana 26.0.3's `t2smap`, GPL-3.0-only under LGPL-2.1's section 3, which lets a copy of the library carry the ordinary GPL instead; the user, 2026-11-08) | ✓ main package (planned: `crates/larmorx-tedana`, from `specs/t2smap.md`, the published equations and black-box runs) |
| nilearn (`compute_epi_mask`, as tedana calls it) | BSD-3-Clause | ✓ main package since 2026-11-08 (`larmorx_interp::nilearn`; the notice in `NOTICE`) | not needed |
| CPython 3.12.16 (`argparse`, `textwrap`, `json`'s encoder, Python's number parsing and printing: a ported Python program's command line and messages) | PSF License Version 2 | ✓ main package since 2026-11-08 (`larmorx_core::python`, re-expressed in Rust; the licence and the summary of changes its clause 3 asks for in `NOTICE`) | not needed |
| CORE-MATH, nibabel | MIT | ✓ main package | not needed |
| sdcflows (the NiPreps developers; its field-map estimation) | Apache-2.0 | ✓ main package since 2026-11-08 (`crates/larmorx-sdcflows`, `lx.sdcflows`: an exact port of sdcflows 2.17.1, the field maps' part A; the notice in `NOTICE`) | not needed for the licence; a later clean-room original of every program (the user, 2026-11-06), in another crate |
| scikit-learn, SciPy, numpy, OpenBLAS and the reference LAPACK OpenBLAS bundles (their arithmetic on the paths the ports reach: `Ridge`'s `sparse_cg`, `cg`, `BSpline`, sparse products, `ndimage`, numpy's reductions, `sdot`, `svd`, `eigh`, `det`; since 2026-11-08 for tedana's replica numpy's percentiles, float32 `log`, float printing and `lstsq`, SciPy's `curve_fit` and `scoreatpercentile`) | BSD-3-Clause (LAPACK: modified BSD) | ✓ main package (followed operation by operation in `larmorx_sdcflows`, `larmorx_transform::openblas` and `numpy_linalg`, `larmorx_interp::ndimage`, `larmorx_core::numpy`, `larmorx-optim`; the notices in `NOTICE`) | not needed |
| Arm's Optimized Routines (v19.11: the `sinf`, `cosf`, `sincosf`, `logf`, `expf`, `powf` and double `exp`, `log`, `pow` glibc 2.35 ships) | MIT | ✓ main package since 2026-11-04 (`larmorx_core::math`'s `*_glibc`, `LICENSES/MIT-arm-optimized-routines.txt`): one translation for `reproduce`, the AFNI replicas and the FSL replicas' core (the AFNI replicas' `sincosf.rs` from 2026-10-29) | not needed |
| fdlibm (as glibc 2.35 carries it: `atanf`, `atan2f`, `tanhf`, `expm1f`; netlib's fdlibm 5.3: `tanh`) | Sun's notice (`LicenseRef-fdlibm`) | ✓ main package (`larmorx_core::math`'s `atanf_glibc`, `atan2f_glibc` since 2026-11-04 and `tanh_glibc`; the AFNI replicas' `tanhf`, `expm1f`) | not needed |
| EISPACK (`svd`, `pythag`: Argonne National Laboratory, 1983) and f2c's libf2c (`d_sign`), as AFNI 26.3.05 carries them (`src/eispack/`, `src/f2c/`) | EISPACK: freely available (netlib; AFNI's `LICENSE.txt`: "License: Free"); libf2c: AT&T Bell Laboratories and Bellcore's permission notice (`src/f2c/Notice`) | ✓ main package since 2026-11-06 (`crates/larmorx-afni-replica/src/eispack.rs`: 3dvolreg's 3 × 3 singular value decomposition; the notice in `NOTICE`) | the clean-room original of 3dvolreg may use any SVD of a rotation (`specs/3dvolreg.md`) |
| the GNU C Library 2.35's own code (IBM's Accurate Mathematical Library: the double `sin`, `cos`, `sincos`, `tan`, `asin`, `acos`, `atan`; `cbrtf`) | LGPL-2.1-or-later | ✗ never copied or translated (verification tools and a spec writer's model in `crates-gpl/tools/` only) | ✓ main package, from behaviour specs: `cbrtf_glibc` (`specs/cbrtf.md`), `sin`, `cos`, `sincos` (`specs/glibc-dbl.md`, its tables from definitions); `tan`, `asin`, `acos`, `atan` correctly rounded in both modes (the user, 2026-11-04) |

## Packages

Each licence family is a separate set of crates, a separate Python wheel and a separate
command-line binary. Code from different families is never linked into one binary.

| Package | Licence | Contents | Depends on |
|---|---|---|---|
| `larmorx` | Apache-2.0 | everything permissive, plus every original | – |
| `larmorx-gpl` | GPL-3.0 (the program; its tedana-derived crate GPL-3.0-only, the rest GPL-3.0-or-later) | tedana's `t2smap` replica (since 2026-11-08: the `larmorx-gpl` program and its binary-only wheel); replicas of Workbench (none yet; until 2026-10-29 also AFNI's MCW files); development tools that keep or read glibc's LGPL code (never in the wheel) | `larmorx` (Apache-2.0 code may be included in GPL-3 works) |
| `larmorx-nc` | the FSL Licence's terms (non-commercial, source included) | replicas of FSL | `larmorx` |
| `larmorx-freesurfer` | FreeSurfer Software License | replicas of FreeSurfer | `larmorx` |

- **`larmorx-gpl` and `larmorx-nc` are separate,** and neither may link the other. The FSL
  Licence's non-commercial and "impose these conditions" clauses are incompatible with the
  GPL, which forbids added restrictions.
- **A replica package is a program, not a Python module** (decided 2026-10-11). `larmorx-nc`
  is a binary-only wheel (maturin's `bin` bindings, `crates-nc/pyproject.toml`, licence
  `LicenseRef-FSL`): it installs the `larmorx-nc` program into the environment's scripts
  directory and nothing that Python can import. `larmorx-gpl` was packaged the same way for the
  AFNI replicas until 2026-10-29 (its wheel, program and `larmorx[exact]` extra are in git
  history); since 2026-11-08 it is again, for tedana's `t2smap` (below), and will be for Workbench. `larmorx-nc`'s wheel carries the FSL Licence (`crates-nc/LICENSE`), the FSL,
  Armadillo, fslpy, fdlibm, Arm and SGI STL (HP/SGI) notices (`crates-nc/NOTICE`), the
  Apache-2.0 text and `NOTICE` of the larmorx crate it includes and Arm's MIT licence
  (`crates-nc/LICENSES/`), and, because the
  FSL Licence requires "all original and amended source code" in every transmitted product,
  FSL's original source of the replicated parts (`crates-nc/fsl-source/`, with `MANIFEST.tsv`)
  and the Rust translation itself, all as licence files. `crates-nc/larmorx-nc-cli/tests/notices.rs`
  checks the copies and the source manifest.
  BET (merged 2026-10-26) is the second FSL replica: `larmorx-nc fsl bet`, `bet2` and `betsurf`,
  translated from FSL 6.0.7.17's fsl-bet2, fsl-meshclass, fsl-newimage, fsl-newnifti,
  fsl-miscmaths, fsl-utils, fsl-znzlib and fsl-avwutils, whose original source is in
  `crates-nc/fsl-source/` beside mcflirt's (one `MANIFEST.tsv`). Also translated, each with its
  notice in `crates-nc/NOTICE`: fslpy 3.21.1's `imtest`, `remove_ext` and `imglob` (Apache-2.0),
  fdlibm's `tanh` and `expm1` (Sun's notice), glibc 2.35's `pow` and `exp` from Arm's Optimized
  Routines v19.11 (MIT: the code glibc ships, as for the float functions; no LGPL code; since
  2026-11-04 these and the float functions are translated once, in `larmorx_core::math`, which
  the FSL replicas' core re-exports, with fdlibm's `atanf` and `atan2f`), and
  bet2's `--ct` conversion, which FSL adapted from Chris Rorden's BSD-2-clause `clinical_h2c.m`.
  The core's `std::sort` and `std::nth_element` emulation (FSL-22, FSL-55; the mesh sweep) is
  written from the SGI STL's introsort and introselect under the HP/SGI permission notice, with
  the one step where libstdc++'s order differs written from that step's description: no
  libstdc++ (GPL-3.0) text (decision log, 2026-10-26).
  FLIRT (2026-11-01, part 1) is the third FSL replica: `larmorx-nc fsl flirt` and `convert_xfm`,
  translated from FSL 6.0.7.23's fsl-flirt 2111.4 (`flirt.cc`, `globaloptions.{h,cc}`,
  `defaultschedule.h`, `convert_xfm.cc`; FSL's `bbr.sch` copied with them), whose files are in
  `crates-nc/fsl-source/fsl-flirt/` (`scripts/fsl_source.py`; the wheel's licence files now
  include `fsl-source/*/*/*` for the schedule).
- **`larmorx-gpl`, tedana's `t2smap`** (2026-11-08, the user's decision): tedana is LGPL-2.1
  only; its section 3 lets a copy of the library carry the ordinary GPL instead ("If a newer
  version than version 2 of the ordinary GNU General Public License has appeared, then you can
  specify that version instead"), so the tedana-derived crate `crates-gpl/larmorx-gpl-tedana` is
  GPL-3.0-only (every file's header says so: "converted from tedana's LGPL-2.1 under its section
  3", with the modification notices), and GPL-3.0 accepts the Apache-2.0 crates it links. The
  program `larmorx-gpl` (`crates-gpl/larmorx-gpl-cli`; `larmorx-gpl tedana t2smap <t2smap's
  arguments>`) is a binary-only wheel (`crates-gpl/pyproject.toml`, licence `GPL-3.0-only`;
  `larmorx[all]` installs it), whose licence files are the GPL-3.0 text (`crates-gpl/LICENSE`),
  tedana 26.0.3's `LICENSE` byte for byte (`crates-gpl/LICENSES/LGPL-2.1-tedana.txt`),
  `crates-gpl/NOTICE` (tedana's notice "Copyright 2023, The ME-ICA Developers", what was translated
  and modified, the section 3 conversion, the Apache crates included, where the source is), the
  Apache-2.0 text and `NOTICE` of the larmorx crates it includes (`crates-gpl/LICENSES/`), the
  workspace's Cargo files and the two crates' Rust source; the complete Corresponding Source is
  this repository at the release's commit, which `crates-gpl/NOTICE` names (GPL-3.0 section 6).
  `crates-gpl/larmorx-gpl-cli/tests/notices.rs` checks the copies (whenever `NOTICE` changes,
  `crates-gpl/LICENSES/NOTICE-larmorx.txt` must be the same file, as crates-nc's copy). The
  workspace's glibc tools stay out of the wheel. The permissive numerics the replica needs
  (numpy's, SciPy's, nilearn's, CPython's) are in the main package's Apache crates, so nothing
  BSD, MIT or PSF is relicensed; the clean-room original will be in the main package.
- **FSL 6.0.7.23** (checked 2026-10-29): the replicas follow it; `crates-nc/fsl-source/` holds
  its original files of everything the translation now comes from (68 files, `MANIFEST.tsv`
  with each package's version and SHA-256; fsl-znzlib 2511.3's C++ files replace 2111.0's
  `znzlib.c`, which leaves the directory with the version it belonged to, kept in git history
  and in `~/fsl`; FSL 6.0.7.17's copies also in `<workspace>/archive/2026-10-29-fsl-source-6.0.7.17/`).
  Nothing is still translated from 6.0.7.17 alone: the files 6.0.7.23 did not change are the same
  bytes. Armadillo's matrix rules (`arma.rs`) are still Armadillo 10.5.1's (fsl-armawrap 0.6.0,
  Apache-2.0, not FSL code: notice in `crates-nc/NOTICE`). New pure-Rust dependencies of
  `larmorx-nc-core` for zstd and bzip2: `ruzstd` 0.9.1 (MIT), `bzip2` 0.6.1 (MIT OR Apache-2.0)
  with `libbz2-rs-sys` 0.2.5 (the bzip2-1.0.6 licence: permissive, the copyright notice kept in
  source). No C library.
- **Armadillo 14.2.1, OpenBLAS 0.3.33 and reference LAPACK 3.12.0** (2026-10-29): the FSL
  replicas follow FSL 6.0.7.23's matrix layer, whose 4 × 4 inverses, solves and determinants go
  through LAPACK; `larmorx-nc-core/src/lapack.rs` translates the routines from OpenBLAS 0.3.33
  (BSD-3-Clause) and the reference LAPACK 3.12.0 it bundles (modified BSD), Armadillo 14.2.1's
  rules are in `arma.rs` (Apache-2.0); their notices are in `crates-nc/NOTICE`. Permissive
  sources, included in the FSL-licensed package as the other permissive ones are.
- **standard_space_roi and fslroi; fslpy's `imcp` and `immv`; the shell tools the scripts run**
  (2026-11-04, BET's `-S`/`-B`/`-A`/`-A2` replica part): `larmorx-nc fsl standard_space_roi`
  (fsl-flirt 2111.4's bash script, added to `crates-nc/fsl-source/` with `MANIFEST.tsv`) and
  `larmorx-nc fsl fslroi` (fsl-avwutils 2209.6's `fslroi.cc`) are translations under the FSL
  Licence; fslpy 3.27.0's `imcp` and `immv` rules (Apache-2.0, notice in `crates-nc/NOTICE`) are
  translated in `larmorx-nc-core/src/fslpy.rs`. GNU dc 1.4.1 (`bet -S`'s eye threshold, `-R`'s
  distance, standard_space_roi's sizes) and bash's `[`, `echo` and word splitting are emulated as
  their manuals document them and as they behave in observed runs (the crates' `PROVENANCE.md`):
  no GNU code (GPL-3.0) is copied or translated.
- **FSL's standard images** (the user's decision of 2026-11-04; decision log, "The data the
  programs read"): `larmorx-nc`'s wheel carries the three images FSL's `bet` script (`-S`, `-B`,
  `-A`, `-A2`) and `standard_space_roi` read, `MNI152_T1_2mm`, `MNI152_T1_2mm_brain_mask_dil` and
  `MNI152_T1_2mm_eye_mask` (`.nii.gz`, 1,420,897 bytes), byte for byte from FSL 6.0.7.23's
  `fsl-data_standard` 2208.0, committed in `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/`
  (maturin's wheel data: installed at `<prefix>/share/larmorx-nc/fsl-data`, where the replica finds
  them without an FSL installation) with FSL's Licence (`LICENSE`), the package's origin and MNI's
  notice (`NOTICE`, also in `crates-nc/NOTICE` and among the wheel's licence files) and
  `MANIFEST.tsv` (file, bytes, SHA-256, package, version, the programs that read it), checked by
  `crates-nc/larmorx-nc-cli/tests/fsl_data.rs`. FSL's data package names no licence of its own, so
  FSL's Licence applies, which only `larmorx-nc` meets: the main package carries none of them.
  `MNI152_T1_2mm` (FSL's MNI152, the MNI ICBM 152 non-linear 6th generation average made for FSL by
  Andrew Janke) also carries the McConnell Brain Imaging Centre's permissive notice (copies keep
  it). They are the one exception to the repository's rule of no image data in git (CLAUDE.md).
  The clean-room original (since 2026-11-05, `larmorx_fsl::data`) reads the same files as input
  data at run time: from the directory the call names (`fsl_data=`, `LARMORX_FSL_DATA`), else
  `larmorx-nc`'s copy when installed (read as data, checked against FSL's SHA-256; nothing of
  larmorx-nc is run or linked), else `$FSLDIR/data`, else larmorx's data cache, filled once by a
  checksum-verified download of FSL's package from FSL's site of which only the three images are
  kept (`larmorx fetch fsl-standard` fetches ahead; `specs/bet.md` §5.4,
  `specs/standard_space_roi.md`); the main package ships none of them, and FSL's terms then bind
  the files' use, in either implementation.
- **Nothing is on PyPI yet.** Build the wheels locally:
  ```bash
  maturin build --release --locked --out dist                    # larmorx (repository root): the originals and the AFNI replicas
  (cd crates-nc && maturin build --release --locked --out ../dist)    # larmorx-nc (FSL Licence)
  pip install --find-links dist larmorx-nc                            # the FSL replicas, by name
  pip install --find-links dist "larmorx[all]"                        # everything
  ```
- **In the repository** (all in this one repository; for FSL decided 2026-10-10):
  - `crates/` holds the Apache crates;
  - `crates-gpl/`, `crates-nc/` and `crates-freesurfer/` hold the others, each directory
    with its own `LICENSE`;
  - `crates-nc/` also holds FSL's original source files for every replicated part: the FSL
    Licence requires "all original and amended source code" in any transmitted product;
  - every source file carries an SPDX header;
  - `cargo deny` enforces the allowed licences per directory.
- **Names:** no package may present itself as AFNI, FSL, Workbench or FreeSurfer (PLAN.md
  §12); "nc" stands for non-commercial.

## Notes checked 2026-10-19

- **FreeSurfer's licence key is not a licence term.** FreeSurfer's programs refuse to run
  without `license.txt` (`FS_LICENSE`), but that is its own runtime check
  (`utils/chklc.cpp`); its `LICENSE.txt` (FreeSurfer Software License v1.0) has no
  registration or key requirement. `larmorx-freesurfer` therefore does not check for one. What
  the licence does require is kept: its full text with the required preface ("All or portions
  of this licensed product ... obtained under license from The General Hospital
  Corporation..."), the notices, modifications marked; every user inherits its section 4
  (research purposes only; clinical applications neither recommended nor advised;
  commercialisation at the user's own risk) and its as-is terms. Still to check when the
  FreeSurfer track starts: the terms of FreeSurfer's atlas files (GCA, parcellation and
  folding atlases), on which recon-all's results depend.
- **FreeSurfer's data: what recon-all needs, its size and its terms** (measured in the
  FreeSurfer 7.3.2 that fMRIPrep 25.2.6's image ships, trimmed to what fMRIPrep runs; FreeSurfer
  8's atlases are the same files):

  | Part | Unpacked | gzip-6 |
  |---|---|---|
  | `average/`: the two GCA atlases (2020), the parcellation atlases (`.gcs`, three pairs), the folding atlas, `mni305.cor.mgz`, the Talairach atlas images | 487 MB | 144 MB |
  | `subjects/fsaverage` (BA labels, the Talairach check, `--subject fsaverage`) | 503 MB | 320 MB |
  | `subjects/fsaverage6`, `fsaverage5` (fMRIPrep's surface output spaces) | 197 MB | 166 MB |
  | `mni/` (MNI's N3 tools' data) | 57 MB | 20 MB |
  | **total** | **about 1.24 GB** | **about 650 MB** |

  Not needed: FreeSurfer's own `bin/` (348 MB), `python/` (500 MB), `lib/`. Most of it may be
  redistributed under the FreeSurfer licence, on the same conditions as its code. **Exception:
  the Talairach step** (`talairach_avi`, Washington University, A. Z. Snyder): its source says
  "This software may not be reproduced, copied, or distributed without written permission of
  Washington University", and its three atlas images (`711-2C_as_mni_average_305.4dfp.img`,
  `711-2B_..._mask`, `3T18yoSchwartzReactN32_as_orig`, 192 MB unpacked) come with it. So no
  replica of `talairach_avi`, and its atlases are not ours to ship: a clean-room original of
  the step (an affine registration to MNI305, validated against recon-all's
  `talairach.xfm`), WashU's written permission, or the atlases downloaded by the user from
  FreeSurfer at first use. How fMRIPrep does it (checked 2026-10-19): its build downloads MGH's official FreeSurfer 7.3.2
  release (`fmriprep/Dockerfile.base:52`) and its published image carries a trimmed, unmodified
  copy, `talairach_avi` and the three atlas images included: FreeSurfer redistributed as
  FreeSurfer, relying on MGH's arrangement with Washington University (not documented in the
  sources; FreeSurfer's licence text is not under the image's `/opt/freesurfer` either). A Rust
  translation of WashU's code is a derivative work that arrangement would not cover. Simplest
  way to settle it: ask Washington University (A. Z. Snyder's group) for written permission to
  port the code and ship the atlases. To decide when the FreeSurfer track starts. `mni305.cor.mgz` is
  derived from MNI's MINC average (MNI's terms are permissive).
- **Eigen (MPL-2.0), one file in `larmorx-core`** (decided by the user, 2026-10-23: kept).
  `crates/larmorx-core/src/eigen_sym.rs` translates Eigen 3.4.90's self-adjoint eigensolver (as
  ITK v5.4.5 bundles it) to Rust, so ITK's `VersorRigid3DTransform` landmark fit (antsAI `-b`) is
  reproduced bit for bit. MPL-2.0 is file-level copyleft: the file stays under MPL-2.0 (SPDX
  `Apache-2.0 AND MPL-2.0`, Eigen's notice in the file and in NOTICE, its source public here),
  which MPL-2.0 allows inside an Apache-2.0 package (a "Larger Work"); Eigen does not use the
  "Incompatible With Secondary Licenses" notice, so the GPL wheel may contain it too. The
  project's licence is unchanged.
- **AFNI: one implementation or two is decided per routine.** AFNI's `LICENSE.txt`: code
  written at NIH is public domain; the "major portions" written at the Medical College of
  Wisconsin are GPL-2, marked at the top of each file. A program needs a GPL replica and a
  clean-room original when the code its results depend on comes from MCW files (dataset I/O
  aside: larmorx has its own); the first step of each AFNI port traces the call graph routine
  by routine. **Checked 2026-10-23 for 3dAutomask and 3dUnifize: both need the two
  implementations.** Their own files carry no MCW header (`3dAutomask.c`, `3dUnifize.c`,
  `thd_automask.c`, `thd_cliplevel.c`, `mri_invert.c`, `thd_dset_nbhd.c`, `thd_median.c`,
  `thd_dsetto3D.c`, `mri_get_cmass.c`: NIH, public domain), but their results depend on MCW
  routines:

  The routine-by-routine table (each MCW routine, its file and what it decides) is source-derived
  and kept in a restricted findings file, [findings/afni-licence-routines.md](findings/afni-licence-routines.md):
  **clean-room implementers must not read it** (CLAUDE.md rule 1).

  3dUnifize's squash also calls the C library's `tanhf`, and `-EPI` its `cbrtf`. Since 2026-10-29 the
  replica (in the main package) carries fdlibm's `tanhf` (Sun's notice), checked against glibc on all
  2^32 float inputs, and computes `-EPI`'s cube root correctly rounded (CORE-MATH, MIT): glibc's own
  `cbrtf` is LGPL-2.1 code, and its last bit changes the ball of neighbours on some voxel volumes, so
  the replica warns there (`docs/findings/afni-relicensing-26.md`, "The C library's maths"; the
  user decides how to keep the exact result). `specs/3dAutomask.md` and `specs/3dUnifize.md` are the
  clean-room originals' input. The routine-by-routine tables, with file:line, are in the restricted
  findings `docs/findings/afni-automask.md` and `afni-unifize.md`.
- **AFNI 26.3.05 relicenses MCW's code** (checked 2026-10-29,
  [findings/afni-relicensing-26.md](findings/afni-relicensing-26.md)): \"As of May 12, 2026, MCW
  has granted NIH a Creative Commons Attribution License (CC BY 4.0) for all of the code under
  its copyright\" (`doc/README/README.copyright`); every MCW file's header now says CC BY 4.0, and
  none of the 55 AFNI files the replicas derived from then is GPL; 3dvolreg's replica (2026-11-06) added 22, none GPL either: 13 of MCW's (CC BY 4.0 in their headers), 6 of the NIH's (no header: public domain) and EISPACK's `eis_svd.c` and `eis_pythag.c` with libf2c's `d_sign.c` (above). **The AFNI replicas moved into the
  main package on 2026-10-29** (decided by the user): `crates/larmorx-afni-replica`, Apache-2.0,
  with CC BY 4.0's attribution (section 3(a): the creator, the copyright notice, the licence and
  its URI, the disclaimer, the source and its tag, the modification) in every MCW-derived file's
  header, in the crate's `PROVENANCE.md` and in `NOTICE`, the legal code in `LICENSES/CC-BY-4.0.txt`
  (CC BY 4.0 lets an adaptation carry the adapter's licence, section 3(a)(4)). The C library's
  functions are permissive: `sinf`/`cosf` translated from Arm's Optimized Routines (MIT,
  `LICENSES/MIT-arm-optimized-routines.txt`), `tanhf`/`expm1f` fdlibm's; glibc's LGPL `cbrtf` is
  not included (above). larmorx's own code written for `larmorx-gpl` was relicensed to Apache-2.0
  by its author. `scripts/check_spdx.py` allows CC BY 4.0 only in that crate.

## Publishing

**Published in this repository:**
- our own code, specs and docs, and the clean-room originals;
- replicas of permissive upstreams, with their notices (`NOTICE`, `PROVENANCE.md`);
- the replica directories (`crates-gpl/`, `crates-nc/`, `crates-freesurfer/`), each under
  its upstream's terms:
  - GPL: the GPL text, the upstream copyright notices, changes marked (tedana's `t2smap`,
    since 2026-11-08: its LGPL-2.1 text and notice beside the GPL's). Once wheels ship,
    the GPL requires the source to be public anyway.
  - FSL: free of charge only, the FSL Licence passed on, FSL's original source included.
  - FreeSurfer: the licence with its required preface, notices kept, marked as modified, no
    MGH or FreeSurfer names used for promotion.
- the scripts that build the oracles (they fetch and compile upstreams; they contain none
  of their code).

**Never published:**
- the upstream clones, `tags/` and `reference_src/` (outside the repository), except the
  FSL source that `crates-nc/` must include;
- oracle builds and oracle outputs, and test images (they stay in `larmorx-testdata`);
- anything translated from MSM_HOCR;
- licence keys and personal data;
- long verbatim excerpts of restricted code in our docs (descriptions and file:line
  references are fine).

## Choosing at run time

**The licences stay separate through process boundaries** (decided 2026-10-11). The Apache
package `larmorx` never imports, loads or links the code of a replica under another licence
family (`larmorx-nc`), in Rust or in Python. **A replica under a licence the main package
allows is built in** (the AFNI replicas since 2026-10-29): it runs in the same process, with the
arguments, directory and environment a separate program would get, and gives the same files,
messages and exit status. For the others, larmorx runs the replica package's program as a separate process, with the tool's original arguments, and
the two exchange NIfTI files. Loading a GPL extension module into the Python process would make
one combined work of the two, under the GPL, so it is not done. larmorx knows the replica's
program name and arguments, nothing more.

The clean-room originals exist since 2026-10-23 (`crates/larmorx-afni`, Apache-2.0: `automask`,
`unifize`; provenance in `crates/larmorx-afni/PROVENANCE.md`); the main package uses
CORE-MATH's correctly rounded `tanhf` and `cbrtf` (MIT, `larmorx_core::math`) where AFNI calls the C
library's.
- **Python:** `lx.<family>.<tool>(..., implementation="auto")` (built for `lx.afni.tshift`).
  - `"auto"` (the default) runs the replica when it is built in (AFNI's) or its program is
    found (FSL's), and the original otherwise; `"replica"` and `"original"` force one.
  - `"replica"` without a separate replica's program raises `lx.ReplicaNotFoundError`, which names the
    install command.
  - A replica that is found but fails raises `lx.ReplicaError`. larmorx never falls back to
    the original then: the two differ in the last bits, so a silent fallback would change
    results without notice.
  - The wrapper writes in-memory inputs to a temporary directory, runs the replica there (a
    built-in one in this process) and reads its output back. Thread counts reach it as `OMP_NUM_THREADS`.
  - The result records what ran: `Image.implementation` (`lx.Implementation`: `"original"` or
    `"replica"`, the package, its version, the program's path).
  - There is no global setting; each call looks for the program again.
- **Command line:** `larmorx <family> <tool>` runs the replica's program, with the same
  arguments, when it is found, and the original otherwise.
  - `LARMORX_IMPLEMENTATION=auto|replica|original` overrides (default `auto`), so no option
    clashes with the tool's own arguments.
  - The replica's output and exit code pass through unchanged. larmorx's own exit codes:
    127 when `replica` is asked for and the program is missing, 126 when it cannot be
    started, 2 for an invalid `LARMORX_IMPLEMENTATION` or `LARMORX_NC_BIN`.
  - With `-verbose`, each implementation's first line names it, with its package and version.
- **Finding the program** (the same rules for both, `larmorx_cli::replica`):
  1. the package's environment variable, `LARMORX_NC_BIN` (an error if it names no file);
  2. the current Python environment's scripts directory (`sysconfig`), where `pip` puts the
     program, so a venv works without being activated; for the standalone `larmorx` binary,
     the directory of that binary;
  3. `PATH`.
- **Installing the replicas is explicit:** the AFNI replicas come with `pip install larmorx`
  (since 2026-10-29); `larmorx-nc` (non-commercial) comes only by name or with `pip install
  "larmorx[all]"`, which installs everything (the user's decision of 2026-10-16), so its licence
  is accepted knowingly either way.
- **larmorprepx** (Apache-2.0) uses `implementation="auto"`, so it gives the most accurate
  results whenever the replicas are installed. Its reports say which tools ran which
  implementation.

## Validation

Both tracks are validated against the same oracle runs:
- replicas: bit-identical as the target, with any difference explained in `docs/findings/`;
- originals: within the PLAN.md §11.3 thresholds.

Each tool's validation record reports both.

## Process rules

- Replica authors may read everything.
- Clean-room implementers of originals must not open:
  - the upstream source;
  - the replica crates (they are derived from it), and `crates-gpl/` (its tools keep or read
    glibc's LGPL code; `tools/glibc-dbl-model` is the spec writer's model of `specs/glibc-dbl.md`);
  - `docs/findings/` notes written from that source.

  They work from `specs/<tool>.md`, documentation, papers and black-box runs of the
  upstream binary.
- **The proof:** the clean-room audit ([validation/clean-room-audit.md](validation/clean-room-audit.md),
  since 2026-11-06) reads every session transcript, made at the time, and shows per clean-room
  original which agents wrote it, in which role, what each had seen before and while it wrote,
  and every slip with its nature; it runs at every merge and stops one that would let an agent
  edit a clean-room original after seeing restricted material of that tool's family. The
  transcripts themselves stay private, archived with their SHA-256.
- **Never** move replica code into the main package, even renamed.

## Options considered

1. **Everything under one licence (GPL-3).** Simplest, but larmorx would stop being
   Apache-2.0, and FSL replicas still could not be included (the FSL Licence is
   incompatible with the GPL). Rejected.
2. **Originals only (strict clean-room).** Permissive everywhere, but results differ from
   the upstream in the last bits wherever its own numerics (FFTs, solvers) decide them.
   Rejected as the only track, because accuracy comes first. Kept as the Apache track.
3. **Replicas only, under each upstream's licence.** Most accurate, but every user of
   AFNI-like or Workbench-like functions would get GPL code, and no permissive version would
   exist. Rejected as the only track.
4. **Replicas kept private** (local validation only). Not useful to users. Rejected.
5. **One wheel per upstream** (`larmorx-afni-gpl`, `larmorx-workbench-gpl`, ...). Possible,
   but AFNI's MCW files and Workbench share a licence. One wheel per licence family is
   simpler. Rejected for now.
6. **Both tracks, one package per licence family** (this page). Chosen.

How `larmorx` reaches a replica (2026-10-11):

1. **A Python extension module in the replica package** (`larmorx_gpl._core`), imported by
   `lx.afni.tshift`. Fastest (no files, no process), but importing GPL code into the process
   makes one combined work, so `larmorx` and every program built on it would fall under the
   GPL whenever the replica is installed. Rejected.
2. **The replica's program, run as a separate process, files in a temporary directory**
   (this page). Costs a process start and a NIfTI write and read per call: on two real BOLD
   runs, `lx.afni.tshift` with the replica took at most 15 ms longer than the program run by
   hand (`docs/api/afni-tshift.md`, "Choosing the implementation"). Chosen, for the command
   line and Python alike.
