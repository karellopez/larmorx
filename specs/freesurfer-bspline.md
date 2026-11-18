# FreeSurfer 8.2.0's B-splines: behaviour spec (`fs.bspline`)

Written by the replica author of queue item 3.2-01 (track C, relay 39, 2026-11-18) as its spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room (queue C3.2-01, in `larmorx-fs-core`) from this file, black-box runs
of the official programs, `specs/freesurfer-mri.md` (the volume structure, `MRIgetVoxVal`,
`MRIsetVoxVal`, the allocation and header copy), `specs/freesurfer-maths.md` (the `float` matrix
product, the inverse), `specs/freesurfer-volume.md` (`MRIindexNotInVolume`, `MRIcloneDifferentType`,
`MRIclear`, the RAS-to-voxel conversion, the transform-array type change, the geometry functions) and
the published method: P. Thévenaz, T. Blu and M. Unser, "Interpolation Revisited", IEEE Transactions on
Medical Imaging 19(7), 739-758, 2000; M. Unser, A. Aldroubi and M. Eden, "The L2-polynomial spline
pyramid", IEEE PAMI 15(4), 364-379, 1993; P. Brigger, F. Müller, K. Illgner and M. Unser, "Centered
pyramids", IEEE Transactions on Image Processing 8(9), 1254-1264, 1999.

**Whoever implements it must not open:** FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`, any
other copy), EPFL's example code (the file this one follows), the replica crates (`crates-freesurfer/`,
among them `larmorx-freesurfer-volume`), the documents written from FreeSurfer's source
(`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the replica's probes (`validation/probes/fs-*`,
among them `fs-volume` and `fs-bspline-bench`), the unit oracle's build tree and tools, `docs/parts/` and
the scratch folders of FreeSurfer's parts. **It may read** this spec, the FreeSurfer specs it cites, the
papers above, IEEE 754-2008, C11, the main package's open code and documentation, and run as black
boxes the official FreeSurfer 8.2.0 programs (§9).

Status: complete for the module (every function the programs reach, §2 to §8); the open probe of §9
is the spec finisher's next piece (track E). What black-box runs confirmed is marked **(observed)**.

## 1. Conventions

- Those of `specs/freesurfer-mri.md` §1: x86-64 SSE2 arithmetic in IEEE `double` (no fused
  multiply-add, no extended precision), every intermediate rounded to `double`; a NaN result made from
  numbers is the default NaN with the sign bit set (`0xfff8000000000000`); a NaN operand is passed on
  quieted with its sign and payload; **of two NaN operands the first's** (the "first operand" of each
  operation is named below wherever two NaNs can meet). Conversions: `float` to `double` quiets a
  signalling NaN; `double` to `float` rounds to nearest (a NaN keeps its sign and the top of its
  payload).
