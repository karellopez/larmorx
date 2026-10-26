// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Karel Lopez Vilaret.
//
// libstdc++'s std::sort on floats with NaN values, as data for
// larmorx_nc_core::stdsort::sort_f32_checked (FSL-22: calc_bval's sort). Vectors from a seeded
// xorshift64 (sizes, NaN fractions, NaN signs and tie densities below) are sorted twice by the
// compiler's own std::sort: once with operator< (what FSL's calc_bval instantiates) and once
// with an equivalent comparator that records whether any element read lies outside the vector
// (the vector sits between guard cells holding NaN, which stop every unguarded loop). Each
// case's result (its bits, or "outside") goes into an FNV-1a hash that the Rust unit test
// recomputes from its own emulation.
//
//   g++ -O2 -std=c++17 sort_probe.cc -o sort_probe && ./sort_probe [--dump <case>]
//
// It holds no code from libstdc++: it calls std::sort.

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <vector>

static uint64_t s = 0x2545f4914f6cdd1dULL;
static uint64_t next() {
  s ^= s << 13;
  s ^= s >> 7;
  s ^= s << 17;
  return s;
}

static uint32_t bits(float f) {
  uint32_t u;
  std::memcpy(&u, &f, 4);
  return u;
}

static float from_bits(uint32_t u) {
  float f;
  std::memcpy(&f, &u, 4);
  return f;
}

struct Guarded {
  const float *lo, *b, *e, *hi;
  bool *outside;
  bool operator()(const float &x, const float &y) const {
    for (const float *p : {&x, &y})
      if ((p >= lo && p < b) || (p >= e && p < hi)) *outside = true;
    return x < y;
  }
};

int main(int argc, char **argv) {
  int dump = argc > 2 && std::strcmp(argv[1], "--dump") == 0 ? std::atoi(argv[2]) : -1;
  const int sizes[] = {0, 1, 2, 3, 5, 16, 17, 18, 31, 32, 33, 64, 100, 257, 1000, 5000};
  const int nan_per_mille[] = {0, 10, 100, 500, 900, 1000};
  const uint64_t modulos[] = {3, 1000};
  const size_t G = 1 << 16;
  uint64_t h = 0xcbf29ce484222325ULL;
  auto mix = [&](uint8_t byte) { h = (h ^ byte) * 0x100000001b3ULL; };
  int cases = 0, outside_cases = 0, nan_inside = 0, mismatch = 0;
  for (int n : sizes)
    for (int f : nan_per_mille)
      for (uint64_t m : modulos)
        for (int rep = 0; rep < 4; rep++) {
          std::vector<float> v(n);
          bool any_nan = false;
          for (int i = 0; i < n; i++) {
            uint64_t r = next();
            if ((int)(r % 1000) < f) {
              v[i] = from_bits((r >> 20) & 1 ? 0xffc00000u : 0x7fc00000u);
              any_nan = true;
            } else {
              v[i] = (float)((r >> 10) % m);
            }
          }
          std::vector<float> plain = v;
          std::sort(plain.begin(), plain.end());
          std::vector<float> buf(G + n + G, from_bits(0x7fc00000u));
          std::copy(v.begin(), v.end(), buf.begin() + G);
          bool outside = false;
          Guarded cmp{buf.data(), buf.data() + G, buf.data() + G + n, buf.data() + buf.size(),
                      &outside};
          std::sort(buf.begin() + G, buf.begin() + G + n, cmp);
          if (!outside) {
            for (int i = 0; i < n; i++)
              if (bits(buf[G + i]) != bits(plain[i])) mismatch++;
          }
          if (cases == dump) {
            std::printf("in ");
            for (float x : v) std::printf(" %08x", bits(x));
            std::printf("\nout%s", outside ? " outside" : "");
            for (float x : plain) std::printf(" %08x", bits(x));
            std::printf("\n");
          }
          if (outside) {
            outside_cases++;
            mix(0xee);
          } else {
            if (any_nan) nan_inside++;
            for (float x : plain) {
              uint32_t u = bits(x);
              for (int k = 0; k < 4; k++) mix((u >> (8 * k)) & 0xff);
            }
          }
          mix(0x55);
          cases++;
        }
  std::printf("cases %d outside %d nan_inside %d mismatch %d hash %016llx\n", cases,
              outside_cases, nan_inside, mismatch, (unsigned long long)h);
  return mismatch != 0;
}
