// SPDX-License-Identifier: Apache-2.0
// An LD_PRELOAD interposer for FSL's mcflirt (not FSL code): every call of
// NEWIMAGE::Costfn::cost(const Matrix&) is passed on to FSL's own function, and its affine's 16
// doubles and the cost it returns are printed on standard error as bits ("DBG <n> <hex,...>
// <cost hex> <cost>"), the same line the larmorx-nc replica's debug build prints.
#include <dlfcn.h>

#include <cstdint>
#include <cstdio>
#include <cstring>

#include "newimage/costfns.h"

using CostFn = float (*)(const NEWIMAGE::Costfn*, const NEWMAT::Matrix&);

float NEWIMAGE::Costfn::cost(const NEWMAT::Matrix& affmat) const {
  static CostFn real = nullptr;
  static long n = 0;
  if (!real) {
    real = reinterpret_cast<CostFn>(
        dlsym(RTLD_NEXT, "_ZNK8NEWIMAGE6Costfn4costERKN8armawrap8AWMatrixIdEE"));
  }
  float c = real(this, affmat);
  char buf[16 * 17 + 64];
  int p = 0;
  for (int k = 0; k < 16; k++) {
    double d = affmat(k % 4 + 1, k / 4 + 1);
    uint64_t u;
    std::memcpy(&u, &d, 8);
    p += std::snprintf(buf + p, sizeof(buf) - p, k ? ",%016llx" : "%016llx", (unsigned long long)u);
  }
  uint32_t cb;
  std::memcpy(&cb, &c, 4);
  std::fprintf(stderr, "DBG %ld 0 %s %08x %e\n", n++, buf, cb, c);
  return c;
}
