/* SPDX-License-Identifier: GPL-3.0-or-later
   Copyright 2026 Karel Lopez Vilaret.

   The C library's tanhf and cbrtf, for floatmath-verify: in a Rust program the symbol cbrtf
   resolves to the Rust standard library's own (correctly rounded) cube root, so the C
   library's results are computed here, by a C program linked with libm.

     gcc -O2 -o glibc_reference glibc_reference.c -lm
     ./glibc_reference checksums     # hash of every result, to compare with --checksums
     ./glibc_reference vectors N     # up to N inputs per function where the C library is not
                                     # correctly rounded, as Rust test vectors

   The hash is FNV-1a-like over the 32-bit results in input order (NaN as 0x7fc00000). */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t bits(float y) {
  uint32_t r;
  memcpy(&r, &y, 4);
  return y != y ? 0x7fc00000u : r;
}

static uint64_t checksum(float (*f)(float)) {
  uint64_t h = 1469598103934665603ULL;
  for (uint64_t k = 0; k <= 0xffffffffULL; k++) {
    uint32_t b = (uint32_t)k;
    float x;
    memcpy(&x, &b, 4);
    h = (h ^ bits(f(x))) * 1099511628211ULL;
  }
  return h;
}

/* Spread over [lo, hi]: the first input of each block where f is not the double result
   rounded to float (a correctly rounded reference in practice). */
static void vectors(const char *name, float (*f)(float), double (*d)(double), float lo, float hi,
                    int n) {
  uint32_t a, z, step, b;
  memcpy(&a, &lo, 4);
  memcpy(&z, &hi, 4);
  step = (z - a) / (uint32_t)n;
  if (step == 0) step = 1;
  printf("// %s\n", name);
  for (b = a; b <= z; b += step) {
    for (uint32_t k = b; k < b + step && k <= z; k++) {
      float x;
      memcpy(&x, &k, 4);
      if (bits(f(x)) != bits((float)d((double)x))) {
        printf("    (0x%08x, 0x%08x),\n", k, bits(f(x)));
        break;
      }
    }
  }
}

int main(int argc, char **argv) {
  if (argc > 1 && strcmp(argv[1], "checksums") == 0) {
    printf("tanhf checksum %016llx\n", (unsigned long long)checksum(tanhf));
    printf("cbrtf checksum %016llx\n", (unsigned long long)checksum(cbrtf));
    return 0;
  }
  if (argc > 2 && strcmp(argv[1], "vectors") == 0) {
    int n = atoi(argv[2]);
    vectors("tanhf", tanhf, tanh, 0.001f, 40.0f, n);
    vectors("cbrtf", cbrtf, cbrt, 0.001f, 1000.0f, n);
    return 0;
  }
  fprintf(stderr, "usage: %s checksums | vectors N\n", argv[0]);
  return 2;
}
