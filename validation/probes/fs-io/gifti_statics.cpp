// SPDX-License-Identifier: Apache-2.0
// fs.io's probe, piece 3: FreeSurfer 8.2.0's utils/gifti.cpp compiled into the probe from the unit
// oracle's source tree, unchanged (build.sh: the official compiler and flags), so that the
// probe can call its two file-local helpers, gifti_alloc_and_add_darray and
// gifti_get_DA_value_2D, which the official library has no symbol for. This translation unit
// defines every external symbol of gifti.cpp, so the linker never takes the library's gifti.o.
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).

#include "gifti.cpp"

giiDataArray *fsio_gifti_alloc_and_add_darray(gifti_image *image)
{
  return gifti_alloc_and_add_darray(image);
}

double fsio_gifti_get_DA_value_2D(giiDataArray *da, int row, int col)
{
  return gifti_get_DA_value_2D(da, row, col);
}
