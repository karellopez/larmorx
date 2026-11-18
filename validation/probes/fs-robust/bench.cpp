// SPDX-License-Identifier: Apache-2.0
// fs.robust's speed and memory against FreeSurfer's own code (build.sh builds it beside fsrobust).
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).
//
//     bench <op> <n>     op: partials | subsample | valscale | blur | centroid | all; one n^3 float
//                        volume of a fixed pattern; prints each op's seconds and the FNV-1a of its
//                        outputs' voxels (the port's example fs_robust_bench prints the same hashes)
#include <chrono>
#include <cinttypes>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <string>
#include <vector>

#include "MyMatrix.h"
#include "CostFunctions.h"
#include "MyMRI.h"
#include "mri.h"

const char *Progname = "bench";

static uint64_t fnv(const MRI *m, uint64_t h = 0xcbf29ce484222325ull)
{
  const unsigned char *p = (const unsigned char *)m->chunk;
  for (size_t i = 0; i < m->bytes_total; i++) {
    h ^= p[i];
    h *= 0x100000001b3ull;
  }
  return h;
}

static MRI *pattern(int n)
{
  MRI *m = MRIallocSequence(n, n, n, MRI_FLOAT, 1);
  for (int z = 0; z < n; z++)
    for (int y = 0; y < n; y++)
      for (int x = 0; x < n; x++) {
        int r = (x - n / 2) * (x - n / 2) + (y - n / 2) * (y - n / 2) + (z - n / 3) * (z - n / 3);
        float v = r < n * n / 6 ? (float)((x * 7 + y * 13 + z * 29) % 97 + 100) : 0.0f;
        MRIsetVoxVal(m, x, y, z, 0, v);
      }
  m->xsize = m->ysize = m->zsize = 1.0f;
  MRIreInitCache(m);
  return m;
}

int main(int argc, char **argv)
{
  if (argc != 3) { fprintf(stderr, "usage: bench <op> <n>\n"); return 2; }
  std::string op = argv[1];
  int n = atoi(argv[2]);
  MRI *src = pattern(n);
  auto run = [&](const char *name, std::function<uint64_t()> f) {
    if (op != "all" && op != name) return;
    auto t0 = std::chrono::steady_clock::now();
    uint64_t h = f();
    double s = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
    printf("%s %.3f %016" PRIx64 "\n", name, s, h);
  };
  run("partials", [&] {
    MRI *fx = NULL, *fy = NULL, *fz = NULL, *bl = NULL;
    MyMRI::getPartials(src, fx, fy, fz, bl);
    uint64_t h = fnv(bl, fnv(fz, fnv(fy, fnv(fx))));
    MRIfree(&fx); MRIfree(&fy); MRIfree(&fz); MRIfree(&bl);
    return h;
  });
  run("subsample", [&] {
    MRI *s = MyMRI::subSample(src, NULL, true, 0);
    uint64_t h = fnv(s);
    MRIfree(&s);
    return h;
  });
  run("valscale", [&] {
    MRI *s = MyMRI::MRIvalscale(src, NULL, 1.1, 3.0);
    uint64_t h = fnv(s);
    MRIfree(&s);
    return h;
  });
  run("blur", [&] {
    MRI *s = MyMRI::getBlur(src, NULL);
    uint64_t h = fnv(s);
    MRIfree(&s);
    return h;
  });
  run("centroid", [&] {
    std::vector<double> c = CostFunctions::centroid(src);
    uint64_t h = 0;
    for (double v : c) { uint64_t u; memcpy(&u, &v, 8); h = h * 31 + u; }
    return h;
  });
  return 0;
}
