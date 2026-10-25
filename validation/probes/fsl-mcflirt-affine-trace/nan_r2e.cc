// SPDX-License-Identifier: Apache-2.0
// FSL's rotmat2euler (libfsl-miscmaths, not FSL code here) on rotation matrices of NaNs, signs
// given as a 9-character pattern of 'n' (-nan), 'p' (+nan) and '1'/'0' (numbers), row by row:
// prints the three angles' bits. Usage: nan_r2e <pattern> ...
#include <cstdint>
#include <cstdio>
#include <cstring>

#include "miscmaths/miscmaths.h"

static double bits(uint64_t u) {
  double d;
  std::memcpy(&d, &u, 8);
  return d;
}

int main(int argc, char** argv) {
  for (int a = 1; a < argc; a++) {
    NEWMAT::Matrix r(3, 3);
    for (int k = 0; k < 9; k++) {
      char ch = argv[a][k];
      double v = ch == 'n' ? bits(0xfff8000000000000ull) : ch == 'p' ? bits(0x7ff8000000000000ull)
                 : ch == '1' ? 1.0 : 0.0;
      r(k / 3 + 1, k % 3 + 1) = v;
    }
    NEWMAT::ColumnVector ang(3);
    MISCMATHS::rotmat2euler(ang, r);
    uint64_t u[3];
    for (int i = 0; i < 3; i++) std::memcpy(&u[i], &ang(i + 1), 8);
    std::printf("%s -> %016llx %016llx %016llx\n", argv[a], (unsigned long long)u[0],
                (unsigned long long)u[1], (unsigned long long)u[2]);
  }
  return 0;
}
