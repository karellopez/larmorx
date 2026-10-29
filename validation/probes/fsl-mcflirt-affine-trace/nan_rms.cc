// SPDX-License-Identifier: Apache-2.0
// A probe of FSL's rms_deviation (not FSL code: it calls the libfsl-miscmaths of the FSL it is
// built against, as mcflirt does) on pairs of affines as mcflirt passes them (the previous
// volume's and this volume's, or the identity and this volume's), with numbers, NaNs of both
// signs and payloads, all-NaN affines (FSL-14's matrices) and infinities, seeded: one
// "RMS <a1 16 hex> <a2 16 hex> <centre 3 hex> <rmax float hex> -> <rms float hex>" line per case,
// which crates-nc's affine-trace-replay replays through the replica.
//
// Usage: nan_rms <cases> <seed>, or nan_rms - (lines "<a1 16 hex> <a2 16 hex> <centre 3 hex>
// <rmax float hex>" on standard input: those calls).
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
#include <string>
#include <vector>

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

static std::string hxf(float f) {
  uint32_t u;
  std::memcpy(&u, &f, 4);
  char b[12];
  std::snprintf(b, sizeof b, "%08x", (unsigned)u);
  return b;
}

static std::string mat(const Matrix& a) {
  std::string s;
  for (int k = 1; k <= 4; k++)
    for (int r = 1; r <= 4; r++) s += (k + r > 2 ? "," : "") + hx(a(r, k));
  return s;
}

static std::vector<double> parse(const std::string& s) {
  std::vector<double> v;
  size_t i = 0;
  while (i < s.size()) {
    size_t j = s.find(',', i);
    if (j == std::string::npos) j = s.size();
    v.push_back(from_bits(std::strtoull(s.substr(i, j - i).c_str(), nullptr, 16)));
    i = j + 1;
  }
  return v;
}

static int from_stdin() {
  char a1s[400], a2s[400], cs[80], rs[20];
  while (std::scanf("%399s %399s %79s %19s", a1s, a2s, cs, rs) == 4) {
    Matrix a1(4, 4), a2(4, 4);
    std::vector<double> v1 = parse(a1s), v2 = parse(a2s), c = parse(cs);
    for (int k = 0; k < 16; k++) {
      a1(k % 4 + 1, k / 4 + 1) = v1[k];
      a2(k % 4 + 1, k / 4 + 1) = v2[k];
    }
    ColumnVector centre(3);
    for (int i = 1; i <= 3; i++) centre(i) = c[i - 1];
    uint32_t ru = std::strtoul(rs, nullptr, 16);
    float rmax;
    std::memcpy(&rmax, &ru, 4);
    float rms = rms_deviation(a1, a2, centre, rmax);
    std::printf("RMS %s %s %s %s -> %s\n", a1s, a2s, cs, rs, hxf(rms).c_str());
  }
  return 0;
}

int main(int argc, char** argv) {
  if (argc > 1 && std::string(argv[1]) == "-") return from_stdin();
  int cases = argc > 1 ? std::atoi(argv[1]) : 200;
  unsigned seed = argc > 2 ? std::atoi(argv[2]) : 1;
  std::mt19937_64 rng(seed);
  const double PNAN = from_bits(0x7ff8000000000000ull), NNAN = from_bits(0xfff8000000000000ull);
  const double PAY = from_bits(0x7ff8000012345000ull);
  auto motion = [&](Matrix& a) {
    a = 0.0;
    a(4, 4) = 1.0;
    double t = 0.01 * (int)(rng() % 21) - 0.1, u = 0.005 * (int)(rng() % 21) - 0.05;
    a(1, 1) = std::cos(t); a(1, 2) = std::sin(t); a(2, 1) = -std::sin(t);
    a(2, 2) = std::cos(t) * std::cos(u); a(2, 3) = std::sin(u); a(3, 2) = -std::sin(u);
    a(3, 3) = std::cos(u);
    a(1, 4) = 0.1 * (int)(rng() % 31) - 1.5; a(2, 4) = -0.2; a(3, 4) = 0.05 * (int)(rng() % 9);
  };
  for (int c = 0; c < cases; c++) {
    Matrix a1(4, 4), a2(4, 4);
    motion(a1);
    motion(a2);
    int kind = c % 10;
    for (int r = 1; r <= 3; r++)
      for (int k = 1; k <= 4; k++) {
        uint64_t u = rng();
        if (kind == 0) a2(r, k) = PNAN;                                // abs or rel, +nan
        else if (kind == 1) a2(r, k) = NNAN;                           // -nan
        else if (kind == 2) a1(r, k) = PNAN;                           // the previous one NaN
        else if (kind == 3) { a1(r, k) = NNAN; a2(r, k) = PNAN; }      // both, other signs
        else if (kind == 4) a2(r, k) = PAY;                            // a payload
        else if (kind == 5 && (u % 3 == 0)) a2(r, k) = (u & 4) ? NNAN : PNAN;  // mixed
        else if (kind == 6 && (u % 4 == 0)) a1(r, k) = (u & 8) ? NNAN : PAY;   // mixed
        else if (kind == 7 && (u % 5 == 0)) a1(r, k) = (u & 8) ? INFINITY : -INFINITY;  // a2 is inverted: FSL exits on a singular one
      }
    if (c % 3 == 0 && kind != 2 && kind != 3 && kind != 6) a1 = NEWMAT::IdentityMatrix(4);
    ColumnVector centre(3);
    for (int i = 1; i <= 3; i++) centre(i) = (c % 11 == 10) ? PNAN : 10.0 + i;
    float rmax = (c % 2) ? 80.0f : 64.5f;
    float rms = rms_deviation(a1, a2, centre, rmax);
    std::printf("RMS %s %s %s,%s,%s %s -> %s\n", mat(a1).c_str(), mat(a2).c_str(),
                hx(centre(1)).c_str(), hx(centre(2)).c_str(), hx(centre(3)).c_str(),
                hxf(rmax).c_str(), hxf(rms).c_str());
  }
  return 0;
}
