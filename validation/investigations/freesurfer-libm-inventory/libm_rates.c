/* SPDX-License-Identifier: Apache-2.0 */
/* How often the C library's function (glibc 2.35 on the reference machine) differs from the
   correctly rounded value (CORE-MATH's cr_*), for the C-library maths FreeSurfer's reached code
   calls that larmorx_core::math has no glibc-exact version of; and the finite argument sets
   FreeSurfer passes in two places. Calls only; no glibc code.
   Build: see README.md (the CORE-MATH sources of each function are compiled beside it). */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
double cr_atan2(double, double);
double cr_log10(double);
double cr_log2(double);
float cr_log10f(float);
float cr_log2f(float);
float cr_acosf(float);
float cr_asinf(float);
float cr_tanhf(float);
double cr_acos(double);
static uint64_t sm(uint64_t *s)
{
  uint64_t z = (*s += 0x9e3779b97f4a7c15ULL);
  z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ULL;
  z = (z ^ (z >> 27)) * 0x94d049bb133111ebULL;
  return z ^ (z >> 31);
}
static double u01(uint64_t *s) { return (double)(sm(s) >> 11) * 0x1p-53; }
static uint64_t b64(double v) { uint64_t b; memcpy(&b, &v, 8); return b; }
static uint32_t b32(float v) { uint32_t b; memcpy(&b, &v, 4); return b; }
static int same64(double a, double b) { return b64(a) == b64(b) || (isnan(a) && isnan(b)); }
static int same32(float a, float b) { return b32(a) == b32(b) || (isnan(a) && isnan(b)); }
#define N (1L << 24)
int main(void)
{
  uint64_t s = 0x6672656573757266ULL; /* "freesurf" */
  long d;
  /* atan2: uniform on [-1, 1]^2, and on [-1000, 1000]^2 */
  d = 0;
  for (long i = 0; i < N; i++) { double y = 2 * u01(&s) - 1, x = 2 * u01(&s) - 1; d += !same64(atan2(y, x), cr_atan2(y, x)); }
  printf("atan2 (y, x uniform on [-1, 1]): %ld of %ld differ\n", d, N);
  d = 0;
  for (long i = 0; i < N; i++) { double y = 2000 * u01(&s) - 1000, x = 2000 * u01(&s) - 1000; d += !same64(atan2(y, x), cr_atan2(y, x)); }
  printf("atan2 (y, x uniform on [-1000, 1000]): %ld of %ld differ\n", d, N);
  /* log10, log2: log-uniform magnitudes 2^-40 .. 2^40 */
  long d10 = 0, d2 = 0;
  for (long i = 0; i < N; i++) {
    double x = ldexp(1 + u01(&s), (int)(sm(&s) % 81) - 40);
    d10 += !same64(log10(x), cr_log10(x));
    d2 += !same64(log2(x), cr_log2(x));
  }
  printf("log10 (log-uniform 2^-40 .. 2^41): %ld of %ld differ\nlog2 (same arguments): %ld of %ld differ\n", d10, N, d2, N);
  /* the float functions: every float in their domain (all 2^32 patterns for log10f, log2f, tanhf;
     acosf and asinf on [-1, 1]) */
  long f10 = 0, f2 = 0, th = 0, ac = 0, as = 0, n1 = 0;
  for (uint64_t k = 0; k < (1ULL << 32); k++) {
    uint32_t b = (uint32_t)k;
    float x;
    memcpy(&x, &b, 4);
    f10 += !same32(log10f(x), cr_log10f(x));
    f2 += !same32(log2f(x), cr_log2f(x));
    th += !same32(tanhf(x), cr_tanhf(x));
    if (fabsf(x) <= 1) { n1++; ac += !same32(acosf(x), cr_acosf(x)); as += !same32(asinf(x), cr_asinf(x)); }
  }
  printf("log10f (every float): %ld of 4294967296 differ\nlog2f (every float): %ld differ\ntanhf (every float): %ld differ\n", f10, f2, th);
  printf("acosf (every float in [-1, 1]): %ld of %ld differ\nasinf (same): %ld differ\n", ac, n1, as);
  /* FreeSurfer's finite sets: the table of fastApproxAtan2f, (float) atan2(i, 100000) for i in
     [0, 100000] */
  d = 0;
  long dd = 0;
  for (int i = 0; i <= 100000; i++) {
    double a = atan2((double)i, 100000.0), c = cr_atan2((double)i, 100000.0);
    dd += !same64(a, c);
    d += !same32((float)a, (float)c);
  }
  printf("fastApproxAtan2f's table, atan2(i, 100000), i = 0 .. 100000: %ld doubles differ, %ld floats differ\n", dd, d);
  return 0;
}
