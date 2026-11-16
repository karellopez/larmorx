/* SPDX-License-Identifier: Apache-2.0 */
/* Checksums of CORE-MATH's cr_log10f (MIT, compiled in from the pinned checkout at commit
   040ee482a8ca) over every 4099th float bit pattern and over all 2^32 of them, computed as the
   tests of larmorx_core::math::log10f compute theirs (FNV-1a over the result bits, NaNs as
   0x7fc00000). Equal checksums show the Rust port bit-identical to the C code. It also counts the
   positive finite floats where cr_log10f differs from CORE-MATH's double cr_log10 rounded to float
   (the double-rounding cases, each printed) and from the C library's log10f (glibc 2.35's on the
   main machine: the arguments where Libm::log10f's two choices differ).

     gcc -O2 -ffp-contract=off -c -o log10.o <workspace>/tags/core-math/src/binary64/log10/log10.c
     gcc -O2 -ffp-contract=off -I <workspace>/tags/core-math/src -o log10f_checksums \
         log10f_checksums.c log10.o -lm
     <workspace>/larmorx/scripts/orchestration/boxed.sh 2 ./log10f_checksums

   The results are in the README. */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "binary32/log10/log10f.c"

double cr_log10(double);

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

static uint32_t bits(float f) {
  uint32_t b;
  memcpy(&b, &f, 4);
  return b;
}

int main(void) {
  printf("log10f 4099 %016llx\n", (unsigned long long)checksum(cr_log10f, 4099));
  printf("log10f 1 %016llx\n", (unsigned long long)checksum(cr_log10f, 1));
  uint64_t n = 0, dbl = 0, libc = 0;
  for (uint32_t x = 1; x < 0x7f800000u; x++) {
    float fx;
    memcpy(&fx, &x, 4);
    n++;
    uint32_t a = bits(cr_log10f(fx));
    uint32_t d = bits((float)cr_log10((double)fx));
    uint32_t g = bits(log10f(fx));
    if (a != d) {
      dbl++;
      printf("double differs %08x cr_log10f %08x double %08x\n", x, a, d);
    }
    if (a != g) {
      if (libc < 20) printf("libc differs %08x cr_log10f %08x libc %08x\n", x, a, g);
      libc++;
    }
  }
  printf("positive finite floats %llu, cr_log10f != (float)cr_log10 on %llu, != libc log10f on %llu\n",
         (unsigned long long)n, (unsigned long long)dbl, (unsigned long long)libc);
  printf("special 0 %08x -0 %08x -1 %08x -inf %08x inf %08x snan %08x tiny %08x\n",
         bits(cr_log10f(0.0f)), bits(cr_log10f(-0.0f)), bits(cr_log10f(-1.0f)),
         bits(cr_log10f(-INFINITY)), bits(cr_log10f(INFINITY)),
         bits(cr_log10f(((union {uint32_t u; float f;}){.u = 0x7f800001u}).f)),
         bits(cr_log10f(((union {uint32_t u; float f;}){.u = 1u}).f)));
  return 0;
}
