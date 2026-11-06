/* SPDX-License-Identifier: Apache-2.0
 *
 * A probe of AFNI's own library routines on 3dvolreg's path, linked against the AFNI oracle's
 * libmri.a (scripts/build_afni_oracle.sh), to compare them one at a time with larmorx's
 * replica (crates/larmorx-afni-replica). It contains no AFNI code: it only calls AFNI's
 * routines. Raw volumes are float32, x fastest.
 *
 *   volreg_probe blur  in.raw nx ny nz dx dy dz sx sy sz out.raw
 *       EDIT_blur_volume_3d (float), the result written to out.raw
 *   volreg_probe fir   in.raw nx ny nz dx dy dz sigma out.raw
 *       FIR_blur_volume
 *   volreg_probe rot   in.raw nx ny nz dx dy dz method pad ax1 th1 ax2 th2 ax3 th3 sx sy sz out.raw
 *       THD_rota_method(method) (0 NN, 1 linear, 2 cubic, 3 Fourier, 4 quintic, 5 heptic),
 *       THD_rota_setpad(pad,pad,pad), THD_rota3D(im, ax1,th1, ax2,th2, ax3,th3, -1, sx,sy,sz)
 *   volreg_probe gauss m dx
 *       fir_gaussian_load's weights, through FIR_blur_volume on a unit impulse
 */
#include "mrilib.h"

static float *readraw(const char *name, size_t n)
{
  float *f = malloc(sizeof(float) * n);
  FILE *fp = fopen(name, "rb");
  if (fp == NULL || fread(f, sizeof(float), n, fp) != n) { fprintf(stderr, "read %s\n", name); exit(2); }
  fclose(fp);
  return f;
}

static void writeraw(const char *name, const float *f, size_t n)
{
  FILE *fp = fopen(name, "wb");
  fwrite(f, sizeof(float), n, fp);
  fclose(fp);
}

int main(int argc, char **argv)
{
  if (argc < 2) return 2;
  if (strcmp(argv[1], "blur") == 0 && argc == 13) {
    int nx = atoi(argv[3]), ny = atoi(argv[4]), nz = atoi(argv[5]);
    size_t n = (size_t)nx * ny * nz;
    float *f = readraw(argv[2], n);
    EDIT_blur_volume_3d(nx, ny, nz, atof(argv[6]), atof(argv[7]), atof(argv[8]), MRI_float, f,
                        atof(argv[9]), atof(argv[10]), atof(argv[11]));
    writeraw(argv[12], f, n);
    return 0;
  }
  if (strcmp(argv[1], "fir") == 0 && argc == 11) {
    int nx = atoi(argv[3]), ny = atoi(argv[4]), nz = atoi(argv[5]);
    size_t n = (size_t)nx * ny * nz;
    float *f = readraw(argv[2], n);
    FIR_blur_volume(nx, ny, nz, atof(argv[6]), atof(argv[7]), atof(argv[8]), f, atof(argv[9]));
    writeraw(argv[10], f, n);
    return 0;
  }
  if (strcmp(argv[1], "rot") == 0 && argc == 21) {
    int nx = atoi(argv[3]), ny = atoi(argv[4]), nz = atoi(argv[5]);
    size_t n = (size_t)nx * ny * nz;
    MRI_IMAGE *im = mri_new_vol(nx, ny, nz, MRI_float), *out;
    float *f = readraw(argv[2], n);
    memcpy(MRI_FLOAT_PTR(im), f, sizeof(float) * n);
    im->dx = atof(argv[6]); im->dy = atof(argv[7]); im->dz = atof(argv[8]);
    THD_rota_method(atoi(argv[9]));
    THD_rota_setpad(atoi(argv[10]), atoi(argv[10]), atoi(argv[10]));
    out = THD_rota3D(im, atoi(argv[11]), atof(argv[12]), atoi(argv[13]), atof(argv[14]),
                     atoi(argv[15]), atof(argv[16]), -1, atof(argv[17]), atof(argv[18]),
                     atof(argv[19]));
    writeraw(argv[20], MRI_FLOAT_PTR(out), n);
    return 0;
  }
  fprintf(stderr, "usage: see the source\n");
  return 2;
}
