/* SPDX-License-Identifier: Apache-2.0 */
/* What MINC 1.5.1's volume_io needs from the rest of MINC for the .xfm code, in this probe:
 * micreate_tempfile as libsrc/netcdf_convenience.c defines it with mkstemp ($TMPDIR, else P_tmpdir,
 * "/minc-XXXXXX"; NULL when the file cannot be made), and MINC volumes, which the probe never reads
 * or writes (a grid transform's volume fails here; the .xfm comparisons leave grids to their parse). */
#include <internal_volume_io.h>
#include <unistd.h>

char *micreate_tempfile(void) {
    const char pat[] = "/minc-XXXXXX";
    const char *dir = getenv("TMPDIR");
    char *name;
    int fd;
    if (dir == NULL) dir = P_tmpdir;
    name = malloc(strlen(dir) + sizeof pat);
    if (name == NULL) return NULL;
    strcpy(name, dir);
    strcat(name, pat);
    fd = mkstemp(name);
    if (fd >= 0) close(fd);
    else {
        free(name);
        name = NULL;
    }
    return name;
}

/* MINC volumes: never read or written by the probe. Each prints a marker the driver's dialect
 * prints too, so a grid transform is compared up to the volume's file name. */
Status input_volume(STRING filename, int n_dimensions, STRING dim_names[], nc_type volume_nc_data_type,
                    BOOLEAN volume_signed_flag, Real volume_voxel_min, Real volume_voxel_max,
                    BOOLEAN create_volume_flag, Volume *volume, minc_input_options *options) {
    fprintf(stderr, "probe: input_volume(%s)\n", filename);
    return ERROR;
}
Status output_volume(STRING filename, nc_type file_nc_data_type, BOOLEAN file_signed_flag,
                     Real file_voxel_min, Real file_voxel_max, Volume volume, STRING history,
                     minc_output_options *options) {
    fprintf(stderr, "probe: output_volume(%s)\n", filename);
    return ERROR;
}
void set_default_minc_input_options(minc_input_options *options) {}
void set_minc_input_vector_to_scalar_flag(minc_input_options *options, BOOLEAN flag) {}
Volume copy_volume(Volume volume) { return volume; }
void delete_volume(Volume volume) {}
int get_volume_n_dimensions(Volume volume) { return 0; }
void get_volume_sizes(Volume volume, int sizes[]) {}
STRING *get_volume_dimension_names(Volume volume) { return NULL; }
void delete_dimension_names(Volume volume, STRING dimension_names[]) {}
void grid_transform_point(General_transform *transform, Real x, Real y, Real z, Real *x_transformed,
                          Real *y_transformed, Real *z_transformed) {}
void grid_inverse_transform_point(General_transform *transform, Real x, Real y, Real z,
                                  Real *x_transformed, Real *y_transformed, Real *z_transformed) {}
STRING get_date(void) { return create_string("date"); }
