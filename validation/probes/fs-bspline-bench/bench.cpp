// SPDX-License-Identifier: Apache-2.0
// fs.bspline's speed and memory against FreeSurfer 8.2.0's own code (CLOSED TO FREESURFER
// CLEAN-ROOM IMPLEMENTERS: it names FreeSurfer's functions; running it is a black-box run of
// FreeSurfer's code). Linked with the unit oracle's libraries as the official programs are
// (build.sh). One volume of the size given, its voxels a fixed integer pattern, then:
//
//     bench <w> <h> <d> <type: 0 uchar, 3 float> <transform: 0 none, 1 MRIlinearTransformBSpline>
//
// prints one line per step: the step, its seconds (wall clock, one thread: OMP_NUM_THREADS=1),
// the process's peak resident memory after it (kB, getrusage), and the FNV-1a (64 bits) of the
// voxels the step made. The port's side is crates-freesurfer/larmorx-freesurfer-volume's example
// fs_bspline_bench, which prints the same lines.

#include <sys/resource.h>
#include <sys/time.h>

#include <cinttypes>
#include <cstdio>
#include <cstdlib>

#include "matrix.h"
#include "mri.h"
#include "mriBSpline.h"

const char *Progname = "fsbsplinebench";

static double now()
{
  struct timeval tv;
  gettimeofday(&tv, NULL);
  return tv.tv_sec + 1e-6 * tv.tv_usec;
}

static long peak_kb()
{
  struct rusage ru;
  getrusage(RUSAGE_SELF, &ru);
  return ru.ru_maxrss;
}

static uint64_t fnv(const MRI *m)
{
  const unsigned char *p = (const unsigned char *)m->chunk;
  uint64_t h = 0xcbf29ce484222325ull;
  for (size_t i = 0; i < m->bytes_total; i++) {
    h ^= p[i];
    h *= 0x100000001b3ull;
  }
  return h;
}

int main(int argc, char *argv[])
{
  if (argc != 6) {
    fprintf(stderr, "usage: bench <w> <h> <d> <type> <transform>\n");
    return 2;
  }
  int w = atoi(argv[1]), h = atoi(argv[2]), d = atoi(argv[3]), type = atoi(argv[4]), xf = atoi(argv[5]);
  MRI *src = MRIallocSequence(w, h, d, type, 1);
  for (int z = 0; z < d; z++)
    for (int y = 0; y < h; y++)
      for (int x = 0; x < w; x++) {
        // a head-like blob: smooth in the middle, noise-like texture from an integer hash
        int dx = x - w / 2, dy = y - h / 2, dz = z - d / 2;
        int r2 = dx * dx + dy * dy + dz * dz, R = w / 3;
        unsigned hsh = (unsigned)(x * 73856093u) ^ (unsigned)(y * 19349663u) ^ (unsigned)(z * 83492791u);
        int v = r2 < R * R ? 80 + (int)((hsh >> 7) % 40u) + (r2 < R * R / 4 ? 60 : 0) : (int)((hsh >> 9) % 8u);
        MRIsetVoxVal(src, x, y, z, 0, (float)v);
      }
  printf("fnv-src 0 %ld %016" PRIx64 "\n", peak_kb(), fnv(src));
  double t0 = now();
  MRI_BSPLINE *b = MRItoBSpline(src, NULL, 3);
  double t1 = now();
  printf("coeff %.3f %ld %016" PRIx64 "\n", t1 - t0, peak_kb(), fnv(b->coeff));
  if (xf) {
    // a small rotation about the centre (about 3 degrees around z and 2 around x) and a shift
    MATRIX *A = MatrixIdentity(4, NULL);
    double c = 0.99862953475, s = 0.05233595624, c2 = 0.99939082702, s2 = 0.03489949670;
    double m[3][3] = {{c, -s * c2, s * s2}, {s, c * c2, -c * s2}, {0, s2, c2}};
    for (int i = 0; i < 3; i++)
      for (int j = 0; j < 3; j++) A->rptr[i + 1][j + 1] = (float)m[i][j];
    double cx = w / 2.0, cy = h / 2.0, cz = d / 2.0;
    for (int i = 0; i < 3; i++) {
      double t = (i == 0 ? cx : i == 1 ? cy : cz) - (m[i][0] * cx + m[i][1] * cy + m[i][2] * cz);
      A->rptr[i + 1][4] = (float)(t + 0.375 * (i + 1));
    }
    double t2 = now();
    MRI *r = MRIlinearTransformBSpline(b, NULL, A);
    double t3 = now();
    printf("transform %.3f %ld %016" PRIx64 "\n", t3 - t2, peak_kb(), fnv(r));
  }
  return 0;
}
