// SPDX-License-Identifier: Apache-2.0
// The complex absolute value and square root the fs-robust probe links in place of glibc's (`ld
// --wrap=cabs,--wrap=csqrt`, build.sh): the project's decision for FreeSurfer's matrix square root
// (track C relay 41, the user's D4: `hypot` correctly rounded in both modes), the same functions as
// larmorx_core::cplx's `cabs` and `csqrt`, so the probe compares everything else bit for bit.
//
// - cabs(z): CORE-MATH's correctly rounded `cr_hypot(re, im)` (compiled beside this file).
// - csqrt(z): Kahan's square root (Kahan, "Branch Cuts for Complex Elementary Functions", 1987)
//   with that hypot, in the form glibc 2.35's `csqrt` was measured to take as a black box
//   (`validation/investigations/complex-runtime/README.md`, its "csqrt's form" section): the
//   special values of C11's Annex G with glibc's NaNs (the default NaN 0x7ff8000000000000), a zero
//   real part as sqrt(|y| / 2) in both parts, a zero imaginary part as sqrt(|x|), scaling by 1/4
//   above DBL_MAX / 4 and by 2^108 below 2 DBL_MIN, then t = sqrt((hypot(x, y) + |x|) / 2) and the
//   other part (y / t) / 2.
#include <complex.h>
#include <float.h>
#include <math.h>

double cr_hypot(double, double);

double __wrap_cabs(double _Complex z) { return cr_hypot(creal(z), cimag(z)); }

double _Complex __wrap_csqrt(double _Complex z)
{
  double x = creal(z), y = cimag(z), re, im;
  const double qnan = __builtin_nan("");
  if (isinf(y)) return CMPLX(INFINITY, y);
  if (isinf(x)) {
    if (x < 0) return isnan(y) ? CMPLX(qnan, copysign(INFINITY, y)) : CMPLX(0.0, copysign(INFINITY, y));
    return isnan(y) ? CMPLX(INFINITY, qnan) : CMPLX(INFINITY, copysign(0.0, y));
  }
  if (isnan(x) || isnan(y)) return CMPLX(qnan, qnan);
  if (x == 0 && y == 0) return CMPLX(0.0, y);
  int k = 0;
  if (fabs(x) > DBL_MAX / 4 || fabs(y) > DBL_MAX / 4) {
    x *= 0.25;
    y *= 0.25;
    k = 1;
  } else if (fabs(x) < 2 * DBL_MIN && fabs(y) < 2 * DBL_MIN) {
    x *= 0x1p108;
    y *= 0x1p108;
    k = -54;
  }
  if (x == 0) {
    double r = sqrt(0.5 * fabs(y));
    re = r;
    im = copysign(r, y);
  } else if (y == 0) {
    if (x > 0) {
      re = sqrt(x);
      im = y;
    } else {
      re = 0.0;
      im = copysign(sqrt(-x), y);
    }
  } else {
    double t = sqrt(0.5 * (cr_hypot(x, y) + fabs(x)));
    if (x >= 0) {
      re = t;
      im = 0.5 * (y / t);
    } else {
      re = 0.5 * (fabs(y) / t);
      im = copysign(t, y);
    }
  }
  if (k == 1) {
    re *= 2.0;
    im *= 2.0;
  } else if (k) {
    re *= 0x1p-54;
    im *= 0x1p-54;
  }
  return CMPLX(re, im);
}
