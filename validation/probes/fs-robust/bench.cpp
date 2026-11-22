// SPDX-License-Identifier: Apache-2.0
// fs.robust's speed and memory against FreeSurfer's own code (build.sh builds it beside fsrobust).
// CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).
//
//     bench <op> <n>     op: partials | subsample | valscale | blur | centroid | sqrtm | rstep |
//                        rstepa | all; one n^3
//                        float volume of a fixed pattern (sqrtm: n 4 x 4 square roots); prints each op's seconds and the FNV-1a of its
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
// 3.2-03: the registration step (its registration's fields opened, as in fsrobust.cpp)
#include <fstream>
#include <limits>
#include <sstream>
#include <typeinfo>
#include <vnl/algo/vnl_qr.h>
#include <vnl/algo/vnl_svd.h>
#include "Quaternion.h"
#include "Transformation.h"
#include "RobustGaussian.h"
#define private public
#define protected public
#include "Regression.h"
#include "Registration.h"
#include "RegRobust.h"
#include "RegistrationStep.h"
#undef protected
#undef private

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

static MRI *pattern(int n, int sx = 0)
{
  MRI *m = MRIallocSequence(n, n, n, MRI_FLOAT, 1);
  for (int z = 0; z < n; z++)
    for (int y = 0; y < n; y++)
      for (int x0 = 0; x0 < n; x0++) {
        int x = x0 - sx;
        int r = (x - n / 2) * (x - n / 2) + (y - n / 2) * (y - n / 2) + (z - n / 3) * (z - n / 3);
        float v = r < n * n / 6 ? (float)((x * 7 + y * 13 + z * 29) % 97 + 100) : 0.0f;
        MRIsetVoxVal(m, x0, y, z, 0, v);
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
  MRI *src = op == "sqrtm" ? NULL : pattern(n);  // sqrtm: n is a count, no volume
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
  // n square roots of 4 x 4 transforms (a rotation by atan(4/3) about z, scaled, translated by k)
  run("sqrtm", [&] {
    uint64_t h = 0;
    for (int k = 0; k < n; k++) {
      vnl_matrix<double> m(4, 4, 0.0);
      m[0][0] = 0.6; m[0][1] = -0.8; m[1][0] = 0.8; m[1][1] = 0.6; m[2][2] = 1.02; m[3][3] = 1.0;
      m[0][3] = k * 0.001; m[1][3] = -k * 0.002; m[2][3] = 0.5;
      vnl_matrix<double> r = MyMatrix::MatrixSqrt(m);
      for (unsigned i = 0; i < 4; i++)
        for (unsigned j = 0; j < 4; j++) { uint64_t u; memcpy(&u, &r[i][j], 8); h = h * 31 + u; }
    }
    return h;
  });
  // one robust registration step (RegistrationStep<float>, the registration's defaults: SATr,
  // float SVD) between the pattern and the pattern moved by one voxel along x: rigid, then
  // affine with the intensity scale; the hash of the matrix, the parameter and the weights
  auto step = [&](int code, bool iscale) {
    MRI *t = pattern(n, 1);
    RegRobust R;
    R.iscale = iscale;
    R.trans = code == 11 ? (Transformation *)new Transform3dRigid : (Transformation *)new Transform3dAffine;
    R.verbose = 0;
    RegistrationStep<float> S(R);
    S.setFloatSVD(true);
    std::pair<vnl_matrix_fixed<double, 4, 4>, double> md = S.computeRegistrationStep(src, t);
    uint64_t h = fnv(S.getWeights());
    for (int i = 0; i < 4; i++)
      for (int j = 0; j < 4; j++) { uint64_t u; memcpy(&u, &md.first[i][j], 8); h = h * 31 + u; }
    uint64_t u;
    memcpy(&u, &md.second, 8);
    h = h * 31 + u;
    MRIfree(&t);
    return h;
  };
  run("rstep", [&] { return step(11, false); });
  run("rstepa", [&] { return step(7, true); });
  return 0;
}
