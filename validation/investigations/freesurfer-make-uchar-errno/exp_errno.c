/* SPDX-License-Identifier: Apache-2.0 */
/* Which arguments make the C library's exp() set errno (ERANGE): a black-box measurement of the
   system's libm (glibc 2.35 on the main machine). Prints each argument, the result's bits and
   errno, then the boundary below which a negative argument sets it. */
#include <errno.h>
#include <math.h>
#include <stdio.h>

int main(void) {
  double xs[] = {-700, -708.39, -708.3964, -708.4, -709, -720, -740, -744.4, -745.1, -745.13,
                 -745.1332191019411, -745.14, -746, -800, -1e5, -INFINITY, NAN, 709.7, 709.79,
                 710, INFINITY, 0.0, -1e-300};
  for (unsigned i = 0; i < sizeof(xs) / sizeof(xs[0]); i++) {
    volatile double x = xs[i];
    errno = 0;
    volatile double y = exp(x);
    printf("%.17g -> %a errno %d\n", x, y, errno);
  }
  double lo = -760, hi = -700;
  for (int k = 0; k < 200; k++) {
    double mid = 0.5 * (lo + hi);
    volatile double xm = mid;
    errno = 0;
    volatile double y = exp(xm);
    (void)y;
    if (errno) lo = mid; else hi = mid;
  }
  printf("errno boundary between %.17g and %.17g: exp(hi) = %a\n", lo, hi, exp(hi));
  return 0;
}
