# `mri_robust_template` (FreeSurfer 8.2.0): behaviour spec

Status: queue items 3.2-14 and 3.2-15 (track C relays 50 and 51): the command line, the loading of
the inputs, the registrations to the initial target, the rigid and the affine mean space (§4.4), the
global iterations, the averages and every output, the exit statuses, the defects with their switches
and the validation plan (§9): the whole program. Written by the replica's author as the spec writer: behaviour, conventions and messages, for
the clean room (`larmorx-fs-volumes`), which works from this file, `specs/mri_robust_register.md` (the
pairwise registration's messages and the library it shares), the library modules' specs
(`specs/freesurfer-robust.md` for the registration, `freesurfer-volume.md` for the transforms and the
resampling, `freesurfer-bspline.md` for the cubic B-splines, `freesurfer-io.md` for the volumes,
`freesurfer-core.md` for the version options, the help renderer and error messages) and black-box runs
of the official program only.

Conventions: as `specs/mri_robust_register.md`'s. A **number printed as C++ prints it** is
`std::ostream`'s default (`%g` with 6 significant digits); `<b>` a boolean as `true` or `false`; a
matrix or vector in VNL's long format is `specs/freesurfer-robust.md` §1's (`name = [ ... ]` and an
empty line). An **error** is FreeSurfer's error message; `ErrorExit(code)` ends with `code`'s status
(`ERROR_NOFILE` 255, `ERROR_BADFILE` 251, `ERROR_OUT_OF_BOUNDS` 250).

## 1. The command line

1. First, always: `8.2.0` and an empty line on standard output.
2. The version options (`specs/freesurfer-core.md`; recon-all's `-all-info`): when one is given and
   nothing else, status 0.
3. No argument at all: the help (`mri_robust_template.help.xml`, rendered as the core's help), status 1.
4. The options, while the next argument starts with `-` (an empty argument or a word without a dash
   stops them; the rest is ignored). One dash or two are dropped and the rest compared in upper case
   (ASCII). An option taking one argument takes the next one whatever it is. A **list option** takes
   the arguments after it up to the next one that starts with `-` (none: the list is left as it is,
   no error); a list option given twice adds to its list. Each option prints its line on standard
   output (`<v>` the argument as given; `<n>` read with C's `atoi`, `<x>` with `atof`, printed as C++
   prints them):
   - `--mov` (list): one line per name, `--mov: Using <name> as movable/source volume.`, then
     `    Total: <count> input volumes`.
   - `--template <v>`: `--template: Using <v> as template output volume.`
   - `--lta` (list): `--lta: Will output LTA transforms`; `--masks` (list): `--masks: Will use masks on
     inputs.`; `--iscaleout` (list, sets `--iscale`): `--iscaleout: Will perform intensity scaling and
     output results`; `--iscalein` (list): `--iscalein: Will use init intensity scales`; `--ixforms`
     (list): `--ixforms: Will use init XFORMS.`; `--weights` (list): `--weights: Will output weights in
     target space`; `--warp` or `--mapmov` (list): `--mapmov: Will save mapped movables/sources!`;
     `--mapmovhdr` (list): `--mapmovhdr: Will save header-adjusted movables!`
   - `--average <n>`: `--average: Using method <n> for template computation.` (0 mean, 1 median, the
     default; 2 and 3 Tukey's biweight, §4.6)
   - `--vox2vox`: `--vox2vox: Output transforms as VOX2VOX. `; `--affine`, `-a`: `--affine: Enabling
     affine transform!`; `--iscale`, `-i`: `--iscale: Enabling intensity scaling!`; `--transonly`:
     `--transonly: Using only translation!`; `--iscaleonly` (sets `--iscale`, clears `--transonly` and
     `--affine`): `--iscaleonly: Computing only global scaling!`; `--nomulti`: `--nomulti: process highest
     resolution only!`; `--leastsquares`, `-l`: `--leastsquares: Using standard least squares
     (non-robust)!`
   - `--maxit <n>`: `--maxit: Performing maximal <n> iterations on each resolution.` (the global
     iterations' count, default 5; `-1` means 6); `--highit <n>`: `--highit: Performing maximal <n>
     iterations on highest resolution`; `--epsit <x>`: `--epsit: Stop iterations when change is less
     than <x> . ` (the global iterations' threshold, default -1: 0.03); `--pairmaxit <n>`:
     `--pairmaxit: Performing maximal <n> iterations on each resolution for individual pairwise
     registrations.` (default 5); `--pairepsit <x>`: `--pairepsit: Stop individual pairwise iterations
     when change is less than <x> . ` (default 0.01)
   - `--sat <x>`: `--sat: Using saturation <x> in M-estimator!`; `--satit`: `--satit: Will estimate SAT
     iteratively!`
   - `--subsample <n>`: `--subsample: Will subsample if size is larger than <n> on all axes!` when 0
     or more, else `--subsample -1: Will not subsample on any scale!`
   - `--debug`: `--debug: Will output debug info and files!` (§6); `--noit`: `--noit: Will output only
     first template (no iterations)!`; `--fixtp`: `--fixtp: Will map everything to init TP!`;
     `--allow-diff-vox-size`: `--allow-diff-vox-size ` (a trailing blank); `--doubleprec`:
     `--doubleprec: Will perform algorithm with double precision (higher mem usage)!`
   - `--test <n> <image>`: `--test: TEST-MODE `, then the registration library's test mode
     (`specs/mri_robust_register.md` §2.4) and status 0.
   - `--conform <v>`: `--conform: Will output conform template (256^3 and 1mm voxels)!` (§5, FS-1091)
   - `--floattype`: `--floattype: Use float images internally (independent of input)!`;
     `--finalnearest`: `--finalnearest: Use nearest neighbor interpolation for final average!`
   - `--inittp <n>`: `--inittp 0: No initialization, construct first mean from original TPs` for 0,
     else `--inittp: Using TP <n> as target for initialization` (a negative number is no choice: the
     random target, §3)
   - `--oneminusw`: `--oneminusw: Will output 1-weights (zero=outlier), as in earlier versions!` (the
     weights are written as they are; without it as `1 - w`)
   - `--seed <n>`: `--seed: Will use random seed <s>` (`<s>` the number as an unsigned 32-bit integer)
   - `--cras`: `--cras: Will center template at avgerage CRAS!`
   - `--help`, `--usage`, `-h`, `-u`: the help, status 0.
   - `--res-thresh <x>`: `--res-thresh: Voxsize threshold <x>` (`<x>` as a single-precision number);
     `--frobnorm-thresh <x>`: `--frobnorm-thresh: Matrix frobenius norm threshold <x>`
   - `--threads <n>`: no line (the registrations' threads; §7).
   - Anything else: on standard error an empty line, an empty line, `ERROR: Option <arg> unknown !!! `
     (`<arg>` with its dashes, the rest in upper case), an empty line, an empty line; the help; status 1.
5. The checks, in order, each on standard error with status 1 (no help):
   - no `--mov`: `ERROR: Specify at least 2 inputs with --mov (or one 4D input)!`
   - no `--template`: `ERROR: Specify output image with --template !`
   - `--warp` given with another count than `--mov`: `ERROR: Number of filenames for --warp should
     equal number of inputs!`; the same for `--mapmovhdr`, `--lta`, `--ixforms`, `--weights` (each its
     own option in the message); `--iscalein` with another count than `--ixforms`: `ERROR: Number of
     filenames for --iscalein should agree with number of init LTAs (--ixforms)!`; `--iscaleout` with
     another count than `--mov`: `ERROR: Number of filenames for --iscaleout should equal number of
     inputs!`
   - neither `--satit` nor a `--sat` above 0, unless `--noit` with `--ixforms`: `ERROR: Specify either
     --satit or set sensitivity manually with --sat <real> !`

## 2. The inputs

1. `Setting iscale ...` when the intensity scaling is on, `Setting iscaleonly ...` for `--iscaleonly`.
2. **Several names** (`--mov` of two or more): for each, `reading source '<name>'...`, the volume read
   (not read: `MultiRegistration::loadMovables: could not open input volume <name>.`, `ERROR_NOFILE`;
   more than one frame: `MultiRegistration::loadMovables: only pass single frame MRI <name>.`,
   `ERROR_NOFILE`); with `--masks`: `masking source frame <i> ...` (`<i>` from 0), the mask read (not
   read: `... could not open mask volume <name>.`; more than one frame: `... only pass single frame mask
   <name>.`, both `ERROR_NOFILE`) and the input's voxels where the mask is not above 0 set to 0; then
   `converting source '<name>' to bspline ...` and the input's cubic B-spline coefficients
   (`specs/freesurfer-bspline.md`: `MRItoBSpline degree 3`). Then each input's voxel sizes against the
   first's: a difference above `--res-thresh` (single precision, default 0.01) in any axis gives on
   standard error `ERROR: MultiRegistration::loadMovables: images have different voxel sizes.`, `
   Currently not supported, maybe first make conform?`, `  Debug info: size(<i>) = <x>, <y>, <z>
   size(0) = <x>, <y>, <z>` and the error `MultiRegistration::loadMovables: voxel size is different
   <name>.` (`ERROR_BADFILE`); with `--allow-diff-vox-size` on standard output `WARNING:
   MultiRegistration::loadMovables: images have different voxel sizes.`, `  mov[0] = (<x>,<y>,<z>)
   mov[<i>] = (<x>,<y>,<z>)` (`%g`) and `  BUT AllowDiffVoxSize set so continuing ...`
3. **One name**: `trying to read 4D source '<name>'...`, the volume read (not read: the error above),
   ` Input has <n> frames.`; one frame: on standard error `ERROR: only single 3D volume passed!` and
   `    Pass several inputs, or 4D input volume`, status 1. With one mask: the mask read (its frame count
   other than the input's: `ERROR: frame count in mask does not agree with frames in mov!`, status 1).
   Each frame becomes an input (its name `Frame_<i>`): `masking source frame <i> ...` with the mask's
   frame, `converting source frame <i> to bspline ...`. Without a mask, the program then prints the
   error `MRIfree: null pointer` on standard error (FreeSurfer frees the absent mask; a message only,
   kept in both modes).
4. Fewer than two inputs after this: on standard error `Could not load movables!`, status 1 (not
   reached).
5. `--ixforms`: each transform read (an LTA, a MINC `.xfm`, ...; `identity.nofile`: the identity with
   the input's geometry as source and target); a transform without a source or target geometry: the
   error `MultiRegistration::loadLTAs no source geometry, use lta with valid geometry ( <name> )` (or
   `no target geometry`), `ERROR_BADFILE`; each made voxel to voxel. `--iscalein`: each file's first
   number as C++'s `>>` reads it (an empty file or a folder keeps 1; a word gives 0); a file that does
   not open: the error `MultiRegistration::loadIntensities no such file ( <name> )`, `ERROR_BADFILE`.

## 3. The initial target

Without `--inittp` (or a negative one) and without `--ixforms`: the seed is `--seed`'s, or the images'
(the absolute values of each input's voxels on the three axes through its centre voxel, `(width / 2,
height / 2, depth / 2)`, up to 19 voxels each way, the centre itself six times, summed in double; 0
prints `WARNING: image seed is zero, are images really empty at center?`, otherwise the sum is
multiplied by 10 until it is at least 10; then truncated to an unsigned integer); then the C library's
`srand(seed)` and `rand() % m + 1` for `m` the number of names given to `--mov` (glibc 2.35's generator,
`specs/glibc-random.md`); `Will use TP <t> as random initial target (seed <s> ).` FS-1082, FS-1083,
FS-1097.

## 4. The template

### 4.1 The paths
- `--noit`, or exactly two inputs: the initial registrations (§4.2; unless `--ixforms`, or the initial
  target 0), then the template with the final sample type (§4.5).
- More inputs: the initial registrations the same way, then the global iterations (§4.3) with
  `--maxit` and `--epsit` and the pairwise `--pairmaxit` and `--pairepsit`; when `--finalnearest`
  changed the sample type, the template again (§4.5).

### 4.2 The registrations to the initial target (`t`, from 1)
1. An empty line, `MultiRegistration::initializing Xforms (init <t> , maxres 0 , iterate 5 , epsit 0.01
   ) : `; with `--debug` the settings (§6).
2. Each other input `j` in order (the inputs' order with the target and the first swapped: the target
   is index 0, the first input takes the target's place): an empty line, `[init] =========================
   TP <j> to TP <t> ==============================`, `         Register TP <j> ( <name> )`, `
   to      TP <t> ( <name> )`, an empty line; then the robust registration of input `j` onto the target
   (`specs/mri_robust_register.md` §2 and `specs/freesurfer-robust.md`): rigid (affine with
   `--affine`), the intensity scale with `--iscale`, translation only, intensity only, the saturation of
   `--sat`, double precision, `--subsample` (when above 0) and `--highit` (when 0 or more), the images'
   types kept unless `--floattype`, verbose 0 (1 with `--debug`); `--satit`: the saturation search
   first; `--nomulti`: the iterations on the full images, else the multiresolution registration to the
   finest level; 5 iterations, 0.01. Its final voxel transform and intensity factor; its centroid
   mapped into the target, plus (for the first registration only) the target's centroid: summed over
   the registrations into the centroid.
3. The sum divided by the number of inputs (times `1 / n`). For each registration that did not
   converge: `* WARNING: TP <j> to <t> did not converge !! `.
4. Each input's transform: voxel to voxel onto the target's geometry (the target's own the identity),
   its intensity factor.
5. `--fixtp`: done (the template is in the target's space).
6. **The mean space** (rigid): the intensity factors divided by their geometric mean (the product's
   `1 / n`-th power, `pow`: the correctly rounded `pow` by default, glibc's with `reproduce`); each
   other input's transform made RAS to RAS and inverted; the mean of their translations and (with the
   identity of the target) of their rotations: `Input rotation's max deviation from rotation is: <d>`
   (the largest Frobenius norm of `R R^T - I`); above `--frobnorm-thresh` (default 0.0001) on standard
   error ` MyMatrix::RotationMean input(s) not rotation`, on standard output ` The threshold is: <x>.
   This can be adjusted using --frobnorm-thresh <>.`, status 1. The mean rotation: the mean of the
   rotation vectors (each matrix's quaternion, its rotation axis times its angle `2 acos(a / |q|)`)
   made a rotation again. The template's geometry: isotropic voxels of the largest of the inputs'
   smallest sides; each side the largest over the inputs of `ceil(extent in mm / size - 0.0001)`; 2-D
   and 3-D inputs together: on standard error ` ERROR (createTemplateGeo) : mixing 2d and 3d input
   images not possible!`, status 1; the direction cosines the first input's when all agree within 1e-9,
   otherwise (` Cosines of input transforms not identical !` once per differing element) each input's
   axes reordered and flipped to the first's and the mean made a rotation by the polar decomposition
   (a reordering that is no permutation: `WARNING: reorder not clear ...`, the matrix, ` xd: <x> yd:
   <y> zd: <z>` and, for a negative determinant, `ERROR: vox2vox det: <d> < 0` and two lines, status
   1); the centre the mean of the inputs' centres, or (without `--cras`) the centroid of §4.2 mapped to
   RAS and by the mean transform. Then for each input: `  computing mean coord of TP <j> ( <name> ) `,
   the mean transform times its RAS transform made a rotation again (the polar decomposition: a
   reflection is the error `Internal Error:  produced reflection.`; a scale above 0.0001 prints on
   standard error `Internal Error for tp <j-1> -> template`, ` Rotation should not scale ( <x> )`,
   ` Debug Info: `, the matrices `A`, `R`, `S`, ` Make sure input voxel sizes are identical for all
   images!` and the error `Internal Error: Rotation should not scale.`, both `ERROR_OUT_OF_BOUNDS`),
   `   mapping back to rot, err = <x>`, its transform onto the template's geometry, voxel to voxel.

### 4.3 The global iterations
`Computing first template`; without transforms (the initial target 0): ` MultiRegistration::
initialAverageSet `, `    -- aligning image centroids (translation)` (each input's centroid in RAS,
their mean, a template of the first input's geometry centred there, each input's translation onto
it; with `--iscaleonly` `    -- averaging without alignment (iscale only)`: the identity onto the
first) and the template (§4.5); otherwise the template from the transforms. Then while the iteration
count is below `--maxit` and the largest change above `--epsit`: the header (`=====...` lines and
`Working on global iteration : <k>`; without initial transforms the first iteration runs four times,
coarser: `  noxformits = <m>`), each input `Working on TP <i>`, its registration onto the template from
its current transform and intensity (` - running multi-resolutional registration on TP <i>...`, or `
- running high-res registration on TP <i>...` with `--nomulti` or `--iscaleonly`), `   *** WARNING: TP
<i> to template did not converge ***`, `   tp <i> distance: <d>` (the RMS distance of the old and new
RAS transforms for a radius of 100 mm), ` - mapping tp <i> to template (cubic bspline) ...` (or
`...`), ` - adjusting intensity of mapped tp <i> by <s>`, ` - backup weights tp <i> ...` (with
`--weights`), `Finished TP : <i>` and a line of `=`; then `Global Iteration <k> Distances: `, the
distances, `Maxchange: <m>`; `Computing new template <k>`; at the end ` DONE : computeTemplate `.

### 4.4 The affine mean space
`--affine` without `--fixtp`: §4.2's step 6 with the linear maps (each other input's RAS transform
inverted, its upper 3 x 3 block; the target's the identity) averaged by their **geometric mean**
`exp((1/n) sum_i log(L_i))` (`n` the number of inputs, the logarithms summed in input order, then
each element times `1/n`) instead of the mean rotation; no rotation check and no polar decomposition
afterwards (the mean transform times each input's RAS transform is kept as it is). The matrix
logarithm and exponential are those of MATLAB's `logm` and `expm` (Higham's methods), in double
precision with C++'s complex arithmetic (a complex product and quotient as C99 computes them, Annex
G's recoveries included; `abs`, `sqrt` and `log` of a complex number the project's decided `cabs`,
`csqrt` and `clog`: `larmorx_core::cplx`; the complex Frobenius norm's squares `abs(z)^2`):

1. **Logarithm of a 3 x 3 real `A`.** The complex Schur form `A = U T U^H` (`specs/freesurfer-robust.md`'s
   `SchurComplex`: reference LAPACK 3.10.0's `ZGEES`). For each diagonal element of `T` with real part
   at most 0 and imaginary part below `1e-9` in magnitude, on standard error `MatrixLog Error: `, `
   Principal matrix logarithm is not defined for A with `, `          nonpositive real eigenvalues. `
   (FS-1099: by default then status 1; `reproduce` goes on). The diagonal elements grouped: each new
   element starts a group, two elements of different groups within `0.1` (the absolute value of their
   difference) merge their groups (the larger group number into the smaller, the numbers above it
   lowered); the groups ordered by the mean of their elements' positions (ties by group number), and
   the Schur form reordered so each group's elements are consecutive (LAPACK's `ZTRSEN` with `JOB =
   'N'`, `COMPQ = 'V'`, called for each group from the last in the order to the first whose elements
   are not already at the front, each time selecting that group's current positions). Then `F`, upper
   triangular: each diagonal block (a group) by (2); above the diagonal, for each block column from
   the left and each block row from the one above it up: between two single elements `F(i, j) =
   (T(i, j) (F(i, i) - F(j, j)) + sum_k F(i, k) T(k, j) - sum_k T(i, k) F(k, j)) / (T(i, i) - T(j,
   j))` (`k` strictly between `i` and `j`, both sums from 0 in `k` order); otherwise the block solves
   `T_ii X + X N = F_ii T_ij - T_ij F_jj + F_ik T_kj - T_ik F_kj` (`N = -T_jj`, each element
   negated; `k` the blocks between; the right-hand side's products and sums in that order) column by
   column: column `c` from `(T_ii + N(c, c) I) x = rhs(:, c) - X(:, 0:c) N(0:c, c)` (the matrix-vector
   product's sums from 0 in column order), each system by (4); FS-1100: FreeSurfer leaves the last
   solved column out of that product (`X(:, 0:c-1) N(0:c-1, c)`), `reproduce` does as it does. Then `U F U^H`. If the one-norm of its imaginary part
   passes `10 n 2^-52` times its own one-norm: on standard error ` MatrixLog Error: `, `  Result too
   imaginary to ignore! ( <x> )` (`%g`), then at precision 16 ` A = ` and `A` (each element and a
   blank, each row a newline) and an empty line, ` T = ` and `T` (complex elements `(re,im)`), ` U = `
   and `U` likewise, ` fro( real(U * T * U^*) - A) = <x>` and two newlines, ` log(A) = ` and the complex
   result, ` fro( exp(real(log(A))) - A)  = <x>` and two newlines, status 1 (FS-1312: by default only
   above `sqrt(2^-52)` times the norm). The result is the real part.
2. **Logarithm of a triangular block.** A single element: its complex `log`. Otherwise inverse scaling
   and squaring: while the Frobenius norm of `T - I` is above `0.25`, the upper triangular square root
   (column by column, the diagonal `sqrt(T(j, j))`, then from row `j - 1` up `R(i, j) = (T(i, j) - sum_k
   R(i, k) R(k, j)) / (R(i, i) + R(j, j))`) replaces `T`; the 99th root still above ends with
   `MatrixLog_isst:tooManyIterations Too many square roots.` on standard error, status 1. Then `2^s`
   times (3) of `T - I` (`s` the number of roots).
3. **`log(I + X)`**: the [8/8] Pade approximant in partial fractions, `sum_j w_j X (I + x_j X)^-1` over
   the 8-point Gauss-Legendre nodes `x_j` and weights `w_j` mapped to `[0, 1]` (`(x + 1) / 2`, `w /
   2`; the nodes and weights by Golub and Welsch: the eigenvalues, ascending, of the symmetric
   tridiagonal matrix with `i / sqrt(4 i^2 - 1)` beside its diagonal and `2 v(0)^2` of each
   eigenvector, VNL's symmetric eigensystem); each term solved as `(I + x_j X^T) Y = X^T` by (4), the
   terms summed in order, the sum transposed.
4. **The complex solves** are VNL's `vnl_svd<std::complex<double>>`: LINPACK's `zsvdc` (VNL's f2c
   translation; `job = 21`), the singular values' absolute values, then for a matrix right-hand side
   `V (diag(1/w) (U^H B))` (`1/w` C++'s complex quotient `(1, 0) / (w, 0)`, the row times it; 0 rows
   kept for `w = 0`), for a vector `V (U^H y ./ w)`.
5. **Exponential** (`expm`, Higham 2005): `m` the one-norm of the matrix; up to `5.371920351148152` the
   Pade approximant of the first degree 3, 5, 7, 9 or 13 whose `theta` (`1.495585217958292e-2`,
   `2.539398330063230e-1`, `9.504178996162932e-1`, `2.097847961257068`, `5.371920351148152`) it does
   not pass; otherwise `s` the exponent of `m / theta13` (`frexp`, one less when the fraction is
   exactly 1/2), the degree-13 approximant of the matrix times `2^-s`, squared `s` times. The
   approximant of degree 3 to 9: the powers `I, A^2, A^4, ...` (each the previous times `A^2`), `U = A
   sum_{j odd, from the degree down} c_j A^(j-1)`, `V = sum_{j even, from the degree minus 1 down} c_j
   A^j` (each running sum from zeros, each term `c_j` times the power element by element); of degree
   13 Higham's grouping `U = A (A6 (c13 A6 + c11 A4 + c9 A2) + c7 A6 + c5 A4 + c3 A2 + c1 I)`, `V = A6
   (c12 A6 + c10 A4 + c8 A2) + c6 A6 + c4 A4 + c2 A2 + c0 I` (left to right); then `(V - U)^-1 (U + V)`
   by VNL's double `vnl_svd` solve (`-U + V`, `U + V`; its warning when LINPACK does not converge). The
   coefficients are the numerators of the `[m/m]` Pade approximants of `exp` (3: 120, 60, 12, 1; 5:
   30240, 15120, 3360, 420, 30, 1; 7: 17297280, 8648640, 1995840, 277200, 25200, 1512, 56, 1; 9:
   17643225600, 8821612800, 2075673600, 302702400, 30270240, 2162160, 110880, 3960, 90, 1; 13:
   64764752532480000, 32382376266240000, 7771770303897600, 1187353796428800, 129060195264000,
   10559470521600, 670442572800, 33522128640, 1323241920, 40840800, 960960, 16380, 182, 1). FS-1311:
   FreeSurfer's official build computes with numbers from memory it does not own instead (NaN or
   arbitrary results): both modes use these, with the notice.
6. With `--debug` the mean matrix is printed as for the rigid mean (`meanr`, its determinant and its
   decomposition into `Rot`, `Shear`, `Scale`).

### 4.5 The template
`mapping movs and creating initial template...` (` allow intensity scaling` with `--iscale`); each input
mapped by its transform onto its target's geometry (cubic B-spline coefficients, the input's type;
`--finalnearest`: nearest neighbour), its pulse parameters copied, scaled by its intensity factor
(§4.2's or the iterations'); `--iscaleonly`: `creating intitial template (iscale only)` and each input
only scaled. Then the average (§4.6) and `       -- Template : (<x>, <y>, <z>)mm and (<w>, <h>, <d>)
voxels.` (`%g`).

### 4.6 The averages
`--average 0`: `    using mean` (the running mean, voxel by voxel, in double, stored as the first
image's type); `1`: ` using median ` (the median of the images' values at each voxel; for an even count
the mean of the two middle values in single precision); `2`: `    using tukey biweight` (each voxel's
robust location; FS-1065); `3`: `    using tukey biweight (alltoghether)` (one robust regression of every
voxel at once; FS-1095); another: on standard error ` averageSet  method <m> unknown`, status 1.

## 5. The outputs
`Writing final template: <name>` and the template written (a write that fails prints FreeSurfer's
writer error; FS-1096). `--conform <name>` (FS-1091): `warping movs and creating conform template...`,
for each input on standard error ` MultiRegistration::averageConformSet not implemented, yet !!!`, the
template of the inputs mapped trilinearly into the template's space, written. `Writing final transforms
(warps etc.)...`; `--lta`: each transform RAS to RAS (voxel to voxel with `--vox2vox`), its file names
the template's and the input's, written as an LTA, ` Determinant( lta[ <i> ]) : <d>` and an empty line
(the voxel transform's determinant; with `--affine` also ` Decompose into Rot * Shear * Scale : `, an
empty line and the matrices `Rot`, `Shear`, `Scale`); `--mapmov`: the mapped inputs (one name: `
Writing single output (4D volume) ...` and one 4-D image, FS-1090); `--mapmovhdr`: each input with its
voxel-to-RAS matrix moved by its transform (a write that fails: `ERROR: Can't write <name>` and the
rest skipped); `--iscaleout`: each intensity factor as C++ prints it, no newline; `--weights` (not with
`--leastsquares`): the global iterations' weights in the template's space (as `1 - w` unless
`--oneminusw`), ` No weights constructed, skipping output of weights` without iterations. Then
`registration took <m> minutes and <s> seconds.` (the clock), an empty line and the citation (`
Thank you for using RobustTemplate! `, ` If you find it useful and use it for a publication, please
cite: `, an empty line, ` Within-Subject Template Estimation for Unbiased Longitudinal Image Analysis`,
` M. Reuter, N.J. Schmansky, H.D. Rosas, B. Fischl. NeuroImage 2012.`, `
http://dx.doi.org/10.1016/j.neuroimage.2012.02.084`, ` http://reuter.mit.edu/papers/reuter-long12.pdf`,
an empty line). Status 0.

## 6. The debug output (`--debug`)
The settings before the initial registrations (` MultiRegistration Parameters `, then ` Outdir: `, `
TransOnly: `, ` Rigid: `, ` Robust: `, ` Satit: ` or ` Sat: `, ` Iscale: `, ` IscaleOnly: `, ` NoMulti:
`, ` SubsampleSize: `, ` HighIt: `, ` FixVoxel: `, ` KeepType: `, ` Average: `, ` DoublePrec: `, `
BackupWeights: `, ` SampleType: `, ` CRASCenter: `, ` Resthresh: `, ` Frobnormthresh: `, ` Debug: `,
aligned as the official program aligns them, empty lines around); the registrations at verbose 1; the
centroids (`CentroidT`, `CentroidSinT`, `Centroid`), `meant`, `meanr`, its determinant and
decomposition; the files, in the debug folder (FS-1098): `tp<j>_to_mcoord.lta`, `.mgz`,
`-intensity.txt`; `tp<i>_to_template-it<k>.mgz`, `.lta`, `-intensity.txt`, `-weights.mgz`, `-www.mgz`
(with the half-way maps printed); `template-it<k>.mgz` (`debug: saving template-it0.mgz`, `debug:
writing template to <name>`).

## 7. Exit statuses and threads
0; 1 (the checks, the voxel sizes' and frames' errors, the rotation threshold, an unknown average);
255, 251, 250 (`ErrorExit`); with the defects' switches below. The official build runs the
registrations of §4.2 and §4.3 on OpenMP threads (`--threads`, `OMP_NUM_THREADS`; FS-1081): the replica
and the clean room give the one-thread results.

## 8. The defects (`larmorx_core::defects`)
FS-1080 (a missing argument: a crash; error by default), FS-1081 (threads: the one-thread order), FS-1082
(a seed beyond 32 bits: the largest by default), FS-1083 (the clock's seed: 0 by default), FS-1084
(fewer masks: an error in both modes), FS-1085 and FS-1086 (a 4-D input's masks: errors by default),
FS-1087, FS-1088, FS-1089 (`--ixforms`: an unread file, a morph, too few for a 4-D input), FS-1090 (the
4-D `--mapmov`: the template's header by default), FS-1091 (`--conform`: kept), FS-1092 (`--inittp`
past the inputs: an error), FS-1093 (no transforms: an error), FS-1094 (weights with `--leastsquares`:
none kept), FS-1095 (`--average 3` above 2,048 voxels: an error), FS-1096 (a failed write: status 1
by default), FS-1097 (a 4-D input's random target: frame 1, kept), FS-1098 (the debug folder: the
template's folder by default), FS-1099 (a nonpositive real eigenvalue in the affine mean space: status 1
by default), FS-1100 (the logarithm's coupling equations: correct by default), FS-1311 (the
exponential's numbers from memory FreeSurfer does not own: the method's in both modes, noted),
FS-1312 (the logarithm's imaginary bound: `sqrt(2^-52)` by default). Each text is in
`larmorx_core::defects`.

## 9. Validation plan (for the clean room)
- The probe `validation/probes/fs-robust-template/` runs the replica (closed); the clean room checks
  against the official program as a black box with the same kinds of cases: the command line's paths
  and failures, two to four small head-like images of every type, extent and orientation, 4-D inputs,
  masks, initial transforms and intensity scales, every option and output; the official program at one
  thread with the decided `cabs` and `csqrt` preloaded (the user's D4: the matrix square roots), the
  correctly rounded `acos` and the decided `clog` (§4.4).
- The affine mean space (§4.4): its own cases (`--affine` without `--fixtp`, inputs with rotated and
  scaled headers: single and separate eigenvalue groups); the official program's results there are
  memory contents (FS-1311), so the reference is the official sources rebuilt with the exponential's
  coefficients defined (the probe's `rebuild.sh`, first checked to give the official program's
  outputs on the other cases). The matrix functions alone (logarithm, exponential, geometric mean) are
  compared with the official code case by case (fs-robust's probe, `--family logm`).
- fMRIPrep's recorded calls (ds004173's three T1w, ds005454's two) and recon-all's motion correction on
  them, and sMRIPrep's longitudinal call, byte for byte.
