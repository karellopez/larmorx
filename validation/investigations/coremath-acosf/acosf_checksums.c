/* SPDX-License-Identifier: Apache-2.0 */
/* Checksums of CORE-MATH's cr_acosf (MIT, compiled in from the pinned checkout at commit
   040ee482a8ca) over every 4099th float bit pattern and over all 2^32 of them, computed as the
   tests of larmorx_core::math::acosf compute theirs (FNV-1a over the result bits, NaNs as
   0x7fc00000). Equal checksums show the Rust port bit-identical to the C code. It also prints
   the floats in [-1, 1] where cr_acosf differs from the double cr_acos rounded to float
   (CORE-MATH's own double arc cosine, linked from the preload): the double-rounding cases.

     gcc -O2 -ffp-contract=off -I <workspace>/tags/core-math/src -o acosf_checksums \
         acosf_checksums.c <workspace>/oracles/core-math-preload/libcrmath.so -lm \
         -Wl,-rpath,<workspace>/oracles/core-math-preload
     <workspace>/larmorx/scripts/orchestration/boxed.sh 2 ./acosf_checksums

   The results of 2026-11-13 are in the README. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "binary32/acos/acosf.c"

double acos(double); /* the preload's cr_acos under the C library's name */

static uint64_t checksum(float (*f)(float), uint64_t stride) {
  uint64_t h = 0xcbf29ce484222325ull;
  for (uint64_t i = 0; i < (1ull << 32); i += stride) {
    uint32_t x = (uint32_t)i, b;
    float fx, r;
    memcpy(&fx, &x, 4);
    r = f(fx);
    memcpy(&b, &r, 4);
    if ((b & 0x7fffffffu) > 0x7f800000u) b = 0x7fc00000u;
    h = (h ^ (uint64_t)b) * 0x100000001b3ull;
  }
  return h;
}

int main(void) {
  printf("acosf 4099 %016llx\n", (unsigned long long)checksum(cr_acosf, 4099));
  printf("acosf 1 %016llx\n", (unsigned long long)checksum(cr_acosf, 1));
  uint64_t n = 0, differ = 0;
  for (uint64_t i = 0; i < (1ull << 32); i++) {
    uint32_t x = (uint32_t)i, a, b;
    float fx, r, s;
    memcpy(&fx, &x, 4);
    if (!(fx >= -1.0f && fx <= 1.0f)) continue;
    n++;
    r = cr_acosf(fx);
    s = (float)acos((double)fx);
    memcpy(&a, &r, 4);
    memcpy(&b, &s, 4);
    if (a != b) {
      differ++;
      printf("differs %08x cr_acosf %08x double %08x\n", x, a, b);
    }
  }
  printf("floats in [-1, 1] %llu, cr_acosf != (float)cr_acos on %llu\n",
         (unsigned long long)n, (unsigned long long)differ);
  return 0;
}
