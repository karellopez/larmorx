// SPDX-License-Identifier: Apache-2.0
// Prints nlohmann/json 3.12.0's text of doubles (as FSL 6.0.7.23 ships it, header only):
// "<bits in hex> <dump of the value>" per line, for 2 million values: random bit patterns of
// every exponent, floats widened to double (fslstats' -r, -R, -p, -v), integers and decimal
// fractions, the boundaries (subnormals, the largest double, powers of ten).
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <nlohmann/json.hpp>

static uint64_t s = 0x9E3779B97F4A7C15ull;
static uint64_t next() {
  s ^= s << 13; s ^= s >> 7; s ^= s << 17; return s;
}

static void put(double v) {
  uint64_t b; std::memcpy(&b, &v, 8);
  nlohmann::json j = v;
  std::printf("%016llx %s\n", (unsigned long long)b, j.dump().c_str());
}

int main() {
  for (int k = 0; k < 800000; k++) {
    uint64_t b = next();
    double v; std::memcpy(&v, &b, 8);
    if (v != v || v - v != 0) continue;  // NaN, inf: "null"
    put(v);
  }
  for (int k = 0; k < 800000; k++) {
    uint32_t b = (uint32_t)next();
    float f; std::memcpy(&f, &b, 4);
    if (f != f || f - f != 0) continue;
    put((double)f);
  }
  for (int k = -100000; k < 100000; k++) put((double)k);
  for (int k = 0; k < 100000; k++) put((double)(next() % 1000000) / 1000.0);
  for (int e = -324; e <= 308; e++) { put(std::strtod(("1e" + std::to_string(e)).c_str(), nullptr)); }
  put(4.9406564584124654e-324); put(2.2250738585072014e-308); put(1.7976931348623157e308);
  put(0.0); put(-0.0); put(16777217.0); put((double)(float)16842495);
  return 0;
}
