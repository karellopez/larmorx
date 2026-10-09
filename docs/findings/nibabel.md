# nibabel as the NIfTI reference (nibabel 5.4.2)

The full parity record is [docs/validation/nifti-io.md](../validation/nifti-io.md). These
are the behaviours that cost time to find.

**The in-memory header is not the stored header.** After loading, `img.header` has
`scl_slope`/`scl_inter` reset and `vox_offset` recomputed. To compare stored fields, build
the header class from the raw 348/540 bytes. *Verified.* The parity reference does so.

**Load-time fixes** (`Nifti1Header` checks run with `fix=True`) silently change some fields
when a file is read: bad `pixdim[0]` (qfac), magic and `sizeof_hdr` issues. larmorx applies
the same fixes and reports them (`NiftiImage::fixes`). *Validated.*

**Byte order is guessed from `dim[0]`** (`guessed_endian`): if `dim[0]` is outside 1–7 in
native order, the header is taken as swapped. `dim[0] = 8` is a corner case nibabel handles.
*Validated.*

**Scaling:** `dataobj` is float64 when the header is scaled, otherwise the stored type.
Complex data are scaled with numpy's full complex arithmetic. *Validated.* (ITK scales
differently; see [itk-nifti.md](itk-nifti.md).)

**On writing**, nibabel stores `scl_slope = 1`, `scl_inter = 0` for unscaled data. It
keeps `pixdim` beyond the last dimension only if the shape does not change, so setting the
shape of a 1D or 2D image resets voxel sizes.
*Validated* (an early larmorx writer lost voxel sizes this way).

**Deliberate divergences** (larmorx is stricter or more robust):
- larmorx detects gzip from the content, not from the name;
- a single-file image with `vox_offset = 0` is rejected;
- complex `get_fdata` raises;
- RGB is not supported yet.
