# Provenance: larmorx-nc-bet

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.17's brain extraction programs `bet` (the shell script), `bet2` and `betsurf`, with the parts
of FSL's libraries and of the C and C++ libraries they rely on, written to give FSL's output bit
for bit. It is distributed under the FSL Licence's terms (`LicenseRef-FSL`: non-commercial use
only, `../fsl-source/LICENCE.FSL`), as part of the `larmorx-nc` package (`docs/licensing.md`). The
Apache-2.0 clean-room original of BET is still to be written, from `specs/bet.md`; it will use
none of this crate's code.

The FSL packages are those of the FSL 6.0.7.17 conda installation (`$FSLDIR/src/<package>/`; tags
on `git.fmrib.ox.ac.uk`, pinned in `upstream.tsv`); their original source is kept, unmodified, in
`../fsl-source/` (the FSL Licence requires it with any transmitted product).

## Licences of the translated sources

- **FSL** (fsl-bet2 2111.8, fsl-meshclass 2111.0, fsl-newimage 2203.12, fsl-newnifti 4.1.0,
  fsl-miscmaths 2203.2, fsl-utils 2203.5, fsl-znzlib 2111.0, fsl-avwutils 2209.3): "FMRIB Software
  Library, Release 6.0 (c) 2018, The University of Oxford", the FSL Licence (non-commercial). BET:
  "BETv1 Steve Smith; BETv2 Mickael Pechaud, Mark Jenkinson, Steve Smith; FMRIB Image Analysis
  Group; Copyright (C) 1999-2003 University of Oxford". NewNifti and znzlib derive from the NIfTI
  reference library (public domain).
- **fslpy 3.21.1** (`imtest`, `remove_ext`, `imglob`, which the `bet` script runs): Apache-2.0,
  "Copyright 2016-2024 University of Oxford, Oxford, UK".
