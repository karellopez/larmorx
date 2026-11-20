// SPDX-License-Identifier: Apache-2.0
// libgcc's __divdc3 and __muldc3 (the system's libgcc_s, a black box) against Smith's division and
// the plain product with C11 Annex G.5.1's recovery of infinities and zeros (the algorithms of
// larmorx_core::cplx::divdc3 and muldc3, written here in C from the standard's text), on every
// combination of 15 special and ordinary values for the four parts (15^4 = 50,625 cases each);
// bits compared exactly, NaNs included, and with every NaN read as one.
//
//   gcc -O2 -fno-builtin -o annexg annexg.c -lm && ./annexg
#include <complex.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

extern double _Complex __divdc3(double, double, double, double);
extern double _Complex __muldc3(double, double, double, double);

static uint64_t u(double x) { uint64_t b; memcpy(&b, &x, 8); return b; }
static double d(uint64_t b) { double x; memcpy(&x, &b, 8); return x; }
static double boxed(double v) { return copysign(isinf(v) ? 1.0 : 0.0, v); }
static double zn(double v) { return isnan(v) ? copysign(0.0, v) : v; }

static double complex mul(double a, double b, double c, double d)
{
  double ac = a * c, bd = b * d, ad = a * d, bc = b * c;
  double x = ac - bd, y = ad + bc;
  if (isnan(x) && isnan(y)) {
    int recalc = 0;
    if (isinf(a) || isinf(b)) { a = boxed(a); b = boxed(b); c = zn(c); d = zn(d); recalc = 1; }
    if (isinf(c) || isinf(d)) { c = boxed(c); d = boxed(d); a = zn(a); b = zn(b); recalc = 1; }
    if (!recalc && (isinf(ac) || isinf(bd) || isinf(ad) || isinf(bc))) { a = zn(a); b = zn(b); c = zn(c); d = zn(d); recalc = 1; }
    if (recalc) { x = INFINITY * (a * c - b * d); y = INFINITY * (a * d + b * c); }
  }
  return CMPLX(x, y);
}
static double complex dv(double a, double b, double c, double d)
{
  double x, y;
  if (fabs(c) < fabs(d)) { double r = c / d, den = c * r + d; x = (a * r + b) / den; y = (b * r - a) / den; }
  else { double r = d / c, den = d * r + c; x = (b * r + a) / den; y = (b - a * r) / den; }
  if (isnan(x) && isnan(y)) {
    if (c == 0.0 && d == 0.0 && (!isnan(a) || !isnan(b))) { x = copysign(INFINITY, c) * a; y = copysign(INFINITY, c) * b; }
    else if ((isinf(a) || isinf(b)) && isfinite(c) && isfinite(d)) { a = boxed(a); b = boxed(b); x = INFINITY * (a * c + b * d); y = INFINITY * (b * c - a * d); }
    else if ((isinf(c) || isinf(d)) && isfinite(a) && isfinite(b)) { c = boxed(c); d = boxed(d); x = 0.0 * (a * c + b * d); y = 0.0 * (b * c - a * d); }
  }
  return CMPLX(x, y);
}
int main(void)
{
  const uint64_t v[] = {0x0, 0x8000000000000000ull, 0x3ff0000000000000ull, 0xc000000000000000ull,
    0x7ff0000000000000ull, 0xfff0000000000000ull, 0x7ff8000000000000ull, 0xfff8000000000123ull,
    0x7ff4000000000456ull, 0x0000000000000001ull, 0x7fefffffffffffffull, 0x3fb999999999999aull,
    0x4415af1d78b58c40ull, 0xc3e0000000000000ull, 0x0010000000000000ull};
  const int n = sizeof v / sizeof v[0];
  long cases = 0, dbits = 0, dnan = 0, mbits = 0, mnan = 0;
  for (int i = 0; i < n; i++) for (int j = 0; j < n; j++) for (int k = 0; k < n; k++) for (int l = 0; l < n; l++) {
    double a = d(v[i]), b = d(v[j]), c = d(v[k]), e = d(v[l]);
    cases++;
    double complex q = __divdc3(a, b, c, e), r = dv(a, b, c, e);
    if (u(creal(q)) != u(creal(r)) || u(cimag(q)) != u(cimag(r))) {
      dbits++;
      int same = (isnan(creal(q)) ? isnan(creal(r)) : u(creal(q)) == u(creal(r))) && (isnan(cimag(q)) ? isnan(cimag(r)) : u(cimag(q)) == u(cimag(r)));
      if (!same) { if (dnan < 4) printf("div %a %a / %a %a: libgcc (%a,%a) annex G (%a,%a)\n", a, b, c, e, creal(q), cimag(q), creal(r), cimag(r)); dnan++; }
    }
    double complex p = __muldc3(a, b, c, e), m = mul(a, b, c, e);
    if (u(creal(p)) != u(creal(m)) || u(cimag(p)) != u(cimag(m))) {
      mbits++;
      int same = (isnan(creal(p)) ? isnan(creal(m)) : u(creal(p)) == u(creal(m))) && (isnan(cimag(p)) ? isnan(cimag(m)) : u(cimag(p)) == u(cimag(m)));
      if (!same) { if (mnan < 4) printf("mul %a %a * %a %a: libgcc (%a,%a) annex G (%a,%a)\n", a, b, c, e, creal(p), cimag(p), creal(m), cimag(m)); mnan++; }
    }
  }
  printf("%ld cases: __divdc3 bits differ %ld (beyond NaN bits %ld); __muldc3 bits differ %ld (beyond NaN bits %ld)\n", cases, dbits, dnan, mbits, mnan);
  return 0;
}
