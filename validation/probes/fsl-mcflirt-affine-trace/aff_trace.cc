// SPDX-License-Identifier: Apache-2.0
// An LD_PRELOAD interposer for FSL's mcflirt (not FSL code): every call of
// MISCMATHS::decompose_aff(params, affmat, centre, rotmat2params) and MISCMATHS::rotmat2euler is
// passed on to FSL's own function, and its inputs and outputs are printed on standard error as
// the bits of their doubles ("DEC <affmat 16, column-major> <centre 3> -> <params 12>",
// "R2E <rotmat 9, column-major> -> <angles 3>").
#include <dlfcn.h>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>

#include "armawrap/newmat.h"

using NEWMAT::ColumnVector;
using NEWMAT::Matrix;

static std::string hx(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  char b[20];
  std::snprintf(b, sizeof(b), "%016llx", (unsigned long long)u);
  return b;
}

static std::string mat(const Matrix& m) {
  std::string s;
  for (int c = 1; c <= m.Ncols(); c++)
    for (int r = 1; r <= m.Nrows(); r++) s += (s.empty() ? "" : ",") + hx(m(r, c));
  return s;
}

static std::string vec(const ColumnVector& v) {
  std::string s;
  for (int i = 1; i <= v.Nrows(); i++) s += (s.empty() ? "" : ",") + hx(v(i));
  return s;
}

namespace MISCMATHS {
using R2P = int (*)(ColumnVector&, const Matrix&);

int rotmat2euler(ColumnVector& angles, const Matrix& rotmat) {
  static R2P real = nullptr;
  if (!real)
    real = reinterpret_cast<R2P>(dlsym(
        RTLD_NEXT, "_ZN9MISCMATHS12rotmat2eulerERN8armawrap11AWColVectorIdEERKNS0_8AWMatrixIdEE"));
  int r = real(angles, rotmat);
  std::fprintf(stderr, "R2E %s -> %s\n", mat(rotmat).c_str(), vec(angles).c_str());
  return r;
}

using DEC = int (*)(ColumnVector&, const Matrix&, const ColumnVector&, R2P);

int decompose_aff(ColumnVector& params, const Matrix& affmat, const ColumnVector& centre,
                  R2P rotmat2params) {
  static DEC real = nullptr;
  if (!real)
    real = reinterpret_cast<DEC>(
        dlsym(RTLD_NEXT,
              "_ZN9MISCMATHS13decompose_affERN8armawrap11AWColVectorIdEERKNS0_8AWMatrixIdEERKS2_"
              "PFiS3_S7_E"));
  int r = real(params, affmat, centre, rotmat2params);
  std::fprintf(stderr, "DEC %s %s -> %s\n", mat(affmat).c_str(), vec(centre).c_str(),
               vec(params).c_str());
  return r;
}
}  // namespace MISCMATHS
