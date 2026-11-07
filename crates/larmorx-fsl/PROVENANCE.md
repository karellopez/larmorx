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

## FLIRT's clean-room catch-up (relays 3 and 4, 2026-11-05)

Written by clean-room implementers under the same rules (FSL's source, the replica crates, the
source-derived findings, the register's restricted sections, `docs/parts/` and
`validation/probes/` not opened; the slips are recorded in the part's notes). Their only inputs
were:

| Input | What it gave |
|---|---|
| [`specs/flirt.md`](../../specs/flirt.md) §14's answers of 2026-11-03 (§2.3, §6.2, §8.1 step 9, §8.2, §9.2 to §9.4, §12, §15.1, §15.2, §15.5 to §15.8) | items 1 to 13 of `docs/next-steps.md`'s list for the original |
| Black-box runs of FSL 6.0.7.23 through the oracle cache (`validation/investigations/cleanroom-flirt/catchup_*.py`: the suite's cases with `-verbose 7`, the random-testing findings' shrunk cases, schedules of the probes' own written from FSL's `-v` echo) | the observations of [`docs/findings/fsl-flirt-original.md`](../../docs/findings/fsl-flirt-original.md), "The clean-room catch-up" (both relays) |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `flirt::cost` (`least_squares`, `SquaresSlice`) | `leastsq`'s three-level single-precision sums | §6.2 |
| `flirt::output::FlirtSpline` (`with_store`), `cli::applyxfm4d` | applyxfm4D's spline as FLIRT's (a double image filtered in double, a float image's passes in single, FSL-79's integer passes) | §9.3, §15.7 |
| `flirt::engine` (`resampled_header`, `level_header`, `debug_files`, `bbr_setup`), `flirt::bbr` (`no_points`, `cost_and_debug`) | `-debug`'s levels' headers (also `-applyisoxfm`'s), `global_refweight1`, `-noresample`'s levels and `testvol`'s held voxel sizes (observed), the boundary set-up's lines and error at every evaluation without boundary points, the zero `debug` volume, the first pair's set-up after the verbose centres (observed) | §9.4, §6.5; black-box |
| `flirt::engine` (`State::perturb`), `flirt::optimise` (`finite_start`, the starting matrix), `flirt::search` (`P` composed once, the grids' starting matrix, a NaN fine-grid scale) | a perturbation composed with all twelve parameters, an optimisation's start shown at its dof, NaN parameters started from the identity's (observed), the search's `P` | §8.1 step 9, §8.3; black-box |
| `flirt::output::{resample, padded}` | a NaN pre-image outside the input (observed) | black-box |
| relay 3: `flirt::param`, `flirt::search`, `flirt::mat`, `cli::{midtrans, pointflirt, coords, convert_xfm, pairreg}`, `flirt::schedule::PAIRREG_1..3` | items 1 to 8 (quaternion starts, FSL-90's messages, midtrans and FSL-241, FSL-204, `-warp`, the inverse on NaN matrices, `-fixscaleskew`'s abort, pairreg's schedules from FSL's `-v` echo) | §8.1, §12, §15; black-box |

## FAST (`fast`; relays 1 to 4, 2026-11-03)

`fast` (FAST's tissue segmentation of one channel and several, the tissue priors, its command
line, messages and outputs) was
written (relay 4: the speed work of §11, every output bit-identical to relay 3b's at 1, 7 and 14 threads,
[`docs/benchmarks/fsl-topup.md`](../../docs/benchmarks/fsl-topup.md)) by a clean-room implementer who never opened FSL's source code (`~/fsl*/src`,
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
| `fast::stats`, `fast::single`, `fast::pve`, `fast::multi`, `fast::Buffered`, `cli::fast` (the clean-room catch-up, 2026-11-05) | the spec answers of 2026-11-03 (`specs/fast.md` §12): FSL-202's variance (the double-precision mean's square by default, FAST's single-precision square with `reproduce`, in the class statistics and the partial volumes'), FSL-203's extrema (from the first voxel with `reproduce`, over the values that are not NaN by default), the rule for negative values clearing NaN voxels in its second branch only (`_restore` NaN elsewhere), standard output as FAST buffers it (the unflushed lines lost at an abort: the k-means lines, the partial-volume heading), several channels' class without weight (its means the plain sums, its covariance `0 / 0` as x86-64's NaN); from black-box runs: the heading before the images are read, FSL-100's and fewer than 0 classes' aborts after the image lines, the loops without a class (`-n 0`) and their lines | `specs/fast.md` §4.1, §4.4, §5.2, §5.3, §7, §9.1, §12; findings, "The catch-up of 2026-11-05" (black-box: `fsl_streams.py`, `probe.py`, `glibc_default.py`); the replica's code and FSL's source not opened, nor the spec writer's models (`validation/investigations/spec-answers-3/`: neither read nor run) |

## topup and applytopup (`topup`; relays 1 to 4, 2026-11-03)

`topup` (the estimation of the susceptibility-induced field from reversed phase-encoding images:
its command line, configuration files, levels, field, cost, minimisers and outputs) and
`applytopup` (the correction with that field: Jacobian modulation and least-squares
restoration, its command line and messages; relay 3) were written
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
| J. L. R. Andersson, S. Skare, J. Ashburner, NeuroImage 20(2):870-888, 2003; S. M. Smith et al., NeuroImage 23:S208-S219, 2004; FSL's TopupUsersGuide and ApplytopupUsersGuide; M. F. Møller, "A scaled conjugate gradient algorithm for fast supervised learning", Neural Networks 6:525-533, 1993 (in C. M. Bishop's formulation) | the model, Levenberg-Marquardt and the scaled conjugate gradients |
| Black-box runs of FSL 6.0.7.23's `topup` and `applytopup` (through the oracle cache: the replica suite's cases and the probes of [`validation/investigations/cleanroom-topup/`](../../validation/investigations/cleanroom-topup/README.md), with `-v` and `--debug`) | the observations of [`docs/findings/fsl-topup-original.md`](../../docs/findings/fsl-topup-original.md): the periodic interpolation along every axis, the mask, the coefficients' layout, the gradient's and the Gauss-Newton Hessian's factors, the regulariser (a sum at the integer points of the support), the conjugate-gradient solve of the Levenberg-Marquardt step, the scaled conjugate gradients' parameters, the level transitions, the outputs' headers and values; applytopup's sampling, mask, Jacobians, restoration matrices, movements as matrices (findings 17 to 22); the vb methods' iterations, prior, counts, invalid voxels and mask, FSL-144 (findings 23 to 28) |
| The defect register's summary rows of FSL-116 to FSL-126 and FSL-132 to FSL-135 (`larmorx_core::defects`) | the defects' names in the notices |
| larmorx's own Apache-2.0 code: `bet::image` and `cli::{bet2, common, options, fmt}` (FSL's image names, reading and writing, the matrix reader, `%g`), `larmorx_core` (thread pools, D13) | images, options, numbers, threads |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `topup::images` | the intensity scaling, the regridding (periodic cubic spline, or trilinear with `--interp=linear`), the subsampling, the Gaussian smoothing (none for a negative width: FSL-125 reproduced), each level's periodic cubic spline coefficients (cyclic tridiagonal solve) and their interpolation | §4.1, §4.4, §4.5; findings |
| `topup::spline` | the cubic and quadratic B-spline bases and FSL's coefficient layout, the separable one-axis operators (each output value one row's sum: thread-independent), the refits (penalty: the two second differences that wrap around the ends), the regulariser's 1D Gram matrices | §4.2, §4.4, §4.6; findings |
| `topup::model` | one level's predictions (a line sampler when the displacement is along one axis and nothing moves), the movements' sampling, the mask, the cost, its gradient (the movements' exact derivatives) and the Gauss-Newton Hessian (its voxel weights, the movements' dense blocks), the bending and membrane energies (FSL-117) | §4.3, §4.6; findings |
| `topup::stencil` | the Hessian as a 7 × 7 × 7 stencil per coefficient, assembled by contracting one axis at a time, with the movements' columns and block; its product, single precision (`--numprec=float`) and the preconditioned conjugate-gradient solve | §4.7, §11; larmorx's own design |
| `topup::pool` | none (larmorx's own speed work, relay 4): the level's large arrays kept for reuse at their exact length, so the evaluations do not touch new memory; no behaviour | §11; [`docs/benchmarks/fsl-topup.md`](../../docs/benchmarks/fsl-topup.md) |
| `topup::movement` | the rigid-body matrices (the Euler order and signs, the centres), their derivatives, the free parameters | §4.3, §6; findings 13 |
| `topup::minimise` | Levenberg-Marquardt (damping, retries, the early end, the end on a free parameter) and the scaled conjugate gradients, with `--debug`'s numbering | §4.7; Møller; findings |
| `topup` (`estimate`) | the levels, the transitions between them, the full-resolution predictions | §2.4, §4.4, §6; findings |
| `cli::topup` | the command line, the usage and its errors, the configuration files, the checks of the settings, the log, the outputs and their order, FSL-117 to FSL-126 | §2, §3, §5, §6, §8, §10; findings |
| `cli::topup_debug` | `-v`, `--debug`'s files, `--trace`'s stages, `--profile`'s file | §2.2, §5; findings 9, 16 |
| `topup::apply` | applytopup's corrections: the field of the coefficients or of a field image (its mirrored spline's derivatives), the movements (parameters about the field's centre, or matrices), the sampling and its mask, `--method=jac`, the collections, the resampling matrices and the regularised line solves of `--method=lsr` (dense Cholesky), FSL-132 and FSL-134's Jacobian | §7, §9.4, §10; Andersson et al. 2003; findings 17 to 22 |
| `topup::vb` | the undocumented `--method=vb2D`, `vb3D`, `vb4D`: the mean-field variational-Bayes restoration (Gamma priors on the noise and smoothness precisions, the squared-second-difference prior weighted by the voxel sizes, Gauss-Seidel over the lines, its printed iterations), FSL-141 to FSL-143 | §7.5, §10; findings 23 to 27 |
| `cli::applytopup` | the command line (long and short options), the usage and the compulsory options' block, the checks in FSL's order with FSL's aborts (exit 1), `--topup`'s files, `--datatype` (FSL-09), the output, FSL-132 to FSL-135 and FSL-141 to FSL-144 | §7, §8, §10; findings 17 to 28 |
| `topup::model`, `topup::minimise`, `cli::topup_debug` (relay 5b: random testing) | FSL-146's reproduce (each axis' Jacobian term over the volumes displaced along it), the mask of an oblique phase encoding (no axis wraps), the scaled conjugate gradients' second gradient with the current point's regulariser weight, `--debug`'s images in FSL's order | findings 32 to 35 (black-box: `first_step.py`) |
| `topup::movement`, `topup::model`, `topup::spline`, `topup::images`, `topup::apply`, `topup::vb`, `cli::topup`, `cli::applytopup` (the clean-room catch-up, 2026-11-04) | the spec answers of 2026-11-03 (`specs/topup.md` §13): FSL-201's `reproduce` (FSL's single-precision rotation matrices, with glibc's `cosf` and `sinf` from `larmorx_core::math`, and their forward differences; `--rbmout` FSL's matrices), FSL-145's counts and its single-coefficient errors, FSL-146 and the mask decided by the encoding, the moved volumes' frame and the image's bounds, the regulariser's sampling of quadratic B-splines, the spline prefilter's truncated start (periodic and mirrored), applytopup's matrices row by row by FSL's walk with FSL-133's `reproduce`, FSL-134's field of another shape and its Jacobian's scales, applytopup's FSL-only coefficient counts in `reproduce` | `specs/topup.md` §4.2 to §4.6, §7.3, §7.4, §9.4, §10.1, §13; findings 37 to 44 (black-box: `catchup_probes.py`, `fsl201_spread.py`, the cache's `--debug` runs); the replica's code and FSL's source not opened, nor the spec writer's models (`validation/investigations/spec-answers-3/`: only their README) |

