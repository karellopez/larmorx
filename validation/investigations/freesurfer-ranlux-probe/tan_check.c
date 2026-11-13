/* SPDX-License-Identifier: Apache-2.0 */
/* Compares the C library's tan with CORE-MATH's correctly rounded cr_tan on every argument
   FreeSurfer's large-shape gamma can pass: fl(pi_d * (k * 2^-24)) for k in [0, 2^24). Prints the
   count of differing results and the first few; with the argument "list", every differing k. */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
double cr_tan(double);
static uint64_t bits(double v)
{
  uint64_t b;
  memcpy(&b, &v, 8);
  return b;
}
int main(int argc, char **argv)
{
  int list = argc > 1 && !strcmp(argv[1], "list");
  volatile double pi = 3.141592653589793; /* M_PI */
  long diff = 0, maxd = 0, mind = 0, lo = 0;
  for (long k = 0; k < (1L << 24); k++) {
    double u = (double)k / 16777216.0;
    double x = pi * u;
    double a = tan(x), b = cr_tan(x);
    if (bits(a) != bits(b)) {
      if (list) printf("%ld\n", k);
      else if (diff < 10) printf("k=%ld x=%a glibc=%a cr=%a\n", k, x, a, b);
      diff++;
      long d = (long)(bits(a) - bits(b));
      if (d > maxd) maxd = d;
      if (d < mind) mind = d;
      if (k < (1L << 23)) lo++;
    }
  }
  if (!list) printf("differing: %ld of %ld; bit-pattern difference glibc - cr in [%ld, %ld]; %ld with k < 2^23\n", diff, 1L << 24, mind, maxd, lo);
  return 0;
}
