/* SPDX-License-Identifier: Apache-2.0 */
/* Checksums of CORE-MATH's cr_tanhf and cr_cbrtf (MIT, compiled in from the pinned checkout)
   over every 4099th float bit pattern and over all 2^32 of them, computed as the tests of
   larmorx_core::math::{tanhf, cbrtf} compute theirs (FNV-1a over the result bits, NaNs as
   0x7fc00000). Equal checksums show the Rust ports bit-identical to the C code.

     gcc -O2 -ffp-contract=off -I <workspace>/tags/core-math/src -o coremath_checksums \
         coremath_checksums.c -lm && ./coremath_checksums

   Expected (gcc 11.4 and conda-forge gcc 13.3): tanhf 4099 5d9a7b8492e5ef7b, cbrtf 4099
   d8db7e186a25ff90, tanhf 1 9363b80d987f3611, cbrtf 1 30956ee9e1579229 (about 100 s). */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "binary32/tanh/tanhf.c"
#define b32u32_u b32u32_u_cbrt
#include "binary32/cbrt/cbrtf.c"

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
  printf("tanhf 4099 %016llx\n", (unsigned long long)checksum(cr_tanhf, 4099));
  printf("cbrtf 4099 %016llx\n", (unsigned long long)checksum(cr_cbrtf, 4099));
  printf("tanhf 1 %016llx\n", (unsigned long long)checksum(cr_tanhf, 1));
  printf("cbrtf 1 %016llx\n", (unsigned long long)checksum(cr_cbrtf, 1));
  return 0;
}
