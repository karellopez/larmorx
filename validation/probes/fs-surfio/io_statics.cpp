// SPDX-License-Identifier: Apache-2.0
// fs.surfio's probe, since relay 12: FreeSurfer 8.2.0's utils/mrisurf_io.cpp compiled into the
// probe from the unit oracle's source tree, unchanged (build.sh: its own compile command from the
// unit oracle's build, the same compiler, flags, definitions and prefix map), so that the probe
// can call its file-local functions, which the official library has no symbol for (or has
// inlined everywhere). This translation unit defines every external symbol of mrisurf_io.cpp, so
// the linker never takes the library's mrisurf_io.o: every mrisurf_io.cpp function the probe
// reaches (relay 11's `sfile` ones too) runs from this object.
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).

#include "mrisurf_io.cpp"

int fssurfio_read_curvature_into_array(const char *fname, int size, float **out)
{
  return MRISreadCurvatureIntoArray(fname, size, out);
}

int fssurfio_read_new_curvature_into_array(const char *fname, int size, float **out)
{
  return MRISreadNewCurvatureIntoArray(fname, size, out);
}

int fssurfio_read_curv_old_format(MRIS *mris, const char *fname, MRI *curvmri)
{
  return __mrisreadcurvoldformat(mris, fname, curvmri);
}

void fssurfio_apply_fsgii_read(char *file_to_read, const char *fname, int *filetype)
{
  __MRISapplyFSGIIread(file_to_read, fname, filetype);
}

void fssurfio_apply_fsgii_write(char *file_to_write, const char *fname, int *filetype)
{
  __MRISapplyFSGIIwrite(file_to_write, fname, filetype);
}

// relay 13 (3.1-48 part b): the file-local surface readers and quadCombine
MRIS *fssurfio_read_ascii_file(const char *fname)
{
  return mrisReadAsciiFile(fname);
}

MRIS *fssurfio_read_geo_file(const char *fname)
{
  return mrisReadGeoFile(fname);
}

int fssurfio_read_geo_file_positions(MRIS *mris, const char *fname)
{
  return mrisReadGeoFilePositions(mris, fname);
}

SMALL_SURFACE *fssurfio_read_triangle_vertex_positions_only(const char *fname)
{
  return mrisReadTriangleFileVertexPositionsOnly(fname);
}

int fssurfio_read_triangle_file_positions(MRIS *mris, const char *fname)
{
  return mrisReadTriangleFilePositions(mris, fname);
}

bool fssurfio_quad_combine(int quad[4], int vA[3], int vB[3])
{
  return quadCombine(quad, vA, vB);
}
