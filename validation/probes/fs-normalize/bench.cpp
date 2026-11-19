// SPDX-License-Identifier: Apache-2.0
// fs-normalize's stage bench: where mri_normalize's time goes in FreeSurfer 8.2.0's official code
// (mri_normalize.cpp, its main renamed, compiled into this file as fsnormalize.cpp does; build.sh
// builds it beside fsnormalize). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it names
// FreeSurfer's functions.
//
//     bench bobm <volume> <atlas> <transform> <whalf>   -atlas's mask (real.py's bobmf)
//     bench wmo <volume> <aseg> <intensity_below>       -aseg's outlier removal (real.py's wmomf)
//
// Each stage prints `<stage> <seconds>` (a steady clock), then `digest <FNV-1a of the result's
// voxels>`; the port's example fs_normalize_bench prints the same stages and digest. `wmo`'s `+`
// lines time the outlier removal's first steps on their own (their results freed) before the
// whole function runs.
#define main mri_normalize_main
#include "mri_normalize.cpp"
#undef main

#include <chrono>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>

static double now()
{
  return std::chrono::duration<double>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

static uint64_t fnv(const MRI *m)
{
  uint64_t h = 0xcbf29ce484222325ull;
  const unsigned char *p = (const unsigned char *)m->chunk;
  for (size_t i = 0; i < m->bytes_total; i++) {
    h ^= p[i];
    h *= 0x100000001b3ull;
  }
  return h;
}

int main(int argc, char *argv[])
{
  Progname = "mri_normalize";
  setenv("SURFER_SIDEDOOR", "1", 1);  // the licence key check (none in the replica)
  if (argc < 2) {
    fprintf(stderr, "usage: bench bobm <volume> <atlas> <transform> <whalf> | wmo <volume> <aseg> <ib>\n");
    return 2;
  }
  std::string op = argv[1];
  double t0;
  auto stage = [&](const char *name) {
    double t = now();
    printf("%s %.3f\n", name, t - t0);
    fflush(stdout);
    t0 = t;
  };
  if (op == "bobm" && argc == 6) {
    t0 = now();
    MRI *src = MRIread(argv[2]);
    stage("MRIread");
    GCA *a = GCAread(argv[3]);
    stage("GCAread");
    TRANSFORM *x = TransformRead(argv[4]);
    stage("TransformRead");
    if (!src || !a || !x) return 1;
    MRI *m = build_outside_of_brain_mask(src, a, x, 0, atoi(argv[5]));
    stage("build_outside_of_brain_mask");
    printf("digest %016" PRIx64 "\n", fnv(m));
  } else if (op == "wmo" && argc == 5) {
    t0 = now();
    MRI *src = MRIread(argv[2]);
    stage("MRIread");
    MRI *aseg = MRIread(argv[3]);
    stage("MRIread-aseg");
    if (!src || !aseg) return 1;
    if (src->type != MRI_FLOAT) {
      MRI *t = MRIchangeType(src, MRI_FLOAT, 0, 255, 1);
      MRIfree(&src);
      src = t;
    }
    stage("MRIchangeType");
    MRI *ctrl = MRIclone(aseg, NULL);
    for (size_t i = 0; i < NWM_LABELS; i++) MRIcopyLabel(aseg, ctrl, aseg_wm_labels[i]);
    stage("MRIclone+MRIcopyLabel");
    // the outlier removal's first steps, timed one by one (results freed)
    MRI *bin = MRIbinarize(ctrl, NULL, 1, 0, 1);
    stage("+ MRIbinarize");
    MRI *dist = MRIdistanceTransform(bin, NULL, 1, -1, DTRANS_MODE_SIGNED, NULL);
    stage("+ MRIdistanceTransform");
    MRIscalarMul(dist, dist, -1);
    stage("+ MRIscalarMul");
    MRI *sup = MRInonMaxSuppress(dist, NULL, 0, 1);
    stage("+ MRInonMaxSuppress");
    MRIfree(&bin);
    MRIfree(&dist);
    MRIfree(&sup);
    t0 = now();
    MRIremoveWMOutliersAndRetainMedialSurface(src, ctrl, ctrl, atoi(argv[4]));
    stage("MRIremoveWMOutliersAndRetainMedialSurface");
    printf("digest %016" PRIx64 "\n", fnv(ctrl));
  } else {
    fprintf(stderr, "bench: unknown command\n");
    return 2;
  }
  return 0;
}
