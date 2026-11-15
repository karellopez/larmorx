# MINC 1.5.1's volume input and output over netCDF 3.6.0-p1

**Source-derived from MINC 1.5.1** (volume_io and libsrc; MNI's permissive notice,
`LICENSES/LicenseRef-MNI.txt`) **and netCDF 3.6.0-p1** (UCAR/Unidata's notice, `LICENSES/NetCDF.txt`):
closed to the implementer of a future clean room of these routines (porting guide §1). Found while
porting MINC 1 volume reading into `larmorx-io` (`larmorx_io::minc`) for FreeSurfer's foundations, track
C relay 27, 2026-11-15 (FreeSurfer 8.2.0's programs read `.mnc` volumes with it). The check:
`validation/probes/fs-nrrd-minc`. FreeSurfer's copy of these libraries (how it differs, measured as a
black box): `docs/findings/freesurfer-permissive.md` (closed to FreeSurfer's clean-room implementers);
this file describes MINC 1.5.1 and netCDF 3.6.0-p1.

## The upstream

- MINC 1.5.1's release tarball (`<workspace>/tags/minc-1.5.1`, SHA-256 `0b41d5bb...`): `volume_io/Volumes/`
  (`input_volume.c`, `input_mnc.c`, `volumes.c`, `multidim_arrays.c`, `evaluate.c`, `volume_cache.c`) and
  `libsrc/` (`netcdf_convenience.c`, `minc_convenience.c`, `image_conversion.c`, `dim_conversion.c`,
  `value_conversion.c`, `minc_error.c`).
- netCDF 3.6.0-p1, the version MINC 1.5.1's era used and FreeSurfer 8.2.0 bundles. Unidata no longer
  serves it; a mirror of Unidata's archive does (`https://www.gfd-dennou.org/arch/netcdf/unidata-mirror/
  old/netcdf-3.6.0-p1.tar.gz`, 2005-02-17, SHA-256 `a9e6105e...`, `<workspace>/tags/netcdf-3.6.0-p1`).
  Only its classic-format reader is ported.
- MINC 2 files are HDF5 files: netCDF 3.6.0 and MINC 1.5.1 read them as "Not a netCDF file", and so does
  the port (MINC 2 reading is not needed for FreeSurfer 8.2.0, whose copy has no HDF5 either).

## How volume_io reads a MINC 1 volume (what the port keeps)

- **Dimensions are matched by name** (`match_dimension_names`, three passes: exact names, then
  `any_spatial_dimension`, then empty names); a caller's order is the volume's (FreeSurfer asks for
  `xspace, yspace, zspace, time`, so the volume is indexed x first). A `vector_dimension` last is dropped
  (vector to scalar) and the volume then has one dimension fewer than the file, which fails when the
  caller asked for the file's count.
- **Geometry** comes from each dimension's variable: `step` (default 1), `start` (0), `direction_cosines`
  (the unit vector of its axis), `spacetype` (kept when every spatial dimension agrees). A malformed
  attribute (two steps, a character step) is an error MINC ignores: the default stays. The cosines of
  every dimension are read into **one buffer**: an attribute of fewer than three values keeps the
  values of the dimension read before it (FS-791). The voxel-to-world matrix is
  `directions[c][dim] * separation[c]` with the translation summed axis by axis; one or two spatial axes
  are completed by cross products, a singular set of directions becomes the identity, and a singular
  matrix makes the whole transform the identity (`create_linear_transform`).
- **Values**: the volume takes the file's type and sign (`signtype`, else unsigned for bytes) and its
  voxel range from `valid_range` (else `valid_max`/`valid_min`, else the type's range; reversed bounds
  swapped; integer bounds converted with `(int)` or `(unsigned int)`, float bounds through `float`). The
  image is read through an image conversion variable: each chunk over the dimensions `image-max` and
  `image-min` vary along gets its own scale from that slice's real range onto the volume's voxel range for
  the whole image's real range (`MI_icv_calc_scale`); values outside the file's valid range (widened by
  `10 * FLT_EPSILON` of its width) become the fill value, the voxel of real 0 (`promote_invalid_to_zero`).
  The whole image's real range is the MIN and MAX over every `image-min` and `image-max` value, with C's
  `MIN`/`MAX`: a NaN loses to the value after it and wins over the one before.
- **Conversions** are C's: `ROUND(x)` is `x + 0.5` (or `- 0.5`) truncated; a float widened and narrowed
  as x86 does (a signalling NaN becomes quiet); a NaN converted to an integer type gives x86's integer
  indefinite (0 for bytes, shorts and unsigned ints, `INT_MIN` for ints; FS-786).
- **Slabs**: the volume is read in slabs of at least 10,000 values (the fastest dimensions; one fewer
  dimension when that passes 400,000), each slab through `MI_var_loop`'s pieces of at most 10,000 bytes,
  each piece through netCDF's runs of at most one block.
- **netCDF's reads** go through posixio's two-block buffer (`px_get`), blocks of the file system's
  preferred size (at least 8,192 bytes). A header element whose window passes two blocks fails with
  "Argument list too long" (a name of about 12 KB or more). Past the end of a file the buffer gives
  zeros, except where it moves forward one block while its count of valid bytes ends in the first: the
  old first block stays under the new offset, so a file shorter than its header reads bytes of an
  earlier block (FS-793). The port emulates the buffer's state, call by call.
- **ids are file descriptors**: netCDF's messages print them (`ncvarget: ncid 3; varid 5: ...`); the port
  numbers its files from `Session::first_fd` (3) as `open` would.
- **Errors**: netCDF prints with `NC_VERBOSE` and ends the program with `NC_FATAL` (exit status 3).
  volume_io reads the header with `ncopts` 0 (silent) and the data with `NC_VERBOSE | NC_FATAL`: a MINC
  error during the data ends the program ("MINC package entry point"). A file that does not exist, is
  not netCDF or has no image gives volume_io's messages and status 1.
- **The volume cache**: a volume of more bytes than `VOLUME_CACHE_THRESHOLD` (80,000,000 by default) is
  read on demand through a second opening of the file, made on the volume the first opening set up. For
  a float volume that first opening has set the voxel range to the real range, so the second converts
  out-of-range voxels differently: a large float volume reads other values than a small one. The port
  reads the whole volume through such a second opening.

## Defects (D13; register entries FS-774 to FS-796)

| ID | What MINC or netCDF does | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-774 | a compressed file is decompressed with `gunzip -c` (`bunzip2 -c`, `zcat`, `pcat`) through the shell, the name unquoted (c) | decompressed in the process | the same (a shell command is never run) |
| FS-776 | a version 1 variable whose begin is 2 GiB or more is read at a negative offset: memory beside the buffer, or Invalid argument (a) | Invalid argument | the same |
| FS-777 | a variable whose dimension sizes multiply to a multiple of 2^64 divides by zero (a) | its length is the format's largest | the same |
| FS-778 | a header list (or an attribute) longer than the file holds is read on through zeros until memory runs out (a) | refused as an allocation that failed (more than 2^24 entries, or an attribute of more than 256 MiB past the file's end) | the same |
| FS-779 | a file without an image variable (or of more dimensions than a volume can have) gives a dimension count never written: an error or a crash by memory (a) | an error | the same |
| FS-780 | a time dimension without `spacing` is regular or irregular by memory (a) | regular | regular |
| FS-781 | an image of type char (or unknown) leaves the volume's type never written (a) | an error | no type: the allocation messages and the ICV error that ends the program |
| FS-782 | a dimension name over 64 bytes overflows volume_io's buffer (a) | read whole | the same |
| FS-783 | more than 512 dimensions overflow the dimension arrays (a) | an error | the same |
| FS-784 | an empty string attribute is checked one byte before the buffer (a) | an empty string | the same |
| FS-785 | `image-max`/`image-min` over a dimension the image lacks are read at an index never set (a) | index 0 | the same |
| FS-786 | a NaN converted to an integer voxel (a) | 0 | x86's value |
| FS-787 | a valid-range bound outside its integer type converted with C's undefined conversion (a) | clamped | x86's value |
| FS-789 | a volume of 2^32 voxels or more counts them in 32 bits (a) | allocated whole | the same |
| FS-790 | a dimension of size 0 (a record dimension without records) reads through null pointers (a) | an empty volume | the same |
| FS-791 | a `direction_cosines` of fewer than three values keeps the previous dimension's (c) | none (the default cosine, as for any malformed geometry attribute) | the previous dimension's |
| FS-792 | ... on the first dimension with cosines: memory never written (a) | none | 0 |
| FS-793 | a file shorter than its header reads past its end bytes of an earlier block where netCDF means zeros (c) | zeros | the buffer's bytes |
| FS-794 | with a dimension of size 1 hiding the volume's order (one x slice of a `zspace, yspace, xspace` file read as x, y, z), each slab is read as one run from its first voxel: slabs overwrite each other, the rest is never written (c) | each value in its place | the runs |
| FS-795 | ... a run past the end of the volume's array writes memory beside it (a) | (as FS-794) | stopped at the array's end |
| FS-796 | ... the voxels no slab wrote keep the memory the array was given (a) | (as FS-794) | 0 |

(FS-775 and FS-788 are FreeSurfer's copy's: `docs/findings/freesurfer-permissive.md`.)

## The numbers (2026-11-15)

The probe generates netCDF classic files of three kinds (well-formed MINC files of every type, sign,
valid range, slice range, geometry, dimension order, record dimension and version; the same with
bytes of the header changed or the file cut; the special cases: empty, not netCDF, HDF5, gzip, a
directory, a missing file, char and unknown types, long and empty names, unknown attribute types,
global attributes standing for missing dimension variables, `image-max` over a foreign dimension,
numeric strings) and reads each with the port (`reproduce`) and with MINC 1.5.1 and FreeSurfer 8.2.0's
copy in a forked child (one in ten MINC 1.5.1 runs with the cache forced on,
`VOLUME_CACHE_THRESHOLD=0`). 37,500 files over eleven seeds: every difference the runs found was fixed;
with the final code, seeds 8 and 11 (8,000 files) gave 12,418 comparisons identical (6,113 of them
successful reads) and none different, the cases a defect of undefined behaviour makes incomparable
counted apart (the probe's README). The unit test of 120 seeded files gives MINC 1.5.1's own hash of
its dumps on Linux, under AArch64's NaN rules (emulated) in debug and release, and on Windows x64.

## Not ported (gaps, each reported)

- volume_io's free-format input (a name ending in `.fre`): an error says so.
- The vector dimension's conversions (to colours, or averaged to a scalar), reached only when the caller
  asks for fewer dimensions than the file has: an error says so.
- With the cache, a short file's values past its end follow the order in which the caller reads voxels
  (the cache loads blocks on demand); the port reads slabs.

## Writing a volume (track C relay 28, 2026-11-15)

volume_io's `output_volume` (`output_volume.c`, `output_mnc.c`) over MINC's library (`micreate`, the standard
variables, the attribute puts, `mivarput`, `miicv_put`) and netCDF 3.6.0-p1's classic writer (`nc_create`, the
definitions, `nc_enddef` with `NC_begins`, the header writer and the fill, `nc_put_vara`, `nc_close`), as
FreeSurfer 8.2.0's `mincWrite` calls it: `create_volume` with the names `xspace, yspace, zspace[, time]`, its
sizes, `alloc_volume_data`, separations, `set_volume_direction_cosine` of each axis, `set_volume_translation`
(the centre voxel at `c_ras`), every voxel set, then `output_volume(name, type, sign, min, max, volume, "",
NULL)`. Ported in `larmorx_io::minc` (`output.rs`, `mwrite.rs`, `ncwrite.rs`, `Volume`'s setters).

What decides the file's bytes, kept by the port:

- **The layout.** The root variable first (`rootvariable`, an int with `varid`, `vartype` `group________`,
  `version` `MINC Version    1.0`, `parent` ""), the dimensions in the volume's order, a variable per spatial
  dimension (`step`, `start`, `direction_cosines` only when it is not the axis's own, `units` `mm`,
  `spacetype` unless unknown; `comments`, `spacing` `regular__`, `alignment` `centre`), `image-min` and
  `image-max` (scalar doubles with `_FillValue` 0 and 1 when a global real range exists, which
  `output_volume` always sets from the volume's real range unless it is not a number or empty; else one value
  per slice over the image's slower dimensions), the global `history` (the caller's string appended: "" gives
  a one-byte NUL attribute), and the image last (`complete` `false` then `true_`, `signtype`, `valid_range`:
  floats for a float image, doubles otherwise, left out when the range is empty). The image's `image-max` /
  `image-min` pointers (`--->image-max`), the root's `children` (`image`) and the parents.
- **netCDF's header and offsets.** Variables begin right after the header (`nc_enddef`'s alignment 1), each
  `len` a multiple of 4 for bytes and shorts; every non-record variable is filled with its fill value
  (`_FillValue`, else netCDF's: `0x81`, `0x8001`, `0x80000001`, `0x7cf00000`, `0x479e000000000000`) before the
  data are written, the padding included; the `complete` attribute rewritten at the close (same size) makes the
  header written twice. The file is written through posixio's two-block buffer call by call: a program that ends
  without closing (an exit inside the library) leaves on disk what the buffer had paged out (the empty
  32-byte file `nc_create` made, after a failure at the end of the definitions).
- **The conversion.** The image goes through an image conversion variable of the volume's type with
  normalisation on: the volume's voxel range onto the file's valid range for the image's real range (for
  mincWrite's equal types and ranges, the identity; out-of-range integers are clamped to the type and
  rounded as MINC's `MI_FROM_DOUBLE` does; floats pass unchanged, NaNs too). The volume's voxels are stored
  as C's casts store a double on x86-64 (`set_volume_voxel_value`: an integer type through a 32-bit truncation,
  unsigned int through a 64-bit one).
- **The NaNs.** A NaN in the geometry (a separation, a direction cosine, the world position) reaches the
  file's `start` attributes; their bits follow volume_io's x86-64 object code (GCC 8.4.0 at `-O3`):
  `set_volume_direction_cosine` sums `(x x + y y) + z z` and divides each component by the root;
  `set_volume_translation` subtracts `separation * cosine * voxel` per axis, the second axis's z component as
  `cosine * separation` (GCC swapped that product); `transform_point` adds each row from the left, the last
  column after the three products. The port follows that order with SSE's NaN rules on every platform
  (`larmorx_core::ItkReal`'s `sse_*`).
- **Messages.** As MINC 1.5.1 prints them with `ncopts` `NC_VERBOSE` during the definitions and
  `NC_VERBOSE | NC_FATAL` after: a file that cannot be created gives `nccreate: filename "...": <errno text>`,
  `micreate: MINC package entry point`, `Error: opening MINC file "...".` and `output_volume` returns an error;
  a failure inside the library after the definitions ends the program with status 3. A zero-length direction
  cosine is refused with a warning (the axis keeps its default).

| ID | what (class) | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-797 | FreeSurfer's copy cannot write a volume with a time dimension (more than one frame): `convert_spatial_axis_to_dim_name, bad axis`, exit status 1, before the file is created (c; `docs/findings/freesurfer-permissive.md`) | written as MINC 1.5.1 writes it (rule A) | the message and the exit (FreeSurfer's dialect, track A) |
| FS-798 | a volume with a dimension of size 0: its pointers are set up into an allocation of no bytes; allocating it (MINC 1.5.1) or writing it (FreeSurfer's copy) crashes or reads memory (a) | no voxels; the file written with the dimension of size 0 (netCDF's record dimension: a later image dimension of size 0 is refused, `NC_UNLIMITED in the wrong index`) | (the same) |
| FS-799 | a volume whose voxels were never allocated, written: volume_io follows the array's pointer table (two or more dimensions) or reads the values from a null or unset pointer (a crash) unless the conversion fails first (a) | not written: `output_volume` returns an error after the definitions | (the same) |
| FS-800 | a volume without spatial axes whose geometry was changed: `compute_world_transform` returns without a transform and the volume takes one from memory never written (the output then says `Cannot output null or non-linear transforms.  Using identity.` and `delete_volume` reports `Internal error:  delete_general_transform`) (a) | the earlier transform (the identity) kept | (the same) |
| FS-801 | a volume created without a type (`MI_ORIGINAL_TYPE`): its voxel range and sign were never written, so the file's type, valid range and conversion come from memory contents (a) | 0, 0, unsigned (no data type: nothing is allocated) | (the same) |

(A type `set_volume_type` has no case for, a char volume, is FS-781 on the writing path too.)

### The numbers (2026-11-15)

`validation/probes/fs-nrrd-minc --only write` (its README): every case set up and written by MINC 1.5.1 and by
FreeSurfer 8.2.0's copy in a forked child and by the port (`reproduce`), the file's bytes, the step dumps,
standard output, standard error and the exit compared. 60 % of the cases have mincWrite's shape (compared with
both originals, FreeSurfer's as measured), the rest any call (MINC 1.5.1 only): every type and sign, ranges
empty, reversed and not a number, steps zero, negative, infinite and not a number, cosines zero, oblique, NaN
and huge, sizes up to 300,000 voxels (slabs of more than one dimension), dimension orders and names, file names
missing, a directory, under a file, empty, with blanks, `$VAR`, over an older file. Final code: seeds 2 to 9,
2,000 cases each but seed 2 (300): see the probe's README for each seed's count; 0 different, the cases a defect
of undefined behaviour makes incomparable (FS-781, FS-789, FS-798 to FS-801) counted apart. The unit test
`seeded_volumes_written` (80 cases) gives MINC 1.5.1's own hash.

### Not ported (gaps, each reported)

- `output_modified_volume` with an original file (copying its auxiliary variables and attributes):
  `output_volume` writes without one; FreeSurfer never passes one.
- The progress report volume_io prints when a write has taken more than 5 seconds (a measure of time).
- The volume cache's output file (a cached volume is allocated whole: the same file).
- `NC_SHARE`'s unbuffered I/O (MINC never asks for it).
- The NaN bits of volume_io's Gaussian elimination (`compute_transform_inverse`) when a matrix with NaN
  entries meets a NaN made by its arithmetic: GCC's vectorised loops pass on either operand's NaN by the
  element's place; FreeSurfer's copy uses its own inverse (track A's port, exact). Not met in the runs.

