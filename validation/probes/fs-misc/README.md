# fs-misc: item 3.1-18's pieces against their originals

**Closed to FreeSurfer's clean-room implementers** (`facts.cpp` includes FreeSurfer 8.2.0's FNV
headers; the probe rule's `validation/probes/fs-*` family).

What it compares (`src/main.rs`):

- **miniz's inflater:** generated deflate streams (stored, fixed and dynamic blocks with random
  complete codes and repeat codes, raw and zlib; valid, cut at random, with flipped bits, random
  bytes, trailing bytes; one in three with an undefined length or distance symbol, a pattern
  outside an incomplete code, a single distance code, or a distance past the output) inflated
  four ways (`MZ_FINISH` at once; `MZ_SYNC_FLUSH` with output pieces of 1 byte and of a random
  size; the input byte by byte with `MZ_NO_FLUSH`, then `MZ_FINISH`) with an output room around
  the stream's length, by miniz 1.15 r4 as released (`libup.so`, C), FreeSurfer 8.2.0's copy
  (`libfs.so`, compiled as C++ as `nii_dicom_batch.cpp` includes it) and
  `larmorx_core::miniz` in both modes. Compared: the last return code, the bytes output,
  `total_in`, `adler`. miniz's dictionary is allocated zeroed (`shim.c`), so FS-284's reads of it
  are deterministic; the output buffer has 16 bytes of slack for FS-282's writes past it.
  Expected: libup = libfs everywhere; `reproduce` = miniz except FS-282's writes past the buffer
  (counted as explained); `fix` = miniz wherever it named no defect. Then `mz_uncompress` on
  zlib streams (some cut).
- **FNV:** random byte strings (lengths 0 to 3,000) hashed by FreeSurfer's `FnvHash::add` and
  `fnv_add(fnv_init(), ...)` (`facts fnv`) and by `larmorx_core::fnv::fnv1a_32_wide`.
- **libtiff's handlers:** random modules (or none) and messages through `TIFFError` and
  `TIFFWarning` of the unit oracle's `libtiff.a` (libtiff 3.6.1's files), standard error compared
  with `larmorx_core::libtiff::TiffMessages`.
- **Constants:** NrrdIO's `airFloatQNaN`, `airFloatSNaN`, `airFloatPosInf`, `airFloatNegInf`
  (`libnrrdio.a`) and Cephes' `PI` (`libcephes.a`) against `larmorx_core::nrrdio` and
  `f64::consts::PI`.

Build and run (the unit oracle `<workspace>/oracles/freesurfer-8.2.0-unit`, the pinned
`<workspace>/tags/miniz-1.15r4` and `tags/freesurfer-8.2.0`):

    validation/probes/fs-misc/build.sh <out>
    scripts/orchestration/boxed.sh 12 <out>/fs-misc <out> <scale>

The last line is `VERIFY-PASS` or `VERIFY-FAIL`; differences are listed on standard error.
`LXPROBE_ONLY_CASE=<n>` runs one inflate case. Scale 1: 6,000 streams (24,000 inflate runs per
implementation), 1,000 `mz_uncompress` runs, 20,000 FNV records, 4,000 TIFF messages.