- A volume, its extent `(width, height, depth, nframes)`, `MRIgetVoxVal` (any type to `float`, the
  out-of-range rules) and `MRIsetVoxVal` (a `float` stored by the type's rules) are
  `specs/freesurfer-mri.md`'s. Every voxel read here is read through `MRIgetVoxVal` and widened to
  `double`; every voxel written is written through `MRIsetVoxVal`.
- "Ends the program (message, status 1)": the message on standard output followed by a newline, then
  `exit(1)`. "An error": FreeSurfer's error printer (`specs/freesurfer-core.md`) on standard error.
- The C library's `log`, `pow`, `floor`, `ceil`, `fabs`: `floor`, `ceil` and `fabs` are exact; `log`
  and `pow` give glibc 2.35's last bits (the project's default takes the correctly rounded functions,
  `reproduce` glibc's: `larmorx_core::math::Libm`; §3.3 says where they differ).

## 2. The B-spline

A B-spline is: its **degree** (an `int`), its **coefficients** (a `float` volume), the **source's type**
(an `int`) and whether the source had a **negative** voxel (`srcneg`, 0 or 1).

- `MRIallocBSpline(width, height, depth, nframes)`: coefficients allocated as a `float` volume of that
  extent (`MRIallocSequence`; its failures are its own, §`specs/freesurfer-mri.md`), degree -1, source
  type -1, `srcneg` 0.
- `MRIfreeBSpline(&b)`: with no B-spline, the error `MRIfreeBSpline: null pointer` and
  `ERROR_BADPARM` (-4); otherwise the coefficients freed (`MRIfree`, whose own `MRIfree: null pointer`
  error a B-spline without coefficients gives), the B-spline freed and set to none, `NO_ERROR` (0).

## 3. The coefficients (`MRItoBSpline(src, b, degree)`)

### 3.1 Order of events

1. `MRItoBSpline degree <degree>` (`%i`) on standard output.
2. No B-spline given: a new one (§2) of the source's extent and frames, the source's header copied
   onto its coefficients (`MRIcopyHeader`, which copies neither the type nor `outside_val`), the
   coefficients' `outside_val` set to the source's. A B-spline given without coefficients: new
   coefficients the same way. A given B-spline's existing coefficients are used as they are.
3. Degree, source type and `srcneg = 0` written into the B-spline.
4. Coefficients not of type `float`: ends the program (`ERROR MRItoBSpline: BSpline Coeff MRI needs
   to be float!`, status 1). Coefficients of another width, height, depth or frame count than the
   source: ends the program (`ERROR MRItoBSpline: BSPline Coeff MRI dimensions must be same as Source
   MRI!`, status 1).
5. A degree other than 2 to 9: ends the program (`Invalid spline degree`, status 1).
6. **Pass x**: for each slice `z`, row `y`, frame `f` (any order: every line is independent; the
   official build runs slices in parallel): the line `L[x] = MRIgetVoxVal(src, x, y, z, f)` for `x` in
   `[0, width)`, widened; if `srcneg` is still 0 and some `L[x] < 0` (NaN and -0 are not), `srcneg = 1`;
   if `width > 1` the line is filtered (§3.2); then stored (§3.4) into the coefficients' line.
7. **Pass y** (only when `height > 1`): each line along y of the coefficients (for each `z`, `x`, `f`),
   read back from the coefficients (`float`, widened), filtered, stored.
8. **Pass z** (only when `depth > 1`): each line along z, the same.
9. The B-spline returned.

### 3.2 The line filter (`n` samples `c[0..n)`, poles `z_1..z_P` of the degree)

A line of one sample is left as it is. Otherwise, in `double`:

1. **Gain** `λ = ∏_k (1 - z_k)(1 - 1/z_k)`, accumulated left to right: `λ = (λ * (1 - z_k)) * (1 - 1 /
   z_k)` from `λ = 1`. Every sample `c[i] = c[i] * λ` (`c[i]` first).
2. For each pole `z = z_k` in order:
   - **Causal start** `c[0] = S`: the horizon `H = (long) ceil(log(DBL_EPSILON) / log(fabs(z)))`
     (`DBL_EPSILON` = 2^-52; the quotient converted to `long` by truncation).
     - If `H < n` (the accelerated start): `S = c[0]`, `zn = z`; for `i = 1 .. H-1`: `S = S + c[i] * zn`
       (`S` first; `c[i]` first in the product), then `zn = zn * z`.
     - Otherwise (the full start, mirror boundaries): `zn = z`, `iz = 1 / z`, `z2n = pow(z, n - 1)`
       (the exponent `(double)(n - 1)`); `S = c[n-1] * z2n + c[0]` (the product first: `c[n-1]`'s NaN
       before `c[0]`'s); `z2n = z2n * (z2n * iz)`; for `i = 1 .. n-2`: `S = S + (zn + z2n) * c[i]` (`S`
       first; the product's NaN is `c[i]`'s), then `zn = zn * z`, `z2n = z2n * iz`; finally `S = S / (1
       - zn * zn)`.
   - **Causal recursion** for `i = 1 .. n-1`: `c[i] = c[i-1] * z + c[i]` (the product first:
     `c[i-1]`'s NaN before `c[i]`'s).
   - **Anticausal start** `c[n-1] = (z / (z * z - 1)) * (c[n-2] * z + c[n-1])` (in the sum the product
     first).
   - **Anticausal recursion** for `i = n-2` down to 0: `c[i] = (c[i+1] - c[i]) * z` (`c[i+1]` first).

So a NaN in a line spreads to the whole line, the payload of the earliest NaN within the start's reach
through the causal pass and of the last element through the anticausal pass **(observed)**.

**The poles** (the published values; in `double` from these expressions, `sqrt` correctly rounded):

| degree | poles |
|---|---|
| 2 | `sqrt(8) - 3` |
| 3 | `sqrt(3) - 2` |
| 4 | `sqrt(664 - sqrt(438976)) + sqrt(304) - 19`, `sqrt(664 + sqrt(438976)) - sqrt(304) - 19` |
| 5 | `sqrt(135/2 - sqrt(17745/4)) + sqrt(105/4) - 13/2`, `sqrt(135/2 + sqrt(17745/4)) - sqrt(105/4) - 13/2` |
| 6 | -0.48829458930304475513011803888378906211227916123938, -0.081679271076237512597937765737059080653379610398148, -0.0014141518083258177510872439765585925278641690553467 |
| 7 | -0.53528043079643816554240378168164607183392315234269, -0.12255461519232669051527226435935734360548654942730, -0.0091486948096082769285930216516478534156925639545994 |
| 8 | -0.57468690924876543053013930412874542429066157804125, -0.16303526929728093524055189686073705223476814550830, -0.023632294694844850023403919296361320612665920854629, -0.00015382131064169091173935253018402160762964054070043 |
| 9 | -0.60799738916862577900772082395428976943963471853991, -0.20175052019315323879606468505597043468089886575747, -0.043222608540481752133321142979429688265852380231497, -0.0021213069031808184203048965578486234220548560988624 |

(The decimal strings converted to the nearest `double`; the square-root forms evaluated left to right
as written: e.g. degree 4's first is `(sqrt(664 - sqrt(438976)) + sqrt(304)) - 19`.)

### 3.3 `log` and `pow` **(observed)**

The horizons are 21 (degree 2), 28 (degree 3), and 73 for degree 9's first pole (every pole's horizon is
at least 2). For all 18 poles glibc's and the correctly rounded `log` agree, and so does `pow(z, m)` for
every `m` from 1 to `H - 1` but one: degree 6's third pole with `m = 4` (a line of 5 samples), where
glibc's result is `0x3d9196d024025c2d` and the correctly rounded one `0x3d9196d024025c2c`. The default
takes the correctly rounded value there, `reproduce` glibc's.

### 3.4 Storing a line

For each sample `k` of the line (the target volume's extent along the line's axis): a `uchar` target
takes 255 when `L[k] > 255`, 0 when `L[k] < 0`, else `nint(L[k])` (`specs/freesurfer-core.md`'s `nint`,
then converted to `float`); any other type `(float) L[k]`; stored with `MRIsetVoxVal`. Before the
samples: a line index outside the target ends the program, checked in the order of the line's other two
axes then the frame, with `X limit error`, `Y limit error`, `Z limit error` or `Frames limit error`
(status 1). (The coefficients are `float`: the `uchar` rule matters to the pyramids of §6.)

## 4. Sampling

### 4.1 `MRIsampleBSpline(b, x, y, z, frame, &val)`

Returns `NO_ERROR` (0) always.

1. `frame` outside `[0, nframes)` of the coefficients, or `MRIindexNotInVolume(coefficients, x, y, z)`
   = 1 (outside by more than half a voxel; NaN coordinates included): `val` = the coefficients'
   `outside_val`.
2. For each axis (extent `N`, coordinate `t`, degree `d`): the first index `i0` = `(long)(floor(t) -
   d/2)` for an odd degree, `(long) floor(t + 0.5) - d/2` for an even one (`d/2` the integer half), and
   the indices `i0, i0+1, .., i0+d` (each kept as a 32-bit `int`). The weights (§4.3) of `t` against those
   indices, except on an axis of one voxel, which takes the single weight 1 and the single index 0.
3. **Mirror boundaries** (for `N > 1`, `N2 = 2N - 2`): an index `i < 0` becomes `-i - N2 * ((-i) / N2)`,
   one `i >= 0` becomes `i - N2 * (i / N2)` (integer division truncating toward zero); then an index
   `>= N` becomes `N2 - i`.
4. The sum, z outermost: `sum = 0`; for each `k` (0 to `d`, or 0 alone for a depth of 1): `w2 = 0`; for
   each `j`: `w = 0`; for each `i`: `w = w + C(i, j, k) * wx[i]` (`w` first; the coefficient first in the
   product, `C` the `float` coefficient widened); then `w2 = w * wy[j] + w2` (**the product first**);
   then `sum = w2 * wz[k] + sum` (**the product first**).
5. A source without a negative voxel (`srcneg` 0) clips a negative `sum` to 0 (NaN and -0 are kept);
   `val = sum`.

A B-spline whose degree is not 2 to 9 (one allocated and never computed): with an axis longer than one
voxel the weights end the program (`Invalid spline degree`, status 1) before any index is used, unless
the degree is above 9; on a volume of one voxel degrees 0 and 1 read its coefficient. The other cases
are FS-964 (§7).

### 4.2 `MRIsampleSeqBSpline(b, x, y, z, valvect, first, last)`

Returns `NO_ERROR`. `MRIindexNotInVolume = 1`: `valvect[f] = (float) outside_val` for `f` from `first` to
`last`. Otherwise the indices and weights once (§4.1 steps 2 and 3), then for each `f` from `first` to
`last` the sum of §4.1 step 4 on frame `f` (the frames are not checked against the volume's: a frame
beyond them reads through `MRIgetVoxVal`'s out-of-range rules), clipped as step 5, `valvect[f] = (float)
sum`. `valvect` is indexed by the frame itself.

### 4.3 The weights (`t` the coordinate, `i1`, `i2`, `i3`, `i4` the indices named; `w0 .. wd`)

Each formula is evaluated as written, left to right where not parenthesised, in `double`; a fraction
`p/q` of two integer constants is the constant's nearest `double`. The weights are finite for every
point that reaches them.

- **Degree 2** (`s = t - i[1]`): `w1 = 3/4 - s*s`; `w2 = (1/2) * ((s - w1) + 1)`; `w0 = (1 - w1) - w2`.
- **Degree 3** (`s = t - i[1]`): `w3 = (((1/6) * s) * s) * s`; `w0 = ((1/6) + ((1/2) * s) * (s - 1)) -
  w3`; `w2 = (s + w0) - 2 * w3`; `w1 = ((1 - w0) - w2) - w3`.
- **Degree 4** (`s = t - i[2]`): `s2 = s*s`; `u = (1/6) * s2`; `w0 = 1/2 - s`; `w0 = w0 * w0`; `w0 = w0 *
  ((1/24) * w0)`; `t0 = s * (u - 11/24)`; `t1 = 19/96 + s2 * (1/4 - u)`; `w1 = t1 + t0`; `w3 = t1 - t0`;
  `w4 = (w0 + t0) + (1/2) * s`; `w2 = (((1 - w0) - w1) - w3) - w4`.
- **Degree 5** (`s = t - i[2]`): `s2 = s*s`; `w5 = (((1/120) * s) * s2) * s2`; `s2 = s2 - s`; `s4 = s2 *
  s2`; `s = s - 1/2`; `u = s2 * (s2 - 3)`; `w0 = (1/24) * ((1/5 + s2) + s4) - w5`; `t0 = (1/24) * (s2 *
  (s2 - 5) + 46/5)`; `t1 = ((-1/12) * s) * (u + 4)`; `w2 = t0 + t1`; `w3 = t0 - t1`; `t0 = (1/16) * (9/5
  - u)`; `t1 = ((1/24) * s) * ((s4 - s2) - 5)`; `w1 = t0 + t1`; `w4 = t0 - t1`.
- **Degree 6** (`s = t - i[3]`): `w0 = 1/2 - s`; `w0 = w0 * (w0 * w0)`; `w0 = w0 * (w0 / 720)`;
  `w1 = (361/192 - s * (59/8 + s * (-185/16 + s * (25/3 + (s * (-5/2 + s)) * (1/2 + s))))) / 120`;
  `w2 = (10543/960 + s * (-289/16 + s * (79/16 + s * (43/6 + s * (-17/4 + s * (-1 + s)))))) / 48`;
  `s2 = s*s`; `w3 = (5887/320 - s2 * (231/16 - s2 * (21/4 - s2))) / 36`;
  `w4 = (10543/960 + s * (289/16 + s * (79/16 + s * (-43/6 + s * (-17/4 + s * (1 + s)))))) / 48`;
  `w6 = 1/2 + s`; `w6 = w6 * (w6 * w6)`; `w6 = w6 * (w6 / 720)`;
  `w5 = (((((1 - w0) - w1) - w2) - w3) - w4) - w6`.
- **Degree 7** (`s = t - i[3]`): `w0 = 1 - s`; `w0 = w0 * w0`; `w0 = w0 * (w0 * w0)`; `w0 = w0 * ((1 - s)
  / 5040)`; `s2 = s*s`;
  `w1 = (120/7 + s * (-56 + s * (72 + s * (-40 + s2 * (12 + s * (-6 + s)))))) / 720`;
  `w2 = (397/7 - s * (245/3 + s * (-15 + s * (-95/3 + s * (15 + s * (5 + s * (-5 + s))))))) / 240`;
  `w3 = (2416/35 + s2 * (-48 + s2 * (16 + s2 * (-4 + s)))) / 144`;
  `w4 = (1191/35 - s * (-49 + s * (-9 + s * (19 + (s * (-3 + s)) * (-3 + s2))))) / 144`;
  `w5 = (40/7 + s * (56/3 + s * (24 + s * (40/3 + s2 * (-4 + s * (-2 + s)))))) / 240`;
  `w7 = s2`; `w7 = w7 * (w7 * w7)`; `w7 = w7 * (s / 5040)`;
  `w6 = ((((((1 - w0) - w1) - w2) - w3) - w4) - w5) - w7`.
- **Degree 8** (`s = t - i[4]`): `w0 = 1/2 - s`; `w0 = w0 * w0` twice; then `w0 = w0 * (w0 / 40320)`;
  `s2 = s*s`;
  `w1 = ((39/16 - s * (6 + s * (-9/2 + s2))) * (21/16 + s * (-15/4 + s * (9/2 + s * (-3 + s))))) / 5040`;
  `w2 = (82903/1792 + s * (-4177/32 + s * (2275/16 + s * (-487/8 + s * (-85/8 + s * (41/2 + s * (-5 + s
  * (-2 + s)))))))) / 1440`;
  `w3 = (310661/1792 - s * (14219/64 + s * (-199/8 + s * (-1327/16 + s * (245/8 + s * (53/4 + s * (-8 + s
  * (-1 + s)))))))) / 720`;
  `w4 = (2337507/8960 + s2 * (-2601/16 + s2 * (387/8 + s2 * (-9 + s2)))) / 576`;
  `w5 = (310661/1792 - s * (-14219/64 + s * (-199/8 + s * (1327/16 + s * (245/8 + s * (-53/4 + s * (-8 + s
  * (1 + s)))))))) / 720`;
  `w7 = ((39/16 - s * (-6 + s * (-9/2 + s2))) * (21/16 + s * (15/4 + s * (9/2 + s * (3 + s))))) / 5040`;
  `w8 = 1/2 + s`; `w8 = w8 * w8`; `w8 = w8 * w8`; `w8 = w8 * (w8 / 40320)`;
  `w6 = (((((((1 - w0) - w1) - w2) - w3) - w4) - w5) - w7) - w8`.
- **Degree 9** (`s = t - i[4]`): `w0 = 1 - s`; `w0 = w0 * w0`; `w0 = w0 * w0`; `w0 = w0 * ((w0 * (1 - s))
  / 362880)`;
  `w1 = (502/9 + s * (-246 + s * (472 + s * (-504 + s * (308 + s * (-84 + s * (-56/3 + s * (24 + s * (-8
  + s))))))))) / 40320`;
  `w2 = (3652/9 - s * (2023/2 + s * (-952 + s * (938/3 + s * (112 + s * (-119 + s * (56/3 + s * (14 + s *
  (-7 + s))))))))) / 10080`;
  `w3 = (44117/42 + s * (-2427/2 + s * (66 + s * (434 + s * (-129 + s * (-69 + s * (34 + s * (6 + s * (-6
  + s))))))))) / 4320`;
  `s2 = s*s`; `w4 = (78095/63 - s2 * (700 + s2 * (-190 + s2 * (100/3 + s2 * (-5 + s))))) / 2880`;
  `w5 = (44117/63 + s * (809 + s * (44 + s * (-868/3 + s * (-86 + s * (46 + s * (68/3 + s * (-4 + s * (-4
  + s))))))))) / 2880`;
  `w6 = (3652/21 - s * (-867/2 + s * (-408 + s * (-134 + s * (48 + s * (51 + ((s * (-4 + s)) * (-1 + s)) *
  (2 + s))))))) / 4320`;
  `w7 = (251/18 + s * (123/2 + s * (118 + s * (126 + s * (77 + s * (21 + s * (-14/3 + s * (-6 + s * (-2 +
  s))))))))) / 10080`;
  `w9 = s2 * s2`; `w9 = w9 * ((w9 * s) / 362880)`;
  `w8 = ((((((((1 - w0) - w1) - w2) - w3) - w4) - w5) - w6) - w7) - w9`.

(Degree 9's `w0 = w0 * ((w0 * (1 - s)) / 362880)` and `w9 = w9 * ((w9 * s) / 362880)` group as C's `x *=
a * b / c` does: `x = x * ((a * b) / c)`.)

## 5. Transforms

### 5.1 `MRIlinearTransformBSpline(b, dst, A)` (`A` voxel to voxel)

1. `A^-1` (`MatrixInverse`, `specs/freesurfer-maths.md`; no matrix: its own `MatrixInverse: NULL input
   matrix!` and end); a singular `A`: the error `MRIlinearTransformSpline: xform is singular`,
   `ERROR_BADPARM`, the result none.
2. No `dst`: a volume of the source's type with the coefficients' extent, frames and header
   (`MRIcloneDifferentType(coefficients, source type)`: its `outside_val` 0); a given `dst` is cleared
   (`MRIclear`).
3. For each destination voxel, slices outermost, then rows, columns innermost: the point `A^-1 (y1, y2,
   y3, 1)` with the voxel's indices as `float`s and the product in `float` (`MatrixMultiply`), widened;
   for each frame of the coefficients, `MRIsampleBSpline` there (§4.1) stored into `dst` with
   `MRIsetVoxVal` (as a `float`; frames beyond `dst`'s follow `MRIsetVoxVal`'s rules).
4. `dst`'s `ras_good_flag` = 1; `dst` returned.

### 5.2 `MRIapplyRASlinearTransformBSpline(b, dst, M)`

The voxel-to-voxel matrix `MRIrasXformToVoxelXform(coefficients, dst, M)` (the coefficients' geometry
as the source's, `dst`'s as the destination's, the coefficients' when there is no `dst`;
`specs/freesurfer-volume.md`), then §5.1 with it.

### 5.3 `LTAtransformBSpline(b, dst, lta)`

1. An array of other than one transform: the error `LTAtransformBSpline: numxforms != 1 not supported`
   and the program's end (`ERROR_BADPARM`).
2. No `dst`: with the transform's destination geometry valid (1), a volume of that geometry's extent,
   the source's type and the coefficients' frames, with the coefficients' header, then the geometry
   applied (`useVolGeomToMRI`); else, with `USE_AVERAGE305` in the environment (any value), on standard
   error `INFO: Environmental variable USE_AVERAGE305 set` and `INFO: Modifying dst c_(r,a,s), using
   average_305 values` (two lines), a volume as §5.1 step 2 with no Talairach file name, the centre
   `(-0.0950, -16.5100, 9.7500)` (each `double` stored as `float`), `ras_good_flag` 1 and its cached
   matrices recomputed; else `INFO: Transform dst volume info is not used (valid flag = 0).` on standard
   error and that volume with no Talairach file name.
3. By the array's type: `LINEAR_RAS_TO_RAS` (1): §5.2 with the matrix. `LINEAR_VOX_TO_VOX` (0): the RAS
   matrix `i2r · M · r2i` (in `float`, `i2r · M` first, then times `r2i`), `i2r` the destination
   geometry's voxel-to-RAS matrix when valid, else (with `INFO: LTA dst geometry information missing!`,
   `      We assume that the dst volume passed is the`, `      same as the dst for the transform.` on
   standard error) `dst`'s; `r2i` the source geometry's RAS-to-voxel matrix when valid, else (the same
   three lines with `src`) the coefficients'; then §5.2 with it. `LINEAR_PHYSVOX_TO_PHYSVOX` (2): the
   array changed to `LINEAR_RAS_TO_RAS` (`LTAchangeType`), then §5.2 with its matrix. Any other type
   (`REGISTER_DAT` included): the error `LTAtransformBSpline: unknown linear transform` and the
   program's end (`ERROR_BADPARM`).

## 6. Factor-of-two pyramids (the centred cubic spline filters)

The reduce filter `g` (21 taps) and the expand filter `h` (16 taps) of the centred cubic spline pyramid
(Brigger et al.), as FreeSurfer writes them (each decimal the nearest `double`):

`g` = 0.708792, 0.328616, -0.165157, -0.114448, 0.0944036, 0.0543881, -0.05193, -0.0284868, 0.0281854,
0.0152877, -0.0152508, -0.00825077, 0.00824629, 0.00445865, -0.0044582, -0.00241009, 0.00241022,
0.00130278, -0.00130313, -0.000704109, 0.000704784

`h` = 1.13726, 0.625601, -0.0870191, -0.159256, 0.0233167, 0.0426725, -0.00624769, -0.0114341,
0.00167406, 0.00306375, -0.000448564, -0.000820929, 0.000120192, 0.000219967, -0.0000322054, -0.00005894

### 6.1 Reducing a line (`n_in` samples to `n_in / 2`)

`m = 2 (n_in / 2)` (an odd last sample is left out), `K = 2m`. For each `k` in `[0, m)`: `y[k] = In[k] *
g[0]` (`In[k]` first); for `i = 1 .. 20`: `i1 = k - i`, `i2 = k + i`; `i1 < 0` becomes `(2m - 1 - i1) % K`
and then, if `>= m`, `K - i1 - 1`; `i2 > m - 1` becomes `i2 % K` and then, if `>= m`, `K - i2 - 1`; `y[k] =
y[k] + (In[i1] + In[i2]) * g[i]` (`y[k]` first; `In[i1]` first in the sum). Then `Out[q] = (y[2q + 1] +
y[2q]) * 0.5` (**the odd sample first**).

### 6.2 Expanding a line (`n` samples to `2n`)

`K = 2n`, `k0 = 15` (`(16 / 2) * 2 - 1`). For each `i` in `[0, n)`, `j = 2i`: `Out[j] = In[i] * h[0]`; for
`k = 2, 4, .., 14`: `i1 = i - k/2`, `i2 = i + k/2`; `i1 < 0` becomes `(2n - 1 - i1) % K` and then, if `>=
n`, `K - i1 - 1`; `i2 >= n` becomes `i2 % K` and then, if `>= n`, `K - i2 - 1`; `Out[j] = Out[j] + (In[i1]
+ In[i2]) * h[k]`. `Out[j + 1] = 0`; for `k = -15, -13, .., 15`: `i1 = i + (k + 1) / 2` (integer division
truncating toward zero); `i1 < 0` becomes `(2n - 1 - i1) % K` and then, if `> n - 1`, `K - i1 - 1`; then
`i1 >= n` becomes `i1 % K` and then, if `>= n`, `K - i1 - 1`; `Out[j + 1] = Out[j + 1] + h[|k|] * In[i1]`.
Then, from the top: `Out[j] = (Out[j] + Out[j - 1]) * 0.5` for `j = 2n - 1` down to 2 (`Out[j]` first,
each with the values before this step), `Out[1] = (Out[0] + Out[1]) * 0.5` (**`Out[0]` first**), `Out[0]
= 0.5 * Out[0]`.

### 6.3 `MRIdownsample2BSpline(src, dst)`

Extents out: `max(1, width / 2)`, the same for height and depth; frames kept.
1. Pass x into a `float` volume (`width_out × height × depth`, frames): each line along x of the source
   (`MRIgetVoxVal`, widened) reduced (§6.1) when `width > 1`, else copied; stored (§3.4).
2. Pass y into a second `float` volume (`width_out × height_out × depth`): each line along y of the first
   (read back), reduced when `height > 1`, else copied.
3. No `dst`: a volume of the out extent, the source's type and frames, with the source's header. A `dst`
   (given or made) of another extent or frame count: ends the program (`ERROR MRIupsample2BSpline: MRI
   Dest dimensions not correct!`, status 1; the upsampling's name, as FreeSurfer prints it).
4. Pass z into `dst`: each line along z of the second, reduced when `depth > 1`, else copied; stored by
   §3.4 (a `uchar` destination rounds and clips).
5. `imnr0` the source's, `imnr1 = imnr0 + depth_out - 1`; `xsize`, `ysize`, `zsize`, `thick`, `ps` each
   the source's times 2 (`float`; FS-963); the centre: the source's voxel-to-RAS matrix (`extract_i_to_r`)
   times `(width / 2 + 0.5, height / 2 + 0.5, depth / 2 + 0.5, 1)` (integer halves, as `float`s; the
   product in `float`), its first three elements `c_r`, `c_a`, `c_s`; the cached matrices recomputed.

### 6.4 `MRIupsample2BSpline(src, dst)` (no program calls it)

Extents out: `2 width` when `width > 1`, else 1; the same for height and depth; frames kept. No `dst`: a
volume of the out extent, the source's type and frames, with the source's header; a `dst` of another
extent or frame count: ends the program (`ERROR MRIupsample2BSpline: MRI Dest dimensions not correct!`,
status 1). All passes write into `dst` itself (§3.4), so each pass reads the previous one's values as
`dst`'s type stored them.
1. Pass x: each line along x of the source expanded (§6.2) when `width > 1`, else copied, into `dst`
   (for each frame, slice and source row).
2. Pass y: for each frame, source slice and `dst` column: `height > 1`: the line's first `height` samples
   read from `dst`, expanded, stored; else FS-962.
3. Pass z: for each frame, `dst` row and column: `depth > 1`: the first `depth` samples read from `dst`,
   expanded, stored; else FS-962.
4. `imnr0`, `imnr1` as §6.3; `xsize`, `ysize`, `zsize` each the source's divided by 2.0 (in `double`,
   stored as `float`; FS-963); the centre from `(width / 2.0 - 0.25, height / 2.0 - 0.25, depth / 2.0 -
   0.25, 1)` as §6.3; the cached matrices recomputed.

## 7. Defects (`larmorx_core::defects`; the register's FS entries)

- **FS-962** (rule A): in §6.4's passes y and z on an axis of one voxel FreeSurfer copies the *source's*
  line (`MRIgetVoxVal(src, column, ...)` over the destination's columns and rows, which reach past the
  source's extent: other voxels of the source or memory it does not own) over the lines the earlier
  passes expanded. The default keeps the expanded lines (nothing is copied when an earlier axis was
  expanded; when none was, the copy writes the values already there); `reproduce` copies as FreeSurfer
  does (noted).
- **FS-963** (rule D, pending `mri_robust_register`'s two-dimensional path): §6.3 and §6.4 change the
  voxel size of an axis of one voxel, which they do not resample (and §6.3's centre moves that voxel by
  half a source voxel). Both modes give FreeSurfer's geometry, noted.
- **FS-964** (memory): sampling (§4.1, §4.2) a B-spline whose degree is above 9, or below 0 on a volume of
  one voxel: FreeSurfer writes indices past its arrays or reads indices it never set. An error in both
  modes (`<function>: a B-spline of degree <d> (FreeSurfer reads or writes memory it does not own here)`,
  `ERROR_BADPARM`).

## 8. What is not part of the module

The file's other pyramid filters (the standard and L2 families, the other orders of the centred one) and
its standard reduce and expand: FreeSurfer never chooses them. Threads: every line of §3 and every voxel
of §5 is independent, so any thread count gives the same result.

## 9. Validation plan (black box)

- **The official programs** (FreeSurfer 8.2.0, one thread: `OMP_NUM_THREADS=1`): `mri_vol2vol --mov
  <volume> --targ <volume> --regheader --cubic --o <out>` (and `--interp cubic`) computes §3 with degree 3
  and samples §4.1 at every target voxel; `MRItoBSpline degree 3` appears on standard output. Synthetic
  inputs (up to 32^3, `float` and `uchar`, lines longer than the horizons of §3.3 along one axis, NaN and
  infinite voxels of different payloads, a volume one voxel thick) and small rotations or shifts in the
  registration compare the outputs bit for bit. `mri_convert --resample_type cubic` (cubic B-spline
  through the linear transform of a volume onto another grid) does the same through §4.1. §5.3 and §6.3
  are reached by `mri_robust_template` (its final resampling) and `mri_robust_register` (its pyramid),
  queue items 3.2-13 to 3.2-16.
- **The open probe** (to come from the spec finisher): calls of `MRItoBSpline`, `MRIsampleBSpline`,
  `MRIsampleSeqBSpline`, `MRIlinearTransformBSpline`, `MRIapplyRASlinearTransformBSpline`,
  `LTAtransformBSpline`, `MRIdownsample2BSpline` and `MRIupsample2BSpline` by name on volumes and
  points given in a line protocol, as `validation/investigations/freesurfer-volume-probe/` does for
  fs.volume.
- **Reference numbers (observed)**: §3.3's horizons and its one `pow` difference; a 256^3 `float` volume's
  coefficients take about half a second at one thread on the main machine, and hold one `float` volume of
  the source's extent beside the source.