## fslmaths (`maths`, `cli::fslmaths`; relay 1, 2026-11-03)

`fslmaths` (FSL's image calculator: its command line, computing types, binary operations,
thresholds, masks, voxelwise functions, random numbers, index and grid; relay 2 its kernels and
spatial filters; the next relays add the reductions and the statistics) was written by a clean-room implementer
who never opened FSL's source code (`~/fsl*/src`, `crates-nc/fsl-source/` or any other copy),
the replica crates (`crates-nc/`, `crates/larmorx-afni-replica`, `crates-gpl/`), Armadillo's,
nlohmann's or glibc's sources, the restricted findings (`docs/findings/fsl-maths.md`,
`fsl-topup.md`, `fsl-fast.md`, `fsl-flirt.md`, `fsl-bet.md`, `fsl-mcflirt.md`, the AFNI
source-derived ones, the register's sections marked †), `docs/parts/`, the replica parts' and spec
writers' investigations, `validation/probes/`, `validation/difftest/triage.toml`, the replica
authors' permanent cases or the history of replica and spec-writer commits. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/fslmaths.md`](../../specs/fslmaths.md) (FSL 6.0.7.23, fsl-avwutils 2209.6) and `specs/bet.md` §3, §4.2 | the behaviour: the command line, the usage text, the data types, C's arithmetic, every operation, the robust range, the messages, the numeric conventions, the defects |
| B. A. Wichmann and I. D. Hill, Applied Statistics 31(2):188-190, 1982 (AS 183) | `-rand`'s generator |
| M. Abramowitz and I. A. Stegun, Handbook of Mathematical Functions (1964), 26.2.23 | the start of `Φ⁻¹`'s iterations (relay 3) |
| The `libm` crate (0.2, MIT/Apache-2.0: FreeBSD msun's `erf` and `erfc` in Rust) | `Φ`'s `erf` and `erfc` (`larmorx_core::math` has no `erf` yet) |
| Black-box runs of FSL 6.0.7.23's `fslmaths` (through the oracle cache: the replica suite's cases and the probes of [`validation/investigations/cleanroom-maths/`](../../validation/investigations/cleanroom-maths/README.md)) | the observations of [`docs/findings/fsl-maths-original.md`](../../docs/findings/fsl-maths-original.md): the thresholds' far side (inclusive for `-thr`/`-uthr`, exclusive for `-bin`; `short` wraps too), `-save`'s type and display range, the layouts of an image of other sizes (FSL-159), the input's and an operand's messages; relay 2: the kernel's count, the convolutions' messages and normalising sum, the FFT path's orientation and NaN (FSL-191), `-edge`'s denominator, `-dog_edge` under integer types, the empty mask of `-inm`, the subsampling's last bits; relay 4: `-roc`'s arithmetic, its curve file's creation and FSL-193 |
| The defect register's summary rows of FSL-151 to FSL-180, FSL-187 and GLIBC-07 (`larmorx_core::defects`) | the defects' names in the notices; FSL-191, FSL-192 and FSL-193 are the original's own (findings 9, 14, 20) |
| larmorx's own Apache-2.0 code: `bet::image` and `cli::{bet2, common, options}` (FSL's image names, reading and writing, `strtod`), `fast::rng` (glibc's `rand`, for `-seed`), `flirt::mat::atoi`, `larmorx_mri::hmc::image` (the stored values), `larmorx_core` (correctly rounded maths, thread pools, D13) | images, numbers, threads |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `maths::num` | the computing types (`char`, its unsigned form for FSL-154's correction, `short`, `int`, `float`, `double`), C's arithmetic on x86-64 (promotion, wrapping, truncating division), x86-64's conversions and FSL-151's clamped ones, NaN's bits through the conversions | §2.4, §4 |
| `maths::image` | the input and the operands read in the computing type (scaling, the radiological frame), the output's rounding and conversion, FSL-157, `-save`'s type, the header | §3; findings |
| `maths::ops` | the binary operations with numbers and images and their layouts (FSL-159), the thresholds and their far side (FSL-152, FSL-153), `-bin`, `-binv`, `-invbin`, the robust range (FSL-153, FSL-187) and the percentage thresholds, the voxelwise functions (correctly rounded: GLIBC-07), Wichmann-Hill and `-seed`, `-index`, `-grid` | §5-§8; AS 183; findings |
| `cli::fslmaths` | the command line, the usage text, `-dt`/`-odt` and the computing type (FSL-154, FSL-155), the chain, the messages and exit statuses, FSL-156 and FSL-158's refusals, the orientation warning and the sizes' rule, the display range | §2, §3, §17, §19; findings |
| `maths::kernel` (relay 2) | the current kernel and `-kernel`'s types (`3D`, `2D`, `box`, `boxv`, `boxv3`, `gauss`, `sphere`, `file`, unknown), their sizes and values in single precision, the separable flag, FSL-160's zero-width Gaussian; the Gaussian's exponent as FSL's impulse responses show it (`i² (d d)` per axis, added x, y, z; relay 6) | §9; findings 6, 7, 23 |
| `maths::filters` (relay 2) | the morphological filters' window, count and overflow (Armadillo's bounds error as FSL prints it), the two-partial extremes and sums, the mode and the median (FSL-162), `-dilall`'s queue, the separable filter, the direct sum, FSL's FFT criterion and its orientation (computed by the direct sum; FSL-192), FSL-161's refusals, FSL-191, the warnings | §10.1, §10.2, §18, §19; findings 5-9, 14 |
| `maths::spatial` (relay 2) | `-subsamp2` and `-subsamp2offc` (the grid and geometry, FSL-163), `-edge`, `-dog_edge` (FSL-161, FSL-164), `-fillh`, `-fillh26` (FSL-165), `-inm`, `-ing` (the block sums, FSL-152, FSL-153 for the mask, the empty mask) | §10.3-§10.7, §18, §19; findings 10-13 |
| `maths::reduce` (relay 3) | the reductions along T, X, Y, Z (another letter: none): `max`, `min`, `maxn` from the first value (FSL-153), `mean` and `std` with §18's block sums (FSL-167), `median` and `perc` in libstdc++'s order (FLIRT's `std_sort`; FSL-168, refused where its scans would leave the values), `ar1` in the computing type's arithmetic, the percentile's range error | §11, §18, §19 |
| `maths::stats` (relay 3) | `Φ` in the spec's form (the `libm` crate's `erf`, `erfc`), `Φ⁻¹` (Abramowitz and Stegun 26.2.23 refined by Halley's iterations) with the domain error and `MAXNUM`, `-ztop` (FSL-170), `-ptoz` (FSL-172), `-pval`, `-pval0`, `-cpval` (FSL-169), `-rank`'s ties and `-ranknorm` (FSL-162, FSL-167), `-randn`; the NaN bits of findings 16 | §8.2, §12, §19; findings 16 |
| `maths::regions` (relay 3) | `-roi`'s box (the file's x mapped, the corners ordered, size −1 and 0), `-bptf`'s high-pass (the local weighted line, `c₀ + x − c`, the mean removed) and low-pass (the normalised Gaussian), FSL's evaluation of the weights and products (findings 15), FSL-171 | §13, §19; findings 15 |
| `maths::tfce` (relay 3) | `-tfce` (the interior voxels, the thresholds' single-precision sums, the clusters by connectivity class, `pow(s, E) · powf(h, H)` with `E` and `H` in single precision, the messages; FSL-153, 174, 175, 176), `-tfceS` (the voxel's cluster per threshold, the sum from the top, the output at the threshold reached, at `h₀` or the voxels outside every cluster; FSL-174, 176, 177, 178), `-tfce_minT`, `-tfce_delta` | §14, §19; findings 17 |
| `maths::roc` (relay 4) | `-roc`: the log transform, the thresholds' steps, the truth's mask, true voxels and candidates, the rates and their means, the noise image's family-wise rate and spreads, the areas and the last rectangle, the quartiles, the curve file and the printed line (findings 19), FSL-153, FSL-179, FSL-180, FSL-193; the counts by binary search over each volume's sorted values (the original's own) | §15, §19; findings 19, 20 |
| `maths::tensor` (relay 3) | `-tensor_decomp`: the 6-volume check, the symmetric eigenproblem (cyclic Jacobi rotations, the original's own; FSL calls LAPACK), `FA`, `MD`, `MO` in single precision, the nine images' names and display ranges, the chain on with `FA`, FSL-166 | §16, §19; findings 18 |

## fslstats (`cli::fslstats`; relay 4 of fslmaths' and fslstats' part, 2026-11-03)

`fslstats` (FSL's image statistics: its preoptions, every option, the messages and exit statuses,
the JSON output, FSL-181 to FSL-189) was written by the same clean-room implementer, under the
same rules as fslmaths above (FSL's source, its tests included, never opened; the replica suite's
`fsltest/` cases, which read FSL's own test script and images from FSL's source tree, are left
out of the original's suite). Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/fslstats.md`](../../specs/fslstats.md) (FSL 6.0.7.23, fsl-avwutils 2209.6) | the behaviour: the command line, the usage text, the pass state and the options that change it, every statistic, the output and nlohmann's number form as the spec describes it, the defects |
| Black-box runs of FSL 6.0.7.23's `fslstats` (through the oracle cache: the replica suite's synthetic cases and the probes of [`validation/investigations/cleanroom-maths/`](../../validation/investigations/cleanroom-maths/README.md), `show_fslstats.py`) | the observations of [`docs/findings/fsl-maths-original.md`](../../docs/findings/fsl-maths-original.md) findings 21: the preoptions' rule, the messages' order, the lost output of a crash, the masks `-P`, `-e`, `-E`, `-w` make, the empty forms |
| The defect register's summary rows of FSL-181 to FSL-189 (`larmorx_core::defects`) | the defects' names in the notices |
| larmorx's own Apache-2.0 code: `maths::{image, ops, num, reduce}` (the reading, the robust range, the extremes, the block sums, C's conversions), `flirt::schedule::std_sort_checked` (libstdc++'s sort, for `-p`, `-P` with `reproduce`), `cli::{common, options, fmt}` | images, numbers, formats |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `cli::fslstats` | the preoptions (`-t`, `-K`, `-json`) and the usage, the passes, the thresholds and masks (`-l`, `-u`, `-k`, the masks `-P`, `-e`, `-E`, `-w` make), `-d`, `-a`, `-n`, the counts, the masked and unmasked means and standard deviations in FSL's blocks (FSL-182), `-D`, the extremes and their positions (FSL-181, FSL-184), the robust range (FSL-181, FSL-187), the percentiles (libstdc++'s order where NaN or ±0 make it matter), the histograms (FSL-186, FSL-187), the entropies (FSL-185), the bounding box (FSL-188), the centres of gravity, `-t` with `-k`, `-d` (FSL-183), the text and the JSON document (FSL-189), the messages and statuses, a crash's lost output | §2-§8; findings 21 |
| `cli::fmt::json` | nlohmann/json's number form (the shortest round-trip digits; fixed notation from 1e-4 to below 1e15, an integral value with `.0`; else `d.ddde±XX`; `null`) | §6.2 |

### Speed and FSL-187 in BET and FLIRT (relay 5 of fslmaths' and fslstats' part, 2026-11-03)

By the same clean-room implementer, under the same rules; no new inputs but timed black-box runs
of FSL 6.0.7.23 (the benchmarks of [`docs/benchmarks/fsl-maths.md`](../../docs/benchmarks/fsl-maths.md))
and of the `larmorx-nc` replica's program, and the defect register's FSL-187 row.

| larmorx | What changed | Source |
|---|---|---|
| `maths::filters` | the separable pass and the direct sum a row of x at a time (the weights' sum once per position; the kernel's zeros left out of a finite volume's sums: a sum from +0 never changes by a ±0 product), every voxel's taps still added in their order; NaN results recomputed by x86-64's rules; the convolutions in place, volume by volume; `-fmean`'s convolution of ones computed on one volume for a series (its messages printed per volume) | the original's own design (§20); the same bits as before on every case of the suite and the benchmarks |
| `maths::regions` | `-bptf`: the windows' sums that do not depend on the voxel once per `t`, the others for blocks of 64 voxels in the series' order, in waves written back in place (no copy of the image); a series holding a NaN filtered again on its own | the original's own design; the same bits |
| `maths::image`, `cli::fslmaths` | the input's values moved, not copied, where they already have the computing type (mirrored in place); the output's values moved to the writer where writing converts nothing: one copy of the image held, as FSL holds | the memory rule (CLAUDE.md) |
| `bet::params`, `bet` (`find_surface`), `cli::bet2`, `flirt::engine` (`clamp_range`) | FSL-187's `reproduce` switch in BET's and FLIRT's robust range (a value whose bin does not fit in an `int` counted in the first bin) and its notice where the correction moves the range; the defaults unchanged | the register's FSL-187 row; findings 22 of `docs/findings/fsl-maths-original.md` |

### Random testing's corrections (relay 6 of fslmaths' and fslstats' part, 2026-11-03)

| Module | What changed | From |
|---|---|---|
| `maths::image` (`regrid_x`), `maths::spatial` (`subsample_header`), `maths::reduce` | the geometry of a new grid along x: set matrices through the held (mirrored) frame and the operation, unset ones through the mirroring; a subsampled set qform's quaternion from the product of matrices (its zeros +0) | black-box runs: `probe_geometry.py`, the trial's findings (findings 24 of `docs/findings/fsl-maths-original.md`) |
| `maths::spatial` | FSL-75 named in `-subsamp2` (the unused background value's buffer, by the register's row); the subsampling's sums in double; `-edge`'s differences as C's `int`; FSL-165 on every image with an axis of size 1 | the register's FSL-75 row; findings 24, 27, 29 |
| `maths::filters` | the off-centre warning by the kernel's sizes; no empty-mask message for a kernel without a value at or above 0.5 | `probe_kernel_file.py` (findings 25) |
| `maths::tfce` | the step tested before the sizes; a double image's TFCE in single precision | findings 26 |
| `maths::roc` | the false-positive candidates below the true voxels' bound | findings 29 |
| `maths::stats` | `-ptoz` of NaN: the NaN's sign flipped; `-ztop` of NaN: the positive NaN | findings 30 |
| `cli::fslmaths` | FSL-157 named for every int8 `-odt input`, FSL-154 for every uint8 image computed unsigned | findings 29; the register's rows |
| `cli::fslstats` | the histogram of an empty mask (zeros, or none with an empty range) and the JSON's empty list without a histogram | findings 28 |
| `cli::common` (`name_start`), `cli::topup`, `cli::applytopup` | topup's and applytopup's file names split at `/`, and at `\` on Windows only | `docs/api/fsl-topup.md`, "File names on Windows" |

### The direct sum where FSL takes its FFT (relay 7b of fslmaths' and fslstats' part, 2026-11-04)

| Module | What changed | From |
|---|---|---|
| `maths::filters` (`direct_volume`, `convolve_in_place`) | where FSL convolves by its FFT (§10.2's criterion), the direct sum's products in double precision added to a double-precision sum, rounded once to single precision (FSL's direct path keeps §10.2's single-precision sum); the same order of taps, NaN rules and bits at any thread count | findings 31 (the page's `-kernel sphere 21 -fmean` against its exact result; `fft_cases_reference.py` on the record's FFT cases) |

## standard_space_roi and fslroi (the clean-room catch-up, relay 5, 2026-11-05)

Written by a clean-room implementer under the same rules (FSL's source, the scripts' text in
`~/fsl-6.0.7.23/bin`, the replica crates (`crates-nc/` with its `wheel-data/` and manifest
scripts), the source-derived findings, the register's restricted sections, `docs/parts/` and
`validation/probes/` not opened; FSL's standard images read only as data files, by path). Its
only inputs were:

| Input | What it gave |
|---|---|
| [`specs/standard_space_roi.md`](../../specs/standard_space_roi.md) (FSL 6.0.7.23's `standard_space_roi`, fsl-flirt 2111.4, and `fslroi`, fsl-avwutils 2209.6) | the command lines, the usages, the work's steps and their files, the messages and statuses, bash's report of an abort (its lines and commands), fslpy's `imcp`, the types and the region, FSL-211 to FSL-215 and FSL-217 |
| [`specs/bet.md`](../../specs/bet.md) §5.4 | FSL's standard images: their sizes and SHA-256, FSL's package (its address, size and SHA-256, its members), the order of the search, the refusal |
| Black-box runs of FSL 6.0.7.23's `standard_space_roi`, `fslroi` and `flirt` through the oracle cache (`validation/investigations/cleanroom-bet/ssroi_fsl_runs.py`, `ssroi_stage_probe.py`, `fit_translate.py`; the BET suite's cases) | the observations of [`docs/findings/fsl-bet-original.md`](../../docs/findings/fsl-bet-original.md), "standard_space_roi and fslroi": fslroi's matrices through the held frame, the script's two tests' forms, FLIRT's `-out` display range |
| GNU bash 5.2's `test` builtin run on this machine (its messages for argument lists with extra words) | `[`'s evaluation and messages for names split at blanks (FSL-211, `reproduce`) |
| One download of FSL's `fsl-data_standard-2208.0-0.tar.bz2` from FSL's site (2026-11-05) | the package's tar layout (its members `data/standard/<image>.nii.gz`) and the three images' bytes, checked against the spec's checksums; not kept in the repository |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `cli::fslroi` | `fslroi`'s three forms, `atoi`'s numbers, the region (sizes of −1, a box outside the image, the time range), the output type (`-odt input`'s rule), the matrices moved to the box's corner (FSL's held-frame arithmetic), the messages, FSL-215, FSL-217 | spec §3, black-box (`fit_translate.py`) |
| `cli::ssroi` | the script's options, usage and checks, the five steps run in this process with the original's programs (their messages, bash's `Aborted` report), the field of view resampled once where it is both the mask and the region, the temporary images uncompressed without `-d`, fslpy's `imcp` (a copy, or the layout converted), the cleanup; FSL-211 (bash's splitting and `[`'s messages), FSL-212, FSL-213, FSL-214 | spec §2, §6, §7, black-box |
| `data` | FSL's standard images found as §5.4 says: the named directory, larmorx-nc's copy read as data (checked), `$FSLDIR`, larmorx's data cache filled once by a checksum-verified download (`curl`, else `wget`) of FSL's package, only its three images kept with a manifest; the refusal naming every place looked | `specs/bet.md` §5.4; the decision log, 2026-11-04 |
| `flirt::engine` (the `-out` writers) | `-out`'s display range 0 and 0 | black-box (findings) |

## bet -S, -B, -A, -A2 (the clean-room catch-up, relay 6, 2026-11-05)

Written by a clean-room implementer under the same rules (FSL's source, FSL's `bet` and
`standard_space_roi` scripts' text, the replica crates, the source-derived findings, the
register's restricted sections, `docs/parts/` and `validation/probes/` not opened). Its only
inputs were:

| Input | What it gave |
|---|---|
| [`specs/bet.md`](../../specs/bet.md) §5.3, §5.4, §7, §9 to §11.1 | each variation's programs, their arguments, files and types, the eye threshold in dc's decimal arithmetic, the standard images, the messages and statuses, FSL-44, FSL-46, FSL-63, FSL-216, FSL-217 |
| [`specs/standard_space_roi.md`](../../specs/standard_space_roi.md) §2.4, §3.2; [`specs/fslstats.md`](../../specs/fslstats.md) §4, §5.7 | standard_space_roi's status 0 with `-d`; fslroi's types (FSL-217); `fslstats -l -C` (the robust run's centre) |
| Black-box runs of FSL 6.0.7.23's `bet` (the suite's option cases, with `-d` and `-v` added), `bet2`, `fslstats` through the oracle cache (`validation/investigations/cleanroom-bet/ssroi_fsl_runs.py`) | the observations of [`docs/findings/fsl-bet-original.md`](../../docs/findings/fsl-bet-original.md), "bet -S, -B, -A, -A2": fslstats' six decimals, a negative mean's `dc: stack empty`, the files a failed run leaves, the commands' texts in bash's reports, fslpy's message for a missing image, the sign of bet2's zero minimum |
| GNU dc 1.4.1 run on this machine (`10 k <mean> 3.5 <sd> * + p`, its two streams) | the threshold's text (scales, truncation, `.5`, `0`, `-`), dc's messages split between standard error (`dc: `) and standard output |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `cli::bet_script` | `-S`, `-B`, `-A`, `-A2` as the script's sequences of the original's programs in this process (bet2, standard_space_roi, FLIRT, fslmaths, fslstats, FAST with its streams discarded, betsurf, the script itself), bash's `Aborted` report and the `ERR` trap's line, fslpy's `immv`, the eye threshold (`Decimal`: dc's `*` and `+` at scale 10, its text), FSL-63's removal of `-s`, FSL-216 | spec §5.3, §9; black-box (findings) |
| `cli::bet` | the standard images found before anything runs, the temporary images uncompressed when not kept, a failed run's temporary files left (rewritten in the run's format), FSL-46's cleanup by the files created or changed, `-R`'s centre by `fslstats -l 0.001 -C`, `-F`'s and `-Z`'s temporary images in fslroi's types (FSL-217) | spec §5.1 to §5.4, §9; `specs/standard_space_roi.md` §3.2; `specs/fslstats.md` §5.7 |
| `bet::params` (`min_max`) | of equal extremes the first kept (a zero's sign) | black-box (findings) |
| `cli::ssroi` | fslpy's `Could not find a supported file with prefix "<name>"`; the status 0 with `-d` in `reproduce` | `specs/standard_space_roi.md` §2.4; black-box |

### The clean-room catch-up, relay 7 (2026-11-05): betsurf's profiles, bet -Z's doubles, bet2's zero centre

| Input | What it gave |
|---|---|
| Black-box runs of FSL 6.0.7.23's `betsurf` on single profiles (a 0.2 mm triangle casting one profile; `-v`'s share; through the oracle cache, `validation/investigations/cleanroom-bet/profile_oracle_cached.py`) on the BET options part's random-testing images, and its whole runs (`betsurf_shares.py`, `replay_trial.py`) | the four readings of `specs/bet.md` §6.4 of [`docs/findings/fsl-bet-original.md`](../../docs/findings/fsl-bet-original.md), "The BET part's random-testing differences" |
| Black-box runs of FSL's `bet -Z -d` (case 330 of that trial) and `bet2 -c` on an empty brain's `fslstats -C` centre (the permanent cases `original-bet-R-*`) | `-Z`'s float64 temporaries; a centre of zeros in the radiological frame is "not given" |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `betsurf::profile` | the backward searches never read a profile's first sample; the T1-only scan for `b` starts at the second sample when `a` is the first; step 5's first bright sample strictly within 10 mm, each of its searches after the previous one's sample; an empty outer-skull search range counts the scalp only | spec §6.4; black-box (findings) |
| `cli::betsurf` | FSL-50's stop not reproduced (FSL observed to run on) | black-box (findings) |
| `cli::bet` (`-Z`) | a float64 input's slices and padded image kept double | black-box (findings) |
| `cli::bet2` (`centre_mm`) | `-c` of three zeros after the conversion into the radiological frame is the centre of gravity | spec §3.2; black-box (findings) |

## PRELUDE (`prelude`, `cli::prelude`; relay 1 of PRELUDE's clean-room part, 2026-11-06)

`prelude` (FSL's phase unwrapping: the mask, the linear ramps, the phase intervals and regions,
the constraints between regions and their merging, the slices' joining, the command line, its
messages, failures and defects) was written by a clean-room implementer who never opened FSL's
source code (`~/fsl*/src`, `crates-nc/fsl-source/` or any other copy), the replica crates
(`crates-nc/`, its `larmorx-nc-fugue` above all, `crates/larmorx-afni-replica`, `crates-gpl/`), the
restricted findings (`docs/findings/fsl-prelude.md` and the others of CLAUDE.md rule 1, the
register's sections marked restricted), `docs/parts/`, the replica author's probes
(`validation/investigations/fsl-prelude/`), the replica's suite and evidence modules, its
permanent random-testing cases, `validation/probes/`, or the spec writer's probe and model as text
(`validation/investigations/prelude-spec/blackbox.py`, `model.py`: run as black boxes only). Its
only inputs were:

| Input | What it gave |
|---|---|
| [`specs/prelude.md`](../../specs/prelude.md) (FSL 6.0.7.23, fsl-fugue 2603.0) | the behaviour: the command line, the images, the extrema and the rewrap, the modes, the mask, the ramps, the intervals and regions, the constraints, the choice rule and the folds, the reference region, the slices' join, the messages, the numeric conventions, the defects FSL-261 to FSL-272 |
| M. Jenkinson, "Fast, automated, N-dimensional phase-unwrapping algorithm", Magnetic Resonance in Medicine 49:193-197, 2003; FSL's FUGUE user guide (PRELUDE) | the method |
| Black-box runs of the spec writer's probe (`blackbox.py`) and of the replica's program (`larmorx-nc fsl prelude`, as a program, both D13 modes) on the probe's and larmorx's own generated inputs ([`validation/investigations/fsl-prelude-original/`](../../validation/investigations/fsl-prelude-original/README.md)); fMRIPrep 25.2.6's seven prelude outputs | no `Loading volumes` before a complex image (§4.11's list read with §3.1); relay 1's comparisons |
| The defect register's summary rows of FSL-261 to FSL-272 and GLIBC-07 (`larmorx_core::defects`) | the defects' names in the notices |
| larmorx's own Apache-2.0 code: `bet::image` and `cli::{bet2, common, options, fmt}` (FSL's image names, reading and writing, the parser, `%g`, `%f`), `maths::num` (x86-64's NaN rules), `flirt::schedule::std_sort_checked` (libstdc++'s sort from its documentation), `larmorx_core` (thread pools, D13, `math::Libm`'s `atan2f`) | images, options, numbers, threads |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `prelude::num` | `rnd`, `wrap`, the intervals, FSL's extrema and the default's (FSL-272) | §4.2, §4.6, §6 |
| `prelude::components` | the components (6-connected, or 4-connected in a slice) with roots at their first voxel: the numbering by first voxel and the largest component's tie; larmorx's own design (in-plane unions in parallel over the slices) | §4.4, §4.6, §10 |
| `prelude::merge` | the constraints' sums in the visiting order, `K` and `C`, the choice sequence (largest `C`, smallest pair; NaN by step 1's rule), the folds, the offsets, the reference region; larmorx's own structures (a heap with lazy deletion, neighbour lists, a weighted union-find) | §4.7, §4.8, §10 |
| `prelude::volume` | the default threshold, the candidate set and the largest component (per slice in the slices mode), FSL-271's reading of another grid, FSL-269's removal of NaN phases, the ramps' estimate, removal and restoration (FSL-261's domain), the regions in the three modes, the slices' join, the unwrapped values | §4.4 to §4.10, §9 |
| `cli::prelude` | the parser and the checks in FSL's order, the usage, the images (the complex input's readers, FSL-263 to FSL-265, `atan2f` with GLIBC-07's witness), the volumes processed and their aborts, the range check and the rewrap, the modes, the writes in FSL's order (FSL-262, FSL-266), FSL-270, FSL-271, the notices | §2, §3, §4.1 to §4.3, §4.11, §5, §9 |

### Relay 2 of PRELUDE's clean-room part (2026-11-06): the pair header and the time step

Black-box runs of FSL 6.0.7.23 through the oracle cache (no source read):
[`validation/investigations/fsl-pair-header/`](../../validation/investigations/fsl-pair-header/README.md)
(`probe.py`: every pair `FSLOUTPUTTYPE` with fslmaths, fslroi, bet2, fast, flirt, mcflirt,
prelude, topup, applytopup; `time_step.py`) and the random-testing trial of the original.

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `bet::image::write` (shared by BET, betsurf, FAST, FLIRT and its programs, topup, applytopup, fslmaths, fslroi, standard_space_roi, prelude) | a pair's header file holds the header block, NIfTI's four extension bytes and the kept extensions (352 bytes without extensions, NIfTI-2 544), every pair type, in its own compression; a zstd or bzip2 pair keeps its extensions | `probe.py`: FSL's header files |
| `cli::prelude` | the time step of `-p`, `-a` and `-m` as FSL's image layer reads it (its magnitude; 1 for an image with a time axis and a time step of 0), and of `-c` (its magnitude, a 0 kept), which every output keeps from its source | `time_step.py`, the trial's findings; `specs/flirt.md` §4.1 states the same reading |

### Relay 3 of PRELUDE's clean-room part (2026-11-07): memory, the time step, the spec answers' follow-ups

Black-box runs of FSL 6.0.7.23 through the oracle cache (no source read):
`validation/investigations/fsl-pair-header/time_step.py` (every FSL original that writes images),
`time_step_cases.py` (the permanent cases), `validation/investigations/cleanroom-topup/jac_fsl273.py`.

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `bet::image` (`read_analyze`, `read_series_lean`; `read`, `read_first` through `larmorx_mri::hmc::image`) | the time step as FSL's image layer reads it (its magnitude; 1 for an image with a time axis whose time step is 0), every output keeping it; `cli::prelude`'s own reading removed | `time_step.py`; `specs/flirt.md` §4.1 |
| `flirt::cost` (`normcorr`) | the count's running total `C` takes each point's weight as it is visited (`larmorx_mri::hmc::cost::sample_fold`) | `specs/flirt.md` §6.4 (spec writer, 2026-11-06) |
| `cli::applytopup` (`jac_scales`) | `--method=jac`'s Jacobian scales: 1 by default (the Jacobian of the displacement applied), FSL's `dx / fx`, `dy / fy` (a field image's `dx / fx`, `dx / fy`) with `reproduce`; FSL-273 and FSL-134 named where they change it | `specs/topup.md` §7.3 (spec writer, 2026-11-06); `jac_fsl273.py` |
| `prelude::merge`, `prelude::volume`, `cli::prelude` (`compute`) | the same results with less memory: the slots before the sums' map is freed, the lists at their exact sizes, no copy of the source's mask, the unwrapped phase allocated after the merging, the phase copied only where the other rule's result can be needed | `specs/prelude.md` §10 |
