# Provenance: larmorx-fsl

**Clean-room implementation. No FSL code was used.**

FSL is distributed under the FSL Licence (non-commercial), so larmorx re-implements its tools
(CLAUDE.md rule 1, `docs/licensing.md`). `bet` (BET's `bet2` and the `bet` script; betsurf is
still to be written) was written on 2026-10-30 by an implementer who never opened FSL's source
code (`~/fsl*/src`, `crates-nc/fsl-source/` or any other copy), the replica crates (`crates-nc/`,
`crates/larmorx-afni-replica`, `crates-gpl/`), the findings written from the source
(`docs/findings/fsl-bet.md`, `fsl-mcflirt.md`, the AFNI source-derived ones, the sections of
`docs/findings/upstream-defects.md` marked restricted), `docs/parts/`, the replica parts'
investigations, `validation/difftest/triage.toml`, the replica authors' permanent cases, or
`validation/src/larmorx_validation/parity/fsl_versions.py`. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/bet.md`](../../specs/bet.md) (FSL 6.0.7.23) | the behaviour: names and formats, the robust range, the initial parameters, the surface and its deformation, self-intersection and reruns, the mask, every output, `--binaryMask`, `--ct`, the messages, the `bet` script's variations, the defects and their D13 switches |
| S. M. Smith, "Fast robust automated brain extraction", Human Brain Mapping 17(3):143-155, 2002 | the method |
| Black-box runs of FSL 6.0.7.23's `bet2` and `bet` (`~/fsl-6.0.7.23/bin`, FSL off the `PATH`) on the parity suite's phantoms and the test data: [`validation/investigations/cleanroom-bet/`](../../validation/investigations/cleanroom-bet/README.md) | the observations of [`docs/findings/fsl-bet-original.md`](../../docs/findings/fsl-bet-original.md): the mesh's vertex and triangle numbering and its file order, the rasterisation, the median's rank, the option parser's values, the messages of a missing input and of the script, ANALYZE inputs, single slices, the quaternion, FSL-56's layout, FSL-62's brain, `-F`'s and `-Z`'s files |
| The defect register's summary rows of FSL-40 to FSL-63 (`larmorx_core::defects`, which describe them by behaviour) | the defects' names in the notices |
| larmorx's own Apache-2.0 code: `larmorx_mri::hmc::{files, image}` (FSL 6.0.7.23's image names, compressions and reading, clean-room from `specs/mcflirt.md`), `larmorx_io` (NIfTI headers and writing), `larmorx_core` (correctly rounded `pow`, thread pools, D13), the `%g` formatting of `larmorx_afni::cfmt` (rewritten here) | images, numbers, threads |
| The NIfTI-1 standard's published conversions between a quaternion and its matrix (`quatern_to_mat44`, `mat44_to_quatern`, with the polar decomposition; public domain, as larmorx-io's port in `nifti/itk/write.rs` follows them) | the qform's quaternion FSL writes |
| **Part 2a (2026-10-30, the same implementer):** the spec writer's answers in `specs/bet.md` (lines marked "(spec writer, 2026-10-30)": §3.1, §4.2, §4.4, §4.5, §4.7, §5.2, §8, §8.1, §8.2) and the spec writer's black-box README (`validation/investigations/bet-spec-answers/README.md`) | the deformation's arithmetic operation by operation, the neighbour order, the exact `-e` test, the skull search, the quaternion's precision path, the name fallback, `-F`'s doubles |
| `larmorx_core::math::tanh_glibc` (fdlibm 5.3 from netlib, Sun's notice, black-box-proved equal to glibc 2.35's `tanh`) | the smoothness weight's `tanh` with FSL's bits (§8.1), in `reproduce` and, until part 2b, in the default; `larmorx_core::math::tanh` (CORE-MATH, correctly rounded) is ported for the default |
| `larmorx_afni::x87`'s method (larmorx's own clean-room code: x87 extended precision with integer arithmetic), adapted in `bet::x87` with the rounding to float | §8.2's extended-precision steps |
| Black-box runs of FSL 6.0.7.23's `fslstats -C` on FSL's own `bet -R` output (part 2a, case 622) | the centre printed as `-0.000000` (findings) |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `bet::mesh` (the icosahedron, its subdivision and numbering in the file's order, adjacency by §4.4's move-to-the-end rule, normals and means by division) | BET's tessellated sphere | spec §4.4, §8.1, black-box (`topo2.py`) |
| `bet::params` (robust range, threshold, radius, centre of gravity, median) | §4.2, §4.3, §8.1 | spec, black-box (`py/params.py` of the scratch notes: the median's rank) |
| `bet::surface` (the deformation, the self-intersection measure with a grid of cells, the exact test's sweep and predicate, reruns) | §4.4, §4.5, §8.1 | spec |
| `bet::raster` (drawing, the flood fill, FSL-43's fill by the linear index) | §4.6 | spec, black-box (`raster_probe*.py`) |
| `bet::skull` | §4.7's skull search, §8.1's trilinear interpolation | spec |
| `bet::image` (name resolution, outputs and their headers, ANALYZE inputs, FSL-56, `-F`'s doubles) | §3.1-§3.3, §5.2 | spec, black-box (`header_diff.py`) |
| `bet::quatern`, `bet::x87` | §3.3, §8.2 | the NIfTI standard's conversions in §8.2's precisions, black-box (`quatern_probe.py`) |
| `cli::options` | FSL's option parser | spec §4.1, black-box |
| `cli::bet2` | bet2's command line, messages, outputs, FSL-40, -42, -43, -56, -60, -61, -62 | spec §4, §7, §9, black-box |
| `cli::bet` | the `bet` script: default, `-R` (FSL-63), `-F`, `-Z` (FSL-45), the pending variations, FSL-44, FSL-46 | spec §5, black-box |

The parity record is [docs/validation/fsl-bet.md](../../docs/validation/fsl-bet.md); the
benchmark [docs/benchmarks/fsl-bet.md](../../docs/benchmarks/fsl-bet.md).
