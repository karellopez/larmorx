// SPDX-License-Identifier: Apache-2.0
// glibc 2.35's csqrt measured as a black box against the form larmorx_core::cplx::csqrt takes
// (and the fs-robust probe's complexrt.c), with glibc's own hypot inside the form so only the form
// is compared: the special values, a zero real part as sqrt(|y| / 2) in both parts, a zero
// imaginary part as sqrt(|x|), scaling by 1/4 above DBL_MAX / 4 and by 2^108 below 2 DBL_MIN, then
// Kahan's t = sqrt((hypot(x, y) + |x|) / 2). glibc's code is never read.
//
//   gcc -O2 -fno-builtin -o csqrt_form csqrt_form.c -lm && ./csqrt_form <log2 n>
//
// Families of SplitMix64 arguments: 0 uniform on [-1, 1), 1 log-uniform in [2^-30, 2^30], 2 random
// bit patterns (finite), 3 log-uniform over [2^-1080, 2^1020], 4 within 40 binades of overflow or
// of the subnormals; each in three modes: both parts drawn, the real part zero, the imaginary part
// zero.
#include <complex.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t s;
static uint64_t nx(void)
{
  uint64_t z = (s += 0x9e3779b97f4a7c15ull);
  z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
  z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
  return z ^ (z >> 31);
}
static double unit(void) { return (double)(nx() >> 11) * 0x1p-53; }
static uint64_t u(double x) { uint64_t b; memcpy(&b, &x, 8); return b; }
static double fam(int f)
{
  double m;
  uint64_t b;
  switch (f) {
  case 0: return 2 * unit() - 1;
  case 1: m = exp2(60 * unit() - 30); return (nx() & 1) ? -m : m;
  case 2: b = nx(); memcpy(&m, &b, 8); return m;
  case 3: m = exp2(2100 * unit() - 1080); return (nx() & 1) ? -m : m;
  default: m = exp2(40 * unit() + ((nx() & 1) ? 985.0 : -1085.0)); return (nx() & 1) ? -m : m;
  }
}
static double complex form(double x, double y)
{
  int k = 0;
  double re, im;
  if (fabs(x) > DBL_MAX / 4 || fabs(y) > DBL_MAX / 4) { x *= 0.25; y *= 0.25; k = 1; }
  else if (fabs(x) < 2 * DBL_MIN && fabs(y) < 2 * DBL_MIN) { x *= 0x1p108; y *= 0x1p108; k = -54; }
  if (x == 0) { double r = sqrt(0.5 * fabs(y)); re = r; im = copysign(r, y); }
  else if (y == 0) { if (x > 0) { re = sqrt(x); im = y; } else { re = 0.0; im = copysign(sqrt(-x), y); } }
  else {
    double t = sqrt(0.5 * (hypot(x, y) + fabs(x)));
    if (x >= 0) { re = t; im = 0.5 * (y / t); } else { re = 0.5 * (fabs(y) / t); im = copysign(t, y); }
  }
  if (k == 1) { re *= 2.0; im *= 2.0; } else if (k) { re *= 0x1p-54; im *= 0x1p-54; }
  return CMPLX(re, im);
}
int main(int argc, char **argv)
{
  int lg = argc > 1 ? atoi(argv[1]) : 20;
  long n = 1L << lg;
  for (int mode = 0; mode < 3; mode++)
    for (int f = 0; f < 5; f++) {
      s = 0x1234 + f + 10 * mode;
      long tot = 0, dif = 0;
      for (long i = 0; i < n; i++) {
        double x = fam(f), y = fam(f);
        if (mode == 1) x = (nx() & 1) ? 0.0 : -0.0;
        if (mode == 2) y = (nx() & 1) ? 0.0 : -0.0;
        if (!isfinite(x) || !isfinite(y) || (x == 0 && y == 0)) continue;
        tot++;
        double complex g = csqrt(CMPLX(x, y)), m = form(x, y);
        if (u(creal(g)) != u(creal(m)) || u(cimag(g)) != u(cimag(m))) {
          if (dif < 2) printf("  mode %d family %d: x=%a y=%a glibc=(%a,%a) form=(%a,%a)\n", mode, f, x, y, creal(g), cimag(g), creal(m), cimag(m));
          dif++;
        }
      }
      printf("mode %d family %d: %ld cases, %ld differ\n", mode, f, tot, dif);
    }
  return 0;
}
