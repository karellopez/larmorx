# FreeSurfer 8.2.0's MRI structure: behaviour spec (`fs.mri`)

Written by the replica author of queue item 3.1-55 as its spec writer (`docs/porting-guide.md` §1):
behaviour, maths, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and
the official build's outputs, never code or routines. A clean-room implementer writes the clean room
(with `fs.volume`'s, queue C3.1-05) from this file, black-box runs of the official programs (and of
the probe `validation/probes/fs-mri/`, a black-box run of FreeSurfer's code) and the cited standards.

Status: **part 1** (§1-8): the geometry and its matrices, the volume, its allocation and messages,
voxel access, the header and geometry copies, the colour table's copy. **Part 2** (to come): the
tkregister and FSL matrices, the full copy, world and voxel coordinates, the geometry's printing and
comparisons, the interpolation and precision names.

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
