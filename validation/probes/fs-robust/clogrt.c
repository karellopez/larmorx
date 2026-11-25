// SPDX-License-Identifier: Apache-2.0
// The complex logarithm the fs-robust probe links in place of glibc's (`ld --wrap=clog`, build.sh)
// and fs-robust-template's preload exports as `clog` (decided.sh): the project's decision for
// FreeSurfer's matrix logarithm (track C relay 51: glibc 2.35's `clog` is not correctly rounded and
// matches no simple formula), the same function as larmorx_core::cplx's `clog`, so the probes
// compare everything else bit for bit. On CORE-MATH's correctly rounded `atan2`, `log1p`, `log`
// and `hypot` (compiled from tags/core-math beside this file):
//
// - a NaN part: glibc's NaN 0x7ff8000000000000 in both parts, the real part +inf when the other
//   part is infinite (C11 Annex G, glibc's NaNs measured as a black box);
// - otherwise the imaginary part atan2(y, x); an infinite part gives the real part +inf, both
//   parts zero -inf;
// - otherwise d = |z|^2 - 1 (x x = px + ex and y y = py + ey split exactly by fused multiply-adds;
//   (px - 1) + py by two error-free sums, the low parts added in order, rounded once): for
//   -1/2 < d < 1/2 the real part is 0.5 log1p(d), else log(hypot(x, y)).
#include <complex.h>
#include <math.h>

double cr_atan2(double, double);
double cr_log1p(double);
double cr_log(double);
double cr_hypot(double, double);

static double two_sum(double a, double b, double *e)
{
  double s = a + b, bb = s - a;
  *e = (a - (s - bb)) + (b - bb);
  return s;
}

double _Complex __wrap_clog(double _Complex z)
{
  double x = creal(z), y = cimag(z);
  const double qnan = __builtin_nan("");
  if (isnan(x) || isnan(y)) return CMPLX((isinf(x) || isinf(y)) ? INFINITY : qnan, qnan);
  double im = cr_atan2(y, x);
  if (isinf(x) || isinf(y)) return CMPLX(INFINITY, im);
  if (x == 0 && y == 0) return CMPLX(-INFINITY, im);
  double px = x * x, ex = __builtin_fma(x, x, -px);
  double py = y * y, ey = __builtin_fma(y, y, -py);
  double l1, l2;
  double h1 = two_sum(px, -1.0, &l1);
  double h = two_sum(h1, py, &l2);
  double lo = ((l1 + l2) + ex) + ey;
  double d = h + lo;
  double re = (d > -0.5 && d < 0.5) ? 0.5 * cr_log1p(d) : cr_log(cr_hypot(x, y));
  return CMPLX(re, im);
}
