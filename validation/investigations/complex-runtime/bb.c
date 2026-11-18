// SPDX-License-Identifier: Apache-2.0
// Black-box measurement of the C runtime's complex functions FreeSurfer's MatrixSqrt (and the
// LAPACK it links) call: glibc 2.35's csqrt and cabs, libgcc's __divdc3 and __muldc3 (the system's
// libgcc_s.so.1). Their code is never read: each is compared, bit for bit, with candidate formulas
// written from the published algorithms (Kahan 1987 for the square root; Smith 1962 and the C
// standard's Annex G for division and multiplication) on seeded random arguments.
//
//   gcc -O2 -fno-builtin -o bb bb.c -lm && ./bb <log2 n> <family>
#include <complex.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern double _Complex __divdc3(double, double, double, double);
extern double _Complex __muldc3(double, double, double, double);

static uint64_t s;
static uint64_t next(void) {
  uint64_t z = (s += 0x9e3779b97f4a7c15ull);
  z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
  z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
  return z ^ (z >> 31);
}
static double unit(void) { return (double)(next() >> 11) * 0x1p-53; }
// families of numbers: 0 uniform [-1,1), 1 log-uniform |x| in [2^-30, 2^30] signed, 2 random bits
static double num(int fam) {
  if (fam == 0) return 2.0 * unit() - 1.0;
  if (fam == 1) { double e = 60.0 * unit() - 30.0; double m = exp2(e); return (next() & 1) ? -m : m; }
  uint64_t b = next(); double d; memcpy(&d, &b, 8); return d;
}
static uint64_t bits(double d) { uint64_t u; memcpy(&u, &d, 8); return u; }
static int same(double a, double b) { return bits(a) == bits(b) || (isnan(a) && isnan(b)); }

// csqrt candidates (finite, not both zero)
static double complex sq_a(double x, double y) {
  double t = sqrt(0.5 * (hypot(x, y) + fabs(x)));
  if (x >= 0) return CMPLX(t, 0.5 * (y / t));
  return CMPLX(0.5 * (fabs(y) / t), copysign(t, y));
}
static double complex sq_b(double x, double y) {
  double t = sqrt((hypot(x, y) + fabs(x)) * 0.5);
  if (x >= 0) return CMPLX(t, y / (2.0 * t));
  return CMPLX(fabs(y) / (2.0 * t), copysign(t, y));
}
// division candidates (Smith 1962)
static double complex div_smith(double a, double b, double c, double d) {
  double x, y;
  if (fabs(c) < fabs(d)) { double r = c / d, den = c * r + d; x = (a * r + b) / den; y = (b * r - a) / den; }
  else { double r = d / c, den = d * r + c; x = (b * r + a) / den; y = (b - a * r) / den; }
  return CMPLX(x, y);
}

int main(int argc, char **argv) {
  int lg = argc > 1 ? atoi(argv[1]) : 20, fam = argc > 2 ? atoi(argv[2]) : 0;
  long n = 1L << lg;
  long dq_a = 0, dq_b = 0, dd = 0, dm = 0, nfin = 0;
  s = 0x5eed0000ull + fam;
  for (long i = 0; i < n; i++) {
    double x = num(fam), y = num(fam), c = num(fam), d = num(fam);
    if (!isfinite(x) || !isfinite(y) || !isfinite(c) || !isfinite(d)) continue;
    nfin++;
    double complex g = csqrt(CMPLX(x, y));
    if (!(x == 0 && y == 0)) {
      double complex a = sq_a(x, y), b = sq_b(x, y);
      if (!same(creal(g), creal(a)) || !same(cimag(g), cimag(a))) { if (dq_a < 4) printf("sqA x=%a y=%a g=(%a,%a) a=(%a,%a)\n", x, y, creal(g), cimag(g), creal(a), cimag(a)); dq_a++; }
      if (!same(creal(g), creal(b)) || !same(cimag(g), cimag(b))) dq_b++;
    }
    if (c != 0 || d != 0) {
      double complex q = __divdc3(x, y, c, d), r = div_smith(x, y, c, d);
      if (!same(creal(q), creal(r)) || !same(cimag(q), cimag(r))) { if (dd < 4) printf("div %a %a / %a %a: libgcc (%a,%a) smith (%a,%a)\n", x, y, c, d, creal(q), cimag(q), creal(r), cimag(r)); dd++; }
    }
    double complex m = __muldc3(x, y, c, d);
    double mr = x * c - y * d, mi = x * d + y * c;
    if (!same(creal(m), mr) || !same(cimag(m), mi)) dm++;
  }
  printf("family %d: %ld finite cases: csqrt A differs %ld, B differs %ld; __divdc3 vs Smith %ld; __muldc3 vs plain %ld\n", fam, nfin, dq_a, dq_b, dd, dm);
  return 0;
}
