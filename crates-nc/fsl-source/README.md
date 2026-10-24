# FSL's original source of the larmorx-nc replicas

The FSL Licence (`LICENCE.FSL`, FSL 6.0.7.17) allows reproduction and modification without
financial return only when "the conditions of this Licence are imposed upon the receiver of the
product, and all original and amended source code is included in any transmitted product". The
replicas in `crates-nc/` are amended translations of FSL's source, so FSL's original source of
everything they translate is kept here, unmodified, as the FSL 6.0.7.17 conda packages install it
(`$FSLDIR/src/<package>/`), without the packages' build scripts:

| Directory | Package (version) | Translated by |
|---|---|---|
| `fsl-bet2/` | fsl-bet2 2111.8 (`bet`, `bet2.cpp`, `betsurf.cpp`, `betsurf.h`; `robustfov.cc`, the Tcl GUI and `bet4animal` are not translated) | `larmorx-nc-bet` |
| `fsl-meshclass/` | fsl-meshclass 2111.0 | `larmorx-nc-bet` (`src/mesh/`) |
| `fsl-newimage/` | fsl-newimage 2203.12 | `larmorx-nc-bet` (`src/fsl/volume.rs`, `filename.rs`) |
| `fsl-newnifti/` | fsl-newnifti 4.1.0 | `larmorx-nc-bet` (`src/fsl/nifti.rs`) |
| `fsl-miscmaths/` | fsl-miscmaths 2203.2 | `larmorx-nc-bet` (the parts BET uses) |
| `fsl-utils/` | fsl-utils 2203.5 | `larmorx-nc-bet` (`src/fsl/options.rs`) |
| `fsl-znzlib/` | fsl-znzlib 2111.0 | `larmorx-nc-bet` (`src/fsl/nifti.rs`, gzip I/O) |
| `fsl-avwutils/` | fsl-avwutils 2209.3: `fslstats.cc`, `fslroi.cc`, `fslmerge.cc`, `fslmaths.cc`, `fslval` | `larmorx-nc-bet` (`src/avw.rs`, what `bet` runs of them) |

Which upstream file each Rust file translates, and the other sources the replicas follow (fslpy
3.21.1 and armawrap 0.6.0, Apache-2.0; glibc 2.35's `pow` and `exp`, LGPL-2.1-or-later; fdlibm's
`tanh` and `expm1`), are listed in each crate's `PROVENANCE.md`.
