# Provenance: larmorx-fsl

**Clean-room implementation. No FSL code was used.**

FSL is distributed under the FSL Licence (non-commercial), so larmorx re-implements its tools
(CLAUDE.md rule 1, `docs/licensing.md`). `bet` (BET's `bet2` and the `bet` script) was written on
2026-10-30 and `betsurf` on 2026-10-31 by an implementer who never opened FSL's source
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
| **Part 2b (2026-10-31, the same implementer):** `specs/bet.md` §6 (betsurf) with §3, §4.4, §4.6, §8.1; M. Jenkinson, M. Pechaud, S. Smith, "BET2: MR-based estimation of brain, skull and scalp surfaces", OHBM 2005 (the method) | betsurf: the command line, the mesh and matrix files, the robust maximum, the profiles and their searches, the counting images, the smoothing, the three deformations, the masks and outputs, FSL-43, -47, -48, -50 to -55, -58 |
| Black-box runs of FSL 6.0.7.23's `betsurf` (`validation/investigations/cleanroom-bet/`: `betsurf_cases.py`, `betsurf_inputs.py`, `betsurf_triangles.py`, `betsurf_profile_oracle.py`: one triangle, or one profile, at a time) | the observations of the findings' "betsurf" section: the region that holds its end, the bright samples counted, the scan from `a`, the fallbacks, the T1-only condition, the removed profiles counted with the scalp-only ones, `-p`'s steps, the parser's integers, `manual_input` and `c`, the messages and the order of the outputs |
| The `--tanh` option (2026-10-31): the user's decision on GLIBC-06 | glibc's bits by default, `larmorx_core::math::tanh` on request |
| Black-box runs of FSL 6.0.7.23's `bet2` and `bet` on random testing's cases 1608 and 1053 (2026-11-01; `bet2 -c 0 0 0`, `-c 0 0 1` and no `-c` on the same image; `bet -F -d` on a float64 series) | `-c 0 0 0` as no centre given; `bet -F`'s `<out>_tmp` of a float64 series as float64 |

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
| `betsurf::meshfile` (VTK, OFF and FreeSurfer ASCII meshes read as C++'s streams read them) | §6.2, FSL-52 | spec |
| `betsurf::profile` (the profiles' regions, thresholds and searches; the four kinds of profile) | §6.4, FSL-49, -50, -51 | spec, black-box (`betsurf_profile_oracle.py`, `betsurf_triangles.py`) |
| `betsurf` (the matrix, the robust maximum and libstdc++'s selection for FSL-55, the working grid, the votes, the smoothing, the deformation, the masks, the scalp's sphere) | §6.2-§6.6, FSL-43, -53, -55, -58 | spec, black-box |
| `cli::betsurf` | betsurf's command line, messages, outputs and their order, FSL-47, -48, -52, -53, -54 | spec §6.1, §6.6, §7, §9, black-box |

The parity record is [docs/validation/fsl-bet.md](../../docs/validation/fsl-bet.md); the
benchmark [docs/benchmarks/fsl-bet.md](../../docs/benchmarks/fsl-bet.md).

## FLIRT (`flirt`, `convert_xfm`; part 1, 2026-11-02)

`flirt` (FLIRT's registration, its schedule language, the boundary-based cost, the resampled
image, `-applyxfm`) and `convert_xfm` were written by a clean-room implementer who never opened
FSL's source code (`~/fsl*/src`, `crates-nc/fsl-source/` or any other copy), the replica crates
(`crates-nc/`, `crates/larmorx-afni-replica`, `crates-gpl/`), the restricted findings
(`docs/findings/fsl-flirt.md`, `fsl-bet.md`, `fsl-mcflirt.md`, the AFNI source-derived ones, the
register's sections marked †), `docs/parts/`, the replica parts' investigations and probes,
`validation/difftest/triage.toml`, the replica authors' permanent cases, the git history of the
replica parts, or `validation/src/larmorx_validation/parity/fsl_versions.py`. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/flirt.md`](../../specs/flirt.md) (FSL 6.0.7.23) and the parts of [`specs/mcflirt.md`](../../specs/mcflirt.md) it refers to | the behaviour: the command line and its messages, coordinates and the base scale, the pyramid, the costs, the schedule language, the search, the optimisation, parameter subsets, the sform/qform initialisation, the outputs, the background value, `convert_xfm`, the defects and their D13 switches |
| M. Jenkinson and S. Smith, "A global optimisation method for robust affine registration of brain images", Medical Image Analysis 5(2):143-156, 2001; M. Jenkinson, P. Bannister, M. Brady and S. Smith, "Improved optimization for the robust and accurate linear registration and motion correction of brain images", NeuroImage 17(2):825-841, 2002; D. Greve and B. Fischl, "Accurate and robust brain image alignment using boundary-based registration", NeuroImage 48(1):63-72, 2009 | the methods |
| Black-box runs of FSL 6.0.7.23's `flirt` and `convert_xfm` (`~/fsl-6.0.7.23/bin`, FSL off the `PATH`), through the oracle cache: the `fsl-flirt` suite's cases and [`validation/investigations/cleanroom-flirt/`](../../validation/investigations/cleanroom-flirt/README.md) | the observations of [`docs/findings/fsl-flirt-original.md`](../../docs/findings/fsl-flirt-original.md); the default schedules' lines as `flirt -v` prints them (`flirt::schedule`), the usage texts, the messages of the command line and the schedule |
| The defect register's summary rows of FSL-09, -11, -14, -21, -22, -56, -66, -67, -68, -71, -72, -75 (behaviour only) | the corrections and refusals, the notices' names (`larmorx_core::defects`) |
| larmorx's own Apache-2.0 code, the **shared clean-room registration engine** of `larmorx_mri::hmc` (written from `specs/mcflirt.md`): its volumes and centres of mass, the parametrisation (`rigid`: composition and decomposition), the cost sampling and sums (`cost`: the row ranges, edge weights, normalised correlation with its count, least squares, the mutual-information costs, now with a weighted sampling and the fuzzy fraction), the line search (`search::line_search_from`, now with the first step's multiple), the reference grids, the interpolators and kernels; `bet::image` and `bet::params` (FSL's image names, reading, writing, the robust range), `betsurf::smooth` (the separable Gaussian), `larmorx_core::math` (correctly rounded `exp`, `sin`, `cos`; `tanh_glibc`) | the engine FLIRT runs on, with the head-motion correction: one copy |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `flirt::mat` | 4 x 4 algebra (LU inverses and solves), matrix files read and written, the stream's notation | spec §2.2, §2.5, §12 |
| `flirt::pyramid` | the blur, the isotropic resampling, the halving, the weighted filters, the background value (FSL-75, FSL-22) | spec §5, §10a |
| `flirt::cost`, `flirt::bbr` | the image pair, every cost and its weighted form, `labeldiff`, the boundary points, their normals (FSL-67 kept) and the boundary-based cost with a field map | spec §6, black-box |
| `flirt::optimise`, `flirt::search` | the sweeps, the convergence test, Powell's correction, parameter subsets and their pseudo-inverse, the search over rotations | spec §8.1 to §8.4 |
| `flirt::schedule`, `flirt::engine` | the schedule language, the default schedules, the scales, every command, the run and its outputs, FSL-68 | spec §3, §4, §7, §9, black-box |
| `flirt::output` | the resampled image: the output's blur by the extraslice rule, the interpolations (the windowed sinc of any width and window), the padding, the field map's resampling | spec §9.2, §9.3, black-box |
| `flirt::xfm`, `cli::convert_xfm` | `convert_xfm` | spec §10, black-box |
| `cli::flirt` | FLIRT's command line, its usage and messages | spec §2, black-box |

The parity record is [docs/validation/fsl-flirt.md](../../docs/validation/fsl-flirt.md).

## FLIRT's other programs (`avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`; part 2, 2026-11-02)

Written by a clean-room implementer under the same rules as part 1 (none of the files listed
above opened). Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/flirt.md`](../../specs/flirt.md) §15 to §15.4 (with §2.5, §3, §8.5, §8.6, §12) and [`specs/mcflirt.md`](../../specs/mcflirt.md) §4.1, §4.2, §4.4, §6.1, §10.4 | the programs' command lines, messages, exit statuses, mappings, decompositions and matrices, FSL-76, FSL-77 |
| Black-box runs of FSL 6.0.7.23's six programs (`~/fsl-6.0.7.23/bin`, FSL off the `PATH`): the `fsl-flirt` suite's `programs/` cases through the oracle cache, further command lines through it, and the coordinate programs reading standard input run directly (the cache does not key standard input) | the observations of [`docs/findings/fsl-flirt-original.md`](../../docs/findings/fsl-flirt-original.md), "FLIRT's other programs": `makerot`'s basis and precisions, `avscale`'s precisions, the coordinate programs' default space and standard input |
| The defect register's summary rows of FSL-14, FSL-76 and FSL-77 (behaviour only) | the corrections, refusals and notices |
| larmorx's own Apache-2.0 code: part 1's `flirt::mat` and `flirt::param` (the quaternion parametrisation), `larmorx_mri::hmc::rigid` (the decomposition and composition), `cli::options` (FSL's option parser, now also every occurrence of an option), `bet::image` and `cli::bet2` (FSL's image names and reading), `larmorx_core::math` (correctly rounded `sinf`, `cosf`, `asin`, `cos`) | numbers, images, parameters |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `flirt::programs` (the determinant, `avscale`'s decomposition, half transforms and inverted eddy matrix, the coordinate mappings, the RMS deviations, `makerot`'s rotation with x86-64's NaN rules written out) | §15.1 to §15.4 | spec, black-box |
| `cli::common` (matrix files, the image layer's and the C++ runtime's messages, images read for their header, the centre of gravity with FSL-14) | §15 | spec, black-box |
| `cli::avscale`, `cli::coords`, `cli::rmsdiff`, `cli::makerot` | the four command lines, their usages and messages, FSL-76, FSL-77 | spec §15.1 to §15.4, black-box |

## FLIRT's other programs and scripts (`midtrans`, `pointflirt`, `applyxfm4D`, `pairreg`, `extracttxt`, `aff2rigid`; part 2, relay 2, 2026-11-02)

Written by a clean-room implementer under the same rules (none of the files listed above
opened; FSL's scripts in `~/fsl-6.0.7.23/bin` not opened either, only run). Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/flirt.md`](../../specs/flirt.md) §15, §15.5 to §15.8 (with §8.3, §9.1, §9.2, §10a, §11, §13) | the programs' command lines, messages, exit statuses, fits, resampling, the scripts' steps, FSL-78 to FSL-81 |
| Black-box runs of FSL 6.0.7.23's programs and scripts (FSL off the `PATH`): the `fsl-flirt` suite's `programs/` cases through the oracle cache, further command lines through it (convert_xfm on odd matrix files, applyxfm4D on generated integer series), the system's GNU dc 1.4.1 run directly | the observations of [`docs/findings/fsl-flirt-original.md`](../../docs/findings/fsl-flirt-original.md), "FLIRT's other programs and scripts": the matrix-file reader, midtrans's stages and errors, pointflirt's transposition, applyxfm4D's resampling (no blur), sform zeros and error forms, extracttxt's dc arithmetic |
| The `larmorx-nc` replica's `aff2rigid` program, run as a black box | FSL-81's corrected matrix, to check the original's |
| POSIX's definition of basic regular expressions and GNU grep's manual (`\+`, `\?`, `\|`, `\w`, `\s`, `\b`, `\<`, `\>`) | `cli::bre` |
| larmorx's own Apache-2.0 code: part 1's `flirt::{mat, output, pyramid, engine}` (the resampling, the background value, the image writer), relay 1's `cli::{common, options}`, `larmorx_mri::hmc::kernels` (the spline, now with a per-pass store), `larmorx_core::vnl_svd` (the singular value decompositions) | numbers, images |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `flirt::fits` (midtrans's three stages, pointflirt's normal equations solved by their SVD, aff2rigid's rigid matrix, determinants of any size) | §15.5, §15.6, §15.8 | spec |
| `flirt::mat::parse_matrix_text` (FSL's matrix files: libstdc++'s extraction of doubles) | §2, §15 | black-box |
| `flirt::output::{padded, Resampling::spline}`, `flirt::pyramid::background_f64`, `flirt::engine::{write_values, grouped_type}` (shared with flirt) | §9.2, §10a, §15.7 | spec, black-box |
| `cli::midtrans`, `cli::pointflirt`, `cli::applyxfm4d` | the three command lines, usages, messages, FSL-78, FSL-79, FSL-80 | spec §15.5 to §15.7, black-box |
| `cli::pairreg`, `cli::extracttxt`, `cli::bre`, `cli::aff2rigid` | the three scripts, their messages, FSL-81 | spec §15.8, black-box, POSIX |

## FLIRT's verbose levels, `-debug` and speed (part 2, relay 3, 2026-11-02)

Written by a clean-room implementer under the same rules (none of the files listed above
opened). Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/flirt.md`](../../specs/flirt.md) §2.3, §2.5, §7.3, §9.4, §12 | the levels' outline, the stream's notation, `-debug`'s volumes and final call, which sums may run in parallel |
| Black-box runs of FSL 6.0.7.23's `flirt` (FSL off the `PATH`) through the oracle cache: the fsl-flirt suite's `verbose/` and `bbr/debug` cases and about 30 probe command lines (levels 2 to 7 on short schedules, `-noclamp`, `-noresample`, `-2D`, `-applyxfm`, `-out`, `-debug` over forced and input-only scales, the boundary-based cost set up by `setoption`, short tolerance lists), and part 1's level-7 trace | every line's text, level and place ([findings](../../docs/findings/fsl-flirt-original.md), "The verbose levels and -debug"), the debug volumes' headers, the short tolerance list's index error |
| larmorx's own Apache-2.0 code: part 1's engine, search, optimiser and costs | numbers |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `flirt::trace` (standard output in order, its notation, the evaluation counter, the images' four information lines) | §2.3, §2.5 | spec, black-box |
| `flirt::engine` (the reading's, preparation's, `setscale`'s, `setoption`'s and the end's lines; `-debug`'s volumes, lines and final call; `-debugsave`'s grids), `flirt::optimise` (the optimisations' starts, tolerances, evaluations, Powell's lines), `flirt::search` (the grids', threshold's and minima's lines), `flirt::bbr::cost_and_debug` (BBR's `debug` volume) | §2.3, §9.4 | spec, black-box |
| `flirt::cost` (the correlation ratio's and Woods' bin sums in parallel, each bin's additions in the sampling order: the same bits), `flirt::bbr::mask_distance` (the field map's search skipping the steps that cannot reach its mask: the same point found) | §6.2, §6.5, §12 | spec, larmorx's own design |

## The spec writer's answers (part 2, relay 4, 2026-11-02)

Written by a clean-room implementer under the same rules (none of the files listed above
opened). Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/flirt.md`](../../specs/flirt.md) §3, §4.1, §4.2, §5, §6.2, §6.4, §6.5, §7.1, §7.2, §8.1, §8.2, §8.5, §9.2, §9.3, §10a, §11 (FSL-90 to FSL-93), §12, §14's table; [`specs/mcflirt.md`](../../specs/mcflirt.md) §4.4, §6.2, §6.3, appendix A | the exact arithmetic of the costs, parameters, search, optimiser, normals, spline, sinc, forms; the four defects' defaults |
| Black-box runs of FSL 6.0.7.23's `flirt` and `convert_xfm` (FSL off the `PATH`) through the oracle cache: the fsl-flirt suite's standard tier and `-verbose 7` traces of its cases | where the two traces part, evaluation by evaluation |
| libstdc++'s documented `std::sort` (introsort, insertion sort below 16 elements) | `sort`, `dualsort` and FSL-22's `reproduce` order |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `flirt::param::{euler_compose, euler_decompose, euler_decompose_parts}` | every Euler composition and decomposition of FLIRT and `convert_xfm` (`-fixscaleskew` included) | §3 |
| `flirt::cost::centre_of_mass`; the weighted costs' edge factors one by one | the centres of mass in blocks; `(((w_t·w_r)·w_x)·w_y)·w_z` | §8.1 step 3, §6.4 |
| `flirt::optimise` (the unit along `û`, the convergence weights, `Settings::tol_x0`) | §8.2's line search and convergence test; the search's tolerances | §8.1, §8.2 |
| `flirt::search` (the coarse grid in single precision, its trilinear interpolation, the percentile, the minima's starts, the list, FSL-90) | §8.1's steps 2 to 9 | §8.1, §11 |
| `flirt::engine` (FSL-91, FSL-92, FSL-93, the reference parameters in force, fresh centres, the neurological forms' single-precision field of view, the output blur's edge) | §4.1, §4.2, §6.4, §8.5, §9.3 | §4, §6.4, §8.5, §9.3, §11 |
| `flirt::schedule::{std_sort, std_sort_checked}` | the order of rows of equal cost; NaN's order in the background value with `reproduce` | §7.1, §10a |
| `flirt::bbr::points` (with `pyramid::convolve`) | the boundary normals: a Gaussian of 2 mm, the gradient's weights 3, 1, 1/3, zeros outside | §6.5 |
| `flirt::output::{FlirtSpline, trilinear_constant, SincKernel}`, `Edge` | the spline exactly, `-setbackground`'s constant, the sinc's table and lookup, the edge rule before the blur | §9.2, §9.3 |

### Part 2, relay 5 (2026-11-02): from the random-testing trial (black-box runs of FSL 6.0.7.23)

| Code | Behaviour | Source |
|---|---|---|
| `flirt::engine::apply` (`-applyisoxfm`'s header) | single-precision `st`, each form swapped by both grids, every element in single precision, the qform's quaternion from its matrix | §9.3, §4.1 |
| `flirt::engine::completed_forms` | an unset form written as the set one (a registration's `-out`, applyxfm4D) | observed (docs/findings/fsl-flirt-original.md, "Random testing of every program") |
| `flirt::bbr::setup_errors`, the `(2)` warning of a NaN cost | BBR without a boundary voxel | §6.5; observed (the set-up's count) |
| `flirt::programs::Space::{of_image, unset}`, `cli::coords` | an image without forms at its voxel sizes; the standard programs' warning | §15.2; observed |
| `cli::common::header_bytes` | only the header's bytes read (memory) | observed (FSL's peak) |
| `cli::applyxfm4d`, `cli::makerot` | FSL-76, FSL-79, FSL-80 named only where they change a value | D13 |
| `bet::image::write_pieces` | zstd and bzip2 outputs of every stored type | §10.1 |

### Part 2, relay 6 (2026-11-02): the trial's open items (black-box runs of FSL 6.0.7.23)

| Code | Behaviour | Source |
|---|---|---|
| `cli::applyxfm4d` (`char_reading`, `x86_convert`) | a uint8 image read as signed char (FSL-111, `reproduce`; as stored by default); FSL-79's `(T)(c + 0.5)` and `(T)v` as x86-64 converts (through a 32-bit integer, wrapped to the type's bits) | §15.7; observed (docs/findings/fsl-flirt-original.md, "The open items of the trial closed") |
| `flirt::engine::formless_forms` | a reference without forms: the input's form through `W·D_in⁻¹·A⁻¹·D_ref`, as sform and qform with the input's code (`-out`, applyxfm4D's first matrix) | observed |
| `flirt::engine::Look::quaternion_final`, `bet::image::Output::rewrite_quaternion` | a quaternion computed from a matrix (completed, `-applyisoxfm`'s, a formless reference's) written once | observed |
| `flirt::engine` (`optimise`, `measurecost`), `flirt::search` | `Erroneous dof` once per row and once per free start, in the warnings' order; FSL-90's starts composed once below 6 dof | §8.2; observed |
| `flirt::mat::{inverse, upper_inverse, lu_left_looking, solve_vec}` | the 4 × 4 inverse exactly (the upper triangle's own, the left-looking LU, the columns in pairs) | §12 |
| `flirt::programs::avscale` | §3's decomposition and composition | §15.1, §3 |
| `cli::common::header_image` | the qform as FSL holds it (`bet::quatern`) | §4.1, §15.2 |
| `flirt::output::resample` (nearest neighbour) | the nearest index first, then the extraslice rule | §9.2; observed |
| `larmorx_core::defects::FSL_111` | the new defect's entry | D13, rule A |

## FAST (`fast`; relays 1 to 4, 2026-11-03)

`fast` (FAST's tissue segmentation of one channel and several, the tissue priors, its command
line, messages and outputs) was
written by a clean-room implementer who never opened FSL's source code (`~/fsl*/src`,
`crates-nc/fsl-source/` or any other copy), the replica crates (`crates-nc/`,
`crates/larmorx-afni-replica`, `crates-gpl/`), the restricted findings (`docs/findings/fsl-fast.md`,
`fsl-flirt.md`, `fsl-bet.md`, `fsl-mcflirt.md`, the AFNI source-derived ones, the register's
sections marked †), `docs/parts/`, the replica parts' and spec writers' investigations,
`validation/probes/`, `validation/difftest/triage.toml`, the replica authors' permanent cases or
the history of replica and spec-writer commits. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/fast.md`](../../specs/fast.md) (FSL 6.0.7.23, FAST4 2111.3) | the behaviour: the command line, the images, the mask and log image, the initialisation, the MRF, the bias field, the class order, the partial volumes, the members, the messages, the numeric conventions, the defects |
| Y. Zhang, M. Brady, S. Smith, "Segmentation of brain MR images through a hidden Markov random field model and the expectation-maximization algorithm", IEEE TMI 20(1):45-57, 2001; FSL's FAST documentation | the method |
| P. Selinger, "The GLIBC pseudo-random number generator" (2007), and glibc 2.35's `rand` run as a black box (ctypes) | `fast::rng` |
| Black-box runs of FSL 6.0.7.23's `fast` (through the oracle cache; as built and with the CORE-MATH preload): [`validation/investigations/cleanroom-fast/`](../../validation/investigations/cleanroom-fast/README.md) | the observations of [`docs/findings/fsl-fast-original.md`](../../docs/findings/fsl-fast-original.md): the variance's double second moment, `MRFWeightsTotal` after each sweep, the mixel sweep keeping a type, the initial type 0, the usage's and errors' text, FSL-100 with `-A`, `_restore` at NaN voxels; the bias kernel's double taps; the multi-channel weights, assignment and bias arithmetic; FSL's ANALYZE reading (no scale field, radiological storage for negative voxel sizes); `-H nan`'s estimate; the empty mask's messages |
| Black-box runs of FSL 6.0.7.23's `flirt` (through the oracle cache: `analyze_scale.py`, `applyxfm_tr.py`) | FSL's reading of its standard priors (ANALYZE); `-applyxfm`'s TR (FLIRT's findings) |
| The defect register's summary rows of FSL-96 to FSL-104 (`larmorx_core::defects`) | the defects' names in the notices |
| larmorx's own Apache-2.0 code: `bet::image` and `cli::{bet2, common, options, fmt}` (FSL's image names, reading and writing, the option parser, `%g`), `larmorx_core` (correctly rounded `exp`, `log`, `expf`, `logf`, thread pools, D13) | images, options, numbers, threads |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `fast::grid` | the mask's voxels in storage order, the 18 weighted neighbours and their weights (a padded map, compact per-voxel arrays, each row's range); the in-place sweeps' pipeline of planes (`Grid::gauss_seidel`: row `(y, z)` after plane `z-1`'s row `y+1`, the serial result on any thread count) | §4.2, §10; larmorx's own design |
| `fast::stats` | FSL's blocked sums (block by block in parallel, the blocks' sums in order: the same bits), the percentiles, the class means and variances (the second moment in double: observed), the negative log Gaussian | §4.4, §8.2, §8.3, §10; findings |
| `fast::rng` | glibc's `rand` after `srand(-1)` | §8.1; Selinger; black-box |
| `fast::bias` | the kernels and the separable convolution (lines and slices in parallel) | §4.5, §8.5 |
| `fast::single` | the negative values, the mask (FSL-103), the log image, the initialisation, the MRF iterations (`MRFWeightsTotal` after each sweep), the bias field, the class order, the hard segmentation, the members, the volumes, `-v`'s messages | §4.1, §4.3, §4.5-§4.10; findings |
| `fast::pve` | the mixel types, their likelihoods over the fractions, the sweep of iterated conditional modes (a type kept when none wins), the fractions, `_pveseg` | §4.8, §8.7; findings |
| `fast::multi` | several channels: the mask, the log images, the kernels and MRF weights, the initialisation, the members, the means and covariances (larmorx's own LU, any size), the likelihood, the bias fields, the MRF sweeps, the class order (FSL-98), the partial volumes in the log domain | §5, §8.6, §8.7; findings |
| `cli::fast` | the command line, the usage and its errors, the exit statuses, the outputs and their order, the channels, the priors (FLIRT's `-applyxfm` in process), FSL-96 to FSL-104 | §2, §3, §6, §7, §9; findings |
| `bet::image` (`read_analyze`) | an ANALYZE 7.5 image's scale field not applied; stored voxel sizes of negative product read radiologically | findings (black-box) |
| `cli::options` (`Given::Words`) | an option of several text values (`-A`) | §2.2 |
| `larmorx_core::math::exp_in_place` (relay 3) | the correctly rounded `exp`'s fast path four values at a time (vectorised with AVX2 and FMA through the core's audited dispatch), `exp` itself for the values it leaves: `exp`'s bits for every value; used by the partial volumes' likelihoods and the multi-channel initialisation | §10; CORE-MATH's `exp` (MIT, the core's port) |
| `flirt::engine::apply` (relay 3) | `-applyxfm`'s TR: the input's, as FSL reads it (its absolute value; 1 for a time axis with TR 0) | `specs/flirt.md` §9.3; FLIRT's findings (black-box) |
| `fast` (relay 4: random testing) | the bias field outside the mask `(0 - mean) · 0` (NaN over the image when the mean is not finite); several channels' `KMeans Iteration` line once the covariances are inverted; FSL-101's `-v` line before the crash's refusal; the outputs' expansion and the negative-value rule on the call's own pool (`Grid::expand_on`) | §4.5, §5.2, §9.1; findings, "Random testing" (black-box) |

## topup (`topup`; relay 1, 2026-11-03)

`topup` (the estimation of the susceptibility-induced field from reversed phase-encoding images:
its command line, configuration files, levels, field, cost, minimisers and outputs) was written
by a clean-room implementer who never opened FSL's source code (`~/fsl*/src`,
`crates-nc/fsl-source/` or any other copy), the replica crates (`crates-nc/`,
`crates/larmorx-afni-replica`, `crates-gpl/`), the restricted findings (`docs/findings/fsl-topup.md`,
`fsl-fast.md`, `fsl-flirt.md`, `fsl-bet.md`, `fsl-mcflirt.md`, the AFNI source-derived ones, the
register's sections marked †), `docs/parts/`, the replica parts' and spec writers'
investigations, `validation/probes/`, `validation/difftest/triage.toml`, the replica authors'
permanent cases or the history of replica and spec-writer commits. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/topup.md`](../../specs/topup.md) (FSL 6.0.7.23, fsl-topup 2203.5) | the behaviour: the command line, the configuration files, the acquisition parameters, the levels, the field's basis, the model, the cost, the minimisers, the outputs, the messages, the numeric conventions, the defects |
| J. L. R. Andersson, S. Skare, J. Ashburner, NeuroImage 20(2):870-888, 2003; S. M. Smith et al., NeuroImage 23:S208-S219, 2004; FSL's TopupUsersGuide; M. F. Møller, "A scaled conjugate gradient algorithm for fast supervised learning", Neural Networks 6:525-533, 1993 (in C. M. Bishop's formulation) | the model, Levenberg-Marquardt and the scaled conjugate gradients |
| Black-box runs of FSL 6.0.7.23's `topup` (through the oracle cache: the replica suite's cases and the probes of [`validation/investigations/cleanroom-topup/`](../../validation/investigations/cleanroom-topup/README.md), with `-v` and `--debug`) | the observations of [`docs/findings/fsl-topup-original.md`](../../docs/findings/fsl-topup-original.md): the periodic interpolation along every axis, the mask, the coefficients' layout, the gradient's and the Gauss-Newton Hessian's factors, the regulariser (a sum at the integer points of the support), the conjugate-gradient solve of the Levenberg-Marquardt step, the scaled conjugate gradients' parameters, the level transitions, the outputs' headers and values |
| The defect register's summary rows of FSL-116 to FSL-126 (`larmorx_core::defects`) | the defects' names in the notices |
| larmorx's own Apache-2.0 code: `bet::image` and `cli::{bet2, common, options, fmt}` (FSL's image names, reading and writing, the matrix reader, `%g`), `larmorx_core` (thread pools, D13) | images, options, numbers, threads |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `topup::images` | the intensity scaling, the regridding (periodic cubic spline), the subsampling, the Gaussian smoothing, each level's periodic cubic spline coefficients (cyclic tridiagonal solve) and their interpolation | §4.1, §4.4, §4.5; findings |
| `topup::spline` | the cubic B-spline basis and FSL's coefficient layout, the separable one-axis operators (each output value one row's sum: thread-independent), the refits (penalty: the two second differences that wrap around the ends), the regulariser's 1D Gram matrices | §4.2, §4.4, §4.6; findings |
| `topup::model` | one level's predictions (a line sampler when the displacement is along one axis), the mask, the cost, its gradient and the Gauss-Newton Hessian's voxel weights, the bending energy | §4.3, §4.6; findings |
| `topup::stencil` | the Hessian as a 7 × 7 × 7 stencil per coefficient, assembled by contracting one axis at a time, its product and the preconditioned conjugate-gradient solve | §4.7, §11; larmorx's own design |
| `topup::minimise` | Levenberg-Marquardt (damping, retries, the early end) and the scaled conjugate gradients | §4.7; Møller; findings |
| `topup` (`estimate`) | the levels, the transitions between them, the full-resolution predictions | §2.4, §4.4, §6; findings |
| `cli::topup` | the command line, the usage and its errors, the configuration files, the checks of the settings, the log, `-v`, the outputs and their order, FSL-119 to FSL-121 | §2, §3, §5, §6, §8; findings |
