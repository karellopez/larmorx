# FreeSurfer 8.2.0's MRI structure: behaviour spec (`fs.mri`)

Written by the replica author of queue item 3.1-55 as its spec writer (`docs/porting-guide.md` §1):
behaviour, maths, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and
the official build's outputs, never code or routines. A clean-room implementer writes the clean room
(with `fs.volume`'s, queue C3.1-05) from this file, black-box runs of the official programs (and of
the probe `validation/probes/fs-mri/`, a black-box run of FreeSurfer's code) and the cited standards.

Status: **part 1** (§1-8): the geometry and its matrices, the volume, its allocation and messages,
voxel access, the header and geometry copies, the colour table's copy. **Part 2** (§9-16): the
tkregister and FSL matrices, world and voxel coordinates, the geometry's printing and comparisons,
the interpolation and precision names, the full copy, the volume's transform, the shapes'
equality; the validation plan's second part (§16).

## 1. Conventions

- The arithmetic and NaN conventions of `specs/freesurfer-maths.md` §1 and §7 (x86-64, SSE2, no
  fused operations; a NaN made from numbers is SSE's default NaN with the sign bit set, a NaN operand
  passes on quieted, the left operand's of two). "In double" below: the float fields widened, each
  operation in double, the result rounded once to float where it is stored.
- Integers converted to the C type `size_t` wrap around (a negative extent becomes a huge count);
  products of such counts wrap modulo 2^64.
- Messages go to standard output (`printf`, `std::cout`) or standard error; "an error" is FreeSurfer's
  error line (`error: <message>`, `specs/freesurfer-core.md`), "ends with status s" an exit.

## 2. The geometry (`VOL_GEOM`)

Fields: `valid`, `width`, `height`, `depth` (ints, no initial value: a new geometry's are
unspecified until set), voxel sizes `xsize`, `ysize`, `zsize` (floats, 1), `ras_good_flag` (0), the
direction cosines of columns `x_r, x_a, x_s` (-1, 0, 0), rows `y_r, y_a, y_s` (0, 0, -1) and slices
`z_r, z_a, z_s` (0, 1, 0), the centre `c_r, c_a, c_s` (0), the shears `s_r, s_a, s_s` (0), a file name
(1,024 bytes), and three optional matrices: the cached voxel-to-RAS (16 floats), the cached
RAS-to-voxel (a 4 x 4 matrix, absent when its inverse failed), a registration matrix.

- **Copying** a geometry into a new one copies every field and the cached matrices the source has,
  never the registration matrix (the copy has none).
- **Assigning** a geometry onto an existing one copies every field and the source's cached matrices;
  the destination's registration matrix is untouched. Where the source has no cached matrix,
  FreeSurfer keeps the destination's (computed from its old geometry): FS-59, by default the
  destination then has none either (it equals its source); `reproduce` keeps the old ones.

## 3. The voxel-to-RAS matrices

- **The scanner voxel-to-RAS matrix** of a geometry with a voxel base `b` (0 or 1) and an optional
  shear: the 4 x 4 float matrix whose column j (j = 1, 2, 3) is the j-th direction cosine times its
  voxel size (each product in double, stored as float), last row (0, 0, 0, 1). With the shear option
  and any shear above 1e-5, that 3 x 3 part is multiplied (the double-accumulated float product of
  `specs/freesurfer-maths.md`) by the identity with `s_r` at (1, 2), `s_a` at (1, 3), `s_s` at
  (2, 3); with the diagnostic bit `DIAG_INFO` set this shear matrix is printed first
  (`[DEBUG] MRIxfmCRS2XYZ() matshear:` then the matrix at precision 5). The last column is
  `c - M p` per row (in double: the float centre minus the float product row), where `p` is the
  column (width/2.0 + b, height/2.0 + b, depth/2.0 + b, 1) stored as floats and `M p` the
  double-accumulated product of the matrix as it stands (last column 0).
- `extract_i_to_r`: that matrix with base 0, no shear. `extract_r_to_i`: its inverse (the float
  matrix inverse of `specs/freesurfer-maths.md` §8), absent when the inverse fails.
- **The cache**: the voxel-to-RAS matrix as 16 floats (column by column) and the RAS-to-voxel
  matrix, recomputed from the geometry (`MRIreInitCache`, returns 0).
- `MRImatrixOfDirectionCosines`, `MRImatrixOfVoxelSizes`, `MRImatrixOfTranslations`: into a given
  4 x 4 matrix or a new one of zeros: the three cosines as columns 1-3; the sizes on the diagonal; the
  scanner matrix's last column and 1 on the whole diagonal; each also sets (4, 4) to 1; other
  elements of a given matrix are left as they were.
- **Setting the geometry from a matrix** (`MRIsetVox2RASFromMatrix`): the column lengths (in double,
  `sqrt(x^2 + y^2 + z^2)` summed left to right) are compared with the voxel sizes; a difference above
  0.001 in any prints on standard output `WARNING: MRIsetRas2VoxFromMatrix(): voxels sizes are
  inconsistent`, then `   (%g,%g) (%g,%g) (%g,%g) ` with each stored size and column length, then
  `This is probably due to shear in the vox2ras matrix`, `Input Vox2RAS ------` and the matrix
  (precision 5); the sizes themselves are not changed. The direction cosines become the columns
  divided by their lengths (in double), then the centre is set from the last column as below;
  returns 0.
- **The centre from the first voxel's position** (`MRIp0ToCRAS(r0, a0, s0)`): the scanner matrix
  with its last column replaced by the point (stored as floats), times (width/2.0, height/2.0,
  depth/2.0, 1) in the float product (float accumulation, `specs/freesurfer-maths.md` §8), gives the
  new centre; the cache is recomputed; returns 0.

## 4. The volume (`MRI`)

A volume is a geometry plus: `nframes`, its shape, `imnr0`, `imnr1`, start and end coordinates,
`thick`, `scale`, `ps`, `fov`, boundary index tables, a transform's file name, a MINC general
transform with pointers to its linear and inverse matrices and an ownership flag, `AutoAlign`,
`outside_val`, `mean`, `brightness`, `yinvert`, `dof`, per-frame parameters (type, TE, TR, flip, TI,
TD, ..., a 4 x 4 `m_ras2vox`), a colour table, a region, `tr`, `te`, `ti`, `flip_angle`,
`FieldStrength`, the phase-encode direction, `origRas2Vox`, `location`, the b-vector space, b-values
and b-vectors, `version` (1) and `intent` (0), two GCA-morph geometries, name fields, up to 1,000
command lines, the warp field's format (-1), GCA-morph spacing (1), `exp_k`, affine and labels, the
type, `ptype` (2) and the voxel counts. Types: 0 uchar, 1 int, 2 long (8 bytes), 3 float, 4 short,
5 bitmap, 6 tensor, 7 float complex, 8 double complex, 9 RGB (stored as int), 10 ushort.

- **Bytes per voxel** (`MRIsizeof`): 1, 4, 8, 4, 2, -, 4 (tensor), 8 (complex), -, 4, 2; for bitmap,
  double complex and unknown codes `size_t` -1.
- **Intents**: names MRI, LABEL, SHAPE, WARP, WARP_INV, TIME_SERIES for 0-5, UNKNOWN otherwise; the
  reverse gives -1 for any other name.
- **Allocation** of extent (w, h, d, nframes) and type, with or without voxels:
  1. For each extent that is not positive (nframes included when given; 1 otherwise) standard output
     gets `MRI::Shape::Shape(): volume size must be greater than 0 in every dimension`; the
     volume is made anyway.
  2. `xend = w/2.0`, `xstart = -xend` (likewise y, z), `imnr0 = 1`, `imnr1 = d`, `fov = w`, the
     region's extent (w, h, d) (its origin unspecified); counts per row, slice, volume and in all,
     and bytes, as `size_t` products (§1).
  3. FS-55: a type without a size is refused by default (an error `unsupported MRI data type:
     <type>`, status 1); with `reproduce` allocation goes on with the size -1 (and wrapped counts).
  4. A negative `nframes` ends with the error lines of `Cannot allocate memory` then `MRIalloc:
     could not allocate <n> frame` (FreeSurfer's C-style message, FS-01), status 254. Each frame
     starts zero with a 4 x 4 zero `m_ras2vox`. The cache is computed from the default geometry.
  5. Without voxels the volume ends here (no index tables; reading or writing its voxels: FS-58).
  6. With voxels: `ras_good_flag = 1`; the buffer of the computed bytes, zero (it fails beyond about
     2^47 bytes, as a wrapped size does). `depth * nframes` slices (in int): a negative count ends
     with the error `could not allocate memory for <n> slices` (status 1); with at least one slice, a
     negative height or a buffer that could not be made ends with `could not allocate memory for
     slice 1 out of <n>` (status 1). These failures set the C library's `errno` to "Cannot allocate
     memory", which the next C-style error message prints first.
  7. Index tables for columns, rows and slices: entries from -500 to extent + 499; at or below 0 the
     value 0, at or beyond the extent `extent - 1`, else the index. A table of negative length ends
     with `Cannot allocate memory` then `could not allocate <n> elt index array` (status 254).
- A volume made from a geometry takes its extent, then `ras_good_flag = 1`, its voxel sizes,
  cosines, centre and file name (not `valid`, not the shears), and recomputes the cache.
  `MRIallocFromVolGeom` does the same through `useVolGeomToMRI` (which also copies the extent).
- Freeing a volume that is already freed prints the C-style error `MRIfree: null pointer` and
  returns -4.

## 5. Voxel access

Voxel (c, r, s, f) of a volume of type T is the T at element `c + r W + s W H + f W H D` of the
buffer (little-endian), computed as `size_t` (§1).

- **Reading** (`MRIgetVoxVal`, a float): negative c, r or s give `outside_val`. A complex volume
  gives the real (mode 0) or imaginary (mode 1) part; another mode ends with `MRIgetVoxVal():
  unsupported operation <mode>` (status 253). The integer and float types give their value
  converted to float; tensor, bitmap and the others give -1e10.
- **Writing** (`MRIsetVoxVal`): a complex part is stored as given. Otherwise the value is clipped:
  uchar [0, 255], short [-32768, 32767], ushort [0, 65535], int to [-2^31, 2^31] as floats (so 2^31
  stays, then fails to round), long to [-2^63, 2^63] as floats; then rounded with FreeSurfer's
  `nint` (`specs/freesurfer-core.md`: x + 0.5 or x - 0.5 truncated, beyond the int range FS-12) for
  the integer types and stored (narrowed to the type: uchar and ushort keep the low bits of the
  int). Returns 0, or 1 (nothing stored) for tensor, bitmap and the others.
- **Indices outside the volume** are not checked (but negative c, r, s when reading). FS-52: where
  the element still lies in the buffer it is another voxel (a column past the end of a row is the
  next row's first voxel; a negative index when writing an earlier voxel): by default reading gives
  `outside_val` and writing stores nothing; `reproduce` reads and writes that voxel. FS-53: where it
  lies outside the buffer (and, for writes and complex volumes, where the row or slice index is
  outside the volume: FreeSurfer then follows a pointer from outside its row tables), FreeSurfer
  reads memory contents or writes outside the volume: both modes read `outside_val` by default and 0
  with `reproduce`, and write nothing.
- **Long volumes** (FS-54): FreeSurfer stores the value rounded to an int (`nint`, so -2^31 beyond
  the int range), then stores the same int again as 4 bytes at byte `8 (row start) + 4 c`, i.e. into
  the low (c even) or high (c odd) half of the row's voxel `c / 2`. By default the value is stored
  once, rounded (x ± 0.5 truncated) in the long range; `reproduce` does both stores.
- **Derivatives** (`MRIgetVoxDx`, `Dy`, `Dz`): `(V(+1) - V(-1)) / (2.0 * size)`, the difference of
  the two float reads in float, the division in double, returned as float.

## 6. The header copy (`MRIcopyHeader`)

Into a given volume, or a new volume without voxels of the source's extent, type and frames.
Copied: version, intent, `dof`, `mean`, the voxel sizes; the transform when the source has a linear
one (the destination then owns it: its pointers to the copy's linear matrices, the flag 1); the
transform's file name; `imnr0` and `imnr1` when the depths agree; `ptype`, `fov`, `thick`, `ps`,
`location`, start and end coordinates, flip angle, TR, TE, TI, the file name, cosines, centre,
`ras_good_flag`, `brightness`; the registration matrix (FS-56: FreeSurfer loses it when the
destination has none: by default it is copied; with no source matrix the destination's is
dropped); the subject, path, format and stem names; the cached matrices; `AutoAlign` and
`origRas2Vox` when the source has them; the command lines (the destination's replaced); the colour
table when the source has one (the destination's replaced; with `DIAG_SHOW` and verbose diagnostics
it prints `MRIcopyHeader(): source has ctab. Copying ctab ...`). The destination's counts and bytes
are recomputed from its own extent and type. Not copied: extent, type, frames, `valid`, shears,
`scale`, `yinvert`, `outside_val`, field strength, phase-encode direction, b-values and b-vectors,
the GCA-morph fields, the voxels. FS-60: a destination that owned a transform, given a source with
none, has its transform freed but keeps pointing at it in FreeSurfer; both modes drop the pointers
and the flag.

Also: `MRIcopyPulseParameters` (flip angle, TR, TE, TI); `MRIcopyVolGeomToMRI` (sizes, cosines,
centre; no extent, no name, the cache not recomputed); `MRIcopyVolGeomFromMRI` (sizes, cosines,
centre, extent, name, `valid = ras_good_flag`).

## 7. The colour table an `MRI` carries

A table: entries (each absent or name, colours as ints 0-255 and floats 0-1, tissue type, a voxel
count), the file name, a version, `idbase`, a tissue-type schema and an optional tissue-type table.
Freeing an absent table prints the C-style error `CTABfree: *pct was NULL`, returns -4. The deep
copy copies the name, version, schema, each entry's name, colours and tissue type, and the
tissue-type table (copied the same way); FS-57: FreeSurfer does not copy `idbase` (0) nor the counts
(memory contents): by default both are copied; `reproduce` gives 0 for both. Copying an absent table
prints `CTABdeepCopy: ct was NULL` and gives none.

## 8. Validation plan

- The probe `validation/probes/fs-mri/` (`build.sh`, then `check.py --seed <s> --n <n>`): seeded
  cases of 20 commands (allocation of every type and odd extents, volumes from geometries, the
  matrices of §3 with NaN, infinite and zero sizes and cosines, voxel reads and writes inside and
  outside the volume of every type, header copies with and without destination, matrices and colour
  tables, the colour table's copy, geometry copies and assignments, intents, sizes), each run in a
  child of FreeSurfer's code with its printed text; compare `reproduce` with FreeSurfer's lines but
  where the port notes FS-53 or FS-58, and `fix` only differing with a notice. Measured 2026-11-13:
  seed 7, 20,000 cases: 18,049 of 18,049 comparable lines identical, 1,951 not comparable.
- The port's test `fs_mri_official.rs`: seed 1, 400 cases, the SHA-256 of FreeSurfer's lines
  (`723d33a9...`); it passes with AArch64's NaN emulated (`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`).
- Later: the replays of the first programs that build volumes (`mri_convert`'s items), through the
  readers of `fs.io`.

## 9. The tkregister and FSL matrices

"The product" below is the float matrix product of `specs/freesurfer-maths.md` §8 (each element
accumulated in float), "the inverse" the float matrix inverse of the same file (absent when the
matrix is singular, not square or its inverse not finite). A product with an absent factor ends the
program with that product's messages (`MatrixMultiply(): m1 is null ` or `m2 is null ` with the
file and line on standard output, then the error `MatrixMultiply: m1 is null` or `m2 is null`, exit
status 252); a product whose factors do not fit prints its mismatch messages and gives nothing. Where
a chain below multiplies into its running result, a step whose sizes do not fit leaves the running
result as it was; where it assigns the result, the next step meets an absent factor.

- **The tkregister vox2ras** of a geometry: §3's scanner matrix (base 0, no shear) of a geometry with
  the same extent and voxel sizes, direction cosines (-1, 0, 0) for columns, (0, 0, -1) for rows,
  (0, 1, 0) for slices, centre 0. It is computed through a volume header of that extent and type
  `MRI_UCHAR`, so an extent that is not positive prints §4's shape messages each time.
- **FSL's vox2ras**: the voxel sizes on the diagonal, 1 at (4, 4), zeros elsewhere.
- **A geometry's six matrices**: the scanner vox2ras (base and shear as given) and its inverse; the
  tkregister vox2ras and its inverse; tkregister RAS to scanner RAS (below) and its inverse.
- **Tkregister RAS to scanner RAS** of a geometry: the product of the scanner vox2ras (base 0) and
  the inverse of the tkregister vox2ras, into a given 4 x 4 matrix or a new one (a given matrix of
  another size: the inverse's rule for such an output). When the tkregister vox2ras has no inverse
  (a voxel size of 0) the program ends in the product (the `m2 is null` messages). For a geometry
  value (`VGtkreg2RAS`): the same on a volume header made from the geometry (its extent, voxel sizes,
  direction cosines and centre; its shears and `valid` are not used). Scanner RAS to tkregister
  RAS: the inverse of that (absent when it fails); its output argument is ignored (a new matrix).
- **Tkregister registration to native** (`MRItkReg2Native(ref, mov, R)`): with T the scanner vox2ras
  (base 0) and K the tkregister vox2ras of each geometry, in this order: T of ref, T of mov, K of ref,
  K of mov, the inverses of K mov and of T ref; then `D = T mov x inv(K mov)`, times R (when given),
  times K ref, times inv(T ref), each product into D; then D inverted into itself. A geometry whose
  K has no inverse ends the program in the first product. FS-164: when that last inverse fails
  (R singular), FreeSurfer returns the product itself (or the inverse's infinities, written before
  the failure was seen) with no message; by default the program ends with the error
  `MRItkReg2Native: the registration cannot be inverted` (status 252); `reproduce` returns
  FreeSurfer's matrix.
- **Native registration to tkregister** (`MRItkRegMtx(ref, mov, D)`): T ref, T mov, K ref, K mov,
  the inverses of T mov and K ref; `R = K mov x inv(T mov)`, times inv(D) (when D is given; its
  inverse computed then), times T ref, times inv(K ref).
- **Tkregister registration from a vox2vox** (`MRItkRegMtxFromVox2Vox(ref, mov, V)`): no V: the
  previous with no D. Otherwise K ref, K mov, inv(K ref), `R = K mov x V` (a new matrix; nothing when
  V's rows are not 4, then the program ends in the next product), times inv(K ref).
- **Vox2vox from a tkregister registration** (`MRIvoxToVoxFromTkRegMtx(mov, targ, R)`): K mov, its
  inverse, K targ; `inv(K mov) x R x K targ` (the second product into the first's result), or
  `inv(K mov) x K targ` without R.
- **The old tkregister's fix** (`MRIfixTkReg(mov, R)`): 1,000 points P (a 4 x 1,000 float matrix) in
  a 256 mm cube: for each point in turn x, y, z = 256 (u - 0.5) in double, stored as float, from the
  process's `drand48` sequence (three numbers a point, 3,000 in all), w = 1. C = inv(K mov) x (R x P);
  the old truncation T: rows floor(C1), ceil(C2), floor(C3) (IEEE floor and ceil of the floats, signed
  zeros and NaN kept) and 1; the result `(K mov x (T x P')) x inv(P x P')`, P' the transpose. R of
  another size: the products' messages, then the end of the program in the next product, except R
  with four columns and another row count, where FreeSurfer crashes: FS-165, both modes the error
  `MRIfixTkReg: the registration matrix is not 4 x 4` (status 252).
- **FSL's registrations** (`MRIfsl2TkReg(ref, mov, M)`, `MRItkreg2FSL(ref, mov, R)`): with D the
  voxel sizes' diagonal of each geometry (1 at (4, 4)), and for FSL to tkregister inv(D mov) = 1 /
  size on the diagonal (in float). The environment variable `FSLOUTPUTTYPE` must be set: otherwise
  standard output gets `ERROR: trying to convert FSL registration matrix to FreeSurfer,` (or
  `FreeSurfer registration matrix to FSL,`) and `       but FSLOUTPUTTYPE variable is not set.`, and
  the program ends with status 1. Then `FSLOUTPUTTYPE <value> ` (a trailing blank). For `NIFTI`,
  `NIFTI_PAIR` and `NIFTI_GZ`: the line `fsl2TkReg: mov det = <%g>, ref det = <%g>` (`tkreg2FSL:` for
  the other direction) with the float determinants of both scanner vox2ras (base 0), and for each
  positive one the two lines `INFO: FSL2FreeSurfer: Mov volume is NIFTI with positive det,` (`Ref`
  for the reference) and `      applying LR flip to registration matrix.`, and a left-right flip:
  the (1, 1) element of that volume's D (of inv(D mov) for FSL's moving volume) negated and (1, 4) set
  to `xsize (width - 1)` (float), but `width - 1` for inv(D mov). FSL to tkregister: K mov, K ref,
  inv(K ref), inv(M) (M absent ends the program: `MatrixInverse: NULL input matrix!`), then
  `K mov x inv(D mov) x inv(M) x D ref x inv(K ref)` left to right, each result assigned. Tkregister to
  FSL: inv(D ref), K mov, inv(K mov), K ref; `D mov x inv(K mov) x R x K ref x inv(D ref)` left to
  right, then inverted into itself: absent when that fails.
- **One-based to zero-based voxels** (`MtxCRS1toCRS0(Q)`): the identity with 1 in the fourth column's
  first three rows, into Q (a new matrix when none); Q not 4 x 4: `ERROR: MtxCRS1toCRS0(): input
  matrix is not 4x4` on standard output and nothing.
- **Sphinx position** (`MRIhfs2Sphinx`): the R components of the three direction cosines and of the
  centre negated (the sign bit flipped, NaN too); the A and S components of each swapped; returns 0.

## 10. World and voxel coordinates

- **Voxel to scanner RAS** (`MRIvoxelToWorld`): through the geometry's cached voxel-to-RAS matrix
  (§3; computed and kept when the geometry has none, and the RAS-to-voxel one too): the position
  rounded to float, then per output row `((x c1 + y c2) + z c3) + c4`, `c` the cached matrix's columns
  (16 floats column by column), each product `position x element` and each sum `left + right` in
  float (the NaN operand rule of §1 in that order), widened to double. The cache is not refreshed: a
  geometry changed after its matrices were cached maps through the old ones.
- **Scanner RAS to voxel** (`MRIworldToVoxel`): the cached RAS-to-voxel matrix (computed and kept when
  absent, then the voxel-to-RAS one), times (x, y, z, 1) rounded to float (§9's product), the first
  three widened to double. A geometry whose matrix has no inverse ends the program in that product
  (`m1 is null`). **To voxel indices** (`MRIworldToVoxelIndex`): each coordinate rounded to the
  nearest integer, ties to even; beyond the int range `specs/freesurfer-maths.md`'s conversion rule
  (FS-17).

## 11. The geometry's printing and comparisons

- `vgprint(nocheck)`: when `valid` is 1 or `nocheck`: `volume geometry:`, with `nocheck`
  `valid   : <d>`, then `extent  : (<w>, <h>, <d>)`, and `voxel   `, `x_(ras) `, `y_(ras) `,
  `z_(ras) `, `c_(ras) `, `shears  ` each followed by `: (<a>, <b>, <c>)` with `%7.4f`, then
  `file    : <name>`; otherwise `volume geometry: info is either not contained or not valid.`, all on
  standard output.
- `geomprint(fmt, ...)`: the caller's formatted text (when given), then `        : Dimensions: <w> x
  <h> x <d>`, `        : voxel size: <%6.6f>, <%6.6f>, <%6.6f>`, and for r, a, s: `        : x_r =
  <%8.10f>, y_r = <%8.10f>, z_r = <%8.10f>, c_r = <%10.10f>`.
- **Inequality code** (`isNotEqualThresh(a, b, t)`): 0 when equal, else the first field that differs:
  1 `valid`, 2-4 width, height, depth, 5-7 the voxel sizes, 8-16 the direction cosines (x_r, x_a,
  x_s, y_r, ..., z_s), 17-19 the centre, 20-22 the shears; a float field differs unless the absolute
  value of its float difference is below t (NaN always differs). **Equality** (`==`): 1 when that code
  is 0 at the process's threshold `vg_isEqual_Threshold` (FLT_EPSILON as a double until a program
  sets it), else 0. The file name and the matrices are not compared.
- **The geometry matrix** (`toMatrix`): direction cosines as columns 1-3, the centre as column 4,
  (0, 0, 0, 1) below, into a given matrix or a new 4 x 4.
- `checkgeom(a, b, t, debug)`: true when an element of the two geometry matrices differs by more
  than t (the difference of the widened floats, in double); each such element prints
  `VOL_GEOM::checkgeom(): Volumes differ in geometry row=<r> col=<c> diff=<%.10lf> (thresh=<%g>)`
  (`[DEBUG] ` before it with `debug`).
- `diffvox2ras(m1, m2, t, msg, debug)`: true when an element of the two 4 x 4 matrices differs by
  more than t; with `debug`: `[DEBUG]VOL_GEOM::diffvox2ras(): <msg>` first, each such element as
  `                              : vox2ras differs at row=<r> col=<c> diff=<%.10lf> (thresh=<%g>),
  val1=<%.10lf> val2=<%.10lf>`, and when none differs `                              : vox2ras same`.
  `checkvox2ras(a, b, t, debug, sform)`: that of the two scanner vox2ras (base 0) with the message
  `TAG_RAS_XFORM vs MRI_XFORM`; when they differ and `debug`: with a sform, `NII_SFORM vs MRI_XFORM`
  (sform against b's) and `TAG_RAS_XFORM vs NII_SFORM`, then `VOL_GEOM::checkvox2ras(): vox2ras1
  (TAG_RAS_XFROM):` (sic), the matrix (precision 10), `VOL_GEOM::checkvox2ras(): vox2ras2
  (MRI_XFORM):`, the matrix, and with a sform `VOL_GEOM::checkvox2ras(): nii_sform:` and it.
- **Removing shears** (`shearless_components`): when a shear is above 1e-5 (compared in double), the
  geometry is set from its sheared scanner vox2ras (§3 with the shear, then the setting from a matrix,
  which prints its warning when the column lengths differ from the voxel sizes) and the shears set
  to 0.

## 12. Interpolation and precision names

- Interpolation codes: nearest 0, trilinear 1, sinc 2, cubic 3, weighted 4, cubic B-spline 5, vote
  6. From a name: its first three characters, case folded (ASCII), against `nea` (0), `tri` and `tli`
  (1), `sin` (2), `cub` (5); a name shorter than three characters matches nothing; else -1. To a
  name: 0 `nearest`, 1 `trilinear`, 2 `sinc`, 5 `cubic`, otherwise none.
- Precision (voxel type) names: `uchar` 0, `short` 4, `ushrt` 10, `int` 1, `long` 2, `float` 3,
  compared whole, case folded; else -1; and back (others: none).

## 13. The full copy (`MRIcopy(src, dst)`)

- Without a destination: a volume of the source's extent, type and frames (§4), or without voxels
  (one frame allocated, the frame count then set to the source's) when the source has none. The
  header is copied (§6) and the destination keeps its own `ptype`; a source without voxels ends the
  copy there.
- The same type: the voxels row by row (row y of slice `z + frame depth`, the source's depth used for
  both), each row's `width x size` bytes, the size 1, 2, 2, 4, 4, 4, 8 bytes for uchar, short, ushrt,
  float, rgb, int, long. FS-161: FreeSurfer copies `width` bytes for the other types (a tensor's or
  a complex volume's row is 4 or 8 times longer); by default the whole row; `reproduce` FreeSurfer's.
  FS-166: a tensor volume's rows, seen row by row, all start at its first voxel, so FreeSurfer's copy
  of a tensor volume writes only its first `width` bytes; by default the rows are where they belong;
  `reproduce` gives FreeSurfer's.
- Conversions element by element over the source's extent into the destination's row y of slice
  `z + frame dst_depth`: float to short, unsigned short and unsigned char through `nint`
  (`specs/freesurfer-core.md`; unsigned char first clipped at 255); unsigned char, short, int and rgb
  to float (exactly, an int rounded to the nearest float); short to unsigned char. FS-162: FreeSurfer
  wraps values outside the destination's range (the low 16 or 8 bits, two's complement for short);
  by default they are clipped to the range; `reproduce` wraps. Every other pair of types: voxel by
  voxel over the destination's extent (column outermost, then row, slice, frame innermost) with §5's
  read (mode 0) and write (mode 0), their defects included.
- The destination's file name is the source's.
- FS-163: a given destination of another extent or frame count is refused by default (the error
  `MRIcopy: a <w> x <h> x <d> x <f> destination for a <w> x <h> x <d> x <f> source`, status 252);
  with `reproduce`, FreeSurfer's placement where every row lands inside the destination, and in both
  modes the error `MRIcopy: the destination is too small for the source's voxels` where it would
  not. A destination without voxels for a source with them: FS-58, the error `MRIcopy: the
  destination has no voxels`.

## 14. The volume's transform

A volume may carry a MINC general transform (`larmorx_io::xfm`, MINC 1.5.1's) with its linear
matrix and inverse. The header copy (§6) copies it when the source has a linear one: the copy takes
the inverse flag into its matrices (the copy's forward matrix is the source's effective one, its flag
clear); the linear matrix and inverse of the copy are taken from it. A transform that is not linear
(FreeSurfer reads only linear ones): copying it prints `copy_and_invert_transform bad kind` on
standard error and taking its linear matrix `get_linear_transform_ptr` (its inverse
`get_inverse_linear_transform_ptr`), each ending the program with status 1.

## 15. Shapes

Two shapes are equal when their width, height, depth and frame count are (the product is not
compared).

## 16. Validation plan (part 2)

- The probe (§8) has 43 commands: part 1's 20 and tkreg (the six matrices, FSL's, the tkregister
  vox2ras), ras2s (tkregister to scanner RAS into a given or new matrix, of a geometry value),
  tk2nat, nat2tk, v2v, tkv2v (with absent, singular and odd-sized registrations), fixtk (a seeded
  `drand48`; the next number drawn after it printed), fsl2tk, tk2fsl (unset, NIFTI and other output
  types), crs1, sphinx, v2w (fresh, stale and recomputed caches; points with NaN and infinities),
  codes, vgprint, vgeq (one field perturbed, thresholds from 0 to 10, with and without debug and a
  sform), shearless, tomat, copy (every type pair, with and without a destination, header-only
  sources and destinations, other extents), shape. tk2nat, fsl2tk and tk2fsl are drawn with finite
  numbers only (the port does not yet give the NaN bits of the float determinant and of the inverse written in
  place that they print).
- Measured 2026-11-13: seed 7, 43,000 cases: `reproduce` identical on 40,724 of 40,724 comparable
  lines (2,276 not comparable: FS-53, FS-58, FS-163 where refused, FS-165); the default differs only
  with a defect's notice; the port's 43,000 lines are the same with AArch64's NaN emulated in both
  modes. Seed 1, 860 cases: 825 identical, 35 not comparable, the hash `15997e56...` of
  `fs_mri_official.rs`, passing with the emulation in a debug and a release build.
