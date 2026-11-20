# FreeSurfer 8.2.0's robust registration library: behaviour spec (`fs.robust`), part 1

Written by the replica author of queue item 3.2-02 (track C, relay 40, 2026-11-18) as its spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (queue C3.2-02, in `larmorx-fs-core`) from this file, black-box runs
of the official programs, `specs/freesurfer-mri.md` (the volume structure, `MRIgetVoxVal`,
`MRIsetVoxVal`, the allocation and header copy, the voxel-to-RAS matrix), `specs/freesurfer-maths.md`
(the `float` matrix and its product), `specs/freesurfer-volume.md` (`MRIclone`, `MRIcopyFrame`,
`MRIlinearTransform`), `specs/freesurfer-filter.md` (`MRIconvolve1d`, `MRIconvolveGaussian`) and the
main package's open VNL code (`larmorx_core::vnl_matrix`, `vnl_svd`, `vnl_eigen`, `vnl_fixed`: VXL's
arithmetic as ITK 4.13.2 computes it).

**Whoever implements it must not open:** FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`, any
other copy), the replica crates (`crates-freesurfer/`, among them `larmorx-freesurfer-robust`), the
documents written from FreeSurfer's source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the
replica's probes (`validation/probes/fs-*`, among them `fs-robust`), the unit oracle's build tree and
tools, `docs/parts/` and the scratch folders of FreeSurfer's parts. **It may read** this spec, the
FreeSurfer specs it cites, the main package's open code and documentation, IEEE 754-2008, C11, and run
as black boxes the official FreeSurfer 8.2.0 programs (§8).

Status: **part 1**, the helpers both `mri_robust_register` and `mri_robust_template` reach (the image
helpers of §2, the moments of §3, the matrix helpers of §4), with **part 2's first piece**, the complex
Schur form and the matrix square root (§4.5, §4.6; relay 41, 2026-11-20). The rest of part 2 (the
registration itself, its transformations, steps and robust regression) follows with queue item 3.2-03. What black-box runs confirmed is marked **(observed)**.

## 1. Conventions

- Those of `specs/freesurfer-mri.md` §1: x86-64 SSE2 arithmetic in IEEE `double` and `float` (no fused
  multiply-add, no extended precision), every intermediate rounded to its C type; a NaN made from
  numbers is the default NaN with the sign bit set (`0xfff8000000000000`, `0xffc00000`); a NaN operand is
  passed on quieted with its sign and payload; of two NaN operands the first's (the left operand of the
  expressions written below, unless said otherwise). `float` to `double` quiets a signalling NaN;
  `double` to `float` rounds to nearest.
- A **4 x 4 transform** is 16 `double`s row by row; a **VNL matrix** is `rows x cols` `double`s row by
  row. Its product `A B` is, for each element, `sum = +0; sum = sum + a(i, j) * b(j, k)` for `j` in
  order (the sum the first operand); `A - B` element by element; `A^T` the transpose; `|A|_F` the
  square root of `sum = +0; sum = sum + x * x` over the elements in storage order; `A D` for a diagonal
  `D` scales column `j` by `d_j` (`a(i, j) * d_j`). These are `larmorx_core::vnl_matrix`.
- "Ends the program (message, status 1)": the message and a newline on standard error, then `exit(1)`.
  "An error": FreeSurfer's error printer (`specs/freesurfer-core.md`) on standard error.
- `(int) x` of a `double` beyond the `int` range or NaN: `INT_MIN` on x86-64 (C leaves it undefined;
  the project's default saturates, NaN 0, with a notice, FS-17). `nint(x)`: `(int)(x + 0.5)`, or
  `(int)(x - 0.5)` below 0 (FS-12 likewise).
- `acos`: correctly rounded in both modes (the project's decision for glibc's `acos`, which is not:
  GLIBC-07).

## 2. The image helpers (`MyMRI`)

### 2.1 `MRIvalscale(src, dst, s, b)` (`b` defaults to 0)

Without a destination, a clone of the source (`MRIclone`: its extent, type and frames, voxels 0, its
header). For each frame, slice, row and column in that order, the source's stored value `v` (as `float`)
becomes `val = (float)((double)v - b)`, then `val = (float)((double)val * s)`, stored in the destination
at the same position, read as the source's type:

- `float`: `val` itself;
- `short`: `val` clipped to `[-32768, 32767]` (`val < -32768` then `val > 32767`, as two comparisons: a
  NaN passes both), then `(short) nint(val)`;
- `unsigned short`: clipped to `[0, 65535]` likewise, then `(unsigned short) nint(val)`;
- `uchar`: clipped **only above**, at 255; then `nint(val)` converted to `uchar` modulo 256 (a negative
  result wraps: -3 is stored as 253).

Any other source type: the error `MRIvalScale: unsupported type <type>` (`ERROR_UNSUPPORTED`) and the
destination returned as it is (the fresh clone when none was given). Returns the destination. The source
may be the destination (every voxel is read before it is written). A destination of another type or
extent than the source is written as the source's type with the source's extent (no caller gives one).

### 2.2 The kernels

`getPrefilter()`: a `float` volume of 5 x 1 x 1 voxels, `0.03504 0.24878 0.43234 0.24878 0.03504` (the
`float`s nearest to these decimals); `getDerfilter()`: `-0.10689 -0.28461 0 0.28461 0.10689`. Both are
fresh volumes (`MRIalloc`, the default header).

### 2.3 `getRand(randpos)`

A fixed table of 101 `double`s (below, row by row). A position above 100 becomes `randpos % 101` (and is
given back so: the caller's position changes); then the entry at the position is returned. No caller
passes a negative position.

```
0.825960894329359 0.391799656122844 0.606822280998442 0.923192169644637 0.772662867376799
0.394913419972908 0.562648884384293 0.821213453107628 0.077777173551299 0.536306917657496
0.514107239245956 0.761227511728186 0.978425890910315 0.157030594951506 0.147496203406248
0.783403101626463 0.637363106003275 0.257349279177570 0.109403726553149 0.136954287985675
0.654171911591805 0.204476823923606 0.164700938803686 0.566376934840801 0.854095286894540
0.430581644078462 0.397193805113614 0.045536084142315 0.140620297119782 0.724179303888238
0.210184975302471 0.346103835004574 0.226683913788046 0.308131967172401 0.451070768412025
0.831843821825446 0.086760750364257 0.854135129404520 0.742590231477529 0.053858310273945
0.122791324135168 0.526967625411493 0.320307444448230 0.520062337463421 0.878893684505441
0.443226585521493 0.320014638636649 0.868171615160687 0.764138797526944 0.864611801159897
0.083244000805187 0.340050247951733 0.581607039757426 0.291315250124114 0.467637373824935
0.596640965216293 0.897387480194169 0.121159463814373 0.018178277226624 0.747666956017887
0.670108792666926 0.092391423049263 0.917528192569663 0.347581829497044 0.857229173664249
0.632564501155682 0.363593396666436 0.385827512659221 0.003355759148503 0.969283884272977
0.459201707491767 0.800715562489883 0.296945204007638 0.619917383791628 0.748749776544515
0.394902341568457 0.354479046607957 0.867388084434840 0.627567204750354 0.984294463747630
0.824472893444168 0.295272747204669 0.927251152590456 0.119266587069099 0.241265513320426
0.527775796652046 0.060203502196487 0.835363100840541 0.148398316485924 0.121273963075904
0.683207184266160 0.500002874704566 0.561265939626174 0.234256657698203 0.486854859925734
0.071141206079346 0.442630693187859 0.327200604299592 0.226827609433358 0.971944076189183
0.302612670611030
```

### 2.4 `subSample(src, dst, fixheader, randpos)` (`dst` none, `fixheader` true, `randpos` 0 by default)

The extent `w = width / 2`, `h = height / 2`, `d = depth / 2` (integer halves), `d = 1` when the source
is one slice deep. Without a destination: `MRIallocSequence(w, h, d, type, nframes)` of the source's type
and frames, then the source's header copied into it (`MRIcopyHeader`; its own extent kept). Then for
`z`, `y`, `x` of that extent in that order: when `randpos >= 0`, `dx = (int)(2 getRand(randpos))`, the
position plus 1, `dy` likewise, and, only when `d > 1`, `dz` likewise (else `dz` stays 0); with `randpos
< 0` all offsets 0. Every frame `f`: `MRIsetVoxVal(dst, x, y, z, f, MRIgetVoxVal(src, 2x + dx, 2y + dy, 2z
+ dz, f))`.

With `fixheader`: `imnr0` the source's, `imnr1 = imnr0 + depth - 1` (the destination's depth); the voxel
sizes, `thick` and `ps` each the source's times 2 (in `float`); the centre `(c_r, c_a, c_s)` the first
three rows of the source's voxel-to-RAS matrix (`extract_i_to_r`, a `float` matrix) times the column
`(width / 2 + 0.5, height / 2 + 0.5, depth / 2 + 0.5, 1)` (integer halves plus 0.5, each stored as a
`float`; the `float` product of `specs/freesurfer-maths.md`); then the cached matrices recomputed
(`MRIreInitCache`). Returns the destination.

### 2.5 `getBlur(src, dst)`

`MRIconvolveGaussian(src, dst, getPrefilter())` (`specs/freesurfer-filter.md`): each axis convolved with
the five taps; `dst` may be the source itself; returns what that returns.

### 2.6 `getPartials(mri)`: `fx`, `fy`, `fz`, `blur`

Four clones of the image (`MRIclone`; no `fz` when the image is one slice deep). With `D` the derivative
kernel and `P` the prefilter, `C_a(K, v, src_frame, dst_frame)` the one-dimensional convolution
`MRIconvolve1d` along axis `a` (width, height, depth) into a new `float` volume (no destination) or into
the given one, for each frame `f`:

1. `mdx = C_width(D, mri, f, 0)`; `mdxby = C_height(P, mdx, 0, 0)`; then `fx` frame `f` is
   `C_depth(P, mdxby, 0, f)` into `fx` (one slice deep: `MRIcopyFrame(mdxby, fx, 0, f)` instead);
2. `mbx = C_width(P, mri, f, 0)`; `mbxdy = C_height(D, mbx, 0, 0)`; `mbxby = C_height(P, mbx, 0, 0)`;
   `fy` frame `f` is `C_depth(P, mbxdy, 0, f)` into `fy` (one slice deep: the frame copy);
3. unless one slice deep: `fz = C_depth(D, mbxby, 0, f)` into `fz`, and `fz` **becomes what it
   returns**: none when the convolution fails (`MRIconvolve1d: unsupported dst pixel format <type>`, a
   destination of type `int`, `long` or `unsigned short`), and for the next frame a new one-frame `float`
   volume (no destination given);
4. `blur` frame `f` is `C_depth(P, mbxby, 0, f)` into `blur` (one slice deep: the frame copy).

Returns true. (The convolutions into a destination of the image's own type keep that type's rules:
`specs/freesurfer-filter.md`.)

### 2.7 `findRightSize(mri, conform_size, conform)`

The physical sizes `size_a = |voxel size_a| * extent_a` (in `double`, the `float` size widened). The
extent per axis `(int) ceil(size_a / conform_size - 0.0001)` (`conform_size` widened to `double`; the
division, then the subtraction), the third 1 when the image is one slice deep. Without `conform`, that.
With `conform`: the largest of the width's and height's sizes (and the depth's when the image is deeper
than one slice), `n = (int) ceil(largest / conform_size)`; `n = 256` when `n < 256`, and also when `(n -
256) / 256 < 0.1`; the extent `(n, n, n)`, the third 1 when one slice deep.

### 2.8 `getBackground(mri)`

Frame 0: `min` and `max` from voxel `(0, 0, 0)`, then over every voxel (slices, rows, columns) `v > max`
and `v < min` (NaN never chosen). `eps = (float)((double)(max - min) / 255.0)` (the difference in
`float`). Then the counts `maxs` of voxels with `max - v < eps` and `mins` with `v - min < eps` (in
`float`). Returns `min` when `mins > maxs`, else `max`.

### 2.9 `MRIlinearTransform(src, dst, M)` (a 4 x 4 transform)

`M` converted to a `float` matrix (§4.1), then FreeSurfer's `MRIlinearTransform(src, dst, M)`
(`specs/freesurfer-volume.md`: trilinear).

## 3. The image moments (`CostFunctions`)

### 3.1 `centroid(mri)`

Over frame 0's voxels (slices `d`, rows `h`, columns `w`), each `v` (`MRIgetVoxVal`, widened) set to 0
when `|v - outside_val| < outside_val / 255` (never for an outside value of 0 or below); `n = n + v`,
`m_x = m_x + (w + 1) v`, `m_y = m_y + (h + 1) v`, `m_z = m_z + (d + 1) v` (all from `+0`, in `double`).
The centroid `(m_x / n, m_y / n, m_z / n)` (1-based voxel coordinates). When `m_x/n + m_y/n + m_z/n` is
NaN (an empty or zero image, a NaN or infinite voxel): ends the program with `CostFunctions::centroid is
NAN (empty image? n = <n> )`, `<n>` as C++'s default stream prints a `double` (`%g`, 6 digits), status 1.

### 3.2 `orientation(mri)`

Frame 0's moments, each voxel `v` a `float`: `n = n + v` (widened), and with `W = w + 1`, `H = h + 1`,
`Z = d + 1` (integers), the `float` products `(float)(K) * v` of the integer `K` (`W`, `H`, `Z`, `W W`,
`H H`, `Z Z`, `W H`, `H Z`, `W Z`, each integer product computed first) widened and added into
`m_100`, `m_010`, `m_001`, `m_200`, `m_020`, `m_002`, `m_110`, `m_011`, `m_101` (from `+0`). Means `x =
m_100 / n`, `y = m_010 / n`, `z = m_001 / n`; the covariance `c_xx = m_200 / n - x x`, `c_yy`, `c_zz`
likewise, `c_xy = m_110 / n - x y`, `c_yz = m_011 / n - y z`, `c_xz = m_101 / n - x z`, mirrored. Its
eigenvalues `D` and eigenvectors `V` (columns) from VNL's symmetric eigensystem
(`larmorx_core::vnl_eigen::symmetric_eigensystem`, EISPACK's `rs`). `largest` and `smallest` from index
0: for `k = 1, 2`, `D[largest] < D[k]` makes `largest = k`, then `D[smallest] > D[k]` makes `smallest =
k`. The result's columns are `V`'s columns `largest`, `3 - smallest - largest`, `smallest`; when its 3 x
3 determinant (`larmorx_core::vnl_fixed::det3`) is below 0, its first column negated. FS-966 and FS-967
(§5) where no eigenvalue is above another or the covariance is not finite.

## 4. The matrix helpers (`MyMatrix`)

### 4.1 Conversions

- `convertVNL2MATRIX(m)` (a VNL matrix or vector): a new `float` matrix (`specs/freesurfer-maths.md`) of
  its size (a vector: `n x 1`), each element converted to `float`.
- `convertMATRIX2VNL(m)`: a VNL matrix of the `float` matrix's size, each element widened.
- `getVNLMatrix(d, r)`: `c = len(d) / r` (integer division; `r > 0`), the numbers row by row into `r x c`
  (numbers past `r c` unused).

### 4.2 `isDiag(A, eps)`

`sse = +0`; for `i = 1 .. n - 1`, `j = i - 1` down to 0: `sse = sse + a(i, j) a(i, j)`, then `sse = sse +
a(j, i) a(j, i)`. True when `sse < eps`.

### 4.3 `PolarDecomposition(A)`, `Polar2Decomposition(A)`

VNL's SVD of `A` (`vnl_svd<double>`, LINPACK's `dsvdc`, job 21: `larmorx_core::vnl_svd::vnl_svd_full`),
`W` the singular values' magnitudes (a magnitude of 0 or below set to `+0`). When it converged: `R = U
V^T`, `S = (V W) V^T` (the products of §1). When it did not (`info` not 0): VNL's warning on standard
error (its two lines `<vnl_svd.hxx>: suspicious return value (<info>) from SVDC` and `<vnl_svd.hxx>: M is
<r>x<c>`, then the matrix as `vnl_matlab_print` writes it in its long format: `M = [ ...`, each row's
numbers `%16.13f ` (an exact zero `%16d ` of 0), the last row closed by ` ]`; `<vnl_svd.hxx>` is
`/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK/Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_svd.hxx`,
the official build's path **(observed)**), then ends the program with `MyMatrix PolarDecomposition ERROR:
SVD not possible?`, status 1. Where LINPACK never returns (ITK-116: its deflation steps come back to a
state they had been in, on an infinite or NaN entry) FreeSurfer hangs; an error in both modes (§5).
`Polar2Decomposition`: then for each column `c` in order `D_c = S(c, c)` and column `c` of `S` divided
by it.

### 4.4 Distances between transforms (4 x 4)

- `AffineTransDistSq(a, b, r)`: `d = a - b` (`a - I` when `b` is empty); `t = +0`; for `i = 0, 1, 2`: `t =
  t + d(i, 3) d(i, 3)`, then `d(i, 3) = d(3, i) = 0`; `d(3, 3) = 0`; `d = d^T d`; `tr = +0 + d(0, 0) +
  d(1, 1) + d(2, 2)` (in order); the result `((0.2 r) r) tr + t`.
- `RigidTransDistSq(a, b)`: `d = b a^+` (`a`'s pseudo-inverse from VNL's SVD: `V W^+ U^T`, `W^+` a full
  diagonal matrix of `1 / w` for each non-zero `w`; the products of §1), or `a` when `b` is empty; `t =
  +0 + d(0, 3)^2 + d(1, 3)^2 + d(2, 3)^2` (in order); `rd` the log norm below of `d`; the result `rd rd +
  t`. When the SVD did not converge, VNL's warning first and FS-968 (§5).
- `getFrobeniusDiff(m1, m2)`: `|m1 - m2|_F`.
- `RotMatrixLogNorm(m)`: `det` VNL's 4 x 4 determinant (`larmorx_core::vnl_fixed::determinant`); when
  `|det - 1| > 0.001`, `There is streching! det: <det>` on standard error (`%g`, 6 digits), nothing else
  (FreeSurfer's assertion is compiled out); `t = 0.5 ((+0 + m(0,0) + m(1,1) + m(2,2)) - 1)`, clipped to
  `[-1, 1]` (`t > 1` then `t < -1`); the result `sqrt(2) acos(t)` (`sqrt(2)` the `double` nearest).

### 4.5 `SchurComplex(A, U, T)` (a square `n x n` matrix; part 2, relay 41)

The complex Schur form `A = U T U^H`, `T` upper triangular, `U` unitary: reference LAPACK 3.10.0's
`ZGEES` with `JOBVS = 'V'`, `SORT = 'N'`, `LDA = LDVS = N`, `LWORK = 10 N` on `A` converted to complex
(each element `(a, +0)`), as Ubuntu 22.04's `liblapack.a` (3.10.0-2ubuntu1, gfortran 11) computes it,
which is the main package's open `larmorx_core::lapack_zgees::zgees_vn` (its column-major `a` is `A`
read column by column; `T(i, j)` and `U(i, j)` come back as `a[i + j n]` and `vs[i + j n]`). Its
complex arithmetic and its `ABS` and `SQRT` of a complex number are `larmorx_core::cplx`'s (gfortran's
plain product and Smith's division; the correctly rounded `hypot` inside `cabs` and `csqrt`: the
project's decision D4, §5).

- When `ZGEES` returns a non-zero `info`, on standard error `<MyMatrix.cpp>: info = <info>, something went
  wrong:` and, for `info` from 1 to `n`, `<MyMatrix.cpp>: the QR iteration failed, but the last <n -
  info> eigenvalues may be correct` (for `info < 0`: `... (internal error) the <-info>th argument had an
  illegal value`; `n + 1`: `... EV could not be reordered (ill conditioned)`; `n + 2`: `... roundoff
  error -- maybe due to poor scaling`; otherwise `... unknown error`), each line ended by a newline,
  `<MyMatrix.cpp>` being `/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0/mri_robust_register/MyMatrix.cpp`
  **(observed)**; then it goes on with what `ZGEES` left in `U` and `T`. On the matrices FreeSurfer
  computes this happens only with an infinite or NaN element (FS-969, §5), whose `info` is 2 on the
  2 x 2 matrices observed.
- An empty matrix: FS-970 (§5).

### 4.6 `MatrixSqrt(A)` (a square `n x n` matrix; part 2, relay 41)

The principal square root, from `SchurComplex(A, U, T)` (§4.5), in C++'s complex arithmetic
(`larmorx_core::cplx`: `cxx_mul`, the plain product with C11 Annex G's `_Cmultd` where both parts are
NaN; `cxx_div`, Smith's quotient with Annex G's recovery; `csqrt`; `cabs`) and VNL's complex matrices
(`larmorx_core::vnl_matrix::VnlCMatrix`: products `sum = (0, 0)`, then `sum = sum + a(i, k) b(k, j)` for
`k` in order; `U^H` the conjugate transpose):

1. When `isDiag(real(T), 1e-14)` and `isDiag(imag(T), 1e-14)` (§4.2): `R_i = csqrt(T(i, i))` and the root
   `S = (U diag(R)) U^H`, `U diag(R)` being `U(i, j) R_j` element by element.
2. Otherwise the upper triangular `R`: for each column `j` from 0, `R(j, j) = csqrt(T(j, j))`, then for
   `i` from `j - 1` down to 0: `s = (0, 0)`, `s = s + R(i, k) R(k, j)` for `k = i + 1 .. j - 1`, and `R(i,
   j) = (T(i, j) - s) / (R(i, i) + R(j, j))` (the difference and the sum part by part, the quotient
   `cxx_div`); the other elements `(0, 0)`. The root `S = (U R) U^H`.
3. `imagone` = the one-norm of `imag(S)` (the largest column sum of absolute values, from `+0`, a column
   kept when greater); when `imagone > ((10 n) 2^-52) |S|_1` (`|S|_1` the one-norm of `S` with `cabs`):
   on standard error `MyMatrix::MatrixSqrt Internal Error: ` and a newline, ` Result too imaginary to
   ignore! ( <imagone> )`, ` Debug Info: `, then `A`, `T` and `U` printed by VNL's long format, each
   followed by an empty line, ` fro( real(U * T * U^*) - A) = <f>` and an empty line, `S` printed (`Asqrt`)
   and an empty line, ` fro( real(sqrt(A)) ^2 - A)  = <g>` and an empty line; the program ends, status 1.
   The numbers are C++'s default `%g` (6 digits); `<f>` is the Frobenius norm of `real((U T) U^H) - A`,
   `<g>` of `real(S) real(S) - A`. VNL's long format (`larmorx_core::vnl_matrix::matlab_print_long` and
   `matlab_print_long_complex`): `<name> = [ ...`, each row's elements, ` ]` after the last row's, each row
   then a newline; a `double` as `%16.13f ` (`%16d ` of 0 for a zero of either sign), a complex number's
   real part as `%16.12f ` (`%16d ` of 0 for a zero) and its imaginary part as `+` or `-` (its sign, `-`
   only below zero) and `%15.12fi ` of its magnitude, or 18 blanks (` %15s  ` of nothing) for a zero.
4. `M = real(S)`, `D = M M - A` (VNL's product, then the difference element by element); when `|D|_F >
   1e-10`: on standard error `MyMatrix::MatrixSqrt Internal Error: `, `  Result not close enough to square
   root!`, `  Frobenius norm of difference: <|D|_F>`, then `A`, `M` (as `Asqrt`) and `D` (as `Diff`) in
   VNL's long format, each followed by an empty line; the program ends, status 1.
5. Otherwise the result is `M`.

**(observed)**: `[[-1, 0], [0, 1]]` ends in step 3 with `imagone` 1 and its `T`, `U` and `S` printed;
Jordan blocks with a zero eigenvalue end in step 4.

## 5. Defects (`larmorx_core::defects`; the register's FS entries)

- **FS-966** (memory): in §3.2, when no eigenvalue is above another (all equal, or NaN), `largest =
  smallest = 0` and the middle column's index is 3: FreeSurfer reads a fourth column of the 3 x 3
  eigenvector matrix (stored row by row: its numbers 3 and 6, then the number past the matrix). By
  default the columns 0, 1, 2; `reproduce` gives the two numbers and 0 for the one past the matrix
  (noted).
- **FS-967** (rule B): §3.2 on an image with an infinite or NaN voxel (a covariance that is not finite):
  VNL's eigensystem never returns (FreeSurfer hangs, ITK-115) or stops without eigenvectors (its line
  `vnl_symmetric_eigensystem: ierr = <i>`, the eigenvectors memory never written). By default an error
  and the end of the program: `CostFunctions::orientation: the image has an infinite or NaN voxel (no
  covariance)`, status 1 (as §3.1 ends on such images); `reproduce` gives FreeSurfer's result where VNL
  returns with its eigenvectors, the same end elsewhere.
- **FS-968** (rule B): §4.4's `RigidTransDistSq` when the SVD of `a` did not converge (an infinite or NaN
  entry): FreeSurfer computes the distance from the decomposition reached (an arbitrary number). By
  default, after VNL's warning, `MyMatrix: no inverse of a matrix whose singular value decomposition did
  not converge (an infinite or NaN entry)` and status 1; `reproduce` gives FreeSurfer's number.
- **FS-969** (rule B): §4.5 and §4.6 on a matrix with an infinite or NaN element: the Schur form's QR
  iteration does not converge (§4.5's lines, `info = 2` on the 2 x 2 matrices observed) and NaN factors
  come back, or the NaNs pass through; §4.6's checks never fail on NaNs, so a NaN matrix is returned. By
  default, before anything, `MyMatrix::SchurComplex: the matrix has an infinite or NaN element (FreeSurfer
  goes on with LAPACK's failed or NaN result)` on standard error and status 1; `reproduce` computes as
  FreeSurfer does, except where LAPACK's `ZLAHQR` does not converge: there `ZHSEQR` retries with `ZLAQR0`
  (on a 49 x 49 copy), which the main package does not port (`zgees_vn` returns `NotPorted`): `MyMatrix::
  SchurComplex: LAPACK's ZLAQR0 (ZHSEQR's second attempt where ZLAHQR does not converge) is not ported yet`
  and status 1. Noted in both modes.
- **FS-970** (rule B): §4.5 and §4.6 on an empty matrix: ` ** On entry to ZGEES parameter number  6 had an
  illegal value` on standard output and the end of the program with status 0 **(observed)**. By default
  `MyMatrix::SchurComplex: an empty matrix (FreeSurfer ends here with status 0 after LAPACK's message)` on
  standard error and status 1; `reproduce` FreeSurfer's line and status 0. Noted in both modes.
- **`cabs` and `csqrt`** (not a defect: the project's decision D4, GLIBC-07): the correctly rounded
  `hypot` in both modes where glibc's misses the last bit on about 0.5 % of arguments; `csqrt` otherwise
  glibc's form (`larmorx_core::cplx::csqrt`'s documentation; its measured divergences near overflow).
- **ITK-115**, **ITK-116** (VNL's eigensystem and SVD never returning): an error in both modes, `MyMatrix:
  VNL's singular value decomposition never returns on this matrix (an infinite or NaN entry)` and status
  1 for the SVD.

## 6. What is not part of this piece

- The registration (`RegRobust`, `Registration`'s multi-resolution loop), its transformations, steps and
  regression: part 2 (queue 3.2-03).
- The helpers only one program reaches (`MRInorm255`, the entropy, NLSD, normalised, mean and median
  images, `noiseVar`, `gaussianCube`, `setMaxOutsideVal`, `setTypeUCHAR`, `MRIvoxelXformToRasXform`,
  `isIsotropic`, `isConform`, the cost functions of the Powell registration, the matrix logarithm and
  exponential, the means of rotations and transforms, the LTA conversions): the programs' items (3.2-13
  to 3.2-16).
- Threads: nothing here runs in parallel.

## 7. The numbers of FreeSurfer's build (observed)

- The prefilter's and derivative filter's `float`s: `3d0f8590 3e7ec0da 3edd5cf1 3e7ec0da 3d0f8590` and
  `bddae9e3 be91b8a3 00000000 3e91b8a3 3ddae9e3`.
- `uchar` scaling wraps negatives: `MRIvalscale` of a voxel 10 with `s = 1`, `b = 13` stores 253.

## 8. Validation plan (black box)

- **The official programs** (FreeSurfer 8.2.0, one thread, `OMP_NUM_THREADS=1`): `mri_robust_register
  --mov <a> --dst <b> --lta <out> --satit --iscale` (sMRIPrep's call) and `mri_robust_template` reach
  every function of §2 to §4 on each registration (the Gaussian pyramid of §2.4 and §2.5, the partials
  of §2.6 at every step, the background of §2.8, the centroids of §3.1 for the initial alignment, §3.2
  with `--initorient`, the distances of §4.4 at every iteration, the polar decompositions of §4.3 for
  `--affine` and the half-way maps). Their outputs depend on part 2 too; part 1 alone is checked
  through the open probe below.
- **The open probe** (to come from the spec finisher, track E): calls of the functions of §2 to §4 by name
  on volumes (the fs-volume probe's line protocol) and matrices, compared bit for bit (NaNs as their bits),
  with the cases of §5 counted apart.
- **The square root** (§4.5, §4.6): reached at every step of the symmetric robust registration (both
  programs' default: each step's transform is split into two half transforms), by the half-way maps of
  the result, and by the Powell registration's steps; the open probe should call both functions by name on 4 x 4 registration
  transforms, 3 x 3 rotations (a scale times a rotation), rotations by a right and a straight angle (purely
  imaginary and negative eigenvalues), Jordan blocks, diagonal, triangular and sparse matrices (LAPACK's
  balancing permutes rows and columns that isolate eigenvalues), sizes 1 to 12, matrices scaled by 1e-300
  and 1e300 (LAPACK's scaling below `2^-459` and above `2^459`), columns of tiny numbers (1e-300, the
  reflectors' rescaling; 1e-160, the norm's small sums), negative and zero eigenvalues, the empty matrix and
  special elements. **(observed)** on 10,200 such cases: 4,780 Schur forms and 3,735 square roots
  returned, 906 ends in §4.6's step 3 and 128 in step 4, 516 FS-970 ends, 135 FS-969 matrices on which
  `ZLAHQR` does not converge; everything else reproduced bit for bit, the messages byte for byte, by
  `zgees_vn` and `larmorx_core::cplx` with the correctly rounded `hypot` in the official code too.
- **Reference numbers (observed)**: on 16,100 seeded cases (23 functions; volumes up to 7 x 7 x 6 of every
  type, 1 or 2 frames, ordinary and special voxels; transforms of random rotations, scales and
  translations, near-identities and special entries) FreeSurfer's lines are reproduced but where the
  official code reads memory it does not own, crashes or hangs (363), and four FS-968 lines are identical
  but for the NaNs' bits. At one thread on the main machine, on a 256^3 float volume: `getPartials` 0.8 s
  (FreeSurfer's peak 402 MB), `getBlur` 0.4 s, `MRIvalscale` 0.08 s, `subSample` 0.03 s.
