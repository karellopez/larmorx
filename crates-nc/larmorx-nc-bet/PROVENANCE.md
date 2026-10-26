# Provenance: larmorx-nc-bet

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.17's brain extraction programs `bet` (the shell script), `bet2` and `betsurf`, written to
give FSL's output bit for bit. It is distributed under the FSL Licence's terms (`LicenseRef-FSL`:
non-commercial use only, `../LICENSE`), as part of the `larmorx-nc` package (`docs/licensing.md`).
The parts of FSL's libraries and of the C and C++ libraries it relies on (newimage, NewNifti,
znzlib, meshclass, utils' option parser, what `bet` runs of avwutils and fslpy, Armadillo's
matrix rules, glibc's `pow`, `exp` and `tanh`, x87 `long double`, libstdc++'s formatted input,
`std::sort` and `std::nth_element`) are the shared core's, `../larmorx-nc-core` (its
`PROVENANCE.md`). The Apache-2.0 clean-room original of BET is still to be written, from
`specs/bet.md`; it will use none of this crate's code.

The FSL package is fsl-bet2 2111.8 of the FSL 6.0.7.17 conda installation (`$FSLDIR/src/fsl-bet2/`;
tag on `git.fmrib.ox.ac.uk`, pinned in `upstream.tsv`); its original source is kept, unmodified,
in `../fsl-source/fsl-bet2/` (the FSL Licence requires it with any transmitted product).

## Licences of the translated sources

- **FSL** (fsl-bet2 2111.8): "FMRIB Software Library, Release 6.0 (c) 2018, The University of
  Oxford", the FSL Licence (non-commercial). BET: "BETv1 Steve Smith; BETv2 Mickael Pechaud, Mark
  Jenkinson, Steve Smith; FMRIB Image Analysis Group; Copyright (C) 1999-2003 University of Oxford".
- **The CT conversion** (`bet2 --ct`, `bet2.cpp`'s `h2c`): FSL's code says it is "adapted from:
  https://github.com/neurolabusc/Clinical/blob/master/clinical_h2c.m which is licensed under the
  BSD-2-clause license" (Chris Rorden); translated with the rest of `bet2.cpp`.

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates and where it departs from the original (the D13 corrections, below).

## FSL files and functions translated

| Rust module | FSL file | Functions, classes |
|---|---|---|
| `bet2` | fsl-bet2 `bet2.cpp` | `main`, `bet_parameters`, `adjust_initial_mesh`, `step_of_computation`, `draw_segment`, `draw_mesh`, `draw_mesh_bis`, `make_mask_from_mesh`, `find_skull`, the head-CT conversion (`--ct`), the options, messages and exit statuses |
| `betsurf` | fsl-bet2 `betsurf.cpp`, `betsurf.h` | `main`, `t1only_main`, `standard_step_of_computation`, `special_case`, `co_ext`, `special_extract`, `t1only_co_ext`, `t1only_special_extract`, `write_ext_skull`, `t1only_write_ext_skull`, `draw_segment`, `draw_mesh`, `make_mask_from_mesh`, `trMatrix` |
| `bet` | fsl-bet2 `bet` (bash) | the script: its arguments, `Usage`, the option loop, variations 0 (bet2), 1 (`-R`), 3 (`-Z`), 4 (`-F`); `-S`, `-B`, `-A`, `-A2` refused as pending; the `ERR` trap, the cleanup; `dc`'s arithmetic as the script uses it |
| `cli` | — | the entry points `larmorx-nc fsl bet`, `bet2`, `betsurf` (`larmorx-nc-cli`) |

Until BET's merge (2026-10-25) this crate carried its own translations of the libraries; they
were joined with the mcflirt replica's into `larmorx-nc-core`, one translation of each, and every
case of BET's record and golden test gives the same bytes with the joined core.

## Departures from the originals (D13)

FSL's deterministic defects are corrected by default and reproduced with
`LARMORX_UPSTREAM_DEFECTS=reproduce` (`docs/api/upstream-defects.md`): FSL-40 (bet2's `.hdr`
stripping), FSL-42 (meaningless numeric options), FSL-43 (a mask's fill starting outside the
image), FSL-44 (bet's unquoted expansions), FSL-45 (`-Z`'s crop needs `imglob` on the PATH),
FSL-46 (bet's cleanup glob), FSL-47 (`--t1only`'s scalp mask filled from the wrong surface),
FSL-48 (a T2 on another grid), FSL-50 (one bad profile ends betsurf silently), FSL-51 (profile
searches reading past the samples), FSL-53 (short matrix files), FSL-54 (`--t1only`'s exit
status), FSL-55 (NaN in the 98th percentile), FSL-56 (NewNifti's `vox_offset` of several
extensions). Where FSL crashes on what it leaves undefined, the default refuses with the reason
and `reproduce` reproduces what FSL defines and fills the rest deterministically, naming the
defect (D13's failure cases): FSL-41 (the median of an empty list: 0), FSL-52 (mesh values FSL
never sets: 0; faces naming missing vertices: left out), FSL-43's crash class (a fill FSL writes
far outside the volume for: not done). An uncaught exception of the image layer is FSL's message
and a refusal in both modes. Two of FSL's choices are kept and documented (rule C): FSL-49 and
FSL-57. Details: `docs/findings/fsl-bet.md` (restricted).

The replica does not compute what FSL computes and never uses: betsurf's CSF image (never written)
is left out.

**NaN bits.** Where NaN or infinite voxels reach BET's own arithmetic (bet2's initial threshold,
its centre of gravity, the deformation steps of bet2 and betsurf), it computes with SSE's NaN
rules (`larmorx_nc_core::sse`), in the source's order, so the outputs have FSL's x86-64 bits on
every platform (`validation/probes/fsl-bet-nan`).
