// SPDX-License-Identifier: Apache-2.0
// A probe of FSL's decompose_aff and rotmat2euler (not FSL code: it calls FSL 6.0.7.17's
// libfsl-miscmaths, as mcflirt does) on affines holding NaNs of both signs, infinities and
// numbers, seeded: one "DEC <affine 16 hex, column-major> <centre 3 hex> -> <params 12 hex>" line
// per case, the format affine-trace-replay reads, so the replica's NaNs can be compared.
//
// Usage: nan_decompose <cases> <seed>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
#include <string>

#include "miscmaths/miscmaths.h"

using namespace MISCMATHS;
using NEWMAT::ColumnVector;
using NEWMAT::Matrix;

static double from_bits(uint64_t u) {
  double d;
  std::memcpy(&d, &u, 8);
  return d;
}

static std::string hx(double d) {
  uint64_t u;
  std::memcpy(&u, &d, 8);
  char b[20];
  std::snprintf(b, sizeof b, "%016llx", (unsigned long long)u);
  return b;
}

// The rotation matrix decompose_aff hands to rotmat2euler, printed before the DEC line.
static int r2e_print(ColumnVector& angles, const Matrix& rotmat) {
  std::string s = "ROT ";
  for (int k = 1; k <= 3; k++)
    for (int r = 1; r <= 3; r++) s += (k + r > 2 ? "," : "") + hx(rotmat(r, k));
  std::printf("%s\n", s.c_str());
  return rotmat2euler(angles, rotmat);
}

int main(int argc, char** argv) {
  int cases = argc > 1 ? std::atoi(argv[1]) : 200;
  unsigned seed = argc > 2 ? std::atoi(argv[2]) : 1;
  std::mt19937_64 rng(seed);
  const double PNAN = from_bits(0x7ff8000000000000ull), NNAN = from_bits(0xfff8000000000000ull);
  const double PAY = from_bits(0x7ff8000012345000ull);
  for (int c = 0; c < cases; c++) {
    Matrix a(4, 4);
    a = 0.0;
    a(4, 4) = 1.0;
    // A rotation-like base.
    double t = 0.01 * (int)(rng() % 21) - 0.1;
    a(1, 1) = std::cos(t); a(1, 2) = std::sin(t); a(2, 1) = -std::sin(t); a(2, 2) = std::cos(t);
    a(3, 3) = 1.0;
    a(1, 4) = 1.5; a(2, 4) = -2.0; a(3, 4) = 0.25;
    int kind = c % 8;
    for (int r = 1; r <= 3; r++)
      for (int k = 1; k <= 4; k++) {
        uint64_t u = rng();
        double v = a(r, k);
        if (kind == 0) v = NNAN;                                   // all -nan
        else if (kind == 1) v = PNAN;                              // all +nan
        else if (kind == 2 && k == 4) v = (u & 1) ? NNAN : PNAN;   // translation only
        else if (kind == 3 && (u % 3 == 0)) v = (u & 4) ? NNAN : PNAN;  // a third, mixed
        else if (kind == 4) v = (u & 1) ? NNAN : (u & 2) ? PNAN : PAY;  // all, mixed
        else if (kind == 5 && (u % 5 == 0)) v = (u & 8) ? INFINITY : -INFINITY;
        else if (kind == 6) v = PAY;                               // all one NaN with a payload
        else if (kind == 7) v = -PAY;                              // the same, sign set
        a(r, k) = v;
      }
    ColumnVector centre(3);
    for (int i = 1; i <= 3; i++) {
      uint64_t u = rng();
      centre(i) = (c % 7 == 6) ? ((u & 1) ? NNAN : PNAN) : 10.0 + i;
    }
    ColumnVector params(12);
    decompose_aff(params, a, centre, r2e_print);
    std::string s = "DEC ";
    for (int k = 1; k <= 4; k++)
      for (int r = 1; r <= 4; r++) s += (k + r > 2 ? "," : "") + hx(a(r, k));
    s += " ";
    for (int i = 1; i <= 3; i++) s += (i > 1 ? "," : "") + hx(centre(i));
    s += " ->";
    std::string p;
    for (int i = 1; i <= 12; i++) p += (i > 1 ? "," : "") + hx(params(i));
    std::printf("%s %s\n", s.c_str(), p.c_str());
  }
  return 0;
}