- **armawrap 0.6.0 / Armadillo** (the 4 x 4 matrix arithmetic NEWMAT's interface gives newimage):
  Apache-2.0 (Armadillo: "Copyright 2008-2016 Conrad Sanderson, 2008-2016 National ICT Australia
  (NICTA)").
- **The GNU C Library 2.35** (`fsl/glibc_pow.rs`: `pow`, `exp`): "Copyright (C) 2018-2022 Free
  Software Foundation, Inc.", LGPL-2.1-or-later; the algorithms and tables are Szabolcs Nagy's (Arm),
  which Arm's optimized-routines also publish under the MIT licence. Source: the release tarball in
  `upstream.tsv`. The fused multiply-adds follow the machine code of Ubuntu 22.04's `libm.so.6`
  (glibc 2.35-0ubuntu3), `__pow_fma` and `__exp_fma`.
- **fdlibm** as glibc 2.35 ships it (`fsl/fdlibm.rs`: `tanh`, `expm1`): "Copyright (C) 1993 by Sun
  Microsystems, Inc. All rights reserved. Developed at SunPro, a Sun Microsystems, Inc. business.
  Permission to use, copy, modify, and distribute this software is freely granted, provided that
  this notice is preserved."
- **The CT conversion** (`bet2 --ct`, `bet2.cpp`'s `h2c`): FSL's code says it is "adapted from:
  https://github.com/neurolabusc/Clinical/blob/master/clinical_h2c.m which is licensed under the
  BSD-2-clause license" (Chris Rorden); translated with the rest of `bet2.cpp`.
- **GCC's libstdc++** behaviour (`fsl/stdsort.rs`: `std::sort`, `std::nth_element`;
  `fsl/istream.rs`: formatted input): written from the algorithms' published descriptions (the SGI
  STL's introsort and introselect, the C++ standard's `num_get`), not copied from libstdc++.

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates and where it departs from the original (the D13 corrections, below).

## FSL files and functions translated

| Rust module | FSL file | Functions, classes |
|---|---|---|
| `bet2` | fsl-bet2 `bet2.cpp` | `main`, `bet_parameters`, `adjust_initial_mesh`, `step_of_computation`, `draw_segment`, `draw_mesh`, `draw_mesh_bis`, `make_mask_from_mesh`, `find_skull`, the head-CT conversion (`--ct`), the options, messages and exit statuses |
| `betsurf` | fsl-bet2 `betsurf.cpp`, `betsurf.h` | `main`, `t1only_main`, `standard_step_of_computation`, `special_case`, `co_ext`, `special_extract`, `t1only_co_ext`, `t1only_special_extract`, `write_ext_skull`, `t1only_write_ext_skull`, `draw_segment`, `draw_mesh`, `make_mask_from_mesh`, `trMatrix` |
| `bet` | fsl-bet2 `bet` (bash) | the script: its arguments, `Usage`, the option loop, variations 0 (bet2), 1 (`-R`), 3 (`-Z`), 4 (`-F`); `-S`, `-B`, `-A`, `-A2` refused as pending; the `ERR` trap, the cleanup; `dc`'s arithmetic as the script uses it |
| `avw` | fsl-avwutils `fslstats.cc` (`-C`, `-l`), `fslroi.cc`, `fslmerge.cc` (`-z`), `fslmaths.cc` (`-dilF`, `-mas`), `fslval`; fslpy `fsl/utils/path.py`, `fsl/scripts/imtest.py`, `remove_ext.py`, `imglob.py` | what `bet` runs of them |
| `mesh` | fsl-meshclass `point.cpp/h`, `mpoint.cpp/h`, `triangle.cpp/h`, `mesh.cpp/h`, `pt_special.cpp/h`, `selfintersection.cpp` | `Pt`, `Vec` and their operators, `Mpoint`, `Triangle` (constructor's neighbour bookkeeping, destructor, `normal`, `centroid`, `intersect`), `Mesh` (copy constructor, `operator=`, `update`, `translation`, `rescale`, `retessellate`, `self_intersection`, `real_self_intersection`, `stream_mesh` type 3, `save`), `make_mesh_from_icosa` |
| `mesh::load` | fsl-meshclass `mesh.cpp` | `Mesh::load`, `load_off`, `load_vtk_ASCII`, `load_fs` |
| `mesh::profile` | fsl-meshclass `profile.cpp/h` | `Profile`, `pro_pair` |
| `fsl::volume` | fsl-newimage `newimage.h/cc`, `newimageio.h`, `generalio.cc`, `newimagefns.h/cc` | `volume<T>` (properties, `swapLR`, `makeradiological`, `left_right_order`, `sampling_mat`, `niftivox2newimagevox_mat`, `interpolate` and `q_tri_interpolation` trilinear with zero padding, `extrapolate`, `copyconvert`, `copyproperties`), `read_volume`, `read_volume4D`, `save_volume`, `set_fsl_hdr`, `ConvertAndScale`, `find_thresholds` (`robustmin/max`), `smooth`, `gaussian_kernel1D`, `convolve`, `convolve_separable`, `FslGetEnvOutputType`, `outputExtension` |
| `fsl::filename` | fsl-newimage `newimageio.h`, `generalio.cc` | `make_basename`, `return_validimagefilename`, the file name checks |
| `fsl::nifti` | fsl-newnifti `NewNifti.cc/h`, `legacyFunctions.cc/h`; fsl-znzlib `znzlib.c` | `NiftiIO` (header read and write, NIfTI-1, NIfTI-2, ANALYZE, pairs, extensions), `NiftiHeader`, the legacy quaternion routines (`mat44_to_quatern`, `quatern_to_mat44`, `mat33_polar`, `mat33_inverse`, `mat33_determ`) in x87 `long double` as compiled, gzip I/O |
| `fsl::arma` | armawrap / Armadillo `op_det`, `op_inv`, `gemm_emul_tinysq`, `solve_square_tiny` | the 4 x 4 determinant, inverse, products and solves |
| `fsl::options` | fsl-utils `options.h`, `parse.cc`, `usage.cc`, `functions.cc`, `matches.cc` | `OptionParser`, `Option<T>`, `parse_command_line`, `parse_option`, `parse_long_option`, `usage`, `string_to_T` |
| `fsl::cstr` | (glibc behaviour) | `strtod`, `strtof`, `strtol`, `atof`, `atoi`, `printf`'s `%g`, `%f`, `%e` |
| `fsl::x87` | (x87 behaviour) | `long double` sums, products and quotients rounded to 64 bits, then stored |
| `fsl::glibc_pow` | glibc 2.35 `sysdeps/ieee754/dbl-64/e_pow.c`, `e_pow_log_data.c`, `e_exp.c`, `e_exp_data.c` | `__pow`, `log_inline`, `exp_inline`, `specialcase`, `checkint`; `__exp`, its `specialcase` (as `__pow_fma`, `__exp_fma`) |
| `fsl::fdlibm` | glibc 2.35 `sysdeps/ieee754/dbl-64/s_tanh.c`, `s_expm1.c` | `__tanh`, `__expm1` |
| `fsl::stdsort` | (libstdc++ behaviour) | `std::sort`, `std::nth_element` on doubles |
| `fsl::istream` | (libstdc++ behaviour) | `operator>>` for `int`, `float`, `double`, `std::string`; `std::getline` |

`fsl/` and `mesh/` hold what FSL's other programs share with BET: they are meant to move into the
package's shared core crate (`larmorx-nc-core`) when the FSL replicas are joined.

## Departures from the originals (D13)

FSL's deterministic defects are corrected by default and reproduced with
`LARMORX_UPSTREAM_DEFECTS=reproduce` (`docs/api/upstream-defects.md`): FSL-40 (bet2's `.hdr`
stripping), FSL-42 (meaningless numeric options), FSL-43 (a mask's fill starting outside the
image), FSL-44 (bet's unquoted expansions), FSL-45
(`-Z`'s crop needs `imglob` on the PATH), FSL-46 (bet's cleanup glob), FSL-47 (`--t1only`'s scalp
mask filled from the wrong surface), FSL-48 (a T2 on another grid), FSL-50 (one bad profile ends
betsurf silently), FSL-51 (profile searches reading past the samples), FSL-53 (short matrix files),
FSL-54 (`--t1only`'s exit status), FSL-55 (NaN in the 98th percentile), FSL-56 (NewNifti's
`vox_offset` of several extensions). Where FSL crashes or reads memory it never set, the replica
prints what FSL printed before and refuses, in both modes (FSL-41: no voxel for the median
intensity; FSL-52: mesh files FSL cannot read; an uncaught exception of the image layer). Where
FSL writes outside the image but goes on (FSL-43), `reproduce` does what FSL defines and warns. Two of FSL's choices are kept and documented (rule C):
FSL-49 and FSL-57. Details: `docs/findings/fsl-bet.md` (restricted).

The replica does not compute what FSL computes and never uses: betsurf's CSF image (never written)
is left out.
