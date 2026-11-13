# FreeSurfer 8.2.0's volume structure: behaviour spec (`fs.mri`)

Written by the replica author of queue item 3.1-55 as its spec writer (`docs/porting-guide.md` §1):
behaviour, maths, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and
the official build's outputs, never code or routines. Parts 1 and 2 (§1-16: the geometry and its
matrices, the volume, its allocation, voxels, header and colour-table copies, the tkregister and FSL
matrices, coordinates, printing and comparisons, names, the full copy, the volume's transform,
shapes) and part 3 (§17-22: the bounding box and the linear transform arrays, taken early from
`fs.volume`'s item 3.1-39, first written in a file of its own and joined here). Made ready for the
clean room by the spec finisher (FreeSurfer's foundations, track E, relay 3, 2026-11-13): the
black-box validation plan (§23) with its open probe, a read-through as a clean-room reader, the gaps
filled as behaviour (§0, §3, §4, §6, §9, §11, §14), one new defect (FS-264), the defects' summary
(§24). Amended by the spec finisher's relay 5 (2026-11-13) with what fs.mri's clean room measured
on the probe and the spec writer checked there: the shear's 4 x 4 product (§3), `RASFromSurfaceRAS_`
given a larger matrix (FS-331, the clean room's defect, §9), `surfaceRASFromRAS_` (§9), the
registrations' running results and FS-165's place (§9), the shape messages' counts (§9), the order of
the float-to-byte copy (§13); two new fixed batches (`matrices-shear`, `tkreg-given`). What black-box
runs confirmed is marked **(observed)**, with the comparison group behind it.

**Whoever implements it must not open:** FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`,
any other copy, the installed package's scripts, Python and libraries), the replica crates
(`crates-freesurfer/`, among them `larmorx-freesurfer-mri`), the documents written from FreeSurfer's
source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the replica's probes and checks
(`validation/probes/fs-*`, `validation/probes/x87-extended/`, `validation/investigations/freesurfer-core/`),
the unit oracle's build tree and tools (`<workspace>/oracles/freesurfer-8.2.0-unit/` except running
`probes/fs_mri_probe` and preloading `probes/guard_malloc.so`;
`validation/investigations/freesurfer-unit-oracle/`), the replay tool's and the unit oracle's Python
(`validation/src/larmorx_validation/freesurfer/`, `freesurfer_unit.py`: run, never read),
`docs/parts/` and the scratch folders of FreeSurfer's parts. **It may read** this spec and the specs
it builds on (`specs/freesurfer-maths.md` for the matrices and their arithmetic,
`specs/freesurfer-core.md` for the messages, rounding and conversions), the main package's open code
and documentation (for example `larmorx_io::xfm`, MINC 1.5.1's transforms, and
`larmorx_core::defects`), the open probe folders `validation/investigations/freesurfer-mri-probe/`
(calls only: its README says how to run it) and `validation/investigations/freesurfer-maths-probe/`
(its guard allocator), and run as black boxes the prebuilt probe
`<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe` (with `guard_malloc.so` beside it),
the official FreeSurfer 8.2.0 programs and the C library.

## 0. The functions covered

FreeSurfer's names, as the probe calls them (§23), and where they are specified:

| Section | FreeSurfer's functions |
|---|---|
| §2 | `VOL_GEOM`: its fields, its copy into a new geometry and its assignment |
| §3 | `MRIxfmCRS2XYZ`, `extract_i_to_r`, `extract_r_to_i`, `MRIreInitCache`, `MRImatrixOfDirectionCosines`, `MRImatrixOfVoxelSizes`, `MRImatrixOfTranslations`, `MRIsetVox2RASFromMatrix`, `MRIp0ToCRAS` |
| §4 | `MRIallocSequence`, `MRIallocHeader`, `MRI(vg, type, nframes, HeaderOnly)`, `MRIallocFromVolGeom`, `useVolGeomToMRI`, `MRIfree`, `MRIsizeof`, `MRI::intentName`, `MRI::intentCode` |
| §5 | `MRIgetVoxVal`, `MRIsetVoxVal`, `MRIgetVoxDx`, `MRIgetVoxDy`, `MRIgetVoxDz` |
| §6 | `MRIcopyHeader`, `MRIcopyPulseParameters`, `MRIcopyVolGeomToMRI`, `MRIcopyVolGeomFromMRI` |
| §7 | `CTABdeepCopy`, `CTABfree` |
| §9 | `MRIxfmCRS2XYZtkreg`, `MRIxfmCRS2XYZfsl`, the geometry's `get_Vox2RAS`, `get_RAS2Vox`, `get_Vox2TkregRAS`, `get_TkregRAS2Vox`, `get_RAS2TkregRAS`, `get_TkregRAS2RAS`, `RASFromSurfaceRAS_`, `surfaceRASFromRAS_`, `VGtkreg2RAS`, `VGras2tkreg`, `MRItkReg2Native`, `MRItkRegMtx`, `MRItkRegMtxFromVox2Vox`, `MRIvoxToVoxFromTkRegMtx`, `MRIfixTkReg`, `MRIfsl2TkReg`, `MRItkreg2FSL`, `MtxCRS1toCRS0`, `MRIhfs2Sphinx` |
| §10 | `MRIvoxelToWorld`, `MRIworldToVoxel`, `MRIworldToVoxelIndex` |
| §11 | `vgprint`, `geomprint`, `isNotEqualThresh`, `==` with `vg_isEqual_Threshold`, `toMatrix`, `checkgeom`, `diffvox2ras`, `checkvox2ras`, `shearless_components` |
| §12 | `MRIinterpCode`, `MRIinterpString`, `MRIprecisionCode`, `MRIprecisionString` |
| §13 | `MRIcopy` |
| §14 | the MINC general transform a volume carries, as the header copy copies it |
| §15 | `MRI::Shape`'s `==` and `!=` |
| §17-22 | `MRIboundingBox`, the transform type numbers, `LINEAR_TRANSFORM`, `LTA`, `LTAalloc`, `LTcopy`, `LTAcopy`, `LTAfree`, `initVolGeom` |

Not covered here: the volume readers and writers (`fs.io`), the rest of the transform functions
(`fs.volume`, queue item 3.1-40), the volumes' ITK conversion, hash and file writing. When a volume's one voxel buffer
cannot be made, FreeSurfer tries a buffer per slice instead (§4 step 6's messages when that fails
too); a volume stored so is not reached by the probe and not specified here (the probe prints such
a volume's voxels as `novox`).

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
  voxel size (each product in double, stored as float), last column 0, last row (0, 0, 0, 1). With
  the shear option and any shear above 1e-5 (each compared in double; a NaN or a negative shear is
  not above), this whole 4 x 4 matrix `M` is multiplied **on the right** by the 4 x 4 shear matrix
  `Sh` (the identity with `s_r` at (1, 2), `s_a` at (1, 3), `s_s` at (2, 3), each stored as float):
  `M x Sh`, the double-accumulated float product of `specs/freesurfer-maths.md` §2.5, every element
  summed over all four terms, so a non-finite cosine, voxel size or shear also reaches the last
  column and the last row through zeros times infinities (an infinite `s_r` makes (2, 2), (3, 2) and
  (4, 2) NaN, the last row then (0, NaN, 0, 1)) (**(observed)**: `matrices-shear`). With the
  diagnostic bit `DIAG_INFO` set the shear matrix is printed first, as a 4 x 4 matrix
  (`[DEBUG] MRIxfmCRS2XYZ() matshear:` then the matrix at precision 5, the float print of
  `specs/freesurfer-maths.md` §8.6) (**(observed)**). Then the last column's first three rows are
  `c - M p` per row (in double: the float centre minus the float product row), where `p` is the
  column (width/2.0 + b, height/2.0 + b, depth/2.0 + b, 1) stored as floats and `M p` the
  double-accumulated product of the matrix as it stands (its last column 0 without the shear, the
  product's with it); the last row is left as it stands.
- `extract_i_to_r`: that matrix with base 0, no shear. `extract_r_to_i`: its inverse (the float
  matrix inverse of `specs/freesurfer-maths.md` §8), absent when the inverse fails.
- **The cache**: the voxel-to-RAS matrix as 16 floats (column by column) and the RAS-to-voxel
  matrix, recomputed from the geometry (`MRIreInitCache`, returns 0).
- `MRImatrixOfDirectionCosines`, `MRImatrixOfVoxelSizes`, `MRImatrixOfTranslations`: into a given
  4 x 4 matrix or a new one of zeros: the three cosines as columns 1-3; the sizes on the diagonal; the
  scanner matrix's last column and 1 on the whole diagonal; each also sets (4, 4) to 1; other
  elements of a given matrix are left as they were (a given matrix larger than 4 x 4: its top-left
  4 x 4 so; one with fewer than 4 rows or columns: FS-264, below).
- **Setting the geometry from a matrix** (`MRIsetVox2RASFromMatrix`): the column lengths (in double,
  `sqrt(x^2 + y^2 + z^2)` summed left to right) are compared with the voxel sizes; a difference above
  0.001 in any prints on standard output `WARNING: MRIsetRas2VoxFromMatrix(): voxels sizes are
  inconsistent`, then `   (%g,%g) (%g,%g) (%g,%g) ` with each stored size and column length, then
  `This is probably due to shear in the vox2ras matrix`, `Input Vox2RAS ------` and the matrix
  (precision 5); the sizes themselves are not changed. The direction cosines become the columns
  divided by their lengths (in double), then the centre is set from the last column as below;
  returns 0. A matrix larger than 4 x 4: its top-left 4 x 4 is read; one with fewer than 4 rows or
  columns: FS-264.
- **The centre from the first voxel's position** (`MRIp0ToCRAS(r0, a0, s0)`): the scanner matrix
  with its last column replaced by the point (stored as floats), times (width/2.0, height/2.0,
  depth/2.0, 1) in the float product (float accumulation, `specs/freesurfer-maths.md` §8), gives the
  new centre; the cache is recomputed; returns 0.

- **FS-264 (a matrix of another size).** The functions of this spec that take a 4 x 4 matrix to
  write into or to read (`MRImatrixOfDirectionCosines`, `MRImatrixOfVoxelSizes`,
  `MRImatrixOfTranslations`, `toMatrix` (§11) and `RASFromSurfaceRAS_` (§9) given an output;
  `MRIsetVox2RASFromMatrix` given its input) address element (r, c) of a given matrix as they would
  in a 4 x 4 one, whatever its size. With fewer than 4 rows or 4 columns FreeSurfer writes into or
  reads other elements of the matrix and memory outside it: on the probe's matrices with fewer than
  4 rows it crashes, `MRIsetVox2RASFromMatrix` given 3 x 3 or 4 x 1 reads memory contents, and the
  writers given 4 x 1 or 4 x 3 change elements other than those they name (**(observed)**: the
  batches `matrices-sizes`, `geometry-sizes`, `tkreg-sizes`). Rule B: such a matrix makes no sense
  for these functions. **Both modes:** the C-style error `<function>: the matrix is not 4 x 4` (for
  example `MRIsetVox2RASFromMatrix: the matrix is not 4 x 4`), status 252, noted.

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
- **Freeing** a volume (`MRIfree`) returns 0 and leaves the caller without it; freeing one the
  caller no longer has (freed already) prints the C-style error `MRIfree: null pointer` (FS-01) and
  returns -4 (**(observed)**: `free2`).

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
  lies outside the buffer (and, for writes and complex volumes, wherever the row or slice index is
  outside the volume, the element's place being then taken from memory outside the volume's tables),
  FreeSurfer reads memory contents or writes outside the volume: both modes read `outside_val` by default and 0
  with `reproduce`, and write nothing.
- **Long volumes** (FS-54): FreeSurfer stores the value rounded to an int (`nint`, so -2^31 beyond
  the int range), then stores the same int again as 4 bytes at byte `8 (row start) + 4 c`, i.e. into
  the low (c even) or high (c odd) half of the row's voxel `c / 2`. By default the value is stored
  once, rounded (x ± 0.5 truncated) in the long range; `reproduce` does both stores.
- **A volume without voxels** (FS-58): reading or writing a voxel of it, its derivatives (and the
  bounding box, §17), FreeSurfer crashes. Both modes: the C-style error `<function>(): the volume
  has no voxels (FreeSurfer crashes here)` (for example `MRIgetVoxVal(): ...`), status 252, noted.
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
dropped); the subject, path, format and stem names; the cached matrices (the voxel-to-RAS matrix's 16
floats into the destination's, the RAS-to-voxel matrix by the matrix copy of
`specs/freesurfer-maths.md` §2.4: none when the source has none); `AutoAlign` and
`origRas2Vox` when the source has them; the command lines (the destination's replaced); the colour
table when the source has one (the destination's replaced; with `DIAG_SHOW` and verbose diagnostics
it prints `MRIcopyHeader(): source has ctab. Copying ctab ...`). The destination's counts and bytes
are recomputed from its own extent and type. Not copied: extent, type, frames, `valid`, shears,
`scale`, `yinvert`, `outside_val`, field strength, phase-encode direction, b-values and b-vectors,
the GCA-morph fields, the voxels. FS-60: a destination that owned a transform, given a source with
none, has its transform freed, yet FreeSurfer's copy still reports a linear transform and its
ownership, whose matrices are then freed memory (**(observed)**: `copyxfm`, memory lines); both
modes drop the transform and the flag. FS-252: a source without its cached voxel-to-RAS matrix
crashes FreeSurfer's header copy (FreeSurfer's own volumes always have that matrix: every allocation
of §4 computes it, so only a volume made otherwise lacks it); both modes copy none (the destination
then has none, as its source; its RAS-to-voxel matrix as above) and note FS-252.

Also: `MRIcopyPulseParameters` (flip angle, TR, TE, TI); `MRIcopyVolGeomToMRI` (sizes, cosines,
centre; no extent, no name, the cache not recomputed); `MRIcopyVolGeomFromMRI` (sizes, cosines,
centre, extent, name, `valid = ras_good_flag`); each returns 0.

## 7. The colour table an `MRI` carries

A table: entries (each absent or name, colours as ints 0-255 and floats 0-1, tissue type, a voxel
count), the file name, a version, `idbase`, a tissue-type schema and an optional tissue-type table.
Freeing an absent table prints the C-style error `CTABfree: *pct was NULL`, returns -4. The deep
copy copies the name, version, schema, each entry's name, colours and tissue type, and the
tissue-type table (copied the same way); FS-57: FreeSurfer does not copy `idbase` (0) nor the counts
(memory contents): by default both are copied; `reproduce` gives 0 for both. Copying an absent table
prints `CTABdeepCopy: ct was NULL` and gives none.

## 8. Validation plan

The black-box plan of the whole spec is §23.

## 9. The tkregister and FSL matrices

"The product" below is the float matrix product of `specs/freesurfer-maths.md` §8 (each element
accumulated in float), "the inverse" the float matrix inverse of the same file (absent when the
matrix is singular, not square or its inverse not finite). A product with an absent factor ends the
program with that product's messages (`MatrixMultiply(): m1 is null ` or `m2 is null ` with the
file and line on standard output, then the error `MatrixMultiply: m1 is null` or `m2 is null`, exit
status 252; `m1` is looked at first); a product whose factors do not fit prints its mismatch
messages and gives nothing. Where a chain below multiplies into its running result, a step whose
sizes do not fit leaves the running result as it was; where it also assigns the result (said below),
the running result is then absent and the next step meets an absent factor.

- **The tkregister vox2ras** of a geometry: §3's scanner matrix (base 0, no shear) of a geometry with
  the same extent and voxel sizes, direction cosines (-1, 0, 0) for columns, (0, 0, -1) for rows,
  (0, 1, 0) for slices, centre 0. It is computed through a volume header of that extent and type
  `MRI_UCHAR`, so each computation of it prints §4's shape message once per extent (width, height,
  depth) that is not positive. The geometry-valued conversions (`VGtkreg2RAS`, `VGras2tkreg`, the
  geometry's `get_TkregRAS2RAS` and `get_RAS2TkregRAS`) first make a volume header from the geometry,
  which prints them once more: twice per such extent in all (**(observed)**: one extent of 0 gives 7
  messages in the probe's `tkreg` command and 6 in `ras2s`, two such extents 14 and 12).
- **FSL's vox2ras**: the voxel sizes on the diagonal, 1 at (4, 4), zeros elsewhere.
- **A geometry's six matrices**: the scanner vox2ras (base and shear as given) and its inverse; the
  tkregister vox2ras and its inverse; tkregister RAS to scanner RAS (below) and its inverse.
- **Tkregister RAS to scanner RAS** of a geometry (`RASFromSurfaceRAS_`): the product of the scanner
  vox2ras (base 0) and the inverse of the tkregister vox2ras, into a given 4 x 4 matrix or a new one
  (a given matrix larger than 4 x 4: FS-331, below; smaller: FS-264). When the tkregister vox2ras has
  no inverse (a voxel size of 0) the program ends in the product (the `m2 is null` messages). For a
  geometry value (`VGtkreg2RAS`, the geometry's `get_TkregRAS2RAS`): the same on a volume header made
  from the geometry (its extent, voxel sizes, direction cosines and centre; its shears and `valid`
  are not used), the output argument passed on (FS-331 likewise).
- **Scanner RAS to tkregister RAS.** For a geometry value (`VGras2tkreg`, the geometry's
  `get_RAS2TkregRAS`): the inverse of `VGtkreg2RAS`'s new matrix (absent when it fails); its output
  argument is ignored (a new matrix). `surfaceRASFromRAS_` is computed otherwise: `K x inv(T)`, the
  inverse of the scanner vox2ras T (base 0, no shear) first, then the product of the tkregister
  vox2ras K and it (into the inverse's matrix: §8.1's product copies an input that is also its
  output first); a T without an inverse ends the program in that product (`m2 is null`). Not the
  inverse of tkregister RAS to scanner RAS: the two differ in last bits and in the signs of zeros
  (**(observed)**: `models.py surface` beside the probe, 40 geometries with oblique cosines:
  `surfaceRASFromRAS_` equal bit for bit to the float product of the K and inv(T) the probe's `tkreg`
  command prints in all 40, and different from `VGras2tkreg`'s matrix in all 40; the fs.mri clean
  room's comparison: 121 lines of `tkreg` differ when it is taken as the inverse, none as
  `K x inv(T)`).
- **FS-331 (a larger given matrix).** `RASFromSurfaceRAS_` (and `VGtkreg2RAS`, which passes its
  output on) given a matrix of at least 4 rows and 4 columns that is not 4 x 4: FreeSurfer writes the
  16 numbers of the tkregister vox2ras's inverse into it row by row from its first number, as if it
  were 4 x 4 (a 4 x 5 matrix gets them in its first three rows and the first number of its fourth;
  its other numbers are kept); it takes that inverse as failed when one of the given matrix's
  elements (r, c) with r, c ≤ 4, in the matrix's own rows and columns, is not finite, and the program
  then ends in the product (`m2 is null`, status 252); otherwise the product of the scanner vox2ras
  and the whole given matrix goes into it: with 4 rows the 4 x n product (`given`), with more rows
  nothing, after the product's mismatch messages (`MatrixMultiply(): m1/m2 dim mismatch` and the
  error `MatrixMultiply: m1 cols 4 does not match m2 rows <r>`) (**(observed)**: `tkreg-sizes`, and
  every one of `tkreg-given`'s 65 lines as this model, `models.py fs331`). Rule A: the input
  makes sense (the other helpers of §3 and §11 use a larger matrix's top-left 4 x 4) and the result is
  no conversion matrix. **Default:** the tkregister-to-scanner matrix into the given matrix's top-left
  4 x 4 (its other elements kept), FS-331 noted; **`reproduce`:** FreeSurfer's result, noted.
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
  another size (the 3,000 numbers are drawn first in every case):
  - not four columns: `R x P`'s mismatch messages, then the program ends in the next product
    (`m2 is null`; `m1 is null` when K mov has no inverse);
  - four columns and another row count: `R x P` is made; when K mov has no inverse the program then
    ends in the next product (`m1 is null`, status 252: FreeSurfer's own end, no defect); otherwise
    that product `inv(K mov) x (R x P)` prints its mismatch messages (`MatrixMultiply(): m1/m2 dim
    mismatch` and the error `MatrixMultiply: m1 cols 4 does not match m2 rows <r>`) and FreeSurfer
    then crashes: FS-165, both modes end there, after those messages, with the error `MRIfixTkReg: the
    registration matrix is not 4 x 4` (status 252) (**(observed)**: `registrations-sizes`).
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
  inv(K ref), inv(M) (M absent ends the program: `MatrixInverse: NULL input matrix!`; M not square:
  the inverse's error `MatrixInverse: matrix (<r> x <c>) is not square` (**(observed)**: 4 x 3), and
  M singular: no inverse; either way the program ends in the second product, `m2 is null`), then
  `K mov x inv(D mov) x inv(M) x D ref x inv(K ref)` left to right, each product into the running
  result and the result assigned. Tkregister to FSL: inv(D ref), K mov, inv(K mov), K ref;
  `D mov x inv(K mov) x R x K ref x inv(D ref)` left to right, each product into the running result
  and the result assigned, as in the other direction: R that does not fit prints its step's
  messages (R of 4 x 3: `MatrixMultiply(): m3 dim mismatch` and the error `MatrixMultiply: (4 x 4) *
  (4 x 3) != (4 x 4)`; R of 3 x 4: the `m1/m2 dim mismatch` messages) and leaves no running result,
  so the next product ends the program (`m1 is null`, status 252) (**(observed)**:
  `registrations-sizes`); K mov without an inverse ends it in the first product (`m2 is null`).
  Then the result is inverted into itself: absent when that fails.
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
  (0, 0, 0, 1) below, into a given matrix (larger than 4 x 4: its top-left 4 x 4, the rest kept;
  smaller: FS-264) or a new 4 x 4.
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
  FS-166: on a tensor volume FreeSurfer finds every row of both volumes at the volume's first voxel,
  so its copy changes only the destination's first `width` bytes, which become the source's first
  `width` bytes; by default every row is copied where it belongs; `reproduce` gives FreeSurfer's.
- Conversions element by element over the source's extent into the destination's row y of slice
  `z + frame dst_depth`: float to short, unsigned short and unsigned char through `nint`
  (`specs/freesurfer-core.md`, FS-12 beyond the int range); for unsigned char the rounded integer is
  **then** lowered to 255 when above it (rounding first: a float beyond the int range rounds, with
  `reproduce`, to -2147483648, whose low byte FS-162 keeps: 0, where clipping first would give 255;
  by default FS-12 saturates it and the result is 255, or 0 below) (**(observed)**: `copies`, 2 lines;
  444.9 gives 255, 4.0e25 gives 0); unsigned char, short, int and rgb
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
matrix and inverse, and a flag saying it owns it. The header copy (§6) first deletes the
destination's own transform when it owns one, then copies the source's when the source has a linear
matrix:

- a **linear** transform: the copy takes the inverse flag into its matrices (the copy's forward
  matrix is the source's effective one, its inverse the other, its flag clear); the destination's
  linear matrix and inverse are the copy's, and it owns the copy (**(observed)**: `copyxfm`, the
  16 numbers of each printed as given);
- a **concatenation** (of no transforms, on the probe) is copied, then taking its linear matrix ends
  the program: `get_linear_transform_ptr` + `\n` on standard error, status 1;
- **any other kind** (FreeSurfer's MINC code supports no other): `copy_and_invert_transform bad kind`
  + `\n` on standard error, status 1;
- deleting a destination's own transform of such another kind: `delete_general_transform` + `\n` on
  standard error, status 1 (**(observed)**: `copyxfm`).

A source without a linear matrix leaves the destination as the deletion left it (FS-60, §6).

## 15. Shapes

Two shapes are equal when their width, height, depth and frame count are (the product is not
compared).

## 16. Validation plan (part 2)

The black-box plan of the whole spec is §23.

## 17. The bounding box of a volume (`MRIboundingBox(mri, thresh, &box)`)

A box is six ints: the first corner `x`, `y`, `z` and the extents `dx`, `dy`, `dz`.

- Only the first frame is looked at. A voxel counts when its value is above `thresh`: for
  `MRI_UCHAR`, `MRI_SHORT`, `MRI_USHRT` and `MRI_INT` volumes the stored integer compared with
  `thresh` as an `int`; for `MRI_FLOAT` volumes the stored float compared with `thresh` converted to
  float (rounded to nearest: 2147483647 becomes 2^31). A NaN is never above.
- The corner is the smallest column, row and slice of a voxel that counts; the extent is the largest
  minus the smallest plus 1, in each direction separately.
- With no voxel above the threshold the corner is the last voxel (`width - 1`, `height - 1`,
  `depth - 1`) and the extents are `2 - width`, `2 - height`, `2 - depth` (the largest starts at 0).
  Kept (no defect: an empty box is not a box; callers that loop over the box do nothing).
- Returns 0. Any other type (`MRI_LONG`, `MRI_TENSOR`, `MRI_FLOAT_COMPLEX`, `MRI_RGB`, ...): the
  error `MRIboundingBox: unsupported type <type>` (FreeSurfer's error line, with its NUL byte:
  FS-01), returns `ERROR_UNSUPPORTED` (-3), and the box is left as the start makes it: the corner at
  the last voxel and the extents the volume's own (`width`, `height`, `depth`).
- A volume without voxels (a header) of a supported type with at least one row and one slice:
  FreeSurfer crashes (FS-58); both modes end with §5's error for `MRIboundingBox`. With no rows or
  no slices nothing is read: the empty box.

## 18. The transform type numbers

`TransformType`: `MINC` 0, `TKREG` 1, `GENERIC` 2, `UNKNOWN` -1. The linear transform array's
types: `LINEAR_VOX_TO_VOX` 0, `LINEAR_RAS_TO_RAS` 1, `LINEAR_PHYSVOX_TO_PHYSVOX` 2,
`TRANSFORM_ARRAY_TYPE` 10, `MORPH_3D_TYPE` 11, `MNI_TRANSFORM_TYPE` 12, `MATLAB_ASCII_TYPE` 13,
`REGISTER_DAT` 14, `FSLREG_TYPE` 15, `LINEAR_CORONAL_RAS_TO_CORONAL_RAS` 21.

## 19. The structures

- A **linear transform**: its centre `x0`, `y0`, `z0` and spread `sigma` (floats); three optional
  real matrices: the transform `m_L`, the gradient `m_dL` and the last step `m_last_dL`; a `type`
  (int); the source and destination geometries `src` and `dst` (`VOL_GEOM`, §2); a `label`
  (int).
- A **linear transform array** (`LTA`): `num_xforms` transforms and as many inverse transforms; a
  `type` (int); `fscale` (float); `subject` (a 1,024-byte name).

## 20. A new array (`LTAalloc(n, mri)`)

- The centre: without a volume (0, 0, 0); with one, the bounding box of §17 with threshold 70, and
  each coordinate the corner plus half the extent in integer arithmetic (the half rounded towards
  0: an empty box of width 4 gives `3 + (-2) / 2 = 2`), converted to float. The box's messages and
  failures are §17's (an unsupported type prints its error and uses the box as left).
- `n` transforms and `n` inverse transforms, each: the centre, `sigma` 10000, `m_L` the 4 x 4
  identity, `m_dL` and `m_last_dL` 4 x 4 zeros, `type` -1 (`UNKNOWN`), `label` 0, and both
  geometries as `initVolGeom` makes them from a zeroed geometry: not valid, 256 x 256 x 256 voxels of
  1 x 1 x 1 mm, the coronal direction cosines (columns -1, 0, 0; rows 0, 0, -1; slices 0, 1, 0), the
  centre 0, no shear, `ras_good_flag` 0, the name `unknown`, no cached matrices.
- The array: `num_xforms` `n`, `type` 0, `fscale` 0, an empty subject. `n` 0 gives an array of no
  transforms.
- A negative `n` (or one whose transforms do not fit in memory) ends the program: the error
  `Cannot allocate memory`, then the error `LTAalloc(<n>): could not allocate xforms` (with FS-01's
  NUL byte), status 254 (`ERROR_NOMEMORY`, -2). Only a transform count that does fit but whose
  inverse transforms do not would print `... could not allocate inverse xforms` instead.

## 21. Copies (`LTcopy(lt, dst)`, `LTAcopy(lta, dst)`)

`LTcopy(lt, dst)` copies into the existing transform `dst`:

- the centre and `sigma`;
- each of the three matrices as FreeSurfer's matrix copy into the destination's matrix: where both
  exist and the destination's has at least the source's rows and columns, the source's elements are
  written over the destination's top-left ones (a larger destination keeps the rest); where the
  destination's is smaller, it keeps its old numbers (FS-196); where the destination has none, it
  keeps none (FS-196: the copy is lost); where the source has none and the destination has one,
  FreeSurfer frees the destination's, yet the destination still shows a matrix, now freed memory
  (FS-198: printing, using or freeing it reads freed memory or crashes; both modes leave the
  destination without the matrix, noted);
- the two geometries by `VOL_GEOM`'s assignment (§2, with FS-59 for cached matrices);
- **not** the type and the label (FS-195: the destination keeps its own).

It returns the destination; with no destination it prints `ERROR: LTcopy() destination LT cannot be
NULL` and a newline on standard output and returns none.

`LTAcopy(lta, dst)`:

- With no destination, a new array of `lta`'s count from `LTAalloc(n, no volume)` (§20) is the
  destination (its transforms then start from §20's values: after FS-195 they keep type -1 and
  label 0).
- A destination with another count: `LTAcopy(): ERROR: number of xforms does not match (<n>,<m>)`
  (the source's count, the destination's) and a newline on standard output; returns none; the
  destination is unchanged.
- Otherwise the type, `fscale` and subject (copied as a C string over the destination's name) are
  copied, then every transform and inverse transform in turn by `LTcopy`, transform `i` before
  inverse `i`; returns the destination.

The defects, decided by rules A to D (`reproduce` gives FreeSurfer's results where they are
defined):

- **FS-195** (rule A): the copy's own type and label. By default copied with the rest; `reproduce`
  keeps the destination's (in a new copy -1 and 0). Noted where they differ.
- **FS-196** (rule A): a missing or smaller destination matrix. By default the destination takes a
  copy of the source's matrix (its size); `reproduce` keeps FreeSurfer's (none, or the old numbers).
- **FS-198** (use after free): both modes leave the destination without the matrix, noted.

## 22. Freeing (`LTAfree(&lta)`)

Frees the array, its transforms and their matrices; the caller's array becomes none; returns 0
(`NO_ERROR`). With no array (the caller holds none): FreeSurfer crashes (FS-197); both modes free
nothing and return 0, noted (**(observed)**: `ltafreenull`, `CRASH 11`).

## 23. Validation plan (black box)

The open probe folder `validation/investigations/freesurfer-mri-probe/` holds the probe's source
(calls of FreeSurfer's functions by name only), its README (the commands, the encodings and the
printed forms) and the comparison loop `compare.py`; the probe is prebuilt
(`<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe`, its build in `BUILD_INFO.txt`
there), and its guard runs preload the fs.maths probe's `guard_malloc.so` beside it.

### 23.1 The candidate and the loop

The clean room's own program (the *candidate*) takes `batch <file>` with the probe's lines and
prints the same lines (its library behind the commands; each line's streams its own; a line that
ends the program `EXIT <status>`). `compare.py compare` runs each group's batches on the probe four
times (plainly; plainly again in another process with the stack below each call zeroed; under the
guard allocator with a guard page after each allocation and the fill byte 90; before it, 165: the
fs.maths probe's README, "The guard runs"), through the oracle-output cache (one thread, the family
`freesurfer`), and once on the candidate with `LARMORX_UPSTREAM_DEFECTS` set to the mode; the
candidate's `upstream defect <ID> ...` lines on standard error are taken out and their IDs kept. A
line whose four probe results differ is a **memory** line (FreeSurfer read memory it did not write
or does not own: freed matrices (FS-60, FS-198), reads past a buffer (FS-53), the colour table's
counts (FS-57), matrices of another size (FS-264)). The batch `alloc-limits` (allocations of extents
or frame counts that are not positive, and of types without a size) runs plainly only: the guard
allocator cannot make the wrapped sizes those reach. Every call gets 10 seconds.

- `--mode reproduce`: **every line identical**, except (1) a line where FreeSurfer did not finish
  (`CRASH`, `TIMEOUT`) and the candidate did; (2) a memory line; (3) in the batches
  `matrices-sizes`, `geometry-sizes` and `tkreg-sizes`, a line where the candidate noted FS-264
  (its error holds in both modes).
- `--mode fix`: the same, and a line where the candidate noted a defect may differ (the notice, the
  corrected value).
- Then what the probe cannot show (FreeSurfer crashes or reads memory there, or larmorx defines both
  modes): FS-58's error (§5, §13, §17), FS-197's return, FS-198's dropped matrix, FS-264's error,
  FS-165's error (§9), FS-53's writes (nothing written) and reads (`outside_val`, or 0 with
  `reproduce`), FS-252's copy without the voxel-to-RAS matrix (the probe's volumes always have it):
  the clean room's own tests, against this spec's text.

### 23.2 The groups and their reference hashes **(observed)**

`compare.py refs --seed 7` on the prebuilt probe (2026-11-13, the reference machine: glibc 2.35,
Linux 6.8, x86-64; SHA-256 of each group's lines in order, a memory line or one where FreeSurfer
crashed counted by its place only, as `<memory>`; identical through the cache cold, from it warm
and without it, and in two more passes in other processes). Each group's cases come from a random
sequence of its own (`--groups` with a part of the list makes the same cases).

| Group | Batches | Calls | Memory or crash | Ends the program | What | SHA-256 |
|---|---:|---:|---:|---:|---|---|
| alloc | 2 | 1,014 | 0 | 99 | §4: every type at every extent from 1 to 4 and odd ones (0, -1), 0 to 3 frames, with and without voxels; volumes from geometries; freeing twice; sizes; intents | `0430b3d6c4b2b67eb827967c2913765877f12b5bf1b2a8ab56e572c813d3ac4b` |
| matrices | 3 | 999 | 35 | 0 | §3: the scanner matrix with and without shear (and `DIAG_INFO`), into given matrices, the inverses; setting from a matrix (singular, non-finite); the centre from the first voxel; the cache; matrices of other sizes (FS-264); fixed cases of the shear's product (`matrices-shear`: shears around 1e-5 and non-finite, non-finite cosines, sizes and centres with a shear, removing shears) | `3a910d6a7f455f57382d7fa5e8cb7819515b82b28e617af1fe761e3a7cb05b76` |
| voxels | 2 | 1,000 | 123 | 4 | §5: reads, writes and derivatives of every type with a size, clipping and rounding at the ranges' edges, NaN and infinities, complex parts and modes; one access outside the volume in a quarter of the lines (FS-52, FS-53) | `621802f15b5ed809cfc5f0acadf7aae4a9c7f3c96748f8db8eb8adef8385d2e7` |
| headers | 1 | 590 | 1 | 26 | §6, §14: the header copy with and without a destination (with `DIAG_SHOW` and verbose diagnostics), the pulse parameters, the MINC transform's kinds, the geometry copies | `9297e59c642c39c541b47044c1edc4b7fb0813036707dfe443daf1bd9d045ad2` |
| ctabs | 1 | 252 | 52 | 0 | §7: deep copies of colour tables with absent entries and tissue-type tables (the counts printed in a quarter: FS-57) | `556cec8321c725477409ad9126c227eacaa6413bd7d759c843a83917be3ae228` |
| geometry | 2 | 620 | 20 | 0 | §2, §9, §11: copies and assignments with and without cached and registration matrices (FS-59), the sphinx position, the geometry matrix (FS-264 apart) | `ebe70d6186f94be036702875bccbb58e353c9b07c3a7c13cdf168fac301e1b3b` |
| tkreg | 3 | 595 | 23 | 95 | §9: the tkregister and FSL matrices of geometries with extents from 0 to 6 and odd sizes, the surface RAS conversions, one-based voxels (FS-264 apart); fixed cases of FS-331 (`tkreg-given`: matrices larger than 4 x 4, non-finite numbers at the places the inverse leaves) | `215c63d41b3e1f130293838eed973e9978bdae5327e14a61a521ccc81ca69e16` |
| registrations | 2 | 1,080 | 6 | 341 | §9: tkregister to native and back, vox2vox, the old tkregister's fix (seeded `drand48`), FSL's registrations with every `FSLOUTPUTTYPE`, absent, singular and odd-sized matrices (FS-164, FS-165) | `dc931e92a06c41bd4e7687f370a04affe061554fd7da8ef74bc3655a44b5be66` |
| coords | 1 | 300 | 0 | 58 | §10: voxel to world and back with fresh, stale and recomputed caches, NaN and infinite points, rounding ties and the int range's edges (FS-17) | `516505da321827237107b585ab2cf0ef4b0d1563acf4ba89a5af176d8db597e8` |
| printing | 1 | 600 | 0 | 0 | §11: `vgprint`, `geomprint`, equality codes with one field perturbed at thresholds from 0 to 10, `checkgeom`, `checkvox2ras` with and without a sform and debug, `diffvox2ras`, removing shears | `893ca310ed2eb2539d9f329755a5a20a317fe83c61101710d385d83b698ee10a` |
| names | 1 | 218 | 0 | 0 | §12 | `b10f6df01a5c42ede620cf28e06c9477f3120dd3e7568285ae65cd37c6f4b78a` |
| copies | 1 | 600 | 37 | 0 | §13: every pair of types with a size, with and without a destination, header-only sources and destinations, other extents and frame counts (FS-58, FS-161 to FS-163, FS-166) | `936506603e1c2953e4c28a72a49974f6a9df916be6ae4e05b0ae3931e6554674` |
| shapes | 1 | 100 | 0 | 0 | §15 | `94e8090680e00ebb904f93132e322cfdb1fdacbe1ab02b7ca9e0423b389e337c` |
| bbox | 1 | 500 | 17 | 0 | §17: volumes of 1 to 4 voxels a side, one or two frames, filled or zero, up to three voxels set around the threshold (NaN, -1, large), thresholds 70, 0, -1, 69, 100, 255, 10^6 and the int limits; unsupported types; headers (FS-58) | `c4a2fbfcc4f6306e6a1a0b4d54da1a65603d5edf3cbef4d851e8c4225de9f939` |
| lta | 2 | 911 | 94 | 12 | §20-22: arrays of -5 to 3 transforms with and without a volume; copies into nothing, an array of the same or another count, transforms with absent or odd-sized matrices (FS-195, FS-196, FS-198); a copy into no transform; freeing none (FS-197) | `35a9b034c7ce90e2b6fabe9ea06d0d3c633a746ea890dcdda4c275d6093f5822` |

In all: 9,379 calls, 408 memory or crashed lines, 635 that end the program (the messages and exit
statuses of §3 to §22), none where FreeSurfer never returns. About 2 to 3 minutes for a cold `refs`
on the reference machine, seconds warm; `compare` as long again (the candidate's runs). The groups
`matrices` and `tkreg` with their fixed batches (relay 5): the same hashes cold, warm and without the
cache, and the probe as its own candidate identical in both modes.

### 23.3 The loop's own check **(observed)**

`compare.py compare` with the probe as its own candidate, in both modes: no unexplained difference
(identical lines but memory lines whose contents changed from process to process: `voxels` 18,
`headers` 1, `copies` 2, `lta` 88).

### 23.4 Golden cases and replays

One case per path in the cross-platform golden suite (CLAUDE.md rule 10): allocations of each type,
the scanner, tkregister and FSL matrices, voxel reads and writes at the ranges' edges, the header
copy, the full copy for a few type pairs, the coordinates, the equality codes, the names, the
bounding box and an array's copy. Later: the replays of the first programs that build volumes
(`mri_convert`'s items), through the readers of `fs.io`.

## 24. The defects (summary)

| ID | What | Rule | Default | `reproduce` |
|---|---|---|---|---|
| FS-01 | the C-style errors' NUL byte (`specs/freesurfer-core.md` §2) | A | none | the NUL |
| FS-12 | `nint` beyond the int range in voxel writes (`specs/freesurfer-core.md` §11) | undefined conversion | saturated | -2147483648 |
| FS-17 | world to voxel indices beyond the int range (`specs/freesurfer-maths.md`) | undefined conversion | as there | as there |
| FS-52 | a voxel index outside the volume that lands on another voxel | A | `outside_val`, nothing written | that voxel |
| FS-53 | a voxel index outside the buffer, or a row or slice outside the volume | out-of-bounds access | `outside_val`, nothing written | 0, nothing written |
| FS-54 | long volumes stored as int twice | A | stored once, rounded in the long range | both stores |
| FS-55 | allocation of a type without a size | B | the error | allocation with size -1 |
| FS-56 | the header copy loses the registration matrix | A | copied | FreeSurfer's |
| FS-57 | the colour table's deep copy leaves `idbase` and the counts | A | copied | 0 and 0 |
| FS-58 | voxels of a volume without voxels | crash | the error (both modes) | the error |
| FS-59 | a geometry's assignment keeps stale cached matrices | A | none (as the source) | the old ones |
| FS-60 | a transform freed by the header copy still reported | use after free | dropped (both modes) | dropped |
| FS-161 | the full copy's row length for tensor and complex types | A | the whole row | `width` bytes |
| FS-162 | the full copy's narrowing conversions wrap | A | clipped | wrapped |
| FS-163 | the full copy into a destination of another extent | B | the error | FreeSurfer's placement, the error where it leaves the destination |
| FS-164 | the tkregister-to-native inverse that fails | A | the error | FreeSurfer's matrix |
| FS-165 | the old tkregister's fix with a matrix of four columns and another row count | crash | the error (both modes) | the error |
| FS-166 | the full copy of a tensor volume's rows | A | every row in place | the first `width` bytes |
| FS-195 | an array's copy keeps the destination's type and label | A | copied | the destination's |
| FS-196 | a missing or smaller destination matrix in a transform's copy | A | the copy kept | none, or the old numbers |
| FS-197 | freeing no array | crash | nothing, 0 (both modes) | nothing, 0 |
| FS-198 | a transform's copy from a missing matrix frees the destination's | use after free | dropped (both modes) | dropped |
| FS-252 | the header copy from a source without its cached voxel-to-RAS matrix (§6) | crash | none copied (both modes) | none copied |
| FS-264 | the 4 x 4 helpers given a matrix with fewer than 4 rows or columns | B | the error (both modes) | the error |
| FS-331 | tkregister RAS to scanner RAS into a given matrix larger than 4 x 4 (§9) | A | the result in its top-left 4 x 4 | FreeSurfer's: the inverse laid out, its check, the product |
