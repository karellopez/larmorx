# MINC 1.5.1's volume input over netCDF 3.6.0-p1

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
`VOLUME_CACHE_THRESHOLD=0`). 35,000 files over nine seeds: every difference the runs found was fixed;
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
