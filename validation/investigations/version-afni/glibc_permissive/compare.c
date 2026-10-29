/* SPDX-License-Identifier: Apache-2.0
 *
 * Arm's Optimized Routines (MIT) sinf and cosf, built here as aor_sinf and aor_cosf, against
 * the C library's sinf and cosf on all 2^32 float inputs: the number of inputs whose results
 * differ bit for bit (NaNs compared as bits too). Built twice by run.sh: without FMA against
 * glibc's SSE2 build (GLIBC_TUNABLES=glibc.cpu.hwcaps=-FMA,-AVX2), and with -mfma -mavx2
 * against glibc's FMA build. Usage: compare sin|cos
 */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

float aor_sinf(float);
float aor_cosf(float);

static uint32_t bits(float x) {
  uint32_t u;
  memcpy(&u, &x, 4);
  return u;
}

int main(int argc, char **argv) {
  int cos_ = argc > 1 && strcmp(argv[1], "cos") == 0;
  unsigned long long differ = 0, nan_differ = 0;
  long long first = -1;
#pragma omp parallel for reduction(+ : differ, nan_differ) schedule(static)
  for (long long i = 0; i < (1LL << 32); i++) {
    uint32_t u = (uint32_t)i;
    float x;
    memcpy(&x, &u, 4);
    float a = cos_ ? aor_cosf(x) : aor_sinf(x);
    float g = cos_ ? cosf(x) : sinf(x);
    if (bits(a) != bits(g)) {
      if (isnan(a) && isnan(g))
        nan_differ++;
      else {
        differ++;
#pragma omp critical
        if (first < 0 || i < first) first = i;
      }
    }
  }
  printf("%s: %llu of 4294967296 inputs differ (NaN payloads aside: %llu more)", cos_ ? "cosf" : "sinf",
         differ, nan_differ);
  if (first >= 0) {
    uint32_t u = (uint32_t)first;
    float x;
    memcpy(&x, &u, 4);
    printf("; first at 0x%08x (%a)", u, (double)x);
  }
  printf("\n");
  return differ != 0;
}
