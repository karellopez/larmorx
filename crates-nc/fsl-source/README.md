# FSL's original source of the replicated parts

The FSL Licence (`../LICENSE`) permits passing FSL's software on, modified or not, only
without financial return, with the licence imposed on the receiver, and with "all original
and amended source code included in any transmitted product". larmorx-nc meets the last
condition with two parts, both in every copy of this directory and in the `larmorx-nc` wheel:

- **the amended source**: the Rust translations in `../larmorx-nc-*/src/` (each file names the
  FSL files it was translated from in its header; each crate's `PROVENANCE.md` lists them and
  the changes made);
- **the original source**: the FSL files those translations come from, here, copied unchanged
  from FSL 6.0.7.17's source tree (`$FSLDIR/src/<package>/`), one directory per FSL package.

`MANIFEST.tsv` lists every file with its package, version and SHA-256.
`python scripts/fsl_source.py --check` verifies the copies; `--fsldir <FSL 6.0.7.17>` makes
them again.

| Package | Version | Upstream | Files |
|---|---|---|---|
| fsl-mcflirt | 2111.0 | https://git.fmrib.ox.ac.uk/fsl/mcflirt | `mcflirt.cc`, `Globaloptions.{h,cc}`, `Log.{h,cc}` |
| fsl-newimage | 2203.12 | https://git.fmrib.ox.ac.uk/fsl/newimage | `newimage.{h,cc}`, `newimagefns.{h,cc}`, `newimageio.h`, `generalio.cc`, `costfns.{h,cc}` |
| fsl-miscmaths | 2203.2 | https://git.fmrib.ox.ac.uk/fsl/miscmaths | `miscmaths.{h,cc}`, `optimise.{h,cc}`, `kernel.{h,cc}`, `splinterpolator.h` |
| fsl-newnifti | 4.1.0 | https://git.fmrib.ox.ac.uk/fsl/NewNifti | `NewNifti.{h,cc}`, `legacyFunctions.{h,cc}`, `nifti1.h`, `nifti2.h` |
| fsl-znzlib | 2111.0 | https://git.fmrib.ox.ac.uk/fsl/znzlib | `znzlib.{h,c}` |
| fsl-bet2 | 2111.8 | https://git.fmrib.ox.ac.uk/fsl/bet2 | `bet` (the script), `bet2.cpp`, `betsurf.{h,cpp}` (`robustfov`, the Tcl GUI and `bet4animal` are not translated) |
| fsl-meshclass | 2111.0 | https://git.fmrib.ox.ac.uk/fsl/meshclass | `meshclass.h`, `point.{h,cpp}`, `mpoint.{h,cpp}`, `triangle.{h,cpp}`, `mesh.{h,cpp}`, `pt_special.{h,cpp}`, `selfintersection.cpp`, `profile.{h,cpp}` |
| fsl-utils | 2203.5 | https://git.fmrib.ox.ac.uk/fsl/utils | `options.{h,cc}`, `parse.cc`, `usage.cc`, `functions.cc`, `matches.cc` |
| fsl-avwutils | 2209.3 | https://git.fmrib.ox.ac.uk/fsl/avwutils | `fslstats.cc`, `fslroi.cc`, `fslmerge.cc`, `fslmaths.cc`, `fslval` (what the `bet` script runs of them) |

FSL is Copyright (C) 1999-2023 University of Oxford; the files carry their own notices
(`nifti1.h`, `znzlib.{h,c}` and the code `legacyFunctions.cc` takes from `nifti1_io.c` were
released to the public domain by their authors). Armadillo 10.5.1 and fsl-armawrap 0.6.0, whose matrix rules
`larmorx-nc-core` translates in `src/arma.rs`, are Apache-2.0, not FSL code; their notices are
in `../NOTICE`, as are those of fslpy 3.21.1 (Apache-2.0: the file-name rules of `imtest`,
`remove_ext` and `imglob`, which the `bet` script runs; `src/avwutils.rs`) and of the C library
routines the core carries (fdlibm, glibc 2.35, Arm's Optimized Routines).

These files are not compiled: they are here so that every receiver has the source the
translation was made from.
