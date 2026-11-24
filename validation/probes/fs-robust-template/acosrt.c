/* SPDX-License-Identifier: Apache-2.0 */
/* The double acos the project decided for FreeSurfer's programs (the user's decision of 2026-11-04
   for glibc's acos, applied to FreeSurfer: correctly rounded in both modes; GLIBC-07): CORE-MATH's
   cr_acos (tags/core-math) for numbers, glibc's own for a NaN argument (its quiet NaN), exported
   as acos for the official programs' LD_PRELOAD (decided.sh). */
#define _GNU_SOURCE
#include <dlfcn.h>

double cr_acos(double);

double acos(double x)
{
  if (x != x) {
    static double (*real)(double);
    if (!real)
      real = (double (*)(double))dlsym(RTLD_NEXT, "acos");
    return real(x);
  }
  return cr_acos(x);
}
